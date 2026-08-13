//! The undo **tree** — `T029`, and the edit semantics underneath it.
//!
//! `Q2` splits undo in two: *"the text engine owns the undo tree and edit
//! semantics; the store serialises it"*. This is the first half. `T030` is the
//! second, and the contract between them is written down at the bottom of this
//! header rather than left to be inferred.
//!
//! # A tree, not a stack
//!
//! The vendored fork's history is a stack that **truncates on divergence**:
//!
//! ```text
//!   pub fn push(&mut self, batch: EditBatch) {
//!       while self.edits.len() > self.index {
//!           self.edits.pop_back();          // vendor/…/src/history.rs:19-22
//!       }
//! ```
//!
//! So under the fork, undoing an edit and then typing anything at all destroys
//! the undone edit permanently. That is the failure this module exists to not
//! have: a redo that diverges leaves the branch it walked away from standing,
//! reachable by id.
//!
//! ```text
//!            root ─── 1 ─── 2            "hello" → " world" → "!"
//!              │
//!              ╰───── 3                  undo to root, type "?" → node 3
//!
//!   after that sequence: `redo` follows node 3 (the branch just taken),
//!   nodes 1 and 2 are intact, and `goto(NodeId(2))` walks back to them.
//! ```
//!
//! Every reachable state of the buffer is a [`NodeId`], and that id is exactly
//! what `History::UndoToCheckpoint`'s `CheckpointId` names — both are opaque
//! non-negative integers, and this module's are dense and assigned in creation
//! order.
//!
//! # What one undo step is — decided by `T026`, not here
//!
//! The input machine's header states it as a decision with a reason:
//!
//! > **`3dd` is one Action, and the count folds into the operand.** […] if a
//! > count emitted three Actions, `u` would either undo one third of a `3dd` or
//! > the machine would have to teach the undo model to group them, which puts
//! > the grouping rule in two places. One keystroke sequence, one edit, one
//! > undo step.
//!
//! This module honours that and adds nothing to it. **A step is a group, and
//! the machine says where a group ends** — `History::CommitUndoGroup` is
//! *"closes the current undo group explicitly"*, and it is emitted at exactly
//! the three places vim closes one: leaving insert or replace mode
//! (`phosphor-core/src/input.rs:711`), finishing a non-`c` operator
//! (`input.rs:572`) and finishing a paste (`input.rs:376`). There is no
//! time-based or size-based coalescing here, because there is no second opinion
//! about grouping to be had.
//!
//! Two consequences worth stating, both matching the machine:
//!
//! * **A group with no edits closes to nothing.** `input.rs:569-571` declines to
//!   emit `CommitUndoGroup` after a yank *"A yank changes nothing, so there is
//!   no group to close. Closing one anyway would put an empty step in `T029`'s
//!   undo tree."* — [`UndoTree::commit`] returns [`None`] rather than trusting
//!   that, so the same is true when a door sends the Action directly.
//! * **`c` deliberately leaves the group open** (`input.rs:564-565`): the
//!   delete and the insert that follows are one step, closed by `<esc>`.
//!
//! # Edits are sequential, not simultaneous
//!
//! An [`Edit`]'s `at` is a **char** offset into the text *as it stands after
//! every earlier edit in the same [`Change`] has been applied* — the fork
//! records offsets that way (`code.rs:495`, `code.rs:522`, both pushed at the
//! moment of application), and so do we. Inverting a change is therefore
//! *reverse the order and swap `removed` with `inserted`*, which is
//! [`Change::inverse_edits`], and which is what `Code::undo` does at
//! `code.rs:695-704`. Read a `Vec<Edit>` any other way and it is silently wrong
//! only for multi-edit changes.
//!
//! An [`Edit`] is a replacement, so it expresses insert (`removed` empty),
//! delete (`inserted` empty) and replace in one shape. It carries the removed
//! text, which is what makes the tree invertible without keeping a copy of the
//! buffer at every node.
//!
//! # Applying a step
//!
//! [`UndoTree::undo`], [`UndoTree::redo`] and [`UndoTree::goto`] move the tree's
//! cursor and hand back the [`Step`]s that make the text agree. They do not
//! own the text: this crate's buffer is the fork's `Code`, whose `insert` and
//! `remove` also drive the tree-sitter `InputEdit` (`code.rs:502-511`,
//! `code.rs:529-538`). A host that applied a step straight to a `Rope` would
//! keep the text and lose the parse.
//!
//! So there are two apply paths and they are not interchangeable:
//!
//! * [`Step::to_batch`] → `Editor::apply_batch` (`editor.rs:561`) — the host
//!   path. Keeps the parse tree and the highlight cache honest.
//!   `apply_batch` does **not** move the cursor; the host sets it from
//!   [`Step::caret`].
//! * [`Step::apply`] → a `ropey::Rope` — the test path, and the reference
//!   implementation of what "exact" means. It verifies that the text it is
//!   removing is the text the edit recorded, so a desynchronised tree is an
//!   [`ApplyError`] rather than a corrupted buffer.
//!
//! # What `T030` has to write, and read back
//!
//! Persistence is `phosphor-core`'s and the format is designed there, once,
//! shared with seen-state (`T044`). This tree is the thing it has to hold, and
//! it is deliberately plain: no `Rope`, no fork type, no `Instant`.
//!
//! **To write** — [`UndoTree::into_parts`] hands over everything, and nothing
//! else in this struct is state:
//!
//! * `Vec<Node>`, dense and in creation order. Each [`Node`] is `parent`,
//!   `children`, `redo_child` and `change`; each [`Change`] is `Vec<Edit>` plus
//!   a `before` and `after` [`Caret`]; each [`Edit`] is three fields, two of
//!   them `String`. `children` is derivable from the parents and need not be
//!   stored.
//! * `current` — where the buffer is now.
//! * `saved` — the node the file on disk matches, or [`None`] if it matches no
//!   node in this tree. [`UndoTree::is_modified`] is `current != saved` plus
//!   *"a group is open with something in it"*, and nothing else — no separate
//!   dirty flag exists to get out of step with it.
//!
//! An open group is deliberately **not** in `into_parts`: it is a half-typed
//! insert, and half a keystroke sequence is not a state anything should be
//! restored into. A host that wants the tail of an insert session persisted
//! commits the group first.
//!
//! **To read back** — [`UndoTree::from_parts`], which validates rather than
//! trusts. Four invariants an append-only log gets for free and a compaction
//! pass has to preserve deliberately:
//!
//! 1. Node `0` is the root: no parent, no change.
//! 2. Every other node has a change and a parent, and **the parent's id is
//!    smaller than its own**. This is what makes a truncated log safe: a torn
//!    record at the tail can be dropped and everything before it is still a
//!    well-formed tree, because a child never precedes its parent.
//! 3. A node's `redo_child`, if set, is one of its `children`.
//! 4. `current` and `saved` name nodes that exist.
//!
//! **Two things the tree does not carry, on purpose.**
//!
//! * **No timestamps.** The Action vocabulary has no time-travel verb — vim's
//!   `g-`/`g+` are not in it — so nothing in the editor could read one. If the
//!   log wants a wall-clock stamp per record it is the log's field.
//! * **No text snapshot.** The root is *"the buffer as it was when this tree
//!   started"*, and it is implicit. That is fine for an append-only log and it
//!   is the one thing **compaction must not forget**: dropping the oldest nodes
//!   moves the root, so the compacted file has to carry the full text at the
//!   new root alongside the surviving tree. Compaction that drops nodes without
//!   writing that base text produces a history that replays into garbage, and
//!   nothing in this module can detect it — [`Edit::apply`]'s mismatch check
//!   will fire, which turns silent corruption into a loud one, but the history
//!   is gone either way.
//!
//! Owned by `surface`.

