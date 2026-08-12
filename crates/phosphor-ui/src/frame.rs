//! The frame cache (`T079`) — **the reason a pre-1.0 scheme VM can sit under a
//! terminal UI.**
//!
//! Q12, verbatim: *"Evaluation runs at the rate of state change, not the rate of
//! frames. Rust caches the last view tree and redraws it every frame without
//! re-entering the VM; Steel re-runs only when a ViewModel actually changes. A
//! transcript streaming at 60fps costs one VM invocation per chunk, not sixty
//! per second."*
//!
//! This module is that sentence, and nothing else. It holds one
//! [`Tree`] and the [`Revision`] it was composed at; [`FrameCache::update`]
//! calls the composer **only** when the revision has moved. The composer is an
//! `FnOnce` rather than a trait object on purpose: this crate cannot name Steel
//! (`scripts/lint-no-store-mutation.sh` check 2 — a UI crate's only `phosphor-*`
//! dependency is `phosphor-core`), and it does not need to. Whether the closure
//! enters a VM, reads a `.scm` file, or builds the tree in Rust is the app
//! layer's business; what is guaranteed here is *how often it is called*.
//!
//! # The two counters
//!
//! [`CacheStats`] separates them because `CP-2` gates on the separation:
//!
//! * [`CacheStats::compositions`] — VM invocations. Bumped once per cache miss.
//! * [`CacheStats::frames`] — frames. Every [`FrameCache::update`] call, hit or
//!   miss, because a frame that draws the cached tree is still a frame.
//!
//! The acceptance criterion is that the first stays flat while the second
//! climbs. `benches/frame_cache.rs` measures it against a control arm that
//! composes every frame; `the_vm_does_not_enter_the_frame_path` below asserts
//! the same property deterministically, so `just test` protects it even when
//! nobody runs a benchmark.
//!
//! # What is *not* invalidated by a frame
//!
//! [`Node::Spinner`](phosphor_core::view::Node::Spinner) and
//! [`Node::Elapsed`](phosphor_core::view::Node::Elapsed) carry a
//! [`Millis`](phosphor_core::view::Millis) mark, not a pre-rendered string, and
//! `crate::interpret` renders the difference against the frame's own clock. So
//! an animating spinner produces *different pixels every frame from the same
//! cached tree* — which is the case that would otherwise force a recomposition
//! 12 times a second for nothing. `crate::interpret`'s
//! `an_animation_redraws_from_the_cached_tree` is that case as a test.
//!
//! Owned by `spine`.

use phosphor_core::query::Revision;
use phosphor_core::view::Tree;

/// The two counters `CP-2` reads, plus enough to tell a hit from a miss.
///
/// Cheap to copy and cheap to log: the host can print one of these on `q`
/// without keeping a second set of numbers of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// **VM invocations.** One per cache miss — that is, one per state change
    /// the cache was asked about, never one per frame.
    pub compositions: u64,
    /// Frames drawn from the cached tree without re-entering the composer.
    pub hits: u64,
}

impl CacheStats {
    /// Frames: every [`FrameCache::update`] call, hit or miss.
    ///
    /// Counts *update calls*, not `draw` calls — a host that redraws without
    /// asking the cache (a bare resize repaint, say) is not counted here, and
    /// that is the honest reading: this number exists to be divided into
    /// [`compositions`](CacheStats::compositions).
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.hits + self.compositions
    }

    /// Frames per composition — the ratio the benchmark reports. `None` before
    /// the first composition.
    #[must_use]
    pub fn frames_per_composition(&self) -> Option<f64> {
        (self.compositions > 0).then(|| self.frames() as f64 / self.compositions as f64)
    }
}

/// One view tree, and the revision it was true at.
///
/// Built once and held for the life of the app. The tree it hands back is
/// always drawable: before the first composition it is
/// [`Tree::default`] — `Node::Empty`, which draws nothing — so there is no
/// "no frame yet" state for a caller to handle.
#[derive(Debug, Default)]
pub struct FrameCache {
    tree: Tree,
    /// `None` before the first composition, and after [`FrameCache::invalidate`].
    seen: Option<Revision>,
    stats: CacheStats,
}

