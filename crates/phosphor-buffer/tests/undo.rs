//! `T029` — undo/redo across a scripted edit sequence, exactly.
//!
//! *Done when: undo/redo across a scripted edit sequence is exact.* "Exact"
//! here is text **and** caret, at every intermediate state, in both directions
//! — a step that restores the characters and leaves the cursor somewhere else
//! is not undo, it is a diff. So each group in the scripted sequence records
//! both of its ends, and undo is asserted against the *before* end and redo
//! against the *after* end, which is the only reading of "exact" that a real
//! keystroke sequence satisfies: `A` moves the cursor before the group starts,
//! and undo puts it back where the change began, not where the previous group
//! finished.
//!
//! The harness below is deliberately the whole story: a `ropey::Rope`, a caret,
//! and an `UndoTree`. Nothing here consults the vendored editor's own history,
//! because `Q2` says we do not use it — except `applies_through_the_fork`,
//! which proves the one path that does touch it.
//!
//! The test that matters most is `a_divergent_edit_leaves_the_branch_standing`.
//! The fork's history truncates on divergence (`vendor/…/src/history.rs:19-22`)
//! and so does every undo stack; that is the case that costs a user work, and
//! it is the reason `T029` says *tree*.

use phosphor_buffer::undo::{
    ApplyError, Caret, Change, CharRange, Direction, Edit, Node, NodeId, RestoreError, Step,
    UndoTree,
};
use ropey::Rope;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A buffer, its caret, and its history — the three things an undo step moves.
#[derive(Debug)]
struct Doc {
    rope: Rope,
    caret: Caret,
    tree: UndoTree,
}

/// Text and caret, which together are what "exact" means.
type State = (String, Caret);

impl Doc {
    fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            caret: Caret::at(0),
            tree: UndoTree::new(),
        }
    }

    fn text(&self) -> String {
        self.rope.to_string()
    }

    fn state(&self) -> State {
        (self.text(), self.caret)
    }

    /// A motion. Moves the caret and nothing else — motions are not edits and
    /// are not in the undo tree.
    fn go(&mut self, offset: usize) {
        self.caret = Caret::at(offset);
    }

    /// Char offset of the first occurrence of `needle`.
    fn find(&self, needle: &str) -> usize {
        self.rope
            .byte_to_char(self.text().find(needle).expect("needle is in the text"))
    }

    /// One edit inside the open group. Records where the caret was first, which
    /// is what undo restores.
    fn edit(&mut self, edit: Edit) {
        let before = self.caret;
        edit.apply(&mut self.rope).expect("edit applies");
        self.caret = Caret::at(edit.at + edit.inserted.chars().count());
        self.tree.record(before, edit);
    }

    /// Types at the caret, one char at a time — an insert session, which is
    /// many edits and exactly one undo step.
    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.edit(Edit::insert(self.caret.offset, ch.to_string()));
        }
    }

    /// `History::CommitUndoGroup`.
    fn commit(&mut self) -> Option<NodeId> {
        self.tree.commit(self.caret)
    }

    fn run(&mut self, steps: &[Step]) {
        for step in steps {
            self.caret = step.apply(&mut self.rope).expect("step applies");
        }
    }

    fn undo(&mut self, count: u32) {
        let steps = self.tree.undo(count);
        self.run(&steps);
    }

    fn redo(&mut self, count: u32) {
        let steps = self.tree.redo(count);
        self.run(&steps);
    }

    fn goto(&mut self, id: NodeId) {
        let steps = self.tree.goto(id).expect("node exists");
        self.run(&steps);
    }
}

// ---------------------------------------------------------------------------
// The acceptance: a scripted sequence, exact in both directions
// ---------------------------------------------------------------------------

