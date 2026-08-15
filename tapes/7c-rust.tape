# Screen 7c — "lsp completion + signature help" (TUI Mockups.dc.html), in rust.
# `CP-4`'s VHS list names this first: "the completion float opening over real
# code in all three languages (7c)". This is the rust one, and it carries the
# writeup its four `S4` siblings point back to — `7c-typescript.tape`,
# `7c-python.tape`, `signature-help.tape` and `diagnostics.tape`.
#
# **The server is the fixture, not rust-analyzer, and that is the finding.**
# A Tier-2 reference is a pixel comparison, so a tape is only worth capturing
# if it draws the same frame twice. rust-analyzer decides for itself what to
# offer and when — the list depends on how far indexing has got, which depends
# on the machine — so a tape racing it produces a different reference every
# run, which is the flake `V006` exists to prevent for agent surfaces and
# exactly the red build `docs/TEAM.md` says teaches a team to ignore the
# harness. `crates/phosphor/tests/fixtures/toy_language_server.py` speaks the
# same protocol over the same transport and answers `7c`'s own three labels,
# its detail column and its one row of prose, in constants. What it costs is
# one visible thing: the statusline chip reads `toy-lsp ✓` where the mockup
# draws `rust-analyzer ✓`, because the chip draws the name a server gives
# itself. Left visible rather than disguised — a capture that hid which server
# answered would be the dishonest version of this.
#
# **What is NOT the fixture.** `tapes/lsp-fixture.sh` swaps exactly one line
# of the copied `runtime/languages/rust.scm` — `lsp_command` — so the grammar,
# the extensions and the comment prefix are still whatever `runtime/` declares
# today, and every glyph on this frame is real tree-sitter-rust highlighting
# of a real `.rs` buffer. That is what makes "over real code" true rather than
# claimed. See that script's header for the rest, including why the scratch
# `$PHOSPHOR_RUNTIME` is not optional.
#
# **Verified wired before capturing, not assumed** — this window owns
# `tapes/**` only, so the check below is reading and running, not editing:
#   - `crates/phosphor/src/main.rs`'s insert-mode trigger (the block above
#     `editing.lookup = Some(Lookup::Completion)`) fires off the edit stream,
#     gated on `editing.prefix_len() >= completion_floor(&host)` — default
#     `COMPLETION_MIN_CHARS_DEFAULT`, which is 2 — or on the server's own
#     `completionProvider.triggerCharacters`. So the `de` typed below is what
#     asks; no completion key is pressed on this tape.
#   - `crates/phosphor/tests/loop_pty.rs`'s
#     `typing_in_insert_mode_raises_the_completion_float` presses the same one
#     character against the same fixture server on a real pty and reads
#     `default_delay`, `fn() -> RetryPolicy` and `3 attempts` off the frame.
#     Green this session before this tape was written.
#
# **The readiness sentinel is the chip, and it has to be.** The trigger is
# edge-triggered on the edit and requires `servers.state(language).is_ready()`
# — a character typed one frame early is dropped, and the float never opens,
# and this tape would fail as a `Wait+Screen` timeout on wiring that works.
# `toy-lsp ✓` is the server's own `serverInfo.name` out of its `initialize`
# reply, so waiting on it is "the server is up" stated by the editor rather
# than guessed at by a `Sleep` — the same reasoning `loop_pty.rs`'s `ready()`
# is written on, and the V004 rule this library already runs by. It also
# subsumes the usual mode-chip sentinel: nothing draws `toy-lsp` until the
# statusline has painted.
#
# **The navigation counts lines, and the fixture's tail says so.** `G` lands
# on the empty line a trailing newline leaves behind (confirmed on a real
# capture, not assumed), `k k` walks back to the `join_all` call, and `O`
# opens above it — so `7c`'s missing line is typed where a person would
# actually put it, before the call that uses the policy. `O` copies the
# current line's indentation, so nothing below types a leading space; typing
# one would have produced eight columns of indent, which is how that was
# found.

Output "artifacts/7c-rust.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh 7c-rust rust completion fetch.rs && cd /tmp/phosphor-tape-7c-rust && PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark fetch.rs"
Enter
Show
Wait+Screen@10s /toy-lsp/

Type "G"
Type "k"
Type "k"
Type "O"
Type "let policy = RetryPolicy::de"
Wait+Screen@10s /default_delay/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/7c-rust.png"
