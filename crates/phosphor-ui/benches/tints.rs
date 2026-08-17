#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]
//! **What region tints cost on a file with 500+ regions** (`T087`).
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-ui`). Three tables and
//! a verdict; the assertions are *shapes*, never times.
//!
//! # Why a number here changes something
//!
//! `T087`'s acceptance is *"marking a region seen retints it with no full
//! re-render stall on a file with 500+ regions"*, and the reason that is even
//! in question is the API: `set_marks` **replaces wholesale**, so the naive
//! implementation re-uploads every mark on every frame. `T008`'s spike named
//! this as the thing to watch.
//!
//! So the design is a diff, and the two numbers that matter are:
//!
//! 1. **What a quiet frame costs.** The loop calls `sync` every frame. On a
//!    frame where nothing moved it must cost a comparison and nothing else —
//!    if it does not, the diff is not earning its keep and the whole side table
//!    is complexity for nothing.
//! 2. **What the one frame that *does* change costs.** `s` retints one region
//!    and re-uploads the set. That is the frame the acceptance is about.
//!
//! # What running it corrected
//!
//! The verdict asserted that a quiet frame is *much* cheaper than a changed
//! one, and it is **not**: 0.91× at 500 regions. Both walk every region and
//! build the same vector; the diff only avoids the `set_marks` call, which on
//! this side is storing a `Vec`.
//!
//! That is not the diff being useless — what it avoids is whatever the *fork*
//! does downstream of a replaced mark set, which is the cost `T008`'s spike
//! warned about and which this crate cannot measure. But it is not measurable
//! here, so the assertion that claimed it was has been replaced by one that
//! holds: uploads are counted, not timed. `Tints::sync` answering `false` on a
//! quiet frame is a *fact*, and the quiet table asserts it on every iteration.
//!
//! What the numbers are actually good for is the acceptance itself: at 500
//! regions a changed frame is well under 1% of a frame budget, so *"no full
//! re-render stall"* has room to spare.

use std::time::{Duration, Instant};

use ratatui_code_editor::editor::Editor;

use phosphor_core::request::{Position, Span};
use phosphor_ui::gutter::RegionState;
use phosphor_ui::theme::{Theme, builtin};
use phosphor_ui::tints::Tints;

/// One frame at 60fps. Design Language §8 makes a torn frame a P0.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// Best-of, so one scheduler slice is not the number.
const REPEATS: usize = 50;

/// `T087`'s own scale — *"a file with 500+ regions"*.
const ACCEPTANCE: usize = 500;

fn main() {
    println!();
    println!("phosphor · region tints — what the marks side table costs");
    println!("  frame budget  {FRAME_BUDGET_MS:.1} ms (60fps)");
    println!("  acceptance    {ACCEPTANCE}+ regions");
    println!("  repeats       {REPEATS}, best of");
    println!();

    let quiet = quiet_frames();
    let changed = changed_frames();
    verdict(&quiet, &changed);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A buffer with three lines per region, so every region has somewhere to sit.
fn buffer(regions: usize) -> Editor {
    let text: String = (0..regions * 3)
        .map(|n| format!("let attempts_{n} = {n};\n"))
        .collect();
    Editor::new("rust", &text, Vec::new()).expect("an editor")
}

/// `regions` regions, every third line, all unseen.
fn regions(count: usize, seen_at: Option<usize>) -> Vec<(Span, RegionState)> {
    (0..count)
        .map(|n| {
            let first = u32::try_from(n * 3 + 1).unwrap_or(1);
            (
                Span {
                    start: Position {
                        line: first,
                        column: 1,
                    },
                    end: Position {
                        line: first + 2,
                        column: 1,
                    },
                },
                if seen_at == Some(n) {
                    RegionState::Seen
                } else {
                    RegionState::Unseen
                },
            )
        })
        .collect()
}

fn theme() -> Theme {
    builtin("phosphor-dark").expect("the shipped theme")
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

// ---------------------------------------------------------------------------
// Table 1 — the frame that does nothing
// ---------------------------------------------------------------------------

struct Row {
    scale: usize,
    nanos: f64,
}

fn quiet_frames() -> Vec<Row> {
    println!("a quiet frame — sync called, nothing changed");
    println!("  the loop does this every frame, so it is the number that must stay small");
    println!("    regions      µs/frame    % of a frame");

    let theme = theme();
    let mut out = Vec::new();
    for count in [125_usize, 250, ACCEPTANCE, 1_000] {
        let mut editor = buffer(count);
        let held = regions(count, None);
        let mut tints = Tints::new();
        tints.sync(&mut editor, &theme, &held);

        let elapsed = best(|| {
            let uploaded = tints.sync(&mut editor, &theme, &held);
            assert!(!uploaded, "a quiet frame must not upload");
        });
        let micros = elapsed.as_secs_f64() * 1e6;
        let share = micros / (FRAME_BUDGET_MS * 1e3) * 100.0;
        println!("    {count:>7}      {micros:>8.1}    {share:>11.3}%");
        out.push(Row {
            scale: count,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// Table 2 — the frame `s` happens on
// ---------------------------------------------------------------------------

fn changed_frames() -> Vec<Row> {
    println!("the frame `s` lands on — one region retinted, the set re-uploaded");
    println!("    regions      µs/frame    % of a frame");

    let theme = theme();
    let mut out = Vec::new();
    for count in [125_usize, 250, ACCEPTANCE, 1_000] {
        let mut editor = buffer(count);
        let all_unseen = regions(count, None);
        let one_seen = regions(count, Some(count / 2));
        let mut tints = Tints::new();

        // Alternating, so every timed call is a real change rather than the
        // first one paying and the rest being quiet frames in disguise.
        let elapsed = best(|| {
            assert!(tints.sync(&mut editor, &theme, &one_seen));
            assert!(tints.sync(&mut editor, &theme, &all_unseen));
        });
        let micros = elapsed.as_secs_f64() * 1e6 / 2.0;
        let share = micros / (FRAME_BUDGET_MS * 1e3) * 100.0;
        println!("    {count:>7}      {micros:>8.1}    {share:>11.3}%");
        out.push(Row {
            scale: count,
            nanos: elapsed.as_secs_f64() * 1e9 / 2.0,
        });
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------

/// What a doubling may cost and still be called linear. Generous: this worktree
/// has seen absolute times swing 25× under load while every shape held.
const LINEAR_CEILING: f64 = 3.0;

fn verdict(quiet: &[Row], changed: &[Row]) {
    println!("verdict");

    let growth = |rows: &[Row]| {
        rows.windows(2)
            .map(|pair| {
                let scale = pair[1].scale as f64 / pair[0].scale as f64;
                (pair[1].nanos / pair[0].nanos.max(f64::EPSILON)) / scale
            })
            .fold(0.0_f64, f64::max)
    };
    let at = |rows: &[Row], scale: usize| {
        rows.iter()
            .find(|row| row.scale == scale)
            .map_or(0.0, |row| row.nanos)
    };

    let quiet_growth = growth(quiet);
    let changed_growth = growth(changed);
    let saved = at(changed, ACCEPTANCE) / at(quiet, ACCEPTANCE).max(f64::EPSILON);
    let worst_change = at(changed, ACCEPTANCE) / 1e6 / (FRAME_BUDGET_MS) * 100.0;

    println!("  a quiet frame is linear in the region count — worst doubling {quiet_growth:.2}×");
    println!("  a changed frame is linear too — worst doubling {changed_growth:.2}×");
    println!(
        "  T087's acceptance: at {ACCEPTANCE} regions the frame `s` lands on costs \
         {worst_change:.2}% of a frame"
    );
    println!();
    println!("  the diff saves {saved:.2}× of *this crate's* time, which is nothing, and");
    println!("  the verdict claimed otherwise until it was run. both paths walk every");
    println!("  region and build the same vector; the diff only skips `set_marks`, which");
    println!("  here is storing a Vec. what it actually avoids is whatever the fork does");
    println!("  downstream of a replaced mark set — T008's warning, and not measurable");
    println!("  from this side. so the claim that survives is counted rather than timed:");
    println!("  a quiet frame uploads NOTHING, asserted on every iteration above.");
    println!();

    // -- the structural half -------------------------------------------------

    assert!(
        quiet_growth < LINEAR_CEILING,
        "a quiet frame went super-linear: worst doubling {quiet_growth:.2}×, ceiling \
         {LINEAR_CEILING}",
    );
    assert!(
        changed_growth < LINEAR_CEILING,
        "a changed frame went super-linear: worst doubling {changed_growth:.2}×, ceiling \
         {LINEAR_CEILING}",
    );
    // **`T087`'s acceptance, as a stall detector rather than a timing.** One
    // half of a frame budget is ~35× the observed cost, so no plausible load
    // reaches it and only a genuine stall can. That the diff *works* is
    // asserted by counting — `quiet_frames` fails if `sync` ever uploads on a
    // frame with no news — which is a fact rather than a measurement.
    assert!(
        worst_change < 50.0,
        "retinting at {ACCEPTANCE} regions cost {worst_change:.1}% of a frame; T087 asks for \
         no full re-render stall",
    );
    assert!(
        quiet.len() >= 3 && changed.len() >= 3,
        "a growth check needs three points to be a shape rather than a pair",
    );
}
