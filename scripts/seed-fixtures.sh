#!/usr/bin/env bash
# V006 — runs fixtures/seed/plan.scm against `phosphor --eval` (T023) and
# reports exactly what each line does today.
#
# NOT a lint. Deliberately outside the scripts/lint-*.sh glob `just lint`
# walks: every line in the plan refuses today (the store is S5, review blocks
# are S6, threads are S7, watches are S8 — none built yet, per
# docs/TASKS.md), so wiring this into the gate would redden CI on a build
# that is behaving exactly as designed. harness's own standing instruction
# (docs/TEAM.md) is not to let unbuilt product surfaces gate anything; this
# script is that instruction applied to a seeding mechanism instead of a
# tape. Run it by hand: `bash scripts/seed-fixtures.sh`.
#
# What it actually checks, per line of the plan, by running it and reading
# what came back — not by assuming the shape is right:
#
#   1. The call reaches the registry and decodes into the right Action shape
#      (`(#refused "not built yet — T0xx builds it")`, the scheme-voice
#      refusal `--eval` prints when a well-formed call has no store/session/
#      review/watch subsystem behind it yet). This is EXPECTED today, for
#      every line — see fixtures/README.md's table for which task builds
#      which line.
#   2. A line that does NOT come back this way is flagged loudly, in one of
#      two directions:
#        - `#refused · Error: ...` (the CLI-voice error render, exit 1) means
#          this file's own scheme has drifted from the registry — a real bug
#          in fixtures/seed/plan.scm, script exits nonzero.
#        - anything else (an `#ok`, a real value) means the capability is
#          now IMPLEMENTED — a task landed, and this fixture is finally
#          ready to actually seed store state. Reported, not a failure.
#
# Both classifications were checked empirically before this script was
# written — `mark-seen! "not-a-target"` prints `#refused · Error:
# TypeMismatch: ...` (exit 1); every line in the committed plan prints
# `(#refused "not built yet — T0xx builds it")` (exit 0) — see
# fixtures/README.md for the full transcript.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixtures="$root/fixtures"
plan="$fixtures/seed/plan.scm"

if ! command -v phosphor >/dev/null 2>&1; then
    echo "seed-fixtures: 'phosphor' is not on \$PATH."
    echo "  Same requirement as tapes/ (tapes/README.md's 'Running' section):"
    echo "    cargo build --release --bin phosphor && \\"
    echo "      mkdir -p /tmp/phosphor-bin && \\"
    echo "      ln -sf \"\$PWD/target/release/phosphor\" /tmp/phosphor-bin/phosphor && \\"
    echo "      PATH=\"/tmp/phosphor-bin:\$PATH\" bash scripts/seed-fixtures.sh"
    exit 2
fi

if [ ! -f "$plan" ]; then
    echo "seed-fixtures: no plan at $plan"
    exit 2
fi

# fixtures/ is its own miniature workspace (see plan.scm's header) — every
# path inside the plan is relative to it, matching the design mockups'
# `src/retry.rs` rather than a repo-root-relative `fixtures/src/retry.rs`.
export PHOSPHOR_RUNTIME="$root/runtime"

# Determinism guard: nothing in the committed plan may embed an absolute
# path — that would make CP-5's "identical output on two machines"
# criterion false by construction, on whichever machine's home directory
# happened to be typed into the fixture. Checked mechanically, not assumed:
# a line starting with '/' after the leading '(' is what a hand-typed
# absolute path looks like in a quoted scheme string.
if grep -nE '"/[A-Za-z]' "$plan" >/dev/null 2>&1; then
    echo "seed-fixtures: fixtures/seed/plan.scm contains what looks like an absolute path:"
    grep -nE '"/[A-Za-z]' "$plan"
    echo "  V006's determinism requirement is no absolute paths in seeded output."
    exit 2
fi

expected=0
landed=0
broken=0

echo "seed-fixtures: running $plan against \$(command -v phosphor) from $fixtures"
echo

while IFS= read -r line; do
    # Skip blank lines and ';;'-prefixed commentary — every other
    # non-blank line is exactly one form, per plan.scm's own convention.
    trimmed="${line#"${line%%[![:space:]]*}"}"
    [ -z "$trimmed" ] && continue
    case "$trimmed" in
        ';;'*) continue ;;
    esac

    out="$(cd "$fixtures" && phosphor --eval "$trimmed" 2>&1)"
    code=$?

    case "$out" in
        '(#refused "not built yet'*)
            expected=$((expected + 1))
            printf '  ok      %s\n' "$trimmed"
            printf '            -> %s\n' "$out"
            ;;
        '#refused · Error:'*)
            broken=$((broken + 1))
            printf '  BROKEN  %s\n' "$trimmed"
            printf '            -> %s\n' "$out"
            ;;
        *)
            if [ "$code" -ne 0 ]; then
                broken=$((broken + 1))
                printf '  BROKEN  %s (exit %s)\n' "$trimmed" "$code"
                printf '            -> %s\n' "$out"
            else
                landed=$((landed + 1))
                printf '  LANDED  %s\n' "$trimmed"
                printf '            -> %s\n' "$out"
            fi
            ;;
    esac
done <"$plan"

echo
echo "seed-fixtures: $expected expected refusal(s), $landed landed capability answer(s), $broken broken."

if [ "$broken" -gt 0 ]; then
    echo "seed-fixtures: fixtures/seed/plan.scm has drifted from the registry — fix the plan, not the registry."
    exit 1
fi

if [ "$landed" -gt 0 ]; then
    echo "seed-fixtures: $landed line(s) are no longer refusals — a task landed. This script only reports;"
    echo "  it does not yet persist anything or assert CP-5's fixed-point. See fixtures/README.md's residue table."
fi

exit 0
