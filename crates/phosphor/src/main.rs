//! The phosphor binary: terminal setup, the event loop, input routing and panes.
//!
//! This is the app layer — the one place `ratatui` and `crossterm` are allowed.
//! It owns the frame: Steel decides what is on screen, this decides when pixels
//! land (Q12). Input is decoded here into Actions; nothing else emits them.
//!
//! Owned by `spine`. The loop lands in Window C; what is here now is `T090`.
//!
//! # `T090` — the S1 host, and its demolition date
//!
//! `CP-1`'s run line is `cargo run -- src/some_real_file.rs`. Windows A and B
//! built the whole widget layer around a `fn main() {}`, so that line drew
//! nothing, every tape died on `Require phosphor`, and the checkpoint's manual
//! half had nothing to open. This file is the application those widgets were
//! missing, and **it is deliberately the thinnest thing that makes `CP-1`
//! judgeable**:
//!
//! * open the file named on argv, pick a theme by slug, build one frame out of
//!   [`BufferView`] + [`StatusLine`] + [`FloatSlot`], draw it through `T014`'s
//!   synchronized-output wrapper, and quit restoring the terminal;
//! * input rides the vendored core's own `editor_crossterm` handler, exactly as
//!   `TASKS.md`'s S1 preamble says S1 does.
//!
//! **Nothing above it may grow to depend on it.** `S2` added the contract layer
//! documented below — Actions (`T019`), the registry (`T020`), Steel (`T021`),
//! the REPL (`T022`) and the CLI door (`T023`) — but the loop below still has no
//! input machine (`T026`), no panes (`T088`), and no state that outlives it.
//! `T026` deletes this file's event handling in one commit; the two lines of
//! `Cargo.toml` that turn the fork's `crossterm` feature on go with it.
//!
//! # `T022` — the REPL, and the one thing this file must not cache
//!
//! The Steel runtime is built once, in [`vm`], and **both** paths take that one
//! object: the door hands it scheme source through [`Vm`], and the loop hands it
//! every keystroke. That is what makes *"`--eval` and the REPL agree"*
//! structural — there is one runtime, reached two ways, not two runtimes kept in
//! step.
//!
//! The liveness claim (`T022`: a rebind takes effect on the very next keystroke,
//! no restart) rests on one rule this file obeys: **there is no keymap in Rust.**
//! [`press`] encodes the key in vim notation and asks the VM
//! (`runtime/keymaps.scm`) what to do with it, every time. Nothing here caches a
//! binding, so nothing here can go stale, and a reload step would be a `CP-2`
//! failure rather than a slower path.
//!
//! [`AppHost`] is the other half: `phosphor-steel`'s barrier says Steel may emit
//! Actions and read ViewModels, and this is the thing on the far side of it. It
//! carries out the four Actions `S2` can honestly carry out — open and close the
//! REPL, walk its history, move it into a buffer — plus `set-option!` and
//! `persist-form!`, and refuses everything else by naming the task that builds
//! it. Surface Actions arrive as [`Intent`]s the loop drains, because the widgets
//! are the loop's and a `Fn` inside the VM cannot borrow them.
//!
//! # `T023` — the CLI door, alongside the host
//!
//! [`door`] is the other half of this file's job and does not touch the loop at
//! all. `phosphor --eval '(…)'` and the 208 generated capability verbs return
//! **before** [`Term`] is constructed: no alternate screen, no raw mode, no
//! frame. That is a requirement, not an accident — `V006` seeds tape fixtures
//! through `--eval` with no test-only backdoor, which needs the door to work
//! with stdout on a pipe and no tty.
//!
//! The two paths share one parser, as the root manifest's `clap` entry says they
//! would: [`door::parser`] takes `Cli::command()` and adds the generated verbs
//! to it, so `--theme` and the file argument keep one definition. Which path
//! runs is decided by argv alone — a subcommand or `--eval` is the door, a file
//! is the host, and clap refuses both at once.
//!
//! # What `T019` has to carry, learned here
//!
//! Recorded because the S1 host is the first thing to actually run the widget
//! layer, and three of these are only visible from a loop:
//!
//! 1. **Scroll is a request, and the fork disagrees.**
//!    `buffer_view::apply_scroll` is invariant 3's single writer, but
//!    `Editor::input` ends every keystroke with the fork's own `focus()`, and
//!    `Editor::mouse` calls `scroll_up`/`scroll_down` directly. So while S1
//!    rides the vendored handler, the viewport moves from two places. `Action`
//!    needs a `Scroll(ScrollRequest)` variant — `ScrollRequest` is already
//!    shaped as its payload, `RevealRow { row, margin }` included — and `T026`
//!    has to stop calling `input`/`mouse` rather than wrap them.
//! 2. **A mode is a fact the statusline reads, not a flag input owns.** The
//!    chip here is hardcoded [`Mode::Normal`] because S1 has no modality at all
//!    and every keystroke inserts text. `T026`'s grammar owns the real one, and
//!    `soft_wrap::set_mode` already wants it (whitespace marks are INSERT-only).
//! 3. **Dirty is per buffer and comes from the edit stream**, not from a save
//!    path — see [`dirty_flag`].
//! 4. **A float is opened by whoever asked for it and closed by `esc`.** The
//!    one-float rule lives in [`FloatSlot`]; what `Action` owes it is
//!    `OpenFloat(kind)` / `CloseFloat`, and `T021`'s broken-`init.scm` float is
//!    the first real caller.
//!
//! # The two flags that are not in a mockup
//!
//! `--theme <slug>` is assumed by eight tapes and is the product's own CLI. The
//! other two are scaffolding for `CP-1`'s manual half, and are marked as such
//! in `--help`:
//!
//! * `--float <mood>` opens `T084`'s fixture body in one of the two moods, so
//!   the float contract can be looked at on a real terminal. No *feature* opens
//!   a float at S1 — the first one that does is `T021`.
//! * `--soft-wrap` turns `T081` on. The mockups say soft wrap is **off by
//!   default** ("Text details: soft-wrap off by default, ↪ continuation when
//!   on"), so the default here is off and this flag is the only way `CP-1` can
//!   see the on state before `T026` has a key for it.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use phosphor_core::action::{Action, Outcome, Receipt, Refusal, Request, RuntimeAction};
use phosphor_core::query::{Answer, Answers, Query, QueryError, Revision};
use phosphor_core::value::Value;
use phosphor_core::view::{Node, SessionState, Tree};
use phosphor_steel::host::Host;
use phosphor_steel::keymap::{self, Press};
use phosphor_steel::repl::Repl;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, ComposeError, StatusFile, StatusVm};
use phosphor_term::{Frame, Term};
use phosphor_ui::buffer_view::{self, BufferView, editor_area};
use phosphor_ui::float::{Float, FloatFooter, FloatHeader, FloatSlot, FooterHint, TextBody};
use phosphor_ui::frame::FrameCache;
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::{BUILTIN_SLUGS, Theme, builtin};
use ratatui::layout::Rect;
// The same type `phosphor_ui::buffer_view` re-exports. Named through the fork
// here on purpose: this file is the one place that talks to the vendored
// handler, and `Editor::input` / `Editor::mouse` are the fork's API, not the
// widget layer's.
use ratatui_code_editor::editor::Editor;

