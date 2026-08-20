# Phosphor — task breakdown

Decomposed from [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md), which is itself derived from
the four design docs in [design/](design/). The plan says *what each phase is for*; this file
says *what to build, in what order, and where we stop and look at it*.

**108 tasks + 9 harness tasks · 12 checkpoints · 9 phases**, covering all 34 screens v1 builds.
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

**Window C is built and its mechanical half is green.** The `Action` vocabulary is 218
capabilities generated from one table, the three doors are total functions over it, and the
parity test walks all 654 door checks end to end. (`208`/`624` until `S3` added
`Buffer::SetCase`, `209`/`627` until the repair window added `set-macro-recording`, `register`
and `place-anchor`, `212`/`636` until `S4` added the three `ingest-` verbs the asynchronous
LSP transport needs, `215`/`645` until `T104` added `insert-indent`, and `216`/`648` until
`T093` added `define-float-surface` — the registry `open-float` had always named and nothing
had ever created (§43); the count is
`scripts/lint-one-registry.sh`'s, which reads the tables in
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

**`S4` is built, and `CP-4`'s manual half has not run.** Four of its six tasks are ticked below —
`T036`, `T037`, `T038`, `T039` — and **`T040` and `T082` are deliberately not**, each for a reason
written at the task rather than left to be inferred. The mechanical half is green at **983 tests
and 17 lints**. What makes this window different from `S3`'s is the last phase: a wiring agent ran
after every builder, and **twelve of the `Lsp` domain's fourteen capabilities are named by the
binary** — plus `define-language` and `toggle-comment`, which are not in that domain — where every
one of them was declared and dead when the builders finished. That is rule 2 of *Concurrency* in
[TEAM.md](TEAM.md) being obeyed rather than restated. Three capabilities are still unreached and
each is accounted for at
its task: `apply-workspace-edit` is RECORDED against `T060`, `request-references` was re-homed to
`T047`, and `align-columns` has no creditor at all, which is why `T082` stays unticked.
**No `CP-4` verdict is recorded here, because the manual half is Teej's and has not happened.**

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

**`T101` and `T102` were added by the repair window between `CP-4` and Window E**, in a third such
section, *C · The repair window*. `T102` is a vendored-fork change and those are permanent, so it
needs a task on principle. `T101` needs one for a different reason and it is the first of its
kind here: it is **a ruling that overrides a mockup**. `6b` draws a bare `(keymap-set! …)`
answering *"persisted to init.scm"* and the build no longer does that, so the task is where a
reader finds out why the tree and the drawing disagree — the flag `CLAUDE.md` asks for, rather
than an edit to `docs/design/` nobody sanctioned.

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
| **C · The repair window** | **T101–T103** | *between `CP-4` and Window E — a ruling that overrides a mockup, a fork fix, the fork tests nothing ran, and a second CLI dispatcher* |
| **D · `CP-4`'s manual half** | **T104–T108** | *what Teej found by typing into the shipping binary; see the section at the end* |

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

  > **`T041` landed and it is still not met — the blocker was one layer under the one recorded
  > above.** Three of the plan's lines are live now (`declare-regions!` answers `6`, the two
  > `mark-seen!` lines answer), and the fixture still holds nothing, because **each line is its
  > own `phosphor --eval` process**. The regions declared on line 9 are gone before line 16 runs,
  > so it marks two spans in an empty store. Nothing about that is fixable by building more
  > capabilities: what makes a seeded fixture possible is **persistence**, which is `T044` for
  > seen-state and is owed for regions on the same terms. Recorded in full at `T041`, and
  > `scripts/seed-fixtures.sh` now says it in its own summary — a seeding mechanism that reports
  > three landed capabilities and seeds nothing is exactly what `CP-5`'s tapes would be built on.
  > Two bugs in that script were found by being the first task to actually run it; both are at
  > `T041` too.

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
  >
  > **Pre-`S4` scout — item (1) is closed. `main` is protected.** The same `gh api` call now
  > answers a protection object with six required status checks, which are the six blocking CI
  > jobs by name: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run`,
  > `just lint (structural lints)`, `cargo deny check`, `cargo hack (feature combinations)`.
  > `Tier 2 — VHS pixel diff (non-blocking)` is **deliberately not among them**, which is this
  > task's *done when* stated as configuration rather than as intent. Force-pushes and branch
  > deletion are refused.
  >
  > `enforce_admins` is **false**, and that is the one judgement in it rather than a
  > transcription of the criterion. Teej pushes `master:main` directly and this build has no
  > PR flow; enforcing against admins would have made the next push fail rather than made
  > anything safer, since required checks cannot have run on a commit that does not exist yet.
  > So the protection is real for a PR and for anyone who is not an admin, and the owner's own
  > push still works. Turning it on is one field — `gh api -X PUT
  > repos/TrevorS/phosphor/branches/main/protection/enforce_admins` — the day there is a flow
  > that wants it.
  >
  > **Still outstanding: item (2).** The `tapes-diff` job has still never executed, so there is
  > still no Tier-2 signal to point at, and that is what keeps this unticked. A criterion about
  > which tier blocks cannot be met by the half that blocks alone.

- [ ] **V009 · Degradation tapes**
  `Env TERM xterm-256color` and `Env NO_COLOR 1` variants of the core screens, exercising the
  fallback paths (`▎` markers, underline instead of undercurl, static `✻`).
  *Done when:* the degradation path is captured for `1a` and `2a` without touching a real
  terminal. *Needs:* V005

  > **Both blockers gone, `2a` captured, and it found the thing it was for
  > (2026-08-19).** The outstanding note below said `2a` could not be captured
  > because `T046` was unbuilt, and that none of the three fallback paths
  > rendered on `1a` because the `▎` marker needs regions from `T041`. Both are
  > ticked, and `tapes/seed-state.sh` now puts a seeded store in front of a
  > capture — so `tapes/2a-degraded-nocolor.tape` is the first screen in this
  > repository where a marker has ever had anything to degrade *from*.
  >
  > **~~The `▎` fallback is unreachable.~~ Fixed 2026-08-19**, and the capture
  > above is what found it. `phosphor_term::colour_available` answers the
  > question `crossterm` answers silently — `NO_COLOR` set and non-empty, the
  > same rule at <https://no-color.org> and in `Colored::ansi_color_disabled`,
  > matched deliberately so the editor degrades on exactly the condition that
  > drops the escapes. `BufferView::fill` is the builder that was missing, the
  > binary chooses, and `phosphor-ui` still reads no environment.
  > `the_degraded_state_bar_carries_its_hue_in_a_glyph` asserts the pair rather
  > than the glyph: a `▎` in the ground colour would be as invisible as the
  > block it replaced. Re-recorded, and the markers are on the screen.
  >
  > The finding as it stood: §8 says markers become `▎` when colour
  > is gone; `gutter::state_cell` implements it as `Fill::Marker` and `gutter`'s
  > own unit tests cover it. But `grep -rn "Fill::" crates/` outside that module
  > returns exactly one line — `buffer_view.rs:564` — and it hardcodes
  > `Fill::Block`. Nothing in the shipping editor can select the marker, so
  > under `NO_COLOR` the block simply loses its background and the state column
  > says nothing at all. That is `T016`'s shape one layer over: a path that
  > exists, is tested, and no configuration reaches. It is a **product** finding
  > for `CP-5`, whose failure condition is that the markers do not change how
  > you read the file — on that terminal there are none to read. Recorded rather
  > than fixed: which capability selects the fill, and where detection lives, is
  > a design question.
  >
  > **No `2a-degraded-term` capture, deliberately.** It recorded byte-identical
  > to `2a.png` (same sha256), exactly as `1a-degraded-term.png` is identical to
  > its sibling — `tapes/artifacts/DUPLICATES.md` already records
  > `TERM=xterm-256color`, *"confirmed no visual effect"*. Committing a second
  > one would add a reference image that proves the wrong screen, which
  > `scripts/lint-repo-hygiene.sh` calls a correctness bug rather than waste.
  > The inertness is now confirmed on a store-backed screen too.

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

- [x] **T080 · The `spans` escape hatch**
  One primitive taking styled rows from Steel, for surfaces the primitive set doesn't cover.
  Deliberately the *only* way to draw something custom without a Rust change — one grep-able
  name to check when a frame-budget regression shows up.
  *Done when:* `:arch` (T048) is built entirely from it, with no primitive of its own.
  *Needs:* T079

  > **Built at S2, deliberately not ticked.** The primitive exists, is reachable from Steel,
  > and `scripts/lint-one-escape-hatch.sh` proves it is the *only* custom-draw path — verified
  > on two planted violations. What cannot be met until S5 is the criterion as written: `:arch`
  > is `T048`, and there is no store to query. `T048` ticks this.

  > **`T048` landed and ticked it.** `runtime/arch.scm` draws `6a` — four producers, the store
  > box, two callers, a live count in the middle — and every row of it is `view/spans`. The
  > acceptance was never *"the hatch works"*, which S2 already knew; it was *"a real custom
  > surface needs no primitive of its own"*, and the way that is now checkable is that a Rust
  > primitive for `6a` would have to appear in `phosphor-ui` to be drawn at all, where the lint
  > would find it.

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
  > **↑ That paragraph is spent, and it was still telling readers to grep for its own
  > evidence.** `T097` built the arm and is ticked; `grep -n OpenHelp crates/phosphor/src/main.rs`
  > answers `2022` today. Struck rather than deleted because the instruction it carries — *grep
  > it* — is exactly what a reader would have done, and they would have found the opposite of
  > what it promised. **What `T097` did not settle is the rest of the criterion:** the *done
  > when* is that `6d` reproduces from a keystroke and that a REPL rebind shows up in it, and
  > the two items below are what still stands between the arm and that.
  >
  > **Second, smaller, outstanding item.** The page composes from the **first** `Select` and
  > the **first** `Operator` bound in normal scope — `crates/phosphor/tests/screen_6d.rs:129-132`
  > — which is `v` and `d`, so it has no slot for a third head and cannot draw `gsib` however
  > the keymap is written. One line in that file makes `MarkSeen` a head.
  >
  > **Third: the snapshot's own prose is now false in three places, and no lint sees it.**
  > `crates/phosphor/tests/screen_6d.rs:213-218` (baked into
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

- [x] **T036 · LSP client state**
  In `phosphor-buffer`. Blessed server auto-configured per first-class language, not merely
  discovered.
  *Done when:* rust-analyzer attaches and reports ready. *Needs:* T015

  > **Ticked by the `S4` wiring pass, on two proofs.**
  > `crates/phosphor-buffer/tests/lsp_rust_analyzer.rs::rust_analyzer_attaches_and_reports_ready`
  > is the criterion verbatim, and it ran against a real rust-analyzer rather than skipping —
  > **re-run alone with `--no-capture` by the docs pass, which is the only way to tell the two
  > apart**: the test probes for a usable server and returns without asserting when there is
  > none, so `nextest` prints `PASS` either way and a suite summary is not evidence. What it
  > printed was `T036 acceptance — against rust-analyzer 1.97.1 (8bab26f4 2026-07-14)`. On a
  > machine with no rust-analyzer — CI is one — this criterion is proved by nothing, and the
  > file's own header argues that trade at length.
  > The half that test cannot see is *reachability*: the client is now started by
  > `crates/phosphor/src/main.rs`, its `Post` is `crate::lsp::sink` into the one event queue,
  > and the server a buffer gets comes from that language's `define-language` declaration
  > (`lsp::attach`) with no Rust table in the path.
  >
  > **Two of its capabilities moved rather than being armed, and both are recorded.**
  > `request-references` was **re-homed to `S5`/`T047`** on the `apply-edits` precedent:
  > `LanguageServers::ask` answers a `Vec<FileSpan>` and nothing in the vocabulary carries a
  > list of places, so the attribution was the bug — `T047` builds the symbol source a list is
  > drawn in. `apply-workspace-edit` stayed here and is recorded in
  > `scripts/lint-action-arms.sh` against `T060`: it is the one `Lsp` verb rated `Ask`, and
  > there is no ask queue to put the question in.
  >
  > `request-definition` **did** get an arm, because a single target is an `open-file` and the
  > host is the only thing that can name a `PaneRef`. It uncovered a second gap on the way:
  > `open-file`'s `at` was recorded and dropped, because every caller until now was `:edit`.
  > `crates/phosphor/tests/loop_pty.rs::gd_opens_the_file_the_server_named_at_the_line_it_named`
  > presses `gd` against a real server process and asserts the line the cursor landed on.
  >
  > **Four things the `CP-4` review found, all fixed here.** Each was a wiring defect rather than
  > a client one, and each is now pressed:
  >
  > 1. **`gd` discarded unsaved work.** The open-file arm re-read the target from disk with no
  >    dirty guard and no same-file check, and *`gd` into the file you are already editing* is
  >    the common case. Both arms are in the loop now, and both are driven:
  >    `a_jump_out_of_a_dirty_buffer_refuses_rather_than_discarding_it` (which raises
  >    `WouldLoseWork`, the refusal `close-buffer` and `quit` already had) and
  >    `a_jump_inside_the_open_file_moves_the_cursor_and_keeps_the_edits`.
  > 2. **`initialize` sent `rootUri: null`**, and typescript-language-server 5.3.0 refuses to
  >    initialize without it — so `typescript` and `javascript`, two of the twelve, had no working
  >    server in the shipped configuration. Isolated against the real binary at `CP-4`: identical
  >    params with `rootUri` set initialize fine. `lsp.rs::initialize_params` sends the deprecated
  >    field on purpose and says why.
  > 3. **A failed server was completely silent.** `ServerState::Crashed`, `Failure` and
  >    `ServerIdentity` were read by nothing in the binary — the one call site was the insert
  >    trigger's `is_ready()` — so *"no such file or directory"* reached nobody. `7c`'s
  >    `rust-analyzer ✓` is a `StatusVm` field now, composed by `runtime/statusline.scm`, and
  >    `a_server_that_cannot_start_says_so_on_the_statusline` watches it change **with no key
  >    pressed**.
  > 4. **Nothing woke the loop for it.** That last test needs `events::AppEvent::Woke`, the
  >    variant the queue's own header reserved for *"a producer to post it and a surface that
  >    shows the difference"*. A server state change is the first of each. It is not an Action and
  >    could not be one.
  >
  > **What is built here and still unreached, listed rather than left to be discovered.** None is
  > a ticked-task violation and each has a creditor: `Question::References` and
  > `file_edits_from_lsp` belong to `T047` and `T060` (above); `LanguageServers::stop` has no
  > capability to be called from — `restart-language-server` is the only server verb the
  > vocabulary declares, and *stop* with no way to start again would be a key that breaks the
  > editor until it is restarted; and `ServerSpec::with_initialization_options` has **no caller
  > and no way to acquire one**, because `LanguageSpec` has no field for `initializationOptions`
  > — a declaration can name a server's command and nothing else about it. That is the next thing
  > a per-server fix will want, and it is a `define-language` change rather than a client one.
  >
  > **The `rootUri` fix could not be verified end-to-end on the machine that made it.** This
  > machine has typescript-language-server 5.3.0 but no TypeScript installation it will bind to:
  > the handshake fails identically with `rootUri` set and with it null (*"Could not find a valid
  > TypeScript installation"*), so what is proved here is that the field is sent — the unit test
  > asserts it against the folder's own URI — and not that the server then serves. The review's
  > isolation, in a directory with `typescript@5.7` installed, is what says the two differ.

- [x] **T037 · `define-language` + the 12 declarations**
  TS, JS, Rust, Python, Steel, Markdown, JSON, CSV, TOML, YAML, HTML, CSS — each shipping as a
  `define-language` call in `runtime/`, **not a Rust table**. Binds grammar + LSP command +
  locale hooks.
  *Done when:* a 13th language can be added from the REPL with no Rust change. *Needs:* T036,
  T022, T083

  > **Ticked by the `S4` wiring pass.** The criterion is proved three times, at three depths,
  > and the third is the one that could not exist before this window:
  > `crates/phosphor-steel/tests/shipped_languages.rs::a_thirteenth_language_needs_no_rust`
  > (against a recorder that crate defines itself),
  > `crates/phosphor/src/main.rs`'s `a_thirteenth_language_declared_at_the_repl_claims_its_extension`
  > (against the **shipping** `AppHost` arm, which the first cannot see), and every test under
  > `crates/phosphor/tests/loop_pty.rs`'s `S4` section, each of which declares a thirteenth
  > language with its own server command and comment prefix and then drives it from a keystroke
  > on a real terminal.
  >
  > **The Rust extension table is gone**, which is the half the criterion implies and does not
  > say. `main.rs::language_of` was a `match` over ten extensions deciding which grammar a file
  > opens with; `grammar_of` reads the declarations instead, and
  > `the_grammar_a_file_opens_with_comes_from_the_declarations` fails if
  > `runtime/languages/rust.scm` is deleted. What the binary *can* load is now answered by
  > `phosphor_buffer::grammar::BUNDLED`, which `crates/phosphor-buffer/tests/grammars.rs`
  > recomputes against the fork itself — so `Languages::tier` cannot drift from the manifest.
  >
  > `toggle-comment` is armed and pressed:
  > `loop_pty.rs::gc_comments_with_the_prefix_the_declaration_named` types `gcgc` in a language
  > whose declaration says `;`, which no Rust comment table could answer.
  >
  > **The criterion held only across a restart, and the `CP-4` review caught it.** The loop read
  > the language table **once, at boot**, and the comment above that line claimed a thirteenth
  > declared at `:repl` was *"a fact about the next file opened"* — but `:e` **is** the next file
  > opened, and it read the snapshot. Measured: declare `zz` at the REPL, `:e sample.zz`, `gcgc`,
  > and the statusline answered *"this language declares no line comment"*; restart the binary on
  > the same layer and the same keys comment it. Every test that ticked this task called
  > `AppHost::languages` freshly and was structurally incapable of seeing it.
  >
  > Fixed by reading the table where it is used rather than at boot (`main::adopt`'s header says
  > what that costs), and the test that could not have passed before is
  > `loop_pty.rs::a_language_declared_at_the_repl_is_live_in_the_same_session` — the review's own
  > reproduction, from a keystroke, in one session.

- [ ] **T082 · CSV without tree-sitter** *(spike finding)*
  `tree-sitter-csv` is 2.5 years stale with ~5k downloads, and CSV gets a hand-tuned surface
  (virtual column alignment) rather than generic buffer treatment. A small parser is more
  reliable than a stale grammar **and** yields exactly the column model that surface needs.
  *Done when:* CSV column alignment works, with no `tree-sitter-csv` dependency. *Needs:* T037

  > **Deliberately NOT ticked by the `S4` wiring pass.** The half with no dependency is done —
  > `phosphor_buffer::csv` parses, `phosphor_ui::csv` measures columns and emits `Run`s for the
  > `spans` hatch, both tested against a real `Buffer`. The half that says *"works"* has no arm
  > and cannot be given an honest one today: `align-columns` is virtual alignment **inside a
  > buffer line**, and the vendored fork's virtual text is a row of its own —
  > `VisualRow::Virtual`, inserted *under* its anchor (`VENDOR.md` patch 8). Inline virtual
  > text at a column is a fork patch nobody has written, and `phosphor_ui::csv`'s own header
  > says so.
  >
  > **The capability is not re-homed**, and that is the point of leaving this untethered: no
  > task in the graph builds that patch, so a re-homing would have to name a creditor that does
  > not exist. `align-columns` stays on `T082`, `T082` stays unticked, and
  > `scripts/lint-action-arms.sh` therefore needs no entry for it — the debt is the untick.
  >
  > **The parser also arrived with its own two measurements, which is what the spike bought.** The
  > argument for dropping `tree-sitter-csv` was that a parser we own is more reliable than a stale
  > grammar; the evidence is that it is fuzzed and benchmarked, neither of which a vendored grammar
  > would have been. `fuzz/fuzz_targets/csv_parse.rs` is the fifth fuzz target and
  > `crates/phosphor-buffer/benches/csv.rs` the sixth benchmark — the one that asserts the
  > `MAX_COLUMN_CELLS` cap makes row layout bounded rather than a function of the worst field in
  > the file. Both landed in `0c12f68`, and both **immediately made `SPIKES.md`'s tooling table
  > wrong** in two places, one day after that table was audited. That is
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §30's first instance and it came from here.

- [x] **T038 · Completion via the passive Float**
  Border `#2a3c2e`, **no footer** — the one documented exception to the float contract. The
  primitive itself is `T084`; this adds the third mood and the footer exception to it.
  *Done when:* screen `7c`'s completion reproduces **from a keystroke** — typing in insert mode
  in the running binary raises the float. *Needs:* T036, T084

  > **Ticked by the `S4` wiring pass**, on
  > `crates/phosphor/tests/loop_pty.rs::typing_in_insert_mode_raises_the_completion_float`,
  > which is the criterion word for word: one character is typed into a buffer on a real
  > terminal and the float appears, with no completion key pressed. The loop asks on every
  > insert-mode edit against a server that is ready — gated on *ready* so a language with no
  > server does not raise a refusal per keystroke.
  >
  > `Node::Completion` is composed by `main.rs::passive_float` and its entry in
  > `scripts/lint-node-kinds.sh`'s RECORDED table is gone. The keys that drive it are in
  > `runtime/keymaps.scm` — `<C-x>` asks, `<C-n>`/`<C-p>` move, `<C-y>` accepts, `<C-e>`
  > dismisses — and each is pressed by a test.
  >
  > **`CP-4` found the list unusable against a real server, and that half is fixed here.** Nothing
  > filtered the answer by the typed prefix and nothing applied `sortText`, so one `.` against
  > rust-analyzer drew a float over rows 0–28 of a 30-row terminal — the whole editor hidden,
  > `strict_mul` selected, followed by fourteen rows of raw markdown. Three changes, each tested
  > where it lives:
  >
  > * **The client filters, because the protocol says the client filters.**
  >   `phosphor_buffer::lsp::narrow` matches the typed prefix against `filterText` and orders by
  >   `sortText`, both of which `Completion` now carries; the host applies it in `answering`,
  >   where the prefix the request was made inside is known.
  >   `loop_pty.rs::the_list_is_narrowed_to_the_prefix_the_server_was_never_told_about` types the
  >   rest of a word against a fixture that answers the same three rows whatever the prefix is,
  >   and asserts the two that can no longer match are off the screen.
  > * **The float is a window, not the answer's height.** `float::MAX_ITEM_ROWS` (ten, the number
  >   `pumheight` is usually set to; vim's own default is *as much room as there is*, which is
  >   the behaviour that was measured) and `MAX_DOC_ROWS` (four; `7c` draws one line of prose, a real
  >   server sends a page of markdown source).
  > * **One request in flight, and the edits made under it are coalesced into the next.** The
  >   trigger fired once per keystroke into a single `awaiting` slot, so every superseded answer
  >   missed the routing, fell through to `deliver` and painted `lsp: denied to a producer` on the
  >   statusline **while typing** — reproduced on a pty at a 350 ms gap between keystrokes.
  >   `main::Outstanding` counts what is owed instead;
  >   `loop_pty.rs::a_burst_of_typing_never_says_the_editor_denied_something` types a burst with
  >   `<C-x>` interleaved and reads the row.
  >
  > **And `7c` has the frame the checkpoint asks for at two widths**, composed the way the binary
  > composes it: `crates/phosphor/tests/screen_7c.rs` draws the buffer, the statusline through
  > `runtime/statusline.scm` (server chip included) and the float through the interpreter's
  > `Resources`. The widget's own `7c` frame — `crates/phosphor-ui/tests/screen_7c.rs`, which
  > predates this window — is the mockup transcribed; this is the host drawing it.
  >
  > **One divergence from vim, argued where it is bound.** `<C-n>` opens *and* steps in vim,
  > because vim's keymap and vim's popup are one program; a binding here is data and cannot ask
  > whether a list is open, so `<C-x>` opens and `<C-n>` steps. **And one spelling that had to
  > be given a meaning:** `accept-completion` takes a 1-based index, so `0` — which names no
  > row — means *whichever row is selected*. Without it a keymap could only ever accept a fixed
  > row; the parameter's own description now says so.

