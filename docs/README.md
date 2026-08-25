# Phosphor docs

**Phosphor** is an agent-native terminal editor — a spiritual successor to Light Table for the
age of coding agents, with the terminal as a first-class citizen. The design phase is complete.

## Reading order

The handoff specifies this order, and it matters — each doc assumes the one before it.

1. **[design/Design Brief.dc.html](design/Design%20Brief.dc.html)** — thesis, core bets,
   awareness/directing models, v1 scope, every settled decision, v1.5, cut list. *This is the
   contract; nothing in it is open for relitigating without flagging it explicitly.*
2. **[design/Design Language.dc.html](design/Design%20Language.dc.html)** — the visual and
   interaction laws: actor-color palette (green ALWAYS means Claude), glyph lexicon, gutter
   contract, float anatomy, chrome, voice, region lifecycle, degradation, focus rules,
   ratatui component seeds.
3. **[design/TUI Mockups.dc.html](design/TUI%20Mockups.dc.html)** — 37 screens across 9 turns,
   newest at top. Every screen has a stable id (`1a`…`9c`); the implementation plan uses those
   ids as acceptance targets. *The mockups are the spec for what each surface looks like.*
4. **[design/Component Breakdown.dc.html](design/Component%20Breakdown.dc.html)** — crate
   layout, buy/build calls, widget specs with ViewModel inputs, event flow, the philosophy
   section (read it twice), and the 8-step build order.

Then:

- **[IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md)** — the route through the above: phased
  plan mapped to the build order, per-step acceptance criteria tied to mockup ids, the
  vendored-fork strategy, a dependency table verified against crates.io, and a **decision log**
  recording the 12 questions the plan raised, all answered 2026-08-11.
- **[SPIKES.md](SPIKES.md)** — the M-0 findings: both dependency spikes read against the exact
  published sources with `file:line` citations, the full dependency manifest with verified
  versions, and the hygiene tooling. **Read this before T001** — it inverts one recorded
  decision and uncovers unbudgeted work.
- **[TASKS.md](TASKS.md)** — the plan decomposed into 111 tasks across the 9 phases, plus 9
  verification-harness tasks, with **12 checkpoints** where work stops for manual verification.
  `T084`–`T089` were added by a review of these docs: six widget and primitive tasks the design
  requires — the `Float` chrome primitive, undercurl, the `HelpGrid`, region tints, the pane
  manager, `TabBar` — that the first breakdown had no home for. `T090`, the S1 host, was added
  by the first `CP-1` attempt: the widget layer was finished and green, and nothing built an
  application around it, so `cargo run` drew nothing and the checkpoint could not be judged.
  `T092`–`T098` were added by the `CP-3` audit and sit in their own *Arms owed* section: ten
  mutations the vocabulary declares, the doors advertise, and the binary never applies, plus the
  vim keys that are unbound rather than deferred. `T099` and `T100` were added by the repair
  window between `CP-3` and `S4` and sit in a second such section, *B · The repair window*:
  macros over `feed-keys`, and the one task that makes the door speak §6's voice.
- **[TEAM.md](TEAM.md)** — five teammates owning crates rather than features, gated by those
  checkpoints. Includes the wave-width analysis showing why the early phases are deliberately
  under-staffed, and the kickoff prompt. Each checkpoint splits what can be
  proven mechanically from what needs eyes on a real terminal, because most of this design
  language is perceptual and none of that half survives CI.
- **[WINDOW-F-PLAN.md](WINDOW-F-PLAN.md)** — the implementation plan for `T088`, the pane
  manager, which is the front of Window F and the largest refactor in the build. Twelve ordered
  steps, each ending green, with the three rulings that come before any Rust. Produced by a
  design workflow rather than by one reading, and it carries its own provenance note. **A plan,
  not a specification**: `TASKS.md` and `TEAM.md` stay the authority, and where it and the tree
  disagree the tree wins. Delete it when Window F closes — a stale plan is worse than none.
