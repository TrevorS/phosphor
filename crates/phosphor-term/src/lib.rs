//! Terminal lifecycle (`T014`) — raw mode, the alternate screen,
//! keyboard-protocol negotiation, panic/exit restore, and the
//! synchronized-output draw wrapper every frame goes through.
//!
//! # Where this lives, and why it is its own crate
//!
//! **An ownership seam, resolved here and reported at `CP-1`.** TEAM.md's role
//! list assigns `T014` to `surface`; its ownership table gives `spine`
//! `phosphor/{main,input,panes}.rs`. Terminal lifecycle is neither a widget nor
//! one of those three files, so it needed a home that collides with neither.
//!
//! A crate rather than a fourth module in the `phosphor` binary, because the
//! task's acceptance criterion is a *type-system* obligation — "no frame can be
//! emitted outside the wrapper, enforce by making the raw writer private" — and
//! Rust privacy is module-scoped. Inside the binary, `main.rs` or `panes.rs`
//! could always reach a `pub(crate)` writer and the guarantee would be a
//! convention again. Across a crate boundary a non-`pub` field is unreachable
//! by construction, for every consumer, permanently. It also costs `spine`
//! nothing: `[workspace] members = ["crates/*"]` picks the crate up with no
//! root-manifest edit.
//!
//! # The contract
//!
//! [`Term`] owns the terminal from construction to drop:
//!
//! * **Setup** — raw mode, the alternate screen, mouse capture, and the kitty
//!   keyboard protocol where the emulator has it. Each step is recorded, so
//!   restore undoes exactly what was done and a half-failed setup still cleans
//!   up after itself.
//! * **Drawing** — [`Term::draw`] is the only way to put pixels on the screen,
//!   and it wraps the frame in a synchronized-output block. Design Language §8:
//!   "Synchronized output wraps every frame; a torn frame is a P0 bug." See
//!   `raw.rs` for how that is made unreachable rather than merely encouraged.
//! * **Restore** — on drop, on `main` returning, and on panic. A panic that
//!   leaves raw mode on with the alternate screen up makes the whole editor feel
//!   broken and hides the panic message, so the hook is installed during setup
//!   and chains to whatever hook was there before.
//!
//! # What this deliberately does not do
//!
//! No event reading, no key decoding, no keymaps. Input is the `spine`
//! machine at `T026`; this crate only *negotiates* the protocol and reports
//! what it got via [`Term::capabilities`], so the decoder knows which shape of
//! key event to expect. At S1 the temporary input path is the vendored crate's
//! own `editor_crossterm` handler, which reads events itself — nothing here is
//! coupled to it, and nothing here has to change when `T026` lands.

mod raw;

use std::io::{self, Stdout};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, supports_keyboard_enhancement,
};
use crossterm::{cursor, execute};

pub use ratatui::Frame;
pub use ratatui::layout::Size;

/// Which keyboard protocol the emulator agreed to.
///
/// `T026`'s decoder branches on this. The distinction is not cosmetic: under
/// the legacy protocol a bare <kbd>esc</kbd> is indistinguishable from the
/// start of an escape sequence without a timeout, and <kbd>ctrl</kbd> +
/// punctuation collapses onto control codes — both of which a vim-style
/// grammar cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardProtocol {
    /// The kitty keyboard protocol is active: escape codes are disambiguated,
    /// key events carry press/repeat/release, and alternate keys are reported.
    Kitty,
    /// The emulator does not support the protocol, or negotiation was declined.
    /// Key events arrive in the traditional encoding.
    Legacy,
}

/// What the terminal turned out to support, settled at setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Capabilities {
    /// The negotiated keyboard protocol.
    pub keyboard: KeyboardProtocol,
    /// Whether mouse events will be delivered. `T081` needs this at `CP-1` to
    /// check that clicks land on the right row of a soft-wrapped line.
    pub mouse: bool,
}

