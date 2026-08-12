# VENDOR — `ratatui-code-editor`

Phosphor's fork of [`ratatui-code-editor`](https://github.com/vipmax/ratatui-code-editor),
the core of `BufferView`. Vendored per [plan §2](../../docs/IMPLEMENTATION-PLAN.md), task
`T003`.

| | |
|---|---|
| Upstream | `https://github.com/vipmax/ratatui-code-editor.git` |
| **Last merged** | **`40ff181514914a8602d2e5d2df647cca1f0ef621`** — `release: 0.0.6`, 2026-07-07 |
| Upstream version at that commit | `0.0.6` (`Cargo.toml:18`) |
| Mechanism | `git subtree --squash` into `vendor/ratatui-code-editor` |
| Consumed as | workspace `path` dependency — see the root `Cargo.toml` |
| Licence | MIT |

**A SHA, not a tag, and that is deliberate.** Upstream's newest tag is `v0.0.5`
(`e56faec7d9969b1337b536cd155b086589f692c0`); `0.0.6` was published to crates.io without
one. `40ff181` is the `release: 0.0.6` commit — the same tree crates.io serves. Every
`just vendor-pull` against this fork will need a SHA for the same reason until upstream
starts tagging.

**This is "our code that happens to have an upstream."** Six releases since 2025-10-16,
~3.4k lifetime downloads, one maintainer, and the entire central surface of the editor rests
on it. The plan's phrase, and the reason the discipline below is not optional.

---

## Working on this fork

- **`just vendor-diff ratatui-code-editor`** prints everything below and nothing else. A hunk
  with no entry here is the fork silently becoming a rewrite — add the entry or drop the hunk.
- **Minimal-diff discipline.** Phosphor *additions* go in a `phosphor/` module inside `src/`,
  with the smallest possible edit at the seam that calls into it. Nothing here needs that
  module yet — every patch so far is a gate over code that already existed, which is why
  `src/` sees three touched lines and no new files. The first real addition (S1's soft-wrap,
  `T081`) creates it.
