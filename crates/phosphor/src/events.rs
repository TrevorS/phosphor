//! The one event queue — many producers, one loop.
//!
//! `Component Breakdown.dc.html`, *Event & data flow*: **"Async: tokio; ACP
//! stream, LSP, tree-sitter re-parse, and VCS polls post events into the same
//! queue as input — no widget ever blocks."** This module is that queue for
//! **three of those four producers**; two of the three are wired — the terminal
//! and, since `S4`'s wiring pass, the LSP client (`crate::lsp::sink`) — and the
//! missing fourth is [below](#what-this-queue-cannot-carry-yet) rather than
//! papered over.
//!
//! Owned by `spine`.
//!
//! # Why the queue lands before the first producer that needs one
//!
//! Because this build has already shipped the other order twice, and written
//! both down. `T090`'s own entry says windows A and B produced *"a widget layer
//! with no application around it"* — `main.rs` was `fn main() {}` and every
//! screen tape died on `Require phosphor`. `TEAM.md`'s concurrency rule 2 is the
//! second: sixteen agents, `just gate` green, and pressing `SPC` did nothing,
//! because the file that composes what they built went to phase 2.
//!
//! An LSP server sends **unsolicited** messages. Diagnostics arrive when the
//! server has them, not when the editor asks — so a client built against a loop
//! whose only producer is a blocking `event::read()` has nowhere to put what it
//! produces, and would be the third complete, tested, green subsystem with no
//! application around it. This is that failure prevented rather than recorded.
//!
//! # No runtime here, on purpose
//!
//! There is no `tokio` in this module and no async anything. [`std::sync::mpsc`]
//! is a multi-producer channel that is in every build already and costs no
//! manifest line, and the loop's requirement is one blocking `recv` — not a
//! scheduler. `S4`'s LSP client (`T036`) brings its own runtime and calls
//! [`Poster::post`] from whatever thread it likes; nothing here has to learn
//! that happened.
//!
//! What that buys is the reason to do it this way: **every test below runs on a
//! channel, with no terminal and no server.** A queue whose tests need a runtime
//! to start is a queue nobody can prove anything about until the runtime lands.
//!
//! # The extension point, and why it is an [`Action`]
//!
//! [`AppEvent`] has exactly two variants and the second one is where every
//! future producer arrives. Two decisions make it extensible without editing the
//! loop's terminal arms — which is the whole point, because the agent that adds
//! a producer is not the agent that owns `main.rs`:
//!
//! 1. **The payload is an `Action`**, the vocabulary the Component Breakdown
//!    already names as the single mutation API (*"Actions are the single
//!    mutation API … Steel and MCP invoke the same enum"*). An LSP client posts
//!    `Action::Lsp(LspAction::IngestDiagnostics { … })` — a capability that is
//!    **already registered**, with `T040` on its row — and the loop applies it
//!    through the same `Editing::act` every keystroke goes through. That match
//!    is total and its `_` arm answers with the capability's own task, so a
//!    producer whose Action has no arm yet gets a sentence naming the task
//!    rather than a silent drop. Nothing about the queue changes when `T040`
//!    adds the arm.
//! 2. **The producer names itself with a `&'static str`, not an enum variant.**
//!    An enum would put `Lsp`, `Acp` and `Vcs` in this file, so every new
//!    producer would be an edit to the file that owns the loop — the coupling
//!    this shape exists to remove.
//!
//! **What is deliberately not here: provenance.** Design Language §7 makes an
//! actor load-bearing (*"your own edits never create regions: the machine tracks
//! claude only"*), and `phosphor_core::request::Request` is the envelope that
//! carries one. [`Posted`] carries a bare `Action` instead, because `Editing`
//! applies bare Actions and there is nothing on this side that could honour an
//! actor if one were named — a field the build cannot keep is a promise, not a
//! design. It goes on when the store does (`T041`), together with the `Door` a
//! `Request` also needs and which has no variant for the loop.
//!
//! # What this queue cannot carry yet
//!
//! Recorded here because the header quotes a four-producer sentence and this
//! shape satisfies three of it. **The payload is an `Action`, and two things the
//! loop has to wake up for are deliberately not Actions.** `action.rs`'s own
//! *"What is deliberately not an Action"* names five, and two of them are wakes
//! this module would otherwise be the home for:
//!
//! * **tree-sitter reparse** — the fourth producer in the quoted sentence.
//!   There is no reparse capability in the registry and there is not meant to
//!   be: it *"has no actor, no undo meaning, and nothing to refuse"*. So a
//!   reparse finishing has no [`Posted`] to arrive as. The list in
//!   [`AppEvent::Posted`]'s own doc used to name it, which was a claim about
//!   this file that the table next door disproves.
//! * **the 80ms spinner frame and the 1s elapsed tick** — Design Language §8's
//!   *"only three things animate"*. `view::Node::Spinner` says Rust animates it
//!   *"from the mark without re-entering the VM"*, which needs the loop to draw
//!   ~12.5 times a second **while nothing is typed**; see [`Queue::recv`] for
//!   why nothing does.
//!
//! Both want the same thing and it is not an Action: a **wake** — *something
//! changed, draw again* — with a source and no payload. That variant now
//! exists ([`AppEvent::Woke`]) because `S4`'s repair pass produced the first
//! producer and the first surface for it at once: a language server's state
//! changes on the client's runtime thread and the statusline's server chip is
//! what shows it. **Neither animation is why**, and neither runs yet — the
//! spinner and the elapsed tick need a *timed* producer, and there is still
//! nothing in this process that ticks.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crossterm::event::{self, Event};
use phosphor_core::action::Action;

