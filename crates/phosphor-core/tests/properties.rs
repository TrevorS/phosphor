//! The laws `phosphor-core` states in prose, checked over generated input
//! rather than over three examples each.
//!
//! Every property here is a sentence one of these modules already writes about
//! itself. The point is not coverage — a test written to colour a line proves
//! nothing and costs a maintenance obligation. The point is that three of these
//! sentences were load-bearing and were only ever checked at the two or three
//! inputs whoever wrote them thought of:
//!
//! * `value.rs` — *"[`Wire::to_value`] / [`Wire::from_value`] — the conversion
//!   the doors run"*, and *"implementations do not panic on hostile input:
//!   every door is reachable by something we do not control"*. The MCP door is
//!   where an agent's arbitrary input arrives, so **round-trip identity** and
//!   **rejection** are the two laws that hold the wire model together.
//! * `input/text.rs` — *"the newline boundary is the limit here"*, *"it stays
//!   on one line"*, *"one definition of what `~` means"*. Every editing surface
//!   is downstream of this file answering *what span does this motion cover*,
//!   and an off-by-one here is a delete of the wrong text.
//! * `journal.rs` — *"reads until the first frame that does not check out …
//!   truncates the file to the last good boundary"*, and [`Folded`]'s own
//!   *"folding a snapshot of a state produces that same state. Test the law."*
//!   Torn-tail recovery is what makes `T030`'s `kill -9` acceptance mean
//!   anything, and it was proven by three hand-built tails.
//!
//! House style is `phosphor-ui`'s `status_line.rs` `proptest!` block, whose
//! property is a law the type obeys (*the statusline never wraps, at any
//! width*) rather than a restatement of its code. `SPIKES.md`'s hygiene table
//! names `proptest` for exactly this shape of test.
//!
//! # Case counts
//!
//! Deliberately modest. `just test` runs this on every gate, and a property
//! that takes a minute is a property somebody deletes. The in-memory
//! properties run 256 cases, the ones that touch the filesystem run 32, and
//! the whole file is well under a second.
//!
//! # What generating found
//!
//! Four boundaries in the code, each **measured by a test** rather than
//! asserted in a comment nothing checks — which is the same rule `CLAUDE.md`
//! applies to a `VENDOR.md`:
//!
//! * [`cased_grows_on_a_sharp_s`] — case conversion is not
//!   character-count-preserving in any of the three modes, so `gU` cannot
//!   assume its result fits the span it came from.
//! * [`u64_saturates_on_the_way_out_above_i64_max`] — the wire is signed, and
//!   `wire_unsigned!` saturates rather than failing, silently.
//! * [`a_truncated_header_is_not_a_journal`] — the torn-tail contract starts
//!   after the header, not at byte zero.
//! * [`a_hand_written_redo_on_the_cursor_path_does_not_survive_compaction`] —
//!   the fold accepts strictly more than any writer emits, and compaction
//!   round-trips only what a writer emits.
//!
//! None of the four is reachable by anything the build does today. Each is
//! written down here because the next person to widen one of these types is
//! the person who needs to know.
//!
//! # Every property here was checked against a planted violation
//!
//! *"A planted violation that does not plant proves nothing."* Each property
//! below was run against a deliberate break in the code it covers — the
//! `.min()` removed from `char_right_operand`, the `.max(1)` removed from
//! `clamp`, `try_from` replaced by a cast in `wire_unsigned!`, `scan`'s CRC
//! comparison deleted, `snapshot`'s `Redo` fix-ups dropped, `find_char`'s
//! till-offset moved — and each was reverted once the property failed on it.
//!
//! **Two of them did not fail the first time, and both properties are
//! stronger for it.** They are the reason this section exists:
//!
//! * Deleting the CRC check left [`any_truncation_recovers_a_prefix`]
//!   entirely green, because a plain truncation is caught by the length check
//!   before the checksum is ever consulted. The property now writes a [`Tail`]
//!   of zeros or garbage after the cut, which is the corruption the CRC is
//!   actually for, and the same deletion now fails it.
//! * Moving `find_char`'s till-offset left
//!   [`a_motion_span_never_leaves_the_buffer`] green, because that property
//!   draws one motion in thirty-one and needs the target character on the
//!   line as well. [`a_find_stays_on_its_line_and_till_stops_one_short`] draws
//!   only from the four finds, and catches it.
//!
//! Owned by `spine`.

use std::fmt::Debug;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use phosphor_core::input::text::{self, Text};
use phosphor_core::journal::{DecodeError, Decoder, Encoder, Folded, UndoLog, undo};
use phosphor_core::registry::ParamType;
use phosphor_core::request::{
    Actor, AnchorId, Binding, BlockId, BufferId, CaseChange, Direction, FileSpan, GroupId, HunkId,
    InboxId, Motion, PaneId, PaneRef, Position, RegionId, RegionSpec, SelectionKind, Span, Target,
    TextObject, ThreadId, WatchId,
};
use phosphor_core::value::{Args, Value, Wire, WireError};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

// ===========================================================================
// P1 · The wire model — `value.rs`
// ===========================================================================

/// Integers with the edges in the sample rather than left to chance.
///
/// A uniform `i64` never draws `0`, `-1` or `i64::MIN` in 256 tries, and those
/// are the three that decide whether `u32`'s range check is a check or a cast.
fn any_int() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => any::<i64>(),
        1 => prop::sample::select(vec![
            0_i64,
            -1,
            1,
            i64::MIN,
            i64::MAX,
            i64::from(u32::MAX),
            i64::from(u32::MAX) + 1,
        ]),
    ]
}

/// Text with no control characters — what a door can actually carry.
fn any_text_value() -> impl Strategy<Value = String> {
    "[\\PC]{0,24}"
}

/// Any [`Value`], to a bounded depth. The recursion is what makes this worth
/// generating: a record inside a list inside a record is the shape an MCP
/// payload arrives in, and no hand-written example builds one.
fn any_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any_int().prop_map(Value::Int),
        any_text_value().prop_map(Value::Text),
    ];
    leaf.prop_recursive(3, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::List),
            prop::collection::vec(("[a-z]{1,4}", inner), 0..4)
                .prop_map(|pairs| Value::Record(pairs.into_iter().collect())),
        ]
    })
}

fn any_args() -> impl Strategy<Value = Args> {
    prop::collection::vec(("[a-z]{1,4}", any_value()), 0..4)
        .prop_map(|pairs| pairs.into_iter().collect())
}

/// A `u64` the wire can carry. See
/// [`u64_saturates_on_the_way_out_above_i64_max`] for the half it cannot.
fn any_wire_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        4 => 0_u64..=(i64::MAX as u64),
        1 => prop::sample::select(vec![0_u64, 1, u64::from(u32::MAX), i64::MAX as u64]),
    ]
}

fn any_position() -> impl Strategy<Value = Position> {
    (any::<u32>(), any::<u32>()).prop_map(|(line, column)| Position { line, column })
}

