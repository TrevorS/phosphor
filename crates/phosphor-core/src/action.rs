//! The `Action` enum — the single mutation API, and the vocabulary all three
//! doors speak.
//!
//! Read this file before changing anything downstream of it. It is the decision
//! the plan calls *"reversible: no in practice"* (`IMPLEMENTATION-PLAN.md` §3,
//! S2's scope block), and `TEAM.md` gives it exactly one writer: **only `spine`
//! edits this enum, the query vocabulary, or the view tree.** Everyone else
//! requests an addition and waits.
//!
//! # The shape, and why
//!
//! ```text
//! Request { actor, door, action: Action }   →  apply  →  Outcome
//!                              │
//!                              └── Action::Region(RegionAction::MarkSeen { target })
//!                                          │            │
//!                                          │            └── plain data. No closures,
//!                                          │                no borrows, no Steel values.
//!                                          └── nested by domain, flat door name
//!                                              ("mark-seen").
//! ```
//!
//! Six properties, each forced by something in the tree or the docs rather than
//! by taste:
//!
//! 1. **Plain data, never a closure.** Every payload implements
//!    [`Wire`], so `T020` derives the MCP JSON schema, the
//!    CLI verb and the Steel binding from one declaration. That mechanical
//!    derivation is invariant 2's only real defence (`IMPLEMENTATION-PLAN.md`
//!    §0: *"if MCP tools are registered by hand alongside a separate Steel
//!    binding table, invariant 2 rots within a month"*). A payload that needed a
//!    scheme closure would break it — hence [`Binding`].
//! 2. **Nested by domain, flat by name.** The nesting is for dispatch and
//!    ownership: `phosphor-core` has three owners by module (`TEAM.md`), and one
//!    150-arm match in one file only `spine` may edit is permanent contention.
//!    The *door name* is flat and globally unique because Q6 fixes one of them
//!    literally — `phosphor/declare-review-block` — which no path-derived scheme
//!    produces.
//! 3. **One enum for user intent and for external ingest.** Rejected: a separate
//!    `Event` type for ACP/LSP/disk arrivals. Two write paths into a store that
//!    must have one writer, and `T024` would enumerate half the vocabulary. Two
//!    tasks make ingest a door capability outright: `T053` routes
//!    `declare-review-block` through the registry *so Steel and the CLI can
//!    declare one too* (Q6), and `V006` seeds regions, threads and a transcript
//!    through `phosphor --eval`, **not a test-only backdoor** (`TASKS.md`, V006).
//!    The cost is real: any door can *claim* an author. Mitigated, not removed —
//!    [`Actor`] rides the envelope (who asked) and the payload carries the claim
//!    (what was asserted), and the store keeps both.
//! 4. **Unimplemented is a value, not an absence.** Every variant here exists
//!    from `T019` and returns
//!    [`Refusal::NotYetImplemented`] until its phase lands, naming the task that
//!    builds it. That is what makes `T019`'s acceptance criterion — *every
//!    mutation in S3–S8 has a named Action, even if unimplemented* — a test
//!    (`tests/vocabulary.rs` against `tests/surfaces.txt`) rather than a claim.
//! 5. **MCP parity is enforced by policy, not by a smaller vocabulary.** Every
//!    Action is registered in all three doors; [`McpPolicy`] says which ones an
//!    agent may call without a rule. *"Nothing moves unless you asked"* is about
//!    the actor, not the capability.
//! 6. **`#[non_exhaustive]` at the top, exhaustive per domain.** A new domain is
//!    additive for everything that matches on [`Action`]; a new verb inside a
//!    domain still breaks every match on that domain, which is where the
//!    compiler should be shouting.
//!
//! # What is deliberately *not* an Action
//!
//! Terminal resize · the 80ms spinner frame · the 1s elapsed timer ·
//! tree-sitter reparse · redraw. None has an actor, an undo meaning, or
//! anything to refuse; they are loop events that update derived state. An MCP
//! tool named `phosphor/tick` is the tell that the shape is wrong, and a
//! store revision bumped 12.5 times a second would defeat `T079`'s frame cache
//! before it is built.
//!
//! **Consequence, recorded for `T078`:** the view tree needs two time-derived
//! node kinds — a spinner taking a start instant, and an elapsed-since taking
//! one — that Rust re-renders per frame without re-entering the VM. Without
//! them, Design Language §8's three animations and the frame cache are in direct
//! conflict.
//!
//! [`ScrollRequest`] *is* an Action payload, and
//! it is the boundary case: a viewport that moves is precisely what invariant 3
//! forbids doing unasked, so it is a request with a caller (`main.rs`'s header,
//! finding 1).
//!
//! # Adding one
//!
//! Add a row to the right domain in the `actions!` table below. That single row emits the
//! variant, its rustdoc, its registry entry, its decoder and its encoder — there
//! is nowhere else to edit and nothing else to keep in step. Then add the same
//! `<phase> <task> <name>` line to `tests/surfaces.txt`, or the completeness test
//! goes red.

use crate::registry::{McpPolicy, Param, Since};
use crate::request::{
    AcceptHow, Actor, AskOption, Binding, BufferId, CaseChange, ChangeId, CheckpointId, Completion,
    Diagnostic, DiffMode, Direction, DiskExit, Edit, FileEdits, FileGroup, FoldState, GrantScope,
    Grouping, KeySeq, LanguageId, LanguageSpec, Motion, PaneKind, PaneRef, Position, PromptKind,
    RegionSpec, RegisterName, ScrollRequest, Seek, SelectionKind, Sequence, Severity, Signature,
    SourceId, Span, Target, TextObject, ThemeSlug, WatchId,
};
use crate::value::{Args, Call, Value, Wire, WireError};

// ---------------------------------------------------------------------------
// The spec row
// ---------------------------------------------------------------------------

/// One Action's row in the registry.
///
/// Emitted by the `actions!` macro beside the variant it describes, so the two cannot
/// disagree. [`Capability`](crate::registry::Capability) is the uniform view over
/// this and [`QuerySpec`](crate::query::QuerySpec) that `T020` and `T024` read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    /// The globally unique door name, kebab-case.
    pub name: &'static str,
    /// The domain enum it lives in.
    pub domain: &'static str,
    /// One line, in the product's voice.
    pub doc: &'static str,
    /// The phase and task that implement it.
    pub since: Since,
    /// The MCP door's default policy.
    pub mcp: McpPolicy,
    /// Its arguments, in declaration order.
    pub params: &'static [Param],
}

// ---------------------------------------------------------------------------
// The macro
// ---------------------------------------------------------------------------

