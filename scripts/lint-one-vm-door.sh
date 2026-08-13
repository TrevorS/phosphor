#!/usr/bin/env bash
# One door into the VM, so "arbitrary scheme ran" is recorded in one place.
#
# The rule (`TASKS.md`, `T026`): anything that runs arbitrary scheme — a REPL
# evaluation, a keybinding's thunk — may move state the statusline composer
# reads *without* moving the ViewModel, so the frame cache has to be
# invalidated. The `CP-2` review found the keybinding half of that missing by
# **running it**: a key bound to `(status-order-set! 'right '())` fired and the
# frame that followed wrote no cells.
#
# It was then correct by *remembering*, which is the weakest kind — there were
# two invalidation sites in `main.rs` and nothing said a third would be needed.
# `T026` made it structural instead: `Layer` owns the `Runtime`, every method on
# it that can run user scheme sets one flag, and the loop reads that flag in one
# place. The structure is the *absence* of any other way in — and an absence is
# exactly what a reviewer stops noticing.
#
# So this checks the absence. Two rules:
#
#   1. Nothing hands out the runtime. No signature in the binary returns
#      `&mut Runtime`, and no function outside `impl Layer` takes one.
#   2. Every call that enters the VM is inside `impl Layer`. If a new one is
#      needed it has to be a method there, and the shape of every method there
#      is to set the flag.
#
# A new VM entry point that skips the flag is a frame that silently does not
# repaint. That is invisible in a screenshot, invisible in a unit test that does
# not draw, and obvious only on a real terminal — which is why it is worth a
# lint rather than a comment.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

FILE="crates/phosphor/src/main.rs"
status=0

if [ ! -f "$FILE" ]; then
    echo "lint-one-vm-door: FAILED — $FILE not found"
    exit 1
fi

# The line range of `impl Layer { … }`, by the closing brace in column 0.
range=$(awk '
    /^impl Layer \{$/ { start = NR; inside = 1; next }
    inside && /^\}$/   { print start "," NR; inside = 0 }
' "$FILE")

if [ -z "$range" ]; then
    echo "lint-one-vm-door: FAILED — no \`impl Layer { … }\` block in $FILE"
    echo "    The one door into the VM is that block. If it was renamed, rename it here too."
    exit 1
fi

first=${range%,*}
last=${range#*,}

# Rule 1 — nothing hands the runtime out.
handed=$(grep -nE '(-> *&mut +Runtime|: *&mut +Runtime)' "$FILE" || true)
if [ -n "$handed" ]; then
    echo "lint-one-vm-door: FAILED — $FILE hands out the Steel runtime"
    echo "$handed" | sed 's/^/    /'
    echo "    A caller holding \`&mut Runtime\` can run scheme without \`Layer\` knowing,"
    echo "    and the frame that follows keeps a stale statusline. Add a method to"
    echo "    \`impl Layer\` instead — it is the flag-setting shape by construction."
    status=1
fi

# Rule 2 — every VM entry is inside that block.
#
# What counts as entering: reaching the `Runtime` itself, or calling one of the
# module-level functions that take one. `Runtime::boot` and `Runtime::root`
# construct rather than enter, and `runtime.global` reads a binding without
# running one, so none of those is listed. A call *through* a `Layer` value
# (`layer.evaluate(…)`, `self.0.evaluate(…)`) is the door working, not a hole in
# it, so the pattern names the runtime rather than the method.
entries='self\.runtime|runtime\.evaluate\(|runtime\.call\(|keymap::press|keymap::reset|status::compose'
outside=$(grep -nE "$entries" "$FILE" | awk -F: -v first="$first" -v last="$last" '
    $1 < first || $1 > last { print }
' || true)

# The `use` lines that name those modules are not calls.
outside=$(echo "$outside" | grep -vE '^[0-9]+:use ' || true)

if [ -n "$outside" ]; then
    echo "lint-one-vm-door: FAILED — $FILE enters the Steel VM outside \`impl Layer\`"
    echo "$outside" | sed 's/^/    /'
    echo "    Every entry point that can run user scheme belongs to \`Layer\`, which"
    echo "    records that it ran; the loop invalidates the frame cache from that one"
    echo "    flag. See T026's note in docs/TASKS.md and main.rs's module header."
    status=1
fi

if [ "$status" -eq 0 ]; then
    echo "lint-one-vm-door: clean — one door into the VM (impl Layer, ${first}-${last})"
fi
exit "$status"
