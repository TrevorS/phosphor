# CP-1's six-theme sweep, 6/6 — Tokyo Night Day (`T013`, slug
# `tokyo-night-day` in `crates/phosphor-ui/src/theme/builtin.rs`), the light
# variant, sitting next to `theme-tokyo-night.tape` (the dark one) as the
# pair. See `theme-catppuccin-latte.tape`'s header for why this file exists —
# the original four-theme library was missing both light halves of the two
# non-Phosphor mappings. Same buffer/dimensions as `1a`/`8c`, theme argument
# swapped in.
#
# STATUS: same shape as the other five `theme-*.tape` files.

Output "artifacts/theme-tokyo-night-day.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme tokyo-night-day ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-tokyo-night-day.png"
