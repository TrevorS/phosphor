# CP-1's six-theme sweep, 5/6 — Catppuccin Latte (`T013`, slug
# `catppuccin-latte` in `crates/phosphor-ui/src/theme/builtin.rs`), the light
# variant, sitting next to `theme-catppuccin.tape` (Mocha) as the pair.
# `CP-1`'s task brief calls for all SIX shipped themes on one buffer, not the
# four the original tape library covered (phosphor dark/light, catppuccin
# mocha only, tokyo-night only) — this file and `theme-tokyo-night-day.tape`
# are the two that close that gap. Same buffer/dimensions as `1a`/`8c`, theme
# argument swapped in per the family's own convention note (`theme-catppuccin.
# tape`'s header).
#
# STATUS: same shape as the other five `theme-*.tape` files — needs
# `crates/phosphor/src/main.rs` to be a real host (T090, landed this window)
# and `--theme` to accept this slug (confirmed against `builtin.rs`'s
# `BUILTIN_SLUGS`, not guessed).

Output "artifacts/theme-catppuccin-latte.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme catppuccin-latte ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-catppuccin-latte.png"
