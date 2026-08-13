# Ad hoc investigation, not part of V005's screen library (not one of CP-1's
# four named stills or the sweep/theme sheets) — `_`-prefixed and run
# manually, same convention as `_undercurl-check-*.tape`. Exists to answer a
# question the CP-1 task brief raised directly: does the `↪` soft-wrap
# continuation marker (line-number colour `#414b42`, 2.12 contrast against
# ground) read on a real captured frame, or only in a Tier-1 cell-grid dump?
#
# `status_line.rs`'s own long doc-comment lines (198-206 chars) are all past
# the first screenful, so reaching one costs ~500 scripted `Down` presses for
# no benefit over a purpose-built fixture — a 206-char comment line typed here
# instead (`/tmp/soft-wrap-fixture.rs`, not committed, built by this
# investigation only) wraps at 100 columns on line 1, visible with no
# scrolling. `--soft-wrap` is T090's scaffolding flag for exactly this (no
# Action to toggle it exists yet).

Output "artifacts/_soft-wrap-check.gif"

Require phosphor

Source "_config.tape"

Set Width 1028
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --soft-wrap /tmp/soft-wrap-fixture.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
Sleep 500ms
Screenshot "artifacts/_soft-wrap-check.png"
