# Phosphor — agent team plan

Derived from [TASKS.md](TASKS.md) and [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md).
Five teammates, owning crates rather than features, gated by the twelve checkpoints.

**107 of 117 tasks are assigned**, each to exactly one owner. `T005` is the single deliberate
co-ownership and is called out where it appears. The ten with no owner are `T008` and `T009` — the
dependency spikes, already complete ([SPIKES.md](SPIKES.md)) — and `T101`–`T108`, which no role
list below names.

> **This line read *"110 of 112"* and *"the two unassigned are `T008` and `T009`"*, and both halves
> were wrong.** Summed this session, the five **Tasks:** lines below carry 29 + 29 + 14 + 25 + 11
> entries and share `T005`, which is **107 distinct** and never was 110; and `T101`, `T102` and
> `T103` were appended by the repair window between `CP-4` and Window E without ever being added
> to a role. `scripts/doc_claims.py` checks the **denominator** against the graph in `TASKS.md`
> and nothing recomputes the numerator or the per-role totals, so the sentence went quietly wrong
> in exactly the shape [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)'s §30 describes. Corrected rather
> than papered over, and the eight are left unassigned rather than assigned here — see the note
> under the ownership table for what the file rule implies about them.

---

## Read this first: the checkpoints are the scheduler, not the dependency graph

The task graph and the checkpoints disagree, and **the checkpoints win.**

Computed from `TASKS.md`, the longest-path wave widths are:

```
wave    0   1   2   3    4    5    6    7   8   9  10
tasks   2   5   8   6   15   24   22   15   14   3   1
```

By the graph, wave 4 is 15-wide and includes `T050` (ACP session client, S6) and `T069`
(dirty-state indicator, S7). **A team that schedules off the graph would be building the agent
transport and disk-watching before anyone has confirmed the theme renders correctly** — past
`CP-1`, `CP-2`, `CP-3` and `CP-4`, none of which a graph edge represents, because they are human
judgements about whether the thing is any good.

So: **checkpoints bound the windows; the graph orders work inside a window.** A checkpoint is a
full stop for the whole team, not per-teammate.

Three other numbers worth carrying:

| | |
|---|---|
| `T001` gates **107 of 117** tasks | The workspace skeleton is the whole build's front door. |
| `T019` gates **77** | The `Action` enum. The plan calls it "reversible: no in practice." |
| `T041` has **14 direct dependents** | Store core — the second serialisation point. |

And the shape that matters most for staffing: **waves 0–3 are 2, 5, 8 and 6 tasks wide.** The
early build is close to single-file. Adding people there buys contention, not speed. The team
goes wide at wave 4 and stays wide through wave 6, which is where five teammates earn their
keep.

**Wave 10 has one task in it.** `T098` — honest refusals for the deferred vim keys — sits alone
at the end of the longest path, because it needs both the keymap (`T033`) and the unknown-key
hint (`T035`) before it can say anything sensible. A lone task at the far end of the graph is
what unwired work looks like from here, and it is worth noticing rather than smoothing over.

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
~21, because it staffs **all of v1** rather than one wave — teammates are persistent role
owners, not task batches. If you'd rather run a wave at a time, take **wave 4** alone: 15 tasks,
and the first wave wide enough to need everyone — that is the shape the skill has in mind.
(Windows are not waves. Window D spans two checkpoints and carries 22 tasks; wave 4 is a
longest-path layer inside it.)

---

## The ownership rule: crates, because the architecture already enforces them

File ownership is not a convention here — it is the same boundary CI checks. `T007`'s structural
lint means `phosphor-ui` *cannot* import `phosphor_core::store`, and `T078` means the view tree
carries neither a Steel nor a ratatui dependency. **The crate graph is the conflict graph**, so
owning crates gives near-zero merge contention for free.

