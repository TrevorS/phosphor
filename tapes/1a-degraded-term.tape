# V009 — degradation tape, `TERM=xterm-256color` variant of `1a`.
#
# Same file, same theme, same width as `1a.tape` — the one difference is
# `TERM=xterm-256color` prefixed onto the typed command. That mirrors the
# inline-env-var pattern the three `_undercurl-check-*.tape` files already
# use (`Type "PHOSPHOR_UNDERCURL=0 undercurl"`), not VHS's own `Env`
# directive — chosen for consistency with a mechanism already proven in
# this library rather than a second, unverified one for the same job.
#
# STATUS, checked against the tree this window and confirmed by running it
# twice (not asserted): pixel-identical to `1a.png` both times — `compare
# -metric AE -fuzz 0.6%` reports 0 against the committed `1a` reference.
# That is expected, not a harness bug: `1a`'s slice has no gutter markers
# yet (`T031`/`gutter.rs` is an empty stub this window — read this session),
# nothing in `crates/phosphor/src/main.rs` reads `TERM` at all (grepped this
# session; the only `TERM`/`UnderlineCapability::resolve` call site anywhere
# in the tree is `crates/phosphor-buffer`, which nothing on this screen
# invokes), and there is no undercurled span or session-state spinner on
# this slice yet (`T040`/`T068`/`T051`, S4/S6/S7, unbuilt). Contrast
# `1a-degraded-nocolor.tape`, same slice, same session: that one is NOT AE
# 0 — a real, diagnosed effect one dependency layer down. `TERM` genuinely
# has nothing to reach here today; `NO_COLOR` does. Don't assume they behave
# the same way just because both are named together in V009's line.
#
# Committed anyway, deliberately: it is the literal `1a` half of V009's
# acceptance line, captured "without touching a real terminal" as asked,
# and it is a tripwire — the day `T031`, `T040` or `T051` change what this
# screen draws, this tape's `tapes/diff-tapes.sh 1a-degraded-term` result
# should stop being a silent match. See `tapes/README.md`'s "V009" section
# for the fuller writeup.

Output "artifacts/1a-degraded-term.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime TERM=xterm-256color phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/
# Same capture-pipeline settle guard `1a.tape` documents in full — not
# repeated per tape, see that file.
Sleep 500ms
Screenshot "artifacts/1a-degraded-term.png"
