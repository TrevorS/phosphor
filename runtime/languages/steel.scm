;; steel — the language this directory is written in, and second tier until the
;; grammar arm lands. See below: the declaration is right, the build cannot act
;; on it yet, and `(languages)` says second-tier rather than promising anchoring
;; it cannot do.
;;
;; grammar: tree-sitter-scheme 0.24.7. T083 parsed the fixture clean and
;; recorded two gaps, both of them reader syntax steel has and the grammar does
;; not: `#u8(...)` (the grammar only has R6RS `#vu8(...)`) and `#%`-prefixed
;; compiler internals. Neither appears in this tree, and
;; crates/phosphor-steel/tests/steel_grammar.rs parses every file we ship with
;; it on every gate, so the day one does is a red test rather than a buffer that
;; quietly stops highlighting.
;;
;; The grammar is named `scheme` and not `steel` because that is what it parses:
;; steel is a scheme dialect, and the grammar is R6RS/R7RS scheme. Nothing in
;; the editor resolves that name today — the vendored fork's grammar table has
;; no arm for it and its own manifest says so ("the two in that twelve the crate
;; does not bundle are Steel … adding them here is S4's job"). So a `.scm`
;; buffer renders unhighlighted until that arm lands, which is the same answer
;; the fork already gives for any name it does not know: no new failure mode,
;; and the declaration is the thing the arm will be keyed off.
;;
;; **That is why `(languages)` answers `second-tier` for steel.** The tier used
;; to be "did the declaration name a grammar", which answered `first-class`
;; here — for a buffer that can never highlight — and answered it for any name
;; at all, including ones nothing in the build has heard of. It is now "does
;; this build have that grammar", so the day the arm lands this file changes
;; answer with nothing in it edited.
;;
;; server: none, and none exists.

(define-language! "steel"
  (hash "extensions" '("scm")
        "grammar" "scheme"
        "lsp_command" '()
        "comment_prefix" ";;"
        ;; two spaces, which is every scheme's convention and this file's own.
        "indent" "  "))
