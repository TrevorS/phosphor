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
  recording the 11 questions the plan raised, all answered 2026-08-11.
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
since changed one, the change lives in the plan's decision log, not in the doc. Two so far:

- **Ayu is no longer the second theme mapping** (Design Brief, "Decided since"). Its identity
  colour is orange, which the language reserves for attention. Tokyo Night replaces it, and
  mockup `9b` is superseded. → [Q7](IMPLEMENTATION-PLAN.md#q7)
- **`ratatui-markdown` is a vendored fork, not a plain buy** (Component Breakdown). It pins
  ratatui 0.29 and the workspace is 0.30. → [Q4](IMPLEMENTATION-PLAN.md#q4)

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
