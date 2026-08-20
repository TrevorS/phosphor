# `T021`'s acceptance criterion, as a Tier-2 capture: "a syntax error in
# init.scm yields a working editor with a legible error float." CP-2 names
# this explicitly — "the broken-init.scm boot with its error float" — as one
# of its three required VHS artifacts.
#
# Mirrors `crates/phosphor-steel/tests/broken_init.rs`'s
# `the_error_float_carries_the_file_the_line_and_the_message`: same planted
# mistake (`(define broken (+ 1 nonesuch))`, a free identifier), so this
# capture and that test are checking the same fault by two different tiers.
#
# **Runs against a scratch $PHOSPHOR_RUNTIME, never the tracked runtime/.**
# harness does not own `runtime/**` (TEAM.md), and `phosphor` boots against
# whatever `$PHOSPHOR_RUNTIME` names (`crates/phosphor-steel/src/runtime.rs`,
# `Runtime::root`) — the setup line below copies the shipped tree into `/tmp`
# and plants the mistake there. Discovered empirically (`_repl-check`-style
# investigation, not guessed): confirmed the boot float renders
# `◆ steel · boot`, `1 fault`, `init.scm:<line>:<col> · free identifier`, the
# offending source line, a forms-ran count, `the editor is up`, and a footer
# offering `:repl` / `:reload-runtime` / `esc close` — all on one real
# captured frame, with the dimmed buffer still visible behind it (§9).
#
# Sentinel: waits on `init.scm`, which only appears once the boot float has
# actually painted (the dimmed buffer behind it never contains that string —
# it's `phosphor-core/src/lib.rs`).

Output "artifacts/broken-init.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

# **The layer is frozen, and that is what makes this reference reproducible.**
#
# It copied `../runtime` — the live layer — and the screen it captures draws two
# numbers the layer's *size* decides: `init.scm:<line>` is where the appended
# form lands, and `N of M forms ran` is the whole boot layer's count. So any
# Scheme form anybody added, anywhere, moved this reference. It read as
# flakiness (`97.5`, `388`, `92.5` px across three runs, matching whenever run
# alone) and was nothing of the kind: three different numbers because the layer
# was different each time. `OPEN-QUESTIONS.md` §42 has the diff that showed it.
#
# `fixtures/runtime/` is a verbatim snapshot of `runtime/` and `harness` owns
# it, which is the point: this tape copies to a scratch in the first place
# because `harness` does not own `runtime/**` (see this file's own note below),
# and a fixture under `tapes/` is the version of that rule which also holds
# still.
#
# **Divergence from the live layer is expected and harmless here.** The claim
# this screen makes is *"a bad form in `init.scm` produces this surface"*, and a
# frozen layer states it exactly as well as a live one. What a frozen layer also
# does is stay true. If it ever stops booting, this tape's sentinel does not
# appear and the capture fails loudly rather than drifting.
#
# `6b.tape` and `repl-liveness.tape` still copy `../runtime` and are left alone:
# neither draws a count, so neither moves when the layer does.
Hide
Type "rm -rf /tmp/phosphor-broken-init && cp -R fixtures/runtime /tmp/phosphor-broken-init && printf '\n(define broken (+ 1 nonesuch))\n' >> /tmp/phosphor-broken-init/init.scm && PHOSPHOR_RUNTIME=/tmp/phosphor-broken-init phosphor --theme phosphor-dark fixtures/core-lib.rs"
Enter
Show
Wait+Screen@10s /init.scm/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/broken-init.png"
