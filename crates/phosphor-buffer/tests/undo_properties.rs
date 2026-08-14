//! `T029`'s laws, stated over generated edit sequences rather than six
//! hand-built groups.
//!
//! `tests/undo.rs` is the acceptance test and stays the readable one: it names
//! the vim gestures — `A`, `o`, `3dd`, `ciw`, a batch, a visual replace — and
//! asserts what each does. This file asks the same questions of sequences
//! nobody chose, because the interesting failures in an undo tree are at the
//! boundaries *between* records, not inside one: the first char of the buffer,
//! the last, an empty buffer, and both sides of a newline. The generator emits
//! those on purpose ([`Anchor`]) instead of hoping a random offset lands on one.
//!
//! # Four laws, and the one that is not what it looks like
//!
//! 1. **Undo/redo is exact** — text *and* caret, at every intermediate state.
//!    "The original caret" means where the first change *began*, not where the
//!    cursor sat when the buffer opened: `A` moves the cursor before the group
//!    starts, and undo puts you where the edit was, which is vim's rule and the
//!    one `tests/undo.rs`'s header already writes down.
//! 2. **A divergent redo leaves the branch standing** — the reason this is a
//!    tree. The fork's `History` truncates (`vendor/…/src/history.rs:19-22`);
//!    for any sequence, undoing partway and editing again must leave every
//!    abandoned node reachable by `goto` with its change intact.
//! 3. **`goto` routes through the common ancestor.** The tempting statement of
//!    this — *"for any two nodes, going from one to the other and back is the
//!    identity on text and caret"* — is **false on the caret**, and a property
//!    is how you find that out rather than by arguing about it. `UndoTree::goto`
//!    hands back `change.before` on the way up and `change.after` on the way
//!    down, so the caret you land on is a function of the **edge** you crossed
//!    last, not of the node: leave a node by one child and return by the other
//!    and the two carets differ. That is not a bug — undoing out of a branch
//!    must put you where *that branch's* edit began — so the law is stated in
//!    the three parts that are true, at
//!    `every_node_has_one_text_and_undo_out_redo_back_returns`: text is a pure
//!    function of the node; a **redo** arrival lands on that node's own `after`
//!    whatever the route, because a node has one parent and so one edge from
//!    above; and undo-out-then-redo-back is the identity from anywhere in the
//!    tree. The counterexample proptest shrank to is kept as a seed in
//!    `undo_properties.proptest-regressions`.
//! 4. **Grouping** — an empty group closes to nothing, through every door; and a
//!    change's inverse is the reverse of its edits' inverses, which is the
//!    `.rev()` in `Change::inverse_edits` and the one place a multi-edit group
//!    silently corrupts if it goes missing.
//!
//! Plus the two seams the acceptance test only touches in one direction: the
//! fork's `EditBatch` is written by `Step::to_batch` and read back by
//! `Change::from_fork`, and `T030`'s `into_parts`/`from_parts` pair.
//!
//! `proptest` is `SPIKES.md`'s hygiene choice for exactly this shape of
//! question.

use std::collections::BTreeMap;

use phosphor_buffer::undo::{
    Caret, Change, CharRange, Direction, Edit, Node, NodeId, Step, UndoTree,
};
use proptest::prelude::*;
use ropey::Rope;

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// Where an op lands, generated as an *intent* rather than an offset.
///
/// A uniformly random offset almost never lands on a buffer end and lands on a
/// newline only by luck, and those are the positions an undo tree breaks at —
/// an off-by-one in a record boundary is invisible in the middle of a line.
#[derive(Debug, Clone, Copy)]
enum Anchor {
    /// Char 0, including on an empty buffer.
    Start,
    /// One past the last char.
    End,
    /// The first newline, so a span from here crosses it.
    OnNewline,
    /// Just after the first newline — the start of the second line.
    AfterNewline,
    /// Anywhere, wrapped into range.
    Anywhere(usize),
}

impl Anchor {
    /// Resolves against the text *as it stands now*, which is what makes a
    /// generated script applicable to any base text.
    fn resolve(self, rope: &Rope) -> usize {
        let chars = rope.len_chars();
        let newline = rope.chars().position(|ch| ch == '\n');
        match self {
            Self::Start => 0,
            Self::End => chars,
            Self::OnNewline => newline.unwrap_or(chars),
            Self::AfterNewline => newline.map_or(chars, |at| at + 1),
            Self::Anywhere(n) => n % (chars + 1),
        }
    }
}

