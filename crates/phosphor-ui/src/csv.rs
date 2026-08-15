//! Virtual column alignment (`T082`) — CSV's bespoke surface.
//!
//! *"CSV/Markdown getting bespoke surfaces (virtual column alignment, live
//! preview) rather than generic buffer treatment"* (Component Breakdown, the
//! first-class languages). **Virtual** is the load-bearing word: the columns
//! line up on screen and the file on disk does not change by a byte. What this
//! module produces is padding, measured in terminal cells, and the padding is
//! never written anywhere the buffer can see.
//!
//! # Why the widths are computed here and not with the parser
//!
//! `phosphor_buffer::csv` answers *what the fields are*; a column is *how wide
//! the widest of them is on this terminal*, which is a display question. A
//! field of `名前` is two characters and four cells, and a layout computed in
//! `chars()` is visibly wrong on the first CJK file anybody opens.
//!
//! This module carried a byte-identical copy of [`crate::interpret::cells`] for
//! a window, which was a defect — three copies of two lines is three places for
//! the measurement a widget lays out with to drift from the one it draws with,
//! as that function's own doc says. Deleting it turned out to expose a second
//! and worse one: `interpret::cells` is not the measurement anything paints
//! with. [`cells`] below has the counterexamples and the CONTRACT.
//!
//! The seam between the two halves is `&str`: [`Layout::measure`] takes rows of
//! string slices and nothing else, so this module has no dependency on the
//! parser and the parser has none on a renderer.
//!
//! # The seam on the other side: [`Run`], not a `Line`
//!
//! [`row_runs`] hands back the escape hatch's own currency, so an aligned row
//! reaches the screen through `Node::Spans` and `interpret.rs` — the one place
//! in this crate that turns composition-supplied text into cells (`T080`,
//! `scripts/lint-one-escape-hatch.sh`). It returned a `ratatui_core::text::Line`
//! for a window, which nothing in this repo draws: every widget here writes into
//! a `Buffer`, and the route a `Line` *could* have taken — `impl Widget for
//! Line` — would have been a second text-to-cells path the hatch lint cannot
//! see, because it greps for the hatch's row type by name rather than for the
//! shape. (Naming that type here fails the lint, which is how this paragraph
//! learned to describe it instead. That is the lint working.)
//!
//! That is also why no [`Theme`](crate::theme::Theme) appears below. A [`Run`]
//! carries a [`Tone`], and which colour a tone is stays where the rest of the
//! frame answers it.
//!
//! # What the terminal will not draw
//!
//! `Buffer::set_stringn` — every painter in this crate bottoms out in it —
//! **drops every grapheme containing a control character**, while
//! `unicode-width` gives a lone `\r`, a NUL and an ESC one cell each. Measure
//! with one and paint with the other and a TSV loses its delimiter entirely,
//! while a field holding a stray `\r` sits one cell left of the column it
//! belongs to — which was this module's state until a reviewer painted a row
//! into a real `Buffer` and read the cells back.
//!
//! So [`cells`] is the painter's own arithmetic, and it measures the exact
//! string [`row_runs`] emits: field text goes through [`drawable`] first, so
//! nothing that will be dropped is ever counted or ever handed to a host. The
//! parser deliberately keeps a lone `\r` as data (its own header says so); the
//! screen cannot show one, and a surface that measured it anyway would misalign
//! the row without ever displaying the byte it was counting.
//!
//! Field text and the delimiter get opposite treatment on purpose. A control
//! character in a **field** is data the terminal cannot render, so it is
//! dropped, exactly as the painter would have dropped it. A control character
//! used as the **delimiter** is *structure* — `people.tsv` is a shipped fixture
//! and `phosphor_buffer::csv::Delimiter::TAB` is a public constant (not a link:
//! that crate is not in this one's graph, which is why the seam is a `&str`) —
//! and dropping it collapses `cc` and `d` into `ccd`. It is drawn as
//! a space, which is one cell, is what a tab means, and is what makes the
//! separator column exist at all.
//!
//! # The cap, and what a 100 KB field does
//!
//! Alignment is only alignment while the columns fit on a screen. One
//! pathological field — a base64 blob in row 4,000 — would otherwise set its
//! column's width to 100,000 cells, and every *other* row would then carry
//! 100,000 spaces of padding it never uses, pushing every later column past the
//! right edge. So a column is capped at [`MAX_COLUMN_CELLS`]: a field wider
//! than the cap overflows its own column and the rest of *that row* is
//! unaligned, rather than the whole file being unusable. It is also what makes
//! the cost of laying out a row bounded rather than a function of the worst
//! field in the file — `phosphor-buffer/benches/csv.rs` measures exactly that
//! and asserts the shape.
//!
//! # What this does not do, and who owes it
//!
//! It does not put the padding *inside* a buffer line. The vendored fork's
//! virtual text is a row of its own — `VisualRow::Virtual`, inserted **under**
//! the row showing its anchor (`VENDOR.md` patch 8) — and inline virtual text
//! at a column is a patch nobody has written; `crate::diagnostics`' header says
//! the same thing about `6c`'s end-of-line `■ E0308`. So the alignment here is
//! a geometry ([`Layout`]) plus a way to draw an aligned row ([`row_runs`]), and
//! composing it into the editor's own rows needs that fork patch first.
//!
//! Owned by `surface`.

