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

;; ---------------------------------------------------------------------------
;; folding a bracketed key — R12
;; ---------------------------------------------------------------------------
;;
;; **a bracketed key is not copied, it is folded.** `<C-K>`, `<S-C-k>` and
;; `<c-k>` are all one chord and the machine asks with exactly one spelling of
;; it, `<C-S-k>` — so a table that copied what was written held bindings no
;; keystroke could ever reach. the rules are `phosphor-core`'s
;; (`input/key.rs`), and they are three:
;;
;;   1. **order.** modifiers are spelled `C-` `A-` `S-` `D-`, in that order,
;;      whatever order they were written in (`Key::notation`).
;;   2. **case, on the character.** a capital under ctrl, alt or super is shift
;;      plus the base letter, because a terminal reporting alternate keys sends
;;      the shifted character *instead of* the shift bit (`Key::new`). ascii
;;      only: the shifted form of `Ä` is layout-dependent and not recoverable.
;;   3. **shift folds into a plain character.** `<S-a>` is `a` and `<S-A>` is
;;      `A`, because there the character already carries it
;;      (`Mods::normalised`) — and a key with no modifiers left is not
;;      bracketed at all.
;;
;; a word that names no key — `<nope>` — is **left exactly as written**, which
;; is not a concession: rust reads it as the six characters it is, so verbatim
;; is already the form the machine asks with.
;;
;; one place this is deliberately more forgiving than rust: the modifier
;; prefixes are read in either case, so `<c-k>` reaches ctrl+k rather than
;; binding five characters nobody types. that only ever turns a dead spelling
;; into a live one.

