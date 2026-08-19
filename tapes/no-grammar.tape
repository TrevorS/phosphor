# `CP-5`: *"the no-grammar file showing line-fallback markers"*, and `T043`'s
# criterion — markers work on a file nothing parses.
#
# **`fixtures/src/deploy` is extensionless on purpose.** Every other file in
# that tree carries one of the twelve declared languages, deliberately, so
# until this fixture there was nothing in the seeded workspace the node tier
# could fail on. An anchor over its lines resolves at the *line* tier because
# there is no syntax tree to name a node in; the marker draws either way, and
# that is the whole claim this screen makes.
#
# The degradation has to feel **honest rather than broken** — `CP-5`'s own
# words, and the half of it a capture cannot answer. What a capture can answer
# is whether anything draws at all, which is what a regression would take away.
#
# SENTINEL. `1 unseen` in the statusline: this file carries exactly one seeded
# region. Waiting on the count rather than on any glyph means the wait is
# satisfied by the *store* having been read for a file with no grammar, which
# is the thing under test.

Output "artifacts/no-grammar.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/deploy"
Enter
Show
# Two waits, and each answers a different question. The first is every tape's:
# the editor is drawing. The second is this screen's: the store was read *for a
# file with no grammar*. A single wait on the count alone raced the first frame
# and timed out — vhs then skips the rest and exits 0, leaving a GIF and no PNG
# (see `2a.tape` on that failure mode).
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
Wait+Screen@10s /1 unseen/

Sleep 500ms
Screenshot "artifacts/no-grammar.png"