use std::collections::HashSet;
use std::fmt;

use ratatui_code_editor::code::{Edit as ForkEdit, EditBatch, EditState, Operation};
use ratatui_code_editor::selection::Selection;
use ropey::Rope;

// ---------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------

/// A point in the undo tree, and the wire form of a checkpoint.
///
/// One-to-one with `phosphor_core::request::CheckpointId`, which is a `u64`
/// for the same reason: it crosses MCP as a non-negative integer. Ids are
/// dense and assigned in creation order, so `NodeId(0)` is always the root and
/// a node's parent always has a smaller id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u64);

impl NodeId {
    /// The state the buffer was in when the tree started.
    pub const ROOT: Self = Self(0);

    fn index(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A half-open range of **char** offsets.
///
/// Not `phosphor_core::request::Span`, which is a pair of 1-based
/// line/column `Position`s: that is the vocabulary's shape because an Action
/// crosses MCP, and this is the engine's shape because a rope indexes by char.
/// Whoever applies an Action converts between them once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CharRange {
    /// First char in the range.
    pub start: usize,
    /// First char after it.
    pub end: usize,
}

impl CharRange {
    /// A range, ordered — `new(9, 4)` is `4..9`.
    #[must_use]
    pub fn new(a: usize, b: usize) -> Self {
        Self {
            start: a.min(b),
            end: a.max(b),
        }
    }

