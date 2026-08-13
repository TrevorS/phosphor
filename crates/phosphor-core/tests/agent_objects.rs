//! `T028`'s acceptance: *"the grammar accepts them and they no-op cleanly
//! rather than erroring."*
//!
//! The four nouns of screen `6d` — `u` unseen region, `h` hunk, `t` thread,
//! `b` review block — and the three sentences the design writes with them:
//!
//! ```text
//!   viu        select inner unseen region
//!   gsib       mark inner block seen — gs composes like an operator
//!   dih        delete inner hunk — revert claude's edit, plain vim delete
//! ```
//!
//! They **parse here and resolve at `S5`** ([Q8](../../../docs/IMPLEMENTATION-PLAN.md)):
//! `text::object_span` answers [`None`] for all four because a region is a
//! store query and there is no store until `T041`. So every test below is
//! about the shape of *nothing happening* — that the keystrokes are accepted,
//! that the machine comes back to a clean state, and that not one of them
//! reaches the buffer.
//!
//! # The fourth form
//!
//! `:'<,'>c` is the same task's fourth sentence and it is not here: the ex line
//! is `T033`'s, it lives in `runtime/keymaps.scm`, and a range grammar is a row
//! in that table rather than anything the input machine can hold. See this
//! window's report.
//!
//! Owned by `spine`.

use phosphor_core::action::{Action, MotionAction, RegionAction};
use phosphor_core::input::Machine;
use phosphor_core::input::key::parse_seq;
use phosphor_core::input::table::{Keymap, Operator, Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::request::{EditMode, Position, SelectionKind, Target, TextObject};

mod support;

/// A buffer that is only ever read: nothing below is allowed to edit it, and
/// the assertion that nothing did is that this type has no mutation at all.
#[derive(Debug)]
struct Source(Vec<String>);

impl Source {
    fn new() -> Self {
        Self(
            ["fn main() {", "    claude_wrote_this();", "}"]
                .iter()
                .map(|line| (*line).to_owned())
                .collect(),
        )
    }
}

impl Text for Source {
    fn lines(&self) -> u32 {
        u32::try_from(self.0.len()).unwrap_or(1).max(1)
    }

    fn line(&self, line: u32) -> Option<String> {
        self.0.get((line as usize).checked_sub(1)?).cloned()
    }

    fn cursor(&self) -> Position {
        Position { line: 2, column: 5 }
    }

    fn viewport(&self) -> Viewport {
        Viewport { top: 1, height: 3 }
    }
}

/// The keymap, with mark-seen where **Teej ruled it goes: `gs`**.
///
/// `6d` draws the operator as `s`, and `s` is vim's substitute — a habit that
/// carries, which `CP-3` is explicitly about. So the mockup is the thing that
/// changed (2026-08-12): mark-seen is `gs`, it still takes an object, and
/// `gsib` is the sentence. `g` bound only `gg` and `gc` before this, so
/// nothing moved out of the way for it.
///
/// The rows live in [`support::table`] with the rest of the keymap; what is
/// *not* yet true is the shipped half — `runtime/keymaps.scm` has no `gs` row
/// and `phosphor-steel/src/keymap.rs`'s decoder has no name for `mark-seen`
/// (read this session, `keymap.rs:356-364`). Both are named in `R1`'s report.
fn table() -> Table {
    support::table()
}

/// Types a sequence and answers the whole Action stream. Nothing is applied:
/// no keystroke here may depend on what the one before it did to the buffer,
/// and if one ever does, that is the finding.
fn drive(machine: &mut Machine, keymap: &mut dyn Keymap, keys: &str) -> Vec<Action> {
    let text = Source::new();
    parse_seq(keys)
        .expect("a spelling this test wrote")
        .into_iter()
        .flat_map(|key| machine.feed(key, keymap, &text))
        .collect()
}

fn names(stream: &[Action]) -> Vec<&'static str> {
    stream.iter().map(Action::name).collect()
}

/// Whether anything in the stream would change the buffer's text.
fn edits(stream: &[Action]) -> Vec<&'static str> {
    stream
        .iter()
        .filter(|action| matches!(action, Action::Buffer(_)))
        .map(|action| action.name())
        .collect()
}

