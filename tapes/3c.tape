# Screen 3c — "the SPC leader popup" (TUI Mockups.dc.html). CP-3's VHS list
# names this first: "the leader popup opening (3c)".
#
# **Verified wired before capturing, not assumed** — this window's brief
# ("harness") owns tapes/** only, so the check below is reading, not editing:
#   - `crates/phosphor/src/main.rs:1411` `fn under(layer, machine)` reads the
#     live keymap table for the pending prefix; `:1318`/`:1339-1340` compose
#     it as a real `Node::KeyHints { density: Density::Grid, .. }` strip off
#     the bottom of the body — confirmed by reading the file this session,
#     not by trusting docs/TASKS.md's citation (which names :1359/:1300 for
#     the same two things — drifted since that prose was written; the line
#     numbers above are current against this tree).
#   - `crates/phosphor/tests/loop_pty.rs:451`
#     `driven::pressing_space_opens_the_leader_popup` presses a real ` ` on a
#     pty and asserts `SPC` and `+claude` land on the frame, then `<esc>`
#     clears it — `cargo nextest run -p phosphor --test loop_pty driven::` was
#     green for this test (and its sibling `a_repl_rebind_reaches_the_leader_
#     popup`, :494) this session before this tape was written.
# So this is a real keystroke reaching a real widget in the shipping binary,
# not a hand-built `Tree` — `crates/phosphor/tests/screen_3c.rs` already
# proves the widget; this proves the wiring.
#
# `+claude` (`runtime/keymaps.scm:740`, `(key/group)` under `SPC c`) is the
# sentinel: specific enough that nothing else on a freshly opened buffer could
# produce it, and it's exactly what the pty test itself asserts on.
#
# **Runs against the tracked `../runtime`, uncopied** — same reasoning as
# `1a.tape`: this session never types a `:` command or anything that reaches
# `persist-form!`, only ` ` (open the popup) and `<esc>` (close it), so there
# is nothing here that could write to `persisted.scm`.
#
# Sentinel (V004): tapes/README.md's "Sentinel contract" section for the mode
# chip; `+claude` on top of it for the popup itself, same convention `6b.tape`
# uses (a word unique to the thing being proved, not a generic prompt glyph).
#
# **Finding, not planned, and not folded in — the "closed" frame is real, not
# a capture mistake.** `<esc>` while `SPC` is pending does not take a
# dedicated cancel path: every key while a sequence is pending goes through
# the same lookup (`crates/phosphor-core/src/input.rs:376-378`, `self.
# pending.keys.push(pressed)` then `keymap.resolve(scope, &keys)` on the
# *whole* accumulated sequence), and `"SPC <esc>"` has no leaf in
# `runtime/keymaps.scm`, so it resolves `Unbound` (`:407`) — which both
# closes the popup (`:417`, `self.cancel(out)`) **and** spends the
# once-per-session unknown-key hint (`:414`,
# `AppAction::ShowUnknownKeyHint`) in the same turn. The second screenshot
# below shows both at once: the popup gone, and `unknown key <space><esc>` on
# the hint row. Confirmed by reading `input.rs` this session, not inferred
# from the pixels — the pty test this tape otherwise mirrors
# (`pressing_space_opens_the_leader_popup`) only asserts `+claude` is gone
# after `<esc>`, so it does not see this. Reported, not fixed: `input.rs`
# is outside this window's file lock (`tapes/**` only).

Output "artifacts/3c.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "PHOSPHOR_RUNTIME=../runtime phosphor ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|INSERT|VISUAL|PAUSED|N|I|V|P)\b/

Space
Wait+Screen@10s /\+claude/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/3c-open.png"

Escape
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
Sleep 500ms
Screenshot "artifacts/3c-closed.png"
