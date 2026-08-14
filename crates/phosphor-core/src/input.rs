//! The input machine: keystrokes in, `Action`s out, no terminal.
//!
//! `edtui` was dropped for two reasons the `T009` spike found and `Q3` records:
//! its register model *"cannot express numeric counts or named registers"*, and
//! our keymaps live in Steel, which makes its 185-entry compile-time table dead
//! weight. Both are answered here — counts and registers are **state in this
//! machine**, and the table is a [`table::Keymap`] asked at runtime — and
//! neither is retrofitted.
//!
//! # The shape
//!
//! ```text
//!   Key ─▶ Machine::feed(key, &mut keymap, &text) ─▶ Vec<Action>
//!            │                 │            │
//!            │                 │            └── text.rs: the buffer, read-only.
//!            │                 │                `dw` needs a span and `Buffer::Delete`
//!            │                 │                takes one.
//!            │                 └── table.rs: what a key *plays* — a role, not a
//!            │                     closure, so `T033` can send one from scheme.
//!            └── the pending count, register and operator. Not decoration on an
//!                Action: the statusline reads them, and `3` on its own is a
//!                state the machine is in.
//! ```
//!
//! Nothing here draws, reads a terminal, or applies anything. The binary
//! applies what comes out, which is what lets a test drive the whole grammar
//! headlessly — and what makes *"a scripted keystroke sequence produces the
//! expected Action stream"* (`T026`'s acceptance) a unit test rather than a
//! terminal session.
//!
//! # Six decisions, argued
//!
//! **1 · `3dd` is one Action, and the count folds into the operand.** Not three
//! deletes: the span `Buffer::Delete` carries covers all three lines. The
//! reason is undo. `T029` owns the undo tree and `History::CommitUndoGroup`
//! *"closes the current undo group explicitly"* — if a count emitted three
//! Actions, `u` would either undo one third of a `3dd` or the machine would
//! have to teach the undo model to group them, which puts the grouping rule in
//! two places. One keystroke sequence, one edit, one undo step.
//!
//! **2 · Counts and registers are state, and their transitions are Actions.**
//! `Input::SetCount`, `Input::SelectRegister` and `Input::CancelPending` exist
//! in the vocabulary (`action.rs`) precisely so the pending state is visible to
//! the statusline and to a door, rather than being a private field the UI has
//! to guess at. So the machine emits them *and* holds them; [`Machine::apply`]
//! is the same transition arriving from a door instead of a keyboard, so there
//! is one implementation of what `"a` means.
//!
//! **3 · An operator lowers to select-then-act.** `Buffer::Delete` takes a
//! `Span` but `Buffer::Yank` takes a `Target`, and `Target::Selection` is
//! late-bound (`request.rs`). So `dw` emits `SelectRange`, then `Yank` over the
//! selection, then `Delete` of the span, then `ClearSelection` — which is also
//! how the same keystroke behaves in visual mode, so there is one path rather
//! than two.
//!
//! **4 · The machine never moves the viewport.** Invariant 3's single writer is
//! `View::Scroll`, and the only keys that emit one are the scroll keys. A
//! cursor motion that leaves the screen is revealed by whoever applies it,
//! *through the same Action* — see `main.rs`'s `reveal`.
//!
//! **5 · `.` is a re-entry, not a recording.** [`Role::Repeat`](table::Role)
//! emits `Input::RepeatLast`, and the host feeds [`Machine::last_change`] back
//! through [`Machine::feed`] one key at a time, applying each key's Actions
//! before the next. Replaying the *Actions* would re-delete the same absolute
//! span; replaying the keys inside one `feed` would compute every position
//! against a buffer that had not moved yet. `Input::FeedKeys` is the same
//! mechanism with the keys named by a door, which is why it reads *"exactly as
//! if typed"*.
//!
//! **6 · An unbound key is an Action too.** `App::ShowUnknownKeyHint` carries
//! the key that was not bound; `T035` shows it once per session (`8e`). The
//! machine does not know that rule and does not need to.
//!
//! # The two seams, taken
//!
//! * **`T027` — kitty chords.** Two encodings of one chord reach [`key::Key`]
//!   and one key comes out of both (`key.rs` argues it); the *event kind*
//!   deliberately never lands, because [`key::Key`] is what a keymap is keyed
//!   by. What the wire cannot carry is the machine's problem instead:
//!   [`Machine::set_protocol`] turns on the one fallback a
//!   [`key::Protocol::Legacy`] terminal needs, and [`Machine::legacy_chord`]
//!   is the whole of it.
//! * **`T028` — agent nouns.** The four nouns parse and no-op — silently, and
//!   [`Machine::object_operand`] argues why against the alternative. `gsib`
//!   needed one thing more than a noun: `6d`'s *"`s` composes like an
//!   operator"* makes marking-seen an [`Operator`], which is what lets it take
//!   an object at all — on `gs`, not `s`, by Teej's ruling of 2026-08-12, since
//!   `s` is vim's substitute and that habit carries. `T049` resolves the nouns
//!   by giving `text::Text` a
//!   neighbour that can answer a region query — the seam is
//!   [`text::object_span`]'s signature, not this file.
//! * **`T033` — the keymap in scheme.** Taken: `runtime/keymaps.scm` is the
//!   whole keymap and the binary seeds an empty [`table::Table`].
//!   [`table::Role`] is the vocabulary a scheme binding names and
//!   [`table::Scope::name`] is the word it spells the scope with. The seed
//!   table that used to live in `input/vim.rs` was transcribed there and is
//!   **deleted**: two keymaps, one of which the binary never loaded, is how a
//!   test comes to prove a table nobody presses.
//!
//! # The next key is data, not a binding
//!
//! Three keys are followed by a keystroke the keymap must not see: `"` names a
//! register, `f`/`F`/`t`/`T` name a character to find, and `r` names the
//! character to write. That is one state ([`Awaiting`]) and one branch at the
//! top of [`Machine::step`], and it is why [`crate::request::Motion`] stays a
//! payload-free choice — **the character rides with the machine, not on the
//! tag**, so the CLI's `--motion` flag and the MCP schema's enum are still a
//! fixed set of names.
//!
//! Owned by `spine`.

