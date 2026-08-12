#!/usr/bin/env bash
# T020 — the three doors are derived views, never tables of their own.
#
# Invariant 2 says Steel, MCP and the CLI share one vocabulary, and the plan is
# blunt about how that rots: "if MCP tools are registered by hand alongside a
# separate Steel binding table, invariant 2 rots within a month"
# (IMPLEMENTATION-PLAN.md §0). The defence is that each door is a *total
# function* of a registry row — crates/phosphor-core/src/registry/{steel,mcp,cli}.rs
# each expose one, and there is nowhere in them to forget a capability.
#
# The one way to break that without deleting a door is to special-case a
# capability inside a door: `if capability.name == "mark-seen" { … }`, or a
# per-capability table beside the generated one. That is exactly the planted
# violation T024's door-parity test is built to fail on, and this lint catches
# the shape before the test has to catch the symptom.
#
# So: no door module may name a capability. Door names are read from the
# `actions!` / `queries!` tables — the same rows the doors derive from — so this
# check cannot drift from the vocabulary either.
#
# Comment lines and everything from `#[cfg(test)]` onward are excluded: tests
# legitimately name capabilities (that is what a fixture is), and prose that
# cites `mark-seen` as an example is documentation, not dispatch.

set -euo pipefail

cd "$(dirname "$0")/.."

core="crates/phosphor-core/src"
registry="${core}/registry.rs"
doors=("${core}/registry/steel.rs" "${core}/registry/mcp.rs" "${core}/registry/cli.rs")

status=0

# Every door module has to exist and be declared by the registry, or a door has
# quietly stopped being generated at all.
for door in "${doors[@]}"; do
    if [ ! -f "$door" ]; then
        echo "lint-one-registry: missing door module ${door}"
        status=1
        continue
    fi
    module="$(basename "$door" .rs)"
    if ! grep -qE "^pub mod ${module};" "$registry"; then
        echo "lint-one-registry: ${registry} does not declare \`pub mod ${module};\`"
        status=1
    fi
done

[ "$status" -eq 0 ] || exit "$status"

# The door names, from the two vocabulary tables.
names="$(
    grep -hoE '^[[:space:]]+[A-Z][A-Za-z0-9]* = "[a-z0-9-]+"' \
        "${core}/action.rs" "${core}/query.rs" |
        sed -E 's/.*"([a-z0-9-]+)"/\1/' | sort -u
)"

if [ -z "$names" ]; then
    echo "lint-one-registry: read no capability names from action.rs / query.rs — the tables moved"
    exit 1
fi

count="$(printf '%s\n' "$names" | wc -l | tr -d ' ')"

for door in "${doors[@]}"; do
    # Dispatch only: drop comment lines and the test module.
    body="$(awk '/^#\[cfg\(test\)\]/ { exit } { print }' "$door" | grep -vE '^[[:space:]]*//' || true)"
    while IFS= read -r name; do
        [ -n "$name" ] || continue
        if printf '%s\n' "$body" | grep -qF "\"${name}\""; then
            echo "lint-one-registry: ${door} names the capability \`${name}\`."
            echo "  A door derives every capability from its registry row; naming one means"
            echo "  it has an exception, and an exception is a second registry (T020, T024)."
            status=1
        fi
    done <<<"$names"
done

if [ "$status" -ne 0 ]; then
    exit "$status"
fi

echo "lint-one-registry: clean — ${#doors[@]} door modules derive all ${count} capabilities, none named"
