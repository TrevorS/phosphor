# Screen 9c — "Original — phosphor (working name)" (TUI Mockups.dc.html):
# the reference theme rendering — near-black green-tinted ground, phosphor
# green for claude — that `9a` (Catppuccin) and `8c` (light) are compared
# against. Uses `Theme::phosphor_dark()` (T010, the built-in default), same
# buffer as `1a` for a fair side-by-side.
#
# STATUS / sentinel: see tapes/1a.tape's header — identical situation, same
# contract, different screen id.

Output "artifacts/9c.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/9c.png"
