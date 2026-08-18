# Contributing

Phosphor is a design-driven build with a specification that predates the code, so the useful
thing to know before opening a pull request is which parts are settled and which are open.

## Before anything else

Run this. It is what CI runs, in CI's order, and it keeps going after a failure so one invocation
tells you everything that is wrong:

```console
$ just gate
```

`docs/README.md` gives the reading order for the design documents. `CLAUDE.md` is the working
agreement — commands, the structural lints and what each one caught, and the rules below in
fuller form.

## Four rules that are not negotiable

**Do not assert what you have not read.** State a fact about a file only if you read that file,
and give `file:line` when the claim is load-bearing. The most expensive defect in this build was
not a bug: a `VENDOR.md` described a licence crisis that did not exist — three claims, all false
against the tree in the same directory, and every gate passed them because nothing verified prose
against reality. Where a document and the tree disagree, the tree wins and the document is the
bug.

**Add a lint by dropping a script into `scripts/lint-*.sh`.** Never by editing the justfile or the
CI workflow. `just lint` runs the glob, so a new lint changes no wiring.

**A new lint or test must be pressed against a planted violation.** Break the thing on purpose,
watch the check go red, put it back. This is not a formality — during this repository's own audit
a freshly written test *passed* a planted violation, because the fixture happened to make a wrong
answer look right. Planting is the only thing that catches an assertion that is true by accident.

**Do not edit `docs/design/*.dc.html`.** They are imported verbatim from the design project that
owns them, and the filenames match the remote paths so they round-trip. If the design and the
build disagree, say so in the pull request; do not fold the change in.

## Commits

A conventional subject (`fix:`, `feat:`, `docs:`, `perf:`, `build:`, `test:`) and a body that
explains what changed and **why** — including findings, deviations, and anything left open. The
git log here is used as a record of what was learned, not just of what moved, so a body that says
what you measured is worth more than one that says what you did.

## Version control

Git only. There is no jj repository here; the colocated one was deleted deliberately.

Do not force-push or rewrite pushed history. If a branch has an open pull request with review
comments on it, adding commits on top keeps those comments anchored where rewriting would detach
them.

## Where the work is

`docs/TASKS.md` holds the task breakdown with acceptance criteria, and the `CP-` checkpoints that
gate each phase. A capability that refuses by naming a task id is pointing straight at its entry
there.