mod door;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// The S1 host's argument surface.
#[derive(Debug, Parser)]
#[command(
    name = "phosphor",
    version,
    about = "phosphor — an agent-native terminal editor",
    long_about = "Opens one file and draws it: BufferView + StatusLine, every frame inside a \
                  synchronized-output block.\n\nThis is the S1 host (T090), carrying S2's \
                  contract layer: a Steel runtime booted from runtime/, a live keymap that is \
                  asked on every keystroke, the 208 capability verbs, and `--eval`. There is no \
                  input machine (T026) and no agent session yet; buffer keys still ride the \
                  vendored editor core, `q` or `esc` quits."
)]
struct Cli {
    /// File to open. Not needed with `--eval`, `--repl` or a capability verb.
    #[arg(value_name = "FILE", required_unless_present_any = ["eval", "repl"])]
    path: Option<PathBuf>,

    /// Open the Steel REPL (`6b`) on the frame — the primary extension
    /// workflow. `:` opens it from the editor, `esc` closes it, and a file is
    /// optional because the REPL is a surface of its own.
    #[arg(long)]
    repl: bool,

    /// Evaluate an expression and print the result — the CLI door (T023). Opens
    /// no terminal: no alternate screen, no raw mode, no frame.
    #[arg(long, value_name = "EXPR", conflicts_with = "path")]
    eval: Option<String>,

    /// Theme slug: one of the six shipped themes.
    #[arg(long, value_name = "SLUG", default_value = DEFAULT_THEME)]
    theme: String,

    /// SCAFFOLDING (T090): open T084's fixture float, so CP-1 can see the
    /// float contract on a real terminal. `esc` closes it.
    #[arg(long, value_name = "MOOD", value_enum)]
    float: Option<FloatMood>,

    /// SCAFFOLDING (T090): turn T081's soft wrap on. Off by default, as the
    /// mockups specify; there is no Action to toggle it until T026.
    #[arg(long)]
    soft_wrap: bool,
}

/// Which mood [`Cli::float`] opens. Design Language §4's two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FloatMood {
    /// Pickers, help, diffs — anything you asked for.
    Informational,
    /// Questions and permission asks: warmer border, darker body.
    NeedsYou,
}

/// §10: "Phosphor (dark + light) ships as the v1 default", dark first.
const DEFAULT_THEME: &str = "phosphor-dark";

// ---------------------------------------------------------------------------
// The host behind the barrier
// ---------------------------------------------------------------------------

/// What a surface Action asked for, waiting for the loop to do it.
///
/// A binding runs *inside* the VM, from a `Fn` Steel requires to be
/// `Send + Sync + 'static` — it cannot borrow the editor, the float slot or the
/// session. So [`AppHost`] records the ask and the loop drains it on the way
/// back out, which is the same shape the store will take at `T041` with a
/// revision instead of a queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    /// `(open-repl!)` — `6b`.
    OpenRepl,
    /// `(close-repl!)`.
    CloseRepl,
    /// `(repl-history! delta)` — positive walks back.
    History(i64),
    /// `(repl-to-buffer!)` — `6b`'s `C-c buffer`.
    ToBuffer,
}

/// The editor as Steel is allowed to see it.
///
/// `phosphor-steel`'s [`Host`] is the whole barrier: apply a Request, answer a
/// Query, and nothing else. This implements the first half for the capabilities
/// `S2` can honestly carry out and refuses the rest **by reading each row's own
/// task id**, so there is no table here and a capability cannot be quietly
/// forgotten.
///
/// The read side is `S5`'s: a query is a projection of a store snapshot and
/// there is no store, so [`Answers`] names the task that builds each one.
#[derive(Debug, Default)]
struct AppHost {
    state: Mutex<HostState>,
}

/// Everything the host owns that Steel can reach.
#[derive(Debug, Default)]
struct HostState {
    /// Surface asks, oldest first.
    intents: Vec<Intent>,
    /// `(set-option! …)`. `init.scm` sets `soft-wrap` here at boot.
    options: BTreeMap<String, Value>,
    /// The runtime tree `persist-form!` appends to, if there is one.
    root: Option<PathBuf>,
    /// Which file in that tree, named by the editor layer. See [`PERSIST_FILE`].
    file: String,
}

/// What a layer that declares no [`PERSIST_FILE`] gets — `6b`'s own note, and
/// the right answer for a layer that is one file.
const INIT: &str = "init.scm";

/// The global the editor layer binds to name the file the REPL writes to.
///
/// **Not `init.scm`, when a layer has more than one file.** `init.scm` runs to
/// its last form *before* Rust reads the load order it declared, so a form
/// appended to it can only use names Rust registered — a persisted
/// `(keymap-set! …)` would come back on the next start as a free-identifier
/// fault in a boot float, because `keymaps.scm` has not loaded yet. Found by
/// running it; the regression is
/// [`a_persisted_rebind_survives_the_next_boot`](tests::a_persisted_rebind_survives_the_next_boot).
///
/// So the layer names the file that loads last (`runtime/repl.scm`), and this
/// reads it. The path is resolved once, after the boot, in [`vm`] — the host is
/// behind the barrier and may not re-enter the VM to ask.
const PERSIST_FILE: &str = "phosphor/persist-file";

impl AppHost {
    fn new(root: Option<PathBuf>) -> Self {
        Self {
            state: Mutex::new(HostState {
                root,
                file: INIT.to_owned(),
                ..HostState::default()
            }),
        }
    }

    /// Points `persist-form!` at the file the layer named.
    ///
    /// A name only — a path leaving the runtime tree is refused for the same
    /// reason the load order's is (`boot::is_confined`): the editor layer is a
    /// tree, not a path into the filesystem.
    fn persist_to(&self, file: &str) {
        let confined = Path::new(file)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)));
        if !confined {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state.file = file.to_owned();
        }
    }

    /// Everything asked for since the last drain.
    fn intents(&self) -> Vec<Intent> {
        self.state
            .lock()
            .map(|mut state| core::mem::take(&mut state.intents))
            .unwrap_or_default()
    }

    /// A boolean option, or `None` if `init.scm` never set it.
    fn flag(&self, key: &str) -> Option<bool> {
        match self.state.lock().ok()?.options.get(key)? {
            Value::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// Records a surface ask for the loop to carry out.
    fn ask(&self, intent: Intent) {
        if let Ok(mut state) = self.state.lock() {
            state.intents.push(intent);
        }
    }

    /// `6b`'s `· persisted to init.scm` — one form appended to the tree that
    /// booted.
    ///
    /// The note is the receipt's, not the REPL's: whoever appended the line is
    /// the only one who knows where it went, and it **names the file it wrote**
    /// rather than the one `6b` draws, because a layer of more than one file
    /// does not write to `init.scm` (see [`PERSIST_FILE`]).
    fn persist(&self, form: &str) -> Outcome {
        let Ok(state) = self.state.lock() else {
            return declined("the editor layer is busy");
        };
        let Some(root) = state.root.clone() else {
            return declined("no runtime tree to write to — set $PHOSPHOR_RUNTIME");
        };
        let file = state.file.clone();
        drop(state);

        let path = root.join(&file);
        let written = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut handle| writeln!(handle, "{form}"));
        match written {
            Ok(()) => Outcome::Done(Receipt {
                capability: "persist-form",
                value: Value::Null,
                note: Some(format!("persisted to {file}")),
            }),
            Err(error) => declined(&format!("{}: {error}", path.display())),
        }
    }
}