/// Setup knobs. [`Default`] is what the editor uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TermConfig {
    /// Ask the terminal to report mouse events. On by default: the design has
    /// click-to-position, and `T081`'s wrapped-line click check at `CP-1`
    /// cannot run without it.
    pub mouse_capture: bool,
    /// Attempt kitty keyboard protocol negotiation. On by default; when the
    /// emulator says no, setup falls back to [`KeyboardProtocol::Legacy`]
    /// rather than failing. Turn it off to force the legacy path — which is how
    /// the degradation terminal in the `CP-1` matrix gets exercised on hardware
    /// that would otherwise support the protocol.
    pub keyboard_enhancement: bool,
}

impl Default for TermConfig {
    fn default() -> Self {
        Self {
            mouse_capture: true,
            keyboard_enhancement: true,
        }
    }
}

/// Anything that can go wrong owning the terminal.
#[derive(Debug)]
#[non_exhaustive]
pub enum TermError {
    /// A [`Term`] already exists in this process.
    ///
    /// Two of them would mean two restore paths for one terminal, and a drop of
    /// either would tear down the screen under the other. There is one
    /// terminal, so there is one `Term`.
    AlreadyActive,
    /// A terminal operation failed.
    Io(io::Error),
}

impl std::fmt::Display for TermError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyActive => f.write_str("a Term is already active in this process"),
            Self::Io(err) => write!(f, "terminal i/o failed: {err}"),
        }
    }
}

impl std::error::Error for TermError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyActive => None,
            Self::Io(err) => Some(err),
        }
    }
}

impl From<io::Error> for TermError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

// ---------------------------------------------------------------------------
// Restore bookkeeping
//
// The panic hook cannot borrow the `Term`, so what setup entered is recorded in
// a process-global bitset instead. `restore_entered` swaps it to zero before
// acting, which makes restore idempotent: the hook and `Drop` can both fire —
// they do, on a panic — and the second one finds nothing left to undo.
// ---------------------------------------------------------------------------

const RAW_MODE: u8 = 1 << 0;
const ALT_SCREEN: u8 = 1 << 1;
const MOUSE: u8 = 1 << 2;
const KITTY: u8 = 1 << 3;

static ENTERED: AtomicU8 = AtomicU8::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static PANIC_HOOK: Once = Once::new();

fn mark(flag: u8) {
    ENTERED.fetch_or(flag, Ordering::SeqCst);
}

/// Undo whatever setup entered, in reverse order, once.
///
/// Returns the first failure but keeps going regardless: a terminal left in raw
/// mode because an earlier step errored is exactly the outcome this exists to
/// prevent, so one broken step must not skip the rest.
fn restore_entered() -> io::Result<()> {
    let entered = ENTERED.swap(0, Ordering::SeqCst);
    if entered == 0 {
        return Ok(());
    }

    let mut first_error = None;
    let mut record = |result: io::Result<()>| {
        if let Err(err) = result
            && first_error.is_none()
        {
            first_error = Some(err);
        }
    };

    let mut out = io::stdout();

    // Close any synchronized-output block first. `Raw::synchronized_frame`
    // closes its own on every path it controls, but a `panic = "abort"` build
    // or a signal mid-frame does not unwind — and a terminal stuck inside
    // ?2026 shows a frozen screen, which reads as a hang rather than a crash.
    record(execute!(out, EndSynchronizedUpdate));

    if entered & KITTY != 0 {
        record(execute!(out, PopKeyboardEnhancementFlags));
    }
    if entered & MOUSE != 0 {
        record(execute!(out, DisableMouseCapture));
    }
    // Raw mode comes off before the alternate screen: it has the wider set of
    // side effects, and leaving it on while switching buffers is what produces
    // the "shell prompt with no echo" aftermath.
    if entered & RAW_MODE != 0 {
        record(disable_raw_mode());
    }
    if entered & ALT_SCREEN != 0 {
        record(execute!(out, LeaveAlternateScreen));
    }
    record(execute!(out, cursor::Show));

    first_error.map_or(Ok(()), Err)
}

/// Install the panic hook, once per process.
///
/// Chained, not replaced: the previous hook still runs, and it runs *after* the
/// restore, so the message lands on the normal screen where the user can read
/// and copy it instead of on an alternate screen that is about to vanish.
fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore_entered();
            previous(info);
        }));
    });
}

/// The terminal, owned.
///
/// Construct one at startup, draw through it, and let it drop. Nothing else in
/// the program touches the terminal.
#[derive(Debug)]
pub struct Term {
    raw: raw::Raw<Stdout>,
    capabilities: Capabilities,
}