use std::borrow::Cow;

use phosphor_core::view::{Run, Tone};
use ratatui_core::buffer::CellWidth;
use ratatui_core::style::Style;
use ratatui_core::text::Span;

/// The widest a column may be, in cells.
///
/// Sized against the narrowest screen `V002` calibrates (80 columns): a column
/// past this is already more than half the window, so aligning to it pushes
/// every column after it off the edge and the alignment has stopped being one.
/// See the module header for what a field wider than this does.
pub const MAX_COLUMN_CELLS: u16 = 40;

/// `text` with everything the painter would silently discard already gone.
///
/// `Buffer::set_stringn` filters out every *grapheme* containing a control
/// character; this filters *characters*, and the two agree because a control
/// character is always a cluster of its own — UAX #29 breaks before and after
/// `Control`, `CR` and `LF`, and the single exception, `CRLF`, is control on
/// both sides. See the module header for why the delimiter is not treated this
/// way.
///
/// Borrows in the case every real file is in, so the extra pass costs a scan
/// and no allocation.
fn drawable(text: &str) -> Cow<'_, str> {
    if text.contains(char::is_control) {
        Cow::Owned(text.chars().filter(|ch| !ch.is_control()).collect())
    } else {
        Cow::Borrowed(text)
    }
}

/// Cells `text` occupies once painted — the painter's own arithmetic.
///
/// **Not [`crate::interpret::cells`], and that is a defect being reported
/// rather than a copy being kept.** That function is `UnicodeWidthStr::width`
/// over the whole string, and its doc calls it *"the same measurement
/// `Buffer::set_stringn` writes with"*; `set_stringn` sums `cell_width()` per
/// **grapheme**, over graphemes it has already filtered, and the two answer
/// differently for at least three inputs a real file contains — all three run
/// in this session against `ratatui-core` 0.1.2:
///
/// | input | `Span::width` | painted |
/// |---|---|---|
/// | `a\rb` | 3 | 2 |
/// | `لأ` (U+0644 U+0623) | 1 | 2 |
/// | `ｶﾞ` (U+FF76 U+FF9E) | 1 | 2 |
///
/// The first is the control-character filter; the second is `unicode-width`
/// applying the Arabic Lam-Alef ligature to a whole string and not to one
/// grapheme; the third is ratatui's own halfwidth-dakuten compensation, which
/// lives in `CellWidth` and which `Span::width` never sees. The `لأ` case is
/// not hypothetical — `fuzz_targets/csv_parse.rs` found it in 90 seconds, from
/// five bytes.
///
/// A layout that measures with one and paints with the other misaligns the
/// column, which is the whole failure this module exists to prevent, so it
/// measures with the painter. **CONTRACT** for `interpret.rs`'s owner: give
/// `cells` this body, and this function becomes a one-line `use`.
fn cells(text: &str) -> u16 {
    Span::raw(text)
        .styled_graphemes(Style::new())
        .map(|grapheme| grapheme.symbol.cell_width())
        .fold(0u16, u16::saturating_add)
}