pub mod key;
pub mod table;
pub mod text;

use crate::action::{
    Action, AppAction, BufferAction, HistoryAction, InputAction, MotionAction, RegionAction,
    ViewAction,
};
use crate::request::{
    CaseChange, EditMode, KeySeq, Motion, PaneRef, Position, RegisterName, ScrollRequest,
    SelectionKind, Span, Target, TextObject,
};

use key::{Key, Mods, Protocol};
use table::{Entry, Goto, Keymap, Operator, Resolution, Role, Scope};
use text::Text;

/// The pending state — *"the 3 in `3dd`, the `"a` in `"ayy`"*.
///
/// Public because the statusline reads it: a pending count with nothing after
/// it is a state the user is in and has to be able to see.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pending {
    /// The count typed before the operator.
    pub count: Option<u32>,
    /// The count typed after it — `2d3w` is six words.
    pub operator_count: Option<u32>,
    /// The register named by `"`.
    pub register: Option<RegisterName>,
    /// The operator waiting for an operand.
    pub operator: Option<Operator>,
    /// Keys typed so far in an unfinished sequence.
    pub keys: Vec<Key>,
}

impl Pending {
    /// The count that applies, with both halves multiplied — vim's rule.
    #[must_use]
    pub fn count(&self) -> u32 {
        let first = self.count.unwrap_or(1).max(1);
        let second = self.operator_count.unwrap_or(1).max(1);
        first.saturating_mul(second).max(1)
    }

    /// Whether a count or a register was typed and not used.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.count.is_none()
            && self.operator_count.is_none()
            && self.register.is_none()
            && self.operator.is_none()
            && self.keys.is_empty()
    }
}

/// Modes, counts, registers, operator-pending and text objects.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    /// What the terminal can say, from `T014`'s negotiation (`T027`).
    protocol: Protocol,
    mode: Mode,
    pending: Pending,
    /// `Some(inner)` between `i`/`a` and the object key — [`Scope::Object`].
    object: Option<bool>,
    /// Whether the operator now running arrived as `~` — the one fused key vim
    /// leaves the cursor *after* what it changed. See [`Machine::land`].
    fused_advance: bool,
    /// What the next key is data for, if it is data — set by `"`, `f` or `r`
    /// and cleared by the key that answers.
    awaiting: Option<Awaiting>,
    /// The last `f`/`F`/`t`/`T` and the character it looked for, which is all
    /// `;` and `,` are.
    last_find: Option<(Motion, char)>,
    /// Where a visual selection started.
    anchor: Option<Position>,
    /// Keys of the command being typed, for `.`.
    record: Vec<Key>,
    /// Whether that command has changed anything yet.
    record_changed: bool,
    /// The last command that changed the buffer.
    last_change: Option<Vec<Key>>,
}

/// What the next keystroke means, when it does not mean a binding.
///
/// One state for the three keys that take a literal, so the branch that reads
/// it is one branch. Every arm is left by exactly one keystroke: the key either
/// types a character, or it does not and the pending command is cancelled —
/// which is what makes `<esc>` after `f` do the obvious thing without an arm of
/// its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    /// `"` — the next key names a register.
    Register,
    /// `f`, `F`, `t`, `T` — the next key is the character to find.
    FindTarget(Motion),
    /// `r` — the next key is the character to write.
    ReplaceChar,
}

/// [`EditMode`] with a [`Default`], which the wire enum has no business having.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Mode(Option<EditMode>);

impl Mode {
    const fn get(self) -> EditMode {
        match self.0 {
            Some(mode) => mode,
            None => EditMode::Normal,
        }
    }
}

impl Machine {
    /// A machine in normal mode with nothing pending.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The mode the statusline's chip reads.
    #[must_use]
    pub const fn mode(&self) -> EditMode {
        self.mode.get()
    }

    /// Tells the machine what the terminal can say (`T027`).
    ///
    /// The host calls this once, with what `T014`'s negotiation settled:
    /// `phosphor_term::KeyboardProtocol::Kitty` is [`Protocol::Kitty`] and
    /// anything else is [`Protocol::Legacy`]. It is a setter rather than a
    /// constructor argument because a terminal can be renegotiated — a
    /// multiplexer reattaching to a different emulator is the case that will
    /// come up first — and because the default is the safe one, so a host that
    /// never calls it is not broken, only undegraded.
    pub const fn set_protocol(&mut self, protocol: Protocol) {
        self.protocol = protocol;
    }

    /// What the machine believes the terminal can say.
    #[must_use]
    pub const fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// The pending count, register, operator and keys.
    #[must_use]
    pub const fn pending(&self) -> &Pending {
        &self.pending
    }

    /// The keys of the last command that changed the buffer — what
    /// `Input::RepeatLast` feeds back (decision 5).
    #[must_use]
    pub fn last_change(&self) -> Option<KeySeq> {
        self.last_change.as_deref().map(key::notation_of)
    }

