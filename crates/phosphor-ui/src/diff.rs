//! `DiffBody` (`T063`) — a diff as a surface, unified or side by side.
//!
//! Draws [`phosphor_core::view::Node::Diff`]: a header, then one group per file,
//! then the hunks inside it. `4b` is the screen this is the body of — *"one
//! review block as one unified diff · folds for the bulk · `s` marks seen
//! piecewise"* — and `2b`'s hunk peek is the same rows in a float three lines
//! tall.
//!
//! # Why there is no diff *algorithm* here
//!
//! `T008`'s spike went looking for a widget to restyle and found the vendored
//! editor's `mod diff` private, with the diff implemented as a *mode of the
//! Editor* — nothing to reuse. What it also found is that `similar` is already
//! in the tree, so computing one costs no dependency.
//!
//! **It is still not computed here.** A widget crate cannot read a file, and the
//! two sides of a diff are a *buffer* and a *disk copy*, or two revisions —
//! things the host holds. So the rows arrive through
//! [`Resources::diff`](crate::interpret::Resources::diff), the same division
//! `Node::Picker` and `Node::Transcript` already draw, and this file's whole job
//! is what a hunk looks like.
//!
//! # `4b`, read out
//!
//! ```text
//! review — ✻ retry logic · 2 files · 6 regions · 2 seen ✓
//! ▾ src/retry.rs                                    · 3 regions
//!   @@ 4
//! + use crate::util::jitter;
//!   @@ 6–10                                                seen ✓
//! + pub struct RetryPolicy {
//!   @@ 12–24 · retry_with_backoff             ⋯ folded · 13 lines
//! ```
//!
//! **A folded hunk is one row and says how many it stands for**, which is §11's
//! *"scale is grouping, not scrolling"* — the same shape the transcript's
//! `⋯ N earlier turn(s)` and the help grid's `and N more` already take.

use phosphor_core::request::{DiffMode, Grouping};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// §2's glyphs, as this surface spends them.
mod glyph {
    /// `▾` — an open file group. `4b` draws every one of them open.
    pub(super) const OPEN: &str = "▾";
    /// `▸` — a closed one.
    pub(super) const FOLDED: &str = "▸";
    /// `⋯` — a hunk folded to one row (§11's drop, made visible).
    pub(super) const ELIDED: &str = "⋯";
    /// `✓` — seen. §2's check, and the one place this surface spends it.
    pub(super) const SEEN: &str = "✓";
    /// `●` — §2's unseen dot, in front of a count (`T065`).
    pub(super) const UNSEEN: &str = "●";
}

/// Cells of air at the left of a file row. Hunks and lines inset further.
pub const PAD_COLS: u16 = 0;

/// Cells a hunk header is indented past its file.
const HUNK_INDENT: u16 = 2;

/// Cells one level of grouping insets a file's name (`T065`).
const NEST_COLS: u16 = 2;

/// What one row of a diff is.
///
/// **Three kinds and not two.** A unified diff is mostly *context* — the
/// unchanged lines that say where a change is — and drawing it as neither added
/// nor removed is what makes the `+` and `−` columns mean something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Present in both sides. Drawn in the text neutral with no sign.
    Context,
    /// Added — `+`, in you-blue. §1: your side of a diff is blue.
    Added,
    /// Removed — `−`, in trouble-red.
    Removed,
}

impl Change {
    /// The sign column, which is one cell wide for all three.
    const fn sign(self) -> &'static str {
        match self {
            Self::Context => " ",
            Self::Added => "+",
            // **U+2212, not a hyphen.** `4b` draws `−`, and the counts in `1b`
            // spell it the same way — one minus in the product.
            Self::Removed => "−",
        }
    }
}

/// One line of a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Which side it is on.
    pub change: Change,
    /// The text, without its sign — the sign is a column, not a prefix.
    pub text: String,
}

/// One hunk: a run of changed lines and the context around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// `4`, or `6–10` — the line range in the new file, as `4b` spells it with
    /// an en dash.
    pub range: String,
    /// What the hunk is *in*, when something knows — `retry_with_backoff`.
    /// Drawn after the range, and absent rather than guessed.
    pub label: Option<String>,
    /// Whether this hunk has been marked seen (`T064`).
    pub seen: bool,
    /// Collapsed to one row that says how many lines it stands for.
    pub folded: bool,
    /// The lines, in file order.
    pub lines: Vec<Line>,
}