| Teammate | Model | Owns (exclusive write) |
|---|---|---|
| **spine** | `claude-opus-5` | `phosphor-core/{action,view}.rs` · `phosphor-steel/**` · `phosphor/{main,input,panes}.rs` · `phosphor-term/**` · `phosphor-ui/{interpret,frame}.rs` · `runtime/{init,keymaps,leader}.scm` · **the root manifest** |
| **surface** | `claude-opus-5` | `vendor/**` · `phosphor-buffer/**` · `phosphor-ui/{theme,buffer_view,status_line,gutter,virtual_text,float,key_hints,unknown_key,tab_bar,soft_wrap}.rs` |
| **store** | `claude-opus-5` | `phosphor-core/{store,region,anchor,seen}.rs` · `phosphor-ui/picker.rs` · `phosphor-vcs/**` · `runtime/pickers/**` |
| **agent** | `claude-sonnet-5` | `phosphor-agent/**` · `phosphor-core/{review,inbox,watch}.rs` · `phosphor-ui/{transcript,prompt_line,question,diff_body,watch_overlay}.rs` · `runtime/{permissions,inbox,watch}.scm` |
| **harness** | `claude-sonnet-5` | `tapes/**` · `.github/**` · `justfile` · `deny.toml` · `rust-toolchain.toml` · snapshot + benchmark infra |

Two things the build added that this table predates:

- **`phosphor-term` is `spine`'s, and `T014` moved with it.** The eighth crate — raw mode, alt
  screen, panic restore, kitty-protocol negotiation, and the synchronized-output wrapper — is
  neither a widget nor one of the three binary files this table used to name, so it landed
  unowned. Settled on five facts rather than on who happened to write it:
  1. **Its only production consumer is `crates/phosphor`**, which is `spine`'s. `phosphor-buffer`
     takes it as a *dev-dependency* for one runnable example, not in its dependency line.
  2. **It is where `crossterm` and `ratatui` live** — the two things
     `scripts/lint-no-app-layer-in-ui.sh` forbids in `phosphor-ui`. `surface`'s crate is defined
     by not being allowed to contain this code.
  3. **Kitty-protocol negotiation is input**, and the input machine (`T026`) is `spine`'s.
  4. **Panic and exit restore are process lifecycle**, which belongs to whoever owns the binary.
  5. **It draws nothing.** No widget, no `Widget` impl. Every other entry on `surface`'s list
     either draws or is a fork.

  This is the `T034`/`T035` precedent run in reverse. Those were `spine` tasks that moved to
  `surface` because they wrote `surface`'s files; the rule is that **the file decides the task,
  not the other way round**. `T014` writes app-layer files, so `T014` is `spine`'s. Window B
  already lists `spine` as live, so no window changes.
- **`scripts/lint-*.sh` is deliberately unowned.** The glob *is* the contract: `just lint` runs
  every script matching it, so any role adds a structural lint by dropping a file in, without
  touching `harness`'s justfile or CI. It started at three — `T006`'s (harness), `T007`'s (spine)
  and the app-layer lint added after `CP-1`, which closes a hole Cargo's feature unification
  opened and no manifest can express — and **the glob is now `ls scripts/lint-*.sh`, deliberately
  not a number here.** Every window since has added one or more, each because the thing it catches
  had already happened; a count in this sentence would be the third stale count this build has
  had to correct, and unlike the task counts nothing recomputes it. `CLAUDE.md` describes what
  each one is for.

