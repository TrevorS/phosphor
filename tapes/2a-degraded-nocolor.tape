# `V009` — screen 2a with `NO_COLOR=1`. The other degradation axis, and the
# harder one: not a narrower palette but *no* palette.
#
# **This is where §1 is under the most pressure.** "Green always means claude"
# is the design's first law, and an unseen region is exactly claude's writing
# you have not read — so a terminal that will not draw green has to say the
# same thing another way. §3's answer is the `▎` marker in place of a filled
# block, carrying the meaning in shape once hue is gone.
#
# `V009` recorded this capture as impossible while `T046` was unbuilt and while
# nothing put regions in front of a screen. Both are ticked, and
# `tapes/seed-state.sh` supplies the store, so the picker has rows and the
# gutter has markers to degrade.
#
# `NO_COLOR=1` is the same spelling `1a-degraded-nocolor.tape` uses — the
# informal standard the crate honours, distinct from `TERM` because a terminal
# can be perfectly capable and still be told not to.
#
# WHAT THIS CAPTURE FOUND, and it is the reason V009 asks for it. The gutter's
# unseen markers are **not there**. §8 says markers become `▎` when colour is
# gone, `gutter::state_cell` implements exactly that as `Fill::Marker`, and
# `gutter`'s own unit tests cover it — but `grep -rn "Fill::" crates/` outside
# that module returns **one line**, `buffer_view.rs:564`, and it hardcodes
# `Fill::Block`. Nothing in the shipping editor can ever select the marker. So
# the block is drawn, its background colour is dropped, and the state column
# says nothing at all.
#
# That is `T016`'s shape one layer over — a path that exists, is tested, and
# no configuration reaches — and it is a product finding for `CP-5`, whose
# failure condition is that the markers do not change how you read the file.
# On this terminal there are none to read. Recorded rather than fixed here:
# which terminal capability should select the fill, and where that detection
# lives, is a design question and not a drive-by.
#
# FIXED, AND THIS CAPTURE IS NOW THE TRIPWIRE FOR IT. Two paragraphs above are
# history: `BufferView::fill` made the marker askable, `phosphor_term::
# colour_available` answers the capability question, and the frame loop
# selects it. **The committed reference has shown `▎` since then** — measured
# 2026-08-20 at pixel (0, 70), inside row 4's state cell: `#C4CCC5` here,
# a glyph left in the terminal's default foreground because `NO_COLOR` dropped
# its hue, against `#3BDB94` at the same pixel in `2a.png`, which is a solid
# block of `actors.claude`.
#
# And it bites. `T088`'s collapse moved the buffer into the view tree, where
# the `Node::Buffer` arm takes `Fill::Block` by default; a build that dropped
# the fill on the way through — the naive collapse, and the exact regression
# the plan named — mismatches this capture by **515 px beyond the 0.6%
# tolerance**, measured against two separately-built binaries. `1a-degraded-
# nocolor` does **not** bite: that slice is unseeded, so its state column has
# nothing in it, and its own header already says the `▎` path is not one of
# the three it exercises. So of the two degraded captures, this is the only
# one holding §8's marker, and `the_fill_reaches_a_tree_composed_buffer` in
# `phosphor-ui`'s interpreter is the headless half of the same proof.
#
# SENTINEL. The picker row, as `2a.tape` uses and for its reason: `retry.rs:4`
# is in the float and nowhere else, so it says the store was read. A `/` cannot
# appear in a `Wait+Screen` pattern — see that tape's header.

Output "artifacts/2a-degraded-nocolor.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime NO_COLOR=1 phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

Type " ul"
Wait+Screen@10s /retry\.rs:4/

Sleep 500ms
Screenshot "artifacts/2a-degraded-nocolor.png"
