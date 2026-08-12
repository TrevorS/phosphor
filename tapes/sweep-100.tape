# CP-1 width sweep — 100 columns. See tapes/sweep-200.tape for the family
# header. Width from `_dimensions.tape`'s calibrated table.

Output "artifacts/sweep-100.gif"

Require phosphor

Source "_config.tape"

Set Width 1028
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/sweep-100.png"
