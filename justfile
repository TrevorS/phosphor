# Phosphor justfile.

# Seam for `surface`'s vendor-diff / vendor-pull recipes (TEAM.md: only `surface`
# touches vendor/). Optional import — tolerates vendor/vendor.just being absent,
# so this file never has to change when that seam is filled in.
import? 'vendor/vendor.just'

# `just` with no arguments runs the FIRST recipe in the file, so which recipe
# that is decides what a bare `just` does. It was `build`, which meant a typo'd
# or half-typed command silently compiled the workspace. This is that slot,
# deliberately claimed: a bare `just` lists what there is.
#
# It has to stay at the top. Moving a recipe above it changes what `just` does,
# which is the kind of thing nobody expects a reordering to do.

# List the recipes (what a bare `just` does).
default:
    @just --list

# T005: CI calls these recipes rather than inlining cargo invocations, so
# "green in CI" and "green from `just <recipe>` on your machine" never drift
# apart — reproduce any CI failure locally with the same command CI ran.

# Build the whole workspace.
build:
    cargo build --workspace

# Format check — matches CI exactly. Drop --check to fix in place locally.
#
# NO `--all`, deliberately. `[workspace] exclude` keeps the two vendored forks
# out of the member list (see Cargo.toml), and `cargo clippy --workspace`
# honours that — but `cargo fmt --all` does NOT: cargo-fmt resolves `--all`
# over every *local* package under the workspace root, which re-includes
# `vendor/ratatui-code-editor` and `vendor/ratatui-markdown` through their path
# deps. With `--all` this recipe fails on 6 upstream files we did not write
# (rustfmt 1.8.0), and the only way to green it would be to reformat the forks
# — which is precisely the divergence `just vendor-diff` exists to prevent.
#
# Without `--all`, cargo-fmt at a virtual manifest root formats exactly the
# workspace members. Verified by planting a formatting violation in all seven
# crates: all seven were flagged, and no vendored file was touched. Coverage of
# our own code is identical; only the fork seam changes.

# Format check — matches CI. Never add `--all` (see above).
fmt:
    cargo fmt --check

# Formats in place. Same scoping rule as `fmt`, and the same prohibition: a
# `--all` here would reformat the forks, which is the one thing `vendor-diff`
# exists to stop. This recipe exists so there is a right verb to reach for —
# agents kept reaching for `cargo fmt --all` and hitting the hook that blocks it.

# Format in place (never `--all`).
fmt-fix:
    cargo fmt

# Clippy, warnings denied — matches CI exactly.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Test suite via cargo-nextest — per-test process isolation (SPIKES.md's
# hygiene table): tests touching the XDG state dir or terminal state get
# flaky under a shared-process runner otherwise.
# --no-tests=pass: nextest's default exits nonzero when the workspace has
# zero tests anywhere (true today — T005's "green on the empty workspace"
# needs this). Safe to keep permanently: it only changes behavior in that
# zero-tests-anywhere case, never once any test exists.

# **nextest does not run doctests, and nothing else did either.** That is a
# documented upstream limitation, not a flag we can set: nextest's runner has no
# doctest support at all, so `cargo nextest run` skips them silently. CI runs
# `just test` and nothing in the repository ran `cargo test --doc`, so every
# example in a doc comment was compiled by `cargo doc` (the doc-links lint) and
# executed by nobody.
#
# One exists today and passes, so this closes a hole rather than a defect — but
# a hole in a harness is worth more attention than a passing test: the next
# runnable example somebody writes would have been dead on arrival, and it would
# have looked like coverage.
#
# Second command rather than folded in, because the two runners are genuinely
# different programs and a reader should see that. It costs about a second on a
# warm build.

# Run the test suite via cargo-nextest, then the doctests nextest cannot see.
test:
    cargo nextest run --workspace --no-tests=pass
    cargo test --doc --workspace

