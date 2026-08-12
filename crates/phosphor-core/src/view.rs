//! The view tree — the contract between Steel composition and Rust primitives.
//!
//! Plain data: which primitives, laid out how, with what props. No ratatui types
//! and no Steel types, so neither side owns the protocol (Q12). `phosphor-steel`
//! produces a tree; `phosphor-ui` interprets it into ratatui calls.
//!
//! Only `spine` writes this module. The node kinds land in `T078`.
//!
//! The "no Steel and no ratatui dependency" half of Q12 is not a comment: this
//! crate's `[dependencies]` table is empty, and `scripts/lint-no-store-mutation.sh`
//! fails CI if either ever appears there.

/// One frame's declarative description of the screen.
///
/// `T078` fills in the node kinds (the primitive set of [Q12], plus the one
/// `spans` escape hatch). Until then this is an opaque root: enough for
/// `phosphor-ui` to name the type it consumes, and enough for the structural
/// lint to have a legitimate import to contrast the store's against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Tree {}
