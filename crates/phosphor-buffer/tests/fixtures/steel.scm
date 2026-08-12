;; T083 fixture — what `runtime/init.scm`, `runtime/keymaps.scm` and
;; `runtime/leader.scm` are going to look like once T033 writes them.
;;
;; This is deliberately not `(+ 1 2)`. Steel is a Scheme dialect, not Scheme, and
;; the point of the fixture is to make `tree-sitter-scheme` fail here if it is
;; going to fail at all. Every form below is one the plan or TASKS.md names:
;; define / lambda, require, quasiquote-built leader trees, strings with escapes,
;; `#\char` literals for key names, keyword arguments, structs, syntax-rules
;; macros, and the statusline segment cond.

#|
  Block comment. The gutter contract (Design Language §; screens 1a–1d) is
  three columns wide; `runtime/` never draws it, it only names the states.
|#

(require "steel/lists")
(require "steel/strings")
(require-builtin steel/time)

(provide default-keymap
         leader-tree
         statusline-segments
         on-buffer-open)

;; ---------------------------------------------------------------- constants

(define *leader* #\space)
(define *escape* #\escape)
(define *tab* #\tab)
(define *newline* #\newline)
(define *hex-a* #\x41)

(define *unseen-glyph* "●")
(define *thinking-glyph* "✻")
(define *needs-you-glyph* "!")

;; Escapes we actually emit: a literal quote inside a register name, a tab in a
;; help grid column, a backslash in a Windows path hint, and a hex escape.
(define *escapes* "quote:\" tab:\t newline:\n backslash:\\ hex:\x41; ")

;; ------------------------------------------------------------------ structs

(struct Binding (keys action count register) #:transparent)

(struct Language (name grammar lsp hooks) #:transparent)

;; ------------------------------------------------------------------ keymaps

(define (bind! keymap keys action #:count [count #f] #:register [register #f])
  (hash-insert keymap keys (Binding keys action count register)))

;; Counts and named registers are the two things CP-3 tests hardest (SPIKES.md,
;; T009) — so the fixture has to carry both, including the `"a` register prefix
;; which needs an escaped quote inside a Steel string.
(define default-keymap
  (let ([km (hash)])
    (bind! km "d d" 'delete-line #:count #t)
    (bind! km "3 d d" 'delete-line #:count 3)
    (bind! km "\"a y y" 'yank-line #:register #\a)
    (bind! km "c i (" 'change-inner)
    (bind! km "g g" 'buffer-top)
    km))

(define-syntax keymap
  (syntax-rules ()
    [(_ name (keys action) ...)
     (define name (list (cons keys (quote action)) ...))]))

(keymap normal-mode
        ("j" cursor-down)
        ("k" cursor-up)
        ("z z" centre-cursor))

;; ------------------------------------------------------------- leader tree

;; The which-key tree is built with quasiquote so a group can splice in a list
;; produced at load time — the picker sources register themselves this way.
(define (picker-entries)
  (list `(#\f "files" ,(lambda () (action! 'picker/files)))
        `(#\b "buffers" ,(lambda () (action! 'picker/buffers)))))

(define leader-tree
  `(leader ,*leader*
           (group #\p "project" ,@(picker-entries))
           (group #\g
                  "git"
                  (#\s "status" ,(lambda () (action! 'vcs/status)))
                  (#\d "diff" ,(lambda () (action! 'vcs/diff))))
           (entry #\q "quit" ,(lambda () (action! 'app/quit)))))

;; ----------------------------------------------------------------- segments

(define (statusline-segments state)
  (cond
    [(eq? (hash-ref state 'mode) 'insert) "-- INSERT --"]
    [(> (hash-ref state 'unseen 0) 0)
     (string-append *unseen-glyph* (number->string (hash-ref state 'unseen)))]
    [(hash-ref state 'thinking #f) *thinking-glyph*]
    [(hash-ref state 'needs-you #f) *needs-you-glyph*]
    [else ""]))

;; --------------------------------------------------------------- languages

(define first-class
  '#(steel rust typescript javascript python markdown json csv toml yaml html css))

(define (define-language! name #:grammar grammar #:lsp [lsp '()] #:hooks [hooks '()])
  (Language name grammar lsp hooks))

(define steel-language
  (define-language! 'steel
                    #:grammar 'scheme
                    #:lsp '("steel-language-server")
                    #:hooks (list (cons 'after-save 'format-buffer))))

;; ------------------------------------------------------------------- hooks

(define (on-buffer-open buf)
  (when (string-suffix? (buffer-path buf) ".scm")
    (set-language! buf 'steel))
  (unless (buffer-read-only? buf)
    (add-hook! 'after-save (lambda (b) (format-buffer b))))
  buf)

(define (count-folds lines)
  (let loop ([rest lines] [n 0])
    (if (null? rest)
        n
        (loop (cdr rest) (if (fold-start? (car rest)) (+ n 1) n)))))

(define (visible-widths lines)
  (map (lambda (l) (string-length l)) (filter (lambda (l) (not (blank? l))) lines)))

;; Numbers the reader has to cope with: rationals, floats, negatives, hex.
(define *numbers* (list 1 -2 3.5 1/2 #xff #b1010 #o17 1e3))

#;(define this-form-is-datum-commented-out
    (never-called))
