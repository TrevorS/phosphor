# `fixtures/` — V006's deterministic fixture repo

**`V006` is not done, and one of its two halves now is.** `T044` landed
seen-state persistence, so *"seeded store state is reachable through
`phosphor --eval`"* is **met**: the two `mark-seen!` lines in the plan answer
`1` where they answered `0` against the empty store of a fresh process. What
is still open is the other half — `CP-5`'s *"tapes produce identical output on
two machines"* — which needs the session/review/watch subsystems the plan's
remaining lines call (`T050`, `T053`, `T054`, `T057`, `T067`, `T068`, `T077` —
`S6`–`S8`), and a determinism answer for whatever stamps time. See "Residue"
at the bottom. Nothing in this file claims otherwise.

## Why `fixtures/`, not `tests/fixtures/`

Picked `fixtures/` at the repo root over `tests/fixtures/` because this tree
is not a Rust integration-test asset scoped to one crate's `tests/`
directory — `tapes/**` and (once `T041` lands) `scripts/seed-fixtures.sh`
both need to reach it from outside any crate, the same way `tapes/**`
already reaches real source files with a repo-root-relative path today
(`tapes/1a.tape`'s `../crates/phosphor-core/src/lib.rs`). A top-level
directory needs no crate to own it and no `Cargo.toml` to register it.

## `fixtures/` is its own miniature workspace

Every path inside `fixtures/seed/plan.scm` is relative to `fixtures/`
itself — `"src/retry.rs"`, not `"fixtures/src/retry.rs"`. That is a
deliberate choice, not an oversight: `docs/IMPLEMENTATION-PLAN.md`'s
[Q1](../docs/IMPLEMENTATION-PLAN.md#q1) keys seen-state on the
*canonicalised workspace root*, and nothing in that decision says a
workspace has to be the repo root. Treating `fixtures/` as its own workspace
root means every path here is byte-identical to what the design mockups
draw (`src/retry.rs:24`, `src/retry.rs:19-21`) instead of needing a
`fixtures/` prefix nobody drew. `scripts/seed-fixtures.sh` `cd`s into
`fixtures/` before calling `phosphor --eval` for exactly this reason.

**Answered by `T044`, and the answer is the one this section hoped for.**
State is keyed on `journal::workspace_key(canonical_root)` — Q1's
*"keyed on path never VCS identity"* — and the root is the directory the
editor was started in. `scripts/seed-fixtures.sh` `cd`s into `fixtures/`
before calling `phosphor --eval`, so `fixtures/` **is** its own workspace
root and every path here stays byte-identical to the mockups.
`a_vcs_directory_does_not_change_where_state_lives` is the assertion:
planting `.jj` and `.git` changes nothing, so the *"nearest VCS root"* rule
this paragraph feared was not merely avoided — it is the rule Q1 rules out.

The original flag, kept because it is why the answer was checked: If a future window picks a different rule
(e.g. "the nearest VCS root," which would make `fixtures/` *not* its own
root since this repo's own root is one level up), this file's paths need
revisiting. Recorded here so that decision has one place to land.

## Layout

```
fixtures/
  README.md            this file
  src/
    retry.rs             Rust — the design mockups' own running example (below)
    fetch.rs              Rust — the review block's second file
    wrap.rs                Rust — T081's soft-wrap fixture (below)
    broken.py             Python — deliberately invalid syntax (below)
    client.ts             TypeScript
    events.js             JavaScript
    helpers.scm           Steel/Scheme
    notes.md              Markdown
    manifest.json         JSON
    config.toml           TOML
    settings.yaml         YAML
    page.html             HTML
    theme.css             CSS
    policy.csv            CSV (T082's hand-rolled parser, not tree-sitter)
  seed/
    plan.scm              the seed plan — see "Seeding" below
```

One file per language in `T037`'s first-class twelve (TS, JS, Rust, Python,
Steel, Markdown, JSON, CSV, TOML, YAML, HTML, CSS — `docs/SPIKES.md`'s
grammar table), plus two extra Rust files for the mockup-aligned review
block and the soft-wrap fixture. None of this is randomly generated or
pulled from a directory walk: every file is hand-written and committed, so
there is no filesystem-iteration-order non-determinism to worry about in
the first place.

## `src/retry.rs` and `src/fetch.rs` — the design mockups' own example

`docs/design/TUI Mockups.dc.html` and `docs/design/Design Language.dc.html`
cite `src/retry.rs`/`src/fetch.rs` by name across a dozen screens — it is
already the design's standing worked example, not something this fixture
invented. Three screens render the *actual code*, not just a filename or a
line number, and were used as the primary source, checked by reading the
HTML and mapping each cited line to its enclosing `<div id="…">` block —
not assumed from memory:

- **`3a` "Anchored exchange"** (`TUI Mockups.dc.html:855-884`) renders
  `retry_with_backoff`'s full body, lines 12-26, with real line numbers, and
  anchors a thread over `match op() { … }`, carrying the exchange *"⚓ you ·
  2m collapse these arms — use the shared backoff helper"* / *"✻ claude ·
  1m collapsed — error carried in `last`, returned after the loop"* (line
  870). This is the single most complete and load-bearing citation in the
  design docs for this file, so `retry.rs`'s lines 1-27 are a byte-for-byte
  transcription of what `3a` draws (function signature, body, and the
  `thread::sleep(jitter(delay))` / `delay = (delay * 2).min(policy.max_delay)`
  pair on lines 23-24).
- **`6c` "Anchors survive the rewrite"** (`TUI Mockups.dc.html:521`) is the
  *same* thread, later: its own virtual text reads *"⚓ thread · was
  retry_with_backoff:19–21 · followed node fn:next_delay"* — an explicit
  numeric span, and the reason this fixture's seed plan anchors the thread
  to lines 19-21 rather than the 4-row region `3a`'s CSS visually groups
  (19-22, the closing `}` sharing the wavy-underline style by coincidence of
  layout, not semantics). `1c`'s own prompt-line anchor chip agrees:
  *"⚓ src/retry.rs:19–21"* (line 1317). Two independent, explicit numeric
  citations against one visual grouping — the explicit ones won.
- **`6b` "Steel REPL"** (`TUI Mockups.dc.html:494`) answers `(unseen-regions
  "src/retry.rs")` with *"`(#region 4 fn:use  #region 6-10 struct:RetryPolicy
  #region 12-24 fn:retry_with_backoff)`"* — the exact three region spans
  this fixture declares for `retry.rs` (`plan.scm`'s `declare-regions!`,
  entries 1-3), independently confirmed a fourth time by `4b`'s *"@@ 12–24 ·
  retry_with_backoff ⋯ folded · 13 lines"* (line 744; 12 through 24
  inclusive is exactly 13 lines) and `8d`'s own copy of the same picker row
  (line 250).
