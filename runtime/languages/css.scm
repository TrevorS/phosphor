;; css — first class. The other half of the web substrate.
;;
;; grammar: tree-sitter-css 0.25.0, bundled. T083: clean, no known gaps.
;;
;; server: vscode-css-language-server, awaiting CP-4.
;;
;; comment prefix: none, and this one surprises people. CSS has `/* */` and
;; nothing else — `//` is a syntax error that takes the rest of the rule with
;; it — so `gc` does nothing here rather than breaking the stylesheet.

(define-language! "css"
  (hash "extensions" '("css")
        "grammar" "css"
        "lsp_command" '("vscode-css-language-server" "--stdio")
        "comment_prefix" void
        ;; two spaces. a rule body is one level deep and nesting is the
        ;; exception, so the width buys nothing and the cost of four shows up in
        ;; the nested cases that do exist — media queries and `@supports`, where
        ;; a selector is already indented before its first declaration. prettier
        ;; and stylelint both default here.
        "indent" "  "))
