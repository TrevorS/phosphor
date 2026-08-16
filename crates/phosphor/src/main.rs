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
//! true, and `arbitrary_scheme_marks_the_frame_stale_and_composing_does_not`
//! tests both halves.
//!
//! Composition is the deliberate exception ([`Layer::compose`]): invalidating
//! on the call that fills the cache would refill it every frame, which is the
//! `T079` regression the cache exists to prevent.
//!
//! # `T023` — the CLI door, alongside the host
//!
//! [`door`] is the other half of this file's job and does not touch the loop at
//! all. `phosphor --eval '(…)'` and the 216 generated capability verbs return
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
//!    path — see [`dirty_flag`]. Undo is the one thing that overrides it, and
//!    it does so with node identity rather than a second flag
//!    ([`Editing::walk`]).
//!
//! # The wiring pass — what a keystroke reaches, and what it did not
//!
//! `S3` shipped four surfaces that were built, tested, green, and unreachable:
//! the `SPC` popup, the unknown-key hint, folds and undo all worked and no key
//! opened any of them. That was this file's fault — it went to one agent in one
//! phase, so nothing built afterwards could be wired — and the rule it produced
//! is that **a task that produces something a user reaches by pressing a key
//! includes the key**. Five items, and the proof for every one of them is
//! `tests/loop_pty.rs`, which presses the key on a real terminal:
//!
//! * **Undo is `T029`'s tree and `T030`'s journal** ([`Timeline`]). The fork's
//!   own history is not a fallback, it is gone: two live histories cannot both
//!   be the history, and the fork's truncates on divergence. The journal opens
//!   before the first frame, so *"quit, reopen, undo"* walks the previous
//!   session's history.
//! * **The leader popup is composed here** ([`under`], [`Overlay`]), out of the
//!   live keymap on the frame that draws it — so a `(keymap-set! …)` typed at
//!   the REPL is in the next popup with nothing else to wire.
//! * **An unbound key teaches once** — `App::ShowUnknownKeyHint` reaches
//!   [`Editing::act`], and the latch that spends the session's one hint is the
//!   loop's, because *"once per session"* is not a fact about a buffer.
//! * **Folds have arms** ([`Editing::set_fold`] and its two neighbours) over
//!   the fork's own `fold_ranges`, which are the language's `folds.scm`.
//! * **The machine is told which keyboard protocol was negotiated**, which is
//!   what makes `T027`'s legacy-chord fallback able to fire at all.
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
use std::time::{Duration, Instant};

use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, ValueEnum};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use phosphor_buffer::grammar;
use phosphor_buffer::lsp::{
    Insight, Insights, LanguageServers, Lookup, Post, Question, ServerSpec, ServerState,
};
use phosphor_buffer::undo::{Caret, CharRange, Edit as TreeEdit, NodeId, Step, UndoTree};
use phosphor_core::action::{
    Action, AppAction, BufferAction, FileAction, HistoryAction, InputAction, LspAction,
    MotionAction, Outcome, PromptAction, Receipt, Refusal, Request, RuntimeAction, ViewAction,
};
use phosphor_core::config;
use phosphor_core::input::key::{Code, Key, Mods, Named};
use phosphor_core::input::table::{Keymap, Layered, Resolution, Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::input::{Machine, key, text as motion};
use phosphor_core::journal::{self, Log, undo as wire_undo};
use phosphor_core::language::{self, Languages};
use phosphor_core::query::{Answer, Answers, Query, QueryError, Revision};
use phosphor_core::registry::McpPolicy;
use phosphor_core::request::{
    Binding, CharRange as SignatureRange, Completion as WireCompletion, EditMode, FoldState,
    KeySeq, LanguageId, Position, PromptKind, RegisterName, SelectionKind,
    Signature as WireSignature, Span, Target, TextObject,
};
use phosphor_core::value::Value;
use phosphor_core::view::{
    Child, Density, Emphasis, Float as ViewFloat, KeyHint, Mood, Node, SessionState, Tone, Tree,
};
use phosphor_steel::boot::{BootFault, BootReport, BootUnit};
use phosphor_steel::float::ExLine;
use phosphor_steel::host::Host;
use phosphor_steel::keymap::{self, Ex};
use phosphor_steel::repl::Repl;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::source;
use phosphor_steel::status::{self, ComposeError, StatusFile, StatusVm};
use phosphor_term::{Frame, KeyboardProtocol, Term};
use phosphor_ui::buffer_view::{self, BufferView, Editor, StateMark, editor_area};
use phosphor_ui::diagnostics::DiagnosticsVm;
use phosphor_ui::float::{
    self, Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody, FloatFooter,
    FloatHeader, FloatSlot, FooterHint, SignatureBody, SignatureVm, TextBody,
};
use phosphor_ui::frame::FrameCache;
use phosphor_ui::gutter;
use phosphor_ui::interpret::{Interpreter, NoResources, Resources};
use phosphor_ui::key_hints::KeyHints;
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::{BUILTIN_SLUGS, Theme, builtin};
use phosphor_ui::unknown_key::{self, UnknownKeyHint};
use phosphor_ui::virtual_text;
use ratatui::layout::Rect;
// The widget layer's re-export, not the fork's own path: after `T026` this file
// no longer talks to the vendored *handler* at all, only to the editor value
// `BufferView` draws. **The fork's `Undo`/`Redo` are gone with `R2`** — two live
// histories cannot both be the history, and the fork's truncates on divergence
// (`vendor/ratatui-code-editor/src/history.rs:19-22`), which is the behaviour
// `T029`'s tree exists not to have. One fork import is left: the selection type
// `SelectRange` sets.
use ratatui_code_editor::selection::Selection;

mod door;
mod events;
mod lsp;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// The S1 host's argument surface.
#[derive(Debug, Parser)]
#[command(
    name = "phosphor",
    version,
    about = "phosphor — an agent-native terminal editor",
    // Every sentence here is checked against the tree, because `--help` is a
    // claim a keystroke can disprove. It said "BufferView + StatusLine" while
    // no `StatusLine` widget was ever drawn, and promised `:q` "with the ex
    // commands" two windows after they landed.
    long_about = "Opens one file and draws it: the buffer, and whatever \
                  runtime/statusline.scm composed on the last row — every frame inside a \
                  synchronized-output block. With no file, the same editor over an empty \
                  buffer with no name; `:write <path>` gives it one.\n\nModes, counts, \
                  named registers, operators and \
                  text objects are the input machine's (T026); the keymap is asked of \
                  runtime/keymaps.scm on every keystroke and the seed table behind it is \
                  empty (T033). `:write`, `:quit`, `:help` and `:repl` are ex commands; \
                  `ZQ` or `ctrl-c` leaves. There is no agent session yet."
)]
struct Cli {
    /// File to open. With none, an empty buffer with no name — `:write <path>`
    /// gives it one (T107).
    #[arg(value_name = "FILE")]
    path: Option<PathBuf>,

    /// Open the Steel REPL (`6b`) on the frame — the primary extension
    /// workflow. `:repl` opens it from the editor, `esc` closes it, and a file
    /// is optional because the REPL is a surface of its own.
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

    /// Turn T081's soft wrap on. Off by default, as the mockups specify.
    /// `init.scm`'s `(set-option! "soft-wrap" …)` sets the same thing;
    /// `set-soft-wrap` is declared and not applied, so nothing toggles it at
    /// runtime until T096.
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

/// The option the editor layer sets to say how much of a word has to be typed
/// before the completion list raises itself — `T038`.
///
/// **`CP-4` found no minimum at all**: the insert-mode trigger asked on every
/// edit, so a space raised the server's whole unfiltered symbol table and the
/// first letter of every identifier raised the widest list that letter can
/// produce. Both are noise, and the widest list is also the worst case for the
/// float's own width.
///
/// Named in the editor layer rather than compiled in, because *how eager is
/// eager* is a preference and this build already has the mechanism for one:
/// `runtime/init.scm` sets it beside `soft-wrap`.
const COMPLETION_MIN_CHARS: &str = "completion-min-chars";

/// What [`COMPLETION_MIN_CHARS`] is worth to a layer that never sets it.
///
/// **Two, and the two other candidates are the argument for it.** Zero is what
/// `CP-4` reported and asks the server for its whole table on a keystroke that
/// typed no word at all. One still fires on the first letter of every
/// identifier — the case Teej called noisy — and a one-character prefix is the
/// least selective one `phosphor_buffer::lsp::narrow` can be handed, so it
/// raises the longest list the buffer's language has. Three is past the whole
/// name for `fs`, `io`, `os` and every other two-letter identifier, which would
/// make the list unreachable by typing for exactly the names that are hardest
/// to remember. Two is the shortest prefix that means *you have committed to a
/// word*.
///
/// **`runtime/init.scm` states the same number**, and that is deliberate
/// duplication of the kind `soft-wrap` already has: the layer is where a person
/// changes it, and this is what a layer that does not mention it gets. If the
/// two ever disagree the layer wins, because the layer runs last.
const COMPLETION_MIN_CHARS_DEFAULT: usize = 2;

/// How many word characters must sit behind the cursor before *typing* asks for
/// completions.
///
/// A negative minimum is a minimum of none rather than the default: `-1` means
/// *never suppress*, which is what a person writing a negative number into a
/// floor is asking for, and silently substituting `2` for it would be the
/// editor overruling a setting it read.
fn completion_floor(host: &AppHost) -> usize {
    host.number(COMPLETION_MIN_CHARS)
        .map_or(COMPLETION_MIN_CHARS_DEFAULT, |least| {
            usize::try_from(least).unwrap_or(0)
        })
}

/// How long typing must pause before the editor asks the server anything.
///
/// **Reported at `CP-4`: *"completion seemed to take longer than it should
/// have"*, and the cause was that there was no timer at all.** The only gate
/// was one-request-in-flight, so a burst of typing sent a request, waited a
/// whole server round trip, and sent the next — every list you saw was one
/// round trip behind the word you had already finished. A pause is what makes
/// the answer be about the prefix you stopped on.
///
/// **250ms is helix's number**, read from its source rather than picked:
/// `completion_timeout: Duration::from_millis(250)` in `helix-view/src/editor.rs`.
/// Taking a measured default from a shipping editor is worth more here than a
/// figure derived from nothing, and this is the one value in this file with an
/// upstream to be wrong against.
///
/// **`<C-x>` does not wait**, for the same reason it ignores
/// [`completion_floor`]: a person who pressed the key has asked, and a delay on
/// an explicit ask is the editor being slow rather than being quiet.
const COMPLETION_DEBOUNCE: Duration = Duration::from_millis(250);

// ---------------------------------------------------------------------------
// `T104` — what one indent level is
// ---------------------------------------------------------------------------

/// How many cells a `\t` advances to. `runtime/init.scm` sets it.
const TAB_WIDTH: &str = "tab-width";

/// Whether one indent level is spaces (`#t`) or a real tab. Vim's `expandtab`,
/// spelled the way this build spells option names.
const EXPAND_TAB: &str = "expand-tab";

/// What [`TAB_WIDTH`] is worth to a layer that never sets it.
///
/// Four, which is `CP-4`'s own words — *"i'd default to rendering at 4 with a
/// tab-width option"* — and is what `utils::indent` already gave the languages
/// it named. Eight is the terminal's historical stop and nothing in this build
/// is a terminal emulator; four is what an editor shows.
const TAB_WIDTH_DEFAULT: usize = 4;

/// One indent level, resolved (`T104`).
///
/// # Why this exists at all, and what it replaced
///
/// `Editing::indent` used to ask `Code::indent`, which is
/// `vendor/ratatui-code-editor`'s `utils::indent` — a `match` on the *grammar
/// name* giving four spaces to eleven languages, a literal tab to `go` and
/// `c_sharp`, and two spaces to everything else. Three things were wrong with
/// that and only the first is visible: nothing a user writes could change it,
/// the `\t` arm was unreachable because nothing in `runtime/languages/`
/// declares `go`, and a *grammar* is not a language — `steel` names the
/// `scheme` grammar, `csv` names none, and both landed on the same arm as a
/// file nobody declared. The unit is the editor layer's now, and the fork's
/// table has no phosphor caller.
///
/// # Two knobs, not vim's four
///
/// `tab-width` and `expand-tab`, plus the per-language `indent` literal.
/// **`shiftwidth` is deliberately absent**: vim needs one because a file can
/// mix tabs and spaces and *"how far `>>` shifts"* is then a different question
/// from *"how wide a tab draws"*. Here one unit answers `>`, `<` and `<tab>`
/// together, which is what every modern editor ships as a single *tab size*,
/// and splitting it is an addition rather than a correction if somebody wants
/// it. **`softtabstop` is absent too, and it is the one with a live gap**: it
/// is what makes `<bs>` eat a whole spaces-indent rather than one space, and
/// `<bs>` reaches [`phosphor_core::input::Machine`]'s `back_span`, which
/// deletes one grapheme. That is a `<bs>` behaviour and this is the `<tab>`
/// task; it is named here so it is absent rather than forgotten.
#[derive(Debug, Clone)]
struct IndentStyle {
    /// What one level is, literally — what `>` splices and `<` removes.
    unit: String,
    /// Cells a `\t` renders to, which the fork's renderer is told per pass.
    tab_width: usize,
}

impl IndentStyle {
    /// The text `<tab>` types at display column `col`.
    ///
    /// **A tab press advances to the next stop, it does not type a fixed
    /// width.** With a four-space unit, `<tab>` after `ab` types two spaces and
    /// lands on column 4 — the same column a real tab would have reached, which
    /// is the whole point of the option being one number: a file indented by
    /// pressing `<tab>` and a file indented with `\t` draw identically.
    ///
    /// A tab unit types a tab and lets the renderer do the arithmetic.
    fn typed_at(&self, col: usize) -> String {
        if self.unit.starts_with('\t') {
            return "\t".to_owned();
        }
        let width = self.width();
        " ".repeat(width - (col % width))
    }

    /// Cells one level occupies — the tabstop for a tab unit, the literal's own
    /// display width otherwise. Never zero: a level of no cells would make
    /// [`IndentStyle::typed_at`] divide by zero and `<` a no-op that looked
    /// like a bug.
    fn width(&self) -> usize {
        if self.unit.starts_with('\t') {
            self.tab_width.max(1)
        } else {
            self.unit.chars().count().max(1)
        }
    }
}

/// [`IndentStyle`] for `language`, read from the layer on every pass.
///
/// **Read per use, never snapshotted.** `T037` shipped a bug where a table was
/// read once at boot and `T101`'s review caught the same shape a second time,
/// so this is a function of the host rather than a field somebody initialises:
/// `(set-option! "tab-width" 8)` typed at the REPL changes the next frame.
///
/// **Precedence: the declaration beats the option.** That is vim's rule for
/// `ftplugin` over a global `set`, and it is the rule this build already
/// follows everywhere a scope and an all-scopes row disagree — the narrower
/// statement wins. A language that declares nothing takes the global answer,
/// which is what all four of `rust`, `python`, `toml` and `html` do.
fn indent_style(
    host: &AppHost,
    languages: &Languages,
    language: Option<&LanguageId>,
) -> IndentStyle {
    let tab_width = host
        .number(TAB_WIDTH)
        .and_then(|width| usize::try_from(width).ok())
        .filter(|width| *width > 0)
        .unwrap_or(TAB_WIDTH_DEFAULT);
    let declared = language.and_then(|language| languages.indent(language));
    let unit = match declared {
        Some(unit) => unit.to_owned(),
        // `expand-tab` unset is spaces, because a build whose renderer had to
        // guess is the state `CP-4` reported, and spaces are what eleven of the
        // twelve shipped languages want.
        None if host.flag(EXPAND_TAB) == Some(false) => "\t".to_owned(),
        None => " ".repeat(tab_width),
    };
    IndentStyle { unit, tab_width }
}

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
#[derive(Debug)]
struct AppHost {
    state: Mutex<HostState>,
    /// `T037`'s table. **The editor holds no list of languages** — this starts
    /// empty and `runtime/languages/*.scm` fills it through
    /// `define-language`, which is what makes the shipped twelve
    /// indistinguishable from a thirteenth typed at `:repl`.
    languages: Mutex<Languages>,
    /// `T040`'s set, shared with the loop. See `crate::lsp::Diagnostics` for
    /// why one store has two handles.
    diagnostics: Arc<lsp::Diagnostics>,
}

/// Everything the host owns that Steel can reach.
#[derive(Debug, Default)]
struct HostState {
    /// Surface asks, oldest first.
    intents: Vec<Intent>,
    /// `(set-option! …)`. `init.scm` sets `soft-wrap` here at boot.
    options: BTreeMap<String, Value>,
    /// The config home `persist-form!` appends to (`T101`), or [`None`] when
    /// neither `XDG_CONFIG_HOME` nor `HOME` names one.
    config: Option<PathBuf>,
    /// Which file in it, named by the editor layer. See [`PERSIST_FILE`].
    file: String,
    /// The head [`AppHost::persist`] writes without asking. See
    /// [`PERSIST_VERB`].
    verb: String,
    /// The heads it *offers* instead of writing. See [`OFFERED_HEADS`].
    offered: Vec<String>,
}

/// The boot file's name, and it names two different files.
///
/// In the **shipped** tree it is what `phosphor_steel::boot` loads first and
/// reads the load order out of. In the **config home** it is the user's own
/// layer, which [`Layer::load_user_layer`] runs on top of that tree (§34).
///
/// **`phosphor_steel::boot::INIT`, not a second spelling of it.** The sentence
/// above asserts the two names are the same name, and a `&str` written twice
/// across a crate barrier is exactly the drift [`PERSIST_VERB`]'s doc refuses:
/// *"two spellings of one name drift, and the one that drifts silently is the
/// Rust one."* This is an alias so the claim is held by the compiler; the doc
/// is here because that is where a reader of this file looks for it.
///
/// # The one place the Emacs argument cuts the other way
///
/// It is also what a layer that declares no [`PERSIST_FILE`] gets — `6b`'s own
/// note, and the right answer for a machine whose only layer is that one file:
/// the file you hand-wrote is then the file `persist!` appends to, and
/// [`Layer::booted_already`] is what keeps it from running twice.
///
/// **That is a machine writing into a human's file**, and
/// `phosphor_core::config`'s header argues against it a screen earlier —
/// *"Emacs makes the same split: `custom.el` sits beside `init.el`, not in a
/// cache."* Both are true of Emacs, which is the reason to record the tension
/// rather than resolve it: Emacs's own default writes customisations into
/// `init.el` until you set `custom-file`, and the split is what you get when
/// you name a second file. Here the shipped layer names it
/// (`runtime/repl.scm`), so a machine with no shipped layer has nobody to name
/// one — and inventing a `persisted.scm` for it in Rust would be this file
/// holding an opinion the layer is supposed to hold.
const INIT: &str = phosphor_steel::boot::INIT;

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
///
/// **`T101` moved the directory out from under it.** The name is joined to the
/// *config home* now (`phosphor_core::config`), never to the runtime root: in a
/// dev checkout that root is the repository, and `CP-4`'s manual test left a
/// `(define-language! "lua" …)` in the tracked `runtime/persisted.scm`.
const PERSIST_FILE: &str = "phosphor/persist-file";

/// The global naming the head [`AppHost::persist`] writes without asking —
/// `persist!` (`T101`).
///
/// The verb is spelled in `runtime/repl.scm` and read here, rather than being a
/// constant in both, for the reason [`PERSIST_FILE`] is: two spellings of one
/// name drift, and the one that drifts silently is the Rust one.
const PERSIST_VERB: &str = "phosphor/persist-verb";

/// The global naming the heads the REPL *offers* rather than keeps.
///
/// The layer routes these to `persist-form!` so the receipt can say what
/// happened to them; this arm is what makes that an offer. Anything not named
/// here is written as given — a direct caller of the capability is already
/// being explicit, which is `7a`'s always-allow (`[2] always allow git push` →
/// *"writes `(allow "git push")`"*), and gating it would break a permission
/// grant that has to survive a restart.
const OFFERED_HEADS: &str = "phosphor/offered-heads";

impl AppHost {
    fn new(config: Option<PathBuf>) -> Self {
        Self {
            state: Mutex::new(HostState {
                config,
                file: INIT.to_owned(),
                ..HostState::default()
            }),
            // The grammar names this build can load, from the crate whose
            // manifest selects them. `Languages::new` takes them rather than
            // guessing because the tier is the *intersection* of what a
            // declaration names with what the binary contains — see
            // `phosphor_buffer::grammar`.
            languages: Mutex::new(Languages::new(grammar::BUNDLED)),
            diagnostics: Arc::new(lsp::Diagnostics::default()),
        }
    }

    /// The declared languages, cloned out from behind the lock.
    ///
    /// Cloned rather than lent, and that is the barrier's shape rather than a
    /// convenience: a binding runs inside the running VM, so `Host::apply`
    /// takes `&self` and the table is behind a `Mutex`. The loop reads it while
    /// holding `&mut Editing`, and a guard held that long would be a lock held
    /// across a redraw — the same rule `lsp::Diagnostics::of` follows one
    /// module over. Twelve records; the clone is a `Vec` of small strings.
    fn languages(&self) -> Languages {
        self.languages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Points `persist-form!` at the file the layer named.
    ///
    /// A name only — a path leaving the config home is refused for the same
    /// reason one leaving the runtime tree is (`boot::is_confined`): the layer
    /// names a file, not a path into the filesystem.
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

    /// Tells `persist-form!` which head it keeps and which it offers (`T101`).
    ///
    /// Read once after the boot alongside [`AppHost::persist_to`], for the same
    /// reason: the host is behind the barrier and may not re-enter the VM when
    /// a form arrives.
    fn persist_policy(&self, verb: String, offered: Vec<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.verb = verb;
            state.offered = offered;
        }
    }

    /// The file the persisted layer is written to and read back from, or
    /// [`None`] when there is no config home.
    ///
    /// One resolution serving both directions is the point: a form written
    /// somewhere the loader does not look is a rebind that survives nothing,
    /// which is what `T101` was reported for.
    fn persist_target(&self) -> Option<PathBuf> {
        let state = self.state.lock().ok()?;
        Some(state.config.as_ref()?.join(&state.file))
    }

    /// The user's own hand-written layer — `$XDG_CONFIG_HOME/phosphor/init.scm`
    /// — or [`None`] when there is no config home (§34).
    ///
    /// **A fixed name, unlike [`AppHost::persist_target`]'s.** The persist file
    /// is whatever the *shipped* layer declares ([`PERSIST_FILE`]), because the
    /// thing that has to load last is a property of that layer's load order.
    /// This one is the file a person is told to create, so it cannot be named
    /// by a layer they may not have: on a machine with no shipped tree at all
    /// this is the only file there is, and a name that moved with the layer
    /// would make *"where do I put my config"* unanswerable.
    ///
    /// It resolves through the same `state.config` the persist target does, so
    /// the file you hand-write and the file `persist-form!` appends to are in
    /// one directory by construction rather than by two agreeing readings of
    /// `$XDG_CONFIG_HOME`.
    fn user_layer(&self) -> Option<PathBuf> {
        let state = self.state.lock().ok()?;
        Some(state.config.as_ref()?.join(INIT))
    }

