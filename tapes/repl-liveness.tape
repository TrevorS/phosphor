# CP-2's hardest, load-bearing capture: "a clip of a REPL rebind taking
# effect on the very next frame — the liveness claim is about motion between
# two states, so a still can't carry it."
#
# What "liveness" means here, precisely (`T022`'s acceptance criterion, and
# `crates/phosphor/src/main.rs`'s own `T022` doc section): a binding created
# at the REPL is live on the very next keystroke, with **no restart** —
# because `crates/phosphor/src/main.rs` caches no keymap at all and asks the
# live VM (`runtime/keymaps.scm`) what every key means, every time.
#
# The sequence this tape scripts, and why each step earns its place:
#
#   1. a plain buffer, NORMAL mode — the "before" state, and proof this is
#      one continuous process, not two recordings spliced together.
#   2. `:repl` opens the REPL — `runtime/keymaps.scm`'s `(ex-set! "repl" …)`,
#      which is `6b`'s own spelling for it. **Not a bare `:`**: this tape said
#      `:` opened the REPL and typed exactly that, which was true only while
#      there was no ex prompt. `:` binds to `open-prompt kind ex` today, so
#      the wait for a REPL chip could never be satisfied and every frame here
#      was unproducible — and `run-tapes.sh`'s `set -e` made that one broken
#      tape stop the whole library.
#   3. `(keymap-set! "gz" (lambda () (open-repl!)))`, submitted — binds a key
#      that was UNBOUND a moment ago, in the same live session.
#   4. `esc` closes the REPL — back to the plain buffer, NORMAL mode again.
#      This is the actual "before" frame for the liveness claim: nothing on
#      screen suggests a REPL exists, and `gz` has never been pressed.
#   5. `gz` — typed as two ordinary keystrokes into the *buffer* (not the
#      REPL; `runtime/keymaps.scm`'s dispatcher only runs on the buffer
#      surface, confirmed empirically, see the note below) — and the REPL
#      reopens, still holding the `keymap-set!` line from step 3 in its
#      history. **That last part is the proof of no restart**: a fresh boot
#      would show an empty session, not one that remembers what step 3 typed.
#
# Nothing between steps 1 and 5 is `Hide`d — the whole motion is one
# unbroken recording, which is the only way a clip (not a pair of stills)
# proves "no restart happened in between."
#
# Found empirically, not guessed, and worth recording because both are easy
# to get backwards from the mockup's own footer text:
#
#   * `esc` closes the REPL — **not `q`**. `6b`'s footer draws "q close", but
#     the REPL's body is a text input today (no REPL-local modes until
#     `T026`), so `q` while the REPL has focus is a character being typed,
#     not a close key. `main.rs`'s own `repl_key` doc-comment says so; this
#     tape is what confirmed it renders that way rather than assuming the
#     footer is executable.
#   * the live keymap (`runtime/keymaps.scm`'s dispatcher) is consulted only
#     when the *buffer* has the frame — while the REPL is open, every key is
#     text for its input line (confirmed by an earlier version of this tape
#     that typed `gz` *inside* the open REPL and watched it land as literal
#     characters in the prompt, not as a keypress dispatch). So the rebind
#     has to be exercised from the buffer, after `esc`, which is exactly
#     what `T022`'s claim is about: a binding is live for the *editor*, not
#     only for the surface that defined it.
#
# **Runs against a scratch $PHOSPHOR_RUNTIME**, not the tracked runtime/ —
# `keymap-set!` is a `repl.scm` persistent head, so a run against the real
# tree would write to the checked-in `persisted.scm`. harness does not own
# `runtime/**` (TEAM.md).
#
# The GIF is the reviewed artifact here, not a free byproduct (V005's usual
# framing) — the motion between "no REPL, `gz` unbound" and "REPL open,
# session intact" is the entire claim under test.

Output "artifacts/repl-liveness.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "rm -rf /tmp/phosphor-repl-liveness && cp -R ../runtime /tmp/phosphor-repl-liveness && PHOSPHOR_RUNTIME=/tmp/phosphor-repl-liveness phosphor --theme phosphor-dark fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
# Settle guard, not a sync primitive — see 1a.tape's header for the finding.
Sleep 500ms
Screenshot "artifacts/repl-liveness-1-before.png"

Type ":repl"
Enter
Wait+Screen@10s /(?m)^ (REPL|R)\b/

Type `(keymap-set! "gz" (lambda () (open-repl!)))`
Enter
Wait+Screen@10s /persisted|not persisted/
Sleep 500ms
Screenshot "artifacts/repl-liveness-2-bound.png"

Escape
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
Sleep 500ms
Screenshot "artifacts/repl-liveness-3-back-to-buffer.png"

Type "gz"
Wait+Screen@10s /(?m)^ (REPL|R)\b/
Sleep 500ms
Screenshot "artifacts/repl-liveness-4-live-on-next-key.png"
