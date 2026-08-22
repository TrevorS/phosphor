;; dashboard.scm — `:cn` and the session dashboard, screens 7d, 5d and 2d (T057).
;;
;; **the second proof that the escape hatch is sufficient**, after arch.scm.
;; every row here is `view/spans`, so this whole surface adds zero lines to
;; phosphor-ui — and it is a better test of the claim than `:arch` was, because
;; this one is *live*: the rows change with the session where the diagram's
;; numbers only change with the store.
;;
;; **2d is the same screen a third time.** "opening mid-task — what you see
;; attaching to a repo where claude's been busy · state, not splash" is 7d with
;; a session attached and something unseen: the session row says what is running
;; instead of "none running", an unseen row appears because there is something
;; to be unseen, and the footer offers `]u` first because the next unseen region
;; is what you came back for. one `dash/rows` answers all three, which is the
;; claim the three mockups make by being the same layout.
;;
;; **two of 2d's five rows are not this task's to draw.** `vcs  jj · trunk@a4f2
;; · clean` needs `vcs-status`, which is T071, and `last  cargo test ✓ 34
;; passed` needs the timeline, which is T073 — both answer `NotYetImplemented`
;; today. the same is true of 7d's `repo` and `history` rows. they are absent
;; rather than stubbed: a row reading `vcs —` would be this file claiming to
;; have looked.
;;
;; 7d and 5d are one screen with different data, and that is not a shortcut —
;; it is what the two drawings say. 7d is "session / none running" and 5d is
;; "session / none — running agents found: …". the difference is whether
;; discovery found anything, and `discover-sessions` answers a list.
;;
;; **it answers an empty one, and will until v1.5.** the two rows 5d draws are a
;; tmux pane and a headless socket; reaching the first needs tmux control mode
;; and the second needs a socket transport, and T050's client speaks stdio to a
;; child it owns. so the surface renders 7d faithfully and 5d's list is the
;; branch nothing takes yet — written anyway, because the day discovery answers
;; something this file already knows what to do with it.

;; ---------------------------------------------------------------------------
;; helpers
;; ---------------------------------------------------------------------------

