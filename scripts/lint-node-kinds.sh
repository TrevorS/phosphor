#!/usr/bin/env bash
# Structural lint: a node kind the interpreter draws that the shipped editor
# composes from nowhere.
#
# The sibling of `scripts/lint-action-arms.sh`, and the same defect one layer
# over. That one watches *mutations*: a ticked task may not declare an `Action`
# the binary never applies. Nothing watched *node kinds* until this, and the gap
# is not hypothetical — `Node::KeyHints` at `Density::Help` was composed by
# nothing while `crates/phosphor/tests/screen_6d.rs` hand-built a tree, rendered
# it, and matched a golden frame. The frame was real; `:help` in the running
# binary answered a refusal. The snapshot proved the widget and said nothing
# about the editor, which is exactly what `T016` taught and exactly what got
# repeated.
#
# It existed *before* the LSP phase, because `S4` builds `Node::Completion` and
# `Node::Signature` — two more kinds that were composed by nothing the day this
# was written. The lint that catches the shape has to precede the window that
# would otherwise repeat it, and both entries are gone from RECORDED below:
# `S4`'s wiring pass composes them in `crates/phosphor/src/main.rs`
# (`passive_float`), which is the outcome this file was betting on.
#
# WHAT IT CHECKS. `crates/phosphor-core/src/view.rs` declares every kind through
# a macro DSL that carries both spellings at once:
#
#     Gutter = "gutter", "the 1-cell state column alone, …" { … }
#
# — the variant Rust composes with, and the tag Steel does. So the question is:
# does any *shipped* source name this kind, in either language?
#
# TWO LANGUAGES, AND THAT IS THE DIFFERENCE FROM THE ACTION LINT. The binary
# composes a handful of kinds directly; most of what the shipped editor draws is
# composed in Steel, in `runtime/*.scm`, through the generated `view/<tag>`
# constructors — `phosphor-steel`'s `view` module walks `<Node as Wire>::TYPE`
# and installs one procedure per kind, so *every* kind is callable from scheme
# and the only question that means anything is whether the shipped layer calls
# one. Naming it is a weak bar on purpose, the same as the sibling's: this proves
# a composition exists, not that it is right.
#
# WHERE COMPOSITION LIVES, and why the list below is a list. Four places, three
# crates: the binary's own `draw`; the boot-report float and the REPL surface,
# both composed in `phosphor-steel` as plain `phosphor_core::view` data; the
# unknown-key hint, composed in `phosphor-ui`'s `unknown_key` module because it
# is a strip a widget hands back rather than a widget it draws; and
# `runtime/*.scm`. Two files name kinds without composing any and are excluded by
# name — `phosphor-core`, which declares them, and `phosphor-ui`'s `interpret`,
# which is one match over all thirty. Scanning that file would mark every kind
# composed and this lint would check nothing.
#
# A list like that rots silently, so it does not get to: any *other* `src/` file
# that names a variant fails the lint as an unknown site. The fix is one line —
# add it to `COMPOSES` if it composes, to `CONSUMES` if it destructures — and the
# failure is loud rather than a quietly shrinking ratchet.
#
# WHY IT IS A RATCHET AND NOT A PASS/FAIL. Fourteen kinds were composed by
# nothing on the tree the day this was written, most of them legitimately waiting
# on a phase that has not happened. A plain failure would have to be switched off
# or lie, so the gaps are RECORDED below and the lint fails four ways, exactly as
# its sibling does:
#
#   1. an unrecorded gap — a declared kind nothing in the shipped configuration
#      composes;
#   2. a recorded gap that is now composed — the record is stale, delete it;
#   3. a recorded gap whose blocking task has since been ticked — the excuse
#      expired, and the composition is owed;
#   4. a record naming a task that does not exist.
#
# (2) and (3) are what stop the list becoming a place to hide things.
#
# WHAT IT DELIBERATELY DOES NOT CATCH.
#
#   * A kind composed from the REPL, or from a user's own `.scm`. This is about
#     what we *ship*, the same way the Action lint is about what the binary
#     applies. `view/picker` typed at `λ` is a feature, not a composition.
#   * A kind reached some way other than a node — `Node::Gutter` is the live
#     example, where the state column is on screen every frame because
#     `BufferView` draws it as its own left column, outside the tree entirely.
#     That is why every record below says what is *true* rather than just
#     "not yet". `Node::Buffer` was the other one and stopped being: `T088`'s
#     collapse deleted the widget path, so the buffer is a composition now and
#     the column arrives inside it.
#
# TWO TRAPS THIS SHAPE INVITES, both hit while writing it.
#
#   * **Comments name kinds.** `crates/phosphor/src/main.rs` says
#     `view::Node::Prompt` in a comment at its ex-line draw site, and
#     `runtime/statusline.scm` says ``view/key-hints`` in one explaining why the
#     hints are a plain label instead. Read naively, both kinds look composed and
#     one of them is `T058`'s whole gap. So comments are stripped in both
#     languages before anything is matched — Rust's `//` and `/* */` with raw
#     strings honoured, Steel's `;` outside a string.
#   * **Tests compose freely, and that is the defect, not the proof.** A test
#     that hand-builds a `Tree` and renders it is precisely what missed
#     `Density::Help`. Only the COLUMN-0 `#[cfg(test)]` is stripped — the test
#     *module*. The sibling's header spells out why, and it is the same anchor
#     here: an indented `#[cfg(test)]` is an attribute on a test-only helper
#     inside a real impl, and cutting there removed 2,300 lines of live dispatch
#     and reported 42 false gaps the one time it was tried.
#
# CITATIONS BELOW NAME SYMBOLS, NEVER LINE NUMBERS. Four line numbers inside the
# Action lint had drifted within a day, because concurrent agents were editing
# those files while it ran, and a wrong line number in a lint reads as
# authoritative. Same rule here.

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
# The recorded gaps: variant -> (blocking task or "", why)
#
# `blocking task` must be a task that EXISTS and is NOT ticked; when it is
# ticked this lint fails and the composition becomes owed. An empty blocking
# task means nothing in the graph owns the work — a debt with no creditor, and
# those are the entries worth reading twice.
# ---------------------------------------------------------------------------
RECORDED = {
    # `Pane` and `Buffer` were recorded here against `T088` and are gone: the
    # frame loop composes `Node::Pane { … child: Node::Buffer { … } }` and
    # `draw` renders it, so the widget path that drew `BufferView` outside the
    # tree no longer exists. Deleted in the commit that landed the composition,
    # because this lint fails four ways on a stale row and one of them is
    # "the shipped configuration composes it now".
    "Gutter": ("", "Same as `Buffer` was, and it has no creditor. The state column ships as "
                   "`BufferView`'s left column — `T031` is ticked and built it — and this kind "
                   "is the column *without* the editor, for a surface that wants it. No task in "
                   "the graph names such a surface, so nothing closes this entry. The "
                   "interpreter already draws the tag (`crate::gutter`), so the gap is the "
                   "composition alone. **`T045`'s picker preview was checked and is not it**: "
                   "`2a` draws that pane as diff lines, not as a buffer with a state column, so "
                   "the preview is `Node::Diff`'s shape (`T063`). Recorded by the pre-window "
                   "scout because the guess is plausible and the drawing settles it. **`T088`'s "
                   "collapse was checked and is not it either**: a tree-composed `Node::Buffer` "
                   "renders `BufferView` with its own `.state_column(…)`, so the pane got the "
                   "column without composing this kind, and composing one beside it would draw "
                   "the column twice."),
    "Spinner": ("", "Same shape as `Gutter` and it has no creditor either. A spinner turns "
                    "twice over in the shipped editor — the statusline (`T051`, "
                    "`Interpreter::session`, off `Node::Session`'s own `since`) and now the "
                    "transcript (`T054`, `crate::transcript::TranscriptPane::row`, off "
                    "`Turn::since`, the same `SPINNER_PERIOD_MS` cadence read through "
                    "`status_line::Spinner` so the two cannot drift into two rhythms) — and "
                    "neither composes this tag. **Recorded against `T051`, then re-recorded "
                    "against `T054` when the first excuse expired, and both were the wrong "
                    "creditor**: ticking either task turned out to add a second inline arm "
                    "rather than a first composition, because a `Node::Spinner` nested inside "
                    "`Node::Session` or `Node::Transcript` would draw a spinner beside the "
                    "thing already drawing one. No surface in the graph wants a *standalone* "
                    "spinner with no session and no turn behind it, so there is no third task "
                    "to guess at. If one appears, this is where its name goes."),
    "Elapsed": ("", "The other half, and the same finding: `T051`'s statusline and `T054`'s "
                    "transcript both render an elapsed counter inline off a `since` they "
                    "already carry, and neither composes this tag. No creditor for the same "
                    "reason `Spinner` has none."),
    # `Diff` was recorded here against `T063`, re-pointed at `T066`, and is
    # gone: `runtime/review.scm` composes `Node::Diff` in the `review` float, so
    # `8b` is a screen a person opens rather than a widget with a test. The
    # re-pointing lasted one task, which is what this table is supposed to do —
    # a row can only shrink, and the reason it shrank is that somebody built the
    # composition rather than finding a better task to blame.
    "Watch": ("T076", "`WatchOverlay` — the `◉ ⇒` stream, which renders *through* a "
                      "`VirtualText` row. That kind is composed (the unknown-key hint); this "
                      "one is the watch's own formatting and is deferred in the interpreter."),
}

