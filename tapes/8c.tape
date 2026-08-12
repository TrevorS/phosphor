# Screen 8c — "Light theme — the palette holds" (TUI Mockups.dc.html):
# `Theme::phosphor_light()` (T012) over the same buffer as `9c`, proving the
# actor contract survives the lightness flip — CP-1 asks by eye whether
# claude-green is still the brightest thing on screen here.
#
# STATUS / sentinel: see tapes/1a.tape's header for the main.rs blocker.
# 8c has a SECOND, independent blocker on top of that one: there is no
# confirmed way to select a non-default theme from outside the process yet.
# `phosphor-ui` has `Theme::phosphor_light()` (T012), but nothing in
# `crates/phosphor` wires a flag to it — no arg parsing exists at all
# (`fn main() {}`). The `--theme phosphor-light` flag below is this tape's
# ASSUMPTION, not a confirmed interface — flagged under `contract` in V003-
# V005's report rather than silently guessed at. Update this `Type` line
# once `spine`/`surface` settle the actual shape (CLI flag vs. a
# `phosphor --eval` call vs. something else).

Output "artifacts/8c.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "phosphor --theme phosphor-light ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED)\b/
Screenshot "artifacts/8c.png"