fn any_span() -> impl Strategy<Value = Span> {
    (any_position(), any_position()).prop_map(|(start, end)| Span { start, end })
}

fn any_file_span() -> impl Strategy<Value = FileSpan> {
    (any_text_value(), prop::option::of(any_span())).prop_map(|(path, span)| FileSpan {
        path: PathBuf::from(path),
        span,
    })
}

/// Every [`Target`] variant, including the four focus-relative fieldless ones
/// — a union's empty arm is a different code path in `wire_union!` from a
/// populated one, and it is the one a hand-written example skips.
fn any_target() -> impl Strategy<Value = Target> {
    prop_oneof![
        Just(Target::Cursor {}),
        Just(Target::Selection {}),
        Just(Target::PickerRow {}),
        Just(Target::FloatRow {}),
        any_wire_u64().prop_map(|id| Target::Buffer { id: BufferId(id) }),
        any_text_value().prop_map(|path| Target::File {
            path: PathBuf::from(path)
        }),
        (any_text_value(), any_span()).prop_map(|(path, span)| Target::Explicit {
            path: PathBuf::from(path),
            span,
        }),
        any_wire_u64().prop_map(|id| Target::Region { id: RegionId(id) }),
        any_wire_u64().prop_map(|id| Target::Anchor { id: AnchorId(id) }),
        any_wire_u64().prop_map(|id| Target::Hunk { id: HunkId(id) }),
        any_wire_u64().prop_map(|id| Target::Block { id: BlockId(id) }),
        any_wire_u64().prop_map(|id| Target::Group { id: GroupId(id) }),
        any_wire_u64().prop_map(|id| Target::Thread { id: ThreadId(id) }),
        any_wire_u64().prop_map(|id| Target::InboxItem { id: InboxId(id) }),
        any_wire_u64().prop_map(|id| Target::Watch { id: WatchId(id) }),
    ]
}

fn any_binding() -> impl Strategy<Value = Binding> {
    prop_oneof![
        (any_text_value(), any_args()).prop_map(|(name, args)| Binding::Capability { name, args }),
        any_text_value().prop_map(|source| Binding::Source { source }),
    ]
}

fn any_region_spec() -> impl Strategy<Value = RegionSpec> {
    (any_text_value(), any_span(), any_actor()).prop_map(|(path, span, author)| RegionSpec {
        path: PathBuf::from(path),
        span,
        author,
    })
}

fn any_actor() -> impl Strategy<Value = Actor> {
    prop::sample::select(vec![
        Actor::You,
        Actor::Claude,
        Actor::Steel,
        Actor::Cli,
        Actor::System,
    ])
}

/// The round-trip law, for one type at one value.
fn round_trips<T>(value: &T) -> Result<(), TestCaseError>
where
    T: Wire + PartialEq + Debug,
{
    let encoded = value.to_value();
    match T::from_value(&encoded) {
        Ok(back) => {
            prop_assert_eq!(
                &back,
                value,
                "round trip changed the value via {:?}",
                encoded
            );
            Ok(())
        }
        Err(error) => Err(TestCaseError::fail(format!(
            "{value:?} encoded to {encoded:?} and would not decode: {error}"
        ))),
    }
}

