# Phosphor — task breakdown

Decomposed from [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md), which is itself derived from
the four design docs in [design/](design/). The plan says *what each phase is for*; this file
says *what to build, in what order, and where we stop and look at it*.

**80 tasks + 9 harness tasks · 12 checkpoints · 9 phases**, covering all 34 screens v1 builds.
Phase ids (`M-0`, `S1`…`S8`) match the plan and the Component Breakdown's build order. Task ids
are stable and assigned in order of creation — reference them in commits. New tasks append
rather than renumber, so `T078`+ sit inside earlier phases.

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
| M-0 · Scaffolding + spikes | T001–T009 | **CP-0** — go/no-go on both bought crates |
| S1 · Theme + BufferView + StatusLine | T010–T018 | **CP-1** — does it look like the mockups |
| S2 · Steel + Action + REPL + view tree | T019–T025, T078–T080 | **CP-2** — is the editor live |
| S3 · Input + undo + gutter | T026–T035 | **CP-3** — does it feel like an editor |
| S4 · LSP | T036–T040 | **CP-4** — boring on purpose |
| S5 · Store + seen + Picker | T041–T049 | **CP-5** — the awareness loop |
| S6 · ACP + MCP + Transcript + Prompt | T050–T062 | **CP-6** session · **CP-7** directing |
| S7 · Diffs + review + dirty + VCS | T063–T073 | **CP-8a/b/c** — three workstreams |
| S8 · Watches | T074–T077 | **CP-9** — ship check |
| **V · Verification harness** | **V001–V009** | *cross-cutting — lands with S1, used from CP-1 on* |

---

## V · Verification harness

The Tier-2 layer. Not product code — it is how every later checkpoint gets cheap. `V001`–`V005`
land alongside S1 so `CP-1` can use them; the rest follow as the surfaces they capture appear.

Separately numbered from the `T` tasks because it is a distinct workstream with a different
lifetime: the harness outlives any single phase and gets extended at every checkpoint.

- [ ] **V001 · Pin VHS and its dependencies**
  VHS 0.11.0 + `ttyd` + `ffmpeg`, pinned by exact version. `Require` at the top of every tape so
  a missing dep fails loudly rather than silently producing a wrong recording. Record the
  reference-regeneration machine and font — pixel comparison is only meaningful against a fixed
  renderer.
  *Done when:* `just tapes` fails with a clear message on a machine with the wrong VHS version.
  *Needs:* —

- [ ] **V002 · Column calibration**
  Map `(FontSize, Width)` → exact column count, since VHS sizes in pixels. Build a probe tape,
  binary-search the width for 80 / 100 / 120 / 200 columns, and commit the table as
  `tapes/_dimensions.tape`. **Also settle the two open empirical questions here:** does undercurl
  survive capture, and do captured pixels match theme hex values exactly?
  *Done when:* a tape asserting "exactly 80 columns" is reproducible, and both questions have
  written answers. *Needs:* V001

- [ ] **V003 · Shared tape config**
  `tapes/_config.tape`, `Source`d by every tape: pinned font and size, `Set CursorBlink false`,
  fixed `TypingSpeed`, fixed `Framerate`, `Set Padding 0`, neutral background. **Every source of
  nondeterminism removed** — anything that varies between runs makes pixel comparison useless.
  *Done when:* the same tape run twice produces byte-identical PNGs. *Needs:* V002

- [ ] **V004 · Deterministic waits — no `Sleep`**
  Use `Wait+Screen /regex/` against a known sentinel instead of sleeping. Phosphor needs a
  stable, greppable ready-state for this; the statusline is the natural sentinel.
  *Done when:* no tape in the library contains a bare `Sleep` as a synchronisation primitive.
  *Needs:* V003

- [ ] **V005 · Tape library convention**
  One tape per screen id: `tapes/<id>.tape` → `Screenshot artifacts/<id>.png`, plus a GIF where
  motion is the point. `Hide`/`Show` around setup so only the interesting frames are captured.
  *Done when:* `just tape 1a` regenerates one screen; `just tapes` regenerates all. *Needs:*
  V004

- [ ] **V006 · Deterministic fixture repo**
  A committed sample tree plus **seeded store state** — regions, seen-state, threads, a canned
  transcript. Without this, every agent-surface tape is flaky, because the content varies run to
  run. Seed it through `phosphor --eval` (`T023`), not a test-only backdoor.
  *Done when:* `CP-5`'s tapes produce identical output on two machines. *Needs:* V005, T023

