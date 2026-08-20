# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Phosphor is an agent-native terminal editor. Rust core, Steel (embedded Scheme) editor layer,
ratatui 0.30 for drawing.

## Commands

Use the `just` recipes, not raw cargo — several of them differ from the obvious invocation:

- **`just gate`** — everything CI runs, in CI's order: `fmt`, `lint`, `clippy`, `test`, `deny`,
  `vendor-diff`. It runs all six even when one fails, so one invocation tells you everything that
  is wrong. **This is the command to run before saying something is green.**
- `just test` — `cargo nextest` (per-test process isolation; tests touch the XDG state dir and
  terminal state) **and then `cargo test --doc`, because nextest cannot run doctests.** That is an
  upstream limitation rather than a flag, so it skips them silently; this line read *"not `cargo
  test`"* while nothing in the repository ran the doc harness at all, and a genre-by-genre audit
  is what found it. One doctest exists and passes — the hole was in the harness, not the tests.
- `just lint` — the structural lints below; runs every `scripts/lint-*.sh`. Add a lint by dropping
  a script in, never by editing the justfile or CI.
- `just fmt` (check) · `just fmt-fix` (in place) · `just build` · `just clippy` (warnings denied) ·
  `just deny` · `just vendor-diff` (bare, the full divergence — `gate` takes `--stat`) ·
  `just review` (`cargo insta review` for the golden frames)
- **Measurements, deliberately not gates.** `just bench` (nine bench targets; asserts *shapes*,
  prints numbers, because a figure that moves with the machine has no business failing a build —
  this worktree saw absolute times swing 25× under concurrent load while every shape assertion
  held) · `just coverage` (per-file, worst first) · `just coverage-html` · `just unused-deps` ·
  `just mutants` (cargo-mutants — the planted violation asked of every line at once; scope it with
  `--file`, a whole run is hours. **Write down what a run finds**, triaged, in
  `docs/OPEN-QUESTIONS.md` — §46 is the precedent and the reason: a survivor count is not a
  finding, and the one that mattered there would have moved markers in files nobody touched.
  Equivalent mutants get named as such and left alive) · `just soak` (thousands of keystrokes
  through a real child editor; asserts growth is *bounded*, since the undo journal is append-only
  and is supposed to grow). None of these is in `gate` or CI, and a coverage floor should not be added: it reddens for
  reasons unrelated to correctness and gets raised until it means nothing.
  **This said "six benchmarks" while nine `[[bench]]` targets were declared** — `doc_claims.py`
  recomputes the task, capability, wave and lint counts and never this one, which is how it drifted
  unseen through the audit that found it.
- `just hack` — `cargo-hack --each-feature`. This one **does** gate, in its own CI job, because
  "feature set X does not compile" has exactly one right answer.
- **The vendor helpers**, which the fork section below assumes: `just vendor-check` (each
  `VENDOR.md`'s recorded SHA against git history) · `just vendor-pull <fork> <ref>` (merge new
  upstream) · `just vendor-build-headless` (proves `arboard` is out of the default graph **and**
  returns with `--features clipboard` — both directions, which is what makes it a proof).
- A bare `just` lists the recipes. It used to run the first one in the file, which was `build`.

**`just --list` is the authority, not this section.** It was five recipes behind at the pre-`S4`
audit — the three above plus `tapes-diff` and `tape-diff` — because nothing recomputes a list in
prose. If the two disagree, the justfile wins.

**Never run `cargo fmt --all`.** `--all` does not mean "workspace members" — it recurses through the
path dependencies into both vendored forks and fails on upstream code. The only way to green it
would be reformatting the forks, which permanently breaks `just vendor-diff`. Use `just fmt-fix`.
A PreToolUse hook blocks the `--all` form.

`just tapes` and `just tape <id>` regenerate the Tier-2 capture library — every screen, or one.
`just tapes-diff` and `just tape-diff <id>` capture fresh and diff against the **committed blob,
read straight out of git**, which is the pair CI runs and the one you want when the question is
*"did this change the screen?"* rather than *"bless this change."* **They overwrite the tracked
PNGs under `tapes/artifacts/` while doing it** — every tape screenshots to the same fixed path
(`tapes/diff-tapes.sh`'s own header: *"capturing fresh always overwrites the file a plain
`git diff` would show you were the reference"*), which is exactly why the reference comes from
`git show` — so a run leaves the working tree dirty and `git checkout -- tapes` puts it back.
This paragraph read *"without overwriting it"* until `CP-4`'s review ran the command and the tree
disagreed with it. All four need the `phosphor` binary on `$PATH` — `just install` puts it there.

## Version control

**Git only. Do not use or re-initialise jj** — the colocated jj repo was deleted deliberately, and
this overrides the global jj-for-local-work rule.

`origin` is `git@github.com:TrevorS/phosphor.git` and `master` tracks `origin/main`. Push only
when Teej asks, and never force-push or rewrite pushed history without asking first.

Commits: conventional subject (`build:`, `fix:`, `docs:`) plus a body explaining what changed and
why, including findings, deviations, and anything left open.

## Lints — CI fails on these

Everything below is enforced mechanically rather than by convention, lives in `scripts/lint-*.sh`,
and is proven to bite on a planted violation. A failure is real. **Add a lint by dropping a script
into that glob** — never by editing the justfile or the CI workflow.

Architecture:

- **No literal colours in `phosphor-ui` outside `theme.rs`.** Every widget takes `&Theme`. Colour
  values come from Design Language §1 — one you invented is a bug even if it compiles.
- **`phosphor-ui` never imports `phosphor_core::store`.** Widgets read `::vm` (ViewModels) and
  `::view` (the view tree). Mutation is the binary's job.
- **No `crossterm::`, `ratatui::` or `editor_crossterm` in `phosphor-ui`.** It takes `ratatui-core`
  only. A source lint because Cargo unifies features per crate across the graph, so the manifest
  cannot express it.
- **No `Action` construction in `phosphor-ui`.** The store lint closes the *applying* half; this
  closes the *building* half, because a widget that can construct a mutation is one refactor from
  applying one.
- **One registry, three derived doors.** The Steel, MCP and CLI modules are total functions over
  the capability table; none may name a capability. That is what makes a one-door Action
  unconstructible rather than merely tested for.
- **One escape hatch.** `Node::Spans` is the only custom-draw path, drawn in exactly one place.
- **The Steel barrier.** `phosphor-steel` reaches `phosphor-core` and the VM, and nothing else.
- **One VM door.** `Layer` owns the `Runtime`, exposes no `&mut Runtime`, and the loop reads
  `Layer::stale()` in one place — so "arbitrary scheme ran, invalidate the frame" is structural
  rather than remembered. `CP-2` found the keybinding half of that rule missing by running it.

Reachability — a ticked task may not ship something no keystroke can reach:

- **Action arms.** Every mutation a *ticked* task declares must be named by the binary. `T016` was
  ticked for three windows with `za` doing nothing, on a snapshot that hand-built the view tree.
  Known gaps live in a RECORDED table that fails four ways, so it can only shrink.
- **Node kinds.** The same check one layer up, over the thirty `Node` variants and the four places
  composition happens — two of them Steel, addressed by tag rather than by Rust path.

Hygiene and truthfulness — each of these exists because the thing it catches already happened:

- **Repo hygiene** — no tracked file over 1 MB, no undocumented byte-identical reference capture,
  no refs outside the normal namespaces.
- **Doc claims** — the task counts, wave widths and gate counts in `TEAM.md` are recomputed from
  the dependency graph in `TASKS.md`, the toolchain version quoted in prose is checked against
  `rust-toolchain.toml`, and every `T0xx` cited in a Rust comment must be a task that exists.
- **Doc links** — `cargo doc` with warnings denied. This codebase cross-references itself through
  intra-doc links and nothing ran `cargo doc` until this lint; the first run found eight broken.
- **Doc line citations** — prose is cited by heading or quoted phrase, never by a markdown path with a line number stapled to it.
  `file:line` is right for *code*, where a moved line usually means a moved fact and two other
  lints already check the references; nothing holds a line number pointed at a paragraph. Two
  citations in `WINDOW-F-PLAN.md` went stale inside a day, one of them because executing that
  plan's own step inserted 82 lines above its own pointer — leaving it aimed at prose that step
  had just written. The 28 that already existed are **per-file budgets that can only shrink**,
  the shape the two reachability lints use, because converting them in bulk is how a confidently
  wrong phrase gets written: the first conversion attempted under this lint replaced an
  already-stale number with a *freshly* wrong phrase, and the real target was three hundred lines
  away.
- **MSRV** — `workspace.package.rust-version` is recomputed from the dependency graph. It read
  `1.85` for two windows while `ratatui` required `1.88`.
- **Vendor provenance** — each `VENDOR.md`'s recorded SHA and claimed licence are checked against
  git history and the fork's own `Cargo.toml`.
- **Vendor hunks** — every file diverging from the upstream tree must be *mentioned* in that
  fork's `VENDOR.md`. This paragraph used to say `just vendor-diff` audited that contract; it
  never did — it prints and exits 0, and a human reading 3,336 lines was the entire check. The
  first version of the lint was vacuous for every source file (a one-component directory match, so
  `src/` "documented" `src/types.rs`) and passed a planted violation silently.
- **Fuzz targets** — the `fuzz/` crate's targets are checked against the parsers they claim.
- **Counts nothing else recomputes** — the capability and parity counts (`209`/`627` went stale in
  six places at once), and the lint count in CI's own prose, which said "six" while sixteen
  existed. **Twenty-two lints now** — and this paragraph itself said seventeen for a window after the
  eighteenth landed, because `doc_claims.py` section 6 globbed `.github/workflows/*.yml` and could
  not see the file every agent reads on entry. It reads this one too now, so the sentence you are
  reading is recomputed rather than remembered.
- **Door callers** — a shell script that runs `phosphor` must survive it refusing, and must not
  match an answer shape the door stopped printing. `scripts/seed-fixtures.sh` did both: under
  `set -e` an `out="$(…)"; code=$?` aborts on the first refusal, so it died on line one of its own
  plan and printed nothing, and its classifier still matched `T100`'s predecessor. It sits outside
  the `scripts/lint-*.sh` glob on purpose, so **nothing ran it for a whole phase** — the one thing
  a script outside the gate can still be held to is whether it is capable of reporting.

## Do not assert what you have not read

The most expensive defect in this build so far was not a bug. A `VENDOR.md` described a licence
crisis that did not exist — a "Synthetic Source License" granting no rights, a `SySL-1.0`
declaration, a `cargo deny` failure — three claims, all false against the tree in the same
directory, and every gate passed them because nothing verified prose against reality.

So: **state a fact about a file only if you read that file in this session**, and give
`file:line` when the claim is load-bearing. If you are recording a finding in a `VENDOR.md`, a
task entry, or a checkpoint report, the standard is what a lint would accept, not what sounds
right. When you find that a document and the tree disagree, the tree wins and the document is the
bug.

## The vendored forks

`vendor/ratatui-code-editor` and `vendor/ratatui-markdown` are `git subtree` forks, deliberately
excluded from `[workspace] members` so our lints stop at the seam.

**Every hunk under `vendor/` needs a matching entry in that fork's `VENDOR.md`** — that is the
acceptance contract. An undocumented hunk is how a fork silently becomes a rewrite. Keep patches
minimal and put phosphor additions in the fork's own `phosphor/` module. Pin by SHA, not tag:
upstream published `ratatui-code-editor` 0.0.6 without ever tagging it.

Two commands, and they do different jobs. `scripts/lint-vendor-hunks.sh` (inside `just lint`) is
the **audit**: it fails if a file diverging from upstream is never mentioned in that fork's
`VENDOR.md`. `just vendor-diff` is the **review**: it prints the divergence so you can read
whether the entry actually explains it, which no lint can judge. This paragraph said the second
one audited the contract; it never did — it prints and exits 0 — and a human reading 3,336 lines
was the entire check.

## docs/ is the specification

Read `docs/README.md` first for the reading order. The plan, the task breakdown, the spike findings
and the team model live there, and tasks carry acceptance criteria worth reading before building.

**Never edit `docs/design/*.dc.html`** — they were imported verbatim from a claude.ai Design project
and the filenames match the remote paths so it round-trips; edit them there, not here. They are the
source of truth for the palette, the mockup screens and the voice. `doc-page.js` and `support.js`
are their runtime and must stay alongside them. If the design and the build disagree, flag it; do
not fold the change in.

## Checkpoints

`CP-0`–`CP-9` in `docs/TASKS.md` are stop-the-line gates for multi-agent window runs: each has a
manual half only Teej can perform, and a window does not start until the previous one passes. They
do not gate ordinary solo work — but do not mark one passed on Teej's behalf.
