# Phosphor — agent team plan

Derived from [TASKS.md](TASKS.md) and [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md).
Five teammates, owning crates rather than features, gated by the twelve checkpoints.

**97 of 99 tasks are assigned**, each to exactly one owner. The two unassigned are `T008` and
`T009` — the dependency spikes, already complete ([SPIKES.md](SPIKES.md)). `T005` is the single
deliberate co-ownership and is called out where it appears.

---

## Read this first: the checkpoints are the scheduler, not the dependency graph

The task graph and the checkpoints disagree, and **the checkpoints win.**

Computed from `TASKS.md`, the longest-path wave widths are:

```
wave    0   1   2   3    4    5    6    7   8   9
tasks   2   5   8   6   14   20   19   11   8   3
```

By the graph, wave 4 is 14-wide and includes `T050` (ACP session client, S6) and `T069`
(dirty-state indicator, S7). **A team that schedules off the graph would be building the agent
transport and disk-watching before anyone has confirmed the theme renders correctly** — past
`CP-1`, `CP-2`, `CP-3` and `CP-4`, none of which a graph edge represents, because they are human
judgements about whether the thing is any good.

So: **checkpoints bound the windows; the graph orders work inside a window.** A checkpoint is a
full stop for the whole team, not per-teammate.

Three other numbers worth carrying:

| | |
|---|---|
| `T001` gates **89 of 99** tasks | The workspace skeleton is the whole build's front door. |
| `T019` gates **60** | The `Action` enum. The plan calls it "reversible: no in practice." |
| `T041` has **14 direct dependents** | Store core — the second serialisation point. |

And the shape that matters most for staffing: **waves 0–3 are 2, 5, 8 and 6 tasks wide.** The
early build is close to single-file. Adding people there buys contention, not speed. The team
goes wide at wave 4 and stays wide through wave 6, which is where five teammates earn their
keep.

> **Staffing follows that curve.** Windows A and B run with **three** teammates (`spine`,
> `surface`, `harness`); Window C drops back to **two** as the contract is defined. `store` and
> `agent` join at `CP-2`. This is deliberate under-staffing of the phases that cannot absorb
> parallelism.

> **Why `surface` is live in Window A** (corrected by the docs review): `T003` and `T004` are the
> two vendoring tasks, and the second single-writer rule below is *only `surface` touches
> `vendor/`*. `T083` is `surface` too. Staffing Window A with `spine` + `harness` alone left
> three of its nine tasks with no legal owner.

### Note on the task count per teammate

The skill this plan came from suggests 5–6 tasks per teammate. This plan gives each teammate
~19, because it staffs **all of v1** rather than one wave — teammates are persistent role
owners, not task batches. If you'd rather run a wave at a time, take **wave 4** alone: 14 tasks,
and the first wave wide enough to need everyone — that is the shape the skill has in mind.
(Windows are not waves. Window D spans two checkpoints and carries 21 tasks; wave 4 is a
longest-path layer inside it.)

---

## The ownership rule: crates, because the architecture already enforces them

File ownership is not a convention here — it is the same boundary CI checks. `T007`'s structural
lint means `phosphor-ui` *cannot* import `phosphor_core::store`, and `T078` means the view tree
carries neither a Steel nor a ratatui dependency. **The crate graph is the conflict graph**, so
owning crates gives near-zero merge contention for free.

| Teammate | Model | Owns (exclusive write) |
|---|---|---|
| **spine** | `claude-opus-5` | `phosphor-core/{action,view}.rs` · `phosphor-steel/**` · `phosphor/{main,input,panes}.rs` · `runtime/{init,keymaps,leader}.scm` · **the root manifest** |
| **surface** | `claude-opus-5` | `vendor/**` · `phosphor-buffer/**` · `phosphor-ui/{theme,buffer_view,status_line,gutter,virtual_text,float,help_grid,keymap_footer,tab_bar,soft_wrap}.rs` |
| **store** | `claude-opus-5` | `phosphor-core/{store,region,anchor,seen}.rs` · `phosphor-ui/picker.rs` · `phosphor-vcs/**` · `runtime/pickers/**` |
| **agent** | `claude-sonnet-5` | `phosphor-agent/**` · `phosphor-core/{review,inbox,watch}.rs` · `phosphor-ui/{transcript,prompt_line,question,diff_body,watch_overlay}.rs` · `runtime/{permissions,inbox,watch}.scm` |
| **harness** | `claude-sonnet-5` | `tapes/**` · `.github/**` · `justfile` · `deny.toml` · `rust-toolchain.toml` · snapshot + benchmark infra |

