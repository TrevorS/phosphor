//! `VirtualText` (`T032`) — `┊`-prefixed rows hanging from a region.
//!
//! Draws `Node::VirtualText`: a row owned by a region id (absent for an unowned
//! hint), indented to the code column. Threads, watches, diagnostics and `T035`'s
//! once-per-session unknown-key hint all render through this one primitive.
//!
//! # The constraint that makes it hard
//!
//! A virtual row must interleave without ever shifting the buffer's own line
//! numbering, **and land in the right place on a soft-wrapped line** — the row
//! stream `T081` reshaped is the coordinate space it has to speak, so placement
//! is a variant within that stream and never a layer over it. `CP-3` is the first
//! checkpoint that can see it.
//!
//! # Two halves, and only one of them is a widget
//!
//! **Placement** is [`install`]: a [`Row`] names a buffer position, and the
//! vendored fork inserts a `VisualRow::Virtual` under the row that shows it
//! (`VENDOR.md` patch 8). That is the half `T081` legislated. It is not drawn
//! here — [`BufferView`] draws it, because it is one of the buffer's rows, and
//! the fork's renderer is the one place that walks them.
//!
//! **Drawing a standalone row** is [`VirtualText`], the widget behind
//! `Node::VirtualText`. The protocol's node carries an owner and its content
//! and *no anchor* (`view.rs`), so a `VirtualText` node in a tree is a row in
//! whatever area composition gave it — a float body, a slot in a split — and
//! not something hung off a buffer. Both halves draw the same two-cell `┊ `
//! rail so the two roads cannot diverge visually.
//!
//! # Why the anchor is a position and not a line
//!
//! [`Anchor::col`] is the whole of acceptance part 3. On a wrapped line the
//! line owns `n` rows, and the row a region hangs under is the *segment*
//! showing its anchor column — `Editor::visual_row_for_position`'s answer, not
//! `visual_row_for_line`'s. The fork resolves it by exactly that rule, and the
//! indent the row inherits is that segment's own text start: 0 on a first
//! segment, [`soft_wrap::CONTINUATION_PREFIX`] on a `↪` continuation, which is
//! what §3's *"indents to code column"* means on a row whose code column moved.
//!
//! # What a virtual row is not
//!
//! It is **not a line**. It carries no `line_idx`, prints no line number,
//! resolves to no source line and owns no char span — so a click on one moves
//! no cursor, `j`/`k` step over it, and every line number below it is exactly
//! where it was before the row appeared. What it *does* occupy is a visual row,
//! which is why [`is_virtual_row`] exists: anything indexed by visual row (the
//! state column, `T031`'s gutter) has to skip these, the same way
//! [`soft_wrap::is_continuation_row`] makes it skip `↪` rows.
//!
//! [`BufferView`]: crate::buffer_view::BufferView
//! [`soft_wrap::CONTINUATION_PREFIX`]: crate::soft_wrap::CONTINUATION_PREFIX
//! [`soft_wrap::is_continuation_row`]: crate::soft_wrap::is_continuation_row
//!
//! Owned by `surface`.

use phosphor_core::request::RegionId;
use ratatui_code_editor::phosphor::virtual_text::{VirtualLine, VirtualRun};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::text::Span;
use ratatui_core::widgets::Widget;

use crate::buffer_view::Editor;
use crate::theme::Theme;

/// §2's virtual-margin rail, and the space after it. Two cells — the same
/// budget `↪ ` spends, so a virtual row and a soft-wrap continuation start
/// their text in the same column.
pub const RAIL: &str = "┊ ";

/// Cells [`RAIL`] costs, in front of a virtual row's text.
pub const RAIL_PREFIX: u16 = 2;

// ---------------------------------------------------------------------------
// The row
// ---------------------------------------------------------------------------

/// Where a virtual row hangs: a buffer position, not a line.
///
/// The column is what places the row on a wrapped line — see the module docs.
/// A column past the line's end lands on its last segment, which is where a
/// region anchored to "the end of this line" belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Anchor {
    /// Source line, 0-based.
    pub line: usize,
    /// Char column within that line.
    pub col: usize,
}

