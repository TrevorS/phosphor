//! The twelve declarations in `runtime/languages/`, recomputed against the two
//! tables in `phosphor_buffer::lsp` (`T036`, `T037`).
//!
//! # Why this file exists
//!
//! `runtime/languages/README.md` says the server commands there are
//! *"transcribed from `phosphor_buffer::lsp::blessed`"*, and until this file
//! nothing recomputed that they still agreed — the exact *"counts nothing else
//! recomputes"* failure class `CLAUDE.md` lists, one directory over. A review of
//! `T037` found both tables had become duplicated data: `blessed`'s nine
//! commands and `FIRST_CLASS`'s twelve names have no non-test consumer left in
//! Rust (`ServerSpec::from_language_spec` reads only `root_markers` off
//! `blessed`), so a drift between the Scheme and the Rust would have shown up as
//! nothing at all until a user typed in a buffer.
//!
//! **This is a test rather than a `scripts/lint-*.sh` because the comparison
//! needs `blessed` itself**, and a shell script would have to re-transcribe the
//! table a third time to check the second one. It is in `phosphor-buffer`
//! because that is the crate that owns both tables; `phosphor-steel`, which
//! boots the declarations, cannot see this crate at all (the Steel barrier).
//!
//! # What it does not check
//!
//! That the declarations *load* — `crates/phosphor-steel/tests/shipped_languages.rs`
//! boots them. This file reads the `.scm` files as text, which is the only way
//! to compare them against Rust from a crate that cannot run Steel.

use std::collections::BTreeMap;
use std::path::PathBuf;

use phosphor_buffer::lsp::{FIRST_CLASS, blessed};
use phosphor_core::request::LanguageId;

/// `runtime/languages/`, relative to this crate.
///
/// The one path assumption here, and it is the same one `just` and CI make
/// about the workspace layout.
fn declarations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/languages")
        .canonicalize()
        .expect("runtime/languages is where the workspace keeps the declarations")
}

/// One `.scm` file's `(define-language! "name" (hash …))`, as far as this file
/// needs it: the name and the server command.
///
/// A reader rather than a parser. Steel is the thing that parses these — this
/// only has to find two keys, and a real reader here would be a second
/// implementation of the one under test.
#[derive(Debug, PartialEq, Eq)]
struct Declaration {
    name: String,
    /// The command and its arguments, in order. Empty is a language that
    /// declares no server, which is a first-class thing to be.
    lsp_command: Vec<String>,
}

/// Everything outside a `;;` comment line — the declarations are one form per
/// file and every file leads with a comment block explaining it.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with(';'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The strings inside `'( … )` starting at `from`, in order.
fn quoted_list(code: &str, from: usize) -> Vec<String> {
    let open = code[from..].find("'(").expect("a quoted list") + from + 2;
    let close = code[open..].find(')').expect("a closed list") + open;
    let mut out = Vec::new();
    let mut rest = &code[open..close];
    while let Some(start) = rest.find('"') {
        let end = rest[start + 1..].find('"').expect("a closed string") + start + 1;
        out.push(rest[start + 1..end].to_owned());
        rest = &rest[end + 1..];
    }
    out
}

fn read(source: &str) -> Declaration {
    let code = code_of(source);
    let head = code.find("(define-language! \"").expect("a declaration") + 19;
    let name_end = code[head..].find('"').expect("a closed name") + head;
    let key = code.find("\"lsp_command\"").expect("the lsp_command key");
    Declaration {
        name: code[head..name_end].to_owned(),
        lsp_command: quoted_list(&code, key),
    }
}

/// Every declaration the runtime ships, by language name.
fn shipped() -> BTreeMap<String, Declaration> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(declarations_dir()).expect("the declarations directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|extension| extension != "scm") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a declaration");
        let declaration = read(&source);
        assert_eq!(
            path.file_stem().expect("a stem").to_string_lossy(),
            declaration.name,
            "{} declares a different language than its filename",
            path.display()
        );
        out.insert(declaration.name.clone(), declaration);
    }
    out
}

/// **The twelve are the twelve**, and `FIRST_CLASS` is not a fourth private
/// copy of a list nothing checks. Add a `.scm` file without adding the name
/// here — or the reverse — and this is the test that says so.
#[test]
fn the_shipped_declarations_are_exactly_first_class() {
    let shipped = shipped();
    let mut declared: Vec<&str> = shipped.keys().map(String::as_str).collect();
    declared.sort_unstable();
    let mut expected: Vec<&str> = FIRST_CLASS.to_vec();
    expected.sort_unstable();
    assert_eq!(declared, expected, "runtime/languages vs lsp::FIRST_CLASS");
}

/// **Every declared command is the blessed one, spelled the same way.**
///
/// `runtime/languages/README.md`'s own word for these is *"transcribed"*, and a
/// transcription with nothing recomputing it is how `blessed` and the runtime
/// drift into disagreeing about which program serves Python — with no symptom
/// until someone opens a `.py` file.
#[test]
fn every_declared_server_is_the_blessed_one() {
    for (name, declaration) in shipped() {
        let expected: Vec<String> = blessed(&LanguageId(name.clone())).map_or_else(
            // A language phosphor blesses no server for declares none. Two of
            // the twelve are deliberate: `steel` has no server in existence and
            // `csv` gets `T082`'s hand-tuned surface.
            Vec::new,
            |spec| {
                let mut command = vec![spec.command];
                command.extend(spec.args);
                command
            },
        );
        assert_eq!(
            declaration.lsp_command, expected,
            "{name}.scm declares a different server than lsp::blessed"
        );
    }
}

/// The reader itself, on a declaration written the way the shipped ones are.
/// Without this the two tests above could both pass by finding nothing.
#[test]
fn the_reader_reads_a_declaration_the_way_steel_does() {
    let source = "\
;; toml — first class. `lsp_command` in a comment is not a declaration.
;; (define-language! \"decoy\" (hash \"lsp_command\" '(\"nope\")))

(define-language! \"toml\"
  (hash \"extensions\" '(\"toml\")
        \"grammar\" \"toml\"
        \"lsp_command\" '(\"taplo\" \"lsp\" \"stdio\")
        \"comment_prefix\" \"#\"))
";
    assert_eq!(
        read(source),
        Declaration {
            name: "toml".to_owned(),
            lsp_command: vec!["taplo".to_owned(), "lsp".to_owned(), "stdio".to_owned()],
        }
    );
    // The serverless shape, which is `'()` and not an omitted key.
    let serverless = "(define-language! \"steel\"\n  (hash \"lsp_command\" '()\n        \
                      \"comment_prefix\" \";;\"))\n";
    assert_eq!(read(serverless).lsp_command, Vec::<String>::new());
}
