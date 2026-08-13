//! The query vocabulary — the read side, over ViewModels.
//!
//! **Where this belongs, and why here.** `vm` holds the ViewModel *types* the
//! widgets render; this module holds the *vocabulary* that asks for them. They
//! are deliberately different files with different owners: a ViewModel lands
//! with the surface that needs it (`store` and `agent` write those), while the
//! set of things a door may ask for is one writer's — `spine`'s — for exactly
//! the reason the `Action` enum is (`TEAM.md`'s first single-writer rule names
//! *"the `Action` enum, the query vocabulary, or the view tree"* in one breath).
//! Actions and queries also share one registry and one door-parity test
//! ([`crate::registry`]), which they could not do if the read side lived inside
//! `vm`.
//!
//! # What a query is
//!
//! **A pure, total, synchronous projection of one store snapshot into owned,
//! serialisable data.** Mockup `6b` fixes the shape before we get a vote:
//! `(unseen-regions "src/retry.rs")` returns records, `(region-author r)` is an
//! ordinary accessor over one of those records, and `(next-region-by claude)`
//! composes into a `goto` (TUI Mockups.dc.html:493-503). So queries return
//! *values*, accessors on the returned value are free and unregistered, and
//! composition happens in Steel. That is what keeps the registry from growing
//! one entry per field.
//!
//! # What a query may not do
//!
//! Each of these is a bug the design already predicts, not a style rule:
//!
//! * **Mutate.** Never `&mut Store`. Opening the inbox must not mark it read —
//!   that is an explicit `mark-seen` (`CP-8a` requires unread to *derive* from
//!   seen-state, so a query that consumed it would break the derivation).
//! * **Return a borrow.** No rope slices, no tree-sitter nodes, no `&Store`.
//!   Steel is garbage-collected and holds what it is given across mutations;
//!   owned snapshots make that stale rather than unsound.
//! * **Block.** No disk, no network, no LSP or ACP round trip, no off-thread
//!   matcher awaited inside. Anything needing I/O is an Action that schedules
//!   and an ingest Action that lands. A query runs inside the frame path when
//!   the frame cache misses (`T079`), and a query that can block can drop a
//!   frame.
//! * **Return unbounded results.** The list queries that can be large take
//!   `limit`/`offset`. A picker over 100k files streams through nucleo
//!   off-thread (`T045`); it does not come back through a query in one lump.
//! * **Be focus-implicit.** Focus-relative scope is an explicit
//!   [`Target`], never a hidden default — otherwise the
//!   same expression means different things in the REPL and over MCP, and
//!   invariant 2 is a slogan.
//! * **Error on a missing id.** An absent thing answers empty. A stale Steel
//!   composition must not be able to break a frame.
//! * **Be assumed fresh mid-frame.** Q12 caches one view tree per state change;
//!   there is exactly one snapshot per evaluation, and [`Revision`] is how a
//!   composition knows its snapshot has moved.
//!
//! Owned by `spine`.

use crate::registry::{McpPolicy, Param, ParamType, Since};
use crate::request::{
    Actor, AnchorId, AskId, BlockId, BufferId, EditMode, KeySeq, LanguageId, PaneRef, Position,
    RegionFilter, RegionId, RegisterName, SourceId, Target, ThreadId, TurnId, WatchId,
};
use crate::value::{Args, Call, Value, Wire, WireError};

// ---------------------------------------------------------------------------
// Freshness
// ---------------------------------------------------------------------------

/// A monotonic freshness token for one projection of the store.
///
/// **This is the mechanism behind `T079`'s frame cache**, and without it the
/// benchmark `CP-2` gates on — *VM invocations per second flat while FPS climbs*
/// — has no way to pass. A Steel composition records which queries it read and
/// at what revision; Rust redraws the cached view tree every frame and re-enters
/// the VM only when one of those revisions has moved.
///
/// Deliberately opaque and deliberately not a timestamp: it is a change counter,
/// so equality means "nothing you read has moved", which is the only question
/// the cache asks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of a store nothing has touched.
    pub const INITIAL: Self = Self(0);

    /// The next revision. Called by the store when a projection changes; never
    /// by a widget, and never per frame.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The raw counter, for logging and for the frame-cache benchmark.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A query result with the revision it was true at.
///
/// What a door hands back, and what a composition stores so `T079` can decide
/// whether to re-run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The data.
    pub value: Value,
    /// The revision it was read at.
    pub revision: Revision,
}

