//! The rendering primitives — `BufferView`, `Float`, `Picker`, `DiffBody`,
//! `TranscriptPane`, `GutterBar`, `VirtualText`. Parameterised here, composed in
//! Steel (Q12): does it produce pixels? Rust. Does it decide which pixels? Steel.
//!
//! Two constraints this crate is built to make mechanical:
//!   * `ratatui-core` only, never `ratatui` (T002).
//!   * `phosphor_core::vm` and `::view` only, never `::store` (T007). Widgets read
//!     ViewModels; they never mutate.
//!
//! Every colour comes from `&Theme` — no literal `Color::Rgb` in this crate (T006).
//!
//! `lib.rs` is the crate's module index and has no single owner in TEAM.md's
//! per-file split — each widget's owner adds their own `pub mod` line and
//! nothing else. Keep it to that.

pub mod buffer_view;
pub mod csv;
pub mod diagnostics;
pub mod float;
pub mod frame;
pub mod gutter;
pub mod interpret;
pub mod key_hints;
pub mod picker;
pub mod soft_wrap;
pub mod status_line;
pub mod tab_bar;
pub mod theme;
pub mod tints;
pub mod unknown_key;
pub mod virtual_text;