fn any_anchor() -> impl Strategy<Value = Anchor> {
    prop_oneof![
        1 => Just(Anchor::Start),
        1 => Just(Anchor::End),
        1 => Just(Anchor::OnNewline),
        1 => Just(Anchor::AfterNewline),
        4 => (0usize..64).prop_map(Anchor::Anywhere),
    ]
}

/// The three shapes an [`Edit`] expresses, before they are resolved against a
/// rope. `len` is a *wanted* length: the resolver clamps it to what is there,
/// so a delete at the end of the buffer becomes the no-op the tree drops.
#[derive(Debug, Clone)]
enum Op {
    Insert {
        at: Anchor,
        text: String,
    },
    Delete {
        at: Anchor,
        len: usize,
    },
    Replace {
        at: Anchor,
        len: usize,
        text: String,
    },
}

impl Op {
    fn to_edit(&self, rope: &Rope) -> Edit {
        let chars = rope.len_chars();
        let cut = |at: Anchor, len: usize| {
            let start = at.resolve(rope);
            let end = start.saturating_add(len).min(chars);
            (start, rope.slice(start..end).to_string())
        };
        match self {
            Self::Insert { at, text } => Edit::insert(at.resolve(rope), text.clone()),
            Self::Delete { at, len } => {
                let (start, removed) = cut(*at, *len);
                Edit::delete(start, removed)
            }
            Self::Replace { at, len, text } => {
                let (start, removed) = cut(*at, *len);
                Edit::replace(start, removed, text.clone())
            }
        }
    }
}

/// Newlines and a multi-byte char are both in the alphabet on purpose: char
/// offsets and byte offsets differ only when the text says so, and a group
/// spanning a line break is the boundary case.
const ALPHABET: &str = "[ab\\né]{1,4}";

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (any_anchor(), ALPHABET).prop_map(|(at, text)| Op::Insert { at, text }),
        (any_anchor(), 0usize..6).prop_map(|(at, len)| Op::Delete { at, len }),
        (any_anchor(), 0usize..6, ALPHABET).prop_map(|(at, len, text)| Op::Replace {
            at,
            len,
            text
        }),
    ]
}

/// A motion, then the edits it precedes. The motion is why `before` and `after`
/// are two different carets rather than the same one twice.
type Group = (Anchor, Vec<Op>);

fn any_script() -> impl Strategy<Value = Vec<Group>> {
    prop::collection::vec((any_anchor(), prop::collection::vec(any_op(), 1..4)), 1..5)
}

