# `runtime/languages/` — the first-class twelve

One file per language, each one nothing but a `(define-language! …)` call and the
commentary that says why its five fields read the way they do.

**This directory is the bundled set.** Not a copy of a Rust table, not a wrapper
over one — the Component Breakdown's *"the bundled set is just the
`define-language` calls we wrote and stand behind"* is only true if there is
nowhere else a language can come from, so `phosphor_core::language` ships with
no list in it and the editor knows exactly what these twelve files declared
(`T037`).

The thirteenth is the acceptance test. `CP-4`'s manual half is Teej adding a
language from the REPL, and the only difference between that and these twelve is
which file the form was typed into.

## The five fields

`define-language!` takes a name and a record — `phosphor_core::request::LanguageSpec`,
whose field names are the hash keys here.

| key | means | absent means |
|---|---|---|
| `extensions` | file extensions, without the dot | required; `'()` claims no file |
| `grammar` | the tree-sitter grammar's name, as the binary spells it | second tier: no node anchoring, no structural text objects, no watches — and so does naming a grammar this build does not have |
| `lsp_command` | the server command and its arguments | required; `'()` is *no server*, and is an honest answer |
| `comment_prefix` | the line-comment prefix `gc` uses | the language has no line comment, and `gc` does nothing |
| `indent` | what one indent level **is**, literally — what `>`, `<` and `<tab>` write | the global answer: `expand-tab` and `tab-width` from `init.scm` |

An absent optional is `void`, spelled out rather than omitted — these files are
read as documentation, and a key that is missing looks like an oversight where a
`void` looks like a decision.

`grammar`, `comment_prefix` and `indent` are the only three you *may* omit:
`Option<T>` is what makes a field optional at the wire door, so `extensions` and
`lsp_command` are required even where the honest value is `'()`. Omitting
`lsp_command` refuses with ``missing required argument `lsp_command` ``, which
is legible but is the one place the five fields do not read alike.

### `indent` is a literal, and that is what makes it one field

Two things vary between languages — how *wide* a level is, and whether it is
made of spaces or a tab — and one string says both: `"    "` is four spaces,
`"\t"` is a tab, `"  "` is what eight of the twelve here declare. A number could
not have said the second, and go wants the second. It is the same shape as
`comment_prefix`: a per-language literal that an editing verb splices, which is
why this needed no new mechanism (`T104`).

**A declaration beats the global setting**, on vim's rule that an `ftplugin`
beats a global `set` — the narrower statement wins. So `rust`, `python`, `toml`
and `html` say `void` and take `init.scm`'s four spaces, and the other eight
declare `"  "` because two is what their communities settled on.

**One tab, or a run of spaces, and a declaration saying neither is refused.**
The field earns being one field by saying both of the things above, so a literal
that says neither is turned away at the door rather than read two different ways
inside: `" \t"` gave `>` a space-tab and `<tab>` two spaces, `""` gave `>` a
no-op and `<tab>` one space, `"\t\t"` gave `>` two tabs and `<tab>` one, and a
two-cell ideographic space measured one. The refusal names the value and the
three shapes that work. Omit the field — `void` — for the global answer; that is
what the empty string looked like it meant and did not.

**It is not the tabstop.** How wide a `\t` *renders* stays global (`tab-width`),
because a tabstop is a property of the grid and of files that mix languages
through injections, not of the language the buffer is declared as. A `"\t"`
indent in a build with `tab-width` 8 writes one character and draws eight
columns.

Before `T104` none of this was reachable: the unit was a `match` on the
**grammar** name inside `vendor/ratatui-code-editor`'s `utils::indent`, so
`steel` (grammar `scheme`) and `csv` (no grammar) landed on the same arm as an
undeclared file, and its `\t` arm — for `go` and `c_sharp` — was unreachable
because nothing here declares either. The twelve values above reproduce what
that table gave exactly, which is the point: the answer moved into the layer
without any *declared* language's behaviour changing.

