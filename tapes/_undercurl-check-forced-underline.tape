# V002's second open question, part 3/3 — "does undercurl survive VHS
# capture?" `PHOSPHOR_UNDERCURL=0` forces the degradation path (a plain
# `Modifier::UNDERLINED`, no escape) so this is the pixel control the
# forced-curl capture is compared against: same span, same colour, same
# glyphs, the only variable is whether the curl escape was sent at all.
#
# `_`-prefixed, run manually — see `_undercurl-check-auto.tape`'s header for
# the shared rationale.

Output "artifacts/undercurl-check-forced-underline.gif"

Require undercurl

Source "_config.tape"

Set Width 828
Set Height 240

Hide
Type "PHOSPHOR_UNDERCURL=0 undercurl"
Enter
Show
Wait+Screen@10s /undercurl fixture/
Screenshot "artifacts/undercurl-check-forced-underline.png"