    /// The config home this process resolved, or [`None`] when there is none.
    ///
    /// `phosphor_core::config`'s header claims [`config::config_dir`] is *"the
    /// one resolution"* and that this type's joins are the only readers of it.
    /// [`run`] called `config_dir()` a second time to hand
    /// [`Layer::note_if_no_layer`] the same directory this host already holds,
    /// which made the sentence narrowly false and put a second environment read
    /// on the startup path. No drift was possible — one function, one process —
    /// but a claim that survives only by coincidence is a claim a later edit
    /// gets to break silently.
    fn config_home(&self) -> Option<PathBuf> {
        self.state.lock().ok()?.config.clone()
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

    /// A numeric option, or `None` if `init.scm` never set it.
    ///
    /// The reader [`AppHost::flag`] is for booleans and this is for counts, and
    /// there is no third: `Value` has one integer case on purpose
    /// (`phosphor_core::value::Value::Int`), so *every* number an option can
    /// carry — a minimum, a delay, a column — comes back through here.
    ///
    /// **A key set to the wrong case reads as unset**, exactly as `flag`
    /// treats `(set-option! "soft-wrap" 3)`. There is nowhere to report a type
    /// error to: `set-option!` returned long before the loop reads the option,
    /// and refusing to draw a frame over it would be a worse answer than the
    /// documented default.
    fn number(&self, key: &str) -> Option<i64> {
        match self.state.lock().ok()?.options.get(key)? {
            Value::Int(number) => Some(*number),
            _ => None,
        }
    }

    /// Records a surface ask for the loop to carry out.
    fn ask(&self, intent: Intent) {
        if let Ok(mut state) = self.state.lock() {
            state.intents.push(intent);
        }
    }

    /// `6b`'s `· persisted to …` — one form appended to the config home.
    ///
    /// The note is the receipt's, not the REPL's: whoever appended the line is
    /// the only one who knows where it went, and it **names the file it wrote**
    /// rather than the one `6b` draws, because a layer of more than one file
    /// does not write to `init.scm` (see [`PERSIST_FILE`]). The bare name and
    /// not the path — `6b`'s line is one row of a narrow surface, and a
    /// receipt is not the place to put somebody's `$HOME` on a screenshot.
    ///
    /// # `T101` — two changes, and the second is the one with teeth
    ///
    /// **The gate.** A form whose head the layer listed in [`OFFERED_HEADS`] is
    /// refused with the verb that would keep it. That is what turns the REPL's
    /// auto-persist into an offer: evaluating is evaluating, and `M-:` does not
    /// write your `custom-file`. Anything else is written as given, because a
    /// caller that reached this capability directly has already been explicit
    /// — `7a`'s always-allow is that caller.
    ///
    /// **One `write_all`, not `writeln!`.** Two phosphors on one config home
    /// both appending is an ordinary case, and `write_fmt` is free to issue a
    /// syscall per format piece — so `writeln!(handle, "{form}")` could put the
    /// newline of one process between the halves of another's form, under
    /// `O_APPEND`, on a local filesystem. One buffer means one `write`, which
    /// is the granularity `O_APPEND` actually promises to keep whole.
    /// `a_form_is_appended_whole_when_several_writers_race` plants that.
    fn persist(&self, form: &str) -> Outcome {
        let Ok(state) = self.state.lock() else {
            return declined("the editor layer is busy");
        };
        let Some(config) = state.config.clone() else {
            return declined("no config home to write to — set $XDG_CONFIG_HOME or $HOME");
        };
        let file = state.file.clone();
        if let Some(head) = head(form)
            && head != state.verb
            && state.offered.iter().any(|offered| offered == head)
        {
            let verb = state.verb.clone();
            drop(state);
            // The remedy alone. `Repl::persist` prepends `not persisted — `,
            // and a reason that opened with its own `session only — ` made the
            // receipt `#ok · not persisted — session only — (persist! …) keeps
            // it` — two em dashes, the first joining a restatement of the head.
            // §6 spells one dash, for cause. Neither side could see it: each
            // supplies half.
            return declined(&format!("({verb} …) keeps it"));
        }
        drop(state);

        let path = config.join(&file);
        let line = format!("{form}\n");
        let written = std::fs::create_dir_all(&config).and_then(|()| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut handle| handle.write_all(line.as_bytes()))
        });
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

/// The head of a form: `(persist! …)` → `persist!`.
///
/// The same three lines `phosphor_steel::repl::head` runs, and deliberately a
/// second copy rather than a shared one: that module's is private, and the
/// alternative — exporting it so the gate and the router agree — would put a
/// `pub fn` on the barrier crate for one string slice. If a third caller
/// appears, lift it; two is not a table.
fn head(source: &str) -> Option<&str> {
    let rest = source.trim_start().strip_prefix('(')?;
    let end = rest
        .find(|character: char| character.is_whitespace() || character == '(' || character == ')')
        .unwrap_or(rest.len());
    Some(&rest[..end]).filter(|head| !head.is_empty())
}

impl Answers for AppHost {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        match query {
            // `T037`. Built by the table itself rather than assembled here, so
            // the host cannot disagree with `Languages::tier` about what it
            // holds (`language.rs`).
            Query::Ui(phosphor_core::query::UiQuery::Languages { language }) => Ok(Answer {
                value: Value::List(self.languages().answer(language.as_ref())),
                // `Revision::INITIAL` because the store has no revision until
                // `T041` and a number invented here would be one a cache could
                // trust wrongly.
                revision: Revision::INITIAL,
            }),
            // `T040`. Answered off the same store the gutter draws from.
            Query::Review(phosphor_core::query::ReviewQuery::Diagnostics { path }) => Ok(Answer {
                value: Value::List(self.diagnostics.answer(path.as_deref())),
                revision: Revision::INITIAL,
            }),
            // Everything else, by its own row. Derived, never listed.
            query => Err(QueryError::NotYetImplemented {
                task: query.spec().since.task,
            }),
        }
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
            // `T037` — the road up, and the whole of what "the bundled set" is.
            // The twelve shipped files reach this arm at boot and a thirteenth
            // typed at `:repl` reaches the same one, which is the criterion.
            //
            // The two refusals are the table's (`language::Invalid`): a
            // nameless language and an extension written with its dot are
            // declarations that *land* and then never match a file, which is
            // worse than a refusal because the road up is walked once.
            Action::Runtime(RuntimeAction::DefineLanguage { language, spec }) => self
                .languages
                .lock()
                .map_or_else(
                    |_| Err("the editor layer is busy".to_owned()),
                    |mut table| {
                        table
                            .declare(language.clone(), spec.clone())
                            .map_err(|invalid| invalid.to_string())
                    },
                )
                .map_or_else(|reason| declined(&reason), |_| done(Value::Null)),
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
fn bind_form(keys: &KeySeq, binding: &Binding, mode: Option<&EditMode>) -> String {
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

/// Whether two paths name the same file on disk.
///
/// Canonicalised, because the two sides reach here by different routes:
/// `Runtime::root`'s third candidate is the relative `runtime`, its override is
/// whatever `$PHOSPHOR_RUNTIME` holds, and the persist target is always
/// absolute. Textual equality would miss `./runtime/init.scm` against
/// `/repo/runtime/init.scm`. A path that cannot be canonicalised does not exist,
/// and two names that do not exist are the same file only if they are the same
/// name.
fn same_file(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
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
    /// The boot report, **plus the persisted layer's** (`T101`).
    ///
    /// `persisted.scm` is outside the runtime tree now, so `Runtime::report()`
    /// has never heard of it and a fault in it would otherwise be invisible.
    /// One report rather than two floats: a person opening the editor wants to
    /// know what did not load, not which of two mechanisms failed to load it.
    report: BootReport,
    /// Every file [`Layer::load_after_boot`] has run, as it was given.
    ///
    /// The boot's own files are in [`Layer::report`] relative to a root, so
    /// they answer [`Layer::booted_already`] on their own; these have no root
    /// to be relative to, and there are two of them now — the user's layer and
    /// the persisted one — which on a machine with no shipped tree are **the
    /// same path** ([`INIT`]). Without this list that file runs twice.
    after_boot: Vec<PathBuf>,
    /// How many files the **boot** loaded, before anything stacked on top.
    ///
    /// [`Layer::report`] grows as the layers run, so `units.is_empty()` stops
    /// answering *"did an editor layer load"* the moment a user's own
    /// `init.scm` reads — and that is §34's whole population, not an edge. This
    /// is the count taken before [`Layer::load_after_boot`] can touch it, which
    /// is what [`Layer::has_editor_layer`] reads.
    booted_units: usize,
}

impl Layer {
    fn new(runtime: Runtime) -> Self {
        let report = runtime.report().clone();
        Self {
            runtime,
            ran: false,
            booted_units: report.units.len(),
            report,
            after_boot: Vec::new(),
        }
    }

    /// Whether the shipped tree loaded anything at all — the fact `:` and every
    /// other key depend on (§34).
    ///
    /// **The boot's own units, not the report's.** Every binding in the editor
    /// is in `runtime/keymaps.scm` and the seed table is empty by construction
    /// (`crates/phosphor/tests/no_bindings_in_rust.rs`), so a process where
    /// this is false has no keymap, no ex line and no way to quit — however
    /// many files layered on top of the nothing it found. A config-home
    /// `init.scm` is a layer, never an editor.
    const fn has_editor_layer(&self) -> bool {
        self.booted_units > 0
    }

    /// Runs the user's own `init.scm` — **on top of the shipped tree, not
    /// instead of it** (§34).
    ///
    /// Teej ruled this on 2026-08-14 and the model is Emacs's, which is the one
    /// `T101` was decided on to begin with: shipped lisp loads, then your
    /// `init.el` runs over it. Until then the config home was
    /// [`Runtime::root`]'s second *candidate*, so an `init.scm` there became
    /// the runtime tree and the shipped fifteen files never loaded at all —
    /// measured on the built binary as an empty statusline, `:` drawing
    /// `unknown key :`, `ZQ` doing nothing, and no float, because the user's
    /// one form ran cleanly.
    ///
    /// **The ruling settles the direction question too.** A user's file may
    /// *remove* a shipped binding as well as add one, and the vocabulary
    /// already spells it: `keymap-remove!` is defined in `runtime/keymaps.scm`
    /// and listed in `runtime/repl.scm`'s persistable heads. Layering gives
    /// both directions with no new verb.
    ///
    /// **It runs before the persisted layer** ([`Layer::load_persisted`]), and
    /// that order is a claim about intent rather than about mechanism: a form
    /// you hand-wrote should beat the shipped default, and a form you
    /// deliberately kept at the REPL — the later act, and the more explicit one
    /// — should beat both. The same argument the other way round would make
    /// `persist!` unable to change anything you had ever written down.
    ///
    /// **One file, deliberately.** The shipped tree declares its load order in
    /// `phosphor/boot-files` and the boot reads that global once; a user layer
    /// that declared its own would be redefining the *same* global, so anything
    /// read afterwards is either the shipped fifteen names — which do not exist
    /// in the config home, and would be fifteen `unreadable` faults on every
    /// start — or a list a user has to restate in full. Both are worse than
    /// saying so: a second file in the config home wants a name of its own, and
    /// that is a vocabulary question rather than a load-path one.
    fn load_user_layer(&mut self, path: &Path) {
        // The whole path, shortened at `$HOME`. Every other layer is named by
        // its bare file name because the tree it sits in is unambiguous; this
        // one shares the name `init.scm` with the shipped boot file, and a
        // float that says `init.scm:1:2 · free identifier` in front of a person
        // with two of them has answered nothing.
        //
        // **It buys that with the overflow `note_if_no_layer` refuses to pay**,
        // and the two are not in conflict: there the path is a remedy and the
        // label is the news, here the path IS the fault's name and losing it
        // costs the reader the whole answer. `config::abbreviated` is what
        // keeps the bill to `~/.config/phosphor/init.scm` in the field — a
        // pty's temp directory is longer, which is why the row was measured
        // there first.
        let named = config::abbreviated(path);
        self.load_after_boot(path, named);
    }

    /// Runs the persisted layer — **last, after the whole load order and after
    /// the user's own file** (`T101`, then §34).
    ///
    /// The property this shape exists for was found by running it: `init.scm`
    /// evaluates to its last form *before* Rust reads the load order it
    /// declared, so a persisted `(keymap-set! …)` written there faults on the
    /// next boot with `keymaps.scm` not yet loaded. The old answer was to put
    /// `persisted.scm` last in `phosphor/boot-files`; the file lives in the
    /// config home now, so "last" is a call site rather than a list position
    /// and nobody can reorder it.
    fn load_persisted(&mut self, path: &Path) {
        // Named the way the load order would have named it, so the float's
        // `persisted.scm:3:1` reads like every other boot fault. The bare name
        // is unambiguous here: no file in the shipped tree is called that —
        // `runtime/init.scm`'s load order says why in full.
        let named = PathBuf::from(path.file_name().unwrap_or(path.as_os_str()));
        self.load_after_boot(path, named);
    }

    /// Runs one file of forms after the boot, naming its faults `file`.
    ///
    /// Form by form, for `crate::boot`'s reason one layer over: a stray paren
    /// in a file the header invites you to hand-edit must cost one line, not
    /// the keymap. A missing file is a fresh install and not a fault.
    ///
    /// **Never twice.** The config home holds both files this loads, and on a
    /// machine with no shipped tree they are one file: nothing declares
    /// [`PERSIST_FILE`], so the persist target falls back to [`INIT`], which is
    /// also the user's layer. Pointing `$PHOSPHOR_RUNTIME` at the config home
    /// makes the boot root a third name for it. [`Layer::booted_already`] is
    /// what keeps every form in it from running once per role it plays —
    /// reproduced on the built binary with `(displayln "BOOTED-ONCE")`, which
    /// printed twice.
    fn load_after_boot(&mut self, path: &Path, file: PathBuf) {
        if self.booted_already(path) {
            return;
        }
        self.after_boot.push(path.to_path_buf());
        self.ran = true;
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };

        let (forms, unterminated) = source::top_level_forms(&source);
        let mut ran = 0;
        for form in &forms {
            match self.runtime.eval(form.text(&source)) {
                Ok(_) => ran += 1,
                // `boot::steel_fault`, not a second copy of it. The copy that
                // was here kept Steel's CamelCase kind in the message while
                // `boot::message` stripped it, so one fault in `keymaps.scm`
                // and the same fault in `persisted.scm` reached one float in
                // two voices — the rule `T100` landed in this same window to
                // remove (Design Language §6, and `phosphor-term`'s
                // `no_error_kind_reaches_a_reader_as_a_rust_name`).
                Err(error) => self.report.faults.push(phosphor_steel::boot::steel_fault(
                    &file, &source, *form, &error,
                )),
            }
        }
        if let Some(open) = unterminated {
            let (line, column) = source::line_and_column(&source, open.start);
            self.report.faults.push(BootFault {
                file: file.clone(),
                at: Some(Position { line, column }),
                label: "unterminated",
                message: format!("this {} is never closed", open.what),
                source_line: source::nth_line(&source, line).map(str::to_owned),
            });
        }
        self.report.units.push(BootUnit {
            file,
            forms: forms.len(),
            ran,
        });
    }

    /// Whether this layer has already run the file at `path` (`T101`, §34).
    ///
    /// Two records, because the files reach it two ways. The boot report names
    /// its root and every file it read relative to it, so that half is the
    /// boot's own record rather than a second guess at the load order: a layer
    /// that declares `phosphor/boot-files` gets each of those checked too, and
    /// one that declares a `persist-file` the boot never loaded still loads
    /// here. [`Layer::after_boot`] is the other half — the user's layer and the
    /// persisted layer, which are one file whenever nothing declares a
    /// [`PERSIST_FILE`].
    fn booted_already(&self, path: &Path) -> bool {
        let booted = self.report.root.as_deref().is_some_and(|root| {
            self.report
                .units
                .iter()
                .any(|unit| same_file(&root.join(&unit.file), path))
        });
        booted || self.after_boot.iter().any(|ran| same_file(ran, path))
    }

    /// Says so when **nothing loaded an editor layer at all** — §34's
    /// disclosure half.
    ///
    /// *"So that an editor with no keymaps is a legible state rather than a
    /// mystery"* is §34's own phrasing of what it wanted, and layering answers
    /// most of it: a config home that used to replace the shipped tree now adds
    /// to it, and a user's file that throws reaches the float like any other
    /// fault. What layering cannot answer is the state where there was nothing
    /// to layer over — an installed binary run from outside its checkout with
    /// no `$PHOSPHOR_RUNTIME`. That editor has no keymaps, no statusline and no
    /// way to quit, and until this it said **nothing**, because a boot that read
    /// no files has no faults to report.
    ///
    /// # The guard is [`Layer::has_editor_layer`], and the first one was wrong
    ///
    /// It read `self.report.units.is_empty()`, which is *"nothing has loaded"*
    /// and not *"no editor loaded"* — and the two part company for exactly the
    /// population §34 is about. A user's own `init.scm` is a unit, so an
    /// installed binary outside a checkout, no `$PHOSPHOR_RUNTIME`, config home
    /// holding §34's own one-line file, reproduced §34's symptom verbatim and
    /// still said nothing: measured on a pty, `soft-wrap` applied, no
    /// statusline, no float, `SPC` drawing `unknown key <space>`, `ZQ` doing
    /// nothing, the process killed. **Writing the file the float told you to
    /// write is what turned the float off.** An empty `init.scm` did it too, on
    /// the same arm: `load_after_boot` pushes a unit for a file that read,
    /// whether or not it held a form.
    ///
    /// # Two remedies, and one of them is not always one
    ///
    /// *Write the file* is only advice when the file is not already there — a
    /// reader who wrote it wants the other half of the answer, which is that
    /// nothing shipped a layer for it to sit on. So the message names the file
    /// when nothing in the config home ran, and names the variable when
    /// something did. There is no config home at all on a third path
    /// (`$XDG_CONFIG_HOME` unset and no `$HOME`), and then only the variable is
    /// left.
    ///
    /// Called on the loop's path only ([`run`]) and not in [`vm`]/[`stack`]:
    /// `--eval` answers the expression it was asked and has no float to open,
    /// and a door that started printing boot findings would be answering a
    /// different question than the one on the command line.
    ///
    /// **The path goes in the message, not in the `file`.** `float::fault_rows`
    /// draws `place() · label` as one row and the float does not wrap, so a long
    /// path in the `file` pushes `no editor layer` off the right edge — observed
    /// on a pty, where the config home is a temp directory. The name of what is
    /// missing is short and always the same, so it is the half that goes there.
    ///
    /// [`Layer::load_user_layer`] makes the opposite call one screen up and
    /// both are right, which is worth saying because the reason above rules
    /// the other one out if it is read as a rule. There, the path **is** the
    /// fault's identity: two files are called `init.scm` once the layers stack,
    /// and a row reading `init.scm:1:2 · free identifier` in front of a person
    /// who has one of each has answered nothing, so the overflow is worth
    /// paying and `config::abbreviated` is what keeps the bill down. Here
    /// nothing is ambiguous — no file loaded — the label is the news, and the
    /// path is a remedy rather than a name.
    fn note_if_no_layer(&mut self, config: Option<&Path>) {
        if self.has_editor_layer() {
            return;
        }

        // Two ways out, and a reader with no keymaps needs both: write the file
        // — named, because *where* is the question — or point the variable at a
        // layer somebody else installed. Once the file exists the first is not
        // a way out any more, and repeating it is what made this float useless
        // to the person likeliest to see it.
        let wrote_one = !self.report.units.is_empty();
        let message = match config {
            Some(config) if !wrote_one => format!(
                "nothing loaded — write {}, or set $PHOSPHOR_RUNTIME",
                config::abbreviated(&config.join(INIT)).display()
            ),
            Some(_) => {
                "your init.scm ran over nothing — set $PHOSPHOR_RUNTIME to a layer".to_owned()
            }
            None => "nothing loaded — set $PHOSPHOR_RUNTIME to a layer".to_owned(),
        };
        self.report.faults.push(BootFault {
            file: PathBuf::from(INIT),
            at: None,
            label: "no editor layer",
            message,
            source_line: None,
        });
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

    /// Every binding the layer declares, in reading order — what a keymap
    /// surface draws.
    ///
    /// **Read live, on the frame that needs it, and never cached** (`T034`).
    /// The `SPC` popup is composed out of this call, so a `(keymap-set! …)`
    /// typed at the REPL is in the next popup with no invalidation of its own —
    /// which is what `a_repl_rebind_reaches_the_leader_popup` drives through
    /// the loop.
    ///
    /// Sets the flag, unlike [`Layer::resolve`]: `keymap-entries` is a scheme
    /// function the layer defines and may redefine, so the argument that
    /// `phosphor/resolve` only reads the table does not carry over to a name a
    /// user owns. It is only called while a sequence is half-typed, so the cost
    /// is one composition per keystroke of an open popup rather than one per
    /// frame.
    fn entries(&mut self) -> Vec<keymap::Entry> {
        self.ran = true;
        keymap::entries(&mut self.runtime)
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
    ///
    /// Composed from [`Layer::report`] rather than from `Runtime::boot_float`,
    /// because the persisted layer's faults are this type's and not the
    /// `Runtime`'s (`T101`).
    ///
    /// [`Layer::has_editor_layer`] and not the report's own units decides
    /// whether the footer may teach `:repl` — the two disagree the moment a
    /// user's `init.scm` is the only file that read, which is the state the
    /// float most often opens in (§34).
    fn boot_float(&self) -> Option<phosphor_core::view::Float> {
        let ex = if self.has_editor_layer() {
            ExLine::Bound
        } else {
            ExLine::Unbound
        };
        phosphor_steel::float::boot_float(&self.report, ex)
    }

    /// The boot report itself. Test-only: what the *program* does with a
    /// fault is open the float ([`Layer::boot_float`]), and a second reader of
    /// the same facts in the loop would be a second place to keep in step.
    #[cfg(test)]
    const fn report(&self) -> &BootReport {
        &self.report
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
    // `T101`: the tree that *booted* and the directory that is *written to* are
    // two different places now. In a dev checkout the first is the repository.
    //
    // Two environment reads and nothing else. Everything a test would want to
    // vary is an argument to [`stack`], which is what makes the order below
    // the *shipping* order rather than a second copy of it.
    stack(Runtime::root().as_deref(), config::config_dir().ok())
}

/// [`vm`] over two values rather than over the environment — **the stack
/// itself, and the only copy of it** (§34).
///
/// Split out for `phosphor_steel::runtime::root_from`'s reason and one more.
/// The reason it shares: `std::env::set_var` is `unsafe` in edition 2024 and
/// this workspace denies `unsafe_code`, so no in-process test can point
/// `$PHOSPHOR_RUNTIME` or `$XDG_CONFIG_HOME` anywhere.
///
/// **The reason it does not.** The tests beside this file used to reach the
/// stack through a hand-maintained reconstruction of `vm` — same four calls,
/// written twice — and its own doc claimed it made *"every call `vm()` makes,
/// in the same order"*. Nothing held that: a review moved the two `if let`
/// blocks below past each other in `vm` and the whole suite, including
/// `the_persisted_layer_runs_after_the_users_own_file`, stayed green. The
/// duplicate is gone rather than tested twice — the order is one function now,
/// and a test that calls it is looking at the editor the program builds.
fn stack(root: Option<&Path>, config: Option<PathBuf>) -> (Layer, Arc<AppHost>) {
    let host = Arc::new(AppHost::new(config));
    let runtime = boot(root, &host);
    let mut layer = Layer::new(runtime);
    // **The stack, and these three lines are the whole of the order** (§34).
    // Shipped tree, then the file you hand-wrote, then the file `persist!`
    // wrote for you. It is three call sites rather than three entries in a list
    // for `T101`'s reason: a position in `phosphor/boot-files` is something a
    // later edit can reorder, and the first time this order was a list the
    // rebind at the bottom of it came back as a free-identifier fault.
    if let Some(path) = host.user_layer() {
        layer.load_user_layer(&path);
    }
    if let Some(path) = host.persist_target() {
        layer.load_persisted(&path);
    }
    (layer, host)
}

/// Boots the editor layer against `host`, and reads back what it persists.
///
/// The reads happen **once, after the boot**: the layer decides the file
/// ([`PERSIST_FILE`]), the verb ([`PERSIST_VERB`]) and what is merely offered
/// ([`OFFERED_HEADS`]), and the host is behind the barrier and may not re-enter
/// the VM to ask when a form arrives.
fn boot(root: Option<&Path>, host: &Arc<AppHost>) -> Runtime {
    let runtime = Runtime::boot(root, Arc::clone(host) as Arc<dyn Host>);
    let read = |name| {
        runtime
            .global(name)
            .ok()
            .and_then(|value| phosphor_steel::convert::from_steel(&value).ok())
    };
    if let Some(Value::Text(file)) = read(PERSIST_FILE) {
        host.persist_to(&file);
    }
    // A layer that declares neither offers nothing and keeps everything, which
    // is the pre-`T101` behaviour and the right answer for a layer with no
    // opinion: the capability's own contract is *"appends a form"*.
    if let Some(Value::Text(verb)) = read(PERSIST_VERB) {
        let offered = match read(OFFERED_HEADS) {
            Some(Value::List(heads)) => heads
                .into_iter()
                .filter_map(|head| match head {
                    Value::Text(head) => Some(head),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        host.persist_policy(verb, offered);
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

    // No file is a command line, not a mistake (`T107`). The argument used to
    // carry `required_unless_present_any = ["eval", "repl"]` and this used to be
    // a second refusal behind it, so `phosphor` answered clap's *"the following
    // required arguments were not provided"* and exited `2` — a usage error for
    // the most ordinary thing you can type. [`run`] opens an empty buffer with
    // no name instead.
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
    // §34's disclosure half, and the loop is where it belongs: an editor that
    // found no layer to load has no keymaps and no way out, and the float is
    // the only surface that can say so before the keyboard is needed. The
    // directory comes off the host rather than from a second `config_dir()` —
    // see `AppHost::config_home`.
    layer.note_if_no_layer(host.config_home().as_deref());
    let boot = layer.boot_float();

    // Whether the file named on the command line has nothing behind it yet, so
    // the first frame can say so. `None` for `--repl`, which has no file at all
    // and is a different thing from a file that does not exist.
    let mut fresh: Option<PathBuf> = None;
    let (mut editor, path) = match &cli.path {
        Some(file) => {
            // **`opening` and not `read_to_string`, for the reason it gives:**
            // `phosphor notes.md` on a name nothing is behind is how you start
            // writing `notes.md`, and refusing it here would leave `:e` as the
            // only door to a new file. Its `Err` is still an error, and it
            // still lands on stderr before the alternate screen exists.
            let found = opening(file).map_err(|err| format!("{}: {err}", file.display()))?;
            if found.is_none() {
                fresh = Some(file.clone());
            }
            // The path as the user typed it. Repo-relative is what the mockups
            // draw, but the repo root is `phosphor-vcs`'s answer (`T071`) and
            // inventing one here would be a value nobody asked for.
            (
                buffer(
                    grammar_of(&host.languages(), file),
                    &found.unwrap_or_default(),
                    &theme,
                )?,
                Some(file.clone()),
            )
        }
        // **No file at all** — `--repl`, and since `T107` a bare `phosphor`.
        // One arm for both, because they are the same buffer: `"text"` is the
        // grammar for a name no declaration claims and a buffer with no name
        // claims none either, so the language machinery degrades through the
        // path it already had ([`adopt`] returns `None` and `grammar_of` is
        // never asked). The statusline says so by drawing no file segment at
        // all — see the notice below for why that is the whole answer.
        None => (buffer("text", "", &theme)?, None),
    };
    let (dirty, edits) = dirty_flag(&mut editor);

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

    // `R2` — `T030`'s journal, opened before the first frame so *"quit, reopen,
    // undo"* restores rather than starting over. A scratch buffer has no file to
    // key one on and gets a tree with nowhere to write itself.
    let (timeline, restore_note) = match path.as_deref() {
        Some(file) => Timeline::opened(file),
        None => (Timeline::detached(), None),
    };
    let mut editing = Editing::with_timeline(
        editor,
        path,
        Rc::clone(&dirty),
        timeline,
        Arc::clone(&host.diagnostics),
    );

    // `T033`'s ex line, and the one line of chrome that answers it. Both live
    // here rather than in a widget: `view::Node::Prompt` is the vocabulary's
    // shape for this and `phosphor-ui` defers it to `T058`, so what S3 can hold
    // is the primitives — a row of labels where the statusline goes, which is
    // where vim puts it too.
    let mut ex_line = String::new();
    // A journal that could not be opened outranks *"new file"* for the reason
    // the `:e` arm gives: one row, two true things, and the surprising one has
    // to be the one said out loud.
    //
    // **The third rung is `T107`'s** and it is last because it is the least
    // surprising of the three. `IMPLEMENTATION-PLAN.md`'s third invariant is
    // *nothing moves unless you asked*, not *nothing is said*: a bare
    // `phosphor` has no surface in front of it explaining itself the way
    // `--repl` does, so the one row of chrome says what this buffer is and what
    // turns it into a file. `Surface::Buffer` is the guard rather than
    // `cli.repl`, because a boot fault and the `--float` fixture are also
    // surfaces that answer the question themselves.
    let mut notice: Option<String> = restore_note
        .or_else(|| fresh.as_deref().map(new_file))
        .or_else(|| (editing.file.is_none() && matches!(surface, Surface::Buffer)).then(no_file));

    // `T035`'s latch, and the row it produced. The latch is per *session*,
    // which is what makes it the loop's and not the buffer's; the row lives
    // until the next keystroke acknowledges it, the way a notice does.
    let mut taught = UnknownKeyHint::new();
    let mut hint: Option<Node> = None;

    // `T097`'s page, once `:help` has asked for one. Composed when the ask is
    // drained rather than per frame: `Layer::entries` re-enters the VM and
    // marks the frame stale, so a page rebuilt every frame would refill the
    // statusline cache every frame for a screen that cannot change while it is
    // up (nothing but `esc` and `q` reaches the loop while `Surface::Help` has
    // it). Read at open is what makes the liveness claim true — a REPL rebind
    // is in the next `:help`, with no wiring of its own.
    let mut help_page: Option<phosphor_core::view::Float> = None;

    let mut term = Term::new()?;
    // `R10` — `T027`'s degradation, reachable at last. `phosphor-core`'s
    // `legacy_chord` fallback is built and tested and could never fire, because
    // nothing ever told the machine which protocol was negotiated. The snippet
    // is `phosphor-term`'s own header, and `$PHOSPHOR_KEYBOARD=legacy|kitty`
    // forces either side of it without different hardware.
    machine.set_protocol(match term.capabilities().keyboard {
        KeyboardProtocol::Kitty => key::Protocol::Kitty,
        KeyboardProtocol::Legacy => key::Protocol::Legacy,
    });

    // **The one queue** (`events`). The loop below blocks on `queue.recv()`
    // rather than on the terminal, so a producer that is not the keyboard has
    // somewhere to arrive: `S4`'s LSP client, `S6`'s ACP stream and `T071`'s
    // VCS polls all clone a `Poster` and post into this.
    //
    // **After `Term::new()`, never before** — keyboard-protocol negotiation
    // reads the terminal's answer back through `crossterm`'s own event source,
    // and a reader thread started first would take it. `read_terminal` says so
    // at length.
    let (queue, poster) = events::open();
    // **The first producer** (`T036`). `events`' `AppEvent::Posted` carried an
    // `expect(dead_code)` saying it should disappear when one landed; this is
    // the line that made it disappear. The clone is what lets the terminal
    // reader take the other one — the queue is multi-producer and this is the
    // second of the design's four.
    let post = lsp::sink(poster.clone());
    // **Two doors, because a wake is not a mutation.** The first carries what a
    // server produced; the second carries *that a server changed*, which has no
    // Action to be and is what keeps the statusline's server chip from being
    // correct and stale — see `lsp::waking`.
    let servers = LanguageServers::start(Arc::clone(&post), lsp::waking(poster.clone()));
    events::read_terminal(poster);

    // What this editor is still owed an answer to — see `Outstanding`, which
    // is where the reason it counts rather than remembering the last request
    // is written down.
    let mut outstanding = Outstanding::default();
    // Whether the last key typed into the buffer while in insert mode. Set by
    // the key arm and read by the trigger below; a `bool` rather than a
    // re-derivation, because *"the edit stream moved"* is only knowable across
    // the call that moved it.
    let mut typing = false;
    // When the debounce is up, and so when typing may ask the server anything.
    // [`None`] is *nothing is pending* — which is most of the time, and is what
    // keeps a quiet editor parked in `recv` with no deadline at all.
    let mut due: Option<Instant> = None;
    // Whether the *previous* turn ended in insert mode. What makes "the
    // signature closes when the insert session that raised it ends" a
    // transition rather than a state — read one way it would close a hover the
    // moment it arrived, since normal mode is where hover is read.
    let mut was_inserting = false;

    // The document the servers have been told about, and how many edits ago.
    // `didChange` is sent from the top of the loop rather than from the edit,
    // because an Action that edits several times — `J`, `>` over a range,
    // accepting a completion — is one change as far as a server is concerned,
    // and telling it three times would have it answering about text the user
    // never saw.
    let mut synced: Option<Document> = adopt(&mut editing, &host.languages(), &servers);
    let mut sent = edits.get();

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
        // `T104` — what one indent level is, and how wide a `\t` draws. Both
        // are read per pass for the reason `soft-wrap` and the completion floor
        // are: the option is the editor layer's, `(set-option! …)` at the REPL
        // has to reach the next keystroke, and a value cached at boot would
        // make the setting a fact about the last restart. `set_tab_width` is
        // free when the number has not moved and rebuilds the row stream when
        // it has, because a wider tab moves every wrap point.
        editing.indent_style = indent_style(&host, &host.languages(), editing.language.as_ref());
        editing.editor.set_tab_width(editing.indent_style.tab_width);

        // `8e`'s whitespace marks are INSERT-only, and the mode is the
        // machine's — the first thing in this loop that is not hardcoded.
        //
        // **The boundary conversion is gone.** It existed because
        // `soft_wrap::EditMode` was a two-value copy that said of itself *"the
        // real mode enum is `spine`'s and does not exist yet (`T026`)"*; the
        // widget re-exports `phosphor_core::request::EditMode` now, so there is
        // one enum and nothing to convert.
        soft_wrap::set_mode(&mut editing.editor, machine.mode());

        // `T038`'s document sync. Once per turn and only when the edit stream
        // moved: `T036` sent `didOpen` and nothing after it, so every request
        // against a file the user had typed into asked about the text as it was
        // when the buffer opened — *"completions for a prefix that is no longer
        // there is not a stale-looking list; it is a wrong one, and nothing on
        // screen says so"* (`LanguageServers::change`).
        if edits.get() != sent {
            sent = edits.get();
            if let (Some(language), Some(document)) = (editing.language.clone(), synced.as_ref()) {
                servers.change(&language, document.path.clone(), editing.contents());
            }
        }

        // `T040` — the diagnostics on screen, resolved against *this* buffer.
        //
        // **The state column is computed once, here.** `diagnostics.rs`'s
        // header is explicit that its `regions` are one source among several
        // and that the host *"concatenates them with every other source of
        // regions — unseen edits, threads, failures — and calls
        // `gutter::state_column` once"*, which is what makes §3's ladder a
        // property of the composition rather than of one widget. There is one
        // source today; `T041` adds the rest to this `Vec` and nothing else
        // here changes.
        let published = synced
            .as_ref()
            .map(|document| editing.diagnostics.of(&document.key))
            .unwrap_or_default();
        let shown = DiagnosticsVm::new(&published);
        let mut regions = Vec::new();
        regions.extend(shown.regions(&editing.editor));
        let rows = shown.rows(&theme);
        let underlines = shown.underlines(&editing.editor, &theme);
        virtual_text::install(&mut editing.editor, &rows);
        editing.editor.set_styled_spans(underlines);
        // As many rows as any region reaches and no more. `BufferView`'s own
        // contract is that *"rows past the end of the slice are
        // `StateMark::None`"*, so a column sized to the buffer would be the
        // same answer with a `Vec` the length of the file in it.
        let deepest = regions.iter().map(|region| region.rows.end).max();
        let marks = gutter::state_column(&regions, deepest.unwrap_or(0));

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
            // `6d`, the same way: an empty root is a float over what the
            // widgets painted, so the buffer and the statusline stay behind it
            // and §9 dims them.
            (Surface::Help, _) => help_page
                .as_ref()
                .map(|float| Tree::new(Node::Empty {}).with_float(float.clone())),
            // `T038`, `T039` — the completion list and signature help, as
            // floats over the buffer you are still typing into. Same shape as
            // the two above and for the same reason: `Mood::Passive` *"is not
            // in front of anything"* (§9), so an empty root is what leaves the
            // code at full strength behind it.
            (Surface::Buffer, _) => passive_float(&editing),
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
            // `7c`'s `rust-analyzer ✓`, and the only place a failed server is
            // ever heard from — see `server_chip`.
            server: editing.language.as_ref().and_then(|language| {
                server_chip(&servers.state(language), editing.server.as_deref())
            }),
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

        // `R17` — which-key. **Composed here, from the live table**, which is
        // the whole of `T034`'s liveness claim: `Layer::entries` asks the VM on
        // the frame that draws the popup, so a `(keymap-set! …)` typed at the
        // REPL is in the next one with nothing else to wire. The popup is not
        // `SPC`'s alone — it opens for whatever prefix is half-typed, because
        // *what is bound under what I have typed* is which-key's whole question
        // and `SPC` is one prefix among them.
        let leader = matches!(surface, Surface::Buffer)
            .then(|| under(&mut layer, &machine))
            .unwrap_or_default();

        let overlay = Overlay {
            chrome,
            leader: &leader,
            hint: hint.as_ref(),
            marks: &marks,
            completion: editing.completion.as_ref(),
            signature: editing.signature.as_ref(),
        };
        term.draw(|frame| {
            draw(
                frame,
                &editing.editor,
                &theme,
                status_tree,
                &floats,
                tree.as_ref(),
                &overlay,
            );
        })?;

        // **The one blocking point, and the only one.** `recv_until` parks
        // until a producer has something — or until a pending deadline passes,
        // which today is only the completion debounce. `Wake::Closed` is
        // *every* producer gone, the terminal reader included: nothing left in
        // the process could ever wake this loop, and waiting forever on that is
        // the one state a modal editor must not be able to reach.
        //
        // **The deadline is dropped while a request is in flight**, and that is
        // the line that keeps a quiet editor quiet. A deadline already in the
        // past makes `recv_until` return immediately, so keeping one here while
        // nothing may be sent would spin the loop at full tilt for the length
        // of a server round trip. The answer arriving is itself an event, so
        // parking is right: `due` survives the wait and is honoured on the pass
        // after the answer lands, which is the same re-arm the one-in-flight
        // gate has always had.
        let deadline = if outstanding.awaiting(Lookup::Completion) {
            None
        } else {
            due
        };
        let event = match queue.recv_until(deadline) {
            events::Wake::Event(event) => event,
            // Nothing happened; the deadline is what woke us, and what was due
            // is decided below with everything else this pass decides. `Woke`
            // is the variant its own doc reserved for exactly this — *"the
            // spinner frame and the elapsed tick are the other two waiting on
            // this variant"* — and its arm is already a no-op, which is the
            // whole of what a debounce wake has to do here.
            events::Wake::Elapsed => events::AppEvent::Woke("debounce"),
            events::Wake::Closed => break,
        };
        match event {
            // Everything a terminal can send. **A new producer adds no arm
            // here** — it arrives below as an Action, which is what makes the
            // extension point (`events`) something another agent can use
            // without editing the file that owns the loop.
            events::AppEvent::Term(event) => {
                // A notice says what the last ex line did, and the next key
                // is the acknowledgement — there is no dismiss and nothing to
                // remember. `8e`'s hint is acknowledged the same way: it has
                // been on screen for a frame, and the session's one hint is
                // already spent.
                if matches!(event, Event::Key(_)) {
                    notice = None;
                    hint = None;
                    // `T039`'s float is acknowledged the same way — **in
                    // normal mode**. Hover answers a question you asked once
                    // and the next key is the answer to *"have you read it"*,
                    // which is the only dismissal a passive float can offer:
                    // §4's exception means it has no footer to put a key in.
                    //
                    // **In insert mode that rule is exactly backwards**, and
                    // it shipped that way for a window: signature help exists
                    // to be read *while you type the arguments*, so the first
                    // character of the first argument — the keystroke the
                    // float is there to support — dismissed it. Measured at
                    // `CP-4`: `<C-s>` inside `add(` drew the signature with
                    // `left: i32` in the active tone, and typing `1` cleared
                    // it. In insert, the float is closed by leaving insert
                    // (below, where the completion list is), by `<C-e>` and by
                    // `esc` — which is `cancel-completion`, and clears both.
                    if machine.mode() != EditMode::Insert {
                        editing.signature = None;
                    }
                }
                match event {
                    Event::Key(key) if !is_press(key) => {}
                    Event::Key(key) if matches!(surface, Surface::Repl) => {
                        match repl_key(key, &mut repl, &mut layer) {
                            ReplStep::Handled => {}
                            ReplStep::Close => surface = Surface::Buffer,
                            ReplStep::ToBuffer => {
                                editing.editor = session_buffer(&repl, &theme)?;
                                track_dirty(&mut editing.editor, &dirty, &edits);
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
                    Event::Key(key) if matches!(surface, Surface::Ex) => {
                        match ex_key(key, &mut ex_line) {
                            ExStep::Typing => {}
                            ExStep::Cancel => surface = Surface::Buffer,
                            ExStep::Submit => {
                                surface = Surface::Buffer;
                                notice = submit_ex(&mut layer, &mut editing, &ex_line);
                            }
                        }
                    }
                    // **Every key, through one machine.** The keymap is still asked of
                    // the VM on every keystroke and still never cached (`T022`); what
                    // changed is that the answer is one of two tables the machine
                    // resolves against, and what is left over is a grammar rather than
                    // the fork's handler.
                    Event::Key(key) => {
                        if let Some(pressed) = decode(key) {
                            let before = edits.get();
                            Session {
                                machine: &mut machine,
                                layer: &mut layer,
                                seed: &mut seed,
                                editing: &mut editing,
                            }
                            .key(pressed);
                            typing = machine.mode() == EditMode::Insert && edits.get() != before;
                        }
                    }
                    Event::Mouse(mouse) => {
                        for action in mouse_actions(&mut machine, &editing, mouse) {
                            // `Input::SetMode` is the machine reporting a
                            // transition it has already made — `Machine::click`
                            // and `Machine::drag` mutate it directly, the way
                            // `feed` does — so there is nothing here to apply
                            // and `Editing` has no arm for one.
                            if !matches!(action, Action::Input(_)) {
                                let _ = editing.apply(&action);
                            }
                        }
                    }
                    // A resize redraws from the new size on the next turn of the loop;
                    // so does everything else this arm swallows (focus, paste).
                    _ => {}
                }
            }
            // The second producer's door, and `T036`'s LSP client is what
            // arrives through it: diagnostics a server published unasked, and
            // the answers to the lookups this loop asked for. The two are
            // routed differently and `answers` is the difference.
            // **Nothing to apply — the frame at the top of the next turn is
            // the whole of it.** A producer that has something to *show* and
            // nothing to *do* arrives here: today that is a language server
            // changing state, and the statusline's server chip is what says so.
            events::AppEvent::Woke(_) => {}
            events::AppEvent::Posted(posted) => {
                if outstanding.answers(&posted.action) {
                    // The user's own request coming back. Applied through
                    // `act` and not `apply`, for the reason `deliver` gives:
                    // a reveal is `View::Scroll`, and nothing that is not the
                    // user may move the viewport the user is looking at.
                    drop(editing.act(&posted.action));
                } else if let Some(note) = deliver(&mut editing, &posted) {
                    notice = Some(note);
                }
            }
        }

        // What the Actions asked for that only the loop can do: `open-file`
        // needs the theme and the language table, and `open-prompt` needs the
        // surface. Both are recorded by `Editing::act` and performed here, for
        // the same reason `Intent` exists — the thing that decides is not the
        // thing that owns.
        if let Some(file) = editing.open.take() {
            // **`gd` into the file you are already in is the common case**, and
            // re-reading it from disk would throw away everything typed since
            // the last write. Compared through `lsp::absolute` because the two
            // spellings genuinely differ: the buffer holds the path as the user
            // typed it (`src/lib.rs`) and a server answers in absolute URIs.
            let same = editing
                .file
                .as_deref()
                .is_some_and(|open| lsp::absolute(open) == lsp::absolute(&file));
            // And a jump *out* of a dirty buffer says so rather than silently
            // discarding it. `close-buffer` and `quit` both raise
            // `WouldLoseWork`; `open-file` did not, and `gd` is the verb that
            // made that reachable without an ex line — measured at `CP-4`,
            // where a `[+]` buffer lost its edit with no prompt and no notice.
            // There is no `force` on `open-file` to offer, so the sentence is
            // the whole refusal: `:w` and press it again.
            if !same && dirty.get() {
                editing.open_at = None;
                notice = Some(format!(
                    "{}: {}",
                    file.display(),
                    phosphor_steel::answer::why(&Refusal::WouldLoseWork)
                ));
            } else if same {
                // Nothing to open, so nothing about the buffer changes: the
                // language, the journal, the server's document and the dirty
                // flag are all already this file's. `gd` is a motion here, and
                // it is the user's own — `apply`, so the viewport follows.
                if let Some(at) = editing.open_at.take() {
                    drop(editing.apply(&Action::Motion(MotionAction::SetCursor {
                        position: at,
                        buffer: None,
                    })));
                }
            } else {
                match opening(&file) {
                    Ok(found) => {
                        // Empty when the path is free. `buffer` takes the same
                        // grammar either way — a declaration claims the
                        // extension, and the extension is in the name.
                        let fresh = found.is_none();
                        let text = found.unwrap_or_default();
                        editing.editor =
                            buffer(grammar_of(&host.languages(), &file), &text, &theme)?;
                        track_dirty(&mut editing.editor, &dirty, &edits);
                        let (timeline, note) = Timeline::opened(&file);
                        editing.timeline = timeline;
                        editing.depth = 0;
                        // A journal that could not be opened outranks *"new
                        // file"*: both are true, one row holds one of them, and
                        // the surprising one is the one that has to be said.
                        // The other is visible in the buffer, which is empty.
                        notice = note.or_else(|| fresh.then(|| new_file(&file)));
                        editing.file = Some(file);
                        surface = Surface::Buffer;
                        // The server hears about the swap in both directions:
                        // `didClose` for what it was holding — after which it falls
                        // back to what is on disk, which is the specification's own
                        // rule — and `didOpen` for what took its place.
                        if let (Some(language), Some(document)) =
                            (editing.language.clone(), synced.take())
                        {
                            servers.close(&language, &document.path);
                        }
                        synced = adopt(&mut editing, &host.languages(), &servers);
                        sent = edits.get();
                        // A new buffer is a new place; a list anchored in the old
                        // one would be drawn over code it knows nothing about.
                        editing.close_completion();
                        editing.signature = None;
                        // `gd` landing. Applied as the Action it is, so the
                        // cursor moves through the one path every cursor move
                        // goes through and the viewport follows it — `apply`
                        // rather than `act`, because this *is* the user's jump.
                        if let Some(at) = editing.open_at.take() {
                            drop(editing.apply(&Action::Motion(MotionAction::SetCursor {
                                position: at,
                                buffer: None,
                            })));
                        }
                    }
                    Err(error) => {
                        editing.open_at = None;
                        notice = Some(format!("{}: {error}", file.display()));
                    }
                }
            }
        }
        if editing.prompt.take().is_some() {
            ex_line.clear();
            surface = Surface::Ex;
        }
        // `T097` — `:help`, composed from the live table. A topic that narrows
        // to nothing says so on the statusline rather than opening an empty
        // float: an empty grid is indistinguishable from a broken one.
        if let Some(ask) = editing.help.take() {
            match help_float(&mut layer, &ask) {
                Some(float) => {
                    help_page = Some(float);
                    surface = Surface::Help;
                }
                None => notice = Some(no_help(&ask)),
            }
        }
        // `T038`'s **done when**: *"typing in insert mode in the running binary
        // raises the float"*. So a completion is not only a key — it is what
        // typing does, which is what makes the float a completion list rather
        // than a lookup surface.
        //
        // **Gated on a server that is actually ready**, and that gate is
        // load-bearing rather than an optimisation: without it, every
        // keystroke in a buffer whose language has no server (or whose server
        // failed to spawn) would raise the *"no language server for this
        // buffer"* notice below, once per character, and bury the statusline
        // under it. A key that asks explicitly still says that, because there
        // the user asked.
        //
        // **One request in flight at a time, and the edits made while it is in
        // flight are coalesced into the next one.** This is the whole of the
        // debounce, and it is a gate on `Outstanding` rather than a timer: the
        // loop blocks on `recv` and has no tick to hang a timer off, so *"ask
        // again when the last answer is in"* is the shape that fits. A burst of
        // typing costs one round trip plus one, instead of one per character.
        //
        // **`typing` is deliberately not cleared while a request is
        // outstanding**, which is what makes the second request happen at all:
        // the first answer is for a prefix the cursor has already left, the
        // `at` guard drops it, and without the re-arm a fast typist would be
        // left with no list until they pressed one more key.
        //
        // The gate used to read `editing.lookup.is_none()` alone, which is a
        // field this loop drains every pass and is therefore always true by the
        // time it is read. See `Outstanding` for what that cost.
        //
        // **And it had no minimum, which `CP-4` reported.** Typing raised the
        // list on the first character of a word, and on a keystroke that typed
        // no word at all — a space asked the server for its whole table. The
        // floor is the editor layer's ([`COMPLETION_MIN_CHARS`]) and it is read
        // per pass for the same reason `soft-wrap` is: the option can change at
        // the REPL and a value cached at boot would make the setting a fact
        // about the last restart.
        //
        // **Measured on `Editing::prefix_len`, not on keystrokes since the
        // float closed**, and the two genuinely differ. `prefix_len` is the
        // span the request is *about* — what `Editing::prefix` filters the
        // answer against and what `Editing::accept` overwrites — so a floor
        // measured on it cannot suppress a list for a word that is already
        // long: put the cursor in the middle of a twelve-character identifier
        // and type one character and you get the list, which is right, where a
        // keystroke counter would call that one character. It also carries no
        // state, so there is no reset to forget on `<C-e>`, on a motion, on
        // leaving insert or on opening a file — four places a counter would
        // have to be cleared and one of them would eventually be missed.
        //
        // What it deliberately does **not** change: `<C-e>` inside a long word
        // still re-raises the list on the very next character, because the
        // prefix is already past the floor. That is a dismissal question rather
        // than a minimum one, and this gate is not where it would be answered.
        //
        // **The floor is here and not in `Editing::act`**, which is what makes
        // `<C-x>` ignore it. `Action::Lsp(RequestCompletion)` is what the key
        // sends and what the MCP and CLI doors send; a floor inside that arm
        // would make *asking* subject to a threshold for typing, and the user
        // pressing `<C-x>` on an empty line has asked.
        //
        // **And a floor on identifier prefixes cannot be the only gate**, which
        // the reasoning above never weighed: `prefix_len` counts word characters
        // backwards, so `foo.` measures **zero** and a floor of two hid the most
        // common completion moment in every dotted language behind `<C-x>`.
        // `.`, `::` and `(` are not prefixes at all — they are the server's own
        // `completionProvider.triggerCharacters`, which is the one list that
        // knows what a language means by *"ask now"*, and
        // `LanguageServers::completion_triggers` reads it off `initialize`. A
        // server that advertises none is exactly as it was.
        let floor = completion_floor(&host);
        let ready = editing
            .language
            .as_ref()
            .filter(|language| servers.state(language).is_ready());
        // **Typing arms the deadline; it does not send.** Every keystroke
        // pushes it out again, so a burst of typing costs one request placed
        // where the fingers stopped rather than a chain of them each answering
        // about a prefix the cursor has already left. That was `CP-4`'s
        // *"completion seemed to take longer than it should have"* — not a slow
        // server, a list that was always one round trip stale.
        if core::mem::take(&mut typing) {
            due = Some(Instant::now() + COMPLETION_DEBOUNCE);
        }
        // Cleared whether or not the ask happens. A deadline left set after it
        // has passed is one `recv_until` would answer instantly forever, and
        // the floor and the ready-server gate below are both reasons it can
        // pass with nothing sent. The re-arm the one-in-flight gate needs is
        // *above* this, in the deadline the loop parks on: while a request is
        // outstanding the loop takes no deadline at all, so this line is not
        // reached until the answer has landed.
        let elapsed = due.is_some_and(|at| Instant::now() >= at);
        if elapsed {
            due = None;
        }
        if editing.lookup.is_none()
            && !outstanding.awaiting(Lookup::Completion)
            && elapsed
            && let Some(language) = ready
            && (editing.prefix_len() >= floor
                || editing.after_trigger(&servers.completion_triggers(language)))
        {
            editing.lookup = Some(Lookup::Completion);
        }
        // A completion list belongs to the insert session that raised it.
        // `<esc>` is the machine's key and closes no float — §9's top-down
        // rule is about surfaces — so leaving insert mode is what closes this
        // one, and it closes it wherever the mode changed from.
        if machine.mode() != EditMode::Insert && editing.completion.is_some() {
            editing.close_completion();
        }
        // And the signature raised inside that session goes with it. A hover
        // read in normal mode is dismissed by the next key above; this is the
        // half that key no longer does, because in insert it is the argument
        // being typed.
        if machine.mode() != EditMode::Insert && was_inserting && editing.signature.is_some() {
            editing.signature = None;
        }
        was_inserting = machine.mode() == EditMode::Insert;
        // `T038`, `T039` — the ask, sent from the one place that holds the
        // servers. Non-blocking, like everything else on that client: the
        // answer arrives on the runtime thread, posts into the queue, and is
        // applied on a later turn of this loop.
        if let Some(lookup) = editing.lookup.take() {
            match (editing.language.clone(), synced.as_ref()) {
                (Some(language), Some(document)) => {
                    let at = editing.text().cursor();
                    outstanding.sent(lookup);
                    let path = document.path.clone();
                    // **The word being completed goes with the request**, and
                    // it is read here rather than when the answer lands
                    // because it is the prefix the request is *about*. The
                    // `at` guard already drops an answer the cursor has left,
                    // so the two agree by construction.
                    let prefix = editing.prefix();
                    servers.look_up(
                        &language,
                        lookup,
                        path,
                        at,
                        answering(lookup, at, prefix, &post),
                    );
                }
                // Not a failure and not silence: a second-tier buffer has no
                // server by construction, and a key that asked one a question
                // has to say so (`T098`'s rule).
                _ => notice = Some("no language server for this buffer".to_owned()),
            }
        }
        // `T036` — `gd`. A question answers in places, and the host is what
        // turns one into an `open-file`: the client cannot, because a
        // `PaneRef` is knowledge `phosphor-buffer` does not have and must not
        // guess (`lsp::Locations`).
        if let Some(question) = editing.question.take() {
            match (editing.language.clone(), synced.as_ref()) {
                (Some(language), Some(document)) => {
                    let at = editing.text().cursor();
                    let path = document.path.clone();
                    servers.ask(&language, question, path, at, jumping(&post));
                }
                _ => notice = Some("no language server for this buffer".to_owned()),
            }
        }
        // `T036` — `restart-language-server`. The state goes back to
        // `Starting` and every document this client holds for that language is
        // replayed to the new process, which is the client's own contract.
        if let Some(language) = editing.restart.take() {
            servers.restart(&language);
            notice = Some(format!("restarting {}'s language server", language.0));
        }
        // `T107` — an Action that succeeded and has something to say anyway.
        // Drained before the refusal below because the two cannot both be set
        // by one key and the order should still be the honest one: a refusal is
        // what the key *did*, and a note is a caveat on what it did.
        if let Some(note) = editing.note.take() {
            notice = Some(note);
        }
        // `T098` — a key that was refused says why, the way an ex line does.
        if let Some(refusal) = editing.refused.take() {
            notice = Some(phosphor_steel::answer::why(&refusal));
        }
        // `R18` — `T035`'s hint, on the unbound-key path. The latch is what
        // makes *"shown once"* on the row true: `teach` answers `Some` exactly
        // once in the life of a session and `None` for every unknown key after,
        // so a caller cannot draw a second one.
        if let Some(key) = editing.unknown.take() {
            hint = taught.teach(&key);
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
                    track_dirty(&mut editing.editor, &dirty, &edits);
                    surface = Surface::Buffer;
                }
                // The CLI and MCP doors, arriving in the editor layer's own
                // words. `Layer` runs it, so the frame cache learns that
                // arbitrary scheme ran from the one place that records it.
                // `T100`: this was an `if let Outcome::Refused(…)`, which is why
                // it is worth a comment — a form that *raised* went nowhere and
                // said nothing, and adding the case to the enum did not make
                // that a compile error the way it did at the other two sites.
                // The shared reduction is what closes it.
                Intent::Keymap(form) => {
                    if let Some(said) = phosphor_steel::answer::trouble(&layer.evaluate(&form)) {
                        notice = Some(said);
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

/// What the statusline says about this buffer's language server (`7c`,
/// `T036`).
///
/// **The one thing on screen that ever mentions a server**, which is why every
/// state that is not *"nothing here"* answers something. `ServerState` was
/// complete, tested and read by exactly one call site — the insert-mode
/// trigger's `is_ready()` — so a server that could not start was
/// indistinguishable from a language with no server at all: the editor drew the
/// buffer, drew the statusline, and said nothing, forever. Measured at `CP-4`
/// with typescript-language-server failing `initialize`; two presses of `<C-x>`
/// over forty seconds produced no float, no notice and no refusal.
///
/// The states, and why each says what it does:
///
/// * **`NotStarted` / `Stopped`** — nothing. A second-tier buffer is an honest
///   thing to be (§5's *"always present and truthful"* is about the session),
///   and a chip reading `✗` beside a Markdown file would be a claim that
///   something is wrong.
/// * **`Starting`** — the declaration's own command with an ellipsis. rust-analyzer
///   answers `initialize` fast and indexes afterwards, so this is short; a
///   buffer that stays on it is the *"wedged rather than slow"* case
///   `READY_TIMEOUT` exists for, and the chip is what makes that visible.
/// * **`Ready`** — `rust-analyzer ✓`, `7c` exactly, and the name is the
///   **server's own** `serverInfo.name` rather than the command we ran, because
///   a wrapper script named `ra-wrapper` still speaks for rust-analyzer.
/// * **`Crashed`** — the command, `✗`, and [`phosphor_buffer::lsp::Failure`]'s own sentence, which
///   for a missing binary is the OS's words: `rust-analyzer ✗ could not start:
///   No such file or directory (os error 2)`. §11 sheds it before the cursor
///   position on a narrow terminal; nothing shortens it here, because the whole
///   value of that string is that it is specific.
fn server_chip(state: &ServerState, named: Option<&str>) -> Option<String> {
    let named = named?;
    match state {
        ServerState::NotStarted | ServerState::Stopped => None,
        ServerState::Starting => Some(format!("{named} …")),
        ServerState::Ready(identity) => Some(format!("{} ✓", identity.name)),
        ServerState::Crashed(failure) => Some(format!("{named} ✗ {failure}")),
    }
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

/// Takes `rows` off the bottom of `body` for a strip, or [`None`] if they will
/// not fit.
///
/// §11's *"narrow terminals drop, never squeeze"*, applied to height: a strip
/// that cannot have its rows without leaving the buffer at least one is not
/// drawn at all, rather than drawn shorter than it needs.
fn take_rows(body: &mut Rect, rows: u16) -> Option<Rect> {
    if rows == 0 || body.height <= rows {
        return None;
    }
    body.height -= rows;
    Some(Rect {
        y: body.y + body.height,
        height: rows,
        ..*body
    })
}

/// How many answers this editor is still owed, per [`Lookup`] (`T038`,
/// `T039`).
///
/// **What separates an answer from an unsolicited push**, and that separation
/// is `action.rs`'s own: the three `ingest-*` verbs a lookup answers with are
/// `Deny` so that *"an agent that may not ask could not make the answer
/// appear"*, which means they may not go through [`deliver`]'s producer
/// policy — and an editor that applied any ingest it was handed would have
/// reopened exactly that hole from the server's side instead.
///
/// # Why it counts rather than holding the last request
///
/// It held `Option<Lookup>` for one window and the cost was **a refusal on the
/// statusline while you typed**. The insert-mode trigger asks once per edit; a
/// slot keeps only the newest, so every superseded answer arrived unrecognised,
/// fell through to `deliver`, and painted `lsp: denied to a producer — only the
/// keyboard asks for this` over the file being edited. Reproduced at `CP-4` on
/// a pty against a real rust-analyzer, at a 350 ms gap between keystrokes —
/// slower than human typing — and in a nine-character burst, where the last
/// frame of the session is the notice.
///
/// So a request is **owed** until it is answered, and
/// [`LanguageServers::look_up`] promising *exactly one answer per request on
/// every path* is what makes a count safe: it cannot leak, and it needs no
/// timeout of its own. Applying a superseded answer is harmless because the
/// `at` guard in the ingest arms drops one the cursor has left — that is the
/// mechanism the trigger's comment always claimed, one layer down from where
/// it was written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Outstanding {
    completion: u32,
    signature: u32,
    hover: u32,
}

impl Outstanding {
    /// The counter `lookup` is answered out of.
    const fn slot(&mut self, lookup: Lookup) -> &mut u32 {
        match lookup {
            Lookup::Completion => &mut self.completion,
            Lookup::SignatureHelp => &mut self.signature,
            Lookup::Hover => &mut self.hover,
        }
    }

    /// A request has gone to a server.
    const fn sent(&mut self, lookup: Lookup) {
        *self.slot(lookup) = self.slot(lookup).saturating_add(1);
    }

    /// Whether an answer to `lookup` is still owed. Read by the insert-mode
    /// trigger, so that typing sends one request at a time rather than one per
    /// keystroke.
    const fn awaiting(&mut self, lookup: Lookup) -> bool {
        *self.slot(lookup) > 0
    }

    /// Whether `action` is an answer this editor is owed — and takes it off the
    /// count if it is.
    ///
    /// **Matched on the kind of answer, not merely on "is this an ingest".** A
    /// server that pushed `IngestCompletions` while the editor was waiting only
    /// for hover would otherwise open a completion list nobody asked for, which
    /// is the same hole `action.rs` rates these three `Deny` to close.
    fn answers(&mut self, action: &Action) -> bool {
        let lookup = match action {
            Action::Lsp(LspAction::IngestCompletions { .. }) => Lookup::Completion,
            Action::Lsp(LspAction::IngestSignatureHelp { .. }) => Lookup::SignatureHelp,
            Action::Lsp(LspAction::IngestHover { .. }) => Lookup::Hover,
            _ => return false,
        };
        let owed = self.slot(lookup);
        if *owed == 0 {
            return false;
        }
        *owed -= 1;
        true
    }
}

/// The callback one lookup's answer comes back through (`T038`, `T039`).
///
/// **The `at` it captures is the cursor the request was made at**, and every
/// ingest carries it so *"an answer the cursor has left is dropped rather than
/// drawn in the wrong place"* — the declaration's own words, and the reason
/// this is a closure over a position rather than a plain forward.
///
/// [`Insight::Nothing`] is why `lookup` is captured too: it is the client's
/// one answer for *every* way of not answering — no server, no reply inside
/// the timeout, a reply with no content — and which **empty** Action that
/// becomes depends on which float has to close. A hover that answered an empty
/// completion list would leave stale prose beside the cursor forever.
///
/// **`prefix` is the word the request was made inside**, and
/// [`phosphor_buffer::lsp::narrow`] is applied here rather than in the client
/// or in the widget: a transport does not know what has been typed, and by the
/// time a widget sees the list it is a `CompletionVm` with no `filterText` left
/// on it. Without it, a real server's answer — *the whole set that could go at
/// this position*, which is what the protocol says a server sends — buries the
/// code being typed into.
fn answering(lookup: Lookup, at: Position, prefix: String, post: &Post) -> Insights {
    let post = Arc::clone(post);
    Arc::new(move |insight: Insight| {
        let action = match insight {
            Insight::Completions(items) => LspAction::IngestCompletions {
                items: phosphor_buffer::lsp::narrow(items, &prefix)
                    .into_iter()
                    .map(offered)
                    .collect(),
                at,
                buffer: None,
            },
            Insight::Signature(signature) => LspAction::IngestSignatureHelp {
                signature: Some(signed(*signature)),
                at,
                buffer: None,
            },
            Insight::Hover(prose) => LspAction::IngestHover {
                prose,
                at,
                buffer: None,
            },
            Insight::Nothing => match lookup {
                Lookup::Completion => LspAction::IngestCompletions {
                    items: Vec::new(),
                    at,
                    buffer: None,
                },
                Lookup::SignatureHelp => LspAction::IngestSignatureHelp {
                    signature: None,
                    at,
                    buffer: None,
                },
                Lookup::Hover => LspAction::IngestHover {
                    prose: Vec::new(),
                    at,
                    buffer: None,
                },
            },
        };
        post(Action::Lsp(action));
    })
}

/// The callback a `request-definition` answers through (`T036`).
///
/// **The first place, and the rest are dropped — say so rather than pretend.**
/// `LanguageServers::ask` answers a `Vec<FileSpan>` and the vocabulary has no
/// verb that carries a list of them; `request-references`, which is a list by
/// definition, was re-homed to `T047` for exactly that reason. A *definition*
/// is one place in every language this build blesses, so the first is the
/// answer rather than a truncation — and where a server does send several
/// (a trait method with implementations), the others are unreachable until
/// `T047` builds the surface a list is drawn in.
///
/// An empty answer posts nothing at all. There is nothing to open, and the
/// arrival of "no definition" is not a mutation — the caller sees the cursor
/// stay where it is, which is what every editor does.
fn jumping(post: &Post) -> phosphor_buffer::lsp::Locations {
    let post = Arc::clone(post);
    Arc::new(move |places: Vec<phosphor_core::request::FileSpan>| {
        let Some(place) = places.into_iter().next() else {
            return;
        };
        post(Action::File(FileAction::OpenFile {
            path: place.path,
            at: place.span.map(|span| span.start),
            pane: phosphor_core::request::PaneRef::Focused {},
        }));
    })
}

/// One completion, from the client's shape into the vocabulary's.
///
/// Two types for one thing, on purpose and in both directions: `phosphor-ui`'s
/// `CompletionItemVm` is the drawing half and this is the wire half, because a
/// widget crate may depend on `phosphor-core` and nothing else. This is the
/// binary doing the mapping, exactly as it already does for the statusline.
fn offered(item: phosphor_buffer::lsp::Completion) -> WireCompletion {
    WireCompletion {
        label: item.label,
        detail: item.detail,
        documentation: item.documentation,
        insert: item.insert,
        kind: item.kind,
        source: item.source,
        deprecated: item.deprecated,
    }
}

/// One signature, the same crossing.
///
/// The active-parameter range is characters into the label on both sides —
/// the UTF-16 conversion stopped at the LSP seam, which is `lsp.rs`'s whole
/// *"no surface above has to know that encoding exists"*.
fn signed(signature: phosphor_buffer::lsp::Signature) -> WireSignature {
    WireSignature {
        label: signature.label,
        active: signature.active.map(|(start, end)| SignatureRange {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
        }),
        documentation: signature.documentation,
    }
}

/// `T038`/`T039`'s float, composed when there is something to draw in it.
///
/// **This is the composition `scripts/lint-node-kinds.sh` recorded as owed.**
/// Both kinds were drawn by `phosphor_ui::interpret` and composed by nothing —
/// the lint's own header says it was written before `S4` precisely to catch
/// *"a kind drawn by the interpreter and composed by nobody"*, naming these
/// two.
///
/// The node carries **no props**: `view.rs` gives `Completion` and `Signature`
/// none, because *"there is one active completion session and the store holds
/// it — composition decides only where it goes"*. What is in the list arrives
/// through [`Painted`], which is the `Resources` door the interpreter reads.
///
/// **Completion outranks signature help when both are live.** They anchor to
/// the same cell and §9 allows one float; the list is the one the next
/// keystroke acts on, and a signature line hidden under it comes back when the
/// list closes because nothing here discards it.
fn passive_float(editing: &Editing) -> Option<Tree> {
    let body = if editing.completion.is_some() {
        Node::Completion {}
    } else if editing.signature.is_some() {
        Node::Signature {}
    } else {
        return None;
    };
    Some(Tree::new(Node::Empty {}).with_float(ViewFloat::new(Mood::Passive, body)))
}

/// What the host lends the interpreter for one frame (`T038`, `T039`, `T040`).
///
/// `Resources` is the seam that lets `phosphor-ui` draw a buffer and a
/// completion list without being able to reach the store or mutate anything:
/// there is no `&mut` in the trait and there must never be one. This is the
/// binary's implementation of it, and it is where `NoResources` stops being
/// enough — a passive float composed with no session behind it draws nothing,
/// which is the *"an absent thing answers empty"* rule and not a bug, but it is
/// also not a completion list.
#[derive(Clone, Copy)]
struct Painted<'a> {
    editor: &'a Editor,
    marks: &'a [StateMark],
    completion: Option<&'a CompletionVm>,
    signature: Option<&'a SignatureVm>,
}

impl std::fmt::Debug for Painted<'_> {
    /// The editor holds a rope, a tree-sitter tree and a highlight cache and
    /// implements no `Debug`; what is printable is what this frame is showing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painted")
            .field("marks", &self.marks.len())
            .field("completion", &self.completion.is_some())
            .field("signature", &self.signature.is_some())
            .finish_non_exhaustive()
    }
}

impl Resources for Painted<'_> {
    /// **One buffer, and it is implicit.** `T088` is what makes there be more
    /// than one and what makes a `BufferId` name anything; until then every id
    /// resolves to the buffer that is on screen, which is the honest answer to
    /// *"the editor behind this id"* in an editor with one.
    fn editor(&self, _buffer: phosphor_core::request::BufferId) -> Option<&Editor> {
        Some(self.editor)
    }

    fn state_marks(&self, _buffer: phosphor_core::request::BufferId) -> &[StateMark] {
        self.marks
    }

    fn completion(&self) -> Option<&CompletionVm> {
        self.completion
    }

    fn signature(&self) -> Option<&SignatureVm> {
        self.signature
    }
}

/// What rides over the buffer on this frame, and takes rows from it.
///
/// One struct rather than three parameters, so [`draw`] stays inside
/// `clippy::too_many_arguments` — and because they compose in one place: the
/// two strips come off the bottom of the body in `8e`'s order, and the ex line
/// and the notice take the statusline's row.
#[derive(Debug, Clone, Copy)]
struct Overlay<'a> {
    /// The ex line, or a notice, where the statusline goes.
    chrome: Option<Chrome<'a>>,
    /// `3c`'s which-key grid, for whatever prefix is half-typed. Empty when
    /// nothing is.
    leader: &'a [KeyHint],
    /// `8e`'s once-per-session unknown-key row, on the frame it was taught.
    hint: Option<&'a Node>,
    /// `T040`'s state column, already resolved through §3's ladder — one mark
    /// per visual row, computed **once** by the loop over every source of
    /// regions there is. See where it is built for why that is the host's job
    /// and not the gutter's.
    marks: &'a [StateMark],
    /// The live completion session and the live signature-help or hover answer
    /// (`T038`, `T039`), which [`Painted`] lends the interpreter. They ride
    /// here because they are the two things the frame needs that the buffer
    /// does not hold.
    completion: Option<&'a CompletionVm>,
    signature: Option<&'a SignatureVm>,
}

/// One frame: buffer, the strips over it, then the statusline.
///
/// The order is `8d`'s — [`FloatSlot::render`] dims what is behind it, so it
/// runs after the buffer and over the buffer's area only. The statusline never
/// dims: §9's dim means "behind", and chrome is not behind anything.
///
/// **The two strips take rows from the buffer rather than covering it**, which
/// is what `3c` and `8e` draw: the leader grid is a row slot above the
/// statusline and the hint is a one-row strip set off from the code. Neither is
/// a float — a float would impose a border, a header and a footer, and neither
/// drawing has any of the three.
fn draw(
    frame: &mut Frame<'_>,
    editor: &Editor,
    theme: &Theme,
    status: Option<&Tree>,
    floats: &FloatSlot<'_>,
    tree: Option<&Tree>,
    overlay: &Overlay<'_>,
) {
    let area = frame.area();
    let (mut body, status_area) = split(area);
    let painted = Painted {
        editor,
        marks: overlay.marks,
        completion: overlay.completion,
        signature: overlay.signature,
    };

    // A surface composed as a view tree owns the whole frame — `6b` draws its
    // own statusline, so the widgets below would be drawing it twice.
    if let Some(tree) = tree.filter(|tree| !matches!(tree.root, Node::Empty { .. })) {
        Interpreter::new(theme, &painted).render(tree, area, frame.buffer_mut());
        return;
    }

    // The strips, bottom-up: the leader grid sits directly above the
    // statusline, the hint between it and the code.
    let grid = (!overlay.leader.is_empty())
        .then(|| {
            let rows =
                KeyHints::new(overlay.leader, Density::Grid, theme).desired_height(body.width);
            take_rows(&mut body, rows)
        })
        .flatten();
    let hint_row = overlay
        .hint
        .and_then(|_| take_rows(&mut body, 1))
        .zip(overlay.hint);

    // The state column is empty on purpose: §3's marks are a store query
    // (`T041`, S5) and there is no store. The column is still reserved, which
    // is the half of the 3-column contract S1 can be held to.
    frame.render_widget(
        BufferView::new(editor, theme).state_column(overlay.marks),
        body,
    );
    if let Some((row, hint)) = hint_row {
        let strip = Tree::new(unknown_key::strip(
            hint.clone(),
            buffer_view::gutter_width(editor),
        ));
        Interpreter::new(theme, &NoResources).render(&strip, row, frame.buffer_mut());
    }
    if let Some(row) = grid {
        let strip = Tree::new(Node::KeyHints {
            density: Density::Grid,
            hints: overlay.leader.to_vec(),
        });
        Interpreter::new(theme, &NoResources).render(&strip, row, frame.buffer_mut());
    }
    // A tree with an empty root is a float over what the widgets painted —
    // `T021`'s boot report, `T097`'s help page, and `T038`'s completion list.
    if let Some(tree) = tree {
        Interpreter::new(theme, &painted).render(tree, body, frame.buffer_mut());
    }
    floats.render(body, frame.buffer_mut(), theme);
    let chrome = overlay.chrome;
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

/// What is bound one key past what has been typed — which-key's whole question
/// (`R17`, `3c`).
///
/// Empty when nothing is half-typed, which is what makes the popup appear on
/// `SPC` and vanish on the key after it: [`Machine`] clears
/// `Pending::keys` on every resolution that is not [`Resolution::Pending`].
///
/// **The prefix is asked in the machine's own spelling.** `key::notation_of` is
/// what `Layer::resolve` asks the layer with, and `keymap-entries` answers in
/// the same canonical notation, so `SPC c` and `<space>c` are one prefix here
/// for the same reason they are one binding there. The remainder is parsed
/// rather than counted, so `<C-w>` under a prefix is one key and not four.
fn under(layer: &mut Layer, machine: &Machine) -> Vec<KeyHint> {
    let typed = &machine.pending().keys;
    if typed.is_empty() {
        return Vec::new();
    }
    let prefix = key::notation_of(typed).0;
    let scope = Scope::of(machine.mode());
    layer
        .entries()
        .iter()
        .filter(|entry| entry.scope == scope.name())
        .filter(|entry| {
            entry
                .keys
                .0
                .strip_prefix(&prefix)
                .is_some_and(|rest| key::parse_seq(rest).is_some_and(|keys| keys.len() == 1))
        })
        .map(keymap::Entry::hint)
        .collect()
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
fn dirty_flag(editor: &mut Editor) -> (Rc<Cell<bool>>, Rc<Cell<u64>>) {
    let dirty = Rc::new(Cell::new(false));
    let edits = Rc::new(Cell::new(0));
    track_dirty(editor, &dirty, &edits);
    (dirty, edits)
}

/// Points the two counters at a different buffer, clean.
///
/// A new [`Editor`] carries no callback, so a swapped-in buffer would leave the
/// flag frozen at whatever the last one made it — `[+]` on a buffer nobody has
/// touched. `C-c buffer` is the one thing that swaps one today.
///
/// **`edits` is the second counter and it is `T038`'s** — *"how many times has
/// this buffer changed"*, which is what tells the loop to send a `didChange`.
/// It rides the same callback rather than a second one because there is one
/// callback slot on the fork's editor, and because the two questions have the
/// same answer source: dirty is *whether* the edit stream moved since the last
/// write, this is *how far* it has moved since the last thing that cared.
/// It is deliberately **not** reset when the buffer is swapped — the loop
/// compares it against its own last value, so a counter that restarted at zero
/// would read as *"nothing changed"* on the frame a new file opened.
fn track_dirty(editor: &mut Editor, dirty: &Rc<Cell<bool>>, edits: &Rc<Cell<u64>>) {
    dirty.set(false);
    let flag = Rc::clone(dirty);
    let count = Rc::clone(edits);
    editor.set_change_callback(Box::new(move |_| {
        flag.set(true);
        count.set(count.get().wrapping_add(1));
    }));
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

// ---------------------------------------------------------------------------
// `R2` — the undo tree, its journal, and the conversion between them
// ---------------------------------------------------------------------------

/// `T029`'s tree and `T030`'s journal, held together.
///
/// **The fork's history is not a fallback here, it is gone.** `T090` answered
/// `History::Undo` with `editor.apply(Undo)` and the fork's own stack, which
/// *"truncates on divergence"* (`vendor/ratatui-code-editor/src/history.rs:19-22`)
/// — undo one edit, type anything, and the undone edit is destroyed. That is
/// the exact failure `crates/phosphor-buffer/src/undo.rs:12-22` exists not to
/// have, and two live histories cannot both be the history. Nothing in this
/// binary reads the fork's stack any more; it still fills, because `Code::commit`
/// pushes to it and there is no public way to say no, but it is write-only and
/// the build proves it (the `Undo`/`Redo` imports are gone).
///
/// # The seam, and why the conversion is here
///
/// `phosphor-core` may not depend on `phosphor-buffer` — that crate carries the
/// vendored fork, `ropey` and `tree-sitter`, and taking it in the floor crate
/// would put all three in `phosphor-ui`'s graph
/// (`crates/phosphor-core/src/journal.rs:107-117`). So [`wire_undo::History`]
/// mirrors [`UndoTree`]'s nodes field for field and hands back exactly the
/// `(nodes, current, saved)` triple [`UndoTree::from_parts`] takes, and **the
/// copy lives in the binary**, which is the one crate holding both. That is
/// [`restored`] and [`journalled`] below, and they are the whole of it.
struct Timeline {
    tree: UndoTree,
    /// The on-disk log, when this buffer has a file to key one on. `None` for
    /// a scratch buffer and for a workspace with no state directory — a
    /// session that cannot persist still undoes.
    log: Option<Log<wire_undo::History>>,
}

impl std::fmt::Debug for Timeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timeline")
            .field("nodes", &self.tree.node_count())
            .field("current", &self.tree.current())
            .field(
                "journal",
                &self.log.as_ref().map(|log| log.journal().path()),
            )
            .finish()
    }
}

impl Timeline {
    /// A history with nowhere to write itself.
    fn detached() -> Self {
        Self {
            tree: UndoTree::new(),
            log: None,
        }
    }

    /// The history for `file`, restored from disk if a journal survived.
    ///
    /// A file that does not exist yet gets one too — see [`journal_key`].
    ///
    /// Every failure is soft and named: a state directory that cannot be made,
    /// a torn log, a hash collision on the file's own journal, a tree that
    /// fails [`UndoTree::from_parts`]'s four invariants. The editor opens with
    /// an empty history and a notice rather than refusing the file, because a
    /// history is not the file.
    fn opened(file: &Path) -> (Self, Option<String>) {
        match Self::open_at(file) {
            Ok(timeline) => (timeline, None),
            Err(reason) => (Self::detached(), Some(reason)),
        }
    }

    fn open_at(file: &Path) -> Result<Self, String> {
        let canonical = journal_key(file)?;
        // The workspace is the directory the editor was started in. `T071` is
        // what makes it the repository root; keying on the cwd is Q1's rule
        // ("keyed on the path and never on VCS identity") with the honest root
        // S3 has.
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let dir = journal::workspace_dir(&root).map_err(|error| error.to_string())?;
        let path = journal::undo_path(&dir, &canonical);
        // `Recovery` is discarded on purpose: `Log::open` has already truncated
        // the torn tail and the tree it folded is complete without it. What a
        // crash cost is one undo step, and there is nothing the editor could
        // do about it that reopening the file does not already do.
        let (log, _recovery) =
            Log::<wire_undo::History>::open(&path).map_err(|error| error.to_string())?;

        let origin = canonical.to_string_lossy().to_string();
        if log.state().origin().is_some_and(|owner| owner != origin) {
            return Err(format!(
                "{}: an undo journal for another file lives here",
                path.display()
            ));
        }
        let mut timeline = Self {
            tree: restored(log.state().clone())?,
            log: Some(log),
        };
        if timeline
            .log
            .as_ref()
            .is_some_and(|log| log.state().origin().is_none())
        {
            timeline.append(wire_undo::Record::Origin { path: origin });
        }
        // **The text on disk is at `saved`, not at `current`.** A session that
        // ended dirty left the tree ahead of the file, so the tree's cursor is
        // moved back to the node the file matches *without* applying the steps
        // — the text is already there. A tree that matches disk nowhere is not
        // a history of this file at all, and is dropped.
        let saved = timeline.tree.saved();
        match saved {
            Some(node) if node != timeline.tree.current() => {
                let _ = timeline.tree.goto(node);
                timeline.append(wire_undo::Record::Cursor { to: node.0 });
            }
            Some(_) => {}
            None => return Ok(Self::detached()),
        }
        Ok(timeline)
    }

    /// Gives a detached history somewhere to write itself, now that the buffer
    /// has a file (`T107`).
    ///
    /// Answers whatever the row has to say about it. A failure leaves the
    /// timeline exactly as it was — the caller has already written the text to
    /// disk, so a failure here costs the *history* and never the file. That is
    /// [`Timeline::opened`]'s rule (*"a history is not the file"*) at the other
    /// end of a buffer's life.
    ///
    /// # Why the tree is replayed rather than the log simply opened
    ///
    /// A scratch buffer arrives here with a tree the journal has never seen,
    /// and [`journal::undo::History`]'s fold requires node ids to be **dense
    /// and in creation order** (`FoldError::OutOfOrder`). Appending only what
    /// happens *after* the write would hand a fresh log a `Node { id: 7 }` as
    /// its first record, which the fold refuses and [`Timeline::append`] then
    /// answers by dropping the log — a history that silently stops persisting,
    /// which is the exact failure this whole subsystem is least able to notice.
    /// So the seed is the whole tree, in the same record sequence
    /// `History::snapshot` writes on a compaction, and everything typed into
    /// the scratch buffer survives the next open rather than only what follows
    /// the save.
    ///
    /// # A journal that is already there is replaced, and the row says so
    ///
    /// **The first version of this left it alone and that was a corruption**,
    /// found by the test that now pins the fix. `:write <path>` onto an
    /// existing file replaces its bytes wholesale, so the tree under that key
    /// describes text that is gone — and a tree that is merely *stale* is worse
    /// than one that is missing, because nothing downstream can tell. The
    /// measurement: a file holding `owned\n` with one saved edit, written over
    /// by a scratch buffer holding `new`, reopened, `u` — and the buffer became
    /// `ew`, because undo applied the inverse of an edit against text that no
    /// longer existed. [`Timeline::open_at`]'s own rule is the one being
    /// followed here (*"a tree that matches disk nowhere is not a history of
    /// this file at all, and is dropped"*); it only checks the case where
    /// `saved` is absent, and this is the case where `saved` is present and
    /// wrong.
    ///
    /// So the old journal goes and the row says it went, because a history
    /// disappearing silently is the half a person would want back. **A journal
    /// belonging to a *different* file still stops this**: that is Q1's
    /// collision guard, the same check and the same sentence
    /// [`Timeline::open_at`] uses, and deleting somebody else's history over a
    /// hash collision is not a thing a save may do.
    fn attach(&mut self, file: &Path) -> Option<String> {
        // **Said out loud rather than branched on.** This read
        // `if self.log.is_some() { return None; }`, which looked like a real
        // case and was none: [`Editing::write`] is the only caller and it calls
        // this behind `!named`, while `run` pairs a `Timeline::opened` log with
        // a file and `Timeline::detached`'s `None` with no file — so a buffer
        // reaching here has no journal by construction. The other direction
        // (`opened` failing and answering `detached()`) leaves the file set, so
        // `named` is true and this is not called at all.
        debug_assert!(
            self.log.is_none(),
            "a buffer with no file name had a journal open on it"
        );
        match self.attach_at(file) {
            Ok(note) => note,
            Err(reason) => Some(reason),
        }
    }

    fn attach_at(&mut self, file: &Path) -> Result<Option<String>, String> {
        let canonical = journal_key(file)?;
        let origin = canonical.to_string_lossy().to_string();
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let dir = journal::workspace_dir(&root).map_err(|error| error.to_string())?;
        let path = journal::undo_path(&dir, &canonical);
        let (log, _recovery) =
            Log::<wire_undo::History>::open(&path).map_err(|error| error.to_string())?;
        if log.state().origin().is_some_and(|owner| owner != origin) {
            return Err(format!(
                "{}: an undo journal for another file lives here",
                path.display()
            ));
        }
        // `nodes()` is never empty — index `0` is the root — so *more than the
        // root* is what "this key already holds a history" means. `origin` is
        // checked too, because a journal whose only record is its own origin is
        // still one this session did not write.
        let occupied = log.state().nodes().len() > 1 || log.state().origin().is_some();
        drop(log);
        if occupied {
            std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        }
        let (mut log, _recovery) =
            Log::<wire_undo::History>::open(&path).map_err(|error| error.to_string())?;
        for record in seeding(&self.tree, origin) {
            log.append(record).map_err(|error| error.to_string())?;
        }
        self.log = Some(log);
        // **No path in front of it**, unlike every other notice this file
        // composes. §11's rule is that the statusline truncates from the right
        // and never wraps, and the path here is one the user typed a keystroke
        // ago — leading with it costs exactly the half of the sentence that
        // says what happened. Measured: at 120 columns a path-prefixed version
        // of this row was shed mid-clause, leaving a fact about the file where
        // the consequence should have been.
        Ok(occupied.then(|| {
            "undo history replaced — the one here described the text this write overwrote"
                .to_owned()
        }))
    }

    /// Appends a record, and drops the log if the write fails.
    ///
    /// A journal that cannot be written is not a reason to stop editing, and it
    /// is a reason to stop pretending: the in-memory tree keeps working and
    /// nothing tries the same failing write once per keystroke.
    fn append(&mut self, record: wire_undo::Record) {
        let failed = self
            .log
            .as_mut()
            .is_some_and(|log| log.append(record).is_err());
        if failed {
            self.log = None;
        }
    }

    /// Closes the open group, if there is one, and writes the node it became.
    ///
    /// **The group boundary is the machine's** — `History::CommitUndoGroup`,
    /// emitted at exactly the three places vim closes one — so this is called
    /// from that Action's arm and from the two places that must not walk a tree
    /// with a half-typed insert in it (`undo`, `redo`).
    fn close(&mut self, after: Caret) {
        let Some(id) = self.tree.commit(after) else {
            return;
        };
        let Some(change) = self
            .tree
            .node(id)
            .and_then(|node| node.change.as_ref())
            .cloned()
        else {
            return;
        };
        let parent = self
            .tree
            .node(id)
            .and_then(|node| node.parent)
            .unwrap_or(NodeId::ROOT);
        self.append(journalled(id, parent, &change));
    }
}

/// A committed node as the record that reproduces it.
fn journalled(
    id: NodeId,
    parent: NodeId,
    change: &phosphor_buffer::undo::Change,
) -> wire_undo::Record {
    wire_undo::Record::Node {
        id: id.0,
        parent: parent.0,
        edits: change
            .edits
            .iter()
            .map(|edit| wire_undo::Edit {
                at: edit.at,
                removed: edit.removed.clone(),
                inserted: edit.inserted.clone(),
            })
            .collect(),
        before: caret_out(change.before),
        after: caret_out(change.after),
    }
}

/// A whole live tree as the records that reproduce it (`T107`).
///
/// **The order is `journal::undo::History::snapshot`'s and that is not a
/// coincidence** — it is the one sequence the fold is known to accept, because
/// it is what every compaction already writes and what every reopen already
/// reads. Two things in it are easy to get wrong from first principles and are
/// therefore copied rather than re-derived:
///
/// * **Replaying the nodes leaves every branch point pointing at its newest
///   child**, because the fold sets `redo_child` as each node arrives. So a
///   `Redo` record is needed exactly where the live tree's `redo_child` is not
///   its last child, and nowhere else.
/// * **The trailing `Cursor` re-points the path** from the newest leaf back to
///   where the buffer actually is. Without it a scratch buffer that was undone
///   before it was saved would reopen at the wrong node.
///
/// No `Saved` record: [`Editing::write`] appends one the moment this returns,
/// and it is the whole reason the journal is opened at that instant.
fn seeding(tree: &UndoTree, origin: String) -> Vec<wire_undo::Record> {
    let id_of = |index: usize| NodeId(u64::try_from(index).unwrap_or(u64::MAX));
    let mut out = vec![wire_undo::Record::Origin { path: origin }];
    for (index, node) in tree.nodes().iter().enumerate().skip(1) {
        // Only the root has no parent and no change, and `skip(1)` is past it.
        // A `continue` here would leave a gap in the ids and the fold would
        // refuse the next record, so this is unreachable rather than lenient.
        let (Some(parent), Some(change)) = (node.parent, node.change.as_ref()) else {
            continue;
        };
        out.push(journalled(id_of(index), parent, change));
    }
    for (index, node) in tree.nodes().iter().enumerate() {
        if let Some(child) = node.redo_child
            && node.children.last() != Some(&child)
        {
            out.push(wire_undo::Record::Redo {
                node: id_of(index).0,
                child: child.0,
            });
        }
    }
    out.push(wire_undo::Record::Cursor {
        to: tree.current().0,
    });
    out
}

fn caret_out(caret: Caret) -> wire_undo::Caret {
    wire_undo::Caret {
        offset: caret.offset,
        selection: caret.selection.map(|range| wire_undo::CharRange {
            start: range.start,
            end: range.end,
        }),
    }
}

fn caret_in(caret: wire_undo::Caret) -> Caret {
    Caret {
        offset: caret.offset,
        selection: caret
            .selection
            .map(|range| CharRange::new(range.start, range.end)),
    }
}

/// The journal's folded state as a live tree — the field copy the seam names.
fn restored(history: wire_undo::History) -> Result<UndoTree, String> {
    let (nodes, current, saved) = history.into_parts();
    let nodes = nodes
        .into_iter()
        .map(|node| phosphor_buffer::undo::Node {
            parent: node.parent.map(NodeId),
            children: Vec::new(),
            redo_child: node.redo_child.map(NodeId),
            change: node.change.map(|change| phosphor_buffer::undo::Change {
                edits: change
                    .edits
                    .into_iter()
                    .map(|edit| TreeEdit {
                        at: edit.at,
                        removed: edit.removed,
                        inserted: edit.inserted,
                    })
                    .collect(),
                before: caret_in(change.before),
                after: caret_in(change.after),
            }),
        })
        .collect();
    UndoTree::from_parts(nodes, NodeId(current), saved.map(NodeId))
        .map_err(|error| error.to_string())
}

/// The absolute path an undo journal is keyed on — for a file that may not
/// exist yet.
///
/// `std::fs::canonicalize` is the right key and it needs the file to be there:
/// it resolves symlinks, so a journal follows the file rather than the name
/// somebody reached it through. A **new** buffer has nothing to resolve, and
/// before `CP-4` that did not arise because a path with nothing behind it never
/// got as far as a journal.
///
/// So a missing file is keyed on its *directory* resolved, plus the name as
/// typed. That is the same string `canonicalize` will answer the moment the
/// buffer is written — which is the whole requirement: type, `:w`, quit,
/// reopen, and `u` walks the history it just wrote, because both sessions
/// hashed the same path. Getting this wrong is not a crash; it is a silently
/// orphaned journal, which is the failure this build's `T030` work is least
/// able to notice.
///
/// A directory that does not resolve is still an error, and it reaches the
/// statusline as a notice rather than refusing the buffer ([`Timeline::opened`]
/// — *"a history is not the file"*).
fn journal_key(file: &Path) -> Result<PathBuf, String> {
    match std::fs::canonicalize(file) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let name = file
                .file_name()
                .ok_or_else(|| format!("{}: no file name to key a history on", file.display()))?;
            let directory =
                std::fs::canonicalize(holding(file)).map_err(|error| error.to_string())?;
            Ok(directory.join(name))
        }
        Err(error) => Err(error.to_string()),
    }
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
    /// Where in it the cursor goes — `gd`'s whole point, and `None` for the
    /// `:edit <path>` that has no opinion.
    open_at: Option<Position>,
    /// A prompt `open-prompt` asked for, drained the same way.
    prompt: Option<PromptKind>,
    /// A `:help` `open-help` asked for (`T097`), drained the same way — the
    /// page is composed from the live keymap and the keymap is the layer's.
    help: Option<Help>,
    /// What the last key's Actions were refused with, if they were.
    ///
    /// **`T098`'s enabling half.** A refusal from the ex line has always been
    /// said out loud ([`submit_ex`]); a refusal from a *key* was dropped on the
    /// floor, so a key bound to a capability the binary does not apply yet was
    /// indistinguishable from a key bound to nothing. Recorded here and drained
    /// into the notice by the loop, which is where "what the last thing you did
    /// answered" already lives.
    refused: Option<Refusal>,
    /// Something true the last Action could not say through its [`Outcome`].
    ///
    /// **One writer, and `T107` is why it exists**: `:write <path>` at a buffer
    /// with no file *succeeds* and may still fail to give that buffer's history
    /// somewhere to live ([`Timeline::attach`]). A [`Refusal`] would be a lie —
    /// the text is on disk — and silence would be the failure mode `T030`'s
    /// journal is least able to notice, so the surprising half rides the notice
    /// row beside [`Editing::refused`], the way [`Timeline::opened`]'s already
    /// does at startup.
    note: Option<String>,
    /// A key nothing was bound to (`T035`). Drained by the loop, because
    /// *"once per session, never again"* is session state and this struct is
    /// per buffer.
    unknown: Option<KeySeq>,
    /// This buffer's language, as the `Languages` table named it — `None` for
    /// a file no declaration claims, which is second tier and a normal state.
    ///
    /// Set when the buffer opens, not looked up per keystroke: a redeclaration
    /// at `:repl` changes what the *next* file opens as, and re-deriving it
    /// under the cursor would change what `gc` inserts halfway through a
    /// session.
    language: Option<LanguageId>,
    /// What this language's server is *called*, off its declaration's
    /// `lsp_command` — `rust-analyzer`. [`None`] where it declares none.
    ///
    /// Kept beside the language because the statusline needs a name for a
    /// server that has not answered `initialize` yet: `ServerState::Ready`
    /// carries the server's own `serverInfo.name` and the other four states
    /// carry no name at all, so a chip that only knew the ready one could not
    /// say *which* server failed to start.
    server: Option<String>,
    /// The line-comment prefix `gc` uses, off that language's declaration
    /// (`T037`). `None` where the declaration named none — CSV and JSON have
    /// no comment syntax, and `gc` in one must do nothing rather than corrupt
    /// the file.
    comment_prefix: Option<String>,
    /// What one indent level is, for `>`, `<` and `<tab>` (`T104`).
    ///
    /// **Refreshed by the loop on every pass, not set when the buffer opens**,
    /// and the two neighbours above show why the difference matters:
    /// [`Editing::comment_prefix`] is deliberately fixed at open, because
    /// re-deriving it under the cursor would change what `gc` inserts halfway
    /// through a session. An indent unit is the opposite — it comes from
    /// `set-option!` as well as from the declaration, and `(set-option!
    /// "tab-width" 8)` typed at the REPL has to reach the very next `>>`. So
    /// this field is a per-pass copy of [`indent_style`]'s answer and never a
    /// snapshot of it.
    indent_style: IndentStyle,
    /// `T038`/`T039`'s ask: which *"tell me about this place"* request the last
    /// key made. Drained by the loop, which is the only thing holding the
    /// servers.
    lookup: Option<Lookup>,
    /// `T036`'s `restart-language-server`, drained the same way.
    restart: Option<LanguageId>,
    /// `T036`'s `request-definition`. Same drain, different client call: a
    /// question answers in *places* rather than in text about a place.
    question: Option<Question>,
    /// The live completion session (`T038`), or `None` when the float is
    /// closed. **This is what `Resources::completion` lends the interpreter**;
    /// the tree says whether the list is on screen and this says what is in it.
    completion: Option<CompletionVm>,
    /// What each row of [`Editing::completion`] does when it is chosen, in the
    /// same order.
    ///
    /// Beside the ViewModel rather than in it, because a `CompletionVm` is what
    /// a *widget* needs and a widget has no business knowing what a row would
    /// write into the buffer — `phosphor-ui` may not construct an Action and
    /// may not mutate. The two are built and dropped together, in
    /// `IngestCompletions`' arm and in [`Editing::accept`].
    offered: Vec<Offer>,
    /// Whether the user has **chosen** a row in this session, rather than
    /// merely been offered one — set by `move-completion` and by nothing else.
    ///
    /// This is the state `accept-completion`'s `otherwise` reads, and it is the
    /// whole reason that argument can exist: **a keymap is data**. A binding
    /// names a capability and its arguments and cannot ask whether a row is
    /// selected, so the guard that makes `<space>` usable — accept only if the
    /// user steered, otherwise type a space — has to be the host's. The
    /// keymap's `<C-x>` note argues the same constraint for the *opening* key.
    ///
    /// **Three writers, and they are one setter and the two ways a session
    /// ends** — counted by `grep`, because the first version of this comment
    /// said *"cleared in [`Editing::close_completion`] and nowhere else"*, the
    /// `MoveCompletion` arm said *"the one writer"*, the `IngestCompletions`
    /// arm said *"the two are the only writers"*, and the three were an
    /// invariant claim nothing checked:
    ///
    /// * `Lsp::MoveCompletion`'s arm sets it — pressing `<C-n>` is the whole of
    ///   what *"the user chose a row"* means.
    /// * [`Editing::close_completion`] clears it, and is the one place a
    ///   session is **dropped** (`esc`, an accept, leaving insert mode).
    /// * `Lsp::IngestCompletions`' arm clears it, and is the one place a
    ///   session is **replaced** — a fresh answer puts the selection back on
    ///   row 0, so the row the user steered to no longer exists.
    ///
    /// That is the invariant worth stating, and it is what makes the flag safe
    /// rather than the count: **it can only be true inside a session the user
    /// steered in**, because dropping a session and replacing one are the only
    /// two exits and both clear it.
    chosen: bool,
    /// Whether a keystroke's `otherwise` capability is running right now.
    ///
    /// **A depth stop, and one level is all any binding needs.** A fall-through
    /// runs an Action, and that Action may be another `move-completion` with an
    /// `otherwise` of its own — the data is finite so it terminates, but the
    /// depth is whatever a `runtime/keymaps.scm` wrote and the recursion is on
    /// the stack. One level is the whole of what a key means by *"and if there
    /// is no list, do the ordinary thing"*; a second level is a keymap asking
    /// for something this argument does not offer, and it gets a sentence
    /// rather than a stack.
    falling_through: bool,
    /// The live signature-help or hover answer (`T039`). One field for two
    /// features because they are one surface — see `float::SignatureVm`.
    signature: Option<SignatureVm>,
    /// `T040`'s set, shared with [`AppHost`] so the gutter and the
    /// `diagnostics` query cannot disagree.
    diagnostics: Arc<lsp::Diagnostics>,
    /// The unnamed register is `"`; `"a` is `a` (`request::RegisterName`).
    registers: BTreeMap<String, Register>,
    /// What the last `SelectRange` said, so a yank knows whether it is linewise.
    selection_kind: SelectionKind,
    /// The offset of the character a live selection is anchored at — the end
    /// that does *not* move. `None` when nothing is selected.
    ///
    /// **`ExtendSelection` cannot be applied without it, and shipping without
    /// it made the highlight lie.** See the Action's arm.
    selection_from: Option<usize>,
    /// `T029`'s tree and `T030`'s journal.
    timeline: Timeline,
    /// How many nested [`Editing::begin`] calls are open. An Action that edits
    /// more than once — `Replace`, `J`, `>` over a range — is one edit as far
    /// as the fork's batch and the undo tree are concerned.
    depth: u32,
    dirty: Rc<Cell<bool>>,
    /// Set by `App::Quit`; the loop reads it once per turn.
    quit: bool,
    /// What mode the machine says it is in — **the machine's report, kept so
    /// that a host-side edit can type the way a keystroke would**.
    ///
    /// One writer, and it is the `Input::SetMode` arm of [`Session::key`]:
    /// `Action::Input` is the one family that never reaches [`Editing::act`]
    /// (the loop answers it, so the machine and the host stay in step through
    /// one path), and `Machine::set_mode` emits exactly this Action on every
    /// change. Read by [`Editing::accept`] and by nothing else.
    ///
    /// **`CP-4`'s `R` defect is why it exists.** `Scope::of` folds `Replace`
    /// into the insert scope — vim's `:imap` does the same — so binding
    /// `<space>` and `<cr>` there bound them in `R` too, where no completion
    /// float can ever be open and the fall-through therefore always fires. It
    /// spliced text in, so `R` stopped overwriting: `abcdef` with `RXY Z` read
    /// `XY Zdef` instead of vim's `XY Zef`.
    mode: EditMode,
}

/// The unnamed register, as vim spells it.
const UNNAMED: &str = "\"";

/// One completion row's two answers: what it writes, and what it says about
/// itself (`T038`).
///
/// The half of `request::Completion` that is not a label — see
/// [`Editing::offered`] for why it does not ride in the ViewModel.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Offer {
    /// What to type when this row is accepted.
    insert: String,
    /// This row's documentation, one line per row (§11: nothing wraps).
    documentation: Vec<String>,
}

/// Where `move-completion` lands, **wrapping** at both ends.
///
/// Vim's own `<C-n>` wraps and so does every completion menu a user has met;
/// stopping at the last row would make `<C-p>` on the first row do nothing,
/// which reads as a stuck key. An empty list has no rows to land on and stays
/// at zero — the list is not on screen in that state, because an empty ingest
/// closes the float.
fn moved(selected: usize, rows: usize, delta: i64) -> usize {
    let Ok(rows) = i64::try_from(rows) else {
        return selected;
    };
    if rows == 0 {
        return 0;
    }
    let at = i64::try_from(selected).unwrap_or(0);
    usize::try_from((at + delta).rem_euclid(rows)).unwrap_or(0)
}

impl std::fmt::Debug for Editing {
    /// The editor holds a rope, a tree-sitter tree and a highlight cache, and
    /// implements no `Debug`; what is worth printing is the state this file
    /// owns.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editing")
            .field("area", &self.area)
            .field("registers", &self.registers)
            .field("selection_kind", &self.selection_kind)
            .field("timeline", &self.timeline)
            .field("quit", &self.quit)
            .finish_non_exhaustive()
    }
}

impl Editing {
    /// A buffer with a history that goes nowhere. Test-only: the loop always
    /// has a file to key a journal on, or knows it has none
    /// ([`Timeline::detached`]), and a second constructor on the shipping path
    /// would be a second answer to *"where does undo go"*.
    #[cfg(test)]
    fn new(editor: Editor, file: Option<PathBuf>, dirty: Rc<Cell<bool>>) -> Self {
        Self::with_timeline(
            editor,
            file,
            dirty,
            Timeline::detached(),
            Arc::new(lsp::Diagnostics::default()),
        )
    }

    fn with_timeline(
        editor: Editor,
        file: Option<PathBuf>,
        dirty: Rc<Cell<bool>>,
        timeline: Timeline,
        diagnostics: Arc<lsp::Diagnostics>,
    ) -> Self {
        Self {
            editor,
            area: Rect::ZERO,
            file,
            open: None,
            open_at: None,
            prompt: None,
            help: None,
            refused: None,
            note: None,
            unknown: None,
            language: None,
            server: None,
            comment_prefix: None,
            // The shipped defaults, which the loop overwrites before the first
            // key is handled. Not an `Option`: `>` with nothing set has to mean
            // something, and *four spaces* is the answer this build documents.
            indent_style: IndentStyle {
                unit: " ".repeat(TAB_WIDTH_DEFAULT),
                tab_width: TAB_WIDTH_DEFAULT,
            },
            lookup: None,
            restart: None,
            question: None,
            completion: None,
            offered: Vec::new(),
            chosen: false,
            falling_through: false,
            signature: None,
            diagnostics,
            registers: BTreeMap::new(),
            selection_kind: SelectionKind::Char,
            selection_from: None,
            timeline,
            depth: 0,
            dirty,
            quit: false,
            mode: EditMode::Normal,
        }
    }

    /// The whole rope, as a server and a `:write` both want it.
    fn contents(&self) -> String {
        let code = self.editor.code_ref();
        code.slice(0, code.len_chars())
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
            Action::Buffer(BufferAction::InsertIndent {}) => {
                self.insert_indent();
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
                self.selection_kind = *kind;
                let (from, to) = self.selected(from, to);
                self.editor.set_selection(Some(Selection::new(from, to)));
                // **The anchor is kept only while it is still inside the range
                // it anchors.** `v` puts it under the cursor; every
                // `SelectRange` after it — `G` and the finds go through `jump`,
                // a drag re-measures from the press — names a range with the
                // *same* fixed end and a new moving one, so recomputing then
                // would let the fixed end follow the pointer.
                //
                // It used to be `get_or_insert`, cleared only by
                // `ClearSelection` — and `Machine::select` emits one when it
                // *leaves* visual mode, never when it enters. That is sound for
                // the machine's own stream and wrong for the other three doors:
                // `select-range` is a declared capability (`T026`), so Steel,
                // MCP or `--do` can set a selection, and the next `v` inherited
                // that anchor and extended from it. Measured: a scripted
                // selection of cols 1–4, a `SetCursor` to 7, then `v` and one
                // `ExtendSelection` gave `0..8` where `6..8` is right.
                //
                // Containment is the invariant rather than a fourth clearing
                // site, because a clearing site is a thing to remember at each
                // new door: an anchor outside the range it is the anchor *of* is
                // not that range's anchor, whoever sent it.
                let anchored = self
                    .selection_from
                    .is_some_and(|anchor| (from..to).contains(&anchor));
                if !anchored {
                    let cursor = self.editor.get_cursor();
                    self.selection_from = Some(if cursor <= from {
                        from
                    } else {
                        to.saturating_sub(1)
                    });
                }
                done()
            }
            // **The highlight and the operand were one character apart, and
            // the highlight was the one lying** (`CP-4`, measured on the
            // shipping binary: `v l l` drew `ab` and `d` deleted `abc`).
            //
            // `span_between` is inclusive of the character under the cursor —
            // *"which is what visual mode means by selected"*, in its own words
            // — so every `SelectRange` covers it and every operator takes it.
            // This arm called `Editor::extend_selection`, whose `Selection` is
            // half-open and whose anchor is *"the end the cursor is not at"* —
            // and `SelectRange` never moved the cursor, so that answer was
            // wrong as well as short. Both halves are fixed by knowing the
            // anchor ([`Editing::selection_from`]) and re-stating the same
            // inclusive rule: `[min, max + 1)`.
            Action::Motion(MotionAction::ExtendSelection { motion, count }) => {
                let to = motion::cursor_after(&self.text(), self.text().cursor(), *motion, *count);
                let head = self.offset(to);
                let anchor = *self.selection_from.get_or_insert(head);
                self.editor.set_cursor(head);
                let last = anchor.max(head);
                let end = last
                    .saturating_add(1)
                    .min(self.editor.code_ref().len_chars());
                let (from, to) = self.selected(anchor.min(head), end);
                self.editor.set_selection(Some(Selection::new(from, to)));
                done()
            }
            Action::Motion(MotionAction::ClearSelection {}) => {
                self.editor.clear_selection();
                self.selection_from = None;
                done()
            }
            // The record of what was *asked for*. What it covers arrives as the
            // `SelectRange` behind it, when this side can resolve it at all —
            // the four agent nouns cannot until `T049`.
            Action::Motion(MotionAction::SelectObject { .. }) => done(),
            Action::Buffer(BufferAction::SetCase { target, case }) => {
                self.set_case(target, *case);
                done()
            }
            // **No conversion.** `R7-ScrollRequest` closed in the repair window
            // between `CP-3` and `S4`: `buffer_view::ScrollRequest` is a
            // re-export of this very type now, so the vocabulary's request goes
            // straight to the widget and the 1-based-to-0-based arithmetic lives
            // once, in `Viewport::scrolled`.
            Action::View(ViewAction::Scroll { request, .. }) => {
                buffer_view::apply_scroll(&mut self.editor, *request, self.area);
                done()
            }
            // `R19` — folds. `T016`'s whitespace half shipped with `8e`; this is
            // the half that never had a call site, and the machinery is the
            // fork's (`code.rs`'s `fold_query` / `fold_ranges`, read out of
            // `langs/<lang>/folds.scm`).
            Action::View(ViewAction::SetFold { target, state }) => {
                if self.set_fold(target, *state) {
                    done()
                } else {
                    declined("no fold here — the language's folds.scm names none at the cursor")
                }
            }
            Action::View(ViewAction::FoldAll { level }) => {
                self.fold_all(*level);
                done()
            }
            Action::View(ViewAction::UnfoldAll {}) => {
                self.unfold_all();
                done()
            }
            // `R2` — `T029`'s tree, on the shipping path. The count is one
            // argument rather than a loop: `UndoTree::undo` walks `count` nodes
            // towards the root and hands back the steps that make the text
            // agree, which is one route rather than `count` of them.
            Action::History(HistoryAction::Undo { count }) => {
                let after = self.caret();
                self.timeline.close(after);
                let steps = self.timeline.tree.undo((*count).max(1));
                self.walk(&steps);
                done()
            }
            Action::History(HistoryAction::Redo { count }) => {
                let after = self.caret();
                self.timeline.close(after);
                let steps = self.timeline.tree.redo((*count).max(1));
                self.walk(&steps);
                done()
            }
            // The group boundary the machine marks, and now the only thing that
            // closes one. `input.rs` emits it at exactly the three places vim
            // closes a group — leaving insert or replace mode, finishing a
            // non-`c` operator, finishing a paste — so an insert session is one
            // `u` rather than one per character.
            Action::History(HistoryAction::CommitUndoGroup {}) => {
                let after = self.caret();
                self.timeline.close(after);
                done()
            }
            // `T035`'s hint. Recorded rather than shown: *"once per session,
            // never again"* is the loop's latch, not this buffer's.
            Action::App(AppAction::ShowUnknownKeyHint { key }) => {
                self.unknown = Some(key.clone());
                done()
            }
            // `T097` — the arm `T086` could not pass without. Recorded, not
            // composed: the grid is read off the *live* keymap and the keymap
            // is behind the Steel barrier, which this struct is on the wrong
            // side of.
            Action::App(AppAction::OpenHelp { topic }) => {
                self.help = Some(topic.clone().map_or(Help::Index, Help::Topic));
                done()
            }
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
            // **`at` is honoured now, and that is `T036`'s doing.** The arm
            // recorded the path and dropped the position, which nothing had
            // noticed because every caller so far was `:edit <path>` — and
            // `gd` is the first one that means *this line of it*.
            Action::File(FileAction::OpenFile { path, at, .. }) => {
                self.open = Some(path.clone());
                self.open_at = *at;
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
            // `T037`'s locale hook, and the reason a language declaration
            // carries a `comment_prefix` at all. The decision half is
            // `phosphor_core::language::toggle_comment` — a pure function over
            // lines — and this is the applying half, which has no branches:
            // one batch, so `gcip` is one `u`.
            Action::Buffer(BufferAction::ToggleComment { target }) => {
                match self.comment_prefix.clone() {
                    Some(prefix) => {
                        self.toggle_comment(target, &prefix);
                        done()
                    }
                    // Not `NotYetImplemented`: the capability is built and the
                    // language genuinely has no line comment. CSV's own
                    // declaration says so, and a `#` line there would be a data
                    // row with one field.
                    None => declined(
                        "this language declares no line comment — \
                         `comment_prefix` in its define-language",
                    ),
                }
            }
            // `T038`, `T039` — the three asks. Recorded rather than sent: the
            // servers are the loop's, because a reply arrives on another thread
            // and has to reach the queue rather than this struct.
            Action::Lsp(LspAction::RequestCompletion {}) => {
                self.lookup = Some(Lookup::Completion);
                done()
            }
            Action::Lsp(LspAction::RequestSignatureHelp {}) => {
                self.lookup = Some(Lookup::SignatureHelp);
                done()
            }
            Action::Lsp(LspAction::RequestHover {}) => {
                self.lookup = Some(Lookup::Hover);
                done()
            }
            // The three verbs that drive the float once it is up. Each refuses
            // with a sentence rather than doing nothing when there is no
            // session, because `<C-n>` with no float open is a key that must
            // say why (`T098`'s rule, applied to a surface instead of a task).
            Action::Lsp(LspAction::MoveCompletion { delta, otherwise }) => {
                let offered = &self.offered;
                // `stepped` and not `moved`: the free function [`moved`] is
                // called inside this binding's own initializer, and it resolves
                // there only because a `let` is not in scope until after it.
                // That compiles and is correct, and it is one hoist away from
                // silently meaning something else — `CP-4`'s review asked for
                // the shadow to go rather than for a comment about it.
                let stepped = match &mut self.completion {
                    Some(session) => {
                        session.selected = moved(session.selected, session.items.len(), *delta);
                        // The prose under the rule follows the selection, with
                        // no second request: `request::Completion` carries
                        // documentation **per item** for exactly this, and a
                        // list that kept one block would show the first item's
                        // prose under every other row.
                        session.documentation = offered
                            .get(session.selected)
                            .map(|offer| offer.documentation.clone())
                            .unwrap_or_default();
                        true
                    }
                    None => false,
                };
                // **The only writer of [`Editing::chosen`] that sets it** — the
                // other two clear it, and the field's own doc counts all three.
                // Pressing `<C-n>` is the whole of what *"the user chose a
                // row"* means, and it is what `<space>`'s `otherwise` consults
                // before it types instead of accepting. Outside the match
                // because the session is borrowed inside it.
                self.chosen |= stepped;
                if stepped {
                    return done();
                }
                // **The fall-through, and it is why `<tab>` can mean both
                // things.** With no list open this key is not a completion key
                // at all, so it does what it would have done — `insert-indent`
                // for `<tab>` — and the *condition* stays here where the state
                // is while the *alternative* stays in the keymap where the
                // key's meaning is. That is the same split `accept-completion`
                // already uses for `<space>` and `<cr>`; the only difference is
                // that this one names a capability instead of literal text,
                // because an indent level is a per-language value no keymap can
                // spell (`OPEN-QUESTIONS.md` §38).
                let Some(binding) = otherwise else {
                    return declined("no completion list is open");
                };
                self.fall_through(binding)
            }
            Action::Lsp(LspAction::AcceptCompletion {
                index,
                then,
                otherwise,
            }) => match self.accept(*index, then.as_deref(), otherwise.as_deref()) {
                Ok(()) => done(),
                Err(reason) => declined(&reason),
            },
            Action::Lsp(LspAction::CancelCompletion {}) => {
                // Both, and deliberately: `esc` closes what is on screen, and
                // §9 says it closes top-down rather than one surface per press.
                self.close_completion();
                self.signature = None;
                done()
            }
            // The answers. Each carries the cursor its request was made at, so
            // an answer the cursor has left is **dropped rather than drawn in
            // the wrong place** — that is the declaration's own wording and the
            // whole reason `at` is on the wire.
            Action::Lsp(LspAction::IngestCompletions { items, at, .. }) => {
                if self.text().cursor() == *at {
                    // An **empty list closes the float**, which the declaration
                    // says out loud: the client answers exactly once on every
                    // path, so `Insight::Nothing` arrives here as an empty list
                    // and a float that suppressed it would leave a stale list
                    // beside the cursor forever.
                    let next = (!items.is_empty()).then(|| self.completions(items));
                    self.completion = next.map(|mut vm| {
                        // The session's identity is the **word** it is
                        // completing, and the anchor is that word's first cell:
                        // it does not move while you type into the word, and it
                        // moves the moment you start a different one. So this
                        // carries the width across a keystroke and resets it
                        // across a word, which is what "for the life of the
                        // session" means for this surface.
                        vm.width_floor = self
                            .completion
                            .as_ref()
                            .filter(|held| held.anchor == vm.anchor)
                            .map_or(0, |held| {
                                held.width_floor
                                    .max(CompletionList::new(held).desired_width())
                            });
                        vm
                    });
                    self.offered = items
                        .iter()
                        .map(|item| Offer {
                            insert: item.insert.clone(),
                            documentation: item.documentation.clone(),
                        })
                        .collect();
                    // A fresh answer is a fresh session: `Editing::completions`
                    // puts the selection back on row 0, so the row the user
                    // had steered to no longer exists and `<space>` must go
                    // back to typing a space. **Replacing a session**, where
                    // [`Editing::close_completion`] is **dropping** one — the
                    // two exits, and the field's doc counts them beside the one
                    // writer that sets it.
                    self.chosen = false;
                }
                done()
            }
            Action::Lsp(LspAction::IngestSignatureHelp { signature, at, .. }) => {
                if self.text().cursor() == *at {
                    let next = signature.as_ref().map(|signature| SignatureVm {
                        label: Some(signature.label.clone()),
                        active: signature
                            .active
                            .map(|range| (range.start as usize, range.end as usize)),
                        // §11 is "nothing ever wraps", so the wrapping is here
                        // and the width is the float's own — see `wrapped`.
                        prose: self.wrapped(&signature.documentation),
                        anchor: self.anchor(0),
                        width_floor: 0,
                    });
                    self.signature = next.map(|vm| self.held_to_widest(vm));
                }
                done()
            }
            Action::Lsp(LspAction::IngestHover { prose, at, .. }) => {
                if self.text().cursor() == *at {
                    let next = (!prose.is_empty()).then(|| SignatureVm {
                        // Hover has no callable to name; the whole answer is
                        // prose. `SignatureVm` is one type for both features
                        // and this is the difference between them.
                        label: None,
                        active: None,
                        prose: self.wrapped(prose),
                        anchor: self.anchor(0),
                        width_floor: 0,
                    });
                    self.signature = next.map(|vm| self.held_to_widest(vm));
                }
                done()
            }
            // `T040`. Unsolicited by construction — this is the arm the event
            // queue was built ahead of, and the only `Lsp` verb a producer is
            // allowed to reach (`deliver`).
            Action::Lsp(LspAction::IngestDiagnostics { path, diagnostics }) => {
                self.diagnostics.replace(path.clone(), diagnostics.clone());
                done()
            }
            // `T036` — `gd`. Recorded like the lookups, and answered by an
            // `open-file` rather than by a float: a definition is a *place*.
            Action::Lsp(LspAction::RequestDefinition {}) => {
                self.question = Some(Question::Definition);
                done()
            }
            // `T036`. Recorded, for the same reason the lookups are.
            Action::Lsp(LspAction::RestartLanguageServer { language }) => {
                if language.0.trim().is_empty() {
                    // `:restart-server` with nothing after it. Declined by
                    // name rather than guessed at: see that command's own
                    // note in `runtime/keymaps.scm` for why neither candidate
                    // meaning of an empty language is honest.
                    return declined("which language — :restart-server rust");
                }
                self.restart = Some(language.clone());
                done()
            }
            action => Outcome::Refused(Refusal::NotYetImplemented {
                task: action.spec().since.task,
            }),
        }
    }

    /// Writes the buffer out, to `path` or to where it came from.
    ///
    /// The whole rope, not a diff. **The tree learns where disk is**, which is
    /// what makes `[+]` mean *"different from the file"* rather than
    /// *"touched"*: undoing back past a write makes the buffer clean again,
    /// because [`UndoTree::is_modified`] is node identity and not a flag.
    ///
    /// # `:write <path>` at a buffer that had no file — `T107`
    ///
    /// The refusal below is the whole of what `:write` could do at a buffer
    /// with no name, and it is unchanged: there is nothing to write to, and
    /// guessing a name is the one thing an editor must not do with a person's
    /// text. What is new is the line after the write — the moment a buffer
    /// **gains** its first file is the moment its history gains somewhere to
    /// live, and until `T107` that moment passed silently. A scratch buffer
    /// written to `notes.md` kept undoing for the rest of the session and then
    /// reopened with an empty history, which is the one failure `T030`'s
    /// journal exists to prevent and the one shape nothing was watching.
    fn write(&mut self, path: Option<&Path>) -> Result<(), String> {
        let named = self.file.is_some();
        let target = path
            .map(Path::to_path_buf)
            .or_else(|| self.file.clone())
            .ok_or_else(|| "no file name — :write <path>".to_owned())?;
        let code = self.editor.code_ref();
        let text = code.slice(0, code.len_chars());
        std::fs::write(&target, text).map_err(|error| format!("{}: {error}", target.display()))?;
        // **After the write and before `mark_saved`.** After, because
        // [`journal_key`] canonicalizes and a journal keyed on a path that does
        // not exist yet is keyed on a guess; before, because the `Saved` record
        // three lines down is the first thing this history should say about
        // disk, and a journal attached after it would never record the save it
        // was opened by.
        if !named {
            self.note = self.timeline.attach(&target);
        }
        self.timeline.tree.mark_saved();
        let node = self.timeline.tree.saved().map(|node| node.0);
        self.timeline.append(wire_undo::Record::Saved { node });
        // `fsync` at a quiet point, which a write to disk is — `Log::append`
        // deliberately does not (`journal.rs`'s two-tier durability).
        if let Some(log) = self.timeline.log.as_ref() {
            let _ = log.sync();
        }
        self.dirty.set(false);
        self.file = Some(target);
        Ok(())
    }

    // -- `R2` — the undo tree ------------------------------------------------

    /// Where the cursor and selection are, as the tree records them.
    fn caret(&mut self) -> Caret {
        let offset = self.editor.get_cursor();
        let selection = self
            .editor
            .get_selection()
            .map(|selection| CharRange::new(selection.start, selection.end));
        Caret { offset, selection }
    }

    /// Applies a route through the tree to the text.
    ///
    /// [`Step::to_batch`] and `Editor::apply_batch` rather than
    /// [`Step::apply`]: the fork's `Code` drives the tree-sitter `InputEdit`
    /// from `insert`/`remove`, so a step written straight to a rope would keep
    /// the text and lose the parse (`undo.rs:88-104`). The cursor is the host's
    /// — `apply_batch` does not move it.
    fn walk(&mut self, steps: &[Step]) {
        if steps.is_empty() {
            return;
        }
        for step in steps {
            self.editor.apply_batch(&step.to_batch());
            self.editor.set_cursor(step.caret.offset);
            match step.caret.selection {
                Some(range) => self
                    .editor
                    .set_selection(Some(Selection::new(range.start, range.end))),
                None => self.editor.clear_selection(),
            }
        }
        self.editor.reset_highlight_cache();
        let to = self.timeline.tree.current().0;
        self.timeline.append(wire_undo::Record::Cursor { to });
        // Node identity, not the edit stream: `apply_batch` fires the change
        // callback that sets the flag, and an undo back to the saved node is
        // not a modification.
        self.dirty.set(self.timeline.tree.is_modified());
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

    /// The offsets a selection of [`Editing::selection_kind`] actually covers.
    ///
    /// **`V` drew one cell and deleted the line** (`CP-4`, on the shipping
    /// binary: `V j` highlighted the first row and a single character of the
    /// second, while `V j d` took both rows whole). The machine already knows
    /// the difference — `Machine::operator` widens a linewise operand with
    /// `text::line_span` before it acts — but the *selection* it emits on the
    /// way in is `span_between`'s characterwise one, and the vendored
    /// `Selection` is a flat offset range with no kind of its own. So the
    /// widening has to happen here, once, on the path every selection takes,
    /// or the highlight goes on describing a different span from the one the
    /// operator takes.
    ///
    /// Blockwise is deliberately left characterwise: a column selection is not
    /// expressible as one offset range at all, and drawing it needs a render
    /// path the fork does not have. Reported at `CP-4` rather than faked here.
    fn selected(&self, from: usize, to: usize) -> (usize, usize) {
        if self.selection_kind != SelectionKind::Line {
            return (from, to);
        }
        let code = self.editor.code_ref();
        let chars = code.len_chars();
        let last = to.saturating_sub(1).max(from).min(chars.saturating_sub(1));
        let (first_row, _) = code.point(from.min(chars.saturating_sub(1)));
        let (last_row, _) = code.point(last);
        let start = code.offset(first_row, 0);
        let end = if last_row + 1 < code.len_lines() {
            code.offset(last_row + 1, 0)
        } else {
            chars
        };
        (start.min(end), end)
    }

    /// Opens an edit batch, recording where the cursor was.
    ///
    /// **Re-entrant** ([`Editing::depth`]): an Action that edits more than once
    /// — `Replace`, `J`, `>` over a range, `gU` — opens one batch and closes
    /// one, so it is one entry in the fork's batch and one span in the undo
    /// group rather than several.
    ///
    /// The undo *group* is not opened here and is deliberately wider than a
    /// batch: [`UndoTree::begin`] is idempotent and first-wins, so an insert
    /// session's `before` caret is where the `i` was pressed, and the group
    /// closes when the machine says so (`History::CommitUndoGroup`).
    fn begin(&mut self) {
        self.depth += 1;
        if self.depth > 1 {
            return;
        }
        let cursor = self.editor.get_cursor();
        let selection = self.editor.get_selection();
        let before = self.caret();
        self.timeline.tree.begin(before);
        let code = self.editor.code_mut();
        code.tx();
        code.set_state_before(cursor, selection);
    }

    /// Closes it, recording where the cursor ended up — which is what undo
    /// restores.
    fn commit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth > 0 {
            return;
        }
        let cursor = self.editor.get_cursor();
        let selection = self.editor.get_selection();
        let code = self.editor.code_mut();
        code.set_state_after(cursor, selection);
        code.commit();
        self.editor.reset_highlight_cache();
    }

    fn insert(&mut self, at: Position, text: &str) {
        let offset = self.offset(at);
        self.splice(offset, offset, text);
    }

    fn remove(&mut self, span: Span) {
        let (from, to) = self.range(span);
        self.splice(from, to, "");
    }

    /// The one mutation in this program: `from..to` becomes `text`.
    ///
    /// Insert, delete and replace are the same edit with one side empty, which
    /// is also how [`TreeEdit`] carries it — so the undo tree learns every
    /// change through one call and there is no path that edits the rope without
    /// telling it.
    fn splice(&mut self, from: usize, to: usize, text: &str) {
        let removed = if to > from {
            self.editor.code_ref().slice(from, to)
        } else {
            String::new()
        };
        if removed.is_empty() && text.is_empty() {
            return;
        }
        self.begin();
        let before = self.caret();
        self.timeline.tree.record(
            before,
            TreeEdit {
                at: from,
                removed: removed.clone(),
                inserted: text.to_owned(),
            },
        );
        if !removed.is_empty() {
            self.editor.code_mut().remove(from, to);
        }
        if text.is_empty() {
            self.editor.set_cursor(from);
        } else {
            self.editor.code_mut().insert(from, text);
            self.editor.set_cursor(from + text.chars().count());
        }
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

    /// `gc` — comments or uncomments the lines a target covers (`T037`).
    ///
    /// **Whole lines, always.** A line comment is a line's property, so a
    /// target that covers three characters in the middle of a line comments
    /// that line; the alternative is a `//` in the middle of an expression.
    ///
    /// One batch, so `gcip` over a paragraph is one `u`. The decision — which
    /// direction, where the prefix goes, what happens to a blank line — is
    /// [`language::toggle_comment`]'s and is tested there over lines; this
    /// half is a splice.
    fn toggle_comment(&mut self, target: &Target, prefix: &str) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let code = self.editor.code_ref();
        let first = code.char_to_line(from);
        let last = code.char_to_line(to.saturating_sub(1).max(from));
        let start = code.line_to_char(first);
        // The end of the last line's text, never the newline after it: a
        // linewise `\n` inside the span would be re-emitted by the join below
        // and grow the file by one line per press.
        let end = code
            .line_to_char(last)
            .saturating_add(code.line(last).chars().filter(|c| *c != '\n').count());
        let lines: Vec<String> = (first..=last)
            .map(|line| {
                self.editor
                    .code_ref()
                    .line(line)
                    .chars()
                    .filter(|character| *character != '\n')
                    .collect()
            })
            .collect();
        let toggled = language::toggle_comment(&lines, prefix).join("\n");
        self.begin();
        self.splice(start, end, &toggled);
        self.commit();
    }

    /// The screen cell a passive float hangs off, `back` **characters** to the
    /// left of the cursor on its own row.
    ///
    /// `phosphor_ui::float::Anchor` is documented as screen cells and says why:
    /// *"this crate cannot map a buffer coordinate to a cell — the gutter
    /// width, the viewport and soft wrap all sit between the two — and the host
    /// is already doing that arithmetic to draw the cursor"*. This is that
    /// arithmetic, and it goes through the fork's own
    /// `char_col_to_visual` rather than counting characters, so a
    /// completion after a tab or a CJK identifier lands under the word rather
    /// than beside it.
    ///
    /// `(0, 0)` when the cursor is scrolled off screen. A float at the top-left
    /// is wrong, but the request that produced it was made at a cursor nobody
    /// can see, and there is no cell to be right about.
    fn anchor(&self, back: usize) -> Anchor {
        let Some((x, y)) = self.editor.get_visible_cursor(&self.area) else {
            return Anchor::new(0, 0);
        };
        let code = self.editor.code_ref();
        let cursor = self.editor.get_cursor();
        let line = code.char_to_line(cursor);
        let column = cursor.saturating_sub(code.line_to_char(line));
        let start = column.saturating_sub(back);
        let shift = code
            .char_col_to_visual(line, column)
            .saturating_sub(code.char_col_to_visual(line, start));
        Anchor::new(x.saturating_sub(u16::try_from(shift).unwrap_or(0)), y)
    }

    /// The offset of the newline that ends the line `cursor` is on, or the end
    /// of the rope on the last line.
    ///
    /// The boundary `R` stops at: [`Editing::accept`]'s fall-through overwrites
    /// one character in replace mode, and *"one character"* must not be the
    /// newline — vim's `R` at the end of a line appends and leaves the line
    /// break alone. The same clamp `Editing::offset` applies to a `Position`
    /// whose column is past the end, which is how `Machine::insert_key`'s own
    /// replace span degrades to an insert there.
    fn line_end(&self, cursor: usize) -> usize {
        let code = self.editor.code_ref();
        let line = code.char_to_line(cursor);
        code.line_to_char(line) + code.line_len(line)
    }

    /// How many characters of a word are behind the cursor — the prefix a
    /// completion replaces, and how far left of the cursor its float hangs.
    ///
    /// Word characters are `char::is_alphanumeric` plus `_`, which is what
    /// every server this build blesses treats as an identifier. It is
    /// deliberately not the fork's `word_boundaries`: that answers the whole
    /// word including what is *ahead* of the cursor, and accepting a completion
    /// must not eat the rest of an identifier the cursor is sitting inside.
    fn prefix_len(&self) -> usize {
        let code = self.editor.code_ref();
        let cursor = self.editor.get_cursor();
        let start = code.line_to_char(code.char_to_line(cursor));
        let before: Vec<char> = code.char_slice(start, cursor).chars().collect();
        before
            .iter()
            .rev()
            .take_while(|character| character.is_alphanumeric() || **character == '_')
            .count()
    }

    /// Whether the text behind the cursor ends in one of the server's
    /// `completionProvider.triggerCharacters` (`T038`).
    ///
    /// **The other half of the typing gate**, and the half `CP-4`'s review
    /// found missing: [`Editing::prefix_len`] counts identifier characters, so
    /// it is `0` after `foo.` and a minimum of two made member completion
    /// unreachable by typing. This asks the question a floor cannot — *did the
    /// language just say "now"* — and it asks it of the server rather than of a
    /// table here, because `.` is Rust's and `->` is C's and neither is this
    /// file's to know.
    ///
    /// Measured on the buffer rather than on the keystroke, for the same reason
    /// `prefix_len` is: it carries no state, so there is no flag to clear on
    /// `<C-e>`, on a motion, or on leaving insert. A trigger is only ever
    /// several characters long (`::`, `->`), so this reads back the longest one
    /// and compares suffixes.
    fn after_trigger(&self, triggers: &[String]) -> bool {
        if triggers.is_empty() {
            return false;
        }
        let longest = triggers.iter().map(|trigger| trigger.chars().count()).max();
        let Some(longest) = longest.filter(|count| *count > 0) else {
            return false;
        };
        let cursor = self.editor.get_cursor();
        let code = self.editor.code_ref();
        let start = code
            .line_to_char(code.char_to_line(cursor))
            .max(cursor.saturating_sub(longest));
        let behind = code.slice(start, cursor);
        triggers
            .iter()
            .any(|trigger| !trigger.is_empty() && behind.ends_with(trigger.as_str()))
    }

    /// The word being completed, as the server's `filterText` is matched
    /// against it (`phosphor_buffer::lsp::narrow`).
    ///
    /// The same span [`Editing::prefix_len`] measures and
    /// [`Editing::accept`] replaces, read as text — so what is filtered on and
    /// what an accepted row overwrites cannot disagree.
    fn prefix(&self) -> String {
        let code = self.editor.code_ref();
        let cursor = self.editor.get_cursor();
        code.slice(cursor.saturating_sub(self.prefix_len()), cursor)
    }

    /// Prose wrapped to the columns a passive float can actually give it
    /// (`CP-4`).
    ///
    /// **This is the seam §11 puts here.** `phosphor-ui` draws one string on
    /// one row and never wraps, so a server's paragraph arrives as one very long
    /// line and the float truncates it with `⋯` — at 120 columns that threw away
    /// roughly 40% of every hover line with no key to reveal the rest, which is
    /// what `CP-4` measured against rust-analyzer. `float::anchored_wrap_cols`
    /// publishes the width and `float::wrap_prose` is the wrapping; both live
    /// beside the chrome so the host cannot wrap to a width the float disagrees
    /// with.
    ///
    /// It is measured on [`Editing::area`] — the text area this buffer is drawn
    /// in — because that is what the float is capped against. A zero-width area
    /// (a buffer that has never been laid out) wraps to nothing and hands the
    /// lines back whole, which truncates exactly as before rather than looping.
    fn wrapped(&self, lines: &[String]) -> Vec<String> {
        float::wrap_prose(lines, float::anchored_wrap_cols(self.area.width))
    }

    /// Carries a signature/hover session's width across an answer.
    ///
    /// **The row, not the cell.** A signature float hangs off the *cursor*
    /// ([`Editing::anchor`] with no prefix), so its anchor moves on every
    /// keystroke while you type an argument list — the case the anti-thrash
    /// floor exists for. The call being described is on one row, so the row is
    /// what identifies the session here, where the word's first cell identifies
    /// a completion session.
    fn held_to_widest(&self, mut vm: SignatureVm) -> SignatureVm {
        vm.width_floor = self
            .signature
            .as_ref()
            .filter(|held| held.anchor.row == vm.anchor.row)
            .map_or(0, |held| {
                held.width_floor
                    .max(SignatureBody::new(held).desired_width())
            });
        vm
    }

    /// One completion session, as the float needs it (`T038`).
    ///
    /// The anchor is the **first cell of the word being completed**, which is
    /// where `7c` draws the list: under the word, not under the cursor.
    fn completions(&self, items: &[WireCompletion]) -> CompletionVm {
        CompletionVm {
            items: items
                .iter()
                .map(|item| CompletionItemVm {
                    label: item.label.clone(),
                    detail: item.detail.clone(),
                    kind: item.kind,
                    source: item.source.clone(),
                    deprecated: item.deprecated,
                })
                .collect(),
            selected: 0,
            // Wrapped for the same reason hover prose is: a doc comment's first
            // paragraph is one line on the wire and `MAX_DOC_ROWS` rows on
            // screen. The labels and details are *not* — those truncate, because
            // a label is the text that gets inserted and a wrapped identifier is
            // not that identifier.
            documentation: self.wrapped(
                &items
                    .first()
                    .map(|item| item.documentation.clone())
                    .unwrap_or_default(),
            ),
            anchor: self.anchor(self.prefix_len()),
            // Seeded by the caller, which is the only place that can see the
            // session this one replaces.
            width_floor: 0,
        }
    }

    /// Runs a keystroke's `otherwise` capability — what the key does when the
    /// surface it drives is not there.
    ///
    /// **This is the general half of a mechanism `accept-completion` has the
    /// literal half of.** `<space>` falls through to *text*, because typing is
    /// what that key would have done and text is a thing a keymap can spell.
    /// `<tab>` falls through to a *capability*, because one indent level is a
    /// per-language value (`set-option!`, `define-language!`) that a keymap
    /// spelling as four spaces would freeze for every language — the
    /// rust-table-in-scheme shape `T033` exists to forbid. Same split either
    /// way: the condition is the host's, the alternative is the binding's.
    ///
    /// [`Binding::Source`] is representable here and is **refused**. Scheme
    /// source needs the VM, and this runs inside [`Editing::act`], which is the
    /// buffer's own state machine and holds no [`Layer`]. A binding that wants
    /// to run scheme on a key already has a door — `keymap-set!` takes a
    /// [`Binding`] and the VM resolves it — and routing a second path to the VM
    /// through here would put arbitrary evaluation inside an arm that is
    /// supposed to be a text edit.
    fn fall_through(&mut self, binding: &Binding) -> Outcome {
        let (name, args) = match binding {
            Binding::Capability { name, args } => (name, args),
            Binding::Source { .. } => {
                return declined(
                    "a key's fall-through runs a capability, not scheme — \
                     use keymap-set! for a binding that evaluates source",
                );
            }
        };
        if self.falling_through {
            return declined("a fall-through may not fall through again");
        }
        let action = match Action::from_call(name, args) {
            Ok(action) => action,
            Err(error) => return declined(&error.to_string()),
        };
        self.falling_through = true;
        let outcome = self.act(&action);
        self.falling_through = false;
        outcome
    }

    /// Drops the completion session, whatever ended it.
    ///
    /// **The one place a session is dropped**, which is what makes
    /// [`Editing::chosen`] safe to add: a flag cleared at five call sites is a
    /// flag that survives one of them by the end of the next window, and this
    /// session's state is already three fields that have to agree.
    ///
    /// Dropping is not the only exit — `Lsp::IngestCompletions`' arm *replaces*
    /// a session, and clears the same flag for its own reason. The field's doc
    /// counts both.
    fn close_completion(&mut self) {
        self.completion = None;
        self.offered.clear();
        self.chosen = false;
    }

    /// Accepts a completion, replacing the word prefix under the cursor.
    ///
    /// # The three arguments, and the one that is a guard
    ///
    /// `then` is text typed **after** the accepted item — the space `<space>`
    /// leaves behind, and empty for `<enter>`. It is part of the same edit
    /// batch, so one `u` undoes the completion and its space together.
    ///
    /// `otherwise` is text typed **instead**, when the user has not chosen a
    /// row with `move-completion`. It is what makes `<space>` and `<enter>`
    /// bindable at all: a completion float is open for most of the time you
    /// are typing, so a `<space>` that accepted unconditionally would complete
    /// a word every time you finished one, and `<enter>` would stop making
    /// newlines. `nvim-cmp` spells the same rule `select = false` — the key
    /// acts only on a selection the user steered to. `<C-y>` passes `None`
    /// here and so keeps vim's meaning exactly: it accepts whatever is
    /// highlighted, because pressing it *is* the choosing.
    ///
    /// **Deliberately not a mode or a setting.** The guard lives in the
    /// argument because the keymap is where the fall-through *text* lives, and
    /// nothing else knows what key was pressed: `<space>` types `" "`,
    /// `<enter>` types `"\n"`, and a host that had to work that out would be
    /// the keymap-in-rust `T033` exists to forbid.
    ///
    /// # The fall-through types the way the mode types
    ///
    /// *"What the key would have typed"* is not one edit — in `R` it overwrites
    /// the character under the cursor. `Scope::of` folds `EditMode::Replace`
    /// into the insert scope (so does vim's `:imap`), and the loop's completion
    /// trigger is gated on `EditMode::Insert`, so in `R` there is never a float
    /// and this branch fires on **every** `<space>` and `<cr>`. A fall-through
    /// that always spliced turned `R` into `i`: `CP-4`'s review typed `RXY Z`
    /// over `abcdef` and got `XY Zdef` where vim gives `XY Zef`.
    ///
    /// So the span this replaces is [`Machine::insert_key`]'s own — the one
    /// character under the cursor, clamped to the end of the line, which is why
    /// `R` at the end of a line appends instead of eating the newline.
    ///
    /// # Errors
    ///
    /// A sentence, when there is nothing to accept and no `otherwise` to type
    /// instead, or when `index` names no row.
    fn accept(
        &mut self,
        index: u32,
        then: Option<&str>,
        otherwise: Option<&str>,
    ) -> Result<(), String> {
        // The guard, and it reads *"there is a session and the user steered in
        // it"* — one condition, because a key with no float open and a key over
        // a float nobody has touched are the same situation to the hands.
        if let Some(fallthrough) = otherwise
            && !(self.chosen && self.completion.is_some())
        {
            let cursor = self.editor.get_cursor();
            let over = if self.mode == EditMode::Replace {
                self.line_end(cursor).min(cursor + 1)
            } else {
                cursor
            };
            self.begin();
            self.splice(cursor, over, fallthrough);
            self.commit();
            return Ok(());
        }
        let session = self
            .completion
            .as_ref()
            .ok_or_else(|| "no completion list is open".to_owned())?;
        // **`0` is the selected row**, and it is not a row number: the
        // declaration counts from 1, so nothing else can mean it. A keymap is
        // data and cannot read a selection the host holds, so without this
        // spelling `<C-y>` could only ever accept a fixed row — which is the
        // one thing an accept key must not do.
        let row = if index == 0 {
            session.selected
        } else {
            (index as usize) - 1
        };
        let insert = self
            .offered
            .get(row)
            .map(|offer| offer.insert.clone())
            .ok_or_else(|| format!("no completion {row} in a list of {}", self.offered.len()))?;
        let back = self.prefix_len();
        let cursor = self.editor.get_cursor();
        self.close_completion();
        self.begin();
        self.splice(cursor - back, cursor, &insert);
        if let Some(trailing) = then.filter(|trailing| !trailing.is_empty()) {
            let at = self.editor.get_cursor();
            self.splice(at, at, trailing);
        }
        self.commit();
        Ok(())
    }

    /// Types one indent level at the cursor — what `<tab>` does in insert mode
    /// (`T104`).
    ///
    /// **Not [`Editing::indent`] with a one-line target.** That shifts the
    /// whole line from wherever the caret is, which is vim's `<C-t>`; `<Tab>`
    /// types at the cursor, and mid-line those are different edits.
    ///
    /// The column is a **display** column, not a character count
    /// (`Code::char_col_to_visual`), which is the arithmetic this repo has
    /// shipped three bugs from: press `<tab>` after a CJK character and the
    /// stop it advances to is measured from the two cells that character
    /// occupies, not from the one `char` it is.
    ///
    /// **In `R` this overwrites, exactly as [`Editing::accept`]'s fall-through
    /// does.** `Scope::of` folds `EditMode::Replace` into the insert scope, so
    /// the `<tab>` row binds in `R` too — and a version of this that always
    /// spliced made `R` into `i` for one more key, which is the third time this
    /// window found that shape (`<space>`, `<cr>`, and now this).
    ///
    /// **One character, not one level.** `R<Tab>` over `abcdefgh` with a
    /// four-cell stop gives `    bcdefgh`: the tab spends four cells and
    /// consumes one character, so the line grows. Measured against
    /// `nvim -u NONE` with `set expandtab tabstop=4 softtabstop=0` this
    /// session, which is where the earlier claim that vim inserts here came
    /// from and did not survive being run.
    fn insert_indent(&mut self) {
        let cursor = self.editor.get_cursor();
        let code = self.editor.code_ref();
        let line = code.char_to_line(cursor);
        let column = code.char_col_to_visual(line, cursor - code.line_to_char(line));
        let typed = self.indent_style.typed_at(column);
        let over = if self.mode == EditMode::Replace {
            self.line_end(cursor).min(cursor + 1)
        } else {
            cursor
        };
        self.begin();
        self.splice(cursor, over, &typed);
        self.commit();
    }

    /// Shifts whole lines by one indent level, as `>` and `<` mean it.
    ///
    /// One batch for the whole range ([`Editing::begin`] is re-entrant), so
    /// `>` over ten lines is one undo step rather than ten.
    ///
    /// **The unit is [`Editing::indent_style`]'s and no longer the fork's.**
    /// It was `Code::indent` — a `match` on the grammar name inside
    /// `vendor/ratatui-code-editor` that nothing a user writes could reach.
    fn indent(&mut self, target: &Target, delta: i64) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let unit = self.indent_style.unit.clone();
        let first = self.editor.code_ref().char_to_line(from);
        let last = self
            .editor
            .code_ref()
            .char_to_line(to.saturating_sub(1).max(from));
        self.begin();
        for line in first..=last {
            let start = self.editor.code_ref().line_to_char(line);
            if delta > 0 {
                self.splice(start, start, &unit);
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
                    self.splice(start, start + width, "");
                }
            }
        }
        self.commit();
    }

    // -- `R19` — folds -------------------------------------------------------

    /// The line a fold covering `target` starts on, innermost first.
    ///
    /// `za` from anywhere inside a fold closes it, which is vim's rule and not
    /// the fork's: `Editor::toggle_fold_at_line` needs the line the fold
    /// *starts* on, so this walks `Code::fold_ranges` — the ranges the
    /// language's own `folds.scm` produced — and takes the narrowest one that
    /// contains the cursor.
    fn fold_start(&mut self, target: &Target) -> Option<usize> {
        let (from, _) = self.target_range(target)?;
        let line = self.editor.code_ref().char_to_line(from);
        self.editor
            .code_ref()
            .fold_ranges()
            .iter()
            .filter(|range| range.start_line <= line && line <= range.end_line)
            .min_by_key(|range| range.end_line - range.start_line)
            .map(|range| range.start_line)
    }

    /// `za` / `zc` / `zo` — the fold at a target, in the state asked for.
    fn set_fold(&mut self, target: &Target, state: FoldState) -> bool {
        let Some(start) = self.fold_start(target) else {
            return false;
        };
        let folded = self.editor.fold_hidden_lines(start).is_some();
        let wanted = match state {
            FoldState::Folded => true,
            FoldState::Unfolded => false,
            FoldState::Toggle => !folded,
        };
        if wanted == folded {
            return true;
        }
        self.editor.toggle_fold_at_line(start)
    }

    /// `zM` — every fold deeper than `level` closed.
    ///
    /// Vim's `foldlevel`, and its arithmetic: a fold's level is one plus the
    /// number of folds that contain it, and a fold closes when its level is
    /// **greater than** `level`. So `FoldAll { level: 0 }` closes everything,
    /// which is what `zM` means.
    fn fold_all(&mut self, level: u32) {
        let ranges: Vec<(usize, usize)> = self
            .editor
            .code_ref()
            .fold_ranges()
            .iter()
            .map(|range| (range.start_line, range.end_line))
            .collect();
        for range in &ranges {
            let depth = ranges
                .iter()
                .filter(|other| *other != range && other.0 <= range.0 && range.1 <= other.1)
                .count();
            let deep = u32::try_from(depth).unwrap_or(u32::MAX).saturating_add(1) > level;
            if deep && self.editor.fold_hidden_lines(range.0).is_none() {
                self.editor.toggle_fold_at_line(range.0);
            }
        }
    }

    /// `zR` — every collapsed fold opened.
    fn unfold_all(&mut self) {
        let starts: Vec<usize> = self
            .editor
            .code_ref()
            .fold_ranges()
            .iter()
            .map(|range| range.start_line)
            .collect();
        for start in starts {
            if self.editor.fold_hidden_lines(start).is_some() {
                self.editor.toggle_fold_at_line(start);
            }
        }
    }

    /// `~`, `gu`, `gU` — the letters a target covers, recased.
    ///
    /// `phosphor_core::input::text::cased` is the one definition of what each
    /// of the three means, so a door sending `set-case` and a keystroke cannot
    /// differ.
    ///
    /// **The cursor ends where the change ends**, which is [`Editing::splice`]'s
    /// own rule and not a decision made here. That is exactly vim for `~` —
    /// the case operator fused with `l`, so `~~~` walks a word — and one cell
    /// off for `gUiw`, where vim leaves the cursor at the *start* of what it
    /// changed. The difference between the two is fused-versus-operator and
    /// lives in the machine, which emits no cursor move for either; closing it
    /// means an Action that says where the cursor goes, which is a vocabulary
    /// change rather than an edit here. **Flagged, not folded in.**
    fn set_case(&mut self, target: &Target, case: phosphor_core::request::CaseChange) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let end = to.min(self.editor.code_ref().len_chars());
        if end <= from {
            return;
        }
        let text = self.editor.code_ref().slice(from, end);
        let cased = motion::cased(&text, case);
        if cased == text {
            return;
        }
        self.splice(from, end, &cased);
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
        // One batch: `J` over a range is one undo step, and each join is two
        // edits (the newline out, the space in).
        self.begin();
        for _ in first..last {
            let text = self.text();
            let Some(next) = text.line(u32::try_from(first).unwrap_or(0) + 2) else {
                break;
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
        self.commit();
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
        // One key is one sentence, and it is the **first** one — see the write
        // below, which is the only thing that reads this.
        let mut said = false;
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
                    // `T098`: **a refused key says why.** This was `let _ =`,
                    // and that is why a deferred binding read as a broken one —
                    // the ex line has always spoken its refusals and a keystroke
                    // never did, so `q` bound to something unbuilt looked
                    // exactly like `q` bound to nothing.
                    //
                    // **First refusal wins, and it used to be the last.** A
                    // `key/run` row is several Actions and the later ones fail
                    // *because* the earlier one did: `ZZ` is `save-buffer` then
                    // `quit`, so on a buffer with no name it said `unsaved work
                    // — force it or save first` and swallowed `no file name —
                    // :write <path>`, which is the half that says what to type.
                    // Found by hand at `CP-4`. [`submit_ex`] has always taken
                    // the first — it is a `find_map` — so `:wq` and `ZZ` are
                    // the same Action list and were answering differently.
                    if let Outcome::Refused(refusal) = self.editing.apply(&action)
                        && !said
                    {
                        self.editing.refused = Some(refusal);
                        said = true;
                    }
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
                    // **The one writer of [`Editing::mode`]**, and it is here
                    // rather than in `Editing::act` because `Action::Input` is
                    // the family that never reaches it. See the field.
                    InputAction::SetMode { mode } => self.editing.mode = *mode,
                    InputAction::SetCount { .. } | InputAction::SelectRegister { .. } => {}
                    // `T099`, and the one arm in this match that answers rather
                    // than keeps state. [`Machine::apply`]'s own arm is
                    // deliberately a no-op and `apply` returns nothing, so the
                    // host is the only place a `set-macro-recording` can become
                    // the refusal `T098` asks for — `input.rs` says exactly that
                    // where the no-op is.
                    //
                    // The task is read off the capability's row, never written
                    // here: the day `T099` builds the recorder this arm is what
                    // it replaces, and until then `q` declines by name instead
                    // of succeeding silently, which is the failure the whole
                    // deferred-keys section exists to end.
                    InputAction::SetMacroRecording { .. } => {
                        if !said {
                            self.editing.refused = Some(Refusal::NotYetImplemented {
                                task: action.spec().since.task,
                            });
                            said = true;
                        }
                    }
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
///
/// # Why `accept-completion` is on this list
///
/// It edits the buffer without being a `Buffer::…` Action — [`Editing::accept`]
/// calls [`Editing::splice`] directly, and its `otherwise` fall-through types
/// whatever the key would have typed. So it moves the cursor exactly as much as
/// the text it wrote, and until `T106` it was the one editing verb this
/// function did not name.
///
/// **`CP-4` is what made that fatal.** Before `<cr>` was bound in the insert
/// scope, an accept moved the cursor by a few columns and a viewport that did
/// not follow was invisible. Binding `<cr>` routes *every newline typed in
/// insert mode* through this Action: with the reveal missing, pressing enter
/// past the last visible row walked the cursor off the bottom of the screen and
/// you typed where you could not see. Driven on the installed binary at 80x24
/// — `A` then thirty `<cr>` — the viewport stayed on lines 1..23 with the
/// statusline reading `31:1`.
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
                // `T104`. `CP-4` found `accept-completion` missing from this
                // list and the symptom was that enter stopped scrolling — you
                // type where you cannot see. `insert-indent` writes at the
                // cursor and moves it, so it belongs here for the same reason.
                | BufferAction::InsertIndent { .. }
        ) | Action::History(HistoryAction::Undo { .. } | HistoryAction::Redo { .. })
            | Action::Lsp(LspAction::AcceptCompletion { .. })
    )
}

/// One posted event, authorized and applied — the loop's whole handling of a
/// second producer.
///
/// **There is no second match here, and that is the design** ([`events`]): a
/// producer posts an [`Action`], and an Action is applied by [`Editing::act`],
/// which every keystroke already goes through. So an LSP client posting
/// `ingest-diagnostics` needs no arm of its own in this file, and when `T040`
/// gives that capability an arm it is the same arm a key would reach.
///
/// The notice is what a producer gets instead of silence. `Editing::act`'s `_`
/// arm answers with the capability's own task, so a posted Action the binary
/// does not apply yet says *which task builds it* — and this prefixes *which
/// producer asked*, because a task id with no caller is a message nobody can
/// trace. `T098` made the same argument for a refused key.
///
/// # Why there is a policy check, added by review
///
/// *"A producer needs no arm of its own"* is the property that makes this an
/// extension point, and it is the same property that made it **a fourth door
/// with weaker authorization than the three the registry knows about**.
/// Invariant 2 is *one API, three doors*, and what varies per door is
/// authorization: [`McpPolicy`] rates `quit` `Deny` (*"refused unless a rule in
/// `init.scm` opens it"*) and `apply-workspace-edit` `Ask` (`T060`'s queue).
/// The first version of this function handed whatever a producer posted
/// straight to `Editing`, so `App::Quit { force: true }` — which has a live arm
/// that sets `quit` and breaks the loop — was reachable from any future
/// producer. Nothing posts today; that is the window to close it in.
///
/// **The MCP door's rating is borrowed, deliberately, and it is the strictest
/// published one.** A producer is not the user: an LSP server, an ACP stream
/// and a VCS poll are all *"something that is not your keyboard asking for a
/// mutation"*, which is the class [`McpPolicy`] rates. Reading `spec().mcp`
/// names no capability, so this stays a total function over the table rather
/// than a list here that could rot. The refusals are [`Refusal::Declined`] and
/// not [`Refusal::DoorDenied`] because `registry::Door` has no variant for the
/// loop — the same absence `events`' header records for the actor a `Request`
/// would need.
///
/// # Why [`Editing::act`] and not [`Editing::apply`]
///
/// `apply` reveals the cursor when [`moves_cursor`] says the Action moved it,
/// and a reveal is `View::Scroll` — **invariant 3's single writer, moving the
/// viewport the user is looking at, on behalf of something that is not the
/// user**. `IMPLEMENTATION-PLAN.md`'s scroll-authority note is explicit that
/// this is enforced by not calling it. So a posted `Buffer::Insert` edits the
/// buffer and leaves the viewport exactly where it was; the user scrolls to it
/// or does not. Found by review, which also found that this function's own doc
/// already named `act` while the code called `apply`.
fn deliver(editing: &mut Editing, posted: &events::Posted) -> Option<String> {
    let outcome = match posted.action.spec().mcp {
        McpPolicy::Allow => editing.act(&posted.action),
        // Not applied and not dropped: the ask queue is where this goes when it
        // exists, and until then a producer is told what it is waiting for.
        McpPolicy::Ask => declined("needs an ask first — T060 builds the queue"),
        McpPolicy::Deny => declined("denied to a producer — only the keyboard asks for this"),
    };
    // `T100`: one reduction of an `Outcome` to a notice, shared with the ex line
    // and with `Intent::Keymap`, so a fourth case cannot be handled here and
    // dropped there.
    phosphor_steel::answer::trouble(&outcome).map(|said| format!("{}: {said}", posted.source))
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
    match surface {
        Surface::Buffer => false,
        // `6d` draws `q close` in the footer, and here — unlike `6b`, where
        // the same footer was the `CP-3` amendment — it is honest: a keymap
        // grid is not a text input, so `q` is not a character you are typing.
        // `esc` still closes, because §9's `esc` always does.
        Surface::Help => matches!(key.code, KeyCode::Esc | KeyCode::Char('q')),
        _ => matches!(key.code, KeyCode::Esc),
    }
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
///
/// **The press and the drag are the machine's** ([`Machine::click`],
/// [`Machine::drag`]), and `CP-4` is why: this function used to build a
/// selection out of the editor alone, so a drag left the editor holding a
/// highlight the machine had never heard of — unclearable by `<esc>`,
/// unmovable by a motion, and invisible to an operator. The wheel stays here
/// because scrolling is a request about a viewport and nothing to do with the
/// grammar.
fn mouse_actions(machine: &mut Machine, editing: &Editing, mouse: MouseEvent) -> Vec<Action> {
    let editor = &editing.editor;
    // **The area comes off `editing`, not beside it.** It was a fourth
    // parameter, and the loop bound `let area = editing.area;` on the line above
    // the call to feed it — two names for one `Copy` field across a seam, which
    // is a place for the two to be different.
    let area = editing.area;
    let at = || {
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
        MouseEventKind::Down(MouseButton::Left) => {
            at().map_or_else(Vec::new, |position| machine.click(position))
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            at().map_or_else(Vec::new, |position| machine.drag(position, &editing.text()))
        }
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
    /// `6d` — `:help`, and `:help <topic>`. The float is [`Editing::help`]'s
    /// ask, resolved against the live keymap by [`help_float`].
    Help,
}

/// What `:help` asked for (`T097`).
///
/// Recorded by [`Editing::act`] and drained by the loop, for the same reason
/// `open-file` and `open-prompt` are: composing the page needs the editor
/// layer, and [`Editing`] is behind the barrier from it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Help {
    /// `:help` — every binding the layer has.
    Index,
    /// `:help <topic>` — the same grid, narrowed. See [`about`].
    Topic(String),
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
            .find_map(|action| phosphor_steel::answer::trouble(&editing.apply(action))),
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

// ---------------------------------------------------------------------------
// `T097` — `:help`
// ---------------------------------------------------------------------------

/// `6d`'s footer: *"every legal key, always visible"* (§4).
///
/// One key, because there is one: the grid takes no input, so `esc` and `q`
/// both close it and the footer names the one `6d` draws.
fn help_footer() -> Child {
    Child::new(Node::KeyHints {
        density: Density::Footer,
        hints: vec![KeyHint {
            key: KeySeq("q".to_owned()),
            verb: "close".to_owned(),
        }],
    })
}

/// The topics `:help` offers, in the order the index lists them.
///
/// The five scopes first — *"what can I press right now"* is the question a
/// person actually has — then the four families the grammar already has joints
/// at. **The list is names only**: what each one *holds* is counted off the
/// live table when the index is drawn, so a topic that binds nothing is left
/// off rather than offered and empty.
const TOPICS: &[(&str, &str)] = &[
    ("normal", "normal mode"),
    ("visual", "visual mode, and its two line kinds"),
    ("operator-pending", "what an operator takes as its operand"),
    ("object", "the nouns after i and a"),
    ("insert", "insert mode, which is mostly text"),
    ("motions", "every motion and line address"),
    ("operators", "the operators, fused and not"),
    ("objects", "every text object"),
    (
        "agent-objects",
        "6d's four: unseen region, hunk, thread, review block",
    ),
];

/// The `:help` page, read off the live keymap.
///
/// **Nothing here is a page of prose.** There is no help *text* in this editor
/// and deliberately so — prose about a keymap is a second copy of the keymap,
/// and it goes stale the first time somebody rebinds a key. A topic is a
/// *narrowing of the same grid* ([`about`]), so every row is still the table's
/// own words and still true after a `(keymap-set! …)` at the REPL.
///
/// `:help` with no topic is **the index** — `open-help`'s own wording, and the
/// right answer for a table of 200-odd bindings that a float can show 25 of.
/// Listing the whole table there would draw the motions and stop, and a help
/// page that silently ends is worse than one that says where to look.
///
/// [`None`] when the narrowing keeps nothing: the caller says so on the
/// statusline rather than opening an empty float.
fn help_float(layer: &mut Layer, ask: &Help) -> Option<phosphor_core::view::Float> {
    let entries = layer.entries();
    let hints: Vec<KeyHint> = match ask {
        Help::Index => index(&entries),
        Help::Topic(topic) => {
            let topic = topic.to_lowercase();
            entries
                .iter()
                .filter(|entry| about(entry, &topic))
                .map(keymap::Entry::hint)
                .collect()
        }
    };
    if hints.is_empty() {
        return None;
    }
    Some(phosphor_core::view::Float {
        // `6d`'s own header: the command that opened it, spelled in full (§6),
        // and what to type next as the meta half.
        header: Some(phosphor_core::view::FloatHeader {
            left: spelled(ask),
            right: matches!(ask, Help::Index).then(|| ":help <topic>".to_owned()),
        }),
        mood: Mood::Informational,
        body: Child::new(Node::KeyHints {
            density: Density::Help,
            hints,
        }),
        footer: Some(help_footer()),
    })
}

/// The index: one row per topic that holds anything, with its own count.
///
/// The count is the point. It is the same filter the topic itself runs
/// ([`about`]), so a row can never promise bindings the topic does not answer
/// with — and a `(keymap-set! …)` at the REPL moves the number.
fn index(entries: &[keymap::Entry]) -> Vec<KeyHint> {
    TOPICS
        .iter()
        .filter_map(|(topic, what)| {
            let bound = entries.iter().filter(|entry| about(entry, topic)).count();
            // The topic alone in the key column. A `KeySeq` is spelled by the
            // widget and a space in one is a *separator between keys*
            // (`key_hints.rs`), so `:help normal` would draw as `:helpnormal`;
            // the command that opens a row is the header's meta half instead.
            (bound > 0).then(|| KeyHint {
                key: KeySeq((*topic).to_owned()),
                verb: format!("{what} — {bound} bound"),
            })
        })
        .collect()
}

/// The command that opened a page, as `6d` draws it in the header.
fn spelled(ask: &Help) -> String {
    match ask {
        Help::Index => ":help".to_owned(),
        Help::Topic(topic) => format!(":help {topic}"),
    }
}

/// What a topic nobody's bindings answer to gets told.
fn no_help(ask: &Help) -> String {
    match ask {
        // Unreachable while any binding exists at all; honest if the layer is
        // empty, which is what a broken `keymaps.scm` leaves behind.
        Help::Index => "nothing is bound — :help has nothing to show".to_owned(),
        Help::Topic(topic) => format!("no help for {topic} — nothing is bound under it"),
    }
}

/// Whether a binding belongs on `:help <topic>`.
///
/// Three questions, asked of the table rather than of a list kept here, so a
/// topic cannot name rows that no longer exist:
///
/// * **a scope** — `:help visual` is what visual mode can do;
/// * **a role family** ([`family`]) — `:help operators`, `:help agent-objects`;
/// * **the verb** — anything else is a substring of what the binding says it
///   does, which is what makes `:help fold` and `:help claude` work with
///   nothing written down for either.
///
/// `topic` is already lowercased by the caller.
fn about(entry: &keymap::Entry, topic: &str) -> bool {
    entry.scope.eq_ignore_ascii_case(topic)
        || family(entry.role.as_ref(), topic)
        || entry.verb.to_lowercase().contains(topic)
}

/// Whether a role is in the family `topic` names.
///
/// The four families are the grammar's own joints, not a taxonomy invented
/// here: a key is an operator, an operand, an object, or none of the three.
/// **`agent-objects` is `6d`'s topic** and is the one that is not merely a
/// role — it is [`TextObject`]'s four agent-native nouns, which the vocabulary
/// already separates from vim's (`request.rs`: *"`u` — the unseen region under
/// the cursor (`6d`)"*).
fn family(role: Option<&Role>, topic: &str) -> bool {
    let Some(role) = role else {
        return false;
    };
    match topic {
        "motions" => matches!(role, Role::Motion(_) | Role::Goto(_)),
        "operators" => matches!(role, Role::Operator(_) | Role::Fused { .. }),
        "objects" => matches!(role, Role::Object { .. }),
        "agent-objects" => {
            matches!(role, Role::Object { object, .. } if is_agent_noun(*object))
        }
        _ => false,
    }
}

/// The four nouns `6d` is about: *"u unseen region · h hunk · t thread · b
/// review block"*.
const fn is_agent_noun(object: TextObject) -> bool {
    matches!(
        object,
        TextObject::UnseenRegion | TextObject::Hunk | TextObject::Thread | TextObject::Block
    )
}

/// What is on disk at `file`, or `None` when the path is free.
///
/// **A path with nothing behind it is a new buffer, not a refusal**, and until
/// `CP-4` this editor was the only one that disagreed: the open arm treated
/// every `Err` from `read_to_string` alike, so `:e /tmp/x.lua` on a file that
/// did not exist printed *"No such file or directory"* and opened nothing. That
/// is not a corner — it is how a file gets created at all, and vim says
/// `"x.lua" [New File]` rather than declining.
///
/// So the cases are told apart here instead of being collapsed into one arm:
///
/// - **Nothing at the path, and its directory exists** — a new buffer. The
///   language is still knowable, because a declaration claims an *extension*
///   and an extension is in the name (`grammar_of`, `adopt`).
/// - **Nothing at the path and no directory either** — still an error, and this
///   is the case worth keeping. An empty buffer that `:w` cannot write is a
///   worse answer than the refusal it replaced: the refusal costs a keystroke,
///   and the buffer costs whatever was typed into it before anyone found out.
///   The message names the *directory*, because the file is not the thing that
///   is missing.
/// - **Anything else** — a directory, a permission, bytes that are not UTF-8 —
///   still an error. The path exists and this editor cannot show what is behind
///   it, which is a fact the user has to be told rather than a blank to fill.
fn opening(file: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let directory = holding(file);
            if directory.is_dir() {
                Ok(None)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no directory {}", directory.display()),
                ))
            }
        }
        Err(error) => Err(error),
    }
}

/// The directory a path names a file in.
///
/// `Path::parent` answers `Some("")` for a bare `x.lua` and `""` is not a
/// directory anything can be tested against, so the cwd is named explicitly.
/// Without this, `:e x.lua` in the directory you are already in would report
/// that the directory does not exist.
fn holding(file: &Path) -> &Path {
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// What the statusline says about a buffer with nothing on disk behind it yet.
///
/// It names the write, because *"new file"* alone leaves the one question a
/// person actually has — is this real — unanswered until they try.
fn new_file(file: &Path) -> String {
    format!("{}: new file — :w creates it", file.display())
}

/// What the first frame of a bare `phosphor` says (`T107`).
///
/// **Not `[No Name]`, and the difference is the point.** vim names the absence
/// and stops; the one question a person has at a buffer with no file is what
/// turns it into one, and §6's rule is that the UI teaches the whole command
/// — *"never cryptic contractions"*. So this is the same sentence
/// [`Editing::write`] refuses with, said before it is asked rather than after,
/// and typing `:write` at this buffer produces no new information.
///
/// It rides the notice row, so the next keystroke spends it. That is deliberate
/// — §6 is telegraphic and this is an opening line, not a state — and it is why
/// the *statusline* is left drawing nothing where a file would go: a permanent
/// `[No Name]` would be a segment saying the same thing every frame for the
/// rest of the session.
fn no_file() -> String {
    "no file — :write <path> creates one".to_owned()
}

/// The grammar the fork should parse `path` with, off the declarations
/// (`T037`).
///
/// **The Rust extension table this replaced is gone**, and its going is the
/// task: it mapped ten extensions to ten grammars in a `match`, which made the
/// bundled languages privileged in exactly the way `define-language` exists to
/// prevent. Every answer here now comes from a `.scm` file — `.rs` is Rust
/// because `runtime/languages/rust.scm` claims `rs`, and a thirteenth
/// declaration typed at `:repl` claims its own extension the same way.
///
/// `"text"` for a file no declaration claims, for one whose declaration names
/// no grammar (`csv`), **and for one naming a grammar this build cannot load**
/// (`steel`, which declares `scheme`). That is not an error and not a
/// degradation: `Code::new` skips parser setup for a name it does not know and
/// the buffer renders in `syntax.text`, which is what second tier looks like.
///
/// The last of those three is what keeps this in step with
/// [`Languages::tier`], which is *"the intersection"* of what a declaration
/// names with what the host can load. Passing `scheme` through would work — the
/// fork answers `None` for it — but it would make this function and the
/// `languages` query disagree about the same file, and one of them would be
/// wrong the day a `scheme` grammar is bundled.
fn grammar_of<'a>(languages: &'a Languages, path: &Path) -> &'a str {
    languages
        .by_path(path)
        .and_then(|language| languages.get(language))
        .and_then(|spec| spec.grammar.as_deref())
        .filter(|grammar| languages.grammars().any(|known| known == *grammar))
        .unwrap_or("text")
}

/// Points the language machinery at whatever buffer is now open: its
/// declaration, its comment prefix, and its server (`T036`, `T037`).
///
/// Answers the absolute path the servers were told about, which is what a
/// later `didChange` and `didClose` have to name — LSP addresses documents by
/// URI and the path the user typed has none (`lsp::absolute`).
///
/// **Attaching is idempotent by construction.** `LanguageServers::attach`
/// *"replaces whatever was running for that language"*, so opening a second
/// Rust file does not start a second rust-analyzer; it re-roots the one that is
/// running, which is what a different project root would need anyway.
///
/// A buffer with no file — `--repl`, `C-c buffer`, and since `T107` a bare
/// `phosphor` — tells no server anything. There is no document to open: a
/// server addresses files.
///
/// **And it stays that way until the buffer is opened again.** `:write
/// notes.rs` gives a scratch buffer a name; it does not re-run this, so the
/// language, the grammar and the server are still the ones a nameless buffer
/// had. That is deliberate rather than pending: adopting a grammar means
/// rebuilding the `Editor` the way the `open-file` arm does, which throws away
/// the cursor and the selection of a buffer the user is still in the middle of.
/// `:e` on the file the write just created is the door that already exists, and
/// it costs one command rather than a surprise.
///
/// # The table is asked for again at every open, and that is `T037`'s criterion
///
/// The loop held one `Languages` from boot and passed it here, with a comment
/// claiming a thirteenth language declared at `:repl` was *"a fact about the
/// next file opened"*. It was not: `:e` **is** the next file opened, and it
/// read the snapshot. Measured at `CP-4` — `(define-language! "zz" …)` at the
/// REPL, `:e sample.zz`, then `gcgc`, and the statusline answered *"this
/// language declares no line comment"* until the binary was restarted, at which
/// point the same layer commented it correctly.
///
/// So every caller now reads `AppHost::languages` at the moment it needs it.
/// The cost is one table clone per file opened; the alternative is a criterion
/// that is true only across a restart, which is the one thing *"no Rust
/// change"* was supposed to mean.
fn adopt(
    editing: &mut Editing,
    languages: &Languages,
    servers: &LanguageServers,
) -> Option<Document> {
    let language = editing
        .file
        .as_deref()
        .and_then(|file| languages.by_path(file))
        .cloned();
    editing.comment_prefix = language
        .as_ref()
        .and_then(|id| languages.comment_prefix(id))
        .map(str::to_owned);
    editing.server = language
        .as_ref()
        .and_then(|id| Some((id, languages.get(id)?)))
        .and_then(|(id, spec)| ServerSpec::from_language_spec(id, spec))
        .map(|spec| spec.command);
    editing.language.clone_from(&language);
    let (Some(language), Some(file)) = (language, editing.file.clone()) else {
        return None;
    };
    let path = lsp::absolute(&file);
    let root = lsp::attach(servers, languages, &language, &path);
    // Sent even when no server is running: the client records the text anyway,
    // which is what lets it convert a UTF-16 column if one attaches later
    // (`LanguageServers::open`).
    servers.open(&language, path.clone(), editing.contents());
    Some(Document {
        key: lsp::key_for(&path, root.as_deref()),
        path,
    })
}

/// The buffer as the servers know it: the path they were told, and the key
/// their diagnostics come back under.
///
/// **Two paths and not one, and the second is not redundant.** The client
/// addresses a document by the absolute path it was given and publishes about
/// it by a path made relative to the project root — see `lsp::key_for`. A host
/// that carried one of them would either send `didChange` for a document no
/// server has, or read a diagnostic store nothing ever writes to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Document {
    /// What `didChange` and `didClose` name — absolute, because a URI has no
    /// other spelling.
    path: PathBuf,
    /// What `ingest-diagnostics` arrives under.
    key: PathBuf,
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
    use phosphor_core::request::{Actor, Position, Severity, Span};
    use phosphor_core::value::Value;
    use phosphor_steel::answer;
    use phosphor_steel::host::Host;
    use ratatui::layout::Rect;

