# V002 — column calibration table.
#
# VHS sizes the terminal in *pixels* (`Set Width`/`Set Height`), never columns.
# This file is the committed `(FontSize, Width) -> column count` mapping,
# derived empirically (binary search against a real `tput cols` readout, not
# a computed guess) against the font V001 pinned.
#
# Not run by `just tapes` / `run-tapes.sh` — like every `_`-prefixed file it's
# a reference, skipped by convention. It IS a valid, runnable tape (`vhs
# validate tapes/_dimensions.tape` passes) and its one live stanza below is
# the reproducibility proof V002 asks for. Run manually to re-verify after
# any font/VHS/ttyd version bump; regenerate the whole table if the numbers
# move.
#
# ── Fixed rendering settings (recorded here first; V003 codifies these as
#    `_config.tape`, `Source`d by every real tape) ──────────────────────────
#   Set FontFamily "Menlo"     — V001's pin
#   Set FontSize   16          — chosen by V002; no doc pinned a size before
#                                 this, and nothing downstream depends on a
#                                 different one yet
#   Set Padding    0
#   Set Margin     0
#   (WindowBar left unset / default: adds title-bar height, not width — does
#   not affect the column math below, only Height/row math which is out of
#   V002's scope)
#
# With Padding 0 / Margin 0 / WindowBar unset, the *rendered PNG* is exactly
# `Width x Height` px (verified: `sips -g pixelWidth -g pixelHeight` on a
# capture matches the tape's `Set Width`/`Set Height` line for line, at an
# even Width — see the gotcha below).
#
# ── The table ────────────────────────────────────────────────────────────
#   columns | Width (px) | valid range (any px in range -> same column count)
#   --------|------------|---------------------------------------------------
#      40   |    428     | [425, 434]  — added by V005, see note below
#      60   |    628     | [625, 634]  — added by V005, see note below
#      80   |    828     | [825, 834]
#     100   |   1028     | [1025, 1034]
#     120   |   1228     | [1225, 1234]
#     200   |   2028     | [2025, 2034]
#
# The 40/60 rows were added by V005 (not V002) to cover CP-1's width-sweep
# requirement (200/120/100/80/60/40), which V002's original four calibration
# points didn't reach. Same method, not a formula guess: each was verified
# with the identical `tput cols` stanza this file runs below (Width 428 ->
# `^40$`, Width 628 -> `^60$`), both asserted true, and the `Screenshot`
# hashed byte-identical across two in-worktree runs each (628px:
# f3b6d0f2ff78a1dce2ff8b5fc35c58c60c6d06e6d26be7747af26af9d31f391e). Both
# also land exactly 3px off their range's lower edge, same as the original
# four — the new rows confirm the pattern rather than extrapolate past it.
#
# Formula that falls out of it (Menlo @ FontSize 16 under this VHS/ttyd pin):
# cell width = 10px exactly; Width(cols) = 10*cols + 29, and any of the 10
# pixel values per column both round to the same `tput cols` reading — the
# table picks a value 3-4px off the lower edge of that range, never the edge
# itself, so a future off-by-one-px rendering nudge doesn't flip the count.
# Row/Height calibration is a separate question and not in V002's scope;
# `Height 500` below is just "tall enough to see the prompt," not calibrated.
#
# ── Two gotchas found getting here (undocumented upstream, cost real time
#    to isolate — recorded so nobody re-discovers them the hard way) ───────
#
# 1. Any `Output`/`Screenshot`/`Source` path argument that starts with `_`
#    fails to parse UNLESS quoted. `Source _config.tape` is a parse error
#    ("Expected path after Source"); `Source "_config.tape"` is fine. This
#    matters for V003: every real tape will `Source "_config.tape"` — quoted,
#    always, because of the leading underscore, not by style preference.
#
# 2. `Set Width`/`Set Height`/etc. only take effect if they appear before the
#    *first* non-`Set` command in the file (Type/Enter/Wait/Screenshot/...).
#    A `Set Width` after that point is silently accepted by the parser but
#    ignored at runtime (VHS prints a WARN). Consequence: **one tape cannot
#    sweep multiple widths by re-`Set`ting mid-file** — each column count
#    needs its own tape (or its own `Set Width` at the very top). The four
#    rows above were captured as four separate single-width tapes for
#    exactly this reason.
#
# 3. (Sub-finding of the table itself, not a separate gotcha, but easy to
#    trip on the same way:) an ODD `Set Width` breaks the capture — ffmpeg
#    fails with "Padded dimensions cannot be smaller than input dimensions"
#    and neither the GIF nor any `Screenshot` gets written. 829px (the exact
#    midpoint of the 80-column range) fails; 828px (one lower, still in
#    range) works. Reproduced in isolation: 828 -> capture succeeds, 829 ->
#    capture fails, only that one bit changed. Always pick an even Width.
#    Height does not have this constraint (verified 501 succeeds).
#
# 4. `Screenshot` is intermittently unreliable specifically when `vhs` runs
#    with its cwd inside this (sandboxed) worktree — no error, no nonzero
#    exit, the PNG just doesn't appear about half the time (~4/8 in one
#    back-to-back run). The GIF output and the `Wait+Screen` assertion this
#    table's exactness actually rests on were 100% reliable across every
#    trial (n>=10) in both locations — only the bonus Screenshot flakes, and
#    only here. The identical tape run from outside the worktree (`/tmp`)
#    was 3/3. Root cause not chased down (sandbox FS layer vs. the
#    headless-Chrome CDP screenshot write is the leading guess). Flagging for
#    V004/V007: a tape whose pass/fail is a `Wait+Screen` match is fine as-is;
#    anything that treats a missing `Screenshot` file as the failure signal
#    needs a retry, or V007's pixel-diff runner will show sporadic false
#    "reference missing" reports that have nothing to do with the product.
#
# ── Reproducibility proof (the "done when" case) ───────────────────────────
# Two independent runs of the stanza below produced byte-identical
# `dimensions-check.png` (sha256 matched exactly across a rm + re-run, and
# again against a third run from outside the worktree). See V002's report
# for the hashes. The `Wait+Screen` match itself — the actual column-count
# assertion — passed on every single run in this investigation.

Output "dimensions-check.gif"

Require bash

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 16
Set Padding 0
Set Margin 0
Set Width 828
Set Height 500

Type "clear; tput cols"
Enter
Wait+Screen /(?m)^80$/
Screenshot "dimensions-check.png"
