//! Soft wrap (`T081`) and the two other text details screen `8e` draws —
//! collapsed folds and insert-only whitespace marks (`T016`).
//!
//! # Why one module
//!
//! All three are **variants in one row stream**, not layers over it. The
//! vendored core resolves every screen row through `VisualRow`, and `T081`'s
//! instruction is that soft wrap must be a variant *inside* that enum
//! alongside the existing fold and ghost variants: row↔line mapping, cursor
//! placement, click targeting and — from `T032` — virtual-text placement all
//! read the same list, and a wrap that lives above it desynchronises all four.
//! So the mechanism is in the fork (`VENDOR.md` patches 6 and 7) and this
//! module is only its configuration: which cells to wrap at, which colours the
//! new glyphs take, and when the whitespace marks are on.
//!
//! # Screen `8e`, cell by cell
//!
//! ```text
//!  12  pub fn retry_with_backoff<T, E>( ▸⋯ 13 lines
//!  26
//!  27  // long doc comment wraps softly and the continuation row
//!      ↪ // carries no line number — the gutter stays honest
//!  28      resp.json().await.map_err(FetchError::Decode)··
//! ```
//!
//! * **Fold** — `▸⋯ n lines` in [`NeutralRamp::meta`], inline after the header
//!   line's code, one space clear of it. Not a row of its own and not a gutter
//!   column: the hidden lines are simply absent from the row stream, which is
//!   what the vendored core's existing code folds already do.
//! * **Soft wrap** — `↪ ` in [`NeutralRamp::line_numbers`] at the head of each
//!   continuation row, and the line-number column left blank. "*carries no
//!   line number — the gutter stays honest*".
//! * **Whitespace** — trailing spaces render `·` in [`ActorPalette::trouble`]
//!   on [`RegionTints::failure`], and **only in INSERT** ("← INSERT only").
//!
//! # The row-stream contract, for `T032`
//!
//! `VirtualText` lands three windows after this one and cannot be verified
//! here, so what soft wrap owes it is written down instead. A row is resolved
//! through `Editor::row_span`, and these hold:
//!
//! 1. **One row is one `VisualRow`.** `Editor::visual_len_lines` counts rows,
//!    not lines, so anything measuring a viewport already counts wrapped
//!    segments.
//! 2. **A line owns a contiguous run of rows**, in document order, never
//!    interleaved with another line's. A line that fits is one `Real` row; a
//!    line that does not is `n >= 2` `Wrapped` rows, `segment = 0..n`.
//! 3. **The spans partition the line.** `end_col` of a segment is `start_col`
//!    of the next, the first `start_col` is 0, the last `end_col` is the line
//!    length. No column is on two rows and none is on none.
//! 4. **A column resolves to exactly one row**, via
//!    `Editor::visual_row_for_position(line, col)` — which is what a region
//!    anchored at `(line, col)` must use to find the row to hang under. A
//!    column on a segment boundary belongs to the later row.
//! 5. **`prefix_cells` is where a row's text starts**, relative to the text
//!    column: 0 normally, 2 on a continuation. Virtual text indenting "to the
//!    code column" (§3) on a wrapped row means this, not the gutter width.
//!
//! A `VisualRow::Virtual` variant inserted between rows keeps all five. What
//! it must not do is live outside the stream — and the mechanical cost of
//! joining it is exactly the arms `Wrapped` had to add: `line_for_visual_row`,
//! `visual_row_for_line`, `visual_row_for_position`, `row_span`, `prev_line`,
//! `next_line`, `is_changed`, and the renderer's match.
//!
//! [`ActorPalette::trouble`]: crate::theme::ActorPalette::trouble
//! [`NeutralRamp::line_numbers`]: crate::theme::NeutralRamp::line_numbers
//! [`NeutralRamp::meta`]: crate::theme::NeutralRamp::meta
//! [`RegionTints::failure`]: crate::theme::RegionTints::failure

use ratatui_core::layout::Rect;
use ratatui_core::style::Style;

use crate::buffer_view::{Editor, gutter_width};
use crate::theme::Theme;

/// The cells a `↪ ` continuation marker costs a row, in front of its text.
/// The fork spends the same two, and `8e` draws them.
pub const CONTINUATION_PREFIX: u16 = 2;

/// The narrowest text column soft wrap will use. Below it a continuation row
/// would be almost all marker, so wrapping switches off rather than degrade
/// into one character per row.
pub const MIN_WRAP_WIDTH: u16 = 8;

