# CP-1's four-theme sweep, 2/4 — `Theme::phosphor_light()` (T012). Same
# buffer and dimensions as the other three `theme-*.tape` files. Same slice
# as `8c` (which stays as its own tape — a named acceptance target, not just
# a sweep frame); this is `1a`'s tape with the theme argument swapped in.
#
# STATUS: blocked exactly like `8c` — see `1a.tape`'s header for the
# main.rs blocker and `8c.tape`'s header for the SECOND, independent
# blocker this one shares: `--theme phosphor-light` is an ASSUMED flag
# shape, not a confirmed interface (no arg parsing exists at all yet).
# Update the `Type` line alongside `8c.tape`'s if the real shape differs.

Output "artifacts/theme-phosphor-light.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme phosphor-light ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/theme-phosphor-light.png"
