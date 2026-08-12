# Vendored-fork recipes (plan §2). Imported by the root justfile via
# `import? 'vendor/vendor.just'` — this file is `surface`'s, the justfile is
# `harness`'s, and the seam between them is that one optional import.
#
# Two forks live here, both added by `git subtree ... --squash`:
#
#   vendor/ratatui-code-editor   the BufferView core (T003)
#   vendor/ratatui-markdown      transcript prose, bumped to ratatui 0.30 (T004)
#
# Provenance for each is in its own VENDOR.md. The last-merged upstream SHA is
# NOT read from those files — it is read from the `git-subtree-split` trailer
# `git subtree` writes into history, so it cannot drift from what was actually
# merged. VENDOR.md carries the same SHA for humans, and `just vendor-check`
# proves the two agree.

# Divergence of a vendored fork from the upstream commit we last merged.
# `just vendor-diff` covers both forks; `just vendor-diff ratatui-markdown` one.
# Any git-diff flag passed after the name changes the shape:
#     just vendor-diff ratatui-code-editor --stat
#
# THIS IS THE ONE COMMAND THAT KEEPS A FORK A FORK. Everything it prints is
# phosphor's to carry forever and must have an entry in that fork's VENDOR.md.
# Anything it prints that VENDOR.md does not explain is the fork quietly
# becoming a rewrite.

# Show a fork's divergence from the upstream SHA it was merged at.
vendor-diff *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    source vendor/_vendor-lib.sh
    forks=()
    flags=()
    for a in {{ args }}; do
        case "$a" in
            -*) flags+=("$a") ;;
            *)  forks+=("$a") ;;
        esac
    done
    [ ${#forks[@]} -eq 0 ] && forks=("${VENDOR_FORKS[@]}")
    for fork in "${forks[@]}"; do
        prefix="vendor/${fork}"
        vendor_require_fork "$fork"
        sha="$(vendor_merged_sha "$prefix")"
        vendor_ensure_commit "$fork" "$sha"
        echo "══ ${prefix} — divergence from upstream ${sha:0:12} ══"
        git --no-pager diff ${flags[@]+"${flags[@]}"} \
            "${sha}^{tree}" "$(vendor_worktree_tree "$prefix")"
        echo
    done

# Merge a new upstream revision into a fork. <ref> is a tag, branch, or SHA —
# use a SHA when upstream has not tagged the release you need (`ratatui-code-editor`
# publishes 0.0.6 on crates.io with no v0.0.6 tag; see its VENDOR.md).
#
#     just vendor-pull ratatui-markdown v0.3.7
#
# Requires a clean working tree. Afterwards: re-run `just vendor-diff`, confirm
# every surviving hunk still has a VENDOR.md entry, and update that file's
# "last merged" line to the new SHA — `just vendor-check` will fail until you do.

# Merge new upstream into a fork — `just vendor-pull <fork> <ref>`.
vendor-pull fork ref:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    source vendor/_vendor-lib.sh
    vendor_require_fork "{{ fork }}"
    git subtree pull --prefix "vendor/{{ fork }}" "$(vendor_url "{{ fork }}")" "{{ ref }}" --squash
    echo
    echo "Merged. Now: just vendor-diff {{ fork }} — then update vendor/{{ fork }}/VENDOR.md."

# Both sides of the clipboard gate. `arboard` links X11/Wayland on Linux and a bare
# container has neither, so the default workspace build must not contain it — and
# the check is that it is absent from `cargo tree`, not merely unused.
#
# This is the half of `T003`'s acceptance criterion that is provable without a
# container. The other half — that the build actually completes on a machine with
# no X11 and no Wayland — needs a Linux container and has NOT been run.

# Prove arboard is out of the default graph and returns with --features clipboard.
vendor-build-headless:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    cargo build --workspace
    if cargo tree --workspace -e normal | grep -q arboard; then
        echo "FAIL: arboard is in the default workspace graph" >&2
        exit 1
    fi
    echo "ok — default workspace build does not link arboard"
    cargo build -p phosphor-buffer --features clipboard
    if ! cargo tree -p phosphor-buffer -e normal --features clipboard | grep -q arboard; then
        echo "FAIL: --features clipboard did not bring arboard back — the gate is dead" >&2
        exit 1
    fi
    echo "ok — --features clipboard builds and does link arboard"

# Does each VENDOR.md name the SHA that history says was actually merged? Cheap,
# and it is the one way the provenance file can silently go stale.

# Check each VENDOR.md's recorded SHA against git history.
vendor-check:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    source vendor/_vendor-lib.sh
    status=0
    for fork in "${VENDOR_FORKS[@]}"; do
        prefix="vendor/${fork}"
        sha="$(vendor_merged_sha "$prefix")"
        if grep -q "$sha" "${prefix}/VENDOR.md"; then
            echo "ok   ${fork} — VENDOR.md records ${sha:0:12}"
        else
            echo "FAIL ${fork} — history merged ${sha}, VENDOR.md does not mention it"
            status=1
        fi
    done
    exit "$status"
