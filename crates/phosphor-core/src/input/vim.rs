//! The seed keymap — **scaffolding with a demolition date, like `T090` before
//! it.**
//!
//! `T033`'s acceptance is *"every binding lives in `runtime/`, none in Rust"*,
//! and this file is Rust full of bindings. It exists because `CP-3` is
//! *"actually edit something real for a while"* and `runtime/keymaps.scm` binds
//! exactly one key today (`:`, to the REPL) — an editor whose keyboard does
//! nothing cannot be judged on whether vim habits carry.
//!
//! **What `T033` does with it:** transcribe [`table`] into `runtime/keymaps.scm`
//! as `(keymap-set! <scope> <keys> <role>)` forms — the role names are
//! [`Role`]'s arms and [`Scope::name`] is already the word scheme will use —
//! then delete this file and the one line in `main.rs` that seeds it. Nothing
//! else in the machine reads it: [`super::Machine`] holds a
//! [`Keymap`](super::table::Keymap), never a table.
//!
//! # What is deliberately not here
//!
//! * **`:` and the leader.** `runtime/keymaps.scm` owns `:`, and the layer is
//!   asked first in normal mode, so binding it here would be dead weight that
//!   silently disagrees with the editor layer. `SPC` is `T033`'s to define —
//!   the leader tree is the part of the keymap two reasonable users differ on
//!   most, which is exactly the placement test's answer for Steel.
//! * **`f`, `t`, `;`, `,` and `W`/`B`/`E`.** Not omissions of taste:
//!   `request::Motion` is a payload-free `wire_choice!`, so a motion cannot
//!   carry the character `f` needs, and there are no big-word arms. See this
//!   task's report — it is a vocabulary change and the vocabulary has one
//!   writer.
//! * **`:w`, `:q`, `ZZ`.** There is no save path until `T033`; `ZQ` and
//!   `<C-c>` leave, and a `ZZ` that quit without writing would be a lie.

use crate::action::{Action, AppAction, BufferAction};
use crate::request::{Motion, ScrollRequest, SelectionKind, Target, TextObject};

use super::table::{Entry, Goto, Operator, Role, Scope, Table};

/// Every motion, bound the same way in the three scopes that take one.
const MOTIONS: &[(&str, Motion)] = &[
    ("h", Motion::CharLeft),
    ("<left>", Motion::CharLeft),
    ("l", Motion::CharRight),
    ("<right>", Motion::CharRight),
    ("k", Motion::LineUp),
    ("<up>", Motion::LineUp),
    ("j", Motion::LineDown),
    ("<down>", Motion::LineDown),
    ("w", Motion::WordForward),
    ("b", Motion::WordBackward),
    ("e", Motion::WordEnd),
    ("0", Motion::LineStart),
    ("^", Motion::FirstNonBlank),
    ("<home>", Motion::FirstNonBlank),
    ("$", Motion::LineEnd),
    ("<end>", Motion::LineEnd),
    ("{", Motion::ParagraphBackward),
    ("}", Motion::ParagraphForward),
    ("%", Motion::MatchingBracket),
    ("H", Motion::ScreenTop),
    ("M", Motion::ScreenMiddle),
    ("L", Motion::ScreenBottom),
    ("<C-d>", Motion::HalfPageDown),
    ("<C-u>", Motion::HalfPageUp),
];

/// The text objects, named after `i` or `a`.
///
/// The last four are `6d`'s agent nouns. They parse here and resolve at `T049`
/// (`text::object_span` answers [`None`] for all four), which is `T028`'s
/// *"the grammar accepts them and they no-op cleanly rather than erroring"*.
/// `t` is the thread rather than the markup tag, per `request::TextObject` and
/// `6d`.
const OBJECTS: &[(&str, TextObject, Option<char>)] = &[
    ("w", TextObject::Word, None),
    ("W", TextObject::BigWord, None),
    ("s", TextObject::Sentence, None),
    ("p", TextObject::Paragraph, None),
    ("(", TextObject::Delimited, Some('(')),
    (")", TextObject::Delimited, Some('(')),
    ("{", TextObject::Delimited, Some('{')),
    ("}", TextObject::Delimited, Some('{')),
    ("[", TextObject::Delimited, Some('[')),
    ("]", TextObject::Delimited, Some('[')),
    ("<", TextObject::Delimited, Some('<')),
    (">", TextObject::Delimited, Some('<')),
    ("\"", TextObject::Delimited, Some('"')),
    ("'", TextObject::Delimited, Some('\'')),
    ("`", TextObject::Delimited, Some('`')),
    ("u", TextObject::UnseenRegion, None),
    ("h", TextObject::Hunk, None),
    ("t", TextObject::Thread, None),
    ("b", TextObject::Block, None),
];

