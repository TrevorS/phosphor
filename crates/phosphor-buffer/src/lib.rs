//! The buffer engine: rope, tree-sitter parse state, and the undo model.
//!
//! Owns the undo log outright — the vendored editor's `History` is opaque, but its
//! `Edit`/`EditBatch` primitives are public and replayable, so we keep our own
//! (Q2, SPIKES.md T008). `phosphor-core` persists what this crate records.
//!
//! Owned by `surface`.

pub mod undo;
