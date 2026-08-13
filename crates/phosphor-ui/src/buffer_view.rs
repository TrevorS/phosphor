//! `BufferView` — the 3-column contract, and the editor's scroll authority
//! (`T015`).
//!
//! # The three columns
//!
//! Design Language §3, verbatim: *"Column 1: the 1-cell state bar
//! (unseen/diagnostic/none — priority: trouble > attention > claude). Column 2:
//! line numbers, always `#414b42`. Column 3: text."*
//!
//! Read off the mockups, which agree with each other to the cell — `1a` and
//! `1d` are authored as literal monospace text, `8c` / `8d` / `8e` / `9c` as
//! flexbox with pixel widths that divide out to the same thing:
//!
//! ```text
//!  ┌ column 1: the state bar, 1 cell, background-coloured
//!  │┌ one cell of air
//!  ││┌ column 2: line numbers, right-aligned, ≥2 cells, always #414b42
//!  │││ ┌ two cells of air
//!  ▼▼▼▼▼▼
//!  █ 16      let mut delay = policy.base_delay;
//!    17      let mut last = None;
//!    ~
//!  ▲▲▲▲▲▲▲
//!        └ column 3: text starts here — cell 6 for a two-digit file
//! ```
//!
//! `1a` writes each gutter row as `<span> </span>  1` … `<span> </span> 24` —
//! a background-coloured state cell, then three cells holding a right-aligned
//! number — and starts its text `<pre>` 14px later, which at that mockup's
//! metrics is two cells. `8e` measures the same: state bar at 3px, the number
//! field `flex: 0 0 41px` with `padding-right: 14px`, text at 44px ≈ 5.87 cells
//! at 7.5px/cell → column 6. So for the two-digit files every mockup shows,
//! the gutter is **six cells** and it decomposes as 1 + 1 + 2 + 2.
//!
//! Rows past the end of the buffer carry `~` in the line-number column, in the
//! line-number colour, and nothing else (`1d`).
//!
//! # How it is drawn
//!
//! Compose-around, as the `T008` spike prescribed ([SPIKES.md] seam 2). The
//! vendored editor owns text, highlighting, cursor and selection and draws its
//! own line numbers at *its* left edge; we hand it a `Rect` inset by the two
//! cells that belong to us, and overpaint the state bar, the air, and the `~`
//! rows afterwards. `Widget for &Editor` writes into a `Buffer` we already own,
//! so the second pass is legitimate rather than a hack.
//!
//! Three things the vendored core hardcoded had to become theme lookups for
//! this to be true — the line-number colour, the colour of text no highlight
//! covers, and the minimum width of the number column (upstream's is 5, which
//! would make the gutter nine cells wide on `1a`). That is patch 4 in
//! `vendor/ratatui-code-editor/VENDOR.md`.
//!
//! # Scroll authority
//!
//! Invariant 3 — *"nothing moves unless you asked"* — in its most literal form.
//! [`ScrollRequest`] is the only thing that can move a viewport and
//! [`apply_scroll`] is the only function that applies one; everything else in
//! this module, the render path included, takes `&Editor` and physically cannot.
//! The tests at the bottom prove both halves: a matrix of resizes, cursor
//! moves, mark changes, theme swaps and cache invalidations that must leave the
//! offsets bit-identical, and a scan of this file's own source that fails if a
//! second call site to the vendored core's viewport mutators ever appears.
//!
//! # The one duplicated type, and why it is still two
//!
//! [`ScrollRequest`] was written before `Action` existed, deliberately shaped
//! as the payload one variant of it would carry. `T019` carried it:
//! `phosphor_core::request::ScrollRequest` is the same seven arms, and
//! `request.rs`'s own header calls this file's copy out by name and asks
//! `surface` to collapse them.
//!
//! **No lint stands in the way.** `scripts/lint-no-action-in-ui.sh` lists
//! `phosphor_core::request` as ALLOWED in as many words — *"Position, Span,
//! ScrollRequest, EditMode … naming a place is not asking for it to change"* —
//! and `scripts/lint-no-store-mutation.sh` forbids only `::store`. That is why
//! [`soft_wrap::EditMode`] collapsed to a re-export in the same pass and this
//! did not: the two definitions differ in *coordinates*, not in shape. The
//! vocabulary counts visual rows from 1 in `u32` because a person types them;
//! this counts from 0 in `usize` because it indexes them. Collapsing moves that
//! conversion from the host into [`Viewport::scrolled`]'s `ToRow` and
//! `RevealRow` arms, and deletes `scroll_request()` at
//! `crates/phosphor/src/main.rs:1886` — a file this crate's owner does not
//! hold. Raised as a contract request rather than half-done here.
//!
//! [`soft_wrap::EditMode`]: crate::soft_wrap::EditMode
//!
//! # Not here
//!
//! `T031` (`GutterBar`) owns the resolution of a row's region set down to one
//! [`StateMark`] and the `▎` degradation for terminals without truecolor;
//! [`StateMark`] here is only the already-resolved answer, which is all the
//! column contract needs in order to reserve and paint the cell. Folds are
//! `T016`, soft-wrap `T081`, undercurl `T085`, virtual text `T032`, region
//! tints `T087`.
//!
//! [SPIKES.md]: ../../../../docs/SPIKES.md

