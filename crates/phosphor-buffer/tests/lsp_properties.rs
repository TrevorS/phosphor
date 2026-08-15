//! `T036`'s laws, over generated input rather than chosen examples.
//!
//! `tests/lsp.rs` is the readable file and stays that way: it names the cases —
//! an emoji, a CRLF line, a restart, a hung server — and says what each does.
//! This one asks the same two questions of inputs nobody picked, because both
//! subjects have the shape a property is for.
//!
//! # Two laws, and why each needs generating
//!
//! 1. **The state machine reaches no state it has no right to.** Five states
//!    and five events is twenty-five transitions, but the bugs are not in one
//!    transition — they are in *sequences*: a reply arriving after a restart, a
//!    crash reported after a stop, a restart of something that never started.
//!    Enumerating pairs would miss all three. The law is stated as an invariant
//!    over the whole fold, checked at every step, so any prefix of any sequence
//!    is also covered.
//!
//! 2. **The UTF-16 conversion round-trips.** The alphabet is where the work is:
//!    [`GLYPHS`] mixes one-, two-, three- and four-byte characters on purpose,
//!    so a generated line is *usually* mixed rather than usually ASCII. A
//!    generator over `char` would produce ASCII almost every time and prove
//!    nothing — the same reason `undo_properties.rs` generates anchors instead
//!    of offsets.
//!
//! **What the second law is not.** It is not *"every code unit maps back to
//! itself"*: half of a surrogate pair names no column, so it canonicalises to
//! the character that contains it. Stating the law that way would have to be
//! either false or restricted to ASCII, and the restricted version is exactly
//! the test that would have passed while the bug shipped.

use phosphor_buffer::lsp::{
    Failure, ServerEvent, ServerIdentity, ServerState, column_from_utf16, line_at,
    position_from_lsp, position_to_lsp, span_from_lsp, utf16_from_column, utf16_len,
};
use phosphor_core::request::Position;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// The alphabet a generated line is built from: one byte, two, three, and two
/// astral characters that are two UTF-16 units each.
///
/// `\r` is in the list deliberately — a stray carriage return inside a line is
/// something a real file contains, and [`line_at`] only strips a *terminating*
/// one.
const GLYPHS: [&str; 10] = ["a", "Z", " ", "ß", "é", "中", "✓", "🎉", "𝄞", "\r"];

fn line() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(GLYPHS.as_slice()), 0..24)
        .prop_map(|glyphs| glyphs.concat())
}

/// A whole document: several generated lines, joined by the terminator under
/// test.
fn text(terminator: &'static str) -> impl Strategy<Value = String> {
    prop::collection::vec(line(), 1..6).prop_map(move |lines| lines.join(terminator))
}

/// One generated event. The identity is constant because nothing in the law
/// depends on its contents — what varies is *when* an `Initialized` arrives.
fn event() -> impl Strategy<Value = ServerEvent> {
    prop_oneof![
        Just(ServerEvent::Attached),
        Just(ServerEvent::Initialized(ServerIdentity {
            name: "generated".to_owned(),
            version: None,
        })),
        Just(ServerEvent::Failed(Failure::Exited("generated".to_owned()))),
        Just(ServerEvent::Failed(Failure::Timeout)),
        Just(ServerEvent::Restarted),
        Just(ServerEvent::Stopped),
    ]
}

