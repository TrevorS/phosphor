//! The store on disk — `T044`, and `Stream::SEEN`'s schema.
//!
//! Seen-state is worth nothing if it does not survive a restart: *"claude wrote
//! here and you have not looked"* is a claim about a session that outlives the
//! session. This module is the [`Folded`] implementation that makes it, and it
//! is deliberately the *whole* of `T044`'s persistence surface — everything
//! else (framing, checksums, torn-tail recovery, atomic compaction, the state
//! directory, the codec) is [`crate::journal`]'s and was designed two phases
//! ago for exactly this.
//!
//! `journal.rs`'s own header says so: *"Supplies: an `impl Folded for` its own
//! state — a record enum, `apply`, and `snapshot`. That is the whole of
//! `T044`'s persistence surface."* This file is that sentence, honoured.
//!
//! # Regions, not just a seen flag
//!
//! `T041` found this task owes the regions themselves, and the reason is one
//! sentence: **a seen flag refers to a region, and if the regions are gone the
//! flag has no subject.** Persisting `{region 4: seen}` into a store that has
//! never heard of region 4 restores nothing. So a row is a whole region.
//!
//! Anchors ride along in the same log. A mark is user state in exactly the way
//! seen-state is — `ma` in a file you come back to tomorrow should still be
//! there — and `journal.rs` frames the decision as one to make *"once for the
//! whole thing rather than per sub-store"*. Diagnostics deliberately do **not**
//! persist: they are a language server's assertion about the current text, and
//! a restored one would be a claim nobody is standing behind.
//!
//! # Why every record is a whole row
//!
//! The alternative is deltas — `Seen { id, state }`, `Moved { id, span }` — and
//! it is smaller on disk and worse everywhere else. [`Folded`]'s law is
//!
//! ```text
//!   fold(snapshot(state)) == state
//! ```
//!
//! and [`Log::compact`] rewrites the file as `snapshot(state)`, so a `snapshot`
//! that loses something loses it **permanently and silently**. With whole-row
//! upserts, `snapshot` is *"one record per live row"* and the law is true by
//! construction rather than by care. A delta schema makes `snapshot` a
//! reconstruction problem, which is the shape of bug that only shows up after a
//! compaction, in a file the user cannot read, a week later.
//!
//! The cost is bytes, and bytes are what compaction is for.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::journal::{DecodeError, Decoder, Encoder, FoldError, Folded, Log, Stream};
use crate::request::{Actor, AnchorId, Position, RegionId, Span};

use super::anchor::{Anchor, Fingerprint, SyntaxStep, Tier};
use super::region::{Region, SeenState};

/// The seen-state log — [`Stream::SEEN`], folded into a [`Seen`].
pub type SeenLog = Log<Seen>;

const TAG_REGION: u64 = 1;
const TAG_REGION_GONE: u64 = 2;
const TAG_ANCHOR: u64 = 3;
const TAG_ANCHOR_GONE: u64 = 4;
const TAG_MINTED: u64 = 5;

/// One entry in the log.
///
/// Four, and two of them are tombstones. A `Gone` is not optional: without it a
/// dropped region comes back on the next restart, because the log still carries
/// the record that created it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A whole region, created or updated.
    Region(Box<Region>),
    /// A region that is no longer there.
    RegionGone(RegionId),
    /// A whole anchor, created or updated.
    Anchor(Box<Anchor>),
    /// An anchor that is no longer there.
    AnchorGone(AnchorId),
    /// How many ids each collection has ever minted.
    ///
    /// **Not derivable from the live rows, which is why it is a record.** Both
    /// collections mint monotonically *"so a surface holding a dropped id must
    /// get nothing back"*, and the largest id still alive is not the largest
    /// that ever existed — drop the highest region and that fact leaves with
    /// it. Restoring from live rows alone would reissue a retired id after a
    /// restart. One record, coalesced to exactly one by every compaction.
    Minted {
        /// Regions minted.
        regions: u64,
        /// Anchors minted.
        anchors: u64,
    },
}

/// The store's persistable half, folded from a log.
///
/// Two maps and nothing derived. This is not a [`super::Store`] — it holds no
/// revision and no diagnostics — because what persists and what a query answers
/// are different questions, and a type that tried to be both would have to
/// decide whether a restored store starts at revision 0 or at the one it had.
/// It starts at 0, which is what a fresh process means.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Seen {
    /// Every live region, by id.
    pub regions: BTreeMap<RegionId, Region>,
    /// Every live anchor, by id.
    pub anchors: BTreeMap<AnchorId, Anchor>,
    /// Ids minted so far — see [`Record::Minted`].
    pub minted_regions: u64,
    /// Ids minted so far — see [`Record::Minted`].
    pub minted_anchors: u64,
}

