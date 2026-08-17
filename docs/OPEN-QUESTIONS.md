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

**Swept again the same day by the pre-`S4` scout**, which is a narrower pass: it asks only *what
must be true before the next window opens*, and it changed three things. `R6` is done and moved
to the list above — the fix it proposed would itself have shipped a list missing four names, so
the lint now derives that list instead of enumerating it. §26 is new, and it is the only finding
here that is specifically about running a **window** rather than about the product. And §10's
*"nothing is gained by starting it in `S3`"* has now expired by its own terms; it is `S4` work and
is called out as such below rather than left reading like a deferral.

**Then the scout's own list was worked, in the same window.** §8 and §14 are ruled and built,
`R15` is done, §26 is measured and fixed, and §20 is hardened without having been reproduced.
**Three of those five contradicted the entry that recorded them**, which is the argument for
running a register rather than reading it: §26's guess about where the time went was wrong by two
orders of magnitude, §14 turned out to be a live inconsistency rather than a forward-looking
trap, and `R15` said only Teej could do it when the API takes it. Every one of those corrections
came from executing the thing, and none from thinking harder about it.

**Swept again after `S4`, on 2026-08-14, and §10 makes the point a fourth time.** Four entries were
added — §27 through §30, in their own group below — and §10 was re-checked and found to be wrong in
three ways at once: the frame it calls blocked is built, it was looking in a file that could never
have held it, and the unblocking it expected from `T037` was never `T037`'s to give. Nothing in
that entry was a lie when it was written; it went wrong by standing still while the tree moved.
**Re-read an entry before quoting it**, and prefer the tree to the entry when they disagree — which
is `CLAUDE.md`'s rule, and is now the fourth consecutive sweep to earn it.

**And once more after `CP-4`'s manual half**, which added exactly one entry — §38, in its own
group at the end. The five tasks that half produced are work rather than questions and live in
[TASKS.md](TASKS.md)'s §`D`; what came here is the one thing among them that **two of those tasks
cannot decide between them**, which is the only shape that belongs in this file. The pass also
corrected the *"Five entries"* line on the group above, which had been six since §37 landed.

**Swept again after `CP-4`'s second sitting, 2026-08-16, and it added no entries and closed two.**
Teej ran the binary again after `T104`–`T107` and both things he reported were already questions
in this file rather than new ones: §38 is **re-ruled**, by the first option it had weighed and not
taken, and §29's item 3 is **ruled** — the judgement it said *"cannot be answered without typing"*
got typed. Both rulings stay here rather than moving to *Closed*, because §29 and §38 each carry
live residue their entries name. **The pattern is worth stating: a register earns its keep when
the next session's findings land in entries that already exist**, and this is the first sweep
where every finding did. It also corrects the claim §29 item 3 made about *why* there was no
timer — *"the loop blocks on `recv` and has no tick to hang one off"* — which was true when
written and is what changed.

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

**§8 is ruled and closed too**, by the pre-`S4` scout, taking its own recommendation: `path:line`
is a `Target` spelling now. **§14 went with it**, from *Raised by Window D's S3 run* below — the
two were one seam, and running §14 rather than reasoning about it found that the CLI door already
disagreed with itself.

*Nothing stands open in this category.*

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

**The frame exists. Neither gap closed. And the entry was looking in the wrong file — all three
checked against the tree on 2026-08-14.**

- **`crates/phosphor/tests/screen_6b.rs` is the `6b` golden frame**, at two widths
  (`screen_6b_draws` at 120 columns, `screen_6b_draws_at_80_columns`), with committed snapshots at
  `crates/phosphor/tests/snapshots/screen_6b__6b.snap` and `…__6b-80.snap`. It landed in `5017293`
  — the pre-`S4` repair window, one of *"the six things that had to be true before S4 opens"* —
  and `TASKS.md`'s header already records the missing `6b` snapshot as one of `CP-2`'s three gate
  findings. **`golden_frames.rs` still carries no `6b` and never will**, and the reason is
  structural rather than an omission — `screen_6b.rs`'s own module header, section *"Why it lives
  in the binary crate"*, is where it is written down. `6b` is composed by Steel and drawn by
  the interpreter, so a test of it needs `phosphor-steel` *and* `phosphor-ui` at once, and
  `scripts/lint-no-store-mutation.sh` check 2 allows `phosphor-ui` exactly one `phosphor-*`
  dependency. The binary crate is the only place both are visible — the same reason `parity.rs`
  lives there. **This entry named a file that could not have held the answer**, which is worth
  more than the correction: a blocker recorded against the wrong file reads as blocked long after
  it is built.
- **The `#1a201a` ground is still not drawn.** The committed 80-column snapshot's `bg` plane is
  empty on every row except the `REPL` badge on row 23, and its legend names five colours
  (`#9ec98c`, `#0c0f0c`, `#c6cec6`, `#9aa39a`, `#59635a`) — no ground among them. Unchanged, still
  `spine`'s, still a view-tree contract question.
- **`T037` did not bring the literals, and could never have.** The typed rows in that snapshot are
  one colour end to end (`h`, `neutrals.text`), not per-token. The blocker was never
  `define-language`: `runtime/languages/steel.scm` declares `"grammar" "scheme"` and
  `phosphor_buffer::grammar::BUNDLED` is ten names that **do not include `scheme`** — the vendored
  fork has no arm for it, its own manifest says adding one is `S4`'s job, and nobody added one.
  `steel` is `second-tier` by `Languages::tier` for exactly this reason, and its declaration file
  spends five paragraphs saying so. So a `.scm` buffer renders unhighlighted today, and **the
  remaining half of this entry is a fork grammar arm that no task in the graph names.**

*Recommendation, revised: close the frame half — it is built, and this entry should stop claiming
otherwise. Keep the ground question, which is unchanged and is `spine`'s. Re-file the literals
half: it is not `T037`'s and never was, and a grammar arm in the vendored fork is `surface`'s work
under `T083`'s subject matter with no task naming it. This is §30's pattern wearing different
clothes — a claim nothing recomputes, drifting quietly — except that here what drifted was the
blocker's attribution rather than a count, and no lint could have caught it.*

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

**A third conflict joined this category on 2026-08-14, and it is a different axis: §27 below.**
The two above are mockup-versus-mockup, where either rendering could be made true. §27 is a mockup
against Design Language §3's **prose**, which the build already implements — so unlike these, one
of the two sources is load-bearing in the tree today and the other is a picture. Same resolution,
same owner, and worth reading together: it is now three drawings in this build that disagree with
something, and none of them was found by a checkpoint. All three were found by an agent
transcribing a drawing into a fixture or a widget, which is the only activity that reads a mockup
closely enough.

**§14 is ruled and closed** — `phosphor --eval`'s exit code. It was recorded as a
forward-looking trap; running it found the CLI door's two routes already answering different
exit codes for the same refusal, which made the ruling a correction rather than a decision.

## Raised by the test-depth run

Property tests, fuzzing and benchmarks found these. None was known before; three are product
bugs, one is a design question with a number attached, and one is costing us signal right now.

**Worked in the window after the pre-`S4` scout. §21, §22 and §25 are fixed; §23 and §24 are
deliberately not, and the difference is worth stating.** Those two are *measurements*, not
defects: their own text says all three numbers are `T095`'s input, and `T095` — history
maintenance — is an unticked task in *A · Arms owed*. Acting on them now would be building
product work nothing has scheduled, on the strength of a benchmark. They stay here as the input
that task should start from rather than the assumption it would otherwise start from, which is
what they were collected for. `R3` in the repair list below is the same shape: a question about
how a composition should reach a capability for a node kind nothing composes, not a defect with a
fix waiting.

**Two of the three fixes were not where the entry said they were** — see §21 and §22, each of
which named a recommendation that would have changed correct code or saved nothing.

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

**Pre-`S4` scout — worked, and it stays open with its status changed rather than ticked. It
could not be reproduced.**

- **~400 executions, none failed.** 30 runs of the single test under 20 spinning processes on 10
  cores; 24 concurrent runs of the whole binary — four at a time, 16 pty children each — which is
  the shape of three agents running `just test` at once that the entry describes.
- **Two hypotheses were formed and measurement killed both.** First: that `Editor::open` waits for
  one frame while startup draws several, so the first `press` is satisfied by leftovers and no key
  is ever handled. Startup was measured drawing **exactly one** frame and holding at one after
  1.5 s idle. Second: that some key draws no frame, or two. Every key these tests press was
  measured at **exactly one** — `l`, `g`, `U`, `i`, `w`, `X`, `esc`, `:`, `w`, `\r`, `SPC`.
- **What was found instead is real, and is a hole of the right shape.** The harness's
  synchronisation was **one-sided**. Too *few* frames times out, loudly. Too *many* — one key
  drawing two — was invisible, and its consequence lands on the *next* `press`: `target` is
  computed from a counter the surplus already inflated, so that press returns without its keys
  having been handled, and the assertion at the end reads a buffer that never saw them. That
  produces a plausible mis-sequencing rather than a timeout, which is the symptom this entry
  records, and it is load-sensitive because whether the surplus lands before or after `press`
  returns is a scheduling question.
- **So the surplus is accounted for now.** Each press records what it waited for and the next one
  requires the counter to still be there. Proven on a planted off-by-one baseline; the failure
  names the unaccounted frame, the key being typed, and the last frame drawn. All 16 tests pass
  with it, which is itself the stronger statement the probes could not make: **no key in any of
  them draws a surplus frame.**

**Not asserted: that this was the cause.** It is a mechanism that produces the symptom, closed.
If the flake recurs it now says why instead of being a mystery a second time — and if it recurs
*without* tripping this assertion, that is a genuinely new fact and worth more than a fix would
have been. The second-order problem is untouched: `cargo llvm-cov --workspace` still cannot
complete, because that depends on the test never failing rather than on it failing legibly.

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

**FIXED — and the defect was not where this entry put it.** Both options above are about the
*splice*, and the splice was already correct. Measured through the shipping binary, `|` marking
where the cursor was left:

```text
gUiw  straße beta   ->  |STRASSE beta      correct, before and after
~     ßxy           ->  S|Sxy              the bug
~~    ßxy           ->  Ss|xy              its consequence
```

