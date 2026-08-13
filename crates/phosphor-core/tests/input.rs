//! `T026`'s acceptance: *"a scripted keystroke sequence produces the expected
//! Action stream, including counts and named registers."*
//!
//! The two the dropped crate could not express get the most attention here,
//! because they are also the two `CP-3` says to *"test hardest"*: `3dd` and
//! `"ayy` ([Q3](../../../docs/IMPLEMENTATION-PLAN.md)).
//!
//! # Why this test applies the Actions
//!
//! A keystroke sequence is not a pure function of the keys: `dw` reads the
//! cursor, and the cursor is where the last Action left it. So the driver below
//! is a **miniature editor** — it applies each key's Actions before feeding the
//! next, exactly as the loop in `crates/phosphor/src/main.rs` does. That makes
//! the assertions about the *stream* honest, and it makes the buffer's final
//! text an assertion too: a stream that reads correctly but edits the wrong
//! span fails here rather than at `CP-3`.
//!
//! Owned by `spine`.

use std::collections::BTreeMap;

use phosphor_core::action::{Action, BufferAction, InputAction, MotionAction};
use phosphor_core::input::Machine;
use phosphor_core::input::key::parse_seq;
use phosphor_core::input::table::{Keymap, Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::request::{
    CaseChange, EditMode, Motion, Position, RegisterName, SelectionKind, Span, Target,
};

mod support;

// ---------------------------------------------------------------------------
// The miniature editor
// ---------------------------------------------------------------------------

/// A buffer, a cursor, a selection and the registers — everything the Action
/// stream touches, and nothing else.
#[derive(Debug)]
struct Buffer {
    rows: Vec<String>,
    cursor: Position,
    selection: Option<(Span, SelectionKind)>,
    registers: BTreeMap<String, String>,
    mode: EditMode,
    quit: bool,
}

impl Buffer {
    fn new(content: &str) -> Self {
        Self {
            rows: content.split('\n').map(str::to_owned).collect(),
            cursor: Position { line: 1, column: 1 },
            selection: None,
            registers: BTreeMap::new(),
            mode: EditMode::Normal,
            quit: false,
        }
    }

    fn content(&self) -> String {
        self.rows.join("\n")
    }

    fn at(&mut self, line: u32, column: u32) {
        self.cursor = Position { line, column };
    }

    fn slice(&self, span: Span) -> String {
        let mut out = String::new();
        for line in span.start.line..=span.end.line.min(self.lines()) {
            let row: Vec<char> = self.rows[line as usize - 1].chars().collect();
            let from = if line == span.start.line {
                span.start.column as usize - 1
            } else {
                0
            };
            let to = if line == span.end.line {
                (span.end.column as usize - 1).min(row.len())
            } else {
                row.len()
            };
            out.extend(row[from.min(row.len())..to.max(from.min(row.len()))].iter());
            if line < span.end.line {
                out.push('\n');
            }
        }
        out
    }

    fn remove(&mut self, span: Span) {
        let head: String = self.rows[span.start.line as usize - 1]
            .chars()
            .take(span.start.column as usize - 1)
            .collect();
        let tail: String = self
            .rows
            .get(span.end.line as usize - 1)
            .map(|row| row.chars().skip(span.end.column as usize - 1).collect())
            .unwrap_or_default();
        let last = (span.end.line as usize).min(self.rows.len());
        self.rows.splice(
            span.start.line as usize - 1..last,
            std::iter::once(head + &tail),
        );
        self.cursor = span.start;
    }

    fn insert(&mut self, at: Position, text: &str) {
        let row = &self.rows[at.line as usize - 1];
        let head: String = row.chars().take(at.column as usize - 1).collect();
        let tail: String = row.chars().skip(at.column as usize - 1).collect();
        let mut parts: Vec<String> = format!("{head}{text}{tail}")
            .split('\n')
            .map(str::to_owned)
            .collect();
        let lines = parts.len();
        let last = parts.pop().unwrap_or_default();
        let column = u32::try_from(
            format!("{head}{text}")
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count(),
        )
        .unwrap_or(0)
            + 1;
        parts.push(last);
        self.rows
            .splice(at.line as usize - 1..at.line as usize, parts);
        self.cursor = Position {
            line: at.line + u32::try_from(lines).unwrap_or(1) - 1,
            column,
        };
    }
}

impl Text for Buffer {
    fn lines(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(1).max(1)
    }

    fn line(&self, line: u32) -> Option<String> {
        self.rows.get((line as usize).checked_sub(1)?).cloned()
    }

    fn cursor(&self) -> Position {
        self.cursor
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            top: 1,
            height: self.lines(),
        }
    }
}