/// Which editor mode the buffer is in, for the details that depend on it.
///
/// **The input machine's enum, re-exported — not a second one.** This module
/// carried a two-value copy while `T026` did not exist, and said so; `T026`
/// landed, and a widget may name [`phosphor_core::request`] (that is what
/// `scripts/lint-no-action-in-ui.sh` allows and `scripts/lint-no-store-mutation.sh`
/// does not forbid), so the copy is gone and the host's boundary conversion has
/// nothing left to convert.
///
/// Only INSERT differs today — `8e`'s whitespace marks are annotated "INSERT
/// only" — but the argument is a mode rather than a bool so the call site reads
/// as the mode it is, and so the six others can differ later without a
/// signature change.
pub use phosphor_core::request::EditMode;

/// Installs the `8e` text details on an [`Editor`].
///
/// **Call it after [`buffer_view::configure`], never before.** That function
/// installs the syntax map wholesale and turns folding off with its gutter;
/// this one adds the three keys that map does not carry and puts folding back
/// *without* the gutter column, which is the combination `8e` draws — a fold
/// marker inline after the code, and no extra cells between the numbers and
/// the text.
///
/// Idempotent, and it never touches the viewport or the cursor.
///
/// [`buffer_view::configure`]: crate::buffer_view::configure
pub fn configure(editor: &mut Editor, theme: &Theme) {
    for (key, style) in detail_styles(theme) {
        editor.set_theme_key(key, style);
    }
    // Folds exist; the gutter column does not (`8e` renders the marker inline).
    editor.set_code_folding_enabled(true);
    editor.set_fold_gutter_visible(false);
}

