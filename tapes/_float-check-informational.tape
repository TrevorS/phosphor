# Ad hoc investigation, not part of V005's screen library — `_`-prefixed and
# run manually, same convention as `_undercurl-check-*.tape` and
# `_soft-wrap-check.tape`.
#
# Added by the CP-1 gate pass. CP-1 asks for the `Float` contract (`T084`) to
# be checked on a real terminal — both moods, header / body / footer — and
# none of the four committed stills opens a float: `8d.tape` captures the
# 80-column *statusline* shed order and is byte-identical to `sweep-80.png`.
# `--float <mood>` is `T090`'s scaffolding flag for exactly this (no Action
# opens a float until `T021`), so these three tapes give the checkpoint pixel
# evidence for the primitive without touching the committed screen library.
#
# This one: the centered, boxed form at 120 columns. 60-80% of width, centered,
# never within 4 cols of an edge (Design Language §4/§9); border
# `float.informational` #2a5c44; code behind it dimmed to #232823.

Output "artifacts/_float-check-informational.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --float informational fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/_float-check-informational.png"
