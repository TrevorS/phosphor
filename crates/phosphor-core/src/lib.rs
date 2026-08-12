//! The semantic store, the ViewModels derived from it, and the declarative view
//! tree the UI interprets.
//!
//! The three-way module split is load-bearing, not cosmetic. `vm` and `view` are
//! the crate's public face to `phosphor-ui`; `store` is not, and `T007` turns that
//! into a lint rather than a convention (Q12).
//!
//! Ownership crosses module lines here (TEAM.md): `spine` owns `action` and `view`,
//! `store` owns `store` and its neighbours, `agent` owns `review`/`inbox`/`watch`.
//! The split above is what makes that safe.
//!
//! **No crate-root re-export of `store`.** A `pub use store::*;` here would put the
//! mutation API at `phosphor_core::Store`, reachable from `phosphor-ui` without the
//! word `store` ever appearing — the split would still be there and would mean
//! nothing. `scripts/lint-no-store-mutation.sh` checks this file for exactly that.
//! `vm` and `view` may be re-exported freely; only `store` is load-bearing here.

pub mod store;
pub mod view;
pub mod vm;
