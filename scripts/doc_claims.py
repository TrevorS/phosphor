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

# The same check, pointed at the source. Every `T0xx` in a Rust comment is a
# claim about the breakdown — "T026 deletes this", "T041 builds it", "owed to
# T078" — and there are hundreds of them threaded through the module headers
# that carry this build's reasoning. Nothing verified them until this block: a
# renumbered or dropped task left every citation of it silently wrong, and the
# rot would surface as an agent chasing a task id that no longer means what the
# comment says.
#
# Scoped to `crates/`: `vendor/` is upstream code we do not write, and its
# comments are not ours to hold to this standard.
# `T999` is reserved as the obviously-fake id: `parity.rs`'s planted-violation
# tests build a capability that cites it, and `vocabulary.rs` names it when
# explaining what a dangling citation looks like. Reserving one id keeps those
# honest without weakening the check — a real typo lands on a plausible number,
# not on this one.
FAKE_TASK = "T999"

src_refs: dict[str, list[str]] = {}
for path in sorted(pathlib.Path("crates").rglob("*.rs")):
    text = path.read_text(encoding="utf-8", errors="replace")
    for ref in set(re.findall(r"\b((?:T|V)\d{3})\b", text)):
        if ref not in defined and ref != FAKE_TASK:
            src_refs.setdefault(ref, []).append(str(path))

if src_refs:
    detail = "; ".join(
        f"{ref} in {', '.join(sorted(paths)[:3])}"
        + (f" (+{len(paths) - 3} more)" if len(paths) > 3 else "")
        for ref, paths in sorted(src_refs.items())
    )
    fail("crates/: source comments cite tasks that do not exist", detail)

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
        quoted: set[str] = set()
        for line in read(path).splitlines():
            # Scoped to lines that actually claim to be *about* the toolchain —
            # an unscoped whole-file regex here once flagged `insta 1.48.0`
            # (a crate version quoted in a CI comment, ci.yml:70) as a stale
            # toolchain pin just because the digit shape matched. Every real
            # toolchain quote in these two files sits next to the word
            # "toolchain" or "channel" (ci.yml:34, tapes/README.md:59) — this
            # narrows the scan to that, rather than any `1.NN.N`-shaped string.
            if not re.search(r"toolchain|channel", line, re.IGNORECASE):
                continue
            quoted.update(re.findall(r"\b(1\.\d{2}\.\d)\b", line))
        wrong = sorted(v for v in quoted if v != pinned)
        if wrong:
            fail(
                f"{path}: quotes a toolchain version that is not the pin",
                f"found {', '.join(wrong)}; rust-toolchain.toml pins {pinned}",
            )

# ── 5 · the capability and parity counts ──────────────────────────────────
#
# Six prose citations of `208` capabilities / `624` door checks survived the
# window `S3` added `Buffer::SetCase` and pushed the true numbers to `209` /
# `627` (`docs/TASKS.md:24` names them). Nothing above recomputes either
# number or checks prose against them — sections 2 and 3 recompute *task*
# counts from the dependency graph, but the capability vocabulary is a
# different table in a different file, and nothing walked it.
#
# The capability count is read the way `scripts/lint-one-registry.sh` reads
# it — the same `NAME = "kebab-name"` table shape in `action.rs`/`query.rs` —
# so the two cannot drift from each other into disagreeing "true" counts. The
# parity multiplier is read from `Door::ALL` in `registry.rs` rather than
# assumed to be `3`: `crates/phosphor/tests/parity.rs` derives it the same
# way (`Door::ALL.len()`, and asserts `== 3` as a fact about the *current*
# build, not a constant baked into the test).

CAPABILITY_NAME_RE = re.compile(r'^[ \t]+[A-Z][A-Za-z0-9]*[ \t]=[ \t]"([a-z0-9-]+)"', re.M)

capability_names: set[str] = set()
for path in (
    "crates/phosphor-core/src/action.rs",
    "crates/phosphor-core/src/query.rs",
):
    capability_names.update(CAPABILITY_NAME_RE.findall(read(path)))

