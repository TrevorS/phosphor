;; keymaps.scm — the live keymap. **every binding in the editor is in this file.**
;;
;; there is no keymap in rust. T033's acceptance is exactly that sentence, and
;; `crates/phosphor-steel/tests/no_bindings_in_rust.rs` is what holds it: it
;; reads the rust tree and fails if any of it binds a key. the machine
;; (`phosphor_core::input`) knows how to *compose* a grammar — operators take
;; operands, counts multiply, `i` names an object — and knows nothing about
;; which key plays which part. it asks this file, on every keystroke, and never
;; caches the answer. that is the whole of the liveness claim: a binding you
;; change at :repl is in force on the very next key, with no reload step.
;;
;; ---------------------------------------------------------------------------
;; what a binding is
;; ---------------------------------------------------------------------------
;;
;; two kinds, and the difference is who acts:
;;
;;   a **role** — plain data, `(key/motion "word-forward")`. the machine reads
;;   it and composes: `w` moves, `dw` deletes a word, `2dw` deletes two. only a
;;   role can be an operator's operand, because only the machine can compose.
;;
;;   a **thunk** — `(lambda () (open-repl!))`. scheme runs it and the machine
;;   emits nothing. this is 6b's form, and it is what a rebind at the repl
;;   writes. a thunk can do anything scheme can do; it cannot be `d`'s operand,
;;   because a closure has no operand.
;;
;; a role that names a capability is `(key/run (key/cmd "quit" "force" #true))`.
;; the actions it names are applied by the binary, so a role reaches the buffer;
;; a thunk reaches whatever the steel door reaches. both are legible when the
;; capability's phase has not landed — the refusal names the task.
;;
;; ---------------------------------------------------------------------------
;; the read side — the contract T034 and T086 build on
;; ---------------------------------------------------------------------------
;;
;; `(keymap-entries)` answers a list of hashes, in reading order:
;;
;;   "scope"  "normal" | "insert" | "visual" | "operator-pending" | "object"
;;   "keys"   the sequence, canonical vim notation — `"gg"`, `"<C-r>"`, `"<space>c"`
;;   "verb"   what it does, spelled out **in full** — Design Language §6: a
;;            keyhint says `:reattach`, never `:ca`. abbreviation is a typing
;;            affordance (see the ex table below), never a label.
;;   "group"  #true when something longer is bound under these keys — `SPC c` is
;;            a group, `SPC c p` is a leaf. **derived, never stored**, so a group
;;            cannot claim children it does not have.
;;
;; that is one datum for three consumers at three densities: the float footer,
;; the `SPC` grid (`3c`), the `:help` body (`6d`) — `view::Density`'s three arms
;; over one `view::KeyHint` (`key`, `verb`). the list is read *live*, so a
;; rebind at :repl appears in all three with no wiring at all.

;; ---------------------------------------------------------------------------
;; how a key sequence is spelled
;; ---------------------------------------------------------------------------

;; the canonical spelling, which is the one rust produces from a keystroke:
;; `SPC` is `<space>`, and a space is a separator rather than a key. write
;; either — `"SPC f"` and `"<space>f"` are the same binding — and this is what
;; makes them the same one.
;;
;; `no_bindings_in_rust.rs` checks this against `Key::notation` for every
;; spelling the shipped table uses, so the two parsers cannot drift.
(define (phosphor/keys spelled)
  (let loop ([at 0] [out ""])
    (cond
      [(>= at (string-length spelled)) out]
      ;; a bracketed key is copied whole: `<C-r>`, `<esc>`.
      [(equal? (substring spelled at (+ at 1)) "<")
       (let scan ([to (+ at 1)])
         (cond
           [(>= to (string-length spelled)) (string-append out (substring spelled at to))]
           [(equal? (substring spelled to (+ to 1)) ">")
            (loop (+ to 1) (string-append out (substring spelled at (+ to 1))))]
           [else (scan (+ to 1))]))]
      ;; the leader, as 3c and the Design Language spell it.
      [(and (<= (+ at 3) (string-length spelled))
            (equal? (substring spelled at (+ at 3)) "SPC"))
       (loop (+ at 3) (string-append out "<space>"))]
      ;; a run of spaces separates tokens and is never itself a key.
      [(equal? (substring spelled at (+ at 1)) " ") (loop (+ at 1) out)]
      [else (loop (+ at 1) (string-append out (substring spelled at (+ at 1))))])))