- [ ] **V007 · Pixel-diff runner**
  Compare fresh captures against committed references; on mismatch, emit a side-by-side diff
  image and **fail soft with a request to look**, not a build break. Reference updates are an
  explicit, reviewed commit — never automatic.
  *Done when:* a deliberate one-cell colour change is caught and produces a legible diff image.
  *Needs:* V005

- [ ] **V008 · CI wiring**
  Tier 1 snapshots **gate**. Tier 2 runs, uploads artifacts, and posts the diff summary without
  blocking. Keep them in separate CI jobs so a flaky renderer can never redden a correct build.
  *Done when:* a Tier-1 failure blocks merge and a Tier-2 diff does not. *Needs:* V007, T005

- [ ] **V009 · Degradation tapes**
  `Env TERM xterm-256color` and `Env NO_COLOR 1` variants of the core screens, exercising the
  fallback paths (`▎` markers, underline instead of undercurl, static `✻`).
  *Done when:* the degradation path is captured for `1a` and `2a` without touching a real
  terminal. *Needs:* V005

---

## M-0 · Scaffolding and the spikes

Nothing here is blocked on a decision. The two spikes are reads, not builds, and they size
everything after them — do them first, together.

- [ ] **T001 · Cargo workspace skeleton**
  Seven crates (`phosphor`, `-core`, `-buffer`, `-ui`, `-agent`, `-steel`, `-vcs`) plus
  `runtime/` as a plain source dir. Stub lib/main only.
  *Done when:* `cargo build` green. *Needs:* —

- [ ] **T002 · Pin the dependency floor**
  `ratatui 0.30.2`, `ratatui-core 0.1.2`, `steel-core =0.8.2` (exact, per Q5), `ropey`,
  `tree-sitter`, `crossterm 0.29`. `phosphor-ui` gets `ratatui-core` only — never `ratatui`.
  *Done when:* `cargo tree` shows no second ratatui major. *Needs:* T001

- [ ] **T003 · Vendor `ratatui-code-editor`**
  `git subtree` into `vendor/`, workspace path dep, `VENDOR.md` with upstream SHA + patch log,
  `just vendor-diff` and `just vendor-pull`.
  *Done when:* `just vendor-diff` prints an empty diff against the merged tag. *Needs:* T001

- [ ] **T004 · Vendor `ratatui-markdown` and bump it to 0.30**
  Version bump only — no phosphor behaviour inside it (Q4). Feature-gated; per-language
  highlight features off.
  *Done when:* it compiles in-workspace and the gate can be toggled off cleanly. *Needs:* T002

- [ ] **T005 · CI: fmt, clippy, test**
  `cargo fmt --check`, `clippy -D warnings`, `cargo test` on every push.
  *Done when:* green on the empty workspace. *Needs:* T001

- [ ] **T006 · Structural lint — no literal colours in `phosphor-ui`**
  Every widget takes `&Theme`. Grep-level lint over `Color::Rgb` / `Color::Indexed` in that
  crate is sufficient.
  *Done when:* CI fails on a deliberately planted literal. *Needs:* T005

- [ ] **T007 · Structural lint — no store mutation from `phosphor-ui`**
  Split `phosphor_core::vm` (ViewModels) and `phosphor_core::view` (the view tree, Q12) — both
  public to the UI — from `phosphor_core::store` (mutation, not). Enforced by dependency
  direction, not convention.
  *Done when:* CI fails on a deliberately planted `store::` import in `phosphor-ui`.
  *Needs:* T005

- [ ] **T008 · SPIKE — the five seams in `ratatui-code-editor`**
  Read the 0.0.6 source and answer, in `VENDOR.md`, yes/no + how for each: marks API, gutter
  column injection, virtual-text row interleaving, scroll authority, diff view separability.
  Plus: **is the undo history reachable and serialisable?** (settles the open half of Q2).
  *Done when:* `VENDOR.md` has six answers with file/line citations. *Needs:* T003

- [ ] **T009 · SPIKE — can edtui's handler emit Actions?**
  Read `edtui`'s `KeyEventHandler`. Can it drive an external `Action` sink, or is it welded to
  its own `EditorState`? Write up the seam or the blocker (Q3).
  *Done when:* a written verdict with a proof-of-concept diff or a clear "no". *Needs:* T002

