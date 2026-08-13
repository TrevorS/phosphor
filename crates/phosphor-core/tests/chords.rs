//! `T027`'s acceptance: *"`ctrl+shift+<key>` is distinguishable from
//! `ctrl+<key>` on the primary terminal"* — and what happens on the terminal
//! that cannot tell them apart.
//!
//! # Why the fallback gets more tests than the chord
//!
//! The kitty half is verified on hardware and nowhere else: the browser-based
//! terminal VHS drives does not implement the protocol
//! ([`TASKS.md`](../../../docs/TASKS.md)'s fixture table), so no tape can see a
//! modifier chord. The *legacy* half is the one that ships broken, because
//! everyone building this has the good terminal and nothing about a chord that
//! silently never fires shows up in a review. So both paths are driven here,
//! headlessly, through the real machine and a real keymap.
//!
//! What is deliberately **not** here: `crossterm::KeyEvent`. The conversion is
//! the app layer's one function (`main.rs`'s `decode`), and the shapes it can
//! produce are asserted where the rule lives, in `input::key`'s own tests —
//! this file is about what the *grammar* does with the key that comes out.
//!
//! Owned by `spine`.

use phosphor_core::action::{Action, MotionAction};
use phosphor_core::input::key::{Code, Key, Mods, Protocol, parse_seq};
use phosphor_core::input::table::{Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::input::{Machine, key};
use phosphor_core::request::{Motion, Position};

/// One line, a cursor at its start, and nothing else — a chord is resolved
/// before any of the buffer is read.
#[derive(Debug)]
struct Line;

impl Text for Line {
    fn lines(&self) -> u32 {
        1
    }

    fn line(&self, line: u32) -> Option<String> {
        (line == 1).then(|| "the buffer a chord never touches".to_owned())
    }

    fn cursor(&self) -> Position {
        Position { line: 1, column: 1 }
    }

    fn viewport(&self) -> Viewport {
        Viewport { top: 1, height: 1 }
    }
}

/// The two chords, bound to motions that cannot be confused with each other.
fn table() -> Table {
    let mut table = Table::new();
    table.bind(Scope::Normal, "<C-k>", Role::Motion(Motion::LineUp));
    table.bind(
        Scope::Normal,
        "<C-S-k>",
        Role::Motion(Motion::ParagraphBackward),
    );
    table
}

/// The one keystroke a terminal sends for ctrl+k — and, under the legacy
/// encoding, for ctrl+shift+k as well.
fn ctrl_k() -> Key {
    Key::new(Code::Char('k'), Mods::CTRL)
}

/// ctrl+shift+k as a kitty terminal that reports alternate keys sends it:
/// the shifted character, with the shift bit already spent (crossterm 0.29
/// `event/sys/unix/parse.rs:594-606`).
fn kitty_ctrl_shift_k() -> Key {
    Key::new(Code::Char('K'), Mods::CTRL)
}

/// The same chord from a kitty terminal that reports the modifier instead.
fn csi_u_ctrl_shift_k() -> Key {
    Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT))
}

fn motion_of(stream: &[Action]) -> Option<Motion> {
    stream.iter().find_map(|action| match action {
        Action::Motion(MotionAction::MoveCursor { motion, .. }) => Some(*motion),
        _ => None,
    })
}

fn names(stream: &[Action]) -> Vec<&'static str> {
    stream.iter().map(Action::name).collect()
}

#[test]
fn on_the_primary_terminal_the_two_chords_are_two_bindings() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.protocol(),
        Protocol::Kitty,
        "the default is the undegraded path"
    );
    let mut keymap = table();

    assert_eq!(
        motion_of(&machine.feed(ctrl_k(), &mut keymap, &Line)),
        Some(Motion::LineUp),
        "ctrl+k is its own binding"
    );
    assert_eq!(
        motion_of(&machine.feed(kitty_ctrl_shift_k(), &mut keymap, &Line)),
        Some(Motion::ParagraphBackward),
        "ctrl+shift+k reaches the binding written <C-S-k>"
    );
    // The acceptance criterion itself, stated as the difference it makes.
    assert_ne!(
        motion_of(&machine.feed(ctrl_k(), &mut keymap, &Line)),
        motion_of(&machine.feed(kitty_ctrl_shift_k(), &mut keymap, &Line))
    );
}

#[test]
fn both_kitty_encodings_of_the_chord_reach_the_same_binding() {
    // A terminal with REPORT_ALTERNATE_KEYS sends the shifted character; one
    // without sends the modifier. A keymap is written once.
    let mut machine = Machine::new();
    let mut keymap = table();

    for pressed in [kitty_ctrl_shift_k(), csi_u_ctrl_shift_k()] {
        assert_eq!(
            motion_of(&machine.feed(pressed, &mut keymap, &Line)),
            Some(Motion::ParagraphBackward),
            "{} did not reach <C-S-k>",
            pressed.notation()
        );
    }
}

#[test]
fn under_kitty_a_chord_binding_is_never_reached_by_the_unshifted_key() {
    // The fallback below must not fire here: the terminal told the truth, so
    // ctrl+k on a keymap that only binds <C-S-k> is an unbound key.
    let mut machine = Machine::new();
    let mut keymap = Table::new();
    keymap.bind(
        Scope::Normal,
        "<C-S-k>",
        Role::Motion(Motion::ParagraphBackward),
    );

    let stream = machine.feed(ctrl_k(), &mut keymap, &Line);
    assert_eq!(
        names(&stream),
        ["show-unknown-key-hint", "cancel-pending"],
        "{stream:#?}"
    );
}

