//! The bundled set is the `define-language!` calls we shipped — `T037`.
//!
//! The Component Breakdown's claim is *"the bundled set is just the
//! `define-language` calls we wrote and stand behind"*, and the way that claim
//! fails is not a red test: it is a Rust table with a scheme wrapper over it,
//! green everywhere, indistinguishable from the real thing until somebody tries
//! to add a thirteenth language and finds they cannot.
//!
//! So this test never names a language of its own. It boots `runtime/` behind a
//! [`Recorder`] that does exactly what the host will do — apply
//! `Action::Runtime(DefineLanguage { … })` into a
//! [`phosphor_core::language::Languages`] — and then asks that table what it
//! holds. Every expectation below is a *consequence* of the twelve files: their
//! count, their order, their servers.
//!
//! **[`booted`] asserts the table is empty before the boot, and that assertion
//! is the whole file's load-bearing one.** Without it this suite could not tell
//! a declaration from a wrapper, which is the exact failure the paragraph above
//! claims it catches: a hand-written `impl Default for Languages` seeding the
//! twelve survived every test here, and so did deleting
//! `runtime/languages/rust.scm` on top of it, because `Languages::declare`
//! replaces in place and a pre-seeded twelve is indistinguishable from a
//! declared twelve in count, order and every field.
//! [`a_runtime_with_no_languages_directory_declares_nothing`] states the same
//! property from the other side.
//!
//! [`a_thirteenth_language_needs_no_rust`] is `CP-4`'s manual half, run from
//! Rust: a form typed at the REPL, and the whole of the road up — with
//! [`the_grammar_is_the_one_thing_a_thirteenth_still_needs_rust_for`] naming,
//! and failing on, the one half of it that is not there yet.
//!
//! Owned by `spine`.

use std::sync::{Arc, Mutex};

use phosphor_core::action::{Action, Outcome, Receipt, Refusal, Request, RuntimeAction};
use phosphor_core::language::Languages;
use phosphor_core::query::{Answer, Answers, Query, QueryError, Revision, UiQuery};
use phosphor_core::request::{LanguageId, Tier};
use phosphor_core::value::Value;
use phosphor_steel::host::Host;
use phosphor_steel::runtime::Runtime;

// ---------------------------------------------------------------------------
// The host the wiring agent will write, in miniature
// ---------------------------------------------------------------------------

/// The grammar names this binary can actually load.
///
/// `vendor/ratatui-code-editor`'s `grammars-phosphor` feature, which is what
/// `crates/phosphor-buffer/Cargo.toml` selects, and each one is an arm of that
/// fork's `Code::get_language`. Written out here because the Steel barrier
/// (`scripts/lint-the-steel-barrier.sh`) puts `phosphor-buffer` out of this
/// crate's reach; the host installs the same set, from the crate that owns it.
///
/// `scheme` is **not** in it, which is why `steel` is second tier below, and
/// neither is `csv` — the two of the twelve the fork's own manifest says it
/// does not bundle.
const GRAMMARS_THIS_BUILD_BUNDLES: [&str; 10] = [
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

/// A host that applies `define-language` and answers `languages`, and refuses
/// everything else.
///
/// **The exact two arms `crates/phosphor/src/main.rs` owes**, kept here so the
/// seam is exercised before the arm exists rather than after — the failure
/// `TEAM.md` records is a surface that landed complete, tested and uncomposed.
/// A `Mutex` because [`Host::apply`] takes `&self`: Steel calls a binding from
/// inside the running VM, so the interior mutability is the caller's
/// (`host.rs`).
///
/// There is no `Default`: [`Languages::new`] wants the grammar list, and a
/// recorder that could be built without one is a recorder whose tiers mean
/// nothing.
#[derive(Debug)]
struct Recorder {
    languages: Mutex<Languages>,
}

impl Recorder {
    /// A recorder holding an empty table that can load
    /// [`GRAMMARS_THIS_BUILD_BUNDLES`].
    fn new() -> Self {
        Self {
            languages: Mutex::new(Languages::new(GRAMMARS_THIS_BUILD_BUNDLES)),
        }
    }

    /// The table as it stands, cloned out from behind the lock.
    fn languages(&self) -> Languages {
        self.languages.lock().expect("no test panics here").clone()
    }
}

impl Answers for Recorder {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        match query {
            Query::Ui(UiQuery::Languages { language }) => Ok(Answer {
                value: Value::List(self.languages().answer(language.as_ref())),
                revision: Revision::INITIAL,
            }),
            other => Err(QueryError::NotYetImplemented {
                task: other.spec().since.task,
            }),
        }
    }
}