/// One Action, applied. The same match `main.rs` has, minus the terminal.
fn apply(buffer: &mut Buffer, action: &Action) {
    match action {
        Action::Buffer(BufferAction::Insert { at, text }) => buffer.insert(*at, text),
        Action::Buffer(BufferAction::Delete { span }) => buffer.remove(*span),
        Action::Buffer(BufferAction::Replace { span, text }) => {
            buffer.remove(*span);
            buffer.insert(span.start, text);
        }
        Action::Buffer(BufferAction::Yank { target, register }) => {
            let span = match target {
                Target::Selection {} => buffer.selection.map(|(span, _)| span),
                _ => None,
            };
            if let Some(span) = span {
                let text = buffer.slice(span);
                let name = register
                    .as_ref()
                    .map_or_else(|| "\"".to_owned(), |name| name.0.clone());
                buffer.registers.insert(name, text);
            }
        }
        Action::Buffer(BufferAction::Paste {
            register, before, ..
        }) => {
            let name = register
                .as_ref()
                .map_or_else(|| "\"".to_owned(), |name| name.0.clone());
            if let Some(text) = buffer.registers.get(&name).cloned() {
                let at = if *before {
                    buffer.cursor
                } else {
                    Position {
                        column: buffer.cursor.column + 1,
                        ..buffer.cursor
                    }
                };
                buffer.insert(at, &text);
            }
        }
        // `gu`, `gU`, `~`. The host does the same thing with the same helper,
        // so "what toggle means" has one definition (`text::cased`).
        Action::Buffer(BufferAction::SetCase { target, case }) => {
            let span = match target {
                Target::Selection {} => buffer.selection.map(|(span, _)| span),
                _ => None,
            };
            if let Some(span) = span {
                let cased = phosphor_core::input::text::cased(&buffer.slice(span), *case);
                buffer.remove(span);
                buffer.insert(span.start, &cased);
            }
        }
        Action::Motion(MotionAction::MoveCursor { motion, count }) => {
            buffer.cursor =
                phosphor_core::input::text::cursor_after(buffer, buffer.cursor, *motion, *count);
        }
        Action::Motion(MotionAction::SetCursor { position, .. }) => buffer.cursor = *position,
        Action::Motion(MotionAction::SelectRange { span, kind }) => {
            buffer.selection = Some((*span, *kind));
        }
        Action::Motion(MotionAction::ExtendSelection { motion, count }) => {
            buffer.cursor =
                phosphor_core::input::text::cursor_after(buffer, buffer.cursor, *motion, *count);
        }
        Action::Motion(MotionAction::ClearSelection {}) => buffer.selection = None,
        // The record of what was asked for. What it *covers* arrives as the
        // `select-range` behind it, when this side can resolve it at all.
        Action::Motion(MotionAction::SelectObject { .. }) => {}
        Action::Input(InputAction::SetMode { mode }) => buffer.mode = *mode,
        Action::App(_) | Action::Input(_) | Action::History(_) | Action::View(_) => {}
        other => panic!("the machine emitted something this driver does not apply: {other:?}"),
    }
}

/// Types a sequence, applying as it goes. Answers the whole Action stream.
fn drive(
    machine: &mut Machine,
    keymap: &mut dyn Keymap,
    buffer: &mut Buffer,
    keys: &str,
) -> Vec<Action> {
    let mut stream = Vec::new();
    for key in parse_seq(keys).expect("a spelling this test wrote") {
        let emitted = machine.feed(key, keymap, buffer);
        for action in &emitted {
            apply(buffer, action);
        }
        stream.extend(emitted);
    }
    stream
}

fn names(stream: &[Action]) -> Vec<&'static str> {
    stream.iter().map(Action::name).collect()
}

// ---------------------------------------------------------------------------
// The two the dropped crate could not express
// ---------------------------------------------------------------------------

#[test]
fn a_count_folds_into_the_operand_and_3dd_is_one_delete() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("one\ntwo\nthree\nfour\nfive");
    buffer.at(2, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "3dd");

    assert_eq!(
        names(&stream),
        [
            "set-count",
            "set-mode",
            "select-range",
            "yank",
            "delete",
            "set-cursor",
            "move-cursor",
            "clear-selection",
            "set-mode",
            "commit-undo-group",
        ],
        "one keystroke sequence, one edit, one undo group ({stream:#?})"
    );
    // The count is in the span, not in a repeat count — decision 1.
    let deleted = stream
        .iter()
        .find_map(|action| match action {
            Action::Buffer(BufferAction::Delete { span }) => Some(*span),
            _ => None,
        })
        .expect("3dd deletes");
    assert_eq!(deleted.start, Position { line: 2, column: 1 });
    assert_eq!(deleted.end, Position { line: 5, column: 1 });
    assert_eq!(buffer.content(), "one\nfive");
    assert_eq!(
        buffer.cursor(),
        Position { line: 2, column: 1 },
        "'startofline': a linewise delete lands on the first non-blank of the \
         line that took its place"
    );
    assert_eq!(machine.mode(), EditMode::Normal);
    assert!(machine.pending().is_clear(), "the count is spent");
}