# One slice of the suite, for CI's matrix. `just test` is still the whole thing
# and is what you run locally.
#
# **Sharding rather than more threads, and that is a measurement not a taste.**
# The test phase on a runner was 866s of a 17m job — the build was only 163s of
# it — because `ubuntu-latest` on a private repo is a **2-vCPU** box and nextest
# defaults to one thread per core. The same suite takes 319s at `--test-threads
# 2` on the machine this was measured on, so the runner is not doing anything
# strange; it has two slow cores.
#
# The obvious fix is to raise the thread count, because these tests are
# latency-bound rather than CPU-bound: the pty harness waits 250ms of quiet
# after every keystroke and the fake language servers sleep on purpose. Measured
# on the pty suite, 2 threads → 146s, 4 → 74s, 8 → 46s. But at 12 threads it went
# *back up* to 52s and a test failed, and that is the whole argument against
# turning the knob: the harness has 30s deadlines, and a starved child editor
# blows them. Flakes appeared exactly when threads exceeded logical CPUs.
#
# Four 2-thread runners are the same parallelism with none of the starvation —
# each shard runs at the concurrency that is proven clean, and they run at once.
#
# `--partition count:k/n` is nextest's own splitter, so nothing here maintains a
# list of which tests go where. A hand-written split is a list that rots the
# first time somebody adds a test file.
test-shard k n:
    cargo nextest run --workspace --no-tests=pass --partition count:{{ k }}/{{ n }}

# The doctests alone. One shard runs this so the rustdoc pass happens exactly
# once — it is a second compiler invocation, not a second test run.
test-doc:
    cargo test --doc --workspace

# cargo-deny: bans a second major of ratatui/ratatui-core (the rule SPIKES.md
# says matters most), plus licenses and the RustSec advisory DB. See
# deny.toml for the advisory-ignore list and why each entry is there.

# cargo-deny: advisories, licences, and the duplicate-ratatui ban.
deny:
    cargo deny check

# Every benchmark here exists because a NUMBER WOULD CHANGE A DECISION, and
# each is a target rather than a script so `just clippy --all-targets` keeps it
# compiling. `harness = false` throughout — no libtest, no criterion. Each one
# prints its numbers and asserts only the SHAPE (flat versus climbing, O(n)
# versus O(n²), a count in versus a count out), because a threshold on wall
# clock goes red for reasons unrelated to correctness. That is also why this
# recipe is not in `gate`: see the Measurements block below, which makes the
# same argument about `coverage`.
#
#   phosphor-ui/benches/frame_cache.rs      T079 — VM invocations flat while
#                                           frames climb. CP-2 reads its verdict.
#   phosphor/benches/vm_invocations.rs      T091 — the same claim counted from
#                                           outside the shipping binary, on a pty.
#   phosphor-core/benches/journal.rs        B1 — what an undo record costs on the
#                                           keystroke path, what fsync would cost
#                                           instead, and what a compaction of a
#                                           long session reclaims (T095's input).
#   phosphor-ui/benches/soft_wrap.rs        B2 — what a resize costs on the one
#                                           uncached path in the frame. This
#                                           said NOT RUN BY THIS RECIPE YET,
#                                           pending a `[[bench]] name =
#                                           "soft_wrap"` / `harness = false`
#                                           pair that was outside the writing
#                                           agent's file lock. The pair landed
#                                           (crates/phosphor-ui/Cargo.toml), so
#                                           it runs. The warning is worth
#                                           keeping though, because the failure
#                                           is SILENT: without the pair cargo
#                                           autodiscovers the file under
#                                           libtest's harness, it compiles, this
#                                           recipe reports it, and it runs zero
#                                           measurements. A benchmark that finds
#                                           a 5.7-second resize would say
#                                           nothing at all.
#
# There is deliberately no benchmark of the input machine. It was measured and
# came back at 57-302 ns per keystroke, flat between a 100-line buffer and a
# 20,000-line one — five orders of magnitude inside a frame. The cost that IS on
# the keystroke path is `phosphor/resolve`, which is a Steel call and uncached by
# design (T022), and `vm_invocations.rs` already counts it. A benchmark that will
# never change a decision is maintenance for nothing.

# Run the benchmarks (frame cache, VM invocations, journal, soft wrap).
bench:
    cargo bench --workspace