/// One file's hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The path as this row draws it.
    ///
    /// **Whatever the host put here, verbatim.** `8b` draws
    /// `handlers/users.rs` under a `src/api/` group and `src/errors.rs` at the
    /// top level, so a row's path is relative to *where it sits* — which only
    /// the thing that built the tree knows. Trimming a prefix here would guess.
    pub path: String,
    /// `· 3 regions`, or claude's own note about the file. Absent draws
    /// nothing.
    pub annotation: Option<String>,
    /// `●4` — how many of this file's hunks are unseen (`T065`).
    ///
    /// [`None`] draws no chip, which is not the same as `Some(0)`: a file
    /// nobody counted and a file with nothing left to read are different
    /// facts, and `4b` shows the first while `8b` shows the second.
    pub unseen: Option<u32>,
    /// Collapsed to its header row.
    pub folded: bool,
    /// The hunks, in file order.
    pub hunks: Vec<Hunk>,
}

/// A run of files drawn as one row (`T065`).
///
/// `⋯ 12 more files, same pattern · S here marks all 12` — §11's *"scale is
/// grouping, not scrolling"* at the file level, the same shape a folded hunk
/// takes at the line level.
///
/// **The host decides what to elide and says so; this only draws it.** Which
/// twelve of forty files are *"the same pattern"* is a judgement about the
/// change, and a widget that picked the tail of the list would be asserting one
/// it cannot make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elided {
    /// How many files this row stands for.
    pub files: usize,
    /// `same pattern` — why they were collapsed together. Absent draws the
    /// count alone.
    pub note: Option<String>,
    /// `S here marks all 12` — the key that acts on them, spelled whole per
    /// Design Language §6. Absent draws nothing.
    pub hint: Option<String>,
}

/// One group of files — `8b`'s `▾ src/api/` row and what hangs off it (`T065`).
///
/// **The grouping is claude's, not the filesystem's**, which is the finding
/// that shaped this type. `8b` draws `src/errors.rs` as a *peer* of `src/api/`
/// and `src/db/` even though its parent directory is `src/` — so the tree
/// cannot be derived from the paths, and a widget that grouped by parent would
/// draw a different screen from the one the mockup draws. The host groups and
/// annotates; this draws what it is handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// `src/api/` — the group's name, as `8b` spells it with its trailing
    /// slash.
    pub path: String,
    /// `the meat: handler signatures`. Claude's, through `annotate-group`.
    pub annotation: Option<String>,
    /// `●31 unseen` — how many unseen hunks are under this group.
    pub unseen: u32,
    /// How many files it holds *in total*, which is not `files.len()` when some
    /// were elided. `8b`'s `14 files` over two drawn rows and a `⋯ 12 more`.
    pub files: usize,
    /// Collapsed to its own row.
    pub folded: bool,
    /// The files drawn under it, in the host's order.
    pub children: Vec<File>,
    /// The rest of them, as one row.
    pub elided: Option<Elided>,
}

/// One row of the surface's top level.
///
/// **One list and not two**, because `8b` interleaves them: `src/api/`,
/// `src/db/`, then the bare file `src/errors.rs`, then `tests/`. Two fields
/// would lose that order, and the order is claude's statement about what to
/// read first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A file with no group above it — `4b`'s whole shape, and `8b`'s
    /// `src/errors.rs`.
    File(File),
    /// A group of files (`T065`).
    Group(Group),
}

/// What the diff surface draws (`T063`, grouped at `T065`).
///
/// A ViewModel: derived from the store, read-only, rebuilt when it moves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffVm {
    /// `4b`'s first row — `review — ✻ retry logic · 2 files · 6 regions`. Empty
    /// draws no header, which is `2b`'s hunk peek: a float three lines tall has
    /// no room for one and no need.
    pub header: String,
    /// The top level, in the order the host built it.
    pub entries: Vec<Entry>,
    /// Which top-level row is highlighted, 0-based (`T065`).
    ///
    /// **`8b` draws no highlight and this exists anyway.** The mockup is a
    /// snapshot of a screen at rest; `za fold · s seen · S group seen` in its
    /// own footer are all verbs that act on *a* row, and a surface where you
    /// cannot see which row that is has three keys and no way to aim them.
    /// The picker spends the theme's selection ground on the same job.
    ///
    /// [`None`] draws none, which is `4b` and `2b` — bodies you read rather
    /// than steer.
    pub selected: Option<usize>,
}

impl phosphor_core::vm::ViewModel for DiffVm {}

/// One drawn row of the top level, after grouping has been applied (`T065`).
///
/// Built once by [`DiffBody::rows`] and walked by both the measure and the
/// render, so *"how tall is this"* and *"what did it draw"* cannot answer
/// differently.
#[derive(Debug, Clone, Copy)]
enum Row<'a> {
    /// `▸   handlers/users.rs   ●4  Result<_, ApiError> throughout`.
    File {
        /// The file.
        file: &'a File,
        /// How many levels in — `0` at the top, `1` under a group.
        indent: u16,
    },
    /// `▾ src/api/        ●31 unseen · 14 files · the meat: …`.
    Group(&'a Group),
    /// `⋯ 12 more files, same pattern · S here marks all 12`.
    Elided(&'a Elided),
}

/// A diff, as a surface body.
#[derive(Debug, Clone, Copy)]
pub struct DiffBody<'a> {
    vm: &'a DiffVm,
    theme: &'a Theme,
    mode: DiffMode,
    grouping: Grouping,
}