#[test]
fn a_named_register_is_state_the_yank_reads() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("alpha\nbeta");
    buffer.at(1, 3);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "\"ayy");

    assert_eq!(
        names(&stream),
        [
            "select-register",
            "set-mode",
            "select-range",
            "yank",
            "set-cursor",
            "clear-selection",
            "set-mode",
        ],
        "{stream:#?}"
    );
    assert_eq!(
        buffer.cursor(),
        Position { line: 1, column: 3 },
        "a linewise yank keeps its column (change.txt:1254)"
    );
    let register = stream
        .iter()
        .find_map(|action| match action {
            Action::Buffer(BufferAction::Yank { register, .. }) => register.clone(),
            _ => None,
        })
        .expect("the yank names the register");
    assert_eq!(register, RegisterName("a".to_owned()));
    assert_eq!(
        buffer.registers.get("a").map(String::as_str),
        Some("alpha\n")
    );
    assert_eq!(buffer.content(), "alpha\nbeta", "a yank changes nothing");

    // And it comes back out of the register it went into, not the unnamed one.
    buffer.at(2, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "\"ap");
    assert_eq!(buffer.content(), "alpha\nbalpha\neta");
}

#[test]
fn the_two_counts_multiply() {
    // vim's rule: `2d3w` is six words. The machine holds both halves because
    // the second one arrives after the operator.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("a b c d e f g h");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "2d3w");
    assert_eq!(buffer.content(), "g h");
}

// ---------------------------------------------------------------------------
// Operators, objects and modes
// ---------------------------------------------------------------------------

#[test]
fn an_operator_over_a_text_object_selects_then_acts() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("call(alpha, beta);");
    buffer.at(1, 8);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "ci(");

    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "select-object",
            "select-range",
            "yank",
            "delete",
            "set-cursor",
            "clear-selection",
            "set-mode",
        ],
        "{stream:#?}"
    );
    assert_eq!(buffer.content(), "call();");
    assert_eq!(
        machine.mode(),
        EditMode::Insert,
        "c leaves you in insert, with the undo group still open"
    );
    // What was typed is on the unnamed register, as vim does.
    assert_eq!(
        buffer.registers.get("\"").map(String::as_str),
        Some("alpha, beta")
    );
}

#[test]
fn an_operator_over_a_motion_takes_the_motions_span() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("alpha beta gamma");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "dw");
    assert_eq!(buffer.content(), "beta gamma");

    drive(&mut machine, &mut keymap, &mut buffer, "de");
    assert_eq!(buffer.content(), " gamma", "de takes the last character");
}

#[test]
fn insert_mode_types_text_and_esc_closes_the_undo_group() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("bc");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "ia<esc>");
    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "insert",
            "set-mode",
            "move-cursor",
            "commit-undo-group"
        ],
        "{stream:#?}"
    );
    assert_eq!(buffer.content(), "abc");
    assert_eq!(machine.mode(), EditMode::Normal);
}

#[test]
fn o_opens_a_line_below_and_keeps_the_indent() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("    let x = 1;\nnext");
    buffer.at(1, 5);

    drive(&mut machine, &mut keymap, &mut buffer, "oy<esc>");
    assert_eq!(buffer.content(), "    let x = 1;\n    y\nnext");
}

#[test]
fn visual_mode_extends_a_selection_and_the_operator_takes_it() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("one\ntwo\nthree");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "Vjd");
    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "select-range",
            "extend-selection",
            "select-range",
            "yank",
            "delete",
            "set-cursor",
            "move-cursor",
            "clear-selection",
            "set-mode",
            "commit-undo-group",
        ],
        "{stream:#?}"
    );
    assert_eq!(buffer.content(), "three");
    assert_eq!(machine.mode(), EditMode::Normal);
}

#[test]
fn x_is_a_fused_operator_and_p_puts_it_back() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "x");
    assert_eq!(buffer.content(), "bc");
    drive(&mut machine, &mut keymap, &mut buffer, "p");
    assert_eq!(buffer.content(), "bac");
}

#[test]
fn a_line_address_is_a_count_and_not_a_repeat() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("one\ntwo\nthree\nfour");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "3G");
    assert_eq!(buffer.cursor(), Position { line: 3, column: 1 });
    drive(&mut machine, &mut keymap, &mut buffer, "G");
    assert_eq!(
        buffer.cursor(),
        Position { line: 4, column: 1 },
        "no count means the last line"
    );
    drive(&mut machine, &mut keymap, &mut buffer, "gg");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
}

