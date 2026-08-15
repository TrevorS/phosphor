//! The raw writer, and the **only** place in the tree that emits a frame.
//!
//! `T014`'s acceptance criterion is a type-system obligation, not a convention:
//! *no frame can be emitted outside the synchronized-output wrapper, enforced by
//! making the raw writer private.* This module is how that is enforced, and it
//! is deliberately tiny so the claim can be checked by reading it in full.
//!
//! Three walls, outermost first:
//!
//! 1. **Crate.** `raw` is a private module of `phosphor-term`. Nothing outside
//!    this crate can name [`Raw`], and the crate exports no backend, no
//!    `Terminal`, and no writer.
//! 2. **Module.** `Raw`'s writer is a private field. Even `lib.rs` — the only
//!    other module in the crate — cannot touch it; it can only call the
//!    `pub(crate)` methods below.
//! 3. **Function.** Exactly one of those methods *draws a frame*:
//!    [`Raw::synchronized_frame`]. There is no accessor returning the terminal,
//!    the backend or the writer, no `Deref`, and no way to build a `Raw` around
//!    a writer somebody else still holds.
//!
//! So the shortest path to a frame from anywhere in the program is
//! `Term::draw`, and every such path passes through the `?2026h` / `?2026l`
//! pair. Design Language §8: "Synchronized output wraps every frame; a torn
//! frame is a P0 bug."
//!
//! **Wall 3 used to read "exactly one of those methods draws", full stop, and
//! that was false.** [`Raw::clear`] also puts bytes on the screen, outside the
//! pair, and asks the terminal a question first. It has no caller — which is
//! why the claim survived two windows — and `Term::clear`'s doc comment
//! carries the whole finding, including what it would take to give it one.
//! The distinction the sentence now draws is the real one: a *frame* has
//! exactly one door; the screen has two, and the second is unused.
//!
//! The one thing this cannot stop is a caller writing bytes to stdout behind
//! the library's back. The workspace's `clippy::print_stdout` /
//! `print_stderr` lints (root `Cargo.toml`, denied in CI) cover the honest
//! version of that mistake, and the crate docs cover the rest.

use std::io::{self, Write};

use crossterm::execute;
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Size;
use ratatui::{Frame, Terminal};

/// The terminal and its writer, sealed.
///
/// Generic over the writer only so the wrapper's byte ordering can be tested
/// without a tty (see the tests at the bottom of this file). `Term`
/// instantiates it at `Stdout`, and the parameter never appears in this crate's
/// public API.
pub(crate) struct Raw<W: Write> {
    /// **Private to this module. Do not widen this.** Widening it to
    /// `pub(crate)` — or adding an accessor that returns it, the backend, or
    /// `&mut W` — is what would make `T014` false, because a caller could then
    /// draw without the synchronized-output pair around the frame.
    terminal: Terminal<CrosstermBackend<W>>,
}