(define (dash/run text tone) (view/run text tone 'plain))
(define (dash/row . runs) (view/span-row runs void))

;; 7d's two-column layout: a meta label, then the fact. the label column is
;; padded to the widest of the four the mockup draws (`session`), which is what
;; makes the facts line up without a table primitive.
(define (dash/pad text width)
  (if (>= (string-length text) width)
      text
      (dash/pad (string-append text " ") width)))

(define (dash/field label value)
  (dash/row (dash/run (dash/pad label 10) 'meta)
            (dash/run value 'text)))

;; ---------------------------------------------------------------------------
;; the rows
;; ---------------------------------------------------------------------------

;; what the statusline would say, said in a sentence. §5's six states are the
;; vocabulary; this is the one place they are spelled for a *reader* rather than
;; drawn as a glyph, because 7d has room and the strip does not.
(define (dash/session-line state attached)
  (cond
   [(equal? state "none") "none running"]
   [(equal? state "idle") (string-append "idle · " (or attached "attached"))]
   [(equal? state "working") "working"]
   [(equal? state "waiting") "waiting on you"]
   [(equal? state "paused") "paused"]
   [(equal? state "lost") "lost — :reattach"]
   [else "none running"]))

;; 2d's unseen row — "6 regions in 2 files — ✻ retry logic, review ready".
;;
;; the count is the store's; the file count and the title come from the newest
;; review block, because "review ready" is a claim about a *block* and 2d draws
;; the two together. with regions but no block the row says what it knows and
;; stops, which is the honest half: something arrived, nobody has said it is a
;; review yet.
(define (dash/plural n one many)
  (string-append (number->string n) " " (if (= n 1) one many)))

(define (dash/unseen-line count blocks)
  (if (null? blocks)
      (dash/plural count "region" "regions")
      (let* ([block (car (reverse blocks))]
             [files (length (hash-ref block "files"))])
        (string-append (dash/plural count "region" "regions")
                       " in " (dash/plural files "file" "files")
                       " — ✻ " (hash-ref block "title")
                       ", review ready"))))

(define (dash/rows)
  (let* ([session (session)]
         [state (if (hash? session) (hash-try-get session "state") "none")]
         [attached (if (hash? session) (hash-try-get session "attached") #false)]
         [unseen (hash-ref (arch) "unseen")]
         [found (discover-sessions!)])
    (append
     (list
      ;; 7d's header pair. the version is the editor's own; what terminal it is
      ;; running in is not a question any capability answers yet, so it is not
      ;; claimed — a row that guessed would be the kind of almost-true this
      ;; build spends its lints avoiding.
      (dash/field "phosphor" "v0.1")
      (dash/field "session" (dash/session-line state attached)))
     ;; 2d's third row, and only when there is one. an `unseen  0 regions` row
     ;; on a cold start would be 7d claiming a fact 7d does not draw.
     (if (> unseen 0)
         (list (dash/field "unseen" (dash/unseen-line unseen (review-blocks))))
         (list))
     ;; 5d's list, when there is one.
     (if (null? found)
         (list)
         (cons (dash/row (dash/run "" 'meta))
               (map (lambda (agent)
                      (dash/row (dash/run "⠿ " 'transient)
                                (dash/run (or (hash-try-get agent "label") "claude") 'text)
                                (dash/run "   ↵ adopt" 'meta)))
                    found)))
     (list
      (dash/row (dash/run "" 'meta))
      ;; **the footer follows the screen.** 7d's caption is "three verbs, then
      ;; out of the way" and 2d's is "state, not splash", and the difference
      ;; shows up here: with nothing running the verbs are how you start, and
      ;; with a session already busy the first thing offered is the unseen work
      ;; you came back to. `dismiss-dashboard-hint` is the "then" in both.
      (if (equal? state "none")
          (dash/row (dash/run ":e" 'you) (dash/run " edit · " 'meta)
                    (dash/run ":cn" 'you) (dash/run " start claude · " 'meta)
                    (dash/run ":f" 'you) (dash/run " find file" 'meta))
          (dash/row (dash/run "]u" 'you) (dash/run " next unseen · " 'meta)
                    (dash/run ":transcript" 'you) (dash/run " transcript · " 'meta)
                    (dash/run ":claude" 'you) (dash/run " claude · " 'meta)
                    (dash/run ":e" 'you) (dash/run " edit" 'meta)))))))

;; the float. informational — 7d is not in front of anything and asks nothing —
;; and 5d's own footer sentence, which is the thesis of the whole screen: the
;; editor works without a session.
(define-float-surface!
  "dashboard"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"phosphor\" \"session\")
                 (view/spans (dash/rows))
                 (view/key-hints 'footer
                                 (list (view/key-hint \"esc\" \"skip — the editor works fine without one\")))))")

;; ---------------------------------------------------------------------------
;; the commands
;; ---------------------------------------------------------------------------

;; `:cn` — 7d and 5d both draw it as the verb that starts a session. the
;; argument is the agent's command, which is `agent-command`'s value said once
;; rather than set: `(set-option! …)` is the persistent form and this is the
;; one-off.
(ex-set! "cn" "start a claude session — :cn <command>"
         (lambda (rest bang)
           (key/run (key/cmd "start-session" "agent" rest))))

(ex-set! "dash[board]" "the session dashboard — 7d"
         (lambda (rest bang)
           (key/run (key/cmd "open-dashboard"))))

(ex-set! "detach" "detach, leaving the session"
         (lambda (rest bang)
           (key/run (key/cmd "detach-session"))))

(ex-set! "end[-session]" "end the session; banged, mid-turn too"
         (lambda (rest bang)
           (key/run (key/cmd "end-session" "force" bang))))

(ex-set! "adopt" "adopt a discovered session — :adopt <handle>"
         (lambda (rest bang)
           (key/run (key/cmd "adopt-session" "handle" rest))))

(ex-set! "disc[over]" "look for running agents — 5d"
         (lambda (rest bang)
           (key/run (key/cmd "discover-sessions"))))

(ex-set! "att[ach]" "attach to a session endpoint — :attach <command>"
         (lambda (rest bang)
           (key/run (key/cmd "attach-session" "endpoint" rest))))

(ex-set! "seam" "record a seam — :seam paused|lost|resumed"
         (lambda (rest bang)
           (key/run (key/cmd "session-seam"
                             "kind" (if (equal? rest "") "lost" rest)))))

(ex-set! "hint" "dismiss 7d's hint line"
         (lambda (rest bang)
           (key/run (key/cmd "dismiss-dashboard-hint"))))