/// The rejection law's second half, for one type at one arbitrary [`Value`].
///
/// *Never a panic* is what running this at all proves. *Never a silently-wrong
/// value* is what this asserts: a decoder that accepted the wrong shape and
/// invented something would produce a `T` whose own encoding decodes to
/// something else. Decoding has to be a retraction or it is a guess.
fn decodes_or_refuses<T>(value: &Value) -> Result<(), TestCaseError>
where
    T: Wire + PartialEq + Debug,
{
    if let Ok(decoded) = T::from_value(value) {
        let again = T::from_value(&decoded.to_value());
        prop_assert!(
            again.as_ref() == Ok(&decoded),
            "{value:?} decoded to {decoded:?}, which does not survive its own encoding: {again:?}"
        );
    }

    // The **iff**, and it is the half that has teeth. Everything above is
    // idempotence, which an accept-everything decoder satisfies for free:
    // garbage -> the first variant -> its own tag -> the first variant. The
    // `CP-3` test-depth gate proved exactly that by planting a `wire_choice!`
    // that returns the first variant for every tag — all 21 properties and all
    // 204 phosphor-core tests stayed green.
    //
    // So state the law the doors actually rely on: a choice accepts a tag **if
    // and only if** it is one the choice declares. `ParamType::Choice` carries
    // that list, it is the same list MCP publishes as its schema enum and the
    // CLI publishes as its flag values, and a decoder that drifts from it is a
    // door that accepts what it did not advertise.
    //
    // **What this does and does not catch, stated rather than implied.** For a
    // type generated by `wire_choice!` both sides of the iff come from the same
    // `$tag` list, so the macro cannot violate it and no plant against the
    // macro's *current output* can make this fail — attempting one produced a
    // tree that would not compile, which proves nothing. What it guards is a
    // hand-written `Wire` impl, and any future edit that lets `TYPE` and
    // `from_value` be written separately. That is a smaller claim than
    // "this catches an accept-everything decoder", and it is the true one.
    // It is still strictly more than the idempotence above, which an
    // accept-everything decoder satisfies for free.
    if let (ParamType::Choice(tags), Value::Text(text)) = (T::TYPE, value) {
        let declared = tags.contains(&text.as_str());
        let accepted = T::from_value(value).is_ok();
        prop_assert_eq!(
            accepted,
            declared,
            "{:?} is {} of {:?}, but from_value {} it",
            text,
            if declared { "one" } else { "not one" },
            tags,
            if accepted { "accepted" } else { "refused" }
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// **Round-trip identity for the primitives.**
    ///
    /// `Value` is deliberately smaller than JSON — no floats, no signed and
    /// unsigned split, no arbitrary-key maps (`value.rs`'s header) — so the
    /// primitives are where that smallness either holds or leaks. It holds
    /// everywhere the wire is signed and the payload is too.
    #[test]
    fn primitives_round_trip(
        flag in any::<bool>(),
        signed in any_int(),
        small in any::<u32>(),
        large in any_wire_u64(),
        text in any_text_value(),
        character in any::<char>(),
        path in any_text_value(),
    ) {
        round_trips(&flag)?;
        round_trips(&signed)?;
        round_trips(&small)?;
        round_trips(&large)?;
        round_trips(&text)?;
        round_trips(&character)?;
        round_trips(&PathBuf::from(path))?;
    }

    /// **Round-trip identity through the collection and option impls.**
    ///
    /// `Option<T>` decodes `Null` as `None`, so it is the one combinator that
    /// can lose information — for any `T` whose own encoding can *be* `Null`.
    /// No such `T` is in the vocabulary (only `Option` and `Value` encode to
    /// `Null`, and neither is nested inside an `Option` anywhere), which is
    /// what makes this law true rather than nearly true.
    #[test]
    fn collections_round_trip(
        numbers in prop::collection::vec(any_int(), 0..8),
        maybe in prop::option::of(any_text_value()),
        nested in prop::collection::vec(prop::option::of(any::<u32>()), 0..6),
        args in any_args(),
        value in any_value(),
    ) {
        round_trips(&numbers)?;
        round_trips(&maybe)?;
        round_trips(&nested)?;
        round_trips(&args)?;
        round_trips(&value)?;
    }

    /// **Round-trip identity for the vocabulary's own shapes** — the records,
    /// the tagged unions and the choices every door actually carries.
    #[test]
    fn vocabulary_types_round_trip(
        position in any_position(),
        span in any_span(),
        file_span in any_file_span(),
        target in any_target(),
        pane in prop::sample::select(vec![
            PaneRef::Focused {},
            PaneRef::Id { id: PaneId(3) },
            PaneRef::Direction { direction: Direction::Up },
            PaneRef::Next {},
            PaneRef::Prev {},
        ]),
        binding in any_binding(),
        region in any_region_spec(),
        motion in any_motion(),
        object in any_text_object(),
        case in any_case_change(),
        actor in any_actor(),
        direction in prop::sample::select(vec![
            Direction::Left, Direction::Right, Direction::Up, Direction::Down,
        ]),
    ) {
        round_trips(&position)?;
        round_trips(&span)?;
        round_trips(&file_span)?;
        round_trips(&target)?;
        round_trips(&pane)?;
        round_trips(&binding)?;
        round_trips(&region)?;
        round_trips(&motion)?;
        round_trips(&object)?;
        round_trips(&case)?;
        round_trips(&actor)?;
        round_trips(&direction)?;
    }

    /// **The rejection law.** Any [`Value`] at all, at every payload type: an
    /// `Err`, or an `Ok` that survives its own encoding. Never a panic, and
    /// never a value the wire did not carry.
    ///
    /// This is the property a fuzzer would hammer next, stated here first. It
    /// is also the one that covers the arms `wire_union!` and `wire_record!`
    /// generate for the *wrong* shape — a record where a tag was due, a tag
    /// nothing declares, a negative where a `u32` was declared — which is most
    /// of `value.rs`'s uncovered region count.
    #[test]
    fn any_value_is_decoded_or_refused(value in any_value()) {
        decodes_or_refuses::<bool>(&value)?;
        decodes_or_refuses::<i64>(&value)?;
        decodes_or_refuses::<u32>(&value)?;
        decodes_or_refuses::<u64>(&value)?;
        decodes_or_refuses::<String>(&value)?;
        decodes_or_refuses::<char>(&value)?;
        decodes_or_refuses::<PathBuf>(&value)?;
        decodes_or_refuses::<Option<u32>>(&value)?;
        decodes_or_refuses::<Vec<i64>>(&value)?;
        decodes_or_refuses::<Args>(&value)?;
        decodes_or_refuses::<Value>(&value)?;
        decodes_or_refuses::<Position>(&value)?;
        decodes_or_refuses::<Span>(&value)?;
        decodes_or_refuses::<FileSpan>(&value)?;
        decodes_or_refuses::<Target>(&value)?;
        decodes_or_refuses::<Binding>(&value)?;
        decodes_or_refuses::<RegionSpec>(&value)?;
        decodes_or_refuses::<Motion>(&value)?;
        decodes_or_refuses::<Actor>(&value)?;
        decodes_or_refuses::<BufferId>(&value)?;
        decodes_or_refuses::<PaneRef>(&value)?;
        decodes_or_refuses::<CaseChange>(&value)?;
    }

    /// **A tag nothing declares is a [`WireError::Tag`], listing what was
    /// allowed** — for every choice type, at any text at all.
    ///
    /// The union half is the same rule and is covered by
    /// [`any_value_is_decoded_or_refused`]; this states the *message*, because
    /// `WireError::Tag`'s whole reason to exist is that a door can tell an
    /// agent what it should have said.
    #[test]
    fn an_undeclared_tag_names_what_was_allowed(tag in "[a-z-]{0,12}") {
        let value = Value::Text(tag.clone());
        if let Err(error) = Motion::from_value(&value) {
            match error {
                WireError::Tag { got, expected } => {
                    prop_assert_eq!(got, tag);
                    prop_assert!(expected.contains(&"char-left"));
                }
                other => return Err(TestCaseError::fail(format!(
                    "expected a tag error for {tag:?}, got {other}"
                ))),
            }
        }
    }
}

/// The wire is signed, and a `u64` past `i64::MAX` does not survive it.
///
/// **This is a real hole, not a curiosity, and it is recorded here because
/// nothing else records it.** `wire_unsigned!`'s `to_value` is
/// `i64::try_from(*self).unwrap_or(i64::MAX)` and the `ids!` macro's is the
/// same expression, so `BufferId(u64::MAX)` encodes as `BufferId(i64::MAX)`
/// with no error anywhere — [`Wire::to_value`] returns a `Value`, not a
/// `Result`, so saturating is the only thing it *can* do.
///
/// Why the build is nonetheless sound today, which is why this is a pinned
/// boundary rather than a failing test: the only way a value that large
/// reaches `to_value` is for phosphor itself to mint one, and every `u64` in
/// the vocabulary is a dense counter — an id or a revision. Nothing a door
/// hands *in* can be affected, because an inbound integer arrives as a
/// `Value::Int`, which is an `i64` already.
///
/// If that ever stops being true — a hash used as an id, a `u64` of flags —
/// this test is where the assumption is written down.
#[test]
fn u64_saturates_on_the_way_out_above_i64_max() {
    let past_the_wire = (i64::MAX as u64) + 1;
    assert_eq!(past_the_wire.to_value(), Value::Int(i64::MAX));
    assert_eq!(
        u64::from_value(&past_the_wire.to_value()),
        Ok(i64::MAX as u64),
        "the round trip is lossy above i64::MAX, silently"
    );
    assert_eq!(
        BufferId(u64::MAX).to_value(),
        Value::Int(i64::MAX),
        "every ids! newtype saturates the same way"
    );
    // And it holds exactly up to the edge.
    let at_the_edge = i64::MAX as u64;
    assert_eq!(u64::from_value(&at_the_edge.to_value()), Ok(at_the_edge));
}

// ===========================================================================
// P2 · Motions and spans — `input/text.rs`
// ===========================================================================

/// Every [`Motion`], checked against the enum's own declared tag list so the
/// array cannot go stale when a motion is added.
fn all_motions() -> Vec<Motion> {
    let motions = vec![
        Motion::CharLeft,
        Motion::CharRight,
        Motion::LineUp,
        Motion::LineDown,
        Motion::WordForward,
        Motion::WordBackward,
        Motion::WordEnd,
        Motion::BigWordForward,
        Motion::BigWordBackward,
        Motion::BigWordEnd,
        Motion::FindCharForward,
        Motion::FindCharBackward,
        Motion::TillCharForward,
        Motion::TillCharBackward,
        Motion::RepeatFind,
        Motion::RepeatFindReverse,
        Motion::LineStart,
        Motion::FirstNonBlank,
        Motion::LineEnd,
        Motion::BufferStart,
        Motion::BufferEnd,
        Motion::ParagraphForward,
        Motion::ParagraphBackward,
        Motion::MatchingBracket,
        Motion::ScreenTop,
        Motion::ScreenMiddle,
        Motion::ScreenBottom,
        Motion::HalfPageDown,
        Motion::HalfPageUp,
        Motion::SearchNext,
        Motion::SearchPrev,
    ];
    if let ParamType::Choice(tags) = <Motion as Wire>::TYPE {
        assert_eq!(
            motions.len(),
            tags.len(),
            "a motion was added to the enum and not to this list, so the \
             properties below stopped covering it"
        );
    }
    motions
}

fn any_motion() -> impl Strategy<Value = Motion> {
    prop::sample::select(all_motions())
}

fn all_text_objects() -> Vec<TextObject> {
    let objects = vec![
        TextObject::Word,
        TextObject::BigWord,
        TextObject::Sentence,
        TextObject::Paragraph,
        TextObject::Delimited,
        TextObject::Tag,
        TextObject::UnseenRegion,
        TextObject::Hunk,
        TextObject::Thread,
        TextObject::Block,
    ];
    if let ParamType::Choice(tags) = <TextObject as Wire>::TYPE {
        assert_eq!(objects.len(), tags.len(), "a text object is not covered");
    }
    objects
}

fn any_text_object() -> impl Strategy<Value = TextObject> {
    prop::sample::select(all_text_objects())
}

fn any_case_change() -> impl Strategy<Value = CaseChange> {
    prop::sample::select(vec![
        CaseChange::Upper,
        CaseChange::Lower,
        CaseChange::Toggle,
    ])
}

/// A buffer of lines, which is every fixture this module needs — the same
/// shape `input/text.rs`'s own unit tests use.
#[derive(Debug, Clone)]
struct Lines {
    rows: Vec<String>,
}

impl Text for Lines {
    fn lines(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(1).max(1)
    }

    fn line(&self, line: u32) -> Option<String> {
        self.rows.get((line as usize).checked_sub(1)?).cloned()
    }

    fn cursor(&self) -> Position {
        Position { line: 1, column: 1 }
    }
}

impl Lines {
    /// Characters on a line, not counting the newline.
    fn width(&self, line: u32) -> u32 {
        self.line(line)
            .map_or(0, |row| u32::try_from(row.chars().count()).unwrap_or(0))
    }

    /// A char offset into the whole buffer, lines joined by one newline.
    ///
    /// [`None`] when the position is not addressable — which is the failure
    /// [`a_motion_span_never_leaves_the_buffer`] is looking for.
    fn offset(&self, position: Position) -> Option<usize> {
        if position.line < 1 || position.line > self.lines() {
            return None;
        }
        if position.column < 1 || position.column > self.width(position.line) + 1 {
            return None;
        }
        let mut offset = 0_usize;
        for line in 1..position.line {
            offset += self.width(line) as usize + 1;
        }
        Some(offset + (position.column as usize - 1))
    }

    fn char_len(&self) -> usize {
        let mut total = 0_usize;
        for line in 1..=self.lines() {
            total += self.width(line) as usize;
        }
        total + (self.lines() as usize - 1)
    }
}

/// The alphabet the off-by-ones live in: blanks, word characters, punctuation
/// that opens and closes, quotes, sentence terminators, and three characters
/// whose char count and byte count differ.
fn any_line() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            ' ', '\t', 'a', 'b', 'Z', '_', '1', '.', '!', ',', '(', ')', '[', ']', '{', '}', '"',
            '\'', 'é', '日', '👍',
        ]),
        0..10,
    )
    .prop_map(|characters| characters.into_iter().collect())
}

