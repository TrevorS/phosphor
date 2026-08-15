//! `T082` — the hand-written CSV parser, over bytes nobody wrote.
//!
//! # Why this target exists
//!
//! Because the parser is ours. The other eleven first-class languages get a
//! tree-sitter grammar that thousands of other people fuzz; CSV deliberately
//! does not (`runtime/languages/csv.scm`, `T082`), and the whole argument for
//! writing one by hand — *a small parser we own beats a stale dependency* —
//! only holds while somebody is actually attacking it. A hand-rolled state
//! machine over `&str` with three lenient readings in it is exactly the shape
//! that has an off-by-one waiting in it.
//!
//! And it is on the open path: an editor parses whatever file you point it at,
//! including the 40 MB export with a truncated quoted field at the end. A panic
//! here is an editor that will not open a file.
//!
//! # Input
//!
//! `[delimiter: u8][the file]`. The leading byte is the delimiter — an invalid
//! one ([`Delimiter::new`] refuses `"`, `\r`, `\n` and everything past ASCII)
//! falls back to a comma, so no input is wasted, and a fuzzer flipping that one
//! byte re-reads the whole file under a different grammar. The rest is decoded
//! **lossily**: `parse` takes a `&str`, and rejecting non-UTF-8 outright would
//! throw away most of the corpus rather than exercising the parser with the
//! replacement characters a real editor would show.
//!
//! `seeds/csv_parse/` holds `crates/phosphor-buffer/tests/fixtures/csv/` in
//! this framing — the RFC's own example, an embedded newline, a ragged file, a
//! CJK one, a TSV, the malformed one, and both degenerate files. That is the
//! point of a seeded corpus: a fuzzer starting from nothing spends its first
//! hour rediscovering that `""` means a quote.
//!
//! # The laws
//!
//! 1. **Totality.** Every byte string is a table. No panic, no slice out of
//!    bounds, no `char` boundary violation — the last one being the specific
//!    hazard of a parser that scans `as_bytes()` and slices `&str`.
//! 2. **The writer is the parser's inverse.** `to_csv` then `parse` gives the
//!    same values back, *including* for input that was malformed: whatever
//!    lenient reading a broken file got, saving it must not change what it
//!    means. `tests/csv_properties.rs` states this over generated tables; here
//!    it runs against inputs no generator would produce.
//! 3. **The written form is a fixed point** — so a file does not drift a byte
//!    per save.
//! 4. **Spans locate what they claim to.** In bounds, forward, non-overlapping,
//!    and the slice a span names re-parses to that field's value. The spans are
//!    what an aligning edit indexes with, so one that is off by a byte is a
//!    padding run inserted inside a quoted field.
//! 5. **The row measures what the terminal draws.** `Layout::row_cells` says
//!    how wide an aligned row is; painting `row_runs` into a real `Buffer` says
//!    how wide it *is*. The two disagreeing is not hypothetical — it is the
//!    defect this module shipped, because `Buffer::set_stringn` drops every
//!    grapheme holding a control character and `unicode-width` does not, so a
//!    TSV lost its delimiter and a field holding a lone `\r` (which the parser
//!    deliberately keeps as data) sat one cell left of its column.
//!
//!    Stated this way on purpose: the assertion here used to be `padding <=
//!    width`, which is `width.saturating_sub(cells)`'s own postcondition and
//!    could not fail for any input at all.

#![no_main]

use libfuzzer_sys::fuzz_target;
use phosphor_buffer::csv::{Delimiter, Row, Table, parse};
use phosphor_core::view::{Run, Tone};
use phosphor_ui::csv::{Layout, MAX_COLUMN_CELLS, row_runs};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;

/// The widest row this target will paint, in cells.
///
/// A fuzz input is small, but `Layout` caps a *column*, not a row: 5,000 fields
/// of 40 cells is a legal row. Painting is the oracle, not the thing under
/// test, so a row past this is skipped rather than allocating a buffer for it.
const PAINTABLE_CELLS: u16 = 4_096;

/// The values of a table, owned, for comparing two parses.
fn values(table: &Table<'_>) -> Vec<Vec<String>> {
    table
        .rows()
        .iter()
        .map(|row| row.values().map(str::to_owned).collect())
        .collect()
}

