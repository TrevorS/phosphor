# CP-1's four-theme sweep, 4/4 — Tokyo Night (`T013`, slug `tokyo-night` in
# `crates/phosphor-ui/src/theme/builtin.rs`), the dark variant. Q7 dropped
# Ayu (its identity colour is orange, which the language reserves for
# attention) and put Tokyo Night in its place; there is no mockup for it,
# so per IMPLEMENTATION-PLAN.md's S1 amendment, `9b` stands as the *shape*
# of this acceptance test — same slice of UI, a second palette, actor
# contract intact — with Tokyo Night substituted for Ayu. Same
# buffer/dimensions as `1a`, theme argument swapped in.
#
# STATUS: passes. Both blockers this header used to list are gone (`T090`
# landed the S1 host and `--theme <slug>`); corrected during the CP-1 gate
# pass, where the slug was confirmed against `builtin.rs`.

Output "artifacts/theme-tokyo-night.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --theme tokyo-night ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/theme-tokyo-night.png"
