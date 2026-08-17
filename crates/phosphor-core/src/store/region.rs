//! Regions and seen-state — Design Language §7's one state machine (`T041`).
//!
//! *"claude writes → unseen `--s-->` seen"*, and *"claude revises → unseen
//! again"*. Seen-state is the only mutable flag the user owns; everything else
//! in this module derives from what a door declared.
//!
//! # Your own edits never create regions
//!
//! §7 is unconditional about it, and [`RegionSpec`]'s own doc says what the
//! store does with a declaration that claims otherwise: *"a declaration
//! claiming [`Actor::You`] is a no-op the store records rather than an error"*.
//! So [`Regions::declare`] counts it — [`Declared::ignored`] — and creates
//! nothing. A refusal would be wrong: the door did nothing illegal, it declared
//! something that is not a region.
//!
//! **Two actors are kept, not one.** [`Region::author`] is what the caller
//! *claimed*; [`Region::declared_by`] is the envelope's
//! [`Actor`], which is the door's own account of who
//! asked. `request.rs` says why the pair exists — *"any door can claim an
//! author, so the store keeps both — who asked, and what was claimed"* — and
//! this is the module that keeps it.
//!
//! # What identity a region has before anchors exist
//!
//! A revision has to find the region it revises, and `T042` is the task that
//! makes that precise: an anchor binds to a tree-sitter node and survives the
//! rewrite that moved it. Until then the only coordinates a declaration carries
//! are a path and a span, so **a declaration revises every region it overlaps
//! on the same path with the same claimed author**, absorbing them into one
//! whose span is the union.
//!
//! That rule is chosen rather than defaulted into, and the alternative is worse
//! in a way the mockups make visible. Treating every declaration as a new
//! region makes the count in `1a` — *"retry logic — 2 files · 6 regions"* —
//! grow every time claude touches the same function, so `●6` becomes `●9` for a
//! session that changed nothing new. Union keeps *"claude wrote here and you
//! have not looked"* true of every row that was ever covered, which is the
//! thesis the gutter is drawing.
//!
//! Half-open spans mean touching is **not** overlapping ([`overlaps`]); a
//! zero-width span is a point, and a point inside a span overlaps it, which is
//! what makes `s` at the cursor find the region under the cursor.
//!
//! Owned by `store`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::request::{Actor, Position, RegionId, RegionSpec, Span};
use crate::value::{Args, Value, Wire as _};

// ---------------------------------------------------------------------------
// The state machine
// ---------------------------------------------------------------------------

/// Where a region sits in §7's state machine.
///
/// Two states and no third: §7 draws `unseen --s--> seen` and calls its
/// overlays — ⚓ thread, ◉ watch, ■ diagnostic — orthogonal to it.
/// `phosphor_ui::gutter::RegionState` carries those overlays because the
/// *column* has to rank them against each other; nothing here does, because a
/// thread is not a state a region is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SeenState {
    /// Claude wrote it and you have not looked yet. Every region starts here,
    /// and a revision puts a seen one back.
    #[default]
    Unseen,
    /// You looked. §3's row 18: *"seen — marker cleared, line is plain"*.
    Seen,
}

impl SeenState {
    /// `true` for [`Self::Unseen`] — the predicate every count in this module
    /// is written against, named once so `!matches!(…)` never appears twice.
    #[must_use]
    pub const fn unseen(self) -> bool {
        matches!(self, Self::Unseen)
    }

    /// The tag the query answers this state as.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unseen => "unseen",
            Self::Seen => "seen",
        }
    }
}

/// One claude-authored span, and what you have done about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// The store's own id, stable across revisions of the same region.
    pub id: RegionId,
    /// Workspace-relative path, exactly as the declaring door spelled it. The
    /// store never interprets a path — a host that mixes relative and absolute
    /// forms sees two files, which is the same contract
    /// [`crate::store::diagnostics`] keeps.
    pub path: PathBuf,
    /// The span it covers, widened by every revision that overlapped it.
    pub span: Span,
    /// Who is *claimed* to have written it.
    pub author: Actor,
    /// Who asked — the envelope's actor, which no door can forge.
    pub declared_by: Actor,
    /// Seen or unseen.
    pub state: SeenState,
    /// How many declarations have landed on this region after the first.
    ///
    /// Not decoration: *"claude revises → unseen again"* is the one transition
    /// that is invisible from the outside — a region that was seen and is
    /// unseen again looks exactly like one that was never seen — and this is
    /// how a surface tells the two apart.
    pub revisions: u32,
}