### ✋ CP-0 — Go/no-go on both bought crates

The most consequential checkpoint in the build, and the cheapest. Both spikes are reads.

**Claude verifies:** `cargo build` green · both structural lints fail on planted violations ·
`VENDOR.md` answers all six seam questions with citations · both vendored subtrees build.

**Teej verifies:** read both spike write-ups and make the buy-vs-build call. This is a judgement
about how much of the editor we want to own, and it is yours.

**Decision table:**

| Spike outcome | Consequence |
|---|---|
| Both seams present | Proceed as planned. Best case. |
| `ratatui-code-editor` seams missing | `BufferView` is built on `ropey` + `tree-sitter`; S1 grows substantially. Budgeted in the plan §2. |
| Undo not serialisable | We own the undo stack; the bought editor drops to renderer + edit primitives. Q2's open half resolves toward `phosphor-buffer` owning more. |
| edtui welded to its own state | Custom input machine behind the same `Action` layer. S3 grows. |
| **Both fail** | **Stop and re-plan.** The buy-first posture doesn't hold, and the shape of the whole UI layer is back on the table — including whether edtui becomes the buffer core instead (the alternative logged under Q3). |

---

## S1 · Theme + BufferView + StatusLine shell

First phase with anything to look at. Sized by CP-0.

- [ ] **T010 · `Theme` struct**
  Actor/state palette (`claude, you, attention, trouble, transient, steel`) + neutral ramp +
  syntax map. Values from Design Language §1.
  *Done when:* every colour in the language has a named field. *Needs:* T001

- [ ] **T011 · base16-style loading + actor-hue validation**
  A theme that reassigns an actor hue is **rejected at load**, not accepted-and-warned.
  *Done when:* a fixture theme with a red `claude` fails to load with a legible error.
  *Needs:* T010

- [ ] **T012 · Phosphor dark + light built in**
  Dark is the default. Light is "warm paper with deepened hues" — claude-green `#1a9a62`.
  *Done when:* both load and pass validation. *Needs:* T011

- [ ] **T013 · Catppuccin + Tokyo Night mappings**
  The two shipped mappings (Q7 — Ayu is out). Each dark + light. Screen `9a` is Catppuccin;
  Tokyo Night has no mockup and inherits `9b`'s acceptance shape.
  *Done when:* all four pass actor-hue validation. *Needs:* T011

- [ ] **T014 · Terminal setup + synchronized output**
  Raw mode, alt screen, panic/exit restore, and a **draw wrapper that puts every frame inside a
  synchronized-output block**. Kitty keyboard protocol negotiation with fallback detection.
  *Done when:* no frame can be emitted outside the wrapper (enforce by making the raw writer
  private). *Needs:* T002

- [ ] **T015 · BufferView — the 3-column contract**
  1-cell state bar → line numbers (`#414b42`, always) → text. Tree-sitter highlighting through
  the vendored core. **Scroll authority lives here** — the viewport moves only on an explicit
  Action.
  *Done when:* a file renders with correct columns and the viewport provably never
  self-scrolls. *Needs:* T014, CP-0

- [ ] **T016 · Soft-wrap and folds**
  `↪` continuations carry no line number. Fold rows render `▸ ⋯ n lines`. Insert-only
  trailing-whitespace marks.
  *Done when:* screen `8e`'s text details reproduce. *Needs:* T015

- [ ] **T017 · StatusLine**
  Mode chip (the only inverted text on screen) + file + dirty flag + spring + `SessionState`
  (renders `None` for now) + counters, joined by `│`. **Truncation enforced in the widget** —
  emitting a second row must be impossible, not merely avoided.
  *Done when:* a property test at widths 40–200 never produces two rows. *Needs:* T010

- [ ] **T018 · Golden-frame snapshot harness**
  Render a fixed buffer + state to a cell grid, compare against a committed snapshot. This is
  how every later phase gets cheap regression cover on layout.
  *Done when:* snapshots exist for `1a`-minus-agent, `9c`, `8c`, `8d`. *Needs:* T015, T017

### ✋ CP-1 — Does it look like the mockups?

The baseline visual checkpoint, and the one that establishes your terminal matrix. Everything
after this trusts that colours and frames are right.

**Run:** `cargo run -- src/some_real_file.rs`

