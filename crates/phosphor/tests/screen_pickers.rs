//! `T045`–`T047` — screens `2a`, `3d` and `8a` as **Tier-1 golden frames**.
//!
//! `CP-5` names *"`1a`, `2a`, `3d`, `8a`, `6a` snapshots"* among the things
//! Claude verifies, and until this file three of those five were covered by
//! VHS and by nothing at Tier 1. Tier 1 is the committed cell grid — *"what we
//! told the terminal to draw. Exact, diffable, fast"* — and it is the only tier
//! that gates CI (`TASKS.md`'s tier table). Tier 2 photographs these screens
//! and is a change *detector*; it cannot see a palette regression and it does
//! not run on a pull request.
//!
//! # Why three screens share one file
//!
//! Because they are one screen. `runtime/pickers.scm`'s own header:
//!
//! > nothing in rust knows what a row *means* — which is why `2a`, `3d` and
//! > `8a` are one widget with three sources rather than three screens.
//!
//! Splitting them into three files would put that sentence's opposite into the
//! layout of the test suite. The snapshot *names* stay canonical — `2a`, `3d`,
//! `8a` and their 80-column variants — so a reader looking for one screen finds
//! its frame under the name the mockup uses.
//!
//! # Everything in these rows is live
//!
//! No row here is a fixture. Each one is what `runtime/pickers.scm` answered
//! when called through [`phosphor_steel::picker::rows`] — the same function
//! `AppHost` calls on the product path — against a real
//! [`phosphor_core::store::Store`] seeded with `fixtures/seed/plan.scm`'s own
//! spans. A change to the Scheme moves these frames, which is the property
//! that makes them worth committing.
//!
//! # Why it lives in the binary crate
//!
//! Same reason as `screen_6b.rs` and `screen_3c.rs`: a frame composed from
//! Steel and drawn by the interpreter needs `phosphor-steel` **and**
//! `phosphor-ui` at once, and `phosphor-ui` may not have the first —
//! `scripts/lint-no-store-mutation.sh` check 2 allows it exactly one
//! `phosphor-*` dependency. The serialiser is `T018`'s, included by path rather
//! than copied, so these frames diff against the `CP-1` frames in the same
//! alphabet.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see the module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use frame_grid::Frame;
use phosphor_core::action::{Action, Outcome, PickerAction, Receipt, Refusal, Request};
use phosphor_core::query::{Answer, Answers, Query, QueryError, RegionQuery};
use phosphor_core::request::{Actor, Position, RegionSpec, SourceId, Span};
use phosphor_core::store::{SeenState, Store};
use phosphor_core::value::{Args, Value};
use phosphor_core::view::{
    Axis, Child, Constraint, Float, FloatHeader, Millis, Mood, Node, SessionState, Slot, SpanRow,
    Tree,
};
use phosphor_steel::host::Host;
use phosphor_steel::picker;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, StatusFile, StatusVm};
use phosphor_ui::buffer_view::{self, Editor, ScrollRequest, StateMark, apply_scroll, editor_area};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::picker::{PickerVm, RowVm, RunVm};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

// ---------------------------------------------------------------------------
// The store behind the sources
// ---------------------------------------------------------------------------

/// `fixtures/seed/plan.scm`'s `declare-regions!` call, as spans.
///
/// **Restated here rather than run**, and the reason is the same one
/// `fixtures/README.md` gives for the residue list: `scripts/seed-fixtures.sh`
/// drives `phosphor --eval` in a subprocess against an `XDG_STATE_HOME`, which
/// is a shell script and a built binary — neither of which a unit test may
/// depend on. What is asserted below is that these are the plan's numbers, and
/// `the_seeded_spans_are_the_plans_own` is the test that keeps them so.
const DECLARED: &[(&str, u32, u32)] = &[
    ("src/retry.rs", 4, 4),
    ("src/retry.rs", 6, 10),
    ("src/retry.rs", 12, 24),
    ("src/fetch.rs", 10, 14),
    ("src/fetch.rs", 17, 20),
    ("src/fetch.rs", 31, 35),
    ("src/deploy", 18, 20),
];

