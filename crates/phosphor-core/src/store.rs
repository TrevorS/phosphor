//! The semantic store — regions, seen-state, anchors, threads, watches, inbox,
//! review blocks. Every surface is a query over this (invariant 4), and this is
//! the only module that mutates.
//!
//! Not part of the crate's face to `phosphor-ui`: `T007` fails CI on a `store::`
//! import from that crate. Owned by `store` from `CP-2`; this placeholder exists
//! so the module boundary is real from the first commit.
