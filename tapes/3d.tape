# Screen 3d — "The file picker" (TUI Mockups.dc.html), one of the five `CP-5`
# names. Its caption is the whole specification: *"the file picker carries
# agent state: unseen counts + activity, **not just names**"*.
#
# **That caption is why this capture matters and why the picker was broken.**
# The `files` source listed only paths the store had regions for, so an
# ordinary session opened an empty picker under a key labelled "files" — the
# store is the annotation, never the filter. Reported by Teej at a real
# terminal, fixed, and this is the screen that would have shown it.
#
# Seeded, so the annotation has something to say: see `tapes/seed-state.sh`
# and `2a.tape`'s header for the store the launch below reads.
#
# SENTINEL. `fetch.rs` — a row in the float and nowhere else on the screen,
# because the statusline draws the *open* file (`src/retry.rs`). It says the
# workspace was walked, which is the half that was broken.
# Not a `/`-bearing pattern: see `2a.tape` on why `Wait+Screen` cannot take one.

Output "artifacts/3d.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "cd ../fixtures && PHOSPHOR_RUNTIME=../runtime phosphor src/retry.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

# `SPC f` — `3c`'s `files`, the shipped binding.
Type " f"
Wait+Screen@10s /fetch\.rs/

Sleep 500ms
Screenshot "artifacts/3d.png"
