#!/usr/bin/env bash
# Fill the capture run's scratch `$XDG_STATE_HOME` from `V006`'s seed plan, so
# a `CP-5` screen has a store to draw.
#
# # Why this is separate from tape-env.sh
#
# `tape-env.sh` gives every capture an **empty** state home, which is what most
# of the library wants: those screens draw files under `tapes/fixtures/`, which
# no seed touches, and an empty store is the deterministic answer for them.
# Seeding costs about two dozen process launches, and paying that for forty
# tapes so that five can use it is the wrong trade.
#
# So this is opt-in and idempotent: a tape that needs the store sources
# `tape-env.sh` first (its runner already did) and then runs this.
#
# # Idempotent, and it has to be
#
# `declare-regions!` **appends**. Running the plan twice into one state home
# leaves twice the regions, which is a different screen — so this clears the
# workspace's journal before seeding rather than trusting it to be fresh. That
# is the same reasoning `scripts/seed-determinism.sh` uses to compare two runs
# honestly, and the property it proves is what makes this worth doing at all:
# same plan, same tree, same store, every time.
#
# # What it seeds
#
# `fixtures/`, the repository-root tree `V006` owns — not `tapes/fixtures/`,
# which is a separate six-file tree the older screens draw. A `CP-5` tape
# therefore opens files under `../fixtures`, which is also where the seed keys
# its store: seen-state keys on the canonicalised directory the editor started
# in, so the workspace root and the seeded root have to be the same directory
# or the store reads empty.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

if [[ -z "${XDG_STATE_HOME:-}" ]]; then
    echo "seed-state: \$XDG_STATE_HOME is unset — source tapes/tape-env.sh first." >&2
    exit 2
fi

if ! command -v phosphor >/dev/null 2>&1; then
    echo "seed-state: 'phosphor' is not on \$PATH — 'just install' puts it there." >&2
    exit 2
fi

# Clear only this run's store, never the operator's: `tape-env.sh` already
# pointed `$XDG_STATE_HOME` at a scratch directory, and refusing to touch
# anything outside it is what keeps that true if somebody sources this alone.
case "${XDG_STATE_HOME}" in
    "${TMPDIR:-/tmp}"*) rm -rf "${XDG_STATE_HOME:?}/phosphor" ;;
    *)
        echo "seed-state: \$XDG_STATE_HOME is not a scratch path (${XDG_STATE_HOME})." >&2
        echo "  Refusing to clear a store this script did not create." >&2
        exit 2
        ;;
esac
mkdir -p "${XDG_STATE_HOME}"

bash "$root/scripts/seed-fixtures.sh" >/dev/null 2>&1

# **Report what landed, and fail if nothing did.** A silent seed that seeded
# nothing produces a screen full of nothing, and a pixel diff would bless it —
# which is `lint-door-callers.sh`'s whole argument about scripts that call the
# door and cannot say whether it answered.
count="$(cd "$root/fixtures" && phosphor --eval '(unseen-count)' 2>&1 </dev/null)"
if [[ "$count" == "0" || -z "$count" ]]; then
    echo "seed-state: FAILED — the plan left no unseen regions (answered '${count}')." >&2
    echo "  Run 'bash scripts/seed-fixtures.sh' and read the store lines." >&2
    exit 1
fi

echo "seed-state: ${count} unseen region(s) in ${XDG_STATE_HOME}"