impl Region {
    /// The record a `region` query answers.
    ///
    /// Hand-built rather than `wire_record!`: nothing decodes a `Region` from a
    /// door, so a `Wire` impl would be a decoder nobody calls plus a schema row
    /// claiming a store type is an argument.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Record(
            Args::new()
                .with("id", self.id.to_value())
                .with("path", Value::Text(self.path.display().to_string()))
                .with("span", self.span.to_value())
                .with("author", self.author.to_value())
                .with("declared-by", self.declared_by.to_value())
                .with("state", Value::Text(self.state.name().to_owned()))
                .with("revisions", Value::Int(i64::from(self.revisions))),
        )
    }
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// What a [`Target`](crate::request::Target) narrowed down to, once the host
/// has resolved everything focus-relative.
///
/// **The store has no cursor, and this type is why.** Four `Target` arms mean
/// something different depending on where focus is (`request.rs`), and a store
/// in `phosphor-core` has no editor to ask. So the host resolves those arms —
/// it is the only thing that can — and hands down coordinates. The arms it
/// cannot resolve yet belong to later tasks and refuse by their own row, which
/// keeps *"which targets work"* a fact about the vocabulary rather than a
/// silent gap in here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scope {
    /// Every region the store holds. `unseen-count` with no `within`.
    #[default]
    Everywhere,
    /// One file, whole.
    File(PathBuf),
    /// A span inside one file. A zero-width span is a point — the cursor.
    Span {
        /// Workspace-relative path.
        path: PathBuf,
        /// The span.
        span: Span,
    },
    /// One region, by id.
    One(RegionId),
}

impl Scope {
    /// Whether `region` is inside this scope.
    #[must_use]
    pub fn holds(&self, region: &Region) -> bool {
        match self {
            Self::Everywhere => true,
            Self::File(path) => region.path == *path,
            Self::Span { path, span } => region.path == *path && overlaps(*span, region.span),
            Self::One(id) => region.id == *id,
        }
    }
}

/// A [`RegionFilter`](crate::request::RegionFilter) with its `within` already
/// resolved. See [`Scope`] for why the resolution happens above the store.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lens {
    /// Only regions claiming this author.
    pub author: Option<Actor>,
    /// Only unseen ones.
    pub unseen_only: bool,
    /// Only inside this scope.
    pub within: Scope,
}

impl Lens {
    /// Everything, unfiltered.
    #[must_use]
    pub fn everything() -> Self {
        Self::default()
    }

    /// Whether `region` passes.
    #[must_use]
    pub fn admits(&self, region: &Region) -> bool {
        self.author.is_none_or(|author| region.author == author)
            && (!self.unseen_only || region.state.unseen())
            && self.within.holds(region)
    }
}

// ---------------------------------------------------------------------------
// Declaring
// ---------------------------------------------------------------------------

/// What one `declare-regions` did.
///
/// Three counts rather than one, because they mean three different things to
/// the caller and collapsing them is how a door learns nothing: `created` is
/// new work to look at, `revised` is work you may already have marked seen and
/// now have to look at again, and `ignored` is a declaration §7 does not let
/// become a region at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declared {
    /// Regions that did not exist before.
    pub created: Vec<RegionId>,
    /// Regions an overlapping declaration put back to
    /// [`SeenState::Unseen`].
    pub revised: Vec<RegionId>,
    /// Declarations claiming an author §7 does not create regions for.
    pub ignored: usize,
}