    use super::door::Evaluate as _;
    use phosphor_core::input::key::parse_seq;
    use phosphor_core::input::table::{Resolution, Role, Scope};
    use phosphor_core::input::text::Text as _;
    use phosphor_ui::float::{CompletionList, FloatBody as _, anchored_wrap_cols};

    use phosphor_core::language::Languages;
    use phosphor_core::request::LanguageId;

    use super::{
        AppHost, COMPLETION_MIN_CHARS, COMPLETION_MIN_CHARS_DEFAULT, Caret, Cli,
        CommandFactory as _, EXPAND_TAB, Editing, ExStep, FromArgMatches as _, IndentStyle, Intent,
        Key, Layer, Lookup, Machine, NodeId, Outstanding, Repl, ReplStep, Session, StatusVm,
        Surface, TAB_WIDTH, Table, UndoTree, Vm, WireCompletion, boot, buffer, closes_surface,
        completion_floor, decode, deliver, door, ex_key, grammar_of, indent_style, is_press,
        repl_key, restored, seeding, server_chip, split, submit_ex, vm, wire_undo,
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

    /// A layer over a runtime tree with its own config home — **[`stack`]
    /// itself**, which is the function `vm` is two environment reads and a
    /// call to.
    ///
    /// It used to be a second copy of those calls, and the copy is what made
    /// the order untested: a review swapped the two `if let` blocks in `vm`,
    /// changed nothing here, and every test that claimed to protect the order
    /// passed. There is one order now and this is a name for it, so a mutation
    /// in the shipping path is a mutation in what these tests run.
    ///
    /// **`None` is a shape a test has to be able to ask for**, not a
    /// convenience: it is what `Runtime::root` answers on an installed binary
    /// run from outside a checkout with no `$PHOSPHOR_RUNTIME` — the machine
    /// whose *only* layer is the config home, which is the population §34 is
    /// about. A report with no root at all takes a different arm of
    /// [`Layer::booted_already`].
    fn booted_with_config(root: Option<&Path>, config: &Path) -> (Layer, Arc<AppHost>) {
        crate::stack(root, Some(config.to_path_buf()))
    }

    /// The shipped layer, with nowhere to persist to: the host has no config
    /// home, so a persist is refused rather than writing into the
    /// repository's own runtime tree.
    fn booted() -> (Layer, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(None));
        let runtime = boot(Some(&tree()), &host);
        (Layer::new(runtime), host)
    }

