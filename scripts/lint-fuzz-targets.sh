#!/usr/bin/env bash
# Every fuzz target is declared, built and seeded — and `fuzz/` stays out of the
# workspace.
#
# `just gate` cannot run the fuzzer and must not: fuzzing needs nightly for
# `-Zsanitizer`, the root toolchain is pinned to 1.97.1 for tape determinism,
# and a search whose answer is "nothing yet" has no business gating a build
# (`harness`'s standing rule, the same one that keeps Tier 2 and `just coverage`
# out). So what CI can check about `fuzz/` is *structure*, and each of the four
# checks below is a way a fuzz crate silently stops testing anything:
#
#   1. **A target with no `[[bin]]`.** cargo-fuzz builds the crate's binaries,
#      not the contents of `fuzz_targets/`. A `.rs` file added there without a
#      manifest entry compiles never, runs never, and appears in no listing —
#      it looks like coverage and is a dead file.
#   2. **A `[[bin]]` with no source.** The mirror image; `cargo fuzz build`
#      fails, but only for whoever runs it, which by design is nobody in CI.
#   3. **A target with no seeds.** A fuzzer starting from an empty corpus spends
#      its first hour rediscovering the file format and, past a checksum, never
#      gets there at all — `fuzz_targets/journal_records.rs`'s header is the
#      worked example. An unseeded target is the "corpus nobody will look at"
#      failure, and it is invisible until someone runs it for an hour.
#   4. **`fuzz` missing from the root `[workspace] exclude`.** Then `cargo
#      build --workspace` pulls the fuzz crate in, `#![no_main]` targets fail to
#      link on stable, and `just gate` goes red for a reason that has nothing to
#      do with the change in front of you.
#
# Deliberately NOT checked: that the targets compile. That needs nightly and a
# full sanitizer build — `scripts/fuzz.sh build` is the command, and it belongs
# to the person doing the fuzzing.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

manifest="fuzz/Cargo.toml"
violations=0

if [ ! -d fuzz ]; then
    echo "lint-fuzz-targets: no fuzz/ directory — nothing to check"
    exit 0
fi

[ -f "$manifest" ] || {
    echo "fuzz/ exists but $manifest does not — a cargo-fuzz crate needs one"
    exit 1
}

# The declared targets: every `path = "fuzz_targets/<name>.rs"` in the manifest.
# Matching on the path rather than on `name =` is deliberate — it is the field
# that has to agree with the file on disk, and it is what makes check 2 real.
declared="$(sed -n 's/^path = "fuzz_targets\/\(.*\)\.rs"$/\1/p' "$manifest" | sort)"

# The files on disk.
on_disk=""
if [ -d fuzz/fuzz_targets ]; then
    on_disk="$(
        find fuzz/fuzz_targets -maxdepth 1 -name '*.rs' -type f |
            sed 's|.*/||; s|\.rs$||' | sort
    )"
fi

# ── 1 · every source file is declared ───────────────────────────────────────
while IFS= read -r target; do
    [ -n "$target" ] || continue
    if ! printf '%s\n' "$declared" | grep -qxF "$target"; then
        echo "fuzz/fuzz_targets/${target}.rs has no [[bin]] entry in $manifest"
        echo "    cargo-fuzz builds binaries, not a directory — add:"
        echo "        [[bin]]"
        echo "        name = \"${target}\""
        echo "        path = \"fuzz_targets/${target}.rs\""
        echo "        test = false"
        echo "        doc = false"
        echo "        bench = false"
        violations=$((violations + 1))
    fi
done < <(printf '%s\n' "$on_disk")

# ── 2 · every declared target has a source file ─────────────────────────────
while IFS= read -r target; do
    [ -n "$target" ] || continue
    if [ ! -f "fuzz/fuzz_targets/${target}.rs" ]; then
        echo "$manifest declares fuzz_targets/${target}.rs, which does not exist"
        violations=$((violations + 1))
    fi
done < <(printf '%s\n' "$declared")

# ── 3 · every declared target has at least one seed ─────────────────────────
while IFS= read -r target; do
    [ -n "$target" ] || continue
    [ -f "fuzz/fuzz_targets/${target}.rs" ] || continue
    seeds=0
    if [ -d "fuzz/seeds/${target}" ]; then
        seeds=$(find "fuzz/seeds/${target}" -type f | wc -l | tr -d ' ')
    fi
    if [ "$seeds" -eq 0 ]; then
        echo "fuzz target '${target}' has no seeds in fuzz/seeds/${target}/"
        echo "    An empty corpus is a fuzzer that never reaches the format."
        echo "    Add a case to fuzz/examples/seed.rs, then: scripts/fuzz.sh seed"
        violations=$((violations + 1))
    fi
done < <(printf '%s\n' "$declared")

# ── 4 · fuzz/ is excluded from the workspace ────────────────────────────────
# Matched on the `exclude = [...]` line specifically, not anywhere in the file:
# the word `fuzz` appears in this repo's manifests in comments, and a lint that
# passes on a comment is the vacuous kind this repo has already shipped once.
if ! grep -E '^exclude = \[' Cargo.toml | grep -q '"fuzz"'; then
    echo "the root Cargo.toml's [workspace] exclude does not list \"fuzz\""
    echo "    Without it, cargo build --workspace pulls in #![no_main] targets"
    echo "    and just gate goes red for a reason unrelated to your change."
    violations=$((violations + 1))
fi

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-fuzz-targets: FAILED — $violations problem(s) (see above)"
    exit 1
fi

count=$(printf '%s\n' "$declared" | grep -c . || true)
echo "lint-fuzz-targets: clean — $count target(s) declared, sourced, seeded; fuzz/ excluded"
exit 0
