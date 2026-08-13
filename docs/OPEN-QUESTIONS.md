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
citation below was checked against the tree on 2026-08-12.

---

## Doc-versus-tree disagreements

These are cheap: the tree is right and a document has not caught up. They are listed because
`docs/` is the specification, and a specification that disagrees with the build is a bug in the
specification — but nobody may quietly edit it into agreement.

### 1 · Is `Node::KeyHints` one widget file or two?

`TEAM.md:76` gives `surface` both `help_grid.rs` and `keymap_footer.rs`, and `TEAM.md:124`
names `keymap_footer.rs` again when explaining why `T034`/`T035` moved from `spine`. Neither file
exists. The Window D seam pass created `key_hints.rs` instead, because `Node::KeyHints` is **one**
node kind carrying a `Density` (`crates/phosphor-core/src/view/props.rs:496`), drawn at three
densities: the float footer, the `SPC` leader grid, and the `:help` body.

`TEAM.md:122` states the rule that decides it: *"`phosphor-ui` is split per widget file. A new
widget file needs `spine` to add its view-tree node kind first."* Spine added one kind.

- **Amend `TEAM.md:76` and `:124` to `key_hints.rs`.** One kind, one file, one draw site — the same
  principle `scripts/lint-one-escape-hatch.sh` enforces for `Node::Spans`.
- **Or split the widget across two files**, which means two draw sites for one node kind, and
  `T034`/`T086` become two agents rather than one.

*Recommendation: amend TEAM.md.*

### 2 · Who owns `T027`, the kitty keyboard protocol?

`TEAM.md:76` lists `T027` under `surface`. `TEAM.md:199` states what that role is — *"`surface`
draws, and never touches a terminal"* — and the `spine` section names kitty-protocol negotiation as
part of the app layer. Both cannot hold. The negotiation already exists in `phosphor-term`
(`T014`, `crates/phosphor-term/src/lib.rs`, `KeyboardProtocol::Kitty`), which is `spine`'s crate.

This is the same move that sent `T014` itself to `spine` after `CP-1`: the file decides the task.
`scripts/lint-no-app-layer-in-ui.sh` fails CI on a `crossterm::` reference from `phosphor-ui`, so
the lint has already ruled and only the table disagrees.

*Recommendation: confirm `spine`, amend the `surface` task list. Window D built it that way.*

Related and separate: `TASKS.md:106` records that VHS's browser-based terminal does not implement
the protocol, so `T027` is verifiable on hardware only. That is a verification limit, not an
ownership question.

### 3 · Window D's live-teammate count

`TEAM.md`'s window table says Window D has **all five** teammates live. The `agent` role owns
`T050`–`T070` and `T074`–`T077`, none of which fall in Window D. Four roles are live: `spine`,
`surface`, `store`, `harness`.

*Recommendation: correct the table to four and add a one-line note, the way `TEAM.md` already
explains `harness` having no `T`/`V` tasks after Window D.*

---

## Scope questions

### 4 · `V006` cannot meet its own acceptance criterion in Window D

`TASKS.md:222` asks for a committed sample tree **plus seeded store state** — regions, seen-state,
threads, a canned transcript — and its *done when* is *"`CP-5`'s tapes produce identical output on
two machines."* The semantic store is `T041`, at S5, two windows away. The capability registry
names the store verbs; nothing implements them.

- **Split it, on the `T022` precedent** — `V006` closes on the fixture tree and the
  `phosphor --eval` seeding mechanism; the seeded-store half becomes a criterion on the S5 task
  that lands the store.
- **Or leave it open until S5** — accurate, but it sits unticked across two windows with no record
  of which half exists.

*Recommendation: split it. This is the shape `CP-2` already ruled for `T022`, and for the same
reason: a task whose mechanism is provable now and whose subject arrives later should not be a
binary.*

### 5 · `6b`'s footer promises `q close` on a surface whose body is a text input

`q` types; `esc` closes (Design Language §9). Surfaced by `T022` and left as drawn, because it
needed modes to be decidable — `T026` lands them in Window D.

- **Make the footer mode-aware**, as an acceptance criterion on `T034`: the footer reads the live
  keymap already, so "the footer tells the truth about what this key does *in this mode*" is nearly
  free while that widget is being written and a rewrite afterwards.
- **Or change the drawing** to say `esc close`.

