//! Terminal lifecycle (`T014`) — raw mode, the alternate screen,
//! keyboard-protocol negotiation, panic/exit restore, and the
//! synchronized-output draw wrapper every frame goes through.
//!
//! # Where this lives, and why it is its own crate
//!
//! **An ownership seam, reported at `CP-1` and settled after it: this crate is
//! `spine`'s, and `T014` moved with it.** The breakdown had assigned `T014` to
//! `surface`, which built it, while the ownership table gave `spine`
//! `phosphor/{main,input,panes}.rs` — and terminal lifecycle is neither a widget
//! nor one of those three files, so it needed a home that collides with neither.
//!
//! It is `spine`'s because the only production consumer is the binary
//! (`phosphor-buffer` takes it as a dev-dependency, for one example); because
//! this is where `crossterm` and `ratatui` live, and
//! `scripts/lint-no-app-layer-in-ui.sh` fails CI on either of them appearing in
//! `phosphor-ui`; because keyboard-protocol negotiation is input, and the input
//! machine (`T026`) is `spine`'s; and because nothing here draws. The rule is
//! the one that moved `T034`/`T035` the other way: **the file decides the task.**
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
//!
//! # `T027` — what negotiating here costs the decoder there
//!
//! Two consequences of the flags pushed below, both of which land in
//! `phosphor-core`'s input machine rather than here:
//!
//! * `REPORT_EVENT_TYPES` means every press is also reported as a release, so
//!   a loop that acted on both would apply every keystroke twice. The loop
//!   drops releases (`main.rs`'s `is_press`) and the *kind* deliberately never
//!   reaches a key — `phosphor_core::input::key`'s header argues why.
//! * `REPORT_ALTERNATE_KEYS` means crossterm reports a shifted chord as the
//!   shifted character with the shift bit **cleared**
//!   (`event/sys/unix/parse.rs:594-606` in crossterm 0.29). So
//!   <kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> is `Char('K')` + `CTRL`
//!   here and `Char('k')` + `CTRL | SHIFT` on a terminal without the flag;
//!   `phosphor_core::input::key::Key::new` folds both into the one spelling a
//!   keymap is written in.
//!
//! And the negotiated protocol is not just a report — the machine changes
//! behaviour on it, because under the legacy encoding
//! <kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> and
//! <kbd>ctrl</kbd>+<kbd>k</kbd> are the same byte. The host wires the two
//! together in one line:
//!
//! ```text
//! machine.set_protocol(match term.capabilities().keyboard {
//!     KeyboardProtocol::Kitty => key::Protocol::Kitty,
//!     KeyboardProtocol::Legacy => key::Protocol::Legacy,
//! });
//! ```
//!
//! **`$PHOSPHOR_KEYBOARD` is how the second half of that gets tested at all.**
//! `CP-3` asks for modifier chords *"on the primary terminal, then on the
//! degradation terminal"*, and the degradation terminal is the one nobody
//! building this has open. `PHOSPHOR_KEYBOARD=legacy phosphor …` turns the
//! good terminal into the bad one for the length of one run — the same escape
//! hatch `PHOSPHOR_UNDERCURL` is for `T085`, for the same reason.

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

