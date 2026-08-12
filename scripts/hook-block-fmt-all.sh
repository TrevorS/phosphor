#!/usr/bin/env bash
# PreToolUse/Bash hook — refuse `cargo fmt --all`.
#
# `--all` does not mean "workspace members". cargo-fmt resolves it over every
# local package under the workspace root, so it recurses through the two
# vendored path dependencies and fails on upstream code that we did not write
# and must not reformat. The only way to make `--all` pass would be to reformat
# both forks, which turns every future `just vendor-pull` into a whole-file
# conflict and permanently breaks `just vendor-diff` — the one command that
# keeps a fork from silently becoming a rewrite.
#
# `just fmt` omits `--all` deliberately. Coverage of our own code is identical:
# `[workspace] exclude` stops at the vendor seam, and a formatting violation
# planted in all seven crate roots is caught either way.
#
# This is a hook rather than a line in CLAUDE.md because the tempting repair —
# "CI is red on vendored code, let me just format it" — is exactly the wrong
# one, and a written rule does not stop it. It has already been made once and
# reverted at the CP-1 gate.
#
# Reads the PreToolUse payload on stdin and denies by printing the documented
# permissionDecision JSON. Anything else exits 0 silently and the command runs.
#
# Not named `lint-*.sh`: `just lint` globs that prefix, and this is a hook, not
# a lint.

set -euo pipefail

cmd="$(jq -r '.tool_input.command // ""')"

# Match a `cargo fmt` invocation carrying `--all`, allowing a `+toolchain`
# between them. `[^;&|]*` keeps the match inside a single command, so an
# unrelated `--all` after a separator — `cargo fmt --check && cargo clippy
# --all` — is not a false positive.
if printf '%s' "$cmd" | grep -qE 'cargo([[:space:]]+\+[^[:space:]]+)?[[:space:]]+fmt[^;&|]*[[:space:]]--all([[:space:]]|$)'; then
    jq -nc '{
        hookSpecificOutput: {
            hookEventName: "PreToolUse",
            permissionDecision: "deny",
            permissionDecisionReason: "`cargo fmt --all` recurses through the path dependencies into vendor/ratatui-code-editor and vendor/ratatui-markdown and fails on upstream code. Greening it would mean reformatting both forks, which permanently breaks `just vendor-diff`. Use `just fmt` — it omits --all deliberately and covers all seven of our crates."
        }
    }'
    exit 0
fi

exit 0