/// Declares the Action vocabulary: variants, rustdoc, registry rows, decoder and
/// encoder, from one table.
///
/// One row per capability:
///
/// ```text
/// MarkSeen = "mark-seen" [S5 / "T041" / Allow]
///     "clears the unseen marker on a region, a hunk, a file or a whole block" {
///     target: Target = "what to mark",
/// }
/// ```
///
/// Every variant is brace-form, including the empty ones (`CloseFloat {}`).
/// Uniformity beats terseness here: one generated match pattern covers all of
/// them and there is no special case to get wrong.
macro_rules! actions {
    (
        $(
            $(#[$dmeta:meta])*
            $domain:ident($domain_ty:ident) = $domain_name:literal {
                $(
                    $variant:ident = $name:literal
                        [$phase:ident / $task:literal / $policy:ident]
                        $doc:literal
                    {
                        $( $field:ident : $fty:ty = $fdoc:literal ),* $(,)?
                    }
                )*
            }
        )*
    ) => {
        $(
            $(#[$dmeta])*
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub enum $domain_ty {
                $(
                    #[doc = $doc]
                    #[doc = ""]
                    #[doc = concat!("Door name: `", $name, "` · lands at `", $task, "`.")]
                    $variant {
                        $(
                            #[doc = $fdoc]
                            $field: $fty,
                        )*
                    },
                )*
            }
        )*

        /// The single mutation API.
        ///
        /// See the module docs for the shape and the six properties behind it.
        /// Nested by domain; the door name is flat and lives in
        /// [`ActionSpec::name`].
        #[derive(Debug, Clone, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Action {
            $(
                $(#[$dmeta])*
                $domain($domain_ty),
            )*
        }

        /// Every Action's registry row, in declaration order.
        ///
        /// Enumerated by [`crate::registry::capabilities`], which is what
        /// `T024`'s door-parity test walks. A hand-written list rots; this one
        /// cannot, because the same macro row emits the variant.
        pub const ACTIONS: &[ActionSpec] = &[
            $($(
                ActionSpec {
                    name: $name,
                    domain: $domain_name,
                    doc: $doc,
                    since: Since {
                        phase: $crate::registry::Phase::$phase,
                        task: $task,
                    },
                    mcp: McpPolicy::$policy,
                    params: &[
                        $(Param {
                            name: stringify!($field),
                            doc: $fdoc,
                            ty: <$fty as Wire>::TYPE,
                            required: <$fty as Wire>::REQUIRED,
                        },)*
                    ],
                },
            )*)*
        ];

        impl Action {
            /// This Action's door name.
            #[must_use]
            pub const fn name(&self) -> &'static str {
                match self {
                    $($(
                        Self::$domain($domain_ty::$variant { .. }) => $name,
                    )*)*
                }
            }

            /// The domain this Action belongs to.
            #[must_use]
            pub const fn domain(&self) -> &'static str {
                match self {
                    $(Self::$domain(_) => $domain_name,)*
                }
            }

            /// Encodes into a door-neutral call.
            ///
            /// Every declared argument is present, `None` encoding as
            /// [`Value::Null`], so a round trip through [`Action::from_call`] is
            /// exact and `T024` can compare two calls directly.
            #[must_use]
            pub fn to_call(&self) -> Call {
                match self {
                    $($(
                        Self::$domain($domain_ty::$variant { $($field,)* }) => {
                            #[allow(unused_mut, reason = "a zero-argument capability sets nothing")]
                            let mut args = Args::new();
                            $(args.set(stringify!($field), Wire::to_value($field));)*
                            Call { name: $name.to_owned(), args }
                        }
                    )*)*
                }
            }

            /// Decodes a call from any door.
            ///
            /// # Errors
            ///
            /// [`ActionError::Unknown`] if no capability has that name — which is
            /// also what a typo at the REPL or a stale agent gets, so the message
            /// matters. [`ActionError::Argument`] if an argument is missing or the
            /// wrong shape, naming the argument.
            pub fn from_call(name: &str, args: &Args) -> Result<Self, ActionError> {
                let action = match name {
                    $($(
                        $name => Self::$domain($domain_ty::$variant {
                            $($field: args.field(stringify!($field)).map_err(|source| {
                                ActionError::Argument { name: $name, source }
                            })?,)*
                        }),
                    )*)*
                    _ => return Err(ActionError::Unknown { name: name.to_owned() }),
                };
                Ok(action)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// The vocabulary
// ---------------------------------------------------------------------------

actions! {
    /// Text mutation. Every edit anywhere in the build lowers to one of these —
    /// an LSP code action, a hunk revert, a paste, a snippet.
    Buffer(BufferAction) = "buffer" {
        Insert = "insert" [S3 / "T026" / Allow]
            "inserts text at a position" {
            at: Position = "where to insert",
            text: String = "what to insert",
        }
        Delete = "delete" [S3 / "T026" / Allow]
            "removes a span" {
            span: Span = "the span to remove",
        }
        Replace = "replace" [S3 / "T026" / Allow]
            "replaces a span with text, in one edit" {
            span: Span = "the span to replace",
            text: String = "what replaces it",
        }
        ApplyEdits = "apply-edits" [S6 / "T052" / Allow]
            "applies a batch of edits as one undo group; the primitive T029's log replays" {
            edits: Vec<Edit> = "the edits, in order",
        }
        Yank = "yank" [S3 / "T026" / Allow]
            "copies a target into a register — \"ay ib" {
            target: Target = "what to copy",
            register: Option<RegisterName> = "which register; absent means the unnamed one",
        }
        Paste = "paste" [S3 / "T026" / Allow]
            "puts a register's contents at a target" {
            at: Target = "where to put it",
            register: Option<RegisterName> = "which register; absent means the unnamed one",
            before: bool = "put it before the target rather than after",
        }
        SetRegister = "set-register" [S3 / "T026" / Allow]
            "sets a named register's contents outright" {
            register: RegisterName = "which register",
            text: String = "the contents",
        }
        Indent = "indent" [S3 / "T026" / Allow]
            "shifts a target by whole indent levels; negative dedents" {
            target: Target = "what to shift",
            delta: i64 = "levels, negative to dedent",
        }
        // `T104`, and it is a second verb rather than an argument on `indent`
        // because the two answer different questions. `indent` shifts *lines*
        // by a level and is what `>>` and `>ap` mean; this types whitespace at
        // the **cursor**, which is what `<tab>` means in the middle of a line.
        // Binding `<tab>` to `indent` would have shifted the whole line from
        // wherever the caret happened to be, which is vim's `<C-t>` and not
        // vim's `<Tab>`.
        //
        // No arguments, and that is the design rather than an omission: how
        // wide one level is comes from `set-option!` and `define-language!`
        // (`Editing::indent_style`), so a keymap naming a width here would be
        // four spaces frozen into `runtime/keymaps.scm` for every language —
        // the Rust-table-in-scheme shape `T033` exists to forbid, and exactly
        // what `OPEN-QUESTIONS.md` §38 says a literal fall-through cannot do.
        InsertIndent = "insert-indent" [S4 / "T104" / Allow]
            "types one indent level at the cursor, advancing to the next tabstop" {
        }
        JoinLines = "join-lines" [S3 / "T026" / Allow]
            "joins the lines of a target onto one" {
            target: Target = "what to join",
        }
        SetCase = "set-case" [S3 / "T026" / Allow]
            "upper-cases, lower-cases or toggles the letters of a target — gU, gu, ~" {
            target: Target = "whose letters",
            case: CaseChange = "upper, lower or toggle",
        }
        ToggleComment = "toggle-comment" [S4 / "T037" / Allow]
            "comments or uncomments a target, using the language's own prefix" {
            target: Target = "what to toggle",
        }
        AlignColumns = "align-columns" [S4 / "T082" / Allow]
            "re-aligns a delimited file's columns — CSV's bespoke surface, no grammar involved" {
            target: Target = "what to align",
        }
    }

    /// The cursor, the selection, and jumps. Motion is not input: a keymap emits
    /// these, and so does a picker accept, a diagnostic jump and an OSC 8 link.
    Motion(MotionAction) = "motion" {
        MoveCursor = "move-cursor" [S3 / "T026" / Deny]
            "moves the cursor by a motion, count times" {
            motion: Motion = "which motion",
            count: u32 = "how many times; 1 is the plain form",
        }
        SetCursor = "set-cursor" [S3 / "T026" / Deny]
            "puts the cursor at an absolute position — what a click and a jump both lower to" {
            position: Position = "where",
            buffer: Option<BufferId> = "which buffer; absent means the focused one",
        }
        SelectRange = "select-range" [S3 / "T026" / Deny]
            "selects a span characterwise, linewise or blockwise" {
            span: Span = "the span",
            kind: SelectionKind = "characterwise, linewise or blockwise",
        }
        SelectObject = "select-object" [S3 / "T028" / Deny]
            "selects a text object — including the agent nouns u, h, t and b" {
            object: TextObject = "which object",
            inner: bool = "inner (i) rather than around (a)",
            count: u32 = "how many, for nesting",
            delimiter: Option<char> = "the delimiter, for a delimited object",
        }
        ExtendSelection = "extend-selection" [S3 / "T026" / Deny]
            "extends the live selection by a motion" {
            motion: Motion = "which motion",
            count: u32 = "how many times",
        }
        ClearSelection = "clear-selection" [S3 / "T026" / Deny]
            "drops the selection" {
        }
        GotoSequence = "goto-sequence" [S5 / "T049" / Allow]
            "walks a sequence — ]u unseen regions, ]] block files, ]! the ask queue" {
            sequence: Sequence = "which sequence",
            seek: Seek = "next, prev, first or last",
            filter: Option<crate::request::RegionFilter> = "narrow it, e.g. to claude's regions",
        }
        GotoLocation = "goto-location" [S6 / "T056" / Allow]
            "opens a file at a position — a picker accept, a transcript tool row, an OSC 8 link" {
            path: std::path::PathBuf = "workspace-relative path",
            position: Option<Position> = "where in it; absent means where you last were",
            pane: PaneRef = "which pane it lands in",
        }
        GotoAnchor = "goto-anchor" [S5 / "T042" / Allow]
            "jumps to an anchor, which survives the rewrite that moved it" {
            anchor: Option<crate::request::AnchorId> = "which anchor, by id",
            label: Option<String> = "or by the label place-anchor wrote — m's a-z",
            exact: bool = "backtick lands on the column, quote lands on the line",
        }
        Jump = "jump" [S5 / "T042" / Deny]
            "walks the jumplist" {
            seek: Seek = "next (forward) or prev (back)",
        }
    }

    /// The viewport, folds and wrap. Invariant 3 lives here: nothing in this
    /// domain happens as a side effect of drawing.
    View(ViewAction) = "view" {
        Scroll = "scroll" [S3 / "T026" / Deny]
            "moves a viewport, and is the only thing that may — invariant 3's single writer" {
            request: ScrollRequest = "what to move, and how far",
            pane: PaneRef = "which pane",
        }
        SetSoftWrap = "set-soft-wrap" [S3 / "T081" / Allow]
            "turns soft wrap on or off; off is the default and ↪ marks continuations" {
            target: Target = "which buffer",
            on: bool = "on or off",
        }
        SetFold = "set-fold" [S3 / "T016" / Allow]
            "folds or unfolds at a target — za, a diff hunk, a transcript turn" {
            target: Target = "what to fold",
            state: FoldState = "folded, unfolded or toggle",
        }
        FoldAll = "fold-all" [S3 / "T016" / Allow]
            "folds every foldable node to a depth" {
            level: u32 = "depth to fold to",
        }
        UnfoldAll = "unfold-all" [S3 / "T016" / Allow]
            "unfolds everything in the focused buffer" {
        }
        SetVirtualTextVisible = "set-virtual-text-visible" [S3 / "T032" / Allow]
            "collapses or expands a virtual-text rail — a thread's ┊ rows, a watch's values" {
            owner: Target = "whose rows",
            on: bool = "shown or collapsed",
        }
    }

    /// The input machine's own state, which the statusline reads. Every one of
    /// these is `Deny` on MCP: they are the user's keyboard, not an editor
    /// capability, and `tests/vocabulary.rs` holds the whole domain to that.
    /// (`show-unknown-key-hint` is therefore in `app` — it shows a hint, it does
    /// not touch the machine.)
    Input(InputAction) = "input" {
        SetMode = "set-mode" [S3 / "T026" / Deny]
            "sets the edit mode; PAUSED is a session state and is not one of these" {
            mode: crate::request::EditMode = "which mode",
        }
        SetCount = "set-count" [S3 / "T026" / Deny]
            "sets the pending numeric count — the 3 in 3dd" {
            count: u32 = "the count",
        }
        SelectRegister = "select-register" [S3 / "T026" / Deny]
            "sets the pending register — the \"a in \"ayy" {
            register: RegisterName = "which register",
        }
        CancelPending = "cancel-pending" [S3 / "T026" / Deny]
            "drops the pending count, register and operator — esc out of SPC pending" {
        }
        FeedKeys = "feed-keys" [S3 / "T026" / Deny]
            "feeds a key sequence to the input machine, exactly as if typed" {
            keys: KeySeq = "vim notation, e.g. `<C-q>` or `SPC f`",
        }
        RepeatLast = "repeat-last" [S3 / "T026" / Deny]
            "repeats the last change — ." {
            count: u32 = "how many times",
        }
        SetMacroRecording = "set-macro-recording" [S3 / "T099" / Deny]
            "records the keys you type into a register, or stops — q's other half, replayed by feed-keys" {
            register: RegisterName = "which register the keys land in",
            on: bool = "start recording into it, or stop and keep what it holds",
        }
    }

    /// Undo. The model is `phosphor-buffer`'s (Q2) and persistence is
    /// `phosphor-core`'s; these are the verbs over both.
    History(HistoryAction) = "history" {
        Undo = "undo" [S3 / "T029" / Allow]
            "steps back through the buffer's undo tree" {
            count: u32 = "how many steps",
        }
        Redo = "redo" [S3 / "T029" / Allow]
            "steps forward again" {
            count: u32 = "how many steps",
        }
        UndoToCheckpoint = "undo-to-checkpoint" [S3 / "T029" / Allow]
            "returns the buffer to a named point in its undo tree" {
            checkpoint: CheckpointId = "which point",
        }
        CommitUndoGroup = "commit-undo-group" [S3 / "T029" / Allow]
            "closes the current undo group explicitly, so the next edit starts a new one" {
        }
        CompactHistory = "compact-history" [S3 / "T030" / Allow]
            "compacts persisted history for a target — the periodic sweep Q1 and Q2 share" {
            target: Target = "whose history",
        }
    }

    /// Files, and the disk underneath them. Invariant 3 is at its sharpest here:
    /// a changed file is *indicated*, never injected.
    File(FileAction) = "file" {
        OpenFile = "open-file" [S3 / "T033" / Allow]
            "opens a file in a pane" {
            path: std::path::PathBuf = "workspace-relative path",
            at: Option<Position> = "where to put the cursor",
            pane: PaneRef = "which pane it lands in",
        }
        OpenAlternate = "open-alternate" [S3 / "T033" / Allow]
            "opens the file you were in before this one — vim's `CTRL-^`" {
            pane: PaneRef = "which pane it lands in",
        }
        SaveBuffer = "save-buffer" [S3 / "T033" / Allow]
            "writes a buffer to disk, optionally under a new name" {
            target: Target = "which buffer",
            path: Option<std::path::PathBuf> = "write here instead",
        }
        SaveAll = "save-all" [S3 / "T033" / Allow]
            "writes every dirty buffer" {
        }
        CloseBuffer = "close-buffer" [S3 / "T033" / Allow]
            "closes a buffer; refuses a dirty one unless forced" {
            target: Target = "which buffer",
            force: bool = "close even if it is dirty",
        }
        ReloadFromDisk = "reload-from-disk" [S7 / "T069" / Ask]
            "re-reads a buffer from disk — invariant 3's explicit consent, never automatic" {
            target: Target = "which buffer",
        }
        NoteDiskChange = "note-disk-change" [S7 / "T069" / Allow]
            "records that a file changed underneath us; sets ✱ and moves nothing else" {
            path: std::path::PathBuf = "which file",
            changed_by: Actor = "who is claimed to have changed it",
        }
        OpenDiskDiff = "open-disk-diff" [S7 / "T070" / Allow]
            "shows your unsaved buffer against what is on disk — :diff-disk" {
            target: Target = "which buffer",
        }
        ResolveDiskDiff = "resolve-disk-diff" [S7 / "T070" / Ask]
            "takes one of the three manual exits from a disk conflict; there is no auto-merge" {
            target: Target = "which buffer",
            exit: DiskExit = "take disk, keep mine, or ask claude",
        }
        SetFileWatch = "set-file-watch" [S7 / "T069" / Allow]
            "starts or stops watching a path for changes" {
            path: std::path::PathBuf = "which path",
            on: bool = "watching or not",
        }
    }

    /// Floats — the one chrome primitive. Design Language §9: at most one has
    /// focus, opening a second replaces the first, esc closes top-down, and
    /// needs-you never steals focus (Q9).
    ///
    /// # The registry `open-float` names
    ///
    /// [`OpenFloat`](FloatAction::OpenFloat) takes a
    /// [`SurfaceId`](crate::request::SurfaceId) — *"a registry key, not a Rust
    /// enum"* — and until `T093` nothing created an entry in that registry and
    /// no verb could. `OPEN-QUESTIONS.md` §43 is where that was found and
    /// ruled: [`DefineFloatSurface`](FloatAction::DefineFloatSurface) is the
    /// missing half, and it is deliberately the same shape as
    /// `define-picker-source` — **an id and a `String` of scheme**, because no
    /// `SteelVal` may ride in a payload and source text is how a body crosses
    /// the barrier.
    ///
    /// That symmetry is the point rather than a convenience. `T048` requires
    /// `:arch` to be *"built entirely from the `spans` hatch"* and to add zero
    /// lines to `phosphor-ui`; a surface the editor layer registers and the
    /// host merely calls is what makes that possible, and a `FloatKind` enum
    /// would have made every new surface a Rust edit.
    Float(FloatAction) = "float" {
        OpenFloat = "open-float" [S2 / "T021" / Allow]
            "opens a float by surface id, with that surface's own arguments" {
            surface: crate::request::SurfaceId = "which surface — a registry key, not a Rust variant",
            args: Args = "the surface's own arguments",
        }
        CloseFloat = "close-float" [S2 / "T021" / Allow]
            "closes the focused float" {
        }
        CloseAllFloats = "close-all-floats" [S2 / "T021" / Allow]
            "closes every open float" {
        }
        DefineFloatSurface = "define-float-surface" [S2 / "T093" / Allow]
            "defines or redefines a float surface; open-float names one by id" {
            surface: crate::request::SurfaceId = "the surface id open-float will name",
            body: String = "scheme source producing the float — a procedure of one argument",
        }
        FloatSelect = "float-select" [S5 / "T045" / Deny]
            "moves the highlighted row of the focused float" {
            delta: i64 = "rows to move; negative goes up",
        }
        FloatSelectRow = "float-select-row" [S5 / "T045" / Deny]
            "highlights an absolute row of the focused float" {
            row: u32 = "1-based row",
        }
        FloatAccept = "float-accept" [S5 / "T045" / Deny]
            "runs the focused float's primary verb — ↵" {
        }
        FloatAnswer = "float-answer" [S6 / "T059" / Deny]
            "answers the focused ask by digit — 4a's amber option digits" {
            digit: u32 = "which option, 1-based",
        }
        FloatToggleFold = "float-toggle-fold" [S7 / "T065" / Deny]
            "folds or unfolds a row inside a float body — za in a diff or a group list" {
            row: u32 = "1-based row",
        }
    }

    /// Panes: splits, focus and routing. `spine`'s, in the binary's loop —
    /// panes are not a widget (`TEAM.md`).
    Pane(PaneAction) = "pane" {
        SplitPane = "split-pane" [S6 / "T088" / Allow]
            "splits a pane and puts something in the new half" {
            pane: PaneRef = "which pane to split",
            direction: Direction = "which way",
            kind: PaneKind = "what the new pane holds",
        }
        FocusPane = "focus-pane" [S6 / "T088" / Deny]
            "moves focus to a pane; survives float churn (Design Language §9)" {
            pane: PaneRef = "which pane",
        }
        ClosePane = "close-pane" [S6 / "T088" / Allow]
            "closes a pane" {
            pane: PaneRef = "which pane",
        }
        ResizePane = "resize-pane" [S6 / "T088" / Allow]
            "resizes a pane by whole cells" {
            pane: PaneRef = "which pane",
            delta: i64 = "cells, negative to shrink",
        }
        SetPaneContent = "set-pane-content" [S6 / "T054" / Allow]
            "changes what a pane holds — :transcript is this, not a separate capability" {
            pane: PaneRef = "which pane",
            kind: PaneKind = "what it holds",
        }
        CreatePaneFromView = "create-pane-from-view" [V15 / "v1.5" / Deny]
            "fills a pane from a view tree claude emitted — v1.5's agent-built pane (Q12)" {
            pane: PaneRef = "which pane",
            tree: Value = "the view tree, as plain data",
        }
    }

    /// The picker — one widget, sources defined in Steel (`T046`).
    Picker(PickerAction) = "picker" {
        OpenPicker = "open-picker" [S5 / "T046" / Allow]
            "opens the picker over a source" {
            source: SourceId = "which source",
            query: Option<String> = "seed the filter with this",
        }
        SetPickerQuery = "set-picker-query" [S5 / "T045" / Deny]
            "replaces the picker's filter text" {
            text: String = "the filter",
        }
        CyclePickerSource = "cycle-picker-source" [S5 / "T047" / Deny]
            "cycles the picker's source — tab, 8a's grep/files/symbols" {
            delta: i64 = "how many, negative goes back",
        }
        PickerAccept = "picker-accept" [S5 / "T047" / Deny]
            "accepts the highlighted row — ↵ open, or every row into the quickfix list" {
            how: AcceptHow = "open, split, or quickfix",
        }
        TogglePickerPreview = "toggle-picker-preview" [S5 / "T045" / Deny]
            "shows or hides the preview pane; it drops below 100 columns regardless" {
        }
        DefinePickerSource = "define-picker-source" [S5 / "T046" / Allow]
            "defines or redefines a picker source; an open picker re-derives from it" {
            source: SourceId = "the source id",
            body: String = "scheme source producing the rows",
        }
        InvalidatePickerSource = "invalidate-picker-source" [S5 / "T046" / Allow]
            "drops a source's cached rows, so the next open recomputes them" {
            source: SourceId = "which source",
        }
    }

    /// The `:` line — ex commands and messages to claude, one widget.
    Prompt(PromptAction) = "prompt" {
        OpenPrompt = "open-prompt" [S6 / "T058" / Allow]
            "opens the prompt line; a live visual selection rides along as an anchor (1c)" {
            kind: PromptKind = "ex, claude, or search",
            seed: Option<String> = "prefill",
            anchor: Option<Target> = "what the message is about",
        }
        SetPromptText = "set-prompt-text" [S6 / "T058" / Deny]
            "replaces the prompt's text" {
            text: String = "the text",
        }
        SubmitPrompt = "submit-prompt" [S6 / "T058" / Deny]
            "submits the prompt" {
        }
        CancelPrompt = "cancel-prompt" [S6 / "T058" / Deny]
            "closes the prompt without submitting" {
        }
        PromptHistory = "prompt-history" [S6 / "T058" / Deny]
            "walks prompt history; prompts to claude are ex history too (6d's q:)" {
            delta: i64 = "how far back, negative goes forward",
        }
    }

    /// Regions and seen-state — Design Language §7, the one state machine.
    /// Seen-state is the only mutable flag the user owns.
    Region(RegionAction) = "region" {
        MarkSeen = "mark-seen" [S5 / "T041" / Allow]
            "marks a target seen — s, and S over a whole group, file or block" {
            target: Target = "what to mark; a hunk and an inbox item are targets too",
        }
        MarkUnseen = "mark-unseen" [S5 / "T041" / Allow]
            "marks a target unseen again — what claude revising a region does" {
            target: Target = "what to mark",
        }
        DeclareRegions = "declare-regions" [S5 / "T041" / Allow]
            "records claude-authored spans as regions; your own edits never create one" {
            regions: Vec<RegionSpec> = "the spans, each with its claimed author",
        }
        PlaceAnchor = "place-anchor" [S5 / "T042" / Allow]
            "anchors a target and answers the id — m writes one, goto-anchor reads it back" {
            at: Target = "what to anchor to",
            label: Option<String> = "a name to find it by — m's a-z, or a caller's own",
        }
        Reanchor = "reanchor" [S5 / "T042" / Allow]
            "re-resolves a file's anchors after a rewrite — node tier, then line+content" {
            path: std::path::PathBuf = "which file",
        }
        DropRegions = "drop-regions" [S5 / "T041" / Allow]
            "forgets the regions under a target — a deleted file, a stale span" {
            target: Target = "which regions",
        }
    }

    /// Anchored exchange (`3a`). No lifecycle beyond resolve and delete: the
    /// brief permits no review ceremony.
    Thread(ThreadAction) = "thread" {
        StartThread = "start-thread" [S7 / "T068" / Allow]
            "starts a thread anchored to a target — your comment in the margin" {
            anchor: Target = "what it is about",
            body: String = "what you said",
        }
        ReplyToThread = "reply-to-thread" [S7 / "T068" / Allow]
            "adds a reply; claude's side arrives the same way yours does" {
            thread: crate::request::ThreadId = "which thread",
            body: String = "the reply",
        }
        ResolveThread = "resolve-thread" [S7 / "T068" / Allow]
            "marks a thread done without deleting it" {
            thread: crate::request::ThreadId = "which thread",
        }
        DeleteThread = "delete-thread" [S7 / "T068" / Allow]
            "deletes a thread outright" {
            thread: crate::request::ThreadId = "which thread",
        }
        BroadcastThread = "broadcast-thread" [S7 / "T068" / Allow]
            "posts one message against every match of a pattern — :g/TODO/c, many anchors" {
            pattern: String = "the pattern to match",
            body: String = "the message",
        }
    }

    /// Review blocks, diffs and hunks. Claude declares a block; you read it.
    /// There is no approve and no reject (invariant 5).
    Review(ReviewAction) = "review" {
        DeclareReviewBlock = "declare-review-block" [S6 / "T053" / Allow]
            "declares a review block — files, spans and claude's own annotations" {
            title: String = "what this block is",
            files: Vec<FileGroup> = "the files and spans it covers",
            annotation: Option<String> = "claude's note about the block as a whole",
        }
        OpenReviewBlock = "open-review-block" [S7 / "T066" / Allow]
            "opens a review block" {
            block: crate::request::BlockId = "which block",
        }
        SetDiffMode = "set-diff-mode" [S7 / "T066" / Allow]
            "switches a diff between unified and side-by-side" {
            mode: DiffMode = "which mode",
        }
        ExpandDiffContext = "expand-diff-context" [S7 / "T066" / Allow]
            "expands a folded context run — 4b's ⋯ 13 lines" {
            hunk: crate::request::HunkId = "which hunk",
            lines: u32 = "how many more lines",
        }
        SetDiffGrouping = "set-diff-grouping" [S7 / "T065" / Allow]
            "groups a block's files by directory or flat; scale is grouping, not scrolling" {
            grouping: Grouping = "directory or flat",
        }
        AnnotateGroup = "annotate-group" [S7 / "T065" / Allow]
            "annotates a group — 8b's \"mechanical\" against \"the meat\"" {
            group: crate::request::GroupId = "which group",
            text: String = "the annotation",
        }
        RevertHunk = "revert-hunk" [S7 / "T064" / Ask]
            "reverts one hunk; it lowers to edits, so your undo tree has it" {
            hunk: crate::request::HunkId = "which hunk",
        }
        OpenHunkPeek = "open-hunk-peek" [S7 / "T066" / Allow]
            "opens 2b's anchored peek at a hunk, without leaving the buffer" {
            target: Target = "which hunk or region",
        }
    }

    /// The inbox — one list of everything claude said (`5c`). Unread *derives*
    /// from seen-state (`CP-8a`), so marking one read is
    /// `mark-seen` against an inbox-item target, not a capability of its own.
    Inbox(InboxAction) = "inbox" {
        OpenInbox = "open-inbox" [S7 / "T067" / Allow]
            "opens the inbox" {
        }
        OpenInboxItem = "open-inbox-item" [S7 / "T067" / Allow]
            "opens one item, jumping to whatever it is anchored to" {
            item: crate::request::InboxId = "which item",
        }
        Notify = "notify" [S7 / "T067" / Allow]
            "posts a note to the inbox; severity is one flag" {
            severity: Severity = "info, attention or trouble",
            title: String = "one line",
            body: Option<String> = "the rest",
            anchor: Option<Target> = "what it is about",
        }
    }

    /// Questions and permission asks. **Queued, never barged in** (Q9): an ask
    /// sets the statusline `!` and waits for no float to hold focus.
    Ask(AskAction) = "ask" {
        EnqueueAsk = "enqueue-ask" [S6 / "T060" / Allow]
            "queues a question for you; it never steals focus" {
            prose: String = "what claude is asking",
            options: Vec<AskOption> = "the numbered options, if any",
        }
        AnswerAsk = "answer-ask" [S6 / "T059" / Deny]
            "answers a queued ask, by digit or in prose" {
            ask: crate::request::AskId = "which ask",
            digit: Option<u32> = "which option, 1-based",
            prose: Option<String> = "a prose answer instead",
        }
        DeferAsk = "defer-ask" [S6 / "T060" / Deny]
            "puts an ask back in the queue — esc later" {
            // **Optional, and *"the focused one"* is the same idiom
            // `set-cursor`'s `buffer` already uses.** A door has to be able to
            // name an ask; a person has exactly one question in front of them
            // and no id on screen to read off. Requiring the number would make
            // `:defer` a command you cannot type.
            ask: Option<crate::request::AskId> = "which ask; absent means the one on screen",
        }
        RequestPermission = "request-permission" [S6 / "T061" / Allow]
            "asks to run something, showing the exact invocation (7a)" {
            invocation: String = "the exact command, as it will run",
            files: Vec<std::path::PathBuf> = "the files it touches, if it is a file operation",
        }
        GrantPermission = "grant-permission" [S6 / "T061" / Deny]
            "grants a permission ask; always also writes a legible rule to init.scm" {
            ask: crate::request::AskId = "which ask",
            scope: GrantScope = "once, or always",
        }
        DenyPermission = "deny-permission" [S6 / "T061" / Deny]
            "denies a permission ask" {
            ask: crate::request::AskId = "which ask",
        }
    }

    /// The agent session, over ACP. The last six are the stream itself: six
    /// explicit capabilities rather than one opaque ingest, because `V006` seeds
    /// a canned transcript through `--eval` with no test-only backdoor.
    Session(SessionAction) = "session" {
        StartSession = "start-session" [S6 / "T057" / Allow]
            "starts an agent session" {
            agent: String = "which agent binary or endpoint",
            cwd: Option<std::path::PathBuf> = "working directory; absent means the workspace root",
        }
        DiscoverSessions = "discover-sessions" [S6 / "T057" / Allow]
            "looks for sessions already running — 5d's adopt-without-restarting" {
        }
        AdoptSession = "adopt-session" [S6 / "T057" / Allow]
            "adopts a discovered session as-is" {
            handle: String = "the discovered session's handle",
        }
        AttachSession = "attach-session" [S6 / "T057" / Allow]
            "attaches to a session endpoint" {
            endpoint: String = "where it is",
        }
        ReattachSession = "reattach-session" [S6 / "T057" / Allow]
            "reattaches after a seam — :reattach" {
        }
        DetachSession = "detach-session" [S6 / "T057" / Allow]
            "detaches, leaving the session running" {
        }
        EndSession = "end-session" [S6 / "T057" / Ask]
            "ends the session" {
            force: bool = "end it even mid-turn",
        }
        SendMessage = "send-message" [S6 / "T058" / Allow]
            "sends a message to claude, with whatever it is anchored to — :claude" {
            body: String = "the message",
            anchors: Vec<Target> = "what it is about; a selection, a region, many of them",
        }
        InterruptSession = "interrupt-session" [S6 / "T062" / Allow]
            "pauses the turn at the next tool boundary — esc, and 7e's seam" {
        }
        SteerSession = "steer-session" [S6 / "T062" / Allow]
            "sends a correction and resumes — ↵ steer & resume" {
            body: String = "the correction",
        }
        ResumeSession = "resume-session" [S6 / "T062" / Allow]
            "resumes a paused turn — :resume" {
        }
        AbortTurn = "abort-turn" [S6 / "T062" / Allow]
            "abandons the current turn — :abort" {
        }
        TurnBegan = "turn-began" [S6 / "T050" / Allow]
            "records that a turn started" {
            turn: crate::request::TurnId = "which turn",
            prompt: Option<String> = "what started it",
        }
        TurnEnded = "turn-ended" [S6 / "T050" / Allow]
            "records that a turn finished" {
            turn: crate::request::TurnId = "which turn",
            summary: Option<String> = "how it ended",
        }
        SessionProse = "session-prose" [S6 / "T054" / Allow]
            "appends a chunk of claude's prose to the transcript" {
            turn: crate::request::TurnId = "which turn",
            chunk: String = "the text",
        }
        ToolCallStarted = "tool-call-started" [S6 / "T054" / Allow]
            "records a tool call starting — the transcript's verb + target row" {
            turn: crate::request::TurnId = "which turn",
            call: crate::request::ToolCallId = "the call's id",
            verb: String = "what it is doing — read, edit, run",
            target: Option<String> = "what it is doing it to",
            // **`path` is not `target` and the two must not be merged**
            // (`T056`). ACP carries a `title` — what the row *says* — and a
            // separate `locations` list of absolute paths, and an agent's title
            // is a sentence: `Replacing the reconnect loop's hand-rolled
            // sleep`. A jump link built from the title would point at a file
            // named after a sentence.
            path: Option<String> = "the absolute path this call touches, for the jump link",
            line: Option<u32> = "one-based line within that file, when the agent named one",
        }
        ToolCallProgress = "tool-call-progress" [S6 / "T054" / Allow]
            "adds progress to a running tool call" {
            call: crate::request::ToolCallId = "which call",
            note: String = "the progress line",
        }
        ToolCallCompleted = "tool-call-completed" [S6 / "T054" / Allow]
            "completes a tool call, with the counts the transcript row shows" {
            call: crate::request::ToolCallId = "which call",
            summary: String = "one line",
            added: u32 = "lines added",
            removed: u32 = "lines removed",
        }
        SessionSeam = "session-seam" [S6 / "T057" / Allow]
            "records a seam — paused, lost, or resumed (7b, 7e)" {
            kind: crate::request::SeamKind = "which seam",
            note: Option<String> = "what to say about it",
        }
    }

    /// Watches. First-class languages only — they need node anchoring, and the
    /// second tier says so honestly rather than degrading silently.
    Watch(WatchAction) = "watch" {
        PlaceWatch = "place-watch" [S8 / "T077" / Allow]
            "places a watch on an anchor — gw, and (watch-place …) from the REPL" {
            anchor: Target = "what to watch",
            expr: String = "the expression to evaluate",
        }
        RemoveWatch = "remove-watch" [S8 / "T074" / Allow]
            "removes a watch" {
            watch: WatchId = "which watch",
        }
        SetWatchVisible = "set-watch-visible" [S8 / "T076" / Allow]
            "shows or collapses a watch's value rows" {
            watch: WatchId = "which watch",
            on: bool = "shown or collapsed",
        }
        PushWatchValues = "push-watch-values" [S8 / "T075" / Allow]
            "delivers values from a real run; they arrive over ACP, not MCP (Q6)" {
            watch: WatchId = "which watch",
            values: Vec<String> = "the value sequence, in order",
            provenance: Option<String> = "where they came from — 5a's \"cargo test · … · 40s ago\"",
        }
        ClearWatchValues = "clear-watch-values" [S8 / "T075" / Allow]
            "clears a watch's values without removing it" {
            watch: WatchId = "which watch",
        }
    }

    /// The language server. Completion is the one passive float (`7c`) — it
    /// never takes focus, which is why its verbs are here and not in `float`.
    ///
    /// # Why there are four `ingest-` verbs and not one
    ///
    /// The transport is asynchronous by construction: `phosphor-buffer`'s
    /// `lsp::LanguageServers::look_up` answers on the runtime thread, and the
    /// event queue's `Posted` carries an [`Action`] plus the name of the
    /// subsystem that posted it — no payload of its own. So an
    /// answer needs a *verb* to arrive through, exactly as an unsolicited
    /// `publishDiagnostics` does — property 3 above, one enum for user intent
    /// and for external ingest. Modelling completion, signature help and hover
    /// as request-only is what a **synchronous** editor would do, and it left
    /// three surfaces that could be asked for and could not arrive.
    ///
    /// Each answers *exactly once per request, including the empty answer*,
    /// which is the contract `phosphor-buffer`'s `Answer` type keeps on every
    /// path (its `Drop` gives `Insight::Nothing`). That is why an empty list,
    /// an absent signature and empty prose are legal payloads rather than
    /// calls the client suppresses: they are how a float that is already open
    /// closes, and a suppressed empty answer leaves stale prose beside the
    /// cursor forever.
    ///
    /// # Why these three are `Deny` where `ingest-diagnostics` is `Allow`
    ///
    /// Not symmetry with the other ingest — symmetry with the **request each
    /// one answers**. Those three are `Deny`, and so are the three verbs that
    /// drive the completion float once it is up. A diagnostic set is a fact
    /// about a *file*, addressed by path, unsolicited by construction, and it
    /// lands in a gutter the user reads at their leisure; an agent pushing one
    /// is a linter reporting, which is a capability worth having. These three
    /// are addressed by the user's **cursor** and open a float against it, and
    /// the completion float is one keystroke from typing its contents into the
    /// buffer through `accept-completion`. An `Allow` here would be a hole
    /// around the `Deny` on `request-completion`: an agent that may not ask
    /// could still make the answer appear. As everywhere else, `Deny` is the
    /// *default* and not a wall — the rule that opens it is one the user wrote
    /// and can read (`7a`, `T061`) — and the host applies these in-process,
    /// never through the MCP door.
    Lsp(LspAction) = "lsp" {
        RequestCompletion = "request-completion" [S4 / "T038" / Deny]
            "asks for completions at the cursor" {
        }
        // **`otherwise` is §38's first option, taken.** That entry ruled
        // `<tab>` to `insert-indent` by its *third* option — give the key to
        // one task and the other a different key — and recorded what would
        // reverse it: *"`otherwise` widens from text to a capability to run
        // instead"*. Teej ruled it reversed at `CP-4`'s manual half, on Helix:
        // its completion menu binds `Tab`/`Down`/`C-n` to `move_down()` and
        // `move_down` from `None` lands on row 0, so **the first `<tab>`
        // selects the first row** and `<cr>` then accepts it. Both keys the
        // report asked about are one mechanism.
        //
        // **It is a [`Binding`] rather than a nested `Action`** because that
        // type already exists for exactly this — *"a binding is either a named
        // capability with its arguments, or scheme source text"* — and it is
        // already wire-safe across all three doors. A parallel nested-Action
        // type would be a second answer to a question `request.rs` answered
        // once, and would add a second [`ParamType::Any`] site to carry the
        // arguments that one already carries.
        //
        // [`ParamType::Any`]: crate::registry::ParamType::Any
        MoveCompletion = "move-completion" [S4 / "T038" / Deny]
            "moves the completion selection" {
            delta: i64 = "rows, negative goes up",
            otherwise: Option<Binding> = "the capability to run when no completion list is open; present is what makes a key step the list and still do its ordinary job when there is no list",
        }
        AcceptCompletion = "accept-completion" [S4 / "T038" / Deny]
            "accepts a completion" {
            index: u32 = "which item, 1-based; 0 is whichever row is selected, which is the only thing a keymap can name",
            then: Option<String> = "text to type after the accepted item — the space the `<space>` key leaves behind",
            otherwise: Option<String> = "text to type when no row has been chosen; present is what makes a key fall through instead of accepting",
        }
        CancelCompletion = "cancel-completion" [S4 / "T038" / Deny]
            "dismisses the completion float" {
        }
        RequestHover = "request-hover" [S4 / "T039" / Deny]
            "asks for hover at the cursor" {
        }
        RequestSignatureHelp = "request-signature-help" [S4 / "T039" / Deny]
            "asks for signature help at the cursor" {
        }
        RequestDefinition = "request-definition" [S4 / "T036" / Deny]
            "asks where the symbol at the cursor is defined" {
        }
        // **Re-homed from `S4`/`T036` by that phase's wiring pass, on the
        // `apply-edits` precedent.** `LanguageServers::ask` answers a
        // `Vec<FileSpan>` and **nothing in the vocabulary carries a list of
        // places**: `open-file` takes one path, and a references result is a
        // list by definition rather than by accident. `T047` is the task that
        // builds the surface a list of places is drawn in — *"grep / symbols
        // source … results carry who-touched-them"* — so the attribution was
        // the bug, exactly as it was for `jump` and `apply-edits` in the repair
        // window. `request-definition` stayed on `T036`, because a single
        // target is an `open-file` and that arm exists.
        RequestReferences = "request-references" [S5 / "T047" / Deny]
            "asks what references the symbol at the cursor" {
        }
        IngestCompletions = "ingest-completions" [S4 / "T038" / Deny]
            "delivers the answer to a completion request; an empty list closes the float" {
            items: Vec<Completion> = "the items, in the order the server ranked them; empty means it had nothing",
            at: Position = "the cursor the request was made at, so an answer the cursor has left is dropped rather than drawn in the wrong place",
            buffer: Option<BufferId> = "which buffer asked; absent means the focused one",
        }
        IngestSignatureHelp = "ingest-signature-help" [S4 / "T039" / Deny]
            "delivers the answer to a signature-help request; absent closes the float" {
            signature: Option<Signature> = "the active signature, absent when the server had none",
            at: Position = "the cursor the request was made at, so a late answer is dropped rather than drawn in the wrong place",
            buffer: Option<BufferId> = "which buffer asked; absent means the focused one",
        }
        IngestHover = "ingest-hover" [S4 / "T039" / Deny]
            "delivers the answer to a hover request; empty prose closes the float" {
            prose: Vec<String> = "the hover text, one row per line; empty means the server had nothing",
            at: Position = "the cursor the request was made at, so a late answer is dropped rather than drawn in the wrong place",
            buffer: Option<BufferId> = "which buffer asked; absent means the focused one",
        }
        IngestDiagnostics = "ingest-diagnostics" [S4 / "T040" / Allow]
            "records a server's diagnostics for a file; they reach the gutter at trouble priority" {
            path: std::path::PathBuf = "which file",
            diagnostics: Vec<Diagnostic> = "the diagnostics, replacing that file's set",
        }
        ApplyWorkspaceEdit = "apply-workspace-edit" [S4 / "T036" / Ask]
            "applies a server's edit across files — a rename, a code action, a format" {
            files: Vec<FileEdits> = "the edits, per file",
        }
        RestartLanguageServer = "restart-language-server" [S4 / "T036" / Allow]
            "restarts a language server" {
            language: LanguageId = "which language's server",
        }
    }

    /// Version control — an *enhancement*, never a dependency. Every one of
    /// these refuses with [`Refusal::NoRepository`] in a bare directory, and
    /// that is a normal state rather than an error path.
    Vcs(VcsAction) = "vcs" {
        RefreshVcs = "refresh-vcs" [S7 / "T071" / Allow]
            "re-reads VCS status — the statusline's jj ✓ segment" {
        }
        OpenTimeline = "open-timeline" [S7 / "T073" / Allow]
            "opens the timeline; agent turns are changes (3b)" {
        }
        ShowChangeDiff = "show-change-diff" [S7 / "T073" / Allow]
            "shows one change's diff — d diff" {
            change: ChangeId = "which change",
        }
        OpenOperationLog = "open-operation-log" [S7 / "T073" / Allow]
            "opens the full operation log — o full op log (mockups:901)" {
        }
        EditAtChange = "edit-at-change" [S7 / "T073" / Ask]
            "moves the working copy to a change — ↵ edit here" {
            change: ChangeId = "which change",
        }
        RestoreChange = "restore-change" [S7 / "T073" / Ask]
            "undoes a change through the VCS — u undo (jj), mockups:1046; undo is time travel" {
            change: ChangeId = "which change",
        }
    }

    /// The Steel layer talking about itself: evaluation, keymaps, languages,
    /// options, themes. Invariant 1 — this is the editor layer's own door.
    Runtime(RuntimeAction) = "runtime" {
        Eval = "eval" [S2 / "T023" / Deny]
            "evaluates scheme source; the CLI door and the REPL are both this" {
            source: String = "the source text",
        }
        LoadRuntimeFile = "load-runtime-file" [S2 / "T021" / Deny]
            "loads a scheme file; a broken one leaves a working editor and an error float" {
            path: std::path::PathBuf = "which file",
        }
        ReloadRuntime = "reload-runtime" [S2 / "T021" / Deny]
            "re-runs the boot sequence — init.scm is just the REPL session that runs at boot" {
        }
        OpenRepl = "open-repl" [S2 / "T022" / Allow]
            "opens the REPL (6b) — the primary extension workflow, not a debug tool" {
        }
        CloseRepl = "close-repl" [S2 / "T022" / Allow]
            "closes the REPL" {
        }
        ReplHistory = "repl-history" [S2 / "T022" / Deny]
            "walks REPL history" {
            delta: i64 = "how far back, negative goes forward",
        }
        ReplToBuffer = "repl-to-buffer" [S2 / "T022" / Allow]
            "moves the REPL session into a buffer — C-c buffer (mockups:510)" {
        }
        PersistForm = "persist-form" [S2 / "T022" / Ask]
            "appends a form to init.scm — 6b's \"· persisted to init.scm\", 7a's always-allow rule" {
            form: String = "the form to write",
        }
        SetKeybinding = "set-keybinding" [S3 / "T033" / Allow]
            "binds a key, live; the next frame has it and which-key knows" {
            keys: KeySeq = "vim notation, e.g. \"]r\"",
            binding: Binding = "a capability with arguments, or scheme source",
            mode: Option<crate::request::EditMode> = "which mode; absent means normal",
        }
        RemoveKeybinding = "remove-keybinding" [S3 / "T033" / Allow]
            "unbinds a key" {
            keys: KeySeq = "vim notation",
            mode: Option<crate::request::EditMode> = "which mode; absent means normal",
        }
        DefineLanguage = "define-language" [S4 / "T037" / Allow]
            "declares a language: grammar, server, extensions — the road up from second tier" {
            language: LanguageId = "the language's name",
            spec: LanguageSpec = "what it is made of",
        }
        SetOption = "set-option" [S2 / "T021" / Allow]
            "sets a declared option" {
            key: String = "the option's name",
            value: Value = "its value",
        }
        SetTheme = "set-theme" [S1 / "T012" / Allow]
            "switches theme by slug" {
            slug: ThemeSlug = "which theme",
        }
        ReloadTheme = "reload-theme" [S1 / "T011" / Allow]
            "re-reads the current theme file and re-validates its actor hues" {
        }
    }

    /// The editor as a program.
    App(AppAction) = "app" {
        Quit = "quit" [S1 / "T090" / Deny]
            "leaves, restoring the terminal; refuses on unsaved work unless forced" {
            force: bool = "leave even with unsaved buffers",
        }
        ShowUnknownKeyHint = "show-unknown-key-hint" [S3 / "T035" / Allow]
            "shows the unknown-key hint, once per session and never again (8e)" {
            key: KeySeq = "the key that was not bound",
        }
        OpenHelp = "open-help" [S3 / "T086" / Allow]
            "opens help — :help agent-objects (6d)" {
            topic: Option<String> = "which topic; absent opens the index",
        }
        OpenArch = "open-arch" [S5 / "T048" / Allow]
            "opens :arch — a store query rendered through the spans hatch, no Rust primitive" {
        }
        OpenDashboard = "open-dashboard" [S6 / "T057" / Allow]
            "opens the session dashboard (7d)" {
        }
        DismissDashboardHint = "dismiss-dashboard-hint" [S6 / "T057" / Allow]
            "dismisses 7d's one hint line; after that it is just an editor" {
        }
    }
}

// ---------------------------------------------------------------------------
// Aliases
// ---------------------------------------------------------------------------

/// A second door name for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alias {
    /// The alternative spelling.
    pub alias: &'static str,
    /// The canonical [`ActionSpec::name`] it resolves to.
    pub canonical: &'static str,
    /// Why it exists. Every alias is a documented exception, not a habit.
    pub reason: &'static str,
}

/// Every alias, and why it exists.
///
/// Kept deliberately short. An alias is a place two names mean one thing, which
/// is the drift the single registry exists to prevent — so each one here is a
/// *drawing* the build has to honour rather than a convenience.
pub const ALIASES: &[Alias] = &[Alias {
    alias: "watch-place",
    canonical: "place-watch",
    reason: "6b draws `(watch-place \"src/retry.rs:24\" 'delay)` — mutating, no bang, and \
             noun-first against the rest of the vocabulary (TUI Mockups.dc.html:502, and the \
             plan repeats the spelling at IMPLEMENTATION-PLAN.md's S8). Reconciling the drawing \
             with the naming rule edits a design doc, so it is flagged for Teej and aliased here \
             in the meantime.",
}];

// ---------------------------------------------------------------------------
// The envelope
// ---------------------------------------------------------------------------

/// An Action plus who asked and through which door.
///
/// The store never sees a bare [`Action`]. Provenance is not an audit feature
/// here — Design Language §7 makes it load-bearing (*"your own edits never
/// create regions: the machine tracks claude only"*), and the MCP door's refusal
/// of focus-relative targets needs [`Request::door`] to decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Who asked.
    pub actor: Actor,
    /// Which door they came through.
    pub door: crate::registry::Door,
    /// What they asked for.
    pub action: Action,
}

impl Request {
    /// A request from a door.
    #[must_use]
    pub const fn new(actor: Actor, door: crate::registry::Door, action: Action) -> Self {
        Self {
            actor,
            door,
            action,
        }
    }
}

/// Several requests that land as one undo group, one re-derive and one frame.
///
/// This is where sequencing lives, deliberately **not** as an `Action::Sequence`
/// variant: a sequence has no door name, no schema and no single task, and
/// making an Action a tree means every interpreter recurses. A keymap that fires
/// three Actions emits three requests in one batch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    /// The requests, in order.
    pub requests: Vec<Request>,
    /// What to call the resulting undo group, if it makes one.
    pub label: Option<String>,
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// What applying a [`Request`] produced.
///
/// Actions return a value rather than `()` because `6b` draws one:
/// `(keymap-set! …)` answers `#ok · persisted to init.scm` and `(watch-place …)`
/// answers `#watch-3 · streaming from next run` (TUI Mockups.dc.html:499-504).
/// An Action returning nothing cannot draw that screen, and `T023`'s *"`--eval`
/// and the REPL return identical results"* would have nothing to compare.
///
/// # Why there are three cases and not two (`T100`)
///
/// [`Done`](Self::Done) is *it happened* and [`Refused`](Self::Refused) is *it
/// did not, and that is a normal state*. Scheme source that began evaluating
/// and then blew up is **neither**, and until `T100` there was no case for it:
/// `phosphor-steel`'s `Runtime::evaluate` landed a raise in
/// [`Refusal::Declined`], which means *a rule, a hook or the user said no*. So
/// the wrong case carried Steel's own error text, envelope and all, into a line
/// [`Refusal::why`] promises is the product's voice.
///
/// A refused **query** showed it most plainly, because a query that cannot be
/// answered *raises* rather than answering a value — deliberately, and
/// `phosphor-steel`'s `registry.rs` says why. So a `QueryError` already phrased
/// in Design Language §6's voice became a `SteelErr` and came back wearing
/// Steel's envelope, measured against the built binary before the fix:
///
/// ```text
/// phosphor --eval '(unseen-regions "src/main.rs")'
/// #refused · Error: Generic: not built yet — T041 builds it
/// ```
///
/// [`Raised`](Self::Raised) is that third case. It is not a refusal — nothing
/// declined anything — and it is not an error type either: a raise is an
/// [`Outcome`], because the request was well formed and the evaluator did run.
/// The doors that render it own only the sigil; the sentence is
/// [`struct@Raised`]'s, for the same reason [`Refusal::why`] is the enum's.
///
/// `OPEN-QUESTIONS.md` §7, ruled into `T100` with §9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It happened.
    Done(Receipt),
    /// It did not, and here is why. **Not an error** — a refusal is a normal
    /// state (a bare directory has no VCS; an agent may not move your cursor).
    Refused(Refusal),
    /// It ran, and it raised.
    ///
    /// Only an evaluator produces this: everything else in the editor answers
    /// in Rust and either does the thing or declines it. A `Host` that returned
    /// one would be re-raising at the Steel door rather than handing scheme a
    /// value, which is `phosphor-steel`'s *"refusals are values; errors are
    /// errors"* rule read from this side.
    Raised(Raised),
}

/// What an evaluation that ran and then raised has to say for itself.
///
/// Two halves rather than one string, because the halves come from two places
/// and only one of them is ours. [`kind`](Self::kind) is a closed vocabulary —
/// a `&'static str` chosen by the crate that owns the evaluator, so a raise
/// cannot invent a category — and [`message`](Self::message) is whatever the
/// evaluator said, which belongs to it exactly as [`Refusal::Declined`]'s
/// reason belongs to the rule that wrote it.
///
/// The join lives in [`Raised::why`] beside [`Refusal::why`] so that Design
/// Language §6's *em dash for cause* is spelled once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    /// What kind of failure it was, in the product's voice: `"wrong type"`,
    /// `"unbound identifier"`.
    ///
    /// [`None`] when naming the kind would add nothing a reader can act on —
    /// the evaluator's generic envelope around a message that is already a
    /// finished sentence, which is how a refused query's *"not built yet —
    /// `T074` builds it"* reaches a caller unwrapped.
    pub kind: Option<&'static str>,
    /// What the evaluator said, with its own envelope stripped.
    pub message: String,
}

