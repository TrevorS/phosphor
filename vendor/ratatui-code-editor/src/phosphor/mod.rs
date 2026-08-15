//! PHOSPHOR PATCH 5 — phosphor's additions to this fork.
//!
//! Everything upstream does not have lives under this module, so `just
//! vendor-diff` shows one new directory plus a handful of seam lines in
//! upstream files, rather than a rewrite spread through them. See `VENDOR.md`.
//!
//! **Not upstreamable as a whole.** Individual pieces may be; each one says so
//! in its own `VENDOR.md` section.

pub mod cell_style;
// PHOSPHOR PATCH 6 — the wrap engine behind `VisualRow::Wrapped`.
pub(crate) mod soft_wrap;
// PHOSPHOR PATCH 8 — the placement rule behind `VisualRow::Virtual`.
pub mod virtual_text;
// PHOSPHOR PATCH 11 — the one place a tab's column-dependent width is computed.
pub mod tabs;
