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
- **[TASKS.md](TASKS.md)** — the plan decomposed into 90 tasks across the 9 phases, plus 9
  verification-harness tasks, with **12 checkpoints** where work stops for manual verification.
  `T084`–`T089` were added by a review of these docs: six widget and primitive tasks the design
  requires — the `Float` chrome primitive, undercurl, the `HelpGrid`, region tints, the pane
  manager, `TabBar` — that the first breakdown had no home for. `T090`, the S1 host, was added
  by the first `CP-1` attempt: the widget layer was finished and green, and nothing built an
  application around it, so `cargo run` drew nothing and the checkpoint could not be judged.
- **[TEAM.md](TEAM.md)** — five teammates owning crates rather than features, gated by those
  checkpoints. Includes the wave-width analysis showing why the early phases are deliberately
  under-staffed, and the kickoff prompt. Each checkpoint splits what can be
  proven mechanically from what needs eyes on a real terminal, because most of this design
  language is perceptual and none of that half survives CI.
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
since changed one, the change lives in the plan's decision log, not in the doc. **Four so far:**

- **`edtui` is dropped; the input machine is ours** (Component Breakdown, *"buy (input)"*; and
  the handoff's settled-decisions list). Its register model cannot express numeric counts or
  named registers, and our keymaps live in Steel, which makes the 185-entry table we would be
  buying dead weight. → [Q3](IMPLEMENTATION-PLAN.md#q3), [SPIKES.md](SPIKES.md)
- **`ratatui-markdown` is a vendored fork, not a plain buy** (Component Breakdown). It pins
  ratatui 0.29 and the workspace is 0.30. → [Q4](IMPLEMENTATION-PLAN.md#q4)
- **Ayu is no longer the second theme mapping** — named in *three* docs: Design Brief "Decided
  since", Design Language §10, and the Component Breakdown's `Theme` spec. Its identity colour is
  orange, which the language reserves for attention. Tokyo Night replaces it, and mockup `9b` is
  superseded. → [Q7](IMPLEMENTATION-PLAN.md#q7)
- **Needs-you asks queue rather than appear-and-wait** (Design Language §9, and the Component
  Breakdown's `QuestionBody`). Both say the ask *renders* while something else holds focus;
  queueing means it does not render until no float has focus. Consequence in §5/§11: the
  statusline shed order's last-standing set becomes `✻` / `●n` / **`!`**, not the documented
  `✻`/`●n` pair. → [Q9](IMPLEMENTATION-PLAN.md#q9)

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