// ---------------------------------------------------------------------------
// The machinery around the grammar
// ---------------------------------------------------------------------------

#[test]
fn repeat_is_a_re_entry_the_host_drives() {
    // Decision 5: `.` emits `repeat-last`, and the keys come back through
    // `feed` one at a time. Replaying the Actions would delete the same
    // absolute span twice.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("abcdef");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "x");
    let stream = drive(&mut machine, &mut keymap, &mut buffer, ".");
    assert_eq!(names(&stream), ["repeat-last"]);
    assert_eq!(buffer.content(), "bcdef", "the Action itself edits nothing");

    let keys = machine.last_change().expect("x is a change");
    assert_eq!(keys.0, "x");
    drive(&mut machine, &mut keymap, &mut buffer, &keys.0);
    assert_eq!(buffer.content(), "cdef");
}

#[test]
fn an_unbound_key_asks_for_the_hint_rather_than_going_quiet() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("text");

    // `&` — vim's repeat-substitute, which this editor has no search to repeat
    // and so does not bind. (`~` used to stand here and is a case change now.)
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "&");
    assert_eq!(names(&stream), ["show-unknown-key-hint", "cancel-pending"]);
}

#[test]
fn the_table_changes_at_runtime_and_the_next_key_sees_it() {
    // The claim `T033` rests on, at the level this crate can hold it: the
    // machine holds a `Keymap`, not a table, so a binding added between two
    // keystrokes is in force on the second one.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("abc");

    let before = drive(&mut machine, &mut keymap, &mut buffer, "Q");
    assert_eq!(names(&before), ["show-unknown-key-hint", "cancel-pending"]);

    keymap.bind(Scope::Normal, "Q", Role::Motion(Motion::LineEnd));
    let after = drive(&mut machine, &mut keymap, &mut buffer, "Q");
    assert_eq!(names(&after), ["move-cursor"]);
}

#[test]
fn a_binding_the_layer_ran_itself_emits_nothing() {
    /// A layer that handles one key by running scheme of its own.
    #[derive(Debug)]
    struct Layer;

    impl Keymap for Layer {
        fn resolve(&mut self, _: Scope, keys: &[phosphor_core::input::key::Key]) -> _Resolution {
            if keys == parse_seq("z").expect("a key").as_slice() {
                _Resolution::Ran
            } else {
                _Resolution::Unbound
            }
        }
    }
    use phosphor_core::input::table::Resolution as _Resolution;

    let mut machine = Machine::new();
    let mut layer = Layer;
    let mut buffer = Buffer::new("abc");

    let stream = drive(&mut machine, &mut layer, &mut buffer, "z");
    assert!(
        stream.is_empty(),
        "the layer ran a thunk; the machine has nothing to say about it"
    );
}

#[test]
fn the_agent_nouns_parse_and_no_op() {
    // `T028`'s done-when, held here because the grammar is the half that
    // exists: `diu` is accepted, records what was asked for, and deletes
    // nothing — there is no store to resolve `u` against until `T049`.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("one\ntwo");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "diu");
    assert_eq!(
        names(&stream),
        ["set-mode", "select-object", "cancel-pending", "set-mode"],
        "{stream:#?}"
    );
    assert_eq!(buffer.content(), "one\ntwo");
    assert_eq!(machine.mode(), EditMode::Normal);
}

#[test]
fn esc_drops_a_half_typed_command() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("one two");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "3d");
    assert!(!machine.pending().is_clear());
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "<esc>");
    assert_eq!(names(&stream), ["cancel-pending", "set-mode"]);
    assert!(machine.pending().is_clear());
    assert_eq!(machine.mode(), EditMode::Normal);
    assert_eq!(buffer.content(), "one two");
}

#[test]
fn a_scroll_is_the_only_key_that_moves_a_viewport() {
    // Invariant 3, from the input side: the count scales the request rather
    // than emitting three of them.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("a\nb\nc\nd\ne\nf");

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "3<C-e>");
    assert_eq!(names(&stream), ["set-count", "scroll"]);
    let Action::View(phosphor_core::action::ViewAction::Scroll { request, .. }) = &stream[1] else {
        panic!("the scroll key emits a scroll");
    };
    assert_eq!(
        *request,
        phosphor_core::request::ScrollRequest::Rows { rows: 3 }
    );
}

#[test]
fn a_door_moves_the_machine_the_same_way_a_key_does() {
    // `Machine::apply` is the transition arriving from Steel or the CLI. It has
    // to mean the same thing, or `"a` means one thing typed and another
    // evaluated.
    let mut machine = Machine::new();
    machine.apply(&InputAction::SelectRegister {
        register: RegisterName("b".to_owned()),
    });
    machine.apply(&InputAction::SetCount { count: 4 });
    assert_eq!(machine.pending().count(), 4);

    let mut keymap = support::table();
    let mut buffer = Buffer::new("one\ntwo\nthree\nfour\nfive");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "yy");
    assert_eq!(
        buffer.registers.get("b").map(String::as_str),
        Some("one\ntwo\nthree\nfour\n"),
        "the register and the count both came from the door"
    );
}