impl Host for Recorder {
    fn apply(&self, request: &Request) -> Outcome {
        match &request.action {
            Action::Runtime(RuntimeAction::DefineLanguage { language, spec }) => self
                .languages
                .lock()
                .expect("no test panics here")
                .declare(language.clone(), spec.clone())
                .map_or_else(
                    |invalid| {
                        Outcome::Refused(Refusal::Declined {
                            reason: invalid.to_string(),
                        })
                    },
                    |_| Outcome::Done(Receipt::ok("define-language")),
                ),
            other => Outcome::Refused(Refusal::NotYetImplemented {
                task: other.spec().since.task,
            }),
        }
    }
}

/// The `runtime/` directory as shipped, from this crate's manifest.
fn runtime_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// The shipped editor layer, booted clean, and the recorder that watched it.
///
/// The [`Runtime`] comes back with it because dropping it would drop the VM,
/// and a test that wants to type a thirteenth declaration into it needs it
/// alive.
///
/// The emptiness assertion is not a sanity check. It is the only thing in this
/// file that a Rust table cannot satisfy: every other expectation here — the
/// count, the order, the fields, the nine servers — is equally true of a
/// `Languages` that arrived pre-seeded, so without this line the suite passed
/// with `runtime/languages/rust.scm` deleted.
fn booted() -> (Runtime, Arc<Recorder>) {
    let recorder = Arc::new(Recorder::new());
    assert!(
        recorder.languages().is_empty(),
        "nothing but runtime/languages/ declares a language; this table starts empty"
    );
    let runtime = Runtime::boot(Some(&runtime_dir()), Arc::clone(&recorder) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    (runtime, recorder)
}

// ---------------------------------------------------------------------------
// The twelve
// ---------------------------------------------------------------------------

/// The Component Breakdown's own list, in its own order — *"TypeScript,
/// JavaScript, Rust, Python, Steel, Markdown, JSON, CSV, plus the config
/// dialects agents live in (TOML, YAML) and the web substrate (HTML, CSS)"*.
///
/// Written out here rather than read from anywhere, because this is the
/// checklist and the tree is the subject. `phosphor_buffer::lsp::FIRST_CLASS`
/// holds the same twelve in the same order for the same reason.
const FIRST_CLASS: [&str; 12] = [
    "typescript",
    "javascript",
    "rust",
    "python",
    "steel",
    "markdown",
    "json",
    "csv",
    "toml",
    "yaml",
    "html",
    "css",
];

#[test]
fn booting_the_shipped_tree_declares_the_twelve_in_order() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();
    let declared: Vec<&str> = languages.iter().map(|(name, _)| name.0.as_str()).collect();
    assert_eq!(
        declared, FIRST_CLASS,
        "runtime/languages/ is the bundled set; nothing else declares a language"
    );
}

