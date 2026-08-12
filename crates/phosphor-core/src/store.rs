//! The semantic store — regions, seen-state, anchors, threads, watches, inbox,
//! review blocks. Every surface is a query over this (invariant 4), and this is
//! the only module that mutates.
//!
//! Not part of the crate's face to `phosphor-ui`: `T007` fails CI on a `store::`
//! import from that crate. Owned by `store` from `CP-2`; this placeholder exists
//! so the module boundary is real from the first commit.

/// The store handle. Mutation goes through `&mut Store`, and that is precisely
/// the API `phosphor-ui` must never hold.
///
/// Opaque on purpose: `T041` builds the real thing (regions, anchors, seen-state)
/// and `T019`'s `Action` is the vocabulary of what may be applied to it. This type
/// exists at `T007` only so the forbidden import has something real to name — a
/// lint that guards an empty module proves nothing.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Store {}
