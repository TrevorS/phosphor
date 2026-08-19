# Screen 6a — `:arch`, the editor's own architecture drawn over the live store
# (TUI Mockups.dc.html). One of the five `CP-5` names.
#
# **`T048`'s third clause is what this captures**: the screen adds zero lines
# to `phosphor-ui`. Every row of `runtime/arch.scm` is `view/spans`, and
# `scripts/lint-one-escape-hatch.sh` proves that is the only custom-draw path
# there is — a Rust primitive for this screen would have to appear in the
# widget crate to be drawn at all. So this is a mockup screen reproduced
# entirely in the editor layer, which is the claim worth a picture.
#
# Seeded, because the second clause is that it reflects the **actual** store:
# the counts in the boxes are read live, not drawn from a constant.
#
# SENTINEL. `semantic store` — a box label `runtime/arch.scm` writes, on a
# screen nothing else draws. `arch` alone would match the `:arch` still echoing
# on the ex line before the float opens.

Output "artifacts/6a.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

Type ":arch"
Enter
Wait+Screen@10s /semantic store/

Sleep 500ms
Screenshot "artifacts/6a.png"
