#!/usr/bin/env bash
# The fuzz runner — `scripts/fuzz.sh <target> [seconds] [-- libfuzzer args…]`.
#
# Fuzzing is a thing somebody runs deliberately, for as long as they choose. It
# is NOT in `just gate` and must not become part of it: a fuzzer's answer is
# "nothing yet", it gets slower the longer it is right, and a check that can
# redden for reasons unrelated to correctness is one people learn to skip
# (`harness`'s standing rule; the same reason Tier 2 and `just coverage` do not
# gate). This script exists so that "run the fuzzer" is one command with the
# corpus already in place rather than six flags to remember.
#
# Usage:
#
#   scripts/fuzz.sh                     # list the targets and their corpora
#   scripts/fuzz.sh seed                # regenerate seeds/ from the repo
#   scripts/fuzz.sh build               # compile every target (does not run)
#   scripts/fuzz.sh journal_open        # run until you stop it
#   scripts/fuzz.sh journal_open 60     # run for 60 seconds
#   scripts/fuzz.sh key_notation 300 -- -jobs=4
#
# What it does before every run: copies `fuzz/seeds/<target>/` into
# `fuzz/corpus/<target>/`. The seeds are tracked and the corpus is not
# (`fuzz/.gitignore` says why), so a fresh clone starts from the real inputs and
# a long-running machine keeps what it found.
#
# Toolchain: `fuzz/rust-toolchain.toml` selects nightly for that directory
# alone, because `-Zsanitizer` is nightly and the root pin (1.97.1, for tape
# determinism) must not move. Nothing here overrides a toolchain; cd-ing into
# `fuzz/` is what selects it, which is why every cargo call below runs from
# there.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fuzz="$root/fuzz"

die() {
    echo "scripts/fuzz.sh: $*" >&2
    exit 1
}

[ -d "$fuzz" ] || die "no fuzz/ directory at $fuzz"

command -v cargo-fuzz >/dev/null 2>&1 ||
    die "cargo-fuzz is not installed —
    cargo install cargo-fuzz"

# The nightly check is up front and by name. Without it the first failure is a
# wall of `error: the option 'Z' is only accepted on the nightly compiler`,
# which reads as a broken script rather than as a missing toolchain.
rustup toolchain list 2>/dev/null | grep -q '^nightly' ||
    die "cargo-fuzz needs nightly for -Zsanitizer, and it is not installed —
    rustup toolchain install nightly
  The root toolchain pin (1.97.1) is unaffected: fuzz/rust-toolchain.toml
  selects nightly for that directory only."

targets() {
    (cd "$fuzz" && cargo fuzz list 2>/dev/null)
}

# ── no argument · what is there ─────────────────────────────────────────────
if [ $# -eq 0 ]; then
    echo "fuzz targets:"
    while IFS= read -r target; do
        [ -n "$target" ] || continue
        seeds=0
        corpus=0
        [ -d "$fuzz/seeds/$target" ] && seeds=$(find "$fuzz/seeds/$target" -type f | wc -l | tr -d ' ')
        [ -d "$fuzz/corpus/$target" ] && corpus=$(find "$fuzz/corpus/$target" -type f | wc -l | tr -d ' ')
        printf '  %-18s %4s seeds  %6s in corpus\n' "$target" "$seeds" "$corpus"
    done < <(targets)
    echo
    echo "  scripts/fuzz.sh <target> [seconds] [-- libfuzzer args…]"
    echo "  scripts/fuzz.sh seed     — regenerate seeds/ from the repo"
    echo "  scripts/fuzz.sh build    — compile every target"
    exit 0
fi

command="$1"
shift

# ── seed · regenerate the corpus inputs from real repo files ────────────────
if [ "$command" = "seed" ]; then
    cd "$fuzz"
    cargo run --release --example seed
    exit 0
fi

# ── build · compile every target, run none ──────────────────────────────────
if [ "$command" = "build" ]; then
    cd "$fuzz"
    cargo fuzz build
    exit 0
fi

# ── <target> [seconds] [-- …] · run one ─────────────────────────────────────
target="$command"
targets | grep -qxF "$target" ||
    die "no such fuzz target: $target
  known: $(targets | tr '\n' ' ')"

seconds=""
if [ $# -gt 0 ] && [ "$1" != "--" ]; then
    case "$1" in
    '' | *[!0-9]*) die "expected a number of seconds, got '$1'" ;;
    *) seconds="$1" && shift ;;
    esac
fi
[ "${1:-}" = "--" ] && shift

seed_dir="$fuzz/seeds/$target"
corpus_dir="$fuzz/corpus/$target"
mkdir -p "$corpus_dir"
if [ -d "$seed_dir" ]; then
    # `cp` per file rather than `cp -r`: the corpus is flat and a seed named the
    # same as a found input should be overwritten by the seed, not nested.
    count=0
    while IFS= read -r file; do
        cp "$file" "$corpus_dir/"
        count=$((count + 1))
    done < <(find "$seed_dir" -type f)
    echo "seeded $corpus_dir with $count file(s) from seeds/$target"
else
    echo "no seeds/$target — starting from whatever is already in the corpus"
fi

cd "$fuzz"
args=("$target" "corpus/$target")
[ -n "$seconds" ] && args+=("--" "-max_total_time=$seconds")
[ $# -gt 0 ] && args+=("$@")

echo "── cargo fuzz run ${args[*]} ──"
echo
# No pipe. The exit code IS the result — a crash is a nonzero exit and a
# reproducer written to fuzz/artifacts/<target>/ — and reading it through a pipe
# would read the pipe's status instead (CLAUDE.md; this has bitten twice).
status=0
cargo fuzz run "${args[@]}" || status=$?

if [ "$status" -ne 0 ]; then
    echo
    echo "scripts/fuzz.sh: $target FAILED (exit $status)."
    echo "  The reproducer is under fuzz/artifacts/$target/. Next steps:"
    echo "    cd fuzz && cargo fuzz tmin $target artifacts/$target/<file>"
    echo "  then add the minimised input as a regression test in the owning"
    echo "  crate's test file — not as a blob in fuzz/artifacts/, which is"
    echo "  gitignored precisely so a reproducer has to become a test."
fi
exit "$status"