impl Anchor {
    /// The start of a line — the anchor an unwrapped region has.
    #[must_use]
    pub const fn line(line: usize) -> Self {
        Self { line, col: 0 }
    }

    /// A position inside a line.
    #[must_use]
    pub const fn at(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// One styled piece of a virtual row's text.
///
/// Styles arrive resolved. §3 is *"meta-gray with colored spans"*, so
/// [`Run::prose`] is the default and anything louder is the caller naming a
/// theme colour deliberately — there are no colours in this file.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The text, without the rail.
    pub text: String,
    /// How it is painted.
    pub style: Style,
}

impl Run {
    /// A run in a style the caller already resolved against a [`Theme`].
    #[must_use]
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// §3's default: meta-gray, the colour every other aside on screen takes.
    #[must_use]
    pub fn prose(text: impl Into<String>, theme: &Theme) -> Self {
        Self::new(text, Style::new().fg(theme.neutrals.meta))
    }

    /// Cells this run occupies.
    #[must_use]
    fn width(&self) -> u16 {
        u16::try_from(Span::raw(&self.text).width()).unwrap_or(u16::MAX)
    }
}

/// A `┊` row and the region it belongs to.
///
/// **Four consumers, one type.** A thread is two of these anchored to the same
/// position (`3a`), a watch is one carrying its value stream (`4b`), a
/// diagnostic is one carrying its message (`6b`), and `T035`'s unknown-key hint
/// is one with no owner at all (`8e`). Nothing here knows which it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The region this row hangs from, or `None` for an unowned hint.
    pub owner: Option<RegionId>,
    /// Where it hangs.
    pub anchor: Anchor,
    /// What it says, after the rail.
    pub runs: Vec<Run>,
}

impl Row {
    /// An unowned row at `anchor`.
    #[must_use]
    pub fn new(anchor: Anchor, runs: Vec<Run>) -> Self {
        Self {
            owner: None,
            anchor,
            runs,
        }
    }

    /// The same row, tagged with the region that owns it.
    #[must_use]
    pub fn owned_by(mut self, owner: RegionId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Cells the row's text needs, rail included.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.runs
            .iter()
            .fold(RAIL_PREFIX, |total, run| total.saturating_add(run.width()))
    }
}

// ---------------------------------------------------------------------------
// Placement — the half that lives in the row stream
// ---------------------------------------------------------------------------

/// Installs the rail's colour on an [`Editor`].
///
/// Call it alongside [`buffer_view::configure`] and
/// [`soft_wrap::configure`], and again on a theme change — the fork paints the
/// `┊` from its own theme map for the same reason it paints `↪` from one, and
/// a row's runs carry their styles with them.
///
/// Idempotent, and it never touches the viewport or the cursor.
///
/// [`buffer_view::configure`]: crate::buffer_view::configure
/// [`soft_wrap::configure`]: crate::soft_wrap::configure
pub fn configure(editor: &mut Editor, theme: &Theme) {
    editor.set_theme_key("virtual_rail", rail_style(theme));
}

/// §3: the rail is meta-gray, like the prose it introduces.
#[must_use]
pub fn rail_style(theme: &Theme) -> Style {
    Style::new().fg(theme.neutrals.meta)
}

/// Replaces every `┊` row on this buffer, in draw order.
///
/// **This is the acceptance criterion, and it is one function call.** The rows
/// enter the vendored fork's row stream as a `VisualRow::Virtual` variant, so
/// row↔line mapping, cursor placement and click targeting all see them at the
/// same index this does. Rows anchored to the same visual row keep the order
/// they were given in, which is what lets a thread read as an exchange.
///
/// A row whose anchor the stream does not show — a line inside a collapsed
/// fold, or past the end of the buffer — is not drawn. It is **not** clamped
/// somewhere else: a thread that moved out of view is invisible, not
/// mispositioned.
///
/// Rebuilds the row stream when the list changes and does nothing when it has
/// not, so calling it every time regions move is free. It moves no viewport.
pub fn install(editor: &mut Editor, rows: &[Row]) {
    editor.set_virtual_lines(rows.iter().map(vendor_line).collect());
}

