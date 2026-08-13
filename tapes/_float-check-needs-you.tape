# Ad hoc investigation — see `_float-check-informational.tape`'s header for
# why these three exist and why they are `_`-prefixed.
#
# This one: the needs-you mood. Warmer border `float.needs_you` #6b5426 over a
# darker body #171207 (Design Language §4, both values named there verbatim),
# with the header title in `actor.attention` rather than text.

Output "artifacts/_float-check-needs-you.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --float needs-you ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/_float-check-needs-you.png"