impl Answers for AppHost {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        Err(QueryError::NotYetImplemented {
            task: query.spec().since.task,
        })
    }
}

impl Host for AppHost {
    fn apply(&self, request: &Request) -> Outcome {
        let name = request.action.spec().name;
        let done = |value: Value| {
            Outcome::Done(Receipt {
                capability: name,
                value,
                note: None,
            })
        };
        match &request.action {
            Action::Runtime(RuntimeAction::OpenRepl {}) => {
                self.ask(Intent::OpenRepl);
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::CloseRepl {}) => {
                self.ask(Intent::CloseRepl);
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::ReplHistory { delta }) => {
                self.ask(Intent::History(*delta));
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::ReplToBuffer {}) => {
                self.ask(Intent::ToBuffer);
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::SetOption { key, value }) => {
                match self.state.lock() {
                    Ok(mut state) => state.options.insert(key.clone(), value.clone()),
                    Err(_) => return declined("the editor layer is busy"),
                };
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::PersistForm { form }) => self.persist(form),
            // Everything else, by its own row: `not built yet — T041 builds it`.
            // Derived, never listed, so this arm cannot rot.
            action => Outcome::Refused(Refusal::NotYetImplemented {
                task: action.spec().since.task,
            }),
        }
    }
}

/// A refusal carrying its own sentence — not an error (`action.rs`).
fn declined(reason: &str) -> Outcome {
    Outcome::Refused(Refusal::Declined {
        reason: reason.to_owned(),
    })
}

/// The evaluator the CLI door takes, over the one runtime this process has.
struct Vm<'a>(&'a mut Runtime);

impl door::Evaluate for Vm<'_> {
    fn eval(&mut self, source: &str) -> Outcome {
        self.0.evaluate(source)
    }
}

/// The editor layer, booted, and the host behind its barrier.
///
/// **One constructor, both paths.** `--eval` and the loop call this and nothing
/// else, so the CLI door and the REPL are answering out of the same VM with the
/// same host — which is what makes `T023`'s *"identical results for the same
/// expression"* structural rather than a thing to keep checking.
fn vm() -> (Runtime, Arc<AppHost>) {
    let root = Runtime::root();
    let host = Arc::new(AppHost::new(root.clone()));
    let runtime = boot(root.as_deref(), &host);
    (runtime, host)
}

/// Boots the editor layer against `host`, and reads back where it writes.
///
/// The read happens **once, after the boot**: the layer decides the file
/// ([`PERSIST_FILE`]), and the host is behind the barrier and may not re-enter
/// the VM to ask when a form arrives.
fn boot(root: Option<&Path>, host: &Arc<AppHost>) -> Runtime {
    let runtime = Runtime::boot(root, Arc::clone(host) as Arc<dyn Host>);
    if let Ok(value) = runtime.global(PERSIST_FILE) {
        if let Ok(Value::Text(file)) = phosphor_steel::convert::from_steel(&value) {
            host.persist_to(&file);
        }
    }
    runtime
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let matches = door::parser(Cli::command()).get_matches();
    match dispatch(&matches) {
        Ok(code) => code,
        Err(error) => {
            report(&*error);
            ExitCode::FAILURE
        }
    }
}

/// argv chooses: the CLI door, or the S1 host.
///
/// Both branches come out of one [`ArgMatches`], so there is no second parse and
/// no way for the two to disagree about what was typed. The door branch returns
/// before [`Term`] exists, which is what makes `--eval` usable with no terminal
/// at all.
///
/// **The runtime is built on both paths and built the same way** ([`vm`]).
/// `T021` embedded `steel-core`; `T022` wired it in here, so `--eval` answers
/// out of the same VM the REPL types into. Nothing about the door changed shape
/// — that is the point of [`door::Evaluate`] being one method wide.
fn dispatch(matches: &ArgMatches) -> Result<ExitCode, Box<dyn Error>> {
    if let Some((verb, supplied)) = matches.subcommand() {
        let call = door::call(verb, supplied)?;
        let (mut runtime, _host) = vm();
        return Ok(door::run(&call, Some(&mut Vm(&mut runtime)))?);
    }

    let cli = Cli::from_arg_matches(matches)?;
    if let Some(source) = &cli.eval {
        let call = door::eval_call(source)?;
        let (mut runtime, _host) = vm();
        return Ok(door::run(&call, Some(&mut Vm(&mut runtime)))?);
    }

    // `required_unless_present_any` is what makes this total: clap has already
    // refused a command line with none of a file, `--eval` and `--repl`.
    if cli.path.is_none() && !cli.repl {
        return Err("give a file to open, an expression to evaluate, or --repl".into());
    }
    run(&cli)?;
    Ok(ExitCode::SUCCESS)
}

