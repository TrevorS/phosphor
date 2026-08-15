//! The declared languages — where a `define-language!` call comes to rest, and
//! the one locale hook the vocabulary carries (`T037`).
//!
//! # Why the table is here, and not beside the thing it configures
//!
//! `scripts/lint-the-steel-barrier.sh` holds `phosphor-steel` to
//! `phosphor-core` plus the VM. So the Steel door **cannot name**
//! `phosphor_buffer::lsp::ServerSpec` — the type a declaration exists to
//! produce — and a `define-language!` binding that returned one is not
//! expressible. The seam that is left is the only one that works: a declaration
//! crosses the barrier as **data** ([`LanguageSpec`], already a wire record),
//! the host records it in a [`Languages`], and
//! `phosphor_buffer::lsp::ServerSpec::from_language_spec` is the door it comes
//! back through on the far side. `phosphor-steel` and `phosphor-buffer` both
//! depend on this crate and neither depends on the other, so this module is the
//! only place both can see.
//!
//! That is also why nothing here mentions tree-sitter or LSP. A [`Languages`]
//! knows that `"rust"` claims `rs`, wants the grammar named `rust` and starts
//! `rust-analyzer`; it has no idea what any of those three are for. Loading the
//! grammar is `phosphor-buffer`'s and spawning the server is `phosphor-buffer`'s.
//!
//! # Why the table is told which grammars exist
//!
//! [`Tier`] used to be `spec.grammar.is_some()`, which made the tier a fact
//! about the *spelling* of a declaration rather than about the editor. It
//! answered `first-class` for `(define-language! "elixir" … "grammar"
//! "elixir" …)` — a name nothing in this build can load — and it already
//! misreported one of the shipped twelve: `runtime/languages/steel.scm` names
//! the grammar `scheme`, the vendored fork bundles no such grammar, and that
//! file's own header says a `.scm` buffer renders unhighlighted until an arm
//! lands. *"The tier is honest"* cannot survive a discriminator that never
//! looks at what the binary contains, so [`Languages::new`] takes the grammar
//! names the host can resolve and [`Languages::tier`] is the intersection.
//!
//! # Why it is not a Rust table
//!
//! `T036` shipped `lsp::blessed`, a `match` over nine languages, and recorded
//! that it was the *default* half of the promise rather than the whole of it.
//! The Component Breakdown states the whole: *"the bundled set is just the
//! `define-language` calls we wrote and stand behind."* So the twelve live in
//! `runtime/languages/*.scm` and arrive here the same way a thirteenth typed at
//! `:repl` does — through [`Languages::declare`], with no Rust in the path.
//! This module holds **no list of languages**; a default list here would make
//! the shipped twelve privileged, and privileged is exactly what they must not
//! be. `CP-4`'s manual half is adding one from the REPL, which is the same test
//! read from the user's end.
//!
//! Owned by `spine`.

use std::fmt;
use std::path::Path;

use crate::request::{LanguageId, LanguageSpec, Tier};
use crate::value::{Args, Value, Wire as _};

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// Every language that has been declared, in declaration order.
///
/// A `Vec` rather than a map, for [`Args`]' reasons and one of its own:
/// declaration order is what `runtime/init.scm`'s load order fixes, so it is
/// the order the `languages` query answers in and the order `:help` would read
/// in. Twelve-ish entries make the linear scan the cheaper structure anyway,
/// and the crate has no dependencies to hash with.
///
/// Redeclaring a language **replaces it in place** — the same rule
/// [`Args::set`] follows, and the same reason: a redeclaration is an edit, not
/// a second language, and a list that grew a duplicate would answer the query
/// twice for one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Languages {
    declared: Vec<(LanguageId, LanguageSpec)>,
    grammars: Vec<String>,
}

/// Why a declaration was refused (`T037`).
///
/// Two shapes, and both of them are declarations that *land* and then never
/// match a file — which is worse than a refusal, because the road up from
/// second tier is a road you walk once and the failure is silent at the far
/// end of it. `define-language!` is a capability, so the host turns one of
/// these into a `Refusal` and the REPL prints it under the form you just typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    /// The language name was empty or all whitespace. It would still win an
    /// extension, so a `.zz` file would open in a language with no name.
    Nameless,
    /// An extension carrying a `.`, which is how a user writes one and is the
    /// one spelling that can never match: [`Path::extension`] answers `rs`,
    /// never `.rs`, and never a two-part `tar.gz`.
    DottedExtension(String),
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nameless => write!(f, "a language needs a name"),
            Self::DottedExtension(extension) => write!(
                f,
                "extension `{extension}` has a dot in it; write it the way \
                 `Path::extension` answers, without one"
            ),
        }
    }
}

