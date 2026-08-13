//! The shipped keymap, driven by the shipped input machine.
//!
//! Every other test of `runtime/keymaps.scm` asks it a question directly:
//! *what does `gs` resolve to?* That proves a row decodes, and it is exactly
//! the proof that let four `S3` surfaces ship complete and unreachable — a test
//! that hand-builds its subject passes whether or not anything composes it.
//!
//! So this one presses keys. `phosphor_core::input::Machine` is the machine the
//! binary runs, `runtime/keymaps.scm` is the table it ships with, and the only
//! thing here that the binary does not also have is the buffer under the
//! cursor. What comes out is the [`Action`] stream — which is the same stream
//! `main.rs` applies, so a binding that is wrong here is wrong when you type it.
//!
//! **What this cannot prove.** That the host applies the Actions. `main.rs`
//! owns that half and is tested where it lives; this file's claim is that the
//! keys reach the vocabulary, which is the half that was missing.
//!
//! Owned by `spine`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_core::action::{Action, BufferAction, MotionAction, RegionAction, ViewAction};
use phosphor_core::input::Machine;
use phosphor_core::input::key::{Key, parse_seq};
use phosphor_core::input::table::{Keymap, Resolution, Scope};
use phosphor_core::input::text::Text;
use phosphor_core::request::{CaseChange, FoldState, Motion, Position, Seek, Sequence, TextObject};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::keymap::{self, Ex};
use phosphor_steel::runtime::Runtime;

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// The shipped editor layer, booted clean.
fn layer() -> Runtime {
    let tree: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime");
    let runtime = Runtime::boot(Some(&tree), Arc::new(Detached) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    runtime
}

/// The live keymap as the machine sees it — the adapter `main.rs` also has.
struct LiveKeymap<'a> {
    runtime: &'a mut Runtime,
}

impl Keymap for LiveKeymap<'_> {
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution {
        keymap::resolve(self.runtime, scope, keys)
    }
}

/// One line of text with a cursor in it.
///
/// Enough for every motion that is not `H`/`M`/`L`, and the cursor **moves**:
/// a find that emits `SetCursor` and a `;` that repeats it are two keystrokes
/// apart, so a fixture that ignored the first could not test the second.
struct Row {
    text: String,
    cursor: Position,
}

impl Row {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            cursor: Position { line: 1, column: 1 },
        }
    }
}

impl Text for Row {
    fn lines(&self) -> u32 {
        1
    }

    fn line(&self, line: u32) -> Option<String> {
        (line == 1).then(|| self.text.clone())
    }

    fn cursor(&self) -> Position {
        self.cursor
    }
}

/// One editing session: the shipped layer, one machine, one buffer.
///
/// **One machine for the whole test**, because half of what is under test is
/// state the machine keeps between commands — `;` repeats the find `f` made,
/// and a fresh machine per command would have nothing to repeat.
struct Session {
    runtime: Runtime,
    machine: Machine,
    row: Row,
}

impl Session {
    fn on(text: &str) -> Self {
        Self {
            runtime: layer(),
            machine: Machine::new(),
            row: Row::new(text),
        }
    }

    /// Types `spelled` and answers everything it emitted.
    ///
    /// `SetCursor` is applied to the fixture, because the next keystroke is
    /// asked against where the last one left the cursor — which is what makes
    /// `;` a repeat rather than a second first find.
    fn typed(&mut self, spelled: &str) -> Vec<Action> {
        let mut emitted = Vec::new();
        for key in keys(spelled) {
            let step = {
                let mut live = LiveKeymap {
                    runtime: &mut self.runtime,
                };
                self.machine.feed(key, &mut live, &self.row)
            };
            for action in &step {
                if let Action::Motion(MotionAction::SetCursor { position, .. }) = action {
                    self.row.cursor = *position;
                }
            }
            emitted.extend(step);
        }
        emitted
    }

    /// Puts the cursor somewhere, for a motion that has to run backwards.
    fn at(&mut self, column: u32) {
        self.row.cursor = Position { line: 1, column };
    }
}

fn keys(spelled: &str) -> Vec<Key> {
    parse_seq(spelled).expect("a spelling these tests wrote")
}