impl<'a> DiffBody<'a> {
    /// A body over `vm`, unified.
    #[must_use]
    pub const fn new(vm: &'a DiffVm, theme: &'a Theme) -> Self {
        Self {
            vm,
            theme,
            mode: DiffMode::Unified,
            grouping: Grouping::Directory,
        }
    }

    /// Which of the two shapes to draw.
    #[must_use]
    pub const fn mode(mut self, mode: DiffMode) -> Self {
        self.mode = mode;
        self
    }

    /// Whether to draw the group rows or flatten past them (`T065`).
    ///
    /// **[`Grouping::Flat`] does not rebuild the tree — it skips the group
    /// rows and draws their files at the top level.** The order is the host's
    /// either way; what flattening drops is a level of indent, the fold rows
    /// and the elisions. It is `8d`'s answer at 80 columns and `4b`'s ordinary
    /// shape, not a different set of files.
    #[must_use]
    pub const fn grouping(mut self, grouping: Grouping) -> Self {
        self.grouping = grouping;
        self
    }

    /// The top level as it will be drawn: groups kept, or flattened away.
    ///
    /// Answers `(indent, file)` pairs interleaved with the group rows, so the
    /// measure and the render walk one sequence and cannot disagree about how
    /// many rows there are — which is the bug `desired_height` exists to not
    /// have.
    fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        for entry in &self.vm.entries {
            match entry {
                Entry::File(file) => rows.push(Row::File { file, indent: 0 }),
                Entry::Group(group) => match self.grouping {
                    Grouping::Flat => {
                        for file in &group.children {
                            rows.push(Row::File { file, indent: 0 });
                        }
                    }
                    Grouping::Directory => {
                        rows.push(Row::Group(group));
                        if group.folded {
                            continue;
                        }
                        for file in &group.children {
                            rows.push(Row::File { file, indent: 1 });
                        }
                        if let Some(elided) = &group.elided {
                            rows.push(Row::Elided(elided));
                        }
                    }
                },
            }
        }
        rows
    }

    /// Rows this body wants.
    ///
    /// **Measured rather than drawn twice**, for the reason every other body
    /// here is: a float sizes itself before it paints.
    #[must_use]
    pub fn desired_height(&self) -> u16 {
        let mut rows = u16::from(!self.vm.header.is_empty());
        for row in self.rows() {
            rows = rows.saturating_add(1);
            let Row::File { file, .. } = row else {
                continue;
            };
            if file.folded {
                continue;
            }
            for hunk in &file.hunks {
                rows = rows.saturating_add(1);
                if !hunk.folded {
                    rows = rows
                        .saturating_add(u16::try_from(self.drawn_rows(hunk)).unwrap_or(u16::MAX));
                }
            }
        }
        rows
    }

    /// How many rows a hunk's lines occupy in the current mode.
    ///
    /// **Side by side is not half as tall.** A removal and the addition that
    /// replaced it share a row; a run of three removals against one addition
    /// does not pair up three times. So the count is the longer of the two
    /// columns, computed the same way [`DiffBody::paired`] draws them.
    fn drawn_rows(&self, hunk: &Hunk) -> usize {
        match self.mode {
            DiffMode::Unified => hunk.lines.len(),
            DiffMode::SideBySide => Self::paired(hunk).len(),
        }
    }

    /// The lines as left/right pairs, for [`DiffMode::SideBySide`].
    ///
    /// **Context lines appear on both sides**, which is what makes the two
    /// columns readable as one file: a row with text on the left and nothing on
    /// the right is a *deletion*, and if context were one-sided every row would
    /// look like one.
    fn paired(hunk: &Hunk) -> Vec<(Option<&Line>, Option<&Line>)> {
        fn flush<'l>(
            rows: &mut Vec<(Option<&'l Line>, Option<&'l Line>)>,
            removed: &mut Vec<&'l Line>,
            added: &mut Vec<&'l Line>,
        ) {
            for index in 0..removed.len().max(added.len()) {
                rows.push((removed.get(index).copied(), added.get(index).copied()));
            }
            removed.clear();
            added.clear();
        }

        let mut rows = Vec::new();
        let mut removed: Vec<&Line> = Vec::new();
        let mut added: Vec<&Line> = Vec::new();
        for line in &hunk.lines {
            match line.change {
                Change::Removed => removed.push(line),
                Change::Added => added.push(line),
                Change::Context => {
                    flush(&mut rows, &mut removed, &mut added);
                    rows.push((Some(line), Some(line)));
                }
            }
        }
        flush(&mut rows, &mut removed, &mut added);
        rows
    }
}