/// Cells `field` will occupy once drawn — measured on the exact string
/// [`row_runs`] emits, which is the only way the two can agree.
fn field_cells(field: &str) -> u16 {
    cells(&drawable(field))
}

/// What stands in the separator column for `delimiter`.
///
/// The delimiter itself whenever it draws in exactly one cell, and a space
/// otherwise — a tab, a NUL, and anything else `Delimiter::new` admits that a
/// terminal will not paint. Not the empty string: the column has to exist or
/// the fields on either side of it touch.
fn separator(delimiter: char) -> char {
    let mut encoded = [0u8; 4];
    if field_cells(delimiter.encode_utf8(&mut encoded)) == 1 {
        delimiter
    } else {
        ' '
    }
}

/// How wide each column is, in cells.
///
/// Ragged rows are the normal case, not an error: a column exists as soon as
/// one row has a field in it, and a row that stops early simply contributes
/// nothing to the columns past its end.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    widths: Vec<u16>,
}

impl Layout {
    /// Measures every field of every row.
    ///
    /// Linear and nothing quadratic: two passes over each field — one scan for
    /// a control character, one grapheme walk — and no pass over anything else.
    #[must_use]
    pub fn measure<'a, Rows, Fields>(rows: Rows) -> Self
    where
        Rows: IntoIterator<Item = Fields>,
        Fields: IntoIterator<Item = &'a str>,
    {
        let mut widths: Vec<u16> = Vec::new();
        for row in rows {
            for (column, field) in row.into_iter().enumerate() {
                let width = field_cells(field).min(MAX_COLUMN_CELLS);
                match widths.get_mut(column) {
                    Some(current) => *current = (*current).max(width),
                    None => widths.push(width),
                }
            }
        }
        Self { widths }
    }

    /// How many columns the file has — the widest row's field count.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.widths.len()
    }

    /// Column `column`'s width in cells, or 0 for a column that does not exist.
    #[must_use]
    pub fn width(&self, column: usize) -> u16 {
        self.widths.get(column).copied().unwrap_or(0)
    }

    /// The cells of padding that go after `field` to reach its column.
    ///
    /// Zero for a field at or past the cap, and zero for a column this layout
    /// never saw — both of which are "draw it as it is", which is the only
    /// answer that cannot make a row wider than the file says it is.
    #[must_use]
    pub fn padding(&self, column: usize, field: &str) -> u16 {
        self.width(column).saturating_sub(field_cells(field))
    }

    /// Cells an aligned row occupies, delimiters included.
    ///
    /// The last column takes no padding — there is nothing after it to line up
    /// — so this is not simply the sum of the widths. What a host needs it for
    /// is the horizontal extent of a row it has not drawn yet.
    #[must_use]
    pub fn row_cells<'a>(&self, fields: impl IntoIterator<Item = &'a str>) -> u16 {
        let mut fields = fields.into_iter().enumerate().peekable();
        let mut total = 0u16;
        while let Some((column, field)) = fields.next() {
            total = total.saturating_add(field_cells(field));
            // Every separator is one cell wide, including a tab's — `separator`
            // is what guarantees it.
            if fields.peek().is_some() {
                total = total
                    .saturating_add(self.padding(column, field))
                    .saturating_add(1);
            }
        }
        total
    }
}

