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

**A second regression hit the same claim in Window D**, this time from S2
rather than a missing host — every tape typing a bare `phosphor …` silently
lost the ability to find `runtime/` once Steel composition landed. Fixed;
see "Window D — the `PHOSPHOR_RUNTIME` regression" below for the mechanism
and the proof, rather than trusting this sentence twice in a row.
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

## CP-2 — the spine's tapes

Three tapes, matching `docs/TASKS.md`'s `CP-2` line for what VHS produces: a
clip of a REPL rebind live on the very next frame, the broken-`init.scm` boot
with its error float, and `6b`. All three landed clean on the first real run
against Window C's `T019`–`T025`/`T078`–`T080` build (`crates/phosphor/src/main.rs`
gained `--repl`/`--eval`, `AppHost`, and the Steel-composed REPL surface during
this window).

**New finding, load-bearing for all three: run against a scratch
`$PHOSPHOR_RUNTIME`, never the tracked `runtime/`.** `Runtime::root()`
(`crates/phosphor-steel/src/runtime.rs`) falls back to `./runtime` when the env
var is unset, which is exactly the checked-in tree — and every one of these
three tapes either plants a mistake into `init.scm` or types a
`keymap-set!` (one of `repl.scm`'s `phosphor/persistent-heads`, which
`persist-form!` really does append to a file on disk). harness does not own
`runtime/**` (`TEAM.md`), so each tape's `Hide`den setup line copies
`../runtime` into a fresh `/tmp` directory first and points
`PHOSPHOR_RUNTIME` at the copy. Confirmed empirically that this is necessary,
not defensive: the first investigation run (since deleted, its findings are
what's below) used the real tree with no override and left a `persisted.scm`
diff behind.

- **`broken-init.tape`** — plants the same mistake
  `crates/phosphor-steel/tests/broken_init.rs`'s
  `the_error_float_carries_the_file_the_line_and_the_message` test does
  (`(define broken (+ 1 nonesuch))`) and screenshots the boot float:
  `◆ steel · boot`, `1 fault`, `init.scm:<line>:<col> · free identifier`, the
  offending line, `45 of 46 forms ran · the editor is up`, and a footer
  offering `:repl` / `:reload-runtime` / `esc close` — over the dimmed buffer
  (§9), not a blank screen. Sentinel: `init.scm`, a string that appears only
  once the float has actually painted (the buffer behind it is
  `phosphor-core/src/lib.rs`, which never contains it).

- **`6b.tape`** — types the same four lines, in order, as
  `crates/phosphor-steel/tests/screen_6b.rs`'s `TYPED` (itself transcribed
  from the mockup). **What it captures is `S2` truth, not the mockup's
  prose**: three of the four lines hit a free identifier or a not-yet-built
  store (`region-author`, `claude`, `T041`) and the fourth hits a shape gap
  (`place-watch`'s `anchor` wants a `Target`, the mockup passes a string) —
  see that test's own module doc for the per-line table. The tape reproduces
  exactly that, on real pixels, rather than a value nobody asked for. All
  four lines fit at 120 columns with no wrap. Per-line sentinels wait on a
  word unique to each answer, so a tape racing ahead of the VM times out
  rather than screenshotting a partial session.

- **`repl-liveness.tape`** — the hard one, and the one that matters (per this
  window's brief). `T022`'s claim is that a binding created at the REPL is
  live on the *editor's* very next keystroke, with no restart, because
  `main.rs` caches no keymap and asks the live VM
  (`runtime/keymaps.scm`) every time. The tape: plain buffer (NORMAL) →
  `:` opens the REPL → `(keymap-set! "gz" (lambda () (open-repl!)))`,
  submitted → `esc` closes back to the plain buffer (the real "before" frame:
  nothing on screen suggests a REPL exists, and `gz` has never been pressed)
  → `gz`, typed as two ordinary keystrokes into the *buffer* → the REPL
  reopens, still holding the `keymap-set!` line in its history. Nothing
  between the first and last screenshot is `Hide`d, so the GIF is one
  unbroken recording — the only way a *clip* (not a pair of stills) proves
  "no restart happened in between." The GIF is the reviewed artifact here,
  not V005's usual free byproduct.

  Two things this tape found empirically, and got backwards on the first
  attempt by trusting the mockup's own footer text instead of the render:

  - **`esc` closes the REPL, not `q`.** `6b`'s footer draws "q close", but
    the REPL's body is a plain text input today (no REPL-local modes until
    `T026`) — `q` while it has focus is a character being typed, matching
    `repl_key`'s own doc comment in `main.rs`. This tape is what confirmed
    that on a real capture rather than assuming the footer is executable.
  - **The live keymap only runs on the buffer surface.** A first version of
    this tape typed `gz` *inside* the still-open REPL, expecting the rebind
    to fire immediately — it didn't; `gz` landed as two literal characters on
    the input line (`main.rs`'s loop only calls `press(&mut runtime, key)`,
    the keymap dispatcher, when `surface` is `Buffer`; while `Repl` has the
    frame every key is text for its own prompt). The rebind is live for the
    *editor*, which is `T022`'s actual claim — reopening the REPL from
    *inside* the REPL was never the right demonstration of it.

  `repl-liveness-2-bound.png` (REPL open, rebind just submitted) and
  `repl-liveness-4-live-on-next-key.png` (REPL reopened via the new binding)
  are byte-identical — documented in `tapes/artifacts/DUPLICATES.md` as
  identical *by construction*: the same session, nothing typed into it
  between the two screenshots, so a pixel difference would be the actual bug.

Naming note for whoever adds the next non-mockup-id capture: none of these
three is a `TUI Mockups.dc.html` id except `6b` itself, so they follow the
library's existing precedent for descriptive real-tape names (`sweep-*`,
`theme-*`) rather than inventing a fourth naming scheme.