impl Widget for DiffBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let meta = Style::new().fg(self.theme.neutrals.meta);
        let mut y = area.y;

        if !self.vm.header.is_empty() {
            write(buf, area, area.x + PAD_COLS, y, &self.vm.header, meta);
            y += 1;
        }

        for (index, row) in self.rows().into_iter().enumerate() {
            if y >= area.bottom() {
                return;
            }
            // **Painted before the text, across the whole width.** The picker
            // does the same and for the same reason: a highlight that stopped
            // at the end of the longest run would draw a ragged right edge.
            if self.vm.selected == Some(index) {
                let ground = Style::new().bg(self.theme.regions.selection);
                for x in area.x..area.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(ground);
                    }
                }
            }
            let file = match row {
                Row::Group(group) => {
                    self.group_row(group, area, y, buf);
                    y += 1;
                    continue;
                }
                Row::Elided(elided) => {
                    self.elided_row(elided, area, y, buf);
                    y += 1;
                    continue;
                }
                Row::File { file, indent } => {
                    self.file_row(file, indent, area, y, buf);
                    y += 1;
                    file
                }
            };
            if file.folded {
                continue;
            }
            for hunk in &file.hunks {
                if y >= area.bottom() {
                    return;
                }
                self.hunk_row(hunk, area, y, buf);
                y += 1;
                if hunk.folded {
                    continue;
                }
                y = self.lines(hunk, area, y, buf);
            }
        }
    }
}