`gU` and `gu` were never affected, because an operator lands at the **start** of what it touched
(vim's `*operator-resulting-pos*`, which `Machine::land` already implements) — and a start does
not move when an end does. `~` is the exception: it is `g~` fused with `l`, so it *advances*, and
it advanced to `span.end` — a position measured on the text **before** the edit. `ß` upper-cases
to `SS`, so that column was the second `S`, and the cursor landed inside what it had just written.
`~~` then re-cased that `S` instead of moving on to `x`, which is where the `"ss"` in the
paragraph above came from.

The landing is computed from the *cased* length now (`fused_case_end` in
`crates/phosphor-core/src/input.rs`) and deliberately not clamped there, because the machine's
clamp runs against the pre-edit text — which is the very staleness at issue. The host clamps when
it converts the position, against the real buffer, which is the only place the post-edit line
length is known. Pinned at the pty by
`a_case_change_that_grows_leaves_the_cursor_past_what_it_wrote`, including the ASCII rows: a fix
that moved the cursor differently when nothing grew would break every `~` a person actually types.

*The lesson is the entry itself.* It reasoned from a true general fact — case conversion is not
length-preserving — to a specific caller that turned out to be innocent, and named two fixes that
would both have changed correct code. One run of the real binary found the actual line.

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

**FIXED, by none of the three, and the recommended one would not have helped.** A cache keyed on
`(width, revision)` pays off when a width *recurs*. Dragging 120 → 80 visits forty widths **once
each**, so every lookup is a miss and every rebuild still happens: the cache would have added a
map and saved nothing on the one scenario this entry is named after. It would help a drag that
returns to where it started, which is not the case that costs 5.7 seconds.

What was actually redundant is the *frames*, not the rebuilds. The loop reads `term.size()` fresh
each turn, so each queued resize costs one wrap and one draw for a width the user has already
dragged past. `coalesce_resizes` in `crates/phosphor/src/main.rs` drops the resizes that another
event is already sitting behind. There is **no timer and nothing waits** — the poll is
`Duration::ZERO`, so it reads only what is already queued, which is what separates this from the
debounce the entry rules out. Invariant 3 is untouched: you land at the size you asked for,
without the editor drawing the ones you dragged through.

**It is self-correcting, which is the property worth having.** Events queue only because the
rebuild is slower than the drag, so the bigger the buffer the more it skips, and on a buffer small
enough to wrap between two events it does nothing at all. The 41 ns/character is unchanged and
one rebuild at 3.3 MB still costs 138 ms; what is gone is paying it forty times to draw frames
nobody sees.

*A pty harness cannot test this* — the slave fd is moved into the child, so the test side has
nothing to resize, and Apple's master rejects `TIOCSWINSZ`. So the decision is split from the
terminal and tested against a queue: a drag collapses to the size it ended at, **a keystroke
behind a resize is never swallowed**, and a non-resize event does not cause a poll that could
consume the one behind it.

**Still open, and it is the real ceiling:** the rebuild is whole-buffer. Wrapping only the visible
window is `T081`'s core and the entry's second option, and this makes it less urgent rather than
unnecessary.

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

**FIXED, all three, and the diagnosis needed splitting in two.** Only the codec one was a
*generator* problem. The two `cased` properties had generators covering `ß`, `İ`, `ǰ` and emoji
already — what they lacked was an assertion that said which way the conversion went. Counting is
symmetric and so is idempotence, so between them they proved `gU` changed case without ever
saying *to what*.

- **`cased_never_loses_a_character`** now asserts direction outright: `Upper` leaves no lowercase
  character behind, `Lower` leaves no uppercase one. `Toggle` gets no such law, because producing
  both is its job.
- **`upper_and_lower_are_idempotent`** gained two. That the two are *different functions*, which
  idempotence alone does not say — and, because a **swap** leaves them different as well as
  idempotent, the one that actually catches the plant: the last one applied wins. `Lower` then
  `Upper` leaves no lowercase; `Upper` then `Lower` leaves no uppercase.
- **`every_record_round_trips_through_the_codec`** was the real generator gap. `Step::Commit`
  carried an edit *count*, and `records_from` turned it into `removed: String::new()` with
  `inserted: format!("x{index}")` — so every record ever generated was an insertion of one or two
  ASCII bytes, the varint length prefix never exceeded one byte, and the branch that handles a
  removal never ran. It carries the text now: empty strings stay in the domain, multi-byte
  characters exercise bytes-versus-characters, and a 40–90 character arm pushes the length prefix
  past 128 for the first time. It passes, which is the codec earning the property it had.

Verified by planting the swap the entry names. Before: only the hand-written
`cased_grows_on_a_sharp_s` failed. After: **all three fail**, and the two properties are no longer
carried by one example.

## Raised by the pre-`S4` scout

### 26 · One test is 96% of the test suite's wall clock

Measured in this worktree on 2026-08-13, on an otherwise idle `just gate`:
`phosphor::parity::every_capability_is_reachable_at_every_door` took **176.136 s** of a
**182.498 s** run over 682 tests. Every other test in the repository finishes inside the
remaining ~6 seconds. `nextest` reports it as the one `SLOW` test and crosses both its 60 s and
120 s thresholds on the way.

The shape explains the size, and the shape is deliberate. `Doors::open` boots one `Runtime`
(`crates/phosphor/tests/parity.rs`, `impl Doors`), and `steel_door` then builds a fresh source
string per capability and calls `Runtime::evaluate` on it — so the Steel third is one full
parse/compile/eval per capability, and the test as a whole is one check per capability per door —
`636` over `212` capabilities on the day this was measured. That is exactly the thing worth
having: it is the test that makes *one API, three doors* true rather than asserted.

> **Every count in this entry is the count of 2026-08-13**, deliberately, and written the way
> `scripts/doc_claims.py` section 5 spells a historical one so it is not recomputed against a
> vocabulary that has since grown. A timing divided by a live count would be a different
> measurement every window; the run it describes happened once.

**Why it is worth an entry now rather than whenever somebody notices.** `nextest` isolates tests
per process and can run 682 of them concurrently, but it cannot split one test *function* — so
this is a hard floor under `just gate` that no amount of parallelism removes. `S4` is a ~10-agent
window and the standing instruction is that every agent gates before it hands work back; that
floor gets paid once per agent. It is also the same CPU-contention window in which §20's flaky
`loop_pty` test is most likely to fire, and a three-minute gate is how a flake stops being cheap
to re-run.

**Not asserted:** that the Steel third is where the time goes. The shape says so and 176 s over
the `212` capabilities of that run is ~0.83 s each, which is consistent with parse-and-compile;
but nothing has profiled it, and the MCP and CLI thirds have not been timed separately. Do not
"optimise Steel" on the strength of this paragraph.

- **Split it per door.** Three test functions instead of one, and `nextest` runs them
  concurrently. It is already three independent assertions wearing one name — `check` is a
  `match` on `Door` — so it costs almost nothing and is the only option needing no measurement
  first. **But do not expect it to fix the wall clock.** A split bounds the run at the *largest*
  third, and if the guess above is right — that the Steel door owns nearly all of the 176 s —
  then the largest third is nearly 176 s and the speedup rounds to nothing. Its real value is
  that `nextest` then prints the three numbers, which is how the guess stops being one.
- **Hoist the compile** — bind each capability's call once and apply it per check, rather than
  evaluating a freshly built source string. Bigger win if the guess above is right, and worthless
  if it is wrong.
- **Leave it.** Defensible: it is one test, it is green, and it buys the invariant the whole
  registry design exists for.

*Recommendation: split it per door before `S4` opens — not as the fix, but because it is the
cheapest way to buy the measurement, and because the file is otherwise spoken for (see the note
on `T100` in [TASKS.md](TASKS.md), which is scheduled to have `parity.rs` to itself at the front
of Window E). Then decide with a number in hand. Hoisting the compile is the only option that
could actually move 176 s, and it is not worth writing until something has shown that the compile
is where the time goes.*

**Pre-`S4` scout — done, and the guess above was wrong by two orders of magnitude.** The split
landed and the three numbers came back on the first run:

| door  | time     | what its check does                              |
| ----- | -------- | ------------------------------------------------ |
| Steel | 1.19 s   | one parse/compile/eval per capability, in-process |
| MCP   | 1.14 s   | in-process                                        |
| CLI   | 157.62 s | **spawns the shipping binary, once per capability** |

Everything above hedged on the Steel door owning the time, because it is the one doing 212
parse-compile-evals. It owns **0.8%** of it. The CLI door owns the rest and for a reason nothing
in this entry considered: a door with an argv and an exit code is not a function you can call
in-process, so `cli_door` launches the binary, and each launch boots the Steel layer on the way
up. That is 212 process spawns, and it is structural rather than a defect.

**So the fix was not the split, and it was not hoisting the compile either.** The spawns are
independent — separate processes, null stdin, nothing shared, nothing written — so they run
across lanes instead of end to end. **157.6 s → 30.1 s**, no longer `nextest`'s slowest test, with
every capability still checked exactly as before.

Kept as a record rather than trimmed to the answer, because the wrong guess is the useful part:
every option above was ranked against a hypothesis with no measurement behind it, the hedge
(*"not asserted: that the Steel third is where the time goes"*) was the only thing that stopped
it becoming a wasted optimisation, and the cheapest possible experiment settled it in one run.

---

## Raised by the `S4` window

Four entries, from three commits (`b76b8ee`, `52183a6`, `0c12f68`) and the wiring pass that
followed them. Every citation below was checked against the tree on 2026-08-14, and the design-doc
ones against `docs/design/` in the same session. **One of them is a correction to the window's own
handoff report rather than a question** — §29's third item — and it is written up as a finding
because the report said the opposite of what the code says, which is the failure mode this whole
file exists against.

### 27 · A diagnostic on a row claude just wrote — `6c` and §3 draw it differently

The second finding of §12's kind, and a different axis: §12 is two *mockups* disagreeing with each
other, and this is a mockup disagreeing with the **prose spec**. Both resolve the same way, which
is why they are cross-referenced.

- **`Design Language.dc.html:67` (§3)** fixes the ladder: *"Column 1: the 1-cell state bar
  (unseen/diagnostic/none — priority: trouble &gt; attention &gt; claude)."*
- **`TUI Mockups.dc.html:523` (screen `6c`, line 64)** draws that exact overlap and draws it the
  other way. The row carries `■ E0308: expected Duration, found u128` and a `#d97b6c` wavy
  underline, and its state bar is `background:#3ddc97` — **claude green** — because it sits inside
  the region claude just wrote (lines 62 and 63 carry the same green bar, at `:520` and `:522`).

The prose and that drawing cannot both be right about one cell. §3's own render is silent on it:
its row 19, *"diagnostic region"*, carries the trouble bar but carries no unseen state, so it
never draws the overlap.

**The build follows the prose, and it was flagged rather than folded in — which is the rule
working.** `phosphor_ui::gutter::RegionState::mark` sends `Diagnostic` and `Failure` to
`StateMark::Trouble`, `state_column` folds each row's set with `raise`, and the higher rank wins;
`crates/phosphor-ui/src/diagnostics.rs` carries a header section titled *"Where 6c and §3 disagree,
and which one this follows"* that states the choice and its reason — the ladder is written
mechanically, `T031` already implements it, and the alternative reading is written down nowhere.

**A second, smaller question came out of the same task and belongs with it.** `T040` added
`RegionState::Warning`, because mapping every LSP warning onto trouble-red makes a file with
unused imports read as a file that does not build. It resolves to `StateMark::Attention`, and
§3 enumerates the column as *"unseen/diagnostic/none"* — no amber tier in the bar at all.
`gutter.rs`'s `Warning` doc **calls its own placement a reading rather than a transcription** and
records that no mockup draws an amber bar (`6c`, `1a`, `8c` and `9a`–`9c` draw claude-green and
trouble-red only; that enumeration is the `T040` agent's, quoted rather than re-derived here). The
tier is not new — `NeedsYou` already reaches it — so what is genuinely unwritten is a compiler
warning arriving in the cell that otherwise says *"claude is waiting on you"*.

*Recommendation: Teej rules both at claude.ai, on the drawings, not in the tree — there is no
build defect here and nothing to fix until the answer exists. If `6c` wins, the reading behind it
is that an **inline** `■` releases the bar, and note that the inline draw does not exist: a
`virtual_text::Row` is a row of its own (`VENDOR.md` patch 8) and end-of-line virtual text is a
fork patch nobody has written, so `6c` cannot currently be reproduced either way. If §3 wins, the
mockup is the bug and `6c`'s line 64 gets a trouble bar. The change to the tree is one arm of
`RegionState::mark` and a snapshot, in both directions.*

### 28 · The `Lsp` vocabulary could be asked and could not answer

Not a defect that shipped — it was found and fixed inside the window — but a fact about how the
vocabulary was designed, which is worth more than the three rows that fixed it.

At `0c12f68` — the last committed state before the wiring pass, read this session with
`git show HEAD:crates/phosphor-core/src/action.rs` — `Action::Lsp` had eleven verbs: five spelled
`request-`, one spelled `ingest-`, and five that do something else entirely (`move-`, `accept-`
and `cancel-completion` drive a list that is already up; `apply-workspace-edit` and
`restart-language-server` are neither questions nor answers).
**Three of the five requests had no verb their answer could arrive through.** The transport is
asynchronous by construction — `phosphor-buffer`'s `lsp::LanguageServers::look_up` answers on the
runtime thread, and the event queue's `Posted` carries an `Action` plus the name of the subsystem
that posted it and **no payload of its own** — so an answer needs a *verb*, exactly as an
unsolicited `publishDiagnostics` does. Completion, signature help and hover could be asked for and
could not come back.

**Why it went unnoticed is the interesting half.** `ingest-diagnostics` existed from the start,
because `publishDiagnostics` is *obviously* unsolicited and nothing about it looks like a function
call. The other three look exactly like function calls with return values — and there is no return
value anywhere in this design. So the asymmetry is not one row somebody forgot: it is what happens
when a request/response protocol is modelled as user intent and the answer's door is left implicit
because a synchronous editor would not need one.

**What was done.** `ingest-completions`, `ingest-signature-help` and `ingest-hover` were added
(`212` → `215` capabilities), each carrying the cursor the request was made at so a late answer is
dropped rather than drawn in the wrong place, and each answering *exactly once per request,
including the empty answer* — which is how a float that is already open closes. The durable half
is a test rather than the paragraph: `an_lsp_answer_is_exactly_as_open_as_the_request_it_answers`
pairs each answer with its request and asserts their MCP defaults move together, so relaxing an
ingest without relaxing the request it answers fails the build.

*Recommendation: nothing to rule for `S4` — it is built. The question worth putting in front of a
later window is whether **"every request verb declares the verb its answer arrives through"**
should be a property of the registry rather than three pairs someone remembered. The pairing test
enumerates those three by name and would not notice a fourth, and `S5`–`S8` add more asynchronous
sources than `S4` did (`T053` routes agent ingest; `T069` watches files). This is a `spine`
question about the capability table, not an `S4` question, and it costs a table property plus the
column it reads.*

### 29 · Four decisions `S4` made that a keymap could not make for itself

Grouped because they share a cause: **a binding is data**. It names a capability and its arguments
and cannot ask what the host is holding, so wherever vim's answer depends on host state, this
build had to pick something else and write down why. All four are in the tree and pressed by
tests; none is a defect; each is a place a vim user's hands will land differently, which is
`CP-4`'s subject.

1. **`<C-x>` opens the completion float; `<C-n>` only steps.** In vim `<C-n>` does both, because
   vim's keymap and vim's popup are one program. `runtime/keymaps.scm` argues it where it binds
   it: one key cannot mean two capabilities when a binding cannot ask whether a list is open.
   `<C-x>` is the prefix vim's own completion submode is spelled with, and mostly it is not
   pressed at all — the loop asks by itself whenever an insert-mode edit lands against a ready
   server (subject to item 3's throttle), so `<C-x>` is how you ask *again*, after `<C-e>` or on a
   line you have not typed into. **Closing this needs a role that reads host state, which is a
   change to the machine and not to the keymap.**
2. **`gcgc` comments a line; `gcc` does not.** Doubling an operator is a lookup in
   operator-pending — the rule that makes `dd` linewise — so the doubled form of a *two-key*
   operator is the two keys again, and `gcc` parses as `gc` followed by the `change` operator.
   vim-commentary users reach for `gcc`. Flagged, not fixed: a special case for it is a change to
   the input machine, which is `T026`'s.
3. **The typing trigger's throttle is one request in flight, not a timer — and this window's own
   handoff report said there was no throttle at all.** The tree is the authority and it disagrees:
   `crates/phosphor/src/main.rs` gates the trigger on `!outstanding.awaiting(Lookup::Completion)`
   and its comment there calls that *"the whole of the debounce"*, chosen over a timer because the
   loop blocks on `recv` and has no tick to hang one off. A burst of typing costs one round trip
   plus one, not one per character, and `a_burst_of_typing_never_says_the_editor_denied_something`
   is the test. **So the open question is not "add a debounce" — it is whether a throttle whose
   period is the server's round trip feels right**, which is exactly `CP-4`'s *"fast enough to be
   useful, or fast enough to be annoying"* and cannot be answered without typing.
4. **`accept-completion`'s `index` `0` means *whichever row is selected*.** The list is 1-based, so
   `0` named no row and was free; it had to be given a meaning, because a keymap that could only
   name a literal row would make `<C-y>` accept the same row forever. It is an invention rather
   than a transcription, and the parameter's own description in `action.rs` says so. Worth a look
   at `CP-4` on the same grounds as the others: it is a spelling a person will eventually read.

> **Item 1 was answered by `CP-4`, in a way this entry did not consider.** See §38: the mechanism
> that landed is neither a role that reads host state nor a new scope — `accept-completion` grew a
> fall-through argument, so the condition sits in the host and the alternative *text* sits in the
> keymap, and a binding stays a fixed list. The recommendation below is left as written because it
> was the reasoning available, and because the thing it got wrong is instructive: it framed the
> choice as *"may a binding ask a question"* when the answer was *"widen what the capability
> takes"*. Items 2 and 3 are untouched.
>
> **Item 1 went further at `CP-4`'s manual half, and item 3 is now ruled — by typing.** The
> fall-through argument widened again, from text to a capability (`move-completion`'s `otherwise`,
> a `Binding`), so `<tab>` steps the list and still indents when there is no list. §38 carries the
> ruling and the helix reading behind it.
>
> **Item 3's answer is *annoying*, and the reason was not the period — it was that there was no
> period.** This entry framed the question as *"whether a throttle whose period is the server's
> round trip feels right"*. Teej at the keyboard: *"completion seemed to take longer than it
> should have"*. Reproduced by a pty test that counts what reached the server
> (`a_burst_of_typing_asks_the_server_once_rather_than_once_per_character`): with one-in-flight as
> the only gate, nine characters typed together ask as fast as the round trip allows, every answer
> is about a prefix the cursor has already left, the `at` guard drops it, and the list **never
> catches up to the word** — the test fails on its 30s wait, not on its count. So the symptom was
> never latency in the ordinary sense; it was a list that was permanently stale.
>
> `COMPLETION_DEBOUNCE` is **250ms**, which is helix's `completion_timeout`
> (`helix-view/src/editor.rs`), taken as a measured default from a shipping editor rather than
> invented. The claim this entry made about *why* there was no timer — *"the loop blocks on `recv`
> and has no tick to hang one off"* — was true and is now false: `Queue::recv_until` takes a
> deadline, and the loop carries none while nothing is pending, so a quiet editor is still parked.
> `AppEvent::Woke` is the wake, which is the variant its own doc reserved for *"the elapsed
> tick"*. **`<C-x>` does not wait**, for the same reason it ignores the floor.

*Recommendation: none of these is a build change today. Items 1 and 4 are one question wearing two
hats — whether a binding may carry a **role** that reads host state — and answering it yes would
close both and cost a change to the input machine. Item 2 is a `T026` special case and small. Item
3 is a judgement and needs the manual half of `CP-4`. Rule item 1 first, because it is the only one
that changes what a keymap **is**.*

### 30 · Three counts drifted in one session, and `doc_claims.py` checks none of them

`scripts/doc_claims.py` recomputes task counts, wave widths, capability and parity counts, the
structural-lint count, the toolchain version and the `T0xx` references in Rust comments. It is the
reason six stale copies of `208` went red in one run. **It does not recompute the benchmark list,
the tool table, or the recipe list**, and all three moved this session:

1. **`docs/SPIKES.md`'s tooling table**, whose struck `divan` row said *"all four benchmarks are
   `harness = false` … four `[[bench]]` entries"* and whose `cargo-fuzz` row said *"Four
   targets"*. The tree has **six** `[[bench]]` entries and **five** fuzz targets: `csv.rs`,
   `diagnostics.rs` and `fuzz_targets/csv_parse.rs` all landed in `0c12f68`, this window's third
   commit. Fixed in this pass — that table is `docs/`. **It had been audited the previous day**
   (`fa28004`, *"the tooling table had drifted in both directions"*), and its own blockquote says
   *"nothing recomputes it"*. It then drifted again within twenty-four hours, which is the
   strongest evidence available that the audit is not the fix.
2. **The `justfile`'s `bench` recipe.** Its docstring reads *"Run the benchmarks (frame cache, VM
   invocations, journal, soft wrap)"* and the comment block above it names those same four files
   with a paragraph each. `cargo bench --workspace` runs all six. **This is what `just --list`
   prints**, so the four-item list is what every agent who follows `CLAUDE.md`'s *"`just --list` is
   the authority"* instruction reads. Not fixed here — the `justfile` is `harness`'s file, not
   `docs/`.
3. **`CLAUDE.md`'s own commands section**, which records that it *"was five recipes behind at the
   pre-`S4` audit"* — `vendor-check`, `vendor-pull`, `vendor-build-headless`, `tapes-diff` and
   `tape-diff`. That one is already fixed, and its fix was to add a sentence naming `just --list`
   as the authority. Which is the point: **the repair for a list nothing recomputes has twice been
   a sentence telling the reader to go look somewhere else.**

The shape is identical in all three: a *list* transcribed into prose, where the source of truth is
a directory or a manifest, and where adding an item to the tree is a normal thing to do and
updating every prose copy is not. Counts of *tasks* do not drift, because `doc_claims.py` fails on
them.

**And this is not the first time it has been noticed.** `docs/README.md`'s amendments section
already carries the same observation about itself — *"this list was two behind it until the `CP-3`
audit, which is its own small lesson about a list nothing recomputes"* — and the response then was
also an audit rather than a check. Four instances, and every response so far has been a person
reading carefully once. **The failure recurs on a schedule set by how often somebody adds a file;
the fix recurs on a schedule set by how often somebody audits.** Those two schedules are not
related, which is the whole argument for option (b) below.

*Recommendation: this is Teej's, because it is a change to how the team is gated rather than a
correction, and no agent should add a gate to `scripts/lint-*.sh` on its own initiative. The
options, cheapest first. **(a) Do nothing** — defensible; none of the three misled anybody into a
wrong decision, and a stale tool table costs a reader one `ls`. **(b) Extend `doc_claims.py`** with
a section that recomputes the benchmark count from `[[bench]]` entries, the fuzz-target count from
`fuzz/Cargo.toml`, and the recipe list from `just --list`, and fails on a prose copy that disagrees
— roughly the shape section 5 already has for capabilities, and it would have caught all three the
day they broke. **(c) Stop transcribing lists into prose at all** and make each of these a pointer,
which is what `CLAUDE.md` did by hand and costs nothing — but it also deletes the description that
made the list worth reading, and this repo's prose earns its keep by explaining why each item
exists. (b) is the recommendation; (c) is the one to take if (b) looks like more machinery than
the problem deserves.*

---

### 31 · One crate guards a P0 with eleven tests, and its real coverage is an accident

Counted from the tree on 2026-08-14, after `S4` closed: `#[test]` occurrences under `src/` and
`tests/`, `proptest! {` blocks anywhere in the crate, files under `benches/`, and targets in
`fuzz/fuzz_targets/` attributed to the crate they exercise.

| crate | unit | integ | prop | fuzz | bench |
|---|---|---|---|---|---|
| `phosphor` | 67 | 71 | 1 | — | 1 |
| `phosphor-core` | 113 | 124 | 5 | 3 | 1 |
| `phosphor-ui` | 235 | 27 | 2 | 1 | 3 |
| `phosphor-buffer` | 8 | 153 | 5 | 1 | 1 |
| `phosphor-steel` | 104 | 48 | **0** | — | **0** |
| `phosphor-term` | **11** | **0** | **0** | **0** | **0** |
| `phosphor-agent` | — | — | — | — | — |
| `phosphor-vcs` | — | — | — | — | — |

`phosphor-agent` (7 lines) and `phosphor-vcs` (6 lines) are placeholders for `S6`/`S7`. Zero is
correct there and they are listed only so the two blank rows are not read as a gap.

**`phosphor-term` is the finding.** 906 lines across `lib.rs` and `raw.rs`, eleven unit tests, and
nothing else — no integration test, no property, no fuzz target, no benchmark. It owns raw mode,
capability detection, and the synchronized-output wrapper, and Design Language §8 makes a torn
frame a **P0**.

**Its real assurance is accidental, which is the part worth acting on.**
`crates/phosphor/tests/loop_pty.rs` counts `\x1b[?2026l` closers to check **frame accounting** —
one frame per key. So the only thing in this repository that exercises synchronized output end to
end lives in a *different crate's* test asserting a *different property*, and it disappears
silently the day someone rewrites that harness for a reason unrelated to terminals. A test that
protects something it does not mention is not a test of that thing.

Four smaller gaps, in the order they would pay:

- **`phosphor-steel` has 152 tests and no properties.** Keymap resolution is property-shaped over
  a space no example set covers — 55 KB of `keymaps.scm`, prefix resolution, operator-pending,
  counts, registers. The law worth stating is *a prefix never resolves to something its extension
  does not, and resolution is deterministic under any generated sequence*, which is what `CP-2`'s
  liveness claim rests on.
- **Four unfuzzed decoders, all taking foreign input**: `registry::…::parse` (argv, from the CLI
  door), the `from_value` wire surface (~15 impls across `value.rs` and `request.rs`),
  `target_from_text` (the `path:line` spelling), and **LSP JSON-RPC framing** — the highest value
  of the four, because "truncated header, bad `Content-Length`, server closes mid-message" was a
  review finding *in this window* and it is input from a subprocess we do not control.
- **Three unbenched expensive paths**: completion float draw with a large list (the `S4` review
  found a real server filling 29 of 30 rows), document sync on rapid typing (`didChange` per
  keystroke is the cost `T038` chose), and keymap resolution per keystroke against that 55 KB file.
- **`phosphor-ui` is 235 unit / 27 integration**, inverted from every other crate. Widgets tested
  in isolation, few tested composed — the shape that produced this window's `T016`-class findings,
  where `lint-node-kinds` caught what the tests did not.

**Counts are not the measure, and this window is the proof.** `S4`'s review found **five tests
that could not fail** — a property oracle asserting the output of the function under test, an
acceptance test that survived both a planted Rust table and deleting the file it claimed to depend
on, a guard test carried entirely by a trailing newline in its fixture, and a tautological
`assert_eq!`. Every one of them is counted in the table above. So the table measures *surface*,
not assurance.

*Recommendation: take `phosphor-term` before Window E — that window adds four surfaces that draw,
and the crate that keeps them from tearing is the least tested thing in the workspace. Take the
LSP-framing fuzz target with it; it is cheap and the input is hostile. The rest can wait. And
whatever is done here must include an adversarial "does this test bite" sweep rather than only a
gap-filling sweep: a pass that raises the numbers without planting mutations would leave this
build more confident and no safer, which is the exact trade the five vacuous tests already made.*

---

## Raised by the repair window between `CP-4` and Window E

Five entries. The first is **a ruling recorded rather than a question asked** — the exception this
file otherwise refuses. It is here because the half that is still open is a drawing only Teej can
amend, and because `CLAUDE.md`'s rule is that a design conflict is flagged and never folded in:
`T101` changed the build in a direction `6b` draws the other way, so somebody has to be able to
find out why the tree and the mockup disagree without re-deriving the argument.

The second is `T100`'s neighbour: the task was scoped to the door's *voice*, and running it found
that one of the two sentences it was sent to fix is false for a reason no wording can repair.

The third and fourth were found by the review of that repair window, and both are about
`$XDG_CONFIG_HOME/phosphor/`: what happens when a user puts an `init.scm` there, and what happens
to the one form `7a` will write there. The fifth is what fixing the two unrunnable tapes revealed
about the other thirty-one.

> **Six, not five — §37 was appended here without the sentence above being touched.** Left as a
> correction rather than a rewrite, because the drift is the same one §30 is about and a count in
> prose that nothing recomputes is worth catching in the act. §37 is the statusline lying about
> the cursor after a jump, and it belongs to this window by when it was found rather than by
> subject.

### 32 · `6b` draws auto-persist, and `T101` removed it — ruled by Teej, 2026-08-14

**What `6b` draws.** Its fourth line, from `docs/design/TUI Mockups.dc.html` (screen `6b`, *"Steel
REPL"*), read this session:

> ```
> λ (keymap-set! "]r" (lambda () (goto (next-region-by claude))))
> ⇒ #ok · persisted to init.scm
> ```

A bare `(keymap-set! …)`, typed at the prompt, answering *persisted*. That is persistence **by head
name**: `runtime/repl.scm` listed eight heads and any form with one of them was written to disk for
having been evaluated. Try a theme, keep it forever.

**Why it was ruled out.** Teej's argument is Emacs, and it is that Emacs has two mechanisms and
phosphor had neither: `M-:` and `ielm` never persist — evaluating is evaluating — while
`M-x customize` is a deliberate *save this* UI that writes `custom-file`. Auto-persisting by head
is a third thing, and it sits badly against this build's third invariant, **nothing moves unless
you asked**: the user asked to evaluate.

**What the tree does now.** The mechanism is kept and the automatic is gone. `runtime/repl.scm`
defines `persist!`, an identity function that is a *mark* rather than a mechanism — the REPL is
still the only thing that writes, so a `(persist! …)` read back at boot evaluates its argument and
appends nothing. A bare config verb is answered `⇒ #ok · not persisted — (persist!
…) keeps it`, which is `6b`'s receipt offering the verb at the moment you would want it. Both
halves of *"a `persist!` verb, or `6b`'s receipt offering it, or both"* landed;
`the_repl_keeps_what_the_verb_marks_and_offers_the_rest` in `crates/phosphor/tests/loop_pty.rs`
drives all three cases through the shipping binary.

**`7a` is untouched and that was a constraint, not an accident.** `7a` draws `[2] always allow git
push` → *"writes `(allow "git push")` to init.scm"*, captioned *"always-allow writes a legible
rule"*. The user pressed a digit, so the act is already explicit and a permission grant has to
survive a restart. The gate is on the heads the layer *listed*; a head it never listed is written
as given, which is the call the permission surface will make when `T061` builds it
(`a_head_the_layer_never_offered_is_written_as_given`).

*What is still open, and it is Teej's:* `6b`'s fourth line. The tree and the drawing now disagree
about one row, and `docs/design/*.dc.html` round-trips to claude.ai — so the amendment happens
there, not here. Nothing in the build waits on it. **The same treatment §12 and §27 get**, and the
third entry in a row where the answer is *"rule it on the drawing"*.

### 33 · The CLI verb route is a second dispatcher, and it contradicts the door it lives in

**`T100` was sent to fix two sentences and could only fix one.** The task's brief named a live
example of the wrong voice, and it is real — verified against the built binary this session, after
`T100` landed and unchanged by it:

```text
phosphor set-case --target cursor --case upper
#refused · not built yet — T026 builds it            exit 1
```

`T026` is ticked, `Editing::act` has a live `Action::Buffer(BufferAction::SetCase)` arm, and the
keys work — `the_case_keys_edit_through_the_shipped_keymap` drives them through the shipped
keymap. So the door says a built, keyboard-reachable capability is unbuilt.
`scripts/lint-action-arms.sh` is satisfied, because `main.rs` *does* name the variant.

**Running the same capability both ways is what shows it is not a wording problem.** One binary,
one process, one absence of an editor, two answers:

```text
phosphor open-repl                   #refused · not built yet — T022 builds it   exit 1
phosphor --eval '(open-repl!)'       #ok                                         exit 0
```

The verb route says the capability is unbuilt. The eval route, in the same process, **carries it
out**. That is `§14`'s shape one level up: that one was two exit codes for one refusal; this is
*refused* against *done* for one capability.

**The cause, read this session.** `door.rs`'s `apply` is a `match` with three arms —
`Action::Runtime(Eval)` with a runtime, `Eval` without one, and *everything else* answering
`not_yet(action.spec().since.task)`. It is an `S2` stub: it predates `T022` wiring a real host in,
and `main.rs`'s `dispatch` now builds that host (`vm()`) on the verb path too and then never asks
it anything. The Steel door asks the same host and gets `AppHost::apply`, which carries out nine
capabilities and refuses the rest **by the same derived sentence** — so the falsehood exists on
both doors for those rows, in one place, and only the CLI door has a *second* place.

Counted, so the size is not an adjective: **55 action rows** whose declaring task is ticked *and*
whose variant the binary names are told *not built yet* by `phosphor <verb>` — 21 of them `T026`'s.

It read **56** until this repair pass recomputed it, and the missing one is worth naming rather
than quietly correcting: `Eval`. It satisfies the definition — ticked, and named by the binary —
but it is named in `door.rs` rather than in `main.rs`, and it is named there precisely *because*
`apply`'s first arm carries it out. So it is the one row of the 56 the verb route does not lie
about, and counting it made the sentence one larger than the defect it describes.

**Why this is not `T100`'s enum change, having considered making it one.** `Outcome::Raised` is a
missing case in *what happened*: the evaluation neither completed nor was declined. *"Built, but
this door has nothing to act on"* is a well-formed request the editor declined, which is precisely
what `Refusal` already means — a different enum. And no new `Refusal` variant repairs it either,
because **every sentence that arm could say is false for some row**: *"no session here"* is false
for the nine `AppHost` carries out, and *"not built yet"* is false for the 55 above. The behaviour
is the defect and the wording is downstream of it. Adding a case would have put a better-worded
lie in the same place, which is worse than leaving the honest-looking one visible.

**What the fix looks like, and the one thing that has to be ruled first.** Delete the second
dispatcher: route the verb path's `Action` to the host `main.rs` already built, exactly as the
Steel door does. Then `phosphor open-repl` answers `#ok`, `phosphor mark-seen` still answers *not
built yet — T041 builds it* (honest — `AppHost` genuinely does not implement it), and `set-case`'s
lie survives in **one** place instead of two, where it is the ordinary *arms-owed* debt
`lint-action-arms.sh` exists for rather than a door disagreeing with itself.

The thing to rule: `phosphor persist-form --form '(…)'` would then **write to the user's
`init.scm`** from a subprocess, and `crates/phosphor/tests/parity.rs`'s CLI walk runs every verb
with its canonical example — so a `just gate` would append `sample` to a real config home. That is
a side effect, a test-isolation question and a `T101` question at once, and it is why this was not
folded into a voice task. → `T103`.

There is also a **weakening of the parity walk** hiding behind the same sentence, worth fixing
whichever way `T103` goes: `cli_door`'s expectation is `#refused · not built yet — {task} builds
it`, and a task id is shared by many rows — 21 rows say `T026`. So a verb that dispatched to a
*neighbour with the same task* prints the identical line and passes.
`a_verb_that_answers_for_another_capability_is_caught` plants a fake task id and does not reach
this. The capability's own name is unique per row and is equally derived.

### 34 · A user's own `init.scm` replaces the shipped layer, and nothing says so

**Run this session, on the built binary.** A config home holding the one file
`phosphor_core::config`'s header used to draw — an `init.scm` with a single `(set-option!
"soft-wrap" #t)` in it — with no `$PHOSPHOR_RUNTIME` and a working directory outside the checkout:

```text
phosphor --eval '(length phosphor/boot-files)'
#raised · unbound identifier — Cannot reference an identifier before its definition: phosphor/boot-files

$PHOSPHOR_RUNTIME=<the shipped tree>, same expression
15
```

Fifteen shipped files against none. The review that found this drove the same config home through
a pty: an empty statusline, `:` drawing `┊ unknown key : — SPC opens the keymap`, and `ZQ` doing
nothing — the process had to be killed. **No boot float and no fault**, because `init.scm` ran its
one form cleanly, so nothing on screen says the product is missing.

**Why.** `Runtime::root` is a first-match-wins `find` over `$PHOSPHOR_RUNTIME`, the config home,
and `./runtime`. A config home containing an `init.scm` *is* the runtime tree, so it **replaces**
the shipped layer rather than loading after it. `runtime/README.md` has said so since `T101`
(*"candidate 2 replaces candidate 3 rather than layering over it"*, *"still open"*); the module
header in `config.rs` said the opposite — a three-layer stack with the config-home file drawn as
*"yours. hand-written."* on top of the shipped one — which invited exactly the file that bricks the
editor. **The header is corrected; the behaviour is not**, and correcting the behaviour is a
feature rather than a repair: it needs a load path, an order, and a ruling on whether a user's file
can *remove* a shipped binding as well as add one.

The Emacs argument `T101` was decided on has two halves and only one is built. *"Never writes into
its own source tree"* is genuinely achieved and pty-tested
(`the_repl_keeps_what_the_verb_marks_and_offers_the_rest` asserts
`!runtime.join("persisted.scm").exists()`). *"Shipped lisp plus a user `init.el`"* is approximated:
a user can only replace.

**The cheap half, if the full one waits.** A boot that picks candidate 2 could say so — one line on
the statusline, or a float naming which root was chosen — so that an editor with no keymaps is a
legible state rather than a mystery. That is a `T09x`-sized change and is not being folded into a
repair pass. → needs a task.

**RULED AND BUILT — Teej, 2026-08-14: layer it, Emacs's model.** Shipped lisp loads, then the
user's file runs on top; not Helix's replace-the-file model. The reproduction above, re-run
against the built binary from the same cold config home **with the working directory inside the
checkout**, so that `./runtime` answers and there is a shipped tree to layer over:

```text
$ cd <checkout>; XDG_CONFIG_HOME=<the cold config home> phosphor --eval '(length phosphor/boot-files)'
15
```

**The cwd clause is load-bearing and this block said the opposite for one revision.** It claimed
the same 15 *"from the same cold config home"* under the reproduction's own conditions — no
`$PHOSPHOR_RUNTIME` and a working directory outside the checkout — and that is still `#raised`,
because `Runtime::root` then answers `None` and there is no shipped tree anywhere on the machine.
Both halves measured this session against `target/release/phosphor`: cwd `/tmp/pref/cwd` →
`rc = 1`, `#raised · unbound identifier — Cannot reference an identifier before its definition:
phosphor/boot-files`; cwd the checkout → `rc = 0`, `15`. What layering fixed is that a config-home
`init.scm` no longer *displaces* a tree that exists. What no layering can fix is a machine with no
shipped tree on it at all, and that is what the disclosure half below is for.

Three files, in three call sites in `main.rs`'s `vm` — the shipped tree, then
`$XDG_CONFIG_HOME/phosphor/init.scm`, then that directory's `persisted.scm`. Call sites rather
than list positions for `T101`'s reason: a position in `phosphor/boot-files` is something a later
edit can reorder, and the first time this order *was* a list the rebind at the bottom of it came
back as a free-identifier fault a boot float found.

- **The mechanism is the one that already existed.** `Layer::load_persisted` ran a file of forms
  after `Runtime::boot` returned and merged its faults into the same `BootReport` the float draws;
  it is now `Layer::load_after_boot`, called twice, and the user's layer gets the whole of that
  behaviour — per-form isolation, faults in the boot's voice, and a place in the float — for free.
- **`Runtime::root` has two candidates now**, `$PHOSPHOR_RUNTIME` and `./runtime`. The config home
  is not one, and *being a candidate* was the defect rather than being second: a first-match-wins
  search cannot express *"and also"*.
- **Why the persisted layer beats the hand-written one.** A form you deliberately kept at the REPL
  is the later act and the more explicit one; the other order would make `persist!` unable to
  change anything you had ever written down. `the_persisted_layer_runs_after_the_users_own_file`
  is that order, and swapping the two `if let` blocks in `stack` reddens it — checked by doing it,
  `1 test run: 0 passed, 1 failed`. That sentence said `vm` for one revision and was false: the
  test reached the stack through `booted_with_config`, a hand-maintained second copy of `vm`'s
  four calls, so the mutation had to be made twice to be seen and a review made it once and
  watched 187 tests pass. `vm` is now two environment reads and a call to `stack`, which is the
  only copy of the order; `booted_with_config` is a name for `stack`.
- **The direction question is settled by the ruling, with no new verb.** A user's file may remove
  a shipped binding: `keymap-remove!` is defined in `runtime/keymaps.scm` and is already in
  `repl.scm`'s persistable set, and it reaches the shipped table only because the shipped table
  loaded first (`a_user_init_scm_overrides_one_shipped_binding_and_removes_another`).
- **`$PHOSPHOR_RUNTIME` still replaces**, deliberately: it is how the pty harness and every tape
  point the binary at a scratch tree, and the 54 `Editor::open` call sites in
  `crates/phosphor/tests/loop_pty.rs` — `grep -c "Editor::open("`, this session — depend on it.
  (55 is what you get from `grep -o "Editor::open"`, 56, minus the one `[Editor::open]` doc link;
  it also counts the single `Editor::open_forced(` site.) A user layer
  still loads on top of whatever it named — the same rule `T101` already gave `persisted.scm`,
  *"whichever tree booted"*.

**The disclosure half, and it is narrower than the entry asked for.** With layering in place the
common case stops being silent — a config-home `init.scm` no longer costs the keymap, and one that
throws reaches the float like any other fault. What was left was the state with nothing to layer
over: an installed binary run from outside its checkout with no `$PHOSPHOR_RUNTIME`, which said
nothing at all, because a boot that read no files has no faults to report. That now opens the boot
float on `init.scm · no editor layer` with
`nothing loaded — write ~/.config/phosphor/init.scm, or set $PHOSPHOR_RUNTIME` underneath it
(`Layer::note_if_no_layer`; `an_editor_that_loaded_no_layer_at_all_says_so_in_the_float` for the
sentence and `driven::a_boot_that_found_no_layer_says_so_on_the_first_frame` for the call site,
which is the half that would otherwise have gone unwired without anyone noticing — its symptom is
silence). The path goes in the message and not in the `file`, because the float does not wrap and
a long one pushed the label off the row: seen on a pty before it was seen anywhere else.
Deliberately **not** drawn: which root was chosen on a boot that succeeded. Every pty test and
every tape sets `$PHOSPHOR_RUNTIME`, so a line saying so would be permanently on screen in the
capture library and would teach nobody anything.

**The first guard for it was silent for exactly this entry's population**, which is worth recording
because it is the second time the same conflation cost the same thing. It read
`report.units.is_empty()` — *"nothing has loaded"* — and a user's own `init.scm` is a unit. So an
installed binary outside a checkout, no `$PHOSPHOR_RUNTIME`, config home holding §34's own one-line
`(set-option! "soft-wrap" #t)`, reproduced this entry's opening measurement verbatim and still said
nothing: driven on a pty, soft-wrap applied, no statusline row, no float, `SPC` answered
`┊ unknown key <space>`, `ZQ` did nothing, the process was killed. **Writing the file the float
tells you to write was what turned the float off.** An empty `init.scm` did it too, on the same arm:
`Layer::load_after_boot` records a unit for any file that reads, form or no form. The guard is
`Layer::has_editor_layer` now — the count of files the *boot* loaded, taken before anything can
stack on top — and the message has a second arm, because *write the file* is not advice for
somebody who wrote it: `your init.scm ran over nothing — set $PHOSPHOR_RUNTIME to a layer`.
`driven::writing_the_file_the_float_asks_for_does_not_buy_silence` is the pty half and
`a_user_init_scm_with_nothing_under_it_is_still_an_editor_with_no_layer` /
`an_empty_user_init_scm_does_not_buy_silence` are the unit halves; restoring the old guard reddens
all three.

**The footer had the same defect and it is now `phosphor_steel::float::ExLine`.** The float taught
`:repl open the repl · :reload-runtime run the boot again · esc close`, and in the one state this
float is guaranteed to open in, two of those three cannot be typed: `:` is bound in
`runtime/keymaps.scm`, which is precisely the file that did not run. Driven on a pty: `esc` closed
the float (Rust handles it), pressing `:` changed nothing on the frame. A boot with no editor layer
now gets a footer of `esc` alone, and the `n more · :repl to read them` overflow row loses its
instruction for the same reason. §4's *"every legal key, always visible"* is satisfied by the
reading that matters — **legal**.

**The capture library was reading the operator's config, and `$PHOSPHOR_RUNTIME` is exactly why
that was not obvious.** The override shields a tape from `./runtime`; it does not shield one from
layer 2, because layer 2 runs *after* whatever the override named. Every tape set
`PHOSPHOR_RUNTIME=../runtime` and nothing else, so `just tapes` recorded whoever ran it — and
`CP-4` reads that library as a change detector. Measured with a probe tape and the release binary:
an `$XDG_CONFIG_HOME` holding `phosphor/init.scm` = `(no-such-verb 1)` puts a boot float on a
capture that has none without it. `tapes/tape-env.sh` exports a scratch config home and the three
paths that run `vhs` source it (`run-tapes.sh`, `diff-tapes.sh`, `record-one.sh` behind
`just tape <id>`); with it, the same environment captures byte-identical to a clean one. It could
not go in `_config.tape`, which holds only `Set` lines so that a bare `Source` does not close vhs's
before-first-command window, and vhs 0.11 has no `Env` command. `tapes/README.md` carries the
writeup.

**Two limits, recorded rather than discovered later.**

- **A user's layer is one file.** Rust reads `phosphor/boot-files` once, after `init.scm`, and a
  second `define` of that global in a user's file would either name the shipped fifteen — which do
  not exist in their config home, so fifteen `unreadable` faults every start — or make them
  restate the whole list to add one name. A second file of their own wants a name of its own,
  which is a vocabulary question rather than a load-path one. → not filed as a task; nobody has
  asked for it.
- **`phosphor/persist-file` is read from the shipped layer only.** `main.rs`'s `boot` reads it,
  `phosphor/persist-verb` and `phosphor/offered-heads` off the VM immediately after
  `Runtime::boot` — before layer 2 runs — because the host is behind the barrier and may not
  re-enter the VM when a form arrives. So a user redefining it in their own `init.scm` has no
  effect, silently. Nothing writes that name today outside `runtime/repl.scm`.

**One thing §34 measured is still true and is nobody's yet:** an editor with no layer cannot be
quit. `ZQ` is `runtime/keymaps.scm`'s, the seed table is empty by construction
(`no_bindings_in_rust.rs`), and the boot float teaches `esc` — so the float now says *why* the
editor is inert, and killing it is still the way out. A rescue binding in Rust would be a
deliberate exception to *"Rust holds no copy of the table"* and is a ruling, not a repair.
→ needs a ruling if anyone wants it.

### 35 · `7a`'s always-allow writes a form that faults on the next boot until `T061`

**The write half is intact and tested.** `a_head_the_layer_never_offered_is_written_as_given` puts
`(allow "git push")` through `persist-form!` with the shipped policy loaded and reads the file
back: the form is written as given, ungated, which is what `7a`'s *"pressed a digit"* earns.

**The read half faults.** Run this session against the shipped layer, with the form sitting in a
config home's `persisted.scm`:

```text
phosphor --eval '(allow "git push")'
#raised · unbound identifier — Cannot reference an identifier before its definition: allow
```

`allow` is a free identifier until `T061` builds `runtime/permissions.scm`, and
`Layer::load_persisted` runs each form and records a fault the boot float draws
(`a_broken_persisted_form_costs_one_line_and_reaches_the_boot_float`). So the grant persists, and
then opens a boot float on every start.

**Nothing writes that form today** — the permission surface is `T061` and unbuilt, so this is
forward-looking rather than live. It is recorded because `T101` ticked `7a`'s clause without
naming it, and because the answer is a constraint on `T061` rather than a defect in `T101`:
whatever `runtime/permissions.scm` defines has to load **before** the persisted layer, which
`Layer::load_persisted`'s *"last, after the whole load order"* already guarantees for anything in
`phosphor/boot-files`. The thing to check when `T061` lands is that `allow` is defined there and
not somewhere the boot reaches later. → `T061`.

### 36 · The whole capture library's references are a window or three old

**What running it says now.** With `6b.tape` and `repl-liveness.tape` repaired and
`run-tapes.sh` collecting failures instead of stopping at the first, a full
`tapes/diff-tapes.sh` this session captured **every tape with zero capture failures** and reported:

```text
diff-tapes.sh: 8 frames matched, 33 mismatched, 0 screens skipped
```

So the tapes are healthy and the *references* are stale. `git log -1` on the drifted PNGs, read
this session: `broken-init.png` last written at `9a5c0e3` (S2), and `8c.png`, `9c.png`,
`theme-catppuccin.png`, `sweep-80.png` and `1a.png` at `e702d8a` (Window B). Three windows of
UI have landed since — the statusline, the gutter, the floats, S4's whole surface — so a mismatch
is the expected answer and not a regression signal.

**Why that is a finding rather than a chore.** A change detector whose baseline predates three
windows detects nothing: everything is red, so nothing stands out. That is the same failure mode
`V007` was written for one layer down — it could not see per-frame tapes, so six of them reported
*"no reference yet"* forever.

**Two frames were blessed here and thirty-one were not, deliberately.** `6b` and
`repl-liveness`'s four frames are the ones this pass *caused* to change — their tapes could not be
captured at all before it, so their references were unreproducible by construction, and all five
were looked at as images before being kept. The rest were restored with `git checkout`: blessing a
screen is a review act, `CP-4` is not passed, and thirty-one frames nobody has looked at is exactly
the kind of bulk approval a golden library dies of. → Teej, at `CP-4` or the window after it.

**Until then Tier 2 is dark, and saying so is the point of this paragraph.** Thirty-three of
forty-one frames mismatch, so the runner reports red for everything and therefore distinguishes
nothing — a change detector at 80% false positive is not a weaker signal, it is no signal, and
the danger is that a real regression arrives inside that noise and reads as more of it. `V008`'s
design already stops it costing anything (Tier 2 is `continue-on-error` and never gates), so the
cost is entirely that a reader may take a red Tier-2 job as information. It is not, today.

**The number is also the reason the obvious fix is wrong.** Thirty-three frames is more than
anyone reviews carefully in one sitting, so "bless them at `CP-4`" risks becoming exactly the
bulk approval this entry refused, with extra ceremony. Two honest shapes, and the choice is
Teej's:

- **By screen group, across several sittings** — the statusline screens, then the gutter, then
  the floats, then `S4`'s surface. Each group is small enough to actually look at, and a group
  that goes green stops contributing noise to the next.
- **Declare the baseline abandoned and re-capture wholesale at `CP-4`**, on the argument that a
  frame nobody has compared since Window B is not a baseline but a historical artifact. Cheaper
  and honest, and it forfeits the one thing a golden library is for — so it is only right if the
  intervening three windows are believed on other evidence.

*Recommendation: by screen group, starting with whatever `CP-4` makes Teej look at anyway. And
whichever is chosen, do not run `just tapes` to make the red go away: that is the one-command
version of the bulk approval, and it is indistinguishable afterwards from having reviewed them.*

---

### 37 · The statusline can say `1:1` while the cursor is on line 2

**Found sideways, and recorded late.** Chasing a flaky pty test in the `S4` window, a wait was
written on the statusline redrawing the cursor position after a `gd` jump. It hung for the
harness's full 30 seconds. Reading the session transcript back, `1:1` is the **only** position
the statusline drew in a run where the cursor demonstrably ended on line 2 — the `x` that
followed deleted a character from that line, and the file it wrote proves where the cursor was.

**The cause is a cache whose key does not include the cursor.** The buffer is drawn live from
`&Editor` every frame; `status_cache` is the statusline's alone, which an earlier reviewer had
already established while ruling out a different hypothesis. A jump moves the cursor without
touching anything the key covers, so the cached row survives and the position on it is stale.

**This is a product defect rather than a harness quirk, and that distinction is why this entry
exists.** A person reads the position to know where they are; one that lies after a jump is
worse than one that is absent, and `gd` is the motion most likely to leave you unsure. It was
written into a comment in `crates/phosphor/tests/loop_pty.rs` and nowhere else — which is how
the four register entries that turned out to be wrong about their own cause got that way. A
finding that lives in one file's comment is a finding nobody audits.

**Not asserted: the extent.** Only `gd` was observed. Whether every cursor motion is affected, or
only those arriving through the event queue rather than in a keystroke's own frame, has not been
measured — and the difference decides what this is: the second would make it a symptom of the
queue rather than of the cache. **Measure before fixing.** A cache key widened on a guess is how
a frame budget goes quietly, and `T079` exists because that budget is worth something.

*Recommendation: it needs a task, and it belongs with whoever next owns the statusline —
`T086`'s neighbourhood. The cheap experiment first, and it is two runs: press `j`, read the
position, then `gd`, read it again, on the same buffer. That settles which of the two causes it
is before a line of it is written.*

---

## Raised by `CP-4`'s manual half

One entry. The five tasks that half produced are in [TASKS.md](TASKS.md)'s §`D`; this is the one
thing among them that is a **question** rather than work, because two of those tasks want the same
key and neither may decide it alone.

### 38 · Two tasks want `<tab>`, and a binding cannot ask which

`T104` wants `<tab>` in insert mode to insert one indent level — that is the whole of the report
*"tab only seems to go a space at a time when indenting"*. `T105` wants `<tab>` to take the
selected completion, because that is the key every completion UI a person has used answers to and
because `7c`'s no-footer exception means the float can only be driven by keys your hands already
know. **The same key, the same scope, and the right answer depends on whether a list is open.**

**Why it cannot be settled in `runtime/keymaps.scm`.** Read this session:
`phosphor_core::input::table::Role`'s richest case is `Run(Vec<Action>)` — a fixed list of
capabilities with their arguments baked in, and nothing that reads host state — and `Scope` is a
five-value Rust enum (`Normal`, `Insert`, `Visual`, `OperatorPending`, `Object`) derived from the
edit mode by `Scope::of`. The thing that knows a list is open is `Editing::completion` in
`crates/phosphor/src/main.rs`, and no binding can ask it. So a keymap can give `<tab>` one meaning
and only one.

**This is §29 item 1 arriving a second time, from the other side.** That entry recorded `<C-x>`
having to open the float because `<C-n>` could not both open and step, and its recommendation was
*"rule item 1 first, because it is the only one that changes what a keymap **is**"*. Nothing ruled
it, and the same wall is now load-bearing for a key a user pressed rather than for a divergence a
reader might notice. `<CR>` is in exactly the same position — `Machine::insert_key` gives it
`"\n"` — and so is every arrow key, which `runtime/keymaps.scm` binds in the `insert` scope to the
`line-up`/`line-down` motions.

**`T105`'s answer arrived while this was being written, and it changes the question rather than
closing it.** The working tree carried it uncommitted, read this session:
`accept-completion` (`crates/phosphor-core/src/action.rs`) grew two optional arguments —
`then`, *"text to type after the accepted item"*, and `otherwise`, *"text to type when no row has
been chosen; present is what makes a key fall through instead of accepting"* — and
`Editing::accept` (`crates/phosphor/src/main.rs`) reads a new `Editing::chosen` field, written only
by the `move-completion` arm. **Neither obvious mechanism was needed**: not a sixth `Scope`, not a
conditional `Role`. The condition lives in the host, where the state is; the fall-through *text*
lives in the keymap, where the key's meaning is; and a binding stays a fixed list of capabilities
with their arguments. That is the cleanest form of *"the keymap is data"* this build has produced,
and it settles `<space>` and `<CR>` outright.

**It does not settle `<tab>`, and the reason is exact.** `otherwise` is an `Option<String>` — a
*literal* — so `<tab>` can be bound as *accept if steered, otherwise type this text*. `T104`'s
`<tab>` does not want text. It wants **one indent level**, which is a per-language value the
keymap cannot name: today it is `utils::indent(&self.lang)` inside the vendored fork, and after
`T104` it is whatever `set-option!` or `define-language!` holds. A keymap that spelled it as a
literal would be four spaces frozen into `runtime/keymaps.scm` for every language, which is the
Rust-table-in-scheme shape `T033` exists to forbid.

**So the residue is one question, and it is narrow.** Either

- **`otherwise` widens** from text to *a capability to run instead* — one argument becoming a
  nested Action, which makes the fall-through general (any key, any alternative) and is a change
  to how one capability's arguments are shaped rather than to the input machine; or
- **the vocabulary gains an insert-mode indent** — a verb meaning *"one indent level here"*,
  which `<tab>` names as its `otherwise` in whatever form that argument ends up taking, and which
  `T104` needs a home for anyway once the unit stops being the fork's; or
- **`<tab>` is given to one of them and the other gets a different key.** No mechanism at all, and
  legitimate — several editors do exactly this — but it must be *chosen*, because the default
  today is that `<tab>` types a character that renders in one cell and neither task's user gets
  what they asked for.

*Recommendation: rule the residue before `T104` is scheduled, not before `T105` — `T105` is
unblocked and `<space>`/`<CR>` do not touch this. The first option is the one to weigh first: a
nested Action in `otherwise` would also serve `<CR>` in the Picker (`T045`) and the prompt
(`T058`), which are the same shape arriving later, so the cost amortises across three surfaces
instead of being paid for one key. And whichever way it goes, `T104` and `T105` cannot run
concurrently in one window under [TEAM.md](TEAM.md)'s rule 1: they write the same files.*

**RULED by the third option, in `T104`, 2026-08-15.** `<tab>` in the insert scope runs
`insert-indent` (`runtime/keymaps.scm`), and completion keeps `<C-y>`, `<space>` and `<CR>`.
`T105` had already landed by then and had **not** taken `<tab>`, so nothing was taken away from
it: the two keys `CP-4` asked for are bound, `<C-y>` is vim's own, and indenting had no key at
all. The recommendation above says to weigh the first option first, and it was weighed and not
taken — widening `otherwise` from `Option<String>` to a nested Action is a change to how a
capability's arguments are *shaped*, and shaping it for a key that already has three alternatives
is paying the cost before the surfaces that amortise it (`T045`, `T058`) exist to share it.

**The residue is smaller than it was, and it is worth stating what changed.** §38's second option
— *"the vocabulary gains an insert-mode indent, a verb meaning 'one indent level here'"* — **has
happened**: `Buffer::InsertIndent` (`insert-indent`) is a declared capability with no arguments,
because the width it types comes from `set-option!` and `define-language!` rather than from its
caller. So the day somebody wants tab-to-accept, the fall-through has a verb to name and the only
open piece is the argument's *type*. Before `T104` there was nothing to name.

**What is still true and still unruled** is §29 item 1's underlying question, which this did not
answer: a keymap remains data that cannot read host state, and every conditional key in this
build is a condition in the host with its text in the binding. That shape has now settled three
keys (`<space>`, `<CR>`, and `<C-y>` by passing neither argument) and refused a fourth. If a
fourth arrives — the Picker's `<CR>`, the prompt's — the argument for widening `otherwise` gets
its second and third surfaces and should be re-weighed then.

**RE-RULED by the first option, 2026-08-16, and the fourth surface was Teej's hands.** The
paragraph above says to re-weigh when a fourth key arrives and names two that had not been built
yet; what actually arrived was the *same* key, from the person the third option had told to use a
different one. Running the shipped binary at `CP-4`'s manual half: *"in this form i should be able
to hit tab or something to select"*, and — on the same float — *"enter or space doesnt accept"*.

**Both halves are one fact, and it is the guard working exactly as designed.** Nothing had been
chosen, so `select = false` held and `<space>` and `<CR>` correctly fell through. The keys were
not broken; there was no comfortable key to *choose* with. `<C-n>` was the only one, and it is not
where a hand goes first — which is `7c`'s no-footer exception in as many words.

**Helix is the prior art, read this session rather than recalled.** `helix-term/src/ui/menu.rs`
binds `Tab`, `Down` and `C-n` to the same `move_down()`, and `Menu::cursor` is an
`Option<usize>` starting at `None`, so `move_down` lands on row 0 — **the first `Tab` selects,
it does not accept**, and `Enter` (which accepts only when `selection()` is `Some`, else returns
`Ignored(close_fn)` and lets the newline through) then takes it. Helix is `select = false` too;
what it has that this build did not is `Tab` on the stepper. Its `smart-tab.supersede-menu`
defaults to `#false`, which is the same precedence: the menu gets the key while it is open.

So `otherwise` widened after all, and on `move-completion` rather than on `accept-completion`:

- **It is a [`Binding`], not a new nested-Action type.** `request.rs` already answers *"a
  capability to run, as data, across three doors"* — `Binding::Capability { name, args }` — and a
  parallel type would be a second answer to that question plus a second `ParamType::Any` site.
- **`accept-completion`'s `otherwise` stays text**, and the split is not an inconsistency: what
  `<space>` falls through to is *what the key would have typed*, which is text a keymap can spell;
  what `<tab>` falls through to is one indent level, which is a per-language value from
  `set-option!` and `define-language!` that a keymap spelling as four spaces would freeze.
- **`Binding::Source` is representable and refused**, with a sentence. Scheme needs the VM and
  this runs inside `Editing::act`, which holds none; a binding that wants to evaluate source has
  `keymap-set!` already.
- **One level of fall-through**, guarded by `Editing::falling_through` — a finite `Binding` tree
  terminates on its own, but the depth would be whatever a `keymaps.scm` wrote, on the stack.

`<tab>` steps forwards, `<S-tab>` backwards with no `otherwise` at all, and with no list open
`<tab>` is `insert-indent` exactly as `T104` left it. Proven by a keymap-level test that presses
the key (`phosphor-steel`'s `shipped_grammar.rs`) and two pty tests that run it in the binary,
one for each state — the both-states rule `T105`'s *done when* already set. The third option's
own sentence is what expired: it said *"it must be chosen, because the default today is that
`<tab>` types a character that renders in one cell and neither task's user gets what they asked
for"*. It was chosen, and then the user said which one he wanted.

---

### 39 · The statusline's diagnostic count is `■ 1` and the `Counter` node cannot spell it

**Raised by building `2b`'s count**, which `CP-4`'s second sitting found had never been built at
all — see `TASKS.md` §`D`. The count is now composed, and it draws `■1` where the mockup draws
`■ 1`. One space, and it is a real one: read out of the raw HTML this session,
`docs/design/TUI Mockups.dc.html` carries `<span style="color:#d97b6c;padding-right:12px;">■ 1</span>`.

**The node has two renderings and neither is that one.** `Node::Counter` (`view.rs`) is *"a glyph
and a number, with an optional word"*, and `interpret.rs` draws it as `{count} {word}` with a
label and `{glyph}{count}` without. The second is what `●6` needs — checked against the same file,
which spells the unseen counter `●6` with **no** space, seventeen times. So the two are not one
rendering used twice: `●6` is §11's *contracted* form and `■ 1` is a full-width form that happens
to use a glyph instead of a word, and the node has no case for it.

**Not folded in, per `CLAUDE.md`.** Adding the space to the no-label arm would respell `●6` as
`● 6` in every frame that draws it; a third rendering means a new field on a node kind, which
moves counts `lint-doc-claims.py` recomputes and touches the node-kinds lint — a large change to
buy one space. The build ships `■1`.

*Recommendation: rule whether the space matters before spending a node field on it. If it does,
the cheapest honest shape is a third arm keyed on the label being `Some("")` rather than a new
field — but that is a spelling trick and should be weighed against just amending `2b`. Nothing
downstream depends on the answer, which is why this is a question rather than a task.*

---

### 40 · Twenty-one tape references photograph the live source tree, so they can never match

**Raised by `CP-4`'s first full `tapes-diff` run, 2026-08-16** — the first one this repository has
ever completed, because the version gate had been refusing it (fixed in the same change; see
`tapes/check-versions.sh`'s header). Result: **9 frames matched, 30 mismatched, 2 captures
failed.**

The 30 are three different things and only one of them is a defect:

- **9 frames changed for a real, explained reason.** `folds-*` (3) and `insert-whitespace-marks-*`
  (2) differ by exactly `152.962` px each — read off the diff images, that is the statusline
  gaining `rust-analyzer ✓`, which is `S4` landing against references captured before it.
  `7c-*` (3) differ by `T106`'s kind and source columns. `diagnostics` (1) differs by the cursor
  moving to the line the publish is about and the statusline gaining `■1`. Each of these is a
  reviewed reference update waiting for an eye — **`7c`'s three are Teej's `T106` ruling and must
  not be blessed before it**, since blessing them folds in a design change.
- **21 frames cannot match and never could.** `1a`, `1a-degraded-*`, `3c-*`, `8c`, `8d`, `8e-*`,
  `9c`, `broken-init`, `sweep-40/60/100/120/200` and all six `theme-*` open a file under
  `../crates/` — **the live source tree** — and screenshot it. `tapes/artifacts/1a.png` was
  captured at `e702d8a` on 2026-08-12; `crates/phosphor-core/src/lib.rs`, the file it
  photographs, has changed in three commits since (`S2`, `S3`, `S4`). The reference is a
  photograph of a moving target, so `tapes-diff` reports a mismatch for a reason that has nothing
  to do with drawing, forever.

**The convention to fix it already exists and these tapes predate it.** `tapes/fixtures/` holds
five frozen files (`call.rs`, `fetch.py`, `fetch.rs`, `fetch.ts`, `policy.rs`) and the `S4` tapes
use them — which is exactly why `signature-help-*` and `diagnostics-undercurl` match today.
**25 tapes reference `../crates/`**, grepped this session.

*Recommendation: one task, and it is `harness`'s. Repoint the 25 at frozen fixtures under
`tapes/fixtures/`, then regenerate their references once — after which a mismatch means what the
tool says it means. Doing it needs the ffmpeg pin resolved (the regeneration path still checks
it, and homebrew no longer carries an `ffmpeg@8`), so **rule the pin first**: either bump it to
9.0.1 and regenerate everything, or record 8.1.2 as unobtainable and drop the pin to a presence
check. The pixel path no longer depends on the answer.*

**RULED and DONE, both halves, 2026-08-16.** Teej: bump the pin and repoint. The pin is `9.0.1`
and every reference was regenerated in the same commit, which is the only way `tapes/README.md`
permits a pin to move. All 25 tapes open `tapes/fixtures/core-lib.rs` — a verbatim copy of the
file they used to photograph, frozen at `1e2e631`.

**It was one fixture rather than twenty-five**, because all 25 opened the same path, and it was
safe to do mechanically because none of them ever read the file's contents: their sentinels are
the mode chip, the leader float's `+claude` and `8e`'s `shown once`.

**Proven rather than asserted: `just tapes-diff` now reports `41 frames matched, 0 mismatched`.**
That is the number this entry exists for — the library reproduces itself, so a future mismatch
means a drawing changed. The run before this work reported 9 matched, 30 mismatched and 2
captures that failed outright.

**One frame needed a second pass and it is worth keeping.** `sweep-60` kept its pre-repoint
reference through the bulk `just tapes` — the tape was repointed, the reference was not — and the
`tapes-diff` immediately afterwards caught it as the single remaining mismatch. `just tape
sweep-60` produced it correctly on its own, so nothing is wrong with the tape or the recorder;
the bulk run dropped one. **The lesson is the sequence:** a regeneration is not finished until a
`tapes-diff` against it comes back all-matched, because that is what proves the recorder wrote
what it said it wrote.

**What this does not settle** is `7c-{rust,python,typescript}`, which regenerated with everything
else and now carry `T106`'s kind and source columns. The reference agreeing with the build is
what a Tier-2 reference *is*; whether the build should draw them is §`D`'s open ruling.

---

## Raised by `T041`, the store

Two entries. Both are decisions the task had to make to ship at all, both are recorded where they
are implemented, and both are **product-visible in a way a unit test cannot argue about** — so
they are here for a ruling rather than filed as settled.

### 41 · `s` on a line with no region says nothing at all

`Regions::set_state` answers *how many regions were in scope*, and the arms turn that into the
capability's return value (`Editing::mark`, `AppHost::mark`). On a line no region covers the
answer is `0`, and `0` is not trouble — `phosphor_steel::answer::trouble` reduces a `Done` to
`None`, so the ex line stays empty and **the screen does not change in any way**.

That is right for a script: `(mark-seen! …)` answering a count is composable, and a refusal would
make the ordinary case an error. Whether it is right for a **person** is the question. Pressing
`SPC u s` and getting no acknowledgement at all is indistinguishable from the key not being bound,
which is precisely the class of defect `CP-4`'s manual half kept finding — `gr` "not working" was
one of them, and it was a real gap; this one would be correct behaviour that reads the same way.

Design Language §6 has no rule for *"you asked and there was nothing to do"*. The nearest
precedent in the build is `:restart-server` with no argument, which **declines by name**
(`main.rs`, that Action's arm) rather than silently doing nothing — but that is a malformed
request, and this is a well-formed one whose answer is zero.

*Recommendation: leave the value a count, and let the surface decide. The cheapest honest version
is that the operator says `no unseen region here` on the ex line when it marked none — one line at
the call site in `Editing::mark`, nothing in the vocabulary, and nothing a script sees. Do not make
it a refusal: `S` over a block that happens to be fully seen would then read as an error.*

### 42 · A door cannot ask about the cursor, and `runtime/` is a door

`AppHost::scope` refuses `cursor`, `selection`, `picker-row` and `float-row` for the `region`
queries, naming the three tags it does take (`RESOLVABLE` in `main.rs`). The reason is not policy
— **the host genuinely has no editor**; `Editing::scope_of` is the half that does, and it lives on
the other side of the Steel barrier.

`request.rs` already draws this line for *agents* — `Target::focus_relative`, and the MCP door
refuses them because *"an agent has no cursor"*. But `runtime/*.scm` is not an agent. A keymap
thunk runs **while the user's cursor is somewhere specific**, and `(unseen-count 'cursor)` from one
is a reasonable thing to write and currently refuses. The *Actions* do not have this problem —
`SPC u s` passes `(key/at-cursor)` and resolves fine — because an Action from a keystroke reaches
`Editing::act`. It is only the **queries** that are one-sided.

So the vocabulary says a focus-relative target is refused *over MCP*, and the build refuses it over
Steel and CLI as well, for an implementation reason rather than the documented one. That gap is
what needs the ruling.

*Recommendation: it is a real seam and not worth closing at `T041`, because the fix is structural —
either the host learns the cursor (a snapshot the loop pushes each frame, which is a second copy of
editor state and invites exactly the staleness `Target`'s doc says late binding exists to avoid), or
queries from the VM route through the loop the way Actions do. The second is right and it is
`T046`'s neighbourhood, because a picker source is the first thing that will want to ask "what is
unseen **here**". Until then the refusal names the three tags that work, which is at least a
sentence a caller can act on.*

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
- **R15 · DONE — `main` is protected, and it was not "only Teej" after all.** This entry said a
  GitHub settings change only he could make; the API takes it, and the pre-`S4` scout made it on
  his instruction. Six required status checks, which are the six blocking CI jobs by name, with
  `Tier 2 — VHS pixel diff (non-blocking)` deliberately excluded — that exclusion is `V008`'s
  *done when* written as configuration. Force-pushes and deletion refused. `enforce_admins` is
  **false** on purpose: `master:main` is pushed directly here and there is no PR flow, and
  required checks cannot have run on a commit that does not exist yet, so enforcing against
  admins would have broken the next push rather than protected anything. It closes item (1) of
  `V008`; item (2), which is that the Tier-2 job has still never executed, is what keeps that
  task unticked.
- **R6 · DONE — and the entry below understated it by five.** The rot was real and in both
  directions: `scripts/lint-one-vm-door.sh`'s `entries=` regex named `keymap::press` and
  `keymap::reset`, which do not exist, and it also **missed five functions that do**.
  `grep -rn '^pub fn .*runtime: &mut Runtime' crates/phosphor-steel/src/` answers seven —
  `keymap::{resolve, resolve_seq, canonical, entries, ex_entries, ex}` and `status::compose` —
  against the two the regex listed. So the fix this entry proposed (*"should read
  `keymap::resolve|keymap::ex`"*) would itself have shipped a list missing four.
  **Fixed by deriving the list rather than correcting it**, since a hand-written enumeration of
  another crate's surface is what rotted: the script now builds the alternation from that
  signature at run time, and adding a VM-entering function to `phosphor-steel` extends the lint
  in the same commit. It fails loudly if the derivation yields nothing, so a changed signature
  convention cannot silently empty it.
  Coverage never actually lapsed, exactly as the entry said — `Layer` is the only owner of a
  `Runtime` (rule 1), so every real call site also matches `self\.runtime`. Proven by planting
  `keymap::canonical(rt, "zz")` outside `impl Layer`: the lint fails on it now, and the old
  hand-written list contained no alternative that matches that line. **The defect was a lint whose
  stated rule was wider than what it checked** — which is the same class as the vacuous vendor-hunk
  directory match, and worth the same treatment.

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
- **R13 · PARTIAL.** Every *component* of `6d`'s sentences is now in the live keymap — `gs` as an
  operator, the four nouns as object rows, `]u`/`[u` as sequence rows, and `:c` over a range —
  and `help_narrows_to_the_agent_objects_topic` reads the four nouns off a real frame. What is
  still absent is a row that spells a **sentence**: `:help agent-objects` teaches you `gs` and
  `ib` and never shows you `gsib`. Arguably correct for a grid rendered from the live keymap,
  since a sentence is a composition and not a binding — which makes this a question for whoever
  next holds `T086`, not a defect to fix on sight.
### The third repair window's list — between `CP-4` and Window E

Staged 2026-08-14 by the `S4` run. The test applied to each is the one [TASKS.md](TASKS.md)'s §B
used for the window between `CP-3` and `S4`: **does this get more expensive if the next window
runs first?** That is what put the node-kinds lint before `S4` rather than after — the lint that
would catch the next window's repeat had to exist before the window that would repeat it.

**Take before Window E, because waiting costs more:**

- **§12's mockup conflict, and it has a deadline nothing else here has.** `fixtures/` is a
  byte-exact transcription of the worked example, so whichever rendering wins is what every
  agent-surface tape at `CP-5` shows — and **Window E ends at `CP-5`**. Settling it afterwards
  means re-capturing. This is Teej's and it is the one item on this list with a hard ordering
  constraint rather than a preference.
- **§27's `6c`-versus-§3 ruling**, because `T041` builds the region states the disagreement is
  about. Deciding it after the states exist means changing them.
- **§31's `phosphor-term` coverage**, because Window E adds four surfaces that draw and that
  crate is what keeps them from tearing. The LSP-framing fuzz target rides along.
- **`T100`** — **done in this window**, which is a change to what [TEAM.md](TEAM.md) schedules. It
  sat at the front of Window E *"in a phase where nothing else is rewriting the parity
  expectations, because that is the whole cost of it"* — but `S4` had just rewritten them (three
  capabilities, 212 → 215) and Window E will rewrite them again. It needed only `T020` and `T024`,
  both ticked, and touched `door.rs`, `answer.rs` and `parity.rs`, none of which `T041` wants. It
  also turned out to reach `action.rs`, `runtime.rs`, `registry.rs`, `repl.rs`, `main.rs` and both
  `6b` snapshots, because a third `Outcome` case is a compile error at every consumer — which is
  an argument for having run it while the tree was quiet rather than against it. It found `§33`.

**Can wait, with a reason rather than by omission:**

- **`T082`'s alignment surface** — inline virtual text at a column, which needs a
  `vendor/ratatui-code-editor` patch and its `VENDOR.md` entry. **This is the only item in the
  window with no creditor**: no task in the graph builds that patch, which is why `T082` is
  unticked and not re-homed. It costs no more later than now.
- **§31's remaining property and benchmark gaps** — real, and none of them blocks Window E.
- **§30's count-lint decision** — a decision, not work.

**Not repair work at all**, listed so nobody adds them to a window: `T040`'s tick (`T041` closes
it), `apply-workspace-edit` (`T060`), `request-references` (`T047`), `V006` (`T041`), `V008`(2)
and `V009`. Every one has a named creditor inside a scheduled window.

> **Window E's shape, checked against [TASKS.md](TASKS.md) on 2026-08-14.** Every prerequisite
> outside the window is ticked — `T019`, `T020`, `T024`, `T028`, `T030`, `T084`, `T015`. `T080` is
> unticked and annotated *"Built at S2, deliberately not ticked — `T048` ticks this"*, the same
> honest pattern `T040` just took, so it blocks nothing.
>
> The graph is a chain with one root, exactly like `S4`'s: `T041` gates `T042`, `T043`, `T044`,
> `T045`, `T048`, `T049` and `T087`, with `T046` behind `T045` and `T047` behind `T046`. Peak
> concurrency after the root is seven.
>
> **But plan it by crate, not by task, because `S4` proved the file graph is narrower than the
> task graph.** `S4`'s three dependency-independent tasks all wanted `theme.rs`, `interpret.rs`
> and `lib.rs`; running them concurrently would have been the collision rule 1 of *Concurrency*
> exists for, and the window ran two-wide and then serially instead. A dependency graph says what
> *may* run together; only the file list says what *can*.

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

**Two more from the pre-`S4` scout — §8 and §14, both ruled and both built.** They are together
here because they turned out to be the same shape twice: a door that was awkward at the spelling a
person actually uses, and a door that answered two different things depending on how you reached
it.

- **§8 · `place-watch` takes a `Target`; `6b` passes a string. RULED: the mockup was right and
  the build was the bug.** `path:line` is now a `Target` spelling —
  `crates/phosphor-core/src/request.rs`'s `target_from_text`, reached through an optional
  `text = …` clause on `wire_union!` that every other union declines by default. It is
  deliberately narrow: a colon and a number are required, so `"cursor"` stays a tag error rather
  than becoming a file target and turning a typo into a different request. `to_value` still
  answers the tagged record, so the spelling is an input and never an encoding, and the three
  doors keep one wire form. Proven at the shipping binary, not at a unit boundary:
  `phosphor --eval '(mark-seen! "src/retry.rs:24")'` decodes and refuses on its own terms.

  **The most useful thing it turned up was that three tests were pinning the bug.** `6b`'s two
  golden frames went red, and the diff is the whole ruling in two lines:

  ```text
  - ⇒ #refused · Error: TypeMismatch: `place-watch`: argument `anchor`: expected a tagged Target record, found text
  + ⇒ (#refused "not built yet — T077 builds it")
  ```

  The line the mockup draws now reaches the dispatcher and gets declined in the product's voice
  instead of the VM's. Alongside them, `crates/phosphor-steel/tests/screen_6b.rs` asserted
  `answers[3].contains("place-watch")` — which was reading the *error message*, and passes on any
  message that happens to mention the capability. It asserts `T077` now, and separately that the
  answer is **not** a `TypeMismatch`, which only a decoded call can satisfy. Both frames also
  carried a `NOTES` block, rendered into the snapshot, stating that this line *"fails on shape"*.
  All of it was accurate when written and all of it described the defect as the specification.
  That is the same shape as the `6d` prose recorded on `T086` in [TASKS.md](TASKS.md), found the
  same way — by a behaviour change forcing a frame to move.

- **§14 · `phosphor --eval` cannot report refusal through its exit code. RULED: the two routes
  had to agree, and the eval one was wrong.** The entry framed this as a forward-looking trap.
  Running it found something sharper — the *same door* already disagreed with itself:

  ```text
  phosphor mark-seen --target=cursor        #refused · not built yet …   exit 1
  phosphor --eval '(mark-seen! "a.rs:1")'   (#refused "not built yet …") exit 0
  ```

  A verb decodes to an Action and gets `Outcome::Refused`; the eval route runs scheme, and the
  refusal comes back as the *value* the scheme evaluated to, inside a perfectly successful
  `Outcome::Done`. So this is not a new contract — it is `T023`'s existing one applied to the
  route that was skipping it. `Answer::happened` in `crates/phosphor/src/door.rs` now reads the
  result, branching on `phosphor_steel::registry::REFUSED`, whose own doc comment says the shape
  is two elements *"so the reason survives to … a composition that wants to branch on it"*.
  Pinned by `the_two_cli_routes_agree_on_what_a_refusal_exits`.

  **Done now precisely because the entry said nothing was wrong yet.** Nothing reads `$?` from
  this route today — the parity walk reads stdout, and `scripts/seed-fixtures.sh` matches the
  refusal text before it ever consults `code` — so the change is free. It stops being free the
  day `T041` lands and refusals start turning into successes, which is what §14 was warning
  about. Distinct from `T100`, which is the *voice* of a refusal; this is only the exit code.

- **§7 and §9 · The door does not speak §6's voice. RULED: one task, as both entries
  recommended.** They are the same defect — no `Outcome` case for *"it ran and raised"*, so a
  refused query surfaces Steel's `Error: Kind:` envelope; and `door.rs::why` against
  `answer::why` phrasing one enum two ways — and they rewrite the same parity expectation set, so
  doing them separately means regenerating and reviewing it twice. → `T100` in the new
  *B · The repair window* section of [TASKS.md](TASKS.md), which carries the scope block that
  stood here, and to `spine` in [TEAM.md](TEAM.md). The expectation count is deliberately not
  quoted in prose any more: `scripts/doc_claims.py` recomputes it, and a hand-written copy is
  what went stale.
  **`T100` is done, in the repair window between `CP-4` and Window E** rather than at the front
  of Window E — `Outcome::Raised` exists and `phosphor --eval '(unseen-regions …)'` answers
  `#raised · not built yet — T041 builds it` where it used to wear `Error: Generic:`. §9's half
  turned out to have been closed by `5050b58` and its guard turned out to be a test that could no
  longer fail; both are recorded on the task. **It also found §33 below**, which is the behaviour
  under one of the two sentences and could not be fixed by any wording.

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
  already went, which is why `TASKS.md` read 209 when this was written. (It reads **215** now —
  the repair window between `CP-3` and `S4` added `set-macro-recording`, `register` and
  `place-anchor`, and `S4` added `ingest-completions`, `ingest-signature-help` and
  `ingest-hover`. The count is recomputed by `scripts/doc_claims.py`, so it is not a claim
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
