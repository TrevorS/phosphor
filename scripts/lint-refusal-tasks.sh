#!/usr/bin/env bash
# Structural lint: a refusal may not send the reader to a task that is finished.
#
# `Refusal::NotYetImplemented { task }` renders through `Refusal::sentence` in
# `crates/phosphor-core/src/action.rs` as:
#
#     not built yet — T063 builds it
#
# That is a promise with an address on it, and the address can rot. This lint
# exists because on 2026-08-24 **every one of them had**. Scouting the plan
# turned up `]b` answering *"not built yet — T053 builds it"* while `T053` was
# ticked, and pulling that thread found the same defect at fourteen sites and
# from two different causes:
#
#   * **Nine capability rows** stamped with the task that *declared* the verb
#     rather than the one that will build it — `set-theme` at `T012`,
#     `reload-runtime` at `T021`, `compact-history` at `T030`, and so on. The
#     irony is that `scripts/lint-action-arms.sh`'s RECORDED table already knew
#     the real creditor for seven of them (`T092`, `T094`, `T095`, `T070`); the
#     lint and the user-facing sentence simply disagreed, and only the lint was
#     ever read by a machine. Those seven were re-stamped and the table shrank
#     from nine gaps to two.
#
#   * **Five literals in `Editing::goto_sequence`**, which no capability row
#     governs at all, each naming the task that built the *store* the walk would
#     read. `T109` and `T110` were added to `docs/TASKS.md` as the real
#     creditors, on OPEN-QUESTIONS.md §18's precedent — *"Eleven declared
#     mutations that no task will ever close. RULED: add the tasks."*
#
# WHY NEITHER EXISTING LINT SAW IT. `lint-action-arms.sh` proves a ticked task's
# mutation is *named* by the binary; a refusal names it, so an arm that only
# refuses satisfies it. `lint-key-coverage.sh` proves every bound key is pressed
# by a test; the tests pressed these keys and asserted the refusal, which is
# exactly what made the staleness invisible — `loop_pty.rs` asserted `shows(&
# themed, "T012")` and went green for three phases while `T012` sat ticked.
# `lint-doc-claims.sh` checks that a `T0xx` in a Rust comment is a task that
# EXISTS, and every one of these did exist. Between the three there was nothing
# that asked whether the task was still *open*, which is the only property that
# makes the sentence true.
#
# **AND IT MISSED HALF THE SURFACE ON ITS FIRST DAY.** This lint was written
# against `action.rs` alone. `crates/phosphor-core/src/query.rs` stamps its rows
# in exactly the same shape — `Capabilities = "capabilities" [S2 / "T024"]` —
# and an unanswered query reaches the caller through
# `QueryError::NotYetImplemented`, which renders the same sentence from the same
# stamp. Twelve query rows were stale the whole time, measured on the built
# binary the next morning:
#
#     phosphor --eval '(capabilities)'   #raised · not built yet — T024 builds it
#
# with `T024` ticked since `S2`. `T111` is the creditor all twelve were
# re-stamped to. The lesson is the one this file already teaches one level down:
# a check that covers one of two identical tables reports clean on half a
# problem, and the clean line's own count is what makes the omission visible —
# it read 171 while the vocabulary had 220 rows.
#
# WHAT IT CHECKS. Two sources, because the id reaches the user two ways.
#
#   A. Rows in the macro tables — BOTH `action.rs` and `query.rs` — whose
#      variant the binary never names, so the door answers a derived refusal.
#      Their stamped task must be UNTICKED.
#   B. Task-id literals in the binary's production code — `Some("T###")`,
#      `task: "T###"`, `const …TASK: &str = "T###"`, and any task id inside a
#      string literal, which is how two `declined(…)` sentences got through the
#      first pattern. Same rule.
#
# Column-0 `#[cfg(test)]` is stripped for (B), the same anchor
# `lint-action-arms.sh` uses and for the same reason: a literal inside a test
# module is fixture data, not a sentence anybody reads.
#
# WHY IT IS A RATCHET. Two sites survive as RECORDED below, each for a reason
# that is about the tree rather than about effort. The lint fails on:
#
#   1. an unrecorded citation naming a ticked task;
#   2. a recorded citation that no longer names a ticked task — stale, delete it;
#   3. a recorded citation whose snippet is no longer in the file — the code
#      moved and the record is pointing at nothing;
#   4. a record naming a blocking task that does not exist, or that is ticked.
#
# (2) and (3) are what stop this becoming somewhere to hide a stale id.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# **`tests.rs` is skipped by name.** The binary's unit tests moved out of
# `main.rs` on 2026-08-25 to get back under the 1 MB hygiene ceiling. This
# scan strips the column-0 `#[cfg(test)]` to find the production half — and
# in a file that *is* the test module there is no attribute to strip, so
# without this line 5,000 lines of fixtures would read as production and a
# test that constructs an Action would count as an arm.
python3 - <<'PYEOF'
import pathlib
import re
import sys