/// Multi-line, unicode, empty lines, one-character lines, and a last line with
/// no newline after it — which is what `rows` means here.
fn any_buffer() -> impl Strategy<Value = Lines> {
    prop::collection::vec(any_line(), 1..6).prop_map(|rows| Lines { rows })
}

/// A position the machine could really be at: on a line, with the column on a
/// character or at the newline boundary past the last one.
///
/// That boundary is legal (`text.rs`'s [`Text`] header: *"a column may be one
/// past the last character of a line"*), and it is the domain both callers use
/// — the host applies `MoveCursor` from a clamped cursor, and the machine
/// builds an operator's span from the same place.
fn place(text: &Lines, line_seed: u32, column_seed: u32) -> Position {
    let line = line_seed % text.lines() + 1;
    let column = column_seed % (text.width(line) + 1) + 1;
    Position { line, column }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// **`clamp` is idempotent, and its output is always addressable.**
    ///
    /// The positions here are deliberately *not* valid — that is what `clamp`
    /// is for. Line zero, column zero and `u32::MAX` in either axis all arrive
    /// through a door, because `Position`'s wire form is two `u32`s and
    /// nothing between the door and here narrows them.
    #[test]
    fn clamp_is_idempotent_and_lands_on_a_character(
        text in any_buffer(),
        line in prop_oneof![Just(0_u32), Just(u32::MAX), 0_u32..12],
        column in prop_oneof![Just(0_u32), Just(u32::MAX), 0_u32..12],
    ) {
        let clamped = text::clamp(&text, Position { line, column });
        prop_assert_eq!(
            text::clamp(&text, clamped), clamped,
            "clamp is not idempotent at {:?}", Position { line, column }
        );
        prop_assert!((1..=text.lines()).contains(&clamped.line));
        prop_assert!(clamped.column >= 1);
        prop_assert!(
            clamped.column <= text.width(clamped.line).max(1),
            "clamp left the column past the last character of line {}", clamped.line
        );
    }

    /// **A motion's span never leaves the buffer, and applying it cannot
    /// lengthen it.**
    ///
    /// Stated as: both endpoints are addressable char offsets, and the start
    /// is not after the end. That is exactly what a splice needs to be
    /// well-defined — `Buffer::Delete` takes this span and removes
    /// `end - start` characters, so an endpoint that is not addressable is a
    /// panic or a wrong delete at the call site, and `end < start` is a delete
    /// of the rest of the file.
    #[test]
    fn a_motion_span_never_leaves_the_buffer(
        text in any_buffer(),
        line in any::<u32>(),
        column in any::<u32>(),
        motion in any_motion(),
        count in 1_u32..=4,
        // Always a character, and one from the buffer's own alphabet: a find
        // with no target resolves to "stay put", so `None` here would spend a
        // thirteenth of the cases proving nothing.
        target in prop::sample::select(vec!['a', 'b', '(', ')', ' ', '.', '日']),
    ) {
        let from = place(&text, line, column);
        let Some((span, kind)) =
            text::motion_span_with_target(&text, from, motion, count, Some(target))
        else {
            return Ok(());
        };
        let start = text.offset(span.start).ok_or_else(|| TestCaseError::fail(format!(
            "{motion:?} x{count} from {from:?} produced start {:?}, which is not in the buffer",
            span.start
        )))?;
        let end = text.offset(span.end).ok_or_else(|| TestCaseError::fail(format!(
            "{motion:?} x{count} from {from:?} produced end {:?}, which is not in the buffer",
            span.end
        )))?;
        prop_assert!(start <= end, "{motion:?} produced a reversed span {span:?}");
        prop_assert!(end <= text.char_len());
        if kind == SelectionKind::Line {
            prop_assert_eq!(span.start.column, 1, "a linewise span starts at column 1");
        }
    }

    /// **`char_right_operand` never runs past the end of its line** — rule
    /// `B1`, proven today by three examples.
    ///
    /// `l` as an operand reaches one place `l` as a motion may not: the
    /// newline boundary, because `3x` on a three-character line has to delete
    /// three characters (`text.rs`, citing `vim91/doc/change.txt:31-33`). What
    /// it may never do is cross into the next line, which is vim's default
    /// `'whichwrap'` and the reason `5x` two characters from the end takes two
    /// and does not join.
    #[test]
    fn char_right_as_an_operand_stays_on_its_line(
        text in any_buffer(),
        line in any::<u32>(),
        column in any::<u32>(),
        count in 1_u32..=9,
    ) {
        let from = place(&text, line, column);
        let Some((span, _)) = text::motion_span(&text, from, Motion::CharRight, count) else {
            return Ok(());
        };
        prop_assert_eq!(span.start.line, from.line);
        prop_assert_eq!(span.end.line, from.line, "the operand crossed a line boundary");
        prop_assert!(
            span.end.column <= text.width(from.line) + 1,
            "the operand ran past the newline boundary of line {}", from.line
        );
    }

    /// **The four find motions stay on their own line, and `t` lands exactly
    /// one short of `f` in whichever direction it ran.**
    ///
    /// `find_char` is *"the one place the off-by-one that separates them
    /// lives"* (`text.rs`), and *"on this line only, which is vim's rule and
    /// the reason `dt)` is safe at the end of a line"*. Both halves are one
    /// property.
    ///
    /// It is separate from [`a_motion_span_never_leaves_the_buffer`] because
    /// that property draws one motion from thirty-one and needs the character
    /// to be on the line as well — the four finds were reached rarely enough
    /// that a planted off-by-one in `find_char` survived it. Drawing only from
    /// the finds fixes the sampling, and asserting the `t`/`f` relation
    /// directly is a stronger claim than "in the buffer somewhere".
    #[test]
    fn a_find_stays_on_its_line_and_till_stops_one_short(
        text in any_buffer(),
        line in any::<u32>(),
        column in any::<u32>(),
        target in prop::sample::select(vec!['a', 'b', '(', ')', ' ', '.', '日']),
        count in 1_u32..=3,
    ) {
        let from = place(&text, line, column);
        let go = |motion| text::cursor_after_with_target(&text, from, motion, count, Some(target));
        let width = text.width(from.line);
        for motion in [
            Motion::FindCharForward,
            Motion::FindCharBackward,
            Motion::TillCharForward,
            Motion::TillCharBackward,
        ] {
            let to = go(motion);
            prop_assert_eq!(to.line, from.line, "{:?} left its line", motion);
            prop_assert!(to.column >= 1);
            prop_assert!(
                to.column <= width + 1,
                "{:?} for {:?} x{} from {:?} landed at column {}, past the {} \
                 characters of its line",
                motion, target, count, from, to.column, width
            );
        }
        // `t` is `f`'s landing minus one, and `T` is `F`'s plus one. Stated
        // only where the find lands at all: a count that cannot be met leaves
        // the cursor where it started, for both of each pair, because they run
        // the same search.
        let (find, till) = (go(Motion::FindCharForward), go(Motion::TillCharForward));
        if find != from {
            prop_assert_eq!(
                till.column + 1, find.column,
                "t did not stop one short of f for {:?} x{}", target, count
            );
        } else {
            prop_assert_eq!(till, from, "t landed where f found nothing");
        }
        let (back, till_back) = (go(Motion::FindCharBackward), go(Motion::TillCharBackward));
        if back != from {
            prop_assert_eq!(
                till_back.column, back.column + 1,
                "T did not stop one short of F for {:?} x{}", target, count
            );
        } else {
            prop_assert_eq!(till_back, from, "T landed where F found nothing");
        }
    }

    /// **A text object's span never leaves the buffer either.**
    ///
    /// The same law as [`a_motion_span_never_leaves_the_buffer`], for the
    /// other half of the operator grammar. The four agent nouns answer
    /// [`None`] here by design and are covered by the `else` — what is being
    /// checked is that the five that *do* answer, answer inside the buffer.
    #[test]
    fn an_object_span_never_leaves_the_buffer(
        text in any_buffer(),
        line in any::<u32>(),
        column in any::<u32>(),
        object in any_text_object(),
        inner in any::<bool>(),
        count in 1_u32..=3,
        delimiter in prop::option::of(prop::sample::select(vec!['(', '[', '{', '"', '\''])),
    ) {
        let from = place(&text, line, column);
        let Some((span, _)) = text::object_span(&text, from, object, inner, count, delimiter)
        else {
            return Ok(());
        };
        let start = text.offset(span.start).ok_or_else(|| TestCaseError::fail(format!(
            "{object:?} inner={inner} at {from:?} produced start {:?}", span.start
        )))?;
        let end = text.offset(span.end).ok_or_else(|| TestCaseError::fail(format!(
            "{object:?} inner={inner} at {from:?} produced end {:?}", span.end
        )))?;
        prop_assert!(start <= end, "{object:?} produced a reversed span {span:?}");
    }

    /// **`cased` never loses a character**, and on ASCII it preserves the
    /// count exactly.
    ///
    /// The law worth wanting is the stronger one — that `gU` over a selection
    /// returns exactly as many characters as it was given, so the result
    /// splices back into the span it came from. **That law is false**, and
    /// Unicode is why: `char::to_uppercase('ß')` is `"SS"`, two characters
    /// from one, and `char::to_lowercase('İ')` is `"i̇"`, also two. `Toggle`
    /// inherits both. [`cased_grows_on_a_sharp_s`] is the counterexample,
    /// pinned so the consequence is written down somewhere.
    ///
    /// What is true, and is what a caller may actually rely on: the count
    /// never *drops* (every `to_uppercase` yields at least one character), and
    /// for text with no such character — which is all ASCII — it is exact.
    #[test]
    fn cased_never_loses_a_character(
        source in prop::collection::vec(
            prop::sample::select(vec!['a', 'Z', ' ', '1', 'é', '日', 'ß', 'İ', '👍']),
            0..12,
        ).prop_map(|characters| characters.into_iter().collect::<String>()),
        case in any_case_change(),
    ) {
        let out = text::cased(&source, case);
        prop_assert!(
            out.chars().count() >= source.chars().count(),
            "cased dropped a character: {source:?} -> {out:?}"
        );
        if source.is_ascii() {
            prop_assert_eq!(
                out.chars().count(), source.chars().count(),
                "ascii is the domain where the count is exact"
            );
            prop_assert!(out.is_ascii());
        }
    }

    /// **`Upper` and `Lower` are idempotent; `Toggle` is not, and cannot be.**
    ///
    /// Idempotence is what makes `gUgU` safe. `Toggle` is an involution only
    /// where the case map is — `~~` on `ß` gives `ss`, because the uppercase
    /// of one character is two and toggling those two back lowercases both.
    #[test]
    fn upper_and_lower_are_idempotent(
        source in prop::collection::vec(
            prop::sample::select(vec!['a', 'Z', ' ', '1', 'é', '日', 'ß', 'İ', 'ǰ', '👍']),
            0..12,
        ).prop_map(|characters| characters.into_iter().collect::<String>()),
    ) {
        for case in [CaseChange::Upper, CaseChange::Lower] {
            let once = text::cased(&source, case);
            prop_assert_eq!(
                text::cased(&once, case), once.clone(),
                "{:?} is not idempotent on {:?}", case, source
            );
        }
    }
}

