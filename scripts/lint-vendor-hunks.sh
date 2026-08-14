#!/usr/bin/env bash
# Structural lint: every file a fork diverges in is documented in its VENDOR.md.
#
# CLAUDE.md says: *"Every hunk under `vendor/` needs a matching entry in that
# fork's VENDOR.md — that is the acceptance contract, audited by
# `just vendor-diff`."* The first half is the rule this repo lives by. The second
# half was not true: `vendor-diff` **prints** the divergence and exits 0 whatever
# it contains. Its only failure modes are a missing fork and an unfetchable
# commit. A human reading 3,336 lines was the audit.
#
# That is the `surfaces.txt` defect again — a document describing a check that
# nothing performed — and it is worse here, because the thing being trusted is
# the seam that keeps a fork a fork. An undocumented hunk is how a fork silently
# becomes a rewrite, and nothing was watching for one.
#
# So this performs it. For each fork: ask `git` which files differ from the
# upstream tree we last merged, and require each to appear in that fork's
# VENDOR.md.
#
# WHAT COUNTS AS DOCUMENTED, and why it is three things rather than one. A
# VENDOR.md entry is prose for a human, so it names files the way prose does:
#   * by path — `src/editor.rs`, which is how the code hunks are written;
#   * by basename — `mermaid-image.gif`, which is how a list of deleted assets
#     reads when the directory is named in the heading above it;
#   * by directory — `examples/screenshots/`, which documents a whole removal
#     without listing six binaries twice.
# Requiring the full path everywhere would fail on documentation that is already
# clear, and this lint exists to catch an undocumented hunk, not to impose a
# citation format on a file people have to read.
#
# The directory form needs **two components at least** (`examples/screenshots/`,
# `src/phosphor/`), and that restriction is the whole difference between this
# lint working and not. The first version accepted any directory, so a file at
# `src/types.rs` was "documented" by the string `src/` — which appears in every
# other entry in the file. It passed on a planted violation, silently, for every
# source file in both forks. A one-component directory is not a group somebody
# documented; it is the word `src`.
#
# DELIBERATE LIMIT. This proves a file is *mentioned*, not that the mention is
# accurate or current. Nothing mechanical can check that a paragraph explains the
# diff beside it — that is what review is for, and `just vendor-diff` is still
# the command that shows you the diff to review. What this closes is the case
# nobody reviewed at all: a file that changed and was never written down.
#
# It also cannot see a fork that was never added, or one added without
# `git subtree` — `vendor_merged_sha` reads the `git-subtree-split` trailer the
# merge itself wrote, so a hand-copied directory has no upstream to diff against
# and fails loudly rather than passing vacuously.
#
# Exit 0 = every diverged file is documented, exit 1 = one is not.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ ! -f vendor/_vendor-lib.sh ]; then
    echo "lint-vendor-hunks: vendor/_vendor-lib.sh is missing — the fork seam moved" >&2
    exit 1
fi

# shellcheck source=/dev/null
source vendor/_vendor-lib.sh

status=0
checked=0
documented=0

for fork in "${VENDOR_FORKS[@]}"; do
    prefix="vendor/${fork}"
    manifest="${prefix}/VENDOR.md"

    if [ ! -d "$prefix" ]; then
        echo "lint-vendor-hunks: ${prefix} is not on disk"
        status=1
        continue
    fi

    if [ ! -f "$manifest" ]; then
        echo "lint-vendor-hunks: ${manifest} is missing — a fork with no provenance"
        status=1
        continue
    fi

    sha="$(vendor_merged_sha "$prefix")"
    vendor_ensure_commit "$fork" "$sha" >/dev/null 2>&1 || {
        echo "lint-vendor-hunks: cannot reach upstream ${sha:0:12} for ${fork}."
        echo "  A --squash subtree leaves that commit unreachable, so it may have been"
        echo "  garbage-collected. \`just vendor-diff ${fork}\` fetches it."
        status=1
        continue
    }

    changed="$(git diff --name-only "${sha}^{tree}" "$(vendor_worktree_tree "$prefix")")"

    while IFS= read -r file; do
        [ -n "$file" ] || continue
        # VENDOR.md is the documentation; it cannot be required to document
        # itself, and it diverges by construction because upstream has none.
        [ "$file" = "VENDOR.md" ] && continue

        checked=$((checked + 1))
        base="$(basename "$file")"
        dir="$(dirname "$file")"

        # `$dir` is only a group reference when it names one — two components
        # at least. See the header: `src/` is not documentation.
        if grep -qF -- "$file" "$manifest" ||
            grep -qF -- "$base" "$manifest" ||
            { [[ "$dir" == */* ]] && grep -qF -- "${dir}/" "$manifest"; }; then
            documented=$((documented + 1))
            continue
        fi

        echo "lint-vendor-hunks: ${prefix}/${file} diverges from upstream and"
        echo "  ${manifest} never mentions it."
        echo "  Every hunk under vendor/ is phosphor's to carry across every upstream"
        echo "  merge. One nobody wrote down is how a fork becomes a rewrite."
        echo "  See it with: just vendor-diff ${fork} -- ${file}"
        echo
        status=1
    done <<<"$changed"
done

if [ "$status" -ne 0 ]; then
    echo "lint-vendor-hunks: FAILED — ${checked} diverged files, $((checked - documented)) undocumented"
    exit 1
fi

echo "lint-vendor-hunks: clean — ${documented} diverged files across ${#VENDOR_FORKS[@]} forks, each documented"