    /// One keystroke.
    ///
    /// The Actions come back in the order they must be applied; applying them
    /// out of order changes the meaning (`Yank` reads the selection `SelectRange`
    /// just set).
    pub fn feed(&mut self, pressed: Key, keymap: &mut dyn Keymap, text: &dyn Text) -> Vec<Action> {
        let mut out = Vec::new();
        self.record.push(pressed);
        self.step(pressed, keymap, text, &mut out);

        if out.iter().any(is_edit) {
            self.record_changed = true;
        }
        // A command is over when the machine is back in normal mode with
        // nothing pending. Only then is it worth repeating — and a command that
        // changed nothing is not a change to repeat.
        //
        // **A key that is waiting for its literal is not "nothing pending"**,
        // even though the count and the operator are clear: after the `r` of
        // `ra` the mode is still normal, and finishing the command there would
        // record `a` alone as the last change.
        if self.mode.get() == EditMode::Normal
            && self.pending.is_clear()
            && self.object.is_none()
            && self.awaiting.is_none()
        {
            if self.record_changed {
                self.last_change = Some(core::mem::take(&mut self.record));
            } else {
                self.record.clear();
            }
            self.record_changed = false;
        }
        out
    }

    /// The same transitions, arriving from a door rather than a keyboard.
    ///
    /// `set-mode`, `set-count` and `select-register` are `Deny` on MCP — they
    /// are the user's keyboard (`action.rs`) — but the Steel and CLI doors have
    /// them, and this is what makes them mean the same thing there.
    pub fn apply(&mut self, action: &InputAction) {
        match action {
            InputAction::SetMode { mode } => self.mode = Mode(Some(*mode)),
            InputAction::SetCount { count } => self.pending.count = Some(*count),
            InputAction::SelectRegister { register } => {
                self.pending.register = Some(register.clone());
            }
            InputAction::CancelPending {} => {
                self.pending = Pending::default();
                self.object = None;
                self.awaiting = None;
            }
            // Both are re-entries the host drives: it reads `last_change` or the
            // Action's own keys and feeds them back through `feed`.
            InputAction::FeedKeys { .. } | InputAction::RepeatLast { .. } => {}
            // The recorder is `T099`'s and is deliberately not built here. The
            // capture machinery it will grow out of is `record`/`record_changed`
            // above — the same stream `.` already keeps — but generalising it to
            // a named register is that task's, not the vocabulary's. Until then
            // this is a no-op, and because `apply` returns nothing the *host* is
            // the only place that can turn the call into a refusal naming
            // `T099`; a silent success here is the failure `T098` exists to end.
            InputAction::SetMacroRecording { .. } => {}
        }
    }

    /// The scope the next key is looked up in.
    fn scope(&self) -> Scope {
        if self.object.is_some() {
            Scope::Object
        } else {
            Scope::of(self.mode.get())
        }
    }

    fn step(
        &mut self,
        pressed: Key,
        keymap: &mut dyn Keymap,
        text: &dyn Text,
        out: &mut Vec<Action>,
    ) {
        if let Some(awaiting) = self.awaiting.take() {
            let Some(character) = pressed.typed() else {
                // A key that types nothing — `<esc>`, an arrow, a chord — is
                // not a literal, and the half-typed command goes with it.
                self.cancel(out);
                return;
            };
            match awaiting {
                Awaiting::Register => {
                    let register = RegisterName(character.to_string());
                    self.pending.register = Some(register.clone());
                    out.push(Action::Input(InputAction::SelectRegister { register }));
                }
                Awaiting::FindTarget(motion) => {
                    // `;` and `,` are this state, remembered. The character is
                    // stored before the motion runs, so a find that lands on
                    // nothing is still repeatable.
                    self.last_find = Some((motion, character));
                    let count = self.pending.count();
                    self.motion(motion, count, Some(character), text, out);
                }
                Awaiting::ReplaceChar => self.replace_char(character, text, out),
            }
            return;
        }

        let scope = self.scope();
        if self.count_digit(pressed, scope, out) {
            return;
        }

        self.pending.keys.push(pressed);
        let mut keys = self.pending.keys.clone();
        let mut answer = keymap.resolve(scope, &keys);
        // `T027`'s degradation, and the only place the protocol is read.
        if answer == Resolution::Unbound
            && let Some(chord) = self.legacy_chord(&keys)
        {
            let retried = keymap.resolve(scope, &chord);
            if retried != Resolution::Unbound {
                // The sequence continues in the spelling that answered: a
                // `Pending` here means the *next* key extends `<C-S-k>`, and
                // asking with the unshifted prefix again would lose it.
                self.pending.keys.clone_from(&chord);
                keys = chord;
                answer = retried;
            }
        }
        match answer {
            // Wait for the next key; `pending.keys` is what which-key draws.
            Resolution::Pending => {}
            // Arbitrary scheme ran. The machine emits nothing — and the frame
            // that follows is stale, which is the loop's rule, not this one's.
            Resolution::Ran => {
                self.pending.keys.clear();
                self.pending.count = None;
                self.pending.operator_count = None;
            }
            Resolution::Role(role) => {
                self.pending.keys.clear();
                self.role(role, text, out);
            }
            Resolution::Unbound => {
                self.pending.keys.clear();
                if scope == Scope::Insert {
                    for typed in &keys {
                        self.insert_key(*typed, text, out);
                    }
                } else {
                    out.push(Action::App(AppAction::ShowUnknownKeyHint {
                        key: key::notation_of(&keys),
                    }));
                    self.cancel(out);
                }
            }
        }
    }

