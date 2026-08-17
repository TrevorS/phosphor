# Diagnostics — `CP-4`'s VHS list, third item, and **half of what it asks for
# is not capturable in this window.** The line reads *"a file with real
# diagnostics showing gutter priority against other region states"*, and the
# second clause is exactly why `T040` is unticked: there is one source of
# regions until `T041`. The host concatenates diagnostic regions with every
# other source and calls `gutter::state_column` once — the composition is
# written and the `Vec` has one element in it, so *"priority against other
# states"* has nothing to be correct against. The ladder is unit-tested in
# `phosphor-ui`'s `gutter`, which is where the claim lives until the store
# exists. **This tape is the first clause and says so**; it is not the
# checkpoint item, and re-capturing it after `T041` with a second source in
# that `Vec` is what would make it one.
#
# `7c-rust.tape` carries the `S4` writeup — why the server is the fixture,
# what `lsp-fixture.sh` swaps, why the readiness sentinel is the statusline
# chip. Only what differs is here.
#
# **Nobody presses a key, and that is the property.** The fixture server runs
# in `diagnostics` mode: it publishes one error on `didOpen` and answers no
# lookups. The editor was parked in `Queue::recv` with no timeout and no tick,
# so what reaches the screen arrives on the same queue the keyboard uses, from
# a process that decided to say something. `crates/phosphor/tests/loop_pty.rs`'s
# `a_published_diagnostic_reaches_the_screen_with_nobody_asking` reads the same
# row off a real pty with no key pressed at all — green this session before
# this tape was written.
#
# **The mode is not a flag on the same process.** One server cannot both
# answer lookups and publish under a frame-counting harness — the fixture's own
# header says why — so this capture and the three `7c` ones are different
# processes, which is also why this tape presses nothing and they type.
#
# **The range is the server's constant and the fixture is written around it**:
# line 1, characters 0..5, zero-based and half-open. `fixtures/policy.rs`
# therefore starts its second line at column 0 with a five-character word, and
# with a line about which `expected Duration, found u128` is a true sentence.
#
# Sentinel: the message itself, which cannot appear until the publish has
# crossed the queue and been composed — the buffer behind it never contains it.
#
# **This tape presses one key now, and it used to press none.** `CP-4` reported
# eleven cascade parse errors drawn as eleven rows, so an inline diagnostic row
# is bounded to the cursor's line (`RowPolicy`, `runtime/init.scm`'s
# `diagnostic-rows`). The publish is on file line 2 — the server's constant, as
# the paragraph above says — and a buffer opens on line 1, so the sentinel this
# tape waits for is on a line the cursor is not on and the capture timed out
# rather than failing loudly. `j` is that one key.
#
# **The screen this captures is still the one it always captured**, which is
# why the reference is a reviewed update rather than a new screen: the row, the
# undercurl and the state bar are unchanged; what moved is which line has to be
# under the cursor for the row to speak. The statusline now also carries `■1`,
# which is §3's third surface and is the whole reason bounding the rows is
# quieting rather than hiding.

Output "artifacts/diagnostics.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh diagnostics rust diagnostics policy.rs && cd /tmp/phosphor-tape-diagnostics && PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark policy.rs"
Enter
Show
# Onto the line the publish is about, so the row draws at all. See the header.
Type "j"
Wait+Screen@10s /expected Duration, found u128/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/diagnostics.png"
