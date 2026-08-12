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
;;   leader.scm       the leader tree behind which-key                (T033)
;;   pickers/         picker sources and their columns                (T045)
;;   permissions.scm  the rules an always-allow answer writes         (T061)
;;   inbox.scm        what the inbox groups by                        (T067)
;;   watch.scm        watch placement and formatting                  (T075)
;;
;; names that do not exist are left out rather than listed: a boot float on
;; every start would teach you to ignore boot floats.
;; persisted.scm is last on purpose: it holds what the repl wrote down, and a
;; form written there may use anything the files before it defined.
(define phosphor/boot-files '("keymaps.scm" "statusline.scm" "repl.scm" "persisted.scm"))

;; ---------------------------------------------------------------------------
;; defaults
;; ---------------------------------------------------------------------------

;; soft wrap off. long lines run off the right edge and you scroll to them;
;; turning it on gives you ↪ continuations instead. off is the default because
;; a wrapped line moves every row under it, and code is read by shape.
(set-option! "soft-wrap" #f)
