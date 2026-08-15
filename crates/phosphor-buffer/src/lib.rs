//! The buffer engine: rope, tree-sitter parse state, the undo model, and the
//! language-server client.
//!
//! Owns the undo log outright — the vendored editor's `History` is opaque, but its
//! `Edit`/`EditBatch` primitives are public and replayable, so we keep our own
//! (Q2, SPIKES.md T008). `phosphor-core` persists what this crate records.
//!
//! Owns the language-server client for the same reason ([`lsp`], `T036`): a
//! server's positions are UTF-16 code units into a line and the buffer is the
//! only thing that knows what that line contains, so the conversion cannot live
//! anywhere else without shipping the text there too.
//!
//! Owns the answer to *"which grammars does this binary have"* ([`grammar`],
//! `T037`) because this is the crate whose manifest selects them: the tier a
//! `define-language` declaration lands in is the intersection of what it names
//! with what was compiled in, and only the crate holding the feature line can
//! say what that is.
//!
//! Owns the delimited-text parser ([`csv`], `T082`) because CSV is the one
//! first-class language with **no** grammar: what would be a tree-sitter parse
//! for the other eleven is a hand-written state machine here, and its output is
//! a column model rather than a tree.
//!
//! Owned by `surface`.

pub mod csv;
pub mod grammar;
pub mod lsp;
pub mod undo;
