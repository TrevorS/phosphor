#!/usr/bin/env bash
# Prose is cited by heading or quoted phrase, never by line number.
#
# `file:line` is right for CODE. A moved line there usually means a moved fact,
# `lint-doc-claims.sh` already checks that every `T0xx` in prose is a real task,
# and `lint-doc-links.sh` runs `cargo doc` with warnings denied over the
# intra-doc links. Nothing holds a line number pointed at a paragraph.
#
# WHY THIS EXISTS, and it is not a style preference. Two citations in
# `WINDOW-F-PLAN.md` went stale inside a day, and one of them went stale in the
# most instructive way available: executing that plan's own step 1 inserted 82
# lines above its own pointer, so the citation ended up aimed at prose the step
# had just written. The other sat on a line the same step *rewrote* to fix two
# other numbers, and the third number on it was left. A spot check of the five
# citations outside that file found roughly half already wrong — one aimed at an
# unrelated task's paragraph, one past the end of its file.
#
# A quoted phrase is greppable, survives every edit above it, and tells the
# reader what they are being sent to. A line number tells them a row.
#
# CONVERTING ONE IS NOT MECHANICAL, and the first conversion done under this
# lint proved it. `TASKS.md:106` was cited for the claim that VHS's terminal
# does not implement the kitty keyboard protocol. Line 106 is about a decision
# table and `CP-0`'s lesson — so the citation was ALREADY wrong — and the
# replacement phrase written for it on the first pass ("the wave table") was
# wrong in a second, fresh way. The real target is this repository's *"What
# stays irreducibly Tier 3, and why"* table, three hundred lines off.
#
# That is the whole argument for budgets over a bulk rewrite: a stale line
# number is visibly a number, while a confidently wrong phrase reads like
# someone checked.
#
# THE BUDGETS BELOW CAN ONLY SHRINK. This is `lint-action-arms.sh` and
# `lint-node-kinds.sh`'s RECORDED shape, one layer over: a per-file count of
# citations that existed when the rule landed. Converting nine of them in one
# pass is how a wrong phrase gets introduced, so they are recorded instead and
# converted as each section is next touched. The lint fails four ways:
#
#   1. a file over its budget — a NEW citation, which is the thing being banned;
#   2. a file under its budget — some were converted and the budget is stale,
#      so lower it and keep the ratchet tight;
#   3. a file with citations and no budget — a new file joining the backlog;
#   4. a budget for a file with none left, or for a file that is gone.
#
# This script is exempt from its own rule: its budget table and this comment
# both have to write the pattern down. That is the one exemption and it is here
# rather than in a list somewhere else.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

python3 - <<'PY'
import pathlib
import re
import sys

# Path -> how many line-number citations into a markdown file it had when this
# lint landed (2026-08-20). Lower a number when you convert one; delete the row
# when it reaches zero.
RECORDED = {
    "crates/phosphor-core/src/journal.rs": 3,
    "docs/OPEN-QUESTIONS.md": 3,
    "docs/TASKS.md": 1,
    "docs/WINDOW-F-PLAN.md": 14,
    "scripts/doc_claims.py": 4,
    "scripts/lint-no-literal-colours.sh": 1,
    "scripts/lint-no-store-mutation.sh": 1,
}

# This file writes the pattern down in its own table and header.
SELF = "scripts/lint-doc-line-citations.sh"

PATTERN = re.compile(r"[A-Za-z][A-Za-z-]*\.md:[0-9]+")
ROOTS = ("docs", "crates", "scripts", "runtime", "tapes", "fixtures")
SUFFIXES = (".md", ".rs", ".sh", ".py", ".scm", ".tape", ".yml")

found: dict[str, int] = {}
for root in ROOTS:
    for path in sorted(pathlib.Path(root).rglob("*")):
        if not path.is_file() or path.suffix not in SUFFIXES:
            continue
        if str(path) == SELF:
            continue
        try:
            text = path.read_text()
        except (UnicodeDecodeError, OSError):
            continue
        hits = PATTERN.findall(text)
        if hits:
            found[str(path)] = len(hits)

for path in sorted(pathlib.Path(".").glob("*.md")):
    hits = PATTERN.findall(path.read_text())
    if hits:
        found[str(path)] = len(hits)

problems: list[str] = []

for path, count in sorted(found.items()):
    budget = RECORDED.get(path)
    if budget is None:
        problems.append(
            f"{path} cites a markdown line number {count}x and has no budget.\n"
            f"    Cite the heading or a quoted phrase instead — a line number "
            f"pointed at a paragraph has nothing holding it.\n"
            f"    If it genuinely must stay, add `\"{path}\": {count},` to "
            f"RECORDED in {SELF} with a reason."
        )
    elif count > budget:
        problems.append(
            f"{path} cites markdown line numbers {count}x, budget is {budget}. "
            f"A new one was added.\n"
            f"    Cite the heading or a quoted phrase instead. The budget is a "
            f"backlog, not an allowance."
        )
    elif count < budget:
        problems.append(
            f"{path} is down to {count} from a budget of {budget} — good, and "
            f"the budget is now stale.\n"
            f"    Lower it to {count} (or delete the row) so the ratchet stays "
            f"tight."
        )

for path, budget in sorted(RECORDED.items()):
    if path not in found:
        if not pathlib.Path(path).exists():
            problems.append(f"RECORDED budgets {path} ({budget}), which no longer exists.")
        else:
            problems.append(
                f"RECORDED budgets {path} ({budget}) and it now cites none. "
                f"Delete the row."
            )

if problems:
    print("lint-doc-line-citations: FAILED", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    sys.exit(1)

total = sum(found.values())
print(
    f"lint-doc-line-citations: clean — {len(found)} file(s) carry {total} "
    f"recorded citation(s), none new"
)
PY
