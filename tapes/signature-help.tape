# Signature help — `CP-4`'s VHS list, second item, and the one the mockup
# cannot supply. `TUI Mockups.dc.html`'s `7c` is captioned *"lsp completion +
# signature help"* and **draws only the completion float**; signature help is
# in the caption and nowhere in the picture. So this is a descriptive name
# rather than a screen id, following the library's own precedent for a capture
# with no mockup behind it (`repl-liveness`, `folds`).
#
# `7c-rust.tape` carries the `S4` writeup — why the server is the fixture, what
# `lsp-fixture.sh` swaps, and why the readiness sentinel is the statusline
# chip. Only what differs is here.
#
# **Verified wired before capturing.** `crates/phosphor/tests/loop_pty.rs`'s
# `signature_help_and_hover_reach_the_same_passive_float` presses `<C-s>` in
# insert against the same fixture server on a real pty and reads
# `policy: RetryPolicy` and the documentation under the rule off the frame;
# `signature_help_survives_the_argument_being_typed_under_it` presses the
# second half this tape scripts. Both green this session before this was
# written.
#
# **Three frames, and the middle one is the `CP-4` repair.** The rule was
# *"the next key is the answer to have you read it"* for hover and signature
# help alike, and at `CP-4` that meant the first character of the first
# argument cleared the float the argument exists to be typed under
# (`docs/TASKS.md`, `T039`). It now lives until the insert session ends. So:
#
#   1 `open`        — `<C-s>` inside `retry(`, the signature drawn with the
#                     active parameter in its own tone.
#   2 `typing`      — `policy` typed *under* it. The float is still up, and
#                     the proof is negative: `UNDER_THE_FLOAT_IS_THIS_LINE`
#                     and the line below it are still covered. The fixture's
#                     own header explains why the assertion has to be shaped
#                     that way.
#   3 `dismissed`   — `<esc>` ends the insert session, the float goes, and the
#                     two covered rows repaint. That repaint is what this tape
#                     waits on, so the dismissal is a positive V004 sentinel
#                     rather than a `Sleep` (which is what `8e.tape` had to
#                     settle for, and it says so).
#
# **Typing `retry(` does not open a completion list over the signature**, and
# that is a fact about the fixture rather than luck: the client filters by the
# typed prefix (`T038`'s repair, `phosphor_buffer::lsp::narrow`), and none of
# `default` / `default_delay` / `defaults_for` can become `retry`. A tape
# written against a server that offered anything matching would capture two
# floats fighting for the frame.
#
# **`<C-s>` is safe here even though it is XOFF.** The editor is in raw mode
# (`phosphor-term`), which clears `IXON`, so the byte reaches the application
# rather than freezing the pty. Confirmed by capture, not by reasoning about
# termios.
#
# Navigation: `G` lands on the empty line a trailing newline leaves, `k k k`
# walks back over `}` and the two shouting lines to the first of them, and `O`
# opens above it — so the call is typed directly on top of the rows the float
# will cover. `O` copies the current line's indent; nothing below types a
# leading space.

Output "artifacts/signature-help.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh signature-help rust completion call.rs && cd /tmp/phosphor-tape-signature-help && PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark call.rs"
Enter
Show
Wait+Screen@10s /toy-lsp/

Type "G"
Type "k"
Type "k"
Type "k"
Type "O"
Type "retry("
Ctrl+S
Wait+Screen@10s /policy: RetryPolicy/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/signature-help-open.png"
# …and a second one *after* it, which the library did not need until this
# tape. See the header: a `Screenshot` is asynchronous to the key stream, and
# without this the next key's frame lands in the previous key's PNG.
Sleep 500ms

Type "policy"
Wait+Screen@10s /retry\(policy/
Sleep 500ms
Screenshot "artifacts/signature-help-typing.png"
Sleep 500ms

Escape
Wait+Screen@10s /UNDER_THE_FLOAT_IS_THIS_LINE/
Sleep 500ms
Screenshot "artifacts/signature-help-dismissed.png"
