# Tapes — the Tier-2 verification harness

VHS-captured PTY recordings: real terminal escape sequences → PNG frames / GIFs,
compared against committed references. Proves what actually appeared on screen,
which Tier-1 snapshot tests (ratatui `TestBackend`) structurally cannot — see
`docs/TASKS.md`'s "three verification tiers" for the full split.

`V001` built the harness tapes run inside: pinned tool versions, the version
gate, and the `just tapes` entry point. `V002` added the first `_`-prefixed
reference file — the column-width table. `V003`–`V005` (this phase) add the
shared config fragment, the no-`Sleep` / sentinel convention, and the first
real per-screen tapes and recipes — see the three sections below, in that
order (each is `Needs:` the one before it in `docs/TASKS.md`).

**Every real tape in the library records clean.** This paragraph used to say
the opposite — that nothing could pass because `crates/phosphor/src/main.rs`
was still `fn main() {}` — and that was true right up until `T090` landed the
S1 host. It is no longer, and the stale wording was corrected during the
`CP-1` gate pass (it contradicted "Screen library convention" below, which is
the current and correct account). Building the binary and putting it on
`$PATH` is still yours to do; nothing in this repo does it for you.
See "Screen library convention" for what each tape captures.

## Pinned versions

Pixel comparison is only meaningful against a fixed renderer — a font
substitution, a `ttyd` rendering change, or a `vhs` release that reflows
padding all produce a diff that means nothing. So every tool in the capture
pipeline is pinned by exact version, and `just tapes` refuses to run against
anything else.

| tool | pinned version | checked by |
|---|---|---|
| `vhs` | `0.11.0` | `tapes/check-versions.sh` |
| `ttyd` | `1.7.7` | `tapes/check-versions.sh` |
| `ffmpeg` | `8.1.2` | `tapes/check-versions.sh` |

`ttyd`/`ffmpeg` are pinned to what was actually installed and verified working
on the reference machine below, per SPIKES.md's instruction to pin what we
have rather than an arbitrary version. Bumping any of the three is a
deliberate, one-line edit to `tapes/check-versions.sh`, done alongside
re-regenerating every reference image — never a silent drift.

## Reference-regeneration machine

References (once V002+ exist) are only comparable to a fresh capture if both
came off the same renderer. Regenerate them only on hardware matching this
profile, and update this record — not just the images — the day that changes:

- **Model:** Mac mini (Mac16,10), Apple M4, arm64
- **OS:** macOS 26.5 (build 25F71), Darwin 25.5.0
- **Toolchain:** whatever `rust-toolchain.toml` pins at the root (currently
  `1.97.1`) — the binary being captured must be built the same way every time,
  per SPIKES.md's reasoning for pinning it at all. **Raising that pin
  invalidates every reference image**, which is why it is a deliberate,
  reviewed commit rather than a drive-by bump

## Font

VHS's own built-in default (unpinned) is a ten-font CSS fallback chain headed
by `JetBrains Mono` (`vhs.go`, `defaultFontFamily`: `JetBrains Mono, DejaVu
Sans Mono, Menlo, Bitstream Vera Sans Mono, Inconsolata, Roboto Mono, Hack,
Consolas, ui-monospace, monospace` + an `Apple Symbols` fallback). That is
exactly the nondeterminism this file exists to remove: neither of the first
two fonts is installed on the reference machine above, so the *unpinned*
default silently resolves to `Menlo` here today, and would silently resolve to
`JetBrains Mono` on any machine that happens to have it — same tape, different
pixels, no diff that explains why.

**Pinned font: `Menlo`**, at **size `16`** — decided by `V002`, since sizing is
what the column calibration below needed fixed before it could mean anything.
Menlo ships with every Mac (`/System/Library/Fonts/Menlo.ttc` — confirmed
present on the reference machine, zero install cost) and is what the unpinned
default already falls through to here, so pinning it costs nothing and matches
today's captures if any exist before `_config.tape` is written explicitly.
16 was not derived from anything in the design docs — nothing pins a size —
it's simply a normal, legible terminal size that produces whole-pixel cell
metrics (10px/column, see below); any future change to it invalidates the
whole calibration table and needs a full re-run of `V002`'s method.

This becomes `Set FontFamily "Menlo"` + `Set FontSize 16` lines in
`tapes/_config.tape` (`V003`, Window B — `Source`d by every tape). Recorded
here first because V001's brief is where the pin belongs; V003 is where it
becomes code.