/// Six groups, then all the way back, then all the way forward.
///
/// Every group's two ends are captured on the way out and asserted on the way
/// back — so this fails on an off-by-one in a single edit's offset, on a caret
/// restored to the wrong side of an insertion, and on a group boundary landing
/// one edit early or late.
#[test]
fn a_scripted_sequence_undoes_and_redoes_exactly() {
    let mut doc = Doc::new("fn main() {}\n");
    let mut groups: Vec<(State, State)> = Vec::new();

    // 1 · `A` at the end of line 1, then an insert session.
    doc.go(12);
    let before = doc.state();
    doc.type_text("\n    let x = 1;");
    doc.commit();
    groups.push((before, doc.state()));
    assert_eq!(doc.text(), "fn main() {}\n    let x = 1;\n");

    // 2 · `o` — open a line below and type on it.
    doc.go(doc.rope.len_chars());
    let before = doc.state();
    doc.type_text("\nlet y = 2;");
    doc.commit();
    groups.push((before, doc.state()));

    // 3 · `3dd`-shaped: one Delete over a multi-line span, one group.
    let before = doc.state();
    let span = "    let x = 1;\n";
    let at = doc.find(span);
    doc.edit(Edit::delete(at, span));
    doc.commit();
    groups.push((before, doc.state()));
    assert_eq!(doc.text(), "fn main() {}\n\nlet y = 2;");

    // 4 · `ciw`-shaped: a delete and the insert that replaces it, one group,
    //     because `c` leaves the group open (`input.rs:564-565`).
    let before = doc.state();
    let at = doc.find("main");
    doc.edit(Edit::delete(at, "main"));
    doc.type_text("start");
    doc.commit();
    groups.push((before, doc.state()));
    assert_eq!(doc.text(), "fn start() {}\n\nlet y = 2;");

    // 5 · a batch — `Buffer::ApplyEdits`, one group by construction.
    let before = doc.state();
    let header = Edit::insert(0, "// header\n");
    header.apply(&mut doc.rope).expect("batch applies");
    doc.go(10);
    doc.tree
        .record_batch(before.1, [header], doc.caret)
        .expect("a node");
    groups.push((before, doc.state()));

    // 6 · a replace with a selection live going in.
    let at = doc.find("start");
    doc.caret = Caret {
        offset: at,
        selection: Some(CharRange::new(at, at + 5)),
    };
    let before = doc.state();
    let edit = Edit::replace(at, "start", "run");
    edit.apply(&mut doc.rope).expect("replace applies");
    doc.tree.record(before.1, edit);
    doc.go(at + 3);
    doc.commit();
    groups.push((before, doc.state()));
    assert_eq!(doc.text(), "// header\nfn run() {}\n\nlet y = 2;");

    assert_eq!(
        doc.tree.node_count(),
        groups.len() + 1,
        "one node per group, plus the root"
    );

    // Back, one step at a time.
    for group in groups.iter().rev() {
        doc.undo(1);
        assert_eq!(doc.state(), group.0, "undo is not exact");
    }
    assert_eq!(doc.tree.current(), NodeId::ROOT);

    // Forward again, one step at a time.
    for group in &groups {
        doc.redo(1);
        assert_eq!(doc.state(), group.1, "redo is not exact");
    }

    // And a count walks the same path in one call.
    let count = u32::try_from(groups.len()).expect("six");
    doc.undo(count);
    assert_eq!(doc.state(), groups[0].0);
    doc.redo(count);
    assert_eq!(doc.state(), groups.last().expect("six").1);
}

/// Undo past the root and redo past the leaf both stop, rather than wrapping or
/// panicking.
#[test]
fn the_ends_of_the_history_hold() {
    let mut doc = Doc::new("x");
    doc.go(1);
    doc.type_text("y");
    doc.commit();

    doc.undo(99);
    assert_eq!(doc.text(), "x");
    assert_eq!(doc.tree.current(), NodeId::ROOT);
    doc.undo(1);
    assert_eq!(doc.text(), "x");

    doc.redo(99);
    assert_eq!(doc.text(), "xy");
    doc.redo(1);
    assert_eq!(doc.text(), "xy");
}

// ---------------------------------------------------------------------------
// A tree, not a stack
// ---------------------------------------------------------------------------

/// The case an undo *stack* loses: undo, then edit, and the undone work is
/// gone forever.
#[test]
fn a_divergent_edit_leaves_the_branch_standing() {
    let mut doc = Doc::new("hello");

    doc.go(5);
    doc.type_text(" world");
    let first = doc.commit().expect("a node");
    assert_eq!(doc.text(), "hello world");

    doc.go(doc.rope.len_chars());
    doc.type_text("!");
    let second = doc.commit().expect("a node");
    assert_eq!(doc.text(), "hello world!");

    // Back to the start, then somewhere else entirely.
    doc.undo(2);
    assert_eq!(doc.text(), "hello");
    assert_eq!(doc.tree.current(), NodeId::ROOT);

    doc.go(5);
    doc.type_text("?");
    let third = doc.commit().expect("a node");
    assert_eq!(doc.text(), "hello?");

    // The abandoned branch is still there, intact, and still reachable.
    assert!(doc.tree.branches().is_empty(), "the leaf has no children");
    let root = doc.tree.node(NodeId::ROOT).expect("root");
    assert_eq!(
        root.children,
        vec![first, third],
        "both branches hang off the root"
    );
    let kept = doc.tree.node(second).expect("the old leaf survives");
    assert_eq!(
        kept.change.as_ref().expect("its change").edits,
        vec![Edit::insert(11, "!")],
        "and its change is intact"
    );

    // Walk back onto it by id — `History::UndoToCheckpoint`.
    doc.goto(second);
    assert_eq!(doc.text(), "hello world!");
    assert_eq!(doc.tree.current(), second);

    // And back to the new branch, from a node that is not its ancestor: the
    // route goes up to the root and down the other side.
    doc.goto(third);
    assert_eq!(doc.text(), "hello?");
}