impl Term {
    /// Take over the terminal with the default configuration.
    ///
    /// # Errors
    ///
    /// [`TermError::AlreadyActive`] if a `Term` already exists in this process,
    /// or [`TermError::Io`] if a setup step failed — in which case any step
    /// that had already succeeded is undone before returning, so a failure
    /// never leaves the terminal half-configured.
    pub fn new() -> Result<Self, TermError> {
        Self::with_config(TermConfig::default())
    }

    /// Take over the terminal with an explicit configuration.
    ///
    /// # Errors
    ///
    /// As [`Term::new`].
    pub fn with_config(config: TermConfig) -> Result<Self, TermError> {
        if ACTIVE.swap(true, Ordering::SeqCst) {
            return Err(TermError::AlreadyActive);
        }

        // From here on every early return has to hand the terminal back.
        match Self::setup(config) {
            Ok(term) => Ok(term),
            Err(err) => {
                let _ = restore_entered();
                ACTIVE.store(false, Ordering::SeqCst);
                Err(err)
            }
        }
    }

    fn setup(config: TermConfig) -> Result<Self, TermError> {
        // Before anything is entered, so a panic during setup itself is covered.
        install_panic_hook();

        let mut out = io::stdout();

        enable_raw_mode()?;
        mark(RAW_MODE);

        execute!(out, EnterAlternateScreen)?;
        mark(ALT_SCREEN);

        let mouse = config.mouse_capture;
        if mouse {
            execute!(out, EnableMouseCapture)?;
            mark(MOUSE);
        }

        // Negotiation, with fallback detection. `supports_keyboard_enhancement`
        // round-trips a query to the emulator; a terminal that does not answer
        // (or is not a tty at all, as under a test harness) surfaces as an
        // error, and an error here means "no" — it must not stop the editor
        // from starting. Legacy is a supported path, not a failure.
        let keyboard =
            if config.keyboard_enhancement && supports_keyboard_enhancement().unwrap_or(false) {
                execute!(
                    out,
                    PushKeyboardEnhancementFlags(
                        // The three an editor needs, and no more.
                        //   DISAMBIGUATE_ESCAPE_CODES — a bare `esc` stops being
                        //     ambiguous with the start of a sequence, which is what
                        //     makes a vim grammar's mode exit instant instead of
                        //     timeout-driven.
                        //   REPORT_EVENT_TYPES — press/repeat/release, so held keys
                        //     can be distinguished from autorepeat.
                        //   REPORT_ALTERNATE_KEYS — the base layout's key alongside
                        //     the shifted one, for non-US layouts.
                        // REPORT_ALL_KEYS_AS_ESCAPE_CODES is left off deliberately:
                        // it routes plain text input through escape sequences too,
                        // which buys nothing here and breaks IME and paste on some
                        // emulators.
                        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                            | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                            | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    )
                )?;
                mark(KITTY);
                KeyboardProtocol::Kitty
            } else {
                KeyboardProtocol::Legacy
            };

        let raw = raw::Raw::new(io::stdout())?;

        Ok(Self {
            raw,
            capabilities: Capabilities { keyboard, mouse },
        })
    }

