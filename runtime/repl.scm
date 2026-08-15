;; repl.scm — what the repl session does with what you type.
;;
;; the repl itself is rust (it owns the input line, the history and the view
;; tree). what belongs here is the one policy question it has to ask: which
;; forms outlive the session.

;; ---------------------------------------------------------------------------
;; persistence
;; ---------------------------------------------------------------------------

;; the verb that keeps a form.  `(persist! (keymap-set! "]r" …))` evaluates the
;; rebind and writes the whole line to `phosphor/persist-file`.
;;
;; **it is a mark, not a mechanism, and that is what makes it idempotent.** the
;; repl is the only thing that writes — it is the only place with the *source
;; text*, since a scheme closure cannot be printed back as the form that made
;; it — so a `(persist! …)` read back at boot evaluates its argument and appends
;; nothing.  identity here is the whole implementation.
;;
;; ruled 2026-08-14 (T101), and it overrides what 6b draws.  6b answers a bare
;; `(keymap-set! "]r" …)` with `⇒ #ok · persisted to init.scm`, which is
;; auto-persistence by head name: try a theme, keep it forever.  emacs is the
;; argument — `M-:` and `ielm` never persist, and `M-x customize` is a
;; deliberate *save this* that writes `custom-file` — and so is this build's
;; third invariant, **nothing moves unless you asked**.  you asked to evaluate.
;; the conflict is recorded in docs/OPEN-QUESTIONS.md; the drawing is amended at
;; claude.ai, not here.
(define (persist! kept) kept)

;; the head rust writes without asking anything.  read once after the boot
;; (crates/phosphor/src/main.rs, `PERSIST_VERB`) so the host behind the barrier
;; never re-enters the vm to ask.
(define phosphor/persist-verb "persist!")

;; heads the repl *offers* to keep.  a form with one of these runs, does not
;; reach the file, and answers `⇒ #ok · not persisted — (persist! …)` so the
;; receipt teaches the verb at the moment you would want it.
;;
;; this is the list that used to be `phosphor/persistent-heads` itself, and the
;; judgement in it is unchanged: two reasonable users would disagree about which
;; forms are decisions rather than experiments, so it is a list here rather than
;; a table in rust.  `(+ 1 2)` and every query are deliberately absent — they get
;; no persistence line at all, because persisted.scm is a file of decisions and
;; the receipt should not become a transcript either.
(define phosphor/offered-heads
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

;; what the repl carries to `persist-form!` at all: the verb, which is written,
;; and the offered heads, which are answered.  everything else is session-only
;; and silent.
;;
;; the name is rust's (`phosphor_steel::repl::PERSISTENT_HEADS`) and is one
;; window stale — it now means *"heads persistence is a question for"*, and only
;; the first entry is persistent.  derived from the two lists above rather than
;; written out, so the spelling of a verb lives in exactly one place.
(define phosphor/persistent-heads
  (cons phosphor/persist-verb phosphor/offered-heads))

;; where those forms are written — **a name in the config home, not in the tree
;; that booted** (T101).
;;
;; it used to be joined to the runtime root, and in a dev checkout that root is
;; the repository: CP-4's manual test left a `(define-language! "lua" …)` in the
;; tracked runtime/persisted.scm.  emacs's equivalent would be writing custom.el
;; into emacs/lisp/.  the path is `$XDG_CONFIG_HOME/phosphor/` — config rather
;; than state, because a binding you deliberately kept belongs with your
;; dotfiles, and because there is nothing per-project about a keymap
;; (crates/phosphor-core/src/config.rs).
;;
;; **and it loads last, which is the reason it is a file of its own.** init.scm
;; runs to its last form *before* rust reads the load order it declared, so a
;; form appended to it can only use names rust registered — `(keymap-set! …)` is
;; defined in keymaps.scm, which has not loaded yet, and a persisted rebind would
;; come back on the next start as a free-identifier fault in a boot float.  found
;; by running it.  this file is no longer in `phosphor/boot-files` at all: the
;; binary loads it after the whole load order has run, so "last" is structural
;; rather than a position in a list somebody can reorder.
(define phosphor/persist-file "persisted.scm")
