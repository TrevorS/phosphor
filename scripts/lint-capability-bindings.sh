#!/usr/bin/env bash
# A built mutation a person can reach, or a recorded reason it is not one.
#
# WHY THIS EXISTS. `T088` shipped `split-pane`, `focus-pane`, `close-pane` and
# `resize-pane` with arms, a query, and a passing gate — and **nothing bound to
# any of them**. The task's acceptance asked for arms and a query and never for
# keys, so the only way a person could make a split was a side effect of the
# files picker. Four capabilities, reachable by an agent over MCP and by nobody
# at a keyboard, and every existing lint said clean:
#
#   * `lint-action-arms.sh` proves a ticked task's mutations are NAMED by the
#     binary. All four were.
#   * `lint-key-coverage.sh` proves every BOUND key is pressed by a test. None
#     of them was bound, so it had nothing to say.
#
# Between those two there was no lint at all, and the gap is exactly the shape
# of `T016` — reachable, working, and unreachable through the door a person
# uses. `set-virtual-text-visible` had been sitting in it since `T032`.
#
# WHAT IT CHECKS. Every capability that (a) belongs to a ticked task, (b) the
# binary names — so an arm exists — and (c) is a *mutation a person drives*
# must be written down somewhere in `runtime/`. The three spellings the layer
# uses are all accepted, and getting that wrong is how this was miscounted
# twice while it was being written:
#
#   `(key/cmd "split-pane" …)`   a binding row or an ex body
#   `(open-repl!)`               a Steel procedure, which `:repl` uses
#   `(list 'paste before)`       a quoted symbol inside a `key/*` helper
#
# WHAT IT DOES NOT CHECK, and each is a real category rather than an excuse:
#
#   * the input machine emits it (`move-cursor`, `set-mode`, `set-count` …) —
#     a keymap never names one, the machine turns keys into them;
#   * a producer posts it (`ingest-*`) — nobody types an LSP answer;
#   * a surface handles its own keys in Rust (`picker-accept`, `repl-history`,
#     `float-select`, `submit-prompt`) — `picker_key`, `repl_key` and `ex_key`
#     are those keymaps;
#   * it is the agent's door (`declare-regions`, `set-keybinding`) — a verb
#     whose whole point is that something other than a person calls it.
#
# Those four sets are listed below with the reason each is in one. A capability
# in none of them, with an arm and a ticked task and no binding, fails.
#
# RECORDED can only shrink, which is `lint-action-arms.sh`'s shape and for the
# same reason: a row here is a promise, not a place to put things. It fails
# four ways — a new unbound capability, a RECORDED row that is now bound, a
# RECORDED row naming a capability that no longer exists, and an EMITTED row
# for one that does not either.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

# **`tests.rs` is skipped by name.** The binary's unit tests moved out of
# `main.rs` on 2026-08-25 to get back under the 1 MB hygiene ceiling. This
# scan strips the column-0 `#[cfg(test)]` to find the production half — and
# in a file that *is* the test module there is no attribute to strip, so
# without this line 5,000 lines of fixtures would read as production and a
# test that constructs an Action would count as an arm.
python3 - <<'PY'
import pathlib
import re
import sys

ACTIONS = pathlib.Path("crates/phosphor-core/src/action.rs")
TASKS = pathlib.Path("docs/TASKS.md")
BIN = pathlib.Path("crates/phosphor/src")
RUNTIME = pathlib.Path("runtime")