VIEW = pathlib.Path("crates/phosphor-core/src/view.rs")
TASKS = pathlib.Path("docs/TASKS.md")
RUNTIME = pathlib.Path("runtime")

# Rust that composes a node for the shipped frame.
COMPOSES = [
    pathlib.Path("crates/phosphor/src"),
    pathlib.Path("crates/phosphor-steel/src"),
    pathlib.Path("crates/phosphor-ui/src/unknown_key.rs"),
]
# Rust that names kinds without composing one: the declaration, and the one
# match that draws them all.
CONSUMES = [
    pathlib.Path("crates/phosphor-core/src"),
    pathlib.Path("crates/phosphor-ui/src/interpret.rs"),
]

# `impl Node`'s constructor helpers: a composition may write `Node::line([…])`
# rather than `Node::Line { … }`, and both are compositions. Guarded below —
# a helper that is renamed must be renamed here too, or this lint quietly stops
# seeing the kind it builds.
HELPERS = {"line": "Line", "split": "Split"}

failures = []


def fail(message):
    failures.append(message)


# -- stripping ---------------------------------------------------------------


def strip_rust_comments(text):
    """Rust with `//`, `/* */` and doc comments removed. String literals —
    raw ones included — are preserved, so a `//` inside one is not a comment."""
    out = []
    i, n = 0, len(text)
    while i < n:
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
            continue
        if text.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif text.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
            continue
        raw = re.match(r'r(#*)"', text[i:])
        if raw:
            close = '"' + raw.group(1)
            end = text.find(close, i + len(raw.group(0)))
            i = n if end < 0 else end + len(close)
            out.append(" ")
            continue
        if text[i] == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
            i += 1
            out.append(" ")
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def strip_test_module(text):
    """Everything from the COLUMN-0 `#[cfg(test)]` on. Column 0 only — see the
    header, and the sibling's."""
    module = re.search(r"^#\[cfg\(test\)\]", text, re.M)
    return text[: module.start()] if module else text


