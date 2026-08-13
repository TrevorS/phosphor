//! The phosphor binary: terminal setup, the event loop, input routing and panes.
//!
//! This is the app layer — the one place `ratatui` and `crossterm` are allowed.
//! It owns the frame: Steel decides what is on screen, this decides when pixels
//! land (Q12). Input is decoded here into Actions; nothing else emits them.
//!
//! Owned by `spine`. `T090` built the shell of it; `T026` built the loop.
//!
//! # `T026` — the input machine, and what it deleted
//!
//! `T090` built the S1 host on the vendored core's own `editor_crossterm`
//! handler, and `TASKS.md` gave that a demolition date. This is it. **Three
//! things went, rather than being wrapped:**
//!
//! 1. **`Editor::input` and `Editor::mouse`.** Both ended every keystroke with
//!    the fork's own `focus()`, and `mouse` called `scroll_up`/`scroll_down`
//!    directly — so the viewport had two writers and invariant 3 did not hold.
//!    Every key now goes through [`Machine`], every mouse event lowers to an
//!    Action, and the only thing that moves a viewport is
//!    `View::Scroll` ([`Editing::act`]).
//! 2. **`T022`'s per-keystroke dispatch.** The loop asked the keymap and then
//!    handed whatever was left to the fork. The keymap is still asked on every
//!    key and still never cached — that is `T022`'s liveness claim and it is
//!    untouched — but it is asked *through* [`Layered`], as one of the
//!    two tables the machine resolves against.
//! 3. **The three lines in `Cargo.toml`** that turned the fork's `crossterm`
//!    feature on. Nothing in this binary reads a `crossterm` event through the
//!    fork any more, and the build proves it.
//!
//! # The one rule the loop obeys and no test could see
//!
//! *Anything that runs arbitrary scheme may move state the statusline composer
//! reads without moving the ViewModel, so the frame cache has to be
//! invalidated.* `CP-2` found the keybinding half of that missing **by running
//! it** — a key bound to `(status-order-set! 'right '())` fired and the frame
//! that followed wrote no cells — and the fix was correct by *remembering*,
//! which is the weakest kind.
//!
//! It is structural now. [`Layer`] is the only path into the VM this file has,
//! and **every method on it that runs user scheme sets one flag**;
//! [`Layer::stale`] reads it, in exactly one place, at the top of the loop.
//! ([`Layer::resolve`] sets it when a binding actually fired rather than on every
//! key it is asked about, and its own doc argues why that is the rule and not a
//! loophole.)
//! There is no accessor handing out the `Runtime`, so a new way to enter the VM
//! has to be a new method here — and the shape of every method here is to set
//! the flag. `scripts/lint-one-vm-door.sh` fails the build if that stops being
//! true, and `arbitrary_scheme_marks_the_frame_stale` tests both halves.
//!
//! Composition is the deliberate exception ([`Layer::compose`]): invalidating
//! on the call that fills the cache would refill it every frame, which is the
//! `T079` regression the cache exists to prevent.
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
//! # What the loop owes the machine
//!
//! Four things only a running loop can hold, and each is a rule rather than a
//! detail:
//!
//! 1. **The machine reads the buffer; the loop writes it.** [`EditorText`] is
//!    the read-only window `phosphor_core::input::text::Text` asks for, and
//!    [`Editing::act`] is the only thing that mutates. A motion resolves in
//!    `phosphor-core` and is *applied* here, so `w` and `dw` cannot disagree
//!    about where a word ends.
//! 2. **A cursor that moved is revealed by a request, not by drawing.**
//!    [`Editing::reveal`] emits `View::Scroll { RevealRow … }` and applies it
//!    through the same match as every other Action, so *"the viewport moves
//!    from one place"* stays true with the cursor following it.
//! 3. **`.` and `feed-keys` re-enter this loop, not the machine.** Both are
//!    Actions; [`Session::key`] queues the keys they name and applies each
//!    key's Actions before feeding the next, because a replay computed against
//!    a buffer that has not moved yet edits the wrong span.
//! 4. **Dirty is per buffer and comes from the edit stream**, not from a save
//!    path — see [`dirty_flag`].
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
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use phosphor_core::action::{
    Action, AppAction, BufferAction, FileAction, HistoryAction, InputAction, MotionAction, Outcome,
    PromptAction, Receipt, Refusal, Request, RuntimeAction, ViewAction,
};
use phosphor_core::input::key::{Code, Key, Mods, Named};
use phosphor_core::input::table::{Keymap, Layered, Resolution, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::input::{Machine, key, text as motion};
use phosphor_core::query::{Answer, Answers, Query, QueryError, Revision};
use phosphor_core::request::{
    EditMode, Position, PromptKind, RegisterName, SelectionKind, Span, Target,
};
use phosphor_core::value::Value;
use phosphor_core::view::{Child, Emphasis, Node, SessionState, Tone, Tree};
use phosphor_steel::host::Host;
use phosphor_steel::keymap::{self, Ex};
use phosphor_steel::repl::Repl;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, ComposeError, StatusFile, StatusVm};
use phosphor_term::{Frame, Term};
use phosphor_ui::buffer_view::{self, BufferView, Editor, editor_area};
use phosphor_ui::float::{Float, FloatFooter, FloatHeader, FloatSlot, FooterHint, TextBody};
use phosphor_ui::frame::FrameCache;
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::{BUILTIN_SLUGS, Theme, builtin};
use ratatui::layout::Rect;
// The widget layer's re-export, not the fork's own path: after `T026` this file
// no longer talks to the vendored *handler* at all, only to the editor value
// `BufferView` draws. The two fork imports that remain are undo (`T029` takes
// them) and the selection type `SelectRange` sets.
use ratatui_code_editor::actions::{Redo, Undo};
use ratatui_code_editor::selection::Selection;

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
                  synchronized-output block.\n\nModes, counts, named registers, operators and \
                  text objects are the input machine's (T026); the keymap is asked of \
                  runtime/keymaps.scm on every keystroke and falls back to the seed table \
                  T033 replaces. `ZQ` or `ctrl-c` leaves — `:q` arrives with the ex \
                  commands. There is no save path and no agent session yet."
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
#[derive(Debug, Clone, PartialEq, Eq)]
enum Intent {
    /// `(open-repl!)` — `6b`.
    OpenRepl,
    /// `(close-repl!)`.
    CloseRepl,
    /// `(repl-history! delta)` — positive walks back.
    History(i64),
    /// `(repl-to-buffer!)` — `6b`'s `C-c buffer`.
    ToBuffer,
    /// `set-keybinding` / `remove-keybinding` arriving from the CLI or MCP
    /// door, as the `(keymap-set! …)` form the editor layer would have been
    /// typed. **The table has one writer** — `runtime/keymaps.scm` — so the
    /// other two doors reach it by writing scheme rather than by holding a
    /// second table (`T033`).
    Keymap(String),
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
/// `a_persisted_rebind_survives_the_next_boot`, in this file's `tests` module.
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
            // Invariant 2, on the keymap: the Steel door types
            // `(keymap-set! …)` directly and these two carry the CLI's and
            // MCP's version of the same form into the same table.
            Action::Runtime(RuntimeAction::SetKeybinding {
                keys,
                binding,
                mode,
            }) => {
                self.ask(Intent::Keymap(bind_form(keys, binding, mode.as_ref())));
                done(Value::Null)
            }
            Action::Runtime(RuntimeAction::RemoveKeybinding { keys, mode }) => {
                self.ask(Intent::Keymap(format!(
                    "(keymap-remove! {} {})",
                    scheme_text(&keys.0),
                    scheme_text(Scope::of(mode.unwrap_or(EditMode::Normal)).name()),
                )));
                done(Value::Null)
            }
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

/// One `set-keybinding` as the form the editor layer would have been typed.
///
/// **A door that cannot type scheme cannot bind a key, and that is the point.**
/// `request::Binding` is either a capability with arguments or scheme source
/// precisely because no `SteelVal` may ride in a payload; both spellings land
/// in `runtime/keymaps.scm`'s own vocabulary here, so there is one table and
/// one writer of it.
fn bind_form(
    keys: &phosphor_core::request::KeySeq,
    binding: &phosphor_core::request::Binding,
    mode: Option<&EditMode>,
) -> String {
    use phosphor_core::request::Binding;
    let scope = Scope::of(mode.copied().unwrap_or(EditMode::Normal));
    let what = match binding {
        Binding::Capability { name, args } => {
            let mut spelled = format!("(key/run (key/cmd {}", scheme_text(name));
            for (field, value) in args.iter() {
                spelled.push_str(&format!(" {} {}", scheme_text(field), scheme_value(value)));
            }
            spelled.push_str("))");
            spelled
        }
        Binding::Source { source } => format!("(lambda () {source})"),
    };
    format!(
        "(keymap-set! {} {what} \"\" {})",
        scheme_text(&keys.0),
        scheme_text(scope.name()),
    )
}

/// A wire value as scheme source.
///
/// The inverse of `phosphor-steel`'s `convert::from_steel`, written here
/// because it is the app layer that has to *say* a form rather than evaluate
/// one. Records become hashes and lists become lists, which is exactly the
/// table that module documents.
fn scheme_value(value: &Value) -> String {
    match value {
        Value::Null => "void".to_owned(),
        Value::Bool(flag) => (if *flag { "#true" } else { "#false" }).to_owned(),
        Value::Int(number) => number.to_string(),
        Value::Text(text) => scheme_text(text),
        Value::List(items) => {
            let spelled: Vec<String> = items.iter().map(scheme_value).collect();
            format!("(list {})", spelled.join(" "))
        }
        Value::Record(args) => {
            let spelled: Vec<String> = args
                .iter()
                .map(|(name, field)| format!("{} {}", scheme_text(name), scheme_value(field)))
                .collect();
            format!("(hash {})", spelled.join(" "))
        }
    }
}

/// A string literal, with the two characters scheme's reader cares about
/// escaped.
fn scheme_text(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

// ---------------------------------------------------------------------------
// The one door into the VM
// ---------------------------------------------------------------------------

/// The editor layer, and **the only place this file may enter it**.
///
/// The rule this type exists for is in the module header: anything that runs
/// arbitrary scheme may move state the statusline composer reads without moving
/// the ViewModel. Every method below that can run user scheme sets `ran`;
/// [`Layer::stale`] is read once per turn of the loop. The `Runtime` is private
/// and **nothing hands out a `&mut Runtime`** — that absence is the structure,
/// and `scripts/lint-one-vm-door.sh` is what keeps it absent.
struct Layer {
    runtime: Runtime,
    /// Whether arbitrary scheme has run since the last [`Layer::stale`].
    ran: bool,
}

impl Layer {
    const fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            ran: false,
        }
    }

