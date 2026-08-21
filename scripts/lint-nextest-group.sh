#!/usr/bin/env bash
# Every test binary that spawns a child process is in nextest's
# `spawns-a-child` group, and every binary the group names exists.
#
# `.config/nextest.toml` charges those tests two threads apiece, because they
# run a second process nextest cannot see and then wait on a wall clock for it.
# Four CI runs went red on four *different* pty tests before that existed, each
# a 30s harness timeout on a starved child, and each passing on a re-run.
#
# The list is a hand-written filter, which is a list that rots two ways — and
# both ways are silent:
#
#   1. **A new spawner nobody added.** It runs at full width, competes with the
#      children it does not know about, and the flake comes back wearing a
#      different test's name. Writing this list by eye already missed two:
#      `phosphor-core::journal` and `phosphor-term::owning_the_terminal`, the
#      second of which opens a pty exactly as `loop_pty` does.
#   2. **A name that matches nothing.** A renamed or deleted test file leaves a
#      `binary_id(=…)` that quietly selects zero tests. nextest does not warn —
#      the filter is still valid — so the group shrinks and the run goes green.
#
# The spawn markers are `Command::new`, `open_pty`, `ServerSpec::new` and
# `SessionSpec::new`, and a file carrying one **must** be in the group.
#
# **The last two were added after the first two missed a spawner that then took
# the suite down.** `phosphor-buffer::lsp_documents` builds a `ServerSpec` around
# `sh -c` and hands it to the client, which spawns it — so the file has neither
# of the original markers, was never in the group, and `.config/nextest.toml`'s
# own prose said of it *"neither of which spawns anything"*. It ran at full
# width until `T050` added six more child-spawning tests, and then its 20-second
# poll for a `didChange` came back with a log holding nothing but `initialize`.
# A spec naming a command **is** a spawn marker: it is how this workspace asks a
# library to start a process, and both clients spell it the same way.
#
# Extras are still allowed on purpose: `phosphor-buffer::lsp` carries no marker
# at all and belongs there anyway. A library can spawn on a test's behalf; the
# deadline is the test's either way. So this is a subset check in one direction
# and an existence check in the other, which is the strongest pair that is
# true.
#
# Deliberately NOT checked: that nextest parses the filter, or that the group
# binds at runtime. `cargo nextest run` does both on every invocation — an
# unparseable filter is a hard error — and reproducing it here would mean
# building the whole workspace inside `just lint`.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

config=".config/nextest.toml"
violations=0

[ -f "$config" ] || {
    echo "$config does not exist — the spawns-a-child group lives there"
    exit 1
}

grep -q 'test-groups.spawns-a-child' "$config" || {
    echo "$config has no [test-groups.spawns-a-child] — see this script's header"
    exit 1
}

# 1. Every spawner is named.
while IFS= read -r file; do
    package_dir="${file%%/tests/*}"
    package="$(
        sed -n 's/^name *= *"\(.*\)"/\1/p' "$package_dir/Cargo.toml" | head -1
    )"
    stem="$(basename "$file" .rs)"
    id="$package::$stem"
    if ! grep -qF "binary_id(=$id)" "$config"; then
        echo "$file spawns a child process but $id is not in the spawns-a-child group"
        echo "  add  + binary_id(=$id)  to the filter in $config"
        violations=$((violations + 1))
    fi
done < <(grep -lE 'Command::new|open_pty|ServerSpec::new|SessionSpec::new' crates/*/tests/*.rs)

# 2. Every name is a real test binary.
while IFS= read -r id; do
    package="${id%%::*}"
    stem="${id##*::}"
    found=0
    for dir in crates/*/; do
        name="$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/Cargo.toml" | head -1)"
        if [ "$name" = "$package" ] && [ -f "$dir/tests/$stem.rs" ]; then
            found=1
            break
        fi
    done
    if [ "$found" -eq 0 ]; then
        echo "$config names $id, which is not a test binary — it selects zero tests"
        violations=$((violations + 1))
    fi
done < <(sed -n 's/.*binary_id(=\([^)]*\)).*/\1/p' "$config")

if [ "$violations" -ne 0 ]; then
    echo
    echo "lint-nextest-group: $violations problem(s)"
    exit 1
fi

echo "lint-nextest-group: the spawns-a-child group matches the tree"
