//! **What re-resolving a file's anchors costs**, and whether the tier ladder
//! is linear in the two things that actually grow (`T042`, `T043`).
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-core`). It prints
//! three tables and a verdict, and asserts the structural half of what it
//! prints — shapes, never times, for the reason `CLAUDE.md` gives: *"a figure
//! that moves with the machine has no business failing a build."*
//!
//! # Why a number here changes something
//!
//! [`store::anchor`]'s header makes two claims that are design decisions
//! somebody could reverse, and both are about cost:
//!
//! * *"the host hands down a [`Snapshot`]: the file's lines, each with the
//!   syntax path covering it"* — one `syntax_path` per line, eagerly. The
//!   alternative is a lazier seam where core asks for a line's syntax only when
//!   it needs one. That trade is only worth making if the eager build is a
//!   material part of the cost, and **the eager build is the host's and is not
//!   measured here** — this measures what core does with the snapshot once it
//!   has one. The two together are what a reanchor costs; this is the half that
//!   lives in a dependency-free crate and can be measured without a parser.
//! * *"reanchoring runs after a rewrite, never on the frame path"* — that is
//!   the claim licensing a linear scan per anchor. It is stated in prose in
//!   `main.rs` and nothing checked what "linear" costs at a realistic file. The
//!   third table is that number, and `CP-5`'s *"no full re-render stall on a
//!   file with 500+ regions"* is the acceptance it feeds.
//!
//! # The shape that matters, and the one that would be a bug
//!
//! [`resolve`] scans the snapshot once per tier. So one anchor against `L`
//! lines is `O(L)`, and `A` anchors is `O(A·L)` — quadratic in *the product*,
//! linear in each. That is fine at the sizes an editor sees and it is a real
//! ceiling worth knowing: a 10,000-line file with 500 anchors is five million
//! line comparisons per reanchor.
//!
//! What would be a bug is **super**-linear in either — an accidental clone of
//! the snapshot per anchor, or a per-line allocation — and that is what the
//! assertions check. They compare ratios against a generous ceiling rather than
//! an exact factor, because a doubling that measures 2.4× on a loaded machine
//! is still linear and failing on it would make this a flake.
//!
//! # What running it corrected
//!
//! The third table's prose said *"a miss is the expensive case: it scans both
//! tiers before answering none"*. It is the **cheap** case, by 5×, and the
//! reason is worth keeping: a fingerprint whose syntax path has a different
//! number of steps from a line's fails on `Vec`'s length comparison before a
//! single string is examined. The expensive case is a node-tier *hit* — equal
//! lengths, so every line pays a full step-by-step string compare. That
//! inverts where an optimisation would go if one were ever needed: hash the
//! path, do not shorten the scan.
//!
//! [`store::anchor`]: phosphor_core::store::anchor
//! [`Snapshot`]: phosphor_core::store::Snapshot
//! [`resolve`]: phosphor_core::store::anchor::resolve

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use phosphor_core::request::{Position, Span};
use phosphor_core::store::anchor::resolve;
use phosphor_core::store::{Anchors, Fingerprint, Snapshot, SyntaxStep, Tier};

/// One frame at 60fps. Design Language §8 makes a torn frame a P0.
const FRAME_BUDGET_MS: f64 = 16.7;

/// How many times each measurement repeats, so one slow scheduler slice does
/// not become the number.
const REPEATS: usize = 5;

fn main() {
    println!();
    println!("phosphor · the anchor ladder — resolution, and what a reanchor costs");
    println!("  frame budget  {FRAME_BUDGET_MS} ms (60fps)");
    println!("  repeats       {REPEATS}, best of");
    println!();

    let by_lines = resolution_by_file_size();
    let by_anchors = reanchor_by_anchor_count();
    let tiers = tier_costs();

    verdict(&by_lines, &by_anchors, &tiers);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A file of `lines` lines that looks like code: a repeating impl/fn shape, so
/// the node tier has real paths to match and the line tier has real duplicates
/// to disambiguate.
fn snapshot_of(lines: usize) -> Snapshot {
    let mut text = String::new();
    for index in 0..lines {
        match index % 4 {
            0 => text.push_str("impl Backoff {\n"),
            1 => text.push_str("fn retry(&self) -> u32 {\n"),
            2 => text.push_str(&format!("let attempts = {index};\n")),
            _ => text.push_str("}\n"),
        }
    }
    let mut snapshot = Snapshot::of(&text);
    for index in 0..lines {
        let group = index / 4;
        snapshot = snapshot.with_syntax(
            index,
            vec![
                SyntaxStep::new("impl_item", format!("Backoff{group}")),
                SyntaxStep::new("function_item", "retry"),
            ],
        );
    }
    snapshot
}

fn at(line: u32) -> Span {
    Span {
        start: Position { line, column: 1 },
        end: Position { line, column: 1 },
    }
}

fn path() -> PathBuf {
    PathBuf::from("src/retry.rs")
}

/// The best of [`REPEATS`] runs of `body`, which is the number least polluted
/// by whatever else the machine is doing.
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
// Table 1 — one anchor, against files of growing size
// ---------------------------------------------------------------------------

struct Row {
    scale: usize,
    nanos: f64,
}

fn resolution_by_file_size() -> Vec<Row> {
    println!("resolution — one anchor, node tier, against a file that grows");
    println!("    lines      µs/resolve    ns/line    resolves per frame");

    let mut rows = Vec::new();
    for lines in [500_usize, 1_000, 2_000, 4_000, 8_000] {
        let snapshot = snapshot_of(lines);
        // The last group, so the scan is not short-circuited by an early hit.
        let group = (lines - 1) / 4;
        let fingerprint = Fingerprint::new(
            vec![
                SyntaxStep::new("impl_item", format!("Backoff{group}")),
                SyntaxStep::new("function_item", "retry"),
            ],
            "let attempts = 0;",
            u32::try_from(lines).unwrap_or(u32::MAX),
        );

        let elapsed = best(|| {
            let found = resolve(&fingerprint, &snapshot);
            assert!(found.is_some(), "the fixture always has a match");
        });
        let micros = elapsed.as_secs_f64() * 1e6;
        let per_line = elapsed.as_secs_f64() * 1e9 / lines as f64;
        let per_frame = FRAME_BUDGET_MS * 1e3 / micros.max(f64::EPSILON);
        println!("    {lines:>6}      {micros:>9.1}    {per_line:>7.1}    {per_frame:>17.0}");
        rows.push(Row {
            scale: lines,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    rows
}

// ---------------------------------------------------------------------------
// Table 2 — a whole file's anchors, against a growing anchor count
// ---------------------------------------------------------------------------

fn reanchor_by_anchor_count() -> Vec<Row> {
    println!("reanchor — a 4,000-line file, as the anchor count grows");
    println!("  `CP-5` asks for no stall at 500+; the last row is that case and past it");
    println!("    anchors        ms/reanchor    µs/anchor    frames");

    let lines = 4_000;
    let snapshot = snapshot_of(lines);
    let mut rows = Vec::new();

    for count in [50_usize, 100, 250, 500, 1_000] {
        let mut anchors = Anchors::new();
        for index in 0..count {
            let line = u32::try_from(index * (lines / count) + 1).unwrap_or(1);
            let group = (index * (lines / count)) / 4;
            anchors.place(
                path(),
                at(line),
                None,
                Fingerprint::new(
                    vec![
                        SyntaxStep::new("impl_item", format!("Backoff{group}")),
                        SyntaxStep::new("function_item", "retry"),
                    ],
                    "let attempts = 0;",
                    line,
                ),
            );
        }

        let elapsed = best(|| {
            let mut scratch = anchors.clone();
            let outcome = scratch.reanchor(&path(), &snapshot);
            assert_eq!(outcome.total(), count, "every anchor is considered");
        });
        let millis = elapsed.as_secs_f64() * 1e3;
        let per_anchor = elapsed.as_secs_f64() * 1e6 / count as f64;
        let frames = millis / FRAME_BUDGET_MS;
        println!("    {count:>7}        {millis:>11.2}    {per_anchor:>9.1}    {frames:>6.2}");
        rows.push(Row {
            scale: count,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    rows
}

// ---------------------------------------------------------------------------
// Table 3 — what each rung costs
// ---------------------------------------------------------------------------

struct TierCost {
    tier: &'static str,
    nanos: f64,
}

fn tier_costs() -> Vec<TierCost> {
    println!("the ladder — what each rung costs on a 4,000-line file");
    println!("  the node tier is the dear one, and not for the reason you would guess");
    println!("    outcome              µs      note");

    let lines = 4_000;
    let snapshot = snapshot_of(lines);
    let group = (lines - 1) / 4;
    let node = Fingerprint::new(
        vec![
            SyntaxStep::new("impl_item", format!("Backoff{group}")),
            SyntaxStep::new("function_item", "retry"),
        ],
        "let attempts = 0;",
        u32::try_from(lines).unwrap_or(u32::MAX),
    );
    // No syntax at all — `T043`'s grammar-free file, which starts one rung down.
    let line = Fingerprint::new(Vec::new(), "let attempts = 3998;", 3_999);
    // Matches nothing, so both scans run to the end.
    let lost = Fingerprint::new(
        vec![SyntaxStep::new("impl_item", "NothingLikeThis")],
        "a line that is not in the file at all",
        1,
    );

    let mut out = Vec::new();
    for (name, fingerprint, expected) in [
        ("node tier hit", &node, Some(Tier::Node)),
        ("line tier hit", &line, Some(Tier::Line)),
        ("lost", &lost, None),
    ] {
        let elapsed = best(|| {
            let found = resolve(fingerprint, &snapshot);
            assert_eq!(found.map(|(_, tier)| tier), expected, "{name}");
        });
        let micros = elapsed.as_secs_f64() * 1e6;
        let note = match expected {
            Some(Tier::Node) => "full Vec<SyntaxStep> compare on every line — the dear one",
            Some(Tier::Line) => "no syntax, so the node scan never runs",
            _ => "both scans, but the node one dies on a length compare",
        };
        println!("    {name:<20} {micros:>5.0}      {note}");
        out.push(TierCost {
            tier: name,
            nanos: elapsed.as_secs_f64() * 1e9,
        });
    }
    println!();
    out
}

// ---------------------------------------------------------------------------
// The verdict, and the assertions
// ---------------------------------------------------------------------------

/// The ceiling a doubling is allowed to reach and still be called linear.
///
/// Generous on purpose. A perfectly linear doubling is 2.0; this worktree has
/// seen absolute times swing 25× under concurrent load while every shape held,
/// so a tight bound here would be a flake rather than a check. What it still
/// catches is the thing worth catching — an accidental quadratic, which lands
/// at 4.0 and keeps climbing.
const LINEAR_CEILING: f64 = 3.0;

fn growth(rows: &[Row]) -> Vec<f64> {
    rows.windows(2)
        .map(|pair| {
            let scale = pair[1].scale as f64 / pair[0].scale as f64;
            let cost = pair[1].nanos / pair[0].nanos.max(f64::EPSILON);
            cost / scale
        })
        .collect()
}

fn verdict(by_lines: &[Row], by_anchors: &[Row], tiers: &[TierCost]) {
    println!("verdict");

    let lines_growth = growth(by_lines);
    let anchors_growth = growth(by_anchors);
    let worst_lines = lines_growth.iter().copied().fold(0.0_f64, f64::max);
    let worst_anchors = anchors_growth.iter().copied().fold(0.0_f64, f64::max);

    println!(
        "  resolution is linear in file size — worst doubling costs {worst_lines:.2}× its scale"
    );
    println!(
        "  reanchor is linear in anchor count — worst doubling costs {worst_anchors:.2}× its scale"
    );

    let lost = tiers
        .iter()
        .find(|cost| cost.tier == "lost")
        .map_or(0.0, |cost| cost.nanos);
    let hit = tiers
        .iter()
        .find(|cost| cost.tier == "node tier hit")
        .map_or(0.0, |cost| cost.nanos);
    println!(
        "  a miss costs {:.2}× a node-tier hit — CHEAPER, and the first draft of this",
        lost / hit.max(f64::EPSILON)
    );
    println!("  line asserted the opposite. A miss scans both rungs and is still the");
    println!("  cheap case, because a fingerprint whose path has a different number of");
    println!("  steps fails on `Vec`'s length compare before one string is looked at.");
    println!("  The dear case is a node-tier *hit*: same length, so every line pays a");
    println!("  full step-by-step string comparison. If the node tier ever needs to be");
    println!("  faster, that is the comparison to hash — not the scan to shorten.");
    println!();
    println!("  what is NOT measured here: building the Snapshot. That is one");
    println!("  `Code::syntax_path` per line and it is the host's, in the binary,");
    println!("  because `phosphor-core` has no parser. A reanchor costs that plus");
    println!("  this; if the pair ever matters, the seam to make lazy is that one.");
    println!();

    // -- the structural half, and the only thing that can fail this build ----

    assert!(
        worst_lines < LINEAR_CEILING,
        "resolution went super-linear in file size: worst doubling was {worst_lines:.2}× \
         its scale, ceiling is {LINEAR_CEILING}. A scan that clones the snapshot per \
         line lands here.",
    );
    assert!(
        worst_anchors < LINEAR_CEILING,
        "reanchor went super-linear in anchor count: worst doubling was {worst_anchors:.2}× \
         its scale, ceiling is {LINEAR_CEILING}.",
    );
    assert!(
        by_lines.len() >= 3 && by_anchors.len() >= 3,
        "a growth check needs at least three points to be a shape rather than a pair",
    );
    assert_eq!(tiers.len(), 3, "every rung of the ladder is measured");
}
