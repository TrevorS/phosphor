# Screen 6b — "the Steel REPL" (TUI Mockups.dc.html, lines 492-505). CP-2
# names `6b` as one of its three required VHS artifacts.
#
# Types the same four lines, in the same order, as
# `crates/phosphor-steel/tests/screen_6b.rs`'s `TYPED` — which is itself
# transcribed from the mockup. That test already proved what S2's real
# vocabulary answers each one with, and it is not what the mockup draws
# (`⇒ #ok · …`): three of the four reach a free identifier or a
# not-yet-built store (`region-author`, `claude`, `T041`), and the fourth hits
# a shape gap (`place-watch`'s `anchor` wants a Target, the mockup passes a
# string). That is `S2` truth, not a capture bug — the module doc on that test
# has the per-line table, and this tape reproduces it on real pixels rather
# than asserting anything prettier. All four lines fit at 120 columns with no
# wrap.
#
# **Runs against a scratch $PHOSPHOR_RUNTIME**, not the tracked runtime/ —
# the third line is `keymap-set!`, one of `repl.scm`'s persistent heads, so a
# run against the real tree would write to the checked-in `persisted.scm`.
# harness does not own `runtime/**` (TEAM.md).
#
# Sentinels wait on a word unique to each answer (`T041`, `region-author`,
# `claude`, `place-watch`) rather than a generic prompt glyph, so a tape that
# raced ahead of the VM would time out instead of screenshotting a partial
# session.

Output "artifacts/6b.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "rm -rf /tmp/phosphor-6b && cp -R ../runtime /tmp/phosphor-6b && PHOSPHOR_RUNTIME=/tmp/phosphor-6b phosphor --repl --theme phosphor-dark"
Enter
Show
Wait+Screen@10s /(?m)^ (REPL|R)\b/

Type `(unseen-regions "src/retry.rs")`
Enter
Wait+Screen@10s /T041/

Type `(map region-author (block-regions "retry logic"))`
Enter
Wait+Screen@10s /region-author/

Type `(keymap-set! "]r" (lambda () (goto (next-region-by claude))))`
Enter
Wait+Screen@10s /claude/

Type `(watch-place "src/retry.rs:24" 'delay)`
Enter
Wait+Screen@10s /place-watch/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/6b.png"
