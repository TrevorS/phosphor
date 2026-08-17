# The same published diagnostic with the undercurl forced on — the primary
# terminal's treatment of the range, beside `diagnostics.png`'s degraded one.
#
# `diagnostics.tape` carries the writeup for everything else about this screen,
# including which half of `CP-4`'s line it does not answer.
#
# **Why this tape has to exist at all, and it is a fact about VHS rather than
# about phosphor.** `UnderlineCapability::detect`
# (`vendor/ratatui-code-editor/src/phosphor/cell_style.rs`) resolves from the
# process environment and `vhs`'s own ttyd session reports
# `TERM=xterm-256color`, which the table resolves to `Underline`. So the
# *default* capture of any underlined range in this library is the degradation
# path, and the primary terminal's look needs the escape hatch —
# `PHOSPHOR_UNDERCURL=1`, prefixed inline onto the typed command, the same
# pattern `1a-degraded-nocolor.tape` and the `_undercurl-check-*` trio already
# use rather than VHS's own `Env` directive. `tapes/README.md`'s V002 section
# recorded that consequence for `V009` and this is the first tape in the
# library to have a *product* surface to spend it on: the `_undercurl-check-*`
# trio drove `phosphor-buffer`'s standalone example, because when they were
# written nothing in the shipping binary reached
# `UnderlineCapability::resolve`. `main.rs`'s `set_styled_spans` does now.
#
# **`CP-4`'s manual half asks for exactly this pair** — *"undercurl on the
# primary terminal; underline fallback on the degradation terminal"* — and a
# tape cannot answer it, because the question is about two real terminals.
# What this pair answers is the half that is a fact about the build rather
# than about the hardware: the same styled span, the same theme, the same
# file, one environment variable apart. Teej's half is unchanged.

Output "artifacts/diagnostics-undercurl.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh diagnostics-undercurl rust diagnostics policy.rs && cd /tmp/phosphor-tape-diagnostics-undercurl && PHOSPHOR_UNDERCURL=1 PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark policy.rs"
Enter
Show
# Onto the line the publish is about: an inline row is bounded to the cursor's
# line since `CP-4` (`RowPolicy`), and the publish is on file line 2. Same one
# key as `diagnostics.tape`, whose header carries the reasoning.
Type "j"
Wait+Screen@10s /expected Duration, found u128/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/diagnostics-undercurl.png"
