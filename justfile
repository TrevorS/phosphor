# Phosphor justfile.

# Seam for `surface`'s vendor-diff / vendor-pull recipes (TEAM.md: only `surface`
# touches vendor/). Optional import — tolerates vendor/vendor.just being absent,
# so this file never has to change when that seam is filled in.
import? 'vendor/vendor.just'

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
fmt:
    cargo fmt --check

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
test:
    cargo nextest run --workspace --no-tests=pass

# cargo-deny: bans a second major of ratatui/ratatui-core (the rule SPIKES.md
# says matters most), plus licenses and the RustSec advisory DB. See
# deny.toml for the advisory-ignore list and why each entry is there.
deny:
    cargo deny check

# The structural-lint seam (T005 + T006/T007). Every structural lint is one
# executable script matching scripts/lint-*.sh — that glob is the entire
# contract. This recipe runs all of them in sorted order and is the ONLY
# thing CI calls for lints, so two people (T006: no-literal-colours,
# T007: no-store-mutation) can each add one without ever touching this file
# or .github/workflows/ci.yml. Passes vacuously today — no scripts exist yet.
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
# `just tapes`, but runs a single `tapes/<id>.tape`. Runs with cwd `tapes/`
# (matching run-tapes.sh) so a tape's relative paths — `Source
# "_config.tape"`, `Screenshot "artifacts/<id>.png"` — resolve identically
# whether it's regenerated this way or via `just tapes`.
tape id:
    @bash tapes/check-versions.sh
    cd tapes && vhs "{{ id }}.tape"
