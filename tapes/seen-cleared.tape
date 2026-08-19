# `CP-5`: *"a clip of `s` clearing a marker and the gutter updating"*.
#
# **The key is `SPC u s`, not `s`, and that is a disagreement worth flagging
# rather than folding.** `CP-5`'s prose says "Mark seen with `s`"; the shipped
# keymap binds bare `s` to vim's *substitute character* and puts mark-seen on
# `SPC u s` (`3c`'s `+unseen · mark seen`). Checked against `(keymap-entries)`
# this session, not assumed. A capture that pressed `s` would record the buffer
# being edited, which is the opposite of the claim. So this presses what ships,
# and the difference is written here for whoever sits the checkpoint — see
# `CLAUDE.md`: where the design and the build disagree, flag it.
#
# **This is the thesis, in three keystrokes.** `CP-5` fails if the markers do
# not change how you read the file, and the loop being tested is "glance, mark
# seen, move on". The GIF is the deliverable — the PNG below is the settled
# end state, which is what a pixel diff can compare.
#
# SENTINEL. The statusline's own count, `1 unseen`, which can only appear after
# the mark lands: this file starts with two unseen regions, and the launch wait
# below is satisfied only once `2 unseen` is drawn. So the two waits bracket
# exactly the transition being recorded.

Output "artifacts/seen-cleared.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /2 unseen/

# Onto line 4, the first unseen region — three `j` rather than a count, so the
# tape depends on nothing but the motions every other tape already uses.
Type "jjj"
Sleep 400ms

# `SPC u s` — mark the region under the cursor seen.
Type " us"
Wait+Screen@10s /1 unseen/

Sleep 500ms
Screenshot "artifacts/seen-cleared.png"