impl Seen {
    /// How many rows it holds, both kinds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len() + self.anchors.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty() && self.anchors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The codec
// ---------------------------------------------------------------------------

fn put_actor(out: &mut Encoder, actor: Actor) {
    out.u64(match actor {
        Actor::You => 0,
        Actor::Claude => 1,
        Actor::Steel => 2,
        Actor::Cli => 3,
        Actor::System => 4,
    });
}

fn get_actor(input: &mut Decoder<'_>) -> Result<Actor, DecodeError> {
    Ok(match input.u64()? {
        0 => Actor::You,
        1 => Actor::Claude,
        2 => Actor::Steel,
        3 => Actor::Cli,
        // Anything else is a schema this version does not know. `System` rather
        // than an error, because an unknown *actor* does not make the region it
        // is attached to meaningless — and refusing here would discard a whole
        // session's seen-state over one field.
        _ => Actor::System,
    })
}

fn put_span(out: &mut Encoder, span: Span) {
    out.u64(u64::from(span.start.line));
    out.u64(u64::from(span.start.column));
    out.u64(u64::from(span.end.line));
    out.u64(u64::from(span.end.column));
}

fn get_span(input: &mut Decoder<'_>) -> Result<Span, DecodeError> {
    let line = |input: &mut Decoder<'_>| -> Result<u32, DecodeError> {
        Ok(u32::try_from(input.u64()?).unwrap_or(u32::MAX))
    };
    Ok(Span {
        start: Position {
            line: line(input)?,
            column: line(input)?,
        },
        end: Position {
            line: line(input)?,
            column: line(input)?,
        },
    })
}

fn put_syntax(out: &mut Encoder, syntax: &[SyntaxStep]) {
    out.seq_len(syntax.len());
    for step in syntax {
        out.str(&step.kind);
        out.str(&step.name);
    }
}

fn get_syntax(input: &mut Decoder<'_>) -> Result<Vec<SyntaxStep>, DecodeError> {
    let count = input.seq_len()?;
    let mut steps = Vec::with_capacity(count);
    for _ in 0..count {
        steps.push(SyntaxStep {
            kind: input.str()?,
            name: input.str()?,
        });
    }
    Ok(steps)
}

fn put_fingerprint(out: &mut Encoder, fingerprint: Option<&Fingerprint>) {
    match fingerprint {
        Some(fingerprint) => {
            out.bool(true);
            put_syntax(out, &fingerprint.syntax);
            out.str(&fingerprint.text);
            out.u64(u64::from(fingerprint.line));
        }
        None => out.bool(false),
    }
}

fn get_fingerprint(input: &mut Decoder<'_>) -> Result<Option<Fingerprint>, DecodeError> {
    if !input.bool()? {
        return Ok(None);
    }
    let syntax = get_syntax(input)?;
    let text = input.str()?;
    let line = u32::try_from(input.u64()?).unwrap_or(u32::MAX);
    Ok(Some(Fingerprint { syntax, text, line }))
}

impl Folded for Seen {
    type Record = Record;

    const STREAM: Stream = Stream::SEEN;