/// The two the plan marks seen — `retry.rs:6-10` and `fetch.rs:10-14`.
const SEEN: &[(&str, u32, u32)] = &[("src/retry.rs", 6, 10), ("src/fetch.rs", 10, 14)];

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

/// A store holding the seeded turn: seven regions, two of them read.
fn seeded_store() -> Store {
    let mut store = Store::new();
    let specs: Vec<RegionSpec> = DECLARED
        .iter()
        .map(|(path, from, to)| RegionSpec {
            path: PathBuf::from(path),
            span: span(*from, *to),
            author: Actor::Claude,
        })
        .collect();
    store.declare_regions(&specs, Actor::Claude);
    for (path, from, to) in SEEN {
        store.set_seen(
            &phosphor_core::store::Scope::Span {
                path: PathBuf::from(path),
                span: span(*from, *to),
            },
            SeenState::Seen,
        );
    }
    store
}

/// A host that can answer what a picker source asks, record what one is, and
/// refuse everything else.
///
/// # One query
///
/// And that is not a shortcut — it is the whole store surface
/// `runtime/pickers.scm` touches. `unseen` maps `unseen-regions`, `files` and
/// `grep` each fold it into a lookup table, and `references` reads its places
/// out of `args`. A host that answered more would be answering questions no
/// source asks.
///
/// # One Action, and finding out which cost a red test
///
/// `define-picker-source!` is a **mutation**, so a host that refused every
/// Action refused the six `define-picker-source!` calls in `pickers.scm` — and
/// the boot report still came back clean, because a refusal is a legible answer
/// rather than a fault. The layer booted, no source existed, and the failure
/// arrived four calls later as *"no picker source `unseen`"*.
///
/// It is recorded here because the shape generalises: **a refusing host is not
/// a neutral one.** `Detached` refuses everything and `runtime/init.scm` runs
/// end to end against it precisely because refusals are ordinary — which means
/// a boot that is *clean* says nothing about whether the layer's definitions
/// took effect.
///
/// What this does instead is exactly what the loop does with the same Action:
/// `AppHost` pushes an `Intent::DefineSource` and `main.rs` turns it into
/// [`picker::define_form`] evaluated in the VM. [`Self::install`] is that
/// second half, and it is why the sources here are the shipped ones rather
/// than bodies this test wrote.
#[derive(Debug)]
struct Seeded {
    store: Mutex<Store>,
    /// `(id, body)` for every `define-picker-source!` the layer asked for, in
    /// the order it asked.
    sources: Mutex<Vec<(String, String)>>,
}

impl Seeded {
    fn new() -> Self {
        Self {
            store: Mutex::new(seeded_store()),
            sources: Mutex::new(Vec::new()),
        }
    }

    /// Binds every recorded source into `runtime`, the way the loop binds one.
    fn install(&self, runtime: &mut Runtime) {
        let sources = self.sources.lock().expect("no other thread holds this");
        assert!(
            !sources.is_empty(),
            "runtime/pickers.scm defines at least one source; the layer asked for none"
        );
        for (id, body) in sources.iter() {
            let outcome = runtime.evaluate(&picker::define_form(id, body));
            assert!(
                matches!(outcome, Outcome::Done(_)),
                "binding the `{id}` source: {outcome:?}"
            );
        }
    }
}

impl Answers for Seeded {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        let store = self
            .store
            .lock()
            .map_err(|_| QueryError::NotYetImplemented {
                task: query.spec().since.task,
            })?;
        match query {
            Query::Region(RegionQuery::UnseenRegions { path }) => Ok(Answer {
                value: Value::List(store.answer_unseen(path.as_deref())),
                revision: store.revision(),
            }),
            other => Err(QueryError::NotYetImplemented {
                task: other.spec().since.task,
            }),
        }
    }
}

impl Host for Seeded {
    fn apply(&self, request: &Request) -> Outcome {
        match &request.action {
            Action::Picker(PickerAction::DefinePickerSource { source, body }) => {
                if let Ok(mut sources) = self.sources.lock() {
                    sources.push((source.0.clone(), body.clone()));
                }
                Outcome::Done(Receipt {
                    capability: request.action.spec().name,
                    value: Value::Null,
                    note: None,
                })
            }
            other => Outcome::Refused(Refusal::NotYetImplemented {
                task: other.spec().since.task,
            }),
        }
    }
}