/// `redo` follows the branch you last took, which is vim's rule and the only
/// one that is not surprising.
#[test]
fn redo_follows_the_branch_last_walked() {
    let mut doc = Doc::new("");

    doc.type_text("a");
    let a = doc.commit().expect("a node");
    doc.undo(1);

    doc.type_text("b");
    let b = doc.commit().expect("a node");
    doc.undo(1);

    // `b` was created last, so it is where redo goes.
    assert_eq!(doc.tree.branches(), &[a, b]);
    doc.redo(1);
    assert_eq!(doc.text(), "b");
    assert_eq!(doc.tree.current(), b);

    // Walking onto `a` makes `a` the live branch: undo then redo returns there.
    doc.goto(a);
    assert_eq!(doc.text(), "a");
    doc.undo(1);
    doc.redo(1);
    assert_eq!(doc.text(), "a");
    assert_eq!(doc.tree.current(), a);
}

/// A route between two nodes on different branches is exactly the changes that
/// differ — it does not walk to the root when it does not have to.
#[test]
fn a_route_stops_at_the_common_ancestor() {
    let mut doc = Doc::new("");
    doc.type_text("base ");
    let base = doc.commit().expect("a node");

    doc.type_text("left");
    let left = doc.commit().expect("a node");
    doc.goto(base);

    doc.type_text("right");
    let right = doc.commit().expect("a node");
    assert_eq!(doc.text(), "base right");

    let steps = doc.tree.goto(left).expect("left exists");
    assert_eq!(steps.len(), 2, "one up, one down — not via the root");
    assert_eq!(steps[0].direction, Direction::Undo);
    assert_eq!(steps[0].to, base);
    assert_eq!(steps[1].direction, Direction::Redo);
    assert_eq!(steps[1].to, left);

    doc.run(&steps);
    assert_eq!(doc.text(), "base left");
    assert!(doc.tree.node(right).is_some());
}

// ---------------------------------------------------------------------------
// What one step is — `T026`'s decision, honoured
// ---------------------------------------------------------------------------

/// `3dd` folds the count into the operand, so it arrives as one `Buffer::Delete`
/// and is one step. Three separate deletes would be three.
#[test]
fn three_dd_is_one_undo_step() {
    let text = "one\ntwo\nthree\nfour\n";
    let mut doc = Doc::new(text);

    // The span `3dd` covers, as the machine computes it: three whole lines.
    doc.edit(Edit::delete(0, "one\ntwo\nthree\n"));
    doc.commit();
    assert_eq!(doc.text(), "four\n");
    assert_eq!(doc.tree.node_count(), 2, "root plus one step");

    doc.undo(1);
    assert_eq!(doc.text(), text);
}

/// An insert session is many edits and one step.
#[test]
fn an_insert_session_is_one_undo_step() {
    let mut doc = Doc::new("");
    doc.type_text("hello world");
    doc.commit();

    assert_eq!(doc.tree.node_count(), 2);
    let node = doc.tree.node(NodeId(1)).expect("the node");
    let change = node.change.as_ref().expect("a change");
    assert_eq!(change.edits.len(), 11, "one edit per keystroke, one group");

    doc.undo(1);
    assert_eq!(doc.text(), "");
}

/// `input.rs:569-570`: *"A yank changes nothing, so there is no group to close.
/// Closing one anyway would put an empty step in `T029`'s undo tree."* The tree
/// guards it too, so a door sending the Action cannot do it either.
#[test]
fn an_empty_group_leaves_no_step() {
    let mut doc = Doc::new("x");
    assert_eq!(doc.commit(), None);
    doc.tree.begin(Caret::at(0));
    assert_eq!(doc.commit(), None);
    assert_eq!(doc.tree.node_count(), 1, "root only");

    // And a no-op edit is not a step either.
    doc.tree.record(Caret::at(0), Edit::replace(0, "x", "x"));
    assert_eq!(doc.commit(), None);
    assert_eq!(doc.tree.node_count(), 1);
}

