;; csv — the one with no grammar, deliberately.
;;
;; T082 is the finding: tree-sitter-csv is 2.5 years stale with ~5k downloads,
;; and CSV gets column alignment rather than generic buffer treatment anyway. A
;; small parser is more reliable than a stale grammar *and* yields exactly the
;; column model that surface needs, so there is no tree-sitter dependency here
;; and `grammar` is void.
;;
;; **That makes the `languages` query answer `second-tier` for csv**, because
;; the tier is derived from the grammar and nothing else — specifically, from
;; the grammar *this build can load* (phosphor_core::language::Languages::tier).
;; The Component Breakdown lists CSV among the
;; first-class twelve. Both sentences are in the design and they disagree; this
;; file follows the vocabulary, which is the half a test can check, and the
;; disagreement is flagged for CP-4 rather than resolved here.
;;
;; What csv actually gets is `align-columns` (T082), which is a capability of
;; its own and needs no grammar to reach.
;;
;; server: none exists. comment prefix: none — CSV has no comment syntax, and a
;; `#` line would be a data row with one field.

(define-language! "csv"
  (hash "extensions" '("csv")
        "grammar" void
        "lsp_command" '()
        "comment_prefix" void))
