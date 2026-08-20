;; json — first class.
;;
;; grammar: tree-sitter-json 0.24.8, bundled. T083 recorded one gap: `1e+2` — a
;; `+` exponent sign is valid per RFC 8259 §6 and the grammar's exponentPart
;; only allows an optional `-`, so a legal document highlights as an error. The
;; characterisation test in grammar_abi.rs holds that gap open; it is the
;; grammar's bug, not ours, and it is worth knowing before you believe a red
;; number.
;;
;; server: vscode-json-language-server, the one vscode ships, awaiting CP-4.
;; No root markers: a JSON file is its own project.
;;
;; comment prefix: none. JSON has no comments. `//` in a `.json` file is JSON5 or
;; JSONC, which is a different language and would want its own declaration.

(define-language! "json"
  (hash "extensions" '("json")
        "grammar" "json"
        "lsp_command" '("vscode-json-language-server" "--stdio")
        "comment_prefix" void
        ;; two spaces, and this is the one language where the value is written
        ;; by machines more often than by hands: `JSON.stringify(x, null, 2)` is
        ;; the idiom, `npm` writes `package.json` with two, and an editor that
        ;; shifted with four would make every hand edit a diff against the tool
        ;; that wrote the file.
        "indent" "  "))
