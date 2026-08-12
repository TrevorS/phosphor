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
