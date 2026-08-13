//! The keymap seam: what a key *means*, asked of a table that changes at
//! runtime.
//!
//! # The one decision this module exists to make
//!
//! `T033` puts every binding in `runtime/*.scm`, redefinable from the REPL, and
//! `SPIKES.md`'s `T009` verdict is that a compile-time `HashMap` of 185 entries
//! *"is the wrong shape for a keymap that changes at runtime"*. So the machine
//! never holds a table — it holds a **[`Keymap`]**, and asks it. Two things
//! implement one today: [`Table`], which is data anyone can mutate, and the
//! binary's adapter over `runtime/keymaps.scm`, which asks the VM.
//!
//! The vocabulary in between is [`Role`] — what a key *plays* in the grammar,
//! not what it does. That is the whole seam, and the reason it is a small
//! closed enum rather than a closure: **a closure would have to come from
//! Steel**, and `request::Binding`'s own header records why no `SteelVal` may
//! ride in a payload. `Role` crosses the barrier as plain data; `T033` writes
//! scheme that names these and needs no Rust change to do it.
//!
//! # Scopes, and why they are not [`EditMode`]
//!
//! A table is looked up in a [`Scope`], and there is one scope
//! ([`Scope::Object`]) that is not a mode: the state between `i` and the object
//! key in `ci(`. Keying the table on `EditMode` would put `w` — a motion in
//! operator-pending, a word object after `i` — in one slot with two meanings.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as MapEntry;

use crate::action::Action;
use crate::request::{EditMode, Motion, ScrollRequest, SelectionKind, TextObject};

use super::key::{Key, parse_seq};

/// Where a lookup happens.
///
/// Ordered so a [`Table`]'s `BTreeMap` groups by scope and a prefix search is
/// one range scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// Keys are commands.
    Normal,
    /// Keys are text — only the bindings that escape it live here.
    Insert,
    /// A selection is live.
    Visual,
    /// An operator is waiting for its operand.
    OperatorPending,
    /// `i` or `a` has been typed and the next key names the object. **Not a
    /// mode** — the statusline still reads `OPERATOR-PENDING`.
    Object,
}

impl Scope {
    /// The scope a mode is looked up in.
    #[must_use]
    pub const fn of(mode: EditMode) -> Self {
        match mode {
            EditMode::Normal => Self::Normal,
            EditMode::Insert | EditMode::Replace => Self::Insert,
            EditMode::VisualChar | EditMode::VisualLine | EditMode::VisualBlock => Self::Visual,
            EditMode::OperatorPending => Self::OperatorPending,
        }
    }

    /// The scope's name, as scheme will spell it at `T033`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Insert => "insert",
            Self::Visual => "visual",
            Self::OperatorPending => "operator-pending",
            Self::Object => "object",
        }
    }
}

/// What an operator does to its operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// `d`.
    Delete,
    /// `c` — delete, then insert.
    Change,
    /// `y`.
    Yank,
    /// `>`.
    Indent,
    /// `<`.
    Dedent,
    /// `gc` (`T037` builds it; the grammar carries it now).
    ToggleComment,
    /// `s` — marks its operand seen, which is what makes `sib` a sentence
    /// (`T028`, screen `6d`: *"mark inner block seen — `s` composes like an
    /// operator"*).
    ///
    /// **The one operator that is not an edit.** It changes no text, so it
    /// opens no undo group and fills no register; what it moves is seen-state,
    /// which Design Language §7 calls the only mutable flag the user owns. The
    /// Action it lowers to is `Region::MarkSeen`, built by `T041` — until then
    /// a door answers *"`T041` builds this"*, which is the vocabulary's own
    /// design for a capability that is named before it is built.
    MarkSeen,
}

/// How insert mode is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// `i` — before the cursor.
    Before,
    /// `a` — after it.
    After,
    /// `I` — at the first non-blank.
    LineStart,
    /// `A` — at the end of the line.
    LineEnd,
    /// `o` — a new line below.
    OpenBelow,
    /// `O` — a new line above.
    OpenAbove,
    /// `R` — replace.
    Replace,
}

/// `gg` and `G`: a count names a line, and no count means an end of the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goto {
    /// `gg`.
    First,
    /// `G`.
    Last,
}