    fn encode(record: &Self::Record, out: &mut Encoder) {
        match record {
            Record::Region(region) => {
                out.u64(TAG_REGION);
                out.u64(region.id.0);
                out.str(&region.path.display().to_string());
                put_span(out, region.span);
                put_actor(out, region.author);
                put_actor(out, region.declared_by);
                out.bool(region.state.unseen());
                out.u64(u64::from(region.revisions));
                put_fingerprint(out, region.fingerprint.as_ref());
            }
            Record::RegionGone(id) => {
                out.u64(TAG_REGION_GONE);
                out.u64(id.0);
            }
            Record::Anchor(anchor) => {
                out.u64(TAG_ANCHOR);
                out.u64(anchor.id.0);
                out.str(&anchor.path.display().to_string());
                match anchor.label.as_deref() {
                    Some(label) => {
                        out.bool(true);
                        out.str(label);
                    }
                    None => out.bool(false),
                }
                put_span(out, anchor.span);
                out.u64(match anchor.tier {
                    Tier::Node => 0,
                    Tier::Line => 1,
                    Tier::Lost => 2,
                });
                put_syntax(out, &anchor.fingerprint.syntax);
                out.str(&anchor.fingerprint.text);
                out.u64(u64::from(anchor.fingerprint.line));
            }
            Record::AnchorGone(id) => {
                out.u64(TAG_ANCHOR_GONE);
                out.u64(id.0);
            }
            Record::Minted { regions, anchors } => {
                out.u64(TAG_MINTED);
                out.u64(*regions);
                out.u64(*anchors);
            }
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self::Record, DecodeError> {
        let mut input = Decoder::new(bytes);
        let record = match input.u64()? {
            TAG_REGION => {
                let id = RegionId(input.u64()?);
                let path = PathBuf::from(input.str()?);
                let span = get_span(&mut input)?;
                let author = get_actor(&mut input)?;
                let declared_by = get_actor(&mut input)?;
                let state = if input.bool()? {
                    SeenState::Unseen
                } else {
                    SeenState::Seen
                };
                let revisions = u32::try_from(input.u64()?).unwrap_or(u32::MAX);
                let fingerprint = get_fingerprint(&mut input)?;
                Record::Region(Box::new(Region {
                    id,
                    path,
                    span,
                    author,
                    declared_by,
                    state,
                    revisions,
                    fingerprint,
                }))
            }
            TAG_REGION_GONE => Record::RegionGone(RegionId(input.u64()?)),
            TAG_ANCHOR => {
                let id = AnchorId(input.u64()?);
                let path = PathBuf::from(input.str()?);
                let label = if input.bool()? {
                    Some(input.str()?)
                } else {
                    None
                };
                let span = get_span(&mut input)?;
                let tier = match input.u64()? {
                    0 => Tier::Node,
                    1 => Tier::Line,
                    _ => Tier::Lost,
                };
                let syntax = get_syntax(&mut input)?;
                let text = input.str()?;
                let line = u32::try_from(input.u64()?).unwrap_or(u32::MAX);
                Record::Anchor(Box::new(Anchor {
                    id,
                    path,
                    label,
                    span,
                    fingerprint: Fingerprint { syntax, text, line },
                    tier,
                }))
            }
            TAG_ANCHOR_GONE => Record::AnchorGone(AnchorId(input.u64()?)),
            TAG_MINTED => Record::Minted {
                regions: input.u64()?,
                anchors: input.u64()?,
            },
            other => {
                return Err(DecodeError::UnknownRecord { tag: other });
            }
        };
        input.finish()?;
        Ok(record)
    }

    fn apply(&mut self, record: Self::Record) -> Result<(), FoldError> {
        match record {
            Record::Region(region) => {
                self.regions.insert(region.id, *region);
            }
            // A tombstone for a row that is not there is **not** an error. Two
            // ordinary paths produce one: a compaction drops the row and keeps
            // the tombstone nowhere, and `drop-regions` over a scope that
            // already lost a row to an earlier drop names it twice. Refusing
            // here would make a valid log unreadable, which is the one failure
            // mode `Folded::apply` running on the way back in exists to avoid.
            Record::RegionGone(id) => {
                self.regions.remove(&id);
            }
            Record::Anchor(anchor) => {
                self.anchors.insert(anchor.id, *anchor);
            }
            Record::AnchorGone(id) => {
                self.anchors.remove(&id);
            }
            // `max` rather than assignment: records are folded in file order
            // and a counter can only go up, so an out-of-order or replayed
            // record can never walk it backwards into reissuing an id.
            Record::Minted { regions, anchors } => {
                self.minted_regions = self.minted_regions.max(regions);
                self.minted_anchors = self.minted_anchors.max(anchors);
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Vec<Self::Record> {
        // One record per live row, and no tombstones — a compacted log
        // describes what *is*, and a tombstone for a row nothing mentions is
        // bytes that decode to a no-op forever.
        std::iter::once(Record::Minted {
            regions: self.minted_regions,
            anchors: self.minted_anchors,
        })
        .chain(
            self.regions
                .values()
                .map(|region| Record::Region(Box::new(region.clone()))),
        )
        .chain(
            self.anchors
                .values()
                .map(|anchor| Record::Anchor(Box::new(anchor.clone()))),
        )
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(id: u64, line: u32, state: SeenState) -> Region {
        Region {
            id: RegionId(id),
            path: PathBuf::from("src/retry.rs"),
            span: Span {
                start: Position { line, column: 1 },
                end: Position { line, column: 9 },
            },
            author: Actor::Claude,
            declared_by: Actor::Cli,
            state,
            revisions: 2,
            fingerprint: Some(Fingerprint::new(
                vec![SyntaxStep::new("function_item", "retry")],
                "let attempts = 3;",
                line,
            )),
        }
    }

    fn anchor(id: u64, label: Option<&str>) -> Anchor {
        Anchor {
            id: AnchorId(id),
            path: PathBuf::from("src/retry.rs"),
            label: label.map(ToOwned::to_owned),
            span: Span {
                start: Position { line: 4, column: 2 },
                end: Position { line: 4, column: 2 },
            },
            fingerprint: Fingerprint::new(Vec::new(), "attempts", 4),
            tier: Tier::Line,
        }
    }

    fn round_trip(record: &Record) -> Record {
        let mut out = Encoder::new();
        Seen::encode(record, &mut out);
        Seen::decode(&out.finish()).expect("what this schema wrote, it reads")
    }

    #[test]
    fn every_record_round_trips_through_the_codec() {
        for record in [
            Record::Region(Box::new(region(1, 6, SeenState::Seen))),
            Record::Region(Box::new(region(2, 9, SeenState::Unseen))),
            Record::RegionGone(RegionId(7)),
            Record::Anchor(Box::new(anchor(1, Some("a")))),
            Record::Anchor(Box::new(anchor(2, None))),
            Record::AnchorGone(AnchorId(9)),
            Record::Minted {
                regions: 41,
                anchors: 3,
            },
        ] {
            assert_eq!(round_trip(&record), record);
        }
    }

    /// A region with no fingerprint is the ordinary door-declared case, and the
    /// `Option` is a byte in the encoding rather than an absence of one.
    #[test]
    fn an_unfingerprinted_region_round_trips() {
        let mut bare = region(3, 1, SeenState::Unseen);
        bare.fingerprint = None;
        let record = Record::Region(Box::new(bare));
        assert_eq!(round_trip(&record), record);
    }

    /// [`Folded`]'s law, and the one that makes compaction safe:
    ///
    /// ```text
    ///   fold(snapshot(state)) == state
    /// ```
    ///
    /// `Log::compact` rewrites the file as `snapshot(state)`, so a `snapshot`
    /// that loses something loses it permanently and silently. The trait's own
    /// doc says *"test the law"*.
    #[test]
    fn folding_a_snapshot_gives_the_same_state() {
        let mut state = Seen::default();
        for record in [
            Record::Minted {
                regions: 12,
                anchors: 4,
            },
            Record::Region(Box::new(region(1, 6, SeenState::Seen))),
            Record::Region(Box::new(region(2, 9, SeenState::Unseen))),
            Record::Anchor(Box::new(anchor(1, Some("a")))),
            Record::RegionGone(RegionId(1)),
        ] {
            state.apply(record).expect("every record fits");
        }

        let mut rebuilt = Seen::default();
        for record in state.snapshot() {
            rebuilt.apply(record).expect("a snapshot folds");
        }

        assert_eq!(rebuilt, state);
    }

    /// A tombstone survives compaction by *not being there* — the row is gone
    /// from the snapshot, so nothing recreates it and nothing has to remember
    /// that it died.
    #[test]
    fn a_dropped_row_does_not_come_back_through_a_snapshot() {
        let mut state = Seen::default();
        state
            .apply(Record::Region(Box::new(region(1, 6, SeenState::Seen))))
            .expect("fits");
        state.apply(Record::RegionGone(RegionId(1))).expect("fits");

        assert!(state.snapshot().iter().all(|record| !matches!(
            record,
            Record::Region(region) if region.id == RegionId(1)
        )));
    }

    /// A tombstone for a row that is not there is not an error. Two ordinary
    /// paths produce one — a compaction that already dropped it, and a
    /// `drop-regions` over a scope that names a row an earlier drop took.
    #[test]
    fn a_tombstone_for_nothing_is_not_an_error() {
        let mut state = Seen::default();
        assert!(state.apply(Record::RegionGone(RegionId(99))).is_ok());
        assert!(state.apply(Record::AnchorGone(AnchorId(99))).is_ok());
        assert!(state.is_empty());
    }

    /// The counter only ever goes up, so a replayed or out-of-order record can
    /// never walk it back into reissuing an id.
    #[test]
    fn the_minted_counter_never_walks_backwards() {
        let mut state = Seen::default();
        state
            .apply(Record::Minted {
                regions: 40,
                anchors: 9,
            })
            .expect("fits");
        state
            .apply(Record::Minted {
                regions: 2,
                anchors: 1,
            })
            .expect("fits");

        assert_eq!(state.minted_regions, 40);
        assert_eq!(state.minted_anchors, 9);
    }

    #[test]
    fn an_unknown_tag_is_refused_rather_than_guessed_at() {
        let mut out = Encoder::new();
        out.u64(64);
        assert!(matches!(
            Seen::decode(&out.finish()),
            Err(DecodeError::UnknownRecord { tag: 64 })
        ));
    }
}
