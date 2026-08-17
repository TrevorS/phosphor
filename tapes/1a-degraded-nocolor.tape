# V009 — degradation tape, `NO_COLOR=1` variant of `1a`.
#
# Same file, same theme, same width as `1a.tape` — the one difference is
# `NO_COLOR=1` prefixed onto the typed command, the same inline-env-var
# pattern `_undercurl-check-forced-underline.tape` already uses. See
# `1a-degraded-term.tape`'s header for why inline-prefix rather than VHS's
# `Env` directive.
#
# STATUS: real, measured difference from `1a.png` — 35360.5 px (`compare
# -metric AE -fuzz 0.6%`), reproduced against a same-session, same-content
# rerun (a `1a`-vs-`1a` control captured back to back with this tape scored
# AE 0, which rules out capture-to-capture antialiasing jitter as the cause).
#
# NOT what a first grep suggests. `crates/phosphor/src` and
# `crates/phosphor-ui/src` have no `NO_COLOR` call site at all — but
# `crossterm` 0.29 (the pinned version, `T002`) does:
# `crossterm-0.29.0/src/style.rs` implements NO_COLOR (no-color.org) itself,
# stripping `SetForegroundColor`/`SetBackgroundColor` unless the app calls
# `force_color_output(true)` — checked this session, and nothing in
# `crates/` calls it. So every explicitly-coloured span (the syntax map,
# this file's `//!` comment colour among them) falls back to the terminal's
# plain default foreground the moment `NO_COLOR` is set, one layer below any
# code phosphor owns. Sampled pixels confirm the mechanism, not just the
# aggregate count: e.g. `(35,42,38)` in `1a.png` (a comment glyph, dimmed by
# its syntax colour) reads `(93,100,95)` here — brighter and less saturated,
# consistent with losing an explicit dim colour and inheriting the pinned
# terminal default (`neutrals.text` `#c6cec6`) instead.
#
# So `NO_COLOR` **is** a live degradation path today — just not through any
# of the three named in V009's acceptance line (▎ marker, undercurl,
# spinner), none of which this slice renders yet (see `1a-degraded-term.tape`
# for why). It degrades general syntax colour instead, for free, at the
# dependency layer. Flagged as a finding in the harness report, not folded
# in silently — first instinct (grep found nothing, assume no effect) was
# wrong, and only running it caught that.
#
# Committed as the literal `1a` half of V009's acceptance line, and as a
# tripwire the opposite way from `1a-degraded-term.tape`: if this one *ever*
# starts reading AE 0 against `1a.png`, something changed how colour is
# emitted (e.g. a `force_color_output(true)` call landing somewhere) and
# that is itself worth knowing.

Output "artifacts/1a-degraded-nocolor.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime NO_COLOR=1 phosphor fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Same capture-pipeline settle guard `1a.tape` documents in full — not
# repeated per tape, see that file.
Sleep 500ms
Screenshot "artifacts/1a-degraded-nocolor.png"
