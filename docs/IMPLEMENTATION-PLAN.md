# Phosphor — implementation plan

Derived from `docs/design/` (Design Brief, Design Language v0.2, TUI Mockups, Component
Breakdown) per `docs/design/CLAUDE-CODE-HANDOFF.md`. The design docs are the contract; this
document is the route through them.

The eleven questions this plan raised were **answered on 2026-08-11** and are recorded in
[§5 Decisions](#5-decisions). Two of them amend the design docs rather than merely filling a
gap, and are marked as amendments there. Nothing below is an open question any more; where a
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

---

## 1. Dependency reality check

The Component Breakdown's buy list was verified against crates.io on 2026-08-11. Most of it
holds. **Two entries do not**, and one carries far more risk than the doc implies.

| crate | latest | ratatui dep | verdict |
|---|---|---|---|
| `ratatui` | 0.30.2 | — | OK — pin workspace here |
| `ratatui-core` | 0.1.2 | — | OK — `phosphor-ui` depends on this only |
| `ratatui-code-editor` | **0.0.6** | `ratatui-core ~0.1.0` | **RISK** — compatible, but 3.4k downloads, 6 releases, single maintainer. Vendor; see §2 |
| `edtui` | 0.11.6 | `ratatui-core ^0.1` | OK — healthy; but see [Q3](#q3) on data-model impedance |
| `tui-textarea` | 0.7.0 (2024-10-22) | **`ratatui ^0.29`** | **BROKEN** — stale 2 yrs, incompatible with 0.30 |
| `ratatui-textarea` | 0.9.2 | `ratatui-core ^0.1.1` | **USE THIS** — the maintained fork |
| `ratatui-markdown` | 0.3.6 | **`ratatui ^0.29`** | **VENDOR + PATCH** — unbuildable as published; forked and bumped to 0.30, see [Q4](#q4) |
| `nucleo` | 0.5.0 | none (pure matcher) | OK — no ratatui coupling; helix's engine |
| `tui-tree-widget` | 0.24.1 | `ratatui-core ^0.1.0` | OK |
| `throbber-widgets-tui` | 0.11.1 | `ratatui ^0.30` | OK |
| `ratatui-comfy-tabs` | 0.5.12 | `ratatui-core ^0.1.2` | 600 downloads — **build TabBar instead**, the doc already allows it |
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

### Mechanics

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

### The seams we need from it

The phosphor layer wraps the bought editor; these are the extension points the fork must
provide:

1. **Marks API** — the substrate for unseen regions and region tints.
2. **Gutter column injection** — the 1-cell state bar left of line numbers.
3. **Virtual-text row interleaving** — `┊`-prefixed rows that consume screen rows without
   existing in the rope.
4. **Scroll authority** — the wrapper, not the widget, decides when the viewport moves
   (invariant 3).
5. **Diff view restyle** — seeds `DiffBody`.

### Spike before commitment (M-0)

The design doc asserts the crate "carries more than expected": tree-sitter highlighting with
per-viewport caching, full editing + undo/redo, visual marks, diff views with expandable
unchanged sections, grapheme-correct cursor/selection, mouse. At 0.0.6 those claims must be
**read against the actual source before the plan depends on them.** The published feature
flags are only `bench-internals, crossterm, default`, which tells us nothing about the marks
or diff APIs.

The spike answers three questions:

- Does a marks/decoration API exist that can carry per-region state, or would we be adding it?
- Is the diff view a separable widget, or entangled with its own editor state?
- Is the undo history reachable and serialisable? (This is [Q2](#q2) — persistent undo is a
  v1 scope item and bought undo stacks are usually in-memory only.)

**Fallback, budgeted now:** if the spike says the seams aren't there, `BufferView` is built
directly on `ropey` + `tree-sitter` and we own one large widget. That is a real possibility at
0.0.x, not a remote one. The `Action` layer and the ViewModel boundary mean this choice does
not leak past `phosphor-ui` — which is precisely why those boundaries land in M-0 and S2,
before any of this is decided.

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

## 3. Phased plan

`M-0` is scaffolding (the handoff's "crate scaffolding first"). `S1`–`S8` are the Component
Breakdown's build order, unchanged in order and intent, with acceptance criteria attached.

Screen ids (`1a`, `2a`, …) refer to `docs/design/TUI Mockups.dc.html`.

---

### M-0 · Scaffolding and the structural tests

**Goal:** an empty editor that already cannot violate the architecture.

- Cargo workspace with the units from the Component Breakdown: `phosphor`, `phosphor-core`,
  `phosphor-buffer`, `phosphor-ui`, `phosphor-agent`, `phosphor-steel`, `phosphor-vcs`, plus
  `runtime/` (not a crate — the Steel source tree).
- Pin `ratatui 0.30.2` / `ratatui-core 0.1.2` at the workspace root. `phosphor-ui` depends on
  `ratatui-core` only.
- `vendor/ratatui-code-editor` subtree + `VENDOR.md` + the `just vendor-*` recipes (§2).
- The **ratatui-code-editor spike** (§2) — this is M-0's real content, and its outcome sizes S1.
- CI: `fmt`, `clippy -D warnings`, `test`, and two structural lints that encode the invariants:
  - **no literal colours in `phosphor-ui`** — every widget takes `&Theme` (Design Language
    §12). A grep-level lint over `Color::Rgb` / `Color::Indexed` in that crate is enough.
  - **no store mutation from `phosphor-ui`** — enforced by dependency direction: `phosphor-ui`
    sees ViewModel types only, never the store's `&mut` API. Model it as separate modules
    (`phosphor_core::vm` vs `phosphor_core::store`) with the widget crate importing only the
    former.

**Scope**
- Files: 7 `Cargo.toml` + 7 `lib.rs`/`main.rs` stubs, 1 `justfile`, 1 CI workflow, 1 `VENDOR.md`
- Named units: 2 structural CI lints, 1 vendored subtree, 1 dependency spike
- Verification: CI green on an empty workspace; spike written up in `VENDOR.md`
- Risk: public API no · data migration no · cross-module no · reversible yes · external
  blocker **yes — the spike outcome sizes S1**

**Done when:** `cargo build` is green, both structural lints run in CI, and `VENDOR.md`
records a yes/no on each of the five seams in §2.

---

### S1 · Theme + BufferView + StatusLine shell

**Goal:** *"phosphor renders and edits a file with highlighting on day one; feels like the
mockups."*

- `Theme`: actor/state palette struct (`claude, you, attention, trouble, transient, steel`) +
  neutral ramp + syntax map. base16-style loading. **Actor-hue validation at load** — a theme
  reassigning actor hues is rejected, not themed (Design Language §10).
- Phosphor dark + light built in. **Catppuccin and Tokyo Night as the first two mappings**
  ([Q7](#q7) — Ayu is dropped; this amends the Design Brief).
- `BufferView` over the vendored editor: 3-column contract (1-cell state bar → line numbers →
  text), soft-wrap `↪` continuations without line numbers, fold rows.
- `StatusLine` shell: mode chip, file + dirty flag, spring, `SessionState` (rendering `None`
  for now), counters. **Truncation enforced in the widget** — never wraps, a second line is a
  bug.
- Synchronized output wrapping every frame from the first draw call. A torn frame is P0;
  retrofitting this later means auditing every render path.

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
- Files: `phosphor-ui/{theme,buffer_view,status_line}.rs`, `phosphor-buffer/{rope,ts}.rs`,
  3 theme files
- Named units: 3 widgets, 2 built-in themes + 2 mappings (each dark + light), 1 theme
  validator, 1 synchronized-output wrapper
- Verification: golden-frame snapshot tests per screen id + a manual terminal pass at 80 and
  120 columns
- Risk: public API no · data migration no · cross-module no · reversible yes · external
  blocker **yes — depends on the M-0 spike**

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
- Verification: door-parity test (every registered Action reachable from all three doors — a
  test that *enumerates the registry*, so it cannot be forgotten), REPL liveness test
- Risk: public API **yes — this is the public API** · data migration no · cross-module **yes
  (every crate above `phosphor-core`)** · reversible **no in practice** · external blocker no

---

### S3 · Input + persistent undo + gutter/virtual-text layer

**Goal:** plain editor complete.

- **Step 0 of this step is the edtui spike** ([Q3](#q3)): can its `KeyEventHandler` emit
  `Action`s instead of mutating state it owns? That question gates everything below it, and is
  answered *before* agent nouns are wired, not after. If the answer is no, the fallback is a
  custom input machine behind the same `Action` layer — the docs already budget for it.
- Agent nouns registered as custom text objects: `viu`, `sib`, `dih`, `:'<,'>c`.
- **Persistent undo** on disk, surviving restarts. **`phosphor-buffer` owns the undo model,
  `phosphor-core` owns persistence** ([Q2](#q2)) — `phosphor-core` already owns the on-disk
  story for seen-state, and the two share one file format and one compaction path.
- The gutter/virtual-text layer: `GutterBar` (1-cell state column, priority trouble >
  attention > claude-unseen > none, `▎` degradation) and `VirtualText` (`┊`-prefixed rows
  owned by a region id).
- Keymaps and the leader tree in Steel; `KeymapFooter` / WhichKey reads the *live* keymap.
- The once-per-session unknown-key hint.

**Acceptance:** `3c` (leader popup) · `6d` (`:help agent-objects`) renders from the live
keymap · `8e` (first keystroke teaches once; folds, soft-wrap continuation, insert-only
whitespace marks).

> **Flag — two acceptance targets in the docs cannot be met at this step.**
> The build order lists `7c` here ("plain editor complete: 7c"), but `7c` is *"lsp completion
> + signature help"* and LSP is S4. **Retargeted:** `7c`-minus-completion here, full `7c` at S4.
> Similarly `6d` displays the agent nouns, but `viu` ("inside unseen") cannot *resolve*
> without the semantic store (S5). **Retargeted:** the help surface and grammar render here;
> the nouns become functional at S5. See [Q8](#q8).

**Scope**
- Files: `phosphor-ui/{gutter,virtual_text,keymap_footer}.rs`, `phosphor-buffer/undo.rs`,
  `phosphor/input.rs`, `runtime/{keymaps,leader}.scm`
- Named units: 3 widgets, 4 agent text objects, 1 persistent-undo store, 1 input adapter
- Verification: text-object unit tests against real source files; undo round-trip across a
  simulated restart
- Risk: public API no · data migration **yes — the undo file format is on disk from here** ·
  cross-module no · reversible yes · external blocker **yes — [Q3](#q3) may force a custom
  input machine**

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
- Files: `phosphor-core/{store,region,anchor,seen}.rs`, `phosphor-ui/picker.rs`,
  `runtime/pickers/*.scm`
- Named units: 1 store, 1 region state machine, 2 anchor strategies, 1 Picker widget, 3 Steel
  picker sources, 1 `ArchDiagram` body
- Verification: anchor-survival tests (apply a real refactor, assert threads/seen/watches
  follow); line-fallback tests on an extensionless file; restart- and crash-persistence tests
- Risk: public API no · data migration **yes — seen-state on disk** · cross-module **yes** ·
  reversible yes · external blocker **yes — [Q1](#q1)**

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
  `phosphor-ui/{transcript,prompt_line,question}.rs`, `runtime/permissions.scm`
- Named units: 1 ACP client, 1 MCP server, 3 widgets, 1 `SessionState` enum, 10 screens
- Verification: session-lifecycle tests (drop mid-turn, reattach, adopt); a torn-frame check
  under streaming load; permission rules round-trip to `init.scm`
- Risk: public API no · data migration no · cross-module **yes** · reversible yes · external
  blocker **yes — [Q6](#q6) wire details**

---

### S7 · Diffs, review blocks, inbox, dirty-state, VCS

**Goal:** the surfaces that make a review block readable and a disk conflict honest.

Three workstreams the docs bundle into one step (see [Q10](#q10)):

**7a — Review surfaces.** `DiffBody` (vendored diff view restyled + per-hunk seen state +
directory grouping via `tui-tree-widget` + Claude's group annotations). Review blocks. The
inbox — one list of everything Claude said, severity as a single MCP flag, unread = unseen.
→ `2b` (hunk peek), `4b` (block diff), `5c` (inbox), `8b` (the 40-file block: grouping, not
scrolling).

**7b — Dirty state.** The changed-underneath indicator (`✱`) and offer to refresh;
`:diff-disk` with its three-exit footer and **no auto-merge**. This is invariant 3 at its
sharpest. → `1d`, `5b`.

**7c — VCS.** `phosphor-vcs`: jj first, git second, both behind a trait, compiled in and
activated on detection. **No feature may assume a repo exists** — the adapter's absence is a
normal state, not an error path. → `3b` (jj timeline: agent turns are changes, undo is time
travel).

**Acceptance:** `2b` · `4b` · `5c` · `8b` · `1d` · `5b` · `3b` · `3a` (anchored exchange: your
comment and Claude's reply as virtual text under the region) · **every one of the above also
passing in a directory with no VCS at all.**

> **Flag:** `3a` is unassigned in the docs; it is the visible form of a thread overlay and
> lands with the review surfaces. Assigned to 7a.

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
  blocker **yes — [Q6](#q6) streaming mechanism**

---

### Cross-cutting, every step

These are not phases; they are checks that run at each phase boundary:

- **`8d` (80 columns)** — drop, never squeeze. Statusline shed order: counters → jj → cursor
  pos → session prose (glyph stays) → mode word (initial stays). `✻` / `●n` is the last thing
  standing. Pickers lose the preview split under 100 cols; floats go full-width.
- **Degradation** — markers → `▎`, undercurl → underline, spinner → static `✻` on dumb
  terminals.
- **Torn frames are P0** — synchronized output wraps every frame, checked whenever a new async
  source starts posting events.
- **Voice** — lowercase, telegraphic, factual. Counts, not adjectives. Keyhints spell the whole
  command (`:reattach`, never `:ca`).

### Unplaced by design

`6a` (`:arch` — the editor draws its own architecture) and `4c` / `4d` (the pane Claude built;
tmux control mode) are v1.5 or unscheduled. `6a` is a demonstration of invariant 4 and costs
little once the store exists (an `ArchDiagram` float body over a store query) — a candidate to
slot into S5 if it's wanted in v1. Raised as [Q11](#q11), not assumed.

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

The eleven questions this plan raised were answered on **2026-08-11**. They keep their `Q`
numbers as stable ids, since the phases cross-reference them. Each records what was decided,
why, and what cost the decision accepts.

**Two amend the design docs** rather than filling a gap in them — [Q4](#q4) and [Q7](#q7) — and
each says so where it sits. The handoff asks that nothing in the Design Brief be relitigated without
flagging it explicitly; both were flagged before being decided, and the amendment is recorded
here rather than absorbed silently into the build.

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

**Still to confirm:** whether the vendored editor's undo history is reachable and serialisable
at all — that is one of the three questions the M-0 spike answers. If it isn't, we own the undo
stack outright and the bought editor is reduced to a renderer plus edit primitives.

<a id="q3"></a>
### Q3 · Spike edtui before wiring agent nouns; keep the fallback budgeted

*Was: the handoff budgets for edtui's operator-pending grammar failing to host agent nouns, but
the nearer problem is that edtui is a full editor widget whose `KeyEventHandler` mutates state it
owns, rather than emitting Actions over a rope.*

**Decided:** answer "can the handler emit Actions instead of mutating?" first, before any agent
nouns are wired. A proven vim grammar is worth adapting to if it can be adapted at all, and the
question is cheap to answer by reading. If the answer is no, the fallback is a custom input
machine behind the same `Action` layer — already budgeted by the docs, and now triggered by the
*right* test rather than by the noun test that would have come much later.

**Alternative considered:** making edtui the buffer core outright and dropping
`ratatui-code-editor`, on the strength of 242k downloads against 3.4k. Rejected for now — it
trades the riskier dependency for the loss of the bought diff view and per-viewport highlight
caching, and the gutter contract still has to be built either way. The M-0 spike may reopen it.

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

**Amendment recorded:** the Design Brief's "Decided since" list names *"Catppuccin and Ayu as the
first two mappings."* Catppuccin is unchanged; Ayu is out. Screen `9b` is superseded — there is
no Tokyo Night mockup, so `9b` stands as the *shape* of the S1 acceptance test (same slice of UI,
a second palette, actor contract intact) with a different palette substituted.

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
### Q9 · Asks are queued; the one-float rule is never broken

*Was: the design language states both "opening a second float replaces the first, there is no
float-over-float, ever" and "needs-you never steals focus." These conflict when Claude asks a
question while a picker is open.*

**Decided:** queue the ask. It sets the statusline `!` flag immediately and waits; the float
surfaces once no other float holds focus, and `]!` jumps to a pending ask. Both written rules stay
literally true, nothing is destroyed under the user, and it matches the brief's "the user reads
when they get a chance."

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
S7 splits at its three workstream boundaries (review surfaces, dirty state, VCS), each
independently shippable. Both are recorded in the phases above.

<a id="q11"></a>
### Q11 · `:arch` ships in v1, at S5

*Was: screen `6a` — the editor drawing its own architecture — is in the mockups but in no build
step and no scope list.*

**Decided:** in scope, slotted into S5 as an `ArchDiagram` float body over a store query. It is
cheap once the store exists, and it turns "every surface is a query over one store" from a claim
into something you can look at — which is worth having when explaining the project to anyone,
including a future contributor.

---

## 6. Risk register

| risk | trigger | mitigation | lands |
|---|---|---|---|
| `ratatui-code-editor` seams don't exist at 0.0.6 | M-0 spike | build `BufferView` on `ropey` + `tree-sitter`; `Action` / ViewModel boundary contains the blast radius | M-0 |
| `edtui` handler can't emit Actions ([Q3](#q3)) | S3 spike | custom input machine behind the same `Action` layer — already budgeted by the docs | S3 |
| Steel embedding API churn ([Q5](#q5)) | `steel-core` upgrade | pin `=0.8.2`; door-parity test as the upgrade gate | S2 |
| The three doors drift apart | any new capability | one registry, generated bindings, enumerating parity test | S2 |
| Policy accretes in Rust | ongoing | the placement test in code review; `runtime/*.scm` is where keymaps/segments/sources live | S2+ |
| Torn frames | new async event source | synchronized output from S1; re-check at each phase boundary | S1 |
| Anchors don't survive real rewrites | S5 | `6c` as an executable acceptance test, not a demo | S5 |
| Second-tier languages feel broken rather than honest | S5 | line-fallback markers tested on an extensionless file as an S5 gate | S5 |
| Two vendored forks to carry ([Q4](#q4)) | every upstream bump | `ratatui-markdown` stays a version bump, not a feature fork; feature gate retained so a broken bump degrades to plain text | S6 |
| A queued ask sits unnoticed ([Q9](#q9)) | Claude asks while a float is open | the statusline `!` survives shedding at every width; the ask is also in the inbox | S6 |
| Seen-state doesn't survive a fresh clone ([Q1](#q1)) | new machine or moved checkout | accepted — everything re-shows as unseen, nothing is lost; documented in the UI's cold-start copy (`7d`) | S5 |

---

## 7. Immediate next steps

**Nothing is blocked on a decision.** All eleven questions are answered (§5), so what remains is
two spikes and the scaffolding — and the spikes are reads, not builds.

1. **M-0 scaffolding** — workspace, the `=0.8.2` and 0.30.2 pins, both vendored subtrees, the
   two structural lints.
2. **The `ratatui-code-editor` spike** — the one action that removes the most uncertainty. It
   sizes S1 and settles the open half of [Q2](#q2): whether the bought undo history is
   serialisable, or whether we own the undo stack outright.
3. **The `edtui` spike** ([Q3](#q3)) — can the handler emit Actions rather than mutate state it
   owns? Cheap to answer by reading, and it decides whether S3 adapts a bought grammar or builds
   one. Worth doing alongside the first spike rather than waiting for S3, since a "no" from both
   would reopen the buy-vs-build shape of the whole UI layer.

Two things are settled and need no further input: `tui-textarea` → `ratatui-textarea` (§1), and
the `ratatui-markdown` bump, which is scoped in §2 and only has to land before S6.
