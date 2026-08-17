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

use phosphor_core::query::Revision;
use phosphor_core::request::{Actor, Diagnostic, RegionId, RegionSpec, Span};
use phosphor_core::store::{Declared, Lens, Region, Scope, SeenState, Store};
use phosphor_core::value::Value;

/// The store, shared.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    store: Mutex<Store>,
}

impl Shared {
    /// **`declare-regions`.**
    pub(crate) fn declare(&self, specs: &[RegionSpec], asked_by: Actor) -> Declared {
        let specs: Vec<RegionSpec> = specs
            .iter()
            .map(|spec| RegionSpec {
                path: key_for(&spec.path),
                ..spec.clone()
            })
            .collect();
        self.lock().declare_regions(&specs, asked_by)
    }

    /// **`mark-seen` and `mark-unseen`.** Answers how many regions were in
    /// scope.
    pub(crate) fn set_seen(&self, scope: &Scope, state: SeenState) -> usize {
        self.lock().set_seen(scope, state)
    }

    /// **`drop-regions`.** Answers how many went.
    pub(crate) fn drop_regions(&self, scope: &Scope) -> usize {
        self.lock().drop_regions(scope)
    }

    /// **`ingest-diagnostics`.**
    pub(crate) fn publish(&self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        self.lock().publish_diagnostics(path, diagnostics);
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
