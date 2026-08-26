# Phosphor — task breakdown

Decomposed from [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md), which is itself derived from
the four design docs in [design/](design/). The plan says *what each phase is for*; this file
says *what to build, in what order, and where we stop and look at it*.

**112 tasks + 9 harness tasks · 12 checkpoints · 9 phases**, covering all 34 screens v1 builds.
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

**Window C is built and its mechanical half is green.** The `Action` vocabulary is 220
capabilities generated from one table, the three doors are total functions over it, and the
parity test walks all 660 door checks end to end. (`208`/`624` until `S3` added
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
**Deferred by Teej on 2026-08-20**, together with `CP-5`'s, until `S6`–`S8` are built — a standing
waiver of the rule that no window starts on an unpassed checkpoint. It is recorded at
[TEAM.md](TEAM.md)'s *Windows and gates* with what it costs, and it is a deferral rather than a
pass: nothing below may be read as a verdict, and the residue is unchanged.

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
  > this file's *"What stays irreducibly Tier 3, and why"* table records that the browser-based terminal VHS drives does not implement it. The hardware
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

- [x] **T086 · `HelpGrid` — the `:help` float body** 📌
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
  >
  > **Ticked 2026-08-23. All three items above are closed, and the first two were closed by
  > other tasks that said so.**
  >
  > **The keystroke half was `T097`'s**, and its record already names the three pty tests that
  > carry it: `:help` opens the grid, `:help agent-objects` narrows it, and a REPL rebind reads
  > back out of it. Nothing was owed here.
  >
  > **The third head is one line and the entry named it.** `agent_object_hints` asked for the
  > first `Select` and the first `Operator` in normal scope, which is `v` and `d` — so `6d`'s
  > third grammar row had nowhere to go however the keymap was written, and the frame drew `dib`
  > where `gsib` belongs. It asks for `Operator(MarkSeen)` by name now, and the asymmetry is the
  > point: `6d`'s claim about the first two is *"whatever your select and delete are"*, and its
  > claim about the third is about **mark-seen specifically**. The frame draws twelve rows
  > (`viu`…`gsib`) where it drew eight.
  >
  > **The false note is gone and it had been false since the comment verb landed.** `c[omment]`
  > is bound with a one-letter minimum, so `:c` resolves — `cl[aude]` needs two. The snapshot
  > said *"there is still no `:c` command"*, and the frame did not move, so `insta` passed the
  > prose along with it. That is the failure mode this entry itself identified, and it took a
  > reader checking each clause against the tree rather than a lint, because **no lint reads a
  > snapshot's prose**.
  >
  > **Two assertions were pinned to layout incidentals**, and growing the grid found both. The
  > rebind test asserted `viU  visual …` with the exact gap — which is the key column's padding,
  > the width of the *longest* key in the table, so `gsib` moved it from two spaces to three and
  > a correct grid failed. It reads the row and checks the verb is on it now. And the frame at
  > 24 rows no longer fits twelve grammar rows, so the rebind test draws at 40: the float's
  > *"and N more"* row is the shipped answer to a body taller than its float and is already
  > tested at `a_help_body_taller_than_its_float_names_what_it_dropped`, but it is not this
  > test's subject.
  >
  > **What is still owed and is not this task's:** nothing scrolls a `Density::Help` body, so
  > `:help normal` stops at what fits and says how much it dropped. That is honest rather than
  > silent, which is what `T018`'s rule asks; making it *scroll* is a surface with a viewport
  > and belongs to whoever gives floats one.

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
  > **The fill-only rule gained one exception, 2026-08-20, and `OPEN-QUESTIONS.md` §43 is why.**
  > A fingerprint taken *before the grammar parsed* carries an empty syntax path, and an empty
  > path means the node tier never applies again for the life of that region — so which tier a
  > marker rides was decided by a race between the parser and the first describe. §43's prescribed
  > experiment was run and confirms it: same region, same rewrite, **nine lines apart** depending
  > only on that race. `fingerprint_in` now upgrades a syntax-less fingerprint when the file is
  > described with syntax, under the one condition that keeps the rule intact — the line must
  > still say what the fingerprint says it says.
  >
  > **The other direction is still open and is this task's to finish.** A *good* fingerprint
  > meeting a snapshot with no syntax — which is what the editor produces before its own parse is
  > ready — falls to the line tier and can move the region. That is not a bug in either function:
  > the fall-through is a law with a property test on it
  > (`a_node_tier_miss_still_lands_on_the_line_tier`), and reconciling the two turns on whether a
  > `Snapshot` should be able to distinguish *"the construct is gone"* from *"nobody has parsed
  > this yet"*. Pinned by `a_good_fingerprint_meeting_an_unparsed_snapshot_moves_to_the_wrong_line`
  > so a change to the ladder has to look at it.
  >
  > **And a mutation survivor here was a real defect** (`OPEN-QUESTIONS.md` §46): `reanchor_in`'s
  > filter read `region.path == path && region.fingerprint.is_some()`, and replacing `&&` with
  > `||` survived the whole suite — every other test in the module uses one path, so nothing
  > noticed that the mutant would resolve *other files'* regions against this file's snapshot.
  > `reanchoring_one_file_leaves_another_files_regions_where_they_are` closes it.
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

> **Deferred by Teej on 2026-08-20**, with `CP-4`'s, until `S6`–`S8` are built. The mechanical
> half below is complete; **the manual half is the one this checkpoint is actually for**, and it
> has not run. `CP-5`'s own failure condition — *"the markers don't change how you read the file
> … worth stopping over, not building past"* — is the thing being deferred, not a formality, and
> [TEAM.md](TEAM.md)'s waiver entry carries the cost. Nothing here is a pass.
>
> **The mechanical half, recorded 2026-08-19 and closed 2026-08-20 — and this is not a verdict.**
> `CP-5` has two halves and only Teej can run the second. This is written here because `CP-2`'s rule puts it here: **a
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
> * **~~Anchor-survival across a real refactor (`6c`): partly.~~ Met, 2026-08-20.** The tier ladder
>   was already tested — `the_node_tier_follows_a_construct_that_moved`,
>   `a_rename_falls_off_the_node_tier`, `a_node_tier_miss_still_lands_on_the_line_tier` — and a
>   property test carried an anchor through an insertion. What did not exist was the `6c`-shaped
>   end-to-end case, and it does now:
>   `loop_pty.rs::an_unseen_marker_follows_its_construct_through_a_rewrite_in_the_running_editor`
>   declares a region through the door, **types four lines into the live buffer**, runs `reanchor!`
>   over the editor's own snapshot, and reads the region's new line back out through
>   `unseen-regions`. It moves from 6 to 10 and stays unseen.
>
>   The distinction is worth stating because it is the one every test above it could not make:
>   those prove `resolve` is correct **given** a snapshot, and this proves the editor takes a
>   correct one. A host that described the wrong text, carried stale syntax, or had an off-by-one
>   in its line indexing would pass all of them. That is not hypothetical —
>   `OPEN-QUESTIONS.md` §43 is a defect in exactly that seam, found in the same window.
> * **~~`1a`, `2a`, `3d`, `8a`, `6a` snapshots: the captures exist, the *snapshot tests* do not.~~
>   All five committed, 2026-08-20.** `crates/phosphor/tests/snapshots/` held `3c`, `6b`, `6d`,
>   `7c` and `8e`, and these five screens were covered by VHS and by nothing at Tier 1. Three new
>   files close it, each at 120 and 80 columns:
>
>   * `screen_1a.rs` — the flagship. The state column is built by the *same two calls the frame
>     loop makes* (`gutter::spans`, then `gutter::state_column`) over a real store, so the frame
>     shows §3's ladder deciding that the seen region draws ground while the two unseen ones do
>     not. This is `CP-5`'s failure condition — *"the markers don't change how you read the
>     file"* — expressed as cells.
>   * `screen_pickers.rs` — `2a`, `3d` and `8a`, in one file because `runtime/pickers.scm` says
>     they are one screen: *"nothing in rust knows what a row means — which is why `2a`, `3d` and
>     `8a` are one widget with three sources."* Every row is what the shipped Scheme answered
>     through `phosphor_steel::picker::rows`, against a store seeded with `plan.scm`'s own spans.
>   * `screen_6a.rs` — `:arch`, composed entirely through `T080`'s spans hatch with its five
>     counts answered off the store. If this frame draws, the escape hatch is sufficient for a
>     whole screen, which is `T048`'s acceptance restated as a picture.
>
>   **Two findings came out of writing them**, which is the argument for Tier 1 over Tier 2 in one
>   sentence. `2a`'s preview pane does not draw at 120 columns — `PREVIEW_AT` is a terminal-column
>   number checked against a float body (`OPEN-QUESTIONS.md` §45) — and `8a`'s row is the whole
>   matched line, which silently settles half of §12 in the mockups' absence. Neither is visible
>   in a capture, because neither changes a pixel anybody would look at twice.
>
> **The VHS half — complete, 2026-08-20.** `2a`, `3d`, `8a`, `6a`, `seen-cleared` and
> `no-grammar` are captured and match. The clip of `s` clearing a marker is `seen-cleared`, and it
> presses **`SPC u s`** rather than `s`: the shipped keymap binds bare `s` to vim's substitute, and
> `CP-5`'s wording predates that ruling.
>
> **`1a` in full against the `V006` fixture is `tapes/1a-seeded.tape`.** `1a.tape` opens
> `tapes/fixtures/core-lib.rs` — the frozen file §40 repointed all twenty-five tapes at — with an
> empty store, so the flagship screen was the one screen here drawn with nothing behind it. The new
> capture opens `fixtures/src/retry.rs` against the seeded store: markers on line 4 and lines
> 12–24, the region tint on those rows, `2 unseen` on the statusline. A second tape rather than a
> repoint, because `1a` is a `CP-1` artifact and `1a-degraded-{term,nocolor}` are `V009` variants
> of that same frame — changing what it photographs would settle one checkpoint item by
> invalidating three.
>
> **Two frames of forty-eight do not match**, and neither is drift: `6b` is a blessed change
> (`unseen-regions` answered `T041`'s deferral until `T041` shipped; it answers `()` now, and the
> tape's sentinel waited ten seconds for a word the editor had stopped saying), and `broken-init`
> is `OPEN-QUESTIONS.md` §42 — it photographs the boot layer's own form count, so any Scheme form
> anybody adds moves it.

---

## S6 · The session and the directing loop

Split at the internal checkpoint from Q10. Two checkpoints.

- [x] **T050 · ACP session client**
  `agent-client-protocol`. One Claude Code session per editor per repo.
  *Done when:* a session attaches and a turn completes. *Needs:* T019

  > **Built 2026-08-21.** `crates/phosphor-agent/src/session.rs` is the client —
  > one thread, one runtime, one child process, and every method returns without
  > waiting, which is deliberately `phosphor-buffer`'s LSP client's shape.
  > `crates/phosphor/src/agent.rs` is the loop's half of the seam.
  > `agent-client-protocol` 2.0.0, Apache-2.0, `rust-version` 1.88.0 — the
  > workspace floor exactly, so it moved no MSRV. No features: all six are
  > `unstable_*` and a turn that completes needs none of them.
  >
  > **Reachable from a REPL line, which is what makes it testable now.**
  > `(set-option! "agent-command" "npx @zed-industries/claude-code-acp")` and the
  > next frame attaches; `:claude <message>` starts a turn. An **option** rather
  > than a capability, deliberately — `T057` owns the lifecycle verbs, and an
  > option cannot say *which of several running sessions*, which is the question
  > that task exists to answer. `send-message` is `T058`'s and is armed here
  > because *"a turn completes"* is unreachable without a way to start one;
  > `T058` owns the **line** (`1c`, the `⚓` chip, ex-style history), and the arm
  > refuses any message carrying anchors by naming it.
  >
  > **`StatusVm.session` is filled in, which is what the sentence there
  > promised** — it read *"`T050` and `T071` fill those two in; a fixture here
  > would be a lie on a real terminal"* and drew `SessionState::None` forever.
  > It is `session_state(life, turn)` now: the client's report about the
  > *connection* joined to the editor's record of the *turn*, with the join in
  > the binary so a rendering decision stays out of a transport.
  >
  > **The app clock was never read.** `Interpreter::at` existed, nothing called
  > it, so `now` sat at `Millis(0)` and neither `Node::Spinner` nor
  > `Node::Elapsed` could move. Honest while there was nothing to wait on; the
  > loop reads its own epoch now and the planted-defect run photographed
  > `⠋ claude working · 0:00`.
  >
  > **`Life::Starting` draws as `None`, and that is a gap this task records
  > rather than closes.** §5 lists *"idle, working+elapsed, waiting, paused,
  > lost"* and a session that is spawning is none of them. `None` is the least
  > wrong of the six and still wrong: for the second an agent takes to answer
  > `initialize`, the statusline says there is no session while one is starting.
  > `T051`'s *Done when* is *"every state renders"*, so the sixth state is that
  > task's to add or to rule out.
  >
  > **One turn at a time, enforced rather than assumed.** The client documented
  > it and did not hold to it: two `send_prompt`s overwrote the turn id and
  > emitted one `turn-ended` for two `turn-began`s, leaving a transcript row
  > that never closes. Prompts queue now. Found by a test that sends two without
  > waiting, because that is what a person typing quickly does.
  >
  > **The dependency changed another crate's wire bytes** —
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §51. Both ACP crates ask `serde_json`
  > for `preserve_order`; cargo unifies features across a `--workspace` build,
  > so `Value`'s map became an `IndexMap` and every LSP message `async-lsp`
  > round-trips came out in declaration order. A `phosphor-buffer` test that
  > asserted a multi-key JSON substring went red — its own comment admitted the
  > order was a fact about `serde_json`'s map type rather than about LSP. It
  > parses now.
  >
  > **A spawner two lints could not see.** `phosphor-agent::session_client`
  > drives children the SDK spawns for it, and `phosphor-buffer::lsp_documents`
  > builds a `ServerSpec` around `sh -c` — neither carries `Command::new` or
  > `open_pty`, and `.config/nextest.toml` said in prose that the second
  > *"[does not] spawn anything"*. Both are in the `spawns-a-child` group now,
  > and `lint-nextest-group.sh` counts `ServerSpec::new`/`SessionSpec::new` as
  > spawn markers.
  >
  > **Verification.** Six client tests against `toy_acp_agent.py` — a fixture
  > that speaks the wire form and imports none of the SDK, so a pass means both
  > sides agree with the protocol rather than with each other — plus two pty
  > tests in the running binary. Five planted defects, five caught: an un-queued
  > second prompt, a `turn-began` that records nothing, a `turn-ended` that never
  > clears, a `session_state` that never reports a session, and a
  > `lint-nextest-group` row removed. `just gate` green.

- [x] **T051 · `SessionState` + statusline**
  One enum — Idle, Working{elapsed}, Waiting, Paused, Lost, None — **rendered identically
  everywhere it appears**. Always present, always truthful.
  *Done when:* every state renders and the statusline is never stale. *Needs:* T050, T017

  > **Built 2026-08-21.** `T017` shipped this strip with a note reading
  > *"`SessionState` (renders `None` for now)"*; `T050` made four of the six
  > real, and this closes the rest.
  >
  > **"Never stale" is the half only a terminal can answer, and it found a
  > defect.** The editor draws when something tells it to, and a session dying
  > tells nobody — so the test attaches an agent and then **presses nothing**,
  > polling the screen while the agent exits on its own schedule. The first
  > version used the `deaf` fixture, which exits the instant it has answered
  > `initialize` — inside the keystrokes that set it up — so the editor had
  > already redrawn before the quiet phase began and the test **passed with the
  > wake removed**. `linger` exits two seconds later, which is the only
  > arrangement in which the poll is evidence.
  >
  > **What that then exposed: the client could not tell a dead agent from a
  > quiet one.** `AcpAgent` spawns the process and keeps the handle, and
  > `ActiveSession` holds its own update sender — so when the agent exited,
  > `read_update` waited forever on a channel that would never close and the
  > session went on reporting `Attached`. Measured:
  > `an_agent_that_dies_mid_session_is_a_drop` timed out at thirty seconds.
  > The client spawns the child itself now (`tokio::process` +
  > `tokio_util::compat`, the pair `phosphor-buffer`'s LSP client already uses)
  > and selects on its exit, so *"the agent is gone"* is an event. A spawn
  > failure is a `Result` from the OS rather than a string to classify —
  > `classify` is deleted, and `Failure::Spawn` versus `Failure::Dropped` is
  > now a fact rather than a guess about a message.
  >
  > **The `session` query answers the value the strip drew**, not a second
  > derivation of it: the loop composes the state once per frame and publishes
  > *that*, the way `T088`'s `panes` does. §5's *"one enum rendered identically
  > everywhere it appears"* is structural under that arrangement — there is
  > nothing for a surface and the strip to disagree with.
  >
  > **Every state renders, through both paths.** `status_line`'s proptest
  > already walked all six through the widget; a new test walks them through the
  > **tree** — `Node::Session`, the interpreter, the composition `T025` replaced
  > the widget with — and asserts the six rows are *distinct*, because six
  > states drawing one row would satisfy every other assertion and tell a reader
  > nothing.
  >
  > **`Life::Starting` is answered without inventing a state** —
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §52. §5 lists five states and a none
  > and a spawning session is none of them; rather than amend the design, the
  > strip keeps §5's list and the **notice row** carries the change (`starting
  > claude`, `claude attached`, and the failure's own sentence). §6 already puts
  > events there. Two shapes for putting it on the strip instead are written
  > down; both are Teej's call, since §5 is imported verbatim.
  >
  > **Verification.** Two pty tests (the staleness poll, the query), one new
  > `phosphor-ui` tree test, one new client test. Planted and caught: a `Woke`
  > that does nothing — which fails the staleness poll by name, and which passed
  > before the fixture was fixed.

- [x] **T052 · MCP server from the registry**
  `rmcp`, generated from T020 so the vocabulary cannot drift.
  *Done when:* Claude can call an editor tool and the same capability works from Steel and CLI,
  **`apply-edits` among them** — a batch applied as one undo group, which is the shape an agent
  writes through. *Needs:* T020, T050

  > **The `apply-edits` half is an arm this task owes**, not a new task. It is declared and
  > unapplied because there is no caller until there is a session — `T029`'s tree already
  > supports it (`record_batch`) — and it is recorded in `scripts/lint-action-arms.sh`'s RECORDED
  > table with that reason. This is the task where the caller appears, so the debt is filed here
  > rather than in the *Arms owed* section below.

  > **Built 2026-08-21.** `crates/phosphor-agent/src/mcp.rs` is the server,
  > `phosphor --mcp` serves it on stdio, and `door::mcp_call` dispatches it —
  > through the same `answer` the CLI door runs, so *"the same capability works
  > from Steel and CLI"* is structural rather than a thing to keep in step. A
  > door that disagreed with another about a capability would have to disagree
  > with itself first. `rmcp` 3.1.4, Apache-2.0, `rust-version` 1.88 — the
  > workspace floor exactly.
  >
  > **Nothing in that module is a list**: `tools()` is one `map` over
  > `capabilities()`. `--mcp` is a **flag** and not a subcommand, because the
  > subcommand namespace is generated one verb per capability and
  > `lint-one-registry.sh` holds the CLI module to owning no name of its own.
  >
  > **`parity.rs`'s MCP third is a live round-trip now**, which is what that
  > file's own header said this task would do. All 218 tools are called on one
  > server and each answer must name **that row's** task; a tool dispatching to
  > a neighbour names the neighbour's and fails by printing both. One server for
  > the whole walk rather than the CLI third's 218 process launches — MCP is a
  > session, so the walk went 1.14 s → 2.6 s instead of joining the CLI third at
  > ~158 s. The schema checks stayed: deleting them to make room would have
  > traded a precise message for `is_error: true`.
  >
  > **The gap that made `apply-edits` a defect rather than an omission.** There
  > are two appliers — `AppHost::apply` is the VM's and `Editing::act` is the
  > loop's, which holds the rope — and nothing joined them, so *every*
  > buffer-domain capability typed at `:repl` answered `#refused · not built
  > yet` including ones that shipped three phases ago. Keys reached the buffer;
  > scheme never had. `Intent::Act` is the join, in the shape the eleven intents
  > beside it already have, and it is **one capability wide** on purpose: a
  > blanket `Action::Buffer(_)` arm would turn every unarmed capability's honest
  > refusal into `#ok` for something that never happened, and that refusal is
  > what the CLI and MCP doors are *for* at this phase.
  >
  > **Two things the tests only found by being wrong first**, both recorded
  > where they happened:
  >
  > * **The ordering test proved nothing at first.** `apply-edits` applies
  >   last-first so earlier spans stay valid, and the test used two edits on
  >   *different lines* — which survives either order, because a `Span` is
  >   line-and-column and is resolved against the document as it stands. It
  >   **passed with the sort planted front-to-back**. Two edits on one line is
  >   where the order is load-bearing, and that is the pair it uses now.
  > * **The undo group is free, and the comment claiming otherwise was wrong.**
  >   Deleting the arm's `begin`/`commit` pair left the test green: the boundary
  >   is the input machine's (`History::CommitUndoGroup`, per `Timeline::close`),
  >   so every edit made while applying one Action is already one group. The
  >   assertion stays because it is this task's acceptance; the pair stays
  >   because it buys one fork transaction and one highlight-cache reset instead
  >   of N. Neither is what the other was claimed to be.
  >
  > **Verification.** Six unit tests over the tool list and the schema walk, the
  > 219-capability live parity walk, and a pty test for the batch. Planted and
  > caught: a `row_for` that dispatches every tool to the first row, a
  > front-to-back edit order. Also caught by measurement rather than by a
  > planted defect: `enable_time` missing from the server's tokio runtime, which
  > panics on the first `tools/call` — *after* the handshake, so the server looks
  > healthy right until it is asked to do something. `just gate` green.

- [x] **T053 · `phosphor/declare-review-block`**
  The review-block signal as an MCP tool call carrying file+range list and per-group annotations
  (Q6). Routed through the registry, so Steel and CLI can declare one too.
  *Done when:* a declared block becomes a grouped set of unseen markers + a notification.
  *Needs:* T052, T041

  > **Built 2026-08-21.** `store::Shared::declare_block` declares each group's
  > spans through the same `declare` `declare-regions` uses — so a block's
  > markers *are* §7 regions, counted by the statusline and drawn by the gutter,
  > with no second path — and records which regions arrived together under a
  > `BlockId`. `review-blocks` answers off the same store at the same revision,
  > so a block and its markers cannot disagree about what landed.
  >
  > **Ids, not spans.** A block holds `RegionId`s; the region *is* the span.
  > A block carrying its own copy would drift from the markers the first time
  > `T043`'s reanchor moves one.
  >
  > **`Actor::Claude` whoever calls it, and that is the capability rather than
  > a shortcut.** §7 rules that *"your own edits never create regions"*, and
  > `FileGroup` carries no author to disagree with — unlike `RegionSpec`, which
  > does, because `declare-regions` is the general verb. A review block *is* the
  > claim that claude wrote these spans. Declaring one with `request.actor`
  > would make the same call from `:repl` produce zero markers and a
  > notification about them, which is the worst of both — measured, since that
  > is exactly what the first version did.
  >
  > **The notification needed a channel that did not exist.** A door answers its
  > *caller*: `Receipt::note` reaches the shell that ran the verb or the agent
  > that called the tool. A review block is news to the person at the terminal,
  > who made no call at all. `Intent::Say` is that channel — a sentence from the
  > far side of the VM onto §6's notice row, in `1b`'s own words: `review ready
  > · retry logic — 1 file(s), 2 region(s)`.
  >
  > **And it had to be parked rather than drawn.** `6b`'s REPL owns its whole
  > frame — `draw` returns early for `Composed::Frame`, statusline included — so
  > a notice set while the REPL is up is drawn to nobody, which is every
  > declaration a `:repl` test can make. It waits on `Shell::saying` for the
  > first frame with a notice row. **Measured**: the pty test asserted markers
  > and the query, was named *"and a notice"*, and went green with
  > `Intent::Say` deleted — because the notification it was named for had never
  > been visible to assert. Two more things that finding cost, both now written
  > into the test: a notice borrows the *whole* statusline row, so waiting for
  > `NORMAL` after it lands waits for a mode chip that is not drawn, and the
  > unseen counter has nowhere to go until a keystroke clears the sentence.
  >
  > **Verification.** One pty test driving the Steel door — the point being that
  > the capability is *routed through the registry*, one row and three doors —
  > asserting the query, the notice and the markers. Two planted defects caught:
  > a block that declares no regions, and a block that says nothing.

- [x] **T088 · Pane manager — splits and focus** 📌
  `T054` calls the transcript *"a pane, not a float — splits, holds focus like a window, survives
  float churn"*, and nothing was tasked to provide panes. This is that: the split/focus model in
  the binary's event loop, pane kinds (buffer, transcript, and in v1.5 claude-built), focus
  routing that survives a float opening and closing over the top, and the rule from Design
  Language §9 that **panes never dim each other** — only floats dim what's behind them.
  Placed at S6 because the transcript is the first surface that forces a second pane. If the
  files picker (`T046`) ever opens results into a *new* pane rather than replacing the current
  buffer, this moves to S5 — decide that when `T046` lands, and note the answer here.
  *Done when:* two panes split, focus moves between them, opening then closing a float returns
  focus exactly where it was, **and the four `[S6 / "T088"]` capabilities have arms** —
  `split-pane`, `focus-pane`, `close-pane`, `resize-pane` (`action.rs:630-647`) — **plus the
  `panes` query** (`query.rs:410`). *Needs:* T019, T015

  > ### Built 2026-08-20, in twelve steps — what the five clauses came to
  >
  > *Done when* asked for five things. Each is met and each has a test that fails without it:
  >
  > 1. **Two panes split.** `PaneTree` is a pure data structure — no terminal, no `Editor`, no
  >    theme and no `Rect` — so split, close, resize and direction are unit-tested with no geometry
  >    at all, which is what let the model be right before a pixel existed.
  > 2. **Focus moves between them.** `Panes::resolve` answers all five `PaneRef` variants off the
  >    tree; `Next`/`Prev` walk its order rather than the map's, because `<C-w>w` cycles windows as
  >    they are *arranged*.
  > 3. **Opening then closing a float returns focus exactly where it was.** The plan proposed a
  >    focus-return stack. **It needs none, and a stack would have been wrong**: not one float verb
  >    carries a `PaneRef`, so none *can* name a pane, and a verb that cannot name a pane cannot
  >    move focus to one. Had something else moved focus while the float was open — `focus-pane`
  >    from a keymap or an agent — snapping back would have undone what was asked for.
  > 4. **The four capabilities have arms.** And they are ordinary arms on `Editing::act`, which the
  >    plan said was impossible because they *"mutate the tree an `Editing` was borrowed out of"*.
  >    That stopped being true at step 4c: an `Editing` comes out of `Buffers` and the tree lives in
  >    `Panes`, two structs.
  > 5. **The `panes` query.** Plain data, not a view tree: the row says *"the pane tree, with which
  >    one has focus"*, so it answers what the arrangement **is** rather than how to draw one.
  >
  > **What the build found that no plan predicted.** Making `Resources::editor` a real lookup
  > exposed a composition naming `BufferId(1)` while `Buffers` minted from zero — an id that had
  > named nothing, unnoticed, because nothing read it. The state column went blank the instant the
  > door started looking, and a *screen* test caught it, which is the only kind that could.
  >
  > **And the rule that shaped every step**: three times a field or a variant the plan asked for had
  > no reader yet, and `dead_code` under `-D warnings` refused it. *A ticked task may not ship
  > something no keystroke can reach* turns out to apply one layer down, to a struct field, and the
  > compiler enforces it for free.
  >
  > Twelve commits, `e2ce0db`..`HEAD`. `WINDOW-F-PLAN.md` carries the per-step record.

  > ### Teej's three rulings, 2026-08-20: **follow nvim and telescope**
  >
  > `WINDOW-F-PLAN.md` opened three decisions the tree cannot make. All three are ruled the way
  > neovim and telescope answer them, and the first two turn out to be **one** decision.
  >
  > **(1) and (3) — one `Editor` per `BufferId`, and the viewport is per *pane*.** This is
  > neovim's buffer/window split exactly: a *buffer* is the text, the undo history, the marks and
  > the language server attachment; a *window* is a view onto one, with its **own cursor and its
  > own scroll offset**. `:sp` on an open file gives two views of one buffer — read the top while
  > editing the bottom — and that is the thing people split *for*.
  >
  > **The plan ruled the first half right and the second half wrong**, and the two questions it
  > filed separately are the same one. A shared viewport makes "the same file in two panes" a
  > feature nobody wants: two halves scrolling in lockstep, which the plan's own note called
  > *"reads as a bug on screen"*. So: **allowed, with independent viewports.** Refusing would be
  > strange in a vim-shaped editor, and sharing would be worse than refusing.
  >
  > **What this does *not* cost.** `Node::Buffer`'s declaration says *"It carries no viewport —
  > invariant 3 puts the viewport behind an Action, and a redraw may never move it"*
  > (`view.rs:437-441`). Following nvim does not break that sentence; it makes it more true. The
  > viewport still moves in exactly one place — `buffer_view::apply_scroll`, whose doc calls
  > itself *"the single place a buffer's viewport moves"* and which
  > `the_viewport_moves_from_exactly_one_place` enforces by reading the file. What changes is
  > *whose* viewport moves, which is what `ViewAction::Scroll`'s `pane: PaneRef`
  > (`action.rs:428-431`) has claimed all along. The contradiction the plan found is resolved in
  > the `PaneRef`'s favour.
  >
  > **And it needs neither a fork patch nor a protocol change**, which is what made the plan rule
  > the other way. The viewport lives in the vendored `Editor` (`editor.get_offset_y()` /
  > `set_offset_y`), and splitting that into document and view would be permanent `VENDOR.md`
  > debt against a fork pinned by SHA — correctly ruled out. The third path the plan did not
  > consider: **the host owns each pane's viewport and hands it down through the door the
  > `Resources` trait already is.** `Resources` gains `viewport(&self, pane: PaneId)`, beside
  > `picker(&SourceId)` and `completion()` — same seam, same argument, still no `&mut`. And
  > `BufferView` gains a `.viewport(…)` builder, the same shape as `.fill(…)` and for the
  > identical reason that builder gives: *"the caller decides, because deciding needs the
  > environment and this crate reads none."* `Node::Pane` already carries `pane: PaneId`, so the
  > interpreter knows which pane it is drawing. The tree still carries no viewport — literally
  > true, because a door is not a prop.
  >
  > *The cursor is the same shape and is the larger half*, since selections and operators read
  > it; whoever builds this should expect the cursor to cost more than the offsets and should not
  > assume one commit covers both.
  >
  > **(2) The picker replaces the focused pane; splits are explicit keys.** Telescope's defaults —
  > `<CR>` opens in the current window, `<C-v>` vertical, `<C-x>` horizontal. The vocabulary
  > already agrees: `AcceptHow::Open` is documented *"`↵ open` — open it in the focused pane"* and
  > `AcceptHow::Split` is *"Open it in a new split"* (`request.rs:1392-1402`). So this ruling adds
  > no vocabulary — it answers the question `T088`'s entry was asked to answer, and turns
  > `accept_picker`'s `AcceptHow::Split => declined("one pane until T088 splits it")` into an arm
  > plus two picker bindings. **Not a new pane by default**: a picker that split on every `↵`
  > would make finding a file a window-management decision, which is the thing telescope's
  > defaults exist to avoid.

  > ### Rulings (b) and (c), 2026-08-20 — these two the tree settles, not Teej
  >
  > `WINDOW-F-PLAN.md` §0 filed three rulings. The block above is (a), together with §6's picker
  > question, and both of those were Teej's because the tree genuinely could not make them. These
  > are the other kind: the tree already answers them, and what was missing was writing the answer
  > down where the builder reads it. Every citation below was re-read in the worktree on
  > 2026-08-20.
  >
  > **(b) `collapsed: BTreeSet<RegionId>` is per *buffer*, and ruling (a) forces it.** The set is
  > an `Editing` field (`main.rs:4977`) and its own doc sends you to `Editing::collapse`
  > (`main.rs:6327`) for why it is not in the fork: *"The fork's own toggle is one flag for the
  > whole editor (`virtual_text::set_visible`), and this capability addresses a rail by owning
  > region… the host installs the row list every frame, so a collapsed owner's rows are simply not
  > in the list it installs."* That install is
  > `virtual_text::install(&mut editing.editor, &rows)` (`main.rs:2644`), and the filter that
  > builds `rows` is the read: `Some(owner) if editing.collapsed.contains(&owner) => None`
  > (`main.rs:2636`). So a collapse is expressed **by what is in the `Editor`**, and ruling (a)
  > puts one `Editor` behind each `BufferId`. Two panes on one buffer therefore cannot hold two
  > collapse sets without either a second row list inside one editor or the per-region flag inside
  > the fork — and that flag is the permanent `VENDOR.md` debt ruling (a) has already declined.
  > Per buffer, and mechanically so rather than by preference.
  >
  > *The one place this shows through, and it is worth stating because it looks like a
  > contradiction:* a per-pane **viewport** is expressible because the host owns it — that is the
  > `Resources::viewport(pane)` door above. A per-pane **virtual-text list** is not, because the
  > host hands it *to the fork* and the fork has exactly one. Same fork, opposite answers, and the
  > difference is only who holds the value.
  >
  > **(c) The buffer-swap reset list is written down, and it is two fields short today.** The swap
  > is the `else` arm of the `open-file` drain — where a *different* file arrives —
  > `main.rs:3061-3121`, its `Ok` body at `:3063-3115`. It rewrites `editor` (`:3069`),
  > re-registers the dirty callback (`:3071`), `timeline` (`:3073`), `depth` (`:3074`),
  > `alternate` (`:3087`) and `file` (`:3088`); drops the completion session through
  > `close_completion()` (`:3103`, body at `main.rs:7172-7176`, clearing `completion`, `offered`
  > and `chosen`); clears `signature` (`:3104`); and re-runs `adopt` (`:3099`), which rewrites
  > `comment_prefix`, `server` and `language` (`main.rs:8569-8578`).
  >
  > **It does not touch `selection_kind` (`main.rs:5029`) or `selection_from` (`main.rs:5035`),
  > and both are facts about a rope that is gone.** `selection_from` has exactly two clearing
  > sites — `ClearSelection`'s arm (`:5370`) and undo (`:6038`) — and `ExtendSelection` reads it
  > as `*self.selection_from.get_or_insert(head)` (`:5358`), which keeps whatever a `Some` holds.
  > `SelectRange` guards its own anchor for containment (`:5329-5331`, *"an anchor outside the
  > range it is the anchor **of** is not that range's anchor, whoever sent it"*) and
  > `ExtendSelection` has no such guard. The swap replaces `editing.editor` wholesale, so the
  > *highlight* goes with the old rope while the *anchor* survives it — and the first `v` plus a
  > motion in the newly-opened file builds a `Selection` from a char offset measured in the
  > previous one. That is the same defect class `CP-4`'s review already found once, documented at
  > `main.rs:9985-9994`: *"a scripted selection left an anchor behind that the next `v` inherited
  > and the next motion extended from."*
  >
  > **So the reset list adds exactly two entries: `selection_from` → `None` and `selection_kind`
  > → `SelectionKind::Char`** (the constructor's own defaults, `main.rs:5178-5179`). Written as an
  > explicit named reset and **not** by constructing a fresh `Editing`: a fresh construction would
  > silently wipe `registers` (`:5027`), `jumplist` (`:5013`), `jump_at` (`:5025`), `store`
  > (`:4959`), `wake` (`:4973`), `picker` (`:4985`), `source_order` (`:5002`), `mode` (`:5060`),
  > `quit` (`:5044`), `falling_through` (`:4953`) and the whole mailbox, and no test covers any of
  > them surviving a swap.
  >
  > **And what must *not* reset, so the list is closed rather than open-ended.** `registers` —
  > vim's are global, and after the session fields move that is a decision rather than a reading.
  > `jumplist` and `jump_at` — an entry is an `AnchorId` the store holds under
  > `store::key_for(&path)` (`Editing::push_here`, `main.rs:6824-6835`), so it names a place in a
  > *named file* rather than an offset in the current one, and clearing the list on a swap would
  > throw away exactly the history `<C-o>` exists to walk. `alternate` — the swap *sets* it
  > (`:3087`) and that is the point of it. `collapsed` — the set has exactly one read site,
  > `:2636`, and it is consulted only for owners `store.covering(&path, at)` answers for the file
  > now open, so another file's collapsed rails are inert rather than stale; clearing the set on a
  > swap would lose every collapse across a `CTRL-^` round trip. And `dirty` needs no reset
  > because the swap is refused outright while it is set — `if !same && dirty.get()` answers
  > `WouldLoseWork` at `:3043`.

  > **The arms were added to this criterion on 2026-08-20, because the gate demands them and the
  > sentence did not.** There are **zero** `Action::Pane` arms in `main.rs` today — the domain
  > falls through to `NotYetImplemented`, which is why every pane verb already refuses by naming
  > this task for free. The moment `T088` is ticked, `scripts/lint-action-arms.sh` requires an arm
  > for every mutation a ticked task declares, so a `T088` that met the old *Done when* exactly
  > would have ticked and immediately failed `just lint`. Found by the design workflow reading the
  > declaration table against the binary.

- [x] **T089 · `TabBar`** 📌
  Chrome strip one of three (Design Language §5), untasked until now, and the plan already
  decided to **build rather than buy** it (`ratatui-comfy-tabs`, 600 downloads). Appears only at
  2+ panes. Flat vim-style: active tab = 2px actor-coloured top rule + bright text, inactive =
  meta-gray, **per-tab unseen counts (`●n`)**. Input is `Vec<TabVM { title, kind, unseen }>`.
  *Done when:* it appears on the second pane and never on the first, and per-tab unseen counts
  track the store. *Needs:* T088, T010, T041

  > **Built 2026-08-21.** `crates/phosphor-ui/src/tab_bar.rs` draws the strip, `compose_tabs` in
  > `crates/phosphor/src/main.rs` builds it, and `Geometry::take_tab_bar` spends the row. The
  > `TabBar` row is gone from `scripts/lint-node-kinds.sh`'s RECORDED table and from
  > `interpret.rs`'s deferred set, both of which fail four ways on a stale entry.
  >
  > **The condition is in two places because it does two things, and only one of them is
  > visible.** `compose_tabs` answers `Node::Empty` below two panes — that is what keeps a tab
  > off the screen. `Geometry::take_tab_bar` declines the row below two panes — that is what
  > keeps the row for the *buffer*. **Measured**: the first pty test asked only whether the word
  > `panes` appeared anywhere, and a `take_tab_bar` planted to spend the row at one pane
  > **passed it**, because the composition still drew nothing into the row and the only symptom
  > was a buffer silently one line shorter. The test now asserts what row zero *is*.
  >
  > **The `2px` top rule has no terminal form and is recorded rather than approximated** —
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §50. §5 asks for a 2px actor-coloured top rule on the
  > active tab and a 1px rule under the strip; §8 fixes the strip at one row; a cell has no top
  > edge. §8 wins, because a row count is a claim a terminal can honour and `2px` is a unit a
  > cell does not have. What the rule carried survives except for the actor colour — bright text
  > on `chrome.statusline` marks the active tab, `●n` keeps its claude green — and with that
  > colour goes the only drawable consequence of `Tab::kind`, which the interpreter's arm records
  > as deliberately unread. §7's *"the machine tracks claude only"* is why it costs one colour
  > and not a distinction: all three `PaneKind`s are claude's work, so the map would be one
  > colour written three ways. Two shapes for getting the rule back are written down there;
  > neither was taken, and `chrome.tab_bar_rule` is for now a §5 colour nothing draws with.
  >
  > **The pane count is derived, and the title rule is the workspace's.** §5's strip ends in
  > `3 panes`; `Node::TabBar` carries no such prop and the Component Breakdown's input spec has
  > no room for one, so the widget counts the tabs — composition's contract is one tab per pane.
  > A title is the path relative to the workspace when it is under it (§5 draws `src/retry.rs`)
  > and the **basename** otherwise, which is vim's own rule and the only one that keeps the strip
  > usable: an absolute path out of a temp directory is fifty cells before it says anything, and
  > two of them push §11's second rung on an 80-column terminal.
  >
  > **Shedding is two rungs and the active tab is below both.** The pane count goes first — its
  > whole content is recoverable by counting the tabs — then the run drops tabs from the *left*
  > until the active one's right edge fits. An active tab wider than the strip is clipped, since
  > there is no rung below *"show the tab you are looking at"*.
  >
  > **Four planted defects, four caught**: a `take_tab_bar` that never takes a row and one that
  > takes it at one pane (both pty), a count that is a constant rather than the store's (pty,
  > through `gs` marking a region seen while both tabs watch), and a `compose_tabs` that composes
  > at one pane (unit — the composition half draws nothing, so only a test of the tree sees it).
  > `just gate` green at 1,432 tests.

- [x] **T054 · TranscriptPane**
  **A pane, not a float** — splits, holds focus, survives float churn. Turn list, prompt lines
  `❯`, prose, tool rows, seam markers. Folds by turn. Streams during Working.
  *Done when:* screen `1b` reproduces **from a keystroke** — the binding that opens the pane
  opens it in the running binary. *Needs:* T050, T088

  > **Built 2026-08-21.** `crates/phosphor-ui/src/transcript.rs` is the widget —
  > a header, then one block per turn: the `❯` prompt, claude's prose, tool
  > rows right-aligned on their counts, and the seam. `Node::Transcript` composes
  > as `SPC t`'s split (below `T088`'s `split-pane`, one call, because the
  > capability takes what the new pane holds) and reproduces `1b` in the running
  > binary — asserted keystroke to grid, header through tool row.
  >
  > **`Node::Spinner`/`Node::Elapsed` are re-recorded again, and this time
  > with no creditor.** `T051` had recorded them against `T054` on the guess
  > that *"a streaming turn row is the surface that composes them
  > standalone"* — and ticking `T054` showed that guess wrong the same way
  > `T051`'s own creditor was wrong: the transcript animates a running turn's
  > spinner and elapsed counter inline, off `Turn::since`, through the shared
  > `SPINNER_PERIOD_MS` cadence so it cannot drift from the statusline's — but
  > that is a second *arm*, `TranscriptPane::row`'s own, not a composition of
  > the tag `Node::Spinner` nests. Two tasks in a row guessed a creditor and
  > were wrong for the identical reason `Gutter` has none: the capability
  > ships twice over and the node kind ships never. Recorded with no task now,
  > which is the honest answer until a surface actually wants a *standalone*
  > spinner with no session and no turn behind it.
  >
  > **The seam this task's own arms had to close.** `AcpAgent` spawns an agent
  > with its own connection; the SDK's `SessionMessage` from `read_update()`
  > carries the raw `Dispatch` for everything but the stop reason, and nothing
  > before this turned one into an Action. `transcribe` is that turn — prose and
  > thought chunks become `session-prose`, `ToolCall`/`ToolCallUpdate` become the
  > three tool-call verbs — and a real defect surfaced in getting it right: the
  > catch-all arm for `SessionMessage`'s `#[non_exhaustive]` growth was written
  > *above* the two real ones, where a wildcard silently ate every notification
  > before the specific arms could see it. The transcript came out with a prompt
  > line and no prose. Order, not logic, and caught by the pty test rather than
  > a unit test — the crate-level tests build the Actions directly and never
  > exercised the match order that mattered.
  >
  > **A tool call's name is the agent's problem and its id is this editor's.**
  > `request.rs`'s ids are opaque non-negative integers by construction; an
  > agent's tool-call name is a string it invents. `Shared::name` is the map,
  > stable for the session, and it is why `tool-call-progress` and
  > `tool-call-completed` — which the wire correlates by the agent's string —
  > can still reach the row `tool-call-started` created under this editor's own
  > number.
  >
  > **A focused pane no longer has to hold a buffer, and that was a real crash
  > waiting for this task.** The loop's own line read
  > `panes.at(focus).buffer.expect("… until step 11 gives it anything else to
  > hold")`, and `SPC t` is exactly that something else: pressing it took the
  > editor down mid-test with an I/O error on the next keystroke, the shape a
  > `.unwrap()` panic takes against a pty that has already gone. `held` resolves
  > to the nearest pane that *does* hold one now, falling back to any open
  > buffer, and stays total.
  >
  > **`:transcript` and `SPC t` are the one capability the row already
  > names** — *"`:transcript` is this, not a separate capability"* — reached two
  > ways: `set-pane-content` alone turns the focused pane into the transcript
  > and `:transcript buffer` turns it back (`1b`'s *"closes back to full
  > buffer"*), and `SPC t` composes it with `split-pane` the way `<C-w>v`
  > composes `split-pane` with `focus-pane`, so the code stays on screen the way
  > `1b`'s drawing does.
  >
  > **Verification.** The transcript widget's own row/column arithmetic, the
  > pty test pressing `SPC t` and reading `1b` off the grid, and two planted
  > defects caught: `session-prose` dropped, a tool call never recorded. `just
  > gate` green at 1,462 tests.

- [x] **T055 · Markdown prose behind the gate**
  Via the vendored fork (T004). **Plain-text path must stay readable with the gate off.**
  *Done when:* both paths render acceptably. *Needs:* T004, T054

  > **Built 2026-08-21.** `crates/phosphor-ui/src/prose.rs` — one function, two
  > implementations, chosen by the `markdown` feature. The transcript calls it
  > and does not know which one it got, because a caller that branched would be
  > a second place for the two paths to diverge.
  >
  > **The plain path was not readable and the comment said it was.** `T054`
  > drew prose as one row per `\n`, written with `set_stringn` — which clips at
  > the pane edge — under a comment reading *"**Wrapped, not truncated.** …
  > A transcript that clipped claude's sentences at the pane edge would be
  > unreadable at any width."* That comment shipped with the code that did
  > exactly what it forbade, and it took this task's own acceptance to measure
  > it. The tree won and the comment was the bug. **This is the second time in
  > two tasks that prose about the build outlasted the build**; §54 was the
  > first.
  >
  > **Wrapping is `float::wrap_prose`, not a fourth copy.** The same helper the
  > float bodies use — whose own doc block already named this task:
  > *"rendering markdown properly is the transcript's job at `S6`"*. Its rules
  > come along for free and are the right ones here: a token longer than the
  > width gets its own row rather than being cut mid-way, a blank line is a
  > paragraph break and survives, and `cols == 0` hands the source back so a
  > degenerate width cannot loop.
  >
  > **`Row::Prose` carries a `Line`, not a `String`.** Both paths answer styled
  > lines, so §11's grouping counts the same rows either way and the renderer's
  > own tones — a heading, a fenced block, inline code — reach the buffer
  > instead of being flattened to one colour at the draw.
  >
  > **The fork's fifteen-slot `RichTextTheme` is bridged, not defaulted.**
  > Every colour comes from Design Language §1 through `Theme`; the trait's own
  > defaults are `Color::White` and `Color::Black`, which would be a sixteenth
  > palette nobody chose and precisely what
  > `scripts/lint-no-literal-colours.sh` exists to stop. The JSON slots belong
  > to the fork's tree view, which the transcript does not draw — answered from
  > the palette anyway, because the trait requires them.
  >
  > **The gate-off rendering is the source and that is the honest fallback.**
  > What an ACP agent streams is markdown whether or not anything renders it, so
  > `# Retry logic` reading as `# Retry logic` is the source showing through —
  > not a half-parse, and not a degradation. There is a test for each side
  > asserting exactly that difference.
  >
  > **Verification.** Four unit tests over `prose::lines` — a paragraph wraps to
  > its width *and* keeps every word (a width check alone passes against a
  > renderer that truncates every row), zero width is answered rather than
  > divided by, and one test per side of the gate on the same source. One
  > keystroke test through the transcript at 120 columns, needled on the
  > paragraph's **last** word and on the row it lands on being later than the
  > first — the only thing that distinguishes wrapping from clipping. Two
  > planted defects, two caught, one per path: the plain path reverted to
  > `T054`'s behaviour failed both the unit test and the pty test, and a gate
  > that rendered nothing failed the markdown half. `just gate` and
  > `just hack` green.

- [x] **T056 · OSC 8 tool-row jump links**
  *Done when:* clicking a tool row jumps to the file and range, on the primary terminal.
  *Needs:* T054

  > **Built 2026-08-21.** `transcript::link` writes the sequence, `jump_uri`
  > builds the URI, and `tool-call-started` grew the two fields that make a
  > jump possible. **The press itself stays `CP-6`'s** — `docs/TASKS.md`'s own
  > Tier-3 table says *"links may render, but nothing can click one"* — so what
  > ships is everything up to it, proven at the byte.
  >
  > **`path` is not `target`, and merging them would have been the bug.** ACP
  > carries a `title` — what the row *says* — and a separate `locations` list of
  > absolute paths. A real agent's title is a sentence: `7b`'s own mockup draws
  > *"Replacing the reconnect loop's hand-rolled sleep"*. A link built from the
  > title would point at a file named after a sentence. `1b` draws a path in the
  > title, which is exactly the coincidence that makes the mistake easy.
  >
  > **The whole sequence is one cell's symbol, and that is the design.** OSC 8
  > is stateful: `ESC]8;;uri ST` opens a link and everything printed until the
  > empty closer belongs to it. Ratatui paints by diffing two cell grids and
  > emitting only what changed, so an opener and a closer in separate cells are
  > two independent decisions — and the frame where the URI changes but the last
  > character does not prints the opener, skips the closer, and leaves the link
  > running across everything drawn after it. **That is not a rare race**; it is
  > what happens the first time claude edits a different file whose name ends in
  > the same letter. One cell can only be emitted or skipped whole.
  >
  > **Ratatui 0.30.1 added the two options this needs, for this.**
  > `CellDiffOption::ForcedWidth` because the anchor's symbol measures dozens of
  > columns and occupies as many as the text does, and `CellDiffOption::Skip` on
  > the cells it covers so nothing paints into the middle of a sequence. The
  > upstream doc says so in as many words: *"prevent the buffer from overwriting
  > a cell that is covered by something from an escape sequence, such as
  > graphics or links."*
  >
  > **Clipped before the sequence is built, never after.** The tail of the
  > string is the closer; truncating the finished thing is the one failure that
  > escapes the pane it was drawn in.
  >
  > **The link is underlined**, which is
  > `phosphor_core::view::Emphasis::Underline`'s own definition — *"an OSC 8
  > jump link in the transcript"* — and the only affordance a link has where
  > hovering costs nothing and clicking is the verb.
  >
  > **`Editor::raw` is a new reader in the pty harness and the only one that
  > skips `printable`.** An escape sequence occupies no cell, so a grid cannot
  > carry this claim; every other assertion in that file goes through the grid,
  > and its doc says why — needling the raw stream for *text* is what
  > `OPEN-QUESTIONS.md` §54 records going wrong.
  >
  > **Two footer hints were claiming keys that do not do that.** `1b` draws
  > `q close` and `q` is vim's macro-recording key in this build, bound in
  > `keymaps.scm` in normal mode everywhere; the footer says `<C-w> c close`
  > now. `1b`'s `↵ jump to file` is gone rather than corrected, because a
  > keyboard jump needs a focused row inside a transcript pane and no task owns
  > that selection model — recorded at §56 with the ruling left to Teej.
  > `T088`'s lesson is why this was worth stopping for: a verb with an arm, a
  > passing gate, and nothing bound to it survived three windows.
  >
  > **Percent-encoding is not done, deliberately.** A path with a space, a `#`
  > or a `%` produces a URI a strict parser reads wrongly. Encoding is a table,
  > the table is a crate, and a hand-rolled subset is the almost-right this
  > build's lints exist to catch — so it is a dependency decision and it is
  > `spine`'s. §56 records it.
  >
  > **The task owns a verb I had not noticed, and two lints found it in
  > sequence.** `goto-location` is declared `[S6 / "T056"]` — *"opens a file at
  > a position — a picker accept, a transcript tool row, an OSC 8 link"* — and
  > ticking the task without it made `lint-action-arms` fail by name. Arming it
  > in `Editing::act` then made `lint-capability-bindings` fail, because nothing
  > in `runtime/` names it. And **arming it was still not enough**: running
  > `(goto-location! …)` at the REPL answered
  > `#refused · not built yet — T056 builds it` with a clean arms lint, because
  > every one of its three callers arrives through a *door* — which is
  > `AppHost::apply`, not the loop's applier. It is the second capability on
  > that forwarding list, and the list stays one capability wide for the reason
  > `T052` gave it: a blanket arm would turn every unarmed verb's honest refusal
  > into a `#done` for something that never happened. Same shape as
  > `discover-sessions` in `T057`, one applier the other way round.
  >
  > It is `EMITTED` rather than bound, and the reason is the capability's own
  > sentence: a click lands in the *terminal*, which resolves the `file://` URI
  > itself; a picker accept is the picker's binding; `open-file` is the one a
  > person types. None of the three is a key.
  >
  > **Verification.** Four widget tests asserting the exact bytes — the whole
  > sequence in one cell, the declared width being the twelve columns
  > `src/retry.rs` occupies rather than the sixty its symbol measures, every
  > covered cell skipped *and* the column after it drawable again, a call with
  > no file drawn as plain text, and a pane too narrow clipping the text while
  > still closing the link. One keystroke test through a real pty, reading the
  > raw stream for the opener with the file and line the agent gave and for the
  > closer immediately after the text. The toy agent sends `locations` now.
  > One more keystroke test drives `goto-location` through the REPL — the door
  > a pty can reach — and asserts the *position*, because landing at the top of
  > the right file is the wrong answer drawn convincingly. Four planted defects,
  > four caught: an anchor that stops declaring its width, covered cells left
  > paintable, a URI that forgets the line, and a jump that forgets the
  > position.
  >
  > **Two stuck children on the way, both `leave_by`'s untimed `child.wait()`
  > and both my test's fault** — `T058`'s lesson arriving twice more. Opening a
  > file closes the REPL float over the pane, so `(close-repl!)` sent after the
  > jump is eleven normal-mode keys of which `o` is *open a line*: the editor
  > sat in INSERT where `ZQ` is two more characters. And waiting for
  > `elsewhere.txt` matched the REPL's *echo* of the path being typed, one frame
  > before anything opened. The test presses `q` — the REPL footer's own key —
  > and needles the target file's contents, which are in neither the form nor
  > the echo. `just gate` green.

- [x] **T057 · Session lifecycle**
  Cold start (`7d`), attach/adopt/start (`5d`), drop and reattach (`7b`), opening mid-task
  (`2d`). **Editing never blocks on session trouble.**
  *Done when:* all four screens reproduce in the running binary and the editor stays usable
  through a mid-turn drop. *Needs:* T051

  > **Built 2026-08-21.** Ten lifecycle arms in `crates/phosphor/src/main.rs`,
  > `runtime/dashboard.scm` for the dashboard screens, and `7b`'s seam in
  > `crates/phosphor-ui/src/transcript.rs`. Four screens, four keystroke tests,
  > and one of the four is honestly partial — see the last section.
  >
  > **Three of the four screens are one surface.** `7d`, `5d` and `2d` are the
  > same field list with different data, which is what the three drawings say
  > rather than a shortcut taken to draw them: cold start says
  > `session  none running`, discovery adds a list under it, and mid-task swaps
  > the session line, grows an `unseen` row and leads its footer with `]u`
  > instead of `:cn`. `dash/rows` answers all three from `(session)`, `(arch)`
  > and `(review-blocks)`.
  >
  > **Drawn entirely through the spans hatch**, like `:arch` — zero lines added
  > to `phosphor-ui` for three screens, and a better proof of `T080`'s claim
  > than the first one was: `6a`'s numbers move with the store, and these rows
  > move with the *session*.
  >
  > **`7b`'s seam is written by the drop, not by a verb.** `:seam` is the manual
  > form and it earns its place for a pause or a resume, which nothing observes.
  > A connection going while a turn is open *is* observed — by the same
  > life-change block that puts `session lost` on the strip — so the transcript
  > row appears with nothing pressed. The screen's caption is *"the transcript
  > shows the seam honestly"*, and a transcript whose honesty depended on the
  > reader remembering to ask for it would not be that.
  >
  > **The seam is one row in two tones rather than two rows.** `1b` ends a turn
  > with `✻ review ready …` in claude-green and `7b` ends one with
  > `✕ connection lost mid-turn` in trouble-red; `transcript::Seam` carries the
  > flag and both glyph and tone move together, because a red sentence behind a
  > green glyph is the half-truth §5 spends its rules on.
  >
  > **`survived()` is the line under it**, and all three of its clauses are
  > claims this build can make: the buffers are on disk because nothing in the
  > session path writes them, the region count is the store's own, and *may be
  > incomplete* is the truthful modal — a client cannot know whether an agent
  > that stopped answering had finished. The middle clause is dropped at zero
  > rather than drawn as `0 regions`, because a reassurance about nothing is
  > noise on a row that exists to reassure.
  >
  > **The transcript footer is new and `1b` wanted it too.** `T054` shipped the
  > pane with no footer at all; both mockups draw one. It is `KeyHints` at
  > `Density::Footer` — the same widget the floats use — fed from
  > `TranscriptVm::hints`, so *what a transcript offers* stays the host's
  > question. The strip is taken off the room **before** §11 groups the turns,
  > since a footer painted over the last turn after the fitting had promised it
  > would fit is the *"a list that stopped mid-turn"* failure one row lower.
  >
  > **A real bug the verbs exposed.** The loop's option-driven attach compared
  > `agent-command` against `Shell::agent` — *what is attached*. `:cn` sets that
  > without the option moving, so the very next frame read *"the option changed
  > to nothing"* and stopped the session the verb had just started.
  > `Shell::wanted` is the last option seen now and `Shell::agent` is what is
  > attached; one field could not tell those apart.
  >
  > **`discover-sessions` is armed in `AppHost::apply`, not `Editing::act`**,
  > because a float surface body runs in the VM — which reaches that applier and
  > not the loop's. It needs no buffer and no session handle, so there is
  > nothing to reach for.
  >
  > **`5d`'s list is a branch nothing takes.** Its two rows are a tmux pane and
  > a headless socket; the first needs tmux control mode (v1.5) and the second
  > needs a socket transport, and `T050`'s client speaks stdio to a child it
  > owns. There is nowhere to look, and a guessed row is one `↵` could not
  > adopt. `discover-sessions` says so in its note and `adopt-session` refuses
  > by *target* rather than by task — the verb is built and the handle is the
  > thing that does not exist.
  >
  > **`7d`'s `repo` and `history` rows and `2d`'s `vcs` and `last` rows are
  > absent, and that is the honest rendering.** `vcs jj · trunk@a4f2 · clean`
  > needs `vcs-status`, which is `T071`; `last cargo test ✓ 34 passed` and
  > `history —` need the timeline, which is `T073`; `repo … · 214 files` needs a
  > file count no capability answers. All of them answer `NotYetImplemented`
  > today. A row reading `vcs —` would be `dashboard.scm` claiming to have
  > looked. **This is the part of *"all four screens reproduce"* that is not
  > true yet**, it is named here rather than in a green tick, and each row
  > appears the day its query stops refusing without an edit to this file.
  >
  > **§54 is closed and it was never a bug.** The task was held back a session
  > by a probe reporting that `:cn` reached the client and never finished the
  > handshake while `(set-option! "agent-command" …)` with the identical command
  > did. The probe searched the **raw pty byte stream**; a settled statusline is
  > written by ratatui's diff renderer in pieces separated by cursor moves, so
  > `claude idle` was on the screen and was not a substring of the bytes that
  > drew it. Five theories were ruled out correctly and the conclusion was still
  > wrong, because every one of them was about the build and none was about the
  > instrument — the same mistake as §53, five days earlier. `Editor::shown_on_grid`
  > exists for exactly this and the probe was hand-rolled Python that used
  > neither it nor the harness.
  >
  > **§55 is raised and is Teej's.** `7b` draws `:ca reattach` and `2d` draws
  > `:tr transcript · :c claude`, while Design Language §6 — quoted in
  > `KeyHint`'s own doc — names `:ca` as its counter-example. The build follows
  > the rule, as `status_line` already did; the drawings disagree and the ruling
  > is not a lint's to make.
  >
  > **Verification.** Four keystroke tests. `7d` from `:dashboard` — `none
  > running`, the three verbs, `:reattach` declining by name — and `:cn`
  > attaching, read off the composed grid in two waits because the
  > `claude attached` notice holds the whole row until a keystroke retires it.
  > `2d` with a declared review block — `2 regions in 1 file`, the block's title,
  > and the footer leading with the work. `7b` with a new `drop` fixture mode
  > that answers a prompt and exits before any stop reason, which is the only
  > mode where a turn is still open when the connection goes. Editing through a
  > mid-turn drop. Five planted defects, five caught — and the fifth caught the
  > *test*: a bare needle on `reattach` went green with the footer hint deleted,
  > because the statusline three rows below already said
  > `✕ session lost — :reattach`. The assertion reads the footer's own row now.
  > `just gate` green.

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

- [x] **T058 · PromptLine**
  The `:` line. `⚓` anchor chip when a selection rides along — visual-select, hit the prompt,
  file and range ride automatically. Routes to command parse or Claude message. Ex-style
  history.
  *Done when:* screen `1c` reproduces **from a keystroke** — pressing `:` in the running binary
  raises the line, anchor chip included. *Needs:* T050

  > **Built 2026-08-21.** `crates/phosphor-ui/src/prompt.rs` draws
  > `Node::Prompt` — **the demolition [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)
  > §13 scheduled.** That ruling let the ex row be built from `Node::Line` and
  > `Node::Label` in the binary, *"scaffolding with a demolition date"*, and
  > named this task as the date. `main.rs` composes no prompt out of primitives
  > any more.
  >
  > **The anchor resolves when the prompt opens, not when it is submitted.**
  > `Target::Selection {}` is a *question* — "whatever is selected" — and the
  > selection is gone by the time you finish typing, so the chip has to name a
  > range that will still be true. `1c`'s caption is the whole feature.
  >
  > **The range is inclusive**, because that is what a line range means. A
  > line-wise selection of 2–4 ends at the offset that *begins* line 5, so the
  > raw conversion reads `2–5` and names a line nobody selected; vim's `'<,'>`
  > is inclusive and `1c` draws `19–21`.
  >
  > **`1c`'s two rows ship.** It is the only screen in the set that draws a
  > prompt at all, and it draws one below a statusline that is still there —
  > `Geometry::prompt` takes that row when the prompt carries an anchor, and an
  > unanchored `:` keeps vim's borrowed last row, which is every other screen
  > and every other pty test in the file.
  >
  > **`shown_path` is one rule with two callers now.** `T089`'s tab titles
  > established it and the chip needed the same thing; its doc says why it is
  > *not* `store::key_for`, which must keep an outside path absolute or it names
  > a different file.
  >
  > **The line paints no ground**, unlike `tab_bar` and the leader grid: those
  > *are* their strip, and a prompt borrows a row the caller has already
  > painted. Caught by
  > `the_chrome_strip_is_painted_under_the_statusline_and_the_ex_line`.
  >
  > **A day lost to a wrong diagnosis, recorded at §53.** Every pty test that
  > opened the prompt ran to a deadline, four theories were ruled out by running
  > them, and the entry blamed `press`'s frame accounting. It was `leave_by`'s
  > `child.wait()` with no timeout: `ZQ` typed while the prompt is open is two
  > characters on the prompt line, so the child never exits and the test hangs
  > in its own teardown — *after* every assertion in it has passed. The timing
  > said so and nobody did the arithmetic. Two real constraints surfaced on the
  > way and are now written into the tests: `V` after a motion draws two frames,
  > and a `.rs` fixture attaches a grammar and a server that draw their own.
  >
  > **Search is the half this task did not build**, and the refusal says which:
  > a search prompt needs somewhere to search, which is the search machinery
  > rather than the line.
  >
  > **Verification.** Two keystroke tests in the running binary — visual-select
  > then `:` for `1c`'s chip and its two rows, and `SPC c p` / `SPC c s` for the
  > claude prompt — plus four widget tests over the row itself. Planted and
  > caught: an anchor that does not ride along, and a half-open range.
  > `scripts/key_coverage.py`'s `RECORDED` is empty. `just gate` green.

- [x] **T059 · QuestionBody**
  Prose + amber digit options `[1]`–`[n]` + full-command footer. Digits answer only while
  focused.
  *Done when:* screen `4a` reproduces in the running binary and its digits answer while focused.
  *Needs:* T057, T084

  > **Built 2026-08-21.** `crates/phosphor-ui/src/question.rs` draws
  > `Node::Question`, `runtime/asks.scm` composes `4a` around it, and the two
  > ask verbs are armed in both appliers.
  >
  > **The third editor-layer surface, and the first that is not the spans
  > hatch.** `:arch` and the dashboard are `view/spans` because they are
  > drawings; `4a` is ordinary chrome — a needs-you float with a header, a body
  > and a footer — which is what `T084`'s primitive is *for*. The body is a real
  > node kind because *"prose, amber digit options, and the full command in the
  > footer"* is a shape three screens share (`4a`, `7a`, `4b`) rather than one
  > drawing.
  >
  > **The float names the ask it shows, and that is load-bearing.**
  > `Resources::ask` is keyed where its four neighbours are implicit — there is
  > one completion list, one picker, one transcript, and there are as many
  > questions as claude has asked. A float composed for ask 8 draws ask 8
  > whatever has arrived behind it, so answering what you are reading is the
  > same thing as answering what you meant.
  >
  > **Two fields raise the float, and neither applier composes one.**
  > `Shell::asking` is what should be asked and `Shell::asked` is what is on
  > screen; the loop compares them once a pass. That is what lets `enqueue-ask`
  > be armed in *both* appliers without either knowing what a float is — and it
  > has to be armed twice, which is the day's second instance of the same
  > lesson: `(enqueue-ask! …)` at the REPL raised `4a` while `:ask …` answered
  > *"not built yet — T060 builds it"*, because an ex command is a keystroke and
  > a keystroke lands in `Editing::act`. Measured at the terminal. What is *not*
  > duplicated is the write — both call `Shell::enqueue_ask`, and both mint from
  > one counter shared with `AppHost` the way the store already is, because two
  > counters hand two questions the same id the first time both doors are used.
  >
  > **`Node::Question` had no height arm and the symptom was in that code's own
  > comment.** `Ctx::height`'s `_ => 0` gave the float a zero-row body, and it
  > drew a header, two rules and nothing — which is exactly what `Node::Picker`'s
  > arm records happening to *it*: *"this arm was missing for a build, and the
  > symptom is exactly that."*
  >
  > **And it is the first node kind whose height depends on its width.**
  > `NodeBody::desired_height` discarded the width under a comment reading
  > *"nothing in the tree reflows"*, true of all thirty kinds until a paragraph
  > went in a float body. `Ctx::height_at` lists the kinds that reflow and
  > delegates the rest, so there is one height table and not two.
  >
  > **`enqueue-ask` is `T060`'s capability, armed here.** That task's *Done
  > when* is about the **queue** — a question waiting behind a float that has
  > focus, `]!`, the `!` surviving a 40-column shed, one store query behind all
  > three — and none of it is the verb existing. `4a` cannot reproduce without a
  > producer, so the producer lands with the screen and the queueing lands with
  > the queue. `esc later` is honest and incomplete for the same reason: the
  > float closes and the ask stays queued, and nothing yet brings it back.
  >
  > **Waiting outranks working, which is the point of the state.** `4a`'s strip
  > says `! claude waiting` while a turn is still running. What the `!` means is
  > *the next move is yours*, and a strip that said `working` would be truthful
  > about the agent and useless to the person it is drawn for.
  >
  > **The answer goes nowhere yet and the notice says so.** Getting it back to
  > the agent is a wire — ACP's response to whatever asked — and the thing that
  > asks is `T060`'s queue and `T061`'s permission flow. What `4a`'s acceptance
  > is about is that a digit answers.
  >
  > **The task owns a second verb, and the lint named it.** `float-answer` is
  > declared `[S6 / "T059"]` — *"answers the focused ask by digit — `4a`'s amber
  > option digits"* — and the first version of this went straight from the key
  > arm to `answer-ask`, around it. It is the better shape and the vocabulary
  > already knew: **the focused ask is resolved by the verb rather than carried
  > as a parameter**, which is the whole difference between the two. A digit
  > that named an ask id would be a digit that could answer a question you are
  > not looking at. The key arm now decides only *that* a digit was pressed, and
  > `float-answer` delegates to `answer-ask` the same way `float-accept`
  > delegates to `picker-accept`.
  >
  > **Both are `EMITTED`, and the reasons are different.** `float-answer` is
  > *"the float's own key handling"* — `float-accept`'s existing row, and the
  > keymap for a digit has to be Rust because a digit means two things depending
  > on what holds the screen, which `keymaps.scm` has no way to ask.
  > `answer-ask` is emitted by `float-answer`, having resolved which ask is
  > focused.
  >
  > **`answer-ask` is deliberately not armed in `AppHost::apply`.** It is rated
  > `Deny`, so no door reaches that applier with one; an arm there would be a
  > path nothing can take. The first version had one, and the notice it wrote
  > was the only thing keeping the test green — routing the digit through
  > `float-answer` made the arm unreachable and the notice vanished. It lives in
  > `Shell::answer_ask` now, through `Shell::saying`, which is `T053`'s channel
  > and exists for exactly this: a `Receipt::note` reaches whoever *called*, and
  > the caller here is a digit.
  >
  > **The float body is a helper and not an expression inside the surface
  > string**, which is a rule this file learned from `lint-node-kinds.sh` rather
  > than from taste: that lint strips string literals before it looks, so a
  > `(view/question …)` living only inside a `define-float-surface!` body is
  > invisible to it and `Node::Question` reads as a kind nothing reaches. It is
  > also better — the composition is code, so the REPL can call it. **Twenty-five
  > of thirty node kinds are composed now**, and the `Question` row is out of the
  > RECORDED table.
  >
  > **Verification.** Two keystroke tests. `4a` from `:ask` — the needs-you
  > float, the prose wrapping across two rows, `[1]`–`[3]` each carrying the
  > digit that answers it, `! claude waiting` on the strip, an unoffered digit
  > declining **by name** rather than being swallowed, and `2` answering, which
  > closes the float and stops the strip waiting. And the other half of *"only
  > while focused"*: `3j` is still three lines down, and a digit at a float that
  > is **not** a question is not an answer either.
  >
  > **That second case is one the first version of the test could not see, and a
  > planted defect is what found it.** With `Shell::asked` deleted from the gate
  > — leaving only *"a float holds the screen"* — both tests went green, because
  > neither had a non-question float open. Pressing `1` at `:arch` now proves
  > the condition. Three planted defects, three caught. `just gate` green.

- [x] **T060 · The ask queue**
  Per Q9: a question arriving while another float holds focus **sets the statusline `!` and
  waits**. Surfaces when nothing else holds focus; `]!` jumps to it. The queue is a **store
  query, not widget state**, so `]!`, the inbox, and the statusline read one truth.
  *Done when:* asking while a picker is open destroys nothing, and the `!` survives shedding at
  40 columns. **And `apply-workspace-edit` applies**, which is the first arm this queue owes to a
  task that is not its own. *Needs:* T059, T041

  > **Built 2026-08-21.** The queue is derived rather than stored, `esc later`
  > converges, `]!` walks it, Q9's `!` is fed at last, and
  > `apply-workspace-edit` applies — the debt the entry below has been carrying
  > since `S4`.
  >
  > **The queue is a query because it is not a second collection.** Q9 asks that
  > *"`]!`, the inbox and the statusline read one truth"*, and `T059` had a
  > `Shell::asking` field beside the map — two things that must agree are one
  > thing that can disagree. What wants the screen is now `Shell::head_ask`, the
  > oldest ask you have not pushed back, computed from the map; `pending-asks`
  > and `ask` are that same map published, the way `session` and `transcript`
  > already are.
  >
  > **`esc later` needs a deferral set or it does not converge.** Deferring
  > leaves the question *pending* — it still counts toward the `!`, still
  > answers `pending-asks` — and what it stops doing is asking for the screen
  > back. Without that, `esc` closes the float and the very next pass finds the
  > same head still pending and raises it again. **The first version had exactly
  > that bug** and the symptom read as a hang: the float would not close, and
  > the condition at fault was checking only that the ask still existed.
  >
  > **`]!` is a motion and clears a deferral; it opens nothing.** The float
  > follows the queue by the ordinary rule, so *"jump to the pending ask"* is
  > the same thing as *"stop pushing it back"*. **No `[!` beside it**, unlike
  > every other pair in that keymap block: those walk spans in a file and have
  > two directions because a cursor does, and this walks a queue, which has the
  > order you put things into.
  >
  > **`StatusLineVm::ask_pending` carried Q9's own sentence since `T017` and the
  > binary handed it `false`.** *"It sets the statusline `!` flag immediately and
  > waits"* was implemented on the drawing side and on nothing else — the widget
  > had the field, the doc, the suppression rule beside `Waiting`, and no
  > source. It is the queue now. Deferred asks count: pushing a question back
  > does not answer it, and a `!` that vanished when you deferred one would be
  > the editor forgetting on your behalf.
  >
  > **The rating became a mechanism.** `McpPolicy::Ask` means *only the keyboard
  > says yes to this*, and `deliver` answered `needs an ask first — T060 builds
  > the queue` for four windows because there was nowhere to put the question.
  > Now an `Ask`-rated action from a producer becomes one: `held_question` names
  > **who asked and what for**, the action waits in `Shell::held` keyed by the
  > ask, and `[1]` releases it into `Shell::granted` for the loop to run. Keyed
  > rather than a single slot, because two servers can each want a rename while
  > you are reading something else — and for a rating whose whole point is
  > consent, silently overwriting the first is the worst available failure.
  >
  > **The rating is about the action, not the door.** A rename arriving from
  > Steel needs the same yes as one from an LSP client, so `AppHost::apply`
  > queues a question too — the third entry on that applier's forwarding list
  > and the first that does not apply.
  >
  > **§47's four rules, answered where the code is.** `apply-workspace-edit`
  > records and the loop performs, because an `Editing` holds one rope and a
  > rename is edits in several. **What attaches an entry:** this, and nothing
  > else — a file a rename touches becomes a buffer whether or not you had
  > opened it, since the alternative is an edit that silently skipped the files
  > you were not looking at. **What an unattached buffer wraps to:** nothing,
  > and it needed no invention — `soft_wrap::wrap_to` runs over
  > `panes.tree.layout`, so an entry no pane points at is simply never wrapped,
  > exactly as §47 predicted. **`:wall` and `:q`** were answered by `T088` on
  > its way past, both over the map rather than the focused entry. **Nothing is
  > written to disk here**: the edits land in buffers and the buffers are dirty,
  > which is `[+]` and `:wall` — the same two steps a rename you typed yourself
  > would take.
  >
  > **A test that asserted the old refusal is now the record of the debt.**
  > `a_posted_action_the_mcp_door_asks_about_waits_for_the_ask_queue` checked for
  > the sentence `deliver` no longer says; it asserts the question now, under
  > its own name.
  >
  > **Verification.** Three keystroke tests — a question arriving behind the
  > REPL waits and surfaces when the REPL closes with its options intact,
  > `esc`/`]!` round-trip with the `!` outliving both, and a workspace edit that
  > becomes a question, is granted with `1`, reaches a file **no pane was
  > showing** and is written by `:wall`. Two unit tests over the hold/grant
  > pair, and one widget test walking widths 40–60 for Q9's flag surviving every
  > rung of the shed — the acceptance's own second clause. Three planted
  > defects, three caught: a question that stops waiting, an `esc` that stops
  > deferring, and a `[2]` that grants.
  >
  > **Two pty lessons, both mine and both recorded in the tests.** `\x1b`
  > immediately followed by `j` in one write is the terminal's ESC-prefix
  > ambiguity and arrives as `<A-j>` — the editor said so on its own hint row,
  > which is how it was found. And `shown_on_grid` waiting for `1:1` returned
  > the frame *before* the keys were processed, because `1:1` was already there:
  > a wait has to be on what the keystroke makes **true**, since the grid reader
  > cannot wait for an absence.
  >
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
  >
  > **`T088` saw it and did not take it, 2026-08-20 — the reading is now a citation.** Filed as
  > [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) §47, and `scripts/lint-action-arms.sh`'s row cites that
  > rather than a bare `T088`. The split: **`T088` ships the container and nothing that fills
  > it** — `Buffers` is a `BTreeMap<BufferId, Editing>`, nothing in a map requires a pane to point
  > at an entry, and no verb `T088` adds creates one that has no pane. **This task inherits the
  > container and owes the rules**, because they are judgements about product behaviour and this
  > is the task with a caller for them: what attaches and detaches an entry, what `:wall` counts
  > once it is a question about `Buffers`, what `:q` counts, and what an unattached buffer's wrap
  > width is — `soft_wrap::wrap_to` takes a `Rect` and there is no pane to supply one. §47 carries
  > the evidence and the two options it was chosen over.

- [x] **T061 · Permission asks + rule writing**
  Screen `7a`: exact invocation shown; always-allow **writes a legible rule**.
  *Done when:* the written rule is readable by a human and takes effect next time. *Needs:*
  T059

  > **Built 2026-08-21.** `runtime/permissions.scm` defines `allow` and draws
  > `7a`; three arms carry the ask and the two grants; the rule takes effect in
  > the same session and on the next one. **`OPEN-QUESTIONS.md` §35 is closed**
  > and the check it asked for was made.
  >
  > **The rule is an option, and the option is a published copy.** The natural
  > shape is to read the allow-list back and append to it, and this build has no
  > reader: `(options)` is `T021` and unarmed, so `(hash-try-get (options) …)`
  > answers `#raised · not built yet`. So the list lives in Steel and
  > `set-option!` writes it out — one truth with a copy for whoever reads it,
  > which is the shape `session` and `transcript` already have.
  >
  > **A rule is a verb, not a command line.** `(allow "git push")` has to cover
  > `git push origin retry-backoff` or it is a rule that never applies twice —
  > and it must not cover `gitleaks`, which is the difference between a prefix
  > *rule* and a prefix *match*, and the way an allow-list quietly grants more
  > than it says. The boundary is a space or the end of the string.
  >
  > **The rule is in the option's label, which is one better than the mockup.**
  > `7a` puts `2 writes (allow "git push")` in the footer; here it is on the
  > thing you are pressing. A legible rule is one you read *before* you agree to
  > it, not one you go looking for afterwards.
  >
  > **`7a` is `4a`'s body under different chrome**, and that is the smallest true
  > reading of two drawings that differ by a sentence at the top:
  > *"needs input"* against *"wants to run"*. A second `view/question` would be
  > the same node twice. Which surface a question raises is a fact the queue
  > already holds — a permission ask is the one the editor is holding a verb for.
  >
  > **The three digits are three verbs, not three answers.** `[1]` and `[2]`
  > both let it run and differ in what they *write*; `[3]` refuses.
  > `grant-permission` and `deny-permission` exist because that distinction is a
  > vocabulary fact, and routing a permission digit through `answer-ask` would
  > lose it — `[2]` would be an answer of `2` and the rule would never be
  > written.
  >
  > **A rule that already permits it is not a question**, checked on the path
  > that would otherwise ask. That is *"takes effect next time"*, and it means
  > a grant from a previous session is honoured by the same code as one from
  > thirty seconds ago.
  >
  > **`persist!` is an identity function.** `runtime/repl.scm` defines it as
  > `(define (persist! kept) kept)` — a *marker*, and what writes is the REPL
  > noticing that head and routing the form. So evaluating `(persist! …)` from
  > the loop returns the string and writes nothing; this calls `AppHost::persist`
  > directly. The first version did the former and the test caught it by reading
  > an empty file. §35 now records that too.
  >
  > **`:allowed` is the audit**, and it exposed a failure mode worth writing
  > down: `open-float` takes `surface` **and** `args`, and leaving `args` out
  > raises inside `phosphor/ex` — which the bridge reads as `Ex::Unknown`. So a
  > command that *is* registered answers *"no such command"*.
  > `(phosphor/ex-bound? "allowed")` said `#t` while the ex line said otherwise,
  > which is how it was found.
  >
  > **A comment in `init.scm` silently truncated the load order.**
  > `the_load_order_and_the_directory_agree` read the list by finding the first
  > opening bracket and the first closing one, so the `T061` comment added
  > beside `permissions.scm` — which quoted a form, brackets and all — cut the
  > list at that point and hid the last six languages. **The failure named them
  > as missing from a list they were in**, which is the worst kind: a true
  > alarm with a false explanation. Scheme's own reader ignores comments, so the
  > editor had loaded the whole list the entire time and only the test
  > disagreed; it strips comments now, and the two readers agree about what the
  > data is. The sentence first written to warn about the trap sprang it a
  > second time.
  >
  > **`defer-ask`'s `ask` became optional**, and the vocabulary already had the
  > idiom — *"absent means the focused one"*, which is what `set-cursor`'s
  > `buffer` says. A door has to be able to name an ask; a person has one
  > question in front of them and no id on screen to read off. Found by
  > `every_ex_command_decodes`, which types every command with an empty
  > argument: `(string->number "")` is `#false`, which raises inside `key/cmd`
  > and reaches the ex bridge as `Ex::Unknown` — so `:defer` answered *no such
  > command* about a command that exists, the same failure `:allowed` had for a
  > different reason.
  >
  > **Verification.** One keystroke test end to end: `7a` shows the exact
  > invocation and its three answers, `[2]` says what it did, the next
  > invocation of the same verb asks nothing, `:allowed` reads the rule back,
  > and `persisted.scm` holds `(allow "git push")` as a form a person can read.
  > One unit test over the prefix rule, including the `gitleaks` boundary and
  > the empty list. Three planted defects, three caught: a prefix match instead
  > of a prefix rule, a written rule that stops taking effect, and a rule that
  > never reaches disk.
  >
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

- [x] **T062 · Interrupt and steer**
  `esc` pauses at the next tool boundary → steer / resume / abort. The seam is recorded in the
  transcript.
  *Done when:* screen `7e` reproduces **from a keystroke** — `esc` mid-turn in the running
  binary reaches the next tool boundary. *Needs:* T057

  > **Built 2026-08-21.** Four verbs over one pair of fields, a third seam tone,
  > and `session/cancel` on the wire.
  >
  > **`Shell::pausing` is the request and `Shell::paused` is what it becomes.**
  > That pair is the whole design: an interrupt that took effect *now* would
  > land in the middle of whatever the agent was doing, which is the thing a
  > tool boundary exists to avoid. The boundary is `tool-call-started` — the
  > agent has said what it is about to do and has not done it — and the seam is
  > written there rather than by the verb, for `7b`'s reason exactly: the pause
  > is a fact about a moment the verb cannot see.
  >
  > **The held call is drawn and not run.** `▸ next: edit tests/ws_test.rs` is
  > `7e`'s own row, and it is what makes the boundary a thing on screen: a pause
  > you cannot see the edge of is indistinguishable from a hang.
  >
  > **`Seam::trouble` was a `bool` and is now three tones.** `1b` ends a turn in
  > claude-green, `7b` stops one in trouble-red, and `7e` stops one in §1's
  > attention-amber — because a pause is a thing *you* did, and amber is the
  > palette's word for waiting on you. The flag was honest about two cases and
  > became a lie at three.
  >
  > **The cancel goes over the wire, and the first version did not.** A pause
  > that stopped *drawing* the agent's work while the agent went on doing it is a
  > strip saying `⏸ claude paused` about something that is not — §5's *"always
  > truthful"* failing in the moment it matters most. Measured at the terminal:
  > the toy agent finished its turn and `✻ EndTurn` overwrote the seam that had
  > just been written. `Ask::Interrupt` sends `session/cancel` and empties the
  > prompt queue with it, because a prompt still waiting behind an interrupted
  > turn is one you asked for before you changed your mind.
  >
  > **A pause outranks the stop reason.** ACP's own note is that an agent may
  > send final updates after a cancel, and an agent that has not honoured one
  > sends the whole turn — so `turn-ended` leaves a paused seam alone. Otherwise
  > the screen forgets the pause it is still in.
  >
  > **A held call's progress and completion are dropped, not refused.** The same
  > run put `acp: no such tool call` on the notice row of an otherwise correct
  > screen — the editor complaining about its own decision, since the call was
  > held on purpose and never entered the transcript.
  >
  > **`esc` is the only one of the four that is a key**, and it is scoped: §9's
  > `esc` closes top-down, so a float, a picker or the ex line takes it first,
  > and what is left is `esc` in a buffer while a turn is running — `7e`'s own
  > gesture. What to do next is `:resume`, `:steer` or `:abort`, which are
  > decisions rather than reflexes.
  >
  > **`:steer` differs from `:resume` in one thing**: it sends the correction as
  > a *prompt*, which is what makes it steering rather than a note. The arm
  > holds the body because it has no session handle; the loop sends it and
  > resumes in one place, and the order is what makes the two one gesture.
  >
  > **The `dawdle` fixture mode exists for this and only this.** `esc` has to
  > arrive while a turn is running and *before* the agent's next tool call;
  > every other mode reaches the boundary in microseconds, so a test would pass
  > or fail on scheduling.
  >
  > **`7e`'s `PAUSED` mode chip is not built, deliberately.** The mockup draws it
  > where `NORMAL` goes, and that cell is the *editor's* mode: the buffer is
  > still in normal mode while a turn is paused, and a chip that said otherwise
  > would be the one inverted thing on screen telling you about something else.
  > The session state is on the strip, which is where §5 puts it —
  > `⏸ claude paused`. Recorded at `OPEN-QUESTIONS.md` §57 rather than folded in.
  >
  > **Verification.** Two keystroke tests. `7e` from a keystroke: `esc` says what
  > it asked for, the boundary arrives with nothing pressed, the strip says
  > `claude paused`, and the transcript carries `acp · paused`, the held
  > `next: edit` row, the seam, and the three ways on — and the seam is *not*
  > overwritten by the turn ending. Then `:abort`, whose held call does not run.
  > The second test proves the other two: `:resume` moves the held call into the
  > transcript and takes the seam with it, and `:steer` does that *and* is heard
  > — the toy agent echoes the correction back. Three planted defects, three
  > caught: a boundary that never fires, a stop reason that overwrites the seam,
  > and a steer that resumes silently.

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

- [x] **T063 · DiffBody** — **built on `similar`, not on a bought widget.** The T008 spike found
  `mod diff` private and the diff implemented as a *mode of the Editor*, so there is nothing to
  restyle. Unified and side-by-side; fold rows for unchanged spans. `similar` already arrives
  transitively via the vendored crate, so this adds no dependency.
  *Done when:* renders a real diff correctly. *Needs:* T041, T084

  > **Built and ticked 2026-08-23.** `crates/phosphor-ui/src/diff.rs` — `Change`/`Line`/`Hunk`/
  > `File`/`DiffVm` as the ViewModel and `DiffBody` as the widget, drawn from the interpreter's
  > `Node::Diff` arm through a new `Resources::diff(&DiffSource)` door.
  >
  > **`similar` is a dev-dependency and not a dependency**, which is the entry's own claim read
  > exactly: *"adds no dependency"* is true because **the widget does not compute a diff**. It
  > draws one. The rows arrive already classified through `Resources`, the same division
  > `Node::Transcript` has — a host that has a rope and a disk copy computes; a widget that has a
  > `Buffer` paints. What needed `similar` was the *test*, because `T063`'s acceptance is
  > *"renders a real diff correctly"* and a hand-written `Vec<Line>` proves the renderer against
  > the test author's idea of a diff rather than against one. So the tests run `similar::TextDiff`
  > over `4b`'s own before-and-after and render what comes back. It adds a line to a manifest and
  > no crate to the shipped graph — `just deny` and the dependency tree agree.
  >
  > **Three planted defects, three catches, each by the test whose claim it breaks.** Making
  > side-by-side context one-sided failed only the pairing test; swapping `−` (U+2212) for an
  > ASCII hyphen failed only the three-kinds test; making the fold cosmetic — drawing a folded
  > hunk's lines anyway — failed only the fold test.
  >
  > **Side by side is not half as tall, and one test exists because of it.** A run of three
  > removals against one addition does not pair three times, so the measured height and the drawn
  > rows can disagree and a float would size itself wrong. `the_measured_height_is_the_height_it_draws`
  > asks both in both modes.
  >
  > **`paired()` needed a nested `fn`, not a closure.** The flush that empties the removal and
  > addition runs into the row list while the two `Vec`s are still borrowed; a closure captures
  > them and a `fn flush<'l>(…)` taking all three does not.
  >
  > **`Node::Diff` is no longer deferred, and the two lints that watch that both moved.**
  > `interpret.rs`'s module table is down to a single row (`watch`) for the first time, and the
  > fixture in `an_unbuilt_primitive_is_reported_not_silently_blank` has now been outlived by the
  > build four times over — `Transcript`, `Picker`, `Question`, `Diff` — and this time there was
  > **nothing unbuilt left to pair it with**. It used two deferred kinds to prove the report is a
  > list rather than a flag; it now proves that a deferred kind is named *once* however many are
  > on screen and that a drawing sibling stays out of the list. Both are `Report::defer`'s
  > `contains` guard and neither was tested before.
  >
  > **All three capabilities declared against this task have moved, and it now declares none.**
  > `lint-action-arms` said so about two of them the moment it was ticked; the third has no lint
  > and was found by asking the same question. A refusal reads *"not built yet — {task} builds
  > it"* (`crates/phosphor-core/src/action.rs:1511`), so a capability citing a **ticked** task
  > tells a user a falsehood. `set-diff-mode` and `expand-diff-context` act on a diff that is *on
  > screen* and went to `T066`, beside the `open-review-block` already there — so the task that
  > opens a review block is the task where switching its mode and expanding its context become
  > reachable. `hunks` answers *"a block's hunks, with each one's seen state"*
  > (`crates/phosphor-core/src/query.rs:451`), which is `T064`'s sentence word for word, and went
  > there. This is `jump` → `T042` and `apply-edits` → `T052` exactly, and that table's own note on
  > those two says why it beats a RECORDED row: **the attribution was the bug, not the absent
  > arm.** `crates/phosphor-core/tests/surfaces.txt` was regenerated and its diff is the record.
  >
  > **So `T063` is a task with no capability, and that is a decision on the record rather than an
  > omission**: `vocabulary.rs`'s `NO_CAPABILITY` gains an entry, and the answer it gets is the one
  > `T031` got — *"a widget. It renders … state the store already holds; nothing about it is a
  > mutation"*. This widget draws rows a host hands it; it computes no diff, holds no hunk and
  > mutates nothing. `every_mutating_task_in_s3_to_s8_has_a_capability` is what forced the entry to
  > be written rather than the absence to be silent.
  >
  > **One comment moved with them.** `crates/phosphor-core/src/input/text.rs:765` told `vih` that
  > hunks were `T063`'s, which would have read as *built* the moment this line was ticked. A hunk
  > `vih` can select is one with an id and a seen bit, and that is `T064`'s store.
  >
  > **`lint-node-kinds.sh`'s `Diff` row is re-pointed at `T066`, not deleted.** Nothing composes
  > a `Node::Diff` in the shipped configuration, because nothing yet resolves a `DiffSource`.
  > Its four arms carry four creditors (`crates/phosphor-core/src/view/props.rs:601`):
  > `ReviewBlock` is `T053`'s and `T053` **is ticked** — but it shipped `declare_block`, grouped
  > unseen markers and a notification, and no surface that draws the block, so recording against
  > it would be the wrong creditor a third time in that table. `T066` is the first unticked task
  > whose acceptance names a screen that draws one (`4b`, `2b`). `T064` and `T065` sit between,
  > and their acceptance is about seen-state and about grouping rather than about a composed
  > surface — if either composes one first, the lint reddens and says to delete the row.
- [x] **T064 · Per-hunk seen state** — `s`/`S` compose over any group.
  *Done when:* marking one hunk seen leaves the rest unseen. *Needs:* T063, T041

  > **Built and ticked 2026-08-23.**
  >
  > **A hunk is a region, and `store::Hunk` is the type that says so once.**
  > `declare-review-block` already makes one region per changed span, so inside a review block
  > *one span is one region is one hunk* — three names for the thing `4b` draws a sign beside and
  > `s` marks seen. The alternative was a hunk table, and it would have been a second place for
  > seen-state to live: §7 has one mutable flag and it is on the region, so a hunk row carrying
  > its own would be two records of one bit, disagreeing the first time a rewrite moved a span.
  > `store::Block`'s own doc had already made this ruling one noun in — *"a block holds ids, not
  > spans — the region **is** the span"* — and this is the same ruling one noun further out.
  > `Hunk::region_of` and `Hunk::id_of` are the only places the two spellings meet.
  >
  > **`gsih` was a binding that did nothing, and now it is the acceptance.** `h` has been bound to
  > `(key/object "hunk")` since the objects landed; `TextObject::Hunk` answered `None`, so the
  > whole phrase parsed and was a no-op. `Text::hunk_at` is the new seam, defaulting to `None` the
  > way `unseen_at` does, and `EditorText` answers it off the same store the gutter draws from.
  > `marking_one_hunk_seen_leaves_the_rest_of_the_block_unseen` presses it through a real pty: three
  > spans declared, cursor into the second, `gsih`, and `(unseen-count)` goes 3 → 2 with
  > `(#f #t #f)` naming which.
  >
  > **`vih` and `viu` are two nouns, not one with a filter**, and the asymmetry runs both ways. A
  > hunk is a region a *review block* declared, so an ordinary `declare-regions` marker is not one
  > — otherwise the two nouns would be the same noun. And a hunk you have already marked **is
  > still a hunk**, where `viu` excludes what you have read: `s` has to be able to reach a hunk you
  > marked in order to unmark it. `only_a_block_declared_region_is_a_hunk` and
  > `a_seen_hunk_is_still_a_hunk_and_a_seen_region_is_not_unseen` hold both halves.
  >
  > **`Scope::These` is the new scope, and an empty one is not `Everywhere`.** `8b`'s
  > `S here marks all 12` is a target that resolves to twelve ids, so the store needed a set-of-ids
  > scope beside `One`. The hazard of carrying a collection is the empty case inverting into *"all
  > of them"*, so it is stated on the variant and tested. Beside it: **an id that names nothing is
  > absent, not empty.** `block_regions(99)` is `None` and refuses; a block whose regions were all
  > dropped is `Some(vec![])` and marks nothing, saying `no region here`. Collapsing the two would
  > make a typo look like a no-op.
  >
  > **The three review targets do not split across the door/loop line, and `review_scope` is why.**
  > That split exists for *focus* — `selection` means something different depending on where the
  > cursor is, and a door has no cursor — but `hunk`, `group` and `block` name regions by id and
  > need no editor. So both halves resolve them and resolve them identically, through one function.
  > `RESOLVABLE` goes from three tags to six.
  >
  > **Groups got ids, and they are minted across the session rather than within a block.** `Group`
  > had none at all, so `Target::Group { id }` and `annotate-group` both took an id nothing could
  > produce — the same gap `T088`'s pane verbs had. The counter sits beside `blocks` rather than
  > being derived from `blocks.len()`, because deriving it would mint a colliding id the first time
  > anything removed a block; nothing removes one today and this is what keeps that from being
  > load-bearing. `review-blocks` rows carry `group` now.
  >
  > **A review block is not a span, and `gsib` cannot be an operator.** The keymap's own comment
  > calls `gsib` *"the sentence 6d is about"*, so this was checked rather than assumed: a block is
  > twelve regions across three files, and the widest thing an operator can be handed is one span
  > in one buffer — so `gsib` could only mark everything between the first region and the last.
  > `8b`'s `S here marks all 12` is a key on the review **surface**, which is `T066`'s, and
  > `TextObject::Block` stays `None` naming it. That is a finding, not a gap, and the keymap says
  > so where a reader will hit it.
  >
  > **Five planted defects, five catches, each by the test whose claim it breaks.** Letting any
  > region count as a hunk failed only `only_a_block_declared_region_is_a_hunk`; reporting every
  > hunk unseen failed the acceptance test and the block-wide one; minting one group id for every
  > group failed only the minting test; answering an empty scope for an unknown block failed only
  > `an_unknown_block_is_absent_rather_than_empty`; and putting `hunk_at` back to `None` failed the
  > pty test and nothing else, which is the point of having it.
  >
  > **One duplication was caught by clippy rather than by review.** `Hunk::region(self)` and the
  > `RegionId(id.0)` written out inside `review_scope` were two spellings of the conversion the
  > type exists to keep in one place; `region` had no caller outside the tests, which is what that
  > looks like from the outside. One `region_of(HunkId)` now, called by both.
  >
  > **`revert-hunk` is declared against this task and is not seen-state.** It lowers to edits, so
  > it needs the text that was there *before* the hunk — and a review block records where claude
  > wrote, not what it replaced. Recorded against `T066` rather than re-declared, because which
  > task owns the verb is downstream of a question nobody has answered: of `DiffSource`'s four
  > arms only `Disk` (`T070`) and `Change` (`T073`) have a before-side at all, `ReviewBlock`
  > has none, and `Hunk` inherits whichever the peek was opened from. That is a ruling about what
  > `4b` **draws**, so it is written up as OPEN-QUESTIONS.md §59 with three candidate answers and
  > left to the task with the screen in front of it.
- [x] **T065 · Directory grouping + annotations** — `tui-tree-widget`; Claude's group
  annotations ("mechanical" vs "the meat"). **Scale is grouping, not scrolling.**
  *Done when:* screen `8b`'s 40-file block is navigable. *Needs:* T064

  > **Built and ticked 2026-08-23.** `8b` opens with `:review`, walks with `j`/`k`, folds with
  > `za`, takes claude's note with `:annotate` and reflows with `:grouping flat`. One pty test
  > presses all of it.
  >
  > **`tui-tree-widget` was fetched, read and not used**, which is the entry's own instruction
  > overruled and so is stated rather than buried. It re-exports `ratatui_widgets::scrollbar`
  > (`src/lib.rs:13`), so taking it puts a widget crate into the one crate that is
  > deliberately `ratatui-core`-only; it is a `StatefulWidget` over a `TreeState` that owns the
  > selection and the open set, which every other body here keeps in a ViewModel; and a
  > `TreeItem` carries one `Text` blob per node, so `8b`'s `▾ src/api/` on the left with
  > `●31 unseen · 14 files · the meat: …` after it is our layout either way. What it would have
  > contributed is fold arrows and a flatten-with-indent — about thirty lines. **The Component
  > Breakdown says buy and the build says no**, the same shape `T063`'s entry records for the
  > bought diff view, and it is flagged here rather than folded into the design docs.
  >
  > **`8b`'s grouping is claude's, not the filesystem's**, and that is the finding that shaped the
  > ViewModel. The mockup draws `src/errors.rs` as a *peer* of `src/api/` and `src/db/` even though
  > its parent is `src/` — so the tree cannot be derived from the paths, and a widget that grouped
  > by parent would draw a different screen. `Entry` is therefore **one list and not two**: `8b`
  > interleaves group rows and bare files, and two fields would lose the order, which is claude's
  > statement about what to read first.
  >
  > **The host groups and the widget draws.** Nothing in the vocabulary carries claude's judgement
  > yet, so `review_vm` groups by parent directory — the honest grouping available today — and when
  > claude's arrives it replaces that function and not the widget. `Grouping::Flat` is a *rendering*
  > choice in `DiffBody` and a *building* choice in the host, and they are not the same edit: the
  > float is a snapshot composed once with `"directory"` on its node, so a `:grouping flat` typed
  > afterwards has to change what the rows *are*. The first version set the session and nothing
  > moved.
  >
  > **`8b` draws no diff lines, and that is not a shortfall.** It is the navigation — 41 files as
  > eight rows — and every field it draws is a fact the store has: paths, counts, annotations, hunk
  > ranges. The `+`/`−` lines are `4b` and need the text a hunk replaced, which nothing records
  > (§59). **`8b` and `4b` are one surface at two fold depths** — both open
  > `review — ✻ <title> · N files · N regions · N seen ✓` — which is why there is one float, one
  > session and one verb, and why `T066` inherits the surface rather than building it.
  >
  > **`open-review-block` is `T066`'s verb, armed here**, because *"`8b` is navigable"* is not a
  > claim you can make about a screen nothing opens. It is not re-declared: an unticked task's verb
  > arriving early is a task getting a head start, not a misattribution, and `T066`'s entry can say
  > so. Its `block` became `Option` — **absent means the newest** — which is `defer-ask`'s ruling,
  > and `annotate-group`'s `group` became one too.
  >
  > **Three things only a run found, and each is a rule now:**
  >
  > * **An ex lambda that calls a query is a command that says it does not exist.** `:review` bare
  >   resolved the newest block by calling `(review-blocks)` inside the lambda, and a raise inside
  >   `phosphor/ex` reads as `Ex::Unknown`. `every_ex_command_decodes` caught it, as it caught
  >   `(string->number "")` at `T060`. The resolution moved into the arm. `:annotate` had the same
  >   shape and lost its id argument entirely — it annotates the row you are on, which also removes
  >   an ambiguity rather than deferring one, since `:annotate 3 handler signatures` would have to
  >   guess whether `3` is a group.
  > * **A float that owns every key owns the two commands that only work inside it.** The review's
  >   keys were guarded on *"a review is open"* and swallowed `:`. `4a`'s digits guard on the keys
  >   themselves and always did; `review_key` does now, and [`None`] means *"not ours"*.
  > * **The ex line returned to the buffer and closed the review.** The float was never dropped —
  >   only `surface` moved. Narrowed to the review on purpose: whether a picker or a question float
  >   should survive an ex line is a separate question with its own screens.
  >
  > **`lint-node-kinds.sh`'s `Diff` row is deleted, one task after being re-pointed.** `T063`
  > recorded it against `T063`, ticking that re-pointed it at `T066`, and `runtime/review.scm`
  > composes `Node::Diff` now — so the lint reddened with *"the shipped configuration composes it
  > now"* and the row went. That is the table working: it can only shrink, and it shrank because
  > somebody built the composition rather than finding a better task to blame.
  >
  > **Eleven planted defects, eleven catches.** Six against the widget — a folded group drawing its
  > children, a fully-seen group counting zero, a group row reporting what it drew, an indented fold
  > arrow, flat grouping dropping files, an empty annotation stored instead of clearing. Five
  > against the host — an off-by-one fold key, a directory counting one file's unseen, a nested file
  > repeating its directory, flat grouping still building group rows, and annotate writing to a
  > fixed group.
  >
  > **A twelfth defect passed, and the fixture was the bug.** `open.files = 1` survived a one-file
  > fixture, because with one file per directory the right answer and the wrong one are the same
  > number. The fixture is two files in one directory now — which is also the first fixture in which
  > the grouping does anything — and it catches it.
- [x] **T066 · Review block + hunk peek** — screens `4b`, `2b`. *Needs:* T065, T053

  > **Built and ticked 2026-08-23.** `4b` opens over `8b` — the same float, deeper: `s`/`S`
  > mark a hunk, its file or its directory; `]]` jumps to the next file; `za` folds a hunk
  > (added to what `T065` already folded — a directory) or unfolds it. `2b` opens with `gh` at
  > the cursor, without leaving the buffer, and closes with `q` or `esc` like every float now
  > does — `Surface::Float` learned `q` here, the way `Surface::Help` already had it.
  >
  > **§59 ruled first: the before-side is claude's to state.** Neither screen can draw a `−`
  > line without knowing what a hunk replaced, and nothing in the graph produced one for a
  > review block. Ruled (2): a VCS's copy answers a different question and `T071` may not
  > assume a repo exists; the file itself has lost the prior text; only claude's own declaration
  > knows it. `FileGroup::spans` changed from `Vec<Span>` to `Vec<ChangedSpan>` — a wire change
  > to a ticked task's own verb, taken rather than worked around, with a record per span rather
  > than a parallel array beside it. `store::Change` is the one place the before-text lives, and
  > `store::Hunk::was` is `None` for a pure insertion — which `4b` draws as `+` lines and no `−`
  > at all, the truthful reading and not a fallback.
  >
  > **The after-side is read live, buffer first.** `hunk_lines`/`span_text` slice whichever
  > text is current — the open rope if the file is a buffer, disk otherwise — so `s` on a hunk
  > moves the counts without recomposing, and an unsaved edit shows on the screen rather than
  > what was last written. One scan of the open buffers (`open_texts`) is shared by `4b`/`8b`
  > and `2b` rather than run twice.
  >
  > **A real store bug, found by a test that was already there.** `hunk_of`'s first version
  > scanned every declared block for a region id, so a span two blocks both declared came back
  > attributed to whichever block declared it *first* — for *both* blocks' `hunks()` calls.
  > `group_ids_are_minted_across_blocks_not_within_one` (`T064`'s own test) caught it:
  > `GroupId(0) != GroupId(0)` failed with both sides equal. Fixed by keeping `hunks(block)`
  > scoped to that block's own groups — it already has the answer in hand — and reserving
  > `hunk_of`'s global scan for the one caller that has an id and no block, `2b`'s peek, where
  > the same ambiguity is a real but narrower case now pinned by its own test and documented as
  > a stated convention rather than an accident.
  >
  > **`S` widens by one level, whatever the row is — hunk → file, file → directory, directory →
  > block.** `4b` draws `s seen · S all` beside a hunk; `8b` draws `S here marks all 12` beside a
  > directory. A single fixed meaning would make `S` on a hunk do the same thing as `S` on a
  > directory, which is a key that stops telling you where you are.
  >
  > **A two-key prefix machine, because `za` and `]]` are each two keys and the real input
  > machine does not run over a float.** `Review::pending` holds the first key; a bare `a` no
  > longer folds and a lone `]` no longer jumps — both were live bugs the first version of the
  > `4b` key test caught by pressing the actual sequences rather than one key at a time.
  >
  > **Both notes draw together, and the module doc was wrong about that.** `4b` draws
  > `@@ 9–14 · tests   ⋯ folded · 6 lines   seen ✓` on one row — a folded hunk that has also
  > been read says both, because they answer different questions: *how much is hidden* and
  > *have you read it*. The comment claiming they were *"one of two and never both"* sounded
  > right and was checked against the mockup rather than trusted.
  >
  > **`2b`'s `s` reuses `mark-seen` with an explicit `Target::Hunk`, built at `T041` and resolved
  > through the same `review_scope` both doors already share** — no new capability for the peek's
  > one verb. `open-hunk-peek`'s own target resolution covers `Target::Hunk`/`Target::Region`
  > (an agent's spelling) and `Target::Cursor` (the keyboard's), the latter needing the editor
  > and so living in `Editing::act`, the same seam `open-review-block` already crossed at `T065`.
  >
  > **One surface, one session — a probe found the gap before a test did.** `gh` pressed while
  > a review float held the screen fired anyway, because `review_key` doesn't claim `g` and
  > unclaimed keys fall through to the buffer's ordinary keymap even while a float is up. Left
  > unhandled, `shell.review` would survive a peek opening over it, and the next `s` would read
  > whichever session's guard happened to match first — silently wrong rather than refused.
  > Both arms now clear the other session on open;
  > `opening_a_peek_while_a_review_is_open_replaces_it_not_layers_it` pins it.
  >
  > **`SetDiffMode` and `ExpandDiffContext` also went unbuilt, and both by the same kind of
  > check.** Neither has a key on any screen this task or the ones after it draw. `4b`'s own
  > design-brief line says the block diff is *"one unified diff"*, full stop — the side-by-side
  > mode `DiffBody` already draws (`T063`) belongs to `:dv`, `T070`'s screen, whose own line is
  > *"a side-by-side of buffer vs disk"* — so `SetDiffMode` is recorded against `T070`.
  > `ExpandDiffContext` has no home at all yet; recorded with no creditor, the same shape
  > `Gutter`/`Spinner`/`Elapsed` take in the other lint's table.
  >
  > **`revert-hunk` stays unbuilt, and this time by a finding rather than a deferral.** The
  > before-side this task built made the *rich* revert buildable, which is what made it worth
  > checking against the mockups instead of assuming: the only "revert" key any screen draws is
  > `6d`'s `dih  delete inner hunk — revert claude's edit, plain vim delete` — `T026`'s delete
  > operator over `T064`'s hunk text object, already reachable before this task started. `4b`'s
  > footer has no revert key; `2b`'s `u undo (jj)` is `T073`'s different verb over a different
  > store. `dih` deletes; `revert-hunk` would restore what claude's edit replaced; nothing asks
  > for the difference. Recorded with no creditor rather than re-pointed at a task that would
  > never close it, the same shape `lint-node-kinds.sh` uses for `Gutter`/`Spinner`/`Elapsed`.
  > `a_declared_hunks_dih_reverts_it_plain_vim_delete_style` proves `dih` against a declared
  > hunk that *has* a before-side, and proves it is not written back — the exact distinction the
  > record turns on.
  >
  > **Nine planted defects across the two screens, nine catches** — six against `T065`'s carried
  > forward (folded-group leak, wrong unseen count, wrong file count, indented arrow, flat
  > grouping keeping rows, an annotation that didn't clear), plus three found and closed this
  > task: `hunk_of`'s cross-block attribution (a store test), `peek_vm` forgetting to slice its
  > after-text to the hunk's span (draws the whole file), and the one-surface-one-session gap
  > (the probe-then-test pattern above).
  >
  > `just gate` green.
- [x] **T067 · Inbox** — one list of everything Claude said; severity is a single MCP flag;
  unread = unseen. Screen `5c`. *Needs:* T053, T041

  > **Built and ticked 2026-08-24.** `:inbox` opens `5c`; `↵`, `s` and `esc` work; `:notify`
  > posts a note from the keyboard and the door's `notify` capability posts one from an agent.
  > The strip draws `inbox N unread` on §11's counter rung, dropping the word before the glyph
  > the way `unseen` does — except there is no glyph, because §2's lexicon has none for an inbox
  > and this file does not invent one.
  >
  > **The inbox is a merge, and CP-8a's requirement is what that word has to mean in code, not
  > prose.** `5c` is *"everything claude said"*, and the three things he says already live
  > somewhere: a pending ask (`T060`'s queue), a declared review block (`T053`), a note
  > (`store::Note`, the one addition here). `AppHost::inbox_rows` reads all three and computes
  > `unread` per kind — pending for an ask, any-region-unseen for a block, its own bit for a
  > note — and stores nothing. The statusline's `inbox_unread` applies the identical three rules
  > independently, so the strip and the float cannot disagree by construction; a planted defect
  > that counted every note instead of unread ones caught nothing until the test marked one note
  > read and checked the strip afterward, which is when *"the same number either way"* stopped
  > being true.
  >
  > **A row's identity had to survive the query that produced it, and an index would not.** A
  > note arriving between two `(inbox)` calls renumbers everything after it in a merged list — so
  > `InboxId` encodes its own source (`InboxSource::{Ask,Block,Note}`) rather than being one. Two
  > functions, `id()`/`of()`, are the whole codec, the shape `Hunk::region_of`/`id_of` already
  > used for the other id coupling in this file. A test pins the round-trip and the
  > no-collision property directly, because an off-by-one in the encoding is the kind of bug a
  > screenshot does not catch.
  >
  > **A note is the one inbox row that is not a region, and both `mark-seen` appliers take the
  > same shortcut for it.** `Editing::mark` and `AppHost::mark` each check for
  > `Target::InboxItem` naming a note *before* the region-scope machinery runs, because a note
  > has no file and no span to resolve one from. The two had to be written twice and had to
  > agree — the pty test marks a note through the door's capability (a repl call) and reads the
  > result off the keyboard-driven float, which is the only way to prove they mean the same
  > thing rather than assert it.
  >
  > **`↵` means three different things because a row does**, and a note with no anchor refuses
  > by name rather than opening nothing silently — *"that note is not about anywhere"* is the
  > honest answer to a sentence that is not a place.
  >
  > **The times are relative, and that is a stated deviation from `5c`, not an oversight.** The
  > mockup draws `2m` for the newest row and `14:41` for the older three; nothing in this
  > dependency graph can render the second half — there is no timezone-aware wall clock — and
  > adding one to format a timestamp is not a trade this task makes. `store::Note::at` is an
  > `Instant`, every row renders relative, and the ordering and the recency claim both still
  > hold. Flagged per the standing rule rather than folded into the design docs.
  >
  > **`5c` is navigable, and that needed a second pass the first tick did not have.** `:inbox`
  > opened a static list; `j`/`k` did nothing. `view/spans` is a *snapshot* — `layer.surface`'s
  > own words — and unlike `4b`/`8b`/`2b` there is no `Resources` door into it, so navigation
  > recomposes: `Shell::inbox` holds the row index, `j`/`k` re-run the surface with a new
  > `selected`, and `Tint::Selection` (which the vocabulary already had, for exactly this) draws
  > the highlight. `s` marks the highlighted row and `↵` opens what it names. `inbox_row_id`
  > rebuilds the same order `inbox_rows` draws, so a keystroke cannot act on a different row than
  > the one under the highlight.
  >
  > **Ten planted defects, ten catches — and three of them only after the fixture was fixed.**
  > The first six: a block computing its own unread flag instead of deriving it, marking one row
  > read marking every row, the merge sorting oldest-first, the strip counting every note instead
  > of unread ones, two id kinds colliding at the same ordinal, and a note posted already read.
  > Four more against the interactive half: `j` not moving, `s` acting on row 0 regardless of the
  > highlight, blocks sorted on `BlockId` instead of the shared arrival clock, and the key guard
  > swallowing `esc`.
  >
  > **Two of those exposed a real bug and a real gap in my own testing.**
  >
  > * **The arrival clock exists because a planted defect found its absence.** `BlockId` and a
  >   note's counter mint independently, so ordering the merge on either is wrong the moment they
  >   interleave. `Shared::arrivals` is one clock both stamp from. Proving it took **three**
  >   fixtures: with one block, `BlockId(0)` and arrival `0` are the same number; with one note
  >   between two blocks, the wrong key *ties* and a stable sort breaks the tie correctly by luck;
  >   only with two notes do the keys diverge enough to change the drawn order. The test's own doc
  >   records all three, because the first two versions passed against a defect and that is worth
  >   more written down than fixed silently.
  > * **The `esc` defect is caught as a *hang*, not a failure.** With the guard broad again, the
  >   inbox never closes, and `leave_by` does a `child.wait()` with no timeout. The guard is
  >   narrowed to the keys it uses — `review_key`'s rule, a third time — and the arm's own comment
  >   names the failure so the next person widening it knows what they are buying.
  >
  > `just gate` green — confirmed with a bare, unpiped run after a mid-session reminder that a
  > verification command piped into `grep`/`tail` reports the filter's exit code and not the
  > command's.
- [x] **T068 · Anchored exchange / threads** — your comment and Claude's reply as virtual text
  under the region. Screen `3a`. The region itself carries Design Language §3's full anchored
  treatment — **tint + undercurl** — which is `T087` and `T085` composed, not the marks API alone.
  *Needs:* T032, T042, T085, T087

  > **Built and ticked 2026-08-24.** `:comment` opens a thread at the cursor; claude replies
  > through the door; both draw as `┊` rows under the anchored line; the strip counts the
  > conversation you are still in. `vit` selects a thread, `:g/TODO/c` broadcasts one message
  > against every match.
  >
  > **Four ticked tasks composed and almost nothing new drawn.** The entry's own line — *"`T087`
  > and `T085` composed, not the marks API alone"* — turned out to describe the whole build:
  > `gutter::RegionState::Thread` was already in the ladder mapping to `StateMark::None` (§3's
  > row 20: an overlay tints a row and says nothing in the column), `Tints::tint` already
  > answered the anchor hue for it, `theme.regions.anchor_undercurl` already existed with §3's
  > name on it, and `T032`'s `VirtualText` already had an owner field. What `T068` adds is a
  > store, five arms, and about forty lines of composition in `decorate` — the drawing layer
  > had been waiting.
  >
  > **A thread is a span, not a region and not an anchor.** `T042`'s anchors are the machinery
  > for surviving a rewrite and `T041`'s regions are §7's seen-state; a conversation is neither.
  > It hangs where you put it, and the one thing it borrows is the *owner* of the region under
  > it — so `set-virtual-text-visible` collapses a thread's rail with the rest of that line's
  > rails rather than leaving it behind alone.
  >
  > **The actor is which door was used, and that is the capability rather than a field.** §7's
  > rule is that the machine tracks claude; `reply-to-thread` is armed in *both* appliers and
  > each passes its own `Actor` — `Editing::act` yours, `AppHost::apply` his. A `who` parameter
  > would let the keyboard post as claude, which is the one thing that rule rules out. A planted
  > defect swapping the door's actor is caught by the pty test.
  >
  > **`:comment` anchors linewise, and a planted point anchor proved why that is `3a` rather
  > than a simplification.** The mockup hangs its rows under line 22 and tints that whole line;
  > a zero-width span draws the rows identically and produces **no marks at all**, because
  > `Tints::marks` needs `start < end`. So the defect was invisible in every assertion about the
  > rows and visible only in `Screen::tinted`. The test asserts the tint now, and its comment
  > says which half of §3 row 20 each assertion is holding.
  >
  > **Resolve is not delete, and a reply reopens.** `3a`'s subtitle is that the exchange is the
  > record of *why* a line looks the way it does — a verb that could only destroy it would make
  > finishing with a thread the lossy move. And an answer posted to a resolved thread reopens it,
  > because an answer hidden behind *"nobody is talking"* is an answer nobody reads.
  >
  > **The three other actors are not silently one of the two.** §1 gives `you` and `claude` a
  > colour each; Steel, the CLI and the editor itself are *doors*, and a reply that arrived
  > through one draws its own name in the meta neutral rather than borrowing a hue that names a
  > different actor.
  >
  > **All five verbs are typeable, and the lint chain is what made that true.** Ticking the task
  > turned `lint-capability-bindings` red for four of them — armed, on a ticked task, and reachable
  > by nothing — so `:reply`, `:resolve`, `:unthread` and `:broadcast` joined `:comment`. That
  > turned `lint-key-coverage` red in turn (a bound key no test presses), which is what put all
  > four into the pty test rather than leaving `3a` provable only through the REPL. Two lints in
  > sequence, each catching what the other could not see.
  >
  > **The `(string->number "")` trap sprang a third time.** An empty argument makes it `#false`,
  > which raises inside `key/cmd` and reaches the ex bridge as *"no such command"* — so a
  > registered command reports that it does not exist. `T060`'s `:defer` and `T067`'s `:annotate`
  > hit it before this; `every_ex_command_decodes` types every name with an empty argument and
  > has now caught it three times. `thread/id` guards all three numeric commands, and the comment
  > beside it names the count so the fourth time is a shorter debug.
  >
  > **`broadcast-thread` matches a literal substring, not a regex**, and that is a stated limit:
  > nothing in this tree parses one, and a `pattern` that quietly treated `.*` as three
  > characters would be worse than a narrow verb that says what it does. Loop-only, because it
  > needs the buffer's text.
  >
  > **Five planted defects, five catches** — a reply leaving a resolved thread resolved,
  > `thread_covering` ignoring the file, the strip counting resolved threads, the door replying
  > as the wrong actor, and the point anchor above (missed until the tint assertion existed).
  >
  > **One gap the test found before a lint could.** `resolve-thread`, `delete-thread` and
  > `start-thread` were armed only in the loop, so `(resolve-thread! 0)` at the REPL answered
  > `not built yet` while `:resolve` would have worked — a repl call is a *door* call. All three
  > have both halves now; `start-thread` on the door takes an explicit target only, since
  > `Target::Cursor` needs an editor. `broadcast-thread` stays loop-only for the same reason and
  > says so.
  >
  > `just gate` green — 1526 tests, confirmed with a bare unpiped run reading the exit code.

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

- [x] **T069 · Changed-on-disk indicator** — `✱` + offer to refresh. **Buffer holds stable.**
  Watching disk is `notify` + `notify-debouncer-full` (added by the spike — the design requires
  this and no document listed a dependency). **Debouncing is load-bearing:** an agent writing a
  file produces a burst of events, and one `✱` per burst is the honest signal.
  Screen `1d`. *Needs:* T015

  > **Built and ticked 2026-08-25.** A `notify-debouncer-full` watcher on the focused file's
  > *parent*, a `✱ disk changed` segment on the strip, `1d`'s corner box, and `:reload` /
  > `SPC r r` as the way out. Five planted defects, five catches.
  >
  > **The dependency was recommended, not added, and the entry said otherwise.** This task's own
  > line reads *"`notify` + `notify-debouncer-full` (added by the spike)"*. `SPIKES.md` names them
  > in its manifest at 8.2.0 / 0.7.0 and calls them *"not named anywhere in the design docs"* —
  > it **recommended** them; no `Cargo.toml` in the tree had ever carried either. The spike's job
  > was to choose, and choosing is not installing. Taken as **`notify-debouncer-full` only**,
  > because it depends on `notify` and re-exports it whole, so the manifest's pair is one crate in
  > the graph and a second direct row is a dependency `just unused-deps` is right to ask about.
  > In the binary rather than a library crate, on `nucleo`'s stated precedent: it owns a thread,
  > and a crate that spawns one outlives a frame.
  >
  > **The two design documents disagree about this screen's own keys — [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s
  > §61.** `1d` draws `:rr refresh · :dv diff`; Design Language §6 says *"spell the whole
  > command … never cryptic contractions like `:ca` or `:rr`"* and names `:rr` as its own
  > counter-example. §6 wins — it is normative where the mockup is illustrative, it was written
  > against that exact string, and the whole-word forms are the only ones registered: `:rr`
  > resolves to nothing, so the mockup's box offers two commands you cannot type. Ruled rather
  > than folded, because `docs/design/*.dc.html` round-trips elsewhere.
  >
  > **`1d`'s box is a float, and it must not take focus — those two facts nearly collided.** The
  > mockup's markup is `position:absolute; top:14px; right:14px` on `#171207` with a `#6b5426`
  > border, which is §4's needs-you pair exactly, so it is a *float* rather than the notice row it
  > reads as. But every float in this tree becomes `Surface::Float` and the keys follow it — and a
  > box that stole the cursor to say *nothing moved your cursor* would break the invariant the
  > screen exists to demonstrate. `T038`'s completion list already solved this: the
  > `(Surface::Buffer, _)` arm draws a float over a buffer you are still typing into. `1d` rides
  > that arm and borrows the **placement**, not the mood — §4 gives its box needs-you amber, not
  > the passive green completion uses.
  >
  > **The editor announced its own saves back to you, and an unrelated test caught it.**
  > `wall_writes_without_leaving` began drawing a frame no key had asked for: `:wall` wrote the
  > file, the watcher saw its own editor's write, and the buffer was told someone had changed it
  > underneath. Fixed by **comparing content** in the arm — `✱` means the two *disagree*, so
  > matching bytes are not a change — rather than suppressing by timing, which was the other way
  > out and is the wrong one: a timestamp window silently swallows a real change that lands inside
  > it, and the window has to be guessed. Content is the actual question, and it drops `touch`, an
  > identical rewrite and a formatter that changed nothing, for free.
  >
  > **`reload` preserves the line, not the character offset.** An offset is a position in a
  > *string*, so restoring one after the string changed puts the cursor wherever that many
  > characters now lands — reloading `before one` as `after one` moved it a column, because the
  > first line got shorter and offset 11 stopped meaning *"start of line 2"*. vim's `:e!` keeps
  > the line and so does this. It also goes through `Editing::splice` rather than rebuilding the
  > editor, which buys two things: the viewport does not scroll to the top, and the reload lands
  > in the undo tree so `u` takes you back. A refresh you cannot undo is a destructive act wearing
  > a refresh's name.
  >
  > **Three cursor assertions in a row passed against the defect they existed to catch**, and
  > that is the finding worth more than the feature. The first read `Screen::row`/`column` — the
  > *terminal's* cursor, which does not move under a planted `set_cursor(len)`. The second read
  > the strip's `line:column` but **captured** the expected value instead of writing it down, so
  > it compared a drifted reading to itself. The third wrote it down but reloaded a file of the
  > *same length*, where *"keep the line"* and *"jump to the end of the splice"* land on the same
  > row. Only the fourth — expectation written down, fixture deliberately longer — can fail. Each
  > was found by planting the defect; none would have been found by reading the test.
  >
  > **The burst count was flaky and moved out of the pty suite.** Querying `disk-state` right
  > after the `✱` appears races the debouncer's own window: it caught a planted `bursts += 2` on
  > one run and missed it on the next. A test whose verdict depends on the machine is worse than
  > no test, so the counter is held by `store::tests::one_delivery_is_one_burst`, which has no
  > clock in it. How long a filesystem takes to tell us is `notify-debouncer-full`'s property, not
  > this build's, and nothing here re-tests it.
  >
  > **`SPC r r` left the deferred-key table**, which is that table shrinking for the right reason
  > — the key acts now instead of naming a task. `SPC r d` stays until `T070`.
  >
  > **Verification.** `spc_r_r_takes_what_is_on_disk` presses the binding `lint-key-coverage`
  > asked for by name; `a_disk_change_under_the_buffer_moves_nothing` holds both halves of
  > invariant 3 — the strip says `✱ disk changed` while not one buffer row and not the cursor
  > moved — and `one_delivery_is_one_burst` holds the counter. Planted and caught: the strip never
  > saying `✱`, a change taken rather than offered, one save counting as two, a reload that does
  > not take what is on disk, and a reload that dumps the cursor at the end of its own splice.
  > `just gate` green — 1529 tests.
- [x] **T070 · `:diff-disk`** — your unsaved buffer vs Claude's disk write. Three manual exits,
  **no auto-merge**. Screen `5b`. *Needs:* T063, T069

  > **Built and ticked 2026-08-25.** `SPC r d` and `:diff-disk` open `5b`; the buffer is the left
  > column and claude's disk copy the right; `:take-disk`, `:keep-mine` and `:ask-claude` are the
  > three ways out. Five planted defects, five catches.
  >
  > **The vocabulary had already reserved the whole seam.** `DiffSource::Disk { buffer }` was
  > declared for this screen, `OpenDiskDiff`, `ResolveDiskDiff` and `DiskExit`'s three variants
  > all existed, and `Resources::diff` read `DiffSource::Disk { .. } | DiffSource::Change { .. }
  > => None` with a comment saying *"neither store exists"*. `T070` is that arm filled in and
  > almost nothing else invented — the widget (`T063`) and the disk state (`T069`) were both
  > already there.
  >
  > **The buffer is the diff's *from* side, and getting that wrong compiles.** `DiffBody` renders
  > side-by-side with the removed side on the left — its own words, *"a row with text on the left
  > and nothing on the right is a deletion"* — and `5b` draws `buffer · yours` against
  > `disk · claude`. So `similar::TextDiff::from_lines(mine, theirs)`. Reversed, it produces a
  > perfectly correct diff of the wrong two things: both versions still appear, nothing errors,
  > and the columns are backwards. **Only a position check catches it**, which is why the test
  > asserts *which half of the screen* each line lands in rather than that both are present. The
  > first version of that test asserted presence, passed against the reversal, and is the reason
  > this paragraph exists.
  >
  > **`5b`'s footer names a command that exists and does something else — §62.** The mockup draws
  > `:rr take disk · :w keep mine · :c ask claude`. §61 already ruled on `:rr` (Design Language §6
  > names it as its own counter-example); what is new here is that **`:c` is registered**, as
  > `c[omment]`, `T068`'s thread verb. A footer you can follow *into the wrong verb* is worse than
  > one you cannot follow. The exits take `DiskExit`'s own wire names instead, so the footer, the
  > ex line and the Action spell each exit identically.
  >
  > **No auto-merge, asserted as an absence.** Each exit test checks that the *other* side is
  > **gone** — `:take-disk` leaves nothing of yours in the buffer, `:keep-mine` leaves nothing of
  > claude's on disk. A merge would keep both and still look like a plausible file, which is
  > exactly why presence is not enough.
  >
  > **`:ask-claude` resolves nothing and says so.** It hands the disagreement over and leaves `5b`
  > open, because whether the file changes next is claude's turn rather than the command's — the
  > `✱` stays true until something actually moves. With no agent attached it declines by name; a
  > planted version that quietly fell back to `:take-disk` is caught, because an editor that picks
  > a side when nobody is listening is the auto-merge wearing a different hat. The message names
  > the file and the disagreement and carries **neither version**: claude wrote one and can read
  > the other, and pasting both would be the editor deciding which part of the diff mattered.
  >
  > **`set-diff-mode` is armed, and its `lint-action-arms` record is gone.** `4b` is *"one review
  > block as one unified diff"* and `5b` is the design brief's `:dv`, *"a side-by-side of buffer
  > vs disk"* — two surfaces, two modes, one widget. The mode rides in the surface args and the
  > arm **recomposes the float**, because a float is a snapshot: setting the field alone would
  > change a value nothing redraws, and the verb would look broken while being applied.
  >
  > **`similar` joined the binary and added no crate** — it is already in the graph through
  > `vendor/ratatui-code-editor`. `phosphor-ui` takes it dev-only on purpose, and its own note
  > says why the production side cannot live there: the two sides here *"are a buffer and a disk
  > copy, which a widget crate cannot read"*.
  >
  > **`SPC r d` left the deferred-key table**, the last row `S7.2` put there.
  >
  > **Verification.** Four keystroke tests — the diff drawn with each version in its own column
  > and the strip reading `DISKDIFF`, then each of the three exits taken in turn. Planted and
  > caught: the diff running backwards, `:take-disk` merging instead of replacing, `:keep-mine`
  > not writing, `:ask-claude` picking a side with nobody to ask, and a header that does not name
  > which side is which. `just gate` green.

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

- [x] **T071 · VCS trait + jj adapter** — compiled in, activated on detection. **No feature may
  assume a repo exists.** *Needs:* T041

  > **Built and ticked 2026-08-25.** `phosphor-vcs` was a six-line stub; it is now detection, a
  > `Status`, a jj reader and the `jj qpvuntsm ✓` chip. Five planted defects, five catches.
  >
  > **`None` rather than `Result`, and that is the acceptance line in the type.** *"No feature may
  > assume a repo exists"* — so `detect` answers an `Option` and there is no error to format
  > anywhere above it. The vocabulary had already said this twice before the crate did anything:
  > `vcs-status`'s own declaration reads *"every one of these answers empty in a bare directory —
  > no repository is a normal state, not an error"*, and the `Vcs` action group's reads *"an
  > enhancement, never a dependency"*. `Refusal::NoRepository` already existed for the actions.
  >
  > **Read on demand, never polled — and that is a correctness decision as much as a cost one.**
  > `refresh-vcs` exists precisely because the answer is *re-read*, so the binary caches a `Status`
  > and asks again. A poller would have put an asynchronous producer into every pty test in the
  > suite: the harness counts a frame per draw, `press` asserts one frame per key byte, and
  > `T069`'s watcher had to be switched off in tests for exactly that reason. This never needs the
  > switch. The statusline chip is the cache; the `vcs-status` query reads **fresh**, because a
  > chip is redrawn many times a second and a query is somebody asking now.
  >
  > **Detection is filesystem-only, and that is what makes it testable.** `detect` walks up for a
  > marker and never runs a subprocess, so *"is this a jj repo"* is answerable on a machine with
  > no jj installed. All six unit tests run identically with and without it — a fixture that
  > shelled out to a binary CI may not have would be a test that is quietly skipped there.
  >
  > **jj before git, and a colocated repo answers jj.** `jj git init --colocate` produces both
  > markers, and in that repo the jj store is the truth while `.git` is an export of it. Answering
  > `git` would describe the file the tool writes rather than the tool you are using. A planted
  > reversal is caught.
  >
  > **A `.git` *file* is a repository.** This very worktree is one — git marks a linked worktree
  > with a file pointing at the real gitdir, not a directory — so detection tests existence rather
  > than directory-ness. Found by running the query against the tree rather than by reasoning
  > about it: `(vcs-status)` answered `backend "git"` from inside the worktree.
  >
  > **The chip has three states, not two.** `jj qpvuntsm ✓`, `jj qpvuntsm ●`, and a bare `jj`.
  > *"I could not ask"* is not *"nothing to report"*: a backend whose binary is missing is still a
  > detected repository, and a chip that claimed clean would be inventing the one fact it failed
  > to read. A planted version that ticks on `None` is caught.
  >
  > **One `jj log` and not two.** `jj status` would answer the clean question directly, but a
  > second subprocess per refresh is a second chance to be slow on a big repo — and the template
  > already knows both, because `empty` is jj's own word for *"this change touches nothing"*,
  > which for `@` is exactly *"the working copy matches its change"*. Verified against a real jj
  > repo rather than assumed: `clean` before a write and `dirty` after.
  >
  > **`git` is detected here and read at `T072`**, so the chip says `git` and nothing it has not
  > earned. Detection could not wait for that task, because the colocated case has to be decided
  > in one place or not at all.
  >
  > **A test slot graduated for the third time.**
  > `a_posted_action_with_no_arm_names_its_task_and_its_producer` needs an Action with no arm; it
  > used `ingest-diagnostics` until `T040`, then `refresh-vcs` until this task. It is
  > `expand-diff-context` now, which is different in kind and should be the last: that is one of
  > the two capabilities `lint-action-arms.sh` records with **no creditor at all**, so nothing is
  > going to graduate it out from under the test.
  >
  > **Verification.** Six unit tests over detection and the chip — bare, nested, colocated, git,
  > a marker with no backend behind it, and the three chip states — plus a keystroke test that
  > `:refresh-vcs` reaches an arm and names the backend rather than a task. Planted and caught:
  > detection that does not walk up, a colocated repo answering git, a bare directory reporting a
  > repo, a chip that claims clean when it could not ask, and a status that forgets which backend
  > it detected. `just gate` green — 1542 tests.
- [x] **T072 · git adapter** — same trait. *Needs:* T071

  > **Built and ticked 2026-08-25.** `Repo::status`'s git arm, which `T071` left answering
  > all-`None` with a comment naming this task. Five planted defects, five catches.
  >
  > **Same trait, and *"same trait"* turned out to be the whole design.** Nothing here is new
  > shape: one subprocess, a `Status`, three chip states. What differs is only which two facts get
  > read and out of what — `git status --porcelain=v2 --branch` against jj's templated `log`.
  >
  > **The four states were captured before the parser was written**, from a real repository, and
  > they are the test fixtures verbatim: clean (headers only), untracked-only (`? b.txt`),
  > modified-tracked (`1 .M N… a.txt`), and detached (`# branch.head (detached)`). Writing those
  > from memory is how a parser ends up matching a format nobody emits — and two of the four would
  > have been guessed wrong, because the header set is larger than the docs' example and
  > `(detached)` is a literal rather than an absence.
  >
  > **The parser is a free function over the text**, so the parsing half runs on a machine with no
  > git — the same rule `T071` set for detection. Only the one-line subprocess needs the binary.
  >
  > **Untracked counts as dirty, and both backends agree.** git reports it as `? path`, and a tree
  > holding a file git has never seen is not one you could walk away from. jj reaches the same
  > answer from the other side, because its `empty` counts untracked files into the change — so
  > `●` means the same thing on both, which is what makes the chip readable at all.
  >
  > **A detached head names the short commit.** *"Which change am I on"* has an answer even with
  > nothing pointing at it, and that answer is exactly what jj's change id already is — so the two
  > backends produce the same *kind* of string rather than one of them producing a hole. A
  > repository with no commits reports `(initial)` as its oid and gets no id at all, because there
  > is genuinely no commit to name.
  >
  > **Verified live, not only against fixtures.** `(vcs-status)` from inside this worktree
  > answered `backend "git" change "worktree-s7-finish" clean #f` while the tree was dirty with
  > this very task's work.
  >
  > **One test was renamed rather than deleted.**
  > `a_git_repo_is_detected_before_its_adapter_exists` was true while `T072` was open; the adapter
  > exists now and what the test actually holds is the other thing — a `.git` marker with no git
  > behind it is still a repository, and reports the backend and nothing it has not earned.
  >
  > **Verification.** Eight unit tests in `phosphor-vcs`, all of which run with or without either
  > backend installed. Planted and caught: untracked not counting as dirty, `(detached)` reported
  > as a change name, `(initial)` invented into a commit id, the branch read from
  > `# branch.upstream`, and a clean tree reporting unknown. `just gate` green — 1544 tests.
- [x] **T073 · jj timeline** — agent turns are changes; undo is time travel. Screen `3b`.
  *Needs:* T071

  > **Built and ticked 2026-08-25**, and it closes `S7`. `SPC j` and `:timeline` open `3b`;
  > `↵` edits at a change, `d` shows its diff, `o` opens the operation log.
  >
  > **The one place this deviates from the mockup, and it is deliberate.** `3b` draws `· you` and
  > `· claude` against each change. **This build cannot honestly produce the second.** Nothing
  > creates a jj change per agent turn — `S6` built sessions and none of them commits — so every
  > change in a real repository is authored by whoever configured jj. What is drawn is the
  > *recorded* author, which is truthful and is ready for the day a turn becomes a change.
  > Inventing `claude` from a heuristic would be `3b`'s one claim no data supports, and the task's
  > own line — *"agent turns are changes"* — stays **aspirational** until something makes it true.
  > That is the honest reading of this task, not a shortfall discovered late.
  >
  > **Absence gets three sentences, because `CP-8c` reads them.** That checkpoint asks *"does
  > anything feel degraded or apologetic?"* — so a bare directory answers `no repository here`, a
  > git repository answers `the timeline is jj's — this repository is git`, and the `timeline`
  > query answers an **empty list** rather than refusing in either. Three situations, three
  > answers. Saying *"no"* without saying *"to what"* is what would read as apologetic.
  >
  > **The arm reads the cache, not a fresh detection**, and that is `T071`'s leak avoided rather
  > than repeated: detecting again inside the arm would step around `PHOSPHOR_VCS`, which is
  > exactly how the vcs chip put the phosphor checkout onto every pty test's statusline.
  >
  > **One templated `jj log`, parsed by a free function** — the third time in `S7.3`, after
  > `T071`'s status and `T072`'s `git status`. Six tab-separated fields per change, and the
  > template was verified against a real repository before the parser was written. `~root()`
  > filters jj's root commit, which has no author and no description and would otherwise draw as a
  > blank row at the bottom of every timeline; the op log drops its `0000` root for the same
  > reason.
  >
  > **A row with the wrong number of fields is dropped, not guessed at.** If the template ever
  > changes shape, losing a row is recoverable and producing one with its fields shifted along is
  > not — a change id that is really an email is the kind of wrong that reaches a screen looking
  > entirely plausible.
  >
  > **`---` and `+++` are headers, not a removal and an addition.** The diff parser is one
  > `starts_with` away from turning every file header into two phantom edits, and the result would
  > look correct. There is a test whose whole job is that line.
  >
  > **A test fixture was wrong and the code was right**, which is worth recording because it cost
  > a debugging pass: Rust's `\` line-continuation eats the newline *and* the following
  > indentation, so a diff context line written as ` kept` arrives as `kept`, the parser sees no
  > leading space, and a correct parser fails. The fixture spells it `\x20` now and says why.
  >
  > **`3b`'s keys are routed in Rust and listed in EMITTED**, which is the reason CLAUDE.md
  > already allows — *"a surface whose keymap is Rust"* — and the same shape `5c`'s inbox takes.
  > Guarded on the keys they use rather than on the surface, because the inbox arm records what
  > the other way costs: a guard on the surface alone swallowed `esc` and the float could not be
  > closed.
  >
  > **`main.rs` crossed the 1 MB hygiene ceiling on this task's last commit** — 22,123 lines. It
  > is allowlisted with a comment and filed at [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §64,
  > because deciding the shape of the binary is not this task's call and should not be made by
  > whichever task happens to add the byte that crosses the line.
  >
  > **`SPC j` left the deferred-key table**, the third row to graduate in `S7` after `SPC r r` and
  > `SPC r d`. What remains there is `T109`'s and `T110`'s, both open.
  >
  > **Verification.** Fourteen unit tests in `phosphor-vcs`, every one of which runs with or
  > without jj installed, plus a keystroke test that both spellings decline by naming the state in
  > a bare directory and in a git repository. `just gate` green — 1550 tests.

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

- [x] **T092 · Runtime theme switching — the rebuild path** 📌
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

  > **DONE 2026-08-25.**
  >
  > **Re-configured, not rebuilt.** `buffer_view::configure` and `soft_wrap::configure` are the
  > same two calls `buffer` makes at construction, and applying them to a live `Editor` keeps its
  > text, its cursor and its undo history — a `buffer(…)` rebuild would throw all three away. That
  > is the difference between switching theme and reopening every file, and it is why this is a
  > *rebuild path* rather than an arm: every widget takes a `&Theme`, so all of them have to be
  > handed the new one in the same beat, and the frame cache invalidated with them.
  >
  > **Both appliers needed an arm, and the pty test is what found that.** `AppHost::apply` serves
  > the three doors and `Editing::act` serves the loop; `:theme tokyo-night` is a *keystroke*, so
  > the door's arm never sees it. `phosphor --eval '(set-theme! "tokyo-night")'` answered `#ok`
  > while the running editor still said *"not built yet — T092 builds it"*. This is the same
  > two-appliers seam `T103` records, met from the keyboard side.
  >
  > **The slug is validated in the arm, not in the loop**, so `:theme nonesuch`, an MCP call and a
  > CLI verb answer one sentence — and the loop never has to invent a notice for a name it could
  > not resolve. Teej's ruling of 2026-08-13 was that `:theme` stays bound *"but only if something
  > is going to close it"*; this is that something.
  >
  > **`reload-theme` re-applies the palette that is drawing.** For a built-in that is the same work
  > as switching to it — the palette is a `const fn` and re-running it re-validates the actor hues
  > `T011`'s validator locks — and it is not a no-op, because `configure` is what puts a palette
  > *into* an `Editor`. A theme loaded from disk would re-read the file; none is, and saying so
  > beats pretending the two cases differ.
  >
  > **The test asserts the colour, not the text**, and it has to: a theme switch changes no
  > characters at all, so every text assertion in `loop_pty.rs` would pass against an editor that
  > ignored the command. `Screen::background` gives the escape sequence the terminal was actually
  > sent; `phosphor-dark`'s ground is `#0c0f0c` and `tokyo-night`'s is `#1a1b26`, so a `:theme`
  > that did nothing leaves the two readings equal — which is exactly how the missing keystroke arm
  > was found.
  >
  > **Two things about the loop's order, learned by getting them wrong.** The frame is composed
  > near the top of a pass and the Action's ask is drained near the bottom, so the palette that
  > changes during pass N is the one pass N+1 draws with — in a session that is invisible because
  > the next thing to arrive is you typing, but a test has to ask for it. And `press_quietly`
  > settles on *quiet* rather than on a frame, so it read the screen before the redraw landed;
  > `shown_on_grid` waits for the frame and is the right instrument. Neither was a defect in the
  > build, and both cost a run to find.
  >
  > Two planted defects, two catches: the theme local never reassigned, and an unknown slug
  > accepted.

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

- [x] **T094 · Reloading the editor layer** 📌
  `load-runtime-file` and `reload-runtime` are declared and unapplied: **the layer cannot be
  reloaded without restarting the editor.** `init.scm` reads the load order once at startup and
  the REPL evaluates forms; neither of those is this. It matters more than the other five,
  because invariant 1 is *"the editor layer is Steel in `runtime/*.scm`, **redefinable at
  runtime**"* and `CP-2` is the checkpoint that asks whether that is true. A layer you restart to
  reload is a config file with a longer reload cycle.
  *Done when:* editing a `runtime/*.scm` file and calling `reload-runtime` takes effect on the
  next frame with no restart; a broken file leaves the previous layer standing and reports the
  error the way a broken `init.scm` already does; a pty test covers both. *Needs:* T021, T026

  > **DONE 2026-08-25 — invariant 1's second half, and `CP-2` was the checkpoint for it.**
  >
  > `Layer::reload` re-runs the whole boot sequence in place. **The new runtime is built beside
  > the old one and swapped in only if its boot produced no fault**, which is the requirement
  > that shapes everything else: reloading in place and repairing on failure cannot work, because
  > half the load order has already run by the time the fault appears and there is nothing to roll
  > back to. A broken file therefore leaves the editor you already had — buffers, cursor, keymap —
  > and draws the boot float over it, which is *the same mechanism* a broken `init.scm` uses at
  > startup rather than a second one.
  >
  > **It is an Intent, not an arm.** The host is behind the Steel barrier and holds no `Layer`;
  > `Layer` is the one door into the VM and the loop owns it. The reason is sharper here than for
  > `Intent::Keymap`, which sits on the same seam: the thing being replaced is the runtime the arm
  > would be running inside.
  >
  > **The stack is re-run, not remembered.** `after_boot` exists to stop a file running twice
  > *within one boot*; a reload is a new boot, so the list is cleared or the user's own layer is
  > skipped as already-loaded — leaving the editor missing exactly the customisations the person
  > just asked to reload. `a_reload_runs_the_users_own_layer_again` is the test for it, and a
  > planted `after_boot` that survives is caught by that test alone.
  >
  > **Reachable by typing**: `:rel[oad]` re-runs the layer, `:reload <path>` loads one file on top
  > — the difference between *"pick up my changes"* and *"run this"*. Whole words, per Design
  > Language §6.
  >
  > **The pty test was the wrong instrument for the happy path, and finding out is the useful
  > part.** Intents drain *after* the frame is drawn, so a note set by the reload does not appear
  > until something else causes a redraw — and `press_quietly` settles on quiet, so the test saw a
  > screen with no notice on it and no way to tell *"the reload did nothing"* from *"the reload ran
  > and the new form did not take"*. Three unit tests over `Layer::reload` answer that directly and
  > in a second and a half; the pty test keeps the half it is genuinely good at — that after a
  > failed reload `x` still deletes a character, which needs the keymap the reload just failed to
  > replace.
  >
  > **A fixture bug worth recording, because it is this repository's recurring shape.** The
  > user-layer test first wrote to `<config>/phosphor/init.scm` — but `config::config_dir` already
  > resolves `$XDG_CONFIG_HOME/phosphor` and `AppHost::user_layer` joins the bare file name onto
  > it, so the file sat one directory below where the layer looks and the *boot* loaded nothing.
  > Had the test asserted only the reload, it would have passed against an editor that never ran
  > the user's file at all. The before-half is what caught it.
  >
  > **Three planted defects, three catches**, each by exactly the test written for it: an
  > `after_boot` that is never cleared, a broken layer swapped in anyway, and a reload that returns
  > without doing anything.

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

- [x] **T096 · `set-soft-wrap` — the verb** 📌
  The narrowest of the six and the clearest statement of the shape. **Soft wrap works.** `T081`
  built it, `--soft-wrap` turns it on, and `host.flag("soft-wrap")` at
  `crates/phosphor/src/main.rs:891` reads what `init.scm`'s `(set-option! …)` set. What does not
  work is the verb: `set-soft-wrap` is declared, generated into all three doors, and never
  applied — so it cannot be toggled at runtime from Steel, MCP or the CLI. A capability that the
  doors advertise and that does nothing is worse than one that is absent.
  *Done when:* `set-soft-wrap` toggles wrapping on the next frame from each of the three doors,
  and the flag and the verb read one piece of state rather than two. *Needs:* T081, T026

  > **Built 2026-08-23.** The verb applies, the flag seeds the option instead of
  > racing it, and `:wrap` / `:nowrap` are what a person types.
  >
  > **It was reachable from no door at all, and an arm was not enough.** Arming
  > `set-soft-wrap` in `Editing::act` makes it a *key's* verb; every door lands
  > in `AppHost::apply`, and the two appliers do not fall through to one another
  > on purpose. So `(set-soft-wrap! …)` at the REPL went on answering
  > `#refused · not built yet — T081 builds it` with the arm sitting right
  > there. It is the **fourth** capability this window to need an explicit line
  > on that forwarding list — after `apply-edits`, `goto-location` and
  > `apply-workspace-edit` — and the pattern is named in the comment now: a verb
  > whose whole point is that three doors can call it needs a line there.
  >
  > **One piece of state.** `cli.soft_wrap || host.flag("soft-wrap")` was read
  > every frame, which made the flag and the option two answers to one question
  > — and left the verb unable to turn wrapping *off* in a session started with
  > `--soft-wrap`, because the `||` put it straight back. The flag seeds the
  > option once at boot, **after** the layer loads so a command line overrides
  > `init.scm` the way one should, and the loop reads the option and nothing
  > else.
  >
  > **The target is honoured rather than ignored.** The capability's row says
  > *"which buffer"*, and a global toggle wearing a per-buffer signature is the
  > kind of almost-true this build spends its lints on. `Editing::soft_wrap` is
  > `Option<bool>`: a buffer that has been told answers for itself, one that has
  > not follows the option. A `bool` would make opening a file a decision about
  > wrapping. A target naming a *different* buffer is refused rather than
  > applied to the focused one — an `Editing` is one rope.
  >
  > **The `else` is the task.** Without it the loop wrapped and never unwrapped,
  > so the option moved and the rope did not: a toggle that works exactly once,
  > and the half a test of "does `:wrap` wrap" would never have found.
  >
  > **Verification.** One keystroke test over a line wider than the terminal:
  > no `↪` at rest, `:wrap` puts one there, `:nowrap` takes it away. Both
  > commands are pressed, which `scripts/lint-key-coverage.sh` required and
  > which made the test better than the REPL version it replaced. One planted
  > defect — the missing `unwrap` — caught on the second half. `just gate` green.

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

- [x] **T098 · Honest refusals for the deliberately-deferred vim keys** 📌
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
  >
  > **Closed 2026-08-23 by `T099`, and by more than a binding.** `m` went with `T042`; `q` and
  > `@` are bound to verbs that *do what the keys mean* rather than to refusals that name a task,
  > which is one better than this clause asked for. The entry's own diagnosis — *"a keymap cannot
  > ask a query"* — turned out to be half right and pointed at the wrong line: asking was fine,
  > and `phosphor/resolve` discarding a thunk's **answer** was the wall. Nothing in
  > `runtime/keymaps.scm` is bound to `key/deferred` any more.

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

- [x] **T099 · Macros — `q` and `@`, over `feed-keys`** 📌
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

  > **Built 2026-08-23. The wall was one line of scheme, and it was not the one
  > the entry named.** `runtime/keymaps.scm` recorded that *"a keymap cannot ask
  > a query"* — and asking was always fine. `phosphor/resolve` called a function
  > binding and **discarded its answer**: `[(function? binding) (begin (binding)
  > 'ran)]`. So a thunk could open a float or set an option and could not run an
  > Action, which is why `@` had nowhere to put the keys it had just read. It
  > honours a role now — a thunk that answers a *list* means it, and
  > `key/deferred`'s `void` still means `'ran`, which is what it always meant.
  >
  > **This is the generalisation the whole editor layer was missing**, not a
  > macro fix: a binding can now be *computed* from the editor's own state. `@`
  > is the first caller and the toggle below is the second.
  >
  > **It is `key/role?` and not `list?`, and a test taught the difference.**
  > The first version took *any* list as a role — and a thunk whose last
  > expression is a **capability call** answers the door's own receipt, which is
  > also a list. `the_rebind_is_live_on_the_very_next_key` binds
  > `(lambda () (open-repl!))`, and against a refusing host that made the
  > refusal itself look like a role: the key went `Unbound`. Only the five heads
  > the `key/…` constructors build are roles, which keeps a thunk free to end in
  > a call the way most of them do.
  >
  > **Commands, not keys.** `Machine::recording` holds `Vec<Vec<Key>>` because
  > the thing that must be dropped when recording stops is the `q<reg>` that
  > stopped it — one command and an unknown number of keystrokes. Keeping the
  > boundaries makes that a `pop` where counting keys would be wrong the first
  > time a register name took two presses. The **starting** `q<reg>` is excluded
  > for free: `apply` runs after `feed`, so recording is still `None` while the
  > command that turns it on completes.
  >
  > **The machine records and the host stores.** A register is the *editor's* —
  > `q` and `y` write the same thirty-odd slots — so a macro lands where a yank
  > lands and `@a` and `"ap` read one table. A second register table inside the
  > input machine would be two things that must agree about what `@a` plays.
  >
  > **One new query, and it earns its row.** `recording` answers which register
  > `q` is filling. It is what makes the toggle honest rather than a guess, and
  > it is the reader §5's strip would use to draw vim's `recording @a` — which no
  > mockup asks for and which is the natural companion if `q` is ever made
  > faithful.
  >
  > **`q<reg>` toggles where vim's `q` alone stops**, and that is a deviation
  > recorded at `OPEN-QUESTIONS.md` §58 rather than glossed. `q` is a prefix here
  > — twenty-six children, one per register — and a bare `q` meaning *stop* would
  > have to beat the prefix while recording and lose to it otherwise, which is a
  > resolution rule that depends on editor state where the resolver reads a table
  > and nothing else. The information is no longer the obstacle; the resolver's
  > legibility is.
  >
  > **Closes `T098`'s third clause.** `q` and `@` are bound to verbs that do what
  > the keys mean, so neither is silent and neither names a task.
  >
  > **Verification.** One keystroke test in the running binary: `qa`, `x`, `qa`,
  > then the register read back through the REPL door as `("x" "")` — the macro
  > and nothing recording — and `@a` replaying it, `alpha` → `lpha` → `pha`. Two
  > planted defects, two caught: a stop command that records itself (the register
  > read `"xqa"`, a macro that would turn itself off half way through its first
  > replay) and a resolver that discards a thunk's answer again. `just gate`
  > green.

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
  kind's arm — a question about composing something nothing composes. *(R3's capability half is
  answered as of 2026-08-20: `Interpreter::fill` is the channel, and the `Node::Gutter` arm reads
  it. The composition half is untouched.)*

  > **Stale on both numbers, and this is the paragraph above's warning coming true.** Run
  > 2026-08-20 in Window F's worktree: *30 node kinds, **21** composed, **9** recorded gaps (1
  > with no task that closes them)*. Two of the three closures were `T088`'s collapse composing
  > `Node::Pane` and `Node::Buffer`; the third moved before that and nobody noticed, which is the
  > point. The one with no creditor is still `Node::Gutter`, and everything the bullet says about
  > it holds.
  >
  > *(Moved here 2026-08-20. It was inserted mid-sentence — between "the state" and "column
  > **without** an editor" — which split the bullet in two and left its second half rendering as
  > a top-level paragraph. Found by the step-3 adversarial verifier, which read the diff rather
  > than the rendered page.)*

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

- [x] **T103 · The CLI verb route dispatches to the host** 📌
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

  > **DONE 2026-08-25 — and the MCP door had the same defect, undiscovered.**
  >
  > `dispatch` built a host and bound it to `_host` on both the verb path *and* the `--mcp` path.
  > The CLI half is what the task describes; **the agent-facing half was the same line, and
  > nothing named it**. Every MCP tool call but `eval` answered *"not built yet"* naming a task
  > that had shipped, which means the door an agent talks through was a stub for eleven tasks.
  > Found by fixing the CLI call site and grepping for the shape.
  >
  > `Evaluate` widened rather than went — the Scope's own *"which may widen or go"*. It has
  > `act` and `ask` beside `eval` now, and `Vm` carries `&AppHost` beside `&mut Layer`, so all
  > three doors reach one `Host::apply` and one `Answers::answer`.
  >
  > **The refusal had to split, and the splitting fact is not a table.** There are two appliers
  > that do not fall through to each other — `AppHost::apply`, which the doors reach, and
  > `Editing::act`, which the loop reaches — so a capability armed only in the second landed on
  > the first's fallthrough and answered *"not built yet — T026 builds it"* about a ticked task
  > with working keys. What separates the two cases is whether the **process has an editor at
  > all**, which `T111`'s published snapshot already answers: a `phosphor <verb>` invocation
  > starts, applies and exits without drawing a frame. So the arm reads that, and a running
  > editor is unaffected.
  >
  > **One more distinction was needed and the row already carried it.** `remove-watch` is
  > `S8`/`T074` and genuinely unbuilt, so *"no editor"* buried the useful answer — the door test
  > caught it. Unbuilt now outranks no-editor, keyed on `Since::phase` rather than on a list of
  > capabilities, so the set shrinks by itself when `S8` lands.
  >
  > **The refusal names the capability, and that is load-bearing.** All three parity walks proved
  > a call reached the dispatcher *as itself* by reading the task id out of the refusal — the
  > only per-row thing the line carried. With the doors dispatching there is no task id to read,
  > and the walks would have degraded to *"some capability answered"*. The name is the better
  > discriminator anyway: **21 rows share `T026` and no two rows share a name**, which is the
  > sharpening the entry above asked to carry either way.
  >
  > **Four walks had to learn what proves reach**, and getting it wrong first is where the
  > understanding came from. The strong form — *"both routes answer byte for byte"* — failed on
  > three rows that are **not defects**: `--eval` is itself an Action whose value is whatever the
  > expression produced, so a query answering `Null` prints `#ok` there and `#nil` through the
  > verb, and `vcs-status` diverges because the `--eval` answer passes through a Steel hash,
  > which prints fields sorted. Both are true sentences about different questions. The byte
  > compare is kept for Actions; a query is held to *"the door did not invent a refusal"*. And a
  > refusal in the capability's own terms — `no such thread` — is **stronger** evidence of reach
  > than any string match, so only a `not built yet` has to name this row.
  >
  > **A fourth cause was mine**: `eval_args` filtered on `required` while `cli_door` pushes every
  > flag the verb declares, so the two routes were handed different calls and then compared for
  > agreement. Three rows failed on that alone.
  >
  > **The side effect was ruled, and then it happened anyway.** The ruling is that the door is
  > *not* the place to refuse a write — writing is what `persist-form` is for — and the test is
  > the place to be isolated, via `Command::env`, since `std::env::set_var` is `unsafe` in
  > edition 2024 and this workspace denies `unsafe_code`. Isolating `parity.rs`'s `run` and
  > `door.rs`'s `phosphor()` helper left **one spawn uncovered — the `--mcp` server** — and a
  > green run wrote four lines of the literal word `sample` into a real
  > `~/.config/phosphor/init.scm`. That file is not merely untidy: `sample` is an unbound
  > identifier, so it raises a boot fault float on every subsequent start. Removed, the third
  > spawn isolated, and **verified by looking at the directory after a green run** rather than by
  > reasoning about which helpers were covered — which is the check that found it both times.
  >
  > Both isolated homes live under `target/`, so `cargo clean` takes them and no lint that walks
  > the worktree sees a stray file.

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

## E · The refusal audit — task ids a user reads that name finished work

Found on 2026-08-24 by scouting the plan, and the shape is one this build has met twice already
one layer down. `scripts/lint-action-arms.sh` proves a ticked task's mutation is *named* by the
binary and `scripts/lint-key-coverage.sh` proves every bound key is *pressed* by a test. Neither
reads the **sentence the refusal prints**, and that sentence carries a task id.

`Refusal::NotYetImplemented { task }` renders as *"not built yet — `{task}` builds it"*
(`crates/phosphor-core/src/action.rs`, `Refusal::sentence`). The id comes from one of two places:
the capability's own row in the macro table — derived, which is the shape `T098` praised — or a
string literal in the binary. **Both had gone stale, and in the same direction: every one of them
named a task that is ticked.** A reader who pressed the key was told to wait for work that had
already shipped.

**Nine capability rows were re-stamped on 2026-08-24 and needed no new task**, because the real
creditor already existed and `lint-action-arms.sh`'s own RECORDED table already named it —
`set-theme`/`reload-theme` to `T092`, `load-runtime-file`/`reload-runtime` to `T094`,
`undo-to-checkpoint`/`compact-history` to `T095`, `set-diff-mode` to `T070`. The table knew who
owed the work; the refusal named someone else. Re-stamping made the two agree and shrank that
table from nine recorded gaps to two.

**The two tasks below are the other half** — the literal task ids in `Editing::goto_sequence`,
which no capability row governs and which no creditor existed for at all. `docs/OPEN-QUESTIONS.md`
§18 set the precedent for exactly this: *"Eleven declared mutations that no task will ever close.
RULED: add the tasks."* An unowned debt recorded in a lint is a debt nobody is going to pay.

- [ ] **T109 · The sequence walks** 📌
  `Editing::goto_sequence` walks `Sequence::UnseenRegion` off the store and answers
  `Sequence::Ask` from the queue. **Four of the remaining sequences refuse, and every store they
  would walk is already built** — `Hunk` named `T063`, `BlockFile` named `T053`, `Diagnostic`
  named `T085`, `Thread` named `T068`, and all four of those tasks are ticked. Read against the
  tree this session: `store::Shared::hunks` answers hunks with their regions (`T064`),
  `review-blocks` answers a block's file groups (`T053`), `T085` draws diagnostics the gutter
  already ranks, and `store::Shared::threads_in` plus `region_span` answer a thread's place
  (`T068`). What is missing is the walk, not the rows.
  **Only `]b`/`[b` are bound** — `runtime/keymaps.scm` binds no key at all for hunk, diagnostic
  or thread, so those three refuse only through Steel, MCP and the CLI today. Binding them is
  half the task and `lint-key-coverage.sh` will ask for the presses.
  **It owns opening the file too, and `]b` is why it has no choice.** That key's own help text is
  *"next file in the review block"* — a walk that cannot leave the current buffer is not the
  feature. The same wall is already reachable from the other side: `Editing::jump` declines an
  anchor in another file, and until 2026-08-24 it declined by naming `T056`, which is OSC 8
  tool-row links and is ticked. Cross-file navigation from a store row is one capability with two
  callers, and this is it.
  *Done when:* `]h`/`[h`, `]b`/`[b`, `]d`/`[d` and `]t`/`[t` each move the cursor to the next and
  previous row of their own sequence in the running binary, each wraps the way `]u` wraps, each
  says something honest when its store is empty rather than refusing, a row in another file opens
  that file rather than declining — for the walks and for `jump` alike — and a pty test presses
  all eight. *Needs:* T049, T063, T064, T068, T085

- [x] **T110 · Search machinery** 📌
  `/` and `?` are bound to `open-prompt` with `kind` `search`, and `n`/`N` to `goto-sequence` over
  `search-match`. **All four refuse**, and until 2026-08-24 all four named `T058` — whose *done
  when* is `1c` raising from a keystroke, which shipped. `T058`'s own record says it plainly:
  *"Search is the half this task did not build … a search prompt needs somewhere to search, which
  is the search machinery rather than the line."* That machinery has never had a task.
  **The prompt is not the missing piece.** `T058` built the line, the anchor chip and the history;
  `PromptKind::Search` reaches an arm that declines. What does not exist is a matcher over the
  buffer, a match sequence for `goto-sequence` to walk, and the highlight that shows where the
  matches are. `T047`'s grep picker is *not* it and says so — it is nucleo over open buffer lines,
  a fuzzy picker, not an in-buffer regex search with a cursor.
  **Ruling to make before building, not after:** regex or literal. Nothing in this tree parses a
  regex, and `broadcast-thread` (`T068`) already took the narrow road and said so rather than
  treating `.*` as three characters. A `/` that silently matched literally would be the worse
  version of that choice, because vim users will type a regex on the first day.
  *Done when:* `/` and `?` search the buffer from the cursor in the running binary, `n` and `N`
  walk the matches and wrap, the ruling above is recorded at this entry, and a pty test presses
  all four. *Needs:* T058, T049

  > **DONE 2026-08-25. THE RULING: regex, not literal — and the argument against it was true of
  > phosphor's source and false of the binary.**
  >
  > The entry's case for the narrow road is *"nothing in this tree parses a regex"*. `cargo tree`
  > says otherwise: **`regex` 1.13.1 is already linked**, pulled in by `tree-sitter`, which every
  > phosphor crate depends on. Naming it directly adds no crate to the graph, no licence for
  > `cargo deny` and no MSRV movement — it is a `Cargo.toml` line over code that is already
  > compiled into the binary.
  >
  > That is what separates this from `broadcast-thread` (`T068`), whose narrow road was right
  > *there* because the alternative was a real new dependency. Here it is not, and the entry's own
  > warning decides it: *"a `/` that silently matched literally would be the worse version of that
  > choice, because vim users will type a regex on the first day."* The cost of the ruling is that
  > a pattern can be **wrong**, so an invalid one reports the crate's own message and **leaves the
  > previous search standing** — `T094`'s reload rule at a smaller scale.
  >
  > **`open-prompt` grew a `backward` argument** rather than the vocabulary growing a second
  > capability: `/` and `?` differ in which way `n` walks and in nothing else. The direction
  > belongs to the *search*, not to the key, which is vim — `?foo` then `n` goes up.
  >
  > **Two submit paths had to learn it, and only one is reachable by typing.** `PromptStep::Submit`
  > is what a door or a binding uses; while the prompt is open the **ex surface owns every key**,
  > so a typed `\r` goes through `ExStep::Submit` instead. Wiring the first alone left `/target`
  > being looked up in the ex command table and answered *"no such command — :target"*, which is
  > what the pty test found. Both route now.
  >
  > **Offsets are characters, not bytes.** `regex` answers byte offsets and `Editor::set_cursor`
  > takes a character index; one non-ASCII character above the cursor would put every jump wrong,
  > and every ASCII fixture would pass. The conversion is one pass rather than a
  > `chars().count()` per match, because searching for `a` in a large file is exactly the
  > quadratic case.
  >
  > **Matches are recomputed when the buffer has moved under them**, on the same `Editing::edits`
  > gate `T111`'s text snapshot and the LSP document sync use. Stale offsets would send `n` to a
  > position the text no longer has — worse than finding nothing, because it looks like it worked.
  >
  > **What the tests had to work around, and it is not a defect.** A notice occupies the
  > statusline row while it is up, so the `line:col` readout is *not* on the frame that says
  > *"3 matches"* — the landing position is read from where the next `n` goes, which is a stronger
  > assertion anyway: `4:1` after one `n` is only true if `/` landed on line 2.
  >
  > **Two tests elsewhere had to give up their rows**, both for the happy reason: `?` and `N` left
  > `a_deferred_binding_names_the_task_that_builds_it`'s table, and `/` and `n` left
  > `a_deferred_key_names_the_task_that_builds_it`. What stayed is the refusal that is still
  > honest — `n` before any search declines with *"no search yet — / or ?"*.
  >
  > **Not built, and not in the *done when*: the highlight.** The entry names *"the highlight that
  > shows where the matches are"* in its prose and does not ask for it in the criterion. It is not
  > here. `T087`'s marks side table is where it would go.

---

- [x] **T111 · The query answers** 📌
  **Twelve declared queries answer `not built yet` and every task they name is ticked.** Found
  2026-08-25 while building `T069`, by the same method that found `T109` and `T110`: reading a
  refusal and checking the task it cites. Measured against the built binary this session, not
  inferred —
  ```text
  phosphor --eval '(capabilities)'     #raised · not built yet — T024 builds it
  phosphor --eval '(options)'          #raised · not built yet — T021 builds it
  phosphor --eval '(dirty-buffers)'    #raised · not built yet — T033 builds it
  ```
  and `T024`, `T021` and `T033` are all ticked. The other nine are `describe-capability`,
  `describe-key`, `buffer-text`, `buffer-lines`, `mode`, `pending-keys`, `next-region-by`,
  `block-regions` and `floats`.
  **`scripts/lint-refusal-tasks.sh` could not see any of them**, and that is the finding inside
  the finding: it was written a day earlier against exactly this defect, and it reads capability
  rows in `action.rs` while `query.rs` carries the same `[phase / "task"]` stamp through the same
  `NotYetImplemented` sentence. A lint that covers one of two identical tables is a lint that
  reports clean on half a problem. It reads both now.
  **`capabilities` is the sharp one.** It is how a door enumerates what it can do, `T024`'s own
  *done when* is about that enumeration, and `6b`'s help surface is built on it — so the query
  the introspection story rests on has been refusing by name since `S2`.
  *Done when:* each of the twelve answers its own row from the host rather than refusing, a
  door test calls every one of them through `--eval` and asserts none raises, and the row is
  re-stamped back off `T111` as each lands. *Needs:* T024

  > **DONE 2026-08-25 — and it was twelve in the task and twenty in the tree.**
  >
  > The twelve above were found the way the entry says, one refusal at a time. Auditing every
  > row mechanically — declared queries against the arms actually matched in
  > `impl Answers for AppHost` — found **22 refusing, and 20 of them naming a task that is
  > ticked**. The eight the entry does not list are `buffer`, `buffers`, `cursor`, `selection`,
  > `viewport`, `keymap`, `review-block` and `theme`, naming `T033`, `T026`, `T010` and `T066`.
  > Measured on the built binary, not inferred: `(theme)` answered *"not built yet — T010 builds
  > it"* about a task that shipped at `S1`.
  >
  > **`lint-refusal-tasks.sh` could not see any of the eight, and the reason is the finding.**
  > Its arming check asked `re.search(r"\bVariant\b", production)` — *"does this name appear
  > anywhere?"* — and every one of those eight variant names is also an ordinary Rust identifier
  > in `main.rs`. `Buffers` occurs 28 times, `Cursor` 16, `Theme` 9, `Keymap` 9. All eight read
  > as armed. The check now requires the name to appear as an **enum path** —
  > `\w*(?:Query|Action)::Variant` — which `phosphor_ui::theme::Theme` fails and `UiQuery::Theme`
  > passes. **Third widening of this lint, and the third one found by measuring the binary rather
  > than by reading the lint**: actions-only, then both tables, now both tables precisely.
  >
  > **Where the answers come from, and what each costs.** Two are pure registry reads
  > (`capabilities`, `describe-capability`) and cost nothing — a `const` table cannot move, so
  > they answer at `Revision::INITIAL`. One reads the options map the host already holds. Three
  > read the store. The remaining fourteen are questions about the *editor*, and everything the
  > editor is made of is on the loop's side of the barrier — `Editing` holds an `Rc<Cell<bool>>`
  > and is not `Send`. So they read a per-frame `EditorSnapshot`, which is
  > `HostState::panes`' published-rather-than-reached-for pattern applied to the rest of the
  > screen.
  >
  > **The text is the one field that could not be published naively**, and the guard is
  > `Editing::edits` — the same counter the LSP document sync in the same loop already uses, so
  > a buffer nobody typed into is not copied and the cost is one copy per committed edit batch
  > rather than per frame. `the_snapshot_reuses_text_the_edit_counter_says_has_not_moved` is
  > built to fail if that guard is ever removed: the carried entry claims the counter the buffer
  > really has and holds text the buffer never contained, so a rebuild overwrites it and the
  > guard keeps it.
  >
  > **The keymap is the one that cannot be read per frame at all.** `Layer::entries` sets the
  > stale flag *on purpose* — `keymap-entries` is a name the layer owns and may redefine — so
  > calling it every frame would recompose the statusline every frame and delete the point of
  > `T079`'s cache and `CP-2`'s benchmark. It is refreshed only on a frame where scheme had
  > already run, and the flag its own call raises is taken back immediately, because letting it
  > survive would ratchet the editor into permanently-stale. **The narrow cost is recorded
  > rather than hidden**: a `keymap-entries` that mutated state would not invalidate the
  > following frame. This is the same self-invalidating-frame trap `1d` fell into at `T069`,
  > met from the other side and avoided by reading the flag's own doc first.
  >
  > **One ruling the build had to make**, at the `next-region-by` arm. The row takes a bare
  > `from: Option<Position>` with no path, and regions live in files — line 12 of one file does
  > not sort against line 12 of another, so a position alone cannot order the walk. The file
  > therefore always comes from focus and `from` only moves the position *within* it; the order
  > is (path, line, column) across the workspace and it wraps, so `]r` runs off the end of one
  > file into the next the way `]u` already does. With no buffer focused it answers `Null`
  > rather than guessing at the first region in the workspace.
  >
  > **`floats` answers by surface rather than by body**, which is what makes it cheap enough to
  > publish every frame: a float's `body` is the whole composed subtree, and copying it would
  > put the picker's contents into every frame that has a picker open. A passive float —
  > completion, signature help, `1d`'s notice — is deliberately not listed as focused, because
  > §9's rule is about which surface takes the keys and `Mood::Passive` takes none.
  >
  > **Verification, and every one of it was watched failing.** `parity.rs`'s
  > `every_query_answers_or_names_a_task_that_is_open` walks the registry, builds each call from
  > `registry::sample` with **required arguments only** — `(keymap)`, not `(keymap #false
  > #false)` — and runs it through the real binary; the two `S8` watch queries are a shrink-only
  > `OWED` table. Removing the `theme` arm makes it fail with `(theme) -> #raised · not built
  > yet — T010 builds it`. Three unit tests cover what `--eval` cannot see, because an empty
  > answer is a pass there: three planted defects — the text guard bypassed, the closed-buffer
  > `retain` removed, and a 0-based cursor — were each caught by exactly the test written for
  > them.
  >
  > **What is left refusing is two rows, and both are honest**: `watches` (`T074`) and
  > `watch-values` (`T075`), which are `S8` and cannot answer about a watch that has no model
  > yet.

---

## F · What `S7` left aspirational

One task, and it exists because `T073` ticked with a claim its own title makes
and its own build could not meet. Recorded as a task rather than swallowed,
because the alternative was drawing something the data does not support.

- [ ] **T112 · An agent turn is a change** 📌
  **`T073`'s title says *"agent turns are changes"* and nothing makes that true.** `3b` draws
  `· you` and `· claude` against each row; what the timeline actually draws is the *recorded*
  author, which in every real repository is whoever configured jj. `T073` shipped the truthful
  version and said so at the field — inventing `claude` from a heuristic would be `3b`'s one
  claim no data supports.
  **What would make it true, and the decision inside it.** A turn would have to *become* a jj
  change: `jj new` when a turn begins, `jj describe` with the prompt when it ends. `S6` built
  every part of the turn lifecycle — `turn-began` and `turn-ended` already exist as capabilities
  — so the hook has somewhere to go.
  **The reason this is not obviously right.** It makes the editor *write to your repository* on
  the agent's behalf, which is a different promise from anything `S7` shipped: `T069` watches
  disk and never refreshes, `T070` offers three exits and merges nothing, `T071` reads and never
  commits. Every VCS surface so far has been strictly read-only, and this would be the first
  that is not. Invariant 5 — *"VCS is the safety net that lets there be no review ceremony"* —
  is an argument **for** it, and `7a`'s permission gate is the shape that would make it
  acceptable; but it is a promise change and belongs to Teej rather than to whichever task
  notices it is missing.
  **Cheaper half, if the full one is unwanted:** attribute from the author *email* when it
  differs from the repo's configured user. That costs one `jj config get` and makes `3b`'s two
  names real for anyone whose agent already commits under its own identity — without this
  editor ever writing a change itself.
  *Done when:* either a turn creates a change and `3b` draws `· claude` against it from recorded
  data, or the entry is closed with a ruling that it should not and `3b`'s actor column is
  amended to what the build can honestly draw. *Needs:* T050, T073

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
