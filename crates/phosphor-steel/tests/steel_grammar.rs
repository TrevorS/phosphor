//! `tree-sitter-scheme` against the Steel we actually ship — `T037`.
//!
//! `runtime/languages/steel.scm` declares `steel` first class on the grammar
//! `scheme`, and `docs/IMPLEMENTATION-PLAN.md` attached a condition to that
//! declaration before it was written: *"verify it parses real `runtime/*.scm`
//! before committing to it. Steel is a Scheme dialect, not Scheme."*
//!
//! `T083` did half of it — one hand-written fixture,
//! `crates/phosphor-buffer/tests/fixtures/steel.scm`, parsed clean and two gaps
//! recorded (`#u8(…)`, `#%`-prefixed identifiers). That fixture is a file
//! written to be parsed. This is the other half: **every `.scm` this repository
//! ships**, which is the editor's own source and the thing a user opens on
//! their first day.
//!
//! The bar is `T083`'s and not a softer one — **no `ERROR` and no `MISSING`
//! node anywhere in the tree**, because tree-sitter recovers from anything and a
//! returned tree proves nothing. A grammar that produces an `ERROR` node over
//! `keymaps.scm` is a grammar that would highlight the editor's own keymap as
//! broken, and it would be better to declare `steel` second tier and say so.
//!
//! Owned by `spine`.

// The defect table is the deliverable when this fails: a `file:line:column` per
// bad node, which is what makes the failure actionable rather than a boolean.
#![allow(clippy::print_stdout)]

use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser};

/// The `runtime/` directory as shipped.
fn runtime_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// Every `.scm` under `runtime/`, one level deep, sorted.
///
/// The same walk `shipped_runtime.rs` does, and duplicated rather than shared
/// because the two files answer to different crates' worth of dependency:
/// pulling this into a common module would put `tree_sitter` in
/// `shipped_runtime`'s build for nothing.
fn shipped() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![runtime_dir()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory)
            .expect("runtime/ is part of the repo")
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.is_dir() && directory == runtime_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|ext| ext == "scm") {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(
        files.len() >= 12,
        "runtime/ has almost no .scm files — wrong path?"
    );
    files
}

/// Every `ERROR` / `MISSING` node in a tree, as `line:column kind "text"`.
///
/// Lifted from `crates/phosphor-buffer/tests/grammar_abi.rs` so the two report
/// the same way; a defect line that reads differently in two places is one
/// somebody has to translate.
fn defects(root: Node<'_>, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            let at = node.start_position();
            let text = node.utf8_text(source.as_bytes()).unwrap_or("<non-utf8>");
            let snippet: String = text.chars().take(48).collect();
            out.push(format!(
                "{}:{} {} {snippet:?}",
                at.row + 1,
                at.column + 1,
                if node.is_missing() {
                    "MISSING"
                } else {
                    "ERROR"
                },
            ));
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    out.sort();
    out
}

/// **The declaration's own condition, checked.**
///
/// Prints a row per file so a pass is legible and a failure names the form.
#[test]
fn the_scheme_grammar_parses_every_steel_file_we_ship() {
    let language: tree_sitter::Language = tree_sitter_scheme::LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("tree-sitter-scheme 0.24.7 loads against the 0.26 runtime (T083)");

    let mut failures = Vec::new();
    println!(
        "\ntree-sitter-scheme {} vs runtime/",
        language.abi_version()
    );
    for path in shipped() {
        let source = std::fs::read_to_string(&path).expect("a readable .scm file");
        if source.trim().is_empty() {
            // `persisted.scm` ships empty on purpose — it is what the REPL
            // appends to. An empty file has no forms to get wrong.
            continue;
        }
        let tree = parser
            .parse(&source, None)
            .expect("the parser returns a tree");
        let defects = defects(tree.root_node(), &source);
        let name = path
            .strip_prefix(runtime_dir())
            .unwrap_or(&path)
            .display()
            .to_string();
        println!(
            "  {:<28} {:>5} bytes  {}",
            name,
            source.len(),
            if defects.is_empty() {
                "clean".to_owned()
            } else {
                format!("{} defect(s)", defects.len())
            }
        );
        for defect in &defects {
            println!("      {name}:{defect}");
            failures.push(format!("{name}:{defect}"));
        }
    }

    assert!(
        failures.is_empty(),
        "tree-sitter-scheme cannot read the Steel we ship:\n  {}\n\n\
         `runtime/languages/steel.scm` declares this grammar and the plan made \
         that conditional on exactly this check. Either the form is one the \
         grammar cannot have (record it as a known gap beside T083's two), or \
         steel is second tier and the declaration should say so.",
        failures.join("\n  ")
    );
}

/// The two gaps `T083` recorded are still gaps.
///
/// `runtime/languages/steel.scm` tells the reader that two pieces of Steel's
/// reader syntax do not parse, and a note like that is a claim with a shelf
/// life: if the grammar moves, the note becomes a lie in a file people read to
/// find out what is true. So the note has a test. The test above is what
/// catches either construct actually *appearing* in `runtime/` — it would be an
/// `ERROR` node — so this one only has to hold the claim itself.
#[test]
fn steels_reader_extensions_are_still_the_two_gaps_t083_found() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_scheme::LANGUAGE.into())
        .expect("the grammar loads");

    for (source, what) in [
        ("(define bytes #u8(1 2 3))", "the R7RS bytevector spelling"),
        ("(define x #%unbox)", "a #%-prefixed compiler internal"),
    ] {
        let tree = parser.parse(source, None).expect("a tree");
        assert!(
            !defects(tree.root_node(), source).is_empty(),
            "{what} now parses — tree-sitter-scheme has moved, and \
             runtime/languages/steel.scm's note about it is stale"
        );
    }
}
