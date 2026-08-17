//! Anchors — locations that survive the rewrite that moves them (`T042`,
//! `T043`).
//!
//! A region, a thread, a watch and a vim mark are all the same problem wearing
//! four hats: *"this place in this file"*, asserted before an edit and still
//! meant after one. A byte offset does not survive; a line number does not
//! survive; what survives is a description of the location good enough to find
//! it again.
//!
//! # The ladder
//!
//! Two tiers, tried in order, and a third state for honest failure:
//!
//! ```text
//!   Tier::Node   the named-construct chain — "retry, in impl Backoff"   (T042)
//!   Tier::Line   the line's own text, nearest the line it was on        (T043)
//!   Tier::Lost   neither matched; the span is left where it was
//! ```
//!
//! **The line tier is the floor, not a degraded extra.** That is `T043`'s own
//! framing and it is what makes markers a *store* feature rather than a
//! language feature (invariant 4): a `.env`, a `Makefile` and a file with no
//! extension at all get anchors that work, because the tier that catches them
//! is the one that needs no grammar. The node tier is an optimisation over it —
//! a better answer when a grammar happens to be loaded — and the ladder is
//! written in that order so nothing has to ask whether a language resolved.
//!
//! # Why the store does not parse
//!
//! `phosphor-core` has no dependencies, deliberately, and tree-sitter is not
//! about to be its first. So the host — which owns the buffer and the fork that
//! keeps its tree current — hands down a [`Snapshot`]: the file's lines, each
//! with the syntax path covering it. Matching is pure data over that, here.
//!
//! This is the same shape [`crate::language::Languages::new`] uses for grammar
//! names and [`Scope`](super::Scope) uses for the cursor: *the host resolves
//! what only the host can, and the store is handed coordinates.* It is what
//! keeps the ladder testable without a parser — every test in this module
//! builds its `Snapshot` by hand.
//!
//! # What an anchor is not
//!
//! It is **not** a rename-follower. A path names a construct, and renaming the
//! construct makes a different one; an anchor that followed the rename would
//! claim someone had seen code they had not. It falls to the line tier, and
//! then to [`Tier::Lost`], which is the truthful answer.
//!
//! It is **not** unique per line. Two anchors inside one function share a
//! syntax path, and the node tier says so rather than inventing precision the
//! grammar does not give — which is exactly why the line tier runs underneath
//! it and why [`resolve`] prefers a candidate whose *text* also matches.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::request::{AnchorId, Position, Span};
use crate::value::{Args, Value, Wire as _};

// ---------------------------------------------------------------------------
// The tier
// ---------------------------------------------------------------------------

/// Which rung of the ladder an anchor last resolved at.
///
/// Carried on the anchor rather than recomputed, because *"what tier did this
/// resolve at"* is a question the `anchors` query answers and a surface draws:
/// an anchor that fell to [`Tier::Line`] is still working, and one that reached
/// [`Tier::Lost`] is a location nobody can be sent to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Tier {
    /// Matched a named-construct chain. Requires a grammar.
    #[default]
    Node,
    /// Matched the line's own text. The floor, and grammar-free (`T043`).
    Line,
    /// Neither tier matched. The span is left where it was, and saying so is
    /// the point — a silently-moved anchor is worse than a stale one.
    Lost,
}

impl Tier {
    /// The wire spelling.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Line => "line",
            Self::Lost => "lost",
        }
    }

    /// Whether an anchor at this tier can still be jumped to.
    #[must_use]
    pub fn resolves(self) -> bool {
        !matches!(self, Self::Lost)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// The fingerprint
// ---------------------------------------------------------------------------

/// One step of a syntax path: a named construct's kind, and what names it.
///
/// The store's own type rather than the fork's, because the store may not
/// depend on the fork — the host converts at the seam. Deliberately two owned
/// `String`s: a fingerprint outlives every tree it was read from, which is the
/// entire reason it exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SyntaxStep {
    /// The grammar's node kind — `function_item`, `class_definition`.
    pub kind: String,
    /// What identifies it — a name, or a trait-and-type pair.
    pub name: String,
}

