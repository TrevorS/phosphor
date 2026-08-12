# VENDOR — `ratatui-markdown`

Phosphor's fork of [`ratatui-markdown`](https://github.com/celestia-island/ratatui-markdown),
for the transcript's prose body. Vendored per [plan §2](../../docs/IMPLEMENTATION-PLAN.md) and
[Q4](../../docs/IMPLEMENTATION-PLAN.md#q4), task `T004`.

| | |
|---|---|
| Upstream | `https://github.com/celestia-island/ratatui-markdown` |
| **Last merged** | **`9f4a2c06927859247c1c69ec8cd428facd857e6d`** — tag `v0.3.6`, 2026-05-22 |
| Upstream version at that commit | `0.3.6` |
| Mechanism | `git subtree --squash` into `vendor/ratatui-markdown` |
| Consumed as | workspace `path` dependency, **optional**, `default-features = false` |
| Licence | `MIT OR Apache-2.0` (`Cargo.toml:7`); `LICENSE` is the full Apache-2.0 text |

**A deliberately thinner relationship than the other fork.** This one is vendored for exactly
one reason: it pins `ratatui ^0.29` and the workspace is `0.30`. **We carry a version bump, not
a feature fork.** No phosphor-specific behaviour goes in here. If upstream ships a
0.30-compatible release, the subtree is deleted and replaced by the published crate — so every
patch below is a liability, and there is one.

---

## Licence — no blocker, and a correction

Upstream is **`MIT OR Apache-2.0`** (`Cargo.toml:7`), and `LICENSE` is the standard 201-line
Apache-2.0 text. Both identifiers are already on `deny.toml`'s allow list, and
`cargo deny check licenses` passes with this dependency enabled. Nothing here needs a decision.

**An earlier revision of this file claimed otherwise** — that `LICENSE` was a 16-line "Synthetic
Source License v1.0" granting no rights to the software, and that `Cargo.toml` declared
`SySL-1.0`. All three claims, including the `cargo deny` failure they predicted, were false
against the tree in this directory and were removed at the `CP-0` gate after being checked
against it. Recorded rather than quietly deleted, because a fork's provenance file is only worth
anything if it is known to have been verified.

One genuine imprecision remains, upstream of here: [SPIKES.md](../../docs/SPIKES.md) records
"two vendored forks, both MIT". That is exact for `ratatui-code-editor` and approximate for this
one, which is dual-licensed. Harmless — both arms are allowed — but it is Teej's file to correct.

The dependency is **off by default** for the reason given above: it is a version bump we carry
until upstream ships a 0.30-compatible release, not code we want in every build.

---

## Working on this fork

- **`just vendor-diff ratatui-markdown`** prints everything below and nothing else.
- **Version bump only.** A hunk here that is not the bump or a direct consequence of it does not
  belong in this fork — it belongs in `phosphor-ui`.
- **Excluded from the workspace** (root `Cargo.toml`'s `exclude`) but still a path dependency:
  it builds, and `clippy --workspace -D warnings` stops at the seam. `cargo fmt --all` does
  *not* — it recurses into path dependencies — which is a live `just fmt` failure owned by
  `harness`; see `../ratatui-code-editor/VENDOR.md`. Do not reformat this fork to appease it.
- **Do not build inside this directory.** It has its own `Cargo.lock`, and a local `cargo build`
  rewrites it (~1000 lines, mostly this crate's heavy dev-dependencies: `resvg`, `font-kit`,
  `mermaid-rs-renderer`). That lock is upstream's and is not used by our build — the root
  `Cargo.lock` governs. Build through the workspace instead:
  `cargo build -p phosphor-ui --features markdown`.

---

## Patches

### 1 · `ratatui ^0.29` → `^0.30`

**File:** `Cargo.toml`, one line.
**Upstreamable:** yes — this *is* the upstream fix, and it is the whole fork.

The bump is the entire patch. **No source changes were needed**: the crate compiles clean
against ratatui 0.30 with default features, with the `markdown`-only feature set we consume,
and with `highlight-lang-*` grammars enabled. One pre-existing `unused import` warning
(`prelude::Stylize`) is upstream's and is left alone.

That the diff is one line is the argument for offering it upstream immediately. Until then the
fork exists only to hold that line.

### 2 · Delete `examples/screenshots/`

**Files:** six binary assets removed — `mermaid-image.gif` (7.0 MB), plus `code-highlight.webp`,
`custom-block.webp`, `image.webp`, `mermaid-pure-text.webp` and `tree-view.webp`.
**Upstreamable:** no. Upstream wants its own screenshots; this is a vendoring decision, not a
defect, and it is the one patch here that should *not* be offered back.

7.9 MB of the repository was this directory, and the GIF alone was **80% of the entire packed
object store** — for a fork whose actual content is a one-line version bump. Nothing references
any of it: not the fork's source, not its tests, not its README (which loads the logo from
`raw.githubusercontent.com`), not the translated guides under `docs/guides/`.

Two things were deliberately **kept**, because they are referenced and the goal was weight, not
tidiness: `examples/logo.webp` (98 KB), which every `docs/guides/*/index.md` embeds by relative
path, and `examples/demo.webp` (160 KB), which `src/markdown/render_tests.rs` reads — deleting
that one would break the fork's own test suite.

Expect this to conflict on `just vendor-pull` whenever upstream touches a screenshot. Re-delete
the directory; that is the whole resolution.

---

## Feature gate, and why the shape of it matters

`phosphor-ui` takes this crate as `optional = true`, `default-features = false`,
`features = ["markdown"]`, behind its own `markdown` feature which is **off by default**.

- **[Q4](../../docs/IMPLEMENTATION-PLAN.md#q4)'s guardrail is that the transcript must render
  with the gate on *and* off.** Building the transcript is `S6`/`T054` and not this task's job,
  but the gate is designed not to make it impossible: the dependency is optional at the crate
  boundary, so the plain-text path is a `#[cfg(not(feature = "markdown"))]` arm inside
  `phosphor-ui`, not a second crate. `cargo hack --feature-powerset` is what will prove it.
- **Per-language highlight features stay off.** Upstream ships 37 `highlight-lang-*` features
  plus mermaid, image, tree, preview and viewer extras. Every one enabled is more surface to
  keep compiling across the bump, for a surface the design does not ask for yet.
- **Enabling `markdown` puts full `ratatui` in `phosphor-ui`'s dependency tree**, which the
  `ratatui-core`-only rule (`T002`) otherwise forbids. Same major as the workspace pin — one
  ratatui in the tree, `cargo tree` stays clean — but it is a real relaxation of that rule and
  is flagged to `spine` rather than assumed. It follows from Q4 (vendor this crate) meeting T002
  (`phosphor-ui` gets `ratatui-core` only), and the two decisions do not otherwise meet.
