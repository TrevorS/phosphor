<!-- Produced by the `window-f-front` design workflow on 2026-08-20: four mapping
agents over the tree, one empirical transport de-risk, three independent designs,
nine judges, one synthesis. Ranked 23/21/20 of 30. This is a *plan*, not a
specification — docs/TASKS.md and docs/TEAM.md remain the authority, and where
this document and the tree disagree the tree wins.

CITATION STYLE, learned the hard way on 2026-08-20: cite OTHER PROSE DOCUMENTS by
heading or by a quoted phrase, never by line number. Two line citations in this
file into TEAM.md and TASKS.md went stale within a day — one of them because
executing this plan's own step 1 inserted 82 lines above its target, leaving the
pointer aimed at prose that step had just written. `file:line` is right for CODE,
where `lint-doc-claims.sh` checks the task references and a moved line is usually
a moved fact; for prose it is a pointer with nothing holding it. A quoted phrase
is greppable, survives every edit above it, and tells the reader what they are
being sent to.

Partially applied: the three citations that had ALREADY rotted are converted, and
nine others into TEAM.md / TASKS.md / OPEN-QUESTIONS.md are not. They are correct
today and were left rather than rewritten in bulk, because inventing nine quoted
phrases in one pass is a good way to introduce a wrong one — convert each as its
section is next touched. The durable answer is a lint under scripts/lint-*.sh
that rejects `<doc>.md:<line>` inside docs/, which is this repo's own mechanism
for a rule it wants kept rather than remembered. Not written yet. -->

# Window F front — the plan for `T088`

Synthesised from three designs and nine judgements. **Spine: "state-first — ids before geometry, geometry before pixels"** (23/30). Grafted onto it: the collapse step and the `Geometry` value from "Tree-First" (21/30), promoted to run *early*, because that is what closes the DEBT judge's fatal finding. Every file:line below was read in this session; where I am relying on another agent's run rather than my own read, I say so.

---

## 0. The three rulings that come before any Rust

These are decisions the tree cannot make. They are written into `docs/TASKS.md` first because ruling (a) determines where `Editor` lives, and that is the one call in this plan that cannot be walked back cheaply.

**(a) One `Editor` per `BufferId`. Each pane owns its own viewport.**

> **RULED BY TEEJ, 2026-08-20 — follow nvim. This reverses the second half of what the plan
> proposed below, and the paragraph is kept because its reasoning is why the reversal needs a
> mechanism rather than just a decision.** The plan proposed panes *sharing* one viewport on the
> grounds that a per-pane viewport is a vocabulary change belonging to `spine`. It is not, and the
> plan missed the third path: the host owns each pane's viewport and hands it down through the
> door `Resources` already is — `viewport(&self, pane: PaneId)` beside `picker(&SourceId)` and
> `completion()`, plus a `BufferView::viewport(…)` builder shaped exactly like `.fill(…)`.
> `Node::Pane` already carries the `PaneId`. No fork patch, no `&mut` on `Resources`, and
> `Node::Buffer` still carries no viewport — a door is not a prop. Full ruling at `T088`'s entry
> in `docs/TASKS.md`; §6's questions **1 and 2** are answered there and are one question, not
> two. *(This read "1 and 3" until 2026-08-20: the ruling was written against the three
> decisions as they were put to Teej, not against §6's own numbering, and the two lists
> disagree. §6 is the one a reader of this file will count.)*
>
> **Steps 4a, 6 and 11 change shape**: `Pane` gains `viewport` (and, in its own commit, `cursor`);
> the `ViewAction::Scroll` arm resolves its `PaneRef` to a pane's viewport rather than to the
> shared editor's; and step 11's *"two panes on one buffer means two `wrap_to` widths on one
> `Editor`, the last one wins"* stops being a ruling-(a) consequence and becomes a real per-pane
> value. The cursor is the larger half and should not be assumed to ride along in the same commit.

The original reasoning, superseded:
 The tree contradicts itself and the contradiction is real: `ViewAction::Scroll { request, pane: PaneRef }` (`crates/phosphor-core/src/action.rs:428-431`) says a viewport is per-pane, while `Node::Buffer`'s own declaration says *"It carries no viewport"* (`crates/phosphor-core/src/view.rs:439-440`) and the interpreter resolves it through `Resources::editor(*buffer)` (`crates/phosphor-ui/src/interpret.rs:481-486`). Both alternatives are closed: one `Editor` per pane means two ropes over one file, and applying a per-pane viewport during the walk is impossible because `Resources` *"has no `&mut` in it and must never grow one"* (`crates/phosphor/src/main.rs:4025-4027`). A genuine per-pane viewport is a **vocabulary change** — a prop on `Node::Buffer` or `Node::Pane` — and belongs to `spine`, not to `T088`. Record the ruling at the field.

**(b) `collapsed: BTreeSet<RegionId>` is per buffer, and ruling (a) forces it.** `virtual_text::install(&mut editing.editor, &rows)` at `main.rs:2644` installs the row list into the *editor*. With one `Editor` per `BufferId`, a per-pane `collapsed` is not expressible without a fork patch. This is a ruling, not a preference — the two designs disagreed and the tree settles it.

> **RECORDED at `T088`'s entry in `docs/TASKS.md`, 2026-08-20, and the citation checked.** The call is where the plan says it is, and the read the plan did not name is the filter above it — `Some(owner) if editing.collapsed.contains(&owner) => None` at `main.rs:2636`. `Editing::collapse`'s own doc (`main.rs:6327`) is why it cannot move into the fork: the fork's toggle is one flag for the whole editor.

**(c) The buffer-swap reset list is written down.** See step 5.

> **RECORDED at `T088`'s entry, 2026-08-20 — the plan is right, and the list is two fields, not one.** The swap block is `main.rs:3061-3121` (the plan cites `3062-3117`; the `else` arm opens at `:3061` and its `Ok` body is `:3063-3115`). It rewrites exactly what the plan claims, and resets neither `selection_from` (`:5035`) nor `selection_kind` (`:5029`). **`selection_kind` is the second field** and step 5 must reset it to `SelectionKind::Char` as well: it drives `Editing::selected`'s linewise widening and the yank's `linewise` flag, and is as much a fact about the departed rope as the anchor is.

