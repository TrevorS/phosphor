//! The keymap the tests in this directory drive, and the check that it is the
//! one the binary loads.
//!
//! # Why this exists at all
//!
//! `T033` put every binding in `runtime/keymaps.scm` and the binary seeds an
//! **empty** [`Table`]; the seed table that used to live in
//! `phosphor-core/src/input/vim.rs` is deleted (`R1`/`R5`). That left the
//! scripted-keystroke tests in this directory driving a keymap nothing loads,
//! which the `S3` gate found and named: *"18 of 20 scripted-stream tests proved
//! a table the binary does not load"*. A grammar test that passes on a table
//! nobody presses is exactly the failure `CP-3` keeps meeting — the widget
//! works, the key does nothing.
//!
//! `phosphor-core` is the floor of the crate graph and cannot embed a Steel VM,
//! so it cannot *evaluate* `runtime/keymaps.scm`. What it can do is refuse to
//! diverge from it: [`table`] is a transcription, and
//! [`the_fixture_is_the_shipped_keymap`] reads the scheme file and fails if a
//! row this fixture claims is not in it.
//!
//! **What that proves, exactly:** these tests exercise key sequences the
//! shipped keymap really binds, to the roles it really binds them to. What it
//! does not prove is that the *layer* decodes those roles — that is
//! `phosphor-steel/tests/no_bindings_in_rust.rs::every_shipped_binding_resolves`,
//! which walks the whole shipped table through the real decoder — nor that the
//! loop composes a frame after them, which is `crates/phosphor/tests`'.
//!
//! Owned by `spine`.

// Two test binaries include this module and neither uses all of it; without
// this, the half one of them does not call is a dead-code warning, and clippy
// denies warnings.
#![allow(dead_code)]

use phosphor_core::action::{Action, AppAction, BufferAction};
use phosphor_core::input::table::{Entry, Goto, Operator, Role, Scope, Table};
use phosphor_core::request::{Motion, ScrollRequest, SelectionKind, Target, TextObject};

/// Every motion, in the three scopes that take one.
///
/// The second column is the wire tag `runtime/keymaps.scm` spells the same
/// motion with, which is what [`the_fixture_is_the_shipped_keymap`] looks for.
const MOTIONS: &[(&str, Motion, &str)] = &[
    ("h", Motion::CharLeft, "char-left"),
    ("<left>", Motion::CharLeft, "char-left"),
    ("l", Motion::CharRight, "char-right"),
    ("<right>", Motion::CharRight, "char-right"),
    ("k", Motion::LineUp, "line-up"),
    ("<up>", Motion::LineUp, "line-up"),
    ("j", Motion::LineDown, "line-down"),
    ("<down>", Motion::LineDown, "line-down"),
    ("w", Motion::WordForward, "word-forward"),
    ("b", Motion::WordBackward, "word-backward"),
    ("e", Motion::WordEnd, "word-end"),
    ("W", Motion::BigWordForward, "big-word-forward"),
    ("B", Motion::BigWordBackward, "big-word-backward"),
    ("E", Motion::BigWordEnd, "big-word-end"),
    ("f", Motion::FindCharForward, "find-char-forward"),
    ("F", Motion::FindCharBackward, "find-char-backward"),
    ("t", Motion::TillCharForward, "till-char-forward"),
    ("T", Motion::TillCharBackward, "till-char-backward"),
    (";", Motion::RepeatFind, "repeat-find"),
    (",", Motion::RepeatFindReverse, "repeat-find-reverse"),
    ("0", Motion::LineStart, "line-start"),
    ("^", Motion::FirstNonBlank, "first-non-blank"),
    ("<home>", Motion::FirstNonBlank, "first-non-blank"),
    ("$", Motion::LineEnd, "line-end"),
    ("<end>", Motion::LineEnd, "line-end"),
    ("{", Motion::ParagraphBackward, "paragraph-backward"),
    ("}", Motion::ParagraphForward, "paragraph-forward"),
    ("%", Motion::MatchingBracket, "matching-bracket"),
    ("H", Motion::ScreenTop, "screen-top"),
    ("M", Motion::ScreenMiddle, "screen-middle"),
    ("L", Motion::ScreenBottom, "screen-bottom"),
    ("<C-d>", Motion::HalfPageDown, "half-page-down"),
    ("<C-u>", Motion::HalfPageUp, "half-page-up"),
];