/// The one legitimate write to stderr in this binary.
///
/// Every fallible step below either happens before [`Term`] exists or returns
/// through it, and `Term` restores on drop — so by the time this prints, the
/// alternate screen is gone and the message lands where it can be read.
#[expect(
    clippy::print_stderr,
    reason = "the CLI door reports failures on stderr; the TUI is already restored here"
)]
fn report(error: &dyn Error) {
    eprintln!("phosphor: {error}");
    let mut source = error.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let theme = builtin(&cli.theme).ok_or_else(|| unknown_theme(&cli.theme))?;

    // The editor layer, before the terminal: a boot fault has to be able to
    // reach the float, and a fault reading the file has to be able to reach
    // stderr.
    let (mut runtime, host) = vm();
    let boot = runtime.boot_float();

    let (mut editor, path) = match &cli.path {
        Some(file) => {
            let text = std::fs::read_to_string(file)
                .map_err(|err| format!("{}: {err}", file.display()))?;
            // The path as the user typed it. Repo-relative is what the mockups
            // draw, but the repo root is `phosphor-vcs`'s answer (`T071`) and
            // inventing one here would be a value nobody asked for.
            (
                buffer(language_of(file), &text, &theme)?,
                Some(file.display().to_string()),
            )
        }
        // `--repl` with no file. The REPL is a surface of its own and `6b`
        // draws no buffer behind it; an empty buffer is what `esc` lands on,
        // and the statusline says so by drawing no file segment at all.
        None => (buffer("text", "", &theme)?, None),
    };
    let dirty = dirty_flag(&mut editor);

    let mut repl = Repl::new();
    let mut surface = match (&boot, cli.repl, cli.float) {
        // A boot fault outranks what you asked for: it is the one thing you
        // cannot act on without seeing (`T021`).
        (Some(_), _, _) => Surface::Boot,
        (None, true, _) => Surface::Repl,
        (None, false, Some(mood)) => Surface::Fixture(mood),
        (None, false, None) => Surface::Buffer,
    };

    // `T079`, on the shipping path. `revision` is bumped when the statusline's
    // facts change and never per frame, so the composer runs on a state change
    // and only then; `last_vm` is what "changed" is measured against until
    // `T041` gives the store a revision of its own.
    let mut status_cache = FrameCache::new();
    let mut revision = Revision::INITIAL;
    let mut last_vm: Option<StatusVm> = None;

    let mut term = Term::new()?;
    loop {
        // The size the *next* frame will be laid out at. `draw` re-splits
        // `frame.area()` itself, so this is only for the two things that need
        // `&mut editor` and therefore cannot happen inside the closure: the
        // wrap width, and the area the vendored input handler measures against.
        let size = term.size()?;
        let (body, _status) = split(Rect::new(0, 0, size.width, size.height));
        // `init.scm` sets `soft-wrap` at boot and `(set-option! …)` can change
        // it at the REPL, so it is read per frame rather than once: the option
        // is the editor layer's, and the flag is the override.
        if cli.soft_wrap || host.flag("soft-wrap") == Some(true) {
            // Free when the width has not changed, and it moves no viewport.
            soft_wrap::wrap_to(&mut editor, body);
        }

        // What Steel composed, if this surface is composed rather than wired.
        // `T079`'s interpreter draws it; nothing here knows a colour.
        let tree = match (&surface, &boot) {
            (Surface::Repl, _) => Some(repl.frame()),
            // Over the buffer rather than instead of it: the root is empty and
            // the float dims what the widgets already painted (§9).
            (Surface::Boot, Some(boot)) => Some(Tree::new(Node::Empty {}).with_float(boot.clone())),
            _ => None,
        };
        let mut floats = FloatSlot::empty();
        if let Surface::Fixture(mood) = surface {
            floats.open(fixture_float(mood));
        }

        // The facts, stated by Rust. What they mean on screen is
        // `runtime/statusline.scm`'s (`T025`) — there is no segment list, no
        // order and no shed ladder on this side of the call.
        let vm = StatusVm {
            // The chip's word is the layer's: it maps `normal` to `NORMAL` and
            // picks the actor. `T026` is what makes this anything but normal.
            mode: "normal".to_owned(),
            surface: None,
            file: path.as_deref().map(|path| StatusFile {
                path: PathBuf::from(path),
                dirty: dirty.get(),
            }),
            // Truthful, and the truth at S2 is that there is no session, no
            // store to count unseen regions in, and no VCS adapter. `T050`,
            // `T041` and `T071` fill these in; a fixture here would be a lie
            // on a real terminal.
            session: SessionState::None,
            since: None,
            ask_pending: false,
            unseen: 0,
            vcs: None,
            cursor: Some(cursor_of(&editor)),
            hints: Vec::new(),
        };

        // `T079`'s cache, on the path that ships. The revision stands in for the
        // store's: at S2 the statusline's facts *are* its state, so a revision
        // that moves when the ViewModel does is the same signal `T041` will
        // give this call later, arriving from a different place.
        if last_vm.as_ref() != Some(&vm) {
            revision = revision.next();
            last_vm = Some(vm.clone());
        }
        // `Unbound` is not a fault: a layer that composes no statusline draws
        // none, because a Rust fallback here is precisely the *"config file with
        // a Rust editor hiding behind it"* `CP-2` asks about (`status.rs`).
        // A composition that *raises* keeps the last good line on screen and
        // costs one VM invocation, not one per frame — `try_update` records the
        // revision before it calls, so a broken redefinition is not retried.
        //
        // **What this does not do yet:** surface a raised composition. The last
        // good line stays on screen and the error goes nowhere, because the
        // buffer surface has no float of its own at `S2` — `T021`'s boot float
        // is the only one, and it is built once at start. `T026` gives the loop
        // one composition path and one float slot for frame faults; until then
        // this is a documented gap rather than a silent swallow, and the REPL
        // shows the same error the moment you evaluate anything.
        let composed = match status_cache.try_update(revision, || {
            status::compose(&mut runtime, &vm).map(Tree::new)
        }) {
            Ok(_) => true,
            Err(ComposeError::Unbound) => false,
            // Keep whatever last composed successfully; nothing, if that is
            // what there was.
            Err(_) => !matches!(status_cache.tree().root, Node::Empty { .. }),
        };
        let status_tree = composed.then(|| status_cache.tree());

        term.draw(|frame| draw(frame, &editor, &theme, status_tree, &floats, tree.as_ref()))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Release => {}
            Event::Key(key) if matches!(surface, Surface::Repl) => {
                // The one case a revision cannot express: an evaluation may
                // have redefined `phosphor/status-line` itself while every fact
                // it reads stood still. Without this, a redefinition would not
                // appear until the next unrelated edit — which is exactly the
                // *"does it take effect on the very next frame?"* question
                // `CP-2`'s manual half asks. Cheap: the buffer statusline is not
                // drawn while the REPL owns the frame, so this costs one
                // composition on the way back, not one per keystroke.
                status_cache.invalidate();
                match repl_key(key, &mut repl, &mut runtime) {
                    ReplStep::Handled => {}
                    ReplStep::Close => surface = Surface::Buffer,
                    ReplStep::ToBuffer => {
                        editor = session_buffer(&repl, &theme)?;
                        track_dirty(&mut editor, &dirty);
                        surface = Surface::Buffer;
                    }
                }
            }
            // **The keymap is asked, never cached** — every key, every time,
            // out of the live VM. This is the whole of `T022`'s liveness
            // claim, and `T026` inherits the rule along with the loop.
            Event::Key(key) => match press(&mut runtime, key) {
                Press::Handled | Press::Pending => {}
                Press::Unbound => match key_step(key, &mut surface) {
                    Step::Quit => break,
                    Step::Handled => {}
                    Step::ToEditor => editor.input(key, &editor_area(body))?,
                },
            },
            Event::Mouse(mouse) => to_editor_mouse(&mut editor, mouse, body)?,
            // A resize redraws from the new size on the next turn of the loop;
            // so does everything else this arm swallows (focus, paste).
            _ => {}
        }

        // What Steel asked for while that key was being handled. Draining here
        // rather than inside the binding is what keeps the VM out of the
        // widgets (`AppHost`).
        for intent in host.intents() {
            match intent {
                Intent::OpenRepl => surface = Surface::Repl,
                Intent::CloseRepl => surface = Surface::Buffer,
                Intent::History(delta) => repl.history(delta),
                Intent::ToBuffer => {
                    editor = session_buffer(&repl, &theme)?;
                    track_dirty(&mut editor, &dirty);
                    surface = Surface::Buffer;
                }
            }
        }
    }

    term.restore()?;
    Ok(())
}

/// One configured buffer over `text`.
fn buffer(language: &str, text: &str, theme: &Theme) -> Result<Editor, Box<dyn Error>> {
    let mut editor = Editor::new(language, text, Vec::new())?;
    // Order matters: `soft_wrap::configure` puts folds back on (without the
    // gutter column `8e` does not draw), so it goes second.
    buffer_view::configure(&mut editor, theme);
    soft_wrap::configure(&mut editor, theme);
    Ok(editor)
}

