#!/usr/bin/env bash
# T007 — structural lint: no store mutation from `phosphor-ui`.
#
# Invariant §0: "phosphor-ui must not be able to mutate. Widgets read ViewModels;
# input maps key -> Action; Actions mutate the store; the store re-derives."
# The plan asks for that to be enforced by *dependency direction* rather than
# convention (IMPLEMENTATION-PLAN.md:210-213, Q12): `phosphor_core::vm` and
# `phosphor_core::view` are the crate's public face to the UI; `phosphor_core::store`
# is not.
#
# Rust has no way to say "this module is public, but not to that crate", so the
# direction is checked here instead. Three checks, in increasing order of how
# easy the hole would be to miss:
#
#   1. No reference to `phosphor_core::store` from a UI-side crate — including
#      `use phosphor_core::store as _;`, `use phosphor_core::{vm, store};` split
#      across lines by rustfmt, an aliased `use phosphor_core as pc;` (which would
#      make every later path unrecognisable), and a `use phosphor_core::*;` glob
#      (which would put `store` in scope with no `phosphor_core::` prefix left to
#      grep for).
#   2. Dependency direction in the manifests: `phosphor-core` declares no ratatui
#      and no Steel dependency (Q12 verbatim — the view tree is plain data, so
#      neither side can come to own the protocol), and `phosphor-ui`'s only
#      `phosphor-*` dependency is `phosphor-core`. The second one closes the
#      transitive door check 1 cannot see: a UI crate that depended on
#      `phosphor-steel` could dispatch an Action, and a UI crate that renamed its
#      core dependency could spell the forbidden path under another name.
#   3. `phosphor-core`'s crate root does not re-export `store`. `pub use store::*;`
#      in lib.rs would put the mutation API at `phosphor_core::Store` — check 1
#      would pass, the module split would still be there, and it would mean
#      nothing. Same check verifies the split still exists at all, so this lint
#      can never pass because its subject was deleted.
#
# Deliberate limits, so nobody trusts this further than it goes: it strips
# comments before matching (a doc comment naming the forbidden path is not a
# violation — this crate's own lib.rs header names it) but it cannot see through
# macros, `include!`, or a path assembled at the token level. Those are hostile,
# not accidental; this lint is aimed at the honest mistake and the honest
# refactor. Transitive *third-party* duplication is cargo-deny's job (deny.toml).
#
# Exit 0 = clean, exit 1 = at least one offence, printed with file:line.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# UI-side crates: the ones that draw, and therefore the ones that must only read.
# One entry today; a second UI crate would be added here and nowhere else.
UI_CRATES=("crates/phosphor-ui")

CORE_MANIFEST="crates/phosphor-core/Cargo.toml"
CORE_LIB="crates/phosphor-core/src/lib.rs"
CORE_SRC="crates/phosphor-core/src"

violations=0

offence() {
    # $1 = location (file or file:line), $2 = what, $3 = the offending text (optional)
    echo "$1: $2"
    if [ -n "${3:-}" ]; then
        printf '    %s\n' "$(printf '%s' "$3" | sed -e 's/^[[:space:]]*//')"
    fi
    violations=$((violations + 1))
}

# Blank out comments, preserving line numbering: block comments keep their
# newlines, line comments keep theirs. Side effect: a `//` inside a string
# literal blanks the rest of that line, which can only ever make this lint
# quieter, never louder.
strip_comments() {
    perl -0777 -pe 's{/\*.*?\*/}{ $& =~ tr/\n//cdr }gse; s{//[^\n]*}{}g' "$1"
}

# ---------------------------------------------------------------------------
# Check 1 — no `phosphor_core::store` reachable from a UI-side crate.
# ---------------------------------------------------------------------------

RULE_NAMES=(
    "imports the store (phosphor_core::store) — phosphor-ui reads ViewModels, it does not mutate (T007, invariant §0)"
    "pulls store out of a phosphor_core::{...} group — phosphor-ui may import vm and view only (T007, Q12)"
    "renames the phosphor_core crate on import — spell the path literally, an alias hides store:: from this lint (T007)"
    "glob-imports phosphor_core — that puts store in scope unqualified; import vm and view by name (T007)"
)
RULE_PATTERNS=(
    'phosphor_core[[:space:]]*::[[:space:]]*(r#)?store\b'
    'phosphor_core[[:space:]]*::[[:space:]]*\{[^}]*[[:space:]{,]+(r#)?store\b'
    '(use|extern[[:space:]]+crate)[[:space:]]+(::)?phosphor_core[[:space:]]+as[[:space:]]'
    'phosphor_core[[:space:]]*::[[:space:]]*\*'
)

files_checked=0

for crate_dir in "${UI_CRATES[@]}"; do
    [ -d "$crate_dir" ] || continue
    while IFS= read -r -d '' file; do
        files_checked=$((files_checked + 1))
        stripped="$(strip_comments "$file")"
        # One pass per rule over the numbered lines, then one over the same text
        # with newlines flattened — that second pass is what catches the perfectly
        # ordinary rustfmt-wrapped `use phosphor_core::{\n    vm::X,\n    store::Y,\n};`.
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

# ---------------------------------------------------------------------------
# Check 2 — dependency direction in the manifests.
# ---------------------------------------------------------------------------