use std::collections::HashMap;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::widgets::Widget;

pub use ratatui_code_editor::editor::Editor;

use crate::gutter::Fill;
use crate::theme::{SyntaxMap, Theme};

// ---------------------------------------------------------------------------
// The column contract
// ---------------------------------------------------------------------------

/// Column 1: the state bar. One cell, Design Language §3.
pub const STATE_BAR_WIDTH: u16 = 1;

/// The cell of air between the state bar and the line numbers. `1a` writes
/// line 1 as `<span> </span>  1` — state cell, then two spaces before the
/// digit, of which one is this and one is the number field's own padding.
pub const STATE_BAR_GAP: u16 = 1;

/// The air between the line numbers and the text. Two cells in every mockup.
pub const CODE_GAP: u16 = 2;

/// The narrowest the line-number column ever gets. Every mockup shows a
/// two-digit file (`1a` runs to 24, `1d` to 11, `8e` to 28) with the numbers
/// right-aligned in two cells, so two is the floor rather than a choice.
///
/// The vendored core's own floor is 5, which is why this has to be set at all;
/// see [`configure`].
pub const MIN_LINE_NUMBER_DIGITS: usize = 2;

/// What the 1-cell state bar shows for one row, already resolved.
///
/// **Resolution is not here, and neither is the colour.** Design Language §3's
/// priority — *trouble > attention > claude* — over a row's whole region set is
/// `T031`'s (`GutterBar`), together with the `▎` fallback for terminals that
/// cannot do a background colour. This type is that decision's output, which is
/// what the column contract needs and no more; turning one into a drawable cell
/// is [`crate::gutter::state_cell`], which this widget's render calls for its
/// own column so that the two paints cannot differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StateMark {
    /// Nothing to say about this row: the bar renders as ground.
    #[default]
    None,
    /// Claude wrote this row and you have not looked at it yet (§3, `1a`).
    ClaudeUnseen,
    /// Waiting, paused, dirty, permission (§1).
    Attention,
    /// A diagnostic or a failure (§1). Highest priority.
    Trouble,
}

// ---------------------------------------------------------------------------
// Scroll authority
// ---------------------------------------------------------------------------

/// Where the viewport sits, in rows and columns of the buffer.
///
/// Plain data with no behaviour beyond [`Viewport::scrolled`], which is a pure
/// function. A `Viewport` that differs from the one before it was *asked* to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Viewport {
    /// Index of the visual row drawn at the top of the area.
    pub top_row: usize,
    /// Horizontal scroll, in columns.
    pub left_col: usize,
}

/// Everything [`Viewport::scrolled`] needs that the request itself does not
/// carry: how much there is to scroll through, and how much of it fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewportBounds {
    /// Total rows the buffer would draw — the vendored core's
    /// `visual_len_lines()`, so folds and (later) soft-wrap continuations are
    /// already counted.
    pub rows: usize,
    /// Height of the text area, in rows.
    pub height: usize,
}

/// The only thing that can move a viewport.
///
/// Every variant is a caller saying what it wants; none of them is a side
/// effect of drawing. `T019` carried this shape into the vocabulary as
/// `phosphor_core::request::ScrollRequest`; the host converts between them in
/// one place, and the module header says what collapsing the two would take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollRequest {
    /// Relative, in rows. Negative scrolls towards the top of the buffer.
    Rows(i64),
    /// Relative, in screenfuls.
    Pages(i64),
    /// Relative, in columns. Negative scrolls left.
    Columns(i64),
    /// Absolute: put this visual row at the top.
    ToRow(usize),
    /// The first screenful.
    ToTop,
    /// The last screenful.
    ToBottom,
    /// Bring `row` inside the viewport with at least `margin` rows of context
    /// on the side it entered from, moving as little as possible — and not at
    /// all if it is already there.
    ///
    /// **This is the whole of "follow the cursor", and it is a request.** The
    /// vendored core has its own `focus()` that does this implicitly on every
    /// keystroke; nothing here calls it, because a viewport that moves because
    /// the cursor moved is only legitimate when the caller asked for both.
    RevealRow { row: usize, margin: usize },
}

impl Viewport {
    /// Applies a request. Pure — no editor, no clock, no I/O.
    #[must_use]
    pub fn scrolled(self, request: ScrollRequest, bounds: ViewportBounds) -> Self {
        let height = bounds.height.max(1);
        // The last row may be scrolled to the top, but the buffer may never be
        // scrolled off the screen entirely. This matches the vendored core's
        // own clamp in `set_offset_y`, so applying a viewport can never move it
        // somewhere other than where the arithmetic here said.
        let max_top = bounds.rows.saturating_sub(1);
        let last_page = bounds.rows.saturating_sub(height);

        let mut next = self;
        match request {
            ScrollRequest::Rows(delta) => next.top_row = shift(self.top_row, delta),
            ScrollRequest::Pages(delta) => {
                next.top_row = shift(self.top_row, delta.saturating_mul(height as i64));
            }
            ScrollRequest::Columns(delta) => next.left_col = shift(self.left_col, delta),
            ScrollRequest::ToRow(row) => next.top_row = row,
            ScrollRequest::ToTop => next.top_row = 0,
            ScrollRequest::ToBottom => next.top_row = last_page,
            ScrollRequest::RevealRow { row, margin } => {
                // A margin that does not fit centres instead of fighting itself.
                let margin = margin.min(height.saturating_sub(1) / 2);
                let first = self.top_row.saturating_add(margin);
                let last = self.top_row + height - 1 - margin;
                if row < first {
                    next.top_row = row.saturating_sub(margin);
                } else if row > last {
                    next.top_row = row + 1 + margin - height;
                }
            }
        }
        next.top_row = next.top_row.min(max_top);
        next
    }
}

