;; disk.scm — screen 1d, the file changed underneath you (T069).
;;
;; two lines in a corner box: what happened, and the two ways out. that is the
;; whole surface, and its smallness is the point — 1d's caption is *"buffer
;; holds stable; nothing moves unless you asked — indicate, offer to refresh"*,
;; and everything this file draws is the *indicate* half.
;;
;; **it does not take focus, and that is the feature rather than an omission.**
;; every other float in this tree becomes `Surface::Float` and the keys go to
;; it; this one is composed into the `Surface::Buffer` arm beside `T038`'s
;; completion list, so the buffer behind it stays live and you keep typing
;; through it. a box that stole your cursor to tell you nothing had moved your
;; cursor would be the funniest possible way to break invariant 3.
;;
;; **needs-you amber, and it is read off the mockup rather than chosen.** 1d's
;; box is `border:1px solid #6b5426` on `background:#171207`, which Design
;; Language §4 names as the needs-you pair — not the `#2a3c2e` passive one the
;; completion list uses. the mood is the border's only meaning (§4), and this
;; border is asking you to decide something.

;; the message. the actor is named only when the machine actually knows it.
;;
;; **§7's rule is that the machine tracks claude**, and a filesystem event has
;; no author — `notify` reports that bytes moved, not who moved them. the loop
;; attributes the change to claude when a turn was running, which is the same
;; condition 1d itself draws: its statusline reads `✻ claude working` beside
;; the `✱`. with no turn running the sentence drops the clause rather than
;; guessing, because *"by claude"* over a `git checkout` is the editor
;; asserting something it does not know.
(define (disk/said by)
  (if (equal? by "claude")
      "✱ changed on disk by claude"
      "✱ changed on disk"))

;; the two ways out, spelled whole.
;;
;; **1d draws `:rr refresh · :dv diff` and this file does not**, which is a
;; disagreement between two design documents rather than between the design and
;; the build. Design Language §6 rules on the exact strings: *"spell the whole
;; command … never cryptic contractions like `:ca` or `:rr`"* — it names `:rr`
;; as its own counter-example, so the rule was written against that spelling and
;; the mockup is the older artifact. recorded as OPEN-QUESTIONS.md §61.
;;
;; the whole-word forms are also the ones that exist: `:reload` and
;; `:diff-disk` are registered ex commands, and `:rr` would resolve to nothing.
;; **`view/run` takes three arguments**, and every other surface in this
;; directory defines this same two-argument helper rather than repeating
;; `'plain` at each call — arch.scm, inbox.scm, dashboard.scm and pickers.scm
;; all do. This file did not, and passed two: an `ArityMismatch` that only fires
;; when the float is *composed*, so it survived `T069` entirely because nothing
;; opened `1d`. `CP-8c`'s matrix found it by opening a different float.
(define (disk/run text tone) (view/run text tone 'plain))

;; **`view/spans` takes rows, and a row is a `view/span-row`** — not a bare
;; list of runs. arch.scm and inbox.scm both wrap: `(view/span-row runs tint)`.
;; This file passed the runs directly, which is a `TypeMismatch` raised only
;; when the float composes — the second of two faults `1d` carried out of
;; `T069` for want of a test that opened it.
(define (disk/row . runs) (view/span-row runs void))

;; **the footer is `view/key-hints`, not `view/spans`.**
;;
;; the third fault this file carried out of `T069`, and the quietest: a
;; `view/spans` in the footer slot draws **nothing at all** — no error, just an
;; empty row between two rules. every other surface in this directory uses
;; `view/key-hints` for a footer (`5b`, `5c`, `8b`, `4a`), and `view/spans` is
;; T080's escape hatch for a *body*.
;;
;; all three faults — the arity, the bare runs, and this — were invisible until
;; something opened the box, which nothing did for the whole of `T069`.
(define (disk/ways)
  (view/key-hints 'footer
                  (list (view/key-hint ":reload" "take what is on disk")
                        (view/key-hint ":diff-disk" "compare them"))))

;; 1d, composed.
;;
;; **no header and no footer.** `Float` takes both as options and this screen
;; wants neither: the mockup's box is two lines, and a header would make it
;; three and push the buffer text it deliberately does not cover. the body
;; carries both rows itself.
(define-float-surface!
  "disk"
  "(lambda (args)
     (view/float 'needs-you
                 void
                 (view/spans (list (disk/row (disk/run (disk/said (hash-try-get args \"by\")) 'attention))))
                 (disk/ways)))")

;; `:reload` — take what is on disk, spelled the way §6 asks.
;;
;; **the ex door for a key that already exists.** `SPC r r` runs the same
;; capability; this is the spelling 1d's own box points at, and a box naming a
;; command nothing registers would be an offer you cannot accept.
(ex-set! "reload" "take what is on disk — :reload"
         (lambda (rest bang)
           (key/run (key/cmd "reload-from-disk" "target" (key/at-cursor)))))
