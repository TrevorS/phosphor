//! PHOSPHOR PATCH 11 — a tab advances to the next tabstop.
//!
//! `T104`. **Every test here fails on the unpatched crate**, and each says how
//! in its own doc comment. Upstream measured `\t` with `unicode_width` — which
//! answers `1` — and drew `g.to_string().replace('\t', " ")`, so a tab was one
//! cell everywhere: on screen, under the cursor, under the mouse and at the
//! wrap point.
//!
//! The assertions read the **drawn buffer** rather than a width function, which
//! is the point: the defect `CP-4` reported was *"tab only seems to go a space
//! at a time when indenting"*, and a width helper answering 4 while the
//! renderer painted 1 would satisfy every arithmetic test and none of these.
//!
//! Columns are reported relative to the text column, because
//! `Editor::get_line_number_width` reserves a gutter even with line numbers off
//! (`left_code_padding` plus the fold column) and a test that hardcoded the
//! offset would break on a gutter change that has nothing to do with tabs.

use ratatui_code_editor::editor::Editor;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 24,
    height: 4,
};

fn editor(text: &str) -> Editor {
    let mut editor = Editor::new("text", text, vec![]).unwrap();
    editor.show_line_numbers(false);
    editor
}

fn drawn(editor: &Editor) -> Buffer {
    let mut buf = Buffer::empty(AREA);
    editor.render(AREA, &mut buf);
    buf
}

/// Text column of the first cell on `y` drawing `symbol`.
///
/// A cell and not a character: a wide grapheme leaves its second cell blank,
/// which is exactly what makes "column" mean cells here.
fn column_of(editor: &Editor, buf: &Buffer, y: u16, symbol: &str) -> usize {
    let gutter = editor.get_line_number_width();
    for x in gutter as u16..buf.area.width {
        if buf[(x, y)].symbol() == symbol {
            return x as usize - gutter;
        }
    }
    panic!("{symbol:?} was never drawn on row {y}");
}

/// **The report, exactly.** `\tx` drew the `x` at column 1 — one cell of indent
/// — and now draws it at 4, where a four-column tabstop puts it.
#[test]
fn a_tab_at_the_start_of_a_line_reaches_the_tabstop() {
    let editor = editor("\tx\n");
    let buf = drawn(&editor);
    assert_eq!(column_of(&editor, &buf, 0, "x"), 4);
}

/// **A tab is not a fixed number of spaces**, which is the whole difference
/// between a tabstop and a substitution: the same `\t` spends 3 cells after one
/// character and 1 after three. A renderer expanding `\t` to `tab_width` spaces
/// passes the test above and fails this one, putting the `x`s at 5 and 7.
#[test]
fn a_tab_finishes_the_column_it_starts_in() {
    let editor = editor("a\tx\nabc\tx\n");
    let buf = drawn(&editor);
    assert_eq!(column_of(&editor, &buf, 0, "x"), 4);
    assert_eq!(column_of(&editor, &buf, 1, "x"), 4);
}

/// **Cells, not characters** — the arithmetic this repo has shipped three bugs
/// from. `漢` is two cells wide, so a tab after two of them starts at column 4
/// and spends a whole stop, and a tab after three starts at 6 and spends two.
/// A walk counting `chars` would put both `x`s at 4.
#[test]
fn a_tab_after_cjk_measures_from_the_cells_it_left_behind() {
    let editor = editor("漢漢\tx\n漢漢漢\tx\n");
    let buf = drawn(&editor);
    assert_eq!(column_of(&editor, &buf, 0, "x"), 8);
    assert_eq!(column_of(&editor, &buf, 1, "x"), 8);
}

/// **The tabstop is a setting**, and the row stream is rebuilt when it moves.
#[test]
fn the_tabstop_is_what_it_was_set_to() {
    let mut editor = editor("\tx\n");
    editor.set_tab_width(8);
    let buf = drawn(&editor);
    assert_eq!(column_of(&editor, &buf, 0, "x"), 8);
    assert_eq!(editor.tab_width(), 8);
}

/// **The cursor is drawn where the text is.** Upstream summed grapheme widths
/// for the cursor column and measured the tab as one, so a cursor on the `x` of
/// `\tx` was drawn on top of the indent while the `x` was painted at 4.
#[test]
fn the_cursor_lands_on_the_column_the_tab_pushed_the_text_to() {
    let mut editor = editor("\tx\n");
    editor.set_cursor(1);
    let gutter = editor.get_line_number_width() as u16;
    assert_eq!(editor.get_visible_cursor(&AREA), Some((gutter + 4, 0)));
}

/// **A click lands on the character under the pointer.** With a tab measured as
/// one cell, every column from 1 to 4 of `\tx` mapped past the `x`.
#[test]
fn a_click_inside_a_tab_stays_on_the_tab() {
    let editor = editor("\tx\n");
    let gutter = editor.get_line_number_width() as u16;
    // Columns 0..4 are the tab's own cells; column 4 is the `x`.
    assert_eq!(editor.cursor_from_mouse(gutter, 0, &AREA), Some(0));
    assert_eq!(editor.cursor_from_mouse(gutter + 3, 0, &AREA), Some(0));
    assert_eq!(editor.cursor_from_mouse(gutter + 4, 0, &AREA), Some(1));
}

