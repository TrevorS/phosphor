//! The semantic store — regions, seen-state, anchors, threads, watches, inbox,
//! review blocks. Every surface is a query over this (invariant 4), and this is
//! the only module that mutates.
//!
//! Not part of the crate's face to `phosphor-ui`: `T007` fails CI on a `store::`
//! import from that crate. Owned by `store` from `CP-2`.
//!
//! # What `T041` folded in
//!
//! [`diagnostics`] held real state before this task and said so — *"this is not
//! the store, and it is deliberately too small to become one … when `T041`
//! lands, this map is what folds into it"*. It has. [`Store`] owns it now,
//! beside [`region::Regions`], behind one [`Revision`] that both move.
//!
//! **The fold was not cosmetic.** Nothing outside that module imported it: the
//! binary had its own `BTreeMap<PathBuf, Vec<Diagnostic>>` in
//! `crates/phosphor/src/lsp.rs` with its own `replace`/`of`/`answer`, written
//! at `T040` because it needed a `Mutex` and this crate holds no locks. So the
//! documented store and the real one were two maps with one name, and the
//! header promising the fold was the only thing connecting them. One store
//! with two handles is the shape `lsp.rs` argued for and now gets literally.
//!
//! # No lock in here
//!
//! [`Store`] is plain data. Sharing is the host's problem, because the host is
//! the thing that knows whether it has one thread or a VM running on another —
//! `phosphor-core` deciding for it would put a `Mutex` on the crate floor for
//! every consumer including the ones that only decode.

pub mod anchor;
pub mod diagnostics;
pub mod region;

use std::path::Path;

use crate::query::Revision;
use crate::request::{Actor, Diagnostic, RegionSpec};
use crate::value::Value;

pub use self::anchor::{Anchor, Anchors, Fingerprint, Reanchored, Snapshot, SyntaxStep, Tier};
pub use self::diagnostics::Diagnostics;
pub use self::region::{Declared, Lens, Region, Regions, Scope, SeenState};

/// The store handle. Mutation goes through `&mut Store`, and that is precisely
/// the API `phosphor-ui` must never hold.
///
/// # The revision is the point of the wrapper
///
/// Every method here that can change what a query answers bumps
/// [`Store::revision`], and none of the sub-stores can — they are plain
/// collections and have no counter. That is what makes *"the revision moved, so
/// re-run the composition"* a property of the type rather than of every caller
/// remembering. `crates/phosphor/src/main.rs` answered `Revision::INITIAL` to
/// every query until this task with the note *"the store has no revision until
/// `T041` and a number invented here would be one a cache could trust
/// wrongly"*; this is that number.
///
/// **Bumped only when something moved.** A `mark-seen` over a region that is
/// already seen answers how many regions it found — the user's `s` did land on
/// something — and does not move the revision, because nothing a query can see
/// changed. `T079`'s cache is the reader that would otherwise redraw on every
/// keystroke that marked an already-marked region.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Store {
    regions: Regions,
    diagnostics: Diagnostics,
    anchors: Anchors,
    revision: Revision,
}

impl Store {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What every [`Answer`](crate::query::Answer) off this store is true at.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// The regions, read-only. Every query in the `region` domain goes through
    /// here.
    #[must_use]
    pub const fn regions(&self) -> &Regions {
        &self.regions
    }

    /// The diagnostics, read-only.
    #[must_use]
    pub const fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// The anchors, read-only. Every query in the `anchor` domain goes through
    /// here.
    #[must_use]
    pub const fn anchors(&self) -> &Anchors {
        &self.anchors
    }

    // -----------------------------------------------------------------------
    // The arms
    // -----------------------------------------------------------------------

    /// **`declare-regions`.** `asked_by` is the envelope's actor.
    pub fn declare_regions(&mut self, specs: &[RegionSpec], asked_by: Actor) -> Declared {
        let declared = self.regions.declare(specs, asked_by);
        if declared.moved() {
            self.moved();
        }
        declared
    }

    /// **`mark-seen` and `mark-unseen`.** Answers how many regions were in
    /// scope — see [`Regions::set_state`] for why that is the number and not
    /// how many changed.
    pub fn set_seen(&mut self, scope: &Scope, state: SeenState) -> usize {
        if self.regions.revision_moved(scope, state) {
            self.moved();
        }
        self.regions.set_state(scope, state)
    }

    /// **`drop-regions`.** Answers how many went.
    pub fn drop_regions(&mut self, scope: &Scope) -> usize {
        let dropped = self.regions.drop_in(scope);
        if dropped > 0 {
            self.moved();
        }
        dropped
    }