impl Declared {
    /// Whether anything about the store changed — the predicate the revision
    /// counter is bumped on.
    #[must_use]
    pub fn moved(&self) -> bool {
        !self.created.is_empty() || !self.revised.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The set
// ---------------------------------------------------------------------------

/// Every region the store holds.
///
/// A `BTreeMap` rather than a `HashMap`: `regions` answers a list, and a list
/// that reshuffles between two identical reads is a diff nobody can read. The
/// same argument [`crate::store::diagnostics`] records for its own ordering,
/// and here it costs nothing — ids are allocated in order, so iteration is
/// declaration order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Regions {
    by_id: BTreeMap<RegionId, Region>,
    next: u64,
}

impl Regions {
    /// Nothing declared yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// **The arm of `declare-regions`.**
    ///
    /// `asked_by` is the envelope's actor and is recorded on every region this
    /// touches; the *claimed* author comes from each [`RegionSpec`] and is what
    /// §7's rule is applied to.
    pub fn declare(&mut self, specs: &[RegionSpec], asked_by: Actor) -> Declared {
        let mut declared = Declared::default();
        for spec in specs {
            // §7: the machine tracks claude only. Recorded, not refused —
            // see the module header.
            if spec.author != Actor::Claude {
                declared.ignored += 1;
                continue;
            }
            let overlapping: Vec<RegionId> = self
                .by_id
                .values()
                .filter(|region| {
                    region.path == spec.path
                        && region.author == spec.author
                        && overlaps(region.span, spec.span)
                })
                .map(|region| region.id)
                .collect();

            let Some((&keep, absorbed)) = overlapping.split_first() else {
                let id = self.mint();
                self.by_id.insert(
                    id,
                    Region {
                        id,
                        path: spec.path.clone(),
                        span: spec.span,
                        author: spec.author,
                        declared_by: asked_by,
                        state: SeenState::Unseen,
                        revisions: 0,
                    },
                );
                declared.created.push(id);
                continue;
            };

            // The union of everything this declaration reached, so no row that
            // was covered stops being covered. `split_first` gives the lowest
            // id because the map iterates in key order, and the lowest id is
            // the one a surface has been showing longest.
            let mut span = spec.span;
            for id in absorbed {
                if let Some(gone) = self.by_id.remove(id) {
                    span = union(span, gone.span);
                }
            }
            if let Some(region) = self.by_id.get_mut(&keep) {
                span = union(span, region.span);
                region.span = span;
                region.declared_by = asked_by;
                region.state = SeenState::Unseen;
                region.revisions = region.revisions.saturating_add(1);
            }
            declared.revised.push(keep);
        }
        declared
    }

    /// **The arms of `mark-seen` and `mark-unseen`.** Answers how many regions
    /// were in scope.
    ///
    /// In scope, not changed: `s` on a line with no region has to answer `0` so
    /// nothing on screen claims something happened, and marking an already-seen
    /// region seen is not nothing — it is the user saying so about a region
    /// that is there. A caller that needs "did anything move" has
    /// [`Self::revision_moved`].
    pub fn set_state(&mut self, scope: &Scope, state: SeenState) -> usize {
        let ids: Vec<RegionId> = self.in_scope(scope).map(|region| region.id).collect();
        for id in &ids {
            if let Some(region) = self.by_id.get_mut(id) {
                region.state = state;
            }
        }
        ids.len()
    }

    /// Whether setting every region in `scope` to `state` would change one.
    ///
    /// Read before [`Self::set_state`] by a caller that owns a revision
    /// counter, so a no-op mark does not invalidate a frame cache.
    #[must_use]
    pub fn revision_moved(&self, scope: &Scope, state: SeenState) -> bool {
        self.in_scope(scope).any(|region| region.state != state)
    }

    /// **The arm of `drop-regions`.** Answers how many went.
    pub fn drop_in(&mut self, scope: &Scope) -> usize {
        let ids: Vec<RegionId> = self.in_scope(scope).map(|region| region.id).collect();
        for id in &ids {
            self.by_id.remove(id);
        }
        ids.len()
    }

    /// Every region in a scope, in id order.
    pub fn in_scope<'a>(&'a self, scope: &'a Scope) -> impl Iterator<Item = &'a Region> {
        self.by_id
            .values()
            .filter(move |region| scope.holds(region))
    }

    /// Every region a lens admits, in id order.
    pub fn matching<'a>(&'a self, lens: &'a Lens) -> impl Iterator<Item = &'a Region> {
        self.by_id
            .values()
            .filter(move |region| lens.admits(region))
    }

    /// One file's unseen regions — `6b`'s first line.
    pub fn unseen_in<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a Region> {
        self.by_id
            .values()
            .filter(move |region| region.path == path && region.state.unseen())
    }

    /// One region.
    #[must_use]
    pub fn get(&self, id: RegionId) -> Option<&Region> {
        self.by_id.get(&id)
    }