impl<W: Write> Raw<W> {
    /// Wraps a writer in a fullscreen terminal.
    ///
    /// Queries the backend for the screen size, so it needs a real tty; the
    /// tests build a fixed viewport instead.
    pub(crate) fn new(writer: W) -> io::Result<Self> {
        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(writer))?,
        })
    }

    /// Draw one frame, inside a synchronized-output block.
    ///
    /// The sequence is `?2026h` → the frame's diff → flush → `?2026l`, which is
    /// what makes the update atomic from the emulator's point of view: it keeps
    /// showing the previous frame until the closing sequence arrives, so a slow
    /// or partial write is never visible as a torn frame.
    ///
    /// **`?2026l` is emitted on every exit path — error and unwind included.**
    /// Leaving the block open would freeze the display on the last good frame,
    /// so the terminal would look hung rather than broken, and a panic message
    /// printed after it would never appear. That is the worse failure, and it
    /// is why the draw is caught rather than left to unwind through: the block
    /// is closed, then the panic is resumed unchanged.
    ///
    /// **Including the path where the *open* failed**, which is the one this
    /// missed until the coverage pass ran a failing writer over it. An
    /// `io::Error` from `execute!` does not say whether the eight bytes of
    /// `?2026h` reached the terminal — `queue!` writes and `flush` is a
    /// separate syscall, so a write that succeeded and a flush that failed is
    /// an error over bytes already committed. Assuming they arrived costs one
    /// redundant `?2026l` on a path that is already failing; assuming they did
    /// not costs a frozen terminal. `restore_entered` in `lib.rs` opens with
    /// the same unconditional close for the same reason, and this is the two
    /// of them agreeing.
    ///
    /// Terminals that do not implement mode 2026 ignore both sequences (an
    /// unknown private mode is discarded), so this needs no capability check —
    /// it degrades to an ordinary unsynchronized draw.
    pub(crate) fn synchronized_frame<F>(&mut self, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        if let Err(err) = execute!(self.terminal.backend_mut(), BeginSynchronizedUpdate) {
            // No frame is drawn: an unsynchronized draw after a failed open is
            // the tear §8 forbids, dressed as a recovery.
            let _ = execute!(self.terminal.backend_mut(), EndSynchronizedUpdate);
            return Err(err);
        }

        let terminal = &mut self.terminal;
        // AssertUnwindSafe: on the unwind path the only thing touched before
        // resuming is the writer, to close the block. No terminal state is read
        // back, so a torn internal buffer cannot be observed.
        let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            // `draw` borrows the terminal for as long as the returned
            // `CompletedFrame` lives; map it away so the backend is free below.
            terminal.draw(render).map(|_| ())
        }));

        let closed = execute!(self.terminal.backend_mut(), EndSynchronizedUpdate);

        match drawn {
            Ok(result) => {
                result?;
                closed
            }
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// The current screen size, for callers that need to lay out before
    /// drawing.
    pub(crate) fn size(&self) -> io::Result<Size> {
        self.terminal.size()
    }

    /// Clear the screen and force the next frame to redraw in full.
    pub(crate) fn clear(&mut self) -> io::Result<()> {
        self.terminal.clear()
    }
}

impl<W: Write> std::fmt::Debug for Raw<W> {
    /// Opaque on purpose. The workspace denies `missing_debug_implementations`,
    /// and the derived form would print two full cell buffers — thousands of
    /// cells — into whatever log asked for it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Raw { .. }")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::{self, Write};
    use std::rc::Rc;

    use proptest::prelude::*;
    use ratatui::layout::Rect;
    use ratatui::widgets::Paragraph;
    use ratatui::{TerminalOptions, Viewport};

    use super::{CrosstermBackend, Raw, Terminal};

    const BEGIN: &[u8] = b"\x1b[?2026h";
    const END: &[u8] = b"\x1b[?2026l";

    /// An in-memory writer the test keeps a handle to after handing it to the
    /// backend. Needed because the backend's own `writer()` accessor is behind
    /// ratatui's `instability` gate, and a test is not a reason to turn an
    /// unstable feature on for the whole crate.
    ///
    /// It records **two** streams, and the distinction is the whole of the
    /// error-path testing below. `offered` is every byte handed to `write`,
    /// succeeded or not — what this crate *attempted*, which is the only thing
    /// it controls. `delivered` is what a successful `write` accepted — the
    /// wire, and therefore what the terminal's mode-2026 state is a function
    /// of. A dead pipe makes those two disagree, and asserting on the wrong one
    /// turns "the writer is broken" into "the crate forgot to close the block".
    #[derive(Clone, Default)]
    struct Tap(Rc<RefCell<TapState>>);

    /// What a [`Tap`] has seen, and how it is set up to fail.
    #[derive(Default)]
    struct TapState {
        offered: Vec<u8>,
        delivered: Vec<u8>,
        /// `write` calls so far. Indices into this counter are what
        /// `failing_writes` names.
        writes: usize,
        /// `write` call indices that return `Err` instead of accepting.
        failing_writes: Vec<usize>,
        flushes: usize,
        /// `flush` call indices that return `Err`.
        ///
        /// A flush failure is the interesting one and the reason this is
        /// separate from `failing_writes`: `execute!` is a `write_all` followed
        /// by a `flush`, so a failed flush is an error reported over bytes the
        /// writer has *already accepted*. That is the case
        /// [`Raw::synchronized_frame`]'s doc argues about, and the one the
        /// pre-coverage-pass code got wrong.
        failing_flushes: Vec<usize>,
    }

