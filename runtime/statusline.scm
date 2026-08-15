;; statusline.scm — what the bottom row says, and what it gives up first.
;;
;; the statusline is composed here, not in rust. rust hands this file one
;; view-model — what is true right now — and this file answers a view tree: which
;; segments, in what order, joined how, and which of them go first when the
;; terminal is too narrow to hold them all. rust draws what comes back and has no
;; opinion about any of it.
;;
;; that is the whole of Q12 on one surface: does it produce pixels? rust. does it
;; decide which pixels? here.
;;
;; the view-model is a hash. every field is a fact, never a decision:
;;
;;   mode        "normal" | "insert" | "visual" | "paused" | "repl" | …
;;   surface     the surface's own name, where a file would go — 6b draws "steel"
;;   file        (hash "path" … "dirty" …), or void
;;   session     "none" | "idle" | "working" | "waiting" | "paused" | "lost"
;;   since       when the current turn started, for the elapsed counter
;;   ask_pending a queued ask is waiting (Q9)
;;   unseen      unseen regions in this file
;;   vcs         "jj ✓", or void outside a repo
;;   server      "rust-analyzer ✓", or void where the language declares none
;;   cursor      (hash "line" … "col" …), or void
;;   hints       the keys this surface teaches
;;
;; the nodes come from `view/…`, one per node kind, generated from the protocol
;; itself — so a node kind that lands in rust is callable here the same day, and
;; there is no list of them in this file to go stale. an absent optional is
;; `void`; `#false` is a value the protocol would try to decode.
;;
;; two house rules, both from how the boot compiles this file — each top-level
;; form on its own, one after another:
;;
;;   * a name has to be defined above the form that uses it. helpers first, the
;;     line itself last.
;;   * to change something at :repl, mutate it — `status-set!`,
;;     `status-order-set!`, `status-ladder-set!` — rather than writing a second
;;     `define`. a `define` binds the name for forms compiled after it, and this
;;     file is already compiled, so it would go on reading the old one. the
;;     exception is `phosphor/status-line` itself, which rust asks for by name
;;     on every composition: redefine that and the next frame is whatever you
;;     said.

;; ---------------------------------------------------------------------------
;; the ladder
;; ---------------------------------------------------------------------------

