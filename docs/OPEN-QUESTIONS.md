# Open questions

Things a checkpoint surfaced that need a ruling, and are not yet one. Each entry carries the
evidence with `file:line`, the options, and a recommendation — so the answer is a sentence rather
than a re-derivation.

**What this file is not.** It is not a backlog and it is not a place to record decisions. Once a
question is ruled, the ruling goes where it belongs — the amendment table in
[IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md)'s decision log if it changes a design doc, a task
entry in [TASKS.md](TASKS.md) if it changes the graph, an ownership row in [TEAM.md](TEAM.md) if it
changes who writes a file — and the entry moves to *Closed* below with a pointer. A question that
lives here after it has been answered is the same rot this repo already has lints against.

**The standard for an entry** is the one `CLAUDE.md` sets for everything else: state a fact about a
file only if you read that file, and give `file:line` when the claim is load-bearing. Every
citation below was checked against the tree on 2026-08-12, and every citation added or amended
in the 2026-08-13 ruling pass was re-checked then.

**Citations added from 2026-08-13 name a symbol rather than a line**, per rule 5 of *Concurrency*
in [TEAM.md](TEAM.md): concurrent agents in one worktree move each other's line numbers, and a
wrong line number in a register like this one reads as authoritative. The older entries keep
their `file:line` and are correct as of the date above; treat a line that has drifted as a
citation to re-derive, not as a claim that failed.

**Swept 2026-08-13, in the repair window between `CP-3` and `S4`.** Four entries moved to
*Closed* — §19 on a ruling, and §6, §13 and §17 because the tree had already answered them and
nobody had noticed. The repair-pass list at the end was re-checked item by item and now carries
what is done, what is still open, and one thing that was ruled not worth doing.

---

## Doc-versus-tree disagreements

These are cheap: the tree is right and a document has not caught up. They are listed because
`docs/` is the specification, and a specification that disagrees with the build is a bug in the
specification — but nobody may quietly edit it into agreement.

**All four that stood here are ruled and closed** — §1, §2, §3 and §19, all amending
[TEAM.md](TEAM.md). Nothing has taken their place.

---

## Scope questions

**§4, §5 and §6 are ruled and closed** — `V006`'s split, `6b`'s `q close` footer, and the three
editor-layer names `6b` types that nothing binds.

*Nothing stands open in this category.*

---

## The door's voice

**§7 and §9 are ruled and closed** — the missing `Outcome` case and the two `why` implementations
became one task, `T100`, exactly as both entries recommended. The scope block that stood here
moved with them.

### 8 · `place-watch` takes a `Target`; `6b` passes a string

`(watch-place "src/retry.rs:24" 'delay)` decodes to the alias and then fails on shape.

Here the build is the bug, which is the opposite direction from the other two mockup
disagreements and worth stating plainly. `"path:line"` is what a person types and what an agent
sends over MCP, and `Value` is deliberately smaller than JSON — no arbitrary-key maps — so a
structured `Target` is most awkward at exactly the door that matters most. The alias decoding
already sits at that seam.

*Recommendation: make `path:line` a valid `Target` spelling at the door. The mockup drew what
someone would naturally write, and that is evidence.*

---

## Blocked, and on what

### 10 · The `6b` golden frame

`crates/phosphor-ui/tests/golden_frames.rs`. Was blocked on `surface`, who was not live in Window
C. Two known gaps remain, and they unblock at different times:

- **A tree-composed statusline cannot ask for §5's `#1a201a` ground.** This is a view-tree contract
  question, so it is `spine`'s — and `spine` is fully consumed by `T026` in Window D.
- **`6b`'s coloured literals need the Steel grammar**, which arrives with `define-language`
  (`T037`, S4).

*Recommendation: raise the ground question in the S4 run's first phase, where `spine` is free, and
land the frame once the literals exist. Nothing is gained by starting it in S3.*

**Still open, and now due, checked 2026-08-13.** `crates/phosphor-ui/tests/golden_frames.rs`
carries `screen_1a_minus_agent`, `screen_9c`, `screen_8c` and `screen_8d`, and no `6b` — read this
session. S3 is over: `CP-3` passed both halves and `S4` is the next window, so the *"nothing is
gained by starting it in S3"* clause has expired and the recommendation's own trigger has fired.
Both gaps are `S4`'s to clear — `T037` brings the literals, and the ground question wants
`spine`'s first free phase, which is the same window.

---

## Raised by Window D's S3 run

**§11, §13, §16, §17 and §18 are ruled and closed** — the file lock, the ex line's second draw
path, the hand-rolled codec, `CP-3`'s VHS artifacts, and the eleven declared mutations with no
creditor.

### 12 · Two mockups disagree with two other mockups

A new category — previous findings were build-versus-design. `V006`'s agent transcribed the
worked example byte-for-byte and found the drawings disagree with each other:

- `TUI Mockups.dc.html:164-166` (screen `8a`) and `:872` (screen `3a`) render `retry.rs` line 24
  differently — `.min(policy.max_delay)` alone against the full statement on one line.
- `TUI Mockups.dc.html:1003` (screen `2a`) cites `fetch.rs:3-7` for `fetch_json`, while `2b`/`3b`
  render that function's content at a different implied line range.

This matters more than it looks: `fixtures/` is now a byte-exact transcription of that example, so
whichever rendering wins is what every agent-surface tape at `CP-5` will show.

*Recommendation: Teej picks one rendering per conflict at claude.ai, and `fixtures/` follows. There
is no build change here — nothing is wrong in the tree.*

### 14 · `phosphor --eval` cannot report refusal through its exit code

Verified empirically by `V006`'s agent: a well-formed call the editor *refuses* and a trivial
`(+ 1 2)` both exit `0`, because the refusal is data the evaluation returned rather than a failure
of the evaluation. Only a Steel-level error — unbound identifier, bad arity — exits `1`.