# ---------------------------------------------------------------------------
# The recorded citations: exact source snippet -> (blocking task or "", why)
#
# The key is the literal line as it appears in the file, so a record cannot
# outlive the code it describes — if the line changes, check (3) fires. Cite
# SYMBOLS in the prose, never line numbers; `lint-action-arms.sh` learned that
# the hard way and the reasoning is in its header.
# ---------------------------------------------------------------------------
RECORDED = {
    '"this is T084\'s fixture body. pickers, diffs and",': (
        "",
        "**Not a refusal at all** — it is the body of the sample buffer `T084`'s fixture writes, "
        "and the task id is inside prose a *fixture* prints rather than a sentence the editor "
        "says about itself. Recorded rather than excluded by a cleverer pattern, because the "
        "narrow pattern this lint started with is exactly what let two real refusals through: "
        "scanning every string and recording the handful that are not promises is the shape that "
        "fails safe. If the fixture text changes, check (3) fires and this entry goes.",
    ),
    'const RUNTIME_TASK: &str = "T021";': (
        "",
        "`T021` is genuinely the task that embeds the VM, and it is ticked — but this arm only "
        "fires for `Action::Runtime(Eval)` when the evaluator is `None`, which the shipping "
        "binary never does: `main`'s `dispatch` builds a real host on every route. So the "
        "sentence is unreachable rather than wrong, and re-stamping it would make the const's "
        "own prose false.\n"
        "    **This waited on `T103` and `T103` landed, which is the update.** That task "
        "reworked `door.rs::apply` so the verb route dispatches to the host — and it "
        "deliberately *kept* this arm, because the two branches answer different questions: "
        "with no evaluator there is no VM to hand source to, and naming `T021` is the honest "
        "answer to that. It has no creditor now and is not expected to gain one. What keeps it "
        "from rotting is that `door.rs`'s own tests are the caller — they pass `None` on "
        "purpose, which is the shape a unit test of this door has.",
    ),
    'return Outcome::Refused(Refusal::NotYetImplemented { task: "T058" });': (
        "",
        "**No creditor, and this one is a finding rather than a debt.** It refuses "
        "`SendMessage` when `anchors` is non-empty — deliberately, because the comment beside it "
        "is right that dropping an anchor silently is worse than refusing. But `T058` is ticked, "
        "and `1c` — the anchor chip riding a visual selection into the prompt — is the screen "
        "`T058` was accepted on. So the feature draws and refuses to send. Whether carrying "
        "anchors over ACP belongs to a new task or to an existing session one is a ruling, not a "
        "guess, and it is recorded as such in OPEN-QUESTIONS.md rather than being invented here.",
    ),
}

# ---------------------------------------------------------------------------
# The recorded capability rows: variant -> (blocking task or "", why)
#
# Check (A)'s half of the record. These are verbs the binary does not arm whose
# stamped task is ticked, and which cannot honestly be re-stamped because no
# task in the graph owns them. Both survivors are the same two entries
# `scripts/lint-action-arms.sh` records with no creditor, and the full
# mockup-by-mockup reasoning lives there rather than being copied here — one
# source of truth for why nobody owes this work.
# ---------------------------------------------------------------------------
RECORDED_ROWS = {
    "ExpandDiffContext": (
        "",
        "No creditor. `T066` is ticked and stamped here because it declared the verb; no mockup "
        "asks to see more lines around a hunk than `T066` already draws, so there is no later "
        "task to re-stamp it against. Re-stamping it at a task that does not want it would be a "
        "worse lie than the one this lint exists to catch. Reasoning in lint-action-arms.sh.",
    ),
    "RevertHunk": (
        "",
        "No creditor, and `T066` built the before-side that would have made the rich revert "
        "possible — reading the mockups against it is what closed the row instead: `6d`'s `dih` "
        "is the only revert key any screen draws, and it is `T026`'s delete operator over "
        "`T064`'s hunk text object, already reachable. Reasoning in lint-action-arms.sh.",
    ),
}

ACTIONS = pathlib.Path("crates/phosphor-core/src/action.rs")
QUERIES = pathlib.Path("crates/phosphor-core/src/query.rs")
TASKS = pathlib.Path("docs/TASKS.md")
SOURCES = [
    pathlib.Path("crates/phosphor/src"),
    pathlib.Path("crates/phosphor-steel/src"),
]

failures = []


def fail(message):
    failures.append(message)