**Claude verifies:** snapshot tests pass for `1a`-minus-agent, `9c`, `8c`, `8d` · statusline
property test (widths 40–200, never two rows) · all four themes pass actor-hue validation · the
planted bad theme is rejected · no `Color::Rgb` literal survives in `phosphor-ui`.

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

- [ ] **T019 · `Action` enum + query vocabulary**
  The single mutation API — buffer edits, seen marks, session messages, float open/close. Plus
  the read side over ViewModels. Design it for the surfaces in *all* the mockups, not just S1's.
  *Done when:* every mutation in phases S3–S8 has a named Action, even if unimplemented.
  *Needs:* T007

- [ ] **T020 · The tri-door registry**
  One registration per capability yields the Steel binding, the MCP tool schema, and the CLI
  verb. Adding a capability to one door must add it to all **by construction** — this is
  invariant 2's only real defence.
  *Done when:* a new Action registered in one place appears in all three doors with no further
  edits. *Needs:* T019

- [ ] **T021 · Embed `steel-core`; boot `init.scm`**
  A **broken `init.scm` boots the editor anyway**, with the error in a float. Steel can emit
  Actions and read ViewModels — nothing else.
  *Done when:* a syntax error in `init.scm` yields a working editor with a legible error float.
  *Needs:* T020

- [ ] **T022 · Steel REPL**
  The primary extension workflow, not a debug tool. `(keymap-set! …)` is live; the next frame
  has it.
  *Done when:* screen `6b` reproduces. *Needs:* T021

- [ ] **T023 · `phosphor --eval` (the CLI door)**
  Nearly free once the registry exists.
  *Done when:* `phosphor --eval` and the REPL return identical results for the same expression.
  *Needs:* T020

- [ ] **T024 · Door-parity test**
  A test that **enumerates the registry** and asserts every capability is reachable from all
  three doors. Enumeration is the point — a hand-written list rots.
  *Done when:* adding an Action reachable from only one door fails CI. *Needs:* T020, T023

- [ ] **T025 · StatusLine composition moves to Steel**
  Not just segment *order* — the statusline is **composed as a view tree returned from Steel**
  (Q12): which segments, in what order, with what shed priority. The first real surface to prove
  the tree protocol on, and small enough to get wrong cheaply.
  *Done when:* redefining the whole statusline composition in the REPL changes the next frame.
  *Needs:* T017, T022, T079

> **Appended after the initial breakdown** (Q12). Ids are assigned in order of creation, not
> position, so `T001`–`T077` keep the meanings they were committed with.

- [ ] **T078 · The view-tree protocol**
  `phosphor_core::view` — the tree as plain data: **no Steel dependency, no ratatui
  dependency**, so neither side owns the contract. Node kinds for every `phosphor-ui` primitive
  plus layout and the `spans` escape hatch.
  *Done when:* the crate compiles with neither `steel-core` nor `ratatui` in its dependency
  tree. *Needs:* T019

- [ ] **T079 · Tree interpreter + frame cache**
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

### ✋ CP-2 — Is the editor live?

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

Shape depends on CP-0's edtui verdict — T026 is the fork point.

- [ ] **T026 · Input adapter → Actions**
  Either edtui's handler driving an Action sink, or a custom modal machine behind the same
  layer. Modes, counts, registers, operator-pending.
  *Done when:* a scripted keystroke sequence produces the expected Action stream. *Needs:* CP-0

- [ ] **T027 · Kitty keyboard protocol**
  Real modifier chords, with graceful fallback where unsupported.
  *Done when:* `ctrl+shift+<key>` is distinguishable from `ctrl+<key>` on the primary terminal.
  *Needs:* T014, T026

- [ ] **T028 · Agent nouns as text objects**
  `viu`, `sib`, `dih`, `:'<,'>c` register in the grammar. **They parse here and resolve at
  S5** — there is no store to resolve against yet (Q8).
  *Done when:* the grammar accepts them and they no-op cleanly rather than erroring.
  *Needs:* T026

- [ ] **T029 · Undo model in `phosphor-buffer`**
  Owns the undo tree and edit semantics (Q2).
  *Done when:* undo/redo across a scripted edit sequence is exact. *Needs:* T026