impl KeyboardProtocol {
    /// What `$PHOSPHOR_KEYBOARD` forces, if it forces anything.
    ///
    /// A pure function of the value so every branch is testable without a
    /// terminal; [`KeyboardProtocol::forced`] is the one caller that reads the
    /// process.
    ///
    /// **It does not spell the same words as `PHOSPHOR_UNDERCURL`**, which this
    /// comment claimed until the coverage pass checked it against
    /// `vendor/ratatui-code-editor/src/phosphor/cell_style.rs`'s
    /// `TerminalEnv::forced`. That one also takes `yes`/`always` and
    /// `no`/`never`, and does not take `none`; here `PHOSPHOR_KEYBOARD=yes` is
    /// an unrecognised value and the query decides. The two are the same *kind*
    /// of escape hatch and always were — the vocabulary sentence was the part
    /// that was wrong, and `the_two_escape_hatches_do_not_spell_the_same_words`
    /// is what keeps it from drifting back.
    ///
    /// * `legacy`, `0`, `off`, `false`, `none` → [`KeyboardProtocol::Legacy`].
    ///   **The degradation terminal, on the hardware you have.** Negotiation is
    ///   skipped entirely rather than pushed and ignored, so what the editor
    ///   receives is what a terminal without the protocol would send.
    /// * `kitty`, `1`, `on`, `true`, `force` → [`KeyboardProtocol::Kitty`]. For
    ///   an emulator that supports the protocol but answers the *query* badly —
    ///   a multiplexer in the middle is the usual reason. The flags are pushed
    ///   without asking first.
    /// * anything else, including unset and empty → [`None`], and the query
    ///   decides. An unrecognised value is ignored rather than refused: this
    ///   runs during terminal setup, where there is nowhere to report to yet.
    ///
    /// It overrides both the query and [`TermConfig`], which is what "escape
    /// hatch" has to mean to be worth having.
    #[must_use]
    pub fn from_env_value(value: Option<&str>) -> Option<Self> {
        let value = value?.trim().to_ascii_lowercase();
        match value.as_str() {
            "legacy" | "0" | "off" | "false" | "none" => Some(Self::Legacy),
            "kitty" | "1" | "on" | "true" | "force" => Some(Self::Kitty),
            _ => None,
        }
    }

    /// [`KeyboardProtocol::from_env_value`] against the real environment.
    #[must_use]
    pub fn forced() -> Option<Self> {
        Self::from_env_value(std::env::var(KEYBOARD_ENV).ok().as_deref())
    }
}

