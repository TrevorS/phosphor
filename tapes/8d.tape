# Screen 8d — "80 columns — drop, don't squeeze" (TUI Mockups.dc.html): the
# statusline shed order at exactly 80 columns. Width comes straight from
# `_dimensions.tape`'s calibrated table (828px == 80 cols), not guessed.
#
# STATUS / sentinel: passes. The `main.rs` blocker this header used to name
# is gone (`T090`); corrected during the CP-1 gate pass, which also settled
# the question below.
#
# SETTLED: `status_line.rs`'s doc note was right. The ladder is fit-driven,
# not width-labelled — at a real 80 columns with the mockup's short
# `src/retry.rs` nothing sheds, and with this tape's 98-character absolute
# path the mode word is already `N` and the file already `lib.rs`. Order
# matches 8d's caption; the widths in the drawing do not. Awaiting Teej's
# ruling on which is the contract.
#
# NOTE: this capture is byte-identical to `sweep-80.png`, and it does not
# open a float — so the docked, full-width `Float` that is most of what
# mockup 8d draws is not in this still. `_float-check-fullwidth-80.tape`
# captures that shape; `golden_frames__8d.snap` covers it at Tier 1.

Output "artifacts/8d.gif"

Require phosphor

Source "_config.tape"

Set Width 828
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/8d.png"
