;; markdown — first class.
;;
;; grammar: tree-sitter-md 0.5.3, bundled. T083: clean. The crate carries a
;; second grammar, `markdown-inline`, for the pass over emphasis, links and code
;; spans inside a block; the fork resolves that name too, and it is an injection
;; rather than a language you open a file in, so it gets no declaration here.
;;
;; server: none. Not an oversight and not "we could not find one" — the design
;; gives markdown a surface of its own (live preview) rather than generic buffer
;; treatment, and no markdown server has been run against this build.
;;
;; comment prefix: none. Markdown's comment is the HTML one, `<!-- -->`, which
;; is a delimiter pair rather than a line prefix — so `gc` does nothing here
;; rather than inserting something that would render.

(define-language! "markdown"
  (hash "extensions" '("md" "markdown")
        "grammar" "markdown"
        "lsp_command" '()
        "comment_prefix" void))