// ---------------------------------------------------------------------------
// What the loop wakes up for
// ---------------------------------------------------------------------------

/// One thing the loop has to wake up for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppEvent {
    /// The terminal: a key, a mouse report, a resize, a paste, a focus change.
    ///
    /// First, and first for a reason — every other producer is defined against
    /// *"the same queue as input"*, so input is the one this is measured
    /// against.
    Term(Event),
    /// Everything that is not the terminal: `S4`'s LSP client, `S6`'s ACP
    /// stream, `T071`'s VCS polls.
    ///
    /// **Three of the design's four producers, and the list is exhaustive on
    /// purpose.** A tree-sitter re-parse finishing was named here and cannot
    /// be: the payload is an [`Action`] and reparse is one of the five things
    /// `action.rs` declares is deliberately not one. See the module header.
    ///
    /// **The `expect(dead_code)` that used to be here is gone, and its going
    /// is the record.** It said this attribute *"should disappear when the
    /// first producer lands"*; `S4`'s wiring pass landed it —
    /// `crate::lsp::sink` hands `LanguageServers::start` a `Post` that builds
    /// exactly this variant — and `#[expect]` is what made that removal
    /// non-optional rather than something anybody had to remember.
    Posted(Posted),
    /// **Something changed; draw again.** No payload, and that is the
    /// definition rather than an omission.
    ///
    /// The module header reserved this variant and said what it was waiting
    /// for: *"one more `AppEvent` variant when there is a producer to post it
    /// and a surface that shows the difference; there is neither today"*. `S4`'s
    /// repair pass has both. `T036`'s server state machine is the producer —
    /// `Starting` → `Ready` → `Crashed` happens on the client's own runtime
    /// thread, on its own schedule, and is not a mutation of anything the
    /// editor owns — and the statusline's server chip (`7c`'s
    /// `rust-analyzer ✓`) is the surface. Without it the chip was correct and
    /// **stale**: a server that failed to spawn said so on the next keystroke
    /// and a server that became ready never said so at all, because this loop
    /// draws when an event arrives and nothing arrived.
    ///
    /// It is deliberately **not** a [`Posted`] with some no-op Action in it.
    /// `action.rs`'s *"what is deliberately not an Action"* is the rule: a wake
    /// has no actor, no undo meaning and nothing to refuse, so an Action for it
    /// would be a capability the doors would have to rate and the registry
    /// would have to name. The spinner frame and the elapsed tick are the other
    /// two waiting on this variant, and they arrive the same way.
    Woke(
        /// Which producer woke it — the same string [`Posted::source`] carries,
        /// and for the same reason.
        &'static str,
    ),
}

impl AppEvent {
    /// Whether this is a resize that a later event can make stale.
    ///
    /// The one predicate [`coalesce`] turns on, and the reason it is a method
    /// rather than a `matches!` at the call site: *"only a resize is droppable"*
    /// is the safety property of this module, and it is stated once.
    const fn is_resize(&self) -> bool {
        matches!(self, Self::Term(Event::Resize(..)))
    }
}

/// An event from a producer that is not the terminal.
///
/// See the module header for why the payload is an [`Action`] and why the
/// source is a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Posted {
    /// Which producer posted it — `"lsp"`, `"acp"`, `"vcs"`.
    ///
    /// It exists so that an Action the binary does not apply yet says *which
    /// subsystem asked for it*. That is the difference between a debuggable
    /// refusal and one that names a task nobody can trace back to a caller.
    pub(crate) source: &'static str,
    /// What it is asking the editor to do.
    pub(crate) action: Action,
}

