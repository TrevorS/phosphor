//! **What opening a large CSV costs** — `T082`'s parse and its column model.
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-buffer`). It prints
//! four tables and a verdict, and asserts the structural half of what it
//! prints.
//!
//! # Why a number here changes something
//!
//! CSV is the one first-class language with no grammar, so nothing incremental
//! stands between a keystroke and a re-parse: tree-sitter's edit-and-reparse is
//! exactly what this build gave up for the other eleven languages, and what
//! replaces it here is *parsing the file again*. Two questions follow, and only
//! measurement answers them.
//!
//! 1. **Is the parse linear in the file?** It has to be — a 50 MB export is a
//!    normal thing to open — and the shape is not obvious from reading it,
//!    because a quoted field with `""` in it leaves the borrow path and
//!    allocates.
//! 2. **Is the layout quadratic in the widest column?** This is the one that
//!    bites. A column is as wide as its widest field, and *every other row*
//!    pays that width in padding. One base64 blob in row 4,000 of a 10,000-row
//!    file would otherwise cost 10,000 × 100,000 cells of padding to lay out —
//!    quadratic in a number the file chose, on a path the draw loop runs.
//!    `phosphor_ui::csv::MAX_COLUMN_CELLS` is the answer, and table 3 is the
//!    proof that it is.
//!
//! # The five tables
//!
//! 0. **sniff** — the *first* call the open sequence makes, and for a window
//!    the most expensive: it ran four whole-file parses and kept the header.
//!    It is a table of its own because a cost that is not measured is a cost
//!    the doc comment gets to be wrong about, and this one was — the headline
//!    below excluded `sniff` entirely while `sniff` was 6.7× the parse it
//!    precedes.
//! 1. **size ladder** — one parse against a 16× climb in rows. Flat
//!    nanoseconds-per-byte is O(n).
//! 2. **row shape** — the same byte count arranged three ways: many narrow
//!    rows, few wide rows, and one enormous row. Cost that tracks bytes rather
//!    than row shape is the claim.
//! 3. **the widest column** — a fixed 2,000-row file with **one** field whose
//!    width climbs 8 → 1,048,576 cells. The file grows by that field and nothing
//!    else, so a layout that is quadratic in the widest column shows up as cost
//!    climbing with it and a capped one does not.
//! 4. **quoted against plain** — the same fields, escaped and not. `""` is the
//!    one thing that takes a field off the borrow path, and this is what the
//!    allocation costs.
//!
//! # What these numbers are not
//!
//! Wall clock on one machine. The *shapes* are asserted — linear versus
//! quadratic — because those are machine-independent; the absolute
//! milliseconds are information for a person deciding whether `T082` needs a
//! second pass. Same rule as `phosphor-ui/benches/soft_wrap.rs`, and the reason
//! `just bench` is deliberately not part of `just gate`.
//!
//! Owned by `harness`.

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use phosphor_buffer::csv::{Delimiter, Row, parse, sniff};
use phosphor_ui::csv::{Layout, MAX_COLUMN_CELLS, row_runs};

/// One frame at 60fps, in milliseconds — the budget a draw-path cost is read
/// against, and the same one `soft_wrap.rs` uses.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// Rows per rung of the size ladder. A 16× climb, which is what makes O(n) and
/// O(n²) different answers rather than different noise.
const LADDER: [usize; 5] = [4_096, 8_192, 16_384, 32_768, 65_536];

/// Columns in the ladder's fixture — a plausible export.
const LADDER_COLUMNS: usize = 8;

/// Bytes in every arm of the row-shape table. One number, three arrangements.
const SHAPE_BYTES: usize = 4_000_000;

/// Rows in the widest-column table. Fixed, so the only thing that moves is the
/// one wide field.
const WIDE_ROWS: usize = 2_000;

/// The widths that one field takes, in cells. `MAX_COLUMN_CELLS` is in the
/// middle on purpose: the cap is the point where the shape has to change.
const WIDTHS: [usize; 5] = [8, MAX_COLUMN_CELLS as usize, 4_096, 65_536, 1_048_576];

fn main() {
    println!(
        "phosphor · T082 CSV — what opening a large delimited file costs, and whether the column \
         model is quadratic in the widest field"
    );
    println!(
        "  frame budget  {FRAME_BUDGET_MS:.1} ms at 60fps · column cap {MAX_COLUMN_CELLS} cells"
    );
    println!();

    let ladder: Vec<Rung> = LADDER.iter().map(|rows| rung(*rows)).collect();
    let shapes = [
        shaped("many narrow rows", SHAPE_BYTES / 40, 4),
        shaped("few wide rows", SHAPE_BYTES / 400, 40),
        shaped("one enormous row", 1, SHAPE_BYTES / 10),
    ];
    let widths: Vec<Widest> = WIDTHS.iter().map(|cells| widest(*cells)).collect();
    let quoting = [quoted("plain", false), quoted("quoted, escaped", true)];

    sniff_table(&ladder);
    ladder_table(&ladder);
    shape_table(&shapes);
    widest_table(&widths);
    quoting_table(&quoting);
    verdict(&ladder, &shapes, &widths, &quoting);
}

// ---------------------------------------------------------------------------
// The measurements
// ---------------------------------------------------------------------------

/// One rung of the size ladder.
#[derive(Debug, Clone, Copy)]
struct Rung {
    rows: usize,
    bytes: usize,
    sniff: Duration,
    parse: Duration,
    layout: Duration,
}

impl Rung {
    fn parse_nanos_per_byte(&self) -> f64 {
        self.parse.as_secs_f64() * 1e9 / self.bytes as f64
    }

    fn layout_nanos_per_byte(&self) -> f64 {
        self.layout.as_secs_f64() * 1e9 / self.bytes as f64
    }

    fn sniff_micros(&self) -> f64 {
        self.sniff.as_secs_f64() * 1e6
    }

    /// What the documented open sequence costs, end to end: `sniff`, then
    /// `parse`, then one `Layout::measure`.
    fn millis(&self) -> f64 {
        (self.sniff + self.parse + self.layout).as_secs_f64() * 1e3
    }
}

fn rung(rows: usize) -> Rung {
    let source = grid(rows, LADDER_COLUMNS, 9);
    let sniff_cost = time(|| sniff(&source));
    let parse_cost = time(|| {
        let table = parse(&source, Delimiter::COMMA);
        table.rows().len()
    });
    let table = parse(&source, Delimiter::COMMA);
    let layout_cost = time(|| {
        let layout = Layout::measure(table.rows().iter().map(Row::values));
        layout.columns()
    });
    Rung {
        rows,
        bytes: source.len(),
        sniff: sniff_cost,
        parse: parse_cost,
        layout: layout_cost,
    }
}

/// One arrangement of a fixed number of bytes.
#[derive(Debug, Clone, Copy)]
struct Shape {
    name: &'static str,
    rows: usize,
    columns: usize,
    bytes: usize,
    parse: Duration,
}

impl Shape {
    fn nanos_per_byte(&self) -> f64 {
        self.parse.as_secs_f64() * 1e9 / self.bytes as f64
    }
}

fn shaped(name: &'static str, rows: usize, columns: usize) -> Shape {
    let source = grid(rows, columns, 9);
    let parse_cost = time(|| parse(&source, Delimiter::COMMA).rows().len());
    Shape {
        name,
        rows,
        columns,
        bytes: source.len(),
        parse: parse_cost,
    }
}

/// One width of the one wide field, with everything else held fixed.
#[derive(Debug, Clone, Copy)]
struct Widest {
    cells: usize,
    bytes: usize,
    layout: Duration,
    draw: Duration,
}

impl Widest {
    /// The number that matters: what drawing **one ordinary row** costs when
    /// some *other* row in the file is this wide. Uncapped, this climbs with
    /// `cells`, because every ordinary row carries the wide field's width in
    /// padding; capped, it does not.
    ///
    /// The wide row itself is deliberately not in this figure. Drawing a
    /// 1 MB field costs 1 MB whatever the cap does, and averaging that over
    /// 2,000 rows would put a linear term into a measurement whose whole
    /// purpose is to isolate the quadratic one.
    fn nanos_per_row(&self) -> f64 {
        self.draw.as_secs_f64() * 1e9 / WIDE_ROWS as f64
    }
}

fn widest(cells: usize) -> Widest {
    let source = grid_with_one_wide_field(WIDE_ROWS, cells);
    let table = parse(&source, Delimiter::COMMA);
    let layout_cost = time(|| {
        let layout = Layout::measure(table.rows().iter().map(Row::values));
        layout.columns()
    });
    let layout = Layout::measure(table.rows().iter().map(Row::values));
    // The ordinary rows only — `grid_with_one_wide_field` appends the wide one
    // last, and it is excluded for the reason `nanos_per_row` gives.
    let ordinary = &table.rows()[..WIDE_ROWS];
    let draw_cost = time(|| {
        let mut drawn = 0;
        for row in ordinary {
            drawn += row_runs(row.values(), &layout, ',').len();
        }
        drawn
    });
    Widest {
        cells,
        bytes: source.len(),
        layout: layout_cost,
        draw: draw_cost,
    }
}

/// The borrow path against the allocating one.
#[derive(Debug, Clone, Copy)]
struct Quoting {
    name: &'static str,
    bytes: usize,
    parse: Duration,
}

impl Quoting {
    fn nanos_per_byte(&self) -> f64 {
        self.parse.as_secs_f64() * 1e9 / self.bytes as f64
    }
}

fn quoted(name: &'static str, escape: bool) -> Quoting {
    let mut source = String::new();
    for row in 0..LADDER[0] {
        for column in 0..LADDER_COLUMNS {
            if column > 0 {
                source.push(',');
            }
            if escape {
                // `""` is the byte pair that takes a field off the borrow path.
                let _ = write!(source, "\"f{row}-{column}\"\"x\"");
            } else {
                let _ = write!(source, "f{row}-{column}xxx");
            }
        }
        source.push('\n');
    }
    let parse_cost = time(|| parse(&source, Delimiter::COMMA).rows().len());
    Quoting {
        name,
        bytes: source.len(),
        parse: parse_cost,
    }
}

/// Times `work` until it has run long enough to be legible, and returns the
/// cost of one run.
///
/// A single `Instant::now()` pair around a 20 ms parse is fine; around a 2 µs
/// one it measures the clock. The loop is what makes the same helper usable
/// for both, and the returned `Duration` is per iteration either way.
fn time<T>(mut work: impl FnMut() -> T) -> Duration {
    // One warm-up run, so the first iteration's page faults are not the
    // measurement.
    let _ = work();
    let mut runs = 0u32;
    let started = Instant::now();
    loop {
        let _ = work();
        runs += 1;
        let elapsed = started.elapsed();
        if elapsed >= Duration::from_millis(50) || runs >= 1_000 {
            return elapsed / runs;
        }
    }
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// A plain grid — `rows` × `columns` fields of about `width` bytes each.
fn grid(rows: usize, columns: usize, width: usize) -> String {
    let mut source = String::with_capacity(rows * columns * (width + 1));
    for row in 0..rows {
        for column in 0..columns {
            if column > 0 {
                source.push(',');
            }
            let cell = format!("r{row}c{column}");
            source.push_str(&cell);
            for _ in cell.len()..width {
                source.push('x');
            }
        }
        source.push('\n');
    }
    source
}

/// `rows` narrow rows, and one row whose second field is `cells` wide.
///
/// The file grows by exactly that field, so anything that climbs with `cells`
/// faster than the file does is the layout paying for it on every *other* row.
fn grid_with_one_wide_field(rows: usize, cells: usize) -> String {
    let mut source = grid(rows, 3, 6);
    let _ = writeln!(source, "tail,{},end", "w".repeat(cells));
    source
}

// ---------------------------------------------------------------------------
// The tables
// ---------------------------------------------------------------------------

fn sniff_table(ladder: &[Rung]) {
    println!(
        "0 · sniff — the first call on open, against the same 16x climb. It reads the first record \
         and stops, so this column is flat"
    );
    println!(
        "    {:>8}  {:>10}  {:>10}  {:>14}",
        "rows", "bytes", "sniff us", "% of the parse"
    );
    for rung in ladder {
        println!(
            "    {:>8}  {:>10}  {:>10.2}  {:>13.1}%",
            rung.rows,
            rung.bytes,
            rung.sniff_micros(),
            100.0 * rung.sniff.as_secs_f64() / rung.parse.as_secs_f64(),
        );
    }
    println!();
}

fn ladder_table(ladder: &[Rung]) {
    println!("1 · size ladder — one parse and one layout, against a 16x climb in rows");
    println!(
        "    {:>8}  {:>10}  {:>9}  {:>9}  {:>9}  {:>9}",
        "rows", "bytes", "parse ms", "ns/byte", "layout ms", "ns/byte"
    );
    for rung in ladder {
        println!(
            "    {:>8}  {:>10}  {:>9.2}  {:>9.2}  {:>9.2}  {:>9.2}",
            rung.rows,
            rung.bytes,
            rung.parse.as_secs_f64() * 1e3,
            rung.parse_nanos_per_byte(),
            rung.layout.as_secs_f64() * 1e3,
            rung.layout_nanos_per_byte(),
        );
    }
    println!();
}

fn shape_table(shapes: &[Shape; 3]) {
    println!("2 · row shape — the same {SHAPE_BYTES} bytes, arranged three ways");
    println!(
        "    {:>18}  {:>9}  {:>8}  {:>10}  {:>9}  {:>9}",
        "arrangement", "rows", "columns", "bytes", "parse ms", "ns/byte"
    );
    for shape in shapes {
        println!(
            "    {:>18}  {:>9}  {:>8}  {:>10}  {:>9.2}  {:>9.2}",
            shape.name,
            shape.rows,
            shape.columns,
            shape.bytes,
            shape.parse.as_secs_f64() * 1e3,
            shape.nanos_per_byte(),
        );
    }
    println!();
}

fn widest_table(widths: &[Widest]) {
    println!(
        "3 · the widest column — {WIDE_ROWS} ordinary rows plus one wide field, laid out and drawn"
    );
    println!(
        "    {:>9}  {:>10}  {:>10}  {:>9}  {:>12}",
        "cells", "bytes", "layout ms", "draw ms", "ns/row"
    );
    for width in widths {
        println!(
            "    {:>9}  {:>10}  {:>10.2}  {:>9.2}  {:>12.0}",
            width.cells,
            width.bytes,
            width.layout.as_secs_f64() * 1e3,
            width.draw.as_secs_f64() * 1e3,
            width.nanos_per_row(),
        );
    }
    println!();
}

fn quoting_table(quoting: &[Quoting; 2]) {
    println!("4 · quoted against plain — what leaving the borrow path costs");
    println!(
        "    {:>16}  {:>10}  {:>9}  {:>9}",
        "fields", "bytes", "parse ms", "ns/byte"
    );
    for arm in quoting {
        println!(
            "    {:>16}  {:>10}  {:>9.2}  {:>9.2}",
            arm.name,
            arm.bytes,
            arm.parse.as_secs_f64() * 1e3,
            arm.nanos_per_byte(),
        );
    }
    println!();
}

/// The largest ratio in a set of measurements — 1.0 is perfectly flat.
fn spread(values: &[f64]) -> f64 {
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(0.0, f64::max);
    high / low.max(f64::MIN_POSITIVE)
}

fn verdict(ladder: &[Rung], shapes: &[Shape; 3], widths: &[Widest], quoting: &[Quoting; 2]) {
    let parse_spread = spread(
        &ladder
            .iter()
            .map(Rung::parse_nanos_per_byte)
            .collect::<Vec<_>>(),
    );
    let layout_spread = spread(
        &ladder
            .iter()
            .map(Rung::layout_nanos_per_byte)
            .collect::<Vec<_>>(),
    );
    let shape_spread = spread(&shapes.iter().map(Shape::nanos_per_byte).collect::<Vec<_>>());
    // `sniff` against the parse it precedes, on the largest file. Not a spread
    // over the ladder: `sniff` is a sub-microsecond figure and the smallest
    // rung's warm-up moves that ratio between 2x and 4x run to run, which is
    // a threshold measuring the machine. This ratio is four orders of magnitude
    // from its bound and cannot be noise.
    let sniff_share = 100.0 * ladder[ladder.len() - 1].sniff.as_secs_f64()
        / ladder[ladder.len() - 1].parse.as_secs_f64();
    let quote_ratio = quoting[1].nanos_per_byte() / quoting[0].nanos_per_byte();

    // Table 3's shape, stated against the cap rather than against the whole
    // ladder: below `MAX_COLUMN_CELLS` a wider field legitimately costs more,
    // because the column really is wider. Above it, nothing may.
    let capped: Vec<f64> = widths
        .iter()
        .filter(|width| width.cells >= MAX_COLUMN_CELLS as usize)
        .map(Widest::nanos_per_row)
        .collect();
    let capped_spread = spread(&capped);

    // The ladder climbs 16x. O(n) leaves ns/byte flat and O(n²) puts ~16x into
    // it; 8 is the midpoint on a log scale and the widest a machine's noise has
    // any business being.
    let parse_is_linear = parse_spread < 8.0;
    // The claim the open sequence rests on: `sniff` then `parse` is one file
    // read, not five. 1% is where a prefix scan and a whole-file pass are
    // separated by everything and no threshold argument is needed — measured
    // **0.003%** here and **288%** with the four-whole-file-parses
    // implementation restored.
    let last = &ladder[ladder.len() - 1];
    let sniff_is_a_prefix_not_a_pass = sniff_share < 1.0;
    let layout_is_linear = layout_spread < 8.0;
    let shape_is_indifferent = shape_spread < 10.0;
    // The wide field climbs 26,214x past the cap and the 2,000 ordinary rows
    // must not notice. Measured on this machine: **1.0x** with the cap, and
    // **6.1x** with `.min(MAX_COLUMN_CELLS)` deleted from `Layout::measure` —
    // so the threshold sits between two numbers that were run, not guessed.
    //
    // 6.1 rather than 26,214 because of a *second* ceiling, not because of
    // memory bandwidth as this comment claimed for a window: `interpret::cells`
    // is `u16::try_from(width).unwrap_or(u16::MAX)`, so an uncapped column
    // stops widening at 65,535 and the top two rungs of `WIDTHS` — 65,536 and
    // 1,048,576 — lay out the identical padding. The uncapped ratio is
    // therefore 65,535/40 in principle and whatever `" ".repeat` of that costs
    // in practice. The shape is still unmistakable, which is all the assertion
    // needs.
    let layout_is_capped = capped_spread < 3.0;

    println!("verdict");
    println!(
        "  sniff         {:.2} us at {} bytes, {:.2} at {} ({sniff_share:.3}% of that file's parse \
         — {})",
        ladder[0].sniff_micros(),
        ladder[0].bytes,
        last.sniff_micros(),
        last.bytes,
        if sniff_is_a_prefix_not_a_pass {
            "FIRST RECORD ONLY"
        } else {
            "READING THE WHOLE FILE"
        },
    );
    println!(
        "  parse         {:.2} ns/byte at {} bytes, {:.2} at {} ({parse_spread:.1}x over a 16x \
         climb — {})",
        ladder[0].parse_nanos_per_byte(),
        ladder[0].bytes,
        ladder[ladder.len() - 1].parse_nanos_per_byte(),
        ladder[ladder.len() - 1].bytes,
        if parse_is_linear { "LINEAR" } else { "WORSE" },
    );
    println!(
        "  layout        {layout_spread:.1}x over the same climb — {}",
        if layout_is_linear { "LINEAR" } else { "WORSE" },
    );
    println!(
        "  row shape     {shape_spread:.1}x between {} narrow rows and one row of {} bytes — {}",
        shapes[0].rows,
        shapes[2].bytes,
        if shape_is_indifferent {
            "the cost follows bytes, not row shape"
        } else {
            "a long row costs more than its bytes"
        },
    );
    println!(
        "  widest column {capped_spread:.1}x per row between a {}-cell field and a {}-cell one \
         — {}",
        MAX_COLUMN_CELLS,
        widths[widths.len() - 1].cells,
        if layout_is_capped {
            "FLAT past the cap, which is what MAX_COLUMN_CELLS is for"
        } else {
            "QUADRATIC in the widest field — the cap is not holding"
        },
    );
    println!(
        "  quoting       {quote_ratio:.2}x — what `\"\"` costs by taking a field off the borrow \
         path"
    );
    println!();
    println!("  the number to act on:");
    println!(
        "    a {}-byte file sniffs, parses and lays out in {:.1} ms — {:.1} frames.",
        last.bytes,
        last.millis(),
        last.millis() / FRAME_BUDGET_MS,
    );
    println!(
        "    All three are once-per-open costs today, not once-per-frame: nothing here is on the \
         draw"
    );
    println!(
        "    path until a host caches a Layout per buffer and re-measures on edit. When one does,"
    );
    println!(
        "    this is the number that says how big a file may be before an edit tears a frame."
    );
    println!();
    println!(
        "  T082: {}",
        if parse_is_linear
            && layout_is_linear
            && shape_is_indifferent
            && layout_is_capped
            && sniff_is_a_prefix_not_a_pass
        {
            "PASS — sniff reads one record, the parse is linear in bytes and indifferent to row \
             shape, and the column model is flat past the cap"
        } else {
            "FAIL — see the tables above"
        }
    );

    assert!(
        sniff_is_a_prefix_not_a_pass,
        "sniff cost {:.2} us against the {:.2} us parse it precedes — {sniff_share:.1}% of it. It \
         is documented as reading the first record and stopping; a cost of this order is a \
         whole-file pass per candidate wearing that sentence, and the host pays it before the \
         first byte is drawn",
        last.sniff_micros(),
        last.parse.as_secs_f64() * 1e6,
    );
    assert!(
        parse_is_linear,
        "the parse cost per byte climbed {parse_spread:.1}x over a 16x growth in file size; every \
         byte is visited once, so this is superlinear where it should not be"
    );
    assert!(
        layout_is_linear,
        "the layout cost per byte climbed {layout_spread:.1}x over a 16x growth; `Layout::measure` \
         is one width call per field and should not notice"
    );
    assert!(
        shape_is_indifferent,
        "{shape_spread:.1}x between arrangements of the same {SHAPE_BYTES} bytes — a wide row \
         costs more than its bytes do, which means a scan is restarting somewhere"
    );
    assert!(
        layout_is_capped,
        "laying out an ordinary row cost {capped_spread:.1}x more with a {}-cell field elsewhere \
         in the file than with a {}-cell one. That is the quadratic MAX_COLUMN_CELLS exists to \
         prevent: every row paying the widest field's width in padding",
        widths[widths.len() - 1].cells,
        MAX_COLUMN_CELLS,
    );
}
