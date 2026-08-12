//! The view tree — the contract between Steel composition and Rust primitives.
//!
//! Plain data: which primitives, laid out how, with what props. No ratatui types
//! and no Steel types, so neither side owns the protocol (Q12). `phosphor-steel`
//! produces a tree; `phosphor-ui` interprets it into ratatui calls.
//!
//! Only `spine` writes this module. The node kinds land in `T078`.