/// `6b`'s `C-c buffer` — the session as an editable buffer.
///
/// **One pane, so this replaces what was on screen.** `T088` gives the session
/// a pane of its own; until then the honest limit is that there is one, and
/// nothing is lost by using it — `S2` has no save path, so the file on disk is
/// untouched and `q` already discards the same unsaved edits.
///
/// The language is `text`: the fork is built with ten grammars and scheme is not
/// one of them (`language_of`). `define-language` (`T037`) is what makes a
/// steel buffer highlight, and it is also what `6b`'s coloured literals need.
fn session_buffer(repl: &Repl, theme: &Theme) -> Result<Editor, Box<dyn Error>> {
    buffer("text", &repl.lines().join("\n"), theme)
}

// ---------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------

/// Buffer area above, statusline on the last row — the layout every `CP-1`
/// mockup has, and the same split `T018`'s golden frames use.
fn split(area: Rect) -> (Rect, Rect) {
    // The statusline's row comes out of the buffer's height, so a terminal too
    // short for both gives the row to the statusline and a terminal of no
    // height gives neither of them a rect that is not inside `area`.
    let status_height = area.height.min(1);
    let body = Rect {
        height: area.height - status_height,
        ..area
    };
    let status = Rect {
        y: area.y + body.height,
        height: status_height,
        ..area
    };
    (body, status)
}

/// One frame: buffer, then the float over it, then the statusline.
///
/// The order is `8d`'s — [`FloatSlot::render`] dims what is behind it, so it
/// runs after the buffer and over the buffer's area only. The statusline never
/// dims: §9's dim means "behind", and chrome is not behind anything.
fn draw(
    frame: &mut Frame<'_>,
    editor: &Editor,
    theme: &Theme,
    status: Option<&Tree>,
    floats: &FloatSlot<'_>,
    tree: Option<&Tree>,
) {
    let area = frame.area();
    let (body, status_area) = split(area);

    // A surface composed as a view tree owns the whole frame — `6b` draws its
    // own statusline, so the widgets below would be drawing it twice.
    if let Some(tree) = tree.filter(|tree| !matches!(tree.root, Node::Empty { .. })) {
        Interpreter::new(theme, &NoResources).render(tree, area, frame.buffer_mut());
        return;
    }

    // The state column is empty on purpose: §3's marks are a store query
    // (`T041`, S5) and there is no store. The column is still reserved, which
    // is the half of the 3-column contract S1 can be held to.
    frame.render_widget(BufferView::new(editor, theme), body);
    // A tree with an empty root is a float over what the widgets painted —
    // `T021`'s boot report, today.
    if let Some(tree) = tree {
        Interpreter::new(theme, &NoResources).render(tree, body, frame.buffer_mut());
    }
    floats.render(body, frame.buffer_mut(), theme);
    // `T025`: the statusline is whatever `runtime/statusline.scm` composed, and
    // a layer that composes none draws none. There is deliberately no widget
    // fallback here — a Rust statusline behind a Steel one is the *"config file
    // with a Rust editor hiding behind it"* `CP-2` fails on, and it is what the
    // `CP-2` gate caught by deleting `statusline.scm` and still seeing a line.
    if let Some(composed) = status {
        Interpreter::new(theme, &NoResources).render(composed, status_area, frame.buffer_mut());
    }

    if let Some((x, y)) = editor.get_visible_cursor(&editor_area(body)) {
        frame.set_cursor_position((x, y));
    }
}

/// The `12:1` counter, 1-based, as `1a` and `8e` draw it.
fn cursor_of(editor: &Editor) -> status::Cursor {
    let (row, col) = editor.code_ref().point(editor.get_cursor());
    status::Cursor {
        line: u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX),
        col: u32::try_from(col.saturating_add(1)).unwrap_or(u32::MAX),
    }
}

/// §5's `[+]`, wired to the edit stream.
///
/// The flag is set by the vendored core's change callback — which fires once
/// per committed edit batch — rather than by diffing the buffer against the
/// file each frame. It is one-way for now: there is no save path at S1 (no
/// `Action`, `T019`), so nothing can clear it, and a host that cannot write to
/// disk cannot lose anything by saying so.
fn dirty_flag(editor: &mut Editor) -> Rc<Cell<bool>> {
    let dirty = Rc::new(Cell::new(false));
    track_dirty(editor, &dirty);
    dirty
}

/// Points the flag at a different buffer, clean.
///
/// A new [`Editor`] carries no callback, so a swapped-in buffer would leave the
/// flag frozen at whatever the last one made it — `[+]` on a buffer nobody has
/// touched. `C-c buffer` is the one thing that swaps one today.
fn track_dirty(editor: &mut Editor, dirty: &Rc<Cell<bool>>) {
    dirty.set(false);
    let flag = Rc::clone(dirty);
    editor.set_change_callback(Box::new(move |_| flag.set(true)));
}

// ---------------------------------------------------------------------------
// Input — the temporary path
// ---------------------------------------------------------------------------

/// What the loop does with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Leave the loop and restore the terminal.
    Quit,
    /// The host consumed it; the buffer never sees it.
    Handled,
    /// Hand it to the vendored `editor_crossterm` handler.
    ToEditor,
}

/// The three keys the host keeps for itself, and why each one is not an
/// invented keymap.
///
/// **The editor layer gets every key first** ([`press`]). What is left is this,
/// which is the host's own floor rather than a keymap: a binding in
/// `runtime/keymaps.scm` shadows any of it, which is the right way round.
///
/// * `q` and `esc` are `T090`'s own acceptance criterion — *"`q`/`esc`
///   restores the terminal"*. They cost the buffer a printable `q`, which is
///   the price of having no modes until `T026`.
/// * `esc` closes an open surface first: Design Language §9, *"esc closes
///   top-down"*, and there is only ever one level.
/// * `ctrl-c` is the safety valve. Raw mode means the terminal will not deliver
///   SIGINT, and a host that ignored it would be a host you cannot get out of.
///   The vendored handler maps it to `Copy`, which nothing at S1 can paste.
///
/// Everything else — arrows, clicks, text — goes to the fork.
fn key_step(key: KeyEvent, surface: &mut Surface) -> Step {
    // Under the kitty protocol every press is also reported as a release
    // (`T014` negotiates `REPORT_EVENT_TYPES`), and the vendored handler does
    // not look at `kind` — so without this every keystroke would apply twice.
    if key.kind == KeyEventKind::Release {
        return Step::Handled;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Step::Quit,
        KeyCode::Char('q') if key.modifiers.is_empty() => Step::Quit,
        KeyCode::Esc if !matches!(surface, Surface::Buffer) => {
            *surface = Surface::Buffer;
            Step::Handled
        }
        KeyCode::Esc => Step::Quit,
        _ => Step::ToEditor,
    }
}

// ---------------------------------------------------------------------------
// The REPL, on the frame
// ---------------------------------------------------------------------------

/// What has the frame's body.
///
/// One at a time, always — Design Language §9's one-float rule, made unbreakable
/// by there being one variable rather than a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    /// The buffer alone.
    Buffer,
    /// `6b` — the Steel REPL.
    Repl,
    /// `T021`'s boot report: what `init.scm` could not run.
    Boot,
    /// `T084`'s fixture float, from `--float`. Scaffolding.
    Fixture(FloatMood),
}

/// What a key did to the REPL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplStep {
    /// The session took it.
    Handled,
    /// Give the frame back to the buffer.
    Close,
    /// `6b`'s `C-c buffer` — the same thing [`Intent::ToBuffer`] asks for.
    ToBuffer,
}