    /// The same sequence with shift held on the key just pressed, when the
    /// terminal could not have told us whether it was (`T027`).
    ///
    /// **Three conditions, and each one is load-bearing.**
    ///
    /// * [`Protocol::Legacy`] only. Under kitty the answer arrived intact, and
    ///   retrying there would fire a `<C-S-k>` binding on a plain `<C-k>` — the
    ///   wrong command on the terminal that was telling the truth.
    /// * Ctrl only. A control byte has no case, so it is the one modifier that
    ///   loses the shift; alt sends the character itself (`<A-K>` arrives as a
    ///   capital), and shift alone *is* the character.
    /// * The plain form came back [`Resolution::Unbound`]. A binding on `<C-k>`
    ///   is what the user actually pressed and must win; the fallback only
    ///   reaches a chord that would otherwise be unreachable on this terminal,
    ///   so nothing is ever shadowed by it.
    ///
    /// The inherent ambiguity — `<C-k>` firing a `<C-S-k>` binding when the
    /// user held no shift — is the terminal's, not ours, and it is the strictly
    /// better half of the trade: the alternative is a documented chord that
    /// does nothing at all and no way to tell why.
    fn legacy_chord(&self, keys: &[Key]) -> Option<Vec<Key>> {
        if self.protocol != Protocol::Legacy {
            return None;
        }
        let (last, head) = keys.split_last()?;
        if !last.mods.has(Mods::CTRL) || last.mods.has(Mods::SHIFT) {
            return None;
        }
        let shifted = last.shifted();
        if shifted == *last {
            return None;
        }
        let mut chord = head.to_vec();
        chord.push(shifted);
        Some(chord)
    }

    /// A digit that is part of a count rather than a binding.
    ///
    /// `0` is a motion until a count is under way, which is the one rule that
    /// makes `d0` and `10d` both work.
    fn count_digit(&mut self, pressed: Key, scope: Scope, out: &mut Vec<Action>) -> bool {
        if !matches!(
            scope,
            Scope::Normal | Scope::Visual | Scope::OperatorPending
        ) || !self.pending.keys.is_empty()
        {
            return false;
        }
        let Some(digit) = pressed.typed().and_then(|typed| typed.to_digit(10)) else {
            return false;
        };
        let after_operator = self.pending.operator.is_some();
        let slot = if after_operator {
            &mut self.pending.operator_count
        } else {
            &mut self.pending.count
        };
        if digit == 0 && slot.is_none() {
            return false;
        }
        *slot = Some(slot.unwrap_or(0).saturating_mul(10).saturating_add(digit));
        out.push(Action::Input(InputAction::SetCount {
            count: self.pending.count(),
        }));
        true
    }

    /// One arm per role, deliberately: splitting the grammar across functions
    /// hides it.
    fn role(&mut self, role: Role, text: &dyn Text, out: &mut Vec<Action>) {
        let count = self.pending.count();
        match role {
            // `f`, `F`, `t`, `T` — the destination is not known until the next
            // key, so nothing is emitted and nothing is cleared: the count
            // typed before the `f` still applies to `3fx`.
            Role::Motion(motion) if text::is_find(motion) => {
                self.awaiting = Some(Awaiting::FindTarget(motion));
            }
            // `;` and `,` — the last find, and the same find the other way.
            Role::Motion(motion @ (Motion::RepeatFind | Motion::RepeatFindReverse)) => {
                match self.last_find {
                    Some((last, character)) => {
                        let repeated = if motion == Motion::RepeatFindReverse {
                            reversed(last)
                        } else {
                            last
                        };
                        self.motion(repeated, count, Some(character), text, out);
                    }
                    // Nothing to repeat. vim beeps; the pending command is
                    // dropped, which is what the statusline needs to hear.
                    None => self.cancel(out),
                }
            }
            Role::Motion(motion) => self.motion(motion, count, None, text, out),
            Role::Goto(goto) => self.goto(goto, text, out),
            Role::Operator(operator) => self.operator(operator, text, out),
            Role::Fused { operator, motion } => {
                self.pending.operator = Some(operator);
                // `~`, and only `~`: the case operator fused with `l` is the one
                // key that ends to the right of what it changed
                // (`change.txt:315-318`). `g~l` is the same operator over the
                // same motion, unfused, and takes the general rule — so the
                // fact rides on the fusion rather than on the operator.
                self.fused_advance =
                    operator == Operator::ToggleCase && motion == Motion::CharRight;
                if text::is_find(motion) {
                    self.awaiting = Some(Awaiting::FindTarget(motion));
                } else {
                    self.motion(motion, count, None, text, out);
                }
            }
            Role::Object { object, delimiter } => self.object_operand(object, delimiter, text, out),
            Role::Inner => self.object = Some(true),
            Role::Around => self.object = Some(false),
            Role::Enter(entry) => self.enter(entry, text, out),
            Role::Select(kind) => self.select(kind, text, out),
            Role::Paste { before } => {
                for _ in 0..count {
                    out.push(Action::Buffer(BufferAction::Paste {
                        at: Target::Cursor {},
                        register: self.pending.register.clone(),
                        before,
                    }));
                }
                out.push(Action::History(HistoryAction::CommitUndoGroup {}));
                self.clear(out);
            }
            Role::History { redo } => {
                out.push(Action::History(if redo {
                    HistoryAction::Redo { count }
                } else {
                    HistoryAction::Undo { count }
                }));
                self.clear(out);
            }
            Role::Scroll(request) => {
                out.push(Action::View(ViewAction::Scroll {
                    request: scaled(request, count),
                    pane: PaneRef::Focused {},
                }));
                self.clear(out);
            }
            Role::Repeat => {
                out.push(Action::Input(InputAction::RepeatLast { count }));
                self.clear(out);
            }
            Role::Escape => self.escape(out),
            Role::Register => self.awaiting = Some(Awaiting::Register),
            Role::ReplaceChar => self.awaiting = Some(Awaiting::ReplaceChar),
            Role::Run(actions) => {
                out.extend(actions);
                self.clear(out);
            }
        }
    }