    /// Evaluates source — the REPL, `--eval`, a keybinding's own form.
    fn evaluate(&mut self, source: &str) -> Outcome {
        self.ran = true;
        self.runtime.evaluate(source)
    }

    /// Submits what the REPL has typed. Runs it, by definition.
    fn submit<'a>(&mut self, repl: &'a mut Repl) -> Option<&'a phosphor_steel::repl::Entry> {
        self.ran = true;
        repl.submit(&mut self.runtime)
    }

    /// Asks the live keymap what a sequence plays in `scope`.
    ///
    /// **The flag is set when a binding *ran*, not when the VM was entered.**
    /// Every key is asked, so an unconditional flag would compose the statusline
    /// once per keystroke and the frame cache would mean nothing — the exact
    /// cost `Q12` and `T079` exist to remove (*"Steel runs at the rate of state
    /// change rather than the rate of frames"*). `Pending`, `Unbound` and a
    /// role ran only `phosphor/resolve` itself, which reads the table and
    /// touches nothing the composer reads; `Ran` ran a thunk that could have set
    /// anything. That is `CP-2`'s finding, kept as the rule and moved to the one
    /// place that records it.
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution {
        let answered = keymap::resolve(&mut self.runtime, scope, keys);
        self.ran |= answered == Resolution::Ran;
        answered
    }

    /// Runs an ex line — `T033`'s command table, which is scheme all the way
    /// down. Counts for the same reason a thunk does: a command may be one.
    fn ex(&mut self, line: &str) -> Ex {
        self.ran = true;
        keymap::ex(&mut self.runtime, line)
    }

    /// The statusline, composed. **The deliberate exception** (module header):
    /// marking the frame stale on the call that fills the cache would refill it
    /// every frame, which is the cost `T079`'s cache exists to remove.
    fn compose(&mut self, vm: &StatusVm) -> Result<Node, ComposeError> {
        status::compose(&mut self.runtime, vm)
    }

    /// `T021`'s boot report, as a float. Reads a value; runs nothing.
    fn boot_float(&self) -> Option<phosphor_core::view::Float> {
        self.runtime.boot_float()
    }

    /// The boot report itself. Test-only: what the *program* does with a
    /// fault is open the float ([`Layer::boot_float`]), and a second reader of
    /// the same facts in the loop would be a second place to keep in step.
    #[cfg(test)]
    fn report(&self) -> &phosphor_steel::boot::BootReport {
        self.runtime.report()
    }

    /// Whether arbitrary scheme has run since this was last asked. Clears.
    fn stale(&mut self) -> bool {
        core::mem::take(&mut self.ran)
    }
}

/// The evaluator the CLI door takes, over the one layer this process has.
struct Vm<'a>(&'a mut Layer);

impl door::Evaluate for Vm<'_> {
    fn eval(&mut self, source: &str) -> Outcome {
        self.0.evaluate(source)
    }
}

/// The editor layer as a [`Keymap`] — the whole keymap, since `T033`.
///
/// **The whole sequence, every time.** `T022`'s dispatcher kept a half-typed
/// sequence of its own and was handed one key at a time; the machine keeps one
/// too, and two copies can only disagree. `runtime/keymaps.scm` is stateless
/// now, so this passes what the machine holds and the layer answers a complete
/// question — which is also what lets it answer per *scope*, so `w` is a motion
/// in normal mode and a word object after `i`.
///
/// It answers [`Resolution::Role`] for a binding that is data and
/// [`Resolution::Ran`] for one that is a thunk, which is the seam `T026` left
/// for this task: a role crosses the barrier, a closure never does.
struct LayerKeymap<'a> {
    layer: &'a mut Layer,
}

impl std::fmt::Debug for LayerKeymap<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LayerKeymap")
    }
}

impl Keymap for LayerKeymap<'_> {
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution {
        self.layer.resolve(scope, keys)
    }
}

/// The editor layer, booted, and the host behind its barrier.
///
/// **One constructor, both paths.** `--eval` and the loop call this and nothing
/// else, so the CLI door and the REPL are answering out of the same VM with the
/// same host — which is what makes `T023`'s *"identical results for the same
/// expression"* structural rather than a thing to keep checking.
fn vm() -> (Layer, Arc<AppHost>) {
    let root = Runtime::root();
    let host = Arc::new(AppHost::new(root.clone()));
    let runtime = boot(root.as_deref(), &host);
    (Layer::new(runtime), host)
}