    /// Asks the live keymap about a sequence, the way the loop does.
    /// The sentence inside a `(#refused "…")` an evaluation answered with, or
    /// [`None`] if it answered anything else.
    ///
    /// A capability refused *inside* a form does not refuse the `eval` around
    /// it — `phosphor-steel`'s door turns the refusal into a value, so the
    /// REPL can print it under what you typed. Matching on `Outcome::Refused`
    /// here would therefore pass for a form that never ran at all.
    fn refused(outcome: &Outcome) -> Option<&str> {
        let Outcome::Done(receipt) = outcome else {
            return None;
        };
        let Value::List(parts) = &receipt.value else {
            return None;
        };
        match parts.as_slice() {
            [Value::Text(tag), Value::Text(why)] if tag == "#refused" => Some(why),
            _ => None,
        }
    }

    fn resolved(layer: &mut Layer, spelled: &str) -> Resolution {
        let keys = parse_seq(spelled).expect("a spelling these tests wrote");
        layer.resolve(Scope::Normal, &keys)
    }

    /// A writable copy of the shipped layer.
    fn copy_of_the_layer(name: &str) -> PathBuf {
        let root = scratch(name);
        copy_scm_tree(&tree(), &root);
        root
    }

    /// Every `.scm` under `from`, into `to`, **preserving relative paths**.
    ///
    /// Flat was enough until `T037` added `runtime/languages/`: `init.scm`'s
    /// `phosphor/boot-files` names twelve entries under it, and a flat copy
    /// stages an `init.scm` asking for files nobody put there.
    /// `a_persisted_rebind_survives_the_next_boot` failed that way, reporting a
    /// boot fault rather than the missing copy.
    fn copy_scm_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a scratch tree");
        for entry in std::fs::read_dir(from).expect("the shipped layer") {
            let entry = entry.expect("a readable entry");
            let path = entry.path();
            if path.is_dir() {
                copy_scm_tree(&path, &to.join(entry.file_name()));
            } else if path.extension().is_some_and(|ext| ext == "scm") {
                std::fs::copy(&path, to.join(entry.file_name())).expect("copy");
            }
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("phosphor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch tree");
        path
    }