**`T101`–`T108` are unassigned, and here is what the file rule implies about them** — a note for
whoever schedules the next window, **not an assignment made here**, because the two repair windows
and `CP-4`'s manual half appended tasks and no role list was updated. By *"the file decides the
task"*: `T101` (config home, `phosphor-core/src/config.rs` + `main.rs` + `runtime/`) and `T103`
(the CLI verb route, `phosphor/src/{door,main}.rs`) are `spine`'s; `T102` (the undo crash) is
`surface`'s by the second single-writer invariant, since it writes `vendor/`. Of `CP-4`'s five:
`T105` (`phosphor-core/src/input/table.rs`, `runtime/keymaps.scm`, `main.rs`) and `T107`
(`main.rs`) are `spine`'s; `T106` (`phosphor-buffer/**`, `phosphor-ui/float.rs`) is `surface`'s,
with a contract request to `spine` for the `request::Completion` fields it needs. **Two do not
resolve, and that is the point of writing this down rather than filling in a table.** `T104`
crosses the boundary in a way no precedent covers — its renderer half is `vendor/` and therefore
`surface`'s by invariant 2, while its keymap and input-machine halves are `spine`'s — so it is
either split into two tasks or given a co-ownership like `T005`'s, and that is Teej's call.
`T108` has no files at all until its design session runs.

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
  were originally `spine` tasks writing into `phosphor-ui/key_hints.rs` and
  `virtual_text.rs`, which are `surface` files. They moved to `surface`; `spine` keeps `T033`,
  the keymaps themselves, which live in `runtime/`. The live keymap reaches the widget as a
  ViewModel like everything else.
  - **`key_hints.rs` is one file, and the rule above is why.** The table used to name
    `help_grid.rs` and `keymap_footer.rs`; neither ever existed. `spine` added **one** node kind
    — `Node::KeyHints` at `crates/phosphor-core/src/view.rs:500`, carrying a `Density`
    (`crates/phosphor-core/src/view/props.rs:496`) — drawn at three densities: the float footer,
    the `SPC` leader grid, and the `:help` body. One kind, one file, one draw site, which is the
    same principle `scripts/lint-one-escape-hatch.sh` enforces for `Node::Spans`. So `T034` and
    `T086` are one widget at two densities, not two widgets.
  - **`interpret.rs` and `frame.rs` are `spine`'s, and they are the exception the rule needs.**
    They are the only two files in `phosphor-ui` that draw no node kind: both are `T079` —
    *tree interpreter + frame cache* — which this table's task list already assigns to `spine`,
    and `interpret.rs` is where a `Node` kind *becomes* pixels rather than a widget that paints
    one. That makes it the view-tree protocol's other half, and single-writer rule 1 says only
    `spine` edits the view-tree protocol. Ruled 2026-08-13 on the same rule that moved `T014`
    and `T027` — **the file decides the task** — and stated positively so the boundary reads in
    one direction: **`surface` owns every file in `phosphor-ui` that draws one node kind, and
    `spine` owns the two that draw none.** It is not a hypothetical seam: every widget task
    touches `interpret.rs` to add its arm, and `scripts/lint-one-escape-hatch.sh` already
    treats its single `Node::Spans` draw site as load-bearing. A widget task that needs an arm
    there requests it, the way it already requests the node kind.
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

**And the app layer under all of it** — `phosphor-term` (`T014`) and the S1 host (`T090`). This
role is the only one allowed to touch a terminal: raw mode, the alt screen, panic restore,
kitty-protocol negotiation, and the synchronized-output wrapper every frame goes through. The
rule that keeps it honest is mechanical — `scripts/lint-no-app-layer-in-ui.sh` fails CI on a
`crossterm::` or `ratatui::` reference from `phosphor-ui`, so "spine decides when pixels land,
surface decides what they look like" is a boundary the build enforces rather than a convention.

**Tasks:** T001, T002, T007, **T014**, T019–T026, **T027**, T033, T078–T080, T088, **T090**,
**T091**, **T092–T098**, **T099**, **T100** · **29**

**`T090` is why `spine` is live in Window B.** The window table always listed it there, and the
task breakdown gave it nothing to do — a contradiction nobody noticed until `CP-1` failed for
want of an application to run. The S1 host writes `phosphor/main.rs`, which is spine's file, and
it is deliberately *not* the Window C loop: no `Action`, no Steel, no input machine. Building it
early is what lets four terminals see S1 at all.

**`T027` is `spine`'s, and `T092`–`T098` are the arms it owes.** The kitty-keyboard task moved
here at the `CP-3` audit by the same rule that moved `T014`: the negotiation already lives in
`phosphor-term` (`KeyboardProtocol::Kitty`, `crates/phosphor-term/src/lib.rs:124`), which is a
`spine` crate, and the table's own line for `surface` — *"`surface` draws, and never touches a
terminal"* — cannot hold with `T027` on that list. The seven tasks `T092`–`T098` arrived the same
way: six are declared mutations whose missing arm is in `crates/phosphor/src/main.rs` and one is
a set of bindings missing from `runtime/keymaps.scm` — both `spine` files, and the file decides
the task. They are the mechanical proof of the wiring rule below: verbs that shipped declared,
tested at the widget, and unreachable from a keystroke.

**Opus, without hesitation:** `T019` gates 56 tasks; `T079`'s frame cache is what keeps a pre-1.0
scheme VM out of the frame budget; `T026` is a from-scratch vim grammar including the counts and
named registers the dropped crate couldn't express. Every one of these cascades if it's wrong.