    /// How many regions in scope are unseen — the statusline's `●n`.
    #[must_use]
    pub fn unseen_count(&self, scope: &Scope) -> usize {
        self.in_scope(scope)
            .filter(|region| region.state.unseen())
            .count()
    }

    /// How many regions in scope are seen.
    #[must_use]
    pub fn seen_count(&self, scope: &Scope) -> usize {
        self.in_scope(scope)
            .filter(|region| !region.state.unseen())
            .count()
    }

    /// Every region, in id order. The `regions` query with no filter.
    pub fn all(&self) -> impl Iterator<Item = &Region> {
        self.by_id.values()
    }

    /// How many regions the store holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the store holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// The next id. Monotonic and never reused — a surface holding a dropped
    /// region's id must get nothing back, not somebody else's region.
    fn mint(&mut self) -> RegionId {
        self.next += 1;
        RegionId(self.next)
    }
}

// ---------------------------------------------------------------------------
// Span arithmetic
// ---------------------------------------------------------------------------

/// Whether two spans overlap.
///
/// Half-open, so touching is not overlapping: a region ending at `10:2` and one
/// starting there are two regions. The exception is a **zero-width** span,
/// which is a point rather than a range — `Cursor` resolves to one — and a
/// point inside `[start, end)` overlaps it. Without that case `s` at the cursor
/// would find nothing, because no zero-width span overlaps anything at all
/// under the plain rule, including itself.
#[must_use]
pub fn overlaps(a: Span, b: Span) -> bool {
    if a.start == a.end {
        return holds_point(b, a.start);
    }
    if b.start == b.end {
        return holds_point(a, b.start);
    }
    a.start < b.end && b.start < a.end
}

/// Whether `at` is inside `[span.start, span.end)`.
fn holds_point(span: Span, at: Position) -> bool {
    span.start <= at && at < span.end
}

/// The smallest span covering both.
#[must_use]
pub fn union(a: Span, b: Span) -> Span {
    Span {
        start: a.start.min(b.start),
        end: a.end.max(b.end),
    }
}

#[cfg(test)]
mod tests {
    use super::{Declared, Lens, Region, Regions, Scope, SeenState, overlaps, union};
    use crate::request::{Actor, Position, RegionId, RegionSpec, Span};

    fn at(line: u32, column: u32) -> Position {
        Position { line, column }
    }

    fn span(from: (u32, u32), to: (u32, u32)) -> Span {
        Span {
            start: at(from.0, from.1),
            end: at(to.0, to.1),
        }
    }

    fn spec(path: &str, from: (u32, u32), to: (u32, u32), author: Actor) -> RegionSpec {
        RegionSpec {
            path: path.into(),
            span: span(from, to),
            author,
        }
    }

    /// `fixtures/seed/plan.scm`'s first three spans, verbatim — the ones
    /// `1a` counts as *"retry logic — 2 files · 6 regions"*.
    fn claude(path: &str, from: (u32, u32), to: (u32, u32)) -> RegionSpec {
        spec(path, from, to, Actor::Claude)
    }

    fn seeded() -> Regions {
        let mut regions = Regions::new();
        regions.declare(
            &[
                claude("src/retry.rs", (4, 1), (4, 18)),
                claude("src/retry.rs", (6, 1), (10, 2)),
                claude("src/retry.rs", (12, 1), (24, 51)),
                claude("src/fetch.rs", (10, 1), (14, 2)),
                claude("src/fetch.rs", (17, 1), (20, 2)),
                claude("src/fetch.rs", (31, 1), (35, 2)),
            ],
            Actor::Cli,
        );
        regions
    }

    // -----------------------------------------------------------------------
    // The state machine, exhaustively
    // -----------------------------------------------------------------------

    /// Every region starts unseen. §7's entry edge, and the whole reason the
    /// gutter has anything to draw.
    #[test]
    fn a_declared_region_starts_unseen() {
        let regions = seeded();
        assert_eq!(regions.len(), 6);
        assert!(regions.all().all(|region| region.state.unseen()));
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 6);
        assert_eq!(regions.seen_count(&Scope::Everywhere), 0);
    }

