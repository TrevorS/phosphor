;; javascript — first class.
;;
;; grammar: tree-sitter-javascript 0.25.0, bundled. T083: clean, no known gaps.
;;
;; server: the same binary typescript gets. That is not a shortcut —
;; typescript-language-server serves javascript through the same tsserver, and
;; giving it its own declaration is what lets the root markers differ
;; (jsconfig.json before package.json) and what lets you swap one language's
;; server without touching the other.
;;
;; `jsx` is left out for the reason `tsx` is: it is a different grammar.

(define-language! "javascript"
  (hash "extensions" '("js" "mjs" "cjs")
        "grammar" "javascript"
        "lsp_command" '("typescript-language-server" "--stdio")
        "comment_prefix" "//"))