    /// **`ingest-diagnostics`.** Replaces one file's whole set — the protocol's
    /// own rule, restated at [`Diagnostics::ingest`].
    ///
    /// The comparison is against the set *after* ingest rather than the vector
    /// handed in, because [`Diagnostics::ingest`] sorts: a server that
    /// republishes the same two errors in the other order has published no
    /// news, and comparing against its own ordering would say it had.
    pub fn publish_diagnostics(&mut self, path: std::path::PathBuf, published: Vec<Diagnostic>) {
        let before = self.diagnostics.of(&path).to_vec();
        self.diagnostics.ingest(path.clone(), published);
        if before != self.diagnostics.of(&path) {
            self.moved();
        }
    }

    /// **`place-anchor`.** Answers the id, which is what `m` writes down.
    ///
    /// Always moves the revision: a new anchor is a new row in the `anchors`
    /// query, and unlike `mark-seen` there is no already-in-that-state case.
    pub fn place_anchor(
        &mut self,
        path: std::path::PathBuf,
        span: crate::request::Span,
        label: Option<String>,
        fingerprint: Fingerprint,
    ) -> crate::request::AnchorId {
        let id = self.anchors.place(path, span, label, fingerprint);
        self.moved();
        id
    }

    /// **`reanchor`.** Re-resolves one file's anchors *and its regions* against
    /// its rewritten text, node tier then line tier.
    ///
    /// **Both row types, one ladder, one call.** `T043`'s acceptance is that
    /// markers work on a file with no grammar, and a marker is a region — so a
    /// reanchor that moved only the anchors would leave every unseen marker
    /// behind on the line it used to be on. They share [`anchor::resolve`]
    /// rather than each having a copy, which is what stops *"node tier, then
    /// line, then lost"* from meaning two different things.
    ///
    /// Regions are fingerprinted here too, on the way past: a region declared
    /// before anyone described the file has no way to find itself, and this is
    /// the first moment the store is told what the file looks like. Filling
    /// only what is missing is deliberate — see [`Regions::fingerprint_in`].
    ///
    /// Moves the revision only when something actually moved or was lost — a
    /// rewrite that leaves everything where it was is not news, and `T079`'s
    /// cache is the reader that would otherwise redraw on every save.
    pub fn reanchor(&mut self, path: &Path, snapshot: &Snapshot) -> Reanchored {
        let outcome = self.anchors.reanchor(path, snapshot);
        let regions_moved = self.regions.reanchor_in(path, snapshot);
        self.regions.fingerprint_in(path, snapshot);
        if outcome.changed() || regions_moved > 0 {
            self.moved();
        }
        outcome
    }

    /// Describe a file to the store, so the regions in it can find themselves
    /// again later (`T043`).
    ///
    /// The host calls this after declaring regions for a file it has open. It
    /// is separate from [`Store::declare_regions`] because a `RegionSpec` is a
    /// wire type carrying a path and a span — a fingerprint needs the file's
    /// *text*, and the store has none.
    ///
    /// Answers how many regions gained one. Never moves the revision: nothing a
    /// query can see changed, which is the same rule `mark-seen` over an
    /// already-seen region follows.
    pub fn fingerprint_regions(&mut self, path: &Path, snapshot: &Snapshot) -> usize {
        self.regions.fingerprint_in(path, snapshot)
    }

    /// Forget one file's anchors — a deleted file, a closed buffer.
    pub fn drop_anchors(&mut self, path: &Path) -> usize {
        let dropped = self.anchors.drop_in(path);
        if dropped > 0 {
            self.moved();
        }
        dropped
    }

    // -----------------------------------------------------------------------
    // Answers
    // -----------------------------------------------------------------------

    /// The `anchors` query: one file's anchors and the tier each resolved at.
    #[must_use]
    pub fn answer_anchors(&self, path: &Path) -> Vec<Value> {
        self.anchors.in_file(path).map(Anchor::to_value).collect()
    }

    /// The `anchor` query: one anchor.
    #[must_use]
    pub fn answer_anchor(&self, id: crate::request::AnchorId) -> Option<Value> {
        self.anchors.get(id).map(Anchor::to_value)
    }

    /// The `regions` query: every region a lens admits.
    #[must_use]
    pub fn answer_regions(&self, lens: &Lens) -> Vec<Value> {
        self.regions.matching(lens).map(Region::to_value).collect()
    }

    /// The `unseen-regions` query: one file's, or everywhere's.
    #[must_use]
    pub fn answer_unseen(&self, path: Option<&Path>) -> Vec<Value> {
        match path {
            Some(path) => self.regions.unseen_in(path).map(Region::to_value).collect(),
            None => self.answer_regions(&Lens {
                unseen_only: true,
                ..Lens::everything()
            }),
        }
    }