- [x] **T039 · Signature help + hover**
  *Done when:* screen `7c` reproduces in full **from a keystroke**. *Needs:* T038

  > **Ticked by the `S4` wiring pass**, on
  > `crates/phosphor/tests/loop_pty.rs::signature_help_and_hover_reach_the_same_passive_float`:
  > `<C-s>` in insert asks what the call takes and `K` in normal asks what is under the cursor,
  > both against a real server process, and both draw through the one `Node::Signature` — whose
  > entry in `scripts/lint-node-kinds.sh`'s RECORDED table is also gone.
  >
  > **What *"in full"* was measured against, since the criterion does not say and a later reader
  > cannot tell.** `TUI Mockups.dc.html`'s `7c` **draws only the completion float** — the
  > viewport, the popup with its three rows and one line of prose, and the statusline with
  > `rust-analyzer ✓`. Signature help appears in that screen's *caption* (*"lsp completion +
  > signature help · no agent anywhere · boring on purpose"*) and nowhere in the picture. So the
  > drawing is `T038`'s and the caption is this task's, and ticking on the drawing alone would
  > have been ticking on somebody else's work. The proof used instead is the stronger one: both
  > verbs pressed against a real server process, in the running binary.
  >
  > The float is dismissed by the next keystroke, which is the only dismissal a passive float
  > can offer: §4's documented exception means it has no footer to put a key in.
  >
  > **That rule was right for hover and backwards for signature help**, and the `CP-4` review
  > measured it: `<C-s>` inside `add(` drew `fn add(left: i32, right: i32)` with `left: i32` in
  > the active tone, and typing `1` — the first character of the argument the float exists to
  > help type — cleared it. In insert mode the float now lives until the insert session that
  > raised it ends; in normal mode the next key still dismisses hover, which is the question
  > *"have you read it"* being answered. `loop_pty.rs::signature_help_survives_the_argument_being_typed_under_it`
  > presses both halves, and reads the dismissal as an **erasure** — the row under the float
  > coming back — because a frame is a diff and a float that is still up is not redrawn.

- [x] **T040 · Diagnostics → gutter + virtual text**
  Trouble priority in `GutterBar`; `■` rows via `VirtualText`; undercurl with underline
  fallback.
  *Done when:* a file with real errors shows correct gutter priority against other states.
  *Needs:* T031, T032, T036, T085

  > **Deliberately NOT ticked by the `S4` wiring pass, and the missing half is exactly the two
  > words *"other states"*.** Everything else is done and pressed: a real server publishes
  > unasked, the Action crosses the one event queue, and
  > `crates/phosphor/tests/loop_pty.rs::a_published_diagnostic_reaches_the_screen_with_nobody_asking`
  > reads the `■` row off a real terminal with no key pressed at all. The host concatenates
  > diagnostic regions with every other source and calls `gutter::state_column` **once**, which
  > is the composition `diagnostics.rs`'s header asks for.
  >
  > There is exactly **one** source. Unseen, seen, thread and failure regions are the store's
  > and the store is `T041`, so *"correct priority **against other states**"* has nothing to be
  > correct against — the ladder is unit-tested in `phosphor-ui`'s `gutter`, and the claim this
  > criterion makes is about a composition that cannot exist yet. Tick it when `T041` puts a
  > second source in that `Vec`; the line that concatenates them is already written.
  >
  > **What `CP-4`'s second sitting found, 2026-08-16 — and it is not the criterion above.** Teej
  > half-typed `path:` in `crates/phosphor/src/main.rs`, rust-analyzer answered with **eleven**
  > cascade parse errors (`expected COMMA`, `expected R_PAREN`, `expected field declaration` —
  > four of them the same sentence twice or more), and `DiagnosticsVm::rows` mapped the set
  > one-to-one. Eleven rows of a parser resynchronising pushed the code being edited off the
  > bottom of the screen. *"we have to do better with these error msgs"*.
  >
  > **Two things were wrong and only one of them was the rows.** §3 gives a diagnostic three
  > surfaces — the state bar in gutter column 1, the undercurl, and the inline row — and screen
  > `2b` draws a fourth thing beside them: the statusline count `■ 1`, next to
  > `1 thread · 2 unseen`. **Nothing had ever computed it.** Grepped this session before the fix:
  > neither `runtime/statusline.scm` nor `phosphor_steel::status` mentioned a diagnostic at all.
  > So the only place a file's error count appeared was one row per error, which is precisely why
  > eleven of them had to be drawn to say *"there are eleven"*.
  >
  > Both landed together, because either alone is worse than neither: `DiagnosticsVm::rows` takes
  > a `RowPolicy` (`cursor-line` by default, capped at three, identical sentences at one anchor
  > deduped, and the overflow **said** — a fourth row reads `■ n more here`), and `StatusVm` grew
  > `trouble`/`attention` so the ones that stay quiet are still counted. Bounding without the
  > count would have been hiding.
  >
  > **The design agrees, and `6c` is the proof rather than the assertion.** `■` appears exactly
  > twice in all 37 mockup screens — once as a single inline row, once as that count — so an
  > unbounded row was the departure and this is the return. `crates/phosphor-ui/tests/screen_6c.rs`
  > now draws under the **shipped default** and its golden frame is byte-identical: `6c`'s cursor
  > is on line 64 and line 64 is the one carrying `E0308`, so the mockup and the default are the
  > same picture. One space of divergence is left and is flagged rather than folded —
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §39.
  >
  > **None of this ticks `T040`.** The criterion is still *"against other states"* and there is
  > still one source of regions until `T041`.

  > **~~The blocker named above is gone, and this is still not ticked.~~ Ticked 2026-08-17, and
  > the proof is the thing that was missing.** `T041` landed the store and `T042`/`T087` gave the
  > column two more sources: the loop concatenates diagnostic regions with the store's unseen and
  > seen ones and calls `gutter::state_column` once, so *"other states"* finally exist to be
  > correct against. The entry above refused to tick on the grounds that a criterion which *could*
  > be met is not one that *is*, and that was right — the test is what closes it.
  >
  > `loop_pty.rs::a_diagnostic_outranks_an_unseen_region_on_the_same_row`. The toy server publishes
  > its error on buffer line 2; a declaration covers lines 2 **and** 3, so one row carries trouble
  > *and* claude-unseen while its neighbour carries claude-unseen alone. Three assertions: line 2's
  > bar is **unchanged** by the arrival of the lower state, line 3's bar **did** change (so the
  > declaration landed and the first assertion is not vacuous), and the two **differ** (so the two
  > states are distinguishable on screen at all).
  >
  > **Read as a colour, because a state bar is a one-cell background** — a glyph assertion would be
  > asserting nothing. No colour literal appears in the test: every assertion is *equal to what it
  > was* or *different from its neighbour*, which leaves §1 owning the hues and
  > `scripts/lint-no-colours.sh` unbothered. Pressed against a planted violation (`rank`'s
  > `ClaudeUnseen` raised above `Trouble`) and it fails with the two real hues off a real terminal,
  > `48;2;218;123;108` demoted to `48;2;61;220;151`.
  >
  > One incidental finding worth keeping: this session's statusline never draws the word `NORMAL`.
  > A server chip and an unseen count are enough for §11's ladder to contract the mode to `N`, so
  > the `press_until(…, "NORMAL")` that every other `close-repl!` in that file waits on hangs here.
  > The settle is the buffer's own first line instead — closing the float repaints the rows being
  > read, which is a better thing to wait for anyway.

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

> **The mechanical half, item by item, recorded 2026-08-14 — and this is not a verdict.** `CP-4`
> has two halves and only Teej can run the second. What follows is what the build can say for
> itself, written *here* because that is where the `CP-2` entry's rule puts it: **a checkpoint
> verdict is written where the checkpoint is, or it did not happen** — and the same holds for the
> half that comes before one. Three of the four *Claude verifies* items are met, one is met in
> part, and the VHS half of the checkpoint has not been produced at all.
>
> * **The gate is green** — `983` tests and `17` lints.
> * **`7c`'s snapshot: yes** — `crates/phosphor/tests/screen_7c.rs`, at 120 and 80 columns,
>   composed through the shipped statusline and the interpreter's `Resources` rather than a
>   hand-built tree, which is the distinction `CP-2`'s first gate failure was about.
> * **A 13th language from the REPL with no Rust change: yes** — `T037`, proved three ways, the
>   third of them from a keystroke in a live session.
> * **Completion and signature help against rust-analyzer, tsserver and pyright: rust-analyzer
>   only.** `crates/phosphor-buffer/tests/lsp_rust_analyzer.rs` is the one test in this repository
>   that talks to a shipped server; every other LSP test drives a fake made of `sh`. `typescript`,
>   `javascript` and `python` declare `typescript-language-server` and `pyright-langserver`
>   (`runtime/languages/*.scm`), and **nothing automated has ever attached to either**. The
>   `rootUri` defect recorded at `T036` is what that gap costs — two of the twelve shipped with a
>   server that could not initialize, and a human running the binary is what found it.
> * **~~Diagnostic gutter priority against other region states: no, and not in this window.~~ Yes,
>   as of 2026-08-17.** It was `no` for the right reason — there was one source of regions until
>   `T041`, so the criterion had nothing to be correct *against*. `T041`, `T042` and `T087` put
>   the store's regions in the same `Vec`, and
>   `loop_pty.rs::a_diagnostic_outranks_an_unseen_region_on_the_same_row` presses the ladder on a
>   row carrying both, reading the state bar as the colour it is. `T040` is ticked and its entry
>   carries the detail.
> * **~~The VHS half has not been produced.~~ It was produced, in the same commit that wrote this
>   line, and the line was false the moment it was committed.** Corrected 2026-08-16 against
>   `git ls-files tapes/`: `tapes/7c-rust.tape`, `tapes/7c-python.tape`,
>   `tapes/7c-typescript.tape`, `tapes/diagnostics.tape`, `tapes/diagnostics-undercurl.tape` and
>   `tapes/signature-help.tape` are all tracked, each with a `.png` and a `.gif` under
>   `tapes/artifacts/`, and **`git log` says every one of them arrived in `4a41700`** — the S4
>   commit this paragraph is part of. The undercurl precondition is settled the right way —
>   `tapes/README.md` records *"does undercurl survive VHS capture? — Answered: yes"* — so it does
>   not stay Tier 3.
>
>   **~~What is true, and is a different claim, is that the captures are stale.~~ Run, 2026-08-19,
>   and they were not.** `e096f88`'s message says *"Tier-2 tapes were NOT re-run and drawing
>   changed"* and §36 called the library a window or three old, so the outstanding work was
>   recorded as `just tapes-diff`. It has now been run end to end, twice: **46 of 48 frames match**.
>   The staleness was inherited prose rather than a measurement, and the two that do not match are
>   named in `CP-5`'s record below — one a blessed change, one `OPEN-QUESTIONS.md` §42.
>
>   **The run was not possible before this window, and neither was CI's.** `diff-tapes.sh` never
>   seeded the store, so the seven screens that read one were compared against an empty store; and
>   CI's Tier-2 job had never compared a pixel, dying in under a second on a missing ImageMagick
>   under `continue-on-error`. Both fixed. What CI's job reports now is §41's question rather than
>   an answer: `_config.tape` sets `Menlo`, which is a macOS system font the runner does not have,
>   so every glyph is a substitution and every frame mismatches whole.
>
>   **~~One `CP-4` VHS item is still owed~~ — captured, 2026-08-20.**
>   *"A file with real diagnostics showing gutter priority **against other region states**"* is
>   `tapes/diagnostics-regions.tape`. It declares an unseen region over lines 1–8 of `policy.rs`
>   through the CLI door while the toy server publishes an error on line 2, so **line 2 carries
>   both** and §3's ladder is observable on it: trouble-red where the two compete, claude-green on
>   the rows the region holds alone.
>
>   A second tape rather than a re-capture of `diagnostics.png`, which is both a `CP-4` artifact of
>   its own and the fixed point the undercurl pair's ink-coverage measurement reads from.
>   **`CP-4`'s VHS list is complete.**
>
>   **Worth more than the correction is how it got here**, because it is this repo's own rule
>   failing in the one place nothing lints: a claim about a directory, written in the same change
>   that filled the directory. `CLAUDE.md`'s *"state a fact about a file only if you read that file
>   in this session"* is exactly the standard it missed, and a checkpoint's own mechanical half is
>   the worst place to miss it — it is the paragraph a later reader trusts instead of looking.
>
> **The two servers nothing had ever attached to now attach, in a container (2026-08-17).**
> `docker/lsp.Dockerfile` holds rust-analyzer, `typescript-language-server` and
> `pyright-langserver` at pinned versions; `just lsp-docker` builds it and runs
> `crates/phosphor-buffer/tests/lsp_servers.rs`, which was `lsp_rust_analyzer.rs` and covered one.
> **3 passed, 0 skipped.** Deliberately outside `gate` and CI: it needs a Docker daemon, which is
> the kind of dependency that reddens a build for reasons unrelated to the code.
>
> It found four things on its first run, which is the argument for it:
>
> * **`typescript-language-server` cannot be driven by typescript 7.** The host this was written on
>   has 7.0.2 globally — the native-port rewrite, which ships `tsgo` and no `lib/tsserver.js` — so
>   the server initializes, answers *"Could not find a valid TypeScript installation"*, and lands
>   in `Crashed`. The test skips on such a host **naming the reason**; the container pins a pair
>   that works.
> * **~~A `.ts` file in a directory with no `node_modules` gets a crashed server**, for the same
>   reason: resolution walks up from the workspace and never consults the global install.~~
>   **Half of this was wrong and the container refuted it (2026-08-17).** The crash is real on the
>   host it was found on; the cause given for it is not. A test asserting it reached `Ready` in the
>   container with no `node_modules` at all, and
>   `require.resolve("typescript", { paths: ["…/typescript-language-server/lib"] })` answers
>   `/usr/lib/node_modules/typescript/lib/typescript.js` — a globally installed server finds a
>   globally installed `typescript` as its **sibling**, which is what node's resolution is for. So
>   there is one cause here, not two: typescript 7.0.2 ships no `lib/tsserver.js`, which is the
>   item above. `usable_typescript` was already asking the right question — it looks for the file
>   rather than comparing versions — and the test that asserted the wrong one is deleted, with the
>   refutation kept where it was written.
> * **`pyright-langserver --version` exits 1** — *"Connection input stream is not set"* — which is
>   the server parsing its arguments, not a failure to run. The probe read that as a broken server
>   and skipped. The test and the image's own build-time probe now tell the two apart and agree by
>   construction: an image that builds is one where the test will not skip.
> * **Only rust-analyzer sends `serverInfo`.** The other two send none, so `ServerIdentity` falls
>   back to `spec.command` — meaning `7c`'s chip draws the *command* for two of three, where its
>   own line is *"the name a server gives itself"*. Nothing on screen is wrong; the prose is
>   looser than it reads.
>
> **Completion latency, measured (2026-08-19).** `CP-4`'s first *Teej verifies*
> item — *"Is completion fast enough to be useful, or fast enough to be
> annoying? Both are findings"* — had no instrument.
> `lsp_servers.rs::completion_latency` is one, and it **prints rather than
> asserts**, on the benchmarks' rule that a figure which moves with the machine
> has no business failing a build. What it asserts is a shape: every lookup was
> answered, which is the client's own one-answer-per-lookup contract.
>
> Ten samples each, in the container, `look_up` to callback:
>
> | server | first | min | median | returned a list |
> | --- | --- | --- | --- | --- |
> | `rust-analyzer` | 8.0 ms | 1.1 ms | 1.4 ms | **0/10** |
> | `pyright-langserver` | 123.6 ms | 3.2 ms | 4.8 ms | 10/10 |
> | `typescript-language-server` | 320.6 ms | 10.3 ms | 15.2 ms | 10/10 |
>
> **The interesting column is the last one, and it is a finding on the first
> run.** rust-analyzer answers a freshly-opened project in about a millisecond
> and answers it *empty* — it has not indexed yet. To a person that is not
> "fast", it is "this editor has no completions", which is the worse half of the
> question `CP-4` asks. The same number on the host, so it is not the container.
> Reported rather than papered over: waiting for a non-empty list would have
> measured a different thing and hidden this one.
>
> The other two are honest: a third of a second cold for typescript, then
> comfortably interactive.

> **~~A defect it found and did not fix~~ — found, then fixed (2026-08-17).**
> `typescript-language-server` reached `Ready` and was gone within the second with
> `Exited("the underlying channel reached EOF")`. It was **ours**, it was one line, and the
> suspected mechanism recorded here — closing its stdin — was a red herring.
>
> **What it actually was.** `initialize_params` announces `window.workDoneProgress: true`, and that
> flag is the only thing entitling a server to send `window/workDoneProgress/create`.
> `typescript-language-server` sends it the moment it opens a project; `router`'s catch-all
> answered `METHOD_NOT_FOUND`; and its `handleResponse` turns a rejected response into an
> **uncaught exception**, so node exits and the client sees the pipe close. A capability announced
> is a request promised, and the client was announcing one it refused. Fixed by answering it —
> accepting a progress token is not a promise to draw a progress bar, it is a promise to *answer* —
> and `METHOD_NOT_FOUND` stays for methods the client never invited, which is the protocol-correct
> answer for those. Both halves are pinned by fakes in `crates/phosphor-buffer/tests/lsp.rs`
> (`a_capability_we_announced_is_a_request_we_answer`,
> `a_request_we_never_invited_is_still_refused`), so no node is required to hold the line, and
> both were run against a planted violation — the refusal bytes the planted version emits are
> character-for-character the ones that killed the server.
>
> **Why the stdin theory looked right, which is the more useful half.** The node script that
> "proved" the server survives an identical handshake never *answered* the request, and a request
> left hanging is survivable where an error answer is not. Closing that script's stdin does kill
> the server instantly — a true fact about the server with nothing to do with this crash. Two true
> observations, one wrong conclusion, and what separated them was reading the server's stderr.
>
> **Which the client was throwing away, and that is the second fix.** A server's stderr was
> `Stdio::null()` for a good reason — Design Language §8, a stack trace over the frame is a P0 —
> but *"do not draw it"* and *"do not read it"* are different decisions and only the first was
> intended. Two of the four findings above were sentences the server had already written down
> while the client reported `the underlying channel reached EOF`, which is true and says nothing.
> `LastWords` in `crates/phosphor-buffer/src/lsp.rs` pipes it, keeps a bounded tail, and quotes it
> into `Failure::Exited`, `Failure::Protocol` and `Failure::Timeout` — which gained a payload for
> that, so the rule is uniform: **every failure a live server can cause carries what that server
> said.** `Failure::Spawn` is the honest exception, there being no process to have said anything.
>
> Draining is not optional either — an unread pipe fills at 64 KiB and blocks the server on its
> next log line, so the choice was never "null or nothing", it was "null or read it".
> `a_chatty_stderr_does_not_wedge_the_server` writes twelve pipes' worth before answering
> `initialize`, and wedges for the full 20 seconds against a planted violation that holds the pipe
> without reading it. The tail's bounds are a property test over arbitrary bytes, because a client
> that quoted a server without bounding it would have traded a lost message for an allocation the
> process cannot survive — the same shape `MAX_HEADER_BYTES` exists for, one pipe over.
>
> `just lsp-docker` now reports **3 passed, 0 skipped** with no `KNOWN` line, and the `stays`
> parameter that existed only to tolerate this is gone.

