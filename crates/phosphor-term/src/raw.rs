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
//! 3. **Function.** Exactly one of those methods draws:
//!    [`Raw::synchronized_frame`]. There is no accessor returning the terminal,
//!    the backend or the writer, no `Deref`, and no way to build a `Raw` around
//!    a writer somebody else still holds.
//!
//! So the shortest path to a frame from anywhere in the program is
//! `Term::draw`, and every such path passes through the `?2026h` / `?2026l`
//! pair. Design Language §8: "Synchronized output wraps every frame; a torn
//! frame is a P0 bug."
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
    /// Terminals that do not implement mode 2026 ignore both sequences (an
    /// unknown private mode is discarded), so this needs no capability check —
    /// it degrades to an ordinary unsynchronized draw.
    pub(crate) fn synchronized_frame<F>(&mut self, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        execute!(self.terminal.backend_mut(), BeginSynchronizedUpdate)?;

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
    #[derive(Clone, Default)]
    struct Tap(Rc<RefCell<Vec<u8>>>);

    impl Write for Tap {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
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
        let tap = Tap::default();
        let viewport = Viewport::Fixed(Rect::new(0, 0, width, height));
        let terminal = Terminal::with_options(
            CrosstermBackend::new(tap.clone()),
            TerminalOptions { viewport },
        )
        .expect("a fixed viewport needs no backend size query");
        (Raw { terminal }, tap)
    }

    fn written(tap: &Tap) -> Vec<u8> {
        tap.0.borrow().clone()
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
}
