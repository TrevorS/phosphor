# V002's second open question, part 4/4 — completes the set alongside
# `_undercurl-check-{auto,forced-curl,forced-underline}.tape`.
# `crates/phosphor-buffer/examples/undercurl.rs`'s own header names this
# exact invocation `# V009's other one`; this tape is that.
#
# STATUS: attempted, not yet captured. `vhs` failed 9/9 tries this session
# with "no frames" / "recording failed" — including on
# `_undercurl-check-auto.tape` and `-forced-underline.tape`, both of which
# are already-proven-good siblings that had captured cleanly earlier in this
# same session. That makes it the sandbox's own already-documented
# Screenshot/recording flakiness (tapes/README.md, "Screenshot flakiness in
# this sandboxed worktree"), not a defect in this tape — flagged rather than
# forced. Re-run `vhs _undercurl-check-no-color.tape` (cwd `tapes/`) to get
# the PNG; no `.tape` change should be needed.
#
# The ORIGINAL prediction here — byte-identical to `-forced-underline`,
# because `TERM=xterm-256color` alone already resolves to `Underline` and
# VHS's ttyd always reports that `TERM` — is now suspect on other grounds,
# not just untested: `1a-degraded-nocolor.tape`'s header found this session
# that `crossterm` 0.29 (the pinned version) implements NO_COLOR itself,
# stripping `SetForegroundColor`/`SetBackgroundColor` beneath any code
# phosphor owns. `UnderlineCapability::resolve`'s `TERM`-based *shape*
# decision (curl vs. underline) is a separate mechanism from *colour*, so
# the shape prediction may still hold — but whether `ANCHOR_UNDERCURL` /
# `FAILURE_UNDERCURL`'s colour also survives is exactly the kind of thing
# the 1a finding says not to assume from reading the code. Settle it from
# the capture once one lands, not from this paragraph.
#
# `_`-prefixed: investigation tape, not part of the V005 screen library —
# skipped by run-tapes.sh, run manually.

Output "artifacts/undercurl-check-no-color.gif"

Require undercurl

Source "_config.tape"

Set Width 828
Set Height 240

Hide
Type "NO_COLOR=1 undercurl"
Enter
Show
Wait+Screen@10s /undercurl fixture/
Screenshot "artifacts/undercurl-check-no-color.png"