fn runtime_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// The shipped editor layer, booted clean over the seeded store, with its
/// picker sources bound.
fn layer() -> Runtime {
    let host = Arc::new(Seeded::new());
    let mut runtime = Runtime::boot(Some(&runtime_tree()), host.clone() as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    host.install(&mut runtime);
    runtime
}

// ---------------------------------------------------------------------------
// Rows, through the door the binary uses
// ---------------------------------------------------------------------------

/// A `SpanRow` as a row the widget draws.
///
/// **A copy of `crates/phosphor/src/picker.rs`'s `row_of`, and the duplication
/// is recorded rather than hidden.** That function is `pub(crate)` in a crate
/// with no library target, so an integration test cannot call it; including the
/// module by path is not available either, because it reaches `nucleo` and the
/// event queue. Three lines is the smallest honest answer, and
/// `the_conversion_still_drops_what_row_of_drops` below is what keeps this from
/// drifting into a second opinion about what a row is.
fn row_of(span: &SpanRow) -> RowVm {
    RowVm::new(
        span.runs
            .iter()
            .map(|run| RunVm::text(run.text.clone()).toned(run.tone))
            .collect(),
    )
}

/// Calls a source and converts what it answered.
fn rows_of(runtime: &mut Runtime, source: &str, args: &Value) -> Vec<RowVm> {
    let answered = picker::rows(runtime, source, args).unwrap_or_else(|why| {
        panic!("runtime/pickers.scm's `{source}` source answers rows: {why}")
    });
    assert!(
        !answered.is_empty(),
        "`{source}` answered no rows at all, so this frame would prove nothing"
    );
    answered.iter().map(row_of).collect()
}

/// The picker's ViewModel: every row matched, the first selected.
///
/// `matching: false` because nothing here runs nucleo — a golden frame is a
/// picture of a settled list, and `T045`'s *"still working"* state is asserted
/// by `picker.rs`'s own tests where it can be observed rather than posed.
fn picker_vm(rows: Vec<RowVm>, preview: Vec<String>) -> PickerVm {
    let total = rows.len();
    PickerVm {
        rows,
        selected: 0,
        preview,
        total,
        matching: false,
    }
}

// ---------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------

/// The buffer behind the float, and the picker in front of it.
struct Screen {
    editor: Editor,
    source: SourceId,
    picker: PickerVm,
}

impl core::fmt::Debug for Screen {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Screen").finish_non_exhaustive()
    }
}

impl Resources for Screen {
    fn editor(&self, _buffer: phosphor_core::request::BufferId) -> Option<&Editor> {
        Some(&self.editor)
    }

    fn state_marks(&self, _buffer: phosphor_core::request::BufferId) -> &[StateMark] {
        // The code behind a picker is dimmed by §9 and the gutter is not what
        // these three screens are about. `1a` is the frame that draws it.
        &[]
    }

    fn picker(&self, source: &SourceId) -> Option<&PickerVm> {
        (source == &self.source).then_some(&self.picker)
    }
}

/// `2a`'s statusline: the file, and the four regions still unread.
fn status_vm(unseen: u32) -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: None,
        file: Some(StatusFile {
            path: PathBuf::from("src/retry.rs"),
            dirty: false,
        }),
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        threads: 0,
        inbox_unread: 0,
        disk_changed: false,
        unseen,
        trouble: 0,
        attention: 0,
        vcs: None,
        server: None,
        cursor: None,
        hints: Vec::new(),
    }
}

