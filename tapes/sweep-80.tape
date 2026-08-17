# CP-1 width sweep — 80 columns. See tapes/sweep-200.tape for the family
# header. Width from `_dimensions.tape`'s calibrated table. This is also the
# acceptance width for T017's own property test (40-200) and mockup `8d`'s
# heading — `tapes/8d.tape` captures the same width on its own, since it's
# also one of CP-1's four named stills, not just a sweep frame.

Output "artifacts/sweep-80.gif"

Require phosphor

Source "_config.tape"

Set Width 828
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/sweep-80.png"
