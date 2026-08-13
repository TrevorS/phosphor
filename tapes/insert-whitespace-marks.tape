# `T016`'s insert-only trailing-whitespace marks, as a Tier-2 capture.
#
# `CP-3`'s VHS list names this on its own line, separate from the `8e`
# snapshot (which already covers it as a single static Tier-1 grid dump,
# `crates/phosphor/tests/screen_8e.rs`) — what a still cannot show is the
# *insert-only* half of the claim: that the same trailing whitespace draws no
# mark in NORMAL and a `··` mark the instant the buffer enters INSERT. So this
# tape captures both frames off one real session: `phosphor` opened, waited
# for the NORMAL sentinel, screenshotted; then `i`, waited for the INSERT
# sentinel, screenshotted again. Same file, same line, same cursor — the only
# variable between the two frames is the mode, which is `soft_wrap.rs`'s own
# claim (`soft_wrap::set_mode`, driven off `machine.mode()` every frame,
# `crates/phosphor/src/main.rs:825-832`).
#
# **No leader popup, fold toggle, or unknown-key hint tape exists alongside
# this one.** Checked against the tree this session, not assumed — see this
# window's report for the file:line evidence on each:
#   - the leader popup (`3c`) has a widget (`phosphor-ui/src/key_hints.rs`)
#     and a Tier-1 test that hand-builds its own `Tree`
#     (`crates/phosphor/tests/screen_3c.rs`), but `crates/phosphor/src/main.rs`
#     never composes a `Node::KeyHints` slot on `SPC`-pending — grepped clean
#     for `KeyHints`/`Density::Grid`, and `Surface`/`Intent` (`main.rs:2040`,
#     `main.rs:227`) have no leader-popup variant. Confirmed empirically too:
#     a real capture of `phosphor` before and after `Space` diffed at 0 px
#     (ImageMagick `compare -metric AE`, exact match).
#   - folds have an `Action` (`SetFold`/`FoldAll`/`UnfoldAll`,
#     `crates/phosphor-core/src/action.rs:414-424`, doc'd "za") but no `za`
#     binding exists anywhere in `runtime/keymaps.scm` (grepped clean for
#     `"z`), and `main.rs`'s `Editing::act` has no match arm for any `ViewAction`
#     besides `Scroll` — a fold Action falls through to the catch-all
#     `Refused(NotYetImplemented)`. A live capture of `za` shows exactly the
#     vim primitive `a` (append + enter INSERT), not a fold — `z` is silently
#     swallowed as unbound.
#   - the unknown-key hint (`T035`) has a module
#     (`phosphor-ui/src/unknown_key.rs`) and a Tier-1 test
#     (`crates/phosphor/tests/screen_8e.rs`), but `unknown_key`/`UnknownKeyHint`
#     is never referenced anywhere under `crates/phosphor/src/` — grepped
#     clean. There is no code path in the real loop that could ever call
#     `UnknownKeyHint::teach`.
# All three are real, tested widgets one wiring step away from being
# reachable by a keystroke in the shipping binary — not missing surfaces, a
# missing connection `spine` owns (`main.rs`). Flagged, not invented around.
#
# **Fixture, not `fixtures/`.** `V006`'s committed fixture tree
# (`fixtures/src/*.rs`) is fmt-clean and carries no trailing whitespace by
# construction, and this window's file lock scopes this task to `tapes/**` +
# `scripts/**` only — so the fixture line lives inline in this tape's own
# `Hide`den setup, the same pattern `broken-init.tape` uses for its planted
# `init.scm` mistake. Two trailing spaces on the marked line, matching `8e`'s
# own `MARKED_LINE` convention (`screen_8e.rs`).
#
# Sentinel (V004): `tapes/README.md`'s "Sentinel contract" section — the mode
# chip is the one piece of the statusline never shed to nothing. Two waits
# here, one per mode word, so the second screenshot cannot race ahead of the
# `i` keystroke actually landing.

Output "artifacts/insert-whitespace-marks.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "printf 'fn total(items: &[u64]) -> u64 {\n    let mut sum = 0;  \n    for item in items {\n        sum += item;\n    }\n    sum\n}\n' > /tmp/phosphor-whitespace-fixture.rs && PHOSPHOR_RUNTIME=../runtime phosphor /tmp/phosphor-whitespace-fixture.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/insert-whitespace-marks-normal.png"

Type "i"
Wait+Screen@10s /(?m)^ (INSERT|I)\b/
Sleep 500ms
Screenshot "artifacts/insert-whitespace-marks-insert.png"