/// Boots the editor layer against `host`, and reads back where it writes.
///
/// The read happens **once, after the boot**: the layer decides the file
/// ([`PERSIST_FILE`]), and the host is behind the barrier and may not re-enter
/// the VM to ask when a form arrives.
fn boot(root: Option<&Path>, host: &Arc<AppHost>) -> Runtime {
    let runtime = Runtime::boot(root, Arc::clone(host) as Arc<dyn Host>);
    if let Ok(value) = runtime.global(PERSIST_FILE)
        && let Ok(Value::Text(file)) = phosphor_steel::convert::from_steel(&value)
    {
        host.persist_to(&file);
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
    let (mut layer, host) = vm();
    let boot = layer.boot_float();

    let (mut editor, path) = match &cli.path {
        Some(file) => {
            let text = std::fs::read_to_string(file)
                .map_err(|err| format!("{}: {err}", file.display()))?;
            // The path as the user typed it. Repo-relative is what the mockups
            // draw, but the repo root is `phosphor-vcs`'s answer (`T071`) and
            // inventing one here would be a value nobody asked for.
            (
                buffer(language_of(file), &text, &theme)?,
                Some(file.clone()),
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

    // `T026`'s machine, and — since `T033` — **an empty seed**. Every binding
    // in the editor is in `runtime/keymaps.scm`; the fallback table is still
    // here because `table::Layered` is what gives an operator's operand back to
    // the grammar, and a layer that rebinds `w` must not swallow the `w` in
    // `dw`. Filling it from Rust is what `no_bindings_in_rust.rs` fails on.
    let mut machine = Machine::new();
    let mut seed = Table::new();
    let mut editing = Editing::new(editor, path, Rc::clone(&dirty));

    // `T033`'s ex line, and the one line of chrome that answers it. Both live
    // here rather than in a widget: `view::Node::Prompt` is the vocabulary's
    // shape for this and `phosphor-ui` defers it to `T058`, so what S3 can hold
    // is the primitives — a row of labels where the statusline goes, which is
    // where vim puts it too.
    let mut ex_line = String::new();
    let mut notice: Option<String> = None;

    let mut term = Term::new()?;
    loop {
        // The size the *next* frame will be laid out at. `draw` re-splits
        // `frame.area()` itself, so this is only for what needs `&mut editor`
        // and therefore cannot happen inside the closure: the wrap width, and
        // the area a scroll is measured against.
        let size = term.size()?;
        let (body, _status) = split(Rect::new(0, 0, size.width, size.height));
        editing.area = editor_area(body);
        // `init.scm` sets `soft-wrap` at boot and `(set-option! …)` can change
        // it at the REPL, so it is read per frame rather than once: the option
        // is the editor layer's, and the flag is the override.
        if cli.soft_wrap || host.flag("soft-wrap") == Some(true) {
            // Free when the width has not changed, and it moves no viewport.
            soft_wrap::wrap_to(&mut editing.editor, body);
        }
        // `8e`'s whitespace marks are INSERT-only, and the mode is the
        // machine's — the first thing in this loop that is not hardcoded.
        //
        // Converted at the boundary: `soft_wrap::EditMode` is two values and
        // says of itself *"the real mode enum is `spine`'s and does not exist
        // yet (`T026`)"*. It does now, and collapsing the two deletes a
        // `surface`-owned type — a request, not an edit `spine` makes here.
        soft_wrap::set_mode(
            &mut editing.editor,
            if machine.mode() == EditMode::Insert {
                soft_wrap::EditMode::Insert
            } else {
                soft_wrap::EditMode::Normal
            },
        );

        // **The one place the frame cache learns that arbitrary scheme ran.**
        // Not per call site, not by remembering: `Layer` is the only way into
        // the VM and every method on it that can run user scheme sets the flag
        // this reads. `CP-2` found the keybinding half of the old rule missing
        // by running it; this is what makes that unfindable-by-running rather
        // than merely fixed.
        if layer.stale() {
            status_cache.invalidate();
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
            mode: mode_word(machine.mode()).to_owned(),
            surface: None,
            file: editing.file.clone().map(|path| StatusFile {
                path,
                dirty: dirty.get(),
            }),
            // Truthful, and the truth at S3 is that there is no session, no
            // store to count unseen regions in, and no VCS adapter. `T050`,
            // `T041` and `T071` fill these in; a fixture here would be a lie
            // on a real terminal.
            session: SessionState::None,
            since: None,
            ask_pending: false,
            unseen: 0,
            vcs: None,
            cursor: Some(cursor_of(&editing.editor)),
            hints: Vec::new(),
        };

        // `T079`'s cache, on the path that ships. The revision stands in for the
        // store's: at S3 the statusline's facts *are* its state, so a revision
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
        let composed = match status_cache.try_update(revision, || layer.compose(&vm).map(Tree::new))
        {
            Ok(_) => true,
            Err(ComposeError::Unbound) => false,
            // Keep whatever last composed successfully; nothing, if that is
            // what there was.
            Err(_) => !matches!(status_cache.tree().root, Node::Empty { .. }),
        };
        let status_tree = composed.then(|| status_cache.tree());

        // What is on the statusline's row instead of the statusline. The ex
        // line takes it while it is open — vim's own placement — and a notice
        // borrows it until the next key.
        let typed = format!(":{ex_line}");
        let chrome = if matches!(surface, Surface::Ex) {
            Some(Chrome {
                text: &typed,
                caret: true,
            })
        } else {
            notice.as_deref().map(|text| Chrome { text, caret: false })
        };

        term.draw(|frame| {
            draw(
                frame,
                &editing.editor,
                &theme,
                status_tree,
                &floats,
                tree.as_ref(),
                chrome,
            );
        })?;

        let event = event::read()?;
        // A notice says what the last ex line did, and the next key is the
        // acknowledgement — there is no dismiss and nothing to remember.
        if matches!(event, Event::Key(_)) {
            notice = None;
        }
        match event {
            Event::Key(key) if !is_press(key) => {}
            Event::Key(key) if matches!(surface, Surface::Repl) => {
                match repl_key(key, &mut repl, &mut layer) {
                    ReplStep::Handled => {}
                    ReplStep::Close => surface = Surface::Buffer,
                    ReplStep::ToBuffer => {
                        editing.editor = session_buffer(&repl, &theme)?;
                        track_dirty(&mut editing.editor, &dirty);
                        surface = Surface::Buffer;
                    }
                }
            }
            // §9: esc closes top-down, and a float that is not a surface of its
            // own is closed here before the machine ever sees the key. There is
            // only ever one level (`Surface`).
            Event::Key(key) if closes_surface(key, surface) => surface = Surface::Buffer,
            // The ex line owns every key while it is open, which is what makes
            // it a line editor rather than a mode of the machine.
            Event::Key(key) if matches!(surface, Surface::Ex) => match ex_key(key, &mut ex_line) {
                ExStep::Typing => {}
                ExStep::Cancel => surface = Surface::Buffer,
                ExStep::Submit => {
                    surface = Surface::Buffer;
                    notice = submit_ex(&mut layer, &mut editing, &ex_line);
                }
            },
            // **Every key, through one machine.** The keymap is still asked of
            // the VM on every keystroke and still never cached (`T022`); what
            // changed is that the answer is one of two tables the machine
            // resolves against, and what is left over is a grammar rather than
            // the fork's handler.
            Event::Key(key) => {
                if let Some(pressed) = decode(key) {
                    Session {
                        machine: &mut machine,
                        layer: &mut layer,
                        seed: &mut seed,
                        editing: &mut editing,
                    }
                    .key(pressed);
                }
            }
            Event::Mouse(mouse) => {
                for action in mouse_actions(&editing.editor, mouse, editing.area) {
                    let _ = editing.apply(&action);
                }
            }
            // A resize redraws from the new size on the next turn of the loop;
            // so does everything else this arm swallows (focus, paste).
            _ => {}
        }

        // What the Actions asked for that only the loop can do: `open-file`
        // needs the theme and the language table, and `open-prompt` needs the
        // surface. Both are recorded by `Editing::act` and performed here, for
        // the same reason `Intent` exists — the thing that decides is not the
        // thing that owns.
        if let Some(file) = editing.open.take() {
            match std::fs::read_to_string(&file) {
                Ok(text) => {
                    editing.editor = buffer(language_of(&file), &text, &theme)?;
                    track_dirty(&mut editing.editor, &dirty);
                    editing.file = Some(file);
                    surface = Surface::Buffer;
                }
                Err(error) => notice = Some(format!("{}: {error}", file.display())),
            }
        }
        if editing.prompt.take().is_some() {
            ex_line.clear();
            surface = Surface::Ex;
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
                    editing.editor = session_buffer(&repl, &theme)?;
                    track_dirty(&mut editing.editor, &dirty);
                    surface = Surface::Buffer;
                }
                // The CLI and MCP doors, arriving in the editor layer's own
                // words. `Layer` runs it, so the frame cache learns that
                // arbitrary scheme ran from the one place that records it.
                Intent::Keymap(form) => {
                    if let Outcome::Refused(refusal) = layer.evaluate(&form) {
                        notice = Some(phosphor_steel::answer::why(&refusal));
                    }
                }
            }
        }

        if editing.quit {
            break;
        }
    }

    term.restore()?;
    Ok(())
}

/// The word the chip draws, per mode.
///
/// `runtime/statusline.scm` keeps the table of words to actor fields and
/// upper-cases anything it does not know, so these are *names* rather than
/// labels — `v-line` is `9c`'s `V-LINE` and `pending` is `3c`'s pending state.
/// Widening that table is the editor layer's edit, not this file's.
const fn mode_word(mode: EditMode) -> &'static str {
    match mode {
        EditMode::Normal => "normal",
        EditMode::Insert => "insert",
        EditMode::Replace => "replace",
        EditMode::VisualChar => "visual",
        EditMode::VisualLine => "v-line",
        EditMode::VisualBlock => "v-block",
        EditMode::OperatorPending => "pending",
    }
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
    chrome: Option<Chrome<'_>>,
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
    match chrome {
        // The ex line and the notice both take the statusline's row rather
        // than a row of their own: `8d`'s ladder is about a line that has to
        // fit, and two lines of chrome is a different frame.
        Some(chrome) => {
            let row = Tree::new(Node::Line {
                children: vec![Child::new(Node::Label {
                    text: chrome.text.to_owned(),
                    tone: Tone::Text,
                    emphasis: Emphasis::Plain,
                })],
            });
            Interpreter::new(theme, &NoResources).render(&row, status_area, frame.buffer_mut());
        }
        None => {
            if let Some(composed) = status {
                Interpreter::new(theme, &NoResources).render(
                    composed,
                    status_area,
                    frame.buffer_mut(),
                );
            }
        }
    }

    match chrome.filter(|chrome| chrome.caret) {
        Some(chrome) => {
            let typed = u16::try_from(chrome.text.chars().count()).unwrap_or(u16::MAX);
            let x = status_area
                .x
                .saturating_add(typed)
                .min(status_area.right().saturating_sub(1));
            frame.set_cursor_position((x, status_area.y));
        }
        None => {
            if let Some((x, y)) = editor.get_visible_cursor(&editor_area(body)) {
                frame.set_cursor_position((x, y));
            }
        }
    }
}

/// One row of text where the statusline goes.
///
/// Two callers, one shape: the ex line while it is open, and the notice that
/// says what it did. `caret` is what separates them — a line you are typing
/// into has the cursor, and a line that is telling you something does not.
#[derive(Debug, Clone, Copy)]
struct Chrome<'a> {
    /// The whole row, `:` and all.
    text: &'a str,
    /// Whether the cursor belongs at the end of it.
    caret: bool,
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
/// file each frame. `T033` gave it the other direction: [`Editing::write`] is
/// the one thing that clears it, so `[+]` means *"different from what is on
/// disk"* rather than *"touched at some point"*.
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
// Input — the machine, and what applies what it says
// ---------------------------------------------------------------------------

/// One register's contents.
///
/// `linewise` is not decoration: `p` after `dd` opens a new line and `p` after
/// `dw` does not, and the only place that difference can be recorded is where
/// the yank happened.
#[derive(Debug, Clone)]
struct Register {
    text: String,
    linewise: bool,
}

/// The buffer, the registers, and **the only thing in this program that
/// mutates either**.
///
/// Every field here is state the Action stream moves. Nothing reads a key: the
/// machine turned keys into Actions two calls ago, and this cannot tell a key
/// from an MCP call — which is invariant 2 holding at the point where it costs
/// something.
struct Editing {
    editor: Editor,
    /// The text area, for scrolls and reveals.
    area: Rect,
    /// Where `:write` puts it, and what `:write <path>` replaces.
    file: Option<PathBuf>,
    /// A file `open-file` asked for. The loop performs it: opening one needs
    /// the theme and the language table, and neither is this struct's.
    open: Option<PathBuf>,
    /// A prompt `open-prompt` asked for, drained the same way.
    prompt: Option<PromptKind>,
    /// The unnamed register is `"`; `"a` is `a` (`request::RegisterName`).
    registers: BTreeMap<String, Register>,
    /// What the last `SelectRange` said, so a yank knows whether it is linewise.
    selection_kind: SelectionKind,
    dirty: Rc<Cell<bool>>,
    /// Set by `App::Quit`; the loop reads it once per turn.
    quit: bool,
}

/// The unnamed register, as vim spells it.
const UNNAMED: &str = "\"";

impl std::fmt::Debug for Editing {
    /// The editor holds a rope, a tree-sitter tree and a highlight cache, and
    /// implements no `Debug`; what is worth printing is the state this file
    /// owns.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editing")
            .field("area", &self.area)
            .field("registers", &self.registers)
            .field("selection_kind", &self.selection_kind)
            .field("quit", &self.quit)
            .finish_non_exhaustive()
    }
}