/// **A tab inside a selection is selected for its whole width.** The renderer
/// painted one cell for it, so the other three kept whatever background was
/// under them.
#[test]
fn a_selected_tab_is_painted_across_every_cell_it_spends() {
    use ratatui_code_editor::selection::Selection;
    let mut editor = editor("\tx\n");
    editor.set_selection(Some(Selection { start: 0, end: 2 }));
    let buf = drawn(&editor);
    let gutter = editor.get_line_number_width() as u16;
    let selected = buf[(gutter, 0)].bg;
    for cell in 1..4u16 {
        assert_eq!(
            buf[(gutter + cell, 0)].bg,
            selected,
            "cell {cell} of the tab kept a different background"
        );
    }
}

/// **A tab on a continuation row is measured from where the *line* has reached,
/// not from where the row has.** The renderer's `base_col`, which is the one
/// piece of this patch a person reading a row of the screen cannot see.
///
/// A `↪` row starts its text at screen column zero-plus-the-marker, and a
/// tabstop is absolute — so a row-relative measurement puts the same tab in a
/// different place depending on where the line happened to wrap.
///
/// The fixture wraps at 20 (the text column of [`AREA`]) and breaks at its one
/// space, which is what leaves the third row starting at line column 21 — a
/// column no multiple of the four-cell stop lands on. The tab there spends
/// **one** cell and puts the `X` at 5; measured from the row's own zero it
/// would spend two and put it at 6.
#[test]
fn a_tab_on_a_continuation_row_measures_from_the_line_and_not_the_row() {
    let mut editor = editor(&format!("aa {}\tX", "b".repeat(20)));
    editor.set_soft_wrap(Some(AREA.width as usize - editor.get_line_number_width()));
    assert_eq!(
        editor.visual_len_lines(),
        3,
        "the fixture is written for three rows and the last one is the assertion"
    );

    let buf = drawn(&editor);
    assert_eq!(column_of(&editor, &buf, 2, "X"), 5);

    // And again with that row drawn **first**, which is the other half of the
    // same answer: the base column is carried from the row above where there is
    // one and walked from the line start where there is not, and a viewport
    // scrolled into the middle of a wrapped line has no row above to carry.
    // The tab may not move because the screen did.
    editor.set_offset_y(2);
    let scrolled = drawn(&editor);
    assert_eq!(column_of(&editor, &scrolled, 0, "X"), 5);
}

/// **A grapheme wider than the cells left to it is dropped, not painted past
/// the area.** The second behaviour change riding on this patch, disclosed in
/// `VENDOR.md` §11 and asserted by nothing until now.
///
/// The clamp that puts a tab inside its own row (`room` in the grapheme loop)
/// replaced a `set_string`, and `set_string` stops at the **buffer** edge
/// rather than the area's — so a two-cell CJK grapheme with one cell of text
/// column left painted into whatever the host had composed to the right of the
/// editor. That is not a tab defect and it is not visible in any of the tests
/// above, all of which render into a buffer exactly the size of the editor,
/// where the buffer edge and the area edge are the same line.
///
/// So this one renders into a buffer four columns wider than the area and puts
/// a host's own composition in the column beside it. Revert `room` to
/// `set_string` and the `│` at column 24 is gone.
#[test]
fn a_wide_grapheme_with_one_cell_left_stays_inside_the_area() {
    // The gutter is read off an editor with the same line count, because
    // `get_line_number_width` reserves by digits and a one-line buffer and a
    // two-line one do not reserve alike.
    let gutter = editor("x").get_line_number_width();
    let text = AREA.width as usize - gutter;
    // (text - 1) cells of filler, then a grapheme wanting two where one is
    // left.
    let editor = editor(&format!("{}漢", "a".repeat(text - 1)));

    let wide = Rect {
        width: AREA.width + 4,
        ..AREA
    };
    let mut buf = Buffer::empty(wide);
    for y in 0..wide.height {
        buf[(AREA.width, y)].set_symbol("│");
    }
    editor.render(AREA, &mut buf);

    assert_eq!(
        buf[(AREA.width, 0)].symbol(),
        "│",
        "the second cell of `漢` landed outside the editor's area, over what the host \
         had drawn there"
    );
}

/// **Soft wrap breaks where the text actually is.** `segments` measured a tab
/// as one cell, so a line twelve cells wide was judged eight and never wrapped.
///
/// No trailing newline, deliberately: `"…\n"` is two lines to ropey and an
/// empty second one would make `> 1` true whether or not anything wrapped.
#[test]
fn a_wrap_point_counts_the_cells_a_tab_spends() {
    let mut editor = editor("\t\tabcd");
    assert_eq!(editor.code_ref().len_lines(), 1);
    editor.set_soft_wrap(Some(8));
    assert!(
        editor.visual_len_lines() > 1,
        "a line twelve cells wide did not wrap at eight"
    );
}
