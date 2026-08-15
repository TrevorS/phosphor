//! **What a publish costs** — `T040`'s `ingest-diagnostics`, from the Action to
//! the screen.
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-ui`). It prints three
//! tables and a verdict, and asserts the structural half of what it prints.
//!
//! # Why a number here changes something
//!
//! A publish is not a keystroke and it is not a frame — it is a **producer**.
//! `phosphor-buffer`'s LSP client turns one `publishDiagnostics` into an
//! `Action` and posts it into the event queue, and the loop applies it between
//! frames, on the thread that draws. rust-analyzer re-publishes a file every
//! time it finishes analysing a change, and a large Rust file mid-refactor
//! produces **hundreds** at once — one per unresolved call, for as long as the
//! refactor is half-done. So the question is whether one publish fits inside a
//! frame, and if not, at what size it stops fitting. Design Language §8 makes a
//! torn frame a P0.
//!
//! # What a publish does, and the one piece to watch
//!
//! Four pieces per publish, plus one that is per frame:
//!
//! 1. [`DiagnosticsVm::regions`] — a visual row per span end, through the
//!    fork's `Editor::visual_row_for_position`.
//! 2. [`DiagnosticsVm::rows`] — one `┊ ■` row per diagnostic.
//! 3. `virtual_text::install` — those rows into the fork's row stream.
//! 4. [`DiagnosticsVm::underlines`] — one `StyledSpan` per diagnostic.
//! 5. `gutter::state_column` — the ladder. Per **frame**, not per publish.
//!
//! **Piece 1 is the one to watch, and the reason is in the fork.**
//! `View::visual_row_for_line` (`vendor/ratatui-code-editor/src/view.rs`) is
//! `self.rows.iter().position(…)` — a scan from row 0 — whenever the row stream
//! is non-empty, and `View::rebuild` in the same file fills that stream with one
//! `VisualRow::Real` per line **even with soft wrap and folding off**. So the
//! empty-stream fast path above it is unreachable in a running editor, and a
//! publish of `N` diagnostics walks the stream `2N` times. That is the shape the
//! size ladder below measures, and it is why there is no second ladder for the
//! "cold" case: there is no cold case.
//!
//! # The three tables
//!
//! 1. **the count ladder** — a 64x climb in diagnostics against a fixed file.
//!    Flat microseconds-per-diagnostic is O(n) in the count, which is the claim
//!    about this crate's own code: a publish walks its input once.
//! 2. **the file ladder** — a 16x climb in file size at a fixed count. The
//!    number to act on comes out of this one.
//! 3. **the parts** — one realistic publish split five ways, so a reader who
//!    has to fix something knows which piece to open.
//!
//! # What these numbers are not
//!
//! Wall clock on one machine. The *shapes* are asserted — linear versus
//! quadratic — because those are machine-independent; the absolute milliseconds
//! are information for a person deciding whether the row mapping needs a second
//! pass. Same rule as `benches/soft_wrap.rs` and `benches/frame_cache.rs`, and
//! the reason `just bench` is deliberately not part of `just gate`.
//!
//! Each piece is the **fastest of [`REPETITIONS`] runs** rather than one cold
//! sample ([`timed`]). The verdict is a spread across a ladder, so one rung
//! polluted by whatever else the machine was doing moves a ratio, not just a
//! number — and cold single samples had the file ladder coming back
//! non-monotonic on an idle machine.
//!
//! Owned by `surface`.

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::time::{Duration, Instant};

use phosphor_core::request::{Diagnostic, Position, Severity, Span};
use phosphor_ui::buffer_view::{Editor, configure as configure_buffer};
use phosphor_ui::diagnostics::DiagnosticsVm;
use phosphor_ui::gutter;
use phosphor_ui::theme::Theme;
use phosphor_ui::virtual_text;

/// One frame at 60fps, in milliseconds. A publish that costs this much tears
/// the frame it lands in.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// Diagnostics per publish, climbing 64x. 256 is a large Rust file mid-refactor
/// — an unresolved call per line over a few hundred lines — and 1 024 is what a
/// workspace-wide rename produces before the rename finishes.
const COUNTS: [usize; 4] = [16, 64, 256, 1_024];

/// Lines in the file, climbing 16x.
const SIZES: [usize; 5] = [1_024, 2_048, 4_096, 8_192, 16_384];

/// The file the count ladder runs against.
const FIXED_LINES: usize = 4_096;

/// The publish the file ladder runs — a plausible mid-refactor count.
const FIXED_COUNT: usize = 256;

fn main() {
    println!("phosphor · T040 diagnostics — what one publish costs, from the Action to the gutter");
    println!(
        "  frame budget  {FRAME_BUDGET_MS:.1} ms at 60fps — a publish is applied between frames \
         on the thread that draws, so one over budget tears that frame"
    );
    let theme = Theme::phosphor_dark();

    let counts: Vec<Measured> = COUNTS
        .iter()
        .map(|count| measure(&theme, FIXED_LINES, *count))
        .collect();
    let sizes: Vec<Measured> = SIZES
        .iter()
        .map(|lines| measure(&theme, *lines, FIXED_COUNT))
        .collect();
    println!();

    count_table(&counts);
    size_table(&sizes);
    parts_table(&sizes);
    verdict(&counts, &sizes);
}

// ---------------------------------------------------------------------------
// The measurement
// ---------------------------------------------------------------------------

/// One publish, timed in five pieces.
#[derive(Debug, Clone, Copy)]
struct Measured {
    lines: usize,
    count: usize,
    rows: usize,
    regions: Duration,
    column: Duration,
    build_rows: Duration,
    install: Duration,
    underlines: Duration,
}

impl Measured {
    /// What the publish costs. The state column is left out: it is a per-frame
    /// read of the result, not part of applying the Action.
    fn publish(&self) -> Duration {
        self.regions + self.build_rows + self.install + self.underlines
    }

    fn millis(&self) -> f64 {
        self.publish().as_secs_f64() * 1e3
    }

    /// The count ladder's shape column. Flat means the publish walks its own
    /// input once and nothing worse.
    fn micros_per_diagnostic(&self) -> f64 {
        self.publish().as_secs_f64() * 1e6 / self.count as f64
    }

    /// The file ladder's shape column: the cost of one diagnostic's row lookup,
    /// per row of the stream it scans. Flat means the scan is linear in the
    /// stream — the *fork's* shape, measured through this crate's use of it.
    fn nanos_per_row_per_diagnostic(&self) -> f64 {
        self.regions.as_secs_f64() * 1e9 / (self.rows * self.count) as f64
    }

    fn frames(&self) -> f64 {
        self.millis() / FRAME_BUDGET_MS
    }
}

/// One publish onto a file that already carries the previous one's rows — the
/// steady state, and the only state a running editor is in after the first
/// publish.
fn measure(theme: &Theme, lines: usize, count: usize) -> Measured {
    let source = fixture(lines);
    let diagnostics = diagnostics(&source, count);
    let mut editor = editor(theme, &source);
    let vm = DiagnosticsVm::new(&diagnostics);
    virtual_text::install(&mut editor, &vm.rows(theme));

    let (regions, t_regions) = timed(|| vm.regions(&editor));
    let (_marks, t_column) = timed(|| gutter::state_column(&regions, editor.visual_len_lines()));
    let (rows, t_build) = timed(|| vm.rows(theme));
    let (_, t_install) = timed(|| virtual_text::install(&mut editor, &rows));
    let (spans, t_underlines) = timed(|| vm.underlines(&editor, theme));
    editor.set_styled_spans(spans);

    Measured {
        lines,
        count,
        rows: editor.visual_len_lines(),
        regions: t_regions,
        column: t_column,
        build_rows: t_build,
        install: t_install,
        underlines: t_underlines,
    }
}

/// Runs per piece. The first is the warm-up and the minimum discards it, which
/// is the whole reason there is more than one.
const REPETITIONS: usize = 5;

/// The **fastest** of [`REPETITIONS`] runs, and the last run's value.
///
/// A minimum rather than a mean: every source of noise on a developer's machine
/// — a scheduler slice, a rebuild in another worktree — makes a sample slower
/// and none makes one faster, so the smallest sample is the one closest to what
/// the code costs. It matters here because the verdict is a **spread** across a
/// ladder (`count_spread`, `scan_spread`), and one polluted rung moves a ratio
/// that has to survive a 25x swing under concurrent load; the first version
/// timed each rung exactly once, cold, and the file ladder came back
/// non-monotonic on an idle machine.
///
/// `FnMut` rather than `FnOnce`, so the pieces that mutate the editor
/// (`install`) can be measured too — every one of them is idempotent, which is
/// what makes repeating them measure the same work each time.
fn timed<T>(mut work: impl FnMut() -> T) -> (T, Duration) {
    let mut best = Duration::MAX;
    let mut out = None;
    for _ in 0..REPETITIONS {
        let started = Instant::now();
        let value = work();
        best = best.min(started.elapsed());
        out = Some(value);
    }
    (out.expect("REPETITIONS is not zero"), best)
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// A Rust file of `lines` lines, mid-refactor: a call per line to a function
/// whose signature just changed, which is the shape that produces one
/// diagnostic per line.
fn fixture(lines: usize) -> String {
    let mut out = String::with_capacity(lines * 48);
    out.push_str("use std::time::Duration;\n\nfn main() {\n");
    for line in 0..lines.saturating_sub(4) {
        out.push_str("    jitter(exp.min(self.max_delay), ");
        out.push_str(&line.to_string());
        out.push_str(");\n");
    }
    out.push_str("}\n");
    out
}

/// `count` diagnostics spread evenly over the file, each covering the call on
/// its line.
///
/// Two thirds errors and one third warnings, because a real refactor produces
/// both and they take different arms of `diagnostics::state` — a fixture of
/// pure errors would leave the amber arm unmeasured.
fn diagnostics(source: &str, count: usize) -> Vec<Diagnostic> {
    let lines = source.lines().count();
    let step = (lines / count.max(1)).max(1);
    (0..count)
        .map(|index| {
            let line = u32::try_from((index * step % lines) + 1).unwrap_or(1);
            Diagnostic {
                span: Span {
                    start: Position { line, column: 5 },
                    end: Position { line, column: 36 },
                },
                severity: if index.is_multiple_of(3) {
                    Severity::Attention
                } else {
                    Severity::Trouble
                },
                message: "expected Duration, found u128".to_owned(),
                source: Some("rust-analyzer".to_owned()),
            }
        })
        .collect()
}

/// An editor configured the way `BufferView` configures one. Soft wrap stays
/// off: `View::rebuild` fills the row stream either way, and wrapping on top of
/// it would only make the ladders harder to read.
fn editor(theme: &Theme, source: &str) -> Editor {
    let mut editor = Editor::new("rust", source, Vec::new()).expect("the rust grammar loads");
    configure_buffer(&mut editor, theme);
    virtual_text::configure(&mut editor, theme);
    editor
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn count_table(measured: &[Measured]) {
    println!("the count ladder — one publish onto a {FIXED_LINES}-line file");
    println!("    diagnostics       rows    ms/publish    us/diagnostic    frames");
    for one in measured {
        println!(
            "  {:>13}  {:>9}    {:>10.3}    {:>13.2}    {:>6.2}",
            one.count,
            one.rows,
            one.millis(),
            one.micros_per_diagnostic(),
            one.frames(),
        );
    }
    println!();
}

fn size_table(measured: &[Measured]) {
    println!(
        "the file ladder — a publish of {FIXED_COUNT} diagnostics against a 16x climb in file size"
    );
    println!("          lines       rows    ms/publish    us/diagnostic    ns/row/diag    frames");
    for one in measured {
        println!(
            "  {:>13}  {:>9}    {:>10.3}    {:>13.2}    {:>11.3}    {:>6.2}",
            one.lines,
            one.rows,
            one.millis(),
            one.micros_per_diagnostic(),
            one.nanos_per_row_per_diagnostic(),
            one.frames(),
        );
    }
    println!();
}

fn parts_table(measured: &[Measured]) {
    let one = measured.last().expect("the ladder is not empty");
    println!(
        "the parts — one publish of {} diagnostics onto {} lines, split five ways",
        one.count, one.lines
    );
    println!("    piece                          ms      share    per frame?");
    for (name, cost, per_frame) in [
        ("regions (visual rows)", one.regions, false),
        ("rows (the ┊ ■ text)", one.build_rows, false),
        ("install (the row stream)", one.install, false),
        ("underlines (the spans)", one.underlines, false),
        ("state_column (the ladder)", one.column, true),
    ] {
        println!(
            "  {:<28}  {:>8.3}    {:>6.1}%    {}",
            name,
            cost.as_secs_f64() * 1e3,
            cost.as_secs_f64() * 100.0 / one.publish().as_secs_f64().max(f64::MIN_POSITIVE),
            if per_frame { "yes" } else { "no" },
        );
    }
    println!();
}

/// Largest over smallest. ~1 means the cost does not track that ladder's
/// parameter at all.
fn spread(values: &[f64]) -> f64 {
    let max = values.iter().copied().fold(f64::MIN, f64::max);
    let min = values.iter().copied().fold(f64::MAX, f64::min);
    max / min.max(f64::MIN_POSITIVE)
}

fn verdict(counts: &[Measured], sizes: &[Measured]) {
    let per_diagnostic: Vec<f64> = counts.iter().map(Measured::micros_per_diagnostic).collect();
    let per_row: Vec<f64> = sizes
        .iter()
        .map(Measured::nanos_per_row_per_diagnostic)
        .collect();
    let count_spread = spread(&per_diagnostic);
    let scan_spread = spread(&per_row);

    // The count ladder climbs 64x. O(n) in the count leaves us/diagnostic flat;
    // 8 is the widest a machine's noise has any business being, and the same
    // bound `soft_wrap.rs` uses for the same reason.
    let linear_in_count = count_spread < 8.0;
    // The file ladder climbs 16x. A scan is linear in the stream, so the cost
    // per row per diagnostic is flat; anything that made the lookup quadratic
    // in the stream would put the whole climb into this number.
    let scan_is_linear = scan_spread < 8.0;

    let biggest = sizes.last().expect("the ladder is not empty");
    let rows_per_frame = FRAME_BUDGET_MS * 1e6
        / (per_row[per_row.len() - 1] * biggest.count as f64).max(f64::MIN_POSITIVE);

    println!("verdict");
    println!(
        "  count         {:.2} us/diagnostic at {}, {:.2} at {} ({count_spread:.1}x over a 64x \
         climb — {})",
        per_diagnostic[0],
        counts[0].count,
        per_diagnostic[per_diagnostic.len() - 1],
        counts[counts.len() - 1].count,
        if linear_in_count { "LINEAR" } else { "WORSE" },
    );
    println!(
        "  file          {:.2} us/diagnostic at {} lines, {:.2} at {} — the publish tracks the \
         FILE, not only its own input",
        sizes[0].micros_per_diagnostic(),
        sizes[0].lines,
        biggest.micros_per_diagnostic(),
        biggest.lines,
    );
    println!(
        "  the scan      {:.3} ns per row per diagnostic, {scan_spread:.1}x across the climb — \
         {}",
        per_row[per_row.len() - 1],
        if scan_is_linear {
            "a linear scan of the row stream, once per span end"
        } else {
            "worse than a scan; the lookup is superlinear in the stream"
        },
    );
    println!(
        "  regions       {:.0}% of the publish at the top of the ladder",
        biggest.regions.as_secs_f64() * 100.0
            / biggest.publish().as_secs_f64().max(f64::MIN_POSITIVE),
    );
    println!();
    println!("  the number to act on:");
    println!(
        "    a publish of {} diagnostics costs a whole frame at about {rows_per_frame:.0} visual \
         rows.",
        biggest.count,
    );
    println!(
        "    Below that it lands between frames unnoticed; above it, every publish while a \
         refactor is"
    );
    println!(
        "    half-done tears a frame. The fix is not a smaller constant: \
         `DiagnosticsVm::regions` resolves"
    );
    println!(
        "    each span end through `Editor::visual_row_for_position`, and `View::rebuild` fills \
         the row"
    );
    println!(
        "    stream with a row per line even unwrapped and unfolded \
         (`vendor/ratatui-code-editor/src/view.rs`),"
    );
    println!(
        "    so the fast path above the scan is unreachable and N diagnostics walk the stream 2N \
         times."
    );
    println!(
        "    One walk per publish, bucketing the diagnostics by line as it goes, makes it O(rows \
         + N)"
    );
    println!("    — and that is a change to this crate, not to the fork.");
    println!();
    println!(
        "  T040: {}",
        if linear_in_count && scan_is_linear {
            "PASS — the publish is linear in the diagnostics it is given, and the row lookup \
             behind it is a linear scan rather than something worse"
        } else {
            "FAIL — see the tables above"
        }
    );

    assert!(
        linear_in_count,
        "the cost per diagnostic climbed {count_spread:.1}x over a 64x climb in the count; a \
         publish walks its own input once, so this is superlinear where it should not be"
    );
    assert!(
        scan_is_linear,
        "the row lookup's cost per row per diagnostic climbed {scan_spread:.1}x over a 16x climb \
         in the row stream — `visual_row_for_position` is a linear scan, so this says it stopped \
         being one and the publish is now quadratic in the file"
    );
}