impl Editing {
    fn new(editor: Editor, file: Option<PathBuf>, dirty: Rc<Cell<bool>>) -> Self {
        Self {
            editor,
            area: Rect::ZERO,
            file,
            open: None,
            prompt: None,
            registers: BTreeMap::new(),
            selection_kind: SelectionKind::Char,
            dirty,
            quit: false,
        }
    }

    /// The buffer as the machine reads it.
    fn text(&self) -> EditorText<'_> {
        EditorText {
            editor: &self.editor,
            height: self.area.height,
        }
    }

    /// One Action, applied, and the cursor revealed if it moved.
    ///
    /// **The reveal is an Action too** ([`Editing::reveal`]), which is what
    /// keeps *"`View::Scroll` is the only thing that moves a viewport"* true
    /// with the cursor still following.
    fn apply(&mut self, action: &Action) -> Outcome {
        let outcome = self.act(action);
        if moves_cursor(action) {
            self.reveal();
        }
        outcome
    }

    /// Bring the cursor into view, moving as little as possible.
    ///
    /// Measured in **visual** rows, which is why it happens here and not in the
    /// machine: a soft-wrapped line is several rows and only the widget layer
    /// knows how many (`T081`).
    fn reveal(&mut self) {
        let Some(row) = self.editor.visual_row_for_cursor() else {
            return;
        };
        let row = u32::try_from(row).unwrap_or(0) + 1;
        let _ = self.act(&Action::View(ViewAction::Scroll {
            request: phosphor_core::request::ScrollRequest::RevealRow { row, margin: 0 },
            pane: phosphor_core::request::PaneRef::Focused {},
        }));
    }

    /// One Action. The `_` arm answers with the task that builds it, derived
    /// from the capability's own row rather than from a list here.
    fn act(&mut self, action: &Action) -> Outcome {
        let name = action.spec().name;
        let done = || {
            Outcome::Done(Receipt {
                capability: name,
                value: Value::Null,
                note: None,
            })
        };
        match action {
            Action::Buffer(BufferAction::Insert { at, text }) => {
                self.insert(*at, text);
                done()
            }
            Action::Buffer(BufferAction::Delete { span }) => {
                self.remove(*span);
                done()
            }
            Action::Buffer(BufferAction::Replace { span, text }) => {
                self.remove(*span);
                self.insert(span.start, text);
                done()
            }
            Action::Buffer(BufferAction::Yank { target, register }) => {
                self.yank(target, register.as_ref());
                done()
            }
            Action::Buffer(BufferAction::Paste {
                register, before, ..
            }) => {
                self.paste(register.as_ref(), *before);
                done()
            }
            Action::Buffer(BufferAction::SetRegister { register, text }) => {
                self.registers.insert(
                    register.0.clone(),
                    Register {
                        text: text.clone(),
                        linewise: text.ends_with('\n'),
                    },
                );
                done()
            }
            Action::Buffer(BufferAction::Indent { target, delta }) => {
                self.indent(target, *delta);
                done()
            }
            Action::Buffer(BufferAction::JoinLines { target }) => {
                self.join(target);
                done()
            }
            Action::Motion(MotionAction::MoveCursor { motion, count }) => {
                let to = motion::cursor_after(&self.text(), self.text().cursor(), *motion, *count);
                let offset = self.offset(to);
                self.editor.set_cursor(offset);
                done()
            }
            Action::Motion(MotionAction::SetCursor { position, .. }) => {
                let offset = self.offset(*position);
                self.editor.set_cursor(offset);
                done()
            }
            Action::Motion(MotionAction::SelectRange { span, kind }) => {
                let (from, to) = self.range(*span);
                self.editor.set_selection(Some(Selection::new(from, to)));
                self.selection_kind = *kind;
                done()
            }
            Action::Motion(MotionAction::ExtendSelection { motion, count }) => {
                let to = motion::cursor_after(&self.text(), self.text().cursor(), *motion, *count);
                let offset = self.offset(to);
                self.editor.set_cursor(offset);
                // Reads the anchor off the live selection, which the
                // `SelectRange` that entered visual mode has always set.
                self.editor.extend_selection(offset);
                done()
            }
            Action::Motion(MotionAction::ClearSelection {}) => {
                self.editor.clear_selection();
                done()
            }
            // The record of what was *asked for*. What it covers arrives as the
            // `SelectRange` behind it, when this side can resolve it at all —
            // the four agent nouns cannot until `T049`.
            Action::Motion(MotionAction::SelectObject { .. }) => done(),
            Action::View(ViewAction::Scroll { request, .. }) => {
                buffer_view::apply_scroll(&mut self.editor, scroll_request(*request), self.area);
                done()
            }
            // `T029` owns the undo model and takes both of these with it; until
            // then the fork's own history is the honest answer, and it is
            // already keyed on the edit batches this file commits.
            Action::History(HistoryAction::Undo { count }) => {
                for _ in 0..(*count).max(1) {
                    self.editor.apply(Undo);
                }
                done()
            }
            Action::History(HistoryAction::Redo { count }) => {
                for _ in 0..(*count).max(1) {
                    self.editor.apply(Redo);
                }
                done()
            }
            // The group boundary the machine marks. `T029` is what makes it do
            // something; the fork commits a batch per edit already, so honouring
            // it here would *narrow* undo rather than widen it.
            Action::History(HistoryAction::CommitUndoGroup {}) => done(),
            // `T033`'s four file capabilities. The seed table's own note said
            // *"there is no save path until `T033`"*; this is it.
            Action::File(FileAction::SaveBuffer { path, .. }) => {
                match self.write(path.as_deref()) {
                    Ok(()) => done(),
                    Err(reason) => declined(&reason),
                }
            }
            // One buffer, so this is `save-buffer` under its other name. It is
            // still the honest implementation of *"writes every dirty buffer"*
            // — there is exactly one, and `T088` is what makes there be more.
            Action::File(FileAction::SaveAll {}) => match self.write(None) {
                Ok(()) => done(),
                Err(reason) => declined(&reason),
            },
            Action::File(FileAction::OpenFile { path, .. }) => {
                self.open = Some(path.clone());
                done()
            }
            // Not `NotYetImplemented`: the capability is built and the limit is
            // real. One pane holds one buffer until `T088`, so closing it is
            // leaving, and `:quit` is how you say that.
            Action::File(FileAction::CloseBuffer { force, .. }) => {
                if !*force && self.dirty.get() {
                    return Outcome::Refused(Refusal::WouldLoseWork);
                }
                declined(
                    "one buffer, one pane — :quit leaves; T088 gives a buffer somewhere to close to",
                )
            }
            // The ex line. `T058` builds the message and search prompts and the
            // anchor chip that rides with them; the ex half is `T033`'s, because
            // an editor you cannot type `:write` into is not one CP-3 can judge.
            Action::Prompt(PromptAction::OpenPrompt { kind, .. }) => match kind {
                PromptKind::Ex => {
                    self.prompt = Some(PromptKind::Ex);
                    done()
                }
                PromptKind::Claude | PromptKind::Search => declined(
                    "only the ex line exists yet — T058 builds the message and search prompts",
                ),
            },
            Action::App(AppAction::Quit { force }) => {
                if !*force && self.dirty.get() {
                    return Outcome::Refused(Refusal::WouldLoseWork);
                }
                self.quit = true;
                done()
            }
            action => Outcome::Refused(Refusal::NotYetImplemented {
                task: action.spec().since.task,
            }),
        }
    }

    /// Writes the buffer out, to `path` or to where it came from.
    ///
    /// The whole rope, not a diff: `T029` owns the undo model and `T030` the
    /// on-disk log, and neither exists to write incrementally against yet.
    fn write(&mut self, path: Option<&Path>) -> Result<(), String> {
        let target = path
            .map(Path::to_path_buf)
            .or_else(|| self.file.clone())
            .ok_or_else(|| "no file name — :write <path>".to_owned())?;
        let code = self.editor.code_ref();
        let text = code.slice(0, code.len_chars());
        std::fs::write(&target, text).map_err(|error| format!("{}: {error}", target.display()))?;
        self.dirty.set(false);
        self.file = Some(target);
        Ok(())
    }

    // -- the buffer ---------------------------------------------------------

    /// A position as a character offset, clamped into the buffer.
    fn offset(&self, position: Position) -> usize {
        let code = self.editor.code_ref();
        let line = (position.line as usize).saturating_sub(1);
        let line = line.min(code.len_lines().saturating_sub(1));
        let column = (position.column as usize)
            .saturating_sub(1)
            .min(code.line_len(line));
        code.offset(line, column).min(code.len_chars())
    }

    fn range(&self, span: Span) -> (usize, usize) {
        let from = self.offset(span.start);
        (from, self.offset(span.end).max(from))
    }

    /// Opens an edit batch, recording where the cursor was.
    fn begin(&mut self) {
        let cursor = self.editor.get_cursor();
        let selection = self.editor.get_selection();
        let code = self.editor.code_mut();
        code.tx();
        code.set_state_before(cursor, selection);
    }

    /// Closes it, recording where the cursor ended up — which is what undo
    /// restores.
    fn commit(&mut self) {
        let cursor = self.editor.get_cursor();
        let selection = self.editor.get_selection();
        let code = self.editor.code_mut();
        code.set_state_after(cursor, selection);
        code.commit();
        self.editor.reset_highlight_cache();
    }

    fn insert(&mut self, at: Position, text: &str) {
        let offset = self.offset(at);
        self.begin();
        self.editor.code_mut().insert(offset, text);
        self.editor.set_cursor(offset + text.chars().count());
        self.commit();
    }

    fn remove(&mut self, span: Span) {
        let (from, to) = self.range(span);
        if from == to {
            return;
        }
        self.begin();
        self.editor.code_mut().remove(from, to);
        self.editor.set_cursor(from);
        self.commit();
    }

    /// What a target covers, in character offsets.
    ///
    /// Only the two focus-relative arms the input machine emits are answered;
    /// everything else is the store's to resolve (`T041`) and refuses rather
    /// than guessing.
    fn target_range(&mut self, target: &Target) -> Option<(usize, usize)> {
        match target {
            Target::Selection {} => self
                .editor
                .get_selection()
                .map(|selection| selection.sorted()),
            Target::Cursor {} => {
                let cursor = self.editor.get_cursor();
                Some((cursor, cursor + 1))
            }
            _ => None,
        }
    }

    fn yank(&mut self, target: &Target, register: Option<&RegisterName>) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let text = self
            .editor
            .code_ref()
            .slice(from, to.min(self.editor.code_ref().len_chars()));
        let linewise = self.selection_kind == SelectionKind::Line;
        let name = register.map_or_else(|| UNNAMED.to_owned(), |name| name.0.clone());
        self.registers.insert(name, Register { text, linewise });
    }

    fn paste(&mut self, register: Option<&RegisterName>, before: bool) {
        let name = register.map_or_else(|| UNNAMED.to_owned(), |name| name.0.clone());
        let Some(register) = self.registers.get(&name).cloned() else {
            return;
        };
        let cursor = self.text().cursor();
        if register.linewise {
            let trimmed = register.text.trim_end_matches('\n').to_owned();
            if before {
                self.insert(
                    Position {
                        line: cursor.line,
                        column: 1,
                    },
                    &format!("{trimmed}\n"),
                );
                let start = self.offset(Position {
                    line: cursor.line,
                    column: 1,
                });
                self.editor.set_cursor(start);
            } else {
                let at = motion::end_of_line(&self.text(), cursor.line);
                self.insert(at, &format!("\n{trimmed}"));
                let start = self.offset(Position {
                    line: cursor.line + 1,
                    column: 1,
                });
                self.editor.set_cursor(start);
            }
            return;
        }
        let at = if before {
            cursor
        } else {
            Position {
                column: cursor.column + 1,
                ..cursor
            }
        };
        self.insert(at, &register.text);
    }

    /// Shifts whole lines by one indent level, as `>` and `<` mean it.
    fn indent(&mut self, target: &Target, delta: i64) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let unit = self.editor.code_ref().indent();
        let first = self.editor.code_ref().char_to_line(from);
        let last = self
            .editor
            .code_ref()
            .char_to_line(to.saturating_sub(1).max(from));
        for line in first..=last {
            let start = self.editor.code_ref().line_to_char(line);
            if delta > 0 {
                self.begin();
                self.editor.code_mut().insert(start, &unit);
                self.commit();
            } else {
                let width = self
                    .editor
                    .code_ref()
                    .line(line)
                    .chars()
                    .take(unit.chars().count())
                    .take_while(|character| character.is_whitespace() && *character != '\n')
                    .count();
                if width > 0 {
                    self.begin();
                    self.editor.code_mut().remove(start, start + width);
                    self.commit();
                }
            }
        }
    }

    /// `J` — the newline and the next line's indent become one space.
    fn join(&mut self, target: &Target) {
        let (from, to) = self
            .target_range(target)
            .unwrap_or_else(|| (self.editor.get_cursor(), self.editor.get_cursor()));
        let code = self.editor.code_ref();
        let first = code.char_to_line(from);
        let last = code
            .char_to_line(to.saturating_sub(1).max(from))
            .max(first + 1);
        for _ in first..last {
            let text = self.text();
            let Some(next) = text.line(u32::try_from(first).unwrap_or(0) + 2) else {
                return;
            };
            let head = motion::end_of_line(&text, u32::try_from(first).unwrap_or(0) + 1);
            let blanks = next.chars().take_while(|c| c.is_whitespace()).count();
            let span = Span {
                start: head,
                end: Position {
                    line: head.line + 1,
                    column: u32::try_from(blanks).unwrap_or(0) + 1,
                },
            };
            self.remove(span);
            self.insert(head, " ");
        }
    }
}