/// Removes every `┊` row.
pub fn clear(editor: &mut Editor) {
    editor.clear_virtual_lines();
}

/// Shows or hides the installed rows without discarding them — the payload of
/// `set-virtual-text-visible` (`T032`'s Action).
pub fn set_visible(editor: &mut Editor, visible: bool) {
    editor.set_virtual_text_visible(visible);
}

/// Whether the installed rows are drawn.
#[must_use]
pub fn is_visible(editor: &Editor) -> bool {
    editor.virtual_text_visible()
}

/// Whether a visual row is a virtual one rather than buffer text.
///
/// **Anything indexed by visual row needs this.** A virtual row is not a line,
/// so a state column that marked one would be claiming a region covers more
/// rows than it does — the same reason
/// [`soft_wrap::is_continuation_row`](crate::soft_wrap::is_continuation_row)
/// exists for `↪` rows.
#[must_use]
pub fn is_virtual_row(editor: &Editor, visual_row: usize) -> bool {
    editor.virtual_line_at(visual_row).is_some()
}

/// The region that owns the row at `visual_row`, if it is a virtual row with
/// an owner.
#[must_use]
pub fn owner_at(editor: &Editor, visual_row: usize) -> Option<RegionId> {
    editor.virtual_line_at(visual_row)?.owner.map(RegionId)
}

/// Cells of indent before the `┊` on a virtual row — 0 under a whole line or a
/// first segment, [`soft_wrap::CONTINUATION_PREFIX`] under a `↪` continuation.
/// `None` when the row is not a virtual one.
///
/// [`soft_wrap::CONTINUATION_PREFIX`]: crate::soft_wrap::CONTINUATION_PREFIX
#[must_use]
pub fn indent_at(editor: &Editor, visual_row: usize) -> Option<u16> {
    editor
        .virtual_row_indent(visual_row)
        .map(|indent| u16::try_from(indent).unwrap_or(u16::MAX))
}

/// Every visual row currently drawn for `owner`, in stream order.
#[must_use]
pub fn rows_of(editor: &Editor, owner: RegionId) -> Vec<usize> {
    (0..editor.visual_len_lines())
        .filter(|row| owner_at(editor, *row) == Some(owner))
        .collect()
}

/// A [`Row`] as the fork's own type. The one place the two shapes meet.
fn vendor_line(row: &Row) -> VirtualLine {
    VirtualLine {
        line_idx: row.anchor.line,
        col: row.anchor.col,
        owner: row.owner.map(|RegionId(id)| id),
        runs: row
            .runs
            .iter()
            .map(|run| VirtualRun::new(run.text.clone(), run.style))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// The widget — a standalone row, for `Node::VirtualText`
// ---------------------------------------------------------------------------

/// One `┊` row drawn into an area of its own.
///
/// What `Node::VirtualText` renders as. The protocol's node has no anchor, so
/// this draws where composition put it and does not touch a buffer; a row that
/// hangs from a *line* goes through [`install`] instead.
///
/// Draws one row — `area`'s first — and clips to it. §11 is *"nothing ever
/// wraps"*, so a row too long for its area is cut at the right edge rather
/// than continued.
#[derive(Debug, Clone, Copy)]
pub struct VirtualText<'a> {
    runs: &'a [Run],
    theme: &'a Theme,
}

impl<'a> VirtualText<'a> {
    /// A row saying `runs`, painted with `theme`.
    #[must_use]
    pub const fn new(runs: &'a [Run], theme: &'a Theme) -> Self {
        Self { runs, theme }
    }

    /// Cells this row wants: the rail plus its runs.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.runs
            .iter()
            .fold(RAIL_PREFIX, |total, run| total.saturating_add(run.width()))
    }
}

impl Widget for VirtualText<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let mut x = write(buf, area, area.x, RAIL, rail_style(self.theme));
        for run in self.runs {
            if x >= area.right() {
                break;
            }
            x = write(buf, area, x, &run.text, run.style);
        }
    }
}

