# Ad hoc investigation — see `_float-check-informational.tape`'s header.
#
# This one is the shape mockup `8d` draws: at 80 real columns the float goes
# full-width and docks to the bottom of the buffer area (§11,
# `Layout::FullWidth`) — rules instead of a box, header / body / footer in the
# same order. It is what `8d.png` would show if `8d.tape` opened a float; it
# does not, and is byte-identical to `sweep-80.png`, so this is the only pixel
# capture of the docked form.
#
# 828px == 80 columns, straight from `_dimensions.tape`'s calibrated table.
#
# The mockup's picker body (per-row selection colours, the query cursor block)
# is `T045` at S5 — the body here is `T084`'s flat `TextBody` fixture, exactly
# as the `8d` golden frame documents.

Output "artifacts/_float-check-fullwidth-80.gif"

Require phosphor

Source "_config.tape"

Set Width 828
Set Height 700

Hide
Type "phosphor --float informational ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/_float-check-fullwidth-80.png"