**Every file no declaration claims did change, from two spaces to four**, and it
is the majority case rather than a corner: `utils::indent`'s `_` arm gave two
spaces to every `.sh`, `.c`, `.go`, `.lua`, `.txt` and `.log` — and to the
scratch buffer a bare `phosphor` opens — because all of them answered `"text"`.
Nothing claims them now either, so they take `init.scm`'s `tab-width` of 4.
Deliberate, and reversible with `(set-option! "tab-width" 2)`. `csv` is the one
that would have gone with them and does not, because it declares `"  "` rather
than `void`.

**Without the dot** is a rule and not a style: a declaration writing `".ex"` —
the way a user writes an extension — is **refused**, because `Path::extension`
answers `ex` and nothing would ever have matched it. So is a nameless language,
and so is a two-part `"tar.gz"` (that path's extension is `gz`). The reason
comes back under the form you typed, `(#refused "…")`.

## The tier is what this build can parse, not what a file claims

`(languages)` answers `first-class` for a language whose `grammar` names a
grammar **the binary actually has**, and `second-tier` otherwise. Naming one it
does not have is not an error and does not refuse — it is how `steel` is
written, and the day the arm lands that declaration changes answer with nothing
in it edited — but it does not buy anchoring either.

So of the twelve, ten are first class and two are not: `csv` names no grammar
(`T082`, deliberately) and `steel` names `scheme`, which the vendored fork does
not bundle and says so in its own manifest. Both files explain themselves.

**A thirteenth reusing a bundled grammar is first class with no Rust at all** —
`(define-language! "jsonc" (hash "extensions" '("jsonc") "grammar" "json" …))`
is the whole of it, and `crates/phosphor-steel/tests/shipped_languages.rs`
runs exactly that. A thirteenth needing a grammar nobody compiled in is second
tier until somebody compiles it in: the resolution table is a `match` in
`vendor/ratatui-code-editor`'s `Code::get_language` behind `grammar-*` features,
there is no dynamic grammar loading in this tree, and adding one costs an arm, a
feature, a dependency, a pass-through in `crates/phosphor-buffer` and a
`VENDOR.md` entry. That is the one part of the road up that is not userspace's,
and the tier says so out loud rather than promising anchoring it cannot do.

## Three of the twelve have no server, on purpose

`steel` and `csv` have no language server *in existence*, and the Component
Breakdown gives `markdown` a surface of its own — *"live preview … rather than
generic buffer treatment"* — so no markdown server has been run against this
build. `'()` is what that looks like. A first-class language with no server is
not a contradiction — `markdown` is exactly that — because the tier is about
the grammar and nothing else.

## What is declared here and what is verified elsewhere

The nine servers are transcribed from `phosphor_buffer::lsp::blessed`, which
records their verification status honestly: `rust-analyzer` is the only one this
build has spawned (`crates/phosphor-buffer/tests/lsp_rust_analyzer.rs`), and the
other eight await `CP-4`. The eleven grammars are `T083`'s verdict table
(`crates/phosphor-buffer/tests/GRAMMAR-ABI.md`), each parsed clean against the
0.26 runtime.

**A transcription is a second table**, so it is recomputed rather than trusted:
`crates/phosphor-buffer/tests/language_declarations.rs` reads these twelve files
as text and compares them against `blessed` and `FIRST_CLASS`. Edit a command
here without editing the other, or the other way round, and that test goes red —
which is what stops this directory from being the *"counts nothing else
recomputes"* failure class `CLAUDE.md` lists. It lives there and not in
`scripts/` because it needs `blessed` itself, and a shell script would have to
transcribe the table a third time to check the second one.

Root markers are **not** here, and that is now a gap rather than a decision.
`LanguageSpec` has no field for them and `ServerSpec::from_language_spec` reads
them from `blessed`, so a shipped language keeps its markers when you swap
`rust-analyzer` for a wrapper script — but a *thirteenth* language is in nobody's
`blessed`, gets `root_markers: []`, and `ServerSpec::root_for` returns `None` on
an empty list, so its server starts rootless. Adding the field is only half a
fix, and the consuming half is `phosphor-buffer`'s; raised, not folded in.
