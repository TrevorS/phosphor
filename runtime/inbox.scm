;; inbox.scm — screen 5c, everything claude said (T067).
;;
;; **the third proof that the escape hatch is sufficient**, after arch.scm and
;; dashboard.scm. every row is `view/spans`, so 5c adds zero lines to
;; phosphor-ui — and like the dashboard's it is *live*: the rows are the `inbox`
;; query, and the query is a merge over three stores rather than a store of its
;; own.
;;
;; **unread is not a column this file owns.** CP-8a asks that unread *derive*
;; from seen-state rather than duplicate it, and the merge on the rust side is
;; where that happens: an ask is unread while it is pending, a block while any
;; of its regions is, a note by its own bit — the one row with nowhere else for
;; the fact to live. this file draws what it is told and keeps no count.
;;
;; **the times are relative and 5c's are not, and that is a deviation rather
;; than an oversight.** the mockup draws `2m` for the newest row and `14:41`
;; for the older three; nothing in this tree can render the second half —
;; there is no timezone-aware clock in the dependency graph, and adding one to
;; format a timestamp is not a trade T067 makes. relative orders the rows
;; identically and says the same thing about recency.

(define (inbox/run text tone) (view/run text tone 'plain))
(define (inbox/row tint . runs) (view/span-row runs tint))

;; 5c's kind column, padded to the widest of the three the mockup draws.
(define (inbox/pad text width)
  (if (>= (string-length text) width)
      text
      (inbox/pad (string-append text " ") width)))

;; §1: each colour names exactly one actor or state. the glyph and the tone are
;; the *severity*, which `notify` carries as one flag and the other two rows
;; derive — an ask is always attention (it is on the list because it is
;; waiting), a block is always claude's own news.
(define (inbox/glyph severity kind)
  (cond
   [(equal? kind "needs input") "!"]
   [(equal? kind "review ready") "✻"]
   [(equal? severity "trouble") "✕"]
   [(equal? severity "attention") "!"]
   [else "·"]))

(define (inbox/tone severity kind)
  (cond
   [(equal? kind "needs input") 'attention]
   [(equal? kind "review ready") 'claude]
   [(equal? severity "trouble") 'trouble]
   [(equal? severity "attention") 'attention]
   [else 'meta]))

;; `2m`, `9m`, `1h`. absent for the two kinds that carry no clock — an ask's age
;; is the turn's and a block's is its regions', and neither is a fact this row
;; has. a column that guessed would be the kind of almost-true the build spends
;; its lints avoiding.
;; **`number?` and not `(not …)`.** an absent field crosses the wire as
;; `Value::Null` and arrives in scheme as `#<void>`, which is neither `#false`
;; nor a number — so `(not seconds)` lets it through and `(< seconds 60)` raises
;; `expected real numbers, found: #<void>`, which the surface reports as a
;; failed float rather than a missing column. asking what it *is* rather than
;; what it is not is the check that holds for both spellings of absent.
(define (inbox/age seconds)
  (cond
   [(not (number? seconds)) ""]
   [(< seconds 60) "now"]
   [(< seconds 3600) (string-append (number->string (quotient seconds 60)) "m")]
   [else (string-append (number->string (quotient seconds 3600)) "h")]))

(define (inbox/line item selected)
  (let* ([kind (hash-ref item "kind")]
         [severity (hash-ref item "severity")]
         [unread (hash-ref item "unread")]
         [age (inbox/age (hash-try-get item "age"))])
    (inbox/row
     (if selected 'selection void)
     (inbox/run (string-append (inbox/glyph severity kind) " ") (inbox/tone severity kind))
     (inbox/run (inbox/pad kind 14) 'meta)
     (inbox/run (hash-ref item "title") 'text)
     (inbox/run (string-append "  " age) 'meta)
     ;; §2's check, and the one place this surface spends it. `seen ✓` rather
     ;; than a blank column, so a read row says so instead of merely lacking
     ;; something.
     (inbox/run (if unread "" "  seen ✓") 'meta))))

;; **`selected` is a row index, passed at open and on every navigation key**,
;; not read live. `view/spans` is `T063`'s escape hatch and a float is composed
;; once (`layer.surface`'s own doc: *"a float is a snapshot of an answer"*) — so
;; `j`/`k` over 5c re-runs `open-inbox` with the new index rather than mutating
;; anything already drawn. One extra query per keystroke over a list short
;; enough that the cost is not worth measuring, and it is what keeps this
;; screen at zero new lines of `phosphor-ui`.
;;
;; **`selected` may arrive as `#<void>`, not `#false`.** `hash-try-get` on an
;; absent key and a wire `Null` both cross that way — `number?` is the check
;; that holds for both, the same lesson `inbox/age` already learned.
(define (inbox/rows selected)
  (let ([at (if (number? selected) selected 0)])
    (let loop ([items (inbox)] [i 0])
      (cond
       [(and (null? items) (= i 0))
        (list (inbox/row void (inbox/run "nothing yet — claude has not said anything" 'meta)))]
       [(null? items) '()]
       [else (cons (inbox/line (car items) (= i at)) (loop (cdr items) (+ i 1)))]))))

;; informational. 5c is something you read; it asks nothing and is not in front
;; of anything, so `needs-you`'s amber would be spent on a screen with no
;; question on it — even though one of its *rows* is a question.
(define-float-surface!
  "inbox"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"inbox\" \"everything claude said\")
                 (view/spans (inbox/rows (hash-try-get args \"selected\")))
                 (view/key-hints 'footer
                                 (list (view/key-hint \"↵\" \"open\")
                                       (view/key-hint \"s\" \"mark seen\")
                                       (view/key-hint \"esc\" \"close\")))))")

;; `:inbox` — 5c's own label, and the door a person has.
(ex-set! "inbox" "everything claude said — :inbox"
         (lambda (rest bang)
           (key/run (key/cmd "open-inbox"))))

;; `:notify` — the producer, so 5c can be reached from a keyboard.
;;
;; **the real producer is the agent**, over MCP, where severity is the one flag
;; the task's own line names. this is here for the reason every other one-off ex
;; command in this directory is: a screen nothing can put a row on is a screen
;; nothing can check.
;;
;; `:notify! <text>` is the banged form and posts at `trouble`; plain is `info`.
;; two severities rather than three from the keyboard, because a person typing a
;; note to themselves is either making a remark or raising an alarm, and
;; `attention` is what claude uses when *he* wants your eyes.
(ex-set! "notify" "post a note to the inbox — :notify <text>, :notify! for trouble"
         (lambda (rest bang)
           (key/run (key/cmd "notify"
                             "severity" (if bang "trouble" "info")
                             "title" rest))))