Nothing is wrong today. The trap is forward-looking: any seeding or tape tooling that checks `$?`
to decide whether a call worked will silently misreport the day `T041` lands and refusals turn into
successes. `scripts/seed-fixtures.sh` checks the printed value instead.

*Recommendation: decide whether refusal is an error at the CLI door. It is the one door with an
exit code, and "the editor declined" is arguably a `1`. Touches `T023`'s contract, so it is
`spine`'s.*

## Raised by the test-depth run

Property tests, fuzzing and benchmarks found these. None was known before; three are product
bugs, one is a design question with a number attached, and one is costing us signal right now.

### 20 · A gating test is load-flaky, and it makes the coverage tool unusable

`crates/phosphor/tests/loop_pty.rs`'s `driven::an_operator_leaves_the_cursor_where_the_next_key_
can_prove_it` failed under three-way CPU contention with `left: "XALPHA BETa"` against
`right: "XALPHA beta"` — the pty driver read the screen before the last keystroke rendered. It
passes on an isolated re-run of the same binary. **It gates CI, and a saturated GitHub runner is
the same condition.** Two agents hit it independently.

Second-order, and worse: **`cargo llvm-cov --workspace` cannot complete on this repo.** llvm-cov
drives `cargo test`, not nextest, so that one test aborts the run and no report is written — the
map denies you exactly when you want it. `just coverage` works around it with
`--no-report --no-fail-fast`, which is a workaround, not a fix.

*Recommendation: fix the test, not the runner. It is a synchronisation bug in the driver — it
should wait for the frame that proves the keystroke landed rather than sampling after a delay.
The same pattern is in 35 other `loop_pty` tests and only this one has been seen to fail, which
is worth understanding before assuming the rest are safe.*

### 21 · `gU` splices back a different length than it took

`text::cased` is **not** character-count-preserving, in any of its three modes. Measured under
this toolchain: `to_uppercase('ß')` is `"SS"`, `'ﬁ'` is `"FI"`, `'İ'` and `'ǰ'` both grow, and
toggle inherits both — so `~~` on `ß` gives `"ss"` and is not an involution.

`Buffer::SetCase` carries a `Target`, and the host replaces the target's span with the cased text.
When the cased text is longer, everything after it moves. That is a real editing bug on German,
Turkish, and any ligature.

- **Make the caller span-aware** — re-derive the span from the result rather than assuming length.
- **Or restrict `gU` to ASCII**, which is a smaller product.

*Recommendation: the first. The property `cased_never_loses_a_character` is shipped and true;
what is missing is a caller that believes it.*

### 22 · Dragging a window edge can cost 5.7 seconds

A soft-wrap rebuild is **41 ns/character**, dead linear over a 16× climb, indifferent to line
shape — and nothing caches it. `crates/phosphor/src/main.rs` calls `soft_wrap::wrap_to` once per
turn of the main loop, and the no-op path is genuinely free (10 ns), so the module header's
*"calling it every frame is free"* holds exactly. **Only real width changes pay, and they pay a
lot**: ~400 KiB of buffer is one whole frame per resize; 3.3 MB is 138 ms; dragging 120 → 80
columns on that buffer is 5.7 seconds of solid CPU and one late frame per column.

Design Language §8 makes a torn frame a P0. This is how you get one.

- **Cache by (width, revision)** — the obvious fix, and the rebuild is already keyed on exactly
  those two things.
- **Or wrap incrementally**, which is a rewrite of `T081`'s core.
- **Or debounce the resize**, which hides it rather than fixing it and breaks *nothing moves
  unless you asked* the moment a debounce fires late.

*Recommendation: the cache. `T081`'s own note says nothing caches because rebuild happens when
buffer, folds or width change — that was a reason not to bother, and the number says otherwise.*

### 23 · Compaction reclaims nothing, and nothing calls it

Three measurements, all `T095`'s input:

- A typing session's compaction is **net negative**: 16,385 records in, **16,387 out**. 620 KiB
  rewritten to save 0 KiB, because `undo::History::snapshot` emits one `Record::Node` per node and
  drops none, then appends its own `Cursor` and `Saved`. A walking session reclaims 12.3%, which
  is `Cursor`/`Saved` churn — so compaction *works*, and a history that only grows has nothing to
  collapse.
- **`should_compact()` is false on every freshly opened log, always.** `Log::open` sets its
  denominator to `state.snapshot().len()`, which for an undo history is one record per node — so a
  session starts already as short as compaction would make it. A 4,096-record log needed 4,100
  *more* groups in that process before the policy first said yes.
- **Nothing in `crates/phosphor/src/main.rs` calls `compact_if_needed` or `compact` at all.**

*Recommendation: `T095` should start from these numbers rather than from the assumption that a log
needs compacting. The real cost is not size — it is the startup fold, below.*

### 24 · Startup fold is superlinear in undos

`Record::Cursor`'s fold is `History::walk_to`, which re-points `redo_child` from the target back to
the root — O(depth) per undo. 16,385 typing records fold in 2.4 ms and the per-record cost *falls*;
18,689 walking records fold in **69 ms** and the per-record cost *climbs* 4.6× over the same 16×
range. That is 4.1 frames before the buffer is drawn, on a session with a lot of undo in it.

Deliberately printed and not asserted by the benchmark: the day somebody makes `walk_to`
incremental, the assertion would be the thing that broke.

### 25 · Three tests that pass for the wrong reason

The gate's theatre check planted against every property added and found five weak or false. Two
were fixed in the same window (`any_value_is_decoded_or_refused` gained the `iff`; the varint
decoder gained `NonMinimalVarint` and a pinned case). **Three remain, recorded rather than
concealed:**

- `cased_never_loses_a_character` and `upper_and_lower_are_idempotent` both survive **swapping
  `Upper` and `Lower`** in `text::cased`. Only the pinned example `cased_grows_on_a_sharp_s`
  catches it, so the entire defence of `gU` meaning the right thing is one hand-written example.
- `every_record_round_trips_through_the_codec` never generates a deletion, a replacement, a
  multi-byte character, or a string longer than two ASCII bytes — `removed` is always empty and
  `inserted` is always `x{0..2}`.

*Recommendation: fix the generators. A property whose generator only produces the happy case is
the specific failure this build has now shipped three times — a vacuous lint, a CRC property that
could not fail, and these.*

---

## Repair pass — queued work, not questions

These need no ruling. They were collected here because every one of them lands in a file that no
agent in the S3 run owns, so none of them could be done inside it.

**Most of them have run.** The pass happened before `CP-3`'s manual half, which has now passed
(2026-08-13, no findings — [TASKS.md](TASKS.md)'s `CP-3` entry). Each item below is marked with
what the tree says **checked on 2026-08-13**, by symbol rather than line, because concurrent
agents move line numbers inside a window:

- **DONE** — verified in the tree this session, with what was read named beside it.
- **OPEN** — verified *still* open in the tree this session. These are the second repair
  window's list.
- **RULED NOT TO DO** — looked at, and the answer was to leave it, with the reasoning recorded
  in the file itself rather than here.

The status marks are the point of keeping this section rather than deleting it: a list that says
only what is left cannot tell you whether the rest was done or forgotten.

> **Why there are so many, and it is one cause.** The S3 run gave `crates/phosphor/src/main.rs` to
> exactly one agent, in phase 2, so that concurrent agents could never collide in the host. That
> made the run safe and it starved the integration point: every widget built in phases 3 and 4
> landed complete, tested and **uncomposed**, because by then nobody could write the file that
> composes it. The result is a window where sixteen agents finished, `just gate` is green, and
> four of the surfaces `CP-3` judges do nothing when you press the key.
>
> Verified in the tree on 2026-08-12: `grep -c 'KeyHints' crates/phosphor/src/main.rs` = **0**;
> `grep -rn 'unknown_key\|UnknownKeyHint' crates/phosphor/src/` = **0**;
> `grep -c '"z' runtime/keymaps.scm` = **0**; `grep -c 'SetFold\|FoldAll' main.rs` = **0**.
>
> The fix for the *window* is `R2` and `R17`–`R19` below, **all four of which have landed.** The
> fix for the *method* is to give the host to a wiring agent in the last phase of every window
> from now on, whose whole job is that nothing shipped this window is unreachable from a
> keystroke — now rule 2 of *Concurrency* in [TEAM.md](TEAM.md).

### Done

- **R2 · DONE — undo is wired into the host.** `struct Timeline` in
  `crates/phosphor/src/main.rs` owns an `UndoTree`, `Timeline::opened` restores from the journal
  and `Timeline::detached` covers the scratch buffer; `fn restored` rebuilds the tree through
  `UndoTree::from_parts`. The fork's history path is gone rather than kept as a fallback. Proven
  through the shipping loop, not the widget: `undo_survives_quitting_and_reopening`,
  `undo_survives_a_kill_9` and `undo_and_redo_walk_the_tree_through_the_loop` in
  `crates/phosphor/tests/loop_pty.rs`.
- **R17 · DONE — `SPC` opens the leader popup.** `main.rs` composes `Node::KeyHints` with
  `Density::Grid`; `driven::pressing_space_opens_the_leader_popup` and
  `a_repl_rebind_reaches_the_leader_popup` drive the real binary on a pty.
- **R18 · DONE — the unknown-key hint fires.** `UnknownKeyHint` is constructed in the loop and
  `Action::App(AppAction::ShowUnknownKeyHint { .. })` has an arm.
  `an_unbound_key_teaches_once_and_never_again` covers both the firing and the *not* firing.
- **R19 · DONE — folds exist.** `Editing::act` has arms for `ViewAction::SetFold`, `FoldAll` and
  `UnfoldAll`, the `z` bindings are in `runtime/keymaps.scm`, and
  `za_closes_the_fold_the_cursor_is_in` proves it from a keystroke.
- **R4 · DONE — `parse_seq` spells a bare `<`.** `input/key.rs`'s `'<'` arm falls back to
  `Key::char('<')` when the bracket does not close, so `.` after `<<` repeats.
- **R5 · DONE — `crates/phosphor-core/src/input/vim.rs` is deleted**, and `input.rs` no longer
  declares the module. Checked by `ls`, not by grep.
- **R8 / R16 · DONE — the stale comments are fixed, and one of them is now mechanical.**
  `crates/phosphor-ui/src/interpret.rs`'s header carries a *still deferred* table held to the
  tree by `tests::the_deferred_set_is_exactly_the_kinds_named_here`, which draws one node of
  every listed kind and asserts the tags — so a widget that starts drawing reddens the test and
  the fix is to delete a row. `crates/phosphor-ui/Cargo.toml`'s comment now records that `T026`
  turned the fork's `crossterm` feature back off. `TEAM.md`'s *"CP-2's manual half is
  outstanding"* sentence is gone.
- **R9 · DONE — one colour mapping.** `fn hue` in `crates/phosphor-ui/src/gutter.rs` is the only
  `StateMark` → `Color` map left; `buffer_view.rs` has none.
- **R10 · DONE — the legacy chord fallback is reachable.** `machine.set_protocol(…)` reads
  `term.capabilities().keyboard` in `main.rs`, and
  `the_legacy_chord_fallback_is_reachable_on_a_legacy_terminal` drives it on a pty with the
  keyboard forced.
- **R11 · DONE — the ex line has a range grammar.** `phosphor/ex-range-at` splits the range off
  before `phosphor/ex-split` sees the name, and `phosphor/ex-current-range` carries it to the
  command. `crates/phosphor-steel/tests/shipped_grammar.rs` asserts `:'<,'>c` names `:c` over a
  range and that `:'<,'>w` is `:write` with the range read off.
- **R12 · DONE — the layer's canonicaliser folds a bracketed key.** `runtime/keymaps.scm` carries
  the three rules by name (order, case-on-the-character, shift-folds-into-a-plain-character) and
  says which `phosphor-core` function each mirrors.
- **R14 · DONE — the toolchain regex is scoped.** `scripts/doc_claims.py` section 4 now requires
  a `toolchain`/`channel` word on the same line before treating a version as a pin quote, so the
  check that caught a real stale pin at `CP-0` is intact and the `insta` comment no longer
  reddens it.
- **R7 · DONE, both halves — and the *"ruled not to do"* half was overtaken by the tree.**
  `soft_wrap.rs`'s `EditMode` collapsed to `pub use phosphor_core::request::EditMode` in the
  first pass. The `ScrollRequest` half was recorded here as *ruled to stay two*; the repair
  window between `CP-3` and `S4` collapsed it anyway, on both sides of the seam and in one
  window. `crates/phosphor-ui/src/buffer_view.rs` now reads
  `pub use phosphor_core::request::ScrollRequest;`, the 1-based-`u32` → 0-based-`usize`
  conversion moved into `Viewport::scrolled` behind a private `index_of`, and the host's own
  converter is gone — `crates/phosphor/src/main.rs`'s `ViewAction::Scroll` arm passes `*request`
  straight to `buffer_view::apply_scroll` and there is no `fn scroll_request` left in the file.
  Read in the tree on 2026-08-13. **What the old entry pointed at no longer says what it said:**
  `buffer_view.rs`'s header section is now titled *"The duplicated type, and how it became one"*
  and argues the collapse rather than the split.
  **One consequence is still owed and belongs to whoever next holds `phosphor-core`:**
  `crates/phosphor-core/src/request.rs`'s doc comment on `ScrollRequest` is now false in three
  places — it calls `buffer_view::ScrollRequest` *"the same shape"* when it is a re-export, it
  carries a *"Contract note for Window D"* asking `surface` to collapse the two, and it says
  *"`T026` converts at the boundary until then"* when nothing converts at the boundary. Its
  `buffer_view.rs:180` citation has also drifted, which is the failure *Concurrency* rule 5 in
  [TEAM.md](TEAM.md) names. No lint reads doc prose against the tree, so this will not surface
  on its own.
- **R20 · DONE — recaptured, and it answered its own question.** The pair now differs
  (`insert-whitespace-marks-normal.png` and `-insert.png` have different digests), so the
  byte-identical stills were the VHS pipeline duplicating a frame and **not** the surface failing
  to render — the build was right all along. `tapes/artifacts/DUPLICATES.md` records the pairs
  that are identical by construction.

### Still open — the second repair window's list

- **R3 · OPEN, and it turned out to be the smaller half of something.**
  `crates/phosphor-ui/src/interpret.rs`'s `Node::Gutter` arm says in its own words that the
  degraded `▎` is unreachable from here: the tree carries no terminal capability and adding a
  prop is `spine`'s call. The widget's degraded form is tested and reachable directly.
  **The larger half is that `Node::Gutter` is composed by nothing at all** — found by
  `scripts/lint-node-kinds.sh`, added in this window, which records it as one of two gaps with
  *no creditor*: the state column ships as `BufferView`'s left column (`T031`, ticked and built),
  and this kind is that column **without** the editor, for a surface that wants it — and no task
  in the graph names such a surface. So R3 is asking how a composition should reach a capability
  for a node kind nothing composes. Whoever picks it up should decide the second question first.
- **R6 · OPEN.** `scripts/lint-one-vm-door.sh`'s `entries=` regex still lists
  `keymap::press|keymap::reset`; neither name exists. Coverage is unaffected — both real call
  sites match a different alternative — but the two dead alternatives should read
  `keymap::resolve|keymap::ex`.
- **R13 · PARTIAL.** Every *component* of `6d`'s sentences is now in the live keymap — `gs` as an
  operator, the four nouns as object rows, `]u`/`[u` as sequence rows, and `:c` over a range —
  and `help_narrows_to_the_agent_objects_topic` reads the four nouns off a real frame. What is
  still absent is a row that spells a **sentence**: `:help agent-objects` teaches you `gs` and
  `ib` and never shows you `gsib`. Arguably correct for a grid rendered from the live keymap,
  since a sentence is a composition and not a binding — which makes this a question for whoever
  next holds `T086`, not a defect to fix on sight.
- **R15 · OPEN, and re-verified.** `gh api repos/TrevorS/phosphor/branches/main/protection`
  answers *"Branch not protected"* (404) on 2026-08-13. The CI jobs are right; nothing enforces
  them at the repository. A GitHub settings change only Teej can make.

### Ruled not to do

*Empty since 2026-08-13.* `R7` lived here and no longer does — see the `R7` entry under **Done**
above. The heading stays because the distinction it draws is worth keeping: an item looked at and
left is not the same as an item forgotten.

### The originals, for the reasoning

The entries below are the list as it was written on 2026-08-12, kept because each one records
*how* the gap was found, which the status lines above do not. Read them for the method, and the
marks above for the state.

- **R20 · `tapes/insert-whitespace-marks.tape` needs recapturing, and its artifacts are gone.**
  The two stills it produced — `-normal.png` and `-insert.png` — were **byte-identical**, 51,293
  bytes each, and were committed in `aa00473`. The tape is written correctly: it waits for the
  `NORMAL` chip, screenshots, types `i`, waits for the `INSERT` chip, screenshots again. **The
  mode chip alone should differ between those two frames**, so byte-identical output means the
  second screenshot never advanced — which points at the VHS capture pipeline rather than at
  whitespace marks. The same session recorded VHS answering "no frames" 10/10 times on a
  known-good sibling tape, so sandbox flakiness is the leading explanation. Not asserted: recapture
  is what settles it.
  Two lessons, both cheap and both already paid for. `scripts/lint-repo-hygiene.sh:51` walks
  `git ls-files`, so an untracked duplicate is invisible — **a green `just gate` before a commit
  does not survive the commit**, and the gate must be re-run after staging. And an agent's claim to
  have "verified by a real capture" is only as good as the capture: this one said it saw `··` in
  INSERT and not in NORMAL, from two files that are the same bytes.
- **R17 · The `SPC` leader popup does nothing.** `main.rs` never composes `Node::KeyHints` when the
  machine is `SPC`-pending; there is no leader variant in `Surface` (`main.rs:2040-2052`) or
  `Intent` (`main.rs:227`). Proven empirically rather than by reading: a real VHS capture of a
  frame before and after pressing Space diffed at **0 pixels**. `T034`'s `3c` snapshot passes
  because the test hand-builds the tree — its own module doc at `tests/screen_3c.rs:27-35` says so.
  **`CP-3`'s manual half asks "is the `SPC` namespace learnable?"** against a build where `SPC`
  does nothing.
- **R18 · The unknown-key hint never fires.** `UnknownKeyHint` is referenced nowhere under
  `crates/phosphor/src/` outside its own test. `T035` is complete and Tier-1 tested at three
  widths including the negative case; no call site exists in the event loop.
- **R19 · Folds do not exist.** No `z`-prefixed binding in `runtime/keymaps.scm`, and `Editing::act`
  has no arm for `ViewAction::SetFold`/`FoldAll`/`UnfoldAll` (declared at `action.rs:414-424`), so
  they fall to `Refused(NotYetImplemented)` at `main.rs:1505`. Typing `za` today runs vim's plain
  `a` and enters insert, with `z` silently swallowed. `CP-3`'s VHS list asks for folds collapsing
  and expanding; unlike the other three this one has **no S3 task behind it at all**, so it is new
  work rather than wiring.

- **R2 · Wire undo into the host. The largest single gap in the window, and invisible from the
  test count.** `main.rs:1440-1455` still answers `HistoryAction::Undo/Redo` with the fork's
  `self.editor.apply(Undo)` and treats `CommitUndoGroup` as a no-op. `T029`'s tree and `T030`'s
  journal are both built, both green, both proven with real `SIGKILL`s — and **neither is
  connected to the editor**: `grep -n 'journal|UndoTree'` in `main.rs` returns nothing. So today
  `u`/`<C-r>` work with the *fork's* semantics, which truncate on divergence
  (`vendor/…/history.rs:19-22`) and cap at 1000 batches; branch-preserving undo does not exist in
  the running editor; and "quit, reopen, undo" restores nothing. Both `CP-3` criteria that mention
  undo are PARTIAL for this one reason.
  Three parts: wire the tree, wire the journal, and write the `phosphor-buffer` ↔ `phosphor-core`
  conversion **in the binary** — `phosphor-core` cannot depend on `phosphor-buffer`, which carries
  the fork, ropey and tree-sitter. `journal.rs`'s `pub mod undo` already mirrors
  `phosphor_buffer::undo` field-for-field and hands back exactly the triple `UndoTree::from_parts`
  takes. The fork's undo path must *go* rather than remain a fallback — two live histories fight.
- **R3 · The gutter's `▎` degradation is unreachable from composition.** `Node::Gutter` carries
  only a `BufferId` and the `Interpreter` has no terminal-capability channel, so the arm always
  draws the block. The widget's degraded form is tested and reachable directly. Adding the channel
  is a view-tree change, so `spine`'s.
- **R4 · `parse_seq` cannot spell a bare `<`** (`input/key.rs:317-322` — `<` opens a bracketed
  token and an unclosed bracket answers `None`). Consequence: `.` silently does nothing after `<<`
  or `<w`, because `last_change` round-trips through `notation_of` and back. The keyboard path is
  fine; only `parse_seq`-based paths (`.` repeat, feed-keys) are affected.
- **R5 · Delete `crates/phosphor-core/src/input/vim.rs`.** `T033` transcribed it into
  `runtime/keymaps.scm` and unwired it, but could not delete it — not its file set. Needs the
  `pub mod vim;` line at `input.rs:94` dropped and 20 `vim::table()` call sites in
  `tests/input.rs` repointed. Until then `no_bindings_in_rust.rs` exempts that one path *by name*.
- **R6 · `scripts/lint-one-vm-door.sh:83`** lists `keymap::press|keymap::reset` in its VM-entry
  regex; neither name exists any more. Coverage is unaffected — both real call sites match a
  different alternative — but two alternatives are dead and should read `keymap::resolve|keymap::ex`.
- **R7 · Two duplicated types at the `spine`/`surface` seam** — **both collapsed**, `EditMode` in
  the first repair pass and `ScrollRequest` in the `CP-3` one. Each is now a single definition in
  `phosphor-core` that `phosphor-ui` re-exports, and the coordinate conversion `main.rs` used to do
  at the boundary lives in `Viewport::scrolled` instead. See the entry above for why the
  `ScrollRequest` half was recorded as *ruled to stay two* and then done anyway.
- **R8 · Two comments that stopped being true when `T026` landed.**
  `crates/phosphor-ui/Cargo.toml:20-37` describes the fork's `crossterm` feature as ON; `T026`
  deleted it. `crates/phosphor-ui/src/interpret.rs:52` says the five Window D node kinds each
  defer; the gutter draws. Both go stale further as `T032`/`T034`/`T086` land, so this is one edit
  by the file's owner at the end, not five.
- **R10 · One line makes the legacy chord fallback reachable.** After `let mut term = Term::new()?`
  (`main.rs:802`), `machine.set_protocol(…)` from `term.capabilities().keyboard`. `T027` built and
  tested the fallback in `tests/chords.rs`, and nothing in the binary calls it — so **`CP-3`'s
  "then on the degradation terminal" proves nothing as built.** `$PHOSPHOR_KEYBOARD=legacy|kitty`
  exists in `phosphor-term` to make that testable without different hardware.
- **R11 · An ex range grammar.** `runtime/keymaps.scm:602-633` — `phosphor/ex-split` takes name and
  args only, so `:'<,'>c` looks up a command called `'<,'>c` and *errors*, which is exactly what
  `T028`'s done-when forbids. Cannot be fixed from `phosphor-core`: the ex line is scheme all the
  way down (`phosphor-steel/src/keymap.rs:233` hands the whole line to the layer).
- **R12 · The layer's canonicaliser should fold case and order like `Key::new` does.**
  `runtime/keymaps.scm:63-81` copies a bracketed key verbatim, so `<C-K>`, `<S-C-k>` and `<C-s-k>`
  are bindings **no keystroke can ever reach** — the machine now always asks with `<C-S-k>`.
  `no_bindings_in_rust.rs` cannot see this, because it only drives keys the decoder produces.
- **R13 · `6d`'s three sentences are not in the live keymap.** `runtime/keymaps.scm:337-340` has
  the four nouns as object rows but no `viu` / `sib` / `dih` / `:'<,'>c` / `]u` / `[u` help
  entries. `T086` renders from the live keymap by design, so `:help agent-objects` draws the nouns
  and none of the sentences. Whoever holds the keymap adds the rows; `T086` needs no change.
- **R14 · `scripts/doc_claims.py:214` reads any `1.NN.N` in `ci.yml` as a toolchain quote**, so a
  comment citing `insta 1.48.0` (added with `V008`) reddens `just lint`. **This is what the gate is
  currently failing on.** Narrow the regex — require a `toolchain`/`channel`/`rust` context, or
  anchor on the pin's shape — rather than deleting the comment, or the check that caught a real
  stale pin at `CP-0` gets weaker.
- **R15 · `main` has no branch protection.** `V008`'s report claimed *"a Tier-1 failure blocks
  merge"*; the gate checked `gh api …/branches/main/protection` and got a 404. The CI jobs are
  right; nothing enforces them at the repository. That is a GitHub settings change, not a code one.
- **R16 · Three stale doc claims to fix in one edit.** `TEAM.md:299` still says *"CP-2's manual
  half is outstanding, and Window D does not start until it passes"*. `interpret.rs:28,51` is
  headed *"Primitives that do not exist yet"* and says the five Window D kinds are "each still
  deferring" — three now draw (`:434`, `:452`, `:506`). And `phosphor-ui/Cargo.toml:20-37`
  describes the fork's `crossterm` feature as ON. Three agents flagged rather than folded, which
  is correct; someone should now make the one edit.
- **R9 · A colour mapping in two files.** `StateMark` becomes a colour at
  `buffer_view.rs:136` (private) and again in `gutter.rs`'s `hue`. The *priority ladder* is not
  duplicated — `buffer_view` has no resolution at all — so `gutter.rs` owns that outright. Only the
  colour half needs collapsing.

---

## Closed

Rulings of **2026-08-13** first, then what came before. Each says where the ruling now lives, so
this section is a set of pointers and not a second copy of the answer.

Six moved here in the repair window between `CP-3` and `S4` — §19, §6, §13, §17, and §7 with §9
as one. Three of them were closed by *reading the tree*, not by anyone deciding anything: the
work that answered them had already landed and the question outlived its answer, which is the rot
this file's own header names.

- **§7 and §9 · The door does not speak §6's voice. RULED: one task, as both entries
  recommended.** They are the same defect — no `Outcome` case for *"it ran and raised"*, so a
  refused query surfaces Steel's `Error: Kind:` envelope; and `door.rs::why` against
  `answer::why` phrasing one enum two ways — and they rewrite the same parity expectation set, so
  doing them separately means regenerating and reviewing it twice. → `T100` in the new
  *B · The repair window* section of [TASKS.md](TASKS.md), which carries the scope block that
  stood here, and to `spine` in [TEAM.md](TEAM.md). The expectation count is deliberately not
  quoted in prose any more: `scripts/doc_claims.py` recomputes it, and a hand-written copy is
  what went stale.

- **§19 · Who owns `phosphor-ui/{interpret,frame}.rs`? RULED 2026-08-13: `spine`.** Both files
  are `T079`'s — *tree interpreter + frame cache* — which [TEAM.md](TEAM.md) already assigns to
  `spine`, and `interpret.rs` is where a `Node` kind *becomes* pixels rather than a widget that
  paints one, which makes it the view-tree protocol's other half and therefore single-writer rule
  1's. Same rule that moved `T014` and `T027`: **the file decides the task.** Stated positively
  so it reads in one direction — `surface` owns every file in `phosphor-ui` that draws one node
  kind, `spine` owns the two that draw none. → `spine`'s row in [TEAM.md](TEAM.md)'s ownership
  table, plus the `phosphor-ui` bullet under *Shared boundaries* and a note in `surface`'s role.

- **§6 · Three editor-layer names `6b` types that nothing binds. CLOSED BY THE TREE: the flag is
  a test, and it already exists.** `goto`, `claude` and `region-author` stay unbound until `T041`
  returns the records they accessorise — that half was always settled. The open half was the
  *form of the flag*, and `crates/phosphor-steel/tests/screen_6b.rs`'s
  `the_session_is_typable_but_the_store_is_s5` is it: it runs all four of `6b`'s lines and
  asserts what came back names each unbound identifier, so the day `T041` binds them the
  assertions go red and force the binding rather than waiting to be recalled. That is exactly
  what the recommendation asked for, written before the question was swept. → no file changes;
  the test is the record, and `crates/phosphor/tests/screen_6b.rs`'s snapshot carries the same
  claim on a frame.

- **§13 · The ex line draws outside the view tree. RULED as recommended: scaffolding with a
  demolition date, and the comment is in the tree.** `Node::Prompt` is still deferred — it is a
  row in `crates/phosphor-ui/src/interpret.rs`'s *still deferred* table, against `T058` — and the
  ex row is drawn from `Node::Line` / `Node::Label` in the binary. `main.rs` says so where
  `ex_line` is declared: *"`view::Node::Prompt` is the vocabulary's shape for this and
  `phosphor-ui` defers it to `T058`, so what S3 can hold is the primitives."* The same trade
  `T090` made and `T026` collected on. → the demolition is recorded on `T058` in
  [TASKS.md](TASKS.md); the comment at the site is what makes it findable.

- **§17 · Does `CP-3` sign off without its VHS artifacts? ANSWERED BY THE WORK, and then by the
  checkpoint.** All four surfaces went live in the repair pass, all four were captured, and
  `CP-3` passed both halves on 2026-08-13. The recapture also settled the question the first
  attempt dodged: the whitespace pair now **differs**, so the byte-identical stills were the VHS
  pipeline duplicating a frame rather than the surface failing to render — deleting them greened
  `scripts/lint-repo-hygiene.sh` without answering it, and recapturing answered it. The build was
  right all along. → `tapes/`, `tapes/artifacts/DUPLICATES.md`, and R20 above; what remains is
  ordinary `harness` standing work under `V005`.

- **§1 · Is `Node::KeyHints` one widget file or two? RULED: one — `key_hints.rs`.** `spine` added
  one node kind (`Node::KeyHints`, `crates/phosphor-core/src/view.rs:500`) carrying a `Density`
  (`crates/phosphor-core/src/view/props.rs:496`), and `TEAM.md`'s own rule is that a widget file
  exists because `spine` added a node kind. `help_grid.rs` and `keymap_footer.rs` never existed;
  `crates/phosphor-ui/src/key_hints.rs` does. One kind, one file, one draw site — the same
  principle `scripts/lint-one-escape-hatch.sh` enforces for `Node::Spans`. → the ownership table
  and the per-widget rule in [TEAM.md](TEAM.md), both amended.

- **§2 · Who owns `T027`, the kitty keyboard protocol? RULED: `spine`.** The file decides the
  task, as it did for `T014`: the negotiation is in `phosphor-term`
  (`KeyboardProtocol::Kitty`, `crates/phosphor-term/src/lib.rs:124`) and the arm that consumes it
  is `machine.set_protocol(…)` in the binary — both `spine` crates — while `TEAM.md`'s line for
  the other role is *"`surface` draws, and never touches a terminal."* → `T027` moved to `spine`'s
  task list in [TEAM.md](TEAM.md); `surface` is 29 tasks, `spine` 26.

- **§3 · Window D's live-teammate count. RULED: four, not five.** `agent` owns `T050`–`T070` and
  `T074`–`T077`, and none of them falls in Window D. → the window table in [TEAM.md](TEAM.md),
  with a note beside it in the style of the `harness` one.

- **§4 · `V006` cannot meet its own acceptance criterion in Window D. RULED: split it, on the
  `T022` precedent.** `V006` keeps the fixture tree and the `phosphor --eval` seeding mechanism —
  the half whose mechanism is provable now — and the seeded store state becomes a criterion on
  the S5 task that lands the store. → both halves written into [TASKS.md](TASKS.md), at `V006`
  and at `T041`.

- **§5 · `6b`'s footer promises `q close` on a surface whose body is a text input. RULED: the
  build wins; the drawing is amended to `esc close`.** `q` types and `esc` closes (Design
  Language §9), and this became decidable only when `T026` landed modes in Window D. Teej amends
  `TUI Mockups.dc.html` at claude.ai — **never here.** → the amendment list in
  [README.md](README.md) and §5's table in [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md); the
  build's owed half (a mode-aware footer) is recorded on `T034` in [TASKS.md](TASKS.md).

- **§11 · `just fmt-fix` writes every file, so a file lock cannot hold. RULED: the `TEAM.md`
  rule, not a per-crate recipe.** In a concurrent window, run `just fmt` (check) and fix only
  your own files by hand; a per-crate recipe would invite the `cargo fmt --all` reflex the hook
  already exists to block. → rule 3 of *Concurrency — several agents, one worktree* in
  [TEAM.md](TEAM.md), alongside the four other findings from the same two windows.

- **§16 · Hand-rolled codec and XDG paths, or the crates `SPIKES.md` recommends? RULED: the
  hand-rolled ones stay.** `phosphor-core` is deliberately dependency-free at the floor
  (`crates/phosphor-core/Cargo.toml:9` says so), `T030`'s LEB128 + length-prefixed-UTF-8 codec is
  `SIGKILL`-tested, and the FNV-1a 64 state-dir key is pinned by literal precisely because
  `std`'s `DefaultHasher` is documented-unstable across releases and a toolchain bump would
  silently orphan every user's state. **Do not add `postcard` or `etcetera`.** `SPIKES.md`'s two
  recommendations are superseded on this point and nothing else. → no file changes; the ruling
  is the record.

- **§18 · Eleven declared mutations that no task will ever close. RULED: add the tasks.** An ex
  command that exists and declines beats one that vanished, *but only if something will close
  it* — so `:theme` stays bound and gets a task, and the rest are grouped rather than one task
  per verb. Three of the thirteen gaps had a creditor already and became a line on that task's
  *done when* (`jump` → `T042`, `set-virtual-text-visible` → `T041`, `apply-edits` → `T052`). →
  `T092`–`T097` in the new *A · Arms owed* section of [TASKS.md](TASKS.md), assigned to `spine`
  in [TEAM.md](TEAM.md), and still recorded in `scripts/lint-action-arms.sh`'s RECORDED table —
  **which now needs its empty blocking-task fields filled in with the new ids**, a `scripts/`
  edit this pass could not make.
  **Down to one, checked 2026-08-13.** The lint reports *13 recorded gaps (1 with no task that
  closes them)*, and the one is `ApplyEdits`. The repair window re-homed the capability row itself
  from `S3 / T029` to `S6 / T052`, so the creditor now exists and is named in the vocabulary; what
  is still empty is the RECORDED table's own blocking-task field, which no agent in that window
  owned `scripts/` to fill. That is the whole remaining edit.
  **And it is not cosmetic — checked against the lint's own logic on 2026-08-13.** The
  stale-record check (2) reads `if variant not in unreachable and task in ticked`, where `task`
  is the *declaring* task on the capability row. Re-homing both rows moved them to unticked
  tasks, so `Jump` and `ApplyEdits` have left the `unreachable` set and neither check can fire on
  them today: both entries are **inert**. `Jump` re-arms correctly — its blocker `T042` is also
  its declaring task, so check (3) fires the day `T042` is ticked without an arm. `ApplyEdits`
  does **not**: with an empty blocker, check (3) is skipped, and check (1) is excused by the
  record itself, so ticking `T052` with no arm would hide the gap permanently. Filling the field
  with `T052` is what closes that hole. Two smaller corrections belong in the same edit: both
  `why` texts quote the old `[S3 / "T026"]` and `[S3 / "T029"]` rows and are now false against
  `crates/phosphor-core/src/action.rs`, and the comment above the pair — *"The two below disagree
  with their own declared task"* — is no longer true of either.

- **R1 · The `Motion` vocabulary. CLOSED AS BUILT, and the open half is ruled.** R1 said `f` `F`
  `t` `T` `;` `,` and `W` `B` `E` were not expressible and that there was no case-change
  capability. **All of that is false against the tree**, checked this session:
  `wire_choice!(Motion …)` at `crates/phosphor-core/src/request.rs:669` carries
  `FindCharForward`, `FindCharBackward`, `TillCharForward`, `TillCharBackward`, `RepeatFind`,
  `RepeatFindReverse`, `BigWordForward`, `BigWordBackward` and `BigWordEnd`;
  `runtime/keymaps.scm:420`–`431` binds all nine; and `SetCase` is a capability
  (`crates/phosphor-core/src/action.rs:336`) bound at `keymaps.scm:463` (`gu`), `:464` (`gU`),
  `:529` and `:556` (`~`). R1's arithmetic — *"the vocabulary goes 208 → 209"* — is the tell: it
  already went, which is why `TASKS.md` read 209 when this was written. (It reads **212** now —
  the repair window between `CP-3` and `S4` added `set-macro-recording`, `register` and
  `place-anchor`. The count is recomputed by `scripts/doc_claims.py`, so it is not a claim
  anybody maintains by hand.)
  **The design question R1 was really asking is ruled, 2026-08-13: the character does not ride
  inside `Motion`.** A payload-carrying arm would make `ParamType::Choice` the wrong type for
  `motion` and break the CLI's flag value and the MCP schema's enum in one edit — all three
  doors at once. At the doors, find-char reaches the editor as `input/feed-keys`
  (`action.rs:459`); inside the machine the character rides *beside* the motion, the way
  `SelectObject`'s delimiter already does, and `gg`/`G` are the standing precedent for a
  machine-resolved absolute `set-cursor` (`action.rs:359`). The tree already argues the same
  thing in its own words at `request.rs:586`–`600`.

- **§15 · `s` — the mark-seen operator, or vim's substitute? RULED 2026-08-12: `s` stays vim's
  substitute.** Vim habits carry; the drawing is what changes. Mark-seen moved to **`gs`**, which
  takes an object (`gsib`). Built and verified against the tree at the `CP-3` re-audit:
  `runtime/keymaps.scm:525` binds `s` to `(key/fused "change" "char-right")` in normal and `:555`
  to `(key/operator "change")` in visual — unchanged — while `:475` adds
  `(key/operator "mark-seen")` on `gs`, decoded by a new arm in `crates/phosphor-steel/src/keymap.rs`.
  `crates/phosphor-steel/tests/shipped_grammar.rs:297`
  `mark_seen_is_gs_and_s_is_still_substitute` asserts both halves against the shipped layer, and
  `crates/phosphor-core/tests/agent_objects.rs:149` drives `gsib` to a clean no-op.
  **The consequence owed to the design docs is now recorded.** Mockup `6d`'s *"`s` composes like
  an operator"* is the sentence that loses, and `TUI Mockups.dc.html` is imported verbatim — Teej
  amends it at claude.ai. It is tabled in [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md) §5 as
  a `CP-3` amendment and appears in [README.md](README.md)'s prose list, which is the one a
  cold reader hits first. Teej also noted vim-surround (`cs"'`) as the shape `s` should stay compatible with;
  not built, not tasked, and a `v1.5` line rather than a task, since `cs` is `c` then a surround
  object over the operator machinery `T026` already has.
- **Would `ratatui-textarea` need a third vendored fork?** `SPIKES.md:292-293` names it and
  `nucleo` for `T045`'s Picker, neither is in `Cargo.toml`'s dependency table, and its predecessor
  `tui-textarea` is the crate whose ratatui-0.29 pin turned `ratatui-markdown` into a fork. Checked
  against the published manifest on 2026-08-12: **no fork needed.** `ratatui-textarea` 0.9.2 takes
  `ratatui-core` 0.1.1 (the workspace is on 0.1.2, compatible), its `ratatui-crossterm` dependency
  is optional behind a default feature that `default-features = false` drops, and its MSRV of
  1.86.0 is below the workspace floor of 1.88. It does pull `ratatui-widgets` 0.3.1
  non-optionally, adding one crate to the graph. `nucleo` 0.5.0 is MPL-2.0, which `deny.toml:54`
  already allows.
- **`surfaces.txt:221` carries `V15 v1.5 create-pane-from-view`, outside S1–S8.** Asked whether
  `v1.5` was a hole in the vocabulary test's task-column check. It is not: `v1.5` is an explicit
  exemption at `crates/phosphor-core/tests/vocabulary.rs:313`, and every other capability's task
  must exist in `TASKS.md` or the test fails.