/// What answers queries.
///
/// Implemented by the store at `T041`; the three doors take a `&dyn Answers` and
/// none of them reaches into the store directly. Deliberately one method — the
/// vocabulary is in [`Query`], not in a trait that grows a method per capability
/// and drifts from the registry.
pub trait Answers {
    /// Answers one query against the current snapshot.
    ///
    /// # Errors
    ///
    /// [`QueryError::NotYetImplemented`] for a capability whose phase has not
    /// landed — which is most of them at `S2`, and is a legible answer rather
    /// than a missing binding.
    fn answer(&self, query: &Query) -> Result<Answer, QueryError>;
}

// ---------------------------------------------------------------------------
// The spec row
// ---------------------------------------------------------------------------

/// One query's row in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuerySpec {
    /// The globally unique door name, kebab-case, no bang.
    pub name: &'static str,
    /// The domain enum it lives in.
    pub domain: &'static str,
    /// One line, in the product's voice.
    pub doc: &'static str,
    /// The phase and task that implement it.
    pub since: Since,
    /// Its arguments, in declaration order.
    pub params: &'static [Param],
    /// The declared shape of the result.
    ///
    /// [`ParamType::Any`] where the concrete ViewModel lands with its surface —
    /// those records are `store`'s and `agent`'s to define, and inventing their
    /// fields here would be a second definition to drift from. The door schema
    /// says "object" until the surface lands and the shape is real.
    pub returns: ParamType,
}

// ---------------------------------------------------------------------------
// The macro
// ---------------------------------------------------------------------------