# -- the checklist ----------------------------------------------------------
tasks_text = TASKS.read_text(encoding="utf-8")
ticked = set(re.findall(r"^- \[x\] \*\*(T\d+|V\d+)", tasks_text, re.M))
known = ticked | set(re.findall(r"^- \[ \] \*\*(T\d+|V\d+)", tasks_text, re.M))
if not ticked or not known:
    fail(f"read no tasks from {TASKS} — the checklist's shape moved. Fix the pattern.")

# -- (A) the derived refusals -----------------------------------------------
ROW = re.compile(
    r'^\s+([A-Z][A-Za-z0-9]*) = "([a-z0-9-]+)" \[\s*(\w+)\s*/\s*"([^"]+)"',
    re.M,
)
declared = ROW.findall(ACTIONS.read_text(encoding="utf-8"))
if len(declared) < 100:
    fail(
        f"read only {len(declared)} action variants from {ACTIONS} — the macro's shape moved "
        "and check (A) is now checking nothing. Fix the pattern, do not delete the lint."
    )

# **`query.rs` carries the same table, and leaving it out hid twelve rows.**
#
# This lint was written on 2026-08-24 against refusals that name a finished
# task, and it read capability rows only. `query.rs` stamps its rows in exactly
# the same shape — `Capabilities = "capabilities" [S2 / "T024"]` — and an
# unanswered query reaches the caller through `QueryError::NotYetImplemented`,
# which renders the same sentence. So half the surface was unchecked, and the
# half that was unchecked had twelve stale rows in it: `(capabilities)` answered
# *"not built yet — T024 builds it"* against a ticked `T024`, measured on the
# built binary. `T111` is the creditor they were re-stamped to.
#
# A lint that covers one of two identical tables reports clean on half a
# problem. Both are read here, and the count in the clean line says both.
queried = ROW.findall(QUERIES.read_text(encoding="utf-8"))
if len(queried) < 20:
    fail(
        f"read only {len(queried)} query rows from {QUERIES} — the macro's shape moved and "
        "half of check (A) is now checking nothing. Fix the pattern, do not delete the lint."
    )
declared = declared + queried

# The binary's production text, test modules stripped. Same anchor as
# `lint-action-arms.sh`: column-0 `#[cfg(test)]` is the test *module*; the
# indented ones are attributes on test-only helpers inside real impls.
production = []
for root in SOURCES:
    for path in sorted(root.glob("*.rs")):
        if path.name == "tests.rs":
            continue
        text = path.read_text(encoding="utf-8")
        module = re.search(r"^#\[cfg\(test\)\]", text, re.M)
        production.append((path, text[: module.start()] if module else text))
if sum(len(t) for _, t in production) < 10_000:
    fail("read almost no production source — the crate layout moved.")

body = "\n".join(text for _, text in production)

# **A bare word is not an arm, and reading it as one hid eight rows.**
#
# This was `re.search(rf"\b{variant}\b", body)` — *"does the variant's name
# appear anywhere in production?"* — and the answer is yes for every variant
# whose name is also an ordinary Rust identifier. `Theme`, `Buffer`, `Buffers`,
# `Cursor`, `Selection`, `Viewport`, `Keymap` and `ReviewBlock` are all types,
# fields or locals in `main.rs` (`Buffers` alone occurs 28 times), so all eight
# read as armed while `(theme)`, `(buffers)` and `(cursor …)` each answered
# *"not built yet"* against a **ticked** task — measured on the built binary
# on 2026-08-25, the same way `T111`'s twelve were found a day earlier.
#
# The narrower pattern is the one the code actually writes: an arm names a
# variant through its domain enum — `BufferQuery::Buffers`,
# `MotionAction::SetCursor` — so the name has to be preceded by a domain type
# ending in `Query` or `Action`. `phosphor_ui::theme::Theme` does not match
# that and `UiQuery::Theme` does, which is exactly the distinction the bare
# word could not draw.
#
# **This is the third time this lint has been widened by measuring the binary
# rather than reading the lint**, and the shape repeats: check (A) was
# actions-only, then both tables, and now both tables read precisely. A lint
# that answers *"probably armed"* reports clean on a real refusal.
ARM = re.compile(
    r"\w*(?:Query|Action)::(" + "|".join(sorted({v for v, _, _, _ in declared})) + r")\b"
)
named = set(ARM.findall(body))

unarmed = {
    variant: (verb, task)
    for variant, verb, _phase, task in declared
    if variant not in named and task in ticked
}

for variant, (verb, task) in sorted(unarmed.items()):
    if variant in RECORDED_ROWS:
        continue
    fail(
        f"`{verb}` is not armed, so calling it answers \"not built yet — {task} builds it\" — "
        f"and `{task}` is ticked. The reader is sent to finished work.\n"
        f"    Re-stamp `{variant}`'s row in {ACTIONS} against the task that will really build "
        f"it, or arm the verb. If neither is true yet, {TASKS} needs a task that owns it — see "
        f"OPEN-QUESTIONS.md §18."
    )

