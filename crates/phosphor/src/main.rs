//! The phosphor binary: terminal setup, the event loop, input routing and panes.
//!
//! This is the app layer — the one place `ratatui` and `crossterm` are allowed.
//! It owns the frame: Steel decides what is on screen, this decides when pixels
//! land (Q12). Input is decoded here into Actions; nothing else emits them.
//!
//! Owned by `spine`. The loop lands in Window C; there is nothing to run yet.

fn main() {}