    /// A motion: the operand of a pending operator, an extension of a live
    /// selection, or a cursor move — in that order of precedence.
    ///
    /// `target` is the character `f`, `F`, `t` and `T` were given, and [`None`]
    /// for every other motion.
    fn motion(
        &mut self,
        motion: Motion,
        count: u32,
        target: Option<char>,
        text: &dyn Text,
        out: &mut Vec<Action>,
    ) {
        if let Some(operator) = self.pending.operator {
            match text::motion_span_with_target(text, text.cursor(), motion, count, target) {
                Some((span, kind)) => self.operate(operator, span, kind, text, out),
                // A motion with no span — a search with no search state, an
                // `f` that found nothing — leaves the operator waiting rather
                // than deleting something else.
                None => self.cancel(out),
            }
            return;
        }
        // **A find resolves here rather than at the applier.** `MoveCursor`
        // carries a `Motion` and a count and no character, so a find would
        // arrive at the host without the one thing it needs; the machine knows
        // the destination already, and an absolute `SetCursor` is how it says
        // so — the same path `gg` and `G` take, for the same reason.
        if target.is_some() {
            let position =
                text::cursor_after_with_target(text, text.cursor(), motion, count, target);
            self.jump(position, text, out);
            return;
        }
        if self.anchor.is_some() {
            out.push(Action::Motion(MotionAction::ExtendSelection {
                motion,
                count,
            }));
        } else {
            out.push(Action::Motion(MotionAction::MoveCursor { motion, count }));
        }
        self.clear(out);
    }

    /// `gg` / `G` — a count names a line rather than repeating.
    fn goto(&mut self, goto: Goto, text: &dyn Text, out: &mut Vec<Action>) {
        let counted = self.pending.count.is_some() || self.pending.operator_count.is_some();
        let line = if counted {
            self.pending.count()
        } else {
            match goto {
                Goto::First => 1,
                Goto::Last => text.lines().max(1),
            }
        };
        if let Some(operator) = self.pending.operator {
            let span = text::line_span(
                text,
                text.cursor().line.min(line),
                text.cursor().line.max(line),
            );
            self.operate(operator, span, SelectionKind::Line, text, out);
            return;
        }
        let position = text::first_non_blank(text, line.clamp(1, text.lines().max(1)));
        self.jump(position, text, out);
    }

    /// A move to a position the machine worked out itself.
    ///
    /// Two callers, and they are the two motions whose destination the *host*
    /// cannot recompute from the Action alone: `gg`/`G`, whose count names a
    /// line rather than a repetition, and the finds, whose character does not
    /// ride on the tag. A live selection is extended in the same breath,
    /// because `ExtendSelection` has the same missing argument.
    fn jump(&mut self, position: Position, text: &dyn Text, out: &mut Vec<Action>) {
        if let Some(anchor) = self.anchor {
            out.push(Action::Motion(MotionAction::SelectRange {
                span: span_between(anchor, position, text),
                kind: self.selection_kind(),
            }));
        }
        out.push(Action::Motion(MotionAction::SetCursor {
            position,
            buffer: None,
        }));
        self.clear(out);
    }

    /// `r{char}` — `count` characters under the cursor become one character.
    ///
    /// Refuses rather than truncates when the line is too short, which is
    /// vim's rule: `5rx` at three characters from the end changes nothing at
    /// all, and a partial replace would be an edit nobody asked for.
    fn replace_char(&mut self, character: char, text: &dyn Text, out: &mut Vec<Action>) {
        let count = self.pending.count();
        let cursor = text.cursor();
        let width = text
            .line(cursor.line)
            .map_or(0, |row| u32::try_from(row.chars().count()).unwrap_or(0));
        if cursor.column.saturating_add(count) > width.saturating_add(1) {
            self.cancel(out);
            return;
        }
        out.push(Action::Buffer(BufferAction::Replace {
            span: Span {
                start: cursor,
                end: Position {
                    column: cursor.column + count,
                    ..cursor
                },
            },
            text: character.to_string().repeat(count as usize),
        }));
        // `r` replaces IN PLACE. Vim leaves the cursor on the character it just
        // wrote — on the *last* one for a count, so `3rx` ends two to the right
        // of where it started and `rx` does not move at all (change.txt: "replace
        // the character under the cursor"). Without this the applier's splice
        // leaves the cursor past the replacement, so every `r` drifted one cell
        // right and `rx` twice in a row skipped a character. Found by Teej at
        // `CP-3`, editing.
        //
        // Not `Machine::land`: that is the operator landing rule, and `r` is not
        // an operator — it takes no motion and no text object.
        out.push(set_cursor(text::clamp(
            text,
            Position {
                column: cursor.column.saturating_add(count).saturating_sub(1),
                ..cursor
            },
        )));
        out.push(Action::History(HistoryAction::CommitUndoGroup {}));
        self.clear(out);
    }

    /// An operator key: doubled, over a selection, or waiting for an operand.
    fn operator(&mut self, operator: Operator, text: &dyn Text, out: &mut Vec<Action>) {
        // `dd`, `yy`, `cc` — the doubled form is linewise over `count` lines.
        if self.pending.operator == Some(operator) {
            let first = text.cursor().line;
            let span = text::line_span(text, first, first + self.pending.count() - 1);
            self.operate(operator, span, SelectionKind::Line, text, out);
            return;
        }
        // In visual mode the operand is already on screen.
        if let Some(anchor) = self.anchor {
            let kind = self.selection_kind();
            let span = match kind {
                SelectionKind::Line => {
                    let cursor = text.cursor();
                    text::line_span(
                        text,
                        anchor.line.min(cursor.line),
                        anchor.line.max(cursor.line),
                    )
                }
                _ => span_between(anchor, text.cursor(), text),
            };
            self.operate(operator, span, kind, text, out);
            return;
        }
        self.pending.operator = Some(operator);
        self.set_mode(EditMode::OperatorPending, out);
    }

