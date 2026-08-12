# CP-1's four-theme sweep, 3/4 — Catppuccin Mocha (`T013`, slug
# `catppuccin-mocha` in `crates/phosphor-ui/src/theme/builtin.rs`), the dark
# variant, to sit next to the other three dark-background frames in the
# contact sheet. This is the screen `9a` refers to in the mockups — no `9a`
# tape exists separately (unlike `9c`/`8c`), so this file is that acceptance
# target's only capture. Same buffer/dimensions as `1a`, theme argument
# swapped in per this file's own convention note.
#
# STATUS: same two blockers as `theme-phosphor-light.tape` — `main.rs`
# still `fn main() {}`, and `--theme <slug>` is this tape's ASSUMPTION about
# a CLI interface that does not exist yet, not a confirmed one. Once real,
# confirm the slug string matches `builtin.rs`'s `catppuccin-mocha` exactly.

Output "artifacts/theme-catppuccin.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme catppuccin-mocha ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/theme-catppuccin.png"
