;; keymaps.scm — the live keymap.
;;
;; the table is here, in the editor layer, and there is no copy of it in rust.
;; that is the whole of the liveness claim: the host asks this file what a key
;; is bound to on every keystroke, so a binding you change at :repl is in force
;; on the very next one. nothing caches, so nothing can go stale, so there is no
;; reload step to forget.
;;
;; T033 fills this file in — every binding, counts, named registers, the leader
;; tree. what is here now is the machinery plus the one seed the repl needs to
;; be reachable, and it is deliberately almost empty: an invented default is a
;; decision two reasonable users would want to differ on, made by nobody.

;; ---------------------------------------------------------------------------
;; the table
;; ---------------------------------------------------------------------------

;; key sequence in vim notation -> a thunk. `"]r"`, `"<C-c>"`, `":"`.
(define phosphor/keymap (hash))

;; what has been typed so far in an unfinished sequence. `]` on its own is
;; pending, not unbound, because `]r` is bound.
(define phosphor/pending "")

;; ---------------------------------------------------------------------------
;; binding
;; ---------------------------------------------------------------------------

;; bind keys to a thunk, live. 6b types exactly this:
;;
;;   (keymap-set! "]r" (lambda () (goto (next-region-by claude))))
;;
;; the repl persists the form afterwards — see repl.scm — which is why this
;; returns `void` (`#ok`) and writes nothing itself: a form loaded from
;; init.scm at boot must not append itself back to the file it came from.
;;
;; T033 routes this through the `set-keybinding!` capability, so the cli and mcp
;; doors reach the same table. today that row refuses (`not built yet — T033
;; builds it`), so the table is reached from scheme alone.
(define (keymap-set! keys thunk)
  (set! phosphor/keymap (hash-insert phosphor/keymap keys thunk))
  void)

(define (keymap-remove! keys)
  (set! phosphor/keymap (hash-remove phosphor/keymap keys))
  void)

;; what is bound, for which-key and :help to read (T033, T086).
(define (keymap-keys) (hash-keys->list phosphor/keymap))

;; ---------------------------------------------------------------------------
;; dispatch
;; ---------------------------------------------------------------------------

;; is `seq` a proper prefix of some bound sequence?
(define (phosphor/prefix? seq)
  (define (longer-match? keys)
    (cond
      [(null? keys) #f]
      [(and (starts-with? (car keys) seq)
            (> (string-length (car keys)) (string-length seq)))
       #t]
      [else (longer-match? (cdr keys))]))
  (longer-match? (hash-keys->list phosphor/keymap)))

;; one keystroke, in vim notation. answers:
;;
;;   'handled  — a binding ran
;;   'pending  — the sequence so far is a prefix; wait for the next key
;;   'unbound  — nothing here wants it; the host may have its own use for it
;;
;; the host calls this before it does anything else with a key, and does not
;; interpret the binding — it only learns whether one ran.
(define (phosphor/press key)
  (let ([seq (string-append phosphor/pending key)])
    (cond
      [(hash-contains? phosphor/keymap seq)
       (set! phosphor/pending "")
       ((hash-try-get phosphor/keymap seq))
       'handled]
      [(phosphor/prefix? seq)
       (set! phosphor/pending seq)
       'pending]
      [else
       (set! phosphor/pending "")
       'unbound])))

;; drop an unfinished sequence — esc, and anything else that interrupts.
(define (phosphor/press-reset)
  (set! phosphor/pending "")
  void)

;; ---------------------------------------------------------------------------
;; the seed
;; ---------------------------------------------------------------------------

;; `:` opens the repl. one binding, and not an invented one: `:` is the
;; editor's prompt key, and until the ex prompt lands the repl is the only
;; prompt there is. when it lands, `:` belongs to it and `:repl` — the command
;; 6b's header names — opens this.
(keymap-set! ":" (lambda () (open-repl!)))
