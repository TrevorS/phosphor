# CP-1's four-theme sweep, 1/4 — `Theme::phosphor_dark()` (T010, the
# built-in default), same buffer as the other three `theme-*.tape` files so
# the four can be laid side by side as one contact sheet. Same slice as `9c`
# (which stays as its own tape — it is a mockup-id acceptance target in its
# own right, not just a sweep frame); this is `1a`'s tape with the theme
# argument swapped in, per the convention recorded in this file's own
# "Screen library convention" section — copy, don't reinvent.
#
# STATUS: passes. The `fn main() {}` blocker this header used to name is gone
# (`T090`); corrected during the CP-1 gate pass. No theme flag is needed —
# phosphor-dark IS the default — which is also why this capture is
# byte-identical to `1a.png`, `9c.png` and `sweep-120.png`.

Output "artifacts/theme-phosphor-dark.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-phosphor-dark.png"