impl SyntaxStep {
    /// A step, from anything string-shaped.
    pub fn new(kind: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
        }
    }

    /// The record the `anchors` query draws.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Record(
            Args::new()
                .with("kind", Value::Text(self.kind.clone()))
                .with("name", Value::Text(self.name.clone())),
        )
    }
}

/// Everything remembered about *where* an anchor was, so it can be found again.
///
/// Both tiers are captured at placement time, always — not the node tier with
/// the line tier derived on demand. The file that gets rewritten is not
/// available to derive from later, and a fingerprint that could only be
/// completed while the original text still existed would be no fingerprint at
/// all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// The named-construct chain covering the location, outermost first.
    ///
    /// Empty is ordinary and not an error: a file with no grammar, a line
    /// outside every construct, a language nothing bundles. It means the node
    /// tier does not apply and the ladder starts one rung down.
    pub syntax: Vec<SyntaxStep>,
    /// The anchored line's text, whitespace-trimmed.
    ///
    /// Trimmed because reindentation is the most common edit that must *not*
    /// break an anchor — a block moving into an `if` shifts every line in it.
    pub text: String,
    /// The 1-based line it was placed on, used only to break ties.
    pub line: u32,
}

impl Fingerprint {
    /// A fingerprint of a location.
    #[must_use]
    pub fn new(syntax: Vec<SyntaxStep>, text: &str, line: u32) -> Self {
        Self {
            syntax,
            text: text.trim().to_string(),
            line,
        }
    }

    /// Whether the node tier applies at all.
    #[must_use]
    pub fn has_syntax(&self) -> bool {
        !self.syntax.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The snapshot the host hands down
// ---------------------------------------------------------------------------

/// One line of a file as the host sees it after a rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnapshotLine {
    /// The line's text. [`Snapshot::of`] trims it; a hand-built one should too.
    pub text: String,
    /// The syntax path covering this line, empty where no grammar resolved.
    pub syntax: Vec<SyntaxStep>,
}

/// A file's current text and syntax, as the only thing the store is told about
/// it.
///
/// The host builds this; see the module header for why the store cannot. Lines
/// are 0-indexed here and 1-based in a [`Position`], and [`Snapshot::position`]
/// is the one place that conversion happens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// Every line, in order.
    pub lines: Vec<SnapshotLine>,
}

impl Snapshot {
    /// A snapshot with no syntax at all — the grammar-free case, and what every
    /// `T043` test builds.
    #[must_use]
    pub fn of(text: &str) -> Self {
        Self {
            lines: text
                .lines()
                .map(|line| SnapshotLine {
                    text: line.trim().to_string(),
                    syntax: Vec::new(),
                })
                .collect(),
        }
    }

    /// Attach a syntax path to a 0-indexed line, for building node-tier
    /// fixtures without a parser.
    #[must_use]
    pub fn with_syntax(mut self, line: usize, syntax: Vec<SyntaxStep>) -> Self {
        if let Some(row) = self.lines.get_mut(line) {
            row.syntax = syntax;
        }
        self
    }