/// Declares the query vocabulary: variants, rustdoc, registry rows, decoder and
/// encoder, from one table. The read-side twin of
/// [`actions!`](crate::action::Action).
macro_rules! queries {
    (
        $(
            $(#[$dmeta:meta])*
            $domain:ident($domain_ty:ident) = $domain_name:literal {
                $(
                    $variant:ident = $name:literal
                        [$phase:ident / $task:literal]
                        -> $returns:expr,
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

        /// The read side of the one API.
        ///
        /// Same registry, same three doors, same parity test as
        /// [`Action`](crate::action::Action) — see the module docs for what a
        /// query may and may not do.
        #[derive(Debug, Clone, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Query {
            $(
                $(#[$dmeta])*
                $domain($domain_ty),
            )*
        }

        /// Every query's registry row, in declaration order.
        pub const QUERIES: &[QuerySpec] = &[
            $($(
                QuerySpec {
                    name: $name,
                    domain: $domain_name,
                    doc: $doc,
                    since: Since {
                        phase: $crate::registry::Phase::$phase,
                        task: $task,
                    },
                    params: &[
                        $(Param {
                            name: stringify!($field),
                            doc: $fdoc,
                            ty: <$fty as Wire>::TYPE,
                            required: <$fty as Wire>::REQUIRED,
                        },)*
                    ],
                    returns: $returns,
                },
            )*)*
        ];

        impl Query {
            /// This query's door name.
            #[must_use]
            pub const fn name(&self) -> &'static str {
                match self {
                    $($(
                        Self::$domain($domain_ty::$variant { .. }) => $name,
                    )*)*
                }
            }

            /// The domain this query belongs to.
            #[must_use]
            pub const fn domain(&self) -> &'static str {
                match self {
                    $(Self::$domain(_) => $domain_name,)*
                }
            }

            /// Encodes into a door-neutral call.
            #[must_use]
            pub fn to_call(&self) -> Call {
                match self {
                    $($(
                        Self::$domain($domain_ty::$variant { $($field,)* }) => {
                            #[allow(unused_mut, reason = "a zero-argument query sets nothing")]
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
            /// [`QueryError::Unknown`] if no query has that name;
            /// [`QueryError::Argument`] if an argument is missing or the wrong
            /// shape.
            pub fn from_call(name: &str, args: &Args) -> Result<Self, QueryError> {
                let query = match name {
                    $($(
                        $name => Self::$domain($domain_ty::$variant {
                            $($field: args.field(stringify!($field)).map_err(|source| {
                                QueryError::Argument { name: $name, source }
                            })?,)*
                        }),
                    )*)*
                    _ => return Err(QueryError::Unknown { name: name.to_owned() }),
                };
                Ok(query)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// The vocabulary
// ---------------------------------------------------------------------------

queries! {
    /// Regions, seen-state and anchors — invariant 4's core. Every awareness
    /// surface in the build is one of these plus a rendering.
    Region(RegionQuery) = "region" {
        Regions = "regions" [S5 / "T041"] -> ParamType::List(&ParamType::Any),
            "every region in scope, seen and unseen" {
            filter: Option<RegionFilter> = "narrow by author, state or scope",
        }
        UnseenRegions = "unseen-regions" [S5 / "T041"] -> ParamType::List(&ParamType::Any),
            "the unseen regions of a file — 6b's first line" {
            path: Option<std::path::PathBuf> = "which file; absent means everywhere",
        }
        Region = "region" [S5 / "T041"] -> ParamType::Any,
            "one region" {
            region: RegionId = "which region",
        }
        UnseenCount = "unseen-count" [S5 / "T041"] -> ParamType::Uint,
            "how many regions are unseen in scope — the statusline's ●n" {
            within: Option<Target> = "scope; absent means the workspace",
        }
        SeenCount = "seen-count" [S5 / "T041"] -> ParamType::Uint,
            "how many regions are seen in scope" {
            within: Option<Target> = "scope; absent means the workspace",
        }
        NextRegionBy = "next-region-by" [S5 / "T049"] -> ParamType::Any,
            "the next region by an author, from a position — 6b binds ]r to this" {
            author: Actor = "whose region",
            from: Option<Position> = "start here; absent means the cursor",
        }
        BlockRegions = "block-regions" [S5 / "T041"] -> ParamType::List(&ParamType::Any),
            "the regions of a review block, named by its title as 6b draws it" {
            block: String = "the block's title",
        }
        Anchors = "anchors" [S5 / "T042"] -> ParamType::List(&ParamType::Any),
            "a file's anchors and what tier each resolved at" {
            path: std::path::PathBuf = "which file",
        }
        Anchor = "anchor" [S5 / "T042"] -> ParamType::Any,
            "one anchor" {
            anchor: AnchorId = "which anchor",
        }
    }

    /// Buffers, the cursor and the viewport.
    Buffer(BufferQuery) = "buffer" {
        Buffers = "buffers" [S3 / "T033"] -> ParamType::List(&ParamType::Any),
            "every open buffer" {
        }
        Buffer = "buffer" [S3 / "T033"] -> ParamType::Any,
            "one buffer: path, language, dirty state, disk state" {
            buffer: Option<BufferId> = "which buffer; absent means the focused one",
        }
        BufferText = "buffer-text" [S3 / "T026"] -> ParamType::Text,
            "the text of a target" {
            target: Target = "what to read",
        }
        BufferLines = "buffer-lines" [S3 / "T026"] -> ParamType::List(&ParamType::Text),
            "the lines of a target" {
            target: Target = "what to read",
        }
        Cursor = "cursor" [S3 / "T026"] -> <Position as Wire>::TYPE,
            "where the cursor is — the statusline's 12:1" {
            pane: PaneRef = "which pane",
        }
        Selection = "selection" [S3 / "T026"] -> ParamType::Any,
            "the live selection, if there is one" {
            pane: PaneRef = "which pane",
        }
        Viewport = "viewport" [S3 / "T026"] -> ParamType::Any,
            "what is on screen: first row, height, and whether soft wrap is on" {
            pane: PaneRef = "which pane",
        }
        DirtyBuffers = "dirty-buffers" [S3 / "T033"] -> ParamType::List(&ParamType::Any),
            "every buffer with unsaved changes" {
        }
        DiskState = "disk-state" [S7 / "T069"] -> ParamType::Any,
            "whether a file changed underneath us, and when — what ✱ renders" {
            path: std::path::PathBuf = "which file",
        }
    }

    /// The input machine and the keymap. `T034`'s which-key and `T086`'s help
    /// are both compositions over these.
    Input(InputQuery) = "input" {
        Mode = "mode" [S3 / "T026"] -> <EditMode as Wire>::TYPE,
            "the current edit mode — not the statusline chip, which is a surface label" {
        }
        PendingKeys = "pending-keys" [S3 / "T026"] -> ParamType::Text,
            "the keys typed so far in an unfinished sequence — 3c's SPC pending" {
        }
        Register = "register" [S3 / "T099"] -> ParamType::Text,
            "what a register holds — @ reads one back, and an unset one is empty" {
            register: RegisterName = "which register",
        }
        Keymap = "keymap" [S3 / "T033"] -> ParamType::List(&ParamType::Any),
            "the live keymap under a prefix; redefining a binding changes this at once" {
            prefix: Option<KeySeq> = "the prefix; absent means the root",
            mode: Option<EditMode> = "which mode; absent means the current one",
        }
        DescribeKey = "describe-key" [S3 / "T086"] -> ParamType::Any,
            "what a key does, and where that is defined" {
            keys: KeySeq = "the key sequence",
        }
        DescribeCapability = "describe-capability" [S2 / "T024"] -> ParamType::Any,
            "one capability's row: doc, arguments, phase, and its name at each door" {
            name: String = "the door name",
        }
        Capabilities = "capabilities" [S2 / "T024"] -> ParamType::List(&ParamType::Any),
            "the whole registry, as data — what the door-parity test and :help both read" {
            domain: Option<String> = "one domain; absent means all of them",
        }
    }

    /// Panes, floats, the picker, and the theme — the shape of the screen.
    Ui(UiQuery) = "ui" {
        Panes = "panes" [S6 / "T088"] -> ParamType::Any,
            "the pane tree, with which one has focus" {
        }
        Floats = "floats" [S2 / "T021"] -> ParamType::List(&ParamType::Any),
            "the open floats; at most one has focus (Design Language §9)" {
        }
        PickerRows = "picker-rows" [S5 / "T045"] -> ParamType::List(&ParamType::Any),
            "a source's rows under a filter, ranked" {
            source: SourceId = "which source",
            query: Option<String> = "the filter",
            limit: Option<u32> = "how many rows; capped regardless",
            offset: Option<u32> = "skip this many",
        }
        Theme = "theme" [S1 / "T010"] -> ParamType::Any,
            "the active theme: slug, ground, and the actor hues" {
        }
        Options = "options" [S2 / "T021"] -> ParamType::Any,
            "every declared option and its value" {
        }
        Languages = "languages" [S4 / "T037"] -> ParamType::List(&ParamType::Any),
            "the declared languages, and which tier each is" {
            language: Option<LanguageId> = "one language; absent means all",
        }
        Arch = "arch" [S5 / "T048"] -> ParamType::Any,
            "the workspace's shape, for :arch — a store query with no Rust primitive (Q11)" {
        }
    }

    /// Review blocks, hunks, threads and the inbox — everything claude said
    /// about the code.
    Review(ReviewQuery) = "review" {
        ReviewBlocks = "review-blocks" [S6 / "T053"] -> ParamType::List(&ParamType::Any),
            "every declared review block" {
        }
        ReviewBlock = "review-block" [S7 / "T066"] -> ParamType::Any,
            "one block: its files, groups and annotations" {
            block: BlockId = "which block",
        }
        Hunks = "hunks" [S7 / "T063"] -> ParamType::List(&ParamType::Any),
            "a block's hunks, with each one's seen state" {
            block: BlockId = "which block",
        }
        Threads = "threads" [S7 / "T068"] -> ParamType::List(&ParamType::Any),
            "the threads anchored inside a target" {
            within: Option<Target> = "scope; absent means the workspace",
        }
        Thread = "thread" [S7 / "T068"] -> ParamType::Any,
            "one thread and its replies" {
            thread: ThreadId = "which thread",
        }
        Inbox = "inbox" [S7 / "T067"] -> ParamType::List(&ParamType::Any),
            "the inbox; unread derives from seen-state rather than duplicating it" {
            limit: Option<u32> = "how many items",
            offset: Option<u32> = "skip this many",
        }
        Diagnostics = "diagnostics" [S4 / "T040"] -> ParamType::List(&ParamType::Any),
            "the diagnostics for a file, or all of them" {
            path: Option<std::path::PathBuf> = "which file; absent means all",
        }
    }

    /// The session: its state, its turns, and the queue of things waiting on
    /// you.
    Session(SessionQuery) = "session" {
        Session = "session" [S6 / "T051"] -> ParamType::Any,
            "the session's state — what the statusline's ✻ and elapsed timer render" {
        }
        Turns = "turns" [S6 / "T054"] -> ParamType::List(&ParamType::Any),
            "the transcript, newest last" {
            limit: Option<u32> = "how many turns",
            offset: Option<u32> = "skip this many",
        }
        Turn = "turn" [S6 / "T054"] -> ParamType::Any,
            "one turn: its prose, its tool calls, and how it ended" {
            turn: TurnId = "which turn",
        }
        PendingAsks = "pending-asks" [S6 / "T060"] -> ParamType::List(&ParamType::Any),
            "the ask queue — one truth for ]!, the inbox and the statusline ! (Q9)" {
        }
        Ask = "ask" [S6 / "T060"] -> ParamType::Any,
            "one queued ask and its options" {
            ask: AskId = "which ask",
        }
        Watches = "watches" [S8 / "T074"] -> ParamType::List(&ParamType::Any),
            "the placed watches" {
            within: Option<Target> = "scope; absent means the workspace",
        }
        WatchValues = "watch-values" [S8 / "T075"] -> ParamType::List(&ParamType::Any),
            "a watch's values, with where they came from" {
            watch: WatchId = "which watch",
        }
    }

    /// Version control. Every one of these answers empty in a bare directory —
    /// no repository is a normal state, not an error (`S7.3`).
    Vcs(VcsQuery) = "vcs" {
        VcsStatus = "vcs-status" [S7 / "T071"] -> ParamType::Any,
            "the backend, the current change, and whether the tree is clean" {
        }
        Timeline = "timeline" [S7 / "T073"] -> ParamType::List(&ParamType::Any),
            "the timeline, newest first; agent turns are changes (3b)" {
            limit: Option<u32> = "how many entries",
            offset: Option<u32> = "skip this many",
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a query could not be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// No query has that name.
    Unknown {
        /// What was asked for.
        name: String,
    },
    /// An argument was missing or the wrong shape.
    Argument {
        /// The query being called.
        name: &'static str,
        /// What went wrong.
        source: WireError,
    },
    /// Named in the vocabulary, not built yet. Carries the task that builds it.
    NotYetImplemented {
        /// The `docs/TASKS.md` id.
        task: &'static str,
    },
}

impl core::fmt::Display for QueryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown { name } => write!(f, "no such query `{name}`"),
            Self::Argument { name, source } => write!(f, "`{name}`: {source}"),
            Self::NotYetImplemented { task } => {
                write!(f, "not built yet — {task} builds it")
            }
        }
    }
}

impl std::error::Error for QueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Argument { source, .. } => Some(source),
            Self::Unknown { .. } | Self::NotYetImplemented { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Behaviour that is not generated
// ---------------------------------------------------------------------------

impl Query {
    /// This query's registry row.
    ///
    /// # Panics
    ///
    /// Never in practice: the macro emits the variant and the row together, and
    /// `tests/vocabulary.rs` proves it for every row.
    #[must_use]
    pub fn spec(&self) -> &'static QuerySpec {
        let name = self.name();
        QUERIES
            .iter()
            .find(|spec| spec.name == name)
            .expect("every Query variant is emitted with its registry row")
    }

    /// The MCP door's policy for a query: reading is always allowed.
    ///
    /// Here rather than in the table because it is a property of the *kind*, not
    /// of the capability — a query cannot move anything, so there is nothing for
    /// a policy to protect. The refusals that matter are on the write side and
    /// on focus-relative targets.
    #[must_use]
    pub const fn mcp_policy(&self) -> McpPolicy {
        McpPolicy::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixb_spellings_are_the_ones_we_registered() {
        // TUI Mockups.dc.html:493-503 — these three spellings are drawn, so they
        // are fixed. A rename here is a mockup that stops reproducing.
        for name in ["unseen-regions", "block-regions", "next-region-by"] {
            assert!(
                QUERIES.iter().any(|spec| spec.name == name),
                "6b draws ({name} …) and nothing registers it"
            );
        }
    }

    #[test]
    fn a_query_round_trips() {
        let query = Query::Region(RegionQuery::UnseenRegions {
            path: Some(std::path::PathBuf::from("src/retry.rs")),
        });
        let call = query.to_call();
        assert_eq!(call.name, "unseen-regions");
        assert_eq!(Query::from_call(&call.name, &call.args).unwrap(), query);
    }

    #[test]
    fn an_absent_optional_argument_is_legal() {
        let query = Query::from_call("unseen-regions", &Args::new()).unwrap();
        assert_eq!(
            query,
            Query::Region(RegionQuery::UnseenRegions { path: None })
        );
    }

    #[test]
    fn revisions_only_move_forward() {
        let first = Revision::INITIAL;
        let second = first.next();
        assert!(second > first);
        assert_eq!(second.get(), 1);
    }
}
