//! `phosphor_buffer::grammar::BUNDLED`, recomputed against the fork (`T037`).
//!
//! The constant is what the host hands
//! [`phosphor_core::language::Languages::new`], and every `(languages)` answer
//! is derived from it: a name in it is `first-class`, a name outside it is
//! `second-tier` however the declaration is spelled. So a list that drifts from
//! the build does not fail anywhere — it makes the editor *claim* node
//! anchoring and structural text objects for a buffer that renders
//! unhighlighted, which is the failure `Languages::tier` was rewritten to stop
//! and which a list nobody recomputes reintroduces one crate over.
//!
//! **[`CANDIDATES`] is a superset, and that is what makes this bite in both
//! directions.** It names every arm `Code::get_language` has, so turning a
//! grammar feature *on* fails here (an unlisted name loads) and turning one
//! *off* fails here too (a listed name does not). A test that only checked the
//! names already in `BUNDLED` would pass a build that had gained five grammars.

use phosphor_buffer::grammar::{BUNDLED, loads};

/// Every language `vendor/ratatui-code-editor`'s `Code::get_language` has an
/// arm for — the fork's whole grammar table, features or no features.
///
/// Read off that `match` and nothing else. It is allowed to be stale in exactly
/// one way — a grammar the fork *adds* — and that is the way that costs
/// nothing: an arm nobody names here is a grammar no declaration can claim
/// until somebody adds it below.
const CANDIDATES: [&str; 17] = [
    "rust",
    "javascript",
    "typescript",
    "python",
    "go",
    "java",
    "c_sharp",
    "c",
    "cpp",
    "html",
    "css",
    "yaml",
    "json",
    "toml",
    "shell",
    "markdown",
    "markdown-inline",
];

/// **The recomputation.** What the fork can load is what the constant says,
/// exactly — no more and no less.
///
/// `markdown-inline` is the one arm that loads and is deliberately not in
/// `BUNDLED`; it is markdown's own injection grammar rather than a language a
/// file is opened in, so it is subtracted here by name and the reason is in
/// `grammar.rs`.
#[test]
fn the_bundled_list_is_what_this_build_can_actually_load() {
    let mut loadable: Vec<&str> = CANDIDATES
        .into_iter()
        .filter(|name| loads(name))
        .filter(|name| *name != "markdown-inline")
        .collect();
    loadable.sort_unstable();
    let mut declared = BUNDLED.to_vec();
    declared.sort_unstable();
    assert_eq!(
        loadable, declared,
        "`grammar::BUNDLED` and the fork's `grammars-phosphor` feature disagree; \
         the manifest is the truth and the constant is the bug"
    );
}

/// The oracle answers `false` for a name with no arm, which is what makes the
/// test above a measurement rather than a tautology over a function that
/// always says yes.
///
/// `scheme` is not an arbitrary example: `runtime/languages/steel.scm` declares
/// it, and `steel` being second tier is a shipped consequence of this answer.
#[test]
fn a_grammar_the_fork_does_not_bundle_does_not_load() {
    assert!(
        !loads("scheme"),
        "steel's declaration names a grammar nobody bundles"
    );
    assert!(
        !loads("csv"),
        "T082: there is deliberately no tree-sitter-csv"
    );
    assert!(!loads("elixir"), "a name no arm mentions at all");
}