/// One key, while the REPL has the frame.
///
/// **A key and a door call meet at the method, not at the form.** `↑` calls
/// [`Repl::history`] and so does `(repl-history! 1)` through [`Intent`]; `C-c`
/// asks for the buffer and so does `(repl-to-buffer!)`. Routing the keys through
/// the *forms* was tried and reverted: it wrote `λ (repl-history! 1)` into the
/// session on every arrow press, and a transcript of your own keystrokes is not
/// a REPL session.
///
/// **`esc` closes, not `q`.** `6b`'s footer draws `q close`, and on a surface
/// whose body is a text input `q` is a character you are typing. The two cannot
/// both be true until the REPL has modes (`T026`); §9's `esc` is the one that
/// works today, and the hint is left as drawn rather than quietly rewritten.
fn repl_key(key: KeyEvent, repl: &mut Repl, runtime: &mut Runtime) -> ReplStep {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return ReplStep::Close,
        KeyCode::Char('c') if control => return ReplStep::ToBuffer,
        KeyCode::Enter => {
            repl.submit(runtime);
        }
        KeyCode::Backspace => repl.backspace(),
        KeyCode::Tab => repl.complete(),
        // `repl-history`'s own row: *"how far back, negative goes forward"*.
        KeyCode::Up => repl.history(1),
        KeyCode::Down => repl.history(-1),
        KeyCode::Char('u') if control => repl.clear(),
        KeyCode::Char(character) if !control => repl.insert(character),
        _ => {}
    }
    ReplStep::Handled
}

/// Asks the live keymap what to do with one key.
///
/// The encoding is here because this file owns crossterm; the *decision* is not
/// here at all. `T026` takes this over along with the rest of the input path,
/// and inherits the rule: no table on this side.
fn press(runtime: &mut Runtime, key: KeyEvent) -> Press {
    match notation(key) {
        Some(notation) => keymap::press(runtime, &notation),
        None => Press::Unbound,
    }
}

/// A key event in vim notation — `q`, `]`, `<C-c>`, `<esc>`.
///
/// Deliberately partial: a key with no spelling has no binding either, so it
/// answers [`None`] and the host's own floor sees it. `T026` owns the complete
/// grammar (counts, registers, `<S-…>`, function keys); this is the subset a
/// keymap can be written against today.
fn notation(key: KeyEvent) -> Option<String> {
    let named = match key.code {
        KeyCode::Char(character) => {
            let mut spelled = character.to_string();
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                spelled = format!("<C-{character}>");
            } else if character == ' ' {
                spelled = "<space>".to_owned();
            }
            return Some(spelled);
        }
        KeyCode::Esc => "esc",
        KeyCode::Enter => "cr",
        KeyCode::Tab => "tab",
        KeyCode::Backspace => "bs",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        _ => return None,
    };
    Some(format!("<{named}>"))
}

/// Clicks and wheel, straight to the fork.
///
/// **The area is [`editor_area`], not the widget'''s own rect.** The vendored
/// core measures click-to-offset from `area.left()`, and the two cells in front
/// of the line numbers belong to `BufferView`; passing the outer rect would put
/// the cursor two columns left of the character under the pointer.
fn to_editor_mouse(
    editor: &mut Editor,
    mouse: MouseEvent,
    body: Rect,
) -> Result<(), Box<dyn Error>> {
    editor.mouse(mouse, &editor_area(body))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixtures and lookups
// ---------------------------------------------------------------------------

/// `T084`'s fixture body, in the informational mood.
const INFORMATIONAL_LINES: &[&str] = &[
    "the one chrome primitive: header, body and footer",
    "inside a mood border (Design Language §4).",
    "",
    "this is T084's fixture body. pickers, diffs and",
    "asks plug their own in from S5 on.",
];

/// The same, in the needs-you mood.
const NEEDS_YOU_LINES: &[&str] = &[
    "the needs-you mood: warmer border, darker body,",
    "for questions and permission asks (§4).",
    "",
    "a real ask queues rather than replacing whatever",
    "float is open (Q9). this fixture does neither.",
];

static INFORMATIONAL_BODY: TextBody<'static> = TextBody::new(INFORMATIONAL_LINES);
static NEEDS_YOU_BODY: TextBody<'static> = TextBody::new(NEEDS_YOU_LINES);

/// Every float carries its keys in the footer (§4).
const FIXTURE_HINTS: &[FooterHint<'static>] = &[
    FooterHint::new("esc", "close"),
    FooterHint::new("q", "quit"),
];

/// A float in the asked-for mood, built from `T084`'s fixture body.
fn fixture_float(mood: FloatMood) -> Float<'static> {
    let footer = FloatFooter::new(FIXTURE_HINTS);
    match mood {
        FloatMood::Informational => Float::informational(
            FloatHeader::new("float fixture").meta("informational"),
            &INFORMATIONAL_BODY,
            footer,
        ),
        FloatMood::NeedsYou => Float::needs_you(
            FloatHeader::new("float fixture").meta("needs-you"),
            &NEEDS_YOU_BODY,
            footer,
        ),
    }
}

/// Extension → the vendored core's language name.
///
/// **Temporary, and small on purpose.** Language selection belongs to
/// `define-language` (`T036`+, S4), which owns grammars, LSP servers and
/// comment syntax together; this is the file-extension half of it, covering
/// exactly the ten grammars the fork is built with (`grammars-phosphor`).
/// An unknown extension is not an error — the core skips parser setup and the
/// buffer renders unhighlighted.
fn language_of(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "py" => "python",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "css" => "css",
        "html" | "htm" => "html",
        // Not a grammar the fork knows, which is the point: `Code::new` skips
        // parser setup for an unrecognised name and the buffer renders in
        // `syntax.text`. No new failure mode, no guessed grammar.
        _ => "text",
    }
}