/// The tree: code, the statusline, and the picker float over both.
///
/// **The same shape the binary composes** — `main.rs`'s picker arm is
/// `Tree::new(Node::Empty {}).with_float(picker_float(session))` over a buffer
/// the widgets already painted, and the root here carries that buffer instead
/// of leaving it to a previous pass. One tree rather than two passes, same
/// cells: §9's dim is applied by the float, not by the root.
fn tree(runtime: &mut Runtime, source: &SourceId, unseen: u32, preview: bool) -> Tree {
    let status = status::compose(runtime, &status_vm(unseen)).expect("statusline.scm composes");
    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(
                Constraint::Fill { weight: 1 },
                Node::Buffer {
                    buffer: phosphor_core::request::BufferId(1),
                    soft_wrap: false,
                },
            ),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
    .with_float(Float {
        mood: Mood::Informational,
        header: Some(FloatHeader::new(&source.0)),
        body: Child::new(Node::Picker {
            source: source.clone(),
            filter: String::new(),
            // Empty, and deliberately: `main.rs`'s `picker_float` says why —
            // a source supplies styled *runs*, so column widths are the
            // source's own layout decision.
            columns: Vec::new(),
            preview,
        }),
        footer: None,
    })
}

/// `1a`'s file, which is the buffer every one of these floats sits over.
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

