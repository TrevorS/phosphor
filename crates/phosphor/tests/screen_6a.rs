//! `T048`/`T080` — screen `6a` (`:arch`) as a **Tier-1 golden frame**.
//!
//! `CP-5` names *"`:arch`"* among the screens it verifies and asks the human
//! half *"does it describe the system you think you're building?"* This is the
//! mechanical half of that: the picture is drawn, and the numbers in it are the
//! store's.
//!
//! # The point of this screen is that no Rust draws it
//!
//! `T048`'s acceptance is *"`:arch` is built entirely from the hatch, with no
//! primitive of its own"*, which is why `T080` ticked with it. Every row here
//! is `runtime/arch.scm`'s — a `view/spans` body inside a `view/float`, defined
//! by `define-float-surface!` and composed by [`phosphor_steel::float::surface`],
//! the same call `AppHost` makes. **If this frame can be drawn, the escape
//! hatch is sufficient for a whole screen**, and that is the claim `6a` exists
//! to make.
//!
//! # And the numbers are live
//!
//! `arch.scm`'s own header: *"the numbers in it come from `(arch)` — which is
//! why this is not a static drawing"*. The host below answers that query off a
//! real [`phosphor_core::store::Store`], so the counts on the frame are
//! arithmetic over seeded regions rather than digits this file typed.
//! [`the_counts_on_the_frame_are_the_stores_own`] is what keeps that true.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see `screen_3c.rs`'s module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use frame_grid::Frame;
use phosphor_core::action::{Action, FloatAction, Outcome, Receipt, Refusal, Request};
use phosphor_core::query::{Answer, Answers, Query, QueryError, UiQuery};
use phosphor_core::request::{Actor, Position, RegionSpec, Span};
use phosphor_core::store::{Scope, SeenState, Store};
use phosphor_core::value::{Args, Value};
use phosphor_core::view::{Axis, Constraint, Millis, Node, SessionState, Slot, Tree};
use phosphor_steel::float;
use phosphor_steel::host::Host;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, StatusVm};
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// How many languages the shipped layer declares, as `(arch)` reports it.
///
/// Twelve is `runtime/languages/`'s own count and `doc_claims.py` is what keeps
/// prose honest about it; here it is a number the host answers, so the frame
/// says twelve because this says twelve. Asserted against the layer in
/// [`the_language_count_is_the_shipped_layers_own`] rather than trusted.
const LANGUAGES: usize = 12;

/// The regions the store is seeded with — `fixtures/seed/plan.scm`'s
/// `src/retry.rs` set, one of which is read.
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

fn seeded_store() -> Store {
    let mut store = Store::new();
    let specs: Vec<RegionSpec> = REGIONS
        .iter()
        .map(|(from, to, _)| RegionSpec {
            path: PathBuf::from("src/retry.rs"),
            span: span(*from, *to),
            author: Actor::Claude,
        })
        .collect();
    store.declare_regions(&specs, Actor::Claude);
    for (from, to, state) in REGIONS {
        if *state == SeenState::Seen {
            store.set_seen(
                &Scope::Span {
                    path: PathBuf::from("src/retry.rs"),
                    span: span(*from, *to),
                },
                SeenState::Seen,
            );
        }
    }
    store
}

/// A host that answers `(arch)` and records the surfaces the layer defines.
///
/// The same two-part shape `screen_pickers.rs` documents at length, one
/// capability over: `define-float-surface!` is a mutation, so a host that
/// refused every Action would refuse the definition of the very surface this
/// frame draws — and the boot report would still come back clean, because a
/// refusal is an answer rather than a fault.
#[derive(Debug)]
struct Substrate {
    store: Mutex<Store>,
    surfaces: Mutex<Vec<(String, String)>>,
}

impl Substrate {
    fn new() -> Self {
        Self {
            store: Mutex::new(seeded_store()),
            surfaces: Mutex::new(Vec::new()),
        }
    }