/// `value + delta`, saturating at both ends of `usize`.
fn shift(value: usize, delta: i64) -> usize {
    if delta >= 0 {
        value.saturating_add(delta as usize)
    } else {
        value.saturating_sub(usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX))
    }
}

/// Reads the viewport an editor is currently showing.
#[must_use]
pub fn viewport_of(editor: &Editor) -> Viewport {
    Viewport {
        top_row: editor.get_offset_y(),
        left_col: editor.get_offset_x(),
    }
}

/// **The single place a buffer's viewport moves.** Invariant 3 is this
/// function's existence plus the absence of any other call site — which
/// `the_viewport_moves_from_exactly_one_place` checks by reading this file.
///
/// `area` is the widget's whole area, the same `Rect` passed to
/// [`BufferView`]'s render; the text height is what pages and reveals are
/// measured in. Returns where the viewport actually ended up.
pub fn apply_scroll(editor: &mut Editor, request: ScrollRequest, area: Rect) -> Viewport {
    let bounds = ViewportBounds {
        rows: editor.visual_len_lines(),
        height: area.height as usize,
    };
    let next = viewport_of(editor).scrolled(request, bounds);
    editor.set_offset_y(next.top_row);
    editor.set_offset_x(next.left_col);
    viewport_of(editor)
}

// ---------------------------------------------------------------------------
// Configuration — off the render path, deliberately
// ---------------------------------------------------------------------------

/// Puts the phosphor contract onto a freshly built [`Editor`]: the column
/// widths, the syntax theme, and the upstream behaviours that are not ours.
///
/// Call it once after `Editor::new`, and again whenever the theme changes.
/// It is idempotent, and it **never touches the viewport** — that is what keeps
/// a theme swap from being a scroll (there is a test).
///
/// **The palette arrives by two roads and they have to be the same theme.**
/// [`BufferView`] paints the ground, the state bar and the `~` rows from the
/// `&Theme` it is handed each frame; the line numbers and every syntax colour
/// are painted by the vendored core from the map installed here, because its
/// highlight cache bakes styles in and re-deriving them per frame would put a
/// tree-sitter query in the frame budget. Rendering with a theme this was not
/// last called with gets you half of each. A theme change is `configure` plus
/// the next frame, never the frame alone.
///
/// Three upstream defaults are turned off here rather than patched out:
///
/// * **The fold gutter.** Enabled upstream, it inserts two more cells between
///   the numbers and the text and draws a `▸`/`▾` beside every foldable line.
///   No mockup has that column — `8e` renders its fold marker inline, after the
///   code — and it would make the gutter eight cells wide. `T016` owns folds
///   and will need `left_code_padding` to absorb the fold gutter rather than
///   sit beside it.
/// * **Word-occurrence highlighting.** Upstream tints every other occurrence of
///   the word under the cursor. It appears in no mockup and it is movement on
///   screen that nobody asked for.
/// * **Line numbers stay on**, at [`MIN_LINE_NUMBER_DIGITS`] rather than
///   upstream's floor of five.
pub fn configure(editor: &mut Editor, theme: &Theme) {
    editor.show_line_numbers(true);
    editor.set_line_number_min_digits(MIN_LINE_NUMBER_DIGITS);
    editor.set_left_code_padding(CODE_GAP as usize);
    editor.set_code_folding_enabled(false);
    editor.set_word_highlight_enabled(false);
    editor.set_theme(syntax_theme(theme));
}

/// The buffer's whole gutter — everything left of the text — in cells.
///
/// Read back from the editor rather than recomputed, so the overpaint can
/// never drift from where the vendored core actually put the text.
#[must_use]
pub fn gutter_width(editor: &Editor) -> u16 {
    STATE_BAR_WIDTH + STATE_BAR_GAP + editor.get_line_number_width() as u16
}

/// The sub-rect the vendored editor is rendered into: the widget's area minus
/// the two cells in front of the line numbers, which are ours.
///
/// **Anything that hands the vendored core an `area` must use this** — its
/// cursor placement and its click-to-offset mapping both measure from
/// `area.left()`, so a caller passing the outer rect would put the cursor two
/// cells to the left of the character it is on.
#[must_use]
pub fn editor_area(area: Rect) -> Rect {
    let inset = STATE_BAR_WIDTH + STATE_BAR_GAP;
    Rect {
        x: area.x.saturating_add(inset.min(area.width)),
        y: area.y,
        width: area.width.saturating_sub(inset),
        height: area.height,
    }
}

