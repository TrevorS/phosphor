;; pickers.scm — the picker sources the shipped layer defines (T046).
;;
;; a source is a procedure of one argument (the args hash `open-picker`
;; carries) that answers a `view/spans` node. rust takes its rows, hands them
;; to nucleo, and draws whatever matched. nothing in rust knows what a row
;; *means* — which is why `2a`, `3d` and `8a` are one widget with three
;; sources rather than three screens.
;;
;; T080's escape hatch is what a row is made of, deliberately: "styled rows
;; straight from steel" is already the vocabulary for this shape, and a second
;; one would be two places for "a row is runs, left to right" to drift apart.
;;
;; ---------------------------------------------------------------------------
;; why these are steel and not rust
;; ---------------------------------------------------------------------------
;;
;; invariant 4: every surface is a query over the store. a source is that
;; sentence made executable — it reads the store through the same queries any
;; door can call, and it can be redefined at the repl with no restart, which is
;; T046's own acceptance. a rust `unseen` source would be a second answer to
;; "what is unseen" living beside the query that already answers it.

;; ---------------------------------------------------------------------------
;; helpers
;; ---------------------------------------------------------------------------

;; one run, in a tone. `view/run` takes text, a tone and an emphasis; every row
;; here is `'plain`, because §1 spends no colour on decoration and a picker row
;; has nothing to emphasise until a matcher marks what matched (T047).
(define (picker/run text tone) (view/run text tone 'plain))

;; a row from a list of runs. no tint: §3's row tints name a *region state* and
;; a picker row is not in one — the selected row's ground is the widget's.
(define (picker/row . runs) (view/span-row runs void))

;; a count as a word, so "1 region" and "3 regions" both read.
(define (picker/regions n)
  (string-append (number->string n) (if (= n 1) " region" " regions")))

;; ---------------------------------------------------------------------------
;; unseen — screen 2a
;; ---------------------------------------------------------------------------
;;
;; every region claude wrote that you have not looked at, one row each. the
;; path is text and the state is claude's green, because §1 is "green always
;; means claude" and an unseen region is exactly his writing you have not read.
;;
;; the span is drawn as `path:line` — the same spelling `mark-seen!` takes as a
;; target (`target_from_text`), so what a row *says* is what you would type to
;; act on it.
(define (picker/unseen-row region)
  (let* ([path (hash-ref region "path")]
         [span (hash-ref region "span")]
         [line (hash-ref (hash-ref span "start") "line")])
    (picker/row
     (picker/run (string-append path ":" (number->string line)) 'text)
     (picker/run "  " 'meta)
     (picker/run "unseen" 'claude))))

(define-picker-source!
  "unseen"
  "(lambda (args)
     (view/spans (map picker/unseen-row (unseen-regions))))")

;; ---------------------------------------------------------------------------
;; files — screen 3d
;; ---------------------------------------------------------------------------
;;
;; the files claude has touched, with what he left in each. **not every file in
;; the workspace**, and that is a limit worth stating rather than hiding: no
;; capability walks a directory, so what this can enumerate is what the store
;; knows — and what the store knows is exactly the activity column 3d draws.
;; a whole-workspace files picker needs a directory-walking capability that the
;; vocabulary does not have; when one lands, this source grows a second half
;; and its rows keep their shape.
(define (picker/files-rows)
  (let ([by-path (hash)])
    (for-each
     (lambda (region)
       (let* ([path (hash-ref region "path")]
              [so-far (if (hash-contains? by-path path) (hash-ref by-path path) 0)])
         (set! by-path (hash-insert by-path path (+ so-far 1)))))
     (unseen-regions))
    (map (lambda (path)
           (picker/row
            (picker/run path 'text)
            (picker/run "  " 'meta)
            (picker/run (picker/regions (hash-ref by-path path)) 'claude)))
         (hash-keys->list by-path))))

(define-picker-source!
  "files"
  "(lambda (args) (view/spans (picker/files-rows)))")