- **`2b` "Hunk peek"** (`TUI Mockups.dc.html:1023-1056`) and **`3b` "jj
  timeline"** (`TUI Mockups.dc.html:886-911`) both render `src/fetch.rs`'s
  `fetch_json`: `pub async fn fetch_json(url: &str) -> Result<Value,
  FetchError>`, wrapping `client.get(url).send()` through
  `retry_with_backoff(…, &RetryPolicy::default())`. `fetch.rs`'s
  `fetch_json` (lines 10-14) is that function, post-edit — `2b`'s own diff
  shows it *replacing* a bare `client.get(url).send()?` call, which is why
  this fixture shows only the after-state, not a diff.
- **`3b`** also draws the jj timeline row this fixture's seed plan
  transcribes verbatim for `tool-call-completed!`: *"○ 7c3d · claude retry
  logic +51 −3 seen ✓"* (`TUI Mockups.dc.html:897`).

| citation | screen (verified against `<div id>` boundaries) | checked against the committed file |
|---|---|---|
| `use util::jitter;` | `3a`, line 4 of the rendered file | `retry.rs:4` is exactly `use util::jitter;` |
| `pub max_delay: Duration,` at line 9 | `8a` "Search — grep with agent context" (`TUI Mockups.dc.html:164`, a grep-hit-with-context row) | `retry.rs:9` is exactly `    pub max_delay: Duration,` |
| `retry_with_backoff`, lines 12-26 | `3a` (`TUI Mockups.dc.html:859-880`) | `retry.rs:12-27` — see above, byte-for-byte |
| region span `12-24` for `retry_with_backoff` | `6b` line 494, `4b` line 744, `8d` line 250 — three independent citations | `plan.scm`'s `declare-regions!` entry 3: `(12,1)`–`(24,51)` |
| thread anchor, lines 19-21 | `6c` line 521 (explicit *"was retry_with_backoff:19–21"*), `1c` line 1317 (`"⚓ src/retry.rs:19–21"`) | `retry.rs:19-21` is `match op() { Ok(v) => …, Err(e) => …,`; the seed plan's `start-thread!` targets exactly this span |
| `.min(policy.max_delay)` at line 24 | `8a` (`TUI Mockups.dc.html:165`) | **partial** — `8a`'s fragment shows `.min(policy.max_delay)` alone on its own line, i.e. a different line-wrap than `3a`'s single-line `delay = (delay * 2).min(policy.max_delay);`. `3a` was preferred (fuller, more load-bearing citation); `retry.rs:24` matches `3a`, not `8a`'s wrapping. Both still put the text on line 24. |
| `fetch_json(url: &str) -> Result<Value, FetchError>` | `2b`/`3b` | `fetch.rs:10` |
| `RetryPolicy::default()` | `2b`/`3b` | `fetch.rs:11`, and `retry.rs:29-36` (`impl Default for RetryPolicy`) — this fixture's own addition, since neither screen shows `Default`'s body, only the call site |
| `retry_with_backoff(|| client.get(url).send(), &policy)` | `2b` (`TUI Mockups.dc.html:1046`) | `fetch.rs:12` — and fixed `retry.rs`'s own parameter order to match: `op` first, `policy: &RetryPolicy` second (an earlier draft of this fixture had them reversed; corrected against `3a`'s and `2b`'s agreement on the order) |

