;; init.scm — the boot session.
;;
;; not a config file. this is the editor's own first repl session: every form
;; here is one you could type at :repl, and anything you type there can be
;; persisted back into this file. nothing here needs a restart to change.
;;
;; each top-level form is run on its own. one that fails is dropped, named in
;; the boot float with its line, and the forms around it still run — so a stray
;; paren costs you one form, never the editor.
;;
;; what belongs here: the load order, and the defaults two reasonable users
;; would answer differently. what does not: anything that could corrupt a
;; buffer or drop a frame. that is rust's, and steel cannot reach it — the only
;; two things this file can do are emit actions and read the store.

;; ---------------------------------------------------------------------------
;; load order
;; ---------------------------------------------------------------------------

;; the rest of the editor layer, in the order it loads. rust reads this list
;; after the last form in this file has run, then loads each name in turn from
;; the runtime directory — one file's failure never discards the others, and a
;; name that leaves the tree is refused.
;;
;; still to land, each with the task that writes it:
;;   permissions.scm  the rules an always-allow answer writes         (T061)
;;   inbox.scm        what the inbox groups by                        (T067)
;;   watch.scm        watch placement and formatting                  (T075)
;;
;; names that do not exist are left out rather than listed: a boot float on
;; every start would teach you to ignore boot floats.
;;
;; **persisted.scm is not in this list, and that is T101.** it is no longer a
;; file in the shipped tree — it lives in `$XDG_CONFIG_HOME/phosphor/` and the
;; binary loads it after this whole order has run (repl.scm's
;; `phosphor/persist-file`).  it has to load last, because a form written there
;; may name anything the files above defined; naming it here would have made
;; "last" a position in a list rather than a property of the boot.
;;
;; languages/ is twelve entries rather than one, and the length is the feature:
;; this list is the whole of what "the bundled set" means (T037), so a thirteenth
;; language is a file beside them and a name here — no rust, no rebuild. drop one
;; and that language is gone; the editor holds no copy.
(define phosphor/boot-files
  '("keymaps.scm"
    "statusline.scm"
    "pickers.scm"
    "arch.scm"
    ;; `T057` — 7d/5d, the second surface built entirely from the spans hatch.
    "dashboard.scm"
    ;; `T059` — 4a, and the first editor-layer surface built from ordinary
    ;; chrome rather than from the spans hatch.
    "asks.scm"
    ;; `T061` — 7a's rule. **this one has to be in this list**, not loaded
    ;; later: `Layer::load_persisted` runs every persisted form after the whole
    ;; boot order, so `allow` being free at that point is a boot float on every
    ;; start. OPEN-QUESTIONS.md ss35.
    ;;
    ;; a comment here used to be able to truncate this list: the test that
    ;; checks it against the directory found the first opening bracket and the
    ;; first closing one, so a comment quoting a form hid the six languages
    ;; after it — and the sentence explaining that did it again. the test strips
    ;; comments now, the way scheme's own reader always did, so this paragraph
    ;; is prose rather than a hazard.
    "permissions.scm"
    ;; `T065`/`T066` — 8b/4b/2b, the review surfaces.
    "review.scm"
    ;; `T067` — 5c, the inbox.
    "inbox.scm"
    ;; `T069` — 1d, the file that changed underneath you.
    "disk.scm"
    ;; `T070` — 5b, your buffer against what claude wrote.
    "diskdiff.scm"
    ;; `T073` — 3b, the jj timeline.
    "timeline.scm"
    "languages/typescript.scm"
    "languages/javascript.scm"
    "languages/rust.scm"
    "languages/python.scm"
    "languages/steel.scm"
    "languages/markdown.scm"
    "languages/json.scm"
    "languages/csv.scm"
    "languages/toml.scm"
    "languages/yaml.scm"
    "languages/html.scm"
    "languages/css.scm"
    "repl.scm"))

;; ---------------------------------------------------------------------------
;; defaults
;; ---------------------------------------------------------------------------