    /// A text object as an operand — `ci(`, `vip`.
    fn object_operand(
        &mut self,
        object: TextObject,
        delimiter: Option<char>,
        text: &dyn Text,
        out: &mut Vec<Action>,
    ) {
        let inner = self.object.take().unwrap_or(true);
        let count = self.pending.count();
        // The Action is emitted whether or not this side can resolve it: it is
        // the record of what was asked for, and `T049` is what makes the four
        // agent nouns answer.
        out.push(Action::Motion(MotionAction::SelectObject {
            object,
            inner,
            count,
            delimiter,
        }));
        let resolved = text::object_span(text, text.cursor(), object, inner, count, delimiter);
        match (self.pending.operator, resolved) {
            (Some(operator), Some((span, kind))) => self.operate(operator, span, kind, text, out),
            // No span. **`T028`'s no-op, and the choice is deliberate: it is
            // silent, not spoken.** The two candidates were a keystroke that
            // vanishes and one that says *"no regions yet"*, and three things
            // pick the first:
            //
            // * It is what vim does. `di(` outside any parentheses aborts the
            //   operator and says nothing, and `CP-3` is *"vim habits should
            //   carry without thinking about it"* — an editor that popped a
            //   sentence at a text object that found nothing would be the less
            //   familiar of the two, not the more helpful.
            // * It is not actually silent. `SelectObject` above is the record
            //   of what was asked for, and the `CancelPending` below is what
            //   the statusline reads — the `d` visibly stops waiting. What is
            //   missing is a *sentence*, and the vocabulary already has the
            //   right one in the right place: `Refusal::NotYetImplemented`
            //   naming the task, which is the applier's to give when `T049`
            //   resolves these against the store.
            // * The alternative cannot be told from the real thing. At `S5` a
            //   file with no unseen regions is a normal state, and `viu` there
            //   must do exactly what it does here — so a message saying "not
            //   built yet" would have to be removed again, and one saying "none
            //   here" would be a lie today.
            (Some(_), None) => self.cancel(out),
            (None, Some((span, kind))) => {
                if self.anchor.is_some() {
                    out.push(Action::Motion(MotionAction::SelectRange { span, kind }));
                }
                self.clear(out);
            }
            (None, None) => self.clear(out),
        }
    }

    /// Select, act, land, clear — the one path an operator takes (decision 3).
    fn operate(
        &mut self,
        operator: Operator,
        span: Span,
        kind: SelectionKind,
        text: &dyn Text,
        out: &mut Vec<Action>,
    ) {
        let register = self.pending.register.clone();
        out.push(Action::Motion(MotionAction::SelectRange { span, kind }));
        match operator {
            Operator::Delete | Operator::Change => {
                out.push(Action::Buffer(BufferAction::Yank {
                    target: Target::Selection {},
                    register,
                }));
                out.push(Action::Buffer(BufferAction::Delete { span }));
            }
            Operator::Yank => out.push(Action::Buffer(BufferAction::Yank {
                target: Target::Selection {},
                register,
            })),
            Operator::Indent | Operator::Dedent => {
                out.push(Action::Buffer(BufferAction::Indent {
                    target: Target::Selection {},
                    delta: if operator == Operator::Indent { 1 } else { -1 },
                }));
            }
            Operator::ToggleComment => out.push(Action::Buffer(BufferAction::ToggleComment {
                target: Target::Selection {},
            })),
            // `gU`, `gu`, `g~` — and `~`, which is the same operator fused with
            // `l`. One capability, three words (`request::CaseChange`).
            Operator::Upper | Operator::Lower | Operator::ToggleCase => {
                out.push(Action::Buffer(BufferAction::SetCase {
                    target: Target::Selection {},
                    case: match operator {
                        Operator::Upper => CaseChange::Upper,
                        Operator::Lower => CaseChange::Lower,
                        _ => CaseChange::Toggle,
                    },
                }));
            }
            // `sib`, and `s` over any other operand. Nothing is yanked and
            // nothing is deleted: seen-state is not the buffer.
            Operator::MarkSeen => out.push(Action::Region(RegionAction::MarkSeen {
                target: Target::Selection {},
            })),
        }
        self.land(operator, span, kind, text, out);
        out.push(Action::Motion(MotionAction::ClearSelection {}));
        self.anchor = None;
        if operator == Operator::Change {
            // The undo group stays open: what you type next is part of this
            // change, and `<esc>` closes it.
            self.set_mode(EditMode::Insert, out);
        } else {
            self.set_mode(EditMode::Normal, out);
            // A yank changes nothing, so there is no group to close. Closing
            // one anyway would put an empty step in `T029`'s undo tree — and
            // `s` is the same case for the same reason: seen-state is not text,
            // so `u` after `sib` must undo whatever edit came before it.
            if !matches!(operator, Operator::Yank | Operator::MarkSeen) {
                out.push(Action::History(HistoryAction::CommitUndoGroup {}));
            }
        }
        self.pending = Pending::default();
        self.object = None;
        self.fused_advance = false;
    }