/// Undo arriving mid-group closes it deterministically rather than undoing half
/// of it. The machine does not do this — `u` is a normal-mode key and `<esc>`
/// commits first — but a door can.
#[test]
fn undo_mid_group_closes_it_first() {
    let mut doc = Doc::new("");
    doc.type_text("abc");
    assert!(doc.tree.has_open_group());

    doc.undo(1);
    assert_eq!(doc.text(), "", "the whole group, not one keystroke");
    assert_eq!(doc.tree.node_count(), 2, "the group still became a node");
    assert!(!doc.tree.has_open_group());

    doc.redo(1);
    assert_eq!(doc.text(), "abc");
}

// ---------------------------------------------------------------------------
// Edit semantics
// ---------------------------------------------------------------------------

/// Offsets are chars, not bytes, and inversion round-trips through text that
/// makes the difference visible.
#[test]
fn offsets_are_chars_not_bytes() {
    let mut doc = Doc::new("héllo → wörld");
    let at = doc.find("→");
    assert_eq!(at, 6, "char 6, byte 7");

    doc.edit(Edit::replace(at, "→", "->"));
    doc.commit();
    assert_eq!(doc.text(), "héllo -> wörld");

    doc.undo(1);
    assert_eq!(doc.text(), "héllo → wörld");
}

/// Within a group, each edit's offset is against the text as it stands after
/// the previous one — so undo has to reverse the order as well as invert each
/// edit.
#[test]
fn edits_in_a_group_are_sequential() {
    let mut doc = Doc::new("abcdef");
    doc.edit(Edit::delete(0, "ab")); // "cdef"
    doc.edit(Edit::insert(2, "XY")); // "cdXYef"
    doc.edit(Edit::delete(4, "e")); // "cdXYf"
    doc.commit();
    assert_eq!(doc.text(), "cdXYf");

    doc.undo(1);
    assert_eq!(doc.text(), "abcdef");
    doc.redo(1);
    assert_eq!(doc.text(), "cdXYf");
}

/// A step applied against the wrong text is an error at the point of
/// divergence, not a corrupted buffer.
#[test]
fn a_desynchronised_step_is_an_error() {
    let mut rope = Rope::from_str("hello");
    assert_eq!(
        Edit::delete(0, "goodbye").apply(&mut rope),
        Err(ApplyError::OutOfBounds {
            at: 0,
            len: 7,
            chars: 5
        })
    );
    assert_eq!(
        Edit::delete(0, "help").apply(&mut rope),
        Err(ApplyError::Mismatch {
            at: 0,
            expected: "help".to_owned(),
            found: "hell".to_owned()
        })
    );
    assert_eq!(rope.to_string(), "hello", "a rejected edit changed nothing");
}

/// The saved marker is node identity, so undoing back past a write makes the
/// buffer clean again.
#[test]
fn saved_is_a_node_not_a_flag() {
    let mut doc = Doc::new("x");
    assert!(!doc.tree.is_modified());

    doc.go(1);
    doc.type_text("y");
    assert!(doc.tree.is_modified(), "modified with the group still open");
    doc.commit();
    assert!(doc.tree.is_modified());

    doc.tree.mark_saved();
    assert!(!doc.tree.is_modified());

    doc.type_text("z");
    doc.commit();
    assert!(doc.tree.is_modified());

    doc.undo(1);
    assert!(!doc.tree.is_modified(), "back at the state on disk");
    doc.undo(1);
    assert!(doc.tree.is_modified(), "past it again");
}

// ---------------------------------------------------------------------------
// The two seams: the fork, and `T030`
// ---------------------------------------------------------------------------

