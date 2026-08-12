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
  with the smallest possible edit at the seam that calls into it. Patches 1–3 needed no such
  module — each is a gate over code that already existed. S1 created it, and it now holds
  `cell_style` (patch 5, undercurl) and `soft_wrap` (patch 6). A new addition belongs there
  too: a seam line in an upstream file is a merge conflict forever, a file upstream does not
  have is not.
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

> **A note on the numbering, before it bites someone.** The `PHOSPHOR PATCH n` markers in the
> source run one ahead of the headings above: the grammar gate is marked `PATCH 2` in
> `src/code.rs` and the clipboard gate `PATCH 3` in `src/editor.rs`, under headings 1 and 2.
> That predates this section. Renumbering the markers would be a whole-file churn hunk in a
> diff whose entire purpose is to stay small, so the offset stands and section 4 below takes
> the number its own markers carry. **Match the marker, not the heading**, when tracing a hunk.

### 4 · The gutter stops being hardcoded

**Files:** `src/editor.rs` (one field, three new methods, two visibility widenings),
`src/render.rs` (three hunks inside `Widget::render`).
**Upstreamable:** yes — every hunk is *"a constant becomes configurable, with the constant as
the default."* Nothing here changes what a standalone build of this crate renders.

`T015` composes phosphor's 3-column contract (Design Language §3) around this widget: a 1-cell
state bar, then line numbers *always* `#414b42`, then text. The spike's compose-around plan
(`SPIKES.md` seam 2) covers the state bar — we hand the widget a `Rect` inset by two cells and
overpaint the rest. Three things it could not cover, because they are decided inside
`Widget::render` from literals:

| what | upstream | why it had to move |
|---|---|---|
| line-number colour | `Style::default().fg(Color::DarkGray)` | §3 fixes it at `#414b42`, and the whole point of `T010`–`T013` is that a colour comes from the `Theme` |
| colour of text no highlight covers | `Style::default().fg(Color::White)` | identifiers, operators and punctuation carry no capture in any of the ten grammars; the mockups draw them `#c6cec6`, not white |
| minimum digits in the number column | `.max(5)` | every mockup file is two-digit and its gutter is six cells; upstream's floor makes it nine |

Seven hunks:

1. **`editor.rs`** — new field `line_number_min_digits: usize`, initialised to `5`.
2. **`editor.rs`** — `pub fn set_line_number_min_digits`, and `pub fn set_theme(Theme)`. The
   second exists because `Editor::new` takes the theme as `Vec<(&str, &str)>` of hex strings,
   and phosphor's caller is `phosphor-ui`, where a hex literal is a *lint failure*
   (`scripts/lint-no-literal-colours.sh`) — it owns `ratatui` `Style`s and had no way in.
   It also makes a live theme switch keep the buffer, the cursor and the viewport, which
   `CP-1`'s four-theme comparison needs. Invalidates the highlight cache, which bakes styles in.
3. **`editor.rs`** — `pub fn line_number_digits()`, the one place the digit count is computed.
   `get_line_number_width` and `render.rs` both called `.max(5)` on their own copy of the same
   expression; they now cannot disagree about where the text column starts.
4. **`editor.rs`** — `get_line_number_width` and `visual_len_lines` go `pub(crate)` → `pub`.
   A consumer composing its own gutter has to know where text begins, and a consumer that owns
   scrolling (invariant 3 — phosphor never lets the widget scroll itself) has to know how many
   rows there are to clamp against.
5. **`render.rs`** — the two styles above are read with `self.theme_style(key).fg.unwrap_or(…)`,
   the pattern this function already uses for `diff_added` / `diff_deleted` / `word_highlight`.
   Keys: `line_number` and `default_text`. Neither is a tree-sitter capture name, so neither
   can collide with a grammar, and both fall back to the previous literal when absent.
6. **`render.rs`** — the local digit computation calls `self.line_number_digits()`.
7. **`editor.rs`** — `pub fn set_marks_colored(Vec<(usize, usize, Color)>)`, beside upstream's
   `set_marks`, which parses hex strings. Same reason as `set_theme`: a `Color` formatted back
   to hex is a round-trip only `Color::Rgb` survives, and `phosphor-ui` cannot write a hex
   literal at all. `T015` needs it to prove that a mark arriving does not scroll the viewport;
   `T087` (region tints) is its real consumer. Purely additive — `set_marks` is untouched.