# -- (B) the literals -------------------------------------------------------
# Any task id inside a STRING LITERAL in production code. The narrow form —
# `Some("T###")`, `task: "T###"` — was the first version and it missed two
# sites on the day it was written, because a refusal does not have to travel
# through `Refusal::NotYetImplemented` to reach a reader: `declined("search is
# T058's other half")` and `declined("that anchor is in another file — T056
# opens it")` are the same promise in a plain sentence, and `T058` and `T056`
# are both ticked. `just gate` is what found them — the pty tests that assert
# these frames went red while this lint said clean.
#
# Comment lines are skipped: a `T0xx` in a comment is prose about the build,
# `lint-doc-claims.sh` already checks that it names a real task, and nobody
# reads it out of a running editor.
LITERAL = re.compile(r'"[^"]*\b(T\d+)\b[^"]*"')

seen_snippets = set()
for path, text in production:
    for line in text.split("\n"):
        stripped = line.strip()
        if stripped.startswith("//"):
            continue
        match = LITERAL.search(line)
        if not match:
            continue
        task = next(g for g in match.groups() if g)
        snippet = stripped
        seen_snippets.add(snippet)
        if task not in known:
            fail(f"{path} cites `{task}`, which is not a task in {TASKS}:\n    {snippet}")
            continue
        if task not in ticked or snippet in RECORDED:
            continue
        fail(
            f"{path} builds a refusal naming `{task}`, which is ticked — the sentence a caller "
            f"reads is \"not built yet — {task} builds it\" about work that shipped.\n"
            f"    {snippet}\n"
            f"    Point it at the task that will really build it, or record it in "
            f"scripts/lint-refusal-tasks.sh's RECORDED table with a reason."
        )

# -- the row record can only shrink -----------------------------------------
declared_variants = {variant for variant, _, _, _ in declared}
for variant, (blocker, why) in sorted(RECORDED_ROWS.items()):
    if variant not in declared_variants:
        fail(f"RECORDED_ROWS names `{variant}`, which {ACTIONS} does not declare. Remove it.")
        continue
    if variant not in unarmed:
        fail(
            f"RECORDED_ROWS still lists `{variant}`, but it is armed now or its row is stamped "
            f"at an unticked task. Either way the refusal is honest — delete the entry."
        )
    if blocker:
        if blocker not in known:
            fail(f"RECORDED_ROWS says `{variant}` waits on `{blocker}`, not a task in {TASKS}.")
        elif blocker in ticked:
            fail(
                f"RECORDED_ROWS says `{variant}` waits on `{blocker}` — and `{blocker}` is "
                f"ticked now. Re-stamp the row or arm the verb."
            )

# -- the snippet record can only shrink -------------------------------------
for snippet, (blocker, why) in sorted(RECORDED.items()):
    if snippet not in seen_snippets:
        fail(
            f"RECORDED holds a snippet no production source contains any more — the code moved "
            f"or the citation went with it, and this record now points at nothing. Remove it.\n"
            f"    {snippet}"
        )
        continue
    match = LITERAL.search(snippet)
    task = next(g for g in match.groups() if g)
    if task not in ticked:
        fail(
            f"RECORDED holds `{snippet}`, whose task `{task}` is not ticked — so it is not a "
            f"stale promise and nothing above checks this entry any more. Remove it; the "
            f"refusal is honest now."
        )
    if blocker:
        if blocker not in known:
            fail(f"RECORDED says `{snippet}` waits on `{blocker}`, which is not a task in {TASKS}.")
        elif blocker in ticked:
            fail(
                f"RECORDED says `{snippet}` waits on `{blocker}` — and `{blocker}` is ticked "
                f"now. The excuse expired: fix the citation or re-record it against whatever "
                f"actually blocks it."
            )

# -- report -----------------------------------------------------------------
if failures:
    print("lint-refusal-tasks: FAILED")
    print()
    for message in failures:
        print(f"  {message}")
        print()
    sys.exit(1)

# Counted the way the check counts — comment lines skipped — so the number in
# the clean line is the number of sentences this lint actually governs.
cited = len(seen_snippets)
records = len(RECORDED) + len(RECORDED_ROWS)
owed = sum(1 for _, (blocker, _) in list(RECORDED.items()) + list(RECORDED_ROWS.items()) if not blocker)
print(
    f"lint-refusal-tasks: clean — {len(declared)} capability and query rows, {cited} literals in "
    f"production, {records} recorded ({owed} with no task that closes them)"
)
PYEOF