fn objects(stream: &[Action]) -> Vec<TextObject> {
    stream
        .iter()
        .filter_map(|action| match action {
            Action::Motion(MotionAction::SelectObject { object, .. }) => Some(*object),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The three sentences
// ---------------------------------------------------------------------------

#[test]
fn viu_selects_an_inner_unseen_region_and_selects_nothing_yet() {
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "viu");

    assert_eq!(
        names(&stream),
        ["set-mode", "select-range", "select-object"],
        "{stream:#?}"
    );
    // `v` enters visual and selects the character under the cursor; `iu` asks
    // for the region and gets no span, so the selection is left where it was
    // rather than being cleared out from under the user.
    assert_eq!(objects(&stream), [TextObject::UnseenRegion]);
    assert!(edits(&stream).is_empty(), "{stream:#?}");
    assert_eq!(machine.mode(), EditMode::VisualChar);
    assert!(machine.pending().is_clear(), "nothing is left half-typed");
}

#[test]
fn gsib_marks_an_inner_block_seen_and_marks_nothing_yet() {
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "gsib");

    assert_eq!(
        names(&stream),
        ["set-mode", "select-object", "cancel-pending", "set-mode"],
        "{stream:#?}"
    );
    assert_eq!(objects(&stream), [TextObject::Block]);
    // The operand did not resolve, so the operator is dropped rather than
    // applied to something else — and nothing was marked.
    assert!(
        !stream
            .iter()
            .any(|action| matches!(action, Action::Region(_))),
        "{stream:#?}"
    );
    assert!(edits(&stream).is_empty(), "{stream:#?}");
    assert_eq!(machine.mode(), EditMode::Normal);
    assert!(machine.pending().is_clear());
}

#[test]
fn dih_deletes_an_inner_hunk_and_deletes_nothing_yet() {
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "dih");

    assert_eq!(
        names(&stream),
        ["set-mode", "select-object", "cancel-pending", "set-mode"],
        "{stream:#?}"
    );
    assert_eq!(objects(&stream), [TextObject::Hunk]);
    assert!(
        edits(&stream).is_empty(),
        "`dih` with no store must not delete something else: {stream:#?}"
    );
    assert_eq!(machine.mode(), EditMode::Normal);
    assert!(machine.pending().is_clear());
}

#[test]
fn a_yank_into_a_named_register_over_an_agent_noun_is_the_same_no_op() {
    // `6d`'s fourth row: `"ay ib`. The register is named, the object does not
    // resolve, and the yank never happens — the register is not overwritten
    // with something else, which is the failure that would matter.
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "\"ayib");

    assert_eq!(
        names(&stream),
        [
            "select-register",
            "set-mode",
            "select-object",
            "cancel-pending",
            "set-mode"
        ],
        "{stream:#?}"
    );
    assert!(edits(&stream).is_empty(), "{stream:#?}");
    assert!(machine.pending().is_clear(), "the register is spent");
}

#[test]
fn every_agent_noun_is_bound_in_both_moods() {
    // `i` and `a` over all four, in the operator and the visual paths, so a
    // noun that was bound in one scope and forgotten in the other is loud.
    let mut machine = Machine::new();
    let mut keymap = table();

    for (keys, object) in [
        ("diu", TextObject::UnseenRegion),
        ("dau", TextObject::UnseenRegion),
        ("dih", TextObject::Hunk),
        ("dah", TextObject::Hunk),
        ("dit", TextObject::Thread),
        ("dat", TextObject::Thread),
        ("dib", TextObject::Block),
        ("dab", TextObject::Block),
        ("viu", TextObject::UnseenRegion),
        ("vih", TextObject::Hunk),
        ("vit", TextObject::Thread),
        ("vib", TextObject::Block),
        ("gsib", TextObject::Block),
        ("yiu", TextObject::UnseenRegion),
    ] {
        let stream = drive(&mut machine, &mut keymap, keys);
        assert_eq!(objects(&stream), [object], ":{keys} — {stream:#?}");
        assert!(edits(&stream).is_empty(), ":{keys} — {stream:#?}");
        // Back to a state the next sentence can start from, whatever mood the
        // last one left off in.
        drive(&mut machine, &mut keymap, "<esc>");
        assert_eq!(machine.mode(), EditMode::Normal, ":{keys}");
        assert!(machine.pending().is_clear(), ":{keys}");
    }
}

// ---------------------------------------------------------------------------
// `gs` as an operator — the half of `6d` that is grammar rather than store
// ---------------------------------------------------------------------------

