//! The semantic store, seen from the binary (`T041`).
//!
//! `phosphor_core::store::Store` is plain data — that crate holds no locks, for
//! the reason its own header gives — and this is the handle that makes it
//! shareable. Two readers on different sides of the Steel barrier, which is the
//! shape `crate::lsp` already argued for its diagnostics and now gets for the
//! whole store:
//!
//! * the **loop** reads it per frame, to build the gutter's regions and the
//!   statusline's `●n`, while holding `&mut Editing`;
//! * the **host** ([`crate::AppHost`]) applies `declare-regions`, `mark-seen`,
//!   `mark-unseen` and `drop-regions` to it and answers the `region` queries
//!   off it, from inside a running VM, behind `&self`.
//!
//! One store with two handles is what keeps those two from disagreeing about a
//! file — the alternative is the statusline counting regions the gutter is not
//! drawing.
//!
//! # `crate::lsp::Diagnostics` used to be this module
//!
//! It held a `BTreeMap<PathBuf, Vec<Diagnostic>>` and its own
//! `replace`/`of`/`answer`, written at `T040` because the map it should have
//! been using — `phosphor_core::store::diagnostics` — has no lock and this
//! binary needed one. So there were two maps with one name, and the *documented*
//! store had no importer at all. `T041` folded it in: the core store owns the
//! diagnostics beside the regions, one revision moves for both, and what is
//! left here is the lock.
//!
//! # Paths are workspace-relative, and this module is where that becomes true
//!
//! `request::RegionSpec` documents its path as workspace-relative and the store
//! never interprets one — *"a host that mixes the two forms sees two files"*.
//! The host is this binary, so reconciling them is this module's job:
//! [`key_for`] strips the working directory off an absolute path, on **both**
//! the declaring side and the looking-up side. A door that declares
//! `src/retry.rs` and an editor showing `/work/src/retry.rs` then agree, which
//! is the difference between a marker appearing and a marker silently not.
//!
//! Owned by `spine` — everything here is the loop's half of the seam.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use phosphor_core::journal;
use phosphor_core::query::Revision;
use phosphor_core::request::{
    Actor, AnchorId, BlockId, Diagnostic, FileGroup, FileSpan, GroupId, HunkId, InboxId, Position,
    RegionId, RegionSpec, Severity, Span, ThreadId,
};
use phosphor_core::store::{
    Anchor, Declared, Fingerprint, Lens, Reanchored, Region, Scope, SeenLog, SeenState, Snapshot,
    Store, persist,
};
use phosphor_core::value::Value;

/// One declared review block (`T053`).
///
/// **Regions grouped by the declaration that made them**, which is the whole
/// of what a block adds: `declare-regions` already creates markers one span at
/// a time, and `8b`'s review surface needs to know which of them arrived
/// together and what claude said about each file. So a block holds ids, not
/// spans — the region *is* the span, and a block that carried its own copy
/// would be a second place for the two to disagree after a rewrite moves one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    /// Which block.
    pub(crate) id: BlockId,
    /// What this block is — `1b`'s `review ready · retry logic`.
    pub(crate) title: String,
    /// Claude's note about the block as a whole.
    pub(crate) annotation: Option<String>,
    /// One entry per file, in the order declared.
    pub(crate) groups: Vec<Group>,
    /// When this block was declared, against [`Shared::arrivals`] (`T067`).
    ///
    /// **The one thing a `BlockId` cannot answer on its own.** `5c` interleaves
    /// asks, blocks and notes by recency, and `BlockId`, `AskId` and
    /// `Note::arrival` are three counters that mint independently — a second
    /// block declared after a note would have a *lower* `BlockId` than the
    /// note's own inbox-encoded id if the merge sorted on either counter
    /// alone. This is the shared clock the merge actually needs.
    pub(crate) arrival: u64,
}

/// One file's contribution to a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    /// Which group (`T064`).
    ///
    /// Minted per group across the whole session rather than per block, so a
    /// [`GroupId`] names one group without a block beside it — which is what
    /// `Target::Group { id }` is, a target carrying one id and no context.
    pub(crate) id: GroupId,
    /// Workspace-relative, through [`key_for`] like every other path here.
    pub(crate) path: PathBuf,
    /// Claude's own annotation for this group — `8b`'s *"mechanical"* versus
    /// *"the meat"*.
    pub(crate) annotation: Option<String>,
    /// The regions this group declared, each with what it replaced (`T066`).
    pub(crate) regions: Vec<Change>,
}

/// One declared region and the text it replaced (`T066`).
///
/// **The before-side of a hunk, and the only place it lives.** The after-side is
/// read live — the region's text now — so this is not a copy of anything the
/// store also holds. OPEN-QUESTIONS.md §59 rules why it is claude's to state and
/// why carrying it does not make a block a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    /// The region the span became.
    pub(crate) region: RegionId,
    /// What was there before, verbatim. [`None`] means it removed nothing.
    pub(crate) was: Option<String>,
}

/// **A hunk is a region, and this is the type that says so once** (`T064`).
///
/// `declare-review-block` makes one region per changed span
/// ([`Shared::declare_block`]), so inside a review block *one span is one
/// region is one hunk* — three names for the thing `4b` draws a `+`/`−` beside
/// and `s` marks seen. [`HunkId`] is therefore the region's id under the review
/// surface's name, and this conversion is the only place the two spellings
/// meet.
///
/// **The alternative was a hunk table, and it would have been a second place
/// for seen-state to live.** §7 has one mutable flag and it is on the region;
/// a hunk row carrying its own would be two records of one bit, disagreeing the
/// first time a rewrite moved a span. The block's own doc already made this
/// ruling for spans — *"a block holds ids, not spans"* — and this is the same
/// ruling one noun further out.
///
/// **No longer `Copy`** — it carries the before-side, which is a `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    /// Which hunk — the region's id, read as a hunk.
    pub(crate) id: HunkId,
    /// Which group it belongs to.
    pub(crate) group: GroupId,
    /// Which file it is in, so a caller can read the after-side (`T066`).
    pub(crate) path: PathBuf,
    /// Where it is, so a caller can jump to it without a second query.
    pub(crate) span: Span,
    /// The text it replaced, if claude said (`T066`, §59). [`None`] means it
    /// removed nothing.
    pub(crate) was: Option<String>,
    /// Whether it has been read.
    pub(crate) seen: bool,
}

impl Hunk {
    /// The region a hunk id names.
    ///
    /// **Takes the id rather than the row**, because the caller that matters
    /// is `Target::Hunk { id }` — a target carries an id and no row. A
    /// convenience `fn region(self)` beside this one lasted exactly as long as
    /// it took clippy to notice nothing outside the tests called it, which is
    /// what a second spelling of one conversion looks like from the outside.
    pub(crate) const fn region_of(hunk: HunkId) -> RegionId {
        RegionId(hunk.0)
    }

    /// The hunk a region is.
    pub(crate) const fn id_of(region: RegionId) -> HunkId {
        HunkId(region.0)
    }
}

/// What an inbox row is a row *of* (`T067`).
///
/// `5c` is *"everything claude said"*, and the three things he says already
/// live in three places: a pending question (`T060`'s queue), a declared review
/// block (`T053`), and a note (`Shared::notify`). The inbox is a **view** over
/// them, not a fourth store — which is what `CP-8a` asks for when it says
/// unread must derive from seen-state rather than duplicate it.
///
/// That leaves one problem: `open-inbox-item` and `Target::InboxItem` both name
/// a row by [`InboxId`], and a row's identity has to outlive the query that
/// produced it. An index into the merged list would not — a note arriving
/// renumbers every row under it. So the id **carries its own source**, which is
/// what makes `InboxId(9)` mean the same row on two consecutive calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InboxSource {
    /// A question waiting to be answered — `5c`'s `! needs input`.
    Ask(u64),
    /// A declared review block — `5c`'s `✻ review ready`.
    Block(BlockId),
    /// A note claude posted — `5c`'s `· note`.
    Note(u64),
}

/// How many kinds an [`InboxId`] can encode.
///
/// Four rather than three, so a fourth kind is an added arm rather than a
/// renumbering of every id in a running session.
const INBOX_KINDS: u64 = 4;

impl InboxSource {
    /// This source's row id.
    ///
    /// **The whole encoding, in one pair of functions**, the shape
    /// [`Hunk::region_of`] already uses for the other id coupling in this file:
    /// two spellings of one fact meet in exactly one place, so there is nowhere
    /// for them to disagree.
    pub(crate) const fn id(self) -> InboxId {
        let (kind, n) = match self {
            Self::Ask(n) => (0, n),
            Self::Block(block) => (1, block.0),
            Self::Note(n) => (2, n),
        };
        InboxId(n * INBOX_KINDS + kind)
    }

    /// The source a row id names, or [`None`] for a kind nothing encodes.
    pub(crate) const fn of(id: InboxId) -> Option<Self> {
        let n = id.0 / INBOX_KINDS;
        match id.0 % INBOX_KINDS {
            0 => Some(Self::Ask(n)),
            1 => Some(Self::Block(BlockId(n))),
            2 => Some(Self::Note(n)),
            _ => None,
        }
    }
}