;; ---------------------------------------------------------------------------
;; the table
;; ---------------------------------------------------------------------------

;; one ordered list, not a hash of hashes. order is data here — `3c` draws the
;; leader groups in the order they are declared — and a hash has none. it is
;; walked per keystroke, which is a few hundred string compares at the rate a
;; person types.
(define phosphor/keymap '())

(define (phosphor/entry scope keys verb binding)
  (hash "scope" scope "keys" keys "verb" verb "binding" binding))

(define (phosphor/scope-of entry) (hash-try-get entry "scope"))
(define (phosphor/keys-of entry) (hash-try-get entry "keys"))
(define (phosphor/verb-of entry) (hash-try-get entry "verb"))
(define (phosphor/binding-of entry) (hash-try-get entry "binding"))

(define (phosphor/same? entry scope keys)
  (and (equal? (phosphor/scope-of entry) scope)
       (equal? (phosphor/keys-of entry) keys)))

;; is some longer sequence bound under `keys` in `scope`? this is what makes
;; `SPC` pending rather than unbound, and what makes a leader group a group.
(define (phosphor/prefix? scope keys)
  (let loop ([entries phosphor/keymap])
    (cond
      [(null? entries) #f]
      [(and (equal? (phosphor/scope-of (car entries)) scope)
            (starts-with? (phosphor/keys-of (car entries)) keys)
            (> (string-length (phosphor/keys-of (car entries))) (string-length keys)))
       #t]
      [else (loop (cdr entries))])))

(define (phosphor/lookup scope keys)
  (let loop ([entries phosphor/keymap])
    (cond
      [(null? entries) #f]
      [(phosphor/same? (car entries) scope keys) (car entries)]
      [else (loop (cdr entries))])))

;; bind keys to a role or a thunk, live. 6b types exactly this:
;;
;;   (keymap-set! "]r" (lambda () (goto (next-region-by claude))))
;;
;; the repl persists the form afterwards — see repl.scm — which is why this
;; returns `void` (`#ok`) and writes nothing itself: a form loaded at boot must
;; not append itself back to the file it came from.
;;
;; the two optional arguments are last so that 6b's two-argument form keeps
;; working and reads the same:
;;
;;   (keymap-set! keys binding)
;;   (keymap-set! keys binding verb)
;;   (keymap-set! keys binding verb scope)
;;
;; a rebind keeps its position in the list, so which-key does not reshuffle
;; because you changed what a key does.
(define (keymap-set! keys binding . rest)
  (let* ([verb (if (null? rest) "" (car rest))]
         [scope (if (or (null? rest) (null? (cdr rest))) "normal" (car (cdr rest)))]
         [canon (phosphor/keys keys)]
         [fresh (phosphor/entry scope canon verb binding)])
    (set! phosphor/keymap
          (if (phosphor/lookup scope canon)
              (map (lambda (entry) (if (phosphor/same? entry scope canon) fresh entry))
                   phosphor/keymap)
              (append phosphor/keymap (list fresh))))
    void))

(define (keymap-remove! keys . rest)
  (let ([scope (if (null? rest) "normal" (car rest))]
        [canon (phosphor/keys keys)])
    (set! phosphor/keymap
          (filter (lambda (entry) (not (phosphor/same? entry scope canon)))
                  phosphor/keymap))
    void))


;; what is bound, for which-key and :help to read (T034, T086). see the header
;; for the shape — and note that no procedure crosses: a thunk is a closure and
;; a closure is exactly what may not ride on the wire.
(define (keymap-entries)
  (map (lambda (entry)
         (hash "scope" (phosphor/scope-of entry)
               "keys" (phosphor/keys-of entry)
               "verb" (phosphor/verb-of entry)
               "role" (let ([binding (phosphor/binding-of entry)])
                        (if (function? binding) void binding))
               "group" (phosphor/prefix? (phosphor/scope-of entry)
                                         (phosphor/keys-of entry))))
       phosphor/keymap))

;; every sequence bound in a scope. the older, narrower read side.
(define (keymap-keys . rest)
  (let ([scope (if (null? rest) "normal" (car rest))])
    (map phosphor/keys-of
         (filter (lambda (entry) (equal? (phosphor/scope-of entry) scope))
                 phosphor/keymap))))

;; ---------------------------------------------------------------------------
;; roles — the vocabulary a binding names
;; ---------------------------------------------------------------------------

;; each of these is one arm of `input::table::Role`. they are *data*: the
;; machine reads them, and nothing here is a closure, because a closure cannot
;; be an operator's operand and cannot cross to the other two doors.
(define (key/motion name) (list 'motion name))
(define (key/goto where) (list 'goto where))
(define (key/operator name) (list 'operator name))
(define (key/fused operator motion) (list 'fused operator motion))
(define (key/object name . delimiter) (cons 'object (cons name delimiter)))
(define (key/inner) (list 'inner))
(define (key/around) (list 'around))
(define (key/enter how) (list 'enter how))
(define (key/select kind) (list 'select kind))
(define (key/paste before) (list 'paste before))
(define (key/history redo) (list 'history redo))
(define (key/scroll request) (list 'scroll request))
(define (key/repeat) (list 'repeat))

;; a namespace rather than a command: `SPC c` is `+claude`, and pressing it
;; waits for the key that names one. it is a binding rather than a bare prefix
;; because 3c draws it with a label, and a label has to be written somewhere.
(define (key/group) (list 'group))
(define (key/escape) (list 'escape))
(define (key/register) (list 'register))

;; a capability call, as data — the name the three doors share, and its
;; arguments by name. `(key/cmd "quit" "force" #true)`.
(define (key/cmd name . pairs) (list name (apply hash pairs)))

;; a binding that is capability calls, in order.
(define (key/run . calls) (cons 'run calls))

;; the viewport's only door (`ScrollRequest`), spelled as its wire union.
(define (key/rows n) (hash "kind" "rows" "rows" n))
(define (key/pages n) (hash "kind" "pages" "pages" n))

;; the two focus-relative targets, which are the only ones a key can mean.
(define (key/at-cursor) (hash "kind" "cursor"))
(define (key/at-selection) (hash "kind" "selection"))
(define (key/focused-pane) (hash "kind" "focused"))

;; ---------------------------------------------------------------------------
;; dispatch
;; ---------------------------------------------------------------------------

;; what `keys` means in `scope`. the machine asks this and nothing else:
;;
;;   'unbound     nothing here wants it
;;   'pending     a proper prefix of something bound; wait for the next key
;;   'ran         a thunk fired — the machine emits nothing and the frame
;;                that follows is stale, because arbitrary scheme ran
;;   a role       the list a `key/…` constructor built
;;
;; **stateless.** the machine already tracks the unfinished sequence and passes
;; the whole of it, so a second copy here could only disagree with it.
(define (phosphor/resolve scope keys)
  (let* ([canon (phosphor/keys keys)]
         [entry (phosphor/lookup scope canon)])
    (cond
      [entry
       (let ([binding (phosphor/binding-of entry)])
         (cond
           [(function? binding) (begin (binding) 'ran)]
           ;; a group with nothing under it is not a namespace, it is a typo.
           [(equal? binding (key/group))
            (if (phosphor/prefix? scope canon) 'pending 'unbound)]
           [else binding]))]
      [(phosphor/prefix? scope canon) 'pending]
      [else 'unbound])))

;; ---------------------------------------------------------------------------
;; the grammar — motions, operators, objects
;; ---------------------------------------------------------------------------

;; bind one row in several scopes at once. a row is `(keys role verb)`.
(define (keymap-set-rows! scopes rows)
  (for-each
   (lambda (scope)
     (for-each
      (lambda (row)
        (keymap-set! (list-ref row 0) (list-ref row 1) (list-ref row 2) scope))
      rows))
   scopes))

;; the three scopes a motion is a motion in: it moves the cursor in normal,
;; extends a selection in visual, and is an operand in operator-pending.
(define phosphor/motion-scopes '("normal" "visual" "operator-pending"))

(define phosphor/motions
  (list
   (list "h" (key/motion "char-left") "left")
   (list "<left>" (key/motion "char-left") "left")
   (list "l" (key/motion "char-right") "right")
   (list "<right>" (key/motion "char-right") "right")
   (list "k" (key/motion "line-up") "up a line")
   (list "<up>" (key/motion "line-up") "up a line")
   (list "j" (key/motion "line-down") "down a line")
   (list "<down>" (key/motion "line-down") "down a line")
   (list "w" (key/motion "word-forward") "next word")
   (list "b" (key/motion "word-backward") "previous word")
   (list "e" (key/motion "word-end") "end of word")
   (list "0" (key/motion "line-start") "start of line")
   (list "^" (key/motion "first-non-blank") "first non-blank")
   (list "<home>" (key/motion "first-non-blank") "first non-blank")
   (list "$" (key/motion "line-end") "end of line")
   (list "<end>" (key/motion "line-end") "end of line")
   (list "{" (key/motion "paragraph-backward") "previous paragraph")
   (list "}" (key/motion "paragraph-forward") "next paragraph")
   (list "%" (key/motion "matching-bracket") "matching bracket")
   (list "H" (key/motion "screen-top") "top of screen")
   (list "M" (key/motion "screen-middle") "middle of screen")
   (list "L" (key/motion "screen-bottom") "bottom of screen")
   (list "<C-d>" (key/motion "half-page-down") "half a page down")
   (list "<C-u>" (key/motion "half-page-up") "half a page up")
   ;; a count names a line, so these are addresses rather than motions.
   (list "gg" (key/goto "first") "first line")
   (list "G" (key/goto "last") "last line")))

(keymap-set-rows! phosphor/motion-scopes phosphor/motions)

;; the operators, bound in operator-pending too so that doubling one — `dd`,
;; `yy`, `cc` — is a lookup rather than a special case in the machine.
(define phosphor/operators
  (list
   (list "d" (key/operator "delete") "delete")
   (list "c" (key/operator "change") "change")
   (list "y" (key/operator "yank") "yank")
   (list ">" (key/operator "indent") "indent")
   (list "<" (key/operator "dedent") "dedent")
   (list "gc" (key/operator "toggle-comment") "toggle comment")))

(keymap-set-rows! phosphor/motion-scopes phosphor/operators)

;; the text objects, named after `i` or `a`. the last four are 6d's agent
;; nouns: they parse here and resolve at T049, so `viu` selects nothing yet and
;; errors at nothing either (T028).
(keymap-set-rows!
 '("object")
 (list
  (list "w" (key/object "word") "word")
  (list "W" (key/object "big-word") "whitespace-delimited word")
  (list "s" (key/object "sentence") "sentence")
  (list "p" (key/object "paragraph") "paragraph")
  (list "(" (key/object "delimited" "(") "parentheses")
  (list ")" (key/object "delimited" "(") "parentheses")
  (list "{" (key/object "delimited" "{") "braces")
  (list "}" (key/object "delimited" "{") "braces")
  (list "[" (key/object "delimited" "[") "brackets")
  (list "]" (key/object "delimited" "[") "brackets")
  (list "<" (key/object "delimited" "<") "angle brackets")
  (list ">" (key/object "delimited" "<") "angle brackets")
  (list "\"" (key/object "delimited" "\"") "double quotes")
  (list "'" (key/object "delimited" "'") "single quotes")
  (list "`" (key/object "delimited" "`") "backticks")
  (list "u" (key/object "unseen-region") "unseen region")
  (list "h" (key/object "hunk") "hunk")
  (list "t" (key/object "thread") "thread")
  (list "b" (key/object "block") "review block")))

;; `i` and `a` name an object inside an operator and inside a selection.
(keymap-set-rows!
 '("operator-pending" "visual")
 (list
  (list "i" (key/inner) "inside")
  (list "a" (key/around) "around")))

;; ---------------------------------------------------------------------------
;; normal mode
;; ---------------------------------------------------------------------------

;; the fused edits — vim's one-key spellings of an operator and its operand.
(keymap-set-rows!
 '("normal")
 (list
  (list "x" (key/fused "delete" "char-right") "delete character")
  (list "<del>" (key/fused "delete" "char-right") "delete character")
  (list "X" (key/fused "delete" "char-left") "delete character before")
  (list "D" (key/fused "delete" "line-end") "delete to end of line")
  (list "C" (key/fused "change" "line-end") "change to end of line")
  (list "s" (key/fused "change" "char-right") "substitute character")
  (list "Y" (key/fused "yank" "line-end") "yank to end of line")
  (list "i" (key/enter "before") "insert")
  (list "a" (key/enter "after") "insert after")
  (list "I" (key/enter "line-start") "insert at first non-blank")
  (list "A" (key/enter "line-end") "insert at end of line")
  (list "o" (key/enter "open-below") "open a line below")
  (list "O" (key/enter "open-above") "open a line above")
  (list "R" (key/enter "replace") "replace")
  (list "v" (key/select "char") "visual")
  (list "V" (key/select "line") "visual line")
  (list "<C-v>" (key/select "block") "visual block")
  (list "p" (key/paste #f) "paste after")
  (list "P" (key/paste #t) "paste before")
  (list "u" (key/history #f) "undo")
  (list "<C-r>" (key/history #t) "redo")
  (list "." (key/repeat) "repeat the last change")
  (list "\"" (key/register) "name a register")
  (list "J" (key/run (key/cmd "join-lines" "target" (key/at-cursor))) "join lines")))

;; in visual the fused keys act on the selection, so they are the operator
;; itself, and `p` replaces what is selected.
(keymap-set-rows!
 '("visual")
 (list
  (list "x" (key/operator "delete") "delete the selection")
  (list "s" (key/operator "change") "change the selection")
  (list "v" (key/select "char") "leave visual")
  (list "V" (key/select "line") "visual line")
  (list "<C-v>" (key/select "block") "visual block")
  (list "p" (key/paste #f) "paste over the selection")
  (list "\"" (key/register) "name a register")
  (list "J" (key/run (key/cmd "join-lines" "target" (key/at-selection))) "join the selection")))

;; the viewport's only door. invariant 3: nothing else may move it, which is
;; why these are the only keys in the whole table that name a scroll.
(keymap-set-rows!
 '("normal" "visual")
 (list
  (list "<C-e>" (key/scroll (key/rows 1)) "scroll down a line")
  (list "<C-y>" (key/scroll (key/rows -1)) "scroll up a line")
  (list "<C-f>" (key/scroll (key/pages 1)) "scroll down a page")
  (list "<C-b>" (key/scroll (key/pages -1)) "scroll up a page")))

;; leaving. `<C-c>` is the safety valve — raw mode means the terminal will not
;; deliver SIGINT, so an editor with no binding for it is one you cannot get
;; out of. `ZZ` writes first; `ZQ` does not, and says so.
(keymap-set-rows!
 '("normal")
 (list
  (list "<C-c>" (key/run (key/cmd "quit" "force" #t)) ":quit!")
  (list "ZQ" (key/run (key/cmd "quit" "force" #t)) ":quit!")
  (list "ZZ"
        (key/run (key/cmd "save-buffer" "target" (key/at-cursor))
                 (key/cmd "quit" "force" #f))
        ":write-quit")))

;; `<esc>` is a mode key everywhere, including insert.
(keymap-set-rows!
 '("normal" "insert" "visual" "operator-pending" "object")
 (list (list "<esc>" (key/escape) "cancel")))

;; insert mode is text, with four exceptions that are not.
(keymap-set-rows!
 '("insert")
 (list
  (list "<left>" (key/motion "char-left") "left")
  (list "<right>" (key/motion "char-right") "right")
  (list "<up>" (key/motion "line-up") "up a line")
  (list "<down>" (key/motion "line-down") "down a line")))

;; ---------------------------------------------------------------------------
;; the leader tree — screen 3c
;; ---------------------------------------------------------------------------
;;
;; `SPC` opens the agent-native namespace, and the namespace is the reason it
;; exists: 3c's caption is *"the agent-native namespace is learnable, not
;; memorized"*.
;;
;; a group — `+claude` — is `(key/group)`, which resolves to pending while
;; anything is bound under it and to unbound when nothing is. it is a binding
;; rather than a bare prefix for one reason: 3c draws the row with a *label*,
;; and a label has to be written somewhere.
;;
;; the leaves under `+claude`, `+unseen` and `+disk` name capabilities whose
;; phase has not landed. that is deliberate and it is the design's own rule —
;; *"unimplemented is a value, not an absence"* — so pressing one answers `not
;; built yet — T058 builds it` rather than nothing at all. the binding does not
;; change when the phase lands.

;; the six rows 3c draws, in the order it draws them.

;; +claude — "prompt · steer · interrupt".
;;
;; prompt and steer are the same door: a correction is a message, and the
;; prompt line is how you type one. where a submitted prompt goes while a turn
;; is running is T062's question, not the keymap's.
(keymap-set! "SPC c" (key/group) "+claude · prompt · steer · interrupt" "normal")
(keymap-set! "SPC c p" (key/run (key/cmd "open-prompt" "kind" "claude"))
             ":claude — prompt claude" "normal")
(keymap-set! "SPC c s" (key/run (key/cmd "open-prompt" "kind" "claude"))
             ":steer — correct the turn in flight" "normal")
(keymap-set! "SPC c i" (key/run (key/cmd "interrupt-session"))
             ":interrupt — pause at the next tool boundary" "normal")

;; +unseen — "next · list · mark seen".
(keymap-set! "SPC u" (key/group) "+unseen · next · list · mark seen" "normal")
(keymap-set! "SPC u n"
             (key/run (key/cmd "goto-sequence" "sequence" "unseen-region" "seek" "next"))
             ":unseen-next — jump to the next unseen region" "normal")
(keymap-set! "SPC u l" (key/run (key/cmd "open-picker" "source" "unseen"))
             ":unseen — list every unseen region" "normal")
(keymap-set! "SPC u s" (key/run (key/cmd "mark-seen" "target" (key/at-cursor)))
             ":mark-seen — mark this region seen" "normal")

;; t transcript — a leaf, as 3c draws it.
(keymap-set! "SPC t"
             (key/run (key/cmd "set-pane-content" "pane" (key/focused-pane)
                               "kind" "transcript"))
             "transcript" "normal")

;; +disk — "refresh · diff". the two exits from a file that changed under you,
;; both manual: invariant 3 says a disk change is indicated, never injected.
(keymap-set! "SPC r" (key/group) "+disk · refresh · diff" "normal")
(keymap-set! "SPC r r" (key/run (key/cmd "reload-from-disk" "target" (key/at-cursor)))
             ":reload — take what is on disk" "normal")
(keymap-set! "SPC r d" (key/run (key/cmd "open-disk-diff" "target" (key/at-cursor)))
             ":diff-disk — your buffer against disk" "normal")

;; j jj timeline, f files — the last two leaves.
(keymap-set! "SPC j" (key/run (key/cmd "open-timeline")) "jj timeline" "normal")
(keymap-set! "SPC f" (key/run (key/cmd "open-picker" "source" "files")) "files" "normal")

;; **`:help` and `:repl` are deliberately not here.** 3c draws six rows and
;; those are the six; both surfaces are one ex command away, and a leader popup
;; that does not match its own drawing teaches the wrong thing.

;; ---------------------------------------------------------------------------
;; ex commands
;; ---------------------------------------------------------------------------
;;
;; a command is spelled **in full** everywhere it is displayed — `:write`,
;; `:reattach` — and may be *typed* short. Design Language §6 is about the
;; label; abbreviation is about the keyboard, and getting those two the wrong
;; way round is the visible design violation.
;;
;; **the rule is vim's, and it is one field.** a command is declared with the
;; abbreviable tail in brackets — `"w[rite]"` — so `:w`, `:wr`, `:wri`, `:writ`
;; and `:write` all name it and `:` alone does not. the brackets are stripped
;; for display, so there is one spelling of the name and no second table of
;; abbreviations to keep in step with it. `"wq"` has no brackets and so has no
;; short form, which is exactly why `:w` is `:write` and not ambiguous.
;;
;; a trailing `!` is a **bang**, never part of the name: `:q!` is `:quit`
;; forced. it is split off before the lookup, which is what makes it compose
;; with abbreviation rather than fight it.

;; declared -> (hash "name" … "min" … "verb" … "run" …). `run` takes the
;; argument text — everything after the first run of spaces, or "" — and the
;; bang, and answers what `phosphor/resolve` answers: a role, or `'ran` if it
;; did the work itself.
(define phosphor/ex-commands '())

(define (phosphor/ex-name-of command) (hash-try-get command "name"))
(define (phosphor/ex-min-of command) (hash-try-get command "min"))
(define (phosphor/ex-verb-of command) (hash-try-get command "verb"))
(define (phosphor/ex-run-of command) (hash-try-get command "run"))

;; `"w[rite]"` -> the name `"write"` and the fewest characters that name it, 1.
;; a spelling with no brackets may not be shortened at all.
(define (phosphor/ex-spelling declared)
  (let loop ([at 0] [name ""] [min #f])
    (cond
      [(>= at (string-length declared))
       (list name (or min (string-length name)))]
      [(equal? (substring declared at (+ at 1)) "[")
       (loop (+ at 1) name (string-length name))]
      [(equal? (substring declared at (+ at 1)) "]") (loop (+ at 1) name min)]
      [else
       (loop (+ at 1) (string-append name (substring declared at (+ at 1))) min)])))

(define (phosphor/ex-bound? name)
  (let loop ([commands phosphor/ex-commands])
    (cond
      [(null? commands) #f]
      [(equal? (phosphor/ex-name-of (car commands)) name) #t]
      [else (loop (cdr commands))])))

;; declare a command. `declared` is the name with its abbreviable tail in
;; brackets — `"q[uit]"`, or `"wq"` for one that may not be shortened.
(define (ex-set! declared verb run)
  (let* ([spelling (phosphor/ex-spelling declared)]
         [name (list-ref spelling 0)]
         [fresh (hash "name" name "min" (list-ref spelling 1) "verb" verb "run" run)])
    (set! phosphor/ex-commands
          (if (phosphor/ex-bound? name)
              (map (lambda (command)
                     (if (equal? (phosphor/ex-name-of command) name) fresh command))
                   phosphor/ex-commands)
              (append phosphor/ex-commands (list fresh))))
    void))

(define (ex-remove! name)
  (set! phosphor/ex-commands
        (filter (lambda (command) (not (equal? (phosphor/ex-name-of command) name)))
                phosphor/ex-commands))
  void)

;; every command, for :help and the ex line's own hints (T086). one hash per
;; command: "name" — always whole — and "min", how few characters of it may be
;; typed. a help grid that wants to show `w[rite]` has what it needs, and one
;; that only wants the name does not have to know the rule.
(define (ex-entries)
  (map (lambda (command)
         (hash "name" (phosphor/ex-name-of command)
               "shortest" (phosphor/ex-min-of command)
               "verb" (phosphor/ex-verb-of command)))
       phosphor/ex-commands))

;; does `typed` name this command? a prefix of it, at least as long as the
;; command allows.
(define (phosphor/ex-names? command typed)
  (and (>= (string-length typed) (phosphor/ex-min-of command))
       (starts-with? (phosphor/ex-name-of command) typed)))

;; the command `typed` names, #false if none, and the symbol `ambiguous` if
;; more than one does.
(define (phosphor/ex-lookup typed)
  (let loop ([commands phosphor/ex-commands] [found #f])
    (cond
      [(null? commands) found]
      [(equal? (phosphor/ex-name-of (car commands)) typed) (car commands)]
      [(phosphor/ex-names? (car commands) typed)
       (if found 'ambiguous (loop (cdr commands) (car commands)))]
      [else (loop (cdr commands) found)])))

;; the head of an ex line, and the rest of it.
(define (phosphor/ex-split line)
  (let loop ([at 0])
    (cond
      [(>= at (string-length line)) (list line "")]
      [(equal? (substring line at (+ at 1)) " ")
       (list (substring line 0 at) (trim (substring line at (string-length line))))]
      [else (loop (+ at 1))])))

;; run one ex line. `:` is not part of it — the prompt owns that.
;;
;;   'unbound     no command answers to what was typed
;;   'ambiguous   more than one does
;;   'ran         the command did the work itself
;;   a role       the actions the binary should apply
(define (phosphor/ex line)
  (let* ([parts (phosphor/ex-split (trim line))]
         [head (list-ref parts 0)]
         [rest (list-ref parts 1)]
         [banged (and (> (string-length head) 0)
                      (equal? (substring head (- (string-length head) 1)
                                         (string-length head))
                              "!"))]
         [typed (if banged (substring head 0 (- (string-length head) 1)) head)]
         [command (phosphor/ex-lookup typed)])
    (cond
      [(equal? typed "") 'unbound]
      [(equal? command 'ambiguous) 'ambiguous]
      [command ((phosphor/ex-run-of command) rest banged)]
      [else 'unbound])))

;; the commands themselves, spelled the way vim spells them. every one answers
;; a role, so the binary applies it through exactly the path a key does — there
;; is no second way for an ex command to reach the buffer.
(ex-set! "w[rite]" "save this buffer"
         (lambda (rest bang)
           (key/run (if (equal? rest "")
                        (key/cmd "save-buffer" "target" (key/at-cursor))
                        (key/cmd "save-buffer" "target" (key/at-cursor) "path" rest)))))

(ex-set! "wa[ll]" "save every buffer"
         (lambda (rest bang) (key/run (key/cmd "save-all"))))

(ex-set! "wq" "save this buffer and leave"
         (lambda (rest bang)
           (key/run (key/cmd "save-buffer" "target" (key/at-cursor))
                    (key/cmd "quit" "force" bang))))

(ex-set! "x[it]" "save and leave — vim's other spelling of :wq"
         (lambda (rest bang)
           (key/run (key/cmd "save-buffer" "target" (key/at-cursor))
                    (key/cmd "quit" "force" bang))))

(ex-set! "q[uit]" "leave; refuses on unsaved work unless banged"
         (lambda (rest bang) (key/run (key/cmd "quit" "force" bang))))

(ex-set! "e[dit]" "open a file"
         (lambda (rest bang)
           (key/run (key/cmd "open-file" "path" rest "pane" (key/focused-pane)))))

(ex-set! "clo[se-buffer]" "close this buffer"
         (lambda (rest bang)
           (key/run (key/cmd "close-buffer" "target" (key/at-cursor) "force" bang))))

(ex-set! "h[elp]" "the whole keymap, at width"
         (lambda (rest bang)
           (key/run (if (equal? rest "")
                        (key/cmd "open-help")
                        (key/cmd "open-help" "topic" rest)))))

(ex-set! "th[eme]" "switch theme"
         (lambda (rest bang) (key/run (key/cmd "set-theme" "slug" rest))))

(ex-set! "repl" "a steel prompt — 6b"
         (lambda (rest bang) (begin (open-repl!) 'ran)))

(ex-set! "tr[anscript]" "what claude has said"
         (lambda (rest bang)
           (key/run (key/cmd "set-pane-content" "pane" (key/focused-pane)
                             "kind" "transcript"))))

(ex-set! "ti[meline]" "agent turns are changes — 3b"
         (lambda (rest bang) (key/run (key/cmd "open-timeline"))))

(ex-set! "in[box]" "everything claude has said to you"
         (lambda (rest bang) (key/run (key/cmd "open-inbox"))))

(ex-set! "d[iff-disk]" "your buffer against what is on disk"
         (lambda (rest bang)
           (key/run (key/cmd "open-disk-diff" "target" (key/at-cursor)))))

(ex-set! "reat[tach]" "reattach to a running session"
         (lambda (rest bang) (key/run (key/cmd "reattach-session"))))

;; ---------------------------------------------------------------------------
;; the prompt key
;; ---------------------------------------------------------------------------

;; `:` opens the ex line. this is the binding the seed comment promised: the
;; repl kept `:` while there was no ex prompt, and it moves to `SPC :` now that
;; there is one. `:repl` is still one of the commands above, which is 6b's own
;; spelling for it.
(keymap-set! ":" (key/run (key/cmd "open-prompt" "kind" "ex")) ":" "normal")
(keymap-set! ":" (key/run (key/cmd "open-prompt" "kind" "ex" "anchor" (key/at-selection)))
             ":" "visual")