    /// How many lines it has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether it has none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The whole of a 0-indexed line, as a span.
    ///
    /// Column 1 to column 1 of the next line is deliberate rather than
    /// end-of-text: an anchor is a *place*, and a zero-width span at the start
    /// of the line is what [`super::region::overlaps`] treats as a point.
    #[must_use]
    fn position(&self, line: usize) -> Span {
        let one_based = u32::try_from(line + 1).unwrap_or(u32::MAX);
        Span {
            start: Position {
                line: one_based,
                column: 1,
            },
            end: Position {
                line: one_based,
                column: 1,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Where a fingerprint resolves in a snapshot, and at which tier.
///
/// Free rather than a method so the ladder can be tested on its own, without
/// minting an anchor to carry it.
///
/// # The rule
///
/// 1. **Node tier** — lines whose syntax path equals the fingerprint's. Among
///    them, one whose text also matches wins; otherwise the one nearest the
///    line it was on.
/// 2. **Line tier** — lines whose trimmed text equals the fingerprint's,
///    nearest the line it was on.
/// 3. **Lost** — neither.
///
/// The *"nearest the line it was on"* tie-break is what makes the common case
/// stable. Every line inside one function shares a syntax path, so the node
/// tier alone would land on the first of them; preferring an exact text match
/// and then proximity keeps an anchor on its own line through an edit that
/// moves the whole function.
#[must_use]
pub fn resolve(fingerprint: &Fingerprint, snapshot: &Snapshot) -> Option<(Span, Tier)> {
    if fingerprint.has_syntax() {
        let candidates: Vec<usize> = snapshot
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.syntax == fingerprint.syntax)
            .map(|(index, _)| index)
            .collect();
        if let Some(line) = pick(&candidates, fingerprint, snapshot) {
            return Some((snapshot.position(line), Tier::Node));
        }
    }

    if !fingerprint.text.is_empty() {
        let candidates: Vec<usize> = snapshot
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.text == fingerprint.text)
            .map(|(index, _)| index)
            .collect();
        if let Some(line) = nearest(&candidates, fingerprint.line) {
            return Some((snapshot.position(line), Tier::Line));
        }
    }

    None
}

/// The best of several node-tier candidates: exact text first, then proximity.
fn pick(candidates: &[usize], fingerprint: &Fingerprint, snapshot: &Snapshot) -> Option<usize> {
    let exact: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|index| {
            snapshot
                .lines
                .get(*index)
                .is_some_and(|line| line.text == fingerprint.text)
        })
        .collect();
    if let Some(found) = nearest(&exact, fingerprint.line) {
        return Some(found);
    }
    nearest(candidates, fingerprint.line)
}

/// The candidate nearest a 1-based line, ties going to the earlier line.
fn nearest(candidates: &[usize], line: u32) -> Option<usize> {
    let was = usize::try_from(line.saturating_sub(1)).unwrap_or(0);
    candidates
        .iter()
        .copied()
        .min_by_key(|index| (index.abs_diff(was), *index))
}

// ---------------------------------------------------------------------------
// The anchor
// ---------------------------------------------------------------------------

/// One anchored location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The store's own id, stable across every reanchoring.
    pub id: AnchorId,
    /// Workspace-relative path, exactly as the placing door spelled it. The
    /// store never interprets a path — the same contract
    /// [`super::region`] and [`super::diagnostics`] keep.
    pub path: PathBuf,
    /// A name to find it by: `m`'s `a`–`z`, or a caller's own.
    pub label: Option<String>,
    /// Where it currently points.
    pub span: Span,
    /// How to find it again.
    pub fingerprint: Fingerprint,
    /// Which rung it last resolved at.
    pub tier: Tier,
}

impl Anchor {
    /// The record an `anchor` query answers.
    ///
    /// Hand-built for the reason [`super::region::Region::to_value`] gives:
    /// nothing decodes an `Anchor` from a door, so a `Wire` impl would be a
    /// decoder nobody calls.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let syntax = Value::List(
            self.fingerprint
                .syntax
                .iter()
                .map(SyntaxStep::to_value)
                .collect(),
        );
        Value::Record(
            Args::new()
                .with("id", self.id.to_value())
                .with("path", Value::Text(self.path.display().to_string()))
                .with(
                    "label",
                    self.label
                        .as_ref()
                        .map_or(Value::Null, |label| Value::Text(label.clone())),
                )
                .with("span", self.span.to_value())
                .with("tier", Value::Text(self.tier.name().to_owned()))
                .with("syntax", syntax),
        )
    }
}

// ---------------------------------------------------------------------------
// What a reanchoring did
// ---------------------------------------------------------------------------

/// The outcome of reanchoring one file.
///
/// Three lists rather than a count, because the three are acted on differently:
/// `moved` invalidates a frame, `held` is the quiet majority, and `lost` is the
/// only one worth telling a person about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reanchored {
    /// Anchors whose span changed.
    pub moved: Vec<AnchorId>,
    /// Anchors that resolved to where they already were.
    pub held: Vec<AnchorId>,
    /// Anchors that resolved at no tier.
    pub lost: Vec<AnchorId>,
}

