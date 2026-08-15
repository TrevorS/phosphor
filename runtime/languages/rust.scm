;; rust — first class.
;;
;; grammar: tree-sitter-rust 0.24.2, bundled. T083: clean, no known gaps.
;;
;; server: rust-analyzer, and the only one of the nine this build has actually
;; spawned — crates/phosphor-buffer/tests/lsp_rust_analyzer.rs drives it to
;; ready. No `--stdio`: rust-analyzer speaks LSP on its standard streams with no
;; flag, and passing one makes it exit.
;;
;; The name is not searched for. If `rust-analyzer` on your PATH is the rustup
;; shim for a toolchain that does not have it, it prints one line and exits, and
;; you get a spawn failure that says so — which is the whole reason this is a
;; command we chose rather than a discovery (lsp.rs, "blessed, not discovered").

(define-language! "rust"
  (hash "extensions" '("rs")
        "grammar" "rust"
        "lsp_command" '("rust-analyzer")
        "comment_prefix" "//"))
