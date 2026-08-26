//! The binary's unit tests.
//!
//! **Moved out of `main.rs` on 2026-08-25, and the reason was a lint.**
//! `scripts/lint-repo-hygiene.sh` refuses a tracked file over 1 MB and
//! `main.rs` crossed it during `T073` at 22,123 lines. This module was 5,023 of
//! them — 23% — and lifting it puts the binary back under the ceiling with room
//! to spare, without moving a single line of production code.
//!
//! **It is still a unit-test module**, declared `#[cfg(test)] mod tests;` from
//! `main.rs`, so it keeps its access to private items. That is the whole reason
//! it is here rather than in `tests/`: most of what it asserts is about
//! internals an integration test cannot see.
//!
//! **Four lints had to learn about this file.** `lint-action-arms`,
//! `lint-capability-bindings`, `lint-node-kinds` and `lint-refusal-tasks` each
//! glob `crates/phosphor/src/*.rs` and strip the column-0 `#[cfg(test)]` to
//! find the production half. With the tests in their own file there is no
//! attribute to strip, so every one of them would have read this as production
//! — and a fixture that constructs an Action would have counted as an arm. They
//! skip it by name now.
//!
//! `docs/OPEN-QUESTIONS.md` §64 has what a real split of `main.rs` would cost;
//! this is the part that needed no design decision.

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
    IndentStyle, Intent, Key, Layer, Lookup, Machine, NodeId, Outstanding, Painted, Pane, PaneTree,
    Panes, Repl, ReplStep, Session, Shell, StatusVm, Surface, TAB_WIDTH, Table, Timeline, UndoTree,
    Vm, WireCompletion, boot, buffer, closes_surface, completion_floor, decode, deliver, door,
    ex_key, grammar_of, indent_style, is_press, repl_key, restored, seeding, server_chip, split,
    submit_ex, vm, wire_undo,
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
        .filter(|intent| !matches!(intent, Intent::DefineSource(..) | Intent::DefineSurface(..)))
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
        searching: None,
        search: None,
        store: Arc::new(store::Shared::default()),
        timeline: None,
        disk_change_diff: None,
        // A test shell is in no repository unless a test says so.
        vcs: None,
        disk_diff: None,
        diff_mode: phosphor_core::request::DiffMode::SideBySide,
        disk_box: None,
        // A test shell watches nothing: `Watch::idle` starts no thread, so
        // the suite pays for no filesystem watchers and no `notify` state.
        watch: crate::watch::Watch::idle(),
        review: None,
        peek: None,
        inbox: None,
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
    // `expand-diff-context` is `Allow`, so it reaches `Editing::act` and
    // falls to the `_` arm — which is the case this test is about.
    //
    // **Third occupant of this slot, and each move is the good news.** It
    // was `ingest-diagnostics` until `T040` armed it, then `refresh-vcs`
    // until `T071` armed it. This one is different in kind and should be
    // the last: `expand-diff-context` is one of the two capabilities
    // `scripts/lint-action-arms.sh` records with **no creditor at all** —
    // no mockup draws a key for it — so unlike its predecessors it is not
    // waiting for a task, and nothing is going to graduate it out from
    // under this test.
    let (buffer, mut cx) = editing.split();
    let note = deliver(
        buffer,
        &mut cx,
        &super::events::Posted {
            source: "review",
            action: Action::Review(phosphor_core::action::ReviewAction::ExpandDiffContext {
                hunk: phosphor_core::request::HunkId(0),
                lines: 3,
            }),
        },
    );
    assert_eq!(
        note.as_deref(),
        Some("review: not built yet — T066 builds it"),
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

    // **`reload-runtime` was this example until `T094` built it**, which is the
    // ordinary way a fixture like this expires: the capability picked to stand
    // for *"unbuilt"* stops being unbuilt. It is an ask the loop drains now,
    // like its two neighbours above.
    assert!(matches!(
        ask(&host, Action::Runtime(RuntimeAction::ReloadRuntime {})),
        Outcome::Done(_)
    ));
    assert_eq!(host.intents(), vec![Intent::ReloadRuntime]);

    // Everything else answers its own row's task — derived, never listed.
    //
    // **A capability in a phase this build has not reached**, chosen for that
    // property rather than for being handy: `T103` made the fallthrough answer
    // *"no editor in this process"* for a capability that is built but needs a
    // running editor, and `place-watch` is `S8` — genuinely unbuilt, so the
    // task id is the honest answer and stays the honest answer until `S8`
    // lands.
    let Outcome::Refused(Refusal::NotYetImplemented { task }) = ask(
        &host,
        Action::Watch(phosphor_core::action::WatchAction::PlaceWatch {
            anchor: phosphor_core::request::Target::Cursor {},
            expr: "1 + 1".to_owned(),
        }),
    ) else {
        panic!("an unbuilt capability names the task that builds it");
    };
    assert_eq!(task, "T077");
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
    let written = std::fs::read_to_string(config.join("init.scm")).expect("the file exists now");
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
    let written = std::fs::read_to_string(config.join("init.scm")).expect("the file exists now");
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
    let (mut layer, host) = booted();
    let source = "(+ 1 2)";
    let door = Vm {
        layer: &mut layer,
        host: &host,
    }
    .eval(source);

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
            backward: None,
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
    // **Search was the half `T058` did not build, and `T110` built it.** This
    // asserted a refusal naming `T110` — the third task id this one sentence
    // has carried, after `T058` and before it `T049`. It opens like the other
    // two now, and what is worth asserting is the thing that is *different*
    // about it: the direction rides along, and the anchor does not.
    assert!(
        matches!(
            editing.apply(&open(phosphor_core::request::PromptKind::Search)),
            Outcome::Done(_)
        ),
        "a search raises the prompt line like the other two kinds"
    );
    assert_eq!(
        editing.prompt,
        Some(phosphor_core::request::PromptKind::Search),
    );
    assert_eq!(
        editing.searching,
        Some(false),
        "`/` walks forward; `?` is the same capability with `backward` set"
    );
    // **The anchor is cleared rather than carried.** `1c`'s chip names a range
    // a message to claude is *about*; a search is about a position, so a chip
    // here would be the last prompt's range still on screen.
    assert_eq!(editing.anchor, None, "a search carries no anchor chip");
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
        review: None,
        peek: None,
        disk: None,
        change_diff: None,
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

// ---------------------------------------------------------------------------
// `T111` — the editor snapshot the queries read
// ---------------------------------------------------------------------------

/// The snapshot every `T111` editor query answers from, over one buffer.
fn snapshot_of(text: &str) -> super::EditorSnapshot {
    snapshot_carrying(text, BTreeMap::new())
}

/// The same, with a text map carried in from a previous frame.
fn snapshot_carrying(
    text: &str,
    carried: BTreeMap<u64, (u64, Vec<String>)>,
) -> super::EditorSnapshot {
    let bench = editing(text);
    let (buffers, _) = Buffers::new(bench.editing);
    super::editor_snapshot(
        &buffers,
        &bench.panes,
        phosphor_core::request::EditMode::Normal,
        "",
        &super::builtin("phosphor-dark").expect("a shipped theme"),
        "phosphor-dark",
        Surface::Buffer,
        Vec::new(),
        carried,
    )
}

/// **The snapshot describes the buffer that is actually open** (`T111`).
///
/// The fourteen editor queries are reads of this structure and nothing else, so
/// a snapshot that described the wrong buffer would make every one of them
/// wrong at once while each still *answered* — which is exactly the failure the
/// `--eval` walk in `parity.rs` cannot see, because an empty answer is a pass
/// there.
#[test]
fn the_snapshot_describes_the_open_buffer() {
    let held = snapshot_of("one\ntwo\nthree\n");

    assert_eq!(held.buffers.len(), 1, "one buffer is open");
    assert_eq!(
        held.focused,
        Some(0),
        "and it is the focused one — `buffer` with no argument answers about it"
    );
    // Three lines, not four: a trailing newline ends the last line rather than
    // starting an empty one, which is what `str::lines` means and what a person
    // counting the rows on screen would say.
    assert_eq!(
        super::field_of(&held.buffers[0], "lines"),
        Value::Int(3),
        "the line count is the buffer's, not the rope's byte length"
    );
    assert_eq!(
        super::field_of(&held.buffers[0], "dirty"),
        Value::Bool(false),
        "nothing has been typed into it"
    );
    assert_eq!(held.mode, "normal", "the mode is the machine's spelling");

    // The cursor is **1-based**, which is what `1a` and `8e` draw and what the
    // `12:1` counter means. A 0-based answer here would put every `goto`
    // composed in Steel one line off.
    let pane = held.panes.get(&0).expect("the focused pane is described");
    assert_eq!(
        super::field_of(&super::field_of(pane, "cursor"), "line"),
        Value::Int(1),
        "line 1, not line 0"
    );
    assert_eq!(
        super::field_of(&super::field_of(pane, "cursor"), "column"),
        Value::Int(1),
        "column 1, not column 0"
    );
    // No selection is `Null` rather than an empty span at the cursor — an empty
    // span reads as a real selection of nothing.
    assert_eq!(
        super::field_of(pane, "selection"),
        Value::Null,
        "there is no live selection"
    );

    assert_eq!(
        held.refs.get("focused"),
        Some(&0),
        "`{{focused}}` resolves, so `(cursor (hash \"kind\" \"focused\"))` has a pane"
    );
    assert_eq!(
        super::field_of(&held.theme, "slug"),
        Value::Text("phosphor-dark".to_owned()),
        "the theme answers the slug the frame was drawn with"
    );
}

/// **The text is copied only when the edit stream moved** (`T111`).
///
/// This is the assertion that the guard is *consulted* rather than decorative,
/// and it is deliberately built so that a snapshot which rebuilt the text
/// unconditionally would fail: the carried entry claims the same edit counter
/// the buffer really has, and holds text the buffer has never contained. A
/// rebuild overwrites it; the guard keeps it.
///
/// **Why it matters enough to test.** Without the guard this is a full copy of
/// every open buffer on every frame, which would make an idle editor's cost a
/// function of file size — the exact shape `HostState::transcript` already
/// refuses one field over. With it, the cost is one copy per committed edit
/// batch, which is the price the LSP document sync in the same loop already
/// pays off the same counter.
#[test]
fn the_snapshot_reuses_text_the_edit_counter_says_has_not_moved() {
    let mut carried = BTreeMap::new();
    // Edit counter 0 is what an untouched buffer reports, so this entry claims
    // to be current — and its text is a sentinel no buffer here ever held.
    carried.insert(0_u64, (0_u64, vec!["carried, not rebuilt".to_owned()]));

    let held = snapshot_carrying("one\ntwo\nthree\n", carried);

    assert_eq!(
        held.text.get(&0).map(|(_, lines)| lines.clone()),
        Some(vec!["carried, not rebuilt".to_owned()]),
        "the counter had not moved, so the previous frame's copy was kept"
    );
    // And the row's line count is read off the same map, so the two cannot
    // disagree about a buffer — one derivation, not two.
    assert_eq!(
        super::field_of(&held.buffers[0], "lines"),
        Value::Int(1),
        "the line count comes off the text that was published"
    );
}

/// **A closed buffer's text leaves the map** (`T111`).
///
/// Ids are never reused, so a stale entry could never answer about the wrong
/// file — but it would never be freed either, and a long session that opened
/// and closed many files would carry every one of them forever. `retain` is the
/// whole fix and this is what proves it runs.
#[test]
fn the_snapshot_forgets_a_buffer_that_is_no_longer_open() {
    let mut carried = BTreeMap::new();
    carried.insert(0_u64, (0_u64, vec!["current".to_owned()]));
    carried.insert(99_u64, (7_u64, vec!["closed long ago".to_owned()]));

    let held = snapshot_carrying("one\n", carried);

    assert!(
        held.text.contains_key(&0),
        "the open buffer is still described"
    );
    assert!(
        !held.text.contains_key(&99),
        "a buffer nothing has open is dropped rather than carried for the session"
    );
}

// ---------------------------------------------------------------------------
// `T094` — the editor layer, reloaded
// ---------------------------------------------------------------------------

/// The value an evaluation produced, or [`None`] if it did not produce one.
///
/// A refusal and a raise both answer `None` here, which is right for every
/// caller below: each is asking *"what does this global say now"*, and a form
/// that did not run has not said anything.
fn answered(outcome: Outcome) -> Option<Value> {
    match outcome {
        Outcome::Done(receipt) => Some(receipt.value),
        Outcome::Refused(_) | Outcome::Raised(_) => None,
    }
}

/// A minimal runtime tree: an `init.scm` naming one file, and that file.
///
/// **Minimal rather than a copy of the shipped layer**, because what is being
/// tested is the *reload*, and a fifteen-file tree would make every assertion
/// here depend on all fifteen still parsing. The load order is the mechanism
/// under test — `init.scm` reads it once at boot, which is exactly why a
/// reload is a different thing from evaluating a form.
fn tiny_layer(root: &Path, body: &str) {
    std::fs::create_dir_all(root).expect("a runtime tree");
    std::fs::write(
        root.join("init.scm"),
        "(define phosphor/boot-files (list \"extra.scm\"))\n",
    )
    .expect("an init.scm");
    std::fs::write(root.join("extra.scm"), body).expect("a layer file");
}

/// **A reload picks up a file edited since boot** (`T094`).
///
/// Invariant 1 is *"the editor layer is Steel in `runtime/*.scm`, **redefinable
/// at runtime**"*, and `CP-2` is the checkpoint that asks whether that is true.
/// It was not: `init.scm` reads the load order once at boot and the REPL
/// evaluates *forms*, and neither picks up a file you have since changed.
///
/// **The before-half is what makes this a test rather than a demonstration.**
/// The same global is read twice, so a `reload` that did nothing at all would
/// leave the second read answering what the first did.
#[test]
fn a_reload_runs_the_load_order_again_against_the_file_on_disk() {
    let root = scratch("t094-reload");
    let config = scratch("t094-reload-config");
    tiny_layer(&root, "(define phosphor/probe \"before\")\n");

    let (mut layer, host) = booted_with_config(Some(&root), &config);
    assert_eq!(
        answered(layer.evaluate("phosphor/probe")),
        Some(Value::Text("before".to_owned())),
        "the booted layer defines the global"
    );

    // The file changes on disk, the way a person editing their own layer
    // changes it. Nothing has restarted.
    std::fs::write(
        root.join("extra.scm"),
        "(define phosphor/probe \"after\")\n",
    )
    .expect("the layer is writable");

    let units = layer
        .reload(Some(&root), &host)
        .expect("a clean layer reloads");
    // `init.scm` is a unit too — the load order is a file that runs, not a
    // manifest that is parsed — so a one-entry `phosphor/boot-files` is two.
    assert_eq!(units, 2, "init.scm and the one file it names");
    assert_eq!(
        answered(layer.evaluate("phosphor/probe")),
        Some(Value::Text("after".to_owned())),
        "the reloaded layer is the file that is on disk now"
    );
}

/// **A broken file leaves the previous layer standing** (`T094`).
///
/// The requirement that shapes the implementation. The new runtime is built
/// *beside* the old one and swapped in only if its boot produced no fault —
/// reloading in place and repairing on failure cannot work, because half the
/// load order has already run by the time the fault appears and there is
/// nothing to roll back to.
///
/// **What is asserted is that the old layer still answers**, not merely that
/// the reload reported a failure. An editor that returned an error and left
/// itself with no keymap would pass the weaker claim.
#[test]
fn a_broken_reload_keeps_the_layer_that_was_working() {
    let root = scratch("t094-broken");
    let config = scratch("t094-broken-config");
    tiny_layer(&root, "(define phosphor/probe \"before\")\n");

    let (mut layer, host) = booted_with_config(Some(&root), &config);

    // An unbalanced paren: the ordinary way a hand-edited layer breaks.
    std::fs::write(
        root.join("extra.scm"),
        "(define phosphor/probe \"after\"\n(define phosphor/other 1)\n",
    )
    .expect("the layer is writable");

    let report = layer
        .reload(Some(&root), &host)
        .expect_err("a layer that does not parse is not swapped in");
    assert!(
        !report.faults.is_empty(),
        "the failure carries what went wrong, for the same float a broken init.scm draws"
    );
    assert_eq!(
        answered(layer.evaluate("phosphor/probe")),
        Some(Value::Text("before".to_owned())),
        "the layer that was working is the layer that is still running"
    );
}

/// **A reload re-runs the user's own layer, not just the shipped tree**
/// (`T094`, §34).
///
/// `stack` loads three things in order — the shipped tree, the file you
/// hand-wrote, then the file `persist!` wrote — and a reload that re-ran only
/// the first would silently drop the other two. It is a live hazard rather
/// than a hypothetical: `Layer::after_boot` exists to stop a file running
/// *twice within one boot*, so a reload that forgot to clear it would skip the
/// user's layer as already-loaded and leave the editor missing exactly the
/// customisations the person just asked to reload.
#[test]
fn a_reload_runs_the_users_own_layer_again() {
    let root = scratch("t094-user");
    let config = scratch("t094-user-config");
    tiny_layer(&root, "(define phosphor/probe \"shipped\")\n");
    // **The config *home* already ends in `phosphor`** — `config::config_dir`
    // resolves `$XDG_CONFIG_HOME/phosphor`, and `AppHost::user_layer` joins the
    // bare file name onto it. Writing to `<config>/phosphor/init.scm` here put
    // the file one directory below where the layer looks, and the boot quietly
    // loaded nothing; the first version of this test asserted the reload and
    // would have passed on an editor that never ran the user's file at all.
    std::fs::create_dir_all(&config).expect("a config home");
    std::fs::write(
        config.join(super::INIT),
        "(define phosphor/mine \"yours\")\n",
    )
    .expect("a user layer");

    let (mut layer, host) = booted_with_config(Some(&root), &config);
    assert_eq!(
        answered(layer.evaluate("phosphor/mine")),
        Some(Value::Text("yours".to_owned())),
        "the boot ran the user's file"
    );

    std::fs::write(
        config.join(super::INIT),
        "(define phosphor/mine \"still yours\")\n",
    )
    .expect("the user layer is writable");

    layer
        .reload(Some(&root), &host)
        .expect("a clean layer reloads");
    assert_eq!(
        answered(layer.evaluate("phosphor/mine")),
        Some(Value::Text("still yours".to_owned())),
        "and the reload ran it again, at its current contents"
    );
}

// ---------------------------------------------------------------------------
// `T095` — history maintenance
// ---------------------------------------------------------------------------

/// **A checkpoint id round-trips back to that state** (`T095`).
///
/// `UndoTree::goto` and `CheckpointId` both existed and nothing routed one to
/// the other, so the id was a number an agent could hold and not spend. This is
/// what makes *an agent turn* a unit of undo — the shape `T073`'s jj timeline
/// reads — because a turn can record the checkpoint it began at and coming back
/// is one Action rather than a guessed number of `u`.
///
/// **Two edits, not one.** With a single edit, "return to the checkpoint" and
/// "undo once" are the same movement and the test could not tell a `goto` from
/// a `u`. The second edit is what makes the walk have to cross two nodes.
#[test]
fn a_checkpoint_id_returns_the_buffer_to_that_state() {
    let mut bench = editing("one\n");

    bench.editing.editor.set_cursor(4);
    bench.apply(&Action::Buffer(
        phosphor_core::action::BufferAction::Insert {
            at: Position { line: 1, column: 4 },
            text: "-two".to_owned(),
        },
    ));
    bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::CommitUndoGroup {},
    ));
    // The point to come back to, taken the way a caller would: the tree's
    // current node after the edit it wants to keep.
    let checkpoint = bench.editing.timeline.tree.current();
    let kept = bench.editing.contents();

    bench.apply(&Action::Buffer(
        phosphor_core::action::BufferAction::Insert {
            at: Position { line: 1, column: 8 },
            text: "-three".to_owned(),
        },
    ));
    bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::CommitUndoGroup {},
    ));
    assert_ne!(
        bench.editing.contents(),
        kept,
        "the second edit moved the buffer off the checkpoint"
    );

    let outcome = bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::UndoToCheckpoint {
            checkpoint: phosphor_core::request::CheckpointId(checkpoint.0),
        },
    ));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert_eq!(
        bench.editing.contents(),
        kept,
        "the checkpoint id put the buffer back where it was"
    );
}