**Where it goes wrong:** designing the `Action` enum for S1–S3 only. It must name a mutation for
every surface through S8 — including ones nobody builds for months — or the registry grows a
second shape later.

---

### `surface` — pixels · `claude-opus-5`

Both vendored forks, the buffer engine, and every primitive widget that draws.

**Tasks:** T003, T004, T005*, T010–T013, T015–T018, T029, T031, T032, T034–T040,
T081–T087, T089 · **29**

The largest list. It grew by seven in the docs review — five new widget tasks that the design
docs require and the first breakdown had no home for (`T084` Float, `T085` undercurl, `T086`
HelpGrid, `T087` region tints, `T089` TabBar), plus `T034`/`T035` moving here from `spine` because
they write `surface` files — and shrank by one at the `CP-3` audit, when `T027` went to `spine`.
Two of the five are fork work inside `vendor/`, which only this role may touch.

**`T014` went the other way, after `CP-1`, and `T027` followed it at `CP-3`.** Terminal setup
landed in a crate of its own, `phosphor-term`, and that crate is `crossterm`, the alt screen and
panic restore — the app layer, whose only production consumer is the binary. It is `spine`'s now,
by the same rule that brought `T034`/`T035` here: the file decides the task. `T027`, the kitty
keyboard protocol, was on this list and is the same crate — the negotiation is
`KeyboardProtocol::Kitty` in `phosphor-term`, and the arm that consumes it is
`machine.set_protocol(…)` in the binary. The line is worth stating positively, because it is what
this role *is* — **`surface` draws, and never touches a terminal**, and a task on this list that
touches one is a mis-filed task, not an exception.

**Two files in `phosphor-ui` are not this role's**, settled 2026-08-13 by the same rule:
`interpret.rs` and `frame.rs` are `T079`'s and therefore `spine`'s. The test is whether a file
draws one node kind — every file on the list above does, and those two do not. See the
`phosphor-ui` bullet under *Shared boundaries*.

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
| **D** | `CP-3`/`CP-4` | spine, surface, store, harness | T026–T040, T082, **T086**, **T097**, V006–V009 |
| **E** | `CP-5` | store, surface, harness | T041–T049, **T087** |
| **F** | `CP-6`/`CP-7` | agent, store, surface, spine, harness | T050–T062, **T088**, **T089** |
| **G** | `CP-8a/b/c` | agent, store, surface | T063–T073 |
| **H** | `CP-9` | agent, harness | T074–T077 |

> **Window D's `S4` half ran with `harness` absent, and the row above is the plan rather than the
> record.** No `7c` tape and no diagnostics tape exist in `tapes/` (listed 2026-08-14), so `CP-4`'s
> *"VHS produces"* half is unproduced. This is not a blocked task — `V006`, `V008` and `V009` are
> each unticked for their own reasons, recorded at each of them, and none of them is *"capture this
> window's screens"*. Producing a checkpoint's tapes under `V005`'s convention is **standing work
> rather than a numbered task**, which the note further down already says of Windows E onward; what
> `S4` shows is that it is true one window earlier. **A standing instruction is the one kind of work
> no agent's prompt names**, and this run's prompts named files. That is rule 2 one layer over: work
> nobody is assigned is work nobody does, and the gate stays green while it does not happen.
>
> **Window D runs with four, not five.** The table said *all five* until the `CP-3` audit
> checked it against the task lists: `agent` owns `T050`–`T070` and `T074`–`T077`, and **not one
> of them falls in Window D**. The live roles are `spine`, `surface`, `store` and `harness`.
> `agent` joins at `CP-2` in the staffing narrative and has nothing to build until `S6`, which is
> Window F — the same shape as `harness` being live in E, F and H with no numbered task, noted
> below.

> **`T092`–`T096` and `T098` are unscheduled by window on purpose.** They are the arms owed for verbs
> already declared and already shipped in the vocabulary — see `TASKS.md`'s *Arms owed* section
> and the RECORDED table in `scripts/lint-action-arms.sh`. Each belongs to the window that next
> touches its surface (`T092` theme, `T093` floats, `T094` the Steel layer, `T095` history,
> `T096` soft wrap, `T098` the deferred vim keys), and putting them in a window now would be
> inventing a schedule rather than recording one. `T097` is the exception: `T086` cannot pass without it, so it sits in Window D
> with `T086`.

