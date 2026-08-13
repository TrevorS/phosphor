# CP-1's four-theme sweep, 3/4 — Catppuccin Mocha (`T013`, slug
# `catppuccin-mocha` in `crates/phosphor-ui/src/theme/builtin.rs`), the dark
# variant, to sit next to the other three dark-background frames in the
# contact sheet. This is the screen `9a` refers to in the mockups — no `9a`
# tape exists separately (unlike `9c`/`8c`), so this file is that acceptance
# target's only capture. Same buffer/dimensions as `1a`, theme argument
# swapped in per this file's own convention note.
#
# STATUS: passes. Both blockers this header used to list are gone (`T090`),
# and the slug below was confirmed against `builtin.rs`'s `catppuccin-mocha`
# during the CP-1 gate pass. Corrected there.

Output "artifacts/theme-catppuccin.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --theme catppuccin-mocha ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-catppuccin.png"
