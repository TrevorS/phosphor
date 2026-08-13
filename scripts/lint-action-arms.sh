#!/usr/bin/env bash
# Structural lint: a ticked task may not declare a mutation the binary never applies.
#
# This exists because of `T016`. It is ticked in `docs/TASKS.md` — "Folds and
# whitespace marks" — on a *done when* of "screen 8e's fold and whitespace
# details reproduce", and that screen reproduced: `crates/phosphor/tests/
# screen_8e.rs` builds a `Tree` by hand and renders it. The whitespace half was
# genuinely wired into the loop. The fold half never was: no `z` binding, no arm
# for `Action::View(SetFold)`, so `za` fell to `NotYetImplemented` and typing it
# ran vim's plain `a`. That survived `CP-1` and two windows after it, because
# every gate asked "does the snapshot match" and none asked "can you press the
# key".
#
# The same shape then repeated across Window D at a larger scale — the `SPC`
# leader popup, the unknown-key hint and undo all shipped built, tested, ticked
# and uncomposed — which is what prompted the backwards audit that wrote this.
#
# WHAT IT CHECKS. `crates/phosphor-core/src/action.rs` declares every mutation
# through a macro DSL that already stamps its owning task:
#
#     SetFold = "set-fold" [S3 / "T016" / Allow]
#
# So the check needs no new bookkeeping: parse the variant and its task, keep
# the ones whose task is ticked in `docs/TASKS.md`, and require that the binary
# at minimum *names* the variant. Naming it is a weak bar deliberately — this
# lint proves an arm exists, not that the arm is right; that is a test's job.
# The bar it actually enforces is that nobody can tick a task whose mutation the
# application has never heard of.
#
# WHY IT IS A RATCHET AND NOT A PASS/FAIL. There were 13 such gaps on the tree
# the day this was written, several of them legitimately waiting on a phase that
# has not happened. A lint introduced as a plain failure would have to either be
# switched off or lie. So the gaps are RECORDED below, and the lint fails on:
#
#   1. an unrecorded gap — a ticked task declaring a mutation nothing applies;
#   2. a recorded gap that is now reachable — the record is stale, delete it;
#   3. a recorded gap whose blocking task has since been ticked — the excuse
#      expired, and the arm is now owed;
#   4. a record naming a task that does not exist.
#
# (2) and (3) are what stop the list becoming a place to hide things. It can
# only shrink on its own; growing it is an edit somebody has to justify.
#
# WHAT IT DELIBERATELY DOES NOT CATCH. A variant named only inside the binary's
# own `#[cfg(test)]` module is treated as absent, because a test that calls
# `editing.apply(&Action::View(SetFold))` directly is exactly the proof that
# missed `T016` — it exercises the arm while skipping keystroke → keymap →
# machine → Action. Only the column-0 `#[cfg(test)]` (the test *module*) is
# stripped; the indented ones at `main.rs:650` and `:1748` are attributes on
# test-only helpers inside real impls, and cutting there would remove 2,300
# lines of live dispatch and report 42 false gaps. That mistake was made while
# writing this, which is why the anchor is spelled out.
#
# It also cannot see a mutation that is reachable by some path other than an
# Action arm — `set-soft-wrap` is the live example, where the feature works
# through `--soft-wrap` and `host.flag("soft-wrap")` at `main.rs:891` while the
# verb answers a refusal. That is why every record below says what is true
# rather than just "not yet".

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - <<'PYEOF'
import pathlib
import re
import sys

