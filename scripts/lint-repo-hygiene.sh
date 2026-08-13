#!/usr/bin/env bash
# Repo hygiene — the three ways junk actually got into this repository.
#
# Not a general-purpose tidiness lint. Every check below exists because the
# thing it catches already happened here and nothing noticed:
#
#   1. **A 7 MB demo GIF** arrived inside `vendor/ratatui-markdown` on a
#      `git subtree add` and sat there for two windows — 80% of the packed
#      object store, for a fork whose real content is a one-line version bump.
#      Removing it needed a history rewrite, which is cheap at 12 commits and
#      expensive later. A size ceiling catches it at the moment it lands.
#   2. **Six byte-identical reference captures.** `tapes/artifacts/9c.png` is
#      the same bytes as `1a.png`, so it does not show `9c`'s anchored region
#      at all — and `V007`'s pixel-diff runner would report it regenerating
#      identically forever while the screen it claims to prove was never
#      captured. A duplicate reference image is a *correctness* bug, not waste.
#   3. **Seven `refs/jj/keep/*` orphans** survived deleting the jj repo,
#      invisible to `git status`, `git branch` and `git worktree list`, pinning
#      old commits so `git gc` silently reclaimed nothing.
#
# Checks 1 and 2 carry allowlists, on the `just vendor-diff` principle:
# divergence is allowed, undocumented divergence is not. A new offender fails;
# a known one has a line saying why.
#
# Checks 1 and 2 also run over untracked-but-not-ignored files (`git ls-files
# --others --exclude-standard`), not just `git ls-files`. That is not
# hypothetical either: the six byte-identical captures in item 2 above passed
# a `just gate` run clean *before* `git add` — `git ls-files` cannot see a
# file that has not been staged, so a gate run on the working tree ahead of a
# commit proved nothing about what the commit was about to contain, and
# reddened the instant the files landed. The tracked-file checks below are
# unchanged; each has an untracked-file counterpart alongside it.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# A tracked file bigger than this fails. The largest legitimate file today is a
# ~250 KB reference PNG, so this leaves a lot of headroom and still catches the
# class of thing that motivated it by an order of magnitude.
MAX_TRACKED_BYTES=$((1024 * 1024))

ALLOW_LARGE="scripts/allow-large-files.txt"
ALLOW_DUPES="tapes/artifacts/DUPLICATES.md"

violations=0

# ── 1 · no oversized tracked files ──────────────────────────────────────────
while IFS= read -r file; do
    [ -f "$file" ] || continue
    size=$(wc -c <"$file" | tr -d ' ')
    [ "$size" -le "$MAX_TRACKED_BYTES" ] && continue
    if [ -f "$ALLOW_LARGE" ] && grep -qxF "$file" "$ALLOW_LARGE"; then
        continue
    fi
    printf '%s: %s bytes — over the %s-byte ceiling for a tracked file\n' \
        "$file" "$size" "$MAX_TRACKED_BYTES"
    echo "    If it belongs here, add its exact path to $ALLOW_LARGE with a comment saying why."
    violations=$((violations + 1))
done < <(git ls-files)

# ── 1b · no oversized untracked-but-not-ignored files ───────────────────────
# Same ceiling, same allowlist — the only difference from check 1 is the file
# source, so a large file cannot dodge this by arriving one `git add` early.
while IFS= read -r file; do
    [ -f "$file" ] || continue
    size=$(wc -c <"$file" | tr -d ' ')
    [ "$size" -le "$MAX_TRACKED_BYTES" ] && continue
    if [ -f "$ALLOW_LARGE" ] && grep -qxF "$file" "$ALLOW_LARGE"; then
        continue
    fi
    printf '%s: %s bytes — over the %s-byte ceiling for a tracked file (untracked, not yet added)\n' \
        "$file" "$size" "$MAX_TRACKED_BYTES"
    echo "    Not tracked yet, but \`git add\` would make it so. If it belongs here, add its exact path to $ALLOW_LARGE; if not, delete it before it lands."
    violations=$((violations + 1))
done < <(git ls-files --others --exclude-standard)

# ── 2 · no undocumented byte-identical reference captures ───────────────────
# Scoped to the committed reference library: elsewhere identical files are
# merely redundant, but here they are a reference that proves the wrong screen.
# The hash corpus is tracked *and* untracked-but-not-ignored captures together
# — a strict superset of the tracked-only corpus, so every duplicate this
# caught before it still catches identically, and a duplicate one side of
# which has not been `git add`ed yet is no longer invisible to it.
if [ -d tapes/artifacts ]; then
    dupes="$(
        { git ls-files 'tapes/artifacts/*.png'; git ls-files --others --exclude-standard 'tapes/artifacts/*.png'; } |
            while IFS= read -r f; do
                [ -f "$f" ] || continue
                printf '%s  %s\n' "$(git hash-object "$f")" "$f"
            done |
            sort |
            awk '{ if ($1 == prev) { if (!shown[$1]++) print prevline; print $2 } prev = $1; prevline = $2 }'
    )"
    if [ -n "$dupes" ]; then
        undocumented=""
        while IFS= read -r f; do
            [ -z "$f" ] && continue
            base="$(basename "$f")"
            if [ -f "$ALLOW_DUPES" ] && grep -qF "$base" "$ALLOW_DUPES"; then
                continue
            fi
            undocumented="${undocumented}${f}"$'\n'
        done < <(printf '%s\n' "$dupes")
        if [ -n "${undocumented//[$'\n' ]/}" ]; then
            echo "byte-identical reference captures with no entry in $ALLOW_DUPES:"
            printf '%s' "$undocumented" | sed 's/^/    /'
            echo "    A duplicate reference proves the wrong screen. Record it there, or recapture."
            violations=$((violations + 1))
        fi
    fi
fi

# ── 3 · no stray ref namespaces ─────────────────────────────────────────────
# `git status` is clean while these exist, which is exactly why they survived.
stray="$(git for-each-ref --format='%(refname)' |
    grep -Ev '^refs/(heads|remotes|tags|stash|notes)/' || true)"
if [ -n "$stray" ]; then
    echo "refs outside the expected namespaces — these pin objects and block gc:"
    printf '%s\n' "$stray" | sed 's/^/    /'
    echo "    Delete with: git for-each-ref --format='%(refname)' <namespace> | xargs -n1 git update-ref -d"
    violations=$((violations + 1))
fi

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-repo-hygiene: FAILED — $violations hygiene violation(s) (see above)"
    exit 1
fi

echo "lint-repo-hygiene: clean — no oversized files (tracked or untracked), no undocumented duplicate captures (tracked or untracked), no stray refs"
exit 0