These are not exclusive, and probably both: `6b`'s frame is drawn mid-typing at the λ prompt, so
its footer is wrong for that frame even once the build is mode-aware. Teej edits
`TUI Mockups.dc.html` at claude.ai — **never edit the `.dc.html` here.**

*Recommendation: both. Window D's `T034` was launched before this was raised, so the mode-aware
half lands as a follow-up rather than an original criterion.*

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

*"T041 builds this"* against *"not built yet — T041 builds it"*. Unifying them rewrites 624 parity
expectations.

*Recommendation for both: one task, `spine`, scheduled in the S4 run.* They are the same defect —
the door does not speak §6's voice — and they rewrite the same expectation set, so doing them
separately means regenerating and reviewing 624 expectations twice.

> **Scope**
> - Files: `crates/phosphor/src/door.rs`, `crates/phosphor-steel/src/answer.rs`,
>   `crates/phosphor/tests/parity.rs`
> - Named units: 1 enum (`Outcome`), 2 `why` implementations, ~624 parity expectations
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

### 11 · `just fmt-fix` writes every file, so a file lock cannot hold

Window D runs several agents concurrently in one worktree, each owning a named set of files. That
discipline is what makes the run safe — and `just fmt-fix` is workspace-wide, so an agent running
the recipe `CLAUDE.md` sanctions reformats files it does not own, mid-edit. Observed: `T029`'s
agent reformatted `crates/phosphor/src/main.rs` while `spine` was writing it. Formatting only, and
`just fmt` is what CI checks, so nothing broke — but the rule is unenforceable as written.

- **Write it into `TEAM.md`'s concurrency rules**: in a concurrent window, run `just fmt` (check)
  and fix only your own files.
- **Or scope the recipe** — `cargo fmt -p <crate>`, which needs the agent to know its crates.

*Recommendation: the `TEAM.md` rule. A per-crate recipe invites the `--all` reflex the hook
already exists to block.*

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

### 15 · `s` — the mark-seen operator, or vim's substitute?

`TUI Mockups.dc.html`'s screen `6d` says *"`s` composes like an operator"* and makes it mark-seen.
Vim's `s` is substitute, and `CP-3`'s criterion is *"vim habits should carry without thinking about
it."* `action.rs:665` implies `S` goes the same way.

`T028` built `Operator::MarkSeen` into the `Role` vocabulary and **deliberately did not change the
keymap**, so `sib` today substitutes a character and types `ib`. `runtime/keymaps.scm:365` and
`:391` are the two rows, paired with one arm at `phosphor-steel/src/keymap.rs:356-364` — without
the arm the row decodes to nothing and the key goes dead.

- **Take `6d`'s `s`.** `cl` still substitutes, and seen-state is the more phosphor-shaped key.
- **Keep vim's `s`** and give mark-seen another key, amending `6d`.

*No recommendation — this is a taste call about muscle memory, which is the one thing a checkpoint
measures and an agent cannot. It wants deciding before `T044`/`T049` build on it.*

### 16 · Hand-rolled codec and XDG paths, or the crates `SPIKES.md` recommends?

`SPIKES.md:307` recommends `postcard` for exactly `T030`'s append-only log, and `:304` recommends
`etcetera` for the XDG paths. Neither is in `Cargo.toml`'s dependency table, and
`crates/phosphor-core/Cargo.toml:9` says the crate is *"deliberately dependency-free at the
floor"* — so `T030` hand-rolled both: a LEB128 + length-prefixed-UTF-8 codec, and
`journal::state_home`.

One consequence is already load-bearing: the state-dir key is a hand-rolled FNV-1a 64
(`journal.rs:1261`) pinned by literal in a test, precisely because `std`'s `DefaultHasher` is
documented-unstable across releases and would silently orphan every user's state on a toolchain
bump. That reasoning is right whichever way this goes.

*Reversible either way — one section and one function. `spine`'s call, because it owns
`Cargo.toml`.*

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

---

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

- **R1 · The `Motion` vocabulary.** `f` `F` `t` `T` `;` `,` and `W` `B` `E` are not expressible
  (`request.rs:588-631` is a payload-free `wire_choice!`), and there is no case-change capability,
  so `~` `gu` `gU` cannot be bound. Both patterns already exist in the tree:
  `SelectObject` carries `delimiter: Option<char>`, and `Role::Register` is already
  "the next key names a literal". 9 new `Motion` tags, 2 `Role` arms, 1 capability — and the
  vocabulary goes 208 → 209, which `docs/TASKS.md:22` states in prose.
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