# ---------------------------------------------------------------------------
# The recorded gaps: variant -> (blocking task or "", why, checked-against)
#
# `blocking task` must be a task that EXISTS and is NOT ticked; when it is
# ticked this lint fails and the arm becomes owed. An empty blocking task means
# nothing in the graph owns the work — it is a debt with no creditor, and those
# are the entries worth reading twice.
# ---------------------------------------------------------------------------
# Citations here name SYMBOLS, never line numbers. The first version of this
# table cited four (`main.rs:794` for the theme local, `:924` for the float slot,
# `:891` for the soft-wrap flag, `text.rs:600` for `char_right_operand`) and all
# four had drifted within a day, because concurrent agents were editing those
# files while this ran. A wrong line number inside a lint is worse than one in a
# comment: it reads as authoritative. TEAM.md's concurrency rules say cite
# symbols; this is that rule applied to itself.
RECORDED = {
    "SetTheme": ("T092", "`:theme <slug>` is bound in runtime/keymaps.scm and answers a refusal. "
                         "The theme is an immutable local — `builtin(&cli.theme)` in `main` — "
                         "baked into each `Editor` at construction, so runtime switching is a "
                         "rebuild path, not an arm. `--theme <slug>` works and is what the eight "
                         "theme tapes use."),
    "ReloadTheme": ("T092", "Same rebuild path as `SetTheme`; re-reading a theme file also needs "
                            "a user-theme path, and `main` only ever calls `builtin()`."),
    "OpenFloat": ("T093", "The float slot exists (`FloatSlot::empty` in `main`) and the boot "
                          "report opens one, but no verb reaches it, so Steel and MCP cannot "
                          "open a float."),
    "CloseFloat": ("T093", "The paired half of `OpenFloat`; `esc` closes a float through the "
                           "input machine, not through this verb."),
    "CloseAllFloats": ("T093", "Same seam as `CloseFloat`."),
    "LoadRuntimeFile": ("T094", "Evaluating a further `.scm` after boot. The REPL evaluates forms "
                                "and `init.scm` reads the load order once at startup; neither is "
                                "this."),
    "ReloadRuntime": ("T094", "Re-booting the editor layer without restarting. Nothing rebuilds a "
                              "`Runtime` in place today."),
    "UndoToCheckpoint": ("T095", "`UndoTree::goto` and `CheckpointId` both exist and `Timeline` "
                                 "owns the tree; nothing routes a checkpoint id to it."),
    "CompactHistory": ("T095", "`journal.rs` implements compaction and proves it under a real "
                               "`SIGKILL`; nothing triggers it, so a history only grows."),
    "SetSoftWrap": ("T096", "Soft wrap is reachable and works — `--soft-wrap` and "
                            "`host.flag(\"soft-wrap\")` — but the verb is not applied, so it "
                            "cannot be toggled from Steel, MCP or the CLI door. `T081` is ticked; "
                            "this is `T016`'s shape and was found by the same audit."),
    "SetVirtualTextVisible": ("T041", "Collapsing a virtual-text rail addresses it by owning "
                                      "region, and regions arrive with the store at `T041`. The "
                                      "one live rail today is the unknown-key hint, which is "
                                      "unowned by design (`Node::VirtualText.owner` is None)."),
    # The two below disagree with their own declared task, which is an attribution
    # question rather than a missing arm — see docs/OPEN-QUESTIONS.md.
    "Jump": ("T042", "The jumplist. `action.rs` declares `jump` as `[S3 / \"T026\"]`, but `T026` "
                     "is ticked and its own coverage diff records marks and the jumplist as "
                     "deliberately deferred because *anchors are the store's*. So the row's task "
                     "and the reason it is unbuilt name different tasks. Consequence, found by "
                     "the wiring agent: a truthful refusal derived from the row would read "
                     "*T026 builds it*, which is false."),
    "ApplyEdits": ("", "A batch of edits applied as one undo group — the shape an agent writes "
                       "through. `T029`'s tree supports it (`record_batch`); no caller exists "
                       "until there is a session. Declared `[S3 / \"T029\"]`, and `T029` is "
                       "ticked and did not build it — the same attribution question as `Jump`, "
                       "which is why this one still has no blocker."),
}