    /// How many chars it covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether it covers nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// Where the cursor and selection were.
///
/// Recorded on both sides of every [`Change`] because undo restores it: vim
/// puts you back where you were, and a step that got the text right and the
/// cursor wrong is not exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caret {
    /// Char offset of the cursor.
    pub offset: usize,
    /// The selection, if there was one.
    pub selection: Option<CharRange>,
}

impl Caret {
    /// A caret with no selection.
    #[must_use]
    pub fn at(offset: usize) -> Self {
        Self {
            offset,
            selection: None,
        }
    }

    /// The fork's equivalent, for `Code::set_state_before` and friends.
    #[must_use]
    pub fn to_state(self) -> EditState {
        EditState {
            offset: self.offset,
            selection: self
                .selection
                .map(|range| Selection::new(range.start, range.end)),
        }
    }

    /// The reverse. An absent state is the origin, which is what the fork's own
    /// `EditBatch::new` leaves behind (`code.rs:42-48`).
    #[must_use]
    pub fn from_state(state: Option<EditState>) -> Self {
        state.map_or_else(Self::default, |state| Self {
            offset: state.offset,
            selection: state
                .selection
                .map(|selection| CharRange::new(selection.start, selection.end)),
        })
    }
}

// ---------------------------------------------------------------------------
// Edits
// ---------------------------------------------------------------------------

/// One replacement: at char offset `at`, `removed` becomes `inserted`.
///
/// Both directions are carried, which is what makes the tree invertible without
/// snapshotting the buffer. `removed` empty is an insertion, `inserted` empty a
/// deletion, both empty a no-op that [`UndoTree::record`] drops.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edit {
    /// Char offset, against the text as it stands after every earlier edit in
    /// the same [`Change`].
    pub at: usize,
    /// What was there.
    pub removed: String,
    /// What replaces it.
    pub inserted: String,
}

impl Edit {
    /// Text arriving at `at`.
    pub fn insert(at: usize, text: impl Into<String>) -> Self {
        Self {
            at,
            removed: String::new(),
            inserted: text.into(),
        }
    }

    /// Text leaving `at`. The caller passes what was there — this type never
    /// reads the buffer.
    pub fn delete(at: usize, text: impl Into<String>) -> Self {
        Self {
            at,
            removed: text.into(),
            inserted: String::new(),
        }
    }

    /// One for the other, in a single edit.
    pub fn replace(at: usize, removed: impl Into<String>, inserted: impl Into<String>) -> Self {
        Self {
            at,
            removed: removed.into(),
            inserted: inserted.into(),
        }
    }