if not capability_names:
    fail(
        "crates/phosphor-core/src/{action,query}.rs: read no capability names",
        "the `Name = \"kebab-name\"` table moved; scripts/lint-one-registry.sh "
        "reads the same shape and would also be blind to this",
    )

capability_count = len(capability_names)

REGISTRY = read("crates/phosphor-core/src/registry.rs")
m = re.search(r"pub const ALL:\s*&'static \[Self\]\s*=\s*&\[(.*?)\];", REGISTRY, re.S)
if not m:
    fail("crates/phosphor-core/src/registry.rs: could not find `Door::ALL`", "regex found nothing")
    door_count = 0
else:
    door_count = len(re.findall(r"Self::\w+", m.group(1)))
    if door_count == 0:
        fail(
            "crates/phosphor-core/src/registry.rs: `Door::ALL` names no doors",
            "regex matched the array but found no `Self::Variant` entries",
        )

parity_count = capability_count * door_count

# A doc-comment continuation (`/// text\n/// more`) reads as one sentence to a
# person but not to a line-scoped regex — this is exactly how `door.rs:219`
# splits "208" from "subcommands" across two source lines. Flatten `///`/`//!`
# continuations into their preceding line before matching; `.rs` files only,
# markdown prose does not use a comment prefix.
DOC_CONTINUATION_RE = re.compile(r"\n[ \t]*(?://[/!]|//)[ \t]?")


def flatten_doc_comments(text: str) -> str:
    return DOC_CONTINUATION_RE.sub(" ", text)


# Each pattern's unit word has to sit directly against the number — no
# whitespace-then-backtick, no arrow — which is what keeps a historical aside
# from matching: "(`208`/`624` until `S3` added…" has a backtick, not a space,
# right after the digits, and "vocabulary goes 208 → 209" has an arrow, not a
# unit word. `~?` allows the scope-template spelling ("~624 parity
# expectations", `docs/OPEN-QUESTIONS.md:141`). Plural only for
# "capabilities": a singular "1 capability" is a proposal citing how many a
# future task would add, not a claim about the vocabulary's size
# (`docs/OPEN-QUESTIONS.md:378`).
CAPABILITY_PATTERNS = [
    re.compile(r"~?\b(\d+)\s+capabilities\b"),
    re.compile(r"~?\b(\d+)\s+subcommands\b"),
    re.compile(r"~?\b(\d+)\s+generated capability verbs\b"),
]
PARITY_PATTERNS = [
    re.compile(r"~?\b(\d+)\s+door checks?\b"),
    re.compile(r"~?\b(\d+)\s+(?:parity\s+)?expectations\b"),
]

count_files = sorted(pathlib.Path("docs").rglob("*.md")) + sorted(
    pathlib.Path("crates").rglob("*.rs")
)

for path in count_files:
    text = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix == ".rs":
        text = flatten_doc_comments(text)
    bad: list[str] = []
    for pattern, expected, label in [
        *((p, capability_count, "capabilities") for p in CAPABILITY_PATTERNS),
        *((p, parity_count, "door checks") for p in PARITY_PATTERNS),
    ]:
        for match in pattern.finditer(text):
            found = int(match.group(1))
            if found != expected:
                bad.append(f"{match.group(0).strip()!r} (expected {expected} {label})")
    if bad:
        fail(f"{path}: cites a stale capability/parity count", "; ".join(sorted(set(bad))))

# ── 6 · the lint count, where CI's own prose quotes it ───────────────────────
#
# The last unchecked count anyone has found in this repo. `.github/workflows/ci.yml`'s
# `lint` job carries the sentence that explains the whole seam — every lint is one
# `scripts/lint-*.sh` script, so nobody edits the workflow to add one — and it
# ends by naming how many exist. That number said "six" while sixteen existed:
# ten behind, corrected by hand, and *the comment says so itself*. Nothing above
# reads a `.yml` file at all; sections 2 and 3 recompute task counts from the
# dependency graph and section 5 walks the capability table, and the workflow
# was outside both.
#
# The irony is the argument. A comment whose subject is "nothing recomputes a
# number written in a comment" is the one comment that has to be recomputed.
#
# The count is read from the glob that `just lint` runs (`justfile`'s `lint`
# recipe) rather than from a list here, so the lint and the recipe cannot drift
# into disagreeing about what a lint *is*.

