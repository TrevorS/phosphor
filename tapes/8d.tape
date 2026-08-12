# Screen 8d — "80 columns — drop, don't squeeze" (TUI Mockups.dc.html): the
# statusline shed order at exactly 80 columns. Width comes straight from
# `_dimensions.tape`'s calibrated table (828px == 80 cols), not guessed.
#
# STATUS / sentinel: see tapes/1a.tape's header — same main.rs blocker, no
# second blocker (default dark theme, no flags needed). The one thing worth
# re-checking once this can actually run: `status_line.rs`'s own doc note
# that mockup 8d draws the *end* of the shed ladder under a width heading
# that doesn't match 80 real columns — "at a real 80 columns this widget
# keeps more, because more fits." This tape is what settles that, once it
# can run.

Output "artifacts/8d.gif"

Require phosphor

Source "_config.tape"

Set Width 828
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/8d.png"