proptest! {
    // -----------------------------------------------------------------------
    // The state machine
    // -----------------------------------------------------------------------

    /// **The law: under any sequence of events, nothing *becomes* `Ready`
    /// except an `Initialized` arriving while it was `Starting`.**
    ///
    /// Both halves matter and neither is checkable at a single transition. The
    /// first is what makes `is_ready` mean *"there is a process listening"*;
    /// the second is what stops a reply from a killed process promoting its
    /// replacement, which is the failure a restart creates and nothing else
    /// does.
    ///
    /// **"Becomes", and the word was earned.** Written as *"whenever the state
    /// is `Ready`, the previous state was `Starting`"* this failed on its
    /// third generated input — `[Attached, Initialized, Initialized]` — and the
    /// implementation was right: a duplicate response leaves a ready server
    /// ready, which is a no-op rather than a promotion. The law is about the
    /// **edge into** `Ready`, and stating it about the state instead would have
    /// forced the code to either reset a live server or track a flag it has no
    /// use for.
    #[test]
    fn ready_is_only_ever_reached_from_starting_by_initialize(
        events in prop::collection::vec(event(), 0..40),
    ) {
        let mut state = ServerState::NotStarted;
        for event in &events {
            let before = state.clone();
            state = state.after(event);
            if state.is_ready() && !before.is_ready() {
                prop_assert!(
                    matches!(event, ServerEvent::Initialized(_)),
                    "became Ready on {:?}",
                    event
                );
                prop_assert_eq!(
                    before,
                    ServerState::Starting,
                    "became Ready from a state that was not Starting"
                );
            }
        }
    }

    /// **The law: a stop is final until something asks for a start.** Once
    /// `Stopped`, only `Attached` or `Restarted` moves the state — in
    /// particular a `Failed`, which is what the EOF after `exit` produces,
    /// leaves it alone.
    ///
    /// The consequence worth naming: a clean shutdown can never be drawn as a
    /// crash, however many times the transport reports the pipe closing.
    #[test]
    fn a_stop_is_never_turned_into_a_crash(
        events in prop::collection::vec(event(), 0..40),
    ) {
        let mut state = ServerState::NotStarted;
        for event in &events {
            let before = state.clone();
            state = state.after(event);
            if before == ServerState::Stopped
                && !matches!(event, ServerEvent::Attached | ServerEvent::Restarted)
            {
                prop_assert_eq!(
                    &state,
                    &ServerState::Stopped,
                    "a stopped server moved on {:?}",
                    event
                );
            }
        }
    }

    /// **The law: a restart always lands on `Starting`, from every state.**
    /// `restart-language-server` is `Allow`, so it is reachable from a keymap
    /// at any moment — including on a language that never had a server.
    #[test]
    fn a_restart_from_anywhere_is_a_start(
        events in prop::collection::vec(event(), 0..40),
    ) {
        let mut state = ServerState::NotStarted;
        for event in &events {
            state = state.after(event);
            if matches!(event, ServerEvent::Restarted) {
                prop_assert_eq!(&state, &ServerState::Starting);
            }
        }
        // And from every state directly, which the fold may not have visited.
        for state in [
            ServerState::NotStarted,
            ServerState::Starting,
            ServerState::Ready(ServerIdentity { name: "x".to_owned(), version: None }),
            ServerState::Crashed(Failure::Timeout),
            ServerState::Stopped,
        ] {
            prop_assert_eq!(state.after(&ServerEvent::Restarted), ServerState::Starting);
        }
    }

    // -----------------------------------------------------------------------
    // The UTF-16 seam
    // -----------------------------------------------------------------------

    /// **The law: every column round-trips through UTF-16, exactly.**
    ///
    /// This is the direction that has to be exact, because it is the one the
    /// editor uses to *ask* — a completion request at the cursor, a definition
    /// at the cursor. A column that comes back as a different column is a
    /// request about the wrong place.
    ///
    /// Checked past the end of the line as well, which is where a cursor sits
    /// in insert mode at end of line, and where a naive implementation clamps.
    #[test]
    fn every_column_round_trips_through_utf16(line in line(), column in 1_u32..40) {
        let units = utf16_from_column(&line, column);
        prop_assert_eq!(
            column_from_utf16(&line, units),
            column,
            "column {} became unit {} became something else on {:?}",
            column,
            units,
            line
        );
    }

    /// **The law: a code unit that starts a character round-trips too, and one
    /// that does not lands on the character containing it.**
    ///
    /// The second half is the honest statement of the surrogate case, and it is
    /// checked as *idempotence*: converting a mid-pair offset to a column and
    /// back gives the pair's first unit, and doing it again changes nothing.
    #[test]
    fn a_code_unit_round_trips_or_canonicalises(line in line(), character in 0_u32..40) {
        let column = column_from_utf16(&line, character);
        let back = utf16_from_column(&line, column);
        prop_assert!(back <= character, "canonicalisation may only move left");
        prop_assert_eq!(
            column_from_utf16(&line, back),
            column,
            "the second conversion changed the answer"
        );
        prop_assert_eq!(
            utf16_from_column(&line, column_from_utf16(&line, back)),
            back,
            "canonicalisation is not idempotent"
        );
    }

    /// **The law: reading left to right never goes backwards.** A monotone
    /// conversion is what makes a span's end never precede its start, which is
    /// the invariant every consumer of a diagnostic assumes without checking.
    #[test]
    fn the_conversion_is_monotone(line in line()) {
        let mut last = 0;
        for character in 0..(utf16_len(&line) + 4) {
            let column = column_from_utf16(&line, character);
            prop_assert!(column >= last, "column went backwards at unit {character}");
            last = column;
        }
    }

    /// **The law: a span is a span.** Whatever the server sends, the converted
    /// start never comes after the converted end.
    #[test]
    fn a_converted_span_never_inverts(
        body in text("\n"),
        first in 0_u32..8,
        second in 0_u32..8,
        line_a in 0_u32..6,
        line_b in 0_u32..6,
    ) {
        let (start, end) = if (line_a, first) <= (line_b, second) {
            ((line_a, first), (line_b, second))
        } else {
            ((line_b, second), (line_a, first))
        };
        let span = span_from_lsp(&body, phosphor_buffer::lsp::lsp_types::Range {
            start: phosphor_buffer::lsp::lsp_types::Position { line: start.0, character: start.1 },
            end: phosphor_buffer::lsp::lsp_types::Position { line: end.0, character: end.1 },
        });
        prop_assert!(span.start <= span.end, "{span:?} inverted");
    }

    /// **The law: line endings are not content.** The same document written
    /// with `\n` and with `\r\n` converts every position identically — the
    /// `\r` is a terminator, and counting it would put a phantom column at the
    /// end of every line of a CRLF file.
    ///
    /// This is the one law here that a Windows checkout would have found the
    /// hard way, on every file.
    #[test]
    fn crlf_and_lf_convert_alike(lines in prop::collection::vec(line(), 1..6), character in 0_u32..12) {
        // A line whose own content ends in `\r` is excluded: joined with
        // `\r\n` it produces `\r\r\n`, and the second `\r` genuinely *is*
        // content on the LF side and part of nothing on the CRLF side. The
        // ambiguity is the file format's, not the conversion's.
        prop_assume!(lines.iter().all(|line| !line.ends_with('\r')));
        let lf = lines.join("\n");
        let crlf = lines.join("\r\n");
        for line in 0..lines.len() as u32 {
            let at = phosphor_buffer::lsp::lsp_types::Position { line, character };
            prop_assert_eq!(
                position_from_lsp(&lf, at),
                position_from_lsp(&crlf, at),
                "line {} differs between LF and CRLF",
                line
            );
        }
    }

    /// **The law: a line's text never contains its own terminator.**
    ///
    /// Stated over CRLF, which is the case that can get it wrong, and with the
    /// same exclusion `crlf_and_lf_convert_alike` needs: a line whose *content*
    /// ends in `\r` produces `\r\r\n` in the file, and [`line_at`] strips
    /// exactly one — which is right, and which this law cannot distinguish from
    /// a terminator surviving. The ambiguity belongs to the file, not to the
    /// function. Found by generating it.
    #[test]
    fn a_line_is_never_handed_back_with_its_terminator(lines in prop::collection::vec(line(), 1..6)) {
        prop_assume!(lines.iter().all(|line| !line.ends_with('\r')));
        let body = lines.join("\r\n");
        for line in 0..8_u32 {
            let text = line_at(&body, line);
            prop_assert!(!text.contains('\n'), "{:?} spans a line break", text);
            prop_assert!(!text.ends_with('\r'), "{:?} kept its terminator", text);
        }
    }

    /// **The law: a position is always inside the coordinate system.** Lines and
    /// columns are 1-based on phosphor's side and 0-based on the server's, and
    /// the conversion may never produce a zero — a `Position { line: 0 }` is not
    /// a position, it is an off-by-one waiting to be indexed with.
    #[test]
    fn a_converted_position_is_never_zero(body in text("\n"), line in 0_u32..8, character in 0_u32..12) {
        let at = position_from_lsp(&body, phosphor_buffer::lsp::lsp_types::Position { line, character });
        prop_assert!(at.line >= 1, "line {} is not 1-based", at.line);
        prop_assert!(at.column >= 1, "column {} is not 1-based", at.column);
        let back = position_to_lsp(&body, at);
        prop_assert_eq!(back.line, line, "the line survived the round trip");
        prop_assert_eq!(
            position_from_lsp(&body, back),
            at,
            "and so did the column, up to canonicalisation"
        );
    }

    /// **The law: phosphor's own positions survive the trip out and back.**
    /// The direction the editor uses to ask a question.
    #[test]
    fn a_phosphor_position_survives_the_trip_to_the_server(
        body in text("\n"),
        line in 1_u32..8,
        column in 1_u32..12,
    ) {
        let at = Position { line, column };
        let back = position_from_lsp(&body, position_to_lsp(&body, at));
        prop_assert_eq!(back, at);
    }
}