    /// Whether this changes nothing.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.removed == self.inserted
    }

    /// The edit that undoes this one, at the same offset.
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            at: self.at,
            removed: self.inserted.clone(),
            inserted: self.removed.clone(),
        }
    }

    /// Applies it to a rope, verifying that what is being removed is what was
    /// recorded.
    ///
    /// The check is the point: it turns a desynchronised tree into an error at
    /// the moment of divergence instead of a buffer that is quietly wrong.
    ///
    /// # Errors
    ///
    /// [`ApplyError::OutOfBounds`] if the span runs past the end of the rope,
    /// [`ApplyError::Mismatch`] if the text there is not `removed`.
    pub fn apply(&self, rope: &mut Rope) -> Result<(), ApplyError> {
        let len = self.removed.chars().count();
        let chars = rope.len_chars();
        let end = self.at.checked_add(len).filter(|end| *end <= chars).ok_or(
            ApplyError::OutOfBounds {
                at: self.at,
                len,
                chars,
            },
        )?;

        if len > 0 {
            let found = rope.slice(self.at..end).to_string();
            if found != self.removed {
                return Err(ApplyError::Mismatch {
                    at: self.at,
                    expected: self.removed.clone(),
                    found,
                });
            }
            rope.remove(self.at..end);
        }
        if !self.inserted.is_empty() {
            rope.insert(self.at, &self.inserted);
        }
        Ok(())
    }

    /// The fork's form — one entry for an insert or a delete, two for a
    /// replace, in the order `Editor::apply_batch` replays them
    /// (`editor.rs:571-581`).
    #[must_use]
    pub fn to_fork(&self) -> Vec<ForkEdit> {
        let mut out = Vec::with_capacity(2);
        if !self.removed.is_empty() {
            out.push(ForkEdit {
                start: self.at,
                text: self.removed.clone(),
                operation: Operation::Remove,
            });
        }
        if !self.inserted.is_empty() {
            out.push(ForkEdit {
                start: self.at,
                text: self.inserted.clone(),
                operation: Operation::Insert,
            });
        }
        out
    }

    /// The reverse: what the fork recorded, as ours.
    #[must_use]
    pub fn from_fork(edit: &ForkEdit) -> Self {
        match edit.operation {
            Operation::Insert => Self::insert(edit.start, edit.text.clone()),
            Operation::Remove => Self::delete(edit.start, edit.text.clone()),
        }
    }
}

/// What went wrong applying an [`Edit`] to a rope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// The span runs past the end of the text.
    OutOfBounds {
        /// Char offset the edit names.
        at: usize,
        /// How many chars it wanted to remove.
        len: usize,
        /// How many the rope has.
        chars: usize,
    },
    /// The text at the offset is not the text the edit recorded — the tree and
    /// the buffer have diverged.
    Mismatch {
        /// Char offset the edit names.
        at: usize,
        /// What the edit recorded.
        expected: String,
        /// What is actually there.
        found: String,
    },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { at, len, chars } => {
                write!(f, "edit at {at} removes {len} chars past the end ({chars})")
            }
            Self::Mismatch {
                at,
                expected,
                found,
            } => write!(f, "edit at {at} expected {expected:?}, found {found:?}"),
        }
    }
}

impl std::error::Error for ApplyError {}

// ---------------------------------------------------------------------------
// Changes and steps
// ---------------------------------------------------------------------------

/// One undo step: the edits of a single group, and the caret on both sides.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    /// In application order. Each `at` is against the text after the previous.
    pub edits: Vec<Edit>,
    /// Where the caret was before the group.
    pub before: Caret,
    /// Where it ended up.
    pub after: Caret,
}

impl Change {
    /// The edits that undo this change, in the order they must be applied:
    /// reversed, each inverted.
    #[must_use]
    pub fn inverse_edits(&self) -> Vec<Edit> {
        self.edits.iter().rev().map(Edit::inverse).collect()
    }

