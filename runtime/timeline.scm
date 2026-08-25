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
        (list (view/run (string-append "  +" (number->string added)) 'claude)
              (view/run (string-append " −" (number->string removed)) 'trouble))
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
   (list (view/run (timeline/glyph row) 'claude)
         (view/run (string-append " " (hash-try-get row "change")) 'text)
         (view/run (string-append " · " (timeline/who row) "  ") 'meta)
         (view/run (hash-try-get row "description") 'text))
   (timeline/stat row)))

;; **the empty case is a sentence, not a blank float.**
;;
;; a git repository and a bare directory both land here, and neither is broken.
;; CP-8c reads this line: *"does anything feel degraded or apologetic?"* — so it
;; says what is true and offers nothing.
(define (timeline/rows)
  (let ([rows (timeline)])
    (if (null? rows)
        (list (list (view/run "no changes to show — the timeline is jj's" 'meta)))
        (map timeline/row rows))))

(define (timeline/footer)
  (view/key-hints 'footer
                  (list (view/key-hint "↵" "edit here")
                        (view/key-hint "d" "diff")
                        (view/key-hint "o" "full op log")
                        (view/key-hint "esc" "close"))))

;; **informational.** the timeline asks nothing; it is a place you go to look.
(define-float-surface!
  "timeline"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"jj\" \"timeline\")
                 (view/spans (timeline/rows))
                 (timeline/footer)))")
