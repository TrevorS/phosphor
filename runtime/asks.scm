;; asks.scm — screen 4a, claude asking mid-turn (T059).
;;
;; the third surface built entirely in the editor layer, after arch.scm and
;; dashboard.scm — and the first of the three that is not `view/spans`. 4a is
;; ordinary chrome: a needs-you float with a header, a body, and a footer, which
;; is exactly what T084's primitive is for. the body is `view/question`, a real
;; node kind drawn by `phosphor-ui`, because "prose, amber digit options, and
;; the full command in the footer" is a shape three screens share (4a, 7a and
;; 4b) rather than one drawing.
;;
;; **the float names the ask it is showing, and that is load-bearing.** every
;; other body on this surface is implicit — there is one completion list, one
;; picker, one transcript — and there are as many questions as claude has asked.
;; a float composed for ask 8 draws ask 8 whatever has arrived behind it, so
;; answering what you are reading is the same thing as answering what you meant.

;; 4a's footer. **the digits are stated as a range, not as three hints.**
;; "1–3 answer" is one fact about the screen; three rows saying "1 answer",
;; "2 answer", "3 answer" would be the option list drawn twice, once without its
;; labels. the range is the body's own `[n]` column said once.
;;
;; `:claude` and `esc` are spelled in full — Design Language §6, and the same
;; reading OPEN-QUESTIONS.md ss55 records for 7b's `:ca`.
(define (ask/footer)
  (view/key-hints 'footer
                  (list (view/key-hint "1–n" "answer")
                        (view/key-hint ":claude" "reply in prose")
                        (view/key-hint "esc" "later"))))

;; **`esc later` means the queue, not the screen** — T060. esc defers: the
;; question stays pending, still counts toward the statusline's `!`, and `]!`
;; brings it back. this paragraph said "nothing yet brings it back" for exactly
;; one task.
;; **the body is a helper and not an expression inside the surface string**, and
;; that is a rule this file learned from a lint rather than from taste.
;; `scripts/lint-node-kinds.sh` proves every node kind is composed by the shipped
;; configuration, and it strips string literals before it looks — so a
;; `(view/question …)` that lived only inside a `define-float-surface!` body is
;; invisible to it, and `Node::Question` read as a kind nothing reaches. it is
;; also simply better: the composition is code, so the REPL can call it.
(define (ask/body id) (view/question id))

(define-float-surface!
  "question"
  "(lambda (args)
     (view/float 'needs-you
                 (view/float-header \"✻ claude\" \"needs input\")
                 (ask/body (hash-ref args \"ask\"))
                 (ask/footer)))")

;; `:ask` — the producer, so 4a can be reached from a keyboard.
;;
;; **the real producer is the agent**, over ACP's `session/request_permission`
;; and whatever a question turns out to be on that wire, which is T060's and
;; T061's. this is the door a *person* has, and it exists for the reason every
;; other one-off ex command here does: a screen nothing can put on screen is a
;; screen nothing can check.
;;
;; the options are positional and one-based — `:ask are you sure?|yes|no` — so
;; the digit a row draws is its position in what you typed, and there is nowhere
;; for the two to disagree.
(define (ask/split text)
  (let loop ([left (string->list text)] [word '()] [out '()])
    (cond
     [(null? left)
      (reverse (cons (list->string (reverse word)) out))]
     [(char=? (car left) #\|)
      (loop (cdr left) '() (cons (list->string (reverse word)) out))]
     [else (loop (cdr left) (cons (car left) word) out)])))

(define (ask/options labels)
  (let loop ([left labels] [digit 1] [out '()])
    (if (null? left)
        (reverse out)
        (loop (cdr left)
              (+ digit 1)
              (cons (hash "digit" digit "label" (car left)) out)))))

;; `:defer` — 4a's `esc later` said as a command. **esc is the key and this is
;; the door**: a person presses esc, and the verb has to be reachable from a
;; place a test and an agent can name.
;;
;; **bare means the one you are looking at**, and that is not a convenience:
;; `defer-ask` takes an id because a *door* has to name one, and a person has
;; exactly one question in front of them. an ex command that required the number
;; would be asking you to read an id off a screen that does not draw one — and
;; `(string->number "")` is `#false`, which raises inside `key/cmd` and reaches
;; the ex bridge as *"no such command"*. `every_ex_command_decodes` types every
;; name with an empty argument and is what caught it.
(ex-set! "defer" "push the focused question back — :defer"
         (lambda (rest bang)
           (if (equal? rest "")
               (key/run (key/cmd "defer-ask"))
               (key/run (key/cmd "defer-ask" "ask" (string->number rest))))))

(ex-set! "ask" "ask a question — :ask prose|option|option"
         (lambda (rest bang)
           (let ([parts (ask/split rest)])
             (key/run (key/cmd "enqueue-ask"
                               "prose" (car parts)
                               "options" (ask/options (cdr parts)))))))