## Window D — the `PHOSPHOR_RUNTIME` regression in every S1-era tape

Found and fixed while building `V007`/`V009`, and worth recording because it
silently broke sixteen committed tapes plus three CP-1 investigation tapes —
every real tape in the library except the three that already knew better
(`broken-init.tape`, `6b.tape`, `repl-liveness.tape`).

**The bug.** Since S2 landed, `crates/phosphor/src/main.rs` composes the
statusline in Steel with no Rust fallback — `CP-2`'s own finding, deleting
`runtime/statusline.scm` draws no statusline at all. `Runtime::root()`'s
third fallback (`crates/phosphor-steel/src/runtime.rs`) is a bare
`PathBuf::from("runtime")`, resolved relative to the *process's* current
working directory. `just tape`/`just tapes` always run `vhs` with cwd
`tapes/` (both `run-tapes.sh` and the `tape id` recipe `cd tapes` first) —
and no `runtime/` directory exists there. Every tape that types a bare
`phosphor …` with no override — `1a`, `9c`, `8c`, `8d`, all six
`sweep-*`, all six `theme-*`, and `_float-check-{informational,needs-you,
fullwidth-80}`, `_soft-wrap-check` — inherited this the moment S2 shipped:
`Runtime::root()` returns `None`, nothing composes a statusline, and the
V004 sentinel (which matches specifically on the statusline's mode word)
never appears. The tape doesn't error — it hangs to the `Wait+Screen@10s`
timeout and reports `recording failed`.

**Confirmed empirically this session, not inferred:** `1a.tape` reran with
no changes timed out twice running (`failed to execute command: timeout
waiting for … last value was: <file content, no statusline>`); the identical
tape with `PHOSPHOR_RUNTIME=../runtime` prefixed onto the typed command
succeeded on the first try. Ruled out as sandbox noise by a same-tape,
back-to-back control rerun scoring `compare -metric AE` `0` against itself —
if it were general capture jitter, a bare rerun would show the same kind of
drift, and it did not.

**The fix.** The same inline-env-var prefix the CP-2 tapes already
established (`Type "PHOSPHOR_UNDERCURL=0 undercurl"`, not VHS's own `Env`
directive — see V009 below for why that choice, not `Env`, was made again
here) — `PHOSPHOR_RUNTIME=../runtime` prefixed onto every affected `Type`
line, pointing directly at the real, tracked `runtime/` rather than a
scratch copy. Safe un-copied, unlike `broken-init.tape`/`6b.tape`
/`repl-liveness.tape`: none of these sixteen tapes types a `:` command or
anything that calls `persist-form!`, so there is nothing for them to write
back into the tracked tree. `1a.tape` carries the full writeup; the other
nineteen point back to it rather than repeating it.

**What this means for anyone regenerating the library cold:** if `just
tapes` starts hanging to `Wait+Screen` timeouts again after a runtime change,
check `Runtime::root()`'s fallback order before assuming the sandbox — this
exact failure has one specific cause and it isn't flaky infrastructure.

## V007 — pixel-diff runner

`tapes/diff-tapes.sh` (`just tapes-diff` / `just tape-diff <id>`). Captures a
screen fresh, compares it against the reference committed at `tapes/artifacts/
<id>.png` **at git HEAD** — not just whatever is sitting in the working tree,
since every real tape's `Screenshot` path is the same `artifacts/<id>.png`
every time (V005), so a fresh capture always overwrites the file a bare `git
diff` would otherwise show you were the reference — and on a mismatch writes
a side-by-side diff image instead of failing. The script's own header carries
the full contract (usage, `--no-capture`, exit codes); this section carries
the proof.

**Never a build gate, by construction, not by omission.** Harness's own
characteristic failure (`docs/TEAM.md`) is letting Tier 2 gate CI, so the
soft-fail is written into the script rather than left for whoever wires this
into CI (`V008`, not built yet) to remember `continue-on-error`:
`diff-tapes.sh` exits `0` for any number of pixel mismatches — they're
printed as findings with a pointer to the diff image, never a nonzero exit.
The only thing that sets a nonzero exit is a genuine tool failure: a missing
`.tape` file, `vhs` itself failing, or `compare` returning something that
isn't a pixel count. Nothing in `just gate` calls this recipe, and it should
stay that way.

**Fuzz tolerance.** `0.6%`, chosen against V002's own finding above (up to
`1/255`, ~0.39%, per-channel drift between an escape code's intended colour
and what a capture actually shows, sourced somewhere in the headless-Chromium
canvas → PNG path). Confirmed this session against a real, deterministic
capture rather than assumed: a same-content, same-tape rerun of `1a.tape`
(the `_probe-rerun.tape` control described below) scored `compare -metric AE`
`0` at this tolerance.