impl Reanchored {
    /// How many anchors were considered.
    #[must_use]
    pub fn total(&self) -> usize {
        self.moved.len() + self.held.len() + self.lost.len()
    }

    /// Whether anything changed — the frame-invalidation question.
    #[must_use]
    pub fn changed(&self) -> bool {
        !self.moved.is_empty() || !self.lost.is_empty()
    }

    /// The record the `reanchor` capability answers.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Record(
            Args::new()
                .with("moved", Value::Int(as_int(self.moved.len())))
                .with("held", Value::Int(as_int(self.held.len())))
                .with("lost", Value::Int(as_int(self.lost.len()))),
        )
    }
}

fn as_int(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// The collection
// ---------------------------------------------------------------------------

/// Every anchor the store holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anchors {
    by_id: BTreeMap<AnchorId, Anchor>,
    next: u64,
}

impl Anchors {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Place an anchor, answering its id.
    ///
    /// A `label` **replaces** any existing anchor with that label in the same
    /// file, which is vim's rule for `m`: `ma` twice is one mark, not two. The
    /// id is fresh either way — a surface holding the old one must get nothing
    /// back rather than the new location, for the reason
    /// [`super::region::Regions`] mints monotonically.
    pub fn place(
        &mut self,
        path: PathBuf,
        span: Span,
        label: Option<String>,
        fingerprint: Fingerprint,
    ) -> AnchorId {
        if let Some(label) = label.as_deref() {
            let displaced: Vec<AnchorId> = self
                .by_id
                .values()
                .filter(|anchor| anchor.path == path && anchor.label.as_deref() == Some(label))
                .map(|anchor| anchor.id)
                .collect();
            for id in displaced {
                self.by_id.remove(&id);
            }
        }
        let id = self.mint();
        self.by_id.insert(
            id,
            Anchor {
                id,
                path,
                label,
                span,
                fingerprint,
                tier: Tier::Node,
            },
        );
        id
    }

    /// One anchor.
    #[must_use]
    pub fn get(&self, id: AnchorId) -> Option<&Anchor> {
        self.by_id.get(&id)
    }

    /// The anchor with a label, in a file.
    #[must_use]
    pub fn labelled(&self, path: &Path, label: &str) -> Option<&Anchor> {
        self.by_id
            .values()
            .find(|anchor| anchor.path == path && anchor.label.as_deref() == Some(label))
    }

    /// Every anchor in a file, in id order.
    pub fn in_file<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a Anchor> {
        self.by_id
            .values()
            .filter(move |anchor| anchor.path == path)
    }

    /// Every anchor, in id order.
    pub fn all(&self) -> impl Iterator<Item = &Anchor> {
        self.by_id.values()
    }

    /// Forget every anchor in a file — a deleted file, a closed buffer.
    pub fn drop_in(&mut self, path: &Path) -> usize {
        let doomed: Vec<AnchorId> = self.in_file(path).map(|anchor| anchor.id).collect();
        for id in &doomed {
            self.by_id.remove(id);
        }
        doomed.len()
    }

    /// Re-resolve every anchor in a file against its rewritten text.
    ///
    /// A [`Tier::Lost`] anchor keeps its span. It is stale, and it is *labelled*
    /// stale, which is the only pair of facts a surface can act on — moving it
    /// somewhere plausible would be the lie.
    pub fn reanchor(&mut self, path: &Path, snapshot: &Snapshot) -> Reanchored {
        let mut out = Reanchored::default();
        let ids: Vec<AnchorId> = self.in_file(path).map(|anchor| anchor.id).collect();
        for id in ids {
            let Some(anchor) = self.by_id.get_mut(&id) else {
                continue;
            };
            match resolve(&anchor.fingerprint, snapshot) {
                Some((span, tier)) => {
                    let was = anchor.span;
                    anchor.span = span;
                    anchor.tier = tier;
                    anchor.fingerprint.line = span.start.line;
                    if was == span {
                        out.held.push(id);
                    } else {
                        out.moved.push(id);
                    }
                }
                None => {
                    anchor.tier = Tier::Lost;
                    out.lost.push(id);
                }
            }
        }
        out
    }

