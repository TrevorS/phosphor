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
(`8e`). **One of the four is captured.** The other three are real, tested
widgets — each has its own Rust module and its own Tier-1 golden-frame test —
but none is reachable by a keystroke in `crates/phosphor/src/main.rs` today,
checked against the tree this session rather than assumed from the task
list. Building a tape against a surface the running binary cannot show would
be a false artifact, so none was built for them; this section is the record
of what was checked and how, so the next window doesn't have to rediscover
it.

- **`insert-whitespace-marks.tape` — captured.** `T016`'s claim (trailing
  whitespace marks in INSERT only) is genuinely wired: `main.rs` calls
  `soft_wrap::configure` once at startup and `soft_wrap::set_mode` every
  frame, driven off `machine.mode()`
  (`crates/phosphor/src/main.rs:825-832`, `:1066-1067`). Two screenshots off
  one real session — before `i` (NORMAL, no mark) and after (INSERT, `··`
  appears on the same line) — prove the mode-gating live rather than only in
  `crates/phosphor/tests/screen_8e.rs`'s hand-built `Tree`. Fixture is
  created inline in the tape's own `Hide`den setup (`printf` to
  `/tmp/phosphor-whitespace-fixture.rs`), not added to `fixtures/`
  (`V006`'s tree is fmt-clean by construction and out of this window's file
  lock, which scopes to `tapes/**` + `scripts/**`).

- **The leader popup (`3c`) — not captured; not wired.** The widget exists
  (`phosphor-ui/src/key_hints.rs`) and `crates/phosphor/tests/screen_3c.rs`
  proves it renders correctly from the live keymap table — but that test
  builds its own `Tree` by hand (its own module doc says so: *"The only Rust
  in the composition is the split itself and the strip's height"*).
  `crates/phosphor/src/main.rs` never references `KeyHints`, `Node::KeyHints`
  or `Density::Grid` (grepped clean), and neither its `Surface` enum
  (`main.rs:2040-2052`: `Buffer`/`Repl`/`Boot`/`Fixture`/`Ex`, no leader
  variant) nor its `Intent` enum (`main.rs:227`:
  `OpenRepl`/`CloseRepl`/`History`/`ToBuffer`/`Keymap`, same) has anything a
  `SPC`-pending state could route through. Confirmed empirically, not just by
  reading: a real capture of `phosphor`, one frame before `Space` and one
  frame after, diffed at **0 px** (`magick compare -metric AE`, exact
  match — investigation tape and screenshots not committed, this is the
  finding).

- **Folds — not captured; not wired.** `Action::View(ViewAction::SetFold |
  FoldAll | UnfoldAll)` exists and is documented `"za"`
  (`crates/phosphor-core/src/action.rs:414-424`), but no `"z` binding of any
  kind exists in `runtime/keymaps.scm` (grepped clean), and
  `Editing::act`'s match in `main.rs` has no arm for any `ViewAction` besides
  `Scroll` (`main.rs:1433`) — a fold action falls through to the catch-all
  `Refused(NotYetImplemented)` (`main.rs:1505`). Confirmed empirically: a
  real capture of `phosphor`, typing `za`, shows exactly the vim primitive
  `a` (append: cursor moves right one cell, mode switches NORMAL → INSERT) —
  `z` is silently swallowed as unbound, and nothing fold-shaped happens. The
  vendored editor's own fold API (`toggle_fold_at_line`) is real and used by
  `screen_8e.rs`'s test setup, but nothing in the shipping input path calls
  it.

- **The unknown-key hint (`T035`, `8e`) — not captured; not wired.** The
  module (`phosphor-ui/src/unknown_key.rs`, `UnknownKeyHint`) and its Tier-1
  test (`screen_8e.rs`) both exist and pass, but `unknown_key`/
  `UnknownKeyHint` is never referenced anywhere under `crates/phosphor/src/`
  (grepped clean) — there is no call to `UnknownKeyHint::teach` anywhere in
  the real event loop, so no key, bound or not, can make the hint appear on
  a running `phosphor`.

**What this means, and what it doesn't.** All three ungapped surfaces are one
wiring step in `main.rs` away from being real — `spine`'s file, not
`harness`'s, and product work is frozen for this gate per this window's
brief. This is not "the feature is missing"; `T034`/`T035`/`T016`'s fold half
all have working widgets and passing Tier-1 tests, which is most of the
work. It is a gap between the widget landing and the binary's event loop
composing it in, and CP-3's manual half (*"SPC leader popup — is the
namespace learnable?"*) cannot be judged against a build where pressing
`SPC` does nothing at all. Flagged for `spine`/Teej, not folded in — per
harness's own standing instruction (`docs/TEAM.md`), the tape and reference
for each get built once the surface is.

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
  insert-whitespace-marks.tape           the CP-3 tape — see "CP-3 — harness's
                                          tapes" above
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
the first line of every real tape:

```
cargo build --release --bin phosphor && \
  mkdir -p /tmp/phosphor-bin && \
  ln -sf "$PWD/target/release/phosphor" /tmp/phosphor-bin/phosphor && \
  PATH="/tmp/phosphor-bin:$PATH" just tapes
```