/// Every declaration reaches the table with the fields the file wrote.
///
/// Spot-checked at the three shapes rather than at all twelve, because the
/// point is that each *shape* survives the crossing: a server with arguments, a
/// server with none, and a language with no grammar at all.
#[test]
fn the_declared_fields_cross_the_barrier_intact() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();

    let rust = languages
        .get(&LanguageId("rust".to_owned()))
        .expect("rust is declared");
    assert_eq!(rust.extensions, ["rs"]);
    assert_eq!(rust.grammar.as_deref(), Some("rust"));
    assert_eq!(rust.lsp_command, ["rust-analyzer"]);
    assert_eq!(rust.comment_prefix.as_deref(), Some("//"));

    let toml = languages
        .get(&LanguageId("toml".to_owned()))
        .expect("toml is declared");
    assert_eq!(
        toml.lsp_command,
        ["taplo", "lsp", "stdio"],
        "a command and its arguments are one list, and the order is the shell's"
    );

    let csv = languages
        .get(&LanguageId("csv".to_owned()))
        .expect("csv is declared");
    assert_eq!(csv.grammar, None, "T082: no tree-sitter-csv, deliberately");
    assert_eq!(csv.comment_prefix, None);
}

/// **`T104`'s per-language indent, over all twelve rather than a spot check.**
///
/// This is the assertion the shipped tree earns and no unit test can: before
/// `T104` the unit was a `match` on the **grammar** name inside
/// `vendor/ratatui-code-editor`'s `utils::indent`, and these twelve values
/// reproduce what it answered **exactly** — four for the four it named, two for
/// everything else, including `steel` (whose grammar is `scheme`) and `csv`
/// (which names none). So the answer moved out of the fork and into the layer
/// with no language's behaviour changing, and that is only checkable by
/// enumerating them.
///
/// `None` is not a hole here: it is *"take the global answer"*, which
/// `runtime/init.scm` sets to four spaces. Four of the twelve say it.
#[test]
fn every_shipped_language_declares_the_indent_it_used_to_be_given() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();

    for (language, expected) in [
        ("rust", None),
        ("python", None),
        ("toml", None),
        ("html", None),
        ("typescript", Some("  ")),
        ("javascript", Some("  ")),
        ("steel", Some("  ")),
        ("markdown", Some("  ")),
        ("json", Some("  ")),
        ("csv", Some("  ")),
        ("yaml", Some("  ")),
        ("css", Some("  ")),
    ] {
        let id = LanguageId(language.to_owned());
        assert!(languages.get(&id).is_some(), "{language} is declared");
        assert_eq!(
            languages.indent(&id),
            expected,
            "{language}'s indent is not what its file declares"
        );
    }
}

/// Three of the twelve declare no server, and that is the shape
/// `define-language!` had to accept.
///
/// If a declaration required a command, `steel`, `csv` and `markdown` could not
/// be declared at all and the bundled set would be nine — which is the exact
/// failure `T036`'s contract asked this task to avoid.
#[test]
fn a_declaration_may_name_no_server_at_all() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();

    let serverless: Vec<&str> = languages
        .iter()
        .filter(|(_, spec)| spec.lsp_command.is_empty())
        .map(|(name, _)| name.0.as_str())
        .collect();
    assert_eq!(serverless, ["steel", "markdown", "csv"]);

    assert_eq!(
        languages.tier(&LanguageId("markdown".to_owned())),
        Tier::FirstClass,
        "a language server is not what first class means; a grammar is"
    );
}

/// The tier of each of the twelve, as this binary can actually deliver it.
///
/// Two are second tier and the reasons differ. `csv` names no grammar at all —
/// `T082`, deliberately. `steel` names `scheme`, which this build cannot load:
/// the vendored fork bundles no such grammar and its own manifest says adding
/// it is `S4`'s job. The tier said `first-class` for `steel` until `Tier::of`
/// stopped trusting the spelling of a declaration, which meant the `languages`
/// query promised node anchoring and structural text objects for a buffer that
/// renders unhighlighted.
#[test]
fn the_tier_of_each_of_the_twelve_is_what_this_binary_can_deliver() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();

    let second_tier: Vec<&str> = languages
        .iter()
        .map(|(name, _)| name)
        .filter(|name| languages.tier(name) == Tier::SecondTier)
        .map(|name| name.0.as_str())
        .collect();
    assert_eq!(
        second_tier,
        ["steel", "csv"],
        "the other ten name a grammar this build bundles"
    );
    assert_eq!(
        languages
            .get(&LanguageId("steel".to_owned()))
            .expect("steel is declared")
            .grammar
            .as_deref(),
        Some("scheme"),
        "steel's declaration is right; it is the grammar table that has no arm"
    );
}

