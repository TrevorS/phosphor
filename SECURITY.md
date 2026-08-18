# Security

## Reporting a vulnerability

Use GitHub's [private vulnerability reporting](https://github.com/TrevorS/phosphor/security/advisories/new)
rather than a public issue. That opens a private thread with the maintainer and gives us somewhere
to talk before anything is public.

Please do not open a public issue for anything you believe is exploitable.

## Scope, honestly

Phosphor is a build in progress and is not deployed anywhere. The parts most worth a security
reader's attention are the ones that parse or execute something that did not come from the person
at the keyboard:

- **The LSP client** (`crates/phosphor-buffer/src/lsp.rs`) reads frames from a language server
  over a pipe. It bounds both a frame's declared `Content-Length` and a header line's length,
  because `async-lsp` allocates from the declared length before a byte of the body arrives — an
  unbounded number there is an allocation failure, and Rust's answer to one is `abort()`. The
  scanner that enforces those bounds is fuzzed (`fuzz/fuzz_targets/lsp_wire.rs`).
- **The Steel layer** runs Scheme from `runtime/*.scm` and from the user's own config home. It is
  the user's code by design — a `:repl` that could not reach the editor would not be a repl — so
  the barrier is architectural rather than a sandbox: `phosphor-steel` reaches `phosphor-core` and
  the VM and nothing else, and every capability crosses one registry.
- **The parsers** behind `fuzz/fuzz_targets/` — the journal, key notation, theme files, CSV.

`cargo deny check` runs in CI over advisories, licences, bans and sources. Its ignore list is in
`deny.toml`, with a note per entry saying why.

## What is not a vulnerability

A capability that refuses by naming an unbuilt task is working as intended — see the README.