    /// What the terminal agreed to at setup.
    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Draw one frame, inside a synchronized-output block.
    ///
    /// The only way to put anything on the screen. See `raw.rs` for why that is
    /// enforceable rather than aspirational.
    ///
    /// # Errors
    ///
    /// [`TermError::Io`] if the frame could not be written. The
    /// synchronized-output block is closed either way.
    ///
    /// # Panics
    ///
    /// Propagates a panic from `render` unchanged — after closing the block, so
    /// the terminal is not left frozen on the previous frame.
    pub fn draw<F>(&mut self, render: F) -> Result<(), TermError>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        self.raw.synchronized_frame(render).map_err(TermError::Io)
    }

    /// The current screen size.
    ///
    /// # Errors
    ///
    /// [`TermError::Io`] if the terminal could not be queried.
    pub fn size(&self) -> Result<Size, TermError> {
        self.raw.size().map_err(TermError::Io)
    }

    /// Clear the screen; the next [`Term::draw`] repaints in full.
    ///
    /// # Errors
    ///
    /// [`TermError::Io`] if the clear could not be written.
    pub fn clear(&mut self) -> Result<(), TermError> {
        self.raw.clear().map_err(TermError::Io)
    }

    /// Restore the terminal now, surfacing any failure.
    ///
    /// Consumes the `Term`, so nothing can draw afterwards. [`Drop`] does the
    /// same thing silently; use this when `main` wants to report a cleanup
    /// failure on its way out.
    ///
    /// # Errors
    ///
    /// [`TermError::Io`] from the first restore step that failed. The remaining
    /// steps are attempted regardless.
    pub fn restore(self) -> Result<(), TermError> {
        // `Drop` still runs and finds the bitset already cleared, which is the
        // whole point of `restore_entered` being idempotent.
        restore_entered().map_err(TermError::Io)
    }
}

impl Drop for Term {
    /// Restore on the way out, including the error path where `main` returns
    /// early. Failures are swallowed: a `Drop` cannot report and must not
    /// panic, and by this point the process is leaving anyway.
    fn drop(&mut self) {
        let _ = restore_entered();
        ACTIVE.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ALT_SCREEN, Capabilities, ENTERED, KITTY, KeyboardProtocol, MOUSE, Ordering, RAW_MODE,
        TermConfig, TermError, mark, restore_entered,
    };

    /// The restore bookkeeping, exercised without a terminal.
    ///
    /// These tests do not enter raw mode — there is no tty under nextest, and a
    /// test that grabbed the real terminal would be the flakiest thing in the
    /// suite. What they cover is the state machine that decides *what* gets
    /// undone, which is the part with the interesting failure mode: a hook and
    /// a `Drop` both firing on a panic.
    ///
    /// They share the process-global bitset, so they clear it first rather than
    /// assuming a starting state; nextest's process-per-test isolation
    /// (SPIKES.md's hygiene table) is what keeps that honest.
    #[test]
    fn restore_is_idempotent() {
        ENTERED.store(0, Ordering::SeqCst);
        mark(RAW_MODE | ALT_SCREEN | MOUSE | KITTY);
        assert_ne!(ENTERED.load(Ordering::SeqCst), 0);

        // Writing escape sequences to a captured stdout is harmless; what is
        // being asserted is that the bitset is consumed exactly once.
        let _ = restore_entered();
        assert_eq!(
            ENTERED.load(Ordering::SeqCst),
            0,
            "restore did not consume the entered set"
        );

        let _ = restore_entered();
        assert_eq!(
            ENTERED.load(Ordering::SeqCst),
            0,
            "a second restore must be a no-op — the panic hook and Drop both run"
        );
    }

    #[test]
    fn nothing_entered_means_nothing_to_undo() {
        ENTERED.store(0, Ordering::SeqCst);
        assert!(
            restore_entered().is_ok(),
            "restoring an untouched terminal must not fail"
        );
    }

    #[test]
    fn setup_flags_are_distinct_bits() {
        // A collision here would make restore undo the wrong step, silently.
        let all = [RAW_MODE, ALT_SCREEN, MOUSE, KITTY];
        let union = all.iter().fold(0u8, |acc, flag| acc | flag);
        assert_eq!(
            u32::from(union).count_ones(),
            all.len() as u32,
            "two setup flags share a bit"
        );
    }

    #[test]
    fn the_default_config_is_what_cp1_needs() {
        let config = TermConfig::default();
        // `T081` checks mouse clicks on a wrapped line at `CP-1`.
        assert!(config.mouse_capture);
        assert!(config.keyboard_enhancement);
    }

    #[test]
    fn legacy_is_a_capability_not_an_error() {
        // Fallback detection is a reported state, never a setup failure.
        let caps = Capabilities {
            keyboard: KeyboardProtocol::Legacy,
            mouse: false,
        };
        assert_eq!(caps.keyboard, KeyboardProtocol::Legacy);
    }

    #[test]
    fn already_active_reads_legibly() {
        assert_eq!(
            TermError::AlreadyActive.to_string(),
            "a Term is already active in this process"
        );
    }
}