/// The operators, in the three scopes, so that doubling one (`dd`, `guu`) is a
/// lookup rather than a special case.
const OPERATORS: &[(&str, Operator, &str)] = &[
    ("d", Operator::Delete, "delete"),
    ("c", Operator::Change, "change"),
    ("y", Operator::Yank, "yank"),
    (">", Operator::Indent, "indent"),
    ("<", Operator::Dedent, "dedent"),
    ("gc", Operator::ToggleComment, "toggle-comment"),
    ("gu", Operator::Lower, "lower"),
    ("gU", Operator::Upper, "upper"),
    ("g~", Operator::ToggleCase, "toggle-case"),
    // Teej's ruling, 2026-08-12: `s` stays vim's substitute and mark-seen moves
    // to `gs`, which takes an object — `gsib`. `6d` draws `s`; the mockup is
    // what changes.
    ("gs", Operator::MarkSeen, "mark-seen"),
];

/// The text objects, named after `i` or `a`.
const OBJECTS: &[(&str, TextObject, Option<char>, &str)] = &[
    ("w", TextObject::Word, None, "word"),
    ("W", TextObject::BigWord, None, "big-word"),
    ("s", TextObject::Sentence, None, "sentence"),
    ("p", TextObject::Paragraph, None, "paragraph"),
    ("(", TextObject::Delimited, Some('('), "delimited"),
    (")", TextObject::Delimited, Some('('), "delimited"),
    ("{", TextObject::Delimited, Some('{'), "delimited"),
    ("}", TextObject::Delimited, Some('{'), "delimited"),
    ("[", TextObject::Delimited, Some('['), "delimited"),
    ("]", TextObject::Delimited, Some('['), "delimited"),
    ("<", TextObject::Delimited, Some('<'), "delimited"),
    (">", TextObject::Delimited, Some('<'), "delimited"),
    ("\"", TextObject::Delimited, Some('"'), "delimited"),
    ("'", TextObject::Delimited, Some('\''), "delimited"),
    ("`", TextObject::Delimited, Some('`'), "delimited"),
    ("u", TextObject::UnseenRegion, None, "unseen-region"),
    ("h", TextObject::Hunk, None, "hunk"),
    ("t", TextObject::Thread, None, "thread"),
    ("b", TextObject::Block, None, "block"),
];

/// The fused edits — vim's one-key spellings of an operator and its operand.
const FUSED: &[(&str, Operator, Motion, &str, &str)] = &[
    (
        "x",
        Operator::Delete,
        Motion::CharRight,
        "delete",
        "char-right",
    ),
    (
        "<del>",
        Operator::Delete,
        Motion::CharRight,
        "delete",
        "char-right",
    ),
    (
        "X",
        Operator::Delete,
        Motion::CharLeft,
        "delete",
        "char-left",
    ),
    ("D", Operator::Delete, Motion::LineEnd, "delete", "line-end"),
    ("C", Operator::Change, Motion::LineEnd, "change", "line-end"),
    (
        "s",
        Operator::Change,
        Motion::CharRight,
        "change",
        "char-right",
    ),
    ("Y", Operator::Yank, Motion::LineEnd, "yank", "line-end"),
    // `~` is `g~l` in one key, which is why it is fused rather than a motion.
    (
        "~",
        Operator::ToggleCase,
        Motion::CharRight,
        "toggle-case",
        "char-right",
    ),
];

/// Rows this fixture binds that `runtime/keymaps.scm` does not carry **yet**.
///
/// `R1` extended the vocabulary; the keys are the keymap agent's to add, and
/// this list is what they are adding. It is an allowance, not an assertion —
/// when a row lands in the scheme file the check below simply stops needing
/// the exemption, so nothing here has to be deleted in step.
const NOT_YET_SHIPPED: &[&str] = &[
    "big-word-forward",
    "big-word-backward",
    "big-word-end",
    "find-char-forward",
    "find-char-backward",
    "till-char-forward",
    "till-char-backward",
    "repeat-find",
    "repeat-find-reverse",
    "lower",
    "upper",
    "toggle-case",
    "mark-seen",
    "replace-char",
];