impl FrameCache {
    /// An empty cache. The first [`update`](FrameCache::update) always misses.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The tree to draw this frame. Never stale in a way a caller can observe:
    /// it is whatever the last successful composition produced.
    #[must_use]
    pub const fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The revision the cached tree was composed at, or `None` before the first
    /// composition.
    #[must_use]
    pub const fn revision(&self) -> Option<Revision> {
        self.seen
    }

    /// The two counters.
    #[must_use]
    pub const fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Force the next [`update`](FrameCache::update) to recompose, whatever the
    /// revision says.
    ///
    /// For the one case a revision cannot express: the *composition itself*
    /// changed rather than the state it reads — a `(define-statusline …)` at
    /// the REPL rebinding the composer while every store projection stands
    /// still. This is what `T022`'s REPL will need after an evaluation that
    /// redefined anything; without it, a redefinition would not appear until the next
    /// unrelated edit, which is exactly the "does it take effect on the very
    /// next frame?" question `CP-2`'s manual half asks.
    pub fn invalidate(&mut self) {
        self.seen = None;
    }

    /// **The whole point.** Recompose if `revision` has moved; otherwise keep
    /// the cached tree and do not call `compose`.
    ///
    /// Call once per frame. Returns `true` if this frame recomposed.
    ///
    /// Any change of revision is a miss, in either direction: a revision is a
    /// change counter and equality is the only question worth asking of it
    /// (`query.rs`'s own note on [`Revision`]).
    pub fn update(&mut self, revision: Revision, compose: impl FnOnce() -> Tree) -> bool {
        if self.hit(revision) {
            return false;
        }
        self.tree = compose();
        true
    }

    /// [`update`](FrameCache::update) for a composer that can fail.
    ///
    /// A failing composition **keeps the last good tree on screen** and still
    /// records the revision as seen, so a broken redefinition costs one VM
    /// invocation rather than one per frame until someone fixes it. That is the
    /// frame-path half of the same rule `T021` applies at boot: *"a broken
    /// `init.scm` boots the editor anyway, with the error in a float"* — the
    /// error surfaces as a float the host opens, never as a blank screen.
    ///
    /// # Errors
    ///
    /// Whatever the composer returned. The cache is left consistent either way.
    pub fn try_update<E>(
        &mut self,
        revision: Revision,
        compose: impl FnOnce() -> Result<Tree, E>,
    ) -> Result<bool, E> {
        if self.hit(revision) {
            return Ok(false);
        }
        self.tree = compose()?;
        Ok(true)
    }