/// Phosphor's [`Theme`] as the vendored core's capture-name → [`Style`] map.
///
/// The lookup in the core is an exact match on the tree-sitter capture name
/// with no dotted fallback (`code.rs`), so every name that must not render as
/// plain text has to appear here in full. The names are the union of the ten
/// grammars the fork bundles for us, read out of
/// `vendor/ratatui-code-editor/langs/*/highlights.scm`.
///
/// **Everything absent from this map renders as `default_text`** — identifiers,
/// members, parameters, namespaces, operators, punctuation, properties. That is
/// not an oversight: `1a` draws `std::thread`, `max_attempts`, `policy` and
/// every delimiter in plain `#c6cec6`, and the eight fields of [`SyntaxMap`]
/// are the whole vocabulary a theme owns.
#[must_use]
pub fn syntax_theme(theme: &Theme) -> HashMap<String, Style> {
    let SyntaxMap {
        text,
        keyword,
        ty,
        function,
        constant,
        string,
        number,
        comment,
    } = theme.syntax;

    let groups: [(Color, &[&str]); 9] = [
        // Not a capture name — the two keys patch 4 added to the vendored
        // renderer so the gutter and unclassified text stop being hardcoded.
        (theme.neutrals.line_numbers, &["line_number"]),
        (text, &["default_text"]),
        (
            keyword,
            &[
                "keyword",
                "keyword.control",
                "keyword.control.import",
                "keyword.control.repeat",
                "keyword.operator",
                "keyword.storage.modifier.ref",
                // CSS at-rules read as keywords in every palette that has one.
                "charset",
                "import",
                "keyframes",
                "media",
                "preproc",
                "supports",
            ],
        ),
        (ty, &["type", "type.builtin", "type.enum.variant"]),
        (
            function,
            &[
                "function",
                "function.builtin",
                "function.macro",
                "function.method",
            ],
        ),
        (
            constant,
            &[
                "boolean",
                "constant",
                "constant.builtin",
                "constant.builtin.boolean",
                "constructor",
            ],
        ),
        (
            number,
            &[
                "constant.numeric",
                "constant.numeric.float",
                "constant.numeric.integer",
                "number",
            ],
        ),
        (
            string,
            &[
                "constant.character",
                "constant.character.escape",
                "string",
                "string.escape",
                "string.regex",
                "string.special",
            ],
        ),
        (comment, &["comment"]),
    ];

    let mut map = HashMap::new();
    for (colour, names) in groups {
        for name in names {
            map.insert((*name).to_owned(), Style::default().fg(colour));
        }
    }
    map
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// The central surface: state bar, line numbers, text.
///
/// Holds `&Editor`, never `&mut` — which is how "rendering cannot scroll"
/// stops being a promise and becomes a compile error.
#[derive(Clone, Copy)]
pub struct BufferView<'a> {
    editor: &'a Editor,
    theme: &'a Theme,
    state_column: &'a [StateMark],
}

impl std::fmt::Debug for BufferView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Editor` is not `Debug`; the two numbers that matter for a failing
        // test are the viewport and the gutter width.
        f.debug_struct("BufferView")
            .field("viewport", &viewport_of(self.editor))
            .field("gutter_width", &gutter_width(self.editor))
            .field("theme", &self.theme.name)
            .field("state_column", &self.state_column.len())
            .finish()
    }
}

impl<'a> BufferView<'a> {
    /// A view over `editor`, painted with `theme`.
    #[must_use]
    pub fn new(editor: &'a Editor, theme: &'a Theme) -> Self {
        Self {
            editor,
            theme,
            state_column: &[],
        }
    }

    /// The state bar's contents, indexed by **visual row** — the same
    /// coordinate space as [`Viewport::top_row`], not by screen row. Rows past
    /// the end of the slice are [`StateMark::None`].
    #[must_use]
    pub fn state_column(mut self, marks: &'a [StateMark]) -> Self {
        self.state_column = marks;
        self
    }

    /// Where the text column starts, relative to the widget's left edge.
    #[must_use]
    pub fn gutter_width(&self) -> u16 {
        gutter_width(self.editor)
    }

    /// The viewport this view is showing.
    #[must_use]
    pub fn viewport(&self) -> Viewport {
        viewport_of(self.editor)
    }

    fn mark_at(&self, visual_row: usize) -> StateMark {
        self.state_column
            .get(visual_row)
            .copied()
            .unwrap_or_default()
    }
}

impl Widget for BufferView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Ground under everything. §1: `#0c0f0c` is the editor background, and
        // the terminal's own default is not it.
        buf.set_style(area, Style::default().bg(self.theme.neutrals.ground));

        // The vendored core draws columns 2 and 3 — its own line numbers at its
        // left edge, then the text. `&Editor`: this call cannot scroll.
        self.editor.render(editor_area(area), buf);

        let top_row = self.editor.get_offset_y();
        let rows = self.editor.visual_len_lines();
        let digits = self.editor.line_number_digits() as u16;
        let line_number_style = Style::default().fg(self.theme.neutrals.line_numbers);

        for dy in 0..area.height {
            let y = area.y + dy;
            let visual_row = top_row + dy as usize;

            // Column 1, and the cell of air behind it. Written explicitly
            // rather than left to the ground fill so a stale symbol from a
            // widget drawn underneath cannot survive in the gutter.
            //
            // Through `gutter::state_cell` rather than a hue lookup of its own:
            // this column is drawn in two widgets and a `StateMark` becomes a
            // cell in exactly one place (`R9` in `docs/OPEN-QUESTIONS.md`).
            let (symbol, style) =
                crate::gutter::state_cell(self.mark_at(visual_row), self.theme, Fill::Block);
            set_cell(buf, area, area.x, y, symbol, style);
            set_cell(
                buf,
                area,
                area.x + STATE_BAR_WIDTH,
                y,
                " ",
                Style::default().bg(self.theme.neutrals.ground),
            );

            // Past the end of the buffer: `~`, right-aligned in the number
            // column, in the line-number colour (`1d`). The vendored core draws
            // nothing at all down here.
            if visual_row >= rows && digits > 0 {
                let x = area.x + STATE_BAR_WIDTH + STATE_BAR_GAP + digits - 1;
                set_cell(buf, area, x, y, "~", line_number_style);
            }
        }
    }
}

