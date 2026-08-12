"""Check the numbers the docs assert against the numbers the repo actually has.

Called by `scripts/lint-doc-claims.sh`. Every check here exists because the claim
it verifies had already gone stale and been corrected by hand:

  * task counts drifted twice, when the docs review added `T084`-`T089` and again
    when `CP-1` added `T090`;
  * the wave widths and gate counts in `TEAM.md` are derived from the dependency
    graph in `TASKS.md`, and nothing recomputed them when tasks were added — wave
    4 sat at 14 while the graph said 15;
  * the toolchain pin was raised 1.93.1 -> 1.97.1 at `CP-0` and two files went on
    quoting the old value, one of them `tapes/README.md`, where the pin is the
    thing that makes reference images comparable at all.

The point is not arithmetic. It is that a document asserting a number nobody can
recompute is indistinguishable from a document asserting a number that is wrong,
and this build has now produced both.
"""

import pathlib
import re
import sys
from collections import Counter

FAILURES: list[str] = []


def fail(what: str, detail: str) -> None:
    FAILURES.append(f"{what}\n    {detail}")


def read(path: str) -> str:
    return pathlib.Path(path).read_text(encoding="utf-8")


TASKS = read("docs/TASKS.md")
TEAM = read("docs/TEAM.md")
PLAN = read("docs/IMPLEMENTATION-PLAN.md")
README = read("docs/README.md")

# ── the dependency graph, recomputed from TASKS.md ───────────────────────────

parts = re.split(r"\n(?=- \[[ x]\] \*\*(?:T\d{3}|V\d{3}))", TASKS)
deps: dict[str, list[str]] = {}
for part in parts:
    m = re.match(r"- \[[ x]\] \*\*((?:T|V)\d{3})", part)
    if not m:
        continue
    needs = re.search(r"\*Needs:\*(.*?)(?:\n\n|\Z)", part, re.S)
    deps[m.group(1)] = (
        re.findall(r"\b((?:T|V)\d{3})\b", needs.group(1)) if needs else []
    )

defined = set(deps)
t_tasks = {t for t in defined if t.startswith("T")}
v_tasks = {t for t in defined if t.startswith("V")}

# ── 1 · no dangling task or anchor references ────────────────────────────────

for path in sorted(pathlib.Path("docs").glob("*.md")):
    text = path.read_text(encoding="utf-8")
    refs = set(re.findall(r"\b((?:T|V)\d{3})\b", text))
    missing = sorted(r for r in refs if r not in defined)
    if missing:
        fail(f"{path}: references tasks that do not exist", ", ".join(missing))

anchors = set(re.findall(r'<a id="(q\d+)">', PLAN))
q_refs = set(re.findall(r"#(q\d+)\)", TASKS + TEAM + PLAN + README))
dangling_q = sorted(q_refs - anchors)
if dangling_q:
    fail("decision anchors referenced but not defined in the plan", ", ".join(dangling_q))

# ── 2 · task counts ──────────────────────────────────────────────────────────

m = re.search(r"\*\*(\d+) tasks \+ (\d+) harness tasks", TASKS)
if not m:
    fail("docs/TASKS.md: could not find the 'N tasks + N harness tasks' header", "regex found nothing")
elif (int(m.group(1)), int(m.group(2))) != (len(t_tasks), len(v_tasks)):
    fail(
        "docs/TASKS.md header task counts are wrong",
        f"says {m.group(1)} tasks + {m.group(2)} harness; the file defines "
        f"{len(t_tasks)} + {len(v_tasks)}",
    )

m = re.search(r"decomposed into (\d+) tasks", README)
if m and int(m.group(1)) != len(t_tasks):
    fail(
        "docs/README.md task count is wrong",
        f"says {m.group(1)}; TASKS.md defines {len(t_tasks)}",
    )

m = re.search(r"\*\*(\d+) of (\d+) tasks are assigned\*\*", TEAM)
if not m:
    fail("docs/TEAM.md: could not find the 'N of M tasks are assigned' line", "regex found nothing")