/// The nine servers are the nine `lsp::blessed` knows, spelled the same way.
///
/// `T036` left this as a contract — *"transcribe `lsp::blessed`'s nine entries
/// into `runtime/languages/*.scm`"* — and a transcription nobody checks is a
/// second table. This crate cannot see `phosphor-buffer` (the Steel barrier),
/// so the commands are written out here and
/// `crates/phosphor-buffer/tests/lsp.rs` is where the other end of the same
/// pair lives.
#[test]
fn the_nine_servers_are_the_blessed_nine() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();

    let expected: [(&str, &[&str]); 9] = [
        ("typescript", &["typescript-language-server", "--stdio"]),
        ("javascript", &["typescript-language-server", "--stdio"]),
        ("rust", &["rust-analyzer"]),
        ("python", &["pyright-langserver", "--stdio"]),
        ("json", &["vscode-json-language-server", "--stdio"]),
        ("toml", &["taplo", "lsp", "stdio"]),
        ("yaml", &["yaml-language-server", "--stdio"]),
        ("html", &["vscode-html-language-server", "--stdio"]),
        ("css", &["vscode-css-language-server", "--stdio"]),
    ];
    for (language, command) in expected {
        let spec = languages
            .get(&LanguageId(language.to_owned()))
            .unwrap_or_else(|| panic!("{language} is declared"));
        assert_eq!(spec.lsp_command, command, "{language}'s server");
    }
}

/// Opening a file picks its language, and the shipped extensions cover the tree
/// this repo is written in.
#[test]
fn the_shipped_extensions_recognise_this_repository() {
    let (_runtime, recorder) = booted();
    let languages = recorder.languages();
    for (path, language) in [
        ("crates/phosphor/src/main.rs", "rust"),
        ("Cargo.toml", "toml"),
        ("runtime/init.scm", "steel"),
        ("docs/TASKS.md", "markdown"),
        (".github/workflows/ci.yaml", "yaml"),
    ] {
        assert_eq!(
            languages.by_path(std::path::Path::new(path)),
            Some(&LanguageId(language.to_owned())),
            "{path}"
        );
    }
    assert_eq!(
        languages.by_path(std::path::Path::new("LICENSE")),
        None,
        "second tier is the honest answer, not an error"
    );
}

// ---------------------------------------------------------------------------
// CP-4's manual half
// ---------------------------------------------------------------------------