- **[OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)** — what a checkpoint surfaced and nobody has ruled on
  yet, each with the evidence at `file:line`, the options, and a recommendation. It is a register,
  not a backlog: a ruling leaves this file for the decision log, `TASKS.md` or `TEAM.md`, and the
  entry moves to *Closed* with a pointer. Read it before disagreeing with a doc — the disagreement
  may already be recorded.
- **[design/CLAUDE-CODE-HANDOFF.md](design/CLAUDE-CODE-HANDOFF.md)** — the brief that produced
  the plan. Useful as a summary of the five things most likely to be lost in translation.

## Viewing the design docs

The `.dc.html` files are self-contained pages that render in any browser — `doc-page.js` and
`support.js` are their runtime and must stay alongside them:

```
open "docs/design/Design Brief.dc.html"
```

They were imported verbatim from the claude.ai Design project
(`9234741f-228d-4014-9e3c-aea1475f8270`). Filenames match the remote paths exactly so the
project round-trips; edit them there, not here.

## Amendments to the design docs

The design docs are the contract, and they are imported unmodified — so where a decision has
since changed one, the change lives in the plan's decision log, not in the doc. **Sixteen so
far** — four from the decision log, three from `CP-1`, two from `CP-2`, three from `CP-3`, two
from reading the docs against the tree, one from the repair window after `CP-4`, and one from
`CP-4`'s manual half.

