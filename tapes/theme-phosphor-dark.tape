# CP-1's four-theme sweep, 1/4 — `Theme::phosphor_dark()` (T010, the
# built-in default), same buffer as the other three `theme-*.tape` files so
# the four can be laid side by side as one contact sheet. Same slice as `9c`
# (which stays as its own tape — it is a mockup-id acceptance target in its
# own right, not just a sweep frame); this is `1a`'s tape with the theme
# argument swapped in, per the convention recorded in this file's own
# "Screen library convention" section — copy, don't reinvent.
#
# STATUS: blocked exactly like `1a`/`9c`/`8c`/`8d` — see `1a.tape`'s header.
# No default-theme selection blocker (phosphor-dark IS the default), so this
# one only needs `crates/phosphor/src/main.rs` to stop being `fn main() {}`.

Output "artifacts/theme-phosphor-dark.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/theme-phosphor-dark.png"