    /// Where an operator leaves the cursor, said out loud.
    ///
    /// **The rule is vim's and it is written down.** `motion.txt:71-74`
    /// (`*operator-resulting-pos*`): *"After applying the operator the cursor is
    /// mostly left at the start of the text that was operated upon. For example,
    /// `yfe` doesn't move the cursor, but `yFe` moves the cursor leftwards to
    /// the `e` where the yank started."* Nothing in this machine used to say so,
    /// and the cursor ended wherever the *applier* left it — which for `gUiw` is
    /// the far end of the word, because a case change is a splice and a splice
    /// ends where it ends. A rule that lives in the applier is a rule each
    /// applier can get differently; this is the same argument as
    /// [`text::cased`].
    ///
    /// Three exceptions, each documented, and one refusal:
    ///
    /// * **A linewise yank keeps its column.** *"With a linewise yank command
    ///   the cursor is put in the first line, but the column is unmodified"*
    ///   (`change.txt:1254-1255`). A charwise yank is the general rule already —
    ///   *"the first yanked character that is closest to the start of the
    ///   buffer"* (`change.txt:1246-1249`) — which is why `yl` does not move the
    ///   cursor and `yh` does.
    /// * **`~` moves right.** *"Switch case of the character under the cursor
    ///   and move the cursor to the right. If a \[count\] is given, do that many
    ///   characters"* (`change.txt:315-318`, `'notildeop'`, the default).
    /// * **`'startofline'`, for a linewise `d`, `<` and `>` only.** *"the
    ///   commands listed below move the cursor to the first non-blank of the
    ///   line … `d`, `<<`, `==` and `>>` with a linewise operator"*
    ///   (`options.txt:8260-8266`, on by default), and `motion.txt:75` says it
    ///   applies to those and to nothing else — `c` and the case trio are not on
    ///   the list. It is asked for as a `MoveCursor` rather than computed here,
    ///   because *which* line the cursor lands on only exists once the delete
    ///   has been applied and this machine reads the buffer before it.
    ///
    /// The refusal is `gs`. Mark-seen is not vim's, it changes no text, and a
    /// keystroke that moved the cursor for neither reason would be an invention.
    /// `gc` takes the general rule by analogy — vim has no comment operator, so
    /// there is nothing to cite and this is a phosphor ruling rather than a
    /// reading.
    fn land(
        &self,
        operator: Operator,
        span: Span,
        kind: SelectionKind,
        text: &dyn Text,
        out: &mut Vec<Action>,
    ) {
        if operator == Operator::MarkSeen {
            return;
        }
        if self.fused_advance {
            out.push(set_cursor(text::clamp(text, span.end)));
            return;
        }
        let position = if operator == Operator::Yank && kind == SelectionKind::Line {
            Position {
                line: span.start.line,
                column: text.cursor().column,
            }
        } else {
            span.start
        };
        out.push(set_cursor(position));
        if kind == SelectionKind::Line
            && matches!(
                operator,
                Operator::Delete | Operator::Indent | Operator::Dedent
            )
        {
            out.push(Action::Motion(MotionAction::MoveCursor {
                motion: Motion::FirstNonBlank,
                count: 1,
            }));
        }
    }

    /// `i`, `a`, `I`, `A`, `o`, `O`, `R`.
    fn enter(&mut self, entry: Entry, text: &dyn Text, out: &mut Vec<Action>) {
        let cursor = text.cursor();
        let indent: String = text
            .line(cursor.line)
            .unwrap_or_default()
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect();
        let width = u32::try_from(indent.chars().count()).unwrap_or(0);
        match entry {
            Entry::Before => {}
            Entry::After => out.push(set_cursor(Position {
                column: cursor.column + 1,
                ..cursor
            })),
            Entry::LineStart => out.push(set_cursor(text::first_non_blank(text, cursor.line))),
            Entry::LineEnd => out.push(set_cursor(text::end_of_line(text, cursor.line))),
            Entry::OpenBelow => {
                out.push(Action::Buffer(BufferAction::Insert {
                    at: text::end_of_line(text, cursor.line),
                    text: format!("\n{indent}"),
                }));
            }
            Entry::OpenAbove => {
                out.push(Action::Buffer(BufferAction::Insert {
                    at: Position {
                        line: cursor.line,
                        column: 1,
                    },
                    text: format!("{indent}\n"),
                }));
                out.push(set_cursor(Position {
                    line: cursor.line,
                    column: width + 1,
                }));
            }
            Entry::Replace => {}
        }
        self.set_mode(
            if entry == Entry::Replace {
                EditMode::Replace
            } else {
                EditMode::Insert
            },
            out,
        );
        self.pending = Pending::default();
    }

    /// `v`, `V`, `<C-v>` — and the same key again leaves.
    fn select(&mut self, kind: SelectionKind, text: &dyn Text, out: &mut Vec<Action>) {
        let wanted = match kind {
            SelectionKind::Char => EditMode::VisualChar,
            SelectionKind::Line => EditMode::VisualLine,
            SelectionKind::Block => EditMode::VisualBlock,
        };
        if self.mode.get() == wanted {
            out.push(Action::Motion(MotionAction::ClearSelection {}));
            self.anchor = None;
            self.set_mode(EditMode::Normal, out);
            self.clear(out);
            return;
        }
        let cursor = text.cursor();
        self.anchor = Some(cursor);
        self.set_mode(wanted, out);
        out.push(Action::Motion(MotionAction::SelectRange {
            span: span_between(cursor, cursor, text),
            kind,
        }));
        self.pending = Pending::default();
    }

    /// A key in insert or replace mode that no binding wanted: text.
    fn insert_key(&mut self, pressed: Key, text: &dyn Text, out: &mut Vec<Action>) {
        let cursor = text.cursor();
        let typed = match pressed.code {
            key::Code::Named(key::Named::Enter) => Some("\n".to_owned()),
            // A literal tab. What a tab *inserts* is an option two reasonable
            // users differ on, which makes it `T033`'s `set-option!` rather than
            // a number invented here.
            key::Code::Named(key::Named::Tab) => Some("\t".to_owned()),
            key::Code::Named(key::Named::Backspace) => {
                if let Some(span) = back_span(text, cursor) {
                    out.push(Action::Buffer(BufferAction::Delete { span }));
                }
                None
            }
            key::Code::Named(key::Named::Delete) => {
                out.push(Action::Buffer(BufferAction::Delete {
                    span: Span {
                        start: cursor,
                        end: Position {
                            column: cursor.column + 1,
                            ..cursor
                        },
                    },
                }));
                None
            }
            _ => pressed.typed().map(|character| character.to_string()),
        };
        let Some(typed) = typed else { return };
        if self.mode.get() == EditMode::Replace {
            out.push(Action::Buffer(BufferAction::Replace {
                span: Span {
                    start: cursor,
                    end: Position {
                        column: cursor.column + 1,
                        ..cursor
                    },
                },
                text: typed,
            }));
        } else {
            out.push(Action::Buffer(BufferAction::Insert {
                at: cursor,
                text: typed,
            }));
        }
    }