/// Writes `text` at `x` on `area`'s first row, clipped to both. Returns the
/// column after the last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, text: &str, style: Style) -> u16 {
    if x >= area.right() {
        return x;
    }
    let room = (area.right() - x) as usize;
    let (next, _) = buf.set_stringn(x, area.y, text, room, style);
    next.min(area.right())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer_view::{self, BufferView, gutter_width};
    use crate::soft_wrap::{self, CONTINUATION_PREFIX};

    /// Nine lines. Line 8 (1-based) is long enough to take three rows in the
    /// width the wrapped tests render at, which is what "first, middle and
    /// last visual row of the wrap" needs.
    const SOURCE: &str = "\
pub fn retry_with_backoff<T, E>(
    op: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    todo!()
}

// long doc comment wraps softly and the continuation row carries no line number because the gutter stays honest about how many lines there actually are
    resp.json().await.map_err(FetchError::Decode)
";

    /// The same width `soft_wrap.rs`'s own tests use, so the wrap here is the
    /// wrap that module proved.
    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 20,
    };

    /// 1-based, as the gutter prints them.
    const COMMENT_LINE: usize = 8;

    fn plain(theme: &Theme) -> Editor {
        let mut editor = Editor::new("rust", SOURCE, vec![]).expect("fixture parses");
        buffer_view::configure(&mut editor, theme);
        soft_wrap::configure(&mut editor, theme);
        configure(&mut editor, theme);
        editor
    }

    fn wrapped(theme: &Theme) -> Editor {
        let mut editor = plain(theme);
        soft_wrap::wrap_to(&mut editor, AREA);
        editor
    }

    fn render(editor: &Editor, theme: &Theme) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        BufferView::new(editor, theme).render(AREA, &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    /// The visual rows line `line_1` occupies, in order.
    fn segments(editor: &Editor, line_1: usize) -> Vec<usize> {
        (0..editor.visual_len_lines())
            .filter(|row| {
                editor
                    .row_span(*row)
                    .is_some_and(|span| span.line_idx == line_1 - 1)
            })
            .collect()
    }

    fn thread(theme: &Theme, anchor: Anchor) -> Vec<Row> {
        vec![
            Row::new(
                anchor,
                vec![
                    Run::new("⚓ you · 2m", Style::new().fg(theme.actors.you)),
                    Run::prose("  cap check?", theme),
                ],
            )
            .owned_by(RegionId(4)),
            Row::new(
                anchor,
                vec![
                    Run::new("✻ claude · 1m", Style::new().fg(theme.actors.claude)),
                    Run::prose("  max_attempts is the cap", theme),
                ],
            )
            .owned_by(RegionId(4)),
        ]
    }

    // -- 1. rows interleave -------------------------------------------------

    #[test]
    fn a_row_lands_directly_under_its_anchor_line() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        let before = editor.visual_len_lines();

        install(
            &mut editor,
            &[Row::new(
                Anchor::line(3),
                vec![Run::prose("1 diagnostic", &theme)],
            )],
        );

        assert_eq!(editor.visual_len_lines(), before + 1, "one row appeared");
        // Line 4 (1-based) is visual row 3; the virtual row is 4.
        assert!(!is_virtual_row(&editor, 3));
        assert!(is_virtual_row(&editor, 4));
        assert_eq!(
            editor.row_span(3).expect("a real row").line_idx,
            3,
            "the anchor row is still the anchor line"
        );
        assert_eq!(
            editor.row_span(5).expect("the row below").line_idx,
            4,
            "and the line under it is untouched"
        );
    }

    #[test]
    fn rows_on_one_anchor_keep_the_order_they_were_given() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        install(&mut editor, &thread(&theme, Anchor::line(3)));
        let buf = render(&editor, &theme);

        assert!(
            row_text(&buf, 4).contains("you · 2m"),
            "{:?}",
            row_text(&buf, 4)
        );
        assert!(
            row_text(&buf, 5).contains("claude · 1m"),
            "{:?}",
            row_text(&buf, 5)
        );
        assert_eq!(rows_of(&editor, RegionId(4)), vec![4, 5]);
    }

    #[test]
    fn rows_on_different_anchors_interleave_in_document_order() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        // Installed out of document order on purpose: placement is by anchor,
        // not by the order the host happened to collect them in.
        install(
            &mut editor,
            &[
                Row::new(Anchor::line(4), vec![Run::prose("second", &theme)]),
                Row::new(Anchor::line(1), vec![Run::prose("first", &theme)]),
            ],
        );
        // Line 1 is visual row 1, so its row is 2 — and line 4, one row lower
        // than it was, hangs its own at 6.
        let buf = render(&editor, &theme);
        assert!(
            row_text(&buf, 2).contains("first"),
            "{:?}",
            row_text(&buf, 2)
        );
        assert!(
            row_text(&buf, 6).contains("second"),
            "{:?}",
            row_text(&buf, 6)
        );
    }

    #[test]
    fn a_row_anchored_out_of_the_stream_is_dropped_rather_than_clamped() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        let before = editor.visual_len_lines();
        install(
            &mut editor,
            &[Row::new(
                Anchor::line(9_000),
                vec![Run::prose("nowhere", &theme)],
            )],
        );
        assert_eq!(editor.visual_len_lines(), before);

        // The same is true of a line hidden inside a collapsed fold: the
        // thread goes with the code it hangs from.
        let mut editor = plain(&theme);
        install(
            &mut editor,
            &[Row::new(
                Anchor::line(3),
                vec![Run::prose("inside", &theme)],
            )],
        );
        assert_eq!(editor.visual_len_lines(), before + 1);
        editor.set_code_folding_enabled(true);
        assert!(editor.toggle_fold_at_line(0), "line 1 opens a fold");
        assert!(
            (0..editor.visual_len_lines()).all(|row| !is_virtual_row(&editor, row)),
            "a folded anchor takes its virtual rows with it"
        );
    }

    #[test]
    fn the_toggle_hides_the_rows_without_losing_them() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        let before = editor.visual_len_lines();
        install(&mut editor, &thread(&theme, Anchor::line(3)));
        assert_eq!(editor.visual_len_lines(), before + 2);

        set_visible(&mut editor, false);
        assert!(!is_visible(&editor));
        assert_eq!(editor.visual_len_lines(), before);
        assert_eq!(rows_of(&editor, RegionId(4)), Vec::<usize>::new());

        set_visible(&mut editor, true);
        assert_eq!(editor.visual_len_lines(), before + 2);
        assert_eq!(rows_of(&editor, RegionId(4)), vec![4, 5]);

        clear(&mut editor);
        assert_eq!(editor.visual_len_lines(), before);
    }

    // -- 2. the line numbering never moves ----------------------------------

    /// **Acceptance part 2, as the gutter draws it.** Every line's number is
    /// what it was before the rows arrived, and the virtual rows carry none.
    #[test]
    fn a_virtual_row_never_shifts_the_buffers_line_numbering() {
        let theme = Theme::phosphor_dark();
        let editor = plain(&theme);
        let numbers = |editor: &Editor| -> Vec<Option<usize>> {
            (0..editor.visual_len_lines())
                .map(|row| editor.row_span(row).map(|span| span.line_idx))
                .collect()
        };
        let before = numbers(&editor);

        let mut editor = plain(&theme);
        install(
            &mut editor,
            &[
                Row::new(Anchor::line(0), vec![Run::prose("a", &theme)]),
                Row::new(Anchor::line(3), vec![Run::prose("b", &theme)]),
                Row::new(Anchor::line(3), vec![Run::prose("c", &theme)]),
                Row::new(Anchor::line(8), vec![Run::prose("d", &theme)]),
            ],
        );
        let after = numbers(&editor);

        // Every line still appears, exactly once, in the same order; the four
        // new rows are the `None`s, and they belong to no line.
        assert_eq!(after.iter().filter(|n| n.is_none()).count(), 4);
        assert_eq!(
            after.into_iter().flatten().collect::<Vec<_>>(),
            before.into_iter().flatten().collect::<Vec<_>>()
        );

        // And on screen: the number column is blank on a virtual row.
        let buf = render(&editor, &theme);
        let gutter = gutter_width(&editor);
        for row in 0..editor.visual_len_lines().min(AREA.height as usize) {
            if !is_virtual_row(&editor, row) {
                continue;
            }
            for x in 0..gutter {
                assert_eq!(
                    buf[(x, row as u16)].symbol(),
                    " ",
                    "gutter cell {x} of virtual row {row} must be blank"
                );
            }
        }
    }

    #[test]
    fn a_virtual_row_is_not_a_line_for_anything_that_asks() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        install(
            &mut editor,
            &[Row::new(Anchor::line(3), vec![Run::prose("x", &theme)])],
        );
        let row = 4;
        assert!(is_virtual_row(&editor, row));

        // No char span, so no column resolves to it …
        assert!(editor.row_span(row).is_none());
        // … and no click on it resolves to a cursor.
        let area = buffer_view::editor_area(AREA);
        let gutter = gutter_width(&editor);
        assert_eq!(
            editor.cursor_from_mouse(gutter + 2, row as u16, &area),
            None
        );
    }

    #[test]
    fn vertical_motion_steps_over_a_virtual_row() {
        use ratatui_code_editor::actions::{MoveDown, MoveUp};

        let theme = Theme::phosphor_dark();
        let mut editor = wrapped(&theme);
        let line = COMMENT_LINE - 1;
        install(
            &mut editor,
            &[Row::new(
                Anchor::at(line, 0),
                vec![Run::prose("under the first segment", &theme)],
            )],
        );
        let rows = segments(&editor, COMMENT_LINE);
        assert!(
            rows.len() >= 3,
            "the fixture must wrap three ways: {rows:?}"
        );
        assert!(
            is_virtual_row(&editor, rows[0] + 1),
            "the row hangs under the first segment"
        );

        editor.set_cursor(editor.code_ref().line_to_char(line));
        assert_eq!(editor.visual_row_for_cursor(), Some(rows[0]));
        editor.apply(MoveDown { shift: false });
        assert_eq!(
            editor.visual_row_for_cursor(),
            Some(rows[1]),
            "down lands on the next *text* row, not the `┊` between them"
        );
        editor.apply(MoveUp { shift: false });
        assert_eq!(editor.visual_row_for_cursor(), Some(rows[0]));
    }

    // -- 3. placement on a soft-wrapped line --------------------------------

    /// **Acceptance part 3, the gate item.** The anchor's column decides which
    /// segment the row hangs under — first, middle and last, one assertion
    /// each.
    #[test]
    fn a_row_hangs_under_the_segment_holding_its_anchor_column() {
        let theme = Theme::phosphor_dark();
        let probe = wrapped(&theme);
        let rows = segments(&probe, COMMENT_LINE);
        assert!(
            rows.len() >= 3,
            "the fixture must wrap three ways: {rows:?}"
        );
        let spans: Vec<_> = rows
            .iter()
            .map(|row| probe.row_span(*row).expect("a segment"))
            .collect();
        let line = COMMENT_LINE - 1;

        for (segment, span) in spans.iter().enumerate() {
            // A column in the middle of this segment, so no boundary rule is
            // being leaned on.
            let col = (span.start_col + span.end_col) / 2;
            let mut editor = wrapped(&theme);
            install(
                &mut editor,
                &[Row::new(
                    Anchor::at(line, col),
                    vec![Run::prose("hangs here", &theme)],
                )],
            );

            // The row's own segments have not moved relative to each other,
            // and the virtual row sits immediately after segment `segment`.
            let after = segments(&editor, COMMENT_LINE);
            assert_eq!(after.len(), rows.len(), "the wrap itself is unchanged");
            assert!(
                is_virtual_row(&editor, after[segment] + 1),
                "column {col} anchors to segment {segment}, so the row belongs \
                 immediately under visual row {}",
                after[segment]
            );
            assert_eq!(
                editor.visual_row_for_position(line, col),
                Some(after[segment]),
                "and the row stream agrees about which segment that column is on"
            );
        }
    }

    /// The desync `T081` named: with the virtual row *inside* the stream, every
    /// column of the wrapped line still resolves to its own segment.
    #[test]
    fn a_row_between_segments_does_not_desynchronise_the_wrap() {
        let theme = Theme::phosphor_dark();
        let mut editor = wrapped(&theme);
        let line = COMMENT_LINE - 1;
        install(
            &mut editor,
            &[Row::new(
                Anchor::at(line, 0),
                vec![Run::prose("between segment 0 and 1", &theme)],
            )],
        );

        for row in 0..editor.visual_len_lines() {
            let Some(span) = editor.row_span(row) else {
                continue;
            };
            for col in span.start_col..span.end_col {
                assert_eq!(
                    editor.visual_row_for_position(span.line_idx, col),
                    Some(row),
                    "column {col} of line {} must resolve to its own row",
                    span.line_idx
                );
            }
        }

        // And the cursor, which is the same question asked by the renderer.
        let rows = segments(&editor, COMMENT_LINE);
        let last = *rows.last().expect("segments");
        let span = editor.row_span(last).expect("a segment");
        editor.set_cursor(editor.code_ref().line_to_char(line) + span.start_col + 1);
        assert_eq!(editor.visual_row_for_cursor(), Some(last));
        let (_, cursor_y) = editor
            .get_visible_cursor(&buffer_view::editor_area(AREA))
            .expect("the cursor is on screen");
        assert_eq!(cursor_y as usize, last);
    }

    /// §3's *"indents to code column"* on a row whose code column moved: a row
    /// under a `↪` continuation starts where that continuation's text starts.
    #[test]
    fn a_row_under_a_continuation_inherits_its_indent() {
        let theme = Theme::phosphor_dark();
        let probe = wrapped(&theme);
        let rows = segments(&probe, COMMENT_LINE);
        let second = probe.row_span(rows[1]).expect("a continuation");
        assert!(second.wrapped && second.segment > 0);
        assert_eq!(second.prefix_cells, CONTINUATION_PREFIX as usize);
        let line = COMMENT_LINE - 1;

        let mut editor = wrapped(&theme);
        install(
            &mut editor,
            &[
                Row::new(Anchor::at(line, 0), vec![Run::prose("head", &theme)]),
                Row::new(
                    Anchor::at(line, second.start_col),
                    vec![Run::prose("tail", &theme)],
                ),
            ],
        );
        let after = segments(&editor, COMMENT_LINE);
        let head = after[0] + 1;
        let tail = after[1] + 1;
        assert_eq!(indent_at(&editor, head), Some(0));
        assert_eq!(indent_at(&editor, tail), Some(CONTINUATION_PREFIX));

        // Cell by cell: the rail sits at the text column, and two cells in on
        // the continuation — the same column its own text starts at.
        let buf = render(&editor, &theme);
        let gutter = gutter_width(&editor);
        assert_eq!(buf[(gutter, head as u16)].symbol(), "┊");
        assert_eq!(
            buf[(gutter + CONTINUATION_PREFIX, tail as u16)].symbol(),
            "┊"
        );
        assert_eq!(buf[(gutter, tail as u16)].symbol(), " ");
        // Which is exactly where the continuation's own text starts, past its
        // `↪ ` marker — the two columns are the same column.
        assert_eq!(buf[(gutter, after[1] as u16)].symbol(), "↪");
        assert_ne!(
            buf[(gutter + CONTINUATION_PREFIX, after[1] as u16)].symbol(),
            " ",
            "the continuation's text starts where the rail under it does"
        );
    }

    #[test]
    fn a_column_past_the_end_of_a_wrapped_line_hangs_from_its_last_segment() {
        let theme = Theme::phosphor_dark();
        let mut editor = wrapped(&theme);
        let line = COMMENT_LINE - 1;
        install(
            &mut editor,
            &[Row::new(
                Anchor::at(line, usize::MAX),
                vec![Run::prose("end of line", &theme)],
            )],
        );
        let rows = segments(&editor, COMMENT_LINE);
        let last = *rows.last().expect("segments");
        assert!(is_virtual_row(&editor, last + 1));
    }

    // -- the rail, and what it is painted in --------------------------------

    #[test]
    fn the_rail_is_meta_gray_and_the_runs_keep_their_own_colours() {
        let theme = Theme::phosphor_dark();
        let mut editor = plain(&theme);
        install(&mut editor, &thread(&theme, Anchor::line(3)));
        let buf = render(&editor, &theme);
        let gutter = gutter_width(&editor);

        assert_eq!(buf[(gutter, 4)].symbol(), "┊");
        assert_eq!(buf[(gutter, 4)].fg, theme.neutrals.meta);
        assert_eq!(buf[(gutter + 1, 4)].symbol(), " ");
        assert_eq!(buf[(gutter + RAIL_PREFIX, 4)].symbol(), "⚓");
        assert_eq!(buf[(gutter + RAIL_PREFIX, 4)].fg, theme.actors.you);
        assert_eq!(buf[(gutter + RAIL_PREFIX, 5)].fg, theme.actors.claude);

        // §3: "meta-gray with colored spans" — the prose half is meta.
        let row = row_text(&buf, 4);
        let at = row.find("cap check?").expect("the prose is drawn") as u16;
        assert_eq!(buf[(at, 4)].fg, theme.neutrals.meta);
    }

    #[test]
    fn the_rail_reads_the_theme_rather_than_agreeing_with_it_by_coincidence() {
        // The same proof `buffer_view.rs` uses for the line numbers: a theme
        // carrying an unmistakably different value in that field moves the
        // rail with it. The substitute is another *theme* colour, so no value
        // enters this file.
        let mut recoloured = Theme::phosphor_dark();
        recoloured.neutrals.meta = recoloured.actors.trouble;
        let expected = recoloured.actors.trouble;

        let mut editor = plain(&recoloured);
        install(
            &mut editor,
            &[Row::new(
                Anchor::line(3),
                vec![Run::new("x", Style::new().fg(recoloured.actors.you))],
            )],
        );
        let buf = render(&editor, &recoloured);
        assert_eq!(buf[(gutter_width(&editor), 4)].fg, expected);
    }

    // -- the standalone widget ----------------------------------------------

    #[test]
    fn the_widget_draws_the_same_rail_the_stream_does() {
        let theme = Theme::phosphor_dark();
        let runs = vec![
            Run::prose("unknown key ", &theme),
            Run::new("gq", Style::new().fg(theme.actors.attention)),
            Run::prose(" — SPC opens the keymap", &theme),
        ];
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        VirtualText::new(&runs, &theme).render(area, &mut buf);

        assert_eq!(row_of(&buf, 0), "┊ unknown key gq — SPC opens the keymap");
        assert_eq!(buf[(0, 0)].fg, theme.neutrals.meta);
        assert_eq!(buf[(2, 0)].fg, theme.neutrals.meta);
        assert_eq!(buf[(14, 0)].symbol(), "g");
        assert_eq!(buf[(14, 0)].fg, theme.actors.attention);
        assert_eq!(
            VirtualText::new(&runs, &theme).width() as usize,
            Span::raw("┊ unknown key gq — SPC opens the keymap").width()
        );
    }

    #[test]
    fn a_row_too_long_for_its_area_is_clipped_rather_than_wrapped() {
        let theme = Theme::phosphor_dark();
        let runs = vec![Run::prose("a very long virtual row indeed", &theme)];
        let area = Rect::new(0, 0, 8, 3);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 3));
        VirtualText::new(&runs, &theme).render(area, &mut buf);
        assert_eq!(row_of(&buf, 0), "┊ a very");
        assert_eq!(row_of(&buf, 1), "", "§11: nothing ever wraps");
    }

    #[test]
    fn a_degenerate_area_does_not_panic() {
        let theme = Theme::phosphor_dark();
        let runs = vec![Run::prose("x", &theme)];
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 2));
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(5, 1, 1, 1),
            Rect::new(20, 20, 4, 1),
        ] {
            VirtualText::new(&runs, &theme).render(area, &mut buf);
        }
    }

    fn row_of(buf: &Buffer, y: u16) -> String {
        (buf.area.x..buf.area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }
}
