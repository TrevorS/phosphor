# CP-1's four-theme sweep, 4/4 — Tokyo Night (`T013`, slug `tokyo-night` in
# `crates/phosphor-ui/src/theme/builtin.rs`), the dark variant. Q7 dropped
# Ayu (its identity colour is orange, which the language reserves for
# attention) and put Tokyo Night in its place; there is no mockup for it,
# so per IMPLEMENTATION-PLAN.md's S1 amendment, `9b` stands as the *shape*
# of this acceptance test — same slice of UI, a second palette, actor
# contract intact — with Tokyo Night substituted for Ayu. Same
# buffer/dimensions as `1a`, theme argument swapped in.
#
# STATUS: same two blockers as `theme-catppuccin.tape` — `main.rs` still
# `fn main() {}`, and `--theme <slug>` is this tape's ASSUMPTION, not a
# confirmed CLI interface.

Output "artifacts/theme-tokyo-night.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme tokyo-night ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/theme-tokyo-night.png"