    /// One `Editing` over `text` with the cursor at the end of it, laid out in
    /// a `width`-column area — the shape the completion and hover gates read.
    fn typed(text: &str, width: u16) -> Editing {
        let mut editing = editing(text);
        editing.area = Rect::new(0, 0, width, 24);
        editing.editor.set_cursor(text.chars().count());
        editing
    }

    /// One `Editing` over `text`, with nothing to save to.
    fn editing(text: &str) -> Editing {
        Editing::new(
            buffer(
                "text",
                text,
                &super::builtin("phosphor-dark").expect("a shipped theme"),
            )
            .expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        )
    }

    /// **The second producer, reaching the buffer.** A posted event is applied
    /// by the same `Editing::act` a keystroke reaches — there is no second
    /// interpreter — so this is the whole of what the loop does with one.
    #[test]
    fn a_posted_action_lands_through_the_arm_a_key_would_reach() {
        use phosphor_core::input::text::Text as _;

        let mut editing = editing("hello");
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::Buffer(phosphor_core::action::BufferAction::Insert {
                    at: Position { line: 1, column: 1 },
                    text: "// ".to_owned(),
                }),
            },
        );
        assert_eq!(note, None, "a posted Action that applied says nothing");
        assert_eq!(editing.text().line(1).as_deref(), Some("// hello"));
    }

