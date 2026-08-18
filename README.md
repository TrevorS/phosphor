# Phosphor

An agent-native terminal editor. A spiritual successor to Light Table for the age of coding
agents, with the terminal as a first-class citizen.

Rust core, [Steel](https://github.com/mattwparas/steel) (embedded Scheme) editor layer,
[ratatui](https://ratatui.rs) for drawing.

## Status: in development, and it will tell you so

This is a build in progress, not a released editor. It opens files, edits them the way vim's
grammar does, talks to real language servers, and draws the surfaces its design specifies — but
a great deal of the vocabulary is **declared and not yet built**.

That is deliberate and it is visible rather than hidden. Every capability is a row in one
registry, and a row whose implementation has not landed answers by naming the task that will
build it:

```console
$ phosphor open-timeline
#refused · not built yet — T073 builds it
```

So if you press a key and get a sentence with a task id in it, that is the editor being honest,
not broken. `docs/TASKS.md` is what those ids refer to.

## Try it

Needs the pinned toolchain in `rust-toolchain.toml` (rustup installs it automatically) and
[`just`](https://github.com/casey/just).

```console
$ just install          # builds and puts `phosphor` on your PATH
$ phosphor src/main.rs
```

`ZQ` or `ctrl-c` leaves. `:help` lists the keymap, `:repl` opens a Scheme prompt against the
running editor.

Without a file it opens an empty buffer; `:write <path>` gives it a name.

## How it is put together

Four things are unusual enough to be worth knowing before reading any code.

**There is no keymap in Rust.** Every binding in the editor lives in `runtime/keymaps.scm`, and
the input machine asks that file on every keystroke without caching the answer. A binding you
change at `:repl` is in force on the very next key, with no reload step. `crates/phosphor-steel/tests/no_bindings_in_rust.rs` fails if any Rust file binds a key.

**One registry, three doors.** A capability is declared once; the Steel, MCP and CLI front-ends
are total functions over that table and none of them may name a capability. That is what makes a
one-door capability unconstructible rather than merely tested against — and
`crates/phosphor/tests/parity.rs` walks every row through all three doors so the claim is
enumerated rather than asserted.

**The lints are structural, and each exists because the thing it catches already happened.**
They live in `scripts/lint-*.sh`, run under `just lint`, and enforce things a type system cannot:
no literal colours outside the theme, no `Action` construction in the widget layer, every
mutation a ticked task declares reachable from a keystroke, every claim in the docs recomputed
from the tree it describes. Adding one means dropping a script into that directory — never
editing the justfile or CI.

**The design documents are the specification.** `docs/design/*.dc.html` are imported verbatim
from the design project that produced them; the palette, the 37 mockup screens and the voice come
from there. Where the build and the design disagree, that gets flagged rather than quietly
reconciled.

### Crates

| Crate | What it holds |
| --- | --- |
| `phosphor-core` | The vocabulary, the Action spine, the store. Zero runtime dependencies. |
| `phosphor-ui` | Widgets. Takes `ratatui-core` only, reads ViewModels, never mutates. |
| `phosphor-steel` | The Scheme layer and the barrier around it. |
| `phosphor-buffer` | Buffers, grammars, the LSP client. |
| `phosphor-term` | Terminal setup and the synchronized-output frame. |
| `phosphor` | The binary: the loop, the host, the doors. |
| `phosphor-agent`, `phosphor-vcs` | Declared, largely unbuilt. |

`vendor/` holds two `git subtree` forks kept deliberately outside the workspace so our lints stop
at the seam. Every hunk that diverges from upstream has to be explained in that fork's
`VENDOR.md`.

## Building on it

```console
$ just            # list every recipe
$ just gate       # everything CI runs, in CI's order — run this before calling something green
$ just test       # cargo-nextest, plus the doctests nextest cannot see
$ just lint       # the structural lints
```

`just --list` is the authority on what exists, not this file.

## Reading more

- `docs/README.md` — the reading order for the design documents and the plan. Start there.
- `CLAUDE.md` — the working agreement: commands, the lints and what each one caught, the rules
  about vendored code, and the standard for asserting things you have not read.
- `docs/TASKS.md` — the task breakdown, with acceptance criteria and the checkpoints.

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. This is the convention across Rust crates, and it matches the vendored forks.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
