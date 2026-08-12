# Phosphor — implementation plan

Derived from `docs/design/` (Design Brief, Design Language v0.2, TUI Mockups, Component
Breakdown) per `docs/design/CLAUDE-CODE-HANDOFF.md`. The design docs are the contract; this
document is the route through them.

The twelve questions this plan raised were **answered on 2026-08-11** and are recorded in
[§5 Decisions](#5-decisions). Four of them amend the design docs rather than merely filling a
gap, and are marked as amendments there; **three further amendments came out of `CP-1` and two
out of `CP-2`**, where a running program disagreed with a drawing — and, at `CP-2`, where two
drawings disagreed with each other. Nothing below is an open question any more; where a
decision has consequences for a phase, those are folded into the phase.

---

## 0. Orientation — the five invariants

Everything below is in service of these. If a design decision in review can't be traced to
one, it is probably wrong.

1. **Emacs architecture, literally.** Rust is the C core (rope, tree-sitter, transports,
   store, renderer, input decoder). The editor — keymaps, picker sources, statusline
   segments, permission rules, float layouts — is Steel in `runtime/*.scm`, redefinable from
   a live REPL. Placement test: *"would two reasonable users want this to differ?"* → Steel.
   *"Can this corrupt a buffer or drop a frame?"* → Rust.
2. **One API, three doors.** Steel (in-process), MCP (Claude), CLI (`phosphor --eval`) share
   one Action/query vocabulary over the semantic store. No capability lands in one door only.
3. **Nothing moves unless you asked.** Buffers never update under the cursor; disk changes are
   *indicated* (✱ + offer to refresh), never injected. Enforced in the BufferView wrapper, not
   by convention.
4. **The semantic store is the product.** Regions, seen-state, anchors, threads, watches,
   inbox, review blocks — every surface is a query over it. Unseen markers work on *any* file
   (line-based fallback); node anchoring is a first-class-language enhancement.
5. **No review ceremony.** Seen-tracking + Claude-declared review blocks + VCS-as-safety-net
   is the whole model. No approve/reject, no gates.

Two structural consequences worth building *tests* for, not just intentions:

- `phosphor-ui` must not be able to mutate. Widgets read ViewModels; input maps key →
  `Action`; Actions mutate the store; the store re-derives.
- The three doors must share one registry. If MCP tools are registered by hand alongside a
  separate Steel binding table, invariant 2 rots within a month. Build it so adding a
  capability to one door adds it to all *by construction* (S2).
- **The UI is composed in Steel over a declarative view tree** ([Q12](#q12)). Rust owns the
  primitives and the frame; Steel decides what is on screen. Refined placement test: *does it
  produce pixels? Rust. Does it decide which pixels? Steel.*

---

## 1. Dependency reality check

The Component Breakdown's buy list was verified against crates.io on 2026-08-11, then read
against the source in the M-0 spikes. **Three of its entries did not survive** — two are
unbuildable against the ratatui 0.30 workspace the same document specifies, and one is a
perfectly healthy crate that is simply the wrong fit. Four dependencies the design needs but
never named have been added. One carries far more risk than the doc implies.

| crate | latest | ratatui dep | verdict |
|---|---|---|---|
| `ratatui` | 0.30.2 | — | OK — pin workspace here |
| `ratatui-core` | 0.1.2 | — | OK — `phosphor-ui` depends on this only |
| `ratatui-code-editor` | **0.0.6** | `ratatui-core ~0.1.0` | **RISK** — compatible, but 3.4k downloads, 6 releases, single maintainer. Vendor; see §2 |
| ~~`edtui`~~ | 0.11.6 | `ratatui-core ^0.1` | **DROPPED** — healthy crate, wrong fit; the T009 spike inverted this, see [Q3](#q3) |
| `notify` + `notify-debouncer-full` | 8.2.0 / 0.7.0 | — | **ADDED** — changed-on-disk detection (`1d`); required by the design, listed in no doc |
| `similar` | 3.1.2 | — | **ADDED** — `DiffBody`'s engine now that the bought diff view turns out not to be separable ([Q3](#q3) spike) |
| `async-lsp` | 0.2.4 | — | **ADDED** — maintained LSP client; `tower-lsp` is 2023 and `lsp-types` alone is 2024-06 |
| `etcetera` | 0.11.0 | — | **ADDED** — XDG paths for [Q1](#q1); preferred over `directories` |
| `tui-textarea` | 0.7.0 (2024-10-22) | **`ratatui ^0.29`** | **BROKEN** — stale 2 yrs, incompatible with 0.30 |
| `ratatui-textarea` | 0.9.2 | `ratatui-core ^0.1.1` | **USE THIS** — the maintained fork |
| `ratatui-markdown` | 0.3.6 | **`ratatui ^0.29`** | **VENDOR + PATCH** — unbuildable as published; forked and bumped to 0.30, see [Q4](#q4) |
| `nucleo` | 0.5.0 | none (pure matcher) | OK — no ratatui coupling; helix's engine |
| `tui-tree-widget` | 0.24.1 | `ratatui-core ^0.1.0` | OK |
| `throbber-widgets-tui` | 0.11.1 | `ratatui ^0.30` | OK |
| `ratatui-comfy-tabs` | 0.5.12 | `ratatui-core ^0.1.2` | 600 downloads — **build TabBar instead** (`T089`, S6), the doc already allows it |
| `ratatui-explorer` | 0.3.0 | `ratatui ^0.30` | study only; nucleo + Picker likely covers it |
| `tui-logger` | 0.18.3 | `ratatui ^0.30` | OK — dev-only |
| `steel-core` | 0.8.2 (2026-02-22) | — | **PIN `=0.8.2`** — pre-1.0, small, load-bearing; see [Q5](#q5) |
| `ropey` | 1.6.1 | — | OK |
| `tree-sitter` | 0.26.12 | — | OK |
| `crossterm` | 0.29.0 | — | OK |
| `agent-client-protocol` | 2.0.0 | — | OK — **the ACP transport exists**, 3.6M downloads |
| `rmcp` | 3.1.2 | — | OK — **official MCP Rust SDK**, 19.7M downloads |

**Actions from this table:**

- The Component Breakdown lists `tui-textarea / ratatui-textarea` as one row. They are *not*
  interchangeable in 2026: `tui-textarea` is two years stale and pins ratatui 0.29. The
  handoff's settled-decisions list already says `ratatui-textarea` — that list is correct, the
  Component Breakdown row is loose. **Resolved in favour of `ratatui-textarea`; no user
  decision needed.**
- `agent-client-protocol` and `rmcp` were not named in the docs but are the transports for the
  ACP session and the MCP editor-tool server. **Confirmed** — see [Q6](#q6).
- `ratatui-markdown` is a **second vendored fork** ([Q4](#q4)), so §2's fork discipline applies
  to two crates, not one.
- **The M-0 spikes have run.** [`SPIKES.md`](SPIKES.md) carries the findings with `file:line`
  citations; the rows above marked DROPPED and ADDED are their consequences. The full verified
  manifest and the hygiene tooling live there rather than being duplicated here.
- `steel-core`'s `dylibs` / `dylib-build` features are what make `define-language`'s
  "loadable as dylibs" story real. Confirmed available.

---

## 2. Vendored-fork strategy

Two crates are vendored: `ratatui-code-editor` (the BufferView core) and `ratatui-markdown`
(transcript prose, [Q4](#q4)). The mechanics below are written for the first and applied to
both; the second is a much smaller carry — a version bump and whatever follows from it, with
no phosphor-specific behaviour layered on.

### `ratatui-code-editor`

The handoff calls this out as "the one dependency we must be able to patch same-day," and the
crates.io data makes the case stronger than the doc does: **v0.0.6, six releases since
2025-10-16, ~3.4k lifetime downloads, one maintainer.** The entire central surface of the
editor rests on it. Treat it as *our code that happens to have an upstream*.

#### Mechanics

- **Vendor by `git subtree`** into `vendor/ratatui-code-editor`, consumed as a workspace
  `path` dependency. Not a submodule (breaks single-clone workflow, adds a fetch step to every
  CI job) and not a `[patch.crates-io]` git fork (patching still requires a push–pull round
  trip before a local build sees the fix). A subtree lets a same-day patch be a normal commit
  in this repo.
- **`vendor/ratatui-code-editor/VENDOR.md`** records the upstream tag/SHA we last merged, and
  one entry per local patch — what changed, why, and whether it is upstreamable.
- **Minimal-diff discipline.** Phosphor additions live in a `phosphor/` module inside the
  fork, with the smallest possible edits at the seams to call into it. Upstream merges then
  conflict in a handful of known places rather than everywhere.
- **`just vendor-diff`** shows divergence from the last-merged upstream tag; **`just
  vendor-pull <tag>`** performs the subtree merge. Divergence being visible in one command is
  what keeps the fork from silently becoming a rewrite.
- **Upstreaming policy:** anything not phosphor-specific (bug fixes, perf, API generalisation)
  goes upstream as a PR. Phosphor-specific behaviour (the 3-column gutter contract, region
  tints, virtual-text interleaving) stays local, permanently.

#### The seams we need from it — spike results

The T008 spike read these against the published source. Full citations in
[`SPIKES.md`](SPIKES.md); the verdicts:

| Seam | Verdict |
|---|---|
| **Marks API** | **Partial.** Exists as `(start, end, Color)` with no id, no style, and wholesale replacement only (`editor.rs:660-682`). Carries region tints; **cannot** carry the gutter contract or undercurl. |
| **Gutter column injection** | **Not injectable — compose-around works.** `set_left_code_padding` reserves cells (`editor.rs:1009`); we overpaint the state column into the same `Buffer` after the widget renders. Line-number style is hardcoded `DarkGray` (`render.rs:33`) and wants a one-line patch. |
| **Virtual-text rows** | **Absent, but the hook is already there.** The renderer iterates `VisualRow`, not source lines (`render.rs:57`), and fold separators and diff ghosts already insert non-source rows (`types.rs:20-36`). A `Virtual` variant plus a render arm — an enum arm, not an architecture change. |
| **Scroll authority** | **Clean.** `fit_cursor()` is called from exactly two explicit places (`editor.rs:143`, `509`). **The widget does not self-scroll on render**, so invariant 3 is enforceable by not calling them. |
| **Diff view** | **Not separable.** `mod diff` is private (`lib.rs:4`); the diff is a *mode of the Editor*, not a component. `DiffBody` loses its bought base and is built on `similar` instead. |

**Plus one thing nobody asked about: there is no soft-wrap in the crate at all.** `VisualRow` has
no wrapped variant and no wrapping logic exists anywhere. Our design requires `↪` continuations
without line numbers. This is the largest unbudgeted item the spike found, and it lands in S1.

**Verdict: vendor it, with a larger fork than planned.** The reassurance is the size — 4,936
lines total, of which the renderer is 281. That is small enough to own with confidence. The risk
was never that the fork would be unmaintainable; it was that the design doc credited the crate
with capabilities it does not have.

**The fallback is not triggered.** `BufferView` stays on the vendored core rather than being
rebuilt on `ropey` + `tree-sitter` directly.

### `ratatui-markdown`

Vendored for one reason: it pins `ratatui ^0.29` and the workspace is 0.30 ([Q4](#q4)). Same
subtree, same `VENDOR.md`, same `just vendor-*` recipes — but a deliberately thinner
relationship. **We carry a version bump, not a feature fork.** No phosphor-specific behaviour
goes into it; if upstream ever ships a 0.30-compatible release the subtree is replaced by the
published crate and the fork is deleted.

Two guardrails, since this is a non-core surface carrying maintenance cost:

- **Feature-gate it anyway.** The transcript must still render readably with the gate off, so a
  broken bump is a degraded surface rather than a broken build. The plain-text path is the
  fallback that keeps the fork optional.
- **Keep the language features off.** The crate ships per-language highlight features
  (`highlight-lang-*`) and mermaid/JSON-tree extras; enable only what the transcript actually
  needs. Every enabled feature is more surface to keep compiling across the bump.

---

## 3. Phased plan

`M-0` is scaffolding (the handoff's "crate scaffolding first"). `S1`–`S8` are the Component
Breakdown's build order, unchanged in order and intent, with acceptance criteria attached.

Screen ids (`1a`, `2a`, …) refer to `docs/design/TUI Mockups.dc.html`.

---

### M-0 · Scaffolding and the structural tests

**Goal:** an empty editor that already cannot violate the architecture.

> **The two dependency spikes are done** — see [SPIKES.md](SPIKES.md). What remains in M-0 is
> construction, plus one unresolved check (grammar ABI, below).

- Cargo workspace with the units from the Component Breakdown: `phosphor`, `phosphor-core`,
  `phosphor-buffer`, `phosphor-ui`, `phosphor-agent`, `phosphor-steel`, `phosphor-vcs`, plus
  `runtime/` (not a crate — the Steel source tree).
- Pin `ratatui 0.30.2` / `ratatui-core 0.1.2` at the workspace root. `phosphor-ui` depends on
  `ratatui-core` only.
- **Both vendored subtrees** — `vendor/ratatui-code-editor` and `vendor/ratatui-markdown` — each
  with its own `VENDOR.md`, plus the shared `just vendor-*` recipes (§2).
- **The grammar ABI check** (`T083`). Load all eleven grammars we ship — the first-class twelve
  minus CSV, which `T082` implements by hand — against tree-sitter 0.26 and parse a fixture;
  settle whether `tree-sitter-scheme` handles real Steel. The grammar crates were
  built against bindings spanning 0.23–0.25, and tree-sitter versions its language ABI. Cheap
  here, expensive at S4 — **this is the one unknown the spikes surfaced but did not close.**
- CI: `fmt`, `clippy -D warnings`, `test`, and two structural lints that encode the invariants:
  - **no literal colours in `phosphor-ui`** — every widget takes `&Theme` (Design Language
    §12). A grep-level lint over `Color::Rgb` / `Color::Indexed` in that crate is enough.
  - **no store mutation from `phosphor-ui`** — enforced by dependency direction: `phosphor-ui`
    sees ViewModel and view-tree types only, never the store's `&mut` API. Model it as separate
    modules (`phosphor_core::vm` and `phosphor_core::view` vs `phosphor_core::store`), with the
    widget crate importing only the first two ([Q12](#q12)).

**Scope**
- Files: 7 `Cargo.toml` + 7 `lib.rs`/`main.rs` stubs, 1 `justfile`, 1 CI workflow,
  2 `VENDOR.md`, 1 `rust-toolchain.toml`, 1 `deny.toml`
- Named units: 2 structural CI lints, 2 vendored subtrees, 1 grammar ABI check, the hygiene
  tooling ([SPIKES.md](SPIKES.md))
- Verification: CI green on an empty workspace; both lints fail on planted violations; all
  eleven grammars parse a fixture under tree-sitter 0.26
- Risk: public API no · data migration no · cross-module no · reversible yes · external
  blocker no

**Done when:** `cargo build` is green, both structural lints run in CI and fail on planted
violations, `cargo-deny` rejects a duplicate `ratatui` major, and every bundled grammar parses
a fixture under tree-sitter 0.26.

---

### S1 · Theme + BufferView + StatusLine shell

**Goal:** *"phosphor renders and edits a file with highlighting on day one; feels like the
mockups."*

> **"Renders and edits a file" needs a program, and the first pass at this phase did not build
> one** — `T090` exists because of it. Every widget below landed, tested and lint-clean, around a
> `main.rs` that was still `fn main() {}`, so `CP-1`'s own `cargo run --` line drew nothing and
> the checkpoint's manual half could not happen at all. The host is thin and disposable — it
> rides the fork's `editor_crossterm` handler and `T026` deletes it — but without it S1 is a
> library, not a day one.

- `Theme`: actor/state palette struct (`claude, you, attention, trouble, transient, steel`) +
  neutral ramp + syntax map. base16-style loading. **Actor-hue validation at load** — a theme
  reassigning actor hues is rejected, not themed (Design Language §10).
- Phosphor dark + light built in. **Catppuccin and Tokyo Night as the first two mappings**
  ([Q7](#q7) — Ayu is dropped; this amends the Design Brief).
- `BufferView` over the vendored editor: 3-column contract (1-cell state bar → line numbers →
  text) via padding-reserve plus overpaint, fold rows.
- **Soft-wrap — unbudgeted, and it lands here.** The vendored crate has none, so we own `↪`
  continuations outright. This is the largest surprise from the T008 spike, and it is not a
  contained one: soft-wrap touches row↔line mapping, cursor positioning, click targeting and
  virtual-text placement at the same time. Build it against `VisualRow` alongside the fold and
  ghost variants rather than layering it on afterwards.
- `StatusLine` shell: mode chip, file + dirty flag, spring, `SessionState` (rendering `None`
  for now), counters. **Truncation enforced in the widget** — never wraps, a second line is a
  bug.
- **`Float` — the one chrome primitive** (`T084`, added by the docs review). Header / body /
  footer, mood borders, the one-float rule, dim-behind, full-width under 100 cols. It lands here
  rather than at S4 because `T021` needs a float to show a broken `init.scm` in at **S2**, two
  phases before the only float task the first breakdown had (`T038`, the passive variant). Every
  later body — Picker, DiffBody, QuestionBody, HelpGrid, ArchDiagram — plugs into it.
- **Undercurl, with underline fallback** (`T085`, added by the docs review). The marks API is
  colour-only, so the undercurl half of Design Language §3's anchored-region treatment is fork
  work we own. Landing it here lets `V002` settle "does undercurl survive VHS capture" against a
  real implementation rather than a guess.
- Synchronized output wrapping every frame from the first draw call. A torn frame is P0;
  retrofitting this later means auditing every render path.

> **What drives editing at S1.** The goal is *"renders and edits a file on day one,"* but the
> input machine is S3. S1 rides the vendored crate's own `editor_crossterm` handler as a
> temporary path — which is what makes `T081`'s cursor and click checks possible at `CP-1`.
> `T026` replaces it outright; nothing may grow to depend on it.

**Acceptance:** `1a` reproducible in a real terminal *minus the agent layer* (no gutter
markers, no review-ready float, statusline session segment reads idle-empty) · `9c` (phosphor
original) and `8c` (light) render the same slice with the actor contract intact · `8d`
(80 columns) — the statusline sheds right-to-left and nothing wraps.

> **Amendment:** `9a` (Catppuccin) is an acceptance target for this step's theme system but
> appears in no build step in the docs; assigned here. `9b` (Ayu) is **superseded** — Ayu's
> identity colour is orange, which the language reserves for attention, and the actor contract
> wins ([Q7](#q7)). **Tokyo Night replaces it**, chosen because blue-violet collides with no
> actor hue and Tokyo Night Day is a real light variant rather than an afterthought. There is
> no mockup for it; `9b` stands as the *shape* of the acceptance test — same slice of UI, a
> second palette, actor contract intact — with a different palette substituted.

**Scope**
- Files: `phosphor-ui/{theme,buffer_view,status_line,float,soft_wrap}.rs`,
  `phosphor-buffer/{rope,ts}.rs`, 3 theme files, plus the vendored fork's `VisualRow` and
  undercurl patches
- Named units: 4 widgets, 2 built-in themes + 2 mappings (each dark + light), 1 theme
  validator, 1 synchronized-output wrapper, **1 soft-wrap implementation** (unbudgeted — the
  vendored crate has none), **1 float primitive + 1 undercurl patch** (added by the docs review —
  neither had a task)
- Verification: golden-frame snapshot tests per screen id + a manual terminal pass at 80 and
  120 columns + wrapped-line correctness for cursor motion and click targeting (virtual text on
  a wrapped line is checked at `CP-3`, where `VirtualText` exists) + undercurl on the primary
  terminal and underline on the degradation terminal
- Risk: public API no · data migration no · cross-module **yes — soft-wrap reaches cursor,
  click targeting and virtual text** · reversible yes · external blocker no

---

### S2 · Steel runtime + Action/query bindings + REPL — *the spine*

**Goal:** the milestone the handoff names explicitly. *From here on, every keymap, segment,
and picker source lands in `runtime/*.scm`, not Rust.*

This step defines the `Action` enum and the query vocabulary — the decision the rest of the
build is hardest to reverse. Get the vocabulary right and invariants 1, 2 and 4 hold
themselves up.

- Embed `steel-core`, pinned **`=0.8.2`** exactly, not caret ([Q5](#q5)) — a Steel upgrade is
  its own scheduled task gated by the door-parity test, so 0.x embedding-API churn can never
  arrive unannounced in an unrelated build. Boot sequence: `init.scm` is just the REPL session
  that runs at boot.
- **`Action` enum** — the single mutation API. Buffer edits, seen marks, session messages,
  float open/close. Everything that changes state is one of these.
- **Query vocabulary** — the read side, over the store's ViewModels.
- **The view-tree protocol** ([Q12](#q12)) — the second half of the spine, and as load-bearing
  as the `Action` enum. `phosphor_core::view` defines the tree as plain data (no Steel dep, no
  ratatui dep); `phosphor-steel` produces it; `phosphor-ui` interprets it into ratatui calls.
  Rust caches the last tree and redraws every frame without re-entering the VM, so Steel runs at
  the rate of state change rather than the rate of frames.
- **One registry, three doors.** Register each Action/query *once*, with its Steel binding,
  MCP tool schema, and CLI verb derived from that single registration. This is the mechanical
  guarantee for invariant 2; the MCP door has no consumer until S6, but it is generated from
  day one so it cannot drift.
- **REPL** as the primary extension workflow: `(keymap-set! …)` is live and which-key knows
  it; redefining a picker source re-derives an open picker.
- **Broken `init.scm` boots the editor anyway**, with the error in a float. Safety comes from
  the barrier — Steel emits Actions and reads ViewModels, so live redefinition can
  misconfigure but never corrupt a buffer.
- `phosphor --eval` (the CLI door) ships here, since it is nearly free once the registry
  exists.

**Acceptance:** `6b` (Steel REPL) reproducible · a keybinding redefined at the REPL takes
effect on the next frame without restart · `phosphor --eval '(…)'` and the in-process REPL
produce identical results for the same expression · a deliberately broken `init.scm` boots
with the error in a float.

**Scope**
- Files: `phosphor-steel/{runtime,registry,repl,bindings}.rs`, `phosphor-core/action.rs`,
  `runtime/init.scm`
- Named units: 1 `Action` enum, 1 tri-door registry, 1 REPL, 1 CLI eval path, ~2 seed `.scm`
- Verification: door-parity test (every registered Action present in all three doors — a test
  that *enumerates the registry*, so it cannot be forgotten). **What "present" means before S6:**
  the Steel binding and CLI verb are invoked end to end, the MCP tool schema is generated and
  well-formed. The MCP server itself is S6; `T052` upgrades that third to a live round-trip
  without changing the test's shape. Plus the REPL liveness test
- Risk: public API **yes — this is the public API** · data migration no · cross-module **yes
  (every crate above `phosphor-core`)** · reversible **no in practice** · external blocker no

---

### S3 · Input + persistent undo + gutter/virtual-text layer

**Goal:** plain editor complete.

- **The input machine is ours** ([Q3](#q3), resolved by the T009 spike). Modes, counts,
  registers, operator-pending, text objects — built against the `Action` layer, over
  Steel-defined keymaps. **Counts and named registers are designed in from the start**, since
  they are precisely what the bought option could not express. Diff our verb/object coverage
  against edtui's `Action` enum before `CP-3`: a good completeness checklist even though we no
  longer depend on the crate.
- Agent nouns registered as custom text objects: `viu`, `sib`, `dih`, `:'<,'>c`.
- **Persistent undo** on disk, surviving restarts. **`phosphor-buffer` owns the undo model,
  `phosphor-core` owns persistence** ([Q2](#q2)) — `phosphor-core` already owns the on-disk
  story for seen-state, and the two share one file format and one compaction path.
- The gutter/virtual-text layer: `GutterBar` (1-cell state column, priority trouble >
  attention > claude-unseen > none, `▎` degradation) and `VirtualText` (`┊`-prefixed rows
  owned by a region id).
- Keymaps and the leader tree in Steel; `KeymapFooter` / WhichKey reads the *live* keymap, and
  **`HelpGrid`** — the `:help` float body, same data at a third density (`T086`, added by the
  docs review; `6d` was an acceptance target here with no task behind it).
- The once-per-session unknown-key hint.

**Acceptance:** `3c` (leader popup) · `6d` (`:help agent-objects`) renders from the live
keymap · `8e` (first keystroke teaches once; folds and insert-only whitespace marks — `8e`'s
soft-wrap continuations are S1's, and are already passing by the time this step is assessed).

> **Flag — two acceptance targets in the docs cannot be met at this step.**
> The build order lists `7c` here ("plain editor complete: 7c"), but `7c` is *"lsp completion
> + signature help"* and LSP is S4. **Retargeted:** `7c`-minus-completion here, full `7c` at S4.
> Similarly `6d` displays the agent nouns, but `viu` ("inside unseen") cannot *resolve*
> without the semantic store (S5). **Retargeted:** the help surface and grammar render here;
> the nouns become functional at S5. See [Q8](#q8).

**Scope**
- Files: `phosphor-ui/{gutter,virtual_text,keymap_footer,help_grid}.rs`,
  `phosphor-buffer/undo.rs`, `phosphor/input.rs`, `runtime/{keymaps,leader}.scm`
- Named units: 4 widgets, 4 agent text objects, 1 persistent-undo store, **1 input machine**
  (ours — modes, counts, named registers, operator-pending)
- Verification: text-object unit tests against real source files; undo round-trip across a
  simulated restart
- Risk: public API no · data migration **yes — the undo file format is on disk from here** ·
  cross-module no · reversible yes · external blocker no

---

### S4 · LSP + completion float

**Goal:** the editing experience is complete and boring on purpose. Highlighting was already
bought at S1, so this step is diagnostics, completion, signature help, and hover.

- LSP client state in `phosphor-buffer`; blessed server auto-configured per first-class
  language (not merely discovered).
- Completion via the **passive** Float variant (`#2a3c2e` border, **no footer** — the one
  documented exception to the float contract).
- Diagnostics feed `GutterBar` at trouble priority and `VirtualText` as `■` rows.
- **`define-language`** lands here: the first-class set ships *as `define-language` calls in
  `runtime/`*, not as Rust tables. TS, JS, Rust, Python, Steel, Markdown, JSON, CSV, TOML,
  YAML, HTML, CSS.
  - **CSV is not tree-sitter.** The only grammar crate is 2.5 years stale with ~5k downloads,
    and CSV gets a hand-tuned surface (virtual column alignment) rather than generic buffer
    treatment. A small parser is more reliable than a stale grammar and yields exactly the
    column model that surface needs.
  - **Steel uses `tree-sitter-scheme`** (0.24.7) — verify it parses real `runtime/*.scm`
    before committing to it. Steel is a Scheme dialect, not Scheme.
  - **Grammar ABI compatibility is checked in M-0, not here.** The crates were built against
    tree-sitter bindings spanning 0.23–0.25 while the runtime is 0.26; S4 assumes that check
    already passed.

> **Flag:** `define-language` is required for v1 (the Component Breakdown puts the
> first-class-set declarations in `runtime/`) but appears in no build step. Assigned here,
> because it binds grammar + LSP command + locale hooks and both halves exist by S4.

**Acceptance:** `7c` in full — LSP completion and signature help, no agent anywhere.

**Scope**
- Files: `phosphor-buffer/lsp.rs`, `phosphor-ui/float.rs` (passive variant),
  `runtime/languages/*.scm` (12 files)
- Named units: 1 LSP client, 1 passive float body, 12 `define-language` declarations
- Verification: completion + signature help exercised against 3 real servers (rust-analyzer,
  tsserver, pyright)
- Risk: public API no · data migration no · cross-module no · reversible yes · external
  blocker no

---

### S5 · Semantic store + seen-tracking + Picker — *the awareness loop*

**Goal:** the first half of the product. This is where Phosphor stops being an editor.

- **The semantic store** in `phosphor-core`: regions, seen-state, node anchors, threads,
  watches, inbox, review blocks. Every surface downstream is a query over it.
- **Region lifecycle** — the one state machine: `claude writes → unseen --s--> seen`, and
  `claude revises → unseen again`. Seen-state is the only mutable flag the user owns;
  everything else is derived. Overlays (`⚓` thread, `◉` watch, `■` diagnostic) are orthogonal
  and bind to the node, not the state. **Your own edits never create regions.**
- **Anchoring, two tiers** (invariant 4): tree-sitter node anchors for first-class languages;
  **line + content matching as the fallback so markers work on any file at all.** The fallback
  is not a degraded extra — it is what makes the store a store feature rather than a language
  feature.
- **Seen-state persistence** ([Q1](#q1)): **out-of-tree, keyed on the canonicalised workspace
  root path** — `$XDG_STATE_HOME/phosphor/<hash-of-root>/`, never VCS identity. This is what
  makes "no VCS required, ever" literally true: the same code path serves a jj repo and a bare
  directory, with no second keying mode and no migration when a directory later becomes a repo.
  It also keeps phosphor from ever dirtying the user's own VCS. Format is an **append-only log
  with periodic compaction**, so a crash mid-session loses at most the tail. Shares its format
  and compaction path with persistent undo ([Q2](#q2)).
  - *Accepted cost:* seen-state does not travel with the checkout — a fresh clone or a moved
    directory starts with everything unseen. That is the honest failure mode (nothing is lost,
    only re-shown), and it is the right one to accept for a per-user, per-machine reading log.
- **Region tints, through the marks API** (`T087`, added by the docs review). The one seam the
  T008 spike found the bought marks API genuinely good for, and nothing was tasked to build it.
  Three constraints come straight from the spike: marks carry **no id**, so region ↔ mark mapping
  needs our own side table keyed by offset range; `set_marks` **replaces wholesale**, so every
  seen-state change re-uploads the full set and wants diffing before upload; and the state column
  and undercurl are **not** marks — they resolve separately and compose per row.
- `Picker` on the `nucleo` engine: filter line (`ratatui-textarea`) + off-thread matcher +
  list + preview split (dropped under 100 cols). **Sources are Steel** —
  `(define-picker-source …)`, so adding one is userspace.
- Picker sources shipped here: unseen regions, files (with agent-activity columns).
- **`:arch` (`6a`)** ([Q11](#q11)): an `ArchDiagram` float body over a store query. Cheap once
  the store exists, and it is what turns "every surface is a query over one store" from an
  assertion into something you can look at.

**Acceptance:** `1a` **in full** (unseen gutter markers + review-ready float + statusline
counters) · `2a` (review-block picker with diff preview) · `3d` (files picker carrying unseen
counts and activity) · `8a` (grep with agent context — same picker anatomy) · `6a` (`:arch`) ·
unseen markers demonstrably working on a file type with **no** tree-sitter grammar · a picker
source added live from the REPL appears without restart · seen-state survives a restart *and* a
`kill -9` mid-session.

> **Flag:** `8a` (search picker), `6c` (anchors survive a rewrite) and `6a` (`:arch`) are
> unassigned in the docs. `8a` assigned here — it is the same Picker with a different Steel
> source. `6c` is the *proof* of node anchoring and is assigned here as an acceptance test
> rather than a feature. `6a` is in v1 by decision ([Q11](#q11)).

**Scope**
- Files: `phosphor-core/{store,region,anchor,seen}.rs`, `phosphor-ui/{picker,buffer_view}.rs`,
  `runtime/pickers/*.scm`
- Named units: 1 store, 1 region state machine, 2 anchor strategies, 1 Picker widget, 3 Steel
  picker sources, 1 `ArchDiagram` body, **1 marks side table** (added by the docs review)
- Verification: anchor-survival tests (apply a real refactor, assert threads/seen/watches
  follow); line-fallback tests on an extensionless file; restart- and crash-persistence tests
- Risk: public API no · data migration **yes — seen-state on disk** · cross-module **yes** ·
  reversible yes · external blocker no

---

### S6 · ACP + MCP + Transcript + Prompt — *the directing loop*

**Goal:** the second half of the product. Claude is now in the editor.

- **ACP session client** (`agent-client-protocol`) — one Claude Code session per editor per
  repo in v1.
- **MCP server** (`rmcp`) exposing editor tools to Claude, generated from the S2 registry so the
  vocabulary matches the other doors by construction. **Review blocks are an MCP tool call**
  ([Q6](#q6)) — `phosphor/declare-review-block`, carrying a file+range list and per-group
  annotations. It is an editor-facing capability, which is exactly what the docs reserve MCP
  for, and routing it through the registry means Steel and the CLI can declare one too.
- **The pane manager and `TabBar`** (`T088`, `T089`, added by the docs review). The transcript is
  described everywhere as *"a pane, not a float — it splits, holds focus like a window, and
  survives float churn,"* and nothing provided panes; the tab bar is one of Design Language §5's
  three chrome strips and appears the moment a second pane does. Both gate `TranscriptPane`, so
  they run at the front of this step. The split — **panes are focus and event routing in the
  binary's loop, `TabBar` is a widget over them** — is the same one input already uses.
- `TranscriptPane` — **a pane, not a float**: turn list, prompt lines (`❯`), prose, tool rows
  (`▸ verb file ±counts`, OSC 8 jump links), seam markers (`⏸` paused, `✕` lost). Folds by
  turn at scale. Streams during Working. Prose renders through the **vendored `ratatui-markdown`**
  ([Q4](#q4)), feature-gated, with the plain-text path as the fallback that keeps the fork
  optional.
- `PromptLine` — the `:` line; `⚓` anchor chip when a selection rides along; routes to command
  parse or Claude message. **Selections anchor automatically** — visual-select, hit the
  prompt, file and range ride along.
- `QuestionBody` — needs-input and permission asks; amber digit options `[1]`–`[n]`;
  always-allow **writes a legible rule to `init.scm`**.
  - **Asks are queued, never barged in** ([Q9](#q9)). When a question arrives while another
    float holds focus, it sets the statusline `!` flag immediately and waits; the float surfaces
    once no other float has focus, and `]!` jumps to a pending ask. This keeps both design rules
    literally true — the one-float rule is never broken, and nothing is destroyed under the
    user. *Accepted cost:* an ask can sit unnoticed, so the statusline flag is not optional
    chrome — it is the whole notification, and it must survive statusline shedding at narrow
    widths (`✻`/`●n`/`!` are the last things standing).
  - The **queue is a store query**, not widget state — pending asks are a ViewModel like
    everything else, which is what lets `]!`, the inbox, and the statusline all read the same
    truth (invariant 4).
- `SessionState` becomes real: Idle, Working{elapsed}, Waiting, Paused, Lost, None — one enum,
  rendered identically everywhere.
- `esc` pauses at the next tool boundary → steer / resume / abort, and the seam is recorded.

**Acceptance:** `1b` (transcript summoned as a pane) · `1c` (`:`-prompt with selection anchor)
· `4a` (Claude needs input mid-turn) · `7a` (permission ask, exact invocation shown,
always-allow writes a rule) · `7e` (interrupt & steer) · `2c` (mid-turn live — buffer
untouched while Claude works, zero tearing) · `7b` (session dropped — editing never blocks) ·
`7d` (cold start) · `5d` (attach, adopt, or start) · `2d` (opening mid-task dashboard).

> **Flag:** the docs name only `1b, 1c, 4a, 7a, 7e` for this step, but `2c, 7b, 7d, 5d, 2d`
> are all session-lifecycle screens with no other possible home. Assigned here. This makes S6
> the largest step by surface count (10 screens).

**Internal checkpoint** ([Q10](#q10)). S6 is reviewed in two halves rather than renumbered:

1. **Session attaches and streams** — ACP client, MCP server, `SessionState`, `TranscriptPane`,
   and the lifecycle screens (`2c`, `7b`, `7d`, `5d`, `2d`). Shippable on its own: Claude is
   visible in the editor.
2. **Directing** — `PromptLine`, `QuestionBody`, the ask queue, permissions, interrupt-and-steer
   (`1b`, `1c`, `4a`, `7a`, `7e`).

**Scope**
- Files: `phosphor-agent/{acp,mcp,transcript,session}.rs`,
  `phosphor-ui/{transcript,prompt_line,question,tab_bar}.rs`, `phosphor/panes.rs`,
  `runtime/permissions.scm`
- Named units: 1 ACP client, 1 MCP server, 4 widgets, **1 pane manager** (added by the docs
  review), 1 `SessionState` enum, 10 screens
- Verification: session-lifecycle tests (drop mid-turn, reattach, adopt); a torn-frame check
  under streaming load; permission rules round-trip to `init.scm`
- Risk: public API no · data migration no · cross-module **yes** · reversible yes · external
  blocker no

---

### S7 · Diffs, review blocks, inbox, dirty-state, VCS

**Goal:** the surfaces that make a review block readable and a disk conflict honest.

Three workstreams the docs bundle into one step (see [Q10](#q10)):

**S7.1 — Review surfaces.** `DiffBody` — **built on `similar` and our own body**, not on a bought
widget: the T008 spike found the vendored crate's diff is a *mode of the Editor* rather than a
separable component (`mod diff` is private), so there is nothing to restyle. Since `DiffBody`
needs per-hunk seen state, directory grouping and Claude's annotations anyway, driving the
Editor's diff mode would have fought us harder than owning it. `similar` is already a
transitive dependency of the vendored crate, so this costs no new dependency. Plus review
blocks, and the inbox — one list of everything Claude said, severity as a single MCP flag,
unread = unseen.
→ `2b` (hunk peek), `4b` (block diff), `5c` (inbox), `8b` (the 40-file block: grouping, not
scrolling).

**S7.2 — Dirty state.** The changed-underneath indicator (`✱`) and offer to refresh;
`:diff-disk` with its three-exit footer and **no auto-merge**. This is invariant 3 at its
sharpest. Watching disk is `notify` + `notify-debouncer-full` — a dependency the design requires
and no document listed until the spike. Debouncing is load-bearing: an agent writing a file
produces a burst of events, and one `✱` per burst is the honest signal. → `1d`, `5b`.

**S7.3 — VCS.** `phosphor-vcs`: jj first, git second, both behind a trait, compiled in and
activated on detection. **No feature may assume a repo exists** — the adapter's absence is a
normal state, not an error path. → `3b` (jj timeline: agent turns are changes, undo is time
travel).

**Acceptance:** `2b` · `4b` · `5c` · `8b` · `1d` · `5b` · `3b` · `3a` (anchored exchange: your
comment and Claude's reply as virtual text under the region) · **every one of the above also
passing in a directory with no VCS at all.**

> **Flag:** `3a` is unassigned in the docs; it is the visible form of a thread overlay and
> lands with the review surfaces. Assigned to S7.1.

**Scope**
- Files: `phosphor-ui/diff_body.rs`, `phosphor-core/{review,inbox}.rs`, `phosphor-vcs/*`,
  `runtime/inbox.scm`
- Named units: 1 diff widget, 1 review-block model, 1 inbox, 2 VCS adapters, 8 screens
- Verification: the full acceptance set run twice — once in a jj repo, once in a bare directory
- Risk: public API no · data migration no · cross-module **yes** · reversible yes · external
  blocker no

---

### S8 · Watches

**Goal:** the Light Table inheritance, agent-powered. Last, because it rides on everything
above — the store (S5), the session (S6), and virtual text (S3).

- `WatchOverlay`: `◉ ⇒` value sequences with a run-provenance line, rendered through
  `BufferView`'s virtual-text rows. **This widget only formats** — values arrive over the
  session from real executions.
- **Values stream as ACP session notifications** ([Q6](#q6)), not MCP tool calls: they arrive
  continuously during a turn and are session state rather than an editor mutation, so the
  request/response shape of a tool call fits them badly.
- `(watch-place …)` from the REPL sprouts virtual text in the buffer.
- Watches are first-class-language only (they need node anchoring); second tier does not get
  them, and says so honestly.

**Acceptance:** `5a` (live values from real runs as virtual text).

**Scope**
- Files: `phosphor-ui/watch_overlay.rs`, `phosphor-core/watch.rs`, `runtime/watch.scm`
- Named units: 1 widget, 1 watch model, 1 Steel entry point
- Verification: values from a real `cargo test` / `pytest` run streaming into a buffer
- Risk: public API no · data migration no · cross-module no · reversible yes · external
  blocker no

---

### Cross-cutting, every step

These are not phases; they are checks that run at each phase boundary:

- **`8d` (80 columns)** — drop, never squeeze. Statusline shed order: counters → jj → cursor
  pos → session prose (glyph stays) → mode word (initial stays). `✻` / `●n` / `!` are the last
  things standing — the ask flag is load-bearing, not chrome ([Q9](#q9)). Pickers lose the preview split under 100 cols; floats go full-width.
- **Degradation** — markers → `▎`, undercurl → underline, spinner → static `✻` on dumb
  terminals.
- **Torn frames are P0** — synchronized output wraps every frame, checked whenever a new async
  source starts posting events.
- **Voice** — lowercase, telegraphic, factual. Counts, not adjectives. Keyhints spell the whole
  command (`:reattach`, never `:ca`).

### Deferred past v1

`4c` (the pane Claude built) and `4d` (tmux control mode) are v1.5 — except `4d`'s "coexists
politely with your panes," which S1 covers by being tmux-friendly rather than tmux-native.

`6a` (`:arch`) is **not** on this list: [Q11](#q11) put it in v1, at S5.

---

## 4. Screen coverage

The Component Breakdown's build order names **18 of the 37 mockup screens** as acceptance
targets. The remaining 19 had no assigned home. This plan places **16 of them into build
steps**, leaving **34 of 37 screens built in v1**:

| step | from the docs | added by this plan |
|---|---|---|
| S1 | — | `9c`, `8c`, `8d`, `9a` |
| S2 | — | `6b` |
| S3 | `3c`, `6d`, (`7c` → S4) | `8e` |
| S4 | — | `7c` (moved from S3) |
| S5 | `1a`, `2a`, `3d` | `8a`, `6c`, `6a` |
| S6 | `1b`, `1c`, `4a`, `7a`, `7e` | `2c`, `7b`, `7d`, `5d`, `2d` |
| S7 | `2b`, `4b`, `5c`, `1d`, `5b`, `3b` | `8b`, `3a` |
| S8 | `5a` | — |

**The 3 not built:** `9b` is **superseded** — the Ayu mockup, replaced by a Tokyo Night mapping
([Q7](#q7)); its acceptance *shape* survives at S1 with a different palette. `4c` (the pane
Claude built) and `4d` (tmux control mode) are v1.5, apart from `4d`'s "coexists politely with
your panes," which S1 already covers.

---

## 5. Decisions

The twelve questions this plan raised were answered on **2026-08-11**. Q12 arrived later than
the rest, from a design conversation rather than from reading the docs. They keep their `Q`
numbers as stable ids, since the phases cross-reference them. Each records what was decided,
why, and what cost the decision accepts.

**Four amend the design docs** rather than filling a gap in them — [Q3](#q3), [Q4](#q4),
[Q7](#q7) and [Q9](#q9) — and each says so where it sits. The handoff asks that nothing in the
design docs be relitigated without flagging it explicitly; each was flagged before being decided,
and the amendment is recorded here rather than absorbed silently into the build. **Three more
came out of `CP-1` and two out of `CP-2`**, tabled directly below the twelve.

| | amends | what changes |
|---|---|---|
| [Q3](#q3) | Component Breakdown | `edtui` was **"buy (input)"**; the input machine is ours |
| [Q4](#q4) | Component Breakdown | `ratatui-markdown` was a plain feature-gated buy; it is a vendored fork |
| [Q7](#q7) | Design Brief · Design Language §10 · Component Breakdown | Ayu → Tokyo Night as the second mapping |
| [Q9](#q9) | Design Language §9 · §11 | needs-you asks **queue** rather than appear-and-wait; `!` joins the last-standing statusline set |

**Three more were settled at `CP-1`**, against the build rather than on paper — they are the
first amendments that came from looking at a running program, which is what the checkpoint is
for. Same rule applies: recorded here, and the `.dc.html` files are edited in the Design project,
not in this repo.

| | amends | what changes |
|---|---|---|
| `CP-1` | Design Language §10 | *"Claude owns the brightest colour on screen"* is **dark-mode only**. Measured against each theme's own ground, claude is top of the actors on dark (10.91:1) and **5th of 6 on paper** (3.21:1, below `neutrals.meta` at 3.33:1 and 0.04 clear of steel-green). The light palettes are as mockup `8c` draws them, so the contract was the thing that did not survive, not the values. On light, actor identity rests on hue — which validation already enforces — rather than on brightness. |
| `CP-1` | Design Language §5 | Segments join with a thin bar **within the counter group only**. §5's prose reads as though every segment does, but §5's own reference render and all four of `1a`, `9c`, `8c`, `8d` draw a plain gap between session state and the counters. The drawings won; the build drew the bar and now does not. |
| `CP-1` | TUI Mockups `8d` | The shed ladder is **fit-driven**, not width-labelled. `8d` is titled *"80 columns"* and draws the ladder's floor; at a real 80 columns with `src/retry.rs` nothing has dropped yet, because it all fits. §11's *order* is exactly what the build does — only the trigger differed, and a width-labelled trigger would drop content that fits. `8d` is relabelled as illustrating the end of the ladder. |

**Two more were settled at `CP-2`**, both against a running program and both about mockup `6b`.
That screen is now the only one whose drawing has contradicted itself and the build in the same
window; the rule is unchanged — recorded here, edited in the Design project.

| | amends | what changes |
|---|---|---|
| `CP-2` | TUI Mockups `6b` | A persisted form goes to **the file that loads last**, and `6b`'s receipt reads `· persisted to persisted.scm`, not `init.scm`. Found by running it: `init.scm` runs to its last form *before* Rust reads the load order it declared, so a `(keymap-set! …)` appended there comes back on the next boot as a free-identifier fault in a float — `keymap-set!` is defined in `keymaps.scm`, which has not loaded yet. The layer names its own target (`phosphor/persist-file`); a one-file layer still gets `init.scm`, which is what `6b` drew and why. Regression test: `a_persisted_rebind_survives_the_next_boot`. |
| `CP-2` | TUI Mockups `6b` | The λ prompt is **steel** `#9ec98c`, not claude green `#3ddc97`. Here the two drawings disagree with each other rather than with the build: Design Language's glyph lexicon draws `λ ◆` in steel and captions it *"steel prompt · steel surface"*, while `6b` draws the same glyph in claude green. Teej's ruling is that the lexicon governs — it is the drawing that is specifically about this glyph — so `6b` is the bug. The build already composed `Tone::Steel`, and did not fold either reading in while the question was open. |

<a id="q1"></a>
### Q1 · Seen-state lives out-of-tree, keyed on the workspace root path

*Was: the handoff says "per-repo," but the brief says no VCS is ever required — "per-repo" is
undefined for a plain directory.*

**Decided:** `$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/`, keyed on the canonicalised
workspace root path and never on VCS identity. Append-only log with periodic compaction, sharing
its format and compaction path with persistent undo ([Q2](#q2)).

One keying mode serves a jj repo and a bare directory identically, which is what makes "no VCS
required, ever" true in the code rather than only in the brief — there is no second path to
maintain and no migration when a directory later becomes a repo. Staying out of the tree also
keeps phosphor from ever showing up as a dirty file in the user's own VCS.

**Accepted cost:** seen-state does not travel with the checkout. A fresh clone, or a moved
directory, starts with everything unseen. Nothing is lost — material is only re-shown — and for
a per-user, per-machine reading log that is the right failure mode.

<a id="q2"></a>
### Q2 · `phosphor-buffer` owns the undo model, `phosphor-core` owns persistence

*Was: the crate layout assigns undo persistence to `phosphor-core` and persistent undo to
`phosphor-buffer`. Both cannot own it.*

**Decided:** the text engine owns the undo tree and edit semantics; the store serialises it.
`phosphor-core` already owns the on-disk story for seen-state, and the two want one file format,
one compaction path, and one crash-safety story rather than two.

**Confirmed by the T008 spike** ([SPIKES.md](SPIKES.md)) — the decision stands, the mechanism
changed. The bought `History` has private fields, no iterator and no serde, so it **cannot** be
serialised. It doesn't need to be: `Edit`, `EditBatch`, `EditState` and `Operation` are public
with public fields (`code.rs:22`, `28`, `35`, `52`) and `Editor::apply_batch` is public
(`editor.rs:482`). **We keep our own log of batches and replay them, bypassing upstream `History`
rather than extending it** — which also means the on-disk format is ours to version instead of
being hostage to an upstream struct.

<a id="q3"></a>
### Q3 · Build the input machine — `edtui` is dropped *(resolved by spike; amends the Component Breakdown)*

*Was: spike edtui before wiring agent nouns, and keep the custom-input-machine fallback budgeted.
The predicted blocker was data-model impedance — a `KeyEventHandler` welded to its own
`EditorState` rather than emitting Actions over a rope.*

**Resolved by the T009 spike** ([SPIKES.md](SPIKES.md)): **don't buy it. Build the input
machine.** The fallback is now the plan.

The separation the spike went looking for does exist — a public `enum Action` and a separate
`pub trait Execute` (`actions.rs:49`, `129-130`) — and exposing it is a one-line fork, since only
the resolver `get()` is private (`events/key.rs:115`). **The predicted blocker was real but
surmountable. Two others, neither on the list, are not:**

- **The register model cannot express numeric counts or named registers.** Lookup is exact
  key-sequence prefix matching (`events/key.rs:115-136`), so `3dd` has nowhere to live, and
  there is no named-register concept in the crate. `CP-3` tests both.
- **Our keymaps live in Steel.** T033 puts every binding in `runtime/`, redefinable at runtime.
  edtui's register is a compile-time `HashMap` of 185 entries — the main thing we would be
  buying, and it is dead weight by our own design.

What's left to buy is a **28-line prefix matcher**, delivered inside a 10,164-line crate carrying
its own `edtui-jagged` buffer (not ropey), its own undo, renderer, `syntect` highlighting and
line wrapper.

**Consequences:** counts and named registers are designed into the input machine from the start
rather than retrofitted. edtui's `Action` enum stays useful as a **completeness checklist** for
the vim grammar — it is a good inventory of verbs and objects, and worth diffing our coverage
against before `CP-3`.

**Amendment recorded:** the Component Breakdown lists `edtui` as **"buy (input)"** with the
reasoning that its handler is a customisable `KeyEventHandler`. That is accurate as far as it
goes and still the wrong call for us.

**Alternative considered and now closed:** making edtui the buffer core outright, on the strength
of 242k downloads against 3.4k. The spike killed it from the other direction — `ratatui-code-editor`
turned out to be 4,936 lines with a 281-line renderer, which is small enough to own with
confidence, while edtui's data model is the thing furthest from what we need.

<a id="q4"></a>
### Q4 · Vendor and patch `ratatui-markdown` to ratatui 0.30 — *amends the Component Breakdown*

*Was: the crate pins `ratatui ^0.29` and cannot compile against the 0.30 workspace the same
document specifies.*

**Decided:** fork it and do the bump ourselves, so rendered transcript prose ships in v1. The
Component Breakdown's reasoning stands — Claude writes markdown whether we render it or not, and
buying now beats plain text plus a later migration — so the fix is to make the buy possible
rather than to give up the surface.

**Amendment recorded:** the Component Breakdown lists this as a straightforward feature-gated
buy. It is now a **second vendored fork**, with the maintenance that implies. §2 sets the terms
that keep it cheap: a version bump rather than a feature fork, no phosphor-specific behaviour
inside it, the feature gate retained so a broken bump degrades to plain text instead of breaking
the build, and the crate's per-language highlight features left off. If upstream ever ships a
0.30-compatible release, the subtree is replaced by the published crate and the fork is deleted.

**Accepted cost:** two forks to carry instead of one, for a non-core surface.

<a id="q5"></a>
### Q5 · Pin `steel-core` at `=0.8.2` exactly

*Was: 0.8.2 released February 2026 with nothing since, pre-1.0, small user base — and the entire
editor layer sits on it.*

**Decided:** exact pin, not caret. 0.x crates routinely move embedding APIs in patch releases,
and this one carries the whole editor layer, so an upgrade becomes its own scheduled task gated
by the door-parity test. Churn can never arrive unannounced inside an unrelated build.

**Alternative considered:** vendoring Steel too. Rejected — it is a far larger body of code to
carry than a widget, and an exact pin already buys the control that matters.

<a id="q6"></a>
### Q6 · Review blocks over MCP, watch values over ACP

*Was: named as open in the handoff — the design says only "over the session."*

**Decided:** `agent-client-protocol` 2.0.0 for the session, `rmcp` 3.1.2 for the editor-tool
server. Then the split follows the nature of each signal:

- **Review blocks are an MCP tool call** — `phosphor/declare-review-block`, carrying a file+range
  list and per-group annotations. Declaring one is an editor-facing capability, which is exactly
  what the docs reserve MCP for. Routing it through the S2 registry means Steel and the CLI can
  declare a review block too, which invariant 2 requires and which a session-only mechanism
  would have quietly denied them.
- **Watch values are ACP session notifications** — they stream continuously during a turn and
  are session state, not an editor mutation. The request/response shape of a tool call fits
  them badly.

<a id="q7"></a>
### Q7 · Ayu is dropped; Tokyo Night is the second mapping — *amends the Design Brief*

*Was: screen `9b` flags the tension itself — Ayu's identity colour is orange, which the language
reserves for attention, and theme validation rejects a theme that reassigns actor hues.*

**Decided:** the actor contract holds without exception, and Ayu is replaced rather than bent.
Tokyo Night takes its place: blue-violet collides with no actor hue, and Tokyo Night Day is a
real light variant rather than an afterthought, so the dark/light pair is honest.

**Amendment recorded, in three places.** Ayu is named in the Design Brief's "Decided since"
(*"Catppuccin and Ayu as the first two mappings"*), in **Design Language §10** (*"Catppuccin and
Ayu ship first"*), and in the **Component Breakdown's `Theme` spec** (*"Catppuccin and Ayu ship as
mappings"*). All three are superseded; Catppuccin is unchanged in each. Screen `9b` is superseded
too — there is no Tokyo Night mockup, so `9b` stands as the *shape* of the S1 acceptance test
(same slice of UI, a second palette, actor contract intact) with a different palette substituted.

**Alternatives considered:** demoting Ayu's orange to a syntax role and shipping it anyway
(rejected — it would not look like Ayu, so the recognition that justified choosing it is gone);
and relaxing validation to hue *families* so Ayu could shift attention within a warm band
(rejected — it makes "hue is the contract" fuzzy and needs a distinguishability test to replace
a rule that currently needs none).

<a id="q8"></a>
### Q8 · Accept the retargeting of `7c` and `6d`

*Was: the build order puts `7c` at S3, but `7c` is LSP completion and signature help and LSP is
S4; and `6d` displays agent nouns that cannot resolve without the store at S5.*

**Decided:** `7c`-minus-completion at S3, full `7c` at S4; `6d` renders from the live keymap at
S3 and becomes functional at S5. No design change and no resequencing — just acceptance criteria
that describe what is actually true at each boundary.

**Alternatives considered:** swapping S3 and S4 so `7c` lands whole (rejected — it builds
completion floats before there is a modal input machine to drive them, inverting a natural
dependency); and merging S3 and S4 into one "plain editor" milestone (rejected — it is a large
step, and it breaks the 8-step numbering the design docs cross-reference).

<a id="q9"></a>
### Q9 · Asks are queued; the one-float rule is never broken — *amends the Design Language*

*Was: the design language states both "opening a second float replaces the first, there is no
float-over-float, ever" and "needs-you never steals focus." These conflict when Claude asks a
question while a picker is open.*

**Decided:** queue the ask. It sets the statusline `!` flag immediately and waits; the float
surfaces once no other float holds focus, and `]!` jumps to a pending ask. Nothing is destroyed
under the user, and it matches the brief's "the user reads when they get a chance."

**Amendment recorded — this changes a written rule rather than reconciling two.** Design Language
§9 says needs-you asks *"appear, set the statusline flag, and wait,"* and the Component
Breakdown's `QuestionBody` says *"renders alongside a waiting statusline flag."* Both describe an
ask that **is on screen** while something else holds focus. Queueing means it is **not** on
screen until the coast is clear. That is the better call — the alternative it displaces is
float-over-float in everything but name, which §9 forbids outright — but it is a change to §9,
not a literal reading of it, and the handoff's rule says to say so.

**Second amendment, same decision:** Design Language §5 and §11 both say *"the `✻`/`●n` pair is
the last thing standing"* in the statusline shed order. Because the queued ask's only notification
is the flag, `!` **joins that set** — `✻` / `●n` / `!` are now the last three standing, at every
width down to 40 columns. This plan asserts the three-glyph version throughout; §5 and §11 are
superseded on that point.

**The alternative that keeps §9 literal** — render the ask unfocused beneath the focused float,
keystrokes still going where they went before — is rejected because it is float-over-float in
layout, and §9's "no float-over-float, ever" is the stricter rule of the two.

Two consequences the build has to honour:

- **The statusline flag is the whole notification**, not decoration. It must survive shedding at
  narrow widths — `✻` / `●n` / `!` are the last things standing.
- **The queue is a store query, not widget state.** Pending asks are a ViewModel like everything
  else, so `]!`, the inbox, and the statusline all read one truth (invariant 4).

**Accepted cost:** an ask can sit unnoticed. That is the deliberate trade against destroying an
open picker, and the flag plus the inbox are what keep it from being lost.

<a id="q10"></a>
### Q10 · Internal checkpoints for S6 and S7; the 8-step numbering stands

*Was: S6 carries ten screens and both transports, S7 bundles three independent workstreams, and
S8 carries one — the steps are very unevenly sized.*

**Decided:** keep the numbering, since all four design docs cross-reference it, and add review
points inside the two large steps. S6 splits at *session attaches and streams* / *directing*;
S7 splits at `S7.1` / `S7.2` / `S7.3` (review surfaces, dirty state, VCS), each independently
shippable. Both are recorded in the phases above.

<a id="q11"></a>
### Q11 · `:arch` ships in v1, at S5

*Was: screen `6a` — the editor drawing its own architecture — is in the mockups but in no build
step and no scope list.*

**Decided:** in scope, slotted into S5 as an `ArchDiagram` float body over a store query. It is
cheap once the store exists, and it turns "every surface is a query over one store" from a claim
into something you can look at — which is worth having when explaining the project to anyone,
including a future contributor.

<a id="q12"></a>
### Q12 · The UI is composed in Steel, over a declarative view tree

*Was: the Component Breakdown puts float layouts, picker sources and columns, statusline
segments, the inbox and which-key in Steel, and reserves "the renderer" for Rust — but never
says where the renderer ends and the editor begins. The proposal on the table was to wrap
ratatui in Steel directly, so the UI could be redefined.*

**Decided:** the high-level UI lives in Steel, but Steel **never calls ratatui**. It returns a
**declarative view tree** — plain data describing which primitives, laid out how, with what
props — and Rust interprets that tree into ratatui calls.

Handing a GC'd scheme a `&mut Buffer` is the one thing that can both corrupt a frame and drop
one, which is precisely what the placement test assigns to Rust, and torn frames are P0. The
view tree gets the redefinability without the hazard.

**Three layers:**

| Layer | Owns | Crate |
|---|---|---|
| **Primitives** | `BufferView`, `Float`, `Picker`, `DiffBody`, `TranscriptPane`, `GutterBar`, `VirtualText`. Parameterised in Rust, *composed* elsewhere. | `phosphor-ui` |
| **The view tree** | The contract: plain data, **no Steel dependency and no ratatui dependency**, so neither side owns it. | `phosphor-core::view` |
| **Composition** | What is on screen, where, containing what. Float layouts, statusline composition, picker columns, pane contents. | `runtime/*.scm` |

`phosphor-steel` produces the tree; `phosphor-ui` consumes it. The M-0 structural lint extends
to match: `phosphor-ui` may import `phosphor_core::vm` and `phosphor_core::view`, never
`::store`.

**Evaluation runs at the rate of state change, not the rate of frames.** Rust caches the last
view tree and redraws it every frame without re-entering the VM; Steel re-runs only when a
ViewModel actually changes. A transcript streaming at 60fps costs one VM invocation per chunk,
not sixty per second. This is what keeps `steel-core` — pre-1.0, with unmeasured per-frame
characteristics — permanently out of the frame budget, and it falls out of the store →
ViewModel → re-derive loop the design already specifies.

**Steel composes primitives; it does not define them.** A new primitive is a Rust change.
Without that line, custom widgets get written in scheme and the frame budget comes back.

**One escape hatch:** a `spans` primitive taking styled rows from Steel, for surfaces the
primitive set doesn't cover. `:arch` ([Q11](#q11)) is exactly this — a store query rendered as
text — and it needs no Rust primitive of its own as a result.

**The refined placement test**, sharper than the general one for this question:
*does it produce pixels? Rust. Does it decide which pixels? Steel.*

**Why now rather than at v1.5:** agent-built panes ([Design Brief, v1.5](design/Design%20Brief.dc.html))
become "Claude emits a view tree" — same door, no new machinery. The Component Breakdown is
blunt that excavating baked-in Rust into Steel is a rewrite rather than a refactor, so the cost
of deciding this late is the thing the whole architecture was chosen to avoid.

**Accepted cost:** the `spans` hatch is a slope toward writing renderers in scheme. It is
signposted rather than fenced — one grep-able primitive name, so when a frame-budget regression
appears there is exactly one place to look.

---

## 6. Risk register

| risk | trigger | mitigation | lands |
|---|---|---|---|
| ~~`ratatui-code-editor` seams don't exist~~ | — | **Retired by the T008 spike.** Three of five seams usable, virtual text is a clean enum-variant addition, fallback not triggered. | M-0 |
| ~~`edtui` handler can't emit Actions~~ | — | **Retired by the T009 spike** — the crate is dropped ([Q3](#q3)), so the risk is gone with it. | S3 |
| **Soft-wrap is unbudgeted work** ([SPIKES.md](SPIKES.md)) | S1 — the vendored crate has none | build it against `VisualRow` alongside folds and ghosts, not layered on after; it touches row↔line mapping, cursor position, click targeting and virtual text at once | S1 |
| **The input machine is now ours to get right** ([Q3](#q3)) | S3 | counts and named registers designed in from the start; verb/object coverage diffed against edtui's `Action` enum before `CP-3` | S3 |
| Grammar ABI mismatch across the first-class set | S4 | grammar crates span tree-sitter bindings 0.23–0.25 against a 0.26 runtime — load all of them and parse a fixture in M-0, before `define-language` depends on it | M-0 |
| Steel embedding API churn ([Q5](#q5)) | `steel-core` upgrade | pin `=0.8.2`; door-parity test as the upgrade gate | S2 |
| The three doors drift apart | any new capability | one registry, generated bindings, enumerating parity test | S2 |
| Policy accretes in Rust | ongoing | the placement test in code review; `runtime/*.scm` is where keymaps/segments/sources live | S2+ |
| The Steel VM lands in the frame path ([Q12](#q12)) | a view tree rebuilt per frame instead of per state change | cache the tree in Rust and re-derive only on ViewModel change; assert VM invocations per second stays flat under streaming load | S2 |
| Renderers get written in the `spans` hatch ([Q12](#q12)) | custom surfaces growing past `:arch` | one grep-able primitive name; a new primitive is a Rust change by rule | S5+ |
| Torn frames | new async event source | synchronized output from S1; re-check at each phase boundary | S1 |
| Anchors don't survive real rewrites | S5 | `6c` as an executable acceptance test, not a demo | S5 |
| Second-tier languages feel broken rather than honest | S5 | line-fallback markers tested on an extensionless file as an S5 gate | S5 |
| Two vendored forks to carry ([Q4](#q4)) | every upstream bump | `ratatui-markdown` stays a version bump, not a feature fork; feature gate retained so a broken bump degrades to plain text | S6 |
| A queued ask sits unnoticed ([Q9](#q9)) | Claude asks while a float is open | the statusline `!` survives shedding at every width; the ask is also in the inbox | S6 |
| Seen-state doesn't survive a fresh clone ([Q1](#q1)) | new machine or moved checkout | accepted — everything re-shows as unseen, nothing is lost; documented in the UI's cold-start copy (`7d`) | S5 |

---

## 7. Immediate next steps

**M-0 and S1 are built.** `CP-0` passed both halves and `CP-1`'s mechanical half passed; the
sections below describe what was planned, and this one records where that left things.

**`CP-1` passed both halves on 2026-08-12**, including Teej's four-terminal pass, and produced
four rulings — three of them design-doc amendments, tabled in [§5](#5-decisions). **Window C is
next: `T019`–`T025` and `T078`–`T080`, `spine`'s contract phase, ending at `CP-2`.**

**`T019` is the task to get right.** It gates 60 others and the plan calls it *"reversible: no in
practice."* Four things the S1 host learned by being the first thing to actually run the widget
layer, all recorded in `crates/phosphor/src/main.rs`'s header:

- **Scroll is a request, and today the viewport moves from two places.**
  `buffer_view::apply_scroll` is invariant 3's single writer, but the vendored handler S1 rides
  calls `focus()` on every keystroke and `scroll_up`/`scroll_down` on every mouse event. `Action`
  needs a `Scroll(ScrollRequest)` variant — `ScrollRequest` is already shaped as its payload —
  and `T026` has to **stop calling** `input`/`mouse` rather than wrap them.
- **A mode is a fact the statusline reads, not a flag input owns.** The chip is hardcoded to
  `Normal` because S1 has no modality; `soft_wrap::set_mode` already wants the real one
  (whitespace marks are INSERT-only).
- **Dirty is per buffer and comes from the edit stream**, not from a save path.
- **Floats need `OpenFloat(kind)` / `CloseFloat`.** The one-float rule lives in the widget;
  `T021`'s broken-`init.scm` float is the first real caller.

**The Steel VM is checked, not assumed.** `crates/phosphor-steel/tests/embed_smoke.rs` confirms
`steel-core 0.8.2` embeds, evaluates Steel-dialect `define`/`lambda`, and lets Rust register a
value the Scheme side then uses — the direction invariant 1 depends on. Same reasoning as the
grammar ABI check: the pin is exact *because* the VM is pre-1.0, and six tasks were about to be
built on top of an assumption nothing had exercised.

What the two checkpoints changed, beyond ticking tasks:

1. **`T090` had to be invented.** `CP-1` failed on its first attempt because Windows A and B
   built a complete, tested, lint-clean widget layer around a `main.rs` that was still
   `fn main() {}` — so `cargo run` drew nothing, every tape died on `Require phosphor`, and the
   checkpoint could not be judged at all. The plan assumed an application existed and no task
   built one. It is `spine`'s, thin, and `T026` deletes it.
2. **Three design-doc amendments** came out of `CP-1`, tabled in [§5](#5-decisions). They are the
   first that came from looking at a running program rather than from reasoning on paper, which
   is precisely what the checkpoint exists to produce.
3. **The grammar ABI unknown is closed.** All eleven shipped grammars plus two auxiliary load
   and parse under tree-sitter 0.26 with no `ERROR` nodes. `tree-sitter-scheme` handles Steel
   with two characterised gaps — `#u8(...)` bytevectors and `#%`-prefixed compiler internals —
   which will bite `S4`/`T037` only if `T033`'s Steel uses either.

The two things worth carrying into S3: **the input machine is ours**, so counts and named
registers are designed in rather than retrofitted; and **the S1 host moves the viewport from two
places today** — `buffer_view::apply_scroll` and the fork's own handler — so `T019`'s `Action`
needs a `Scroll` variant and `T026` must stop calling `input`/`mouse` rather than wrap them.

Settled and needing no further input: `tui-textarea` → `ratatui-textarea` (§1), the
`ratatui-markdown` bump (§2, before S6), and the full dependency manifest and hygiene tooling
([SPIKES.md](SPIKES.md)).