**What this patch deliberately does not do.** It does not touch the fold gutter, the selection
colour (`Color::DarkGray`, `render.rs`), or the fold-separator style — `T016` and the visual-mode
work own those. `T015` turns the fold gutter *off* (`set_code_folding_enabled(false)`) rather
than patching it, because it is two extra cells between the numbers and the text that no mockup
shows; `T016` will need `left_code_padding` to absorb it instead of sitting beside it.

### 5 · Undercurl, with an underline fallback — the cell-style capability

**Files:** `src/phosphor/mod.rs` + `src/phosphor/cell_style.rs` (**new**, the whole capability),
`src/lib.rs` (one `pub mod`), `src/editor.rs` (two fields, one initialiser, five methods),
`src/render.rs` (one `use`, one per-frame read, one per-cell layer), `Cargo.toml`
(`ratatui-core` `~0.1.0` → `~0.1.2`).
**Upstreamable:** partly. The capability is phosphor's (`T085`, Design Language §3) and stays
local; if upstream ever wants styled marks, `phosphor::cell_style` is the shape to offer.

Upstream's marks are `(start, end, Color)` — a background tint, no style, no priority
(`SPIKES.md` seam 1). §3 draws an anchored region as **"tint + undercurl"**: the tint is the
marks API's job and the undercurl was nobody's. `T040` (diagnostics) and `T068` (anchored
regions) are the consumers; this is the capability, landed at S1 so `V002` can settle *"does
undercurl survive VHS capture"* against a real implementation.

**Why it cannot be a `Style`.** `Modifier` has nine bits and none of them is curly; the SGR is
`4:3`, a sub-parameter form no ratatui backend emits. The only channel from a `Buffer` cell to
the terminal that carries arbitrary bytes is the cell's **symbol**, which every backend writes
verbatim — so the SGR pair rides there, wrapped around the glyph: `ESC[4:3m` `ESC[58;…m` glyph
`ESC[59m` `ESC[4m`. Self-contained per cell, and it restores the *straight* underline the cell's
own `Modifier::UNDERLINED` already told the backend about, so a partial redraw cannot leave a
neighbour curled.

**The degradation path is the absence of an addition.** A span always sets
`Modifier::UNDERLINED`; on a terminal without `4:3` the escape is simply never emitted and what
is left is a straight underline. There is no second code path to drift — a test asserts the two
renders are byte-identical apart from the symbol.

**Capability detection** is `UnderlineCapability::resolve`, a pure function of four environment
variables, with `UnderlineCapability::detect` (a `OnceLock`) the only part that reads the world.
`PHOSPHOR_UNDERCURL` overrides; `NO_COLOR` degrades; then **`TERM` is the authority** — an
`Smulx` name (kitty, ghostty, wezterm, foot, contour, alacritty, rio) gets the curl, a
multiplexer or a plain family (`xterm*`, `vt*`, `linux`, …) does not, and `TERM_PROGRAM` is
consulted only when `TERM` matched nothing at all. That ordering is what makes `V009`'s
`TERM=xterm-256color` tape capture the degraded path even when it is recorded from a terminal
that could have drawn the curl. **The allowlist points one way on purpose:** missing undercurl
costs a flat underline; sending `4:3` to a terminal that mis-parses sub-parameters costs visible
garbage in the buffer.

