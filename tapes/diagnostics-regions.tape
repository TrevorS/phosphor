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

Output "artifacts/diagnostics-regions.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh diagnostics-regions rust diagnostics policy.rs && cd /tmp/phosphor-tape-diagnostics-regions"
Enter
Type `PHOSPHOR_RUNTIME=runtime phosphor --eval '(declare-regions! (list (hash "path" "policy.rs" "span" (hash "start" (hash "line" 1 "column" 1) "end" (hash "line" 8 "column" 2)) "author" "claude")))'`
Enter
Type "PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark policy.rs"
Enter
Show

# The editor is drawing, the store was read, and the server has published.
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
Wait+Screen@10s /1 unseen/
Type "j"
Wait+Screen@10s /expected Duration, found u128/

# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/diagnostics-regions.png"
