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
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/sweep-40.png"
