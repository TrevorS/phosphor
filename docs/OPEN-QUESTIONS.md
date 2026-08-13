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

---

## Doc-versus-tree disagreements

These are cheap: the tree is right and a document has not caught up. They are listed because
`docs/` is the specification, and a specification that disagrees with the build is a bug in the
specification — but nobody may quietly edit it into agreement.

**All three that stood here are ruled and closed** — §1, §2 and §3, all amending
[TEAM.md](TEAM.md). One new one has taken their place.

### 19 · Who owns `phosphor-ui/{interpret,frame}.rs`?

Found while making the §1 amendment, and it is the same shape. `TEAM.md`'s widget list splits
`phosphor-ui` per file across four owners, and it names neither `interpret.rs` nor `frame.rs`.
Both exist (`crates/phosphor-ui/src/`), and both are `T079`'s — *tree interpreter + frame cache* —
which `TEAM.md` assigns to `spine`. So the crate that `surface` owns contains two files whose
task belongs to `spine`, and the table does not say so either way.

This is not a hypothetical: `interpret.rs` is where a new `Node` kind becomes pixels, so it is
touched by every widget task, and `scripts/lint-one-escape-hatch.sh` already treats its single
`Node::Spans` draw site as load-bearing.

- **Name them in `spine`'s row**, the way `T014` and `T027` were, on the same rule — the file
  decides the task, and the interpreter is the view-tree protocol's other half rather than a
  widget. `surface` keeps every file that draws one node kind.
- **Or give them to `surface`** and move `T079` with them, which contradicts `TEAM.md`'s first
  single-writer rule: *only `spine` edits the view-tree protocol.*

*Recommendation: `spine`, and add the row. The first option is the only one consistent with the
rule that is already mechanical.*

---

## Scope questions

**§4 and §5 are ruled and closed** — `V006`'s split, and `6b`'s `q close` footer.

### 6 · Three editor-layer names `6b` types that nothing binds

`goto`, `claude`, `region-author`. They belong in `runtime/` over the records `T041` returns at S5;
writing them now would invent the record shape, which is why `CP-2` flagged rather than invented
them. That part is settled.

What is open is the *form of the flag*. Today it is a note, and this build's most recent defect was
a rule that held by remembering — the keybinding invalidation `CP-2` found missing by running it.

*Recommendation: keep them unbound, and add a test asserting they are still unbound, so the day
`T041` lands the test goes red and forces the binding rather than waiting to be recalled.*

---

## The door's voice

### 7 · There is no `Outcome` case for "it ran and raised"

A refused query surfaces as `#refused · Error: Generic: not built yet — T041 builds it`. The
`Error: Kind:` envelope is Steel's and is not the §6 voice.

### 9 · `door.rs::why` and `answer::why` phrase one enum two ways

*"T041 builds this"* against *"not built yet — T041 builds it"*. Unifying them rewrites 627 parity
expectations.

*Recommendation for both: one task, `spine`, scheduled in the S4 run.* They are the same defect —
the door does not speak §6's voice — and they rewrite the same expectation set, so doing them
separately means regenerating and reviewing 627 expectations twice.

> **Scope**
> - Files: `crates/phosphor/src/door.rs`, `crates/phosphor-steel/src/answer.rs`,
>   `crates/phosphor/tests/parity.rs`
> - Named units: 1 enum (`Outcome`), 2 `why` implementations, ~627 parity expectations
> - Verification: existing parity suite
> - Risk: public API yes (one `Outcome` case) · data migration no · cross-module yes
>   (`phosphor`, `phosphor-steel`) · reversible yes · external blocker no

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

---

## Raised by Window D's S3 run

**§11, §16 and §18 are ruled and closed** — the file lock, the hand-rolled codec, and the eleven
declared mutations with no creditor.

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

### 13 · The ex line draws outside the view tree

`open-prompt` is declared `S6 / "T058"` (`crates/phosphor-core/src/action.rs:640`), but `T033`
needed an ex line at S3 or `CP-3` cannot be judged — you cannot save. The binary implements
`PromptKind::Ex` and declines Claude and Search with a reason naming `T058`. Because
`Node::Prompt` is still deferred (`crates/phosphor-ui/src/interpret.rs:453`), the ex row is drawn
from `Node::Line` / `Node::Label` in `main.rs`'s `draw` rather than through the prompt node.

That is a second draw path for a surface that has a node kind, which is the shape
`scripts/lint-one-escape-hatch.sh` exists to prevent for `Node::Spans`.

