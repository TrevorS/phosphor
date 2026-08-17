# CP-1 width sweep — 100 columns. See tapes/sweep-200.tape for the family
# header. Width from `_dimensions.tape`'s calibrated table.

Output "artifacts/sweep-100.gif"

Require phosphor

Source "_config.tape"

Set Width 1028
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/sweep-100.png"
