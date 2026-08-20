# `CP-4`'s gutter-priority screen — a diagnostic and an unseen region on the
# same rows, which is the half `diagnostics.tape` cannot show.
#
# `CP-4`'s VHS list asks for *"a file with real diagnostics showing gutter
# priority **against other region states**"*. `diagnostics.png` is the first
# clause and `tapes/README.md` says exactly why it is only the first: there was
# one source of regions until `T041`, so `gutter::state_column` was called with
# a `Vec` holding a single element and there was nothing for the priority to be
# *against*. That file's own words — *"re-capturing it once `T041` puts a
# second source in that `Vec` is what would make it the checkpoint item"*.
#
# `T041` shipped. This is that screen.
#
# **A new tape rather than a re-capture, deliberately.** `diagnostics.png` is a
# `CP-4` artifact in its own right and the undercurl pair is measured against
# it (README's Signal-3 table, `x=62..112`). Rewriting what it photographs
# would settle the checkpoint item by invalidating a different one.
#
# # What is on the screen, and why the region overlaps the diagnostic
#
# The toy server publishes one error on **line 2** of `policy.rs` on `didOpen`.
# The declaration below puts an unseen region over **lines 1–8**, so:
#
#   * line 2 carries a diagnostic *and* an unseen region — the row the
#     checkpoint item is about. §3's ladder is trouble > attention > claude, so
#     the state bar there must be trouble-red, not claude-green.
#   * lines 1 and 3–8 carry the region alone and stay green.
#
# A region that merely sat *near* the diagnostic would show two colours in one
# column and prove nothing about precedence: the ladder is only observable
# where two states compete for one cell.
#
# # The store is seeded through the door, into this tape's own scratch
#
# `seed-state.sh` seeds `fixtures/`, which is a different workspace — seen-state
# keys on the canonicalised directory the editor starts in (Q1), and this tape
# starts in `/tmp/phosphor-tape-diagnostics-regions`. So the declaration runs
# there, through `phosphor --eval`, which is the same CLI door the `V006` seed
# plan uses and not a test-only backdoor.
#
# `tape-env.sh` has already pointed `$XDG_STATE_HOME` at a scratch, so the
# `--eval` and the editor below share one store and the operator's own is
# untouched.

# # KNOWN FLAKE — `OPEN-QUESTIONS.md` §43, and it is the editor's, not this
# # tape's
#
# This screen renders its region on lines 4–8 or on 5–9, about one open in
# five, at exactly 209 px. The committed reference is one of the two outcomes,
# so `just tapes-diff` will call this a mismatch occasionally. **It is not a
# drawing change and must not be blessed as one.** §43 has the measurements
# that ruled out the settle, the store and the contested cell, and names
# `Regions::reanchor_in` as where it almost certainly lives.

Output "artifacts/diagnostics-regions.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
# **The store is cleared first, and it has to be.** `declare-regions!` appends,
# and `$XDG_STATE_HOME` outlives a capture — seen-state keys on the workspace
# root, which is the same path every run, so a second capture into an uncleared
# store draws a second region. Measured: this tape matched, mismatched and
# matched again over three consecutive captures before this line existed.
# `seed-state.sh` clears for the same reason and says so in the same words.
Type `rm -rf "$XDG_STATE_HOME/phosphor"`
Enter
Type "bash lsp-fixture.sh diagnostics-regions rust diagnostics policy.rs && cd /tmp/phosphor-tape-diagnostics-regions"
Enter
Type `PHOSPHOR_RUNTIME=runtime phosphor --eval '(declare-regions! (list (hash "path" "policy.rs" "span" (hash "start" (hash "line" 4 "column" 1) "end" (hash "line" 8 "column" 2)) "author" "claude")))'`
Enter
Type "PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark policy.rs"
Enter
Show

# The editor is drawing, the store was read, and the server has published.
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
Wait+Screen@10s /1 unseen/
Type "j"
Wait+Screen@10s /expected Duration, found u128/

# **1500ms, not the library's usual 500.** Measured: at 500ms this tape matched
# three captures in four and drew its state column one row off in the fourth —
# the region and the diagnostic land in the gutter from different directions
# (the store on open, the server on publish) and the last recomposition can fall
# after the virtual text this tape waits on. Waiting on that text proves the
# diagnostic *arrived*, not that the column has settled around it.
#
# This is the one screen in the library where two region sources compete for one
# cell, which is exactly why it is the one that needs the longer settle.
Sleep 1500ms
Screenshot "artifacts/diagnostics-regions.png"