    impl Tap {
        fn failing(writes: &[usize], flushes: &[usize]) -> Self {
            Self(Rc::new(RefCell::new(TapState {
                failing_writes: writes.to_vec(),
                failing_flushes: flushes.to_vec(),
                ..TapState::default()
            })))
        }

        /// Every byte this crate handed to the writer, accepted or not.
        fn offered(&self) -> Vec<u8> {
            self.0.borrow().offered.clone()
        }

        /// Every byte the writer accepted — the wire.
        fn delivered(&self) -> Vec<u8> {
            self.0.borrow().delivered.clone()
        }
    }

    impl Write for Tap {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut state = self.0.borrow_mut();
            let index = state.writes;
            state.writes += 1;
            state.offered.extend_from_slice(buf);
            if state.failing_writes.contains(&index) {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            state.delivered.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let mut state = self.0.borrow_mut();
            let index = state.flushes;
            state.flushes += 1;
            if state.failing_flushes.contains(&index) {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            Ok(())
        }
    }

    /// A `Raw` over a `Tap`, with a fixed viewport.
    ///
    /// `Viewport::Fixed` is what makes this hermetic: it takes the area as given
    /// instead of asking the backend for the screen size, so the test runs with
    /// no tty attached — which is the case in CI, and under nextest's
    /// process-per-test isolation.
    fn fixed(width: u16, height: u16) -> (Raw<Tap>, Tap) {
        over(Tap::default(), width, height)
    }

    /// The same, over a `Tap` the caller has already set up to fail.
    fn over(tap: Tap, width: u16, height: u16) -> (Raw<Tap>, Tap) {
        let viewport = Viewport::Fixed(Rect::new(0, 0, width, height));
        let terminal = Terminal::with_options(
            CrosstermBackend::new(tap.clone()),
            TerminalOptions { viewport },
        )
        .expect("a fixed viewport needs no backend size query");
        (Raw { terminal }, tap)
    }

    fn written(tap: &Tap) -> Vec<u8> {
        tap.delivered()
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn count(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .filter(|w| *w == needle)
            .count()
    }

    /// Every `?2026h` and `?2026l` in `bytes`, in the order they appear.
    ///
    /// `true` opens, `false` closes. The two markers cannot overlap — they
    /// differ only in the final byte and neither is a prefix of the other — so
    /// a single left-to-right scan is exact rather than approximate.
    fn markers(bytes: &[u8]) -> Vec<bool> {
        let mut out = Vec::new();
        let mut at = 0;
        while at + BEGIN.len() <= bytes.len() {
            let window = &bytes[at..at + BEGIN.len()];
            if window == BEGIN {
                out.push(true);
                at += BEGIN.len();
            } else if window == END {
                out.push(false);
                at += END.len();
            } else {
                at += 1;
            }
        }
        out
    }

    /// The law: open, close, open, close — never two opens running, never a
    /// close with nothing open, and never a trailing open.
    ///
    /// A trailing open is the P0. Design Language §8: the emulator holds the
    /// previous frame until `?2026l` arrives, so a block left open is a
    /// terminal that has stopped updating and looks hung rather than broken.
    fn alternates(markers: &[bool]) -> bool {
        markers
            .iter()
            .enumerate()
            .all(|(n, open)| *open == n.is_multiple_of(2))
            && !markers.last().copied().unwrap_or(false)
    }

    #[test]
    fn a_frame_is_wrapped_in_a_synchronized_output_block() {
        let (mut raw, tap) = fixed(20, 3);
        raw.synchronized_frame(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new("phosphor"), area);
        })
        .expect("an in-memory writer cannot fail");

