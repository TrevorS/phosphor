;; html — first class. Half of the web substrate.
;;
;; grammar: tree-sitter-html 0.23.2, bundled. T083: clean, no known gaps.
;;
;; server: vscode-html-language-server, awaiting CP-4.
;;
;; comment prefix: none. `<!-- -->` is a delimiter pair, not a line prefix, so
;; `gc` does nothing here rather than half-commenting a tag.

(define-language! "html"
  (hash "extensions" '("html" "htm")
        "grammar" "html"
        "lsp_command" '("vscode-html-language-server" "--stdio")
        "comment_prefix" void))