## Column calibration (V002)

VHS sizes the terminal in pixels (`Set Width`/`Set Height`), never columns.
`tapes/_dimensions.tape` is the committed `(FontSize, Width) -> columns`
table, derived by binary search against a real `tput cols` readout inside the
capture (not computed/assumed) — full methodology, the table, and three
non-obvious parser/renderer gotchas are documented in that file's header.
Short version for reference here:

| columns | `Set Width` (px) |
|---|---|
| 80  | 828  |
| 100 | 1028 |
| 120 | 1228 |
| 200 | 2028 |

(Full table with valid ranges lives in `_dimensions.tape` itself — this is a
pointer, not a second copy to keep in sync.) Requires `Set Padding 0`,
`Set Margin 0`, `WindowBar` unset, and — this bit is load-bearing, not
style — **an even `Set Width`**; an odd value breaks VHS's ffmpeg pipeline
outright (`Padded dimensions cannot be smaller than input dimensions`, no
GIF, no Screenshot). `_dimensions.tape` also carries a `vhs`-parser gotcha
that affects `V003`: any `Output`/`Screenshot`/`Source` path starting with
`_` must be quoted (`Source "_config.tape"`, not `Source _config.tape`) or
the tape fails to parse.

**Proven reproducible**, per V002's *"done when"*: the calibration tape's
`Wait+Screen` assertion for exactly 80 columns passed on every run in the
investigation (n>=10, two different working directories), and its
`Screenshot` output hashed byte-identical (sha256) across independent runs
where it succeeded. `Screenshot` itself is separately flaky specifically
when `vhs` runs with cwd inside this worktree (~50% miss rate, no error, no
nonzero exit) — not a V002 blocker (nothing here depends on the PNG existing
to prove the count), but flagged for `V004`/`V007` since a pixel-diff runner
that treats a missing PNG as *the* failure will misreport this as a product
regression. Not reproduced outside the worktree (3/3 clean in `/tmp`).

## Open empirical questions (owned by V002) — both now closed

Two questions V002 was asked to settle, closed in two passes as their
surfaces landed. `T010` (a real theme, see `crates/phosphor-ui/src/theme.rs`)
answered the first one against real values earlier in this window; `T085`
(undercurl in the vendored renderer) answered the second once it existed to
capture — see its own method below rather than being inferred from VHS's
documentation, per V002's brief.

