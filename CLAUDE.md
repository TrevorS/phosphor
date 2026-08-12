# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Phosphor is an agent-native terminal editor. Rust core, Steel (embedded Scheme) editor layer,
ratatui 0.30 for drawing.

## Commands

Use the `just` recipes, not raw cargo — several of them differ from the obvious invocation:

- `just test` — `cargo nextest`, not `cargo test` (per-test process isolation; tests touch the XDG
  state dir and terminal state)
- `just lint` — the structural lints below; runs every `scripts/lint-*.sh`. Add a lint by dropping
  a script in, never by editing the justfile or CI.
- `just build` · `just clippy` (warnings denied) · `just deny` · `just vendor-diff` · `just tapes`

**Never run `cargo fmt --all`.** `--all` does not mean "workspace members" — it recurses through the
path dependencies into both vendored forks and fails on upstream code. The only way to green it
would be reformatting the forks, which permanently breaks `just vendor-diff`. Use `just fmt`, which
omits `--all` deliberately.

`just tapes` and `just tape <id>` need the `phosphor` binary on `$PATH`; nothing in the repo puts it
there for you.

## Version control

**Git only. Do not use or re-initialise jj** — the colocated jj repo was deleted deliberately, and
this overrides the global jj-for-local-work rule. There is no remote; never push.

Commits: conventional subject (`build:`, `fix:`, `docs:`) plus a body explaining what changed and
why, including findings, deviations, and anything left open.

## Structural lints — CI fails on these

The architecture is enforced mechanically, not by convention. All three are in `scripts/` and are
proven to bite; a failure is real.

- **No literal colours in `phosphor-ui` outside `theme.rs`.** Every widget takes `&Theme`. Colour
  values come from Design Language §1 — one you invented is a bug even if it compiles.
- **`phosphor-ui` never imports `phosphor_core::store`.** Widgets read `::vm` (ViewModels) and
  `::view` (the view tree). Mutation is the binary's job.
- **No `crossterm::`, `ratatui::` or `editor_crossterm` in `phosphor-ui`.** It takes `ratatui-core`
  only. This is a source lint because Cargo unifies features per crate across the graph, so the
  manifest cannot express it.

## The vendored forks

`vendor/ratatui-code-editor` and `vendor/ratatui-markdown` are `git subtree` forks, deliberately
excluded from `[workspace] members` so our lints stop at the seam.

**Every hunk under `vendor/` needs a matching entry in that fork's `VENDOR.md`** — that is the
acceptance contract, audited by `just vendor-diff`. An undocumented hunk is how a fork silently
becomes a rewrite. Keep patches minimal and put phosphor additions in the fork's own `phosphor/`
module. Pin by SHA, not tag: upstream published `ratatui-code-editor` 0.0.6 without ever tagging it.

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