elif int(m.group(2)) != len(defined):
    fail(
        "docs/TEAM.md assignment denominator is wrong",
        f"says {m.group(2)} total; TASKS.md defines {len(defined)}",
    )

# ── 3 · wave widths and gate counts, recomputed from the graph ───────────────

depth: dict[str, int] = {}


def longest_path(task: str, stack: tuple[str, ...] = ()) -> int:
    if task in depth:
        return depth[task]
    if task in stack:
        raise SystemExit(f"lint-doc-claims: dependency cycle {stack + (task,)}")
    ds = [longest_path(x, stack + (task,)) for x in deps.get(task, []) if x in deps]
    depth[task] = 0 if not ds else max(ds) + 1
    return depth[task]


# T008/T009 are the two complete spikes and are excluded from the staffing curve,
# matching how TEAM.md's table was originally computed.
excluded = {"T008", "T009"}
widths = Counter(longest_path(t) for t in deps if t not in excluded)
computed_waves = [widths[i] for i in range(max(widths) + 1)]

m = re.search(r"^tasks\s+((?:\d+\s+)*\d+)\s*$", TEAM, re.M)
if not m:
    fail("docs/TEAM.md: could not find the wave-width table", "regex found nothing")
else:
    claimed = [int(x) for x in m.group(1).split()]
    if claimed != computed_waves:
        fail(
            "docs/TEAM.md wave widths do not match the graph in TASKS.md",
            f"table says {claimed}; recomputed {computed_waves}",
        )

reverse: dict[str, list[str]] = {}
for task, needs in deps.items():
    for n in needs:
        reverse.setdefault(n, []).append(task)


def downstream(root: str) -> set[str]:
    seen: set[str] = set()
    stack = [root]
    while stack:
        for child in reverse.get(stack.pop(), []):
            if child not in seen:
                seen.add(child)
                stack.append(child)
    return seen


m = re.search(r"`T001` gates \*\*(\d+) of (\d+)\*\*", TEAM)
if m:
    gated = len(downstream("T001"))
    if (int(m.group(1)), int(m.group(2))) != (gated, len(defined)):
        fail(
            "docs/TEAM.md: T001's gate count is wrong",
            f"says {m.group(1)} of {m.group(2)}; recomputed {gated} of {len(defined)}",
        )

m = re.search(r"`T019` gates \*\*(\d+)\*\*", TEAM)
if m and int(m.group(1)) != len(downstream("T019")):
    fail(
        "docs/TEAM.md: T019's gate count is wrong",
        f"says {m.group(1)}; recomputed {len(downstream('T019'))}",
    )

m = re.search(r"`T041` has \*\*(\d+) direct dependents\*\*", TEAM)
if m and int(m.group(1)) != len(reverse.get("T041", [])):
    fail(
        "docs/TEAM.md: T041's direct-dependent count is wrong",
        f"says {m.group(1)}; recomputed {len(reverse.get('T041', []))}",
    )

# ── 4 · the toolchain pin, wherever prose quotes it ──────────────────────────

m = re.search(r'channel\s*=\s*"([^"]+)"', read("rust-toolchain.toml"))
if not m:
    fail("rust-toolchain.toml: no channel found", "regex found nothing")
else:
    pinned = m.group(1)
    for path in ("tapes/README.md", ".github/workflows/ci.yml"):
        text = read(path)
        quoted = set(re.findall(r"\b(1\.\d{2}\.\d)\b", text))
        wrong = sorted(v for v in quoted if v != pinned)
        if wrong:
            fail(
                f"{path}: quotes a toolchain version that is not the pin",
                f"found {', '.join(wrong)}; rust-toolchain.toml pins {pinned}",
            )

# ── report ───────────────────────────────────────────────────────────────────

if FAILURES:
    print("lint-doc-claims: FAILED — the docs assert things the repo contradicts:\n")
    for f in FAILURES:
        print(f"  {f}\n")
    sys.exit(1)

print(
    f"lint-doc-claims: clean — {len(t_tasks)} tasks + {len(v_tasks)} harness, "
    f"waves {computed_waves}, no dangling references, toolchain quoted consistently"
)