ACTIONS = pathlib.Path("crates/phosphor-core/src/action.rs")
TASKS = pathlib.Path("docs/TASKS.md")
BIN = pathlib.Path("crates/phosphor/src")

failures = []


def fail(message):
    failures.append(message)


# -- the vocabulary ---------------------------------------------------------
declared = re.findall(
    r'^\s+([A-Z][A-Za-z0-9]*) = "([a-z0-9-]+)" \[\s*(\w+)\s*/\s*"([^"]+)"',
    ACTIONS.read_text(encoding="utf-8"),
    re.M,
)
if len(declared) < 100:
    fail(
        f"read only {len(declared)} action variants from {ACTIONS} — the macro's shape moved "
        "and this lint is now checking nothing. Fix the pattern, do not delete the lint."
    )

# -- the checklist ----------------------------------------------------------
tasks_text = TASKS.read_text(encoding="utf-8")
ticked = set(re.findall(r"^- \[x\] \*\*(T\d+|V\d+)", tasks_text, re.M))
known = ticked | set(re.findall(r"^- \[ \] \*\*(T\d+|V\d+)", tasks_text, re.M))
if not ticked:
    fail(f"read no ticked tasks from {TASKS} — the checklist's shape moved.")

# -- what the binary actually names -----------------------------------------
#
# Column-0 `#[cfg(test)]` only: that is the test *module*. See the header.
body = []
for path in sorted(BIN.glob("*.rs")):
    text = path.read_text(encoding="utf-8")
    module = re.search(r"^#\[cfg\(test\)\]", text, re.M)
    body.append(text[: module.start()] if module else text)
body = "\n".join(body)
if len(body) < 10_000:
    fail(f"read only {len(body)} bytes of {BIN}/*.rs — the binary's layout moved.")

named = {variant for variant, _, _, _ in declared if re.search(rf"\b{variant}\b", body)}

# -- the three ways this can be wrong ---------------------------------------
unreachable = {
    variant: (verb, task)
    for variant, verb, _phase, task in declared
    if task in ticked and variant not in named
}

for variant, (verb, task) in sorted(unreachable.items()):
    if variant in RECORDED:
        continue
    fail(
        f"`{task}` is ticked and declares `{variant}` (\"{verb}\"), and nothing in "
        f"{BIN}/*.rs names it — the verb answers `NotYetImplemented` and any key bound to "
        f"it does nothing.\n"
        f"    Wire it, or record it in scripts/lint-action-arms.sh's RECORDED table with a "
        f"reason and, if some task will close it, that task's id."
    )

for variant, (blocker, why) in sorted(RECORDED.items()):
    declared_here = [d for d in declared if d[0] == variant]
    if not declared_here:
        fail(f"RECORDED names `{variant}`, which {ACTIONS} does not declare. Remove the record.")
        continue
    task = declared_here[0][3]
    if variant not in unreachable and task in ticked:
        fail(
            f"RECORDED still lists `{variant}`, but the binary names it now. The record is "
            f"stale — delete the entry. ({why[:60]}…)"
        )
    if blocker:
        if blocker not in known:
            fail(f"RECORDED says `{variant}` waits on `{blocker}`, which is not a task in {TASKS}.")
        elif blocker in ticked:
            fail(
                f"RECORDED says `{variant}` waits on `{blocker}` — and `{blocker}` is ticked now. "
                f"The arm is owed: wire `{variant}`, or re-record it against whatever actually "
                f"blocks it."
            )

# -- report -----------------------------------------------------------------
if failures:
    print("lint-action-arms: FAILED")
    print()
    for message in failures:
        print(f"  {message}")
        print()
    sys.exit(1)

owed = sum(1 for v, (b, _) in RECORDED.items() if not b)
print(
    f"lint-action-arms: clean — {len(declared)} declared mutations, {len(ticked)} ticked tasks, "
    f"{len(RECORDED)} recorded gaps ({owed} with no task that closes them)"
)
PYEOF
