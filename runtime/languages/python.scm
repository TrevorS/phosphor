;; python — first class.
;;
;; grammar: tree-sitter-python 0.25.0, bundled. T083: clean, no known gaps.
;;
;; server: pyright-langserver, awaiting CP-4, whose checklist names it. Root
;; markers are pyproject.toml, setup.py, setup.cfg — three because python has
;; had three answers to "where does this project start" and all three are still
;; in the wild.
;;
;; `pyi` rides along: a stub file is python, parses with the same grammar, and a
;; server that has your `.py` open wants the `.pyi` beside it.

(define-language! "python"
  (hash "extensions" '("py" "pyi")
        "grammar" "python"
        "lsp_command" '("pyright-langserver" "--stdio")
        "comment_prefix" "#"))
