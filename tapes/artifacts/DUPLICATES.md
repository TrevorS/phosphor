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
