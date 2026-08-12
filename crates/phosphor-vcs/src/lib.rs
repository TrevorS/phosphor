//! The VCS adapter — git via `gix`, with the jj question deferred to S7.
//!
//! VCS is the safety net that lets there be no review ceremony (invariant 5), so
//! this crate answers queries about what changed; it never gates an edit.
//!
//! Owned by `store`.
