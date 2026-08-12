#!/usr/bin/env bash
# Tooling pass — structural lint: the declared MSRV is the real one.
#
# `workspace.package.rust-version` is a promise that the workspace builds on that
# compiler. Nothing in this repo ever tests it: `rust-toolchain.toml` pins 1.97.1
# for tape determinism (see its own header), so every build anyone runs is far
# above the floor and a wrong floor is invisible.
#
# It was wrong. From `T002` until this script, the workspace declared `1.85` —
# edition 2024's floor, and the vendored fork's — while `ratatui` 0.30.2 and the
# five crates released with it declare `rust-version = "1.88"`, as does `time`
# 0.3.55. A floor beneath a dependency's is not a floor.
#
# This recomputes it the only way that cannot go stale: ask `cargo metadata` what
# every package in the resolved graph requires, take the maximum, and compare.
# The next dependency bump that raises the real floor fails here rather than
# silently making the manifest lie.
#
# Deliberate limit: this checks the floor is not *too low*. It does not prove the
# workspace compiles at it — that needs the toolchain installed and a real build,
# which is a CI matrix job rather than a lint. What it catches is the way this
# claim actually rots, which is a dependency moving under it.
#
# Exit 0 = declared floor is at least the graph's, exit 1 = it is below.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "lint-msrv: cargo not on PATH" >&2
    exit 1
fi

cargo metadata --format-version 1 2>/dev/null | python3 -c '
import json
import re
import sys
import pathlib


def parse(version):
    """`1.88.0` / `1.88` -> (1, 88). Patch is ignored: nobody floors on one."""
    parts = re.findall(r"\d+", version)[:2]
    return tuple(int(p) for p in parts) + (0,) * (2 - len(parts))


meta = json.load(sys.stdin)

declared_text = None
for line in pathlib.Path("Cargo.toml").read_text(encoding="utf-8").splitlines():
    match = re.match(r"\s*rust-version\s*=\s*\"([^\"]+)\"", line)
    if match:
        declared_text = match.group(1)
        break

if declared_text is None:
    print("lint-msrv: FAILED — Cargo.toml declares no workspace rust-version")
    sys.exit(1)

declared = parse(declared_text)

# Workspace members declare the floor; everything else in the graph constrains
# it. `cargo metadata` without --no-deps gives us both.
required = []
for package in meta["packages"]:
    version = package.get("rust_version")
    if version:
        required.append((parse(version), package["name"], package["version"], version))

required.sort(reverse=True)
if not required:
    print("lint-msrv: no package in the graph declares a rust-version")
    sys.exit(0)

highest, name, ver, text = required[0]

if declared < highest:
    print("lint-msrv: FAILED — the declared MSRV is below what the graph requires.")
    print()
    print(f"  Cargo.toml declares : {declared_text}")
    print(f"  the graph requires  : {text}")
    print()
    print("  Raised by:")
    for req, n, v, t in required:
        if req == highest:
            print(f"    {n} {v} needs {t}")
    print()
    print("  Set workspace.package.rust-version to the higher value, or pin the")
    print("  dependency back. A floor beneath a dependency is not a floor.")
    sys.exit(1)

print(f"lint-msrv: clean — declared {declared_text}, graph requires {text} ({name} {ver})")
'