def strip_steel_comments(text):
    """Scheme with `;`-to-end-of-line removed outside string literals."""
    out = []
    i, n = 0, len(text)
    while i < n:
        if text[i] == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
            i += 1
            out.append(" ")
            continue
        if text[i] == ";":
            end = text.find("\n", i)
            i = n if end < 0 else end
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def shipped_rust(path):
    return strip_rust_comments(strip_test_module(path.read_text(encoding="utf-8")))


# -- the vocabulary ----------------------------------------------------------
view_text = VIEW.read_text(encoding="utf-8")
table = re.search(r"^nodes! \{$", view_text, re.M)
if not table:
    fail(f"{VIEW} has no `nodes!` invocation at column 0 — the macro moved and this lint is "
         "checking nothing. Fix the pattern, do not delete the lint.")
    declared = []
else:
    declared = re.findall(
        r'^    ([A-Z][A-Za-z0-9]*) = "([a-z0-9-]+)", ',
        view_text[table.start():],
        re.M,
    )
if len(declared) < 25:
    fail(f"read only {len(declared)} node kinds from {VIEW} — the macro's shape moved and this "
         "lint is now checking nothing. Fix the pattern, do not delete the lint.")

for helper, variant in HELPERS.items():
    if not re.search(rf"pub fn {helper}\(", view_text):
        fail(f"`Node::{helper}()` is gone from {VIEW}, and this lint counts it as composing "
             f"`Node::{variant}`. Update HELPERS.")

# -- the checklist -----------------------------------------------------------
tasks_text = TASKS.read_text(encoding="utf-8")
ticked = set(re.findall(r"^- \[x\] \*\*(T\d+|V\d+)", tasks_text, re.M))
known = ticked | set(re.findall(r"^- \[ \] \*\*(T\d+|V\d+)", tasks_text, re.M))
if not ticked:
    fail(f"read no ticked tasks from {TASKS} — the checklist's shape moved.")

# -- what the shipped configuration composes ---------------------------------
sources = []
for entry in COMPOSES:
    if entry.is_dir():
        sources.extend(
            path for path in sorted(entry.rglob("*.rs")) if path.name != "tests.rs"
        )
    elif entry.is_file():
        sources.append(entry)
    else:
        fail(f"COMPOSES names {entry}, which does not exist. The composition sites moved.")