/// The buffer as `phosphor_core::input::text::Text` asks for it.
///
/// Read-only by construction: the machine holds one of these and cannot reach
/// the editor through it.
struct EditorText<'a> {
    editor: &'a Editor,
    height: u16,
}

impl std::fmt::Debug for EditorText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorText")
            .field("lines", &self.lines())
            .field("cursor", &self.cursor())
            .finish_non_exhaustive()
    }
}

impl Text for EditorText<'_> {
    fn lines(&self) -> u32 {
        u32::try_from(self.editor.code_ref().len_lines())
            .unwrap_or(1)
            .max(1)
    }

    fn line(&self, line: u32) -> Option<String> {
        let code = self.editor.code_ref();
        let index = (line as usize).checked_sub(1)?;
        if index >= code.len_lines() {
            return None;
        }
        Some(
            code.line(index)
                .to_string()
                .trim_end_matches(['\n', '\r'])
                .to_owned(),
        )
    }

    fn cursor(&self) -> Position {
        let (row, column) = self.editor.code_ref().point(self.editor.get_cursor());
        Position {
            line: u32::try_from(row).unwrap_or(0) + 1,
            column: u32::try_from(column).unwrap_or(0) + 1,
        }
    }

    fn viewport(&self) -> Viewport {
        // The top *visual* row back to the buffer line it belongs to: `H` moves
        // the cursor, and the cursor lives in the buffer even when the row it
        // is drawn on is a wrap continuation (`T081`).
        let top = self
            .editor
            .row_span(self.editor.get_offset_y())
            .map_or(1, |span| u32::try_from(span.line_idx).unwrap_or(0) + 1);
        Viewport {
            top,
            height: u32::from(self.height).max(1),
        }
    }
}

/// One turn of the input loop: a key in, the buffer moved.
///
/// Held as four borrows rather than one owned struct because the machine reads
/// the buffer while the layer answers about the key, and those are two
/// different halves of the same turn.
struct Session<'a> {
    machine: &'a mut Machine,
    layer: &'a mut Layer,
    seed: &'a mut Table,
    editing: &'a mut Editing,
}

/// How many keys a `repeat-last` or `feed-keys` may put back into the loop
/// before it is refused.
///
/// One level, deliberately: `.` is `repeat-last` and a keymap that bound `.` to
/// something ending in `.` would otherwise be a hang. A `feed-keys` inside a
/// replay is the same shape.
const REENTRY: u8 = 1;

impl Session<'_> {
    /// One keystroke, and everything it asked for.
    ///
    /// **Each key's Actions are applied before the next key is fed** — which is
    /// what makes `.` correct: a replay resolves its spans against the buffer as
    /// it is when the replayed key arrives, not as it was when `.` was pressed.
    fn key(&mut self, pressed: Key) {
        let mut queue = std::collections::VecDeque::from([(pressed, 0_u8)]);
        while let Some((pressed, depth)) = queue.pop_front() {
            let emitted = {
                let text = self.editing.text();
                let mut layer = LayerKeymap { layer: self.layer };
                let mut keymap = Layered::new(&mut layer, self.seed);
                self.machine.feed(pressed, &mut keymap, &text)
            };
            for action in emitted {
                let Action::Input(input) = &action else {
                    let _ = self.editing.apply(&action);
                    continue;
                };
                self.machine.apply(input);
                match input {
                    // Nothing to keep in step: `T033` made the layer stateless,
                    // so the machine's pending sequence is the only one there is.
                    InputAction::CancelPending {} => {}
                    InputAction::FeedKeys { keys } => {
                        Self::enqueue(&mut queue, &keys.0, depth);
                    }
                    InputAction::RepeatLast { count } => {
                        if let Some(keys) = self.machine.last_change() {
                            for _ in 0..(*count).max(1) {
                                Self::enqueue(&mut queue, &keys.0, depth);
                            }
                        }
                    }
                    InputAction::SetMode { .. }
                    | InputAction::SetCount { .. }
                    | InputAction::SelectRegister { .. } => {}
                }
            }
        }
    }

    /// Puts a key sequence back into this turn, one level deep at most.
    fn enqueue(queue: &mut std::collections::VecDeque<(Key, u8)>, keys: &str, depth: u8) {
        if depth >= REENTRY {
            return;
        }
        for key in key::parse_seq(keys).unwrap_or_default() {
            queue.push_back((key, depth + 1));
        }
    }
}