# Capabilities something other than a person is meant to emit. The value is the
# reason, and it is printed when a row goes stale so the next reader does not
# have to reconstruct it.
EMITTED = {
    # The input machine turns keys into these. A keymap names an *operator* or
    # a motion; the machine emits the mutation.
    "move-cursor": "the input machine emits it from a motion key",
    "set-cursor": "the input machine emits it; `gg`/`G`/a find resolve here",
    "extend-selection": "the input machine emits it while an anchor is set",
    "clear-selection": "the input machine emits it on leaving visual mode",
    "select-object": "the input machine emits it from a text object",
    "set-mode": "the machine's own report of a transition it already made",
    "set-count": "the machine's pending count",
    "select-register": "the machine's pending register",
    "cancel-pending": "the machine clearing a half-typed sequence",
    "feed-keys": "the machine replaying, for `.` and macros",
    "repeat-last": "the machine replaying the last change",
    "commit-undo-group": "the machine closing a batch after an edit",
    # `T095`. **A key cannot carry the argument.** The row takes a
    # `CheckpointId`, which is a number minted by the undo tree — a person has
    # nowhere to read one from and nothing to type it into, and inventing an
    # ex command that took a raw node id would be a door onto the tree's
    # internals rather than a feature. Its caller is the agent: a turn records
    # the checkpoint it began at and spends it to come back, which is what
    # makes an agent turn a unit of undo (`T073`'s timeline reads that shape).
    "undo-to-checkpoint": "the agent's door — a turn spends the checkpoint it recorded",
    "set-case": "the machine emits it from `key/operator \"toggle-case\"`",
    # Producers post these; nobody types an answer.
    "ingest-completions": "the LSP client posts it",
    "ingest-diagnostics": "the LSP client posts it",
    # `T069`. Both are the disk watcher's, and neither is a sentence a person
    # has any way to mean: `note-disk-change` is a *report* that bytes moved —
    # typing it would be claiming a change the editor could check and disprove
    # — and `set-file-watch` follows the focused buffer once per frame, so a
    # key for it would be a key that fights the loop. What a person types is
    # the way *out*: `:reload` and `SPC r r`, which are bound.
    # `T073`. `3b`'s footer — `↵ edit here · d diff · o full op log · esc` — is
    # routed in the loop rather than in `runtime/`, the same way `5c`'s inbox
    # is, and for the reason CLAUDE.md already allows: *"a surface whose keymap
    # is Rust"*. The keys are pressed by
    # `the_timeline_declines_by_naming_the_state` and its siblings.
    "edit-at-change": "`3b`'s `↵`, routed in the loop — the timeline's keymap is Rust",
    "show-change-diff": "`3b`'s `d`, routed in the loop — the timeline's keymap is Rust",
    "open-operation-log": "`3b`'s `o`, routed in the loop — the timeline's keymap is Rust",
    "note-disk-change": "crate::watch's debouncer thread posts it",
    "set-file-watch": "the loop emits it per frame, following the focused buffer",
    # `T050`. The same shape one door over: a turn boundary is something the
    # agent did, so the ACP client posts it and nobody types it. What a person
    # types is `:claude`, which is `send-message`, and that is bound.
    "turn-began": "the ACP session client posts it when a prompt goes out",
    "turn-ended": "the ACP session client posts it when the agent stops",
    "ingest-hover": "the LSP client posts it",
    "ingest-signature-help": "the LSP client posts it",
    # A surface with its own key handler in Rust. `picker_key` and `repl_key`
    # are keymaps too; they are just not written in Scheme.
    "picker-accept": "`picker_key` handles it — the picker's own keymap",
    "set-picker-query": "`picker_key` handles it, on every printable key",
    "cycle-picker-source": "`picker_key` handles it — `<tab>`",
    "toggle-picker-preview": "`picker_key` handles it",
    # `T058`. The prompt line is the third surface with its own key handler in
    # Rust: `ex_key` is its keymap, and the four verbs below are what those keys
    # mean. `open-prompt` is the one a *keymap* names — `:` and `SPC c p` — and
    # it is bound.
    "set-prompt-text": "`ex_key` handles it, on every printable key",
    "submit-prompt": "`ex_key` handles it — `<cr>`",
    "cancel-prompt": "`ex_key` handles it — `esc`, and backspace off an empty line",
    "prompt-history": "`ex_key`'s keymap is where `<up>` lands; the verb is the door's spelling",
    "float-select": "the float's own key handling",
    "float-select-row": "the float's own key handling",
    "float-accept": "the float's own key handling",
    # `T059`. 4a's digits, and the keymap for them is Rust because a digit means
    # two different things depending on what holds the screen: over a buffer it
    # is vim's count prefix, and `keymaps.scm` has no way to ask what surface is
    # up. The loop gates on both conditions and emits this verb.
    "float-answer": "the float's own key handling — 4a's amber digits",
    # Emitted by `float-answer` above, which resolves the focused ask and
    # delegates. `answer-ask` names an ask by *id*, which is what makes it the
    # wrong thing for a keyboard: a digit that carried one could answer a
    # question you are not looking at. Its prose half is T060's and T061's wire.
    "answer-ask": "float-answer emits it, having resolved which ask is focused",
    # `T065`. 8b's `za`, and the keymap for it is Rust for `float-answer`'s
    # reason exactly: over a buffer `za` is `set-fold` and folds *code*, and
    # `keymaps.scm` has no way to ask what surface holds the screen. `review_key`
    # gates on the surface and on the key, and answers `None` for everything it
    # does not name — which is what keeps `:annotate` and `:grouping`, the two
    # commands that only make sense while 8b is up, typeable from 8b.
    # `T066`. `4b`'s `s seen · S all`, Rust-side for `float-answer`'s reason —
    # over a buffer `s` is an ordinary character and `S` a shift of it, and
    # `keymaps.scm` has no way to ask what surface holds the screen. The peek's
    # own `s` is a *separate* Rust handler (`mark-seen` with an explicit
    # `Target::Hunk`, T041's verb) rather than this one, because a peek has one
    # row and no `all` to widen into.
    "float-mark": "the review float's own key handling — 4b's `s`/`S`",
    "float-toggle-fold": "the review float's own key handling — 8b's `za`",
    # `T067`. 5c's `↵`, Rust-side for the same reason the two above are: `view/
    # spans` is a snapshot and `keymaps.scm` cannot ask which row is
    # highlighted. `j`/`k`/`s` recompose the float through `open-inbox`'s own
    # capability and stay off this table; `↵` dispatches `open-inbox-item`
    # directly, because that is the verb that decides what a row *means* and
    # `T067`'s own arm already had to answer that question for the door.
    "open-inbox-item": "the inbox float's own key handling — 5c's `↵`",
    "close-float": "`closes_surface` handles `esc` for every float",
    "close-all-floats": "`closes_surface` handles `esc` for every float",
    "close-repl": "`repl_key` handles it — `esc`",
    "repl-history": "`repl_key` handles it — up and down",
    "repl-to-buffer": "`repl_key` handles it — `<C-c>`",
    "eval": "the REPL's own enter key, and `--eval` at the CLI door",
    "persist-form": "the REPL routes a config verb to it",
    "show-unknown-key-hint": "the loop raises it when a key is bound to nothing",
    # The agent's door. A verb whose point is that something else calls it.
    "declare-regions": "the agent's door — `T041`'s whole subject",
    # `T052`. The batch verb, and its row says so: *"the shape an agent writes
    # through"*. Nobody types a list of spans and replacement texts; an agent
    # emits one over MCP, and `Intent::Act` is what carries it to the rope.
    "apply-edits": "the agent's door — the batch an agent writes through",
    # `T054`. `session-prose` is bound through `send-message` and `open-prompt`
    # only in the sense that a person triggers the *turn* — the verb itself is
    # never typed, because the ACP client's own `transcribe` is what turns an
    # `AgentMessageChunk` into one. The three tool-call verbs are the same
    # shape one level down: `ToolCall`/`ToolCallUpdate` are the agent narrating
    # its own tool use, and there is no key for narration.
    "session-prose": "the ACP session client posts it as claude's prose streams in",
    "tool-call-started": "the ACP session client posts it when the agent starts a tool call",
    "tool-call-progress": "the ACP session client posts it as a tool call reports progress",
    "tool-call-completed": "the ACP session client posts it when a tool call ends",
    "drop-regions": "the agent's door",
    # `T053`. Q6's review-block signal, and the row's own subject: a file and
    # span list with per-group annotations is something an agent emits after a
    # turn, not something a person types. `1b`'s seam is what a person sees.
    "declare-review-block": "the agent's door — Q6's review-block signal",
    "mark-unseen": "the agent's door",
    "reanchor": "the agent's door",
    "define-picker-source": "the Steel API — `runtime/pickers.scm` defines with it",
    "invalidate-picker-source": "the Steel API, for a source whose data moved",
    "define-language": "the Steel API — `runtime/languages/` declares with it",
    "define-float-surface": "the Steel API — `runtime/arch.scm` registers with it",
    "set-keybinding": "the agent's door; the shipped layer keeps its own table",
    "remove-keybinding": "the agent's door, the same way",
    "set-register": "the agent's door — a person yanks into one",
    # `T060`. A *server* emits it — a rename or a code action comes back as a
    # `WorkspaceEdit`, which `lsp::file_edits_from_lsp` reads. It is the one
    # `Lsp` capability rated `Ask`, so it does not apply on arrival: it becomes
    # a question in the queue and the answer is the binding a person has.
    "apply-workspace-edit": "a language server emits it; the queue asks you",
    # `T061`. 7a's `[1]`/`[2]`/`[3]`, and the keymap for a digit is Rust for
    # `float-answer`'s reason: a digit means different things depending on what
    # holds the screen. `float-answer` resolves the focused ask and emits these
    # two when it is a permission — which is the distinction that makes them
    # separate verbs, since `[2]` writes a rule and an `answer-ask` cannot.
    "grant-permission": "float-answer emits it for 7a's [1] and [2]",
    "deny-permission": "float-answer emits it for 7a's [3]",
    # `T056`. The capability's own sentence names its three callers — "a picker
    # accept, a transcript tool row, an OSC 8 link" — and none of them is a
    # person naming a file. A click lands in the *terminal*, which resolves the
    # `file://` URI itself; `open-file` is the one a person types (`:e`). A
    # keyboard jump from a focused transcript row would make this bindable and
    # needs a selection model that pane does not have — OPEN-QUESTIONS.md ss56.
    "goto-location": "the agent's door, a picker accept, and an OSC 8 click",
}

