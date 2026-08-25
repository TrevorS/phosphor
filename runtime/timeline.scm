;; timeline.scm — screen 3b, the jj timeline (T073).
;;
;; **live, like 5c's inbox.** the rows are the `timeline` query rather than a
;; snapshot passed in at open, so a change made while the float is up is a
;; change the float shows. that is what makes 3b a *view* of the repository
;; instead of a photograph of it.
;;
;; **an enhancement view, only when jj is present** — 3b's own subtitle. the
;; query answers an empty list in a git repository and in a bare directory, and
;; this file draws that as a sentence rather than an error, because CP-8c fails
;; if any message implies something is missing.

;; `3b` draws `@` for the working copy and `○` for everything else.
;;
;; **`○` is the mockup's glyph and `o` is what the template emits** — the
;; backend answers a boolean and this file spends the glyph, which keeps §2's
;; lexicon on this side of the barrier where the rest of it lives.
;; `view/run` takes three arguments; this is the two-argument helper every
;; other surface in this directory defines. See disk.scm's note for what
;; skipping it costs — an arity error that only fires when the float composes.
(define (timeline/run text tone) (view/run text tone 'plain))

;; a row is a `view/span-row`, not a bare list of runs — see disk.scm's note.
(define (timeline/row-of runs) (view/span-row runs void))

(define (timeline/glyph row)
  (if (eq? (hash-try-get row "working_copy") #true) "@" "○"))

;; the `+11 −18` pair, and it is absent rather than zeroed when nothing moved.
;;
;; 3b draws counts on three of its five rows and none on the two that are
;; descriptions of a state — `working copy — editing src/fetch.rs` and
;; `manual edit src/retry.rs`. a `+0 −0` on those would be four characters
;; saying nothing.
(define (timeline/stat row)
  (let ([added (hash-try-get row "added")]
        [removed (hash-try-get row "removed")])
    (if (and (number? added) (number? removed) (or (> added 0) (> removed 0)))
        (list (timeline/run (string-append "  +" (number->string added)) 'claude)
              (timeline/run (string-append " −" (number->string removed)) 'trouble))
        '())))

(define (timeline/before-at text)
  (let loop ([left (string->list text)] [head '()])
    (cond
     [(null? left) (list->string (reverse head))]
     [(char=? (car left) #\@) (list->string (reverse head))]
     [else (loop (cdr left) (cons (car left) head))])))

;; **the author, shown as the local part of the email.**
;;
;; 3b draws `· you` and `· claude`, and this build cannot honestly produce the
;; second: nothing creates a change per agent turn yet, so every change in a
;; real repository is authored by whoever configured jj. what is drawn is the
;; *recorded* author — truthful, and ready for the day a turn becomes a change.
;; inventing `claude` from a guess would be 3b's one claim no data supports.
(define (timeline/who row)
  (let ([author (hash-try-get row "author")])
    (if (string? author)
        (let ([at (timeline/before-at author)])
          (if (equal? at "") author at))
        "?")))

(define (timeline/row row)
  (append
   (list (timeline/run (timeline/glyph row) 'claude)
         (timeline/run (string-append " " (hash-try-get row "change")) 'text)
         (timeline/run (string-append " · " (timeline/who row) "  ") 'meta)
         (timeline/run (hash-try-get row "description") 'text))
   (timeline/stat row)))

;; **the empty case is a sentence, not a blank float.**
;;
;; a git repository and a bare directory both land here, and neither is broken.
;; CP-8c reads this line: *"does anything feel degraded or apologetic?"* — so it
;; says what is true and offers nothing.
;; the op log's rows — `3b`'s `o full op log`.
;;
;; **handed in rather than queried**, because the vocabulary declares `timeline`
;; and no `operations`. widening the wire to feed one float would be the
;; opposite of the rule the three doors are built on, so the arm reads them and
;; passes them through the surface args.
(define (timeline/op-row row)
  (list (timeline/run "· " 'meta)
        (timeline/run (hash-try-get row "operation") 'text)
        (timeline/run (string-append "  " (hash-try-get row "description")) 'meta)))

;; the cursor `3b` draws on the row you are on.
(define (timeline/mark rows selected)
  (let loop ([left rows] [at 0] [out '()])
    (if (null? left)
        (reverse out)
        (loop (cdr left)
              (+ at 1)
              (cons (cons (timeline/run (if (= at selected) "› " "  ") 'you) (car left)) out)))))

(define (timeline/rows args)
  (let ([ops (hash-try-get args "operations")]
        [selected (let ([n (hash-try-get args "selected")]) (if (number? n) n 0))])
    (cond
     ;; the op log, when the arm handed one over
     [(and (list? ops) (not (null? ops)))
      (map (lambda (op) (timeline/row-of (timeline/op-row op))) ops)]
     [else
      (let ([rows (timeline)])
        (if (null? rows)
            (list (timeline/row-of
                   (list (timeline/run "no changes to show — the timeline is jj's" 'meta))))
            (map timeline/row-of (timeline/mark (map timeline/row rows) selected))))])))

(define (timeline/footer)
  (view/key-hints 'footer
                  (list (view/key-hint "↵" "edit here")
                        (view/key-hint "d" "diff")
                        (view/key-hint "o" "full op log")
                        (view/key-hint "esc" "close"))))

;; **informational.** the timeline asks nothing; it is a place you go to look.
;; `:restore-change` — bring what was there to where you are.
;;
;; **an ex command rather than a key, because 3b draws no key for it.** the
;; footer is `↵ edit here · d diff · o full op log · esc`, and adding a fifth
;; would be this file editing the screen rather than building it. the verb is
;; real and declared, so it gets the door a person can type instead.
;;
;; **not the same verb as `↵`.** `edit-at-change` moves *where you are*;
;; `restore-change` brings *what was there* to where you already are. 3b's
;; subtitle calls undo time travel, and those are its two directions.
;;
;; the argument is a change id — text, not a choice — so an empty or wrong one
;; reaches jj and comes back in jj's own words rather than as "no such command".
(ex-set! "restore-change" "bring a change's content here — :restore-change <id>"
         (lambda (rest bang)
           (key/run (key/cmd "restore-change" "change" (trim rest)))))

(define-float-surface!
  "timeline"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"jj\" \"timeline\")
                 (view/spans (timeline/rows args))
                 (timeline/footer)))")
