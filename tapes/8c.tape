# Screen 8c — "Light theme — the palette holds" (TUI Mockups.dc.html):
# `Theme::phosphor_light()` (T012) over the same buffer as `9c`, proving the
# actor contract survives the lightness flip — CP-1 asks by eye whether
# claude-green is still the brightest thing on screen here.
#
# STATUS / sentinel: passes. Both blockers this header used to list are gone
# — `T090` landed the S1 host AND `--theme <slug>`, so the flag below is the
# real interface rather than this tape's assumption about one. Corrected
# during the CP-1 gate pass; `phosphor-dark|light`, `catppuccin-mocha|latte`
# and `tokyo-night|-day` are the six slugs `BUILTIN_SLUGS` ships.
#
# NOTE, found during that pass: this capture is byte-identical to
# `theme-phosphor-light.png` — same file, same width, same theme. 8c's own
# subject (the anchored region on line 18, tint + undercurl) needs a store
# the S1 host has no CLI door to, so the light-theme *palette* is all this
# still shows. The rest of 8c is covered at Tier 1 only, by
# `golden_frames__8c.snap`.

Output "artifacts/8c.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor --theme phosphor-light ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/8c.png"
