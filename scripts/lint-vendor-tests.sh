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
# dependency and never builds its tests. The fork carries ten phosphor patches
# (VENDOR.md numbers them 1, 2 and 4-11) laid over thirty-two upstream tests
# (`code.rs` 11, `diff.rs` 5, `tests/` 16 — recounted against `git show 40ff181`),
# and until this script nothing in CI had
# ever executed one of them — the seam that stops our lints at the fork boundary
# was also stopping our test runner. `tests/change_events.rs` is the first test
# file phosphor has written here and `tests/tabs.rs` (patch 11, `T104`) is the
# second; the other four are upstream's, unmodified.
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
# **Every phosphor-written test file** must appear in the output AND report at
# least one test passed. Emptying one, renaming it or deleting it all fail here.
# Proven by planting each of the three, plus a one-token mutation of the fix
# itself.
#
# The list below is the guard's own non-vacuity, so it has to grow with the
# files. It named `change_events` alone for one window after `tests/tabs.rs`
# landed — the whole `T104` regression suite, which `just test` cannot see —
# and the gate line said so out loud (`8 suites green, 5 of them in
# change_events`) without anything reading it.
#
# Exit 0 = the fork's suite ran and passed, exit 1 = it did not.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

FORKS=(ratatui-code-editor)

# The test binaries whose absence means the guard is gone rather than green —
# every `tests/*.rs` phosphor wrote in this fork. The other four are upstream's.
REQUIRED_BINARIES=(change_events tabs syntax_path)

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
    # **`--color never`, and it is the whole reason this lint was red on CI and
    # green here.** `.github/workflows/ci.yml` sets `CARGO_TERM_COLOR: always`,
    # so cargo wraps its own words in SGR escapes: the line becomes
    # `<esc>[1m<esc>[32m   Running<esc>[0m tests/change_events.rs (…)`, and the
    # `index($0, "Running tests/…")` below — a literal substring match — stops
    # matching. The suite ran, every test passed, and the lint reported that the
    # file contributed none. A parser that reads another program's output has to
    # say how it wants that output; inheriting the ambient environment is what
    # made this pass on one machine and fail on the other.
    #
    # Reproduce the failure without CI: `CARGO_TERM_COLOR=always` in front of
    # this script.
    set +e
    cargo test --manifest-path "$manifest" --color never \
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

    # How many tests each required binary actually ran. Zero — or no such
    # binary — is the failure mode this block exists for, and it is not the
    # same question as "did cargo exit 0": an empty test file compiles and
    # passes. `Doc-tests` legitimately reports zero here and is not counted.
    tally=""
    vacuous=0
    for required in "${REQUIRED_BINARIES[@]}"; do
        required_passed="$(
            awk -v want="tests/${required}.rs" '
                index($0, "Running " want) { armed = 1; next }
                armed && /^test result:/ { print $4; exit }
            ' "$log"
        )"

        if [ -z "$required_passed" ] || [ "$required_passed" -eq 0 ]; then
            echo "lint-vendor-tests: ${fork}'s suite ran, but tests/${required}.rs"
            echo "  contributed ${required_passed:-no} tests. Those are phosphor's own"
            echo "  regression tests for a patch this fork carries — T102's undo crash"
            echo "  that reached the shipping binary, T104's tabstop. Emptying, renaming"
            echo "  or deleting them silently removes the only thing standing between"
            echo "  those and a green gate, because just test cannot see this fork."
            vacuous=1
            continue
        fi
        tally="${tally}${tally:+, }${required_passed} in ${required}"
    done

    if [ "$vacuous" -ne 0 ]; then
        status=1
        rm -f "$log"
        continue
    fi

    binaries="$(grep -cE '^test result: ok\.' "$log" || true)"
    echo "lint-vendor-tests: ${fork} — ${binaries} suites green, ${tally}"
    rm -f "$log"
done

if [ "$status" -ne 0 ]; then
    echo "lint-vendor-tests: FAILED"
    exit 1
fi

exit 0