    /// **A producer does not move the viewport you are looking at.**
    ///
    /// `deliver` goes through `Editing::act` and not `Editing::apply`, so no
    /// reveal fires and `View::Scroll` — invariant 3's single writer — stays
    /// the user's. Under the version that called `apply`, an edit posted at
    /// line 1 snapped a viewport 70 lines down back to the top, because
    /// `moves_cursor` is true for `Buffer::Insert`.
    #[test]
    fn a_posted_edit_does_not_scroll_the_viewport_the_user_is_looking_at() {
        let text: String = (1..=100).map(|line| format!("line {line}\n")).collect();
        let mut editing = editing(&text);
        editing.area = Rect::new(0, 0, 80, 10);
        // The user's own turn at the one writer, so there is a viewport worth
        // not moving.
        let _ = editing.act(&Action::View(phosphor_core::action::ViewAction::Scroll {
            request: phosphor_core::request::ScrollRequest::RevealRow { row: 80, margin: 0 },
            pane: phosphor_core::request::PaneRef::Focused {},
        }));
        let looking_at = editing.editor.get_offset_y();
        assert!(
            looking_at > 0,
            "this test is about a scrolled viewport and needs one"
        );

        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::Buffer(phosphor_core::action::BufferAction::Insert {
                    at: Position { line: 1, column: 1 },
                    text: "// ".to_owned(),
                }),
            },
        );
        assert_eq!(note, None, "the edit landed");
        assert_eq!(
            editing.editor.get_offset_y(),
            looking_at,
            "a producer edited the buffer and left the viewport where the user put it"
        );
    }

    /// **The policy gate, on the capability the loop has a live arm for.**
    ///
    /// `quit` is rated `Deny` — *"refused unless a rule in `init.scm` opens
    /// it"* — and `Editing::act`'s arm for it sets `quit`, after which the loop
    /// breaks. Without the gate, any future producer could close the editor
    /// over unsaved work by posting one Action, since `force` skips
    /// `WouldLoseWork` too.
    #[test]
    fn a_posted_action_the_mcp_door_denies_is_refused_rather_than_applied() {
        let mut editing = editing("hello");
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::App(phosphor_core::action::AppAction::Quit { force: true }),
            },
        );
        assert_eq!(
            note.as_deref(),
            Some("lsp: denied to a producer — only the keyboard asks for this"),
        );
        assert!(!editing.quit, "a producer did not get to end the session");
    }

    /// The middle rating, and the one an LSP client meets first: `T036`'s
    /// `apply-workspace-edit` is `Ask`, so it waits for `T060`'s queue rather
    /// than editing the buffer on a server's say-so. The producer is told what
    /// it is waiting for, which is the same contract a missing arm gets.
    #[test]
    fn a_posted_action_the_mcp_door_asks_about_waits_for_the_ask_queue() {
        use phosphor_core::input::text::Text as _;

        let mut editing = editing("hello");
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::Lsp(phosphor_core::action::LspAction::ApplyWorkspaceEdit {
                    files: Vec::new(),
                }),
            },
        );
        assert_eq!(
            note.as_deref(),
            Some("lsp: needs an ask first — T060 builds the queue"),
        );
        assert_eq!(
            editing.text().line(1).as_deref(),
            Some("hello"),
            "nothing was applied"
        );
    }

    /// What an LSP client posts on day one, and what it gets back today:
    /// `ingest-diagnostics` is registered with `T040` on its row and has no arm
    /// yet, so the loop answers with **the task that builds it and the producer
    /// that asked** rather than dropping it.
    ///
    /// This is the extension point tested end to end without an LSP client —
    /// which is the reason the queue is dependency-free.
    #[test]
    fn a_posted_action_with_no_arm_names_its_task_and_its_producer() {
        let mut editing = editing("hello");
        // `refresh-vcs` is `Allow`, so it reaches `Editing::act` and falls to
        // the `_` arm — which is the case this test is about. It used to be
        // `ingest-diagnostics`; that one has an arm now (`T040`), and the test
        // below is what took its place.
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "vcs",
                action: Action::Vcs(phosphor_core::action::VcsAction::RefreshVcs {}),
            },
        );
        assert_eq!(
            note.as_deref(),
            Some("vcs: not built yet — T071 builds it"),
            "a producer whose Action has no arm is told which task builds it"
        );
    }

    /// `T040`, from the producer's end: a server publishes and the set is in
    /// the store the gutter and the `diagnostics` query both read.
    ///
    /// **Through `deliver`, not through `act`**, which is the whole point —
    /// `ingest-diagnostics` is the one `Lsp` verb rated `Allow`, so it is the
    /// one a producer may reach, and this exercises that rating rather than
    /// asserting it.
    #[test]
    fn a_published_diagnostic_reaches_the_store_the_gutter_reads() {
        let mut editing = editing("fn main() {}\n");
        let path = PathBuf::from("/tmp/retry.rs");
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::Lsp(phosphor_core::action::LspAction::IngestDiagnostics {
                    path: path.clone(),
                    diagnostics: vec![phosphor_core::request::Diagnostic {
                        span: Span {
                            start: Position { line: 1, column: 4 },
                            end: Position { line: 1, column: 8 },
                        },
                        severity: Severity::Trouble,
                        message: "expected Duration, found u128".to_owned(),
                        source: Some("rust-analyzer".to_owned()),
                    }],
                }),
            },
        );
        assert_eq!(note, None, "an applied Action says nothing");
        let held = editing.diagnostics.of(&path);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].message, "expected Duration, found u128");
    }

    /// The other half of the same rating, and the reason `deliver` has a policy
    /// check at all: the three verbs that drive the completion float are
    /// `Deny`, so a producer that is not answering a request this editor made
    /// cannot open one.
    #[test]
    fn a_producer_cannot_open_a_completion_float_nobody_asked_for() {
        let mut editing = editing("hello");
        let note = deliver(
            &mut editing,
            &super::events::Posted {
                source: "lsp",
                action: Action::Lsp(phosphor_core::action::LspAction::IngestCompletions {
                    items: vec![WireCompletion {
                        label: "default()".to_owned(),
                        detail: None,
                        documentation: Vec::new(),
                        insert: "default()".to_owned(),
                        kind: None,
                        source: None,
                        deprecated: false,
                    }],
                    at: Position { line: 1, column: 1 },
                    buffer: None,
                }),
            },
        );
        assert_eq!(
            note.as_deref(),
            Some("lsp: denied to a producer — only the keyboard asks for this")
        );
        assert!(
            editing.completion.is_none(),
            "an unsolicited answer must not raise the float"
        );
    }

    /// …and the routing that lets the *answer to the user's own request*
    /// through anyway.
    ///
    /// [`Outstanding::answers`] is the whole difference, and it is matched per
    /// lookup kind: a hover answer arriving while the editor waits only for
    /// completions is still an unsolicited push. Without the third assertion
    /// this could be *"is anything outstanding"* and pass.
    #[test]
    fn an_answer_is_told_from_a_push_by_which_lookup_is_outstanding() {
        let completions = ingest_completions();
        let hover = Action::Lsp(phosphor_core::action::LspAction::IngestHover {
            prose: Vec::new(),
            at: Position { line: 1, column: 1 },
            buffer: None,
        });

        let mut outstanding = Outstanding::default();
        assert!(
            !outstanding.answers(&completions),
            "nothing outstanding means nothing is an answer"
        );

        outstanding.sent(Lookup::Completion);
        assert!(
            !outstanding.answers(&hover),
            "a hover is not the answer to a completion request"
        );
        assert!(outstanding.answers(&completions));

        outstanding.sent(Lookup::Hover);
        assert!(
            !outstanding.answers(&completions),
            "a completion list is not the answer to a hover"
        );
        assert!(outstanding.answers(&hover));
    }

    /// **The `CP-4` defect, as a unit.** The insert-mode trigger asks once per
    /// edit; while it held one slot, the *second* request's answer replaced the
    /// first's and every superseded answer fell through to [`deliver`] to be
    /// refused on the statusline — `lsp: denied to a producer` painted over the
    /// file, while typing, against a real server.
    ///
    /// Every request is owed an answer and each answer takes one off the count,
    /// so the run of three below is silent and the fourth arrival — which
    /// nothing asked for — is not.
    #[test]
    fn every_request_is_owed_an_answer_and_a_superseded_one_is_still_an_answer() {
        let mut outstanding = Outstanding::default();
        for _ in 0..3 {
            outstanding.sent(Lookup::Completion);
        }
        assert!(outstanding.awaiting(Lookup::Completion));
        for arrival in 1..=3 {
            assert!(
                outstanding.answers(&ingest_completions()),
                "answer {arrival} of 3 is one this editor asked for"
            );
        }
        assert!(
            !outstanding.awaiting(Lookup::Completion),
            "and then nothing is owed"
        );
        assert!(
            !outstanding.answers(&ingest_completions()),
            "a fourth arrival is a push, and `deliver` refuses it"
        );
    }

    /// **Every state a server can be in says something, or nothing, on
    /// purpose** — and the one that matters is the failure, because until this
    /// existed `ServerState::Crashed` was read by nothing in the binary and a
    /// server that could not start was indistinguishable from a language that
    /// declares none.
    ///
    /// The pty test presses no key and watches the chip change; this is where
    /// the OS's own sentence is asserted, because §11 sheds it on a terminal
    /// too narrow to hold both it and a long path.
    #[test]
    fn every_server_state_says_what_the_statusline_should_say_about_it() {
        use phosphor_buffer::lsp::{Failure, ServerIdentity, ServerState};

        let named = Some("rust-analyzer");
        assert_eq!(server_chip(&ServerState::NotStarted, named), None);
        assert_eq!(server_chip(&ServerState::Stopped, named), None);
        assert_eq!(
            server_chip(&ServerState::Starting, named).as_deref(),
            Some("rust-analyzer …")
        );
        // `7c`, and the name is the server's own rather than the command that
        // was run — a wrapper script still speaks for rust-analyzer.
        assert_eq!(
            server_chip(
                &ServerState::Ready(ServerIdentity {
                    name: "rust-analyzer".to_owned(),
                    version: Some("1.97.1".to_owned()),
                }),
                Some("ra-wrapper"),
            )
            .as_deref(),
            Some("rust-analyzer ✓")
        );
        assert_eq!(
            server_chip(
                &ServerState::Crashed(Failure::Spawn(
                    "No such file or directory (os error 2)".to_owned()
                )),
                named,
            )
            .as_deref(),
            Some("rust-analyzer ✗ could not start: No such file or directory (os error 2)"),
            "the OS's own words, which is what `Failure::Spawn` carries them for"
        );
        // A language that declares no server has nothing to say about one.
        assert_eq!(server_chip(&ServerState::Starting, None), None);
    }

    /// An empty completion answer, which is all these two need.
    fn ingest_completions() -> Action {
        Action::Lsp(phosphor_core::action::LspAction::IngestCompletions {
            items: Vec::new(),
            at: Position { line: 1, column: 1 },
            buffer: None,
        })
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

    /// One buffer and one session, so a test can type a sequence and look at
    /// what it did. Everything here is the shipping path: the shipped keymap
    /// answers first on every key, and `Editing` applies what comes out.
    struct Typed {
        editing: Editing,
        machine: Machine,
        seed: Table,
        layer: Layer,
    }

    impl Typed {
        fn on(text: &str) -> Self {
            Self::with_dirty(text, false)
        }

        /// The same buffer with the loop's dirty flag already raised, which is
        /// what a `quit` that is not forced reads. `Typed::on`'s flag is `false`
        /// and stays false — nothing in these tests owns the other end of that
        /// `Rc` — so a test about *refusing to leave* has to set it here.
        fn unsaved(text: &str) -> Self {
            Self::with_dirty(text, true)
        }

        fn with_dirty(text: &str, dirty: bool) -> Self {
            let theme = super::builtin("phosphor-dark").expect("a shipped theme");
            let mut editing = Editing::new(
                buffer("text", text, &theme).expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(dirty)),
            );
            editing.area = Rect::new(0, 0, 80, 24);
            let (layer, _host) = booted();
            Self {
                editing,
                machine: Machine::new(),
                seed: Table::new(),
                layer,
            }
        }

        /// Types a sequence in the notation `runtime/keymaps.scm` is written
        /// in, through the same decode-and-feed the loop does.
        fn keys(&mut self, spelled: &str) -> &mut Self {
            for key in parse_seq(spelled).expect("a spelling these tests wrote") {
                Session {
                    machine: &mut self.machine,
                    layer: &mut self.layer,
                    seed: &mut self.seed,
                    editing: &mut self.editing,
                }
                .key(key);
            }
            self
        }

        fn content(&self) -> String {
            self.editing.editor.get_content()
        }
    }

    /// **`ZZ` on a buffer with no name says what would give it one** — found by
    /// hand at `CP-4`.
    ///
    /// `ZZ` is one row and two Actions, `save-buffer` then `quit`, and both
    /// refuse here: the buffer has no file and the work is unsaved. The notice
    /// slot holds one sentence, and it used to hold the **last** — *"unsaved
    /// work — force it or save first"*, which tells a person to do the thing
    /// they just tried and never names the command that would do it.
    ///
    /// The same two Actions through the ex line ([`submit_ex`], a `find_map`)
    /// have always answered with the first, so `:wq` and `ZZ` were two doors
    /// onto one list giving different answers. This is the keystroke door
    /// agreeing.
    ///
    /// **This bites:** drop the `&& !said` from [`Session::key`]'s refusal
    /// write and this reads `unsaved work — force it or save first`.
    #[test]
    fn zz_with_nothing_to_write_to_names_the_write_and_not_the_quit() {
        let mut typed = Typed::unsaved("precious");
        typed.keys("ZZ");
        assert_eq!(
            typed.editing.refused.as_ref().map(answer::why).as_deref(),
            Some("no file name — :write <path>"),
            "the cause, not the consequence"
        );
        assert!(!typed.editing.quit, "and it is still open to be saved in");
    }

    #[test]
    fn the_case_keys_edit_through_the_shipped_keymap() {
        // `R2`'s sibling: the vocabulary agent added `Buffer::SetCase` and the
        // machine emits it for `~`, `gu` and `gU`; without an arm here all
        // three fall to `NotYetImplemented` and the keys do nothing.
        assert_eq!(
            Typed::on("hello world").keys("gUw").content(),
            "HELLO world"
        );
        assert_eq!(
            Typed::on("HELLO world").keys("guw").content(),
            "hello world"
        );
        // `~` is fused with a right motion, so it recases one character and
        // steps — which is what makes `~~~` walk a word.
        assert_eq!(Typed::on("abc").keys("~~").content(), "ABc");
    }

    #[test]
    fn the_undone_branch_survives_a_divergent_edit() {
        // **The whole reason `T029` exists.** The fork's history truncates on
        // divergence (`vendor/ratatui-code-editor/src/history.rs:19-22`), so
        // under it this sequence destroys the `A` permanently. The tree keeps
        // it: node 1 is still there, and `redo` follows the branch just taken
        // rather than the abandoned one.
        let mut typed = Typed::on("base");
        typed.keys("iA<esc>");
        assert_eq!(typed.content(), "Abase");
        typed.keys("u");
        assert_eq!(typed.content(), "base");
        typed.keys("iB<esc>");
        assert_eq!(typed.content(), "Bbase");

        // Three states reachable, not two: root, the `A` branch, the `B`
        // branch. A stack would have three nodes only if nothing was thrown
        // away.
        assert_eq!(
            typed.editing.timeline.tree.node_count(),
            3,
            "the undone branch is still in the tree"
        );
        typed.keys("u");
        assert_eq!(typed.content(), "base");
        typed.keys("<C-r>");
        assert_eq!(
            typed.content(),
            "Bbase",
            "redo takes the branch last walked, which is the divergent one"
        );
    }

    #[test]
    fn an_insert_session_is_one_undo_step_and_esc_is_the_boundary() {
        // The machine's rule, honoured here: `History::CommitUndoGroup` is
        // emitted on leaving insert mode and is the only thing that closes a
        // group, so `u` after typing three characters takes all three.
        let mut typed = Typed::on("");
        typed.keys("iabc<esc>");
        assert_eq!(typed.content(), "abc");
        typed.keys("u");
        assert_eq!(
            typed.content(),
            "",
            "one `<esc>`, one group, one undo — not one per character"
        );
    }

    #[test]
    fn a_fold_closes_and_opens_at_the_cursor() {
        // The `View` arms, over a real tree-sitter fold. The ranges are the
        // language's own — `langs/rust/folds.scm`, read by the fork's
        // `fold_query` — so an empty range list here is a grammar problem and
        // not a wiring one, which is why the assertion says so.
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let mut editing = Editing::new(
            buffer(
                "rust",
                "fn outer() {\n    let a = 1;\n    let b = 2;\n}\n",
                &theme,
            )
            .expect("a buffer"),
            None,
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );
        editing.area = Rect::new(0, 0, 80, 24);
        assert!(
            !editing.editor.code_ref().fold_ranges().is_empty(),
            "the bundled rust grammar produces fold ranges"
        );

        let at_cursor = phosphor_core::request::Target::Cursor {};
        let outcome = editing.apply(&Action::View(super::ViewAction::SetFold {
            target: at_cursor,
            state: phosphor_core::request::FoldState::Toggle,
        }));
        assert!(matches!(outcome, Outcome::Done(_)));
        assert!(
            editing.editor.fold_hidden_lines(0).is_some(),
            "za closes the fold the cursor is in"
        );

        let outcome = editing.apply(&Action::View(super::ViewAction::UnfoldAll {}));
        assert!(matches!(outcome, Outcome::Done(_)));
        assert!(
            editing.editor.fold_hidden_lines(0).is_none(),
            "zR opens everything"
        );
    }

    #[test]
    fn which_key_answers_for_whatever_prefix_is_half_typed() {
        // `R17`, at the seam the loop draws from. The pty test presses the key;
        // this one holds the composition still and asks what it answered, so a
        // change in the shipped table is a legible failure rather than a blank
        // frame.
        let mut typed = Typed::on("x");
        assert!(
            super::under(&mut typed.layer, &typed.machine).is_empty(),
            "nothing is half-typed, so there is nothing to show"
        );
        typed.keys("<space>");
        let hints = super::under(&mut typed.layer, &typed.machine);
        assert!(
            hints.iter().any(|hint| hint.key.0 == "<space>c"),
            "SPC c is one key under the leader: {hints:?}"
        );
        assert!(
            hints.iter().all(|hint| {
                parse_seq(hint.key.0.strip_prefix("<space>").unwrap_or_default())
                    .is_some_and(|keys| keys.len() == 1)
            }),
            "only one key past what has been typed: {hints:?}"
        );
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

    /// The floor a layer that never mentions `completion-min-chars` gets, and
    /// the three ways a layer can mean something other than a count.
    ///
    /// **`runtime/init.scm` sets this option, so the pty tests can never see
    /// [`COMPLETION_MIN_CHARS_DEFAULT`]** — a layer's value always wins, which
    /// is the point of the option. This is the one place the constant is
    /// reachable, and it is worth reaching: a custom runtime tree with no
    /// `defaults` section is exactly the case `CP-4` reported, and it would
    /// come back silently.
    #[test]
    fn the_completion_floor_falls_back_and_reads_only_counts() {
        let host = AppHost::new(None);
        // **The literal, not the constant.** Asserting a constant against
        // itself is a tautology, and the number is the decision — two, argued
        // at `COMPLETION_MIN_CHARS_DEFAULT`. Changing it is a decision, so it
        // should cost a line here.
        assert_eq!(
            completion_floor(&host),
            2,
            "a layer that never sets it gets the shipped floor"
        );

        let set = |value: Value| {
            drop(ask(
                &host,
                Action::Runtime(RuntimeAction::SetOption {
                    key: COMPLETION_MIN_CHARS.to_owned(),
                    value,
                }),
            ));
        };

        set(Value::Int(4));
        assert_eq!(completion_floor(&host), 4);

        // A negative floor is *no* floor, not the default: it is what a person
        // writing `-1` into a minimum is asking for, and substituting `2` would
        // be the editor overruling a setting it read.
        set(Value::Int(-1));
        assert_eq!(completion_floor(&host), 0);

        // The wrong case reads as unset, the way `flag` treats a number. There
        // is nowhere to report a type error to.
        set(Value::Bool(true));
        assert_eq!(completion_floor(&host), COMPLETION_MIN_CHARS_DEFAULT);

        // **And the shipped layer spells it the same way.** The option is a
        // string on both sides of the barrier, so renaming the constant without
        // renaming it in `runtime/init.scm` would leave the layer setting a key
        // nothing reads and the floor silently back at its Rust default —
        // which is a silent revert of a `CP-4` finding rather than a build
        // failure.
        let init = std::fs::read_to_string(tree().join("init.scm"))
            .expect("the shipped layer is where the workspace keeps it");
        assert!(
            init.contains(&format!("(set-option! \"{COMPLETION_MIN_CHARS}\"")),
            "runtime/init.scm does not set {COMPLETION_MIN_CHARS}"
        );
    }

    // -- `T104` — what one indent level is ----------------------------------

    /// A `Languages` table with one declaration, so a test can say what the
    /// language declares without booting a layer.
    fn declaring(language: &str, indent: Option<&str>) -> (Languages, LanguageId) {
        let mut languages = Languages::new(["rust"]);
        let id = LanguageId(language.to_owned());
        languages
            .declare(
                id.clone(),
                phosphor_core::request::LanguageSpec {
                    extensions: vec!["zz".to_owned()],
                    grammar: None,
                    lsp_command: Vec::new(),
                    comment_prefix: None,
                    indent: indent.map(str::to_owned),
                },
            )
            .expect("a declaration these tests wrote");
        (languages, id)
    }

    /// The three answers `>` can get, and which one wins.
    ///
    /// **The precedence is the assertion.** Every line below sets `tab-width`
    /// to 8, so a resolver that ignored the declaration would answer eight
    /// spaces in all three cases and a resolver that ignored the option would
    /// answer four — neither passes.
    #[test]
    fn a_declaration_beats_the_option_and_the_option_beats_the_default() {
        let host = AppHost::new(None);
        let set = |key: &str, value: Value| {
            drop(ask(
                &host,
                Action::Runtime(RuntimeAction::SetOption {
                    key: key.to_owned(),
                    value,
                }),
            ));
        };

        // Nothing set anywhere: the documented default, and the literal rather
        // than the constant, because the number is the decision.
        let (languages, id) = declaring("toy", None);
        assert_eq!(indent_style(&host, &languages, Some(&id)).unit, "    ");

        // The option alone.
        set(TAB_WIDTH, Value::Int(8));
        assert_eq!(indent_style(&host, &languages, Some(&id)).unit, "        ");

        // `expand-tab` off is a real tab, whatever the width — the width is
        // then how wide it *draws*, which is the tab_width the fork is told.
        set(EXPAND_TAB, Value::Bool(false));
        let style = indent_style(&host, &languages, Some(&id));
        assert_eq!(style.unit, "\t");
        assert_eq!(style.tab_width, 8);

        // And a declaration beats both of them, in either direction: a
        // narrower unit than the global, in a build set to tabs.
        let (declared, declared_id) = declaring("toy", Some("  "));
        assert_eq!(
            indent_style(&host, &declared, Some(&declared_id)).unit,
            "  "
        );

        // A file no declaration claims takes the global answer rather than
        // falling over — second tier is a normal state.
        assert_eq!(indent_style(&host, &declared, None).unit, "\t");
    }

    /// **`<tab>` advances to a stop; it does not type a fixed count.** Two
    /// characters in, a four-cell unit types two spaces — the difference
    /// between a tabstop and a substitution, and the reason a file indented by
    /// pressing this key draws the same as one indented with `\t`.
    #[test]
    fn a_tab_press_types_the_cells_left_to_the_next_stop() {
        let style = IndentStyle {
            unit: "    ".to_owned(),
            tab_width: 4,
        };
        assert_eq!(style.typed_at(0), "    ");
        assert_eq!(style.typed_at(1), "   ");
        assert_eq!(style.typed_at(2), "  ");
        assert_eq!(style.typed_at(3), " ");
        assert_eq!(style.typed_at(4), "    ");

        // A tab unit types one character and lets the renderer do it.
        let tabs = IndentStyle {
            unit: "\t".to_owned(),
            tab_width: 4,
        };
        assert_eq!(tabs.typed_at(0), "\t");
        assert_eq!(tabs.typed_at(3), "\t");

        // A two-space unit stops every two columns, not every four: one press
        // is one *level*, and the level is what was declared.
        let narrow = IndentStyle {
            unit: "  ".to_owned(),
            tab_width: 4,
        };
        assert_eq!(narrow.typed_at(1), " ");
        assert_eq!(narrow.typed_at(2), "  ");
    }

    /// **`<tab>` through the shipped keymap**, which is the half a unit test of
    /// [`IndentStyle`] cannot reach: `runtime/keymaps.scm` has to bind the key
    /// in the insert scope, or it falls through to `Machine::insert_key`'s
    /// literal `"\t"` and this reads one character instead of four.
    #[test]
    fn tab_types_one_indent_level_through_the_shipped_keymap() {
        assert_eq!(Typed::on("").keys("i<tab>x").content(), "    x");
        // Two characters in, the stop is two cells away — not another four.
        assert_eq!(Typed::on("ab").keys("A<tab>x").content(), "ab  x");
    }

    /// **Cells, not characters.** `漢` is one `char` and two columns, so the
    /// stop after it is two cells away. A column counted in `char`s would type
    /// three spaces and leave the `x` at column 4 rather than 4 cells in.
    #[test]
    fn a_tab_after_a_wide_character_advances_by_cells() {
        assert_eq!(Typed::on("漢").keys("A<tab>x").content(), "漢  x");
    }

    /// **`R` is still vim's `R`, for the third key this window has had to teach
    /// it.** `Scope::of` folds `EditMode::Replace` into the insert scope, so
    /// the `<tab>` row binds there too, and
    /// [`Editing::insert_indent`] spliced unconditionally — which made `R` into
    /// `i` exactly as the `<space>` and `<cr>` fall-through had before it.
    ///
    /// The two cases are the whole behaviour: a tab spends the cells left to
    /// the stop, and consumes **one character** doing it. Measured against
    /// `nvim -u NONE` with `set expandtab tabstop=4 softtabstop=0` this
    /// session: `R<Tab>` over `abcdefgh` gives `····bcdefgh` and `Rx<Tab>`
    /// gives `x···cdefgh`, both reproduced below.
    ///
    /// **This bites:** drop the `EditMode::Replace` arm from `insert_indent`
    /// and the first case keeps its `a` (`    abcdefgh`) and the second its `b`
    /// (`x   bcdefgh`) — the line grows by a whole level and nothing is
    /// replaced, which is `i`.
    #[test]
    fn a_tab_in_replace_mode_overwrites_the_character_it_lands_on() {
        assert_eq!(
            Typed::on("abcdefgh").keys("R<tab>").content(),
            "    bcdefgh"
        );
        // And from column 1, where a stop and a fixed four cannot be confused:
        // three spaces, one character eaten.
        assert_eq!(
            Typed::on("abcdefgh").keys("Rx<tab>").content(),
            "x   cdefgh"
        );
    }

    /// **A tab at the end of a line in `R` appends rather than eating the
    /// newline**, which is [`Editing::line_end`]'s clamp and the same rule
    /// [`Editing::accept`] follows. Without it the `\n` is the character under
    /// the cursor and `R<Tab>` at the end of a line joins it to the next one.
    #[test]
    fn a_tab_in_replace_mode_at_the_end_of_a_line_keeps_the_newline() {
        // `Rxy` overwrites both characters and leaves the caret *on* the
        // newline, which is the only place the clamp is load-bearing. Without
        // it the tab eats the `\n` and joins the two lines: `xy  cd`.
        assert_eq!(
            Typed::on("ab\ncd\n").keys("Rxy<tab>").content(),
            "xy  \ncd\n"
        );
    }

    /// **`>` shifts by the same unit `<tab>` types**, which is the whole of
    /// *"the unit comes from something a user set"*: both read
    /// [`Editing::indent_style`], and nothing reads `Code::indent` any more.
    ///
    /// `>` is pressed here for the first time in this build's history — the arm
    /// and the binding both existed and `TASKS.md` records that no test in
    /// `crates/phosphor/tests/` or `crates/phosphor-core/tests/` had ever typed
    /// one.
    #[test]
    fn the_shift_operator_uses_the_unit_the_layer_set() {
        let mut typed = Typed::on("a\nb\n");
        typed.editing.indent_style = IndentStyle {
            unit: "  ".to_owned(),
            tab_width: 4,
        };
        assert_eq!(typed.keys(">>").content(), "  a\nb\n");
        // And `<` takes the same unit back off, so the pair is symmetric under
        // a setting neither of them names.
        assert_eq!(typed.keys("<<").content(), "a\nb\n");

        // A tab unit writes a tab, which is what a `go` declaration would get.
        let mut tabs = Typed::on("a\n");
        tabs.editing.indent_style = IndentStyle {
            unit: "\t".to_owned(),
            tab_width: 4,
        };
        assert_eq!(tabs.keys(">>").content(), "\ta\n");
    }

    /// **The option names are one string on both sides of the barrier.**
    /// Renaming a constant without renaming it in `runtime/init.scm` leaves the
    /// layer setting a key nothing reads and the width silently back at its
    /// Rust default — the same silent-revert shape
    /// `the_completion_floor_falls_back_and_reads_only_counts` guards, and the
    /// same one `CP-4` reported.
    #[test]
    fn the_shipped_layer_sets_the_indent_options_this_file_reads() {
        let init = std::fs::read_to_string(tree().join("init.scm"))
            .expect("the shipped layer is where the workspace keeps it");
        for option in [TAB_WIDTH, EXPAND_TAB] {
            assert!(
                init.contains(&format!("(set-option! \"{option}\"")),
                "runtime/init.scm does not set {option}"
            );
        }
        // And the key is bound, or none of the above is reachable by typing.
        //
        // **Two substrings rather than the whole row**, and that is the repair
        // this assertion needed rather than a loosening. It read the row
        // verbatim — `(list "<tab>" (key/run (key/cmd "insert-indent"))` — so
        // it broke the moment `<tab>` grew the completion fall-through
        // (`OPEN-QUESTIONS.md` §38, re-ruled at `CP-4`) even though the key
        // still types an indent level, which is the one thing this test is
        // about. What it can honestly claim from a file's text is that the key
        // is bound and that the indent verb is what it reaches for; that the
        // Action actually comes out is
        // `phosphor-steel`'s `tab_in_insert_steps_the_completion_list_and_carries_the_indent_fall_through`,
        // which presses the key, and the host running it is `loop_pty.rs`'s
        // `tab_with_no_completion_list_open_types_one_indent_level`.
        let keymaps = std::fs::read_to_string(tree().join("keymaps.scm"))
            .expect("the shipped keymap is where the workspace keeps it");
        assert!(
            keymaps.contains(r#"(list "<tab>""#),
            "runtime/keymaps.scm does not bind <tab>"
        );
        // The `"otherwise"` is carried deliberately: `key/capability`'s own doc
        // comment shows `(key/capability "insert-indent")` as its example, so
        // the bare form matches the *prose* and would pass with every binding
        // deleted. Matching the argument position is what makes this about a
        // row.
        assert!(
            keymaps.contains(r#""otherwise" (key/capability "insert-indent")"#),
            "runtime/keymaps.scm binds <tab> without reaching insert-indent"
        );
    }

    /// **A scripted selection does not anchor the next visual mode** (`CP-4`
    /// review).
    ///
    /// [`Editing::selection_from`] was cleared only by `ClearSelection`, and
    /// `Machine::select` emits one when it *leaves* visual mode, never when it
    /// enters — sound for the machine's own stream and wrong for the other
    /// three doors, because `select-range` is a declared capability (`T026`)
    /// that Steel, MCP and `--do` can all reach. So a scripted selection left an
    /// anchor behind that the next `v` inherited and the next motion extended
    /// from.
    ///
    /// This is the reported sequence, applied through `Editing::act` because
    /// that is the one arm all four doors land in: a scripted `1..5`, a cursor
    /// move, the degenerate range `v` sends, and one `l`. Both halves are
    /// asserted — the second `SelectRange` re-anchors, and a `SelectRange` that
    /// really is an extension of the live one does **not**.
    #[test]
    fn a_scripted_selection_does_not_anchor_the_next_visual_mode() {
        use phosphor_core::action::MotionAction;
        use phosphor_core::request::{Motion, SelectionKind};

        let span = |from: u32, to: u32| Span {
            start: Position {
                line: 1,
                column: from,
            },
            end: Position {
                line: 1,
                column: to,
            },
        };
        let select = |from, to| {
            Action::Motion(MotionAction::SelectRange {
                span: span(from, to),
                kind: SelectionKind::Char,
            })
        };

        let mut editing = typed("abcdefghij", 120);
        // The scripted door: cols 1–4, and nothing ever clears it.
        drop(editing.act(&select(1, 5)));
        drop(editing.act(&Action::Motion(MotionAction::SetCursor {
            position: Position { line: 1, column: 7 },
            buffer: None,
        })));
        // What `v` sends: the degenerate range under the cursor.
        drop(editing.act(&select(7, 8)));
        drop(editing.act(&Action::Motion(MotionAction::ExtendSelection {
            motion: Motion::CharRight,
            count: 1,
        })));
        let selection = editing.editor.get_selection().expect("a selection");
        assert_eq!(
            (selection.start, selection.end),
            (6, 8),
            "the `v` re-anchored under the cursor, not at the scripted anchor"
        );

        // And the other direction: a range that still contains the live anchor
        // is the same selection growing, so the fixed end must not move.
        let mut editing = typed("abcdefghij", 120);
        drop(editing.act(&select(3, 4)));
        drop(editing.act(&select(3, 9)));
        drop(editing.act(&Action::Motion(MotionAction::ExtendSelection {
            motion: Motion::CharRight,
            count: 1,
        })));
        let selection = editing.editor.get_selection().expect("a selection");
        assert_eq!(
            (selection.start, selection.end),
            (2, 10),
            "an extension keeps the anchor `v` put down"
        );
    }

    /// **The floor is not the only gate, and `.` is why** (`CP-4` review).
    ///
    /// [`Editing::prefix_len`] counts word characters, so it is `0` right after
    /// a `.` — and the shipped floor of two therefore hid member completion,
    /// the most common completion moment in every dotted language, behind
    /// `<C-x>`. [`Editing::after_trigger`] is the other half, and it asks the
    /// *server's* list rather than one written here.
    ///
    /// Four cases, and the last two are the ones a naive "any punctuation"
    /// rule gets wrong: a server that advertises nothing behaves exactly as it
    /// did, and a character the server did not name is not a trigger.
    #[test]
    fn a_trigger_character_asks_where_an_identifier_prefix_would_not() {
        let dot = [".".to_owned(), "::".to_owned()];
        let dotted = typed("foo.", 120);
        assert_eq!(dotted.prefix_len(), 0, "a `.` is not an identifier");
        assert!(dotted.after_trigger(&dot), "`foo.` is the server's `.`");

        let named = typed("foo.ba", 120);
        assert_eq!(named.prefix_len(), 2);
        assert!(
            !named.after_trigger(&dot),
            "and two letters later it is not a trigger any more — the floor is"
        );

        // Multi-character triggers are compared as suffixes, so `::` is one
        // trigger and not two `:` keystrokes.
        let scoped = typed("RetryPolicy::", 120);
        assert!(scoped.after_trigger(&dot));

        // A server that advertises none leaves the gate exactly as it was, and
        // a character it did not name is not a trigger.
        assert!(!scoped.after_trigger(&[]));
        assert!(!scoped.after_trigger(&["->".to_owned()]));
    }

    /// **Prose is wrapped by the host, at the width the float will give it**
    /// (`CP-4` review).
    ///
    /// §11 is *"nothing ever wraps"* and `phosphor-ui` honours it literally, so
    /// a server's paragraph arrived as one very long line and the float
    /// truncated roughly 40% of it with an `⋯` no key could reveal — the
    /// published `anchored_wrap_cols` had **no** caller. Stated over the
    /// ViewModel the float is built from, at two widths, and against the words
    /// of the source: nothing may be lost, and no row may overrun.
    #[test]
    fn hover_prose_is_wrapped_to_the_float_it_will_be_drawn_in() {
        let source = "The expression immediately following in must implement the IntoIterator \
                      trait, and the loop runs once per item it yields."
            .to_owned();
        for width in [80u16, 120] {
            let mut editing = typed("x", width);
            let outcome = editing.act(&Action::Lsp(
                phosphor_core::action::LspAction::IngestHover {
                    prose: vec![source.clone()],
                    at: editing.text().cursor(),
                    buffer: None,
                },
            ));
            assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
            let vm = editing.signature.as_ref().expect("hover raises the float");
            let cols = anchored_wrap_cols(width);
            assert!(vm.prose.len() > 1, "at {width}: {:?}", vm.prose);
            for row in &vm.prose {
                assert!(
                    u16::try_from(row.chars().count()).expect("a row") <= cols,
                    "at {width}, {row:?} overruns {cols} columns"
                );
            }
            assert_eq!(
                vm.prose.join(" ").split_whitespace().collect::<Vec<_>>(),
                source.split_whitespace().collect::<Vec<_>>(),
                "at {width}: the words are all still there"
            );
        }
    }

    /// **The session carries its widest width across an answer** (`CP-4`).
    ///
    /// `Float::with_width_floor` had no non-test caller, so every anchored
    /// float still recomputed its width per keystroke — bounded by the cap,
    /// still stepping under the cursor. The host is the only thing that
    /// outlives a frame, so this is where the running maximum lives.
    ///
    /// Both directions: a second answer at the *same* anchor inherits, and one
    /// at a different anchor does not — a floor that never reset would make a
    /// short completion three words later as wide as the widest thing that
    /// happened on that line.
    #[test]
    fn a_completion_session_keeps_the_widest_width_it_has_had() {
        let wide = WireCompletion {
            label: "with_a_rather_long_name".to_owned(),
            detail: Some("fn(D) -> Result<Self, Error>".to_owned()),
            documentation: Vec::new(),
            insert: "with_a_rather_long_name".to_owned(),
            kind: None,
            source: None,
            deprecated: false,
        };
        let narrow = WireCompletion {
            label: "with_a".to_owned(),
            detail: None,
            documentation: Vec::new(),
            insert: "with_a".to_owned(),
            kind: None,
            source: None,
            deprecated: false,
        };
        let ingest = |items: Vec<WireCompletion>, at| {
            Action::Lsp(phosphor_core::action::LspAction::IngestCompletions {
                items,
                at,
                buffer: None,
            })
        };

        let mut editing = typed("with_a", 120);
        let at = editing.text().cursor();
        drop(editing.act(&ingest(vec![wide.clone()], at)));
        let grown = editing
            .completion
            .as_ref()
            .map(|vm| CompletionList::new(vm).desired_width())
            .expect("a session");

        drop(editing.act(&ingest(vec![narrow.clone()], at)));
        let held = editing.completion.as_ref().expect("still a session");
        assert!(
            CompletionList::new(held).desired_width() < grown,
            "the second answer really is narrower"
        );
        assert_eq!(
            held.width_floor, grown,
            "so the float is held to the widest the session has been"
        );

        // A different word is a different session: the anchor moves and the
        // floor goes with it.
        let mut editing = typed("with_a then_a_new_word", 120);
        let moved = editing.text().cursor();
        drop(editing.act(&ingest(vec![narrow], moved)));
        let fresh = editing.completion.as_ref().expect("a new session");
        assert_eq!(fresh.width_floor, 0, "a new word starts content-sized");
    }

    /// **The worst regression `CP-4`'s review found: enter stopped scrolling.**
    ///
    /// `runtime/keymaps.scm` binds `<cr>` in the insert scope, so every newline
    /// typed in insert mode is now an `accept-completion` whose `otherwise` is
    /// `"\n"` — and `moves_cursor` did not name that Action, so `Editing::apply`
    /// skipped the reveal. On the installed binary at 80x24 the cursor reached
    /// line 31 with the viewport still showing lines 1..23: you type where you
    /// cannot see.
    ///
    /// Driven through [`Editing::apply`] and not `act`, because `apply` is
    /// where the reveal lives and it is what [`Session::key`] calls.
    ///
    /// **This bites:** drop the `Action::Lsp` arm from [`moves_cursor`] and the
    /// viewport stays at row 0 with the cursor thirty rows below it.
    #[test]
    fn accepting_a_completion_reveals_the_cursor_it_moved() {
        use phosphor_core::action::LspAction;

        let text: String = (1..=100).map(|line| format!("line {line}\n")).collect();
        let mut editing = editing(&text);
        editing.area = Rect::new(0, 0, 80, 10);
        editing.mode = phosphor_core::request::EditMode::Insert;
        editing.editor.set_cursor(0);

        // Thirty newlines, exactly as `<cr>` sends them with no float open.
        for _ in 0..30 {
            drop(editing.apply(&Action::Lsp(LspAction::AcceptCompletion {
                index: 0,
                then: None,
                otherwise: Some("\n".to_owned()),
            })));
        }

        assert_eq!(
            editing.text().cursor().line,
            31,
            "thirty newlines put the cursor on line 31"
        );
        assert!(
            editing.editor.get_offset_y() > 0,
            "the viewport followed the cursor down; it was still on row {} \
             with the cursor on line 31, which is eight rows below a 10-row window",
            editing.editor.get_offset_y()
        );
        assert!(
            editing
                .editor
                .get_visible_cursor(&editing.area)
                .is_some_and(|(_, y)| u32::from(y) < u32::from(editing.area.height)),
            "and the cursor is on screen rather than below it"
        );
    }

    /// **`<tab>` reveals the row it pushed the cursor onto** — the third arm of
    /// [`moves_cursor`] `CP-4` is responsible for, and the one nothing pressed.
    ///
    /// `insert-indent` writes at the cursor and moves it, so it was added to
    /// `moves_cursor` by analogy with `accept-completion` above — and deleting
    /// `| BufferAction::InsertIndent { .. }` left the whole suite green.
    ///
    /// **Soft wrap is what makes the arm reachable, and it is not decoration.**
    /// [`Editing::reveal`] emits `RevealRow`, and `Viewport::scrolled`'s arm for
    /// it touches `top_row` alone — there is no horizontal reveal in this build
    /// — so with wrapping off a `<tab>` cannot change the cursor's visual row
    /// and the arm has nothing to do. With wrapping on it can: a tab past the
    /// wrap width puts the caret on a continuation row that did not exist
    /// before the key, and if that row is below the viewport you type where you
    /// cannot see, which is the `CP-4` symptom exactly.
    ///
    /// **This bites:** drop the `InsertIndent` arm from `moves_cursor` and the
    /// viewport stays at row 0 with the cursor on row 5 of a 5-row window.
    #[test]
    fn a_tab_that_wraps_the_line_reveals_the_row_it_made() {
        use phosphor_core::action::BufferAction;

        // Five rows of window over six short lines, so row 0 is the top and the
        // fifth line is the last row that fits.
        let text: String = std::iter::repeat_n("abcdefgh\n", 6).collect();
        let mut editing = editing(&text);
        editing.area = Rect::new(0, 0, 20, 5);
        editing.editor.set_soft_wrap(Some(10));
        editing.mode = phosphor_core::request::EditMode::Insert;
        // End of the fifth line — visual row 4, the bottom of the window. Nine
        // chars a line, eight of them text.
        editing.editor.set_cursor(4 * 9 + 8);
        assert_eq!(editing.editor.get_offset_y(), 0, "nothing has scrolled yet");

        // Two tabs: the first fills the line to exactly ten cells and the
        // second is the one that has to wrap.
        for _ in 0..2 {
            drop(editing.apply(&Action::Buffer(BufferAction::InsertIndent {})));
        }

        assert!(
            editing.editor.visual_len_lines() > 6,
            "the line never wrapped, so this asserts nothing about a revealed row"
        );
        assert!(
            editing.editor.get_offset_y() > 0,
            "the viewport stayed on row 0 while the tab pushed the caret onto a \
             continuation row below a five-row window"
        );
        assert!(
            editing
                .editor
                .get_visible_cursor(&editing.area)
                .is_some_and(|(_, y)| u32::from(y) < u32::from(editing.area.height)),
            "and the cursor is on screen rather than below it"
        );
    }

    /// **`R` is still vim's `R`** — the second half of the same `CP-4` defect.
    ///
    /// `Scope::of` folds `EditMode::Replace` into the insert scope, so the
    /// `<space>` and `<cr>` rows bind in `R` too. No completion float can ever
    /// be open there (the loop's trigger is gated on `EditMode::Insert`), so
    /// [`Editing::accept`]'s `otherwise` branch fires unconditionally — and
    /// while it spliced, `R` quietly stopped overwriting.
    ///
    /// **This bites:** delete the `EditMode::Replace` arm in `accept` and the
    /// second case reads `ab cdef` instead of `ab def` — the `c` survives,
    /// which is the whole defect.
    #[test]
    fn the_fall_through_types_the_way_the_mode_types() {
        use phosphor_core::action::LspAction;
        use phosphor_core::request::EditMode;

        let space = Action::Lsp(LspAction::AcceptCompletion {
            index: 0,
            then: None,
            otherwise: Some(" ".to_owned()),
        });
        let at = |mode, text: &str, cursor| {
            let mut editing = editing(text);
            editing.area = Rect::new(0, 0, 80, 24);
            editing.mode = mode;
            editing.editor.set_cursor(cursor);
            drop(editing.apply(&space));
            editing.contents()
        };

        assert_eq!(
            at(EditMode::Insert, "abcdef\n", 2),
            "ab cdef\n",
            "in insert the fall-through inserts, as it always has"
        );
        assert_eq!(
            at(EditMode::Replace, "abcdef\n", 2),
            "ab def\n",
            "in replace it overwrites the character under the cursor — the `c` goes"
        );
        // `R` at the end of a line appends in vim; the newline is not a
        // character it may eat, which is what `Editing::line_end` clamps to.
        assert_eq!(
            at(EditMode::Replace, "ab\ncd\n", 2),
            "ab \ncd\n",
            "at the end of a line it appends rather than joining the next one"
        );
    }

    #[test]
    fn persist_form_appends_to_the_config_home_and_says_where() {
        let config = scratch("persist");
        // No layer booted here, so the defaults stand: a layer that declares
        // no file writes to `init.scm`, which is what `6b` draws, and a layer
        // that offers nothing keeps everything.
        let host = AppHost::new(Some(config.clone()));
        let form = r#"(keymap-set! "]r" (lambda () 1))"#;
        let Outcome::Done(receipt) = ask(
            &host,
            Action::Runtime(RuntimeAction::PersistForm {
                form: form.to_owned(),
            }),
        ) else {
            panic!("a config home persists");
        };
        // `6b`: `⇒ #ok · persisted to init.scm`.
        assert_eq!(receipt.note.as_deref(), Some("persisted to init.scm"));
        // The directory did not exist a moment ago — a cold start that
        // persists nothing leaves no trace, so the writer makes it.
        let written =
            std::fs::read_to_string(config.join("init.scm")).expect("the file exists now");
        assert!(written.contains(form), "{written:?}");

        let _ = std::fs::remove_dir_all(&config);
    }

    /// **The gate `T101` added, and the pair that shows it is a gate.**
    ///
    /// Same form, same host, one wrapped in the verb: the bare one is offered
    /// and the marked one is kept. Teej ruled this on 2026-08-14 and it
    /// overrides `6b`, which draws a bare `(keymap-set! …)` answering
    /// `⇒ #ok · persisted to init.scm`. Recorded in
    /// `docs/OPEN-QUESTIONS.md` §32.
    #[test]
    fn a_form_is_kept_only_when_the_verb_marks_it() {
        let config = scratch("explicit");
        let host = AppHost::new(Some(config.clone()));
        host.persist_policy("persist!".to_owned(), vec!["keymap-set!".to_owned()]);

        let bare = r#"(keymap-set! "]r" (lambda () 1))"#;
        let Outcome::Refused(Refusal::Declined { reason }) = ask(
            &host,
            Action::Runtime(RuntimeAction::PersistForm {
                form: bare.to_owned(),
            }),
        ) else {
            panic!("evaluating is evaluating — a bare config verb is session-only");
        };
        // The remedy, and one em dash in the receipt `Repl::persist` builds
        // out of it (§6). The reason used to open `session only — `, which
        // put a second dash and a restatement of `not persisted` into the
        // line a reader sees.
        assert_eq!(reason, "(persist! …) keeps it");
        assert!(
            !config.join("init.scm").exists(),
            "a refused persist writes nothing at all, not even an empty file"
        );

        let marked = format!("(persist! {bare})");
        let Outcome::Done(_) = ask(
            &host,
            Action::Runtime(RuntimeAction::PersistForm {
                form: marked.clone(),
            }),
        ) else {
            panic!("the verb is the explicit act");
        };
        let written =
            std::fs::read_to_string(config.join("init.scm")).expect("the file exists now");
        assert_eq!(written, format!("{marked}\n"));

        let _ = std::fs::remove_dir_all(&config);
    }

    /// **`7a` still writes its rule.** *"`[2] always allow git push` → writes
    /// `(allow "git push")` to init.scm"* — the user pressed a digit, so the
    /// act is already explicit and the gate must not stand in front of it. A
    /// head the layer never listed is written as given, which is what keeps a
    /// permission grant surviving a restart (`T061` builds the surface).
    #[test]
    fn a_head_the_layer_never_offered_is_written_as_given() {
        let config = scratch("always-allow");
        let host = AppHost::new(Some(config.clone()));
        host.persist_policy("persist!".to_owned(), vec!["keymap-set!".to_owned()]);

        let rule = r#"(allow "git push")"#;
        let Outcome::Done(receipt) = ask(
            &host,
            Action::Runtime(RuntimeAction::PersistForm {
                form: rule.to_owned(),
            }),
        ) else {
            panic!("7a's always-allow is not the REPL's auto-persist and is not gated");
        };
        assert_eq!(receipt.note.as_deref(), Some("persisted to init.scm"));
        assert_eq!(
            std::fs::read_to_string(config.join("init.scm")).expect("the rule"),
            format!("{rule}\n")
        );

        let _ = std::fs::remove_dir_all(&config);
    }

    /// **Two phosphors on one config home is an ordinary case.**
    ///
    /// `O_APPEND` makes one `write` atomic against another; it promises
    /// nothing about a `write_fmt` that issues a syscall per format piece. The
    /// mutation is one character wide — `writeln!(handle, "{form}")` in place
    /// of the single `write_all` — and it puts one process's newline inside
    /// another's form often enough for this to go red.
    #[test]
    fn a_form_is_appended_whole_when_several_writers_race() {
        let config = scratch("racing");
        let host = Arc::new(AppHost::new(Some(config.clone())));
        // Long enough that a torn write is visible rather than lucky: a
        // formatter splitting on the newline leaves a short line behind.
        let form = format!("(persist! (set-option! \"pad\" \"{}\"))", "x".repeat(400));

        let writers = 8;
        let each = 25;
        std::thread::scope(|scope| {
            for _ in 0..writers {
                let host = Arc::clone(&host);
                let form = form.clone();
                drop(scope.spawn(move || {
                    for _ in 0..each {
                        drop(host.persist(&form));
                    }
                }));
            }
        });

        let written = std::fs::read_to_string(config.join("init.scm")).expect("the file");
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), writers * each, "no line was lost or split");
        assert!(
            lines.iter().all(|line| *line == form),
            "every line is one whole form"
        );

        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn a_persisted_rebind_survives_the_next_boot() {
        // **The regression that a pty run found and no unit test would have.**
        // `init.scm` runs to its last form before Rust reads the load order it
        // declared, so a `(keymap-set! …)` appended *there* comes back on the
        // next start as a free-identifier fault — `keymaps.scm` has not loaded
        // yet. `T101` moved the file out of the tree entirely and made "last"
        // a call site (`Layer::load_persisted`); this types the rebind, throws
        // the editor away, and starts a new one over the same two directories.
        let root = copy_of_the_layer("reboot");
        let config = scratch("reboot-config");
        let form = r#"(persist! (keymap-set! "gz" (lambda () (open-repl!))))"#;

        {
            let (mut layer, _host) = booted_with_config(Some(&root), &config);
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
        // Written where the runtime tree is not — the whole of `T101`'s second
        // half. `CP-4` left a `(define-language! "lua" …)` in the tracked
        // `runtime/persisted.scm` because these two used to be one directory.
        assert!(config.join("persisted.scm").is_file());
        assert!(!root.join("persisted.scm").exists());

        // A second editor over the same pair — a restart, in one process.
        let (mut layer, host) = booted_with_config(Some(&root), &config);
        assert!(
            layer.report().is_clean(),
            "a persisted form must not fault the next boot: {:?}",
            layer.report().faults
        );
        assert_eq!(resolved(&mut layer, "g"), Resolution::Pending);
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);
        assert_eq!(host.intents(), vec![Intent::OpenRepl]);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **A layer that is also the boot root runs once, not twice.**
    ///
    /// The config home was `Runtime::root`'s second candidate when this was
    /// written, so for a user with one file the boot root and the persist
    /// target were the same path — and `vm()` called `load_persisted` on it
    /// unconditionally. Every form in their `init.scm` ran at boot and again
    /// straight after: reproduced on the built binary with `(displayln
    /// "BOOTED-ONCE")`, which printed twice before `Layer::booted_already`.
    ///
    /// **§34 took the config home out of `Runtime::root` and this test kept its
    /// teeth**, because the shape it describes did not go away — it is what
    /// `$PHOSPHOR_RUNTIME` pointed at a config home gives you, and the boot
    /// report is still the only thing that knows. The population that lost its
    /// *only* root is covered by
    /// [`the_only_file_a_machine_has_runs_once_though_it_is_both_layers`],
    /// which takes the other arm of `booted_already`.
    ///
    /// `open-repl!` rather than a `define`, because a repeated `define` is
    /// invisible: the assertion has to be over something that *accumulates*.
    /// Two intents is what double evaluation looks like from outside.
    #[test]
    fn a_one_file_layer_in_the_config_home_boots_once_rather_than_twice() {
        let config = scratch("one-file-layer");
        std::fs::write(config.join("init.scm"), "(open-repl!)\n").expect("a one-file layer");

        // One directory playing every part: the boot root, the user's layer
        // and the persist target, which is what `$PHOSPHOR_RUNTIME` aimed at a
        // config home builds.
        let (layer, host) = booted_with_config(Some(&config), &config);

        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl],
            "the user's own init.scm ran twice"
        );
        assert_eq!(
            layer
                .report()
                .units
                .iter()
                .filter(|unit| unit.file == Path::new("init.scm"))
                .count(),
            1,
            "and the boot report said so: {:?}",
            layer.report().units
        );

        let _ = std::fs::remove_dir_all(&config);
    }

    /// The other half of the same guard: a persist file the boot did **not**
    /// read still loads, even when it sits in the boot root.
    ///
    /// Without this the fix would be *"skip anything in the config home"*,
    /// which would silently stop loading `persisted.scm` for a user whose
    /// layer lives there — the failure `T101` was reported for, restored by
    /// its own repair.
    #[test]
    fn a_persist_file_the_boot_never_loaded_still_runs() {
        let config = scratch("root-is-config");
        std::fs::write(
            config.join("init.scm"),
            "(define phosphor/persist-file \"persisted.scm\")\n",
        )
        .expect("a layer that names a persist file");
        std::fs::write(config.join("persisted.scm"), "(open-repl!)\n").expect("a persisted layer");

        let (layer, host) = booted_with_config(Some(&config), &config);

        assert!(layer.report().is_clean(), "{:?}", layer.report().faults);
        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl],
            "the persisted layer is in the boot root but not in its load order"
        );

        let _ = std::fs::remove_dir_all(&config);
    }

    /// A form in the persisted file that does not run costs that line and
    /// nothing else, and the fault reaches the same float every boot fault
    /// does — the file the header invites you to hand-edit is the one that
    /// most needs it.
    #[test]
    fn a_broken_persisted_form_costs_one_line_and_reaches_the_boot_float() {
        let root = copy_of_the_layer("broken-persist");
        let config = scratch("broken-persist-config");
        std::fs::create_dir_all(&config).expect("a scratch config home");
        std::fs::write(
            config.join("persisted.scm"),
            "(no-such-verb 1)\n(persist! (keymap-set! \"gz\" (lambda () (open-repl!))))\n",
        )
        .expect("a hand-edited persisted layer");

        let (mut layer, _host) = booted_with_config(Some(&root), &config);
        let faults = &layer.report().faults;
        assert_eq!(faults.len(), 1, "{faults:#?}");
        // Column 2, not 1: the fault is placed at the identifier Steel
        // objected to rather than at the paren, which is what makes the
        // float's source line worth reading.
        assert_eq!(faults[0].place(), "persisted.scm:1:2");
        // **One voice, not two.** This file's own copy of `boot::steel_fault`
        // stripped `Error: ` and stopped, so a persisted fault drew
        // `FreeIdentifier: Cannot reference …` in the same float a `keymaps.scm`
        // fault drew `cannot reference …` in — a Rust type name reaching a
        // reader, which §6 and `phosphor-term`'s
        // `no_error_kind_reaches_a_reader_as_a_rust_name` both forbid. The old
        // assertion here was `!starts_with("Error: ")`, which permitted it.
        assert_eq!(faults[0].label, "free identifier");
        assert_eq!(
            faults[0].message, "cannot reference an identifier before its definition: no-such-verb",
            "the persisted layer speaks in the boot's voice"
        );
        assert!(
            layer.boot_float().is_some(),
            "a fault outside the runtime tree still reaches the float"
        );
        // And the form under it still ran.
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    // -----------------------------------------------------------------------
    // §34 — a user's own `init.scm` layers over the shipped one
    // -----------------------------------------------------------------------

    /// **The defect §34 measured, at the seam that caused it.**
    ///
    /// A config home holding one `(set-option! "soft-wrap" #t)` used to *be*
    /// the runtime tree, so the shipped fifteen files never loaded and the
    /// editor had no keymaps, no statusline and no way to quit — with no boot
    /// float, because that one form ran cleanly.
    ///
    /// Both halves are asserted because either alone would pass over the bug:
    /// the option alone passed before this change too (the user's file ran —
    /// it was the *only* thing that ran), and the keymap alone passes for a
    /// config home nobody wrote in.
    #[test]
    fn a_user_init_scm_layers_over_the_shipped_layer_rather_than_replacing_it() {
        let root = copy_of_the_layer("user-layer");
        let config = scratch("user-layer-config");
        std::fs::write(
            config.join("init.scm"),
            "(set-option! \"soft-wrap\" #t)\n(keymap-set! \"gz\" (lambda () (open-repl!)))\n",
        )
        .expect("the file a user writes first");

        let (mut layer, host) = booted_with_config(Some(&root), &config);

        assert!(layer.report().is_clean(), "{:?}", layer.report().faults);
        assert_eq!(
            host.flag("soft-wrap"),
            Some(true),
            "the user's own form never ran"
        );
        // `runtime/init.scm` sets `soft-wrap` to `#f`, so the assertion above
        // is not that the user's value is *a* value — it is that it is the
        // later one.
        //
        // And `ZQ` is the key §34 pressed and nothing happened: the whole
        // reproduction ended in `kill`, so the binding that quits is the one
        // worth naming here rather than any shipped binding at all.
        assert_eq!(
            resolved(&mut layer, "ZQ"),
            Resolution::Role(Role::Run(vec![Action::App(
                phosphor_core::action::AppAction::Quit { force: true }
            )])),
            "the shipped keymap did not survive a user's init.scm"
        );
        // And the user's own binding is live, which is only possible because
        // `keymap-set!` — defined in the shipped `keymaps.scm` — was already
        // bound when their file ran. That ordering is the whole ruling.
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **Both directions, and neither needed a new verb.**
    ///
    /// Teej's ruling of 2026-08-14 settled the question §34 said needed
    /// settling — whether a user's file may *remove* a shipped binding as well
    /// as add one — by observing that layering already expresses it:
    /// `keymap-remove!` is defined in `runtime/keymaps.scm` and listed among
    /// `runtime/repl.scm`'s persistable heads.
    ///
    /// `gg` is rebound rather than added, because *overriding* and *adding* are
    /// the same call and only the first proves the shipped entry was there to
    /// be replaced: `runtime/keymaps.scm` binds `gg` to `(key/goto "first")`,
    /// and `Resolution::Ran` is a thunk, which a goto role is not.
    #[test]
    fn a_user_init_scm_overrides_one_shipped_binding_and_removes_another() {
        let root = copy_of_the_layer("user-override");
        let config = scratch("user-override-config");
        std::fs::write(
            config.join("init.scm"),
            "(keymap-set! \"gg\" (lambda () (open-repl!)))\n(keymap-remove! \"ZQ\")\n",
        )
        .expect("a layer that edits the shipped one");

        let (mut layer, host) = booted_with_config(Some(&root), &config);

        assert!(layer.report().is_clean(), "{:?}", layer.report().faults);
        assert_eq!(
            resolved(&mut layer, "gg"),
            Resolution::Ran,
            "the shipped `gg` goto was not overridden"
        );
        assert_eq!(host.intents(), vec![Intent::OpenRepl]);
        assert_eq!(
            resolved(&mut layer, "ZQ"),
            Resolution::Unbound,
            "keymap-remove! did not reach the shipped table"
        );
        // Removing one binding removes one binding: `ZZ` shares its prefix and
        // is untouched.
        assert!(matches!(resolved(&mut layer, "ZZ"), Resolution::Role(_)));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// A user's file that throws costs that form and nothing else — and the
    /// fault says **which** `init.scm` it came from.
    ///
    /// There are two files with that name once the layers stack, so a float
    /// reading `init.scm:1:2 · free identifier` in front of a person who has
    /// one of each has answered nothing. `config::abbreviated` is what makes
    /// the row name the config home without putting `$HOME` on a screenshot.
    #[test]
    fn a_broken_user_init_costs_one_form_and_names_itself_in_the_float() {
        let root = copy_of_the_layer("broken-user");
        let config = scratch("broken-user-config");
        std::fs::write(
            config.join("init.scm"),
            "(no-such-verb 1)\n(keymap-set! \"gz\" (lambda () (open-repl!)))\n",
        )
        .expect("a hand-edited layer with a mistake in it");

        let (mut layer, _host) = booted_with_config(Some(&root), &config);

        let faults = &layer.report().faults;
        assert_eq!(faults.len(), 1, "{faults:#?}");
        assert_eq!(faults[0].label, "free identifier");
        assert_eq!(
            faults[0].place(),
            format!("{}:1:2", config.join("init.scm").display()),
            "a fault in the user's layer must name the user's file"
        );
        assert!(
            layer.boot_float().is_some(),
            "a fault outside the runtime tree still reaches the float"
        );
        // The form under it ran, and so did the whole shipped layer.
        assert_eq!(resolved(&mut layer, "gz"), Resolution::Ran);
        assert!(matches!(resolved(&mut layer, "gg"), Resolution::Role(_)));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **The order, where it is observable: the persisted layer wins.**
    ///
    /// Both files bind the same key, so whichever ran last is the one that
    /// answers. A form you deliberately kept at the REPL is the later act and
    /// the more explicit one; a hand-written `init.scm` beats the shipped
    /// default and loses to that.
    ///
    /// **Swap the two `if let` blocks in [`stack`] and this goes red** —
    /// checked by doing it, `1 test run: 0 passed, 1 failed`. That sentence
    /// used to say `vm` and was false: `booted_with_config` was a second copy
    /// of those calls, so the mutation had to be made twice to be seen, and a
    /// review made it once and watched 187 tests pass. There is one copy now
    /// and [`booted_with_config`] is a name for it.
    #[test]
    fn the_persisted_layer_runs_after_the_users_own_file() {
        let root = copy_of_the_layer("stack-order");
        let config = scratch("stack-order-config");
        std::fs::write(
            config.join("init.scm"),
            "(keymap-set! \"gz\" (key/goto \"first\") \"hand-written\")\n",
        )
        .expect("a hand-written layer");
        std::fs::write(
            config.join("persisted.scm"),
            "(persist! (keymap-set! \"gz\" (lambda () (open-repl!)) \"persisted\"))\n",
        )
        .expect("a persisted layer over the same key");

        let (mut layer, host) = booted_with_config(Some(&root), &config);

        assert!(layer.report().is_clean(), "{:?}", layer.report().faults);
        assert_eq!(
            resolved(&mut layer, "gz"),
            Resolution::Ran,
            "the hand-written binding outranked the persisted one"
        );
        assert_eq!(host.intents(), vec![Intent::OpenRepl]);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **One file playing two parts still runs once.**
    ///
    /// On a machine with no shipped tree nothing declares a [`PERSIST_FILE`],
    /// so the persist target falls back to [`INIT`] — which is also the user's
    /// own layer. Both call sites in `vm` then name the same path, and the boot
    /// report cannot say so, because there is no boot: `Runtime::root` answers
    /// `None` and the report has no root to measure a file against.
    /// [`Layer::after_boot`] is the record that answers, and deleting it puts
    /// every form in that file through the VM twice.
    ///
    /// `(open-repl!)` rather than a `define`, for
    /// `a_one_file_layer_in_the_config_home_boots_once_rather_than_twice`'s
    /// reason: a repeated `define` is invisible and an intent accumulates.
    #[test]
    fn the_only_file_a_machine_has_runs_once_though_it_is_both_layers() {
        let config = scratch("only-file-config");
        std::fs::write(config.join("init.scm"), "(open-repl!)\n").expect("the only layer there is");

        let (layer, host) = booted_with_config(None, &config);

        assert_eq!(
            host.intents(),
            vec![Intent::OpenRepl],
            "the user's own init.scm ran twice — once as their layer and once \
             as the persist target"
        );
        // One unit, and it is that file: the boot read nothing at all, so
        // everything in this report came from a call site in `vm`.
        assert_eq!(
            layer.report().units.len(),
            1,
            "and the boot report said so: {:?}",
            layer.report().units
        );
        assert!(
            layer.report().units[0].file.ends_with("init.scm"),
            "{:?}",
            layer.report().units
        );

        let _ = std::fs::remove_dir_all(&config);
    }

    /// **§34's disclosure half: an editor that loaded nothing says so.**
    ///
    /// The state is an installed binary run from outside its checkout with no
    /// `$PHOSPHOR_RUNTIME` and no config home — no keymaps, no statusline, no
    /// way to quit, and until this, no fault either, because a boot that read
    /// no files has nothing to report. The float names the file to create.
    #[test]
    fn an_editor_that_loaded_no_layer_at_all_says_so_in_the_float() {
        let nowhere = scratch("no-layer");
        let config = scratch("no-layer-config");
        let (mut layer, _host) = booted_with_config(Some(&nowhere), &config);
        assert!(
            layer.report().is_clean(),
            "an empty root is not a fault on its own"
        );
        assert!(
            layer.boot_float().is_none(),
            "and it draws nothing until the loop asks"
        );

        layer.note_if_no_layer(Some(&config));

        let faults = &layer.report().faults;
        assert_eq!(faults.len(), 1, "{faults:#?}");
        assert_eq!(faults[0].label, "no editor layer");
        // The short half on the row that carries the label — see
        // `note_if_no_layer` for why the path may not go here.
        assert_eq!(faults[0].place(), "init.scm");
        assert!(
            faults[0]
                .message
                .contains(&config.join("init.scm").display().to_string()),
            "the file to create is not named: {}",
            faults[0].message
        );
        assert!(
            faults[0].message.contains("PHOSPHOR_RUNTIME"),
            "the other way out is not named: {}",
            faults[0].message
        );
        assert!(layer.boot_float().is_some());

        // **Said once, and only when there is nothing.** A layer that loaded
        // anything at all is a working editor and this must stay silent — the
        // float is `T021`'s alarm and an alarm that fires on a clean boot is
        // one people learn to close.
        let root = copy_of_the_layer("no-layer-shipped");
        let (mut booted, _host) = booted_with_config(Some(&root), &config);
        booted.note_if_no_layer(Some(&config));
        assert!(booted.report().is_clean(), "{:?}", booted.report().faults);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&nowhere);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **Writing the file the float tells you to write must not turn the float
    /// off** — §34's own population, and the arm the first guard missed.
    ///
    /// An installed binary outside a checkout with no `$PHOSPHOR_RUNTIME` and a
    /// config home holding §34's own one-line `init.scm`. That form runs, so
    /// `report.units` is not empty — and the disclosure was guarded on exactly
    /// that, so this state said nothing while reproducing §34's symptom
    /// verbatim: measured on a pty, `soft-wrap` applied, no statusline, no
    /// float, `SPC` drawing `unknown key <space>`, `ZQ` doing nothing, the
    /// process killed.
    ///
    /// [`an_editor_that_loaded_no_layer_at_all_says_so_in_the_float`] passes an
    /// **empty** config home and therefore cannot see this, which is why it is
    /// a second test rather than a second assertion.
    ///
    /// The message differs from that one's, and the difference is the whole
    /// point: *write the file* is not advice for somebody who wrote it. The
    /// remaining remedy is the variable, and the assertion below is that the
    /// float stopped repeating the other one.
    #[test]
    fn a_user_init_scm_with_nothing_under_it_is_still_an_editor_with_no_layer() {
        let nowhere = scratch("wrote-it-anyway");
        let config = scratch("wrote-it-anyway-config");
        std::fs::write(config.join("init.scm"), "(set-option! \"soft-wrap\" #t)\n")
            .expect("§34's own one-line file");

        let (mut layer, host) = booted_with_config(Some(&nowhere), &config);
        // The form ran — this is not a boot that read nothing.
        assert_eq!(host.flag("soft-wrap"), Some(true));
        assert_eq!(layer.report().units.len(), 1, "{:?}", layer.report().units);

        layer.note_if_no_layer(Some(&config));

        let faults = &layer.report().faults;
        assert_eq!(
            faults.len(),
            1,
            "an editor with no keymaps said nothing: {faults:#?}"
        );
        assert_eq!(faults[0].label, "no editor layer");
        assert!(
            faults[0].message.contains("PHOSPHOR_RUNTIME"),
            "{}",
            faults[0].message
        );
        assert!(
            !faults[0].message.contains("write "),
            "the file is already written — telling them to write it is the \
             advice that made this float useless: {}",
            faults[0].message
        );
        // And the footer may not teach `:repl` here: `:` is `keymaps.scm`'s and
        // `keymaps.scm` did not load. `phosphor_steel::float::ExLine`.
        let float = layer.boot_float().expect("a fault opens the float");
        let phosphor_core::view::Node::KeyHints { hints, .. } =
            float.footer.expect("a footer").node().clone()
        else {
            panic!("the footer is a keymap surface");
        };
        assert_eq!(hints.len(), 1, "{hints:?}");
        assert_eq!(hints[0].key.0, "esc");

        let _ = std::fs::remove_dir_all(&nowhere);
        let _ = std::fs::remove_dir_all(&config);
    }

    /// **An `init.scm` with no forms in it is the same state**, and it reaches
    /// the guard by a different door.
    ///
    /// `load_after_boot` pushes a [`BootUnit`] for any file that *reads*,
    /// whether or not it held a form, so an empty file made `units` non-empty
    /// and silenced the disclosure while leaving the editor with no bindings at
    /// all. That is the literal shape of *"I created the file and nothing
    /// happened"* — the first thing a person does after being told to create
    /// one.
    #[test]
    fn an_empty_user_init_scm_does_not_buy_silence() {
        let nowhere = scratch("empty-init");
        let config = scratch("empty-init-config");
        std::fs::write(config.join("init.scm"), "\n").expect("a file with nothing in it");

        let (mut layer, _host) = booted_with_config(Some(&nowhere), &config);
        assert_eq!(layer.report().forms_ran(), 0);

        layer.note_if_no_layer(Some(&config));

        let faults = &layer.report().faults;
        assert_eq!(faults.len(), 1, "{faults:#?}");
        assert_eq!(faults[0].label, "no editor layer");

        let _ = std::fs::remove_dir_all(&nowhere);
        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn with_no_config_home_a_persist_is_refused_rather_than_guessed() {
        // `T101` moved this case rather than deleting it: "nowhere to write"
        // is still reachable — a CI runner or a container with neither
        // `XDG_CONFIG_HOME` nor `HOME` — and inventing a path would be worse
        // than a refusal. It used to name `$PHOSPHOR_RUNTIME`, which was the
        // wrong variable even then: the runtime tree is what you *read*.
        let Outcome::Refused(Refusal::Declined { reason }) = ask(
            &AppHost::new(None),
            Action::Runtime(RuntimeAction::PersistForm {
                form: r#"(set-option! "soft-wrap" #t)"#.to_owned(),
            }),
        ) else {
            panic!("there is nowhere to write, and inventing one would be worse");
        };
        assert_eq!(
            reason,
            "no config home to write to — set $XDG_CONFIG_HOME or $HOME"
        );
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
        // `T098`: `q` is **known and not built**, so it does not spend `T035`'s
        // one teaching row. It used to resolve to a thunk that did nothing,
        // because the vocabulary had no macro verb to name; the repair window
        // between `CP-3` and `S4` added `set-macro-recording`, so it resolves
        // to a capability call now and the refusal names `T099` instead of the
        // key saying nothing. `Q` is what genuinely nobody binds.
        assert!(
            matches!(resolved(&mut layer, "q"), Resolution::Role(Role::Run(_))),
            "`q` names the verb that will record, and declines by naming its task"
        );
        // `@` is the one that is still a thunk, and it is argued where it is
        // bound: playing a macro is `feed-keys` over the `register` query's
        // answer, and a keymap cannot ask a query.
        assert_eq!(
            resolved(&mut layer, "@"),
            Resolution::Ran,
            "`@` is deferred on purpose, and a deferred key is bound"
        );
        assert_eq!(
            resolved(&mut layer, "Q"),
            Resolution::Unbound,
            "`Q` is nobody's — the machine turns it into the unknown-key hint"
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

    /// The grammar a file opens with comes from the **declarations**, not from
    /// a Rust table (`T037`).
    ///
    /// The table this replaced was a `match` over ten extensions in this file,
    /// and every assertion below used to pass against it. What makes this test
    /// about the new thing rather than the old one is the last three: `steel`
    /// declares the grammar `scheme` and this build bundles none, `csv`
    /// declares none at all — both are `"text"`, and both answers are
    /// *consequences of the `.scm` files* rather than of an arm here. Delete
    /// `runtime/languages/rust.scm` and the first line goes red.
    #[test]
    fn the_grammar_a_file_opens_with_comes_from_the_declarations() {
        let (_layer, host) = booted();
        let languages = host.languages();
        assert_eq!(languages.len(), 12, "the twelve declared at boot");

        assert_eq!(grammar_of(&languages, "src/main.rs".as_ref()), "rust");
        assert_eq!(grammar_of(&languages, "Cargo.toml".as_ref()), "toml");
        // Case is the filesystem's business, not the language's.
        assert_eq!(grammar_of(&languages, "notes.MD".as_ref()), "markdown");
        // Declared, and second tier: the fork bundles no `scheme`.
        assert_eq!(grammar_of(&languages, "runtime/init.scm".as_ref()), "text");
        // Declared with no grammar at all — `T082`, deliberately.
        assert_eq!(grammar_of(&languages, "rows.csv".as_ref()), "text");
        // Undeclared, which is second tier and a normal state.
        assert_eq!(grammar_of(&languages, "README".as_ref()), "text");
    }

    /// A thirteenth language, typed the way `CP-4`'s manual half types it, is
    /// what the *next* file opens as — through the shipping host's own
    /// `define-language` arm rather than a recorder standing in for it.
    ///
    /// `crates/phosphor-steel/tests/shipped_languages.rs` proves the same
    /// property against a miniature host it defines itself; this is the half
    /// that could not be proved there, because that crate cannot see the
    /// binary. Without this, `AppHost`'s arm could be missing entirely and
    /// that suite would still be green.
    #[test]
    fn a_thirteenth_language_declared_at_the_repl_claims_its_extension() {
        let (mut layer, host) = booted();
        assert_eq!(
            grammar_of(&host.languages(), "notes.jsonc".as_ref()),
            "text",
            "nothing claims .jsonc before the declaration"
        );
        let outcome = layer.evaluate(
            r##"(define-language! "jsonc"
                  (hash "extensions" '("jsonc")
                        "grammar" "json"
                        "lsp_command" '()
                        "comment_prefix" "//"))"##,
        );
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
        assert_eq!(
            grammar_of(&host.languages(), "notes.jsonc".as_ref()),
            "json",
            "a first-class thirteenth, over a grammar the binary already has"
        );
    }

    /// **An `indent` that says neither of the two things it is for is refused
    /// at the door**, which is the only place it can be.
    ///
    /// The field's argument (`LanguageSpec::indent`) is that one literal says
    /// *width* and *tabs-vs-spaces* together. A literal saying neither was
    /// accepted and then read differently by each of its two readers:
    /// [`IndentStyle::typed_at`] pads to the next stop for anything not
    /// starting with `\t`, [`Editing::indent`] splices the literal — so `" \t"`
    /// gave `>` a space-tab and `<tab>` two spaces, `""` gave `>` a no-op and
    /// `<tab>` one space, and `"\t\t"` gave `>` two tabs and `<tab>` one.
    /// `IndentStyle::width` counts `chars`, so `"　"` (one ideographic space,
    /// two cells) measured one.
    ///
    /// All four are refused here, and the legal three are declared afterwards
    /// so this cannot pass by refusing everything.
    ///
    /// **This bites:** delete the `Invalid::Indent` arm from
    /// `Languages::declare` and the first four assertions fail.
    #[test]
    fn an_indent_that_says_neither_width_nor_whitespace_is_refused() {
        let (mut layer, host) = booted();
        let declared = |layer: &mut Layer, indent: &str| {
            layer.evaluate(&format!(
                r##"(define-language! "zz"
                      (hash "extensions" '("zz") "grammar" void "lsp_command" '()
                            "comment_prefix" void "indent" "{indent}"))"##
            ))
        };
        // The last is one ideographic space, written as itself: `\t` reaches
        // Steel as an escape (the legal `"\t"` below is accepted, which proves
        // it) and a `\u{…}` spelling would only prove that Steel does not read
        // one.
        for unit in ["", " \\t", "\\t\\t", "\u{3000}"] {
            let outcome = declared(&mut layer, unit);
            assert!(
                refused(&outcome).is_some_and(|why| why.contains("neither one tab nor a run")),
                "indent {unit:?} was accepted: {outcome:?}"
            );
        }
        assert_eq!(
            host.languages().len(),
            12,
            "no refused declaration reached the table"
        );

        // And the three legal shapes land, so the guard is a rule rather than a
        // wall: two spaces, four spaces, one tab.
        for unit in ["  ", "    ", "\\t"] {
            let outcome = declared(&mut layer, unit);
            assert!(
                matches!(outcome, Outcome::Done(_)),
                "indent {unit:?} is legal and was refused: {outcome:?}"
            );
        }
        assert_eq!(host.languages().len(), 13, "one thirteenth, redeclared");
    }

    /// The two refusals `Languages::declare` owes, reaching the REPL through
    /// the shipping arm. Both are declarations that would *land* and then
    /// never match a file, which is worse than a refusal.
    #[test]
    fn a_declaration_that_could_never_match_a_file_is_refused() {
        let (mut layer, host) = booted();
        // A refusal raised inside an evaluation comes back **inside** the
        // evaluation's own value — `(#refused "…")`, which is what the REPL
        // prints under the form you typed — rather than as a refused `eval`.
        let dotted = layer.evaluate(
            r##"(define-language! "elixir"
                  (hash "extensions" '(".ex") "grammar" void "lsp_command" '()))"##,
        );
        assert!(
            refused(&dotted).is_some_and(|why| why.contains("has a dot in it")),
            "an extension with a dot never matches: {dotted:?}"
        );
        let nameless = layer.evaluate(
            r##"(define-language! "  "
                  (hash "extensions" '("zz") "grammar" void "lsp_command" '()))"##,
        );
        assert!(
            refused(&nameless).is_some_and(|why| why.contains("needs a name")),
            "{nameless:?}"
        );
        assert_eq!(
            host.languages().len(),
            12,
            "neither refusal mutates the table"
        );
    }

    // -----------------------------------------------------------------------
    // `T107` — a buffer with no file
    // -----------------------------------------------------------------------

    /// **The seed folds back into the tree it came from**, which is the law
    /// `journal::Folded` states for `snapshot` — *"a `snapshot` that loses
    /// something loses it permanently and silently"*.
    ///
    /// [`seeding`] is the one place in this build that writes a whole tree into
    /// a journal that has never seen it, and a pty test cannot reach the shape
    /// that breaks it: the tree below has a **branch point whose redo pointer
    /// is on its older child, off the path to the cursor**, which takes a
    /// checkpoint walk to produce and survives only if `seeding` says both of
    /// the two things replaying the nodes alone cannot say.
    ///
    /// Both bite, and each on its own line:
    ///
    /// * Drop the `Redo` loop and `node 1` comes back pointing at `3` — the
    ///   fold points every branch at its newest child as the nodes arrive, and
    ///   nothing on the path to `4` ever revisits `1`. A redo in the next
    ///   session would then walk into the wrong branch.
    /// * Drop the trailing `Cursor` and the buffer comes back at `4`'s newest
    ///   sibling rather than where it was left.
    ///
    /// The origin is checked too, because [`Timeline::open_at`] refuses a
    /// journal whose origin is not the file it was opened for — a seed that
    /// wrote the wrong one would produce a history that opens exactly once.
    #[test]
    fn the_seed_a_scratch_buffer_writes_folds_back_into_the_tree_it_came_from() {
        use phosphor_buffer::undo::Edit as TreeEdit;
        use phosphor_core::journal::Folded as _;

        let at = |offset| Caret {
            offset,
            selection: None,
        };
        let commit = |tree: &mut UndoTree, from: usize, text: &str| {
            tree.record(at(from), TreeEdit::insert(from, text));
            tree.commit(at(from + text.len()))
                .expect("a committed node")
        };

        let mut tree = UndoTree::new();
        let one = commit(&mut tree, 0, "a");
        let two = commit(&mut tree, 1, "b");
        tree.goto(one);
        commit(&mut tree, 1, "c");
        // Back onto the *older* child, which is what leaves `one`'s redo
        // pointer somewhere the newest-child rule would never put it…
        tree.goto(two);
        // …and then away from that subtree entirely, so nothing on the path to
        // the cursor ever passes through `one` again to correct it.
        tree.goto(NodeId::ROOT);
        let four = commit(&mut tree, 0, "d");
        commit(&mut tree, 1, "e");
        // And the cursor is left one step back from the newest node, which is
        // the other thing replaying alone gets wrong: a fold that has just read
        // the nodes is sitting on the last one it read.
        tree.goto(four);

        let mut folded = wire_undo::History::default();
        for record in seeding(&tree, "/tmp/scratch.txt".to_owned()) {
            folded
                .apply(record)
                .expect("the fold accepts its own shape");
        }
        assert_eq!(folded.origin(), Some("/tmp/scratch.txt"));

        let rebuilt = restored(folded).expect("the seed restores");
        assert_eq!(rebuilt.nodes(), tree.nodes(), "every node, and every link");
        assert_eq!(rebuilt.current(), tree.current(), "and where the buffer is");
    }

    /// **`:write` at a buffer with no file refuses, and says what would work.**
    ///
    /// The unit half of the pty test with the same subject: this one pins the
    /// sentence, and the pty one pins that it is the sentence a person meets
    /// rather than clap's.
    ///
    /// [`editing`] builds a buffer with `file: None`, which is exactly what a
    /// bare `phosphor` opens.
    #[test]
    fn writing_a_buffer_with_no_file_refuses_by_naming_the_whole_command() {
        let mut editing = editing("typed into a scratch buffer");
        assert_eq!(
            editing.write(None),
            Err("no file name — :write <path>".to_owned())
        );
        assert!(
            editing.timeline.log.is_none(),
            "a refused write opened a journal anyway"
        );
    }

    /// **Every command line that worked still parses to what it did**, and the
    /// bare one now parses at all.
    ///
    /// `T107` deleted `required_unless_present_any = ["eval", "repl"]` from the
    /// file argument, which is the one constraint standing between these five
    /// invocations and each other. A parse test rather than a pty one because
    /// four of the five never reach a terminal, and because what regressed
    /// would regress *in clap* — `--eval` still refusing a file beside it is
    /// the half a permissive change is most likely to take with it.
    #[test]
    fn dropping_the_required_file_left_every_other_invocation_alone() {
        let parse = |argv: &[&str]| {
            door::parser(Cli::command())
                .try_get_matches_from(argv)
                .map(|matches| Cli::from_arg_matches(&matches).expect("the shape is Cli's"))
        };

        let bare = parse(&["phosphor"]).expect("a bare phosphor parses");
        assert_eq!(bare.path, None);
        assert!(!bare.repl && bare.eval.is_none());

        assert_eq!(
            parse(&["phosphor", "notes.md"])
                .expect("a file parses")
                .path,
            Some(PathBuf::from("notes.md"))
        );
        assert!(parse(&["phosphor", "--repl"]).expect("--repl parses").repl);
        assert_eq!(
            parse(&["phosphor", "--eval", "(+ 1 2)"])
                .expect("--eval parses")
                .eval
                .as_deref(),
            Some("(+ 1 2)")
        );
        assert!(
            parse(&["phosphor", "--eval", "(+ 1 2)", "notes.md"]).is_err(),
            "--eval and a file are still refused together"
        );
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