> **`T099` and `T100` are the repair window's, and they are scheduled differently from each
> other.** `T100` — the door's voice — is `spine`'s and belongs at the **front of Window E**, in a
> phase where nothing else is rewriting the parity expectations, because that is the whole cost of
> it. `T099` — macros over `feed-keys` — belongs to whichever window next opens
> `runtime/keymaps.scm` and the input machine together, and putting it in one now would be
> inventing a schedule rather than recording one, the same call `T092`–`T096` got. What stops
> `T099` going quiet is not the RECORDED table but the capability rows themselves:
> `set-macro-recording`, `register` and `place-anchor` each cite an unticked task, so
> `scripts/lint-action-arms.sh` fails the moment one is ticked with no arm behind it. `T100` has
> no such guard — it is the door's voice, not an arm — which is the argument for scheduling it
> rather than leaving it to a window that next happens to touch `door.rs`.

> **Window F reopens `spine` and `surface` briefly.** `T088` (pane manager) and `T089` (`TabBar`)
> both gate `T054`, so they run at the front of F and then those two roles go quiet again. It is
> the one place the "windows narrow as the build goes on" shape doesn't hold, and the reason is
> structural: the transcript is the first surface that forces a second pane into existence.

> **`harness` has no `T`/`V` tasks after Window D**, yet is live in E, F and H. That is
> deliberate — from `CP-5` on, its work is producing each checkpoint's tapes under `V005`'s
> one-tape-per-screen convention, which is standing work rather than a numbered task. The
> standing instruction below is what governs it.

> **Where the build actually is.** `CP-0` and `CP-1` have both passed, both halves each.
> Windows A and B are complete: the workspace, both vendored forks, three structural lints proven
> to bite, the grammar ABI check, the whole S1 widget layer, the S1 host, and a calibrated tape
> harness. **Window C is built** — `spine` and `harness` — and `CP-2`'s mechanical half is
> green: 215 capabilities, three doors derived from one table, 645 door checks walked end to
> end, Steel booted from `runtime/`, the REPL live, and the statusline composed in the editor
> layer. **Window D's S3 half is built too**, across two concurrent runs and a repair pass, and
> **`CP-3` has passed, both halves** — the mechanical half green at 639 tests and 14 lints, and
> Teej's manual half on **2026-08-13** with **no findings**. The verdict is written at the
> checkpoint in [TASKS.md](TASKS.md), which is the rule below being obeyed rather than restated.
> A second repair window ran between the two, on debt this build had already written down.
>
> **Window D's `S4` half is built too, and `CP-4` is outstanding.** `T036`, `T037`, `T038` and
> `T039` are ticked; `T040` and `T082` are deliberately not, each for a reason written at the task
> — `T040`'s criterion says *"against other states"* and there is one source of gutter regions
> until `T041`, and `T082`'s `align-columns` has no honest arm and, unusually, **no creditor to be
> re-homed to**. The mechanical half is green at **983 tests and 17 lints**. `CP-4`'s manual half
> has not run and no verdict is recorded anywhere — the mechanical half is written *at the
> checkpoint* in [TASKS.md](TASKS.md), item by item, on the rule below.
>
> **This is the first window run with rule 2 in force, and it is the reason to keep it.** A wiring
> agent ran last, after every builder, and twelve of the `Lsp` domain's fourteen capabilities are
> named by the binary — every one of them declared and dead to the keyboard when the builders
> finished. Running `CP-4`'s checklist against the *running binary* then found a second class of
> defect no builder's gate could have reached, each now pressed by a test in
> `crates/phosphor/tests/loop_pty.rs`: `gd` discarded unsaved work
> (`a_jump_out_of_a_dirty_buffer_refuses_rather_than_discarding_it`), a failed server was silent
> on every surface (`a_server_that_cannot_start_says_so_on_the_statusline`), a language declared
> at the `:repl` only took effect after a restart
> (`a_language_declared_at_the_repl_is_live_in_the_same_session`), a burst of typing painted a
> denial on the statusline (`a_burst_of_typing_never_says_the_editor_denied_something`), and two
> of the twelve languages shipped with a server that could not `initialize`. **Every one is a
> composition defect**, invisible to a green crate and to every widget test in the repository.
>
> That bookkeeping gap is closed. `TASKS.md` now carries `CP-2 · **PASSED**` and dates the verdict
> to 2026-08-12, where it belonged all along: the manual half was run and answered in conversation,
> and never written down, so this file and `TASKS.md` disagreed with the build for a whole window.
> The rule the gap produced is worth more than the entry it fixed — **a checkpoint verdict is
> written where the checkpoint is, or it did not happen.**
>
> `CP-1`'s manual half produced four rulings, three of which amend design docs. That is the
> checkpoint doing its job: they are the first amendments in this build that came from looking at
> a running program rather than reasoning on paper. `CP-3`'s audit produced the next two, plus
> the concurrency rules below — which came from watching sixteen agents finish green and leave
> four surfaces dead to the keyboard.
>
> Window B also cost more than the table below says. `T090`, the S1 host, did not exist when
> this plan was written: the first `CP-1` attempt failed because a complete, tested widget layer
> had no application around it, so `cargo run` drew nothing and there was nothing to look at on
> any terminal. It is `spine`'s, and it is why `spine` is listed live in Window B.

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