# Print "<crate>|<line>|<text>" for every entry in a dependency table. Good enough
# TOML for manifests that declare one dependency per line, and it handles the
# three spellings these manifests actually use:
#   ratatui-core = "0.1.2"                    plain key
#   phosphor-core.workspace = true            dotted key — crate is the first segment
#   [target.'cfg(unix)'.dependencies]         qualified table
# plus `[dependencies.foo]`, where the crate name is in the header rather than in
# any key. Inner keys of a multi-line inline table leak through as false keys
# (version, features, workspace); none of them match the names checked below.
manifest_deps() {
    awk '
        /^[[:space:]]*\[/ {
            table = $0
            sub(/#.*/, "", table)
            gsub(/[][[:space:]"'"'"']/, "", table)
            in_deps = 0
            if (table ~ /(^|\.)(dependencies|dev-dependencies|build-dependencies)$/) {
                in_deps = 1
            } else if (match(table, /(^|\.)(dependencies|dev-dependencies|build-dependencies)\./)) {
                name = substr(table, RSTART + RLENGTH)
                sub(/\..*/, "", name)
                print name "|" NR "|" $0
            }
            next
        }
        in_deps && /^[[:space:]]*["A-Za-z0-9_.-]+[[:space:]]*=/ {
            key = $0
            sub(/[[:space:]]*=.*/, "", key)
            gsub(/[[:space:]"]/, "", key)
            sub(/\..*/, "", key)
            print key "|" NR "|" $0
        }
    ' "$1"
}

if [ -f "$CORE_MANIFEST" ]; then
    while IFS='|' read -r key lineno text; do
        [ -z "$key" ] && continue
        if printf '%s' "$key" | grep -qE '^(ratatui|steel)([-_][A-Za-z0-9_-]+)?$'; then
            offence "$CORE_MANIFEST:$lineno" \
                "phosphor-core depends on '$key' — the view tree carries neither a Steel nor a ratatui dependency, so neither side owns the protocol (Q12, T078)" \
                "$text"
        fi
    done < <(manifest_deps "$CORE_MANIFEST")
fi

for crate_dir in "${UI_CRATES[@]}"; do
    manifest="$crate_dir/Cargo.toml"
    [ -f "$manifest" ] || continue
    while IFS='|' read -r key lineno text; do
        [ -z "$key" ] && continue
        case "$key" in
            phosphor-core) ;;
            phosphor-*)
                offence "$manifest:$lineno" \
                    "$crate_dir depends on '$key' — a UI crate reaches the core through phosphor-core::{vm,view} and nothing else; any other phosphor crate is a mutation path this lint cannot see (T007)" \
                    "$text"
                ;;
        esac
    done < <(manifest_deps "$manifest")
    # A renamed dependency (`core = { package = "phosphor-core" }`) would let the
    # forbidden path be spelled `core::store::…`, which check 1 would never see.
    while IFS=: read -r lineno content; do
        [ -z "$lineno" ] && continue
        offence "$manifest:$lineno" \
            "phosphor-core is renamed on import — keep the crate name literal so the store path stays greppable (T007)" \
            "$content"
    done < <(grep -nE 'package[[:space:]]*=[[:space:]]*"phosphor-core"' "$manifest" || true)
done

# ---------------------------------------------------------------------------
# Check 3 — the split itself: still there, and not re-exported around.
# ---------------------------------------------------------------------------

if [ ! -f "$CORE_LIB" ]; then
    offence "$CORE_LIB" "missing — T007's module split is the thing this lint guards; without it the lint means nothing"
else
    core_lib_stripped="$(strip_comments "$CORE_LIB")"
    for module in store vm view; do
        if [ ! -f "$CORE_SRC/$module.rs" ] && [ ! -d "$CORE_SRC/$module" ]; then
            offence "$CORE_SRC/$module.rs" "missing — phosphor-core's vm/view/store split is what makes 'the UI cannot mutate' structural (T007)"
        fi
        if ! printf '%s' "$core_lib_stripped" | grep -qE "^[[:space:]]*pub[[:space:]]+mod[[:space:]]+$module[[:space:]]*;"; then
            offence "$CORE_LIB" "does not declare 'pub mod $module;' — the vm/view/store split is T007's whole mechanism (T007)"
        fi
    done

    REEXPORT_PATTERNS=(
        'pub[[:space:]]+use[[:space:]]+((::)?(crate|self)[[:space:]]*::[[:space:]]*)?(r#)?store[[:space:]]*(::|;| as )'
        'pub[[:space:]]+use[[:space:]]+[^;]*\{[^}]*[[:space:]{,]+(r#)?store\b'
    )
    for pattern in "${REEXPORT_PATTERNS[@]}"; do
        while IFS=: read -r lineno content; do
            [ -z "$lineno" ] && continue
            offence "$CORE_LIB:$lineno" \
                "re-exports the store at the crate root — that puts the mutation API at phosphor_core::* where the UI can reach it without ever naming store (T007)" \
                "$content"
        done < <(printf '%s\n' "$core_lib_stripped" | grep -nE -- "$pattern" || true)
    done
fi

# ---------------------------------------------------------------------------

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-no-store-mutation: FAILED — $violations violation(s)."
    echo "  phosphor-ui reads ViewModels (phosphor_core::vm) and the view tree"
    echo "  (phosphor_core::view). Mutation is an Action against the store, dispatched"
    echo "  by the binary — never a call from a widget (invariant §0, Q12, T007)."
    exit 1
fi

echo "lint-no-store-mutation: OK — $files_checked file(s) in ${UI_CRATES[*]} clean, module split intact, dependency direction holds"