/// What a key plays in the grammar.
///
/// The machine composes these; it does not interpret keys. Adding a verb is a
/// row in a table — in Rust today, in scheme at `T033` — and never a new arm in
/// the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// A cursor motion, and an operator's operand.
    Motion(Motion),
    /// A line address: `gg`, `G`.
    Goto(Goto),
    /// An operator, waiting for an operand.
    Operator(Operator),
    /// An operator with its operand baked in — `x` is `dl`, `D` is `d$`.
    Fused {
        /// What to do.
        operator: Operator,
        /// To what.
        motion: Motion,
    },
    /// A text object, named after `i` or `a`.
    Object {
        /// Which object.
        object: TextObject,
        /// The delimiter, for [`TextObject::Delimited`].
        delimiter: Option<char>,
    },
    /// `i` in [`Scope::OperatorPending`] — the next key names an inner object.
    Inner,
    /// `a` in [`Scope::OperatorPending`].
    Around,
    /// Enters insert mode.
    Enter(Entry),
    /// Enters or leaves a visual mode.
    Select(SelectionKind),
    /// `p` / `P`.
    Paste {
        /// Before the cursor rather than after it.
        before: bool,
    },
    /// `u` / `<C-r>`. The count rides on the Action.
    History {
        /// Forwards rather than back.
        redo: bool,
    },
    /// `<C-e>`, `<C-f>`, `zz` — the viewport's only door.
    Scroll(ScrollRequest),
    /// `.`.
    Repeat,
    /// `<esc>` — drop the pending count, register and operator.
    Escape,
    /// `"` — the next key names a register.
    Register,
    /// A binding that is just capabilities, in order. **This is what a scheme
    /// `(keymap-set! "…" (capability …))` becomes**; the count and register do
    /// not reach it, because a named capability already carries its arguments.
    Run(Vec<Action>),
}

/// What the layer said about a sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// It plays this part in the grammar.
    Role(Role),
    /// The layer ran a binding of its own. **Arbitrary scheme ran** — the
    /// machine emits nothing and the caller's frame is stale (`main.rs`).
    Ran,
    /// A prefix of something longer. Wait for the next key.
    Pending,
    /// Nothing here.
    Unbound,
}

/// A live keymap.
///
/// `&mut self` because the only implementation that matters re-enters a VM to
/// answer, and answering may run a binding.
pub trait Keymap {
    /// What `keys` means in `scope`.
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution;
}

/// A keymap that is data.
///
/// The seed table (`super::vim`) is one of these, the tests build their own,
/// and `T033` deletes the seed rather than this type — a scheme layer that
/// wants a Rust-side table for speed can still fill one.
#[derive(Debug, Clone, Default)]
pub struct Table {
    entries: BTreeMap<(Scope, Vec<Key>), Role>,
}

impl Table {
    /// An empty table. **The default, deliberately**: an invented default
    /// keymap is a decision two reasonable users would want to differ on, made
    /// by nobody (`runtime/keymaps.scm`'s own header).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a sequence, spelled the way a keymap is written — `"gg"`,
    /// `"<C-r>"`, `"SPC f"`.
    ///
    /// A spelling this cannot parse binds nothing. It is not an error type
    /// because both callers are literal tables checked by their own tests
    /// (`vim::the_seed_binds_what_cp3_asks_for`), and a `Result` here would be
    /// unwrapped at every one of eighty call sites.
    pub fn bind(&mut self, scope: Scope, keys: &str, role: Role) {
        if let Some(parsed) = parse_seq(keys) {
            self.entries.insert((scope, parsed), role);
        }
    }

    /// Drops a binding.
    pub fn unbind(&mut self, scope: Scope, keys: &str) {
        if let Some(parsed) = parse_seq(keys) {
            self.entries.remove(&(scope, parsed));
        }
    }

    /// Every sequence bound in a scope, for which-key and `:help` (`T034`,
    /// `T086`).
    #[must_use]
    pub fn bound(&self, scope: Scope) -> Vec<(Vec<Key>, &Role)> {
        self.entries
            .range((scope, Vec::new())..)
            .take_while(|((at, _), _)| *at == scope)
            .map(|((_, keys), role)| (keys.clone(), role))
            .collect()
    }

    /// How many bindings there are, in every scope.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether some longer sequence starts with `keys`.
    fn is_prefix(&self, scope: Scope, keys: &[Key]) -> bool {
        self.entries
            .range((scope, keys.to_vec())..)
            .take_while(|((at, _), _)| *at == scope)
            .any(|((_, bound), _)| bound.len() > keys.len() && bound.starts_with(keys))
    }

    /// Merges another table over this one, entry by entry, keeping this one's
    /// binding where both have one.
    pub fn under(&mut self, other: &Self) {
        for (key, role) in &other.entries {
            if let MapEntry::Vacant(slot) = self.entries.entry(key.clone()) {
                slot.insert(role.clone());
            }
        }
    }
}

impl Keymap for Table {
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution {
        if let Some(role) = self.entries.get(&(scope, keys.to_vec())) {
            return Resolution::Role(role.clone());
        }
        if self.is_prefix(scope, keys) {
            return Resolution::Pending;
        }
        Resolution::Unbound
    }
}

