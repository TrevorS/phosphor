# CP-1's four-theme sweep, 2/4 — `Theme::phosphor_light()` (T012). Same
# buffer and dimensions as the other three `theme-*.tape` files. Same slice
# as `8c` (which stays as its own tape — a named acceptance target, not just
# a sweep frame); this is `1a`'s tape with the theme argument swapped in.
#
# STATUS: passes. Both blockers this header used to list are gone — `T090`
# landed the S1 host and `--theme <slug>`, so the flag below is the real
# interface. Corrected during the CP-1 gate pass.

Output "artifacts/theme-phosphor-light.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme phosphor-light ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-phosphor-light.png"
