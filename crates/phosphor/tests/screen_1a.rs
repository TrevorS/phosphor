//! `T041`/`T087` — screen `1a` as a **Tier-1 golden frame**.
//!
//! `1a` is the product's front page: a file open, the state column pulling your
//! eye to what Claude wrote and you have not read, and the statusline counting
//! it. `CP-5` names it first among the five snapshots it verifies, and *"the
//! markers don't change how you read the file"* is that checkpoint's stated
//! failure condition — so this is the frame with the most riding on it.
//!
//! # Why there was no Tier-1 frame for it until now
//!
//! There are two `1a` captures at Tier 2 and neither could stand in. `1a.png`
//! opens `tapes/fixtures/core-lib.rs` against an **empty store**, so the one
//! screen the whole product is about was photographed with nothing behind it;
//! `1a-seeded.png` fixes that and is still a picture, which cannot see a
//! palette regression and does not run on a pull request.
//!
//! # What is live here
//!
//! The gutter is not a fixture. The marks come from a real
//! [`phosphor_core::store::Store`], through the *same two calls the frame loop
//! makes* — [`gutter::spans`] to turn store spans into visual rows, then
//! [`gutter::state_column`] to fold them into one column by §3's ladder. The
//! statusline is `runtime/statusline.scm`'s, composed by the editor layer.
//!
//! **The seen region is handed over with the unseen ones**, which is `main.rs`'s
//! own rule and worth restating because it is the half a test would otherwise
//! skip: `RegionState::Seen` resolves to `StateMark::None` — §3's *"seen —
//! marker cleared, line is plain"* — so the ladder is what decides it draws
//! nothing, in the one place that decides it. A frame built from unseen regions
//! only would draw the same pixels and prove less.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see `screen_3c.rs`'s module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::request::{Actor, BufferId, Position, RegionSpec, Span};
use phosphor_core::store::{Scope, SeenState, Store};
use phosphor_core::view::{Axis, Constraint, Millis, Node, SessionState, Slot, Tree};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, StatusFile, StatusVm};
use phosphor_ui::buffer_view::{self, Editor, ScrollRequest, StateMark, apply_scroll, editor_area};
use phosphor_ui::gutter::{self, RegionState};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// The file `1a` draws, and the one the worked example is written about.
const PATH: &str = "src/retry.rs";

/// `fixtures/seed/plan.scm`'s three `src/retry.rs` regions, with the state the
/// plan leaves each in.
///
/// `6-10` is the one `mark-seen!` clears — the middle of three, deliberately,
/// so the frame shows a cleared marker *between* two live ones rather than at
/// an edge where an off-by-one would hide.
const REGIONS: &[(u32, u32, SeenState)] = &[
    (4, 4, SeenState::Unseen),
    (6, 10, SeenState::Seen),
    (12, 24, SeenState::Unseen),
];

fn span(from: u32, to: u32) -> Span {
    Span {
        start: Position {
            line: from,
            column: 1,
        },
        end: Position {
            line: to,
            column: 1,
        },
    }
}

/// The store, seeded with the turn the fixture describes.
fn seeded_store() -> Store {
    let mut store = Store::new();
    let specs: Vec<RegionSpec> = REGIONS
        .iter()
        .map(|(from, to, _)| RegionSpec {
            path: PathBuf::from(PATH),
            span: span(*from, *to),
            author: Actor::Claude,
        })
        .collect();
    store.declare_regions(&specs, Actor::Claude);
    for (from, to, state) in REGIONS {
        if *state == SeenState::Seen {
            store.set_seen(
                &Scope::Span {
                    path: PathBuf::from(PATH),
                    span: span(*from, *to),
                },
                SeenState::Seen,
            );
        }
    }
    store
}

/// `1a`'s buffer and its state column.
struct Screen {
    editor: Editor,
    marks: Vec<StateMark>,
}

impl core::fmt::Debug for Screen {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Screen").finish_non_exhaustive()
    }
}

