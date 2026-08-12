#!/usr/bin/env bash
# Run the fork provenance checks as part of `just lint`, and therefore CI.
#
# `just vendor-check` verifies two things about each fork: that the SHA its
# VENDOR.md records is the SHA git history actually merged, and that the licence
# that file claims is the licence the fork's own Cargo.toml declares. Both are
# checks of *prose against the tree*, which is the category this repo has been
# worst at — the licence check exists because a VENDOR.md once invented a
# licence crisis out of nothing and every gate passed it.
#
# It lives here rather than as a sixth CI job because `scripts/lint-*.sh` is the
# seam: `just lint` globs it, CI calls `just lint`, and adding a check therefore
# never touches the workflow or the justfile. This is a thin delegation — the
# implementation stays in `vendor/vendor.just`, so there is one copy of it and
# `just vendor-check` on its own keeps working for humans.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v just >/dev/null 2>&1; then
    echo "lint-vendor-provenance: FAILED — just not found; cannot run vendor-check"
    exit 1
fi

# Half of what vendor-check proves lives in git history — each fork's
# `git-subtree-split` trailer, which is how the recorded SHA is verified against
# what was actually merged. A shallow clone has no such commit, and the failure
# it produces ("no git-subtree-split trailer") reads like a broken fork rather
# than a truncated clone. This is not hypothetical: the first CI run of this
# lint failed exactly that way, because `actions/checkout` is shallow by
# default. Say so plainly instead.
if [ "$(git rev-parse --is-shallow-repository)" = "true" ]; then
    echo "lint-vendor-provenance: FAILED — this is a shallow clone, so the subtree"
    echo "    merge commits are absent and provenance cannot be verified at all."
    echo "    CI: give this job 'fetch-depth: 0'. Locally: git fetch --unshallow"
    exit 1
fi

if just vendor-check; then
    echo "lint-vendor-provenance: clean — both forks' SHA and licence claims match the tree"
    exit 0
fi

echo
echo "lint-vendor-provenance: FAILED — a VENDOR.md describes something the tree contradicts (see above)"
exit 1