#[test]
fn quitting_is_an_action_like_everything_else() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("text");
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "ZQ");
    assert_eq!(names(&stream), ["quit"]);
    assert!(!buffer.quit, "the driver does not act on it; the loop does");
}

#[test]
fn every_binding_in_the_fixture_answers_with_a_role() {
    // `Table::bind` drops a spelling it cannot parse, so a typo in the
    // transcription would bind nothing at all. This is what makes it loud.
    let mut table: Table = support::table();
    for scope in [
        Scope::Normal,
        Scope::Insert,
        Scope::Visual,
        Scope::OperatorPending,
        Scope::Object,
    ] {
        for (keys, _) in table.clone().bound(scope) {
            assert!(
                matches!(
                    table.resolve(scope, &keys),
                    phosphor_core::input::table::Resolution::Role(_)
                ),
                "{scope:?} {keys:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// R1 — the motions a vim user reaches for next
// ---------------------------------------------------------------------------

#[test]
fn a_scheme_keymap_can_name_every_new_motion() {
    // **The half of "the keymap is in Steel" that this crate can prove.**
    // `phosphor-steel/src/keymap.rs:297` decodes `(key/motion "…")` as
    // `Motion::from_value` of the tag and nothing else, so a tag that round
    // trips here is a tag `runtime/keymaps.scm` can write with no Rust change
    // at all. The four new *roles* are not like this — `replace-char` and the
    // three case operators need an arm in that file, which `R1`'s report asks
    // for by name.
    use phosphor_core::value::{Value, Wire};

    for (motion, tag) in [
        (Motion::FindCharForward, "find-char-forward"),
        (Motion::FindCharBackward, "find-char-backward"),
        (Motion::TillCharForward, "till-char-forward"),
        (Motion::TillCharBackward, "till-char-backward"),
        (Motion::RepeatFind, "repeat-find"),
        (Motion::RepeatFindReverse, "repeat-find-reverse"),
        (Motion::BigWordForward, "big-word-forward"),
        (Motion::BigWordBackward, "big-word-backward"),
        (Motion::BigWordEnd, "big-word-end"),
    ] {
        assert_eq!(motion.to_value(), Value::Text(tag.to_owned()));
        assert_eq!(
            Motion::from_value(&Value::Text(tag.to_owned())).ok(),
            Some(motion),
            "the layer cannot name {tag}"
        );
    }
    for (case, tag) in [
        (CaseChange::Upper, "upper"),
        (CaseChange::Lower, "lower"),
        (CaseChange::Toggle, "toggle"),
    ] {
        assert_eq!(case.to_value(), Value::Text(tag.to_owned()));
        assert_eq!(
            CaseChange::from_value(&Value::Text(tag.to_owned())).ok(),
            Some(case)
        );
    }
}

#[test]
fn find_and_till_differ_by_one_character_in_both_directions() {
    // **The off-by-one this vocabulary exists to get right.** Four keys, one
    // line, four different columns.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("alpha, beta, gamma");

    // `f,` lands *on* the comma; `t,` stops one short of it.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "f,");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 6 });
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "t,");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 5 });

    // Backwards, `F,` lands on it and `T,` stops one *after* it.
    buffer.at(1, 18);
    drive(&mut machine, &mut keymap, &mut buffer, "F,");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 12
        }
    );
    buffer.at(1, 18);
    drive(&mut machine, &mut keymap, &mut buffer, "T,");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 13
        }
    );

    // A count picks the nth, and the till stops one short of *that* one rather
    // than stalling on the first.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "2f,");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 12
        }
    );
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "2t,");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 11
        }
    );

    // A find that cannot land does not move — and never leaves the line.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "fz");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
}

#[test]
fn an_operator_over_a_find_is_inclusive_forward_and_exclusive_back() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    // `df,` swallows the character it lands on…
    let mut buffer = Buffer::new("alpha, beta");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "df,");
    assert_eq!(buffer.content(), " beta");

    // …`dt,` stops one short of it.
    let mut buffer = Buffer::new("alpha, beta");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "dt,");
    assert_eq!(buffer.content(), ", beta");

    // Backwards is exclusive: the character under the cursor stays.
    let mut buffer = Buffer::new("alpha, beta");
    buffer.at(1, 8);
    drive(&mut machine, &mut keymap, &mut buffer, "dF,");
    assert_eq!(buffer.content(), "alphabeta");

    // A find with nothing to find cancels the operator rather than deleting
    // something else.
    let mut buffer = Buffer::new("alpha, beta");
    buffer.at(1, 1);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "dfz");
    assert_eq!(buffer.content(), "alpha, beta");
    assert_eq!(names(&stream), ["set-mode", "cancel-pending", "set-mode"]);
    assert_eq!(machine.mode(), EditMode::Normal);
}