// ---------------------------------------------------------------------------
// The two halves
// ---------------------------------------------------------------------------

/// The producer half: cloneable, [`Send`], and the only way into the queue.
///
/// Hand a clone to every producer. Whoever holds the last one keeps the loop
/// alive — see [`Queue`] for why that is the shutdown rule and not an accident.
#[derive(Debug, Clone)]
pub(crate) struct Poster(Sender<AppEvent>);

impl Poster {
    /// Posts one event, and says whether anyone is still listening.
    ///
    /// `false` means the loop has ended and its [`Queue`] is dropped; a producer
    /// that sees it should stop rather than spin. Nothing is queued for a loop
    /// that no longer exists, so this cannot be made infallible by waiting.
    pub(crate) fn post(&self, event: AppEvent) -> bool {
        self.0.send(event).is_ok()
    }
}

/// The consumer half: **the loop's one blocking point**.
///
/// **This holds no [`Sender`] of its own, deliberately.** If it did, [`recv`]
/// could never observe that every producer is gone, and an editor whose terminal
/// reader died would block forever with no key able to reach it — the one state
/// a modal editor must not be able to enter. Keeping only the receiving end
/// makes *"nothing can ever wake this loop again"* an answer rather than a hang.
///
/// [`recv`]: Queue::recv
#[derive(Debug)]
pub(crate) struct Queue {
    events: Receiver<AppEvent>,
}

/// Opens the queue: the loop's end, and the first poster.
pub(crate) fn open() -> (Queue, Poster) {
    let (sender, events) = mpsc::channel();
    (Queue { events }, Poster(sender))
}

impl Queue {
    /// Blocks until something happens, then hands back the one event to handle
    /// this turn. [`None`] when every producer is gone.
    ///
    /// **No timeout, no tick, no sleep.** A quiet editor is parked in `recv`
    /// using no CPU, exactly as it was parked in `event::read` before there was
    /// a queue. The drain behind it ([`coalesce`]) reads only what is *already*
    /// queued, so nothing here waits for something that might arrive.
    ///
    /// **That is a cost, not only a virtue, and it is unchanged rather than
    /// new.** A loop that only wakes for a producer does not redraw while
    /// nothing is typed, so Design Language §8's two timed animations — the
    /// 80ms spinner frame and the 1s elapsed tick — cannot run from here. The
    /// blocking `event::read` this replaces had the same hole; what is new is
    /// that the queue is the mechanism that could close it, and does not yet.
    /// The module header says what closing it takes and why it is not a
    /// [`Posted`].
    pub(crate) fn recv(&self) -> Option<AppEvent> {
        let first = self.events.recv().ok()?;
        // `try_recv` folds "empty" and "every producer is gone" into the same
        // `None`, which is what the drain wants: both mean *nothing is queued
        // behind this*, and a disconnect must still hand back the event already
        // in hand rather than lose it.
        Some(coalesce(first, || self.events.try_recv().ok()))
    }
}

// ---------------------------------------------------------------------------
// The terminal producer
// ---------------------------------------------------------------------------

/// Starts the thread that blocks on the terminal and posts what it reads.
///
/// **Call this after `Term::new()`, never before, and this is crossterm's own
/// rule rather than a guess.** `phosphor-term` settles the keyboard protocol
/// with `supports_keyboard_enhancement`, which writes `ESC [ ? u ESC [ c` and
/// reads the reply back through the *same* internal event source `event::read`
/// drains (`crossterm-0.29.0/src/terminal/sys/unix.rs`,
/// `query_keyboard_enhancement_flags_raw`). Its doc says so in as many words:
/// *"this function will block and possibly time out while `crossterm::event::read`
/// or `crossterm::event::poll` are being called."* A reader thread started first
/// takes that reply, negotiation times out, and the editor falls back to
/// `KeyboardProtocol::Legacy` on a terminal that supports kitty — `T027`'s
/// degradation path firing on hardware that does not need it.
///
/// **Detached on purpose: nothing joins it.** It is parked inside a blocking
/// `read()` whenever the user is not typing, so there is no point at which it
/// could be asked to stop cooperatively; the process exiting is what ends it,
/// which is exactly what happened to the terminal read this replaces. It drops
/// its `Poster` on the way out, so a reader that dies is visible to
/// [`Queue::recv`] rather than silent.
pub(crate) fn read_terminal(poster: Poster) {
    drop(thread::spawn(move || {
        // A read that fails is a terminal that is gone — there is nothing to
        // report it *to*, since the report would be drawn on it.
        while let Ok(event) = event::read() {
            if !poster.post(AppEvent::Term(event)) {
                return;
            }
        }
    }));
}