### Concurrency — several agents, one worktree

The role table above assumes one agent per role. Windows C and D were run differently: a role was
split across many agents working **concurrently in a single worktree**, each given a named set of
files. That is faster and it is how the rest of this build will run, so the rules it needs belong
here rather than in a prompt somebody rewrites each time. **All five come from something that
already went wrong.**

1. **One writer per file-group per phase, and you `CONTRACT` rather than reach.** An agent's
   prompt names the files it owns; creating, editing or deleting anything outside that set is not
   a judgement call, it is a report — *`CONTRACT` requesting `<thing>` — `<why>`* — and the agent
   moves on to something else. This is the crate-ownership rule from above at a finer grain, and
   it is the only thing making concurrent writes to one tree safe.

2. **The wiring agent goes last, always.** Window D's S3 run gave `crates/phosphor/src/main.rs`
   to one agent in **phase 2**, so that concurrent agents could never collide in the host. The
   run was safe and it starved the integration point: every surface built in phases 3 and 4
   landed complete, tested, ticked and **uncomposed**, because by then nobody could write the
   file that composes it. Sixteen agents finished, `just gate` was green, and pressing `SPC` did
   nothing — the leader popup, the unknown-key hint, folds and undo were all built and all dead
   to the keyboard. **The last phase of every window belongs to a wiring agent** whose entire job
   is that nothing shipped this window is unreachable from a keystroke. `scripts/lint-action-arms.sh`
   is the mechanical half of the same rule; this is the scheduling half.

3. **`just fmt-fix` is workspace-wide.** It rewrites files the agent running it does not own,
   mid-edit — observed: a `T029` agent reformatted `main.rs` while `spine` was writing it.
   Nothing broke, because formatting is what CI checks and `just fmt` is that check — but the
   file lock is unenforceable as written. **In a concurrent window, run `just fmt` (check) and
   fix only your own files by hand.** Never `cargo fmt --all`, in any window: it recurses through
   the path dependencies into both vendored forks, and a hook blocks it.

4. **Only the final gate counts.** A shared crate is often uncompilable mid-window, because
   another agent is halfway through it. So a per-agent *"gate: green"* is a claim about a moment
   and about that agent's own files, not about the tree. Report **what is red and whose it is**;
   the gate that means something is the one run after every agent has landed. The corollary:
   never read an exit code through a pipe — `just lint | tail` gives you `tail`'s status, and
   this build has twice reported a lint green that was red exactly that way.

5. **`file:line` citations drift, so cite symbols.** Two agents editing neighbouring files in the
   same window move each other's line numbers, and a report written at minute five is wrong by
   minute forty. A gate that audited agent reports killed eleven claims across two windows,
   several of them line numbers that had simply slid. **Name the symbol** — `fn under`,
   `struct Timeline`, `driven::pressing_space_opens_the_leader_popup` — and add the line number
   as a convenience, not as the identifier.

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
