;; toml — first class. One of the two config dialects agents live in.
;;
;; grammar: tree-sitter-toml-ng 0.7.0, bundled. The `-ng` fork rather than
;; tree-sitter-toml, which is unmaintained; T083: clean, no known gaps.
;;
;; server: taplo, and its argument list is `lsp stdio` — two positional words,
;; not a flag. That is the shape LanguageSpec's single `lsp_command` list exists
;; for: the command and its arguments are one sequence, and the server decides
;; what they mean.

(define-language! "toml"
  (hash "extensions" '("toml")
        "grammar" "toml"
        "lsp_command" '("taplo" "lsp" "stdio")
        "comment_prefix" "#"))
