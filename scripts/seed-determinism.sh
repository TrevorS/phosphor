#!/usr/bin/env bash
# V006 / CP-5 — the fixed point: seeding twice from clean leaves the same store.
#
# `scripts/seed-fixtures.sh` reports what each line of the plan *answers*. It
# says so itself, in the paragraph this script exists to delete:
#
#   "WHAT IS STILL NOT ASSERTED: CP-5's fixed-point. This script reports what
#    each line answers; it does not check that running it twice leaves the same
#    store, or that two machines get identical output."
#
# That is the property `CP-5`'s tapes stand on. A capture of the unseen picker
# is evidence about the editor only if the store behind it is the same store
# every time it is made; otherwise a pixel diff is measuring the seed, and the
# checkpoint's own sweep asks for "identical output on two machines".
#
# WHAT THIS CAN AND CANNOT ANSWER. Two machines is not something one machine
# can check. What *is* checkable here — and is the part that would actually
# break — is whether the seed is a function of its input at all: same plan,
# same tree, two clean state homes, same observable store. A seed that varied
# run to run on one machine could never agree across two; one that is stable
# here has had the only cause this repository controls removed. The rest is
# `fixtures/README.md`'s residue item 8, and it is about clocks in subsystems
# that do not exist yet.
#
# NOT A LINT, for `seed-fixtures.sh`'s reason and one more. It needs `phosphor`
# on `$PATH`, and it runs the whole seed plan twice — two dozen process
# launches — which is not something to put in front of every `just lint`. Run
# it by hand, or before blessing a tape:
#
#     just seed-determinism
#
# `scripts/lint-door-callers.sh` holds it to the one rule a script outside the
# gate can still be held to: it must survive the door refusing, and must not
# match an answer shape the door stopped printing.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixtures="$root/fixtures"

if ! command -v phosphor >/dev/null 2>&1; then
    echo "seed-determinism: 'phosphor' is not on \$PATH."
    echo "  Put it there first:"
    echo "    cargo build --release --bin phosphor && \\"
    echo "      mkdir -p /tmp/phosphor-bin && \\"
    echo "      ln -sf \"\$PWD/target/release/phosphor\" /tmp/phosphor-bin/phosphor && \\"
    echo "      export PATH=\"/tmp/phosphor-bin:\$PATH\""
    exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# What "the store" means from outside the process: everything the seed plan can
# put there and a query can read back. Deliberately the *observable* store and
# not the journal bytes — a journal carries record ids and append order, which
# are implementation and would make this assert something narrower than it
# claims. Two stores that answer every query identically are the same store as
# far as anything drawing a screen is concerned.
queries=(
    '(unseen-count)'
    '(length (unseen-regions))'
    '(unseen-regions)'
)

# One clean seeded run. Answers land in $2, one query per line.
seed_and_observe() {
    local home="$1" out="$2"
    rm -rf "$home"
    mkdir -p "$home"
    # The seed plan itself, unchanged and through the same door. Its own output
    # is not what is under test here — what it *left behind* is — so it is
    # discarded, and a failure to run at all is caught by the emptiness check
    # below rather than by trusting its exit code.
    XDG_STATE_HOME="$home" bash "$root/scripts/seed-fixtures.sh" >/dev/null 2>&1

    : >"$out"
    local query answer
    for query in "${queries[@]}"; do
        # `cd` into the fixture root for `seed-fixtures.sh`'s reason: the store
        # keys on the canonicalised directory the editor started in, so a query
        # run from the repo root reads a different — empty — workspace.
        if answer="$(cd "$fixtures" && XDG_STATE_HOME="$home" phosphor --eval "$query" 2>&1 </dev/null)"; then
            printf '%s -> %s\n' "$query" "$answer" >>"$out"
        else
            printf '%s -> FAILED TO RUN\n' "$query" >>"$out"
        fi
    done
}

echo "seed-determinism: seeding two clean state homes and comparing what they answer."
seed_and_observe "$work/a" "$work/answers-a"
seed_and_observe "$work/b" "$work/answers-b"

# **The seed has to have done something.** Two empty stores agree perfectly and
# prove nothing, which is exactly how this check would rot the day the plan
# stops landing anything — see `lint-door-callers.sh`'s own reason for existing.
if grep -q '^(unseen-count) -> 0$' "$work/answers-a"; then
    echo
    echo "seed-determinism: FAILED — the seed left an empty store, so agreement means nothing."
    echo "  Run 'bash scripts/seed-fixtures.sh' and read what the store lines answered."
    cat "$work/answers-a"
    exit 1
fi

if diff -u "$work/answers-a" "$work/answers-b" >"$work/diff"; then
    echo
    echo "seed-determinism: the two runs agree."
    sed 's/^/  /' "$work/answers-a"
    echo
    echo "  Same plan, same tree, two clean state homes, same answers. That removes the"
    echo "  only cause of tape drift this repository controls; the clock question for the"
    echo "  session subsystems is fixtures/README.md's residue item 8, and they do not exist yet."
    exit 0
fi

echo
echo "seed-determinism: FAILED — two clean runs of the same plan left different stores."
echo "  A tape captured against this seed is measuring the seed, not the editor."
sed 's/^/  /' "$work/diff"
exit 1
