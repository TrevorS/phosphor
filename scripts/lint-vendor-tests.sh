#!/usr/bin/env bash
# Structural lint: the vendored editor fork's own test suite runs, and passes.
#
# `T102`. This exists because of what it would have caught. `Code::notify_changes`
# turned each edit's offset into a `(row, col)` *after* the whole batch had been
# applied, so an undo step — whose edits run in descending offset order — asked
# the finished rope for a position it no longer had. Typing two characters at the
# end of a file and pressing `u` exited 101. That shipped, in the binary, past a
# green `just gate`.
#
# WHY GATE COULD NOT SEE IT. `[workspace] exclude` keeps both forks out of the
# member list (root `Cargo.toml`, deliberately — membership would put upstream
# code inside `cargo fmt --check` and `clippy -D warnings`). `just test` is
# `cargo nextest run --workspace`, so it compiles `ratatui-code-editor` as a
# dependency and never builds its tests. The fork carries nine phosphor patches
# (VENDOR.md numbers them 1, 2 and 4-10) laid over thirty-two upstream tests
# (`code.rs` 11, `diff.rs` 5, `tests/` 16 — recounted against `git show 40ff181`),
# and until this script nothing in CI had
# ever executed one of them — the seam that stops our lints at the fork boundary
# was also stopping our test runner. `tests/change_events.rs` is the first test
# file phosphor has written here; the other four are upstream's, unmodified.
#
# SCOPED TO ONE FORK, and the reason is measured rather than assumed:
# `vendor/ratatui-markdown` cannot be tested standalone at all. It carries no
# `[workspace]` table, so `cargo test` inside it resolves upward — in an agent
# worktree that is the *parent* checkout's root manifest, whose `exclude` paths
# do not match, and cargo errors out before compiling anything. Adding an empty
# `[workspace]` table gets further and then fails: two of its examples
# (`image`, `mermaid_image`) do not compile against the ratatui 0.30 bump. Both
# are recorded in that fork's VENDOR.md under known divergence. When they are
# fixed, add it to `FORKS` below.
#
# COST. 42s from a cold target directory on the machine this was written on,
# under 1s warm. No `CARGO_TARGET_DIR` override on purpose: the fork's default
# `target/` is the same one a human's `cargo test` in that directory uses (and
# the fork's own `.gitignore` covers it), so the two warm each other rather than
# each paying the 42s.
#
# NOT VACUOUS. An empty test file compiles, runs nothing, and exits 0 — which is
# how this check would rot into a no-op — so a green `cargo` is not the verdict.
# `tests/change_events.rs` must appear in the output AND report at least one
# test passed. Emptying it, renaming it or deleting it all fail here. Proven by
# planting each of the three, plus a one-token mutation of the fix itself.
#
# Exit 0 = the fork's suite ran and passed, exit 1 = it did not.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

FORKS=(ratatui-code-editor)

# The test binary whose absence means the guard is gone rather than green.
REQUIRED_BINARY="change_events"

status=0

for fork in "${FORKS[@]}"; do
    manifest="vendor/${fork}/Cargo.toml"

    if [ ! -f "$manifest" ]; then
        echo "lint-vendor-tests: ${manifest} is missing — the fork seam moved"
        status=1
        continue
    fi

    log="$(mktemp)"
    # **Headless, and that is not a preference.** The fork's `default` is
    # `["crossterm", "clipboard", "grammars-all"]` — it reproduces upstream's
    # dependency set exactly, which is the point of PATCH 2/3 — and `clipboard`
    # pulls `arboard`, which links X11/Wayland on Linux. So a bare `cargo test`
    # here passes on a developer's mac and, on CI, fails to build the very test
    # binary this lint exists to count: `change_events` reported *no tests* and
    # the lint fired for the right reason with the wrong cause. Found by
    # pushing, which is the only machine that has ever run it on Linux.
    #
    # `crossterm` stays because `tests/input.rs` and `tests/folding.rs` do not
    # compile without it. What is left is the configuration phosphor actually
    # consumes (`default-features = false` + `grammars-phosphor`), which is
    # also what `just vendor-build-headless` proves is buildable in both
    # directions — so this runs the fork the way we ship it rather than the way
    # upstream defaults it.
    features="crossterm,grammars-phosphor"
    # Run, THEN read the status. A pipe here would report the pipe's exit code,
    # which is how this repo has twice called a red check green.
    set +e
    cargo test --manifest-path "$manifest" \
        --no-default-features --features "$features" >"$log" 2>&1
    rc=$?
    set -e

    if [ "$rc" -ne 0 ]; then
        echo "lint-vendor-tests: ${fork}'s own suite failed (cargo exited ${rc})"
        echo "  Every hunk under vendor/ is phosphor's to carry, and so is every"
        echo "  test that proves one. Reproduce with:"
        echo "      cargo test --manifest-path ${manifest} \\"
        echo "          --no-default-features --features ${features}"
        echo
        sed 's/^/    /' "$log" | tail -40
        rm -f "$log"
        status=1
        continue
    fi

    # How many tests the required binary actually ran. Zero — or no such binary
    # — is the failure mode this block exists for, and it is not the same
    # question as "did cargo exit 0": an empty test file compiles and passes.
    # `Doc-tests` legitimately reports zero here and is not counted.
    required_passed="$(
        awk -v want="tests/${REQUIRED_BINARY}.rs" '
            index($0, "Running " want) { armed = 1; next }
            armed && /^test result:/ { print $4; exit }
        ' "$log"
    )"

    if [ -z "$required_passed" ] || [ "$required_passed" -eq 0 ]; then
        echo "lint-vendor-tests: ${fork}'s suite ran, but tests/${REQUIRED_BINARY}.rs"
        echo "  contributed ${required_passed:-no} tests. Those are T102's regression"
        echo "  tests — the undo crash that reached the shipping binary. Emptying,"
        echo "  renaming or deleting them silently removes the only thing standing"
        echo "  between that panic and a green gate."
        status=1
        rm -f "$log"
        continue
    fi

    binaries="$(grep -cE '^test result: ok\.' "$log" || true)"
    echo "lint-vendor-tests: ${fork} — ${binaries} suites green," \
        "${required_passed} of them in ${REQUIRED_BINARY}"
    rm -f "$log"
done

if [ "$status" -ne 0 ]; then
    echo "lint-vendor-tests: FAILED"
    exit 1
fi

exit 0