/// The counterexample to *"case conversion preserves character count"*,
/// pinned.
///
/// **`gU` over a selection cannot assume the result fits the span it came
/// from.** A caller that computes an `n`-character span, calls
/// [`text::cased`] and splices the result back has to re-derive the end of
/// the span from the *result's* length, not from the span's. Nothing in
/// `phosphor-core` does that today — `Buffer::SetCase` carries a span and the
/// host applies it — and this test exists so that the next caller finds the
/// rule stated rather than discovering it at a German keyboard.
///
/// **It is not a `gU`-only problem**: `gu` grows too, and `~` inherits both.
/// All four characters below were measured under this toolchain rather than
/// recalled, and each is asserted here so the claim in the doc comment is
/// checked rather than written down.
#[test]
fn cased_grows_on_a_sharp_s() {
    // `gU` — one character becomes two.
    assert_eq!(text::cased("ß", CaseChange::Upper), "SS");
    assert_eq!(text::cased("ß", CaseChange::Upper).chars().count(), 2);
    assert_eq!(text::cased("ﬁ", CaseChange::Upper), "FI");

    // `gu` — the same, in the other direction: a dotted capital I lowercases
    // to `i` plus a combining dot above.
    assert_eq!(text::cased("İ", CaseChange::Lower).chars().count(), 2);
    assert_eq!(text::cased("ǰ", CaseChange::Upper).chars().count(), 2);

    // `~` inherits both, and is therefore not an involution.
    assert_eq!(text::cased("ß", CaseChange::Toggle), "SS");
    let toggled = text::cased("ß", CaseChange::Toggle);
    assert_eq!(text::cased(&toggled, CaseChange::Toggle), "ss");
}

