# Screen 9c — "Original — phosphor (working name)" (TUI Mockups.dc.html):
# the reference theme rendering — near-black green-tinted ground, phosphor
# green for claude — that `9a` (Catppuccin) and `8c` (light) are compared
# against. Uses `Theme::phosphor_dark()` (T010, the built-in default), same
# buffer as `1a` for a fair side-by-side.
#
# STATUS / sentinel: passes. See tapes/1a.tape's header — identical
# situation, same contract, different screen id.
#
# NOTE, found during the CP-1 gate pass: this capture is byte-identical to
# `1a.png`, `sweep-120.png` and `theme-phosphor-dark.png` — same file, same
# width, same theme. That is faithful to what this tape says it captures (the
# reference *theme* rendering), but it means 9c's other half — the anchored
# region on line 18, tint + undercurl — appears in no committed still. There
# is no CLI door to the store at S1 to plant one, so that half is covered at
# Tier 1 only, by `golden_frames__9c.snap`, plus the `_undercurl-check-*`
# investigation captures for the cell-style half.

Output "artifacts/9c.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/9c.png"