/// One note claude posted — `notify`, and `5c`'s `· note` row (`T067`).
///
/// **The only inbox row with storage of its own, and that is the whole of what
/// `T067` adds.** `5c` is *"everything claude said"*, which is three things
/// that already exist: a pending ask (`T060`'s queue), a declared review block
/// (`T053`), and a note. The first two are read where they live — `CP-8a` asks
/// that unread *derive* from seen-state rather than being copied, and a copy is
/// exactly what a fourth store would be.
///
/// A note has nowhere else to live, so its `seen` bit is not a duplicate of
/// anything; it is the fact itself.
#[derive(Debug, Clone)]
pub(crate) struct Note {
    /// Which item.
    pub(crate) id: InboxId,
    /// `info`, `attention` or `trouble` — one flag, as the task says.
    pub(crate) severity: Severity,
    /// `5c`'s one line.
    pub(crate) title: String,
    /// The rest, if there is any.
    pub(crate) body: Option<String>,
    /// What it is about, when claude named a place.
    pub(crate) anchor: Option<FileSpan>,
    /// When it was posted.
    ///
    /// **[`Instant`], not a wall clock.** `5c` draws `2m` for the newest row
    /// and `14:41` for the older ones; nothing in this tree can render the
    /// second half — there is no timezone-aware clock in the dependency graph
    /// and adding one for a timestamp format is not a trade this task makes.
    /// So every row renders relative, which orders them identically and says
    /// the same thing about recency. Recorded rather than faked.
    pub(crate) at: Instant,
    /// Whether it has been read.
    pub(crate) seen: bool,
    /// When this note was posted, against [`Shared::arrivals`] (`T067`).
    /// See [`Block::arrival`] for why one shared clock is needed at all.
    pub(crate) arrival: u64,
}

/// One message in a thread — `3a`'s `⚓ you · 2m` or `✻ claude · 1m` (`T068`).
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    /// Who said it. §1 gives each actor exactly one colour, and this is the
    /// field that picks it.
    pub(crate) actor: Actor,
    /// What they said, on one row.
    pub(crate) body: String,
    /// When, for the relative age `3a` draws. [`Instant`] for
    /// [`Note::at`]'s reason — there is no wall clock in this tree.
    pub(crate) at: Instant,
}

/// One anchored exchange — `3a` (`T068`).
///
/// **A thread is a conversation *about a place*, and the place is a span
/// rather than a region.** `T042`'s anchors are the machinery for surviving a
/// rewrite and `T041`'s regions are §7's seen-state; a thread is neither. It
/// hangs where you put it, is drawn as virtual text under that line, and gives
/// the line §3's row-20 treatment — a tint and an undercurl, and no bar in the
/// column, because a conversation is an overlay and not a claim about
/// attention.
#[derive(Debug, Clone)]
pub(crate) struct Thread {
    /// Which thread.
    pub(crate) id: ThreadId,
    /// Workspace-relative, through [`key_for`] like every other path here.
    pub(crate) path: PathBuf,
    /// Where it is anchored.
    pub(crate) span: Span,
    /// Your comment, then whatever came back — oldest first, which is how a
    /// conversation reads and how `3a` stacks the rows.
    pub(crate) replies: Vec<Reply>,
    /// Marked done without being deleted (`resolve-thread`).
    ///
    /// **Resolved is not deleted, and `3a` is why.** *"revised lines go unseen
    /// again"* — the exchange is the record of why a line looks the way it
    /// does, and a verb that could only destroy it would make the honest move
    /// (finishing with a thread) also the lossy one.
    pub(crate) resolved: bool,
}

/// A file that changed on disk under an open buffer (`T069`, screen `1d`).
///
/// **This is a fact about the file, not a decision about the buffer.**
/// Invariant 3 is that nothing moves unless you asked, so the store records
/// that disk and buffer disagree and stops there; `reload-from-disk` is the
/// only thing that closes the gap, and only a person or a door can call it.
#[derive(Debug, Clone)]
pub(crate) struct DiskChange {
    /// Who the change is attributed to. [`Actor::System`] is the honest
    /// answer, not a missing one.
    ///
    /// **A watcher cannot see an author** — `notify` reports that bytes moved,
    /// not who moved them — so this is a stated heuristic rather than a
    /// measurement: a turn was running when the burst landed, and §7's rule is
    /// that the machine tracks claude. `1d` draws *"changed on disk by
    /// claude"* on a statusline that also reads `✻ claude working`, which is
    /// the same condition.
    ///
    /// With no turn running this is [`Actor::System`] and the notice drops the
    /// *"by …"* clause rather than guessing, because *"by claude"* over a
    /// `git checkout` would be the editor asserting something it does not know.
    /// The vocabulary already framed it this way: `note-disk-change`'s own
    /// parameter doc reads *"who is **claimed** to have changed it"*, so the
    /// Action carries an attribution and this field is the same attribution
    /// held still. There is no `Option` here for that reason — a change with no
    /// known author is `System`, which is a name, not an absence.
    pub(crate) actor: Actor,
    /// How many debounced bursts have landed since the buffer last agreed with
    /// disk.
    ///
    /// Recorded so the load-bearing half of `T069` is assertable directly
    /// rather than by counting glyphs on a screen: the task's own line is that
    /// *"an agent writing a file produces a burst of events, and one `✱` per
    /// burst is the honest signal"*. One save must move this by exactly one.
    pub(crate) bursts: u64,
}

/// The store, shared.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    store: Mutex<Store>,
    /// Declared review blocks (`T053`).
    ///
    /// **Beside the region store rather than inside it, and not journalled.**
    /// `phosphor_core::store` is the *seen-state* machine — §7's one mutable
    /// flag, persisted so a marker survives a restart — and a block is a
    /// statement claude made once. The regions it created are journalled like
    /// any others; the grouping is not, because `T067`'s inbox is the surface
    /// for what claude said that outlives a session, and duplicating it here
    /// would be two records of one sentence.
    blocks: Mutex<Vec<Block>>,
    /// **The one clock `5c`'s merge needs** — minted by [`Shared::mint_arrival`]
    /// and stamped on a [`Block`] and a [`Note`] alike, so the two can be
    /// sorted together by when they actually arrived. Blocks and notes each
    /// already have their own id counter; neither is comparable to the
    /// other's, which is exactly the bug `T067`'s own test found before this
    /// field existed — a second block declared after a note sorted as older
    /// than it.
    arrivals: Mutex<u64>,
    /// Anchored exchanges (`T068`), oldest first.
    ///
    /// Beside `blocks` and `notes` and not journalled, for the reason those
    /// two give: `phosphor_core::store` is the *seen-state* machine, and a
    /// thread is a conversation. What that costs is that a thread does not
    /// survive a restart — the same trade `T053`'s blocks already make.
    threads: Mutex<Vec<Thread>>,
    /// Notes claude posted (`T067`), oldest first.
    ///
    /// Beside `blocks` and not journalled, for exactly `blocks`'s reason: a note
    /// is a statement claude made once, and `phosphor_core::store` is the
    /// *seen-state* machine. What that costs is that a note does not survive a
    /// restart — which is the same trade `T053`'s blocks already make, and the
    /// inbox is a session's news rather than an archive.
    notes: Mutex<Vec<Note>>,
    /// How many groups have been minted this session (`T064`).
    ///
    /// Beside `blocks` rather than derived from it, because a group id must be
    /// stable and never reused: counting the groups already in `blocks` would
    /// mint a fresh id that collided with an old one the moment a block was
    /// ever removed. Nothing removes one today, and this is the field that
    /// keeps that from being load-bearing.
    groups: Mutex<u64>,
    /// Files that changed on disk under an open buffer (`T069`), keyed the
    /// way every other path in this module is keyed.
    ///
    /// Beside the region store and not journalled, for `blocks`'s reason one
    /// field up and one more of its own: this is a disagreement between two
    /// things that both exist *now*. Restoring it would mean restoring a claim
    /// about a file the editor has not looked at since, and the first thing
    /// `T069`'s watcher does on open is establish the truth anyway.
    disk: Mutex<BTreeMap<PathBuf, DiskChange>>,
    /// The seen-state journal (`T044`), or [`None`] when there is nowhere to
    /// put one.
    ///
    /// **Absent is a working editor, not a broken one.** No `XDG_STATE_HOME`
    /// and no `HOME`, a read-only state directory, a corrupt header — each
    /// means seen-state does not survive this session, and none of them is a
    /// reason to refuse to edit. The same call the undo journal makes
    /// (`Timeline::opened`), for the same reason it gives: *"a history is not
    /// the file."*
    log: Mutex<Option<SeenLog>>,
}

impl Shared {
    /// A store restored from this workspace's journal, and what went wrong if
    /// anything did (`T044`).
    ///
    /// The workspace is the directory the editor was started in — Q1's *"keyed
    /// on the path and never on VCS identity"*, with the honest root `S3` has,
    /// which is the same key `Timeline::open_at` uses.
    pub(crate) fn opened() -> (Self, Option<String>) {
        match Self::open_at() {
            Ok(shared) => (shared, None),
            Err(reason) => (Self::default(), Some(reason)),
        }
    }