# ── Measurements ─────────────────────────────────────────────────────────────
#
# `coverage`, `hack` and `unused-deps` are the three below, and none of them is
# in `gate`. That is one decision, not three, and it is the same one `bench` and
# `tapes-diff` already carry: a check that can fail a build for a reason
# unrelated to correctness teaches the team to stop reading it.
#
# For `coverage` specifically, because the instinct on reading the next recipe
# will be to wire it into `gate` with a floor: A COVERAGE FLOOR IS A CHANGE
# DETECTOR. It reddens when a refactor deletes tested code, when a `#[cfg]` arm
# stops being compiled on this platform, when a test moves between crates —
# every one of those a green build that now reports red. The response is always
# to lower the floor or to write a test whose only job is to colour a line, and
# a test written to colour a line costs a maintenance obligation and proves
# nothing. The number is an input to a person deciding where to look, and it
# stops being that the moment it can fail CI.

# Coverage, as a per-file table sorted worst first — `just coverage [substring]`.
#
# `--json --summary-only` rather than the default text report, because the text
# report sorts by path: it answers "how covered is file X" and answers "where is
# the suite thin" only if you read every row. `scripts/coverage_report.py` sorts
# it and truncates to the worst 20, so the thin files and the TOTAL are both on
# screen when the command returns (`--all` for every row; a bare substring
# filters — `just coverage journal`).
#
# `nextest`, matching `just test` — same runner, same per-test process
# isolation, so the coverage figure describes the suite that actually gates
# rather than a differently-isolated one that does not.
#
# CARGO_TARGET_DIR, and what it is NOT for. The obvious reason to set it is that
# `-Cinstrument-coverage` is a different fingerprint from every other recipe
# here, so a shared `target/` would have `just coverage` and `just test`
# invalidate each other and rebuild from scratch every alternation. That reason
# is wrong, and it was in this comment until a running process disproved it:
# cargo-llvm-cov already nests its own build under `target/llvm-cov-target`
# (visible in the `cargo nextest run --target-dir …` it spawns), so `target/debug`
# was never at risk and this override buys nothing there.
#
# What it does buy is one worktree, several agents — how this build runs windows
# (TEAM.md's concurrency rules). cargo-llvm-cov clears the stale `.profraw` set
# when a run starts, so a bare `cargo llvm-cov` in another shell and this recipe
# would silently delete each other's profile data. Two runs of THIS recipe still
# collide; the override narrows the window rather than closing it. Worth keeping
# for that alone, and worth dropping the moment the shared tree stops being one.
#
# Run in two phases — `--no-report`, then `report` — rather than as the single
# `cargo llvm-cov nextest --json …` invocation this started as. The first shape
# was found wrong by running it: one flaky pty test failed, nextest's default
# fail-fast cancelled 642 of 679 tests, and cargo-llvm-cov never wrote a report
# at all. So the answer to "where is the suite thin" was nothing, because one
# test was red — the opposite of what a measurement is for, and exactly when you
# most want the map. `--no-fail-fast` runs the rest; splitting the phases means
# the report is built from whatever profile data exists either way.
#
# The exit code is still the test run's. This recipe does not gate anything, so
# nothing depends on that — but a measurement that reports success over a red
# suite is its own small lie, and the banner plus the status keep it honest.

# Per-file coverage, worst first — `just coverage [substring|--all]`. Never gates.
coverage *args:
    #!/usr/bin/env bash
    set -uo pipefail
    cd "{{ justfile_directory() }}"
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
        echo "just coverage: cargo-llvm-cov is not installed —"
        echo "    cargo binstall cargo-llvm-cov"
        exit 1
    fi
    out="target/coverage/summary.json"
    mkdir -p "$(dirname "$out")"
    tests=0
    CARGO_TARGET_DIR=target/coverage \
        cargo llvm-cov nextest --workspace --no-tests=pass \
        --no-report --no-fail-fast || tests=$?
    CARGO_TARGET_DIR=target/coverage \
        cargo llvm-cov report --json --summary-only --output-path "$out" || exit $?
    echo
    python3 scripts/coverage_report.py "$out" {{ args }}
    if [ "$tests" -ne 0 ]; then
        echo
        echo "  NOTE: the test run exited ${tests} — some tests failed or were not run,"
        echo "  so the figures above describe a partial suite. \`just test\` is the gate."
    fi
    exit "$tests"

