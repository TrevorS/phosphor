# M-0 spikes, dependency manifest, and hygiene tooling

Read against the **exact published sources** — `ratatui-code-editor 0.0.6` and `edtui 0.11.6`
pulled from `static.crates.io`, not git `main`, since the tarball is what we would vendor.
All citations are `file:line` in those crates. Dependency versions verified against crates.io
on **2026-08-11**.

No code and no settings were written. This is the T008 / T009 verdict and the input to T001–T007.

---

## Verdicts

| | Question | Verdict |
|---|---|---|
| **T008** | `ratatui-code-editor` — the five seams | **Vendor, but the fork is larger than the plan assumed.** Two seams clean, one workable, one absent-with-a-clear-hook, one absent entirely. |
| **T008** | Is undo history serialisable? ([Q2](IMPLEMENTATION-PLAN.md#q2)) | **The bought `History` is opaque — but it doesn't matter.** The edit primitives are public and replayable, so we own persistence outright. |
| **T009** | `edtui` — can the handler emit Actions? ([Q3](IMPLEMENTATION-PLAN.md#q3)) | **Don't buy it. Build the input machine.** And not for the reason Q3 budgeted. |

The headline: **`ratatui-code-editor` is 4,936 lines total, and its renderer is 281.** That is
small enough to own with confidence. The risk in vendoring it was never that the fork would be
unmaintainable — it's that the design doc credited it with capabilities it does not have.

---

## T008 · `ratatui-code-editor 0.0.6`

`src/lib.rs` exposes `actions, click, code, editor, editor_crossterm, history, render,
selection, theme, types, utils`. **`mod diff` and `mod view` are private** (`lib.rs:4`,
`lib.rs:16`) — only two benchmark functions escape, behind the `bench-internals` feature
(`lib.rs:5-6`).

### Seam 1 · Marks API — **partial, and thinner than described**

Exists as four methods (`editor.rs:660-682`):

```rust
pub fn set_marks(&mut self, marks: Vec<(usize, usize, &str)>)
pub fn remove_marks(&mut self)
pub fn has_marks(&self) -> bool
pub fn get_marks(&self) -> Option<&Vec<(usize, usize, Color)>>
```

A mark is `(start_offset, end_offset, colour)` and nothing else — `set_marks` parses the `&str`
straight to `Color::Rgb` (`editor.rs:664-666`). Consequences:

- **No id.** A mark cannot carry a region id, so mapping marks back to store regions needs our
  own side table keyed by offset range.
- **Wholesale replacement only.** `set_marks` overwrites; `remove_marks` clears everything.
  There is no add/remove of a single mark, so every seen-state change re-uploads the full set.
- **Colour only.** No undercurl, no priority, no style. Our gutter contract needs *state*
  (unseen / diagnostic / anchor) with priority resolution, and the anchor treatment is
  specifically an undercurl.

**Usable for region background tints. Not usable for the gutter contract or undercurl.**

### Seam 2 · Gutter column injection — **not injectable, but compose-around works**

The renderer owns the whole area and draws line numbers itself at `area.left()`
(`render.rs:99-105`), with the text starting at `area.left() + line_number_width`
(`render.rs:123`). The style is **hardcoded** `Style::default().fg(Color::DarkGray)`
(`render.rs:33`) — not themed, so it will not honour our `#414b42` without a patch.

But `get_line_number_width()` is `digits + left_code_padding + fold_gutter_width`
(`editor.rs:130-136`), and `set_left_code_padding(char_count)` is public (`editor.rs:1009`).

**So: reserve cells with `set_left_code_padding`, let the editor render, then overpaint our
1-cell state column into the reserved gap.** `Widget for &Editor` writes into a `Buffer`
(`render.rs:22-23`) we already own, so a second pass over the same `Rect` is legitimate rather
than a hack. Line-number restyling still wants a one-line patch.

### Seam 3 · Virtual-text rows — **absent, but the right abstraction is already there**

This is the most encouraging finding in the spike. The renderer does not iterate source lines;
it iterates **visual rows** (`render.rs:57`), resolved through a private `View` that flattens
lines into a `VisualRow` stream (`view.rs:30-50`, `view.rs:95` `rebuild`):

```rust
pub(crate) enum VisualRow {          // types.rs:20-36
    Real { line_idx, is_added, orig_line_idx },
    FoldSeparator { hidden_lines, hidden_start, hidden_end },
    GhostDeleted { anchor_line, original_line_idx, curr_line_idx },
}
```

**Fold separators and diff ghost lines already insert non-source rows into the stream**, and the
line↔row mapping helpers (`view.rs:259` `line_for_visual_row`, `view.rs:279`
`visual_row_for_line`, `view.rs:289` `line_visible`) already cope with the result.

Virtual text is the same shape: a `VisualRow::Virtual { owner_region, .. }` variant plus a
render arm. **The fork is a new enum variant and a match arm, not an architectural change** —
which is a far better answer than "not supported."

### Seam 4 · Scroll authority — **clean, and better than expected**

`set_offset_y` / `set_offset_x` / `get_offset_y` / `get_offset_x` are public
(`editor.rs:702-714`), as are `scroll_up` / `scroll_down` (`editor.rs:597-603`).

Critically, **`fit_cursor()` is called from exactly two places** — `focus()` (`editor.rs:143`)
and `set_cursor()` (`editor.rs:509`) — and both are explicit calls we make. **The widget does
not self-scroll during render.** Invariant 3 is enforceable by simply not calling those two
methods except from an Action.

### Seam 5 · Diff view — **exists, but is not a separable widget**

`mod diff` is private (`lib.rs:4`). The diff is a *mode of the Editor*, not a component: you set
an original buffer and the same widget renders the comparison
(`editor.rs:394-482` — `set_original_code`, `has_diff`, `set_diff_enabled`, `toggle_diff_focus`,
`set_diff_focus_context`, `set_diff_expand_amount`, `set_diff_options`, `get_line_diff`,
`expand_hidden_diff_at_mouse`).

So `DiffBody` cannot wrap a bought diff widget. Either drive a second `Editor` in diff mode and
inherit its layout, or compute hunks ourselves — the crate uses `similar` internally, which we
would depend on anyway. **Given that `DiffBody` needs per-hunk seen state, directory grouping
and Claude's annotations, driving the Editor's diff mode is likely to fight us more than
`similar` + our own body would.**

### Bonus finding · **No soft-wrap, at all**

`VisualRow` has no wrapped variant, and there is no wrapping logic anywhere in the crate. Our
design language requires `↪` soft-wrap continuations without line numbers (T016, screen `8e`).

**This is unbudgeted work.** Soft-wrap touches row↔line mapping, cursor positioning, click
targeting and virtual-text placement simultaneously — it is the single largest thing this spike
uncovered, and it lands in S1 rather than S3.

### Q2 · Undo — the history is opaque; the primitives are public

`History` holds `VecDeque<EditBatch>` with **all fields private** and only `new / push / undo /
redo` (`history.rs:4-52`). No iterator, no accessor, no serde. **The bought history cannot be
serialised.**

It doesn't need to be. These are all public with public fields (`code.rs:22`, `28`, `35`, `52`):

```rust
pub enum Operation { Insert, Remove }
pub struct Edit { pub start: usize, pub text: String, pub operation: Operation }
pub struct EditBatch { pub edits: Vec<Edit>, pub state_before: Option<EditState>,
                       pub state_after: Option<EditState> }
pub struct EditState { pub offset: usize, pub selection: Option<Selection> }
```

— and `Editor::apply_batch(&mut self, batch: &EditBatch)` is public (`editor.rs:482`), as are
`Code::undo()` / `Code::redo()` returning `Option<EditBatch>` (`code.rs:670`, `code.rs:689`).

**So we bypass the bought `History` entirely:** keep our own log of `EditBatch`, persist it,
replay with `apply_batch`. No serde derives exist (serde isn't a dependency), so we convert to
our own wire type — trivial, and it means the on-disk format is ours to version rather than
being hostage to an upstream struct.

**[Q2](IMPLEMENTATION-PLAN.md#q2) resolves exactly as decided:** `phosphor-buffer` owns the undo
model, `phosphor-core` owns persistence. The only change is that we ignore the upstream
`History` type rather than extending it.

### What we inherit by vendoring

Worth knowing before T003:

- **16 tree-sitter grammars as hard, non-optional dependencies** — bash, c, c-sharp, cpp, css,
  go, html, java, javascript, json, md, python, rust, toml-ng, typescript, yaml. It does **not**
  bundle Scheme (for Steel) or CSV, both of which our first-class set requires. Grammar
  selection belongs in `runtime/` via `define-language`, so this list wants pruning to features
  in the fork. → **now owned by `T003`**, which had no line about it.
- **`arboard`** (clipboard) and **`rust-embed`** — both non-optional, both pulling system
  dependencies we may not want in a headless test or VHS run. `arboard` wants X11/Wayland, which
  a bare CI container will not have. → **also `T003`**; its *done when* now includes building in
  a container with neither.
- **`similar`** — already there for diffing, so it costs nothing to use directly.
- Its own `Action` **trait** with ~20 concrete action structs (`actions.rs`), which collides by
  name with our `Action` **enum**. Cosmetic, but worth a rename in the fork to avoid confusion.

---

## T009 · `edtui 0.11.6` — don't buy

**The structural separation Q3 hoped for does exist.** `edtui` has a public
`enum Action` (~60 variants, `actions.rs:49`) and a separate `pub trait Execute { fn
execute(&mut self, state: &mut EditorState); }` (`actions.rs:129-130`). Grammar and execution
are genuinely different types.

**But it is not exposed, and more importantly it is not worth exposing.**

- The resolver `fn get(&mut self, c: KeyInput, mode: EditorMode) -> Option<Action>` is
  **private** (`events/key.rs:115`). The only public entry is
  `on_key_event<T>(&mut self, event: T, state: &mut EditorState)` (`events/mod.rs:66`), which
  demands the state and executes immediately.
- Making `get` public is a one-line fork. Two insert-mode paths bypass the register and execute
  directly anyway (`events/key.rs:1105` `InsertChar(c).execute(state)`, `events/key.rs:1125`
  `AppendCharToSearch`), so they'd need routing too.

So a small fork would work. The reason not to is what's on the other side of it:

**The resolver is 28 lines** (`events/key.rs:115-136`) — push the key onto a lookup buffer,
prefix-match against the register, return on a unique hit, reset on no match. That is the entire
"operator-pending grammar" the plan proposed buying.

**The register is a compile-time `HashMap<KeyEventRegister, Action>`** (`events/key.rs:36`) with
185 entries (`vim_keybindings()`, `key.rs:139+`). **Our keymaps live in Steel** — T033 requires
every binding in `runtime/`, redefinable at runtime. So the 185-entry table is worthless to us
by design, and the register type is the wrong shape for a keymap that changes at runtime.

**And it structurally cannot express two things CP-3 tests:**

| Required | Status in edtui |
|---|---|
| **Numeric counts** — `3dd`, `5j` | **Absent.** The register is exact key sequences; a count prefix cannot be expressed. The only repeat is `.` (`RepeatLastChange`, `events/key.rs:796-799`). |
| **Named registers** — `"ayy` | **Absent.** No named-register concept anywhere in the crate. |
| Text objects — `ci(`, `diw` | Present (`ChangeInnerWord`, `SelectInnerBetween`, …). |
| Modes, `.` repeat | Present. |

So the buy is: **28 lines of prefix matching**, plus a keymap table we're replacing with Steel,
plus a 60-variant Action enum we're replacing with ours — delivered inside a **10,164-line**
crate that brings its own `edtui-jagged` buffer (not ropey), its own undo, its own renderer, its
own `syntect` highlighting, and its own line wrapper.

**Verdict: build the input machine.** Keep edtui's `Action` enum open in a browser tab as a
completeness checklist for the vim grammar — it is a genuinely good inventory of verbs and
objects — and implement resolution ourselves over Steel-defined keymaps, with counts and named
registers designed in from the start rather than retrofitted.

> **Note on how this differs from what Q3 budgeted.** Q3 predicted the blocker would be
> *data-model impedance* — a handler welded to its own `EditorState`. That is real but
> surmountable. The actual disqualifiers are different and were not on the list: **counts and
> named registers are structurally inexpressible in the register model, and our keymaps live in
> Steel anyway, which makes the register — the main thing we'd be buying — dead weight.**
> The decision Q3 approved (spike first, fallback budgeted) produced the right answer; the
> reasoning behind it changed.

---

## Dependency manifest

Verified 2026-08-11. Everything below resolved — no missing crates.

### Core

| Crate | Version | Note |
|---|---|---|
| `ratatui` | 0.30.2 | app-level only |
| `ratatui-core` | 0.1.2 | **`phosphor-ui` depends on this alone** |
| `ratatui-widgets` | 0.3.2 | pulled in via ratatui; needed directly only if we use its primitives |
| `crossterm` | 0.29.0 | matches both vendored crates |
| `ropey` | 1.6.1 | |
| `tree-sitter` | 0.26.12 | |
| `steel-core` + `steel-derive` | 0.8.2 | **pin `=0.8.2`** ([Q5](IMPLEMENTATION-PLAN.md#q5)) |

### Grammars — the first-class twelve

| Language | Crate | Version | Note |
|---|---|---|---|
| Rust | `tree-sitter-rust` | 0.24.2 | |
| TypeScript | `tree-sitter-typescript` | 0.23.2 | last updated 2024-11 |
| JavaScript | `tree-sitter-javascript` | 0.25.0 | |
| Python | `tree-sitter-python` | 0.25.0 | |
| Markdown | `tree-sitter-md` | 0.5.3 | |
| JSON | `tree-sitter-json` | 0.24.8 | last updated 2024-11 |
| TOML | `tree-sitter-toml-ng` | 0.7.0 | |
| YAML | `tree-sitter-yaml` | 0.7.2 | |
| HTML | `tree-sitter-html` | 0.23.2 | last updated 2024-11 |
| CSS | `tree-sitter-css` | 0.25.0 | |
| **Steel** | `tree-sitter-scheme` | 0.24.7 | closest available; Steel is a Scheme dialect, so verify it parses `runtime/*.scm` before committing |
| **CSV** | ~~`tree-sitter-csv`~~ | — | **DROPPED** per the finding below — hand-written parser in `T082`. Was 1.2.0: 5.4k downloads, last updated 2024-01-24. |

> **Two grammar findings.**
>
> **CSV probably shouldn't use tree-sitter at all.** The only crate is 2.5 years stale with
> negligible adoption, and the design gives CSV a hand-tuned surface (virtual column alignment)
> rather than generic buffer treatment. A ~200-line CSV parser is more reliable than a stale
> grammar and gives us exactly the column model that surface needs. Recommend dropping the
> dependency and implementing it.
>
> **Verify grammar ABI compatibility in M-0.** The grammar crates were built against tree-sitter
> bindings spanning 0.23–0.25 while the runtime is 0.26. tree-sitter versions its language ABI,
> and mixing generations across one runtime is a known source of breakage. This is a cheap check
> — load all twelve and parse a fixture — and an expensive surprise if deferred.

### Transports and protocol

| Crate | Version | Note |
|---|---|---|
| `agent-client-protocol` | 2.0.0 | the ACP session ([Q6](IMPLEMENTATION-PLAN.md#q6)) |
| `rmcp` | 3.1.2 | the MCP editor-tool server |
| `async-lsp` | 0.2.4 | **recommended over `tower-lsp`** (2023) and `lsp-types` alone (2024-06); maintained, client-capable |
| `tokio` / `tokio-util` | 1.53.1 / 0.7.19 | |

### Widgets and engines

| Crate | Version | Note |
|---|---|---|
| `ratatui-code-editor` | 0.0.6 | **vendored** |
| `ratatui-markdown` | 0.3.6 | **vendored + bumped to 0.30** ([Q4](IMPLEMENTATION-PLAN.md#q4)) |
| `ratatui-textarea` | 0.9.2 | prompt line, picker filter, REPL input |
| `nucleo` | 0.5.0 | picker matcher |
| `tui-tree-widget` | 0.24.1 | directory grouping |
| `throbber-widgets-tui` | 0.11.1 | spinner |
| `similar` | 3.1.2 | diff engine — already inside the vendored editor |
| `tui-logger` | 0.18.3 | dev only |
| ~~`edtui`~~ | — | **dropped, per T009** |

### Store, persistence, and the rest

| Crate | Version | For |
|---|---|---|
| `etcetera` | 0.11.0 | XDG paths — **preferred over `directories`**, better maintained and simpler for our one use ([Q1](IMPLEMENTATION-PLAN.md#q1)) |
| `blake3` | 1.8.6 | hashing the canonical workspace root into a state-dir name |
| `serde` / `serde_json` | 1.0.229 / 1.0.151 | |
| `postcard` | 1.1.3 | **recommended** for the append-only log — compact, `no_std`-friendly, stable format; `bincode 3.0.0` is a fine alternative but changed format across majors |
| `toml` | 1.1.4 | reading any non-Steel config |
| `notify` + `notify-debouncer-full` | 8.2.0 / 0.7.0 | **changed-on-disk detection (`✱`, screen `1d`)** — not named anywhere in the design docs, but S7's dirty-state work cannot happen without it |
| `ignore` | 0.4.33 | respecting ignore files in the files picker |
| `gix` | 0.86.0 | git adapter — pure Rust, no libgit2 build dependency |
| `jj-lib` | 0.44.0 | **optional**; shelling out to the `jj` binary is likely safer given how fast its API moves. Decide at S7. |
| `anyhow` + `thiserror` | 1.0.104 / 2.0.20 | errors in the binary / typed errors in libraries |
| `tracing` + `tracing-subscriber` | 0.1.44 / 0.3.23 | |
| `clap` | 4.6.6 | `phosphor --eval` |
| `unicode-width` / `unicode-segmentation` | 0.2.2 / 1.13.3 | grapheme correctness |
| `arboard` | 3.6.1 | clipboard — arrives via the vendored editor regardless |

---

## Hygiene tooling

Each of these is here because it catches a failure this project has already demonstrated it can
produce. Nothing on the list is a default.

| Tool | Version | The specific risk it catches |
|---|---|---|
| **`cargo-deny`** | 0.20.2 | **The one that matters most.** A `[bans]` rule denying multiple versions of `ratatui` / `ratatui-core` would have caught the `tui-textarea` and `ratatui-markdown` 0.29-vs-0.30 split *mechanically*, before either reached the plan. Also covers licences (two vendored forks, both MIT) and the advisory DB — which subsumes `cargo-audit`, so we don't need both. |
| **`insta`** | 1.48.0 | Tier 1 golden-frame snapshots (T018). Purpose-built for exactly this, with review tooling for intentional changes. |
| **`proptest`** | 1.11.0 | T017's statusline invariant — never two rows, at every width from 40 to 200. This is a property, not a set of examples. |
| **`divan`** | 0.1.21 | T079's benchmark: VM invocations flat while FPS climbs. Lighter and clearer output than `criterion` for a single tracked number. |
| **`cargo-hack`** | 0.6.45 | [Q4](IMPLEMENTATION-PLAN.md#q4)'s guardrail: the transcript must render with the markdown feature **on and off**. `--feature-powerset` proves it rather than trusting it. |
| **`cargo-nextest`** | 0.9.143 | Per-test process isolation. We have tests that touch the XDG state dir and terminal state; shared-process test runners make those flaky in ways that waste hours. |
| **`cargo-machete`** | 0.9.2 | Vendored forks accumulate dependencies we don't use — the editor fork alone arrives with 16 grammars, `arboard` and `rust-embed`. |
| **`typos-cli`** | 1.49.0 | The design language specifies voice down to the word. User-facing strings are a product surface. |
| **`cargo-sort`** | 2.1.4 | Seven crates' worth of `Cargo.toml` staying diff-friendly. |
| **`just`** | 1.58.0 | Already assumed by the plan (`vendor-diff`, `vendor-pull`, `tapes`). |

Plus two things that are configuration rather than tools, and belong in the same conversation:

- **`rust-toolchain.toml`, pinned.** `ratatui-code-editor` is `edition = "2024"`, so we need
  1.85+ regardless — but the real reason is determinism: VHS reference images (V001) are only
  comparable if the binary producing them is built the same way.
- **`[workspace.lints]`** in the root manifest. One clippy configuration shared across seven
  crates, rather than seven `#![warn(...)]` preambles that drift. This is also where the T006 /
  T007 structural lints get their teeth.

---

## What this changed — *all six folded in*

Applied to [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md) and [TASKS.md](TASKS.md). Three of
these moved decisions that were already recorded.

1. **[Q3](IMPLEMENTATION-PLAN.md#q3) inverted** and is marked as amending the Component
   Breakdown. `T026` is now "the input machine" outright, with **counts and named registers
   designed in from the start** — the two things the bought option couldn't express. `CP-3`
   calls them out as the ones to test hardest.
2. **Soft-wrap became `T081`** in S1, flagged unbudgeted. It builds as a `VisualRow` variant
   alongside folds and ghosts rather than as a layer above them, because row↔line mapping,
   cursor position, click targeting and virtual text all read that one row stream. `CP-1` gained
   an explicit check for it. `T016` narrowed to folds and whitespace marks.
3. **`T063` rebuilt on `similar`** — no bought base to restyle. Costs no new dependency, since
   `similar` already arrives transitively through the vendored crate.
4. **[Q2](IMPLEMENTATION-PLAN.md#q2) closed** — decision unchanged, mechanism rewritten to
   bypass upstream `History` rather than extend it.
5. **`T082`** drops `tree-sitter-csv` for a small parser; **`T083`** checks grammar ABI
   compatibility across the 0.23–0.25 bindings against the 0.26 runtime, and settles whether
   `tree-sitter-scheme` parses real Steel. `T083` runs in M-0, before `define-language` depends
   on it.
6. **`notify` + `notify-debouncer-full` joined the manifest** and `T069`, with debouncing called
   out as load-bearing — an agent writing a file produces a burst of events, and one `✱` per
   burst is the honest signal.

Retired from the risk register: the two spike risks. Added: soft-wrap, owning the input machine,
and grammar ABI mismatch.
