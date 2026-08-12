;; repl.scm — what the repl session does with what you type.
;;
;; the repl itself is rust (it owns the input line, the history and the view
;; tree). what belongs here is the one policy question it has to ask: which
;; forms outlive the session.

;; ---------------------------------------------------------------------------
;; persistence
;; ---------------------------------------------------------------------------

;; 6b, line 4:
;;
;;   λ (keymap-set! "]r" (lambda () (goto (next-region-by claude))))
;;   ⇒ #ok · persisted to init.scm
;;
;; a form whose head is in this list is appended to init.scm after it runs; a
;; form whose head is not is session-only. that is the difference between a
;; decision and an experiment, and it is a judgement two reasonable users would
;; make differently — so it is a list here rather than a table in rust.
;;
;; `(+ 1 2)` and every query are deliberately absent: init.scm is a file of
;; decisions, not a transcript.
(define phosphor/persistent-heads
  '("keymap-set!"
    "keymap-remove!"
    "set-option!"
    "set-theme!"
    "define-language!"
    ;; statusline.scm's three: which segments, in what order, and what gets
    ;; given up first. a statusline you rearranged is a decision, and it should
    ;; still be yours after a restart.
    "status-set!"
    "status-order-set!"
    "status-ladder-set!"))

;; where those forms are written.
;;
;; **not init.scm, and the reason is the boot order.** init.scm runs to its last
;; form *before* rust reads the load order it declared, so a form appended to it
;; can only use names rust registered — `(keymap-set! …)` is defined in
;; keymaps.scm, which has not loaded yet, and a persisted rebind would come back
;; on the next start as a free-identifier fault in a boot float. found by
;; running it.
;;
;; so a persisted form goes to the file that loads *last*, where everything it
;; could depend on is already defined. a layer that does not declare this gets
;; init.scm, which is right for a one-file layer and is what 6b draws.
(define phosphor/persist-file "persisted.scm")