#[test]
fn semicolon_repeats_the_find_and_comma_reverses_it() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("a,b,c,d");
    buffer.at(1, 1);

    drive(&mut machine, &mut keymap, &mut buffer, "f,");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 2 });
    drive(&mut machine, &mut keymap, &mut buffer, ";");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 4 });
    drive(&mut machine, &mut keymap, &mut buffer, ";");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 6 });
    drive(&mut machine, &mut keymap, &mut buffer, ",");
    assert_eq!(
        buffer.cursor(),
        Position { line: 1, column: 4 },
        "`,` is the same find the other way"
    );
    // `,` does not become the find that `;` repeats.
    drive(&mut machine, &mut keymap, &mut buffer, ";");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 6 });

    // And it composes with an operator, on the remembered character — with the
    // inclusiveness of the find it repeats, so `d;` here is `df,`.
    drive(&mut machine, &mut keymap, &mut buffer, "0d;");
    assert_eq!(buffer.content(), "b,c,d");

    // With nothing found yet, `;` drops the pending command rather than
    // guessing a character.
    let mut fresh = Machine::new();
    let stream = drive(&mut fresh, &mut keymap, &mut buffer, ";");
    assert_eq!(names(&stream), ["cancel-pending"]);
}

#[test]
fn a_big_word_is_everything_that_is_not_blank() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("foo_bar(1); next");

    // `w` stops at the punctuation run; `W` steps over the whole token.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "w");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 8 });
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "W");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 13
        }
    );

    buffer.at(1, 13);
    drive(&mut machine, &mut keymap, &mut buffer, "B");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });

    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "E");
    assert_eq!(
        buffer.cursor(),
        Position {
            line: 1,
            column: 11
        },
        "E ends on the `;`, which `e` would have stopped before"
    );

    // `dW` takes the punctuation with it, which is the whole reason `W` exists.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "dW");
    assert_eq!(buffer.content(), "next");
}

#[test]
fn r_replaces_under_the_cursor_and_repeats() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("abcd");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "rx");
    assert_eq!(names(&stream), ["replace", "commit-undo-group"]);
    assert_eq!(buffer.content(), "xbcd");
    assert_eq!(machine.mode(), EditMode::Normal, "r is not R");

    // The count replaces that many, and refuses when the line is too short —
    // vim's rule, and the reason a partial replace is not a thing.
    buffer.at(1, 2);
    drive(&mut machine, &mut keymap, &mut buffer, "2ry");
    assert_eq!(buffer.content(), "xyyd");
    buffer.at(1, 3);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "9rz");
    assert_eq!(names(&stream), ["set-count", "cancel-pending"]);
    assert_eq!(
        buffer.content(),
        "xyyd",
        "9rz with two characters left refuses"
    );

    // `.` repeats it: the keys round-trip through the notation and back.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "rq");
    let keys = machine.last_change().expect("r is a change");
    assert_eq!(keys.0, "rq");
    buffer.at(1, 2);
    drive(&mut machine, &mut keymap, &mut buffer, &keys.0);
    assert_eq!(buffer.content(), "qqyd");

    // A key that types nothing is not a literal: `<esc>` after `r` cancels.
    buffer.at(1, 1);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "r<esc>");
    assert_eq!(names(&stream), ["cancel-pending"]);
    assert_eq!(buffer.content(), "qqyd");
}

#[test]
fn case_change_is_an_operator_and_tilde_is_the_fused_form() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    // `gUiw` — an operator over an object, and the stream says so.
    let mut buffer = Buffer::new("alpha beta");
    buffer.at(1, 3);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "gUiw");
    assert_eq!(
        names(&stream),
        [
            "set-mode",
            "select-object",
            "select-range",
            "set-case",
            "set-cursor",
            "clear-selection",
            "set-mode",
            "commit-undo-group",
        ],
        "{stream:#?}"
    );
    assert_eq!(buffer.content(), "ALPHA beta");
    assert_eq!(
        buffer.cursor(),
        Position { line: 1, column: 1 },
        "the cursor lands at the start of what was changed, not the end of it \
         (motion.txt:71, *operator-resulting-pos*)"
    );
    assert!(
        stream.iter().any(|action| matches!(
            action,
            Action::Buffer(BufferAction::SetCase {
                case: CaseChange::Upper,
                ..
            })
        )),
        "{stream:#?}"
    );

    // The doubled form is linewise, by the same rule as `dd`.
    let mut buffer = Buffer::new("ALPHA BETA\nNEXT");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "gugu");
    assert_eq!(buffer.content(), "alpha beta\nNEXT");

    // `~` is `g~l` in one key, and a count takes that many characters.
    let mut buffer = Buffer::new("abcdef");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "~");
    assert_eq!(buffer.content(), "Abcdef");
    assert_eq!(
        buffer.cursor(),
        Position { line: 1, column: 2 },
        "`~` switches the case *and moves the cursor to the right* \
         (change.txt:315-318) — the one operator that does not land on its start"
    );
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "3~");
    assert_eq!(buffer.content(), "aBCdef");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 4 });

    // …and `g~l`, the same operator over the same motion unfused, is the
    // general rule instead: it does not move.
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "g~l");
    assert_eq!(buffer.content(), "ABCdef");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
}

