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
use phosphor_core::input::key::parse_seq;
use phosphor_core::input::table::{Keymap, Role, Scope, Table};
use phosphor_core::input::text::{Text, Viewport};
use phosphor_core::input::{Machine, vim};
use phosphor_core::request::{
    EditMode, Motion, Position, RegisterName, SelectionKind, Span, Target,
};

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
    let mut keymap = vim::table();
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
    assert_eq!(machine.mode(), EditMode::Normal);
    assert!(machine.pending().is_clear(), "the count is spent");
}

#[test]
fn a_named_register_is_state_the_yank_reads() {
    let mut machine = Machine::new();
    let mut keymap = vim::table();
    let mut buffer = Buffer::new("alpha\nbeta");
    buffer.at(1, 1);

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "\"ayy");

    assert_eq!(
        names(&stream),
        [
            "select-register",
            "set-mode",
            "select-range",
            "yank",
            "clear-selection",
            "set-mode",
        ],
        "{stream:#?}"
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
    let mut buffer = Buffer::new("    let x = 1;\nnext");
    buffer.at(1, 5);

    drive(&mut machine, &mut keymap, &mut buffer, "oy<esc>");
    assert_eq!(buffer.content(), "    let x = 1;\n    y\nnext");
}

#[test]
fn visual_mode_extends_a_selection_and_the_operator_takes_it() {
    let mut machine = Machine::new();
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
    let mut buffer = Buffer::new("text");

    let stream = drive(&mut machine, &mut keymap, &mut buffer, "~");
    assert_eq!(names(&stream), ["show-unknown-key-hint", "cancel-pending"]);
}

#[test]
fn the_table_changes_at_runtime_and_the_next_key_sees_it() {
    // The claim `T033` rests on, at the level this crate can hold it: the
    // machine holds a `Keymap`, not a table, so a binding added between two
    // keystrokes is in force on the second one.
    let mut machine = Machine::new();
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
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

    let mut keymap = vim::table();
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
    let mut keymap = vim::table();
    let mut buffer = Buffer::new("text");
    let stream = drive(&mut machine, &mut keymap, &mut buffer, "ZQ");
    assert_eq!(names(&stream), ["quit"]);
    assert!(!buffer.quit, "the driver does not act on it; the loop does");
}

#[test]
fn every_seed_binding_answers_with_a_role() {
    // The seed is data and `Table::bind` drops a spelling it cannot parse, so
    // this is what makes a typo in `vim.rs` loud rather than silent.
    let mut table: Table = vim::table();
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