/// The empty buffer is in here explicitly — it is the case where every anchor
/// collapses to the same offset and every delete is a no-op.
fn any_base() -> impl Strategy<Value = String> {
    prop_oneof![
        1 => Just(String::new()),
        1 => Just("fn main() {}\n".to_owned()),
        3 => "[ab\\né]{0,24}",
    ]
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Text and caret, which together are what "exact" means.
type State = (String, Caret);

/// A buffer, its caret, and its history — the same three things
/// `tests/undo.rs`'s `Doc` holds, kept separate because an integration test is
/// its own crate.
#[derive(Debug)]
struct Doc {
    rope: Rope,
    caret: Caret,
    tree: UndoTree,
}

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

    fn go(&mut self, offset: usize) {
        self.caret = Caret::at(offset);
    }

    /// One edit inside the open group. Returns whether it was recorded.
    ///
    /// A no-op is skipped entirely rather than applied and dropped, because
    /// `UndoTree::record` drops it and the model has to agree with the tree:
    /// applying it would move this harness's caret past text the tree never
    /// heard about. That the tree drops it is
    /// `an_empty_group_closes_to_nothing`'s claim, not this method's.
    fn edit(&mut self, edit: Edit) -> bool {
        if edit.is_noop() {
            return false;
        }
        let before = self.caret;
        edit.apply(&mut self.rope)
            .expect("every generated edit is cut from the live rope");
        self.caret = Caret::at(edit.at + edit.inserted.chars().count());
        self.tree.record(before, edit);
        true
    }

    /// Applies one group and closes it — a motion, then edits, then
    /// `History::CommitUndoGroup`.
    ///
    /// [`None`] when every op in it was a no-op, which is the same answer
    /// `UndoTree::commit` gives.
    fn group(&mut self, motion: Anchor, ops: &[Op]) -> Option<(NodeId, State, State)> {
        self.go(motion.resolve(&self.rope));
        let before = self.state();
        for op in ops {
            let edit = op.to_edit(&self.rope);
            self.edit(edit);
        }
        let node = self.tree.commit(self.caret)?;
        Some((node, before, self.state()))
    }

    /// Every group in a script, keeping only the ones that became a node.
    fn script(&mut self, script: &[Group]) -> Vec<(NodeId, State, State)> {
        script
            .iter()
            .filter_map(|(motion, ops)| self.group(*motion, ops))
            .collect()
    }

    fn run(&mut self, steps: &[Step]) {
        for step in steps {
            self.caret = step
                .apply(&mut self.rope)
                .expect("a step from this tree applies to this text");
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

    /// `goto`, returning how the last step arrived — [`None`] when the target
    /// was already the current node and no step moved.
    fn goto(&mut self, id: NodeId) -> Option<Direction> {
        let steps = self.tree.goto(id).expect("the id came from this tree");
        let arrival = steps.last().map(|step| step.direction);
        self.run(&steps);
        arrival
    }

    fn ids(&self) -> Vec<NodeId> {
        (0..self.tree.node_count())
            .map(|index| NodeId(index as u64))
            .collect()
    }
}

/// Applies a change's edits to a copy of `text`, front to back.
fn replay(text: &str, edits: &[Edit]) -> Result<String, TestCaseError> {
    let mut rope = Rope::from_str(text);
    for edit in edits {
        edit.apply(&mut rope)
            .map_err(|err| TestCaseError::fail(err.to_string()))?;
    }
    Ok(rope.to_string())
}

// ---------------------------------------------------------------------------
// The laws
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// **Law 1.** Undoing every group returns the original text and the caret
    /// the first change began at; redoing every group returns the final text
    /// and caret. Asserted at every intermediate state in both directions, and
    /// then again with a count, which walks the same path in one call.
    #[test]
    fn undo_all_then_redo_all_is_exact(base in any_base(), script in any_script()) {
        let mut doc = Doc::new(&base);
        let groups = doc.script(&script);
        prop_assert_eq!(doc.tree.node_count(), groups.len() + 1, "one node per group, plus the root");
        if groups.is_empty() {
            return Ok(());
        }

        for (_, before, _) in groups.iter().rev() {
            doc.undo(1);
            let state = doc.state();
            prop_assert_eq!(&state, before, "undo is not exact");
        }
        prop_assert_eq!(doc.tree.current(), NodeId::ROOT);
        prop_assert_eq!(doc.text(), base.clone(), "the text the tree started from");

        for (_, _, after) in &groups {
            doc.redo(1);
            let state = doc.state();
            prop_assert_eq!(&state, after, "redo is not exact");
        }

        let count = u32::try_from(groups.len()).expect("at most four groups");
        doc.undo(count);
        let state = doc.state();
        prop_assert_eq!(&state, &groups[0].1);
        doc.redo(count);
        let state = doc.state();
        prop_assert_eq!(&state, &groups[groups.len() - 1].2);
    }

    /// Undoing past the root and redoing past the leaf stop rather than wrap,
    /// for any sequence and any count — including counts far past the depth of
    /// the tree.
    #[test]
    fn the_ends_of_the_history_hold(
        base in any_base(),
        script in any_script(),
        over in 1u32..64,
    ) {
        let mut doc = Doc::new(&base);
        let groups = doc.script(&script);
        if groups.is_empty() {
            return Ok(());
        }
        // The last *committed* group's end, not `doc.state()`: a trailing group
        // of no-ops moves this harness's caret and creates no node, so the two
        // disagree — correctly.
        let leaf = groups[groups.len() - 1].2.clone();

        doc.undo(over + u32::try_from(groups.len()).expect("at most four groups"));
        prop_assert_eq!(doc.tree.current(), NodeId::ROOT);
        prop_assert_eq!(doc.text(), base);
        doc.undo(over);
        prop_assert_eq!(doc.tree.current(), NodeId::ROOT);

        doc.redo(over + u32::try_from(groups.len()).expect("at most four groups"));
        let state = doc.state();
        prop_assert_eq!(&state, &leaf);
        doc.redo(over);
        let state = doc.state();
        prop_assert_eq!(&state, &leaf);
    }

    /// **Law 2.** The case an undo *stack* loses. Undo partway, edit again, and
    /// every node of the abandoned branch is still there, still carrying its
    /// change, and still reachable — the tree grew by exactly one node rather
    /// than truncating.
    #[test]
    fn a_divergent_edit_leaves_the_branch_standing(
        base in any_base(),
        script in any_script(),
        back in 0usize..8,
        fork_at in any_anchor(),
        fork_text in ALPHABET,
    ) {
        let mut doc = Doc::new(&base);
        let groups = doc.script(&script);
        if groups.is_empty() {
            return Ok(());
        }
        let (leaf, _, leaf_state) = groups[groups.len() - 1].clone();

        let back = u32::try_from(back % groups.len() + 1).expect("at most four groups");
        doc.undo(back);
        let before_fork = doc.tree.node_count();

        doc.go(fork_at.resolve(&doc.rope));
        let at = doc.caret.offset;
        prop_assert!(doc.edit(Edit::insert(at, fork_text)), "a non-empty insert is an edit");
        let forked = doc.tree.commit(doc.caret).expect("a non-empty insert is a step");
        let forked_state = doc.state();

        prop_assert_eq!(
            doc.tree.node_count(),
            before_fork + 1,
            "the divergent edit added a node and truncated nothing",
        );
        for (id, _, after) in &groups {
            let node = doc.tree.node(*id).expect("the abandoned node survives");
            let change = node.change.as_ref().expect("with its change intact");
            prop_assert_eq!(change.after, after.1, "and the caret it recorded");
        }

        // The abandoned branch is reachable, and the route back to the new one
        // goes up to the fork point and down the other side.
        doc.goto(leaf);
        let state = doc.state();
        prop_assert_eq!(&state, &leaf_state, "the abandoned leaf, exactly");
        doc.goto(forked);
        let state = doc.state();
        prop_assert_eq!(&state, &forked_state);
    }

    /// **Law 3**, in its true form. Walking the whole tree in a generated order:
    ///
    /// * a node's **text** is the same however you arrive at it, so `goto` is a
    ///   pure function of the target on text — which subsumes "there and back
    ///   is the identity" on text;
    /// * arriving by **redo** lands on that node's own `after` caret, whatever
    ///   route got you there, because a node has exactly one parent and so
    ///   exactly one edge from above;
    /// * **undo out and redo back is the identity**, from any node in the tree
    ///   and not only along the branch you are standing on. That is the
    ///   `redo_child` bookkeeping in `UndoTree::goto`'s up-loop: leaving a node
    ///   makes the branch you left the live one.
    ///
    /// The caret arriving by **undo** is deliberately *not* keyed by node here,
    /// and that is the finding this property produced: it is a function of the
    /// edge, not the node. Undoing out of a node puts you where *that node's*
    /// change began, so a node with two children has two different undo-arrival
    /// carets, and "there and back is the identity on the caret" is false for a
    /// route that leaves by one child and returns by the other. It is not a bug
    /// — undo from branch A must put you where A's edit was — but it is why the
    /// law is stated in three parts instead of one.
    ///
    /// A route is never longer than the two ancestries it joins, which is the
    /// observable form of "through the common ancestor".
    #[test]
    fn every_node_has_one_text_and_undo_out_redo_back_returns(
        base in any_base(),
        first in any_script(),
        back in 0usize..8,
        second in any_script(),
        order in prop::collection::vec(0usize..64, 2..24),
    ) {
        let mut doc = Doc::new(&base);
        let groups = doc.script(&first);
        if !groups.is_empty() {
            doc.undo(u32::try_from(back % groups.len() + 1).expect("at most four groups"));
        }
        doc.script(&second);

        let ids = doc.ids();
        let mut text_at: BTreeMap<NodeId, String> = BTreeMap::new();
        let mut redo_caret_at: BTreeMap<NodeId, Caret> = BTreeMap::new();

        for index in order {
            let id = ids[index % ids.len()];
            let depth = doc.tree.node_count();
            let steps = doc.tree.goto(id).expect("the id came from this tree");
            prop_assert!(
                steps.len() < 2 * depth,
                "a route joins two ancestries and does not loop",
            );
            let arrival = steps.last().map(|step| step.direction);
            doc.run(&steps);
            prop_assert_eq!(doc.tree.current(), id);

            let text = doc.text();
            if let Some(seen) = text_at.get(&id) {
                prop_assert_eq!(&text, seen, "a node's text depends on the node alone");
            } else {
                text_at.insert(id, text);
            }

            if arrival == Some(Direction::Redo) {
                if let Some(seen) = redo_caret_at.get(&id) {
                    prop_assert_eq!(&doc.caret, seen, "redo into a node lands the same way");
                } else {
                    redo_caret_at.insert(id, doc.caret);
                }
            }

            if doc.tree.node(id).and_then(|node| node.parent).is_some() {
                doc.undo(1);
                doc.redo(1);
                prop_assert_eq!(doc.tree.current(), id, "redo returns to the branch just left");
                let normalised = doc.state();
                doc.undo(1);
                doc.redo(1);
                let again = doc.state();
                prop_assert_eq!(&again, &normalised, "undo out and redo back returns you exactly");
            }
        }
    }

    /// **Law 4a.** An empty group closes to nothing, whichever door opens it:
    /// `begin`/`commit` with nothing between them, a group of no-op edits, and
    /// `record_batch` — the `Buffer::ApplyEdits` primitive — with the same. And
    /// none of them dirties the buffer.
    #[test]
    fn an_empty_group_closes_to_nothing(
        base in any_base(),
        at in 0usize..64,
        text in ALPHABET,
        noops in 1usize..5,
    ) {
        let mut doc = Doc::new(&base);
        let caret = Caret::at(at);
        let noop = || Edit::replace(at, text.clone(), text.clone());

        doc.tree.begin(caret);
        prop_assert!(doc.tree.has_open_group());
        prop_assert_eq!(doc.tree.commit(caret), None, "nothing between begin and commit");

        for _ in 0..noops {
            doc.tree.record(caret, noop());
        }
        prop_assert!(!doc.tree.has_open_group(), "a dropped no-op does not open a group");
        prop_assert_eq!(doc.tree.commit(caret), None, "a group of no-ops");

        let batch: Vec<Edit> = (0..noops).map(|_| noop()).collect();
        prop_assert_eq!(doc.tree.record_batch(caret, batch, caret), None, "a batch of no-ops");

        prop_assert_eq!(doc.tree.node_count(), 1, "root only");
        prop_assert_eq!(doc.tree.current(), NodeId::ROOT);
        prop_assert!(!doc.tree.is_modified(), "an empty group did not dirty the buffer");
    }

    /// **Law 4b.** A change's inverse is the reverse of its edits' inverses.
    ///
    /// Stated against the text rather than against the `Vec`, so it is a claim
    /// about undo and not a restatement of `Change::inverse_edits`: replaying
    /// the inverse over the text the group produced must give back the text the
    /// group started from. Within a group each `at` is against the text after
    /// the previous edit, so dropping the `.rev()` is wrong for every group
    /// whose edits do not commute — which generated groups produce constantly
    /// and a hand-written pair might not.
    #[test]
    fn a_groups_inverse_is_the_reverse_of_its_edits_inverses(
        base in any_base(),
        ops in prop::collection::vec(any_op(), 2..6),
    ) {
        let mut doc = Doc::new(&base);
        let before = doc.text();
        let mut edits = Vec::new();
        for op in &ops {
            let edit = op.to_edit(&doc.rope);
            if doc.edit(edit.clone()) {
                edits.push(edit);
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        let after = doc.text();
        prop_assert_eq!(replay(&before, &edits)?, after.clone(), "the group, replayed");

        let change = Change { edits, before: Caret::at(0), after: Caret::at(0) };
        prop_assert_eq!(replay(&after, &change.inverse_edits())?, before, "and undone");
    }

    /// The seam to the fork. `Step::to_batch` writes an `EditBatch` and
    /// `Change::from_fork` reads one back; the pair is only used in one
    /// direction by the build today, so the round trip is where the other half
    /// gets exercised.
    ///
    /// A replace crosses as two fork edits — a remove and an insert at the same
    /// offset — so the law is equality of the *text they produce*, not of the
    /// `Vec`. The carets are exact: `state_after` carries the step's caret and
    /// `state_before` is deliberately absent, which reads back as the origin.
    #[test]
    fn a_step_round_trips_through_the_forks_batch(
        base in any_base(),
        ops in prop::collection::vec(any_op(), 1..5),
        caret_at in 0usize..64,
        selection in prop::option::of((0usize..64, 0usize..64)),
    ) {
        let mut doc = Doc::new(&base);
        let before = doc.text();
        let mut edits = Vec::new();
        for op in &ops {
            let edit = op.to_edit(&doc.rope);
            if doc.edit(edit.clone()) {
                edits.push(edit);
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        let caret = Caret {
            offset: caret_at,
            selection: selection.map(|(a, b)| CharRange::new(a, b)),
        };
        let step = Step {
            edits: edits.clone(),
            caret,
            to: doc.tree.current(),
            direction: Direction::Redo,
        };

        let parsed = Change::from_fork(&step.to_batch());
        prop_assert_eq!(
            replay(&before, &parsed.edits)?,
            replay(&before, &edits)?,
            "the fork's batch says the same thing ours does",
        );
        prop_assert_eq!(parsed.after, caret, "state_after carries the step's caret");
        prop_assert_eq!(parsed.before, Caret::default(), "an absent state is the origin");
    }

    /// `Caret` crosses to the fork and back unchanged, selection included —
    /// and `CharRange::new` orders, which is what makes that true for a
    /// selection built either way round.
    #[test]
    fn a_caret_round_trips_through_the_forks_state(
        offset in 0usize..64,
        selection in prop::option::of((0usize..64, 0usize..64)),
    ) {
        let caret = Caret {
            offset,
            selection: selection.map(|(a, b)| CharRange::new(a, b)),
        };
        prop_assert_eq!(Caret::from_state(Some(caret.to_state())), caret);
        prop_assert_eq!(Caret::from_state(None), Caret::default());

        if let Some((a, b)) = selection {
            let range = CharRange::new(a, b);
            prop_assert_eq!(range, CharRange::new(b, a), "a range is ordered, not directed");
            prop_assert_eq!(CharRange::new(range.start, range.end), range, "and ordering settles");
            prop_assert_eq!(range.len(), a.abs_diff(b));
            prop_assert_eq!(range.is_empty(), a == b);
        }
    }

    /// `T030`'s contract, over trees with branches rather than one hand-built
    /// one: what `into_parts` hands over comes back through `from_parts` as the
    /// same tree — including from a log that stored only the parent links, since
    /// `children` is recomputed — and the restored tree reaches every node with
    /// the same text.
    #[test]
    fn a_branched_tree_round_trips_through_its_parts(
        base in any_base(),
        first in any_script(),
        back in 0usize..8,
        second in any_script(),
    ) {
        let mut doc = Doc::new(&base);
        let groups = doc.script(&first);
        if !groups.is_empty() {
            doc.undo(u32::try_from(back % groups.len() + 1).expect("at most four groups"));
        }
        doc.script(&second);
        doc.tree.mark_saved();

        let text_now = doc.text();
        let ids = doc.ids();
        let mut text_at: BTreeMap<NodeId, String> = BTreeMap::new();
        for id in &ids {
            doc.goto(*id);
            text_at.insert(*id, doc.text());
        }
        doc.goto(doc.tree.saved().expect("mark_saved set it"));

        let (nodes, current, saved) = doc.tree.clone().into_parts();
        let stripped: Vec<Node> = nodes
            .iter()
            .map(|node| Node { children: Vec::new(), ..node.clone() })
            .collect();
        let restored = UndoTree::from_parts(stripped, current, saved).expect("a tree we built restores");
        prop_assert_eq!(restored.nodes(), nodes.as_slice(), "children are recomputed");
        prop_assert_eq!(restored.current(), current);
        prop_assert_eq!(restored.saved(), saved);

        doc.tree = restored;
        prop_assert_eq!(doc.text(), text_now, "the restored tree is where the old one was");
        prop_assert!(!doc.tree.is_modified(), "and still clean at the saved node");
        for id in &ids {
            doc.goto(*id);
            prop_assert_eq!(&doc.text(), &text_at[id], "every node still reaches its text");
        }

        // `forget_saved` is the scratch-buffer case: no state in this tree
        // matches disk, so the buffer is modified wherever it stands.
        doc.tree.forget_saved();
        prop_assert_eq!(doc.tree.saved(), None);
        prop_assert!(doc.tree.is_modified());
    }
}