**A second citation not integrated, for the same reason.** `2a` ("Review-
block picker," `TUI Mockups.dc.html:1003`) labels a picker row *"fetch.rs:3–7
fetch_json wired"* — a different line range than the one this fixture uses
(`fetch.rs:10-14`), implying a version of `fetch.rs` with little or no file
header before the function. `2b`/`3b` show the function's *content* in full
and agree with each other; `2a` only shows a line-range label with no
content to check it against, so it was treated as the weaker citation and
not chased into reshaping `fetch.rs` a second time. Recorded so a future
reader who greps for `fetch.rs:3` and gets nothing doesn't conclude the
fixture is wrong without reading this paragraph first.

**One citation deliberately not integrated.** Design Language's own example
render (a separate document from the TUI Mockups) shows a fragment reading
`jitter(exp.min(self.max_delay))`, which implies a *method* on some `self`
(a `Jitter` struct, going by the surrounding text) — structurally
incompatible with `3a`'s fuller and more authoritative version, where
`jitter` is a free function imported via `use util::jitter;` and called
directly (`thread::sleep(jitter(delay))`, line 23). Rather than force an
artificial reconciliation, this fixture follows `3a` and does not claim
line-64 parity with that one fragment. The two design documents disagreeing
with each other about the same illustrative code is not new to this
session — `docs/IMPLEMENTATION-PLAN.md`'s `CP-2` ruling on `6b`'s λ colour
is the same class of finding, resolved the same way (pick the more specific
drawing, record the discrepancy, move on).

**What was not chased further:** column offsets beyond what a script could
check mechanically. Every `start`/`end` column in `fixtures/seed/plan.scm`
is `len(line) + 1` for the line's real committed text (computed with a
one-line Python check against the actual file, not counted by eye or
guessed), so a span's *end* is always right after the line's last
character. Nothing has verified this is the column convention `T041` will
actually expect (1-based, character-counted, per `request.rs`'s own
`Position` doc — that much is read from the source; whether the store's
real anchoring agrees is untested because it doesn't exist yet).

Together the two files carry **six** regions and **two seen**, matching the
"retry logic — 2 files · 6 regions" and "2 seen ✓" counts the mockups draw
repeatedly (`1a` line ~1195, `4b` line ~733, `5c` line ~653, `2d` line
~1120) — see `fixtures/seed/plan.scm`'s `declare-regions!`/`mark-seen!`
entries.

## `src/wrap.rs` — the soft-wrap fixture `T081`'s own investigation flagged as missing

`tapes/_soft-wrap-check.tape`'s header records this directly: reaching a
naturally-long line (`status_line.rs`'s own 198-206-character doc comments)
costs "~500 scripted `Down` presses for no benefit over a purpose-built
fixture," so that investigation used an uncommitted
`/tmp/soft-wrap-fixture.rs` instead and said so. `src/wrap.rs` is that
fixture, committed: two lines, 424 and 317 characters, both on screen with
zero scrolling, long enough to wrap at every width `tapes/_dimensions.tape`
calibrates (80/100/120/200 columns).

## `src/broken.py` — the syntax-error fixture

Two unclosed parens, on purpose (see the file's own header, which says so
in-band to stop anyone "fixing" it). Exercises the same thing
`crates/phosphor-buffer`'s tree-sitter integration has to handle honestly on
any real, mid-edit file: a parse that produces `ERROR` nodes rather than a
clean tree, which is what unseen-marker fallback (`T043`) and diagnostics
(`T040`) both have to degrade against rather than assume away.

## Seeding

`scripts/seed-fixtures.sh` runs `fixtures/seed/plan.scm` — 18 calls through
`phosphor --eval` (`T023`), **not a test-only backdoor**, exactly what
`docs/TASKS.md`'s `V006` line asks for. Each line is commented with the
capability, its phase and task, and (for the less obvious ones) why it is
shaped the way it is; `plan.scm`'s own header carries the full contract.

**Verified this session, by running every line individually before it went
into the committed plan** (including after `retry.rs`/`fetch.rs` were
rewritten to match `3a`/`2b`/`3b` — the plan was re-run against the final
files, not just the first draft), and then by running the whole plan
through the script:

```
$ bash scripts/seed-fixtures.sh
...
seed-fixtures: 18 expected refusal(s), 0 landed capability answer(s), 0 broken.
```

Every one of the 18 calls **decodes into the correct Action shape** — the
registry accepts it, `Action::from_call` builds the right variant — and
every one **refuses**, naming the task that builds the subsystem behind it:

| capability | task | phase |
|---|---|---|
| `start-session!`, `session-seam!` | `T057` | S6 |
| `turn-began!`, `turn-ended!` | `T050` | S6 |
| `session-prose!`, `tool-call-started!`, `tool-call-progress!`, `tool-call-completed!` | `T054` | S6 |
| `declare-regions!`, `mark-seen!` | `T041` | S5 |
| `declare-review-block!` | `T053` | S6 |
| `place-watch!` | `T077` | S8 |
| `start-thread!` | `T068` | S7 |
| `notify!` | `T067` | S7 |

This is the "which of those verbs the CLI door actually exposes today"
check the task asked for, run for real rather than inferred from
`action.rs`: **all 15 distinct capabilities above are exposed and decode
correctly; none can act yet**, because none of `T041`/`T050`/`T053`/`T054`/
`T057`/`T067`/`T068`/`T077` exist. The two failure modes that would show a
problem — an `Unknown` capability (a typo against the registry) or a
`TypeMismatch`/`ArityMismatch` (a wrong shape) — were both provoked
deliberately this session to see what they look like (`mark-seen!
"not-a-target"` → `#refused · Error: TypeMismatch: ...`, exit 1;
`(mark-seen!)` → `#refused · Error: ArityMismatch: ...`, exit 1), so
`scripts/seed-fixtures.sh` can tell the two apart from the expected
`(#refused "not built yet — T0xx builds it")` shape (exit 0) without
guessing at the format.

**Load-bearing finding, worth carrying forward:** `--eval`'s exit code
alone cannot distinguish "the editor refused this" from "this crashed" —
both `(mark-seen! (hash "kind" "region" "id" 3))` (a well-formed call,
editor-refused) and a bare `(+ 1 2)` (nothing to refuse) exit `0`, because
`--eval` evaluates successfully either way; the refusal is *data* the
evaluation returned, not a failure of the evaluation itself (`action.rs`'s
own "a refusal is not an error"). Only a genuine Steel-level error (unbound
identifier, bad shape, wrong arity) exits `1`. A seeding script — or a tape
— that checks `$?` to decide whether seeding worked will be wrong the
moment `T041` lands and a call starts *succeeding*: success and "refused,
but evaluated fine" both exit `0`. `scripts/seed-fixtures.sh` checks the
printed value instead, for exactly this reason.

## Determinism

The three things `V006`'s brief names, and how each is met:

- **Fixed timestamps.** Not applicable yet, and flagged rather than
  papered over: none of the Action payloads reachable today
  (`crates/phosphor-core/src/action.rs`, checked this session) carry a
  wall-clock field — no `RegionSpec`, `FileGroup`, or `Session*` payload
  takes an explicit "when." Whatever stamps a region's or a turn's time is
  presumably internal to the store/session implementation `T041`/`T050`
  build, not something this door-level seed plan can pin from the outside.
  **Residue:** when `T041`/`T044`/`T050` land, whoever builds them needs to
  either accept a caller-supplied logical/wall clock (so a fixture can pin
  it) or guarantee the persisted *order* is what downstream surfaces render
  rather than an absolute timestamp — otherwise "two machines, identical
  tapes" (`CP-5`'s own criterion) is false by construction the moment either
  machine's clock differs. Not this task's to solve; recorded so it isn't
  rediscovered cold at `CP-5`.
- **Sorted / fixed ordering.** `fixtures/seed/plan.scm` is a hand-authored,
  committed file read top to bottom — there is no directory walk or hash-map
  iteration in the seeding path to be non-deterministic in the first place.
  `scripts/seed-fixtures.sh` preserves that order exactly (a `while read`
  loop over the file, no sorting needed because nothing here was ever
  unordered).
- **No absolute paths in output.** Every path in `plan.scm` is workspace-
  relative (`"src/retry.rs"`, never `"fixtures/src/retry.rs"` or a `/Users/…`
  path); `start-session!`'s optional `cwd` is left absent so it defaults to
  "the workspace root" rather than naming this sandbox's home directory.
  `scripts/seed-fixtures.sh` greps the plan for a quoted string starting
  with `/` before running anything and refuses to proceed if it finds one —
  a mechanical guard, not a promise.

## Residue — what `V006` still needs, precisely

Read against the tree this session, not assumed:

1. ~~**`T041` (store core), then `T044` (persistence).**~~ **Both landed, and
   the store half of `V006` is met.** The history is worth keeping because
   each step corrected the one before it:

   `T041` made `declare-regions!` answer `6` and the fixture still held
   nothing — **`scripts/seed-fixtures.sh` runs one `phosphor --eval` process
   per line**, so the store a declaration wrote to was gone before the next
   line started and line 16 marked two spans in an empty store, answering `0`.
   This entry had read *"cannot persist anything"*, and the operative word
   turned out to be **persist**, not *declare*.

   `T044` is that persistence, and it took regions with it for the reason
   `T041` found: a seen flag refers to a region, and if the regions are gone
   the flag has no subject. The two `mark-seen!` lines answer `1` now — they
   find, in a *different process*, the regions an earlier line wrote. The
   script's own closing paragraph said the opposite for a phase and is
   rewritten to say what is true.

   Running the script for the first time since `T100` also found two bugs in
   it — it aborted on its own first line under `set -e`, and its classifier
   still matched the pre-`T100` refusal shape. Both fixed there.
2. **`T050`/`T054`/`T057` (ACP session, transcript, session lifecycle,
   `S6`).** The eight session-shaped calls in the seed plan
   (`start-session!` through `session-seam!`) need these before the "canned
   transcript" half of this task is real rather than a verified-but-inert
   call sequence.
3. **`T053` (review blocks, `S6`)** for `declare-review-block!`.
4. **`T068` (threads, `S7`)** for `start-thread!`.
5. **`T067` (inbox, `S7`)** for `notify!`.
6. **`T077` (watches, `S8`)** for `place-watch!`.
7. ~~**The workspace-root question above.**~~ **Answered by `T044`**, and in
   this tree's favour: state is keyed on the canonical path of the directory
   the editor started in, never on VCS identity, so `fixtures/` is its own
   workspace root exactly as this file assumed. See the section above.
8. **Whichever of `T044`/`T050` ends up stamping time** needs to do so in a
   way two machines agree on — see "Determinism" above. **Still open, and
   `T044` did not close it**: the seen journal persists *what* and not
   *when*, so nothing it writes can differ between machines — which removes
   the store from the list of suspects without answering the question for the
   session subsystems that will carry a clock.

9. ~~**The fixed point.**~~ **Asserted, 2026-08-18.**
   `scripts/seed-determinism.sh` (`just seed-determinism`) seeds two clean
   `XDG_STATE_HOME`s from the same plan and the same tree, then asks each the
   same queries and diffs the answers. They agree: four unseen regions — six
   declared, two marked seen — with the same ids in the same order.

   This is what `CP-5`'s tapes stand on. A capture of the unseen picker is
   evidence about the *editor* only if the store behind it is the same store
   every time it is made; otherwise the pixel diff is measuring the seed.

   **What it does not claim** is "identical on two machines", which one machine
   cannot check. What it removes is the only cause of drift this repository
   controls — a seed that varied run to run here could never agree across two.
   The remainder is item 8 above, and it is about clocks in subsystems that do
   not exist yet.

   Two ways it can fail, and both were pressed: an extra region declared into
   one home only produces a diff naming the count and the id, and a seed that
   lands *nothing* is reported as a failure rather than as agreement — because
   two empty stores agree perfectly and prove nothing, which is exactly how
   this check would rot the day the plan stops landing anything.

None of this is `V006` marking itself done. `docs/TASKS.md`'s checkbox for
`V006` stays unchecked — the store half is met and the session half is not,
and a half-met checkbox is the kind of half-truth this file exists to avoid.
This is the residue list the next window reads instead of rediscovering the
gap from scratch.