**The proof, run for real and then reverted** (the task's own instruction —
prove it by making one, not by asserting it):

```
$ magick tapes/artifacts/9c.png -fill "#ff00ff" \
    -draw "rectangle 200,150 209,169" tapes/artifacts/9c.png   # one ~10x20px cell
$ bash tapes/diff-tapes.sh --no-capture 9c
x 9c — MISMATCH (127.948 px beyond 0.6% tolerance) — see artifacts/_diffs/9c.diff.png
    left to right: committed reference | fresh capture | diff (red = changed)

diff-tapes.sh: 0 matched, 1 mismatched, 0 skipped (no reference yet)
$ echo $?
0
```

`artifacts/_diffs/9c.diff.png` showed the three panels side by side —
clean reference, the planted magenta cell clearly visible in the middle
panel, and a single red mark in the right panel with everything else
dimmed to grey. Legible at a glance, per the acceptance line. The probe was
then reverted: `git checkout -- tapes/artifacts/9c.png` and
`rm -rf tapes/artifacts/_diffs` — nothing from this test is committed.

**A second, unplanned proof landed for free.** `just tape-diff 9c` (the real
capture path, not `--no-capture`) reported a genuine mismatch against the
*actual* current build — `crates/phosphor-core/src/lib.rs` (the file every
`1a`/`9c`/`8c`/`8d`/`sweep-*`/`theme-*` tape opens) had been edited by a
concurrent teammate mid-session, so the fresh capture legitimately differed
from the committed reference. `diff-tapes.sh` caught it, reported it, wrote
the diff image, and exited `0` — exactly the intended behaviour, and a live
demonstration of exactly why `V006`'s deterministic fixture repo
(`docs/TASKS.md`, not built yet — `Needs: V005, T023`) matters: every one of
these tapes points at real, moving source, and will keep reporting
false-positive-looking mismatches under concurrent development until
something stops moving under them. Reverted the same way afterward.

### V007 saw one frame per tape, and six tapes draw no such frame (CP-4)

`diff-tapes.sh` compared `artifacts/<id>.png` and nothing else. **Six tapes in
the library never write that file**: `3c`, `folds`, `8e`,
`insert-whitespace-marks`, `repl-liveness` and now `signature-help` each
screenshot a *named moment* instead — `3c-open.png`, `folds-reopened.png`,
`repl-liveness-4-live-on-next-key.png`. Every one of them printed *"no
committed reference yet, nothing to diff against"* and was counted as
`skipped`, indistinguishable from a screen nobody had captured. That is the
worst answer a change detector can give: the tapes it could not see are
exactly the ones capturing what a **keystroke** does, which is the whole
lesson of the `CP-3` history section above.

Found while adding `signature-help`, whose three frames would have been
invisible from the day they landed. A screen's frames are now `<id>.png` plus
every committed `<id>-<suffix>.png`, with one exclusion that is not optional:
**a name that is itself a tape id belongs to that tape.**
`1a-degraded-term.png` is its own tape's frame and not `1a`'s, and
`diagnostics-undercurl.png` is not `diagnostics`'s — without that rule the
variant tapes this library already has would each be diffed twice, once
against the wrong screen. Coverage went from 25 frames to **41**, and the
summary line now counts frames rather than screens so the two cannot be
confused again.

**Proven by planting one**, in a frame the old runner could not have reached:
a ~10x20px magenta cell drawn into `signature-help-typing.png`,
`diff-tapes.sh --no-capture signature-help` → `x signature-help-typing —
MISMATCH (126.767 px beyond 0.6% tolerance)` with its two siblings still
matching, exit code still `0`. Reverted with `git checkout --`, and the
`_diffs` directory removed.

**Two bash facts this cost, both recorded in the script**: `declare -A` is a
syntax error on the bash macOS ships (3.2.57), and — the one that matters —
`git ls-tree HEAD:tapes/artifacts` run with cwd `tapes/` resolves the path
against the prefix and prints **nothing at all with exit code 0**, so the
first working version reported every screen as having no reference and
"passed" by doing nothing. `--full-tree` with a root-relative pathspec is the
form that works from a subdirectory.

**Montage note.** The obvious tool for "side by side" is ImageMagick
`montage`, and it was tried first — it failed outright even with `-label
''`, because this machine's ImageMagick has no fonts configured at all
(`convert -list font` returns nothing) and `montage` renders *something*
text-shaped regardless of `-label`. `magick a b c -background '#222'
-splice 4x0+0+0 +append out.png` needs no font and was verified to produce
an equivalent, legible result (the proof above). Left-to-right order is
fixed and documented rather than labelled in-image, since there is no
label to add without the same font dependency.

## V009 — degradation tapes

Acceptance line: *"the degradation path is captured for `1a` and `2a`
without touching a real terminal."* Checked against the tree this session,
not assumed: `2a` (the unseen-region picker with diff preview, `T046`, S5)
has no tape and cannot — it isn't built. `tapes/` today has `1a`, `6b`,
`8c`, `8d`, `9c` and the sweeps/themes; only `1a` is named in the acceptance
line and exists. This is that half.