/// Renders one screen at `width`.
fn render(
    runtime: &mut Runtime,
    theme: &Theme,
    source: &SourceId,
    picker: PickerVm,
    preview: bool,
    width: u16,
) -> Buffer {
    let area = Rect::new(0, 0, width, 24);
    let mut editor = Editor::new("rust", &retry_rs(), Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    apply_scroll(
        &mut editor,
        ScrollRequest::ToRow { row: 1 },
        editor_area(area),
    );

    let resources = Screen {
        editor,
        source: source.clone(),
        picker,
    };
    let tree = tree(runtime, source, 4, preview);
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, &resources).render(&tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "this screen needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// What these frames are missing, and which task owns each absence.
///
/// Goes into the `.snap` itself, on `T018`'s rule: *"nobody has to
/// reverse-engineer an absence."* Every line was checked against the tree in
/// the session that wrote it.
const NOTES: &[&str] = &[
    "Every row is what runtime/pickers.scm answered through",
    "  phosphor_steel::picker::rows against a real Store — not a fixture. A",
    "  change to the Scheme moves these frames.",
    "The rows are unfiltered and unmatched: nucleo owns a thread pool and lives",
    "  in the binary, so a golden frame poses the settled list rather than",
    "  running the matcher. RunVm::matched is therefore false on every run,",
    "  which is what picker.rs's row_of already records as T047's to spend.",
    "2a draws NO preview pane at 120 columns, and that is a finding rather",
    "  than a composition choice: PREVIEW_AT is 100 and is checked against the",
    "  float's *body*, which §8 caps at 80% of the screen less two columns of",
    "  border — 94 at 120 cols. T045's number is written in terminal columns",
    "  and spent in body columns. OPEN-QUESTIONS.md §45; the test that pins it",
    "  is the_preview_pane_is_shed_at_120_columns_....",
    "  What 2a's mockup draws there is a *diff* preview, which is T063 and does",
    "  not exist either — so both halves of that pane are owed.",
    "3d's rows carry `\\u{25cf}N unseen` from the store and no `activity` column:",
    "  the mockup's second annotation is a session fact (T050) and nothing in",
    "  the vocabulary answers it yet.",
    "8a greps the *open buffer*, not the workspace — pickers.scm states the",
    "  limit and its reason (no capability searches files on disk).",
    "8a's row is the WHOLE matched line (`src/retry.rs:4  \\u{25cf}  use",
    "  util::jitter;`). The mockup at TUI Mockups.dc.html:165 draws the matched",
    "  *fragment* instead (`\\u{25cf} .min(policy.max_delay)`), which is one half",
    "  of OPEN-QUESTIONS.md §12 — and this frame is the build's answer to it,",
    "  arrived at by pickers.scm rather than by a ruling. Flagged, not folded.",
    "No footer: main.rs's picker_float composes none, and §4's `every legal key",
    "  always visible` is owed by T045's own row rather than by this frame.",
];

/// Commits one screen at one width.
fn golden(name: &'static str, source: &str, args: &Value, preview: bool, width: u16) {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let id = SourceId(source.to_owned());
    let rows = rows_of(&mut runtime, source, args);
    let vm = picker_vm(rows, if preview { preview_lines() } else { Vec::new() });
    let buf = render(&mut runtime, &theme, &id, vm, preview, width);

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

/// `2a`'s preview pane: the selected region's own lines out of the fixture.
fn preview_lines() -> Vec<String> {
    retry_rs()
        .lines()
        .skip(3)
        .take(6)
        .map(str::to_owned)
        .collect()
}

/// No source reads `args` for `unseen` — it is `(unseen-regions)` and nothing
/// else — but the calling convention still passes a hash.
fn no_args() -> Value {
    Value::Record(Args::new())
}

/// `3d`'s file list. Rust walks the workspace; this is what it would hand down.
///
/// Deliberately holds files the store knows nothing about — `Cargo.toml`,
/// `src/main.rs` — because that is `3d`'s own correction, recorded in
/// `pickers.scm`: *"the store is the annotation, never the filter."*
fn files_args() -> Value {
    Value::Record(
        Args::new().with(
            "files",
            Value::List(
                [
                    "Cargo.toml",
                    "src/deploy",
                    "src/fetch.rs",
                    "src/main.rs",
                    "src/retry.rs",
                ]
                .into_iter()
                .map(|path| Value::Text(path.to_owned()))
                .collect(),
            ),
        ),
    )
}

/// `8a`'s arguments: the open buffer's path and its lines.
fn grep_args() -> Value {
    Value::Record(
        Args::new()
            .with("path", Value::Text("src/retry.rs".to_owned()))
            .with(
                "lines",
                Value::List(
                    retry_rs()
                        .lines()
                        .map(|line| Value::Text(line.to_owned()))
                        .collect(),
                ),
            ),
    )
}

// ---------------------------------------------------------------------------
// The frames
// ---------------------------------------------------------------------------

/// `2a` — the unseen picker, with the preview pane open.
#[test]
fn screen_2a_draws() {
    golden("2a", "unseen", &no_args(), true, 120);
}

/// The same screen at 80 columns, where §11 sheds rather than wraps.
#[test]
fn screen_2a_draws_at_80_columns() {
    golden("2a-80", "unseen", &no_args(), true, 80);
}

/// `3d` — the files picker, carrying agent state rather than just names.
#[test]
fn screen_3d_draws() {
    golden("3d", "files", &files_args(), false, 120);
}

#[test]
fn screen_3d_draws_at_80_columns() {
    golden("3d-80", "files", &files_args(), false, 80);
}

/// `8a` — grep over the open buffer, rows knowing who touched them.
#[test]
fn screen_8a_draws() {
    golden("8a", "grep", &grep_args(), false, 120);
}

#[test]
fn screen_8a_draws_at_80_columns() {
    golden("8a-80", "grep", &grep_args(), false, 80);
}

// ---------------------------------------------------------------------------
// The claims the frames stand on
// ---------------------------------------------------------------------------

/// The spans above are `fixtures/seed/plan.scm`'s, and this is what says so.
///
/// A golden frame whose store drifted from the fixture would still be a valid
/// picture of *something* — which is exactly the failure a committed frame
/// cannot report on its own.
#[test]
fn the_seeded_spans_are_the_plans_own() {
    let plan = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("seed")
            .join("plan.scm"),
    )
    .expect("fixtures/seed/plan.scm is committed");

    for (path, from, to) in DECLARED {
        let spelled = format!(
            "\"path\" \"{path}\" \"span\" (hash \"start\" (hash \"line\" {from} \"column\" 1) \
             \"end\" (hash \"line\" {to} \"column\""
        );
        assert!(
            plan.contains(&spelled),
            "plan.scm no longer declares {path}:{from}-{to} — the store behind \
             these frames has drifted from the fixture"
        );
    }
    for (path, from, to) in SEEN {
        let spelled = format!(
            "(mark-seen! (hash \"kind\" \"explicit\" \"path\" \"{path}\" \"span\" \
             (hash \"start\" (hash \"line\" {from} \"column\" 1) \"end\" (hash \"line\" {to} "
        );
        assert!(
            plan.contains(&spelled),
            "plan.scm no longer marks {path}:{from}-{to} seen"
        );
    }
}

/// **`2a`'s preview split does not draw at 120 columns, and `T045` says it
/// should.**
///
/// Found by writing this file — the `2a` frame came back with a preview pane
/// that was empty, and the reason is an arithmetic seam nobody had cause to
/// look at before a Tier-1 frame drew the whole screen at a stated width.
///
/// [`phosphor_ui::picker::PREVIEW_AT`] is `100`, documented as *"`T045`'s own
/// number — `preview split (dropped under 100 cols)`"*. But
/// `Picker::shows_preview` is asked about **the area the widget was handed**,
/// which is the float's body — and §8 caps a float at 80% of the screen, less
/// two columns of border. At 120 columns that body is 94, so the preview is
/// shed on a terminal `T045`'s sentence says is comfortably wide enough.
///
/// Neither half is wrong on its own. The widget is right to shed on its own
/// width — it does not know what is around it — and §8 is right to cap the
/// float. What is missing is that the *threshold* was written in terminal
/// columns and is spent in body columns, and no test compared the two.
///
/// # What this test pins
///
/// The behaviour, not the number: the preview is absent at the width every
/// golden frame and every VHS tape uses, and present at a width comfortably
/// past the crossover. Asserting the crossover exactly would be asserting §8's
/// percentage, which is `float.rs`'s to change.
///
/// Recorded at `docs/OPEN-QUESTIONS.md` §45. Deliberately not fixed here:
/// raising the float's cap, lowering `PREVIEW_AT`, or teaching the composition
/// to ask for a wider float are three different answers with three different
/// owners, and picking one from inside a snapshot test is how a screen ends up
/// designed by its test.
#[test]
fn the_preview_pane_is_shed_at_120_columns_because_the_float_body_is_not_the_terminal() {
    /// A line that cannot come from the fixture, so finding it means the
    /// preview drew rather than that the buffer happened to say this.
    const SENTINEL: &str = "PREVIEW-PANE-SENTINEL";

    let theme = Theme::phosphor_dark();
    let id = SourceId("unseen".to_owned());

    let at = |width: u16| {
        let mut runtime = layer();
        let rows = rows_of(&mut runtime, "unseen", &no_args());
        let vm = picker_vm(rows, vec![SENTINEL.to_owned()]);
        let buf = render(&mut runtime, &theme, &id, vm, true, width);
        (buf.area.y..buf.area.bottom())
            .map(|y| {
                (buf.area.x..buf.area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        !at(120).contains(SENTINEL),
        "at 120 columns the float body is 94 and `PREVIEW_AT` is 100, so the \
         pane is shed — this is the finding, not the fixture",
    );
    assert!(
        at(160).contains(SENTINEL),
        "and it draws once the body clears the threshold, which is what says \
         the pane works and the seam is the arithmetic",
    );
}

/// The store is what makes `unseen` shorter than `declare`, and this is the
/// arithmetic the frames rest on: seven declared, two read, five to look at.
#[test]
fn the_seeded_store_answers_five_unseen_regions() {
    let store = seeded_store();
    assert_eq!(store.answer_unseen(None).len(), 5);
    assert_eq!(
        store.answer_unseen(Some(Path::new("src/retry.rs"))).len(),
        2,
        "three declared in retry.rs, one of them already read",
    );
}

/// The local `row_of` still drops exactly what the binary's drops.
///
/// The duplication is recorded at [`row_of`]; this is the guard on it. Both
/// halves are things a `SpanRow` carries and a `RowVm` deliberately does not,
/// and either one silently appearing here would make this file a second
/// opinion about what a picker row is.
#[test]
fn the_conversion_still_drops_what_row_of_drops() {
    let mut runtime = layer();
    let answered =
        picker::rows(&mut runtime, "unseen", &no_args()).expect("the unseen source answers");
    let row = answered.first().expect("at least one unseen region");

    assert!(
        row.tint.is_none(),
        "pickers.scm composes rows with no tint, so there is none to drop",
    );
    let converted = row_of(row);
    assert_eq!(
        converted.runs.len(),
        row.runs.len(),
        "every run survives; it is the row's *decoration* that does not",
    );
    assert!(
        converted.runs.iter().all(|run| !run.matched),
        "nothing marks a match until nucleo does (T047)",
    );
}