- [ ] **T030 · Undo persistence in `phosphor-core`**
  Append-only log + compaction, **sharing its format and compaction path with seen-state**
  (T044). Design the format once, here.
  *Done when:* undo history survives a clean restart *and* a `kill -9`. *Needs:* T029

- [ ] **T031 · GutterBar**
  1-cell state column, priority trouble > attention > claude-unseen > none, `▎` degradation.
  Renders from `Vec<RegionState>` — fixtures for now, real regions at S5.
  *Done when:* priority resolution unit-tested across all overlap combinations. *Needs:* T015

- [ ] **T032 · VirtualText**
  `┊`-prefixed rows owned by a region id, indented to the code column. Shared by threads,
  watches, diagnostics, hints.
  *Done when:* rows interleave correctly and never shift the buffer's own line numbering.
  *Needs:* T015

- [ ] **T033 · Keymaps + leader tree in Steel**
  `SPC` leader, full ex commands, vim-style unique-prefix abbreviation.
  *Done when:* every binding lives in `runtime/`, none in Rust. *Needs:* T022, T026

- [ ] **T034 · KeymapFooter / WhichKey**
  Same data, two densities. Reads the **live** keymap, so Steel rebinds appear with no extra
  wiring. Keyhints spell whole commands — `:reattach`, never `:ca`.
  *Done when:* screen `3c` reproduces and a REPL rebind shows up in it. *Needs:* T033

- [ ] **T035 · Unknown-key hint**
  One virtual-text line naming `SPC` and `:help`, once per session, never again.
  *Done when:* screen `8e` reproduces. *Needs:* T032, T034

### ✋ CP-3 — Does it feel like an editor?

The first checkpoint that is mostly about feel, and the only one where muscle memory is the
instrument.

**Run:** `cargo run -- <a real file you'd actually edit>`

**Claude verifies:** scripted keystroke → Action stream tests · undo/redo exactness · undo
survives restart and `kill -9` · gutter priority resolution across all overlaps · `3c` and `8e`
snapshots · every binding lives in `runtime/`.

**VHS produces:** the leader popup opening (`3c`) · folds collapsing and expanding · soft-wrap
continuations · insert-only whitespace marks · the once-per-session unknown-key hint firing and
then *not* firing again (`8e`). Keystroke-driven surfaces are where tapes are strongest — the
input is scripted, so the capture is exact.

**Teej verifies:**
- **Actually edit something real for a while.** Not a test file — something you were going to
  change anyway. Vim habits should carry without thinking about it.
- Where does muscle memory break? Every miss is a finding; note them all, they won't recur to
  you later.
- Counts, registers, operator-pending: `3dd`, `"ayy`, `ci(`. Do they compose?
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
  T022

- [ ] **T038 · Completion via the passive Float**
  Border `#2a3c2e`, **no footer** — the one documented exception to the float contract.
  *Done when:* screen `7c`'s completion reproduces. *Needs:* T036

- [ ] **T039 · Signature help + hover**
  *Done when:* screen `7c` reproduces in full. *Needs:* T038

- [ ] **T040 · Diagnostics → gutter + virtual text**
  Trouble priority in `GutterBar`; `■` rows via `VirtualText`; undercurl with underline
  fallback.
  *Done when:* a file with real errors shows correct gutter priority against other states.
  *Needs:* T031, T032, T036

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
  *Done when:* the state machine is exhaustively unit-tested, including revision-after-seen.
  *Needs:* T019

- [ ] **T042 · Node anchoring**
  Anchors bind to tree-sitter nodes. Threads, seen-state, and watches survive rewrites.
  *Done when:* a real refactor moves code and the anchors follow. *Needs:* T041

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
  *Done when:* it stays responsive filtering a 100k-file list. *Needs:* T041

- [ ] **T046 · Steel picker sources — unseen, files**
  `(define-picker-source …)`. Files carries unseen counts and activity columns.
  *Done when:* screens `2a` and `3d` reproduce, and a source added from the REPL appears with no
  restart. *Needs:* T045, T022

- [ ] **T047 · Grep / symbols source**
  Tab cycles source. Results carry who-touched-them.
  *Done when:* screen `8a` reproduces. *Needs:* T046

- [ ] **T048 · `:arch` / ArchDiagram**
  A float body over a store query (Q11), **built entirely from the `spans` hatch** (T080) — no
  Rust primitive of its own. It is the proof that the escape hatch is sufficient for a real
  custom surface. Turns invariant 4 from a claim into something you can look at.
  *Done when:* screen `6a` reproduces, reflects the *actual* store rather than a static drawing,
  and adds zero lines to `phosphor-ui`. *Needs:* T041, T080