/// The keymap, as `runtime/keymaps.scm` binds it.
#[must_use]
pub(crate) fn table() -> Table {
    let mut table = Table::new();

    for (keys, motion, _) in MOTIONS {
        for scope in [Scope::Normal, Scope::Visual, Scope::OperatorPending] {
            table.bind(scope, keys, Role::Motion(*motion));
        }
    }
    for (keys, operator, _) in OPERATORS {
        for scope in [Scope::Normal, Scope::Visual, Scope::OperatorPending] {
            table.bind(scope, keys, Role::Operator(*operator));
        }
    }
    for (keys, object, delimiter, _) in OBJECTS {
        table.bind(
            Scope::Object,
            keys,
            Role::Object {
                object: *object,
                delimiter: *delimiter,
            },
        );
    }
    for (keys, operator, motion, _, _) in FUSED {
        table.bind(
            Scope::Normal,
            keys,
            Role::Fused {
                operator: *operator,
                motion: *motion,
            },
        );
    }
    for scope in [Scope::Normal, Scope::Visual, Scope::OperatorPending] {
        table.bind(scope, "gg", Role::Goto(Goto::First));
        table.bind(scope, "G", Role::Goto(Goto::Last));
    }

    // In visual these act on the selection, so they are the operator itself.
    table.bind(Scope::Visual, "x", Role::Operator(Operator::Delete));
    table.bind(Scope::Visual, "s", Role::Operator(Operator::Change));

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
    // `r` is not an entry: one character, then normal mode again.
    table.bind(Scope::Normal, "r", Role::ReplaceChar);

    for (keys, kind) in [
        ("v", SelectionKind::Char),
        ("V", SelectionKind::Line),
        ("<C-v>", SelectionKind::Block),
    ] {
        table.bind(Scope::Normal, keys, Role::Select(kind));
        table.bind(Scope::Visual, keys, Role::Select(kind));
    }
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

    for (keys, request) in [
        ("<C-e>", ScrollRequest::Rows { rows: 1 }),
        ("<C-y>", ScrollRequest::Rows { rows: -1 }),
        ("<C-f>", ScrollRequest::Pages { pages: 1 }),
        ("<C-b>", ScrollRequest::Pages { pages: -1 }),
    ] {
        table.bind(Scope::Normal, keys, Role::Scroll(request));
        table.bind(Scope::Visual, keys, Role::Scroll(request));
    }

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
    for keys in ["<C-c>", "ZQ"] {
        table.bind(
            Scope::Normal,
            keys,
            Role::Run(vec![Action::App(AppAction::Quit { force: true })]),
        );
    }
    for scope in [
        Scope::Normal,
        Scope::Insert,
        Scope::Visual,
        Scope::OperatorPending,
        Scope::Object,
    ] {
        table.bind(scope, "<esc>", Role::Escape);
    }
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

/// `runtime/keymaps.scm`, read as text.
fn shipped_keymap() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
        .join("keymaps.scm");
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} is readable", path.display()))
}

/// Whether the scheme file has a row binding `keys` to a role naming `tag`.
///
/// One `(list "keys" (key/… "tag") "verb")` row, on one line, with the keys
/// before the tag — which is the shape every row in that file has, and the
/// order is what keeps `(list ">" (key/operator "indent") …)` from answering
/// for `<`. Deliberately textual: this crate is the floor of the graph and
/// cannot evaluate scheme, and a check that could be fooled by whitespace is
/// still a check that catches a *deleted* binding.
fn shipped_binds(scheme: &str, keys: &str, tag: &str) -> bool {
    let spelled = format!("{keys:?}");
    let wanted = format!("{tag:?}");
    scheme.lines().any(|line| {
        let line = line.trim();
        if !line.starts_with("(list ") {
            return false;
        }
        match (line.find(&spelled), line.find(&wanted)) {
            (Some(at_keys), Some(at_tag)) => at_keys < at_tag,
            _ => false,
        }
    })
}

/// **The check that makes this a transcription rather than a second keymap.**
///
/// Every row above has to be in `runtime/keymaps.scm`, spelled the same way and
/// bound to the same role — unless it is in [`NOT_YET_SHIPPED`], which is the
/// vocabulary `R1` added and the keymap agent binds.
#[test]
fn the_fixture_is_the_shipped_keymap() {
    let scheme = shipped_keymap();
    let mut missing = Vec::new();
    let mut rows: Vec<(&str, &str)> = Vec::new();
    rows.extend(MOTIONS.iter().map(|(keys, _, tag)| (*keys, *tag)));
    rows.extend(OPERATORS.iter().map(|(keys, _, tag)| (*keys, *tag)));
    rows.extend(OBJECTS.iter().map(|(keys, _, _, tag)| (*keys, *tag)));
    rows.extend(FUSED.iter().map(|(keys, _, _, tag, _)| (*keys, *tag)));

    for (keys, tag) in rows {
        if NOT_YET_SHIPPED.contains(&tag) || shipped_binds(&scheme, keys, tag) {
            continue;
        }
        missing.push(format!("{keys} → {tag}"));
    }
    assert!(
        missing.is_empty(),
        "this fixture binds rows runtime/keymaps.scm does not, so the tests \
         driving it prove a keymap the binary never loads. Either the scheme \
         file lost a binding or this transcription drifted:\n{}",
        missing.join("\n")
    );
}