// ===========================================================================
// P3 · The folded log — `journal.rs`
// ===========================================================================

/// A directory that removes itself. No `tempfile` dependency: this crate is
/// dependency-free at the floor and a test is not the place to change that.
/// The same fixture `tests/journal.rs` uses, and for the same reason.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "phosphor-properties-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

/// One operation a real undo session performs, before it knows which node ids
/// exist.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// Commit a group: a new node, branching from wherever the buffer is.
    Commit { edits: u8, at: u16 },
    /// Undo, redo, or walk to a checkpoint. The seed picks an existing node.
    Goto { seed: u16 },
    /// Mark the buffer saved at the current node, or at none.
    Save { here: bool },
}

fn any_step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0_u8..3, any::<u16>()).prop_map(|(edits, at)| Step::Commit { edits, at }),
        2 => any::<u16>().prop_map(|seed| Step::Goto { seed }),
        1 => any::<bool>().prop_map(|here| Step::Save { here }),
    ]
}

/// The record stream a real writer produces, from a sequence of session steps.
///
/// **Shaped like the writer on purpose.** The fold accepts more than a writer
/// emits — [`undo::Record::Redo`] in particular can point a branch point at
/// any of its children, and only [`Folded::snapshot`] ever writes one. See
/// [`compaction_preserves_the_folded_state`] for what that costs and why the
/// generator stops here.
fn records_from(steps: &[Step]) -> Vec<undo::Record> {
    let mut out = vec![undo::Record::Origin {
        path: "/tmp/example.rs".to_owned(),
    }];
    let mut next: undo::NodeId = 1;
    let mut current: undo::NodeId = undo::ROOT;
    for step in steps {
        match *step {
            Step::Commit { edits, at } => {
                let edits = (0..edits)
                    .map(|index| undo::Edit {
                        at: usize::from(at) + usize::from(index),
                        removed: String::new(),
                        inserted: format!("x{index}"),
                    })
                    .collect();
                out.push(undo::Record::Node {
                    id: next,
                    parent: current,
                    edits,
                    before: undo::Caret {
                        offset: usize::from(at),
                        selection: None,
                    },
                    after: undo::Caret {
                        offset: usize::from(at) + 1,
                        selection: Some(undo::CharRange {
                            start: usize::from(at),
                            end: usize::from(at) + 1,
                        }),
                    },
                });
                current = next;
                next += 1;
            }
            Step::Goto { seed } => {
                let to = undo::NodeId::from(seed) % next;
                out.push(undo::Record::Cursor { to });
                current = to;
            }
            Step::Save { here } => {
                out.push(undo::Record::Saved {
                    node: here.then_some(current),
                });
            }
        }
    }
    out
}

fn any_records() -> impl Strategy<Value = Vec<undo::Record>> {
    prop::collection::vec(any_step(), 0..24).prop_map(|steps| records_from(&steps))
}

/// What a crash leaves after the last byte it managed to write.
///
/// Three shapes, and the module header names all three: *"a short tail, a torn
/// payload and the run of zeros some filesystems leave after a crash are all
/// the same answer: stop here, keep everything before."*
#[derive(Debug, Clone)]
enum Tail {
    /// Nothing — the file simply ends. Caught by the length check.
    Short,
    /// A run of zeros, which is what a filesystem that allocated the block and
    /// lost the write leaves behind. Needs the CRC: four zero bytes are a
    /// perfectly well-formed length field.
    Zeros(usize),
    /// Bytes from somewhere else. Needs the CRC for the same reason.
    Garbage(Vec<u8>),
}

impl Tail {
    fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Short => Vec::new(),
            Self::Zeros(len) => vec![0; *len],
            Self::Garbage(bytes) => bytes.clone(),
        }
    }
}

fn any_tail() -> impl Strategy<Value = Tail> {
    prop_oneof![
        2 => Just(Tail::Short),
        1 => (0_usize..24).prop_map(Tail::Zeros),
        1 => prop::collection::vec(any::<u8>(), 0..24).prop_map(Tail::Garbage),
    ]
}