;; soft wrap off. long lines run off the right edge and you scroll to them;
;; turning it on gives you ↪ continuations instead. off is the default because
;; a wrapped line moves every row under it, and code is read by shape.
(set-option! "soft-wrap" #f)

;; how much of a word has to be behind the cursor before *typing* raises the
;; completion list. measured on the word prefix the cursor sits in — the same
;; span the list is filtered against and the same span accepting a row
;; overwrites — so it is not a count of keystrokes and does not reset.
;;
;; **CP-4 found this at zero**, which is what a missing floor looks like: a
;; space raised the server's whole table, and the first letter of an identifier
;; raised the longest list that letter has. two is the shortest prefix that
;; says you meant a word rather than that you pressed a key. set it to 0 for
;; vim's `completeopt`-style eagerness, or higher to be left alone.
;;
;; `<C-x>` is unaffected in either direction: asking is asking, and it answers
;; on an empty line.
(set-option! "completion-min-chars" 2)

;; which lines hang an inline `┊ ■` diagnostic row, and how many.
;;
;; reported at CP-4 and the report is the argument: a half-typed `path:` made
;; rust-analyzer answer with **eleven** cascade parse errors — `expected COMMA`,
;; `expected R_PAREN`, `expected field declaration` — and every one became a
;; row, so eleven rows of the parser resynchronising pushed the code being
;; edited off the bottom of the screen.
;;
;; **nothing is hidden by this.** §3 gives a diagnostic three surfaces and this
;; bounds only the third: the state bar in gutter column 1 still marks every
;; line that has one, the undercurl still sits under every span, and the
;; statusline still counts every one of them (`■ 3`, screen 2b). what the option
;; decides is how many also *speak*.
;;
;; `"cursor-line"` is helix's default (`other-lines: disable`) and is ours.
;; `"all"` is the old behaviour, bounded; `"off"` leaves the bar and the count.
(set-option! "diagnostic-rows" "cursor-line")

;; three rather than helix's ten, because helix soft-wraps its inline block into
;; a bounded width and §11 forbids wrapping outright — so a row here is always
;; a whole line of the buffer's height, and ten is a third of an 80x24 screen.
;; the overflow is said rather than swallowed: a fourth row reads `■ n more
;; here`.
(set-option! "diagnostic-max-rows" 3)

;; how many cells a tab is worth — the tabstop. this is **two answers in one
;; number** and that is deliberate: it is how wide a `\t` in the file *draws*,
;; and it is how wide one indent level is when levels are made of spaces.
;;
;; **CP-4 found the first half missing entirely**: the renderer replaced every
;; tab with a single space, so a tab-indented file showed one column of indent
;; per level — *"tab only seems to go a space at a time when indenting"*. there
;; was no option to set, either; nothing in the build knew what a tabstop was.
;;
;; four, because that is what was asked for and because it is what the vendored
;; editor's own hardcoded table already gave rust, python, toml and html. set it
;; to 8 for the terminal's historical stop, or 2 if you want everything narrow.
(set-option! "tab-width" 4)

;; whether one indent level is spaces or a real tab — vim's `expandtab`.
;;
;; `#t` is spaces, `tab-width` of them. `#f` makes `>`, `<` and `<tab>` all
;; write a literal `\t`, which still *draws* at `tab-width` because that is what
;; a tabstop is.
;;
;; **this is the global answer and a language may override it.** the `indent`
;; field of `define-language!` says what one level is for that language,
;; literally — `"  "` for the eight of the shipped twelve whose communities
;; settled on two spaces — and a declaration beats this pair, the way vim's
;; `ftplugin` beats a global `set`. a language declaring `"indent" "\t"` gets
;; tabs in a build where everything else gets spaces, which is what go wants and
;; is why the field is a string rather than a width.
;;
;; vim's other two knobs are deliberately absent. `shiftwidth` exists because a
;; file can mix tabs and spaces and *"how far `>>` shifts"* is then a different
;; question from *"how wide a tab draws"*; here one unit answers both, which is
;; what every modern editor ships as a single tab size. `softtabstop` is what
;; makes `<bs>` eat a whole spaces-indent — a backspace behaviour, and a real
;; gap rather than a rejected one.
(set-option! "expand-tab" #t)

;; T095 — whether the editor sweeps its own undo journals.
;;
;; `journal.rs` has implemented compaction since T030 and proves it under a real
;; SIGKILL; what it never had was a caller, so a history only ever grew and the
;; first person to keep a long session was the one who found out.
;;
;; The sweep is the layer's policy rather than a rust constant because that is
;; T095's whole shape: *"a journal compacts on a policy the editor layer names
;; rather than on nothing"*. Turning it off is a legitimate thing to want — the
;; journal is how `u` survives a restart, and a person debugging one wants it
;; append-only — so the option exists rather than the behaviour being wired in.
;;
;; When it is on, the loop asks each buffer whose edit stream moved whether its
;; log wants compacting, and `Log::should_compact` decides: at least a floor of
;; records, and at least twice as many as the last compaction left. So a quiet
;; buffer is never rewritten and a busy one is rewritten less and less often.
(set-option! "history-compaction" #t)
