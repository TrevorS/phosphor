;; diskdiff.scm — screen 5b, your unsaved buffer against what claude wrote (T070).
;;
;; two columns and three ways out. the whole surface is `view/diff` over the
;; `disk` source the vocabulary already declared for this screen, so this file
;; adds no lines to phosphor-ui — the fourth proof of that after arch.scm,
;; dashboard.scm and inbox.scm.
;;
;; **side-by-side, where 4b is unified, and that is the design rather than a
;; setting.** 4b's own words are *"one review block as one unified diff"*; the
;; design brief calls this screen `:dv`, *"a side-by-side of buffer vs disk"*.
;; two surfaces, two modes, one widget — which is why `set-diff-mode` belongs to
;; T070 and sat in `lint-action-arms.sh`'s RECORDED table until it did.
;;
;; **no header on the float.** the diff carries its own — `disk ⟷ buffer ·
;; src/fetch.rs`, built in the binary because only the binary can read both
;; sides — and a float header above it would say the same thing twice.

;; 5b's footer: the three exits, spelled whole.
;;
;; **the mockup draws `:rr take disk · :w keep mine · :c ask claude` and this
;; file does not**, which is OPEN-QUESTIONS §62 and the second instance of §61's
;; ruling. two reasons, and the second is new:
;;
;;   1. Design Language §6 — *"spell the whole command … never cryptic
;;      contractions like `:ca` or `:rr`"* — names `:rr` as its own
;;      counter-example, so the rule was written against that spelling.
;;   2. **`:c` is already `:c[omment]`**, T068's thread verb. the mockup's
;;      footer does not merely contract; it names a shipped command that does
;;      something else entirely. a footer you can follow into the wrong verb is
;;      worse than one you cannot follow at all.
;;
;; the whole-word forms are `DiskExit`'s own wire names — `take-disk`,
;; `keep-mine`, `ask-claude` — so the footer, the vocabulary and the ex line all
;; spell the exit the same way.
(define (diskdiff/footer)
  (view/key-hints 'footer
                  (list (view/key-hint ":take-disk" "take what claude wrote")
                        (view/key-hint ":keep-mine" "keep your buffer")
                        (view/key-hint ":ask-claude" "hand it to claude")
                        (view/key-hint "esc" "close"))))

;; **informational, not needs-you.** 5b asks nothing on its own: it is a
;; comparison you opened, and the three exits are yours to take whenever. the
;; thing that *is* asking — `1d`'s `✱` corner box — already spent the amber.
(define-float-surface!
  "disk-diff"
  "(lambda (args)
     (view/float 'informational
                 void
                 (view/diff (hash \"kind\" \"disk\"
                                  \"buffer\" (hash-try-get args \"buffer\"))
                            (or (hash-try-get args \"mode\") \"side-by-side\")
                            \"flat\")
                 (diskdiff/footer)))")

;; the three exits, each naming its own `DiskExit`.
;;
;; **three commands rather than one taking an argument**, because `:resolve-disk-diff
;; take-disk` is a sentence nobody types twice. the exit is the decision, so the
;; decision is the command.
(ex-set! "take-disk" "the file claude wrote wins — :take-disk"
         (lambda (rest bang)
           (key/run (key/cmd "resolve-disk-diff"
                             "target" (key/at-cursor)
                             "exit" "take-disk"))))

(ex-set! "keep-mine" "your buffer wins, and is written — :keep-mine"
         (lambda (rest bang)
           (key/run (key/cmd "resolve-disk-diff"
                             "target" (key/at-cursor)
                             "exit" "keep-mine"))))

;; **the exit that resolves nothing, and says so.** it hands the disagreement to
;; claude and leaves 5b open, because whether the file changes is his turn
;; rather than this command's. the `✱` stays true until something actually moves.
(ex-set! "ask-claude" "hand the disagreement to claude — :ask-claude"
         (lambda (rest bang)
           (key/run (key/cmd "resolve-disk-diff"
                             "target" (key/at-cursor)
                             "exit" "ask-claude"))))

;; the two modes, one command each — the control 5b has that no mockup draws.
;;
;; **not invented for the lint.** the mode is a real choice and the two screens
;; disagree on it for a real reason: 4b is *"one review block as one unified
;; diff"* and 5b is *"a side-by-side of buffer vs disk"*. two columns need
;; width, and a person on eighty columns comparing two long lines wants the
;; unified view — which `DiffBody` already draws and `T063` already tested
;; against both.
;;
;; **two commands rather than one taking an argument**, for the reason the
;; three exits above give and one more that is specific to this verb: `mode` is
;; a *choice* type, so a word that is not one of the two raises inside
;; `key/cmd` and reaches the bridge as "no such command" — a registered command
;; reporting it does not exist. that trap has been paid for three times through
;; `string->number` answering `#false` (T060, T067, T068). a command per value
;; cannot be handed a value.
;;
;; they spell `DiffMode`'s own wire names, so the ex line and the Action agree.
(ex-set! "unified" "one column, changes inline — :unified"
         (lambda (rest bang)
           (key/run (key/cmd "set-diff-mode" "mode" "unified"))))

(ex-set! "side-by-side" "two columns, buffer against disk — :side-by-side"
         (lambda (rest bang)
           (key/run (key/cmd "set-diff-mode" "mode" "side-by-side"))))

;; `:dv` is the design brief's name for this screen and `:diff-disk` is the one
;; keymaps.scm already registers; both open it. no third spelling is added here.
