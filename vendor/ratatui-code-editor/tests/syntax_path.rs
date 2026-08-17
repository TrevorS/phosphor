//! PHOSPHOR PATCH 12 — `Code::syntax_path`, the named-node chain `T042`'s
//! anchors fingerprint a location with. See `VENDOR.md`.
//!
//! These run inside the fork because `just test` cannot see them: the fork is
//! `[workspace] exclude`d, so `cargo nextest run --workspace` compiles it as a
//! dependency and never builds its tests. `scripts/lint-vendor-tests.sh` is
//! what runs this file, and it requires the binary by name so deleting or
//! emptying it fails rather than silently passing.

use ratatui_code_editor::code::Code;

/// `[(kind, name), …]`, which reads better in an assertion than the struct.
fn path(code: &Code, byte: usize) -> Vec<(String, String)> {
    code.syntax_path(byte)
        .into_iter()
        .map(|step| (step.kind, step.name))
        .collect()
}

const RUST: &str = "\
impl Backoff {
    fn retry(&self) -> u32 {
        let attempts = 3;
        attempts
    }

    fn reset(&mut self) {
        self.n = 0;
    }
}
";

#[test]
fn the_chain_names_the_construct_a_person_would_say_out_loud() {
    let code = Code::new(RUST, "rust", None).unwrap();
    let byte = RUST.find("let attempts").expect("the body line is in the fixture");

    assert_eq!(
        path(&code, byte),
        vec![
            ("impl_item".to_owned(), "Backoff".to_owned()),
            ("function_item".to_owned(), "retry".to_owned()),
        ],
        "the path is the named ancestors that carry a name, outermost first",
    );
}

#[test]
fn two_functions_in_one_impl_have_different_paths() {
    let code = Code::new(RUST, "rust", None).unwrap();
    let first = RUST.find("let attempts").expect("in the fixture");
    let second = RUST.find("self.n = 0").expect("in the fixture");

    assert_ne!(
        path(&code, first),
        path(&code, second),
        "the tier would be useless if every line in a type resolved alike",
    );
}

/// The property the whole tier exists for: the construct moves, the path does
/// not. Reordering the two functions shifts every byte offset and every child
/// index, which is exactly what a child-index fingerprint would follow to the
/// wrong node.
#[test]
fn the_path_survives_the_rewrite_that_moves_the_code() {
    let before = Code::new(RUST, "rust", None).unwrap();
    let was = path(
        &before,
        RUST.find("let attempts").expect("in the fixture"),
    );

    let reordered = "\
impl Backoff {
    fn reset(&mut self) {
        self.n = 0;
    }

    // a comment that did not exist before, to move the bytes again
    fn retry(&self) -> u32 {
        let attempts = 3;
        attempts
    }
}
";
    let after = Code::new(reordered, "rust", None).unwrap();
    let now = path(
        &after,
        reordered.find("let attempts").expect("in the fixture"),
    );

    assert_eq!(was, now, "the anchor's fingerprint is unchanged by the move");
}

/// Renaming is deliberately *not* survived — see the module header. An anchor
/// that followed a rename would claim someone had seen code they had not.
#[test]
fn a_rename_is_a_different_construct_and_the_path_says_so() {
    let before = Code::new(RUST, "rust", None).unwrap();
    let was = path(&before, RUST.find("let attempts").expect("in the fixture"));

    let renamed = RUST.replace("fn retry", "fn retry_with_backoff");
    let after = Code::new(&renamed, "rust", None).unwrap();
    let now = path(
        &after,
        renamed.find("let attempts").expect("in the fixture"),
    );

    assert_ne!(was, now, "a rename changes the fingerprint, by design");
}

#[test]
fn a_file_with_no_grammar_answers_an_empty_path() {
    let code = Code::new("some prose, in no language at all\n", "no-such-language", None)
        .expect("an unknown language renders unhighlighted rather than failing");

    assert!(
        code.syntax_path(4).is_empty(),
        "an empty path is how T043's fallback tier is signalled",
    );
}

#[test]
fn a_byte_past_the_end_is_empty_rather_than_a_panic() {
    let code = Code::new(RUST, "rust", None).unwrap();

    assert!(code.syntax_path(RUST.len() + 4_096).is_empty());
}

/// Code genuinely at the top level — not inside any identified construct —
/// resolves to an empty path and therefore to `T043`'s tier. Byte 0 of `RUST`
/// is *not* this case and the first draft of this test wrongly assumed it was:
/// `impl_item` starts at byte 0, so byte 0 is inside it.
#[test]
fn a_byte_outside_every_named_construct_has_an_empty_path() {
    let source = "\
// a bare comment, owned by no construct
const LIMIT: u32 = 3;

impl Backoff {
    fn retry(&self) {}
}
";
    let code = Code::new(source, "rust", None).unwrap();

    assert!(
        code.syntax_path(4).is_empty(),
        "inside the leading comment, which no identified construct covers",
    );
    assert_eq!(
        path(&code, source.find("fn retry").expect("in the fixture")),
        vec![
            ("impl_item".to_owned(), "Backoff".to_owned()),
            ("function_item".to_owned(), "retry".to_owned()),
        ],
        "and the same file still resolves inside the impl",
    );
}

/// `impl_item` carries no `name` field — this test is why `IDENTIFYING` is
/// three fields and not one. Two trait impls of the same type must not share a
/// path, or an anchor in one would resolve into the other.
#[test]
fn two_trait_impls_of_one_type_do_not_collide() {
    let source = "\
impl Display for Backoff {
    fn fmt(&self) -> u32 {
        let shown = 1;
        shown
    }
}

impl Debug for Backoff {
    fn fmt(&self) -> u32 {
        let dumped = 2;
        dumped
    }
}
";
    let code = Code::new(source, "rust", None).unwrap();
    let shown = path(&code, source.find("let shown").expect("in the fixture"));
    let dumped = path(&code, source.find("let dumped").expect("in the fixture"));

    assert_ne!(
        shown, dumped,
        "the trait disambiguates; `type` alone would have collapsed these",
    );
    assert_eq!(shown[0].1, "Display Backoff");
    assert_eq!(dumped[0].1, "Debug Backoff");
}

/// Python, so the tier is not accidentally a Rust-shaped feature — the grammar
/// names the field `name` on its own node kinds and the walk is grammar-blind.
#[test]
fn the_walk_is_grammar_blind_and_python_resolves_too() {
    let source = "\
class Backoff:
    def retry(self):
        attempts = 3
        return attempts
";
    let code = Code::new(source, "python", None).unwrap();
    let byte = source.find("attempts = 3").expect("in the fixture");

    assert_eq!(
        path(&code, byte),
        vec![
            ("class_definition".to_owned(), "Backoff".to_owned()),
            ("function_definition".to_owned(), "retry".to_owned()),
        ],
    );
}
