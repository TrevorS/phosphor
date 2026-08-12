//! ViewModels — read-only projections of the store that widgets render.
//!
//! Public to `phosphor-ui`. Every field here is derived; nothing in this module
//! offers a way to write back (invariant: the UI reads, Actions mutate).
//!
//! `T007` gives this module just enough type surface for the boundary to be real.
//! The concrete ViewModels land with the surfaces that need them.

/// Marker for a read-only projection of the store.
///
/// Deliberately method-free. It names the *direction* — a widget receives a
/// `&impl ViewModel` and has no way to write through it — without pre-empting
/// what any individual ViewModel contains. The store re-derives these; the UI
/// never constructs one from a mutation.
pub trait ViewModel: core::fmt::Debug {}
