#!/usr/bin/env bash
# V006 — runs fixtures/seed/plan.scm against `phosphor --eval` (T023) and
# reports exactly what each line does today.
#
# NOT a lint. Deliberately outside the scripts/lint-*.sh glob `just lint`
# walks: most of the plan refuses today (review blocks are S6, threads are S7,
# watches are S8 — none built yet, per docs/TASKS.md), so wiring this into the
# gate would redden CI on a build that is behaving exactly as designed.
# harness's own standing instruction (docs/TEAM.md) is not to let unbuilt
# product surfaces gate anything; this script is that instruction applied to a
# seeding mechanism instead of a tape. Run it by hand:
# `bash scripts/seed-fixtures.sh`.
#
# **That exemption is also why it was broken for a whole phase and nobody
# knew.** Nothing ran it between T100 and T041, and it had two faults by then:
# it aborted on its own first line under `set -e`, and its classifier matched
# an answer shape T100 had replaced. Both are marked at their sites below, and
# `scripts/lint-door-callers.sh` is what makes the pair structural — the one
# check a script outside the gate can still be held to is whether it is
# *capable* of reporting.
#
# What it checks, per line of the plan, by running it and reading what came
# back — not by assuming the shape is right:
#
#   1. The call reaches the registry and decodes into the right Action shape.
#      `#refused · not built yet — T0xx builds it` (or `#raised · …` for a
#      query) is the door's voice for a well-formed call with no
#      store/session/review/watch subsystem behind it yet. This is EXPECTED
#      for every line whose task has not landed — see fixtures/README.md's
#      table for which task builds which line.
#   2. A line that does NOT come back this way is flagged loudly, in one of
#      two directions:
#        - any other `#refused · …` / `#raised · …` means this file's own
#          scheme has drifted from the registry — a real bug in
#          fixtures/seed/plan.scm, script exits nonzero.
#        - anything else (an `#ok`, a real value) means the capability is
#          now IMPLEMENTED — a task landed. Reported, not a failure.
#
# **A landed line is not a seeded fixture**, and the summary says so at the
# bottom rather than letting a reader infer it: every line is its own
# `phosphor --eval` process, so nothing one line writes survives to the next.
# T044 is the task that changes that.

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

    # **`if`, not a bare assignment, and `set -e` is why.** A refusal exits
    # nonzero — `T100`'s ruling, *"one door, one refusal, one exit code"* — so
    # `out="$(…)"; code=$?` aborts the whole script on the first line of the
    # plan under `set -euo pipefail`, before printing a single row. That is
    # what this script did from `T100` until `T041` ran it: the header below
    # describes a per-line transcript it could not produce, and nothing caught
    # it because this file is deliberately outside the glob `just lint` walks.
    # An `if` condition is the one context where `set -e` is suspended.
    #
    # `</dev/null` because the loop is reading the plan on stdin, and a child
    # that read any of it would eat the rest of the file.
    if out="$(cd "$fixtures" && phosphor --eval "$trimmed" 2>&1 </dev/null)"; then
        code=0
    else
        code=$?
    fi

    # **The shapes below are `T100`'s, not the ones this file was written
    # against.** It matched `(#refused "not built yet — T0xx builds it")` — a
    # bare list, which is what `--eval` printed before the door had a voice —
    # so after `T100` every line in the plan classified as BROKEN and the
    # summary said the plan had drifted from the registry. It had not; the
    # classifier had. Corrected at `T041` by running it.
    case "$out" in
        '#refused · not built yet'* | '#raised · not built yet'*)
            expected=$((expected + 1))
            printf '  ok      %s\n' "$trimmed"
            printf '            -> %s\n' "$out"
            ;;
        '#refused · '* | '#raised · '*)
            # A refusal that is not *"not built yet"* is this file's own scheme
            # disagreeing with the registry — a wrong argument, a wrong shape.
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
    echo "seed-fixtures: $landed line(s) are no longer refusals — a task landed."
    echo
    echo "  ONE PROCESS PER LINE, AND STATE NOW SURVIVES BETWEEN THEM (T044). Every line above is"
    echo "  still its own 'phosphor --eval', but the store is journalled to"
    echo "  \$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/seen.journal, so a line reads what the"
    echo "  lines before it wrote."
    echo
    echo "  This paragraph said the opposite for a phase, and the two 'mark-seen!' lines are the"
    echo "  proof either way: against the empty store of a fresh process they answered 0, and they"
    echo "  answer 1 now — they find the regions 'declare-regions!' wrote in an earlier process."
    echo "  That is V006's criterion ('seeded store state is reachable through phosphor --eval'),"
    echo "  and it was moved onto T041, answered there rather than met, and met here."
    echo
    echo "  WHAT IS STILL NOT ASSERTED: CP-5's fixed-point. This script reports what each line"
    echo "  answers; it does not check that running it twice leaves the same store, or that two"
    echo "  machines get identical output. See fixtures/README.md's residue table."
fi

exit 0