    /// What the fork recorded, as a [`Change`].
    ///
    /// The fork's `Code` builds one of these per `tx`/`commit` pair
    /// (`code.rs:468-486`); this is how a host that lets the fork do the
    /// editing gets it into our tree.
    #[must_use]
    pub fn from_fork(batch: &EditBatch) -> Self {
        Self {
            edits: batch.edits.iter().map(Edit::from_fork).collect(),
            before: Caret::from_state(batch.state_before),
            after: Caret::from_state(batch.state_after),
        }
    }
}

/// Which way a [`Step`] moves through the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards the root: the change on the node being left is inverted.
    Undo,
    /// Away from it: the change on the node being entered is replayed.
    Redo,
}

/// One node's worth of movement — apply these edits in order, then put the
/// caret here.
///
/// A route is a `Vec<Step>` and is applied front to back. Splitting per node
/// rather than flattening keeps `to` meaningful, which is what lets a caller
/// stop early or report where it got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The edits, in application order.
    pub edits: Vec<Edit>,
    /// Where the caret belongs afterwards.
    pub caret: Caret,
    /// The node this step arrives at.
    pub to: NodeId,
    /// Which way it went.
    pub direction: Direction,
}

impl Step {
    /// Applies the step to a rope and returns the caret it ends at.
    ///
    /// The reference implementation of a step, and the one the tests measure
    /// "exact" against. A host with a parse tree wants [`Step::to_batch`]
    /// instead.
    ///
    /// # Errors
    ///
    /// Whatever [`Edit::apply`] returns, at the first edit that fails. Earlier
    /// edits in the same step have already been applied — a failure here means
    /// the tree and the text had already diverged.
    pub fn apply(&self, rope: &mut Rope) -> Result<Caret, ApplyError> {
        for edit in &self.edits {
            edit.apply(rope)?;
        }
        Ok(self.caret)
    }

    /// The step as an `EditBatch`, for `Editor::apply_batch`.
    ///
    /// `state_after` carries [`Step::caret`] so the fork's own record of this
    /// batch is not blank; `state_before` is [`None`] because a step does not
    /// know where the caret was, only where it is going. `apply_batch` does not
    /// move the cursor either way (`editor.rs:561-584`) — the host does that.
    #[must_use]
    pub fn to_batch(&self) -> EditBatch {
        EditBatch {
            edits: self.edits.iter().flat_map(Edit::to_fork).collect(),
            state_before: None,
            state_after: Some(self.caret.to_state()),
        }
    }
}

// ---------------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------------

/// One state of the buffer, and how it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The state this one was reached from. [`None`] only for the root.
    pub parent: Option<NodeId>,
    /// States reached from here, in creation order.
    pub children: Vec<NodeId>,
    /// Which child `redo` takes — the branch most recently created or walked.
    pub redo_child: Option<NodeId>,
    /// The change that turns the parent's text into this one's. [`None`] only
    /// for the root.
    pub change: Option<Change>,
}

impl Node {
    fn root() -> Self {
        Self {
            parent: None,
            children: Vec::new(),
            redo_child: None,
            change: None,
        }
    }
}

/// Why a persisted tree would not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    /// No nodes at all — a tree always has at least a root.
    Empty,
    /// The root has a parent or a change, or a non-root has neither.
    Shape {
        /// The offending node.
        id: NodeId,
    },
    /// A parent id is out of range, or is not smaller than its child's.
    Parent {
        /// The child whose parent is wrong.
        id: NodeId,
    },
    /// A `redo_child` is not one of that node's children.
    RedoChild {
        /// The node whose `redo_child` is wrong.
        id: NodeId,
    },
    /// `current`, `saved` or a child id names a node that is not there.
    UnknownNode {
        /// The missing id.
        id: NodeId,
    },
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("undo tree has no root node"),
            Self::Shape { id } => write!(f, "node {id} has the wrong shape for its position"),
            Self::Parent { id } => write!(f, "node {id} has an out-of-order or missing parent"),
            Self::RedoChild { id } => {
                write!(f, "node {id}'s redo_child is not one of its children")
            }
            Self::UnknownNode { id } => write!(f, "node {id} does not exist"),
        }
    }
}

