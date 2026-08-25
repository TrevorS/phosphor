#!/usr/bin/env bash
# Structural lint: nothing refreshes or rewrites a buffer except through an
# Action.
#
# **This is `CP-8b`'s third bullet, made mechanical.** That checkpoint asks
# Claude to verify *"no code path can refresh a buffer without an explicit
# Action"*, and until this file that was a sentence somebody would have had to
# re-check by reading 22,000 lines. Invariant 3 is the claim it protects —
# *"buffer holds stable; nothing moves unless you asked"* — and it is the
# invariant `CP-8b` calls **the most likely to be violated by accident and the
# most damaging when it is**.
#
# WHAT IT CHECKS. Two confinements, both about the same thing from opposite
# ends: what may change a buffer, and who may ask.
#
#   A. `Editing::reload` — taking what is on disk — is called only from
#      `Editing::act`. That function *is* the Action applier, so a call
#      anywhere else is by construction a refresh nobody asked for. `T069`
#      built the watcher that makes this reachable: it reports and never
#      refreshes, and this is what keeps that true after the next edit.
#
#   B. `code_mut()` — the only handle that mutates the rope — is called only
#      from `Editing::splice` and the transaction pair `begin`/`commit`.
#      Anywhere else is a buffer edit that skips the undo journal, which is a
#      different bug with the same shape: state changing where nothing recorded
#      that it did.
#
# WHY A LIST OF FUNCTIONS AND NOT A COUNT. A count goes stale the first time
# someone adds a legitimate call and updates the number without reading what
# they permitted. Naming the *enclosing function* means the diff that widens
# this list says whose code is now allowed to move a buffer, which is the
# question a reviewer should be asked.
#
# WHAT IT DELIBERATELY DOES NOT CATCH. The vendored fork owns the rope and can
# mutate it from inside; this lint stops at the seam, like every other one in
# this directory. It also cannot see a *future* mutation primitive — if a second
# `code_mut()`-alike is ever introduced, this checks the wrong name and says
# nothing. That is why the patterns below are asserted to match at all: a
# pattern that finds nothing fails rather than passes.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - <<'PYEOF'
import pathlib
import re
import sys

MAIN = pathlib.Path("crates/phosphor/src/main.rs")

# Enclosing functions permitted to hold each call, and why.
PERMITTED = {
    ".reload()": (
        {"act"},
        "`Editing::act` is the Action applier — `reload-from-disk` and "
        "`resolve-disk-diff`'s take-disk exit. A call anywhere else is a "
        "refresh nobody asked for, which is invariant 3 broken.",
    ),
    "code_mut()": (
        {"splice", "begin", "commit"},
        "`splice` is the one edit path and `begin`/`commit` are its transaction "
        "bookkeeping. A call anywhere else edits the rope without recording it, "
        "so undo would not know.",
    ),
}

failures = []

text = MAIN.read_text(encoding="utf-8")
# Column-0 `#[cfg(test)]` is the test *module* — the same anchor
# `lint-action-arms.sh` uses, and for the same reason: a fixture that pokes a
# buffer directly is not a code path a user can reach.
module = re.search(r"^#\[cfg\(test\)\]", text, re.M)
body = text[: module.start()] if module else text
if len(body) < 10_000:
    failures.append(f"read only {len(body)} bytes of {MAIN} — the file's layout moved.")

FN = re.compile(r"^\s*(?:pub(?:\(crate\))? )?(?:const )?(?:async )?fn ([a-z_0-9]+)")

for needle, (allowed, why) in PERMITTED.items():
    seen = []
    fn = "<top level>"
    for number, line in enumerate(body.split("\n"), 1):
        found = FN.match(line)
        if found:
            fn = found.group(1)
        if needle in line:
            seen.append((number, fn, line.strip()))

    if not seen:
        failures.append(
            f"no call to `{needle}` found in {MAIN}. Either the primitive was renamed — in "
            f"which case this lint is now checking nothing and must be pointed at the new "
            f"name — or it is genuinely gone and this entry should be removed."
        )
        continue

    for number, fn, source in seen:
        if fn in allowed:
            continue
        failures.append(
            f"{MAIN}:{number} calls `{needle}` inside `{fn}`, which is not one of "
            f"{sorted(allowed)}.\n"
            f"    {source}\n"
            f"    {why}\n"
            f"    If `{fn}` genuinely should be able to do this, add it to PERMITTED in "
            f"scripts/lint-buffer-refresh.sh — and say in the diff why a new caller may move "
            f"a buffer."
        )

if failures:
    print("lint-buffer-refresh: FAILED")
    print()
    for message in failures:
        print(f"  {message}")
        print()
    sys.exit(1)

total = sum(len(v[0]) for v in PERMITTED.values())
print(
    f"lint-buffer-refresh: clean — {len(PERMITTED)} buffer primitives confined to "
    f"{total} named function(s); nothing refreshes a buffer without an Action (CP-8b)"
)
PYEOF
