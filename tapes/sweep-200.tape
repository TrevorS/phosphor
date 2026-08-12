# CP-1 width sweep — 200 columns. One of six (200/120/100/80/60/40); each
# needs its own tape because `_dimensions.tape`'s gotcha #2 means `Set Width`
# can't be re-set mid-file. Same buffer across all six, so the contact sheet
# shows the shed order step by step rather than six unrelated screens.
# Width is `_dimensions.tape`'s calibrated value for this column count, not
# guessed. STATUS / sentinel: see tapes/1a.tape's header.

Output "artifacts/sweep-200.gif"

Require phosphor

Source "_config.tape"

Set Width 2028
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/sweep-200.png"