/// The operators, bound in normal and visual, and again in operator-pending so
/// that doubling one (`dd`, `yy`, `cc`) is a lookup rather than a special case
/// in the resolver.
const OPERATORS: &[(&str, Operator)] = &[
    ("d", Operator::Delete),
    ("c", Operator::Change),
    ("y", Operator::Yank),
    (">", Operator::Indent),
    ("<", Operator::Dedent),
    ("gc", Operator::ToggleComment),
];

/// The seed table.
#[must_use]
pub fn table() -> Table {
    let mut table = Table::new();

    for (keys, motion) in MOTIONS {
        table.bind(Scope::Normal, keys, Role::Motion(*motion));
        table.bind(Scope::Visual, keys, Role::Motion(*motion));
        table.bind(Scope::OperatorPending, keys, Role::Motion(*motion));
    }
    for (keys, operator) in OPERATORS {
        table.bind(Scope::Normal, keys, Role::Operator(*operator));
        table.bind(Scope::Visual, keys, Role::Operator(*operator));
        table.bind(Scope::OperatorPending, keys, Role::Operator(*operator));
    }
    for (keys, object, delimiter) in OBJECTS {
        table.bind(
            Scope::Object,
            keys,
            Role::Object {
                object: *object,
                delimiter: *delimiter,
            },
        );
    }

    // Line addresses: with a count these name a line, so they are not motions.
    table.bind(Scope::Normal, "gg", Role::Goto(Goto::First));
    table.bind(Scope::Visual, "gg", Role::Goto(Goto::First));
    table.bind(Scope::OperatorPending, "gg", Role::Goto(Goto::First));
    table.bind(Scope::Normal, "G", Role::Goto(Goto::Last));
    table.bind(Scope::Visual, "G", Role::Goto(Goto::Last));
    table.bind(Scope::OperatorPending, "G", Role::Goto(Goto::Last));

    // The fused edits — vim's one-key spellings of an operator and its operand.
    for (scope, keys, operator, motion) in [
        (Scope::Normal, "x", Operator::Delete, Motion::CharRight),
        (Scope::Normal, "<del>", Operator::Delete, Motion::CharRight),
        (Scope::Normal, "X", Operator::Delete, Motion::CharLeft),
        (Scope::Normal, "D", Operator::Delete, Motion::LineEnd),
        (Scope::Normal, "C", Operator::Change, Motion::LineEnd),
        (Scope::Normal, "s", Operator::Change, Motion::CharRight),
        (Scope::Normal, "Y", Operator::Yank, Motion::LineEnd),
    ] {
        table.bind(scope, keys, Role::Fused { operator, motion });
    }
    // In visual, these act on the selection, so they are the operator itself.
    table.bind(Scope::Visual, "x", Role::Operator(Operator::Delete));
    table.bind(Scope::Visual, "s", Role::Operator(Operator::Change));

    // Insert-mode entries.
    for (keys, entry) in [
        ("i", Entry::Before),
        ("a", Entry::After),
        ("I", Entry::LineStart),
        ("A", Entry::LineEnd),
        ("o", Entry::OpenBelow),
        ("O", Entry::OpenAbove),
        ("R", Entry::Replace),
    ] {
        table.bind(Scope::Normal, keys, Role::Enter(entry));
    }

    // Visual entries and exits. The same key toggles, which the machine reads
    // as "already in this kind, so go back to normal".
    for (keys, kind) in [
        ("v", SelectionKind::Char),
        ("V", SelectionKind::Line),
        ("<C-v>", SelectionKind::Block),
    ] {
        table.bind(Scope::Normal, keys, Role::Select(kind));
        table.bind(Scope::Visual, keys, Role::Select(kind));
    }
    // `i`/`a` name an object inside an operator and inside a selection.
    table.bind(Scope::OperatorPending, "i", Role::Inner);
    table.bind(Scope::OperatorPending, "a", Role::Around);
    table.bind(Scope::Visual, "i", Role::Inner);
    table.bind(Scope::Visual, "a", Role::Around);

    table.bind(Scope::Normal, "p", Role::Paste { before: false });
    table.bind(Scope::Normal, "P", Role::Paste { before: true });
    table.bind(Scope::Visual, "p", Role::Paste { before: false });
    table.bind(Scope::Normal, "u", Role::History { redo: false });
    table.bind(Scope::Normal, "<C-r>", Role::History { redo: true });
    table.bind(Scope::Normal, ".", Role::Repeat);
    table.bind(Scope::Normal, "\"", Role::Register);
    table.bind(Scope::Visual, "\"", Role::Register);

    // The viewport's only door, and the one place `View::Scroll` is spoken by a
    // key: invariant 3 says nothing else may move it.
    for (keys, request) in [
        ("<C-e>", ScrollRequest::Rows { rows: 1 }),
        ("<C-y>", ScrollRequest::Rows { rows: -1 }),
        ("<C-f>", ScrollRequest::Pages { pages: 1 }),
        ("<C-b>", ScrollRequest::Pages { pages: -1 }),
    ] {
        table.bind(Scope::Normal, keys, Role::Scroll(request));
        table.bind(Scope::Visual, keys, Role::Scroll(request));
    }

    // `J` takes a target rather than a span, so it needs no resolution — which
    // is also why a count cannot reach it (`Role::Run`'s own note).
    table.bind(
        Scope::Normal,
        "J",
        Role::Run(vec![Action::Buffer(BufferAction::JoinLines {
            target: Target::Cursor {},
        })]),
    );
    table.bind(
        Scope::Visual,
        "J",
        Role::Run(vec![Action::Buffer(BufferAction::JoinLines {
            target: Target::Selection {},
        })]),
    );

    // Leaving. `<C-c>` is the safety valve — raw mode means the terminal will
    // not deliver SIGINT, so a host with no binding for it is a host you cannot
    // get out of (`main.rs`'s `key_step` said the same thing before the machine
    // existed). `ZQ` is vim's, and `ZZ` waits for a save path (`T033`).
    for keys in ["<C-c>", "ZQ"] {
        table.bind(
            Scope::Normal,
            keys,
            Role::Run(vec![Action::App(AppAction::Quit { force: true })]),
        );
    }

    // `<esc>` is a mode key in every scope now, including insert.
    for scope in [
        Scope::Normal,
        Scope::Insert,
        Scope::Visual,
        Scope::OperatorPending,
        Scope::Object,
    ] {
        table.bind(scope, "<esc>", Role::Escape);
    }
    // Insert mode: arrows move, everything else is text.
    for (keys, motion) in [
        ("<left>", Motion::CharLeft),
        ("<right>", Motion::CharRight),
        ("<up>", Motion::LineUp),
        ("<down>", Motion::LineDown),
    ] {
        table.bind(Scope::Insert, keys, Role::Motion(motion));
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::key::parse_seq;
    use crate::input::table::{Keymap, Resolution};

    #[test]
    fn the_seed_binds_what_cp3_asks_for() {
        // `Table::bind` drops a spelling it cannot parse, so a typo in the
        // tables above would be silent. This is what makes it loud.
        let mut table = table();
        for (scope, keys) in [
            (Scope::Normal, "d"),
            (Scope::Normal, "c"),
            (Scope::Normal, "y"),
            (Scope::Normal, "\""),
            (Scope::Normal, "<C-c>"),
            (Scope::Normal, "<C-r>"),
            (Scope::Normal, "gg"),
            (Scope::OperatorPending, "i"),
            (Scope::OperatorPending, "d"),
            (Scope::Object, "("),
            (Scope::Object, "u"),
            (Scope::Insert, "<esc>"),
        ] {
            let keys = parse_seq(keys).expect("a spelling this test wrote");
            assert!(
                matches!(table.resolve(scope, &keys), Resolution::Role(_)),
                "{scope:?} {keys:?}"
            );
        }
    }

    #[test]
    fn the_editor_layer_keeps_the_prompt_key_and_the_leader() {
        // Two keys this table must not claim: the layer binds `:` and `T033`
        // defines `SPC`. Binding either here would shadow nothing (the layer is
        // asked first) and disagree with `runtime/keymaps.scm` quietly.
        let mut table = table();
        for keys in [":", "<space>"] {
            let keys = parse_seq(keys).expect("a spelling this test wrote");
            assert_eq!(table.resolve(Scope::Normal, &keys), Resolution::Unbound);
        }
    }
}