impl std::error::Error for Invalid {}

impl Languages {
    /// A table that can load the grammars `grammars` names, with nothing
    /// declared in it yet.
    ///
    /// Empty of languages rather than pre-seeded with the twelve, which is the
    /// whole point: see the module header.
    ///
    /// The argument is not a convenience. A declaration names its grammar as a
    /// *string* and nothing in this crate can load one — resolution is a
    /// `match` in `vendor/ratatui-code-editor`'s `Code::get_language`, as wide
    /// as the `grammar-*` features `phosphor-buffer` selects. Without being
    /// told, [`Languages::tier`] answered `first-class` for any name at all.
    ///
    /// There is deliberately no `Default` and no zero-argument constructor: a
    /// host that forgot to say what it can load would get a table that lies in
    /// the *other* direction — everything second tier while the buffer
    /// highlights it — and a compile error is the cheaper failure.
    pub fn new(grammars: impl IntoIterator<Item: Into<String>>) -> Self {
        Self {
            declared: Vec::new(),
            grammars: grammars.into_iter().map(Into::into).collect(),
        }
    }

    /// The grammar names this table was told the host can load.
    pub fn grammars(&self) -> impl Iterator<Item = &str> {
        self.grammars.iter().map(String::as_str)
    }

    /// Declares `language`, returning what it replaced.
    ///
    /// The one mutation, so a `define-language!` from `runtime/`, from `:repl`,
    /// from MCP and from the CLI are indistinguishable by the time they land —
    /// which is what makes "a thirteenth from the REPL" the same code path as
    /// the twelve rather than a special case.
    ///
    /// The two [`Invalid`] refusals are here rather than at the wire door
    /// because they are facts about *matching a file*, which is this table's
    /// job and not the record's: `LanguageSpec` is a legal record either way.
    ///
    /// An empty extension list is **not** refused. `runtime/languages/README.md`
    /// documents `'()` as claiming no file, which a language reached some other
    /// way may honestly want; a dotted extension has no reading under which it
    /// works.
    ///
    /// # Errors
    ///
    /// [`Invalid::Nameless`] and [`Invalid::DottedExtension`], neither of which
    /// mutates the table.
    pub fn declare(
        &mut self,
        language: LanguageId,
        spec: LanguageSpec,
    ) -> Result<Option<LanguageSpec>, Invalid> {
        if language.0.trim().is_empty() {
            return Err(Invalid::Nameless);
        }
        if let Some(dotted) = spec.extensions.iter().find(|e| e.contains('.')) {
            return Err(Invalid::DottedExtension(dotted.clone()));
        }
        match self.declared.iter_mut().find(|(name, _)| *name == language) {
            Some((_, existing)) => Ok(Some(core::mem::replace(existing, spec))),
            None => {
                self.declared.push((language, spec));
                Ok(None)
            }
        }
    }

    /// What was declared for `language`, or [`None`] if nothing was.
    #[must_use]
    pub fn get(&self, language: &LanguageId) -> Option<&LanguageSpec> {
        self.declared
            .iter()
            .find(|(name, _)| name == language)
            .map(|(_, spec)| spec)
    }

    /// The tier `language` is in.
    ///
    /// First class means **a grammar this host can load**, not a grammar
    /// somebody named: the two came apart the moment a declaration could be
    /// typed, and [`Languages::new`] carries the difference. So a thirteenth
    /// naming `elixir` is second tier until the binary can parse elixir, which
    /// is the honest answer and the one that makes the `languages` query worth
    /// rendering.
    ///
    /// Total, and an undeclared language is [`Tier::SecondTier`] rather than an
    /// error: *"everything else is second tier, and the tier is honest"* is a
    /// statement about every file you can open, not only the ones somebody
    /// named.
    #[must_use]
    pub fn tier(&self, language: &LanguageId) -> Tier {
        let Some(grammar) = self.get(language).and_then(|spec| spec.grammar.as_deref()) else {
            return Tier::SecondTier;
        };
        if self.grammars.iter().any(|known| known == grammar) {
            Tier::FirstClass
        } else {
            Tier::SecondTier
        }
    }