/// The editor layer over a seed table — the shape the binary runs.
///
/// **The order changes with the scope, and that is the whole rule.** In normal,
/// insert and visual the *layer is asked first*, so a `(keymap-set! …)` shadows
/// the seed, which is the right way round (`main.rs`'s `key_step` header made
/// the same call for the host's three keys). Mid-operator — after `d`, or
/// between `i` and its object — the **seed is asked first**, because a binding
/// on `w` must not swallow the `w` in `dw`; an operator's operand belongs to the
/// grammar.
///
/// A [`Resolution::Pending`] from whichever is asked first stops the fall
/// through: the second table cannot know the first is mid-sequence.
pub struct Layered<'a> {
    layer: &'a mut dyn Keymap,
    seed: &'a mut Table,
}

impl core::fmt::Debug for Layered<'_> {
    /// The layer is a VM and has no useful rendering; the seed is data.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Layered")
            .field("layer", &"<live>")
            .field("seed", &self.seed)
            .finish()
    }
}

impl<'a> Layered<'a> {
    /// The layer, and what it falls back to.
    pub fn new(layer: &'a mut dyn Keymap, seed: &'a mut Table) -> Self {
        Self { layer, seed }
    }
}

impl Keymap for Layered<'_> {
    fn resolve(&mut self, scope: Scope, keys: &[Key]) -> Resolution {
        let grammar_first = matches!(scope, Scope::OperatorPending | Scope::Object);
        let (first, second): (&mut dyn Keymap, &mut dyn Keymap) = if grammar_first {
            (self.seed, self.layer)
        } else {
            (self.layer, self.seed)
        };
        match first.resolve(scope, keys) {
            Resolution::Unbound => second.resolve(scope, keys),
            answered => answered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::key::Key;

    fn keys(spelled: &str) -> Vec<Key> {
        parse_seq(spelled).expect("a spelling the tests wrote")
    }

    fn table() -> Table {
        let mut table = Table::new();
        table.bind(Scope::Normal, "w", Role::Motion(Motion::WordForward));
        table.bind(Scope::Normal, "gg", Role::Goto(Goto::First));
        table.bind(Scope::Normal, "gc", Role::Operator(Operator::ToggleComment));
        table
    }

    #[test]
    fn a_lookup_is_bound_pending_or_unbound() {
        let mut table = table();
        assert_eq!(
            table.resolve(Scope::Normal, &keys("w")),
            Resolution::Role(Role::Motion(Motion::WordForward))
        );
        assert_eq!(
            table.resolve(Scope::Normal, &keys("g")),
            Resolution::Pending
        );
        assert_eq!(
            table.resolve(Scope::Normal, &keys("gg")),
            Resolution::Role(Role::Goto(Goto::First))
        );
        assert_eq!(
            table.resolve(Scope::Normal, &keys("z")),
            Resolution::Unbound
        );
        // A scope is part of the key: `w` is not bound in operator-pending here.
        assert_eq!(
            table.resolve(Scope::OperatorPending, &keys("w")),
            Resolution::Unbound
        );
    }

    #[test]
    fn the_layer_shadows_the_seed_except_inside_an_operator() {
        let mut layer = Table::new();
        layer.bind(Scope::Normal, "w", Role::Repeat);
        layer.bind(Scope::OperatorPending, "w", Role::Repeat);
        let mut seed = table();
        seed.bind(
            Scope::OperatorPending,
            "w",
            Role::Motion(Motion::WordForward),
        );

        let mut layered = Layered::new(&mut layer, &mut seed);
        assert_eq!(
            layered.resolve(Scope::Normal, &keys("w")),
            Resolution::Role(Role::Repeat),
            "a rebind wins in normal mode"
        );
        assert_eq!(
            layered.resolve(Scope::OperatorPending, &keys("w")),
            Resolution::Role(Role::Motion(Motion::WordForward)),
            "the operand of an operator belongs to the grammar"
        );
    }

    #[test]
    fn a_pending_layer_sequence_is_not_overtaken_by_the_seed() {
        let mut layer = Table::new();
        layer.bind(Scope::Normal, "]r", Role::Repeat);
        let mut seed = Table::new();
        seed.bind(Scope::Normal, "]", Role::Motion(Motion::ParagraphForward));

        let mut layered = Layered::new(&mut layer, &mut seed);
        assert_eq!(
            layered.resolve(Scope::Normal, &keys("]")),
            Resolution::Pending,
            "the seed cannot know the layer is mid-sequence"
        );
    }

    #[test]
    fn bound_lists_a_scope_for_which_key() {
        let table = table();
        let listed = table.bound(Scope::Normal);
        assert_eq!(listed.len(), 3);
        assert!(table.bound(Scope::Insert).is_empty());
    }
}
