//! The Steel VM host: the runtime, the Action/query bindings, and the REPL.
//!
//! This crate is the Steel door of the three (invariant 2) and produces the view
//! tree that `phosphor-ui` interprets (Q12). It never hands Steel a ratatui
//! `Buffer` — a GC'd scheme with a `&mut Buffer` is the one thing that can tear a
//! frame — so the dependency on `phosphor-core` is the whole surface area.
//!
//! Owned by `spine`.
