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
  with the smallest possible edit at the seam that calls into it. Patches 1 and 2 needed no
  such module — each is a gate over code that already existed. (**There is no patch 3.** The
  numbering is stable on purpose: entries are cited by number from `TASKS.md` and from the
  source, so a gap is cheaper than a renumber. This bullet said *"Patches 1–3"* until the
  review of the `CP-4` repair window counted the headings.) S1 created it, and it now holds
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
`PHOSPHOR_UNDERCURL` overrides; `NO_COLOR` degrades; then an `Smulx` `TERM` (kitty, ghostty,
wezterm, foot, contour, alacritty, rio) gets the curl and a multiplexer (`screen*`, `tmux*`) does
not; then **`TERM_PROGRAM`**; then a plain family (`xterm*`, `vt*`, `linux`, …) degrades.
**The allowlist points one way on purpose:** missing undercurl costs a flat underline; sending
`4:3` to a terminal that mis-parses sub-parameters costs visible garbage in the buffer.

**Amended at `CP-1` — `TERM_PROGRAM` moved ahead of the plain-family rule.** The original order
made `TERM` the authority throughout, which read well and was wrong in a specific way: iTerm2 and
VS Code both ship `TERM=xterm-256color` *and* both support `4:3`, so `SMULX_PROGRAMS`' own
`iterm.app` and `vscode` entries were unreachable and two capable terminals degraded for nothing.
Multiplexers are still decided first — tmux inside iTerm2 reports both, and it is the multiplexer
that has to carry the escape. The cost of the change is that `TERM` alone no longer forces
degradation, so a degradation capture must set `PHOSPHOR_UNDERCURL=0` explicitly; that is what
`tapes/_undercurl-check-forced-underline.tape` already does, and what `V009` should do when it
lands.

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

### 8 · Virtual text, as a variant of the row stream

**Files:** `src/phosphor/virtual_text.rs` (**new**, the types and the placement rule),
`src/phosphor/mod.rs` (one `mod`), `src/types.rs` (one `VisualRow` variant, one `is_changed`
arm), `src/view.rs` (one `use`, two fields, four accessors, one private `interleave_virtual`,
two one-line calls in `rebuild`, four match arms), `src/editor.rs` (one `use`, six methods, one
loop inside `soft_wrap_row_step`), `src/render.rs` (one constant, one style, one match arm).
**Upstreamable:** no. Virtual text is a phosphor concept end to end — the `┊` rail is Design
Language §2's glyph and the owner tag is a `RegionId`. Local, permanently.

`T032`'s primitive: `┊`-prefixed rows owned by a region id, indented to the code column, shared
by threads (`3a`), watches (`4b`), diagnostics (`6b`) and `T035`'s once-per-session unknown-key
hint (`8e`). Four consumers, one row type.

**Patch 6 wrote this patch's instruction down before it existed** and it is followed literally:
a virtual row is a variant *inside* `VisualRow`, never a layer above it, because row↔line
mapping, cursor placement, click targeting and virtual-text placement all read `View::rows`.
The arms that section named — `line_for_visual_row`, `visual_row_for_line`,
`visual_row_for_position`, `row_span`, `prev_line`, `next_line`, `is_changed` and the renderer's
match — are exactly the arms this patch adds.

```rust
VisualRow::Virtual { index, indent }
```