/// **A thirteenth language, added the way a user would, with no Rust change —
/// and first class.**
///
/// This is `T037`'s acceptance criterion and `CP-4`'s manual half, and it is
/// one `Runtime::evaluate` because that is all the REPL is. Nothing below
/// touches a Rust table: the form goes through the same generated binding the
/// twelve files use, and the assertion is that the same table answers for it.
///
/// It declares `jsonc` over the **bundled** `json` grammar on purpose. The
/// first version of this test declared `elixir` with `"grammar" void` and
/// asserted `SecondTier`, which demonstrated the second-tier road and stood in
/// for the first-class one — so a criterion reading *"a 13th language can be
/// added from the REPL with no Rust change"* was signed off by a test that
/// never walked it. Reusing a grammar the binary already has is the whole of
/// the first-class road that userspace owns;
/// [`the_grammar_is_the_one_thing_a_thirteenth_still_needs_rust_for`] is the
/// half that is not userspace's, stated as a test rather than as prose.
#[test]
fn a_thirteenth_language_needs_no_rust() {
    let (mut runtime, recorder) = booted();
    assert_eq!(recorder.languages().len(), 12);

    let outcome = runtime.evaluate(
        r##"(define-language! "jsonc"
              (hash "extensions" '("jsonc")
                    "grammar" "json"
                    "lsp_command" '("vscode-json-language-server" "--stdio")
                    "comment_prefix" "//"))"##,
    );
    assert!(
        matches!(outcome, Outcome::Done(_)),
        "the REPL's road up: {outcome:?}"
    );

    let languages = recorder.languages();
    let jsonc = LanguageId("jsonc".to_owned());
    assert_eq!(languages.len(), 13);
    assert_eq!(
        languages.by_path(std::path::Path::new(".vscode/settings.jsonc")),
        Some(&jsonc)
    );
    assert_eq!(
        languages.tier(&jsonc),
        Tier::FirstClass,
        "a grammar this build already has needs no Rust to claim"
    );
    assert_eq!(
        languages
            .get(&jsonc)
            .expect("jsonc is declared")
            .lsp_command,
        ["vscode-json-language-server", "--stdio"],
        "and it names its own server"
    );
    assert_eq!(
        languages.comment_prefix(&jsonc),
        Some("//"),
        "and gc works in it, which is what a locale hook buys"
    );
}

/// **A thirteenth language needing a grammar this build does not have is
/// second tier, and saying so is the honest half of an unfinished road.**
///
/// Four fields cross from Scheme with no Rust at all — extensions, comment
/// prefix, server command, and the grammar's *name*. Loading a grammar nobody
/// compiled in is the fifth thing, and it is not userspace's: the resolution
/// table is `Code::get_language` in `vendor/ratatui-code-editor`, a `match`
/// behind `#[cfg(feature = "grammar-*")]`, and there is no `libloading` in this
/// tree (`grep -rn 'libloading|dlopen|Library::new' crates/ vendor/*/src` → no
/// hits). So a genuinely *new* grammar costs an arm in that match, a
/// `grammar-*` feature and dependency in the fork's manifest, an entry in its
/// `grammars-phosphor` set, a pass-through in `crates/phosphor-buffer`, and a
/// `VENDOR.md` hunk — four files, two of them inside a fork.
///
/// What this test pins is that the editor **says so** rather than promising
/// anchoring it cannot do. The day a `scheme` arm lands, `steel` and this both
/// change answer with no declaration edited, which is the shape that makes the
/// grammar table the only missing piece.
#[test]
fn the_grammar_is_the_one_thing_a_thirteenth_still_needs_rust_for() {
    let (mut runtime, recorder) = booted();

    let outcome = runtime.evaluate(
        r##"(define-language! "elixir"
              (hash "extensions" '("ex" "exs")
                    "grammar" "elixir"
                    "lsp_command" '("elixir-ls")
                    "comment_prefix" "#"))"##,
    );
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");

    let languages = recorder.languages();
    let elixir = LanguageId("elixir".to_owned());
    assert_eq!(
        languages.by_path(std::path::Path::new("lib/retry.ex")),
        Some(&elixir),
        "everything that is not the grammar arrives"
    );
    assert_eq!(languages.comment_prefix(&elixir), Some("#"));
    assert_eq!(
        languages.tier(&elixir),
        Tier::SecondTier,
        "naming a grammar nothing can load does not make one; the tier is honest"
    );

    let answer = recorder
        .answer(&Query::Ui(UiQuery::Languages {
            language: Some(elixir),
        }))
        .expect("the recorder answers this one");
    let Value::List(rows) = answer.value else {
        panic!("languages answers a list");
    };
    let [Value::Record(fields)] = rows.as_slice() else {
        panic!("one language asked for, one row back: {rows:?}");
    };
    assert_eq!(
        fields.get("grammar"),
        Some(&Value::Text("elixir".to_owned())),
        "the declaration is kept verbatim — the arm will be keyed off this name"
    );
    assert_eq!(
        fields.get("tier"),
        Some(&Value::Text("second-tier".to_owned())),
        "and the query says which of the two you are actually in"
    );
}