/// The override [`KeyboardProtocol::forced`] reads.
pub const KEYBOARD_ENV: &str = "PHOSPHOR_KEYBOARD";

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
        //
        // `$PHOSPHOR_KEYBOARD` short-circuits both directions (`T027`): forcing
        // legacy skips the push, so the editor receives exactly what a terminal
        // without the protocol sends, which is the only way to exercise the
        // degradation path on hardware that has the protocol.
        let forced = KeyboardProtocol::forced();
        let negotiated = match forced {
            Some(KeyboardProtocol::Legacy) => false,
            Some(KeyboardProtocol::Kitty) => true,
            None => config.keyboard_enhancement && supports_keyboard_enhancement().unwrap_or(false),
        };
        let keyboard = if negotiated {
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
    /// # Read this before giving it a caller
    ///
    /// **Nothing calls this**, and the coverage pass that established that also
    /// established two things about it that the rest of this crate's contract
    /// says should be impossible. Both are `ratatui` 0.30.2's behaviour, read
    /// from its source rather than inferred, and neither is a bug in the code
    /// below — they are what `Terminal::clear` does:
    ///
    /// 1. **It writes outside the synchronized-output block.**
    ///    `CrosstermBackend::clear_region` is an `execute!` — write, then
    ///    flush, immediately. A full-screen erase is the most tearing-visible
    ///    write there is, and `raw.rs`'s header claim that *"exactly one of
    ///    those methods draws"* is true only because this one has no caller.
    /// 2. **It reads from the terminal.** `Terminal::clear` opens with
    ///    `backend.get_cursor_position()`, which is a `CSI 6 n` round trip on
    ///    whatever `/dev/tty` resolves to — so it would compete with `T026`'s
    ///    input machine for the bytes arriving on stdin, and it blocks until it
    ///    is answered. Under `nextest`, with no terminal to answer, it returns
    ///    `ENXIO` having written nothing to the writer this crate seals.
    ///
    /// It is left as it is because a coverage pass is the wrong place to
    /// remove public API or to change what a call does. The choice belongs to
    /// whoever needs it: wrap it in the pair and take the cursor round trip
    /// knowingly, or delete it — a repaint is already reachable through
    /// [`Term::draw`], which is what the frame path uses today.
    ///
    /// # Errors
    ///
    /// [`TermError::Io`] if the clear could not be written, **or if the cursor
    /// query it makes first was not answered**.
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

// ---------------------------------------------------------------------------
// Colour, and whether the terminal will take any
// ---------------------------------------------------------------------------

/// Will this terminal render colour at all?
///
/// # Why the editor has to ask, when `crossterm` already knows
///
/// It does not know it *out loud*. `crossterm` honours `NO_COLOR` where it
/// writes the escape — `Colored::ansi_color_disabled`, per
/// <https://no-color.org> — so a `Style` carrying a background is simply
/// written without one, and nothing above ever learns that happened. For most
/// of a screen that is exactly right: text drawn in the terminal's own
/// foreground is still text.
///
/// **It is wrong for anything whose whole content is a colour.** Design
/// Language §3's state bar is one cell of *background* and no glyph, so with
/// the colour dropped it is a space: the unseen markers vanish, and `CP-5`'s
/// thesis — that the gutter pulls your eye to what Claude touched — has nothing
/// to pull with. §8 already says what to do instead (*"markers become `▎`"*)
/// and `phosphor_ui::gutter::state_cell` already implements it as
/// `Fill::Marker`; what was missing was anybody deciding to ask for it.
///
/// So this is the question the widget layer cannot answer for itself:
/// `phosphor-ui` takes `ratatui-core` only and reads no environment by design.
/// Terminal capability is this crate's, alongside raw mode and the keyboard
/// protocol.
///
/// # The rule, matched to `crossterm`'s deliberately
///
/// Set **and non-empty**. That is `no-color.org`'s wording and
/// `crossterm`'s implementation, and matching it is the point: a build that
/// degraded on a different rule than the one that drops the escapes would draw
/// markers on a terminal still getting colour, or blocks on one that is not.
/// Read live rather than memoized — `crossterm` memoizes, and the difference
/// only shows in a test that sets the variable, which is exactly where being
/// able to see the change is worth more than one atomic load.
#[must_use]
pub fn colour_available() -> bool {
    colour_from(std::env::var("NO_COLOR").ok().as_deref())
}

/// The rule alone, with the environment read taken out of it.
///
/// **Split out so it can be tested at all.** The workspace denies `unsafe_code`
/// and `std::env::set_var` is unsafe in this edition, so a test that exercised
/// [`colour_available`] by setting the variable could not be written — which is
/// why, until `T088`'s step-3 verification walked §8's degradation chain and
/// reported it, this rule was asserted by nothing. The doc above says the
/// distinction *"only shows in a test that sets the variable"*; the way to have
/// that test is to make the part worth testing not need one.
///
/// `None` is unset. `Some("")` is set-and-empty, which means colour is **fine**.
#[must_use]
const fn colour_from(no_colour: Option<&str>) -> bool {
    match no_colour {
        Some(value) => value.is_empty(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        ALT_SCREEN, Capabilities, ENTERED, KEYBOARD_ENV, KITTY, KeyboardProtocol, MOUSE, Ordering,
        RAW_MODE, TermConfig, TermError, colour_from, mark, restore_entered,
    };

    /// **`NO_COLOR` is set-and-non-empty, not merely set** — and until this
    /// test nothing anywhere asserted it.
    ///
    /// Found by `T088`'s step-3 verification, which walked §8's degradation
    /// chain link by link and reported this one covered by neither a test nor
    /// anything that runs in CI.
    ///
    /// The empty case is the one worth having. `NO_COLOR=` means colour is
    /// **fine**: `no-color.org`'s wording is set-and-non-empty and `crossterm`
    /// implements exactly that, so a build that read an empty value as "no
    /// colour" would paint `▎` markers on a terminal still receiving escapes —
    /// degrading against a different rule from the one that drops them, which
    /// is the failure this function exists to avoid.
    #[test]
    fn no_colour_is_set_and_non_empty_rather_than_merely_set() {
        assert!(colour_from(None), "unset means colour");
        assert!(
            colour_from(Some("")),
            "set but EMPTY still means colour — no-color.org's rule, and \
             crossterm's, and the reason this is not a bare `is_ok`",
        );
        assert!(!colour_from(Some("1")), "set and non-empty means no colour");
        assert!(
            !colour_from(Some("anything at all")),
            "the value is not parsed — any non-empty string is the signal",
        );
    }

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
    fn the_override_forces_either_direction_and_ignores_the_rest() {
        // `T027`. Forcing legacy is what makes `CP-3`'s *"then on the
        // degradation terminal"* runnable on a machine with one good terminal;
        // forcing kitty is for an emulator whose query answers badly.
        for value in ["legacy", "LEGACY", " legacy ", "0", "off", "false", "none"] {
            assert_eq!(
                KeyboardProtocol::from_env_value(Some(value)),
                Some(KeyboardProtocol::Legacy),
                "{value:?}"
            );
        }
        for value in ["kitty", "Kitty", "1", "on", "true", "force"] {
            assert_eq!(
                KeyboardProtocol::from_env_value(Some(value)),
                Some(KeyboardProtocol::Kitty),
                "{value:?}"
            );
        }
        // Unset, empty and unrecognised all leave the query in charge. Setup is
        // the wrong place to refuse: there is nowhere to report to yet, and a
        // typo must not stop the editor from starting.
        for value in [
            None,
            Some(""),
            Some("  "),
            Some("yes"),
            Some("2"),
            Some("k"),
        ] {
            assert_eq!(KeyboardProtocol::from_env_value(value), None, "{value:?}");
        }
    }

    #[test]
    fn the_override_is_read_from_one_name() {
        // Named in the crate docs and in `T027`'s report; a rename that missed
        // one of them would be a documented variable nothing reads.
        assert_eq!(KEYBOARD_ENV, "PHOSPHOR_KEYBOARD");
    }

    #[test]
    fn already_active_reads_legibly() {
        assert_eq!(
            TermError::AlreadyActive.to_string(),
            "a Term is already active in this process"
        );
    }

    #[test]
    fn the_two_escape_hatches_do_not_spell_the_same_words() {
        // `from_env_value`'s doc said the vocabulary *"follows
        // `PHOSPHOR_UNDERCURL`'s"* for two windows. It does not, and a user who
        // read that and exported `PHOSPHOR_KEYBOARD=yes` would get the query
        // deciding rather than the protocol they asked for — silently, because
        // an unrecognised value is deliberately ignored here.
        //
        // The list is `TerminalEnv::forced` in
        // `vendor/ratatui-code-editor/src/phosphor/cell_style.rs`: on is
        // `1|true|on|yes|always|force`, off is `0|false|off|no|never`.
        for undercurl_only in ["yes", "always", "no", "never"] {
            assert_eq!(
                KeyboardProtocol::from_env_value(Some(undercurl_only)),
                None,
                "{undercurl_only:?} forces PHOSPHOR_UNDERCURL and is ignored here"
            );
        }
        // The canary, deterministically. `NEAR_MISSES` puts it in front of the
        // property too, but a sampled generator is a probability and this is
        // the one value in the suite whose whole purpose is being rejected.
        assert_eq!(KeyboardProtocol::from_env_value(Some("kitty-please")), None);
        assert_eq!(
            KeyboardProtocol::from_env_value(Some("none")),
            Some(KeyboardProtocol::Legacy),
            "`none` goes the other way: this hatch takes it and that one does not"
        );
    }

    /// Every spelling the doc comment promises, as a second copy.
    ///
    /// Two copies of a list is normally a smell; here it is the point. The
    /// property below asserts that `Some` is returned for **nothing else**, and
    /// a property that read the same list the implementation reads would be
    /// asserting that a function agrees with itself.
    const DOCUMENTED: [&str; 10] = [
        "legacy", "0", "off", "false", "none", "kitty", "1", "on", "true", "force",
    ];

    /// Words a person would plausibly type, and the function must reject.
    ///
    /// **Without these the property below cannot fail.** The first version
    /// generated only `any::<String>()`, and a planted `"yes" => Kitty` arm
    /// survived all 256 cases — proptest's `\PC*` does not produce
    /// three-letter English words by chance, so the law was true and untested,
    /// which is the shape of the seven dead tests this build has already
    /// shipped. Four of these are `PHOSPHOR_UNDERCURL`'s spellings, which is
    /// the exact mistake a reader of the old doc comment would have made.
    ///
    /// `kitty-please` is the canary, and it is here rather than in
    /// [`DOCUMENTED`] — where it sat, because [`DOCUMENTED`] is the property's
    /// *allowed* set and law 1 reads `plain.is_some() ⇒ DOCUMENTED.contains`.
    /// A rejected value in the allowed set widens the law by one and asserts
    /// nothing, so a planted `"kitty-please" => Kitty` arm passed the whole
    /// suite. In this list the same property goes red on it, which is the job
    /// the canary was written for.
    const NEAR_MISSES: [&str; 15] = [
        "yes",
        "no",
        "always",
        "never",
        "y",
        "n",
        "enable",
        "disable",
        "2",
        "-1",
        "kitty!",
        "legacyy",
        "",
        "  ",
        "kitty-please",
    ];

    /// The values the property is run over: free-form strings, the documented
    /// spellings, and the near misses above, so the vocabulary's boundary is
    /// somewhere the generator can actually land.
    fn value() -> impl Strategy<Value = String> {
        prop_oneof![
            2 => any::<String>(),
            3 => prop::sample::select(DOCUMENTED.as_slice()).prop_map(str::to_owned),
            3 => prop::sample::select(NEAR_MISSES.as_slice()).prop_map(str::to_owned),
        ]
    }

    proptest! {
        /// **The vocabulary is closed**, and case and padding do not open it.
        ///
        /// Three laws, none of them a restatement of the match arms:
        ///
        /// 1. Nothing outside [`DOCUMENTED`] ever forces a protocol. This is
        ///    the claim the doc makes — *"anything else, including unset and
        ///    empty → `None`, and the query decides"* — and it is what stops a
        ///    typo from silently selecting the degradation path.
        /// 2. ASCII case is irrelevant. `to_ascii_lowercase` is what makes that
        ///    true and folds nothing else, which is why the generator flips
        ///    only ASCII letters.
        /// 3. Surrounding ASCII whitespace is irrelevant — a value pasted from
        ///    a shell script with a trailing space still works.
        ///
        /// **What the generator cannot produce**: proptest's `String` strategy
        /// is `\PC*`, so no control characters and no lone surrogates; and no
        /// generator here can produce a **non-UTF-8** value, which is the one
        /// input `KeyboardProtocol::forced` actually treats differently —
        /// `std::env::var(..).ok()` turns it into `None` before this function
        /// ever sees it. Nor does it produce non-ASCII whitespace such as
        /// U+00A0, which `str::trim` *does* strip: that widens law 3 rather
        /// than narrowing it, and is untested here. What it *can* produce, and
        /// deliberately, is [`NEAR_MISSES`] — see that constant for why a
        /// free-form generator alone left this property unable to fail.
        #[test]
        fn no_undocumented_spelling_ever_forces_a_protocol(
            value in value(),
            pad_left in prop::collection::vec(prop_oneof![Just(' '), Just('\t')], 0..4),
            pad_right in prop::collection::vec(prop_oneof![Just(' '), Just('\t')], 0..4),
            upper in any::<bool>(),
        ) {
            let plain = KeyboardProtocol::from_env_value(Some(&value));

            if plain.is_some() {
                let folded = value.trim().to_ascii_lowercase();
                prop_assert!(
                    DOCUMENTED.contains(&folded.as_str()),
                    "{value:?} forced {plain:?} and is not a spelling the docs offer"
                );
            }

            let cased: String = if upper {
                value.to_ascii_uppercase()
            } else {
                value.to_ascii_lowercase()
            };
            let padded: String = pad_left
                .iter()
                .chain(cased.chars().collect::<Vec<_>>().iter())
                .chain(pad_right.iter())
                .collect();
            prop_assert_eq!(
                KeyboardProtocol::from_env_value(Some(&padded)),
                plain,
                "{:?} and {:?} disagree",
                padded,
                value
            );
        }
    }
}