impl DiffBody<'_> {
    /// `▾ src/retry.rs                       · 3 regions`, and `8b`'s indented
    /// `▸   handlers/users.rs   ●4  Result<_, ApiError> throughout`.
    fn file_row(&self, file: &File, indent: u16, area: Rect, y: u16, buf: &mut Buffer) {
        let mark = if file.folded {
            glyph::FOLDED
        } else {
            glyph::OPEN
        };
        let meta = Style::new().fg(self.theme.neutrals.meta);
        // **The arrow stays in the left column at every depth, and the name
        // moves.** `8b` draws `▸   handlers/users.rs` under `▾ src/api/` with
        // the two arrows lined up, which is what makes a column of them
        // scannable — indenting the arrow too would turn the fold state into a
        // staircase.
        let after = write(buf, area, area.x + PAD_COLS, y, mark, meta);
        let x = after + 1 + indent.saturating_mul(NEST_COLS);
        let after = write(
            buf,
            area,
            x,
            y,
            &file.path,
            Style::new().fg(self.theme.neutrals.text),
        );
        let mut right = after;
        if let Some(unseen) = file.unseen {
            right = write(
                buf,
                area,
                right + 2,
                y,
                &format!("{}{unseen}", glyph::UNSEEN),
                Style::new().fg(self.theme.actors.claude),
            );
        }
        if let Some(annotation) = &file.annotation {
            // **After the chip when there is one, right-aligned when there is
            // not.** `4b`'s `· 3 regions` is a count against the right edge;
            // `8b`'s `Result<_, ApiError> throughout` is claude's sentence and
            // reads left-to-right after the dot. One field, two placements, and
            // the chip is what tells them apart.
            let at = if file.unseen.is_some() {
                right + 2
            } else {
                area.right()
                    .saturating_sub(cells(annotation))
                    .saturating_sub(PAD_COLS)
            };
            write(buf, area, at, y, annotation, meta);
        }
    }

    /// `▾ src/api/        ●31 unseen · 14 files · the meat: handler
    /// signatures` (`T065`).
    ///
    /// **`seen ✓` replaces the count rather than joining it.** `8b` draws
    /// `tests/  seen ✓ · 17 files` — a group with nothing left to read says so
    /// instead of saying `●0 unseen`, which is the same rule the statusline's
    /// counter follows and the reason `File::unseen` is an `Option`.
    fn group_row(&self, group: &Group, area: Rect, y: u16, buf: &mut Buffer) {
        let meta = Style::new().fg(self.theme.neutrals.meta);
        let mark = if group.folded {
            glyph::FOLDED
        } else {
            glyph::OPEN
        };
        let after = write(buf, area, area.x + PAD_COLS, y, mark, meta);
        let after = write(
            buf,
            area,
            after + 1,
            y,
            &group.path,
            Style::new().fg(self.theme.neutrals.text),
        );

        let mut parts = Vec::new();
        if group.unseen == 0 {
            parts.push(format!("seen {}", glyph::SEEN));
        } else {
            parts.push(format!("{}{} unseen", glyph::UNSEEN, group.unseen));
        }
        parts.push(format!("{} files", group.files));
        if let Some(annotation) = &group.annotation {
            parts.push(annotation.clone());
        }
        // Two cells of air after the longest name would need a second pass over
        // every row; one column is enough to keep the counts off the names.
        write(buf, area, after + 2, y, &parts.join(" · "), meta);
    }

    /// `⋯ 12 more files, same pattern · S here marks all 12` (`T065`).
    fn elided_row(&self, elided: &Elided, area: Rect, y: u16, buf: &mut Buffer) {
        let meta = Style::new().fg(self.theme.neutrals.meta);
        let mut said = format!("{} {} more files", glyph::ELIDED, elided.files);
        if let Some(note) = &elided.note {
            said.push_str(", ");
            said.push_str(note);
        }
        if let Some(hint) = &elided.hint {
            said.push_str(" · ");
            said.push_str(hint);
        }
        write(buf, area, area.x + PAD_COLS + 1 + NEST_COLS, y, &said, meta);
    }

    /// `  @@ 6–10                                    seen ✓`, and the folded
    /// form `  @@ 12–24 · retry_with_backoff   ⋯ folded · 13 lines`.
    fn hunk_row(&self, hunk: &Hunk, area: Rect, y: u16, buf: &mut Buffer) {
        let meta = Style::new().fg(self.theme.neutrals.meta);
        let x = area.x + PAD_COLS + HUNK_INDENT;
        let head = match &hunk.label {
            Some(label) => format!("@@ {} · {label}", hunk.range),
            None => format!("@@ {}", hunk.range),
        };
        write(buf, area, x, y, &head, meta);

        // **The right-hand note is one of two and never both.** A folded hunk
        // says how many lines it stands for; an open one says whether it has
        // been read. Drawing both would put two different kinds of fact in one
        // column.
        let note = if hunk.folded {
            Some((
                format!("{} folded · {} lines", glyph::ELIDED, hunk.lines.len()),
                meta,
            ))
        } else if hunk.seen {
            Some((
                format!("seen {}", glyph::SEEN),
                Style::new().fg(self.theme.neutrals.meta),
            ))
        } else {
            None
        };
        if let Some((note, style)) = note {
            let at = area
                .right()
                .saturating_sub(cells(&note))
                .saturating_sub(PAD_COLS);
            write(buf, area, at, y, &note, style);
        }
    }

    /// A hunk's lines, in whichever shape the mode asks for. Answers the row
    /// after the last one drawn.
    fn lines(&self, hunk: &Hunk, area: Rect, top: u16, buf: &mut Buffer) -> u16 {
        let mut y = top;
        match self.mode {
            DiffMode::Unified => {
                for line in &hunk.lines {
                    if y >= area.bottom() {
                        return y;
                    }
                    self.line(line, area.x + PAD_COLS, area, y, buf);
                    y += 1;
                }
            }
            DiffMode::SideBySide => {
                // Halved down the middle, with a cell of air between. The
                // columns are equal because neither side is the subject.
                let half = area.width.saturating_sub(1) / 2;
                let right = Rect {
                    x: area.x.saturating_add(half).saturating_add(1),
                    width: half,
                    ..area
                };
                let left = Rect {
                    width: half,
                    ..area
                };
                for (was, is) in Self::paired(hunk) {
                    if y >= area.bottom() {
                        return y;
                    }
                    if let Some(line) = was {
                        self.line(line, left.x, left, y, buf);
                    }
                    if let Some(line) = is {
                        self.line(line, right.x, right, y, buf);
                    }
                    y += 1;
                }
            }
        }
        y
    }

    /// One line: its sign, then its text.
    fn line(&self, line: &Line, x: u16, area: Rect, y: u16, buf: &mut Buffer) {
        // §1: your side of a diff is you-blue and what went is trouble-red.
        // Context is the text neutral — it is the file, not the change.
        let style = Style::new().fg(match line.change {
            Change::Context => self.theme.neutrals.text,
            Change::Added => self.theme.actors.you,
            Change::Removed => self.theme.actors.trouble,
        });
        let after = write(buf, area, x, y, line.change.sign(), style);
        write(buf, area, after + 1, y, &line.text, style);
    }
}

/// Write `text` at `x` on row `y`, clipped to the area. Returns the column after
/// the last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, y, text, room as usize, style);
    next.min(area.right())
}

#[cfg(test)]
mod tests {
    use super::{Change, DiffBody, DiffVm, Elided, Entry, File, Group, Hunk, Line};
    use crate::theme::Theme;
    use phosphor_core::request::{DiffMode, Grouping};
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::widgets::Widget;

    /// `4b`'s own before and after, shortened to the part that changes.
    const WAS: &str = "let resp = client.get(url).send()?;\nresp.json().await\n";
    const NOW: &str = "let policy = RetryPolicy::default();\nlet resp = retry_with_backoff(op, &policy)?;\nresp.json().await\n";