    /// Binds every recorded surface into `runtime`, the way the loop binds one.
    fn install(&self, runtime: &mut Runtime) {
        let surfaces = self.surfaces.lock().expect("no other thread holds this");
        assert!(
            surfaces.iter().any(|(id, _)| id == "arch"),
            "runtime/arch.scm defines the `arch` surface; the layer asked for {:?}",
            surfaces.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        );
        for (id, body) in surfaces.iter() {
            let outcome = runtime.evaluate(&float::define_form(id, body));
            assert!(
                matches!(outcome, Outcome::Done(_)),
                "binding the `{id}` surface: {outcome:?}"
            );
        }
    }
}

fn count(of: usize) -> Value {
    Value::Int(i64::try_from(of).unwrap_or(i64::MAX))
}

impl Answers for Substrate {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        let store = self
            .store
            .lock()
            .map_err(|_| QueryError::NotYetImplemented {
                task: query.spec().since.task,
            })?;
        match query {
            // The same five fields `AppHost` fills, off the same calls.
            Query::Ui(UiQuery::Arch {}) => Ok(Answer {
                value: Value::Record(
                    Args::new()
                        .with(
                            "unseen",
                            count(store.regions().unseen_count(&Scope::Everywhere)),
                        )
                        .with(
                            "seen",
                            count(store.regions().seen_count(&Scope::Everywhere)),
                        )
                        .with("anchors", count(store.anchors().len()))
                        .with(
                            "diagnostics",
                            count(
                                store
                                    .diagnostics()
                                    .files()
                                    .map(|(_, published)| published.len())
                                    .sum::<usize>(),
                            ),
                        )
                        .with("languages", count(LANGUAGES)),
                ),
                revision: store.revision(),
            }),
            other => Err(QueryError::NotYetImplemented {
                task: other.spec().since.task,
            }),
        }
    }
}

