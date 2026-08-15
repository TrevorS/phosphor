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
;;   pickers/         picker sources and their columns                (T045)
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
