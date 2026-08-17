#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]
//! **What drawing the picker costs**, and whether it is a function of the
//! window rather than of the corpus (`T045`).
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-ui`). Three tables and
//! a verdict; the assertions are *shapes*, never times, for the reason
//! `CLAUDE.md` gives.
//!
//! # Why a number here changes something
//!
//! `T045`'s criterion is *"it stays responsive filtering a 100k-file list"*,
//! and it splits across two crates. The **matcher** half lives in the binary
//! and is bounded by a test there
//! (`a_hundred_thousand_rows_never_block_a_frame`) because nucleo owns threads
//! and the claim is a bound. This is the other half, and it is the one an
//! assertion can get wrong in a way nobody notices:
//!
//! > **the widget must cost the window, not the corpus.**
//!
//! The host hands `PickerVm` only the rows that fit, so drawing 100k matches
//! should cost the same as drawing 20 — and the way that stops being true is a
//! widget that iterates the whole list to compute a layout. That is a
//! *quadratic* the matcher's own bound cannot see, because it is on the other
//! side of the seam. The first table is exactly that comparison.
//!
//! # What is deliberately not measured
//!
//! Anything about matching. It is not this crate's and it has its own bound.

use std::time::{Duration, Instant};

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

use phosphor_core::view::Tone;
use phosphor_ui::picker::{PREVIEW_AT, Picker, PickerVm, RowVm, RunVm};
use phosphor_ui::theme::builtin;

/// One frame at 60fps. Design Language §8 makes a torn frame a P0.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// Repeats per measurement, best-of, so one scheduler slice is not the number.
const REPEATS: usize = 20;

fn main() {
    println!();
    println!("phosphor · the picker widget — what a frame of it costs");
    println!("  frame budget  {FRAME_BUDGET_MS:.1} ms (60fps)");
    println!("  repeats       {REPEATS}, best of");
    println!();

    let by_window = cost_by_window();
    let by_corpus = cost_by_corpus();
    let ladder = cost_by_width();

    verdict(&by_window, &by_corpus, &ladder);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A row shaped like `3d`'s files picker: a path, a separator, and an actor.
fn row(index: usize) -> RowVm {
    RowVm::new(vec![
        RunVm::text(format!("crates/phosphor-ui/src/module_{index}.rs")),
        RunVm::text("  ·  "),
        RunVm::text("claude").toned(Tone::Claude),
        RunVm::text(" 4m ago").toned(Tone::Meta),
    ])
}

/// A ViewModel holding `window` rows out of a corpus of `total`.
fn vm(window: usize, total: usize) -> PickerVm {
    PickerVm {
        rows: (0..window).map(row).collect(),
        selected: window / 2,
        preview: (0..12).map(|n| format!("    preview line {n}")).collect(),
        total,
        matching: false,
    }
}

fn best(mut body: impl FnMut()) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..REPEATS {
        let start = Instant::now();
        body();
        best = best.min(start.elapsed());
    }
    best
}

/// Draw one frame of the picker into a fresh buffer.
fn draw(vm: &PickerVm, width: u16, height: u16, preview: bool) -> Duration {
    let theme = builtin("phosphor-dark").expect("the shipped theme");
    let area = Rect::new(0, 0, width, height);
    best(|| {
        let mut buf = Buffer::empty(area);
        Picker::new(vm, &theme, "mod", preview).render(area, &mut buf);
    })
}

// ---------------------------------------------------------------------------
// Table 1 — the window grows
// ---------------------------------------------------------------------------

struct Row {
    scale: usize,
    nanos: f64,
}

fn cost_by_window() -> Vec<Row> {
    println!("by window — the rows actually on screen");
    println!("    rows      µs/frame    % of a frame");

    let mut out = Vec::new();
    for rows in [10_usize, 20, 40, 80] {
        let height = u16::try_from(rows + 1).unwrap_or(u16::MAX);
        let elapsed = draw(&vm(rows, 100_000), 120, height, true);
        let micros = elapsed.as_secs_f64() * 1e6;
        let share = micros / (FRAME_BUDGET_MS * 1e3) * 100.0;
        println!("    {rows:>4}      {micros:>8.1}    {share:>11.2}%");
        out.push(Row {
            scale: rows,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// Table 2 — the corpus grows and the window does not
// ---------------------------------------------------------------------------

fn cost_by_corpus() -> Vec<Row> {
    println!("by corpus — 40 rows on screen, the source behind them growing");
    println!("  this is the table that matters: it must be FLAT");
    println!("    corpus       µs/frame");

    let mut out = Vec::new();
    for total in [100_usize, 10_000, 100_000, 1_000_000] {
        let elapsed = draw(&vm(40, total), 120, 41, true);
        let micros = elapsed.as_secs_f64() * 1e6;
        println!("    {total:>7}       {micros:>8.1}");
        out.push(Row {
            scale: total,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// Table 3 — §11's ladder
// ---------------------------------------------------------------------------

fn cost_by_width() -> Vec<(u16, bool, f64)> {
    println!("by width — §11's ladder, which drops the preview under 100 columns");
    println!("    width    preview    µs/frame");

    let theme = builtin("phosphor-dark").expect("the shipped theme");
    let held = vm(40, 100_000);
    let mut out = Vec::new();
    for width in [PREVIEW_AT - 20, PREVIEW_AT - 1, PREVIEW_AT, PREVIEW_AT + 60] {
        let shows = Picker::new(&held, &theme, "mod", true).shows_preview(width);
        let elapsed = draw(&held, width, 41, true);
        let micros = elapsed.as_secs_f64() * 1e6;
        let mark = if shows { "yes" } else { "no " };
        println!("    {width:>5}    {mark:>7}    {micros:>8.1}");
        out.push((width, shows, micros));
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------

/// What a doubling may cost and still be called linear. Generous on purpose —
/// this worktree has seen absolute times swing 25× under load while every shape
/// held, so a tight bound here would be a flake rather than a check.
const LINEAR_CEILING: f64 = 3.0;

/// What a 10,000× corpus growth may cost and still be called flat.
///
/// The widget never sees the corpus, so the true ratio is 1.0 and anything
/// above this is a widget that started iterating something it was not given.
const FLAT_CEILING: f64 = 2.0;

fn verdict(by_window: &[Row], by_corpus: &[Row], ladder: &[(u16, bool, f64)]) {
    println!("verdict");

    let window_growth = by_window
        .windows(2)
        .map(|pair| {
            let scale = pair[1].scale as f64 / pair[0].scale as f64;
            (pair[1].nanos / pair[0].nanos.max(f64::EPSILON)) / scale
        })
        .fold(0.0_f64, f64::max);

    let corpus_spread = {
        let first = by_corpus.first().map_or(1.0, |row| row.nanos);
        let worst = by_corpus
            .iter()
            .map(|row| row.nanos)
            .fold(0.0_f64, f64::max);
        worst / first.max(f64::EPSILON)
    };

    println!("  cost is linear in the window — worst doubling costs {window_growth:.2}× its scale");
    println!(
        "  cost is flat in the corpus — 10,000× more rows behind it costs {corpus_spread:.2}×"
    );

    let dropped = ladder
        .iter()
        .filter(|(_, shows, _)| !shows)
        .map(|(_, _, micros)| micros)
        .fold(f64::MAX, |a, b| a.min(*b));
    let drawn = ladder
        .iter()
        .filter(|(_, shows, _)| *shows)
        .map(|(_, _, micros)| micros)
        .fold(0.0_f64, |a, b| a.max(*b));
    println!(
        "  the ladder saves work as well as space — {dropped:.1}µs without the preview, {drawn:.1}µs with"
    );
    println!();

    // -- the structural half -------------------------------------------------

    assert!(
        window_growth < LINEAR_CEILING,
        "drawing went super-linear in the window: worst doubling was {window_growth:.2}× its \
         scale, ceiling {LINEAR_CEILING}",
    );
    assert!(
        corpus_spread < FLAT_CEILING,
        "drawing cost grew with the CORPUS ({corpus_spread:.2}×, ceiling {FLAT_CEILING}). The \
         widget is handed only the rows that fit — a cost that tracks the source means it \
         started iterating something it was not given, which is the one defect on this side of \
         the seam that the matcher's own bound cannot see.",
    );
    assert!(
        ladder.iter().any(|(_, shows, _)| *shows) && ladder.iter().any(|(_, shows, _)| !shows),
        "the ladder table must straddle the threshold or it is measuring one case twice",
    );
    let threshold_holds = ladder
        .iter()
        .all(|(width, shows, _)| *shows == (*width >= PREVIEW_AT));
    assert!(
        threshold_holds,
        "the preview did not drop exactly at {PREVIEW_AT} columns — §11's ladder is a cliff, \
         not a taper",
    );
}
