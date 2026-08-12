# Shared shell for vendor/vendor.just. Sourced, never executed directly.
# Lives next to the recipes rather than inside them so the two forks are named
# in exactly one place.

# The forks, and where each came from. `vendor-diff` fetches from these URLs when
# the merged commit is missing from the local object store — a `--squash` subtree
# leaves that commit unreachable, so `git gc` is allowed to drop it.
VENDOR_FORKS=(ratatui-code-editor ratatui-markdown)

vendor_url() {
    case "$1" in
        ratatui-code-editor) echo "https://github.com/vipmax/ratatui-code-editor.git" ;;
        ratatui-markdown)    echo "https://github.com/celestia-island/ratatui-markdown" ;;
        *) echo "vendor: unknown fork: $1" >&2; return 1 ;;
    esac
}

vendor_require_fork() {
    local fork="$1"
    for known in "${VENDOR_FORKS[@]}"; do
        [ "$fork" = "$known" ] && { [ -d "vendor/${fork}" ] && return 0; }
    done
    {
        echo "vendor: unknown fork: ${fork}"
        echo "  known: ${VENDOR_FORKS[*]}"
    } >&2
    return 1
}

# The upstream SHA `git subtree` recorded at the last add/pull for this prefix.
# Authoritative: it is written by the merge itself, not maintained by hand.
vendor_merged_sha() {
    local prefix="$1" sha
    sha="$(git log --format='%b' --grep="^git-subtree-dir: ${prefix}\$" \
           | awk '/^git-subtree-split:/ { print $2; exit }')"
    if [ -z "$sha" ]; then
        echo "vendor: no git-subtree-split trailer for ${prefix}" >&2
        echo "  (was it added with \`git subtree add\`?)" >&2
        return 1
    fi
    echo "$sha"
}

# A tree object for the fork **as it is on disk**, committed or not.
#
# Uncommitted divergence is still divergence — a vendor-diff that only compared
# HEAD would go quiet exactly while someone was editing the fork, which is the
# one moment it needs to speak up. Built in a scratch index so the real one is
# untouched, and `git add` still honours .gitignore, so build output stays out.
vendor_worktree_tree() {
    local prefix="$1" idx root tree
    root="$(git rev-parse --show-toplevel)"
    idx="$(mktemp -t phosphor-vendor-index.XXXXXX)"
    trap 'rm -f "$idx"' RETURN
    GIT_INDEX_FILE="$idx" git -C "$root" read-tree HEAD
    GIT_INDEX_FILE="$idx" git -C "$root" add -A -- "$prefix"
    tree="$(GIT_INDEX_FILE="$idx" git -C "$root" write-tree)"
    git rev-parse "${tree}:${prefix}"
}

vendor_ensure_commit() {
    local fork="$1" sha="$2"
    git cat-file -e "${sha}^{commit}" 2>/dev/null && return 0
    echo "── fetching upstream ${sha:0:12} for ${fork} ──"
    git fetch --quiet "$(vendor_url "$fork")" "$sha"
}