LINT_SCRIPTS = sorted(pathlib.Path("scripts").glob("lint-*.sh"))
lint_count = len(LINT_SCRIPTS)

if not lint_count:
    fail(
        "scripts/: no lint-*.sh found",
        "`just lint` globs the same path and would pass vacuously; either the "
        "glob moved or every structural lint was deleted",
    )

# Prose spells small counts as words ("sixteen exist now"), so both spellings
# have to parse. Twenty-nine is far past any plausible number of lint scripts
# and the digits cover the rest.
NUMBER_WORDS = {
    w: i
    for i, w in enumerate(
        "zero one two three four five six seven eight nine ten eleven twelve "
        "thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty "
        "twenty-one twenty-two twenty-three twenty-four twenty-five twenty-six "
        "twenty-seven twenty-eight twenty-nine thirty".split()
    )
}
NUMBER = r"\d+|" + "|".join(sorted(NUMBER_WORDS, key=len, reverse=True))

# Same discipline as section 5: the unit phrase sits directly against the
# number, and the number has to *be* a number — a bare `\w+` here would match
# the `(` in "just lint (structural lints)", which is a job name and not a
# claim. Both rules together are what keep the historical aside in the same
# sentence from matching: `(It said "six" until the count was ten behind` has a
# quote after "six" and "behind" after "ten", so neither is a live claim.
LINT_COUNT_PATTERNS = [
    re.compile(rf"\b({NUMBER})\s+exist now\b"),
    re.compile(rf"\b({NUMBER})\s+(?:structural\s+)?lints?\b"),
    re.compile(rf"\b({NUMBER})\s+lint scripts?\b"),
]

# A YAML comment wraps across lines exactly as a Rust doc comment does, and for
# the same reason: it is prose in a column-limited file. Flatten `#`
# continuations before matching, or a rewrap moves "sixteen" and "exist now"
# onto different lines and the check goes quietly blind.
YAML_CONTINUATION_RE = re.compile(r"\n[ \t]*#[ \t]?")

workflow_claims = 0
for path in sorted(pathlib.Path(".github/workflows").glob("*.yml")):
    text = YAML_CONTINUATION_RE.sub(" ", path.read_text(encoding="utf-8", errors="replace"))
    bad = []
    for pattern in LINT_COUNT_PATTERNS:
        for match in pattern.finditer(text):
            workflow_claims += 1
            token = match.group(1)
            found = NUMBER_WORDS.get(token, None)
            if found is None:
                found = int(token)
            if found != lint_count:
                bad.append(f"{match.group(0).strip()!r} (expected {lint_count})")
    if bad:
        fail(
            f"{path}: cites a stale lint count",
            "; ".join(sorted(set(bad)))
            + f"; scripts/lint-*.sh has {lint_count}: "
            + ", ".join(p.name for p in LINT_SCRIPTS),
        )

# A check with nothing to check is the failure mode every lint in this repo
# exists to prevent, and this one is one careless rewrite away from it: delete
# the sentence and the loop above passes for a tree with any number of lints at
# all. So the absence of a claim is itself a failure — restore the sentence, or
# delete this section deliberately rather than by accident.
if not workflow_claims:
    fail(
        ".github/workflows/: no lint-count claim left to check",
        "this section verifies CI's prose against `scripts/lint-*.sh`, and found "
        "no sentence making a claim — restore it (ci.yml's `lint` job explained "
        "the seam by naming how many lints exist) or remove this section on purpose",
    )

# ── report ───────────────────────────────────────────────────────────────────

if FAILURES:
    print("lint-doc-claims: FAILED — the docs assert things the repo contradicts:\n")
    for f in FAILURES:
        print(f"  {f}\n")
    sys.exit(1)

print(
    f"lint-doc-claims: clean — {len(t_tasks)} tasks + {len(v_tasks)} harness, "
    f"waves {computed_waves}, {capability_count} capabilities, "
    f"{parity_count} parity door checks, {lint_count} structural lints, "
    f"no dangling references, toolchain quoted consistently"
)