impl std::error::Error for RestoreError {}

/// A group that has been opened and not yet closed.
#[derive(Debug, Clone)]
struct Open {
    edits: Vec<Edit>,
    before: Caret,
}

/// The undo tree of one buffer.
///
/// Holds no text. Every state is reachable by replaying changes from the root,
/// which is what keeps it small enough to persist and what makes the root's
/// text `T030`'s problem rather than this module's — see the header.
#[derive(Debug, Clone)]
pub struct UndoTree {
    nodes: Vec<Node>,
    current: NodeId,
    saved: Option<NodeId>,
    open: Option<Open>,
}

impl Default for UndoTree {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoTree {
    /// A tree at the root, with the buffer matching what is on disk.
    ///
    /// `saved` starts at the root because that is what "just opened the file"
    /// means; a scratch buffer that was never on disk gets [`UndoTree::forget_saved`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::root()],
            current: NodeId::ROOT,
            saved: Some(NodeId::ROOT),
            open: None,
        }
    }

    // -- reading ------------------------------------------------------------

    /// Where the buffer is now.
    #[must_use]
    pub fn current(&self) -> NodeId {
        self.current
    }

    /// Every node, indexed by [`NodeId`].
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// One node, or [`None`] if the id is not in this tree.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        id.index().and_then(|index| self.nodes.get(index))
    }

    /// How many states the tree holds, root included. Never zero.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The states reachable forward from here — the branch points a UI would
    /// offer, and the ids `History::UndoToCheckpoint` takes.
    #[must_use]
    pub fn branches(&self) -> &[NodeId] {
        match self.node(self.current) {
            Some(node) => node.children.as_slice(),
            None => &[],
        }
    }

    /// Whether a group is open — an edit has been recorded and not yet closed
    /// by `History::CommitUndoGroup`.
    #[must_use]
    pub fn has_open_group(&self) -> bool {
        self.open.is_some()
    }

    /// The node the file on disk matches.
    #[must_use]
    pub fn saved(&self) -> Option<NodeId> {
        self.saved
    }

    /// Whether the buffer differs from disk.
    ///
    /// Node identity, not text comparison: undoing back to the saved state
    /// makes the buffer clean again, which is the behaviour vim has and the
    /// one an editor that can undo past a write has to have.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| !open.edits.is_empty())
            || self.saved != Some(self.current)
    }

    /// Records that the buffer was written to disk here.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    /// Records that no state in this tree matches disk — a scratch buffer, or
    /// a file changed underneath us.
    pub fn forget_saved(&mut self) {
        self.saved = None;
    }

    // -- recording ----------------------------------------------------------

    /// Opens a group, if one is not open already.
    ///
    /// Idempotent on purpose: `c` leaves its group open across a mode change
    /// (`input.rs:564-565`), so every edit that follows calls this and the
    /// original `before` is the one that survives.
    pub fn begin(&mut self, before: Caret) {
        if self.open.is_none() {
            self.open = Some(Open {
                edits: Vec::new(),
                before,
            });
        }
    }

    /// Records one edit into the open group, opening it first if needed.
    ///
    /// A no-op edit is dropped rather than stored: `u` should not have a step
    /// that changes nothing to walk over.
    pub fn record(&mut self, before: Caret, edit: Edit) {
        if edit.is_noop() {
            return;
        }
        self.begin(before);
        if let Some(open) = self.open.as_mut() {
            open.edits.push(edit);
        }
    }

    /// Closes the open group, returning the node it became.
    ///
    /// [`None`] when there was nothing to close, or when the group was empty —
    /// which is the yank case `input.rs:569-571` describes and declines to
    /// emit. Guarding it here as well means a door sending
    /// `History::CommitUndoGroup` directly cannot put an empty step in the
    /// tree either.
    pub fn commit(&mut self, after: Caret) -> Option<NodeId> {
        let open = self.open.take()?;
        if open.edits.is_empty() {
            return None;
        }
        Some(self.push(Change {
            edits: open.edits,
            before: open.before,
            after,
        }))
    }

    /// Records a whole group at once — the `Buffer::ApplyEdits` primitive,
    /// *"applies a batch of edits as one undo group"*.
    ///
    /// Closes anything already open first, so a batch is never folded into a
    /// half-typed insert.
    pub fn record_batch(
        &mut self,
        before: Caret,
        edits: impl IntoIterator<Item = Edit>,
        after: Caret,
    ) -> Option<NodeId> {
        self.close_dangling();
        let edits: Vec<Edit> = edits.into_iter().filter(|edit| !edit.is_noop()).collect();
        if edits.is_empty() {
            return None;
        }
        Some(self.push(Change {
            edits,
            before,
            after,
        }))
    }

    fn push(&mut self, change: Change) -> NodeId {
        let id = NodeId(u64::try_from(self.nodes.len()).unwrap_or(u64::MAX));
        let parent = self.current;
        self.nodes.push(Node {
            parent: Some(parent),
            children: Vec::new(),
            redo_child: None,
            change: Some(change),
        });
        if let Some(index) = parent.index()
            && let Some(node) = self.nodes.get_mut(index)
        {
            node.children.push(id);
            node.redo_child = Some(id);
        }
        self.current = id;
        id
    }

    /// Closes an open group without a caret to close it with.
    ///
    /// Reached only when undo, redo or a batch arrives mid-group, which the
    /// machine does not do — it emits `CommitUndoGroup` on `<esc>` before
    /// normal mode, and `u` is a normal-mode key. A door can, so the fallback
    /// is deterministic rather than a panic: the caret lands where the last
    /// edit left it, which is where typing would have put it.
    fn close_dangling(&mut self) {
        if let Some(open) = self.open.as_ref() {
            let caret = open.edits.last().map_or(open.before, |edit| {
                Caret::at(edit.at + edit.inserted.chars().count())
            });
            let _ = self.commit(caret);
        }
    }

    // -- moving -------------------------------------------------------------

    /// Steps back `count` times, stopping at the root.
    ///
    /// The returned steps are already reflected in [`UndoTree::current`]; the
    /// caller owes the text.
    pub fn undo(&mut self, count: u32) -> Vec<Step> {
        self.close_dangling();
        let mut target = self.current;
        for _ in 0..count.max(1) {
            match self.node(target).and_then(|node| node.parent) {
                Some(parent) => target = parent,
                None => break,
            }
        }
        self.goto(target).unwrap_or_default()
    }

    /// Steps forward `count` times along the branch last created or walked.
    ///
    /// Where a node has children but no `redo_child` — which only a restored
    /// tree can be in — the most recent child wins, matching what `redo_child`
    /// would have said.
    pub fn redo(&mut self, count: u32) -> Vec<Step> {
        self.close_dangling();
        let mut target = self.current;
        for _ in 0..count.max(1) {
            match self.node(target).and_then(Self::forward) {
                Some(child) => target = child,
                None => break,
            }
        }
        self.goto(target).unwrap_or_default()
    }

    fn forward(node: &Node) -> Option<NodeId> {
        node.redo_child.or_else(|| node.children.last().copied())
    }

    /// Walks to any node in the tree — `History::UndoToCheckpoint`, and the
    /// only way back onto a branch that a divergent edit walked away from.
    ///
    /// Routes up to the common ancestor and back down, so the steps are exactly
    /// the changes that differ between here and there. Returns [`None`] if the
    /// id is not in this tree; an empty `Vec` if it is already the current one.
    pub fn goto(&mut self, target: NodeId) -> Option<Vec<Step>> {
        self.close_dangling();
        self.node(target)?;

        let up = self.ancestry(self.current);
        let down = self.ancestry(target);
        let on_path: HashSet<NodeId> = down.iter().copied().collect();
        let meet = up
            .iter()
            .copied()
            .find(|id| on_path.contains(id))
            .unwrap_or(NodeId::ROOT);

        let mut steps = Vec::new();

        // Up: invert the change on each node we leave, and remember the branch
        // we came from so `redo` returns to it.
        for id in up.iter().copied().take_while(|id| *id != meet) {
            let Some(node) = self.node(id) else { continue };
            let Some(parent) = node.parent else { continue };
            let Some(change) = node.change.as_ref() else {
                continue;
            };
            steps.push(Step {
                edits: change.inverse_edits(),
                caret: change.before,
                to: parent,
                direction: Direction::Undo,
            });
            if let Some(index) = parent.index()
                && let Some(parent_node) = self.nodes.get_mut(index)
            {
                parent_node.redo_child = Some(id);
            }
        }

        // Down: replay each change we enter, marking it as the live branch.
        for id in down
            .iter()
            .copied()
            .take_while(|id| *id != meet)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let Some(node) = self.node(id) else { continue };
            let Some(parent) = node.parent else { continue };
            let Some(change) = node.change.as_ref() else {
                continue;
            };
            steps.push(Step {
                edits: change.edits.clone(),
                caret: change.after,
                to: id,
                direction: Direction::Redo,
            });
            if let Some(index) = parent.index()
                && let Some(parent_node) = self.nodes.get_mut(index)
            {
                parent_node.redo_child = Some(id);
            }
        }

        self.current = target;
        Some(steps)
    }

    /// `id`, then its parent, then its parent's parent, up to and including the
    /// root.
    fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = vec![id];
        let mut at = id;
        while let Some(parent) = self.node(at).and_then(|node| node.parent) {
            out.push(parent);
            at = parent;
        }
        out
    }

    // -- persistence, `T030`'s half -----------------------------------------

    /// Everything `T030` has to write. See this module's header for the format
    /// contract.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Node>, NodeId, Option<NodeId>) {
        (self.nodes, self.current, self.saved)
    }

    /// Rebuilds a tree from what was written, validating rather than trusting.
    ///
    /// `children` is **recomputed** from the parent links and whatever the
    /// nodes arrived with is discarded, so a log that stores only parents
    /// round-trips and a log that stores both cannot disagree with itself.
    ///
    /// # Errors
    ///
    /// A [`RestoreError`] naming the first node that breaks one of the four
    /// invariants in this module's header.
    pub fn from_parts(
        nodes: Vec<Node>,
        current: NodeId,
        saved: Option<NodeId>,
    ) -> Result<Self, RestoreError> {
        if nodes.is_empty() {
            return Err(RestoreError::Empty);
        }

        let count = u64::try_from(nodes.len()).unwrap_or(u64::MAX);
        let mut rebuilt: Vec<Node> = nodes
            .into_iter()
            .map(|node| Node {
                children: Vec::new(),
                ..node
            })
            .collect();

        if rebuilt[0].parent.is_some() || rebuilt[0].change.is_some() {
            return Err(RestoreError::Shape { id: NodeId::ROOT });
        }

        for index in 1..rebuilt.len() {
            let id = NodeId(u64::try_from(index).unwrap_or(u64::MAX));
            if rebuilt[index].change.is_none() {
                return Err(RestoreError::Shape { id });
            }
            let parent = rebuilt[index].parent.ok_or(RestoreError::Parent { id })?;
            if parent.0 >= id.0 {
                return Err(RestoreError::Parent { id });
            }
            let parent_index = parent.index().ok_or(RestoreError::Parent { id })?;
            rebuilt[parent_index].children.push(id);
        }

        for (index, node) in rebuilt.iter().enumerate() {
            let id = NodeId(u64::try_from(index).unwrap_or(u64::MAX));
            if let Some(child) = node.redo_child
                && !node.children.contains(&child)
            {
                return Err(RestoreError::RedoChild { id });
            }
        }

        for id in std::iter::once(current).chain(saved) {
            if id.0 >= count {
                return Err(RestoreError::UnknownNode { id });
            }
        }

        Ok(Self {
            nodes: rebuilt,
            current,
            saved,
            open: None,
        })
    }
}