- **Do captured pixels match theme hex values exactly? — Answered: no, not
  byte-exact, but close enough that a small tolerance is the fix, not a
  problem to solve.**

  Method: paint raw 24-bit truecolor blocks (`printf '\033[48;2;R;G;Bm ...'`)
  for four values actually in `Theme::phosphor_dark()` — `claude` `#3ddc97`,
  `neutrals.text` `#c6cec6`, `actors.attention` `#e0a94e`,
  `neutrals.ground` `#0c0f0c` — plus pure white `#ffffff` as a control, under
  the same `FontSize 16` / `Padding 0` / `Margin 0` pin as the column table.
  Screenshot, sample the flat interior of each block (away from
  antialiased edges) with two independent readers (Python/Pillow and
  ImageMagick `identify` — agreed exactly, so it isn't a reader artifact).

  | theme value | hex | expected RGB | captured RGB | delta |
  |---|---|---|---|---|
  | `actors.claude` | `#3ddc97` | (61, 220, 151) | (60, 219, 150) | (-1, -1, -1) |
  | `neutrals.text` | `#c6cec6` | (198, 206, 198) | (198, 205, 198) | (0, -1, 0) |
  | `actors.attention` | `#e0a94e` | (224, 169, 78) | (224, 168, 78) | (0, -1, 0) |
  | `neutrals.ground` | `#0c0f0c` | (12, 15, 12) | (12, 14, 11) | (0, -1, -1) |
  | control white | `#ffffff` | (255, 255, 255) | (255, 255, 255) | (0, 0, 0) |

  Deterministic, not noise — re-ran the `claude` capture from a clean state
  and got the identical (60, 219, 150) both times. Small (never more than 1
  of 255 in any channel, ~0.4%, invisible by eye), one-directional (every
  observed delta is 0 or -1, never +1), and channel-dependent rather than a
  flat "-1 to everything" (white round-trips exact; the others each lose 1
  in one or more channels). Root cause not chased down — somewhere in the
  headless-Chromium-canvas -> PNG path, not `vhs`'s terminal emulation of
  the escape code itself, but which specific stage wasn't isolated.
  **Consequence for `V007`:** the pixel-diff runner cannot assert byte
  equality against a theme hex; it needs a small per-channel tolerance
  (>=1) baked in from the start, or every reference will show a spurious
  1-bit diff on channels that never actually changed.

- **Does undercurl survive VHS capture? — Answered: yes.** The rendered pixels
  are distinguishable from the underline degradation on three independent
  signals, not just eyeballed.

  Method: `T085`'s fixture (`crates/phosphor-buffer/examples/undercurl.rs`,
  reachable without `crates/phosphor/src/main.rs`, which is still a stub this
  window — the whole reason the fixture is a standalone example) renders one
  call site three ways, each its own `_undercurl-check-*.tape` (investigation
  tapes, `_`-prefixed like `_dimensions.tape`, run manually, not part of the
  V005 screen library):

  | tape | what it forces | what it answers |
  |---|---|---|
  | `_undercurl-check-forced-curl.tape` | `PHOSPHOR_UNDERCURL=1` | does the SGR `4:3` escape survive ttyd → xterm.js → headless-Chromium → PNG |
  | `_undercurl-check-forced-underline.tape` | `PHOSPHOR_UNDERCURL=0` | the degradation-path control: same span, same colour intent, no escape |
  | `_undercurl-check-auto.tape` | nothing (real detection) | what `UnderlineCapability::resolve` sees VHS itself as |

  All three captured clean (Width 828 / Height 240, `_config.tape`'s pins).
  Screenshots at `tapes/artifacts/undercurl-check-{auto,forced-curl,forced-underline}.png`.

  **Signal 1 — not byte-identical.** `forced-curl` and `forced-underline` hash
  differently (sha256 `29db34b8…` vs `88fa34fc…`); `auto` hashes **identical**
  to `forced-underline` — VHS's own ttyd session reports `TERM=xterm-256color`
  (visible in the fixture's own on-screen legend), which
  `crates/phosphor-buffer/tests/undercurl.rs`'s
  `the_capability_is_detected_from_the_environment` test already asserts
  resolves to `UnderlineCapability::Underline`. **Consequence for `V009`:** VHS
  records the *degraded* look by default; capturing the primary-terminal
  undercurl treatment needs `PHOSPHOR_UNDERCURL=1` (or an equivalent forced
  override) in the tape, not just pointing VHS at the fixture.

  **Signal 2 — colour.** Sampling the `anchored` span (anchor-undercurl,
  `#2a5c44`) and the `expect("unanchored")` span (failure-undercurl,
  `#d97b6c`) with Pillow: `forced-curl` shows a green-tinted band under
  `anchored` (greenness 6–10 across every column of the span, background/text
  noise floor is ~4) and a red-tinted pixel in 189/320 columns under
  `expect(…)`; `forced-underline` shows **zero** columns with either tint —
  the degraded path drops the requested colour entirely, it does not
  approximate it.

  **Signal 3 — shape.** A per-row luminance profile under `anchored`
  (`x=140..207`, the span's exact width) is the clearest evidence of the two:
  `forced-curl`'s ink is spread across four rows (`y=18..21`) with *partial*,
  varying coverage per row (9/67, 57/67, 67/67, 22/67 columns) — the signature
  of a line whose y-position moves with x, i.e. a wave. `forced-underline`'s
  ink is a flat, full-width, two-row-thick band (67/67 at both `y=18` and
  `y=19`, zero elsewhere) — a straight line, no waviness at all. Same font,
  same span, same background; the only variable was the escape.

  Not re-run for a second-sample determinism check the way Q1 was (the byte-
  hash match between `auto` and `forced-underline` across two full `vhs`
  invocations already demonstrates the pipeline is deterministic here too),
  but the three signals agree with each other and with the fixture's own
  unit tests, which is a stronger check than a bare re-run would have been.

  **Consequence for `V007`/`CP-1`:** a pixel-diff reference for an undercurled
  region is meaningful — the capability is not silently downsampled to a
  straight line the way, say, a font substitution would. Tier 2 can assert
  "this region is undercurled" the same way it asserts colour, so long as the
  capturing tape forces or genuinely runs on a `TERM` that resolves to
  `UnderlineCapability::Undercurl` — VHS's own default does not.

## V003 — shared tape configuration

`tapes/_config.tape`, `Source`d by every real tape right after `Require` and
before that tape's own `Set Width`/`Set Height` (the position matters — see
the file's own header and `_dimensions.tape`'s gotcha #2). It pins everything
V003 asks for: `FontFamily`/`FontSize` (V001/V002's pin), `Padding 0`,
`Margin 0`, `Shell "bash"`, `CursorBlink false`, a fixed `TypingSpeed` and
`Framerate`, and a neutral background — sourced from
`Theme::phosphor_dark()`'s own `neutrals.ground`/`neutrals.text`
(`crates/phosphor-ui/src/theme.rs`) rather than an invented placeholder or
one of vhs's ~250 bundled named themes.

**Reproducibility proof** (`tapes/_config-check.tape`, the same
`_`-prefixed reference-and-runnable-proof pattern as `_dimensions.tape`):
sources `_config.tape`, types a fixed line, waits for it, screenshots.
Deliberately does not drive `phosphor` — main.rs is still a stub this
window — so it isolates the config fragment's own determinism. **Three
independent runs, same working directory, all three byte-identical:**

```
sha256 2be4295e7ea6a21bac5fc7458663e1e0e4ca516aa34aabc457633e79f1e1303b  config-check.png
```

(All three runs also succeeded on the first try — `_config-check.tape`
never hit the `Screenshot`-flakiness `_dimensions.tape` documented, though
that's not proof it's immune; re-run a few times if it ever misses.)

## V004 — deterministic waits, no `Sleep`

Every tape in the library synchronises with `Wait+Screen@<timeout>
/<regex>/` against a real rendered frame — never a bare `Sleep`. Checked
mechanically: `grep -n Sleep tapes/*.tape` returns nothing (`_config.tape`
and the reference/proof files included).

**The sentinel.** The statusline is the natural ready-state signal (it's
the last thing every real frame draws), and `T017` landed *during* this
phase — so this is read off the actual implementation
(`crates/phosphor-ui/src/status_line.rs`), not guessed at:

```
(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b
```

Why this and not the `│` segment separators the design doc's prose
emphasizes: `status_line.rs`'s own code shows the mode chip is the **one**
piece that is never shed to nothing (only `NORMAL` → `N`, at very narrow
widths) and always the first thing written to the row — see `compose()`,
where `left` starts life as `vec![Piece::new(format!(" {word} "), chip)]`
unconditionally. The `│` separators, by contrast, only appear *between*
two or more right-hand segments (`if i > 0 { row.write(SEP, ...) }`) — at
S1, with no live ACP session (`SessionState::None`) and no store-backed
counters yet, the right-hand group could easily be down to zero or one
piece, in which case no `│` ever renders and a `│`-based wait would hang
until timeout on a perfectly correct frame. The mode word has no such
failure mode: `Mode::Normal` is `StatusLineVm`'s `Default`, so a freshly
opened file renders it before a single keystroke.

Assumes the statusline spans the full terminal width starting at column 0
(§5: "statusline, bottom, always" — the standard case, not the widget's own
unit tests, which use an inset test-harness `Rect` for coverage reasons).
If the temporary S1 main.rs ever boots into a mode other than Normal, or
insets the statusline for some reason, this regex still matches — all four
mode words are covered and the leading-space anchor doesn't depend on which
one shows up.

**Timeout.** Every real tape uses `Wait+Screen@10s`, not vhs's implicit
default — pinned for the same reason everything else in this file is
pinned: an explicit number that fails predictably beats an implicit one
that might change with a vhs upgrade. `10s` is generous for "a file opens
and one frame renders"; tighten it once real timings are known.

## V005 — screen library convention

One tape per screen id: `tapes/<id>.tape` → `Require phosphor` →
`Source "_config.tape"` → per-tape `Set Width`/`Set Height` (from
`_dimensions.tape`'s table) → `Hide` the setup keystrokes → `Show` →
`Wait+Screen` on the V004 sentinel → `Screenshot "artifacts/<id>.png"`.
`Output "artifacts/<id>.gif"` is present on every tape (vhs writes no
`Screenshot` at all without an `Output` — confirmed empirically, not
documented upstream) even where motion isn't the point; the GIF is a free
byproduct there, not the reviewed artifact.

**Recipes** (`justfile`): `just tape <id>` regenerates one screen —
`vhs tapes/<id>.tape`, run with cwd `tapes/` so relative paths
(`Source "_config.tape"`, `Screenshot "artifacts/..."`) resolve the same
way as they do under `just tapes` → `run-tapes.sh` (which also `cd`s into
`tapes/`). Both go through the same version gate first. `tapes/artifacts/`
is a real, committed directory (`.gitkeep`) — vhs does not create missing
parent directories for `Screenshot`/`Output`, confirmed by testing it.

**Library status today — every real tape passes.** `T090` (Window B, S1 host)
landed `crates/phosphor/src/main.rs` and `--theme <slug>`, so the two blockers
this section used to describe are both closed. `just tapes` (binary built via
`cargo build --release --bin phosphor`, put on `$PATH` by hand — nothing in
this repo does that for you yet) records all sixteen real tapes clean:
`1a`, `9c`, `8c`, `8d`, `sweep-{200,120,100,80,60,40}`,
`theme-{phosphor-dark,phosphor-light,catppuccin,catppuccin-latte,tokyo-night,
tokyo-night-day}`. The four-theme sweep is now the **six-theme** sweep — all
six `BUILTIN_SLUGS`, not the four this section originally scoped, closing the
gap CP-1's own task brief flagged. `theme-catppuccin`/`theme-catppuccin-latte`
and `theme-tokyo-night`/`theme-tokyo-night-day` are the only captures of
mockup ids `9a`/`9b`'s acceptance *shape* (IMPLEMENTATION-PLAN.md's S1
amendment) — no separate `9a.tape`/`9b.tape` exists, this family is their
home.

**Two harness bugs found running this for real, both fixed in the tapes
themselves (not the product):**

1. **A capture-pipeline race, not a product tear.** `Hide` … `Enter` … `Show`
   → `Wait+Screen` → `Screenshot`, run against `phosphor` specifically
   (a raw-mode, alt-screen, synchronized-output TUI — not the plain-shell
   `Type`/`Enter` `_config-check.tape` exercises), intermittently either
   failed outright (`vhs`: `no frames` / `recording failed`) or produced a
   *structurally short* PNG (~4KB vs ~220KB for the same matched state,
   missing the state bar entirely, cursor stuck top-left) — i.e. the text
   buffer `Wait+Screen` matches against and the pixels headless-Chromium has
   actually painted are not the same clock. Isolated by bisecting the
   sequence (`Hide`/`Show` placement, `Sleep` before/after `Enter`/`Wait`);
   reproduced deterministically on demand, fixed by a `Sleep 500ms` **between
   `Wait+Screen` and `Screenshot`** in every real tape — 3 back-to-back runs,
   byte-identical sha256 each time. This is a deviation from V004's "no bare
   `Sleep`" convention, done deliberately and flagged rather than silently:
   `Wait+Screen`'s regex is still the correctness gate, this is a settle
   guard for the *screenshot* only, same category as this file's own
   already-documented Screenshot flakiness in this sandboxed worktree
   (below) — a second, distinct manifestation of "the capture pipeline has
   more moving parts than the regex accounts for," not a second copy of the
   same bug. Each real tape's `Wait+Screen` line carries a comment pointing
   back to `1a.tape`'s full writeup.
2. **The V004 sentinel regex didn't account for `T017`'s own shed order.**
   `sweep-40.tape` timed out for real: at 40 columns the mode chip has
   already shed `NORMAL` → `N` (§11's ladder, confirmed live — see the CP-1
   findings below), so `^ (NORMAL|INSERT|VISUAL|PAUSED)\b` never matches at
   the narrowest sweep width. Fixed by widening every real tape's sentinel to
   `^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b` — the shed-to-initial forms
   from `status_line.rs` itself, not guessed.

Both fixes are mechanical, in `tapes/**` only, and were necessary to produce
any capture at all above ~60 columns worth of content — they are not tuning
toward the mockup (see the shed-order finding below, which is exactly the
opposite: the capture is left showing the build's real, un-tuned behaviour).

**Still true, and now more interesting than a placeholder note:**
`_dimensions.tape`'s Screenshot flakiness in this sandboxed worktree (gotcha
#4) is real and reproduces on `phosphor` too — `sweep-40.png`,
`sweep-60.png`, and `theme-tokyo-night-day.png` each needed one extra
`vhs <tape>.tape` re-run this pass before the PNG landed (GIF and the
`Wait+Screen` match were clean on the first try every time; only the bonus
`Screenshot` file intermittently didn't appear, exactly as documented). Not
a product concern; `just tapes`/`run-tapes.sh` don't retry, so a from-scratch
regen of this exact library may need a manual `just tape <id>` rerun for
whichever one or two tapes land short — check `ls tapes/artifacts/*.png`
against the tape count after a fresh run.

## Convention: every tape gets a `Require`

Every `.tape` file must open with a `Require <program>` line for each external
binary the tape's *content* drives — most obviously `Require phosphor`, and
anything else a tape shells out to. VHS's own `Require` command
(`vhs new`'s generated header) checks the program is on `$PATH` before running
a single frame and fails the tape immediately if it's missing, rather than
producing an empty or silently-wrong recording that only fails a pixel diff
much later. `vhs`, `ttyd`, and `ffmpeg` themselves are not `Require`d per-tape
— they're checked once, by version, before any tape runs (see `just tapes`
below) — because a tape doesn't invoke them by name, `vhs` invokes the tape.

## Layout

```
tapes/
  README.md              this file
  check-versions.sh      the version gate `just tapes` / `just tape <id>` run first
  run-tapes.sh            runs every real tapes/*.tape (`_`-prefixed skipped)
  _dimensions.tape         V002 — the column-width calibration table (+ V005's 40/60 rows)
  _config.tape              V003 — Source'd by every real tape
  _config-check.tape         V003 — its reproducibility proof
  _undercurl-check-{auto,forced-curl,forced-underline}.tape
                              V002 — the second open question's investigation,
                              run manually, not part of the V005 screen library
  _soft-wrap-check.tape       CP-1 investigation — does the `↪` continuation
                               marker read on a real captured frame, not just
                               a Tier-1 grid dump. Run manually.
  1a.tape, 9c.tape, 8c.tape, 8d.tape   V005 — the four CP-1 stills
  sweep-{200,120,100,80,60,40}.tape     V005 — the CP-1 width sweep
  theme-{phosphor-dark,phosphor-light,catppuccin,catppuccin-latte,
         tokyo-night,tokyo-night-day}.tape
                                          the CP-1 SIX-theme sweep — all of
                                          BUILTIN_SLUGS, not the four this
                                          library originally scoped
  artifacts/                             V005 — committed Screenshot/gif output
    .gitkeep
```

`_`-prefixed files (`_config.tape`, `_dimensions.tape`, …) are `V002`/`V003`
additions, skipped by `run-tapes.sh` by convention — not standalone
recordings a checkpoint captures. The two aren't quite the same shape,
though: `_config.tape` (`V003`) is a true fragment, meant to be `Source`d
verbatim by every real tape. `_dimensions.tape` is a **reference table**, not
something to `Source` — it sets one specific `Set Width` for one specific
column count, and VHS only honours `Set` directives before a tape's first
real command (see the file's own header), so sourcing it would just lock in
whichever column count happens to be its live stanza. A real tape wanting,
say, 120 columns copies `1228` from the table into its own `Set Width` line
near the top, after `Source "_config.tape"`.

## Running

```
just tapes        # every real tape
just tape 1a       # exactly one, by screen id — V005
```

Both version-check `vhs`, `ttyd`, and `ffmpeg` first; fail loudly and
legibly if any is missing or the wrong version (see
`tapes/check-versions.sh` — the message names what's expected and what was
found). Once versions check out, `just tapes` records every real
`tapes/*.tape` (`_`-prefixed skipped) and `just tape <id>` records just
`tapes/<id>.tape`. **Every real tape passes** since `T090` landed the S1
host — see "Screen library convention" above for what each one captures, and
for the one caveat that survives: `Screenshot` is intermittently flaky in
this sandboxed worktree, so check `ls artifacts/*.png` against the tape count
after a from-scratch run and re-run `just tape <id>` for anything short.

`phosphor` has to be on `$PATH` before any of this — `Require phosphor` is
the first line of every real tape:

```
cargo build --release --bin phosphor && \
  mkdir -p /tmp/phosphor-bin && \
  ln -sf "$PWD/target/release/phosphor" /tmp/phosphor-bin/phosphor && \
  PATH="/tmp/phosphor-bin:$PATH" just tapes
```