impl Raised {
    /// Why the evaluation produced no value, in the product's voice.
    ///
    /// The counterpart of [`Refusal::why`], and a method for the same reason:
    /// a second phrasing has to be a second `match` somebody writes on purpose.
    #[must_use]
    pub fn why(&self) -> String {
        match self.kind {
            // §6: *em dash for cause*.
            Some(kind) => format!("{kind} — {}", self.message),
            None => self.message.clone(),
        }
    }
}

/// What a completed Action has to say for itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The capability that ran.
    pub capability: &'static str,
    /// Its result, if it has one — a new id, a count, a list of rows.
    pub value: Value,
    /// One line for the REPL and the CLI: `"persisted to init.scm"`.
    pub note: Option<String>,
}

impl Receipt {
    /// A receipt with no value and no note.
    #[must_use]
    pub const fn ok(capability: &'static str) -> Self {
        Self {
            capability,
            value: Value::Null,
            note: None,
        }
    }
}

/// Why an Action did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Named in the vocabulary, not built yet. Carries the task that builds it,
    /// which is the whole reason the vocabulary names S3–S8 surfaces at `T019`:
    /// a caller gets *"`T074` builds this"*, not *"unknown action"*.
    ///
    /// The example here used to be `T041`, and it stopped being one the day the
    /// store landed. An id quoted in prose is a claim about the tree like any
    /// other.
    NotYetImplemented {
        /// The `docs/TASKS.md` id.
        task: &'static str,
    },
    /// An agent asked for a focus-relative target. An agent has no cursor;
    /// letting it act on "the selection" is how it edits whatever you happened
    /// to be looking at (see [`Target::focus_relative`](crate::request::Target::focus_relative)).
    FocusRelativeTargetOverMcp,
    /// The door's policy refuses this capability, and no rule in `init.scm`
    /// opens it.
    DoorDenied {
        /// Which door.
        door: crate::registry::Door,
    },
    /// A VCS capability in a directory with no repository. **A normal state**:
    /// `S7.3` says no feature may assume a repo exists.
    NoRepository,
    /// The target does not exist any more — a dropped region, a closed buffer, a
    /// stale id from an agent working off an old query.
    NoSuchTarget,
    /// It would lose unsaved work and was not forced.
    WouldLoseWork,
    /// The capability was refused by a rule, a hook, or the user.
    Declined {
        /// What to say about it.
        reason: String,
    },
}

