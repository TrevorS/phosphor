# V002's second open question, part 1/3 — "does undercurl survive VHS
# capture?" This variant runs `T085`'s fixture (crates/phosphor-buffer/
# examples/undercurl.rs) with NO override, so `UnderlineCapability::resolve`
# sees exactly what VHS's own ttyd/xterm.js session reports for `TERM` — the
# legend line the example prints bakes that value straight into the
# screenshot, so this answers "what does VHS look like to the detector"
# alongside the two forced variants in the sibling `_undercurl-check-*.tape`
# files.
#
# `_`-prefixed: an investigation tape (V002's own method, same shape as
# `_dimensions.tape`/`_config-check.tape`), not part of the V005 screen
# library — skipped by run-tapes.sh, run manually.
#
# Require is the example binary's name, not `phosphor` — this fixture is
# reachable without `crates/phosphor/src/main.rs` (still a stub this
# window), which is exactly why T085 shipped it.

Output "artifacts/undercurl-check-auto.gif"

Require undercurl

Source "_config.tape"

Set Width 828
Set Height 240

Hide
Type "undercurl"
Enter
Show
Wait+Screen@10s /undercurl fixture/
Screenshot "artifacts/undercurl-check-auto.png"