### Two single-writer invariants

These override ownership and are the reason invariant 2 survives contact with a team:

1. **Only `spine` edits the `Action` enum, the query vocabulary, or the view-tree protocol.**
   Everyone else *requests* an addition and waits. A second writer here reproduces exactly the
   drift the tri-door registry exists to prevent.
2. **Only `surface` touches `vendor/`.** Both forks are one person's problem, or `just
   vendor-diff` stops meaning anything.

### Shared boundaries — coordinate, don't assume

- `phosphor-core` has three owners by module (`spine`, `store`, `agent`). The `vm` / `view` /
  `store` module split from `T007` is what keeps that safe; if a change needs to cross those
  modules, it belongs to `spine`.
- `phosphor-ui` is split per widget file. A new widget file needs `spine` to add its view-tree
  node kind first ([Q12](IMPLEMENTATION-PLAN.md#q12)). **This rule decides `T034`/`T035`:** both
  were originally `spine` tasks writing into `phosphor-ui/keymap_footer.rs` and
  `virtual_text.rs`, which are `surface` files. They moved to `surface`; `spine` keeps `T033`,
  the keymaps themselves, which live in `runtime/`. The live keymap reaches the widget as a
  ViewModel like everything else.
- **The pane manager (`T088`) is `spine`, not `surface`.** Panes are focus and event routing in
  the binary's loop, not a widget — `phosphor/panes.rs`. The `TabBar` that renders *over* them
  (`T089`) is `surface`. This is the same split as input: `spine` decides, `surface` draws.
- `runtime/*.scm` is split by directory. Adding a *new* Steel surface is a `spine` decision,
  because it implies a registry entry.
- **Manifests split root-vs-member** (settled at `CP-0`, where the original "the workspace
  manifests" wording first bit). The **root** `Cargo.toml` is `spine`'s alone: it holds the pins,
  `[workspace.dependencies]` and `[workspace.lints]`, and it is the one place a second writer
  would reproduce the drift the pin exists to prevent. A **member** `Cargo.toml` belongs to
  whoever owns the crate — `surface` adding a vendored path dep to `phosphor-buffer` is not a
  contract change, it is that crate's own business. Vendored subtrees are deliberately kept out
  of `[workspace] members`, so a path dep from a member auto-enrols them and needs no root edit
  at all.
  - The one unavoidable crossing is `T001` itself: the workspace cannot compile until every
    member has a `lib.rs`, so `spine` creates the stubs in crates it does not own. They are
    empty by construction and the owner overwrites them on first contact. Expect it once, at
    the very start, and nowhere else.

---

## Roles

### `spine` — the contract · `claude-opus-5`

Owns the two things the whole build is hardest to reverse: the `Action` enum and the view tree.
Also owns the input machine, because it emits Actions and reads Steel-defined keymaps — both
spine surfaces. And the pane manager, which is the same kind of thing one layer out: focus and
event routing in the binary's loop.

**Tasks:** T001, T002, T007, T019–T026, T033, T078–T080, T088, **T090** · **17**

**`T090` is why `spine` is live in Window B.** The window table always listed it there, and the
task breakdown gave it nothing to do — a contradiction nobody noticed until `CP-1` failed for
want of an application to run. The S1 host writes `phosphor/main.rs`, which is spine's file, and
it is deliberately *not* the Window C loop: no `Action`, no Steel, no input machine. Building it
early is what lets four terminals see S1 at all.

**Opus, without hesitation:** `T019` gates 56 tasks; `T079`'s frame cache is what keeps a pre-1.0
scheme VM out of the frame budget; `T026` is a from-scratch vim grammar including the counts and
named registers the dropped crate couldn't express. Every one of these cascades if it's wrong.

**Where it goes wrong:** designing the `Action` enum for S1–S3 only. It must name a mutation for
every surface through S8 — including ones nobody builds for months — or the registry grows a
second shape later.

---

### `surface` — pixels · `claude-opus-5`

Both vendored forks, the buffer engine, and every primitive widget that draws.

**Tasks:** T003, T004, T005*, T010–T018, T027, T029, T031, T032, T034–T040, T081–T087, T089 ·
**31**

The largest list, and it grew by seven in the docs review — five new widget tasks that the design
docs require and the first breakdown had no home for (`T084` Float, `T085` undercurl, `T086`
HelpGrid, `T087` region tints, `T089` TabBar), plus `T034`/`T035` moving here from `spine` because
they write `surface` files. Two of the five are fork work inside `vendor/`, which only this role
may touch.

*(`T005` CI scaffolding is co-owned with `harness` — `surface` needs it in wave 1, `harness`
takes it over at `CP-1`.)*

**Opus:** `T081` (soft-wrap) is unbudgeted work inside a fork, and the plan is explicit that it
touches row↔line mapping, cursor positioning, click targeting and virtual-text placement
simultaneously. That is the definition of high blast radius.

**Where it goes wrong:** implementing soft-wrap as a layer over `VisualRow` instead of as a
variant within it. The four subsystems above all read that one row stream; a wrap that lives
outside it desynchronises every one of them, and the failure shows up as mysterious
off-by-one-row bugs in unrelated surfaces.

---

### `store` — the product · `claude-opus-5`

The semantic store, both anchoring tiers, persistence, the Picker, and VCS.

**Tasks:** T028, T030, T041–T049, T071–T073 · **14**

**Opus:** `T041` has 14 direct dependents, and `T042`/`T043` are the anchoring promise —
*"threads, seen-state and watches survive rewrites"* is the claim `6c` exists to test, and
getting it subtly wrong erodes trust in every marker on screen.

**Where it goes wrong:** treating the line+content fallback (`T043`) as a degraded extra. It is
the floor that makes unseen markers a store feature rather than a language feature — invariant 4
in one task. Build it *first*, before node anchoring, so the fallback is never the afterthought.

---

### `agent` — the session · `claude-sonnet-5`

Both transports, the transcript, the directing surfaces, review blocks, the inbox, and watches.

**Tasks:** T050–T070, T074–T077 · **25**

**Sonnet:** the largest task count, but the most specified work in the build — two mature SDKs
(`agent-client-protocol` 2.0.0, `rmcp` 3.1.2), a decided wire split ([Q6](IMPLEMENTATION-PLAN.md#q6)),
and mockups for all ten screens. Volume, not novelty.

**Where it goes wrong:** implementing the ask queue (`T060`) as widget state. It must be a store
query, or `]!`, the inbox and the statusline drift apart — and that is the whole mechanism
[Q9](IMPLEMENTATION-PLAN.md#q9) chose over letting a question destroy an open picker.

---

### `harness` — proof · `claude-sonnet-5`

The verification tiers, CI, hygiene tooling. **The only stream that is parallel from wave 0.**

**Tasks:** T005*, T006, V001–V009 · **11**

**Sonnet:** mechanical, but not trivial — `V002`'s column calibration exists because VHS sizes in
pixels, and `V006`'s deterministic fixtures are what stop every agent-surface tape from being
flaky.

**Where it goes wrong:** letting Tier 2 gate CI. Pixel comparison across font rendering and VHS
versions is a change *detector*, not a build gate — the exact assertions live in Tier 1. A red
build from a font upgrade will teach the team to ignore the harness.

**Standing instruction:** `harness` never blocks on product work. When a surface isn't ready, it
builds the tape and reference for the surface *after* it.

---

## Windows and gates

Each window ends at a checkpoint. **Nobody starts the next window until Teej passes the
current one** — the manual half of every checkpoint is the point, and a team that runs ahead has
built on an unverified foundation.

| Window | Ends at | Live teammates | Tasks |
|---|---|---|---|
| **A** | `CP-0` (build half) | spine, **surface**, harness | T001–T007, T083, V001 |
| **B** | `CP-1` | spine, surface, harness | T010–T018, **T081**, **T084**, **T085**, **T090**, V002–V005 |
| **C** | `CP-2` | spine, harness | T019–T025, T078–T080 |
| **D** | `CP-3`/`CP-4` | **all five** | T026–T040, T082, **T086**, V006–V009 |
| **E** | `CP-5` | store, surface, harness | T041–T049, **T087** |
| **F** | `CP-6`/`CP-7` | agent, store, surface, spine, harness | T050–T062, **T088**, **T089** |
| **G** | `CP-8a/b/c` | agent, store, surface | T063–T073 |
| **H** | `CP-9` | agent, harness | T074–T077 |

> **Window F reopens `spine` and `surface` briefly.** `T088` (pane manager) and `T089` (`TabBar`)
> both gate `T054`, so they run at the front of F and then those two roles go quiet again. It is
> the one place the "windows narrow as the build goes on" shape doesn't hold, and the reason is
> structural: the transcript is the first surface that forces a second pane into existence.

> **`harness` has no `T`/`V` tasks after Window D**, yet is live in E, F and H. That is
> deliberate — from `CP-5` on, its work is producing each checkpoint's tapes under `V005`'s
> one-tape-per-screen convention, which is standing work rather than a numbered task. The
> standing instruction below is what governs it.

> **`CP-0` is half-passed.** Its go/no-go verdict is settled — both spikes are done and
> [SPIKES.md](SPIKES.md) records them. Its *build* verification (`cargo build` green, both lints
> failing on planted violations, both subtrees building) is Window A's exit gate and is still
> open. `TASKS.md` now says this at the source, so the ✅ there means *the verdict passed*.

**Window C runs with two teammates on purpose.** The contract is being defined; three more
agents would be writing against an interface that changes under them. This is the plan's
single biggest deliberate under-parallelisation, and it is also where the schedule feels worst.
Resist adding people.

---

## Coordination

**Checkpoint protocol.** At each `CP-n`: every teammate stops, reports, and `harness` produces
the VHS artifacts. Teej does the manual half. Nothing resumes until he says so. A failed
checkpoint reopens the tasks named in its "what a failure reopens" line — that is what those
lines are for.

**Contract changes.** A teammate needing a new `Action`, query, or view-tree node opens a request
to `spine` and continues on something else. Never fork the enum locally "temporarily."

**Context discipline.** Every teammate returns a **≤1500-token summary** — what landed, what
broke, what it needs from another teammate. Never raw tool output, never full diffs. The root
agent's context is the scarce resource, and a teammate that dumps its transcript cancels out the
parallelism it was spawned for.

**Reporting format:**

```
DONE      T0xx, T0xx
BLOCKED   T0xx — needs <teammate>: <one line>
CONTRACT  requesting Action::<Name> — <why>
RISK      <anything that would change a decision in the plan>
NEXT      T0xx
```

**Escalate to Teej, don't decide:** anything that would amend a design doc or reverse a numbered
decision. Twelve are recorded in [§5](IMPLEMENTATION-PLAN.md#5-decisions); **four** already amend
the design docs ([Q3](IMPLEMENTATION-PLAN.md#q3), [Q4](IMPLEMENTATION-PLAN.md#q4),
[Q7](IMPLEMENTATION-PLAN.md#q7), [Q9](IMPLEMENTATION-PLAN.md#q9)). The handoff's rule holds for
teammates too — **flag it, don't fold it in.**

---

## Kickoff prompt

```
You are <role> on Phosphor, an agent-native terminal editor.

Read first, in order:
  docs/README.md                 — what this is and the reading order
  docs/IMPLEMENTATION-PLAN.md    — §0 invariants, your phase, §5 decisions
  docs/TASKS.md                  — your tasks, and the checkpoint that ends your window
  docs/SPIKES.md                 — what we learned reading the vendored crates
  docs/TEAM.md                   — your ownership, and the two single-writer rules

You own exactly the files listed for your role in TEAM.md. Do not edit another
teammate's files; request the change instead.

Two rules that override everything:
  1. Only `spine` edits the Action enum, the query vocabulary, or the view tree.
     If you need one, request it and work on something else meanwhile.
  2. Only `surface` touches vendor/.

The five invariants in §0 are the contract. If a change you're about to make
can't be traced to one, stop and ask. In particular: phosphor-ui never mutates
the store, buffers never move unless the user asked, and every surface is a
query over the semantic store.

Work only within your current window (TEAM.md). Stop at the checkpoint and
report — do not start the next window. The manual half of each checkpoint is a
human judgement about whether the thing is any good, and it cannot be skipped.

Report in ≤1500 tokens using the format in TEAM.md. Summaries, not tool output.

Escalate rather than decide: anything that would amend a design doc or reverse a
numbered decision goes to Teej. Flag it; don't fold it in.

Your first task: <T0xx>.
```

---

## What this plan is betting on

**That crate ownership is enough.** It should be — the structural lints in `T006`/`T007` make
the boundaries mechanical rather than social, which is unusual and worth exploiting. If merge
contention shows up anyway, it will be in `phosphor-core` (three owners by module) or
`phosphor-ui` (four owners by file), and the fix is to move the disputed module to `spine`.

**That the checkpoints hold.** They are the only thing preventing five agents from racing ahead
of the verification that makes this a product rather than a pile of surfaces. The graph will
constantly suggest more parallelism than the checkpoints allow, and the graph is wrong.
