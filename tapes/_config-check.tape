# V003 — reproducibility proof for `_config.tape`, in the same spirit as
# `_dimensions.tape`'s own live stanza: not part of the screen library
# (`_`-prefixed, skipped by run-tapes.sh by convention — see tapes/README.md)
# but a real, runnable tape, meant to be re-run by hand whenever
# `_config.tape` changes.
#
# What it proves: with `_config.tape` sourced and nothing else varying, two
# runs of the identical tape produce byte-identical PNGs. Deliberately does
# NOT drive `phosphor` — `crates/phosphor/src/main.rs` is still `fn main()
# {}` this window (Window C's job, T019-T026) — so this isolates the CONFIG
# fragment's own determinism from whichever real per-screen tape ends up
# `Source`-ing it. Once a real tape exists it inherits this proof for free:
# same `_config.tape`, same guarantee.
#
# Method: run twice, `shasum -a 256` both PNGs, same hash = done (V003's
# stated "done when"). Output is a loose file directly in tapes/ — gitignored
# scratch (`/tapes/*.png`, see .gitignore), not a committed reference.
#
# Reproduced 2026-08-11 — the exact hash and run count are recorded in
# tapes/README.md's V003 section (this comment intentionally doesn't
# duplicate a value that would go stale if the file changes and nobody
# updates two copies).

Output "config-check.gif"

Require bash

Source "_config.tape"

Set Width 828
Set Height 500

Hide
Type "clear"
Enter
Show
Type "printf 'phosphor tapes: config self-test ready\n'"
Enter
Wait+Screen /config self-test ready/
Screenshot "config-check.png"
