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
;; 3d's caption is the whole specification: "the file picker carries agent
;; state: unseen counts + activity, **not just names**". so the list is the
;; *workspace*, and the store annotates it — the mockup's own rows include
;; `src/main.rs` and `Cargo.toml` carrying no activity at all.
;;
;; **this listed only files with regions once, and that was wrong.** an
;; ordinary build with nothing declared opened an empty picker under a key
;; labelled "files". reported by Teej testing it; the store is the annotation,
;; never the filter.
;;
;; the file list arrives in `args` because no capability walks a directory and
;; a source runs inside the VM (OPEN-QUESTIONS §42) — the same seam that hands
;; `grep` the buffer's lines. rust walks, steel decides what a row says.
;;
;; **a row's head is its address, and there are two spellings.** the head is the
;; first whitespace-separated token, and `picker-accept` reads it:
;;
;;   `path:line` — a *place*. what `grep`, `unseen` and `references` write, and
;;                 what 8a draws (`src/retry.rs:9`). `↵` opens it at that line.
;;   `path`      — a *file*. what this source writes, and what 3d draws
;;                 (`src/main.rs`, bare, under a footer of `↵ open`). `↵` opens
;;                 it carrying no position, so a fresh buffer starts at the top
;;                 and a file you already have open keeps its cursor.
;;
;; this is written here, and not only in `accept_picker`, because a new source
;; is written in this file: the doc comment on the rust side said *every* source
;; writes `path:line` first, which was false about this one and made `↵` decline
;; every files row until Teej pressed it at a real terminal.

;; how many unseen regions each path has, as a hash.
(define (picker/unseen-by-path)
  (let ([counts (hash)])
    (for-each
     (lambda (region)
       (let* ([path (hash-ref region "path")]
              [so-far (if (hash-contains? counts path) (hash-ref counts path) 0)])
         (set! counts (hash-insert counts path (+ so-far 1)))))
     (unseen-regions))
    counts))

(define (picker/files-rows args)
  (let ([counts (picker/unseen-by-path)])
    (map (lambda (path)
           (if (hash-contains? counts path)
               ;; a file claude has been in. §2's `●` is the unseen marker
               ;; the gutter draws, and §1 makes it green because it is his.
               (picker/row
                (picker/run path 'text)
                (picker/run "  " 'meta)
                (picker/run (string-append "●" (number->string (hash-ref counts path))
                                           " unseen")
                            'claude))
               ;; and one he has not — a name, and nothing claimed about it.
               (picker/row (picker/run path 'text))))
         (hash-ref args "files"))))

(define-picker-source!
  "files"
  "(lambda (args) (view/spans (picker/files-rows args)))")

;; ---------------------------------------------------------------------------
;; grep — screen 8a
;; ---------------------------------------------------------------------------
;;
;; 8a's caption: "same picker anatomy as unseen/files/inbox · results know who
;; touched them". a row is `path:line`, then the unseen dot if the store knows
;; that line, then the line's text — which is exactly what the mockup draws:
;;
;;   ▸ src/retry.rs:9   ●  pub max_delay: Duration,
;;
;; **the open buffer, not the workspace**, and it is the same limit `files`
;; states for the same reason: no capability searches files on disk. what this
;; can read is `buffer-lines`, so grep is a fuzzy search over what is open. the
;; matching itself is nucleo's — a source hands over every line and the filter
;; narrows it, which is why this does no matching of its own and why typing in
;; the picker is grep's own prompt.

;; the lines the store has an unseen region on, as a set keyed by `path:line`.
;; built once per open rather than per row: `8a`'s dot is a *store* fact and
;; asking per line would be a query per row.
(define (picker/unseen-lines)
  (let ([marked (hash)])
    (for-each
     (lambda (region)
       (let* ([path (hash-ref region "path")]
              [span (hash-ref region "span")]
              [from (hash-ref (hash-ref span "start") "line")]
              [to (hash-ref (hash-ref span "end") "line")])
         (let walk ([line from])
           (when (<= line to)
             (set! marked (hash-insert marked
                                       (string-append path ":" (number->string line))
                                       #true))
             (walk (+ line 1))))))
     (unseen-regions))
    marked))

(define (picker/grep-rows args)
  ;; the path is handed *down* in `args` rather than queried: a source runs
  ;; inside the VM and a query from there cannot reach the editor
  ;; (OPEN-QUESTIONS §42). that is the same shape `Scope` uses for the cursor —
  ;; the host resolves what only the host can, and passes coordinates.
  (let* ([path (hash-ref args "path")]
         ;; handed down too, and for the same reason: `buffer-lines` answers on
         ;; the keystroke side only (T026) and a source runs inside the VM.
         [lines (hash-ref args "lines")]
         [marked (picker/unseen-lines)])
    (if (not path)
        '()
        (let walk ([rest lines] [n 1] [rows '()])
          (if (null? rest)
              (reverse rows)
              (let* ([at (string-append path ":" (number->string n))]
                     [seen? (hash-contains? marked at)])
                (walk (cdr rest)
                      (+ n 1)
                      (cons (picker/row
                             (picker/run at 'text)
                             (picker/run "  " 'meta)
                             ;; §2: one cell, one concept. `●` is the unseen
                             ;; marker the gutter draws, and a space keeps the
                             ;; text column aligned when there is nothing to
                             ;; say.
                             (picker/run (if seen? "●" " ") 'claude)
                             (picker/run "  " 'meta)
                             (picker/run (car rest) 'text))
                            rows))))))))

(define-picker-source!
  "grep"
  "(lambda (args) (view/spans (picker/grep-rows args)))")

;; ---------------------------------------------------------------------------
;; references — `gr`, filled by the language server
;; ---------------------------------------------------------------------------
;;
;; `gr` is bound to `request-references`, the server answers a list of places,
;; and this draws them. the places arrive in `args` for the reason the buffer's
;; lines do: **nothing in the vocabulary carries a list of places**, which is
;; the sentence that put `request-references` on T047 in the first place.
;;
;; a row is `path:line` and the file's name, which is what `↵` needs — accept
;; reads the row's own first token, so what you see is what opens.
(define (picker/place-row place)
  (let* ([path (hash-ref place "path")]
         [span (hash-ref place "span")]
         [line (if span (hash-ref (hash-ref span "start") "line") 1)])
    (picker/row
     (picker/run (string-append path ":" (number->string line)) 'text)
     (picker/run "  " 'meta)
     (picker/run "reference" 'steel))))

(define-picker-source!
  "references"
  "(lambda (args) (view/spans (map picker/place-row (hash-ref args \"places\"))))")

;; ---------------------------------------------------------------------------
;; the order tab cycles — 8a's "one float, one grammar"
;; ---------------------------------------------------------------------------
;;
;; **the layer owns this list and rust reads it**, which is what makes a user's
;; fourth source reachable by tab without a rebuild — the same argument
;; `phosphor/boot-files` settles for the load order.
;;
;; `symbols` is absent and that is a gap rather than a choice: the vocabulary's
;; LSP `Question` has `Definition` and `References` and no `DocumentSymbol`, so
;; there is nothing to ask. adding it is a capability change; T047 records it.
(define phosphor/picker-sources '("grep" "files" "unseen"))