/// Folds a record sequence the way [`Log::open`] does.
fn fold(records: &[undo::Record]) -> undo::History {
    let mut history = undo::History::default();
    for record in records {
        history
            .apply(record.clone())
            .expect("the generator emits only records a writer emits");
    }
    history
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// **Every record round-trips through the codec.**
    ///
    /// `Encoder` / `Decoder` are hand-rolled LEB128 and length-prefixed UTF-8
    /// (`journal.rs`'s header says why there is no `postcard`), and the two
    /// are written to be read side by side — *"every method here has exactly
    /// one counterpart there"*. This is the property that says they still do.
    #[test]
    fn every_record_round_trips_through_the_codec(records in any_records()) {
        for record in &records {
            let mut encoder = Encoder::new();
            undo::History::encode(record, &mut encoder);
            let bytes = encoder.finish();
            let back = undo::History::decode(&bytes);
            prop_assert_eq!(
                back.as_ref(), Ok(record),
                "{:?} encoded to {:?} and did not come back", record, bytes
            );
        }
    }

    /// **Arbitrary bytes decode or refuse, never panic and never invent.**
    ///
    /// A record's payload is whatever survived a crash, so the decoder's real
    /// input set is *every byte string*. The second half — that an `Ok` is a
    /// record whose own encoding is those same bytes — is what makes
    /// `Decoder::finish`'s trailing-bytes check meaningful: a decode that
    /// stopped early and ignored the rest would pass the first half and fail
    /// this one.
    #[test]
    fn arbitrary_bytes_decode_or_refuse(bytes in prop::collection::vec(any::<u8>(), 0..48)) {
        if let Ok(record) = undo::History::decode(&bytes) {
            let mut encoder = Encoder::new();
            undo::History::encode(&record, &mut encoder);
            prop_assert_eq!(
                encoder.finish(), bytes.clone(),
                "{:?} decoded to {:?}, which does not encode back to it", bytes, record
            );
        }
    }

    /// **A redundant continuation byte is refused**, pinned from the exact
    /// counterexample the `CP-3` test-depth gate produced by hand.
    ///
    /// `[5, 17, 188, 0]` decoded to `Redo { node: 17, child: 60 }`, because
    /// `[188, 0]` is `0x80|60` followed by a byte carrying nothing — a longer
    /// spelling of `[60]`. That re-encoded to `[5, 17, 60]`, so
    /// [`arbitrary_bytes_decode_or_refuse`] was **false**, and had simply never
    /// drawn one: 256 samples of short uniform bytes almost never produce a
    /// redundant continuation, and proptest reseeds from entropy every run. A
    /// property that is false and passes is worse than no property, and this one
    /// was a random red waiting for whoever ran the suite next.
    ///
    /// `Decoder::u64` now refuses it, which makes the encoding canonical — one
    /// value, one spelling — and that is the property a file format actually
    /// needs, because bytes that are not a function of the state cannot be
    /// compared or checksummed against a rebuild.
    #[test]
    fn a_redundant_continuation_byte_is_not_a_shorter_number(
        value in 0u64..=127,
    ) {
        let long = [0x80 | u8::try_from(value).expect("0..=127"), 0];
        let mut decoder = Decoder::new(&long);
        prop_assert_eq!(
            decoder.u64(),
            Err(DecodeError::NonMinimalVarint),
            "{:?} is a two-byte spelling of {}, and one value gets one spelling",
            long,
            value
        );

        // The minimal spelling of the same value still decodes, so the rule
        // rejects the redundancy rather than the number.
        let short = [u8::try_from(value).expect("0..=127")];
        let mut decoder = Decoder::new(&short);
        prop_assert_eq!(decoder.u64(), Ok(value));
    }

    /// **Compaction preserves the folded state** — [`Folded`]'s own law,
    /// *"folding a snapshot of a state produces that same state"*, over
    /// generated sessions rather than one branchy example.
    ///
    /// `Log::compact` rewrites the file as `snapshot(state)`, so a `snapshot`
    /// that loses something loses it permanently and silently. The trait's
    /// doc comment ends *"Test the law."* — this is that.
    ///
    /// **Where the law stops** is not asserted in prose here — it is measured
    /// by [`a_hand_written_redo_on_the_cursor_path_does_not_survive_compaction`],
    /// which is also why this generator models the writer rather than the
    /// record enum.
    #[test]
    fn compaction_preserves_the_folded_state(records in any_records()) {
        let state = fold(&records);
        let compacted = fold(&state.snapshot());
        prop_assert_eq!(
            &compacted, &state,
            "a snapshot of {} records did not fold back to the same state",
            records.len()
        );
        // And it is a fixed point: compacting twice changes nothing.
        prop_assert_eq!(fold(&compacted.snapshot()), compacted);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// **For any log, any truncation of it and any garbage left after the
    /// cut, recovery yields a valid prefix of the records the writer wrote**
    /// — never an error, and never a record nobody appended.
    ///
    /// This is `T030`'s acceptance criterion stated as a law. The existing
    /// tests build three tails by hand — five bytes of a sixth frame, a kill
    /// mid-flood, a clean exit — and each proves the mechanism at one offset.
    /// A crash can land at *any* offset, including inside a frame's length
    /// field, inside its CRC, inside a multi-byte character in a payload, and
    /// exactly on a boundary.
    ///
    /// **The [`Tail`] is not decoration, and this property was wrong without
    /// it.** A plain truncation is caught by the length check alone — the
    /// declared length runs past the end of the file, so `scan` stops there
    /// and the CRC is never consulted. Deleting the checksum comparison
    /// outright left every assertion here passing. What needs the CRC is the
    /// case `journal.rs`'s header names: *"the run of zeros some filesystems
    /// leave after a crash"*, and a torn payload that something else has since
    /// written over. Both are generated below, and with them the same planted
    /// deletion fails in one case.
    ///
    /// What is asserted at each: the open succeeds, the recovered state is the
    /// fold of the first `recovery.records` records, and the bytes are
    /// accounted for. The middle one is the strong half — it says the
    /// survivors are a *prefix*, not merely a subset.
    #[test]
    fn any_truncation_recovers_a_prefix(
        records in any_records(),
        cut in 0.0_f64..1.0,
        tail in any_tail(),
    ) {
        let dir = TempDir::new("torn");
        let path = dir.join("undo.journal");

        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in &records {
            log.append(record.clone()).expect("a writer's record is appendable");
        }
        log.sync().expect("fsync");
        let full = fs::metadata(&path).expect("metadata").len();
        drop(log);

        // Anywhere in the frames region — the header is a separate contract,
        // pinned by `a_truncated_header_is_not_a_journal`.
        let header = 16_u64;
        let at = header + ((full - header) as f64 * cut) as u64;
        truncate(&path, at);
        let junk = tail.bytes();
        append(&path, &junk);
        let handed = at + junk.len() as u64;

        let (recovered, recovery) = UndoLog::open(&path).map_err(|error| TestCaseError::fail(
            format!("truncating {full} bytes to {at} with a {tail:?} tail \
                     made the log unopenable: {error}")
        ))?;
        let kept = usize::try_from(recovery.records).expect("a small count");
        prop_assert!(
            kept <= records.len(),
            "recovery claims {} records from a log of {}", kept, records.len()
        );
        prop_assert_eq!(
            recovered.state(), &fold(&records[..kept]),
            "the recovered state is not the fold of the first {} records", kept
        );
        // The accounting identity: `open` truncates the file to the last good
        // boundary, so what is left on disk plus what it says it discarded is
        // exactly what it was handed. This is the half that says the next
        // append cannot land after garbage.
        let left = fs::metadata(&path).expect("metadata").len();
        prop_assert_eq!(
            left + recovery.discarded_bytes, handed,
            "recovery neither kept nor discarded {} bytes",
            handed as i64 - (left + recovery.discarded_bytes) as i64
        );
        prop_assert!(left >= header, "the header survived");
    }

    /// **A compaction through the real file preserves the state a reopen
    /// reads back.**
    ///
    /// [`compaction_preserves_the_folded_state`] is the law in memory; this is
    /// the law through `rename`. It also exercises the half that only a file
    /// can: `Journal::rewrite` writes a sibling, `fsync`s it and renames over
    /// the top, then reopens the path for appending — and an append after a
    /// compaction landing in the wrong place is a corruption the in-memory
    /// property cannot see.
    #[test]
    fn compaction_through_the_file_preserves_the_state(
        records in any_records(),
        after in any_records(),
    ) {
        let dir = TempDir::new("compact");
        let path = dir.join("undo.journal");

        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in &records {
            log.append(record.clone()).expect("appendable");
        }
        let before = log.state().clone();
        log.compact().expect("compaction");
        prop_assert_eq!(log.state(), &before, "compaction moved the live state");
        drop(log);

        let (reopened, recovery) = UndoLog::open(&path).expect("a compacted journal reopens");
        prop_assert!(recovery.is_clean(), "a compacted journal has no torn tail");
        prop_assert_eq!(reopened.state(), &before, "the compacted file folds elsewhere");

        // An append after the compaction has to land after the new records,
        // not after the old file's length.
        let mut log = reopened;
        let mut expected = before;
        for record in &after {
            // The generator's ids restart per sequence, so only the records
            // this state can still accept are appended.
            if log.append(record.clone()).is_ok() {
                expected.apply(record.clone()).expect("applied above");
            }
        }
        drop(log);
        let (again, recovery) = UndoLog::open(&path).expect("reopens");
        prop_assert!(recovery.is_clean());
        prop_assert_eq!(again.state(), &expected);
    }
}

/// The exact edge of [`compaction_preserves_the_folded_state`], measured.
///
/// [`Folded::snapshot`] emits its `Redo` fix-ups **before** the trailing
/// `Cursor`, and `apply(Cursor)` re-points every node on the path to `current`
/// (`History::walk_to`). So a state whose redo pointers disagree with its own
/// path to `current` loses that disagreement the first time it is compacted —
/// silently, which is the failure `Folded`'s doc comment warns about in the
/// sentence that ends *"Test the law."*
///
/// **Nothing writes such a log today, and that is the whole reason this is a
/// boundary rather than a bug.** During a session `redo_child` moves in
/// exactly two places — committing a node (which points the parent at it) and
/// walking (which re-points the path) — and both leave the path consistent by
/// construction. `Record::Redo` is written by `snapshot` and by nothing else,
/// and `snapshot` only emits one where the pointer differs from the newest
/// child. So the fold accepts strictly more than any writer emits, and this is
/// the gap.
///
/// It matters for `T044`. That task supplies its own [`Folded`] and inherits
/// this file format; if a future writer ever emits a `Redo` directly — or if a
/// truncating compaction lands and reorders `snapshot`'s output — the law
/// above stops holding and nothing else would say so.
///
/// The state below is reached by hand-writing four records, which is what a
/// corrupt or a hand-edited journal is.
#[test]
fn a_hand_written_redo_on_the_cursor_path_does_not_survive_compaction() {
    let records = vec![
        undo::Record::Node {
            id: 1,
            parent: undo::ROOT,
            edits: Vec::new(),
            before: undo::Caret::default(),
            after: undo::Caret::default(),
        },
        undo::Record::Node {
            id: 2,
            parent: undo::ROOT,
            edits: Vec::new(),
            before: undo::Caret::default(),
            after: undo::Caret::default(),
        },
        // The buffer walks to node 2, so the root's redo child becomes 2…
        undo::Record::Cursor { to: 2 },
        // …and then a record nothing writes points it back at node 1.
        undo::Record::Redo { node: 0, child: 1 },
    ];
    let state = fold(&records);
    assert_eq!(state.current(), 2);
    assert_eq!(
        state.nodes()[0].redo_child,
        Some(1),
        "the fold accepted the hand-written pointer"
    );

    let compacted = fold(&state.snapshot());
    assert_ne!(
        compacted, state,
        "if this now passes, snapshot round-trips more than it used to and \
         compaction_preserves_the_folded_state can generate Redo records"
    );
    assert_eq!(
        compacted.nodes()[0].redo_child,
        Some(2),
        "the trailing Cursor's walk overwrote the Redo record that preceded it"
    );
    // Everything else about the state does survive — the loss is exactly one
    // pointer, not the history.
    assert_eq!(compacted.current(), state.current());
    assert_eq!(compacted.nodes().len(), state.nodes().len());
}

/// The torn-tail contract starts after the header, and this is what happens
/// before it.
///
/// **A crash inside the first sixteen bytes loses the journal**, with
/// `Error::NotAJournal` rather than the empty-but-usable log a zero-length
/// file gets. The window is one `write_all` of sixteen bytes at creation, so
/// it is narrow, and the outcome — a fresh history instead of a restored one —
/// is the same one a missing file gives, reached by a scarier route. Recorded
/// rather than fixed: changing it means deciding whether a file whose magic is
/// half-written is *this* journal, and that is a format question, not a test
/// one.
#[test]
fn a_truncated_header_is_not_a_journal() {
    let dir = TempDir::new("header");
    let path = dir.join("undo.journal");
    let (log, _) = UndoLog::open(&path).expect("a fresh journal opens");
    drop(log);

    // Zero bytes is the ordinary first run: a header is written and the log is
    // empty.
    truncate(&path, 0);
    let (log, recovery) = UndoLog::open(&path).expect("an empty file is a first run");
    assert!(recovery.is_clean());
    assert_eq!(log.state(), &undo::History::default());
    drop(log);

    // One byte short of a header is not.
    truncate(&path, 15);
    let error = UndoLog::open(&path).expect_err("a torn header is refused");
    assert!(
        format!("{error}").contains("not a phosphor journal"),
        "expected NotAJournal, got {error}"
    );
}

fn truncate(path: &Path, len: u64) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncate")
        .set_len(len)
        .expect("truncate");
}

fn append(path: &Path, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open for append")
        .write_all(bytes)
        .expect("append");
}