impl Resources for Screen {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        Some(&self.editor)
    }

    fn state_marks(&self, _buffer: BufferId) -> &[StateMark] {
        &self.marks
    }
}

fn runtime_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// The shipped editor layer, booted clean.
///
/// [`Detached`] and not a store-backed host, because nothing on this screen is
/// composed from a query — the statusline takes its counts as a ViewModel and
/// the gutter is built in Rust from the store directly, which is exactly what
/// `main.rs` does. `screen_pickers.rs` needs the other kind because a picker
/// source *is* a query.
fn layer() -> Runtime {
    let runtime = Runtime::boot(Some(&runtime_tree()), Arc::new(Detached) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    runtime
}

/// `fixtures/src/retry.rs`, which is the worked example transcribed byte for
/// byte — see `fixtures/README.md`.
fn retry_rs() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("src")
            .join("retry.rs"),
    )
    .expect("fixtures/src/retry.rs is committed")
}

/// The state column, built the way the frame loop builds it.
///
/// Two calls, in the order `main.rs` makes them, and neither is reimplemented
/// here: a test that mapped lines to rows itself would pass on a build whose
/// row mapping was wrong, which is the one thing this column can get wrong that
/// the store cannot.
fn state_column(store: &Store, editor: &Editor) -> Vec<StateMark> {
    let spans: Vec<(Span, RegionState)> = store
        .regions()
        .in_scope(&Scope::File(PathBuf::from(PATH)))
        .map(|region| {
            (
                region.span,
                match region.state {
                    SeenState::Unseen => RegionState::Unseen,
                    SeenState::Seen => RegionState::Seen,
                },
            )
        })
        .collect();
    assert_eq!(
        spans.len(),
        REGIONS.len(),
        "every region reached the gutter"
    );

    let rows = gutter::spans(editor, &spans);
    let deepest = rows.iter().map(|row| row.rows.end).max().unwrap_or(0);
    gutter::state_column(&rows, deepest)
}

/// `1a`'s statusline: the file, and the two regions still unread.
///
/// `2` and not `3`: `6-10` is seen. The number is written here because a
/// `StatusVm` is what the loop hands the layer, and
/// [`the_statusline_count_is_the_stores_own`] is what keeps it honest.
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: None,
        file: Some(StatusFile {
            path: PathBuf::from(PATH),
            dirty: false,
        }),
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        inbox_unread: 0,
        unseen: 2,
        trouble: 0,
        attention: 0,
        vcs: None,
        server: None,
        cursor: None,
        hints: Vec::new(),
    }
}

/// The screen: the buffer, and the statusline under it.
fn tree(runtime: &mut Runtime) -> Tree {
    let status = status::compose(runtime, &status_vm()).expect("runtime/statusline.scm composes");
    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(
                Constraint::Fill { weight: 1 },
                Node::Buffer {
                    buffer: BufferId(1),
                    soft_wrap: false,
                },
            ),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
}

/// What this frame is missing, and which task owns each absence.
///
/// `T018`'s rule: *"nobody has to reverse-engineer an absence."* Every line was
/// checked against the tree in the session that wrote it.
const NOTES: &[&str] = &[
    "The state column is the store's, folded by gutter::spans and then",
    "  gutter::state_column — the same two calls, in the same order, that the",
    "  frame loop makes. Three regions go in; the middle one is seen and",
    "  resolves to StateMark::None by §3's ladder, so it draws ground.",
    "1a's mockup draws a row *tint* behind an unseen region as well as the",
    "  marker. That is T087, and it goes through the fork's marks API onto the",
    "  Editor rather than through the view tree — a golden frame renders the",
    "  tree, so the tint is Tier 2's to show (tapes/1a-seeded.png) and this",
    "  frame commits the marker column only.",
    "No session segment and no server chip: both are S6 (T050) and 1a's",
    "  statusline draws neither until there is a session to describe.",
    "No diagnostics. 1a is the quiet screen — 2b is where `\\u{25a0} N` appears,",
    "  and a_diagnostic_outranks_an_unseen_region_on_the_same_row in loop_pty",
    "  is where the two states meet on one row.",
];