// ---------------------------------------------------------------------------
// Resize coalescing
// ---------------------------------------------------------------------------

/// Drops resize events that a newer one has already superseded.
///
/// **`§22`: dragging a window edge from 120 columns to 80 on a 3.3 MB buffer
/// was 5.7 seconds of solid CPU**, and one late frame per column. A soft-wrap
/// rebuild is 41 ns/character, dead linear and uncached — 138 ms at that size —
/// and the loop pays it once per turn in which the width changed. Forty columns
/// is forty rebuilds.
///
/// **This does not make the rebuild cheaper; it makes fewer of them happen.**
/// Nothing here is a debounce: there is no timer and nothing waits. It reads
/// only what is *already queued* and drops the resizes it can prove are stale,
/// which is every one with another event behind it.
///
/// **It is self-correcting, which is the property worth having.** Events pile
/// up only because the rebuild is slower than the drag, so the bigger the
/// buffer the more this skips, and on a buffer small enough to wrap between
/// two events it does nothing at all.
///
/// **Nothing is dropped but a size.** A resize's entire content is the new
/// size, and the loop does not read it — it calls `term.size()` at the top of
/// every turn and gets the current one. The first event that is not a resize
/// stops the drain and is returned to be handled normally, so no keystroke and
/// **no posted event** is ever swallowed. Invariant 3 is untouched: the state
/// you land in is the size you asked for, reached without drawing the ones you
/// dragged through.
///
/// # What the queue changed here, and what it did not
///
/// The rule is the same one; two things about it moved.
///
/// * **It coalesces [`AppEvent`]s, not `crossterm::Event`s.** It has to: the
///   drain hands back the first non-resize it meets, and with a second producer
///   that event can be an LSP push. A version that only understood terminal
///   events would have to either drop it or leave it behind a resize it had
///   already dropped.
/// * **The [`io::Result`](std::io::Result) is gone.** It carried the failure of
///   `event::poll(Duration::ZERO)`, and the terminal is on the other side of a
///   channel now — a `try_recv` that comes back empty and one that comes back
///   disconnected are both *"nothing queued"*, and neither is an error the loop
///   could act on. The source is still a closure, which is what makes this
///   provable against a queue rather than against a real drag: a pty harness
///   cannot exercise it, because the slave fd is moved into the child and
///   Apple's master rejects `TIOCSWINSZ`.
pub(crate) fn coalesce<F>(first: AppEvent, mut next: F) -> AppEvent
where
    F: FnMut() -> Option<AppEvent>,
{
    if !first.is_resize() {
        return first;
    }
    let mut latest = first;
    while let Some(event) = next() {
        let superseded = event.is_resize();
        latest = event;
        if !superseded {
            break;
        }
    }
    latest
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use phosphor_core::action::{Action, LspAction};
    use proptest::prelude::*;

    use super::{AppEvent, Posted, coalesce, open};

    /// `§22` — a queue of events, standing in for a drag.
    fn drain(first: AppEvent, queued: &[AppEvent]) -> AppEvent {
        let mut rest = queued.iter().cloned();
        coalesce(first, move || rest.next())
    }

    fn resize(width: u16) -> AppEvent {
        AppEvent::Term(super::Event::Resize(width, 30))
    }

    fn typed(character: char) -> AppEvent {
        AppEvent::Term(super::Event::Key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::empty(),
        )))
    }

    /// What an LSP client will post, spelled the way it will spell it: a
    /// capability that is already in the registry, with `T040` on its row.
    fn diagnostics(source: &'static str) -> AppEvent {
        AppEvent::Posted(Posted {
            source,
            action: Action::Lsp(LspAction::IngestDiagnostics {
                path: std::path::PathBuf::from("src/retry.rs"),
                diagnostics: Vec::new(),
            }),
        })
    }

    // -----------------------------------------------------------------------
    // `§22`, unchanged: the four rules the drain has always obeyed
    // -----------------------------------------------------------------------

    /// Dragging an edge queues one resize per column, and only the last is the
    /// size you are asking for. Wrapping to the ones in between is what made
    /// `§22`'s drag 5.7 seconds.
    #[test]
    fn a_drag_collapses_to_the_size_it_ended_at() {
        assert_eq!(
            drain(resize(120), &[resize(110), resize(96), resize(80)]),
            resize(80)
        );
    }

    /// The safety half, and the one worth a test of its own: dropping a stale
    /// *size* is dropping nothing, but dropping a keystroke would be a bug far
    /// worse than the one this fixes.
    #[test]
    fn a_key_behind_a_resize_is_never_swallowed() {
        assert_eq!(
            drain(resize(120), &[resize(80), typed('x')]),
            typed('x'),
            "the drain stops at the first event that is not a resize, and hands it back"
        );
    }

    #[test]
    fn a_lone_resize_with_nothing_behind_it_is_itself() {
        assert_eq!(drain(resize(80), &[]), resize(80));
    }

    /// The half `§22`'s four examples could not see, found by planting the
    /// mutation: a drain that ran to the *end* of the queue instead of stopping
    /// at the first non-resize passed all four, because in each of them the
    /// keystroke happens to be last. **Stopping is the rule, not arriving at
    /// the key** — what is behind the key belongs to the next turn of the loop,
    /// and a drain that keeps going hands back a stale size and eats the key.
    ///
    /// The property test below states this in general; this states it as the
    /// one case a reader can check by eye.
    #[test]
    fn the_drain_stops_at_the_key_rather_than_running_to_the_end() {
        assert_eq!(drain(resize(120), &[typed('x'), resize(80)]), typed('x'));
    }

    /// Nothing is read at all unless the first event is a resize — a keystroke
    /// must never cause a drain that could consume the one behind it.
    #[test]
    fn a_key_is_returned_without_touching_the_queue() {
        let mut polled = false;
        let out = coalesce(typed('a'), || {
            polled = true;
            None
        });
        assert_eq!(out, typed('a'));
        assert!(!polled, "a non-resize must not drain anything behind it");
    }

    // -----------------------------------------------------------------------
    // The second producer
    // -----------------------------------------------------------------------

    /// The rule a queue adds to `§22`: a posted event is not a size, so it
    /// stops the drain exactly the way a keystroke does. Swallowing it would
    /// lose a server message with no way to notice — there is no key to press
    /// again.
    #[test]
    fn a_posted_event_behind_a_resize_is_never_swallowed() {
        assert_eq!(
            drain(
                resize(120),
                &[resize(80), diagnostics("lsp"), resize(64), typed('x')]
            ),
            diagnostics("lsp"),
            "the drain stops at the posted event and leaves the rest queued"
        );
    }

    /// And it does not consume the keystroke queued behind *it* either: a
    /// posted event is a non-resize, so the drain never starts.
    #[test]
    fn a_posted_event_does_not_consume_the_key_behind_it() {
        let mut left = vec![typed('x')].into_iter();
        let out = coalesce(diagnostics("lsp"), || left.next());
        assert_eq!(out, diagnostics("lsp"));
        assert_eq!(
            left.next(),
            Some(typed('x')),
            "the key behind a posted event is still queued for the next turn"
        );
    }

    /// A second producer, on a thread of its own, reaching the loop's `recv`.
    /// This is the whole claim of the module in one test.
    #[test]
    fn an_event_posted_from_another_thread_reaches_the_loop() {
        let (queue, poster) = open();
        let handle = std::thread::spawn(move || {
            assert!(poster.post(diagnostics("lsp")), "the loop is listening");
        });
        assert_eq!(queue.recv(), Some(diagnostics("lsp")));
        handle.join().expect("the producer finishes");
    }

    /// Two producers at once, which is the case the terminal and a server are.
    /// Both arrive; neither is lost.
    #[test]
    fn two_producers_both_reach_the_loop() {
        let (queue, poster) = open();
        let second = poster.clone();
        assert!(poster.post(typed('k')));
        assert!(second.post(diagnostics("lsp")));
        drop(poster);
        drop(second);
        let mut seen = Vec::new();
        while let Some(event) = queue.recv() {
            seen.push(event);
        }
        assert_eq!(seen, vec![typed('k'), diagnostics("lsp")]);
    }

    /// The shutdown rule [`super::Queue`] documents: with no producer left,
    /// `recv` answers rather than blocking forever.
    #[test]
    fn a_queue_with_no_producers_left_ends_rather_than_hanging() {
        let (queue, poster) = open();
        drop(poster);
        assert_eq!(queue.recv(), None);
    }

    /// A producer whose loop has ended is told so, rather than queueing into
    /// nothing.
    #[test]
    fn a_producer_is_told_when_the_loop_is_gone() {
        let (queue, poster) = open();
        drop(queue);
        assert!(!poster.post(typed('x')));
    }

    // -----------------------------------------------------------------------
    // The law, over generated interleavings
    // -----------------------------------------------------------------------

    /// How many events one generated interleaving may hold. Bounded so that
    /// [`generated`] can make every event distinct out of printable ASCII.
    const LONGEST: usize = 40;

    /// One generated event, distinct from every other by its index.
    ///
    /// Distinctness is what makes the law checkable: the output can be mapped
    /// back to input positions, so *"in order, exactly once"* is a statement
    /// about indices rather than about equality of two similar keystrokes.
    fn generated(index: usize, kind: u8) -> AppEvent {
        let tag = u8::try_from(index).unwrap_or(u8::MAX);
        match kind {
            0 => resize(u16::from(tag)),
            // `!` onwards — printable, and distinct for every index below
            // [`LONGEST`].
            1 => typed(char::from(b'!' + tag)),
            _ => AppEvent::Posted(Posted {
                source: "lsp",
                action: Action::Lsp(LspAction::IngestDiagnostics {
                    path: std::path::PathBuf::from(format!("src/{index}.rs")),
                    diagnostics: Vec::new(),
                }),
            }),
        }
    }

    proptest! {
        /// **The law: whatever the interleaving, every event that is not a
        /// resize is delivered exactly once and in the order it was posted, and
        /// a resize is dropped only when something arrived after it.**
        ///
        /// That is one statement covering both halves of `§22` and the rule the
        /// queue adds. The first clause is why a keystroke and a server message
        /// cannot be lost; the second is why the drag is fast, and it is stated
        /// as *"only when superseded"* rather than *"resizes may be dropped"*,
        /// because the second would be satisfied by a queue that drops the one
        /// resize you are still waiting to be laid out at.
        ///
        /// Generated over kinds rather than over `crossterm::Event`: a `Resize`
        /// with a different height is the same event as far as this rule is
        /// concerned, and what the law quantifies over is the *interleaving*.
        ///
        /// **The oracle is `kinds`, never [`AppEvent::is_resize`]** — and that
        /// is the correction a review made by running it, not the shape this
        /// was written in. Asserting `event.is_resize()` on a dropped event
        /// states the law in terms of the implementation's own predicate, so
        /// *widening what the drain may drop widens the law with it*: with
        /// `is_resize` mutated to `Term(Resize(..)) | Posted(_)`, the drain
        /// silently swallowed every LSP diagnostic, two unit tests below went
        /// red, and this — *"the strongest evidence"* — passed. [`generated`]
        /// builds a resize **iff** `kind == 0`, so `kinds[index] == 0` is the
        /// same statement with nothing under test on the answering side.
        #[test]
        fn every_non_resize_is_delivered_once_and_in_order(
            kinds in prop::collection::vec(0u8..3, 0..LONGEST),
        ) {
            let posted: Vec<AppEvent> = kinds
                .iter()
                .enumerate()
                .map(|(index, kind)| generated(index, *kind))
                .collect();

            let (queue, poster) = open();
            for event in &posted {
                prop_assert!(poster.post(event.clone()));
            }
            // Dropping the last producer is what ends the drain — the same
            // shutdown rule the loop relies on, used here as a terminator.
            drop(poster);
            let mut delivered = Vec::new();
            while let Some(event) = queue.recv() {
                delivered.push(event);
            }

            // Map each delivered event back to the position it was posted at.
            // Every generated event is distinct, so this is unambiguous.
            let mut at = Vec::new();
            for event in &delivered {
                let index = posted.iter().position(|candidate| candidate == event);
                prop_assert!(index.is_some(), "delivered an event nobody posted: {event:?}");
                at.push(index.unwrap_or_default());
            }
            prop_assert!(
                at.windows(2).all(|pair| pair[0] < pair[1]),
                "delivered out of order or twice: {at:?}"
            );

            // Every index that was not delivered must be a resize with
            // something behind it.
            for (index, event) in posted.iter().enumerate() {
                if at.contains(&index) {
                    continue;
                }
                prop_assert_eq!(
                    kinds[index],
                    0,
                    "dropped something that was not a resize at {}: {:?}",
                    index,
                    event
                );
                prop_assert!(
                    index + 1 < posted.len(),
                    "dropped the last resize, which nothing superseded"
                );
            }
        }
    }
}
