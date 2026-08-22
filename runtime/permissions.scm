;; permissions.scm — screen 7a, and the rule an always-allow writes (T061).
;;
;; **this file exists so that `(allow "git push")` is a form the editor can
;; load.** T101 moved machine-written forms to
;; `$XDG_CONFIG_HOME/phosphor/persisted.scm`, and `Layer::load_persisted` runs
;; each one after the whole boot order and records a fault the boot float draws.
;; before this file, `allow` was a free identifier: a grant written by 7a would
;; have opened an error float on every subsequent start.
;; OPEN-QUESTIONS.md ss35 is that constraint, and its own note says the fix is
;; about *where* the definition goes rather than new machinery — so `allow` is
;; here, and "here" is a name in `phosphor/boot-files`.
;;
;; **the rule is an option, not a table in rust.** 7a's promise is that
;; always-allow "writes a legible rule", and the most legible rule is one made
;; of parts the editor already has: `allow` sets an option, options are read by
;; the binary every frame, and a person reading `persisted.scm` sees
;; `(allow "git push")` — a sentence rather than a serialization.

;; the option the allow-list lives in. one string, `|`-separated, because an
;; option is one value and a second list beside it would be a second truth.
(define phosphor/allowed-option "allowed-commands")

;; **the list lives here and the option is its published copy.** the natural
;; shape would be to read the option back and append to it, and this build has
;; no reader: `(options)` is `T021` and unarmed, so a `(hash-try-get (options)
;; …)` answers `#raised · not built yet — T021 builds it`. that is a real
;; constraint rather than an inconvenience, and the honest response is the one
;; the store already uses for `session` and `transcript` — one truth, written
;; out to whoever needs to read it. the option is never read by this file.
(define phosphor/allowed '())

;; **prefix matching, and the prefix is the point.** 7a's rule is
;; `(allow "git push")` and what it permits is `git push origin retry-backoff`
;; — the *verb*, not the exact invocation. an allow-list of exact command lines
;; would never match twice. the matching itself is the binary's, against the
;; option this file publishes.

(define (permissions/rules) phosphor/allowed)

;; `allow` — the whole public surface of this file, and the shape 7a draws.
;;
;; idempotent, because `persisted.scm` grows by appending and pressing the same
;; digit twice is a thing people do. a duplicate rule changes nothing and should
;; cost nothing.
(define (permissions/join rules)
  (if (null? rules)
      ""
      (foldl (lambda (rule so-far)
               (if (equal? so-far "") rule (string-append so-far "|" rule)))
             ""
             rules)))

(define (allow invocation)
  (if (member invocation phosphor/allowed)
      invocation
      (begin
        (set! phosphor/allowed (append phosphor/allowed (list invocation)))
        (set-option! phosphor/allowed-option (permissions/join phosphor/allowed))
        invocation)))

;; 7a's own float. **the body is 4a's and the chrome is not**, which is what
;; the two screens actually differ by: both draw prose over amber digits, and
;; one says "claude needs input" while the other says "claude wants to run".
;; sharing the body and splitting the header is the smallest true reading of
;; that — a second `view/question` would be the same node twice.
;;
;; the footer drops `4a`'s `1–n` for `1–3`, because a permission ask has exactly
;; three answers and saying so is one fewer thing to work out.
(define-float-surface!
  "permission"
  "(lambda (args)
     (view/float 'needs-you
                 (view/float-header \"✻ claude\" \"wants to run\")
                 (ask/body (hash-ref args \"ask\"))
                 (view/key-hints 'footer
                                 (list (view/key-hint \"1–3\" \"answer\")
                                       (view/key-hint \":claude\" \"ask why\")
                                       (view/key-hint \"esc\" \"later\")))))")

;; `:permit` — the producer, so 7a can be reached from a keyboard.
;;
;; **the real producer is the agent**, over ACP's `session/request_permission`
;; — this is the door a *person* has, and it exists for the reason `:ask` does
;; one file over: a screen nothing can put on screen is a screen nothing can
;; check.
(ex-set! "permit" "ask permission for a command — :permit git push origin main"
         (lambda (rest bang)
           (key/run (key/cmd "request-permission" "invocation" rest "files" (list)))))

;; `:allowed` — read the list back. **a person who cannot see the rules cannot
;; audit them**, and a permission surface whose grants are invisible is a
;; permission surface you stop trusting.
(ex-set! "allowed" "what has been allowed always — 7a's written rules"
         (lambda (rest bang)
           ;; **`args` is not optional**, and leaving it out is invisible: the
           ;; `key/cmd` raises, `phosphor/ex` raises with it, and the bridge
           ;; reads a raise as `Ex::Unknown` — so a command that *is* registered
           ;; answers "no such command". checked with `phosphor/ex-bound?`,
           ;; which said `#t` while the line said otherwise.
           (key/run (key/cmd "open-float" "surface" "allowed" "args" (hash)))))

(define (permissions/rows)
  (let ([rules (permissions/rules)])
    (if (null? rules)
        (list (view/span-row (list (view/run "nothing allowed always" 'meta 'plain)) void))
        (map (lambda (rule)
               (view/span-row (list (view/run "✓ " 'claude 'plain)
                                    (view/run rule 'text 'plain))
                              void))
             rules))))

(define-float-surface!
  "allowed"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \"permissions\" \"always allowed\")
                 (view/spans (permissions/rows))
                 (view/key-hints 'footer
                                 (list (view/key-hint \"esc\" \"close\")))))")