/// The host path. A step goes through `Editor::apply_batch`, which is what
/// keeps the tree-sitter parse and the highlight cache in step with the text —
/// applying to a bare rope would not.
#[test]
fn applies_through_the_fork() {
    use ratatui_code_editor::editor::Editor;

    let mut editor = Editor::new("rust", "fn main() {}\n", Vec::new()).expect("editor");
    let mut tree = UndoTree::new();

    let edit = Edit::replace(3, "main", "start");
    tree.record(Caret::at(3), edit.clone());
    let node = tree.commit(Caret::at(8)).expect("a node");

    let forward = Step {
        edits: vec![edit],
        caret: Caret::at(8),
        to: node,
        direction: Direction::Redo,
    };
    editor.apply_batch(&forward.to_batch());
    editor.set_cursor(forward.caret.offset);
    assert_eq!(editor.get_content(), "fn start() {}\n");

    for step in tree.undo(1) {
        editor.apply_batch(&step.to_batch());
        editor.set_cursor(step.caret.offset);
    }
    assert_eq!(editor.get_content(), "fn main() {}\n");
    assert_eq!(editor.get_cursor(), 3, "undo restores the caret too");
}

/// `T030`'s contract: what `UndoTree::into_parts` hands over comes back through
/// `UndoTree::from_parts` as the same tree, branches and all.
#[test]
fn a_tree_round_trips_through_its_parts() {
    let mut doc = Doc::new("base");
    doc.go(4);
    doc.type_text(" one");
    let one = doc.commit().expect("a node");
    doc.undo(1);
    doc.type_text(" two");
    let two = doc.commit().expect("a node");
    doc.tree.mark_saved();

    let text_now = doc.text();
    let (nodes, current, saved) = doc.tree.clone().into_parts();
    assert_eq!(current, two);
    assert_eq!(saved, Some(two));

    // A log that stored only the parents round-trips: children are recomputed.
    let stripped: Vec<Node> = nodes
        .iter()
        .map(|node| Node {
            children: Vec::new(),
            ..node.clone()
        })
        .collect();
    let restored = UndoTree::from_parts(stripped, current, saved).expect("restores");
    assert_eq!(restored.nodes(), nodes.as_slice());

    // And it behaves the same: the abandoned branch is still reachable.
    doc.tree = restored;
    assert_eq!(doc.text(), text_now);
    doc.goto(one);
    assert_eq!(doc.text(), "base one");
    assert!(doc.tree.is_modified(), "the saved node is not this one");
}

/// `from_parts` validates rather than trusts — the invariants in the module
/// header, each rejected by name.
#[test]
fn a_malformed_tree_is_rejected() {
    let change = || {
        Some(Change {
            edits: vec![Edit::insert(0, "x")],
            before: Caret::at(0),
            after: Caret::at(1),
        })
    };
    let root = Node {
        parent: None,
        children: Vec::new(),
        redo_child: None,
        change: None,
    };
    let child = Node {
        parent: Some(NodeId::ROOT),
        children: Vec::new(),
        redo_child: None,
        change: change(),
    };

    assert_eq!(
        UndoTree::from_parts(Vec::new(), NodeId::ROOT, None).unwrap_err(),
        RestoreError::Empty
    );

    // 1 · the root carries no change.
    let mut bad = root.clone();
    bad.change = change();
    assert_eq!(
        UndoTree::from_parts(vec![bad], NodeId::ROOT, None).unwrap_err(),
        RestoreError::Shape { id: NodeId::ROOT }
    );

    // 2 · a non-root does, and its parent precedes it.
    let mut bad = child.clone();
    bad.change = None;
    assert_eq!(
        UndoTree::from_parts(vec![root.clone(), bad], NodeId::ROOT, None).unwrap_err(),
        RestoreError::Shape { id: NodeId(1) }
    );
    let mut bad = child.clone();
    bad.parent = Some(NodeId(1));
    assert_eq!(
        UndoTree::from_parts(vec![root.clone(), bad], NodeId::ROOT, None).unwrap_err(),
        RestoreError::Parent { id: NodeId(1) }
    );

    // 3 · a `redo_child` is one of the node's own children.
    let mut bad = root.clone();
    bad.redo_child = Some(NodeId(1));
    assert_eq!(
        UndoTree::from_parts(vec![bad], NodeId::ROOT, None).unwrap_err(),
        RestoreError::RedoChild { id: NodeId::ROOT }
    );

    // 4 · `current` and `saved` name nodes that exist.
    assert_eq!(
        UndoTree::from_parts(vec![root.clone(), child.clone()], NodeId(7), None).unwrap_err(),
        RestoreError::UnknownNode { id: NodeId(7) }
    );
    assert_eq!(
        UndoTree::from_parts(vec![root, child], NodeId::ROOT, Some(NodeId(7))).unwrap_err(),
        RestoreError::UnknownNode { id: NodeId(7) }
    );
}