~~Left open for Teej, deliberately: the T046 picker-into-a-split question (`T088`'s entry asks for the answer by name), whether the same file may open in two panes, and whether `Editing` gets renamed. Listed in §5 below.~~ **Two of the three are ruled and the pointer was wrong — corrected 2026-08-20.** The picker question and same-file-in-two-panes are answered at `T088`'s entry (Teej: follow nvim and telescope), leaving `Editing`'s rename and `SetRegister`'s domain. The list is **§6**, not §5; §5 is the doc-versus-tree corrections.

---

## 1. The ordered steps

Every step ends with `just gate` green. Root is `/Users/trevor/Projects/phosphor/.claude/worktrees/window-f/`.

### Step 1 — Rulings and document corrections, docs only

**Files:** `docs/TASKS.md`, `docs/TEAM.md`, `docs/OPEN-QUESTIONS.md`

Write rulings (a), (b), (c) into T088's entry with their citations. Answer the T046 question `T088`'s own entry asks for by name. File the off-screen-buffer question — `TEAM.md`'s *"`T060`'s second blocker may survive `T088`"* bullet — as an OPEN-QUESTIONS entry with the recommendation in §3 below, not as an answer. Apply the document corrections in **§5** — there are **four**, and §4 is the transport verdict. *(Both numbers were wrong in the first draft of this line; corrected 2026-08-20 while executing it.)*

**Verification:** `just lint` — `lint-doc-claims.sh` checks every `T0xx` cited in prose is a task that exists. 21 lint scripts under `scripts/lint-*.sh` (counted).

### Step 2 — `Geometry`: compute the frame layout once

**Files:** `crates/phosphor/src/main.rs`

The layout is computed twice today and the loop's own comment admits it: `main.rs:2503-2505` says *"`draw` re-splits `frame.area()` itself, so this is only for what needs `&mut editor`"*, then calls `split(Rect::new(0,0,size.width,size.height))` at `:2506-2508`, while `draw` calls `split(area)` again at `:4126`. Six consumers read one or the other: `editing.area = editor_area(body)` (`:2509`, via `phosphor_ui::buffer_view::editor_area`, `buffer_view.rs:354`), `soft_wrap::wrap_to` (`:2515`), the picker's `list_rows` (`:2797`), the statusline ground fill, the cursor placement (`:4268`, `:4272`), and `mouse_actions`'s hit test (`:7900`).

Introduce `struct Geometry { body, hint, leader, status }` and `fn lay_out(...)` built out of the existing `split` (`:3658`) and `take_rows` (`:3681`) verbatim. Compute once at the top of the loop; pass it to `draw`, which stops splitting. **This step changes no pixel.** It exists so steps 3 and 11 have somewhere to put N pane rects.

**Verification:** `just gate`; `just tapes-diff` clean on every tape (then `git checkout -- tapes`, since capturing overwrites the tracked PNGs). `crates/phosphor/tests/screen_1a.rs`, `screen_3c.rs`, `screen_8e.rs` unchanged and green.

### Step 3 — THE COLLAPSE, while there is still one pane

**Files:** `crates/phosphor/src/main.rs`, `crates/phosphor-ui/src/interpret.rs`, `scripts/lint-node-kinds.sh`

This is the graft, and it is promoted from last to third. The target composition already exists and already has a golden frame: `crates/phosphor/tests/screen_1a.rs:227-241` builds `Tree::new(Node::split(Axis::Rows, [Slot(Fill{1}, Node::Buffer { buffer: BufferId(1), soft_wrap: false }), Slot(Cells{1}, status)]))` and asserts the pixels. `crates/phosphor-core/src/view.rs:730-738` shows the same shape wrapped in `Node::Pane { pane, holds, focused, child }`. The pixels are proven; only the wiring is missing.

`draw` (`main.rs:4117-4133`) has two paths: the tree path returns at `:4139-4140`, the widget path renders `BufferView` straight into `body` at `:4174-4179`. Compose the frame tree in the loop, wrap the buffer in `Node::Pane` now (the interpreter's Pane arm is one line — `Node::Pane { child, .. } => self.node(child.node(), area, buf)`, `interpret.rs:366`), and delete the widget branch.

Three obstacles, each found by reading, each with a fix that does not touch the view protocol:

- **§8's degraded fill.** `draw` picks `Fill::Block`/`Fill::Marker` from `phosphor_term::colour_available()` at `main.rs:4169-4173` and passes it at `:4174-4179`. The interpreter's `Node::Buffer` arm (`interpret.rs:474-486`) never calls `.fill(..)`, so it takes `BufferView`'s `Fill::Block` default (`buffer_view.rs:504`). Collapsing naively silently un-degrades `V009`'s `NO_COLOR` capture. **Fix: a `fill` builder on `Interpreter`, not a prop on `Node::Buffer`** — the tree carries no terminal capability and adding a prop is `spine`'s call.
- **The cursor.** The tree path returns at `:4140`, before the cursor block at `:4268-4273`. With `Geometry` from step 2, set the cursor after the single render.
- **The float's area.** Floats render into `body` today (`main.rs:4197`, `:4199`). Render the frame tree over `area`, then the float over `geometry.body`. §9's dim is about panes; dimming the statusline is a change nobody asked for.

**Delete the `Pane` and `Buffer` rows from `scripts/lint-node-kinds.sh:119-131` in this same commit.** That lint fails when a recorded gap is now composed — it goes red the instant the composition lands, which is the forcing function working.

**Verification:** `just tapes-diff` must be clean on `1a`, `1a-seeded`, `3c`, `8e`, `2a`, `6a` and `6b` — the whole screen is drawn by a different mechanism and must come out identical, and the last two are the *tree-composed* surfaces, which take `draw`'s early-return path and are the ones a collapse is most likely to break. Re-capture `1a-degraded-nocolor` specifically for the `fill` fix.

> **Two tape ids in this line were wrong — corrected 2026-08-20 by the agent executing step 2, which ran the suite and found them.** There is **no `6d` tape**: `6d` is a Tier-1 golden frame (`crates/phosphor/tests/snapshots/screen_6d__6d.snap`) and has never had a capture, which is how it reached a Tier-2 list. And the degraded tape is `1a-degraded-nocolor`, not `…-no-colour` — `just tape 1a-degraded-no-colour` is a no-such-id. Both would have been discovered as a failing verification step rather than as a wrong id, which is the more expensive way to learn it. `just review` on the golden frames. `scripts/lint-node-kinds.sh` via `just lint`.

### Step 4 — Ids and two maps, one entry each (3 commits: 4a, 4b, 4c)

**Files:** `crates/phosphor/src/main.rs`

The load-bearing step, split three ways because as a single commit it is unbisectable.

- **4a — `Pane`. DONE, and narrower than this bullet asked for.** `struct Pane { area: Rect, alternate, jumplist, jump_at }`, and the four moved off `Editing` exactly as argued: `alternate`'s own doc already said *"the file leaving the pane becomes the alternate"*, and the jumplist holds `AnchorId`s that each carry a path, so neither was ever per-buffer. Vim agrees about the second one in as many words — `:help jumplist` is *"Each window has a separate jump list"*.

  **`holds: PaneKind` and `buffer: Option<BufferId>` are not in it, and that is not a shortcut.** Nothing reads either until step 4c keys `Panes` and `Buffers` on those ids, and a field with no reader is one `dead_code` rejects under `-D warnings`. The same bar is what keeps ruling (a)'s `viewport` out until step 11 gives it `Resources::viewport` to be read through — which is worth noticing, because it means the ruling's *"steps 4a, 6 and 11 change shape"* is really steps 6 and 11: there is nothing for 4a to add that anything can yet look at. The rule this build already runs on — a ticked task may not ship something no keystroke can reach — turns out to apply one layer down, to a struct field, and the compiler enforces it for free.

  **The line citations in the first draft of this bullet were stale on arrival** — `area` was at `main.rs:5061`, not `:4820`; steps 2 and 3 grew the file by roughly 240 lines between the plan being written and being executed. Code citations are still the right form (two lints check the references and a moved line usually means a moved fact), and this is a reminder that *usually* is doing work in that sentence.

- **The 4a/4b boundary moved, and the reason is the same one.** This plan put `Cx` in 4b, with 4a moving the fields — but `self.area` is read by `Editing::text`, `anchor` and `wrapped`, which are `&self` helpers called from inside `act`'s arms, so the fields cannot move without a vehicle to carry the pane in. 4a therefore threads a plain `pane: &mut Pane` through the twelve methods that need one. **`Cx` lands in 4b**, when it has two fields and beats a parameter; a one-field `Cx { view: &mut Pane }` is a wrapper with nothing in it, and this build does not ship those either. Converting the parameter to a `Cx` at 4b is one mechanical pass the compiler drives end to end.

  Counted rather than estimated: 17 `editing.area` sites, 12 signatures, 9 production call sites of `act`/`apply` and 25 in tests — the plan's 34, confirmed by making them fail to compile.
- **4b — `Shell` and `Cx`. DONE.** `Shell { store, wake }` and `Cx<'a> { view: &'a mut Pane, shell: &'a mut Shell }`, threaded through `Editing::act`, `Editing::apply` and the sixteen helpers they reach. Both of `Shell`'s fields were on `Editing`, where each new buffer would have taken its own clone of a handle to the same object — not wrong, since both are shared handles, but a clone per buffer is a thing a constructor can forget to make, and step 8 builds `Editing`s from a place that has no business knowing either exists.

  **`pane: PaneId` and `tree: &PaneTree` are not in it, for step 4a's reason.** Nothing resolves a `PaneRef` until step 6 and nothing walks a tree until 4c, so both are fields `dead_code` would reject. `Cx::new` is a named constructor rather than a struct literal at each call precisely so 4c can widen it in one place.

  **`Editing::text` needed an explicit lifetime**, which is the one thing here that was not mechanical: it returns an `EditorText<'_>` borrowing the editor *and* the store, and once the store moved to the shell those are two different owners. `fn text<'a>(&'a self, cx: &'a Cx<'_>) -> EditorText<'a>` ties them to the shorter.

  **34 call sites: 9 production and 25 in the test module** — the plan's count, and it held. The 25 cost one edit rather than 25, because step 4a landed `Bench` for exactly this.
- **4c — the maps.** `Buffers { map: BTreeMap<BufferId, Editing>, next }` and `Panes { tree: PaneTree, map: BTreeMap<PaneId, Pane>, focus, next }`, both with one entry. `PaneTree` is split from the `BTreeMap<PaneId, Pane>` deliberately so `&PaneTree` and `&mut Pane` borrow at once. **Index by id from the first line, never by position** — `BufferId` and `PaneId` are already declared (`crates/phosphor-core/src/request.rs:52-55`, and `PaneId`'s doc already reads *"A pane in the split tree (`T088`)"*), and `close-pane` invalidates positions the first time it runs.

Behaviour is provably identical because there is still one of everything.

**Verification:** `just gate` green after each of 4a/4b/4c. `Buffers`/`Panes` are plain structs, so unit tests can construct two entries while the binary makes one — that is what makes steps 6-9 testable before any UI exists.

**4a's verification, as run.** `just gate` green, 1,391 tests. The 1,389 that existed all passed unchanged, which is the whole of the claim that behaviour is identical while there is one of everything. Two were added, because a refactor that only moves fields is a rename and should have to prove it did more:

- `two_panes_over_one_buffer_keep_two_jumplists` — the same `Editing`, two `Pane`s, and a jump pushed through one does not appear in the other. This is the test that could not be written before 4a and fails the moment the list goes back on the buffer.
- `the_same_prose_wraps_to_more_lines_in_a_narrower_pane` — `wrapped` measured a hover float against `self.area`, so which width it got depended on which pane had most recently been laid out. It measures the pane it is handed now, and the test hands it two. The numbers are not asserted, only that narrow yields more lines than wide, which is only true if the width came from the pane.

**A test harness landed with it, for step 4b's benefit.** `Bench { editing, pane }` in the test module, deref'ing to `Editing`, so twenty-five tests say `editing.apply(&action)` and do not care what the context is made of. Without it, 4b and 4c each rewrite those twenty-five call sites; with it they change two methods. Test-only scaffolding, said so at the type.

**4b's verification, as run.** `just gate` green, 1,392 tests — the 1,391 before it unchanged, plus one:

- `two_buffers_place_their_anchors_in_one_store` — two buffers, two files, one anchor each, and the count is asserted on the *session's* store. This is what `Shell` makes structural: an arm cannot reach anything except the session's, so a constructor cannot hand a buffer the wrong one.

The harness paid for itself exactly as predicted. `Bench` gained a third field and its two methods build the `Cx`; the twenty-five call sites did not move. The one wrinkle was `Bench::text`, which forwarded to `Editing::text` and so demanded the whole context — two `&mut` borrows for a read, which turned `editing.act(.. editing.text() ..)` into a double borrow. It builds the `EditorText` from its own three fields now and takes `&self`, which is what a read should have asked for.

### Step 5 — The buffer-swap reset list, and a real bug fix

**Files:** `crates/phosphor/src/main.rs`

The swap block (`main.rs:3062-3117`) rewrites `editor`, `timeline`, `depth`, `alternate`, `file`, `signature`, calls `close_completion()` and re-runs `adopt`. It does **not** touch `selection_from` (`:5035`) or `selection_kind` (`:5029`). `ExtendSelection` reads `*self.selection_from.get_or_insert(head)` at `:5358` with no containment guard, unlike `SelectRange` which guards at `:5330-5336`. `selection_from` is cleared only at `:5370` (`ClearSelection`) and `:6038` (undo). **So the swap leaves a stale char offset pointing into a different rope** — the same defect class `CP-4`'s review already found once (`main.rs:9985-9994` documents that finding).

Extract the block into `fn open_into(...)` with an explicit, named reset. **Do not construct a fresh `Editing` here.** That is the form to reach for only after step 8; see §2.

**Verification:** existing file-swap tests in `crates/phosphor/tests/loop_pty.rs`. One new unit test that selects a range in file A, opens file B, and asserts `editing.selection_from.is_none()` — the assertion shape the existing test at `main.rs:8867-8871` already uses. Asserting on `get_selection()` instead would pass on `master` today, because the swap replaces `editing.editor` wholesale at `:3066-3067`.

### Step 6 — Honour the six discarded selectors; stop synthesising `Focused`

**Files:** `crates/phosphor/src/main.rs`

Six arms drop a pane or buffer selector with `..`, each confirmed: `MotionAction::SetCursor { position, .. }` (`:5298`), `ViewAction::Scroll { request, .. }` (`:5386`), `FileAction::SaveBuffer { path, .. }` (`:5453`), `FileAction::OpenFile { path, at, .. }` (`:5470`), and `LspAction::IngestCompletions` / `IngestSignatureHelp` / `IngestHover` (`:5640`, `:5684`, `:5701`). Each `..` becomes a real read through `cx.tree.resolve` or a `Buffers` lookup.

Separately: `Editing::reveal` (`:5224-5235`) hardcodes `PaneRef::Focused {}` at `:5231` and calls `self.act`. It must name `cx.pane` — the moment the `Scroll` arm honours its selector, an unfocused pane's reveal scrolls the focused one. No existing test can see this, because today both halves ignore the ref and are self-consistent by accident.

`mouse_actions` (`:7894-7900`) reads `let area = editing.area;` to hit-test, which inverts the resolution order — the pane must come *from* the click point. Signature becomes `(machine, &Panes, &Buffers, mouse) -> (PaneId, Vec<Action>)`, and its wheel arm (`:7927`) emits `PaneRef::Id` for the pane under the pointer.

**Verification:** unit tests over hand-built two-entry `Buffers`/`Panes` — `SetCursor { buffer: Some(b) }` moves b's cursor and not a's; a reveal from the unfocused pane does not move the focused viewport; a click inside pane B's rect resolves to B. `loop_pty.rs` unchanged.

### Step 7 — The LSP quartet into the buffer record

**Files:** `crates/phosphor/src/main.rs`

`dirty` is already an `Editing` field (`:5042`); the loop holds an `Rc` clone from `dirty_flag` at `:2335`. `synced` and `sent` are loop locals (`:2499-2500`) and the didChange gate is `if edits.get() != sent` at `:2543-2549`. **One `Rc<Cell<u64>>` against one `sent` cannot express "buffer A changed, buffer B did not."** All four become per-`Editing`, and the loop's sync block becomes `for (_, buf) in buffers.iter_mut()` — that is the correction that matters, because `servers.change` must run per buffer regardless of focus or a server answers stale for every file not on screen. Keep `dirty` an `Rc<Cell<bool>>`; un-`Rc`ing it means touching the fork's `set_change_callback` and is a separate fight.

**Verification:** existing didChange tests in `loop_pty.rs`; new unit test — edit buffer A, assert B's `edits` counter did not move and B's `sent` still equals it.

### Step 8 — Session fields into `Shell`; `:wall`, `:q`, `:close-buffer` become questions about `Buffers`

**Files:** `crates/phosphor/src/main.rs`, `crates/phosphor/tests/loop_pty.rs`

Move `store` (`:4959`), `picker` (`:4985`), `registers` (`:5027`), `wake` (`:4973`), `source_order` (`:5002`), `mode` (`:5060`), `quit` (`:5044`), `falling_through` (`:4953`) — 34 sites inside `impl Editing` — onto `Shell`.

`registers` is a **decision, not a reading**: vim's registers are global, so `yy` in one split and `p` in the other must work. `mode` stays session because there is one `Machine` (a loop local) and `InputAction::SetMode`'s arm (`:7666`) is its only writer.

Then three refusals become real, and each becomes a question about `Buffers` rather than about one record — as `Buffers`-level free functions, not `Editing` methods, because an `Editing` cannot see its neighbours and should not learn how:
- `FileAction::SaveAll` (`:5462`, today `self.write(None)` with the comment at `:5461` *"there is exactly one, and `T088` is what makes there be more"*) → write every dirty buffer.
- `AppAction::Quit` (`:5525`) → refuse on *any* dirty buffer.
- `FileAction::CloseBuffer` (`:5505-5511`) → stop declining with *"one buffer, one pane — :quit leaves; T088 gives a buffer somewhere to close to"*.

**Verification:** `loop_pty.rs:2930-2955` (`wall_writes_without_leaving`, whose doc says *"One buffer until `T088`"*) gets a second buffer and a real assertion. `loop_pty.rs:2985` drops its `("close-buffer", "T088")` row from the `deferred` table — that table asserts each refusal *names its task*, so leaving the row fails loudly.

### Step 9 — Completion and signature per buffer; `Outstanding` keyed

**Files:** `crates/phosphor/src/main.rs`

`completion` (`:4903`), `offered` (`:4912`), `chosen` (`:4942`), `signature` (`:4956`) stay on `Editing` — and that is the decision rather than the default. The reason is in the guard: the Ingest arms compare `at: Position` against the live cursor, and a cursor is a fact about a document.

`Outstanding` (`main.rs:3722-3726`) counts in-flight requests with **no key at all** — three bare `u32`s. With N buffers an answer for B is tested against A's cursor and either dropped or drawn against A. It becomes `BTreeMap<BufferId, Outstanding>`, and `lookup` (`:4894`) / `question` (`:4899`) in the mailbox carry the `BufferId` that asked. This is why the mailbox hoist is step 10 and not step 1: hoisted before `BufferId` exists, these two get keyed twice.

**Verification:** existing completion/signature tests in `main.rs`'s test module. New unit test — ingest an answer tagged for buffer B while pane A is focused; assert A's `completion` is still `None` and B's is populated.

### Step 10 — `Asks`, and the pane verbs the loop performs

**Files:** `crates/phosphor/src/main.rs`

Hoist the eleven drain-once fields plus `refused` into `Shell::asks`, each now carrying the `PaneId`/`BufferId` that asked. The pattern is already established and documented at the field: *"A file `open-file` asked for. **The loop performs it**: opening one needs the theme and the language table, and neither is this struct's"* (`main.rs:4823-4825`).

Then the pane verbs land as `Asks` variants the loop drains — **not as arms on `Editing`**, because they mutate the tree an `Editing` was borrowed out of. All five capabilities exist in the vocabulary with policies already set (`crates/phosphor-core/src/action.rs:630-653`): `SplitPane` Allow, `FocusPane` **Deny**, `ClosePane` Allow, `ResizePane` Allow (all four declared `[S6 / "T088"]`), and `SetPaneContent` Allow (declared `[S6 / "T054"]`). **There are zero `Action::Pane` arms in `main.rs` today** — the domain falls through to `Outcome::Refused(Refusal::NotYetImplemented { task: action.spec().since.task })` at `main.rs:5939-5941`, which is why every pane verb already refuses by naming T088 for free. `lint-action-arms.sh` demands the four T088-declared arms the instant T088 is ticked.

`PaneTree` gains `split`, `close`, `resize`, `focus`, `resolve(&PaneRef, focus)`, and a focus-return stack so *"opening then closing a float returns focus exactly where it was"* is state rather than luck. `Query::Ui::Panes` (`crates/phosphor-core/src/query.rs:410`, declared `[S6 / "T088"]`) is answered off the same tree.

**Verification:** `PaneTree` is a pure data structure — no terminal, no `Editor`, no theme. Split/focus/close/resize/resolve unit-tested directly. **This is where `TASKS.md:2718`'s acceptance criterion is proven, before a pixel exists.**

### Step 11 — N panes: layout and the prep pass split in two

**Files:** `crates/phosphor/src/main.rs`

`PaneTree::layout(body: Rect) -> Vec<(PaneId, Rect)>`, written into each `Pane::area`, off step 2's `Geometry`. The prep block (`main.rs:2506-2645`) does eight things against one editor and splits by what each is actually about:

- **Per pane** (geometry-dependent): `area = editor_area(body)` (`:2509`), `soft_wrap::wrap_to` (`:2515`), `set_tab_width` (`:2525`), `soft_wrap::set_mode` (`:2535`).
- **Per buffer** (document-dependent): `indent_style` (`:2524`), `servers.change` (`:2546-2548`), `gutter::spans` (`:2595`), `tints.sync` (`:2604`), `virtual_text::install` + `set_styled_spans` (`:2644-2645`).

Two panes on one buffer means two `wrap_to` widths on one `Editor`; the last one wins. That is ruling (a) showing up in a second place — **record it where it happens.** Everything needing `&mut` stays before the draw, because `Resources` has no `&mut` and must never grow one (`:4025-4027`); the picker's matcher tick at `:2797-2801` is the existing precedent and its comment states the rule.

`Painted` (`main.rs:4020-4029`) stops holding `editor: &'a Editor` and holds the buffer map plus a per-buffer marks map; `Resources::editor` (`:4044-4051`) and `state_marks` (`:4053-4055`) become real lookups and their two *"One buffer, and it is implicit"* docs (`:4045`, `:4066`) are rewritten.

**Verification:** unit tests on `PaneTree::layout` for one, two and three panes including odd widths. Golden frames and tapes unchanged at one pane.

### Step 12 — Retire the refusals and the prose that names them, then tick

**Files:** `crates/phosphor/src/main.rs`, `crates/phosphor/tests/loop_pty.rs`, `crates/phosphor-ui/tests/golden_frames.rs`, `scripts/lint-action-arms.sh`, `docs/TASKS.md`, `docs/TEAM.md`

Ten `T088` sites in `main.rs`, grepped this session: `:591` (`CloseAllFloats` — *"at most one has focus, not at most one exists"*), `:3640` (`session_buffer` — *"One pane, so this replaces what was on screen"*), `:4045` and `:4066` (`Resources`' two implicit-singleton docs), `:5461` (`SaveAll`), `:5503`/`:5510` (`CloseBuffer`), `:6525`/`:6534` (`accept_picker` — `AcceptHow::Split => return declined("one pane until T088 splits it")`), `:8011` (`repl_key` — *"until the REPL is a pane"*). Three in `loop_pty.rs` (`:2930`, `:2968`, `:2985`), one in `golden_frames.rs:277` (*"two renders rather than two panes because panes are `T088`"*).

`accept_picker`'s `AcceptHow::Split` arm becomes step 1's T046 answer as code. `lint-action-arms.sh:114-123`'s `ApplyWorkspaceEdit` entry keeps its T060 attribution, but its parenthetical — *"(It also edits files that are not open, which is `T088`; the ask is the nearer of the two.)"* — is rewritten to cite step 1's ruling, so T060 inherits a citation rather than a reading.

**Verification:** `just gate` — all six, in CI's order, one invocation. Then CP-6's manual half.

---

## 2. How this plan avoids the three fatal flaws the judges found

**CORRECTNESS judge (9/10) — "the plan promises pixels and never lands them; and `interpret.rs:624` defers six kinds, not one."**
Verified: `crates/phosphor-ui/src/interpret.rs:624-629` defers `TabBar | Diff | Question | Transcript | Prompt | Watch` — six. `docs/TEAM.md:482` already says six and is correct; the winning design's *"only `Node::TabBar` is still deferred"* was wrong, and it was the load-bearing premise of its "rendering is mechanical" argument. **This plan does not rest on that argument.** Step 3 is a full composition step with the `fill`, cursor and float-area obstacles named, and steps 3 and 11 together change `Painted`, `draw`'s signature, and both `Resources` methods. The `ViewAction::Scroll` citation is corrected to `action.rs:428-431`.

**BLAST-RADIUS judge (8/10) — "step 2 constructs a fresh `Editing` before the session fields have left it; that silently wipes `registers`, `jumplist`, `store`, `wake`, `picker`, `source_order`, `mode`, `quit`, `falling_through` and the nine mailbox fields on every `:e <path>`, behind a green gate."**
Confirmed by reading the field list: those fields are at `main.rs:4959`, `:4973`, `:4985`, `:5002`, `:5013`, `:5025`, `:5027`, `:5044`, `:5053`, `:5060` and the mailbox block `:4825-4899`. Today's in-place mutation preserves all of them; a fresh construction would not, and no test covers a register or a jumplist surviving a swap. **Avoided two ways.** The bug fix is decoupled from the construction: step 5 ships an *explicit named reset*, not a constructor; the by-construction form is only reachable after step 8, when the session fields are gone. And step 4 is split 4a/4b/4c because as one commit it touches four new types, two signatures, 9 production and 25 test call sites, and is unbisectable.

**DEBT judge (6/10) — "no step composes `Node::Pane` or `Node::Buffer` or deletes the `BufferView` draw at `main.rs:4174-4179`, so following the plan literally produces a tree that cannot tick `T088` without failing `just lint`; and the `Node::Buffer` arm has no `.fill()`, which silently regresses §8's `NO_COLOR` degradation."**
Both confirmed against the tree. **This is why the collapse is promoted to step 3**, executed while there is still one pane so `just tapes-diff` is a true byte-level proof, with the `fill` builder on `Interpreter` as its own named obstacle. The `Pane` and `Buffer` rows leave `scripts/lint-node-kinds.sh:119-131` in that same commit.

---

## 3. What is NOT in T088

- **`TabBar` — T089.** `interpret.rs:624` defers the tag; `lint-node-kinds.sh:124-126` records it against T089, whose own criterion (*"appears on the second pane and never on the first"*, `TASKS.md:2725-2726`) is a different acceptance. T088 must not tick that row.
- **`Node::Gutter` — nobody.** Its RECORDED entry has an **empty blocker** and says so: *"it has no creditor… No task in the graph names such a surface, so nothing closes this entry"* (`lint-node-kinds.sh:132-141`). A tree-composed buffer never composes a `Node::Gutter` because `BufferView` draws its own state column — `interpret.rs:484-485` calls `.state_column(self.interp.resources.state_marks(*buffer))`. **`TEAM.md:469-471` is wrong about this.** See §4.
- **`Node::Prompt`'s demolition — T058.** `OPEN-QUESTIONS.md:2523-2532` rules the ex line as *"scaffolding with a demolition date"* at T058, and `lint-node-kinds.sh:157-162` records it there. T088 makes the demolition possible; it does not perform it.
- **The `SetPaneContent` arm — T054's.** Declared `[S6 / "T054"]` at `action.rs:649-653`, so ticking T088 does not demand it. T088 lands the `PaneTree` operation underneath it.
- **`CreatePaneFromView` — v1.5** (`action.rs:655`).
- **Patching the vendored fork to split `Editor` into document and view.** Still out, and the reasoning survives ruling (a)'s reversal intact: it is permanent `VENDOR.md` debt against a fork pinned by SHA. What changed is that per-pane viewports no longer need it — the `Resources::viewport(pane)` door and the `BufferView::viewport(…)` builder get there without touching the fork.
- **A `&mut` on `Resources`, or a "current pane" on `Painted`.** `main.rs:4025-4027` states the rule.
- **A second `Machine`.** One machine, one mode, focus decides who receives. If T054's transcript wants its own modality it should argue it on T054's evidence.

### The off-screen-buffer question — T060's second blocker

`TEAM.md`'s *"`T060`'s second blocker may survive `T088`"* bullet flags this as a gap with no creditor, and `TASKS.md`'s `T060` entry says the RECORDED attribution to `T088` is *"a reading rather than a citation"*. Both are correct, and this is the one item Window F must not leave to discovery at T060.

> **FILED, 2026-08-20, as `docs/OPEN-QUESTIONS.md` §47 — a question with a recommendation, not an answer.** The recommendation below is taken as option 3 of three. `scripts/lint-action-arms.sh`'s `ApplyWorkspaceEdit` row cites §47 now instead of naming `T088`, and `T060`'s entry and `TEAM.md`'s bullet both point at it. `T088` ruling the policy itself would be a second task answering on `T060`'s behalf, which is the exact thing the RECORDED entry complains about.

**Recommendation: build the capacity, refuse the policy.**

The container comes free. `Buffers` is a `BTreeMap<BufferId, Editing>` and nothing in it requires a pane to point at an entry; `BufferId`'s own doc already reads *"An open buffer. Not a path: the same file can be open once and renamed"* (`request.rs:52-53`).

The **policy** does not come free, and T088 has no basis to invent it. An unattached buffer has no `Pane::area`, so `soft_wrap::wrap_to` (`main.rs:2515`) has no width and a scroll has no bounds to measure against; it has no `StatusVm`; and after step 8, `:wall` would write files the user cannot see — which is either exactly what `ApplyWorkspaceEdit` needs or exactly the surprise nobody wants. That is a judgement about product surprise, not about the tree.

**So T088 ships the container and ships nothing that creates a detached entry.** `close-pane` on the last pane holding a buffer destroys the entry. `docs/TASKS.md`'s T060 entry gets amended to say T060 **inherits a container and owes the rules** — attach/detach policy, what `:wall` and `:q` count, what an unattached buffer's wrap width is — and `lint-action-arms.sh:114-123`'s parenthetical is rewritten to point at that ruling. T060 then has a citation instead of a reading, which is the whole complaint.

---

## 4. Transport verdict

**Both crates enter this workspace. Nothing blocks T050 from starting.** The de-risk agent rsync'd the workspace to `/tmp/phosphor-acp-probe`, restored the committed `Cargo.lock`, added `agent-client-protocol = "2.0.0"` and `rmcp = "3.1.2"` to `crates/phosphor-agent/Cargo.toml`, and ran `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo deny check` — all three pass, `deny` reporting *"advisories ok, bans ok, licenses ok, sources ok"*. **I did not re-run those builds**; the manifest-level findings below I verified myself against `~/.cargo/registry/src/index.crates.io-*/agent-client-protocol-2.0.0/`.

`deny.toml:84-85` bans a duplicate major of `ratatui` and `ratatui-core`. `TEAM.md:495-498` flagged that as *"the kind of thing that only speaks up once they are in."* It spoke, and it passed.

### The one real constraint: ACP 2.0.0 is not a tokio crate

Read this session from the registry manifest: `[dependencies.async-io]` (line 143), `[dependencies.async-process]` (146), `[dependencies.blocking]` (149) — the smol stack, non-optional. `tokio` appears only under `[dev-dependencies.tokio]` (196). `[features] default = []` (44-45) and every other feature is `unstable_*`. **There is no opt-out.** That collides with the root `Cargo.toml`'s deliberate tokio posture (`Cargo.toml:154-167`, whose comment pins `tokio-util`'s `compat` feature precisely because *"`async-lsp`'s main loop reads `futures::io::AsyncRead`, `tokio::process` hands out `tokio::io::AsyncRead`, and `compat` is the adapter"*).

**It is survivable, and the escape hatch is upstream's own.** `ByteStreams<OB, IB>` (`jsonrpc.rs:5551`) implements `ConnectTo` (`:5599`) for any `futures::AsyncRead`/`AsyncWrite` pair, and its doc example at `jsonrpc.rs:5531-5536` is literally `tokio::io::stdout().compat_write()` / `tokio::io::stdin().compat()`. That is the same seam `phosphor-buffer` already uses for `async-lsp`.

**The rule for T050, stated plainly: spawn the agent with `tokio::process` and connect through `ByteStreams` + `tokio_util::compat`. Do not use `AcpAgent` or `stdio.rs`.** The smol usage is confined to exactly two files — `src/stdio.rs:52-53` (`blocking::Unblock::new(std::io::stdin())`/`stdout()`) and `src/acp_agent.rs`, which is the *only* file under `src/` that names `async_io` or `async_process` (verified by grep this session; nothing under `src/jsonrpc*` touches either). Touch neither and the async-io reactor thread and `blocking`'s pool never start, even though the crates compile in.

### Feature agreement: clean

`rmcp`'s tokio features (`sync`, `macros`, `rt`, `time`) are a strict subset of the workspace's. No `net`, no `rt-multi-thread`, no `fs`, no `io-std` enters the graph. ACP contributes no tokio features at all.

**Forward note for T052:** `rmcp`'s `transport-io` feature would add `tokio/io-std`, which is not in the workspace set — and phosphor's own stdin is a terminal in raw mode, so serving MCP over the editor's stdio is the wrong shape regardless. Use the default `transport-async-rw` over a pipe the editor owns. `transport-child-process` adds a new crate (`process-wrap`). Design decision, not a blocker.

### Two traps that look official

- `agent-client-protocol-tokio` 0.11.1 — depends on ACP **0.11.1** (the previous major) and `tokio = { features = ["full"] }`. Taking it forks the ACP major *and* blows the deliberately minimal tokio feature set open.
- `agent-client-protocol-rmcp` 3.0.0 — depends on ACP 2.0.0 (correct) but `rmcp = "2.1.0"` (previous major). Pairs two rmcp majors into the graph.

**Neither may be used.**

### Weight and one new hazard

`Cargo.lock` 393 → 450 (+57 entries), but the **actual compiled graph is 271 → 316 (+45)** — 12 lockfile entries are unselected optional deps of `serde_with` that never compile. Of the +45: 3 ACP, 2 rmcp, **11 smol**, 7 schema/serde, the rest transitive. `polling` is already present via `steel-core`, so `async-io` shares it.

**`tracing` 0.1.44 enters the graph for the first time**, via rmcp and `agent-client-protocol-schema`. The root `Cargo.toml` deliberately dropped `async-lsp`'s `tracing` feature with the note *"this crate emits none, and the subscriber is the binary's"* — still true, since events are no-ops without a subscriber. But the graph now contains crates that **do** emit, and a subscriber installed with the default stdout writer would corrupt the TUI frame. Worth a note beside the `print_stdout` clippy lint.

`docs/SPIKES.md:281-282` names both crates and both versions correctly. Nothing in it is wrong — but it says nothing about ACP being smol-based, and that omission is the whole risk.

---

## 5. Where the tree contradicts the documents

The tree wins. Four corrections, each with what the document should say.

**1. `docs/TEAM.md:469-471` — the `Gutter` claim is false.**
It reads: *"Closing it collapses the two paths into one, which also retires `Node::Prompt`'s scaffolding … and the `Buffer` and `Gutter` composition gaps."*
`scripts/lint-node-kinds.sh:132-141` records `Gutter` with an **empty blocking task** and says explicitly *"it has no creditor… No task in the graph names such a surface, so nothing closes this entry."* And `interpret.rs:484-485` shows a composed `Node::Buffer` renders `BufferView` with its own `.state_column(...)`, so a tree-composed buffer never composes a `Node::Gutter`.
**Should say:** the collapse closes the `Pane` and `Buffer` gaps. `Gutter` keeps its empty creditor and is not T088's.

**2. `docs/TEAM.md:469-470` — the `Prompt` claim is loose.**
`OPEN-QUESTIONS.md:2523-2532` rules the ex line's demolition onto **T058**, and `lint-node-kinds.sh:157-162` records `Prompt` against T058. The ex row is already tree-composed from `Node::Line`/`Node::Label`.
**Should say:** T088 removes the widget path that made a second draw path necessary; T058 retires `Node::Prompt`'s scaffolding.

**3. `docs/TASKS.md:2718` — T088's *Done when* is narrower than the gate.**
It reads *"two panes split, focus moves between them, and opening then closing a float returns focus exactly where it was."* But `scripts/lint-action-arms.sh` will demand arms for the four capabilities declared `[S6 / "T088"]` at `crates/phosphor-core/src/action.rs:630-647` — `SplitPane`, `FocusPane`, `ClosePane`, `ResizePane` — the instant the task is ticked, plus `Query::Ui::Panes` (`query.rs:410`, same declaration). There are **zero `Action::Pane` arms in `main.rs` today**; the domain falls through to `NotYetImplemented` at `main.rs:5939-5941`.
**Should say:** add the four arms and the `panes` query to the *Done when*, so the criterion matches what the gate enforces.

**4. `docs/TEAM.md:495-500` — the external-blocker line is now answerable, and for T088 it is the wrong flag.**
It reads *"external blocker **yes**… `agent-client-protocol` 2.0.0 and `rmcp` 3.1.2 … have never been in this workspace's dependency graph… nothing has resolved or compiled either."* Both now resolve, build, pass clippy with warnings denied and pass `cargo deny check`.
**Should say:** the crates resolve and the `deny.toml` ban is satisfied; the surviving finding is that ACP 2.0.0 is smol-based, which constrains T050's transport shape (`ByteStreams` + `tokio_util::compat`, never `AcpAgent`/`stdio.rs`) rather than blocking the window. And for `T088` specifically, external blocker is **no** — it needs neither crate, which is the second reason it runs first.

*Verified correct, for the record:* `TEAM.md:461-462`'s **193** `editing.` and **78** `self.editor` counts are exact against `main.rs` today, as is *"244 lines"* (`4817`→`5061`) and *"the interpreter's deferred set is six kinds"* (`TEAM.md:482` vs `interpret.rs:624-629`).

---

## 6. Open decisions — Teej's, not the tree's

> **Questions 1 and 2 are RULED, 2026-08-20 — Teej: follow nvim and telescope.** Question 1 is
> answered *no* — telescope's `<CR>` opens in the current window and splits are `<C-v>`/`<C-x>`,
> and `AcceptHow` already carries both. Question 2 is answered *allow, with independent
> viewports*, and it is not a separate question from ruling (a): it is the same decision, because
> a shared viewport makes two panes on one file scroll in lockstep and that is the "reads as a bug
> on screen" outcome this list itself named. Both are recorded at `T088`'s entry in
> `docs/TASKS.md`. Questions 3 and 4 below remain open.

1. **Does the files picker (T046) open results into a new pane?** `TASKS.md:2716-2717` defers it here by name and asks for the answer in this entry. `accept_picker` currently declines `AcceptHow::Split` at `main.rs:6534`. The arm is trivial once splits exist; the question is whether that default is wanted.
2. **Same file in two panes: refuse, or allow with a shared viewport?** Ruling (a) narrows it to those two. Allowing gives two halves that scroll together, which reads as a bug on screen. Refusing is honest but strange for a vim-shaped editor. No acceptance criterion needs it — T054's transcript is `PaneKind::Transcript` and holds no buffer. CP-6's manual half is where this gets settled by looking.
3. **Rename `Editing` → the per-buffer record's real name?** After step 8 it is exactly what its doc always claimed. `Buffer` is taken by `fn buffer` (`main.rs:3629`) and by `Node::Buffer`. Left out of the plan because it churns ~200 sites for zero behaviour and would bury the real diff; as its own commit right after step 8 it is cheap.
4. **`BufferAction::SetRegister` (`main.rs:5280`) is in the wrong domain after step 8** — a `Buffer`-domain capability writing session state. The arm still works; the registry just groups it in the wrong room. Moving it is a `spine` contract change, so T088 should **file a request and continue**, per TEAM.md's coordination rule.

---

## Scope

- **Files:** `crates/phosphor/src/main.rs` (the bulk), `crates/phosphor-ui/src/interpret.rs` (the `fill` builder), `crates/phosphor/tests/loop_pty.rs`, `crates/phosphor-ui/tests/golden_frames.rs`, `scripts/lint-node-kinds.sh`, `scripts/lint-action-arms.sh`, `docs/TASKS.md`, `docs/TEAM.md`, `docs/OPEN-QUESTIONS.md`
- **Named units:** 4 new types (`Pane`, `Shell`, `Cx`, `PaneTree`) + 2 containers (`Buffers`, `Panes`) + `Geometry`; `Editing` 40 fields → ~20; 34 `act`/`apply` call sites re-signatured (9 production, 25 test); 17 `editing.area` sites; 6 `..`-discarding arms honoured; 4 new `Action::Pane` arms + 1 `Query::Ui::Panes`; 3 refusals retired (`SaveAll`, `Quit`, `CloseBuffer`); 12 `T088` prose sites in code and tests; 2 `lint-node-kinds.sh` RECORDED rows deleted; 1 `Outstanding` re-keyed
- **Verification:** `just gate` (all six, CI's order) after each of 14 commits · `just tapes-diff` on `1a`, `1a-seeded`, `1a-degraded-nocolor`, `3c`, `8e`, `2a`, `6a`, `6b` at steps 2, 3 and 11 (there is no `6d` tape — `6d` is a golden frame), with `git checkout -- tapes` after each · `just review` on the golden frames · new unit tests: `PaneTree` split/focus/close/resize/resolve, `PaneTree::layout` at 1/2/3 panes and odd widths, per-buffer `edits`/`sent`, keyed `Outstanding`, selector routing (`SetCursor`, reveal, mouse hit-test), swap-clears-`selection_from` · CP-6's manual half
- **Risk:** public API change **yes** (`Editing`, `Painted`, both `Resources` methods, `draw`) · data migration **no** · cross-module **yes** (`main.rs`, `phosphor-ui`, two test crates, two lint scripts, three docs) · reversible **yes**, step by step, each ending green · external blocker **no** for T088; **resolved** for T050, with the ACP transport shape constrained as above