rust = "\n".join(shipped_rust(path) for path in sources)
if len(rust) < 10_000:
    fail(f"read only {len(rust)} bytes of composing Rust — the binary's layout moved.")
if re.search(r"use [\w:]*view::Node::\{", rust):
    fail("a composing source imports `Node`'s variants, so a composition can spell one without "
         "`Node::`. This lint matches `Node::<Variant>` and would read that kind as a gap. "
         "Spell it `Node::<Variant>` at the composition, or teach this lint the import.")

scm_sources = sorted(RUNTIME.glob("*.scm"))
scm = "\n".join(strip_steel_comments(p.read_text(encoding="utf-8")) for p in scm_sources)
if not re.search(r"\(view/[a-z0-9-]+", scm):
    fail(f"no `view/…` constructor is called anywhere in {RUNTIME}/*.scm — either the editor "
         "layer composes nothing, or the constructor prefix moved (`phosphor-steel`'s "
         "`view::PREFIX`).")

# -- the drift guard ---------------------------------------------------------
#
# A composition site this lint does not know about would go unseen; a
# destructuring site it does not know about would read as a composition. Both
# fail here rather than silently.
listed = [entry.resolve() for entry in COMPOSES + CONSUMES]
for path in sorted(pathlib.Path("crates").glob("*/src/**/*.rs")):
    resolved = path.resolve()
    if any(resolved == entry or entry in resolved.parents for entry in listed):
        continue
    body = shipped_rust(path)
    hits = sorted(v for v, _ in declared if re.search(rf"\bNode::{v}\b", body))
    if hits:
        fail(f"{path} names {', '.join(f'`Node::{h}`' for h in hits)} outside its test module, "
             "and is on neither of this lint's lists. If it composes, add it to COMPOSES; if it "
             "destructures, add it to CONSUMES. Leaving it off means this lint is measuring the "
             "wrong tree.")

# -- the gaps ----------------------------------------------------------------
composed = set()
for variant, tag in declared:
    if re.search(rf"\bNode::{variant}\b", rust):
        composed.add(variant)
    elif re.search(rf"\(view/{re.escape(tag)}(?![a-z0-9-])", scm):
        composed.add(variant)
for helper, variant in HELPERS.items():
    if re.search(rf"\bNode::{helper}\(", rust):
        composed.add(variant)

uncomposed = {variant: tag for variant, tag in declared if variant not in composed}

for variant, tag in sorted(uncomposed.items()):
    if variant in RECORDED:
        continue
    fail(
        f"`Node::{variant}` (\"{tag}\") is declared and the interpreter has an arm for it, and "
        f"nothing in the shipped configuration composes one — not `crates/phosphor/src`, not "
        f"`crates/phosphor-steel/src`, not `runtime/*.scm`. It is unreachable in the editor we "
        f"ship, however good its widget test is.\n"
        f"    Compose it, or record it in scripts/lint-node-kinds.sh's RECORDED table with a "
        f"reason and, if some task will close it, that task's id."
    )

for variant, (blocker, why) in sorted(RECORDED.items()):
    if variant not in {v for v, _ in declared}:
        fail(f"RECORDED names `Node::{variant}`, which {VIEW} does not declare. Remove the record.")
        continue
    if variant not in uncomposed:
        fail(
            f"RECORDED still lists `Node::{variant}`, but the shipped configuration composes it "
            f"now. The record is stale — delete the entry. ({why[:60]}…)"
        )
    if blocker:
        if blocker not in known:
            fail(f"RECORDED says `Node::{variant}` waits on `{blocker}`, which is not a task in "
                 f"{TASKS}.")
        elif blocker in ticked:
            fail(
                f"RECORDED says `Node::{variant}` waits on `{blocker}` — and `{blocker}` is "
                f"ticked now. The composition is owed: compose `Node::{variant}`, or re-record "
                f"it against whatever actually blocks it."
            )

# -- report ------------------------------------------------------------------
if failures:
    print("lint-node-kinds: FAILED")
    print()
    for message in failures:
        print(f"  {message}")
        print()
    sys.exit(1)

owed = sum(1 for _, (blocker, _) in RECORDED.items() if not blocker)
print(
    f"lint-node-kinds: clean — {len(declared)} node kinds, {len(composed)} composed by the "
    f"shipped configuration, {len(RECORDED)} recorded gaps ({owed} with no task that closes them)"
)
PYEOF
