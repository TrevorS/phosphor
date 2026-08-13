# Phosphor — task breakdown

Decomposed from [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md), which is itself derived from
the four design docs in [design/](design/). The plan says *what each phase is for*; this file
says *what to build, in what order, and where we stop and look at it*.

**100 tasks + 9 harness tasks · 12 checkpoints · 9 phases**, covering all 34 screens v1 builds.
Phase ids (`M-0`, `S1`…`S8`) match the plan and the Component Breakdown's build order. Task ids
are stable and assigned in order of creation — reference them in commits. New tasks append
rather than renumber, so `T078`+ sit inside earlier phases.

**`CP-0` has passed, both halves.** The go/no-go verdict was settled by the two spikes
([SPIKES.md](SPIKES.md)), whose consequences are folded in below — `T081`–`T083` exist because of
what they found. The build half followed: the workspace, both vendored subtrees, the structural
lints and the grammar ABI check all landed and the gate ran green. One criterion in `T003` was
never executed and says so at the task.

**`CP-1` has passed too, both halves** — the mechanical gate and Teej's four-terminal pass. Four
rulings came out of the manual half; three amend design docs and are tabled in
[§5](IMPLEMENTATION-PLAN.md#5-decisions).

**Window C is built and its mechanical half is green.** The `Action` vocabulary is 212
capabilities generated from one table, the three doors are total functions over it, and the
parity test walks all 636 door checks end to end. (`208`/`624` until `S3` added
`Buffer::SetCase`, and `209`/`627` until the repair window added `set-macro-recording`, `register`
and `place-anchor`; the count is `scripts/lint-one-registry.sh`'s, which reads the tables in
`crates/phosphor-core/src/{action,query}.rs` — do not compute it by hand. All six prose citations
of `208` are fixed, and `scripts/doc_claims.py` section 5 now recomputes both numbers and fails on
a stale one, so this paragraph cannot go quietly wrong again — as it just did not, when the three
new verbs reddened every stale copy of `209` in one run.)
`CP-2`'s **manual half passed** on 2026-08-12 — Teej ran the editor, exercised the REPL and the
live rebind, and gave the verdict, which is what unblocked S3. It is the checkpoint that asks
whether the Steel layer is the editor or a config file with a Rust editor hiding behind it.
Recorded here late: the verdict was given in conversation and never written down, and Window D ran
past a checkpoint this file still called outstanding. **A checkpoint verdict is written where the
checkpoint is, or it did not happen** — that rule is the finding, not the verdict.

The gate failed on its first run and the three findings are worth keeping, because none was on
its own criterion list: no Tier-1 snapshot for `6b`; `T079`'s frame cache exercised by a
benchmark and by nothing that ships; and the statusline composed in Steel on the REPL surface
only — found by **deleting `runtime/statusline.scm` and watching a statusline still draw**. All
three are fixed. The second and third had one cause: the window's phases froze `main.rs` so that
parallel agents could not collide in it, and the two tasks that needed to wire themselves into
the host had nowhere to do it.

**`CP-3` has passed too, both halves** — the mechanical half green at 639 tests and 14 lints, and
Teej's manual half on **2026-08-13**, with **no findings**. The verdict is written at the
checkpoint, which is where it belongs and what the `CP-2` entry above exists to insist on. It
unblocks `S4` and it settles nothing else: the arms in *A · Arms owed* are still owed and the
repair items still open in [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) are still open.

Checkboxes below track *tasks*, not checkpoints: a task is ticked when its own *done when* is
demonstrably met. The two are deliberately separate, because a checkpoint is a human judgement
about whether the result is any good, and every task in a window can be green while the answer
to that is still no.

**`T084`–`T089` were added by the docs review.** Six widget/primitive tasks the design docs
require and the first breakdown had no home for: the `Float` chrome primitive, undercurl, the
`HelpGrid`, region tints, the pane manager, and `TabBar`. They are marked 📌 where they appear.

**`T092`–`T098` were added by the `CP-3` audit**, and they are a different kind of addition from
either of those: not a surface nobody had tasked, but **work nothing in the graph owned**. Ten
declared mutations had no arm in the binary and no task that would ever give them one, and a `q`
key that a vim user's hands reach for was unbound rather than deferred. They live in their own
section, *A · Arms owed*, at the end of this file — because their point is to rot **visibly**.

**`T099` and `T100` were added by the repair window between `CP-3` and `S4`**, and they sit in a
second such section, *B · The repair window*. Same reason, one turn later: that window added three
capabilities — `set-macro-recording`, `register` and `place-anchor` — because `T098` had stopped
at a wall the vocabulary put there, and a verb added with no task to build it is precisely the
debt *Arms owed* exists to make visible. `T100` is different in kind: it is the door's *voice*
rather than an arm, ruled from two open questions that turned out to be one defect.

**`T090` was added by the first `CP-1` attempt**, which failed on it and nothing else: the widget
layer was complete and green, and no task built an application around it, so `cargo run` drew
nothing and the checkpoint's manual half was impossible. It is marked 📌 too. The lesson is the
same one `CP-0` taught — *a decision table enumerates the outcomes you predicted, and the useful
findings are the ones outside it* — except here the gap was a task nobody wrote rather than an
answer nobody expected.

---

## How checkpoints work

A checkpoint is a full stop. Not a status update: nothing downstream of it starts until it
passes.

They exist because **most of Phosphor's acceptance criteria are perceptual and cannot be
asserted in CI.** A test can prove the statusline truncated at 80 columns; it cannot tell you
whether the shed order left something confusing on screen. It can prove synchronized output
wraps every frame; only a human watching a stream can say whether a frame tore. This is a TUI
whose design language is almost entirely about what things look like, so the verification has to
be split.

### Three verification tiers

Verification is split across three tiers. Each catches a class of failure the others structurally
cannot.

| Tier | What it is | What it proves | Gates CI? |
|---|---|---|---|
| **1 · In-process snapshot** (`T018`) | ratatui `TestBackend` → cell grid → committed text snapshot | *What we told the terminal to draw.* Exact, diffable, fast. | **Yes** |
| **2 · VHS capture** (`V001`–`V009`) | real PTY → real terminal emulator → PNG frames + GIF | *What actually appeared.* Escape sequences, terminal setup, truecolor, the synchronized-output wrapper, the whole pipeline. | No — artifacts + change detection |
| **3 · Teej, real hardware** | you, at your own terminals | Whether it tears, whether chords reach the app, whether it's any *good*. | n/a |

Tier 1 can pass while the real output is broken — wrong escape codes, a mis-wired sync wrapper,
truecolor silently downsampled. Tier 2 catches exactly that gap. Neither can tell you whether
the thing is worth using; that stays Tier 3.

### The wording standard for a *done when*

**A criterion that says "screen `3c` reproduces" is satisfied by a test that hand-builds the view
tree and renders it.** That is not a bad test — it is exactly what Tier 1 is for, and every golden
frame in this repo works that way on purpose. It is the *wording* that is the defect, and it has
now cost this build four surfaces.

`T016` is the worked example. Its criterion read *"screen `8e`'s fold and whitespace details
reproduce"*, and `8e` reproduced: `crates/phosphor/tests/screen_8e.rs` builds a `Tree` by hand.
The whitespace half was genuinely wired into the loop; the fold half never was — no `z` binding,
no arm for `Action::View(SetFold)` — so `za` fell to `NotYetImplemented` and ran vim's plain `a`.
It was ticked, and folds stayed unreachable for three windows, because every gate asked *does the
snapshot match* and none asked *can you press the key*. The same shape then repeated at scale
across Window D: the leader popup, the unknown-key hint and undo all shipped built, tested,
ticked and uncomposed.

So, the standard, in two lines:

> **If a user reaches the surface by pressing a key, the criterion is that it reproduces
> *from a keystroke*, and the proof is a loop-driven or pty test.** If a user cannot — a widget
> whose data is a store query that does not exist yet, a degradation path with no live consumer —
> the criterion says so, names what is missing, and a hand-built tree is the right and honest bar.

`crates/phosphor/tests/loop_pty.rs` is the pattern for the first kind: it drives the shipping
binary on a real pty and reads cells off the frame, so keystroke → keymap → machine → `Action` →
arm → draw is all in the assertion. `crates/phosphor-ui/tests/virtual_text_node.rs` is the pattern
for the second, and `T032`'s entry below states out loud why a hand-built tree is correct there.

Two mechanical halves back this up, and neither replaces the wording:
`scripts/lint-action-arms.sh` fails when a ticked task declares a mutation the binary never names,
and the wiring-agent rule in [TEAM.md](TEAM.md#concurrency--several-agents-one-worktree) makes
composition somebody's explicit job in the last phase of every window.

**Tier 2 converts a lot of Tier 3 into asynchronous review.** Instead of driving the editor by
hand to see the ask queue behave, you watch a ten-second clip. Instead of resizing a window to
watch statusline shedding, you flip through a width sweep. That's the main win — not automation,
but making the perceptual review reproducible, diffable, and reviewable whenever you like.

### What VHS can and cannot reach

Checked against VHS **0.11.0**. Two properties of the tool shape the whole design:

- **There is no text output format.** VHS emits `gif`, `mp4`, `webm`, or a `frames/` PNG
  sequence — nothing assertable as text. So Tier 2 assertions are *pixel* comparisons against
  committed reference PNGs, which are fragile across font rendering, OS, and VHS version.
  **Pin the VHS version, the font, and the machine that regenerates references** (`V001`,
  `V007`), and treat a pixel diff as a change detector that asks for a human look — not a
  build-breaking gate. The exact, stable, diffable assertions stay in Tier 1.
- **`Set Width` / `Set Height` are pixels, not columns.** There is no direct column control, so
  hitting exactly 80 columns takes a calibration pass (`V002`) mapping `(FontSize, Width)` to a
  known column count. This is unavoidable and worth doing once, properly — the 80-column shed
  order is a real acceptance criterion at nearly every checkpoint.

What stays irreducibly Tier 3, and why:

| Cannot be reached by VHS | Why |
|---|---|
| **Torn frames** | Capture is post-composite from the emulator's own renderer, which does not implement synchronized output (DECSET 2026). A tear would be either invisible or always-present — the observation is meaningless either way. **This is the most important limitation**, because a torn frame is P0. |
| **Kitty keyboard protocol** | The browser-based terminal VHS drives does not implement it, so modifier chords can't be distinguished. `T027` is verified on your hardware only. |
| **OSC 8 activation** | Links may *render*, but nothing can click one. |
| **Latency and feel** | "Fast enough to be useful or fast enough to be annoying" is not a measurable a recording exposes. |
| **Real multiplexer passthrough** | tmux can run *inside* a tape, but its passthrough behaviour against a real terminal is what actually breaks. |

Verify empirically during `V002` rather than assuming either way: **undercurl** (the emulator has
curly-underline support, but whether it survives capture is untested) and **truecolor fidelity**
(that captured pixels match the theme's hex values exactly).

What VHS *does* recover from Tier 3: **degradation paths**, via `Env TERM xterm-256color` and
`Env NO_COLOR 1` (`V009`). Those are otherwise tedious to check by hand every time.

> **A payoff from the S2 architecture.** Tapes seed exact editor state through
> `phosphor --eval` — the CLI door from `T023`. Because Steel, MCP, and the CLI share one
> vocabulary (invariant 2), the test harness can drive the editor into any state the agent could,
> with no test-only backdoor. The one-API rule pays for itself here first.

A checkpoint also names **what a failure reopens** — which task, decision, or design assumption
comes back into play. That matters most at `CP-0`, where a failure changes the shape of the
whole UI layer.

### Your terminal matrix

Several checkpoints need more than one terminal, because the capabilities the design assumes
are unevenly supported. Fill in what you actually use:

| Role | Needs | Yours |
|---|---|---|
| **Primary** | kitty keyboard protocol, synchronized output, OSC 8, undercurl, truecolor | *(Ghostty / kitty / WezTerm)* |
| **Secondary** | truecolor, partial kitty support — catches assumptions baked into the primary | *(iTerm2)* |
| **Degradation target** | no kitty protocol, no undercurl, possibly 256-colour | *(Terminal.app)* |
| **Multiplexed** | primary inside tmux — passthrough is where sync output and OSC 8 break | tmux |
| **VHS** | none of the above — a captured environment, not a terminal you trust | pinned in `V001` |

`CP-1` establishes the baseline on all four real terminals and calibrates VHS. After that, VHS
carries most of the repeat coverage, the primary terminal handles the Tier-3 residue, and the
full four-terminal sweep repeats only at `CP-5`, `CP-7`, and `CP-9`.

### The recurring sweep

Re-checked at **every** checkpoint from `CP-1` onward, because each new async event source is a
fresh chance to break them. Listed once here rather than repeated twelve times. The tier that
can actually see each one is noted, since it isn't obvious:

- **No torn frames** — **Tier 3 only.** Synchronized output wraps every frame; a tear is P0.
  No recording can show you this.
- **80 columns** — Tier 2 (calibrated width sweep) + Tier 1 (the truncation property test).
  Shed order counters → jj → cursor → session prose → mode word, `✻`/`●n`/`!` last. Nothing
  wraps. A second statusline row is a bug.
- **Degradation** — Tier 2 via `Env` (`V009`), confirmed on real hardware at the full sweeps.
  Markers become `▎`, undercurl becomes underline, the spinner becomes a static `✻`.
- **tmux** — Tier 3. Passthrough is the thing being tested, so a captured tmux proves little.
- **Nothing moves unless you asked** — Tier 2 is *ideal* here: a GIF showing the cursor and
  viewport dead still while the file changes underneath is the clearest possible evidence.

---

## Task index

| Phase | Tasks | Checkpoint |
|---|---|---|
| M-0 · Scaffolding + spikes | T001–T009, T083 | **CP-0** — ✅ passed, both halves |
| S1 · Theme + BufferView + StatusLine | T010–T018, T081, T084, T085, T090 | **CP-1** — ✅ passed, both halves |
| S2 · Steel + Action + REPL + view tree | T019–T025, T078–T080 | **CP-2** — ✅ passed, both halves |
| S3 · Input + undo + gutter | T026–T035, T086 | **CP-3** — ✅ passed, both halves |
| S4 · LSP | T036–T040, T082 | **CP-4** — boring on purpose |
| S5 · Store + seen + Picker | T041–T049, T087 | **CP-5** — the awareness loop |
| S6 · ACP + MCP + Transcript + Prompt | T050–T062, T088, T089 | **CP-6** session · **CP-7** directing |
| S7 · Diffs + review + dirty + VCS | T063–T073 | **CP-8a/b/c** — one per `S7.1`/`S7.2`/`S7.3` |
| S8 · Watches | T074–T077 | **CP-9** — ship check |
| **V · Verification harness** | **V001–V009** | *cross-cutting — lands with S1, used from CP-1 on* |
| **A · Arms owed** | **T092–T098** | *cross-cutting — debt the `CP-3` audit found; see the section at the end* |
| **B · The repair window** | **T099, T100** | *between `CP-3` and `S4` — the verbs that window added, and their creditors* |

---

## V · Verification harness

The Tier-2 layer. Not product code — it is how every later checkpoint gets cheap. `V001`–`V005`
land alongside S1 so `CP-1` can use them; the rest follow as the surfaces they capture appear.

Separately numbered from the `T` tasks because it is a distinct workstream with a different
lifetime: the harness outlives any single phase and gets extended at every checkpoint.

- [x] **V001 · Pin VHS and its dependencies**
  VHS 0.11.0 + `ttyd` + `ffmpeg`, pinned by exact version. `Require` at the top of every tape so
  a missing dep fails loudly rather than silently producing a wrong recording. Record the
  reference-regeneration machine and font — pixel comparison is only meaningful against a fixed
  renderer.
  *Done when:* `just tapes` fails with a clear message on a machine with the wrong VHS version.
  *Needs:* —

- [x] **V002 · Column calibration**
  Map `(FontSize, Width)` → exact column count, since VHS sizes in pixels. Build a probe tape,
  binary-search the width for 80 / 100 / 120 / 200 columns, and commit the table as
  `tapes/_dimensions.tape`. **Also settle the two open empirical questions here:** does undercurl
  survive capture, and do captured pixels match theme hex values exactly?
  *Done when:* a tape asserting "exactly 80 columns" is reproducible, and both questions have
  written answers. *Needs:* V001

- [x] **V003 · Shared tape config**
  `tapes/_config.tape`, `Source`d by every tape: pinned font and size, `Set CursorBlink false`,
  fixed `TypingSpeed`, fixed `Framerate`, `Set Padding 0`, neutral background. **Every source of
  nondeterminism removed** — anything that varies between runs makes pixel comparison useless.
  *Done when:* the same tape run twice produces byte-identical PNGs. *Needs:* V002

- [x] **V004 · Deterministic waits — no `Sleep`**
  Use `Wait+Screen /regex/` against a known sentinel instead of sleeping. Phosphor needs a
  stable, greppable ready-state for this; the statusline is the natural sentinel.
  *Done when:* no tape in the library contains a bare `Sleep` as a synchronisation primitive.
  *Needs:* V003

- [x] **V005 · Tape library convention**
  One tape per screen id: `tapes/<id>.tape` → `Screenshot artifacts/<id>.png`, plus a GIF where
  motion is the point. `Hide`/`Show` around setup so only the interesting frames are captured.
  *Done when:* `just tape 1a` regenerates one screen; `just tapes` regenerates all. *Needs:*
  V004

- [ ] **V006 · Deterministic fixture repo**
  A committed sample tree plus **seeded store state** — regions, seen-state, threads, a canned
  transcript. Without this, every agent-surface tape is flaky, because the content varies run to
  run. Seed it through `phosphor --eval` (`T023`), not a test-only backdoor.
  *Done when:* the fixture tree is committed, and a seeding run drives every call through
  `phosphor --eval` with no test-only backdoor. *Needs:* V005, T023

  > **Split at the `CP-3` audit, on the `T022` precedent** — and the ruling is that a task whose
  > *mechanism* is provable now and whose *subject* arrives two windows later should not be a
  > binary. `V006` keeps the half that exists: the fixture tree and the seeding mechanism. **The
  > seeded store state moves to `T041`**, the S5 task that lands the store, and `CP-5`'s sweep is
  > where the original sentence — *"`CP-5`'s tapes produce identical output on two machines"* —
  > is finally answered. Nothing is dropped; it is recorded where it can be closed instead of
  > sitting unticked across two windows with no record of which half exists.

  > **CP-3 audit — partial, not ticked, and correctly so.** The buildable half landed:
  > `fixtures/` (14 source files), `fixtures/seed/plan.scm` (127 lines) and
  > `scripts/seed-fixtures.sh`. **Outstanding: the seeded store state, which is the point.**
  > Every capability the plan calls refuses today because none of `S5`–`S8` exists, so there
  > is nothing to make a `CP-5` tape deterministic *with*, and `CP-5`'s tapes do not exist to
  > compare. This criterion cannot be met before `T041`.

- [x] **V007 · Pixel-diff runner**
  Compare fresh captures against committed references; on mismatch, emit a side-by-side diff
  image and **fail soft with a request to look**, not a build break. Reference updates are an
  explicit, reviewed commit — never automatic.
  *Done when:* a deliberate one-cell colour change is caught and produces a legible diff image.
  *Needs:* V005

- [ ] **V008 · CI wiring**
  Tier 1 snapshots **gate**. Tier 2 runs, uploads artifacts, and posts the diff summary without
  blocking. Keep them in separate CI jobs so a flaky renderer can never redden a correct build.
  *Done when:* a Tier-1 failure blocks merge and a Tier-2 diff does not. *Needs:* V007, T005

  > **CP-3 audit — partial, not ticked.** The Tier-2 half is met by construction: the new
  > `tapes-diff` job in `.github/workflows/ci.yml:145` carries job-level
  > `continue-on-error: true` at `:148`, so it cannot redden a run. **Outstanding, two
  > things.** (1) *Nothing blocks a merge.* `gh api repos/TrevorS/phosphor/branches/main/protection`
  > answers `404 Branch not protected` (run this session), so no required status check exists
  > and a red `test` job stops nothing — that is a repo-admin setting, not a file. (2) The
  > `tapes-diff` job has never executed; its `ffmpeg` step installs whatever `apt` resolves
  > and `tapes/check-versions.sh:16` pins `8.1.2`, so its first real run is expected to fail
  > the version gate and produce no Tier-2 signal.
  > It also currently reddens `just lint`: `ci.yml:70` quotes `insta 1.48.0`, and
  > `scripts/doc_claims.py:214` reads any `1.NN.N` in that file as a stale toolchain quote.
  >
  > **CP-3 re-audit (repair pass) — unchanged, and item (1) re-checked.**
  > `gh api repos/TrevorS/phosphor/branches/main/protection` still answers
  > `404 Branch not protected`, run again this session. Nothing blocks a merge. The `just lint`
  > half is fixed — `lint-doc-claims` reports clean on this tree.

- [ ] **V009 · Degradation tapes**
  `Env TERM xterm-256color` and `Env NO_COLOR 1` variants of the core screens, exercising the
  fallback paths (`▎` markers, underline instead of undercurl, static `✻`).
  *Done when:* the degradation path is captured for `1a` and `2a` without touching a real
  terminal. *Needs:* V005

  > **CP-3 audit — partial, not ticked.** `1a` has both variants captured —
  > `tapes/1a-degraded-term.tape` and `tapes/1a-degraded-nocolor.tape`, artifacts present.
  > **Outstanding, two things.** (1) `2a` cannot be captured: it is the unseen picker and
  > `T046` is `S5`, unbuilt. (2) None of the three named fallback paths renders on `1a` today
  > — the `▎` marker needs regions (`T041`), no undercurl consumer exists on that screen, and
  > the spinner needs `T051` — so the `TERM` variant is a no-op capture and the `NO_COLOR`
  > variant exercises `crossterm`'s own handling rather than ours.

---

## M-0 · Scaffolding and the spikes

Nothing here is blocked on a decision. The two spikes are reads, not builds, and they size
everything after them — do them first, together.

- [x] **T001 · Cargo workspace skeleton**
  Seven crates (`phosphor`, `-core`, `-buffer`, `-ui`, `-agent`, `-steel`, `-vcs`) plus
  `runtime/` as a plain source dir. Stub lib/main only.
  *Done when:* `cargo build` green. *Needs:* —

  > **It is eight in the build.** `T014`'s terminal lifecycle — raw mode, alt screen, panic
  > restore, the synchronized-output wrapper — is neither a widget (`surface`'s `phosphor-ui`)
  > nor one of the three binary files the ownership table names for `spine`, so it landed as
  > `phosphor-term`. The `members = ["crates/*"]` glob enrolled it without a root manifest edit,
  > so no single-writer rule was crossed. Recorded here rather than quietly changing the number.

- [x] **T002 · Pin the dependency floor**
  `ratatui 0.30.2`, `ratatui-core 0.1.2`, `steel-core =0.8.2` (exact, per Q5), `ropey`,
  `tree-sitter`, `crossterm 0.29`. `phosphor-ui` gets `ratatui-core` only — never `ratatui`.
  *Done when:* `cargo tree` shows no second ratatui major. *Needs:* T001

- [x] **T003 · Vendor `ratatui-code-editor`**
  `git subtree` into `vendor/`, workspace path dep, `VENDOR.md` with upstream SHA + patch log,
  `just vendor-diff` and `just vendor-pull`.
  **Two inherited-baggage items the spike flagged and nothing else owns:** feature-gate the
  fork's **16 hard grammar dependencies** down to our set (it bundles bash, c, c-sharp, cpp, go,
  java and more that we never load, and bundles neither Scheme nor CSV that we do), and confirm
  its non-optional **`arboard` + `rust-embed`** don't break a headless CI or VHS run — `arboard`
  pulls system clipboard libraries that a bare Linux runner won't have.
  *Done when:* every hunk `just vendor-diff` prints against the merged SHA has a matching
  `VENDOR.md` entry, only the grammars we load are compiled in, and the workspace builds in a
  container with no X11/Wayland.
  *Needs:* T001

  > **One criterion was never executed.** The container build with no X11/Wayland has no verdict:
  > the Docker daemon was down on the build machine at `CP-0` and at `CP-1`. What stands in its
  > place is structural and is checked by `just vendor-build-headless` — `arboard` is absent from
  > the default dependency graph, and `--features clipboard` brings it back, so the gate is live
  > in both directions. That proves nothing links a clipboard library by default; it does not
  > prove a bare Linux runner builds. **CI now exercises `ubuntu-latest` on every push**, which is
  > closer to the real question than anything available when this was written.

  > **Why not "an empty diff".** The first wording asked for one, which this task cannot satisfy:
  > gating 16 grammars and gating `arboard` *are* patches, so a clean fork and an empty diff are
  > mutually exclusive. The contract that means something is that no hunk is undocumented — that
  > is what keeps the fork from silently becoming a rewrite. Against the **SHA**, not a tag:
  > upstream published `0.0.6` to crates.io without ever tagging it.

- [x] **T004 · Vendor `ratatui-markdown` and bump it to 0.30**
  Version bump only — no phosphor behaviour inside it (Q4). Feature-gated; per-language
  highlight features off.
  *Done when:* it compiles in-workspace and the gate can be toggled off cleanly. *Needs:* T002

- [x] **T005 · CI: fmt, clippy, test**
  `cargo fmt --check`, `clippy -D warnings`, `cargo test` on every push.
  *Done when:* green on the empty workspace. *Needs:* T001

  > **Written blind, and it showed.** There was no git remote until after `CP-1`, so this
  > workflow was authored, reviewed and marked done without ever executing. When a remote finally
  > appeared it ran for the first time and needed a fix that only a real runner could surface:
  > three of its five jobs called `just` with no install step. Five jobs run on every push now —
  > `fmt`, `clippy`, `nextest`, `deny`, `lint` — and CI invokes the same `just` recipes a human
  > does, so "green in CI" and "green on my machine" cannot drift.

- [x] **T006 · Structural lint — no literal colours in `phosphor-ui`**
  Every widget takes `&Theme`. Grep-level lint over `Color::Rgb` / `Color::Indexed` in that
  crate is sufficient.
  *Done when:* CI fails on a deliberately planted literal. *Needs:* T005

- [x] **T007 · Structural lint — no store mutation from `phosphor-ui`**
  Split `phosphor_core::vm` (ViewModels) and `phosphor_core::view` (the view tree, Q12) — both
  public to the UI — from `phosphor_core::store` (mutation, not). Enforced by dependency
  direction, not convention.
  *Done when:* CI fails on a deliberately planted `store::` import in `phosphor-ui`.
  *Needs:* T005

- [x] **T083 · Grammar ABI check** *(spike finding)*
  The grammar crates were built against tree-sitter bindings spanning **0.23–0.25** while the
  runtime is **0.26**. tree-sitter versions its language ABI and mixing generations is a known
  breakage source. Load all **eleven** grammars we ship (the first-class twelve minus CSV, which
  `T082` implements by hand) and parse a fixture.
  Also settle here: does `tree-sitter-scheme` (0.24.7) actually parse real `runtime/*.scm`?
  Steel is a Scheme dialect, not Scheme.
  **This is the one unknown the spikes surfaced and did not close** — cheap in M-0, expensive at
  S4, where `T037` assumes it already passed.
  *Done when:* every bundled grammar parses a fixture under tree-sitter 0.26, with a written
  answer on Steel. *Needs:* T002

- [x] **T008 · SPIKE — the five seams in `ratatui-code-editor`** ✅
  Answered in [SPIKES.md](SPIKES.md) with `file:line` citations. Scroll authority clean; marks
  partial (colour spans, no id, no style); gutter not injectable but compose-around works;
  virtual text absent **but `VisualRow` is a clean hook**; diff view **not separable**. Undo
  history opaque, but the edit primitives are public and replayable — Q2 resolved.
  **Plus: no soft-wrap exists anywhere in the crate.** *Was:* T003

- [x] **T009 · SPIKE — can edtui's handler emit Actions?** ✅
  Answered in [SPIKES.md](SPIKES.md). Structurally yes, practically pointless — the resolver is
  28 lines, the 185-entry register is dead weight against Steel keymaps, and the model cannot
  express counts or named registers. **edtui is dropped**; Q3 inverts. *Was:* T002

### ✋ CP-0 — Go/no-go on both bought crates · **PASSED**

The most consequential checkpoint in the build, and the cheapest — both spikes were reads. The
findings, with `file:line` citations, are in [SPIKES.md](SPIKES.md).

**Outcome, against the decision table this checkpoint was written with:**

| Spike question | Result |
|---|---|
| `ratatui-code-editor` seams | **Mixed, and vendoring stands.** Scroll authority clean; marks partial; gutter compose-around; virtual text absent but `VisualRow` is a clean hook; diff view not separable. The `ropey` + `tree-sitter` fallback is **not** triggered. |
| Undo serialisable? | **No — and it doesn't matter.** The bought `History` is opaque, but `Edit`/`EditBatch` are public and `apply_batch` replays them, so we keep our own log. [Q2](IMPLEMENTATION-PLAN.md#q2) stands as decided. |
| edtui handler → Actions? | **Yes, but don't.** [Q3](IMPLEMENTATION-PLAN.md#q3) **inverts** — edtui is dropped and the input machine is ours. |

**Two things the checkpoint did not think to ask, and should have:**

1. **There is no soft-wrap in the vendored crate.** Unbudgeted, lands in S1 as T081, and it
   touches row↔line mapping, cursor position, click targeting and virtual text at once.
2. **`DiffBody` has no bought base.** `mod diff` is private and the diff is a mode of the
   Editor, so T063 is rebuilt on `similar`.

Both are recorded below. The lesson for later checkpoints: a decision table enumerates the
outcomes you predicted, and the useful findings are often the ones outside it.

---

## S1 · Theme + BufferView + StatusLine shell

First phase with anything to look at. Sized by CP-0.

> **What drives editing at S1.** The goal inherited from the build order is *"renders and edits a
> file with highlighting on day one"* — but the input machine is `T026`, at S3. Until then, S1
> rides the vendored crate's own `editor_crossterm` handler as a **temporary** input path, which
> is what makes `T081`'s cursor-motion and click checks possible at `CP-1`. It is scaffolding, not
> a keeper: `T026` replaces it outright, and nothing above it may grow to depend on it.

- [x] **T010 · `Theme` struct**
  Actor/state palette (`claude, you, attention, trouble, transient, steel`) + neutral ramp +
  syntax map. Values from Design Language §1.
  *Done when:* every colour in the language has a named field. *Needs:* T001

- [x] **T011 · base16-style loading + actor-hue validation**
  A theme that reassigns an actor hue is **rejected at load**, not accepted-and-warned.
  *Done when:* a fixture theme with a red `claude` fails to load with a legible error.
  *Needs:* T010

- [x] **T012 · Phosphor dark + light built in**
  Dark is the default. Light is "warm paper with deepened hues" — claude-green `#1a9a62`.
  *Done when:* both load and pass validation. *Needs:* T011

- [x] **T013 · Catppuccin + Tokyo Night mappings**
  The two shipped mappings (Q7 — Ayu is out). Each dark + light. Screen `9a` is Catppuccin;
  Tokyo Night has no mockup and inherits `9b`'s acceptance shape.
  *Done when:* all four pass actor-hue validation. *Needs:* T011

- [x] **T014 · Terminal setup + synchronized output**
  Raw mode, alt screen, panic/exit restore, and a **draw wrapper that puts every frame inside a
  synchronized-output block**. Kitty keyboard protocol negotiation with fallback detection.
  *Done when:* no frame can be emitted outside the wrapper (enforce by making the raw writer
  private). *Needs:* T002

  > **This is `spine`'s, and it landed in its own crate.** The breakdown listed it under
  > `surface`, which built it — but terminal lifecycle is neither a widget nor one of the three
  > binary files the ownership table named, so it became `phosphor-term`, an eighth crate. Settled
  > to `spine` after `CP-1`: its only production consumer is the binary, it is where `crossterm`
  > and `ratatui` live (both of which `scripts/lint-no-app-layer-in-ui.sh` forbids in
  > `phosphor-ui`), kitty-protocol negotiation is input and input is `T026`, and it draws nothing.
  > Same rule that moved `T034`/`T035` to `surface` — **the file decides the task.**

- [x] **T015 · BufferView — the 3-column contract**
  1-cell state bar → line numbers (`#414b42`, always) → text. Tree-sitter highlighting through
  the vendored core. **Scroll authority lives here** — the viewport moves only on an explicit
  Action.
  *Done when:* a file renders with correct columns and the viewport provably never
  self-scrolls. *Needs:* T014, CP-0

- [x] **T016 · Folds and whitespace marks**
  Fold rows render `▸ ⋯ n lines`. Insert-only trailing-whitespace marks. Folds come from the
  vendored crate's existing `VisualRow::FoldSeparator`.
  *Done when:* screen `8e`'s fold and whitespace details reproduce **from a keystroke** — `za`
  closes a fold in the running binary and `zR` opens it. *Needs:* T015

  > **The wording was *"reproduce"* until the `CP-3` audit, and that is the whole story of this
  > task.** See *The wording standard for a done when* above; `T016` is its worked example, and
  > `driven::za_closes_the_fold_the_cursor_is_in` in `crates/phosphor/tests/loop_pty.rs` is what
  > the new sentence asks for.

  > **CP-3 re-audit (repair pass) — the tick stands, and the fold half only became true this
  > pass.** `crates/phosphor-core/src/action.rs` declares `SetFold`/`FoldAll`/`UnfoldAll`
  > against `"T016"`, and until the repair pass nothing in the host had an arm for any of
  > them — the widget drew folds nobody could close. `crates/phosphor/src/main.rs:1911`,
  > `:1918` and `:1922` are the three arms; `runtime/keymaps.scm:586`–`593` are `za zc zo zM
  > zR`; `crates/phosphor/tests/loop_pty.rs:532`
  > `driven::za_closes_the_fold_the_cursor_is_in` presses `za` on a real `.rs` file and watches
  > the fold body leave the frame, then `zR` and watches it come back.
  >
  > **The whitespace half lost its only live evidence in the same pass.**
  > `tapes/artifacts/insert-whitespace-marks-{normal,insert}.png` were byte-identical — the
  > NORMAL and INSERT captures showed the same bytes, so even the mode chip had not changed
  > between them — and `scripts/lint-repo-hygiene.sh` fails on an undocumented identical pair.
  > They were **deleted** (with the `.gif`) rather than recaptured or entered in
  > `tapes/artifacts/DUPLICATES.md`, which greens the lint and removes the artifact. The
  > wiring is real (`crates/phosphor/src/main.rs:903` calls `soft_wrap::set_mode` with
  > `machine.mode()` every frame), but nothing on a shipping screen shows it: there is no pty
  > test for the marks and no capture. `tapes/README.md:684` still says
  > *"`insert-whitespace-marks.tape` — captured"*, which is false against the tree.
  > `just tape insert-whitespace-marks` is the thing that answers it.

- [x] **T081 · Soft-wrap** ⚠️ *unbudgeted — surfaced by the T008 spike*
  **The vendored crate has none.** `↪` continuations carry no line number. Build it as a
  `VisualRow` variant alongside the existing fold and ghost variants, **not** as a layer above
  them — row↔line mapping, cursor positioning, click targeting and virtual-text placement all
  read the same row stream, and a soft-wrap that lives outside it desynchronises all four.
  The fourth subsystem soft-wrap touches — **virtual-text placement on a wrapped line** — cannot
  be verified here: `VirtualText` is `T032`, three windows later. Build the row-stream contract
  that serves it now; the check itself moved to `T032`'s *done when* and to `CP-3`.
  *Done when:* a long line wraps with continuations, and **cursor motion and mouse clicks** land
  correctly on a wrapped line. *Needs:* T015

- [x] **T085 · Undercurl in the vendored renderer, with underline fallback** 📌
  The marks API is **colour only** — no style, no priority (`editor.rs:660-682`) — so the
  undercurl half of Design Language §3's anchored-region treatment (*"tint + undercurl"*) is
  ours to add to the fork. A cell-style capability with a degradation path, so it belongs beside
  the renderer patches rather than with its first consumer.
  Consumers: `T040` (diagnostics), `T068` (anchored regions). Landing it at S1 also means `V002`
  can settle *"does undercurl survive VHS capture"* against a real implementation.
  *Done when:* a styled span renders undercurled on the primary terminal and underlined on the
  degradation terminal, from one call site. *Needs:* T015

- [x] **T017 · StatusLine**
  Mode chip (the only inverted text on screen) + file + dirty flag + spring + `SessionState`
  (renders `None` for now) + counters, joined by `│`. **Truncation enforced in the widget** —
  emitting a second row must be impossible, not merely avoided.
  *Done when:* a property test at widths 40–200 never produces two rows. *Needs:* T010

- [x] **T084 · `Float` — the chrome primitive** 📌
  **The one chrome primitive**, and the first breakdown had no task for it — only the passive
  variant at `T038`, three phases after `T021` already needs a float to show a broken `init.scm`
  in. Block wrapper enforcing **header / body / footer**; mood border (`#2a5c44` informational,
  `#6b5426` needs-you with `#171207` body); the **one-float rule** — opening a second replaces
  the first, `esc` closes top-down, no float-over-float ever; background under it dims to
  `#232823`; full-width under 100 cols; padding 1 row / 2 cols; spans 60–80% of width, centered,
  never within 4 cols of an edge.
  Bodies plug into it and ship with their own tasks: `Picker` (`T045`), `DiffBody` (`T063`),
  `QuestionBody` (`T059`), `HelpGrid` (`T086`), `ArchDiagram` (`T048`).
  *Done when:* both moods render with the full header/body/footer contract, a fixture body plugs
  in, and opening a second float provably replaces the first rather than stacking.
  *Needs:* T010, T014

- [x] **T018 · Golden-frame snapshot harness**
  Render a fixed buffer + state to a cell grid, compare against a committed snapshot. This is
  how every later phase gets cheap regression cover on layout.
  *Done when:* snapshots exist for `1a`-minus-agent, `9c`, `8c`, `8d`. *Needs:* T012, T013, T015,
  T017

- [x] **T090 · The S1 host — something to actually run** 📌
  **`CP-1` says `cargo run -- src/some_real_file.rs`, and until this task nothing in the build
  makes that draw a single cell.** Windows A and B produce a widget layer with no application
  around it: `main.rs` stayed `fn main() {}`, so every screen tape died on `Require phosphor`,
  the width sweep and four-theme sheets could not be produced, and `CP-1`'s manual half — four
  terminals, the whole point of the checkpoint — had nothing to open. The first `CP-1` attempt
  failed on exactly this and nothing else.
  A **thin** host, and thin is the requirement: open the file named on argv, build the frame from
  `Theme` + `BufferView` + `StatusLine` + `Float`, draw it through `T014`'s synchronized-output
  wrapper, and quit cleanly. Plus `--theme <slug>`, which eight tapes already assume and no crate
  implements.
  **Input rides the fork's `editor_crossterm` handler**, exactly as the S1 preamble above says S1
  does — which means turning the fork's `crossterm` feature back on, since `default-features =
  false` is why that handler was never compiled and why `toggle_fold_at_mouse` warns dead in every
  build. It is scaffolding with a demolition date: `T019`'s `Action` enum and `T026`'s input
  machine replace it outright, and **nothing above it may grow to depend on it.**
  *Done when:* `cargo run -- <a real source file>` renders `1a`-minus-agent on a real terminal,
  `--theme` switches between all six, arrow keys and clicks move the cursor through the vendored
  handler, `q`/`esc` restores the terminal, and every tape in `tapes/` gets past `Require phosphor`.
  *Needs:* T014, T015, T017, T084

  > **Why `spine` owns it, and why it is Window B rather than Window C.** It writes
  > `phosphor/main.rs`, which the ownership table gives `spine` — and it is the reason TEAM.md
  > lists `spine` as live in Window B while giving it no numbered task there. It is not `T019`
  > through `T026`: no `Action` enum, no Steel, no input machine, no panes. It is the app shell
  > the S1 preamble already assumes exists, which no task numbered.

### ✋ CP-1 — Does it look like the mockups? · **PASSED**

**Both halves, on 2026-08-12.** The mechanical half is below; the manual half was Teej on the
four-terminal matrix, and it passed on all six checks — `1a` against the mockup at 100×25, the
six themes including the light-mode contrast question, a live 200→40 resize, floats in both
moods with the dim behind them, soft wrap with cursor and click on a continuation, and undercurl
across the matrix.

**Four rulings came out of it**, three of which amend design docs and are tabled in
[§5](IMPLEMENTATION-PLAN.md#5-decisions): §10's brightest-colour contract became dark-mode only,
`8d`'s shed ladder is fit-driven rather than width-labelled, the statusline's bars join the
counter group only, and undercurl detection now consults `TERM_PROGRAM` before the plain-`TERM`
family rule. The fourth is code-only and shipped with them.

**Window C is unblocked.**

The baseline visual checkpoint, and the one that establishes your terminal matrix. Everything
after this trusts that colours and frames are right.

**Run:** `cargo run -- src/some_real_file.rs`

**Claude verifies:** snapshot tests pass for `1a`-minus-agent, `9c`, `8c`, `8d` · statusline
property test (widths 40–200, never two rows) · all four themes pass actor-hue validation · the
planted bad theme is rejected · no `Color::Rgb` literal survives in `phosphor-ui`.

**Also verify (new since the spike):** soft-wrap (`T081`) — long lines wrap with `↪`
continuations carrying no line number, and **cursor motion and mouse clicks** land correctly on a
wrapped line. This is unbudgeted work we now own, so it gets explicit attention at the first
checkpoint that can see it. **Virtual text on a wrapped line is checked at `CP-3`, not here** —
`VirtualText` is `T032`. What `CP-1` can confirm is that the row stream has a place for it.

**Also verify (added by the docs review):** the `Float` contract (`T084`) — both moods, header /
body / footer, and a second float **replacing** the first rather than stacking · undercurl
(`T085`) renders on the primary terminal and degrades to underline on the degradation terminal.

**VHS produces:** stills for `1a`-minus-agent, `9c`, `8c`, `8d` · a **width sweep** contact sheet
at 200/120/100/80/60/40 columns showing the shed order step by step · all four themes on the same
buffer, side by side · the `V009` degradation variants. This is also where VHS itself gets
calibrated (`V002`), so budget for the harness costing more than the review at this one
checkpoint and less at every one after.

**Teej verifies — on all four terminals in the matrix:**
- Open `1a`'s file side by side with the mockup. **Does it look right?** Not "are the colours
  the right hex" — a snapshot proves that — but does the density, the gutter, the line-number
  weight read the way the mockup reads.
- Both themes, and specifically: is claude-green still the brightest thing on screen in light
  mode? That's the contract, and light mode is where it usually breaks.
- Catppuccin and Tokyo Night: does the actor contract survive a foreign palette, or does it
  just look muddy?
- Resize to 80 columns and below. Watch the shed order happen live.
- On the degradation terminal: no garbage, no missing glyphs, no boxes.
- The recurring sweep (above).

**Fails if:** anything wraps, tears, or requires squinting at the mockup to tell whether it's
right. Ambiguity here compounds — every later screen inherits this baseline.

---

## S2 · Steel runtime, the Action spine, and the REPL

The plan calls this the decision the rest of the build is hardest to reverse. Take the extra
care here rather than at S5.

- [x] **T019 · `Action` enum + query vocabulary**
  The single mutation API — buffer edits, seen marks, session messages, float open/close. Plus
  the read side over ViewModels. Design it for the surfaces in *all* the mockups, not just S1's.
  *Done when:* every mutation in phases S3–S8 has a named Action, even if unimplemented.
  *Needs:* T007

- [x] **T020 · The tri-door registry**
  One registration per capability yields the Steel binding, the MCP tool schema, and the CLI
  verb. Adding a capability to one door must add it to all **by construction** — this is
  invariant 2's only real defence.
  *Done when:* a new Action registered in one place appears in all three doors with no further
  edits. *Needs:* T019

- [x] **T021 · Embed `steel-core`; boot `init.scm`**
  A **broken `init.scm` boots the editor anyway**, with the error in a float. Steel can emit
  Actions and read ViewModels — nothing else.
  *Done when:* a syntax error in `init.scm` yields a working editor with a legible error float.
  *Needs:* T020, T084

- [x] **T022 · Steel REPL**
  The primary extension workflow, not a debug tool. `(keymap-set! …)` is live; the next frame
  has it.
  *Done when:* a rebind typed at the REPL is in force on the very next keystroke, with no
  restart. *Needs:* T021

  > **The criterion was split at `CP-2`, on Teej's ruling.** It read *"screen `6b`
  > reproduces"*, and `6b` cannot reproduce at S2: Steel resolves a lambda's free identifiers
  > at *definition*, and three of the four lines `6b` types name `goto`, `claude` and
  > `region-author`, which belong in `runtime/` over records `T041` returns at S5. The chrome,
  > the prompt glyphs, the header and the statusline do reproduce — committed as a Tier-1
  > golden frame at `crates/phosphor/tests/snapshots/`, refusals and all, with a per-line note
  > saying which task closes each gap. Full `6b` reproduction moves to the S5 task that lands
  > the store, and `CP-5`'s sweep re-checks it. The liveness half was verified on a real pty,
  > not only in tests.

- [x] **T023 · `phosphor --eval` (the CLI door)**
  Nearly free once the registry exists.
  *Done when:* `phosphor --eval` and the REPL return identical results for the same expression.
  *Needs:* T020

- [x] **T024 · Door-parity test**
  A test that **enumerates the registry** and asserts every capability is present in all three
  doors. Enumeration is the point — a hand-written list rots.
  **Careful about what "reachable" means before S6.** The MCP *server* is `T052`; at S2 there is
  no consumer. So the test asserts what exists at each phase: the Steel binding and the CLI verb
  are invoked end to end, and the **MCP tool schema is generated and well-formed** for every
  capability. `T052` upgrades the MCP third to a live round-trip without changing the test's
  shape — the point is that a capability can never be *registered* in fewer than three doors.
  *Done when:* adding an Action wired to only one door fails CI. *Needs:* T020, T023

- [x] **T025 · StatusLine composition moves to Steel**
  Not just segment *order* — the statusline is **composed as a view tree returned from Steel**
  (Q12): which segments, in what order, with what shed priority. The first real surface to prove
  the tree protocol on, and small enough to get wrong cheaply.
  *Done when:* redefining the whole statusline composition in the REPL changes the next frame.
  *Needs:* T017, T022, T079

  > **Both surfaces compose, and the `CP-2` gate is why.** As first built, only the REPL
  > surface asked Steel; the buffer surface fell through to the Rust widget. The gate found it
  > by **deleting `runtime/statusline.scm` and watching a statusline still draw** — which is
  > the *"config file with a Rust editor hiding behind it"* reading `CP-2` exists to catch.
  > There is now no widget fallback on the buffer path: a layer that composes no statusline
  > draws none. Re-verified the same way, under a pty, against the shipped tree and against a
  > copy with the file removed. The `phosphor-ui` widget stays for its own golden frames.

> **Appended after the initial breakdown** (Q12). Ids are assigned in order of creation, not
> position, so `T001`–`T077` keep the meanings they were committed with.

- [x] **T078 · The view-tree protocol**
  `phosphor_core::view` — the tree as plain data: **no Steel dependency, no ratatui
  dependency**, so neither side owns the contract. Node kinds for every `phosphor-ui` primitive
  plus layout and the `spans` escape hatch.
  *Done when:* the crate compiles with neither `steel-core` nor `ratatui` in its dependency
  tree. *Needs:* T019

- [x] **T079 · Tree interpreter + frame cache**
  `phosphor-ui` walks a view tree into ratatui calls. **Rust caches the last tree and redraws
  every frame without re-entering the VM** — Steel re-runs only when a ViewModel changes. This
  is the whole reason a pre-1.0 scheme VM can sit under the UI safely.
  *Done when:* a benchmark shows VM invocations per second **flat** while frames per second
  climbs under streaming load. *Needs:* T078, T021

- [ ] **T080 · The `spans` escape hatch**
  One primitive taking styled rows from Steel, for surfaces the primitive set doesn't cover.
  Deliberately the *only* way to draw something custom without a Rust change — one grep-able
  name to check when a frame-budget regression shows up.
  *Done when:* `:arch` (T048) is built entirely from it, with no primitive of its own.
  *Needs:* T079

  > **Built at S2, deliberately not ticked.** The primitive exists, is reachable from Steel,
  > and `scripts/lint-one-escape-hatch.sh` proves it is the *only* custom-draw path — verified
  > on two planted violations. What cannot be met until S5 is the criterion as written: `:arch`
  > is `T048`, and there is no store to query. `T048` ticks this.

### ✋ CP-2 — Is the editor live? · **PASSED**

**Run:** `cargo run` then `:steel` (or however the REPL is bound at this point)

**Claude verifies:** door-parity test enumerates the registry and passes · a planted
one-door-only Action fails CI · broken `init.scm` still boots · `--eval` and REPL agree on a
fixture expression · `6b` snapshot · **`phosphor_core::view` has neither `steel-core` nor
`ratatui` in its dependency tree** · **the frame-cache benchmark shows VM invocations per second
flat while FPS climbs** (T079).

**VHS produces:** a clip of a REPL rebind taking effect on the very next frame — the liveness
claim is about *motion between two states*, so a still can't carry it · the broken-`init.scm`
boot with its error float · `6b`.

**Teej verifies:**
- Rebind a key from the REPL. Does it take effect on the very next keystroke, no restart?
- Redefine a statusline segment from the REPL. Does the next frame have it?
- Break `init.scm` on purpose. Does the editor still boot, and is the error float actually
  readable?
- Redefine the **whole statusline composition** — not just segment order — from the REPL. The
  view tree is the thing being tested; if composition still feels like filling in a Rust-shaped
  form, the protocol is drawn at the wrong level.
- **The judgement call:** read `runtime/*.scm` as it stands. Is this the editor layer, or is it
  a config file with a Rust editor hiding behind it? Apply both placement tests — *would two
  reasonable users want this to differ?* and *does it produce pixels, or decide which pixels?*
  If policy is accreting in Rust, this is the cheapest moment in the entire build to correct it.

**Fails if:** anything needs a restart, or the Steel layer reads as configuration rather than
implementation. The plan is explicit that excavating baked-in Rust into Steel later is a
rewrite, not a refactor.

---

## S3 · Input, persistent undo, and the gutter layer

CP-0 settled the shape: **the input machine is ours.**

- [x] **T026 · The input machine**
  Modes, operator-pending, text objects, and — designed in from the start, not retrofitted —
  **numeric counts (`3dd`) and named registers (`"ayy`)**, which are exactly what the dropped
  crate could not express. Emits `Action`s; keymaps come from Steel (T033), so the resolver
  works against a table that changes at runtime rather than a compile-time map.
  Before CP-3, diff verb/object coverage against edtui's `Action` enum — a good completeness
  checklist even though we no longer depend on it.
  *Done when:* a scripted keystroke sequence produces the expected Action stream, including
  counts and named registers. *Needs:* T019

  > **Two things `T026` deletes, not wraps.** The fork's `Editor::input` ends every keystroke
  > with its own `focus()` and `Editor::mouse` calls `scroll_up`/`scroll_down` directly, so
  > wrapping them leaves two writers on the viewport and invariant 3 stops holding —
  > `Action::Scroll` is the single writer. And `T022` wired temporary per-keystroke dispatch
  > into the host to make the keymap live; that goes too. When both are gone, so do the three
  > lines in `crates/phosphor/Cargo.toml` that turn the fork's `crossterm` feature on.
  >
  > **One rule the loop obeys and no test can see.** Anything that runs arbitrary scheme —
  > a REPL evaluation, a keybinding's thunk — may move state the statusline composer reads
  > without moving the ViewModel, so the frame cache has to be invalidated by hand at each
  > such site. The `CP-2` review found the keybinding half missing by running it: a key bound
  > to `(status-order-set! 'right '())` fired and the frame that followed wrote no cells. It
  > is correct now, and it is correct by *remembering*, which is the weakest kind. `T026`
  > owns the loop; make the rule structural — one place where "arbitrary scheme ran" is
  > recorded — and it becomes testable at the same time.

- [x] **T091 · Real VM invocations, measured in the binary** 📌
  `T079`'s benchmark proves the frame cache with a *Rust* composer, because `phosphor-ui` may
  not depend on `phosphor-steel` — `scripts/lint-no-store-mutation.sh` check 2 allows it
  exactly one `phosphor-*` dependency. That makes the control arm a floor and not a lie, but
  nothing yet counts a real `steel-core` invocation against frames drawn. The binary is the
  one crate that can.
  *Done when:* a measurement in `crates/phosphor` shows real Steel invocations flat while
  frames climb, on the loop that ships. *Needs:* T026, T079

- [x] **T027 · Kitty keyboard protocol**
  Real modifier chords, with graceful fallback where unsupported.
  *Done when:* `ctrl+shift+<key>` is distinguishable from `ctrl+<key>` on the primary terminal.
  *Needs:* T014, T026

  > **CP-3 re-audit (repair pass) — ticked.** The gap the first audit named is closed:
  > `crates/phosphor/src/main.rs:876` calls `machine.set_protocol(…)` off
  > `term.capabilities().keyboard`, so `key::Protocol` is now the negotiated one and the retry
  > path in `chords.rs` can fire. Proven through the shipping loop, not around it —
  > `crates/phosphor/tests/loop_pty.rs:706`
  > `driven::the_legacy_chord_fallback_is_reachable_on_a_legacy_terminal` binds `<C-S-k>` at the
  > live REPL, presses `<C-k>` on a real pty, and asserts the leaf appears under
  > `PHOSPHOR_KEYBOARD=legacy` and does **not** under `=kitty`. Both sides in one test.
  > **Still Teej's, not Claude's:** the criterion says *on the primary terminal*, and
  > `TASKS.md:106` records that VHS's terminal does not implement the protocol. The hardware
  > confirmation is `CP-3`'s manual half.

- [x] **T028 · Agent nouns as text objects**
  `viu`, `sib`, `dih`, `:'<,'>c` register in the grammar. **They parse here and resolve at
  S5** — there is no store to resolve against yet (Q8).
  *Done when:* the grammar accepts them and they no-op cleanly rather than erroring.
  *Needs:* T026

  > **CP-3 re-audit (repair pass) — ticked, with the sentence amended.** All four forms now
  > parse and no-op cleanly. `crates/phosphor-core/tests/agent_objects.rs:128` (`viu`), `:149`
  > (`gsib`), `:175` (`dih`) drive the machine against a fixture that checks itself against
  > `runtime/keymaps.scm`; `crates/phosphor-steel/tests/shipped_grammar.rs:420`
  > `a_visual_range_is_read_as_a_range` asks the **shipped** layer and gets
  > `Ex::Run(StartThread { anchor: Selection, … })` rather than `Ex::Unknown`, because
  > `runtime/keymaps.scm:1003` now declares `c[omment]` and a range grammar reads `'<,'>` off
  > the front of the line.
  >
  > **The sentence is `gsib`, not `sib` — Teej's ruling of 2026-08-12.** `s` stays vim's
  > substitute (`runtime/keymaps.scm:525` normal, `:555` visual) and mark-seen is the `gs`
  > operator (`:475`), decoded by an arm in `crates/phosphor-steel/src/keymap.rs`. Mockup `6d`
  > says *"`s` composes like an operator"* and is the thing that changes; see
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s *Closed* §15. The `.dc.html` is imported verbatim
  > and is Teej's to amend at claude.ai.
  >
  > **Not claimed:** nothing here is proven on the pty. Resolution is `T049` (Q8), so the
  > observable behaviour of all four is *silence*, and silence on a real terminal is
  > indistinguishable from a dead key. That is the correct bar for this task and the wrong bar
  > for `T086`, which draws them.

- [x] **T029 · Undo model in `phosphor-buffer`**
  Owns the undo tree and edit semantics (Q2).
  *Done when:* undo/redo across a scripted edit sequence is exact. *Needs:* T026

  > **CP-3 re-audit (repair pass) — host wiring closed.** The binary uses the tree now:
  > `crates/phosphor/src/main.rs:147` imports `phosphor_buffer::undo::{…, UndoTree}` and
  > `:1481` is `struct Timeline`. The fork's path is deleted, not demoted —
  > `ratatui_code_editor::actions::{Undo, Redo}` no longer appears in `crates/phosphor/src/`
  > and the only surviving `apply(Undo)` is a comment at `:1462`.
  > `crates/phosphor/tests/loop_pty.rs:649`
  > `driven::undo_and_redo_walk_the_tree_through_the_loop` presses `i A <esc> i B <esc> u u`
  > and then `<C-r> <C-r>` on a real pty and reads the file back off disk, so the group
  > boundary (`<esc>`, not per-character) and the divergent branch are both proven in the
  > shipping binary rather than in `phosphor-buffer`'s own suite.

- [x] **T030 · Undo persistence in `phosphor-core`**
  Append-only log + compaction, **sharing its format and compaction path with seen-state**
  (T044). Design the format once, here.
  *Done when:* undo history survives a clean restart *and* a `kill -9`. *Needs:* T029

  > **CP-3 re-audit (repair pass) — host wiring closed, and this now *does* answer the
  > checkpoint item.** The binary opens a journal before its first frame, keyed on cwd + the
  > canonical file path (Q1). Two pty tests, two child processes, one journal:
  > `crates/phosphor/tests/loop_pty.rs:572` `driven::undo_survives_quitting_and_reopening`
  > (edit → `:w` → quit → reopen → `u` → the original bytes are on disk) and `:612`
  > `driven::undo_survives_a_kill_9` (SIGKILL, no destructor, no `fsync`, and the next session
  > undoes into what `write_all` left behind).
  >
  > **One consequence Teej will meet at the manual half:** an undo group the machine never
  > closes is never journalled, by design. Typing in INSERT and quitting *without* `<esc>`
  > restores nothing next session; `<esc>` then quit restores. Both pty tests press `<esc>`
  > for exactly this reason.

- [x] **T031 · GutterBar**
  1-cell state column, priority trouble > attention > claude-unseen > none, `▎` degradation.
  Renders from `Vec<RegionState>` — fixtures for now, real regions at S5.
  *Done when:* priority resolution unit-tested across all overlap combinations. *Needs:* T015

- [x] **T032 · VirtualText**
  `┊`-prefixed rows owned by a region id, indented to the code column. Shared by threads,
  watches, diagnostics, hints.
  *Done when:* rows interleave correctly, never shift the buffer's own line numbering, **and land
  in the right place on a soft-wrapped line** — the fourth subsystem `T081` touches, deferred to
  here because this is the first phase where there is a virtual-text row to place.
  *Needs:* T015, T081

  > **CP-3 re-audit (repair pass) — met, and honestly at the widget level.**
  > `crates/phosphor-ui/tests/virtual_text_node.rs:176`
  > `a_row_lands_on_the_right_segment_of_a_wrapped_line` builds a line that wraps three ways,
  > anchors a row mid-*middle*-segment, and asserts it hangs under that segment;
  > `a_row_never_shifts_a_line_number` is the other half. Both pass.
  >
  > Those hand-build a tree, and that is the right bar **here** and only here: the rows a
  > buffer's own regions produce are a store query (`T041`, `S5`), so no key press can put one
  > on screen and the host's state column is deliberately empty
  > (`crates/phosphor/src/main.rs:1284-1286` says so). The one `Node::VirtualText` the shipping
  > host draws today is `T035`'s unknown-key row, and that one *is* proven on a pty.

- [x] **T033 · Keymaps + leader tree in Steel**
  `SPC` leader, full ex commands, vim-style unique-prefix abbreviation.
  *Done when:* every binding lives in `runtime/`, none in Rust. *Needs:* T022, T026

  > **CP-3 re-audit (repair pass) — ticked.** `crates/phosphor-core/src/input/vim.rs` is
  > deleted, `mod vim` returns no hits anywhere in `crates/*/src`, and the only eight `.bind(`
  > call sites left in the tree are inside `crates/phosphor-core/src/input/table.rs`'s own
  > `#[cfg(test)]` module (`:384` opens it, the calls are `:395`–`:457`). The by-name
  > exemption `no_bindings_in_rust.rs` used to carry for that file is gone with it, so the
  > scan is now unconditional — stricter by subtraction. Its callers were repointed to a
  > shared fixture, `crates/phosphor-core/tests/support/mod.rs`, which checks *itself* against
  > the scheme file (`support::the_fixture_is_the_shipped_keymap`).
  >
  > **One piece of slack, worth knowing about:** that fixture carries a `NOT_YET_SHIPPED`
  > allowance at `crates/phosphor-core/tests/support/mod.rs:169` listing fourteen tags the
  > keymap had not bound yet. All fourteen are bound now (`runtime/keymaps.scm:420`–`431`,
  > `:463`–`465`, `:475`, `:530`), so every one of them short-circuits the self-check instead
  > of being verified. Deleting the list makes the fixture strictly stronger; nothing fails
  > today either way.

- [x] **T034 · KeymapFooter / WhichKey**
  Same data, two densities. Reads the **live** keymap, so Steel rebinds appear with no extra
  wiring. Keyhints spell whole commands — `:reattach`, never `:ca`.
  *Done when:* screen `3c` reproduces **from a keystroke** — pressing `SPC` in the running binary
  opens it — and a REPL rebind shows up in it. *Needs:* T033

  > **CP-3 re-audit (repair pass) — the tick is now earned by a key press.** At the first gate
  > this was `MET` on `crates/phosphor/tests/screen_3c.rs`, which hand-builds the view tree
  > (its own module docs say so) — while pressing `SPC` in the binary drew nothing, because
  > `KeyHints` appeared nowhere in `crates/phosphor/src/main.rs`. The host composes it now:
  > `main.rs:1359` `fn under(layer, machine)` reads `Layer::entries` filtered to one parsed key
  > past what is half-typed, and `:1300` renders it as a row taken off the bottom of the body.
  > It is not `SPC`-only — it opens for any half-typed prefix, which is which-key's actual
  > question. `crates/phosphor/tests/loop_pty.rs:417`
  > `driven::pressing_space_opens_the_leader_popup` presses Space on a real pty and asserts
  > `SPC` and `+claude` are on the frame and gone after `<esc>`; `:460`
  > `driven::a_repl_rebind_reaches_the_leader_popup` types `(keymap-set! "SPC z" … "zebra")` at
  > the live `:repl` prompt and finds `zebra` in the very next popup.

  > **Owed follow-up, from the `6b` amendment — the footer is not mode-aware.** `6b` draws
  > `q close` on a float whose body is a text input, where `q` types and `esc` closes (Design
  > Language §9). The drawing is the thing that changes ([README](README.md)'s amendment list),
  > and the build owes the other half: this widget already reads the **live** keymap, so making
  > it read the live keymap *for the current mode* is a small change and the footer stops
  > promising a key that does something else. It is recorded here rather than made a criterion
  > because `T034` was launched before the question was raised and its own *done when* is met —
  > whoever next writes `crates/phosphor-ui/src/key_hints.rs` closes this.

- [x] **T035 · Unknown-key hint**
  One virtual-text line naming `SPC` and `:help`, once per session, never again.
  *Done when:* screen `8e` reproduces **from a keystroke** — an unbound key draws the hint once
  in the running binary and never again. *Needs:* T032, T034

  > **CP-3 re-audit (repair pass) — same story as `T034`, same fix.** `App::ShowUnknownKeyHint`
  > has an arm in the host, the loop drains it into a session-owned latch, and it is drawn as a
  > one-row strip above the statusline through `phosphor_ui::unknown_key::strip`
  > (`crates/phosphor/src/main.rs:1293`) — a real `Node::VirtualText`
  > (`crates/phosphor-ui/src/unknown_key.rs:151`), which is what `T035` asks for.
  > `crates/phosphor/tests/loop_pty.rs:487`
  > `driven::an_unbound_key_teaches_once_and_never_again` presses `Q` on a real pty, asserts
  > `unknown key` and `shown once` are drawn, presses `Q` again and asserts nothing is.
  > (Small inaccuracy in that test's own comment: it says *"a different one"* and presses the
  > same key. The latch is per session, so the assertion is right either way.)

- [ ] **T086 · `HelpGrid` — the `:help` float body** 📌
  Screen `6d` (`:help agent-objects`) is an S3 acceptance target in the plan with no task behind
  it, and `HelpGrid` is named as a `Float` body in the Component Breakdown. Same data as
  `KeymapFooter`, third density: a full grid, read from the **live** keymap so Steel rebinds and
  `define-language` additions appear with no extra wiring.
  Per the voice rule, entries spell whole commands — `:reattach`, never `:ca`.
  **The agent nouns render here but do not resolve until `T049`** (Q8) — `6d` displays `viu` /
  `sib` / `dih` and their grammar at S3; they bind to real regions at S5.
  *Done when:* screen `6d` reproduces from the live keymap **from a keystroke** — typing
  `:help agent-objects` in the running binary draws the grid — and a REPL rebind shows up in it.
  *Needs:* T084, T034, T097

  > **CP-3 re-audit (repair pass) — still not ticked, and now for a bigger reason than
  > before.** `6d` is a composed frame that no key press can reach. `open-help` is declared at
  > `crates/phosphor-core/src/action.rs:1075` and `runtime/keymaps.scm:959` binds `:h[elp]` to
  > it, but **`OpenHelp` has no arm in `crates/phosphor/src/main.rs`** — grep it; the host's
  > `ViewAction::` arms are `Scroll`, `SetFold`, `FoldAll`, `UnfoldAll` and nothing else. So
  > typing `:help agent-objects` in the running binary produces a refusal, not this screen.
  > This is exactly the `T034`/`3c` failure mode that this repair pass existed to close, and it
  > is the one surface where it was not closed. **Wanted:** an arm in the host and one pty test
  > that types `:help agent-objects` and reads the grid off the frame.
  >
  > **Second, smaller, outstanding item.** The page composes from the **first** `Select` and
  > the **first** `Operator` bound in normal scope — `crates/phosphor/tests/screen_6d.rs:129-132`
  > — which is `v` and `d`, so it has no slot for a third head and cannot draw `gsib` however
  > the keymap is written. One line in that file makes `MarkSeen` a head.
  >
  > **Third: the snapshot's own prose is now false in three places, and no lint sees it.**
  > `crates/phosphor/tests/screen_6d.rs:213-217` (baked into
  > `crates/phosphor/tests/snapshots/screen_6d__6d.snap:11-16`) says the mark-seen operator is
  > not bound, no bracket navigation is bound, and there is no `:c` ex command. All three were
  > true at the first gate and all three are false against the tree now —
  > `runtime/keymaps.scm:475` (`gs`), `:602`–`:608` (`]u` `[u` `]b` `[b`), `:1003`
  > (`c[omment]`). The frame did not move, so `insta` passed it. The notes are the bug.

### ✋ CP-3 — Does it feel like an editor? · **PASSED**

**Both halves, on 2026-08-13.** The mechanical half is below and was green before the manual half
ran — the gate at 639 tests and 14 lints, after the repair pass that wired what S3 had built and
left dead to the keyboard. The manual half was Teej editing a real file, and **the verdict is no
findings.**

That is the whole verdict, and it is worth stating what it does *not* say. "No findings" answers
this checkpoint's question — *does it feel like an editor* — and nothing else. It does not close
the residue this build has already written down: the arms owed in *A · Arms owed* below are still
owed, the repair items still open in [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) are still open, and
the two rulings `CP-3` produced are still pending upstream against the design docs. A checkpoint
that passes on feel is not a checkpoint that passes on debt.

**`S4` is unblocked.**

The first checkpoint that is mostly about feel, and the only one where muscle memory is the
instrument.

**Run:** `cargo run -- <a real file you'd actually edit>`

**Claude verifies:** scripted keystroke → Action stream tests · undo/redo exactness · undo
survives restart and `kill -9` · gutter priority resolution across all overlaps · `3c`, `6d` and
`8e` snapshots · every binding lives in `runtime/` · **virtual-text rows land correctly on a
soft-wrapped line** (`T032`, deferred here from `CP-1` — the fourth subsystem `T081` touches, and
the first checkpoint that can actually see it).

**VHS produces:** the leader popup opening (`3c`) · folds collapsing and expanding ·
insert-only whitespace marks · the once-per-session unknown-key hint firing and then *not*
firing again (`8e`). Soft-wrap continuations were captured at `CP-1`, where they belong (T081).
Keystroke-driven surfaces are where tapes are strongest — the input is scripted, so the capture
is exact.

**Teej verifies:**
- **Actually edit something real for a while.** Not a test file — something you were going to
  change anyway. Vim habits should carry without thinking about it.
- Where does muscle memory break? Every miss is a finding; note them all, they won't recur to
  you later.
- Counts, registers, operator-pending: `3dd`, `"ayy`, `ci(`. Do they compose? **These are the
  two the dropped crate couldn't express** ([Q3](IMPLEMENTATION-PLAN.md#q3)), so they're now
  ours to get right rather than inherit — test them hardest.
- `SPC` leader popup — is the namespace learnable, or does it need the docs?
- Modifier chords on the primary terminal, then on the degradation terminal.
- Quit, reopen, undo. Does history come back intact?
- The recurring sweep.

**Fails if:** you find yourself thinking about the editor instead of the edit. This phase is
"plain editor complete" — if it isn't invisible, the agent surfaces will be built on sand.

**A failure here reopens:** T026, and possibly the CP-0 input verdict.

---

## S4 · LSP and the first-class languages

- [ ] **T036 · LSP client state**
  In `phosphor-buffer`. Blessed server auto-configured per first-class language, not merely
  discovered.
  *Done when:* rust-analyzer attaches and reports ready. *Needs:* T015

- [ ] **T037 · `define-language` + the 12 declarations**
  TS, JS, Rust, Python, Steel, Markdown, JSON, CSV, TOML, YAML, HTML, CSS — each shipping as a
  `define-language` call in `runtime/`, **not a Rust table**. Binds grammar + LSP command +
  locale hooks.
  *Done when:* a 13th language can be added from the REPL with no Rust change. *Needs:* T036,
  T022, T083

- [ ] **T082 · CSV without tree-sitter** *(spike finding)*
  `tree-sitter-csv` is 2.5 years stale with ~5k downloads, and CSV gets a hand-tuned surface
  (virtual column alignment) rather than generic buffer treatment. A small parser is more
  reliable than a stale grammar **and** yields exactly the column model that surface needs.
  *Done when:* CSV column alignment works, with no `tree-sitter-csv` dependency. *Needs:* T037

- [ ] **T038 · Completion via the passive Float**
  Border `#2a3c2e`, **no footer** — the one documented exception to the float contract. The
  primitive itself is `T084`; this adds the third mood and the footer exception to it.
  *Done when:* screen `7c`'s completion reproduces **from a keystroke** — typing in insert mode
  in the running binary raises the float. *Needs:* T036, T084

- [ ] **T039 · Signature help + hover**
  *Done when:* screen `7c` reproduces in full **from a keystroke**. *Needs:* T038

- [ ] **T040 · Diagnostics → gutter + virtual text**
  Trouble priority in `GutterBar`; `■` rows via `VirtualText`; undercurl with underline
  fallback.
  *Done when:* a file with real errors shows correct gutter priority against other states.
  *Needs:* T031, T032, T036, T085

### ✋ CP-4 — Boring on purpose

**Run:** `cargo run -- <a .rs, a .ts, and a .py file>`

**Claude verifies:** completion + signature help against rust-analyzer, tsserver, pyright ·
diagnostic gutter priority vs other region states · `7c` snapshot · a 13th language added from
the REPL with no Rust change.

**VHS produces:** the completion float opening over real code in all three languages (`7c`) ·
signature help · a file with real diagnostics showing gutter priority against other region
states. Undercurl only if `V002` established that it survives capture — otherwise it stays
Tier 3.

**Teej verifies:**
- Type in all three languages. Is completion fast enough to be useful, or fast enough to be
  annoying? Both are findings.
- Undercurl on the primary terminal; underline fallback on the degradation terminal.
- Add a language from the REPL — proves `define-language` is real userspace, not a Rust table
  with a scheme wrapper.
- The recurring sweep.

**Fails if:** it doesn't feel like a normal LSP editor. This phase should be unremarkable.

---

## S5 · The semantic store, seen-tracking, and the Picker

Where Phosphor stops being an editor. The highest-value checkpoint follows it.

- [ ] **T041 · Store core + region state machine**
  `claude writes → unseen --s--> seen`, and `claude revises → unseen again`. Seen-state is the
  only mutable flag the user owns; everything else derives. **Your own edits never create
  regions.**
  *Done when:* the state machine is exhaustively unit-tested, including revision-after-seen, and
  **`V006`'s seeded store state is reachable through `phosphor --eval`** — regions, seen-state,
  threads and a canned transcript — so `CP-5`'s tapes produce identical output on two machines.
  *Needs:* T019

  > **The second criterion arrived from `V006` at the `CP-3` audit**, on the `T022` precedent.
  > `V006` built the fixture tree and `scripts/seed-fixtures.sh`, and every capability its plan
  > calls refuses today because none of `S5`–`S8` exists — so there is nothing to make a `CP-5`
  > tape deterministic *with*. That half cannot be closed before this task and now sits on it.
  >
  > **It also owes an arm.** `set-virtual-text-visible` is declared and unapplied because
  > collapsing a virtual-text rail addresses it by owning region, and regions are this task —
  > recorded in `scripts/lint-action-arms.sh`'s RECORDED table against `T041`. Wire it here
  > rather than leaving it to a later audit.

- [ ] **T042 · Node anchoring**
  Anchors bind to tree-sitter nodes. Threads, seen-state, and watches survive rewrites.
  **And vim's marks, added by the repair window between `CP-3` and `S4`.** `goto-anchor` named an
  `AnchorId` that nothing produced and there was no setter at all, which is why `m`, `'` and
  `` ` `` are bound to silence in `runtime/keymaps.scm` rather than to a refusal naming a task.
  `place-anchor` (`crates/phosphor-core/src/action.rs`, S5, `Allow`) is that setter: it anchors a
  `Target` and answers the id, with an optional label so `m`'s `a`–`z` and a caller's own naming
  are one mechanism. Declared and unapplied. It needs no entry on
  `scripts/lint-action-arms.sh`'s RECORDED table, because its capability row cites this task and
  this task is not ticked — ticking it without an arm is what makes that lint fail.
  *Done when:* a real refactor moves code and the anchors follow, **and `jump` applies** — a
  jumplist target is an anchor, so the arm lands with the anchors — **and `place-anchor` applies**,
  so `m{a-z}` writes a mark, `'{a-z}` and `` `{a-z} `` read it back through `goto-anchor`, and
  `T098`'s third clause closes for those three keys. *Needs:* T041

  > **The `jump` half is an arm this task owes**, not a new task. `Jump` is declared and
  > unapplied, recorded in `scripts/lint-action-arms.sh`'s RECORDED table against `T042` for
  > exactly this reason. It is here rather than in the *Arms owed* section below because it has a
  > creditor already.

- [ ] **T043 · Line + content fallback anchoring**
  **The floor, not a degraded extra** — this is what makes unseen markers a store feature rather
  than a language feature (invariant 4).
  *Done when:* markers work correctly on an extensionless file with no grammar. *Needs:* T041

- [ ] **T044 · Seen-state persistence**
  `$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/`, keyed on path never VCS identity (Q1).
  Append-only log + compaction, **same format as undo** (T030).
  *Done when:* seen-state survives restart and `kill -9`, in both a jj repo and a bare
  directory. *Needs:* T030, T041

- [ ] **T045 · Picker widget**
  `ratatui-textarea` filter line + nucleo matcher **off-thread** + list + preview split (dropped
  under 100 cols). Rows are `Vec<Span>` so agent context renders in actor colours.
  *Done when:* it stays responsive filtering a 100k-file list. *Needs:* T041, T084

- [ ] **T046 · Steel picker sources — unseen, files**
  `(define-picker-source …)`. Files carries unseen counts and activity columns.
  *Done when:* screens `2a` and `3d` reproduce **from a keystroke** — the binding that opens the
  picker opens it in the running binary — and a source added from the REPL appears with no
  restart. *Needs:* T045, T022

- [ ] **T047 · Grep / symbols source**
  Tab cycles source. Results carry who-touched-them.
  *Done when:* screen `8a` reproduces **from a keystroke**, tab included. *Needs:* T046

- [ ] **T048 · `:arch` / ArchDiagram**
  A float body over a store query (Q11), **built entirely from the `spans` hatch** (T080) — no
  Rust primitive of its own. It is the proof that the escape hatch is sufficient for a real
  custom surface. Turns invariant 4 from a claim into something you can look at.
  *Done when:* typing `:arch` in the running binary reproduces screen `6a`, it reflects the
  *actual* store rather than a static drawing, and it adds zero lines to `phosphor-ui`.
  *Needs:* T041, T080, T084

- [ ] **T049 · Agent nouns resolve**
  `viu` / `sib` / `dih` now bind to real regions (completes T028, per Q8).
  *Done when:* screen `6d`'s nouns are functional, and `viu` selects an unseen region.
  *Needs:* T028, T041

- [ ] **T087 · Region tints via a marks side table** 📌
  The seam the T008 spike said the bought marks API *is* good for, and the one nothing was
  tasked to build. Design Language §3 tints the whole row per region state — `#141d16` anchor,
  `#26332a` selection-in-float, `#211114` failure — and the marks API carries exactly that
  (colour spans) and nothing else.
  Three consequences from the spike, all landing here: **marks carry no id**, so region ↔ mark
  mapping needs our own side table keyed by offset range; **`set_marks` replaces wholesale**
  (`editor.rs:660-682`), so every seen-state change re-uploads the full set — keep the upload off
  the hot path and diff before uploading; and **the state column and undercurl are not marks** —
  those are `T031` and `T085`, resolved separately and composed per row.
  *Done when:* marking a region seen retints it with no full re-render stall on a file with 500+
  regions, and the side table survives an edit that shifts every offset. *Needs:* T041, T015,
  T085

### ✋ CP-5 — The awareness loop

**The product moment.** Everything before this was table stakes; this is the thing Phosphor
exists to do. Budget real time here.

**Run:** a repo with genuine recent agent activity — hand-seed regions if no session exists yet.

**Claude verifies:** region state machine exhaustive tests · anchor-survival across a real
refactor (`6c`) · **markers correct on an extensionless file with no grammar** · seen-state
survives restart and `kill -9`, in a jj repo *and* a bare directory · picker responsive on 100k
files · a REPL-added picker source appears without restart · `1a`, `2a`, `3d`, `8a`, `6a`
snapshots.

**VHS produces:** `1a` in full against the `V006` fixture · the unseen picker with diff preview
(`2a`) · files picker with activity columns (`3d`) · grep (`8a`) · `:arch` (`6a`) · a clip of
`s` clearing a marker and the gutter updating · the no-grammar file showing line-fallback
markers. **This checkpoint is why `V006` exists** — without seeded store state none of these are
reproducible.

**Teej verifies — full terminal matrix:**
- Open a file with unseen regions. **Does the gutter pull your eye to the right places?** That
  is the entire thesis; if it doesn't, nothing downstream saves it.
- Mark seen with `s`, move on. Is that satisfying, or does it feel like busywork? "Glance, mark
  seen, move on — that *is* review" is the claim being tested.
- Have something rewrite a file underneath an anchor. Does the thread follow the node, or land
  somewhere stupid?
- Open a file type with no grammar. Markers must work — and the degradation must feel *honest*
  rather than broken.
- `:arch` — does it describe the system you think you're building?
- The recurring sweep.

**Fails if:** the markers don't change how you read the file. The awareness model is the product
bet; a lukewarm result here is worth stopping over, not building past.

**A failure here reopens:** the design brief's awareness model — a conversation, not a bug fix.

---

## S6 · The session and the directing loop

Split at the internal checkpoint from Q10. Two checkpoints.

- [ ] **T050 · ACP session client**
  `agent-client-protocol`. One Claude Code session per editor per repo.
  *Done when:* a session attaches and a turn completes. *Needs:* T019

- [ ] **T051 · `SessionState` + statusline**
  One enum — Idle, Working{elapsed}, Waiting, Paused, Lost, None — **rendered identically
  everywhere it appears**. Always present, always truthful.
  *Done when:* every state renders and the statusline is never stale. *Needs:* T050, T017

- [ ] **T052 · MCP server from the registry**
  `rmcp`, generated from T020 so the vocabulary cannot drift.
  *Done when:* Claude can call an editor tool and the same capability works from Steel and CLI,
  **`apply-edits` among them** — a batch applied as one undo group, which is the shape an agent
  writes through. *Needs:* T020, T050

  > **The `apply-edits` half is an arm this task owes**, not a new task. It is declared and
  > unapplied because there is no caller until there is a session — `T029`'s tree already
  > supports it (`record_batch`) — and it is recorded in `scripts/lint-action-arms.sh`'s RECORDED
  > table with that reason. This is the task where the caller appears, so the debt is filed here
  > rather than in the *Arms owed* section below.

- [ ] **T053 · `phosphor/declare-review-block`**
  The review-block signal as an MCP tool call carrying file+range list and per-group annotations
  (Q6). Routed through the registry, so Steel and CLI can declare one too.
  *Done when:* a declared block becomes a grouped set of unseen markers + a notification.
  *Needs:* T052, T041

- [ ] **T088 · Pane manager — splits and focus** 📌
  `T054` calls the transcript *"a pane, not a float — splits, holds focus like a window, survives
  float churn"*, and nothing was tasked to provide panes. This is that: the split/focus model in
  the binary's event loop, pane kinds (buffer, transcript, and in v1.5 claude-built), focus
  routing that survives a float opening and closing over the top, and the rule from Design
  Language §9 that **panes never dim each other** — only floats dim what's behind them.
  Placed at S6 because the transcript is the first surface that forces a second pane. If the
  files picker (`T046`) ever opens results into a *new* pane rather than replacing the current
  buffer, this moves to S5 — decide that when `T046` lands, and note the answer here.
  *Done when:* two panes split, focus moves between them, and opening then closing a float
  returns focus exactly where it was. *Needs:* T019, T015

- [ ] **T089 · `TabBar`** 📌
  Chrome strip one of three (Design Language §5), untasked until now, and the plan already
  decided to **build rather than buy** it (`ratatui-comfy-tabs`, 600 downloads). Appears only at
  2+ panes. Flat vim-style: active tab = 2px actor-coloured top rule + bright text, inactive =
  meta-gray, **per-tab unseen counts (`●n`)**. Input is `Vec<TabVM { title, kind, unseen }>`.
  *Done when:* it appears on the second pane and never on the first, and per-tab unseen counts
  track the store. *Needs:* T088, T010, T041

- [ ] **T054 · TranscriptPane**
  **A pane, not a float** — splits, holds focus, survives float churn. Turn list, prompt lines
  `❯`, prose, tool rows, seam markers. Folds by turn. Streams during Working.
  *Done when:* screen `1b` reproduces **from a keystroke** — the binding that opens the pane
  opens it in the running binary. *Needs:* T050, T088

- [ ] **T055 · Markdown prose behind the gate**
  Via the vendored fork (T004). **Plain-text path must stay readable with the gate off.**
  *Done when:* both paths render acceptably. *Needs:* T004, T054

- [ ] **T056 · OSC 8 tool-row jump links**
  *Done when:* clicking a tool row jumps to the file and range, on the primary terminal.
  *Needs:* T054

- [ ] **T057 · Session lifecycle**
  Cold start (`7d`), attach/adopt/start (`5d`), drop and reattach (`7b`), opening mid-task
  (`2d`). **Editing never blocks on session trouble.**
  *Done when:* all four screens reproduce in the running binary and the editor stays usable
  through a mid-turn drop. *Needs:* T051

### ✋ CP-6 — Does the session hold?

Half of S6 — shippable on its own: Claude is visible in the editor, but you can't yet direct
from it.

**Run:** attach to a repo with a live Claude Code session; let it work.

**Claude verifies:** session drop mid-turn → reattach → adopt, all recovering · torn-frame check
under sustained streaming load · every `SessionState` variant renders · `1b`, `7d`, `5d`, `7b`,
`2d` snapshots · **the tab bar appears on the second pane and never on the first** (`T089`), and
opening then closing a float returns focus to the pane that had it (`T088`).

**VHS produces:** the transcript streaming a full turn · the session dropping mid-turn and the
seam appearing (`7b`) · cold start (`7d`), attach/adopt (`5d`), mid-task dashboard (`2d`) · a
clip of the buffer sitting perfectly still while Claude works (`2c`).
**Explicitly not the tearing check** — a capture cannot show a tear, so `CP-6`'s highest-risk
item stays entirely on your hardware. Don't let a clean-looking clip talk you out of watching it
live.

**Teej verifies:**
- Watch a real turn stream in. **Zero tearing** — this is the highest-risk moment in the build
  for it, since streaming is the first sustained async render pressure.
- Read the buffer while Claude works. It must not move (`2c`).
- Kill the session mid-turn. Does editing keep working? Does the transcript show the seam
  honestly, or paper over it?
- Click a tool row — does OSC 8 land you in the right place? Then check it inside tmux, where
  passthrough usually breaks.
- Is the statusline session state *truthful* at every moment, or does it lag?

**Fails if:** a frame tears, or a dropped session degrades anything beyond the session itself.

---

- [ ] **T058 · PromptLine**
  The `:` line. `⚓` anchor chip when a selection rides along — visual-select, hit the prompt,
  file and range ride automatically. Routes to command parse or Claude message. Ex-style
  history.
  *Done when:* screen `1c` reproduces **from a keystroke** — pressing `:` in the running binary
  raises the line, anchor chip included. *Needs:* T050

- [ ] **T059 · QuestionBody**
  Prose + amber digit options `[1]`–`[n]` + full-command footer. Digits answer only while
  focused.
  *Done when:* screen `4a` reproduces in the running binary and its digits answer while focused.
  *Needs:* T057, T084

- [ ] **T060 · The ask queue**
  Per Q9: a question arriving while another float holds focus **sets the statusline `!` and
  waits**. Surfaces when nothing else holds focus; `]!` jumps to it. The queue is a **store
  query, not widget state**, so `]!`, the inbox, and the statusline read one truth.
  *Done when:* asking while a picker is open destroys nothing, and the `!` survives shedding at
  40 columns. *Needs:* T059, T041

- [ ] **T061 · Permission asks + rule writing**
  Screen `7a`: exact invocation shown; always-allow **writes a legible rule to `init.scm`**.
  *Done when:* the written rule is readable by a human and takes effect next time. *Needs:*
  T059

- [ ] **T062 · Interrupt and steer**
  `esc` pauses at the next tool boundary → steer / resume / abort. The seam is recorded in the
  transcript.
  *Done when:* screen `7e` reproduces **from a keystroke** — `esc` mid-turn in the running
  binary reaches the next tool boundary. *Needs:* T057

### ✋ CP-7 — The directing loop

Both loops now exist. This is the first checkpoint where Phosphor is the thing the brief
describes.

**Run:** a real working session. Direct Claude entirely from the editor for a while.

**Claude verifies:** ask-while-picker-open destroys nothing · `!` survives statusline shedding
at 40 cols · permission rules round-trip to `init.scm` and take effect · `1c`, `4a`, `7a`, `7e`
snapshots · queue state is a store query (assert no widget-local ask state exists).

**VHS produces:** **the ask-queue clip** — picker open, question arrives, `!` lights up, picker
survives untouched, `]!` answers it. Q9's whole argument is a sequence of states over time, so
this is the one artifact that settles whether the decision was right. Plus `1c` (anchor chip on
the prompt), `4a`, `7a`, `7e`, and a 40-column capture proving `!` survives shedding.

**Teej verifies — full terminal matrix:**
- **Use it for real work.** Visual-select, `:`, talk to Claude. Does the anchor riding along
  feel automatic, or do you check whether it worked every time?
- Let Claude ask you something while a picker is open. **Nothing should be destroyed.** Then
  answer via `]!`. Does the queue feel considerate or does the ask get lost? Q9's accepted cost
  is that an ask can sit unnoticed — this is where we find out whether that cost is acceptable
  in practice.
- Grant an always-allow. Read the rule it wrote into `init.scm`. Is it legible to a human six
  months from now?
- `esc` mid-turn. Does it pause at a sane boundary? Steer, resume, abort — all three.
- The recurring sweep.

**Fails if:** talking to Claude costs more than an ex command. The brief's claim is that the
`:`-prompt is the primary gesture; if it has any ceremony, that claim is false.

**A failure on the ask queue reopens:** Q9 — the alternatives (replace the float, or allow
float-over-float) are logged with their trade-offs.

---

## S7 · Diffs, review blocks, inbox, dirty-state, VCS

Three independent workstreams — `S7.1` / `S7.2` / `S7.3` — one checkpoint each, each
independently shippable. The `S7.n` labels exist because bare `7a`/`7b`/`7c` collide with the
mockup screen ids of the same name, which mean entirely different things.

### S7.1 — Review surfaces

- [ ] **T063 · DiffBody** — **built on `similar`, not on a bought widget.** The T008 spike found
  `mod diff` private and the diff implemented as a *mode of the Editor*, so there is nothing to
  restyle. Unified and side-by-side; fold rows for unchanged spans. `similar` already arrives
  transitively via the vendored crate, so this adds no dependency.
  *Done when:* renders a real diff correctly. *Needs:* T041, T084
- [ ] **T064 · Per-hunk seen state** — `s`/`S` compose over any group.
  *Done when:* marking one hunk seen leaves the rest unseen. *Needs:* T063, T041
- [ ] **T065 · Directory grouping + annotations** — `tui-tree-widget`; Claude's group
  annotations ("mechanical" vs "the meat"). **Scale is grouping, not scrolling.**
  *Done when:* screen `8b`'s 40-file block is navigable. *Needs:* T064
- [ ] **T066 · Review block + hunk peek** — screens `4b`, `2b`. *Needs:* T065, T053
- [ ] **T067 · Inbox** — one list of everything Claude said; severity is a single MCP flag;
  unread = unseen. Screen `5c`. *Needs:* T053, T041
- [ ] **T068 · Anchored exchange / threads** — your comment and Claude's reply as virtual text
  under the region. Screen `3a`. The region itself carries Design Language §3's full anchored
  treatment — **tint + undercurl** — which is `T087` and `T085` composed, not the marks API alone.
  *Needs:* T032, T042, T085, T087

### ✋ CP-8a — Can you actually review a big change?

**Run:** point it at a genuinely large agent change — 40 files if you can find one.

**Claude verifies:** per-hunk seen composes correctly over groups · `4b`, `2b`, `5c`, `8b`,
`3a` snapshots · inbox unread state derives from seen-state rather than duplicating it.

**VHS produces:** a navigation clip through the 40-file block — directories folding, `s`
clearing hunks piecewise, position surviving a stop and restart (`8b`, `4b`) · hunk peek
(`2b`) · inbox (`5c`) · anchored exchange (`3a`).

**Teej verifies:** Read a large review block end to end. Does grouping-by-directory actually
make 40 files tractable, or do you still feel lost? Are Claude's group annotations useful
enough to be worth the screen space? Does per-hunk seen let you stop halfway and come back
without losing your place? The recurring sweep.

**Fails if:** you'd rather read the diff in your terminal with `jj diff`.

### S7.2 — Dirty state

- [ ] **T069 · Changed-on-disk indicator** — `✱` + offer to refresh. **Buffer holds stable.**
  Watching disk is `notify` + `notify-debouncer-full` (added by the spike — the design requires
  this and no document listed a dependency). **Debouncing is load-bearing:** an agent writing a
  file produces a burst of events, and one `✱` per burst is the honest signal.
  Screen `1d`. *Needs:* T015
- [ ] **T070 · `:diff-disk`** — your unsaved buffer vs Claude's disk write. Three manual exits,
  **no auto-merge**. Screen `5b`. *Needs:* T063, T069

### ✋ CP-8b — Invariant 3 at its sharpest

**Run:** open a file, edit without saving, have Claude write to it underneath you.

**Claude verifies:** the buffer's content and cursor are byte-identical before and after the
disk write · `1d`, `5b` snapshots · no code path can refresh a buffer without an explicit
Action.

**VHS produces:** the single best artifact in the whole harness — a GIF of the cursor and
viewport **dead still** while the file changes underneath and `✱` appears. Invariant 3 is a
claim about absence of motion, and a recording is the only way to demonstrate absence. Plus each
of `:diff-disk`'s three exits taken in turn (`5b`).

**Teej verifies:** **Nothing moved, right?** Not "it recovered gracefully" — nothing moved.
Then `:diff-disk`, read both versions, take each of the three exits in turn. Is the choice obvious at
the moment you have to make it? This is the invariant most likely to be violated by accident,
and the most damaging when it is.

**Fails if:** the cursor moved, the viewport scrolled, or any exit silently merged.

### S7.3 — VCS

- [ ] **T071 · VCS trait + jj adapter** — compiled in, activated on detection. **No feature may
  assume a repo exists.** *Needs:* T041
- [ ] **T072 · git adapter** — same trait. *Needs:* T071
- [ ] **T073 · jj timeline** — agent turns are changes; undo is time travel. Screen `3b`.
  *Needs:* T071

### ✋ CP-8c — Does it work with no VCS at all?

**Claude verifies:** **the entire S7 acceptance set runs twice — once in a jj repo, once in a
bare directory.** Plus once in a git repo. `3b` snapshot.

**VHS produces:** the same tape run against all three fixtures — jj, git, bare — as a
three-column contact sheet. Any surface that differs between them is either a deliberate
enhancement or a bug, and putting them side by side is what makes the difference obvious.
Plus the jj timeline (`3b`).

**Teej verifies:** Work in a plain directory with no repo for a while. Does anything feel
degraded or apologetic? The brief's stance is that VCS is an enhancement and its absence is a
normal state, not an error path — if the UI hints otherwise anywhere, that's a bug. Then the jj
timeline: is undo-as-time-travel actually usable?

**Fails if:** any feature is unavailable, or any message implies something is missing.

---

## S8 · Watches

- [ ] **T074 · Watch model in the store** — anchored to nodes; first-class languages only.
  *Needs:* T042
- [ ] **T075 · Watch values over ACP** — session notifications, not MCP tool calls (Q6).
  *Needs:* T050, T074
- [ ] **T076 · WatchOverlay** — `◉ ⇒` sequences + run-provenance line, through `VirtualText`.
  **This widget only formats.** *Needs:* T032, T075
- [ ] **T077 · `(watch-place …)` from the REPL** — evaluate, and the buffer sprouts virtual
  text. *Needs:* T022, T076

### ✋ CP-9 — Watches, and the v1 ship check

Last phase, and the final full sweep before calling v1 done.

**Run:** `(watch-place …)` on a real function, then run its test suite.

**Claude verifies:** values from a real `cargo test` / `pytest` run stream into the buffer ·
watches correctly unavailable on second-tier languages · `5a` snapshot · **full regression: all
34 v1 screen snapshots** · the recurring sweep automated where possible.

**VHS produces:** watch values streaming into a buffer from a real test run (`5a`) · and the
**full contact sheet — all 34 v1 screens regenerated in one pass**, ready to hold against the
mockups side by side. This is what the harness was built for: the final visual regression is one
command and a scroll, not a day of driving the editor by hand.

**Teej verifies — full terminal matrix, one last time:**
- Watch a value stream from a real run. Is this the Light Table moment, or just virtual text?
- On a second-tier language: is the absence honest and clearly explained, or does it look
  broken?
- **Then the ship check:** walk all 34 v1 screens against the mockups. Use the editor for a full
  day of real work. The question is no longer "does each piece work" but "is this the thing the
  brief describes."

**Fails if:** you wouldn't reach for it over your current editor.

---

## A · Arms owed — verbs the vocabulary declares and the binary never applies

Cross-cutting, like the harness above, and for a related reason: these belong to no phase,
because the work is not a surface — it is the arm behind one.

`scripts/lint-action-arms.sh` was written at the `CP-3` audit and found **13** places where a
ticked task declares a mutation the binary never names. Three have a creditor already and are
recorded on the tasks that will close them: `jump` on `T042`, `set-virtual-text-visible` on
`T041`, `apply-edits` on `T052` — each is a line on that task's *done when* above, not a task of
its own, because the phase that supplies the caller is the phase that owes the arm. **The other
ten had no creditor at all.** Nothing in the task graph owned the work, so they would have sat in
that lint's RECORDED table until somebody decided. `T092`–`T097` are that decision; they exist so
the debt can rot **visibly**.

Read them beside *The wording standard for a done when* above — this is the same defect one layer
down. There, a screen reproduces in a snapshot and not on a terminal. Here, a verb is declared,
generated into all three doors, advertised in the help output, and does nothing when called.
**Mostly this is not new feature work.** Soft wrap, compaction, checkpoint traversal and the
float slot are all built and tested; what is missing is the arm between the door and them. Two
are genuinely unbuilt — the theme rebuild path (`T092`) and reloading the layer (`T094`) — and
each says so at the task.

`T098` is the seventh and a slightly different animal — a *binding* that is missing rather than
an arm — but it is the same failure to a user's hands, and it comes from the same ruling pass.

- [ ] **T092 · Runtime theme switching — the rebuild path** 📌
  `set-theme` and `reload-theme` are declared and unapplied; `runtime/keymaps.scm:965` binds
  `:th[eme]` to `set-theme` and the ex command **answers a refusal**. The blocker is structural rather than
  lazy: the theme is an immutable local at `crates/phosphor/src/main.rs:794` — `let theme =
  builtin(&cli.theme)…` — baked into each `Editor` at construction, so runtime switching is a
  **rebuild path, not an arm**. Every widget holds a `&Theme`, so they all have to be handed the
  new one and the frame cache invalidated in the same beat. `--theme <slug>` works and is what
  the theme tapes use. `reload-theme` needs one thing more — a user theme path — since that line
  only ever calls `builtin()`.
  **Teej's ruling, 2026-08-13: `:theme` stays bound.** An ex command that exists and declines
  beats one that vanished, *but only if something is going to close it*, and this is that
  something.
  *Done when:* `:theme <slug>` in the running binary draws the next frame in the new theme with
  no restart, and a pty test proves it. *Needs:* T012, T026

- [ ] **T093 · Floats from the doors** 📌
  `open-float`, `close-float` and `close-all-floats` are declared and unapplied, so **Steel and
  MCP cannot open or close a float.** The slot exists — `FloatSlot::empty()` at
  `crates/phosphor/src/main.rs:924`, and the boot report opens one — and `esc` closes a float
  through the input machine rather than through this verb. So there is a live surface with no
  door to it. This is load-bearing for more than convenience: Design Language §9's one-float rule
  and [Q9](IMPLEMENTATION-PLAN.md#q9)'s ask queue are both *policy about which float is up*, and
  policy belongs in the editor layer — which needs these three verbs to hold any.
  *Done when:* a Steel call opens a float, a second replaces rather than stacks it,
  `close-all-floats` clears the slot, and `phosphor --eval` and the REPL agree on all three.
  *Needs:* T084, T026

- [ ] **T094 · Reloading the editor layer** 📌
  `load-runtime-file` and `reload-runtime` are declared and unapplied: **the layer cannot be
  reloaded without restarting the editor.** `init.scm` reads the load order once at startup and
  the REPL evaluates forms; neither of those is this. It matters more than the other five,
  because invariant 1 is *"the editor layer is Steel in `runtime/*.scm`, **redefinable at
  runtime**"* and `CP-2` is the checkpoint that asks whether that is true. A layer you restart to
  reload is a config file with a longer reload cycle.
  *Done when:* editing a `runtime/*.scm` file and calling `reload-runtime` takes effect on the
  next frame with no restart; a broken file leaves the previous layer standing and reports the
  error the way a broken `init.scm` already does; a pty test covers both. *Needs:* T021, T026

- [ ] **T095 · History maintenance — compaction and checkpoints** 📌
  Two declared, unapplied verbs over machinery that is already built and already proven.
  **`compact-history`:** `journal.rs` implements compaction and proves it under a real `SIGKILL`,
  and nothing triggers it — so a history only grows, and the first person to keep a long session
  is the one who finds out. **`undo-to-checkpoint`:** `UndoTree::goto` and `CheckpointId` both
  exist and `struct Timeline` (`crates/phosphor/src/main.rs:1481`) owns the tree; nothing routes
  a checkpoint id to it. The second is what makes *an agent turn* a unit of undo, which is the
  shape `T073`'s jj timeline reads.
  *Done when:* a journal compacts on a policy the editor layer names rather than on nothing, a
  checkpoint id round-trips through `undo-to-checkpoint` back to that state, and both survive a
  restart. *Needs:* T030

- [ ] **T096 · `set-soft-wrap` — the verb** 📌
  The narrowest of the six and the clearest statement of the shape. **Soft wrap works.** `T081`
  built it, `--soft-wrap` turns it on, and `host.flag("soft-wrap")` at
  `crates/phosphor/src/main.rs:891` reads what `init.scm`'s `(set-option! …)` set. What does not
  work is the verb: `set-soft-wrap` is declared, generated into all three doors, and never
  applied — so it cannot be toggled at runtime from Steel, MCP or the CLI. A capability that the
  doors advertise and that does nothing is worse than one that is absent.
  *Done when:* `set-soft-wrap` toggles wrapping on the next frame from each of the three doors,
  and the flag and the verb read one piece of state rather than two. *Needs:* T081, T026

- [x] **T097 · The `open-help` arm in the host** 📌
  `open-help` is declared at `crates/phosphor-core/src/action.rs:1075` and
  `runtime/keymaps.scm:959` binds `:h[elp]` to it — and **`OpenHelp` has no arm in
  `crates/phosphor/src/main.rs`**, whose `ViewAction` arms are `Scroll`, `SetFold`, `FoldAll` and
  `UnfoldAll` and nothing else. Typing `:help agent-objects` in the running binary today produces
  a refusal rather than screen `6d`.
  It is a task of its own rather than a line on `T086` because it is a different file and
  therefore a different owner: `T086` draws the grid and is `surface`'s; the arm is in the binary
  and is `spine`'s. `T086` cannot pass without it, which is why it sits on `T086`'s *Needs:*.
  *Done when:* `:help` in the running binary opens the float, and a pty test types
  `:help agent-objects` and reads the grid off the frame. *Needs:* T084, T026

  > **Met.** `crates/phosphor/tests/loop_pty.rs` drives the real binary:
  > `help_opens_the_grid_and_closes_on_q`, `help_narrows_to_the_agent_objects_topic`, and
  > `a_repl_rebind_shows_up_in_the_help_grid` — the last one types a rebind at the REPL and reads
  > it back out of the grid, which is what makes "from the live keymap" a claim rather than a
  > hope. `:help <topic>` narrows three ways, all asked of the live table: a scope name, a role
  > family, else a substring.
  >
  > **One limit, owed to `T086` rather than here:** a `Density::Help` body is clamped to the
  > float's height and nothing scrolls one, so `:help normal` has more rows than it can show and
  > stops. The bare `:help` draws an index for that reason.

- [ ] **T098 · Honest refusals for the deliberately-deferred vim keys** 📌
  `q` `@` `m` `/` `?` `n` `N` are unbound in `runtime/keymaps.scm` — macros, marks and search are
  all deferred on purpose — so pressing one draws `T035`'s unknown-key hint the first time and
  **nothing at all** every time after. To a vim user's hands `q` does not read as *deferred*, it
  reads as *broken*, and `CP-3` asks exactly that question: where does muscle memory break.
  **Teej's ruling, 2026-08-13: `T035`'s once-per-session design is unchanged.** The hint is
  right — teaching `SPC` and `:help` twice is nagging, and the spent hint is not the defect. The
  defect is that these keys are *unknown* when they should be **known and not built**. Bind each
  to a refusal that names what it will be and which task builds it, the way the ex line already
  declines the Claude and Search prompts by naming `T058`.
  *Done when:* pressing `q` in the running binary answers with a refusal naming its task, the
  once-per-session hint still fires exactly once on a key that is genuinely unknown, and a pty
  test covers both halves. *Needs:* T033, T035

  > **Repair window — partial, not ticked, and the missing third is now buildable.** Two of the
  > three clauses are met against the shipping loop, proven in `crates/phosphor/tests/loop_pty.rs`
  > rather than at a widget: `a_deferred_key_names_the_task_that_builds_it` presses `/` and reads
  > *"T058 builds the message and search prompts"* off the frame, then presses `n` and reads
  > *"not built yet — T049 builds it"* — the task coming off the capability's own row rather than
  > being written anywhere. `a_deferred_key_does_not_spend_the_session_hint` presses `q` and
  > `Q` in one session and asserts the first is not called unknown while the second still spends
  > `8e`'s one teaching row.
  >
  > **What is not met is `q` itself, and the reason is the interesting part.**
  > `runtime/keymaps.scm` binds `q`, `@`, `m`, `'` and `` ` `` to `key/deferred` — known, silent,
  > and naming no task — and says why in its own words: *"a key that is deferred and has **no
  > capability to name** … binding it to the nearest-looking verb would put a keystroke in front
  > of a capability that means something else, which is worse than silence: the refusal would name
  > the wrong task."* That was correct when it was written. It stopped being correct in the repair
  > window below, which added `set-macro-recording` and `register` (`T099`) and `place-anchor`
  > (`T042`) — so `q`, `@` and `m` now each have a capability that means exactly what the key
  > means, and the refusal they would raise would name the right task. Closing this clause is a
  > line on `T099` and on `T042`, listed there, not new work here.

---

## B · The repair window between `CP-3` and `S4`

`CP-3` passed on 2026-08-13 with no findings and `S4` did not start that day. A window ran between
them, entirely on debt this build had already written down — the queued items in
[OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s repair-pass list, which now carries a per-item status
line saying which of them ran.

**Why it ran before `S4` rather than after, and it is one item.** `S4` builds `Node::Completion`
and `Node::Signature` — two more node kinds that the interpreter will draw and that nothing
composes — and the lint which catches a kind drawn by the interpreter and composed by nobody had
to exist *before* the window that would otherwise repeat the shape. It is the sibling of
`scripts/lint-action-arms.sh` one layer over: that one watches mutations a ticked task declares
and the binary never applies, this one watches node kinds. The gap it closes is not hypothetical
— `Node::KeyHints` at `Density::Help` was composed by nothing while a golden-frame test
hand-built a tree and matched it, which is the same defect `T016` taught and `T097` paid for.

**Two tasks came out of it, and both exist for the same reason.** The vocabulary gained three
capabilities in this window — `set-macro-recording`, `register` and `place-anchor` — because
`T098` and `runtime/keymaps.scm` had both stopped at the same wall: a key that should decline by
naming its task cannot, if the vocabulary has no verb that means what the key means. Adding the
verb is `spine`'s and cheap; building the machine behind it is a task, and these are those tasks.
They are numbered `T099`+ and append rather than renumber, like everything else here.

- [ ] **T099 · Macros — `q` and `@`, over `feed-keys`** 📌
  Ruled 2026-08-12 and recorded in `runtime/keymaps.scm`: **macros are the editor layer's, over
  `input/feed-keys`** — recording is capturing keystrokes into a register and playing is feeding
  them back, so the machinery is `T026`'s `record`/`record_changed` stream that `.` already keeps,
  generalised to a named register. Two things were missing and neither was a keymap's to invent,
  and this window added both: `set-macro-recording` (`crates/phosphor-core/src/action.rs`, S3,
  `Deny` at the MCP door) starts and stops the capture, and the `register` query
  (`crates/phosphor-core/src/query.rs`) answers what one holds so `@` can feed it back.
  **Both are declared and neither is applied.** `Machine::apply`'s
  `InputAction::SetMacroRecording` arm is deliberately a no-op and says so in a comment naming
  this task. That is *not* on `scripts/lint-action-arms.sh`'s RECORDED table, and the difference
  from `T092`–`T096` is worth understanding rather than glossing: that lint fires on a mutation
  declared by a **ticked** task, and `set-macro-recording`'s capability row cites `T099`, which is
  this one and is not ticked. So there is no gap to record — and the moment this task is ticked
  without an arm behind it, the lint fails. Citing the unbuilt task on the row is what makes the
  debt self-enforcing instead of needing an entry someone has to remember to delete.
  Closes `T098`'s third clause for `q` and `@`: once the verb has an arm the layer binds them to
  it, and the refusal names `T099` instead of being silent.
  *Done when:* `q<reg>` records, `@<reg>` replays through `feed-keys`, the `register` query reads
  the same register back through all three doors, and a pty test records a keystroke sequence and
  replays it in the running binary. *Needs:* T026, T033

- [ ] **T100 · The door speaks §6's voice** 📌
  The two halves of one defect, ruled in this window to be one task because they rewrite the same
  expectation set and doing them separately means regenerating and reviewing it twice. Recorded
  at [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §7 and §9:
  **(1)** there is no `Outcome` case for *"it ran and raised"*, so a refused query surfaces the
  `Error: Kind:` envelope, which is Steel's and not Design Language §6's voice; and
  **(2)** `door.rs::why` and `answer::why` phrase one enum two ways — *"T041 builds this"* against
  *"not built yet — T041 builds it"*.
  > **Scope**
  > - Files: `crates/phosphor/src/door.rs`, `crates/phosphor-steel/src/answer.rs`,
  >   `crates/phosphor/tests/parity.rs`
  > - Named units: 1 enum (`Outcome`), 2 `why` implementations, the parity expectation set
  > - Verification: the existing parity suite, regenerated
  > - Risk: public API yes (one `Outcome` case) · data migration no · cross-module yes
  >   (`phosphor`, `phosphor-steel`) · reversible yes · external blocker no
  *Done when:* a query that ran and raised is a distinct `Outcome` from one that was refused, the
  two `why` implementations produce one sentence per enum value, and every door check passes
  against the regenerated expectations. *Needs:* T020, T024

**Two verbs were re-homed rather than added**, which is worth recording because a wrong phase on a
capability row is what put them on `lint-action-arms.sh`'s creditor list in the first place:
`apply-edits` moved from `S3 / T029` to `S6 / T052` and `jump` from `S3 / T026` to `S5 / T042`,
matching the tasks [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §18 ruling had already named as their
creditors. `place-anchor` is the third and it is genuinely new: `goto-anchor` named an `AnchorId`
that nothing produced and there was no setter at all, which is why `m` is bound to silence.

**Two debts still have no creditor, and both lints say so out loud rather than passing quietly.**
Neither gets a task here, because inventing one would be inventing product work rather than
recording debt — but a reader looking for what this window did *not* close should find them named:

- `scripts/lint-action-arms.sh` reports *13 recorded gaps (1 with no task that closes them)*, and
  the one is `ApplyEdits`. Its capability row now says `S6 / T052`, so the creditor exists; the
  RECORDED table's own blocking-task field is what is still empty, and that is a `scripts/` edit.
- `scripts/lint-node-kinds.sh` reports *30 node kinds, 16 composed by the shipped configuration,
  14 recorded gaps (1 with no task that closes them)*, and the one is `Node::Gutter` — the state
  column **without** an editor around it, for a surface that wants it. `T031` built the column and
  `BufferView` ships it as its left column; no task in the graph names a surface that would
  compose the standalone kind. This is the same root as R3 in
  [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md), which asks how a terminal capability should reach that
  kind's arm — a question about composing something nothing composes.

That second lint is also why the window ran before `S4` rather than after: `Completion` and
`Signature` are already in its recorded gaps, against `T038` and `T039`. So the day `S4` builds
those two widgets and composes neither, the lint is what says so — instead of a golden frame
passing on a hand-built tree while the running binary draws nothing, which is the shape this
build has now repeated twice.

---

## Notes on running this

**Checkpoints are stop-the-line.** The value is entirely in not proceeding past a failure.
CP-0, CP-3, CP-5 and CP-7 are the four where stopping is most likely to be correct and most
likely to feel inconvenient.

**Findings during manual verification are worth more than the pass/fail.** The "does it feel
right" prompts exist to surface things a test would never catch — write them down when they
occur, because they will not recur to you later.

**Coverage accumulates; the Tier-3 residue does not.** By CP-9, Tier 1 covers all 34 v1 screens
as text snapshots and Tier 2 regenerates them all as images in one pass. Both get cheaper every
checkpoint. What never shrinks is the Tier-3 list — tearing, kitty chords, OSC 8 activation,
latency, tmux passthrough, and every "is this actually good" judgement. Roughly a dozen items,
constant from CP-1 to CP-9. Plan for them rather than hoping the harness absorbs them.

**Don't let a clean recording substitute for the residue.** The failure mode of a good harness
is trusting it past its edges — most sharply at CP-6, where a smooth-looking streaming clip says
nothing at all about whether frames tore on real hardware.