    /// The hit/miss decision and the two counters, in the one place both
    /// entry points share. Returns `true` on a hit; on a miss it has already
    /// recorded the revision, so a composer that then fails is not retried
    /// every frame.
    fn hit(&mut self, revision: Revision) -> bool {
        if self.seen == Some(revision) {
            self.stats.hits += 1;
            return true;
        }
        self.stats.compositions += 1;
        self.seen = Some(revision);
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{CacheStats, FrameCache};
    use phosphor_core::query::Revision;
    use phosphor_core::view::{Axis, Constraint, Emphasis, Node, Slot, Tone, Tree};

    /// A tree with a little shape to it, tagged by the revision that produced
    /// it so a stale draw is visible in an assertion.
    fn tree_at(revision: u64) -> Tree {
        Tree::new(Node::split(
            Axis::Rows,
            [Slot::new(
                Constraint::Fill { weight: 1 },
                Node::Label {
                    text: format!("chunk {revision}"),
                    tone: Tone::Prose,
                    emphasis: Emphasis::Plain,
                },
            )],
        ))
    }

    fn revision(n: u64) -> Revision {
        let mut r = Revision::INITIAL;
        for _ in 0..n {
            r = r.next();
        }
        r
    }

    /// **`T079`'s acceptance criterion, as an assertion instead of a
    /// benchmark.**
    ///
    /// 10 000 frames over 37 state changes: the composer runs 37 times, not
    /// 10 000. The benchmark measures the same property in frames per second;
    /// this is the version that fails CI.
    #[test]
    fn the_vm_does_not_enter_the_frame_path() {
        const FRAMES: u64 = 10_000;
        const CHANGES: u64 = 37;

        let mut cache = FrameCache::new();
        let mut invocations = 0u64;

        for frame in 0..FRAMES {
            // A change every FRAMES/CHANGES frames — a stream at a fixed rate
            // under a frame loop running far faster than it.
            let state = frame * CHANGES / FRAMES;
            cache.update(revision(state), || {
                invocations += 1;
                tree_at(state)
            });
        }

        assert_eq!(invocations, CHANGES, "one composition per state change");
        assert_eq!(cache.stats().compositions, invocations);
        assert_eq!(cache.stats().frames(), FRAMES);
        assert_eq!(cache.stats().hits, FRAMES - CHANGES);
    }

    /// Frames climb, VM invocations do not. The benchmark's shape, in one test:
    /// the same 37 state changes drawn at four frame counts.
    #[test]
    fn compositions_are_flat_as_frames_climb() {
        const CHANGES: u64 = 37;
        let mut compositions = Vec::new();

        for frames in [1_000u64, 2_000, 4_000, 8_000] {
            let mut cache = FrameCache::new();
            for frame in 0..frames {
                let state = frame * CHANGES / frames;
                cache.update(revision(state), || tree_at(state));
            }
            assert_eq!(cache.stats().frames(), frames);
            compositions.push(cache.stats().compositions);
        }

        assert_eq!(
            compositions,
            vec![CHANGES; 4],
            "eight times the frames must not cost eight times the VM"
        );
    }

    #[test]
    fn an_unmoved_revision_never_recomposes() {
        let mut cache = FrameCache::new();
        let mut invocations = 0;
        for _ in 0..64 {
            cache.update(revision(9), || {
                invocations += 1;
                tree_at(9)
            });
        }
        assert_eq!(invocations, 1);
        assert_eq!(cache.tree(), &tree_at(9));
    }

    #[test]
    fn invalidate_recomposes_at_the_same_revision() {
        // The REPL case: the composer changed, the store did not.
        let mut cache = FrameCache::new();
        cache.update(revision(4), || tree_at(4));
        cache.update(revision(4), || tree_at(4));
        assert_eq!(cache.stats().compositions, 1);

        cache.invalidate();
        assert!(cache.update(revision(4), || tree_at(99)));
        assert_eq!(cache.stats().compositions, 2);
        assert_eq!(cache.tree(), &tree_at(99), "the redefinition is on screen");
    }

    #[test]
    fn a_broken_composition_keeps_the_last_good_frame() {
        let mut cache = FrameCache::new();
        cache.update(revision(1), || tree_at(1));

        let failed: Result<bool, &str> =
            cache.try_update(revision(2), || Err("unbound: statusline"));
        assert_eq!(failed, Err("unbound: statusline"));
        assert_eq!(
            cache.tree(),
            &tree_at(1),
            "a broken redefinition must not blank the screen"
        );

        // And it costs one invocation, not one per frame, until the state moves.
        let mut retries = 0;
        for _ in 0..100 {
            let _ = cache.try_update(revision(2), || {
                retries += 1;
                Err::<Tree, &str>("unbound: statusline")
            });
        }
        assert_eq!(
            retries, 0,
            "a failed composition is not retried every frame"
        );
    }

    #[test]
    fn a_fresh_cache_draws_the_empty_tree() {
        let cache = FrameCache::new();
        assert_eq!(cache.tree(), &Tree::default());
        assert_eq!(cache.revision(), None);
        assert_eq!(cache.stats(), CacheStats::default());
        assert_eq!(cache.stats().frames_per_composition(), None);
    }

    #[test]
    fn the_ratio_is_frames_over_compositions() {
        let mut cache = FrameCache::new();
        for frame in 0..100u64 {
            cache.update(revision(frame / 10), || tree_at(frame / 10));
        }
        assert_eq!(cache.stats().compositions, 10);
        assert_eq!(cache.stats().frames(), 100);
        assert_eq!(cache.stats().frames_per_composition(), Some(10.0));
    }
}
