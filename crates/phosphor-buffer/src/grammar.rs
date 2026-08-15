//! Which tree-sitter grammars *this* binary can load (`T037`).
//!
//! [`phosphor_core::language::Languages::new`] takes the grammar names the host
//! can resolve, and its own header says why it has to be told rather than
//! guess: without the list, [`Tier`] answered `first-class` for
//! `(define-language! "elixir" … "grammar" "elixir" …)` — a name nothing in
//! this build can parse — and already misreported one of the shipped twelve.
//! *"The tier is honest"* is a claim about what the binary contains, so
//! something has to contain the answer.
//!
//! This module is that something, and it lives here because
//! `phosphor-buffer` is the crate that selects the fork's `grammars-phosphor`
//! feature (`Cargo.toml`). `phosphor-core` cannot know — it has no
//! dependencies to load a grammar with — and the binary should not, because a
//! second list beside the manifest that selects the features is exactly the
//! *"counts nothing else recomputes"* failure class.
//!
//! # Why the list is a constant and [`loads`] is the check
//!
//! A Cargo feature of a *dependency* is not visible to `cfg!` here, so the set
//! cannot be written as conditional compilation. It could be computed at
//! startup by probing every name the fork has an arm for — but each probe is a
//! `Query::new` over that grammar's whole `highlights.scm`, which is work the
//! editor would do before its first frame for grammars nobody opened.
//!
//! So [`BUNDLED`] is written down and `tests/grammars.rs` **recomputes it**
//! against the fork, using [`loads`] as the oracle: every candidate the fork
//! has an arm for is probed, and the set that answers `true` must be exactly
//! [`BUNDLED`]. Turning a grammar feature off, or on, fails that test rather
//! than silently changing what `(languages)` claims.
//!
//! [`Tier`]: phosphor_core::request::Tier
//! Owned by `surface`.

use ratatui_code_editor::code::Code;

/// The grammar names this build can load — the fork's `grammars-phosphor`
/// feature, resolved.
///
/// Ten, and the twelve first-class languages are not ten by two deliberate
/// absences: `csv` names no grammar at all (`T082`) and `steel` names `scheme`,
/// which the fork does not bundle. Both are second tier and both say so in
/// their own `runtime/languages/*.scm`.
///
/// **`markdown-inline` is not here even though it loads.** The fork has an arm
/// for it because markdown injects into itself; it is not a language anything
/// declares, and listing it would let a declaration claim first-class status
/// for a grammar no file can be opened in.
pub const BUNDLED: [&str; 10] = [
    "css",
    "html",
    "javascript",
    "json",
    "markdown",
    "python",
    "rust",
    "toml",
    "typescript",
    "yaml",
];

/// Whether the fork can actually parse and highlight `name`.
///
/// `Code::is_highlight` is `query.is_some()`, and the fork sets the query
/// **only** inside the `if let Some(language) = Self::get_language(lang)` arm
/// of `Code::new` — so this answers the compiled-in grammar table itself
/// rather than a copy of it. An unknown name is not an error there (the buffer
/// renders unhighlighted), which is why the question has to be asked this way
/// and not by looking for a failure.
///
/// The empty document is deliberate: this asks whether the *grammar* resolved,
/// and a parse of nothing is the cheapest way to find out.
#[must_use]
pub fn loads(name: &str) -> bool {
    Code::new("", name, None).is_ok_and(|code| code.is_highlight())
}