    /// The language claiming `extension` (given without the dot), most recent
    /// declaration first.
    ///
    /// Reverse order is the road up made to work: a declaration typed at
    /// `:repl` that claims `ts` takes it from the shipped one, because the
    /// alternative — first declaration wins — would make the bundled twelve
    /// unoverridable and the userspace road a dead end.
    ///
    /// ASCII-case-insensitive, because a `.RS` file is a Rust file and the case
    /// of a filename is the filesystem's business rather than the language's.
    #[must_use]
    pub fn by_extension(&self, extension: &str) -> Option<&LanguageId> {
        self.declared.iter().rev().find_map(|(name, spec)| {
            spec.extensions
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(extension))
                .then_some(name)
        })
    }

    /// The language of a path, by its extension.
    ///
    /// [`None`] means second tier, which is a normal answer and not a failure:
    /// a `README` with no extension opens, and the whole agent loop works in
    /// it.
    #[must_use]
    pub fn by_path(&self, path: &Path) -> Option<&LanguageId> {
        let extension = path.extension()?.to_str()?;
        self.by_extension(extension)
    }

    /// The line-comment prefix `toggle-comment` should use for `language`.
    ///
    /// [`None`] where the declaration named none — CSV and JSON have no comment
    /// syntax at all, so `gc` in one of them must do nothing rather than insert
    /// a prefix that corrupts the file.
    #[must_use]
    pub fn comment_prefix(&self, language: &LanguageId) -> Option<&str> {
        self.get(language)?.comment_prefix.as_deref()
    }

    /// Every declaration, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&LanguageId, &LanguageSpec)> {
        self.declared.iter().map(|(name, spec)| (name, spec))
    }

    /// How many languages have been declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.declared.len()
    }

    /// Whether nothing has been declared yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declared.is_empty()
    }

    /// The `languages` query's answer: one record per declaration, or just the
    /// one asked for.
    ///
    /// Built here rather than at the arm because the query's contract — *"the
    /// declared languages, and which tier each is"* — is about this table, and
    /// a host that assembled the records itself would be free to disagree with
    /// [`Languages::tier`] about what it holds.
    ///
    /// The record is the declaration's own fields **flattened**, plus
    /// `language` and `tier`. Flat because a composition reads it with one
    /// `hash-get` — `(hash-get lang "grammar")` rather than
    /// `(hash-get (hash-get lang "spec") "grammar")` — and the nesting would
    /// buy nothing, the two added names not colliding with any field
    /// [`LanguageSpec`] has.
    ///
    /// A language that was never declared answers with an empty list rather
    /// than an error: *"which tier is `elixir`"* has a true answer, and it is
    /// second tier.
    #[must_use]
    pub fn answer(&self, only: Option<&LanguageId>) -> Vec<Value> {
        self.declared
            .iter()
            .filter(|(name, _)| only.is_none_or(|wanted| wanted == name))
            .map(|(name, spec)| {
                let mut args = Args::new()
                    .with("language", name.to_value())
                    .with("tier", self.tier(name).to_value());
                if let Value::Record(fields) = spec.to_value() {
                    for (field, value) in fields.into_pairs() {
                        args.set(&field, value);
                    }
                }
                Value::Record(args)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The locale hook
// ---------------------------------------------------------------------------

/// Comments or uncomments a run of lines with `prefix` — what `toggle-comment`
/// does once it has asked [`Languages::comment_prefix`] which prefix that is.
///
/// A pure function over lines, in this crate, because it is the only part of
/// `gc` that is a decision: *where* the prefix goes, *when* it comes off, and
/// what happens to a blank line in the middle of the run. Applying the result
/// to a rope is the buffer's, and the buffer's half has no branches in it.
///
/// The three rules, each of which is a test below:
///
/// * **Uncomment only when every non-blank line is commented.** A mixed run
///   comments, which is vim's rule and the one that makes `gc` idempotent to
///   press twice on a block you were unsure about.
/// * **The prefix goes at the shallowest indent in the run**, not at column
///   zero and not at each line's own indent — so a commented block keeps its
///   shape and the prefixes line up, which is the reason to read one.
/// * **Blank lines are left exactly alone**, in both directions. A trailing
///   `//` on an empty line is noise the next `gc` would have to guess about.
///
/// Removal takes the prefix and **at most one** following space, the mirror of
/// what insertion adds. So `//x` uncomments to `x` and `//  x` uncomments to
/// ` x`: the second space was the author's.
///
/// An empty `prefix` returns the lines unchanged. It is not a legal comment
/// syntax, and the alternative is inserting nothing everywhere and then
/// believing the whole run is commented.
#[must_use]
pub fn toggle_comment<S: AsRef<str>>(lines: &[S], prefix: &str) -> Vec<String> {
    let all = || lines.iter().map(AsRef::as_ref);
    let content = || all().filter(|line| !line.trim().is_empty());
    let verbatim = || -> Vec<String> { all().map(str::to_owned).collect() };

    // No shallowest indent means no non-blank line, so neither direction has
    // anything to act on — the same nothing-to-do the empty prefix gets. Asking
    // for the indent here rather than in the branch that uses it is what keeps
    // "the run is all blank" from being a second, separately-checked state.
    let Some(indent) = content().map(indent_chars).min() else {
        return verbatim();
    };
    if prefix.is_empty() {
        return verbatim();
    }

    if content().all(|line| line.trim_start().starts_with(prefix)) {
        return all().map(|line| uncomment(line, prefix)).collect();
    }
    all().map(|line| comment(line, prefix, indent)).collect()
}

/// How many leading whitespace **characters** a line has.
///
/// Characters, not bytes, because the answer is compared *across* lines and
/// then used to cut *into* one of them. `str::trim_start` trims Unicode
/// `White_Space`, so a run holding one non-breaking-space-indented line puts
/// the run-wide minimum in the middle of a character on every other line, and
/// `str::split_at` on that byte offset panics — `gc` over a block pasted out of
/// a browser took the editor down.
fn indent_chars(line: &str) -> usize {
    line.chars().count() - line.trim_start().chars().count()
}

/// Inserts `prefix` and a space `indent` characters in, leaving blanks alone.
fn comment(line: &str, prefix: &str, indent: usize) -> String {
    if line.trim().is_empty() {
        return line.to_owned();
    }
    // `indent` is the *minimum* leading-whitespace length in the run, counted
    // by `indent_chars`, so it is at most this line's own — and the byte offset
    // of a character is a character boundary whatever that whitespace is.
    let at: usize = line.chars().take(indent).map(char::len_utf8).sum();
    let (before, after) = line.split_at(at);
    format!("{before}{prefix} {after}")
}

/// Removes the first `prefix`, and at most one space after it.
fn uncomment(line: &str, prefix: &str) -> String {
    if line.trim().is_empty() {
        return line.to_owned();
    }
    // Bytes are safe here where they are not in `comment`: this is *this*
    // line's own trimmed length, so the offset is where a subslice already
    // starts rather than a width borrowed from a neighbour.
    let indent = line.len() - line.trim_start().len();
    let (before, after) = line.split_at(indent);
    let Some(rest) = after.strip_prefix(prefix) else {
        return line.to_owned();
    };
    format!("{before}{}", rest.strip_prefix(' ').unwrap_or(rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table that can load every grammar these tests name, so a case about
    /// *declaration* is not accidentally a case about resolution.
    fn table() -> Languages {
        Languages::new(["rust", "css", "typescript", "json", "markdown"])
    }

    fn spec(extensions: &[&str], grammar: Option<&str>, command: &[&str]) -> LanguageSpec {
        LanguageSpec {
            extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
            grammar: grammar.map(str::to_owned),
            lsp_command: command.iter().map(|a| (*a).to_owned()).collect(),
            comment_prefix: Some("//".to_owned()),
        }
    }

    fn rust() -> LanguageSpec {
        spec(&["rs"], Some("rust"), &["rust-analyzer"])
    }

    fn id(name: &str) -> LanguageId {
        LanguageId(name.to_owned())
    }

    /// [`Languages::declare`], for the cases that are not about refusal.
    fn declare(languages: &mut Languages, name: &str, spec: LanguageSpec) -> Option<LanguageSpec> {
        languages
            .declare(id(name), spec)
            .expect("a legal declaration")
    }

    #[test]
    fn a_redeclaration_replaces_in_place() {
        let mut languages = table();
        declare(&mut languages, "rust", rust());
        declare(&mut languages, "css", spec(&["css"], Some("css"), &[]));
        let replaced = declare(
            &mut languages,
            "rust",
            spec(&["rs"], Some("rust"), &["ra-multiplex"]),
        );

        assert_eq!(
            replaced.expect("rust was declared").lsp_command,
            ["rust-analyzer"]
        );
        assert_eq!(
            languages.len(),
            2,
            "a redeclaration is an edit, not a second language"
        );
        let names: Vec<&str> = languages.iter().map(|(name, _)| name.0.as_str()).collect();
        assert_eq!(
            names,
            ["rust", "css"],
            "and it keeps its place in the order"
        );
    }

    #[test]
    fn a_later_declaration_takes_an_extension_from_an_earlier_one() {
        let mut languages = table();
        declare(
            &mut languages,
            "typescript",
            spec(&["ts"], Some("typescript"), &[]),
        );
        assert_eq!(languages.by_extension("ts"), Some(&id("typescript")));

        declare(
            &mut languages,
            "tree-sitter-query",
            spec(&["ts"], None, &[]),
        );
        assert_eq!(
            languages.by_extension("ts"),
            Some(&id("tree-sitter-query")),
            "the road up is a dead end if the shipped set cannot be overridden"
        );
    }

    #[test]
    fn an_extension_matches_whatever_case_the_filesystem_used() {
        let mut languages = table();
        declare(&mut languages, "rust", rust());
        assert_eq!(
            languages.by_path(Path::new("src/MAIN.RS")),
            Some(&id("rust"))
        );
        assert_eq!(languages.by_path(Path::new("README")), None);
    }

    /// A declaration nothing could ever act on is refused, not filed.
    ///
    /// Both shapes landed silently before `T037`'s review: a nameless language
    /// won an extension, and `".dt"` — the way a user writes an extension — was
    /// accepted and then never matched, because [`Path::extension`] has no dot
    /// in its answer.
    #[test]
    fn a_declaration_that_could_never_match_a_file_is_refused() {
        let mut languages = table();

        assert_eq!(
            languages.declare(id(""), spec(&["zz"], None, &[])),
            Err(Invalid::Nameless)
        );
        assert_eq!(
            languages.declare(id("   "), spec(&["zz"], None, &[])),
            Err(Invalid::Nameless)
        );
        assert_eq!(
            languages.declare(id("dotted"), spec(&[".dt"], None, &[])),
            Err(Invalid::DottedExtension(".dt".to_owned()))
        );
        assert_eq!(
            languages.declare(id("tarball"), spec(&["tar.gz"], None, &[])),
            Err(Invalid::DottedExtension("tar.gz".to_owned())),
            "`a.tar.gz` has extension `gz`; nothing ever answers `tar.gz`"
        );
        assert!(
            languages.is_empty(),
            "a refusal leaves the table where it found it"
        );
        assert_eq!(languages.by_path(Path::new("x.zz")), None);
        assert!(
            !Invalid::DottedExtension(".dt".to_owned())
                .to_string()
                .is_empty(),
            "and it says which extension, because the REPL prints this"
        );

        assert!(
            languages
                .declare(id("private"), spec(&[], None, &[]))
                .is_ok(),
            "claiming no file is documented as legal (runtime/languages/README.md)"
        );
    }

    /// First class is a grammar this build can load, not a grammar somebody
    /// spelled.
    ///
    /// `runtime/languages/steel.scm` is the live case: it names `scheme`, the
    /// vendored fork bundles no such grammar, and the tier said `first-class`
    /// anyway — for a buffer that renders unhighlighted.
    #[test]
    fn the_tier_is_the_grammars_this_build_has() {
        let mut languages = Languages::new(["rust"]);
        declare(&mut languages, "rust", rust());
        declare(&mut languages, "steel", spec(&["scm"], Some("scheme"), &[]));
        declare(&mut languages, "csv", spec(&["csv"], None, &[]));

        assert_eq!(languages.tier(&id("rust")), Tier::FirstClass);
        assert_eq!(
            languages.tier(&id("steel")),
            Tier::SecondTier,
            "a grammar nothing can load is not a grammar"
        );
        assert_eq!(languages.tier(&id("csv")), Tier::SecondTier);
        assert_eq!(languages.tier(&id("never-declared")), Tier::SecondTier);

        let mut with_scheme = Languages::new(["rust", "scheme"]);
        declare(
            &mut with_scheme,
            "steel",
            spec(&["scm"], Some("scheme"), &[]),
        );
        assert_eq!(
            with_scheme.tier(&id("steel")),
            Tier::FirstClass,
            "and the day the arm lands, the same declaration is first class"
        );
        assert_eq!(
            with_scheme.grammars().collect::<Vec<_>>(),
            ["rust", "scheme"]
        );
    }

    /// A language server is not what first class means — a grammar is.
    #[test]
    fn a_declaration_with_no_server_can_still_be_first_class() {
        let mut languages = Languages::new(["markdown"]);
        declare(
            &mut languages,
            "markdown",
            spec(&["md"], Some("markdown"), &[]),
        );
        assert_eq!(languages.tier(&id("markdown")), Tier::FirstClass);
    }

    #[test]
    fn the_query_answer_is_flat_and_carries_the_tier() {
        let mut languages = table();
        declare(&mut languages, "rust", rust());
        declare(&mut languages, "csv", spec(&["csv"], None, &[]));

        assert_eq!(languages.answer(None).len(), 2);
        let one = languages.answer(Some(&id("csv")));
        let [Value::Record(fields)] = one.as_slice() else {
            panic!("one language asked for, one record back: {one:?}");
        };
        assert_eq!(fields.get("language"), Some(&Value::Text("csv".to_owned())));
        assert_eq!(
            fields.get("tier"),
            Some(&Value::Text("second-tier".to_owned()))
        );
        assert_eq!(fields.get("grammar"), Some(&Value::Null));
        assert!(languages.answer(Some(&id("elixir"))).is_empty());
    }

    /// The `languages` query reads the tier off the same table the editor does.
    ///
    /// It used to build each row with a `Tier::of(spec)` of its own, which left
    /// the query free to disagree with [`Languages::tier`] about the row it was
    /// describing — and after the grammar table landed it would have, on every
    /// declaration naming a grammar this build cannot load.
    #[test]
    fn the_answers_tier_is_the_tables_tier() {
        let mut languages = Languages::new(["rust"]);
        declare(&mut languages, "rust", rust());
        declare(&mut languages, "elixir", spec(&["ex"], Some("elixir"), &[]));

        let rows = languages.answer(None);
        assert_eq!(rows.len(), 2);
        for row in rows {
            let Value::Record(fields) = row else {
                panic!("a row is a record");
            };
            let Some(Value::Text(name)) = fields.get("language") else {
                panic!("a row names its language");
            };
            assert_eq!(
                fields.get("tier"),
                Some(&languages.tier(&id(name)).to_value()),
                "{name}"
            );
        }
    }

    #[test]
    fn commenting_puts_the_prefix_at_the_shallowest_indent() {
        let lines = ["fn main() {", "    let x = 1;", "", "    dbg!(x);", "}"];
        assert_eq!(
            toggle_comment(&lines, "//"),
            [
                "// fn main() {",
                "//     let x = 1;",
                "",
                "//     dbg!(x);",
                "// }",
            ]
        );
    }

    #[test]
    fn a_run_that_is_wholly_commented_uncomments() {
        let lines = ["// a", "//   b", "", "//c"];
        assert_eq!(toggle_comment(&lines, "//"), ["a", "  b", "", "c"]);
    }

    #[test]
    fn a_mixed_run_comments_rather_than_uncommenting() {
        let lines = ["// a", "b"];
        assert_eq!(toggle_comment(&lines, "//"), ["// // a", "// b"]);
    }

    /// **`gc` over whitespace it did not expect does not take the editor down.**
    ///
    /// `str::trim_start` trims Unicode `White_Space`, so a run-wide minimum
    /// indent measured in *bytes* lands inside a non-breaking or ideographic
    /// space on a neighbouring line and `str::split_at` panics. Both spellings
    /// turn up in real files — a block pasted out of a browser, CJK source —
    /// and the arm `gc` lowers to calls this on buffer lines, so the panic is
    /// the editor.
    #[test]
    fn an_indent_that_is_not_ascii_whitespace_neither_panics_nor_splits_a_character() {
        assert_eq!(
            toggle_comment(&["   x", "\u{a0}\u{a0}y"], "//"),
            ["  //  x", "\u{a0}\u{a0}// y"],
            "two characters in on both lines, whatever those characters cost"
        );
        assert_eq!(
            toggle_comment(&["  x", "\u{3000}y"], "//"),
            [" //  x", "\u{3000}// y"],
            "the shallowest indent here is one ideographic space"
        );

        let mixed = ["\t x", "\u{a0}  y"];
        assert_eq!(
            toggle_comment(&toggle_comment(&mixed, ";;"), ";;"),
            mixed,
            "and gc twice is still the identity"
        );
    }

    #[test]
    fn a_language_with_no_comment_syntax_leaves_the_file_alone() {
        let mut languages = table();
        declare(
            &mut languages,
            "json",
            LanguageSpec {
                comment_prefix: None,
                ..spec(&["json"], Some("json"), &[])
            },
        );
        assert_eq!(languages.comment_prefix(&id("json")), None);
        assert_eq!(toggle_comment(&["{}"], ""), ["{}"]);
    }
}