/// Whether an Action moved the cursor, and so wants revealing.
const fn moves_cursor(action: &Action) -> bool {
    matches!(
        action,
        Action::Motion(
            MotionAction::MoveCursor { .. }
                | MotionAction::SetCursor { .. }
                | MotionAction::ExtendSelection { .. }
        ) | Action::Buffer(
            BufferAction::Insert { .. }
                | BufferAction::Delete { .. }
                | BufferAction::Replace { .. }
                | BufferAction::Paste { .. }
        ) | Action::History(HistoryAction::Undo { .. } | HistoryAction::Redo { .. })
    )
}

/// The vocabulary's scroll request as the widget layer's.
///
/// **The boundary conversion `request.rs` asks for by name.** Two definitions of
/// one type exist — `phosphor_core::request::ScrollRequest` and
/// `buffer_view::ScrollRequest` — because collapsing them deletes a
/// `surface`-owned type, which is a request rather than an edit `spine` makes.
/// Rows are 1-based in the vocabulary and 0-based in the widget, and this is the
/// only place that is true.
const fn scroll_request(
    request: phosphor_core::request::ScrollRequest,
) -> buffer_view::ScrollRequest {
    use phosphor_core::request::ScrollRequest as Wire;
    match request {
        Wire::Rows { rows } => buffer_view::ScrollRequest::Rows(rows),
        Wire::Pages { pages } => buffer_view::ScrollRequest::Pages(pages),
        Wire::Columns { columns } => buffer_view::ScrollRequest::Columns(columns),
        Wire::ToRow { row } => buffer_view::ScrollRequest::ToRow(row.saturating_sub(1) as usize),
        Wire::ToTop {} => buffer_view::ScrollRequest::ToTop,
        Wire::ToBottom {} => buffer_view::ScrollRequest::ToBottom,
        Wire::RevealRow { row, margin } => buffer_view::ScrollRequest::RevealRow {
            row: row.saturating_sub(1) as usize,
            margin: margin as usize,
        },
    }
}

/// Whether this event is a press rather than a release.
///
/// Under the kitty protocol every press is also reported as a release (`T014`
/// negotiates `REPORT_EVENT_TYPES`), so a loop that acted on both would apply
/// every keystroke twice. `T027` is where the *kind* becomes information rather
/// than noise.
const fn is_press(key: KeyEvent) -> bool {
    !matches!(key.kind, KeyEventKind::Release)
}

/// Whether `esc` should close what is on the frame rather than reach the
/// machine.
///
/// Design Language §9: *esc closes top-down*, and there is only ever one level
/// ([`Surface`]). On the buffer, `esc` is the machine's — it is the key that
/// leaves insert mode, which is the whole of modality.
const fn closes_surface(key: KeyEvent, surface: Surface) -> bool {
    matches!(key.code, KeyCode::Esc) && !matches!(surface, Surface::Buffer)
}

/// A terminal key event as one [`Key`].
///
/// The only place in this program that knows what a `crossterm::KeyEvent` is,
/// and deliberately partial: a key with no spelling has no binding and no text,
/// so it answers [`None`] and the turn ends.
fn decode(event: KeyEvent) -> Option<Key> {
    let mut mods = Mods::NONE;
    for (held, bit) in [
        (KeyModifiers::CONTROL, Mods::CTRL),
        (KeyModifiers::ALT, Mods::ALT),
        (KeyModifiers::SHIFT, Mods::SHIFT),
        (KeyModifiers::SUPER, Mods::SUPER),
    ] {
        if event.modifiers.contains(held) {
            mods = mods.with(bit);
        }
    }
    let code = match event.code {
        KeyCode::Char(character) => Code::Char(character),
        KeyCode::Esc => Code::Named(Named::Esc),
        KeyCode::Enter => Code::Named(Named::Enter),
        KeyCode::Tab => Code::Named(Named::Tab),
        KeyCode::BackTab => {
            mods = mods.with(Mods::SHIFT);
            Code::Named(Named::Tab)
        }
        KeyCode::Backspace => Code::Named(Named::Backspace),
        KeyCode::Delete => Code::Named(Named::Delete),
        KeyCode::Insert => Code::Named(Named::Insert),
        KeyCode::Left => Code::Named(Named::Left),
        KeyCode::Right => Code::Named(Named::Right),
        KeyCode::Up => Code::Named(Named::Up),
        KeyCode::Down => Code::Named(Named::Down),
        KeyCode::Home => Code::Named(Named::Home),
        KeyCode::End => Code::Named(Named::End),
        KeyCode::PageUp => Code::Named(Named::PageUp),
        KeyCode::PageDown => Code::Named(Named::PageDown),
        KeyCode::F(number) => Code::Named(Named::Function(number)),
        _ => return None,
    };
    Some(Key::new(code, mods))
}

/// Clicks and the wheel, as Actions.
///
/// **This is the second half of what `T026` deleted.** `Editor::mouse` called
/// `scroll_up`/`scroll_down` on the editor directly, so a wheel event moved the
/// viewport without asking; a click moved the cursor the same way. Both are
/// requests now, and both go through the same match every other Action does.
///
/// `cursor_from_mouse` is a *read* — it converts a cell to an offset and moves
/// nothing — which is why it may be called from here.
fn mouse_actions(editor: &Editor, mouse: MouseEvent, area: Rect) -> Vec<Action> {
    let at = |editor: &Editor| {
        editor
            .cursor_from_mouse(mouse.column, mouse.row, &area)
            .map(|offset| {
                let (row, column) = editor.code_ref().point(offset);
                Position {
                    line: u32::try_from(row).unwrap_or(0) + 1,
                    column: u32::try_from(column).unwrap_or(0) + 1,
                }
            })
    };
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => at(editor).map_or_else(Vec::new, |position| {
            vec![
                Action::Motion(MotionAction::ClearSelection {}),
                Action::Motion(MotionAction::SetCursor {
                    position,
                    buffer: None,
                }),
            ]
        }),
        MouseEventKind::Drag(MouseButton::Left) => at(editor).map_or_else(Vec::new, |position| {
            let (row, column) = editor.code_ref().point(editor.selection_anchor());
            let anchor = Position {
                line: u32::try_from(row).unwrap_or(0) + 1,
                column: u32::try_from(column).unwrap_or(0) + 1,
            };
            let (start, end) = if (position.line, position.column) < (anchor.line, anchor.column) {
                (position, anchor)
            } else {
                (anchor, position)
            };
            vec![Action::Motion(MotionAction::SelectRange {
                span: Span { start, end },
                kind: SelectionKind::Char,
            })]
        }),
        // Three rows a notch: the conventional wheel step, and the one number
        // here a `set-option!` will want to own (`T033`).
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let rows = if matches!(mouse.kind, MouseEventKind::ScrollDown) {
                3
            } else {
                -3
            };
            vec![Action::View(ViewAction::Scroll {
                request: phosphor_core::request::ScrollRequest::Rows { rows },
                pane: phosphor_core::request::PaneRef::Focused {},
            })]
        }
        _ => Vec::new(),
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
    /// The ex line — `:write`, `:quit`, and every unique prefix of one
    /// (`T033`).
    Ex,
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
/// whose body is a text input `q` is a character you are typing. §9's `esc` is
/// the one that works; the REPL is a text surface and the machine's modes are
/// the buffer's, so this is not the input machine's path and is not meant to
/// become one until the REPL is a pane (`T088`).
fn repl_key(key: KeyEvent, repl: &mut Repl, layer: &mut Layer) -> ReplStep {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return ReplStep::Close,
        KeyCode::Char('c') if control => return ReplStep::ToBuffer,
        KeyCode::Enter => {
            // The one key here that runs arbitrary scheme, and the only one
            // that marks the frame stale — through `Layer`, like everything
            // else. Before `T026` this file invalidated on *every* REPL key
            // because it could not tell the difference.
            layer.submit(repl);
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

/// What a key did to the ex line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExStep {
    /// It is still being typed.
    Typing,
    /// Abandoned; nothing runs.
    Cancel,
    /// Run it.
    Submit,
}

/// One key, while the ex line has the frame.
///
/// Deliberately smaller than the REPL's line editor: this is a command line,
/// not a session, and `T058` is what gives it history, an anchor chip and the
/// two other prompt kinds.
fn ex_key(key: KeyEvent, line: &mut String) -> ExStep {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return ExStep::Cancel,
        KeyCode::Enter => return ExStep::Submit,
        // Backspacing off the end of an empty line leaves, which is vim's own
        // behaviour and the only way `:` is not a trap.
        KeyCode::Backspace => {
            if line.pop().is_none() {
                return ExStep::Cancel;
            }
        }
        KeyCode::Char('u') if control => line.clear(),
        KeyCode::Char('c') if control => return ExStep::Cancel,
        KeyCode::Char(character) if !control => line.push(character),
        _ => {}
    }
    ExStep::Typing
}

