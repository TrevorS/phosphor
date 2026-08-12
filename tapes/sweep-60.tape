# CP-1 width sweep — 60 columns. See tapes/sweep-200.tape for the family
# header. Width verified empirically by V005 (628px -> tput cols 60) since
# V002's original table stopped at 80 — see _dimensions.tape's header for
# the reproducibility record.

Output "artifacts/sweep-60.gif"

Require phosphor

Source "_config.tape"

Set Width 628
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/sweep-60.png"
