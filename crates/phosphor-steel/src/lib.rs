//! The Steel VM host: the runtime, the Action/query bindings, and the REPL.
//!
//! This crate is the Steel door of the three (invariant 2) and produces the view
//! tree that `phosphor-ui` interprets (Q12). It never hands Steel a ratatui
//! `Buffer` — a GC'd scheme with a `&mut Buffer` is the one thing that can tear a
//! frame — so the dependency on `phosphor-core` is the whole surface area.
//!
//! Owned by `spine`.
//!
//! # The barrier, in modules
//!
//! Read them in this order; each is the previous one's consequence.
//!
//! 1. [`convert`] — [`SteelVal`](steel::SteelVal) narrowed onto
//!    `phosphor_core::value::Value`. A closure cannot cross, which is what
//!    keeps a payload plain data and the MCP schema derivable.
//! 2. [`host`] — the two verbs Steel is allowed: apply an Action, answer a
//!    query. *"Steel can only emit Actions and read ViewModels."*
//! 3. [`registry`] — every capability installed into the VM, walked out of
//!    `phosphor_core::registry`. No table of our own.
//! 4. [`view`] — the other half of what Steel may say: one constructor per node
//!    kind, walked out of `phosphor_core::view`'s declared union. The vocabulary
//!    is what Steel may *do*; this is what it may *draw with* (Q12).
//! 5. [`source`] — a `.scm` file split into top-level forms, which is the
//!    granularity the boot fails at.
//! 6. [`boot`] — the load order and the findings. Total: it returns a report,
//!    never an error.
//! 7. [`float`] — those findings as a view-tree float. Composes; does not draw.
//! 8. [`runtime`] — all of the above as one object, and the entry point the
//!    binary takes.
//! 9. [`answer`] — one [`Outcome`](phosphor_core::action::Outcome), one line,
//!    for both front-ends. `6b`'s `⇒` and `--eval`'s stdout are the same text
//!    because they are the same function.
//! 10. [`keymap`] — the live table, *asked* rather than cached. `T022`'s
//!     liveness claim is that there is no copy of it on this side.
//! 11. [`status`] — `T025`'s statusline: a ViewModel out, a view tree back, and
//!     every decision about which segments and in what order in
//!     `runtime/statusline.scm`.
//! 12. [`repl`] — `T022`'s session over all of it, composed as `6b`.
//!
//! # `T021`'s promise
//!
//! *A broken `init.scm` boots the editor anyway, with the error in a float.*
//! [`runtime::Runtime::boot`] returns a `Runtime` and not a `Result`, so the
//! promise is in the signature rather than in a comment, and
//! [`boot::BootReport`] is what a caller inspects to find out what it cost.

pub mod answer;
pub mod boot;
pub mod convert;
pub mod float;
pub mod host;
pub mod keymap;
pub mod picker;
pub mod registry;
pub mod repl;
pub mod runtime;
pub mod source;
pub mod status;
pub mod view;