/// The cell the cursor sits at after each of `runs` is painted.
///
/// The same `set_stringn` every painter in `phosphor-ui` bottoms out in —
/// `interpret::write` is this, clipped to a `Rect`. Deliberately not
/// `phosphor_ui::csv`'s own measurement: a law checked with the function under
/// test is a law about self-consistency.
///
/// One buffer per row rather than one per run: the alignment law needs where
/// every run ended, and allocating 4,096 cells per run costs the fuzzer two
/// orders of magnitude in throughput.
fn painted_stops(runs: &[Run], width: u16) -> Vec<u16> {
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, 1));
    let mut x = 0u16;
    let mut stops = Vec::with_capacity(runs.len());
    for run in runs {
        let room = usize::from(width - x);
        let (next, _) = buffer.set_stringn(x, 0, &run.text, room, Style::new());
        x = next;
        stops.push(x);
    }
    stops
}

fuzz_target!(|data: &[u8]| {
    let (delimiter, rest) = match data.split_first() {
        None => (Delimiter::COMMA, &data[..]),
        Some((byte, rest)) => (Delimiter::new(*byte).unwrap_or(Delimiter::COMMA), rest),
    };
    let source = String::from_utf8_lossy(rest);

    // Law 1 — totality. Reaching the next line is the assertion.
    let table = parse(&source, delimiter);

    // Law 4 — spans locate what they claim to.
    let mut previous_end = 0;
    for row in table.rows() {
        assert!(!row.fields().is_empty(), "a row with no fields");
        assert!(row.span().end <= source.len(), "a row span past the file");
        for field in row.fields() {
            let span = field.span();
            assert!(span.start <= span.end, "an inverted field span");
            assert!(span.start >= previous_end, "field spans run backwards");
            assert!(span.end <= source.len(), "a field span past the file");
            let re_read = parse(&source[span.clone()], delimiter);
            assert_eq!(
                re_read
                    .rows()
                    .first()
                    .and_then(|row| row.fields().first())
                    .map_or("", |field| field.value()),
                field.value(),
                "the slice a span names is not the field it came from"
            );
            previous_end = span.end;
        }
    }

    // Laws 2 and 3 — the round trip, and its fixed point.
    let spelled = table.to_csv(delimiter);
    let again = parse(&spelled, delimiter);
    assert_eq!(
        values(&again),
        values(&table),
        "writing this file out and reading it back changed what it says"
    );
    assert_eq!(
        again.to_csv(delimiter),
        spelled,
        "the written form is not a fixed point — a file would drift on every save"
    );

    // Law 5 — the column model.
    let layout = Layout::measure(table.rows().iter().map(Row::values));
    assert_eq!(
        layout.columns(),
        table.columns(),
        "the layout and the table disagree about how many columns there are"
    );
    for column in 0..layout.columns() {
        assert!(
            layout.width(column) <= MAX_COLUMN_CELLS,
            "a column wider than the cap"
        );
    }
    // Where the geometry says each column begins: every column before it, plus
    // one cell of separator each. Only meaningful while no column hit the cap —
    // an over-cap field overflows its own column and shifts the rest of *that*
    // row right, which the module documents and which is not misalignment.
    let uncapped = (0..layout.columns()).all(|column| layout.width(column) < MAX_COLUMN_CELLS);
    let mut expected = vec![0u16];
    for column in 0..layout.columns() {
        let previous = expected[column];
        expected.push(previous.saturating_add(layout.width(column)).saturating_add(1));
    }

    // The row as measured against the row as painted. A control character and a
    // delimiter the terminal will not draw each break one half and not the
    // other.
    for row in table.rows() {
        let claimed = layout.row_cells(row.values());
        if claimed > PAINTABLE_CELLS {
            continue;
        }
        let runs = row_runs(row.values(), &layout, delimiter.as_char());
        let stops = painted_stops(&runs, PAINTABLE_CELLS);
        assert_eq!(
            stops.last().copied().unwrap_or(0),
            claimed,
            "row_cells says {claimed} and the terminal disagrees"
        );
        if !uncapped {
            continue;
        }
        // …and where each field lands. This is what alignment *is*: the cell a
        // column begins at, the same on every row that reaches it.
        let mut column = 0usize;
        for (run, at) in runs.iter().zip(&stops) {
            if run.tone == Tone::Meta {
                column += 1;
                assert_eq!(
                    *at, expected[column],
                    "column {column} begins at cell {at} on this row and at {} on the layout",
                    expected[column]
                );
            }
        }
    }
});