> **The recurring sweep, run 2026-08-16.** Five items across three tiers; three are Tier 1/2 and
> were run, two are Tier 3 and are Teej's by definition. **The 80-column item found a defect and
> it was this window's own.**
>
> * **80 columns — run, and it failed first.** The statusline's diagnostic counters (added hours
>   earlier, `T040`'s note above) passed the *same node* for their contracted and full forms, so
>   §11's first rung was a no-op for them and they could not shed. At 17 columns the row came out
>   `" N ✻ ■11 │ ■3 │ ●"` — the two counts intact and the **unseen** counter clipped to a bare
>   glyph. That is *"drop, never squeeze"* broken by a segment that could not drop, and it took
>   `●6` — one of §11's last-standing three — with it. Fixed in `runtime/statusline.scm` (`#false`
>   contracted, so they drop on `unseen`'s rung) and pinned by
>   `compose.rs::the_diagnostic_counters_never_take_a_second_row`, which walks 24→200 and asserts
>   the counts at 80, 120 and 200. **Nothing covered it before**: every width test in the
>   repository builds its ViewModel from `screen_9c`, which takes `..StatusVm::default()` — so
>   `trouble` and `attention` were zero in all of them. A new segment with no width coverage is
>   exactly what a sweep is for.
> * **Degradation — run, and short by one for a reason that is not a defect.** Markers →
>   `▎` (`gutter::the_degraded_form_is_the_marker_in_the_same_hue`) and undercurl → underline
>   (three tests, including `phosphor-buffer`'s two against a real terminal profile) all pass.
>   **The spinner's static `✻` has no test and cannot have one yet** — `SessionState::Working` is
>   `T050`, so there is no spinner to degrade. Checked by running the suite: `test(spinner)`
>   matches **zero** tests.
> * **Nothing moves unless you asked — half reachable.** The viewport half is pressed
>   (`loop_pty.rs`, forty newlines and line 1 must be gone). The half the sweep actually
>   describes — *"the file changes underneath"* — is `T069`'s `✱`, which is `S7`. `CP-8b` is
>   where that item first has both halves.
> * **No torn frames — Teej's, and only Teej's.** The sweep's own wording: *"No recording can
>   show you this."*
> * **tmux — Teej's.** *"Passthrough is the thing being tested, so a captured tmux proves
>   little."*
>
> **No verdict is recorded, and none may be recorded here by anyone but Teej.** The manual half —
> is completion fast enough to be useful or fast enough to be annoying, the undercurl pass on two
> terminals, the REPL language, the recurring sweep — has not happened. The first of those
> questions has a build fact worth reading first: the typing trigger's throttle is **one request
> in flight**, not a timer (`main.rs`, above `editing.lookup = Some(Lookup::Completion)`), so what
> bounds the ask rate is the server's round trip and not an interval anybody chose. Whether that
> reads as *useful* or as *annoying* is precisely the judgement this checkpoint asks for, and it
> is [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §29's third item.

---

## S5 · The semantic store, seen-tracking, and the Picker

Where Phosphor stops being an editor. The highest-value checkpoint follows it.

- [x] **T041 · Store core + region state machine**
  `claude writes → unseen --s--> seen`, and `claude revises → unseen again`. Seen-state is the
  only mutable flag the user owns; everything else derives. **Your own edits never create
  regions.**
  *Done when:* the state machine is exhaustively unit-tested, including revision-after-seen, and
  **`V006`'s seeded store state is reachable through `phosphor --eval`** — regions, seen-state,
  threads and a canned transcript — so `CP-5`'s tapes produce identical output on two machines.
  *Needs:* T019

  > **Ticked on the first criterion. The second is answered rather than met, and the answer is
  > that it cannot be met by this task** — see the three findings below. Nothing is quietly
  > deferred: what closes it is named.
  >
  > **What landed.** `crates/phosphor-core/src/store/region.rs` is §7's machine — `Region`,
  > `SeenState`, `Scope`, `Lens`, `Regions` — and `store.rs` is the `Store` that owns it behind
  > one `Revision`. The arms are in two dispatchers on purpose: `Editing::act` for the keyboard,
  > `AppHost::apply` for the three doors. **Only one of them has an editor**, so `cursor` and
  > `selection` resolve on the loop's side and are refused by name on the door's — a query has no
  > cursor, and widening one to the workspace is how `s` on an empty line would mark a whole file.
  > Both apply to the *same* `Arc<store::Shared>`, which is what
  > `a_region_declared_at_the_repl_is_counted_and_a_keystroke_clears_it` presses: the region is
  > declared through the Steel door and cleared with `SPC u s`.
  >
  > **The owed arm is wired.** `set-virtual-text-visible` is `Editing::collapse`, and its RECORDED
  > row is gone from `scripts/lint-action-arms.sh`. It is **per owner without a fork patch**: the
  > host installs the row list every frame, so a collapsed owner's rows are simply not in the list
  > it installs. The fork's own toggle is one global flag and a vendored patch would have been
  > permanent. A diagnostic's row gets its owner from the region covering it, which is what
  > `phosphor_ui::diagnostics` has promised since `T040` — positional here, anchored at `T042`.
  >
  > **The fold happened, and it was not cosmetic.** `store.rs`'s header has said since `T007` that
  > `T041` folds `store::diagnostics` in. Reading it against the tree found that module had **no
  > importer at all**: `crates/phosphor/src/lsp.rs` had its own
  > `BTreeMap<PathBuf, Vec<Diagnostic>>` with its own `replace`/`of`/`answer`, written at `T040`
  > because it needed a `Mutex` and `phosphor-core` holds no locks. Two maps, one name, and the
  > documented one dead. The binary's copy is deleted; `crates/phosphor/src/store.rs` is the lock.
  >
  > **Finding 1 — `scripts/seed-fixtures.sh` has not run since `T100`, and nothing noticed.** It
  > does `out="$(… phosphor --eval …)"; code=$?` under `set -euo pipefail`. `T100` ruled *"one
  > door, one refusal, one exit code"* and made a refusal exit non-zero, so the assignment aborts
  > the script on the **first line of the plan**, before one row is printed. Its own header
  > describes a per-line transcript it could not produce. Fixed with an `if` condition (the one
  > context where `set -e` is suspended). It is deliberately outside the `scripts/lint-*.sh` glob
  > `just lint` walks, which is why a year of gates stayed green over it.
  >
  > **Finding 2 — its classifier was stale in the same way.** It matched
  > `(#refused "not built yet — …")`, the bare list `--eval` printed before the door had a voice.
  > After `T100` every line classified as BROKEN and the summary said *"plan.scm has drifted from
  > the registry"*. The plan had not; the classifier had. It now reads `T100`'s shapes and the run
  > is 15 expected refusals, 3 landed, 0 broken.
  >
  > **Finding 3 — and this is the one that governs the second criterion. `--eval` is one process
  > per line, so the plan seeds nothing.** Line 9 declares six regions and answers `6`; lines 16
  > and 17 `mark-seen!` two of them in a **fresh process with an empty store** and answer `0`. The
  > calls are all real and all reach the store; there is no store left to reach by the time the
  > next one starts. **Seen-state persistence is `T044`**, and regions need the same thing — so
  > `V006`'s seeded state becomes reachable at `T044`, not here, and `CP-5`'s
  > *"identical output on two machines"* rests on it. The script says so in its own summary rather
  > than reporting three landed capabilities and letting a reader infer a seeded fixture.
  >
  > **A flaky test, found by this task and not caused by it.**
  > `a_burst_of_typing_never_says_the_editor_denied_something` asserted
  > `!shows(&drawn, "lsp:")`. `shows` is a **fuzzy** matcher — a space in the frame matches any
  > wanted character, and only two thirds need to match exactly — so over four characters it needs
  > two, and the statusline's own `toy-lsp ` chip matches `lsp:` whenever a redraw puts the chip
  > inside the captured window. Whether it lands there is timing: the test passed alone, failed
  > five times running under load, then passed again with **only its panic message changed**.
  > Checked against `0d53dbb` before blaming it on the store, where it also passed — the sentinel
  > has been ambiguous since it was written and the odds simply moved.
  >
  > A needle too short for a fuzzy matcher is worse than none: it fails on a correct build and,
  > being fuzzy, could equally pass over a real notice. It is now the three notices `deliver` can
  > actually paint — the two policy refusals and the vocabulary's own — each long enough that two
  > thirds of it cannot come from a chip. Re-proved with the plant its own doc names (`*owed = 0`
  > in `Outstanding::answers`), which puts the notice back and turns it red.
  >
  > **Two door tests and two parity tests were pinned to `T041` being unbuilt** and went red the
  > moment it was: `door.rs`'s `EXPR` was `(unseen-regions "src/retry.rs")` and `parity.rs` used
  > `mark-seen`. Repointed at `watches`/`place-watch` (`S8`, `T074`/`T077`) with the hazard written
  > down at each — picking the nearest unbuilt capability as a stand-in guarantees churn at the
  > task that builds it. One of them, `the_eval_route_reaches_what_no_flag_can_express`, was also
  > *narrow* rather than merely stale: it treated `#ok` as the only shape a carried-out capability
  > answers in, and `mark-seen` answers a count.
  >
  > **Scope**
  > - Files: `crates/phosphor-core/src/store.rs` (+335/-27), `crates/phosphor-core/src/store/region.rs` (+1010/-0),
  >   `crates/phosphor/src/store.rs` (+316/-0), `crates/phosphor/src/main.rs` (+330/-40),
  >   `crates/phosphor/src/lsp.rs` (-92), `crates/phosphor-ui/src/gutter.rs` (+140/-0),
  >   `crates/phosphor-ui/src/diagnostics.rs` (-80/+15), `scripts/seed-fixtures.sh`,
  >   `scripts/lint-action-arms.sh`, `crates/phosphor/tests/{loop_pty,door,parity}.rs`
  > - Named units: 5 action arms × 2 dispatchers, 5 query arms, 1 owed arm, `gutter::spans`
  >   (one span→row conversion where there were two), 28 core unit tests, 4 handle tests,
  >   2 pty tests
  > - Verification: `just gate` green — 1192 tests, 18 lints; two planted mutations, each named a
  >   pty test that went red (a revision that stopped un-seeing drew `unseen=2`; a cursor miss
  >   widened to the file drew `unseen=0`)
  > - Risk: public API change yes (`phosphor_core::store`) · data migration no · cross-module yes
  >   (`phosphor-core`, `phosphor-ui`, `phosphor`) · reversible yes · external blocker no

  > **The second criterion arrived from `V006` at the `CP-3` audit**, on the `T022` precedent.
  > `V006` built the fixture tree and `scripts/seed-fixtures.sh`, and every capability its plan
  > calls refuses today because none of `S5`–`S8` exists — so there is nothing to make a `CP-5`
  > tape deterministic *with*. That half cannot be closed before this task and now sits on it.
  >
  > **It also owes an arm.** `set-virtual-text-visible` is declared and unapplied because
  > collapsing a virtual-text rail addresses it by owning region, and regions are this task —
  > recorded in `scripts/lint-action-arms.sh`'s RECORDED table against `T041`. Wire it here
  > rather than leaving it to a later audit.

- [x] **T042 · Node anchoring**
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

  > **All four arms are applied** — `place-anchor`, `goto-anchor`, `reanchor` and `jump` — in
  > both dispatchers, on `T041`'s precedent: `Editing::act` resolves the focus-relative targets
  > because it has an editor, `AppHost::apply` refuses those by name and honours the explicit
  > ones. The RECORDED table is unchanged and still holds eight rows, which is the right answer:
  > a row cites a task, and these cited *this* one.

  > **A `Vec<SyntaxStep>` is the fingerprint, and the child-index path is the design it
  > replaced.** `[3, 1, 0]` is exact and worthless for the one job anchors have: inserting a
  > function above another shifts every index after it, so the anchor follows the most ordinary
  > edit there is to the wrong node. What survives is what a person would say out loud —
  > *"`retry`, in `impl Backoff`"* — so a path is the chain of **named** ancestors, each as its
  > kind plus what identifies it.
  >
  > **The identifying text is three grammar fields, not one, and the fork's own test found
  > it.** `step_of` tries `name`, `trait`, `type` and joins what it finds.
  > `function_item`, `struct_item`, `class_definition` and `function_definition` carry `name`;
  > Rust's `impl_item` does **not**, so the first draft silently dropped `impl Backoff` out of
  > every path — caught by `the_chain_names_the_construct_a_person_would_say_out_loud` failing
  > on its first run. `two_trait_impls_of_one_type_do_not_collide` is the other half: `type`
  > alone renders `impl Display for Backoff` and `impl Debug for Backoff` identical, and an
  > anchor in one would resolve into the other. The list keeps the walk grammar-blind — field
  > names tried on every node, never a table of node kinds — which
  > `the_walk_is_grammar_blind_and_python_resolves_too` holds it to.

  > **`goto-anchor` grew a `label`, and that was the whole vocabulary change.**
  > `runtime/keymaps.scm` had named the gap precisely: *"`place-anchor` writes a `label` that
  > `goto-anchor` cannot read — it takes an `AnchorId`, and no capability turns a label into
  > one."* A binding is **data** (`input::table::Role` — *"nothing here is a closure"*), so `'a`
  > cannot look an id up before naming one, and a literal id in a keymap would be worse than the
  > silence it replaced. The lookup went to the door, where all three doors reach it. `exact` came
  > with it, because backtick-versus-quote is a legitimate ask from a script too.
  >
  > **78 generated keymap rows rather than a fourth `Awaiting` state.** `m` cannot consume the
  > `a` in `ma` by running code, so the alternative was new input-machine machinery beside
  > `Register` and `ReplaceChar`. Generating the pairs in scheme is less machinery for the same
  > 78 rows and keeps the keymap a table anything can read. `<C-o>` / `<C-i>` are bound too.

  > **The jumplist was bound, applied, and unusable — three defects, found by pressing it
  > (2026-08-17).** `<C-o>` and `<C-i>` had no test anywhere; the key survey below is what pressed
  > them, and all three of these were behind that one keystroke.
  >
  > 1. **`<C-o>` refused after a single jump.** `push_jump` set `jump_at = len - 1`, pointing *at*
  >    the entry it had just recorded, so `Seek::Prev` computed `0 - 1 = 0`, hit `jump`'s
  >    no-move guard and answered *"already at the oldest jump"*. The list held exactly where you
  >    came from and there was no way to reach it — the opposite of `push_jump`'s own stated rule.
  >    `jump_at` now means *"the index you are at, or `len` for the present"*, which is the one
  >    extra state the walk needs.
  > 2. **Walking the jumplist deleted the jumplist.** `jump` reached its target through
  >    `goto_anchor`, and `goto_anchor` calls `push_jump` — correct for `` ` `` and `'`, where
  >    arriving *is* a jump, and wrong here, because `push_jump` truncates the forward half. So the
  >    first `<C-o>` wiped every entry, pushed one, and left `<C-i>` with nowhere to go. It now
  >    takes a `record` flag; vim's rule is the same one, that moving along the jumplist does not
  >    add to it.
  > 3. **`<C-i>` was a binding no terminal could reach.** ctrl-i and tab are one byte, `0x09`;
  >    crossterm reports `KeyCode::Tab` and `decode` canonicalises it to `<tab>`, which was bound
  >    only in **insert** scope. A keymap that said only `<C-i>` was asking for a spelling the wire
  >    never produces. `runtime/keymaps.scm` binds `<tab>` beside it in normal scope — a second
  >    spelling of one binding, not a replacement, since a terminal speaking the kitty protocol can
  >    tell them apart and `<C-i>` is the name `:help` should print.
  >
  > `loop_pty.rs::a_region_motion_pushes_a_jump_and_the_jumplist_walks_back` presses `]u`, `<C-o>`
  > and `<C-i>` in one session, because a region motion is the only thing a user can press that
  > pushes a jump — `push_jump` has two callers — so the list cannot be exercised without one.

  > **The key survey (2026-08-17), and why there was one.** The file picker shipped a `↵` that
  > refused every row (`T047`), and the reason nothing caught it was that no test pressed the key.
  > So the same question was asked of every other binding: the live keymap answers
  > `(keymap-entries)` with **428** bindings, **42** of those are leaves naming a capability, and
  > grepping `crates/phosphor/tests/` for the bytes that press them found **19**. Twenty-three
  > command keys were driven by nothing.
  >
  > The grammar keys are deliberately not in that 42 and do not want pty tests — `h`, `w`, `dw`,
  > `ciw` are the input machine's and are covered exhaustively in `phosphor-core`; pressing each
  > through a terminal would be re-testing `Machine::feed`.
  >
  > Three tests came out of it: `J` (one buffer mutation, one arm, one binding, no coverage), the
  > jumplist above, and **`a_deferred_binding_names_the_task_that_builds_it`** — a table over the
  > nine keys whose capability has not landed, asserting each says *which task builds it*.
  > `runtime/keymaps.scm` promises exactly that and nine keys relied on it unpressed. It is a table
  > so it cannot go stale quietly: when a task lands its key stops refusing and the row that named
  > it goes red, the same shape as `scripts/lint-action-arms.sh`'s RECORDED list one layer out.
  > Its expected tasks were wrong three ways when first written from the keymap rather than from
  > `action.rs` — `SPC c p`/`SPC c s` open a *prompt* (`T058`, not their group's session task),
  > `SPC t` is `set-pane-content` (`T054`), and `SPC r d` is `open-disk-diff` (`T070`).
  >
  > **Widened past the keymap, same day.** Keys are one way in; the ex line, the mouse and the
  > floats are the others, and each was counted the same way — enumerate what ships, grep for what
  > presses it.
  >
  > * **Ex commands.** `(ex-entries)` answers **18**, and **nine** were typed by no test at all:
  >   `wall`, `wq`, `xit`, `close-buffer`, `transcript`, `inbox`, `diff-disk`, `reattach`,
  >   `comment`. Probing each showed three are live and six refuse legibly.
  > * **The mouse.** `mouse_actions` handles three kinds — press, drag, **wheel** — and the one
  >   mouse test in the repository pressed the first two. Nothing had ever turned a wheel.
  >
  > Five tests came out of it. `:wq` and `:xit` are the commonest exit in vim and neither was
  > typed, although `Session::key`'s *"first refusal wins"* fix — found by hand at `CP-4` — exists
  > because `:wq` and `ZZ` are the same Action list and were answering differently. That fix was
  > pressed through `ZZ` only. Both new tests assert **on disk**, because a `:wq` that quit without
  > writing leaves a green frame and a lost edit. `:wall` gets the third.
  >
  > The wheel test asserts the invariant the two tested mouse kinds pull the *other* way on: a
  > viewport move is not a cursor move. Reading further down a file must not take the insertion
  > point along, and press and drag both legitimately move it, so nothing was holding that line.
  > Pressed against a planted violation — the wheel emitting `MoveCursor` instead of `Scroll` — and
  > it fails on the statusline reading.
  >
  > The sixth is the ex line's half of the deferred table: `transcript` (`T054`), `inbox`
  > (`T067`), `diff-disk` (`T070`), `reattach` (`T057`), `comment` (`T068`), and `close-buffer`,
  > which is not deferred but declines while there is one pane and can only honestly name `T088`.
  >
  > **The rest of the surface, audited (2026-08-17).** What was left after the two passes above,
  > and what the audit *cleared* as well as what it filled.
  >
  > * **Filled — six live keys, every one the second half of a pair whose first half was tested.**
  >   `zc`/`zo`/`zM` (only `za` and `zR` were pressed; a toggle passing says nothing about whether
  >   the explicit close/open pair are wired to the right states), `[u` and `SPC u n` (one
  >   capability, three bindings, one pressed — the shape the `<C-i>` defect had), and `<C-p>` (one
  >   signed delta with only the positive sign exercised).
  > * **Filled — `esc` on every surface you can open.** Eight `Surface` variants ship; `Boot`,
  >   `Fixture` and `Buffer` are not things a key opens over your file. For the other three, `esc`
  >   was *pressed* in three tests and **asserted in none** — always cleanup before the next
  >   assertion — and help's dismissal is tested through `q`, which its footer documents, not
  >   through `esc`. A surface shipped without a way out would have been caught by nothing.
  > * **Filled — `]b`/`[b` joined the deferred table**, which had nine rows while the deferred
  >   surface had eleven. `goto_sequence` names `T053` for `BlockFile`.
  > * **Cleared — the MCP and CLI doors.** `parity.rs` walks `registrations` and exercises all
  >   three doors at every row, building each call from that capability's own canonical example.
  >   That is enumeration rather than a list, so it cannot rot the way a hand-written one does —
  >   the same principle these surveys are applying, already in place one layer over.
  > * **Cleared — counts, registers, macros, operators, text objects.** Covered in
  >   `phosphor-core/tests`; pressing each through a terminal would be re-testing `Machine::feed`.
  > * **Not testable here, and documented.** Resize: `coalesce`'s own doc records that a pty
  >   harness cannot exercise it, because the slave fd is moved into the child and Apple's master
  >   rejects `TIOCSWINSZ`. Unit tests on `coalesce` and the width walks carry it instead.
  >
  > **Still open**, and named rather than left implied: `<C-d>`/`<C-u>`/`<C-f>`/`<C-b>` press
  > nothing in any pty test. The `ScrollRequest`s they build *are* tested in
  > `phosphor-core/tests/input.rs` and across the screen tests, so what is missing is only the
  > keystroke→request hop — the same hop the `<C-i>` defect lived in, which is why it is written
  > down rather than dismissed.
  >
  > **A test that passed a planted violation, and the fixture was the bug.** The `[u` test was
  > written with two regions and a plant of `Seek::Prev => Next` **passed** it: `Next` wraps
  > (`find(|line| *line > here).unwrap_or(lines[0])`), so from the last of two regions forwards
  > and backwards land on the same line. Three regions with the walk stopping on the middle one is
  > what makes the two answers differ, and the same plant then fails. Planting is not a formality —
  > it is the only thing that catches an assertion which is true for the wrong reason.
  >
  > **A methodological finding worth more than any of them.** The first pass of this survey used
  > a quiet press and a read of the final grid, and reported four working keys as silently broken —
  > including `SPC j`, written up as *"produces nothing at all"* before being checked. The grid is
  > a race with whatever redraws next; a notice is drawn and then overdrawn. `press_until` scans
  > what was **drawn since** the keys and finds it. The converse bit too: that reader is a *delta*,
  > so `J` joining `alpha` and `bravo` writes only ` bravo` and `"alpha bravo"` is on the screen
  > and never in the delta. Two readers, two questions — `Editor::shown_on_grid` is the second one,
  > and its doc says which is for which.

  > **PHOSPHOR PATCH 12** is the one new seam: `Code::syntax_path(byte)`, read-only, over the
  > tree the fork already keeps current. The host was the alternative and loses on three counts —
  > a second grammar table, a second parse per reanchored file, and a tree that can *disagree*
  > with the one the editor highlights from, because the editor's is incremental and a fresh
  > parse is not. Nine tests in `vendor/…/tests/syntax_path.rs`, which `just test` cannot see, so
  > `scripts/lint-vendor-tests.sh` now requires the binary by name.

  > **A door places anchors at full fidelity**, by reading the file off disk and parsing it with
  > the same `grammar_of` the loop uses. An agent asking to anchor `src/retry.rs:24` is talking
  > about the file, not about unsaved buffer state, so disk is the honest source on that side —
  > and *"placed over MCP"* does not mean *"resolves one tier worse"*.

  > **What the bench corrected about its own prose.** `benches/anchor.rs` asserted a miss was the
  > expensive case — both rungs scanned to the end. It is the **cheap** case, by 5×: a
  > fingerprint whose path has a different number of steps dies on `Vec`'s length comparison
  > before one string is read. The dear case is a node-tier *hit*, where equal lengths make every
  > line pay a full step-by-step compare. Recorded because it inverts where an optimisation would
  > go — hash the path, do not shorten the scan. Both growth shapes hold at ~1.06× per doubling.
  >
  > This paragraph claimed the opposite — that `Jump` *was* in
  > `scripts/lint-action-arms.sh`'s RECORDED table — for a window, while the sentence directly
  > above it stated the correct rule for `place-anchor`. The lint's own comment (`:129`) records
  > the removal and the reason: `Jump` and `ApplyEdits` were declared against *ticked* tasks that
  > had demonstrably not built them, so the derived refusal named the wrong task (`jump` said
  > *T026 builds it*, which was false). Re-declaring them against unticked tasks took them out of
  > the table's ticked filter, which is the right answer — the attribution was the bug, not the
  > absent arm. Found by the Window E scout, by grepping the table instead of trusting the prose;
  > no lint catches this class, because `doc_claims.py` recomputes counts, not cross-references
  > into a lint script's internals.

  > **`T041` left this task a rule to replace, and named it as an approximation rather than a
  > design.** With no anchors, a declaration has only a path and a span to find the region it
  > revises by, so the store's identity rule is *overlap on the same path with the same claimed
  > author*, absorbing every region it reaches into one whose span is the union
  > (`store/region.rs`'s header argues why union rather than replace: it keeps *"claude wrote here
  > and you have not looked"* true of every row that was ever covered). That is exactly the rule
  > an anchor makes unnecessary. **When this task lands, `Regions::declare` should find its
  > creditor by anchor and the overlap rule becomes the fallback** — which is `T043`'s tier, not a
  > third mechanism. `store::Shared::covering`, which gives a diagnostic's virtual-text rail its
  > owner, is positional for the same reason and moves with it.

- [x] **T043 · Line + content fallback anchoring**
  **The floor, not a degraded extra** — this is what makes unseen markers a store feature rather
  than a language feature (invariant 4).
  *Done when:* markers work correctly on an extensionless file with no grammar. *Needs:* T041

  > **The tier shipped with `T042` and the criterion was met here**, which is why the two commits
  > are separate. `anchor::resolve` is one function with both rungs in it — writing the node tier
  > without the line tier underneath would have meant a `Tier::Lost` for every `.env` in the
  > world and a second pass to fix it. What `T042` could not tick is this task's actual sentence:
  > *markers*, not anchors. A marker is a **region**, and regions were positional.
  >
  > **So regions ride the same ladder now.** `Region` gained an optional [`Fingerprint`],
  > `Regions::fingerprint_in` fills it from the host's snapshot, and `Regions::reanchor_in` moves
  > the span — through `anchor::resolve`, the same call anchors make, so *"node tier, then line,
  > then lost"* cannot come to mean two things. `Store::reanchor` runs both halves in one call
  > because a reanchor that moved only the anchors would leave every unseen marker behind on the
  > line it used to be on.
  >
  > **Optional, and absent is a state rather than a hole.** A `RegionSpec` is a wire type
  > carrying a path and a span; a fingerprint needs the file's *text*, which the store does not
  > have. So a declaration still creates regions positionally and the host describes the file
  > afterwards — from the buffer on the keystroke side, from disk on the door side, one parse per
  > distinct path. A region for a file nobody has opened keeps `None` and stays exactly as
  > positional as it was before this task, which is the honest degradation.
  >
  > **Three rules that are each a wrong answer avoided**, all with a test: filling is
  > *fill-only*, because recomputing after a rewrite replaces a good description of a location
  > with a description of whatever has since moved onto that line; a **revision drops** the
  > fingerprint, because the span moved and the old description points at a line the region no
  > longer starts at; and a region whose start is **lost stays where it was** — same rule anchors
  > follow, since a marker that moved somewhere plausible is a lie a person acts on.
  >
  > A region keeps its **height**: only the start resolves and the extent is shifted by the same
  > delta, because a region's extent is a property of the declaration and not of the text.
  >
  > Pressed by `a_marker_on_a_grammar_free_file_survives_the_edit_that_moves_it` — a file called
  > `deploy`, no extension, nothing in the bundled ten parses it, so the node tier never applies
  > and the line tier is the only thing holding the marker on. Two lines inserted above it and
  > the region's span moves from 2 to 4. A positional region would still have claimed line 2.
  >
  > [`Fingerprint`]: ../crates/phosphor-core/src/store/anchor.rs

- [x] **T044 · Seen-state persistence**
  `$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/`, keyed on path never VCS identity (Q1).
  Append-only log + compaction, **same format as undo** (T030).
  **And the regions themselves, which `T041` found this task also owes.**
  *Done when:* seen-state survives restart and `kill -9`, in both a jj repo and a bare
  directory, **and `bash scripts/seed-fixtures.sh` leaves a store `phosphor` can open** —
  `V006`'s criterion, which lands here rather than at `T041`. *Needs:* T030, T041

  > **The scope grew by one noun at `T041`, and it is the noun that makes `V006` possible.**
  > Seen-state alone persists nothing worth reading: a seen flag refers to a region, and if the
  > regions are gone the flag has no subject. `T041` proved it by running the seeding plan —
  > **`scripts/seed-fixtures.sh` is one `phosphor --eval` process per line**, so line 9 declares
  > six regions and answers `6`, and lines 16 and 17 `mark-seen!` two of them in a fresh process
  > with an empty store and answer `0`. Every call is real and reaches the store; there is no
  > store left to reach.
  >
  > So `V006`'s *"seeded store state is reachable through `phosphor --eval`"* — moved onto `T041`
  > at the `CP-3` audit, and answered there rather than met — is **this task's**, and `CP-5`'s
  > *"identical output on two machines"* rests on it. The store is one `Revision` over regions and
  > diagnostics (`phosphor_core::store::Store`), so what persists is a decision this task makes
  > once for the whole thing rather than per sub-store.
  >
  > **The workspace-root question comes with it**, unresolved since `V006`: `fixtures/` is its own
  > root, and `phosphor/src/store.rs`'s `key_for` currently reconciles a declared path against the
  > *working directory* — enough for one session, and not the canonical-root hashing Q1 specifies.
  > `fixtures/README.md`'s residue item 7 is the same question.

  > **Built, and it was the small half of what this entry describes.** `journal.rs` had already
  > shipped everything but the fold, two phases early and on purpose — its own header lists what
  > `T044` *"gets"*: the codec, the framing and CRC, `Recovery` and the torn-tail truncation,
  > `Log` with append/compact/`compact_if_needed`, the atomic rewrite, `state_home` /
  > `workspace_dir` / `workspace_key` with Q1's canonical-root keying **and its collision
  > marker**, and `Stream::SEEN` reserved so the two streams cannot collide on disk. What it
  > *"supplies"* is one `impl Folded`, and that is `store/persist.rs`.
  >
  > **The two halves of the workspace-root question above turned out to be separable**, where
  > this entry read them as one problem: the canonical-root hashing Q1 asks for already existed
  > in `journal.rs` and is what the seen journal is keyed on; `key_for`'s working-directory
  > reconciliation is a *path-spelling* rule for region records and is untouched. The first was
  > never missing. The second is not what Q1 was about.
  >
  > **Every record is a whole row, not a delta.** `Folded`'s law is `fold(snapshot(state)) ==
  > state`, and `Log::compact` rewrites the file as `snapshot(state)` — so a `snapshot` that
  > loses something loses it permanently and silently. Whole-row upserts make `snapshot` *"one
  > record per live row"* and the law true by construction. A delta schema makes it a
  > reconstruction problem, which is the shape of bug that surfaces only after a compaction, in a
  > file nobody can read, a week later. The cost is bytes, and bytes are what compaction is for.
  >
  > **A fifth record nobody would guess at: `Minted`.** Both collections mint ids monotonically so
  > a surface holding a dropped id gets nothing back — and the largest id *still alive* is not the
  > largest that ever existed. Drop the highest region and that fact leaves with it, so restoring
  > from live rows alone reissues a retired id after a restart. One record, coalesced to exactly
  > one by every compaction, folded with `max` so a replay cannot walk it backwards.
  >
  > **Anchors ride along**, because a mark is user state in exactly the way seen-state is.
  > **Diagnostics deliberately do not**: they are a language server's assertion about the current
  > text, and a restored one is a claim nobody is standing behind. The revision restarts at its
  > initial value — a revision is a cache key within one process, and restoring a high one would
  > let a `T079` cache from a previous run believe its entries were current.
  >
  > **A failed append disables the journal rather than failing the edit.** The alternative is an
  > editor that refuses `s` because a disk filled up, trading a lost flag for a lost session. Not
  > having a journal is a state this already supports — no `XDG_STATE_HOME`, a read-only state
  > directory — so degrading into it is a path that already works.
  >
  > **`V006` is met, and the proof is the two lines that used to answer `0`.**
  > `scripts/seed-fixtures.sh` is one `phosphor --eval` process per line; its own summary said
  > *"ONE PROCESS PER LINE, AND NOTHING SURVIVES BETWEEN THEM"*, and that `declare-regions!`
  > answered 6 while the two `mark-seen!` lines below it answered 0 against the empty store of a
  > fresh process. They answer **1** now — they find the regions an earlier *process* wrote. That
  > paragraph is rewritten to say what is true, and to name what is still not asserted (`CP-5`'s
  > fixed point: running it twice, and two machines agreeing).
  >
  > *"In both a jj repo and a bare directory"* is **one** behaviour rather than two that agree,
  > and `a_vcs_directory_does_not_change_where_state_lives` is the assertion: planting `.jj` and
  > `.git` changes nothing about where state goes, which is Q1's *"keyed on path never VCS
  > identity"*. The tempting optimisation is the bug — keying on a repository root would make a
  > worktree, a submodule and a fresh clone three stores for one checkout, and would leave a
  > directory that is not a repo at all with nowhere to put anything.
  >
  > `kill -9` is pressed at a real pty by `seen_state_survives_a_kill_nine`: two sessions, one
  > `XDG_STATE_HOME`, three regions with one marked seen, `SIGKILL` rather than a clean quit, and
  > the second process reads `unseen=2` / `seen=1`.

- [x] **T045 · Picker widget**
  `ratatui-textarea` filter line + nucleo matcher **off-thread** + list + preview split (dropped
  under 100 cols). Rows are `Vec<Span>` so agent context renders in actor colours.
  *Done when:* it stays responsive filtering a 100k-file list. *Needs:* T041, T084

  > **Both dependencies are verified and neither is in the graph yet** — checked by the
  > pre-window scout, by resolving and building them rather than by reading `SPIKES.md` again.
  > `SPIKES.md`'s manifest pinned both a phase ago and both still resolve to exactly those
  > versions, which is worth knowing on its own.
  >
  > | crate | version | licence | MSRV | notes |
  > |---|---|---|---|---|
  > | `nucleo` | 0.5.0 | **MPL-2.0** | none declared | allowed at `deny.toml:54`; pulls `nucleo-matcher`, `memchr` |
  > | `ratatui-textarea` | 0.9.2 | MIT | 1.86.0 | under our 1.88 floor, so it moves no MSRV |
  >
  > **The one trap: `ratatui-textarea`'s default feature set includes `crossterm`.** `phosphor-ui`
  > may not reach a terminal, and `scripts/lint-no-app-layer-in-ui.sh` is a *source* lint precisely
  > because *"Cargo unifies features per crate across the graph, so the manifest cannot express
  > it"*. Add it `default-features = false`, the way both vendored forks already are. Dropping the
  > defaults loses only backend bindings (`crossterm`/`termion`/`termwiz`) — the widget renders
  > through `ratatui-core` and takes input from our own machine, so nothing wanted is lost.
  >
  > **No new versions enter the graph.** Built together on the pinned 1.97.1 in a scratch crate:
  > one `ratatui-core`, **v0.1.2**, unified with ours — so `deny.toml`'s
  > `deny-multiple-versions` ban on that crate holds — and `ratatui-widgets` resolves to 0.3.2,
  > which `Cargo.lock:2211` already carries at that exact version.
  >
  > **`nucleo` is two years old** (published 2024-04-02) and declares no `rust-version`. It
  > compiles clean on the pin. It is Helix's engine and the plan chose it on that basis; recorded
  > because "stale" was the finding that killed `tui-textarea`, and the difference here is that
  > this one still builds against a current graph.

  > **The preview is a diff, and that settles a question one layer over.** `2a` draws the preview
  > pane as `+`-prefixed diff lines over `src/retry.rs · 6–10`, not as a buffer with a state
  > column. So this task does **not** create a creditor for `Node::Gutter` — the one node kind in
  > `scripts/lint-node-kinds.sh`'s RECORDED table with no task that closes it. Checked because the
  > guess was plausible and wrong; recorded so it is not re-derived.

  > **`ratatui-textarea` was not taken, and this task asked for it.** Reading the vocabulary the
  > widget has to render says otherwise: `Node::Picker` carries **`filter: String` as a prop**, so
  > the filter's text is composition's and arrives fresh every frame. A textarea inside the widget
  > would hold a second copy and have to be reconciled with the prop on every composition — two
  > maps with one name, which is exactly what `T041` found in `store::diagnostics` and folded
  > away. What the crate would buy is editing *inside* the filter line, and none of that is
  > reachable: keystrokes go to the input machine and `Node::Picker` has no cursor prop to carry a
  > position back. So it would add a dependency, a feature-unification hazard (its defaults
  > include `crossterm`, which `phosphor-ui` may not link) and a duplicate source of truth, for
  > nothing currently expressible. **Flagged rather than folded in** — if a cursor inside the
  > filter is wanted, the change is a prop on `Node::Picker` first and the crate second. `nucleo`
  > *was* taken, in the binary.

  > **The task splits across two crates, and the seam is where the threads are.** `phosphor-ui`
  > draws a `PickerVm`; the binary fills one. A widget crate that owned nucleo's thread pool would
  > be a widget that outlives a frame, and `Resources` has no `&mut` in it and must never grow
  > one — so the loop ticks the matcher once per frame, before the draw, and lends the answer.
  > The tick takes a **1ms deadline** and never blocks: a filter over 100k rows draws a partial
  > result and says so through `PickerVm::matching`, which is what the `…` in `12/100000…` is.

  > **Three assertions about responsiveness were written, run, and found wrong** — kept in the
  > test's own doc because each looked obviously right:
  > *"no tick costs more than a frame"* (a wall clock: `nextest` runs sixteen processes and this
  > worktree has *"seen absolute times swing 25× under concurrent load"*);
  > *"the first tick reports `matching: true`"* (`Status::running` says whether workers are
  > running **now**, not whether work is outstanding);
  > *"the first tick has not matched all 100k"* (with an **empty** pattern there is nothing to
  > match, so the full count is correct and immediate). Even `item_count` is zero on the first
  > tick under load — injection is asynchronous. What survives asserts the settled state, the
  > narrowing property, and a hang detector at 500 frame budgets. *"Never blocks"* is a property
  > of the **API shape** — a deadline-based tick, polled — visible in twelve lines rather than
  > derivable from a measurement.

  > **The bench measures the half a bound cannot see**: the widget is handed only the rows that
  > fit, so its cost must track the **window** and not the corpus. A widget that iterated the
  > whole list to lay out would be a quadratic on the far side of the seam from the matcher.
  > Measured flat at **1.00×** across a 10,000× corpus growth, linear at 1.13× per window
  > doubling, and §11's ladder is a cliff exactly at 100 columns rather than a taper.

  > **`open-picker`'s row cites `T046` and it is applied here anyway.** A widget nothing can put
  > on screen is the reachability gap `T016` was ticked with; what `T046` actually owes is the
  > *rows*. `SPC u l` was already bound to it and answered *"not built yet"* — the binding did not
  > change, which is `runtime/keymaps.scm`'s own rule. An open picker over a source nobody has
  > defined draws `0/0`, which is honest where pretending to a list would not be.
  >
  > All five capabilities the task's own rows name are applied: `set-picker-query` (answering the
  > match count, so a script does not need a second round trip), `toggle-picker-preview`,
  > `float-select`, `float-select-row`, and `float-accept` — which declines by naming `T047`,
  > because accepting a row needs a row that names a *place* and that is a source's to supply.

  > **`Node::Picker` had no arm in the interpreter's `height`**, so the float collapsed to its
  > chrome and drew a header and two rules around nothing. Found by the pty test, which is the
  > only thing that could have: every widget test passed. It answers the filter line plus what
  > matched, **not** `u16::MAX` the way `Node::Buffer` does — §8 is *"no surface is ever taller
  > than its content"*, and a picker over an empty source has one row of content.

- [x] **T046 · Steel picker sources — unseen, files**
  `(define-picker-source …)`. Files carries unseen counts and activity columns.
  *Done when:* screens `2a` and `3d` reproduce **from a keystroke** — the binding that opens the
  picker opens it in the running binary — and a source added from the REPL appears with no
  restart. *Needs:* T045, T022

  > **`OPEN-QUESTIONS.md` §42 is owed here**, ruled 2026-08-17 as a deferral to this task. A door
  > cannot ask about the cursor: `AppHost::scope` refuses `cursor`, `selection`, `picker-row` and
  > `float-row` for the `region` queries, naming the three tags it does take, because the host has
  > no editor — `Editing::scope_of` is the half that does and it is on the other side of the Steel
  > barrier. The *Actions* are fine (`SPC u s` passes `(key/at-cursor)` and resolves); it is only
  > the **queries** that are one-sided.
  >
  > It lands here because a picker source is the first thing that will want to ask *what is unseen
  > **here***. The fix is structural and the shape is ruled: **queries from the VM route through
  > the loop the way Actions do**, rather than the host keeping a copy of the cursor — a second
  > copy of editor state is exactly the staleness `Target`'s own doc says late binding exists to
  > avoid.

  > **§42 is still owed, and this task turned out not to be its caller.** Neither shipped source
  > asks about the cursor: `2a` lists every unseen region in the workspace and `3d` groups them by
  > path, so both are whole-store reads that `unseen-regions` already answers. The ruling put §42
  > here on a prediction, and the prediction was wrong in a way worth recording rather than
  > quietly satisfying — building the routing with no caller would be the *"built, tested and
  > uncomposed"* shape this build has already found twice.
  >
  > What **did** meet §42's edge is `picker-rows`, which is a query tagged `T045` and answers from
  > a snapshot the loop publishes rather than by running a source. Running a source is running
  > scheme; a query arrives from *inside* the VM and `Host::query` takes `&self`. So it answers
  > for the **open** picker and every miss answers an empty list — `query.rs`'s own *"an absent
  > thing answers empty"*, which is also why no refusal variant was added to `QueryError`. The
  > limit is stated at `HostState::picker_rows` rather than hidden. §42's routing is what would
  > lift it, and it now has a concrete second caller instead of a predicted one.

  > **A source answers a `spans` node**, which is `T080`'s hatch — *"styled rows straight from
  > Steel"* is already the vocabulary for this shape. A second one would mean a constructor for
  > scheme to learn, a decoder to maintain, and two places for *"a row is runs, left to right"* to
  > drift. It also keeps the barrier honest: `SpanRow` is `phosphor-core`'s, so `phosphor-steel`
  > answers a core type and never names `phosphor-ui`, and `crate::picker::row_of` is the whole
  > conversion.

  > **The registry is `T093`'s, called rather than copied.** `valid_source_id` *is*
  > `valid_surface_id` — two spellings of one validation is how the weaker one gets found by
  > somebody else — and `define_form`, the prefix and the three-step call are the same shape for
  > the same reasons. An id is validated because it is interpolated into a `define` form and the
  > capability is `Allow` on MCP.

  > **`files` listed only what claude had touched, and that was wrong.** The limit was stated
  > rather than hidden — *"no capability walks a directory"* — but stating a limit does not make
  > it the right design, and this one made `SPC f` open an **empty picker** in any session with
  > nothing declared. Found by Teej testing a normal build: *"file picker has no files in it — is
  > it really a buffer list not a file list"*. It was neither.
  >
  > `3d` settles it and always did: its caption is *"the file picker carries agent state: unseen
  > counts + activity, **not just names**"*, and its own rows include `src/main.rs` and
  > `Cargo.toml` carrying no activity at all. **The list is the workspace and the store annotates
  > it** — never the other way round.
  >
  > The walk is `crate::picker::workspace_files`, in the binary, handed down in the source's args
  > exactly as `grep` gets the buffer's lines and for the same reason (§42). It skips five
  > directories by name rather than reading `.gitignore`: that would mean either ripgrep's
  > `ignore` crate for one picker, or a half-implementation of a format with negations and
  > precedence that would be wrong unpredictably. Capped at 100,000 with the caller told, because
  > the *walk* is on the keystroke where the matcher is not.

  > **A door-opened picker took a keystroke to appear**, because the `open_picker` drain sat
  > *before* the `Intent` drain: a keystroke sets the flag during event handling and is drained
  > later the same iteration, but an `Intent` posted from the VM set it after the drain had
  > already run — and with no further keys the loop simply waits. Moved below the intents, which
  > is where `Intent::OpenSurface` already does its whole job inline. Found by the pty test doing
  > the last thing a person would do: opening it and pressing nothing else.

  > **A seven-step pty session went red under the full suite and green alone**, so it is two tests
  > now. Each `press_until` carries its own deadline, which makes a long test one whose budget is
  > the sum of everything before it — a flake with a cause rather than a mystery. Boot also posts
  > intents now (one `DefineSource` per shipped source), so five tests that asserted an exact
  > intent list gained `after_boot`, which drops registrations and keeps the question *"what did
  > this keystroke ask for"* answerable.

- [x] **T047 · Grep / symbols source**
  Tab cycles source. Results carry who-touched-them. **And `request-references`**, which was
  re-homed here by the `S4` wiring pass: `LanguageServers::ask` answers a `Vec<FileSpan>` and
  nothing in the vocabulary carries a list of places, so this is the task that builds the surface
  one is drawn in. `phosphor_buffer::lsp::Question::References` and `file_edits_from_lsp` are
  built and unreached until it does.
  *Done when:* screen `8a` reproduces **from a keystroke**, tab included — **and `gr` fills it
  from a real server**. *Needs:* T046, **T036**

  > **The `Needs: T036` is the debt made visible**, and it is here because the re-homing hid it.
  > `apply-workspace-edit` stayed on `T036` and is RECORDED in `scripts/lint-action-arms.sh`
  > against `T060`, so the lint still names it every run; moving `request-references`' phase and
  > task tag to an unticked task instead made the lint stop asking — no recorded row, no line in
  > the summary, nothing that expires. Found by the `CP-4` review, which is the second reader this
  > build has had for a re-homing and the first to check whether the creditor knew.

  > **`symbols` is not built, and that is a gap rather than a cut.** The vocabulary's LSP
  > `Question` has `Definition` and `References` and no `DocumentSymbol`, so there is nothing to
  > ask a server for. Adding one is a capability change plus a client path plus a fixture arm —
  > real work, and work for a task that names it. What tab cycles is
  > `phosphor/picker-sources`, which the *layer* owns, so the day a symbols source exists it joins
  > the list with no Rust change. Recorded in `runtime/pickers.scm` at the list itself.

  > **`grep` reads the open buffer, not the workspace** — the same limit `files` states, from the
  > same absence: no capability searches files on disk. What it can read is the buffer's lines, so
  > grep is a fuzzy search over what is open. **The matching is nucleo's**, which is the part
  > worth saying: the source hands over every line and the filter narrows them, so typing in the
  > picker *is* grep's prompt and the source does no searching at all.
  >
  > `8a`'s *"results know who touched them"* is the store's, per row: the unseen dot comes from
  > `unseen-regions`, built once per open into a set keyed by `path:line` rather than queried per
  > row.

  > **A row's text is its address**, and that is the design rather than a shortcut. A row is
  > styled runs and nothing else — no hidden payload, no id alongside — so `picker-accept` parses
  > the row's own head. The alternative is a parallel array of targets beside the rows, and it
  > goes out of step the moment a source is redefined at the REPL: the rows change and the shadow
  > list does not. This cannot, because there is only one thing.
  >
  > **~~every source writes `path:line` first~~ — false, and it shipped a broken key (fixed
  > 2026-08-17).** `grep`, `unseen` and `references` write it and `8a` draws it, but `3d`'s file
  > rows are bare names — `src/main.rs`, `Cargo.toml` — and that is what `files` writes. So `↵` on
  > *every* row of the file picker declined with *"that row does not name a place — sources write
  > `path:line` first"*: a sentence quoting an invariant that was only ever asserted in a doc
  > comment. The one source that followed the mockup was the one that could not be accepted.
  > Reported by Teej at a real terminal.
  >
  > There are two spellings because the mockups draw two, and both are addresses: `path:line`
  > carries a position and the cursor lands on it; a bare path is a whole file and carries none,
  > so `open_at` stays `None` — which is the difference doing work rather than a default standing
  > in for one, since accepting the file you already have open then leaves the cursor where you
  > left it instead of yanking it to line 1.
  >
  > **Nothing had ever pressed `↵` on a picker row.**
  > `grep_rows_carry_the_store_and_tab_cycles_the_source`'s summary line says *"tab, and `↵`
  > opens"* and its body stops after the tab; every other picker test asserts on the list and
  > presses escape. A whole keystroke on a shipped surface, described in a doc comment and covered
  > by nothing. `loop_pty.rs::enter_on_a_picker_row_opens_it_whichever_way_the_row_is_spelled`
  > presses both spellings in one session and fails two ways against planted violations — the old
  > refusal draws Teej's sentence into the frame verbatim, and dropping the span leaves the
  > statusline reading `1:1` where it should read `3:1`.
  >
  > `AcceptHow::Split` and `AcceptHow::Quickfix` decline by naming what they need — one pane
  > until `T088`, and a quickfix list that `request.rs` records as *"drawn once and named in no
  > task"*. Building one here would be inventing a surface nobody asked for.

  > **`request-references` answers into a slot, not an Action**, and the entry above is why: *"no
  > `Action` carries a list of places"*, and it still does not. The callback fills a
  > `Arc<Mutex<Vec<FileSpan>>>`, posts an ordinary `open-picker`, and the loop hands the places to
  > the `references` source as arguments — the same *"the host resolves what only the host can"*
  > seam `grep` uses for the buffer's lines. A capability whose payload is a list of places would
  > be a vocabulary change to carry one answer to one surface; if a second consumer ever wants the
  > list, that is the moment to make it one.
  >
  > The toy language server gained `textDocument/references`, answering **three places in two
  > files**. One place would not have separated a working references picker from a working `gd`:
  > a single place can be answered by opening it, and `8a` exists because a list needs a surface.

  > **The `Needs: T036` debt is paid.** `Question::References` and its whole client path were
  > *"built and unreached"*; `gr` reaches them now, and
  > `gr_fills_the_picker_from_a_real_server` presses the key against a real process.
  > `gr_declines_by_naming_the_task_that_builds_the_list` was that state's pin and is replaced by
  > `gr_is_bound_and_does_not_spend_the_session_hint` — the half of `CP-4`'s finding that outlives
  > the fix, which is that a *bound* key must not spend `8e`'s one teaching row.

- [x] **T048 · `:arch` / ArchDiagram**
  A float body over a store query (Q11), **built entirely from the `spans` hatch** (T080) — no
  Rust primitive of its own. It is the proof that the escape hatch is sufficient for a real
  custom surface. Turns invariant 4 from a claim into something you can look at.
  *Done when:* typing `:arch` in the running binary reproduces screen `6a`, it reflects the
  *actual* store rather than a static drawing, and it adds zero lines to `phosphor-ui`.
  *Needs:* T041, T080, T084

- [x] **T049 · Agent nouns resolve**
  `viu` / `sib` / `dih` now bind to real regions (completes T028, per Q8).
  *Done when:* screen `6d`'s nouns are functional, and `viu` selects an unseen region.
  *Needs:* T028, T041

- [x] **T087 · Region tints via a marks side table** 📌
  The seam the T008 spike said the bought marks API *is* good for, and the one nothing was
  tasked to build. Design Language §3 tints the whole row per region state — `#141d16` anchor,
  `#26332a` selection-in-float, `#211114` failure — and the marks API carries exactly that
  (colour spans) and nothing else.
  Three consequences from the spike, all landing here: **marks carry no id**, so region ↔ mark
  mapping needs our own side table keyed by offset range; **`set_marks` replaces wholesale**
  (`editor.rs:782`, and `set_marks_colored` at `:798` — this read `660-682` for a window, which is
  the code-folding block), so every seen-state change re-uploads the full set — keep the upload off
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

> **The mechanical half, recorded 2026-08-19 — and this is not a verdict.** `CP-5` has two halves
> and only Teej can run the second. This is written here because `CP-2`'s rule puts it here: **a
> checkpoint verdict is written where the checkpoint is, or it did not happen** — and the same
> holds for the half that comes before one. `CP-4` has had such a paragraph since `S4`; this
> checkpoint had none, while the work it describes was spread across commit messages.
>
> * **The gate is green** — `1349` tests and `21` lints.
> * **Region state machine tests: yes.** `crates/phosphor-core/src/store/region.rs` and
>   `tests/properties.rs`.
> * **Markers on an extensionless file with no grammar: yes**, both tiers — the unit test is
>   `anchor.rs::the_line_tier_catches_a_file_with_no_grammar_at_all` and the capture is
>   `tapes/no-grammar.png`, over `fixtures/src/deploy`, which exists because every other fixture
>   file carried one of the twelve declared languages and the node tier had nothing to fail on.
> * **Seen-state survives restart and `kill -9`, in a jj repo and a bare directory: yes** —
>   `loop_pty.rs::seen_state_survives_a_kill_nine`, and `T044`'s own
>   `journal.rs` test for the two workspace shapes.
> * **Picker responsive on 100k files: yes** —
>   `picker.rs::a_hundred_thousand_rows_never_block_a_frame`, which asserts a shape rather than a
>   time for the reason the benchmarks give.
> * **A REPL-added picker source appears without restart: yes** —
>   `loop_pty.rs::a_source_defined_at_the_repl_opens_with_no_restart`.
> * **Anchor-survival across a real refactor (`6c`): partly.** The tier ladder is tested —
>   `the_node_tier_follows_a_construct_that_moved`, `a_rename_falls_off_the_node_tier`,
>   `a_node_tier_miss_still_lands_on_the_line_tier` — and a property test carries an anchor through
>   an insertion. What does not exist is the `6c`-shaped end-to-end case: an anchor followed
>   through a rewrite in the running editor. Recorded as owed rather than ticked.
> * **`1a`, `2a`, `3d`, `8a`, `6a` snapshots: the captures exist, the *snapshot tests* do not.**
>   `crates/phosphor/tests/snapshots/` holds `3c`, `6b`, `6d`, `7c` and `8e`; these five screens
>   are covered by VHS and by nothing at Tier 1. That is a real gap and it is the second thing
>   this checkpoint still owes.
>
> **The VHS half.** `2a`, `3d`, `8a`, `6a`, `seen-cleared` and `no-grammar` are captured and match.
> The clip of `s` clearing a marker is `seen-cleared`, and it presses **`SPC u s`** rather than
> `s`: the shipped keymap binds bare `s` to vim's substitute, and `CP-5`'s wording predates that
> ruling. **`1a` against the `V006` fixture is still owed** — `tapes/1a.tape` opens
> `tapes/fixtures/core-lib.rs`, the frozen file §40 repointed it at, not the seeded tree, so the
> flagship screen is the one screen here drawn without a store behind it.
>
> **Two frames of forty-eight do not match**, and neither is drift: `6b` is a blessed change
> (`unseen-regions` answered `T041`'s deferral until `T041` shipped; it answers `()` now, and the
> tape's sentinel waited ten seconds for a word the editor had stopped saying), and `broken-init`
> is `OPEN-QUESTIONS.md` §42 — it photographs the boot layer's own form count, so any Scheme form
> anybody adds moves it.

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
  40 columns. **And `apply-workspace-edit` applies**, which is the first arm this queue owes to a
  task that is not its own. *Needs:* T059, T041

  > **`S4` made this task a creditor, and it is written here so the creditor knows.** `T036` built
  > the reading half of a server's rename — `phosphor_buffer::lsp::file_edits_from_lsp` turns a
  > `WorkspaceEdit` into `Vec<FileEdits>`, and it is tested — and the applying half is blocked on
  > this queue and nothing else in the near term. `apply-workspace-edit` is **the one `Lsp`
  > capability rated `Ask`**, and the binary already declines it by name: *"lsp: needs an ask
  > first — T060 builds the queue"*. There is nowhere to put the question, so the only arm
  > buildable today is one that skips asking, which is worse than the refusal.
  >
  > It is RECORDED against this task in `scripts/lint-action-arms.sh`, so the lint names it on
  > every run until the arm exists. That is deliberate and it is the *opposite* of what happened
  > to `request-references`, which was re-homed to `T047` and thereby fell out of the ticked
  > filter that produces a recorded row at all — no entry, no line in the summary, nothing that
  > expires. Two debts from one window, tracked two different ways, and only one of them is
  > self-enforcing.
  >
  > **A second blocker exists and is the further one:** a `WorkspaceEdit` edits files that are
  > *not open*, and nothing in this build can hold a buffer that is not on screen. The RECORDED
  > entry names `T088` for it, which is a **reading rather than a citation** — `T088` is splits
  > and focus, and "a buffer the user is not looking at" is adjacent to that rather than inside
  > it. Named here so that closing this queue and finding the arm still impossible is not a
  > surprise, and so that whoever builds `T088` sees the claim made on their behalf.

- [ ] **T061 · Permission asks + rule writing**
  Screen `7a`: exact invocation shown; always-allow **writes a legible rule**.
  *Done when:* the written rule is readable by a human and takes effect next time. *Needs:*
  T059

  > **Two corrections from `T101`, both of which change what this task has to do.**
  >
  > **Where the rule lands.** This entry said `init.scm` until 2026-08-14, and `7a` still draws
  > that word. `T101` moved machine-written forms out of the shipped tree entirely — they go to
  > `$XDG_CONFIG_HOME/phosphor/persisted.scm` now, and `runtime/persisted.scm` is deleted. The
  > write path already works and is tested:
  > `a_head_the_layer_never_offered_is_written_as_given` puts `(allow "git push")` through
  > `persist-form!` with the shipped policy loaded and reads it back **ungated**, which is what
  > `7a`'s *"pressed a digit"* earns — the explicit-persist gate is on the REPL's auto-route, not
  > on this.
  >
  > **The read half faults today, and that is this task's constraint rather than `T101`'s bug**
  > ([OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §35). Run this session:
  > `phosphor --eval '(allow "git push")'` answers
  > `#raised · unbound identifier — Cannot reference an identifier before its definition: allow`.
  > `allow` is free until this task builds `runtime/permissions.scm`, and `Layer::load_persisted`
  > runs each persisted form and records a fault the boot float draws. So a grant written before
  > this task exists would open a boot float on every start. Nothing writes one yet — the
  > permission surface *is* this task — so it is forward-looking rather than live.
  >
  > **What to check when this lands:** that `allow` is defined by a file in
  > `phosphor/boot-files`, and not somewhere the boot reaches later. `Layer::load_persisted`
  > already guarantees "after the whole load order" for anything in that list, so satisfying the
  > constraint is a matter of *where* the definition goes rather than of new machinery.

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

- [x] **T093 · Floats from the doors** 📌
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

  > **The blocker was never the slot.** `open-float` takes a `SurfaceId` documented as *"a
  > registry key, not a Rust enum"*, and **nothing created an entry and no verb could** — the
  > whole vocabulary had two `define-*` capabilities and neither was a surface. Found by the
  > pre-window scout, ruled as [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §43, built here.
  >
  > `define-float-surface` is the missing half and is deliberately `define-picker-source`'s shape:
  > **an id and a `String` of scheme**, because no `SteelVal` may ride in a payload. The layer
  > binds a procedure under `phosphor/float-surface/<id>`; the host calls it and decodes a
  > `view::Float`. So a surface is exactly as live as a `define-language` — redefine it at the
  > REPL and the next `open-float` gets the new one — and it adds **zero lines to `phosphor-ui`**,
  > which is `T048`'s acceptance criterion rehearsed one task early.
  >
  > **The id is validated at the door**, not in the layer: it is interpolated into a `define`
  > form and the capability is `Allow` on MCP, so an unchecked one is scheme injection from an
  > agent. `a_surface_id_that_is_not_a_name_is_refused` plants exactly that.
  >
  > All four arms post an `Intent` rather than acting, because composing a surface runs scheme and
  > a binding is already inside the VM when it calls. A door-opened float gets its own
  > `Surface::Float` rather than borrowing `:help`'s — the drawing is identical, but one is
  > composed by the host from the live keymap and the other by `runtime/*.scm` from whatever it
  > likes, and collapsing them would make the first Steel surface indistinguishable from the one
  > Rust surface that is not a fixture.
  >
  > **Composed once, at open** — `:help`'s shape, not `define-picker-source`'s *"an open picker
  > re-derives"*. A float is a snapshot of an answer; a picker is a live query. Flagged rather
  > than assumed: if a surface ever needs to re-derive, that is a change to this arm and not a
  > property anyone should expect today.
  >
  > **Scope**
  > - Files: `crates/phosphor-core/src/action.rs`, `crates/phosphor-steel/src/{float,view}.rs`,
  >   `crates/phosphor/src/main.rs`, `crates/phosphor/tests/loop_pty.rs`,
  >   `crates/phosphor-core/tests/surfaces.txt`, `scripts/lint-action-arms.sh`
  > - Named units: 1 new capability (216 → **218**, 648 → **654** door checks), 4 door arms,
  >   4 `Intent` variants, `Surface::Float`, `view::float`, `float::{surface, define_form,
  >   valid_surface_id, SurfaceError}`, 2 pty tests
  > - Verification: `just gate` green; the float composes from scheme with no Rust knowing its
  >   words, and `esc` closes it (§9)
  > - Risk: public API change yes (one capability) · data migration no · cross-module yes ·
  >   reversible yes · external blocker no

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

- [x] **T100 · The door speaks §6's voice**
  The two halves of one defect, ruled in this window to be one task because they rewrite the same
  expectation set and doing them separately means regenerating and reviewing it twice. Recorded
  at [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §7 and §9:
  **(1)** there is no `Outcome` case for *"it ran and raised"*, so a refused query surfaces the
  `Error: Kind:` envelope, which is Steel's and not Design Language §6's voice; and
  **(2)** `door.rs::why` and `answer::why` phrase one enum two ways — *"T041 builds this"* against
  *"not built yet — T041 builds it"*.
  > **Scope**
  > - Files: `crates/phosphor-core/src/action.rs`, `crates/phosphor-steel/src/{runtime,answer,
  >   registry,repl}.rs`, `crates/phosphor/src/{door,main}.rs`,
  >   `crates/phosphor/tests/{door,parity}.rs`, `crates/phosphor-steel/tests/{screen_6b,
  >   shipped_languages}.rs`, both `screen_6b` snapshots
  > - Named units: 1 enum case (`Outcome::Raised`) + 1 struct (`Raised`) with `why`,
  >   2 converters (`runtime::raised`, `runtime::kind_of`), 1 shared reduction
  >   (`answer::trouble`) with 3 call sites, 1 sigil (`answer::RAISED`), 6 tests
  > - Verification: the parity suite, the two `6b` golden frames, five planted mutations
  > - Risk: public API yes (one `Outcome` case) · data migration no · cross-module yes
  >   (`phosphor-core`, `phosphor-steel`, `phosphor`) · reversible yes · external blocker no
  *Done when:* a query that ran and raised is a distinct `Outcome` from one that was refused, the
  two `why` implementations produce one sentence per enum value, and every door check passes
  against the regenerated expectations. *Needs:* T020, T024

  > **Half (2) was already closed when this ran, and the tree said so.** `5050b58` (*"the CP-3
  > repairs — a lint for node kinds, three verbs, and one voice"*) moved the phrasing onto the
  > enum as `Refusal::why` and deleted `door.rs`'s copy; `answer::why` is a one-line delegate to
  > it. Read this session, both files. What was left of (2) was a *test* that could no longer
  > fail — `the_cli_door_and_the_repl_phrase_one_enum_one_way` compared `door::render` with
  > `answer::line`, and this task made `render` **call** `answer::line`, so the comparison became
  > a function against itself. It is replaced by
  > `one_enum_value_is_one_sentence_and_this_is_the_sentence`, which writes the seven sentences
  > out; `every_refusal`'s own exhaustive `match` is what stops the table going stale. A pair that
  > agree are still free to agree on the wrong words.
  >
  > **Half (1), measured against the built binary before and after.** The reported symptom and
  > two more found by running every raise shape:
  >
  > ```text
  > phosphor --eval '(unseen-regions "src/main.rs")'
  > -  #refused · Error: Generic: not built yet — T041 builds it
  > +  #raised · not built yet — T041 builds it
  > phosphor --eval '(car 5)'
  > -  #refused · Error: TypeMismatch: car expected a list or pair, found: 5
  > +  #raised · wrong type — car expected a list or pair, found: 5
  > ```
  >
  > `#raised` is a third `Outcome` case and not a `Refusal` variant, because nothing declined
  > anything — the request was well formed and the evaluator ran. `Runtime::evaluate` is its one
  > producer; `runtime::raised` takes Steel's envelope off by **reconstructing** it from
  > `SteelErr::kind` rather than matching a pattern, so a Steel upgrade that changed `Display`
  > reddens `an_envelope_that_stopped_matching_is_caught` instead of leaking. `kind_of` is a total
  > match over `steel::rerrs::ErrorKind` — a new kind is a compile error, not a `TypeMismatch`
  > reaching a reader — and answers `None` for `Generic`, which is the envelope this build's own
  > `registry.rs` puts around a `QueryError` that is already in §6's voice. That is why the
  > reported line comes back unwrapped rather than as *"generic — not built yet…"*.
  >
  > **`6b`'s golden frame had the envelope blessed into it.** Both snapshots carried
  > `⇒ #refused · Error: Generic: not built yet — T041 builds it` and
  > `⇒ #refused · Error: FreeIdentifier: …` — a design-conformance capture of Steel's voice.
  > Re-blessed; the diff is three lines and the style map shifting one column.
  >
  > **What adding the case found that reading could not.** Two of the binary's three
  > outcome-to-notice reductions became compile errors and the third did not: `Intent::Keymap`
  > was an `if let Outcome::Refused(…)`, so a keymap form arriving from the CLI or MCP door and
  > *raising* went nowhere and said nothing. All three go through `answer::trouble` now, which is
  > one `match` that cannot be exhaustive in one place and lossy in another.

  > **It collides with §26, and the pre-`S4` scout found it by reading the two scopes together.**
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §26 wants `crates/phosphor/tests/parity.rs` split per
  > door, because one test function in it takes 176 s of a 182 s suite. That is the same file this
  > task's scope names, and [TEAM.md](TEAM.md) schedules this task at the **front of Window E**
  > precisely so *nothing else is rewriting the parity expectations* while it runs. Under rule 1
  > of *Concurrency* the two cannot be concurrent.
  >
  > So it is one of two things, and picking is cheaper than discovering: either the split lands
  > **before `S4` opens**, or it is **folded into this task's agent** as the first thing it does.
  > The second is defensible — regenerating the expectation set and splitting the harness that
  > walks it are the same sitting — but it must be *said*, because the default (two agents, one
  > file, one window) is the failure rule 1 exists for.
  >
  > **Decided by events, and recorded here so it stops reading as a live choice: option one.** The
  > split landed **before `S4` opened**, in `5017293` (*"the six things that had to be true before
  > S4 opens"*, 2026-08-13), one day before this window's three commits.
  > `crates/phosphor/tests/parity.rs` now carries `every_capability_is_reachable_at_the_steel_door`,
  > `…_at_the_mcp_door` and `…_at_the_cli_door` where one function stood, and
  > `every_capability_is_reachable_at_every_door` no longer exists — read this session. So this
  > task's agent inherits a file already split, and its scope is the `Outcome` case and the two
  > `why` implementations alone.
  >
  > **What the split then found is why this mattered more than the scheduling.** §26's outcome
  > block has it: the guess this whole collision was argued around — that the Steel door owned the
  > 176 s — was wrong by two orders of magnitude, and the CLI door's 212 process spawns owned it.
  > Had the split been folded into this task, that measurement would have arrived in Window E,
  > after every agent in `S4` had already paid the floor once per gate.

**Two verbs were re-homed rather than added**, which is worth recording because a wrong phase on a
capability row is what put them on `lint-action-arms.sh`'s creditor list in the first place:
`apply-edits` moved from `S3 / T029` to `S6 / T052` and `jump` from `S3 / T026` to `S5 / T042`,
matching the tasks [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §18 ruling had already named as their
creditors. `place-anchor` is the third and it is genuinely new: `goto-anchor` named an `AnchorId`
that nothing produced and there was no setter at all, which is why `m` is bound to silence.

**One debt still has no creditor, and the lint says so out loud rather than passing quietly.**
It gets no task here, because inventing one would be inventing product work rather than recording
debt — but a reader looking for what this window did *not* close should find it named:

> **This paragraph said *two* until the pre-`S4` scout, and the numbers under it were three
> versions stale.** It quoted `scripts/lint-action-arms.sh` at *13 recorded gaps (1 with no task
> that closes them)* and named `ApplyEdits` as the one; the lint reports **11 recorded gaps (0
> with no task that closes them)** as of 2026-08-13. The fix was also not the one predicted here.
> This text proposed filling the RECORDED table's empty blocking-task field — a `scripts/` edit —
> and what actually happened is better: `Jump` and `ApplyEdits` were **removed from the table
> entirely**, because re-declaring their capability rows against unticked tasks (`jump` → `T042`,
> `apply-edits` → `T052`) took them out of the ticked filter that puts an entry there at all. The
> note at `scripts/lint-action-arms.sh`'s RECORDED table records the reasoning: neither was ever a
> missing arm, the attribution was the bug. **Nothing recomputes the numbers in this paragraph** —
> `scripts/doc_claims.py` checks capability, parity, task, wave and lint counts, not quoted lint
> output — which is exactly why it drifted, and is worth knowing before quoting a lint in prose
> again.

- `scripts/lint-node-kinds.sh` reports *30 node kinds, 18 composed by the shipped configuration,
  12 recorded gaps (1 with no task that closes them)* — run 2026-08-14, after `S4` — and the one
  with no task is `Node::Gutter`: the state
  column **without** an editor around it, for a surface that wants it. `T031` built the column and
  `BufferView` ships it as its left column; no task in the graph names a surface that would
  compose the standalone kind. This is the same root as R3 in
  [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md), which asks how a terminal capability should reach that
  kind's arm — a question about composing something nothing composes.

That lint is also why the window ran before `S4` rather than after: `Completion` and
`Signature` were in its recorded gaps when it was written, against `T038` and `T039`. So the day
`S4` built those two widgets and composed neither, the lint would be what said so — instead of a
golden frame passing on a hand-built tree while the running binary draws nothing, which is the
shape this build had by then repeated twice.

> **It came out the other way, which is the outcome the bet was for.** `S4`'s wiring pass composes
> both in `crates/phosphor/src/main.rs` (`passive_float`), and **both entries are gone from the
> RECORDED table** — 16 composed became 18, 14 recorded gaps became 12. The lint never fired,
> because the thing it was written to catch did not happen: the window that built the widgets is
> the window that composed them. A gap that closes by being closed rather than by being waived is
> the only evidence this kind of lint can produce, and the file's own header now records the
> outcome next to the bet.

---

## C · The repair window between `CP-4` and Window E

`CP-4` is not passed — its manual half is Teej's. This window ran on work collected because it
gets **more expensive if Window E runs first**, which is the test §B applied to the window before
`S4`. Three items needed tasks: one is a change to a vendored fork and those are permanent, one
changes what a mockup draws, which is Teej's to amend and not an agent's to fold in, and the third
was found *by* running one of the window's own tasks and is behaviour rather than the voice that
task was scoped to.

Numbered `T101`+ and appended rather than renumbered, like `T099` and `T100` before them.

- [x] **T101 · Explicit persist, and a real config home**
  Two changes ruled together by Teej on 2026-08-14, after reading how Emacs handles the same
  problem. Both are recorded at [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §32.
  **(1) Persistence becomes explicit.** `runtime/repl.scm` listed eight heads and *any* form with
  one of them was written to disk **for having been evaluated** — try a theme, keep it forever.
  That is neither of Emacs's two mechanisms (`M-:` and `ielm` never persist; `M-x customize` is a
  deliberate *save this*) and it sits badly against the third invariant, *nothing moves unless you
  asked*. The mechanism is kept and the automatic is gone: `persist!` is the verb, and it is an
  **identity function** — the REPL is still the only thing that writes, so a `(persist! …)` read
  back at boot evaluates its argument and appends nothing. A bare config verb answers `⇒ #ok ·
  not persisted — (persist! …) keeps it`, which is `6b`'s receipt offering the
  verb at the moment you would want it. **This contradicts `6b`**, which draws a bare
  `(keymap-set! …)` answering `· persisted to init.scm`; the drawing is Teej's to amend at
  claude.ai and `docs/design/` was not touched. **`7a`'s always-allow is untouched by
  construction**: the gate reads the heads the layer *listed*, and a head it never listed —
  `(allow "git push")` — is written as given, because pressing a digit was already the explicit
  act.
  **(2) A real config home.** `phosphor/persist-file` was a bare name joined to the runtime root,
  and in a dev checkout that root is the repository: `CP-4`'s manual test left a
  `(define-language! "lua" …)` in the tracked `runtime/persisted.scm`. It is
  `$XDG_CONFIG_HOME/phosphor/` now — **config rather than state**, because the file's own header
  promises *"it is yours to edit"* and a binding you kept belongs with your dotfiles, and because
  there is nothing per-project about a keymap. `crates/phosphor-core/src/config.rs` is the
  sibling of `journal.rs`'s `state_home` and follows it; the root-hashing that is right for undo
  would be wrong here. `runtime/persisted.scm` left the tree and left `phosphor/boot-files` with
  it, so *"loads last"* is now a call site (`Layer::load_persisted`) rather than a position in a
  list somebody can reorder — and a fault in it still reaches the boot float. The old refusal
  moved rather than being deleted: *"no config home to write to — set $XDG_CONFIG_HOME or
  $HOME"*, because *nowhere to write* is still reachable on a CI runner and inventing a path
  would be worse.
  **Two phosphors both appending is an ordinary case**, and the write is one `write_all` of one
  buffer rather than a `writeln!`: `write_fmt` may issue a syscall per format piece, and
  `O_APPEND` promises atomicity per `write`, not per macro.
  > **Scope**
  > - Files: `crates/phosphor-core/src/config.rs` (+185/-0), `crates/phosphor/src/main.rs`,
  >   `crates/phosphor/tests/loop_pty.rs`, `crates/phosphor/benches/vm_invocations.rs`,
  >   `runtime/repl.scm`, `runtime/init.scm`, `runtime/README.md`, `runtime/persisted.scm`
  >   (deleted)
  > - Named units: 1 new module (4 public functions, 1 error), `AppHost::persist` gated,
  >   `AppHost::persist_policy` / `persist_target`, `Layer::load_persisted`, 2 new Steel globals
  >   (`phosphor/persist-verb`, `phosphor/offered-heads`), 1 new Steel verb (`persist!`),
  >   6 unit tests, 6 path tests, 2 pty tests
  > - Verification: `cargo nextest -p phosphor-core -p phosphor`, four planted mutations
  >   (`writeln!` for `write_all`, the gate disabled, `load_persisted` made a no-op, the offered
  >   list misspelled) — each named a test that went red
  > - Risk: public API yes (one new `phosphor-core` module) · data migration **yes, one-way** —
  >   an existing `runtime/persisted.scm` is no longer read; a user moves it to
  >   `$XDG_CONFIG_HOME/phosphor/` · cross-module yes (`phosphor-core`, `phosphor`, `runtime/`)
  >   · reversible yes · external blocker no
  **No new capability**, and the counts do not move: `persist!` is a Steel identity function in
  the editor layer, not an `Action`. `persist-form` is the capability and it already existed.
  *Done when:* a form evaluated at the REPL does not persist, the marked form does, it survives a
  restart of the binary, `7a`'s rule is written as given, and nothing is written into the tree
  that booted. Held by `the_repl_keeps_what_the_verb_marks_and_offers_the_rest` and
  `a_form_kept_at_the_repl_survives_a_restart_of_the_binary`
  (`crates/phosphor/tests/loop_pty.rs`), `a_form_is_kept_only_when_the_verb_marks_it`,
  `a_head_the_layer_never_offered_is_written_as_given`,
  `a_form_is_appended_whole_when_several_writers_race`,
  `a_persisted_rebind_survives_the_next_boot` and
  `a_broken_persisted_form_costs_one_line_and_reaches_the_boot_float`
  (`crates/phosphor/src/main.rs`), and the six in `phosphor_core::config`.
  *Needs:* T021, T022, T030

- [x] **T102 · The undo crash in the vendored editor**
  Found by the agent proving the `:e`-on-a-new-path fix and carried rather than fixed, because
  only `surface` may write `vendor/` and the workaround at our call site costs a tree-sitter
  reparse per edit on batched operators. **It was live in the shipping binary**: open a file, type
  two characters at the end, press `u` — `exit=101`, a `ropey` char-index panic reached through
  `Editor::apply_batch` → `Code::commit` → `Code::notify_changes`, reproduced through a pty before
  anything was changed.
  The defect is upstream's and so is the fix. `notify_changes` turned each edit's offset into a
  `(row, col)` at **commit** time, against the rope as it stood once the whole batch had been
  applied. Inverting a change reverses the edit order, so an undo step reports its highest offset
  first against the shortest the text will ever be. `Code::insert` and `Code::remove` now record
  the change event before they touch the rope, which also fixes the half that does not crash:
  a descending batch that stays in range still reported the wrong column. That half is **latent
  here and live upstream** — `track_dirty` takes `|_|` and keeps only a flag and a counter, and
  `T038`'s `didChange` carries the whole document, so no wrong range has left this editor.
  **The second half of this task is that nothing ran the fork's tests.** `[workspace] exclude`
  keeps both forks out of `cargo nextest run --workspace`, so thirty-two upstream tests and nine
  phosphor patches had never met a runner here — which is how a panic on `u` reached a user past a
  green `just gate`. `scripts/lint-vendor-tests.sh` closes it for the editor fork; the markdown
  fork cannot be tested standalone at all and its `VENDOR.md` now records why.
  > **Scope**
  > - Files: `vendor/ratatui-code-editor/src/code.rs` (+30/-24),
  >   `vendor/ratatui-code-editor/tests/change_events.rs` (+156/-0, new),
  >   `vendor/ratatui-code-editor/VENDOR.md` (§10), `vendor/ratatui-markdown/VENDOR.md`,
  >   `scripts/lint-vendor-tests.sh` (new)
  > - Named units: 1 field (`Code::batch_changes`), 3 methods touched (`tx`, `insert`, `remove`),
  >   1 rewritten (`notify_changes`), 5 tests, 1 lint
  > - Verification: the fork's own suite through `scripts/lint-vendor-tests.sh` inside
  >   `just lint`; the pty repro against the built binary, before and after
  > - Risk: public API no (the field is private and `notify_changes` is private) · data migration
  >   no · cross-module no · reversible yes · external blocker no
  *Done when:* typing two or more characters and undoing them does not panic in the running
  binary, the change events a batch reports are the positions its edits had when they were
  applied, the regression tests fail on the unpatched crate, and `just gate` runs them.
  *Needs:* T003, T029

- [ ] **T103 · The CLI verb route dispatches to the host** 📌
  Found by `T100` and **deliberately not folded into it**, with the reasoning at
  [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §33. `T100` was scoped to the door's voice; this is the
  behaviour underneath one of the two sentences it was sent to fix, and no wording repairs it.
  One binary, one process, two answers for one capability — run against the built binary this
  session:
  ```text
  phosphor open-repl                #refused · not built yet — T022 builds it   exit 1
  phosphor --eval '(open-repl!)'    #ok                                         exit 0
  ```
  `door.rs::apply` is an `S2` stub that refuses every Action but `Eval`, and it predates `T022`
  wiring a real host in — `main.rs`'s `dispatch` builds that host on the verb path too and then
  never asks it anything. The consequence a user meets is `phosphor set-case --target cursor
  --case upper` answering *not built yet — T026 builds it* about a ticked task with a live arm
  and working keys; **56 action rows** are in that position.
  **Rule the side effect before writing the code.** With the verb route dispatching, `phosphor
  persist-form --form '(…)'` writes to the user's `init.scm`, and `crates/phosphor/tests/parity.rs`
  runs every verb with its canonical example — so `just gate` would append to a real config home.
  That is a `T101` question (which home) and a test-isolation question at once.
  **Carry the parity sharpening either way:** `cli_door` expects `#refused · not built yet —
  {task} builds it`, and 21 rows share `T026`, so a verb dispatching to a neighbour with the same
  task passes today. The capability *name* is unique per row and equally derived.
  > **Scope**
  > - Files: `crates/phosphor/src/door.rs`, `crates/phosphor/src/main.rs`,
  >   `crates/phosphor/tests/parity.rs`
  > - Named units: 1 function (`door::apply`), 1 trait (`door::Evaluate`, which may widen or go),
  >   `cli_door`'s expectation, `main::dispatch`'s verb branch
  > - Verification: the parity suite regenerated; a process test that `phosphor <verb>` and
  >   `phosphor --eval '(<verb> …)'` answer the same thing for a capability the host carries out
  > - Risk: public API no · data migration no · cross-module no · reversible yes · external
  >   blocker no — but **writes to the user's config home** unless the side effect is ruled first
  *Done when:* one capability the host carries out answers the same through both CLI routes, no
  verb claims a ticked-and-armed capability is unbuilt for a reason the door invented, the parity
  walk discriminates by capability name rather than by a shared task id, and no gate run writes to
  a real config home. *Needs:* T023, T024, T101

---

## D · What `CP-4`'s manual half found — five tasks from Teej at the keyboard

`CP-4`'s manual half is the one only Teej can run, and this is what running it produced. Section
`C` above collected work that gets *more expensive if Window E runs first*; these are different —
they are things a person met by typing into the shipping binary, which is the entire reason a
checkpoint has a half no gate can perform. Two of them (`T105`, `T106`) were being built while
this section was being written and one (`T108`) is deliberately not buildable yet.

Numbered `T104`+ and appended rather than renumbered, like `T099`–`T103` before them. **📌 marks
the two that are new surfaces rather than repairs**: `T107` and `T108` name things no task in the
graph names at all, while `T104` extends `T026`'s operator machine and `T033`'s keymap and
`T105`/`T106` extend a ticked `T038`.

> **A second sitting, 2026-08-16, and it produced no new tasks — which is itself the finding.**
> Teej ran the binary again after `T104`–`T107` landed and reported five things. Four of them
> resolved into work or rulings that already had a home, and **not one needed a new task row**:
>
> * **`<tab>` should drive the completion list.** §38 re-ruled by its first option; `T105` ticked.
>   The report's other half — *"enter or space doesnt accept"* — was the same fact, not a second
>   bug: nothing had been chosen, so the `select = false` guard held correctly and what was
>   missing was a key to choose *with*.
> * **Completion felt slow.** §29 item 3, ruled *annoying*. There was no timer at all;
>   `COMPLETION_DEBOUNCE` is 250ms, helix's number. The symptom was not latency — with
>   one-in-flight as the only gate the list never caught up to the word being typed.
> * **`gr` does nothing.** Working as designed and **not fixable in this window**:
>   `request-references` is `[S5 / T047 / Deny]`, it declines out loud on the statusline
>   (`loop_pty.rs`, *"not built yet — T047 builds it"*), and the chain to make it real is
>   `T047 → T046 → T045 → T041` — the whole S5 spine, which is Window E and does not open until
>   `CP-4` passes. What Teej remembered fixing was the keymap rewrite that made the key *speak*.
> * **`taplo ✗ could not start` on `Cargo.toml`.** Not a defect: the binary is not installed on
>   the machine, and the chip saying so is
>   `a_server_that_cannot_start_says_so_on_the_statusline` working. Teej is installing it.
>
> The fifth was `T106`'s, and it is **ruled**: Teej, 2026-08-17 — *"add the columns to `7c`"*. The
> drawing gains `kind` and `source`; the build loses nothing. Recorded as the sixteenth entry in
> [README.md](README.md)'s amendment list, which is where a pending-upstream design change lives,
> because `docs/design/*.dc.html` is imported verbatim and is Teej's to edit at claude.ai.
>
> **The push half was re-checked rather than assumed**, since it decides whether an agent can
> carry this out: `DesignSync`'s `get_project` on `9234741f-228d-4014-9e3c-aea1475f8270` answers
> `type: PROJECT_TYPE_PROJECT` with `canEdit: true`, and the tool's own contract is that only a
> `PROJECT_TYPE_DESIGN_SYSTEM` accepts a write and *"that type is immutable at creation"*. So
> `canEdit` is not the gate and the edit stays a hand edit. README's claim of 2026-08-13 still
> holds.
>
> **What this settles downstream:** the three `7c-{rust,python,typescript}` Tier-2 captures
> already draw all four columns, and `OPEN-QUESTIONS.md` recorded that as *"whether the build
> should draw them is §D's open ruling"*. It is not open now — the references are correct.

> **A third sitting, the same day, one finding — and it is worth more than the four above.**
> Teej, on a half-typed `path:` in `main.rs`: *"we have to do better with these error msgs"*.
> Eleven cascade parse errors from rust-analyzer, each drawn as its own row, the code being
> edited pushed off the bottom of the screen. The fix and its reasoning are recorded at `T040`,
> which it does **not** tick.
>
> **The part to carry forward is what the investigation found rather than what the report said.**
> The unbounded rows were the visible half. The invisible half is that screen `2b` draws a
> statusline diagnostic count — `■ 1`, beside `1 thread · 2 unseen` — and **nothing in the build
> had ever computed it**. That absence is *why* the rows had to be unbounded to stay truthful:
> with no count, the only way the editor could say *"there are eleven"* was to draw eleven.
>
> That is the same shape as `T016`'s folds and the four dead `S3` surfaces, one layer over:
> **a design element no task's acceptance criterion happened to name, shipped absent rather than
> broken.** `T040`'s criterion is about gutter *priority*; nothing in it says *count*, so nothing
> was ever red and no lint could have been. What found it was a person typing — and what made it
> findable in one pass was **grepping the mockups for the glyph** rather than re-reading the
> task. `■` occurs twice in 37 screens, and the second occurrence was the whole finding.
>
> **And the sitting above shipped a red lint, which is worth recording where it happened.** The
> paragraph introducing it cited a task number one past the highest that exists, in a sentence
> saying no new task was needed — and `lint-doc-claims.py`'s *"every `T0xx` cited must be a task
> that exists"* names exactly that. It reached `master` because the docs were edited **after**
> that window's `just gate` run and committed without a second one: the gate was green when it
> was read, and the claim it would have caught was written afterwards. `CLAUDE.md` calls
> `just gate` *"the command to run before saying something is green"*; the gap is that it ran
> before the last edit rather than before the commit, and a green gate three edits old is a green
> gate for a tree that no longer exists. Caught one commit later, by the lint, on this window's
> run.
>
> **The correction could not be written in the obvious words**, which is the lint being stricter
> than it looks: naming the offending id here — even inside backticks, even to say it is wrong —
> is itself a citation of a task that does not exist, and the second `just lint` failed on this
> very paragraph. Hence the circumlocution. Worth knowing before someone spends a run on it.

> **The review of this section found a defect older than any of it, and four the section caused**
> — the last two by the review *of* the fixes, which is the same seam catching the same shape a
> second time. Recorded here rather than as tasks, because they are fixes rather than work: adding
> rows to this graph moves counts `scripts/lint-doc-claims.py` recomputes, and none of these is a
> surface.
>
> * **A literal `<` made Rust untypeable, and it predates `S4`.** `phosphor/prefix?` in
>   `runtime/keymaps.scm` compared canonical key spellings with `starts-with?` — on *characters*.
>   A canonical spelling is a concatenation of keys and the character `<` spells itself, so `<` was
>   a prefix of `<space>`, `<esc>`, `<C-x>` and every other bracketed row in its scope. The machine
>   answered `Pending`, held the key, and flushed the batch as text when the sequence died — with
>   every Action in the batch built against one stale cursor, because the host applies them and the
>   machine cannot. Typed into the running binary, `a<u8>b` wrote `a8>bu<` and
>   `let v: Vec<u8> = Vec::new();` wrote `let v: Vec8> = Vec::new();u<`. Both halves are fixed —
>   `phosphor/boundary?` makes a prefix end where a key ends, and `Machine::insert_keys` walks the
>   position across a batch — and each half has a test that was watched going red on its own
>   mutation. Either fix alone rescues the string, which is why the pty test that reads it
>   (`a_rust_generic_types_forwards_in_insert_mode`) is the outcome check and not the isolating one.
> * **Enter stopped scrolling.** `T105` binds `<cr>` in the insert scope, so every newline typed in
>   insert mode is an `accept-completion`; `main.rs`'s `moves_cursor` did not name that Action, so
>   `Editing::apply` skipped the reveal. At 80x24, `A` then thirty `<cr>` left the viewport on lines
>   1..23 with the cursor on line 31 — you type where you cannot see. Every test in the tree passed,
>   because none of them pressed enter in insert mode past the last visible row.
> * **`R` stopped overwriting.** `Scope::of` folds `EditMode::Replace` into the insert scope (so
>   does vim's `:imap`), and the loop's completion trigger is gated on `EditMode::Insert` — so in
>   `R` there is never a float and `accept-completion`'s fall-through fires on every `<space>` and
>   `<cr>`. It spliced, so `R` was `i`: `abcdef` with `RXY Z` read `XY Zdef` where vim gives
>   `XY Zef`. `Editing` now keeps the mode the machine reports and the fall-through types the way
>   the mode types.
>
> * **`R` stopped overwriting a second time, for `<tab>`.** The fix above taught
>   `Editing::accept`'s fall-through to keep the mode, and `T104`'s new `insert-indent` — a
>   different function reached by the same folded scope — spliced unconditionally. So `R<tab>`
>   was `i<tab>`: `abcdefgh` became `    abcdefgh` where vim replaces one character and gives
>   `    bcdefgh`. Found by hand in the installed binary at the review, and the doc comment that
>   said *"vim's `R` does the same thing with `<Tab>`"* was the load-bearing claim — checked
>   against `nvim -u NONE` with `set expandtab tabstop=4 softtabstop=0` and false: `Rx<Tab>` over
>   `abcdefgh` gives `x···cdefgh`, one character eaten. `Editing::insert_indent` takes `accept`'s
>   own mode arm now, with the newline clamp that keeps `R<tab>` at the end of a line from
>   joining it to the next.
> * **`ZZ` told you to save and hid the way to do it.** Not a keymap row's meaning but a keymap
>   row's *shape*: `ZZ` is two Actions, `save-buffer` then `quit`, and on a buffer with no name
>   both refuse. The notice slot holds one sentence and `Session::key` kept the **last**, so the
>   editor said `unsaved work — force it or save first` and swallowed `no file name — :write
>   <path>` — the half that says what to type. `submit_ex` is a `find_map` and has always kept
>   the first, so `:wq` and `ZZ` were two doors onto one Action list answering differently. The
>   keystroke door keeps the first now.
>
> All five are the same shape and it is worth naming: **a keymap row changed the meaning of keys
> nobody was testing.** The machine-level tests drive their own transcribed table, the widget tests
> hand-build ViewModels, and `runtime/keymaps.scm` reaches the buffer only through a pty. That seam
> is where all five lived.

**One key is contested by two of them.** `T104` wants `<tab>` to mean *one indent level* and
`T105` wants it to mean *take this completion* — the same key, in the same scope, with the answer
depending on whether a list is open. `T105`'s mechanism (a fall-through argument on
`accept-completion`) settles `<space>` and `<CR>` and **does not settle this one**, because the
fall-through it takes is literal text and an indent level is a per-language value a keymap cannot
name. Neither task may decide it alone; it is [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §38.

> **Settled twice, and the second time is the one that shipped.** §38 was first ruled by its
> *third* option on 2026-08-15 — `<tab>` to `insert-indent`, completion keeping `<C-y>`,
> `<space>` and `<CR>` — and **re-ruled by its first option on 2026-08-16**, after Teej ran the
> binary and asked for the key back: *"in this form i should be able to hit tab or something to
> select"*. `move-completion` grew an `otherwise` that names a **capability** rather than
> carrying text, so `<tab>` steps the list while one is open and runs `insert-indent` when none
> is. **Both tasks got the key**, which is the outcome the paragraph above says was unavailable —
> and it was unavailable, until `T104` created the verb the argument had to name. `<S-tab>` steps
> backwards. Neither task's *done when* changed; both are met.

- [x] **T104 · `<tab>`, and what one indent level is**
  Reported at `CP-4`: *"tab only seems to go a space at a time when indenting"*. **Four things are
  true underneath that one sentence and not one of them is a missing binding**, so the report is
  worth taking apart before it is fixed.
  **(1) A tab is exactly one cell, and nothing in this build knows what a tabstop is.**
  `vendor/ratatui-code-editor/src/render.rs`'s `impl Widget for &Editor` walks a line's graphemes
  and draws each as `g.to_string().replace('\t', " ")`, advancing `x` by
  `code::grapheme_width_and_bytes_len` — which is `unicode_width::UnicodeWidthStr::width`, and
  `unicode-width` 0.2.2's `tables::width_in_str` answers `1` for every `c <= '\u{A0}'` that is not
  `\n` or `\r`. So a tab measures one column and paints one space, and *"a space at a time"* is
  literally what the renderer was asked to do. `tab_width`, `tabstop`, `shiftwidth`, `expandtab`
  and `softtabstop` have **zero occurrences** across `crates/`, `runtime/`, `vendor/` and
  `scripts/`, grepped this session.
  **(2) `<tab>` in insert mode is not unbound — it is decided, and the decision is a placeholder
  with a task id on it.** Three `keymap-set-rows!` blocks in `runtime/keymaps.scm` name the
  `insert` scope and at `e3af880` they carried eleven rows between them — `<esc>` in the all-scopes row, four
  arrows under the comment *"insert mode is text, with four exceptions that are not"*, and
  `T038`'s six LSP keys, which landed after that comment was written and make it read one binding
  set short. `<tab>` is in none of them, so it reaches `Machine::insert_key`
  (`crates/phosphor-core/src/input.rs`), which has an explicit arm for it:
  `key::Code::Named(key::Named::Tab) => Some("\t".to_owned())`, above a comment saying *"What a
  tab inserts is an option two reasonable users differ on, which makes it `T033`'s `set-option!`
  rather than a number invented here."* The literal tab is deliberate; the option it defers to was
  never built. (That comment names `T033`; the capability row is
  `SetOption = "set-option" [S2 / "T021" / Allow]` and the table is real —
  `runtime/init.scm` sets `soft-wrap` and `completion-min-chars` through it.)
  **(3) `Indent` is armed and bound, and it answers a different question.** `>` and `<` are
  operators in `runtime/keymaps.scm`'s `phosphor/operators`, bound in normal and
  operator-pending so `>>` and `>ap` are lookups rather than special cases;
  `phosphor_steel::keymap` maps `"indent"` to `Operator::Indent`, `Machine::operate`
  (`input.rs`) emits `BufferAction::Indent { target, delta }`, and `main.rs`'s arm calls
  `AppHost::indent`, which batches the whole range into one undo step. **Nothing presses it.**
  There is no test in `crates/phosphor/tests/` or `crates/phosphor-core/tests/` that types `>`,
  which is how the *unit* below stayed invisible through a green gate.
  **(4) The unit is the fork's, hardcoded, and keyed on the grammar name.** `AppHost::indent`
  takes `self.editor.code_ref().indent()` → `Code::indent` → `utils::indent(&self.lang)`, a
  `match` in `vendor/ratatui-code-editor/src/utils.rs`: four spaces for
  `rust|python|php|toml|c|cpp|zig|kotlin|erlang|html|sql`, a **literal tab** for `go|c_sharp`, two
  spaces for everything else. The name it matches is what `main::grammar_of` answered — the
  declaration's `grammar` field, or `"text"` — so of the twelve shipped declarations
  (`runtime/languages/`) rust, python, toml and html get four spaces and the other eight get two;
  `csv` declares `"grammar" void` and so does every undeclared file, both landing on `"text"` and
  two spaces; and the `\t` arm is **unreachable in the shipped configuration**, because nothing
  declares `go` or `c_sharp`. `define-language!` cannot influence any of it.
  **The question this entry may not answer, stated so nobody answers it by accident: spaces or
  tabs, and per-language or global.** `LanguageSpec` (`crates/phosphor-core/src/request.rs`)
  already carries `comment_prefix` — a per-language fact reached through `define-language!`, which
  its own doc calls *"the userspace road up from a second-tier language to a first-class one"* —
  so a per-language indent has an obvious home. Global is `set-option!`, which `insert_key`'s
  comment already points at. They are not exclusive: vim ships both (`shiftwidth`/`expandtab`
  global, `ftplugin` per filetype). Whichever is ruled, **the fork's table stops being the answer**
  and something has to replace it, because a value declared in scheme cannot reach `Code::indent`
  without either a fork patch or the host computing the unit itself and never asking.
  **Contested with `T105` on `<tab>`** — see §38.

  > **What landed, and the three rulings the entry above refused to make.**
  >
  > **Rendering is a fork patch, `VENDOR.md` §11, and it is upstream-shaped.** A tab's width is
  > `tab_width - (col % tab_width)` — a function of the column it starts at, which is why it
  > could not be folded into `grapheme_width` and why every measuring walk had to carry its
  > running column into `phosphor::tabs::cells`. There were **five**: `Code::char_col_to_visual`
  > and `Code::visual_to_char_col`, the renderer's grapheme loop, `Editor::cursor_from_mouse`,
  > and `soft_wrap::segments`. `Editor::get_visible_cursor` was a sixth and is now routed
  > through `char_col_to_visual` instead, so the column the cursor is *drawn* at cannot disagree
  > with the column every motion computes. The stop lives on `Code`, not `Editor`, because two of
  > the five walks are `Code`'s.
  >
  > **Spaces or tabs, and per-language or global: both, and the declaration wins.** Global is
  > `(set-option! "tab-width" 4)` and `(set-option! "expand-tab" #t)` in `init.scm`; per-language
  > is a fifth `define-language!` field, `indent`, holding **what one level is, literally** — a
  > string, because that says width *and* tabs-vs-spaces in one value and a number cannot say the
  > second. Precedence is vim's `ftplugin`-beats-`set`. `shiftwidth` is deliberately absent (one
  > unit answers `>`, `<` and `<tab>` here, which is what modern editors ship as a single *tab
  > size*); `softtabstop` is absent and is a **real gap** — `<bs>` still deletes one grapheme,
  > which is a `<bs>` behaviour and not this task's.
  >
  > **The fork's table stops being the answer, and no *declared* language's behaviour changed.**
  > The twelve shipped declarations reproduce `utils::indent`'s answers exactly — four spaces for
  > `rust`, `python`, `toml` and `html` (declared `void`, so the global), two for the other eight
  > — and `every_shipped_language_declares_the_indent_it_used_to_be_given` enumerates all twelve
  > rather than spot-checking. `Code::indent` now has no phosphor caller.
  >
  > **The thirteenth case is every other file, and it moved from two spaces to four.** This
  > sentence is the correction of the one above, which read *"no language's behaviour changed"*
  > and covered only the twelve — while the case it left out is the majority one. `utils::indent`'s
  > `_` arm gave **two spaces** to everything `grammar_of` answered `"text"` for, which is every
  > file no declaration claims: `.sh`, `.c`, `.go`, `.lua`, `.txt`, `.log`, and the scratch buffer
  > `T107` just made reachable. After the change `adopt` leaves `editing.language` `None` for those
  > and `indent_style` falls to `None => " ".repeat(tab_width)` — `init.scm`'s **four**. It is
  > deliberate and asserted rather than accidental (`the_shift_operator_shifts_by_the_unit_a_
  > declaration_named`: *"a buffer no declaration claims took init.scm's four"*), and it is
  > reversible in one line of scheme, `(set-option! "tab-width" 2)`. `csv` is the one that would
  > have drifted with them and does not, because it declares `"  "` rather than `void`.
  >
  > **§38 is ruled by its third option and the residue is named.** `<tab>` means one indent
  > level; completion keeps `<C-y>`, `<space>` and `<cr>`, which are the keys `CP-4` actually
  > asked for. Reversing it needs §38's *first* option — `otherwise` widening from *text to type*
  > to *a capability to run* — and that is now a smaller change than it was, because
  > `insert-indent` is the capability such an argument would name. See §38 for the whole ruling.
  >
  > **It was reversed on 2026-08-16, and the paragraph above is the reason it was cheap.** Teej
  > ran the shipped binary and asked for `<tab>` on the completion list; the reversal is exactly
  > the change this paragraph predicted, and it named the right capability. `<tab>` binds
  > `move-completion` with `"otherwise" (key/capability "insert-indent")` — stepping the list when
  > one is open, typing one indent level when none is — so **nothing in this task's own claim
  > changed**: the width still comes from `set-option!` and `define-language!`, `insert-indent`
  > still names no width, and `tab_in_insert_mode_advances_to_the_tabstop` and its CJK and
  > replace-mode neighbours pass untouched, because a buffer with no server has no list and takes
  > the fall-through on every press. What the reversal cost was one optional argument on one
  > capability, no new verb, and **no movement in the capability/door-check counts** — which is what makes an
  > argument the right shape for this and a second verb the wrong one.
  >
  > **Two things the review of this entry found, and neither was a gate failure.**
  >
  > *The rebuild went quadratic in one line's length.* `soft_wrap::segments` opened each segment
  > with `code.char_col_to_visual(line_idx, seg_start)` — a grapheme walk of the whole prefix,
  > once per segment — so a minified line paid for its own length again for every row it wrapped
  > to. `phosphor-ui/benches/soft_wrap.rs`'s second table is written to assert exactly that
  > property and read `1454x` between 5,000 short lines and one line of 400,000 (15.0 s per
  > rebuild against 11 ms), `B2: FAIL`, exit 101. **`just bench` is deliberately outside `gate`,
  > which is why *"`just gate` green — 1129 tests, 18 lints"* was true and missed it.** The
  > segments partition the line, so the fix is carrying the column the loop already computes:
  > `1.9x` and `B2: PASS`. `impl Widget for &Editor` had the smaller version of the same shape —
  > once per *drawn* row, bounded by screen height rather than by the line — and takes the same
  > carry, with the walk kept for the first row of a run because a viewport scrolled into the
  > middle of a wrapped line has no row above it.
  >
  > *A declared `indent` that says neither of the two things it is for was accepted.* The field's
  > argument is that one literal says width **and** tabs-vs-spaces; `IndentStyle::typed_at` and
  > `Editing::indent` then read a literal saying neither differently, so `" \t"` gave `>` a
  > space-tab and `<tab>` two spaces, `""` gave `>` a no-op and `<tab>` one space, `"\t\t"` gave
  > `>` two tabs and `<tab>` one, and a two-cell ideographic space measured one.
  > `Languages::declare` refuses all four now — one tab, or a run of spaces — beside the two
  > refusals it already owed, which is the door `runtime/languages/README.md` already says
  > validates a declaration rather than two call sites disagreeing behind it.
  >
  > **One defect found in passing and fixed, in a file this task does not own.**
  > `CompletionList::desired_width` did not count `DETAIL_MIN`, so a list whose widest detail is
  > **one cell** asked for three columns and `layout` then shed the detail at three. Latent since
  > the `CP-4` review added the floor to `layout` alone; found by
  > `nothing_is_shed_at_the_width_the_list_asked_for` during this task's gate run, which is
  > exactly the drift that property exists to catch. The failing seed is committed in
  > `completion_shed.proptest-regressions`.

  > **Scope**
  > - Files: `vendor/ratatui-code-editor/src/phosphor/tabs.rs` (+112/-0, new),
  >   `vendor/ratatui-code-editor/tests/tabs.rs` (+157/-0, new),
  >   `vendor/ratatui-code-editor/src/{code,editor,render,phosphor/soft_wrap,phosphor/mod}.rs`
  >   (+168/-40), `vendor/ratatui-code-editor/VENDOR.md` (§11),
  >   `crates/phosphor/src/main.rs` (+382/-13), `crates/phosphor-core/src/{action,request,language,input}.rs`
  >   (+65/-6), `crates/phosphor/tests/loop_pty.rs` (+233/-0),
  >   `crates/phosphor-steel/tests/shipped_languages.rs` (+62/-11),
  >   `crates/phosphor-ui/src/float.rs` (+15/-3), `runtime/init.scm`, `runtime/keymaps.scm`,
  >   all twelve `runtime/languages/*.scm` + both READMEs
  > - Named units: 1 capability (`insert-indent`), 1 `LanguageSpec` field (`indent`) with
  >   `Languages::indent`, 1 struct (`IndentStyle`) with `typed_at`/`width`, 1 resolver
  >   (`indent_style`), 1 method (`Editing::insert_indent`), 1 fork module
  >   (`phosphor::tabs`: `is_tab`, `stop`, `cells`), 2 fork accessors
  >   (`Editor::set_tab_width`, `Editor::tab_width`), 6 fork call sites, 1 keymap row,
  >   2 options, **25 tests**: 12 in the workspace (1117 → 1129) and 13 in the fork
  >   (`tests/tabs.rs` 8, `src/phosphor/tabs.rs` 5), which `just test` does not run and
  >   `scripts/lint-vendor-tests.sh` does
  > - Verification: `just gate` green — 1129 tests, 18 lints; **eight planted mutations**, each
  >   watched red against a named test — the render seam, `Code::char_col_to_visual`, the
  >   soft-wrap seam, `Editing::indent`'s unit source, `IndentStyle::typed_at`'s stop
  >   arithmetic, `Editing::insert_indent`'s column measured in chars rather than cells, the
  >   declaration lookup in `indent_style`, and the `<tab>` row deleted from `keymaps.scm`
  > - Risk: public API yes (a capability and a `LanguageSpec` field — both additive, both
  >   optional at the wire door) · data migration no · cross-module yes (`phosphor-core`
  >   vocabulary, the host, the fork, the layer) · reversible yes (the binding is one line of
  >   scheme; the options have documented defaults) · external blocker no
  *Done when:* a tab renders at a tabstop this build can name rather than in one cell, `<tab>` in
  insert mode inserts one indent level rather than one character, `>` and `<` shift by that same
  unit, the unit comes from something a user set rather than from `utils::indent`, and **a pty
  test presses `<tab>` and `>>` in the running binary** — an arm and a binding both already exist
  for `>` and neither has ever been pressed. *Needs:* T021, T026, T033

- [x] **T105 · The completion keys a hand already reaches for**
  Reported at `CP-4`: *"i like being able to hit space to select and put a space after or enter to
  select without a space after"*. The float raises itself while you type (`T038`) and then answers
  to five keys nobody's fingers go to first.
  **What the report was about**, at `e3af880`: the list is driven by `<C-x>` (request),
  `<C-n>`/`<C-p>` (step), `<C-y>` (accept) and `<C-e>` (dismiss), all in `runtime/keymaps.scm`'s
  third `insert` block. The keys a person actually presses do something else — `<up>`/`<down>`
  are bound in the same scope to the `line-up`/`line-down` motions, so they walk the cursor out
  from under a float anchored to the word being completed; `<CR>` reaches `Machine::insert_key`
  and inserts `"\n"`; `<tab>` reaches the same function and inserts `"\t"` (`T104`). **None of the
  three touches the session.**
  **Why it was never a keymap edit.** A binding is data: `input::table::Role`'s richest case is
  `Run(Vec<Action>)` — a fixed list of capabilities with their arguments baked in — and `Scope` is
  a five-value Rust enum derived from the edit mode by `Scope::of`. The thing that knows a list is
  open is `Editing::completion` in `crates/phosphor/src/main.rs`, and no binding can ask it. That
  is [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §29 item 1 arriving a second time, from the other
  side; §38 is the question it raises and the mechanism below is the answer that landed.

  > **Mid-flight as this was written, and recorded as such.** The working tree carried the change
  > below **uncommitted** while this entry was being written, `runtime/keymaps.scm` landing after
  > the capability did. Read the commit rather than this paragraph; what is here is the shape and
  > the reasoning, not the tick. **`<tab>` was still unbound** when this was last checked, which
  > is §38 and not an oversight.
  >
  > **The mechanism is neither of the two obvious ones — the capability grew the condition.**
  > `accept-completion` takes two new optional arguments (`crates/phosphor-core/src/action.rs`):
  > `then`, *"text to type after the accepted item — the space `<space>` leaves behind"*, and
  > `otherwise`, *"text to type when no row has been chosen; present is what makes a key fall
  > through instead of accepting"*. So `<space>` is bindable as *accept-if-steered, otherwise type
  > a space*, with the **text** in the keymap where the key's meaning is and the **condition** in
  > the host where the state is. No sixth `Scope`, no conditional `Role`, and `runtime/keymaps.scm`
  > stays data.
  >
  > **The state it reads is a new field with one setter and two clearers.** `Editing::chosen` is
  > *set* by the `MoveCompletion` arm and by nothing else — *"pressing `<C-n>` is the whole of
  > what the user chose a row means"* — and *cleared* at the two ways a session ends: in
  > `Editing::close_completion`, a new method that is now the single place a session is **dropped**
  > (five call sites collapsed into it), and in the `IngestCompletions` arm, the single place one
  > is **replaced**. The invariant is the point rather than the count: the flag can only be true
  > inside a session the user steered in, because those two are the only exits. (This paragraph
  > said *"exactly one writer"* and the three comments in `main.rs` said one, one and two — all
  > four wrong against a `grep` of the file, and corrected at the `CP-4` review.) `Editing::accept`'s guard
  > reads *"there is a session **and** the user steered in it"* as one condition, because a key
  > over no float and a key over a float nobody has touched are the same situation to the hands.
  > `<C-y>` passes `None` for `otherwise` and so keeps vim's meaning exactly. `nvim-cmp` spells the
  > same rule `select = false`.
  >
  > **`then` is why `<space>` is one undo step.** The trailing text is spliced inside the same
  > `begin`/`commit` batch as the accepted item, so `u` takes back the completion and its space
  > together rather than leaving a widowed space behind.
  >
  > **Three keys, one capability, three readings of it** — the two rows `runtime/keymaps.scm`
  > gained answer the report line for line: `<space>` is `"then" " " "otherwise" " "`, `<cr>` is
  > `"otherwise" "\n"` with no `then` at all, and `<C-y>` passes neither and so still accepts
  > whatever is highlighted. The file's new comment block argues it where it binds it, which is
  > where the `<C-x>` divergence is argued too.

  *Done when:* the keys a vim or VS Code user reaches for drive the list while it is open and do
  their ordinary job when it is not, in the running binary, **proved by a pty test that presses
  each contested key in both states** — a test that only presses it with the float up cannot fail
  when the fall-through is wrong, which is the defect class this build has already had to replace
  tests for. `<C-x>`/`<C-n>`/`<C-p>`/`<C-y>`/`<C-e>` keep working, because they are what `7c`'s
  no-footer exception means by *"every key that drives it has to be one your hands already know"*.
  **`<tab>` is not settled by this task alone** — §38. *Needs:* T026, T033, T038

  > **Ticked 2026-08-16, after §38 was re-ruled and `<tab>` came to this task after all.** The
  > *done when* asked for the contested keys pressed **in both states**, and that is what exists:
  > `<space>` and `<cr>` each have a pair in `crates/phosphor/tests/loop_pty.rs` (over an
  > untouched float, and over a chosen row), and `<tab>` now has its own pair —
  > `tab_steps_the_completion_list_and_then_enter_accepts` and
  > `tab_with_no_completion_list_open_types_one_indent_level`. `<C-x>`/`<C-n>`/`<C-p>`/`<C-y>`/`<C-e>`
  > are untouched and still pressed by the tests that always pressed them.
  >
  > **It was held unticked for the right reason and released by a ruling rather than by more
  > code.** The sentence above — *"`<tab>` is not settled by this task alone"* — was the blocker,
  > and Teej settled it at `CP-4`'s manual half by asking for helix's behaviour. What that took
  > was `move-completion` growing an `otherwise` of its own, which is §38's *first* option landing
  > two days after its third one shipped.
  >
  > **The report is answered line for line now.** *"i like being able to hit space to select and
  > put a space after or enter to select without a space after"* — both, since `49ca8da`. *"in
  > this form i should be able to hit tab or something to select"* — `<tab>`, and `<S-tab>`
  > backwards. And *"enter or space doesnt accept"* needed no change to either key: it was the
  > absence of a stepping key, which is why one fix closed two complaints.

- [x] **T106 · What a completion row says about itself**
  Reported at `CP-4`: a row is a label and a type and nothing else, and every completion UI a
  person has used says more.
  **The prior art, recorded because it is the reason for the shape rather than as decoration.**
  Teej named five, and **all five** converge on one row grammar — `<kind> <label> <detail dimmed>
  [source]`: `nvim-cmp` with `lspkind` (kind as a symbol or a word, `menu` carrying the source);
  Emacs `corfu` with `kind-icon` (a coloured margin formatter keyed on the kind) and `company-box`
  (icons plus a documentation child-frame); VS Code (icon, label, `labelDetails.detail` inline and
  `labelDetails.description` right-aligned); and Helix, which draws a plain kind column. The
  agreement across five independent designs is the argument: **the kind is the first thing a
  reader wants and the last thing this build carries.** (This paragraph said *"four of them"* and
  then listed five, and attributed the per-kind hue to `lspkind`; corrected at the `CP-4` review.
  `lspkind` supplies the glyphs or words and has a text-only `mode`; in nvim the **hue** comes
  from `nvim-cmp`'s `CmpItemKind*` highlight groups, and `kind-icon` is the one that makes the
  colour its whole subject. Helix's plain kind column is the one claim here nothing in this tree
  can check.)
  **The finding underneath it, verified against `e3af880` this session, which is what this task
  was written from.** `lsp-types` 0.95.1's `CompletionItem` carries `kind` (25 values,
  `CompletionItemKind::TEXT`…`TYPE_PARAMETER`),
  `label_details: Option<CompletionItemLabelDetails>` with **two** fields — `detail`, *"rendered
  less prominently directly after the label"*, and `description`, *"fully qualified names or file
  path"* — plus `tags: Option<Vec<CompletionItemTag>>` (one value, `DEPRECATED`), `deprecated:
  Option<bool>` and `preselect`. `phosphor_buffer::lsp::completions_from_lsp` reads `label`,
  `detail` (falling back to `label_details.detail`), `documentation`, `insert_text`, `filter_text`
  and `sort_text`. **`kind`, `label_details.description`, `tags`, `deprecated` and `preselect` are
  read by nothing**, and neither `phosphor_buffer::lsp::Completion` nor
  `phosphor_core::request::Completion` nor `phosphor_ui::float::CompletionItemVm` has a field to
  put them in — the last is `label` and `detail` and stops.
  **And the client never asked for two of them**, which is the half a reading of
  `completions_from_lsp` alone would miss: `lsp::initialize_params` sends
  `completion: Some(CompletionClientCapabilities::default())`, whose `completion_item` is `None`,
  so `labelDetailsSupport`, `tagSupport` and `deprecatedSupport` are all unannounced. A
  specification-conformant server may therefore send no `labelDetails` at all — which makes the
  existing fallback at `item.label_details.detail` potentially dead code against exactly the
  servers that behave. **Announcing the capability is part of this task and precedes the drawing**;
  the `initialize_params` header already argues the general form of that point (*"a server is
  entitled to answer nothing to a request the client never said it could use"*).
  **Two design constraints this cannot fold in, and one drawing it changes.** Design Language §2
  is *"one cell, one concept … all single-cell, Nerd-Font-free, present in default terminal
  fonts"*, so the icon half of every one of the five prior-art UIs is **out** — a kind is a word,
  an abbreviation, or one of §2's existing glyphs. §1 is *"each color names exactly one actor or
  state, never decoration"*, so `kind-icon`'s per-kind hue is out too; the nearest thing the
  palette already has is `#cfa86a transient`, whose stated meaning includes *types*. And
  `TUI Mockups.dc.html`'s `7c` **draws two columns** — label, and detail in meta-grey — so **two**
  more columns (`kind` left of the label, `source` right of the detail) are a change to a mockup,
  which under `CLAUDE.md`'s rule is **flagged and not folded in**: Teej amends it at claude.ai, and
  `docs/design/` is not touched here.
  **`7c`'s golden frames are *not* the mechanical cost, and that is the point.** This entry used
  to say `crates/phosphor/tests/screen_7c.rs` and `crates/phosphor-ui/tests/screen_7c.rs` *"both
  re-bless"*; neither did — `git diff --stat HEAD -- '*.snap'` over the `7c` captures is empty.
  Both fixtures transcribe the **mockup**, which has neither new column, so both frames stay
  byte-identical and no divergence is blessed. That is the right outcome (re-blessing a
  conformance capture before the mockup changes is the shape `T100` found in `6b`) and it was
  briefly the *wrong* mechanism: the fixtures took the two new fields as
  `..CompletionItemVm::default()`, which made the frames blind to the change rather than faithful
  to the mockup — a sixth field would have arrived the same silent way. The fields are spelled out
  now, so the next one breaks the fixture and forces a decision. The new columns are captured
  instead in `crates/phosphor-ui/tests/float_width.rs`'s `decorated-80` / `decorated-120` /
  `decorated-selected-deprecated` frames, whose commit notes say `7c` draws neither.

  > **Mid-flight as this was written**, and the same caveat as `T105`: the working tree carried
  > this **uncommitted**, `crates/phosphor-ui/tests/completion_shed.rs` was named by a doc comment
  > and did not yet exist, and no `runtime/` or pty change had landed. Read the commit. What is
  > worth recording here is that **the finding above was answered rather than argued with**, and
  > that the §2 constraint decided the shape:
  >
  > * **`request::CompletionKind` — twenty-five arms, exhaustive**, because the set is closed by
  >   the protocol; `phosphor_buffer::lsp::completion_kind` maps the wire newtype and answers
  >   `None` outside 1–25 rather than guessing, since `lsp_types::CompletionItemKind` is an `i32`
  >   newtype the protocol may extend. Its header takes the §2 argument head-on: *"inventing
  >   twenty-five glyphs is inventing a second lexicon"*, so a kind is a **four-cell lowercase
  >   word** (`fn`, `strc`, `memb`) with a `WIDTH` constant and an `abbreviation` that cannot
  >   exceed it — fixed rather than widest-present, so the labels do not slide sideways as you
  >   press `<C-n>`.
  > * **The client now asks for what it reads.** `initialize_params` announces
  >   `label_details_support`, `tag_support` and `deprecated_support` where it sent
  >   `CompletionClientCapabilities::default()` — the exact gap this entry named, and the comment
  >   there records it was measured against rust-analyzer 1.97.1 rather than assumed.
  > * **`source` is `labelDetails.description`, not `.detail`** — the other half was already read
  >   as a fallback for the top-level `detail`, and conflating them is the mistake the field name
  >   is chosen to prevent. `deprecated` accepts **both** spellings (`tags` and the pre-3.15
  >   boolean) because rust-analyzer sends both.
  > * **A shed order, which is §11 applied to a row.** `float::ListLayout` computes columns from
  >   the width the body was handed rather than from the content, and drops widest-first —
  >   `source`, then `detail`, then `kind`, and the label elides only when everything else is
  >   gone. That is *"drop, never squeeze"*: a column is present at full natural width or absent,
  >   because squeezing puts an `⋯` on every row instead of showing two whole columns. The half
  >   this replaced placed the detail at *widest-label + 2* and let elision absorb the overrun.
  > * **One thing is flagged rather than folded in, and it is in the right place.**
  >   `float::label_style` draws a deprecated row struck through **and** receded one step down §1's
  >   neutral ramp, and its own header says why both: `Modifier::CROSSED_OUT` is SGR 9, which no
  >   capability query can report on, so the treatment that survives a terminal ignoring it has to
  >   carry the meaning alone. **`view::props::Emphasis` does not name strikethrough** — read this
  >   session, it is `Plain`, `Inverted`, `Underline` and `Undercurl` — so the SGR is a fifth
  >   treatment nothing in the design language has blessed. One function, one place to remove it.
  >
  > **What the review of the above changed, all inside the same files:**
  >
  > * **A one-cell detail column is not a squeezed detail, it is the elision mark alone.**
  >   `keep_detail` kept the column whenever a single cell survived, so at 30 columns against
  >   rust-analyzer a row read `meth len   ⋯` — two gap cells and one content cell spent saying
  >   *"something was removed"*, which is the squeeze §11 forbids one column over from where
  >   `ListLayout` argues the elision **is** allowed. `float::DETAIL_MIN` is the floor and
  >   `a_surviving_detail_column_can_say_something` is the law.
  > * **Two of the shed-order proptests could not fail.** Their guards were the implementation's
  >   own branch conditions and their assertions reduced to `x >= x`. Restated: the label one now
  >   also asserts that the cells it was promised are cells no meta column claims (two fields tied
  >   together, red under `label_room = width - label_at`), and the kind one is stated against the
  >   **drawn cells** — *the kind column is on screen and no label carries an `⋯`* — which says
  >   nothing about how `keep_kind` is spelled. Both were watched going red on a planted mutation.
  > * **`label_style`'s selected-and-deprecated arm was executed and asserted by nothing.**
  >   `decorated()` selects row 0 and the deprecated row is row 2, so a mutation swapping the two
  >   neutrals left every committed frame byte-identical. `decorated-selected-deprecated` is the
  >   frame, with a buffer assertion beside it.
  > * **The order is a priority, not a monotone sequence**, and `ListLayout`'s header says so now.
  >   Measured: a kind column, a 10-cell label and an 8-cell detail draw both at 19 cells, lose the
  >   detail at 18, and get it back at 14 when the kind goes and the label stops starting five
  >   columns in. That is what the four steps describe; it is not what the phrase sounds like.
  > * **Ceremony removed.** The kind column's `x` was `label_at - (WIDTH + KIND_GAP)`, arithmetic
  >   that can only evaluate to zero — `ListLayout` has a `kind_at` now, so the layout owns all
  >   four placements. The `column == 0 ? 0 : column + gap` rule was written out four times and is
  >   `float::column_block` once.
  > * **Two doc comments asserted what a third recorded as unverified.** `CompletionItemVm::source`
  >   and `request::Completion::source` said flatly that rust-analyzer fills
  >   `labelDetails.description` with an import path; `completions_from_lsp`, from the same change,
  >   says that case wanted a workspace the measurement did not have. Both now carry the caveat and
  >   point at the measurement.

  *Done when:* a row carries what the server said about it — at minimum the kind — the client
  announces the capabilities it reads, the float still fits `MAX_ITEM_ROWS` and §8's padding at 80
  columns, and it reproduces **from a keystroke** in the running binary rather than from a
  hand-built `CompletionVm`. **Not before the `7c` drawing is amended**, and not with a fifth
  emphasis the view tree cannot name. *Needs:* T036, T038

- [x] **T107 · A buffer with no file — `phosphor` with no argument** 📌
  Teej at `CP-4`: *"we need a scratch buffer when no file is specified mode if thats not already
  on the roadmap"*. **Most of it is already there and one line refuses it.**
  **What refuses.** `Cli::path` in `crates/phosphor/src/main.rs` is
  `#[arg(value_name = "FILE", required_unless_present_any = ["eval", "repl"])]`, so a bare
  `phosphor` never reaches the loop. Run this session against the installed binary: `phosphor`
  answers *"error: the following required arguments were not provided: <FILE>"* and exits `2`,
  which is clap's usage error and not this build's voice at all.
  **What already exists, which is why this is small.** The loop's `match &cli.path` has a `None`
  arm that builds `buffer("text", "", &theme)` for `--repl`; `Timeline::detached` is the
  no-file history and the comment above it already uses the words *"a scratch buffer has no file
  to key one on and gets a tree with nowhere to write itself"*; `Timeline::log`'s own doc says
  `None` is *"a scratch buffer and … a workspace with no state directory — a session that cannot
  persist still undoes"*; `AppHost::write` already answers `no file name — :write <path>`
  when there is nothing to write to; `main::adopt`'s header already says a buffer with no file
  *"tells no server anything … a server addresses files"*; and the statusline draws no file
  segment. So the
  concept is built, tested through `--repl`, and unreachable without a flag that also opens a
  REPL.
  **What is genuinely open**, and it is not the CLI line: what the first frame *says*. `--repl`'s
  empty buffer is *behind* a surface that explains itself; a bare `phosphor` has nothing in front
  of it, and `IMPLEMENTATION-PLAN.md`'s third invariant is *nothing moves unless you asked*, not
  *nothing is said*. Related: `main`'s `fresh: Option<PathBuf>` already exists to let the first
  frame say a named file has nothing behind it yet.
  **Its relation to `7d`, which is a cross-reference and not a merge.** `7d` is *Cold start* and
  `T057` owns it (`S6`, *"cold start (`7d`), attach/adopt/start (`5d`), drop and reattach (`7b`),
  opening mid-task (`2d`)"*, needing `T051`). `TUI Mockups.dc.html`'s own rule for it is *"Cold
  start invites, never nags: an empty dashboard states what it found (no session, no history) and
  lists three verbs. One dismissable line, then it's just an editor."* — and the screen it draws
  is `phosphor` bare, with `repo ~/src/fetchd · 214 files · no vcs detected`, `session none
  running`, `history —`, and `:e edit · :cn start claude · :f find file`. **The overlap is real
  and the subject is not the same**: `7d` is about *no agent session* and needs `T051` to know
  there is none; this is about *no file* and needs nothing. So this task builds the buffer and the
  honest first line, `T057` builds the dashboard over it, and the second must not wait on the
  first — `S6` is three phases away and a bare `phosphor` should work before then.
  *Done when:* `phosphor` with no argument opens an editable buffer, `:w` without a path refuses
  in §6's voice rather than clap's, `:w <path>` gives it a file and a history from that point, and
  a pty test starts the binary with no argument and types into it. *Needs:* T030, T033

  > **What landed, the four questions answered in the code, and one corruption the tests found.**
  >
  > **The entry was right that most of it existed, and one of its citations is wrong.** `write` is
  > `Editing::write`, not `AppHost::write` — checked this session; the enclosing `impl` is
  > `Editing` and `AppHost` has no such method. Everything else it claims about the tree held.
  >
  > **What was deleted.** `Cli::path`'s `required_unless_present_any = ["eval", "repl"]`, and with
  > it `dispatch`'s second refusal behind it (*"give a file to open, an expression to evaluate, or
  > --repl"*), which was unreachable prose guarding a case clap had already refused. The `None`
  > arm of `run`'s `match &cli.path` now serves `--repl` and a bare `phosphor` from one line and
  > needed nothing added to it: `Timeline::detached`, `adopt` returning `None`, `grammar_of` never
  > being asked and the statusline's absent file segment were all already correct for a buffer
  > with no name. `--help`'s `long_about` says so now, which is the claim a keystroke can disprove.
  >
  > * **What the statusline says: nothing, and the first row says the rest.** Not vim's
  >   `[No Name]`. `status/file` in `runtime/statusline.scm` already answers a void `file` with
  >   `'()` and that is the editor layer's decision, not Rust's — the change here would have been
  >   *giving* the layer a name to draw, and the name it would draw says the same thing every
  >   frame for the rest of the session. What a person actually needs is the *verb*, once: the
  >   notice row carries `no file — :write <path> creates one` on the first frame, which is the
  >   same sentence `:write` refuses with, said before it is asked. Guarded on `Surface::Buffer`,
  >   so `--repl`, the boot float and the `--float` fixture — all of which explain themselves —
  >   stay silent. **Reversible in one function** (`main::no_file`) if `[No Name]` is wanted; that
  >   is a layer change plus an `Option<PathBuf>` on `StatusFile`, and it was not made here.
  > * **What `:w` does: exactly what it did.** `no file name — :write <path>` is `Editing::write`'s
  >   own refusal, unchanged, and it is §6's voice already — lowercase, em dash for cause, and the
  >   whole command rather than a contraction. What changed is that it is now reachable: before
  >   `T107` the same mistake produced clap's *"the following required arguments were not
  >   provided"* one layer earlier. The ex line has parsed `:w <path>` since `T033`
  >   (`runtime/keymaps.scm` passes `rest` as `"path"`).
  > * **Undo and the journal: `Timeline::detached` was half of it.** A scratch buffer undid all
  >   session and reopened with nothing, because the moment a buffer *gains* its first file was
  >   the moment nothing was watching. `Timeline::attach` opens a journal at that instant and
  >   seeds it with `seeding(&tree, origin)` — the whole tree, in `History::snapshot`'s own record
  >   order, because the fold requires dense ids and appending only what follows the save would
  >   hand a fresh log a `Node { id: 7 }`, which it refuses and `Timeline::append` answers by
  >   silently dropping the log.
  > * **The language degrades and does not retroactively upgrade.** `adopt` keys on the extension
  >   and answers `None` for a buffer with no file, which was already true and is now reachable.
  >   `:w notes.rs` does **not** re-run it: adopting a grammar means rebuilding the `Editor` the
  >   way the `open-file` arm does, which discards the cursor and the selection of a buffer the
  >   user is in the middle of. `:e` on the file the write just created is the existing door.
  >   Recorded in `adopt`'s header, not left to be discovered.
  >
  > **The corruption, which a test written to pin the opposite behaviour found.** The first
  > version of `attach` left an existing journal alone when `:write <path>` overwrote a file that
  > had one — conservative-sounding, and wrong: the tree under that key describes bytes the write
  > just replaced, and a *stale* history is worse than a missing one because nothing downstream
  > can tell. Measured through the pty: `owned\n` with one saved edit, written over by a scratch
  > buffer holding `new`, reopened, `u` — and the buffer became **`ew`**, undo applying the
  > inverse of an edit against text that no longer existed. The journal is replaced now and the
  > row says a history went, which is `Timeline::open_at`'s own rule (*"a tree that matches disk
  > nowhere is not a history of this file at all, and is dropped"*) applied to the case where
  > `saved` is present and wrong rather than absent. Q1's collision guard still stops it: a
  > journal whose origin is a *different* file is refused, not deleted.
  >
  > **A harness hang, introduced and fixed while planting a mutation.** `Editor::started` held the
  > `Command` alive past the spawn, which holds this side's pty slave fds open — so a child that
  > exits without drawing left `await_frames` to time out correctly and then `Drop` to block
  > forever in `reader.join()`, waiting on an end-of-file that cannot arrive. Found by restoring
  > the required FILE argument to watch four tests go red and getting a run that never ended
  > instead. The `Command` is scoped now and the comment says why.
  >
  > **Eight tests, each watched failing on its own mutation.** Five drive the binary on a pty
  > (`loop_pty.rs`: `a_bare_phosphor_opens_a_buffer_and_says_what_would_give_it_a_file`,
  > `a_bare_phosphor_with_unsaved_work_is_still_quittable`,
  > `write_with_no_path_refuses_by_naming_the_command_that_would_work`,
  > `a_scratch_buffer_written_to_a_path_undoes_into_it_after_a_restart`,
  > `writing_over_a_file_replaces_the_undo_history_that_was_under_it`) and three are `main.rs`'s
  > (`the_seed_a_scratch_buffer_writes_folds_back_into_the_tree_it_came_from`, whose tree has a
  > branch point a pty cannot reach; `writing_a_buffer_with_no_file_refuses_by_naming_the_whole_command`;
  > `dropping_the_required_file_left_every_other_invocation_alone`, which is the guard on
  > `phosphor <file>`, `--repl`, `--eval` and `--eval` beside a file still meaning what they did).
  >
  > **A ninth test was left asserting the constraint this task deleted**, found by the review and
  > not by a gate. `door.rs`'s `the_host_still_needs_a_file_and_the_door_does_not` was named for
  > `Cli::path`'s `required_unless_present_any` and asserted *"no file and no expression is
  > usage"*; it kept passing because `run` puts stdout on a pipe, so a bare `phosphor` now fails
  > on the **terminal** instead of on clap. Measured: exit 1, stdout empty, stderr
  > `phosphor: terminal i/o failed: Device not configured (os error 6)`. Passing for a reason it
  > does not claim is the worst state a test can be in, so it says the new reason —
  > `a_bare_phosphor_is_refused_by_the_terminal_rather_than_by_the_parser`, which fails if
  > `required arguments` ever comes back — and the `--help` half it was bundled with is its own
  > test.
  >
  > **A tenth pins the notice's guard from the other side.** `main::no_file` is guarded on
  > `Surface::Buffer` and argued at length in two doc comments; nothing checked the negative case,
  > and dropping the guard left the suite green. `--repl` cannot see it (the REPL draws over the
  > statusline's row and swallows the notice whether or not the guard is there — observed with
  > the guard planted out), so the fixture float is the surface that can fail:
  > `a_float_over_a_nameless_buffer_says_nothing_about_the_missing_file`.
  >
  > **`7d` is untouched**, as the entry asks. Nothing here reads a session, counts a repo or lists
  > a verb; `T057` still owns the dashboard and can build it over this buffer.

- [ ] **T108 · The file browser — netrw → vinegar → oil.nvim** 📌
  Teej at `CP-4`: *"lets start scouting ahead for what we will need for a netrw inspired vim
  vinegar inspired and eventually oil.nvim inspired file browser, but ill want to implement that
  in detail with you, so im just letting you know now so we can plan"*.
  **This is a placeholder for a design session and it has no *Done when:* on purpose.** Writing
  one would be inventing the surface Teej said he wants to design; a criterion invented here is
  one somebody later builds to. What follows is the lineage, one hypothesis marked as a
  hypothesis, and the three things a design session should not have to re-derive — each checked
  against the tree this session.
  **The lineage, and what each step adds.** *netrw* is a directory **listing in a window**: it
  draws the entries and opens one. *vim-vinegar* keeps netrw and fixes the addressing — `-` opens
  the **parent** directory with the cursor on the file you came from, so "up" is a keystroke and
  you never lose your place. *oil.nvim* makes the directory an **editable buffer**: renaming is
  editing text on a line, deleting is deleting a line, creating is adding one, and `:w` applies
  the accumulated filesystem operations. The three are a progression in *what the buffer is*, not
  in features — a view, an addressable view, and then a mutable one.
  **Why the oil model may fit this build unusually well — a hypothesis, not a decision.** Every
  edit in this editor is already an `Action`, an `Action` already crosses one vocabulary and three
  doors, and undo is already a tree keyed on a buffer. If a directory is a buffer, then a rename
  is an ordinary buffer edit that produces an ordinary Action, which means it is scriptable from
  Steel, callable over MCP, and undoable, **with no second mechanism** — and an agent would rename
  a file through exactly the door a person does, which is invariant 2's whole claim. `7a`'s
  permission ask is the natural gate on the consequential half, and the capability table already
  has the rating for it: `ReloadFromDisk`, `ResolveDiskDiff` and `RevertHunk` are declared `Ask`.
  **Read the caveat with the hypothesis**: nothing enforces `Ask` yet — `main::deliver` answers
  `McpPolicy::Ask` with *"needs an ask first — T060 builds the queue"* — so the gate is a policy
  value and a screen, not a mechanism.
  **What must be scouted before designing, with what the tree says today.**
  **(1) The vocabulary has no filesystem verbs beyond opening and writing.** `Action::File`
  (`crates/phosphor-core/src/action.rs`) is exactly nine: `open-file`, `save-buffer`, `save-all`,
  `close-buffer`, `reload-from-disk`, `note-disk-change`, `open-disk-diff`, `resolve-disk-diff`,
  `set-file-watch`. **There is no create, no rename, no delete, no mkdir anywhere in the table** —
  not in `File` and not in any of the other twenty domains: grepped this session, the only
  `create`/`delete`/`remove` names in it are `create-pane-from-view`, `delete-thread`,
  `remove-watch` and `remove-keybinding`, none of which touches a filesystem. So the oil model is
  not a re-use of
  existing verbs; it is *new capabilities*, which is `spine`'s and is the first thing the design
  session has to rule.
  **(2) A directory listing is neither a store query nor a buffer today — it is nothing.**
  `crates/phosphor-core/src/query.rs` has no filesystem query at all; the closest thing in the
  graph is `T046`'s *files* picker source, which is `(define-picker-source …)` producing rows for
  the Picker (`picker-rows` takes a `SourceId`) — a **transient float over a list**, which is a
  different object from a buffer you can edit and save. Whether the browser reuses that source or
  needs a real listing is a scouting question, and the answer changes which crate owns it.
  **(3) Anchors on a buffer whose lines are paths land on `T043`, not `T042`.** `T042` binds
  anchors to **tree-sitter nodes**; a directory buffer has no grammar, so it falls to `T043` —
  *"line + content fallback anchoring … the floor, not a degraded extra … markers work correctly
  on an extensionless file with no grammar"*. That is the right home and it is worth knowing
  before designing, because the interesting case is not an anchor surviving an edit — it is what a
  region on a line **means** after `:w` has renamed the file that line named. Nothing in the
  region lifecycle (§7: *claude writes → unseen → seen*) is defined over a line that is an
  address rather than content.
  *Needs:* T033

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