impl Refusal {
    /// Why the Action did not happen, in the product's voice — **the only
    /// phrasing of this enum that exists.**
    ///
    /// Design Language §6: *lowercase, telegraphic, factual*; the midline dot
    /// only inside a fact, the em dash for cause — *"session lost —
    /// `:reattach`"*. Each line below is that shape: what is true, then what
    /// to do about it. No Rust or Steel type name reaches a reader here; the
    /// door name comes from [`Door::as_str`](crate::registry::Door::as_str),
    /// which spells `steel`/`mcp`/`cli` and not the variants.
    ///
    /// # Why it is a method and not a function somewhere
    ///
    /// It used to be two functions. `crates/phosphor/src/door.rs` said
    /// *"`T041` builds this"* where `phosphor-steel` said *"not built yet —
    /// `T041` builds it"*, and one enum in two voices is how a vocabulary
    /// stops being one vocabulary — recorded as `OPEN-QUESTIONS.md` §9 and
    /// ruled into `T100`. A convention that the doors agree is not enough,
    /// because the next door is written by whoever writes it. Hanging the
    /// phrasing on the type makes the agreement structural: a second voice is
    /// now a second `match` somebody has to write on purpose, over an enum
    /// that already answers the question.
    ///
    /// Every surface reads this one string — a refused Action's
    /// `(#refused "…")` value in scheme, the REPL's `⇒` line, the CLI door's
    /// stdout, the ex line's diagnostic and a float.
    #[must_use]
    pub fn why(&self) -> String {
        match self {
            Self::NotYetImplemented { task } => format!("not built yet — {task} builds it"),
            Self::FocusRelativeTargetOverMcp => {
                "an agent has no cursor — name the target".to_owned()
            }
            Self::DoorDenied { door } => {
                format!(
                    "the {} door refuses this — open it in init.scm",
                    door.as_str()
                )
            }
            Self::NoRepository => "no repository here".to_owned(),
            // Not *"it may have moved on"*: §6 asks for factual, and a hedge is
            // the one thing a caller cannot act on. These are the two ways a
            // target goes stale, per this variant's own doc.
            Self::NoSuchTarget => "no such target — it was dropped or closed".to_owned(),
            Self::WouldLoseWork => "unsaved work — force it or save first".to_owned(),
            // A rule, a hook or the user already phrased this one.
            Self::Declined { reason } => reason.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a call could not be turned into an [`Action`].
///
/// Distinct from [`Refusal`]: this is a *malformed* call, not a well-formed one
/// the editor declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// No capability has that name.
    Unknown {
        /// What was asked for.
        name: String,
    },
    /// An argument was missing or the wrong shape.
    Argument {
        /// The capability being called.
        name: &'static str,
        /// What went wrong.
        source: WireError,
    },
}

impl core::fmt::Display for ActionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown { name } => write!(f, "no such action `{name}`"),
            Self::Argument { name, source } => write!(f, "`{name}`: {source}"),
        }
    }
}

