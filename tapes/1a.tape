# Screen 1a — "Default view — code-first" (TUI Mockups.dc.html).
# CP-1 needs this as **1a-minus-agent**: BufferView + gutter + StatusLine
# over a real file, with no transcript/agent pane (that's `T054`, S6, not
# built yet). The mockup's unseen-change gutter markers and claude-signaled
# review block also can't render truthfully until the store exists (`T041`,
# S5) — a fixture would be a value nobody asked for, so this tape captures
# what S1 actually has: layout, chrome, syntax highlighting.
#
# STATUS: structurally complete, cannot pass yet — see tapes/README.md's
# "Screen library convention" section for why (`crates/phosphor/src/main.rs`
# is still `fn main() {}`, Window C's event loop) and what `phosphor` needs
# to be on `$PATH` for `Require phosphor` to even start.
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
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/1a.png"
