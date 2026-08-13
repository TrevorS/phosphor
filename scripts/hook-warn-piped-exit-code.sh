#!/usr/bin/env bash
# PreToolUse/Bash hook — warn when a verification command's exit status is
# thrown away by piping it into a filter.
#
# `cmd | tail` (or `head`/`grep`/`wc`) reports the *filter's* exit code, not
# the left-hand command's — a red `just gate` reads as green through `| tail`.
# This produced two false "green" reports in this session, one of them about
# a lint that was genuinely failing. `scripts/hook-block-fmt-all.sh` blocks a
# mistake that is always wrong; this one is not — `| head` to skim output is
# often exactly the right move — so this WARNS rather than denies, and only
# when the command gives no sign the exit code was already accounted for.
#
# Fires when a pipeline that starts with `just`, `cargo`, `bash scripts/…` or
# `./scripts/…` feeds `head`/`tail`/`grep`/`wc`, and the command nowhere
# mentions `PIPESTATUS`. Scoped to the pipeline the verification command
# actually starts — `;`/`&&`/`||` end a pipeline, so an unrelated later `|
# grep` in a compound command is not a false positive (mirrors
# `hook-block-fmt-all.sh`'s `[^;&|]*` scoping, one command over).
#
# Reads the PreToolUse payload on stdin and warns via `systemMessage`,
# leaving `permissionDecision` at `allow` so nothing is blocked. Anything
# else exits 0 silently and the command runs.
#
# Not named `lint-*.sh`: `just lint` globs that prefix, and this is a hook,
# not a lint.

set -euo pipefail

cmd="$(jq -r '.tool_input.command // ""')"

[ -n "$cmd" ] || exit 0

# The escape hatch: the author is already reading the piped-away exit code
# explicitly (`${PIPESTATUS[0]}`), so there is nothing left to warn about.
if printf '%s' "$cmd" | grep -q 'PIPESTATUS'; then
    exit 0
fi

# Split into pipeline segments on the separators that are not `|` itself:
# `&&`, `||`, `;`, and newlines. A pipeline of any length — `a | b | c` — stays
# one segment, because a later stage losing an earlier stage's status is the
# same bug at any depth.
segments="$(printf '%s\n' "$cmd" | awk '{ gsub(/&&/, "\n"); gsub(/\|\|/, "\n"); gsub(/;/, "\n"); print }')"

warn=0
while IFS= read -r segment; do
    [ -n "$segment" ] || continue
    # The segment has to *start* (after leading whitespace) with the
    # verification command being piped away — mentioning `just` earlier in an
    # unrelated compound command is not this bug.
    if printf '%s' "$segment" | grep -qE '^[[:space:]]*(just\>|cargo\>|bash[[:space:]]+scripts/|\./scripts/)'; then
        if printf '%s' "$segment" | grep -qE '\|[^|]*\<(head|tail|grep|wc)\>'; then
            warn=1
            break
        fi
    fi
done <<<"$segments"

if [ "$warn" -eq 1 ]; then
    jq -nc '{
        systemMessage: "Piping a verification command into head/tail/grep/wc reports the filter'\''s exit code, not the command'\''s — a red `just`/`cargo`/`scripts/` run can read as green through `| tail`. Add `set -o pipefail` before the pipeline, run the command bare and inspect its output separately, or read `${PIPESTATUS[0]}` explicitly.",
        hookSpecificOutput: {
            hookEventName: "PreToolUse",
            permissionDecision: "allow"
        }
    }'
    exit 0
fi

exit 0
