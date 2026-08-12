# Screen 1a — "Default view — code-first" (TUI Mockups.dc.html).
# CP-1 needs this as **1a-minus-agent**: BufferView + gutter + StatusLine
# over a real file, with no transcript/agent pane (that's `T054`, S6, not
# built yet). The mockup's unseen-change gutter markers and claude-signaled
# review block also can't render truthfully until the store exists (`T041`,
# S5) — a fixture would be a value nobody asked for, so this tape captures
# what S1 actually has: layout, chrome, syntax highlighting.
#
# STATUS: passes. `T090` landed the S1 host, so the `fn main() {}` blocker
# this header used to describe is gone (corrected during the CP-1 gate pass).
# `phosphor` still has to be on `$PATH` for `Require phosphor` — see
# tapes/README.md's "Running" section for the one-liner that does it.
#
# Sentinel (V004): tapes/README.md's "Sentinel contract" section. Grounded in
# `crates/phosphor-ui/src/status_line.rs` as landed (T017) — the mode chip is
# the one piece never shed to nothing, always the first thing on the row.

Output "artifacts/1a.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Harness finding, this run: without a short settle here, `Screenshot` can
# race the Wait+Screen match — the sentinel regex matches against the
# terminal's text buffer before headless-Chromium's own paint has caught up,
# so the captured PNG intermittently lands one repaint short (missing the
# state bar entirely; cursor stuck at the top-left). Reproduced directly:
# identical tape without this line, run back to back, alternated between a
# ~4KB truncated capture and a correct ~220KB one for the exact same matched
# state. `Sleep 500ms` here (n=3, byte-identical sha256 each time) is a
# capture-pipeline stabilizer, NOT the synchronization primitive — the regex
# above is still what's asserted; this is a settle guard for the *screenshot*
# only, same category as `_dimensions.tape`'s already-documented Screenshot
# flakiness in this sandboxed worktree, not a second wait condition. Flagged
# as a V004 deviation (its "no bare Sleep" convention) rather than silently
# added — see tapes/README.md.
Sleep 500ms
Screenshot "artifacts/1a.png"
