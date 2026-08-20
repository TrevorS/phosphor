;; yaml — first class. The other config dialect agents live in.
;;
;; grammar: tree-sitter-yaml 0.7.2, bundled. T083: clean, no known gaps.
;;
;; server: yaml-language-server, awaiting CP-4. No root markers — a workflow
;; file, a compose file and a k8s manifest have nothing in common to look for,
;; and the workspace root is the honest answer.
;;
;; both spellings of the extension, because both are in every tree.

(define-language! "yaml"
  (hash "extensions" '("yaml" "yml")
        "grammar" "yaml"
        "lsp_command" '("yaml-language-server" "--stdio")
        "comment_prefix" "#"
        ;; two spaces. yaml is the one language where the width is not taste:
        ;; a block sequence under a mapping key is unreadable at four.
        "indent" "  "))