# The HTML report, for reading one file's uncovered lines in place.
#
# Reuses the profile data `just coverage` already produced — `llvm-cov report`
# re-renders it without re-running a test — and falls back to a full run when
# there is none. So the loop is: `just coverage` to find the thin file, then
# `just coverage-html` to see which lines inside it nobody reached.
#
# The reuse test globs for `*.profraw`, not for the target directory: a run that
# compiled and then failed leaves the directory populated and the profile data
# gone, because cargo-llvm-cov clears the old `.profraw` set before it starts.
# Testing for the directory made the common case — `just coverage` red because a
# test does not compile, `just coverage-html` next — fail with llvm-cov's own
# "not found *.profraw files", which reads as a broken recipe rather than as
# "there is nothing to re-render yet".

# The HTML coverage report — read one file's uncovered lines in place.
coverage-html:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    shopt -s nullglob
    profraw=(target/coverage/llvm-cov-target/*.profraw)
    if [ ${#profraw[@]} -gt 0 ]; then
        echo "re-rendering ${#profraw[@]} profile(s) from the last run — no tests re-run"
        CARGO_TARGET_DIR=target/coverage cargo llvm-cov report --html
    else
        echo "no coverage data yet — running the suite first"
        CARGO_TARGET_DIR=target/coverage \
            cargo llvm-cov nextest --workspace --no-tests=pass --html
    fi
    echo
    echo "open target/coverage/llvm-cov/html/index.html"

# Feature combinations — does every one of them build?
#
# SPIKES.md's hygiene table has carried `cargo-hack` since M-0 for exactly one
# reason, [Q4](docs/IMPLEMENTATION-PLAN.md#q4)'s guardrail: the transcript has to
# render with the markdown feature on AND off. `vendor/ratatui-markdown/VENDOR.md`
# promises `cargo hack --feature-powerset` will prove it. Nothing kept that
# promise until this recipe; the tool was not installed.
#
# `--each-feature`, not `--feature-powerset`, is the default here. The powerset
# is 2^n builds per crate and the workspace has three optional features across
# two crates (`phosphor-ui/markdown`, `phosphor-buffer/{clipboard,grammars-extra}`),
# which is small today and is not the argument — the argument is that the defect
# this catches is almost always "a crate that only compiles with default features
# on", and `--each-feature` catches that in n+2 builds instead of 2^n. Reach for
# the powerset when you have added a feature that INTERACTS with another one:
#
#     just hack --feature-powerset
#
# NOT in `gate`, and this one is a cost argument rather than a flakiness one:
# `gate` is run constantly and every extra full-workspace build is paid by
# everybody, every time. Run it when you touch a `[features]` table, and in the
# `features` CI job, which is where a slow check belongs.

# Check every feature combination — `just hack [--feature-powerset]`.
hack *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    command -v cargo-hack >/dev/null 2>&1 || {
        echo "just hack: cargo-hack is not installed —"
        echo "    cargo binstall cargo-hack@0.6.45   # SPIKES.md's hygiene table"
        exit 1
    }
    args=({{ args }})
    # An `if`, not `[ … ] && args=(…)`: under `set -e` an AND-OR list whose test
    # fails is the classic way a recipe exits 1 having done nothing, and this one
    # takes the failing branch precisely when an argument WAS passed.
    if [ ${#args[@]} -eq 0 ]; then
        args=(--each-feature)
    fi
    cargo hack --workspace "${args[@]}" check --all-targets

# Dependencies nothing imports — `cargo-machete` (SPIKES.md's hygiene table).
#
# An unused dependency is build time, audit surface, and a `cargo deny` row for
# nothing. `cargo-machete` over `cargo-udeps` for one disqualifying reason:
# `udeps` requires a nightly toolchain, and `rust-toolchain.toml` pins 1.97.1
# because VHS reference images are only comparable if the binary that made them
# was built the same way. A second toolchain to run one lint is a worse trade
# than machete's imprecision.
#
# And it IS imprecise — it greps for the crate name rather than resolving it, so
# a dependency whose lib name differs from its package name reads as unused.
# `steel-core` is exactly that case in this workspace: the package is
# `steel-core`, the lib is `steel`, and every use site says `use steel::`.
# `--with-metadata` resolves the rename and is why it is on by default here.
#
# NOT in `gate`: it still reports true-but-not-actionable findings — see this
# recipe's own output for the placeholder crates — and a lint whose right answer
# is sometimes "yes, deliberately" is a lint people learn to skip.

# Dependencies nothing imports — cargo-machete over the workspace crates.
unused-deps *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    command -v cargo-machete >/dev/null 2>&1 || {
        echo "just unused-deps: cargo-machete is not installed —"
        echo "    cargo binstall cargo-machete@0.9.2   # SPIKES.md's hygiene table"
        exit 1
    }
    # `crates`, not the repo root: `vendor/` is upstream code we did not write
    # and its dependency hygiene is not ours to hold to this standard — the same
    # scoping every `scripts/lint-*.sh` uses.
    cargo machete --with-metadata --skip-target-dir crates {{ args }}

# The structural-lint seam (T005 + T006/T007). Every structural lint is one
# executable script matching scripts/lint-*.sh — that glob is the entire
# contract. This recipe runs all of them in sorted order and is the ONLY
# thing CI calls for lints, so two people (T006: no-literal-colours,
# T007: no-store-mutation) can each add one without ever touching this file
# or .github/workflows/ci.yml. Three land today: no-literal-colours (T006),
# no-store-mutation (T007) and no-app-layer-in-ui (T002). The vacuous-pass
# branch below is kept as a guard, not as a description of the present.

# Run every structural lint in scripts/lint-*.sh.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    scripts=(scripts/lint-*.sh)
    if [ ${#scripts[@]} -eq 0 ]; then
        echo "just lint: no scripts/lint-*.sh present yet — passing vacuously"
        exit 0
    fi
    status=0
    for script in "${scripts[@]}"; do
        echo "── running ${script} ──"
        if ! bash "$script"; then
            echo "FAILED: ${script}"
            status=1
        fi
    done
    if [ "$status" -ne 0 ]; then
        echo "just lint: one or more structural lints failed (see above)"
    fi
    exit "$status"

# Version-checks vhs/ttyd/ffmpeg (pixel comparison only means anything against
# the pinned renderer, tapes/README.md), then records every tapes/*.tape.
# `_`-prefixed files (Source fragments, reference tables) are skipped by
# run-tapes.sh's own convention — see tapes/README.md's "Layout" section.

# Regenerate the Tier-2 (VHS) tape library — every screen.
tapes:
    @bash tapes/check-versions.sh
    @bash tapes/run-tapes.sh

# V005: regenerate exactly one screen — `just tape 1a`. Same version gate as
# `just tapes`, and the capture itself goes through tapes/record-one.sh so this
# runs in the same cwd and the same environment `run-tapes.sh` gives a full
# regeneration — a scratch `$XDG_CONFIG_HOME` among them, without which a
# single re-record picks up the operator's own `init.scm` and every other
# screen in the library does not.

# Regenerate exactly one screen — `just tape 1a`.
tape id:
    @bash tapes/check-versions.sh
    @bash tapes/record-one.sh "{{ id }}"

# Builds the binary and puts it where the tapes can find it. `just tapes` and
# `just tape <id>` both need `phosphor` on `$PATH`, and until this recipe
# existed nothing in the repo put it there — a gap CLAUDE.md documented rather
# than closed, and one that is hit at every checkpoint that produces artifacts.
#
# Release, not debug: a tape records a real terminal session, and a debug binary
# is slow enough that `Sleep` values calibrated against one would not transfer.

# Build and install `phosphor` on $PATH (for `just tapes`).
install:
    cargo install --path crates/phosphor --locked

# V006 / CP-5 — does seeding the fixture twice leave the same store?
#
# `scripts/seed-fixtures.sh` reports what each line of the seed plan answers;
# this asks the question that one names as unasserted, and it is the question
# `CP-5`'s tapes stand on. A capture of the unseen picker is evidence about the
# editor only if the store behind it is the same store every time it is made —
# otherwise the pixel diff is measuring the seed.
#
# Not in `gate` and not a lint: it needs `phosphor` on $PATH (`just install`)
# and runs the whole plan twice, two dozen process launches. Run it before
# blessing a tape.
seed-determinism:
    bash scripts/seed-determinism.sh

# CP-4's three language servers, in a container that has all of them.
#
# `crates/phosphor-buffer/tests/lsp_servers.rs` attaches to rust-analyzer,
# typescript-language-server and pyright-langserver for real. On a developer's
# machine it skips whichever is missing — deliberately, because a test that
# reddens for an absent tool trains everyone to ignore a red build. This is
# where nothing is absent, so nothing skips.
#
# **Not in `gate` and not in CI.** It needs a Docker daemon and pulls an image;
# both are the kind of dependency that makes a build fail for reasons unrelated
# to the code. Run it when touching the LSP client, and at CP-4.
#
# The named volume is what makes the second run fast: the host's `target/` holds
# macOS objects and the container's must not meet them, so it gets its own.
lsp-docker *ARGS:
    docker build -f docker/lsp.Dockerfile -t phosphor-lsp:latest .
    docker run --rm \
        -v "$(pwd)":/phosphor \
        -v phosphor-lsp-target:/phosphor-target \
        phosphor-lsp:latest {{ ARGS }}

# A shell in the same image, for when a server misbehaves and the question is
# what it actually said.
lsp-docker-shell:
    just lsp-docker bash

# V007 — the pixel-diff runner. Captures fresh, compares against the
# committed reference at git HEAD, and on a mismatch writes a legible
# side-by-side diff image under tapes/artifacts/_diffs/ instead of failing
# the process — see tapes/diff-tapes.sh's own header for the full contract
# and tapes/README.md's V007 section for the worked proof. Deliberately
# absent from `just gate`: Tier 2 is a change detector, not a build gate
# (docs/TASKS.md; harness's own characteristic-failure guard, TEAM.md).

# Diff every screen's fresh capture against its committed reference.
tapes-diff:
    @bash tapes/diff-tapes.sh

# Diff exactly one screen — `just tape-diff 1a`.
tape-diff id:
    @bash tapes/diff-tapes.sh "{{ id }}"

# Everything CI runs, in CI's order, as one command.
#
# CI runs these as five separate jobs and `vendor-diff` inside `lint`, so
# "is it green" has been six invocations to remember and one to forget. A
# checkpoint gate asks that question every time; this is the answer.
#
# Deliberately NOT `bench`: T079's measurement is a measurement, and a number
# that moves with the machine has no business failing a build (harness's
# standing rule — a change detector that gates CI teaches people to ignore it).

# Everything CI runs: fmt, lint, clippy, test, deny, vendor-diff.
gate:
    #!/usr/bin/env bash
    set -uo pipefail
    status=0
    # `vendor-diff` takes `--stat` HERE and nowhere else. Bare, it prints the
    # forks' full divergence — 3,336 lines of hunks, VENDOR.md prose and deleted
    # binaries — which is exactly right when you are reviewing a fork change and
    # exactly wrong as the last step of a gate, because it buries the verdict
    # under a wall of expected output and teaches you to stop reading. `--stat`
    # is the signal: which files diverge, and by how much.
    #
    # Nothing is lost by shortening it. `vendor-diff` never failed on an
    # undocumented hunk in the first place — it prints and exits 0. The audit
    # CLAUDE.md promised is `scripts/lint-vendor-hunks.sh`, which runs inside
    # `just lint` above and fails on a fork file no VENDOR.md mentions.
    for recipe in fmt lint clippy test deny "vendor-diff --stat"; do
        echo "── just ${recipe} ──"
        # shellcheck disable=SC2086 # the one entry with a flag is split on purpose
        if ! just ${recipe}; then
            echo "FAILED: just ${recipe}"
            status=1
        fi
    done
    if [ "$status" -ne 0 ]; then
        echo
        echo "just gate: NOT green — see the FAILED lines above."
    else
        echo
        echo "just gate: green — fmt, lint, clippy, test, deny, vendor-diff."
    fi
    exit "$status"

# `insta` snapshot review — the golden frames (T018) and screen 6b (T022).
#
# A snapshot that changed without a mockup changing is a regression; a layout
# diff is geometry and an fg/bg diff with the text grid unchanged is a palette
# change, which is the more serious of the two. Window D adds four more screens
# (3c, 6d, 8e, 7c), so the review loop is about to matter more than it has.

# Review changed golden frames interactively.
review:
    cargo insta review
