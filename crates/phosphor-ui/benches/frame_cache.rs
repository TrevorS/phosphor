//! `T079`'s acceptance criterion, as a runnable artifact: **VM invocations per
//! second flat while frames per second climbs, under streaming load.**
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-ui`). It prints two
//! tables and a verdict; `CP-2` reads the verdict.
//!
//! # The experiment
//!
//! A transcript streams at a fixed real rate — [`STREAM_HZ`] chunks a second,
//! which is a fast turn — while the frame loop is driven at a ladder of frame
//! rates over the same [`SECONDS`] of *simulated* time. Each frame draws a full
//! screen: a buffer pane through [`BufferView`], a streaming transcript body
//! through the `spans` hatch, and a statusline composed as a line of chrome
//! nodes with a shed ladder, a spinner and counters.
//!
//! Two arms, differing in one line:
//!
//! * **cached** — [`FrameCache::update`] with the stream's [`Revision`]. The
//!   composer runs on a state change and only then.
//! * **every frame** — the control: compose per frame, which is what a view
//!   tree rebuilt in the frame path costs. This is the arm Q12's risk register
//!   describes as *"a view tree rebuilt per frame instead of per state
//!   change"*.
//!
//! The clock is simulated rather than slept on, so the numbers are exact and
//! the benchmark finishes: `frames/s` is the offered rate, `vm/s` is the
//! measured composer invocations divided by the same simulated seconds. Wall
//! time is reported alongside, per frame, so the real cost of each arm is
//! visible too.
//!
//! # What the composer is, and why the control arm is a *lower* bound
//!
//! The composer here is Rust building the same tree Steel would return. A real
//! `phosphor-steel` invocation is strictly more expensive — it evaluates a
//! `.scm` composition, allocates `SteelVal`s and decodes them through
//! `phosphor_core::value` — so the control arm understates what the cache
//! saves. It cannot overstate it. (This crate cannot call Steel at all:
//! `scripts/lint-no-store-mutation.sh` check 2 allows `phosphor-ui` exactly one
//! `phosphor-*` dependency, and it is `phosphor-core`.)
//!
//! Owned by `spine`.

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::time::{Duration, Instant};

use phosphor_core::query::Revision;
use phosphor_core::request::{BufferId, PaneId, PaneKind};
use phosphor_core::view::{
    Axis, Child, Constraint, Emphasis, Glyph, Millis, Node, Run, SessionState, Slot, SpanRow, Tone,
    Tree,
};
use phosphor_ui::buffer_view::{Editor, StateMark, configure};
use phosphor_ui::frame::FrameCache;
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// Chunks a second. A turn streaming this fast is already an unusually chatty
/// one — the point is that it is a *fixed* rate, independent of the frame rate.
const STREAM_HZ: u64 = 20;

/// Simulated seconds per configuration.
const SECONDS: u64 = 2;

/// The frame-rate ladder. 30 to 960 is a 32× climb; the claim is that `vm/s`
/// does not move across it.
const FRAME_RATES: [u64; 6] = [30, 60, 120, 240, 480, 960];

/// A full screen, and a realistic one: `9c`'s proportions.
const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

/// Rows of transcript held in the streaming pane.
const TRANSCRIPT_ROWS: usize = 24;

fn main() {
    let theme = Theme::phosphor_dark();
    let host = Host::new(&theme);
    let mut buf = Buffer::empty(SCREEN);

    // Revisions are a change counter, so build the ladder once rather than
    // walking `next()` inside the frame loop.
    let revisions: Vec<Revision> = {
        let mut r = Revision::INITIAL;
        let mut all = vec![r];
        for _ in 0..=(STREAM_HZ * SECONDS) {
            r = r.next();
            all.push(r);
        }
        all
    };

    println!("phosphor · T079 frame-cache benchmark");
    println!(
        "  screen {}x{} · stream {STREAM_HZ} chunks/s · {SECONDS}s simulated per row",
        SCREEN.width, SCREEN.height
    );
    println!();

    let mut cached = Vec::new();
    let mut control = Vec::new();
    for fps in FRAME_RATES {
        cached.push(run(Arm::Cached, fps, &revisions, &theme, &host, &mut buf));
        control.push(run(
            Arm::EveryFrame,
            fps,
            &revisions,
            &theme,
            &host,
            &mut buf,
        ));
    }

    table("cached — the tree is composed on a state change", &cached);
    table("control — the tree is composed every frame", &control);
    verdict(&cached, &control);
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Which arm is being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// The frame cache decides.
    Cached,
    /// No cache: compose per frame.
    EveryFrame,
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
struct Measurement {
    fps: u64,
    frames: u64,
    compositions: u64,
    wall: Duration,
    /// Wall time spent *inside the composer* — the part of the frame budget a
    /// Steel invocation would occupy.
    composing: Duration,
}

impl Measurement {
    /// Frames per second — the offered rate, by construction.
    fn frames_per_second(&self) -> f64 {
        self.frames as f64 / SECONDS as f64
    }

    /// **The number `CP-2` reads.** Composer invocations per simulated second.
    fn vm_per_second(&self) -> f64 {
        self.compositions as f64 / SECONDS as f64
    }

    /// What one frame actually cost in wall time, composition included.
    fn micros_per_frame(&self) -> f64 {
        self.wall.as_secs_f64() * 1e6 / self.frames as f64
    }

    /// Microseconds of every simulated second spent composing. **This is the
    /// frame budget the VM occupies**, and the number that scales with how
    /// expensive the composer is — a Steel invocation is strictly more
    /// expensive than the Rust one measured here.
    fn compose_micros_per_second(&self) -> f64 {
        self.composing.as_secs_f64() * 1e6 / SECONDS as f64
    }
}

fn run(
    arm: Arm,
    fps: u64,
    revisions: &[Revision],
    theme: &Theme,
    host: &Host,
    buf: &mut Buffer,
) -> Measurement {
    let frames = fps * SECONDS;
    let mut cache = FrameCache::new();
    let mut compositions = 0u64;
    let mut composing = Duration::ZERO;

    let started = Instant::now();
    for frame in 0..frames {
        // The simulated clock. Everything downstream is a function of it.
        let now_ms = frame * 1000 / fps;
        let chunk = now_ms * STREAM_HZ / 1000;
        let interpreter = Interpreter::new(theme, host).at(Millis(now_ms));

        // The one line the two arms differ by.
        match arm {
            Arm::Cached => {
                if cache.update(revisions[chunk as usize], || {
                    let entered = Instant::now();
                    let tree = compose(chunk);
                    composing += entered.elapsed();
                    tree
                }) {
                    compositions += 1;
                }
                interpreter.render(cache.tree(), SCREEN, buf);
            }
            Arm::EveryFrame => {
                let entered = Instant::now();
                let tree = compose(chunk);
                composing += entered.elapsed();
                compositions += 1;
                interpreter.render(&tree, SCREEN, buf);
            }
        }
    }
    let wall = started.elapsed();

    if arm == Arm::Cached {
        assert_eq!(
            cache.stats().compositions,
            compositions,
            "the cache and this loop must agree on what a composition is"
        );
        assert_eq!(cache.stats().frames(), frames);
    }

    Measurement {
        fps,
        frames,
        compositions,
        wall,
        composing,
    }
}

// ---------------------------------------------------------------------------
// The composition — what Steel returns, built in Rust
// ---------------------------------------------------------------------------

/// One frame's view tree for a transcript that has streamed `chunk` chunks.
///
/// Deliberately allocation-heavy in the same places a decoded Steel tree is:
/// every row is owned `String`s, and the whole tree is rebuilt rather than
/// patched. That is what makes the control arm meaningful.
fn compose(chunk: u64) -> Tree {
    let rows: Vec<SpanRow> = (0..TRANSCRIPT_ROWS)
        .map(|row| {
            let n = chunk.saturating_sub(TRANSCRIPT_ROWS as u64 - 1 - row as u64);
            SpanRow {
                runs: vec![
                    Run::new("✻ ", Tone::Claude),
                    Run::new(&format!("chunk {n} · "), Tone::Meta),
                    Run::new(
                        "reading crates/phosphor-ui/src/interpret.rs to find the seam",
                        Tone::Prose,
                    ),
                ],
                tint: None,
            }
        })
        .collect();

    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(
                Constraint::Fill { weight: 1 },
                Node::split(
                    Axis::Columns,
                    [
                        Slot::new(
                            Constraint::Percent { percent: 50 },
                            Node::Pane {
                                pane: PaneId(1),
                                holds: PaneKind::Buffer,
                                focused: true,
                                child: Child::new(Node::Buffer {
                                    buffer: BufferId(1),
                                    soft_wrap: false,
                                }),
                            },
                        ),
                        Slot::new(
                            Constraint::Fill { weight: 1 },
                            Node::Pane {
                                pane: PaneId(2),
                                holds: PaneKind::Transcript,
                                focused: false,
                                child: Child::new(Node::Spans { rows }),
                            },
                        ),
                    ],
                ),
            ),
            Slot::new(Constraint::Cells { cells: 1 }, statusline(chunk)),
        ],
    ))
}

/// The statusline as a view tree — `T025`'s shape, exercised early because it is
/// the densest bit of chrome in the frame and the one with a shed ladder.
fn statusline(chunk: u64) -> Node {
    let gapped = |node: Node| Node::line([Node::Spacer { cells: 1 }, node]);
    Node::line([
        Node::ModeChip {
            label: "NORMAL".to_owned(),
            tone: Tone::Claude,
        },
        Node::Spacer { cells: 1 },
        Node::FileLabel {
            path: "crates/phosphor-ui/src/interpret.rs".into(),
            dirty: true,
        },
        Node::Spring {},
        Node::Session {
            state: SessionState::Working,
            since: Some(Millis(0)),
            prose: true,
        },
        Node::Shed {
            priority: 0,
            contracted: Some(Child::new(gapped(Node::Counter {
                glyph: Glyph::Unseen,
                count: (chunk % 9) as u32 + 1,
                label: None,
                tone: Tone::Meta,
            }))),
            child: Child::new(gapped(Node::Counter {
                glyph: Glyph::Unseen,
                count: (chunk % 9) as u32 + 1,
                label: Some("unseen".to_owned()),
                tone: Tone::Meta,
            })),
        },
        Node::Shed {
            priority: 1,
            contracted: None,
            child: Child::new(gapped(Node::Label {
                text: "jj ✓".to_owned(),
                tone: Tone::Meta,
                emphasis: Emphasis::Plain,
            })),
        },
    ])
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// One real editor behind [`BufferId`] 1, so the buffer pane costs what a
/// buffer pane costs — tree-sitter highlighting and all.
struct Host {
    editor: Editor,
    marks: Vec<StateMark>,
}

/// `Editor` is not `Debug`, and `Resources` requires it — the same reason
/// `BufferView` writes one by hand.
impl core::fmt::Debug for Host {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Host")
            .field("marks", &self.marks.len())
            .finish()
    }
}

impl Host {
    fn new(theme: &Theme) -> Self {
        let source = include_str!("../src/interpret.rs");
        let mut editor = Editor::new("rust", source, Vec::new()).expect("the rust grammar loads");
        configure(&mut editor, theme);
        let marks = (0..SCREEN.height as usize)
            .map(|row| match row % 7 {
                0 => StateMark::ClaudeUnseen,
                3 => StateMark::Attention,
                5 => StateMark::Trouble,
                _ => StateMark::None,
            })
            .collect();
        Self { editor, marks }
    }
}

impl Resources for Host {
    fn editor(&self, buffer: BufferId) -> Option<&Editor> {
        (buffer == BufferId(1)).then_some(&self.editor)
    }

    fn state_marks(&self, _buffer: BufferId) -> &[StateMark] {
        &self.marks
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn table(title: &str, rows: &[Measurement]) {
    println!("{title}");
    println!(
        "    fps    frames    compositions       vm/s      frames/s     µs/frame    compose µs/s"
    );
    for m in rows {
        println!(
            "  {:>5}    {:>6}    {:>12}    {:>7.1}    {:>10.1}    {:>9.1}    {:>12.1}",
            m.fps,
            m.frames,
            m.compositions,
            m.vm_per_second(),
            m.frames_per_second(),
            m.micros_per_frame(),
            m.compose_micros_per_second(),
        );
    }
    println!();
}

fn verdict(cached: &[Measurement], control: &[Measurement]) {
    let vm_rates: Vec<f64> = cached.iter().map(Measurement::vm_per_second).collect();
    let flat = vm_rates
        .iter()
        .all(|rate| (rate - vm_rates[0]).abs() < f64::EPSILON);
    let climbed = cached
        .windows(2)
        .all(|pair| pair[1].frames_per_second() > pair[0].frames_per_second());

    let first = cached.first().expect("the ladder is not empty");
    let last = cached.last().expect("the ladder is not empty");
    let control_last = control.last().expect("the ladder is not empty");

    println!("verdict");
    println!(
        "  frames/s        {:.0} → {:.0}   ({:.0}× climb)",
        first.frames_per_second(),
        last.frames_per_second(),
        last.frames_per_second() / first.frames_per_second(),
    );
    println!(
        "  vm/s cached     {:.1} → {:.1}   ({})",
        vm_rates[0],
        vm_rates[vm_rates.len() - 1],
        if flat { "FLAT" } else { "MOVED" },
    );
    println!(
        "  vm/s control    {:.1} → {:.1}   (tracks the frame rate, as it must)",
        control[0].vm_per_second(),
        control_last.vm_per_second(),
    );
    println!(
        "  at {} fps the cache costs {:.1} VM invocations/s instead of {:.1} — {:.0}× fewer",
        last.fps,
        last.vm_per_second(),
        control_last.vm_per_second(),
        control_last.vm_per_second() / last.vm_per_second(),
    );
    println!(
        "  wall time at {} fps: {:.1} µs/frame cached vs {:.1} µs/frame composing every frame",
        last.fps,
        last.micros_per_frame(),
        control_last.micros_per_frame(),
    );
    println!(
        "  frame budget spent composing at {} fps: {:.0} µs/s cached vs {:.0} µs/s uncached",
        last.fps,
        last.compose_micros_per_second(),
        control_last.compose_micros_per_second(),
    );
    println!("  (the composer here is Rust building the tree Steel would return; a real Steel");
    println!(
        "   invocation is strictly more expensive, so the last line is a floor, not a ceiling)"
    );
    println!();
    println!(
        "  T079: {}",
        if flat && climbed {
            "PASS — VM invocations per second flat while frames per second climbs"
        } else {
            "FAIL — see the tables above"
        }
    );

    assert!(
        flat && climbed,
        "T079's acceptance criterion did not hold; the tables above say how"
    );
}
