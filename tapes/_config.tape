# V003 — shared tape configuration, `Source`d by every real tape in the
# library. This file exists to remove every source of nondeterminism that
# would make a pixel comparison meaningless (docs/TASKS.md, V003's brief) —
# it is deliberately NOT the place for anything that varies per screen
# (`Set Width`/`Set Height`, `Require`, `Output`) — those stay in each real
# tape, immediately after the `Source` line below.
#
# Usage, at the top of every `tapes/<id>.tape`:
#
#   Require phosphor
#   Source "_config.tape"      # quoted — leading `_` is a vhs parse gotcha,
#                               # see _dimensions.tape's header, gotcha #1
#   Set Width <px>              # from _dimensions.tape's table — AFTER
#   Set Height <px>              # Source, per gotcha #2: `Set` only takes
#                                 # effect before the tape's first non-`Set`
#                                 # command, and `Source` inlines these lines
#                                 # in place without closing that window, so
#                                 # a per-tape Set here still lands.
#
# This file contains ONLY `Set` lines for exactly that reason — a bare
# `Source` of it never itself ends the "still before the first command"
# window, so the per-tape Width/Height that follows still takes effect.

# ── Font (V001's pin, fixed by V002's calibration) ─────────────────────────
# Menlo ships on every Mac, zero install cost, and is what VHS's own 10-font
# fallback chain already resolves to on the reference machine — see
# tapes/README.md's "Font" section for why the *unpinned* default is exactly
# the nondeterminism this file exists to remove (a different machine with
# JetBrains Mono installed would silently get different pixels, same tape).
Set FontFamily "Menlo"
Set FontSize 16

# ── Geometry ─────────────────────────────────────────────────────────────
# Padding 0 / Margin 0 is what makes _dimensions.tape's column-width table
# exact (the rendered PNG is Width x Height px with no border to account
# for). WindowBar is deliberately left UNSET, not set to a placeholder value
# — its only accepted values are the four chrome styles (Rings, RingsRight,
# Colorful, ColorfulRight; confirmed via `vhs new`'s generated reference),
# none of which is "off", and V002 already established that leaving it
# unset adds no width, only title-bar height outside the column math.
Set Padding 0
Set Margin 0

# ── Shell ────────────────────────────────────────────────────────────────
# Pinned so the tape's setup commands (before the first `Hide`/`Show`
# boundary) get an identical prompt/init sequence regardless of the
# recording machine's login shell. Matches _dimensions.tape's own pin.
Set Shell "bash"

# ── Motion / timing determinism ─────────────────────────────────────────
# CursorBlink false: a blinking cursor makes a still `Screenshot`'s pixels
# depend on which half of the blink cycle the capture lands in — the single
# biggest risk to "byte-identical PNG on every run" of anything V004/V005
# adds on top of this file.
Set CursorBlink false
# TypingSpeed: vhs's own default (50ms) is fine, but pinned explicitly per
# V003's brief rather than left implicit — an upstream default change must
# not silently change capture pacing (same reasoning as pinning vhs's
# version itself in tapes/check-versions.sh).
Set TypingSpeed 50ms
# Framerate: matters most for GIF byte-identity (frame count/timing); inert
# for a single `Screenshot`. Not derived from any design doc — like V002's
# FontSize 16, this is a plain engineering constant, picked once and fixed.
Set Framerate 60

# ── Neutral background ───────────────────────────────────────────────────
# `Set Theme` controls the *emulated terminal's* palette — what shows before
# the app paints a cell and in any margin (there is none, Margin 0 above).
# Rather than pick one of vhs's ~250 bundled named themes (`vhs themes`),
# almost all of which are colorful and none of which is "the phosphor
# palette", this pins background/foreground/cursor to the real values this
# repo already ships in `Theme::phosphor_dark()`
# (crates/phosphor-ui/src/theme.rs — `neutrals.ground` #0c0f0c,
# `neutrals.text` #c6cec6): a neutral background sourced from the actual
# spec rather than an invented placeholder. Left partial deliberately — vhs
# merges unspecified keys (black/red/green/…) with its own default, and
# nothing downstream depends on the ANSI 16-color slots.
Set Theme {"background": "#0c0f0c", "foreground": "#c6cec6", "cursor": "#c6cec6"}