- **The fork is excluded from the workspace** (root `Cargo.toml`'s `exclude`) while still
  being a path dependency. It builds, and `clippy --workspace -D warnings` stops at the seam.
  Upstream code failing our lints is not a bug we should be fixing in a diff we then have to
  carry — the fork already emits one `dead_code` warning, and that is exactly the shape of the
  problem.
- **`cargo fmt --all` is the exception, and `just fmt` is currently red because of it.**
  `--all` does not mean "workspace members"; cargo-fmt recurses into local *path dependencies*
  too, so it walks this fork and reports 36 diffs in code we did not write. `exclude` does not
  stop it and rustfmt's `ignore` option is nightly-only. The fix is in the `fmt` recipe —
  enumerate the members instead of `--all` — and it belongs to `harness`; see this phase's
  report. **Do not make this fork rustfmt-clean.** That trades a one-line recipe change for a
  permanent whole-file diff on every upstream merge.
- **Upstreaming policy.** Bug fixes, perf, and API generalisation go upstream as PRs.
  Phosphor-specific behaviour (the 3-column gutter contract, region tints, virtual-text
  interleaving) stays local, permanently. Every entry below is marked.

---

## Patches

### 1 · `#[cfg]` the sixteen bundled grammars down to the ten we load

**Files:** `Cargo.toml` (grammar deps → `optional = true`, plus a `[features]` block),
`src/code.rs` (one `#[cfg]` per arm of `Code::get_language`).
**Upstreamable:** yes — it is a pure feature-gate with upstream's dependency set preserved as
the default.

Upstream takes **16 tree-sitter grammars as hard, non-optional dependencies** — bash, c,
c-sharp, cpp, css, go, html, java, javascript, json, md, python, rust, toml-ng, typescript,
yaml — and bundles neither Scheme (which Steel rides) nor CSV, both of which phosphor's
first-class twelve requires. Grammar selection belongs in `runtime/` via `define-language`,
so the fork's job is to make the set a decision rather than a given.

| feature | grammars | note |
|---|---|---|
| `grammars-phosphor` | css, html, javascript, json, markdown, python, rust, toml, typescript, yaml | **the intersection with our first-class twelve — what `phosphor-buffer` enables** |
| `grammars-extra` | bash, c, c-sharp, cpp, go, java | the six we never load; opt-in |
| `grammars-all` | both | what `default` selects, so a standalone build here matches upstream |

Also one `grammar-<lang>` feature per grammar, for a `define-language` entry that wants
exactly one.

**Behaviour when a grammar is gated out:** `get_language` returns `None` — which is already
what it returns for any unrecognised language string. `Code::new` then skips parser setup and
the buffer renders unhighlighted. No caller learns a new failure mode; there is no new panic
and no new `Result`.

**Two things this patch deliberately does not do.** *Adding* grammars the fork lacks (Scheme
for Steel, and a hand-written CSV parser) is `S4`/`T082`, not fork work. And the embedded
highlight queries under `langs/` are **not** gated: `rust-embed` still embeds all 21 language
directories, including the four with no grammar at all (`kotlin`, `lua`, `zig`, `text`). They
are a few KB of `.scm` text with no dependency weight, and gating them would mean patching the
`#[include]` glob — a larger diff for no build-time saving.

### 2 · `arboard` behind a `clipboard` feature

**Files:** `Cargo.toml` (`arboard` → `optional = true`, `clipboard` feature),
`src/editor.rs` (`Editor::set_clipboard` / `get_clipboard`).
**Upstreamable:** yes.

`arboard` was non-optional and links X11/Wayland on Linux, which a bare CI container or a
headless VHS runner does not have. Both functions already carried an in-editor fallback for
the case where `arboard` fails at *runtime* — `self.clipboard`, an `Option<String>` field on
`Editor`. The patch makes that fallback the only path when the feature is off, so behaviour on
a headless machine is what it always was, minus the link-time dependency.

`phosphor-buffer` re-exports the switch as its own `clipboard` feature, and **it is off by
default** — so the headless path is what CI, containers and VHS get for free, and no job has to
remember to turn clipboard support off. (Turning it on is a decision for whoever wires the
paste Action. There is no clipboard Action in the vocabulary yet — `T019` — so nothing would
call it today.) **`just vendor-build-headless`** proves both directions: `arboard` is absent
from the default workspace `cargo tree`, and `--features clipboard` brings it back, so the gate
is live rather than dead.

**What is *not* proven:** that the workspace builds on a machine with no X11 and no Wayland.
The Docker daemon is down in this environment, so `T003`'s container criterion could not be
executed. What we have instead is a stronger structural guarantee — the default build has no
`arboard` node at all — plus the note that `arboard` is taken with `default-features = false`,
whose Linux backend is pure-Rust `x11rb` that dlopens at runtime rather than linking at build
time. Believed fine; unverified.

**`rust-embed` was checked at the same time and left alone.** It is `T003`'s other named
suspect, but it is a proc macro that embeds `langs/*/*` at compile time — no system libraries,
nothing to link, no headless hazard. Not gated.

---

## Known divergence between upstream and what we need — *not yet patched*

Recorded here so the next person reading `vendor-diff` knows what is coming, from the `T008`
spike ([SPIKES.md](../../docs/SPIKES.md)):

- **No soft-wrap anywhere in the crate.** `VisualRow` has no wrapped variant. `T081`, S1 — the
  largest unbudgeted item the spike found, and it will be the first real `phosphor/` module.
- **Marks are `(start, end, Color)` with no id and no style**, replaced wholesale
  (`editor.rs`). Carries region tints; cannot carry the gutter contract or undercurl.
- **The gutter is not injectable** — `set_left_code_padding` reserves cells and we overpaint
  after render. Line-number style is a hardcoded `DarkGray` in `render.rs` and wants a one-line
  patch when the theme lands.
- **Virtual text is absent, but `VisualRow` is the hook** — the renderer iterates visual rows,
  not source lines, and fold separators already insert non-source rows. An enum arm.
- **`mod diff` is private and the diff is a mode of the `Editor`, not a component.** `DiffBody`
  (`T063`) is built on `similar` instead.
- **Upstream's `Action` trait collides by name with phosphor's `Action` enum.** Cosmetic; worth
  a rename in the fork before the two ever appear in one file.
- **`crossterm` is off in our build** (the fork's `crossterm` feature gates
  `editor_crossterm.rs`). Input is phosphor's — `T026` — so we do not want upstream's key
  handling. Revisit only if something in S1 turns out to need that module.