    /// `unseen --s--> seen`, and the count follows.
    #[test]
    fn marking_a_region_seen_moves_it_and_the_count() {
        let mut regions = seeded();
        let marked = regions.set_state(
            &Scope::Span {
                path: "src/retry.rs".into(),
                span: span((6, 1), (10, 2)),
            },
            SeenState::Seen,
        );
        assert_eq!(marked, 1);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 5);
        assert_eq!(regions.seen_count(&Scope::Everywhere), 1);
    }

    /// **The transition this task exists to get right.** A seen region that
    /// claude writes over is unseen again, and it is the *same* region — same
    /// id, one more revision — not a seventh one beside it.
    #[test]
    fn claude_revising_a_seen_region_makes_it_unseen_again() {
        let mut regions = seeded();
        let target = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((6, 1), (10, 2)),
        };
        regions.set_state(&target, SeenState::Seen);
        let before: Vec<RegionId> = regions.all().map(|region| region.id).collect();

        let declared = regions.declare(&[claude("src/retry.rs", (6, 1), (10, 2))], Actor::Claude);

        assert!(declared.created.is_empty(), "no new region for a rewrite");
        assert_eq!(declared.revised.len(), 1);
        assert_eq!(
            regions.all().map(|region| region.id).collect::<Vec<_>>(),
            before,
            "the id set is unchanged — a revision is not a new region"
        );
        let revised = regions.get(declared.revised[0]).expect("still there");
        assert!(revised.state.unseen(), "a revision is unseen again");
        assert_eq!(revised.revisions, 1);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 6);
    }

    /// The one transition a count cannot see: a region that was seen and is
    /// unseen again reads exactly like one nobody ever looked at, so the
    /// revision count is what tells them apart.
    #[test]
    fn a_revised_region_is_distinguishable_from_one_never_seen() {
        let mut regions = seeded();
        let target = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((6, 1), (10, 2)),
        };
        regions.set_state(&target, SeenState::Seen);
        regions.declare(&[claude("src/retry.rs", (6, 1), (10, 2))], Actor::Claude);

        let first = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((4, 1), (4, 18)),
        };
        let revised = regions.in_scope(&target).next().expect("one region");
        let untouched = regions.in_scope(&first).next().expect("one region");
        assert_eq!(revised.state, untouched.state, "both read as unseen");
        assert_eq!(revised.revisions, 1);
        assert_eq!(untouched.revisions, 0);
    }

    /// `mark-unseen` is the same edge run by hand — §7 gives the user the flag,
    /// and giving it back is part of owning it.
    #[test]
    fn marking_a_region_unseen_by_hand_is_the_same_edge() {
        let mut regions = seeded();
        let scope = Scope::File("src/fetch.rs".into());
        assert_eq!(regions.set_state(&scope, SeenState::Seen), 3);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 3);
        assert_eq!(regions.set_state(&scope, SeenState::Unseen), 3);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 6);
    }

    /// Every ordered pair of states, run as a transition. Exhaustive by
    /// construction over a two-state machine: four pairs, and the assertion is
    /// that the state afterwards is the one asked for regardless of the one
    /// before, which is what makes `set_state` idempotent.
    #[test]
    fn every_transition_in_the_two_state_machine_lands_where_it_was_sent() {
        for before in [SeenState::Unseen, SeenState::Seen] {
            for after in [SeenState::Unseen, SeenState::Seen] {
                let mut regions = Regions::new();
                regions.declare(&[claude("a.rs", (1, 1), (2, 1))], Actor::Claude);
                let everywhere = Scope::Everywhere;
                regions.set_state(&everywhere, before);
                regions.set_state(&everywhere, after);
                let region = regions.all().next().expect("one region");
                assert_eq!(
                    region.state, after,
                    "{before:?} -> {after:?} must end at {after:?}"
                );
                assert_eq!(region.revisions, 0, "marking is not revising");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Your own edits never create regions
    // -----------------------------------------------------------------------

    /// §7, unconditionally. Recorded rather than refused — the door did
    /// nothing illegal.
    #[test]
    fn your_own_edits_never_create_regions() {
        let mut regions = Regions::new();
        let declared = regions.declare(
            &[
                spec("a.rs", (1, 1), (2, 1), Actor::You),
                spec("a.rs", (3, 1), (4, 1), Actor::Steel),
                spec("a.rs", (5, 1), (6, 1), Actor::Cli),
                spec("a.rs", (7, 1), (8, 1), Actor::System),
                claude("a.rs", (9, 1), (10, 1)),
            ],
            Actor::You,
        );
        assert_eq!(declared.ignored, 4, "four non-claude authors, four no-ops");
        assert_eq!(declared.created.len(), 1);
        assert_eq!(regions.len(), 1, "only claude's became a region");
        assert_eq!(regions.all().next().expect("one").author, Actor::Claude);
    }

    /// A batch of nothing but your own edits moves the store not at all, so a
    /// caller that bumps a revision on [`Declared::moved`] does not bump one.
    #[test]
    fn a_declaration_of_only_your_own_edits_moves_nothing() {
        let mut regions = Regions::new();
        let declared = regions.declare(&[spec("a.rs", (1, 1), (2, 1), Actor::You)], Actor::You);
        assert!(!declared.moved());
        assert!(regions.is_empty());
    }

    /// The claimed author and the envelope's are both kept. `request.rs`:
    /// *"any door can claim an author, so the store keeps both"*.
    #[test]
    fn the_store_keeps_both_who_asked_and_what_was_claimed() {
        let mut regions = Regions::new();
        regions.declare(&[claude("a.rs", (1, 1), (2, 1))], Actor::Cli);
        let region = regions.all().next().expect("one region");
        assert_eq!(region.author, Actor::Claude, "what was claimed");
        assert_eq!(region.declared_by, Actor::Cli, "who actually asked");
    }

    // -----------------------------------------------------------------------
    // Revision identity
    // -----------------------------------------------------------------------

    /// A declaration that reaches two existing regions absorbs both into one.
    /// Without this the count inflates every time claude rewrites across a
    /// boundary it already wrote.
    #[test]
    fn one_declaration_over_two_regions_absorbs_them_into_one() {
        let mut regions = Regions::new();
        regions.declare(
            &[
                claude("a.rs", (1, 1), (3, 1)),
                claude("a.rs", (5, 1), (7, 1)),
            ],
            Actor::Claude,
        );
        assert_eq!(regions.len(), 2);

        let declared = regions.declare(&[claude("a.rs", (2, 1), (6, 1))], Actor::Claude);
        assert_eq!(regions.len(), 1, "one rewrite over both is one region");
        assert_eq!(declared.revised.len(), 1);
        let region = regions.all().next().expect("one region");
        assert_eq!(
            region.span,
            span((1, 1), (7, 1)),
            "the union, so no covered row stops being covered"
        );
        assert_eq!(region.id, RegionId(1), "the oldest id survives");
    }

    /// Touching is not overlapping: half-open spans that meet are two
    /// regions, not one.
    #[test]
    fn two_spans_that_only_touch_stay_two_regions() {
        let mut regions = Regions::new();
        regions.declare(
            &[
                claude("a.rs", (1, 1), (3, 1)),
                claude("a.rs", (3, 1), (5, 1)),
            ],
            Actor::Claude,
        );
        assert_eq!(regions.len(), 2);
    }

    /// A revision widens rather than replaces, so a rewrite of the middle of a
    /// region does not shrink it and lose the rows around it.
    #[test]
    fn a_narrower_rewrite_does_not_shrink_the_region_it_lands_in() {
        let mut regions = Regions::new();
        regions.declare(&[claude("a.rs", (1, 1), (20, 1))], Actor::Claude);
        regions.declare(&[claude("a.rs", (5, 1), (6, 1))], Actor::Claude);
        let region = regions.all().next().expect("one region");
        assert_eq!(region.span, span((1, 1), (20, 1)));
    }

    /// Same span, different file: two regions. The path is part of identity
    /// and the store never interprets it.
    #[test]
    fn the_same_span_in_two_files_is_two_regions() {
        let mut regions = Regions::new();
        regions.declare(
            &[
                claude("a.rs", (1, 1), (3, 1)),
                claude("b.rs", (1, 1), (3, 1)),
            ],
            Actor::Claude,
        );
        assert_eq!(regions.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Scope and lens
    // -----------------------------------------------------------------------

    /// A zero-width scope is a point — the cursor — and finds the region it is
    /// inside. This is what makes `s` work.
    #[test]
    fn a_cursor_sized_scope_finds_the_region_under_it() {
        let regions = seeded();
        let cursor = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((8, 3), (8, 3)),
        };
        let found: Vec<_> = regions.in_scope(&cursor).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].span, span((6, 1), (10, 2)));
    }

    /// A point on a line no region covers finds nothing, so `s` there answers
    /// zero rather than marking the nearest thing.
    #[test]
    fn a_cursor_outside_every_region_finds_nothing() {
        let mut regions = seeded();
        let cursor = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((5, 1), (5, 1)),
        };
        assert_eq!(regions.set_state(&cursor, SeenState::Seen), 0);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 6);
    }

    /// A point at the very end of a span is outside it — half-open, and the
    /// same rule the span arithmetic keeps everywhere else.
    #[test]
    fn a_point_at_a_spans_end_is_outside_it() {
        let regions = seeded();
        let end = Scope::Span {
            path: "src/retry.rs".into(),
            span: span((10, 2), (10, 2)),
        };
        assert_eq!(regions.in_scope(&end).count(), 0);
    }

    /// Scoping by file, by region id, and by nothing at all.
    #[test]
    fn every_scope_arm_narrows_to_what_it_names() {
        let regions = seeded();
        assert_eq!(regions.in_scope(&Scope::Everywhere).count(), 6);
        assert_eq!(
            regions
                .in_scope(&Scope::File("src/fetch.rs".into()))
                .count(),
            3
        );
        assert_eq!(regions.in_scope(&Scope::One(RegionId(2))).count(), 1);
        assert_eq!(regions.in_scope(&Scope::One(RegionId(99))).count(), 0);
        assert_eq!(regions.in_scope(&Scope::File("nope.rs".into())).count(), 0);
    }

    /// The lens narrows on all three axes at once.
    #[test]
    fn the_lens_narrows_by_author_state_and_scope_together() {
        let mut regions = seeded();
        regions.set_state(&Scope::File("src/fetch.rs".into()), SeenState::Seen);

        assert_eq!(regions.matching(&Lens::everything()).count(), 6);
        assert_eq!(
            regions
                .matching(&Lens {
                    unseen_only: true,
                    ..Lens::everything()
                })
                .count(),
            3
        );
        assert_eq!(
            regions
                .matching(&Lens {
                    author: Some(Actor::You),
                    ..Lens::everything()
                })
                .count(),
            0,
            "no region has a non-claude author to find"
        );
        assert_eq!(
            regions
                .matching(&Lens {
                    unseen_only: true,
                    within: Scope::File("src/fetch.rs".into()),
                    ..Lens::everything()
                })
                .count(),
            0
        );
    }

    /// `unseen-regions` for one file, which is `6b`'s first line.
    #[test]
    fn unseen_in_answers_one_files_unseen_regions() {
        let mut regions = seeded();
        regions.set_state(
            &Scope::Span {
                path: "src/retry.rs".into(),
                span: span((4, 1), (4, 18)),
            },
            SeenState::Seen,
        );
        assert_eq!(
            regions
                .unseen_in(std::path::Path::new("src/retry.rs"))
                .count(),
            2
        );
        assert_eq!(
            regions
                .unseen_in(std::path::Path::new("src/fetch.rs"))
                .count(),
            3
        );
    }

    // -----------------------------------------------------------------------
    // Dropping
    // -----------------------------------------------------------------------

    /// A deleted file's regions go, and nothing else does.
    #[test]
    fn dropping_a_files_regions_leaves_the_other_files_alone() {
        let mut regions = seeded();
        assert_eq!(regions.drop_in(&Scope::File("src/fetch.rs".into())), 3);
        assert_eq!(regions.len(), 3);
        assert!(
            regions
                .all()
                .all(|region| region.path == std::path::Path::new("src/retry.rs"))
        );
    }

    /// An id is never reused. A surface holding a dropped region's id gets
    /// nothing back rather than somebody else's region.
    #[test]
    fn a_dropped_regions_id_is_never_handed_out_again() {
        let mut regions = Regions::new();
        regions.declare(&[claude("a.rs", (1, 1), (2, 1))], Actor::Claude);
        let first = regions.all().next().expect("one region").id;
        assert_eq!(regions.drop_in(&Scope::One(first)), 1);
        regions.declare(&[claude("a.rs", (1, 1), (2, 1))], Actor::Claude);
        let second = regions.all().next().expect("one region").id;
        assert_ne!(first, second);
        assert!(regions.get(first).is_none());
    }

    // -----------------------------------------------------------------------
    // The revision predicate
    // -----------------------------------------------------------------------

    /// Marking seen twice moves the store once, so a frame cache is not
    /// invalidated by a key that changed nothing.
    #[test]
    fn marking_the_same_region_seen_twice_moves_the_store_once() {
        let mut regions = seeded();
        let scope = Scope::File("src/fetch.rs".into());
        assert!(regions.revision_moved(&scope, SeenState::Seen));
        regions.set_state(&scope, SeenState::Seen);
        assert!(!regions.revision_moved(&scope, SeenState::Seen));
    }

    // -----------------------------------------------------------------------
    // Span arithmetic
    // -----------------------------------------------------------------------

    /// Half-open, both directions, and the zero-width exception.
    #[test]
    fn overlap_is_half_open_except_for_a_point() {
        let a = span((1, 1), (3, 1));
        assert!(overlaps(a, span((2, 1), (4, 1))));
        assert!(overlaps(span((2, 1), (4, 1)), a));
        assert!(
            !overlaps(a, span((3, 1), (5, 1))),
            "touching is not overlap"
        );
        assert!(!overlaps(span((3, 1), (5, 1)), a));
        assert!(overlaps(a, span((1, 1), (1, 1))), "a point at the start");
        assert!(overlaps(a, span((2, 5), (2, 5))), "a point inside");
        assert!(!overlaps(a, span((3, 1), (3, 1))), "a point at the end");
        assert!(
            !overlaps(span((1, 1), (1, 1)), span((2, 1), (2, 1))),
            "two different points never meet"
        );
    }

    /// The union is the smallest span covering both, in either order.
    #[test]
    fn the_union_covers_both_spans_whichever_way_round() {
        let a = span((1, 1), (3, 1));
        let b = span((2, 1), (7, 4));
        assert_eq!(union(a, b), span((1, 1), (7, 4)));
        assert_eq!(union(b, a), span((1, 1), (7, 4)));
    }

    // -----------------------------------------------------------------------
    // The answer shape
    // -----------------------------------------------------------------------

    /// Every field a surface needs is on the record, and the state is a tag
    /// rather than a bool — `6b` reads `unseen`, not `#true`.
    #[test]
    fn a_region_answers_as_a_record_naming_its_state() {
        let mut regions = seeded();
        regions.set_state(&Scope::One(RegionId(1)), SeenState::Seen);
        let region = regions.get(RegionId(1)).expect("one region");
        let crate::value::Value::Record(fields) = region.to_value() else {
            panic!("a region answers as a record");
        };
        assert_eq!(
            fields.get("state"),
            Some(&crate::value::Value::Text("seen".to_owned()))
        );
        assert_eq!(
            fields.get("path"),
            Some(&crate::value::Value::Text("src/retry.rs".to_owned()))
        );
        assert_eq!(
            fields.get("author"),
            Some(&crate::value::Value::Text("claude".to_owned()))
        );
        assert!(fields.get("span").is_some());
        assert!(fields.get("id").is_some());
        assert_eq!(fields.get("revisions"), Some(&crate::value::Value::Int(0)));
    }

    /// The default is a store with nothing in it, and every count agrees.
    #[test]
    fn an_empty_store_counts_zero_everywhere() {
        let regions = Regions::new();
        assert!(regions.is_empty());
        assert_eq!(regions.len(), 0);
        assert_eq!(regions.unseen_count(&Scope::Everywhere), 0);
        assert_eq!(regions.seen_count(&Scope::Everywhere), 0);
        assert_eq!(regions.all().count(), 0);
        assert!(!Declared::default().moved());
    }

    /// A `Region` is comparable, which is what lets a test assert a whole set
    /// rather than field by field.
    #[test]
    fn regions_compare_by_value() {
        let mut regions = Regions::new();
        regions.declare(&[claude("a.rs", (1, 1), (2, 1))], Actor::Claude);
        let held: Region = regions.all().next().expect("one region").clone();
        assert_eq!(regions.get(held.id), Some(&held));
    }
}