The same groups are tabled in [§5](IMPLEMENTATION-PLAN.md#5-decisions); this list was two behind
it until the `CP-3` audit, which is its own small lesson about a list nothing recomputes.

> **And it had gone stale again.** This sentence said *"fourteen"* while **fifteen** bullets stood
> below it — corrected on 2026-08-17 by counting them, at the same time the sixteenth landed. It
> is exactly the defect the lints exist for, one layer out from where they reach:
> `scripts/doc_claims.py` recomputes task, wave, capability and lint counts and has no rule for
> this list. The bullets are the authority; the sentence is a summary of them.

> **Every one of them is still pending upstream.** Audited against the live claude.ai project on
> 2026-08-13 by fetching `Design Brief`, `Design Language` and `Component Breakdown` and reading
> them; `TUI Mockups` is 154 KB and was checked against the local import instead, which is
> byte-identical to the remote on every marker that was fetched. Not one amendment has been
> applied to a `.dc.html`.
>
> **The push half does not exist.** The `DesignSync` tool can read this project but cannot write
> to it: the project's type is `PROJECT_TYPE_PROJECT`, not `PROJECT_TYPE_DESIGN_SYSTEM`, and that
> type is fixed at creation. So the edits are Teej's, by hand, at claude.ai — and the only
> mechanical help available is this audit, which can at least tell you they have not happened.
>
> **The audit changed three entries and added one**, because the list recorded where an amendment
> *originated* rather than every place it lands. That is the same defect as a stale count, and it
> is why each entry below now carries a **Where** line naming every document and section, checked
> rather than assumed.

Four from the decision log:

- **`edtui` is dropped; the input machine is ours.** Its register model cannot express numeric
  counts or named registers, and our keymaps live in Steel, which makes the 185-entry table we
  would be buying dead weight. → [Q3](IMPLEMENTATION-PLAN.md#q3), [SPIKES.md](SPIKES.md)
  **Where:** Component Breakdown in *three* places — the buy table's `edtui` row, the
  *Event & data flow* bullet (*"crossterm event → edtui KeyEventHandler"*), and build-order step
  3 (*"edtui input handler wired to Actions"*) — plus the handoff's settled-decisions list. The
  list recorded only the first until the 2026-08-13 audit.
- **`ratatui-markdown` is a vendored fork, not a plain buy** (Component Breakdown). It pins
  ratatui 0.29 and the workspace is 0.30. → [Q4](IMPLEMENTATION-PLAN.md#q4)
- **Ayu is no longer the second theme mapping.** Its identity colour is orange, which the
  language reserves for attention. Tokyo Night replaces it, and mockup `9b` is superseded.
  → [Q7](IMPLEMENTATION-PLAN.md#q7)
  **Where:** four documents — Design Brief's *"Decided since"*, Design Language §10, the
  Component Breakdown's `Theme` spec, and TUI Mockups (2 occurrences, `9b`). The build side is
  mechanically defended: `crates/phosphor-ui/src/theme/builtin.rs:176`'s `ayu_is_not_shipped`.
- **Needs-you asks queue rather than appear-and-wait.** Both docs say the ask *renders* while
  something else holds focus; queueing means it does not render until no float has focus.
  → [Q9](IMPLEMENTATION-PLAN.md#q9)
  **Where:** Design Language §9 (*"appear, set the statusline flag, and wait"*) and the Component
  Breakdown's `QuestionBody`. Its consequence — the shed ladder's last-standing set becomes
  `✻` / `●n` / **`!`**, not the documented `✻`/`●n` pair — lands in **two more** places the list
  did not name: Design Language §11 (*"The ✻/●n pair is the last thing standing"*) and the
  Component Breakdown's `StatusLine` spec (*"✻/●n last"*). The build already agrees:
  `ask` sits outside `phosphor/status-ladder` in `runtime/statusline.scm:62`, so it never sheds.

**Three more came out of `CP-1`**, the first checkpoint with a running program to disagree with:

- **"Claude owns the brightest colour on screen" is dark-mode only** (Design Language §10).
  Measured against each theme's own ground, claude is top of the actors on dark and **5th of 6 on
  paper** — below meta-grey, and 0.04 from steel-green. The light values are what mockup `8c`
  draws, so it is the contract that did not survive, not the palette. On light, actor identity
  rests on hue, which load-time validation already enforces.
- **Statusline bars join the counter group only** (Design Language §5). The prose reads as though
  every segment joins with a thin bar; §5's own reference render and `1a`, `9c`, `8c` and `8d`
  all draw a plain gap between session state and the counters. The drawings won.
- **The shed ladder is fit-driven, not width-labelled** (TUI Mockups `8d`). `8d` is captioned
  *"80 columns"* and draws the ladder's floor, but at a real 80 columns nothing has dropped yet
  because it all fits. §11's order is exactly what the build does; only the trigger differed.
  `8d` is relabelled as illustrating the end of the ladder.

**Two came out of `CP-2`**, both about mockup `6b`, and the second is the first case of two
drawings contradicting each other rather than contradicting the build:

- **A persisted form goes to the file that loads last** (TUI Mockups `6b`). `6b`'s receipt reads
  `· persisted to init.scm`; the build writes to whatever `phosphor/persist-file` names, because
  `init.scm` runs to its last form *before* Rust reads the load order it declared — so a
  `(keymap-set! …)` appended there comes back on the next boot as a free-identifier fault in a
  float. A one-file layer still gets `init.scm`, which is what `6b` drew and why. *(`T101` kept
  this and made it structural: the file left the load order entirely, so "last" is a call site
  rather than a list position — see the `CP-4` amendment below.)*
- **The λ prompt is steel `#9ec98c`, not claude green `#3ddc97`** (TUI Mockups `6b`). The Design
  Language's glyph lexicon draws `λ ◆` in steel and captions it *"steel prompt · steel surface"*;
  `6b` draws the same glyph in claude green. The lexicon governs — it is the drawing that is
  specifically about this glyph — so `6b` is the bug.

**Two more came out of `CP-3`**, the checkpoint that put a vim user's hands on the build:

- **`s` is vim's substitute; mark-seen is `gs`** (TUI Mockups `6d`, *"`s` composes like an
  operator"*). Ruled 2026-08-12. Vim habits carry and the drawing is what changes: `s` stays
  `(key/fused "change" "char-right")` in normal scope and `(key/operator "change")` in visual,
  and the mark-seen operator moved to `gs`, so `6d`'s sentence is **`gsib`**, not `sib`. Both
  halves are asserted against the shipped layer by
  `shipped_grammar::mark_seen_is_gs_and_s_is_still_substitute`. Recorded in full at
  [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s *Closed* §15.
  **Where:** three documents, not one. TUI Mockups `6d` is where it was found, but Design Brief's
  *"Decided since"* names the nouns *"`viu`, `sib`, `dih`, `:'<,'>c`"*, and **Design Language §6's
  voice rule uses `"s mark seen"` as its worked example of a keyhint** — the sentence that teaches
  the convention is itself now wrong. Found by the 2026-08-13 audit; the list had one.
- **`6b`'s footer promises `q close` on a surface whose body is a text input** (TUI Mockups
  `6b`). `q` types and `esc` closes — Design Language §9 — so the footer is wrong for the frame
  it is drawn on, which is mid-typing at the λ prompt. This was raised at `CP-2` and was not
  decidable then, because the answer depends on *mode*, and modes are `T026`. `T026` landed them
  in Window D, so the build wins and the drawing is amended to `esc close`. The footer reads the
  live keymap already, so "the footer tells the truth about what this key does **in this mode**"
  is a small change rather than a rewrite; it is recorded as owed work on `T034` in
  [TASKS.md](TASKS.md).

**And one more from the repair window after `CP-4`** — the fourth against `6b`, and the first that
a test could not have found, because it is about what the product *should* do rather than about
what it does:

- **A bare `(keymap-set! …)` does not persist; `(persist! …)` does** (TUI Mockups `6b`, fourth
  line: *"`⇒ #ok · persisted to init.scm`"*). Ruled by Teej on 2026-08-14. Persisting by head
  name means trying a theme keeps it forever, and Emacs — which has two mechanisms — has neither:
  `M-:` and `ielm` never persist, `M-x customize` is a deliberate *save this*. It also fails the
  third invariant, *nothing moves unless you asked*. The verb is an identity function in
  `runtime/repl.scm`, so the REPL stays the only writer and a persisted form is idempotent at
  boot; a bare config verb answers `· not persisted — (persist! …) keeps it`,
  which is `6b`'s own receipt offering it. `7a`'s always-allow is untouched — pressing a digit
  was already the explicit act. Built as `T101`; reasoning at
  [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §32.
  **The same task moved the file it writes to.** `phosphor/persist-file` was joined to the
  runtime root, which in a dev checkout is *the repository* — `CP-4`'s manual test left a
  `(define-language! "lua" …)` in a tracked `runtime/persisted.scm`. It is
  `$XDG_CONFIG_HOME/phosphor/` now. That amends no drawing; it is [Q1](IMPLEMENTATION-PLAN.md#q1)
  applied a second time, and to *config* rather than state.

- **`KeymapFooter` is one widget at *three* densities, not two.** Design Language §12 describes it
  as *"verb-labeled hints; also renders the which-key grid — same data, two densities"*, and the
  Component Breakdown says the same while listing `HelpGrid` separately as a `Float` body — so the
  docs model three surfaces as two widgets. The build makes them one node kind, `Node::KeyHints`,
  carrying a `Density` of `Footer` / `Grid` / `Help`
  (`crates/phosphor-core/src/view/props.rs:496`), drawn in one file, `phosphor-ui/key_hints.rs`.
  One kind, one file, one draw site is the same principle `scripts/lint-one-escape-hatch.sh`
  enforces for `Node::Spans`, and it is what `TEAM.md`'s own rule implies: a widget file exists
  because `spine` added a node kind, and `spine` added one.
  **Where:** Design Language §12 and the Component Breakdown's `KeymapFooter / WhichKey` spec.
  Found by the 2026-08-13 audit — it is the design half of a ruling whose `TEAM.md` half was
  already made, and nothing had noticed the docs still disagreed.

**Two more the 2026-08-13 audit found by reading the docs against the tree** rather than waiting
for a checkpoint to trip over them. Both are settled — the build is right and nobody had written
them down:

- **The workspace is eight crates, and `phosphor` no longer owns terminal setup.** The Component
  Breakdown's crate-layout table has seven rows and gives `phosphor` *"the binary: event loop,
  **terminal setup**, config load, session bootstrap"*. `T014`'s terminal lifecycle — raw mode,
  the alternate screen, panic restore, the synchronized-output wrapper and kitty negotiation — is
  neither a widget nor one of the three binary files the ownership table names, so it landed as
  its own crate, **`phosphor-term`**. `members = ["crates/*"]` enrolled it with no root manifest
  edit, so no single-writer rule was crossed.
  The *fact* was already recorded at [TASKS.md](TASKS.md)'s `T001` note — *"it is eight in the
  build"* — but only as an implementation note. Nothing said the design doc disagreed, which is
  why it took an audit to notice. `TEAM.md` assigns the crate to `spine`, by the rule that the
  file decides the task.
  **Where:** Component Breakdown's crate layout (the missing row, and `phosphor`'s own row).

- **"Plain editor complete" is not `7c`.** The Component Breakdown's build order puts *"the
  gutter/virtual-text layer (plain editor complete: **7c**)"* at step 3, but `7c` is captioned
  *"lsp completion + signature help · no agent anywhere · boring on purpose"* — it cannot be
  reproduced without LSP, which is step 4. The plan already resolved this silently: `7c` is
  `S4`'s acceptance target and `CP-4`'s snapshot, while the plain-editor milestone is `CP-3`,
  whose screens are `3c`, `6d` and `8e`. Recording it so the two orderings stop disagreeing.
  **Where:** Component Breakdown's build-order step 3.

**And one from `CP-4`'s manual half, ruled by Teej on 2026-08-17** — the first amendment in this
build that *adds* to a drawing rather than correcting one:

- **A completion row has four columns, not two.** `7c` draws `label` and `detail`; the build
  draws **`kind label detail source`**, and Teej ruled the drawing gains the two rather than the
  build losing them. `T106`'s own entry has the argument: five independent completion UIs
  (`nvim-cmp`+`lspkind`, `corfu`+`kind-icon`, `company-box`, VS Code, Helix) converge on that one
  row grammar, and `kind` was *"the first thing a reader wants and the last thing this build
  carries"* — `lsp-types` supplied it all along and nothing read it.
  Two design laws shaped it and both held: §2's *"one cell, one concept … Nerd-Font-free"* rules
  out the icon every one of the five uses, so **the kind is a word** (`fn`, `cnst`, `meth`); and
  §1's *"each color names exactly one actor or state, never decoration"* rules out
  `kind-icon`'s per-kind hue.
  §11's shed order is **source → detail → kind → then the label elides**, which is why at 80
  columns the source is gone and the label has lost nothing.
  **Where:** TUI Mockups `7c`, whose float body becomes four columns. The three
  `7c-{rust,python,typescript}` Tier-2 captures already draw them — a reference agreeing with the
  build is what a reference *is* — so this amendment is what makes those captures correct rather
  than pending.

If you are reading the design docs cold, read this list first — everything else in them stands.

## The five invariants

Lifted to the top of the implementation plan because everything in the build serves them:

1. **Emacs architecture, literally** — Rust is the C core, the editor layer is Steel in
   `runtime/*.scm`, redefinable at runtime.
2. **One API, three doors** — Steel, MCP, and CLI share one Action/query vocabulary.
3. **Nothing moves unless you asked** — buffers never update under the cursor.
4. **The semantic store is the product** — every surface is a query over it.
5. **No review ceremony** — seen-tracking + Claude-declared review blocks + VCS as the safety
   net.
