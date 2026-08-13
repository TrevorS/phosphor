//! The semantic store, the vocabulary that reads and writes it, the ViewModels
//! derived from it, and the declarative view tree the UI interprets.
//!
//! The module split is load-bearing, not cosmetic. `vm`, `view` and `request`
//! are the crate's public face to `phosphor-ui`; `store` is not, and `T007`
//! turns that into a lint rather than a convention (Q12). `action` is not
//! either, and `scripts/lint-no-action-in-ui.sh` (`T019`) turns *that* into a
//! lint: a widget that can construct a mutation is one refactor away from
//! applying one.
//!
//! Ownership crosses module lines here (TEAM.md): `spine` owns `action`,
//! `query`, `registry`, `request`, `value` and `view`; `store` owns `store` and
//! its neighbours; `agent` owns `review`/`inbox`/`watch`. The split above is
//! what makes that safe.
//!
//! **No crate-root re-export of `store`.** A `pub use store::*;` here would put
//! the mutation API at `phosphor_core::Store`, reachable from `phosphor-ui`
//! without the word `store` ever appearing — the split would still be there and
//! would mean nothing. `scripts/lint-no-store-mutation.sh` checks this file for
//! exactly that, and the `T019` lint checks the same hole for `action`. `vm` and
//! `view` may be re-exported freely; only those two are load-bearing here.
//!
//! # Reading order for the modules
//!
//! `T019` landed five of these at once, deliberately in one edit, so the rest of
//! Window C never has to touch this file. They read in this order:
//!
//! 1. [`value`] — the wire model every door converts through, and the `Wire`
//!    trait that makes schema generation derivable.
//! 2. [`request`] — the payload vocabulary: positions, spans, targets, modes.
//!    **Importable from `phosphor-ui`**, unlike `action`: a widget legitimately
//!    names a [`request::Position`] in the ViewModel it renders.
//! 3. [`action`] — the single mutation API. Start here if you are adding a
//!    capability; its module docs carry the shape and the rejected alternatives.
//! 4. [`query`] — the read side, and `Revision`, which is what makes `T079`'s
//!    frame cache possible.
//! 5. [`registry`] — the one table `T020` builds three doors from and `T024`
//!    enumerates.

pub mod action;
pub mod input;
pub mod journal;
pub mod query;
pub mod registry;
pub mod request;
pub mod store;
pub mod value;
pub mod view;
pub mod vm;
