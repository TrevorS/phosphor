#!/usr/bin/env bash
# T019 — structural lint: no Action construction from `phosphor-ui`.
#
# `T007` closed the obvious half of invariant §0: a widget cannot reach
# `phosphor_core::store`, so it cannot *apply* a mutation. This closes the other
# half. `T019` put the whole mutation vocabulary in `phosphor_core::action`, and
# a widget that can construct an `Action` is one refactor away from applying one
# — "widgets cannot mutate" would have quietly become "widgets can build the
# mutation but must hand it to someone else", which is a convention, not a
# boundary.
#
# The split this enforces, and it is the reason `T019` put payload types in
# their own module:
#
#   phosphor_core::vm      — ViewModels. What a widget renders.          ALLOWED
#   phosphor_core::view    — the view tree. What a widget interprets.    ALLOWED
#   phosphor_core::request — Position, Span, ScrollRequest, EditMode.    ALLOWED
#                            A widget legitimately *names* these in the
#                            ViewModel it takes; naming a place is not
#                            asking for it to change.
#   phosphor_core::value   — the wire model. Not forbidden, not needed.  ALLOWED
#   phosphor_core::action  — the mutation vocabulary.                    FORBIDDEN
#   phosphor_core::store   — the store itself (T007's lint).             FORBIDDEN
#
# Input maps key → Action, and input is the binary's (`TEAM.md`: *"spine
# decides, surface draws"*). A widget that wants something to happen exposes it
# in a ViewModel and lets the app layer decide — that is what keeps `T079`'s
# tree interpreter a pure function of the tree.
#
# Deliberate limits, so nobody trusts this further than it goes: it strips
# comments before matching, so a doc comment naming the forbidden path is not a
# violation. It cannot see through macros, `include!`, or a path assembled at the
# token level. It does not need to re-check aliased (`use phosphor_core as pc;`)
# or glob (`use phosphor_core::*;`) imports, because `lint-no-store-mutation.sh`
# already fails on either of those in a UI crate outright — that is why this
# script is short.
#
# Exit 0 = clean, exit 1 = at least one offence, printed with file:line.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

UI_CRATES=("crates/phosphor-ui")
CORE_LIB="crates/phosphor-core/src/lib.rs"

violations=0
files_checked=0

offence() {
    echo "$1: $2"
    if [ -n "${3:-}" ]; then
        printf '    %s\n' "$(printf '%s' "$3" | sed -e 's/^[[:space:]]*//')"
    fi
    violations=$((violations + 1))
}

# Blank out comments, preserving line numbering (same helper as T007's lint).
strip_comments() {
    perl -0777 -pe 's{/\*.*?\*/}{ $& =~ tr/\n//cdr }gse; s{//[^\n]*}{}g' "$1"
}

RULE_NAMES=(
    "imports the Action vocabulary (phosphor_core::action) — widgets read ViewModels and the view tree; building a mutation is the app layer's job (T019, invariant §0)"
    "pulls action out of a phosphor_core::{...} group — import vm, view or request instead (T019)"
)
RULE_PATTERNS=(
    'phosphor_core[[:space:]]*::[[:space:]]*(r#)?action\b'
    'phosphor_core[[:space:]]*::[[:space:]]*\{[^}]*[[:space:]{,]+(r#)?action\b'
)

for crate_dir in "${UI_CRATES[@]}"; do
    [ -d "$crate_dir" ] || continue
    while IFS= read -r -d '' file; do
        files_checked=$((files_checked + 1))
        stripped="$(strip_comments "$file")"
        flattened="$(printf '%s' "$stripped" | tr '\n' ' ')"
        for i in "${!RULE_PATTERNS[@]}"; do
            hits=0
            while IFS=: read -r lineno content; do
                [ -z "$lineno" ] && continue
                hits=$((hits + 1))
                offence "$file:$lineno" "${RULE_NAMES[$i]}" "$content"
            done < <(printf '%s\n' "$stripped" | grep -nE -- "${RULE_PATTERNS[$i]}" || true)
            if [ "$hits" -eq 0 ] && printf '%s' "$flattened" | grep -qE -- "${RULE_PATTERNS[$i]}"; then
                offence "$file" "${RULE_NAMES[$i]} [spelled across lines]"
            fi
        done
    done < <(find "$crate_dir" -name '*.rs' -print0 | sort -z)
done

# The crate root must not re-export the vocabulary either: `pub use action::*;`
# would put `Action` at `phosphor_core::Action`, where the checks above would
# never see it. Same hole T007 closes for the store.
if [ ! -f "$CORE_LIB" ]; then
    offence "$CORE_LIB" "missing — this lint guards a module split that is not there"
else
    core_lib_stripped="$(strip_comments "$CORE_LIB")"
    if ! printf '%s' "$core_lib_stripped" | grep -qE "^[[:space:]]*pub[[:space:]]+mod[[:space:]]+action[[:space:]]*;"; then
        offence "$CORE_LIB" "does not declare 'pub mod action;' — T019's vocabulary is what this lint guards (T019)"
    fi
    REEXPORT_PATTERNS=(
        'pub[[:space:]]+use[[:space:]]+((::)?(crate|self)[[:space:]]*::[[:space:]]*)?(r#)?action[[:space:]]*(::|;| as )'
        'pub[[:space:]]+use[[:space:]]+[^;]*\{[^}]*[[:space:]{,]+(r#)?action\b'
    )
    for pattern in "${REEXPORT_PATTERNS[@]}"; do
        while IFS=: read -r lineno content; do
            [ -z "$lineno" ] && continue
            offence "$CORE_LIB:$lineno" \
                "re-exports the Action vocabulary at the crate root — that puts Action where a widget can reach it without naming action (T019)" \
                "$content"
        done < <(printf '%s\n' "$core_lib_stripped" | grep -nE -- "$pattern" || true)
    done
fi

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-no-action-in-ui: FAILED — $violations violation(s)."
    echo "  A widget renders a ViewModel (phosphor_core::vm) over a view tree"
    echo "  (phosphor_core::view), naming places with phosphor_core::request."
    echo "  Key → Action is the input machine's, and applying one is the binary's"
    echo "  (T019, T026, invariant §0)."
    exit 1
fi

echo "lint-no-action-in-ui: OK — $files_checked file(s) in ${UI_CRATES[*]} construct no Action, vocabulary not re-exported"
