;; arch.scm — `:arch`, screen 6a (T048).
;;
;; **the proof that the escape hatch is sufficient for a real custom surface.**
;; every row below is `view/spans`, T080's one custom-draw path, so this whole
;; screen adds *zero lines* to phosphor-ui. that is the task's acceptance and it
;; is checkable: `scripts/lint-one-escape-hatch.sh` proves `Node::Spans` is the
;; only way to draw something the primitive set does not cover, and nothing in
;; this file is a primitive.
;;
;; 6a's caption is the thesis: "the editor can draw its own architecture · every
;; feature is a query over one store". so the diagram is drawn here and the
;; *numbers in it* come from `(arch)` — which is why this is not a static
;; drawing. open it with regions in the store and the store box says so.

;; ---------------------------------------------------------------------------
;; helpers
;; ---------------------------------------------------------------------------

(define (arch/run text tone) (view/run text tone 'plain))
(define (arch/row . runs) (view/span-row runs void))

;; a row of plain rule-drawing, in the meta neutral — §1 gives box drawing no
;; colour of its own, and meta is what every other rule on screen uses.
(define (arch/rule text) (arch/row (arch/run text 'meta)))

(define (arch/n n) (number->string n))

;; ---------------------------------------------------------------------------
;; the surface
;; ---------------------------------------------------------------------------
;;
;; the four producers across the top, the store in the middle, and the two
;; callers underneath — 6a's own layout. the store's right-hand column carries
;; live counts where the mockup writes the noun list, because a number is the
;; part that can be wrong.
(define (arch/rows)
  (let* ([shape (arch)]
         [unseen (hash-ref shape "unseen")]
         [seen (hash-ref shape "seen")]
         [anchors (hash-ref shape "anchors")]
         [diagnostics (hash-ref shape "diagnostics")]
         [languages (hash-ref shape "languages")])
    (list
     ;; the producers. each in the actor colour it belongs to: tree-sitter and
     ;; LSP are the editor's own machinery (meta), ACP and MCP are claude's
     ;; (green), which is §1's "green always means claude" applied to a wiring
     ;; diagram rather than to a line of code.
     (arch/row
      (arch/run "    tree-sitter        LSP          " 'text)
      (arch/run "ACP              MCP" 'claude))
     (arch/row
      (arch/run "    syntax nodes      meaning       " 'meta)
      (arch/run "agent stream     tool surface" 'meta))
     (arch/rule "         │             │              │                │")
     (arch/rule "         └──────┬──────┴──────┬───────┴────────────────┘")
     (arch/rule "                ▼             ▼")
     (arch/rule "        ┌────────────────────────────────┐")
     ;; the store, and the two rows that make this live.
     (arch/row
      (arch/run "        │  " 'meta)
      (arch/run "semantic store" 'text)
      (arch/run "                │  " 'meta)
      (arch/run (string-append (arch/n unseen) " unseen · " (arch/n seen) " seen") 'claude))
     (arch/row
      (arch/run "        │  " 'meta)
      (arch/run "who touched what, and why" 'meta)
      (arch/run "     │  " 'meta)
      (arch/run (string-append (arch/n anchors) " anchors · "
                               (arch/n diagnostics) " diagnostics")
                'text))
     (arch/rule "        └────────────────────────────────┘")
     (arch/rule "                ▲             ▲")
     (arch/rule "         ┌──────┴──────┐      │")
     (arch/row
      (arch/run "    " 'text)
      (arch/run "steel repl + config" 'steel)
      (arch/run "   " 'text)
      (arch/run "vim grammar" 'text)
      (arch/run " — motions · text objects · ex" 'meta))
     (arch/row
      (arch/run "    one API, two callers: you and claude" 'meta))
     (arch/row (arch/run "" 'meta))
     (arch/row
      (arch/run "    " 'text)
      (arch/run (string-append (arch/n languages) " languages declared") 'meta)
      (arch/run "  ·  every row above is view/spans" 'meta)))))

;; the float itself. informational mood — 6a is not in front of anything and
;; asks nothing (§4/§9) — and `q close` as the footer the mockup draws.
;; `view/float` takes mood, header, body, footer — the order `Float`'s own
;; `wire_record!` declares. a node goes wherever a child is wanted, which is why
;; the body and footer are nodes rather than wrapped.
(define-float-surface!
  "arch"
  "(lambda (args)
     (view/float 'informational
                 (view/float-header \":arch\" \"the substrate\")
                 (view/spans (arch/rows))
                 (view/key-hints 'footer (list (view/key-hint \"q\" \"close\")))))")

;; ---------------------------------------------------------------------------
;; the command
;; ---------------------------------------------------------------------------
;;
;; `:arch`, with no shorter spelling: `:a` is a prefix somebody will want for
;; something else, and 6a's own footer writes the command out in full.
;;
;; **an ex command and not a leader key**, which is the rule this file inherits
;; rather than decides: 3c draws six leader rows and this is not one of them.
(ex-set! "arch" "the editor's own architecture, over the live store"
         (lambda (rest bang)
           (key/run (key/cmd "open-float" "surface" "arch" "args" (hash)))))