;; Design Language §11: "narrow terminals drop, never squeeze." this is the
;; order things are given up in — first in the list goes first — and it is data
;; because it is a judgement, not a mechanism. rust applies one rung at a time
;; until the row fits, and stops the moment it does: nothing is dropped while
;; there is room for it.
;;
;; §11 names five rungs: counters → jj → cursor pos → session prose (glyph
;; stays) → mode word (initial stays). `8d` draws three more below them, on the
;; file. `hints` is ours and is in neither: a surface's keys are the first thing
;; to give up, because the surface teaches them again in its own footer.
;;
;; two rungs are contractions rather than drops — `6 unseen` becomes `●6`, `✻
;; claude idle` becomes `✻` — and a segment that contracts never drops, which is
;; how §11's last-standing set (`✻` / `●n` / `!`) is written here.
;;
;; `server` is one of those contractions, and it is a contraction for a reason
;; the others do not have. 7c draws `rust-analyzer ✓` where 8d draws `jj ✓`, so
;; it sits beside the vcs chip — but a *failed* server says so here and nowhere
;; else in the editor, and a rung that dropped it would make the one place that
;; information exists the first thing a narrow terminal throws away. What goes
;; is the failure's own sentence; `name ✗` stays.
(define phosphor/status-ladder
  '(hints counter-words server vcs cursor session-prose mode-word file-path dirty file))

;; §5: "mode chip (bg = mode color, the only inverted text on screen)". the word
;; and the actor field it sits on, per mode. a surface gets a chip of its own —
;; 6b draws REPL on the steel field, not NORMAL on claude-green — because the
;; chip names *what has the frame*, not only an edit mode.
;;
;; a mode nobody listed here still draws: its own name, upper-cased, on ordinary
;; text. an invented colour would be a claim about an actor.
(define phosphor/status-chips
  (hash "normal" (list "NORMAL" 'claude)
        "insert" (list "INSERT" 'you)
        "visual" (list "VISUAL" 'transient)
        "paused" (list "PAUSED" 'attention)
        "repl" (list "REPL" 'steel)))

;; ---------------------------------------------------------------------------
;; segments
;; ---------------------------------------------------------------------------

;; a segment is (list rung contracted full join):
;;
;;   rung        which rung of the ladder gives it up, or #false for never
;;   contracted  the narrower form to fall back to, or #false to drop instead
;;   full        the form at width
;;   join        'bar, 'gap or 'none — the separator that rides *in front* of it
;;
;; the separator rides inside the segment on purpose: drop the segment and its
;; separator goes with it, so a shed line never leaves a double space or a bar
;; with nothing on one side of it.
(define (status/segment rung contracted full join)
  (list rung contracted full join))

(define (status/rung-of segment) (list-ref segment 0))
(define (status/contracted-of segment) (list-ref segment 1))
(define (status/full-of segment) (list-ref segment 2))
(define (status/join-of segment) (list-ref segment 3))

;; is this field present? absent optionals arrive as void, and #false is a
;; perfectly good value for a flag, so the two are not the same question.
(define (status/present? field)
  (and (not (void? field)) field))

;; where `rung` sits on the ladder, or #false if the ladder does not name it —
;; in which case the segment never sheds at all.
(define (status/rung name)
  (let loop ([rungs phosphor/status-ladder] [at 0])
    (cond
      [(null? rungs) #f]
      [(equal? (car rungs) name) at]
      [else (loop (cdr rungs) (+ at 1))])))

;; §5: "segments join with a thin bar │ in meta-gray" — but only inside the
;; counter group. every drawing (1a, 9c, 8c, 8d) puts a plain gap between the
;; session state and the counters, and CP-1 settled it for the drawings.
(define (status/joined join node)
  (cond
    [(equal? join 'bar)
     (view/line (list (view/spacer 1) (view/divider) (view/spacer 1) node))]
    [(equal? join 'gap) (view/line (list (view/spacer 1) node))]
    [else node]))

;; `full` wrapped in the rung that gives it up, or `full` alone when the ladder
;; does not name that rung.
(define (status/shed rung contracted full)
  (let ([at (status/rung rung)])
    (if at (view/shed at (or contracted void) full) full)))

;; one segment as a node: its separator, then its shed wrapper.
(define (status/draw segment join)
  (let ([contracted (status/contracted-of segment)])
    (status/shed (status/rung-of segment)
                 (and contracted (status/joined join contracted))
                 (status/joined join (status/full-of segment)))))

;; a group of segments, left to right. the first one takes `leading` instead of
;; its own separator — a bar against the edge of a group is a bar with one side
;; missing.
(define (status/group segments leading)
  (if (null? segments)
      '()
      (cons (status/draw (car segments) leading)
            (map (lambda (segment) (status/draw segment (status/join-of segment)))
                 (cdr segments)))))

;; the counter group's first surviving member joins the session with a gap
;; rather than a bar — whichever member that turns out to be once the ladder has
;; had its say.
(define (status/first-joins-with-a-gap segments)
  (if (null? segments)
      '()
      (cons (status/segment (status/rung-of (car segments))
                            (status/contracted-of (car segments))
                            (status/full-of (car segments))
                            'gap)
            (cdr segments))))

;; ---------------------------------------------------------------------------
;; what the segments say
;; ---------------------------------------------------------------------------

;; §11's last rung: the mode word contracts to its initial (`8d` draws `N`).
(define (status/initial word)
  (if (> (string-length word) 0) (substring word 0 1) word))

;; `src/retry.rs` → `retry.rs` (`8d`).
(define (status/basename path)
  (let loop ([at (string-length path)])
    (cond
      [(zero? at) path]
      [(equal? (substring path (- at 1) at) "/") (substring path at (string-length path))]
      [else (loop (- at 1))])))

;; the name and its mark — `rust-analyzer ✓`, `tsserver ✗` — off the front of a
;; server chip whose tail is a failure's own sentence. §11 contracts it to this
;; rather than dropping it: the sentence is the OS's words about why a server is
;; not running, which is worth the row it costs but not the whole row.
(define (status/chip-head chip)
  (let loop ([at 0] [spaces 0])
    (cond
      [(>= at (string-length chip)) chip]
      [(equal? (substring chip at (+ at 1)) " ")
       (if (>= spaces 1) (substring chip 0 at) (loop (+ at 1) (+ spaces 1)))]
      [else (loop (+ at 1) spaces)])))

;; `12:1` (`1a`, `8e`).
(define (status/place cursor)
  (string-append (number->string (hash-try-get cursor "line"))
                 ":"
                 (number->string (hash-try-get cursor "col"))))

;; `C-c buffer · tab complete · q close` — §6: the midline dot goes inside a
;; fact, and a hint row is one fact.
(define (status/hint-text hints)
  (let loop ([hints hints] [out ""])
    (if (null? hints)
        out
        (let* ([hint (car hints)]
               [text (string-append (hash-try-get hint "key") " " (hash-try-get hint "verb"))])
          (loop (cdr hints)
                (if (equal? out "") text (string-append out " · " text)))))))

;; ---------------------------------------------------------------------------
;; the left group: what has the frame, and what is in it
;; ---------------------------------------------------------------------------

;; the chip never goes — it is the one segment that is always on screen (§5).
(define (status/chip vm)
  (let* ([mode (hash-try-get vm "mode")]
         [chip (or (hash-try-get phosphor/status-chips mode)
                   (list (string-upcase mode) 'text))]
         [word (car chip)]
         [tone (car (cdr chip))])
    (list (status/segment 'mode-word
                          (view/mode-chip (status/initial word) tone)
                          (view/mode-chip word tone)
                          'none))))

;; the file and its dirty flag, or — on a surface with no buffer — the surface's
;; own name. `2d`'s dashboard draws neither and goes straight to the spring.
(define (status/file vm)
  (let ([file (status/present? (hash-try-get vm "file"))]
        [surface (status/present? (hash-try-get vm "surface"))])
    (cond
      [file
       (let ([path (hash-try-get file "path")])
         (append
          ;; two rungs on one segment: the path contracts to its basename first
          ;; (`8d`), and the whole thing goes a rung later. the inner rung is
          ;; wrapped directly by the outer one — the ladder reads a chain of shed
          ;; wrappers, and anything between them would hide the inner rung — so
          ;; this segment carries its own leading gap and takes no separator.
          (list (status/segment
                 'file
                 #false
                 (status/shed 'file-path
                              (status/joined 'gap (view/file-label (status/basename path) #false))
                              (status/joined 'gap (view/file-label path #false)))
                 'none))
          (if (hash-try-get file "dirty")
              ;; §1: attention — waiting, paused, *dirty*.
              (list (status/segment 'dirty #false (view/label "[+]" 'attention 'plain) 'gap))
              '())))]
      [surface (list (status/segment #false #false (view/label surface 'text 'plain) 'gap))]
      [else '()])))

;; ---------------------------------------------------------------------------
;; the right group: the session, then the counters
;; ---------------------------------------------------------------------------

;; §5: "session state is always present and truthful". no session is not a
;; failure and draws nothing at all.
(define (status/session vm)
  (let ([state (hash-try-get vm "session")]
        [since (hash-try-get vm "since")])
    (if (equal? state "none")
        '()
        ;; §11: the prose goes, the glyph stays.
        (list (status/segment 'session-prose
                              (view/session state since #false)
                              (view/session state since #true)
                              'gap)))))

;; Q9: an ask queues rather than interrupting, and this flag is its only
;; notification — so it is in the last-standing set and has no rung. never drawn
;; beside `waiting`, whose own glyph is already `!`.
;;
;; it joins with a gap, not a bar: CP-1 put the bars inside the counter group,
;; and a flag about the session belongs to the session.
(define (status/ask vm)
  (if (and (hash-try-get vm "ask_pending")
           (not (equal? (hash-try-get vm "session") "waiting")))
      (list (status/segment #false #false (view/glyph 'needs-you 'attention) 'gap))
      '()))

;; the counter group — the only place a `│` appears.
(define (status/counters vm)
  (let ([unseen (hash-try-get vm "unseen")]
        [server (status/present? (hash-try-get vm "server"))]
        [vcs (status/present? (hash-try-get vm "vcs"))]
        [cursor (status/present? (hash-try-get vm "cursor"))])
    (status/first-joins-with-a-gap
     (append
      ;; §11's first rung: the counters lose their words, not their glyphs.
      (if (> unseen 0)
          (list (status/segment 'counter-words
                                (view/counter 'unseen unseen void 'meta)
                                (view/counter 'unseen unseen "unseen" 'meta)
                                'bar))
          '())
      (if server
          (list (status/segment 'server
                                (view/label (status/chip-head server) 'meta 'plain)
                                (view/label server 'meta 'plain)
                                'bar))
          '())
      (if vcs
          (list (status/segment 'vcs #false (view/label vcs 'meta 'plain) 'bar))
          '())
      (if cursor
          (list (status/segment 'cursor
                                #false
                                (view/label (status/place cursor) 'meta 'plain)
                                'bar))
          '())))))

;; the keys this surface teaches — 6b's `C-c buffer · tab complete · q close`.
;;
;; one label rather than `view/key-hints`, and that is a limit rather than a
;; choice: the key-hints node draws only inside a float footer until T034 builds
;; the keymap surface. swap the constructor when it lands; nothing else changes.
(define (status/hints vm)
  (let ([hints (hash-try-get vm "hints")])
    (if (null? hints)
        '()
        (list (status/segment 'hints
                              #false
                              (view/label (status/hint-text hints) 'meta 'plain)
                              'gap)))))

;; ---------------------------------------------------------------------------
;; the line
;; ---------------------------------------------------------------------------

;; the segments, by name. each one takes the view-model and answers a list of
;; segments — none, one, or several — so a segment that has nothing to say says
;; nothing rather than drawing an empty thing.
(define phosphor/status-segments
  (hash 'chip status/chip
        'file status/file
        'session status/session
        'ask status/ask
        'counters status/counters
        'hints status/hints))

;; §5, left to right: mode chip, file + dirty flag, spring, session state,
;; counters. drop a name to drop the segment; reorder them to reorder the line.
(define phosphor/status-left '(chip file))
(define phosphor/status-right '(session ask counters hints))

;; replace one segment, live. 6b's own idiom, one surface over:
;;
;;   (status-set! 'chip (lambda (vm) (list (status/segment #false #false
;;                                          (view/mode-chip "λ" 'steel) 'none))))
;;
;; **use these rather than a second `define`.** a top-level `define` of a name
;; that already exists binds it for forms compiled *after* it; the composition
;; is already compiled and goes on reading the old one. these mutate the table
;; the composition reads on every frame, so the next frame has it. the same
;; reason `keymap-set!` mutates rather than redefines.
;;
;; T033 routes these through a capability, so the cli and mcp doors reach the
;; same table; today they are reached from scheme alone.
(define (status-set! name make)
  (set! phosphor/status-segments (hash-insert phosphor/status-segments name make))
  void)

;; which segments are drawn, and in what order. `side` is 'left or 'right.
(define (status-order-set! side names)
  (if (equal? side 'left)
      (set! phosphor/status-left names)
      (set! phosphor/status-right names))
  void)

;; what gets given up first. see `phosphor/status-ladder`.
(define (status-ladder-set! rungs)
  (set! phosphor/status-ladder rungs)
  void)

;; one side's segments, in order. a name nothing is bound to contributes
;; nothing, which is how a segment is removed.
(define (status/segments names vm)
  (let loop ([names names] [out '()])
    (if (null? names)
        out
        (let ([make (hash-try-get phosphor/status-segments (car names))])
          (loop (cdr names)
                (append out (if make (make vm) '())))))))

;; the whole composition. the trailing space is the right margin every mockup
;; draws.
;;
;; redefining *this* takes effect immediately — rust asks for it by name every
;; time — so a whole statusline of your own is one form at :repl.
(define (phosphor/status-line vm)
  (view/line
   (append
    (status/group (status/segments phosphor/status-left vm) 'none)
    (list (view/spring))
    (status/group (status/segments phosphor/status-right vm) 'none)
    (list (view/spacer 1)))))