# Built, user-facing, and bound to nothing. Each row is a promise to bind it.
RECORDED = {
    "open-arch": (
        "a second capability for `:arch`, which reaches the same float through "
        "`open-float`. One of the two is redundant and the choice is not this "
        "lint's — recorded so it is a decision rather than a silence."
    ),
    "set-virtual-text-visible": (
        "`T032`'s rail collapse. `za` is bound, but to `set-fold`, which is a "
        "different capability — so a claude rail cannot be collapsed from a "
        "key. Wants a binding; `T041`'s owners make it addressable."
    ),
}

# -- the vocabulary ---------------------------------------------------------
text = ACTIONS.read_text(encoding="utf-8")
declared = re.findall(
    r'^\s+([A-Z][A-Za-z0-9]*) = "([a-z0-9-]+)" \[\s*(\w+)\s*/\s*"([^"]+)"',
    text,
    re.M,
)
if len(declared) < 100:
    print(
        f"lint-capability-bindings: read only {len(declared)} variants from {ACTIONS} — "
        "the macro's shape moved and this lint is now checking nothing.",
        file=sys.stderr,
    )
    sys.exit(1)

ticked = set(re.findall(r"^- \[x\] \*\*(T\d+|V\d+)", TASKS.read_text(encoding="utf-8"), re.M))