*Recommendation: accept it as scaffolding with a demolition date at `T058`, and say so in a comment
at the draw site so it is found. It is the same trade `T090` made and the same one `T026` collected
on.*

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

### 17 · Does `CP-3` sign off without its VHS artifacts?

`CP-3`'s "VHS produces" list names four captures: the leader popup (`3c`), folds collapsing and
expanding, insert-only whitespace marks, and the unknown-key hint firing then *not* firing again
(`8e`). These are `harness` standing work under `V005` rather than any numbered task — which is
exactly why they can fall through a task-driven run.

Two of the four also have no S3 task behind them at all: nothing in `T026`–`T035` builds folds or
insert-only whitespace marks, so a tape for either would be capturing a surface that does not
exist.

*Recommendation: sign off on the two that have surfaces (`3c`, `8e`) and record the other two
against the task that builds them, rather than holding the checkpoint for artifacts of nothing.*

**Updated at the `CP-3` re-audit (repair pass) — the reasoning above no longer holds, and the
position is worse, not better.** All four surfaces now exist in the shipping binary: `3c` opens on
`SPC` (`crates/phosphor/tests/loop_pty.rs:417`), `8e` fires once on an unbound key (`:487`), folds
close and open on `za`/`zR` (`:532`, wired at `crates/phosphor/src/main.rs:1911`-`1922`), and the
INSERT-only whitespace marks are driven off `machine.mode()` every frame (`:903`). So the excuse
for three of the four is gone.

**All four are captured now.** `tapes/` carries `3c.tape`, `8e.tape`, `folds.tape` and
`insert-whitespace-marks.tape`, with eleven committed artifacts between them — `3c` closed and
open, `8e` silent and taught (the negative half `CP-3` asks for by name), `folds` closed, open and
reopened, and the whitespace pair.

**And the recapture answered the question it was left open on.** The whitespace tape's two stills
were byte-identical when this section was first written — the `NORMAL` and `INSERT` frames the same
bytes, mode chip included, which is impossible if the second screenshot advanced. Recaptured, they
**differ**, so it was the VHS capture pipeline duplicating a frame and not the surface failing to
render. Deleting them the first time greened `scripts/lint-repo-hygiene.sh` without answering that;
recapturing did, and the answer is that the build was right all along.

`tapes/artifacts/DUPLICATES.md` now exists for the pairs that *are* identical by construction, with
each group stating whether it is a duplicate by definition or a gap — which is the honest
resolution the first attempt skipped.

*This question is answered, not by a ruling but by the work: it was never an artifact-of-nothing
problem once the surfaces went live. What remains is the ordinary `harness` standing work of
keeping them current.*

## Repair pass — queued work, not questions

These need no ruling. They are collected here because every one of them lands in a file that no
agent in the S3 run owns, so none of them can be done inside it. They run as one `spine`-and-
`surface` pass **after the run and before `CP-3`'s manual half**, because most are things Teej
would otherwise hit in the first minutes of editing.

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
> The fix for the *window* is `R2` and `R17`–`R19` below. The fix for the *method* is to give the
> host to a wiring agent in the last phase of every window from now on, whose whole job is that
> nothing shipped this window is unreachable from a keystroke.

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
- **R7 · Two duplicated types at the `spine`/`surface` seam** (`surface`'s edit, both):
  `soft_wrap.rs:85-100`'s `EditMode` says *"the real mode enum is spine's and does not exist yet
  (T026)"* — it exists now; and `buffer_view.rs:180`'s second `ScrollRequest`. `main.rs` converts
  at the boundary in one place for each.
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

- **R1 · The `Motion` vocabulary. CLOSED AS BUILT, and the open half is ruled.** R1 said `f` `F`
  `t` `T` `;` `,` and `W` `B` `E` were not expressible and that there was no case-change
  capability. **All of that is false against the tree**, checked this session:
  `wire_choice!(Motion …)` at `crates/phosphor-core/src/request.rs:669` carries
  `FindCharForward`, `FindCharBackward`, `TillCharForward`, `TillCharBackward`, `RepeatFind`,
  `RepeatFindReverse`, `BigWordForward`, `BigWordBackward` and `BigWordEnd`;
  `runtime/keymaps.scm:420`–`431` binds all nine; and `SetCase` is a capability
  (`crates/phosphor-core/src/action.rs:336`) bound at `keymaps.scm:463` (`gu`), `:464` (`gU`),
  `:529` and `:556` (`~`). R1's arithmetic — *"the vocabulary goes 208 → 209"* — is the tell: it
  already went, which is why `TASKS.md` reads 209.
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