/// One row, aligned to `layout`, as runs for the `spans` hatch.
///
/// The padding goes **before** the delimiter, so the delimiters line up as a
/// column of their own and the field after each one starts at the same cell on
/// every row. Padding after the delimiter would align the data equally well and
/// leave the commas ragged, which reads as a mistake.
///
/// No trailing padding on the last field: it would be invisible, and it would
/// put trailing whitespace into every captured frame `V007` diffs.
///
/// [`Tone::Meta`] for the delimiter is the protocol's own word for it —
/// *"separators, hints, secondary facts"* — and [`Tone::Text`] is ordinary
/// foreground for the data.
#[must_use]
pub fn row_runs<'a>(
    fields: impl IntoIterator<Item = &'a str>,
    layout: &Layout,
    delimiter: char,
) -> Vec<Run> {
    let separator = separator(delimiter);
    let mut fields = fields.into_iter().enumerate().peekable();
    let mut runs = Vec::new();
    while let Some((column, field)) = fields.next() {
        runs.push(Run::new(&drawable(field), Tone::Text));
        if fields.peek().is_none() {
            break;
        }
        let pad = usize::from(layout.padding(column, field));
        if pad > 0 {
            runs.push(Run::new(&" ".repeat(pad), Tone::Text));
        }
        runs.push(Run::new(&separator.to_string(), Tone::Meta));
    }
    runs
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Style;

    use super::*;

    /// A field of `n` single-cell characters, for the cap tests.
    fn wide(n: usize) -> String {
        "x".repeat(n)
    }

    /// The oracle: what a real `Buffer` holds after the runs are painted into
    /// it, and how far the cursor got.
    ///
    /// Deliberately not `cells`. Every test in this module used to measure the
    /// drawn row with the function that laid it out, which proves the module
    /// agrees with itself and nothing else — the reason a TSV could lose its
    /// delimiter with fifteen tests green. `set_stringn` is what the terminal
    /// will see.
    fn paint(runs: &[Run]) -> (String, u16) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 1));
        let mut x = 0u16;
        for run in runs {
            let room = 120 - usize::from(x);
            let (next, _) = buffer.set_stringn(x, 0, &run.text, room, Style::new());
            x = next;
        }
        let painted = (0..x).map(|at| buffer[(at, 0)].symbol()).collect();
        (painted, x)
    }

    /// The cell each row's field `column` begins at, on screen.
    fn field_starts(
        rows: &[Vec<&str>],
        layout: &Layout,
        delimiter: char,
        column: usize,
    ) -> Vec<u16> {
        rows.iter()
            .map(|row| {
                let runs = row_runs(row.iter().copied(), layout, delimiter);
                // Count separators rather than looking for the delimiter's
                // glyph: a field may itself be a comma.
                let mut seen = 0;
                let mut at = 0;
                for run in &runs {
                    if seen == column {
                        break;
                    }
                    let (_, width) = paint(std::slice::from_ref(run));
                    at += width;
                    if run.tone == Tone::Meta {
                        seen += 1;
                    }
                }
                at
            })
            .collect()
    }

    #[test]
    fn a_column_is_as_wide_as_its_widest_field() {
        let layout = Layout::measure([["a", "bbb"], ["cc", "d"]]);
        assert_eq!(layout.columns(), 2);
        assert_eq!(layout.width(0), 2);
        assert_eq!(layout.width(1), 3);
    }

    /// The defect the whole module exists to avoid: `名前` is two characters
    /// and four cells, so a layout counted in `chars()` under-pads it by two
    /// and every column to its right is two cells left of where it belongs.
    #[test]
    fn a_cjk_field_is_measured_in_cells_not_characters() {
        let layout = Layout::measure([["名前", "x"], ["abc", "y"]]);
        assert_eq!(layout.width(0), 4, "名前 is four cells wide");
        assert_eq!(layout.padding(0, "abc"), 1);
        assert_eq!(layout.padding(0, "名前"), 0);
    }

    #[test]
    fn an_emoji_is_two_cells() {
        let layout = Layout::measure([["🙂"], ["ab"]]);
        assert_eq!(layout.width(0), 2);
        assert_eq!(layout.padding(0, "🙂"), 0);
    }

    /// A control character occupies no cell, because the painter drops it —
    /// `unicode-width` says one, and that one cell is the misalignment.
    #[test]
    fn a_control_character_is_worth_the_cells_the_painter_gives_it() {
        assert_eq!(Layout::measure([["a\rb"]]).width(0), 2);
        assert_eq!(Layout::measure([["\t"]]).width(0), 0);
        assert_eq!(Layout::measure([["c\0d\u{1b}"]]).width(0), 2);
        // …and the painter agrees, which is the half `cells` alone cannot say.
        assert_eq!(paint(&[Run::new("a\rb", Tone::Text)]), ("ab".to_owned(), 2));
    }

    /// The three inputs where `Span::width` and the painter disagree, each
    /// asserted against a real `Buffer`. `لأ` came out of the fuzzer in ninety
    /// seconds and is why this module measures per grapheme: `unicode-width`
    /// applies the Arabic Lam-Alef ligature across the whole string and
    /// `set_stringn` never sees two graphemes as one cell.
    #[test]
    fn the_width_of_a_field_is_the_painters_and_not_unicode_widths() {
        for (field, painted) in [("a\rb", 2), ("\u{644}\u{623}", 2), ("\u{ff76}\u{ff9e}", 2)] {
            assert_eq!(
                Layout::measure([[field]]).width(0),
                painted,
                "measuring {field:?}"
            );
            assert_eq!(
                paint(&[Run::new(&drawable(field), Tone::Text)]).1,
                painted,
                "painting {field:?}"
            );
        }
    }

    /// Ragged rows are the normal case. A column exists because *some* row has
    /// a field there, and the short rows contribute nothing to it.
    #[test]
    fn ragged_rows_widen_the_columns_they_reach() {
        let layout = Layout::measure([vec!["a"], vec!["b", "cccc"], vec!["d", "e", "ff"]]);
        assert_eq!(layout.columns(), 3);
        assert_eq!(layout.width(1), 4);
        assert_eq!(layout.width(2), 2);
    }

    #[test]
    fn a_column_nobody_reached_is_zero_wide_and_pads_nothing() {
        let layout = Layout::measure([["a"]]);
        assert_eq!(layout.width(9), 0);
        assert_eq!(layout.padding(9, "anything"), 0);
    }

    #[test]
    fn an_empty_file_has_no_columns() {
        let layout = Layout::measure(Vec::<Vec<&str>>::new());
        assert_eq!(layout.columns(), 0);
        assert_eq!(layout.width(0), 0);
    }

    /// The cap is what keeps one pathological field from padding every other
    /// row by 100,000 cells.
    #[test]
    fn a_field_past_the_cap_does_not_widen_its_column_beyond_it() {
        let huge = wide(100_000);
        let layout = Layout::measure([vec![huge.as_str(), "a"], vec!["b", "c"]]);
        assert_eq!(layout.width(0), MAX_COLUMN_CELLS);
        assert_eq!(layout.padding(0, &huge), 0, "an over-cap field overflows");
        assert_eq!(layout.padding(0, "b"), MAX_COLUMN_CELLS - 1);
    }

    #[test]
    fn a_row_pads_before_the_delimiter_and_not_after_the_last_field() {
        let layout = Layout::measure([["a", "bbb"], ["cc", "d"]]);
        assert_eq!(
            paint(&row_runs(["a", "bbb"], &layout, ',')).0,
            "a ,bbb",
            "the padding goes before the delimiter"
        );
        assert_eq!(
            paint(&row_runs(["cc", "d"], &layout, ',')).0,
            "cc,d",
            "the last field takes no trailing padding"
        );
    }

    /// The point of the whole module, asserted against a real `Buffer`: every
    /// row's second field begins at the same cell, whatever is in the first.
    ///
    /// The fixture holds the three things that break it — a CJK field, an
    /// emoji, and a lone `\r` the parser preserves as data and the terminal
    /// will not draw.
    #[test]
    fn every_rows_second_field_starts_at_the_same_cell() {
        let rows = vec![
            vec!["id", "名前", "n"],
            vec!["1", "alice", "2"],
            vec!["1000", "🙂", "3"],
            vec!["a\rb", "carol", "4"],
        ];
        let layout = Layout::measure(rows.clone());
        assert_eq!(
            field_starts(&rows, &layout, ',', 1),
            vec![layout.width(0) + 1; 4],
            "the columns do not line up"
        );
    }

    /// The same file with tabs. `Delimiter::TAB` is a public constant and
    /// `people.tsv` is a shipped fixture, so this dialect is not hypothetical —
    /// and a tab paints nothing, which is why the separator column is a space.
    #[test]
    fn a_tab_delimited_row_aligns_the_same_way() {
        let rows = vec![vec!["a", "bbb"], vec!["cc", "d"]];
        let layout = Layout::measure(rows.clone());
        assert_eq!(paint(&row_runs(["a", "bbb"], &layout, '\t')).0, "a  bbb");
        assert_eq!(
            paint(&row_runs(["cc", "d"], &layout, '\t')).0,
            "cc d",
            "without a separator column this row would read `ccd`"
        );
        assert_eq!(field_starts(&rows, &layout, '\t', 1), vec![3, 3]);
    }

    #[test]
    fn a_one_field_row_is_the_field() {
        let layout = Layout::measure([["only"]]);
        assert_eq!(paint(&row_runs(["only"], &layout, ',')).0, "only");
    }

    #[test]
    fn an_empty_row_draws_nothing() {
        let layout = Layout::measure([[""]]);
        assert_eq!(paint(&row_runs([""], &layout, ',')).0, "");
    }

    /// What a host is told a row is worth, against what a terminal does with
    /// it. The fixture is chosen so the two can disagree: a wide field, a
    /// control character, and a tab delimiter each break a different half.
    #[test]
    fn row_cells_is_what_the_row_actually_draws() {
        for (rows, delimiter) in [
            (vec![vec!["a", "名前"], vec!["cccc", "d"]], ','),
            (vec![vec!["a\rb", "x"], vec!["cc", "y"]], ','),
            (vec![vec!["a", "bbb"], vec!["cc", "d"]], '\t'),
        ] {
            let layout = Layout::measure(rows.clone());
            for row in &rows {
                let (_, drawn) = paint(&row_runs(row.iter().copied(), &layout, delimiter));
                assert_eq!(
                    layout.row_cells(row.iter().copied()),
                    drawn,
                    "row {row:?} under {delimiter:?}"
                );
            }
        }
    }

    /// Every field is drawn, in order, with its bytes intact — alignment adds
    /// cells and removes none. A row whose text the surface silently truncated
    /// would be a renderer that lies about the file.
    ///
    /// Two rows, not one: a single-row layout makes every column exactly as
    /// wide as its own field, so no padding run is ever emitted and the half of
    /// this test that says *padding* cannot fail.
    #[test]
    fn alignment_adds_padding_and_changes_no_field() {
        let fields = ["a,b", "\"q\"", "", "  ", "名前"];
        let layout = Layout::measure([fields, ["wider than a,b", "q", "", "", "x"]]);
        let runs = row_runs(fields, &layout, ',');
        let drawn: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(
            drawn,
            vec![
                "a,b",
                "           ",
                ",",
                "\"q\"",
                ",",
                "",
                ",",
                "  ",
                ",",
                "名前"
            ],
            "every field survives, including the empty one and the \
             whitespace-only one, and the padding is a run of its own"
        );
    }
}
