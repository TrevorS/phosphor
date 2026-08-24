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

use phosphor_core::request::DiffMode;
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
}

/// Cells of air at the left of a file row. Hunks and lines inset further.
pub const PAD_COLS: u16 = 0;

/// Cells a hunk header is indented past its file.
const HUNK_INDENT: u16 = 2;

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
    /// Workspace-relative path, as the header draws it.
    pub path: String,
    /// `· 3 regions`, or whatever the host counted. Absent draws nothing.
    pub annotation: Option<String>,
    /// Collapsed to its header row.
    pub folded: bool,
    /// The hunks, in file order.
    pub hunks: Vec<Hunk>,
}

/// What the diff surface draws (`T063`).
///
/// A ViewModel: derived from the store, read-only, rebuilt when it moves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffVm {
    /// `4b`'s first row — `review — ✻ retry logic · 2 files · 6 regions`. Empty
    /// draws no header, which is `2b`'s hunk peek: a float three lines tall has
    /// no room for one and no need.
    pub header: String,
    /// The files, in the order the host grouped them.
    pub files: Vec<File>,
}

impl phosphor_core::vm::ViewModel for DiffVm {}

/// A diff, as a surface body.
#[derive(Debug, Clone, Copy)]
pub struct DiffBody<'a> {
    vm: &'a DiffVm,
    theme: &'a Theme,
    mode: DiffMode,
}

impl<'a> DiffBody<'a> {
    /// A body over `vm`, unified.
    #[must_use]
    pub const fn new(vm: &'a DiffVm, theme: &'a Theme) -> Self {
        Self {
            vm,
            theme,
            mode: DiffMode::Unified,
        }
    }

    /// Which of the two shapes to draw.
    #[must_use]
    pub const fn mode(mut self, mode: DiffMode) -> Self {
        self.mode = mode;
        self
    }

    /// Rows this body wants.
    ///
    /// **Measured rather than drawn twice**, for the reason every other body
    /// here is: a float sizes itself before it paints.
    #[must_use]
    pub fn desired_height(&self) -> u16 {
        let mut rows = u16::from(!self.vm.header.is_empty());
        for file in &self.vm.files {
            rows = rows.saturating_add(1);
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

        for file in &self.vm.files {
            if y >= area.bottom() {
                return;
            }
            self.file_row(file, area, y, buf);
            y += 1;
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
    /// `▾ src/retry.rs                       · 3 regions`.
    fn file_row(&self, file: &File, area: Rect, y: u16, buf: &mut Buffer) {
        let mark = if file.folded {
            glyph::FOLDED
        } else {
            glyph::OPEN
        };
        let meta = Style::new().fg(self.theme.neutrals.meta);
        let after = write(buf, area, area.x + PAD_COLS, y, mark, meta);
        write(
            buf,
            area,
            after + 1,
            y,
            &file.path,
            Style::new().fg(self.theme.neutrals.text),
        );
        if let Some(annotation) = &file.annotation {
            let at = area
                .right()
                .saturating_sub(cells(annotation))
                .saturating_sub(PAD_COLS);
            write(buf, area, at, y, annotation, meta);
        }
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
    use super::{Change, DiffBody, DiffVm, File, Hunk, Line};
    use crate::theme::Theme;
    use phosphor_core::request::DiffMode;
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
            header: "review — retry logic · 1 file".to_owned(),
            files: vec![File {
                path: "src/fetch.rs".to_owned(),
                annotation: Some("· 3 regions".to_owned()),
                folded: false,
                hunks: vec![hunk],
            }],
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
