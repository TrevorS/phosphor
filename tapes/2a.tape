# Screen 2a — "The unseen picker" (TUI Mockups.dc.html), and one of the five
# captures `CP-5` names that did not exist.
#
# **This is the first tape that draws the store**, which is why `tape-env.sh`
# grew an `XDG_STATE_HOME` and `seed-state.sh` exists. Everything the picker
# lists here came out of `fixtures/seed/plan.scm` through `phosphor --eval` —
# no test-only backdoor, which is `V006`'s own criterion — and
# `scripts/seed-determinism.sh` is what says two runs of that plan leave the
# same store. Without that this capture would be measuring the seed.
#
# WORKSPACE ROOT. `cd ../fixtures` before launching, not a path argument, and
# not `tapes/fixtures/`. Seen-state keys on the canonicalised directory the
# editor started in (`T044`), and the seed keyed its store on `fixtures/`, so
# the editor has to start there or the store reads empty and this draws an
# empty picker. `PHOSPHOR_RUNTIME=../runtime` still resolves — from
# `fixtures/`, that is the same tracked tree it is from `tapes/`.
#
# SENTINEL, and a vhs gotcha worth the two lines. Not the word "unseen": the
# statusline draws it too (`2 unseen`), from launch, so the wait would satisfy
# itself before the float opened. And **not the picker's matched/total counter**, which
# was the first choice and never matched — `/` is `Wait+Screen`'s own pattern
# delimiter, so `4\/4` does not parse as an escaped slash and the wait silently
# times out. vhs then skips the rest of the tape and still exits 0, so the only
# symptom is a missing PNG beside a GIF that recorded fine.
#
# A picker row is the honest sentinel anyway: `retry.rs:4` appears in the
# float and nowhere else on the screen — the statusline draws the path without
# a line number. It says the store was read, not merely that a float opened.

Output "artifacts/2a.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

# `SPC u l` — `3c`'s `+unseen · list`, the shipped binding, over the seeded
# store rather than a hand-built list.
Type " ul"
Wait+Screen@10s /retry\.rs:4/

# The settle guard `1a.tape` documents at length: the regex matches the
# terminal's text buffer, which can be one repaint ahead of headless
# Chromium's paint, and a `Screenshot` that races it lands truncated.
Sleep 500ms
Screenshot "artifacts/2a.png"
