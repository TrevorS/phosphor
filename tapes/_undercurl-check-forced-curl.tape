# V002's second open question, part 2/3 — "does undercurl survive VHS
# capture?" `PHOSPHOR_UNDERCURL=1` bypasses `UnderlineCapability::resolve`
# entirely (see the fixture's own header) so this forces the SGR `4:3`
# escape onto VHS's renderer regardless of what its `TERM` actually
# declares — the point is to test the *rendering pipeline* (ttyd -> xterm.js
# in headless Chromium -> PNG), not the detection logic, which is already
# unit-tested in crates/phosphor-buffer/tests/undercurl.rs.
#
# `_`-prefixed, run manually — see `_undercurl-check-auto.tape`'s header for
# the shared rationale.

Output "artifacts/undercurl-check-forced-curl.gif"

Require undercurl

Source "_config.tape"

Set Width 828
Set Height 240

Hide
Type "PHOSPHOR_UNDERCURL=1 undercurl"
Enter
Show
Wait+Screen@10s /undercurl fixture/
Screenshot "artifacts/undercurl-check-forced-curl.png"