    /// `<esc>`: out of insert, out of visual, or out of a half-typed command.
    fn escape(&mut self, out: &mut Vec<Action>) {
        match self.mode.get() {
            EditMode::Insert | EditMode::Replace => {
                self.set_mode(EditMode::Normal, out);
                out.push(Action::Motion(MotionAction::MoveCursor {
                    motion: Motion::CharLeft,
                    count: 1,
                }));
                out.push(Action::History(HistoryAction::CommitUndoGroup {}));
            }
            EditMode::VisualChar | EditMode::VisualLine | EditMode::VisualBlock => {
                out.push(Action::Motion(MotionAction::ClearSelection {}));
                self.anchor = None;
                self.set_mode(EditMode::Normal, out);
            }
            EditMode::OperatorPending => self.cancel(out),
            // Nothing to leave and nothing pending: vim beeps, and a stream
            // with an Action in it for that would be noise.
            EditMode::Normal => {
                if !self.pending.is_clear() || self.object.is_some() || self.awaiting.is_some() {
                    self.cancel(out);
                }
            }
        }
    }

    /// Drops everything half-typed. The Action is what the statusline reads.
    fn cancel(&mut self, out: &mut Vec<Action>) {
        out.push(Action::Input(InputAction::CancelPending {}));
        self.pending = Pending::default();
        self.object = None;
        self.awaiting = None;
        self.fused_advance = false;
        if self.mode.get() == EditMode::OperatorPending {
            self.set_mode(EditMode::Normal, out);
        }
    }

    /// Clears what a completed command consumed, without an Action: nothing was
    /// abandoned.
    fn clear(&mut self, out: &mut Vec<Action>) {
        if self.mode.get() == EditMode::OperatorPending {
            self.set_mode(EditMode::Normal, out);
        }
        self.pending = Pending::default();
        self.object = None;
        self.fused_advance = false;
    }

    fn set_mode(&mut self, mode: EditMode, out: &mut Vec<Action>) {
        if self.mode.get() == mode {
            return;
        }
        self.mode = Mode(Some(mode));
        out.push(Action::Input(InputAction::SetMode { mode }));
    }

    const fn selection_kind(&self) -> SelectionKind {
        match self.mode.get() {
            EditMode::VisualLine => SelectionKind::Line,
            EditMode::VisualBlock => SelectionKind::Block,
            _ => SelectionKind::Char,
        }
    }
}

/// `SetCursor`, which is how an absolute position is asked for.
fn set_cursor(position: Position) -> Action {
    Action::Motion(MotionAction::SetCursor {
        position,
        buffer: None,
    })
}

/// The span from `anchor` to `cursor`, inclusive of the character under the
/// cursor — which is what visual mode means by "selected".
fn span_between(anchor: Position, cursor: Position, text: &dyn Text) -> Span {
    let (start, end) = if (cursor.line, cursor.column) < (anchor.line, anchor.column) {
        (cursor, anchor)
    } else {
        (anchor, cursor)
    };
    let end = text::clamp(
        text,
        Position {
            column: end.column + 1,
            ..end
        },
    );
    let end = if end.column <= start.column && end.line == start.line {
        Position {
            column: start.column + 1,
            ..start
        }
    } else {
        end
    };
    Span { start, end }
}

/// One character back, crossing a line boundary — `<bs>`.
fn back_span(text: &dyn Text, cursor: Position) -> Option<Span> {
    if cursor.column > 1 {
        return Some(Span {
            start: Position {
                column: cursor.column - 1,
                ..cursor
            },
            end: cursor,
        });
    }
    (cursor.line > 1).then(|| Span {
        start: text::end_of_line(text, cursor.line - 1),
        end: cursor,
    })
}

/// A scroll request repeated `count` times, without repeating the Action —
/// `3<C-e>` is one request for three rows, because the viewport has one writer
/// and three requests would be three frames' worth of movement in one.
const fn scaled(request: ScrollRequest, count: u32) -> ScrollRequest {
    let count = count as i64;
    match request {
        ScrollRequest::Rows { rows } => ScrollRequest::Rows {
            rows: rows.saturating_mul(count),
        },
        ScrollRequest::Pages { pages } => ScrollRequest::Pages {
            pages: pages.saturating_mul(count),
        },
        ScrollRequest::Columns { columns } => ScrollRequest::Columns {
            columns: columns.saturating_mul(count),
        },
        other => other,
    }
}

/// Whether an Action changed the buffer — what makes a command worth repeating.
fn is_edit(action: &Action) -> bool {
    matches!(
        action,
        Action::Buffer(
            BufferAction::Insert { .. }
                | BufferAction::Delete { .. }
                | BufferAction::Replace { .. }
                | BufferAction::Paste { .. }
                | BufferAction::Indent { .. }
                | BufferAction::JoinLines { .. }
                | BufferAction::ToggleComment { .. }
                | BufferAction::SetCase { .. }
                | BufferAction::ApplyEdits { .. }
        )
    )
}

/// A find in the other direction — the whole of what `,` means.
const fn reversed(motion: Motion) -> Motion {
    match motion {
        Motion::FindCharForward => Motion::FindCharBackward,
        Motion::FindCharBackward => Motion::FindCharForward,
        Motion::TillCharForward => Motion::TillCharBackward,
        Motion::TillCharBackward => Motion::TillCharForward,
        other => other,
    }
}
