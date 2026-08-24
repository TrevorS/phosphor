;; review.scm — screen 8b, the 40-file block (T065).
;;
;; **8b and 4b are one surface at two fold depths.** both mockups open with
;; `review — ✻ <title> · N files · N regions · N seen ✓` as their first row; what
;; differs is whether the files are grouped and closed, or one is open with its
;; hunks under it. so there is one float, one session in the loop, and
;; `open-review-block` opens it — a verb declared against T066, armed at T065,
;; because "8b is navigable" is not a claim you can make about a screen nothing
;; opens.
;;
;; **the body is `view/diff` and the rows come through `Resources::diff`**, which
;; is `Node::Transcript`'s division exactly: the float is a *snapshot* composed
;; once, and the rows behind it are a live query rebuilt every frame. that is
;; what makes `s` on a hunk move the counts on the row you pressed it from
;; without recomposing anything.
;;
;; the body is a helper rather than an expression inside the surface string, and
;; that is a rule this directory learned from a lint: `lint-node-kinds.sh` strips
;; string literals before it looks for a composition, so a `(view/diff …)` living
;; only inside `define-float-surface!` is invisible to it and `Node::Diff` reads
;; as a kind nothing reaches. asks.scm records the same finding.
(define (review/body block)
  (view/diff (hash "kind" "review-block" "block" block) "unified" "directory"))

;; 4b's and 8b's footer, spelled whole — Design Language §6, *"keyhints spell
;; the whole command … never cryptic contractions"*. 8b draws
;; `za fold · s seen · S group seen · q` and 4b draws
;; `]] next file · za fold · s seen · S all · q`; this is their union, because
;; the two are one surface at two fold depths and a footer that changed as you
;; folded would be teaching you a different editor each time.
;;
;; `q` gets the word it means rather than the letter it is.
;;
;; **primary action first, escape last**, which is §6's other footer rule.
(define (review/footer)
  (view/key-hints 'footer
                  (list (view/key-hint "]]" "next file")
                        (view/key-hint "za" "fold")
                        (view/key-hint "s" "mark seen")
                        (view/key-hint "S" "mark the group seen")
                        (view/key-hint "q" "close"))))

;; **informational.** 8b is something you read and steer; it asks nothing and is
;; not in front of anything — a `needs-you` mood would spend §1's amber on a
;; screen with no question on it.
(define-float-surface!
  "review"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"review\" \"block\")
                 (review/body (hash-ref args \"block\"))
                 (review/footer)))")

;; **`2b`, the peek** (`T066`) — `gh`, without leaving the buffer.
;;
;; the body is `view/diff` over a `DiffSource::Hunk`, no header (the float is
;; three lines tall and `DiffVm::header` empty draws none — `T063`'s own
;; documented case for exactly this), and a two-line footer instead of the
;; block's four: `2b` has no rows to navigate and no groups to widen into, so
;; `]]`, `za` and `S` would be keys the float cannot answer. `s`'s Rust-side
;; handling is beside the review's own, for the same reason both exist —
;; `keymaps.scm` has no way to ask what surface holds the screen.
(define (peek/body hunk)
  (view/diff (hash "kind" "hunk" "hunk" hunk) "unified" "flat"))

(define (peek/footer)
  (view/key-hints 'footer
                  (list (view/key-hint "s" "mark seen")
                        (view/key-hint "q" "close"))))

(define-float-surface!
  "hunk-peek"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"claude changed\" \"here\")
                 (peek/body (hash-ref args \"hunk\"))
                 (peek/footer)))")

;; `:review` — the door a *person* has.
;;
;; the argument is the block id, which `review-blocks` reports and 1b's seam
;; sentence does not. **bare means the newest block**, for the reason `:defer`
;; bare means the question on screen: an id is what a *door* names, and a person
;; who has just been told `review ready · retry logic` has one block in mind and
;; no number for it.
;;
;; **the resolution is the arm's and not this lambda's**, and that is a rule
;; learned twice now. an ex lambda that calls a query raises when the query
;; refuses, and a raise inside `phosphor/ex` reads as *"no such command"* — so
;; the first version of this called `(review-blocks)` here and `:review` was a
;; registered command that said it did not exist. `every_ex_command_decodes`
;; types every name with an empty argument and is what caught it, the same way
;; it caught `(string->number "")` at T060.
(ex-set! "rev[iew]" "open a review block — :review [id]"
         (lambda (rest bang)
           (if (equal? rest "")
               (key/run (key/cmd "open-review-block"))
               (key/run (key/cmd "open-review-block" "block" (string->number rest))))))

;; `:grouping` — 8d's answer at 80 columns, said as a command.
;;
;; **the same files without the group rows**, which is what makes it a rendering
;; choice rather than a different query. `directory` and `flat` are the wire
;; spellings, so what you type is what the vocabulary calls it.
(ex-set! "grouping" "group a review by directory or flat — :grouping directory|flat"
         (lambda (rest bang)
           (key/run (key/cmd "set-diff-grouping"
                             "grouping"
                             (if (equal? rest "") "directory" rest)))))

;; `:annotate` — claude's group note, 8b's *"mechanical"* against *"the meat"*.
;;
;; **it annotates the group you are on, and takes no id.** an id is what a
;; *door* names; a person is looking at a row. that also removes an ambiguity
;; rather than deferring one — `:annotate 3 handler signatures` would have to
;; guess whether `3` is a group or the first word of the note, and an annotation
;; beginning with a digit is not a strange thing to write.
;;
;; **bare clears it**, which is `annotate-group`'s own documented empty case: a
;; verb that could set a wrong sentence and never unset it would leave it on the
;; row forever.
;;
;; the real producer is the agent, over the same registry — this is the door a
;; person has, for the reason every other one-off ex command here exists: a
;; screen nothing can put text on is a screen nothing can check.
(ex-set! "annotate" "annotate the group you are on — :annotate <text>"
         (lambda (rest bang)
           (key/run (key/cmd "annotate-group" "text" rest))))