/// Runs an ex line and answers what to say about it.
///
/// **The command's Actions go through `Editing::apply`, exactly as a key's
/// do.** There is no second path from a command to the buffer — which is what
/// makes `:write` and `SPC f s` the same thing said twice rather than two
/// implementations of saving.
fn submit_ex(layer: &mut Layer, editing: &mut Editing, line: &str) -> Option<String> {
    match layer.ex(line) {
        Ex::Ran => None,
        Ex::Run(actions) => actions
            .iter()
            .find_map(|action| match editing.apply(action) {
                Outcome::Done(_) => None,
                Outcome::Refused(refusal) => Some(phosphor_steel::answer::why(&refusal)),
            }),
        Ex::Ambiguous => Some(format!("ambiguous — :{line} names more than one command")),
        Ex::Unknown => Some(format!("no such command — :{line}")),
    }
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
// Only the decisions, not the frame: everything perceptual belongs to `CP-3`'s
// Tier-2 and Tier-3 halves. What is covered here is what a screenshot cannot
// show — the routing, the one-VM-door rule, and one scripted keystroke sequence
// applied to a real vendored editor, because the grammar is proved headlessly in
// `phosphor-core` and what this file adds is the *applying*.

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
    use ratatui::layout::Rect;

    use super::door::Evaluate as _;
    use phosphor_core::input::key::parse_seq;
    use phosphor_core::input::table::{Resolution, Role, Scope};

    use super::{
        AppHost, Editing, ExStep, Intent, Key, Layer, Machine, Repl, ReplStep, Session, StatusVm,
        Surface, Table, Vm, boot, buffer, closes_surface, decode, ex_key, is_press, language_of,
        repl_key, split, submit_ex, vm,
    };

    fn event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// One key as the machine sees it, through the same decoder the loop uses.
    fn pressed(code: KeyCode) -> Key {
        decode(event(code)).expect("a key with a spelling")
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

    /// A layer over a runtime tree, and the host behind its barrier — the same
    /// two calls `vm()` makes, so a test cannot be looking at a differently
    /// wired editor than the program is.
    fn booted_at(root: &Path) -> (Layer, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(Some(root.to_path_buf())));
        let runtime = boot(Some(root), &host);
        (Layer::new(runtime), host)
    }

    /// The shipped layer, read-only: the host has no root, so a persist is
    /// refused rather than writing into the repository's own runtime tree.
    fn booted() -> (Layer, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(None));
        let runtime = boot(Some(&tree()), &host);
        (Layer::new(runtime), host)
    }

    /// Asks the live keymap about a sequence, the way the loop does.
    fn resolved(layer: &mut Layer, spelled: &str) -> Resolution {
        let keys = parse_seq(spelled).expect("a spelling these tests wrote");
        layer.resolve(Scope::Normal, &keys)
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
    fn leaving_is_an_action_and_esc_is_the_machines() {
        // `T090`'s floor — `q` and `esc` quit — is gone with the handler it
        // protected. `q` is a buffer key and `esc` is the mode key;
        // `runtime/keymaps.scm` binds `ZQ` and `<C-c>`, and this applies
        // `App::Quit` like any other Action.
        let mut editing = Editing::new(
            buffer(
                "text",
                "text",
                &super::builtin("phosphor-dark").expect("a shipped theme"),
            )
            .expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );
        assert!(!editing.quit);
        let outcome = editing.apply(&Action::App(phosphor_core::action::AppAction::Quit {
            force: true,
        }));
        assert!(matches!(outcome, Outcome::Done(_)));
        assert!(editing.quit, "the loop reads this once per turn");
    }

    #[test]
    fn a_quit_that_would_lose_work_is_refused_unless_forced() {
        let dirty = std::rc::Rc::new(std::cell::Cell::new(true));
        let mut editing = Editing::new(
            buffer(
                "text",
                "text",
                &super::builtin("phosphor-dark").expect("a shipped theme"),
            )
            .expect("a buffer"),
            None,
            std::rc::Rc::clone(&dirty),
        );
        let outcome = editing.apply(&Action::App(phosphor_core::action::AppAction::Quit {
            force: false,
        }));
        assert!(matches!(outcome, Outcome::Refused(Refusal::WouldLoseWork)));
        assert!(!editing.quit);
    }

    #[test]
    fn esc_closes_the_surface_before_the_machine_sees_it() {
        assert!(closes_surface(event(KeyCode::Esc), Surface::Repl));
        assert!(closes_surface(event(KeyCode::Esc), Surface::Boot));
        assert!(
            !closes_surface(event(KeyCode::Esc), Surface::Buffer),
            "on the buffer, esc is the key that leaves insert mode"
        );
    }

    #[test]
    fn a_release_is_not_a_second_keystroke() {
        // The kitty protocol reports press *and* release (`T014` asks for
        // REPORT_EVENT_TYPES), so without this filter every keystroke would be
        // applied twice.
        let release = KeyEvent::new_with_kind_and_state(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
            KeyEventState::NONE,
        );
        assert!(!is_press(release));
        assert!(is_press(event(KeyCode::Char('a'))));
    }

    #[test]
    fn a_key_is_spelled_the_way_a_keymap_is_written() {
        // `6b` binds `"]r"` and its footer names `C-c`. Both have to survive the
        // trip from crossterm to the notation `runtime/keymaps.scm` is written
        // in — one decoder, one speller, and `phosphor-core` owns the spelling.
        assert_eq!(pressed(KeyCode::Char(']')).notation(), "]");
        assert_eq!(
            decode(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
                .expect("a key")
                .notation(),
            "<C-c>"
        );
        assert_eq!(pressed(KeyCode::Esc).notation(), "<esc>");
        assert_eq!(pressed(KeyCode::Char(' ')).notation(), "<space>");
        // Function keys have a spelling now; `T090`'s decoder answered `None`.
        assert_eq!(pressed(KeyCode::F(5)).notation(), "<f5>");
    }

    #[test]
    fn a_scripted_sequence_edits_the_real_buffer_through_the_loops_own_path() {
        // The half `phosphor-core`'s tests cannot reach: the grammar is proved
        // headlessly there, and this proves the *applying* — over the vendored
        // editor, through `Session`, with the shipped editor layer answering
        // first on every key.
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let mut editing = Editing::new(
            buffer("text", "one\ntwo\nthree\nfour\nfive", &theme).expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );
        editing.area = Rect::new(0, 0, 80, 24);
        let mut machine = Machine::new();
        let mut seed = Table::new();
        let (mut layer, _host) = booted();

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('3'),
            KeyCode::Char('d'),
            KeyCode::Char('d'),
        ] {
            Session {
                machine: &mut machine,
                layer: &mut layer,
                seed: &mut seed,
                editing: &mut editing,
            }
            .key(pressed(code));
        }

        assert_eq!(editing.editor.get_content(), "one\nfive");
        assert_eq!(
            editing
                .registers
                .get("\"")
                .map(|register| register.text.as_str()),
            Some("two\nthree\nfour\n"),
            "a delete fills the unnamed register, as vim does"
        );
        // And the viewport never moved: five lines in a 24-row area.
        assert_eq!(editing.editor.get_offset_y(), 0);
    }

    #[test]
    fn insert_mode_types_into_the_real_buffer_and_esc_leaves_it() {
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let mut editing = Editing::new(
            buffer("text", "bc", &theme).expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );
        editing.area = Rect::new(0, 0, 80, 24);
        let mut machine = Machine::new();
        let mut seed = Table::new();
        let (mut layer, _host) = booted();

        for code in [KeyCode::Char('i'), KeyCode::Char('a'), KeyCode::Esc] {
            Session {
                machine: &mut machine,
                layer: &mut layer,
                seed: &mut seed,
                editing: &mut editing,
            }
            .key(pressed(code));
        }
        assert_eq!(editing.editor.get_content(), "abc");
        assert_eq!(machine.mode(), phosphor_core::request::EditMode::Normal);
    }

    #[test]
    fn arbitrary_scheme_marks_the_frame_stale_and_composing_does_not() {
        // **The `CP-2` regression, as a rule with a test rather than a habit.**
        // A key bound to `(status-order-set! 'right '())` moves state the
        // composer reads without moving the ViewModel, so the revision cannot
        // see it — the frame has to be invalidated. `Layer` is the only way into
        // the VM and every method that can run user scheme sets the flag.
        let (mut layer, _host) = booted();
        assert!(!layer.stale(), "a fresh layer has run nothing");

        // A binding that is *data* runs no scheme at all: the resolver reads
        // the table and answers. Marking here would compose once per keystroke
        // and the cache would mean nothing.
        assert!(matches!(resolved(&mut layer, "w"), Resolution::Role(_)));
        assert!(!layer.stale(), "asking the keymap is not running a binding");
        assert_eq!(resolved(&mut layer, "\u{2603}"), Resolution::Unbound);
        assert!(!layer.stale(), "nor is asking about a key nobody bound");

        // A thunk is the other kind, and it is the `CP-2` case: it could have
        // set anything the composer reads.
        let _ = layer.evaluate(r#"(keymap-set! "gz" (lambda () 1) "count to one")"#);
        assert!(layer.stale(), "evaluating a form is running scheme");
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);
        assert!(layer.stale(), "a bound key ran a thunk");
        assert!(!layer.stale(), "reading it clears it");

        // Composing is the one exception: invalidating on the call that fills
        // the cache would refill it every frame, which is `T079`'s whole point.
        let _ = layer.compose(&StatusVm::default());
        assert!(
            !layer.stale(),
            "composition must not invalidate the cache it fills"
        );

        // And an evaluation does, whatever it evaluates.
        let _ = layer.evaluate("(+ 1 2)");
        assert!(layer.stale());
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
            let (mut layer, _host) = booted_at(&root);
            let mut session = Repl::new();
            for character in form.chars() {
                session.insert(character);
            }
            let entry = layer.submit(&mut session).expect("a form was typed");
            assert_eq!(
                entry.answered.note.as_deref(),
                Some("persisted to persisted.scm"),
                "the layer names the file that loads last"
            );
        }

        // A second editor over the same tree — a restart, in one process.
        let (mut layer, host) = booted_at(&root);
        assert!(
            layer.report().is_clean(),
            "a persisted form must not fault the next boot: {:?}",
            layer.report().faults
        );
        assert_eq!(resolved(&mut layer, "g"), Resolution::Pending);
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);
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
        let (mut layer, _host) = booted();
        let source = "(+ 1 2)";
        let door = Vm(&mut layer).eval(source);

        let mut session = Repl::new();
        for character in source.chars() {
            session.insert(character);
        }
        let entry = layer.submit(&mut session).expect("a form was typed");
        assert_eq!(answer::answered(&door), entry.answered);
    }

    #[test]
    fn a_rebind_typed_at_the_repl_is_in_force_on_the_next_key() {
        // `T022`'s claim, through the host's own path: type the rebind, then
        // press the key. No reload, no second boot, nothing invalidated.
        let (mut layer, host) = booted();
        let mut session = Repl::new();

        assert_eq!(resolved(&mut layer, "g"), Resolution::Pending);
        for character in r#"(keymap-set! "g" (lambda () (open-repl!)))"#.chars() {
            session.insert(character);
        }
        layer.submit(&mut session).expect("a form was typed");

        assert_eq!(resolved(&mut layer, "g"), Resolution::Ran);
        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl],
            "the binding ran, and what it asked for reached the loop"
        );
    }

    #[test]
    fn the_shipped_layer_is_the_whole_keymap_and_the_seed_is_empty() {
        // `T033`'s acceptance, at the seam this file owns: every binding the
        // loop resolves comes from the editor layer, and `Layered`'s fallback
        // has nothing in it. `no_bindings_in_rust.rs` holds the other half.
        let (mut layer, _host) = booted();
        assert!(Table::new().is_empty(), "the seed T033 emptied");
        assert!(
            matches!(resolved(&mut layer, "d"), Resolution::Role(_)),
            "the grammar is the editor layer's now"
        );
        assert!(matches!(
            resolved(&mut layer, "<C-c>"),
            Resolution::Role(Role::Run(_))
        ));
        assert_eq!(
            resolved(&mut layer, "q"),
            Resolution::Unbound,
            "`q` is nobody's — the machine turns it into the unknown-key hint"
        );
    }

    #[test]
    fn the_prompt_key_opens_the_ex_line_and_nothing_else_does() {
        // `:` is bound to `open-prompt` in `runtime/keymaps.scm`, and this is
        // the half of that binding this file owns: the Action lands, the loop
        // reads the ask. The other two prompt kinds are `T058`'s and say so.
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let mut editing = Editing::new(
            buffer("text", "", &theme).expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );
        let open = |kind| {
            Action::Prompt(phosphor_core::action::PromptAction::OpenPrompt {
                kind,
                seed: None,
                anchor: None,
            })
        };
        assert!(matches!(
            editing.apply(&open(phosphor_core::request::PromptKind::Ex)),
            Outcome::Done(_)
        ));
        assert_eq!(
            editing.prompt,
            Some(phosphor_core::request::PromptKind::Ex),
            "the loop reads this and gives the ex line the frame"
        );
        let Outcome::Refused(Refusal::Declined { reason }) =
            editing.apply(&open(phosphor_core::request::PromptKind::Claude))
        else {
            panic!("a message to claude needs a session and a transcript");
        };
        assert!(reason.contains("T058"), "{reason}");
    }

    #[test]
    fn the_ex_line_types_and_runs_through_the_same_path_a_key_does() {
        // `T033`'s ex half: `:w` is `:write` by the abbreviation rule, and the
        // Actions it names are applied by `Editing`, not by a second path.
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let file = scratch("ex").join("written.txt");
        let mut editing = Editing::new(
            buffer("text", "one\ntwo", &theme).expect("a buffer"),
            Some(file.clone()),
            std::rc::Rc::new(std::cell::Cell::new(true)),
        );
        let (mut layer, _host) = booted();

        let mut line = String::new();
        assert_eq!(ex_key(event(KeyCode::Char('w')), &mut line), ExStep::Typing);
        assert_eq!(ex_key(event(KeyCode::Enter), &mut line), ExStep::Submit);
        assert_eq!(line, "w");
        assert_eq!(submit_ex(&mut layer, &mut editing, &line), None);
        assert_eq!(
            std::fs::read_to_string(&file).expect("the file was written"),
            "one\ntwo"
        );

        // A command nobody defined says so rather than doing nothing.
        assert!(submit_ex(&mut layer, &mut editing, "nosuchthing").is_some());
        // Backspacing off an empty line leaves, so `:` is never a trap.
        let mut empty = String::new();
        assert_eq!(
            ex_key(event(KeyCode::Backspace), &mut empty),
            ExStep::Cancel
        );
        let _ = std::fs::remove_dir_all(scratch("ex"));
    }

    #[test]
    fn the_other_two_doors_reach_the_one_table() {
        // Invariant 2, on the keymap: `set-keybinding` from the CLI or MCP
        // arrives as the `(keymap-set! …)` form the Steel door would have been
        // typed, and the loop evaluates it into the same table.
        use phosphor_core::request::{Binding, KeySeq};
        let host = AppHost::new(None);
        let outcome = ask(
            &host,
            Action::Runtime(RuntimeAction::SetKeybinding {
                keys: KeySeq("gx".to_owned()),
                binding: Binding::Capability {
                    name: "quit".to_owned(),
                    args: phosphor_core::value::Args::new().with("force", Value::Bool(true)),
                },
                mode: None,
            }),
        );
        assert!(matches!(outcome, Outcome::Done(_)));
        let intents = host.intents();
        let [Intent::Keymap(form)] = intents.as_slice() else {
            panic!("the door records the form for the loop to evaluate");
        };

        let (mut layer, _host) = booted();
        assert!(matches!(layer.evaluate(form), Outcome::Done(_)), "{form}");
        assert!(matches!(
            resolved(&mut layer, "gx"),
            Resolution::Role(Role::Run(_))
        ));
    }

    #[test]
    fn a_broken_layer_boots_and_composes_its_float() {
        // `T021`'s promise, at the seam this file is responsible for: the host
        // gets a float to put on the frame, and `T079`'s interpreter draws it.
        let broken = scratch("boot");
        std::fs::write(broken.join("init.scm"), "(define oops\n").expect("write");

        let host = Arc::new(AppHost::new(None));
        let layer = Layer::new(boot(Some(&broken), &host));
        let float = layer
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
        let (mut layer, host) = booted();
        let mut session = Repl::new();

        for character in "(+ 1 2)".chars() {
            assert_eq!(
                repl_key(event(KeyCode::Char(character)), &mut session, &mut layer),
                ReplStep::Handled
            );
        }
        assert_eq!(session.input(), "(+ 1 2)");
        assert_eq!(
            repl_key(event(KeyCode::Enter), &mut session, &mut layer),
            ReplStep::Handled
        );
        assert_eq!(session.entries().len(), 1, "↵ submits");
        assert_eq!(session.entries()[0].answered.head, "3");
        assert!(session.input().is_empty());

        // `q` types.
        repl_key(event(KeyCode::Char('q')), &mut session, &mut layer);
        assert_eq!(session.input(), "q");
        repl_key(event(KeyCode::Backspace), &mut session, &mut layer);
        assert!(session.input().is_empty());

        // `↑` walks the history, and does not write itself into the session.
        repl_key(event(KeyCode::Up), &mut session, &mut layer);
        assert_eq!(session.input(), "(+ 1 2)");
        assert_eq!(session.entries().len(), 1, "an arrow key is not an entry");

        // `C-c` asks for the buffer — the same thing `(repl-to-buffer!)` asks
        // for through the door, which is where the two meet.
        assert_eq!(
            repl_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut session,
                &mut layer,
            ),
            ReplStep::ToBuffer
        );
        let _ = layer.evaluate("(repl-to-buffer!)");
        assert_eq!(host.intents(), vec![Intent::ToBuffer]);

        assert_eq!(
            repl_key(event(KeyCode::Esc), &mut session, &mut layer),
            ReplStep::Close,
            "§9: esc closes top-down"
        );
    }

    #[test]
    fn the_one_runtime_is_built_the_same_way_on_both_paths() {
        // Not a tautology: `vm()` is the only constructor there is, and if a
        // second appeared the door and the loop could answer differently.
        let (layer, _host) = vm();
        assert!(
            layer.report().is_clean() || layer.report().root.is_some(),
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
