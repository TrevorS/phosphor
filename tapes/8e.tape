# Screen 8e — the once-per-session unknown-key hint. CP-3's VHS list names
# this last: "the once-per-session unknown-key hint firing and then *not*
# firing again (8e)".
#
# **Verified wired before capturing, not assumed:**
#   - `crates/phosphor-ui/src/unknown_key.rs:71` (`LEAD`) and `:76`
#     (`TAUGHT`) are the exact strip text: `"unknown key "` + the pressed key
#     + `" — SPC opens the keymap · :help agent-objects · shown once"`.
#   - `crates/phosphor/src/main.rs:875` constructs the session-owned
#     `UnknownKeyHint` latch; `:2009` is the `Action::App(AppAction::
#     ShowUnknownKeyHint { key })` arm; `:1332` composes it as a real
#     `Node::VirtualText` via `unknown_key::strip` — all read this session
#     (docs/TASKS.md's own citation for the third, `main.rs:1293`, has
#     drifted off the tree — current line is 1332).
#   - `crates/phosphor/tests/loop_pty.rs:521`
#     `driven::an_unbound_key_teaches_once_and_never_again` presses `Q`
#     (unbound in the shipped table) on a real pty, asserts `unknown key` and
#     `shown once` land on the frame, presses `Q` again then `<esc>`, and
#     asserts neither string appears in anything drawn since — green this
#     session (`cargo nextest run -p phosphor --test loop_pty driven::`)
#     before this tape was written. This tape scripts the identical order
#     (`Q`, `Q`, `Escape`) for the same reason.
#
# Ruling (Teej, 2026-08-13, docs/TASKS.md): T035's once-per-session design is
# unchanged — this tape captures that design working, not a redesign of it.
#
# `Q` is the same unbound key the pty test presses — nobody's in
# `runtime/keymaps.scm` (grepped clean for a bare `"Q"` binding), so this
# tape and the test are exercising the identical fault.
#
# Two screenshots off one session: right after the first `Q` (hint visible,
# names `SPC` and `:help agent-objects`, says `shown once`), and right after
# a second `Q` (hint gone — the latch held).
#
# **A second, deliberate V004 deviation, flagged like the first.** Every other
# `Wait+Screen` in this tape (and the library) waits for text that newly
# *appears*. This one can't: the second `Q`'s whole point is that nothing new
# appears — `main.rs`'s loop clears `hint` unconditionally at the top of every
# key event (`:1046-1047`, "the row lives until the next keystroke
# acknowledges it") and then `taught.teach(&key)` (`:1147`) answers `None`
# because the latch is already spent, so the frame after the second `Q` has
# no positive regex to wait on — there is no text this tape could assert
# *appeared* to prove the negative. `Sleep 500ms` here (no preceding
# `Wait+Screen`) is doing double duty — settle guard *and* the only
# synchronisation available — rather than the settle-only role it plays
# everywhere else in this tape and library. Flagged rather than silently
# widened, same as `1a.tape`'s original deviation.

Output "artifacts/8e.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/

Type "Q"
Wait+Screen@10s /shown once/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/8e-taught.png"

Type "Q"
Escape
Sleep 800ms
Screenshot "artifacts/8e-silent.png"