impl std::error::Error for ActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Argument { source, .. } => Some(source),
            Self::Unknown { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Behaviour that is not generated
// ---------------------------------------------------------------------------

impl Action {
    /// This Action's registry row.
    ///
    /// Total by construction — the `actions!` macro emits the variant and the row from
    /// one declaration — and `tests/vocabulary.rs` proves it for every row.
    ///
    /// # Panics
    ///
    /// Never in practice: only if a variant existed with no row, which the macro
    /// makes unrepresentable.
    #[must_use]
    pub fn spec(&self) -> &'static ActionSpec {
        let name = self.name();
        ACTIONS
            .iter()
            .find(|spec| spec.name == name)
            .expect("every Action variant is emitted with its registry row")
    }

    /// Whether this capability hands its caller the *user's keyboard* rather
    /// than an editor capability.
    ///
    /// The MCP door denies these by default. Not because an agent must never
    /// have them — `V006` drives the editor through `--eval` and needs exactly
    /// this — but because the rule that opens them should be one the user wrote
    /// and can read (`7a`'s always-allow, `T061`).
    #[must_use]
    pub const fn feeds_the_keyboard(&self) -> bool {
        matches!(
            self,
            Self::Input(_) | Self::Runtime(RuntimeAction::Eval { .. })
        )
    }

    /// Resolves a door name, following [`ALIASES`].
    #[must_use]
    pub fn canonical_name(name: &str) -> &str {
        ALIASES
            .iter()
            .find(|alias| alias.alias == name)
            .map_or(name, |alias| alias.canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Phase;
    use crate::request::{RegionId, Target};

    #[test]
    fn a_call_round_trips() {
        let action = Action::Region(RegionAction::MarkSeen {
            target: Target::Region { id: RegionId(3) },
        });
        let call = action.to_call();
        assert_eq!(call.name, "mark-seen");
        assert_eq!(Action::from_call(&call.name, &call.args).unwrap(), action);
    }

    #[test]
    fn an_unknown_name_says_so() {
        let error = Action::from_call("mark-read", &Args::new()).unwrap_err();
        assert_eq!(
            error,
            ActionError::Unknown {
                name: "mark-read".to_owned()
            }
        );
    }

    #[test]
    fn a_missing_argument_names_the_capability_and_the_argument() {
        let error = Action::from_call("mark-seen", &Args::new()).unwrap_err();
        let ActionError::Argument { name, source } = error else {
            panic!("expected an argument error");
        };
        assert_eq!(name, "mark-seen");
        assert_eq!(source, WireError::Missing { field: "target" });
    }

    #[test]
    fn every_variant_knows_its_own_row() {
        let action = Action::App(AppAction::Quit { force: false });
        assert_eq!(action.name(), "quit");
        assert_eq!(action.domain(), "app");
        assert_eq!(action.spec().since.phase, Phase::S1);
        assert_eq!(action.spec().since.task, "T090");
    }

    #[test]
    fn the_keyboard_is_denied_by_default() {
        let feed = Action::Input(InputAction::FeedKeys {
            keys: KeySeq("<C-q>".to_owned()),
        });
        assert!(feed.feeds_the_keyboard());
        assert_eq!(feed.spec().mcp, McpPolicy::Deny);

        let eval = Action::Runtime(RuntimeAction::Eval {
            source: "(quit!)".to_owned(),
        });
        assert!(eval.feeds_the_keyboard());
        assert_eq!(eval.spec().mcp, McpPolicy::Deny);
    }

    /// The pairing the `Lsp` domain's header argues for, as a check rather
    /// than a paragraph: an answer is exactly as open as the request it
    /// answers. An `Allow` on the answer would be a hole around the `Deny` on
    /// the request — an agent that may not ask could still make the float
    /// appear beside the user's cursor — and relaxing the request without
    /// relaxing its answer would register a capability whose reply nothing may
    /// send. Either drift fails here.
    #[test]
    fn an_lsp_answer_is_exactly_as_open_as_the_request_it_answers() {
        let policy = |name: &str| {
            ACTIONS
                .iter()
                .find(|spec| spec.name == name)
                .unwrap_or_else(|| panic!("`{name}` is registered"))
                .mcp
        };
        for (request, answer) in [
            ("request-completion", "ingest-completions"),
            ("request-signature-help", "ingest-signature-help"),
            ("request-hover", "ingest-hover"),
        ] {
            assert_eq!(
                policy(answer),
                policy(request),
                "`{answer}` answers `{request}`; their MCP defaults move together"
            );
        }
    }

    /// `phosphor-buffer`'s `Answer` promises **exactly one** call per lookup on
    /// every path, including the one its `Drop` takes when a server never
    /// replies. So each of these three verbs has to be able to carry *nothing*:
    /// an empty answer is how an open float closes, and a payload that could
    /// only express a result would leave stale prose beside the cursor for the
    /// rest of the session.
    #[test]
    fn the_empty_answer_is_a_call_each_of_the_three_can_make() {
        let at = Position {
            line: 12,
            column: 1,
        };
        let nothing = [
            Action::Lsp(LspAction::IngestCompletions {
                items: Vec::new(),
                at,
                buffer: None,
            }),
            Action::Lsp(LspAction::IngestSignatureHelp {
                signature: None,
                at,
                buffer: None,
            }),
            Action::Lsp(LspAction::IngestHover {
                prose: Vec::new(),
                at,
                buffer: None,
            }),
        ];
        for action in nothing {
            let call = action.to_call();
            assert_eq!(
                Action::from_call(&call.name, &call.args).unwrap(),
                action,
                "`{}` must round-trip its empty answer",
                call.name
            );
        }

        // And a door that says nothing by *omitting* the field, which is what a
        // CLI invocation with no `--signature` and a Steel call with the
        // keyword absent both look like. `Vec` has no such spelling — an empty
        // list is a list — so this is the signature's case alone.
        let omitted = Args::new().with("at", Wire::to_value(&at));
        assert_eq!(
            Action::from_call("ingest-signature-help", &omitted).unwrap(),
            Action::Lsp(LspAction::IngestSignatureHelp {
                signature: None,
                at,
                buffer: None,
            }),
        );
    }

    /// Documentation rides on the *item*, not on the list, because
    /// `move-completion` changes which prose is on screen without asking the
    /// server again. A per-call documentation field would either re-request on
    /// every arrow key or draw the first item's prose under all of them.
    #[test]
    fn each_completion_carries_its_own_documentation_through_the_wire() {
        use crate::request::CompletionKind;

        let action = Action::Lsp(LspAction::IngestCompletions {
            items: vec![
                Completion {
                    label: "default".to_owned(),
                    detail: Some("fn() -> RetryPolicy".to_owned()),
                    documentation: vec!["Returns the policy with 3 attempts.".to_owned()],
                    insert: "default()".to_owned(),
                    kind: Some(CompletionKind::Function),
                    source: Some("retry".to_owned()),
                    deprecated: false,
                },
                Completion {
                    label: "default_delay".to_owned(),
                    detail: Some("Duration".to_owned()),
                    documentation: vec!["The base delay between attempts.".to_owned()],
                    insert: "default_delay".to_owned(),
                    kind: Some(CompletionKind::Constant),
                    source: None,
                    deprecated: true,
                },
            ],
            at: Position {
                line: 24,
                column: 9,
            },
            buffer: Some(BufferId(3)),
        });
        let call = action.to_call();
        let Action::Lsp(LspAction::IngestCompletions { items, .. }) =
            Action::from_call(&call.name, &call.args).unwrap()
        else {
            panic!("`ingest-completions` decodes to its own variant");
        };
        assert_eq!(items.len(), 2);
        assert_ne!(
            items[0].documentation, items[1].documentation,
            "two items' prose must survive one call as two different things"
        );
        assert_eq!(items[0].label, "default");
        assert_eq!(items[1].insert, "default_delay");
    }

    #[test]
    fn aliases_resolve_and_are_documented() {
        assert_eq!(Action::canonical_name("watch-place"), "place-watch");
        assert_eq!(Action::canonical_name("place-watch"), "place-watch");
        for alias in ALIASES {
            assert!(
                ACTIONS.iter().any(|spec| spec.name == alias.canonical),
                "alias `{}` points at nothing",
                alias.alias
            );
            assert!(
                !alias.reason.is_empty(),
                "alias `{}` has no recorded reason",
                alias.alias
            );
        }
    }
}