# -- which the binary names, so an arm exists -------------------------------
body = []
for path in sorted(BIN.glob("*.rs")):
    if path.name == "tests.rs":
        continue
    source = path.read_text(encoding="utf-8")
    module = re.search(r"^#\[cfg\(test\)\]", source, re.M)
    body.append(source[: module.start()] if module else source)
body = "\n".join(body)

# -- which the layer writes down, in any of its three spellings --------------
runtime = "\n".join(path.read_text(encoding="utf-8") for path in sorted(RUNTIME.rglob("*.scm")))
bound = set(re.findall(r'"([a-z0-9-]+)"', runtime))
bound |= set(re.findall(r"\(([a-z0-9-]+)!", runtime))
bound |= set(re.findall(r"'([a-z0-9-]+)", runtime))

names = {name for _, name, _, _ in declared}
armed = {
    name
    for variant, name, _phase, task in declared
    if task in ticked and re.search(rf"\b{variant}\b", body)
}

problems = []

for name in sorted(armed - bound - set(EMITTED) - set(RECORDED)):
    problems.append(
        f"`{name}` has an arm, belongs to a ticked task, and nothing in {RUNTIME}/ "
        f"names it — no key and no ex command can reach it.\n"
        f"    Bind it, or add it to EMITTED with what emits it instead, or to "
        f"RECORDED with the reason it is not bound yet."
    )

for name in sorted(set(RECORDED) & bound):
    problems.append(
        f"RECORDED lists `{name}` and the layer now names it — delete the row."
    )

for name in sorted(set(RECORDED) - names):
    problems.append(f"RECORDED lists `{name}`, which the vocabulary no longer declares.")

for name in sorted(set(EMITTED) - names):
    problems.append(f"EMITTED lists `{name}`, which the vocabulary no longer declares.")

if problems:
    print("lint-capability-bindings: FAILED", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    sys.exit(1)

reachable = len(armed & bound)
print(
    f"lint-capability-bindings: clean — {len(armed)} armed on ticked tasks, "
    f"{reachable} bound in {RUNTIME}/, {len(set(EMITTED) & armed)} emitted elsewhere, "
    f"{len(RECORDED)} recorded"
)
PY
