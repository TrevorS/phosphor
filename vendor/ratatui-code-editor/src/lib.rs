pub mod actions;
pub mod click;
pub mod code;
mod diff;
#[cfg(feature = "bench-internals")]
pub use diff::{bench_build_diff_rows, bench_build_diff_rows_fast};
pub mod editor;
#[cfg(feature = "crossterm")]
pub mod editor_crossterm;
pub mod history;
// PHOSPHOR PATCH 5 — everything phosphor adds lives under here. See VENDOR.md.
pub mod phosphor;
pub mod render;
pub mod selection;
pub mod theme;
pub mod types;
pub mod utils;
mod view;