/// A declaration that could never match a file is refused, with the reason
/// under the form you typed.
///
/// The `.ex` spelling is the one a user reaches for and the one that silently
/// never matched, because `Path::extension` answers `ex`. A REPL road whose
/// failures are silent is not a road.
#[test]
fn the_repl_refuses_a_declaration_that_could_never_match() {
    let (mut runtime, recorder) = booted();

    let outcome = runtime.evaluate(
        r##"(define-language! "elixir"
              (hash "extensions" '(".ex")
                    "grammar" void
                    "lsp_command" '()
                    "comment_prefix" "#"))"##,
    );
    // A refused Action inside an evaluated form comes back as the form's value,
    // `(#refused "…")` — `phosphor_steel::registry::REFUSED`, which is what the
    // REPL prints under the line you typed.
    let Outcome::Done(receipt) = &outcome else {
        panic!("the form evaluated; the declaration inside it is what refused: {outcome:?}");
    };
    let Value::List(answer) = &receipt.value else {
        panic!("a refusal answers `(#refused \"…\")`, got {receipt:?}");
    };
    let [Value::Text(tag), Value::Text(reason)] = answer.as_slice() else {
        panic!("a refusal answers `(#refused \"…\")`, got {answer:?}");
    };
    assert_eq!(tag, "#refused");
    assert!(
        reason.contains(".ex"),
        "the refusal names the extension: {reason}"
    );
    assert_eq!(
        recorder.languages().len(),
        12,
        "and the table is where it was"
    );
}

