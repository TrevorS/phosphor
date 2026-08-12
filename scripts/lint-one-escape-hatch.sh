#!/usr/bin/env bash
# T080 — the `spans` escape hatch is the ONLY custom-draw path.
#
# Q12 accepts a cost in writing:
#
#   "Accepted cost: the `spans` hatch is a slope toward writing renderers in
#    scheme. It is signposted rather than fenced — ONE GREP-ABLE PRIMITIVE NAME,
#    so when a frame-budget regression appears there is exactly one place to
#    look."
#
# That sentence is only true while there is exactly one. The failure it guards
# against is quiet and reasonable-looking: a second node kind that takes rows of
# styled runs — `Node::Table { rows: Vec<SpanRow> }`, `Node::Sparkline { … }` —
# because a surface needed something the primitives did not cover and a new node
# kind felt cheaper than a new primitive. Nothing breaks that day. What breaks is
# the promise that a frame-budget regression has one place to look, and the rule
# Q12 states in the same breath: "Steel composes primitives; it does not define
# them. Without that line, custom widgets get written in scheme and the frame
# budget comes back."
#
# # What it checks
#
#   1. **The protocol.** In `phosphor_core::view`, exactly one node kind carries
#      styled rows, and it is `Spans` — tagged `spans`.
#   2. **The interpreter.** In `phosphor-ui`, `SpanRow` is reachable from exactly
#      one module (`interpret.rs`), so there is one function that turns
#      composition-supplied text into cells.
#
# Both are greps over source, with the tradeoff the other structural lints
# document: comment lines are skipped, and it cannot see through macros. What it
# can see is the shape the mistake actually takes.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

VIEW="crates/phosphor-core/src/view.rs"
UI_SRC="crates/phosphor-ui/src"

status=0

# A missing subject is a failure, not a pass.
if [ ! -f "$VIEW" ] || [ ! -d "$UI_SRC" ]; then
    echo "lint-one-escape-hatch: FAILED — $VIEW or $UI_SRC not found; this lint guards them, so their absence is an offence"
    exit 1
fi

# --- 1. the protocol -------------------------------------------------------
#
# The node table is everything above the `#[cfg(test)]` line; the samples below
# it legitimately name `SpanRow` once per fixture.
table="$(sed '/#\[cfg(test)\]/,$d' "$VIEW")"

# Field declarations of the form `rows: Vec<SpanRow>` — one per node kind that
# takes styled rows.
carriers="$(printf '%s\n' "$table" |
    grep -nE '^[[:space:]]*[a-z_]+:[[:space:]]*Vec<SpanRow>' || true)"
count="$(printf '%s' "$carriers" | grep -c . || true)"

if [ "$count" -ne 1 ]; then
    echo "$VIEW: $count node kinds carry styled rows; the hatch is exactly one (Q12)"
    printf '%s\n' "$carriers" | sed 's/^/    /'
    echo "    a surface the primitives do not cover is either a new primitive in phosphor-ui,"
    echo "    or Node::Spans — never a third thing"
    status=1
fi

# … and that one is the hatch itself, by name and by tag.
if ! printf '%s\n' "$table" | grep -qE '^[[:space:]]*Spans = "spans"'; then
    echo "$VIEW: the hatch is not declared as \`Spans = \"spans\"\` — the one grep-able name has moved"
    status=1
fi

# --- 2. the interpreter ----------------------------------------------------
#
# One module may name `SpanRow`. A second one is a second place that turns
# composition-supplied text into cells, which is the thing this lint exists to
# make visible.
drawers="$(grep -rlE '(^|[^_[:alnum:]])SpanRow([^_[:alnum:]]|$)' "$UI_SRC" --include='*.rs' | sort)"
expected="$UI_SRC/interpret.rs"

if [ "$drawers" != "$expected" ]; then
    echo "$UI_SRC: styled rows are drawn from more than one place"
    printf '%s\n' "$drawers" | sed 's/^/    /'
    echo "    expected exactly ${expected} — one hatch, one function that draws it"
    status=1
fi

if [ "$status" -ne 0 ]; then
    echo
    echo "lint-one-escape-hatch: FAILED — the escape hatch is no longer one name (see above)"
    exit 1
fi

echo "lint-one-escape-hatch: clean — Node::Spans is the only custom-draw path, drawn in one place"
exit 0