**`CellDiffOption::ForcedWidth` is load-bearing, and the reason the dependency floor moved.**
`Buffer::diff` measures a cell by the display width of its symbol; ~30 bytes of escape measures
~30 columns and the backend silently skips the rest of the line. Verified on a real pty before
the fix — a third of the row was missing from the wire while the `Buffer` was perfectly correct.
`ForcedWidth` exists for exactly this case (*"escape sequences will have some computed width
that does not match what is written to the screen"*) and landed in `ratatui-core` 0.1.2, hence
the one-word `Cargo.toml` hunk. The workspace already pins 0.1.2; the fork's own `Cargo.lock`
still records 0.1.0 and cargo refreshes it on the next standalone build.

**Cost, and the optimisation deliberately not taken.** ~28 bytes per undercurled cell, because
each cell re-establishes and restores the style. A run-length form (open the curl once per span,
close it at the end) would be cheaper and would break the moment ratatui's diff redrew part of a
span, which is the common case. Regions are short; correctness under partial redraw is not
negotiable.

**Where the fixture call site lives — deliberately not in this fork.**
`crates/phosphor-buffer/examples/undercurl.rs` and `crates/phosphor-buffer/tests/undercurl.rs`.
Upstream's own `examples/marks` was the obvious template, but a demo is not something phosphor
has to carry across every upstream merge: this patch is the capability and nothing else.

---

### 6 · Soft wrap, as a variant of the row stream

**Files:** `src/phosphor/soft_wrap.rs` (**new**, the wrap engine), `src/phosphor/mod.rs` (one
`mod`), `src/types.rs` (one `VisualRow` variant, one `is_changed` arm, the new `RowSpan`),
`src/view.rs` (one field, two accessors, two one-line calls in `rebuild`, four match arms, two
new mapping helpers), `src/editor.rs` (one `use`, seven methods, three hunks inside `focus`,
`cursor_from_mouse` and `get_visible_cursor`), `src/render.rs` (one `use`, four constants, one
style, one match arm, three hunks in the row body), `src/actions.rs` (one guarded early return
in each of `MoveUp` and `MoveDown`).
**Upstreamable:** in principle yes — it is a feature upstream lacks entirely and adds no
phosphor concepts — but it is large, and offering it would mean owning its review. Local.

**There is no soft wrap anywhere in this crate** (`SPIKES.md`, T008's bonus finding): `VisualRow`
has no wrapped variant and no wrapping logic exists. Design Language and mockup `8e` require `↪`
continuations that carry no line number. That is `T081`, unbudgeted, and it landed at S1 because
four subsystems read the same row stream and all four had to learn about it at once.

**The instruction that shaped every line of this patch:** soft wrap is a variant *inside*
`VisualRow`, beside `FoldSeparator` and `GhostDeleted` — never a layer above it. The renderer,
the row↔line mapping, cursor placement, click targeting and (at `T032`) virtual-text placement
all read `View::rows`, and a wrap that lives outside that list desynchronises all of them, in
ways that surface months later as off-by-one-row bugs in unrelated surfaces.

```rust
VisualRow::Wrapped { line_idx, segment, start_col, end_col, is_added, orig_line_idx }
```

**A line that fits stays `Real`.** Only a line that does not becomes a run of `n >= 2` `Wrapped`
rows. So with wrapping off — or on, over a buffer of short lines — the stream is byte-for-byte
what upstream builds, which is why all 37 upstream tests pass unchanged.

**The contract, which is the patch's real product.** `View::row_span` (→ `Editor::row_span`)
answers, for one visual row: which line, which segment, `[start_col, end_col)`, how many cells
the row spends on its marker, and whether it ends its line. Every consumer resolves a row through
that one function, so they cannot disagree about what a row shows. Three properties hold and are
tested in `crates/phosphor-ui/src/soft_wrap.rs`: a line owns a **contiguous** run of rows; the
spans **partition** the line (no column on two rows, none on none); and a column resolves to
**exactly one** row via `visual_row_for_position`, which is the hook `T032` will hang virtual
text from.

**Wrapping is by cells, not chars**, using the same `grapheme_width_and_chars_len` the renderer
measures with. It breaks at the last space that fits and hard-breaks mid-word when there is
none; the break space stays on the row before it. A continuation measures against a width two
cells narrower, because that is what `↪ ` costs it.

**The four consumers, and what each needed:**

| consumer | change |
|---|---|
| `render.rs` | a `Wrapped` arm that draws the row's own span rather than a window on the line, a blank number column past segment 0, and the `↪ ` marker |
| `Editor::get_visible_cursor` | the cursor's **segment** via `visual_row_for_position`, its column measured from that segment's start, plus the marker's cells |
| `Editor::cursor_from_mouse` | the click measured inside the row's span, past the marker; a click past a non-final segment's end stays on the row that was clicked rather than falling to the next |
| `Editor::focus` | reveals the cursor's segment, not its line's first row; and pins `offset_x` at 0, because wrapped text has nowhere to scroll sideways to |

**`MoveUp`/`MoveDown` became visual-row motion, and only when wrapping is on.**
`Editor::soft_wrap_row_step` returns `None` with wrapping off, so upstream's line-wise path is
what runs; there is one code path per behaviour and no flag threaded through the actions. These
two arms exist because S1 rides `editor_crossterm` as its **temporary** input path — `T026`
replaces it, and when it does, `soft_wrap_row_step` is the primitive phosphor's own motion will
call instead.

**What this patch deliberately does not do.** Diff ghosts do not wrap: a ghost row's text comes
from the *original* buffer, and `DiffBody` is `T063`, built on `similar` rather than on this
view. Wrapping is refused below four cells rather than degrading into one character per row.
And nothing here caches: `wrap_segments` runs on rebuild, which happens when the buffer, the
folds or the width change — not per frame.

---

### 7 · `8e`'s other two text details — the fold marker and whitespace marks

**Files:** `src/editor.rs` (two fields, one initialiser, four methods, two guard conditions),
`src/view.rs` (one method), `src/render.rs` (three constants, two styles, one pre-loop
computation, one in-loop branch, one post-loop block).
**Upstreamable:** the fold-gutter/folding split is (it is a conflation upstream would probably
accept as a bug); the marker text and the whitespace marks are phosphor's and stay local.

`T016`'s acceptance is *"screen `8e`'s fold and whitespace details reproduce"*, and `8e` draws
both inline in the text column:

```
 12  pub fn retry_with_backoff<T, E>( ▸⋯ 13 lines
 28      resp.json().await.map_err(FetchError::Decode)··
```

**The fold is upstream's; only the marker is ours.** Collapsing a code fold already removes the
hidden lines from the row stream (`View::rebuild` filters them), so nothing about the mechanism
changed. What is added is `View::code_fold_hidden_lines` — the count — and a render block that
draws ` ▸⋯ n lines` after the header line's text, in the theme's `fold_marker` colour. Glyphs
are Design Language §2's (`▸` fold closed, `⋯` elided); the count is singularised at 1.

**Folding had to become separable from its gutter.** Upstream's `code_folding_options.enabled`
means both "folds work" and "there is a fold gutter column", and `T015` turned it off to get rid
of the column — which also turned off folding, so `8e` was unreachable. The new
`fold_gutter_visible` field splits them: `fold_gutter_width()` returns 0 when the column is
hidden, and `code_fold_indicator()` — which is *only* the gutter glyph — returns `None`, because
with a zero-width gutter it would otherwise draw over the first cell of the text. A field rather
than a `CodeFoldingOptions` variant deliberately: adding a field to that public struct would
break upstream's own `tests/folding.rs`, which constructs it by literal.

**Whitespace marks are insert-only and know nothing about modes.** The fork gets
`set_show_trailing_whitespace(bool)`; `phosphor-ui`'s `soft_wrap::set_mode` is what ties it to
INSERT, because the mode enum is `spine`'s and does not exist yet (`T026`). A row's trailing run
is computed once before its grapheme loop and each cell in it renders `·` patched with the
theme's `trailing_whitespace` style — §3's failure tint under the trouble hue, which is exactly
what `8e` draws. A line that is nothing but whitespace is trailing whitespace from column 0,
which is what vim's `trail` listchar does.

**One more theme seam.** `set_theme_key(key, style)` adds a single entry to the theme map
instead of replacing it wholesale, so the three non-capture keys this and patch 6 introduce
(`wrap_indicator`, `fold_marker`, `trailing_whitespace`) can be installed without rebuilding the
syntax map. All three fall back to the previous constants when absent, so a standalone build of
this crate renders exactly as it did.

---

## Known divergence between upstream and what we need — *not yet patched*

Recorded here so the next person reading `vendor-diff` knows what is coming, from the `T008`
spike ([SPIKES.md](../../docs/SPIKES.md)):

- ~~**No soft-wrap anywhere in the crate.**~~ — **closed by patch 6.** `VisualRow::Wrapped` is a
  variant of the row stream, and `RowSpan` is the contract every consumer of that stream reads.
- **Marks are `(start, end, Color)` with no id and no style**, replaced wholesale
  (`editor.rs`). Carries region tints; cannot carry the gutter contract. ~~or undercurl~~ —
  **the undercurl half is closed by patch 5**, which puts styled spans beside the marks rather
  than inside them. Marks still have no id and no priority, and the state column is still
  phosphor's own overpaint (`T031`).
- ~~**The gutter is not injectable**~~ — **closed by patch 4.** The compose-around still stands
  (`set_left_code_padding` plus an inset `Rect`, and phosphor overpaints the state bar and the
  `~` rows), but the line-number style and the digit floor are now theme-driven rather than
  literals.
- **Virtual text is absent, but `VisualRow` is the hook** — the renderer iterates visual rows,
  not source lines, and fold separators already insert non-source rows. An enum arm. Patch 6 is
  the worked example of adding one: `line_for_visual_row`, `visual_row_for_line`,
  `visual_row_for_position`, `row_span`, `prev_line`, `next_line`, `is_changed` and the
  renderer's match are the arms `T032` will have to add too.
- **`mod diff` is private and the diff is a mode of the `Editor`, not a component.** `DiffBody`
  (`T063`) is built on `similar` instead.
- **Upstream's `Action` trait collides by name with phosphor's `Action` enum.** Cosmetic; worth
  a rename in the fork before the two ever appear in one file.
- **`crossterm` is off in our build** (the fork's `crossterm` feature gates
  `editor_crossterm.rs`). Input is phosphor's — `T026` — so we do not want upstream's key
  handling. Revisit only if something in S1 turns out to need that module.
