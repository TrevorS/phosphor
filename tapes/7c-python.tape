# Screen 7c in python — the third of the three languages `CP-4`'s VHS list
# asks for. `7c-rust.tape` carries the writeup; `7c-typescript.tape` carries
# the note about `.` as a trigger character and about what a fixture server
# can and cannot prove for a language nothing automated has attached to. Only
# what differs is repeated here.
#
# **One `k` fewer.** The navigation walks back from the empty line a trailing
# newline leaves to the call the typed line goes above, and python's function
# body ends with that call rather than with a closing brace after it — so this
# tape presses `G k O` where its two siblings press `G k k O`. The fixture's
# own header says the same thing at the other end of the coupling.
#
# **The comment prefix on screen is `#`, and it came from the declaration.**
# `lsp-fixture.sh` swaps only `lsp_command`, so `runtime/languages/python.scm`
# still supplies the grammar and the prefix — which is `T037`'s claim (a
# language is data, not a Rust table) drawn rather than asserted.

Output "artifacts/7c-python.gif"

Require phosphor
Require python3

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "bash lsp-fixture.sh 7c-python python completion fetch.py && cd /tmp/phosphor-tape-7c-python && PHOSPHOR_RUNTIME=runtime phosphor --theme phosphor-dark fetch.py"
Enter
Show
Wait+Screen@10s /toy-lsp/

Type "G"
Type "k"
Type "O"
Type "policy = RetryPolicy.de"
Wait+Screen@10s /default_delay/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/7c-python.png"
