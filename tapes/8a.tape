# Screen 8a — grep results (TUI Mockups.dc.html). Its caption is *"same picker
# anatomy as unseen/files/inbox · results know who touched them"*, and the
# second half is what needs the store: a result row carries the unseen dot for
# the line it names.
#
# **Reached by tab, not by a key of its own.** `phosphor/picker-sources` is
# `("grep" "files" "unseen")` and `SPC f` opens `files`, so `<tab>` twice
# cycles files -> unseen -> grep. That is the shipped path a person has, and
# using it here means this tape also presses the source cycling `T047` built —
# rather than reaching past the keymap through `:repl` to open the source
# directly, which would capture a screen no keystroke produces.
#
# SENTINEL. `grep` — the float's own header, and a word on no other part of
# this screen. The rows are the open buffer's lines, so any text from them
# would also match the buffer underneath.

Output "artifacts/8a.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

Type " f"
Wait+Screen@10s /fetch\.rs/
Tab
Tab
Wait+Screen@10s /grep/

Sleep 500ms
Screenshot "artifacts/8a.png"