    /// **A real diff, computed by `similar`.**
    ///
    /// The point of using it in the *test* rather than in the widget: a
    /// hand-written `Vec<Line>` proves the renderer against the test author's
    /// idea of a diff, and this proves it against one. `T063`'s acceptance is
    /// *"renders a real diff correctly"*, and this is where the word real is
    /// spent.
    fn real_hunk() -> Hunk {
        let diff = similar::TextDiff::from_lines(WAS, NOW);
        let lines = diff
            .iter_all_changes()
            .map(|change| Line {
                change: match change.tag() {
                    similar::ChangeTag::Delete => Change::Removed,
                    similar::ChangeTag::Insert => Change::Added,
                    similar::ChangeTag::Equal => Change::Context,
                },
                text: change.value().trim_end_matches('\n').to_owned(),
            })
            .collect();
        Hunk {
            range: "3–7".to_owned(),
            label: None,
            seen: false,
            folded: false,
            lines,
        }
    }

    fn vm(hunk: Hunk) -> DiffVm {
        DiffVm {
            selected: None,
            header: "review — retry logic · 1 file".to_owned(),
            entries: vec![Entry::File(File {
                path: "src/fetch.rs".to_owned(),
                annotation: Some("· 3 regions".to_owned()),
                unseen: None,
                folded: false,
                hunks: vec![hunk],
            })],
        }
    }

