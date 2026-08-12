#!/usr/bin/env bash
# T021 — structural lint: the Steel barrier holds.
#
# The Component Breakdown states the safety property the whole editor layer
# rests on, and states it as a *limit on reach*:
#
#   "Safety comes from the barrier, not from ceremony: Steel can only emit
#    Actions and read ViewModels, so live redefinition can misconfigure but
#    never corrupt a buffer."
#
# `phosphor-steel`'s own header says the same thing from the other end: it
# "never hands Steel a ratatui `Buffer` — a GC'd scheme with a `&mut Buffer` is
# the one thing that can tear a frame — so the dependency on `phosphor-core` is
# the whole surface area."
#
# Both sentences are prose today. This makes them mechanical, because the
# failure mode is silent: nothing breaks the day someone adds `ratatui-core` to
# `phosphor-steel/Cargo.toml` to "just render this one thing from scheme". The
# frame budget breaks three phases later, and by then the excavation is a
# rewrite rather than a refactor — which the Component Breakdown is explicit
# about.
#
# # What it checks
#
#   1. **The manifest.** `phosphor-steel` may depend on `phosphor-core` and
#      `steel-core` and nothing else. Q12's three layers put the primitives in
#      `phosphor-ui` and the *composition* here; composition needs no renderer.
#   2. **The source.** No `ratatui`, `ratatui_core`, `crossterm` or `Buffer`
#      anywhere under `src/` — the crates a drawing dependency would arrive as,
#      named directly in case one ever arrives transitively.
#   3. **The store.** No `phosphor_core::store`. The barrier is that Steel emits
#      Actions; a crate that can reach `Store` can mutate it without one, and
#      "can misconfigure but never corrupt" stops being true.
#
# Same grep-level tradeoff the other structural lints document: it skips lines
# whose trimmed text starts with `//` (this crate's own module docs quote the
# rule and must not trip it), and it cannot see through macros or re-exports.

set -euo pipefail

# Anchor to the repo root so the lint cannot silently pass from another cwd.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CRATE_DIR="crates/phosphor-steel"
MANIFEST="$CRATE_DIR/Cargo.toml"

# A missing subject is a failure, not a pass — this lint must never go green
# because the crate it guards was renamed out from under it.
if [ ! -d "$CRATE_DIR" ] || [ ! -f "$MANIFEST" ]; then
    echo "lint-the-steel-barrier: FAILED — $CRATE_DIR not found; this lint guards that crate, so its absence is an offence, not a clean run"
    exit 1
fi

violations=0

# --- 1. the manifest -------------------------------------------------------
#
# Read the `[dependencies]` table only: `[lints]`, `[package]` and any
# `[dev-dependencies]` are none of this lint's business.
allowed_deps=("phosphor-core" "steel-core" "steel-derive")

# Both spellings a dependency can take: `dep = { … }` and the dotted
# `dep.workspace = true` this manifest actually uses. Getting the second wrong
# is how a lint passes vacuously, which is why the count is checked afterwards.
found_deps=0
while IFS= read -r dep; do
    [ -z "$dep" ] && continue
    found_deps=$((found_deps + 1))
    ok=0
    for allowed in "${allowed_deps[@]}"; do
        [ "$dep" = "$allowed" ] && ok=1
    done
    if [ "$ok" -eq 0 ]; then
        echo "$MANIFEST: phosphor-steel depends on \`$dep\`"
        echo "    the Steel door's whole surface area is phosphor-core plus the VM (Q12); composition needs no renderer"
        violations=$((violations + 1))
    fi
done < <(awk '
    /^\[/ { in_deps = ($0 == "[dependencies]") ; next }
    in_deps && /^[A-Za-z0-9_-]+[[:space:].=]/ { sub(/[[:space:].=].*/, ""); print }
' "$MANIFEST")

# The crate has dependencies; a parser that found none has stopped reading the
# manifest rather than found it clean.
if [ "$found_deps" -eq 0 ]; then
    echo "$MANIFEST: no [dependencies] entries parsed — this lint has gone vacuous"
    violations=$((violations + 1))
fi

# --- 2. and 3. the source --------------------------------------------------
RULE_NAMES=(
    "ratatui (the app-layer crate)"
    "ratatui-core"
    "crossterm"
    "a ratatui Buffer"
    "the store"
)
RULE_PATTERNS=(
    '(^|[^_[:alnum:]])ratatui::'
    '(^|[^_[:alnum:]])ratatui_core'
    '(^|[^_[:alnum:]])crossterm'
    '(^|[^_[:alnum:]])Buffer([^_[:alnum:]]|$)'
    'phosphor_core::store'
)
RULE_ADVICE=(
    "Steel returns a view tree; Rust interprets it into ratatui calls (Q12)"
    "the primitives live in phosphor-ui and are composed here, never defined here (Q12)"
    "nothing in this crate touches a terminal; the binary owns the frame"
    "a GC'd scheme with a \`&mut Buffer\` is the one thing that can tear a frame (lib.rs)"
    "the barrier is that Steel emits Actions — reaching Store is how 'never corrupt a buffer' stops being true"
)

filelist="$(mktemp)"
trap 'rm -f "$filelist"' EXIT
find "$CRATE_DIR/src" -name '*.rs' -print0 > "$filelist"

while IFS= read -r -d '' file; do
    for i in "${!RULE_NAMES[@]}"; do
        name="${RULE_NAMES[$i]}"
        pattern="${RULE_PATTERNS[$i]}"
        advice="${RULE_ADVICE[$i]}"
        while IFS=: read -r lineno content; do
            [ -z "$lineno" ] && continue
            trimmed="$(printf '%s' "$content" | sed -e 's/^[[:space:]]*//')"
            case "$trimmed" in
                //*) continue ;; # comments describing the rule don't count
            esac
            echo "$file:$lineno: the Steel barrier is breached ($name) — $advice"
            echo "    ${trimmed}"
            violations=$((violations + 1))
        done < <(grep -nE -- "$pattern" "$file" || true)
    done
done < "$filelist"

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-the-steel-barrier: FAILED — $violations breach(es) of the Steel barrier (see above)"
    exit 1
fi

echo "lint-the-steel-barrier: clean — phosphor-steel reaches phosphor-core and the VM, and nothing else"
exit 0