**It carries no `line_idx`, and that is the acceptance criterion.** A virtual row is not a line:
it prints no number, `line_for_visual_row` and `row_span` answer `None` for it, and it owns no
char span. So inserting one shifts nothing about the numbering of the rows below it, no column
resolves to it, and a click on one produces no cursor. It occupies a **visual row**, which is
why `Editor::virtual_line_at` exists — anything indexed by visual row (the state column,
`T031`'s gutter) has to skip these, the same way it skips `↪` continuations.

**The anchor is a position, not a line, and that is what makes wrapped lines work.** A
`VirtualLine` names `(line_idx, col)`; `virtual_text::apply` runs *after* `soft_wrap::apply` and
resolves the anchor by the same rule `View::visual_row_for_position` uses — the first segment
whose `end_col` is past the column, or the line's last segment when the column is past its end.
The `indent` the row inherits is that segment's own text start: 0 under a whole line or a first
segment, `CONTINUATION_PREFIX` under a `↪` continuation, which is what Design Language §3's
*"indents to code column"* means on a row whose code column moved.

**One arm was load-bearing and would have been easy to miss.** A row anchored to an early
segment sits *between* the segments of its own line, and `View::visual_row_for_position` walked
the run with a `_ if offset > first => break`. Without a `Virtual` arm that skips instead, every
column past the first virtual row resolved to the wrong segment — precisely the desync patch 6
warned about, reachable only once a virtual row existed to trigger it. There is a test for it in
`crates/phosphor-ui/src/virtual_text.rs`.

**Anchors are not maintained here.** A line naming a position the stream does not show — inside
a collapsed fold, or past the end of the buffer — is dropped rather than clamped: a thread that
scrolled out of the code it hangs from is invisible, not mispositioned. Re-installing the list
when anchors move is the host's job (`T042`/`T043`).

**Styles arrive resolved.** A `VirtualRun` is `(String, Style)`; the fork has no palette and
must not grow one. The single exception is the rail glyph itself, drawn from a `virtual_rail`
theme key through the same `set_theme_key` seam patch 7 added, falling back to `DarkGray` so a
standalone build still renders. `set_virtual_text_visible` hides the rows without discarding
them, which is what phosphor's `set-virtual-text-visible` Action needs.

**`soft_wrap_row_step` steps over them.** Vertical motion on a wrapped line with a thread under
it would otherwise stall on a row that holds no cursor.

---

### 9 · `Editor::input` and `Editor::mouse` are deleted

**Files:** `src/editor.rs` (two public methods removed, one orphan silenced).
**Upstreamable:** no. This is phosphor taking over a responsibility upstream offers, not a
defect in what it offers.

Recorded late, and that is the finding: `T026` removed both methods in Window D and no entry
was written for them. `scripts/lint-vendor-hunks.sh` did not catch it either — it checks that a
diverging *file* is documented, and `src/editor.rs` is documented several times over for other
patches. **Deleting an upstream public method is the largest divergence a fork can carry**, and
it was the one hunk with nothing written down.

**Why they went.** Both were second writers on state phosphor's invariants say has exactly one.
`Editor::input` ends every keystroke with its own `focus()`, and `Editor::mouse` calls
`scroll_up`/`scroll_down` directly — so with either of them live the viewport has two writers and
*"nothing moves unless you asked"* (invariant 3) stops holding. `Action::Scroll` is the single
writer, and `T026`'s input machine emits it. Wrapping them was not an option: a wrapper cannot
stop the callee from scrolling.

Mouse events still work. They are decoded by the host and lowered to `SetCursor`, `SelectRange`
and `Scroll` like any other input, which is what makes a click and a keystroke the same kind of
thing to everything downstream.

**One orphan, silenced rather than deleted.** `toggle_fold_at_mouse` was reachable only from
`Editor::mouse`, so it is now dead code and warns on every build. It carries `#[allow(dead_code)]`
with a pointer here, rather than a deletion, because it is upstream's own working code and the
day phosphor wants click-to-fold it is what that feature starts from. `T016` owns folding; the
fold *gutter* it targets is off (§4), so nothing calls this today.

---

### 10 · Change events are recorded when the edit happens, not when the batch commits

**Files:** `src/code.rs` (one field, one initialiser, one line in `tx`, one block each in
`insert` and `remove`, `notify_changes` rewritten and its argument dropped),
`tests/change_events.rs` (**new**, five tests).
**Upstreamable: yes, and it should be** — this is a defect in upstream's own code that any
consumer setting a change callback will hit. Nothing here is phosphor-specific; the patch is
portable verbatim and survives `just vendor-pull` as a conflict at worst.

`Code::notify_changes` took the finished batch's `edits` and turned each `start` into a
`(row, col)` **at commit time**, against the rope as it stood when everything had already been
applied. That is only correct for a batch whose offsets still address the finished text. An
undo step is exactly the batch where they do not: inverting a change reverses the edit order,
so the *first* edit reported carries the *highest* offset and the rope it is measured against
is the shortest it will ever be.

Two consequences, and phosphor shipped both:

* **A panic.** `Code::point` calls `Rope::char_to_line`, which unwraps. Open a file, type two
  characters at the end, press `u` — the inverse batch removes at offsets 6 then 5, the rope
  finishes five characters long, and `point(6)` panics inside `ropey`. Reproduced against the
  real binary through a pty: `exit=101`, *"Char index out of bounds: char index 6, Rope/RopeSlice
  char length 5"*. One character does not do it and neither does a group whose highest offset
  survives; **two or more edits whose top offset outlives the final length** is the shape.
* **Silently wrong positions when it does not panic.** A descending batch that stays in range
  still reports the line and column the offset has *afterwards*. **Latent in phosphor, live for
  everyone else** — and the distinction is worth stating precisely rather than dramatising.
  `track_dirty` (`crates/phosphor/src/main.rs`) installs the only callback this binary sets and
  it takes `|_|`: it raises a dirty flag and bumps an edit counter, and the positions are thrown
  away. `T038`'s `didChange` carries the whole document (`phosphor-buffer/src/lsp.rs`,
  `sync_kind` defaults to `FULL` and the `INCREMENTAL` shape covers the entire previous text), so
  no wrong range has ever left this editor. Upstream's own `examples/lsp` forwards the tuples
  straight into a change notification, which is whose bug this is.

