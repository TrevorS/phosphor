//! PHOSPHOR PATCH 10 — what the change callback is told, and when.
//!
//! `T102`. **The first three tests fail on the unpatched crate** — the first
//! two panic inside `ropey`, the third reports a column that is off by one.
//! The bug was that `Code::notify_changes` turned each edit's `start` into a
//! `(row, col)` *after* the whole batch had been applied, so a batch whose
//! later edits sit at lower offsets than its earlier ones — which is every
//! undo step — asked the finished rope for a position it no longer had.
//!
//! The last two pass either way and say so in their own doc comments: a
//! one-edit batch addresses the same position before and after, and
//! `Code::undo` never reaches `notify_changes` at all. They are characterisation,
//! not regression, and this header said *"every test here fails on the
//! unpatched crate"* until the review counted them. `VENDOR.md` §10 had it
//! right — *"each of the three tests named for the defect"* — so the two
//! copies of one claim disagreed, which is why there is now one number here
//! and a pointer there.

use ratatui_code_editor::code::{Code, Edit, EditBatch, Operation};
use ratatui_code_editor::editor::Editor;
use std::cell::RefCell;
use std::rc::Rc;

type Change = (usize, usize, usize, usize, String);

/// An editor whose change events are collected, in order, across batches.
fn recording(text: &str) -> (Editor, Rc<RefCell<Vec<Change>>>) {
    let mut editor = Editor::new("text", text, vec![]).unwrap();
    let log: Rc<RefCell<Vec<Change>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&log);
    editor.set_change_callback(Box::new(move |changes| {
        sink.borrow_mut().extend(changes);
    }));
    (editor, log)
}

fn batch(edits: Vec<Edit>) -> EditBatch {
    EditBatch {
        edits,
        state_before: None,
        state_after: None,
    }
}

fn insert(start: usize, text: &str) -> Edit {
    Edit {
        start,
        text: text.to_string(),
        operation: Operation::Insert,
    }
}

fn remove(start: usize, text: &str) -> Edit {
    Edit {
        start,
        text: text.to_string(),
        operation: Operation::Remove,
    }
}

/// Type two characters at the end of the buffer, then undo them.
///
/// The crash phosphor shipped, at its smallest: `A`, `xy`, `<esc>`, `u` on a
/// five-character file exited 101. The inverse step removes `y` at offset 6
/// and then `x` at offset 5, so by the time the batch commits the rope is
/// five characters long and offset 6 is past its end.
#[test]
fn undoing_a_group_typed_at_the_end_of_the_buffer_does_not_panic() {
    let (mut editor, log) = recording("hello");

    editor.apply_batch(&batch(vec![insert(5, "x"), insert(6, "y")]));
    assert_eq!(editor.code_ref().get_content(), "helloxy");

    log.borrow_mut().clear();
    editor.apply_batch(&batch(vec![remove(6, "y"), remove(5, "x")]));

    assert_eq!(editor.code_ref().get_content(), "hello");
    assert_eq!(
        *log.borrow(),
        vec![
            (0, 6, 0, 7, String::new()),
            (0, 5, 0, 6, String::new()),
        ]
    );
}

/// The same shape across a line break, where a stale position is not merely
/// out of range but points at the wrong line.
#[test]
fn undoing_a_multi_line_group_reports_the_lines_the_edits_removed() {
    let (mut editor, log) = recording("a\nb");

    editor.apply_batch(&batch(vec![insert(3, "\nc"), insert(5, "\nd")]));
    assert_eq!(editor.code_ref().get_content(), "a\nb\nc\nd");

    log.borrow_mut().clear();
    editor.apply_batch(&batch(vec![remove(5, "\nd"), remove(3, "\nc")]));

    assert_eq!(editor.code_ref().get_content(), "a\nb");
    assert_eq!(
        *log.borrow(),
        vec![
            (2, 1, 3, 1, String::new()),
            (1, 1, 2, 1, String::new()),
        ]
    );
}

/// A descending batch that never runs out of buffer still reported the wrong
/// column, which is the same defect without the crash to announce it. Phosphor
/// throws these tuples away today (`track_dirty` takes `|_|`); upstream's
/// `examples/lsp` forwards them into a change notification, so this half is the
/// one that is latent here and live for the crate's other consumers.
#[test]
fn a_descending_batch_reports_positions_from_before_its_own_edits() {
    let (mut editor, log) = recording("0123456789\nabc");

    editor.apply_batch(&batch(vec![remove(11, "a"), remove(0, "0")]));

    assert_eq!(editor.code_ref().get_content(), "123456789\nbc");
    // `a` sat at line 1 column 0 when it was removed. Computed against the
    // finished rope — one character shorter on line 0 — it reads as column 1.
    assert_eq!(
        *log.borrow(),
        vec![
            (1, 0, 1, 1, String::new()),
            (0, 0, 0, 1, String::new()),
        ]
    );
}

/// The ordinary single-edit path, unchanged: one insert, one event, at the
/// position the text had. Here so a fix that stopped reporting anything at all
/// would be caught by this file rather than by a language server going quiet.
#[test]
fn a_single_edit_batch_reports_one_change_at_its_own_position() {
    let (mut editor, log) = recording("a\nb\nc");

    editor.apply_batch(&batch(vec![insert(4, "XY")]));

    assert_eq!(editor.code_ref().get_content(), "a\nb\nXYc");
    assert_eq!(*log.borrow(), vec![(2, 0, 2, 0, "XY".to_string())]);
}

/// `Code::undo` does not notify at all — it applies its inverse with
/// `applying_history` false, so nothing joins a batch and nothing commits.
/// Stated as a test because it is the reason the crash needed `apply_batch`
/// to reach it, and the reason phosphor's own undo tree is the caller that
/// found it.
#[test]
fn code_undo_records_no_batch_and_notifies_nothing() {
    let mut code = Code::new("", "text", None).unwrap();
    let log: Rc<RefCell<Vec<Change>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&log);
    code.set_change_callback(Box::new(move |changes| sink.borrow_mut().extend(changes)));

    code.tx();
    code.insert(0, "a");
    code.insert(1, "b");
    code.commit();
    assert_eq!(log.borrow().len(), 2);

    log.borrow_mut().clear();
    code.undo();
    assert_eq!(code.get_content(), "");
    assert!(log.borrow().is_empty());
}