    fn open_at() -> Result<Self, String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let dir = journal::workspace_dir(&root).map_err(|error| error.to_string())?;
        let path = journal::seen_path(&dir);
        // `Recovery` is discarded for the reason `Timeline::open_at` gives:
        // `Log::open` has already truncated the torn tail, and what a crash
        // cost is the last thing marked — which the next `s` fixes.
        let (log, _recovery) = SeenLog::open(&path).map_err(|error| error.to_string())?;
        Ok(Self {
            store: Mutex::new(Store::restore(log.state().clone())),
            log: Mutex::new(Some(log)),
            // Not restored: a block is a statement, not seen-state. See the
            // field's own note. The group counter restarts with it, which is
            // consistent rather than a gap — ids that name nothing do not need
            // to be reserved.
            blocks: Mutex::new(Vec::new()),
            arrivals: Mutex::new(0),
            threads: Mutex::new(Vec::new()),
            notes: Mutex::new(Vec::new()),
            groups: Mutex::new(0),
            disk: Mutex::new(BTreeMap::new()),
        })
    }

    /// Append records, and drop the journal if it stops working.
    ///
    /// **A failed append disables the journal rather than failing the edit.**
    /// The alternative is an editor that refuses `s` because a disk filled up,
    /// which trades a lost flag for a lost session. The journal going quiet is
    /// the same degradation as never having had one, which is a state this
    /// type already supports.
    fn write(&self, records: Vec<persist::Record>) {
        if records.is_empty() {
            return;
        }
        let Ok(mut held) = self.log.lock() else {
            return;
        };
        let Some(log) = held.as_mut() else {
            return;
        };
        for record in records {
            if log.append(record).is_err() {
                *held = None;
                return;
            }
        }
        // Compaction is the journal's own doubling policy, so this is a check
        // rather than a rewrite on all but a few calls.
        if log.compact_if_needed().is_err() {
            *held = None;
        }
    }

    /// Upserts for live rows and tombstones for the rest, read off the store
    /// under one lock.
    ///
    /// Reading the rows back rather than being handed them is deliberate: what
    /// belongs on disk is *what the store now says*, and a caller that built
    /// records from its own idea of the mutation could write a row the store
    /// never agreed to.
    fn rows_of(store: &Store, regions: &[RegionId], anchors: &[AnchorId]) -> Vec<persist::Record> {
        let mut out = Vec::with_capacity(regions.len() + anchors.len() + 1);
        out.push(persist::Record::Minted {
            regions: store.regions().minted(),
            anchors: store.anchors().minted(),
        });
        for id in regions {
            out.push(match store.regions().get(*id) {
                Some(region) => persist::Record::Region(Box::new(region.clone())),
                None => persist::Record::RegionGone(*id),
            });
        }
        for id in anchors {
            out.push(match store.anchors().get(*id) {
                Some(anchor) => persist::Record::Anchor(Box::new(anchor.clone())),
                None => persist::Record::AnchorGone(*id),
            });
        }
        out
    }

    /// **`declare-regions`.**
    pub(crate) fn declare(&self, specs: &[RegionSpec], asked_by: Actor) -> Declared {
        let specs: Vec<RegionSpec> = specs
            .iter()
            .map(|spec| RegionSpec {
                path: key_for(&spec.path),
                ..spec.clone()
            })
            .collect();
        let (declared, records) = {
            let mut store = self.lock();
            let declared = store.declare_regions(&specs, asked_by);
            let touched: Vec<RegionId> = declared
                .created
                .iter()
                .chain(&declared.revised)
                .copied()
                .collect();
            let records = Self::rows_of(&store, &touched, &[]);
            (declared, records)
        };
        self.write(records);
        declared
    }

    /// **`declare-review-block`.** Declares every group's spans and records
    /// what arrived together (`T053`).
    ///
    /// One `declare` per group rather than one for all of them, because the
    /// answer has to say *which* regions belong to *which* file — and
    /// [`Declared`] reports ids without saying which spec produced them. Per
    /// group, that ambiguity does not arise.
    ///
    /// **`revised` counts as belonging to the block, not just `created`.** An
    /// agent that touches the same span twice in one session has revised a
    /// region rather than made a second one, and a block that listed only the
    /// new ones would under-report exactly the files claude worked hardest on.
    pub(crate) fn declare_block(
        &self,
        title: &str,
        files: &[FileGroup],
        annotation: Option<&str>,
        asked_by: Actor,
    ) -> Block {
        let groups: Vec<Group> = files
            .iter()
            .map(|file| {
                let id = self.mint_group();
                // **One `declare` per *span* now, not per group.** The reason is
                // the one this doc already gives, one level finer: [`Declared`]
                // reports ids without saying which spec produced them, and
                // `T066` needs each region paired with the text *its* span
                // replaced. Per group that ambiguity did not arise between
                // files; per span it does not arise at all.
                //
                // The cost is one lock per span instead of one per file, on a
                // path an agent takes once at the end of a turn.
                let regions: Vec<Change> = file
                    .spans
                    .iter()
                    .flat_map(|changed| {
                        let declared = self.declare(
                            &[RegionSpec {
                                path: file.path.clone(),
                                span: changed.span,
                                author: asked_by,
                            }],
                            asked_by,
                        );
                        declared
                            .created
                            .iter()
                            .chain(&declared.revised)
                            .map(|region| Change {
                                region: *region,
                                was: changed.was.clone(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                Group {
                    id,
                    path: key_for(&file.path),
                    annotation: file.annotation.clone(),
                    regions,
                }
            })
            .collect();

        let mut blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Minted from the count, so ids are stable within a session and never
        // reused — the same rule `Buffers::open` and `Panes::mint` follow.
        let block = Block {
            id: BlockId(blocks.len() as u64),
            title: title.to_owned(),
            annotation: annotation.map(str::to_owned),
            groups,
            arrival: self.mint_arrival(),
        };
        blocks.push(block.clone());
        block
    }

    /// **`notify`** — claude posts a note to the inbox (`T067`).
    ///
    /// Answers the id, so a caller can name the row it just made. Ids are
    /// minted from the count for [`Shared::declare_block`]'s reason: stable
    /// within a session and never reused.
    pub(crate) fn notify(
        &self,
        severity: Severity,
        title: &str,
        body: Option<&str>,
        anchor: Option<FileSpan>,
    ) -> InboxId {
        let mut notes = self
            .notes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = InboxSource::Note(notes.len() as u64).id();
        notes.push(Note {
            id,
            severity,
            title: title.to_owned(),
            body: body.map(str::to_owned),
            anchor: anchor.map(|span| FileSpan {
                path: key_for(&span.path),
                span: span.span,
            }),
            at: Instant::now(),
            seen: false,
            arrival: self.mint_arrival(),
        });
        id
    }

    /// Every note, oldest first (`T067`).
    pub(crate) fn notes(&self) -> Vec<Note> {
        self.notes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Marks one note read, or unread. Answers whether it existed (`T067`).
    ///
    /// **The one inbox row `mark-seen` touches directly.** A block row's seen
    /// state is its regions' and a pending ask's is the queue's; both are
    /// reached through the scopes that already exist, which is what keeps this
    /// from being a second seen-state machine.
    pub(crate) fn set_note_seen(&self, item: InboxId, seen: bool) -> bool {
        let mut notes = self
            .notes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(found) = notes.iter_mut().find(|note| note.id == item) else {
            return false;
        };
        found.seen = seen;
        true
    }

    /// **`start-thread`** — your comment in the margin (`T068`).
    ///
    /// Answers the id. Minted from the count, so ids are stable within a
    /// session and never reused — [`Shared::declare_block`]'s rule.
    pub(crate) fn start_thread(
        &self,
        path: &Path,
        span: Span,
        actor: Actor,
        body: &str,
    ) -> ThreadId {
        let mut threads = self
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = ThreadId(threads.len() as u64);
        threads.push(Thread {
            id,
            path: key_for(path),
            span,
            replies: vec![Reply {
                actor,
                body: body.to_owned(),
                at: Instant::now(),
            }],
            resolved: false,
        });
        id
    }

    /// **`reply-to-thread`** — *"claude's side arrives the same way yours
    /// does"*, which is this function called with a different [`Actor`].
    pub(crate) fn reply_to_thread(&self, thread: ThreadId, actor: Actor, body: &str) -> bool {
        let mut threads = self
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(found) = threads.iter_mut().find(|it| it.id == thread) else {
            return false;
        };
        found.replies.push(Reply {
            actor,
            body: body.to_owned(),
            at: Instant::now(),
        });
        // **A reply reopens it.** `3a`'s exchange is a conversation, and one
        // that answered a resolved thread while leaving it resolved would hide
        // the answer behind the state that said nobody was talking.
        found.resolved = false;
        true
    }

    /// **`resolve-thread`** — done, not deleted.
    pub(crate) fn resolve_thread(&self, thread: ThreadId, resolved: bool) -> bool {
        let mut threads = self
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(found) = threads.iter_mut().find(|it| it.id == thread) else {
            return false;
        };
        found.resolved = resolved;
        true
    }

    /// **`delete-thread`** — gone. Answers whether there was one.
    pub(crate) fn delete_thread(&self, thread: ThreadId) -> bool {
        let mut threads = self
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = threads.len();
        threads.retain(|it| it.id != thread);
        threads.len() != before
    }

    /// One region's file and span, by id (`T068`).
    ///
    /// The lookup `in_thread_scope` needs: a `These`/`One` scope names regions,
    /// and a thread is inside such a scope when its own span overlaps one of
    /// them — which cannot be asked without turning an id back into a place.
    pub(crate) fn region_span(&self, region: RegionId) -> Option<(PathBuf, Span)> {
        self.lock()
            .regions()
            .in_scope(&Scope::One(region))
            .next()
            .map(|found| (found.path.clone(), found.span))
    }

    /// Every thread, oldest first (`T068`).
    pub(crate) fn threads(&self) -> Vec<Thread> {
        self.threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The threads anchored in one file, oldest first (`T068`).
    ///
    /// **Resolved ones are included**, and the caller decides: `3a` draws them
    /// and the statusline's `1 thread` counts the unresolved. A filter here
    /// would make `threads` mean two different things to two readers.
    pub(crate) fn threads_in(&self, path: &Path) -> Vec<Thread> {
        let key = key_for(path);
        self.threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|thread| thread.path == key)
            .cloned()
            .collect()
    }

    /// One thread, by id (`T068`).
    pub(crate) fn thread_of(&self, thread: ThreadId) -> Option<Thread> {
        self.threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|it| it.id == thread)
            .cloned()
    }

    /// The span of the lowest-id thread covering a position — `vit` (`T068`).
    ///
    /// [`Shared::covering`]'s lowest-id rule, for its reason. **Resolved
    /// threads are still threads**: `vit` is *"the thread here"*, and a noun
    /// that skipped the finished ones would make `dit` after resolving one a
    /// delete of whatever else happened to be on the line.
    pub(crate) fn thread_covering(&self, path: &Path, at: Position) -> Option<Span> {
        let key = key_for(path);
        self.threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|thread| thread.path == key)
            // A zero-width span at the position, which is the same question
            // `Scope::Span` asks for a cursor and answered by the same
            // function — rather than a second reading of *"covers"* that could
            // disagree with it at an edge.
            .find(|thread| {
                phosphor_core::store::region::overlaps(thread.span, Span { start: at, end: at })
            })
            .map(|thread| thread.span)
    }

    /// Record that this file changed on disk, and answer how many bursts have
    /// landed since the buffer last agreed with it (`T069`).
    ///
    /// **Counting rather than setting a flag**, because the count is what
    /// makes debouncing provable. The caller is `T069`'s debouncer, which has
    /// already collapsed one save's worth of `notify` events into a single
    /// call; a second increment means a second *save*, not a second write
    /// syscall.
    ///
    /// The actor is taken from the first burst and never overwritten by a
    /// later one. Two writes from different sources between one reload and the
    /// next is a case this cannot describe honestly either way, and keeping the
    /// first is the one that matches what the notice already said.
    pub(crate) fn note_disk_change(&self, path: &Path, actor: Actor) -> u64 {
        let key = key_for(path);
        let mut disk = self.disk.lock().unwrap_or_else(|held| held.into_inner());
        let entry = disk.entry(key).or_insert(DiskChange { actor, bursts: 0 });
        entry.bursts += 1;
        entry.bursts
    }

    /// Forget that this file disagreed with disk — answers whether it did
    /// (`T069`).
    ///
    /// Called when the gap is actually closed: a reload took what was on disk,
    /// or a save made the buffer the thing on disk. Not called when the notice
    /// is dismissed, because dismissing a message does not make two files agree.
    pub(crate) fn clear_disk_change(&self, path: &Path) -> bool {
        let key = key_for(path);
        self.disk
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .remove(&key)
            .is_some()
    }

    /// What this file's disk disagreement is, if it has one (`T069`).
    pub(crate) fn disk_change(&self, path: &Path) -> Option<DiskChange> {
        let key = key_for(path);
        self.disk
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .get(&key)
            .cloned()
    }

    /// A fresh arrival stamp — the clock [`Block::arrival`]/[`Note::arrival`]
    /// read (`T067`).
    fn mint_arrival(&self) -> u64 {
        let mut minted = self
            .arrivals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stamp = *minted;
        *minted += 1;
        stamp
    }

    /// A fresh group id (`T064`).
    fn mint_group(&self) -> GroupId {
        let mut minted = self
            .groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = GroupId(*minted);
        *minted += 1;
        id
    }

    /// **One block's hunks, with each one's seen state** — the `hunks` query
    /// (`T064`).
    ///
    /// In declaration order, which is the order `4b` draws them: claude said
    /// these files in this order and the surface should not resort them behind
    /// its back.
    ///
    /// **Built from this block's own groups, not through [`Shared::hunk_of`],
    /// and that is load-bearing rather than a style choice.** A region two
    /// blocks both declared — the same span, redeclared — is ambiguous about
    /// *which* group it belongs to once you have only the region id, which is
    /// exactly the question `hunk_of`'s global scan answers by *"whichever
    /// block declared it first"*. That answer is wrong here: `hunks(second)`
    /// asking for the group a region is in *for this block* has a real answer
    /// — `second`'s own group — and a test caught the difference the first
    /// time two blocks shared a span.
    ///
    /// A region a block named and something later dropped is **skipped rather
    /// than reported unseen**. `unseen` is a fact about a marker, and a marker
    /// that is gone has no facts — reporting a default would put a row on `4b`
    /// with nothing under it.
    pub(crate) fn hunks(&self, block: BlockId) -> Vec<Hunk> {
        let groups: Vec<Group> = {
            let blocks = self
                .blocks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(found) = blocks.iter().find(|candidate| candidate.id == block) else {
                return Vec::new();
            };
            found.groups.clone()
        };
        let store = self.lock();
        groups
            .iter()
            .flat_map(|group| {
                group.regions.iter().filter_map(|change| {
                    let region = store
                        .regions()
                        .in_scope(&Scope::One(change.region))
                        .next()
                        .cloned()?;
                    Some(Hunk {
                        id: Hunk::id_of(region.id),
                        group: group.id,
                        path: region.path.clone(),
                        span: region.span,
                        was: change.was.clone(),
                        seen: !region.state.unseen(),
                    })
                })
            })
            .collect()
    }

    /// **One hunk, by the region it is** — `2b`'s peek (`T066`).
    ///
    /// **Global, and that is a real narrowing this function accepts.** Given
    /// only a region id and no block to scope the search, *"which group is
    /// this in"* is ambiguous exactly when two blocks declared the same span —
    /// [`Shared::hunks`]'s own doc explains why that case needs the block. A
    /// peek has an id from the cursor or from an agent's call and genuinely has
    /// no block in mind, so it takes the first declaring block's answer, the
    /// same convention [`Shared::covering`]'s lowest-id rule states for a
    /// parallel ambiguity.
    ///
    /// [`None`] for a region no block ever declared — the same *"not a hunk"*
    /// reading [`Shared::hunk_covering`] gives `vih`.
    pub(crate) fn hunk_of(&self, region: RegionId) -> Option<Hunk> {
        let (group, was) = {
            let blocks = self
                .blocks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            blocks
                .iter()
                .flat_map(|block| block.groups.iter())
                .find_map(|group| {
                    group
                        .regions
                        .iter()
                        .find(|change| change.region == region)
                        .map(|change| (group.id, change.was.clone()))
                })?
        };
        let row = self
            .lock()
            .regions()
            .in_scope(&Scope::One(region))
            .next()
            .cloned()?;
        Some(Hunk {
            id: Hunk::id_of(row.id),
            group,
            path: row.path,
            span: row.span,
            was,
            seen: !row.state.unseen(),
        })
    }

    /// The hunk at a position, whole — `2b`'s `gh` (`T066`).
    ///
    /// [`Shared::hunk_covering`] answers the span `vih` selects; this answers
    /// the row a peek is built from, at the cost of the extra lookup
    /// [`Shared::hunk_of`] does. Lowest-id for [`Shared::covering`]'s reason.
    pub(crate) fn hunk_near(&self, path: &Path, at: Position) -> Option<Hunk> {
        let key = key_for(path);
        let declared: Vec<RegionId> = {
            let blocks = self
                .blocks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            blocks
                .iter()
                .flat_map(|block| block.groups.iter())
                .flat_map(|group| group.regions.iter().map(|change| change.region))
                .collect()
        };
        let region = self
            .lock()
            .regions()
            .in_scope(&Scope::Span {
                path: key,
                span: Span { start: at, end: at },
            })
            .find(|region| declared.contains(&region.id))
            .map(|region| region.id)?;
        self.hunk_of(region)
    }

    /// The regions a block's id names (`T064`) — `8b`'s `S here marks all 12`.
    ///
    /// [`None`] when no such block exists, which is *"I do not know that
    /// block"* and refuses; an empty `Some` is *"that block's regions are all
    /// gone"* and marks nothing. Collapsing the two would make a typo look like
    /// a no-op.
    pub(crate) fn block_regions(&self, block: BlockId) -> Option<Vec<RegionId>> {
        let blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let found = blocks.iter().find(|candidate| candidate.id == block)?;
        Some(
            found
                .groups
                .iter()
                .flat_map(|group| group.regions.iter().map(|change| change.region))
                .collect(),
        )
    }

    /// **`annotate-group`** — claude's *"mechanical"* against *"the meat"*
    /// (`T065`).
    ///
    /// Answers whether the group existed. **Replaces rather than appends**: an
    /// annotation is claude's current sentence about a group, not a log of
    /// them, and `8b` draws one line per row. A second call is a revision.
    ///
    /// Not journalled, for [`Shared::blocks`]'s reason — a block is a
    /// statement, and seen-state is the only thing §7 persists.
    pub(crate) fn annotate_group(&self, group: GroupId, text: &str) -> bool {
        let mut blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(found) = blocks
            .iter_mut()
            .flat_map(|block| block.groups.iter_mut())
            .find(|candidate| candidate.id == group)
        else {
            return false;
        };
        // **Empty clears it.** A verb that could set an annotation and never
        // unset one would leave a wrong sentence on the row forever, and `8b`
        // draws groups with no annotation (`tests/`) so the absent case is
        // already a shape the surface has.
        found.annotation = (!text.is_empty()).then(|| text.to_owned());
        true
    }

    /// The regions a group's id names (`T064`). [`None`] and empty mean what
    /// they mean for [`Shared::block_regions`].
    pub(crate) fn group_regions(&self, group: GroupId) -> Option<Vec<RegionId>> {
        let blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        blocks
            .iter()
            .flat_map(|block| block.groups.iter())
            .find(|candidate| candidate.id == group)
            .map(|found| found.regions.iter().map(|change| change.region).collect())
    }

    /// The span of the lowest-id hunk covering a position (`T064`).
    ///
    /// `vih`'s answer, and [`Shared::covering`]'s lowest-id rule for its
    /// reason. **Seen hunks are included**, unlike `viu`'s: `vih` is *"the hunk
    /// here"* and a review surface's `s` has to be able to reach one you
    /// already marked in order to unmark it — which is exactly the asymmetry
    /// `viu` documents in the other direction.
    ///
    /// A region that no block ever named is not a hunk. That is what keeps
    /// `vih` from selecting an ordinary `declare-regions` marker, which would
    /// make the two nouns the same noun.
    pub(crate) fn hunk_covering(&self, path: &Path, at: Position) -> Option<Span> {
        let declared: Vec<RegionId> = {
            let blocks = self
                .blocks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            blocks
                .iter()
                .flat_map(|block| block.groups.iter())
                .flat_map(|group| group.regions.iter().map(|change| change.region))
                .collect()
        };
        let key = key_for(path);
        self.lock()
            .regions()
            .in_scope(&Scope::Span {
                path: key,
                span: Span { start: at, end: at },
            })
            .find(|region| declared.contains(&region.id))
            .map(|region| region.span)
    }

    /// Every declared block, oldest first — the `review-blocks` query
    /// (`T053`).
    pub(crate) fn blocks(&self) -> Vec<Block> {
        self.blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// **`mark-seen` and `mark-unseen`.** Answers how many regions were in
    /// scope.
    pub(crate) fn set_seen(&self, scope: &Scope, state: SeenState) -> usize {
        let (marked, records) = {
            let mut store = self.lock();
            let touched: Vec<RegionId> = store
                .regions()
                .in_scope(scope)
                .map(|region| region.id)
                .collect();
            let marked = store.set_seen(scope, state);
            let records = Self::rows_of(&store, &touched, &[]);
            (marked, records)
        };
        self.write(records);
        marked
    }

    /// **`drop-regions`.** Answers how many went.
    pub(crate) fn drop_regions(&self, scope: &Scope) -> usize {
        let (dropped, records) = {
            let mut store = self.lock();
            // The ids have to be read *before* the drop — afterwards there is
            // nothing left to name, and a tombstone needs a name.
            let doomed: Vec<RegionId> = store
                .regions()
                .in_scope(scope)
                .map(|region| region.id)
                .collect();
            let dropped = store.drop_regions(scope);
            let records = Self::rows_of(&store, &doomed, &[]);
            (dropped, records)
        };
        self.write(records);
        dropped
    }

    /// **`ingest-diagnostics`.**
    pub(crate) fn publish(&self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        self.lock().publish_diagnostics(path, diagnostics);
    }

    /// **`place-anchor`.** Answers the id (`T042`).
    pub(crate) fn place_anchor(
        &self,
        path: PathBuf,
        span: Span,
        label: Option<String>,
        fingerprint: Fingerprint,
    ) -> AnchorId {
        let (id, records) = {
            let mut store = self.lock();
            let id = store.place_anchor(path, span, label, fingerprint);
            // A labelled placement *displaces* whichever anchor held that label
            // — vim's rule, `ma` twice is one mark — so the displaced id needs
            // a tombstone or it comes back on the next restart. Reading the
            // whole file's anchors covers it without the store having to
            // report what it removed.
            let touched: Vec<AnchorId> = store.anchors().all().map(|anchor| anchor.id).collect();
            let records = Self::rows_of(&store, &[], &touched);
            (id, records)
        };
        self.write(records);
        id
    }

    /// **`reanchor`.** One file's anchors *and* regions, against its new text.
    pub(crate) fn reanchor(&self, path: &Path, snapshot: &Snapshot) -> Reanchored {
        let (outcome, records) = {
            let mut store = self.lock();
            let outcome = store.reanchor(path, snapshot);
            let anchors: Vec<AnchorId> =
                outcome.moved.iter().chain(&outcome.lost).copied().collect();
            // Every region in the file, not only the moved ones: a reanchor
            // also *fingerprints*, and a region that gained one without moving
            // has changed on disk even though its span has not.
            let regions: Vec<RegionId> = store
                .regions()
                .all()
                .filter(|region| region.path == path)
                .map(|region| region.id)
                .collect();
            let records = Self::rows_of(&store, &regions, &anchors);
            (outcome, records)
        };
        self.write(records);
        outcome
    }

    /// Describe a file to the store so its regions can find themselves again
    /// (`T043`). Answers how many gained a fingerprint.
    pub(crate) fn fingerprint_regions(&self, path: &Path, snapshot: &Snapshot) -> usize {
        let (filled, records) = {
            let mut store = self.lock();
            let filled = store.fingerprint_regions(path, snapshot);
            if filled == 0 {
                return 0;
            }
            let regions: Vec<RegionId> = store
                .regions()
                .all()
                .filter(|region| region.path == path)
                .map(|region| region.id)
                .collect();
            let records = Self::rows_of(&store, &regions, &[]);
            (filled, records)
        };
        self.write(records);
        filled
    }

    /// One anchor, cloned out from behind the lock.
    ///
    /// Cloned for the reason [`Shared::diagnostics_of`] gives: a caller holding
    /// a reference would be holding the lock, and the caller here is a
    /// keystroke arm that goes on to move the cursor.
    pub(crate) fn anchor(&self, id: AnchorId) -> Option<Anchor> {
        self.lock().anchors().get(id).cloned()
    }

    /// The anchor labelled `label` in `path` — `'{a-z}`'s lookup.
    pub(crate) fn labelled(&self, path: &Path, label: &str) -> Option<Anchor> {
        self.lock().anchors().labelled(path, label).cloned()
    }

    /// The `anchors` query: one file's, with the tier each resolved at.
    pub(crate) fn answer_anchors(&self, path: &Path) -> Vec<Value> {
        self.lock().answer_anchors(path)
    }

    /// The `anchor` query: one.
    pub(crate) fn answer_anchor(&self, id: AnchorId) -> Option<Value> {
        self.lock().answer_anchor(id)
    }

    /// One file's diagnostics, cloned out from behind the lock.
    ///
    /// Cloned because the frame holds it across a `&mut Editor` borrow —
    /// `DiagnosticsVm::rows` installs virtual text — and a guard held that long
    /// would be a lock held across a redraw.
    pub(crate) fn diagnostics_of(&self, path: &Path) -> Vec<Diagnostic> {
        self.lock().diagnostics().of(path).to_vec()
    }

    /// The `diagnostics` query.
    pub(crate) fn answer_diagnostics(&self, only: Option<&Path>) -> Vec<Value> {
        self.lock().answer_diagnostics(only)
    }

    /// The `regions` query.
    pub(crate) fn answer_regions(&self, lens: &Lens) -> Vec<Value> {
        self.lock().answer_regions(lens)
    }

    /// The `unseen-regions` query.
    pub(crate) fn answer_unseen(&self, path: Option<&Path>) -> Vec<Value> {
        let key = path.map(key_for);
        self.lock().answer_unseen(key.as_deref())
    }

    /// The `region` query. [`None`] for an id the store has never minted or has
    /// dropped, which the caller turns into the vocabulary's own refusal.
    pub(crate) fn answer_region(&self, id: RegionId) -> Option<Value> {
        self.lock().regions().get(id).map(Region::to_value)
    }

    /// The `next-region-by` query — `6b`'s `]r` (`T111`).
    ///
    /// **Ordered by (path, line, column) across the workspace, and it wraps.**
    /// The caller's `from` carries the focused file, because a bare `Position`
    /// cannot order regions that live in different files — the reasoning is at
    /// the arm in `main.rs`. Running off the end answers the *first* region by
    /// that author rather than nothing, which is what makes it a walk; `]u`
    /// already behaves this way and a walk that stopped at the last row would
    /// be the one motion in the editor that silently did nothing.
    ///
    /// [`Value::Null`] when the author has no regions at all, and when there is
    /// no focused file to walk from. Both are questions with a legitimate no.
    pub(crate) fn next_region_by(
        &self,
        author: Actor,
        from: Option<&(PathBuf, Position)>,
    ) -> Value {
        let Some((path, at)) = from else {
            return Value::Null;
        };
        let here = key_for(path);
        let held = self.lock();
        let lens = Lens {
            author: Some(author),
            ..Lens::everything()
        };
        // Collected and sorted rather than answered off the store's own order:
        // `Regions::matching` yields in id order, which is *declaration* order,
        // and a walk that followed the order claude happened to declare things
        // in would jump around the file.
        let mut ordered: Vec<&Region> = held.regions().matching(&lens).collect();
        ordered.sort_by(|left, right| {
            key_for(&left.path)
                .cmp(&key_for(&right.path))
                .then(left.span.start.line.cmp(&right.span.start.line))
                .then(left.span.start.column.cmp(&right.span.start.column))
        });
        let after = |region: &Region| {
            let path = key_for(&region.path);
            (path.as_path(), region.span.start.line, region.span.start.column)
                > (here.as_path(), at.line, at.column)
        };
        ordered
            .iter()
            .find(|region| after(region))
            .or_else(|| ordered.first())
            .map_or(Value::Null, |region| region.to_value())
    }

    /// The `unseen-count` query — the statusline's `●n`.
    pub(crate) fn unseen_count(&self, scope: &Scope) -> usize {
        self.lock().regions().unseen_count(scope)
    }

    /// The `seen-count` query.
    pub(crate) fn seen_count(&self, scope: &Scope) -> usize {
        self.lock().regions().seen_count(scope)
    }

    /// How many anchors it holds — `:arch`'s count (`T048`).
    pub(crate) fn anchor_count(&self) -> usize {
        self.lock().anchors().len()
    }

    /// How many diagnostics it holds, across every file.
    pub(crate) fn diagnostic_count(&self) -> usize {
        self.lock()
            .diagnostics()
            .files()
            .map(|(_, published)| published.len())
            .sum()
    }

    /// What every answer off this store is true at.
    pub(crate) fn revision(&self) -> Revision {
        self.lock().revision()
    }

    /// The ids of every region in a scope — what `set-virtual-text-visible`
    /// collapses a rail by.
    pub(crate) fn ids_in(&self, scope: &Scope) -> Vec<RegionId> {
        self.lock()
            .regions()
            .in_scope(scope)
            .map(|region| region.id)
            .collect()
    }

    /// The region covering a position, if one does. What gives a diagnostic's
    /// virtual-text row an owner — `phosphor_ui::diagnostics` has said since
    /// `T040` that *"a region id is the store's and there are no regions until
    /// `T041`, at which point a diagnostic's row is owned by the region
    /// anchored to its node"*.
    ///
    /// The lowest id when more than one covers it, so the answer does not
    /// depend on how the set happened to be iterated. `T042` makes this
    /// anchored rather than positional.
    pub(crate) fn covering(&self, path: &Path, at: Position) -> Option<RegionId> {
        let key = key_for(path);
        self.lock()
            .regions()
            .in_scope(&Scope::Span {
                path: key,
                span: Span { start: at, end: at },
            })
            .map(|region| region.id)
            .next()
    }

    /// The span of the lowest-id **unseen** region covering a position
    /// (`T049`).
    ///
    /// `viu`'s answer. Lowest id for [`Shared::covering`]'s reason — the answer
    /// must not depend on iteration order — and unseen only because `viu` is
    /// *"select the unseen region"*: a noun that also caught regions you had
    /// read would make `s` over it a no-op that looked like a bug.
    pub(crate) fn unseen_covering(&self, path: &Path, at: Position) -> Option<Span> {
        let key = key_for(path);
        self.lock()
            .regions()
            .in_scope(&Scope::Span {
                path: key,
                span: Span { start: at, end: at },
            })
            .find(|region| region.state.unseen())
            .map(|region| region.span)
    }

    /// One file's regions as spans, for the gutter.
    ///
    /// Answers `(span, seen)` pairs rather than the ui's own `RegionState`,
    /// because *which state a region contributes to the column* is a
    /// composition decision and belongs beside the diagnostics ladder in
    /// `main`, not behind a lock in here.
    pub(crate) fn spans_in(&self, path: &Path) -> Vec<(Span, SeenState)> {
        let key = key_for(path);
        self.lock()
            .regions()
            .in_scope(&Scope::File(key))
            .map(|region| (region.span, region.state))
            .collect()
    }

    /// The store, with a poisoned lock read through rather than panicked on —
    /// a region set is not worth taking the editor down for. The same call
    /// `crate::lsp::Diagnostics` made for the same reason.
    fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A path as the store keys it: workspace-relative where it is under the
/// working directory, and unchanged where it is not.
///
/// The same shape and the same argument as `crate::lsp::key_for`, one seam
/// over. A path outside the workspace keeps its absolute form, which is the
/// right answer for the same reason it is there: nothing else will ever name
/// that file, so the two sides still agree with each other.
///
/// **Both directions or neither.** Normalising only on the way in would leave
/// the loop looking up a key that never arrives, which is the exact failure
/// `lsp::key_for`'s doc records finding by pressing no key at all.
pub(crate) fn key_for(path: &Path) -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use phosphor_core::request::{Actor, Position, RegionSpec, Span};
    use phosphor_core::store::{Scope, SeenState};

    use super::{Hunk, Shared, key_for};

    fn span(from: u32, to: u32) -> Span {
        Span {
            start: Position {
                line: from,
                column: 1,
            },
            end: Position {
                line: to,
                column: 1,
            },
        }
    }

    /// A changed span with no before-side — `4b`'s `@@ 4`, a pure insertion.
    fn changed(from: u32, to: u32) -> phosphor_core::request::ChangedSpan {
        phosphor_core::request::ChangedSpan {
            span: span(from, to),
            was: None,
        }
    }

    fn claude(path: &str) -> RegionSpec {
        RegionSpec {
            path: path.into(),
            span: span(1, 3),
            author: Actor::Claude,
        }
    }

    /// A block over one file with three separate spans — three hunks.
    fn block(shared: &Shared) -> phosphor_core::request::BlockId {
        shared
            .declare_block(
                "retry logic",
                &[phosphor_core::request::FileGroup {
                    path: "src/fetch.rs".into(),
                    spans: vec![
                        changed(1, 2),
                        phosphor_core::request::ChangedSpan {
                            span: span(5, 6),
                            was: Some("    let resp = client.get(url).send()?;\n".to_owned()),
                        },
                        changed(9, 10),
                    ],
                    annotation: Some("the meat".to_owned()),
                }],
                None,
                Actor::Claude,
            )
            .id
    }

    /// **`T064`'s acceptance, at the store.** Marking one hunk seen leaves the
    /// rest unseen.
    /// **One delivery is one burst** (`T069`).
    ///
    /// The counter, held with no clock in it. `T069`'s entry calls debouncing
    /// load-bearing — *"an agent writing a file produces a burst of events, and
    /// one `✱` per burst is the honest signal"* — and this is the half of that
    /// claim this build owns: `notify-debouncer-full` decides what a burst *is*,
    /// and `note_disk_change` must count each one exactly once.
    ///
    /// **It lives here rather than in the pty suite deliberately.** The
    /// keyboard-driven version raced the debouncer's own window: it read 1 on
    /// one run and 2 on the next, and caught a planted `bursts += 2` once while
    /// missing it once. A test whose verdict depends on the machine is worse
    /// than no test.
    #[test]
    fn one_delivery_is_one_burst() {
        let shared = Shared::default();
        let path = std::path::Path::new("counted.txt");

        assert_eq!(shared.note_disk_change(path, Actor::Claude), 1);
        assert_eq!(shared.note_disk_change(path, Actor::Claude), 2);

        // **The actor is the first one's and does not drift.** Two writes from
        // different sources between one reload and the next is a case this
        // cannot describe honestly either way, and keeping the first is the one
        // that matches what the notice already said.
        assert_eq!(shared.note_disk_change(path, Actor::System), 3);
        assert_eq!(
            shared
                .disk_change(path)
                .expect("a change is recorded")
                .actor,
            Actor::Claude,
        );

        // Closing the gap forgets it, and forgetting it twice is not an error.
        assert!(shared.clear_disk_change(path));
        assert!(!shared.clear_disk_change(path));
        assert!(shared.disk_change(path).is_none());

        // And the count starts again rather than resuming, because the question
        // is *"how many since the buffer last agreed with disk"*.
        assert_eq!(shared.note_disk_change(path, Actor::Claude), 1);
    }

    #[test]
    fn marking_one_hunk_seen_leaves_the_rest_unseen() {
        let shared = Shared::default();
        let id = block(&shared);

        let before = shared.hunks(id);
        assert_eq!(before.len(), 3, "three spans declared, three hunks");
        assert!(before.iter().all(|hunk| !hunk.seen), "{before:?}");

        let one = before[1].clone();
        let marked = shared.set_seen(&Scope::One(Hunk::region_of(one.id)), SeenState::Seen);
        assert_eq!(marked, 1, "one region in scope, not three");

        let after = shared.hunks(id);
        let seen: Vec<bool> = after.iter().map(|hunk| hunk.seen).collect();
        assert_eq!(
            seen,
            vec![false, true, false],
            "the middle hunk and only it"
        );
        // And the ids did not move under it — a surface holding one across the
        // mark still names the same hunk.
        assert_eq!(
            after.iter().map(|hunk| hunk.id).collect::<Vec<_>>(),
            before.iter().map(|hunk| hunk.id).collect::<Vec<_>>()
        );
    }

    /// **The other direction, which is what a block target is for.** `8b`'s
    /// `S here marks all 12` over three.
    #[test]
    fn a_blocks_regions_are_every_hunk_in_it() {
        let shared = Shared::default();
        let id = block(&shared);

        let regions = shared.block_regions(id).expect("the block exists");
        assert_eq!(regions.len(), 3);
        assert_eq!(
            shared.set_seen(&Scope::These(regions), SeenState::Seen),
            3,
            "all three at once"
        );
        assert!(shared.hunks(id).iter().all(|hunk| hunk.seen));
    }

    /// **An id that names nothing is not an empty scope.** `None` refuses;
    /// `Some(empty)` marks nothing. Collapsing the two would make a typo look
    /// like a no-op — the hazard `Scope::These` documents.
    #[test]
    fn an_unknown_block_is_absent_rather_than_empty() {
        let shared = Shared::default();
        block(&shared);
        assert!(
            shared
                .block_regions(phosphor_core::request::BlockId(99))
                .is_none()
        );
        assert!(
            shared
                .group_regions(phosphor_core::request::GroupId(99))
                .is_none()
        );
        assert!(shared.hunks(phosphor_core::request::BlockId(99)).is_empty());
    }

    /// **A group is named by an id that outlives its position in a block.**
    /// Two blocks, and the second block's group does not reuse the first's id.
    #[test]
    fn group_ids_are_minted_across_blocks_not_within_one() {
        let shared = Shared::default();
        let first = block(&shared);
        let second = block(&shared);
        let ids: Vec<_> = shared
            .hunks(first)
            .iter()
            .chain(shared.hunks(second).iter())
            .map(|hunk| hunk.group)
            .collect();
        assert_eq!(ids[0], ids[1], "one file, one group");
        assert_ne!(
            ids[0], ids[3],
            "a second block's group is a different group"
        );
    }

    /// **A region two blocks both declared is a real case, and `hunks` and
    /// `hunk_of` answer it differently on purpose.**
    ///
    /// Found by `group_ids_are_minted_across_blocks_not_within_one`: a first
    /// version of `hunks(block)` went through `hunk_of`'s global scan, and a
    /// region two blocks shared came back attributed to whichever block
    /// declared it *first* — for both blocks' queries. `hunks(second)` has
    /// `second`'s own group in scope and does not need to guess; `hunk_of`
    /// takes an id with no block behind it and has nothing better than a
    /// convention.
    #[test]
    fn a_shared_region_belongs_to_each_blocks_own_group_and_to_the_first_for_hunk_of() {
        let shared = Shared::default();
        let first = shared
            .declare_block(
                "retry logic",
                &[phosphor_core::request::FileGroup {
                    path: "src/fetch.rs".into(),
                    spans: vec![changed(1, 2)],
                    annotation: None,
                }],
                None,
                Actor::Claude,
            )
            .id;
        // The same span, redeclared under a second block.
        let second = shared
            .declare_block(
                "second pass",
                &[phosphor_core::request::FileGroup {
                    path: "src/fetch.rs".into(),
                    spans: vec![changed(1, 2)],
                    annotation: None,
                }],
                None,
                Actor::Claude,
            )
            .id;

        let via_first = shared.hunks(first);
        let via_second = shared.hunks(second);
        assert_eq!(via_first.len(), 1);
        assert_eq!(via_second.len(), 1);
        assert_eq!(via_first[0].id, via_second[0].id, "one region, one hunk id");
        assert_ne!(
            via_first[0].group, via_second[0].group,
            "but each block reports its own group for it"
        );

        // `hunk_of` has no block to ask and takes the first declaring one —
        // stated as a convention in its own doc, and pinned here so a change to
        // it is a decision rather than a drift.
        let region = Hunk::region_of(via_first[0].id);
        assert_eq!(
            shared.hunk_of(region).map(|h| h.group),
            Some(via_first[0].group)
        );
    }

    /// **`T067`'s id codec round-trips, and that is the whole of what makes an
    /// inbox row nameable.**
    ///
    /// `open-inbox-item` and `Target::InboxItem` both take an `InboxId`, and the
    /// inbox is a view over three stores rather than a store of its own — so a
    /// row's identity has to survive the next call, which an index into the
    /// merged list would not.
    #[test]
    fn an_inbox_id_carries_the_source_it_names() {
        use super::InboxSource;

        // Every kind, at ids that would collide under a narrower encoding: note
        // 1 and ask 1 and block 1 are three different rows.
        let sources = [
            InboxSource::Ask(0),
            InboxSource::Ask(1),
            InboxSource::Block(phosphor_core::request::BlockId(1)),
            InboxSource::Note(1),
            InboxSource::Ask(u64::from(u32::MAX)),
            InboxSource::Note(7),
        ];
        for source in sources {
            assert_eq!(
                InboxSource::of(source.id()),
                Some(source),
                "{source:?} round-trips"
            );
        }

        // And the three kinds at the same ordinal are three different ids —
        // the property an index-based id would break.
        let one = [
            InboxSource::Ask(1).id(),
            InboxSource::Block(phosphor_core::request::BlockId(1)).id(),
            InboxSource::Note(1).id(),
        ];
        assert_ne!(one[0], one[1]);
        assert_ne!(one[1], one[2]);
        assert_ne!(one[0], one[2]);
    }

    /// **A note is the one inbox row with storage of its own**, and its seen bit
    /// is the fact rather than a copy of one.
    #[test]
    fn a_note_is_posted_read_and_unread_again() {
        let shared = Shared::default();
        let first = shared.notify(
            phosphor_core::request::Severity::Info,
            "bumped tokio to 1.41 for sleep jitter",
            None,
            None,
        );
        let second = shared.notify(
            phosphor_core::request::Severity::Trouble,
            "the websocket reconnect loop is hot",
            Some("it retries with no backoff"),
            None,
        );
        assert_ne!(first, second, "two notes, two ids");

        let notes = shared.notes();
        assert_eq!(notes.len(), 2, "oldest first");
        assert_eq!(notes[0].title, "bumped tokio to 1.41 for sleep jitter");
        assert!(notes.iter().all(|note| !note.seen), "posted unread");

        assert!(shared.set_note_seen(second, true));
        let after = shared.notes();
        assert!(!after[0].seen, "the other one is untouched");
        assert!(after[1].seen, "and this one is read");

        // Unread again — `5c`'s footer offers `s seen`, and a mark you cannot
        // undo is a mark you hesitate to make.
        assert!(shared.set_note_seen(second, false));
        assert!(!shared.notes()[1].seen);

        // An id that names nothing says so rather than marking the first note
        // it finds — `annotate_group`'s rule, one store over.
        assert!(!shared.set_note_seen(phosphor_core::request::InboxId(999), true));
    }

    /// **`3a`'s exchange: two actors, one thread, and resolve is not delete**
    /// (`T068`).
    #[test]
    fn a_thread_takes_both_sides_and_resolving_keeps_them() {
        let shared = Shared::default();
        let path = std::path::Path::new("src/retry.rs");
        let id = shared.start_thread(path, span(3, 4), Actor::You, "collapse these arms");

        // Claude's side arrives through the same verb with a different actor —
        // which is the *door*, not a field, and is what makes §7's "the machine
        // tracks claude" checkable rather than trusted.
        assert!(shared.reply_to_thread(id, Actor::Claude, "error carried in `last`"));
        let found = shared.thread_of(id).expect("the thread exists");
        assert_eq!(
            found.replies.len(),
            2,
            "oldest first, as a conversation reads"
        );
        assert_eq!(found.replies[0].actor, Actor::You);
        assert_eq!(found.replies[1].actor, Actor::Claude);
        assert!(!found.resolved);

        // **Resolve keeps the exchange.** `3a`'s record of *why* a line looks
        // the way it does outlives the conversation.
        assert!(shared.resolve_thread(id, true));
        let done = shared.thread_of(id).expect("still there");
        assert!(done.resolved);
        assert_eq!(done.replies.len(), 2, "resolving destroys nothing");

        // **A reply reopens it**, because an answer hidden behind "nobody is
        // talking" is an answer nobody reads.
        assert!(shared.reply_to_thread(id, Actor::You, "good catch"));
        assert!(!shared.thread_of(id).expect("still there").resolved);

        // Delete is the verb that removes it, and it is a different verb.
        assert!(shared.delete_thread(id));
        assert!(shared.thread_of(id).is_none());
        assert!(!shared.delete_thread(id), "and only once");
    }

    /// **`vit` finds a thread by its span, resolved or not** (`T068`).
    ///
    /// The resolved half matters: a noun that skipped finished threads would
    /// make `dit` after resolving one a delete of whatever else was on the
    /// line — `hunk_covering`'s ruling, one overlay over.
    #[test]
    fn a_thread_is_found_at_its_anchor_even_once_resolved() {
        let shared = Shared::default();
        let path = std::path::Path::new("src/retry.rs");
        let id = shared.start_thread(path, span(3, 4), Actor::You, "collapse these arms");

        let inside = Position { line: 3, column: 1 };
        let outside = Position { line: 9, column: 1 };
        assert_eq!(shared.thread_covering(path, inside), Some(span(3, 4)));
        assert_eq!(shared.thread_covering(path, outside), None);

        shared.resolve_thread(id, true);
        assert_eq!(
            shared.thread_covering(path, inside),
            Some(span(3, 4)),
            "a resolved thread is still a thread"
        );

        // And a thread in one file is not in another — `key_for`'s
        // reconciliation, the same as every other path in this module.
        assert!(
            shared
                .thread_covering(std::path::Path::new("src/fetch.rs"), inside)
                .is_none()
        );
        assert_eq!(shared.threads_in(path).len(), 1);
    }

    /// **Blocks and notes interleave by when they actually arrived, not by
    /// either one's own id counter** (`T067`).
    ///
    /// `BlockId` and a note's own `InboxId` are two counters that mint
    /// independently, so sorting rows on either alone is wrong the moment a
    /// block and a note arrive interleaved: a second block declared after a
    /// note has a *lower* `BlockId` than the note's, and would sort as older
    /// than something posted before it existed. `Shared::arrivals` is the one
    /// clock both stamp from.
    #[test]
    fn a_second_block_after_a_note_still_arrives_after_it() {
        let shared = Shared::default();
        // block 0
        block(&shared);
        let note = shared.notify(
            phosphor_core::request::Severity::Info,
            "bumped tokio to 1.41 for sleep jitter",
            None,
            None,
        );
        // block 1 — a higher `BlockId`, and it should also read as arriving
        // after the note, which a lower one would not.
        let second_block = shared
            .declare_block(
                "second pass",
                &[phosphor_core::request::FileGroup {
                    path: "src/fetch.rs".into(),
                    spans: vec![changed(1, 2)],
                    annotation: None,
                }],
                None,
                Actor::Claude,
            )
            .id;

        let blocks = shared.blocks();
        let by_id: std::collections::BTreeMap<phosphor_core::request::BlockId, u64> = blocks
            .iter()
            .map(|block| (block.id, block.arrival))
            .collect();
        let note_arrival = shared
            .notes()
            .into_iter()
            .find(|candidate| candidate.id == note)
            .expect("the note exists")
            .arrival;

        assert!(
            by_id[&second_block] > note_arrival,
            "the second block arrived after the note, on the shared clock"
        );
    }

    /// **`T065`'s verb.** Claude revises a group's annotation, and clearing it
    /// is a thing you can do.
    #[test]
    fn a_group_annotation_is_replaced_and_can_be_cleared() {
        let shared = Shared::default();
        let id = block(&shared);
        let group = shared.hunks(id)[0].group;

        // `declare-review-block` set one; this replaces it rather than
        // appending, because `8b` draws one line per row.
        assert!(shared.annotate_group(group, "mechanical: ? → map_err"));
        let annotated = shared.blocks()[0].groups[0].annotation.clone();
        assert_eq!(annotated.as_deref(), Some("mechanical: ? → map_err"));

        // **Empty clears it**, so a wrong sentence is not permanent.
        assert!(shared.annotate_group(group, ""));
        assert_eq!(shared.blocks()[0].groups[0].annotation, None);

        // And an id that names nothing says so rather than annotating the
        // first group it finds.
        assert!(!shared.annotate_group(phosphor_core::request::GroupId(99), "nope"));
        assert_eq!(shared.blocks()[0].groups[0].annotation, None);
    }

    /// **`vih` finds a declared hunk and not an ordinary marker**, which is
    /// what keeps `viu` and `vih` two nouns rather than one with a filter.
    #[test]
    fn only_a_block_declared_region_is_a_hunk() {
        let shared = Shared::default();
        // An ordinary `declare-regions` marker, on a line no block names.
        shared.declare(
            &[RegionSpec {
                path: "src/fetch.rs".into(),
                span: span(20, 21),
                author: Actor::Claude,
            }],
            Actor::Claude,
        );
        block(&shared);

        let path = std::path::Path::new("src/fetch.rs");
        let inside = Position { line: 5, column: 1 };
        let outside = Position {
            line: 20,
            column: 1,
        };
        assert_eq!(shared.hunk_covering(path, inside), Some(span(5, 6)));
        assert_eq!(
            shared.hunk_covering(path, outside),
            None,
            "a marker no block declared is not a hunk"
        );
        assert!(
            shared.unseen_covering(path, outside).is_some(),
            "but it is still an unseen region — `viu` reaches it"
        );
    }

    /// **A hunk stays a hunk after you mark it**, where an unseen region stops
    /// being one. Without this, `s` over a hunk would be one-way.
    #[test]
    fn a_seen_hunk_is_still_a_hunk_and_a_seen_region_is_not_unseen() {
        let shared = Shared::default();
        let id = block(&shared);
        let path = std::path::Path::new("src/fetch.rs");
        let at = Position { line: 5, column: 1 };
        assert!(shared.unseen_covering(path, at).is_some());

        let one = shared.hunks(id)[1].clone();
        shared.set_seen(&Scope::One(Hunk::region_of(one.id)), SeenState::Seen);

        assert_eq!(
            shared.hunk_covering(path, at),
            Some(span(5, 6)),
            "still reachable, so `s` can unmark it"
        );
        assert!(
            shared.unseen_covering(path, at).is_none(),
            "and `viu` no longer offers it"
        );
    }

    /// **The reconciliation this module exists for.** A door declares an
    /// absolute path under the working directory and the loop looks the file up
    /// by the relative one; both land on the same key, so the marker appears.
    #[test]
    fn an_absolute_declaration_is_found_by_its_workspace_relative_path() {
        let cwd = std::env::current_dir().expect("a working directory");
        let absolute = cwd.join("src/main.rs");
        let shared = Shared::default();
        shared.declare(
            &[RegionSpec {
                path: absolute.clone(),
                ..claude("unused")
            }],
            Actor::Claude,
        );

        assert_eq!(
            shared.spans_in(std::path::Path::new("src/main.rs")).len(),
            1,
            "the relative path finds it"
        );
        assert_eq!(
            shared.spans_in(&absolute).len(),
            1,
            "and so does the absolute one it was declared with"
        );
    }

    /// A path outside the workspace keeps its own form, and still agrees with
    /// itself.
    #[test]
    fn a_path_outside_the_workspace_keeps_its_absolute_form() {
        let outside = std::path::Path::new("/definitely/not/here/a.rs");
        assert_eq!(key_for(outside), outside);
        let shared = Shared::default();
        shared.declare(
            &[RegionSpec {
                path: outside.into(),
                ..claude("unused")
            }],
            Actor::Claude,
        );
        assert_eq!(shared.spans_in(outside).len(), 1);
    }

    /// The counts the statusline reads, narrowed to one file and to everywhere.
    #[test]
    fn the_counts_narrow_to_a_file_and_to_the_workspace() {
        let shared = Shared::default();
        shared.declare(&[claude("a.rs"), claude("b.rs")], Actor::Claude);
        assert_eq!(shared.unseen_count(&Scope::Everywhere), 2);
        assert_eq!(shared.unseen_count(&Scope::File("a.rs".into())), 1);
        assert_eq!(
            shared.set_seen(&Scope::File("a.rs".into()), SeenState::Seen),
            1
        );
        assert_eq!(shared.unseen_count(&Scope::Everywhere), 1);
        assert_eq!(shared.seen_count(&Scope::Everywhere), 1);
    }

    /// Every mutation moves one revision, so a cache reading it sees the
    /// diagnostics and the regions through the same number.
    #[test]
    fn one_revision_covers_the_regions_and_the_diagnostics() {
        let shared = Shared::default();
        let first = shared.revision();
        shared.declare(&[claude("a.rs")], Actor::Claude);
        let second = shared.revision();
        assert!(second.get() > first.get());
        shared.publish(
            "a.rs".into(),
            vec![phosphor_core::request::Diagnostic {
                span: span(1, 2),
                severity: phosphor_core::request::Severity::Trouble,
                message: "boom".to_owned(),
                source: None,
            }],
        );
        assert!(shared.revision().get() > second.get());
    }
}
