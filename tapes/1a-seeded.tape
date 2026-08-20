# `CP-5`'s flagship — `1a` in full, against the `V006` fixture.
#
# The checkpoint's VHS list opens with *"`1a` in full against the `V006`
# fixture"*, and the `1a` this library already had is not that screen. `1a.tape`
# opens `tapes/fixtures/core-lib.rs`, the frozen file §40 repointed all
# twenty-five tapes at, with an **empty** store — so the flagship screen was the
# one screen here drawn with nothing behind it. `1a-minus-agent` is the golden
# frame `CP-1` accepts on, and its name says what it leaves out.
#
# This is the other half: `fixtures/src/retry.rs` with the seeded store, so the
# gutter has something to say. `run-tapes.sh` and `diff-tapes.sh` both seed a
# capture that `cd`s into `../fixtures` — `needs-seed.sh` derives that from the
# tape rather than from a list — so the store below is `fixtures/seed/plan.scm`
# and nothing here has to arrange it.
#
# **A second tape rather than repointing `1a.tape`.** `1a` is a `CP-1` artifact
# and `1a-degraded-term`/`1a-degraded-nocolor` are `V009`'s variants of exactly
# that frame; changing what it photographs would settle a `CP-5` item by
# invalidating three `CP-1`/`V009` ones. The same argument `diagnostics-regions`
# makes against re-capturing `diagnostics.png`.
#
# What the screen shows that `1a.png` cannot: `●` markers in the state column on
# the lines `plan.scm` declares for `src/retry.rs` (4, 6–10, 12–24), and the
# statusline's own unseen counter. That is the whole of `CP-5`'s thesis on one
# frame — *"does the gutter pull your eye to the right places"* — and it is the
# frame Teej is meant to be looking at when answering it.

Output "artifacts/1a-seeded.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show

# Two waits, and they say different things: the first that the editor is
# drawing, the second that it read the store. `no-grammar.tape`'s header has the
# finding behind that pair — a single wait on state alone raced the first frame,
# timed out, and `vhs` skipped the rest of the tape while still exiting 0.
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
Wait+Screen@10s /unseen/

# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/1a-seeded.png"
