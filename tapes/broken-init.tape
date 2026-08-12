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

Hide
Type "rm -rf /tmp/phosphor-broken-init && cp -R ../runtime /tmp/phosphor-broken-init && printf '\n(define broken (+ 1 nonesuch))\n' >> /tmp/phosphor-broken-init/init.scm && PHOSPHOR_RUNTIME=/tmp/phosphor-broken-init phosphor --theme phosphor-dark ../crates/phosphor-core/src/lib.rs"
Enter
Show
Wait+Screen@10s /init.scm/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/broken-init.png"
