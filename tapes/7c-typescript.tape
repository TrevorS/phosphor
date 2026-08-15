# Screen 7c in typescript — the second of the three languages `CP-4`'s VHS
# list asks for. `7c-rust.tape` carries the writeup: why the server is
# `tests/fixtures/toy_language_server.py` and not tsserver, what
# `lsp-fixture.sh` does and does not swap, why the readiness sentinel is the
# statusline chip, and how the line count in the navigation below is arrived
# at. Only what differs from it is repeated here.
#
# **`.` is the trigger, and it is the server's, not ours.** The rust sibling
# reaches the floor by typing two word characters after `::`; here the float
# would open at the `.` itself — `foo.` has a prefix of *zero*, and
# `completionProvider.triggerCharacters` is the one list that knows what a
# language means by "ask now" (`crates/phosphor/src/main.rs`, the trigger
# block; `LanguageServers::completion_triggers` reads it off `initialize`).
# The fixture advertises `.` and `::`, so both paths are live on this library
# and the frame below is reached through whichever fires last. `de` is typed
# anyway, so what is captured is the narrowed list rather than the raw answer
# — the client filters, which is `T038`'s own repair.
#
# **`toy-lsp ✓` on a `.ts` buffer is the point of the sweep**, not a defect in
# it: `docs/TASKS.md`'s `CP-4` entry records that `typescript` and `python`
# declare servers *nothing automated has ever attached to*, and that a
# `rootUri` defect shipped in two of the twelve because of it. This tape
# cannot close that gap — a fixture server proves the editor's half, never the
# real server's — and saying so is worth more than a capture implying
# otherwise. What it does prove is that the `.ts` half of the path is the same
# code: same declaration shape, same attach, same float.

Output "artifacts/7c-typescript.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh 7c-typescript typescript completion fetch.ts && cd /tmp/phosphor-tape-7c-typescript && PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark fetch.ts"
Enter
Show
Wait+Screen@10s /toy-lsp/

Type "G"
Type "k"
Type "k"
Type "O"
Type "const policy = RetryPolicy.de"
Wait+Screen@10s /default_delay/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/7c-typescript.png"
