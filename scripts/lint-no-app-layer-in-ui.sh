#!/usr/bin/env bash
# T002 — structural lint: no app layer in phosphor-ui.
#
# The rule is T002's: "`phosphor-ui` gets `ratatui-core` only — never `ratatui`."
# A widget crate that can reach the terminal is not a widget crate; it is half an
# application, and the split exists so the frame is owned in exactly one place
# (Q12: Steel decides what is on screen, the binary decides when pixels land).
#
# # Why this is a source lint and not a manifest one
#
# `phosphor-ui`'s manifest is already correct — `ratatui-core` only, and the
# vendored editor taken with `default-features = false`. It is still not enough,
# and that is the specific hole this lint closes:
#
#   **Cargo unifies features per crate across the whole graph.** `T090`'s host
#   turns on `ratatui-code-editor`'s `crossterm` feature so S1 can ride the
#   fork's `editor_crossterm` handler, exactly as `TASKS.md`'s S1 preamble says
#   it does. There is one `ratatui-code-editor` instance in the build, so that
#   feature is on for *every* dependent — including `phosphor-ui`, which asked
#   for the opposite. `cargo tree --workspace -e features` shows it plainly.
#
# Cargo has no way to say "this feature is for that dependent only", so the
# manifest cannot express the guarantee and the comment claiming it did was
# wrong. This is the same shape as `lint-no-store-mutation.sh`, whose header
# says: Rust has no way to say "public, but not to that crate", so the direction
# is checked here instead.
#
# The leak is currently latent — no phosphor-ui source uses it — and `T026`
# closes it for good when the input machine replaces the vendored handler and
# the `crossterm` feature goes with it. Until then this lint is what keeps
# latent from becoming load-bearing.
#
# # What it catches
#
#   - `crossterm::` in any form, which is also how `Editor::input`/`Editor::mouse`
#     would have to arrive: both take a crossterm `Event`, so a widget calling
#     them must name the crate.
#   - `ratatui::`, the app-layer umbrella. Written to match the `::` so that
#     `ratatui_core::` and `ratatui_code_editor::` — both legitimate here — do
#     not trip it.
#   - `editor_crossterm`, the fork's gated handler module, named directly.
#
# Grep-level, same tradeoff the other two lints document: it skips lines whose
# trimmed text starts with `//` (buffer_view.rs's own header explains this rule
# and must not trip it), and it cannot see through macros or re-exports.

set -euo pipefail

# Anchor to the repo root so the lint cannot silently pass from another cwd.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CRATE_DIR="crates/phosphor-ui"

# A missing subject is a failure, not a pass — this lint must never go green
# because the crate it guards was renamed out from under it.
if [ ! -d "$CRATE_DIR" ]; then
    echo "lint-no-app-layer-in-ui: FAILED — $CRATE_DIR not found; this lint guards that crate, so its absence is an offence, not a clean run"
    exit 1
fi

RULE_NAMES=(
    "crossterm"
    "ratatui (the app-layer crate, not ratatui-core)"
    "editor_crossterm"
)
RULE_PATTERNS=(
    '(^|[^_[:alnum:]])crossterm::'
    '(^|[^_[:alnum:]])ratatui::'
    'editor_crossterm'
)
RULE_ADVICE=(
    "widgets never touch the terminal; the binary decodes events and passes ViewModels down (T002, Q12)"
    "phosphor-ui takes ratatui-core only — never ratatui; that is the whole point of the T002 pin"
    "the fork's crossterm handler is the binary's temporary input path (T090), not a widget API"
)

violations=0

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
            echo "$file:$lineno: app layer in phosphor-ui ($name) — $advice"
            echo "    ${trimmed}"
            violations=$((violations + 1))
        done < <(grep -nE -- "$pattern" "$file" || true)
    done
done < "$filelist"

if [ "$violations" -gt 0 ]; then
    echo
    echo "lint-no-app-layer-in-ui: FAILED — $violations app-layer reference(s) in phosphor-ui (see above)"
    exit 1
fi

echo "lint-no-app-layer-in-ui: clean — phosphor-ui reaches no terminal and no app-layer ratatui"
exit 0
