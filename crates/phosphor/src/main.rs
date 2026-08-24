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
//! all. `phosphor --eval '(…)'` and the 219 generated capability verbs return
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
use std::collections::{BTreeMap, BTreeSet};
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
    Action, AppAction, AskAction, BufferAction, FileAction, FloatAction, HistoryAction,
    InputAction, LspAction, MotionAction, Outcome, PaneAction, PickerAction, PromptAction, Receipt,
    Refusal, RegionAction, Request, RuntimeAction, SessionAction, ViewAction,
};
use phosphor_core::config;
use phosphor_core::input::key::{Code, Key, Mods, Named};
use phosphor_core::input::table::{Keymap, Layered, Resolution, Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::input::{Machine, key, text as motion};
use phosphor_core::journal::{self, Log, undo as wire_undo};
use phosphor_core::language::{self, Languages};
use phosphor_core::query::{Answer, Answers, Query, QueryError, RegionQuery, Revision};
use phosphor_core::registry::McpPolicy;
use phosphor_core::request::{
    AcceptHow, Actor, AnchorId, AskId, AskOption, Binding, BufferId, CharRange as SignatureRange,
    Completion as WireCompletion, Direction, EditMode, FileSpan, FoldState, KeySeq, LanguageId,
    PaneId, PaneKind, PaneRef, Position, PromptKind, RegionFilter, RegionId, RegisterName, Seek,
    SelectionKind, Sequence, Signature as WireSignature, SourceId, Span, Target, TextObject,
    ToolCallId, TurnId,
};
// `Scope` is already the input table's (`keymaps.scm`'s normal/insert/visual),
// and a second one under the same name in a 9,000-line file is a trap rather
// than an ambiguity the compiler catches — both are `Scope::File`-shaped
// enums.
use crate::picker::PickerSession;
use phosphor_agent::session::Life as SessionLife;
use phosphor_core::store::{
    Fingerprint, Lens, Scope as RegionScope, SeenState, Snapshot as AnchorSnapshot,
    SyntaxStep as AnchorStep,
};
use phosphor_core::value::{Args, Value, Wire as _};
use phosphor_core::view::{
    Axis as ViewAxis, Child, Constraint, Density, Emphasis, Float as ViewFloat, KeyHint, Millis,
    Mood, Node, SessionState, Slot, Tab, Tone, Tree,
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
use phosphor_ui::buffer_view::{self, Editor, StateMark, editor_area};
use phosphor_ui::diagnostics::{DiagnosticsVm, RowPolicy, RowScope, Tally};
use phosphor_ui::float::{
    self, Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody, FloatFooter,
    FloatHeader, FloatSlot, FooterHint, SignatureBody, SignatureVm, TextBody,
};
use phosphor_ui::frame::FrameCache;
use phosphor_ui::gutter::{self, Fill};
use phosphor_ui::interpret::{Interpreter, NoResources, Resources};
use phosphor_ui::key_hints::KeyHints;
use phosphor_ui::picker::PickerVm;
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::{BUILTIN_SLUGS, Theme, builtin};
use phosphor_ui::unknown_key::{self, UnknownKeyHint};
use phosphor_ui::virtual_text;
use ratatui::layout::Rect;
use ratatui::style::Style;
// The widget layer's re-export, not the fork's own path: after `T026` this file
// no longer talks to the vendored *handler* at all, only to the editor value
// `BufferView` draws. **The fork's `Undo`/`Redo` are gone with `R2`** — two live
// histories cannot both be the history, and the fork's truncates on divergence
// (`vendor/ratatui-code-editor/src/history.rs:19-22`), which is the behaviour
// `T029`'s tree exists not to have. One fork import is left: the selection type
// `SelectRange` sets.
// `Code` is already `input::key::Code`'s in this file — the keyboard's, not
// the buffer's — so the fork's arrives under a name that says which.
use ratatui_code_editor::code::Code as SourceCode;
use ratatui_code_editor::selection::Selection;

mod agent;
mod door;
mod events;
mod lsp;
mod picker;
mod store;

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

    /// Serve MCP on stdin and stdout — the agent's door (T052). Opens no
    /// terminal, exactly as `--eval` does, and speaks nothing else on those
    /// pipes: an agent spawns `phosphor --mcp` and every byte either way is
    /// JSON-RPC.
    ///
    /// **A flag rather than a subcommand.** The subcommand namespace is
    /// generated — one verb per capability, and
    /// `scripts/lint-one-registry.sh` holds the CLI module to being a total
    /// function of the table with no name of its own. A hand-written `mcp`
    /// verb would sit in that namespace as the one entry the registry did not
    /// put there.
    #[arg(long, conflicts_with_all = ["path", "eval", "repl"])]
    mcp: bool,
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

/// Which lines hang an inline `┊ ■` diagnostic row — `cursor-line`, `all` or
/// `off`.
///
/// **Reported at `CP-4`, and the report is the whole argument.** A half-typed
/// `path:` in `crates/phosphor/src/main.rs` made rust-analyzer answer with
/// eleven cascade parse errors — `expected COMMA`, `expected R_PAREN`,
/// `expected field declaration` — and [`DiagnosticsVm::rows`] mapped the set
/// one-to-one, so eleven rows of parser resynchronisation pushed the code being
/// edited off the screen.
///
/// Named in the editor layer for the same reason [`COMPLETION_MIN_CHARS`] is:
/// *how much should an error interrupt you* is a preference.
const DIAGNOSTIC_ROWS: &str = "diagnostic-rows";

/// The most inline rows one line may hang before the rest are counted.
const DIAGNOSTIC_MAX_ROWS: &str = "diagnostic-max-rows";

/// The row policy this pass draws with.
///
/// **Three surfaces, and this bounds only the third.** §3 gives a diagnostic
/// the state bar in gutter column 1, an undercurl under its span, and an inline
/// row — and the statusline's `■ N` counts every one of them
/// ([`phosphor_ui::diagnostics::Tally`], `2b`). So a policy that draws no row
/// hides nothing; it decides what *speaks*, and the default is the line you are
/// on, which is helix's default too.
/// What one buffer's decoration comes to for this frame.
///
/// Three answers with three audiences: the state column is the *widget's* and
/// is looked up per buffer through [`Resources::state_marks`], while the two
/// counts are the *statusline's* and are only ever wanted for the buffer the
/// user is looking at.
#[derive(Debug)]
struct Decorated {
    /// One [`StateMark`] per visual row, already resolved through §3's ladder.
    marks: Vec<StateMark>,
    /// How many diagnostics of each severity this buffer holds.
    tally: Tally,
    /// How many unseen regions are in it.
    unseen: usize,
}

/// Resolves everything that decorates one buffer, and installs what the fork
/// holds.
///
/// **Extracted at step 11b, and the extraction is the point.** This ran once
/// per frame against whichever buffer was on screen, which was right while
/// there was one. `Resources::state_marks` takes a `BufferId` and answered the
/// same column for every id it was handed — so a second pane showing a second
/// file would draw the *focused* file's error markers beside its text. Running
/// it per buffer is what makes that door able to tell them apart.
///
/// It mutates: `tints.sync`, `virtual_text::install` and `set_styled_spans`
/// all write to the editor, which is why it happens before the draw and not
/// during it. `Resources` has no `&mut` in it and must never grow one.
fn decorate(
    buffer: &mut Editing,
    store: &store::Shared,
    host: &AppHost,
    theme: &Theme,
) -> Decorated {
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
    let published = buffer
        .synced
        .as_ref()
        .map(|document| store.diagnostics_of(&document.key))
        .unwrap_or_default();
    let shown = DiagnosticsVm::new(&published);
    let tally = shown.tally();
    let mut regions = Vec::new();
    regions.extend(shown.regions(&buffer.editor));
    // **`T041` — the second source this `Vec` was built for.** The comment
    // above has said since `T040` that *"there is one source today; `T041`
    // adds the rest to this `Vec` and nothing else here changes"*, and
    // nothing else here does: the ladder in `gutter::resolve` folds the two
    // together, so a line carrying both an unseen edit and an error is
    // trouble-red by §3's own priority rather than by whichever source ran
    // second.
    //
    // Seen regions are handed over as well as unseen ones, deliberately.
    // `RegionState::Seen` resolves to `StateMark::None` — §3's row 18,
    // *"seen — marker cleared, line is plain"* — so the ladder is what
    // decides they draw nothing, in the one place that decides it.
    let unseen = buffer.file.as_deref().map_or(0, |path| {
        let spans: Vec<_> = store
            .spans_in(path)
            .into_iter()
            .map(|(span, state)| {
                (
                    span,
                    match state {
                        SeenState::Unseen => gutter::RegionState::Unseen,
                        SeenState::Seen => gutter::RegionState::Seen,
                    },
                )
            })
            .collect();
        regions.extend(gutter::spans(&buffer.editor, &spans));
        // `T087` — §3's row tints, through the fork's marks API. The same
        // `spans` the gutter's column is built from, so the tint and the
        // marker cannot disagree about a row.
        //
        // Called every frame and **uploads on almost none of them**:
        // `Tints::sync` diffs first, because `set_marks` replaces wholesale
        // and a 500-region file would otherwise re-upload the whole set on
        // every keystroke. That diff is what keeps this off the hot path.
        let (editor, tints) = (&mut buffer.editor, &mut buffer.tints);
        tints.sync(editor, theme, &spans);
        spans
            .iter()
            .filter(|(_, state)| *state == gutter::RegionState::Unseen)
            .count()
    });
    // **How many of them may speak, reported at `CP-4`.** A half-typed
    // `path:` made rust-analyzer answer with eleven cascade parse errors
    // and every one became a row, so the code being edited went off the
    // bottom of the screen. The policy is read per pass for the same
    // reason the completion floor and `soft-wrap` are: an option changed
    // at the REPL is a fact about now, not about the last restart.
    let rows = shown.rows(theme, &diagnostic_rows(host, &buffer.editor));
    // **`T041` gives a diagnostic's rail an owner, and that is what makes
    // it collapsible.** `phosphor_ui::diagnostics::rows` hands them back
    // unowned and says why: *"a region id is the store's and there are no
    // regions until `T041`, at which point a diagnostic's row is owned by
    // the region anchored to its node"*. Positional here, anchored at
    // `T042`; a row on a line no region covers stays unowned, which is
    // honest — there is nothing to collapse it by.
    //
    // The filter is the whole of `set-virtual-text-visible`'s per-owner
    // half: a collapsed owner's rows are not in the list installed, so the
    // fork's single global flag never has to become a per-region one.
    let rows: Vec<_> = buffer.file.clone().map_or(rows.clone(), |path| {
        rows.into_iter()
            .filter_map(|row| {
                let at = Position {
                    line: u32::try_from(row.anchor.line.saturating_add(1)).unwrap_or(u32::MAX),
                    column: u32::try_from(row.anchor.col.saturating_add(1)).unwrap_or(u32::MAX),
                };
                match store.covering(&path, at) {
                    Some(owner) if buffer.collapsed.contains(&owner) => None,
                    Some(owner) => Some(row.owned_by(owner)),
                    None => Some(row),
                }
            })
            .collect()
    });
    let underlines = shown.underlines(&buffer.editor, theme);
    virtual_text::install(&mut buffer.editor, &rows);
    buffer.editor.set_styled_spans(underlines);
    // As many rows as any region reaches and no more. `BufferView`'s own
    // contract is that *"rows past the end of the slice are
    // `StateMark::None`"*, so a column sized to the buffer would be the
    // same answer with a `Vec` the length of the file in it.
    let deepest = regions.iter().map(|region| region.rows.end).max();
    let marks = gutter::state_column(&regions, deepest.unwrap_or(0));
    Decorated {
        marks,
        tally,
        unseen,
    }
}

fn diagnostic_rows(host: &AppHost, editor: &Editor) -> RowPolicy {
    let scope = match host.text(DIAGNOSTIC_ROWS).as_deref() {
        Some("all") => RowScope::Everywhere,
        Some("off") => RowScope::Off,
        // An unset option is the default, and so is a *misspelled* one: this
        // is the frame path and there is nowhere here to say "that is not a
        // scope" that would not say it sixty times a second. The REPL is where
        // a typo is answered, and `set-option!` is where that belongs.
        _ => RowScope::CursorLine,
    };
    let max = host
        .number(DIAGNOSTIC_MAX_ROWS)
        .and_then(|most| usize::try_from(most).ok())
        .unwrap_or(RowPolicy::default().max);
    // `point` answers a 0-based (row, col) and `Anchor::line` is 0-based too,
    // so the two are the same number and neither is the statusline's 1-based
    // `line` ([`cursor_of`]). Comparing against the wrong one puts the rows on
    // the line above the cursor, which is exactly the off-by-one `T037`'s
    // statusline entry records having shipped once already.
    let (row, _) = editor.code_ref().point(editor.get_cursor());
    RowPolicy {
        scope,
        cursor: row,
        max,
    }
}

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
    /// `define-float-surface` — `T093`, and `OPEN-QUESTIONS.md` §43.
    ///
    /// The id and the scheme source, carried as text for the reason
    /// [`Intent::Keymap`] is: no `SteelVal` crosses the barrier, so a body
    /// arrives as source and the [`Layer`] is what evaluates it.
    DefineSurface(String, String),
    /// `open-float` — the surface id and its own arguments.
    OpenSurface(String, Value),
    /// `define-picker-source` — `T046`, and the same shape as
    /// [`Intent::DefineSurface`] for the same reason: a body crosses the
    /// barrier as **source text**, and the [`Layer`] is what evaluates it.
    DefineSource(String, String),
    /// `open-picker` from a **door** — the source id and a seed filter.
    ///
    /// A separate arm from the keystroke path rather than a shared one, and it
    /// is the seam `T041` established: this side has no editor, so it posts an
    /// ask and the loop performs it against `Editing`, which does.
    OpenPicker(String, Option<String>),
    /// `invalidate-picker-source` — drop a source's cached rows.
    InvalidateSource(String),
    /// `close-float` — §9's *"esc closes top-down"*, reached by a door instead
    /// of by the key.
    CloseFloat,
    /// `close-all-floats`. One slot today, so it is [`Intent::CloseFloat`] with
    /// a different name — and it stays a separate verb because §9's rule is
    /// *"at most one has focus"*, not *"at most one exists"*, and `T088`'s
    /// panes are where those stopped being the same sentence: a float per pane
    /// is expressible now, so *"close them all"* and *"close the focused one"*
    /// are two verbs rather than one under two names.
    CloseAllFloats,
    /// Something for the editor to say on the notice row (`T053`).
    ///
    /// **A door can already answer its caller and could not tell the *editor*
    /// anything.** `Receipt::note` reaches whoever made the call — the shell
    /// that ran `phosphor declare-review-block`, the agent that called the
    /// tool — and a review block's whole point is that it is news to the person
    /// at the terminal, who made no call at all. §6 puts that kind of sentence
    /// on the notice row, and this is how something on the other side of the
    /// VM reaches it.
    Say(String),
    /// `enqueue-ask` — `T059`'s producer, minted on the door's side and
    /// written to the queue on the loop's.
    ///
    /// **The id travels with the question rather than being allocated on
    /// arrival**, because the door has already answered its caller with it: an
    /// agent that asked has to be able to name the ask when the answer comes
    /// back, and a receipt for an id the loop had not chosen yet would be a
    /// promise about the future.
    Enqueue(AskId, phosphor_ui::question::QuestionVm),
    /// An `Ask`-rated action arriving through a door — the action and who sent
    /// it (`T060`).
    ///
    /// **The door cannot enqueue it itself**, because the queue is on `Shell`
    /// and this applier has no `Shell`. So the same three lines `deliver` runs
    /// for a posted action run in the loop for a door's, which is what keeps
    /// one rating from meaning two things depending on where the caller stood.
    Hold(Box<Action>, String),

    /// An Action for the loop to apply to the focused buffer (`T052`).
    ///
    /// **The one intent that carries a mutation rather than a request**, and it
    /// exists because there are two appliers in this program and the VM can
    /// only reach one of them. [`AppHost::apply`] is the layer's — the runtime,
    /// the floats, the store — and [`Editing::act`] is the loop's, holding the
    /// rope. A key reaches the second; scheme could not reach it at all, so
    /// every buffer-domain capability typed at `:repl` answered *"not built
    /// yet"* including ones that shipped three phases ago. `T052`'s
    /// `apply-edits` is the first capability whose acceptance says it must
    /// **work** from Steel, which is what made the gap a defect rather than a
    /// shape.
    ///
    /// **The outcome comes back as a notice, not as the caller's answer**, and
    /// that is the cost of the arrangement rather than an oversight.
    /// [`Intent::OpenPicker`] has the same one: `AppHost::apply` answers the
    /// scheme caller `done` because the loop has not run yet, so a refusal
    /// arrives on the notice row a frame later. Boxed because an [`Action`] is
    /// the largest thing this enum carries and every other variant would grow
    /// to match it.
    Act(Box<Action>),
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
    /// `T041`'s store — regions, seen-state and the diagnostics `T040` folded
    /// into it — shared with the loop. See [`crate::store`] for why one store
    /// has two handles.
    store: Arc<store::Shared>,
    /// The next ask id (`T059`), shared with the loop.
    ///
    /// **One counter with two holders, exactly like [`AppHost::store`]**, and
    /// for the reason that made that one shared: `enqueue-ask` is armed in
    /// *both* appliers — a door lands here and a keystroke lands in
    /// `Editing::act` — and two counters would hand two questions the same id
    /// the first time both doors were used in one session. `Shell` holds the
    /// same `Arc`.
    next_ask: Arc<Mutex<u64>>,
    /// Why the seen-state journal could not be opened, if it could not
    /// (`T044`).
    ///
    /// Held here rather than returned from [`stack`] because the loop is what
    /// has a notice row, and threading it through two call sites and every test
    /// that builds a stack would make the signature carry a fact only one
    /// caller reads.
    store_note: Option<String>,
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
    /// The open picker's source id and its rows, published by the loop so the
    /// `picker-rows` query has something to answer (`T045`, `T046`).
    ///
    /// **A published snapshot and not a live call, and the difference is
    /// `OPEN-QUESTIONS.md` §42.** Running a source is running scheme, and a
    /// query arrives from *inside* the VM — `Host::query` takes `&self` and
    /// must answer without re-entering the runtime. So the loop publishes what
    /// it derived on the frame it derived it, and the door reads that.
    ///
    /// The consequence is stated rather than hidden: this answers for the
    /// **open** picker only. A `picker-rows` naming some other source refuses
    /// by saying so, because the alternative is either a lie or the re-entrant
    /// query routing §42 rules is the right fix and does not have a caller
    /// urgent enough to build it yet.
    picker_rows: Option<(String, Vec<phosphor_core::view::SpanRow>)>,
    /// The heads it *offers* instead of writing. See [`OFFERED_HEADS`].
    offered: Vec<String>,
    /// The shape of the screen, as the loop last laid it out (`T088`).
    ///
    /// **Published rather than reached for**, which is the shape
    /// [`HostState::picker_rows`] already established and for the same reason:
    /// the panes are the *loop's*, this side answers queries on another
    /// thread, and a query that borrowed the live tree would be the re-entrant
    /// routing that pattern exists to avoid.
    panes: Option<Value>,
    /// The transcript, as the loop last published it (`T054`).
    ///
    /// **Published only when it moves**, unlike its two neighbours here: a
    /// transcript grows for as long as the editor is open, so a clone per frame
    /// would make an idle editor's cost a function of how much claude has said
    /// to it. `Transcript::revision` is what the loop compares.
    transcript: Option<Value>,
    /// The session, as the loop last saw it (`T051`).
    ///
    /// Published for [`HostState::panes`]' reason: the session client belongs
    /// to the loop and a query answers on another thread. **The same value the
    /// statusline was composed from**, which is what makes *"rendered
    /// identically everywhere it appears"* a property of the arrangement — the
    /// `session` query and §5's chrome cannot disagree, because there is one
    /// derivation and it happens once per frame.
    session: Option<Value>,
    /// `T099`'s registers, as the loop last published them.
    ///
    /// Published for `session`'s reason: the table lives on `Shell`, the loop
    /// owns `Shell`, and the `register` query is answered from this side of the
    /// barrier. A copy of one truth rather than a second original.
    registers: BTreeMap<String, String>,
    /// Which register `q` is recording into, or empty (`T099`).
    recording: String,
    /// `T060`'s queue, as the loop last published it.
    ///
    /// **Published rather than held, for `session`'s reason exactly**: the
    /// queue lives on `Shell`, the loop owns `Shell`, and Q9 asks that `]!`,
    /// the inbox and the statusline *"read one truth"*. Two collections would
    /// be two truths however carefully they were kept in step, so this is a
    /// copy of the one and never a second original.
    asks: Vec<Value>,
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
    /// A host whose store is restored from this workspace's journal (`T044`),
    /// and what went wrong if anything did.
    ///
    /// The reason it answers a notice rather than swallowing one is
    /// `Timeline::opened`'s: seen-state not surviving is worth saying out loud,
    /// and is never worth refusing to start over.
    fn opened(config: Option<PathBuf>) -> Self {
        let (store, complaint) = store::Shared::opened();
        let mut host = Self::new(config);
        host.store = Arc::new(store);
        host.store_note = complaint;
        host
    }

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
            store: Arc::new(store::Shared::default()),
            next_ask: Arc::new(Mutex::new(1)),
            store_note: None,
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
    /// Set a boolean option from outside the layer (`T096`).
    ///
    /// **One writer for one piece of state.** `--soft-wrap` is the only caller:
    /// a command-line flag is a *default* like `init.scm`'s, so it belongs in
    /// the same place rather than beside it. Everything else sets options
    /// through `set-option!`, which is the door.
    fn set_flag(&self, key: &str, value: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.options.insert(key.to_owned(), Value::Bool(value));
        }
    }

    fn flag(&self, key: &str) -> Option<bool> {
        match self.state.lock().ok()?.options.get(key)? {
            Value::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// A numeric option, or `None` if `init.scm` never set it.
    ///
    /// The reader [`AppHost::flag`] is for booleans and this is for counts.
    /// `Value` has one integer case on purpose
    /// (`phosphor_core::value::Value::Int`), so *every* number an option can
    /// carry — a minimum, a delay, a column — comes back through here.
    ///
    /// **This paragraph said *"and there is no third"* and now there is**
    /// ([`AppHost::text`]). It was true of every option that existed when it
    /// was written — all of them were counts or switches — and stopped being
    /// true the first time an option had to name one of several *behaviours*
    /// rather than tune a number. Recorded rather than quietly deleted,
    /// because the sentence was a claim about the option vocabulary and the
    /// vocabulary is what changed.
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

    /// A text option, or `None` if `init.scm` never set it.
    ///
    /// The third reader, for an option that names a *behaviour* rather than
    /// tuning a number — `(set-option! "diagnostic-rows" "cursor-line")`. A
    /// key set to the wrong case reads as unset, exactly as the two above
    /// treat theirs, and for the same reason: the loop reads an option long
    /// after `set-option!` returned, so there is nowhere to report a type
    /// error that is not a frame.
    ///
    /// **Cloned rather than borrowed**, because the lock cannot outlive this
    /// call and an option is read once per frame at most.
    fn text(&self, key: &str) -> Option<String> {
        match self.state.lock().ok()?.options.get(key)? {
            Value::Text(text) => Some(text.clone()),
            _ => None,
        }
    }

    /// Records a surface ask for the loop to carry out.
    fn ask(&self, intent: Intent) {
        if let Ok(mut state) = self.state.lock() {
            state.intents.push(intent);
        }
    }

    /// The next ask id (`T059`).
    ///
    /// **Minted here rather than in the loop**, so `enqueue-ask`'s receipt can
    /// carry it — see [`Intent::Enqueue`]. Monotonic and never reused: an
    /// answered ask's id coming back would address a different question.
    fn ask_id(&self) -> AskId {
        let mut next = self
            .next_ask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.saturating_add(1);
        AskId(id)
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

/// The target tags a door can resolve on its own.
///
/// **A query has no cursor.** Four `Target` arms mean something different
/// depending on where focus is (`request.rs`), and the thing that knows where
/// focus is, is the loop — `Editing::scope_of` is the other half of this split
/// and resolves all seven. So a query narrowed by `selection` is refused here
/// by naming the three it does take, rather than silently widening to the
/// workspace: a count that quietly answered about the wrong scope is worse than
/// one that did not answer.
const RESOLVABLE: &[&str] = &["file", "explicit", "region"];

impl AppHost {
    /// An [`Answer`] at the store's current revision.
    ///
    /// One place, so no arm can answer at `Revision::INITIAL` by forgetting —
    /// which is what every arm did before `T041` gave the store a revision to
    /// have.
    fn answered(&self, value: Value) -> Answer {
        Answer {
            value,
            revision: self.store.revision(),
        }
    }

    /// A [`Target`] as a store [`Scope`], for the arms a door can resolve.
    fn scope(
        &self,
        name: &'static str,
        within: Option<&Target>,
    ) -> Result<RegionScope, QueryError> {
        let Some(target) = within else {
            return Ok(RegionScope::Everywhere);
        };
        match target {
            Target::File { path } => Ok(RegionScope::File(store::key_for(path))),
            Target::Explicit { path, span } => Ok(RegionScope::Span {
                path: store::key_for(path),
                span: *span,
            }),
            Target::Region { id } => Ok(RegionScope::One(*id)),
            other => Err(QueryError::Argument {
                name,
                source: phosphor_core::value::WireError::Field {
                    field: "within",
                    source: Box::new(phosphor_core::value::WireError::Tag {
                        got: other.to_value().tag().unwrap_or("that").to_owned(),
                        expected: RESOLVABLE,
                    }),
                },
            }),
        }
    }

    /// **`mark-seen` and `mark-unseen`, from a door.** Answers how many
    /// regions were in scope.
    fn mark(&self, name: &'static str, target: &Target, state: SeenState) -> Outcome {
        match self.scope(name, Some(target)) {
            Ok(scope) => {
                let marked = self.store.set_seen(&scope, state);
                Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Int(i64::try_from(marked).unwrap_or(0)),
                    // §41, the door's half. There is no notice row on this side
                    // — the receipt *is* what the caller reads — so the same
                    // sentence rides it and nothing else is needed.
                    note: (marked == 0).then(|| "no region here".to_owned()),
                })
            }
            Err(why) => declined(&why.to_string()),
        }
    }

    /// The file's text and grammar, parsed the way the editor would parse it.
    ///
    /// **Off disk, and that is the honest source for a door.** An agent asking
    /// to anchor `src/retry.rs:24` is talking about the file; a buffer's
    /// unsaved state is the editor's business and this side has no editor. The
    /// grammar comes from the same [`grammar_of`] the loop uses, so a
    /// door-placed anchor fingerprints at the same fidelity as `m` — which is
    /// what stops "placed over MCP" from meaning "resolves one tier worse".
    fn parse(&self, path: &Path) -> Option<SourceCode> {
        let text = std::fs::read_to_string(path).ok()?;
        let grammar = {
            let languages = self.languages.lock().ok()?;
            grammar_of(&languages, path).to_owned()
        };
        SourceCode::new(&text, &grammar, None).ok()
    }

    /// A fingerprint of a 1-based line in a file on disk.
    fn fingerprint_of(&self, path: &Path, line: u32) -> Fingerprint {
        let Some(code) = self.parse(path) else {
            return Fingerprint::new(Vec::new(), "", line);
        };
        let index = usize::try_from(line.saturating_sub(1)).unwrap_or(0);
        let text = if index < code.len_lines() {
            code.line(index).to_string()
        } else {
            String::new()
        };
        let byte = code.char_to_byte(code.line_to_char(index.min(code.len_lines())));
        let syntax: Vec<AnchorStep> = code
            .syntax_path(byte)
            .into_iter()
            .map(|step| AnchorStep::new(step.kind, step.name))
            .collect();
        Fingerprint::new(syntax, &text, line)
    }

    /// The file's lines and syntax, as the store's [`AnchorSnapshot`].
    ///
    /// Shared by `reanchor` and by the fingerprinting a declaration triggers,
    /// so the two cannot describe the same file differently.
    fn snapshot_of(&self, path: &Path) -> Option<AnchorSnapshot> {
        let code = self.parse(path)?;
        let text = code.get_content();
        let mut snapshot = AnchorSnapshot::of(&text);
        for line in 0..snapshot.len() {
            let byte = code.char_to_byte(code.line_to_char(line));
            let steps: Vec<AnchorStep> = code
                .syntax_path(byte)
                .into_iter()
                .map(|step| AnchorStep::new(step.kind, step.name))
                .collect();
            if !steps.is_empty() {
                snapshot = snapshot.with_syntax(line, steps);
            }
        }
        Some(snapshot)
    }

    /// Fingerprint the regions of every file a declaration named (`T043`).
    ///
    /// One parse per *distinct* path rather than per spec: a declaration of
    /// forty spans in one file is the ordinary shape, and parsing it forty
    /// times would make the common case the expensive one.
    fn fingerprint_declared(&self, regions: &[phosphor_core::request::RegionSpec]) {
        let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
        for spec in regions {
            let key = store::key_for(&spec.path);
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(snapshot) = self.snapshot_of(&key) {
                self.store.fingerprint_regions(&key, &snapshot);
            }
        }
    }

    /// **`place-anchor`, from a door.**
    fn place_anchor(&self, name: &'static str, at: &Target, label: Option<&String>) -> Outcome {
        let scope = match self.scope(name, Some(at)) {
            Ok(scope) => scope,
            Err(why) => return declined(&why.to_string()),
        };
        let (path, span) = match scope {
            RegionScope::Span { path, span } => (path, span),
            // A whole file is not a place. `declare-regions` is the verb for
            // "this file has claude-written spans"; an anchor names one point.
            RegionScope::File(_) | RegionScope::One(_) | RegionScope::Everywhere => {
                return declined("anchor a place — name a path and a span");
            }
        };
        let fingerprint = self.fingerprint_of(&path, span.start.line);
        let id = self
            .store
            .place_anchor(path, span, label.cloned(), fingerprint);
        Outcome::Done(Receipt {
            capability: name,
            value: id.to_value(),
            note: None,
        })
    }

    /// **`reanchor`, from a door.** Re-resolves one file's anchors against the
    /// text now on disk.
    fn reanchor_file(&self, name: &'static str, path: &Path) -> Outcome {
        let key = store::key_for(path);
        let Some(snapshot) = self.snapshot_of(&key) else {
            return declined("cannot read that file");
        };
        let outcome = self.store.reanchor(&key, &snapshot);
        Outcome::Done(Receipt {
            capability: name,
            value: outcome.to_value(),
            note: (!outcome.lost.is_empty())
                .then(|| format!("{} anchor(s) lost", outcome.lost.len())),
        })
    }

    /// Publish the screen's shape, so `panes` has an answer (`T088`).
    fn publish_panes(&self, shape: Value) {
        if let Ok(mut state) = self.state.lock() {
            state.panes = Some(shape);
        }
    }

    /// The turns the loop last published, oldest first (`T054`).
    fn turns(&self) -> Vec<Value> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.transcript.clone())
            .map_or_else(Vec::new, |turns| match turns {
                Value::List(turns) => turns,
                _ => Vec::new(),
            })
    }

    /// Publish the transcript, so `turns` and `turn` have an answer (`T054`).
    fn publish_transcript(&self, turns: Value) {
        if let Ok(mut state) = self.state.lock() {
            state.transcript = Some(turns);
        }
    }

    /// Publish the session, so `session` has an answer (`T051`).
    fn publish_session(&self, session: Value) {
        if let Ok(mut state) = self.state.lock() {
            state.session = Some(session);
        }
    }

    /// Publish the registers, so `register` has an answer (`T099`).
    fn publish_registers(&self, registers: BTreeMap<String, String>, recording: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.registers = registers;
            recording.clone_into(&mut state.recording);
        }
    }

    /// Publish the ask queue, so `pending-asks` and `ask` have an answer
    /// (`T060`).
    fn publish_asks(&self, asks: Vec<Value>) {
        if let Ok(mut state) = self.state.lock() {
            state.asks = asks;
        }
    }

    /// Publish what the loop derived, so `picker-rows` has an answer
    /// (`T046`).
    fn publish_picker(&self, rows: Option<(String, Vec<phosphor_core::view::SpanRow>)>) {
        if let Ok(mut state) = self.state.lock() {
            state.picker_rows = rows;
        }
    }

    /// A [`RegionFilter`] as a store [`Lens`].
    fn lens(&self, name: &'static str, filter: Option<&RegionFilter>) -> Result<Lens, QueryError> {
        let Some(filter) = filter else {
            return Ok(Lens::everything());
        };
        Ok(Lens {
            author: filter.author,
            unseen_only: filter.unseen_only,
            within: self.scope(name, filter.within.as_ref())?,
        })
    }
}

impl Answers for AppHost {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        let name = query.spec().name;
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
            // `T088` — the pane tree and which one has focus.
            //
            // **The last frame's shape, not this instant's**, on
            // `picker-rows`' terms: an answer derived on the frame that drew
            // it is the honest one for a question about what is on screen, and
            // the alternative is a query reaching into the loop's own state
            // from another thread.
            //
            // An empty answer before the first frame is not a failure — it is
            // `query.rs`'s *"an absent thing answers empty"*, and there is
            // genuinely no screen yet.
            Query::Ui(phosphor_core::query::UiQuery::Panes {}) => Ok(Answer {
                value: self
                    .state
                    .lock()
                    .ok()
                    .and_then(|state| state.panes.clone())
                    .unwrap_or(Value::Null),
                revision: Revision::INITIAL,
            }),
            // `T051` — *"the session's state — what the statusline's ✻ and
            // elapsed timer render"*, and it is **the same value they render**
            // rather than a second derivation of it: the loop composes
            // `StatusVm` once per frame and publishes the answer here, so a
            // surface that asks and the strip that draws cannot disagree.
            //
            // `Value::Null` before the first frame, which is `query.rs`'s
            // *"an absent thing answers empty"* — there is genuinely no session
            // and no frame yet.
            Query::Session(phosphor_core::query::SessionQuery::Session {}) => Ok(Answer {
                value: self
                    .state
                    .lock()
                    .ok()
                    .and_then(|state| state.session.clone())
                    .unwrap_or(Value::Null),
                revision: Revision::INITIAL,
            }),
            // `T099` — what a register holds, as the loop last published it.
            // **One table behind three readers**: `@` feeds it back, `"ap`
            // pastes it, and the door reads it. An unset one is empty, which is
            // the row's own wording and not an error — asking what an untouched
            // register holds is a question with a legitimate *"nothing"*.
            Query::Input(phosphor_core::query::InputQuery::Register { register }) => Ok(Answer {
                value: Value::Text(
                    self.state
                        .lock()
                        .ok()
                        .and_then(|state| state.registers.get(&register.0).cloned())
                        .unwrap_or_default(),
                ),
                revision: Revision::INITIAL,
            }),
            // `T099` — which register `q` is filling, or empty. **Published like
            // the rest**, because the recorder is the *machine's* and this side
            // of the barrier has no machine: the loop reads
            // `Machine::recording` once a pass and lends the answer.
            Query::Input(phosphor_core::query::InputQuery::Recording {}) => Ok(Answer {
                value: Value::Text(
                    self.state
                        .lock()
                        .ok()
                        .map(|state| state.recording.clone())
                        .unwrap_or_default(),
                ),
                revision: Revision::INITIAL,
            }),
            // `T060` — the queue, oldest first. **One list behind three
            // readers**: `]!` walks it, the statusline's `!` counts it, and
            // `T067`'s inbox will list it.
            Query::Session(phosphor_core::query::SessionQuery::PendingAsks {}) => Ok(Answer {
                value: Value::List(
                    self.state
                        .lock()
                        .ok()
                        .map(|state| state.asks.clone())
                        .unwrap_or_default(),
                ),
                revision: Revision::INITIAL,
            }),
            // One of them, by id. **`Null` rather than an error for an id the
            // queue does not have**, which is `region`'s rule and the same
            // legitimate no: a surface holding an id across an answer asks it.
            Query::Session(phosphor_core::query::SessionQuery::Ask { ask }) => Ok(Answer {
                value: self
                    .state
                    .lock()
                    .ok()
                    .and_then(|state| {
                        state
                            .asks
                            .iter()
                            .find(|value| ask_id_of(value) == Some(ask.0))
                            .cloned()
                    })
                    .unwrap_or(Value::Null),
                revision: Revision::INITIAL,
            }),
            // `T054` — the transcript, newest last, as the loop last
            // published it. [`HostState::transcript`] for why it is published
            // rather than reached for.
            Query::Session(phosphor_core::query::SessionQuery::Turns { limit, offset }) => {
                let turns = self.turns();
                let skipped = offset.map_or(0, |offset| offset as usize);
                let kept: Vec<Value> = turns
                    .into_iter()
                    .skip(skipped)
                    .take(limit.map_or(usize::MAX, |limit| limit as usize))
                    .collect();
                Ok(Answer {
                    value: Value::List(kept),
                    revision: Revision::INITIAL,
                })
            }
            Query::Session(phosphor_core::query::SessionQuery::Turn { turn }) => {
                // Matched on the turn's own id rather than by position: the
                // list is *"newest last"* and a caller that had paged through
                // it would otherwise index into a list that grew underneath.
                let wanted = i64::try_from(turn.0).unwrap_or(i64::MAX);
                let found = self.turns().into_iter().find(|held| {
                    matches!(held, Value::Record(fields) if fields.get("turn") == Some(&Value::Int(wanted)))
                });
                Ok(Answer {
                    value: found.unwrap_or(Value::Null),
                    revision: Revision::INITIAL,
                })
            }
            // `T053` — every declared block, oldest first. Off the same
            // store the markers came from, so a block and its regions cannot
            // disagree about what landed.
            Query::Review(phosphor_core::query::ReviewQuery::ReviewBlocks {}) => Ok(Answer {
                value: Value::List(self.store.blocks().iter().map(block_value).collect()),
                revision: self.store.revision(),
            }),
            // `T040`. Answered off the same store the gutter draws from.
            Query::Review(phosphor_core::query::ReviewQuery::Diagnostics { path }) => Ok(Answer {
                value: Value::List(self.store.answer_diagnostics(path.as_deref())),
                revision: self.store.revision(),
            }),
            // `T041` — invariant 4's core. Every one of these is a read of the
            // same store the gutter and the statusline draw from, at the same
            // revision, which is what "every surface is a query over this"
            // means when it is true rather than claimed.
            Query::Region(RegionQuery::Regions { filter }) => {
                let lens = self.lens(name, filter.as_ref())?;
                Ok(self.answered(Value::List(self.store.answer_regions(&lens))))
            }
            Query::Region(RegionQuery::UnseenRegions { path }) => {
                Ok(self.answered(Value::List(self.store.answer_unseen(path.as_deref()))))
            }
            Query::Region(RegionQuery::Region { region }) => self
                .store
                .answer_region(*region)
                .map(|value| self.answered(value))
                // Not a `NotYetImplemented`: the query is built and the id is
                // wrong. `Null` rather than an error, because *"is there a
                // region with this id"* is a question with a legitimate no —
                // a surface holding an id across a `drop-regions` asks it.
                .map_or_else(|| Ok(self.answered(Value::Null)), Ok),
            Query::Region(RegionQuery::UnseenCount { within }) => {
                let scope = self.scope(name, within.as_ref())?;
                Ok(self.answered(Value::Int(
                    i64::try_from(self.store.unseen_count(&scope)).unwrap_or(0),
                )))
            }
            Query::Region(RegionQuery::SeenCount { within }) => {
                let scope = self.scope(name, within.as_ref())?;
                Ok(self.answered(Value::Int(
                    i64::try_from(self.store.seen_count(&scope)).unwrap_or(0),
                )))
            }
            // `T042`. Read-only, so both of these answer from the door side
            // with no editor involved — an anchor's span is already resolved
            // and the store is the only thing that has to be asked.
            // `T048` — Q11's *"the workspace's shape, for `:arch` — a store
            // query with no Rust primitive"*.
            //
            // **Counts, not a drawing.** `6a`'s boxes are `runtime/arch.scm`'s
            // and are built from the `spans` hatch; what this answers is the
            // handful of numbers that make the picture *"reflect the actual
            // store rather than a static drawing"*. Anything shaped like a
            // layout here would be the Rust primitive the task exists to prove
            // unnecessary.
            Query::Ui(phosphor_core::query::UiQuery::Arch {}) => {
                let scope = phosphor_core::store::Scope::Everywhere;
                let languages = self
                    .languages
                    .lock()
                    .map(|held| held.len())
                    .unwrap_or_default();
                Ok(self.answered(Value::Record(
                    Args::new()
                        .with("unseen", count(self.store.unseen_count(&scope)))
                        .with("seen", count(self.store.seen_count(&scope)))
                        .with("anchors", count(self.store.anchor_count()))
                        .with("diagnostics", count(self.store.diagnostic_count()))
                        .with("languages", count(languages)),
                )))
            }
            // `T045`'s query half, landed with `T046` because it needs the
            // source registry to have anything to answer about.
            //
            // **Every miss answers an empty list**, which is `query.rs`'s own
            // rule — *"an absent thing answers empty"* — and not a shortcut
            // taken for want of a refusal: `QueryError` has `Unknown`,
            // `Argument` and `NotYetImplemented` and no fourth arm, because a
            // query that found nothing has not failed. A source nobody has
            // opened genuinely has no rows.
            //
            // What it answers for is the **open** picker, and that limit is
            // `OPEN-QUESTIONS.md` §42: running a source is running scheme, a
            // query arrives from inside the VM, and `Host::query` takes `&self`
            // and cannot re-enter the runtime. So the loop publishes what it
            // derived on the frame it derived it (`HostState::picker_rows`) and
            // this reads that.
            Query::Ui(phosphor_core::query::UiQuery::PickerRows {
                source,
                query,
                limit,
                offset,
            }) => {
                let held = self
                    .state
                    .lock()
                    .ok()
                    .and_then(|state| state.picker_rows.clone());
                let rows = held
                    .filter(|(open, _)| open == &source.0)
                    // A filter argument would have to be matched, and matching
                    // is nucleo's and the loop's. `set-picker-query` is the
                    // verb that changes what is on screen; this reads it.
                    .filter(|_| query.as_ref().is_none_or(String::is_empty))
                    .map(|(_, rows)| rows)
                    .unwrap_or_default();
                let skip = usize::try_from(offset.unwrap_or(0)).unwrap_or(0);
                let take = limit.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX));
                Ok(self.answered(Value::List(
                    rows.iter()
                        .skip(skip)
                        .take(take)
                        .map(phosphor_core::view::SpanRow::to_value)
                        .collect(),
                )))
            }
            Query::Region(RegionQuery::Anchors { path }) => Ok(self.answered(Value::List(
                self.store.answer_anchors(&store::key_for(path)),
            ))),
            Query::Region(RegionQuery::Anchor { anchor }) => self
                .store
                .answer_anchor(*anchor)
                // `Null` for a wrong id, for the reason `region` gives above:
                // a surface holding an id across a `reanchor` that dropped it
                // is asking a question with a legitimate no.
                .map_or_else(
                    || Ok(self.answered(Value::Null)),
                    |value| Ok(self.answered(value)),
                ),
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
            // `5d`'s *"discovery finds running agents"*, and it finds none.
            //
            // **An empty list is the truthful answer and not a stub.** The two
            // things the mockup draws are a tmux pane (`%3`) and a headless
            // socket (`~/.claude/sock/4f2a`); reaching the first needs tmux
            // control mode, which `docs/TASKS.md` puts at v1.5, and the second
            // needs a socket transport this build does not have — `T050`'s
            // client speaks stdio to a child it owns. So there is nowhere to
            // look, and answering a guess would put a row on `5d` that `↵`
            // could not adopt.
            Action::Session(SessionAction::DiscoverSessions {}) => Outcome::Done(Receipt {
                capability: name,
                value: Value::List(Vec::new()),
                note: Some(
                    "no way to find a running agent yet — v1.5's tmux control mode".to_owned(),
                ),
            }),
            // **Armed here rather than in `Editing::act`, and that is where a
            // Steel caller lands.** `runtime/dashboard.scm` calls this to draw
            // `5d`'s list, and a surface body runs in the VM — which reaches
            // this applier and not the loop's. It needs no buffer and no
            // session handle, so there is nothing to reach for.

            // `T052` — **the door into the loop's applier**, and deliberately
            // one capability wide.
            //
            // A blanket `Action::Buffer(_)` arm here would be worse than the
            // gap it closes: every buffer capability the loop has not armed
            // would stop answering `#refused · not built yet — Txxx builds it`
            // and start answering `#done` for something that never happened.
            // The honest refusal is what the CLI and MCP doors are *for* at
            // this phase, and `parity.rs` reads it. So this is the one
            // capability whose acceptance says it must work from Steel, and
            // the next one adds its own line.
            Action::Buffer(BufferAction::ApplyEdits { .. }) => {
                self.ask(Intent::Act(Box::new(request.action.clone())));
                done(Value::Null)
            }
            // `T056` — **the second line, and the reason the shape is one
            // capability wide rather than a blanket arm.** `goto-location`'s
            // own sentence names its callers — *"a picker accept, a transcript
            // tool row, an OSC 8 link"* — and every one of them arrives through
            // a *door*, which is this applier. The loop's arm holds the rope
            // and cannot be reached from here without saying so.
            //
            // Found by running it: the verb had an arm in `Editing::act`, the
            // arms lint was clean, and `(goto-location! …)` at the REPL still
            // answered `#refused · not built yet — T056 builds it`. The same
            // shape as `discover-sessions` in `T057`, one applier the other
            // way round.
            Action::Motion(MotionAction::GotoLocation { .. }) => {
                self.ask(Intent::Act(Box::new(request.action.clone())));
                done(Value::Null)
            }
            // `T096` — **the fourth line, and the pattern is now worth naming.**
            // A capability armed in `Editing::act` is reachable from a *key*;
            // a door lands here, and the two appliers do not fall through to
            // one another on purpose (see the comment on `apply-edits` above).
            // So a verb whose whole point is that three doors can call it needs
            // a line here, and `set-soft-wrap` is exactly that verb: it was
            // declared, generated into Steel, MCP and the CLI, armed in the
            // loop, and still answered `not built yet — T081 builds it` from
            // every door. Found by running it.
            Action::View(ViewAction::SetSoftWrap { .. }) => {
                self.ask(Intent::Act(Box::new(request.action.clone())));
                done(Value::Null)
            }
            // `T060` — **the third line, and the first that does not apply.**
            // `apply-workspace-edit` is rated `Ask`, and the rating is about the
            // *action*, not about which door it came through: a rename arriving
            // from Steel is as much a thing you have to say yes to as one
            // arriving from an LSP client. So this queues a question rather
            // than forwarding the edit, exactly as `deliver` does for a posted
            // one.
            Action::Lsp(LspAction::ApplyWorkspaceEdit { .. }) => {
                self.ask(Intent::Hold(
                    Box::new(request.action.clone()),
                    format!("{:?}", request.door).to_lowercase(),
                ));
                done(Value::Null)
            }
            // -- `T059`: `4a`, claude asking mid-turn --------------------------
            //
            // **Both arms are here rather than in `Editing::act`, and neither
            // touches a buffer.** An ask arrives through the agent's door and
            // is answered from a float body, which runs in the VM — the same
            // argument `discover-sessions` makes one task earlier, and the same
            // one that makes these two `Intent`s rather than direct writes: the
            // queue lives on `Shell`, which the loop owns.
            //
            // `enqueue-ask` is declared `[S6 / "T060"]` and armed here, which is
            // deliberate. `T060`'s *Done when* is about the **queue** — waiting
            // behind a float that has focus, `]!`, the `!` surviving a 40-column
            // shed, one store query behind all three — and none of that is the
            // verb existing. `4a` cannot reproduce in a running binary without
            // something that produces a question, so the producer lands with the
            // screen and the queueing lands with the queue.
            Action::Ask(AskAction::EnqueueAsk { prose, options }) => {
                let id = self.ask_id();
                self.ask(Intent::Enqueue(
                    id,
                    phosphor_ui::question::QuestionVm {
                        prose: prose.clone(),
                        options: options.clone(),
                    },
                ));
                // (The loop performs it; see `Shell::enqueue_ask`.)
                // **The id is the receipt**, because an agent that asked has to
                // be able to say which ask it asked when the answer comes back.
                done(Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)))
            }
            // **`answer-ask` is deliberately *not* armed here.** It is rated
            // `Deny` — a person only — so no door reaches this applier with
            // one, and an arm would be a path nothing can take. The loop's
            // applier has it, which is where a keystroke lands.
            // `T093` — the four float verbs, and the registry `OPEN-QUESTIONS.md`
            // §43 found missing. All four post an [`Intent`] rather than acting:
            // composing a surface runs scheme, and a binding is already inside
            // the VM when it calls this.
            //
            // **The id is validated here, not in the layer**, because this is
            // the door: `define-float-surface` is `Allow` on MCP and its id is
            // interpolated into a `define` form, so an unchecked one is scheme
            // injection from an agent.
            Action::Float(FloatAction::DefineFloatSurface { surface, body }) => {
                if phosphor_steel::float::valid_surface_id(&surface.0) {
                    self.ask(Intent::DefineSurface(surface.0.clone(), body.clone()));
                    done(Value::Null)
                } else {
                    declined(
                        &phosphor_steel::float::SurfaceError::BadId(surface.0.clone()).to_string(),
                    )
                }
            }
            // `T046` — the picker's three door-side verbs, shaped exactly like
            // the float surface registry above and for the same reasons: an id
            // is validated because it is interpolated into a `define` form, a
            // body crosses as source text, and the work is an `Intent` because
            // this side has no editor to open a picker in.
            Action::Picker(PickerAction::DefinePickerSource { source, body }) => {
                if phosphor_steel::picker::valid_source_id(&source.0) {
                    self.ask(Intent::DefineSource(source.0.clone(), body.clone()));
                    done(Value::Null)
                } else {
                    declined(
                        &phosphor_steel::picker::SourceError::BadId(source.0.clone()).to_string(),
                    )
                }
            }
            Action::Picker(PickerAction::OpenPicker { source, query }) => {
                self.ask(Intent::OpenPicker(source.0.clone(), query.clone()));
                done(Value::Null)
            }
            Action::Picker(PickerAction::InvalidatePickerSource { source }) => {
                self.ask(Intent::InvalidateSource(source.0.clone()));
                done(Value::Null)
            }
            Action::Float(FloatAction::OpenFloat { surface, args }) => {
                self.ask(Intent::OpenSurface(
                    surface.0.clone(),
                    Value::Record(args.clone()),
                ));
                done(Value::Null)
            }
            Action::Float(FloatAction::CloseFloat {}) => {
                self.ask(Intent::CloseFloat);
                done(Value::Null)
            }
            Action::Float(FloatAction::CloseAllFloats {}) => {
                self.ask(Intent::CloseAllFloats);
                done(Value::Null)
            }
            // `T041` — §7's state machine, reached from a door rather than from
            // a keystroke. `Editing::act` has the same four; the difference is
            // that this side has no editor, so a focus-relative target is
            // refused by name here and resolved there. Invariant 2 is kept by
            // both applying to the *same* store, not by both being able to
            // resolve the same targets — a door with no cursor genuinely does
            // not know what `selection` means.
            Action::Region(RegionAction::DeclareRegions { regions }) => {
                let answer = declared(name, &self.store.declare(regions, request.actor));
                // `T043`'s door half. An agent declaring regions is the most
                // common way regions are created at all, so leaving *those*
                // positional would make "markers survive a rewrite" true only
                // for the ones a person declared by hand.
                self.fingerprint_declared(regions);
                Outcome::Done(answer)
            }
            // `T053` — Q6's review-block signal. **The agent's door and only
            // the agent's**: nobody types a file-and-span list, which is why
            // this is armed here rather than in `Editing::act` and why
            // `lint-capability-bindings` carries it as emitted.
            Action::Review(phosphor_core::action::ReviewAction::DeclareReviewBlock {
                title,
                files,
                annotation,
            }) => {
                // **`Actor::Claude`, whoever called it**, and that is the
                // capability rather than a shortcut. §7 rules that *"your own
                // edits never create regions: the machine tracks claude only"*,
                // and `FileGroup` carries no author to disagree with — unlike
                // `RegionSpec`, which does, because `declare-regions` is the
                // general verb. A review block *is* the claim that claude wrote
                // these spans; declaring one with `request.actor` would make
                // the same call from `:repl` produce zero markers and a
                // notification about them, which is the worst of both.
                let block =
                    self.store
                        .declare_block(title, files, annotation.as_deref(), Actor::Claude);
                // `T043`'s door half, the same one `declare-regions` does: a
                // region declared through a door has to survive a rewrite too,
                // and the fingerprint is taken against the text it was declared
                // against rather than whatever has since moved onto that line.
                let specs: Vec<phosphor_core::request::RegionSpec> = files
                    .iter()
                    .flat_map(|file| {
                        file.spans
                            .iter()
                            .map(|span| phosphor_core::request::RegionSpec {
                                path: file.path.clone(),
                                span: *span,
                                author: Actor::Claude,
                            })
                    })
                    .collect();
                self.fingerprint_declared(&specs);
                // **The notification, in `1b`'s own words.** The mockup draws
                // the seam as `✻ review ready · retry logic — 2 files, 6
                // regions`, and this is that sentence: §6's midline dot inside
                // one fact, an em dash before the count.
                let regions: usize = block.groups.iter().map(|group| group.regions.len()).sum();
                let said = format!(
                    "review ready · {} — {} file(s), {regions} region(s)",
                    block.title,
                    block.groups.len(),
                );
                self.ask(Intent::Say(said));
                Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Int(i64::try_from(block.id.0).unwrap_or(i64::MAX)),
                    note: block.annotation.clone(),
                })
            }
            Action::Region(RegionAction::MarkSeen { target }) => {
                self.mark(name, target, SeenState::Seen)
            }
            Action::Region(RegionAction::MarkUnseen { target }) => {
                self.mark(name, target, SeenState::Unseen)
            }
            // `T042` — the door's half of anchors. Same seam as the four
            // above: no editor here, so an explicit target is honoured and a
            // focus-relative one is refused by name.
            //
            // **The text comes off disk**, and that is the honest source for a
            // door: an agent asking to anchor `src/retry.rs:24` is talking
            // about the file, not about whatever unsaved state a buffer holds.
            // The fingerprint is full-fidelity — [`AppHost::fingerprint_of`]
            // parses with the same grammar the editor would — so an anchor
            // placed over MCP resolves at the node tier exactly like one placed
            // by `m`.
            Action::Region(RegionAction::PlaceAnchor { at, label }) => {
                self.place_anchor(name, at, label.as_ref())
            }
            Action::Region(RegionAction::Reanchor { path }) => self.reanchor_file(name, path),
            Action::Region(RegionAction::DropRegions { target }) => {
                match self.scope(name, Some(target)) {
                    Ok(scope) => Outcome::Done(Receipt {
                        capability: name,
                        value: Value::Int(
                            i64::try_from(self.store.drop_regions(&scope)).unwrap_or(0),
                        ),
                        note: None,
                    }),
                    Err(why) => declined(&why.to_string()),
                }
            }
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

/// What `declare-regions` answers.
///
/// The count of regions that now exist because of this call, and a note when
/// §7 dropped part of the batch on the floor. **Silence there would be the
/// wrong receipt**: a door that declared six spans and got `#ok` back has no
/// way to learn that four of them claimed an author the machine does not track,
/// and *"your own edits never create regions"* is a rule it is better to be
/// told about once than to discover from an empty gutter.
fn declared(capability: &'static str, declared: &phosphor_core::store::Declared) -> Receipt {
    let landed = declared.created.len() + declared.revised.len();
    Receipt {
        capability,
        value: Value::Int(i64::try_from(landed).unwrap_or(0)),
        note: (declared.ignored > 0).then(|| {
            format!(
                "{} not claude's — only claude's writes become regions",
                declared.ignored
            )
        }),
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

    /// `T093` — one registered float surface, called with its own arguments.
    ///
    /// The door's half of §43. Goes through this type for the same reason
    /// [`Layer::compose`] does: `Layer` owns the `Runtime` and is the only way
    /// into the VM, so *"arbitrary scheme ran, invalidate the frame"* stays
    /// structural rather than remembered.
    fn surface(
        &mut self,
        id: &str,
        args: &Value,
    ) -> Result<phosphor_core::view::Float, phosphor_steel::float::SurfaceError> {
        phosphor_steel::float::surface(&mut self.runtime, id, args)
    }

    /// `T046` — one registered picker source, called for its rows.
    ///
    /// Through this type for [`Layer::surface`]'s reason: `Layer` owns the
    /// `Runtime` and is the only way into the VM, so *"arbitrary scheme ran,
    /// invalidate the frame"* stays structural.
    ///
    /// Called on **every open**, which is what `define-picker-source`'s own
    /// doc means by *"an open picker re-derives from it"* — a float is a
    /// snapshot of an answer and a picker is a live query.
    fn source(
        &mut self,
        id: &str,
        args: &Value,
    ) -> Result<Vec<phosphor_core::view::SpanRow>, phosphor_steel::picker::SourceError> {
        phosphor_steel::picker::rows(&mut self.runtime, id, args)
    }

    /// `T047` — the source order tab cycles, as the layer declares it.
    ///
    /// Reads a global; runs nothing. Through this type anyway, because `Layer`
    /// owns the `Runtime` and nothing else may hold one.
    fn source_order(&mut self) -> Vec<String> {
        phosphor_steel::picker::order(&mut self.runtime)
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
    let host = Arc::new(AppHost::opened(config));
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

/// One tool call, on its way to the VM's thread.
///
/// The reply channel travels with the question for [`Answer`](phosphor_buffer::lsp)'s
/// reason one door over: a caller that asked must be answered on every path,
/// and a `Sender` dropped by a thread that ended answers `Err` at the receiver
/// rather than hanging it.
type Ask = (
    String,
    serde_json::Map<String, serde_json::Value>,
    std::sync::mpsc::Sender<Result<String, String>>,
);

/// The editor an MCP tool call reaches.
///
/// **A channel, because the VM cannot cross a thread.** `rmcp` hands the
/// handler out by shared reference and requires `Send + Sync`; [`Layer`] owns a
/// `steel-core` runtime built on `Rc`, so it is `!Send` and always will be.
/// The reconciliation is the one this build already uses for the LSP client and
/// the ACP session: the thing that cannot move gets a thread of its own and is
/// reached by message.
///
/// Serialising tool calls is not a cost of that arrangement — it is what one
/// runtime means. Two concurrent calls into one VM would have to serialise
/// somewhere.
struct McpEditor {
    asks: std::sync::mpsc::Sender<Ask>,
}

impl phosphor_agent::mcp::Editor for McpEditor {
    fn call(
        &self,
        capability: &str,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<String, String> {
        let (reply, answered) = std::sync::mpsc::channel();
        self.asks
            .send((capability.to_owned(), args.clone(), reply))
            .map_err(|_| "the editor's runtime has gone".to_owned())?;
        answered
            .recv()
            .map_err(|_| "the editor's runtime answered nothing".to_owned())?
    }
}

/// Serves MCP on stdin and stdout until the client goes.
///
/// One runtime, built the same way `--eval`'s is ([`vm`]), so an agent calling
/// `phosphor/eval` and a shell running `phosphor --eval` are answered out of
/// the same VM — which is `T023`'s *"identical results"* claim extended to the
/// third door rather than restated for it.
///
/// **Nothing in this function may print.** stdout *is* the protocol here, and a
/// stray line is a parse error at the other end rather than a cosmetic problem.
/// That is also why it returns before [`Term`] exists.
fn serve_mcp() -> Result<ExitCode, Box<dyn Error>> {
    let (asks, orders) = std::sync::mpsc::channel::<Ask>();
    // The VM's own thread. It ends when the last `asks` sender is dropped,
    // which is when the server does, which is when the client goes.
    let holding = std::thread::Builder::new()
        .name("phosphor-mcp-vm".to_owned())
        .spawn(move || {
            let (mut runtime, _host) = vm();
            while let Ok((tool, args, reply)) = orders.recv() {
                let answered = door::mcp_call(&tool, &args, Some(&mut Vm(&mut runtime)));
                drop(reply.send(answered));
            }
        })?;

    // A current-thread runtime: this process does one thing, and `rmcp`'s stdio
    // transport is the only thing in it that awaits.
    let served = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        // **`enable_time` is not optional here**, and the failure was loud:
        // `rmcp`'s service loop arms a timeout per request, and a runtime
        // without timers panics *"a Tokio 1.x context was found, but timers are
        // disabled"* on the first `tools/call` — after the handshake, so the
        // server looks healthy right up until it is asked to do something.
        .enable_time()
        .build()?
        .block_on(phosphor_agent::mcp::Server::new(McpEditor { asks }).serve_stdio());
    // The sender is gone with the server, so this joins rather than blocking.
    drop(holding.join());
    // `Box<dyn Error + Send + Sync>` is not `Box<dyn Error>`, and `?` will not
    // bridge the two — the string is what a caller can do anything with anyway.
    served.map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
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
    // `T052`. Before [`Term`] exists, for `--eval`'s reason and one more: this
    // process's stdout **is** the protocol, so anything that printed to it
    // would be a parse error at the other end.
    if cli.mcp {
        return serve_mcp();
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
    // **`--soft-wrap` seeds the option; it is not a second answer** (`T096`).
    //
    // The loop used to read `cli.soft_wrap || host.flag("soft-wrap")` every
    // frame, which made the flag and the option two pieces of state for one
    // question — and left `set-soft-wrap` unable to turn wrapping *off* in a
    // session started with the flag, since the `||` put it straight back. Seeded
    // **after** the layer loads, so it overrides `init.scm` the way a command
    // line should and the verb can still override it afterwards.
    if cli.soft_wrap {
        host.set_flag("soft-wrap", true);
    }
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
    // **The queue, opened here rather than beside its reader.** `events::open`
    // is an `mpsc::channel()` and starts no thread — the *"after `Term::new()`,
    // never before"* rule below belongs to `read_terminal`, which is what races
    // the protocol negotiation. Opening it early is what lets the picker's
    // matcher be handed a way to say *"I have more"*, which it needs at
    // construction and which nothing could give it when the channel came later.
    let (queue, poster) = events::open();
    // `T088`'s step 4a: the pane the buffer is shown in, beside the buffer
    // rather than inside it. One, until step 4c gives the loop a map of them.
    // Step 4b: what the session owns, once rather than once per buffer.
    let mut shell = Shell {
        store: Arc::clone(&host.store),
        asks: BTreeMap::new(),
        next_ask: Arc::clone(&host.next_ask),
        held: BTreeMap::new(),
        granted: Vec::new(),
        asking_about: BTreeMap::new(),
        writing: Vec::new(),
        allowed: None,
        edits: Vec::new(),
        steering: None,
        pausing: false,
        paused: None,
        deferred: BTreeSet::new(),
        asked: None,
        // The directory the editor was started in — `Timeline::open_at`'s rule,
        // and the honest root until `T071` makes it the repository's.
        workspace: std::env::current_dir().unwrap_or_default(),
        wake: picker::waking(poster.clone()),
        registers: BTreeMap::new(),
        picker: None,
        source_order: Vec::new(),
        mode: EditMode::Normal,
        quit: false,
        discard: false,
        falling_through: false,
        wall: false,
        closing: None,
        splitting: None,
        // `T050`. Started here and attached to nothing: the runtime thread is
        // idle until `agent-command` names something, which is the same
        // *"nothing is spawned until attach"* contract `LanguageServers` has.
        session: phosphor_agent::session::Session::start(
            agent::sink(poster.clone()),
            agent::waking(poster.clone()),
        ),
        turn: None,
        agent: None,
        life: phosphor_agent::session::Life::None,
        transcript: Transcript::default(),
        told: 0,
        folded: Vec::new(),
        saying: None,
        prompt_step: None,
        history: Vec::new(),
        recalled: None,
        hinted: false,
        wanted: None,
    };
    // `T088`'s step 4c: every buffer by id, every pane by id, one of each.
    // The maps are the wrong shape for one entry and that is what they are
    // for — see [`Buffers`] on why position is never the key.
    let (mut buffers, first) = Buffers::new(Editing::with_timeline(
        editor,
        path,
        Rc::clone(&dirty),
        Rc::clone(&edits),
        timeline,
    ));
    let (mut panes, _) = Panes::new(Pane::new(first));

    // `T033`'s ex line, and the one line of chrome that answers it. Both live
    // here rather than in a widget: `view::Node::Prompt` is the vocabulary's
    // shape for this and `phosphor-ui` defers it to `T058`, so what S3 can hold
    // is the primitives — a row of labels where the statusline goes, which is
    // where vim puts it too.
    // `T047`'s landing slot for a `request-references` answer. See
    // [`References`] for why it is a slot and not an Action payload.
    let references: References = Arc::new(Mutex::new(Vec::new()));
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
    // **The seen journal outranks the undo one**, and the order is the same
    // argument the rungs below it settle: *"one row, two true things, and the
    // surprising one has to be the one said out loud."* Losing a buffer's undo
    // history costs this file's `u`; losing the seen journal costs every
    // marker in the workspace, which is the larger surprise.
    let mut notice: Option<String> = host
        .store_note
        .clone()
        .or(restore_note)
        .or_else(|| fresh.as_deref().map(new_file))
        .or_else(|| {
            (buffers.at_mut(first).file.is_none() && matches!(surface, Surface::Buffer))
                .then(no_file)
        });

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
    // `T093`'s slot — one, because §9 allows one focused float.
    let mut open_float: Option<phosphor_core::view::Float> = None;

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
    // `T088`'s step 9: in-flight requests keyed by the buffer that asked.
    let mut outstanding = Asking::default();
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
    // **The app's own epoch** (`view::Millis`: *"the epoch is the app's own …
    // because only differences are ever rendered"*). Nothing read a clock
    // before `T050`, so the interpreter's `now` sat at zero and neither
    // `Node::Spinner` nor `Node::Elapsed` could move — which was honest while
    // there was no session to wait on and is not any more.
    let started = Instant::now();
    // `T054`. Held across passes because it is rebuilt only when the transcript
    // moves — see the publish below.
    let mut transcript_vm: Option<phosphor_ui::transcript::TranscriptVm> = None;
    // Where a session is rooted, and what a jump link resolves against. Read
    // once at boot and held on [`Shell`], because a session outlives a frame and
    // `getcwd` per frame would be asking a question whose answer cannot change
    // inside this process. Cloned here so the loop can hand it to `attach`
    // without borrowing `shell` for the rest of the pass.
    let workspace = shell.workspace.clone();

    // The document the servers have been told about, and how many edits ago.
    // `didChange` is sent from the top of the loop rather than from the edit,
    // because an Action that edits several times — `J`, `>` over a range,
    // accepting a completion — is one change as far as a server is concerned,
    // and telling it three times would have it answering about text the user
    // never saw.
    adopt(buffers.at_mut(first), &host.languages(), &servers);

    loop {
        // **Focus, then the pane, then the buffer it holds** — by id, every
        // pass, rather than by a binding held across the loop. There is one of
        // each today, so this resolves to the same two things every time; what
        // it buys is that the *shape* of the resolution is already the one N
        // panes need, and nothing below this line can reach a buffer except
        // through the pane that shows it.
        let focus = panes.focus;
        // **A focused pane does not have to hold a buffer, as of `T054`.** This
        // was an `expect` whose message read *"the focused pane holds a buffer
        // until step 11 gives it anything else to hold"* — and `T054` is what
        // gave it something else. `SPC t` focused a transcript pane and the
        // editor exited with the promise it had made to itself.
        //
        // The fallback is *another pane's* buffer before any open buffer,
        // because a key pressed while the transcript has focus should act on
        // the file you can still see. `Buffers` is never empty — `Buffers::new`
        // takes one — so the last arm is total rather than hopeful.
        let held = panes.at(focus).buffer.or_else(|| {
            panes
                .tree
                .leaves()
                .into_iter()
                .find_map(|id| panes.at(id).buffer)
        });
        let Some(held) = held.or_else(|| buffers.map.keys().next().copied()) else {
            // Unreachable: `Buffers::new` is the only constructor and it takes
            // an `Editing`. A `break` rather than a panic anyway, because the
            // one thing worse than an editor with no buffer is an editor that
            // aborts a terminal it has put into raw mode.
            break;
        };

        // The size the *next* frame will be laid out at, and the layout itself.
        // **`draw` used to re-split `frame.area()`**, so the wrap width and the
        // rect a scroll is measured against were one answer and the rects that
        // were painted were another — see [`Geometry`] for what diverged. The
        // split happens once, here; the two strips come off it below, where
        // whether they exist this frame is known.
        //
        // `term.size()` rather than `frame.area()` because this half needs
        // `&mut editor` and therefore cannot happen inside the closure. The two
        // agree except across a resize between this line and the draw, which
        // ratatui answers with a `Resize` event and the next pass corrects.
        let size = term.size()?;
        let mut geometry = lay_out(Rect::new(0, 0, size.width, size.height));
        // **§5's top strip, taken before the panes are measured.** Unlike the
        // two bottom strips this one's condition is knowable here — it is
        // *"are there two panes"* and not *"did the VM say a prefix is
        // half-typed"* — so it comes off `body` as well as `pane` and the wrap
        // width below is measured against rows the strip has already taken.
        // See [`Geometry::tabs`] for why that difference is the whole reason
        // there are two calls instead of one.
        geometry.take_tab_bar(panes.tree.leaves().len());
        // `init.scm` sets `soft-wrap` at boot and `(set-option! …)` can change
        // it at the REPL, so it is read per frame rather than once: the option
        // is the editor layer's, and the flag is the override.
        //
        // Bound rather than tested in place because the composition below needs
        // the same answer: `Node::Buffer`'s `soft_wrap` prop is the request,
        // and the per-pane pass below is where the request is honoured. Two
        // reads of `host.flag` could disagree inside one frame — the VM runs
        // between them.
        // **One piece of state, which is `T096`'s other half.** `--soft-wrap`
        // used to be OR'd in here every frame, so the flag and the option were
        // two answers to one question and the verb could not turn wrapping
        // *off* in a session that had been started with the flag. The flag
        // seeds the option once at boot (see `soft_wrap_default`); this reads
        // the option and nothing else.
        let soft_wrap = host.flag("soft-wrap") == Some(true);
        // **`T050` — the session follows the option, and only when it moves.**
        // Read per frame for the reason `soft-wrap` is: the value is the editor
        // layer's and `(set-option! "agent-command" …)` at the REPL has to
        // reach the next keystroke. *Acted on* only when it differs, because
        // honouring `soft-wrap` is free and spawning a process is not.
        // **Compared against the last *option* seen, not against what is
        // attached.** Those were one field until `T057` gave a *verb* the same
        // job: `:cn` sets `Shell::agent`, the option is unset, and a check
        // against `agent` then read "the option changed to nothing" on the very
        // next frame and stopped the session the verb had just started.
        // **`T061` — the allow-list, read per pass.** `runtime/permissions.scm`
        // publishes it through `set-option!`, so a grant made at the REPL, by a
        // `[2]`, or by a `(allow …)` in `persisted.scm` at boot all arrive the
        // same way. Read every pass for `agent-command`'s reason: a value
        // cached at boot would make the rules a fact about the last restart.
        shell.allowed = host.text(ALLOWED);

        // **The rules an always-allow agreed to write.** Performed here because
        // writing one is running scheme — `(allow "git push")` is evaluated so
        // the rule takes effect *now* as well as next session, and `persist!`
        // is what puts it in `persisted.scm`. Both, in that order: a rule that
        // only landed on disk would leave the very next invocation asking again.
        for verb in std::mem::take(&mut shell.writing) {
            let form = format!("(allow {})", scheme_text(&verb));
            if let Some(said) = phosphor_steel::answer::trouble(&layer.evaluate(&form)) {
                notice = Some(said);
                continue;
            }
            // **`AppHost::persist`, not `(persist! …)`.** The Steel binding is
            // `(define (persist! kept) kept)` — an *identity function*, and a
            // marker: what writes is the REPL noticing that head and routing
            // the form. Evaluating it here would have returned the string and
            // written nothing, which is exactly what the first version did and
            // what the test caught by reading an empty file.
            //
            // **Ungated, which is what `7a`'s pressed digit earns.**
            // `AppHost::persist` declines a head the layer *offers* to keep —
            // that gate is on the REPL's auto-route, so you are taught the verb
            // rather than surprised by a file — and `allow` is not on that
            // list. `T061`'s own entry says so.
            let written = phosphor_steel::answer::trouble(&host.persist(&form));
            if let Some(said) = written {
                // **Said out loud rather than swallowed.** A rule that took
                // effect for this session and did not reach disk is precisely
                // the surprise `7a` promises against — you would press it once,
                // see it work, and be asked again tomorrow.
                notice = Some(format!("allowed for this session only — {said}"));
            }
        }

        // **`T062` — the steer, sent and resumed in one place.** The arm holds
        // the correction because it has no session handle; this has both, and
        // resuming *after* the prompt is what makes it steering rather than two
        // unrelated things that happened in a row.
        if let Some(body) = shell.steering.take()
            && let Some((turn, held)) = shell.paused.take()
        {
            shell.pausing = false;
            let steered = shell.transcript.at(turn);
            steered.next = None;
            steered.ended = None;
            steered.calls.push(held);
            shell.transcript.revision += 1;
            shell.session.prompt(body);
            notice = Some("steered — carrying on".to_owned());
        }

        let wanted = host.text(agent::COMMAND);
        if wanted != shell.wanted {
            shell.wanted = wanted.clone();
            shell.agent = wanted.clone();
            match wanted.as_deref().and_then(agent::spec_from) {
                Some(spec) => shell.session.attach(spec, workspace.clone()),
                // The option was cleared. `stop` rather than nothing, so
                // `:set agent-command ""` is a way to end a session and not
                // just a way to stop naming one.
                None => shell.session.stop(),
            }
        }
        // **Every pane, from the tree — and everything that depends on a
        // rectangle, in the same pass.**
        //
        // This block used to do four things against the focused editor and the
        // whole frame. Three of them are about a *rectangle* — where the text
        // starts, how wide it wraps, how tall it scrolls — and one pane's
        // rectangle is not another's, so they belong here rather than beside
        // the document work below.
        //
        // **`layout` takes the outer rect and each pane insets its own.**
        // Step 11a laid out `editor_area(body)`, which insets once for the
        // whole frame and then slices it — so with two panes the second one's
        // text would start two cells left of its own gutter. Each pane reserves
        // its own three columns, so the inset is per pane and the tree divides
        // what is outside them.
        //
        // **Two panes on one buffer means two wrap widths on one `Editor`, and
        // the last one wins.** That is ruling (a) showing up a second time and
        // it is recorded here because here is where it happens: the wrap is the
        // fork's, one per `Editor`, and a per-pane wrap needs the per-pane
        // viewport the ruling puts on `Pane` — whose reader is
        // `Resources::viewport`, which does not exist yet. Until it does, the
        // honest behaviour is that the pane laid out last decides, and the
        // honest thing to do is say so rather than let it look intentional.
        for (id, outer) in panes.tree.layout(geometry.body) {
            panes.at_mut(id).area = editor_area(outer);
            let Some(shown) = panes.at(id).buffer else {
                continue;
            };
            let buffer = buffers.at_mut(shown);
            // The buffer's own answer, or the room's. **And the `else` is not
            // tidiness**: without it, turning wrapping off left every buffer
            // wrapped until it was reopened — the option moved and the rope did
            // not, which is a toggle that only works once.
            if buffer.soft_wrap.unwrap_or(soft_wrap) {
                // Free when the width has not changed, and it moves no
                // viewport.
                soft_wrap::wrap_to(&mut buffer.editor, outer);
            } else {
                soft_wrap::unwrap(&mut buffer.editor);
            }
            // `8e`'s whitespace marks are INSERT-only, and the mode is the
            // machine's — the first thing in this loop that is not hardcoded.
            //
            // **The boundary conversion is gone.** It existed because
            // `soft_wrap::EditMode` was a two-value copy that said of itself
            // *"the real mode enum is `spine`'s and does not exist yet
            // (`T026`)"*; the widget re-exports
            // `phosphor_core::request::EditMode` now, so there is one enum and
            // nothing to convert.
            soft_wrap::set_mode(&mut buffer.editor, machine.mode());
        }
        // `T104` — what one indent level is, and how wide a `\t` draws.
        //
        // **Per buffer, not per pane**, and the difference is what the value is
        // *about*: an indent unit comes from the language declaration and from
        // `set-option!`, neither of which knows anything about a rectangle. Two
        // panes on one file indent the same; two files in one pane do not.
        //
        // Read per pass for the reason `soft-wrap` and the completion floor
        // are: the option is the editor layer's, `(set-option! …)` at the REPL
        // has to reach the next keystroke, and a value cached at boot would
        // make the setting a fact about the last restart. `set_tab_width` is
        // free when the number has not moved and rebuilds the row stream when
        // it has, because a wider tab moves every wrap point.
        for buffer in buffers.map.values_mut() {
            buffer.indent_style = indent_style(&host, &host.languages(), buffer.language.as_ref());
            buffer.editor.set_tab_width(buffer.indent_style.tab_width);
        }

        // `T038`'s document sync. Once per turn and only when the edit stream
        // moved: `T036` sent `didOpen` and nothing after it, so every request
        // against a file the user had typed into asked about the text as it was
        // when the buffer opened — *"completions for a prefix that is no longer
        // there is not a stale-looking list; it is a wrong one, and nothing on
        // screen says so"* (`LanguageServers::change`).
        //
        // **Over every buffer, not the focused one.** This block read one
        // `edits` against one `sent` and asked `editing` — whatever was on
        // screen — for its contents. With a second buffer open that is not a
        // partial answer but a wrong one: a server holding file B is never told
        // B changed, so every completion, hover and diagnostic it produces for
        // B is computed against the text as it was when B was last looked at.
        // A file you edited, switched away from, and came back to would answer
        // about a version of itself that no longer exists, and nothing on
        // screen would say so.
        //
        // Each buffer's `edits` and `sent` are its own for the same reason: one
        // pair cannot express *"A changed, B did not"*, and comparing A's
        // counter against B's last-sent is a comparison between two files.
        for buffer in buffers.map.values_mut() {
            if buffer.edits.get() == buffer.sent {
                continue;
            }
            buffer.sent = buffer.edits.get();
            if let (Some(language), Some(document)) =
                (buffer.language.clone(), buffer.synced.as_ref())
            {
                servers.change(&language, document.path.clone(), buffer.contents());
            }
        }

        // **Every buffer, not the one on screen.** `decorate` was this block,
        // inline, against whichever buffer was focused — and
        // `Resources::state_marks` takes a `BufferId` and answered the same
        // column whatever it was handed. A second pane showing a second file
        // would have drawn the focused file's error markers beside its text.
        //
        // The state column is the widget's and is wanted for every buffer; the
        // two counts are the statusline's and are wanted for one, so the
        // focused buffer's are the ones kept.
        let mut columns: BTreeMap<BufferId, Vec<StateMark>> = BTreeMap::new();
        // **`T089` — the count per buffer, which this loop already had.** It
        // computed `decorated.unseen` for every buffer and kept one, because
        // the statusline asks about the file on screen. A tab bar asks about
        // every pane, so the answers are kept instead of dropped; nothing here
        // computes anything it did not compute before, which is what makes
        // *"per-tab unseen counts track the store"* a matter of not throwing
        // the answer away.
        let mut unseen_per_buffer: BTreeMap<BufferId, u32> = BTreeMap::new();
        let mut tally = Tally::default();
        let mut unseen = 0;
        for (id, buffer) in &mut buffers.map {
            let decorated = decorate(buffer, &shell.store, &host, &theme);
            if *id == held {
                tally = decorated.tally;
                unseen = decorated.unseen;
            }
            unseen_per_buffer.insert(*id, u32::try_from(decorated.unseen).unwrap_or(u32::MAX));
            columns.insert(*id, decorated.marks);
        }
        let editing = buffers.at_mut(held);

        // **`T051` — the session, published for the `session` query**, and
        // the transition said out loud.
        //
        // §5 asks for *"always present and truthful"*, and the two halves land
        // in two places: the statusline carries the *state*, and the notice row
        // carries the *change*. A session that dropped while you were reading
        // would otherwise announce itself only by a word quietly changing on a
        // strip you were not looking at.
        let life = shell.session.life();
        if life != shell.life {
            if let Some(said) = session_notice(&life) {
                notice = Some(said);
            }
            // **`7b`'s seam is written by the drop, not by a verb.** `:seam` is
            // the manual form and it exists for a session that paused or
            // resumed — things nothing observes. A connection that goes while a
            // turn is running is observed *here*, and a transcript that showed
            // the seam only if you thought to ask for it would be a transcript
            // whose honesty was your job.
            if let (SessionLife::Lost(_), Some((running, _))) = (&life, shell.turn) {
                let unseen = shell
                    .store
                    .unseen_count(&phosphor_core::store::Scope::Everywhere);
                shell.transcript.at(running).ended = Some(phosphor_ui::transcript::Seam {
                    text: "connection lost mid-turn".to_owned(),
                    detail: Some(survived(unseen)),
                    tone: phosphor_ui::transcript::SeamTone::Trouble,
                });
                // The turn is over, whatever the agent thought. Leaving it open
                // would leave the statusline saying `working` about a session
                // that is gone — §5's *"always truthful"* failing in the one
                // moment it is being read.
                shell.turn = None;
            }
            shell.life = life.clone();
        }
        host.publish_session(session_value(
            &life,
            shell.turn.as_ref(),
            !shell.asks.is_empty(),
            shell.paused.is_some(),
        ));
        // **`T060` — the queue, published every pass beside the session.**
        // Cheap for `session`'s reason and unlike `transcript`: a queue is
        // bounded by how many questions are outstanding, and a transcript grows
        // for as long as the editor is open.
        // **`T099` — cheap for `session`'s reason.** A register table is a few
        // dozen short strings; a transcript grows without bound, which is why
        // that one is guarded by a revision and this one is not.
        host.publish_registers(
            shell
                .registers
                .iter()
                .map(|(name, held)| (name.clone(), held.text.clone()))
                .collect(),
            machine.recording().unwrap_or_default(),
        );
        host.publish_asks(
            shell
                .asks
                .iter()
                .map(|(id, question)| ask_value(*id, question, shell.deferred.contains(id)))
                .collect(),
        );

        // **`T054` — rebuilt and published only when the transcript moves.**
        // Its two neighbours above are a handful of fields and cost nothing per
        // frame; this one grows for as long as the editor is open, so a clone
        // per pass would make an idle editor's cost a function of how much
        // claude has said to it. One `u64` compare is the whole guard.
        if shell.transcript.revision != shell.told {
            shell.told = shell.transcript.revision;
            host.publish_transcript(Value::List(
                shell
                    .transcript
                    .turns
                    .iter()
                    .map(Transcript::describe)
                    .collect(),
            ));
            transcript_vm = Some(shell.transcript.vm(&life, shell.paused.is_some()));
        }
        // The turn in flight carries the mark its spinner counts from, and it
        // moves without the transcript moving — a turn that has said nothing
        // for ten seconds is still a turn that has been running for ten.
        if let (Some(vm), Some((running, began))) = (transcript_vm.as_mut(), shell.turn)
            && let Some(turn) = vm.turns.iter_mut().find(|turn| turn.id == running)
        {
            turn.since = Some(Millis(
                u64::try_from(began.duration_since(started).as_millis()).unwrap_or(u64::MAX),
            ));
        }

        // **The screen's shape, published for the `panes` query** (`T088`).
        // Once per frame, on `picker-rows`' terms: the panes are the loop's
        // and a query answering on another thread cannot borrow them, so the
        // answer is what the last frame laid out. See [`HostState::panes`].
        host.publish_panes(panes.describe());

        // **The one place the frame cache learns that arbitrary scheme ran.**
        // Not per call site, not by remembering: `Layer` is the only way into
        // the VM and every method on it that can run user scheme sets the flag
        // this reads. `CP-2` found the keybinding half of the old rule missing
        // by running it; this is what makes that unfindable-by-running rather
        // than merely fixed.
        if layer.stale() {
            status_cache.invalidate();
        }

        // What the interpreter draws this frame. **One tree, always** — the
        // buffer is a `Node::Pane` holding a `Node::Buffer` now rather than a
        // widget `draw` rendered outside the tree, so there is no frame
        // without a composition and nothing left for a second draw path to do.
        //
        // `T079`'s interpreter draws it; nothing here knows a colour.
        let screen = if matches!(surface, Surface::Repl) {
            // `6b` composes its own statusline, so it owns the whole frame —
            // and it is the only surface that does.
            Composed::Frame(repl.frame())
        } else {
            // Over the buffer rather than instead of it, which is what §9's
            // dim means: these five hang off the pane the host composed, and
            // each used to hang off an empty root standing in for the widgets
            // that painted underneath.
            let float = match (&surface, &boot) {
                (Surface::Boot, Some(boot)) => Some(boot.clone()),
                // `6d`, the same way: the buffer and the statusline stay
                // behind it and §9 dims the first of the two.
                (Surface::Help, _) => help_page.clone(),
                // `T093`. Same shape as the two above and deliberately so: a
                // surface the editor layer composed is drawn by the
                // interpreter like any other tree, which is what lets `T048`'s
                // `:arch` add zero lines to `phosphor-ui`.
                (Surface::Float, _) => open_float.clone(),
                // `T045`: the picker is a float over the buffer, so §9 dims
                // the code behind it and `2a`'s screen is what you get.
                (Surface::Picker, _) => shell.picker.as_ref().map(picker_float),
                // `T038`, `T039` — the completion list and signature help, as
                // floats over the buffer you are still typing into. Same shape
                // as the three above and for a different reason:
                // `Mood::Passive` *"is not in front of anything"* (§9), so the
                // code stays at full strength behind this one.
                (Surface::Buffer, _) => passive_float(editing),
                _ => None,
            };
            let tree = Tree::new(compose_panes(
                &panes.tree,
                &panes,
                focus,
                soft_wrap,
                &shell.folded,
            ));
            Composed::Pane(match float {
                Some(float) => tree.with_float(float),
                None => tree,
            })
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
                dirty: editing.dirty.get(),
            }),
            // **`T050` fills the first of the two in**, which is what the
            // sentence this replaced promised: *"`T050` and `T071` fill those
            // two in; a fixture here would be a lie on a real terminal."* It is
            // the client's report and never the editor's guess — see
            // [`session_state`].
            session: session_state(
                &shell.session.life(),
                shell.turn.as_ref(),
                !shell.asks.is_empty(),
                shell.paused.is_some(),
            ),
            // Where the elapsed counter counts from, in the app's own epoch.
            // A turn's `Instant` is converted here rather than stored as
            // `Millis`, because the arm that records it cannot see `started`
            // and a second epoch is how two clocks disagree.
            since: shell.turn.map(|(_, began)| {
                Millis(u64::try_from(began.duration_since(started).as_millis()).unwrap_or(u64::MAX))
            }),
            // **`T060` — Q9's flag, fed at last.** The widget carried this
            // field with Q9's own sentence in its doc since `T017`, and the
            // binary handed it `false`: *"it sets the statusline `!` flag
            // immediately and waits"* was implemented on the drawing side and
            // on nothing else. It is the queue, not the float — a question
            // waiting behind a picker is exactly the case the flag exists for,
            // and that is the frame where no float is up.
            //
            // **Deferred asks count.** `esc later` is a fact about the screen
            // and this is a fact about the queue: pushing a question back does
            // not answer it, and a `!` that disappeared when you deferred one
            // would be the editor forgetting on your behalf.
            ask_pending: !shell.asks.is_empty(),
            // **`T041` — §5's `●n`, counted rather than zero.** Over the file
            // on screen, which is what `runtime/statusline.scm`'s own VM doc
            // says it is: *"unseen regions in this file"*. The workspace-wide
            // count is `(unseen-count)` with no `within`, and it is a different
            // question — `2a`'s picker asks it, the statusline does not.
            unseen: u32::try_from(unseen).unwrap_or(u32::MAX),
            // **The count `2b` draws and nothing computed until now.** It is
            // over the whole file, never over what the rows drew: bounding the
            // inline rows (`RowPolicy`) is only honest if the ones that stay
            // quiet are still counted somewhere, and this is that somewhere.
            trouble: tally.trouble,
            attention: tally.attention,
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

        // `T053`. A parked sentence, on the first frame with a notice row to
        // put it on. `Surface::Repl` owns its whole frame — `draw` returns
        // early for `Composed::Frame`, statusline included — so a sentence set
        // while it is open would be drawn to nobody; this waits instead of
        // being spent. Immediately before the chrome is built, which is the
        // last point `notice` is still free.
        if !matches!(surface, Surface::Repl)
            && notice.is_none()
            && let Some(said) = shell.saying.take()
        {
            notice = Some(said);
        }

        // `T058`'s four surface verbs, performed where `ex_line` lives.
        match shell.prompt_step.take() {
            Some(PromptStep::Set(text)) => {
                ex_line = text;
                shell.recalled = None;
            }
            Some(PromptStep::Cancel) => {
                surface = Surface::Buffer;
                ex_line.clear();
                shell.recalled = None;
            }
            Some(PromptStep::Submit) => {
                surface = Surface::Buffer;
                if !ex_line.trim().is_empty() {
                    shell.history.push(ex_line.clone());
                }
                shell.recalled = None;
                notice = submit_ex(
                    &mut layer,
                    editing,
                    &mut Cx::new(held, focus, &mut panes, &mut shell),
                    &ex_line,
                );
            }
            Some(PromptStep::History(delta)) => {
                // **Positive walks back**, which is the row's own wording and
                // vim's `<up>`. Clamped at both ends rather than wrapping: a
                // history that wrapped would hand you the newest line when you
                // asked for one older than the oldest, which is the opposite of
                // what you asked for.
                let depth = shell.history.len();
                if depth > 0 {
                    let at = shell.recalled.map_or(0, |at| at);
                    let moved = i64::try_from(at).unwrap_or(i64::MAX) + delta;
                    let clamped = moved.clamp(0, i64::try_from(depth).unwrap_or(i64::MAX));
                    let at = usize::try_from(clamped).unwrap_or(0);
                    if at == 0 {
                        shell.recalled = None;
                        ex_line.clear();
                    } else {
                        shell.recalled = Some(at);
                        ex_line = shell.history[depth - at].clone();
                    }
                }
            }
            None => {}
        }

        // `T058`. **Cloned out of the buffer, not borrowed from it.** The
        // chrome below outlives `editing`'s `&mut`, and a `FileSpan` is a path
        // and two positions — cheaper to copy once a frame than to restructure
        // the borrow around a row that is usually not there.
        let anchored = matches!(surface, Surface::Ex)
            .then(|| editing.anchor.clone())
            .flatten();

        // What is on the statusline's row instead of the statusline. The ex
        // line takes it while it is open — vim's own placement — and a notice
        // borrows it until the next key.
        let typed = format!(":{ex_line}");
        let chrome = if matches!(surface, Surface::Ex) {
            Some(Chrome {
                text: &typed,
                caret: true,
                anchor: anchored.as_ref(),
            })
        } else {
            notice.as_deref().map(|text| Chrome {
                text,
                caret: false,
                anchor: None,
            })
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

        // The rest of the layout, now that both strips' conditions are known.
        // The rects a scroll and the wrap width were measured against above are
        // deliberately the ones from before this call — see [`Geometry`].
        geometry.take_strips(&leader, hint.is_some(), &theme);
        // `T058`. After the two strips, so `1c`'s row sits directly above the
        // statusline whatever else is up.
        geometry.take_prompt(anchored.is_some());

        // `T045`. **Ticked here, once, before the draw** — the matcher needs
        // `&mut` and `Resources` has no `&mut` in it and must never grow one.
        // The deadline is a millisecond, so a 100k-file filter costs the frame
        // that much and finishes on its own threads; what the frame gets is
        // whatever had matched by then, marked `matching` if there is more.
        // The list gets the body minus the filter line, which is the height
        // the widget will actually draw into — asking for more would make the
        // matcher materialise rows nothing can show.
        let list_rows = usize::from(panes.at(focus).area.height.saturating_sub(1));
        let picker_vm = shell
            .picker
            .as_mut()
            .map(|session| session.matcher.tick(list_rows));

        // **Every buffer's editor, lent to the interpreter for one frame.**
        // Borrowed rather than cloned — an `Editor` holds a rope, a tree-sitter
        // tree and a highlight cache — and immutably, which is what makes it
        // safe to hand across the `Resources` door: that trait has no `&mut` in
        // it and must never grow one.
        let editors: BTreeMap<BufferId, &Editor> = buffers
            .map
            .iter()
            .map(|(id, buffer)| (*id, &buffer.editor))
            .collect();
        let editing = buffers.at(held);
        // `T089`. Composed every frame and empty on most of them — a session
        // with one pane composes `Node::Empty` here and `Geometry` gave the
        // strip no row to be drawn into, so the two halves of §5's *"only with
        // 2+ panes"* agree by both asking the same tree the same question.
        let tab_bar = Tree::new(compose_tabs(&panes, &buffers, &unseen_per_buffer));
        let overlay = Overlay {
            asks: &shell.asks,
            chrome,
            status: status_tree,
            leader: &leader,
            hint: hint.as_ref(),
            columns: &columns,
            focused: (held, panes.at(focus).area),
            completion: editing.completion.as_ref(),
            signature: editing.signature.as_ref(),
            picker: picker_vm.as_ref(),
            transcript: transcript_vm.as_ref(),
            tabs: &tab_bar,
            now: Millis(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        };
        term.draw(|frame| {
            draw(
                frame, &editors, &theme, &geometry, &floats, &screen, &overlay,
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
        let deadline = if outstanding.anyone_awaiting(Lookup::Completion) {
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
                // **The focused buffer, resolved for this arm rather than held
                // across the loop.** A keystroke and a click are the two
                // producers that cannot name a buffer — the machine has no
                // ids and a click resolves through a pane — so *this* arm is
                // where "the focused one" is a fact rather than an assumption.
                // The `Posted` arm below resolves its own, because a posted
                // Action can name a buffer that is not on screen.
                let editing = buffers.at_mut(held);
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
                                editing.retrack();
                                surface = Surface::Buffer;
                            }
                        }
                    }
                    // **`T059` — `4a`'s digits, and the whole of *"only while
                    // it is focused"*.**
                    //
                    // Two conditions, both the loop's: a question float is what
                    // holds the screen, and it is showing an ask. Over a buffer
                    // the same key is vim's count prefix and stays that — which
                    // is why this is an arm here rather than a binding in
                    // `keymaps.scm`, where a digit has no way to ask what is on
                    // screen.
                    //
                    // **A digit no option carries is not swallowed.** `4a`
                    // offers `[1]`–`[3]`; pressing `7` at it declines by name
                    // rather than doing nothing, because a float that ate the
                    // key would be indistinguishable from one that had not
                    // noticed.
                    Event::Key(key)
                        if matches!(surface, Surface::Float)
                            && shell.asked.is_some()
                            && digit_pressed(key).is_some() =>
                    {
                        // **Through the capability, not around it.** The key
                        // decides *that* a digit was pressed; `float-answer`
                        // decides what a digit means, which ask is focused and
                        // whether the option exists. A second copy of that
                        // reasoning here is the *"no second path from a command
                        // to the buffer"* rule `T033` set, one surface over.
                        let digit = digit_pressed(key).unwrap_or(0);
                        let outcome = editing.apply(
                            &mut Cx::new(held, focus, &mut panes, &mut shell),
                            &Action::Float(FloatAction::FloatAnswer { digit }),
                        );
                        if let Outcome::Refused(why) = outcome {
                            notice = Some(phosphor_steel::answer::why(&why));
                        }
                    }
                    // `T045`. The picker owns every key while it is open, the
                    // same way the ex line does and for the same reason: it is
                    // a line editor with a list under it, not a mode of the
                    // machine. `esc` is handled by `closes_surface` below.
                    Event::Key(key) if matches!(surface, Surface::Picker) => {
                        if let Some(session) = shell.picker.as_mut() {
                            let step = picker_key(key, session);
                            match step {
                                PickerStep::Typing => {}
                                PickerStep::Cycle(delta) => {
                                    let outcome = editing.apply(
                                        &mut Cx::new(held, focus, &mut panes, &mut shell),
                                        &Action::Picker(PickerAction::CyclePickerSource { delta }),
                                    );
                                    if let Outcome::Refused(why) = outcome {
                                        notice = Some(phosphor_steel::answer::why(&why));
                                    }
                                }
                                PickerStep::Accept | PickerStep::Split(_) => {
                                    // **The direction is the key's, not the
                                    // Action's.** `AcceptHow::Split` says that
                                    // it splits and carries no direction, so
                                    // the two picker keys are the two ways —
                                    // widening the vocabulary to carry one
                                    // would answer a question nobody asked.
                                    let (how, toward) = match step {
                                        PickerStep::Split(toward) => (AcceptHow::Split, toward),
                                        _ => (AcceptHow::Open, Direction::Right),
                                    };
                                    let outcome = editing.accept_picker(
                                        &mut Cx::new(held, focus, &mut panes, &mut shell),
                                        how,
                                        toward,
                                    );
                                    match outcome {
                                        Outcome::Refused(why) => {
                                            notice = Some(phosphor_steel::answer::why(&why));
                                        }
                                        _ => {
                                            host.publish_picker(None);
                                            surface = Surface::Buffer;
                                        }
                                    }
                                }
                                PickerStep::Close => {
                                    shell.picker = None;
                                    // Or `picker-rows` answers a list nothing
                                    // is drawing, which is the staleness a
                                    // published snapshot trades for and the
                                    // one place it has to be paid.
                                    host.publish_picker(None);
                                    surface = Surface::Buffer;
                                }
                            }
                        } else {
                            surface = Surface::Buffer;
                        }
                    }
                    // §9: esc closes top-down, and a float that is not a surface of its
                    // own is closed here before the machine ever sees the key. There is
                    // only ever one level (`Surface`).
                    //
                    // **`T060` — closing `4a` is `defer-ask`, not just closing.**
                    // The mockup's footer says `esc later`, and *later* is a
                    // fact about the queue rather than about the screen: the
                    // question is still pending, still counts toward the
                    // statusline's `!`, and `]!` brings it back. Without the
                    // deferral this key does not converge — the float closes,
                    // the next pass finds the same head still pending, and
                    // raises it again.
                    // **`T062` — `esc` mid-turn pauses, and only when nothing
                    // is over the buffer.** §9's `esc` closes top-down, so a
                    // float, a picker or the ex line gets it first and this
                    // never sees the key; what is left is `esc` in a buffer
                    // while a turn is running, which is `7e`'s own gesture. An
                    // `esc` with no turn goes on meaning what it always did.
                    Event::Key(key)
                        if matches!(key.code, KeyCode::Esc)
                            && matches!(surface, Surface::Buffer)
                            && shell.turn.is_some()
                            && shell.paused.is_none() =>
                    {
                        let outcome = editing.apply(
                            &mut Cx::new(held, focus, &mut panes, &mut shell),
                            &Action::Session(SessionAction::InterruptSession {}),
                        );
                        if let Outcome::Refused(why) = outcome {
                            notice = Some(phosphor_steel::answer::why(&why));
                        }
                    }
                    Event::Key(key) if closes_surface(key, surface) => {
                        if let Some(asked) = shell.asked {
                            let outcome = editing.apply(
                                &mut Cx::new(held, focus, &mut panes, &mut shell),
                                &Action::Ask(AskAction::DeferAsk { ask: Some(asked) }),
                            );
                            if let Outcome::Refused(why) = outcome {
                                notice = Some(phosphor_steel::answer::why(&why));
                            }
                        }
                        surface = Surface::Buffer;
                    }
                    // The ex line owns every key while it is open, which is what makes
                    // it a line editor rather than a mode of the machine.
                    Event::Key(key) if matches!(surface, Surface::Ex) => {
                        match ex_key(key, &mut ex_line) {
                            ExStep::Typing => shell.recalled = None,
                            ExStep::Cancel => surface = Surface::Buffer,
                            ExStep::Submit => {
                                surface = Surface::Buffer;
                                // `T058` — ex-style history. Recorded before it
                                // runs, so a line that fails is still one you
                                // can walk back to and fix, which is the whole
                                // reason vim keeps one.
                                if !ex_line.trim().is_empty() {
                                    shell.history.push(ex_line.clone());
                                }
                                shell.recalled = None;
                                notice = submit_ex(
                                    &mut layer,
                                    editing,
                                    &mut Cx::new(held, focus, &mut panes, &mut shell),
                                    &ex_line,
                                );
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
                                editing,
                                cx: Cx::new(held, focus, &mut panes, &mut shell),
                            }
                            .key(pressed);
                            typing = machine.mode() == EditMode::Insert && edits.get() != before;
                        }
                    }
                    Event::Mouse(mouse) => {
                        for action in mouse_actions(
                            &mut machine,
                            editing,
                            &Cx::new(held, focus, &mut panes, &mut shell),
                            mouse,
                        ) {
                            // `Input::SetMode` is the machine reporting a
                            // transition it has already made — `Machine::click`
                            // and `Machine::drag` mutate it directly, the way
                            // `feed` does — so there is nothing here to apply
                            // and `Editing` has no arm for one.
                            if !matches!(action, Action::Input(_)) {
                                let _ = editing.apply(
                                    &mut Cx::new(held, focus, &mut panes, &mut shell),
                                    &action,
                                );
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
                // **Which buffer the Action names, before deciding what to do
                // with it.** A posted Action is the one kind that can name a
                // buffer that is not the focused one: `set-cursor` over MCP
                // carries a `BufferId`, and the three `ingest-*` answers carry
                // the one that asked. The applier used to drop all four with
                // `..` and act on whatever was in front of the user.
                let named = Buffers::named(&posted.action, held);
                let Some(target) = buffers.get_mut(named) else {
                    // A stale id from an agent working off an old query, which
                    // is what `NoSuchTarget` is for. Said out loud rather than
                    // dropped: the producer asked about a buffer, and silence
                    // is indistinguishable from having done it.
                    notice = Some(phosphor_steel::answer::why(&Refusal::NoSuchTarget).to_owned());
                    continue;
                };
                if outstanding.at(named).answers(&posted.action) {
                    // The user's own request coming back. Applied through
                    // `act` and not `apply`, for the reason `deliver` gives:
                    // a reveal is `View::Scroll`, and nothing that is not the
                    // user may move the viewport the user is looking at.
                    drop(target.act(
                        &mut Cx::new(named, focus, &mut panes, &mut shell),
                        &posted.action,
                    ));
                } else if let Some(note) = deliver(
                    target,
                    &mut Cx::new(named, focus, &mut panes, &mut shell),
                    &posted,
                ) {
                    notice = Some(note);
                }
            }
        }

        // **`:wall` — every dirty buffer, not the one on screen.** The arm
        // recorded the ask because it holds one buffer; this holds them all.
        // `Editing::write` is still the only thing that writes.
        //
        // A buffer with no file name refuses, and that refusal is the whole
        // point of writing them one at a time rather than stopping at the
        // first: `:wall` on four buffers where the second is unnamed should
        // still write the other three and say what it could not do.
        if std::mem::take(&mut shell.wall) {
            let mut trouble = Vec::new();
            for buffer in buffers.map.values_mut() {
                if !buffer.dirty.get() {
                    continue;
                }
                if let Err(reason) = buffer.write(None) {
                    trouble.push(reason);
                }
            }
            if !trouble.is_empty() {
                notice = Some(trouble.join("; "));
            }
        }

        // **A picker row into a new split** (`T088`, step 12). Three things
        // an arm cannot do at once, which is why it asked instead: split the
        // tree, open a *new* buffer, and point the new pane at it.
        //
        // The order matters. The pane is split first and shows the same buffer
        // the picker was opened over, so a failure to read the file leaves a
        // split showing something rather than a pane pointing at nothing — and
        // the failure closes it again, because a split you did not ask for is
        // worse than the refusal you did.
        if let Some((file, toward)) = shell.splitting.take() {
            match opening(&file) {
                Ok(found) => {
                    let fresh = found.is_none();
                    let text = found.unwrap_or_default();
                    let rope = buffer(grammar_of(&host.languages(), &file), &text, &theme)?;
                    let (timeline, note) = Timeline::opened(&file);
                    let mut opened = Editing::with_timeline(
                        rope,
                        Some(file.clone()),
                        Rc::new(Cell::new(false)),
                        Rc::new(Cell::new(0)),
                        timeline,
                    );
                    adopt(&mut opened, &host.languages(), &servers);
                    let id = buffers.open(opened);
                    match panes.split(focus, Pane::new(id), toward) {
                        Some(fresh_pane) => {
                            // **Focus follows the split**, which is what a
                            // picker means: you asked for that file, so you are
                            // looking at it. vim's `:split` does the same.
                            panes.focus = fresh_pane;
                            notice = note.or_else(|| fresh.then(|| new_file(&file)));
                        }
                        None => {
                            buffers.map.remove(&id);
                            notice = Some(phosphor_steel::answer::why(&Refusal::NoSuchTarget));
                        }
                    }
                }
                Err(error) => notice = Some(format!("{}: {error}", file.display())),
            }
            // The loop's own bindings are stale the moment either map changed.
            continue;
        }

        // **`:close-buffer` — and whether there is anywhere to go is the
        // question only this can answer.** The arm already refused a dirty
        // buffer without a `force`; what is left is the pane.
        if let Some(closing) = shell.closing.take() {
            let successor = buffers
                .map
                .keys()
                .copied()
                .find(|id| *id != closing)
                .or_else(|| {
                    // Nothing else is open. `:quit` is the verb for leaving,
                    // and it is a different question — this one asked to close
                    // *a buffer*, and closing the only one would leave a pane
                    // with nothing in it.
                    notice = Some("the only buffer — :quit leaves".to_owned());
                    None
                });
            if let Some(successor) = successor {
                buffers.map.remove(&closing);
                for pane in panes.map.values_mut() {
                    if pane.buffer == Some(closing) {
                        pane.buffer = Some(successor);
                    }
                }
                // The loop's own two bindings are stale the moment the map
                // changed, so this pass ends here and the next resolves fresh.
                continue;
            }
        }

        // What the Actions asked for that only the loop can do: `open-file`
        // needs the theme and the language table, and `open-prompt` needs the
        // surface. Both are recorded by `Editing::act` and performed here, for
        // the same reason `Intent` exists — the thing that decides is not the
        // thing that owns.
        //
        // **Resolved again, and that is the point.** The draw above and the
        // event between them each took their own borrow, so no one binding is
        // alive across the whole pass — which is what lets the `Posted` arm
        // reach a buffer that is not the focused one. Held as one binding, it
        // could not, and the four `buffer` selectors went on being discarded.
        let editing = buffers.at_mut(held);
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
            if !same && editing.dirty.get() {
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
                    drop(editing.apply(
                        &mut Cx::new(held, focus, &mut panes, &mut shell),
                        &Action::Motion(MotionAction::SetCursor {
                            position: at,
                            buffer: None,
                        }),
                    ));
                }
            } else {
                match opening(&file) {
                    Ok(found) => {
                        // Empty when the path is free. `buffer` takes the same
                        // grammar either way — a declaration claims the
                        // extension, and the extension is in the name.
                        let fresh = found.is_none();
                        let text = found.unwrap_or_default();
                        let rope = buffer(grammar_of(&host.languages(), &file), &text, &theme)?;
                        let (timeline, note) = Timeline::opened(&file);
                        // A journal that could not be opened outranks *"new
                        // file"*: both are true, one row holds one of them, and
                        // the surprising one is the one that has to be said.
                        // The other is visible in the buffer, which is empty.
                        notice = note.or_else(|| fresh.then(|| new_file(&file)));
                        // **The swap is one call, and the list of what it
                        // resets lives at [`Editing::opens`].** It was spelled
                        // out here across a dozen lines, which is how two
                        // fields came to be missing from it: a reader checking
                        // the block against the struct has to hold both in
                        // their head, and a reader checking a named list only
                        // has to read it.
                        //
                        // The file leaving the pane becomes the alternate, so
                        // `CTRL-^` goes back to it. Set from this branch's
                        // answer rather than in the capability's arm, because
                        // this is the branch that knows a *different* file
                        // arrived: the `same` case above never swaps, and an
                        // alternate set there would make `CTRL-^` a no-op
                        // pointing at the file you are already in.
                        let leaving = editing.opens(rope, file, timeline);
                        panes.at_mut(focus).alternate = leaving;
                        surface = Surface::Buffer;
                        // The server hears about the swap in both directions:
                        // `didClose` for what it was holding — after which it falls
                        // back to what is on disk, which is the specification's own
                        // rule — and `didOpen` for what took its place.
                        if let (Some(language), Some(document)) =
                            (editing.language.clone(), editing.synced.take())
                        {
                            servers.close(&language, &document.path);
                        }
                        adopt(editing, &host.languages(), &servers);
                        editing.sent = editing.edits.get();
                        // `gd` landing. Applied as the Action it is, so the
                        // cursor moves through the one path every cursor move
                        // goes through and the viewport follows it — `apply`
                        // rather than `act`, because this *is* the user's jump.
                        if let Some(at) = editing.open_at.take() {
                            drop(editing.apply(
                                &mut Cx::new(held, focus, &mut panes, &mut shell),
                                &Action::Motion(MotionAction::SetCursor {
                                    position: at,
                                    buffer: None,
                                }),
                            ));
                        }
                    }
                    Err(error) => {
                        editing.open_at = None;
                        notice = Some(format!("{}: {error}", file.display()));
                    }
                }
            }
        }
        // **`T060` — what you said yes to, run.** `Shell::granted` is filled by
        // `Shell::answer_ask`, which is called from an arm and cannot reach
        // `Buffers`; this is the loop, which can. One pass per grant, through
        // the ordinary applier, so a granted action takes exactly the path it
        // would have taken had it never needed asking about.
        for action in std::mem::take(&mut shell.granted) {
            let outcome = buffers
                .at_mut(held)
                .act(&mut Cx::new(held, focus, &mut panes, &mut shell), &action);
            if let Outcome::Refused(why) = outcome {
                notice = Some(phosphor_steel::answer::why(&why));
            }
        }

        // **`T060` — `apply-workspace-edit`, performed across files.**
        //
        // §47 asked this task for four rules about a buffer no pane is looking
        // at, and this is where three of them are answered. **What attaches
        // one:** this, and nothing else — a file a rename touches becomes an
        // entry whether or not you were reading it, because the alternative is
        // an edit that silently skipped the files you had not opened. **What an
        // unattached buffer's wrap width is:** it has none, and needs no
        // invention — `soft_wrap::wrap_to` runs over `panes.tree.layout`, so an
        // entry no pane points at is simply never wrapped, exactly as §47
        // predicted. **What `:wall` counts:** every dirty buffer, these
        // included; a rename whose files were not written by `:wall` is the
        // surprise, not the safety. `:q`'s answer is `Buffers`' own and
        // unchanged — an unattached dirty buffer is unsaved work, and where it
        // is on screen is not what makes it work.
        //
        // **Nothing is written to disk here.** The edits land in buffers and the
        // buffers are dirty, which is `[+]` and `:wall` — the same two steps a
        // rename you typed yourself would take.
        for file in std::mem::take(&mut shell.edits) {
            let absolute = lsp::absolute(&file.path);
            let existing = buffers.map.iter().find_map(|(id, open)| {
                open.file
                    .as_deref()
                    .is_some_and(|held| lsp::absolute(held) == absolute)
                    .then_some(*id)
            });
            let target = match existing {
                Some(id) => id,
                // **Opened, not skipped.** Built the same way `split-pane`
                // above builds one, so a buffer a rename created is an ordinary
                // buffer in every respect except that no pane is pointing at
                // it — which is the container `T088` shipped and §47 said this
                // task inherits.
                None => match opening(&file.path) {
                    Ok(found) => {
                        let text = found.unwrap_or_default();
                        let rope =
                            buffer(grammar_of(&host.languages(), &file.path), &text, &theme)?;
                        let (timeline, _) = Timeline::opened(&file.path);
                        let mut fresh = Editing::with_timeline(
                            rope,
                            Some(file.path.clone()),
                            Rc::new(Cell::new(false)),
                            Rc::new(Cell::new(0)),
                            timeline,
                        );
                        adopt(&mut fresh, &host.languages(), &servers);
                        buffers.open(fresh)
                    }
                    Err(error) => {
                        notice = Some(format!("{}: {error}", file.path.display()));
                        continue;
                    }
                },
            };
            let outcome = buffers.at_mut(target).act(
                &mut Cx::new(target, focus, &mut panes, &mut shell),
                &Action::Buffer(BufferAction::ApplyEdits {
                    edits: file.edits.clone(),
                }),
            );
            if let Outcome::Refused(why) = outcome {
                notice = Some(format!(
                    "{}: {}",
                    file.path.display(),
                    phosphor_steel::answer::why(&why)
                ));
            }
        }
        let editing = buffers.at_mut(held);

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

        // `T045`. **The rows come from nowhere yet, and that is the task
        // boundary rather than an oversight**: `T046` is *"Steel picker sources
        // — unseen, files"*, and `define-picker-source` is its capability. What
        // this task owes is a picker that opens, filters, selects and closes,
        // and it does — over an empty source, which draws `0/0` and says so
        // honestly rather than pretending to a list.
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
            && !outstanding.at(held).awaiting(Lookup::Completion)
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
            match (editing.language.clone(), editing.synced.as_ref()) {
                (Some(language), Some(document)) => {
                    let at = editing
                        .text(&Cx::new(held, focus, &mut panes, &mut shell))
                        .cursor();
                    outstanding.at(held).sent(lookup);
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
                        answering(lookup, at, prefix, held, &post),
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
            match (editing.language.clone(), editing.synced.as_ref()) {
                (Some(language), Some(document)) => {
                    let at = editing
                        .text(&Cx::new(held, focus, &mut panes, &mut shell))
                        .cursor();
                    let path = document.path.clone();
                    // `T036` answers in *places*, and what a place means
                    // depends on the question: a definition is one place and
                    // you go there; references are a *list* and `8a` draws them
                    // in the picker. Two callbacks rather than one branching
                    // inside, because the client must not know which surface
                    // is downstream (`lsp::Locations`).
                    let answer = match question {
                        Question::Definition => jumping(&post),
                        Question::References => referencing(&post, &references),
                    };
                    servers.ask(&language, question, path, at, answer);
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
                // -- `T059`: the queue's two writes ------------------------
                //
                // **Written here rather than in the applier that took the
                // call**, because the queue is on `Shell` and the loop owns it.
                // The pair is deliberately thin: a question arrives and a
                // question leaves, and *when* a float is raised over it is
                // composition's business one layer up.
                Intent::Enqueue(id, question) => shell.enqueue_ask(id, question),
                Intent::Hold(action, source) => {
                    let id = shell.mint_ask();
                    shell.enqueue_ask(id, held_question(&action, &source));
                    shell.held.insert(id, action);
                }
                Intent::OpenRepl => surface = Surface::Repl,
                Intent::CloseRepl => surface = Surface::Buffer,
                Intent::History(delta) => repl.history(delta),
                Intent::ToBuffer => {
                    editing.editor = session_buffer(&repl, &theme)?;
                    editing.retrack();
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
                // `T093` — the registry §43 found missing. The body is bound to
                // a global here rather than kept in a map on this side, so a
                // surface is exactly as live as a `define-language` or a
                // keybinding: redefine it at the REPL and the next `open-float`
                // gets the new one, with no restart and no cache to invalidate.
                Intent::DefineSurface(id, body) => {
                    let form = phosphor_steel::float::define_form(&id, &body);
                    if let Some(said) = phosphor_steel::answer::trouble(&layer.evaluate(&form)) {
                        notice = Some(said);
                    }
                }
                // `T046`. Same two lines as the surface above; the difference
                // is entirely in what the body is expected to answer.
                Intent::DefineSource(id, body) => {
                    let form = phosphor_steel::picker::define_form(&id, &body);
                    if let Some(said) = phosphor_steel::answer::trouble(&layer.evaluate(&form)) {
                        notice = Some(said);
                    }
                }
                // `T053`. **Parked, not drawn.** The notice row is the
                // statusline's, and `6b`'s REPL owns its whole frame — `draw`
                // returns early for `Composed::Frame`, statusline included — so
                // a sentence set while the REPL is up is drawn to nobody.
                // Measured: the pty test asserted markers and the query and
                // went green with `Intent::Say` deleted, because the notice it
                // was named for had never been visible to assert.
                //
                // Held until a frame that has a notice row, which is every
                // frame the REPL is not on. §6's sentences are not urgent
                // enough to interrupt a surface and are too useful to throw
                // away.
                Intent::Say(said) => shell.saying = Some(said),
                Intent::OpenPicker(id, query) => {
                    shell.picker = Some(PickerSession::open(SourceId(id), query, &shell.wake));
                    editing.open_picker = true;
                }
                // `T052`. Applied through `act` and not `apply`, because `act`
                // is the applier that holds a rope — which is the whole reason
                // this variant exists. The focused buffer, because that is what
                // *"the buffer"* means to a caller that named none; a capability
                // that wants to say which one takes a `buffer` parameter and is
                // routed by `Buffers::named` before it gets here.
                Intent::Act(action) => {
                    let outcome =
                        editing.act(&mut Cx::new(held, focus, &mut panes, &mut shell), &action);
                    if let Some(said) = phosphor_steel::answer::trouble(&outcome) {
                        notice = Some(said);
                    }
                }
                // **Dropping the rows is the whole of invalidation**, because
                // nothing caches them between opens: `Layer::source` runs the
                // procedure on every `open-picker`, which is what
                // `define-picker-source`'s *"an open picker re-derives from
                // it"* asks for. So this re-derives an open picker over that
                // source and does nothing to a closed one.
                Intent::InvalidateSource(id) => {
                    if shell
                        .picker
                        .as_ref()
                        .is_some_and(|session| session.source.0 == id)
                    {
                        editing.open_picker = true;
                    }
                }
                // Composed once, at open — the same shape `:help` has, and not
                // `define-picker-source`'s *"an open picker re-derives"*. A
                // float is a snapshot of an answer; a picker is a live query.
                Intent::OpenSurface(id, args) => match layer.surface(&id, &args) {
                    Ok(float) => {
                        // §9: opening a second replaces the first. One slot is
                        // what makes that true by construction rather than by
                        // a rule somebody remembers.
                        open_float = Some(float);
                        surface = Surface::Float;
                    }
                    Err(why) => notice = Some(why.to_string()),
                },
                Intent::CloseFloat | Intent::CloseAllFloats => {
                    open_float = None;
                    if surface == Surface::Float {
                        surface = Surface::Buffer;
                    }
                }
            }
        }

        // **`T059` / `T060` — the question float follows the queue, in one
        // place, and Q9's whole rule is the three-way `wanted` below.**
        //
        // No applier raises a float: the arms say what has been *asked* and
        // this says what is on *screen*. So `enqueue-ask` from a key and from a
        // door land on the same screen without either of them knowing what a
        // float is.
        //
        // Q9: *"a question arriving while another float holds focus sets the
        // statusline `!` and waits."* The `!` needs nothing here — it is
        // `session_state` reading the same queue — and *waits* is the middle
        // arm: nothing surfaces unless the buffer has the screen. A picker mid
        // filter, a hover, `:arch`, the REPL: all of them keep it.
        let wanted = {
            // **Still queued *and* still asking.** Checking only that the ask
            // exists made `esc later` a no-op that looked like a hang: deferring
            // leaves the question in the queue, so this read `Some`, `wanted`
            // matched `asked`, and the float it had just closed came straight
            // back up. Deferring is precisely the case where a pending question
            // is not the one on screen.
            let showing = shell
                .asked
                .filter(|id| shell.asks.contains_key(id) && !shell.deferred.contains(id));
            if showing.is_some() {
                // A question is up and still unanswered. It stays up, even
                // though an older one may have been deferred back ahead of it:
                // replacing what you are reading mid-read is the one thing a
                // queue must never do.
                showing
            } else if surface == Surface::Buffer || shell.asked.is_some() {
                // Free to surface the next one — either nothing holds the
                // screen, or what held it was the question just answered, and
                // dropping to the buffer only to raise again next pass would
                // flash the frame between two questions.
                shell.head_ask()
            } else {
                None
            }
        };
        if wanted != shell.asked {
            shell.asked = wanted;
            match shell.asked {
                Some(id) => {
                    let mut args = Args::new();
                    args.set("ask", Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)));
                    // Which surface is a fact about the *ask*, and the queue
                    // knows it: a permission ask is the one the editor is
                    // holding a verb for.
                    let surface_id = if shell.asking_about.contains_key(&id) {
                        PERMISSION_SURFACE
                    } else {
                        QUESTION_SURFACE
                    };
                    editing.float = Some((surface_id.to_owned(), Value::Record(args)));
                }
                // Answered or deferred. Leaving the float up over an ask that no
                // longer exists would draw an empty box — `Resources::ask`
                // answers `None` — that the key which made it cannot dismiss.
                None => {
                    open_float = None;
                    if surface == Surface::Float {
                        surface = Surface::Buffer;
                    }
                }
            }
        }

        // `T048`. A keystroke's `open-float`, performed where the door's
        // `Intent::OpenSurface` is performed and by the same two lines — one
        // surface registry, one composition path, whichever side asked.
        if let Some((id, args)) = editing.float.take() {
            match layer.surface(&id, &args) {
                Ok(float) => {
                    open_float = Some(float);
                    surface = Surface::Float;
                }
                Err(why) => notice = Some(why.to_string()),
            }
        }

        // `T046`. The source runs **here**, in the loop, because running it is
        // running arbitrary scheme and `Layer` is the one door into the VM.
        // Every open re-derives — `define-picker-source`'s own *"an open picker
        // re-derives from it"* — which is what makes a redefinition at the REPL
        // land with no restart.
        if std::mem::take(&mut editing.open_picker)
            && let Some(session) = shell.picker.as_mut()
        {
            {
                let id = session.source.0.clone();
                // **What the host knows and a source cannot ask.** A source
                // runs inside the VM and a query from there cannot reach the
                // editor (`OPEN-QUESTIONS.md` §42), so the focused path is
                // handed *down* as an argument rather than queried — the same
                // shape `Scope` uses for the cursor and `Languages::new` for
                // grammar names. `grep` is the caller that needs it: it reads
                // the open buffer's lines and has to know which buffer.
                let args = Value::Record(
                    Args::new()
                        .with("filter", Value::Text(session.filter.clone()))
                        .with(
                            "path",
                            editing.file.as_deref().map_or(Value::Null, |path| {
                                Value::Text(store::key_for(path).display().to_string())
                            }),
                        )
                        // The buffer's lines, for the same reason as the path
                        // and with a cost worth naming: this is a copy of the
                        // open file, per open. It is the same order as the rows
                        // the source is about to build out of them, so it does
                        // not change the shape — and the alternative is a
                        // `buffer-lines` query a source cannot reach, because
                        // `AppHost` has no editor (`T026` answers it on the
                        // keystroke side only).
                        .with(
                            "lines",
                            Value::List(
                                editing
                                    .editor
                                    .code_ref()
                                    .get_content()
                                    .lines()
                                    .map(|line| Value::Text(line.to_owned()))
                                    .collect(),
                            ),
                        )
                        // `T047` — the workspace's files, for `3d`. Walked
                        // here because no capability walks a directory and a
                        // source runs inside the VM (§42), which is the same
                        // reason the buffer's lines are handed down above.
                        //
                        // **Only for the source that asks**, because a walk is
                        // not free: `grep` and `unseen` would pay for a list
                        // they never read.
                        .with(
                            "files",
                            if id == "files" {
                                let root = std::env::current_dir().unwrap_or_default();
                                let (found, truncated) = picker::workspace_files(&root);
                                if truncated {
                                    notice = Some(
                                        "more files than the picker walks — showing the first \
                                         100,000"
                                            .to_owned(),
                                    );
                                }
                                Value::List(found.into_iter().map(Value::Text).collect())
                            } else {
                                Value::List(Vec::new())
                            },
                        )
                        // `T047` — whatever the last `request-references`
                        // answered, for the `references` source to draw. Empty
                        // for every other source, which is what makes this one
                        // argument rather than a second call shape.
                        .with(
                            "places",
                            Value::List(
                                references
                                    .lock()
                                    .map(|held| held.iter().map(FileSpan::to_value).collect())
                                    .unwrap_or_default(),
                            ),
                        ),
                );
                let order = layer.source_order();
                match layer.source(&id, &args) {
                    Ok(spans) => {
                        session.matcher.feed(spans.iter().map(picker::row_of));
                        // Published on the frame it was derived, so
                        // `picker-rows` reads what is on screen rather than
                        // re-running a source from inside the VM. See
                        // `HostState::picker_rows`.
                        host.publish_picker(Some((id, spans)));
                        surface = Surface::Picker;
                        // Read on every open, so a layer that grows a source
                        // is one `open-picker` away from tab reaching it.
                        shell.source_order = order;
                    }
                    // A source that does not exist or raised does **not** open
                    // an empty picker: `T045`'s `0/0` is the honest drawing of
                    // a source with nothing in it, and this is a different
                    // fact. Said on the notice row and nothing opens, which is
                    // `Intent::OpenSurface`'s rule for a float that raised.
                    Err(why) => {
                        shell.picker = None;
                        host.publish_picker(None);
                        notice = Some(why.to_string());
                    }
                }
            }
        }

        // **`:quit` on a session with unsaved work anywhere.** The arm
        // refused if *this* buffer was dirty, which is what it can see; a
        // second buffer edited and switched away from is what it cannot. Two
        // checks, each where the information is — and this one arrives as a
        // notice rather than an `Outcome`, the same shape `open-file`'s
        // `WouldLoseWork` already takes in the swap block above.
        if shell.quit {
            // **The forced spelling counts nothing**, rather than skipping the
            // check — which is the shape that leaves no `break` at all on the
            // forced path, and an editor you cannot leave. Caught by
            // `a_bare_phosphor_with_unsaved_work_is_still_quittable`, twice.
            let unsaved = if shell.discard {
                0
            } else {
                buffers
                    .map
                    .values()
                    .filter(|buffer| buffer.dirty.get())
                    .count()
            };
            if unsaved == 0 {
                break;
            }
            shell.quit = false;
            notice = Some(format!(
                "{unsaved} buffer{} with unsaved work — :wall, or :quit! to discard",
                if unsaved == 1 { "" } else { "s" }
            ));
        }
    }

    term.restore()?;
    Ok(())
}

/// What §5's session segment says, from the client's report and the turn.
///
/// **Two facts, one answer.** [`SessionLife`] is about the *connection* — is there an
/// agent, is it attached, did it go — and the turn is about what that agent is
/// doing. §5's enum spans both, so this is where they meet, and putting the
/// join here rather than inside the client is what keeps a rendering decision
/// out of a transport.
///
/// **[`SessionLife::Starting`] draws as `None`, and that is a gap rather than a
/// choice.** §5 lists five states and a none: *"idle, working+elapsed, waiting,
/// paused, lost"*. A session that is spawning is none of them — it is not
/// working, because no turn has begun, and it is not lost. `None` is the least
/// wrong of the six and it is still wrong: for the second or two an agent takes
/// to hand back its `initialize`, the statusline says there is no session while
/// one is starting. `T051`'s *Done when* is *"every state renders and the
/// statusline is never stale"*, so the sixth state is that task's to add or to
/// rule out.
fn session_state(
    life: &SessionLife,
    turn: Option<&(TurnId, Instant)>,
    asking: bool,
    paused: bool,
) -> SessionState {
    match life {
        SessionLife::None | SessionLife::Starting => SessionState::None,
        SessionLife::Lost(_) => SessionState::Lost,
        // **`T062` — paused outranks waiting and working both.** `7e`'s strip
        // says `⏸ claude paused` while a turn is open and a question may be
        // queued behind it, and what it means is *nothing is moving until you
        // say so*, which is the most useful thing a strip can say when it is
        // true.
        SessionLife::Attached { .. } if paused => SessionState::Paused,
        // **`T059` — waiting outranks working, and that is the point of the
        // state.** `4a`'s strip says `! claude waiting` while a turn is very
        // much still running: what the `!` means is *the next move is yours*,
        // and a strip that said `working` would be truthful about the agent and
        // useless to the person it is drawn for.
        SessionLife::Attached { .. } if asking => SessionState::Waiting,
        SessionLife::Attached { .. } if turn.is_some() => SessionState::Working,
        SessionLife::Attached { .. } => SessionState::Idle,
    }
}

/// The digit a keystroke names, `1`–`9`, or [`None`] (`T059`).
///
/// **Bare digits only.** `4a` offers `[1]`–`[3]` and a modifier makes a
/// different key: `<C-1>` is not option one typed emphatically, and treating it
/// as one is how a chord starts answering questions.
fn digit_pressed(key: KeyEvent) -> Option<u32> {
    // `SHIFT` is left in: a shifted `1` is `!` on most layouts and arrives as a
    // different `KeyCode` anyway, so excluding it would only reject a layout
    // where the digit is the shifted glyph.
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return None;
    }
    match key.code {
        KeyCode::Char(character) => character.to_digit(10).filter(|digit| *digit > 0),
        _ => None,
    }
}

/// One review block as plain data, for the `review-blocks` query (`T053`).
///
/// Region **ids**, not spans — see [`store::Block`] for why a block that
/// carried its own copy of the spans would drift from the markers after a
/// rewrite. A caller that wants the spans asks `region` for them, which is the
/// same store at the same revision.
fn block_value(block: &store::Block) -> Value {
    let mut fields = Args::new();
    fields.set(
        "block",
        Value::Int(i64::try_from(block.id.0).unwrap_or(i64::MAX)),
    );
    fields.set("title", Value::Text(block.title.clone()));
    fields.set(
        "annotation",
        block.annotation.clone().map_or(Value::Null, Value::Text),
    );
    fields.set(
        "files",
        Value::List(
            block
                .groups
                .iter()
                .map(|group| {
                    let mut row = Args::new();
                    row.set("path", Value::Text(group.path.display().to_string()));
                    row.set(
                        "annotation",
                        group.annotation.clone().map_or(Value::Null, Value::Text),
                    );
                    row.set(
                        "regions",
                        Value::List(
                            group
                                .regions
                                .iter()
                                .map(|region| {
                                    Value::Int(i64::try_from(region.0).unwrap_or(i64::MAX))
                                })
                                .collect(),
                        ),
                    );
                    Value::Record(row)
                })
                .collect(),
        ),
    );
    Value::Record(fields)
}

/// Everything the session has said, as `1b` shows it (`T054`).
///
/// **Not in [`store`], and the difference is what a transcript *is*.** The
/// region store is persisted, keyed on the workspace, and outlives the editor —
/// seen-state is *"the only mutable flag the user owns"* (§7) and is written to
/// a journal. A transcript belongs to one session: it is gone when the agent
/// is, and `T067`'s inbox is the surface for what survives. So it lives beside
/// the session that produced it, on [`Shell`].
///
/// Published to [`HostState`] when it moves rather than every frame — see
/// [`Transcript::revision`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Transcript {
    /// The turns, oldest first.
    turns: Vec<phosphor_ui::transcript::Turn>,
    /// Bumped on every change.
    ///
    /// **The whole reason this is not published per frame.** `panes` and
    /// `session` are a handful of fields and cloning them each pass costs
    /// nothing; a transcript grows without bound for as long as you leave the
    /// editor open, and a clone per frame would make an idle editor's cost a
    /// function of how much claude has said to it.
    revision: u64,
}

impl Transcript {
    /// The turn with this id, created if the transcript has not seen it.
    ///
    /// **Created rather than refused**, because the alternative loses prose:
    /// `session-prose` is `Allow`, so it can arrive for a turn this editor
    /// missed the beginning of — an adopted session (`5d`) is exactly that —
    /// and dropping it would leave a transcript that silently disagrees with
    /// the agent.
    fn at(&mut self, turn: TurnId) -> &mut phosphor_ui::transcript::Turn {
        self.revision = self.revision.saturating_add(1);
        if let Some(index) = self.turns.iter().position(|held| held.id == turn) {
            return &mut self.turns[index];
        }
        self.turns.push(phosphor_ui::transcript::Turn {
            next: None,
            id: turn,
            prompt: None,
            prose: String::new(),
            calls: Vec::new(),
            ended: None,
            since: None,
        });
        self.turns.last_mut().expect("just pushed")
    }

    /// The tool call with this id, wherever it is.
    ///
    /// **By id across every turn, not within one.** `tool-call-progress` and
    /// `tool-call-completed` carry only the call — the vocabulary says so —
    /// because a call belongs to one turn and repeating that on every message
    /// would be a second place for the two to disagree.
    fn call(&mut self, call: ToolCallId) -> Option<&mut phosphor_ui::transcript::ToolCall> {
        self.revision = self.revision.saturating_add(1);
        self.turns
            .iter_mut()
            .flat_map(|turn| turn.calls.iter_mut())
            .find(|held| held.id == call)
    }

    /// This transcript as the widget wants it.
    fn vm(&self, life: &SessionLife, paused: bool) -> phosphor_ui::transcript::TranscriptVm {
        phosphor_ui::transcript::TranscriptVm {
            // `1b`'s `claude code · acp · 4f2a` — what is running, the
            // protocol, and the session's own id shortened to the last four,
            // which is what the mockup draws and what a person can read back.
            // **`7b` changes the last field and nothing else** — `claude code
            // · acp · disconnected`, which is the header still telling you what
            // it was rather than blanking and pretending the stream was never
            // anybody's. A transcript you are reading after a drop is the one
            // time the header is load-bearing.
            header: match life {
                // `7e` — the header says `paused` where `1b` says the session's
                // id, because *what is running* is the header's subject and
                // nothing is.
                SessionLife::Attached { .. } if paused => "claude code · acp · paused".to_owned(),
                SessionLife::Attached { session } => {
                    let tail: String = session.chars().rev().take(4).collect();
                    format!(
                        "claude code · acp · {}",
                        tail.chars().rev().collect::<String>()
                    )
                }
                SessionLife::Lost(_) => "claude code · acp · disconnected".to_owned(),
                SessionLife::None | SessionLife::Starting => String::new(),
            },
            turns: self.turns.clone(),
            hints: transcript_hints(life, paused),
        }
    }

    /// One turn as plain data, for the `turn` and `turns` queries.
    fn describe(turn: &phosphor_ui::transcript::Turn) -> Value {
        let mut fields = Args::new();
        fields.set(
            "turn",
            Value::Int(i64::try_from(turn.id.0).unwrap_or(i64::MAX)),
        );
        fields.set(
            "prompt",
            turn.prompt.clone().map_or(Value::Null, Value::Text),
        );
        fields.set("prose", Value::Text(turn.prose.clone()));
        fields.set(
            "ended",
            turn.ended
                .as_ref()
                .map_or(Value::Null, |seam| Value::Text(seam.text.clone())),
        );
        fields.set(
            "calls",
            Value::List(
                turn.calls
                    .iter()
                    .map(|call| {
                        let mut row = Args::new();
                        row.set(
                            "call",
                            Value::Int(i64::try_from(call.id.0).unwrap_or(i64::MAX)),
                        );
                        row.set("verb", Value::Text(call.verb.clone()));
                        row.set(
                            "target",
                            call.target.clone().map_or(Value::Null, Value::Text),
                        );
                        row.set(
                            "summary",
                            call.outcome
                                .as_ref()
                                .map_or(Value::Null, |done| Value::Text(done.summary.clone())),
                        );
                        row.set(
                            "added",
                            call.outcome
                                .as_ref()
                                .map_or(Value::Null, |done| Value::Int(i64::from(done.added))),
                        );
                        row.set(
                            "removed",
                            call.outcome
                                .as_ref()
                                .map_or(Value::Null, |done| Value::Int(i64::from(done.removed))),
                        );
                        Value::Record(row)
                    })
                    .collect(),
            ),
        );
        Value::Record(fields)
    }
}

/// The session, as plain data for the `session` query (`T051`).
///
/// **The same `SessionState` the statusline draws**, so the two cannot drift —
/// see [`HostState::session`]. `since` rides along because §5's `Working` is
/// *"working+elapsed"*: a caller that has the state and not the mark can render
/// five of the six and not the sixth.
fn session_value(
    life: &SessionLife,
    turn: Option<&(TurnId, Instant)>,
    asking: bool,
    paused: bool,
) -> Value {
    let mut fields = Args::new();
    fields.set(
        "state",
        session_state(life, turn, asking, paused).to_value(),
    );
    fields.set(
        "turn",
        turn.map_or(Value::Null, |(turn, _)| {
            Value::Int(i64::try_from(turn.0).unwrap_or(i64::MAX))
        }),
    );
    // What a session *is*, as distinct from what it is doing — the agent's own
    // id when there is one. `5d`'s adoption picker is the reader this is for,
    // and `T057` is where it lands.
    fields.set(
        "attached",
        match life {
            SessionLife::Attached { session } => Value::Text(session.clone()),
            _ => Value::Null,
        },
    );
    Value::Record(fields)
}

/// What the editor says out loud when the session changes (`T051`).
///
/// [`None`] for a transition with nothing to announce. **Only losses and
/// arrivals**, because §6's voice is *state, then the remedy* and the two states
/// with a remedy are the two a person can do something about; a session going
/// idle between turns is not news.
///
/// **`Starting` is the transition this cannot name, and that is
/// `docs/OPEN-QUESTIONS.md` §52.** §5 lists five states and a none, and a
/// session that is spawning is none of them — so the *statusline* says `None`,
/// which is defensible for the second a local agent takes and misleading for
/// the thirty a `npx` first run takes. Saying it here instead keeps §5's list
/// intact: the strip carries the state, and the row below carries the fact that
/// something is happening.
/// The `file://` URI a transcript tool row links to (`T056`).
///
/// **OSC 8's own shape, and the two halves a terminal actually uses.** The
/// scheme takes an authority and an empty one means *this machine* —
/// `file:///Users/…` — which is what every terminal that implements OSC 8
/// accepts and what a hostname would only narrow. The fragment is the line:
/// `#L19` is the spelling editors and forges agree on, and a terminal that does
/// not understand it opens the file, which is the right degradation.
///
/// **Relative paths are resolved against the workspace rather than refused.**
/// ACP declares `locations[].path` absolute and a well-behaved agent sends one;
/// an agent that sends `src/retry.rs` anyway would otherwise produce
/// `file://src/retry.rs`, where `src` is read as an *authority* — a link to
/// another machine. Joining is what makes that impossible rather than unlikely.
///
/// **The path is percent-encoded, and it is the whole charset rather than a
/// subset.** This shipped unencoded with §56 recording the gap and the reason —
/// *"encoding is a table, and a hand-rolled subset of it is the kind of
/// almost-right this build spends its lints avoiding"*. That is true of a
/// subset and not of [`encoded`], which implements RFC 3986's `path-abempty`
/// completely: every byte outside `unreserved`, `sub-delims`, `:`, `@` and the
/// separator is escaped, so there is nothing left to be almost right about. A
/// file called `notes #2.md` produced a URI whose fragment began at the `#`.
fn jump_uri(workspace: &Path, path: &str, line: Option<u32>) -> String {
    let resolved = workspace.join(path);
    let mut uri = format!("file://{}", encoded(&resolved.display().to_string()));
    if let Some(line) = line {
        uri.push_str(&format!("#L{line}"));
    }
    uri
}

/// A path as a URI path component — RFC 3986 `path-abempty` (`T056`).
///
/// **The unreserved set is the specification's and is not shortened.** `-`, `.`,
/// `_` and `~` are unreserved; the sub-delims are `!$&'()*+,;=`; `:` and `@` are
/// allowed in a path segment; `/` is the separator and stays. Everything else —
/// space, `#`, `?`, `%`, `"`, `<`, `>`, and every non-ASCII byte — is escaped,
/// which for UTF-8 means per *byte* rather than per character, because a URI is
/// bytes.
///
/// `%` is escaped like anything else, so encoding is not idempotent — running it
/// twice gives `%2520`. That is correct and is why there is exactly one call
/// site.
fn encoded(path: &str) -> String {
    const UNRESERVED: &str = "-._~";
    const SUB_DELIMS: &str = "!$&'()*+,;=";
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        let plain = byte.is_ascii_alphanumeric()
            || UNRESERVED.as_bytes().contains(&byte)
            || SUB_DELIMS.as_bytes().contains(&byte)
            || byte == b':'
            || byte == b'@'
            || byte == b'/';
        if plain {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The option `runtime/permissions.scm`'s `allow` publishes (`T061`).
///
/// One `|`-separated string, because an option is one value. The list itself
/// lives in Steel — see that file for why it is written out rather than read
/// back — and this is the binary's copy of it.
const ALLOWED: &str = "allowed-commands";

/// Whether a rule already permits `invocation` (`T061`).
///
/// **By prefix, and the prefix is the whole design.** `7a`'s rule is
/// `(allow "git push")` and what it has to permit is
/// `git push origin retry-backoff` — the *verb*, not the exact command line. A
/// list of exact invocations would never match twice, which is a permission
/// system that asks you the same question forever.
///
/// **The boundary is a space or the end**, so `(allow "git")` does not permit
/// `gitleaks`. That is the difference between a prefix rule and a prefix
/// *match*, and getting it wrong is how an allow-list quietly grants more than
/// it says.
fn permitted(rules: Option<&str>, invocation: &str) -> bool {
    rules.is_some_and(|rules| {
        rules
            .split('|')
            .filter(|rule| !rule.is_empty())
            .any(|rule| {
                invocation == rule
                    || invocation
                        .strip_prefix(rule)
                        .is_some_and(|rest| rest.starts_with(' '))
            })
    })
}

/// The question an `Ask`-rated action becomes (`T060`).
///
/// **It names the capability and who asked**, because that is the whole content
/// of the consent: *"the language server wants to apply a workspace edit"* is
/// answerable and *"something wants permission"* is not. `7a` takes this
/// further — `T061`'s screen shows the **exact invocation** — and the shape
/// there is this one with a longer sentence.
///
/// `[1]` is yes and `[2]` is no, in that order, and `Shell::answer_ask` reads
/// the digit rather than the label: the options are built here and nowhere
/// else, so the number and its meaning cannot drift apart.
fn held_question(action: &Action, source: &str) -> phosphor_ui::question::QuestionVm {
    phosphor_ui::question::QuestionVm {
        prose: format!(
            "{source} wants to {}. This one needs you to say so.",
            action.spec().doc
        ),
        options: vec![
            AskOption {
                digit: 1,
                label: "let it".to_owned(),
            },
            AskOption {
                digit: 2,
                label: "not now".to_owned(),
            },
        ],
    }
}

/// `7a`'s question: the exact invocation, and the rule an always-allow writes.
///
/// **The invocation is shown as it will run**, which is the screen's own
/// caption — *"consequential command · exact invocation shown"*. A permission
/// ask that paraphrased what it was asking about would be asking you to trust
/// the paraphrase.
///
/// **`[2]` names the rule it is about to write.** `7a`'s footer says
/// `2 writes (allow "git push") to init.scm`, and the option's label carries
/// the same sentence — a legible rule is one you read *before* you agree to it,
/// not one you go looking for afterwards. The file is `persisted.scm` rather
/// than `init.scm`; `T101` moved machine-written forms out of the shipped tree
/// and `7a` still draws the old word.
///
/// The verb is the first two words, or the first — `git push` from
/// `git push origin retry-backoff`, `cargo` from `cargo`. Two is what `7a`
/// draws and what a subcommand needs; more would be a rule so specific it never
/// matches again.
fn permission_question(invocation: &str, files: &[PathBuf]) -> (String, String) {
    let verb: Vec<&str> = invocation.split_whitespace().take(2).collect();
    let verb = verb.join(" ");
    let touching = match files.len() {
        0 => String::new(),
        1 => format!("\ntouches {}", files[0].display()),
        many => format!("\ntouches {many} files"),
    };
    (format!("$ {invocation}{touching}"), verb)
}

/// One queued ask as plain data, for `pending-asks` and `ask` (`T060`).
///
/// **`deferred` is a field on the row rather than a second list**, because a
/// caller asking *"what is waiting"* wants to know which of them you pushed
/// back — `T067`'s inbox draws that difference and `]!` acts on it. The set on
/// `Shell` is the storage; this is the answer.
fn ask_value(id: AskId, question: &phosphor_ui::question::QuestionVm, deferred: bool) -> Value {
    let mut fields = Args::new();
    fields.set("ask", Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)));
    fields.set("prose", Value::Text(question.prose.clone()));
    fields.set("deferred", Value::Bool(deferred));
    fields.set(
        "options",
        Value::List(
            question
                .options
                .iter()
                .map(|option| {
                    let mut row = Args::new();
                    row.set("digit", Value::Int(i64::from(option.digit)));
                    row.set("label", Value::Text(option.label.clone()));
                    Value::Record(row)
                })
                .collect(),
        ),
    );
    Value::Record(fields)
}

/// The `ask` field of a row [`ask_value`] built, for the `ask` query's lookup.
///
/// **A reader beside the writer**, so the one field the lookup depends on
/// cannot be renamed on one side only.
fn ask_id_of(value: &Value) -> Option<u64> {
    let Value::Record(fields) = value else {
        return None;
    };
    match fields.get("ask") {
        Some(Value::Int(id)) => u64::try_from(*id).ok(),
        _ => None,
    }
}

/// `7b`'s line under the seam: what a dropped turn left behind.
///
/// *"disk state preserved · 2 regions arrived before the drop, marked unseen ·
/// turn may be incomplete"* — the mockup's own sentence, and the reason `7b`'s
/// caption ends *"the transcript shows the seam honestly"*. All three clauses
/// are claims this build can actually make: the buffers are on disk because
/// nothing in the session path writes them, the regions are the store's own
/// unseen count, and *may be* is the truthful modal — the client cannot know
/// whether an agent that stopped answering had finished.
///
/// **The middle clause is dropped when the count is zero** rather than drawn as
/// `0 regions`, because a reassurance about nothing is noise on a row that
/// exists to reassure.
/// The transcript pane's footer strip (`1b`, `7b`).
///
/// **`7b` is the only screen in the set whose footer changes with state**, and
/// it changes because the remedy does: an attached session's transcript offers
/// you the rows, and a dropped one offers you the session back. Both end with
/// `q close`, which is the one thing true of a transcript either way.
///
/// **The verbs are spelled in full and the mockup is not.** `7b` draws
/// `:ca reattach · :cn new session`, and Design Language §6 answers both by
/// name: *"keyhints spell the whole command — `s mark seen`, `:reattach`,
/// `:transcript`, `:diff-disk` — never cryptic contractions like `:ca` or
/// `:rr`. Abbreviations exist for typing; the UI always teaches the full
/// name."* Two of the three commands §6 lists as correct are ones this build
/// draws, so the rule is not being read against its grain. `OPEN-QUESTIONS.md`
/// §55 records the ruling and what it cost: `:cn` was renamed to
/// `:start-session`, because a contraction with no full name to teach is the
/// same mistake with nothing left to look up.
fn transcript_hints(life: &SessionLife, paused: bool) -> Vec<KeyHint> {
    let mut hints = Vec::new();
    // `7e` — three ways on from a boundary, and they are the screen's own.
    if paused {
        hints.push(KeyHint {
            key: KeySeq("<cr>".to_owned()),
            verb: "steer and resume".to_owned(),
        });
        hints.push(KeyHint {
            key: KeySeq(":resume".to_owned()),
            verb: "resume as-was".to_owned(),
        });
        hints.push(KeyHint {
            key: KeySeq(":abort".to_owned()),
            verb: "abandon the turn".to_owned(),
        });
    } else if matches!(life, SessionLife::Lost(_)) {
        hints.push(KeyHint {
            key: KeySeq(":reattach".to_owned()),
            verb: "reattach".to_owned(),
        });
        hints.push(KeyHint {
            key: KeySeq(":start-session".to_owned()),
            verb: "start a new one".to_owned(),
        });
    }
    // **`<C-w> c`, and not `1b`'s `q`.** The mockup's footer draws `q close`,
    // and `q` in this build is vim's macro-recording key — `runtime/keymaps.scm`
    // binds it to `set-macro-recording`, in normal mode, everywhere. A footer
    // naming a key that does something else is worse than a footer with one
    // fewer row, and `T088` is the precedent for how that goes unnoticed: a
    // verb with an arm, a passing gate, and nothing bound to it.
    //
    // **`1b`'s `↵ jump to file` is not here either**, for the harder reason:
    // `T056` made the tool rows clickable through OSC 8, which has no key at
    // all — the underline is the affordance. A *keyboard* jump needs a focused
    // row in a transcript, which no task owns. `OPEN-QUESTIONS.md` §56.
    hints.push(KeyHint {
        key: KeySeq("<C-w> c".to_owned()),
        verb: "close".to_owned(),
    });
    hints
}

fn survived(unseen: usize) -> String {
    let mut said = String::from("disk state preserved");
    if unseen > 0 {
        let plural = if unseen == 1 { "" } else { "s" };
        said.push_str(&format!(
            " · {unseen} region{plural} arrived before the drop, marked unseen"
        ));
    }
    said.push_str(" · turn may be incomplete");
    said
}

fn session_notice(life: &SessionLife) -> Option<String> {
    match life {
        // §6: lowercase, telegraphic, factual.
        SessionLife::Starting => Some("starting claude".to_owned()),
        SessionLife::Lost(failure) => Some(failure.to_string()),
        SessionLife::Attached { .. } => Some("claude attached".to_owned()),
        SessionLife::None => None,
    }
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
/// **It replaces what was on screen, and that is now a choice rather than a
/// limit.** It read *"one pane, so this replaces what was on screen; `T088`
/// gives the session a pane of its own"* — and `T088` landed, so the sentence
/// stopped being about what was possible. Opening it in a split is one
/// `split-pane` away; whether `C-c buffer` *should* is `T054`'s question, since
/// the transcript is the surface that owns *"a pane, not a float"* (Design
/// Language §9). Nothing is lost by replacing: `S2` has no save path, so the
/// file on disk is untouched and `q` already discards the same unsaved edits.
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

/// The host's frame: the split tree, composed.
///
/// **The composition `scripts/lint-node-kinds.sh` recorded as owed** — two
/// rows, `Pane` and `Buffer`, both against `T088`, both deleted in the commit
/// that added this function. The buffer was on screen every frame and was not
/// a node: `draw` rendered `BufferView` straight into the body area, outside
/// the tree, which is the *"reachable, working, and unreachable through the
/// protocol that names it"* shape the record described.
///
/// **`Node::Gutter` is deliberately not composed here**, and the record's
/// empty creditor stays empty. A `Node::Buffer` draws the state column itself
/// — `interpret.rs`'s arm calls `.state_column(resources.state_marks(…))` —
/// so composing a gutter beside it would draw the column twice and would give
/// a creditor to the one entry whose whole point is having none.
///
/// **The ids are the real ones since step 11b, and that is not cosmetic.**
/// They were `THE_BUFFER = BufferId(1)` and `THE_PANE = PaneId(1)`, two
/// constants whose own doc said *"the number here is arbitrary and the naming
/// is not"* — true while `Painted::editor` resolved every id to the buffer on
/// screen. It resolves by id now, and `Buffers` mints from zero, so the
/// composition was naming a buffer that did not exist: the state column went
/// blank the moment the door started looking. Caught by
/// `a_diagnostic_outranks_an_unseen_region_on_the_same_row`, which is a
/// screen test and the only kind that could see it.
///
/// `soft_wrap` is what the option says this frame, not what the editor was
/// last wrapped to. The prop is a **request**: `Resources`' own doc says it
/// *"cannot be honoured from here"* because re-wrapping needs `&mut Editor`,
/// so the loop applies it above and the node reports it. Saying `false` while
/// the loop wraps would make the tree lie about the frame it composed.
/// **The vocabulary already had everything this needs**, which is the check
/// worth stating: `Node::Split` *"divides its area along an angle and gives each
/// child a share"*, `Constraint::Percent` is one of its five shapes, and
/// `Node::Pane` carries the id and the focus flag. Composing N panes added no
/// node kind and no prop — a split tree was always what these were for, and
/// `one_pane` was the degenerate case.
///
/// **`focused` is per pane and exactly one is true.** §9's rule is *"panes
/// never dim each other — only floats dim what is behind them"*, so this says
/// which pane keystrokes go to and says nothing about brightness.
///
/// A pane whose buffer this host does not have composes `Node::Empty`, which
/// draws nothing: `query.rs`'s *"an absent thing answers empty"*, and the state
/// a transcript pane will be in until `T054` fills it.
/// §5's tab bar, one tab per pane, in the order the tree lays them out.
///
/// **Empty below two panes**, which is §5's *"appears only with 2+ panes"* said
/// on the composition side. [`Geometry::take_tab_bar`] says it on the layout
/// side, because that is the half that can decline to spend the row; this half
/// is what makes the strip a composition rather than a widget the host decides
/// to call, and it is the half `scripts/lint-node-kinds.sh` is asking about.
///
/// **The order is [`PaneTree::leaves`]'** — first-then-second, which is left to
/// right and top to bottom — and not [`Panes`]'s `BTreeMap`, whose order is
/// mint order. Split the left pane twice and the ids interleave; the strip has
/// to read the way the screen does.
///
/// `unseen` is the per-buffer count the `decorate` pass already computes for
/// every buffer and used to keep for one. That is what makes *"per-tab unseen
/// counts track the store"* true by construction rather than by a second query:
/// there is one count, computed once, and the statusline and the tab bar are
/// two readers of it.
fn compose_tabs(panes: &Panes, buffers: &Buffers, unseen: &BTreeMap<BufferId, u32>) -> Node {
    let leaves = panes.tree.leaves();
    if leaves.len() < 2 {
        return Node::Empty {};
    }
    // One `getcwd` for the strip rather than one per tab, and only on the
    // frames the strip exists. The workspace is the directory the editor was
    // started in — `Timeline::open_at`'s rule, and the honest root until `T071`
    // makes it the repository's.
    let root = std::env::current_dir().unwrap_or_default();
    let tabs = leaves
        .into_iter()
        .map(|id| {
            let pane = panes.at(id);
            Tab {
                title: tab_title(pane, buffers, &root),
                kind: pane.holds(),
                unseen: pane
                    .buffer
                    .and_then(|held| unseen.get(&held).copied())
                    .unwrap_or(0),
                active: id == panes.focus,
            }
        })
        .collect();
    Node::TabBar { tabs }
}

/// What one tab says.
///
/// §5 draws `src/retry.rs` and `transcript` — a path spelled the way the
/// workspace spells it, and a surface spelled by name.
///
/// **A file outside the workspace is its basename**, which is vim's own rule
/// for a tab label and is the only one that keeps the strip usable: an absolute
/// path out of `/var/folders/…` is fifty cells before it says anything, so two
/// of them would push §11's second rung on an 80-column terminal and leave one
/// tab on screen. Under the workspace the relative path is short *and*
/// unambiguous, which is why it is preferred where it applies; outside it, the
/// basename can collide and the absolute form cannot be read anyway.
///
/// **The title is composition's and not the widget's**, which is why the
/// contraction question does not arise here: §11's ladder shortens a *line*
/// that does not fit, and `T089`'s strip drops whole tabs instead (the widget's
/// module docs argue why). A tab is as long as its file is.
fn tab_title(pane: &Pane, buffers: &Buffers, root: &Path) -> String {
    match pane.holds() {
        PaneKind::Transcript => "transcript".to_owned(),
        // v1.5, and `split-pane` refuses it by naming the task — so this arm is
        // reachable only from a tree the loop did not build.
        PaneKind::Custom => "pane".to_owned(),
        PaneKind::Buffer => pane
            .buffer
            .and_then(|held| buffers.at(held).file.clone())
            .map_or_else(
                // §6's voice: lowercase and factual. vim's `[No Name]`, said
                // the way this editor says things.
                || "[no name]".to_owned(),
                |path| shown_path(&path, root),
            ),
    }
}

/// A path as a **surface** shows it: workspace-relative, or the basename.
///
/// Two callers and one rule, which is the point of it being a function.
/// `T089`'s tab titles established it and `T058`'s anchor chip needed the same
/// thing — `1c` draws `src/retry.rs`, and both surfaces have a row to share.
///
/// **Not [`store::key_for`]**, and the difference matters. That one is the
/// *store's* rule: it strips the working directory so a door declaring
/// `src/retry.rs` and an editor showing `/work/src/retry.rs` agree about which
/// file they mean, and a path outside the workspace has to keep its absolute
/// form or it would name a different file. This one is about **reading**: a
/// path nobody can shorten honestly is still fifty cells of `/private/tmp/…`
/// before it says anything, so a surface falls back to the basename, which is
/// vim's own answer for a tab label. Being wrong here costs a reader a moment;
/// being wrong there costs a marker.
fn shown_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .or_else(|| path.file_name().map(Path::new))
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn compose_panes(
    tree: &PaneTree,
    panes: &Panes,
    focus: PaneId,
    soft_wrap: bool,
    folded: &[TurnId],
) -> Node {
    match tree {
        PaneTree::Leaf(id) => {
            let pane = panes.at(*id);
            Node::Pane {
                pane: *id,
                holds: pane.holds(),
                focused: *id == focus,
                // **What a pane holds decides its child, and that is the whole
                // of `set-pane-content`** (`T054`): `:transcript` writes
                // `Pane::holds` and the next composition draws a different
                // kind. There is no second pane model for a surface that is
                // not a buffer.
                child: Child::new(match pane.holds() {
                    PaneKind::Transcript => Node::Transcript {
                        // `1b` is a transcript you are watching, so it holds
                        // the newest turn. `T056`'s jump links are what will
                        // want this false.
                        follow: true,
                        folded: folded.to_vec(),
                    },
                    // v1.5's agent-built pane. Empty rather than a refusal:
                    // `split-pane` will not make one, so a tree carrying this
                    // came from somewhere that is not the loop.
                    PaneKind::Custom => Node::Empty {},
                    PaneKind::Buffer => pane
                        .buffer
                        .map_or(Node::Empty {}, |buffer| Node::Buffer { buffer, soft_wrap }),
                }),
            }
        }
        PaneTree::Split {
            axis,
            first,
            second,
            first_share,
        } => Node::Split {
            axis: match axis {
                Axis::Columns => ViewAxis::Columns,
                Axis::Rows => ViewAxis::Rows,
            },
            slots: vec![
                Slot::new(
                    Constraint::Percent {
                        percent: u32::from(*first_share),
                    },
                    compose_panes(first, panes, focus, soft_wrap, folded),
                ),
                // **The remainder, not the complement.** `Percent { 100 - n }`
                // would round independently and leave a column nothing owns at
                // odd widths — the same failure `PaneTree::layout` avoids by
                // giving the far side what the near side left, and the same
                // fix: `Fill` takes what is left.
                Slot::new(
                    Constraint::Fill { weight: 1 },
                    compose_panes(second, panes, focus, soft_wrap, folded),
                ),
            ],
        },
    }
}

/// What the interpreter draws this frame, and how much of the frame it owns.
///
/// **The discriminator used to be the tree's own shape** — `draw` read an
/// empty root as *"a float over what the widgets painted"* and anything else
/// as *"this surface owns the frame"*. That reading died with the widget path:
/// the host's frame has a [`Node::Pane`] at its root now, and a shape test
/// would have read it as a whole-frame surface and drawn it over the
/// statusline. Which is not a fact about the tree at all — it is a fact about
/// who composed it — so it is spelled rather than sniffed.
#[derive(Debug)]
enum Composed {
    /// A surface the editor layer composed as a whole frame — `6a`/`6b`'s
    /// REPL, whose own doc calls [`Repl::frame`] *"the whole of `6b`: the
    /// surface, and the statusline under it."* It draws its own chrome, so the
    /// host draws none: no strips, no statusline, no cursor.
    Frame(Tree),
    /// The host's frame: [`compose_panes`], with whatever float this surface hangs
    /// over it. The two strips, the statusline and the cursor are the host's,
    /// around it.
    Pane(Tree),
}

/// §8's degradation, decided once per frame.
///
/// `crossterm` drops a background colour on a `NO_COLOR` terminal where it
/// writes the escape, so a state bar — one cell of background and no glyph —
/// comes out blank and the unseen markers disappear.
/// `phosphor_ui::gutter::state_cell` has always known how to draw `▎` instead;
/// nothing selected it, so §8's fallback was unreachable and `V009`'s degraded
/// capture is what made that visible.
///
/// The capability question is `phosphor-term`'s: `phosphor-ui` takes
/// `ratatui-core` only and reads no environment, deliberately. A function
/// rather than two lines inside [`draw`] so the choice is testable without a
/// terminal — the read itself is `phosphor_term::colour_available`'s.
const fn state_fill(colour: bool) -> Fill {
    if colour { Fill::Block } else { Fill::Marker }
}

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

/// Takes `rows` off the **top** of `body` for a strip, or [`None`] if they will
/// not fit.
///
/// [`take_rows`]'s mirror, and §5 is why there are two: *"Three strips of
/// chrome, ever: tab bar (top …), statusline (bottom …), and tmux below it"*.
/// The same §11 rule holds in both directions — a strip that cannot have its
/// rows without leaving the buffer at least one is not drawn at all.
fn take_top_rows(body: &mut Rect, rows: u16) -> Option<Rect> {
    if rows == 0 || body.height <= rows {
        return None;
    }
    let taken = Rect {
        height: rows,
        ..*body
    };
    body.y += rows;
    body.height -= rows;
    Some(taken)
}

/// Where every part of the frame goes, computed once per pass.
///
/// **It was computed twice**, and the two answers were not the same one. The
/// loop split the terminal for what needs `&mut editor` — the wrap width and
/// the rect a scroll is measured against — and [`draw`] split `frame.area()`
/// again for what it painted, taking the two strips off its own copy of the
/// body. So on a frame with `3c`'s leader grid or `8e`'s hint row up, the loop
/// measured against rows that were about to be given away, and the divergence
/// was invisible because no consumer read both.
///
/// Both are kept here rather than reconciled, and [`Geometry::body`] versus
/// [`Geometry::pane`] is that divergence written down: reconciling them moves a
/// viewport, which is a pixel change and was not that step's.
///
/// **[`PaneTree::layout`] divides [`Geometry::body`] into N rects since step
/// 11**, which is what this type existed to make possible.
/// [`Geometry::pane`] is still the whole strip and is what the chrome measures
/// against; the per-pane rects live on [`Pane::area`], because a rect a *pane*
/// owns is the pane's and not the frame's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Geometry {
    /// The whole terminal — what a tree-composed surface owns.
    frame: Rect,
    /// The buffer's rows **before** the strips come off them. What the loop
    /// measures a wrap width, a scroll and a mouse hit test against.
    body: Rect,
    /// The buffer's rows **after** them: what [`compose_panes`]'s tree is rendered
    /// into, what a float dims, and what the cursor is placed inside.
    pane: Rect,
    /// `3c`'s leader grid, when a prefix is half-typed and its rows fit.
    leader: Option<Rect>,
    /// `8e`'s unknown-key hint row, when there is one and it fits.
    hint: Option<Rect>,
    /// The statusline's row — the ex line and a notice borrow it.
    status: Rect,
    /// `1c`'s anchored prompt row, below the statusline (`T058`).
    ///
    /// **Only when the prompt carries an anchor**, and the mockups are why.
    /// `1c` is the *only* screen in the set that draws a prompt at all, and it
    /// draws one with a chip, below a statusline that is still there. Every
    /// other screen's `:` line is vim's — the last row, borrowed. So an
    /// unanchored prompt keeps that and an anchored one gets its own row: the
    /// chip plus the message does not fit in a row the statusline is also
    /// using.
    prompt: Option<Rect>,
    /// §5's tab bar, when there are two or more panes to name (`T089`).
    ///
    /// **Taken off [`Geometry::body`] as well as [`Geometry::pane`], which the
    /// two bottom strips are not.** That divergence is recorded on this type
    /// and it is a divergence about *timing*: whether the leader grid is up is
    /// the VM's answer and arrives in the middle of the pass, by which point
    /// the wrap width has been measured. Whether there is a second pane is the
    /// tree's answer and is knowable at the top, so this strip can come off
    /// before anything measures anything — and it has to, because the row it
    /// takes is a row of the panes, every frame there is more than one of them.
    tabs: Option<Rect>,
}

/// The frame's layout, from the size the next frame will be drawn at.
///
/// Two calls rather than one, and deliberately: the split is knowable at the
/// top of the pass and the two strips are not. Whether the leader grid is up is
/// [`under`]'s answer, and `under` asks the VM — `Layer::entries` sets the flag
/// `Layer::stale` reads, and that read happens **once**, in the middle of the
/// pass. Hoisting the question above it would move a Steel call across the one
/// door the frame cache learns staleness through, to buy nothing. So the split
/// happens here and [`Geometry::take_strips`] finishes the layout where the
/// conditions are known — one `split`, one pass of `take_rows`, and a `draw`
/// that lays out nothing.
fn lay_out(area: Rect) -> Geometry {
    let (body, status) = split(area);
    Geometry {
        frame: area,
        body,
        pane: body,
        leader: None,
        hint: None,
        status,
        tabs: None,
        prompt: None,
    }
}

impl Geometry {
    /// Every rect intersected with the area actually being rendered.
    ///
    /// **This exists because a rect that outlives its buffer panics the
    /// editor**, and the step that introduced [`Geometry`] made that reachable
    /// for the first time. The rects are laid out from `term.size()` at the top
    /// of the pass; `term.draw` runs ~300 lines later and calls ratatui's
    /// `autoresize()` first, whose own comment is *"otherwise we get glitches if
    /// shrinking or potential desync between widgets and the terminal (if
    /// growing), which may OOB"*. A height shrink landing in that window hands
    /// [`draw`] a `pane` taller than the buffer.
    ///
    /// Nothing downstream catches it. `buffer_view`'s `set_cell` clips to the
    /// rect it was *passed* rather than to the buffer, and `Buffer::set_stringn`
    /// clamps `x` and never `y` — so the write reaches `index_of` and panics
    /// out of `Raw::synchronized_frame`, which re-raises after restoring the
    /// terminal. A resize drag emits a stream of sizes and reopens the window on
    /// every one.
    ///
    /// `Frame::area`'s own doc states the contract this restores: *"It is the
    /// area of the buffer that is actually being rendered for this pass."* On
    /// every pass where the size is stable this is the identity, which is why it
    /// costs nothing to hold to.
    fn clamped_to(&self, area: Rect) -> Self {
        Self {
            frame: area,
            body: self.body.intersection(area),
            pane: self.pane.intersection(area),
            leader: self.leader.map(|rect| rect.intersection(area)),
            hint: self.hint.map(|rect| rect.intersection(area)),
            status: self.status.intersection(area),
            tabs: self.tabs.map(|rect| rect.intersection(area)),
            prompt: self.prompt.map(|rect| rect.intersection(area)),
        }
    }

    /// Takes the two strips off the buffer's rows, bottom-up.
    ///
    /// The order is `8e`'s and it is load-bearing: the leader grid sits
    /// directly above the statusline and the hint row between it and the code,
    /// so the grid comes off first. Both are [`take_rows`], so a terminal too
    /// short drops a strip rather than squeezing it (§11).
    /// Takes §5's tab-bar row off the top, when `panes` is two or more.
    ///
    /// **This is where *"appears only with 2+ panes"* lives**, and it lives
    /// here rather than in the widget for a reason the widget's own module docs
    /// state: a strip that decides whether it exists has already been given a
    /// row, and a row given to a strip that draws nothing is a row the buffer
    /// lost. Composition answers `Node::Empty` on the same condition, so the
    /// rule is stated twice and both statements are tested — but only one of
    /// them can give the row back.
    ///
    /// Off `body` too, unlike [`Geometry::take_strips`] — see
    /// [`Geometry::tabs`].
    fn take_tab_bar(&mut self, panes: usize) {
        if panes < 2 {
            return;
        }
        self.tabs = take_top_rows(&mut self.pane, 1);
        if self.tabs.is_some() {
            take_top_rows(&mut self.body, 1);
        }
    }

    /// Takes `1c`'s prompt row off the bottom, when the prompt has an anchor.
    ///
    /// Below the leader grid and the hint row, which is to say **directly above
    /// the statusline** — `1c` draws it there, and it is where vim's `:` line
    /// lives too, one row further down.
    fn take_prompt(&mut self, anchored: bool) {
        if !anchored {
            return;
        }
        self.prompt = take_rows(&mut self.pane, phosphor_ui::prompt::rows());
    }

    fn take_strips(&mut self, leader: &[KeyHint], hint: bool, theme: &Theme) {
        self.leader = (!leader.is_empty())
            .then(|| {
                let rows =
                    KeyHints::new(leader, Density::Grid, theme).desired_height(self.pane.width);
                take_rows(&mut self.pane, rows)
            })
            .flatten();
        self.hint = hint.then(|| take_rows(&mut self.pane, 1)).flatten();
    }
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

/// Every buffer's in-flight requests, by the buffer that asked (`T088`, step 9).
///
/// **[`Outstanding`] counted with no key at all** — three bare `u32`s for the
/// whole session. With a second buffer open that is not an approximation but a
/// wrong answer twice over: the insert-mode trigger's *"one request in flight
/// at a time"* gate reads a count that a different file's request is holding
/// open, so typing in B waits on A; and an answer for B is taken off A's count,
/// so A's gate re-arms on an answer it never asked for.
///
/// Keyed, both stop being possible. The `Default` per entry is *"nothing in
/// flight"*, which is the right answer for a buffer nobody has asked about yet.
#[derive(Debug, Default)]
struct Asking {
    map: BTreeMap<BufferId, Outstanding>,
}

impl Asking {
    /// This buffer's counts, created empty the first time it asks.
    fn at(&mut self, buffer: BufferId) -> &mut Outstanding {
        self.map.entry(buffer).or_default()
    }

    /// Whether **any** buffer is waiting on a `lookup`.
    ///
    /// The poll deadline is the session's — a parked `recv_until` is what stops
    /// the loop spinning at full tilt for the length of a round trip, and an
    /// answer for any buffer is an event that wakes it. So this one question is
    /// deliberately not keyed, while the two that decide what to *do* are.
    fn anyone_awaiting(&mut self, lookup: Lookup) -> bool {
        self.map
            .values_mut()
            .any(|outstanding| outstanding.awaiting(lookup))
    }
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
fn answering(
    lookup: Lookup,
    at: Position,
    prefix: String,
    buffer: BufferId,
    post: &Post,
) -> Insights {
    let post = Arc::clone(post);
    Arc::new(move |insight: Insight| {
        let action = match insight {
            Insight::Completions(items) => LspAction::IngestCompletions {
                items: phosphor_buffer::lsp::narrow(items, &prefix)
                    .into_iter()
                    .map(offered)
                    .collect(),
                at,
                buffer: Some(buffer),
            },
            Insight::Signature(signature) => LspAction::IngestSignatureHelp {
                signature: Some(signed(*signature)),
                at,
                buffer: Some(buffer),
            },
            Insight::Hover(prose) => LspAction::IngestHover {
                prose,
                at,
                buffer: Some(buffer),
            },
            Insight::Nothing => match lookup {
                Lookup::Completion => LspAction::IngestCompletions {
                    items: Vec::new(),
                    at,
                    buffer: Some(buffer),
                },
                Lookup::SignatureHelp => LspAction::IngestSignatureHelp {
                    signature: None,
                    at,
                    buffer: Some(buffer),
                },
                Lookup::Hover => LspAction::IngestHover {
                    prose: Vec::new(),
                    at,
                    buffer: Some(buffer),
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
    Arc::new(move |places: Vec<FileSpan>| {
        let Some(place) = places.into_iter().next() else {
            return;
        };
        post(Action::File(FileAction::OpenFile {
            path: place.path,
            at: place.span.map(|span| span.start),
            pane: PaneRef::Focused {},
        }));
    })
}

/// Where a `request-references` answer lands on its way to the picker
/// (`T047`).
///
/// **A slot rather than an `Action`, and that is the re-homing note made
/// concrete.** `TASKS.md` records why this task owns `request-references` at
/// all: *"`LanguageServers::ask` answers a `Vec<FileSpan>` and nothing in the
/// vocabulary carries a list of places"*. It still does not. So the answer
/// arrives here, the callback posts an ordinary `open-picker`, and the loop
/// hands the places to the `references` source as arguments — the same
/// *"the host resolves what only the host can"* seam `grep` uses for the
/// buffer's lines.
///
/// The alternative is a capability whose payload is a list of places, which is
/// a vocabulary change to carry one answer to one surface. If a second consumer
/// ever wants the list, that is the moment to make it one.
type References = Arc<Mutex<Vec<FileSpan>>>;

/// The `Locations` callback for `request-references`.
///
/// Fills the slot and asks for the picker. Answering *no* places still opens
/// it: `0/0` is the honest drawing of *"nothing uses this"*, where silence is
/// indistinguishable from a server that never replied.
fn referencing(post: &Post, slot: &References) -> phosphor_buffer::lsp::Locations {
    let post = Arc::clone(post);
    let slot = Arc::clone(slot);
    Arc::new(move |places: Vec<FileSpan>| {
        if let Ok(mut held) = slot.lock() {
            *held = places;
        }
        post(Action::Picker(PickerAction::OpenPicker {
            source: SourceId("references".to_owned()),
            query: None,
        }));
    })
}

/// The surface id `:arch` and `open-arch` both name.
///
/// One constant rather than two string literals, for the reason
/// [`PERSIST_FILE`] gives: two spellings of one name drift, and the one that
/// drifts silently is the Rust one. `runtime/arch.scm` registers it.
const ARCH_SURFACE: &str = "arch";

/// `T057`'s dashboard surface id, as `runtime/dashboard.scm` registers it.
const DASHBOARD_SURFACE: &str = "dashboard";

/// `runtime/asks.scm`'s float — `4a`, claude asking mid-turn (`T059`).
const QUESTION_SURFACE: &str = "question";

/// `runtime/permissions.scm`'s float — `7a`, claude asking to run something
/// (`T061`).
///
/// **A second surface and not a second body.** The two screens differ by their
/// chrome — *"needs input"* against *"wants to run"* — and share
/// `view/question` for everything inside it, which is the smallest true reading
/// of two drawings that are the same drawing with a different sentence at the
/// top.
const PERMISSION_SURFACE: &str = "permission";

/// The start line of a region record, if it names `path` (`T049`).
///
/// Reads the record the `regions` query already answers rather than reaching
/// into the store a second way — so what `]u` walks and what a door reads
/// cannot diverge.
fn region_line(value: &Value, path: &Path) -> Option<u32> {
    let Value::Record(fields) = value else {
        return None;
    };
    let Some(Value::Text(named)) = field(fields, "path") else {
        return None;
    };
    if Path::new(named.as_str()) != path {
        return None;
    }
    let Some(Value::Record(span)) = field(fields, "span") else {
        return None;
    };
    let Some(Value::Record(start)) = field(span, "start") else {
        return None;
    };
    match field(start, "line") {
        Some(Value::Int(n)) => u32::try_from(*n).ok(),
        _ => None,
    }
}

/// One field of a record, by name.
fn field<'a>(fields: &'a Args, name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// A count as the wire's integer, saturating rather than wrapping.
fn count(n: usize) -> Value {
    Value::Int(i64::try_from(n).unwrap_or(i64::MAX))
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
///
/// **It returns the float and not a tree.** It used to hang one off an empty
/// root, because an empty root was how a float said *"over what the widgets
/// painted"*; the widgets are gone and the pane underneath is a composition,
/// so the caller hangs this over [`compose_panes`] instead. Four other surfaces
/// took the same shape and changed the same way.
fn passive_float(editing: &Editing) -> Option<ViewFloat> {
    let body = if editing.completion.is_some() {
        Node::Completion {}
    } else if editing.signature.is_some() {
        Node::Signature {}
    } else {
        return None;
    };
    Some(ViewFloat::new(Mood::Passive, body))
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
    /// Every open buffer's editor, by id.
    ///
    /// **A map, and the door is the reason.** [`Resources::editor`] takes a
    /// `BufferId` and answered the same editor whatever it was handed, under a
    /// doc reading *"one buffer, and it is implicit"* — honest while there was
    /// one, and a second pane showing a second file would have drawn the
    /// focused file's text in both.
    editors: &'a BTreeMap<BufferId, &'a Editor>,
    /// Every open buffer's state column, by id, on the same terms.
    columns: &'a BTreeMap<BufferId, Vec<StateMark>>,
    completion: Option<&'a CompletionVm>,
    signature: Option<&'a SignatureVm>,
    /// `T045`. Computed before the draw rather than during it: the matcher
    /// needs `&mut` to tick and `Resources` has no `&mut` in it and must never
    /// grow one, so the loop ticks once per frame and lends the answer.
    picker: Option<&'a PickerVm>,
    /// `T054`. Built when the transcript moves rather than per frame, for
    /// [`Transcript::revision`]'s reason, and lent here for the same one
    /// [`Painted::picker`] is: `Resources` has no `&mut` in it.
    transcript: Option<&'a phosphor_ui::transcript::TranscriptVm>,
    /// `T059`'s questions, by id — see [`Overlay::asks`] for why this one is
    /// keyed and its neighbours are not.
    asks: &'a BTreeMap<AskId, phosphor_ui::question::QuestionVm>,
}

impl<'a> Overlay<'a> {
    /// The editor behind the focused buffer, if this host still has it.
    fn focused_editor(&self, editors: &'a BTreeMap<BufferId, &'a Editor>) -> Option<&'a Editor> {
        editors.get(&self.focused.0).copied()
    }
}

impl std::fmt::Debug for Painted<'_> {
    /// The editor holds a rope, a tree-sitter tree and a highlight cache and
    /// implements no `Debug`; what is printable is what this frame is showing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painted")
            .field("buffers", &self.editors.len())
            .field("completion", &self.completion.is_some())
            .field("signature", &self.signature.is_some())
            .field("picker", &self.picker.is_some())
            .finish_non_exhaustive()
    }
}

impl Resources for Painted<'_> {
    /// **A real lookup since `T088`'s step 11.** It answered `Some(the one
    /// editor)` for every id, under a doc reading *"one buffer, and it is
    /// implicit"* — which was honest while there was one and became a lie the
    /// moment a second pane could show a second file.
    ///
    /// An id this host does not have answers `None`, which draws nothing: a
    /// stale composition must never be able to break a frame.
    fn editor(&self, buffer: BufferId) -> Option<&Editor> {
        self.editors.get(&buffer).copied()
    }

    /// The state column for a buffer, on [`Resources::editor`]'s terms: a real
    /// lookup, and an id this host does not have draws the ground rather than
    /// another buffer's error markers.
    fn state_marks(&self, buffer: BufferId) -> &[StateMark] {
        self.columns.get(&buffer).map_or(&[][..], Vec::as_slice)
    }

    fn completion(&self) -> Option<&CompletionVm> {
        self.completion
    }

    fn signature(&self) -> Option<&SignatureVm> {
        self.signature
    }

    /// **One picker, and it is implicit** — which [`Resources::editor`] no
    /// longer is, and the difference is the point. That one became a real
    /// lookup at `T088`'s step 11 because there are two buffers to tell apart;
    /// this one stays implicit because there is one session and a host with two
    /// open pickers is not a thing this editor can be in. `view.rs` puts the
    /// source on the node so a composition can name one, and there is one to
    /// name.
    fn picker(&self, _source: &SourceId) -> Option<&PickerVm> {
        self.picker
    }

    /// `T054`. One session, so — like the picker above — no id is consulted:
    /// `Node::Transcript` names none, because there is one transcript and it
    /// is the session's.
    fn transcript(&self) -> Option<&phosphor_ui::transcript::TranscriptVm> {
        self.transcript
    }

    /// `T059`. A real lookup, for the reason [`Resources::editor`] became one:
    /// a float that named an ask and got whichever ask happened to be newest
    /// would let you answer a question you were not reading.
    fn ask(&self, ask: AskId) -> Option<&phosphor_ui::question::QuestionVm> {
        self.asks.get(&ask)
    }
}

/// What rides over the buffer on this frame, and what claims the chrome row.
///
/// One struct rather than several parameters, so [`draw`] stays inside
/// `clippy::too_many_arguments` — and because they compose in one place: the
/// two strips come off the bottom of the body in `8e`'s order, and the ex line,
/// the notice and the statusline are three things competing for one row.
///
/// [`Overlay::status`] moved in here when [`Geometry`] became a parameter: it
/// is read in exactly one place, as [`Overlay::chrome`]'s `None` arm, so the
/// two travelling apart was the accident.
#[derive(Debug, Clone, Copy)]
struct Overlay<'a> {
    /// The ex line, or a notice, where the statusline goes.
    chrome: Option<Chrome<'a>>,
    /// What `runtime/statusline.scm` composed, drawn when [`Overlay::chrome`]
    /// is not borrowing the row. [`None`] when the layer composed none.
    status: Option<&'a Tree>,
    /// `3c`'s which-key grid, for whatever prefix is half-typed. Empty when
    /// nothing is.
    leader: &'a [KeyHint],
    /// `8e`'s once-per-session unknown-key row, on the frame it was taught.
    hint: Option<&'a Node>,
    /// Which buffer has focus, and the rect its pane occupies.
    ///
    /// **Two things in the frame belong to exactly one pane**, however many
    /// there are: the terminal's own cursor goes in one place, and the
    /// unknown-key strip's indent is measured against one buffer's gutter.
    /// Both read the focused editor directly before step 11b, which was the
    /// same thing while the frame *was* the pane.
    focused: (BufferId, Rect),
    /// `T040`'s state column for **every** buffer, already resolved through
    /// §3's ladder — one mark per visual row, computed by the loop over every
    /// source of regions there is. See [`decorate`] for why that is the host's
    /// job and not the gutter's, and why it is a map.
    columns: &'a BTreeMap<BufferId, Vec<StateMark>>,
    /// The live completion session and the live signature-help or hover answer
    /// (`T038`, `T039`), which [`Painted`] lends the interpreter. They ride
    /// here because they are the two things the frame needs that the buffer
    /// does not hold.
    completion: Option<&'a CompletionVm>,
    signature: Option<&'a SignatureVm>,
    /// `T045`. Computed before the draw rather than during it: the matcher
    /// needs `&mut` to tick and `Resources` has no `&mut` in it and must never
    /// grow one, so the loop ticks once per frame and lends the answer.
    picker: Option<&'a PickerVm>,
    /// `T089`'s tab bar, composed by [`compose_tabs`] and drawn into
    /// [`Geometry::tabs`]. `Node::Empty` below two panes, which is the frame
    /// where [`Geometry::tabs`] is [`None`] and nothing asks for this at all.
    tabs: &'a Tree,
    /// `T054`'s transcript, when the session has said anything.
    transcript: Option<&'a phosphor_ui::transcript::TranscriptVm>,
    /// `T059`'s questions, by id.
    ///
    /// **A map where its neighbours are an `Option`**, and `Node::Question`'s
    /// own prop is why: there is one completion list and one transcript, and
    /// there are as many asks as claude has asked. A float composed for ask 8
    /// must draw ask 8 even with a newer one behind it.
    asks: &'a BTreeMap<AskId, phosphor_ui::question::QuestionVm>,
    /// This frame's reading of the app clock (`T050`).
    ///
    /// The whole animation budget: `Node::Spinner` and `Node::Elapsed` render
    /// `now - since`, so a spinner turning costs *frames* and zero
    /// recompositions. It was never read until there was something to wait on.
    now: Millis,
}

/// One frame: the pane, the strips over it, then the statusline.
///
/// **There is one render of the surface and no widget.** The buffer arrives as
/// [`Composed::Pane`]'s tree — a `Node::Pane` holding a `Node::Buffer` — and
/// `draw` renders it into the rect [`Geometry`] gave it. It used to render
/// `BufferView` straight into that rect and then float a second tree over the
/// result, which is the *"a kind drawn by the interpreter and composed by
/// nobody"* shape `scripts/lint-node-kinds.sh` recorded against `T088`.
///
/// What is left is not a second path: the two strips and the statusline are
/// chrome the host lays out around whatever pane it is holding, and every one
/// of them is already a composed tree. [`Composed::Frame`] is the one branch,
/// and it is a branch about ownership rather than about drawing — a surface
/// that composes its own statusline must not be given a second one.
///
/// The order is `8d`'s — [`FloatSlot::render`] dims what is behind it, so it
/// runs after the pane and over the pane's area only. The statusline never
/// dims: §9's dim means "behind", and chrome is not behind anything.
///
/// **The two strips take rows from the buffer rather than covering it**, which
/// is what `3c` and `8e` draw: the leader grid is a row slot above the
/// statusline and the hint is a one-row strip set off from the code. Neither is
/// a float — a float would impose a border, a header and a footer, and neither
/// drawing has any of the three.
///
/// **It lays nothing out.** The rects arrive as a [`Geometry`] the loop
/// computed, because it had to compute them anyway for the wrap width and the
/// scroll bounds, and two answers to one question is how they came to disagree.
fn draw(
    frame: &mut Frame<'_>,
    editors: &BTreeMap<BufferId, &Editor>,
    theme: &Theme,
    geometry: &Geometry,
    floats: &FloatSlot<'_>,
    composed: &Composed,
    overlay: &Overlay<'_>,
) {
    // **Against the buffer, not against the size the loop measured.** See
    // [`Geometry::clamped_to`]: the rects arrive from a `term.size()` read
    // before `autoresize()` ran, and a shrink between the two makes every rect
    // below an out-of-bounds write. Identity on every pass where the size held.
    let geometry = &geometry.clamped_to(frame.area());

    let painted = Painted {
        editors,
        columns: overlay.columns,
        completion: overlay.completion,
        signature: overlay.signature,
        picker: overlay.picker,
        transcript: overlay.transcript,
        asks: overlay.asks,
    };

    // **§8's degradation, asked for once for the whole tree.** It reached
    // `BufferView` directly while the host drew the widget; the arm that draws
    // a composed `Node::Buffer` takes `Fill::Block` by default, so the
    // collapse had to carry the capability across or put the blocks back —
    // blank cells on a `NO_COLOR` terminal, with nothing on screen to say so.
    // See [`Interpreter::fill`] for why it is a builder and not a prop.
    let interpreter = Interpreter::new(theme, &painted)
        .fill(state_fill(phosphor_term::colour_available()))
        .at(overlay.now);

    let tree = match composed {
        // A surface composed as a whole frame owns it — `6b` draws its own
        // statusline, so the chrome below would be drawing it twice, and its
        // own cursor, so the block at the end would be placing a second one.
        Composed::Frame(tree) => {
            interpreter.render(tree, geometry.frame, frame.buffer_mut());
            return;
        }
        Composed::Pane(tree) => tree,
    };

    // The strips, bottom-up: the leader grid sits directly above the
    // statusline, the hint between it and the code. Both rects were taken off
    // the body by `Geometry::take_strips`; what is left here is what goes in
    // them.
    let hint_row = geometry.hint.zip(overlay.hint);

    // The pane, and whatever float this surface hangs over it — `T021`'s boot
    // report, `T097`'s help page, `T045`'s picker, `T038`'s completion list.
    // One call: the interpreter draws the root and then the float over the
    // same rect, which is where the float has always gone. **Over
    // `geometry.pane` and not `geometry.frame`**, so §9's dim reaches the code
    // and not the statusline under it — panes are what a float is in front of.
    //
    // The state column is empty on the frames where §3's marks have no source;
    // the column is still reserved, which is the half of the 3-column contract
    // that holds with no store behind it.
    interpreter.render(tree, geometry.pane, frame.buffer_mut());
    // `T089`, and the third of §5's *"three strips of chrome, ever"*. Above the
    // pane rather than over it — [`Geometry::take_tab_bar`] took the row from
    // the panes before anything measured one, so this draws into rows nothing
    // else is drawing into. `NoResources` because a tab is a title, a count and
    // a flag: the strip asks the store nothing, since the loop asked for it.
    if let Some(row) = geometry.tabs {
        Interpreter::new(theme, &NoResources).render(overlay.tabs, row, frame.buffer_mut());
    }
    if let Some((row, hint)) = hint_row {
        let strip = Tree::new(unknown_key::strip(
            hint.clone(),
            // The focused buffer's gutter: the strip is one row across the
            // frame and lines up with the text the user is looking at.
            overlay
                .focused_editor(editors)
                .map_or(0, buffer_view::gutter_width),
        ));
        Interpreter::new(theme, &NoResources).render(&strip, row, frame.buffer_mut());
    }
    if let Some(row) = geometry.leader {
        let strip = Tree::new(Node::KeyHints {
            density: Density::Grid,
            hints: overlay.leader.to_vec(),
        });
        Interpreter::new(theme, &NoResources).render(&strip, row, frame.buffer_mut());
    }
    floats.render(geometry.pane, frame.buffer_mut(), theme);
    let chrome = overlay.chrome;
    // `T025`: the statusline is whatever `runtime/statusline.scm` composed, and
    // a layer that composes none draws none. There is deliberately no widget
    // fallback here — a Rust statusline behind a Steel one is the *"config file
    // with a Rust editor hiding behind it"* `CP-2` fails on, and it is what the
    // `CP-2` gate caught by deleting `statusline.scm` and still seeing a line.
    // **§5's field, painted by the caller — which is the contract the
    // interpreter states and nothing was holding up.**
    //
    // `interpret.rs`'s header records the gap: *"A `Node::Line` cannot say what
    // ground it is painted on… This interpreter therefore draws a line
    // transparently, over whatever the caller painted."* Nothing painted. The
    // old `StatusLine` widget filled `chrome.statusline` itself; the composed
    // tree that replaced it (`T025`) had no way to ask for a ground and no one
    // supplied one, so the whole strip — statusline, ex line and notice alike —
    // came out on the terminal's own background.
    //
    // Measured through the pty before it was fixed: the buffer row reported
    // `48;2;12;15;12` and this row reported **no background sequence at all**.
    // On a terminal whose default happens to be near `#0c0f0c` that reads as
    // "the statusline lost its field"; on a light terminal it is a white strip
    // under a dark editor.
    //
    // Painted here rather than given to `Node::Line` as a prop, because the
    // view tree is `spine`'s single writer and a prop is a protocol change —
    // which is the reason `interpret.rs` flagged it instead of patching it. The
    // caller owning the strip's ground is the smaller claim and the true one:
    // §5 says *which* three strips exist, and the binary is what lays them out.
    //
    // Before the branch, so all three chrome states get it. The mode chip
    // paints its own actor-coloured field over the top, which is why filling
    // first cannot flatten it.
    frame
        .buffer_mut()
        .set_style(geometry.status, Style::new().bg(theme.chrome.statusline));

    // **`1c`'s anchored prompt, on its own row.** Drawn here rather than as a
    // branch of the match below, so the statusline underneath takes exactly the
    // path it takes on every other frame — one render, and nothing below has to
    // know a prompt happened.
    let chrome = match (geometry.prompt, chrome) {
        (Some(row), Some(prompt)) if prompt.caret => {
            let line = Tree::new(Node::Prompt {
                prompt: PromptKind::Ex,
                text: prompt
                    .text
                    .strip_prefix(':')
                    .unwrap_or(prompt.text)
                    .to_owned(),
                anchor: prompt.anchor.cloned(),
            });
            Interpreter::new(theme, &NoResources).render(&line, row, frame.buffer_mut());
            None
        }
        (_, chrome) => chrome,
    };

    match chrome {
        // **A prompt is `Node::Prompt` now** (`T058`), which is the
        // demolition `docs/OPEN-QUESTIONS.md` §13 scheduled: this arm built the
        // row from `Node::Line` and `Node::Label` because `phosphor-ui`
        // deferred `prompt`, and said so in a comment naming this task.
        //
        // A **notice** is still a label, and that is not scaffolding — it is a
        // sentence the editor is saying, not a line you are typing into, and
        // `Node::Prompt` would draw it a caret it has no business having.
        //
        // **Both take the statusline's row**, chip and all. `1c` draws the
        // anchored prompt on a row of its own *below* a statusline that is
        // still there, and this build does not — see
        // `docs/OPEN-QUESTIONS.md` §53 for the hang that stopped it, which is
        // recorded with a reproduction rather than worked around. The chip
        // shares the row for now, which is vim's placement and every other
        // screen's.
        Some(chrome) => {
            let row = if chrome.caret {
                Tree::new(Node::Prompt {
                    prompt: PromptKind::Ex,
                    // The text arrives with its `:` already on it, and
                    // `PromptLine` adds the prefix its *kind* implies — so the
                    // one the caller typed is stripped rather than doubled.
                    text: chrome
                        .text
                        .strip_prefix(':')
                        .unwrap_or(chrome.text)
                        .to_owned(),
                    anchor: chrome.anchor.cloned(),
                })
            } else {
                Tree::new(Node::Line {
                    children: vec![Child::new(Node::Label {
                        text: chrome.text.to_owned(),
                        tone: Tone::Text,
                        emphasis: Emphasis::Plain,
                    })],
                })
            };
            Interpreter::new(theme, &NoResources).render(&row, geometry.status, frame.buffer_mut());
        }
        None => {
            if let Some(status) = overlay.status {
                Interpreter::new(theme, &NoResources).render(
                    status,
                    geometry.status,
                    frame.buffer_mut(),
                );
            }
        }
    }

    match chrome.filter(|chrome| chrome.caret) {
        Some(chrome) => {
            let typed = u16::try_from(chrome.text.chars().count()).unwrap_or(u16::MAX);
            // Past the chip, when there is one — the widget draws it first and
            // the terminal's own cursor has to agree with where the text
            // started, or it sits inside the anchor.
            let chip = chrome
                .anchor
                .map_or(0, |anchor| phosphor_ui::prompt::chip_width(anchor) + 1);
            let row = geometry.prompt.unwrap_or(geometry.status);
            let x = row
                .x
                .saturating_add(chip)
                .saturating_add(typed)
                .min(row.right().saturating_sub(1));
            frame.set_cursor_position((x, row.y));
        }
        None => {
            // **The focused pane's rect, not the frame's.** `geometry.pane`
            // is the whole body; with two panes the cursor belongs in the one
            // that has focus, and `Pane::area` is already inset for it.
            if let Some((x, y)) = overlay
                .focused_editor(editors)
                .and_then(|editor| editor.get_visible_cursor(&overlay.focused.1))
            {
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
    /// What the prompt is about, when it is a prompt and something rides along
    /// (`T058`, `1c`).
    ///
    /// **[`None`] for a notice**, which is not a prompt and anchors nothing —
    /// the `caret` flag above already separates the two and this is the second
    /// thing that does.
    anchor: Option<&'a FileSpan>,
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

/// One pane of the split tree: where a buffer is *shown*, as opposed to what
/// it is (`T088`, step 4a).
///
/// **The split is by lifetime, not by tidiness.** Every field here answers a
/// question about a rectangle on screen and keeps its answer when the buffer
/// inside it is swapped for another — which is exactly the set [`Editing`] had
/// to stop owning before a second pane could exist. The reverse test is what
/// decided each field: open a different file in this pane, and if the value
/// should survive, it is a pane's.
///
/// **What is deliberately not here yet, and why.** `holds: PaneKind` and
/// `buffer: Option<BufferId>` belong to a pane and are not in this struct,
/// because nothing reads them until step 4c keys `Panes` and `Buffers` on
/// those ids — and a field with no reader is one `dead_code` rejects, which is
/// this build's usual answer and the right one. `viewport` is the same case
/// one ruling further out: ruling (a) puts each pane's scroll offset here,
/// lent to the widgets through the door `Resources` already is, and the reader
/// arrives with `Resources::viewport` in step 11. The cursor is the larger
/// half of that ruling and is deliberately not assumed to ride along with it.
#[derive(Debug, Clone)]
struct Pane {
    /// What this pane shows (`request::PaneKind`).
    ///
    /// **Step 4a left this out for want of a reader** — a field `dead_code`
    /// rejects is a field this build does not ship — and the `panes` query is
    /// that reader: *"the pane tree, with which one has focus"* has to say what
    /// each one holds, and the tree cannot, because a `PaneTree` knows
    /// arrangement and a `Pane` knows contents.
    ///
    /// One variant is reachable today. `Transcript` is `T054`'s and `Custom` is
    /// v1.5's, and `split-pane` refuses both by naming the task.
    holds: PaneKind,
    /// Which buffer is in it, or [`None`] for a pane holding something that is
    /// not one — the transcript, or a view tree claude emitted.
    ///
    /// **This is the indirection the whole step is for.** A pane names a
    /// buffer; it does not contain one. Swapping a file into a pane is a write
    /// to this field, and the buffer that left is still in [`Buffers`] with its
    /// undo history and its LSP document intact — which is what `:bnext` and a
    /// second split showing the same file both need and neither could have
    /// while `Editing` *was* the pane.
    buffer: Option<BufferId>,
    /// The text area, for scrolls and reveals.
    ///
    /// Moved off `Editing` because it is the definition of a pane: the same
    /// buffer shown in two panes has two areas, and a buffer swapped into this
    /// pane inherits this one.
    area: Rect,
    /// The file that was open before this one — vim's alternate file, what
    /// `CTRL-^` goes back to.
    ///
    /// Set by the loop when a *different* file takes the pane, never by the
    /// arm that reads it: the swap is what creates an alternate, and the one
    /// place that knows a swap happened is the one place that performs it.
    ///
    /// **A pane's, and its own doc always said so** — *"the file leaving the
    /// pane becomes the alternate"*. `CTRL-^` in a split goes back to what that
    /// split was showing, not to what some other split was.
    alternate: Option<PathBuf>,
    /// Where `<C-o>` and `<C-i>` walk (`T042`).
    ///
    /// **Anchors and not positions**, which is why the arm lands with that task
    /// rather than with the motions: a jumplist entry has to survive the
    /// rewrite that moves the code it points at, and surviving a rewrite is the
    /// whole of what an anchor is. The entries are unlabelled, so they never
    /// collide with `m{a-z}`'s marks.
    ///
    /// **A pane's, as it is in vim** (`:help jumplist`: *"Each window has a
    /// separate jump list"*). An anchor carries its own path, so an entry
    /// already knows which file it points into — the list was never per buffer
    /// in anything except where it was stored.
    jumplist: Vec<AnchorId>,
    /// Where in [`Pane::jumplist`] `<C-o>` has walked back to. Pushing a new
    /// jump from here truncates the forward half — a history, not a ring.
    ///
    /// **`jumplist.len()` means *the present*** — not walking, cursor wherever
    /// the last jump left it. That one extra state is what makes `<C-o>` able to
    /// reach the newest entry: it used to be set to `len - 1` by
    /// [`Editing::push_jump`], pointing *at* the entry just recorded, so
    /// `Seek::Prev` computed `0 - 1 = 0`, hit `jump`'s no-move guard and
    /// answered *"already at the oldest jump"*. After a single jump you could
    /// never get back — which is the rule `push_jump`'s own doc states, and
    /// nothing pressed `<C-o>` until the key survey that found this.
    jump_at: usize,
}

/// What the session owns, as opposed to what a buffer or a pane does
/// (`T088`, step 4b).
///
/// **One of each, however many buffers there are.** Both fields below were on
/// [`Editing`], where each new buffer would have taken its own clone of a
/// handle to the same object — not wrong, since both are shared handles, but a
/// clone per buffer is a thing a constructor can forget to make, and step 8
/// builds `Editing`s from a place that has no business knowing about either.
/// Hung off the context instead, where an arm reaches them and a new buffer
/// cannot be born without one.
struct Shell {
    /// `T041`'s store, shared with [`AppHost`] so the gutter, the statusline
    /// and the `region` queries cannot disagree about a file.
    store: Arc<store::Shared>,
    /// `T059`'s questions, oldest first, by id.
    ///
    /// **A map on `Shell` rather than widget state**, which is `T060`'s rule
    /// arriving one task early and for a reason that is already true: `4a`'s
    /// float, the statusline's `!`, and whatever answers a digit all have to
    /// agree about what is being asked, and three readers of one map cannot
    /// disagree. What `T060` adds is the *queue* — waiting behind a float that
    /// has focus, `]!`, and the store query — not the storage.
    asks: BTreeMap<AskId, phosphor_ui::question::QuestionVm>,
    /// The ask id counter, shared with [`AppHost::next_ask`] — see there.
    next_ask: Arc<Mutex<u64>>,
    /// Actions waiting on an answer, by the ask that is asking (`T060`).
    ///
    /// **This is what makes the queue a *mechanism* rather than a screen.**
    /// `McpPolicy::Ask` is a rating on a capability — *"only the keyboard says
    /// yes to this"* — and until there was somewhere to put the question the
    /// only honest thing a producer could be told was
    /// *"needs an ask first — T060 builds the queue"*. Now an `Ask`-rated
    /// action arriving from a producer becomes a question, and answering it
    /// runs the action it was asking about.
    ///
    /// Keyed by `AskId` rather than a single slot, because the queue is a queue:
    /// two servers can each want a rename while you are reading something else.
    held: BTreeMap<AskId, Box<Action>>,
    /// Actions whose question you said yes to, waiting for the loop to run
    /// them (`T060`).
    ///
    /// A field rather than a return value, because `Shell::answer_ask` is
    /// called from an *arm* — which cannot reach `Buffers` — and the loop is
    /// what performs an action across buffers.
    granted: Vec<Action>,
    /// Edits to apply across files, from `apply-workspace-edit` (`T060`).
    ///
    /// **The arm cannot do this itself and that is structural**: an `Editing`
    /// holds *one* rope, and a rename is edits in several. So the arm records
    /// and the loop performs, which is the shape `Editing::open` and
    /// `Shell::closing` already have.
    edits: Vec<phosphor_core::request::FileEdits>,
    /// The verb each permission ask is about, by ask id (`T061`).
    ///
    /// **Beside `Shell::held` rather than inside it**, because the two hold
    /// different kinds of thing: `held` is an *action* the editor will run, and
    /// this is a *rule* it may write. `7a`'s `[2]` does both and `[1]` does one
    /// of them, which is exactly why they are not one field.
    asking_about: BTreeMap<AskId, String>,
    /// Rules an always-allow has agreed to write (`T061`).
    ///
    /// A queue for the same reason `Shell::granted` is one: the arm that agrees
    /// cannot reach the layer, and writing a form is running scheme.
    writing: Vec<String>,
    /// The allow-list, as `runtime/permissions.scm` last published it
    /// (`T061`).
    ///
    /// Read from the option each pass, exactly as `agent-command` is, and for
    /// the same reason: a grant made at the REPL has to reach the next
    /// permission ask, and a value cached at boot would make the rules a fact
    /// about the last restart.
    allowed: Option<String>,
    /// A correction to send when the paused turn resumes (`7e`, `T062`).
    ///
    /// Held rather than sent by the arm, because sending is the session's and
    /// the arm has no session handle — the same seam `Shell::wanted` and
    /// `Shell::writing` sit on.
    steering: Option<String>,
    /// Whether `esc` has asked the turn to stop at the next tool boundary
    /// (`7e`, `T062`).
    ///
    /// **A request, not a state.** *"`esc` pauses at the next tool boundary"* is
    /// the screen's own caption, and the gap between asking and stopping is the
    /// feature: an interrupt that took effect *now* would land in the middle of
    /// whatever the agent was doing, which is the thing a tool boundary exists
    /// to avoid. [`Shell::paused`] is the state this becomes.
    pausing: bool,
    /// The turn that stopped at a boundary, and the call it stopped before.
    ///
    /// The pair, because they arrive together and mean nothing apart: a paused
    /// turn with no held call has not reached a boundary, and a held call
    /// belonging to no turn is a call nothing can resume.
    paused: Option<(TurnId, phosphor_ui::transcript::ToolCall)>,
    /// The asks you have pushed back — `4a`'s `esc later` (`T060`).
    ///
    /// **A set beside the queue rather than a flag inside it**, because Q9's
    /// rule is about *the screen* and not about the question: a deferred ask is
    /// still pending, still counts toward the statusline's `!`, and still
    /// answers `pending-asks`. What it has stopped doing is asking for the
    /// screen back. `]!` is what clears it.
    ///
    /// Without this the queue cannot converge: `esc` closes the float, the next
    /// pass finds the same head still pending, and raises it again.
    deferred: BTreeSet<AskId>,
    /// Which ask the float **currently on screen** is showing.
    ///
    /// **Two fields rather than one, and the pair is what raises the float.**
    /// `asking` is what should be on screen and this is what is; the loop
    /// compares them once per pass and opens or closes accordingly. Both
    /// appliers therefore only have to say *what should be asked* — neither
    /// composes a float, which neither is in a position to do.
    asked: Option<AskId>,
    /// Where the editor was started — what a session is rooted at, and what a
    /// tool row's jump link resolves against (`T056`).
    ///
    /// **Held rather than asked for.** `getcwd` per arm is a question whose
    /// answer cannot change inside this process, and an arm that asked it would
    /// be a second definition of *the workspace* to drift from the loop's.
    workspace: PathBuf,
    /// How a picker's matcher says it has results this frame did not show.
    ///
    /// Held by the session because a picker is opened from three places — the
    /// loop's source-cycling and two capability arms — and nucleo takes its
    /// notify at construction. One handed down is the only version of this that
    /// cannot be forgotten at one of the three.
    wake: picker::Wake,
    /// The unnamed register is `"`; `"a` is `a` (`request::RegisterName`).
    registers: BTreeMap<String, Register>,
    /// The open picker, or [`None`] (`T045`).
    ///
    /// On `Editing` rather than beside the other surfaces because the matcher
    /// is per-session state that outlives a frame and a `Node` does not — the
    /// same reason `CompletionVm` lives here. `open-picker` fills it,
    /// `set-picker-query` and `toggle-picker-preview` act on it, and `esc`
    /// drops it.
    picker: Option<PickerSession>,
    /// The order `cycle-picker-source` walks, refreshed by the loop from the
    /// layer's `phosphor/picker-sources` (`T047`).
    ///
    /// Cached on this side because the arm runs during event handling and
    /// `Layer` is `&mut` there — the same reason `Editing` holds a keymap
    /// snapshot rather than asking the VM per keystroke.
    source_order: Vec<String>,
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
    /// Set by `App::Quit`; the loop reads it once per turn.
    quit: bool,
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
    /// `:wall` asked for every dirty buffer to be written. Drained by the loop
    /// (`T088`, step 8).
    ///
    /// **An intent rather than the arm doing it**, for the reason [`Intent`]
    /// exists: an arm holds one buffer and this is a question about all of
    /// them. `Editing::write` is still the one thing that writes — the loop
    /// calls it per buffer rather than a second implementation existing.
    wall: bool,
    /// Whether the `:quit` that set [`Shell::quit`] was forced.
    ///
    /// **The arm's refusal and the loop's are two checks, and only one of them
    /// can see the `force`.** `App::Quit`'s arm answers `WouldLoseWork` for the
    /// buffer it holds; the loop answers for every *other* buffer, which the
    /// arm cannot see. Without this, the loop's half would refuse a `ZQ` the
    /// arm had already been told to discard — and `ZQ` at a scratch buffer is
    /// the one exit a bare `phosphor` has, because there is no file to
    /// `:write` to. An editor you cannot leave is the worst version of this
    /// feature, which is what `a_bare_phosphor_with_unsaved_work_is_still_quittable`
    /// exists to say, and it is what caught this.
    discard: bool,
    /// A file `accept-picker` asked to open in a **new split**, and which way
    /// (`T088`, step 12).
    ///
    /// Drained by the loop, because it is three things an arm cannot do at
    /// once: split the tree, open a *new* buffer, and point the new pane at it.
    /// The middle one is the wall — `Editing::act` holds `&mut self`, which is
    /// an entry in `Buffers`, so minting a sibling out of the same map is an
    /// aliasing error rather than a missing call.
    splitting: Option<(PathBuf, Direction)>,
    /// A buffer `:close-buffer` asked to close, drained the same way.
    ///
    /// The arm answers what it can see — a dirty buffer refuses
    /// `WouldLoseWork` without a `force`, exactly as `:quit` does — and the
    /// loop answers what only it can: whether there is another buffer for the
    /// pane to show afterwards.
    closing: Option<BufferId>,
    /// The ACP session (`T050`).
    ///
    /// **On the shell rather than beside the language servers**, and the
    /// difference is who reaches it: a server is spoken to by the loop alone,
    /// while a session is spoken to by an *arm* — `send-message` prompts it —
    /// and [`Cx`] is the only way an arm reaches anything that is not its own
    /// rope.
    session: phosphor_agent::session::Session,
    /// The turn in flight and when it began, or [`None`] between turns.
    ///
    /// Two readers, and they are the two halves of §5's session segment: which
    /// [`SessionState`] to draw, and where the elapsed counter counts from.
    /// Written by the `turn-began` and `turn-ended` arms and by nothing else,
    /// so *"claude is working"* is the client's report rather than the
    /// editor's guess.
    turn: Option<(TurnId, Instant)>,
    /// What the prompt line has been asked to do, drained by the loop
    /// (`T058`).
    ///
    /// **The four surface verbs are `Deny` on every door**, so nothing but a
    /// key reaches them — and a key is handled in the loop, which is where
    /// `ex_line` lives. An arm cannot touch that local, so it posts here and
    /// the loop performs it, the same shape `Intent` has for the VM.
    prompt_step: Option<PromptStep>,
    /// Ex history, oldest first — `6d`'s `q:`, and `prompt-history` walks it.
    ///
    /// **One list for both kinds.** `prompt-history`'s own row says so:
    /// *"prompts to claude are ex history too"*. What you typed is what you
    /// want back, and which line it went down is not how anyone remembers it.
    history: Vec<String>,
    /// How far back `prompt-history` has walked, or [`None`] at the live line.
    recalled: Option<usize>,
    /// A sentence a door asked the editor to say, waiting for a frame that
    /// has somewhere to put it (`T053`).
    ///
    /// See `Intent::Say` for why it waits: `6b`'s REPL owns its whole frame, so
    /// a notice set while it is open would be drawn to nobody.
    saying: Option<String>,
    /// Everything the session has said (`T054`).
    ///
    /// On the shell for [`Shell::session`]'s reason: the arms that write it
    /// reach it through [`Cx`], and a producer's `session-prose` is an Action
    /// like any other.
    transcript: Transcript,
    /// The transcript revision the host was last told about (`T054`).
    told: u64,
    /// Which turns are collapsed, for `Node::Transcript`'s `folded` prop.
    folded: Vec<TurnId>,
    /// The session's state as of the last frame (`T051`).
    ///
    /// **Kept so a transition can be told from a state.** §5 wants the
    /// statusline truthful, which the state alone gives; §6 wants the editor to
    /// *say* things — *"session lost — :reattach"* is written as an event, not
    /// as a status — and an event is a difference between two frames. One
    /// `PartialEq` is the whole mechanism.
    life: phosphor_agent::session::Life,
    /// The `agent-command` option as of the last frame (`T057`).
    ///
    /// Distinct from [`Shell::agent`], which is *what is attached* — a verb can
    /// set that without the option moving, and the option can be cleared while
    /// a verb-started session runs. One field could not tell those apart.
    wanted: Option<String>,
    /// Whether `7d`'s one hint line has been dismissed (`T057`).
    ///
    /// *"three verbs, then out of the way"* is the screen's own caption, and
    /// this is the "then": `dismiss-dashboard-hint` sets it and the row stops
    /// being drawn. Per session rather than persisted — a fresh editor is a
    /// fresh cold start, and `T101`'s config home is where a *preference* would
    /// go if this ever became one.
    hinted: bool,
    /// The `agent-command` the session is currently attached to.
    ///
    /// Kept so the loop can tell *"the option changed"* from *"the option is
    /// set"*. Without it, reading the option per frame would respawn the agent
    /// per frame — which is the shape `soft-wrap` gets away with because
    /// honouring it is free and spawning a process is not.
    agent: Option<String>,
}

impl Shell {
    /// The next ask id (`T059`). Monotonic and never reused: an answered ask's
    /// id coming back would address a different question.
    fn mint_ask(&self) -> AskId {
        let mut next = self
            .next_ask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.saturating_add(1);
        AskId(id)
    }

    /// Queue a question and make it the one that should be on screen.
    ///
    /// **The one write, called from both appliers.** `enqueue-ask` is armed in
    /// `Editing::act` for the keystroke path and in `AppHost::apply` for the
    /// doors; a second copy of these three lines is a second set of rules about
    /// what a new question does to the screen.
    fn enqueue_ask(&mut self, id: AskId, question: phosphor_ui::question::QuestionVm) {
        self.asks.insert(id, question);
    }

    /// Push a question back — `4a`'s `esc later` (`T060`).
    ///
    /// Answers [`false`] for an id the queue does not have, which is the
    /// ordinary case for a float deferred after it was already answered.
    fn defer_ask(&mut self, id: AskId) -> bool {
        if !self.asks.contains_key(&id) {
            return false;
        }
        self.deferred.insert(id);
        true
    }

    /// Un-defer the oldest pushed-back question and hand back its id — `]!`
    /// (`T060`).
    ///
    /// **`]!` is a *motion* and this is what makes it one.** Q9 says it *jumps
    /// to* the pending ask, and the thing standing between you and a deferred
    /// question is the deferral: clearing it is what puts the question back in
    /// front of you, and the loop raises it on the same pass by the ordinary
    /// rule. Nothing here opens a float.
    ///
    /// **The oldest deferred one, not the newest**, so `]!` walks the pushed-back
    /// questions in the order you pushed them back rather than handing you the
    /// same one twice.
    fn recall_ask(&mut self) -> Option<AskId> {
        let recalled = self
            .asks
            .keys()
            .find(|id| self.deferred.contains(id))
            .copied()?;
        self.deferred.remove(&recalled);
        Some(recalled)
    }

    /// The question that wants the screen: the oldest one you have not pushed
    /// back (`T060`).
    ///
    /// **Derived, never stored**, which is what makes Q9's *"the queue is a
    /// store query, not widget state, so `]!`, the inbox and the statusline
    /// read one truth"* structural rather than remembered. `T059` had a
    /// `Shell::asking` field beside the map; two things that must agree are one
    /// thing that can disagree, and the map is the one that has to be right.
    ///
    /// Oldest first because ids are minted in order and a `BTreeMap` is
    /// ordered: a queue that surfaced the newest would leave the first question
    /// you were asked for last.
    fn head_ask(&self) -> Option<AskId> {
        self.asks
            .keys()
            .find(|id| !self.deferred.contains(id))
            .copied()
    }

    /// Answer a question and take it out of the queue.
    ///
    /// **Removed rather than marked**, because §5's `!`, `4a`'s float and
    /// `T060`'s `]!` all read this one map: an ask that stayed in it with a
    /// flag would be three readers agreeing about a question nobody is being
    /// asked. Answers [`false`] for an id the queue does not have, which is the
    /// ordinary case for a float answered twice.
    fn answer_ask(&mut self, id: AskId, digit: Option<u32>, prose: Option<&str>) -> bool {
        self.deferred.remove(&id);
        if self.asks.remove(&id).is_none() {
            self.held.remove(&id);
            return false;
        }
        // **The action the question was about, released or dropped.** `[1]` is
        // yes by construction — `held_question` builds the options — and
        // anything else is a no, which is the safe reading for a rating whose
        // whole point is that a producer may not do this unasked.
        if let Some(action) = self.held.remove(&id)
            && digit == Some(1)
        {
            self.granted.push(*action);
        }
        // **The sentence goes through `Shell::saying`, which is `T053`'s
        // channel and exists for exactly this shape.** A `Receipt::note`
        // reaches whoever *called*, and the caller here is a digit — there is
        // nobody on that side. The notice row is where §6 puts a sentence for
        // the person at the terminal.
        //
        // **The answer itself goes nowhere yet, and this says what happened
        // rather than pretending.** Getting it back to the agent is a wire —
        // ACP's response to whatever asked — and the thing that asks is
        // `T060`'s queue and `T061`'s permission flow.
        self.saying = Some(match (digit, prose) {
            (Some(digit), _) => format!("answered {digit}"),
            (None, Some(said)) => format!("answered — {said}"),
            // Unreachable: both appliers refuse an answer with neither.
            (None, None) => "answered".to_owned(),
        });
        true
    }
}

impl std::fmt::Debug for Shell {
    /// [`picker::Wake`] is an `Arc<dyn Fn()>` and implements no `Debug`; what
    /// is worth printing is the store's shape, which is the half a failing
    /// assertion is ever about.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("store", &self.store)
            .field("registers", &self.registers)
            .field("mode", &self.mode)
            .field("quit", &self.quit)
            .finish_non_exhaustive()
    }
}

/// What an Action being applied can reach besides the buffer it is applied to.
///
/// **This exists because a mutation needs more than one thing and [`Editing`]
/// can only be one of them.** `Editing::act` takes `&mut self` — the buffer —
/// and an arm that scrolls needs the pane, while one that declares a region
/// needs the store. Step 4a threaded the pane as a bare parameter, which was
/// right while there was one thing to thread; a second is what makes the struct
/// pay for itself, and step 4c's `tree` lands here without touching a call site
/// again.
///
/// It is a context, **not a door**. `phosphor-ui` never sees one: the widgets
/// read through `Resources`, which has no `&mut` in it and must never grow one.
#[derive(Debug)]
struct Cx<'a> {
    /// Which buffer the Action is being applied to.
    ///
    /// **Carried so an arm can refuse a selector it cannot honour.** Routing
    /// happens at the door — the loop reads [`Buffers::named`] and hands the
    /// Action to the buffer it names — but not every door can route: an ex line
    /// runs its Actions against the buffer the ex line was typed in, and a
    /// Steel command could name any id it likes. Where routing is impossible,
    /// refusing is the honest answer, and this is what the refusal compares
    /// against.
    buffer: BufferId,
    /// Which pane the Action lands in.
    ///
    /// **An id rather than the pane itself**, because an arm that resolves a
    /// [`PaneRef`] has to be able to name a pane that is not this one —
    /// `scroll` carries one, and a reveal in an unfocused pane must not move
    /// the focused pane's viewport. [`Cx::view`] is the lookup.
    pane: PaneId,
    /// Every pane, so a resolved reference can reach one that is not the
    /// Action's own.
    panes: &'a mut Panes,
    /// What the session owns.
    shell: &'a mut Shell,
}

/// Every open buffer, by id (`T088`, step 4c).
///
/// **Keyed from the first line, and never indexed by position.** There is one
/// entry today, and a map is the wrong shape for one entry — which is the
/// point. A `Vec` with a `usize` cursor is the shape that works right up until
/// `close-buffer` runs once, and then every index held anywhere else is off by
/// one, silently and with no type error. `BufferId` is already in the
/// vocabulary and already means *"not a path: the same file can be open once
/// and renamed"*, so paying for the id now costs one `BTreeMap` and buys the
/// class of bug that would otherwise be found by a user.
struct Buffers {
    map: BTreeMap<BufferId, Editing>,
    /// The next id to hand out.
    ///
    /// **Ids are never reused.** A closed buffer's id stays closed, so a stale
    /// `BufferId` — one an agent held across a `close-buffer` — refuses rather
    /// than resolving to whatever took its place. That is the same rule
    /// `store::Shared` already keeps for `RegionId`, and for the same reason:
    /// silently answering about the wrong thing is worse than refusing.
    next: u64,
}

impl Buffers {
    /// The buffer a session starts with, and its id.
    fn new(first: Editing) -> (Self, BufferId) {
        let mut buffers = Self {
            map: BTreeMap::new(),
            next: 0,
        };
        let id = buffers.open(first);
        (buffers, id)
    }

    /// Which buffer an Action names, or the focused one when it names none.
    ///
    /// **Four capabilities carry an `Option<BufferId>` and the applier dropped
    /// all four with `..`**: `set-cursor`, and the three `ingest-*` answers.
    /// The doc on each says *"absent means the focused one"*, which the applier
    /// honoured by accident — it read the focused buffer because it had no
    /// other, so `Some(anything)` and `None` did the same thing. An agent
    /// naming a buffer it once knew about got the *focused* buffer's cursor
    /// moved, which is the failure mode `Refusal::NoSuchTarget` exists for:
    /// *"a stale id from an agent working off an old query"*.
    ///
    /// This is a total function over the four, not over the vocabulary. A fifth
    /// capability growing a `buffer` argument is a line here, and the arm that
    /// forgets it goes on discarding — which is why step 9 keys `Outstanding`
    /// rather than leaving the tagging to whoever posts.
    fn named(action: &Action, focus: BufferId) -> BufferId {
        let named = match action {
            Action::Motion(MotionAction::SetCursor { buffer, .. })
            | Action::Lsp(LspAction::IngestCompletions { buffer, .. })
            | Action::Lsp(LspAction::IngestSignatureHelp { buffer, .. })
            | Action::Lsp(LspAction::IngestHover { buffer, .. }) => *buffer,
            _ => None,
        };
        named.unwrap_or(focus)
    }

    /// The buffer `id` names, or [`None`] if it names none.
    ///
    /// The fallible half of [`Buffers::at_mut`], for the callers that have been
    /// handed an id by somebody else and must answer for it rather than trust
    /// it.
    fn get_mut(&mut self, id: BufferId) -> Option<&mut Editing> {
        self.map.get_mut(&id)
    }

    /// The buffer `id` names, read-only — [`Buffers::at_mut`]'s other half,
    /// for a caller holding an immutable borrow of the map alongside it.
    fn at(&self, id: BufferId) -> &Editing {
        self.map.get(&id).expect("a BufferId names an open buffer")
    }

    /// Takes a buffer and answers the id it was given.
    ///
    /// **The one place a `BufferId` is minted**, and it comes off the counter
    /// rather than off `map.len()`. Those two agree exactly until the first
    /// `close-buffer`, after which `len()` starts handing out an id that is
    /// already taken — the failure being designed against, written as the
    /// obvious line somebody reaches for.
    fn open(&mut self, editing: Editing) -> BufferId {
        let id = BufferId(self.next);
        self.next += 1;
        self.map.insert(id, editing);
        id
    }

    /// The buffer `id` names.
    ///
    /// Panics if it names none, which is a bug in whoever held the id rather
    /// than a state a user can reach: nothing removes an entry until step 10's
    /// `close-buffer`, and that step is where a caller starts having to say
    /// what a stale id means.
    fn at_mut(&mut self, id: BufferId) -> &mut Editing {
        self.map
            .get_mut(&id)
            .expect("a BufferId names an open buffer")
    }
}

/// The split tree — which panes there are and how they divide the frame
/// (`T088`, step 10).
///
/// **A pure data structure, deliberately.** No terminal, no `Editor`, no theme
/// and no `Rect`: every operation here is about *arrangement*, and turning an
/// arrangement into rectangles is step 11's separate job. That is what lets
/// `T088`'s acceptance criterion — split, focus, close and resize behaving like
/// vim's windows — be proven before a pixel exists.
///
/// It is split from `Panes`' `BTreeMap<PaneId, Pane>` on purpose, so
/// `&PaneTree` and `&mut Pane` can be borrowed at once. A tree that owned the
/// panes would make *"resolve a direction, then write to the pane it names"*
/// two borrows of one thing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PaneTree {
    /// One pane.
    Leaf(PaneId),
    /// Two subtrees, and how the space between them divides.
    Split {
        /// Whether the children sit side by side or one above the other.
        axis: Axis,
        /// The left or upper child.
        first: Box<PaneTree>,
        /// The right or lower child.
        second: Box<PaneTree>,
        /// What share of the space `first` takes, in percent.
        ///
        /// **Percent rather than cells, and an integer rather than a float.**
        /// A tree that stored cells would be wrong the moment the terminal is
        /// resized, and this structure is deliberately the one that does not
        /// know how big anything is. An integer because a ratio that a test
        /// cannot compare exactly is a ratio a test cannot assert on.
        first_share: u16,
    },
}

/// Which way a [`PaneTree::Split`] divides its space.
///
/// Not `Direction`: a direction has four values and an axis has two, and
/// `Up`/`Down` describe the same division. The conversion is
/// [`Axis::of`], and it is the one place the four collapse into the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// Children side by side; a vertical line between them.
    Columns,
    /// Children stacked; a horizontal line between them.
    Rows,
}

impl Axis {
    /// The axis a split in this direction divides along.
    const fn of(direction: Direction) -> Self {
        match direction {
            Direction::Left | Direction::Right => Self::Columns,
            Direction::Up | Direction::Down => Self::Rows,
        }
    }
}

/// Cuts `area` in two along `axis`, giving the near side `share` percent.
///
/// The far side takes the remainder rather than computing its own share, which
/// is what makes the two tile exactly at any width.
fn divide(area: Rect, axis: Axis, share: u16) -> (Rect, Rect) {
    let total = match axis {
        Axis::Columns => area.width,
        Axis::Rows => area.height,
    };
    let near = u16::try_from(u32::from(total) * u32::from(share) / 100).unwrap_or(total);
    let far = total.saturating_sub(near);
    match axis {
        Axis::Columns => (
            Rect {
                width: near,
                ..area
            },
            Rect {
                x: area.x.saturating_add(near),
                width: far,
                ..area
            },
        ),
        Axis::Rows => (
            Rect {
                height: near,
                ..area
            },
            Rect {
                y: area.y.saturating_add(near),
                height: far,
                ..area
            },
        ),
    }
}

/// An id as the wire spells one: a non-negative integer.
///
/// The same conversion `phosphor_core::request`'s `ids!` macro writes, and the
/// same saturation — an id that will not fit an `i64` is a session with more
/// than nine quintillion panes in it, and clamping is a better answer there
/// than a panic.
fn numbered(id: u64) -> Value {
    Value::Int(i64::try_from(id).unwrap_or(i64::MAX))
}

/// An even split, which is what every new one is.
const EVEN: u16 = 50;

/// The narrowest share a split may be squeezed to, in percent.
///
/// **A resize that can reach zero can make a pane unreachable**: it would still
/// be in the tree, still be focusable, and draw nothing — a state a user can
/// get into with one keystroke and cannot get out of by looking at the screen.
/// vim clamps for the same reason.
const LEAST: u16 = 10;

impl PaneTree {
    /// The panes, left to right and top to bottom.
    ///
    /// **This is cycle order**, and it is the tree's rather than the id map's:
    /// `<C-w>w` walks the windows as they are arranged, not as they were
    /// opened. `Panes::resolve` used the map's key order until this existed,
    /// which is the same answer for one pane and diverges at two.
    fn leaves(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf(id) => vec![*id],
            Self::Split { first, second, .. } => {
                let mut found = first.leaves();
                found.extend(second.leaves());
                found
            }
        }
    }

    /// This tree as plain data, for the `panes` query.
    ///
    /// A leaf is `{"pane": <id>}` and a split is `{"axis", "share", "first",
    /// "second"}`. **Plain data and not a `Node`**: this answers *what the
    /// arrangement is*, and a view tree would answer *how to draw one* — the
    /// query's own row says *"the pane tree, with which one has focus"*, and
    /// the shape is the tree's rather than a picture of it.
    fn describe(&self) -> Value {
        match self {
            Self::Leaf(id) => Value::Record(Args::new().with("pane", numbered(id.0))),
            Self::Split {
                axis,
                first,
                second,
                first_share,
            } => Value::Record(
                Args::new()
                    .with(
                        "axis",
                        Value::Text(
                            match axis {
                                Axis::Columns => "columns",
                                Axis::Rows => "rows",
                            }
                            .to_owned(),
                        ),
                    )
                    .with("share", Value::Int(i64::from(*first_share)))
                    .with("first", first.describe())
                    .with("second", second.describe()),
            ),
        }
    }

    /// Where each pane goes, given the space they all share.
    ///
    /// **The one place this structure meets a rectangle**, and it takes one
    /// rather than storing one: the tree is about arrangement, and a tree that
    /// remembered cells would be wrong the moment the terminal resized. Every
    /// other method here is testable with no geometry at all because of that,
    /// and this one is testable with nothing but geometry.
    ///
    /// **The halves tile exactly** — the second takes what the first left,
    /// rather than both rounding the share independently. Two panes that each
    /// computed `width * share / 100` would leave a one-column gap at odd
    /// widths, and a gap is a column nothing owns and nothing clears.
    ///
    /// There is no separator column. A divider between panes is a drawing
    /// decision and Design Language's to make; this answers where the panes
    /// *are*, and inventing a gutter here would put that decision in the one
    /// place that cannot see a theme.
    fn layout(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        match self {
            Self::Leaf(id) => vec![(*id, area)],
            Self::Split {
                axis,
                first,
                second,
                first_share,
            } => {
                let (near, far) = divide(area, *axis, *first_share);
                let mut placed = first.layout(near);
                placed.extend(second.layout(far));
                placed
            }
        }
    }

    /// Puts `new` beside `at`, splitting the space they share.
    ///
    /// Answers whether `at` was found. `direction` decides both the axis and
    /// **which side the new pane lands on** — `:vsplit` in vim puts the new
    /// window left, `:split` puts it above, and a `Right`/`Down` split is the
    /// mirror. Getting that backwards is not a crash; it is an editor whose
    /// splits open on the wrong side, which is exactly the kind of thing a data
    /// structure with no rectangles can be made to prove.
    fn split(&mut self, at: PaneId, new: PaneId, direction: Direction) -> bool {
        match self {
            Self::Leaf(id) if *id == at => {
                let existing = Self::Leaf(*id);
                let fresh = Self::Leaf(new);
                let (first, second) = match direction {
                    Direction::Left | Direction::Up => (fresh, existing),
                    Direction::Right | Direction::Down => (existing, fresh),
                };
                *self = Self::Split {
                    axis: Axis::of(direction),
                    first: Box::new(first),
                    second: Box::new(second),
                    first_share: EVEN,
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split(at, new, direction) || second.split(at, new, direction)
            }
        }
    }

    /// Removes `at`, collapsing the split it was half of into its sibling.
    ///
    /// Answers whether it was removed. **The last pane cannot be closed** — a
    /// tree with no leaves is not a state this structure can represent, and it
    /// is not one an editor should reach either: closing the only window is
    /// what `:quit` means, and that is a different verb.
    fn close(&mut self, at: PaneId) -> bool {
        let kept = match self {
            // The whole tree is one pane; there is nothing to collapse into.
            Self::Leaf(_) => return false,
            Self::Split { first, second, .. } => {
                if matches!(&**first, Self::Leaf(id) if *id == at) {
                    Some((**second).clone())
                } else if matches!(&**second, Self::Leaf(id) if *id == at) {
                    Some((**first).clone())
                } else {
                    None
                }
            }
        };
        if let Some(kept) = kept {
            *self = kept;
            return true;
        }
        match self {
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => first.close(at) || second.close(at),
        }
    }

    /// Moves the divider `at` sits against, in percentage points.
    ///
    /// Answers whether anything moved. Positive grows the pane; the clamp is
    /// [`LEAST`] at both ends, so a divider cannot be pushed far enough to make
    /// either side unreachable.
    fn resize(&mut self, at: PaneId, delta: i16) -> bool {
        let Self::Split {
            first,
            second,
            first_share,
            ..
        } = self
        else {
            return false;
        };
        // Deeper first: the divider a pane sits *against* is the innermost one
        // that has it on a side, which is what makes a resize in a nested split
        // move the wall next to it rather than the outer frame.
        if first.resize(at, delta) || second.resize(at, delta) {
            return true;
        }
        let step = if first.leaves().contains(&at) {
            delta
        } else if second.leaves().contains(&at) {
            -delta
        } else {
            return false;
        };
        let moved = i32::from(*first_share) + i32::from(step);
        let clamped = moved.clamp(i32::from(LEAST), i32::from(100 - LEAST));
        let clamped = u16::try_from(clamped).unwrap_or(EVEN);
        if clamped == *first_share {
            return false;
        }
        *first_share = clamped;
        true
    }

    /// The pane in a compass direction from `from`, or [`None`] at the edge.
    ///
    /// Walks up to the nearest ancestor that divides along the matching axis
    /// and takes the neighbouring subtree — which is what makes `<C-w>l` in a
    /// nested layout land in the pane actually to the right, rather than in
    /// whichever one happens to be next in some list.
    fn toward(&self, from: PaneId, direction: Direction) -> Option<PaneId> {
        let axis = Axis::of(direction);
        let forward = matches!(direction, Direction::Right | Direction::Down);
        let Self::Split {
            axis: mine,
            first,
            second,
            ..
        } = self
        else {
            return None;
        };
        // Deeper first, so the *nearest* ancestor answers.
        if let Some(found) = first.toward(from, direction) {
            return Some(found);
        }
        if let Some(found) = second.toward(from, direction) {
            return Some(found);
        }
        if *mine != axis {
            return None;
        }
        let (here, there) = if forward {
            (&**first, &**second)
        } else {
            (&**second, &**first)
        };
        if !here.leaves().contains(&from) {
            return None;
        }
        // **The nearest leaf, not the first one.** Going right lands on the
        // left edge of the subtree to the right; going left lands on its
        // *right* edge. Taking `first()` both ways is the kind of thing two
        // panes cannot tell you is wrong and three can.
        let neighbours = there.leaves();
        if forward {
            neighbours.first().copied()
        } else {
            neighbours.last().copied()
        }
    }
}

/// Every pane, by id, and which one has focus (`T088`, step 4c).
///
/// The [`PaneTree`] is beside the map rather than owning it, so `&PaneTree` and
/// `&mut Pane` can be borrowed at once — *"resolve a direction, then write to
/// the pane it names"* would otherwise be two borrows of one thing.
#[derive(Debug)]
struct Panes {
    map: BTreeMap<PaneId, Pane>,
    /// How the panes divide the frame (`T088`, step 10).
    tree: PaneTree,
    /// Which pane a `PaneRef::Focused` means.
    focus: PaneId,
    /// The next id to hand out, on [`Buffers::next`]'s rule.
    next: u64,
}

impl Panes {
    /// The pane a session starts in, and its id.
    fn new(first: Pane) -> (Self, PaneId) {
        let mut panes = Self {
            map: BTreeMap::new(),
            tree: PaneTree::Leaf(PaneId(0)),
            focus: PaneId(0),
            next: 0,
        };
        let id = panes.mint(first);
        panes.tree = PaneTree::Leaf(id);
        panes.focus = id;
        (panes, id)
    }

    /// Takes a pane into the map and answers the id it was given, on
    /// [`Buffers::open`]'s rule.
    ///
    /// **Private, and it does not touch the tree.** A pane in the map and not
    /// in the tree is a pane nothing can reach and nothing will draw, so the
    /// two go together — [`Panes::split`] is the way in, and this is the half
    /// of it that hands out an id.
    fn mint(&mut self, pane: Pane) -> PaneId {
        let id = PaneId(self.next);
        self.next += 1;
        self.map.insert(id, pane);
        id
    }

    /// Puts a new pane beside `at`, and answers its id.
    ///
    /// [`None`] if `at` names no pane. **Focus does not move** — opening a pane
    /// and looking at it are two things, and vim's `:split` moves focus while
    /// `:sbuffer` does not; which of the two a keystroke means is the keymap's
    /// to say, so this does the half that is not in dispute.
    fn split(&mut self, at: PaneId, pane: Pane, direction: Direction) -> Option<PaneId> {
        if !self.map.contains_key(&at) {
            return None;
        }
        let id = self.mint(pane);
        if self.tree.split(at, id, direction) {
            Some(id)
        } else {
            // The tree and the map disagreed, which is this struct failing to
            // keep its own invariant. Undo the mint rather than leave a pane
            // nothing can reach.
            self.map.remove(&id);
            None
        }
    }

    /// Closes `at`, and answers whether it went.
    ///
    /// **Focus moves if it has to**, to the pane the tree puts first after the
    /// collapse — leaving `focus` pointed at a closed pane is the one way this
    /// struct can break the invariant every `at`/`at_mut` depends on.
    /// Moves the divider  sits against, in percentage points.
    fn resize(&mut self, at: PaneId, delta: i16) -> bool {
        self.tree.resize(at, delta)
    }

    fn close(&mut self, at: PaneId) -> bool {
        if !self.tree.close(at) {
            return false;
        }
        self.map.remove(&at);
        if self.focus == at {
            self.focus = self.tree.leaves().first().copied().unwrap_or(self.focus);
        }
        true
    }

    /// The screen's shape, as the `panes` query answers it (`T088`).
    ///
    /// The tree, which one has focus, and what each pane holds — the last is
    /// the half the tree cannot say, because a `PaneTree` knows arrangement
    /// and a `Pane` knows contents.
    fn describe(&self) -> Value {
        let panes = self
            .tree
            .leaves()
            .into_iter()
            .map(|id| {
                let pane = self.at(id);
                Value::Record(
                    Args::new()
                        .with("pane", numbered(id.0))
                        .with(
                            "holds",
                            Value::Text(
                                match pane.holds() {
                                    PaneKind::Buffer => "buffer",
                                    PaneKind::Transcript => "transcript",
                                    PaneKind::Custom => "custom",
                                }
                                .to_owned(),
                            ),
                        )
                        .with(
                            "buffer",
                            pane.buffer.map_or(Value::Null, |id| numbered(id.0)),
                        ),
                )
            })
            .collect();
        Value::Record(
            Args::new()
                .with("tree", self.tree.describe())
                .with("focus", numbered(self.focus.0))
                .with("panes", Value::List(panes)),
        )
    }

    /// The pane `id` names, or [`None`] if it names none.
    fn get(&self, id: PaneId) -> Option<&Pane> {
        self.map.get(&id)
    }

    /// The pane `id` names.
    ///
    /// Panics on the same terms as [`Buffers::at_mut`]: a `PaneId` that names
    /// no pane is a bug in whoever held it, and until step 10's `close-pane`
    /// there is nothing that can remove one.
    fn at(&self, id: PaneId) -> &Pane {
        self.get(id).expect("a PaneId names an open pane")
    }

    /// The pane `id` names, mutably.
    fn at_mut(&mut self, id: PaneId) -> &mut Pane {
        self.map.get_mut(&id).expect("a PaneId names an open pane")
    }

    /// Which pane a [`PaneRef`] means, or [`None`] if it names none.
    ///
    /// **All five are relative to [`Panes::focus`], not to the pane the Action
    /// is being applied to**, and the difference is the whole of what this
    /// method is for. `Focused {}` means the pane the user is looking at; an
    /// Action applied to some *other* pane and naming `Focused` means that
    /// other pane's Action reaching across to the focused one, which is exactly
    /// what a reveal must not do. Passing the Action's own pane in as "focus"
    /// would collapse the two and make the selector unable to express the
    /// distinction — which it did, until `a_reveal_scrolls_the_pane_the_cursor_is_in`
    /// was written and failed to fail.
    ///
    /// **`Next` and `Prev` walk the tree's order, not the map's.** `<C-w>w`
    /// cycles the windows as they are *arranged*, not as they were opened, and
    /// the two agree for one pane and diverge at two. This read the map's key
    /// order until step 10 gave it a tree to walk.
    ///
    /// **`Direction` is answered by the tree** — the nearest ancestor split on
    /// the matching axis, then that neighbour's nearest leaf. It refused before
    /// this step, and refusing was right then: a compass direction is a fact
    /// about arrangement, and answering it from one pane's rectangle would be
    /// answering from no information at all.
    fn resolve(&self, reference: &PaneRef) -> Option<PaneId> {
        let order = self.tree.leaves();
        let at = order.iter().position(|id| *id == self.focus)?;
        match reference {
            PaneRef::Focused {} => Some(self.focus),
            PaneRef::Id { id } => self.map.contains_key(id).then_some(*id),
            PaneRef::Next {} => order.get((at + 1) % order.len()).copied(),
            PaneRef::Prev {} => order.get((at + order.len() - 1) % order.len()).copied(),
            PaneRef::Direction { direction } => self.tree.toward(self.focus, *direction),
        }
    }
}

impl<'a> Cx<'a> {
    /// The context one Action lands in.
    ///
    /// Named rather than written as a literal at each call, because step 4c
    /// changes what it takes — the pane and the tree both come out of `Panes`
    /// by then — and a constructor is one place to change rather than thirty.
    fn new(buffer: BufferId, pane: PaneId, panes: &'a mut Panes, shell: &'a mut Shell) -> Self {
        Self {
            buffer,
            pane,
            panes,
            shell,
        }
    }

    /// The pane this Action lands in.
    fn view(&self) -> &Pane {
        self.panes.at(self.pane)
    }

    /// The pane this Action lands in, mutably.
    fn view_mut(&mut self) -> &mut Pane {
        self.panes.at_mut(self.pane)
    }
}

impl Pane {
    /// What this pane shows.
    const fn holds(&self) -> PaneKind {
        self.holds
    }

    /// The pane a single-pane session starts in, before any layout has run.
    fn new(buffer: BufferId) -> Self {
        Self {
            holds: PaneKind::Buffer,
            buffer: Some(buffer),
            area: Rect::ZERO,
            alternate: None,
            jumplist: Vec::new(),
            jump_at: 0,
        }
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
    /// What that prompt is about (`T058`, `1c`), resolved when it opened.
    ///
    /// Drained with [`Editing::prompt`], because a chip belongs to the prompt
    /// that raised it and a stale one would name a range the next `:` has
    /// nothing to do with.
    anchor: Option<FileSpan>,
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
    /// Whether this buffer wraps, or [`None`] to follow the option (`T096`).
    ///
    /// **Three-valued on purpose.** `soft-wrap` is a *default*: `init.scm` sets
    /// it, `--soft-wrap` seeds it, and the option is what a session means by
    /// *"wrapping is on"*. `set-soft-wrap` names a **buffer**, so a buffer that
    /// has been told answers for itself and one that has not follows the room.
    /// A `bool` here would make opening a file a decision about wrapping.
    soft_wrap: Option<bool>,
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
    /// The live signature-help or hover answer (`T039`). One field for two
    /// features because they are one surface — see `float::SignatureVm`.
    signature: Option<SignatureVm>,
    /// Regions whose virtual-text rail is collapsed — `set-virtual-text-visible`
    /// with `on: false`. See [`Editing::collapse`] for why the set lives here
    /// rather than in the fork.
    collapsed: BTreeSet<RegionId>,
    /// An `open-float` a **keystroke** asked for (`T048`), drained the way
    /// [`Editing::help`] is.
    ///
    /// `T093` applied the float verbs on the door side only, which was right
    /// for what asked for them then — every caller was an agent. `:arch` is the
    /// first *key* that opens a registered surface, and an ex command's Actions
    /// go through this dispatcher. Same seam `T041` established for the region
    /// verbs: two dispatchers, one store, and the difference is only what each
    /// side can resolve.
    float: Option<(String, Value)>,
    /// An `open-picker` the loop has not put on screen yet, drained the way
    /// [`Editing::open`] and [`Editing::help`] are.
    open_picker: bool,
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
    /// `T087`'s side table: what the fork's marks currently hold for *this*
    /// buffer, so a frame with no news uploads nothing.
    ///
    /// **Per buffer, and it was the loop's.** It describes one `Editor`'s
    /// decoration and works by diffing against what it last uploaded — so one
    /// table against N editors would see every switch as a total change and
    /// re-upload the whole set, which is the exact cost the diff exists to
    /// avoid. Moved here at step 11b for the same reason step 7 moved the
    /// edit counter: a value that describes one buffer cannot be the
    /// session's.
    tints: phosphor_ui::tints::Tints,
    /// How many committed edit batches this buffer has seen (`T038`).
    ///
    /// **Per buffer, and one `Rc<Cell<u64>>` against one `sent` could not say
    /// *"A changed, B did not"*.** It rides the fork's one change callback
    /// beside [`Editing::dirty`] — see [`track_dirty`] for why the two share a
    /// slot — and the loop compares it against [`Editing::sent`] to decide
    /// whether a server needs telling.
    ///
    /// Deliberately **not** reset when a rope is swapped in: the loop compares
    /// against its own last value, and a counter restarting at zero would read
    /// as *"nothing changed"* on the frame a new file opened.
    edits: Rc<Cell<u64>>,
    /// The document the servers have been told about, or [`None`] where this
    /// buffer's language declares no server.
    ///
    /// **A loop local until step 7**, which is the same mistake `Outstanding`
    /// makes one layer up: with N buffers, one `synced` means the server is
    /// told about whichever file happens to be on screen and every other open
    /// file goes stale.
    synced: Option<Document>,
    /// The value [`Editing::edits`] had when the last `didChange` went out.
    ///
    /// The gate is `edits != sent`, and both halves have to be this buffer's or
    /// the comparison is between two different files.
    sent: u64,
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
            .field("selection_kind", &self.selection_kind)
            .field("timeline", &self.timeline)
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
            Rc::new(Cell::new(0)),
            Timeline::detached(),
        )
    }

    fn with_timeline(
        editor: Editor,
        file: Option<PathBuf>,
        dirty: Rc<Cell<bool>>,
        edits: Rc<Cell<u64>>,
        timeline: Timeline,
    ) -> Self {
        let mut editing = Self {
            editor,
            file,
            open: None,
            open_at: None,
            prompt: None,
            anchor: None,
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
            soft_wrap: None,
            lookup: None,
            restart: None,
            question: None,
            completion: None,
            offered: Vec::new(),
            chosen: false,
            signature: None,
            collapsed: BTreeSet::new(),
            float: None,
            open_picker: false,
            selection_kind: SelectionKind::Char,
            selection_from: None,
            timeline,
            depth: 0,
            dirty,
            edits,
            tints: phosphor_ui::tints::Tints::new(),
            synced: None,
            sent: 0,
        };
        // **Every buffer counts its own edits from birth.** The change callback
        // used to be installed by whoever built the [`Editor`], which the loop did
        // via [`dirty_flag`] and no other caller did at all — so a buffer
        // constructed anywhere else silently counted nothing, and the
        // `edits != sent` gate on it was closed forever. Found by the test that
        // asserts A's edits do not move B's, which could not get A's to move.
        editing.retrack();
        editing
    }

    /// The whole rope, as a server and a `:write` both want it.
    fn contents(&self) -> String {
        let code = self.editor.code_ref();
        code.slice(0, code.len_chars())
    }

    /// The buffer as the machine reads it.
    fn text<'a>(&'a self, cx: &'a Cx<'_>) -> EditorText<'a> {
        EditorText {
            editor: &self.editor,
            height: cx.view().area.height,
            regions: self
                .file
                .as_deref()
                .map(|path| (&*cx.shell.store, store::key_for(path))),
        }
    }

    /// One Action, applied, and the cursor revealed if it moved.
    ///
    /// **The reveal is an Action too** ([`Editing::reveal`]), which is what
    /// keeps *"`View::Scroll` is the only thing that moves a viewport"* true
    /// with the cursor still following.
    fn apply(&mut self, cx: &mut Cx<'_>, action: &Action) -> Outcome {
        let outcome = self.act(cx, action);
        if moves_cursor(action) {
            self.reveal(cx);
        }
        outcome
    }

    /// Bring the cursor into view, moving as little as possible.
    ///
    /// Measured in **visual** rows, which is why it happens here and not in the
    /// machine: a soft-wrapped line is several rows and only the widget layer
    /// knows how many (`T081`).
    fn reveal(&mut self, cx: &mut Cx<'_>) {
        let Some(row) = self.editor.visual_row_for_cursor() else {
            return;
        };
        let row = u32::try_from(row).unwrap_or(0) + 1;
        // **`Id`, not `Focused`** — a reveal moves the viewport of the pane
        // the cursor moved in, which is not always the pane the user is looking
        // at. It said `Focused` and was right by accident: the `Scroll` arm
        // dropped the selector too, so both halves ignored it and agreed. The
        // moment that arm started reading it, a reveal in an unfocused pane
        // would have scrolled the focused one — a defect no existing test could
        // see, because it needs two panes and the mistake is in the *pair*.
        let pane = cx.pane;
        let _ = self.act(
            cx,
            &Action::View(ViewAction::Scroll {
                request: phosphor_core::request::ScrollRequest::RevealRow { row, margin: 0 },
                pane: PaneRef::Id { id: pane },
            }),
        );
    }

    /// One Action. The `_` arm answers with the task that builds it, derived
    /// from the capability's own row rather than from a list here.
    fn act(&mut self, cx: &mut Cx<'_>, action: &Action) -> Outcome {
        // **The four buffer selectors, checked once here rather than dropped in
        // four arms.** `set-cursor` and the three `ingest-*` answers each carry
        // an `Option<BufferId>` whose doc says *"absent means the focused
        // one"*; the arms wrote `..` and read whatever buffer they were called
        // on, so `Some(anything)` and `None` did the same thing and an id an
        // agent held across a `close-buffer` moved the wrong cursor.
        //
        // The loop routes what it can — [`Buffers::named`] at the posted door
        // sends the Action to the buffer it names — and this is the half that
        // cannot be routed: an ex line runs against the buffer it was typed in,
        // and a Steel command may name any id. `NoSuchTarget` is the refusal
        // whose own doc names this case, *"a stale id from an agent working off
        // an old query"*.
        if Buffers::named(action, cx.buffer) != cx.buffer {
            return Outcome::Refused(Refusal::NoSuchTarget);
        }
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
            // `T052` — **the shape an agent writes through.** Its row calls it
            // *"the primitive `T029`'s log replays"*, and one `u` undoing the
            // whole batch is what makes it that shape: an agent that rewrote
            // nine call sites is one keystroke away from before it, not nine.
            //
            // **The undo group is free here, and saying otherwise was the
            // first version of this comment.** It claimed the `begin`/`commit`
            // below was *"the whole of the undo group"*; removing the pair and
            // re-running the test proved it is not — the test still passed.
            // The boundary belongs to the input machine: `Timeline::close`'s
            // own doc says *"the group boundary is the machine's —
            // `History::CommitUndoGroup`, emitted at exactly the three places
            // vim closes one"*, so every edit made while applying **one**
            // Action is already one group, however many `splice` calls it
            // takes. This arm gets that for nothing, and the test asserts it
            // because it is `T052`'s acceptance rather than because this code
            // causes it.
            //
            // What the pair *does* buy is one fork transaction and one
            // highlight-cache reset for the batch instead of N:
            // `Editing::begin` is depth-counted, so each `splice` inside sees
            // depth 2 and returns early. That is worth having and is not what
            // the acceptance is about.
            Action::Buffer(BufferAction::ApplyEdits { edits }) => {
                // **Last first, and this is not a preference.** The spans are
                // positions in the document *as the agent read it*, and
                // applying one moves every position after it. Descending by
                // start offset keeps every span not yet applied valid, which is
                // the same rule LSP puts on a `WorkspaceEdit` and for the same
                // reason.
                //
                // **What that costs is narrower than it first looks, and the
                // test had to be rewritten to find it.** A [`Span`] is
                // line-and-column, not an offset, and `Editing::range` resolves
                // it against the document as it stands — so two edits on
                // *different* lines survive either order as long as neither
                // changes how many lines there are. The first version of
                // `apply_edits_is_one_undo_group` used exactly that pair and
                // **passed with the sort planted front-to-back**. Two edits on
                // one line is where the order is load-bearing: replacing five
                // columns with three moves everything after it on that row, and
                // the second edit lands three columns late. That is the pair
                // the test uses now.
                //
                // Stable, so two edits at one offset keep the order the agent
                // declared them in — *"the edits, in order"* is the row's own
                // wording, and it is the only thing left for it to mean once
                // position has decided the rest.
                let mut ordered: Vec<&phosphor_core::request::Edit> = edits.iter().collect();
                ordered.sort_by_key(|edit| std::cmp::Reverse(self.offset(edit.span.start)));
                self.begin();
                for edit in ordered {
                    self.remove(edit.span);
                    self.insert(edit.span.start, &edit.text);
                }
                self.commit();
                done()
            }
            Action::Buffer(BufferAction::Yank { target, register }) => {
                self.yank(cx, target, register.as_ref());
                done()
            }
            Action::Buffer(BufferAction::Paste {
                register, before, ..
            }) => {
                self.paste(cx, register.as_ref(), *before);
                done()
            }
            Action::Buffer(BufferAction::SetRegister { register, text }) => {
                cx.shell.registers.insert(
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
                self.insert_indent(cx);
                done()
            }
            Action::Buffer(BufferAction::JoinLines { target }) => {
                self.join(cx, target);
                done()
            }
            Action::Motion(MotionAction::MoveCursor { motion, count }) => {
                let to =
                    motion::cursor_after(&self.text(cx), self.text(cx).cursor(), *motion, *count);
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
                let to =
                    motion::cursor_after(&self.text(cx), self.text(cx).cursor(), *motion, *count);
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
            // **The selector is read, not dropped.** This arm wrote `..` and
            // measured against whatever pane it was called on, so a `scroll`
            // naming a pane that does not exist moved the one in front of the
            // user. The area is a pane's — the page size and a `RevealRow`
            // margin are both counted in its rows — so which pane is named
            // changes the answer.
            //
            // **With one pane a resolved reference is always this pane**, and
            // that is deliberate: the branch where the resolved pane shows a
            // *different* buffer is not written, because it cannot be reached.
            // `apply_scroll` moves `self.editor`'s viewport and `self` is the
            // buffer this Action was routed to; reaching another buffer's is
            // the aliasing wall step 6a hit, and the answer there was routing
            // at the door rather than a branch here. Step 11 is where a second
            // pane makes it reachable, and it needs the routing, not an arm.
            Action::View(ViewAction::Scroll { request, pane }) => {
                let Some(target) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                let area = cx.panes.at(target).area;
                buffer_view::apply_scroll(&mut self.editor, *request, area);
                done()
            }
            // `R19` — folds. `T016`'s whitespace half shipped with `8e`; this is
            // the half that never had a call site, and the machinery is the
            // fork's (`code.rs`'s `fold_query` / `fold_ranges`, read out of
            // `langs/<lang>/folds.scm`).
            // **`T096` — the verb the doors have advertised since `T081`.**
            // `set-soft-wrap` was declared, generated into Steel, MCP and the
            // CLI, and applied by nothing: a capability three doors offer and
            // that does nothing is worse than one that is absent, which is what
            // `scripts/lint-action-arms.sh` has said on every run since.
            //
            // **The target is honoured rather than ignored.** The row says
            // *"which buffer"*, and a global toggle wearing a per-buffer
            // signature is the kind of almost-true this build spends its lints
            // on. `Target::Cursor` is the focused one — the same reading every
            // focus-relative target has.
            Action::View(ViewAction::SetSoftWrap { target, on }) => {
                match target {
                    Target::Cursor {} | Target::Selection {} => {
                        self.soft_wrap = Some(*on);
                        done()
                    }
                    Target::Buffer { id } if *id == cx.buffer => {
                        self.soft_wrap = Some(*on);
                        done()
                    }
                    // **Refused rather than silently applied to the focused
                    // buffer.** An `Editing` is one buffer; a target naming
                    // another is a request this arm cannot honour, and honouring
                    // it against the wrong rope is worse than saying so.
                    Target::Buffer { .. } | Target::File { .. } => {
                        Outcome::Refused(Refusal::NoSuchTarget)
                    }
                    _ => declined("soft wrap is a buffer's, not a row's"),
                }
            }
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
            // **`:wall` writes every dirty buffer, and the loop is what does
            // it.** This arm called `self.write(None)` under a comment saying
            // *"there is exactly one, and `T088` is what makes there be more"*.
            // There are more now, and an arm holds one — so it records the ask
            // and the loop performs it, which is the seam [`Intent`] already
            // establishes for *"the thing that decides is not the thing that
            // owns"*. `Editing::write` is still the only thing that writes;
            // the loop calls it per buffer rather than a second implementation
            // existing.
            Action::File(FileAction::SaveAll {}) => {
                cx.shell.wall = true;
                done()
            }
            // **`at` is honoured now, and that is `T036`'s doing.** The arm
            // recorded the path and dropped the position, which nothing had
            // noticed because every caller so far was `:edit <path>` — and
            // `gd` is the first one that means *this line of it*.
            Action::File(FileAction::OpenFile { path, at, .. }) => {
                self.open = Some(path.clone());
                self.open_at = *at;
                done()
            }
            // **`T056`'s jump, and it is `open-file` with a different name.**
            // The capability's own sentence names its three callers — *"a
            // picker accept, a transcript tool row, an OSC 8 link"* — and what
            // they have in common is that none of them is a *person naming a
            // file*. `open-file` is `:e`; this is a place something already
            // pointed at.
            //
            // **The two are redundant at the arm and that is recorded rather
            // than resolved**, the same way `open-arch` and `open-float` are:
            // collapsing them is a vocabulary decision and this is not the
            // layer that gets to make one. What is *not* redundant is the
            // shape — `position` here is the point of the verb, where
            // `open-file`'s `at` is optional garnish on naming a file — and a
            // door caller that can only say "here" needs a verb that says so.
            Action::Motion(MotionAction::GotoLocation { path, position, .. }) => {
                self.open = Some(path.clone());
                self.open_at = *position;
                done()
            }
            // **vim's `CTRL-^`.** The alternate file is the one you were in
            // before this one, and pressing it twice puts you back — which is
            // the whole of why it is worth a key: the two files you are moving
            // between during an edit are almost never adjacent in any list.
            //
            // It is `open-file` with a path the *editor* supplies rather than
            // the caller, which is why it needs a capability of its own: a
            // keymap is data and cannot compute the path, and a door caller
            // that knew it would not need this verb.
            //
            // Declines rather than doing nothing when there is nowhere to go.
            // A key that silently no-ops is the shape `T016` was ticked with
            // and `lint-action-arms` exists to catch; the first file of a
            // session has no alternate and saying so is the honest answer.
            Action::File(FileAction::OpenAlternate { .. }) => {
                let Some(alternate) = cx.view().alternate.clone() else {
                    return declined("no alternate file — nothing else has been open yet");
                };
                self.open = Some(alternate);
                // Deliberately no `open_at`: vim restores the alternate file's
                // own last cursor position, and this build has nowhere to keep
                // one per file. Landing at the top is the honest version of
                // that until something does — `T030`'s journal is keyed per
                // file and is where it would live.
                self.open_at = None;
                done()
            }
            // Not `NotYetImplemented`: the capability is built and the limit is
            // real. One pane holds one buffer until `T088`, so closing it is
            // leaving, and `:quit` is how you say that.
            // **It closes now.** This declined with *"one buffer, one pane —
            // :quit leaves; T088 gives a buffer somewhere to close to"*, which
            // was true and is not any more.
            //
            // The arm answers what it can see: a dirty buffer refuses without a
            // `force`, exactly as `:quit` does. Whether there is anywhere for
            // the pane to go afterwards is a question about every buffer, so
            // the loop answers that one.
            Action::File(FileAction::CloseBuffer { force, .. }) => {
                if !*force && self.dirty.get() {
                    return Outcome::Refused(Refusal::WouldLoseWork);
                }
                cx.shell.closing = Some(cx.buffer);
                done()
            }
            // ---------------------------------------------------------------
            // `T088`'s pane verbs. **Arms, and the plan said they could not be.**
            //
            // Its reasoning was that they *"mutate the tree an `Editing` was
            // borrowed out of"*, which was true when it was written and stopped
            // being true at step 4c: an `Editing` is borrowed out of `Buffers`
            // and the tree is in `Panes`, which are two structs. Step 6b then
            // put `&mut Panes` in the context so a resolved `PaneRef` could
            // name a pane that is not the Action's own — and that is exactly
            // what these four need. So they are ordinary arms, and the ask/drain
            // indirection the plan reached for is not needed.
            //
            // Every one resolves its `PaneRef` first and refuses `NoSuchTarget`
            // when it names nothing, which is the rule step 6b set for `scroll`.
            Action::Pane(PaneAction::SplitPane {
                pane,
                direction,
                kind,
            }) => {
                let Some(at) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                if matches!(kind, PaneKind::Custom) {
                    // v1.5's agent-built pane. `Transcript` used to be refused
                    // here beside it and is `T054`'s now.
                    return Outcome::Refused(Refusal::NotYetImplemented { task: "v1.5" });
                }
                // **The new pane shows the same buffer**, which is what vim's
                // `:split` does. Opening a *different* file into it is
                // `open-file` with a `PaneRef`, and that is a second Action
                // rather than an argument here.
                //
                // **A transcript pane holds no buffer**, which is what
                // `Pane::buffer`'s [`Option`] has always been for: *"a pane
                // holding something that is not one — the transcript, or a view
                // tree claude emitted"*. `1b` is a split and not a takeover —
                // the code stays above it — so this is one call rather than
                // `split-pane` followed by `set-pane-content`, and `T054`'s
                // binding is one line because of it.
                let mut fresh = Pane::new(cx.buffer);
                fresh.holds = *kind;
                if matches!(kind, PaneKind::Transcript) {
                    fresh.buffer = None;
                }
                match cx.panes.split(at, fresh, *direction) {
                    Some(id) => Outcome::Done(Receipt {
                        capability: "split-pane",
                        value: Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)),
                        note: None,
                    }),
                    None => Outcome::Refused(Refusal::NoSuchTarget),
                }
            }
            Action::Pane(PaneAction::FocusPane { pane }) => {
                let Some(at) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                cx.panes.focus = at;
                done()
            }
            Action::Pane(PaneAction::ClosePane { pane }) => {
                let Some(at) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                if cx.panes.close(at) {
                    done()
                } else {
                    // The tree refuses to become empty, and that is the honest
                    // answer rather than an error: closing the only window is
                    // what `:quit` means, and it is a different verb.
                    declined("the only pane — :quit leaves")
                }
            }
            Action::Pane(PaneAction::ResizePane { pane, delta }) => {
                let Some(at) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                // **Percentage points, not cells**, and the vocabulary says
                // cells — see [`PaneTree::Split::first_share`] for why the tree
                // does not know how big anything is. Step 11 draws the rects
                // and is where a cell can be counted; until then the honest
                // conversion is one point per cell, which is what a `<C-w>+`
                // bound to `delta: 1` means on an 80-column frame anyway.
                let step = i16::try_from(*delta).unwrap_or(0);
                if cx.panes.resize(at, step) {
                    done()
                } else {
                    declined("nothing to resize — one pane, or already at the edge")
                }
            }
            // The ex line. `T058` builds the message and search prompts and the
            // anchor chip that rides with them; the ex half is `T033`'s, because
            // an editor you cannot type `:write` into is not one CP-3 can judge.
            Action::Prompt(PromptAction::OpenPrompt { kind, anchor, .. }) => match kind {
                PromptKind::Ex | PromptKind::Claude => {
                    self.prompt = Some(*kind);
                    // `T058` — `1c`'s whole caption: *"visual-select, hit the
                    // prompt — file & range ride along automatically"*.
                    //
                    // **Resolved here and not carried as a `Target`**, because
                    // a target is a *question* — `Target::Selection {}` means
                    // "whatever is selected", and the selection is gone by the
                    // time the prompt is submitted. The chip has to name a
                    // range that will still be true, so the answer is taken
                    // now.
                    self.anchor = anchor.as_ref().and_then(|target| self.file_span(target));
                    done()
                }
                PromptKind::Search => declined("search is T058's other half — :/ is not built yet"),
            },
            // `T058`'s four surface verbs. Each posts a step the loop
            // performs — see [`Shell::prompt_step`] for why an arm cannot do
            // it here — and each refuses when there is no prompt open, because
            // a verb that silently did nothing would be indistinguishable from
            // one that is not built.
            Action::Prompt(PromptAction::SetPromptText { text }) => {
                if self.prompt.is_none() {
                    return declined("no prompt is open");
                }
                cx.shell.prompt_step = Some(PromptStep::Set(text.clone()));
                done()
            }
            Action::Prompt(PromptAction::SubmitPrompt {}) => {
                if self.prompt.is_none() {
                    return declined("no prompt is open");
                }
                cx.shell.prompt_step = Some(PromptStep::Submit);
                done()
            }
            Action::Prompt(PromptAction::CancelPrompt {}) => {
                if self.prompt.is_none() {
                    return declined("no prompt is open");
                }
                cx.shell.prompt_step = Some(PromptStep::Cancel);
                done()
            }
            Action::Prompt(PromptAction::PromptHistory { delta }) => {
                if self.prompt.is_none() {
                    return declined("no prompt is open");
                }
                cx.shell.prompt_step = Some(PromptStep::History(*delta));
                done()
            }
            Action::App(AppAction::Quit { force }) => {
                if !*force && self.dirty.get() {
                    return Outcome::Refused(Refusal::WouldLoseWork);
                }
                cx.shell.quit = true;
                cx.shell.discard = *force;
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
                self.fall_through(cx, binding)
            }
            Action::Lsp(LspAction::AcceptCompletion {
                index,
                then,
                otherwise,
            }) => match self.accept(cx, *index, then.as_deref(), otherwise.as_deref()) {
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
                if self.text(cx).cursor() == *at {
                    // An **empty list closes the float**, which the declaration
                    // says out loud: the client answers exactly once on every
                    // path, so `Insight::Nothing` arrives here as an empty list
                    // and a float that suppressed it would leave a stale list
                    // beside the cursor forever.
                    let next = (!items.is_empty()).then(|| self.completions(cx, items));
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
                if self.text(cx).cursor() == *at {
                    let next = signature.as_ref().map(|signature| SignatureVm {
                        label: Some(signature.label.clone()),
                        active: signature
                            .active
                            .map(|range| (range.start as usize, range.end as usize)),
                        // §11 is "nothing ever wraps", so the wrapping is here
                        // and the width is the float's own — see `wrapped`.
                        prose: self.wrapped(cx, &signature.documentation),
                        anchor: self.anchor(cx, 0),
                        width_floor: 0,
                    });
                    self.signature = next.map(|vm| self.held_to_widest(vm));
                }
                done()
            }
            Action::Lsp(LspAction::IngestHover { prose, at, .. }) => {
                if self.text(cx).cursor() == *at {
                    let next = (!prose.is_empty()).then(|| SignatureVm {
                        // Hover has no callable to name; the whole answer is
                        // prose. `SignatureVm` is one type for both features
                        // and this is the difference between them.
                        label: None,
                        active: None,
                        prose: self.wrapped(cx, prose),
                        anchor: self.anchor(cx, 0),
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
                cx.shell.store.publish(path.clone(), diagnostics.clone());
                done()
            }
            // `T041` — §7's state machine, reached from a keystroke. The door's
            // copy of these four is in `AppHost::apply`; the difference is
            // exactly the focus-relative targets, which only this side can
            // resolve because only this side has an editor.
            Action::Region(RegionAction::MarkSeen { target }) => {
                self.mark(cx, target, SeenState::Seen)
            }
            Action::Region(RegionAction::MarkUnseen { target }) => {
                self.mark(cx, target, SeenState::Unseen)
            }
            Action::Region(RegionAction::DeclareRegions { regions }) => {
                let answer = declared(name, &cx.shell.store.declare(regions, Actor::You));
                // `T043`. A declaration carries a path and a span; finding that
                // span again after a rewrite needs the file's *text*, and this
                // is the side that has it. Done here rather than lazily at the
                // next reanchor because the text a region was declared against
                // is the text it was declared against — a fingerprint taken
                // later describes whatever has since moved onto that line.
                self.fingerprint_declared(cx);
                Outcome::Done(answer)
            }
            Action::Region(RegionAction::DropRegions { target }) => match self.scope_of(target) {
                Ok(scope) => Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Int(
                        i64::try_from(cx.shell.store.drop_regions(&scope)).unwrap_or(0),
                    ),
                    note: None,
                }),
                Err(why) => declined(&why),
            },
            // `T041`'s owed arm, recorded against this task in
            // `scripts/lint-action-arms.sh`: collapsing a virtual-text rail
            // addresses it by owning region, and regions are this task.
            Action::View(ViewAction::SetVirtualTextVisible { owner, on }) => {
                self.collapse(cx, owner, *on)
            }
            // `T042` — anchors. Four arms and one task: `place-anchor` is the
            // setter `goto-anchor` never had (which is why `m`, `'` and `` ` ``
            // were bound to silence), `reanchor` is the ladder run after a
            // rewrite, and `jump` is here rather than with the motions because
            // a jumplist entry *is* an anchor.
            Action::Region(RegionAction::PlaceAnchor { at, label }) => {
                self.place_anchor(cx, at, label.as_ref())
            }
            Action::Region(RegionAction::Reanchor { path }) => self.reanchor(cx, path),
            Action::Motion(MotionAction::GotoAnchor {
                anchor,
                label,
                exact,
            }) => self.goto_anchor(cx, *anchor, label.as_deref(), *exact, true),
            Action::Motion(MotionAction::Jump { seek }) => self.jump(cx, *seek),
            // `T049` — `]u` / `[u` and `SPC u n`. The other seven sequences
            // decline by naming what builds them, which is the same rule the
            // agent nouns follow one layer down: a sequence with no store is a
            // no-op, and a no-op that moved the cursor somewhere plausible
            // would be worse than one that says nothing happened.
            Action::Motion(MotionAction::GotoSequence {
                sequence,
                seek,
                filter,
            }) => self.goto_sequence(cx, *sequence, *seek, filter.as_ref()),
            // `T048` — the float verbs, from a *key*. `T093` applied these on
            // the door side; `:arch` is the first keystroke that opens a
            // registered surface, and an ex command's Actions come through
            // here. Composing a surface runs scheme, so this records the ask
            // and the loop performs it — exactly what `Editing::open` and
            // `Editing::help` already do.
            Action::Float(FloatAction::OpenFloat { surface, args }) => {
                self.float = Some((surface.0.clone(), Value::Record(args.clone())));
                done()
            }
            // `T048`. **A named verb for one registered surface**, which is
            // what makes `:arch` reachable from a door without the caller
            // having to know the registry exists — `open-arch` is `Allow` for
            // MCP where `open-float` names an id an agent would have to have
            // been told. It lowers to the same call, so there is one
            // composition path and not two.
            Action::App(AppAction::OpenArch {}) => {
                self.float = Some((ARCH_SURFACE.to_owned(), Value::Record(Args::new())));
                done()
            }
            // `T057` — `7d`/`5d`, and a sibling of `:arch` in every way that
            // matters: a **Steel** surface, because the dashboard is rows of
            // facts and `Node::Spans` draws rows of facts. Zero lines in
            // `phosphor-ui`, and the id is the one `runtime/dashboard.scm`
            // registers.
            Action::App(AppAction::OpenDashboard {}) => {
                self.float = Some((DASHBOARD_SURFACE.to_owned(), Value::Record(Args::new())));
                done()
            }
            // *"three verbs, then out of the way"* — `7d`'s own caption, and
            // this is the "then".
            Action::App(AppAction::DismissDashboardHint {}) => {
                cx.shell.hinted = true;
                done()
            }
            // `T045` — the picker's own three. `open-picker`'s row cites
            // `T046` and is applied here anyway: a widget nothing can put on
            // screen is the reachability gap `T016` was ticked with, and the
            // *rows* are what `T046` actually owes. An open picker over a
            // source nobody has defined draws `0/0`, which is honest.
            Action::Picker(PickerAction::OpenPicker { source, query }) => {
                cx.shell.picker = Some(PickerSession::open(
                    source.clone(),
                    query.clone(),
                    &cx.shell.wake,
                ));
                self.open_picker = true;
                done()
            }
            Action::Picker(PickerAction::SetPickerQuery { text }) => {
                let Some(session) = cx.shell.picker.as_mut() else {
                    return declined("no picker open");
                };
                session.filter.clone_from(text);
                session.matcher.filter(text);
                // The count, not `#ok`: a script that filters wants to know
                // whether it found anything, and the alternative is a second
                // round trip through the `picker` query for a number this call
                // already has. Partial while the matcher is still running —
                // `PickerVm::matching` is what says so on screen.
                Outcome::Done(Receipt {
                    capability: "set-picker-query",
                    value: Value::Int(i64::try_from(session.matcher.matched()).unwrap_or(0)),
                    note: None,
                })
            }
            // `T045`'s other two, and they are the *float's* verbs rather than
            // the picker's: `float-select-row` and `float-accept` name a row of
            // whatever float has focus. The picker is the only float with rows
            // to select today, so this is where they land — a completion list
            // is `T038`'s own session and a `6d` help grid has no selection at
            // all. A float without rows declines by name.
            Action::Float(FloatAction::FloatSelect { delta }) => {
                let Some(session) = cx.shell.picker.as_mut() else {
                    return declined("no float with rows is focused");
                };
                session.matcher.select(*delta);
                done()
            }
            Action::Float(FloatAction::FloatSelectRow { row }) => {
                let Some(session) = cx.shell.picker.as_mut() else {
                    return declined("no float with rows is focused");
                };
                // 1-based on the wire, and the delta is against wherever the
                // selection is — `Picker::select` is the one clamp, so a row
                // past the end lands on the last rather than nowhere.
                let target = i64::from(row.saturating_sub(1));
                session.matcher.select_to(target);
                done()
            }
            // `T047`. A float's primary verb, which for the only float with
            // rows is accepting one — so this is `picker-accept` under the
            // float's name, and it delegates rather than duplicating.
            Action::Float(FloatAction::FloatAccept {}) => {
                if cx.shell.picker.is_none() {
                    return declined("no float with a primary verb is focused");
                }
                self.accept_picker(cx, AcceptHow::Open, Direction::Right)
            }
            // `T059`. **`4a`'s digits under the float's name**, and it
            // delegates rather than duplicating — the same shape `float-accept`
            // above takes to `picker-accept`, and for the same reason: what a
            // digit *means* is the float's business and what it *does* is the
            // ask's.
            //
            // **The focused ask is resolved here and is not a parameter**,
            // which is the whole difference between this verb and `answer-ask`.
            // The node's sentence is *"digits answer only while it is
            // focused"*; a digit that carried an ask id would be a digit that
            // could answer a question you are not looking at.
            Action::Float(FloatAction::FloatAnswer { digit }) => {
                let Some(asked) = cx.shell.asked else {
                    return declined("no question is focused");
                };
                let offered =
                    cx.shell.asks.get(&asked).is_some_and(|question| {
                        question.options.iter().any(|it| it.digit == *digit)
                    });
                if !offered {
                    // **Declined by name rather than swallowed.** A float that
                    // ate the key would be indistinguishable from one that had
                    // not noticed.
                    return declined(&format!("no option {digit} — press one that is offered"));
                }
                // **`T061` — a permission ask answers through its own verbs.**
                // `7a`'s three digits are not three answers to one question:
                // `[1]` and `[2]` both let it run and differ in what they
                // *write*, and `[3]` is a refusal. `grant-permission` and
                // `deny-permission` exist because that distinction is a
                // vocabulary fact, and routing a permission digit through
                // `answer-ask` would lose it — `[2]` would be an answer of `2`
                // and the rule would never be written.
                if cx.shell.asking_about.contains_key(&asked) {
                    return match digit {
                        1 => self.act(
                            cx,
                            &Action::Ask(AskAction::GrantPermission {
                                ask: asked,
                                scope: phosphor_core::request::GrantScope::Once,
                            }),
                        ),
                        2 => self.act(
                            cx,
                            &Action::Ask(AskAction::GrantPermission {
                                ask: asked,
                                scope: phosphor_core::request::GrantScope::Always,
                            }),
                        ),
                        _ => self.act(cx, &Action::Ask(AskAction::DenyPermission { ask: asked })),
                    };
                }
                self.act(
                    cx,
                    &Action::Ask(AskAction::AnswerAsk {
                        ask: asked,
                        digit: Some(*digit),
                        prose: None,
                    }),
                )
            }
            Action::Picker(PickerAction::PickerAccept { how }) => {
                self.accept_picker(cx, *how, Direction::Right)
            }
            // `T047` — tab. The *order* is the layer's, so this arm knows how
            // to walk a list and not what is in it. A picker over a source the
            // order does not name starts from the first, which is what makes
            // tab work on a picker opened by id from a door.
            Action::Picker(PickerAction::CyclePickerSource { delta }) => {
                let Some(session) = cx.shell.picker.as_ref() else {
                    return declined("no picker open");
                };
                let order = std::mem::take(&mut cx.shell.source_order);
                if order.is_empty() {
                    return declined("no source order — (define phosphor/picker-sources …)");
                }
                let at = order
                    .iter()
                    .position(|id| id == &session.source.0)
                    .unwrap_or(0);
                let len = i64::try_from(order.len()).unwrap_or(1);
                let next = (i64::try_from(at).unwrap_or(0) + delta).rem_euclid(len);
                let id = order[usize::try_from(next).unwrap_or(0)].clone();
                cx.shell.source_order = order;
                // Re-opened rather than mutated: a source change is a new
                // corpus, and `open-picker` is the one path that fills one.
                let filter = cx.shell.picker.as_ref().map(|s| s.filter.clone());
                cx.shell.picker = Some(PickerSession::open(
                    SourceId(id.clone()),
                    filter,
                    &cx.shell.wake,
                ));
                self.open_picker = true;
                Outcome::Done(Receipt {
                    capability: "cycle-picker-source",
                    value: Value::Text(id),
                    note: None,
                })
            }
            Action::Picker(PickerAction::TogglePickerPreview {}) => {
                let Some(session) = cx.shell.picker.as_mut() else {
                    return declined("no picker open");
                };
                session.preview = !session.preview;
                Outcome::Done(Receipt {
                    capability: "toggle-picker-preview",
                    value: Value::Bool(session.preview),
                    note: None,
                })
            }
            // `T036` — `gd`. Recorded like the lookups, and answered by an
            // `open-file` rather than by a float: a definition is a *place*.
            Action::Lsp(LspAction::RequestDefinition {}) => {
                self.question = Some(Question::Definition);
                done()
            }
            // `T047`, and the arm the `S4` wiring pass re-homed here.
            // `Question::References` and its whole client path were *"built and
            // unreached"* until this task, because what a list of places needs
            // is a surface to be drawn in — which is the picker.
            Action::Lsp(LspAction::RequestReferences {}) => {
                self.question = Some(Question::References);
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

            // -- `T050`: the session ------------------------------------------
            //
            // **Neither of these touches a buffer**, which is exactly why they
            // are arms here rather than in the loop: `Cx` is how an Action
            // reaches something that is not its own rope, and a turn is a fact
            // about the session. The alternative — the loop intercepting two
            // Actions before `act` ever sees them — is a second applier, and
            // this build has spent a window making sure there is one.
            Action::Session(SessionAction::TurnBegan { turn, prompt }) => {
                cx.shell.turn = Some((*turn, Instant::now()));
                // `T054`. The transcript's turn is opened here rather than by
                // the first chunk of prose, so a turn that produces none still
                // has a row — which is what `1b`'s prompt line is: what you
                // asked, whether or not he has answered yet.
                let began = cx.shell.transcript.at(*turn);
                began.prompt.clone_from(prompt);
                done()
            }
            Action::Session(SessionAction::TurnEnded { turn, summary }) => {
                // `T054` — `1b`'s seam marker, which is the row that says a
                // turn is over and what came of it.
                // **A pause outranks the end, and this is not a preference.**
                // `7e`'s seam says *where* the turn stopped and why; the stop
                // reason that follows a `session/cancel` — or that an agent
                // sends anyway, having not honoured one — would replace it with
                // `✻ EndTurn`, and the screen would forget the pause it is
                // still in. Measured: the probe drew exactly that.
                let paused_here = cx
                    .shell
                    .paused
                    .as_ref()
                    .is_some_and(|(held, _)| held == turn);
                let ended = cx.shell.transcript.at(*turn);
                if !paused_here {
                    ended.ended = Some(phosphor_ui::transcript::Seam {
                        text: summary.clone().unwrap_or_else(|| "turn ended".to_owned()),
                        detail: None,
                        tone: phosphor_ui::transcript::SeamTone::Ended,
                    });
                }
                // **Only the turn that is running ends.** A stop reason for a
                // turn the editor has already forgotten is not an error — a
                // session replaced mid-turn produces exactly one — and clearing
                // unconditionally would blank a *newer* turn's clock, leaving
                // the statusline saying `idle` while claude works.
                if cx.shell.turn.is_some_and(|(running, _)| running == *turn) {
                    cx.shell.turn = None;
                }
                done()
            }
            // -- `T059`: `4a`, claude asking mid-turn ------------------------
            //
            // **Armed in both appliers, and the write is one function.** A door
            // lands in `AppHost::apply` and a keystroke lands here — `:ask` is
            // an ex command and an ex command is a keystroke — so the arm has
            // to exist twice or the verb works from one door and answers
            // `not built yet` from the other. It did exactly that, measured at
            // the terminal: `(enqueue-ask! …)` at the REPL raised `4a` and
            // `:ask …` said *"not built yet — T060 builds it"*.
            //
            // What is *not* duplicated is what happens: both call
            // [`Shell::enqueue_ask`], both mint from the same counter, and
            // neither composes a float — the loop raises one by comparing
            // `Shell::asking` against `Shell::asked`.
            Action::Ask(AskAction::EnqueueAsk { prose, options }) => {
                let id = cx.shell.mint_ask();
                cx.shell.enqueue_ask(
                    id,
                    phosphor_ui::question::QuestionVm {
                        prose: prose.clone(),
                        options: options.clone(),
                    },
                );
                Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)),
                    note: None,
                })
            }
            // **`T060` — `apply-workspace-edit`, the arm this queue owed a task
            // that is not its own.** `T036` built the reading half two windows
            // ago and `scripts/lint-action-arms.sh` has named this row on every
            // run since.
            //
            // **Recorded, not applied, and that is structural.** An `Editing`
            // holds one rope; a server's rename is edits in several files. So
            // the arm says what to do and the loop does it — the shape
            // `Editing::open` and `Shell::closing` already have, and the reason
            // is the same in all three: `Buffers` is the loop's.
            Action::Lsp(LspAction::ApplyWorkspaceEdit { files }) => {
                if files.is_empty() {
                    // Not `NoSuchTarget`: an empty edit is a server saying
                    // there was nothing to do, which is a legitimate answer to
                    // a rename that matched nothing.
                    return done();
                }
                cx.shell.edits.extend(files.iter().cloned());
                done()
            }
            // -- `T062`: `7e`, interrupt and steer ---------------------------
            //
            // **Four verbs over one pair of fields**, and the pair is the whole
            // design: `Shell::pausing` is the *request* and `Shell::paused` is
            // what it becomes when the agent reaches a boundary. An interrupt
            // that took effect immediately would land in the middle of whatever
            // the agent was doing, which is the thing a tool boundary exists to
            // avoid.
            Action::Session(SessionAction::InterruptSession {}) => {
                let Some((turn, _)) = cx.shell.turn else {
                    return declined("no turn to interrupt");
                };
                if cx.shell.paused.is_some() {
                    return declined("already paused — ↵ steers, :resume carries on");
                }
                cx.shell.pausing = true;
                // **Over the wire as well as in the editor.** A pause that
                // stopped *drawing* the agent's work while the agent went on
                // doing it would be a strip saying `⏸ claude paused` about
                // something that is not — and it is what the first version did:
                // the toy agent finished its turn and `✻ EndTurn` overwrote the
                // seam that had just been written. ACP's own note is that final
                // updates may still arrive after a cancel, which is why the
                // boundary below is still what decides where it stopped.
                cx.shell.session.interrupt();
                // **Said out loud, because the pause has not happened yet.**
                // The seam appears when the agent reaches a boundary, which may
                // be a second away; `esc` with nothing on the strip would read
                // as a key that did nothing.
                cx.shell.saying = Some("pausing at the next tool boundary".to_owned());
                let _ = turn;
                done()
            }
            // `↵ steer & resume`. **The correction is a prompt**, which is what
            // makes it steering rather than a note: the agent gets it, and what
            // it does next is a turn that heard you.
            Action::Session(SessionAction::SteerSession { body }) => {
                if cx.shell.paused.is_none() {
                    return declined("nothing is paused — esc pauses at the next boundary");
                }
                cx.shell.steering = Some(body.clone());
                done()
            }
            // `:resume` — carry on as it was. The held call runs; nothing is
            // said to the agent.
            Action::Session(SessionAction::ResumeSession {}) => {
                let Some((turn, held)) = cx.shell.paused.take() else {
                    return declined("nothing is paused");
                };
                cx.shell.pausing = false;
                cx.shell.transcript.at(turn).next = None;
                cx.shell.transcript.at(turn).ended = None;
                cx.shell.transcript.at(turn).calls.push(held);
                cx.shell.transcript.revision += 1;
                cx.shell.saying = Some("resumed".to_owned());
                done()
            }
            // `:abort` — the turn is over. **The held call does not run**, which
            // is the difference from `:resume` and the reason a boundary is
            // where this is offered: aborting between calls leaves nothing
            // half-done.
            Action::Session(SessionAction::AbortTurn {}) => {
                let Some((turn, _)) = cx.shell.paused.take() else {
                    return declined("nothing is paused");
                };
                cx.shell.pausing = false;
                let ended = cx.shell.transcript.at(turn);
                ended.next = None;
                ended.ended = Some(phosphor_ui::transcript::Seam {
                    text: "turn abandoned".to_owned(),
                    detail: Some("the held call did not run".to_owned()),
                    tone: phosphor_ui::transcript::SeamTone::Trouble,
                });
                cx.shell.transcript.revision += 1;
                if cx.shell.turn.is_some_and(|(running, _)| running == turn) {
                    cx.shell.turn = None;
                }
                cx.shell.saying = Some("turn abandoned".to_owned());
                done()
            }
            // -- `T061`: `7a`, a permission ask ------------------------------
            //
            // **Rated `Allow` on purpose**: an agent asking permission is the
            // agent behaving, and refusing the *asking* would leave it with
            // nothing to do but not ask. What is `Deny` is granting — the two
            // verbs below — which is the whole shape of consent.
            Action::Ask(AskAction::RequestPermission { invocation, files }) => {
                let (prose, verb) = permission_question(invocation, files);
                // **A rule that already permits it is not a question.** This is
                // the acceptance's second half — *"takes effect next time"* —
                // and it is checked here rather than by the caller, so a grant
                // written in a previous session is honoured by the same code
                // path that would have asked.
                if permitted(cx.shell.allowed.as_deref(), invocation) {
                    return Outcome::Done(Receipt {
                        capability: name,
                        value: Value::Null,
                        note: Some(format!("allowed by a rule — {verb}")),
                    });
                }
                let id = cx.shell.mint_ask();
                cx.shell.enqueue_ask(
                    id,
                    phosphor_ui::question::QuestionVm {
                        prose,
                        options: vec![
                            AskOption {
                                digit: 1,
                                label: "allow once".to_owned(),
                            },
                            AskOption {
                                digit: 2,
                                // **The rule, in the option's own label.** `7a`
                                // puts it in the footer; here it is the thing
                                // you are pressing, which is one fewer place to
                                // look and one fewer thing to keep in step.
                                label: format!("always allow {verb} — writes (allow \"{verb}\")"),
                            },
                            AskOption {
                                digit: 3,
                                label: "deny".to_owned(),
                            },
                        ],
                    },
                );
                cx.shell.asking_about.insert(id, verb);
                Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)),
                    note: None,
                })
            }
            // `7a`'s `[1]` and `[2]`. **The scope is the difference and the
            // only difference**: both let it run, and `Always` also writes the
            // rule that stops it being asked again.
            Action::Ask(AskAction::GrantPermission { ask, scope }) => {
                let Some(verb) = cx.shell.asking_about.remove(ask) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                let always = matches!(scope, phosphor_core::request::GrantScope::Always);
                if !cx.shell.answer_ask(*ask, Some(1), None) {
                    cx.shell.asking_about.insert(*ask, verb);
                    return Outcome::Refused(Refusal::NoSuchTarget);
                }
                // **Said in the permission's own words, after the answer.**
                // `Shell::answer_ask` writes `answered 1`, which is true of a
                // digit and useless about a grant: what you want to read back
                // is *what you just permitted*, and for `[2]` that it will hold
                // next time. Overwritten rather than suppressed, because the
                // answer really did happen.
                cx.shell.saying = Some(if always {
                    cx.shell.writing.push(verb.clone());
                    format!("allowing {verb} from now on")
                } else {
                    format!("allowed once — {verb}")
                });
                done()
            }
            Action::Ask(AskAction::DenyPermission { ask }) => {
                let Some(verb) = cx.shell.asking_about.remove(ask) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                if !cx.shell.answer_ask(*ask, Some(3), None) {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                }
                cx.shell.saying = Some(format!("refused — {verb}"));
                done()
            }
            // `T060`. **`esc later` — the third of `4a`'s three ways out**, and
            // the one the design is actually built around: *"you answer when
            // you get a chance — same philosophy as unseen."*
            Action::Ask(AskAction::DeferAsk { ask }) => {
                // Absent means the one on screen — which is `Shell::asked`, and
                // falls back to the head so `:defer` works from a keyboard with
                // no float up.
                let Some(which) = ask.or(cx.shell.asked).or_else(|| cx.shell.head_ask()) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                if !cx.shell.defer_ask(which) {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                }
                done()
            }
            Action::Ask(AskAction::AnswerAsk { ask, digit, prose }) => {
                if digit.is_none() && prose.is_none() {
                    return declined("no answer given — a digit or prose, or :defer");
                }
                if !cx.shell.answer_ask(*ask, *digit, prose.as_deref()) {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                }
                done()
            }
            // -- `T057`: the session's life ------------------------------------
            //
            // Seven verbs over one client. **None of them can block**, which is
            // the task's own emphasis — *"editing never blocks on session
            // trouble"* — and it is structural rather than careful:
            // `phosphor_agent::session::Session`'s every method returns without
            // waiting, so an arm here is a channel send and a state read.
            Action::Session(SessionAction::StartSession { agent, cwd }) => {
                let Some(spec) = agent::spec_from(agent) else {
                    return declined("no agent named — :cn <command>");
                };
                let root = cwd
                    .clone()
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                cx.shell.agent = Some(agent.clone());
                cx.shell.session.attach(spec, root);
                done()
            }
            // **The endpoint is a command**, and that is the honest reading at
            // this phase: ACP is spoken over a child's stdio, so *where a
            // session is* is the thing that starts one. A socket endpoint is a
            // second transport and `5d`'s `~/.claude/sock/4f2a` is what it
            // would be for — recorded on `discover-sessions` below, which is
            // where the gap actually bites.
            Action::Session(SessionAction::AttachSession { endpoint }) => {
                let Some(spec) = agent::spec_from(endpoint) else {
                    return declined("no endpoint named");
                };
                cx.shell.agent = Some(endpoint.clone());
                cx.shell
                    .session
                    .attach(spec, std::env::current_dir().unwrap_or_default());
                done()
            }
            // Nothing to adopt, because nothing is ever discovered. Refused by
            // *target* rather than by task: the verb is built and the handle is
            // the thing that does not exist.
            Action::Session(SessionAction::AdoptSession { .. }) => {
                Outcome::Refused(Refusal::NoSuchTarget)
            }
            // `:reattach` — `7b`'s remedy, and the reason `Failure::Dropped`'s
            // sentence names it. The spec is the one already attached, so this
            // needs no argument and cannot reattach to the wrong thing.
            Action::Session(SessionAction::ReattachSession {}) => {
                let Some(agent) = cx.shell.agent.clone() else {
                    return declined("no session to reattach — :cn <command>");
                };
                let Some(spec) = agent::spec_from(&agent) else {
                    return declined("no session to reattach — :cn <command>");
                };
                cx.shell
                    .session
                    .attach(spec, std::env::current_dir().unwrap_or_default());
                done()
            }
            // **Detach and end are the same call today, and the difference is
            // recorded rather than pretended.** Detaching is supposed to leave
            // the agent running; this client owns the child and `kill_on_drop`
            // is what stops the editor stranding one, so letting go of a stdio
            // child *is* ending it. The two stay separate verbs because the
            // distinction is the transport's, not the vocabulary's — a socket
            // endpoint would make it real without changing either row.
            Action::Session(SessionAction::DetachSession {}) => {
                cx.shell.session.stop();
                cx.shell.agent = None;
                Outcome::Done(Receipt {
                    capability: name,
                    value: Value::Null,
                    note: Some(
                        "the agent stops with the editor — a stdio child is ours".to_owned(),
                    ),
                })
            }
            Action::Session(SessionAction::EndSession { force }) => {
                // Mid-turn needs the `force`, which is what the row's `Ask`
                // policy is about one layer up: ending a turn that is running
                // throws away work claude has done.
                if !force && cx.shell.turn.is_some() {
                    return declined("a turn is running — :end-session! ends it anyway");
                }
                cx.shell.session.stop();
                cx.shell.agent = None;
                done()
            }
            // `7b`'s seam, recorded into the transcript where the turn is.
            Action::Session(SessionAction::SessionSeam { kind, note }) => {
                let Some((turn, _)) = cx.shell.turn else {
                    return declined("no turn to mark");
                };
                let said = note.clone().unwrap_or_else(|| {
                    match kind {
                        phosphor_core::request::SeamKind::Paused => "paused",
                        phosphor_core::request::SeamKind::Lost => "connection lost mid-turn",
                        phosphor_core::request::SeamKind::Resumed => "resumed",
                    }
                    .to_owned()
                });
                // **Only a lost seam is trouble.** A pause is a thing you did
                // and a resume is a thing that worked; painting either of them
                // §2's `✕` would make the transcript louder than the event.
                let trouble = matches!(kind, phosphor_core::request::SeamKind::Lost);
                let unseen = cx
                    .shell
                    .store
                    .unseen_count(&phosphor_core::store::Scope::Everywhere);
                cx.shell.transcript.at(turn).ended = Some(phosphor_ui::transcript::Seam {
                    text: said,
                    detail: trouble.then(|| survived(unseen)),
                    tone: if trouble {
                        phosphor_ui::transcript::SeamTone::Trouble
                    } else {
                        phosphor_ui::transcript::SeamTone::Paused
                    },
                });
                done()
            }

            // -- `T054`: the transcript ---------------------------------------
            //
            // Four producer verbs and one surface verb. All four are `Allow` —
            // an agent says what it is doing — and none of them touches a
            // buffer, which is why they are `Cx`'s the way `T050`'s two are.
            Action::Session(SessionAction::SessionProse { turn, chunk }) => {
                // **Appended, not replaced.** `session-prose` is *"a chunk"*,
                // and a chunk boundary is a fact about the wire rather than
                // about the paragraph — §8's *"streaming transcript text"* is
                // one of the three things allowed to animate, and it animates
                // by growing.
                cx.shell.transcript.at(*turn).prose.push_str(chunk);
                done()
            }
            Action::Session(SessionAction::ToolCallStarted {
                turn,
                call,
                verb,
                target,
                path,
                line,
            }) => {
                let arriving = phosphor_ui::transcript::ToolCall {
                    id: *call,
                    verb: verb.clone(),
                    target: target.clone(),
                    // **The URI is built here and not in the widget**, for
                    // the same reason every other resolved thing is: a
                    // widget crate cannot know a workspace root, and
                    // `file://` is a fact about this machine.
                    link: path
                        .as_deref()
                        .map(|at| jump_uri(&cx.shell.workspace, at, *line)),
                    notes: Vec::new(),
                    outcome: None,
                };
                // **`T062` — this is the tool boundary.** `esc` asked the turn
                // to stop at one, and *one* is here: the agent has said what it
                // is about to do and has not done it. The call is held rather
                // than recorded, so `7e`'s `▸ next:` row is the thing that was
                // caught rather than a description of it.
                //
                // **The seam is written here and not by the verb**, for `7b`'s
                // reason exactly: the pause is a fact about a moment the verb
                // cannot see, and a transcript whose honesty depended on the
                // asker guessing when to write it would not be one.
                if cx.shell.pausing && cx.shell.paused.is_none() {
                    cx.shell.pausing = false;
                    cx.shell.paused = Some((*turn, arriving));
                    let paused = cx.shell.transcript.at(*turn);
                    paused.next = cx.shell.paused.as_ref().map(|(_, call)| call.clone());
                    paused.ended = Some(phosphor_ui::transcript::Seam {
                        text: "paused at tool boundary · esc".to_owned(),
                        detail: Some(
                            "↵ steers and resumes · :resume carries on · :abort ends the turn"
                                .to_owned(),
                        ),
                        tone: phosphor_ui::transcript::SeamTone::Paused,
                    });
                    return done();
                }
                cx.shell.transcript.at(*turn).calls.push(arriving);
                done()
            }
            Action::Session(SessionAction::ToolCallProgress { call, note }) => {
                // **A call this transcript has never heard of is refused, not
                // invented.** The opposite choice is right for `session-prose`
                // — see `Transcript::at` — and wrong here: prose has a turn to
                // hang from and a progress line has only a call, so a made-up
                // one would be a row with no verb and no target, which is a
                // row that says nothing.
                // **A held call's progress is dropped, not refused** (`T062`).
                // `7e` pauses *before* the call runs, so an agent that has not
                // yet honoured the cancel — or has already sent its updates —
                // reports progress on something that did not happen. Refusing
                // put `acp: no such tool call` on the notice row of a screen
                // that was otherwise correct, which is the editor complaining
                // about its own decision.
                if cx
                    .shell
                    .paused
                    .as_ref()
                    .is_some_and(|(_, held)| held.id == *call)
                {
                    return done();
                }
                let Some(running) = cx.shell.transcript.call(*call) else {
                    return declined("no such tool call");
                };
                running.notes.push(note.clone());
                done()
            }
            Action::Session(SessionAction::ToolCallCompleted {
                call,
                summary,
                added,
                removed,
            }) => {
                // Held, not run — see `tool-call-progress` above.
                if cx
                    .shell
                    .paused
                    .as_ref()
                    .is_some_and(|(_, held)| held.id == *call)
                {
                    return done();
                }
                let Some(finished) = cx.shell.transcript.call(*call) else {
                    return declined("no such tool call");
                };
                finished.outcome = Some(phosphor_ui::transcript::Outcome {
                    summary: summary.clone(),
                    added: *added,
                    removed: *removed,
                });
                done()
            }
            // `:transcript` is this, *"not a separate capability"* — the row
            // says so. A pane is a place and what it holds is a field, so
            // showing the transcript is a write to that field rather than a
            // verb of its own, which is what keeps `T088`'s pane model one
            // model.
            Action::Pane(PaneAction::SetPaneContent { pane, kind }) => {
                let Some(at) = cx.panes.resolve(pane) else {
                    return Outcome::Refused(Refusal::NoSuchTarget);
                };
                let held = cx.buffer;
                let pane = cx.panes.at_mut(at);
                pane.holds = *kind;
                // **Going back needs a buffer to go back to.** A pane split
                // *as* a transcript never had one (`Pane::buffer` is `None` by
                // design there), so `:transcript buffer` on it would otherwise
                // draw `Node::Empty` — a pane that is neither the transcript
                // nor a file. The focused buffer is what a caller that named
                // none means.
                if matches!(kind, PaneKind::Buffer) && pane.buffer.is_none() {
                    pane.buffer = Some(held);
                }
                done()
            }

            // `T058`'s capability, armed here because `T050`'s *Done when* is
            // *"a session attaches and **a turn completes**"* and nothing can
            // complete a turn nobody can start. What `T058` owns is the
            // **line** — `1c`, the `⚓` anchor chip, ex-style history — and the
            // anchors below are the seam between the two.
            Action::Session(SessionAction::SendMessage { body, anchors }) => {
                if !anchors.is_empty() {
                    // **Refused rather than sent without them.** An anchored
                    // message whose anchor is silently dropped is worse than no
                    // anchored message: claude answers about the wrong thing
                    // and nothing on screen said the file and range went
                    // missing.
                    return Outcome::Refused(Refusal::NotYetImplemented { task: "T058" });
                }
                if body.trim().is_empty() {
                    return declined("nothing to say — :claude <message>");
                }
                cx.shell.session.prompt(body.clone());
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
        }
        // **The selection is cleared, not restored**, and this used to replay
        // `step.caret.selection`. Reported at a real terminal: *"when you undo
        // it leaves the selection in place"*.
        //
        // Recording the selection on the caret is right — an undo step is a
        // place, and where you were includes what was selected. Replaying it is
        // not: `u` returns the machine to normal mode, so a restored highlight
        // belongs to no mode. Nothing will clear it, because nothing thinks a
        // selection is open; the next `v` then extends from an anchor the
        // person cannot see, which is the same class of defect
        // `SelectRange`'s own containment note above records.
        //
        // Vim's `u` is normal mode, cursor at the change, no selection. The
        // journal keeps the field, so a future *"restore my visual selection"*
        // still has its data — what changed is only that the walk stops
        // painting it.
        self.editor.clear_selection();
        self.selection_from = None;
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

    /// A target as a file and range, for `1c`'s chip (`T058`).
    ///
    /// **Only the two focus-relative arms**, which is [`Editing::target_range`]'s
    /// own rule one layer up: everything else is the store's to resolve, and a
    /// chip naming a region or a hunk is `T068`'s and `T063`'s to draw. [`None`]
    /// for a buffer with no file — there is nothing to anchor *to*.
    fn file_span(&mut self, target: &Target) -> Option<FileSpan> {
        // **As a surface shows it, which is what `1c` draws** —
        // `src/retry.rs`, not `/private/tmp/…/retry.rs`. [`shown_path`] is the
        // same rule `T089`'s tab titles use, and its own doc says why it is not
        // `store::key_for`.
        let held = self.file.clone()?;
        let path = PathBuf::from(shown_path(
            &held,
            &std::env::current_dir().unwrap_or_default(),
        ));
        let (from, to) = self.target_range(target)?;
        // The fork's own offset-to-point conversion, which is what
        // `cursor_of` uses for the statusline — so a chip and the `12:1` beside
        // it cannot disagree about which line you are on. Both are 1-based on
        // screen and 0-based inside, converted in one place each.
        let at = |offset: usize| {
            let (row, col) = self.editor.code_ref().point(offset);
            Position {
                line: u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX),
                column: u32::try_from(col.saturating_add(1)).unwrap_or(u32::MAX),
            }
        };
        let (start, mut end) = (at(from), at(to));
        // **Half-open to inclusive, which is what a line range *means*.** A
        // line-wise selection of lines 2–4 ends at the offset that begins line
        // 5, so the raw conversion reads `2–5` and names a line nobody
        // selected. vim's `'<,'>` is inclusive and `1c` draws `19–21`; the chip
        // says what was selected.
        //
        // Only when the end sits at column 1 *and* there is a line to step
        // back to — a character-wise selection ending mid-line is already
        // inclusive of the character it covers.
        if end.column == 1 && end.line > start.line {
            end.line -= 1;
        }
        Some(FileSpan {
            path,
            span: Some(Span { start, end }),
        })
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

    /// A [`Target`] as a store [`Scope`] — **the resolution only this side can
    /// do.**
    ///
    /// [`Editing::target_range`]'s own doc has said since `T031` that
    /// everything past the two focus-relative arms is *"the store's to resolve
    /// (`T041`)"*. This is that resolution, and the split it lands on is the
    /// one `request.rs` already draws: an arm that means something different
    /// depending on where focus is needs an editor, and the editor is here.
    ///
    /// The arms that refuse say so in one sentence rather than guessing,
    /// because a target the store silently widened is how `S` over a group
    /// marks a file.
    fn scope_of(&mut self, target: &Target) -> Result<RegionScope, String> {
        match target {
            Target::File { path } => Ok(RegionScope::File(store::key_for(path))),
            Target::Explicit { path, span } => Ok(RegionScope::Span {
                path: store::key_for(path),
                span: *span,
            }),
            Target::Region { id } => Ok(RegionScope::One(*id)),
            // The three that mean "here", and the only three that need an
            // editor to say where that is.
            Target::Buffer { .. } | Target::Cursor {} | Target::Selection {} => {
                let Some(path) = self.file.clone() else {
                    return Err("no file open — name a path".to_owned());
                };
                let path = store::key_for(&path);
                if matches!(target, Target::Buffer { .. }) {
                    return Ok(RegionScope::File(path));
                }
                let Some((from, to)) = self.target_range(target) else {
                    return Err("nothing selected".to_owned());
                };
                Ok(RegionScope::Span {
                    path,
                    span: self.span_between(from, to),
                })
            }
            // Named by the vocabulary rather than by a list here, so an arm
            // added to `Target` cannot quietly refuse under the wrong word.
            other => Err(format!(
                "{} is not a target the store resolves yet",
                other.to_value().tag().unwrap_or("that")
            )),
        }
    }

    /// Two character offsets as the vocabulary's span. The inverse of
    /// [`Editing::target_range`], and the only place the loop crosses back.
    fn span_between(&self, from: usize, to: usize) -> Span {
        let point = |offset: usize| {
            let (row, col) = self.editor.code_ref().point(offset);
            Position {
                line: u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX),
                column: u32::try_from(col.saturating_add(1)).unwrap_or(u32::MAX),
            }
        };
        Span {
            start: point(from.min(to)),
            end: point(to.max(from)),
        }
    }

    /// **`mark-seen` and `mark-unseen`.** Answers how many regions were in
    /// scope, so `s` on a line with no region says `0` rather than nothing.
    ///
    /// # Zero is a receipt, not a refusal — `OPEN-QUESTIONS.md` §41
    ///
    /// The count stays the value: `(mark-seen! …)` answering a number is what
    /// makes it composable from a script, and a refusal would turn the ordinary
    /// case — `S` over a block that happens to be fully seen — into an error.
    ///
    /// **But zero on a keystroke had no sound at all.** A `Done` is not trouble
    /// (`phosphor_steel::answer::trouble`), so the ex line stayed empty and
    /// pressing `SPC u s` on a line with no region was indistinguishable from
    /// the key being unbound — which is the exact class of defect `CP-4`'s
    /// manual half kept finding, except that this one would have been *correct
    /// behaviour reading as a bug*. So the note carries a sentence when it
    /// marked none. The value a door sees is unchanged.
    fn mark(&mut self, cx: &mut Cx<'_>, target: &Target, state: SeenState) -> Outcome {
        let capability = match state {
            SeenState::Seen => "mark-seen",
            SeenState::Unseen => "mark-unseen",
        };
        match self.scope_of(target) {
            Ok(scope) => {
                let marked = cx.shell.store.set_seen(&scope, state);
                // §6's voice: say what is true, in the fewest words that are.
                // Not *"failed"* — nothing failed.
                let said = (marked == 0).then(|| "no region here".to_owned());
                // **On the receipt and on the notice row, and they are not the
                // same reader.** The receipt is what a door sees; the notice
                // row is what a *person* sees, and a `Done` never reaches it
                // (`answer::trouble` answers `None`), which is the whole reason
                // this key was silent. `Editing::note` is the row's own
                // channel and already carries the caveat-on-a-success case.
                self.note.clone_from(&said);
                Outcome::Done(Receipt {
                    capability,
                    value: Value::Int(i64::try_from(marked).unwrap_or(0)),
                    note: said,
                })
            }
            Err(why) => declined(&why),
        }
    }

    /// **`set-virtual-text-visible`.** `T041`'s owed arm.
    ///
    /// # Per owner, without touching the fork
    ///
    /// The fork's own toggle is one flag for the whole editor
    /// ([`virtual_text::set_visible`]), and this capability addresses a rail
    /// *by owning region* — which is the entire reason it waited for the
    /// store. The gap is closed above the fork rather than inside it: the host
    /// installs the row list every frame, so a collapsed owner's rows are
    /// simply not in the list it installs. A vendored patch would have been
    /// permanent and this is not, which is the standing rule for `vendor/`.
    ///
    /// A rail no region owns cannot be collapsed, and that is honest rather
    /// than a gap: there is nothing to name it by.
    fn collapse(&mut self, cx: &mut Cx<'_>, owner: &Target, on: bool) -> Outcome {
        let scope = match self.scope_of(owner) {
            Ok(scope) => scope,
            Err(why) => return declined(&why),
        };
        let owners = cx.shell.store.ids_in(&scope);
        if owners.is_empty() {
            return declined("no region there — a rail is collapsed by its owner");
        }
        for id in &owners {
            if on {
                self.collapsed.remove(id);
            } else {
                self.collapsed.insert(*id);
            }
        }
        Outcome::Done(Receipt {
            capability: "set-virtual-text-visible",
            value: Value::Int(i64::try_from(owners.len()).unwrap_or(0)),
            note: None,
        })
    }

    // -----------------------------------------------------------------------
    // Anchors — `T042`, `T043`
    // -----------------------------------------------------------------------

    /// The buffer as the store is allowed to see it: lines, each with the
    /// syntax path covering it.
    ///
    /// **This is the whole of the seam.** `phosphor-core` has no dependencies
    /// and will not be growing tree-sitter as its first; the fork keeps a tree
    /// current across every edit and `Code::syntax_path` (PHOSPHOR PATCH 12) is
    /// the read. Everything the ladder decides is decided over this, in core,
    /// which is what lets every tier test run without a parser.
    ///
    /// One `syntax_path` per line is linear in the file and deliberately not
    /// cached: reanchoring runs after a rewrite, never on the frame path, and
    /// `benches/anchor.rs` asserts the shape rather than a time.
    fn snapshot(&self) -> AnchorSnapshot {
        let code = self.editor.code_ref();
        let text = code.get_content();
        let mut snapshot = AnchorSnapshot::of(&text);
        for line in 0..snapshot.len() {
            let offset = code.line_to_char(line);
            let byte = code.char_to_byte(offset);
            let steps: Vec<AnchorStep> = code
                .syntax_path(byte)
                .into_iter()
                .map(|step| AnchorStep::new(step.kind, step.name))
                .collect();
            if !steps.is_empty() {
                snapshot = snapshot.with_syntax(line, steps);
            }
        }
        snapshot
    }

    /// **`goto-sequence`.** Walks a sequence of store rows (`T049`).
    ///
    /// Only [`Sequence::UnseenRegion`] has a store to walk. The other seven
    /// name what builds them rather than doing nothing quietly — `]!` needs
    /// `T060`'s ask queue, `]]` needs `T053`'s review blocks, and so on. That
    /// is `T098`'s rule reaching a motion: a bound key that cannot act says
    /// which task will let it.
    ///
    /// **Wraps**, because `]u` is *"the next one"* and a list you can fall off
    /// the end of makes the last region a dead end — vim's own `n` wraps for
    /// the same reason.
    fn goto_sequence(
        &mut self,
        cx: &mut Cx<'_>,
        sequence: Sequence,
        seek: Seek,
        filter: Option<&RegionFilter>,
    ) -> Outcome {
        let task = match sequence {
            Sequence::UnseenRegion => None,
            Sequence::Hunk => Some("T063"),
            Sequence::BlockFile => Some("T053"),
            Sequence::Diagnostic => Some("T085"),
            Sequence::Thread => Some("T068"),
            // `T060`. **A motion over the queue, answered here rather than by
            // walking a store of rows** — the sequences below this one are
            // spans in a file and this one is a float, so what *"the next one"*
            // means is which question comes back rather than where the cursor
            // goes. `Seek` is not consulted for the same reason: a queue you
            // pushed things onto has an order, and `[!` backwards through it
            // would be a second order to keep in step with the first.
            Sequence::Ask => {
                return match cx.shell.recall_ask() {
                    Some(recalled) => Outcome::Done(Receipt {
                        capability: "goto-sequence",
                        value: Value::Int(i64::try_from(recalled.0).unwrap_or(i64::MAX)),
                        note: None,
                    }),
                    // **Declines by name rather than doing nothing**, and the
                    // sentence distinguishes the two ways there is nothing to
                    // recall: an empty queue and a queue you have not pushed
                    // anything back from.
                    None if cx.shell.asks.is_empty() => declined("no questions waiting"),
                    None => declined("nothing pushed back — the question is already up"),
                };
            }
            Sequence::SearchMatch => Some("T058"),
            // A jumplist entry is an anchor and `jump` already walks them, so
            // this arm would be a second spelling of one behaviour.
            Sequence::Jump => return self.jump(cx, seek),
        };
        if let Some(task) = task {
            return Outcome::Refused(Refusal::NotYetImplemented { task });
        }
        let Some(path) = self.file.clone() else {
            return declined("no file open");
        };
        // The filter is honoured where it can be: an author narrows the set,
        // and §7 says only claude's writes make regions, so `you` is an empty
        // set rather than an error.
        let key = store::key_for(&path);
        let lens = Lens {
            author: filter.and_then(|filter| filter.author),
            unseen_only: true,
            // Narrowed to this file in the *lens* rather than by filtering the
            // answers, so the store does the work it is built for.
            within: RegionScope::File(key.clone()),
        };
        let mut lines: Vec<u32> = cx
            .shell
            .store
            .answer_regions(&lens)
            .iter()
            .filter_map(|value| region_line(value, &key))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        if lines.is_empty() {
            return declined("nothing unseen in this file");
        }
        let here = self.text(cx).cursor().line;
        let to = match seek {
            Seek::First => lines[0],
            Seek::Last => lines[lines.len() - 1],
            Seek::Next => lines
                .iter()
                .copied()
                .find(|line| *line > here)
                .unwrap_or(lines[0]),
            Seek::Prev => lines
                .iter()
                .copied()
                .rev()
                .find(|line| *line < here)
                .unwrap_or(lines[lines.len() - 1]),
        };
        self.push_jump(cx);
        let offset = self
            .editor
            .code_ref()
            .offset(usize::try_from(to.saturating_sub(1)).unwrap_or(0), 0);
        self.editor.set_cursor(offset);
        Outcome::Done(Receipt {
            capability: "goto-sequence",
            value: Value::Int(i64::from(to)),
            note: None,
        })
    }

    /// **`picker-accept`.** Opens the highlighted row's place (`T047`).
    ///
    /// # The row's *text* is the address, and that is the design
    ///
    /// A row is styled runs and nothing else — no hidden payload, no id
    /// alongside. So what a row means is what it *says*: the first run is an
    /// address, and the rest is annotation.
    ///
    /// The alternative is a parallel array of targets beside the rows, and it
    /// fails the moment a source is redefined at the REPL: the rows change and
    /// the shadow list does not. This cannot go out of step because there is
    /// only one thing.
    ///
    /// # Three spellings, because a place has three sizes
    ///
    /// **This used to say *"every source writes its first run as
    /// `path:line`"*, and that was false about the shipped layer.** `8a` draws
    /// `src/retry.rs:9` and `grep`, `unseen` and `references` all write it, but
    /// `3d`'s file rows are bare names — `src/main.rs`, `Cargo.toml` — and
    /// bare names are what `files` writes. So pressing `↵` on any row of the
    /// file picker declined with *"that row does not name a place — sources
    /// write `path:line` first"*, which is the invariant this comment asserted
    /// rather than one anything checked. Reported by Teej at a real terminal.
    ///
    /// All three are addresses and a file is a place, so all three open. The
    /// first two are [`Target`]'s own text spellings (`request.rs`'s
    /// `target_from_text`, the `text =` clause on its `wire_union!`):
    ///
    /// * `path:line:column` — a point, and the cursor lands on it. `gr` is why
    ///   this exists: a server answers references *with* columns, the
    ///   `references` source drew only the line, and the jump put you at column
    ///   1 of the right line. Nothing here had to change to fix it — the row is
    ///   the whole address, so widening the spelling widened every source that
    ///   has a column to write.
    /// * `path:line` — a line, and the cursor lands at its start. What `grep`
    ///   and `unseen` write, because a matched line and a region are lines
    ///   rather than points.
    /// * `path` — a whole file, which carries **no** position, so `open_at`
    ///   stays [`None`]. That is the difference doing real work rather than a
    ///   default standing in for one: a fresh buffer starts at the top anyway,
    ///   and accepting the file you already have open leaves the cursor where
    ///   you left it instead of yanking it to line 1.
    ///
    /// The order matters where a path could be read either way. The longest
    /// numeric spelling wins, so a file genuinely named `notes.txt:12` is
    /// unreachable here — an exchange nobody will make, against `8a` being the
    /// picker that exists to name lines.
    ///
    /// Nothing else is refused. A head that does not exist on disk is not this
    /// function's to judge: `open-file` already answers a missing path with
    /// *"new file"*, and having two places decide what a path means is how the
    /// two disagree.
    ///
    /// **`AcceptHow::Split` opens it in a new split**, which is Teej's ruling
    /// at `T088`'s entry: telescope's `<CR>` opens in the current window, `<C-v>`
    /// vertical, `<C-x>` horizontal. It declined with *"one pane until T088
    /// splits it"* and the vocabulary already agreed with the ruling —
    /// `AcceptHow::Open` is documented *"open it in the focused pane"* and
    /// `AcceptHow::Split` *"open it in a new split"* — so this adds no
    /// vocabulary, only the arm.
    ///
    /// **`toward` is the host's and not the Action's**, deliberately.
    /// `AcceptHow::Split` says *that* it splits and carries no direction, so
    /// the two picker keys are the two ways and a `picker-accept` arriving from
    /// Steel or a keymap takes the default. Widening the vocabulary to carry a
    /// direction would be answering a question nobody asked.
    ///
    /// `AcceptHow::Quickfix` still declines by naming its task: the quickfix
    /// list is *"drawn once and named in no task"* (`request.rs`) and building
    /// it here would be inventing a surface nobody has asked for.
    fn accept_picker(&mut self, cx: &mut Cx<'_>, how: AcceptHow, toward: Direction) -> Outcome {
        let Some(session) = cx.shell.picker.as_ref() else {
            return declined("no picker open");
        };
        let splitting = match how {
            AcceptHow::Open => false,
            AcceptHow::Split => true,
            AcceptHow::Quickfix => {
                return declined("no quickfix list — drawn in 8a, named in no task");
            }
        };
        let Some(text) = session.matcher.selected_text() else {
            return declined("no row selected");
        };
        // The first whitespace-separated token: `8a`'s rows are
        // `src/retry.rs:9  ● pub max_delay: Duration,` and `3d`'s are
        // `src/main.rs  ●2 unseen` — the address is the head of either, not the
        // whole line.
        let head = text.split_whitespace().next().unwrap_or_default();
        if head.is_empty() {
            return declined("that row names nothing");
        }
        // `path:line[:column]` if it reads as one, the whole file if it does
        // not. The
        // non-`Explicit` arms of `Target` are focus-relative words like
        // `cursor` that no source writes, so a row's head matching one is a
        // *file* with that name and is opened as one.
        let (path, at) = match Target::from_value(&Value::Text(head.to_owned())) {
            Ok(Target::Explicit { path, span }) => (path, Some(span.start)),
            Ok(_) | Err(_) => (PathBuf::from(head), None),
        };
        cx.shell.picker = None;
        if splitting {
            // The loop performs it: a split needs a *new* buffer and this arm
            // holds one out of the map it would have to mint into.
            cx.shell.splitting = Some((path, toward));
        } else {
            self.open = Some(path);
        }
        self.open_at = at;
        Outcome::Done(Receipt {
            capability: "picker-accept",
            value: Value::Text(head.to_owned()),
            note: None,
        })
    }

    /// Give the focused file's regions a way to find themselves again
    /// (`T043`).
    ///
    /// Only ever the *focused* file: this side describes what it has open, and
    /// a declaration naming some other path is left positional until something
    /// opens it. The door's half ([`AppHost::place_anchor`]'s neighbour) covers
    /// the rest by reading off disk.
    fn fingerprint_declared(&mut self, cx: &mut Cx<'_>) {
        let Some(path) = self.file.clone() else {
            return;
        };
        let snapshot = self.snapshot();
        cx.shell
            .store
            .fingerprint_regions(&store::key_for(&path), &snapshot);
    }

    /// A fingerprint of a 1-based line in the focused buffer.
    fn fingerprint(&self, line: u32) -> Fingerprint {
        let code = self.editor.code_ref();
        let index = usize::try_from(line.saturating_sub(1)).unwrap_or(0);
        let text = if index < code.len_lines() {
            code.line(index).to_string()
        } else {
            String::new()
        };
        let byte = code.char_to_byte(code.line_to_char(index.min(code.len_lines())));
        let syntax: Vec<AnchorStep> = code
            .syntax_path(byte)
            .into_iter()
            .map(|step| AnchorStep::new(step.kind, step.name))
            .collect();
        Fingerprint::new(syntax, &text, line)
    }

    /// **`place-anchor`.** Answers the id, which is what `m{a-z}` writes down.
    ///
    /// The label is vim's `a`–`z` and a caller's own naming through one
    /// mechanism, which is `place-anchor`'s own doc. Placing the same label
    /// twice in one file is one mark, not two — the rule lives in
    /// [`phosphor_core::store::Anchors::place`] so every door gets it.
    fn place_anchor(
        &mut self,
        cx: &mut Cx<'_>,
        target: &Target,
        label: Option<&String>,
    ) -> Outcome {
        let scope = match self.scope_of(target) {
            Ok(scope) => scope,
            Err(why) => return declined(&why),
        };
        let (path, span) = match scope {
            RegionScope::File(path) => {
                let line = self.text(cx).cursor().line;
                (path, self.line_span(line))
            }
            RegionScope::Span { path, span } => (path, span),
            RegionScope::One(_) | RegionScope::Everywhere => {
                return declined("anchor a place, not a region — name a path or a span");
            }
        };
        let fingerprint = self.fingerprint(span.start.line);
        let id = cx
            .shell
            .store
            .place_anchor(path, span, label.cloned(), fingerprint);
        Outcome::Done(Receipt {
            capability: "place-anchor",
            value: id.to_value(),
            note: None,
        })
    }

    /// The whole of a 1-based line, as a zero-width span at its start.
    fn line_span(&self, line: u32) -> Span {
        Span {
            start: Position { line, column: 1 },
            end: Position { line, column: 1 },
        }
    }

    /// **`goto-anchor`.** `'{a-z}` and `` `{a-z} `` read a mark back.
    ///
    /// # The label half is `T042`'s whole reason for touching the vocabulary
    ///
    /// `runtime/keymaps.scm` had `'` and `` ` `` bound to silence with the gap
    /// named in its own comment: *"`place-anchor` writes a `label` that
    /// `goto-anchor` cannot read — it takes an `AnchorId`, and no capability
    /// turns a label into one."* A keybinding is **data** (`input::table::Role`
    /// — *"nothing here is a closure"*), so it cannot look an id up before
    /// naming it, and a literal id baked into a keymap would be worse than
    /// silence. So the door learned the label, which is the only place the
    /// lookup can live and still be reachable from all three doors.
    ///
    /// `exact` is `` ` `` versus `'`, and it is in the vocabulary rather than
    /// in the keymap because both are legitimate asks from a script too.
    ///
    /// A [`Tier::Lost`](phosphor_core::store::Tier::Lost) anchor is declined by
    /// name rather than jumped to. It
    /// still holds its old span, and sending someone to a location the store
    /// knows is stale is the one behaviour worse than saying so.
    /// `record` says whether arriving here is itself a jump.
    ///
    /// **True for `` ` `` and `'`, false for `<C-o>` and `<C-i>`**, and the
    /// difference is not a nicety: [`Editing::push_jump`] truncates the forward
    /// half of the jumplist, so a walk that recorded itself would delete the
    /// list it is walking. It did — `jump` called this unconditionally, so the
    /// first `<C-o>` wiped every entry and pushed one, and `<C-i>` came back
    /// *"already at the newest jump"* with nowhere to go. Vim's rule is the
    /// same one: moving along the jumplist does not add to it.
    fn goto_anchor(
        &mut self,
        cx: &mut Cx<'_>,
        id: Option<AnchorId>,
        label: Option<&str>,
        exact: bool,
        record: bool,
    ) -> Outcome {
        let focused = self.file.as_deref().map(store::key_for);
        let found = match (id, label) {
            (Some(id), _) => cx.shell.store.anchor(id),
            (None, Some(label)) => {
                let Some(path) = focused.as_deref() else {
                    return declined("no file open — a mark is found in a file");
                };
                cx.shell.store.labelled(path, label)
            }
            (None, None) => return declined("name an anchor — an id, or a label"),
        };
        let Some(anchor) = found else {
            let why = label.map_or_else(
                || "no such anchor".to_owned(),
                |label| format!("no mark {label}"),
            );
            return declined(&why);
        };
        let id = anchor.id;
        if !anchor.tier.resolves() {
            return declined("that anchor is lost — the code it named is gone");
        }
        if focused.as_deref() != Some(anchor.path.as_path()) {
            return declined("that anchor is in another file — T056 opens it");
        }
        if record {
            self.push_jump(cx);
        }
        let line = usize::try_from(anchor.span.start.line.saturating_sub(1)).unwrap_or(0);
        // `'` is the line, `` ` `` is the column it was written at.
        let column = if exact {
            usize::try_from(anchor.span.start.column.saturating_sub(1)).unwrap_or(0)
        } else {
            0
        };
        let offset = self.editor.code_ref().offset(line, column);
        self.editor.set_cursor(offset);
        Outcome::Done(Receipt {
            capability: "goto-anchor",
            value: id.to_value(),
            note: None,
        })
    }

    /// **`reanchor`.** Re-resolves one file's anchors after a rewrite.
    ///
    /// Answers `{moved, held, lost}` rather than a bare count, because the
    /// three are acted on differently and a caller that only wanted a number
    /// can read one field.
    fn reanchor(&mut self, cx: &mut Cx<'_>, path: &Path) -> Outcome {
        let key = store::key_for(path);
        let focused = self.file.as_deref().map(store::key_for);
        if focused.as_deref() != Some(key.as_path()) {
            return declined("reanchor reads the focused buffer — open it first");
        }
        let snapshot = self.snapshot();
        let outcome = cx.shell.store.reanchor(&key, &snapshot);
        let note =
            (!outcome.lost.is_empty()).then(|| format!("{} anchor(s) lost", outcome.lost.len()));
        self.note.clone_from(&note);
        Outcome::Done(Receipt {
            capability: "reanchor",
            value: outcome.to_value(),
            note,
        })
    }

    /// **`jump`.** Walks the jumplist, whose entries are anchors — which is
    /// why this arm lands with `T042` rather than with the motions.
    ///
    /// `Seek::Prev` is `<C-o>` and `Seek::Next` is `<C-i>`. `First` and `Last`
    /// are the ends. An empty list declines rather than answering a no-op, so
    /// `<C-o>` in a fresh session says why nothing happened.
    ///
    /// # The first `<C-o>` records where it left, and that is not bookkeeping
    ///
    /// A list of jump *origins* has no entry for where you are standing when
    /// you start walking back, so `<C-i>` would have nothing to return to — you
    /// could go back and never come forward. Vim's answer is to add the current
    /// position the moment you first press `<C-o>`, and that is what the
    /// `at_present` branch does. Without it the walk is one-way, which is how
    /// this read before the key survey pressed it.
    fn jump(&mut self, cx: &mut Cx<'_>, seek: Seek) -> Outcome {
        if cx.view().jumplist.is_empty() {
            return declined("the jumplist is empty");
        }
        // At the present: not walking, so there is nowhere forward to go and a
        // step back has to leave a way home first.
        let at_present = cx.view().jump_at >= cx.view().jumplist.len();
        if at_present && matches!(seek, Seek::Next) {
            return declined("already at the newest jump");
        }
        if at_present && matches!(seek, Seek::Prev) {
            self.push_here(cx);
        }
        let last = cx.view().jumplist.len() - 1;
        let next = match seek {
            Seek::Prev => cx.view().jump_at.min(last).saturating_sub(1),
            Seek::Next => (cx.view().jump_at + 1).min(last),
            Seek::First => 0,
            Seek::Last => last,
        };
        if next == cx.view().jump_at && matches!(seek, Seek::Prev | Seek::Next) {
            return declined(match seek {
                Seek::Prev => "already at the oldest jump",
                _ => "already at the newest jump",
            });
        }
        cx.view_mut().jump_at = next;
        let Some(id) = cx.view().jumplist.get(next).copied() else {
            return declined("the jumplist moved under us");
        };
        match self.goto_anchor(cx, Some(id), None, true, false) {
            Outcome::Done(_) => Outcome::Done(Receipt {
                capability: "jump",
                value: Value::Int(i64::try_from(next).unwrap_or(0)),
                note: None,
            }),
            other => other,
        }
    }

    /// Record where the cursor is as a jumplist entry, before a jump moves it.
    ///
    /// Vim's rule: a jump remembers where you *came from*. Jumping after
    /// walking backwards truncates the forward half, which is what makes the
    /// list a history rather than a ring.
    fn push_jump(&mut self, cx: &mut Cx<'_>) {
        // **Truncated at `jump_at`, not after it.** Everything from where the
        // walk currently stands is unreachable once a new jump happens, and the
        // entry *at* `jump_at` is the position `push_here` is about to record —
        // keeping it would leave the same line in the list twice. At the
        // present this is a no-op, which is the ordinary case.
        let stop = cx.view().jump_at.min(cx.view().jumplist.len());
        cx.view_mut().jumplist.truncate(stop);
        self.push_here(cx);
        let present = cx.view().jumplist.len();
        cx.view_mut().jump_at = present;
    }

    /// Append the cursor's line to the jumplist, leaving `jump_at` alone.
    ///
    /// Two callers with two reasons: [`Editing::push_jump`], which is a jump
    /// recording where it came *from*, and [`Editing::jump`]'s first `<C-o>`,
    /// which is a walk recording where it is *leaving* so `<C-i>` has somewhere
    /// to return to.
    fn push_here(&mut self, cx: &mut Cx<'_>) {
        let Some(path) = self.file.clone() else {
            return;
        };
        let line = self.text(cx).cursor().line;
        let span = self.line_span(line);
        let fingerprint = self.fingerprint(line);
        let id = cx
            .shell
            .store
            .place_anchor(store::key_for(&path), span, None, fingerprint);
        cx.view_mut().jumplist.push(id);
    }

    fn yank(&mut self, cx: &mut Cx<'_>, target: &Target, register: Option<&RegisterName>) {
        let Some((from, to)) = self.target_range(target) else {
            return;
        };
        let text = self
            .editor
            .code_ref()
            .slice(from, to.min(self.editor.code_ref().len_chars()));
        let linewise = self.selection_kind == SelectionKind::Line;
        let name = register.map_or_else(|| UNNAMED.to_owned(), |name| name.0.clone());
        cx.shell.registers.insert(name, Register { text, linewise });
    }

    fn paste(&mut self, cx: &mut Cx<'_>, register: Option<&RegisterName>, before: bool) {
        let name = register.map_or_else(|| UNNAMED.to_owned(), |name| name.0.clone());
        let Some(register) = cx.shell.registers.get(&name).cloned() else {
            return;
        };
        let cursor = self.text(cx).cursor();
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
                let at = motion::end_of_line(&self.text(cx), cursor.line);
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
    fn anchor(&self, cx: &Cx<'_>, back: usize) -> Anchor {
        let Some((x, y)) = self.editor.get_visible_cursor(&cx.view().area) else {
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
    /// It is measured on [`Pane::area`] — the text area this buffer is drawn
    /// in — because that is what the float is capped against. A zero-width area
    /// (a buffer that has never been laid out) wraps to nothing and hands the
    /// lines back whole, which truncates exactly as before rather than looping.
    fn wrapped(&self, cx: &Cx<'_>, lines: &[String]) -> Vec<String> {
        float::wrap_prose(lines, float::anchored_wrap_cols(cx.view().area.width))
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
    fn completions(&self, cx: &Cx<'_>, items: &[WireCompletion]) -> CompletionVm {
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
                cx,
                &items
                    .first()
                    .map(|item| item.documentation.clone())
                    .unwrap_or_default(),
            ),
            anchor: self.anchor(cx, self.prefix_len()),
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
    fn fall_through(&mut self, cx: &mut Cx<'_>, binding: &Binding) -> Outcome {
        let (name, args) = match binding {
            Binding::Capability { name, args } => (name, args),
            Binding::Source { .. } => {
                return declined(
                    "a key's fall-through runs a capability, not scheme — \
                     use keymap-set! for a binding that evaluates source",
                );
            }
        };
        if cx.shell.falling_through {
            return declined("a fall-through may not fall through again");
        }
        let action = match Action::from_call(name, args) {
            Ok(action) => action,
            Err(error) => return declined(&error.to_string()),
        };
        cx.shell.falling_through = true;
        let outcome = self.act(cx, &action);
        cx.shell.falling_through = false;
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
    /// Put a different rope in this buffer, and answer the file that left.
    ///
    /// **The reset list, written down, because the unwritten one was already
    /// wrong.** The swap block in the loop rewrote `editor`, `timeline`,
    /// `depth`, `file`, the completion and the signature, and left two fields
    /// holding facts about a rope that is no longer here:
    ///
    /// * [`Editing::selection_from`] is a **char offset**, and offsets do not
    ///   survive a rope. `SelectRange` guards against a stale one by
    ///   containment — *"an anchor outside the range it is the anchor of is not
    ///   that range's anchor"* — but [`Editing::act`]'s `ExtendSelection` arm
    ///   does not; it reads `get_or_insert(head)` and takes whatever is there.
    ///   That arm is reachable straight after a swap because the *machine* is
    ///   the session's and its visual anchor outlives the buffer, so a motion
    ///   key extends from an offset measured in a file that is no longer open.
    /// * [`Editing::selection_kind`] is the second, and it is the one that made
    ///   the list worth naming rather than the bug worth patching. It drives
    ///   [`Editing::selected`]'s linewise widening and the yank's `linewise`
    ///   flag, and `ExtendSelection` reads it without ever setting it — so `V`
    ///   in the file you left makes the first extend in the file you arrived at
    ///   linewise.
    ///
    /// Both are facts about the departed rope in exactly the way `editor` and
    /// `timeline` are, which is the test this list is built on: if the value
    /// describes the text that just left, it resets here.
    ///
    /// **What deliberately does not reset**, because it is not the rope's:
    /// `registers` (vim's are global), `mode` (the machine's report, and the
    /// machine did not change), `source_order` and `collapsed` — and the pane's
    /// own four, which is what step 4a moved them for.
    ///
    /// This is not a fresh [`Editing`]. That form is right and is what step 8
    /// reaches for, once `Buffers` can hold the one being left behind; until
    /// then a swap is a rewrite and the list is how it stays honest.
    fn opens(&mut self, editor: Editor, file: PathBuf, timeline: Timeline) -> Option<PathBuf> {
        self.editor = editor;
        self.retrack();
        self.timeline = timeline;
        self.depth = 0;
        // A new buffer is a new place; a list anchored in the old one would be
        // drawn over code it knows nothing about.
        self.close_completion();
        self.signature = None;
        self.selection_from = None;
        self.selection_kind = SelectionKind::Char;
        let leaving = self.file.take();
        self.file = Some(file);
        leaving
    }

    /// Point this buffer's dirty flag and edit counter at its current rope.
    ///
    /// **A new [`Editor`] carries no change callback**, so a swapped-in rope
    /// would leave both frozen at whatever the last one made them — `[+]` on a
    /// buffer nobody has touched, and an edit counter that never moves again.
    ///
    /// It is a method rather than a call to [`track_dirty`] with the loop's two
    /// `Rc`s, and that is step 7's point: the loop's pair belongs to the buffer
    /// it booted with. Handing them to a *second* buffer's rope would have both
    /// buffers reporting one file's edits, which is the same shape of mistake
    /// as one `synced` for two open files.
    fn retrack(&mut self) {
        track_dirty(&mut self.editor, &self.dirty.clone(), &self.edits.clone());
    }

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
        cx: &mut Cx<'_>,
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
            let over = if cx.shell.mode == EditMode::Replace {
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
    fn insert_indent(&mut self, cx: &mut Cx<'_>) {
        let cursor = self.editor.get_cursor();
        let code = self.editor.code_ref();
        let line = code.char_to_line(cursor);
        let column = code.char_col_to_visual(line, cursor - code.line_to_char(line));
        let typed = self.indent_style.typed_at(column);
        let over = if cx.shell.mode == EditMode::Replace {
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
    fn join(&mut self, cx: &mut Cx<'_>, target: &Target) {
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
            let text = self.text(cx);
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
    /// The store and the file it is being read for (`T049`).
    ///
    /// Both, because a region is *"a path and a span"* and a store with no path
    /// to key on cannot answer *"the region under the cursor"*. [`None`] for a
    /// buffer with no file, which is the honest answer: a scratch buffer has no
    /// path a declaration could have named.
    regions: Option<(&'a store::Shared, PathBuf)>,
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
    /// `T049` — `viu`, over the same store the gutter draws from.
    ///
    /// The **lowest-id** region when more than one covers the cursor, which is
    /// `store::Shared::covering`'s rule and is here for its reason: the answer
    /// must not depend on how a set happened to be iterated, and the lowest id
    /// is the one a surface has been showing longest.
    ///
    /// Seen regions are excluded. `viu` is *"select the **unseen** region"* —
    /// `6d` draws it in the unseen list — and a noun that also selected regions
    /// you had already read would make `s` over it a no-op that looked like a
    /// bug.
    fn unseen_at(&self, at: Position) -> Option<Span> {
        let (store, path) = self.regions.as_ref()?;
        store.unseen_covering(path, at)
    }

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
    /// Everything the key moves that is not the buffer. A fifth borrow for the
    /// reason the other four are borrows: a keystroke moves a buffer, *and* the
    /// rectangle it is shown in, *and* the store the region verbs write to, and
    /// after step 4b those are three owners.
    cx: Cx<'a>,
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
                let text = self.editing.text(&self.cx);
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
                    if let Outcome::Refused(refusal) = self.editing.apply(&mut self.cx, &action)
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
                    InputAction::SetMode { mode } => self.cx.shell.mode = *mode,
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
                    // **`T099` — this arm used to be the refusal, and the
                    // refusal was the point.** `Machine::apply` returned nothing
                    // and its `set-macro-recording` arm was a no-op, so the host
                    // was the only place a `q` could decline by naming its task
                    // rather than succeeding silently. The recorder exists now;
                    // what is left here is keeping what it made.
                    //
                    // **The register store is the editor's, not the machine's.**
                    // `q` and `y` write the same thirty-odd slots, so a macro
                    // lands where a yank lands and `@a` and `"ap` read one
                    // table.
                    InputAction::SetMacroRecording { .. } => {
                        if let Some((name, keys)) = self.machine.take_recorded() {
                            self.cx.shell.registers.insert(
                                name,
                                Register {
                                    text: keys.0,
                                    // **Not linewise.** A macro is a key
                                    // sequence; pasting one with `p` should put
                                    // the characters where the cursor is rather
                                    // than opening a line for them.
                                    linewise: false,
                                },
                            );
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
fn deliver(editing: &mut Editing, cx: &mut Cx<'_>, posted: &events::Posted) -> Option<String> {
    let outcome = match posted.action.spec().mcp {
        McpPolicy::Allow => editing.act(cx, &posted.action),
        // **`T060` — the queue exists now, and this is what it was for.**
        // Neither applied nor dropped: an `Ask`-rated action becomes a
        // question, and answering `[1]` runs it. This line read
        // *"needs an ask first — T060 builds the queue"* for four windows, and
        // it was the honest answer while there was nowhere to put the question.
        //
        // **The action is held under the ask's id, not beside it.** Two servers
        // can each want a rename while you are reading something else, and a
        // single slot would let the second overwrite the first — which for a
        // rating whose whole point is consent is the worst possible failure.
        McpPolicy::Ask => {
            let id = cx.shell.mint_ask();
            let question = held_question(&posted.action, posted.source);
            cx.shell.enqueue_ask(id, question);
            cx.shell.held.insert(id, Box::new(posted.action.clone()));
            Outcome::Done(Receipt {
                capability: posted.action.spec().name,
                value: Value::Int(i64::try_from(id.0).unwrap_or(i64::MAX)),
                note: Some("queued as a question — answer it when you get a chance".to_owned()),
            })
        }
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
fn mouse_actions(
    machine: &mut Machine,
    editing: &Editing,
    cx: &Cx<'_>,
    mouse: MouseEvent,
) -> Vec<Action> {
    let editor = &editing.editor;
    // **The area is the pane's, and that is why it is a parameter again.**
    // It was a fourth parameter once, fed by `let area = editing.area;` on the
    // line above the call — two names for one `Copy` field across a seam, and
    // folding it into `editing` closed that. Step 4a reopens it on the other
    // side of the same argument: an area is a fact about the rectangle, not
    // about the rope, so there is now exactly one owner to read it from and no
    // second name to disagree with.
    let area = cx.view().area;
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
        MouseEventKind::Drag(MouseButton::Left) => at().map_or_else(Vec::new, |position| {
            machine.drag(position, &editing.text(cx))
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
                pane: PaneRef::Focused {},
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
    /// `2a`, `3d`, `8a` — the picker (`T045`). One widget over one source.
    Picker,
    /// A float a **door** opened: `open-float`, naming a surface the editor
    /// layer registered with `define-float-surface` (`T093`, §43).
    ///
    /// Separate from [`Surface::Help`] though the drawing is identical, because
    /// what is on screen is not the only thing a surface means: `esc` closes
    /// both, but `:help` is composed by the host from the live keymap and this
    /// one is composed by `runtime/*.scm` from whatever it likes. Collapsing
    /// them would make the first Steel surface indistinguishable from the one
    /// Rust surface that is not a fixture.
    Float,
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
/// become one. It read *"until the REPL is a pane (`T088`)"*; panes exist now
/// and the REPL is still a surface, because which of the two it should be is
/// `T054`'s question about the transcript rather than something the split tree
/// decided by existing.
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

/// What the prompt line has been asked to do (`T058`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PromptStep {
    /// `set-prompt-text` — replace what is typed.
    Set(String),
    /// `submit-prompt` — run it.
    Submit,
    /// `cancel-prompt` — close it, changing nothing.
    Cancel,
    /// `prompt-history` — walk back, or forward with a negative delta.
    History(i64),
}

/// What one key did to an open picker (`T045`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerStep {
    /// Still filtering.
    Typing,
    /// Close it.
    Close,
    /// `<tab>` — cycle to the next source (`T047`).
    Cycle(i64),
    /// `↵` — open the highlighted row in the focused pane (`T047`).
    Accept,
    /// `<C-v>` / `<C-x>` — open it in a new split, beside or below.
    ///
    /// **Two keys rather than one, and telescope's two.** A picker that split
    /// on every `↵` would make finding a file a window-management decision,
    /// which is the thing those defaults exist to avoid.
    Split(Direction),
}

/// One key, while the picker has the frame.
///
/// **The filter is edited here and nowhere else**, which is the half of
/// `phosphor_ui::picker`'s *"`ratatui-textarea` is deliberately absent"*
/// argument that is not prose: the string lives on the session, this is the
/// only writer, and `Node::Picker`'s prop is composed from it. A textarea would
/// be a second writer of the same string.
///
/// Backspacing off an empty filter closes, which is `ex_key`'s rule and for the
/// same reason — a surface you cannot back out of is a trap.
fn picker_key(key: KeyEvent, session: &mut PickerSession) -> PickerStep {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return PickerStep::Close,
        // `8a`'s tab, and `↵`. Routed out as steps rather than acted on here
        // because both are *capabilities* — `cycle-picker-source` and
        // `picker-accept` — and a second implementation beside the arms is the
        // thing `T033`'s "no second path from a command to the buffer" rules
        // out. The loop applies the Action.
        KeyCode::Tab => return PickerStep::Cycle(1),
        KeyCode::BackTab => return PickerStep::Cycle(-1),
        KeyCode::Enter => return PickerStep::Accept,
        KeyCode::Backspace => {
            if session.filter.pop().is_none() {
                return PickerStep::Close;
            }
            session.matcher.filter(&session.filter);
        }
        KeyCode::Char('u') if control => {
            session.filter.clear();
            session.matcher.filter("");
        }
        KeyCode::Char('c') if control => return PickerStep::Close,
        // Telescope's spelling: `<C-v>` puts it beside, `<C-x>` below.
        KeyCode::Char('v') if control => return PickerStep::Split(Direction::Right),
        KeyCode::Char('x') if control => return PickerStep::Split(Direction::Down),
        // `<C-n>` / `<C-p>` rather than `j` / `k`: the filter line owns every
        // printable key while it is open, so a letter is filter text and cannot
        // also be a motion.
        KeyCode::Char('n') if control => session.matcher.select(1),
        KeyCode::Char('p') if control => session.matcher.select(-1),
        KeyCode::Down => session.matcher.select(1),
        KeyCode::Up => session.matcher.select(-1),
        KeyCode::Char(character) if !control => {
            session.filter.push(character);
            session.matcher.filter(&session.filter);
        }
        _ => {}
    }
    PickerStep::Typing
}

/// Runs an ex line and answers what to say about it.
///
/// **The command's Actions go through `Editing::apply`, exactly as a key's
/// do.** There is no second path from a command to the buffer — which is what
/// makes `:write` and `SPC f s` the same thing said twice rather than two
/// implementations of saving.
fn submit_ex(
    layer: &mut Layer,
    editing: &mut Editing,
    cx: &mut Cx<'_>,
    line: &str,
) -> Option<String> {
    match layer.ex(line) {
        Ex::Ran => None,
        Ex::Run(actions) => actions
            .iter()
            .find_map(|action| phosphor_steel::answer::trouble(&editing.apply(cx, action))),
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

/// The picker as a float — `2a`'s screen (`T045`).
///
/// **Composed in Rust here and not in Steel**, which is the difference from
/// `T093`'s `open-float`: a picker is a *primitive* (`Node::Picker`), not a
/// custom surface built from the `spans` hatch, so composing it is naming one
/// node with the session's props. `T046`'s `define-picker-source` supplies the
/// rows that go through it, not the composition.
///
/// The header carries the source id because a picker with no rows and no header
/// is indistinguishable from a broken one — the same argument
/// [`help_float`] makes for refusing to open an empty grid, resolved the other
/// way because an empty *picker* is a legitimate state (nothing matched) and an
/// empty help grid is not.
fn picker_float(session: &PickerSession) -> phosphor_core::view::Float {
    phosphor_core::view::Float {
        header: Some(phosphor_core::view::FloatHeader {
            left: session.source.0.clone(),
            right: None,
        }),
        mood: Mood::Informational,
        body: Child::new(Node::Picker {
            source: session.source.clone(),
            filter: session.filter.clone(),
            // Empty, and deliberately: a source supplies styled *runs*, so
            // column widths are the source's own layout decision. `T046` and
            // `T047` are where they are spent.
            columns: Vec::new(),
            preview: session.preview,
        }),
        footer: None,
    }
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
fn adopt(editing: &mut Editing, languages: &Languages, servers: &LanguageServers) {
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
        editing.synced = None;
        return;
    };
    let path = lsp::absolute(&file);
    let root = lsp::attach(servers, languages, &language, &path);
    // Sent even when no server is running: the client records the text anyway,
    // which is what lets it convert a UTF-16 column if one attaches later
    // (`LanguageServers::open`).
    servers.open(&language, path.clone(), editing.contents());
    // **Recorded on the buffer rather than answered to the loop.** It was a
    // return value the loop stored in one local, which is the shape that
    // cannot hold two open files — see [`Editing::synced`].
    editing.synced = Some(Document {
        key: lsp::key_for(&path, root.as_deref()),
        path,
    });
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
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    use phosphor_core::action::{Action, Outcome, PaneAction, Refusal, Request, RuntimeAction};
    use phosphor_core::registry::Door;
    use phosphor_core::request::{
        Actor, BufferId, Direction, PaneId, PaneRef, Position, Severity, Span,
    };
    use phosphor_core::value::Value;
    use phosphor_steel::answer;
    use phosphor_steel::host::Host;
    use phosphor_ui::buffer_view::{Editor, StateMark};
    use phosphor_ui::interpret::Resources;
    use ratatui::layout::Rect;

    use super::door::Evaluate as _;
    use super::store;
    use phosphor_core::input::key::parse_seq;
    use phosphor_core::input::table::{Resolution, Role, Scope};
    use phosphor_core::input::text::Text as _;
    use phosphor_ui::float::{CompletionList, FloatBody as _, anchored_wrap_cols};

    use phosphor_core::language::Languages;
    use phosphor_core::request::LanguageId;

    use super::compose_tabs;
    use super::{
        AppHost, Asking, Buffers, COMPLETION_MIN_CHARS, COMPLETION_MIN_CHARS_DEFAULT, Caret, Cli,
        CommandFactory as _, Cx, EXPAND_TAB, Editing, EditorText, ExStep, FromArgMatches as _,
        IndentStyle, Intent, Key, Layer, Lookup, Machine, NodeId, Outstanding, Painted, Pane,
        PaneTree, Panes, Repl, ReplStep, Session, Shell, StatusVm, Surface, TAB_WIDTH, Table,
        Timeline, UndoTree, Vm, WireCompletion, boot, buffer, closes_surface, completion_floor,
        decode, deliver, door, ex_key, grammar_of, indent_style, is_press, repl_key, restored,
        seeding, server_chip, split, submit_ex, vm, wire_undo,
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

    /// The intents a *keystroke* asked for, with boot's own registrations
    /// dropped.
    ///
    /// `runtime/pickers.scm` registers the shipped sources and
    /// `runtime/arch.scm` its float surface, so booting posts a registration
    /// per definition before any key is pressed. A test about what a binding did has to say
    /// so; a test about what *boot* did (there are several, and they assert an
    /// empty list or a persisted layer's own ask) must not use this.
    fn after_boot(host: &AppHost) -> Vec<Intent> {
        host.intents()
            .into_iter()
            .filter(|intent| {
                !matches!(intent, Intent::DefineSource(..) | Intent::DefineSurface(..))
            })
            .collect()
    }

    /// The shipped layer, with nowhere to persist to: the host has no config
    /// home, so a persist is refused rather than writing into the
    /// repository's own runtime tree.
    fn booted() -> (Layer, Arc<AppHost>) {
        let host = Arc::new(AppHost::new(None));
        let runtime = boot(Some(&tree()), &host);
        // **Drained, because the loop drains.** Boot itself posts intents now
        // — `runtime/pickers.scm` registers the shipped sources with
        // `define-picker-source!` — and the real loop takes them on its first
        // pass. A test that boots and then asserts what a *keystroke* asked
        // for has to start from the same place, or every assertion about
        // intents becomes an assertion about how many things `init.scm`
        // happens to register.
        host.intents();
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

    /// One buffer in one pane, as the loop holds them — what the unit tests in
    /// this module drive.
    ///
    /// **A harness rather than two locals, and step 4b is the reason.** The
    /// context an Action is applied in is about to gain a `Shell` and then a
    /// map of panes; a test that says `editing.apply(&action)` does not change
    /// when that happens, and twenty-five that build the argument by hand all
    /// do. These tests are about `Editing`'s arms, not about the shape of the
    /// context those arms are handed.
    ///
    /// **It derefs to [`Editing`] on purpose**, so `editing.editor` and
    /// `editing.shell.registers` read the way they did and the diff that introduced
    /// the pane stays about the pane. The other half is deliberately *not*
    /// hidden: a test that means the rectangle says `editing.pane().area`, and
    /// one that means the rope says `editing.editor`. Test-only scaffolding —
    /// nothing on the shipping path derefs one type into another.
    #[derive(Debug)]
    struct Bench {
        editing: Editing,
        /// The panes, not *a* pane — step 6b put the map behind the context so
        /// a resolved `PaneRef` can name one that is not the Action's own.
        panes: Panes,
        focus: PaneId,
        shell: Shell,
    }

    impl std::ops::Deref for Bench {
        type Target = Editing;

        fn deref(&self) -> &Editing {
            &self.editing
        }
    }

    impl std::ops::DerefMut for Bench {
        fn deref_mut(&mut self) -> &mut Editing {
            &mut self.editing
        }
    }

    impl Bench {
        /// One Action, applied to this buffer in this pane.
        fn apply(&mut self, action: &Action) -> Outcome {
            let mut cx = Cx::new(self.buffer(), self.focus, &mut self.panes, &mut self.shell);
            self.editing.apply(&mut cx, action)
        }

        /// Which buffer this bench's pane holds.
        fn buffer(&self) -> BufferId {
            self.panes
                .at(self.focus)
                .buffer
                .expect("a bench's pane holds its buffer")
        }

        /// The focused pane, for a test that wants to read its area.
        fn pane(&self) -> &Pane {
            self.panes.at(self.focus)
        }

        /// The focused pane, for a test that wants to lay it out.
        fn pane_mut(&mut self) -> &mut Pane {
            self.panes.at_mut(self.focus)
        }

        /// One Action, applied without the reveal — see [`Editing::act`].
        fn act(&mut self, action: &Action) -> Outcome {
            let mut cx = Cx::new(self.buffer(), self.focus, &mut self.panes, &mut self.shell);
            self.editing.act(&mut cx, action)
        }

        /// The buffer as the machine reads it, in this pane.
        ///
        /// Built here rather than forwarded to [`Editing::text`], because that
        /// one takes the whole context and a context is two `&mut` borrows.
        /// A read of three fields should not have to claim them: forwarding
        /// made `editing.act(.. editing.text() ..)` a double borrow, and the
        /// argument is a *read* of where the cursor is.
        fn text(&self) -> EditorText<'_> {
            EditorText {
                editor: &self.editing.editor,
                height: self.panes.at(self.focus).area.height,
                regions: self
                    .editing
                    .file
                    .as_deref()
                    .map(|path| (&*self.shell.store, store::key_for(path))),
            }
        }

        /// The buffer and its context, borrowed apart — for the free functions
        /// that take both. `deliver` and `submit_ex` are the loop's own path,
        /// so a test of one calls it the way the loop does rather than through
        /// a wrapper only tests would have.
        fn split(&mut self) -> (&mut Editing, Cx<'_>) {
            let buffer = self.buffer();
            let focus = self.focus;
            (
                &mut self.editing,
                Cx::new(buffer, focus, &mut self.panes, &mut self.shell),
            )
        }
    }

    /// One `Editing` over `text` with the cursor at the end of it, laid out in
    /// a `width`-column area — the shape the completion and hover gates read.
    fn typed(text: &str, width: u16) -> Bench {
        let mut bench = editing(text);
        bench.pane_mut().area = Rect::new(0, 0, width, 24);
        bench.editor.set_cursor(text.chars().count());
        bench
    }

    /// One `Editing` over `text`, with nothing to save to.
    fn editing(text: &str) -> Bench {
        Bench {
            editing: Editing::new(
                buffer(
                    "text",
                    text,
                    &super::builtin("phosphor-dark").expect("a shipped theme"),
                )
                .expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        }
    }

    /// One pane over `BufferId(0)`, which is what the bench's buffer is.
    fn one_pane() -> Panes {
        let (panes, _) = Panes::new(Pane::new(BufferId(0)));
        panes
    }

    /// A session's own state, for a test that only needs somewhere for the
    /// region verbs to write. The wake is a no-op: these tests drive `tick`
    /// themselves and there is no loop to wake.
    fn shell() -> Shell {
        Shell {
            store: Arc::new(store::Shared::default()),
            asks: BTreeMap::new(),
            next_ask: Arc::new(std::sync::Mutex::new(1)),
            held: BTreeMap::new(),
            granted: Vec::new(),
            asking_about: BTreeMap::new(),
            writing: Vec::new(),
            allowed: None,
            edits: Vec::new(),
            steering: None,
            pausing: false,
            paused: None,
            deferred: std::collections::BTreeSet::new(),
            asked: None,
            workspace: PathBuf::new(),
            wake: Arc::new(|| {}),
            registers: BTreeMap::new(),
            picker: None,
            source_order: Vec::new(),
            mode: phosphor_core::request::EditMode::Normal,
            quit: false,
            discard: false,
            falling_through: false,
            wall: false,
            closing: None,
            splitting: None,
            // `T050`. Started and attached to nothing, which is what the
            // client's own contract calls for — the runtime thread is idle
            // until an `agent-command` names something, and no test here does.
            session: phosphor_agent::session::Session::start(
                Arc::new(|_| true),
                phosphor_agent::session::unwatched(),
            ),
            turn: None,
            agent: None,
            life: phosphor_agent::session::Life::None,
            transcript: super::Transcript::default(),
            told: 0,
            folded: Vec::new(),
            saying: None,
            prompt_step: None,
            history: Vec::new(),
            recalled: None,
            hinted: false,
            wanted: None,
        }
    }

    /// **`u` leaves a selection painted on screen.** Reported by Teej at a real
    /// terminal: *"when you undo it leaves the selection in place"*.
    ///
    /// [`Editing::walk`] restored `step.caret.selection` — the selection as it
    /// stood *before* the edit being undone. That is the right thing to
    /// **record** (an undo step is a place, and where you were includes what
    /// was selected) and the wrong thing to replay: the machine is in normal
    /// mode after `u`, so what a person sees is a highlight belonging to no
    /// mode, which no key will clear because nothing thinks a selection is
    /// open.
    ///
    /// Vim's `u` leaves you in normal mode with the cursor at the change and no
    /// selection. So the caret's selection stays in the journal — `T030`'s
    /// record is unchanged and a future *"restore my visual selection"* still
    /// has its data — and the walk stops painting it.
    #[test]
    fn undo_does_not_leave_a_selection_painted() {
        let mut editing = editing("alpha\nbravo\ncharlie\n");

        let span = Span {
            start: Position { line: 1, column: 1 },
            end: Position { line: 3, column: 1 },
        };
        // Select the first two lines and delete them, the way `v j d` does.
        editing.apply(&Action::Motion(
            phosphor_core::action::MotionAction::SelectRange {
                span,
                kind: phosphor_core::request::SelectionKind::Line,
            },
        ));
        editing.apply(&Action::Buffer(
            phosphor_core::action::BufferAction::Delete { span },
        ));
        editing.apply(&Action::History(
            phosphor_core::action::HistoryAction::CommitUndoGroup {},
        ));

        editing.apply(&Action::History(
            phosphor_core::action::HistoryAction::Undo { count: 1 },
        ));

        assert_eq!(
            editing.editor.get_selection(),
            None,
            "undo restores the text and the cursor, not a highlight the machine \
             has no mode for",
        );
        assert!(
            editing.selection_from.is_none(),
            "and the machine's own anchor goes with it, or the next `v` extends \
             from a selection nobody can see",
        );
    }

    /// **The second producer, reaching the buffer.** A posted event is applied
    /// by the same `Editing::act` a keystroke reaches — there is no second
    /// interpreter — so this is the whole of what the loop does with one.
    #[test]
    fn a_posted_action_lands_through_the_arm_a_key_would_reach() {
        use phosphor_core::input::text::Text as _;

        let mut editing = editing("hello");
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
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
        editing.pane_mut().area = Rect::new(0, 0, 80, 10);
        // The user's own turn at the one writer, so there is a viewport worth
        // not moving.
        let _ = editing.act(&Action::View(phosphor_core::action::ViewAction::Scroll {
            request: phosphor_core::request::ScrollRequest::RevealRow { row: 80, margin: 0 },
            pane: PaneRef::Focused {},
        }));
        let looking_at = editing.editor.get_offset_y();
        assert!(
            looking_at > 0,
            "this test is about a scrolled viewport and needs one"
        );

        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
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
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
            &super::events::Posted {
                source: "lsp",
                action: Action::App(phosphor_core::action::AppAction::Quit { force: true }),
            },
        );
        assert_eq!(
            note.as_deref(),
            Some("lsp: denied to a producer — only the keyboard asks for this"),
        );
        assert!(
            !editing.shell.quit,
            "a producer did not get to end the session"
        );
    }

    /// The middle rating, and the one an LSP client meets first: `T036`'s
    /// `apply-workspace-edit` is `Ask`, so a server's say-so is not enough.
    ///
    /// **This test asserted the refusal for four windows and now asserts the
    /// queue.** `deliver` answered *"needs an ask first — T060 builds the
    /// queue"*, which was the honest thing to say while there was nowhere to
    /// put the question. There is now: the action becomes a question, the
    /// question carries who asked and what for, and nothing is applied until
    /// you say so.
    #[test]
    fn a_posted_action_the_mcp_door_asks_about_becomes_a_question() {
        use phosphor_core::input::text::Text as _;

        let mut editing = editing("hello");
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
            &super::events::Posted {
                source: "lsp",
                action: Action::Lsp(phosphor_core::action::LspAction::ApplyWorkspaceEdit {
                    files: Vec::new(),
                }),
            },
        );
        assert_eq!(note, None, "a queued question is not trouble to report");
        assert_eq!(
            editing.text().line(1).as_deref(),
            Some("hello"),
            "and nothing was applied on the server's say-so"
        );

        let queued: Vec<_> = editing.shell.asks.values().collect();
        assert_eq!(queued.len(), 1, "the question is in the queue");
        // **It names who asked and what for.** *"Something wants permission"*
        // is not an answerable question; this is the same content `7a` takes
        // further with the exact invocation (`T061`).
        assert!(
            queued[0].prose.contains("lsp") && queued[0].prose.contains("edit"),
            "and says who wants what; prose was {:?}",
            queued[0].prose
        );
        assert_eq!(
            queued[0].options.len(),
            2,
            "with a yes and a no, which is the whole of the consent"
        );
        assert_eq!(
            editing.shell.held.len(),
            1,
            "and the action is held against the ask that is asking"
        );
    }

    /// **`T056`: a jump URI escapes what a URI reader would misread.**
    ///
    /// The half of §56 that was recorded rather than built. The `#` case is the
    /// one that actually bites: a file called `notes #2.md` produced a URI whose
    /// *fragment* began inside the filename, so the link opened `notes ` — a
    /// file that does not exist — and the line number was lost with it.
    #[test]
    fn a_jump_uri_escapes_the_bytes_a_uri_reader_would_take_for_syntax() {
        let root = Path::new("/w");
        // The separator survives; the space and the hash do not.
        assert_eq!(
            super::jump_uri(root, "/w/notes #2.md", Some(19)),
            "file:///w/notes%20%232.md#L19"
        );
        // `%` is escaped like anything else, which is what makes the encoding
        // unambiguous and non-idempotent.
        assert_eq!(
            super::jump_uri(root, "/w/50%.txt", None),
            "file:///w/50%25.txt"
        );
        // Non-ASCII is escaped per *byte* — a URI is bytes, and `é` is two.
        assert_eq!(
            super::jump_uri(root, "/w/café.rs", None),
            "file:///w/caf%C3%A9.rs"
        );
        // And an ordinary path is left completely alone, including the
        // characters the specification allows in a segment.
        assert_eq!(
            super::jump_uri(root, "/w/src/retry_v2-final.rs", Some(1)),
            "file:///w/src/retry_v2-final.rs#L1"
        );
    }

    /// **`T061`: a rule matches a verb, not a command line — and not a word it
    /// merely starts.**
    ///
    /// The whole reason `7a`'s always-allow is worth pressing: `(allow "git
    /// push")` has to cover `git push origin retry-backoff` or it is a rule
    /// that never applies twice. And it must not cover `gitleaks`, which is the
    /// difference between a prefix *rule* and a prefix *match* — the way an
    /// allow-list quietly grants more than it says.
    #[test]
    fn a_rule_covers_the_invocations_it_names_and_no_others() {
        let rules = Some("git push|cargo test");
        for covered in [
            "git push",
            "git push origin retry-backoff",
            "cargo test",
            "cargo test --workspace",
        ] {
            assert!(super::permitted(rules, covered), "{covered:?} is covered");
        }
        for not in [
            // The boundary. A rule is a whole verb.
            "gitleaks scan",
            "git pushx",
            // A different verb entirely.
            "git commit -am wip",
            "rm -rf /",
        ] {
            assert!(!super::permitted(rules, not), "{not:?} is not covered");
        }
        // And an empty list permits nothing, which is the state every session
        // starts in.
        assert!(
            !super::permitted(None, "git push"),
            "no rules, no permission"
        );
        assert!(
            !super::permitted(Some(""), "git push"),
            "and neither does an empty one"
        );
    }

    /// **Answering `[1]` releases what the question was about; `[2]` drops it.**
    ///
    /// The other half of the rating, and the half that makes it a mechanism
    /// rather than a screen. `Shell::granted` is what the loop runs.
    #[test]
    fn granting_a_held_question_releases_its_action_and_denying_drops_it() {
        for (digit, released) in [(1, 1), (2, 0)] {
            let mut editing = editing("hello");
            let (buffer, mut cx) = editing.split();
            drop(deliver(
                buffer,
                &mut cx,
                &super::events::Posted {
                    source: "lsp",
                    action: Action::Lsp(phosphor_core::action::LspAction::ApplyWorkspaceEdit {
                        files: Vec::new(),
                    }),
                },
            ));
            let asked = *editing
                .shell
                .asks
                .keys()
                .next()
                .expect("the question is queued");
            assert!(editing.shell.answer_ask(asked, Some(digit), None));
            assert_eq!(
                editing.shell.granted.len(),
                released,
                "digit {digit} releases {released} action(s)"
            );
            assert!(
                editing.shell.held.is_empty(),
                "and the question stops holding one either way"
            );
        }
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
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
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
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
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
        let held = editing.shell.store.diagnostics_of(&path);
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
        let (buffer, mut cx) = editing.split();
        let note = deliver(
            buffer,
            &mut cx,
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
        let mut editing = Bench {
            editing: Editing::new(
                buffer(
                    "text",
                    "text",
                    &super::builtin("phosphor-dark").expect("a shipped theme"),
                )
                .expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        assert!(!editing.shell.quit);
        let outcome = editing.apply(&Action::App(phosphor_core::action::AppAction::Quit {
            force: true,
        }));
        assert!(matches!(outcome, Outcome::Done(_)));
        assert!(editing.shell.quit, "the loop reads this once per turn");
    }

    #[test]
    fn a_quit_that_would_lose_work_is_refused_unless_forced() {
        let dirty = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut editing = Bench {
            editing: Editing::new(
                buffer(
                    "text",
                    "text",
                    &super::builtin("phosphor-dark").expect("a shipped theme"),
                )
                .expect("a buffer"),
                None,
                std::rc::Rc::clone(&dirty),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        // **Set after construction, not handed in.** Step 7 made every
        // `Editing` install its own change callback at birth, and installing
        // one clears the flag — a freshly tracked rope is clean, which is what
        // makes `[+]` mean *"different from what is on disk"* rather than
        // *"somebody passed true"*. Unsaved work is a state you reach, so the
        // test reaches it.
        dirty.set(true);
        let outcome = editing.apply(&Action::App(phosphor_core::action::AppAction::Quit {
            force: false,
        }));
        assert!(matches!(outcome, Outcome::Refused(Refusal::WouldLoseWork)));
        assert!(!editing.shell.quit);
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
        let mut editing = Bench {
            editing: Editing::new(
                buffer("text", "one\ntwo\nthree\nfour\nfive", &theme).expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        editing.pane_mut().area = Rect::new(0, 0, 80, 24);
        let mut machine = Machine::new();
        let mut seed = Table::new();
        let (mut layer, _host) = booted();

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('3'),
            KeyCode::Char('d'),
            KeyCode::Char('d'),
        ] {
            let (buffer, cx) = editing.split();
            Session {
                machine: &mut machine,
                layer: &mut layer,
                seed: &mut seed,
                editing: buffer,
                cx,
            }
            .key(pressed(code));
        }

        assert_eq!(editing.editor.get_content(), "one\nfive");
        assert_eq!(
            editing
                .shell
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
        let mut editing = Bench {
            editing: Editing::new(
                buffer("text", "bc", &theme).expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        editing.pane_mut().area = Rect::new(0, 0, 80, 24);
        let mut machine = Machine::new();
        let mut seed = Table::new();
        let (mut layer, _host) = booted();

        for code in [KeyCode::Char('i'), KeyCode::Char('a'), KeyCode::Esc] {
            let (buffer, cx) = editing.split();
            Session {
                machine: &mut machine,
                layer: &mut layer,
                seed: &mut seed,
                editing: buffer,
                cx,
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
        editing: Bench,
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
            let flag = std::rc::Rc::new(std::cell::Cell::new(false));
            let mut editing = Bench {
                shell: shell(),
                editing: Editing::new(
                    buffer("text", text, &theme).expect("a buffer"),
                    None,
                    std::rc::Rc::clone(&flag),
                ),
                panes: one_pane(),
                focus: PaneId(0),
            };
            // After construction: installing the change callback clears it, and
            // step 7 moved that installation into the constructor so no caller
            // has to remember it.
            flag.set(dirty);
            editing.pane_mut().area = Rect::new(0, 0, 80, 24);
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
                let (buffer, cx) = self.editing.split();
                Session {
                    machine: &mut self.machine,
                    layer: &mut self.layer,
                    seed: &mut self.seed,
                    editing: buffer,
                    cx,
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
        assert!(
            !typed.editing.shell.quit,
            "and it is still open to be saved in"
        );
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
        let mut editing = Bench {
            editing: Editing::new(
                buffer(
                    "rust",
                    "fn outer() {\n    let a = 1;\n    let b = 2;\n}\n",
                    &theme,
                )
                .expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        editing.pane_mut().area = Rect::new(0, 0, 80, 24);
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
        editing.pane_mut().area = Rect::new(0, 0, 80, 10);
        editing.shell.mode = phosphor_core::request::EditMode::Insert;
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
                .get_visible_cursor(&editing.pane().area)
                .is_some_and(|(_, y)| u32::from(y) < u32::from(editing.pane().area.height)),
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
        editing.pane_mut().area = Rect::new(0, 0, 20, 5);
        editing.editor.set_soft_wrap(Some(10));
        editing.shell.mode = phosphor_core::request::EditMode::Insert;
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
                .get_visible_cursor(&editing.pane().area)
                .is_some_and(|(_, y)| u32::from(y) < u32::from(editing.pane().area.height)),
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
            editing.pane_mut().area = Rect::new(0, 0, 80, 24);
            editing.shell.mode = mode;
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
        assert_eq!(after_boot(&host), vec![Intent::OpenRepl]);

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
        assert_eq!(after_boot(&host), vec![Intent::OpenRepl]);
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
        assert_eq!(after_boot(&host), vec![Intent::OpenRepl]);

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
        // **`T099`: `q` and `@` are prefixes, and their leaves are registers.**
        //
        // This asserted two weaker things in turn, and each was the truth of its
        // window: `q` resolved to a thunk that did nothing while the vocabulary
        // had no macro verb, then to a `Role::Run` that declined by naming
        // `T099`. Both are gone — the task landed, and a key that takes a
        // register is a prefix over twenty-six of them, exactly as `m` is.
        //
        // **`Pending` is the assertion.** A prefix answering anything else is a
        // key that swallowed the register you were about to name.
        assert_eq!(
            resolved(&mut layer, "q"),
            Resolution::Pending,
            "`q` waits for a register the way `m` waits for a mark"
        );
        assert_eq!(
            resolved(&mut layer, "@"),
            Resolution::Pending,
            "and so does `@`"
        );
        // And the leaves are real. `@a` reads the register at press time, which
        // is why it is a thunk answering a role rather than a stored one — see
        // `phosphor/resolve`.
        assert!(
            matches!(resolved(&mut layer, "qa"), Resolution::Role(Role::Run(_))),
            "`qa` records into a"
        );
        assert!(
            matches!(resolved(&mut layer, "@a"), Resolution::Role(Role::Run(_))),
            "`@a` plays it back"
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
        let mut editing = Bench {
            editing: Editing::new(
                buffer("text", "", &theme).expect("a buffer"),
                None,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
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
        // **A message to claude opens the same line**, which is what `T058`
        // built: `1c` is a prompt with a chip, and the chip is the *anchor*
        // rather than the kind. This asserted a refusal naming `T058` until
        // `T058` was the task doing the asserting.
        assert!(
            matches!(
                editing.apply(&open(phosphor_core::request::PromptKind::Claude)),
                Outcome::Done(_)
            ),
            "a message to claude raises the prompt line too"
        );
        assert_eq!(
            editing.prompt,
            Some(phosphor_core::request::PromptKind::Claude),
        );
        // Search is the half `T058` did not build: a search prompt needs
        // somewhere to search, which is the search machinery and not the line.
        let Outcome::Refused(Refusal::Declined { reason }) =
            editing.apply(&open(phosphor_core::request::PromptKind::Search))
        else {
            panic!("search has no machinery behind it yet");
        };
        assert!(reason.contains("T058"), "{reason}");
    }

    /// **Two panes over one buffer keep two jumplists**, which is the whole of
    /// what step 4a moved and the one thing a rename would not have bought.
    ///
    /// Vim's rule, and the reason the field could not stay on `Editing`:
    /// *"Each window has a separate jump list"* (`:help jumplist`). `<C-o>` in
    /// a split walks back through where **that split** has been. With the list
    /// on the buffer, opening the same file twice gave you one history shared
    /// between two cursors, and jumping in either would move the other's idea
    /// of where it had come from.
    ///
    /// This is testable before any UI exists because `Pane` is a plain struct:
    /// the binary makes one and a test can make two. That is what step 4c's
    /// maps are for, a step early.
    #[test]
    fn two_panes_over_one_buffer_keep_two_jumplists() {
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let file = scratch("jumplists").join("split.txt");
        std::fs::write(&file, "one\ntwo\nthree\n").expect("a file to anchor into");
        let mut editing = Editing::new(
            buffer("text", "one\ntwo\nthree\n", &theme).expect("a buffer"),
            Some(file),
            std::rc::Rc::new(std::cell::Cell::new(false)),
        );

        let mut shell = shell();
        let (mut panes, left) = Panes::new(Pane::new(BufferId(0)));
        let right = panes
            .split(left, Pane::new(BufferId(0)), Direction::Right)
            .expect("the left pane splits");

        editing.push_jump(&mut Cx::new(BufferId(0), left, &mut panes, &mut shell));
        editing.push_jump(&mut Cx::new(BufferId(0), left, &mut panes, &mut shell));

        assert_eq!(
            panes.at(left).jumplist.len(),
            2,
            "the pane the jumps were made in records them"
        );
        assert_eq!(
            panes.at(left).jump_at,
            2,
            "and stands at the present, which is `len`"
        );
        assert!(
            panes.at(right).jumplist.is_empty(),
            "the other pane showing the same buffer has been nowhere — this is \
             the assertion that fails the moment the list goes back on `Editing`"
        );
        assert_eq!(panes.at(right).jump_at, 0);
    }

    /// A tree of `n` panes in a row, and their ids left to right.
    fn row(n: usize) -> (PaneTree, Vec<PaneId>) {
        let mut tree = PaneTree::Leaf(PaneId(0));
        let mut ids = vec![PaneId(0)];
        for step in 1..n {
            let fresh = PaneId(u64::try_from(step).expect("a small count"));
            assert!(
                tree.split(ids[step - 1], fresh, Direction::Right),
                "the tree splits at a leaf it contains"
            );
            ids.push(fresh);
        }
        (tree, ids)
    }

    /// **The door tells two buffers apart**, which is the whole of step 11b.
    ///
    /// `Resources::editor` answered `Some(the one editor)` for every id, and
    /// `state_marks` answered the same column — both under a doc reading *"one
    /// buffer, and it is implicit"*, which was honest while there was one. A
    /// second pane showing a second file would have drawn the focused file's
    /// text *and* its error markers beside the other file's name.
    ///
    /// The `None` arms matter as much as the `Some` ones: `query.rs`'s rule is
    /// *"an absent thing answers empty"*, so a composition naming a buffer this
    /// host does not have must draw nothing rather than draw the wrong one.
    #[test]
    fn the_resources_door_answers_per_buffer_and_empty_for_an_id_it_lacks() {
        let alpha = editing("alpha").editing;
        let bravo = editing("bravo\nbravo").editing;
        let (a, b) = (BufferId(0), BufferId(1));

        let editors: BTreeMap<BufferId, &Editor> =
            BTreeMap::from([(a, &alpha.editor), (b, &bravo.editor)]);
        let columns: BTreeMap<BufferId, Vec<StateMark>> = BTreeMap::from([
            (a, vec![StateMark::ClaudeUnseen]),
            (b, vec![StateMark::None, StateMark::Trouble]),
        ]);
        let painted = Painted {
            editors: &editors,
            columns: &columns,
            completion: None,
            signature: None,
            picker: None,
            transcript: None,
            asks: &BTreeMap::new(),
        };

        assert_eq!(
            painted.editor(a).map(Editor::get_content).as_deref(),
            Some("alpha"),
            "an id resolves to its own buffer"
        );
        assert_eq!(
            painted.editor(b).map(Editor::get_content).as_deref(),
            Some("bravo\nbravo"),
            "and the other id to the other one — the assertion that fails while \
             every id resolves to whatever is on screen"
        );
        assert!(
            painted.editor(BufferId(9)).is_none(),
            "an id this host does not have draws nothing"
        );

        assert_eq!(painted.state_marks(a), &[StateMark::ClaudeUnseen]);
        assert_eq!(
            painted.state_marks(b),
            &[StateMark::None, StateMark::Trouble],
            "each buffer's markers are its own, so one pane cannot draw \
             another file's errors beside its text"
        );
        assert!(
            painted.state_marks(BufferId(9)).is_empty(),
            "and an absent thing answers empty rather than borrowing a column"
        );
    }

    /// **Two panes compose to a `Node::Split` holding two `Node::Pane`s, and
    /// exactly one is focused.**
    ///
    /// §9's rule is *"panes never dim each other — only floats dim what is
    /// behind them"*, so `focused` says which pane keystrokes go to and says
    /// nothing about brightness. Exactly one being true is what makes that
    /// sentence checkable.
    ///
    /// The second slot is `Fill`, not `Percent { 100 - n }`: two children that
    /// each round their own share leave a column nothing owns at odd widths,
    /// which is the failure `PaneTree::layout` avoids the same way.
    #[test]
    fn two_panes_compose_to_a_split_with_one_of_them_focused() {
        let (mut panes, left) = Panes::new(Pane::new(BufferId(0)));
        let right = panes
            .split(left, Pane::new(BufferId(1)), Direction::Right)
            .expect("the pane splits");
        panes.focus = right;

        let tree = panes.tree.clone();
        let phosphor_core::view::Node::Split { axis, slots } =
            crate::compose_panes(&tree, &panes, right, false, &[])
        else {
            panic!("two panes are a split");
        };

        assert_eq!(axis, phosphor_core::view::Axis::Columns);
        assert_eq!(slots.len(), 2);
        assert_eq!(
            slots[0].constraint,
            phosphor_core::view::Constraint::Percent { percent: 50 }
        );
        assert_eq!(
            slots[1].constraint,
            phosphor_core::view::Constraint::Fill { weight: 1 },
            "the second takes what the first left, rather than rounding its own \
             share and leaving a column nobody owns"
        );

        let focused: Vec<_> = slots
            .iter()
            .map(|slot| match slot.child.node() {
                phosphor_core::view::Node::Pane { pane, focused, .. } => (*pane, *focused),
                other => panic!("a slot holds a pane, not {other:?}"),
            })
            .collect();
        assert_eq!(
            focused,
            vec![(left, false), (right, true)],
            "one pane is focused and it is the one with focus — §9's rule is \
             about keystrokes, and panes never dim each other"
        );
    }

    /// **Opening then closing a float returns focus exactly where it was**, and
    /// it needs nothing to remember it — `T088`'s acceptance criterion, proven
    /// where it is exact.
    ///
    /// **The plan proposed a focus-return stack** so the return would be state
    /// rather than luck. It is neither: a float is not a pane. Not one of the
    /// three float verbs carries a `PaneRef`, so none of them *can* name a pane,
    /// and a verb that cannot name a pane cannot move focus to one. That is
    /// what this asserts — over the registry, which is the one description of a
    /// capability there is.
    ///
    /// **A stack would have been wrong as well as unnecessary.** If something
    /// did move focus while the float was open — `focus-pane` from a keymap or
    /// an agent — snapping back would undo what was asked for.
    ///
    /// The contrast is the assertion's other half: the four pane verbs *do*
    /// name a pane, every one of them. A test that only checked the floats
    /// would pass against a registry where nothing named anything.
    #[test]
    fn no_float_verb_can_name_a_pane_so_none_can_move_focus() {
        use phosphor_core::registry::ParamType;

        // **A `PaneRef`, not a `PaneId`** — the pane verbs take *which pane*
        // as a reference (`focused`, an id, a direction, next/prev), which is a
        // union rather than a bare id. What they have in common is the argument
        // itself, and a verb with no such argument has no way to say which pane
        // it means.
        let names_a_pane = |action: &Action| {
            action
                .spec()
                .params
                .iter()
                .any(|param| param.name == "pane" && matches!(param.ty, ParamType::Union(_)))
        };

        for float in [
            Action::Float(phosphor_core::action::FloatAction::CloseFloat {}),
            Action::Float(phosphor_core::action::FloatAction::CloseAllFloats {}),
        ] {
            assert!(
                !names_a_pane(&float),
                "{} names a pane, so it could move focus — which is what makes \
                 `opening then closing a float returns focus exactly where it \
                 was` need no state to hold it",
                float.spec().name
            );
        }

        for pane in [
            Action::Pane(PaneAction::FocusPane {
                pane: PaneRef::Focused {},
            }),
            Action::Pane(PaneAction::ClosePane {
                pane: PaneRef::Focused {},
            }),
        ] {
            assert!(
                names_a_pane(&pane),
                "{} is a pane verb and names one — without this the float half \
                 would pass against a registry where nothing named anything",
                pane.spec().name
            );
        }
    }

    /// **The `panes` query answers the tree, the focus and what each pane
    /// holds** — `T088`'s fifth acceptance clause.
    ///
    /// Plain data rather than a view tree: the query's own row says *"the pane
    /// tree, with which one has focus"*, so this answers what the arrangement
    /// **is**, and a `Node` would answer how to draw one.
    ///
    /// The split is between the two structures on purpose. A `PaneTree` knows
    /// arrangement and a `Pane` knows contents, so the tree describes the
    /// shape and the `panes` list describes what is in each leaf — and neither
    /// has to learn the other's job to answer.
    #[test]
    fn the_panes_query_describes_the_tree_the_focus_and_the_contents() {
        let (mut panes, left) = Panes::new(Pane::new(BufferId(0)));
        let right = panes
            .split(left, Pane::new(BufferId(1)), Direction::Right)
            .expect("the pane splits");
        panes.focus = right;

        let Value::Record(shape) = panes.describe() else {
            panic!("the shape is a record");
        };

        assert_eq!(
            shape.get("focus"),
            Some(&Value::Int(i64::try_from(right.0).expect("a small id"))),
            "which one has focus, by id"
        );

        let Some(Value::Record(tree)) = shape.get("tree") else {
            panic!("two panes are a split");
        };
        assert_eq!(tree.get("axis"), Some(&Value::Text("columns".to_owned())));
        assert_eq!(tree.get("share"), Some(&Value::Int(50)));

        let Some(Value::List(leaves)) = shape.get("panes") else {
            panic!("the panes are a list");
        };
        assert_eq!(leaves.len(), 2, "in the tree's order, left to right");
        let Value::Record(first) = &leaves[0] else {
            panic!("each is a record");
        };
        assert_eq!(
            first.get("pane"),
            Some(&Value::Int(i64::try_from(left.0).expect("a small id")))
        );
        assert_eq!(first.get("holds"), Some(&Value::Text("buffer".to_owned())));
        assert_eq!(
            first.get("buffer"),
            Some(&Value::Int(0)),
            "and which buffer is in it — the half the tree cannot say"
        );
    }

    /// **`T089`: the strip is composed at two panes and is `Node::Empty` at
    /// one**, in the tree's own left-to-right order.
    ///
    /// The composition half of §5's *"only with 2+ panes"*.
    /// [`Geometry::take_tab_bar`] is the other half and is tested beside it;
    /// the pty test presses the keys and proves both together. This one exists
    /// because the two halves fail differently — a composition that named two
    /// tabs at one pane would draw nothing (the geometry gives it no row) while
    /// telling every future reader of the tree that there were two panes.
    #[test]
    fn the_tab_bar_is_composed_at_two_panes_and_empty_at_one() {
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let directory = scratch("tabs");
        let make = |name: &str, text: &str| {
            Editing::new(
                buffer("text", text, &theme).expect("a buffer"),
                Some(directory.join(name)),
                std::rc::Rc::new(std::cell::Cell::new(false)),
            )
        };
        let (mut buffers, first) = Buffers::new(make("left.txt", "one\n"));
        let second = buffers.open(make("right.txt", "two\n"));

        let (mut panes, left) = Panes::new(Pane::new(first));
        let unseen = BTreeMap::from([(first, 3), (second, 0)]);

        assert_eq!(
            compose_tabs(&panes, &buffers, &unseen),
            super::Node::Empty {},
            "one pane is no strip — §5's condition, said where the tree is built"
        );

        let mut right = Pane::new(second);
        right.buffer = Some(second);
        let right = panes
            .split(left, right, Direction::Right)
            .expect("the pane splits");
        panes.focus = right;

        let super::Node::TabBar { tabs } = compose_tabs(&panes, &buffers, &unseen) else {
            panic!("two panes are a strip");
        };
        assert_eq!(tabs.len(), 2, "one tab per pane, in the tree's order");
        assert_eq!(tabs[0].unseen, 3, "the store's count for the left buffer");
        assert!(!tabs[0].active, "and focus is on the right");
        assert_eq!(tabs[1].unseen, 0);
        assert!(tabs[1].active);
        assert_eq!(tabs[0].kind, phosphor_core::request::PaneKind::Buffer);
        // The titles are the paths as the workspace spells them — these are
        // under a scratch directory rather than the cwd, so the rule that
        // applies is the basename fallback.
        assert_eq!(tabs[0].title, "left.txt");
        assert_eq!(tabs[1].title, "right.txt");
    }

    /// **The row the strip takes is the panes', and it is only ever taken when
    /// there is a second pane to name.**
    ///
    /// The other half of the condition, and the half that costs something: a
    /// row spent on a strip that draws nothing is a line of the buffer gone
    /// with nothing on screen to say so. **Measured** — the first draft of the
    /// pty test watched only for the word `panes`, and a `take_tab_bar` that
    /// spent the row at one pane passed it.
    ///
    /// It comes off `body` as well as `pane`, which the two bottom strips do
    /// not — see [`Geometry::tabs`] for why that difference is about *when* the
    /// condition is knowable rather than about the strip.
    #[test]
    fn the_tab_bar_row_comes_off_the_panes_and_only_at_two_panes() {
        let full = crate::lay_out(Rect::new(0, 0, 80, 24));

        let mut alone = full;
        alone.take_tab_bar(1);
        assert_eq!(alone.tabs, None, "no strip, and no row spent");
        assert_eq!(alone.pane, full.pane);
        assert_eq!(alone.body, full.body);

        let mut split = full;
        split.take_tab_bar(2);
        let strip = split.tabs.expect("two panes are a strip");
        assert_eq!(strip.height, 1, "§8: the tab bar is one row");
        assert_eq!(strip.y, full.pane.y, "off the top, not the bottom");
        assert_eq!(split.pane.y, full.pane.y + 1);
        assert_eq!(split.pane.height, full.pane.height - 1);
        assert_eq!(
            (split.body.y, split.body.height),
            (full.body.y + 1, full.body.height - 1),
            "and off `body` too, so the wrap width is measured against rows the \
             strip has already taken"
        );
        assert_eq!(split.status, full.status, "the statusline is untouched");

        // §11 in the other direction: a terminal with no room for the strip
        // keeps its buffer rather than drawing a strip over the last line.
        let mut cramped = crate::lay_out(Rect::new(0, 0, 80, 2));
        let before = cramped;
        cramped.take_tab_bar(2);
        assert_eq!(cramped.tabs, None);
        assert_eq!(cramped.pane, before.pane, "and gives nothing away");
    }

    /// **The panes tile the frame exactly, at any width.**
    ///
    /// The far side of a divider takes what the near side left, rather than
    /// both rounding their own share. Two halves that each computed
    /// `width * share / 100` would leave a one-column gap at odd widths — and a
    /// gap is a column nothing owns, nothing draws and nothing clears.
    #[test]
    fn a_layout_tiles_the_frame_with_no_gap_and_no_overlap() {
        let frame = |width: u16, height: u16| Rect::new(0, 0, width, height);

        for width in [80u16, 81, 1, 0, 3] {
            let (tree, ids) = row(3);
            let placed = tree.layout(frame(width, 24));

            assert_eq!(placed.len(), 3, "every leaf is placed");
            assert_eq!(
                placed.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                ids,
                "in the tree's own order, which is left to right"
            );
            assert_eq!(
                placed
                    .iter()
                    .map(|(_, at)| u32::from(at.width))
                    .sum::<u32>(),
                u32::from(width),
                "the widths add up to the frame at width {width} — the \
                 assertion that fails the moment each side rounds its own share"
            );

            // Walking left to right, each pane starts exactly where the last
            // one ended.
            let mut edge = 0u16;
            for (id, at) in &placed {
                assert_eq!(at.x, edge, "{id:?} starts where its neighbour ended");
                assert_eq!(at.height, 24, "a column split does not change height");
                edge = edge.saturating_add(at.width);
            }
        }
    }

    /// **A row split divides height, and a resize moves the divider it names.**
    ///
    /// The two axes are one function with the roles swapped, so this is the
    /// half that would silently keep working if `divide` cut the wrong way.
    #[test]
    fn a_row_split_divides_height_and_follows_the_share() {
        let mut tree = PaneTree::Leaf(PaneId(0));
        assert!(tree.split(PaneId(0), PaneId(1), Direction::Down));

        let placed = tree.layout(Rect::new(0, 0, 80, 24));
        assert_eq!(placed[0].1, Rect::new(0, 0, 80, 12));
        assert_eq!(
            placed[1].1,
            Rect::new(0, 12, 80, 12),
            "the lower pane starts where the upper one ended, full width"
        );

        assert!(tree.resize(PaneId(0), 25));
        let placed = tree.layout(Rect::new(0, 0, 80, 24));
        assert_eq!(placed[0].1.height, 18, "75% of 24");
        assert_eq!(
            placed[1].1.y, 18,
            "and the one below moves down by exactly what the one above gained"
        );
        assert_eq!(placed[0].1.height + placed[1].1.height, 24);
    }

    /// **A split puts the new pane on the side the direction names.**
    ///
    /// `:vsplit` in vim opens the new window to the *left* and `:split` above;
    /// `Right` and `Down` are the mirrors. Getting this backwards is not a
    /// crash — it is an editor whose splits open on the wrong side, which is
    /// exactly what a structure with no rectangles in it can be made to prove.
    #[test]
    fn a_split_puts_the_new_pane_on_the_side_it_was_told() {
        for (direction, expected) in [
            (Direction::Right, vec![PaneId(0), PaneId(1)]),
            (Direction::Down, vec![PaneId(0), PaneId(1)]),
            (Direction::Left, vec![PaneId(1), PaneId(0)]),
            (Direction::Up, vec![PaneId(1), PaneId(0)]),
        ] {
            let mut tree = PaneTree::Leaf(PaneId(0));
            assert!(tree.split(PaneId(0), PaneId(1), direction));
            assert_eq!(
                tree.leaves(),
                expected,
                "{direction:?} decides which side the new pane lands on, and \
                 `leaves` is left-to-right and top-to-bottom"
            );
        }

        let mut tree = PaneTree::Leaf(PaneId(0));
        assert!(
            !tree.split(PaneId(9), PaneId(1), Direction::Right),
            "splitting a pane that is not in the tree does nothing"
        );
    }

    /// **Closing collapses the split into the sibling, and the last pane
    /// cannot be closed.**
    ///
    /// A tree with no leaves is not a state this structure can represent, and
    /// it is not one an editor should reach: closing the only window is what
    /// `:quit` means, and that is a different verb.
    #[test]
    fn closing_a_pane_collapses_its_split_and_the_last_one_stays() {
        let (mut tree, ids) = row(3);
        assert_eq!(tree.leaves(), ids);

        assert!(tree.close(ids[1]));
        assert_eq!(
            tree.leaves(),
            vec![ids[0], ids[2]],
            "the middle pane went and the two beside it are still in order"
        );

        assert!(tree.close(ids[2]));
        assert_eq!(tree.leaves(), vec![ids[0]]);

        assert!(
            !tree.close(ids[0]),
            "the last pane stays — :quit is the verb for leaving"
        );
        assert_eq!(tree.leaves(), vec![ids[0]]);
    }

    /// **A resize moves the divider the pane sits against, and stops short of
    /// hiding either side.**
    ///
    /// A share that can reach zero makes a pane unreachable: still in the tree,
    /// still focusable, drawing nothing — a state one keystroke gets you into
    /// and no amount of looking at the screen gets you out of.
    #[test]
    fn a_resize_moves_the_nearest_divider_and_clamps() {
        let (mut tree, ids) = row(2);

        assert!(tree.resize(ids[0], 10), "the divider moves");
        let PaneTree::Split { first_share, .. } = &tree else {
            panic!("two panes are a split");
        };
        assert_eq!(*first_share, 60, "growing the first pane grows its share");

        assert!(
            tree.resize(ids[1], 10),
            "and the second grows the other way"
        );
        let PaneTree::Split { first_share, .. } = &tree else {
            panic!("two panes are a split");
        };
        assert_eq!(*first_share, 50);

        // Push it as far as it will go, from both ends.
        assert!(tree.resize(ids[0], 500));
        let PaneTree::Split { first_share, .. } = &tree else {
            panic!("two panes are a split");
        };
        assert_eq!(
            *first_share, 90,
            "clamped, so the other side is still there"
        );
        assert!(
            !tree.resize(ids[0], 500),
            "and once clamped it reports that nothing moved, rather than \
             claiming a resize that did not happen"
        );

        assert!(
            !tree.resize(PaneId(9), 10),
            "a pane that is not in the tree resizes nothing"
        );
    }

    /// **A direction lands in the pane actually that way**, which is the
    /// assertion two panes cannot make and three can.
    ///
    /// `toward` walks up to the nearest ancestor dividing along the matching
    /// axis and takes the neighbouring subtree's *nearest* leaf — its right
    /// edge going left, its left edge going right. Taking the first leaf both
    /// ways looks correct until there is a nested split to get wrong.
    #[test]
    fn a_direction_lands_in_the_pane_that_way_and_stops_at_the_edge() {
        let (tree, ids) = row(3);

        assert_eq!(tree.toward(ids[0], Direction::Right), Some(ids[1]));
        assert_eq!(tree.toward(ids[1], Direction::Right), Some(ids[2]));
        assert_eq!(
            tree.toward(ids[2], Direction::Right),
            None,
            "the rightmost pane has nothing to its right — an edge, not a wrap"
        );
        assert_eq!(
            tree.toward(ids[2], Direction::Left),
            Some(ids[1]),
            "and going back lands on the *nearest* pane, not the first one"
        );
        assert_eq!(tree.toward(ids[0], Direction::Left), None);
        assert_eq!(
            tree.toward(ids[0], Direction::Down),
            None,
            "a row has no pane below any of it"
        );

        // **The case that tells `first()` from `last()`.** Splitting the
        // *left* pane nests a subtree on the far side of the divider, so
        // "the pane to the left of the rightmost" has two candidates and only
        // the nearer one is right. `row(3)` nests on the other side, where a
        // single-leaf neighbour makes the two spellings agree — which is why
        // this test passed with the defect planted until this block existed.
        let mut left_nested = PaneTree::Leaf(PaneId(0));
        assert!(left_nested.split(PaneId(0), PaneId(1), Direction::Right));
        assert!(left_nested.split(PaneId(0), PaneId(2), Direction::Right));
        assert_eq!(left_nested.leaves(), vec![PaneId(0), PaneId(2), PaneId(1)]);
        assert_eq!(
            left_nested.toward(PaneId(1), Direction::Left),
            Some(PaneId(2)),
            "the pane immediately left, not the leftmost one in that subtree"
        );

        // A column below a row: `Down` from the top-left must find it, and
        // `Right` from it must still cross the outer divider.
        let mut nested = tree;
        assert!(nested.split(ids[0], PaneId(9), Direction::Down));
        assert_eq!(nested.toward(ids[0], Direction::Down), Some(PaneId(9)));
        assert_eq!(
            nested.toward(PaneId(9), Direction::Right),
            Some(ids[1]),
            "the nearest ancestor on the matching axis is the outer split, so \
             a pane in a nested column still has the next column to its right"
        );
    }

    /// **`Panes` keeps the tree and the map in step**, which is the invariant
    /// every `at`/`at_mut` depends on.
    #[test]
    fn closing_the_focused_pane_moves_focus_somewhere_that_exists() {
        let (mut panes, first) = Panes::new(Pane::new(BufferId(0)));
        let second = panes
            .split(first, Pane::new(BufferId(0)), Direction::Right)
            .expect("the first pane splits");
        panes.focus = second;

        assert!(panes.close(second));

        assert_eq!(
            panes.focus, first,
            "focus left the pane that closed — a `focus` pointing at a removed \
             pane is the one way this struct can break `at`'s expect"
        );
        assert!(panes.get(second).is_none(), "and the map lost it too");
        assert_eq!(panes.tree.leaves(), vec![first]);

        assert!(
            !panes.close(first),
            "the last pane stays, so the map is never empty either"
        );
    }

    /// **An answer tagged for B lands in B, while A is focused.**
    ///
    /// This is the chain step 6a and step 9 build between them: `answering`
    /// tags the Action with the buffer that asked, `Buffers::named` routes it,
    /// and the arm's `at` guard tests it against *that* buffer's cursor. Every
    /// link was missing — the tag was `buffer: None` on all six, and the arm
    /// dropped the field with `..`.
    #[test]
    fn a_completion_answer_lands_in_the_buffer_that_asked_for_it() {
        let (mut buffers, focused) = Buffers::new(typed("alpha", 120).editing);
        let asked = buffers.open(typed("bravo", 120).editing);

        let mut panes = one_pane();
        let mut shell = shell();
        let at = buffers
            .at_mut(asked)
            .text(&Cx::new(asked, PaneId(0), &mut panes, &mut shell))
            .cursor();

        let answer = Action::Lsp(phosphor_core::action::LspAction::IngestCompletions {
            items: vec![WireCompletion {
                label: "bravado".to_owned(),
                insert: "bravado".to_owned(),
                detail: None,
                documentation: Vec::new(),
                kind: None,
                source: None,
                deprecated: false,
            }],
            at,
            buffer: Some(asked),
        });

        let target = Buffers::named(&answer, focused);
        assert_eq!(target, asked, "the tag routes it");

        buffers.at_mut(target).apply(
            &mut Cx::new(target, PaneId(0), &mut panes, &mut shell),
            &answer,
        );

        assert!(
            buffers.at_mut(asked).completion.is_some(),
            "the buffer that asked has the list"
        );
        assert!(
            buffers.at_mut(focused).completion.is_none(),
            "and the one in front of the user does not — the assertion that \
             fails while the answer is untagged and the arm reads `..`"
        );
    }

    /// **One buffer's in-flight request does not hold another's gate shut.**
    ///
    /// `Outstanding` counted with no key at all — three bare `u32`s for the
    /// whole session — and that is wrong twice over with two buffers open. The
    /// insert-mode trigger's *"one request in flight at a time"* gate reads a
    /// count a different file's request is holding open, so typing in B waits
    /// on A; and an answer for B is taken off A's count, so A's gate re-arms on
    /// an answer it never asked for.
    #[test]
    fn a_request_in_flight_for_one_buffer_does_not_gate_another() {
        let mut asking = Asking::default();
        let (a, b) = (BufferId(0), BufferId(1));

        asking.at(a).sent(Lookup::Completion);

        assert!(asking.at(a).awaiting(Lookup::Completion));
        assert!(
            !asking.at(b).awaiting(Lookup::Completion),
            "B may ask, because it is not the one waiting"
        );
        assert!(
            asking.anyone_awaiting(Lookup::Completion),
            "and the poll deadline is the session's, because an answer for any \
             buffer is an event that wakes the loop"
        );

        // B's answer comes back and must not re-arm A.
        assert!(
            !asking.at(b).answers(&ingest_completions()),
            "B was owed nothing"
        );
        assert!(
            asking.at(a).awaiting(Lookup::Completion),
            "so A is still waiting — the assertion that fails while one count \
             serves every buffer"
        );
    }

    /// **`:wall` writes every dirty buffer, and says what it could not write.**
    ///
    /// The arm called `self.write(None)` under a comment reading *"there is
    /// exactly one, and `T088` is what makes there be more"*. There are more,
    /// and an arm holds one — so it records the ask and the loop performs it.
    ///
    /// The assertion that matters is the **third** buffer. Writing them one at
    /// a time rather than stopping at the first failure is the difference
    /// between `:wall` on a session with one unnamed scratch buffer writing
    /// everything else, and writing nothing after it.
    #[test]
    fn wall_writes_past_a_buffer_it_cannot_write() {
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let directory = scratch("wall-all");
        let mut written = Vec::new();

        let mut make = |name: Option<&str>, text: &str| {
            let file = name.map(|name| directory.join(name));
            if let Some(file) = file.as_ref() {
                written.push(file.clone());
            }
            let editing = Editing::new(
                buffer("text", text, &theme).expect("a buffer"),
                file,
                std::rc::Rc::new(std::cell::Cell::new(false)),
            );
            editing.dirty.set(true);
            editing
        };

        let (mut buffers, _) = Buffers::new(make(Some("first.txt"), "one\n"));
        buffers.open(make(None, "scratch\n"));
        buffers.open(make(Some("third.txt"), "three\n"));

        // What the loop's `:wall` block does, over the same map.
        let mut trouble = Vec::new();
        for buffer in buffers.map.values_mut() {
            if !buffer.dirty.get() {
                continue;
            }
            if let Err(reason) = buffer.write(None) {
                trouble.push(reason);
            }
        }

        assert_eq!(
            trouble,
            vec!["no file name — :write <path>".to_owned()],
            "the unnamed buffer refuses in the editor's own voice"
        );
        for file in &written {
            assert!(
                file.exists(),
                "{} was written — including the one *after* the failure, which \
                 is the whole reason the loop does not stop at the first",
                file.display()
            );
        }
    }

    /// **`:quit` counts unsaved work in every buffer**, and `ZQ` counts none.
    ///
    /// Two checks, each where the information is: the arm refuses
    /// `WouldLoseWork` for the buffer it holds, and the loop — which holds them
    /// all — answers for the rest. The forced spelling must count *nothing*
    /// rather than skip the check, because skipping it is the shape that leaves
    /// no way out of the loop at all. `ZQ` at a scratch buffer is the one exit
    /// a bare `phosphor` has, there being no file to `:write` to.
    #[test]
    fn quitting_counts_unsaved_work_in_buffers_that_are_not_on_screen() {
        let (mut buffers, first) = Buffers::new(editing("alpha").editing);
        let second = buffers.open(editing("bravo").editing);

        let unsaved = |buffers: &Buffers| {
            buffers
                .map
                .values()
                .filter(|buffer| buffer.dirty.get())
                .count()
        };

        assert_eq!(unsaved(&buffers), 0, "nothing has been edited");

        buffers.at_mut(second).dirty.set(true);

        assert_eq!(
            unsaved(&buffers),
            1,
            "the buffer nobody is looking at counts — the assertion the arm \
             cannot make, because it holds one buffer"
        );
        assert!(
            !buffers.at_mut(first).dirty.get(),
            "and the focused one is still clean, so its own arm would have said \
             yes to `:quit`"
        );
    }

    /// **`:close-buffer` closes, and the pane it was in shows another.**
    ///
    /// It declined with *"one buffer, one pane — :quit leaves; T088 gives a
    /// buffer somewhere to close to"*, which was true and is not any more. The
    /// loop's half is the question the arm cannot answer: whether there is
    /// anywhere for the pane to go.
    #[test]
    fn closing_a_buffer_points_its_pane_at_another() {
        let (mut buffers, first) = Buffers::new(editing("alpha").editing);
        let second = buffers.open(editing("bravo").editing);
        let (mut panes, only) = Panes::new(Pane::new(first));

        // What the loop's `close-buffer` block does.
        let successor = buffers.map.keys().copied().find(|id| *id != first);
        assert_eq!(successor, Some(second));
        buffers.map.remove(&first);
        for pane in panes.map.values_mut() {
            if pane.buffer == Some(first) {
                pane.buffer = successor;
            }
        }

        assert_eq!(
            panes.at(only).buffer,
            Some(second),
            "the pane shows the buffer that is left"
        );
        assert_eq!(
            buffers.at_mut(second).editor.get_content(),
            "bravo",
            "and it is the one it was pointed at, by id"
        );
        assert!(
            buffers
                .map
                .keys()
                .copied()
                .find(|id| *id != second)
                .is_none(),
            "with one buffer left there is no successor, which is what makes \
             `:close-buffer` on the last one say `:quit` leaves instead"
        );
    }

    /// **Editing A does not move B's edit counter**, which is the whole of what
    /// step 7 makes expressible.
    ///
    /// The counter was one `Rc<Cell<u64>>` held by the loop and compared
    /// against one `sent`. That pair cannot say *"A changed, B did not"*: with
    /// a second buffer open, the server holding B is never told B changed, so
    /// every completion, hover and diagnostic it produces for B is computed
    /// against the text as it was when B was last looked at. A file you edited,
    /// switched away from, and came back to would answer about a version of
    /// itself that no longer exists — and nothing on screen would say so.
    ///
    /// The counters are `Rc`s the change callback holds, so the assertion is on
    /// them rather than on the server: the didChange gate is `edits != sent`,
    /// and this is that gate's two halves.
    #[test]
    fn one_buffers_edits_do_not_move_another_buffers_didchange_gate() {
        let mut alpha = editing("alpha\n");
        let bravo = editing("bravo\n");

        assert_eq!(alpha.edits.get(), 0);
        assert_eq!(bravo.edits.get(), 0);

        alpha.apply(&Action::Buffer(
            phosphor_core::action::BufferAction::Insert {
                at: Position { line: 1, column: 1 },
                text: "// ".to_owned(),
            },
        ));

        assert!(
            alpha.edits.get() > 0,
            "the buffer that was edited counted it"
        );
        assert_eq!(
            bravo.edits.get(),
            0,
            "and the one that was not did not — the assertion that cannot even \
             be written while the counter is the loop's"
        );
        assert_eq!(
            bravo.sent,
            bravo.edits.get(),
            "so B's didChange gate is closed, and its server is not told about \
             an edit made in A"
        );
        assert_ne!(
            alpha.sent,
            alpha.edits.get(),
            "while A's is open, and the next pass sends it"
        );
    }

    /// **A swapped-in rope re-points the counters at itself.**
    ///
    /// A new `Editor` carries no change callback, so without this both the
    /// dirty flag and the edit counter freeze at whatever the last rope made
    /// them: `[+]` on a buffer nobody has touched, and a `didChange` that never
    /// goes out again. `Editing::opens` calls `retrack`, and it is a method on
    /// the buffer rather than a call with the loop's two `Rc`s — handing the
    /// loop's pair to a *second* buffer's rope would have both buffers
    /// reporting one file's edits.
    #[test]
    fn a_buffer_that_takes_a_new_rope_still_counts_its_own_edits() {
        let mut editing = editing("alpha\n");
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let file = scratch("retrack").join("other.txt");

        editing.editing.opens(
            buffer("text", "one\n", &theme).expect("a buffer"),
            file,
            Timeline::detached(),
        );
        let before = editing.edits.get();

        editing.apply(&Action::Buffer(
            phosphor_core::action::BufferAction::Insert {
                at: Position { line: 1, column: 1 },
                text: "x".to_owned(),
            },
        ));

        assert!(
            editing.edits.get() > before,
            "the counter follows the rope it is counting"
        );
        assert!(
            editing.dirty.get(),
            "and so does the dirty flag, which is the half a user sees as `[+]`"
        );
    }

    /// **A reveal moves the pane the cursor moved in, not the focused one.**
    ///
    /// `Editing::reveal` said `PaneRef::Focused {}` and was right by accident:
    /// the `Scroll` arm dropped the selector too, so both halves ignored it and
    /// agreed with each other. The moment that arm started reading it — which
    /// is this step — a reveal in an unfocused pane would have scrolled the
    /// focused one.
    ///
    /// **No existing test could see this**, because the mistake is in the
    /// *pair*: it needs two panes with different areas, and one of them not
    /// focused. This is what step 4c's map is for.
    #[test]
    fn a_reveal_scrolls_the_pane_the_cursor_is_in() {
        let mut editing = editing("x\n".repeat(200).as_str());
        let mut shell = shell();

        // Two panes on the same buffer: a tall one with focus, and a short one
        // where the reveal happens.
        let (mut panes, focused) = Panes::new(Pane {
            area: Rect::new(0, 0, 80, 100),
            ..Pane::new(BufferId(0))
        });
        let elsewhere = panes
            .split(
                focused,
                Pane {
                    area: Rect::new(0, 0, 80, 5),
                    ..Pane::new(BufferId(0))
                },
                Direction::Down,
            )
            .expect("the focused pane splits");
        panes.focus = focused;

        editing.editor.set_cursor(180);
        editing
            .editing
            .reveal(&mut Cx::new(BufferId(0), elsewhere, &mut panes, &mut shell));

        assert!(
            editing.editor.get_offset_y() > 0,
            "the reveal was measured against the five-row pane the cursor is \
             in, which cannot show row 90 without scrolling — measured against \
             the hundred-row focused pane it would not have moved at all"
        );
    }

    /// **A `scroll` naming a pane that does not exist refuses**, rather than
    /// moving the one in front of the user.
    ///
    /// `Direction` refuses for a different reason and it is worth keeping
    /// separate: a compass direction is a fact about where the rectangles are,
    /// and answering it from one pane's rectangle would be answering from no
    /// information. Step 11 resolves it against the tree.
    #[test]
    fn a_scroll_naming_no_pane_refuses_rather_than_moving_the_focused_one() {
        let mut editing = editing("x\n".repeat(200).as_str());
        editing.pane_mut().area = Rect::new(0, 0, 80, 10);

        for (reference, why) in [
            (PaneRef::Id { id: PaneId(99) }, "an id that names no pane"),
            (
                PaneRef::Direction {
                    direction: Direction::Right,
                },
                "a compass direction with one pane, which has no neighbour \
                 that way — step 10 gave this a tree to walk and the tree \
                 answers None at the edge, which is still a refusal",
            ),
        ] {
            let outcome = editing.act(&Action::View(super::ViewAction::Scroll {
                request: phosphor_core::request::ScrollRequest::RevealRow {
                    row: 150,
                    margin: 0,
                },
                pane: reference,
            }));
            assert!(
                matches!(outcome, Outcome::Refused(Refusal::NoSuchTarget)),
                "{why}: {outcome:?}"
            );
            assert_eq!(
                editing.editor.get_offset_y(),
                0,
                "{why}: and the viewport in front of the user did not move"
            );
        }
    }

    /// **`Next` and `Prev` cycle, and with one pane both answer that pane** —
    /// which is vim's answer too.
    #[test]
    fn the_pane_cycle_wraps_at_both_ends() {
        let (mut panes, first) = Panes::new(Pane::new(BufferId(0)));

        assert_eq!(panes.resolve(&PaneRef::Next {}), Some(first));
        assert_eq!(panes.resolve(&PaneRef::Prev {}), Some(first));

        let second = panes
            .split(first, Pane::new(BufferId(0)), Direction::Right)
            .expect("the first pane splits");

        assert_eq!(panes.resolve(&PaneRef::Next {}), Some(second));
        assert_eq!(
            panes.resolve(&PaneRef::Prev {}),
            Some(second),
            "and backward, which with two panes is the same neighbour"
        );

        // **Every reference is relative to focus**, so cycling from the other
        // pane means moving focus — not passing a different pane in. That is
        // the distinction `a_reveal_scrolls_the_pane_the_cursor_is_in` exists
        // to protect: an Action applied to one pane and naming `Focused` means
        // the pane the *user* is looking at.
        panes.focus = second;

        assert_eq!(
            panes.resolve(&PaneRef::Next {}),
            Some(first),
            "the cycle wraps forward"
        );
        assert_eq!(panes.resolve(&PaneRef::Focused {}), Some(second));
        assert_eq!(panes.resolve(&PaneRef::Id { id: first }), Some(first));
        assert_eq!(panes.resolve(&PaneRef::Id { id: PaneId(99) }), None);
    }

    /// **`set-cursor` naming a buffer goes to that buffer**, which is the
    /// whole of what step 6 does for the four selectors the applier discarded.
    ///
    /// `Buffers::named` is the routing, and it is deliberately a read rather
    /// than a method on the Action: the door reads it *before* choosing which
    /// `Editing` to hand the Action to, because an arm holding `&mut self`
    /// cannot reach a sibling out of the same map.
    #[test]
    fn an_action_naming_a_buffer_is_routed_to_that_buffer() {
        let alpha = editing("alpha").editing;
        let bravo = editing("bravo").editing;
        let (mut buffers, first) = Buffers::new(alpha);
        let second = buffers.open(bravo);

        let elsewhere = Action::Motion(phosphor_core::action::MotionAction::SetCursor {
            position: Position { line: 1, column: 4 },
            buffer: Some(second),
        });
        let here = Action::Motion(phosphor_core::action::MotionAction::SetCursor {
            position: Position { line: 1, column: 4 },
            buffer: None,
        });

        assert_eq!(
            Buffers::named(&elsewhere, first),
            second,
            "an Action that names a buffer names it"
        );
        assert_eq!(
            Buffers::named(&here, first),
            first,
            "and one that names none means the focused one, which is what its \
             own doc says"
        );

        let mut shell = shell();
        let (mut panes, only) = Panes::new(Pane::new(second));
        let target = Buffers::named(&elsewhere, first);
        buffers.at_mut(target).apply(
            &mut Cx::new(target, only, &mut panes, &mut shell),
            &elsewhere,
        );

        assert_eq!(
            buffers.at_mut(second).editor.get_cursor(),
            3,
            "the named buffer's cursor moved"
        );
        assert_eq!(
            buffers.at_mut(first).editor.get_cursor(),
            0,
            "and the focused one's did not — the assertion that fails while \
             the arm reads `..`"
        );
    }

    /// **A buffer id that names nothing refuses rather than moving whatever is
    /// in front of the user.**
    ///
    /// The routing at the loop's posted door answers this by not finding the
    /// buffer. This is the other half: a door that *cannot* route — an ex line
    /// runs against the buffer it was typed in — hands the Action to the
    /// focused buffer, and the guard at the top of `Editing::act` is what stops
    /// it being applied there. `NoSuchTarget`'s own doc names this exact case:
    /// *"a stale id from an agent working off an old query"*.
    #[test]
    fn a_stale_buffer_id_refuses_instead_of_moving_the_focused_cursor() {
        let mut editing = editing("alpha bravo");

        let outcome = editing.apply(&Action::Motion(
            phosphor_core::action::MotionAction::SetCursor {
                position: Position { line: 1, column: 7 },
                buffer: Some(BufferId(99)),
            },
        ));

        assert!(
            matches!(outcome, Outcome::Refused(Refusal::NoSuchTarget)),
            "{outcome:?}"
        );
        assert_eq!(
            editing.editor.get_cursor(),
            0,
            "and the cursor in front of the user did not move, which is what \
             the discarded selector did instead"
        );
    }

    /// **A selection anchor does not survive the rope it was measured in.**
    ///
    /// `selection_from` is a char offset. The swap block rewrote `editor`,
    /// `timeline`, `depth`, `file`, the completion and the signature, and left
    /// this one holding a position in a file that is no longer open.
    ///
    /// **The assertion is on the field, not on `get_selection()`,** and that is
    /// deliberate: asserting on the selection would pass on the broken code,
    /// because the swap replaces `editing.editor` wholesale and a fresh editor
    /// has no selection to show. The stale value is invisible until the next
    /// `ExtendSelection` reads it — which is reachable straight after a swap,
    /// because the machine is the *session's* and its visual anchor outlives
    /// the buffer.
    #[test]
    fn opening_a_file_drops_a_selection_measured_in_the_last_one() {
        let mut editing = editing("alpha bravo charlie delta echo\n");

        // Select `bravo`, the way `v e` does, and leave the anchor set.
        editing.apply(&Action::Motion(
            phosphor_core::action::MotionAction::SelectRange {
                span: Span {
                    start: Position { line: 1, column: 7 },
                    end: Position {
                        line: 1,
                        column: 12,
                    },
                },
                kind: phosphor_core::request::SelectionKind::Line,
            },
        ));
        assert!(
            editing.selection_from.is_some(),
            "the anchor is set — otherwise this test proves nothing"
        );

        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let file = scratch("swap").join("other.txt");
        let leaving = editing.editing.opens(
            buffer("text", "one\n", &theme).expect("a buffer"),
            file.clone(),
            Timeline::detached(),
        );

        assert_eq!(leaving, None, "this buffer had no file to leave behind");
        assert_eq!(editing.file.as_deref(), Some(file.as_path()));
        assert!(
            editing.selection_from.is_none(),
            "an offset into the rope that just left is not a position in this one"
        );
        assert_eq!(
            editing.selection_kind,
            phosphor_core::request::SelectionKind::Char,
            "and `V` in the file you left does not make the first extend in the \
             file you arrived at linewise — `ExtendSelection` reads this and \
             never sets it"
        );
    }

    /// **Two panes, two buffers, and every lookup by id** — step 4c's whole
    /// claim, made while the binary still opens one of each.
    ///
    /// The two assertions are the two failures the maps exist to prevent. The
    /// first is that an id survives its neighbour closing: `close-buffer` on
    /// the first entry leaves the second's id naming the second buffer, which
    /// is exactly what a `Vec` and a `usize` would get wrong — every held
    /// index shifts by one, silently and with no type error. The second is that
    /// a reopened buffer is a *new* buffer: the id is minted off the counter,
    /// so the closed one stays closed and a stale `BufferId` refuses rather
    /// than resolving to whatever took its place.
    #[test]
    fn a_closed_buffer_does_not_hand_its_id_to_the_next_one() {
        let (mut buffers, first) = Buffers::new(editing("alpha").editing);
        let second = buffers.open(editing("bravo").editing);

        assert_ne!(first, second, "two buffers, two ids");
        assert_eq!(
            buffers.at_mut(second).editor.get_content(),
            "bravo",
            "an id names a buffer, not a position in a list"
        );

        buffers.map.remove(&first);

        assert_eq!(
            buffers.at_mut(second).editor.get_content(),
            "bravo",
            "and it still names the same one after its neighbour closed — the \
             assertion a Vec and a usize fail"
        );

        let third = buffers.open(editing("charlie").editing);
        assert_ne!(
            third, first,
            "a reopened buffer is a new buffer: ids come off the counter, so a \
             stale one refuses rather than resolving to whatever took its place"
        );
    }

    /// **A pane names a buffer; it does not contain one.** Swapping a file into
    /// a pane is a write to one field, and the buffer that left is still in
    /// `Buffers` with its history intact — which is what `:bnext` and a second
    /// split showing the same file both need, and neither could have while
    /// `Editing` *was* the pane.
    #[test]
    fn swapping_a_buffer_into_a_pane_leaves_the_one_that_left_open() {
        let (mut buffers, first) = Buffers::new(editing("alpha").editing);
        let second = buffers.open(editing("bravo").editing);
        let (mut panes, only) = Panes::new(Pane::new(first));

        panes.at_mut(only).buffer = Some(second);

        assert_eq!(panes.focus, only, "the pane is the same pane");
        assert_eq!(
            buffers.at_mut(first).editor.get_content(),
            "alpha",
            "the buffer that left the pane is still open, with its rope"
        );
        assert_eq!(
            buffers
                .at_mut(panes.at(only).buffer.expect("the pane holds one"))
                .editor
                .get_content(),
            "bravo"
        );
    }

    /// **Two buffers, one store**, which is what step 4b's `Shell` makes
    /// structural rather than remembered.
    ///
    /// The store was a field on `Editing`, so every buffer took its own clone
    /// of a handle — pointing at the same object *if* whoever built the buffer
    /// was handed the right one. Nothing checked that, and step 8 builds
    /// `Editing`s from a place with no business knowing a store exists. Hung
    /// off the context, an arm cannot reach anything except the session's, and
    /// this is the assertion that says so: two buffers, two files, one anchor
    /// each, and the count is on the session.
    #[test]
    fn two_buffers_place_their_anchors_in_one_store() {
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let directory = scratch("one-store");
        let mut shell = shell();
        let (mut panes, only) = Panes::new(Pane::new(BufferId(0)));

        for name in ["alpha.txt", "beta.txt"] {
            let file = directory.join(name);
            std::fs::write(&file, "one\ntwo\n").expect("a file to anchor into");
            let mut editing = Editing::new(
                buffer("text", "one\ntwo\n", &theme).expect("a buffer"),
                Some(file),
                std::rc::Rc::new(std::cell::Cell::new(false)),
            );
            editing.push_jump(&mut Cx::new(BufferId(0), only, &mut panes, &mut shell));
        }

        assert_eq!(
            shell.store.anchor_count(),
            2,
            "both buffers wrote into the session's store, because there is no \
             other one an arm can reach"
        );
    }

    /// **The same buffer in two panes wraps to two widths.** `Editing::wrapped`
    /// used to measure a hover float against `self.area`, so which width it
    /// got depended on which pane had most recently been laid out. It measures
    /// the pane it is handed now, and a test can hand it two.
    ///
    /// The numbers are not the point and are not asserted: what is asserted is
    /// that the narrow pane produces *more* lines from the same prose, which is
    /// only true if the width came from the pane rather than from the buffer.
    #[test]
    fn the_same_prose_wraps_to_more_lines_in_a_narrower_pane() {
        let editing = editing("").editing;
        let prose = vec![
            "A hover answer long enough that a narrow pane has to break it and \
             a wide one does not."
                .to_owned(),
        ];

        let mut shell = shell();
        let (mut panes, narrow) = Panes::new(Pane {
            area: Rect::new(0, 0, 24, 10),
            ..Pane::new(BufferId(0))
        });
        let wide = panes
            .split(
                narrow,
                Pane {
                    area: Rect::new(0, 0, 200, 10),
                    ..Pane::new(BufferId(0))
                },
                Direction::Right,
            )
            .expect("the narrow pane splits");

        let in_narrow = editing
            .wrapped(
                &Cx::new(BufferId(0), narrow, &mut panes, &mut shell),
                &prose,
            )
            .len();
        let in_wide = editing
            .wrapped(&Cx::new(BufferId(0), wide, &mut panes, &mut shell), &prose)
            .len();
        assert!(
            in_narrow > in_wide,
            "the width is the pane's, not the buffer's"
        );
    }

    #[test]
    fn the_ex_line_types_and_runs_through_the_same_path_a_key_does() {
        // `T033`'s ex half: `:w` is `:write` by the abbreviation rule, and the
        // Actions it names are applied by `Editing`, not by a second path.
        let theme = super::builtin("phosphor-dark").expect("a shipped theme");
        let file = scratch("ex").join("written.txt");
        let mut editing = Bench {
            editing: Editing::new(
                buffer("text", "one\ntwo", &theme).expect("a buffer"),
                Some(file.clone()),
                std::rc::Rc::new(std::cell::Cell::new(true)),
            ),
            panes: one_pane(),
            focus: PaneId(0),
            shell: shell(),
        };
        let (mut layer, _host) = booted();
        let (buffer, mut cx) = editing.split();

        let mut line = String::new();
        assert_eq!(ex_key(event(KeyCode::Char('w')), &mut line), ExStep::Typing);
        assert_eq!(ex_key(event(KeyCode::Enter), &mut line), ExStep::Submit);
        assert_eq!(line, "w");
        assert_eq!(submit_ex(&mut layer, buffer, &mut cx, &line), None);
        assert_eq!(
            std::fs::read_to_string(&file).expect("the file was written"),
            "one\ntwo"
        );

        // A command nobody defined says so rather than doing nothing.
        assert!(submit_ex(&mut layer, buffer, &mut cx, "nosuchthing").is_some());
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

    /// **A rect laid out before a resize must never outlive its buffer.**
    ///
    /// The layout is computed from `term.size()` at the top of the pass and
    /// `term.draw` runs hundreds of lines later, after ratatui's `autoresize()`
    /// — whose own comment warns that shrinking "may OOB". A height shrink in
    /// that window used to hand [`draw`] a `pane` taller than the buffer, and
    /// nothing downstream clips it: `buffer_view`'s `set_cell` clips to the rect
    /// it was passed rather than to the buffer, and `Buffer::set_stringn` clamps
    /// `x` and never `y`. The write reaches `index_of` and panics the editor.
    ///
    /// Found by an adversarial read of the commit that introduced [`Geometry`],
    /// not by the gate — every test in this repository renders at a size that
    /// does not change mid-pass, so there was nothing for it to fail.
    #[test]
    fn a_layout_from_before_a_resize_never_reaches_outside_the_buffer() {
        let measured = Rect::new(0, 0, 80, 24);
        let mut geometry = crate::lay_out(measured);
        // A leader hint and the unknown-key row both up: the deepest layout,
        // and therefore the one with the most rects to get wrong.
        geometry.take_strips(
            &[phosphor_core::view::KeyHint {
                key: phosphor_core::request::KeySeq("t".to_owned()),
                verb: "theme".to_owned(),
            }],
            true,
            &phosphor_ui::theme::Theme::phosphor_dark(),
        );
        assert!(
            geometry.hint.is_some(),
            "the strip is up, so there is a rect"
        );

        // The terminal shrank between the measurement and the draw.
        let rendered = Rect::new(0, 0, 80, 20);
        let clamped = geometry.clamped_to(rendered);

        for (name, rect) in [
            ("frame", clamped.frame),
            ("body", clamped.body),
            ("pane", clamped.pane),
            ("leader", clamped.leader.unwrap_or_default()),
            ("hint", clamped.hint.unwrap_or_default()),
            ("status", clamped.status),
        ] {
            // **Empty rects are exempt, and that is the real invariant rather
            // than a loosening.** `intersection` answers an empty rect when
            // there is no overlap at all, and it keeps the stale origin while
            // doing it — the strip that was at row 21 of a 24-row terminal
            // comes back as `y: 21, height: 0`. Nothing writes through one:
            // `buffer_view`'s render loops `0..area.height`, so zero rows is
            // zero cells. What panics is a rect that *writes* outside the
            // buffer, so that is what is asserted. The first draft of this test
            // asserted the position of every rect and failed on exactly this.
            if rect.is_empty() {
                continue;
            }
            assert!(
                rect.bottom() <= rendered.bottom() && rect.right() <= rendered.right(),
                "`{name}` writes outside the buffer being rendered: {rect:?} \
                 against {rendered:?}",
            );
        }

        // And on the ordinary pass — the size held — it changes nothing, which
        // is what makes holding to the contract free.
        let same = crate::lay_out(measured);
        assert_eq!(same.clamped_to(measured), same);
    }

    /// **The two kinds `scripts/lint-node-kinds.sh` recorded as owed to
    /// `T088`, in one node.** The lint checks that *something* composes them;
    /// this checks the shape, because a `Node::Pane` around anything at all
    /// would satisfy the lint and only a pane around the buffer is a frame.
    ///
    /// `soft_wrap` is asserted in both positions on purpose: it is a request
    /// the interpreter reports rather than honours (`Resources`: *"cannot be
    /// honoured from here"*), so a hardcoded `false` would compile, draw
    /// correctly, and quietly make the tree lie about the frame it composed.
    #[test]
    fn the_host_frame_is_a_pane_around_the_buffer() {
        for soft_wrap in [false, true] {
            let phosphor_core::view::Node::Pane {
                pane,
                holds,
                focused,
                child,
            } = frame_of(PaneId(7), BufferId(3), soft_wrap)
            else {
                panic!("the host's frame is a pane");
            };
            // **Ids the caller chose, not constants this file owns.** They were
            // `THE_PANE` and `THE_BUFFER`, and testing a composition against
            // the same two literals it was built from could not tell you the
            // function carried them through at all.
            assert_eq!(pane, PaneId(7));
            assert_eq!(holds, phosphor_core::request::PaneKind::Buffer);
            assert!(focused, "one pane, and keystrokes go to it");
            assert_eq!(
                child.node(),
                &phosphor_core::view::Node::Buffer {
                    buffer: BufferId(3),
                    soft_wrap,
                },
                "the pane holds the buffer, and carries the wrap the loop applied"
            );
        }
    }

    /// One pane over one buffer, composed the way the loop composes the frame.
    ///
    /// `one_pane` was a separate function until step 12 folded it into
    /// `compose_panes` as the degenerate case, which is what it always was — a
    /// `PaneTree::Leaf` with nothing beside it.
    fn frame_of(pane: PaneId, buffer: BufferId, soft_wrap: bool) -> phosphor_core::view::Node {
        let (panes, _) = Panes::new(Pane::new(buffer));
        let mut panes = panes;
        // `Panes::new` mints from zero; this test names its own ids so that
        // passing them through is what is being checked.
        let held = panes.map.remove(&PaneId(0)).expect("the first pane");
        panes.map.insert(pane, held);
        panes.tree = PaneTree::Leaf(pane);
        panes.focus = pane;
        crate::compose_panes(&panes.tree.clone(), &panes, pane, soft_wrap, &[])
    }

    /// §8's degradation, at the binary's end of it.
    ///
    /// The whole path is: `NO_COLOR` → `phosphor_term::colour_available` →
    /// this → `Interpreter::fill` → the `Node::Buffer` arm →
    /// `BufferView::fill` → `gutter::state_cell`. Three of those links are
    /// covered by tests that name them — `the_fill_reaches_a_tree_composed_buffer`
    /// in `phosphor-ui`'s interpreter and
    /// `the_degraded_state_bar_carries_its_hue_in_a_glyph` in its widget — and
    /// this is the one the collapse added: the frame loop had been passing the
    /// answer straight to a widget it no longer draws.
    ///
    /// **`tapes/2a-degraded-nocolor.png` is the end-to-end proof and it is one
    /// capture out of fifty** — measured 2026-08-20 by deleting the fill at
    /// each link in turn and re-capturing: that screen mismatches by 515 px,
    /// and `1a-degraded-nocolor` does not move at all, because that slice is
    /// unseeded and its state column has nothing in it to degrade. A capture
    /// also needs `vhs` and `ttyd` pinned and does not run in CI, so the
    /// headless links are the ones a gate can hold.
    #[test]
    fn a_terminal_that_will_not_paint_a_background_asks_for_the_marker() {
        assert_eq!(crate::state_fill(true), phosphor_ui::gutter::Fill::Block);
        assert_eq!(crate::state_fill(false), phosphor_ui::gutter::Fill::Marker);
    }
}