/// Which of the five keys may be left out, stated rather than discovered.
///
/// `grammar`, `comment_prefix` and `indent` are `Option<String>` and so are
/// omissible; `extensions` and `lsp_command` are `Vec<String>` and are not, because
/// `Wire::REQUIRED` is `false` only for `Option`. That reads as an
/// inconsistency from the REPL — `lsp_command`'s own doc says *"empty means
/// none"*, which sounds like *"leave it out"* — so what makes it acceptable is
/// that the refusal **names the missing key**. A document claiming a refusal is
/// legible, with nothing checking it, is how the claim rots.
///
/// The two refusals arrive in different shapes and both are legible: a record
/// the door cannot decode never becomes an Action, so the *form* is refused,
/// where a decoded declaration the table rejects is a value —
/// [`the_repl_refuses_a_declaration_that_could_never_match`] is that one.
///
/// **The count was two until `T104` added `indent`.** Each string below omits
/// exactly one optional key and names the other two, so a fourth optional key
/// arriving without a case here leaves an assertion nobody makes rather than a
/// test that quietly still passes.
#[test]
fn the_three_optional_keys_are_the_three_optional_types() {
    let (mut runtime, _recorder) = booted();

    let declare = |runtime: &mut Runtime, keys: &str| {
        runtime.evaluate(&format!(r#"(define-language! "probe" (hash {keys}))"#))
    };

    for omitted in [
        r##""extensions" '("pr") "lsp_command" '() "comment_prefix" "#" "indent" "  ""##,
        r#""extensions" '("pr") "grammar" void "lsp_command" '() "indent" "  ""#,
        r##""extensions" '("pr") "grammar" void "lsp_command" '() "comment_prefix" "#""##,
    ] {
        let outcome = declare(&mut runtime, omitted);
        assert!(
            matches!(&outcome, Outcome::Done(receipt)
                if receipt.value == Value::Text("#ok".to_owned())),
            "grammar, comment_prefix and indent may simply be left out: {outcome:?}"
        );
    }

    let outcome = declare(&mut runtime, r#""extensions" '("pr") "grammar" void"#);
    // `T100`: a *raise*, not a refusal. The barrier could not decode the record
    // into an Action, so the form never reached the host and nothing declined
    // anything — which is the distinction the third `Outcome` case exists for,
    // and this assertion used to record the opposite.
    let Outcome::Raised(raised) = &outcome else {
        panic!("a record the door cannot decode never becomes an Action: {outcome:?}");
    };
    assert!(
        raised.why().contains("lsp_command"),
        "and it says which key it wanted: {}",
        raised.why()
    );
}

/// **Nothing but `runtime/languages/` declares a language.**
///
/// The other side of [`booted`]'s emptiness assertion, and the one that
/// survives somebody deciding a default is convenient: boot a tree whose
/// `phosphor/boot-files` is empty and the table stays empty. A `Languages` with
/// twelve baked in fails here no matter what the shipped tree contains.
#[test]
fn a_runtime_with_no_languages_directory_declares_nothing() {
    let root = std::env::temp_dir().join(format!(
        "phosphor-t037-bare-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a temp runtime tree");
    std::fs::write(root.join("init.scm"), "(define phosphor/boot-files '())\n")
        .expect("a bare init.scm");

    let recorder = Arc::new(Recorder::new());
    let runtime = Runtime::boot(Some(&root), Arc::clone(&recorder) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "a tree with no languages is a legal tree: {:?}",
        runtime.report().faults
    );
    assert!(
        recorder.languages().is_empty(),
        "the editor holds no copy of the twelve"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Redeclaring a shipped language from the REPL replaces it, rather than
/// shadowing it with a duplicate.
///
/// The road up has to be a road *over* as well: swapping `rust-analyzer` for a
/// wrapper script is the first thing anyone does, and it must not leave two
/// rows called `rust` for the `languages` query to answer twice.
#[test]
fn redeclaring_a_shipped_language_replaces_it() {
    let (mut runtime, recorder) = booted();

    let outcome = runtime.evaluate(
        r#"(define-language! "rust"
              (hash "extensions" '("rs")
                    "grammar" "rust"
                    "lsp_command" '("ra-multiplex")
                    "comment_prefix" "//"))"#,
    );
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");

    let languages = recorder.languages();
    assert_eq!(languages.len(), 12, "a redeclaration is an edit");
    assert_eq!(
        languages
            .get(&LanguageId("rust".to_owned()))
            .expect("rust")
            .lsp_command,
        ["ra-multiplex"]
    );
}

// ---------------------------------------------------------------------------
// The query
// ---------------------------------------------------------------------------

/// `(languages)` answers what the tree declared, tier included.
///
/// The read side of the same seam: a declaration that lands in the table but
/// cannot be read back is a table nothing can render, and `:help`, the picker
/// and the statusline all reach it this way.
#[test]
fn the_languages_query_answers_the_declared_twelve() {
    let (_runtime, recorder) = booted();
    let answer = recorder
        .answer(&Query::Ui(UiQuery::Languages { language: None }))
        .expect("the recorder answers this one");
    let Value::List(rows) = answer.value else {
        panic!("languages answers a list");
    };
    assert_eq!(rows.len(), 12);

    let answer = recorder
        .answer(&Query::Ui(UiQuery::Languages {
            language: Some(LanguageId("markdown".to_owned())),
        }))
        .expect("the recorder answers this one");
    let Value::List(rows) = answer.value else {
        panic!("languages answers a list");
    };
    let [Value::Record(fields)] = rows.as_slice() else {
        panic!("one language asked for, one row back: {rows:?}");
    };
    assert_eq!(
        fields.get("tier"),
        Some(&Value::Text("first-class".to_owned()))
    );
    assert_eq!(
        fields.get("grammar"),
        Some(&Value::Text("markdown".to_owned()))
    );
    assert_eq!(
        fields.get("lsp_command"),
        Some(&Value::List(Vec::new())),
        "markdown's serverlessness is a declared fact, not a missing field"
    );
}