- [ ] **T049 · Agent nouns resolve**
  `viu` / `sib` / `dih` now bind to real regions (completes T028, per Q8).
  *Done when:* screen `6d`'s nouns are functional, and `viu` selects an unseen region.
  *Needs:* T028, T041

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
  *Done when:* Claude can call an editor tool and the same capability works from Steel and CLI.
  *Needs:* T020, T050

- [ ] **T053 · `phosphor/declare-review-block`**
  The review-block signal as an MCP tool call carrying file+range list and per-group annotations
  (Q6). Routed through the registry, so Steel and CLI can declare one too.
  *Done when:* a declared block becomes a grouped set of unseen markers + a notification.
  *Needs:* T052, T041

- [ ] **T054 · TranscriptPane**
  **A pane, not a float** — splits, holds focus, survives float churn. Turn list, prompt lines
  `❯`, prose, tool rows, seam markers. Folds by turn. Streams during Working.
  *Done when:* screen `1b` reproduces. *Needs:* T050

- [ ] **T055 · Markdown prose behind the gate**
  Via the vendored fork (T004). **Plain-text path must stay readable with the gate off.**
  *Done when:* both paths render acceptably. *Needs:* T004, T054

- [ ] **T056 · OSC 8 tool-row jump links**
  *Done when:* clicking a tool row jumps to the file and range, on the primary terminal.
  *Needs:* T054

- [ ] **T057 · Session lifecycle**
  Cold start (`7d`), attach/adopt/start (`5d`), drop and reattach (`7b`), opening mid-task
  (`2d`). **Editing never blocks on session trouble.**
  *Done when:* all four screens reproduce and the editor stays usable through a mid-turn drop.
  *Needs:* T051

### ✋ CP-6 — Does the session hold?

Half of S6 — shippable on its own: Claude is visible in the editor, but you can't yet direct
from it.

**Run:** attach to a repo with a live Claude Code session; let it work.

**Claude verifies:** session drop mid-turn → reattach → adopt, all recovering · torn-frame check
under sustained streaming load · every `SessionState` variant renders · `1b`, `7d`, `5d`, `7b`,
`2d` snapshots.

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
  *Done when:* screen `1c` reproduces. *Needs:* T050

- [ ] **T059 · QuestionBody**
  Prose + amber digit options `[1]`–`[n]` + full-command footer. Digits answer only while
  focused.
  *Done when:* screen `4a` reproduces. *Needs:* T057

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
  *Done when:* screen `7e` reproduces. *Needs:* T057

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

Three independent workstreams; three checkpoints. Each is independently shippable.

### 7a — Review surfaces

- [ ] **T063 · DiffBody** — vendored diff view restyled; unified and side-by-side; fold rows.
  *Done when:* renders a real diff correctly. *Needs:* CP-0, T041
- [ ] **T064 · Per-hunk seen state** — `s`/`S` compose over any group.
  *Done when:* marking one hunk seen leaves the rest unseen. *Needs:* T063, T041
- [ ] **T065 · Directory grouping + annotations** — `tui-tree-widget`; Claude's group
  annotations ("mechanical" vs "the meat"). **Scale is grouping, not scrolling.**
  *Done when:* screen `8b`'s 40-file block is navigable. *Needs:* T064
- [ ] **T066 · Review block + hunk peek** — screens `4b`, `2b`. *Needs:* T065, T053
- [ ] **T067 · Inbox** — one list of everything Claude said; severity is a single MCP flag;
  unread = unseen. Screen `5c`. *Needs:* T053, T041
- [ ] **T068 · Anchored exchange / threads** — your comment and Claude's reply as virtual text
  under the region. Screen `3a`. *Needs:* T032, T042

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

### 7b — Dirty state

- [ ] **T069 · Changed-on-disk indicator** — `✱` + offer to refresh. **Buffer holds stable.**
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
Then `:dv`, read both versions, take each of the three exits in turn. Is the choice obvious at
the moment you have to make it? This is the invariant most likely to be violated by accident,
and the most damaging when it is.

**Fails if:** the cursor moved, the viewport scrolled, or any exit silently merged.

### 7c — VCS

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
