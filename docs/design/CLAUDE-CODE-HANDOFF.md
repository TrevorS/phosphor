# Message to Claude Code — Phosphor handoff

You're picking up **Phosphor**: an agent-native terminal editor. The design phase is done; your job is to turn four design documents into an implementation plan. Read them in this order:

1. **Design Brief.dc.html** — thesis, core bets, awareness/directing models, v1 scope, "Decided since" (every settled decision), v1.5, cut list. This is the contract; nothing in it is open for relitigating without flagging it explicitly.
2. **Design Language.dc.html** — the visual/interaction laws: actor-color palette (green ALWAYS means claude), glyph lexicon, gutter contract, float anatomy, chrome (tab bar / statusline / tmux, never wraps), voice, region lifecycle, degradation & scale, focus rules, ratatui component seeds.
3. **TUI Mockups.dc.html** — 9 turns of screens, newest at top. Every screen has a stable id (1a…9c); the Component Breakdown's build order references these ids as acceptance targets. The mockups are the spec for what each surface looks like — build to them.
4. **Component Breakdown.dc.html** — crate layout, buy/build calls, widget specs with ViewModel inputs, event flow, the philosophy section (read it twice), and an 8-step build order.

## The five things most likely to be lost in translation

1. **Emacs architecture, literally.** Rust is the C core (rope, tree-sitter, transports, store, renderer, input decoder). The editor layer — keymaps, picker sources, statusline segments, permission rules, float layouts — ships as Steel in `runtime/*.scm`, redefinable at runtime from the REPL. If you find yourself hardcoding policy in Rust, stop; the placement test is in the philosophy section. Steel lands in build step 2, not later.
2. **One API, three doors.** Steel (in-process), MCP (claude), CLI (`phosphor --eval`) share one Action/query vocabulary over the semantic store. Never add a capability to one door only.
3. **Nothing moves unless you asked.** Buffers never update under the cursor; disk changes are indicated (✱ + offer to refresh), never injected. This is enforced in the BufferView wrapper, not by convention.
4. **The semantic store is the product.** Regions, seen-state, node anchors, threads, watches, inbox, review blocks — every surface is a query over it. Unseen markers must work on ANY file (line-based fallback); node anchoring is a first-class-language enhancement.
5. **No review ceremony.** No approve/reject, no gates. Seen-tracking + claude-declared review blocks + VCS-as-safety-net is the whole model.

## Settled decisions (do not re-open silently)

- Name: phosphor. Leader: SPC. Default theme: phosphor dark/light, base16-style; Catppuccin + Ayu as first mappings; actor hues are validated at theme load.
- One Claude Code session per editor per repo (v1). ACP for the session, MCP for editor tools.
- No VCS required, ever; jj first among adapters.
- Buys: ratatui-code-editor (vendored fork — BufferView core), edtui (input handler), ratatui-textarea, nucleo, tui-tree-widget, throbber-widgets-tui, ratatui-markdown (gated). Skips: rat-salsa, tui-overlay, tachyonfx.
- First-class languages: TS, JS, Rust, Python, Steel, Markdown, JSON, CSV, TOML, YAML, HTML, CSS. Everything else: honest plain-text tier.
- Persistent undo + seen-state on disk. Kitty keyboard protocol, synchronized output (torn frame = P0), OSC 8, undercurl with fallback.

## What to produce

A phased implementation plan mapped to the 8-step build order, with: crate scaffolding first, the Steel embedding + Action enum as the second milestone, per-step acceptance criteria tied to mockup ids (e.g. step 5 done = screens 1a/2a/3d reproducible in a real terminal), and the vendored-fork strategy for ratatui-code-editor spelled out. Flag anything in the docs that's contradictory or underspecified rather than deciding silently — open questions go to the user, not into code.

## Known open items (yours to plan, not to decide)

- ACP/MCP wire details: how claude signals a review block, how watch values stream. Design says "over the session"; pick the mechanism and propose it.
- Seen-state file format and location (survives restarts; per-repo).
- edtui ceiling: if the operator-pending grammar can't host agent nouns (viu, sib, dih), the fallback is a custom input machine behind the same Action layer — budget for the possibility.
- v1.5 (agent-scriptable workspace, tmux control mode) is out of scope but don't paint it out: the one-API invariant is what keeps it cheap later.