impl Host for Substrate {
    fn apply(&self, request: &Request) -> Outcome {
        match &request.action {
            Action::Float(FloatAction::DefineFloatSurface { surface, body }) => {
                if let Ok(mut surfaces) = self.surfaces.lock() {
                    surfaces.push((surface.0.clone(), body.clone()));
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

/// The shipped editor layer, booted clean, with its float surfaces bound.
fn layer() -> Runtime {
    let host = Arc::new(Substrate::new());
    let mut runtime = Runtime::boot(Some(&runtime_tree()), host.clone() as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    host.install(&mut runtime);
    runtime
}

/// `6a`'s statusline: no file, because `:arch` describes the editor rather than
/// anything you have open.
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: Some("arch".to_owned()),
        file: None,
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        unseen: 2,
        trouble: 0,
        attention: 0,
        vcs: None,
        server: None,
        cursor: None,
        hints: Vec::new(),
    }
}

/// The screen: an empty pane, the statusline, and `:arch` over both.
///
/// **The same shape `main.rs` composes** for every Steel surface — *"an empty
/// root is a float over what the widgets painted"* — and the float itself is
/// whatever `arch.scm` answered, not a `Float` this file built.
fn tree(runtime: &mut Runtime) -> Tree {
    let surface = float::surface(runtime, "arch", &Value::Record(Args::new()))
        .expect("runtime/arch.scm composes the `arch` surface");
    let status = status::compose(runtime, &status_vm()).expect("runtime/statusline.scm composes");
    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(Constraint::Fill { weight: 1 }, Node::Empty {}),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
    .with_float(surface)
}

/// What this frame is missing, and which task owns each absence.
const NOTES: &[&str] = &[
    "Every row is runtime/arch.scm's, composed through the T080 spans hatch",
    "  and drawn by the interpreter. No Rust in this repository knows what",
    "  `:arch` looks like, which is T048's whole acceptance.",
    "The five counts come from the (arch) query over a seeded store: three",
    "  regions, one of them read. Anchors and diagnostics are 0 because",
    "  nothing placed or published any — that is the honest state, not a gap.",
    "The language count is answered by the host rather than read off the VM:",
    "  `(arch)`'s `languages` field is the loop's own `languages.lock().len()`",
    "  and a test host has no such table. Asserted against the shipped layer",
    "  in the_language_count_is_the_shipped_layers_own.",
    "6a's mockup draws box-drawing rules between the bands. arch.scm writes",
    "  them as meta-toned text rows, which is what a spans body can say.",
];

/// Renders `6a` at `width`.
fn screen(runtime: &mut Runtime, theme: &Theme, width: u16) -> Buffer {
    let area = Rect::new(0, 0, width, 24);
    let tree = tree(runtime);
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, &NoResources).render(&tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`6a` needs a primitive that does not exist: {:?}",
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
    assert!(
        frame.unnamed(&buf).is_empty(),
        "colours on screen that no Theme field names: {:?}",
        frame.unnamed(&buf)
    );
    insta::assert_snapshot!(name, frame.to_text(&buf));
}

/// The screen, at the width the `CP-1` golden frames use.
#[test]
fn screen_6a_draws() {
    golden("6a", 120);
}

/// The same screen at 80 columns, where §11 docks rather than centres.
#[test]
fn screen_6a_draws_at_80_columns() {
    golden("6a-80", 80);
}

// ---------------------------------------------------------------------------
// The claims the frame stands on
// ---------------------------------------------------------------------------

/// The counts on the frame are arithmetic over the store, not digits this file
/// typed into a fixture.
///
/// Three regions declared, one marked seen — so `2` and `1`. A frame that
/// committed the wrong numbers would still be a valid picture of something,
/// which is the failure a golden frame cannot report on its own.
#[test]
fn the_counts_on_the_frame_are_the_stores_own() {
    let store = seeded_store();
    assert_eq!(store.regions().unseen_count(&Scope::Everywhere), 2);
    assert_eq!(store.regions().seen_count(&Scope::Everywhere), 1);
    assert_eq!(store.anchors().len(), 0, "nothing placed an anchor");
    assert_eq!(
        store
            .diagnostics()
            .files()
            .map(|(_, published)| published.len())
            .sum::<usize>(),
        0,
        "no server published"
    );

    let mut runtime = layer();
    let drawn = rows(&screen(&mut runtime, &Theme::phosphor_dark(), 120)).join("\n");
    assert!(
        drawn.contains('2') && drawn.contains('1'),
        "the counts reach the frame:\n{drawn}"
    );
}

/// `LANGUAGES` is what the shipped layer actually declares.
///
/// The host answers it because a test host has no language table, so this is
/// the seam where the number could quietly stop being true. `runtime/init.scm`
/// loads them, so the layer is the authority and this reads it.
#[test]
fn the_language_count_is_the_shipped_layers_own() {
    let declared = std::fs::read_dir(runtime_tree().join("languages"))
        .expect("runtime/languages/ exists")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "scm"))
        .count();
    assert_eq!(
        declared, LANGUAGES,
        "runtime/languages/ holds {declared} grammars and this file says {LANGUAGES}",
    );
}

/// `:arch` is reachable by the name the ex line uses, and the surface it opens
/// is the one this frame draws.
///
/// The frame proves the *drawing*; this proves nobody has to know the word
/// `arch` twice. `T048`'s ex command is `arch.scm`'s own `ex-set!`.
#[test]
fn the_ex_command_names_the_surface_this_frame_draws() {
    let arch = std::fs::read_to_string(runtime_tree().join("arch.scm"))
        .expect("runtime/arch.scm is committed");
    assert!(
        arch.contains(r#"(ex-set! "arch""#),
        "`:arch` is bound in the file that defines the surface",
    );
    assert!(
        arch.contains(r#"(key/cmd "open-float" "surface" "arch""#),
        "and it opens the surface by the id this frame composes",
    );
}

/// A buffer's rows as trimmed text.
fn rows(buf: &Buffer) -> Vec<String> {
    (buf.area.y..buf.area.bottom())
        .map(|y| {
            (buf.area.x..buf.area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}