    /// How many anchors it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// The next id. Monotonic and never reused, for the reason
    /// [`super::region::Regions`] gives: a surface holding a dropped anchor's
    /// id must get nothing back, not somebody else's anchor.
    fn mint(&mut self) -> AnchorId {
        self.next += 1;
        AnchorId(self.next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retry() -> Vec<SyntaxStep> {
        vec![
            SyntaxStep::new("impl_item", "Backoff"),
            SyntaxStep::new("function_item", "retry"),
        ]
    }

    fn at(line: u32) -> Span {
        Span {
            start: Position { line, column: 1 },
            end: Position { line, column: 1 },
        }
    }

    fn path() -> PathBuf {
        PathBuf::from("src/retry.rs")
    }

    fn rust_file() -> Snapshot {
        Snapshot::of(
            "impl Backoff {\nfn retry(&self) -> u32 {\nlet attempts = 3;\nattempts\n}\n}\n",
        )
        .with_syntax(0, vec![SyntaxStep::new("impl_item", "Backoff")])
        .with_syntax(1, retry())
        .with_syntax(2, retry())
        .with_syntax(3, retry())
    }

    // -- the ladder ---------------------------------------------------------

    #[test]
    fn the_node_tier_follows_a_construct_that_moved() {
        let was = Fingerprint::new(retry(), "let attempts = 3;", 3);

        // The whole impl slid down four lines behind a new header comment.
        let now = Snapshot::of(
            "// a header that did not exist\n// and a second line of it\n\n\nimpl Backoff {\nfn retry(&self) -> u32 {\nlet attempts = 3;\nattempts\n}\n}\n",
        )
        .with_syntax(5, retry())
        .with_syntax(6, retry())
        .with_syntax(7, retry());

        let (span, tier) = resolve(&was, &now).expect("the construct is still there");
        assert_eq!(tier, Tier::Node);
        assert_eq!(span, at(7), "the line it moved to, not the line it was on");
    }

    #[test]
    fn among_lines_of_one_construct_the_text_decides() {
        let was = Fingerprint::new(retry(), "attempts", 4);
        let (span, tier) = resolve(&was, &rust_file()).expect("resolves");

        assert_eq!(tier, Tier::Node);
        assert_eq!(
            span,
            at(4),
            "every line of `retry` shares a path, so the text is what separates them",
        );
    }

    #[test]
    fn the_line_tier_catches_a_file_with_no_grammar_at_all() {
        let was = Fingerprint::new(Vec::new(), "PHOSPHOR_LOG=debug", 2);
        let now = Snapshot::of("# a comment\n\nPHOSPHOR_LOG=debug\nOTHER=1\n");

        let (span, tier) = resolve(&was, &now).expect("the line is still there");
        assert_eq!(tier, Tier::Line, "T043's floor, and it needs no grammar");
        assert_eq!(span, at(3));
    }

    #[test]
    fn reindentation_does_not_break_an_anchor() {
        let was = Fingerprint::new(Vec::new(), "call_it();", 1);
        // The line moved into an `if`, so its leading whitespace changed.
        let now = Snapshot::of("if ready {\n        call_it();\n}\n");

        let (span, tier) = resolve(&was, &now).expect("trimming is why this resolves");
        assert_eq!(tier, Tier::Line);
        assert_eq!(span, at(2));
    }

    #[test]
    fn a_rename_falls_off_the_node_tier() {
        let was = Fingerprint::new(retry(), "let attempts = 3;", 3);
        let renamed = vec![
            SyntaxStep::new("impl_item", "Backoff"),
            SyntaxStep::new("function_item", "retry_with_backoff"),
        ];
        let now = Snapshot::of(
            "impl Backoff {\nfn retry_with_backoff(&self) -> u32 {\nlet attempts = 3;\n}\n",
        )
        .with_syntax(1, renamed.clone())
        .with_syntax(2, renamed);

        let (span, tier) = resolve(&was, &now).expect("the line text still matches");
        assert_eq!(
            tier,
            Tier::Line,
            "the node tier deliberately does not follow a rename",
        );
        assert_eq!(span, at(3));
    }

    #[test]
    fn nothing_matching_resolves_at_no_tier() {
        let was = Fingerprint::new(retry(), "let attempts = 3;", 3);
        let now = Snapshot::of("something\nentirely\ndifferent\n");

        assert!(resolve(&was, &now).is_none());
    }

    #[test]
    fn an_empty_fingerprint_cannot_resolve_by_accident() {
        let was = Fingerprint::new(Vec::new(), "   ", 1);
        let now = Snapshot::of("\n\n\n");

        assert!(
            resolve(&was, &now).is_none(),
            "a blank fingerprint would otherwise match every blank line",
        );
    }

    #[test]
    fn duplicate_lines_break_the_tie_by_proximity() {
        let now = Snapshot::of("x();\ny();\nx();\nz();\nx();\n");

        let near_top = Fingerprint::new(Vec::new(), "x();", 1);
        let near_bottom = Fingerprint::new(Vec::new(), "x();", 5);

        assert_eq!(resolve(&near_top, &now).expect("resolves").0, at(1));
        assert_eq!(resolve(&near_bottom, &now).expect("resolves").0, at(5));
    }

    // -- the collection -----------------------------------------------------

    #[test]
    fn a_label_placed_twice_in_one_file_is_one_mark() {
        let mut anchors = Anchors::new();
        let first = anchors.place(
            path(),
            at(1),
            Some("a".to_owned()),
            Fingerprint::new(Vec::new(), "one", 1),
        );
        let second = anchors.place(
            path(),
            at(9),
            Some("a".to_owned()),
            Fingerprint::new(Vec::new(), "nine", 9),
        );

        assert_ne!(first, second, "the id is fresh — vim's rule is one *mark*");
        assert_eq!(anchors.len(), 1);
        assert!(
            anchors.get(first).is_none(),
            "the displaced id answers nothing",
        );
        assert_eq!(
            anchors.labelled(&path(), "a").expect("the mark").span,
            at(9)
        );
    }

    #[test]
    fn the_same_label_in_two_files_is_two_marks() {
        let mut anchors = Anchors::new();
        anchors.place(
            PathBuf::from("a.rs"),
            at(1),
            Some("a".to_owned()),
            Fingerprint::new(Vec::new(), "one", 1),
        );
        anchors.place(
            PathBuf::from("b.rs"),
            at(2),
            Some("a".to_owned()),
            Fingerprint::new(Vec::new(), "two", 2),
        );

        assert_eq!(anchors.len(), 2);
    }

    #[test]
    fn an_id_is_never_reused_after_a_drop() {
        let mut anchors = Anchors::new();
        let first = anchors.place(path(), at(1), None, Fingerprint::new(Vec::new(), "x", 1));
        assert_eq!(anchors.drop_in(&path()), 1);
        let second = anchors.place(path(), at(1), None, Fingerprint::new(Vec::new(), "x", 1));

        assert_ne!(first, second);
        assert!(anchors.get(first).is_none());
    }

    #[test]
    fn reanchoring_sorts_into_moved_held_and_lost() {
        let mut anchors = Anchors::new();
        let moves = anchors.place(
            path(),
            at(9),
            None,
            Fingerprint::new(Vec::new(), "let attempts = 3;", 9),
        );
        let holds = anchors.place(
            path(),
            at(1),
            None,
            Fingerprint::new(Vec::new(), "impl Backoff {", 1),
        );
        let lost = anchors.place(
            path(),
            at(2),
            None,
            Fingerprint::new(Vec::new(), "fn deleted(&self) {", 2),
        );

        let now = Snapshot::of("impl Backoff {\n\nlet attempts = 3;\n");
        let outcome = anchors.reanchor(&path(), &now);

        assert_eq!(outcome.moved, vec![moves], "9 became 3");
        assert_eq!(outcome.held, vec![holds], "1 was already 1");
        assert_eq!(outcome.lost, vec![lost]);
        assert_eq!(outcome.total(), 3);
        assert!(outcome.changed());
    }

    #[test]
    fn a_lost_anchor_keeps_its_span_and_says_it_is_lost() {
        let mut anchors = Anchors::new();
        let id = anchors.place(
            path(),
            at(7),
            None,
            Fingerprint::new(Vec::new(), "gone_forever();", 7),
        );

        anchors.reanchor(&path(), &Snapshot::of("nothing\nlike\nit\n"));

        let anchor = anchors.get(id).expect("still held");
        assert_eq!(anchor.tier, Tier::Lost);
        assert_eq!(anchor.span, at(7), "stale, and labelled stale — not moved");
        assert!(!anchor.tier.resolves());
    }

    #[test]
    fn reanchoring_leaves_other_files_alone() {
        let mut anchors = Anchors::new();
        let elsewhere = anchors.place(
            PathBuf::from("other.rs"),
            at(4),
            None,
            Fingerprint::new(Vec::new(), "untouched();", 4),
        );

        let outcome = anchors.reanchor(&path(), &Snapshot::of("anything\n"));

        assert_eq!(outcome.total(), 0);
        assert_eq!(anchors.get(elsewhere).expect("held").tier, Tier::Node);
    }

    #[test]
    fn an_unchanged_file_reports_no_change() {
        let mut anchors = Anchors::new();
        anchors.place(
            path(),
            at(1),
            None,
            Fingerprint::new(Vec::new(), "impl Backoff {", 1),
        );

        let outcome = anchors.reanchor(&path(), &Snapshot::of("impl Backoff {\n"));

        assert!(
            !outcome.changed(),
            "T079's cache must not redraw on a save that moved nothing",
        );
    }

    // -- the wire -----------------------------------------------------------

    #[test]
    fn the_record_carries_the_tier_and_the_syntax() {
        let mut anchors = Anchors::new();
        let id = anchors.place(
            path(),
            at(2),
            Some("a".to_owned()),
            Fingerprint::new(retry(), "let attempts = 3;", 2),
        );

        let Value::Record(fields) = anchors.get(id).expect("placed").to_value() else {
            panic!("an anchor is a record");
        };
        let pairs: Vec<(String, Value)> = fields.into_pairs().collect();
        let named = |want: &str| {
            pairs
                .iter()
                .find(|(field, _)| field == want)
                .map(|(_, value)| value.clone())
        };

        assert_eq!(named("tier"), Some(Value::Text("node".to_owned())));
        assert_eq!(named("label"), Some(Value::Text("a".to_owned())));
        assert_eq!(named("path"), Some(Value::Text("src/retry.rs".to_owned())));
        let Some(Value::List(syntax)) = named("syntax") else {
            panic!("the syntax path is a list");
        };
        assert_eq!(syntax.len(), 2, "impl_item and function_item");
    }

    #[test]
    fn an_unlabelled_anchor_says_null_rather_than_omitting_the_field() {
        let mut anchors = Anchors::new();
        let id = anchors.place(path(), at(1), None, Fingerprint::new(Vec::new(), "x", 1));

        let Value::Record(fields) = anchors.get(id).expect("placed").to_value() else {
            panic!("a record");
        };
        let label = fields
            .into_pairs()
            .find(|(field, _)| field == "label")
            .map(|(_, value)| value);

        assert_eq!(label, Some(Value::Null));
    }

    #[test]
    fn the_tier_names_round_trip_through_display() {
        for tier in [Tier::Node, Tier::Line, Tier::Lost] {
            assert_eq!(tier.to_string(), tier.name());
        }
        assert!(Tier::Node.resolves());
        assert!(Tier::Line.resolves());
        assert!(!Tier::Lost.resolves());
    }
}