/// Where the cursor was put, if it was.
fn moved_to(emitted: &[Action]) -> Option<Position> {
    emitted.iter().find_map(|action| match action {
        Action::Motion(MotionAction::SetCursor { position, .. }) => Some(*position),
        _ => None,
    })
}

/// The motion a `move-cursor` names, if one was emitted.
fn moved_by(emitted: &[Action]) -> Option<Motion> {
    emitted.iter().find_map(|action| match action {
        Action::Motion(MotionAction::MoveCursor { motion, .. }) => Some(*motion),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// The motions this pass added
// ---------------------------------------------------------------------------

/// `f`, `;` and `,` — the character is the *next keystroke*, not the binding.
#[test]
fn a_find_takes_the_next_key_as_a_literal() {
    //                            1234567890123456789
    let mut session = Session::on("banana bread basket");

    let emitted = session.typed("fb");
    assert_eq!(
        moved_to(&emitted),
        Some(Position { line: 1, column: 8 }),
        "`f` searches from after the cursor and lands on the character"
    );
    // The `b` of `bread` is where the cursor now is, so `;` is the next one.
    let emitted = session.typed(";");
    assert_eq!(
        moved_to(&emitted),
        Some(Position {
            line: 1,
            column: 14
        })
    );
    let emitted = session.typed(",");
    assert_eq!(
        moved_to(&emitted),
        Some(Position { line: 1, column: 8 }),
        "`,` is the same find the other way"
    );
    // `t` stops one short of what `f` lands on.
    session.at(1);
    let emitted = session.typed("tb");
    assert_eq!(moved_to(&emitted), Some(Position { line: 1, column: 7 }));
    // And `F` runs the other way.
    session.at(14);
    let emitted = session.typed("Fb");
    assert_eq!(moved_to(&emitted), Some(Position { line: 1, column: 8 }));
}

/// A find is an operator's operand: `dfx` deletes up to and including `x`.
#[test]
fn a_find_is_an_operand() {
    let mut session = Session::on("banana bread basket");
    let emitted = session.typed("dfd");

    let deleted = emitted.iter().find_map(|action| match action {
        Action::Buffer(BufferAction::Delete { span }) => Some(*span),
        _ => None,
    });
    let span = deleted.expect("`dfd` deletes");
    assert_eq!(span.start, Position { line: 1, column: 1 });
    assert_eq!(
        span.end,
        Position {
            line: 1,
            column: 13
        },
        "up to and including the `d` of `bread`, half-open"
    );
}

/// `W`, `B`, `E` — the blank-separated words.
#[test]
fn the_blank_separated_words_are_bound() {
    let mut session = Session::on("one two.three four");
    for (spelled, motion) in [
        ("W", Motion::BigWordForward),
        ("B", Motion::BigWordBackward),
        ("E", Motion::BigWordEnd),
    ] {
        let emitted = session.typed(spelled);
        assert_eq!(moved_by(&emitted), Some(motion), "{spelled}");
    }
    // …and the small ones still mean what they meant.
    let emitted = session.typed("w");
    assert_eq!(moved_by(&emitted), Some(Motion::WordForward));
}

/// `r{char}` — one keystroke of data, and `count` of them replaced.
#[test]
fn r_replaces_the_character_under_the_cursor() {
    let mut session = Session::on("banana");
    let emitted = session.typed("3rz");

    let replaced = emitted.iter().find_map(|action| match action {
        Action::Buffer(BufferAction::Replace { span, text }) => Some((*span, text.clone())),
        _ => None,
    });
    let (span, text) = replaced.expect("`3rz` replaces");
    assert_eq!(text, "zzz");
    assert_eq!(span.start, Position { line: 1, column: 1 });
    assert_eq!(span.end, Position { line: 1, column: 4 });
}

/// `~`, `gu` and `gU` — one capability, three words, and `gu` takes an operand.
#[test]
fn the_case_keys_compose_the_way_an_operator_does() {
    let mut session = Session::on("banana bread");

    let cased = |emitted: &[Action]| {
        emitted.iter().find_map(|action| match action {
            Action::Buffer(BufferAction::SetCase { case, .. }) => Some(*case),
            _ => None,
        })
    };

    assert_eq!(
        cased(&session.typed("~")),
        Some(CaseChange::Toggle),
        "`~` is the case operator fused with `l`"
    );
    assert_eq!(cased(&session.typed("guw")), Some(CaseChange::Lower));
    assert_eq!(
        cased(&session.typed("gUiw")),
        Some(CaseChange::Upper),
        "`gU` takes a text object the way `d` does"
    );
}

// ---------------------------------------------------------------------------
// Teej's `gs` ruling
// ---------------------------------------------------------------------------

/// `s` stays vim's substitute; mark-seen is `gs`, and it takes an object.
///
/// The ruling of 2026-08-12, as the two keystrokes it is about. `6d` draws the
/// mark-seen operator as `s`; `CP-3` asks that vim habits carry, so the mockup
/// is the thing that changed and this is the assertion that says which way.
#[test]
fn mark_seen_is_gs_and_s_is_still_substitute() {
    let mut session = Session::on("banana bread");

    // `s` — delete the character, then insert. Vim's substitute, unmoved.
    let emitted = session.typed("s");
    assert!(
        emitted
            .iter()
            .any(|action| matches!(action, Action::Buffer(BufferAction::Delete { .. }))),
        "`s` substitutes: {emitted:?}"
    );
    assert!(
        !emitted
            .iter()
            .any(|action| matches!(action, Action::Region(RegionAction::MarkSeen { .. }))),
        "`s` does not mark anything seen: {emitted:?}"
    );

    // …which left the machine in insert mode, because that is what substitute
    // does. Leave it the way a person would.
    session.typed("<esc>");

    // `gsib` — mark inner block seen. The block object has nothing to resolve
    // against until `T049`, so what this proves is the *sentence*: `gs` waits
    // for an operand, `i` names an inner object, and `b` names the block.
    let emitted = session.typed("gsib");
    let asked = emitted.iter().find_map(|action| match action {
        Action::Motion(MotionAction::SelectObject { object, inner, .. }) => Some((*object, *inner)),
        _ => None,
    });
    assert_eq!(
        asked,
        Some((TextObject::Block, true)),
        "`gsib` asks for the inner review block: {emitted:?}"
    );
    // T028: it no-ops rather than erroring — nothing was edited.
    assert!(
        !emitted
            .iter()
            .any(|action| matches!(action, Action::Buffer(_))),
        "an agent noun that resolves to nothing edits nothing: {emitted:?}"
    );
}

// ---------------------------------------------------------------------------
// R19 — the fold keys, and R13's sequence keys
// ---------------------------------------------------------------------------

/// `za`, `zM` and `zR` reach the three fold capabilities.
///
/// Before this pass no `z` binding existed at all, so `za` ran vim's plain `a`
/// and entered insert mode with the `z` swallowed.
#[test]
fn the_fold_keys_reach_the_view_capabilities() {
    let mut session = Session::on("banana bread");

    let emitted = session.typed("za");
    assert!(
        emitted.iter().any(|action| matches!(
            action,
            Action::View(ViewAction::SetFold {
                state: FoldState::Toggle,
                ..
            })
        )),
        "`za` toggles the fold at the cursor: {emitted:?}"
    );
    assert!(
        !emitted
            .iter()
            .any(|action| matches!(action, Action::Input(_))),
        "`za` is not `a`: it enters no mode: {emitted:?}"
    );

    let emitted = session.typed("zM");
    assert!(
        emitted
            .iter()
            .any(|action| matches!(action, Action::View(ViewAction::FoldAll { level: 0 }))),
        "`zM` is vim's foldlevel 0 — everything closed, not everything but the \
         outermost: {emitted:?}"
    );
    let emitted = session.typed("zR");
    assert!(
        emitted
            .iter()
            .any(|action| matches!(action, Action::View(ViewAction::UnfoldAll {}))),
        "{emitted:?}"
    );
}

/// `]u` / `[u` — `6d`'s *"next / previous unseen"*.
#[test]
fn the_unseen_sequence_keys_are_bound() {
    let mut session = Session::on("banana bread");

    for (spelled, wanted) in [("]u", Seek::Next), ("[u", Seek::Prev)] {
        let emitted = session.typed(spelled);
        let walked = emitted.iter().find_map(|action| match action {
            Action::Motion(MotionAction::GotoSequence { sequence, seek, .. }) => {
                Some((*sequence, *seek))
            }
            _ => None,
        });
        assert_eq!(
            walked,
            Some((Sequence::UnseenRegion, wanted)),
            "{spelled}: {emitted:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// R11 — the ex range grammar
// ---------------------------------------------------------------------------

/// `:'<,'>c` is a command with a range, not a command called `'<,'>c`.
///
/// `T028`'s *done when* is *"the grammar accepts them and they no-op cleanly
/// rather than erroring"*, and this line was the one of its four forms that
/// errored: the lookup was handed the whole head, found nothing, and the host
/// answered *no such command*.
#[test]
fn a_visual_range_is_read_as_a_range() {
    let mut runtime = layer();

    let over_selection = keymap::ex(&mut runtime, "'<,'>c looks wrong to me");
    assert_ne!(
        over_selection,
        Ex::Unknown,
        "`:'<,'>c` names `:c` over a range"
    );

    let Ex::Run(actions) = over_selection else {
        panic!("a range command answers Actions the host applies");
    };
    // One call, and it is anchored to the selection the range named.
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert!(
        matches!(
            &actions[0],
            Action::Thread(phosphor_core::action::ThreadAction::StartThread {
                anchor: phosphor_core::request::Target::Selection {},
                body,
            }) if body == "looks wrong to me"
        ),
        "{actions:?}"
    );
}

/// A line range selects itself first, which is how it reaches a `Target`.
#[test]
fn a_line_range_selects_before_the_command_acts() {
    let mut runtime = layer();

    let Ex::Run(actions) = keymap::ex(&mut runtime, "12,20c mind the gap") else {
        panic!("`:12,20c` runs");
    };
    assert_eq!(actions.len(), 2, "{actions:?}");
    // Half-open and linewise: lines 12 through 20 end at 21:1.
    assert!(
        matches!(
            &actions[0],
            Action::Motion(MotionAction::SelectRange { span, .. })
                if span.start == Position { line: 12, column: 1 }
                    && span.end == Position { line: 21, column: 1 }
        ),
        "{actions:?}"
    );
    assert!(
        matches!(
            &actions[1],
            Action::Thread(phosphor_core::action::ThreadAction::StartThread {
                anchor: phosphor_core::request::Target::Selection {},
                ..
            })
        ),
        "{actions:?}"
    );

    // One address is a one-line range.
    let Ex::Run(actions) = keymap::ex(&mut runtime, "12c here") else {
        panic!("`:12c` runs");
    };
    assert!(
        matches!(
            &actions[0],
            Action::Motion(MotionAction::SelectRange { span, .. })
                if span.start == Position { line: 12, column: 1 }
                    && span.end == Position { line: 13, column: 1 }
        ),
        "{actions:?}"
    );
}

/// No range is the cursor, and a command that ignores ranges still works.
#[test]
fn a_range_changes_nothing_for_a_command_that_does_not_read_one() {
    let mut runtime = layer();

    let Ex::Run(actions) = keymap::ex(&mut runtime, "c no range here") else {
        panic!("`:c` runs without a range");
    };
    assert!(
        matches!(
            &actions[0],
            Action::Thread(phosphor_core::action::ThreadAction::StartThread {
                anchor: phosphor_core::request::Target::Cursor {},
                ..
            })
        ),
        "{actions:?}"
    );

    // `:'<,'>w` is `:write` — the range is read off and the command that has no
    // use for one is unaffected.
    assert_eq!(
        keymap::ex(&mut runtime, "'<,'>w"),
        keymap::ex(&mut runtime, "w")
    );
    // And a range that names nothing is not a range: `:12,` is still a lookup,
    // and still answers for itself.
    assert_eq!(keymap::ex(&mut runtime, "12,"), Ex::Unknown);
}