/// **An id the tree never minted is `NoSuchTarget`** (`T095`).
///
/// Not a `Declined`, and the distinction is the vocabulary's own: `NoSuchTarget`
/// is documented as *"a stale id from an agent working off an old query"*, which
/// is exactly who asks this. A decline would put the same fact in prose that no
/// door could match on.
#[test]
fn an_unminted_checkpoint_is_no_such_target() {
    let mut bench = editing("one\n");
    let outcome = bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::UndoToCheckpoint {
            checkpoint: phosphor_core::request::CheckpointId(9_999),
        },
    ));
    assert!(
        matches!(outcome, Outcome::Refused(Refusal::NoSuchTarget)),
        "{outcome:?}"
    );
}

/// **A buffer with no journal says so rather than pretending** (`T095`).
///
/// A scratch buffer, and a workspace with no state directory, both reach this —
/// and neither is a failure: `Timeline::detached`'s own doc says *"a session
/// that cannot persist still undoes"*. What would be wrong is answering `#ok` to
/// a compaction that did not happen, because a script sweeping histories would
/// read that as success.
#[test]
fn compacting_a_buffer_with_no_journal_declines_by_saying_so() {
    let mut bench = editing("one\n");
    let outcome = bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::CompactHistory {
            target: phosphor_core::request::Target::Cursor {},
        },
    ));
    let Outcome::Refused(Refusal::Declined { reason }) = outcome else {
        panic!("a buffer with nowhere to persist has no history to compact");
    };
    assert!(reason.contains("no history on disk"), "{reason}");
}

/// **`compact-history` names whose history, and says so when it cannot** (`T095`).
///
/// The row takes a `Target` and this arm holds one buffer, so anything that is
/// not *this* buffer is a question it cannot answer — a `Log` is keyed on a
/// file, and reaching another buffer's is the loop's. Refusing by saying which
/// scope it does work on is the honest half; silently sweeping the focused
/// buffer when asked about a different file would be the dangerous one.
#[test]
fn compacting_names_the_scope_it_can_reach() {
    let mut bench = editing("one\n");
    let outcome = bench.apply(&Action::History(
        phosphor_core::action::HistoryAction::CompactHistory {
            target: phosphor_core::request::Target::File {
                path: PathBuf::from("somewhere/else.rs"),
            },
        },
    ));
    let Outcome::Refused(Refusal::Declined { reason }) = outcome else {
        panic!("a path that is not this buffer is not this arm's to sweep");
    };
    assert!(reason.contains("focused buffer"), "{reason}");
}