    /// The `diagnostics` query: every diagnostic, or one file's.
    ///
    /// Each record is the [`Diagnostic`] itself with its `path` added, because
    /// the query may answer for every file at once and a record that did not
    /// say which file it was about would be unreadable in that shape.
    #[must_use]
    pub fn answer_diagnostics(&self, only: Option<&Path>) -> Vec<Value> {
        use crate::value::{Args, Wire as _};

        self.diagnostics
            .files()
            .filter(|(path, _)| only.is_none_or(|wanted| wanted == *path))
            .flat_map(|(path, published)| {
                published.iter().map(move |diagnostic| {
                    let mut args =
                        Args::new().with("path", Value::Text(path.display().to_string()));
                    if let Value::Record(fields) = diagnostic.to_value() {
                        for (field, value) in fields.into_pairs() {
                            args.set(&field, value);
                        }
                    }
                    Value::Record(args)
                })
            })
            .collect()
    }

    fn moved(&mut self) {
        self.revision = self.revision.next();
    }
}

#[cfg(test)]
mod tests {
    use super::{Lens, Scope, SeenState, Store};
    use crate::request::{Actor, Position, RegionSpec, Severity, Span};

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

    fn claude(path: &str, from: u32, to: u32) -> RegionSpec {
        RegionSpec {
            path: path.into(),
            span: span(from, to),
            author: Actor::Claude,
        }
    }

    fn diagnostic(message: &str) -> crate::request::Diagnostic {
        crate::request::Diagnostic {
            span: span(1, 2),
            severity: Severity::Trouble,
            message: message.to_owned(),
            source: None,
        }
    }

    /// A declaration that created something moves the revision; one that
    /// created nothing does not.
    #[test]
    fn the_revision_moves_only_when_something_did() {
        let mut store = Store::new();
        let first = store.revision();
        store.declare_regions(&[claude("a.rs", 1, 3)], Actor::Claude);
        let after = store.revision();
        assert!(after.get() > first.get());

        store.declare_regions(
            &[RegionSpec {
                path: "a.rs".into(),
                span: span(9, 10),
                author: Actor::You,
            }],
            Actor::You,
        );
        assert_eq!(
            store.revision(),
            after,
            "a declaration §7 ignores changes nothing a query can see"
        );
    }

    /// Marking a region seen moves it once. The second `s` on the same region
    /// still answers `1` — it found a region — and moves nothing.
    #[test]
    fn a_second_mark_seen_answers_one_and_moves_the_revision_none() {
        let mut store = Store::new();
        store.declare_regions(&[claude("a.rs", 1, 3)], Actor::Claude);
        let before = store.revision();
        assert_eq!(store.set_seen(&Scope::Everywhere, SeenState::Seen), 1);
        let after = store.revision();
        assert!(after.get() > before.get());
        assert_eq!(
            store.set_seen(&Scope::Everywhere, SeenState::Seen),
            1,
            "it still found the region"
        );
        assert_eq!(after, store.revision(), "and nothing moved");
    }

    /// Dropping moves the revision; dropping nothing does not.
    #[test]
    fn dropping_moves_the_revision_only_when_it_dropped_something() {
        let mut store = Store::new();
        store.declare_regions(&[claude("a.rs", 1, 3)], Actor::Claude);
        let before = store.revision();
        assert_eq!(store.drop_regions(&Scope::File("b.rs".into())), 0);
        assert_eq!(store.revision(), before);
        assert_eq!(store.drop_regions(&Scope::File("a.rs".into())), 1);
        assert!(store.revision().get() > before.get());
    }

    /// The folded-in map is behind the same revision as the regions, which is
    /// the whole reason to fold it in rather than keep two.
    #[test]
    fn a_publish_moves_the_same_revision_the_regions_do() {
        let mut store = Store::new();
        let before = store.revision();
        store.publish_diagnostics("a.rs".into(), vec![diagnostic("boom")]);
        let after = store.revision();
        assert!(after.get() > before.get());
        store.publish_diagnostics("a.rs".into(), vec![diagnostic("boom")]);
        assert_eq!(
            store.revision(),
            after,
            "republishing the identical set is not news"
        );
        assert_eq!(
            store.diagnostics().of(std::path::Path::new("a.rs")).len(),
            1
        );
    }

    /// The two query shapes, over a store with both files seeded.
    #[test]
    fn the_query_answers_narrow_the_way_their_arguments_say() {
        let mut store = Store::new();
        store.declare_regions(
            &[
                claude("a.rs", 1, 3),
                claude("a.rs", 5, 7),
                claude("b.rs", 1, 3),
            ],
            Actor::Claude,
        );
        store.set_seen(&Scope::File("a.rs".into()), SeenState::Seen);

        assert_eq!(store.answer_regions(&Lens::everything()).len(), 3);
        assert_eq!(store.answer_unseen(None).len(), 1);
        assert_eq!(
            store
                .answer_unseen(Some(std::path::Path::new("a.rs")))
                .len(),
            0
        );
        assert_eq!(
            store
                .answer_unseen(Some(std::path::Path::new("b.rs")))
                .len(),
            1
        );
    }
}