/// One cell, clipped to `area` as well as to the buffer.
fn set_cell(buf: &mut Buffer, area: Rect, x: u16, y: u16, symbol: &str, style: Style) {
    if x < area.right() && y < area.bottom() {
        buf.set_string(x, y, symbol, style);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup `1a`'s buffer, transcribed. Twenty-four lines, so two digits —
    /// the file the checkpoint holds up next to the screen.
    const RETRY_RS: &str = "\
use std::thread;
use std::time::Duration;

use crate::util::jitter;

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

pub fn retry_with_backoff<T, E>(
    mut op: impl FnMut() -> Result<T, E>,
    policy: &RetryPolicy,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt + 1 == policy.max_attempts => return Err(e),
            Err(_) => thread::sleep(jitter(delay)),
        }
        delay = (delay * 2).min(policy.max_delay);
    }
";

    fn editor(text: &str) -> Editor {
        let theme = Theme::phosphor_dark();
        let mut editor = Editor::new("rust", text, Vec::new()).expect("editor");
        configure(&mut editor, &theme);
        editor
    }

    fn render(editor: &Editor, area: Rect) -> Buffer {
        render_with(editor, area, &[])
    }

    fn render_with(editor: &Editor, area: Rect, marks: &[StateMark]) -> Buffer {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(area);
        BufferView::new(editor, &theme)
            .state_column(marks)
            .render(area, &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect()
    }

    // -- the column contract ------------------------------------------------

    #[test]
    fn the_gutter_is_six_cells_on_every_mockup() {
        // 1a runs to 24, 1d to 11, 8e to 28 — all two-digit, all six cells:
        // state bar 1 + air 1 + digits 2 + air 2.
        let editor = editor(RETRY_RS);
        assert_eq!(editor.line_number_digits(), 2);
        assert_eq!(gutter_width(&editor), 6);
    }

    #[test]
    fn the_gutter_grows_only_with_the_digit_count() {
        for (lines, expected) in [(1, 6), (9, 6), (10, 6), (99, 6), (100, 7), (1000, 8)] {
            let text = vec!["x"; lines].join("\n");
            let editor = editor(&text);
            assert_eq!(
                gutter_width(&editor),
                expected,
                "{lines}-line file: 1 + 1 + digits + 2"
            );
        }
    }

    #[test]
    fn the_columns_land_where_the_mockups_put_them() {
        // Screen 9c, first row: state bar, air, right-aligned "16", two cells
        // of air, then the code at column 6 with its own four-space indent.
        let mut editor = editor(RETRY_RS);
        apply_scroll(
            &mut editor,
            ScrollRequest::ToRow(15),
            Rect::new(0, 0, 80, 5),
        );
        let area = Rect::new(0, 0, 60, 5);
        let buf = render(&editor, area);

        assert_eq!(
            row_text(&buf, area, 0).trim_end(),
            "  16      let mut delay = policy.base_delay;"
        );
        assert_eq!(
            row_text(&buf, area, 1).trim_end(),
            "  17      for attempt in 0..policy.max_attempts {"
        );
        // The gutter is padding, not content: the row is exactly as wide as the
        // area, and nothing hangs off either end.
        assert_eq!(row_text(&buf, area, 0).chars().count(), area.width as usize);
    }

    #[test]
    fn line_numbers_are_the_theme_colour_and_nothing_else() {
        // §3: "Column 2: line numbers, always #414b42." Checked as "whatever
        // the theme's field holds" — a second theme carrying an unmistakably
        // different value in that field proves the widget reads the field
        // rather than agreeing with it by coincidence. The substitute is
        // another *theme* colour, so no value enters this file.
        let mut editor = editor(RETRY_RS);
        let area = Rect::new(0, 0, 40, 6);

        let mut recoloured = Theme::phosphor_dark();
        recoloured.neutrals.line_numbers = recoloured.actors.claude;
        let recoloured_expects = recoloured.actors.claude;

        for (theme, expected) in [
            (
                Theme::phosphor_dark(),
                Theme::phosphor_dark().neutrals.line_numbers,
            ),
            (recoloured, recoloured_expects),
        ] {
            // Both halves of the palette move together — see `configure`.
            configure(&mut editor, &theme);
            let mut buf = Buffer::empty(area);
            BufferView::new(&editor, &theme).render(area, &mut buf);
            for y in 0..area.height {
                for x in STATE_BAR_WIDTH + STATE_BAR_GAP..gutter_width(&editor) - CODE_GAP {
                    assert_eq!(buf[(x, y)].fg, expected, "line-number cell ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn the_state_bar_is_column_zero_and_carries_the_actor_hue() {
        let theme = Theme::phosphor_dark();
        let editor = editor(RETRY_RS);
        let area = Rect::new(0, 0, 40, 4);
        let marks = [
            StateMark::None,
            StateMark::ClaudeUnseen,
            StateMark::Attention,
            StateMark::Trouble,
        ];
        let buf = render_with(&editor, area, &marks);

        let expected = [
            theme.neutrals.ground,
            theme.actors.claude,
            theme.actors.attention,
            theme.actors.trouble,
        ];
        for (y, want) in expected.into_iter().enumerate() {
            let cell = &buf[(0, y as u16)];
            assert_eq!(
                cell.symbol(),
                " ",
                "the bar is a coloured cell, not a glyph"
            );
            assert_eq!(cell.bg, want, "state bar row {y}");
        }
        // And nothing bled into column 1.
        for y in 0..area.height {
            assert_eq!(buf[(1, y)].bg, theme.neutrals.ground);
        }
    }

    #[test]
    fn rows_past_the_end_of_the_buffer_are_tildes() {
        // Screen 1d: `~` right-aligned in the line-number column, in the
        // line-number colour, and nothing in the text column.
        let theme = Theme::phosphor_dark();
        let editor = editor("one\ntwo\n");
        let area = Rect::new(0, 0, 20, 6);
        let buf = render(&editor, area);

        assert_eq!(row_text(&buf, area, 0), "   1  one           ");
        assert_eq!(row_text(&buf, area, 3), "   ~                ");
        assert_eq!(buf[(3, 3)].fg, theme.neutrals.line_numbers);
    }

    #[test]
    fn highlighting_reaches_the_cells_through_the_vendored_core() {
        // T015's other half: tree-sitter, not a colouring of our own. `pub` on
        // line 6 is a keyword, `RetryPolicy` a type, and `max_attempts` is
        // neither — it renders in the plain text colour, as `1a` draws it.
        let theme = Theme::phosphor_dark();
        let editor = editor(RETRY_RS);
        let area = Rect::new(0, 0, 60, 24);
        let buf = render(&editor, area);
        let gutter = gutter_width(&editor);

        let row = 5; // "pub struct RetryPolicy {"
        assert_eq!(buf[(gutter, row)].symbol(), "p");
        assert_eq!(buf[(gutter, row)].fg, theme.syntax.keyword);
        assert_eq!(buf[(gutter + 11, row)].symbol(), "R");
        assert_eq!(buf[(gutter + 11, row)].fg, theme.syntax.ty);

        let row = 6; // "    pub max_attempts: u32,"
        assert_eq!(buf[(gutter + 8, row)].symbol(), "m");
        assert_eq!(buf[(gutter + 8, row)].fg, theme.syntax.text);
    }

    #[test]
    fn the_ground_is_the_themes_and_covers_the_whole_area() {
        let theme = Theme::phosphor_dark();
        let editor = editor("fn main() {}\n");
        let area = Rect::new(3, 2, 30, 8);
        let buf = render(&editor, area);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                assert_eq!(buf[(x, y)].bg, theme.neutrals.ground, "cell ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_degenerate_area_does_not_panic() {
        let editor = editor(RETRY_RS);
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 3, 2),
            Rect::new(0, 0, 6, 1),
        ] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
            BufferView::new(&editor, &Theme::phosphor_dark()).render(area, &mut buf);
        }
    }

    // -- "the viewport provably never self-scrolls" -------------------------

    /// The perturbation matrix. Everything that could plausibly be wired to
    /// move the viewport behind the user's back, applied to one editor, with
    /// the offsets checked after each one.
    ///
    /// The compile-time half of the proof is that [`BufferView`] holds
    /// `&Editor`: `render` has no `&mut` to scroll with. This is the half that
    /// survives someone changing that.
    #[test]
    fn nothing_but_a_request_moves_the_viewport() {
        let theme = Theme::phosphor_dark();
        let mut editor = editor(RETRY_RS);
        let area = Rect::new(0, 0, 80, 8);

        // Put it somewhere interesting: not the top, not the bottom.
        apply_scroll(&mut editor, ScrollRequest::ToRow(9), area);
        let baseline = viewport_of(&editor);
        assert_eq!(baseline.top_row, 9);

        let check = |editor: &Editor, what: &str| {
            assert_eq!(
                viewport_of(editor),
                baseline,
                "the viewport moved on its own: {what}"
            );
        };

        // 1. Rendering, repeatedly, at every width and height in the matrix —
        //    including ones far too small to hold the cursor's line.
        for width in [40u16, 60, 80, 100, 120, 200, 7, 3] {
            for height in [1u16, 2, 5, 8, 24, 60] {
                let a = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(Rect::new(0, 0, 200, 60));
                BufferView::new(&editor, &theme).render(a, &mut buf);
                check(&editor, &format!("render at {width}x{height}"));
            }
        }

        // 2. Moving the cursor — including far outside the viewport, which is
        //    exactly what the vendored core's own `focus()` would chase.
        for line in [0usize, 3, 15, 23] {
            let offset = editor.code_ref().line_to_char(line);
            editor.set_cursor(offset);
            check(&editor, &format!("set_cursor to line {line}"));
            let mut buf = Buffer::empty(Rect::new(0, 0, 80, 8));
            BufferView::new(&editor, &theme).render(area, &mut buf);
            check(&editor, &format!("render after cursor to line {line}"));
        }

        // 3. Marks arriving and being cleared — the region-tint path (T087).
        editor.set_marks_colored(vec![
            (0, 40, theme.regions.anchor),
            (300, 360, theme.regions.failure),
        ]);
        check(&editor, "set_marks");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 8));
        BufferView::new(&editor, &theme).render(area, &mut buf);
        check(&editor, "render with marks");
        editor.remove_marks();
        check(&editor, "remove_marks");

        // 4. Re-theming, and the highlight-cache invalidation it implies.
        configure(&mut editor, &theme);
        check(&editor, "configure (theme swap)");
        editor.reset_highlight_cache();
        check(&editor, "reset_highlight_cache");

        // 5. A cold cache: first render after invalidation is the one that
        //    actually runs tree-sitter.
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 8));
        BufferView::new(&editor, &theme).render(area, &mut buf);
        check(&editor, "render on a cold highlight cache");

        // 6. Selection, which upstream draws but must not chase.
        editor.set_selection(None);
        check(&editor, "set_selection");
    }

    /// The structural half: a second call site to the vendored core's viewport
    /// mutators, anywhere in this file outside [`apply_scroll`], fails here.
    ///
    /// This is the test the task asks for by name — the one that fails if a
    /// future change lets a highlight, a resize, or a cursor move nudge the
    /// viewport on its own. `nothing_but_a_request_moves_the_viewport` proves
    /// today's behaviour; this proves nobody can quietly add tomorrow's.
    #[test]
    fn the_viewport_moves_from_exactly_one_place() {
        let source = include_str!("buffer_view.rs");
        let (code, _) = source
            .split_once("\n#[cfg(test)]")
            .expect("this file ends in its own test module");
        // Comment lines describing the rule are not the rule being broken.
        let code: String = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        let signature = "pub fn apply_scroll";
        let start = code.find(signature).expect("apply_scroll is still here");
        let end = start
            + code[start..]
                .find("\n}")
                .expect("apply_scroll closes at column zero");

        // `focus()` is upstream's scroll-to-the-cursor, called from its
        // temporary crossterm handler on every keystroke. It must not appear.
        assert!(
            !code.contains(".focus("),
            "Editor::focus scrolls to follow the cursor; use ScrollRequest::RevealRow"
        );

        for mutator in [
            ".set_offset_y(",
            ".set_offset_x(",
            ".scroll_up(",
            ".scroll_down(",
        ] {
            for (at, _) in code.match_indices(mutator) {
                assert!(
                    at > start && at < end,
                    "{mutator} is called outside apply_scroll — the viewport has \
                     grown a second author, and invariant 3 with it"
                );
            }
        }
    }

    #[test]
    fn configure_is_not_a_scroll() {
        let mut editor = editor(RETRY_RS);
        apply_scroll(
            &mut editor,
            ScrollRequest::ToRow(12),
            Rect::new(0, 0, 80, 6),
        );
        let before = viewport_of(&editor);
        for _ in 0..3 {
            configure(&mut editor, &Theme::phosphor_dark());
            assert_eq!(viewport_of(&editor), before, "configure moved the viewport");
        }
    }

    // -- the scroll arithmetic ----------------------------------------------

    fn bounds(rows: usize, height: usize) -> ViewportBounds {
        ViewportBounds { rows, height }
    }

    #[test]
    fn relative_scrolling_clamps_at_both_ends() {
        let b = bounds(24, 8);
        let v = Viewport::default();
        assert_eq!(v.scrolled(ScrollRequest::Rows(-1), b).top_row, 0);
        assert_eq!(v.scrolled(ScrollRequest::Rows(3), b).top_row, 3);
        assert_eq!(v.scrolled(ScrollRequest::Rows(i64::MAX), b).top_row, 23);
        assert_eq!(v.scrolled(ScrollRequest::Pages(2), b).top_row, 16);
        assert_eq!(v.scrolled(ScrollRequest::Pages(-2), b).top_row, 0);
    }

    #[test]
    fn to_bottom_shows_the_last_screenful() {
        assert_eq!(
            Viewport::default()
                .scrolled(ScrollRequest::ToBottom, bounds(24, 8))
                .top_row,
            16
        );
        // A buffer shorter than the screen has nowhere to go.
        assert_eq!(
            Viewport::default()
                .scrolled(ScrollRequest::ToBottom, bounds(3, 8))
                .top_row,
            0
        );
    }

    #[test]
    fn reveal_moves_as_little_as_possible_and_not_at_all_when_it_can() {
        let b = bounds(100, 10);
        let v = Viewport {
            top_row: 20,
            left_col: 0,
        };
        let reveal = |row, margin| {
            v.scrolled(ScrollRequest::RevealRow { row, margin }, b)
                .top_row
        };
        assert_eq!(reveal(25, 0), 20, "already visible: no movement");
        assert_eq!(reveal(20, 0), 20, "the top row is visible");
        assert_eq!(reveal(29, 0), 20, "the bottom row is visible");
        assert_eq!(reveal(30, 0), 21, "one row past the bottom: one row down");
        assert_eq!(reveal(15, 0), 15, "above: the row becomes the top");
        assert_eq!(reveal(15, 3), 12, "with a margin of context above it");
        assert_eq!(reveal(30, 3), 24, "and below it");
        // A row inside the margin band is visible but has no context: reveal
        // buys it the margin it asked for rather than calling it good enough.
        assert_eq!(reveal(22, 3), 19, "a row inside the top margin scrolls up");
        assert_eq!(reveal(27, 3), 21, "and inside the bottom margin, down");
    }

    #[test]
    fn a_margin_that_cannot_fit_does_not_thrash() {
        // Two rows of screen cannot hold five rows of context on both sides.
        let b = bounds(100, 2);
        let v = Viewport {
            top_row: 50,
            left_col: 0,
        };
        let next = v.scrolled(ScrollRequest::RevealRow { row: 80, margin: 5 }, b);
        assert!(next.top_row <= 80 && next.top_row + 2 > 80, "{next:?}");
    }

    #[test]
    fn horizontal_scrolling_stops_at_the_left_edge() {
        let b = bounds(24, 8);
        let v = Viewport {
            top_row: 0,
            left_col: 4,
        };
        assert_eq!(v.scrolled(ScrollRequest::Columns(-9), b).left_col, 0);
        assert_eq!(v.scrolled(ScrollRequest::Columns(6), b).left_col, 10);
        assert_eq!(
            v.scrolled(ScrollRequest::Columns(2), b).top_row,
            0,
            "a horizontal scroll is not a vertical one"
        );
    }

    #[test]
    fn an_empty_buffer_has_nowhere_to_scroll() {
        let b = bounds(0, 10);
        let v = Viewport::default();
        for request in [
            ScrollRequest::Rows(5),
            ScrollRequest::Pages(1),
            ScrollRequest::ToBottom,
            ScrollRequest::ToRow(9),
            ScrollRequest::RevealRow { row: 4, margin: 2 },
        ] {
            assert_eq!(v.scrolled(request, b).top_row, 0, "{request:?}");
        }
    }

    #[test]
    fn applying_a_scroll_lands_where_the_arithmetic_said() {
        // The vendored core clamps in `set_offset_y` too; if the two clamps
        // ever disagree, a caller's returned Viewport would be a lie.
        let mut editor = editor(RETRY_RS);
        let area = Rect::new(0, 0, 80, 6);
        let rows = editor.visual_len_lines();
        for request in [
            ScrollRequest::ToTop,
            ScrollRequest::ToBottom,
            ScrollRequest::Rows(5),
            ScrollRequest::Rows(-2),
            ScrollRequest::Pages(9),
            ScrollRequest::ToRow(usize::MAX),
            ScrollRequest::RevealRow { row: 23, margin: 2 },
        ] {
            let expected = viewport_of(&editor).scrolled(
                request,
                ViewportBounds {
                    rows,
                    height: area.height as usize,
                },
            );
            let got = apply_scroll(&mut editor, request, area);
            assert_eq!(got, expected, "{request:?}");
        }
    }

    // -- the syntax map -----------------------------------------------------

    #[test]
    fn the_syntax_map_never_shadows_an_injection() {
        // The vendored core checks the theme for the capture name *first* and
        // only falls through to the `injection.content.<lang>` handling when it
        // finds nothing. A key that starts with that prefix would silently turn
        // off every injected language.
        let map = syntax_theme(&Theme::phosphor_dark());
        assert!(
            !map.keys().any(|k| k.starts_with("injection.")),
            "an injection capture is in the theme map"
        );
        assert!(map.contains_key("line_number"));
        assert!(map.contains_key("default_text"));
    }

    #[test]
    fn the_syntax_map_carries_only_theme_colours() {
        let theme = Theme::phosphor_dark();
        let map = syntax_theme(&theme);
        let allowed = [
            theme.neutrals.line_numbers,
            theme.syntax.text,
            theme.syntax.keyword,
            theme.syntax.ty,
            theme.syntax.function,
            theme.syntax.constant,
            theme.syntax.string,
            theme.syntax.number,
            theme.syntax.comment,
        ];
        for (name, style) in &map {
            let fg = style.fg.expect("every entry sets a foreground");
            assert!(allowed.contains(&fg), "{name} is not a theme colour");
        }
    }
}
