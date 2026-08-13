;; V006 seed plan — a canned agent turn against fixtures/src/retry.rs and
;; fixtures/src/fetch.rs, expressed as phosphor --eval calls (T023), one per
;; line, in the fixed order scripts/seed-fixtures.sh runs them.
;;
;; NOT a test-only backdoor (docs/TASKS.md's V006 line): every form below is a
;; real capability call through the CLI door, in the same vocabulary Steel and
;; MCP share (invariant 2). Nothing here reaches past the door into the store.
;;
;; WORKSPACE ROOT: these paths ("src/retry.rs", "src/fetch.rs") are relative
;; to fixtures/ itself, not the repo root — fixtures/ is its own miniature
;; workspace (Q1: seen-state keys on the canonicalised *workspace* root, and
;; nothing says a workspace must be the repo). scripts/seed-fixtures.sh `cd`s
;; into fixtures/ before running any of these. This keeps every path here
;; identical to what the design mockups draw (`src/retry.rs:24`, "2 files"),
;; rather than "fixtures/src/retry.rs" — see fixtures/README.md.
;;
;; STATUS TODAY (verified by running every line, one at a time, this session
;; — see fixtures/README.md's table): every capability below is registered
;; and every form decodes to the right shape, but the store, review, session
;; and watch subsystems are S5/S6/S7/S8 — none built yet. So every line
;; refuses with `(#refused "not built yet — T0xx builds it")`, naming the
;; task that changes that. scripts/seed-fixtures.sh checks for exactly this
;; shape and flags anything else loudly: a different refusal or a decode
;; error means this file has drifted from the registry; an actual `#ok`
;; means a task landed and this plan is finally live.
;;
;; ORDER IS FIXED. This file is hand-authored, not generated from a directory
;; walk, so there is no filesystem iteration order to make non-deterministic
;; in the first place — the determinism V006 is asked for is that this list
;; never changes shape between two runs, which "committed and read top to
;; bottom" already guarantees.

;; 1. session — start-session! [S6 / T057]
;; Opens the canned turn. `cwd` is left absent (Null) on purpose — the field
;; means "the workspace root" (action.rs's own doc), and naming this
;; sandbox's absolute path here would be exactly the non-determinism V006
;; exists to remove.
(start-session! "claude-code")

;; 2. session — turn-began! [S6 / T050]
(turn-began! 1 "the retry loop doesn't cap delay after jitter - fix it")

;; 3. session — session-prose! [S6 / T054]
(session-prose! 1 "looked at retry_with_backoff — the backoff advances every non-Ok attempt but never capped against max_delay.")

;; 4. session — tool-call-started! [S6 / T054]
(tool-call-started! 1 1 "edit" "src/retry.rs")

;; 5. session — tool-call-progress! [S6 / T054]
(tool-call-progress! 1 "adding the max_delay cap after each backoff step")

;; 6. session — tool-call-completed! [S6 / T054]
;; +51/-3 on purpose, not invented — TUI Mockups.dc.html's `3b` ("jj
;; timeline") draws exactly this row: "○ 7c3d · claude  retry logic
;; +51 −3   seen ✓" (line ~897). A future tape matches the mockup's own
;; numbers without anyone having to make new ones up.
(tool-call-completed! 1 "capped delay at max_delay in src/retry.rs" 51 3)

;; 7. session — tool-call-started! [S6 / T054]
(tool-call-started! 1 2 "edit" "src/fetch.rs")

;; 8. session — tool-call-completed! [S6 / T054]
(tool-call-completed! 2 "wired fetch_json through retry_with_backoff" 12 2)

;; 9. region — declare-regions! [S5 / T041]
;; Six regions across the two files — the "retry logic — 2 files · 6
;; regions" count TUI Mockups.dc.html draws repeatedly (1a line ~1195, 4b
;; line ~733, 5c line ~653, 2d line ~1120). Spans line up with
;; fixtures/src/{retry,fetch}.rs as committed — see fixtures/README.md's
;; citation table for how each span was chosen and checked against the
;; file with a script, not guessed.
(declare-regions! (list (hash "path" "src/retry.rs" "span" (hash "start" (hash "line" 4 "column" 1) "end" (hash "line" 4 "column" 18)) "author" "claude") (hash "path" "src/retry.rs" "span" (hash "start" (hash "line" 6 "column" 1) "end" (hash "line" 10 "column" 2)) "author" "claude") (hash "path" "src/retry.rs" "span" (hash "start" (hash "line" 12 "column" 1) "end" (hash "line" 24 "column" 51)) "author" "claude") (hash "path" "src/fetch.rs" "span" (hash "start" (hash "line" 10 "column" 1) "end" (hash "line" 14 "column" 2)) "author" "claude") (hash "path" "src/fetch.rs" "span" (hash "start" (hash "line" 17 "column" 1) "end" (hash "line" 20 "column" 2)) "author" "claude") (hash "path" "src/fetch.rs" "span" (hash "start" (hash "line" 31 "column" 1) "end" (hash "line" 35 "column" 2)) "author" "claude")))

;; 10. session — session-prose! [S6 / T054]
(session-prose! 1 "both files now share the same policy; opened a review block.")

;; 11. review — declare-review-block! [S6 / T053]
(declare-review-block! "retry logic" (list (hash "path" "src/retry.rs" "spans" (list (hash "start" (hash "line" 4 "column" 1) "end" (hash "line" 4 "column" 18)) (hash "start" (hash "line" 6 "column" 1) "end" (hash "line" 10 "column" 2)) (hash "start" (hash "line" 12 "column" 1) "end" (hash "line" 24 "column" 51))) "annotation" "the backoff itself") (hash "path" "src/fetch.rs" "spans" (list (hash "start" (hash "line" 10 "column" 1) "end" (hash "line" 14 "column" 2)) (hash "start" (hash "line" 17 "column" 1) "end" (hash "line" 20 "column" 2)) (hash "start" (hash "line" 31 "column" 1) "end" (hash "line" 35 "column" 2))) "annotation" "the caller now goes through the shared policy")) "backoff now caps at max_delay, and fetch_json goes through it")

;; 12. session — turn-ended! [S6 / T050]
(turn-ended! 1 "capped backoff at max_delay; retry logic review block ready")

;; 13. watch — place-watch! [S8 / T077]
;; `6b`'s own line (TUI Mockups.dc.html:502) draws `(watch-place
;; "src/retry.rs:24" 'delay)` with a quoted symbol; `docs/OPEN-QUESTIONS.md`
;; and crates/phosphor-steel/tests/screen_6b.rs already record that as a
;; shape gap against the real Action (`expr: String`) — this line is the
;; corrected shape, a string, checked by running it this session.
(place-watch! (hash "kind" "explicit" "path" "src/retry.rs" "span" (hash "start" (hash "line" 24 "column" 1) "end" (hash "line" 24 "column" 1))) "delay")

;; 14. thread — start-thread! [S7 / T068]
;; Anchored at retry.rs:19-21 — three independent citations agree on this
;; exact span: the prompt-line anchor chip "⚓ src/retry.rs:19–21"
;; (TUI Mockups.dc.html:1317, screen `1c`), and `6c` ("Anchors survive the
;; rewrite")'s own thread virtual-text "⚓ thread · was
;; retry_with_backoff:19–21 · followed node fn:next_delay" (line 521) — `6c`
;; is the anchor-survival screen, so this is the thread this fixture models
;; the *before* state of. `3a` ("Anchored exchange", line 870) draws the
;; same exchange over what its own rendering visually groups as lines
;; 19-22 (the closing `}` shares the wavy-underline styling); the two
;; explicit numeric citations were preferred over that visual grouping.
;; Reply text is `3a`/`6c`'s own: "⚓ you · 2m  collapse these arms — use
;; the shared backoff helper" / "✻ claude · 1m  collapsed — error carried
;; in `last`, returned after the loop" (claude's reply is `T068`'s to seed
;; once threads exist, not this door's to invent as a second call).
(start-thread! (hash "kind" "explicit" "path" "src/retry.rs" "span" (hash "start" (hash "line" 19 "column" 1) "end" (hash "line" 21 "column" 38))) "collapse these arms — use the shared backoff helper")

;; 15. inbox — notify! [S7 / T067]
(notify! "attention" "backoff now capped at max_delay" "worth a look before merging" (hash "kind" "explicit" "path" "src/retry.rs" "span" (hash "start" (hash "line" 22 "column" 1) "end" (hash "line" 24 "column" 51))))

;; 16. region — mark-seen! [S5 / T041]
;; Explicit targets, not region ids — declare-regions! (line 9 above) does
;; not hand ids back to this door today, and a Target::Explicit ("the only
;; arm an agent can always use", request.rs) needs none. Two of the six
;; spans, matching the mockups' recurring "2 seen ✓" count (TUI
;; Mockups.dc.html:733).
(mark-seen! (hash "kind" "explicit" "path" "src/retry.rs" "span" (hash "start" (hash "line" 6 "column" 1) "end" (hash "line" 10 "column" 2))))

;; 17. region — mark-seen! [S5 / T041]
(mark-seen! (hash "kind" "explicit" "path" "src/fetch.rs" "span" (hash "start" (hash "line" 10 "column" 1) "end" (hash "line" 14 "column" 2))))

;; 18. session — session-seam! [S6 / T057] — bonus, not one of the "six
;; explicit capabilities" the stream comment in action.rs names (that's
;; #2-8, #9-12 above: TurnBegan/TurnEnded/SessionProse/ToolCallStarted/
;; ToolCallProgress/ToolCallCompleted). Included for 7b/7e's seam surfaces,
;; which want at least one recorded seam in the fixture too.
(session-seam! "resumed" "reattached after the tool boundary")