**Two new tapes, `1a-degraded-term.tape` and `1a-degraded-nocolor.tape`** —
same file, same theme, same width as `1a.tape`, with `TERM=xterm-256color`
or `NO_COLOR=1` prefixed onto the typed command (the same inline-env-var
pattern the `_undercurl-check-*.tape` trio already uses, not VHS's own `Env`
directive — chosen for consistency with a mechanism already proven in this
library over a second, unverified one for the same job).

Of the three fallback paths the acceptance line names — the `▎` marker, undercurl
→ underline, the static `✻` — **none renders on `1a` yet**, checked by reading
the tree this session, not inferring it: `crates/phosphor-ui/src/gutter.rs`
is a header and nothing else (`T031` hasn't landed), nothing on this slice
invokes `UnderlineCapability::resolve` (the only call site anywhere in the
tree is `crates/phosphor-buffer`'s own tests/example), and `SessionState`
doesn't exist yet (`T051`, S6). So neither degradation tape could show any of
the three named paths regardless of what was typed — that is a fact about
the build's current phase, not a harness gap, and both tapes' own headers
record it as a tripwire for when `T031`/`T040`/`T051` land.

**What actually happened when they were captured anyway — and it wasn't
nothing:**

- **`TERM=xterm-256color`**: pixel-identical to `1a.png`. Confirmed by
  running it twice (`compare -metric AE -fuzz 0.6%` reports `0` both times).
  Nothing on this screen reads `TERM` today.
- **`NO_COLOR=1`**: **not** identical — `compare` reports `35360.5` px
  different, reproduced against a same-content control (see the
  `PHOSPHOR_RUNTIME` section above), so it isn't capture noise. First
  instinct was wrong here: grepping `crates/phosphor` and `crates/phosphor-ui`
  for `NO_COLOR` finds nothing, which looks like "no effect" — but
  `crossterm` `0.29.0` (the pinned version, `T002`) implements NO_COLOR
  itself (`crossterm-0.29.0/src/style.rs`, confirmed by reading the vendored
  registry source this session), stripping `SetForegroundColor`/
  `SetBackgroundColor` unless the app calls `force_color_output(true)` —
  which nothing in `crates/` does. So every explicitly-coloured span,
  including this screen's syntax comment colour, silently falls back to the
  terminal's plain default foreground. Sampled pixels confirm the mechanism:
  a comment glyph reading `(35,42,38)` (dim, syntax-coloured) in `1a.png`
  reads `(93,100,95)` (brighter, desaturated) in the same position under
  `NO_COLOR=1`. **`NO_COLOR` is a live degradation path today** — just not
  through any of the three the acceptance line names, and one dependency
  layer below any code phosphor owns.

**A fourth tape, `_undercurl-check-no-color.tape`**, completes the set
`crates/phosphor-buffer/examples/undercurl.rs`'s own header calls for
(`NO_COLOR=1 cargo run … # V009's other one`) — the one degradation path
(undercurl → underline) that a fixture, if not `1a` itself, can actually
exercise. **Not captured this session**: `vhs` failed 9 (later 10) times in
a row with `no frames` / `recording failed`, including on
`_undercurl-check-auto.tape` and `-forced-underline.tape`, both already-proven
siblings that had captured cleanly earlier in the same session — general
sandbox flakiness (this file's own already-documented "Screenshot flakiness
in this sandboxed worktree", one level worse than usual this run), not a
defect in the tape. Its original prediction (byte-identical to
`-forced-underline`) is flagged in its own header as suspect given the
`NO_COLOR`/`crossterm` finding above and left unresolved rather than guessed
at — whether the undercurl's *colour* survives `NO_COLOR` the way its
*shape* likely does is a question for the capture, not for this paragraph.
Re-running `vhs _undercurl-check-no-color.tape` (cwd `tapes/`) should be all
that's needed; no tape content should need to change.

## CP-3 — harness's tapes

`docs/TASKS.md`'s `CP-3` line asks for four captures: the leader popup
opening (`3c`), folds collapsing and expanding, insert-only whitespace marks,
and the once-per-session unknown-key hint firing and then not firing again
(`8e`). **All four are captured now.** This section used to record why three
of the four were refused — `main.rs` couldn't compose any of those widgets
from a keystroke, confirmed empirically as well as by reading — and that
finding is kept below (as *history*) because both the defect and the fix are
worth having on record: `spine`'s repair pass wired all three in the same
window, `crates/phosphor/tests/loop_pty.rs` grew a `driven::` test per
surface, and `harness` re-verified each one this session (`cargo nextest run
-p phosphor --test loop_pty driven::`, 13/13 green) before capturing
anything.

- **`3c.tape` — the `SPC` leader popup.** `main.rs:1411` `fn under(layer,
  machine)` reads the live keymap table for the pending prefix and
  `:1318`/`:1339-1340` compose it as a real `Node::KeyHints { density:
  Density::Grid, .. }` strip; `loop_pty.rs:451`
  `driven::pressing_space_opens_the_leader_popup` and `:494`
  `driven::a_repl_rebind_reaches_the_leader_popup` prove it on a pty. Two
  screenshots: the popup open (`SPC ·` title, the shipped groups — `+claude`,
  `+unseen`, `+disk`, …) and after `<esc>`.
  **Finding, captured rather than avoided:** the "closed" frame is not a
  plain buffer — `<esc>` while `SPC` is pending has no dedicated cancel path
  (every key while a sequence is pending is looked up as part of that
  sequence, `input.rs:376-378`), so `"SPC <esc>"` resolves `Unbound`
  (`input.rs:407`), which closes the popup **and** spends the once-per-session
  unknown-key hint in the same turn (`:414`, `:417`). The tape's own header
  has the full citation trail; not fixed here (`input.rs` is outside
  `tapes/**`), reported to `spine`/Teej.

- **`folds.tape` — `za`/`zR` collapsing and expanding a real fold.**
  `crates/phosphor-core/src/action.rs` declares
  `SetFold`/`FoldAll`/`UnfoldAll`; `main.rs:1964`, `:1971`, `:1975` are the
  three arms (current line numbers — `docs/TASKS.md`'s own citation,
  `:1911`/`:1918`/`:1922`, had already drifted off the tree by the time this
  was checked); `runtime/keymaps.scm:586-593` binds `za zc zo zM zR`;
  `loop_pty.rs:566` `driven::za_closes_the_fold_the_cursor_is_in` proves it.
  Three screenshots: open, closed (`▸⋯ 5 lines`, the hidden body gone from
  the frame), reopened. The first and third are byte-identical by
  construction — `zR` puts back exactly what was there
  (`tapes/artifacts/DUPLICATES.md`).

- **`8e.tape` — the unknown-key hint, firing once and then not again.**
  `unknown_key.rs:71`/`:76` are the exact strip text; `main.rs:875` owns the
  session latch, `:2009` is the `ShowUnknownKeyHint` arm, `:1332` composes
  it; `loop_pty.rs:521` `driven::an_unbound_key_teaches_once_and_never_again`
  proves it (same key order this tape scripts: `Q`, `Q`, `<esc>`). Two
  screenshots: taught (`unknown key Q — SPC opens the keymap · :help
  agent-objects · shown once`) and silent (the row gone, one more buffer
  line visible in its place). The silent frame turned out to be
  pixel-identical to a sibling tape's frame for an unrelated reason — see
  `DUPLICATES.md`'s `1a-degraded-term.png` ≡ `8e-silent.png` entry, which is
  also where a real staleness in the `CP-1` reference stills got found as a
  side effect of this check.

- **`insert-whitespace-marks.tape` — recaptured.** History matters here: the
  previous pair of artifacts were byte-identical (51293 bytes each, both
  frames), which reads as "the second screenshot never advanced" — a
  capture-pipeline symptom, not a whitespace-marks defect, per `T016`'s own
  repair-pass note in `docs/TASKS.md`. They were deleted rather than
  diagnosed or documented, which is the wrong call
  (`tapes/artifacts/DUPLICATES.md` exists for exactly this). Recaptured
  three times this session: `NORMAL` (no mark after `sum = 0;`) and `INSERT`
  (`··` appears on the same line, nothing else on the frame changes)
  produced two **different** hashes every run, and the same two hashes
  across all three runs — reproducible, and the mark is real. The wiring
  citation is unchanged: `main.rs` calls `soft_wrap::configure` once at
  startup and `soft_wrap::set_mode` every frame, driven off
  `machine.mode()`.

**Reproducibility, checked rather than assumed.** Each new tape was run 2-3
times this session (`insert-whitespace-marks` and `3c` three times, `folds`
and `8e` twice) — zero `vhs` failures (`no frames` / `recording failed`),
zero short/truncated PNGs. `folds` and `8e`/`insert-whitespace-marks`
reproduced with byte-identical `sha256` hashes run over run; `3c`'s hashes
varied run over run for the *same* screenshot despite `magick compare
-metric AE` reporting `0` (pixel-identical) between two of those runs —
vhs's PNG encoder is not byte-deterministic across invocations even when the
pixels are, which matters for judging *this* kind of run-to-run diff (bytes
differing is not proof of a real difference; check pixels) but not for the
library's own correctness (only one version of each file is ever
committed).

**What this means for the checkpoint.** `docs/TASKS.md`'s fold task note
already says it plainly: `T016` was ticked *"screen `8e`'s fold and
whitespace details reproduce"* for three windows while the fold half had no
binding at all — every gate asked *does the widget draw correctly* and never
*does a keystroke reach it*. These four tapes are that second question,
answered on a real pty before a single pixel was captured, not inferred from
the widget tests passing. `CP-3`'s manual half (*"`SPC` leader popup — is
the namespace learnable?"*) can now actually be judged against a build where
pressing `SPC` does something.

### History — why three of the four were refused before this window

Kept for the record, and because the method (read the code, then confirm
empirically before trusting the read) is worth showing rather than just the
conclusion.

- **The leader popup (`3c`).** The widget existed
  (`phosphor-ui/src/key_hints.rs`) and `crates/phosphor/tests/screen_3c.rs`
  proved it rendered correctly from the live keymap table — but that test
  built its own `Tree` by hand (its own module doc said so: *"The only Rust
  in the composition is the split itself and the strip's height"*).
  `crates/phosphor/src/main.rs` referenced neither `KeyHints`,
  `Node::KeyHints` nor `Density::Grid` (grepped clean), and neither its
  `Surface` enum nor its `Intent` enum had anything a `SPC`-pending state
  could route through. Confirmed empirically, not just by reading: a real
  capture of `phosphor`, one frame before `Space` and one frame after,
  diffed at **0 px** (`magick compare -metric AE`, exact match —
  investigation tape and screenshots not committed, this was the finding).

- **Folds.** `Action::View(ViewAction::SetFold | FoldAll | UnfoldAll)`
  existed and was documented `"za"`, but no `"z` binding of any kind existed
  in `runtime/keymaps.scm` (grepped clean), and `Editing::act`'s match in
  `main.rs` had no arm for any `ViewAction` besides `Scroll` — a fold action
  fell through to the catch-all `Refused(NotYetImplemented)`. Confirmed
  empirically: a real capture of `phosphor`, typing `za`, showed exactly the
  vim primitive `a` (append: cursor moves right one cell, mode switches
  NORMAL → INSERT) — `z` was silently swallowed as unbound, and nothing
  fold-shaped happened. The vendored editor's own fold API
  (`toggle_fold_at_line`) was real and used by `screen_8e.rs`'s test setup,
  but nothing in the shipping input path called it.

- **The unknown-key hint (`T035`, `8e`).** The module
  (`phosphor-ui/src/unknown_key.rs`, `UnknownKeyHint`) and its Tier-1 test
  (`screen_8e.rs`) both existed and passed, but `unknown_key`/
  `UnknownKeyHint` was never referenced anywhere under `crates/phosphor/src/`
  (grepped clean) — there was no call to `UnknownKeyHint::teach` anywhere in
  the real event loop, so no key, bound or not, could make the hint appear
  on a running `phosphor`.

All three were one wiring step in `main.rs` away from being real —
`spine`'s file, not `harness`'s, and product work was frozen for that gate
per that window's brief. Not "the feature is missing": `T034`/`T035`/`T016`'s
fold half all had working widgets and passing Tier-1 tests, which was most
of the work. It was a gap between the widget landing and the binary's event
loop composing it in — flagged for `spine`/Teej rather than folded in, per
`harness`'s own standing instruction (`docs/TEAM.md`).

## CP-4 — the `S4` tapes, and the server they are not driven by

`docs/TASKS.md`'s `CP-4` line asks for three things: *"the completion float
opening over real code in all three languages (`7c`) · signature help · a file
with real diagnostics showing gutter priority against other region states"*.
Six tapes, and **one of the three is answered in half** — the half that can be.
`docs/TEAM.md`'s Window D note is what said this was outstanding: the `S4`
window ran with `harness` absent, and producing a checkpoint's tapes is
standing work rather than a numbered task, so nobody's prompt named it.

| tape | what it captures |
|---|---|
| `7c-rust.tape` | `7c`'s float over real rust — carries the writeup the other five point back to |
| `7c-typescript.tape` | the same over real typescript, reached through the `.` trigger |
| `7c-python.tape` | the same over real python |
| `signature-help.tape` | `<C-s>` inside a call, three frames: open · the argument typed under it · dismissed by `<esc>` |
| `diagnostics.tape` | a publish nobody asked for, reaching the gutter and a `■` row |
| `diagnostics-undercurl.tape` | the same range with `PHOSPHOR_UNDERCURL=1` — the primary terminal's treatment beside the degraded one |

**The server is a fixture, and that is the decision this section exists to
argue.** `crates/phosphor/tests/fixtures/toy_language_server.py` was written
for the `S4` pty tests; these tapes drive it too. rust-analyzer is the honest
server and the wrong one to hang a *pixel reference* on — it must be
installed, it indexes a crate graph, and what it offers depends on how far
that has got, which depends on the machine. A tape racing indexing produces a
different frame every capture, which is the flake `V006` exists to prevent for
agent surfaces and precisely the red build `docs/TEAM.md` warns teaches a team
to ignore the harness. The fixture speaks the same protocol over the same
transport — framing, JSON-RPC envelope, `initialize`/`didOpen`, and the UTF-16
position encoding the client reads back and refuses on — and answers `7c`'s
own three labels, its detail column and its one row of prose in constants. So
these captures show `7c`'s **shape**, drawn by the shipping binary over a real
pipe to a real process, identically on every run.

**What it costs, left visible.** The statusline chip reads `toy-lsp ✓` where
the mockup draws `rust-analyzer ✓`, because the chip draws the name a server
gives itself. Nothing here disguises that, and nothing here closes the gap
`docs/TASKS.md`'s `CP-4` entry records: `typescript` and `python` declare
servers **nothing automated has ever attached to**, which is what the `rootUri`
defect at `T036` cost. A fixture proves the editor's half of a conversation and
never the real server's. Teej's half of the checkpoint — *is completion fast
enough to be useful, or fast enough to be annoying* — is a question about
rust-analyzer's latency and no tape can answer it.

**What is not the fixture.** `tapes/lsp-fixture.sh` swaps exactly one line of
the *copied* `runtime/languages/<language>.scm` — `lsp_command` — so the
grammar, the extensions and the comment prefix are still whatever `runtime/`
declares today, and every glyph on these frames is real tree-sitter
highlighting of a real buffer. That is what makes *"over real code"* true
rather than claimed, and it means a declaration that changes upstream changes
the capture instead of silently disagreeing with it. The script fails loudly
if a declaration does not have exactly one `lsp_command` line, rather than
rewriting a line nobody looked at. Same scratch-`$PHOSPHOR_RUNTIME` rule as
`broken-init.tape`, for the same reason: `harness` does not own `runtime/**`.

**The diagnostics half `CP-4` asks for and this does not answer.** The line
says *"gutter priority **against other region states**"*, and there is exactly
one source of regions until `T041` — which is why `T040` is unticked and why
its task entry is worth reading before judging the screen. The host
concatenates diagnostic regions with every other source and calls
`gutter::state_column` once; the composition is written and the `Vec` has one
element in it. `diagnostics.png` is the first clause. Re-capturing it once
`T041` puts a second source in that `Vec` is what would make it the checkpoint
item.

### The undercurl pair, and the signal that separates them

`CP-4` says *"undercurl only if `V002` established that it survives capture"*.
It did — see this file's own V002 section — and until now nothing in the
shipping binary had a surface to spend that on: the `_undercurl-check-*` trio
drove `phosphor-buffer`'s standalone example, because when they were written
no code path in `crates/phosphor` reached `UnderlineCapability::resolve`.
`main.rs`'s `set_styled_spans` does now, and a published diagnostic is what
puts an underlined range on a real frame.

`vhs`'s ttyd session reports `TERM=xterm-256color`, which
`UnderlineCapability::detect`
(`vendor/ratatui-code-editor/src/phosphor/cell_style.rs`) resolves to
`Underline` — so the *default* capture of any underlined range in this library
is the degradation path, and the primary terminal's look needs
`PHOSPHOR_UNDERCURL=1`. The two PNGs differ, measured rather than eyeballed,
by V002's own Signal-3 method: per-row ink coverage across the five columns of
the diagnostic's range (`x=62..112`, background `#0c0f0c`):

| row | `diagnostics.png` (degraded) | `diagnostics-undercurl.png` |
|---|---|---|
| `y=35` | 50/50 | 6/50 |
| `y=36` | 50/50 | 28/50 |
| `y=37` | 0/50 | 50/50 |
| `y=38` | 0/50 | 24/50 |

A flat, full-width, two-row band against partial, varying coverage spread over
four rows — a straight line against a line whose y-position moves with x. Same
font, same theme, same file, same span; the only variable was the escape.
That is the same signature `V002` measured on the fixture example, now on a
product surface.

**This pair is not `CP-4`'s manual item.** *"Undercurl on the primary
terminal; underline fallback on the degradation terminal"* is a question about
two real terminals and stays Teej's. What the pair answers is the half that is
a fact about the build rather than about the hardware.

### Reproducibility, and the flake this found in the library's own convention

Every one of the six was captured, compared, and captured again. **Six of the
eight PNGs were byte-identical across two independent runs on the first try**
(`sha256`, not just pixel-equal): `7c-rust`, `7c-typescript`, `7c-python`,
`signature-help-dismissed`, `diagnostics`, `diagnostics-undercurl`.

The two that were not are both `signature-help.tape`'s, and they are the only
two frames in the library taken from a session that **keeps going after the
`Screenshot`** — every other multi-shot tape either ends there or waits on a
key. `signature-help-typing.png` came back showing the *dismissed* frame
(`NORMAL`, the two covered rows repainted) on one run of three, and
`signature-help-open.png` differed by `10.8` px on another: **a `Screenshot` is
asynchronous to the key stream, so the frame a later key paints can land in an
earlier key's PNG.** This is the same class as the settle guard `1a.tape`
documents — the text buffer `Wait+Screen` matches against and the pixels
headless-Chromium has painted are not the same clock — but a second, distinct
manifestation of it: `1a`'s guard sits *before* the `Screenshot`, and this one
has to sit *after* it. Fixed by a `Sleep 500ms` after each non-final
`Screenshot`; **three consecutive runs then produced byte-identical `sha256`
for all three frames.**

> **A latent version of this is in `3c.tape` and `8e.tape` and is left
> alone.** Both take two screenshots with a key between them and neither has
> the trailing guard. Neither has been observed to flake — `CP-3` ran each two
> or three times clean — and re-capturing them would bless new bytes for
> screens this window was not asked to touch. Recorded here rather than fixed;
> if either ever produces a frame from the wrong side of a keystroke, this
> paragraph is the diagnosis.

**Two mutations, planted and reverted**, because a tape whose `Wait+Screen`
cannot fail is a reference that captures whatever was on screen:

* `7c-rust.tape` with the fixture server in `diagnostics` mode (it answers
  `[]` for completion): `recording failed`, no PNG written, the frame it timed
  out on visible in vhs's own output with the typed line and no float. The
  `Wait+Screen@10s /default_delay/` is what caught it.
* `diagnostics.tape` with the server in `completion` mode (it publishes
  nothing): `recording failed`, no PNG. `Wait+Screen@10s /expected Duration,
  found u128/` caught it.

**And one guard this could *not* show biting, stated rather than implied.**
Every `S4` tape waits on `toy-lsp` — the server's own `serverInfo.name` off
its `initialize` reply — before typing, because the insert-mode trigger is
edge-triggered on the edit and requires `servers.state(language).is_ready()`:
a character typed one frame early is dropped and the float never opens.
Replacing that with the library's usual mode-chip sentinel **still passed,
twice**, because the ~1.6 s of scripted typing before the trigger
(`G k k O` plus 28 characters at `TypingSpeed 50ms`) is longer than
`python3`'s startup on this machine. The sentinel is kept because that margin
is a property of this machine and this typing speed rather than of the editor
— `loop_pty.rs`'s `ready()` exists for the same race one tier down, where the
harness presses keys with no delay and the race does fire.

**One thing the pixels say that the Tier-1 assertions cannot.** `T039`'s claim
that the active parameter is drawn in its own style is a claim about *style*,
and every test of it asserts *text*. Sampling the signature row of
`signature-help-open.png` with Pillow: the `policy: RetryPolicy` run reaches
`(255, 255, 255)`, while `fn retry(` peaks at `(182, 190, 183)` and
`-> Result<(), Error>` at `(202, 210, 203)`. The active run is drawn brighter,
on captured pixels, which is the kind of thing Tier 2 is for.

### The line counts, which are load-bearing

Each `7c` tape types the line the mockup catches mid-typing, and reaches the
site by counting back from the end: `G` lands on the empty line a trailing
newline leaves (confirmed on a capture, not assumed), `k` walks back to the
call, `O` opens above it. Rust and typescript press `G k k O`; python presses
`G k O`, one fewer, because its function body ends with the call rather than
with a closing brace after it. Each fixture under `tapes/fixtures/` carries a
header saying its tail is counted, so the coupling is visible at both ends.
`O` copies the current line's indentation — typing a leading space produced
eight columns of indent, which is how that was found.

`fixtures/policy.rs` is shaped by the other end of the same rule: the fixture
server's diagnostic range is the constant *line 1, characters 0..5*, so that
file has exactly one header line and starts its second at column 0 with a
five-character word — and with a line about which `expected Duration, found
u128` is a true sentence. Written with a seven-line header first, which put
the diagnostic on a comment.

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
  diff-tapes.sh            V007 — the pixel-diff runner (`just tapes-diff` /
                            `just tape-diff <id>`); never gates CI
  lsp-fixture.sh            CP-4 — builds the scratch tree the six S4 tapes
                             drive and repoints one `lsp_command` at the
                             deterministic fixture server
  fixtures/                 CP-4 — the buffers those tapes open. Real code in
                             three languages, parsed by the shipped grammars;
                             each file's tail is counted by its tape
  _dimensions.tape         V002 — the column-width calibration table (+ V005's 40/60 rows)
  _config.tape              V003 — Source'd by every real tape
  _config-check.tape         V003 — its reproducibility proof
  _undercurl-check-{auto,forced-curl,forced-underline,no-color}.tape
                              V002/V009 — the undercurl fixture's four
                              investigation captures, run manually, not part
                              of the V005 screen library
  _soft-wrap-check.tape       CP-1 investigation — does the `↪` continuation
                               marker read on a real captured frame, not just
                               a Tier-1 grid dump. Run manually.
  1a.tape, 9c.tape, 8c.tape, 8d.tape   V005 — the four CP-1 stills
  1a-degraded-term.tape,                V009 — the TERM=xterm-256color and
  1a-degraded-nocolor.tape                NO_COLOR=1 variants of `1a`
  sweep-{200,120,100,80,60,40}.tape     V005 — the CP-1 width sweep
  theme-{phosphor-dark,phosphor-light,catppuccin,catppuccin-latte,
         tokyo-night,tokyo-night-day}.tape
                                          the CP-1 SIX-theme sweep — all of
                                          BUILTIN_SLUGS, not the four this
                                          library originally scoped
  broken-init.tape, 6b.tape,             the CP-2 tapes — see "CP-2 — the
  repl-liveness.tape                     spine's tapes" above
  3c.tape, folds.tape,                   the CP-3 tapes — see "CP-3 —
  insert-whitespace-marks.tape,          harness's tapes" above
  8e.tape
  7c-{rust,typescript,python}.tape,      the CP-4 tapes — see "CP-4 — the S4
  signature-help.tape,                   tapes" above. All six need `python3`
  diagnostics.tape,                      as well as `phosphor`, and each
  diagnostics-undercurl.tape             `Require`s both
  artifacts/                             V005 — committed Screenshot/gif output
    .gitkeep
    DUPLICATES.md                        why each byte-identical pair is allowed
    _diffs/                              V007 scratch output — never committed,
                                          written only on a mismatch, safe to
                                          delete any time
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
just tapes-diff    # every real tape, fresh vs. committed reference — V007
just tape-diff 1a  # exactly one, fresh vs. committed reference — V007
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
the first line of every real tape. Since `CP-4` **`python3` does too**: the six
`S4` tapes spawn the fixture language server with it, and each one `Require`s
it beside `phosphor` so a missing interpreter fails the tape immediately
rather than as a `Wait+Screen` timeout ten seconds later.

```
cargo build --release --bin phosphor && \
  mkdir -p /tmp/phosphor-bin && \
  ln -sf "$PWD/target/release/phosphor" /tmp/phosphor-bin/phosphor && \
  PATH="/tmp/phosphor-bin:$PATH" just tapes
```
