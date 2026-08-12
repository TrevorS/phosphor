#!/usr/bin/env bash
# T006 — structural lint: no literal colours in phosphor-ui.
#
# Design Language §12: "theme — actor/state palette as a struct; every widget
# takes &Theme — no literal colors in components." A widget that constructs
# its own Color has escaped the Theme contract, which is the whole point of
# T010-T013 (base16-style themes + actor-hue validation): the palette is
# supposed to be the *only* place colour lives, so a bad theme can be
# rejected at load instead of leaking a wrong hue past validation.
#
# theme.rs (and any theme/ submodule) is the one legitimate exception — it
# defines the palette, so it IS colour literals by definition. Every other
# file in the crate must reach a colour only through &Theme.
#
# Grep-level per the plan (IMPLEMENTATION-PLAN.md:207-208), not an AST lint:
#   - it skips lines whose trimmed text starts with `//` so a doc comment
#     that merely *mentions* Color::Rgb (see lib.rs's own header) doesn't
#     trip it; a trailing `// comment` after real code is not filtered out.
#   - it can't see through macros, re-exports, or multi-line literals.
# That's an accepted tradeoff for this task's scope — a lint a teammate
# can't act on gets disabled, so this stays simple enough to read in full.
#
# Catches four spellings of "a colour that didn't come from the Theme":
# Color::Rgb, Color::Indexed, Color::from_u32, and raw hex literals (both
# `0x1a9a62`-style and `"#1a9a62"`-style) — not just the two the plan names.

set -euo pipefail

# Anchor to the repo root, exactly as lint-no-store-mutation.sh does. Without
# this the relative CRATE_DIR below resolves against the caller's cwd, and the
# script exits 0 with "nothing to check" from anywhere that isn't the repo root
# — a lint that silently stops existing when someone runs it from a subdirectory.
# `just lint` happens to cd here first, so CI was never affected; this makes the
# guarantee independent of the caller.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CRATE_DIR="crates/phosphor-ui"

# A missing subject is a failure, not a pass. Same reasoning as check 3 of
# lint-no-store-mutation.sh: this lint must never go green because the thing it
# guards was deleted or renamed out from under it.
if [ ! -d "$CRATE_DIR" ]; then
    echo "lint-no-literal-colours: FAILED — $CRATE_DIR not found; this lint guards that crate, so its absence is an offence, not a clean run"
    exit 1
fi

# Two parallel arrays, not "name:pattern" strings — the names contain `::`
# (Rust path syntax), which would collide with a colon delimiter.
RULE_NAMES=(
    "Color::Rgb"
    "Color::Indexed"
    "Color::from_u32"
    "raw hex colour literal (0x......)"
    'quoted hex colour literal ("#......")'
)
RULE_PATTERNS=(
    "Color::Rgb"
    "Color::Indexed"
    "Color::from_u32"
    "0x[0-9a-fA-F]{6,8}([^0-9a-fA-F]|\$)"
    '"#[0-9a-fA-F]{6}"'
)

violations=0

filelist="$(mktemp)"
trap 'rm -f "$filelist"' EXIT
find "$CRATE_DIR" -name '*.rs' ! -name 'theme.rs' ! -path '*/theme/*' -print0 > "$filelist"

while IFS= read -r -d '' file; do
    for i in "${!RULE_NAMES[@]}"; do
        name="${RULE_NAMES[$i]}"
        pattern="${RULE_PATTERNS[$i]}"
        while IFS=: read -r lineno content; do
            [ -z "$lineno" ] && continue
            trimmed="$(printf '%s' "$content" | sed -e 's/^[[:space:]]*//')"
            case "$trimmed" in
                //*) continue ;; # doc/line comments describing the rule don't count
            esac
            echo "$file:$lineno: literal colour ($name) — no literal colours in phosphor-ui; every widget takes &Theme instead (Design Language §12, T006)"
            echo "    ${trimmed}"
            violations=$((violations + 1))
        done < <(grep -nE -- "$pattern" "$file" || true)
    done
done < "$filelist"

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-no-literal-colours: FAILED — $violations literal-colour violation(s) in phosphor-ui (see above)"
    exit 1
fi

echo "lint-no-literal-colours: clean — no literal colours found outside theme.rs"
exit 0