#[test]
fn on_a_legacy_terminal_the_chord_binding_is_reachable_at_all() {
    // The degradation. The terminal sends one byte for both, so the machine
    // asks the second question when the first comes back unbound.
    let mut machine = Machine::new();
    machine.set_protocol(Protocol::Legacy);
    let mut keymap = Table::new();
    keymap.bind(
        Scope::Normal,
        "<C-S-k>",
        Role::Motion(Motion::ParagraphBackward),
    );

    assert_eq!(
        motion_of(&machine.feed(ctrl_k(), &mut keymap, &Line)),
        Some(Motion::ParagraphBackward),
        "a documented chord that can never fire is the failure this prevents"
    );
}

#[test]
fn the_key_that_was_actually_pressed_still_wins() {
    // The fallback only reaches what would otherwise be unreachable. With both
    // bound, ctrl+k is ctrl+k — on either terminal.
    let mut machine = Machine::new();
    machine.set_protocol(Protocol::Legacy);
    let mut keymap = table();

    assert_eq!(
        motion_of(&machine.feed(ctrl_k(), &mut keymap, &Line)),
        Some(Motion::LineUp),
        "a <C-S-k> binding must not shadow <C-k>"
    );
}

#[test]
fn the_fallback_carries_a_sequence_that_is_not_finished() {
    // `<C-S-k>w` is bound and `<C-S-k>` alone is not, so the first key answers
    // Pending *in the shifted spelling* — and the machine has to remember it
    // that way or the second key looks for `<C-k>w`, which nothing binds.
    let mut machine = Machine::new();
    machine.set_protocol(Protocol::Legacy);
    let mut keymap = Table::new();
    keymap.bind(
        Scope::Normal,
        "<C-S-k>w",
        Role::Motion(Motion::ParagraphForward),
    );

    let first = machine.feed(ctrl_k(), &mut keymap, &Line);
    assert!(first.is_empty(), "a prefix emits nothing: {first:#?}");
    assert_eq!(
        machine.pending().keys,
        parse_seq("<C-S-k>").expect("a spelling this test wrote"),
        "the sequence continues in the spelling that answered"
    );

    assert_eq!(
        motion_of(&machine.feed(Key::char('w'), &mut keymap, &Line)),
        Some(Motion::ParagraphForward)
    );
}

#[test]
fn only_ctrl_loses_its_shift_and_only_that_is_retried() {
    // Alt sends the character itself, so `<A-K>` arrives as a capital under
    // every protocol and needs no fallback. Retrying it would fire an
    // `<A-S-k>` binding on a plain `<A-k>` for nothing.
    let mut machine = Machine::new();
    machine.set_protocol(Protocol::Legacy);
    let mut keymap = Table::new();
    keymap.bind(
        Scope::Normal,
        "<A-S-k>",
        Role::Motion(Motion::ParagraphBackward),
    );

    let stream = machine.feed(Key::new(Code::Char('k'), Mods::ALT), &mut keymap, &Line);
    assert_eq!(
        names(&stream),
        ["show-unknown-key-hint", "cancel-pending"],
        "{stream:#?}"
    );
    // And the shifted alt key still reaches it, from either spelling.
    assert_eq!(
        motion_of(&machine.feed(Key::new(Code::Char('K'), Mods::ALT), &mut keymap, &Line)),
        Some(Motion::ParagraphBackward)
    );
}

#[test]
fn a_plain_key_is_never_retried_as_a_chord() {
    // Shift on a letter is the capital, and `x` must not reach a binding on
    // `X` no matter what the terminal cannot say.
    let mut machine = Machine::new();
    machine.set_protocol(Protocol::Legacy);
    let mut keymap = Table::new();
    keymap.bind(Scope::Normal, "X", Role::Motion(Motion::ParagraphBackward));

    let stream = machine.feed(Key::char('x'), &mut keymap, &Line);
    assert_eq!(
        names(&stream),
        ["show-unknown-key-hint", "cancel-pending"],
        "{stream:#?}"
    );
    let hint = stream
        .iter()
        .find_map(|action| match action {
            Action::App(phosphor_core::action::AppAction::ShowUnknownKeyHint { key }) => {
                Some(key.0.clone())
            }
            _ => None,
        })
        .expect("the hint names the key");
    assert_eq!(hint, "x", "the hint reports what was pressed");
}

#[test]
fn the_notation_a_hint_reports_is_the_notation_a_keymap_is_written_in() {
    // `T033`'s table is text, and the machine's spelling of a chord has to be
    // the same text or a rebind lands on a key nobody can press.
    assert_eq!(kitty_ctrl_shift_k().notation(), "<C-S-k>");
    assert_eq!(csi_u_ctrl_shift_k().notation(), "<C-S-k>");
    assert_eq!(ctrl_k().notation(), "<C-k>");
    assert_eq!(
        key::notation_of(&[kitty_ctrl_shift_k(), Key::char('w')]).0,
        "<C-S-k>w"
    );
}
