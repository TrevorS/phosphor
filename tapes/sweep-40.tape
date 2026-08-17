# CP-1 width sweep — 40 columns, the narrowest point on the ladder. See
# tapes/sweep-200.tape for the family header. Width verified empirically by
# V005 (428px -> tput cols 40) — see _dimensions.tape's header. This is the
# width where T017's own test (`the_last_standing_set_survives_every_step`)
# says only `✻` / `●n` / `!` should still be standing — worth checking by eye
# once this can run.

Output "artifacts/sweep-40.gif"

Require phosphor

Source "_config.tape"

Set Width 428
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/sweep-40.png"
