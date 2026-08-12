//! ViewModels — read-only projections of the store that widgets render.
//!
//! Public to `phosphor-ui`. Every field here is derived; nothing in this module
//! offers a way to write back (invariant: the UI reads, Actions mutate).