        let out = written(&tap);
        let begin = find(&out, BEGIN).expect("the frame was not opened with ?2026h");
        let end = find(&out, END).expect("the frame was not closed with ?2026l");
        assert!(begin < end, "?2026l was emitted before ?2026h");

        // The payload must land strictly between the two. A wrapper that
        // emitted the pair around nothing would pass the two asserts above, so
        // this is the assert that actually carries the property.
        let text = find(&out, b"phosphor").expect("the frame content was not written");
        assert!(
            begin < text && text < end,
            "frame content escaped the synchronized-output block"
        );
    }

    #[test]
    fn every_frame_gets_its_own_block() {
        let (mut raw, tap) = fixed(20, 3);
        for n in 0..3 {
            raw.synchronized_frame(|frame| {
                let area = frame.area();
                frame.render_widget(Paragraph::new(format!("frame {n}")), area);
            })
            .expect("an in-memory writer cannot fail");
        }

        let out = written(&tap);
        assert_eq!(count(&out, BEGIN), 3, "one ?2026h per frame");
        assert_eq!(count(&out, END), 3, "one ?2026l per frame");
    }

    #[test]
    fn a_panicking_render_still_closes_the_block() {
        // A widget that panics unwinds through `draw`. If the block stayed open
        // the emulator would freeze on the last good frame and the panic
        // message would never be visible — the "feels broken" failure `T014`
        // exists to prevent.
        let (mut raw, tap) = fixed(20, 3);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // keep the expected panic off the test log
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = raw.synchronized_frame(|_frame| panic!("widget blew up"));
        }));
        std::panic::set_hook(previous);
        assert!(caught.is_err(), "the panic should propagate to the caller");

        let out = written(&tap);
        assert_eq!(
            count(&out, BEGIN),
            1,
            "the block was never opened, so this test proves nothing"
        );
        assert_eq!(
            count(&out, END),
            1,
            "a panicking render left the synchronized-output block open"
        );
    }

    #[test]
    fn the_block_opens_before_anything_else_is_written() {
        // "Every frame is inside the pair" has two halves, and the three tests
        // above only cover one. This is the other: nothing at all reaches the
        // writer before `?2026h` does, so there is no prologue — a cursor hide,
        // a mode set, a cleared line — sitting outside the block where a slow
        // terminal could show it on its own.
        let (mut raw, tap) = fixed(20, 3);
        raw.synchronized_frame(|frame| {
            frame.render_widget(Paragraph::new("phosphor"), frame.area());
        })
        .expect("an in-memory writer cannot fail");

        let out = written(&tap);
        assert_eq!(
            find(&out, BEGIN),
            Some(0),
            "{} byte(s) reached the terminal before the block opened",
            find(&out, BEGIN).unwrap_or(out.len()),
        );
    }

    // -----------------------------------------------------------------------
    // The error paths
    // -----------------------------------------------------------------------

    #[test]
    fn a_failed_open_still_offers_the_close() {
        // The defect this pass found. `execute!` writes and then flushes, so an
        // `io::Error` from the opener can be an error over eight bytes the
        // writer already took — and the code returned on it without closing,
        // which is a terminal frozen on its last good frame for as long as the
        // editor keeps running.
        //
        // The writer here accepts every byte and fails the opener's flush,
        // which is exactly that shape.
        let (mut raw, tap) = over(Tap::failing(&[], &[0]), 20, 3);
        let result = raw.synchronized_frame(|frame| {
            frame.render_widget(Paragraph::new("phosphor"), frame.area());
        });

        assert!(result.is_err(), "the flush failure must still be reported");
        let out = written(&tap);
        assert_eq!(count(&out, BEGIN), 1, "the opener reached the terminal");
        assert_eq!(
            count(&out, END),
            1,
            "the block was opened on the wire and never closed — the terminal is \
             frozen on its last frame and the editor does not know"
        );
        assert_eq!(
            find(&out, b"phosphor"),
            None,
            "a frame was drawn after the open failed: an unsynchronized frame is \
             the tear §8 forbids, dressed as a recovery"
        );
    }

    #[test]
    fn a_dead_writer_gets_one_close_offered_per_open() {
        // The honest limit. When every write fails, nothing this crate does can
        // put `?2026l` on the wire — so what is asserted is what it *attempted*,
        // which is the only thing it controls. The distinction matters because
        // the two `Tap` streams are what tell "the pipe is broken" apart from
        // "the crate forgot", and only the second is a bug.
        let (mut raw, tap) = over(Tap::failing(&[0, 1, 2, 3, 4, 5, 6, 7, 8], &[]), 20, 3);
        let result = raw.synchronized_frame(|frame| {
            frame.render_widget(Paragraph::new("phosphor"), frame.area());
        });

        assert!(result.is_err());
        assert!(
            tap.delivered().is_empty(),
            "a writer that fails every call cannot have accepted bytes"
        );
        let offered = tap.offered();
        assert_eq!(count(&offered, BEGIN), 1);
        assert_eq!(
            count(&offered, END),
            1,
            "the close was never even attempted on a broken writer"
        );
    }

    #[test]
    fn a_render_that_panics_after_writing_still_closes_the_block() {
        // The existing panic test panics *before* the widget renders, so the
        // block it closes is an empty one. This one gets a frame's bytes onto
        // the wire first, which is the case that would tear: the emulator is
        // holding a half-written screen when the panic arrives.
        let (mut raw, tap) = fixed(20, 3);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = raw.synchronized_frame(|frame| {
                frame.render_widget(Paragraph::new("phosphor"), frame.area());
                panic!("a widget blew up after painting half the screen");
            });
        }));
        std::panic::set_hook(previous);
        assert!(caught.is_err());

        let out = written(&tap);
        assert!(
            alternates(&markers(&out)),
            "the wire is not open/close/open/close: {:?}",
            markers(&out)
        );
        // ratatui buffers the frame and flushes it inside `draw`, so a panic in
        // the render closure discards it. Asserted rather than assumed: if it
        // ever changes, the half-frame is on the wire and the pair around it is
        // the only thing keeping it invisible.
        assert_eq!(count(&out, BEGIN), 1);
        assert_eq!(count(&out, END), 1);
    }

    // `Raw::clear` is deliberately not tested here, and the reason is the
    // finding rather than an omission — `Term::clear`'s doc comment carries it
    // in full. In one line: `Terminal::clear` opens with
    // `backend.get_cursor_position()`, which is a `CSI 6 n` round trip to
    // whatever `/dev/tty` resolves to, so a test that called it would either
    // write to the developer's real terminal and eat its reply, or pass and
    // fail depending on whether the suite was run from a shell. Both are worse
    // than a documented gap. Running it under nextest is what established this:
    // it returns `ENXIO` — "Device not configured" — having written nothing at
    // all to the writer this module seals.

    // **There is no benchmark in this crate, and that is a finding rather than
    // an omission.** The candidate was the wrapper's own cost: two extra
    // `execute!` calls per frame, each a write and a flush, on the keystroke
    // path. Measured before deciding, over a 120x40 full redraw to a real fd
    // (`/dev/null`, release build, 20,000 frames each way): a bare
    // `Terminal::draw` came in at **62.8 µs** per frame and the wrapped one at
    // **60.8 µs** — the wrapper is *below the noise floor* of the frame it
    // wraps, and its cost measured negative. Against a 16.67 ms budget the
    // whole frame is 0.4% of it.
    //
    // A benchmark exists in this repo when a number would change a decision
    // (the `justfile`'s `bench` block). This number cannot: no plausible
    // movement in it would justify queueing the opener rather than executing
    // it, and `phosphor/benches/vm_invocations.rs` already counts what a frame
    // costs end to end on a pty, from outside the shipping binary. A fifth
    // benchmark measuring a strict subset of that would be maintenance for
    // nothing.

    // -----------------------------------------------------------------------
    // The law
    // -----------------------------------------------------------------------

    /// One frame in a generated session.
    #[derive(Debug, Clone, Copy)]
    enum Step {
        /// A widget renders and returns.
        Clean,
        /// A widget panics, and the panic unwinds through `draw`.
        Panics,
    }

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![Just(Step::Clean), Just(Step::Panics)]
    }

    proptest! {
        /// **Whatever a session does, one close is offered per open.**
        ///
        /// The three example tests above cover three schedules. This covers the
        /// rest: any interleaving of clean frames and panicking ones, over a
        /// writer that fails at an arbitrary set of call indices. What is
        /// asserted is over `offered` — what the crate handed to the writer —
        /// because that is the half it controls; a broken pipe cannot be made
        /// to carry `?2026l` by any amount of care here.
        ///
        /// **What the generator cannot produce**, each because no in-process
        /// test can reach it:
        ///
        /// * A signal or a `panic = "abort"` build. Nothing unwinds, so nothing
        ///   here runs at all — that case is `lib.rs`'s `restore_entered`
        ///   opening with an unconditional `EndSynchronizedUpdate`, and it is
        ///   only observable from another process.
        /// * Concurrent draws. `synchronized_frame` takes `&mut self`, so two
        ///   overlapping blocks are unconstructible rather than untested.
        /// * A writer that accepts a *prefix* of a command. `write_all` is what
        ///   crossterm calls, and this `Tap` either takes a whole buffer or
        ///   fails — so a `?2026h` split across a success and a failure is out
        ///   of reach. It is also not a mode set when it is torn, which is why
        ///   the law is stated over complete markers.
        /// * A terminal that answers. Nothing is read here; input is `T026`'s.
        #[test]
        fn every_schedule_offers_one_close_per_open(
            steps in prop::collection::vec(step(), 1..8),
            failing_writes in prop::collection::vec(0usize..24, 0..4),
            failing_flushes in prop::collection::vec(0usize..24, 0..4),
        ) {
            let healthy = failing_writes.is_empty() && failing_flushes.is_empty();
            let (mut raw, tap) = over(
                Tap::failing(&failing_writes, &failing_flushes),
                20,
                3,
            );

            // The hook is nulled to keep generated panics off the test log, and
            // restored before the first `prop_assert!` — an early return with
            // the null hook still installed would silence every later failure
            // in the process, including proptest's own shrink reports.
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let outcomes: Vec<bool> = steps
                .iter()
                .enumerate()
                .map(|(n, step)| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = raw.synchronized_frame(|frame| {
                            frame.render_widget(Paragraph::new(format!("frame {n}")), frame.area());
                            if matches!(step, Step::Panics) {
                                panic!("generated panic");
                            }
                        });
                    }))
                    .is_err()
                })
                .collect();
            std::panic::set_hook(previous);

            for (outcome, step) in outcomes.iter().zip(&steps) {
                prop_assert!(
                    !*outcome || matches!(step, Step::Panics),
                    "a panic arrived that no step generated"
                );
                // The converse only holds on a healthy writer, and the reason
                // is this pass's own change rather than a weakening: when the
                // *open* fails, `synchronized_frame` closes the block and
                // returns without drawing, so the render closure — and its
                // panic — never runs. The property found that; the examples
                // could not have.
                if healthy {
                    prop_assert_eq!(
                        *outcome,
                        matches!(step, Step::Panics),
                        "a panic was swallowed"
                    );
                }
            }

            let offered = markers(&tap.offered());
            prop_assert!(
                alternates(&offered),
                "offered markers are not open/close/open/close: {:?}",
                offered
            );
            prop_assert_eq!(
                offered.len(),
                steps.len() * 2,
                "one pair per frame, whatever happened inside it"
            );

            // On the wire, the same law — but only when the writer took every
            // byte. A failing writer can drop the close it was handed, and the
            // terminal is then frozen until the next frame's close or the exit
            // path's; that is a fact about the pipe, not a defect here, and
            // asserting it over `delivered` would be asserting that a broken
            // pipe works.
            if healthy {
                let delivered = markers(&tap.delivered());
                prop_assert!(alternates(&delivered), "{:?}", delivered);
                prop_assert_eq!(delivered.len(), steps.len() * 2);
            }
        }
    }
}