/// Renders `1a` at `width`.
fn screen(runtime: &mut Runtime, theme: &Theme, width: u16) -> Buffer {
    let area = Rect::new(0, 0, width, 24);
    let mut editor = Editor::new("rust", &retry_rs(), Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    apply_scroll(
        &mut editor,
        ScrollRequest::ToRow { row: 1 },
        editor_area(area),
    );

    let marks = state_column(&seeded_store(), &editor);
    let resources = Screen { editor, marks };
    let tree = tree(runtime);

    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, &resources).render(&tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`1a` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// Commits one width as a golden frame.
fn golden(name: &'static str, width: u16) {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let buf = screen(&mut runtime, &theme, width);
    let frame = Frame {
        screen: name,
        theme_label: "phosphor-dark",
        theme: &theme,
        notes: NOTES,
    };
    // §12, and the half no grep-based lint can reach: a colour on screen that
    // is not a `Theme` field. `CP-1` asserts this on every golden frame.
    assert!(
        frame.unnamed(&buf).is_empty(),
        "colours on screen that no Theme field names: {:?}",
        frame.unnamed(&buf)
    );
    insta::assert_snapshot!(name, frame.to_text(&buf));
}

/// The screen, at the width the `CP-1` golden frames use.
#[test]
fn screen_1a_draws() {
    golden("1a", 120);
}

/// The same screen at 80 columns. §11 is *"narrow terminals drop, never
/// squeeze"* and the state column is the one thing that never drops — it is
/// the product.
#[test]
fn screen_1a_draws_at_80_columns() {
    golden("1a-80", 80);
}

// ---------------------------------------------------------------------------
// The claims the frame stands on
// ---------------------------------------------------------------------------

/// The `2` on the statusline is the store's answer, not a number this file
/// chose.
///
/// A golden frame commits whatever it was handed; this is what makes being
/// handed the wrong thing a red test rather than a blessed picture.
#[test]
fn the_statusline_count_is_the_stores_own() {
    let store = seeded_store();
    assert_eq!(
        store.answer_unseen(Some(Path::new(PATH))).len(),
        usize::try_from(status_vm().unseen).expect("a small count"),
        "the statusline says what `unseen-regions` would answer",
    );
}

/// §3's ladder, read off the column this frame draws: the seen region's rows
/// are ground, and the two unseen ones are not.
///
/// This is the assertion `CP-5`'s failure condition turns on — *"the markers
/// don't change how you read the file"* — expressed as the smallest thing that
/// can be checked without a person looking.
#[test]
fn the_seen_region_draws_nothing_and_the_unseen_ones_draw_a_marker() {
    let mut editor = Editor::new("rust", &retry_rs(), Vec::new()).expect("rust editor");
    let theme = Theme::phosphor_dark();
    buffer_view::configure(&mut editor, &theme);
    let column = state_column(&seeded_store(), &editor);

    // 1-based store lines to 0-based visual rows, for a file that does not
    // wrap: the fixture's lines are all shorter than 80 columns.
    let at = |line: u32| column[usize::try_from(line).expect("small") - 1];

    assert_eq!(at(4), StateMark::ClaudeUnseen, "the first region is unread");
    for line in 6..=10 {
        assert_eq!(
            at(line),
            StateMark::None,
            "line {line} is inside the region that was marked seen",
        );
    }
    // `12..24`, exclusive at the top: a region span is **half-open**
    // (`region.rs::overlap_is_half_open_except_for_a_point`), so the
    // declaration `(12,1)-(24,1)` covers lines 12 through 23 and line 24 is
    // outside it. The column is 23 rows long for exactly that reason, and this
    // loop reading `..=24` is what found it.
    for line in 12..24 {
        assert_eq!(
            at(line),
            StateMark::ClaudeUnseen,
            "line {line} is still unread",
        );
    }
    assert_eq!(
        column.len(),
        23,
        "the column stops at the last row a region reaches, and the top of a \
         half-open span is not one of them",
    );
}