// ---------------------------------------------------------------------------
// `B1` — a counted fused operator at the end of a line
// ---------------------------------------------------------------------------

/// **`3x` on `abc` deletes three characters.** It used to delete two.
///
/// vim spells `x` as `dl` and `X` as `dh` (`vim91/doc/change.txt:31-33`,
/// `:41-43`), and `l` is an |exclusive| motion (`motion.txt:189`) that stops
/// "at the end of the line" (`motion.txt:170-171`). For a *cursor* the end of
/// the line is the last character; for an *operand* it is the boundary past it,
/// and conflating the two is what lost the last character. One rule, in
/// `text::char_right_operand`, because five spellings share it — which is why
/// this was deferred rather than patched at `x`.
#[test]
fn a_counted_fused_operator_takes_the_last_character_of_the_line() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    // The count is exactly the line: all three go.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "3x");
    assert_eq!(buffer.content(), "");

    // A count past the end takes what is there rather than refusing — `x` is
    // not `r`, which does refuse (`replace_char`).
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "9x");
    assert_eq!(buffer.content(), "");

    // `d3l` is the same keystroke spelled long, and has to agree.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "d3l");
    assert_eq!(buffer.content(), "");

    // `s` — vim's substitute, the same operand under a different operator.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "3s");
    assert_eq!(machine.mode(), EditMode::Insert);
    drive(&mut machine, &mut keymap, &mut buffer, "Z<esc>");
    assert_eq!(buffer.content(), "Z");

    // `~` — the fused case operator, which is what found this.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "3~");
    assert_eq!(buffer.content(), "ABC");

    // `X` is `dh` and was never wrong: column 1 is a real boundary, so the
    // clamp there is the answer rather than an off-by-one.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 3);
    drive(&mut machine, &mut keymap, &mut buffer, "3X");
    assert_eq!(
        buffer.content(),
        "c",
        "`3X` at column 3 takes the two before it"
    );
}

/// End of line and end of buffer are not the same test.
///
/// At the end of a *line* the newline is the thing that must survive: default
/// `'whichwrap'` does not let `l` cross one, so `3x` on a three-character line
/// empties the line and does not join the next. At the end of the *buffer*
/// there is no newline to leave alone, and the span ends past the last
/// character of the last line — which is the case that runs off the end of
/// every position helper if the rule is written carelessly.
#[test]
fn the_char_right_operand_stops_at_the_newline_and_at_the_end_of_the_buffer() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    // End of line: the line empties, the newline stays, the next line is whole.
    let mut buffer = Buffer::new("abc\ndef");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "5x");
    assert_eq!(
        buffer.content(),
        "\ndef",
        "no join: `l` does not cross a line"
    );

    // End of buffer: the last line, with nothing after it.
    let mut buffer = Buffer::new("abc\ndef");
    buffer.at(2, 2);
    drive(&mut machine, &mut keymap, &mut buffer, "5x");
    assert_eq!(buffer.content(), "abc\nd");

    // The same again where the buffer is one line, so the span's end is the end
    // of everything.
    let mut buffer = Buffer::new("abc");
    buffer.at(1, 2);
    drive(&mut machine, &mut keymap, &mut buffer, "2~");
    assert_eq!(buffer.content(), "aBC");
}

/// An operand of no characters is not an operand: vim beeps, and so does this.
///
/// The alternative is a `Delete` of nothing that still closes an undo group —
/// an empty step in `T029`'s tree, and a `.` that repeats it.
#[test]
fn an_operator_over_nothing_cancels_rather_than_editing_nothing() {
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("\nabc");

    // `x` on an empty line.
    buffer.at(1, 1);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "x");
    assert_eq!(names(&stream), ["cancel-pending"]);

    // `X` and `d0` in column 1, which are the same shape from the other side.
    buffer.at(2, 1);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "X");
    assert_eq!(names(&stream), ["cancel-pending"]);
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "d0");
    assert_eq!(names(&stream), ["set-mode", "cancel-pending", "set-mode"]);

    assert_eq!(buffer.content(), "\nabc", "and nothing was edited");
}