;; the modifier a two-character prefix names, or #false.
;;
;; shift is `held` and super is `cmd` here for one flat reason: `shift` and
;; `super` are both bound in steel's own prelude, and a `let` over either is a
;; macro-expansion failure at boot rather than a shadowing.
(define (phosphor/modifier prefix)
  (let ([upper (string-upcase prefix)])
    (cond
      [(equal? upper "C-") 'ctrl]
      [(or (equal? upper "A-") (equal? upper "M-")) 'alt]
      [(equal? upper "S-") 'held]
      [(equal? upper "D-") 'cmd]
      [else #f])))

;; the modifiers at the front of a bracketed key, and what is left of it.
;; answers `(ctrl alt held cmd bare)`.
(define (phosphor/split-mods inside)
  (let loop ([at 0] [ctrl #f] [alt #f] [held #f] [cmd #f])
    (let ([named (if (<= (+ at 2) (string-length inside))
                     (phosphor/modifier (substring inside at (+ at 2)))
                     #f)])
      (cond
        [(equal? named 'ctrl) (loop (+ at 2) #t alt held cmd)]
        [(equal? named 'alt) (loop (+ at 2) ctrl #t held cmd)]
        [(equal? named 'held) (loop (+ at 2) ctrl alt #t cmd)]
        [(equal? named 'cmd) (loop (+ at 2) ctrl alt held #t)]
        [else (list ctrl alt held cmd (substring inside at (string-length inside)))]))))

;; the words a non-character key answers to, and the one it is spelled back
;; with — `Named::from_word` and `Named::word`, which is why `<escape>` and
;; `<Esc>` are both `<esc>`.
(define phosphor/named-keys
  (hash "esc" "esc" "escape" "esc"
        "cr" "cr" "enter" "cr" "return" "cr"
        "tab" "tab"
        "bs" "bs" "backspace" "bs"
        "del" "del" "delete" "del"
        "ins" "ins" "insert" "ins"
        "left" "left" "right" "right" "up" "up" "down" "down"
        "home" "home" "end" "end"
        "pageup" "pageup" "pagedown" "pagedown"
        "f1" "f1" "f2" "f2" "f3" "f3" "f4" "f4" "f5" "f5" "f6" "f6"
        "f7" "f7" "f8" "f8" "f9" "f9" "f10" "f10" "f11" "f11" "f12" "f12"))

;; the four bracketed words that name a *character* rather than a named key.
(define phosphor/bracket-chars (hash "space" " " "lt" "<" "gt" ">" "bslash" "\\"))

;; is this one ascii capital? the boundary rule 2 stops at.
(define (phosphor/ascii-upper? character)
  (and (equal? (string-length character) 1)
       (string-contains? "ABCDEFGHIJKLMNOPQRSTUVWXYZ" character)))

;; one bracketed key, spelled the way a keystroke arrives — or #false when the
;; word inside names no key at all.
(define (phosphor/canon-bracket inside)
  (let* ([parts (phosphor/split-mods inside)]
         [ctrl (list-ref parts 0)]
         [alt (list-ref parts 1)]
         [written (list-ref parts 2)]
         [cmd (list-ref parts 3)]
         [bare (list-ref parts 4)]
         [lowered (string-downcase bare)]
         [character (or (hash-try-get phosphor/bracket-chars lowered)
                        (if (equal? (string-length bare) 1) bare #f))]
         [named (if character #f (hash-try-get phosphor/named-keys lowered))]
         [commanding (or ctrl alt cmd)]
         ;; rule 2, then rule 3.
         [shifted-letter (and character commanding (phosphor/ascii-upper? character))]
         [code (if shifted-letter (string-downcase character) character)]
         [held (cond
                  [shifted-letter #t]
                  [(and character (not commanding)) #f]
                  [else written])]
         [word (cond
                 [(equal? code " ") "space"]
                 [code code]
                 [else named])])
    (cond
      [(not word) #f]
      ;; a character holding nothing is the character: `<lt>` is `<`, `<w>` is
      ;; `w`, and neither is bracketed once it has been read.
      [(and code (not (equal? code " ")) (not ctrl) (not alt) (not held) (not cmd)) code]
      [else
       (string-append "<"
                      (if ctrl "C-" "")
                      (if alt "A-" "")
                      (if held "S-" "")
                      (if cmd "D-" "")
                      word
                      ">")])))

(define (phosphor/keys spelled)
  (let loop ([at 0] [out ""])
    (cond
      [(>= at (string-length spelled)) out]
      ;; a bracketed key is folded to the one spelling the machine asks with.
      [(equal? (substring spelled at (+ at 1)) "<")
       (let scan ([to (+ at 1)])
         (cond
           ;; no `>` before the end of the spelling: the `<` is the character,
           ;; exactly as rust reads `<<` (dedent twice) and `<w`.
           [(>= to (string-length spelled)) (string-append out (substring spelled at to))]
           [(equal? (substring spelled to (+ to 1)) ">")
            (loop (+ to 1)
                  (string-append out
                                 (or (phosphor/canon-bracket (substring spelled (+ at 1) to))
                                     (substring spelled at (+ to 1)))))]
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

;; does `at` fall *between two keys* of the canonical spelling `canon`? walked
;; the way `phosphor/keys` writes it and `parse_seq` reads it back: a `<…>` is
;; one key, and a `<` with no `>` after it is the character — `<w` is two keys,
;; dedent then a word.
(define (phosphor/boundary? canon at)
  (let loop ([from 0])
    (cond
      [(= from at) #t]
      [(>= from (string-length canon)) #f]
      [(equal? (substring canon from (+ from 1)) "<")
       (let scan ([to (+ from 1)])
         (cond
           ;; no `>` before the end: the `<` was the character, so the next
           ;; boundary is one along rather than at the end of the string.
           [(>= to (string-length canon)) (loop (+ from 1))]
           [(equal? (substring canon to (+ to 1)) ">") (loop (+ to 1))]
           [else (scan (+ to 1))]))]
      [else (loop (+ from 1))])))

;; is some longer sequence bound under `keys` in `scope`? this is what makes
;; `SPC` pending rather than unbound, and what makes a leader group a group.
;;
;; **a prefix is counted in keys, not in characters**, and that third condition
;; is what says so. a canonical spelling is a concatenation of keys, so a bare
;; `starts-with?` made the printable character `<` a prefix of `<space>`,
;; `<esc>`, `<C-x>` and every other bracketed binding in its scope. in insert
;; that is fatal rather than untidy: the machine holds the key waiting for the
;; rest of a sequence that never comes, then flushes the whole batch as text.
;; `CP-4` typed `a<u8>b` into a rust file and the buffer read `a8>bu<`, which
;; makes the language untypeable. `phosphor-steel`'s
;; `a_printable_character_is_not_a_prefix_of_a_bracketed_binding` reads this
;; back from the shipped table.
;;
;; the `starts-with?` stays in front of it because it is the cheap half: this
;; walks the whole table on every keystroke, and only the handful of entries
;; that already share a leading substring pay for the boundary walk.
(define (phosphor/prefix? scope keys)
  (let loop ([entries phosphor/keymap])
    (cond
      [(null? entries) #f]
      [(and (equal? (phosphor/scope-of (car entries)) scope)
            (starts-with? (phosphor/keys-of (car entries)) keys)
            (> (string-length (phosphor/keys-of (car entries))) (string-length keys))
            (phosphor/boundary? (phosphor/keys-of (car entries)) (string-length keys)))
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

;; is this a role, as against anything else a thunk might have answered?
;;
;; **the five heads the constructors here build, and nothing else.** a thunk is
;; allowed to end in a capability call — most do — and a door's receipt is a
;; list too. see `phosphor/resolve`, which is the one caller and the reason this
;; exists.
(define (key/role? value)
  (and (list? value)
       (not (null? value))
       (member (car value) '(run motion operator object group))
       #true))
(define (key/escape) (list 'escape))
(define (key/register) (list 'register))

;; `r` — the next keystroke is a *literal*, not a binding, and `count`
;; characters under the cursor become it. a sibling of `key/register` rather
;; than a mode: `R` is the mode, and it stays in one until `<esc>`.
(define (key/replace-char) (list 'replace-char))

;; a capability call, as data — the name the three doors share, and its
;; arguments by name. `(key/cmd "quit" "force" #true)`.
(define (key/cmd name . pairs) (list name (apply hash pairs)))

;; a binding that is capability calls, in order.
(define (key/run . calls) (cons 'run calls))

;; a capability named *as an argument to another capability* — `request.rs`'s
;; `Binding`, spelled as its wire union. this is not `key/cmd`: that one is a
;; call the machine runs, this one is a call some other call carries, and the
;; only place it is used is a fall-through (`<tab>` below).
;;
;; `(key/capability "insert-indent")` — arguments follow the name in pairs,
;; exactly like `key/cmd`.
(define (key/capability name . pairs)
  (hash "kind" "capability" "name" name "args" (apply hash pairs)))

;; the viewport's only door (`ScrollRequest`), spelled as its wire union.
(define (key/rows n) (hash "kind" "rows" "rows" n))
(define (key/pages n) (hash "kind" "pages" "pages" n))

;; the two focus-relative targets, which are the only ones a key can mean.
(define (key/at-cursor) (hash "kind" "cursor"))
(define (key/at-selection) (hash "kind" "selection"))
(define (key/focused-pane) (hash "kind" "focused"))
;; the other three `PaneRef` spellings. `direction` is resolved against the
;; split tree — the nearest ancestor dividing along that axis, then its
;; neighbour's nearest leaf — so `<C-w>l` in a nested layout lands in the pane
;; actually to the right rather than whichever is next in some list.
(define (key/pane-toward direction) (hash "kind" "direction" "direction" direction))
(define (key/next-pane) (hash "kind" "next"))
(define (key/prev-pane) (hash "kind" "prev"))

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
           ;; **a thunk that answers a role means it** — T099.
           ;;
           ;; this discarded the answer and always said `'ran`, which made a
           ;; function binding a *side effect only*: it could open a float or
           ;; write an option, and it could not run an Action. that is the wall
           ;; T098 recorded when it left `@` deferred — *"a keymap cannot ask a
           ;; query"* — and the wall was really this line, because asking is
           ;; fine and it is **answering** that had nowhere to go.
           ;;
           ;; `key/deferred` is `(lambda () void)` and keeps meaning `'ran`: a
           ;; thunk that answers nothing did the work itself, which is what it
           ;; always meant.
           ;;
           ;; **`key/role?` and not `list?`, and a test taught the difference.**
           ;; the first version took any list, and a thunk whose last expression
           ;; is a *capability call* answers the door's own receipt — which is
           ;; also a list. `the_rebind_is_live_on_the_very_next_key` binds
           ;; `(lambda () (open-repl!))`, and against a refusing host that made
           ;; the refusal itself look like a role: the key went `Unbound`. only
           ;; the five heads the constructors below build are roles.
           [(function? binding)
            (let ([answered (binding)])
              (if (key/role? answered) answered 'ran))]
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
   ;; the blank-separated words. same three moves, no punctuation boundaries.
   (list "W" (key/motion "big-word-forward") "next blank-separated word")
   (list "B" (key/motion "big-word-backward") "previous blank-separated word")
   (list "E" (key/motion "big-word-end") "end of blank-separated word")
   ;; the finds. the character is not part of the binding — the machine takes
   ;; the next keystroke as a literal, the way `"` takes a register name — so
   ;; the table names the motion and nothing here spells a target character.
   (list "f" (key/motion "find-char-forward") "find a character forward")
   (list "F" (key/motion "find-char-backward") "find a character back")
   (list "t" (key/motion "till-char-forward") "till before a character")
   (list "T" (key/motion "till-char-backward") "till after a character back")
   (list ";" (key/motion "repeat-find") "repeat the last find")
   (list "," (key/motion "repeat-find-reverse") "repeat the last find, back")
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
   (list "gc" (key/operator "toggle-comment") "toggle comment")
   ;; the case operators. they take an operand exactly the way `d` does —
   ;; `gUiw`, `gu2j` — which is why they are operators and not motions.
   (list "gu" (key/operator "lower") "lower case")
   (list "gU" (key/operator "upper") "upper case")
   (list "g~" (key/operator "toggle-case") "toggle case")
   ;; **`gs`, not `s`** — Teej's ruling of 2026-08-12. mockup 6d draws the
   ;; mark-seen operator as `s`; `s` is vim's substitute and `CP-3` asks that
   ;; vim habits carry, so the drawing is what changed. `g` bound only `gg` and
   ;; `gc`, so this displaced nothing, and `gsib` — *mark inner block seen* —
   ;; is the sentence 6d is about.
   ;;
   ;; the one operator that is not an edit: it opens no undo group and fills no
   ;; register, because seen-state is not the buffer. `Region::MarkSeen` landed
   ;; at T041; T064 gave it the hunk noun, so `gsih` is a keystroke rather than
   ;; a sentence about one.
   (list "gs" (key/operator "mark-seen") "mark seen")))

(keymap-set-rows! phosphor/motion-scopes phosphor/operators)

;; **the doubling shorthand for the two-key operators.** vim accepts `guu` as
;; well as `gugu`, and the rule is that the operator's *last* key doubles it.
;; the line above binds each operator whole in operator-pending, which is what
;; makes `gugu` work; these bind the tail, which is what makes `guu` work.
;;
;; operator-pending **only**. `u` in normal mode is undo and must stay undo;
;; there is no scope here where both readings are live, because the tail is only
;; a doubling when something is already waiting for an operand.
;;
;; `gc` gets no row, and the omission is the interesting one: its tail is `c`,
;; which is already `change` in operator-pending and is what makes `cc` work.
;; A binding cannot ask which operator is pending — a keymap is data — so the
;; two readings of `c` cannot both live here. `gcgc` comments a line; `gcc` does
;; not, and that is a real difference from vim-commentary rather than an
;; oversight. Closing it needs the machine to know the key that started the
;; operator, which is a bigger change than this file.
(keymap-set-rows!
 '("operator-pending")
 (list
  (list "u" (key/operator "lower") "lower case — doubles gu")
  (list "U" (key/operator "upper") "upper case — doubles gU")
  (list "~" (key/operator "toggle-case") "toggle case — doubles g~")
  (list "s" (key/operator "mark-seen") "mark seen — doubles gs")))

;; the text objects, named after `i` or `a`. the last four are 6d's agent
;; nouns.
;;
;; **`u` resolves** (T049) and **`h` resolves** (T064): `viu` selects the unseen
;; region under the cursor and `vih` the hunk, both linewise, both over the same
;; store the gutter draws from — so the noun and the marker cannot disagree.
;; `gsih` is the sentence T064 is about: mark *this* hunk seen and leave the
;; rest of the block unseen.
;;
;; the two nouns are different sets on purpose. a hunk is a region a *review
;; block* declared, so an ordinary marker is not one; and a hunk you have
;; already marked is still a hunk, where `viu` excludes what you have read —
;; because `s` has to be able to reach a hunk you marked in order to unmark it.
;;
;; `t` and `b` still select nothing. a thread needs T068's store. a review block
;; is **not a span** — it is twelve regions across three files, and the widest
;; thing an operator can be handed is one span in one buffer, so `gsib` could
;; only mark everything between the first and the last. 8b's `S here marks all
;; 12` is a key on the review *surface* (T066), not an operator over a buffer,
;; and that is a finding rather than a gap. they answer nothing rather than
;; guessing, which is what makes `dib` a no-op instead of a delete of the wrong
;; thing.
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
  ;; `~` is the case operator fused with `l`, which is exactly what vim's is:
  ;; it changes the character under the cursor and moves on.
  (list "~" (key/fused "toggle-case" "char-right") "toggle the case of a character")
  (list "r" (key/replace-char) "replace a character")
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
  (list "~" (key/operator "toggle-case") "toggle the case of the selection")
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

;; folds — vim's `z` tree, narrowed to the three that matter and the two
;; one-way spellings of the first. nothing here is a mode: a fold is a property
;; of a place in the buffer, so every one of these names a target rather than
;; toggling a state the editor holds.
;;
;; `zM` folds *to a depth*, and it is vim's `foldlevel` arithmetic: a fold
;; closes when its level is **greater than** the argument, and the outermost
;; fold is level 1 — so `0` is *everything closed*, which is what `zM` means.
;; `zR` takes no argument at all, because "everything open" has no degrees.
(keymap-set-rows!
 '("normal" "visual")
 (list
  (list "za" (key/run (key/cmd "set-fold" "target" (key/at-cursor) "state" "toggle"))
        "toggle the fold here")
  (list "zc" (key/run (key/cmd "set-fold" "target" (key/at-cursor) "state" "folded"))
        "close the fold here")
  (list "zo" (key/run (key/cmd "set-fold" "target" (key/at-cursor) "state" "unfolded"))
        "open the fold here")
  (list "zM" (key/run (key/cmd "fold-all" "level" 0)) "fold everything")
  (list "zR" (key/run (key/cmd "unfold-all")) "unfold everything")))

;; the sequence keys — 6d's `]u  [u  · ]b block-wise`. one capability
;; (`goto-sequence`) walks every sequence the store knows, so this is the same
;; row four times with a different noun, and `SPC u n` is the leader's spelling
;; of the first of them.
(keymap-set-rows!
 '("normal" "visual")
 (list
  (list "]u" (key/run (key/cmd "goto-sequence" "sequence" "unseen-region" "seek" "next"))
        "next unseen region")
  (list "[u" (key/run (key/cmd "goto-sequence" "sequence" "unseen-region" "seek" "prev"))
        "previous unseen region")
  ;; `T060` — Q9's `]!`. **no `[!` beside it**, unlike every other pair in this
  ;; block: the others walk spans in a file and have two directions because a
  ;; cursor does. this one walks a *queue*, which has an order you put things
  ;; into, and a backwards `]!` would be a second order to keep in step with the
  ;; first.
  (list "]!" (key/run (key/cmd "goto-sequence" "sequence" "ask" "seek" "next"))
        "bring back a question you pushed aside")
  (list "]b" (key/run (key/cmd "goto-sequence" "sequence" "block-file" "seek" "next"))
        "next file in the review block")
  (list "[b" (key/run (key/cmd "goto-sequence" "sequence" "block-file" "seek" "prev"))
        "previous file in the review block")))

;; ---------------------------------------------------------------------------
;; the deliberately deferred keys — T098
;; ---------------------------------------------------------------------------
;;
;; nine keys a vim user's hands reach for that this editor does not have yet:
;; macros (`q` `@`), marks (`m` `'` backtick) and search (`/` `?` `n` `N`).
;; every one of them was *unbound*, and unbound is the wrong answer twice over.
;; the first one pressed spends T035's single teaching row on a key that is
;; nobody's **by design**, and every one after it does nothing at all — so `q`
;; reads as broken rather than as deferred, which is exactly the question CP-3
;; asks: where does muscle memory break.
;;
;; bound, each says what it will be. that is the rule the leader tree's leaves
;; already follow — *"unimplemented is a value, not an absence"* — and the loop
;; says it out loud now: a key whose actions are refused puts the refusal on the
;; statusline, the way an ex line always has.
;;
;; **two of the five silent ones now speak.** T098 could only defer `q` `@` `m`
;; `'` and backtick to a thunk, because a refusal has to come from a capability
;; and none of them had one. the repair window between CP-3 and S4 gave `q` and
;; `m` theirs — `set-macro-recording` and `place-anchor` — so those two decline
;; **by name** through the same path the ex line uses. the other three are
;; argued where they are bound; each is a missing verb, not a missing decision.
;;
;; **the operand key is not consumed.** vim's `q`, `@`, `m`, `'` and backtick
;; all take a following letter and this machine has no role for one — `"` is
;; `key/register` and a mark is not a register. so `ma` answers on `m` and the
;; `a` then enters insert, which is what an unbound `m` did too. the role
;; arrives with the feature that needs it.

;; search. T058 builds the prompt, and the ex line already declines this same
;; capability by naming it. `/` and `?` name one prompt kind because there is
;; one: which direction a search runs is the prompt's argument, not the
;; keymap's.
;;
;; `n` and `N` walk what a search left behind, and walking a sequence is
;; `goto-sequence` — the same capability `]u` and `SPC u n` name, with the
;; search-match sequence in place of the unseen one.
(keymap-set-rows!
 '("normal" "visual")
 (list
  (list "/" (key/run (key/cmd "open-prompt" "kind" "search")) "search forward")
  (list "?" (key/run (key/cmd "open-prompt" "kind" "search")) "search backward")
  (list "n" (key/run (key/cmd "goto-sequence" "sequence" "search-match" "seek" "next"))
        "next match")
  (list "N" (key/run (key/cmd "goto-sequence" "sequence" "search-match" "seek" "prev"))
        "previous match")))

;; a key that is deferred and has **no capability to name**.
;;
;; it resolves — so the machine does not call it unknown and T035's one hint is
;; not spent on it — and it does nothing, because the vocabulary has no verb it
;; could honestly ask for. binding it to the nearest-looking verb would put a
;; keystroke in front of a capability that means something else, which is worse
;; than silence: the refusal would name the wrong task. the truth is in the
;; *verb*, which `:help` and which-key both draw.
;;
;; a thunk rather than an empty `key/run` on purpose: what lands here is the
;; editor layer's own implementation, and the editor layer's bindings are
;; closures. the shape does not change when the feature arrives.
;;
;; **nothing uses it today, and that is T098 finished rather than a dead
;; helper.** `q`, `@` and `m` were its callers; T099 and T042 gave all three a
;; verb that does what the key means. it is kept because deferring a key is a
;; thing this build will do again — and because `phosphor/resolve` still has to
;; treat a `void`-answering thunk as `'ran`, which is the contract this defines.
(define (key/deferred) (lambda () void))

;; macros. ruled 2026-08-12: **macros are the editor layer's, over
;; `input/feed-keys`** — recording is capturing keystrokes into a register and
;; playing is feeding them back. two things were missing and neither was a
;; keymap's to invent: a verb for *start recording*, and a query that answers a
;; register's contents so `@` can feed them. the repair window between CP-3 and
;; S4 added both, so **`q` now has a verb** and stops being silent.
;;
;; `q` is `set-macro-recording`, which means exactly what the key means, so the
;; refusal names T099 — the task that will build the recorder — instead of
;; naming nothing. the register is `q`, which is what a vim user's `qq` types
;; and the only register this row can name until an operand role exists; the
;; call is refused either way, and T099 rewrites this row when it brings the
;; role that consumes the letter.
;;
;; marks. a mark is an anchor and anchoring is T042's, which built them.
;;
;; this section used to say `'` and backtick were deferred because *"`place-anchor`
;; writes a `label` that `goto-anchor` cannot read — it takes an `AnchorId`, and
;; no capability turns a label into one"*. T042 closed that at the door rather
;; than here: `goto-anchor` now takes `anchor` (an id) **or** `label`, plus
;; `exact` for the backtick/quote difference. the door is the only place the
;; lookup can live and still be reachable from all three doors.
;;
;; **78 rows, generated, and that is the design rather than a shortcut.** a
;; binding is *data* — `input::table::Role`'s own note is "nothing here is a
;; closure" — so `m` cannot consume the `a` in `ma` by running code. the
;; alternative was a fourth `Awaiting` state in the input machine beside
;; `Register` and `ReplaceChar`; it is more machinery for the same 78 pairs, and
;; the pairs are what T042 asks for by name (`m{a-z}`, `'{a-z}`, backtick{a-z}).
;; generating them keeps the keymap a table anything can read.
(define mark-labels
  '("a" "b" "c" "d" "e" "f" "g" "h" "i" "j" "k" "l" "m"
    "n" "o" "p" "q" "r" "s" "t" "u" "v" "w" "x" "y" "z"))

;; macros — T099, and they share the marks' alphabet below because vim gives a
;; register the same twenty-six names.
;;
;; **`@` was deferred because a keymap cannot ask a query**, and the reason it
;; could not was that `register` had no arm: a thunk calling `(register "a")`
;; raised, and a raising thunk answers `Unbound`, which spends T035's one
;; teaching row on a key that is *known*. arming the query is the whole of what
;; closed it. the thunk reads the register at **press** time and hands the keys
;; to `feed-keys`, which is the shape T099 described.
;;
;; **`q<reg>` toggles, and that is a deliberate deviation from vim.** vim's `q`
;; alone stops a recording; a key here cannot be both a leaf and a prefix, and a
;; keymap has no way to ask whether recording is live — which is what `q` alone
;; would have to know. so `qa` starts and `qa` stops. the `(recording)` query is
;; what makes the toggle honest rather than a guess, and it is the same reader
;; §5's strip would use to draw vim's `recording @a`.
;; OPEN-QUESTIONS.md ss58 records the choice and the faithful alternative.
(define (macro-rows)
  (append
   (map (lambda (label)
          (list (string-append "q" label)
                ;; **the toggle is in the thunk, not in the verb.** the
                ;; capability is `on: bool` because a *door* has to be able to
                ;; say which; a person pressing the same two keys twice means
                ;; "the other one".
                (lambda ()
                  (key/run (key/cmd "set-macro-recording"
                                    "register" label
                                    "on" (not (equal? (recording) label)))))
                (string-append "record a macro into " label)))
        mark-labels)
   (map (lambda (label)
          (list (string-append "@" label)
                (lambda () (key/run (key/cmd "feed-keys" "keys" (register label))))
                (string-append "play the macro in " label)))
        mark-labels)))

(keymap-set-rows! '("normal") (macro-rows))

(keymap-set-rows!
 '("normal" "visual")
 (append
  (map (lambda (label)
         (list (string-append "m" label)
               (key/run (key/cmd "place-anchor"
                                 "at" (key/at-cursor)
                                 "label" label))
               (string-append "set mark " label)))
       mark-labels)
  (map (lambda (label)
         (list (string-append "'" label)
               (key/run (key/cmd "goto-anchor" "label" label "exact" #false))
               (string-append "go to mark " label "'s line")))
       mark-labels)
  (map (lambda (label)
         (list (string-append "`" label)
               (key/run (key/cmd "goto-anchor" "label" label "exact" #true))
               (string-append "go to mark " label)))
       mark-labels)))

;; the jumplist. its entries are anchors, which is why `jump` shipped with T042
;; rather than with the motions — a jumplist entry has to survive the rewrite
;; that moves the code it points at, and surviving a rewrite is what an anchor
;; is. `<C-o>` walks back, `<C-i>` forward.
;;
;; **`<tab>` is bound to the same thing, and without it `<C-i>` is a binding no
;; terminal can reach.** ctrl-i and tab are one byte — 0x09 — and crossterm
;; reports it as `KeyCode::Tab`, which `decode` canonicalises to `<tab>`. so a
;; keymap that only says `<C-i>` is asking for a spelling the wire never
;; produces: half the jumplist was unpressable, and nothing noticed because
;; nothing pressed it. vim treats the two as the same key for exactly this
;; reason.
;;
;; `<C-i>` stays. it is the documented name, it is what `:help` should say, and
;; a terminal speaking the kitty keyboard protocol can tell the two apart —
;; this is a second spelling of one binding, not a replacement.
(keymap-set-rows!
 '("normal")
 (list
  (list "<C-o>" (key/run (key/cmd "jump" "seek" "prev")) "back along the jumplist")
  (list "<C-i>" (key/run (key/cmd "jump" "seek" "next")) "forward along the jumplist")
  (list "<tab>" (key/run (key/cmd "jump" "seek" "next")) "forward along the jumplist")))

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

;; insert mode is text, with exceptions that are not: the four arrows here, the
;; six lsp keys below, and `<tab>`.
;;
;; `<tab>` — reported at `CP-4` as *"tab only seems to go a space at a time when
;; indenting"*. it was unbound, so it reached `Machine::insert_key`'s literal
;; `"\t"` arm and the renderer drew that in one cell; both halves are `T104`.
;; `insert-indent` types **one indent level at the cursor**, advancing to the
;; next tabstop rather than typing a fixed number of spaces — so `<tab>` after
;; `ab` lands on column 4 exactly as a real tab would, and a file indented by
;; pressing this key draws the same as one indented with `\t`.
;;
;; **it names no width, and that is the point.** how wide a level is comes from
;; `(set-option! "tab-width" …)` / `(set-option! "expand-tab" …)` in `init.scm`
;; and from the `indent` a `define-language!` declares — so `<tab>` in a yaml
;; buffer types two spaces and in a rust buffer types four, with nothing here
;; changing. a keymap that spelled the width would be four spaces frozen into
;; this file for every language, which is the rust-table-in-scheme shape `T033`
;; exists to forbid.
;;
;; **it also steps the completion list, and that is one key with one meaning
;; rather than two.** `OPEN-QUESTIONS.md` §38 is *"two tasks want `<tab>`"*:
;; `T105` wanted it to drive the float, `T104` wanted an indent level. it was
;; first ruled by §38's *third* option — give the key to one and the other a
;; different key — and Teej reversed that at `CP-4`'s manual half after running
;; the binary: *"in this form i should be able to hit tab or something to
;; select"*.
;;
;; **helix is the prior art and it is exact.** its completion menu binds `Tab`,
;; `Down` and `C-n` to the same `move_down()` (`helix-term/src/ui/menu.rs`), and
;; `move_down` from a `cursor` of `None` lands on row 0 — so the first `<tab>`
;; *selects the first row* rather than accepting it, and `<cr>` then accepts.
;; that is why the report's other half — *"enter or space doesnt accept"* — was
;; never a bug in `<cr>`: nothing had been chosen, `select = false` held, and
;; there was no comfortable key to choose with. one mechanism answers both.
;; helix's `smart-tab.supersede-menu` defaults to `#false`, which is this same
;; precedence: the menu gets the key while it is open.
;;
;; so the binding is `move-completion` with §38's **first** option — `otherwise`
;; widened from *text to type* to *a capability to run*, which `insert-indent`
;; became a name for at `T104`. with a list open `<tab>` steps it; with no list
;; open it types one indent level, and the width still comes from `set-option!`
;; and `define-language!` rather than from anything here.
;;
;; `<S-tab>` steps backwards and carries no `otherwise`: un-indenting is `<<`'s
;; job in normal mode and vim has no insert-mode dedent key to match, so with no
;; list open it says so rather than inventing one.
(keymap-set-rows!
 '("insert")
 (list
  (list "<left>" (key/motion "char-left") "left")
  (list "<right>" (key/motion "char-right") "right")
  (list "<up>" (key/motion "line-up") "up a line")
  (list "<down>" (key/motion "line-down") "down a line")
  (list "<tab>"
        (key/run (key/cmd "move-completion"
                          "delta" 1
                          "otherwise" (key/capability "insert-indent")))
        "next completion, or indent one level")
  (list "<S-tab>"
        (key/run (key/cmd "move-completion" "delta" -1))
        "previous completion")))

;; ---------------------------------------------------------------------------
;; the language server — T036, T038, T039
;; ---------------------------------------------------------------------------
;;
;; 7c is the drawing: a passive float under the word you are typing, with no
;; header and no footer. that last part is what decides these bindings — §4's
;; documented exception means the float carries **no key hints of its own**, so
;; every key that drives it has to be one your hands already know. they are
;; vim's own insert-mode completion keys, with one deliberate difference.
;;
;; **the difference from vim is which key opens.** in vim `<C-n>` opens the
;; popup *and* steps through it, because vim's keymap and vim's popup are the
;; same program. here the keymap is data — a binding names a capability and its
;; arguments, and cannot ask whether a list is open or which row is selected —
;; so one key cannot mean two capabilities. `<C-x>` opens, which is the prefix
;; vim's own completion submode is spelled with; `<C-n>` and `<C-p>` step,
;; which is exactly what they do there. that divergence is written here rather
;; than smoothed over: closing it needs a role that reads host state, which is a
;; change to the machine and not to this file.
;;
;; **and mostly you will not press `<C-x>`.** T038's acceptance is *"typing in
;; insert mode raises the float"*, and the loop asks on every insert-mode edit
;; against a server that is ready. `<C-x>` is how you ask again — after `<C-e>`,
;; or on a line you have not typed into.
;;
;; `<C-y>` accepts and `<C-e>` cancels, both exactly as vim spells them. the
;; index is **0**, which is not a row: the list is 1-based, so 0 is free to mean
;; *whichever row is selected*, and it has to mean something — a keymap is data
;; and cannot read a selection the host holds, so a literal row number here
;; would make `<C-y>` accept the same row forever.
;;
;; **every one of these is a byte a terminal can send unambiguously**, which is
;; not a detail: `<C-j>` is `0x0a`, which is the byte `<enter>` sends, so a
;; binding on it would take newlines away from insert mode on every terminal
;; there is. `<C-e>` is bound to a scroll in normal and visual and to this in
;; insert; that is not a collision — the scroll rows are declared for those two
;; scopes only, and vim gives `<C-e>` the same double life.
;;
;; ---------------------------------------------------------------------------
;; `<space>` and `<cr>`, and the guard that makes them bindable
;; ---------------------------------------------------------------------------
;;
;; reported at `CP-4`: *"i like being able to hit space to select and put a
;; space after or enter to select without a space after"*. both are here, and
;; the four vim keys above are untouched — a vim user's hands already know
;; them, and the point of these two is the hands that do not.
;;
;; **bound naively they are unusable**, and that is the whole design problem.
;; `T038`'s float is raised by *typing*, so it is open for most of the time you
;; are in insert mode: a `<space>` that accepted whatever was highlighted would
;; complete a word every time you finished one, and `<cr>` would stop making
;; newlines. every editor that offers these keys has the same guard and
;; `nvim-cmp` spells it `select = false` — **the key acts only on a row the
;; user steered to**, and otherwise falls through to what it would have typed.
;;
;; **the guard cannot be here, and the fall-through cannot be anywhere else.**
;; a keymap is data: a binding names a capability and its arguments and cannot
;; ask whether a row is selected — the same constraint the `<C-x>` note above
;; is about. so the *condition* is the host's (`Editing::chosen`, written by
;; `move-completion` and by nothing else) and the *text* is this file's,
;; because nothing above knows which key was pressed. `otherwise` carries it:
;;
;;   `<space>` — `then " "` accept and leave a space; `otherwise " "` type one
;;   `<cr>`    — no `then` at all;                    `otherwise "\n"` newline
;;
;; and `<C-y>` passes **neither**, which is what keeps vim's meaning exact:
;; pressing it *is* the choosing, so it accepts whatever is highlighted whether
;; or not you have moved. one capability, three keys, three readings of it.
;;
;; **to turn the guard off, delete the `"otherwise"` pair.** teej asked for
;; space and enter to accept, not for *"accept only when explicitly selected"* —
;; the guard is a judgement made on top of the report, and it is reversible in
;; one place, per key, with no rust:
;;
;;   `(key/cmd "accept-completion" "index" 0 "then" " ")`   — space always accepts
;;   `(key/cmd "accept-completion" "index" 0)`              — `<cr>` always accepts
;;
;; that is what `<C-y>` already does, one line up, so the behaviour is not
;; hypothetical. the argument for keeping the guard is in the paragraph above
;; and in `OPEN-QUESTIONS.md` §38; the argument against it is that it is
;; paternalistic, and this is the sentence that makes it a setting rather than
;; a decision.
;;
;; `<cr>` and not `<C-m>`: `<cr>` is the canonical spelling rust produces for
;; the enter key (`Key::notation`), and `<C-j>`/`0x0a` is deliberately still
;; unbound for the reason two paragraphs up.
;;
;; **these two rows also bind in replace mode**, because `Scope::of` folds
;; `EditMode::Replace` into the insert scope — vim's `:imap` does the same. no
;; float can be open there (the loop's completion trigger is gated on insert),
;; so the fall-through always fires, and `Editing::accept` types it the way the
;; mode types: overwriting, not inserting. `CP-4` found that the hard way, with
;; `R` quietly turned into `i`.
(keymap-set-rows!
 '("insert")
 (list
  (list "<C-x>" (key/run (key/cmd "request-completion")) "completions here")
  (list "<C-n>" (key/run (key/cmd "move-completion" "delta" 1)) "next completion")
  (list "<C-p>" (key/run (key/cmd "move-completion" "delta" -1)) "previous completion")
  (list "<C-y>" (key/run (key/cmd "accept-completion" "index" 0)) "accept the completion")
  (list "<space>"
        (key/run (key/cmd "accept-completion" "index" 0 "then" " " "otherwise" " "))
        "accept a chosen completion, and a space after it")
  (list "<cr>"
        (key/run (key/cmd "accept-completion" "index" 0 "otherwise" "\n"))
        "accept a chosen completion, with no space after it")
  (list "<C-e>" (key/run (key/cmd "cancel-completion")) "dismiss the completions")
  (list "<C-s>" (key/run (key/cmd "request-signature-help")) "what does this call take")))

;; `K` is vim's `keywordprg` key — *"look up what is under the cursor"* — and
;; hover is exactly that, so it keeps its meaning with a better source. the
;; float is dismissed by the next key, which is what a passive float with no
;; footer can offer.
;;
;; `gd` is vim's own *go to definition*, and here it is the server's. the jump
;; is an `open-file` with a position, which is why it works across files and why
;; the cursor lands on the line rather than the top of one.
;;
;; `gr` is *what uses this*, and it is bound to say so.
;;
;; **this row used to argue the other way, and the argument is kept rather than
;; deleted because it was wrong for an instructive reason.** it read: references
;; answer a *list* of places and nothing in the vocabulary carries one;
;; `request-references` was re-homed from T036 to T047 for that reason, and T047
;; builds the picker a list is drawn in — so binding `gr` to a refusal *"would
;; take `gr` away from the surface that will actually own it, and an unbound key
;; here is one keystroke of silence rather than a key that teaches the wrong
;; home."*
;;
;; both halves of that are false against this build's own rule. **silence is
;; what T098 exists to eliminate** — *"these keys are unknown when they should be
;; known and not built"* — and a refusal does not take the key away from T047
;; any more than `/` naming T058 takes `/` away from the search prompt: the row
;; is rewritten by the task that lands, which is exactly what T099 is scheduled
;; to do to `q`'s row above. and the wrong-home worry does not apply here at
;; all, which is the part the earlier reasoning got backwards: it was written for
;; keys with **no capability to name**, where the nearest-looking verb would name
;; the wrong task. `request-references` is not a near-miss — it is the verb `gr`
;; means, declared, with T047 on its own row. reported at CP-4 (*"why is gr
;; unbound it should show uses of that thing"*), which is the muscle-memory
;; question CP-3 asks, answered by a vim user's hands rather than by a reading.
;;
;; so `gr` resolves, declines, and the statusline says `not built yet — T047
;; builds it` — the task coming off the capability's own row, so this row cannot
;; go stale independently of the vocabulary.
(keymap-set-rows!
 '("normal")
 (list
  (list "K" (key/run (key/cmd "request-hover")) "what is this")
  (list "gd" (key/run (key/cmd "request-definition")) "go to the definition")
  (list "gr" (key/run (key/cmd "request-references")) "what uses this")))

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
;; T054 — 1b, and it is a *split*: the caption is "session stream as a pane"
;; and the drawing keeps your code above it. one call rather than split-then-
;; set-content, because `split-pane` takes what the new pane holds.
;;
;; down, not beside: 1b stacks them, and a transcript is a stream you read
;; downward while the code stays where it was.
(keymap-set! "SPC t"
             (key/run (key/cmd "split-pane" "pane" (key/focused-pane)
                               "direction" "down" "kind" "transcript")
                      (key/cmd "focus-pane" "pane" (key/pane-toward "down")))
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

;; vim's `CTRL-^` — back to the file you were in before this one, and again to
;; come back. **two spellings, because the terminal decides which one arrives.**
;; without the kitty protocol, ctrl+6 and ctrl+^ are the same byte (`0x1e`) and
;; crossterm decodes it as `^`; with the protocol the terminal says which key
;; was actually pressed and it arrives as `6`. binding one would work on one
;; terminal and silently not on the other, which is `T027`'s whole subject.
(keymap-set! "<C-^>" (key/run (key/cmd "open-alternate" "pane" (key/focused-pane)))
             "the previous file" "normal")
(keymap-set! "<C-6>" (key/run (key/cmd "open-alternate" "pane" (key/focused-pane)))
             "the previous file" "normal")

;; ---------------------------------------------------------------------------
;; <C-w> — windows, which vim calls them and this calls panes (T088)
;; ---------------------------------------------------------------------------
;;
;; **the four pane capabilities shipped with arms and nothing bound to them.**
;; T088's acceptance asked for arms and a query and got both; it never asked for
;; keys, so for a while the only way a person could make a split was the files
;; picker's <C-v>. these are the keys, and `scripts/lint-capability-bindings.sh`
;; is what stops the next one shipping unreachable.
;;
;; **the split direction is `splitbelow`/`splitright`, not vim's bare default.**
;; vim with neither option set puts the new window *above* and to the *left*;
;; almost every modern config sets both, telescope's <C-v>/<C-x> already behave
;; that way, and this editor's picker keys were written that way first. being
;; internally consistent beats matching an unset default nobody uses.
;;
;; **splitting moves focus, and that is a second call.** `split-pane` does not
;; move it — opening a pane and looking at it are two things, and `:sbuffer`
;; wants the first without the second. vim's <C-w>v does both, so the binding
;; does both: split, then focus in the direction the split went.
(keymap-set! "<C-w> v"
             (key/run (key/cmd "split-pane" "pane" (key/focused-pane)
                               "direction" "right" "kind" "buffer")
                      (key/cmd "focus-pane" "pane" (key/pane-toward "right")))
             "split right" "normal")
(keymap-set! "<C-w> s"
             (key/run (key/cmd "split-pane" "pane" (key/focused-pane)
                               "direction" "down" "kind" "buffer")
                      (key/cmd "focus-pane" "pane" (key/pane-toward "down")))
             "split below" "normal")

;; hjkl, the same four the buffer uses, one level out.
(keymap-set! "<C-w> h" (key/run (key/cmd "focus-pane" "pane" (key/pane-toward "left")))
             "focus left" "normal")
(keymap-set! "<C-w> j" (key/run (key/cmd "focus-pane" "pane" (key/pane-toward "down")))
             "focus below" "normal")
(keymap-set! "<C-w> k" (key/run (key/cmd "focus-pane" "pane" (key/pane-toward "up")))
             "focus above" "normal")
(keymap-set! "<C-w> l" (key/run (key/cmd "focus-pane" "pane" (key/pane-toward "right")))
             "focus right" "normal")

;; **cycle order is the tree's, not the order panes were opened.** <C-w>w walks
;; the windows as they are *arranged*, which is what makes it predictable on a
;; layout you did not build in one sitting.
(keymap-set! "<C-w> w" (key/run (key/cmd "focus-pane" "pane" (key/next-pane)))
             "focus the next pane" "normal")
(keymap-set! "<C-w> W" (key/run (key/cmd "focus-pane" "pane" (key/prev-pane)))
             "focus the previous pane" "normal")

;; closing the last pane refuses — `:quit` is the verb for leaving, and it is a
;; different question.
(keymap-set! "<C-w> c" (key/run (key/cmd "close-pane" "pane" (key/focused-pane)))
             "close this pane" "normal")

;; **the step is percentage points, and the capability's row says cells.** the
;; split tree deliberately does not know how big anything is — a tree that
;; stored cells would be wrong the moment the terminal resized — so the arm
;; reads a delta as points against the divider it moves. five is about a column
;; on an 80-wide frame, which is what a person means by "a bit wider".
(keymap-set! "<C-w> +" (key/run (key/cmd "resize-pane" "pane" (key/focused-pane) "delta" 5))
             "grow this pane" "normal")
(keymap-set! "<C-w> -" (key/run (key/cmd "resize-pane" "pane" (key/focused-pane) "delta" -5))
             "shrink this pane" "normal")

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

;; ---------------------------------------------------------------------------
;; ranges — `:'<,'>c`, `:12,20c`
;; ---------------------------------------------------------------------------
;;
;; 6d: *"anchored message over a range — ranges, like ex intended"*. a range is
;; read off the **front** of the line, before the name — which is the whole of
;; why `:'<,'>c` used to answer *no such command*: the name it looked up was
;; `'<,'>c`.
;;
;; the range is **data**, and the command lowers it, because what a range means
;; is the command's business:
;;
;;   #false                 nothing was typed before the name
;;   (selection)            `'<,'>` — the live visual selection
;;   (lines from to)        `12` or `12,20` — 1-based and inclusive
;;
;; **what is deliberately not read.** `%`, `.` and `$` name the buffer, the
;; cursor's line and the last line, and each needs a *query* answered before it
;; is a range at all — so they are not ranges here, a line starting with one is
;; read as a name, and the answer is what it was before. adding them means
;; running a query on the ex path, which is a decision about the ex line's cost
;; rather than about its grammar.

;; how far the digits at `at` run.
(define (phosphor/ex-digits line at)
  (let loop ([to at])
    (if (and (< to (string-length line))
             (string-contains? "0123456789" (substring line to (+ to 1))))
        (loop (+ to 1))
        to)))

;; the range at the front of `line`, and the line with it removed.
;; answers `(range rest)`; `range` is #false when there is none.
(define (phosphor/ex-range-at line)
  (cond
    [(starts-with? line "'<,'>")
     (list (list 'selection) (substring line 5 (string-length line)))]
    [else
     (let ([one (phosphor/ex-digits line 0)])
       (cond
         [(equal? one 0) (list #f line)]
         [(and (< one (string-length line))
               (equal? (substring line one (+ one 1)) ","))
          (let ([two (phosphor/ex-digits line (+ one 1))])
            (if (equal? two (+ one 1))
                ;; `12,` names no second address; it is not a range, so the
                ;; whole line goes to the lookup and answers for itself.
                (list #f line)
                (list (list 'lines
                            (string->number (substring line 0 one))
                            (string->number (substring line (+ one 1) two)))
                      (substring line two (string-length line)))))]
         [else
          (list (list 'lines
                      (string->number (substring line 0 one))
                      (string->number (substring line 0 one)))
                (substring line one (string-length line)))]))]))

;; the range the line **being run right now** carries. set by `phosphor/ex`
;; around the command's own procedure and cleared after it, so `ex-set!` keeps
;; the two-argument shape it documents and a command that has no use for a
;; range never learns that ranges exist.
(define phosphor/ex-current-range #f)

(define (ex-range) phosphor/ex-current-range)

;; the range as a `Target`. both spellings answer the selection, because a line
;; range is *selected first* (`ex-preamble`) — one arm in a command rather than
;; three, and it is also what ex does: a range is where the command acts.
(define (ex-anchor)
  (if (ex-range) (key/at-selection) (key/at-cursor)))

;; the calls that make `(ex-anchor)` true, to run before the command's own.
;; a linewise span is half-open, so `12,20` ends at the first column of line 21
;; — and it is *linewise*, because an ex address names a line and never a
;; column.
(define (ex-preamble)
  (let ([range (ex-range)])
    (if (and range (equal? (car range) 'lines))
        (list (key/cmd "select-range"
                       "span" (hash "start" (hash "line" (list-ref range 1) "column" 1)
                                    "end" (hash "line" (+ (list-ref range 2) 1) "column" 1))
                       "kind" "line"))
        '())))

;; run one ex line. `:` is not part of it — the prompt owns that.
;;
;;   'unbound     no command answers to what was typed
;;   'ambiguous   more than one does
;;   'ran         the command did the work itself
;;   a role       the actions the binary should apply
(define (phosphor/ex line)
  (let* ([ranged (phosphor/ex-range-at (trim line))]
         [range (list-ref ranged 0)]
         [parts (phosphor/ex-split (trim (list-ref ranged 1)))]
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
      [command
       (begin
         (set! phosphor/ex-current-range range)
         (let ([answered ((phosphor/ex-run-of command) rest banged)])
           (set! phosphor/ex-current-range #f)
           answered))]
      [else 'unbound])))

;; the commands themselves, spelled the way vim spells them. every one answers
;; a role, so the binary applies it through exactly the path a key does — there
;; is no second way for an ex command to reach the buffer.
(ex-set! "w[rite]" "save this buffer"
         (lambda (rest bang)
           (key/run (if (equal? rest "")
                        (key/cmd "save-buffer" "target" (key/at-cursor))
                        (key/cmd "save-buffer" "target" (key/at-cursor) "path" rest)))))

;; `T096` — soft wrap, as vim spells it. **two commands rather than one that
;; toggles**, which is vim's own choice and the right one here: a toggle answers
;; a question you did not ask ("was it on?") and these two state what you want.
;;
;; `:w` stays `:write` — `wrap` carries no brackets, so its shortest form is the
;; whole word and the one-letter prefix belongs to the command that had it.
(ex-set! "wrap" "wrap long lines in this buffer"
         (lambda (rest bang)
           (key/run (key/cmd "set-soft-wrap" "target" (key/at-cursor) "on" #true))))

(ex-set! "nowrap" "let long lines run off the edge"
         (lambda (rest bang)
           (key/run (key/cmd "set-soft-wrap" "target" (key/at-cursor) "on" #false))))

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

(ex-set! "h[elp]" "the keymap — :help <topic> narrows it"
         (lambda (rest bang)
           (key/run (if (equal? rest "")
                        (key/cmd "open-help")
                        (key/cmd "open-help" "topic" rest)))))

(ex-set! "th[eme]" "switch theme"
         (lambda (rest bang) (key/run (key/cmd "set-theme" "slug" rest))))

;; vim's three window commands, the spellings a person's fingers already know.
;; `:split` and `:vsplit` are the <C-w>s / <C-w>v pair under their other names,
;; and go the same way for the same reason.
(ex-set! "sp[lit]" "split this pane, below"
         (lambda (rest bang)
           (key/run (key/cmd "split-pane" "pane" (key/focused-pane)
                             "direction" "down" "kind" "buffer")
                    (key/cmd "focus-pane" "pane" (key/pane-toward "down")))))

(ex-set! "vs[plit]" "split this pane, beside"
         (lambda (rest bang)
           (key/run (key/cmd "split-pane" "pane" (key/focused-pane)
                             "direction" "right" "kind" "buffer")
                    (key/cmd "focus-pane" "pane" (key/pane-toward "right")))))

(ex-set! "clo[se]" "close this pane; the last one refuses — :quit leaves"
         (lambda (rest bang)
           (key/run (key/cmd "close-pane" "pane" (key/focused-pane)))))

(ex-set! "repl" "a steel prompt — 6b"
         (lambda (rest bang) (begin (open-repl!) 'ran)))

;; T054. `set-pane-content` and not a split, because the capability's own row
;; says so: "changes what a pane holds — :transcript is this, not a separate
;; capability". this pane becomes the transcript and `:transcript buffer` puts
;; it back, which is 1b's "closes back to full buffer" from the other end.
;;
;; `SPC t` is the *split* — 1b's drawing, where the code stays above — and it
;; composes two capabilities the way `<C-w>v` does rather than adding a third.
(ex-set! "tr[anscript]" "what claude has said — :transcript buffer goes back"
         (lambda (rest bang)
           (key/run (key/cmd "set-pane-content" "pane" (key/focused-pane)
                             "kind" (if (equal? rest "buffer") "buffer" "transcript")))))

(ex-set! "ti[meline]" "agent turns are changes — 3b"
         (lambda (rest bang) (key/run (key/cmd "open-timeline"))))

(ex-set! "in[box]" "everything claude has said to you"
         (lambda (rest bang) (key/run (key/cmd "open-inbox"))))

(ex-set! "d[iff-disk]" "your buffer against what is on disk"
         (lambda (rest bang)
           (key/run (key/cmd "open-disk-diff" "target" (key/at-cursor)))))

;; `T062` — 7e's three ways on from a tool boundary. **all three are ex
;; commands and only one of them is also a key**: `esc` pauses, which is the key
;; you already have, and what to do next is a decision rather than a reflex.
(ex-set! "resume" "carry on from the tool boundary, unchanged"
         (lambda (rest bang) (key/run (key/cmd "resume-session"))))

(ex-set! "abort" "abandon the paused turn; the held call does not run"
         (lambda (rest bang) (key/run (key/cmd "abort-turn"))))

;; `:steer` — 7e's `↵ steer & resume` said as a command. the correction is a
;; prompt, which is what makes it steering rather than a note.
(ex-set! "steer" "correct claude and carry on — :steer <what you meant>"
         (lambda (rest bang) (key/run (key/cmd "steer-session" "body" rest))))

(ex-set! "reat[tach]" "reattach to a running session"
         (lambda (rest bang) (key/run (key/cmd "reattach-session"))))

;; 6d's `:'<,'>c msg` — *"anchored message over a range — ranges, like ex
;; intended"*. the only command that reads a range today, and the reason the
;; grammar above exists: a thread is anchored to a **span**, so the range is
;; not decoration on this one, it is the argument.
;;
;; `:c` with no range anchors at the cursor, which is the same sentence with
;; the smallest range there is. the thread itself lands at T068 and the door
;; says so until then, which is the design's own rule — unimplemented is a
;; value, not an absence — and is why this answers a role rather than erroring.
;;
;; **not built:** `:g/TODO/c`, 6d's other range form. `broadcast-thread` is
;; declared for it (`action.rs`, *"one message against every match of a
;; pattern"*) and a `/pattern/` grammar is a second parser; no done-when asks
;; for it, so it is named here rather than half-written.
(ex-set! "c[omment]" "an anchored message over a range — 6d's :'<,'>c"
         (lambda (rest bang)
           (apply key/run
                  (append (ex-preamble)
                          (list (key/cmd "start-thread"
                                         "anchor" (ex-anchor)
                                         "body" rest))))))

;; T036 — restarting a language server, the one thing you do to a server rather
;; than through it. an ex command and not a leader key because 3c draws six rows
;; and those are the six; a seventh would teach a namespace the mockup does not
;; have.
;;
;; **the language is typed, and that is not a shortcut taken.** the capability
;; names a `LanguageId` and this file cannot know which language the buffer in
;; front of you is — a binding is data, and the query that would answer it
;; (`buffer`) lands at T041's store. an empty argument would have to *mean*
;; something, and the two candidate meanings — "this buffer's" and "all of
;; them" — are both invented payloads. so `:restart-server rust` says what it
;; restarts, and a bare `:restart-server` is declined by name.
(ex-set! "restart[-server]" "restart a language's server — :restart-server rust"
         (lambda (rest bang)
           (key/run (key/cmd "restart-language-server" "language" rest))))

;; T050 — a message to claude, and the plainest possible way to send one.
;;
;; `SPC c p` above raises T058's PromptLine, which is the surface this editor
;; is supposed to talk through: 1c's line, the ⚓ chip when a selection rides
;; along, ex-style history. that task is not built. this is not a substitute
;; for it and does not want to be — it is the door that makes "a session
;; attaches and *a turn completes*" reachable by a person, because nothing can
;; complete a turn nobody can start.
;;
;; no anchors, deliberately. `send-message` takes them and the arm refuses a
;; message that carries any, by naming T058: an anchor silently dropped means
;; claude answers about the wrong thing with nothing on screen to say the range
;; went missing. the ex line has no selection to offer anyway.
(ex-set! "cl[aude]" "send claude a message — :claude <message>"
         (lambda (rest bang)
           (key/run (key/cmd "send-message" "body" rest "anchors" (list)))))

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
