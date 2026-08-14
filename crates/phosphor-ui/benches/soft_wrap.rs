//! **What a resize costs**, on the one path in the draw loop that has no cache
//! — `T081`'s soft wrap.
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-ui`). It prints four
//! tables and a verdict, and asserts the structural half of what it prints.
//!
//! # Why a number here changes something
//!
//! Every other expensive thing in the frame path is cached. The view tree is —
//! that is `T079`, and `benches/frame_cache.rs` next door is the proof.
//! Highlighting is, inside the vendored core. **The row stream is not.**
//! `soft_wrap::wrap_to`'s own header says the rebuild happens *"whenever the
//! area changes"*, and `crate::soft_wrap`'s header says why it cannot live
//! anywhere else: row↔line mapping, cursor placement, click targeting and
//! virtual-text placement all read that one list, so a wrap that is cached
//! separately from it desynchronises all four.
//!
//! So the rebuild is unavoidable, and the only question left is what it costs.
//! Design Language §8 makes a torn frame a P0. A window drag emits a resize per
//! frame; if one rebuild costs more than [`FRAME_BUDGET_MS`], **every frame of
//! that drag is late**, and the fix is a different design (wrap the viewport
//! and extend lazily, or rebuild off the draw path) rather than a smaller
//! constant.
//!
//! # What the rebuild actually does, and what to watch
//!
//! `phosphor::soft_wrap::apply` in the fork calls `segments` for **every real
//! row**, and `segments` opens with
//! `code.char_col_to_visual(line_idx, line_len) <= width` — a grapheme walk of
//! the whole line, run even for lines that obviously fit. So the floor is
//! O(characters in the buffer) per rebuild, on every width change, with no
//! early exit and nothing memoised. Whether that floor is also the ceiling is
//! the second table: a line's segments are cut by re-slicing from the segment
//! start to the *end of the line* each time round, and if that slice were a
//! copy rather than a rope view, a long line would be quadratic in its own
//! length. It is a `RopeSlice`, so it should not be. Should is why this
//! measures it.
//!
//! # The four tables
//!
//! 1. **size ladder** — one resize against a 16x climb in buffer size, at a
//!    fixed line length. Flat nanoseconds-per-character is O(n).
//! 2. **line shape** — the same character count arranged three ways: many short
//!    lines, few long lines, and one very long line (the minified-JSON case).
//!    Cost that tracks characters rather than line shape is the claim.
//! 3. **the unchanged width** — `wrap_to` is called every frame by design, and
//!    its header promises that *"calling it every frame is free"*. Free is a
//!    word; this is the number.
//! 4. **the drag** — a resize per frame from 120 columns to 80, which is what a
//!    user dragging a window edge produces.
//!
//! # What these numbers are not
//!
//! Wall clock on one machine. The *shapes* are asserted — linear versus
//! quadratic, and a no-op that is orders of magnitude cheaper than a rebuild —
//! because those are machine-independent; the absolute milliseconds are
//! information for a person deciding whether `T081` needs a second pass. Same
//! rule as `benches/frame_cache.rs` and `phosphor/benches/vm_invocations.rs`,
//! and the reason `just bench` is deliberately not part of `just gate`.
//!
//! Owned by `harness`.

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::time::{Duration, Instant};

use phosphor_ui::buffer_view::{Editor, configure as configure_buffer};
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::Theme;
use ratatui_core::layout::Rect;

/// One frame at 60fps, in milliseconds. A rebuild that costs this much turns a
/// window drag into a slideshow, and Design Language §8 calls the result a P0.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// The screen a rebuild is measured at — `9c`'s proportions, and the ones
/// `frame_cache.rs` and `vm_invocations.rs` both use, so the three benchmarks
/// are talking about the same window.
const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

/// Characters per line in the size ladder. Long enough to wrap onto three rows
/// in `SCREEN`'s text column, which is the case the whole module exists for.
const LADDER_LINE: usize = 200;

/// Lines per rung. A 16x climb, which is what makes O(n) and O(n²) different
/// answers rather than different noise.
const LADDER: [usize; 5] = [1_024, 2_048, 4_096, 8_192, 16_384];

/// Characters in every arm of the line-shape table. One number, three
/// arrangements — that is the whole experiment.
const SHAPE_CHARS: usize = 400_000;

/// Calls in the unchanged-width table. Enough that one call's cost is legible
/// in nanoseconds.
const IDLE_CALLS: u32 = 10_000;

/// Columns a drag passes through: 120 down to 80, one per frame.
const DRAG: std::ops::Range<u16> = 80..120;

fn main() {
    println!("phosphor · B2 soft wrap — what a resize costs on the one uncached path in the frame");
    println!(
        "  frame budget  {FRAME_BUDGET_MS:.1} ms at 60fps — a drag emits one resize per frame, so \
         a rebuild over budget tears every frame of it"
    );
    let theme = Theme::phosphor_dark();
    println!(
        "  screen        {}x{} · text column {} cells, read back from the gutter the 3-column \
         contract reserves",
        SCREEN.width,
        SCREEN.height,
        soft_wrap::text_width(&editor(&theme, "// probe\n"), SCREEN),
    );
    println!();

    let ladder: Vec<Rung> = LADDER.iter().map(|lines| rung(&theme, *lines)).collect();
    let shapes = [
        shaped(&theme, "many short lines", SHAPE_CHARS / 80, 80),
        shaped(&theme, "few long lines", SHAPE_CHARS / 4_000, 4_000),
        shaped(&theme, "one minified line", 1, SHAPE_CHARS),
    ];
    let idle = idle_calls(&theme);
    let drag = drag_cost(&theme);

    ladder_table(&ladder);
    shape_table(&shapes);
    idle_table(idle, &ladder);
    drag_table(drag);
    verdict(&ladder, &shapes, idle, drag);
}

// ---------------------------------------------------------------------------
// The measurements
// ---------------------------------------------------------------------------

/// One rung of the size ladder.
#[derive(Debug, Clone, Copy)]
struct Rung {
    lines: usize,
    chars: usize,
    rows: usize,
    rebuild: Duration,
}

impl Rung {
    fn millis(&self) -> f64 {
        self.rebuild.as_secs_f64() * 1e3
    }

    /// The shape column. Flat across the ladder means the rebuild is O(n) in
    /// the characters it walks, which is the floor `segments` sets.
    fn nanos_per_char(&self) -> f64 {
        self.rebuild.as_secs_f64() * 1e9 / self.chars as f64
    }

    fn frames(&self) -> f64 {
        self.millis() / FRAME_BUDGET_MS
    }
}

fn rung(theme: &Theme, lines: usize) -> Rung {
    let source = fixture(lines, LADDER_LINE);
    let chars = source.chars().count();
    let mut editor = editor(theme, &source);
    let rebuild = one_rebuild(&mut editor);
    Rung {
        lines,
        chars,
        rows: editor.visual_len_lines(),
        rebuild,
    }
}

/// One arrangement of a fixed number of characters.
#[derive(Debug, Clone, Copy)]
struct Shape {
    name: &'static str,
    lines: usize,
    line_chars: usize,
    chars: usize,
    rows: usize,
    rebuild: Duration,
}

impl Shape {
    fn millis(&self) -> f64 {
        self.rebuild.as_secs_f64() * 1e3
    }

    fn nanos_per_char(&self) -> f64 {
        self.rebuild.as_secs_f64() * 1e9 / self.chars as f64
    }
}

fn shaped(theme: &Theme, name: &'static str, lines: usize, line_chars: usize) -> Shape {
    let source = fixture(lines, line_chars);
    let chars = source.chars().count();
    let mut editor = editor(theme, &source);
    let rebuild = one_rebuild(&mut editor);
    Shape {
        name,
        lines,
        line_chars,
        chars,
        rows: editor.visual_len_lines(),
        rebuild,
    }
}

/// One width change, timed. The width must actually differ, or
/// `Editor::set_soft_wrap` returns without rebuilding and this measures the
/// next table instead.
fn one_rebuild(editor: &mut Editor) -> Duration {
    soft_wrap::wrap_to(editor, SCREEN);
    let before = soft_wrap::wrap_width(editor);
    let narrower = Rect {
        width: SCREEN.width - 1,
        ..SCREEN
    };
    let started = Instant::now();
    soft_wrap::wrap_to(editor, narrower);
    let elapsed = started.elapsed();
    assert_ne!(
        soft_wrap::wrap_width(editor),
        before,
        "the width did not change, so no rebuild was timed"
    );
    elapsed
}

/// What `wrap_to` costs when the width has not moved — the call the draw path
/// makes on every frame that is not a resize.
fn idle_calls(theme: &Theme) -> Duration {
    let source = fixture(LADDER[LADDER.len() - 1], LADDER_LINE);
    let mut editor = editor(theme, &source);
    soft_wrap::wrap_to(&mut editor, SCREEN);
    let started = Instant::now();
    for _ in 0..IDLE_CALLS {
        soft_wrap::wrap_to(&mut editor, SCREEN);
    }
    started.elapsed()
}

/// A window drag: one resize per column, each a real rebuild.
fn drag_cost(theme: &Theme) -> Duration {
    let source = fixture(LADDER[LADDER.len() - 1], LADDER_LINE);
    let mut editor = editor(theme, &source);
    soft_wrap::wrap_to(&mut editor, SCREEN);
    let started = Instant::now();
    for width in DRAG.rev() {
        soft_wrap::wrap_to(&mut editor, Rect { width, ..SCREEN });
    }
    started.elapsed()
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// A buffer of `lines` lines of `line_chars` characters.
///
/// Rust line comments, because that is what mockup `8e` wraps — *"long doc
/// comment wraps softly and the continuation row carries no line number"* — and
/// because prose has spaces, which is the branch of `segments` that looks for a
/// break rather than hard-cutting. A word list rather than one repeated word so
/// the break positions are not all the same distance apart.
fn fixture(lines: usize, line_chars: usize) -> String {
    const WORDS: [&str; 8] = [
        "retry", "backoff", "deadline", "resp", "decode", "policy", "elapsed", "budget",
    ];
    let mut out = String::with_capacity(lines * (line_chars + 1));
    for line in 0..lines {
        let mut written = 0;
        out.push_str("// ");
        written += 3;
        let mut word = line;
        while written < line_chars {
            let next = WORDS[word % WORDS.len()];
            // The minified arm asks for a single line far longer than any word
            // list, and a run with no spaces at all is the hard-break branch —
            // so every fourth word is joined to the last rather than spaced.
            let joined = word.is_multiple_of(4);
            if !joined && written > 3 {
                out.push(' ');
                written += 1;
            }
            out.push_str(next);
            written += next.len();
            word += 1;
        }
        out.push('\n');
    }
    out
}

/// An editor configured the way `BufferView` configures one, wrapping off.
///
/// `"rust"` is the only grammar this crate's dev-dependencies enable, and it is
/// the right one for a fixture of `//` comments. Highlighting is setup here and
/// not part of any timed region: the rebuild being measured re-cuts rows and
/// does not re-parse.
fn editor(theme: &Theme, source: &str) -> Editor {
    let mut editor = Editor::new("rust", source, Vec::new()).expect("the rust grammar loads");
    configure_buffer(&mut editor, theme);
    soft_wrap::configure(&mut editor, theme);
    soft_wrap::unwrap(&mut editor);
    editor
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn ladder_table(rungs: &[Rung]) {
    println!(
        "size ladder — one width change, {LADDER_LINE} characters per line, wrapping switched on"
    );
    println!("       lines        chars        rows    ms/rebuild    ns/char    frames");
    for rung in rungs {
        println!(
            "  {:>10}    {:>9}    {:>8}    {:>10.2}    {:>7.1}    {:>6.2}",
            rung.lines,
            rung.chars,
            rung.rows,
            rung.millis(),
            rung.nanos_per_char(),
            rung.frames(),
        );
    }
    println!();
}

fn shape_table(shapes: &[Shape; 3]) {
    println!(
        "line shape — {SHAPE_CHARS} characters, arranged three ways. Cost should follow the \
         characters, not the shape"
    );
    println!(
        "    arrangement              lines    chars/line        rows    ms/rebuild    ns/char"
    );
    for shape in shapes {
        println!(
            "  {:<22}    {:>7}    {:>10}    {:>8}    {:>10.2}    {:>7.1}",
            shape.name,
            shape.lines,
            shape.line_chars,
            shape.rows,
            shape.millis(),
            shape.nanos_per_char(),
        );
    }
    println!();
}

fn idle_table(idle: Duration, ladder: &[Rung]) {
    let per_call = idle.as_secs_f64() * 1e9 / f64::from(IDLE_CALLS);
    let rebuild = ladder.last().expect("the ladder is not empty");
    println!(
        "the unchanged width — what wrap_to costs on a frame that is not a resize ({} lines)",
        rebuild.lines
    );
    println!("    calls        total ms    ns/call    versus a rebuild");
    println!(
        "  {:>7}    {:>12.3}    {:>7.1}    {:>16}",
        IDLE_CALLS,
        idle.as_secs_f64() * 1e3,
        per_call,
        format!(
            "{:.0}x cheaper",
            rebuild.rebuild.as_secs_f64() * 1e9 / per_call.max(f64::MIN_POSITIVE)
        ),
    );
    println!();
}

fn drag_table(drag: Duration) {
    let steps = DRAG.len();
    println!(
        "the drag — a window edge dragged from {} to {} columns, one resize per frame",
        DRAG.end, DRAG.start
    );
    println!("    resizes    total ms    ms/resize    frames per resize");
    println!(
        "  {:>9}    {:>8.1}    {:>9.2}    {:>17.2}",
        steps,
        drag.as_secs_f64() * 1e3,
        drag.as_secs_f64() * 1e3 / steps as f64,
        drag.as_secs_f64() * 1e3 / steps as f64 / FRAME_BUDGET_MS,
    );
    println!();
}

/// Largest over smallest. O(1) in the ladder's parameter gives ~1; the ladder's
/// own climb means the cost tracks it.
fn spread(values: &[f64]) -> f64 {
    let max = values.iter().copied().fold(f64::MIN, f64::max);
    let min = values.iter().copied().fold(f64::MAX, f64::min);
    max / min.max(f64::MIN_POSITIVE)
}

fn verdict(ladder: &[Rung], shapes: &[Shape; 3], idle: Duration, drag: Duration) {
    let per_char: Vec<f64> = ladder.iter().map(Rung::nanos_per_char).collect();
    let size_spread = spread(&per_char);
    let shape_spread = spread(&shapes.iter().map(Shape::nanos_per_char).collect::<Vec<_>>());
    let idle_nanos = idle.as_secs_f64() * 1e9 / f64::from(IDLE_CALLS);
    let last = ladder.last().expect("the ladder is not empty");
    let idle_ratio = last.rebuild.as_secs_f64() * 1e9 / idle_nanos.max(f64::MIN_POSITIVE);

    // The ladder climbs 16x. O(n) leaves ns/char flat and O(n²) puts ~16x into
    // it; 8 is the midpoint on a log scale and the widest a machine's noise has
    // any business being.
    let linear_in_size = size_spread < 8.0;
    // A long line re-slices to its own end once per segment. If that were a
    // copy the minified arm would be quadratic in the line, and 400k characters
    // on one line would not come in within an order of magnitude of 5k lines of
    // 80.
    let linear_in_line_length = shape_spread < 10.0;
    // `soft_wrap::wrap_to`'s header: "calling it every frame is free".
    let idle_is_free = idle_ratio > 100.0;

    // The size at which one resize costs a whole frame, from the measured
    // slope. This is the number to act on, and the one to re-read after any
    // change to `segments`.
    let chars_per_frame = FRAME_BUDGET_MS * 1e6 / per_char[per_char.len() - 1];

    println!("verdict");
    println!(
        "  size          {:.1} ns/char at {} chars, {:.1} at {} ({:.1}x over a 16x climb — {})",
        per_char[0],
        ladder[0].chars,
        per_char[per_char.len() - 1],
        last.chars,
        size_spread,
        if linear_in_size { "LINEAR" } else { "WORSE" },
    );
    println!(
        "  line shape    {:.1}x between {} short lines and one line of {} — {}",
        shape_spread,
        shapes[0].lines,
        shapes[2].line_chars,
        if linear_in_line_length {
            "the cost follows characters, not line shape"
        } else {
            "a long line costs more than its characters — re-slicing is not free"
        },
    );
    println!(
        "  idle call     {idle_nanos:.0} ns against a {:.2} ms rebuild — {idle_ratio:.0}x cheaper. \
         The header's \"free\" holds.",
        last.millis(),
    );
    println!(
        "  the drag      {:.2} ms per resize at {} chars — {:.2} of a frame, each frame of the \
         drag.",
        drag.as_secs_f64() * 1e3 / DRAG.len() as f64,
        last.chars,
        drag.as_secs_f64() * 1e3 / DRAG.len() as f64 / FRAME_BUDGET_MS,
    );
    println!();
    println!("  the number to act on:");
    println!(
        "    one resize costs a whole frame at about {:.0} characters — roughly a {:.0} KiB \
         buffer.",
        chars_per_frame,
        chars_per_frame / 1024.0,
    );
    println!(
        "    Below that a drag is smooth; above it every frame of the drag is late, and §8 calls \
         that"
    );
    println!(
        "    a P0. The fix at that point is not a smaller constant — it is wrapping the viewport \
         and"
    );
    println!(
        "    extending lazily, or moving the rebuild off the draw path. `T081` has neither today."
    );
    println!();
    println!(
        "  B2: {}",
        if linear_in_size && linear_in_line_length && idle_is_free {
            "PASS — the rebuild is linear in characters, indifferent to line shape, and free when \
             the width has not moved"
        } else {
            "FAIL — see the tables above"
        }
    );

    assert!(
        linear_in_size,
        "the rebuild's cost per character climbed {size_spread:.1}x over a 16x growth in buffer \
         size; `apply` walks each line once, so this is superlinear where it should not be"
    );
    assert!(
        linear_in_line_length,
        "{shape_spread:.1}x between arrangements of the same {SHAPE_CHARS} characters — a long \
         line costs more than its characters do. `segments` re-slices from the segment start to \
         the end of the line once per segment; if that slice ever stops being a rope view, this \
         is what says so"
    );
    assert!(
        idle_is_free,
        "wrap_to at an unchanged width cost within {idle_ratio:.0}x of a full rebuild, so it is \
         rebuilding when nothing moved — the draw path calls it every frame on the strength of \
         its header saying it does not"
    );
}
