#!/usr/bin/env bash
# A shell script that runs `phosphor` must survive it refusing, and must not
# lie about what it said.
#
# Both checks below exist because `scripts/seed-fixtures.sh` did each of them,
# and neither was found by a gate: that script is deliberately outside the
# `scripts/lint-*.sh` glob `just lint` walks — its own header argues why, and
# the argument is right — so nothing ever ran it. `T041` was the first task with
# a reason to, and found it had been dead since `T100`.
#
#   1. **`out="$(…)"` then `code=$?` under `set -e`.** A refusal exits non-zero
#      — `T100`'s ruling, *"one door, one refusal, one exit code"* — so the
#      assignment itself fails and `set -e` kills the script *before* the line
#      that reads `$?`. The code that was written to handle a refusal is
#      unreachable by construction. `seed-fixtures.sh` aborted on the first line
#      of its own plan and printed nothing; its header describes a per-line
#      transcript it could not produce. The fix is an `if` condition, which is
#      the one context where `set -e` is suspended.
#
#   2. **A hard-coded answer shape the door does not emit.** The same script
#      matched `(#refused "not built yet — …")`, a bare list, which is what
#      `--eval` printed before the door had a voice. After `T100` every line
#      classified as BROKEN and the summary blamed the *plan* — a script
#      confidently reporting a fault in the wrong file. The sigils belong to
#      `phosphor_core::action::{Outcome, Refusal}` and `phosphor-steel`'s
#      renderers; a script that spells one must spell one that exists.
#
# Scope is `scripts/*.sh` **excluding this glob's own lints**, because a lint
# does not call the door. That is not a loophole: a lint that shelled out to
# `phosphor` would be a gate that needs the binary built, which is a different
# argument and one nobody has made.
#
# Deliberately NOT checked: that the scripts *run*. Running `seed-fixtures.sh`
# needs `phosphor` on `$PATH` and takes a process per plan line; that is a
# by-hand command and its own header says so.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

violations=0

# The scripts that may talk to the door: everything in scripts/ that is not one
# of this glob's lints. Derived rather than listed — a new script is covered the
# day it lands.
callers=()
for script in scripts/*.sh; do
    case "$script" in
        scripts/lint-*.sh) continue ;;
    esac
    callers+=("$script")
done

if [ "${#callers[@]}" -eq 0 ]; then
    echo "lint-door-callers: no non-lint scripts in scripts/ — nothing to check"
    exit 0
fi

# ---------------------------------------------------------------------------
# 1. The exit code a `set -e` script cannot read
# ---------------------------------------------------------------------------

for script in "${callers[@]}"; do
    grep -q '^set -[a-z]*e' "$script" || continue

    # An assignment from a command substitution, on its own line, immediately
    # followed by a line capturing `$?`. Both halves are required: the pattern
    # is only a bug when something *wanted* the exit code, and an assignment
    # nobody reads `$?` after is an ordinary way to capture output.
    while IFS= read -r hit; do
        line="${hit%%:*}"
        next=$((line + 1))
        after="$(sed -n "${next}p" "$script" || true)"
        if printf '%s' "$after" | grep -Eq '^[[:space:]]*[A-Za-z_][A-Za-z_0-9]*=\$\?[[:space:]]*$'; then
            echo "$script:$line — \`\$?\` after a command substitution, under \`set -e\`."
            echo "  The assignment is what fails, so \`set -e\` exits before line $next runs."
            echo "  A refusal exits non-zero (T100), so this is unreachable on exactly the"
            echo "  answers it was written to handle. Use an \`if\` condition:"
            echo "      if out=\"\$( … )\"; then code=0; else code=\$?; fi"
            violations=$((violations + 1))
        fi
    done < <(grep -nE '^[[:space:]]*[A-Za-z_][A-Za-z_0-9]*=\"?\$\(' "$script" || true)
done

# ---------------------------------------------------------------------------
# 2. An answer shape the door does not emit
# ---------------------------------------------------------------------------

# Where the sigils are spelled in Rust. Read rather than assumed: `Outcome` and
# `Refusal` render through these, and a script quoting a shape not in them is
# quoting a shape from some other build.
voice=$(cat crates/phosphor-core/src/action.rs crates/phosphor-steel/src/answer.rs 2>/dev/null || true)

if [ -z "$voice" ]; then
    echo "lint-door-callers: cannot read the door's renderers — has a file moved?"
    exit 1
fi

for script in "${callers[@]}"; do
    # **Comment lines are skipped, and that is the point rather than a hole.**
    # What this checks is a *matcher* — the shape a `case` or a `grep` compares
    # against — and a header explaining which shape the door used to print is a
    # correct, useful thing to write. `seed-fixtures.sh`'s own header now
    # quotes the pre-`T100` list form to say why its classifier was wrong; a
    # lint that failed on that would be punishing the record of the bug.
    while IFS= read -r hit; do
        line="${hit%%:*}"
        text="$(sed -n "${line}p" "$script" || true)"
        case "$(printf '%s' "$text" | sed 's/^[[:space:]]*//')" in
            '#'*) continue ;;
        esac
        # The bare-list form. `T100` replaced it with `#refused · `; a script
        # still matching it is reading a build from before that ruling.
        echo "$script:$line — matches the pre-T100 list form \`(#refused …)\`."
        echo "  The door prints \`#refused · <sentence>\` now, so a script comparing against"
        echo "  the old shape classifies every real answer as unrecognised — which is how"
        echo "  seed-fixtures.sh came to report that the *plan* had drifted from the registry."
        violations=$((violations + 1))
    done < <(grep -nE '\(#(ok|refused|raised)' "$script" || true)
done

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-door-callers: FAILED — $violations issue(s)."
    exit 1
fi

echo "lint-door-callers: clean — ${#callers[@]} door-calling script(s), exit codes readable, answer shapes current"