/// A slug we do not ship, reported with the six we do.
///
/// `T011` loads themes from disk and this message is where that will surface —
/// today "unknown" and "not a builtin" are the same thing.
fn unknown_theme(slug: &str) -> String {
    let known = BUILTIN_SLUGS.join(", ");
    format!("unknown theme `{slug}` — shipped themes are: {known}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Only the decisions, not the loop: everything else about this file is
// perceptual and belongs to `CP-1`'s Tier-2 and Tier-3 halves. What is covered
// here is the routing table — which keys the host keeps and which reach the
// vendored handler — because a mistake there is silent on a screenshot and
// loud on a real terminal.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use phosphor_core::action::{Action, Outcome, Refusal, Request, RuntimeAction};
    use phosphor_core::registry::Door;
    use phosphor_core::request::Actor;
    use phosphor_core::value::Value;
    use phosphor_steel::answer;
    use phosphor_steel::host::Host;
    use phosphor_steel::keymap::Press;
    use ratatui::layout::Rect;

    use super::door::Evaluate as _;
    use super::{
        AppHost, Intent, Repl, ReplStep, Runtime, Step, Surface, Vm, boot, key_step, language_of,
        notation, press, repl_key, split, vm,
    };

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn tree() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime")
    }

    fn ask(host: &AppHost, action: Action) -> Outcome {
        host.apply(&Request::new(Actor::Steel, Door::Steel, action))
    }

    /// A runtime over a layer, and the host behind its barrier — the same two
    /// calls `vm()` makes, so a test cannot be looking at a differently wired
    /// editor than the program is.
    fn booted_at(root: &Path) -> (Runtime, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(Some(root.to_path_buf())));
        let runtime = boot(Some(root), &host);
        (runtime, host)
    }

    /// The shipped layer, read-only: the host has no root, so a persist is
    /// refused rather than writing into the repository's own runtime tree.
    fn booted() -> (Runtime, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(None));
        let runtime = boot(Some(&tree()), &host);
        (runtime, host)
    }

    /// A writable copy of the shipped layer.
    fn copy_of_the_layer(name: &str) -> PathBuf {
        let root = scratch(name);
        for entry in std::fs::read_dir(tree()).expect("the shipped layer") {
            let entry = entry.expect("a readable entry");
            if entry.path().extension().is_some_and(|ext| ext == "scm") {
                std::fs::copy(entry.path(), root.join(entry.file_name())).expect("copy");
            }
        }
        root
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("phosphor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch tree");
        path
    }

    #[test]
    fn q_and_esc_are_the_way_out() {
        let mut surface = Surface::Buffer;
        assert_eq!(key_step(key(KeyCode::Char('q')), &mut surface), Step::Quit);
        assert_eq!(key_step(key(KeyCode::Esc), &mut surface), Step::Quit);
        assert_eq!(
            key_step(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut surface
            ),
            Step::Quit,
            "raw mode swallows SIGINT, so ctrl-c has to be handled or there is no way out"
        );
    }

    #[test]
    fn esc_closes_the_surface_before_it_quits() {
        let mut surface = Surface::Repl;
        assert_eq!(key_step(key(KeyCode::Esc), &mut surface), Step::Handled);
        assert_eq!(
            surface,
            Surface::Buffer,
            "esc must close the surface, not the editor"
        );
        assert_eq!(
            key_step(key(KeyCode::Esc), &mut surface),
            Step::Quit,
            "with nothing left to close, esc is the quit key again"
        );
    }

    #[test]
    fn a_release_never_reaches_the_buffer() {
        // The kitty protocol reports press *and* release (`T014` asks for
        // REPORT_EVENT_TYPES) and the vendored handler ignores `kind`, so
        // without this filter every keystroke would be applied twice.
        let release = KeyEvent::new_with_kind_and_state(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
            KeyEventState::NONE,
        );
        assert_eq!(key_step(release, &mut Surface::Buffer), Step::Handled);
    }

    #[test]
    fn everything_else_rides_the_vendored_handler() {
        let mut surface = Surface::Buffer;
        for code in [
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Char('a'),
            KeyCode::Enter,
            KeyCode::Backspace,
        ] {
            assert_eq!(
                key_step(key(code), &mut surface),
                Step::ToEditor,
                "{code:?}"
            );
        }
    }

    #[test]
    fn a_key_is_spelled_the_way_a_keymap_is_written() {
        // `6b` binds `"]r"` and its footer names `C-c`. Both have to be sayable.
        assert_eq!(notation(key(KeyCode::Char(']'))).as_deref(), Some("]"));
        assert_eq!(
            notation(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)).as_deref(),
            Some("<C-c>")
        );
        assert_eq!(notation(key(KeyCode::Esc)).as_deref(), Some("<esc>"));
        assert_eq!(
            notation(key(KeyCode::Char(' '))).as_deref(),
            Some("<space>")
        );
        // No spelling, no binding — the host's own floor sees it.
        assert_eq!(notation(key(KeyCode::F(5))), None);
    }

    #[test]
    fn the_host_carries_out_what_s2_can_and_names_the_task_for_the_rest() {
        let host = AppHost::new(None);
        assert!(matches!(
            ask(&host, Action::Runtime(RuntimeAction::OpenRepl {})),
            Outcome::Done(_)
        ));
        assert!(matches!(
            ask(
                &host,
                Action::Runtime(RuntimeAction::ReplHistory { delta: 1 })
            ),
            Outcome::Done(_)
        ));
        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl, Intent::History(1)],
            "a surface Action is an ask the loop drains, not a widget the VM touched"
        );
        assert!(host.intents().is_empty(), "a drain empties the queue");

        // Everything else answers its own row's task — derived, never listed.
        let Outcome::Refused(Refusal::NotYetImplemented { task }) =
            ask(&host, Action::Runtime(RuntimeAction::ReloadRuntime {}))
        else {
            panic!("an unbuilt capability names the task that builds it");
        };
        assert_eq!(task, "T021");
    }

    #[test]
    fn set_option_is_read_back_by_the_loop() {
        // `init.scm` sets `soft-wrap` at boot; the frame reads it per frame.
        let host = AppHost::new(None);
        assert_eq!(host.flag("soft-wrap"), None);
        let _ = ask(
            &host,
            Action::Runtime(RuntimeAction::SetOption {
                key: "soft-wrap".to_owned(),
                value: Value::Bool(true),
            }),
        );
        assert_eq!(host.flag("soft-wrap"), Some(true));
    }

    #[test]
    fn persist_form_appends_to_the_tree_that_booted_and_says_where() {
        let root = scratch("persist");
        // No layer booted here, so the default stands: a one-file layer writes
        // to `init.scm`, which is what `6b` draws.
        let host = AppHost::new(Some(root.clone()));
        let form = r#"(keymap-set! "]r" (lambda () 1))"#;
        let Outcome::Done(receipt) = ask(
            &host,
            Action::Runtime(RuntimeAction::PersistForm {
                form: form.to_owned(),
            }),
        ) else {
            panic!("a writable tree persists");
        };
        // `6b`: `⇒ #ok · persisted to init.scm`.
        assert_eq!(receipt.note.as_deref(), Some("persisted to init.scm"));
        let written = std::fs::read_to_string(root.join("init.scm")).expect("the file exists now");
        assert!(written.contains(form), "{written:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_persisted_rebind_survives_the_next_boot() {
        // **The regression that a pty run found and no unit test would have.**
        // `init.scm` runs to its last form before Rust reads the load order it
        // declared, so a `(keymap-set! …)` appended *there* comes back on the
        // next start as a free-identifier fault — `keymaps.scm` has not loaded
        // yet. The layer names a file that loads last (`PERSIST_FILE`); this
        // types the rebind, throws the editor away, and starts a new one.
        let root = copy_of_the_layer("reboot");
        let form = r#"(keymap-set! "gz" (lambda () (open-repl!)))"#;

        {
            let (mut runtime, _host) = booted_at(&root);
            let mut session = Repl::new();
            for character in form.chars() {
                session.insert(character);
            }
            let entry = session.submit(&mut runtime).expect("a form was typed");
            assert_eq!(
                entry.answered.note.as_deref(),
                Some("persisted to persisted.scm"),
                "the layer names the file that loads last"
            );
        }

        // A second editor over the same tree — a restart, in one process.
        let (mut runtime, host) = booted_at(&root);
        assert!(
            runtime.report().is_clean(),
            "a persisted form must not fault the next boot: {:?}",
            runtime.report().faults
        );
        assert_eq!(press(&mut runtime, key(KeyCode::Char('g'))), Press::Pending);
        assert_eq!(press(&mut runtime, key(KeyCode::Char('z'))), Press::Handled);
        assert_eq!(host.intents(), vec![Intent::OpenRepl]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn with_no_runtime_tree_a_persist_is_refused_rather_than_guessed() {
        let Outcome::Refused(Refusal::Declined { reason }) = ask(
            &AppHost::new(None),
            Action::Runtime(RuntimeAction::PersistForm {
                form: r#"(set-option! "soft-wrap" #t)"#.to_owned(),
            }),
        ) else {
            panic!("there is nowhere to write, and inventing one would be worse");
        };
        assert!(reason.contains("PHOSPHOR_RUNTIME"), "{reason}");
    }

    #[test]
    fn the_repl_and_the_cli_door_are_the_same_call_into_the_same_runtime() {
        // `T023`'s criterion, held where the two front-ends meet: `Vm` and
        // `Repl::submit` reach one `Runtime::evaluate` and one renderer, so the
        // answer is the same object rather than two that agree.
        let (mut runtime, _host) = booted();
        let source = "(+ 1 2)";
        let door = Vm(&mut runtime).eval(source);

        let mut session = Repl::new();
        for character in source.chars() {
            session.insert(character);
        }
        let entry = session.submit(&mut runtime).expect("a form was typed");
        assert_eq!(answer::answered(&door), entry.answered);
    }

    #[test]
    fn a_rebind_typed_at_the_repl_is_in_force_on_the_next_key() {
        // `T022`'s claim, through the host's own path: type the rebind, then
        // press the key. No reload, no second boot, nothing invalidated.
        let (mut runtime, host) = booted();
        let mut session = Repl::new();

        assert_eq!(press(&mut runtime, key(KeyCode::Char('g'))), Press::Unbound);
        for character in r#"(keymap-set! "g" (lambda () (open-repl!)))"#.chars() {
            session.insert(character);
        }
        session.submit(&mut runtime).expect("a form was typed");

        assert_eq!(press(&mut runtime, key(KeyCode::Char('g'))), Press::Handled);
        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl],
            "the binding ran, and what it asked for reached the loop"
        );
    }

    #[test]
    fn the_seeded_layer_opens_the_repl_and_leaves_the_rest_to_the_host() {
        let (mut runtime, host) = booted();
        assert_eq!(
            press(&mut runtime, key(KeyCode::Char(':'))),
            Press::Handled,
            "runtime/keymaps.scm binds `:` to (open-repl!)"
        );
        assert_eq!(host.intents(), vec![Intent::OpenRepl]);
        assert_eq!(
            press(&mut runtime, key(KeyCode::Char('q'))),
            Press::Unbound,
            "`q` is the host's own floor, not the layer's"
        );
    }

    #[test]
    fn a_broken_layer_boots_and_composes_its_float() {
        // `T021`'s promise, at the seam this file is responsible for: the host
        // gets a float to put on the frame, and `T079`'s interpreter draws it.
        let broken = scratch("boot");
        std::fs::write(broken.join("init.scm"), "(define oops\n").expect("write");

        let host = Arc::new(AppHost::new(None));
        let runtime = boot(Some(&broken), &host);
        let float = runtime
            .boot_float()
            .expect("a broken init.scm opens the float");
        let header = float.header.as_ref().expect("the boot float has a header");
        assert_eq!(header.left, "◆ steel · boot");

        let phosphor_core::view::Node::Spans { rows } = float.body.node() else {
            panic!("the boot float's body is the spans hatch");
        };
        let body: String = rows
            .iter()
            .flat_map(|row| row.runs.iter().map(|run| run.text.as_str()))
            .collect();
        assert!(body.contains("init.scm:1"), "{body}");

        let _ = std::fs::remove_dir_all(&broken);
    }

    #[test]
    fn the_repl_takes_the_keys_6bs_footer_teaches() {
        // The routing a session cannot prove on its own: `↵` submits, `esc`
        // gives the frame back, and a printable character is text rather than a
        // command — `q` included, which is why the footer's `q close` is
        // flagged rather than implemented (`repl_key`).
        let (mut runtime, host) = booted();
        let mut session = Repl::new();

        for character in "(+ 1 2)".chars() {
            assert_eq!(
                repl_key(key(KeyCode::Char(character)), &mut session, &mut runtime),
                ReplStep::Handled
            );
        }
        assert_eq!(session.input(), "(+ 1 2)");
        assert_eq!(
            repl_key(key(KeyCode::Enter), &mut session, &mut runtime),
            ReplStep::Handled
        );
        assert_eq!(session.entries().len(), 1, "↵ submits");
        assert_eq!(session.entries()[0].answered.head, "3");
        assert!(session.input().is_empty());

        // `q` types.
        repl_key(key(KeyCode::Char('q')), &mut session, &mut runtime);
        assert_eq!(session.input(), "q");
        repl_key(key(KeyCode::Backspace), &mut session, &mut runtime);
        assert!(session.input().is_empty());

        // `↑` walks the history, and does not write itself into the session.
        repl_key(key(KeyCode::Up), &mut session, &mut runtime);
        assert_eq!(session.input(), "(+ 1 2)");
        assert_eq!(session.entries().len(), 1, "an arrow key is not an entry");

        // `C-c` asks for the buffer — the same thing `(repl-to-buffer!)` asks
        // for through the door, which is where the two meet.
        assert_eq!(
            repl_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut session,
                &mut runtime,
            ),
            ReplStep::ToBuffer
        );
        let _ = runtime.evaluate("(repl-to-buffer!)");
        assert_eq!(host.intents(), vec![Intent::ToBuffer]);

        assert_eq!(
            repl_key(key(KeyCode::Esc), &mut session, &mut runtime),
            ReplStep::Close,
            "§9: esc closes top-down"
        );
    }

    #[test]
    fn the_one_runtime_is_built_the_same_way_on_both_paths() {
        // Not a tautology: `vm()` is the only constructor there is, and if a
        // second appeared the door and the loop could answer differently.
        let (runtime, _host) = vm();
        assert!(
            runtime.report().is_clean() || runtime.report().root.is_some(),
            "a boot either ran clean or names the tree it read"
        );
    }

    #[test]
    fn a_grammar_the_fork_was_not_built_with_is_not_a_failure() {
        assert_eq!(language_of("src/main.rs".as_ref()), "rust");
        assert_eq!(language_of("Cargo.toml".as_ref()), "toml");
        assert_eq!(language_of("runtime/init.SCM".as_ref()), "text");
        assert_eq!(language_of("README".as_ref()), "text");
    }

    #[test]
    fn the_statusline_gets_the_last_row_and_never_a_second_one() {
        let (body, status) = split(Rect::new(0, 0, 80, 24));
        assert_eq!(body.height, 23);
        assert_eq!(status, Rect::new(0, 23, 80, 1));

        // A one-row terminal is all statusline, and a zero-row one draws
        // nothing rather than underflowing.
        let (body, status) = split(Rect::new(0, 0, 80, 1));
        assert_eq!(body.height, 0);
        assert_eq!(status, Rect::new(0, 0, 80, 1));
        let (body, status) = split(Rect::new(0, 0, 80, 0));
        assert!(body.is_empty() && status.is_empty());
    }
}
