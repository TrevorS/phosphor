# Byte-identical captures, and why each pair is allowed

`scripts/lint-repo-hygiene.sh` fails on any two committed reference PNGs with identical bytes
unless the filename appears below. The rule exists because a duplicate reference is a
**correctness** bug rather than wasted space: `V007`'s pixel-diff runner compares a fresh capture
against the committed one, so a `9c.png` that is really `1a.png` will regenerate identically
forever while the screen it claims to prove was never captured once.

Git stores identical blobs once, so none of this costs anything. What it costs is trust in the
library, which is why each group has to say whether it is **identical by construction** or a
**gap**.

---

## `1a.png` ≡ `9c.png` ≡ `sweep-120.png` ≡ `theme-phosphor-dark.png`

Three of these are identical by construction and one is a gap.

- **`sweep-120.png`** — the width sweep's widest step is 120 columns, which is what `1a` is
  captured at. Same buffer, same theme, same width; the same frame by definition.
- **`theme-phosphor-dark.png`** — phosphor-dark is the default theme, so the theme sweep's first
  entry is `1a` again. Kept as its own name because the sweep is read as a set.
- **`9c.png` — this one is a gap, and the only one here.** `9c`'s distinguishing feature is an
  **anchored region: tint + undercurl on a line of the buffer**. The S1 host renders a plain file
  and has no way to mark a region, so nothing at `CP-1` could produce that frame. Region tints are
  **`T087`** and land at S5; anchored regions are **`T068`** at S7. Until `T087`, this file is `1a`
  wearing `9c`'s name.

  **When `T087` lands, recapture `9c` and delete this entry.** If it is still identical to `1a`
  then, the capture is not exercising the region and the tape is wrong.

## `8c.png` ≡ `theme-phosphor-light.png`

Identical by construction. `8c` is the light-mode screen, and phosphor-light is the theme sweep's
light entry — the same render requested under two names, one for the mockup comparison and one for
the theme set.

## `8d.png` ≡ `sweep-80.png`

Identical by construction. `8d` is the 80-column screen and `sweep-80` is the 80-column step of
the sweep. Worth keeping both: `8d` is compared against a mockup, `sweep-80` is read as one frame
of a ladder, and they will diverge the moment either gains its own fixture.

## `undercurl-check-auto.png` ≡ `undercurl-check-forced-underline.png`

**Identical, and that is the finding.** `_undercurl-check-auto.tape` captures whatever
`UnderlineCapability::resolve` decides on the recording machine; `_undercurl-check-forced-underline`
forces the degraded path with `PHOSPHOR_UNDERCURL=0`. They match because the recording terminal
resolves to the fallback, so the auto path *is* the underline path here.

Not a defect — but it does mean **the recording machine cannot prove the curl renders**, which is
why `CP-1`'s undercurl check is a four-terminal human task and why
`_undercurl-check-forced-curl.png` (which differs from both) is the one that shows the wave.

## `repl-liveness-2-bound.png` ≡ `repl-liveness-4-live-on-next-key.png`

**Identical by construction, and the identity is the proof, not a mistake.** `repl-liveness.tape`
is `CP-2`'s liveness clip: it defines `(keymap-set! "gz" (lambda () (open-repl!)))` at the REPL
(`-2-bound.png`, the REPL open with that line and its `#ok · persisted …` answer in its history),
then `esc`-closes back to the plain buffer, then presses the two keys `g` `z` from the *buffer* —
newly bound, no restart in between — and the REPL reopens (`-4-live-on-next-key.png`).

Nothing was typed into the REPL between those two screenshots — no new form, no history entry —
so `Repl::frame()` renders the exact same session both times. A pixel difference here would be the
actual bug: it would mean either the rebind lost the REPL's prior history (i.e. something *did*
restart) or the "no restart" framing is inaccurate. Byte-identical is the tape working.

**If a future change to `repl-liveness.tape` or the REPL's frame composition makes these two
differ, that is not automatically a fix** — check first whether the difference is because
something in the session *should* have changed between the two frames (in which case update this
entry) or because a restart crept in (in which case the liveness claim itself just broke).

## `repl-liveness-1-before.png` ≡ `repl-liveness-3-back-to-buffer.png`

The other half of the same tape, and identical for the same reason. `-1-before.png` is the plain
buffer before `:` is ever pressed; `-3-back-to-buffer.png` is the plain buffer again, after
opening the REPL, defining the rebind, and `esc`-closing it. No key in between ever reached the
editor (`:` and `esc` are consumed by the keymap and `repl_key` respectively; every other key in
that span was typed into the REPL's own input line), so the buffer's cursor, viewport and content
are exactly what they were before any of it happened — invariant 3 (*"nothing moves unless you
asked"*) holding for the surface underneath the whole demonstration, not just the REPL session on
top of it.

Not observed on the very first capture of this tape (`-1-before.png` and `-3-back-to-buffer.png`
differed by a few bytes that run) — consistent with `1a.tape`'s already-documented
Screenshot-vs-paint race in this sandboxed worktree, not a real state difference. A clean rerun of
`just tape repl-liveness` produced the byte-identical pair recorded here; if a future rerun
produces a *third* distinct hash for either file, that is the race recurring, not a product
regression — rerun before concluding otherwise.
