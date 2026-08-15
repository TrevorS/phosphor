;; typescript — first class.
;;
;; grammar: tree-sitter-typescript 0.23.2, bundled by the fork under the name
;; `typescript`. T083 parsed the fixture clean with no known gaps.
;;
;; server: typescript-language-server, which is the tsserver wrapper rather than
;; tsserver itself — tsserver speaks its own protocol, not LSP. Root markers
;; (tsconfig.json, then package.json) stay in Rust: LanguageSpec has no field for
;; them, so replacing the command here keeps them.
;;
;; `tsx` is not in this list. It is a different grammar — tree-sitter-typescript
;; ships LANGUAGE_TSX beside LANGUAGE_TYPESCRIPT, and JSX in a `.ts` file is a
;; parse error either way — so it wants a declaration of its own rather than a
;; second extension on this one. That declaration is one file in this directory,
;; which is the road up working, not a gap.

(define-language! "typescript"
  (hash "extensions" '("ts" "mts" "cts")
        "grammar" "typescript"
        "lsp_command" '("typescript-language-server" "--stdio")
        "comment_prefix" "//"))