// ---------------------------------------------------------------------------
// `B2` — where an operator leaves the cursor
// ---------------------------------------------------------------------------

/// `*operator-resulting-pos*`, `vim91/doc/motion.txt:71-74`: *"After applying
/// the operator the cursor is mostly left at the start of the text that was
/// operated upon."*
///
/// The exceptions are the interesting half, and they are checked against
/// documentation rather than memory — see [`Machine::land`]'s citations. `gc`
/// and `gs` are not vim's and are not asserted against it here: `gs` moves
/// nothing (it is not an edit) and `gc` takes the general rule by analogy.
#[test]
fn an_operator_lands_the_cursor_at_the_start_of_what_it_touched() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    // The three case operators, from the middle of the word — the report's
    // `gUiw`, and its two siblings.
    for (keys, expected) in [
        ("gUiw", "ALPHA beta"),
        ("guiw", "alpha beta"),
        ("g~iw", "ALPHA beta"),
    ] {
        let mut buffer = Buffer::new("alpha beta");
        buffer.at(1, 3);
        drive(&mut machine, &mut keymap, &mut buffer, keys);
        assert_eq!(buffer.content(), expected, "{keys}");
        assert_eq!(buffer.cursor(), Position { line: 1, column: 1 }, "{keys}");
    }

    // A backwards yank moves the cursor to where the yank started; a forwards
    // one does not move it at all. *"`yfe` doesn't move the cursor, but `yFe`
    // moves the cursor leftwards"* — `motion.txt:73-74`.
    let mut buffer = Buffer::new("alpha beta");
    buffer.at(1, 7);
    drive(&mut machine, &mut keymap, &mut buffer, "yb");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
    assert_eq!(buffer.content(), "alpha beta", "a yank changes nothing");
    buffer.at(1, 1);
    drive(&mut machine, &mut keymap, &mut buffer, "yw");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });

    // `c` lands where the insert begins.
    let mut buffer = Buffer::new("alpha beta");
    buffer.at(1, 3);
    drive(&mut machine, &mut keymap, &mut buffer, "ciw");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
    assert_eq!(machine.mode(), EditMode::Insert);
    drive(&mut machine, &mut keymap, &mut buffer, "X<esc>");
    assert_eq!(buffer.content(), "X beta");
}

/// `'startofline'` — and only for the three operators it names.
///
/// `options.txt:8260-8266` lists *"`d`, `<<`, `==` and `>>` with a linewise
/// operator"*, and `motion.txt:75` says it applies to those and nothing else.
/// So a linewise `d` lands on the first non-blank and a linewise `gU` lands in
/// column 1, which is the distinction this test exists to hold.
#[test]
fn startofline_applies_to_a_linewise_delete_and_not_to_the_case_operators() {
    let mut machine = Machine::new();
    let mut keymap = support::table();

    let mut buffer = Buffer::new("one\n    two\nthree");
    buffer.at(1, 2);
    drive(&mut machine, &mut keymap, &mut buffer, "dd");
    assert_eq!(buffer.content(), "    two\nthree");
    assert_eq!(
        buffer.cursor(),
        Position { line: 1, column: 5 },
        "the first non-blank of the line that took its place"
    );

    // The doubled case operator is linewise by the same rule as `dd`, and lands
    // in column 1 because 'startofline' does not list it.
    let mut buffer = Buffer::new("    two\nthree");
    buffer.at(1, 6);
    drive(&mut machine, &mut keymap, &mut buffer, "gUgU");
    assert_eq!(buffer.content(), "    TWO\nthree");
    assert_eq!(buffer.cursor(), Position { line: 1, column: 1 });
}

#[test]
fn a_find_extends_a_live_selection() {
    // `MoveCursor` and `ExtendSelection` cannot carry the character, so the
    // machine resolves the destination and asks for it absolutely — the path
    // `gg` already takes. What has to be true either way is that visual mode
    // selects up to where the find landed.
    let mut machine = Machine::new();
    let mut keymap = support::table();
    let mut buffer = Buffer::new("alpha, beta");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "vf,");
    assert_eq!(
        names(&stream),
        ["set-mode", "select-range", "select-range", "set-cursor"],
        "{stream:#?}"
    );
    assert_eq!(buffer.cursor(), Position { line: 1, column: 6 });
    let (span, _) = buffer.selection.expect("visual mode selects");
    assert_eq!(span.start, Position { line: 1, column: 1 });
    assert_eq!(
        span.end,
        Position { line: 1, column: 7 },
        "the selection includes the character the find landed on"
    );

    drive(&mut machine, &mut keymap, &mut buffer, "d");
    assert_eq!(buffer.content(), " beta");
}