    fn drawn(vm: &DiffVm, mode: DiffMode, width: u16, height: u16) -> Vec<String> {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        DiffBody::new(vm, &theme).mode(mode).render(area, &mut buf);
        (0..height)
            .map(|row| {
                (0..width)
                    .map(|col| buf[(col, row)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    /// **A unified diff draws the signs `4b` draws**, and draws context with
    /// neither.
    #[test]
    fn a_real_diff_renders_its_three_kinds_of_line() {
        let vm = vm(real_hunk());
        let rows = drawn(&vm, DiffMode::Unified, 80, 12);
        let body = rows.join("\n");

        assert!(
            body.contains("− let resp = client.get(url).send()?;"),
            "what went is a minus; body was:\n{body}"
        );
        assert!(
            body.contains("+ let policy = RetryPolicy::default();"),
            "what arrived is a plus; body was:\n{body}"
        );
        // **The unchanged line carries no sign**, which is what makes the other
        // two mean something. `similar` calls it `Equal`; `4b` draws it plain.
        assert!(
            rows.iter().any(|row| row.trim() == "resp.json().await"),
            "and context is neither; body was:\n{body}"
        );
        // `−` is U+2212 and not a hyphen — one minus in the product.
        assert!(!body.contains("- let resp"), "the sign is a real minus");
    }

    /// The chrome around the hunks: the header, the file row, the range.
    #[test]
    fn the_file_row_and_the_hunk_header_say_where_the_change_is() {
        let vm = vm(real_hunk());
        let rows = drawn(&vm, DiffMode::Unified, 80, 12);
        assert_eq!(rows[0], "review — retry logic · 1 file");
        assert!(rows[1].starts_with("▾ src/fetch.rs"), "{:?}", rows[1]);
        // Right-aligned, because it is a count rather than a name.
        assert!(rows[1].ends_with("· 3 regions"), "{:?}", rows[1]);
        assert_eq!(rows[2].trim(), "@@ 3–7");
    }

    /// **A folded hunk is one row and says what it stands for** — §11's *"scale
    /// is grouping, not scrolling"*, and the same shape the transcript and the
    /// help grid already take.
    #[test]
    fn a_folded_hunk_is_one_row_that_counts_what_it_hides() {
        let mut hunk = real_hunk();
        let count = hunk.lines.len();
        hunk.folded = true;
        hunk.label = Some("fetch_json".to_owned());
        let vm = vm(hunk);
        let rows = drawn(&vm, DiffMode::Unified, 80, 12);
        let body = rows.join("\n");

        assert!(
            body.contains(&format!("⋯ folded · {count} lines")),
            "a folded hunk counts what it hides; body was:\n{body}"
        );
        assert!(
            body.contains("@@ 3–7 · fetch_json"),
            "and keeps its label; body was:\n{body}"
        );
        // And none of its lines are drawn.
        assert!(
            !body.contains("RetryPolicy::default()"),
            "a folded hunk draws no lines; body was:\n{body}"
        );
    }

    /// **Side by side pairs a removal with what replaced it**, and context
    /// appears on *both* sides.
    ///
    /// That second half is what makes two columns readable as one file: a row
    /// with text on the left and nothing on the right means *deleted*, and if
    /// context were one-sided every row would look like one.
    #[test]
    fn side_by_side_pairs_the_two_sides_and_repeats_the_context() {
        let vm = vm(real_hunk());
        let rows = drawn(&vm, DiffMode::SideBySide, 100, 12);

        let paired = rows
            .iter()
            .find(|row| row.contains("client.get(url).send()?;"))
            .expect("the removed line is drawn");
        assert!(
            paired.contains("RetryPolicy::default()"),
            "the removal and its replacement share a row: {paired:?}"
        );

        let context = rows
            .iter()
            .find(|row| row.matches("resp.json().await").count() == 2)
            .expect("context is on both sides");
        assert!(!context.is_empty());
    }

    // -- `T065`, the 40-file block ------------------------------------------

    fn leaf(path: &str, unseen: u32, note: &str) -> File {
        File {
            path: path.to_owned(),
            annotation: Some(note.to_owned()),
            unseen: Some(unseen),
            folded: true,
            hunks: Vec::new(),
        }
    }

    /// **Screen `8b`, built from the mockup's own numbers.**
    ///
    /// 41 files across four top-level rows, of which one — `src/errors.rs` — is
    /// a *bare file beside the groups* rather than under one. That is the
    /// detail this whole shape exists for: its parent directory is `src/`, and
    /// a widget that grouped by parent would have filed it under a `src/` row
    /// the mockup does not draw.
    fn forty_files() -> DiffVm {
        DiffVm {
            selected: None,
            header: "review — ✻ migrate error handling · 41 files · 96 regions · 12 seen ✓"
                .to_owned(),
            entries: vec![
                Entry::Group(Group {
                    path: "src/api/".to_owned(),
                    annotation: Some("the meat: handler signatures".to_owned()),
                    unseen: 31,
                    files: 14,
                    folded: false,
                    children: vec![
                        leaf("handlers/users.rs", 4, "Result<_, ApiError> throughout"),
                        leaf("handlers/orders.rs", 3, "same shape"),
                    ],
                    elided: Some(Elided {
                        files: 12,
                        note: Some("same pattern".to_owned()),
                        hint: Some("S here marks all 12".to_owned()),
                    }),
                }),
                Entry::Group(Group {
                    path: "src/db/".to_owned(),
                    annotation: Some("mechanical: ? → map_err".to_owned()),
                    unseen: 22,
                    files: 9,
                    folded: true,
                    children: vec![leaf("queries.rs", 22, "not drawn — the group is folded")],
                    elided: None,
                }),
                Entry::File(File {
                    path: "src/errors.rs".to_owned(),
                    annotation: Some("the new ApiError enum — read this one".to_owned()),
                    unseen: Some(1),
                    folded: true,
                    hunks: Vec::new(),
                }),
                Entry::Group(Group {
                    path: "tests/".to_owned(),
                    annotation: None,
                    unseen: 0,
                    files: 17,
                    folded: true,
                    children: Vec::new(),
                    elided: None,
                }),
            ],
        }
    }

    /// **`8b` is 41 files in eight rows**, which is §11's *"scale is grouping,
    /// not scrolling"* stated as a number rather than a principle.
    ///
    /// Counted off the mockup rather than reasoned about: a header, three group
    /// rows, the two files drawn under the open one, their elision, and the
    /// bare `src/errors.rs`. The first version of this test asserted six and
    /// the render was right.
    #[test]
    fn forty_one_files_draw_as_eight_rows() {
        let vm = forty_files();
        let rows = drawn(&vm, DiffMode::Unified, 100, 12);
        let used = rows.iter().filter(|row| !row.is_empty()).count();
        assert_eq!(
            used,
            8,
            "header · src/api/ + 2 files + elision · src/db/ · src/errors.rs · tests/:\n{}",
            rows.join("\n")
        );
    }

    /// The group row's three facts, in `8b`'s order and spelling.
    #[test]
    fn a_group_row_counts_what_is_unseen_and_carries_claudes_note() {
        let vm = forty_files();
        let rows = drawn(&vm, DiffMode::Unified, 100, 12);
        let api = rows
            .iter()
            .find(|row| row.contains("src/api/"))
            .expect("the group is drawn");
        assert!(api.starts_with("▾ src/api/"), "{api:?}");
        assert!(
            api.contains("●31 unseen · 14 files · the meat: handler signatures"),
            "{api:?}"
        );
        // `14 files` is the group's total, not the two rows drawn under it.
        assert!(!api.contains("2 files"), "{api:?}");
    }

    /// **A group with nothing left to read says `seen ✓`, not `●0 unseen`.**
    #[test]
    fn a_fully_seen_group_says_so_instead_of_counting_zero() {
        let vm = forty_files();
        let rows = drawn(&vm, DiffMode::Unified, 100, 12);
        let tests = rows
            .iter()
            .find(|row| row.contains("tests/"))
            .expect("the group is drawn");
        assert!(tests.contains("seen ✓ · 17 files"), "{tests:?}");
        assert!(!tests.contains("●0"), "{tests:?}");
    }

    /// **The elision row counts what it hides and names the key that acts on
    /// it**, spelled whole — Design Language §6 forbids `:S` style shorthand
    /// and this is the footer rule applied inside a body.
    #[test]
    fn the_elision_row_says_how_many_and_what_to_press() {
        let vm = forty_files();
        let rows = drawn(&vm, DiffMode::Unified, 100, 12);
        let body = rows.join("\n");
        assert!(
            body.contains("⋯ 12 more files, same pattern · S here marks all 12"),
            "{body}"
        );
    }

    /// **A folded group draws neither its children nor its elision.** `src/db/`
    /// is folded in `8b` and its one child must not appear.
    #[test]
    fn a_folded_group_hides_everything_under_it() {
        let vm = forty_files();
        let body = drawn(&vm, DiffMode::Unified, 100, 12).join("\n");
        assert!(body.contains("▸ src/db/"), "the group row is still drawn");
        assert!(
            !body.contains("queries.rs"),
            "and its child is not:\n{body}"
        );
    }

    /// **The arrows line up and the names step in.**
    ///
    /// A column of fold marks is only scannable if it is a column; indenting
    /// the arrow with the name would turn it into a staircase.
    #[test]
    fn nesting_indents_the_name_and_not_the_arrow() {
        let vm = forty_files();
        let rows = drawn(&vm, DiffMode::Unified, 100, 12);
        let group = rows.iter().find(|row| row.contains("src/api/")).unwrap();
        let child = rows.iter().find(|row| row.contains("users.rs")).unwrap();
        assert_eq!(group.find('▾'), Some(0), "{group:?}");
        assert_eq!(child.find('▸'), Some(0), "{child:?}");
        assert!(
            child.find("handlers").unwrap() > group.find("src/api/").unwrap(),
            "the child's name is further in:\n{group}\n{child}"
        );
    }

    /// **`Grouping::Flat` keeps the files and drops the scaffolding**, which is
    /// what makes it a rendering choice rather than a different query. Same
    /// files, no group rows, no elisions.
    #[test]
    fn flat_grouping_draws_the_files_without_their_groups() {
        let theme = Theme::phosphor_dark();
        let vm = forty_files();
        let area = Rect::new(0, 0, 100, 12);
        let mut buf = Buffer::empty(area);
        DiffBody::new(&vm, &theme)
            .grouping(Grouping::Flat)
            .render(area, &mut buf);
        let body: String = (0..12)
            .map(|row| {
                (0..100)
                    .map(|col| buf[(col, row)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            body.contains("users.rs"),
            "the files are still there:\n{body}"
        );
        assert!(
            body.contains("queries.rs"),
            "including a folded group's, which flattening reveals:\n{body}"
        );
        assert!(!body.contains("src/api/"), "no group rows:\n{body}");
        assert!(!body.contains("12 more files"), "no elisions:\n{body}");
    }

    /// The measure and the render walk one sequence, so they agree under both
    /// groupings — the same claim `T063` makes about the two diff modes, one
    /// level up.
    #[test]
    fn the_measured_height_agrees_under_both_groupings() {
        let theme = Theme::phosphor_dark();
        let vm = forty_files();
        for grouping in [Grouping::Directory, Grouping::Flat] {
            let wanted = DiffBody::new(&vm, &theme)
                .grouping(grouping)
                .desired_height();
            let area = Rect::new(0, 0, 100, 40);
            let mut buf = Buffer::empty(area);
            DiffBody::new(&vm, &theme)
                .grouping(grouping)
                .render(area, &mut buf);
            let used = (0..40)
                .filter(|row| (0..100).any(|col| buf[(col, *row)].symbol().trim() != ""))
                .count();
            assert_eq!(
                usize::from(wanted),
                used,
                "{grouping:?}: measured {wanted} and drew {used}"
            );
        }
    }

    /// **Side by side is not half as tall**, because a run of three removals
    /// against one addition does not pair three times. The measured height and
    /// the drawn rows have to agree, or a float sizes itself wrong.
    #[test]
    fn the_measured_height_is_the_height_it_draws() {
        let theme = Theme::phosphor_dark();
        let vm = vm(real_hunk());
        for mode in [DiffMode::Unified, DiffMode::SideBySide] {
            let wanted = DiffBody::new(&vm, &theme).mode(mode).desired_height();
            let rows = drawn(&vm, mode, 100, 40);
            let used = rows.iter().rposition(|row| !row.is_empty()).unwrap_or(0) + 1;
            assert_eq!(
                usize::from(wanted),
                used,
                "{mode:?}: measured {wanted} and drew {used}"
            );
        }
    }
}