**The fix is to record the event where the information exists.** `insert` and `remove` compute
their `(row, col)` before touching the rope and push the finished change tuple onto
`Code::batch_changes`, under the same `applying_history` guard that decides whether the edit
joins the batch at all; `commit` hands that vector to the callback and `notify_changes` no
longer reconstructs anything. `tx` clears it, so an abandoned batch leaves nothing behind.

**Deliberately guarded on `change_callback.is_some()`**, which keeps upstream's cost profile
exactly: no callback, no per-edit rope lookup and no `String` clone. The one behavioural edge
that buys is that a callback installed *mid-batch* now learns only about the edits after it was
installed, rather than about all of them. No caller does that — phosphor's `track_dirty` and
upstream's own `examples/lsp` both set it once at construction.

**Not fixed at our call site, on purpose.** The obvious phosphor-side workaround is to replay an
undo step as one `apply_batch` per edit, which costs a tree-sitter reparse per edit on batched
operators (`Editor::apply_batch` ends in `reset_highlight_cache`). The bug is upstream's and so
is the fix.

`tests/change_events.rs` is the regression suite, and `scripts/lint-vendor-tests.sh` is what
makes it run: `[workspace] exclude` keeps this fork out of `cargo nextest run --workspace`, so
until `T102` nothing in `just gate` had ever executed a single test in this directory — a fork
carrying nine patches over thirty-two upstream tests, none of which anything here ran. Each
of the three tests named for the defect fails on the unpatched crate: two panic inside `ropey`,
the third reports a column one to the right of the truth.

**Both counts were wrong and are recomputed here.** The headings above number 1, 2, 4–10 —
nine entries, not ten. And `grep -c '#\[test\]'` over the fork this session gives 42: `code.rs`
11, `diff.rs` 5, `tests/{editor,diff_focus,input}` 3 each and `tests/folding` 7 — thirty-two
upstream — plus phosphor's own `src/phosphor/cell_style.rs` 5 (patch 5) and this file's 5.
`git show 40ff181:src/code.rs` and `:src/diff.rs` carry the same 11 and 5, so no inline test
here is ours. "Thirty-seven" counted `cell_style.rs`'s five as upstream's.

---

## Known divergence between upstream and what we need — *not yet patched*

Recorded here so the next person reading `vendor-diff` knows what is coming, from the `T008`
spike ([SPIKES.md](../../docs/SPIKES.md)).

**This section moved below the patches, and a numbered entry does not belong in it.** §9 and §10
were both filed under this heading while being landed, tested patches — §9 set the precedent and
§10 followed it — so a reader scanning headings concluded the undo fix was unpatched. A new
divergence is a bullet here; a patch is a `### n` above, wherever it was written.

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
- ~~**Virtual text is absent, but `VisualRow` is the hook**~~ — **closed by patch 8.**
  `VisualRow::Virtual` is a variant of the row stream, and the arms patch 6 predicted
  (`line_for_visual_row`, `visual_row_for_position`, `row_span`, `is_changed` and the renderer's
  match) are the arms it added.
- **`mod diff` is private and the diff is a mode of the `Editor`, not a component.** `DiffBody`
  (`T063`) is built on `similar` instead.
- **Upstream's `Action` trait collides by name with phosphor's `Action` enum.** Cosmetic; worth
  a rename in the fork before the two ever appear in one file.
- **`crossterm` is off in our build** (the fork's `crossterm` feature gates
  `editor_crossterm.rs`). Input is phosphor's — `T026` — so we do not want upstream's key
  handling. Revisit only if something in S1 turns out to need that module.
