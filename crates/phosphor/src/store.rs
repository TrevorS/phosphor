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

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phosphor_core::journal;
use phosphor_core::query::Revision;
use phosphor_core::request::{Actor, AnchorId, Diagnostic, RegionId, RegionSpec, Span};
use phosphor_core::store::{
    Anchor, Declared, Fingerprint, Lens, Reanchored, Region, Scope, SeenLog, SeenState, Snapshot,
    Store, persist,
};
use phosphor_core::value::Value;

/// The store, shared.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    store: Mutex<Store>,
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
    pub(crate) fn covering(
        &self,
        path: &Path,
        at: phosphor_core::request::Position,
    ) -> Option<RegionId> {
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

    use super::{Shared, key_for};

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

    fn claude(path: &str) -> RegionSpec {
        RegionSpec {
            path: path.into(),
            span: span(1, 3),
            author: Actor::Claude,
        }
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