#[test]
fn gs_composes_like_an_operator_over_an_object_that_does_resolve() {
    // The composition itself, proven against a noun that exists today: `gsiw`
    // selects the word and marks it. `T041` builds `mark-seen`, so what a door
    // answers today is "`T041` builds this" — a refusal, which the vocabulary
    // is explicit is a normal state and not an error.
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "gsiw");

    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "select-object",
            "select-range",
            "mark-seen",
            "clear-selection",
            "set-mode"
        ],
        "{stream:#?}"
    );
    assert!(
        matches!(
            stream
                .iter()
                .find(|action| matches!(action, Action::Region(_))),
            Some(Action::Region(RegionAction::MarkSeen {
                target: Target::Selection {}
            }))
        ),
        "{stream:#?}"
    );
    assert!(
        edits(&stream).is_empty(),
        "seen-state is not text: {stream:#?}"
    );
}

#[test]
fn marking_seen_opens_no_undo_group_and_is_not_a_change_to_repeat() {
    // Two consequences of "it is not an edit", both of which would be felt
    // immediately at `CP-3`: `u` after `gsib` must undo the edit *before* it,
    // and `.` must repeat that edit rather than a mark.
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "gsiw");
    assert!(
        !stream.iter().any(|action| matches!(
            action,
            Action::History(phosphor_core::action::HistoryAction::CommitUndoGroup {})
        )),
        "an empty undo step: {stream:#?}"
    );
    assert_eq!(
        machine.last_change(),
        None,
        "marking seen is not the last change `.` repeats"
    );
}

#[test]
fn the_doubled_operator_marks_the_line_seen() {
    // `dd`, `yy`, `cc` — and `gsgs`, because the doubling rule is a lookup in the
    // machine rather than a special case per operator.
    let mut machine = Machine::new();
    let mut keymap = table();

    let stream = drive(&mut machine, &mut keymap, "gsgs");

    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "select-range",
            "mark-seen",
            "clear-selection",
            "set-mode"
        ],
        "{stream:#?}"
    );
    let selected = stream
        .iter()
        .find_map(|action| match action {
            Action::Motion(MotionAction::SelectRange { span, kind }) => Some((*span, *kind)),
            _ => None,
        })
        .expect("the operand is selected first");
    assert_eq!(selected.1, SelectionKind::Line);
    assert_eq!(selected.0.start, Position { line: 2, column: 1 });
}

#[test]
fn the_transcribed_table_still_spells_s_as_substitute() {
    // **The contract this file cannot close.** `6d` says `s` composes like an
    // operator. The transcribed seed binds it to vim's substitute — and so
    // does the shipped keymap it was transcribed into
    // (`runtime/keymaps.scm:365`, read in this session), whose decoder has no
    // name for `mark-seen` either (`phosphor-steel/src/keymap.rs:356-364`).
    // Neither file is this window's to edit, so `sib` in the *running* editor
    // substitutes a character and then types `ib`.
    //
    // Asserted rather than written down, so the day the seed's row changes
    // this test fails and asks whether the other two changed with it.
    let mut seed = support::table();
    assert_eq!(
        seed.resolve(
            Scope::Normal,
            &parse_seq("s").expect("a spelling this test wrote")
        ),
        phosphor_core::input::table::Resolution::Role(Role::Fused {
            operator: Operator::Change,
            motion: phosphor_core::request::Motion::CharRight,
        }),
        "the transcribed table and the shipped one agree, and neither is 6d yet"
    );
}

#[test]
fn the_grammar_asks_the_store_for_nothing() {
    // The seam `T049` takes. A noun reaches the store through
    // `text::object_span`'s signature — a query the *machine* never makes — so
    // an unresolved noun emits the record of the ask and nothing else. No
    // region id crosses this boundary in either direction, which is what lets
    // `T049` land without re-shaping a single Action.
    let mut machine = Machine::new();
    let mut keymap = table();

    for keys in ["viu", "dih", "gsib", "dit", "dau"] {
        for action in drive(&mut machine, &mut keymap, keys) {
            assert!(
                matches!(action, Action::Motion(_) | Action::Input(_)),
                ":{keys} emitted {action:?} — an unresolved noun is a record, \
                 not a mutation"
            );
        }
        drive(&mut machine, &mut keymap, "<esc>");
    }
}