/// The three theme keys the vendored renderer reads for `8e`'s glyphs.
///
/// Separate from `buffer_view::syntax_theme` because none of them is a
/// tree-sitter capture: they are phosphor's own, added by the fork, and a
/// theme change updates them without rebuilding the syntax map.
#[must_use]
pub fn detail_styles(theme: &Theme) -> [(&'static str, Style); 3] {
    [
        // `↪` — the line-number colour, because it stands in for a number.
        (
            "wrap_indicator",
            Style::default().fg(theme.neutrals.line_numbers),
        ),
        // `▸⋯ n lines` — meta-gray, like every other count the editor says.
        ("fold_marker", Style::default().fg(theme.neutrals.meta)),
        // `··` — trouble on the failure tint, which §3 names for exactly this.
        (
            "trailing_whitespace",
            Style::default()
                .fg(theme.actors.trouble)
                .bg(theme.regions.failure),
        ),
    ]
}

/// Wraps this buffer to `area`, the same `Rect` [`BufferView`] renders into.
///
/// The wrap width is the text column: the area minus the gutter the 3-column
/// contract reserves. Narrower than [`MIN_WRAP_WIDTH`] and wrapping switches
/// off — a two-cell text column is not a soft wrap, it is a column of noise.
///
/// **Call it whenever the area changes**, before rendering. It rebuilds the
/// row stream when the width actually changes and does nothing when it has
/// not, so calling it every frame is free. It moves no viewport: rows below
/// the fold change index when a wrap appears, but the top row does not, which
/// is what invariant 3 asks for.
///
/// [`BufferView`]: crate::buffer_view::BufferView
pub fn wrap_to(editor: &mut Editor, area: Rect) {
    let width = text_width(editor, area);
    set_wrap_width(editor, (width >= MIN_WRAP_WIDTH).then_some(width));
}

/// Turns wrapping off: long lines scroll sideways instead.
pub fn unwrap(editor: &mut Editor) {
    set_wrap_width(editor, None);
}

/// The wrap width in cells, or `None` when this buffer does not wrap.
#[must_use]
pub fn wrap_width(editor: &Editor) -> Option<u16> {
    editor.soft_wrap_width().map(|width| width as u16)
}

/// Whether a visual row is a `↪` continuation — no line number, text indented
/// by [`CONTINUATION_PREFIX`].
///
/// The state bar column is indexed by visual row, so a caller building it for
/// a wrapped buffer needs this to leave continuation rows unmarked: a region
/// marks a *line*, and marking it twice because it wrapped would be a lie
/// about how many there are.
#[must_use]
pub fn is_continuation_row(editor: &Editor, visual_row: usize) -> bool {
    editor
        .row_span(visual_row)
        .is_some_and(|span| span.wrapped && span.segment > 0)
}

/// Marks trailing whitespace, or stops. `8e` shows the marks in INSERT only.
pub fn set_mode(editor: &mut Editor, mode: EditMode) {
    editor.set_show_trailing_whitespace(mode == EditMode::Insert);
}

/// The text column's width in `area`, in cells: everything right of the
/// gutter. Reads the gutter back from the editor rather than recomputing it,
/// so wrap width and the column contract cannot drift apart.
#[must_use]
pub fn text_width(editor: &Editor, area: Rect) -> u16 {
    area.width.saturating_sub(gutter_width(editor))
}

/// The one place the fork's wrap width is set.
fn set_wrap_width(editor: &mut Editor, width: Option<u16>) {
    editor.set_soft_wrap(width.map(usize::from));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::widgets::Widget;

    use super::*;
    use crate::buffer_view::{self, BufferView};

    /// `8e`'s own buffer, near enough: a foldable function, a comment long
    /// enough to wrap in the width the test renders at, and a line with two
    /// trailing spaces. Line numbers below are 1-based, as the gutter draws
    /// them.
    const SOURCE: &str = "\
pub fn retry_with_backoff<T, E>(
    op: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    todo!()
}

// long doc comment wraps softly and the continuation row carries no line number
    resp.json().await.map_err(FetchError::Decode)  
";

    /// Wide enough for the gutter plus a text column that forces the comment
    /// onto three rows, which is what the wrapped-row tests need.
    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 14,
    };

    /// 1-based line numbers of the fixture, as `8e` would print them.
    const COMMENT_LINE: usize = 8;
    const WHITESPACE_LINE: usize = 9;

    fn editor(theme: &Theme) -> Editor {
        let mut editor = Editor::new("rust", SOURCE, vec![]).expect("fixture parses");
        buffer_view::configure(&mut editor, theme);
        configure(&mut editor, theme);
        wrap_to(&mut editor, AREA);
        editor
    }

    fn render(editor: &Editor, theme: &Theme) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        BufferView::new(editor, theme).render(AREA, &mut buf);
        buf
    }

    /// One screen row as a string, trailing blanks trimmed.
    fn row_text(buf: &Buffer, y: u16) -> String {
        let mut out = String::new();
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.trim_end().to_string()
    }

    fn char_at(editor: &Editor, line_1: usize, col: usize) -> usize {
        editor.code_ref().line_to_char(line_1 - 1) + col
    }

    // -- soft wrap -------------------------------------------------------

    #[test]
    fn a_long_line_wraps_and_a_short_one_does_not() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);

        let comment_rows: Vec<_> = (0..editor.visual_len_lines())
            .filter(|row| {
                editor
                    .row_span(*row)
                    .is_some_and(|span| span.line_idx == COMMENT_LINE - 1)
            })
            .collect();
        assert!(
            comment_rows.len() >= 2,
            "the long comment must occupy more than one row, got {comment_rows:?}"
        );

        // Every other line still owns exactly one row.
        let short_rows = (0..editor.visual_len_lines())
            .filter(|row| {
                editor
                    .row_span(*row)
                    .is_some_and(|span| span.line_idx == 3 - 1)
            })
            .count();
        assert_eq!(short_rows, 1, "a line that fits must not become segments");
    }

    #[test]
    fn the_spans_of_a_wrapped_line_partition_it() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let line = COMMENT_LINE - 1;

        let spans: Vec<_> = (0..editor.visual_len_lines())
            .filter_map(|row| editor.row_span(row))
            .filter(|span| span.line_idx == line)
            .collect();

        assert_eq!(spans[0].start_col, 0, "the first segment starts at 0");
        assert_eq!(
            spans.last().expect("at least one segment").end_col,
            editor.code_ref().line_len(line),
            "the last segment ends at the line's end"
        );
        for pair in spans.windows(2) {
            assert_eq!(
                pair[0].end_col, pair[1].start_col,
                "no column may be on two rows or on none"
            );
            assert_eq!(pair[1].segment, pair[0].segment + 1);
        }
    }

    #[test]
    fn a_continuation_row_carries_no_line_number_and_a_marker() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let buf = render(&editor, &theme);

        let first = editor
            .visual_row_for_position(COMMENT_LINE - 1, 0)
            .expect("the comment is visible");
        let head = row_text(&buf, first as u16);
        let continuation = row_text(&buf, first as u16 + 1);

        assert!(
            head.trim_start().starts_with(&COMMENT_LINE.to_string()),
            "the first segment carries the line number: {head:?}"
        );
        let gutter = gutter_width(&editor);
        assert!(
            continuation.starts_with(&" ".repeat(gutter as usize)),
            "the gutter is blank on a continuation row: {continuation:?}"
        );
        assert!(
            continuation.trim_start().starts_with("↪ "),
            "the continuation opens with the marker: {continuation:?}"
        );
        assert!(
            !continuation.contains(&COMMENT_LINE.to_string()),
            "`8e`: carries no line number — the gutter stays honest: {continuation:?}"
        );
        // Blank cell by cell, not merely different.
        for x in 0..gutter {
            assert_eq!(
                buf[(x, first as u16 + 1)].symbol(),
                " ",
                "gutter cell {x} of a continuation row must be blank"
            );
        }
    }

    #[test]
    fn the_marker_takes_the_line_number_colour_and_the_text_follows_it() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let buf = render(&editor, &theme);
        let row = editor
            .visual_row_for_position(COMMENT_LINE - 1, 0)
            .expect("visible") as u16
            + 1;
        let gutter = gutter_width(&editor);

        assert_eq!(buf[(gutter, row)].symbol(), "↪");
        assert_eq!(buf[(gutter, row)].fg, theme.neutrals.line_numbers);
        // Text resumes past the marker, never under it.
        assert_eq!(buf[(gutter + 1, row)].symbol(), " ");
        assert_ne!(buf[(gutter + CONTINUATION_PREFIX, row)].symbol(), " ");
    }

    // -- cursor motion on a wrapped line ---------------------------------

    #[test]
    fn cursor_motion_lands_on_the_wrapped_row_it_is_in() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let line = COMMENT_LINE - 1;
        let segments: Vec<_> = (0..editor.visual_len_lines())
            .filter_map(|row| Some((row, editor.row_span(row)?)))
            .filter(|(_, span)| span.line_idx == line)
            .collect();
        assert!(segments.len() >= 2);

        // A column in the middle of the second segment must report the second
        // segment's row, and the cursor must render on that screen row.
        let (row, span) = segments[1];
        let col = span.start_col + 1;
        editor.set_cursor(char_at(&editor, COMMENT_LINE, col));
        assert_eq!(editor.visual_row_for_cursor(), Some(row));

        let (cursor_x, cursor_y) = editor
            .get_visible_cursor(&buffer_view::editor_area(AREA))
            .expect("the cursor is on screen");
        assert_eq!(cursor_y as usize, row, "cursor renders on its own segment");
        assert!(
            cursor_x >= gutter_width(&editor) + CONTINUATION_PREFIX,
            "and past the `↪ ` marker"
        );
    }

    #[test]
    fn down_and_up_move_by_visual_row_when_wrapped() {
        use ratatui_code_editor::actions::{MoveDown, MoveUp};

        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let line = COMMENT_LINE - 1;
        let first = editor.visual_row_for_position(line, 0).expect("visible");

        editor.set_cursor(char_at(&editor, COMMENT_LINE, 0));
        assert_eq!(editor.visual_row_for_cursor(), Some(first));

        // Down once: still the same source line, one row lower.
        editor.apply(MoveDown { shift: false });
        assert_eq!(
            editor.visual_row_for_cursor(),
            Some(first + 1),
            "down moves one visual row, not one source line"
        );
        assert_eq!(
            editor.code_ref().point(editor.get_cursor()).0,
            line,
            "and stays inside the wrapped line"
        );

        // And back up to where it started.
        editor.apply(MoveUp { shift: false });
        assert_eq!(editor.visual_row_for_cursor(), Some(first));
        assert_eq!(editor.get_cursor(), char_at(&editor, COMMENT_LINE, 0));
    }

    #[test]
    fn motion_is_line_wise_again_when_wrapping_is_off() {
        use ratatui_code_editor::actions::MoveDown;

        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        unwrap(&mut editor);
        assert_eq!(wrap_width(&editor), None);

        editor.set_cursor(char_at(&editor, COMMENT_LINE, 0));
        editor.apply(MoveDown { shift: false });
        assert_eq!(
            editor.code_ref().point(editor.get_cursor()).0,
            COMMENT_LINE, // 0-based index of the *next* line
            "with wrapping off, down is one source line, as upstream"
        );
    }

    // -- clicks on a wrapped line ----------------------------------------

    #[test]
    fn a_click_lands_on_the_row_and_column_it_was_aimed_at() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let area = buffer_view::editor_area(AREA);
        let gutter = gutter_width(&editor);

        for row in 0..editor.visual_len_lines().min(AREA.height as usize) {
            let Some(span) = editor.row_span(row) else {
                continue;
            };
            let cells = span.end_col - span.start_col;
            for cell in 0..cells {
                let x = gutter + span.prefix_cells as u16 + cell as u16;
                if x >= AREA.width {
                    break;
                }
                let cursor = editor
                    .cursor_from_mouse(x, row as u16, &area)
                    .expect("a click inside the text column resolves");
                editor.set_cursor(cursor);

                assert_eq!(
                    editor.visual_row_for_cursor(),
                    Some(row),
                    "a click on row {row}, cell {cell} must put the cursor on row {row}"
                );
                let (line, col) = editor.code_ref().point(cursor);
                assert_eq!(line, span.line_idx);
                assert_eq!(
                    col,
                    span.start_col + cell,
                    "and on the column under the pointer"
                );
            }
        }
    }

    #[test]
    fn a_click_past_the_end_of_a_continuation_stays_on_that_row() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let area = buffer_view::editor_area(AREA);

        let row = editor
            .visual_row_for_position(COMMENT_LINE - 1, 0)
            .expect("visible")
            + 1;
        let span = editor.row_span(row).expect("a continuation row");
        assert!(span.wrapped && span.segment > 0);

        let cursor = editor
            .cursor_from_mouse(AREA.width - 1, row as u16, &area)
            .expect("clicking the far right of a row resolves");
        editor.set_cursor(cursor);
        assert_eq!(
            editor.visual_row_for_cursor(),
            Some(row),
            "the cursor stays on the row that was clicked"
        );
    }

    #[test]
    fn the_gutter_is_not_a_click_target() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let area = buffer_view::editor_area(AREA);
        assert_eq!(editor.cursor_from_mouse(0, 1, &area), None);
    }

    // -- `8e` folds and whitespace ---------------------------------------

    #[test]
    fn a_collapsed_fold_renders_its_marker_inline() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        assert!(editor.toggle_fold_at_line(0), "line 1 opens a fold");
        let hidden = editor.fold_hidden_lines(0).expect("the fold is collapsed");

        let buf = render(&editor, &theme);
        let head = row_text(&buf, 0);
        assert!(
            head.contains(&format!("▸⋯ {hidden} lines")),
            "`8e` draws the marker after the code: {head:?}"
        );
        assert!(
            head.contains("( ▸⋯"),
            "one space clear of the code, as `8e` draws it: {head:?}"
        );

        // Meta-gray, and the hidden lines are gone from the stream rather than
        // drawn over.
        let x = head.find('▸').expect("marker present") as u16;
        assert_eq!(buf[(x, 0)].fg, theme.neutrals.meta);
        // The hidden lines are gone from the row stream, not painted over: the
        // row under the header is the blank line past the function.
        let next = editor.row_span(1).expect("a row under the fold");
        assert_eq!(
            next.line_idx,
            WHITESPACE_LINE - 3,
            "the fold's body left the stream"
        );
    }

    #[test]
    fn the_fold_marker_needs_no_gutter_column() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let before = gutter_width(&editor);
        assert!(editor.toggle_fold_at_line(0));
        assert_eq!(
            gutter_width(&editor),
            before,
            "folding must not widen the gutter — no mockup has that column"
        );
    }

    #[test]
    fn trailing_whitespace_is_marked_in_insert_only() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let line = WHITESPACE_LINE - 1;
        let len = editor.code_ref().line_len(line);
        let row = editor.visual_row_for_position(line, 0).expect("visible");

        set_mode(&mut editor, EditMode::Normal);
        let plain = render(&editor, &theme);
        assert!(
            !row_text(&plain, row as u16).contains('·'),
            "NORMAL leaves whitespace alone"
        );

        set_mode(&mut editor, EditMode::Insert);
        let insert = render(&editor, &theme);
        let text = row_text(&insert, row as u16);
        assert!(
            text.ends_with("··"),
            "INSERT marks the two trailing spaces: {text:?}"
        );

        // Trouble on the failure tint (§3), and only over the trailing run.
        let span = editor.row_span(row).expect("a real row");
        let mark_x = gutter_width(&editor) + span.prefix_cells as u16 + (len - 1) as u16;
        assert_eq!(insert[(mark_x, row as u16)].symbol(), "·");
        assert_eq!(insert[(mark_x, row as u16)].fg, theme.actors.trouble);
        assert_eq!(insert[(mark_x, row as u16)].bg, theme.regions.failure);
        let code_x = gutter_width(&editor) + 4;
        assert_ne!(
            insert[(code_x, row as u16)].bg,
            theme.regions.failure,
            "code is not whitespace"
        );
    }

    #[test]
    fn marks_do_not_bleed_onto_a_line_with_no_trailing_space() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        set_mode(&mut editor, EditMode::Insert);
        let buf = render(&editor, &theme);
        let row = editor
            .visual_row_for_position(4 - 1, 0)
            .expect("line 4 is visible");
        assert!(!row_text(&buf, row as u16).contains('·'));
    }

    /// The acceptance criterion for both tasks, in one frame: `8e`'s four text
    /// details on one screen, in the order the mockup draws them.
    ///
    /// ```text
    /// 8e                              this fixture
    ///  12  pub fn …<T, E>( ▸⋯ 13 lines   1  pub fn …<T, E>( ▸⋯ 5 lines
    ///  26                                7
    ///  27  // long doc comment …         8  // long doc comment …
    ///      ↪ // carries no line number      ↪ row carries no line number
    ///  28      resp.json()…··            9      resp.json()…··
    /// ```
    #[test]
    fn screen_8e_reproduces() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        set_mode(&mut editor, EditMode::Insert);
        assert!(editor.toggle_fold_at_line(0));
        let buf = render(&editor, &theme);

        let rows: Vec<String> = (0..5).map(|y| row_text(&buf, y)).collect();

        // 1 — the fold marker rides the header line, after the code.
        assert_eq!(rows[0], "   1  pub fn retry_with_backoff<T, E>( ▸⋯ 5 lines");
        // 2 — the fold's body is not drawn at all.
        assert_eq!(rows[1], "   7");
        // 3 — the long line takes its number and wraps.
        assert_eq!(
            rows[2],
            "   8  // long doc comment wraps softly and the continuation"
        );
        // 4 — and its continuation carries `↪` and no number.
        assert_eq!(rows[3], "      ↪ row carries no line number");
        // 5 — trailing whitespace, in INSERT.
        assert_eq!(
            rows[4],
            "   9      resp.json().await.map_err(FetchError::Decode)··"
        );
    }

    // -- the contract itself ---------------------------------------------

    #[test]
    fn wrapping_is_refused_rather_than_degenerate_in_a_narrow_area() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        let narrow = Rect {
            width: gutter_width(&editor) + 3,
            ..AREA
        };
        wrap_to(&mut editor, narrow);
        assert_eq!(wrap_width(&editor), None);
    }

    #[test]
    fn rewrapping_to_the_same_width_does_not_move_the_viewport() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(&theme);
        // Through the one function allowed to move a viewport (T015), not by
        // reaching into the vendored core behind its back.
        buffer_view::apply_scroll(&mut editor, buffer_view::ScrollRequest::ToRow(2), AREA);
        assert_eq!(buffer_view::viewport_of(&editor).top_row, 2);
        wrap_to(&mut editor, AREA);
        assert_eq!(buffer_view::viewport_of(&editor).top_row, 2);
        wrap_to(&mut editor, AREA);
        assert_eq!(buffer_view::viewport_of(&editor).top_row, 2);
    }

    #[test]
    fn every_row_of_the_buffer_resolves_to_exactly_one_line() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let mut seen: Vec<(usize, usize)> = Vec::new();
        for row in 0..editor.visual_len_lines() {
            let span = editor.row_span(row).expect("plain mode owns every row");
            for col in span.start_col..span.end_col {
                assert!(
                    !seen.contains(&(span.line_idx, col)),
                    "column {col} of line {} is drawn twice",
                    span.line_idx
                );
                seen.push((span.line_idx, col));
                assert_eq!(
                    editor.visual_row_for_position(span.line_idx, col),
                    Some(row),
                    "column {col} of line {} must resolve back to its own row",
                    span.line_idx
                );
            }
        }
    }

    #[test]
    fn continuation_rows_are_identifiable_for_the_state_column() {
        let theme = Theme::phosphor_dark();
        let editor = editor(&theme);
        let first = editor
            .visual_row_for_position(COMMENT_LINE - 1, 0)
            .expect("visible");
        assert!(!is_continuation_row(&editor, first));
        assert!(is_continuation_row(&editor, first + 1));
    }
}
