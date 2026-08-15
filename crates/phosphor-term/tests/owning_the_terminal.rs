//! **A real `Term`, on a real terminal** — the crate's first integration test.
//!
//! # Why this file exists
//!
//! `phosphor-term` had eleven unit tests and nothing else, and its assurance
//! for the one thing Design Language §8 calls a P0 — *"synchronized output
//! wraps every frame; a torn frame is a P0 bug"* — was **accidental**.
//! `crates/phosphor/tests/loop_pty.rs` counts `\x1b[?2026l` to check frame
//! *accounting*, one frame per key. So the only end-to-end exercise of the
//! synchronized-output wrapper in this repository lived in a different crate's
//! test asserting a different property, and it would disappear the day somebody
//! rewrote that harness for a reason unrelated to terminals. A test that
//! protects something it does not mention is not a test of that thing.
//!
//! # Why a pty, and why a child process
//!
//! Three of this crate's four obligations are invisible in memory:
//!
//! * **Raw mode is termios, not an escape sequence.** "The user's shell is
//!   unusable after the editor died" cannot be read off a byte stream at all —
//!   it is `ECHO` and `ICANON` on the tty, which is why this file takes
//!   `rustix`'s `termios` feature and reads the pty's state after the child is
//!   gone.
//! * **Restore ordering is a property of the wire**, and `restore_entered`
//!   writes to `io::stdout()` rather than to anything injectable. What it
//!   undoes, and in what order, is only observable from outside the process.
//! * **The panic path ends the process.** An in-process test can catch the
//!   unwind but cannot watch the panic hook, the `Drop` and the unwinding
//!   `catch_unwind` all reach the same terminal.
//!
//! So each test starts **this test binary again** with a pty on its standard
//! streams and a mode in `$PHOSPHOR_TERM_CHILD`, and reads what came back.
//! [`on_a_pty::child_process_body`] is the child; it returns immediately when
//! the variable is unset, so under `just test` it costs one no-op. The
//! re-execution idiom is `crates/phosphor-core/tests/journal.rs`'s, which
//! spawns itself to be `SIGKILL`ed; the pty half is
//! `crates/phosphor/tests/loop_pty.rs`'s.
//!
//! # What the child says, and how
//!
//! **Through the only door.** A child reports what it found — the termios it is
//! running under, its [`Capabilities`], its screen size — by *drawing it*, one
//! fact per row, through `Term::draw`. There is no side channel, which is the
//! point: every byte these tests assert on came out of the wrapper under test.
//!
//! Owned by `spine`.

#[cfg(not(unix))]
#[test]
fn a_terminal_is_only_observable_from_outside_the_process_on_unix() {
    // A pty is a unix object, and so is termios. Everything asserted here is
    // platform-independent in principle; what cannot be done elsewhere is watch
    // it happen to a terminal somebody else owns.
}

#[cfg(unix)]
mod on_a_pty {
    use std::ffi::OsString;
    use std::fs::{File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::unix::ffi::OsStringExt as _;
    use std::path::PathBuf;
    use std::process::{Command, ExitStatus, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use phosphor_term::{KEYBOARD_ENV, KeyboardProtocol, Term, TermConfig, TermError};
    use ratatui::layout::Rect;
    use ratatui::widgets::Paragraph;
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    use rustix::termios::{LocalModes, Winsize, tcgetattr, tcsetwinsize};

    /// Which child to be. Unset means "be an ordinary no-op test".
    const CHILD_ENV: &str = "PHOSPHOR_TERM_CHILD";

    /// The screen the child lays out at. Any size does; these are the numbers
    /// `loop_pty.rs` uses, so a failure here and a failure there describe the
    /// same window.
    const SCREEN: Winsize = Winsize {
        ws_row: 30,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // The wire vocabulary. Every one of these is a byte string crossterm 0.29
    // emits, checked against its source rather than remembered.
    const BEGIN: &[u8] = b"\x1b[?2026h";
    const END: &[u8] = b"\x1b[?2026l";
    const ALT_ENTER: &[u8] = b"\x1b[?1049h";
    const ALT_LEAVE: &[u8] = b"\x1b[?1049l";
    /// SGR mouse reporting — the last of the five modes `EnableMouseCapture`
    /// sets, and the one no other feature here uses.
    const MOUSE_ON: &[u8] = b"\x1b[?1006h";
    const MOUSE_OFF: &[u8] = b"\x1b[?1006l";
    /// `PushKeyboardEnhancementFlags` with the three flags `lib.rs` chooses:
    /// `DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES | REPORT_ALTERNATE_KEYS`
    /// is `0b111`, and crossterm writes `CSI > <bits> u`.
    const KITTY_PUSH: &[u8] = b"\x1b[>7u";
    const KITTY_POP: &[u8] = b"\x1b[<1u";
    /// The progressive-enhancement query `supports_keyboard_enhancement` sends.
    /// Its absence is how "negotiation was skipped" is checked.
    const KITTY_QUERY: &[u8] = b"\x1b[?u";
    const CURSOR_SHOW: &[u8] = b"\x1b[?25h";
    /// The primary device attributes query, and a plain VT100's answer to it.
    /// Answering ends negotiation immediately instead of after crossterm's
    /// two-second timeout.
    const DA1_QUERY: &[u8] = b"\x1b[c";
    const DA1_REPLY: &[u8] = b"\x1b[?6c";

    // -----------------------------------------------------------------------
    // The harness
    // -----------------------------------------------------------------------

    /// One child's whole life: what it drew, how it ended, and the state it
    /// left the terminal in.
    #[derive(Debug)]
    struct Session {
        wire: Vec<u8>,
        status: ExitStatus,
        /// `tcgetattr` on the pty once the child is reaped, read through an fd
        /// held open across its whole life — see [`run`] for why the fd has to
        /// be that one. This is the "can the user still use their shell"
        /// question, and it has no spelling as an escape sequence.
        after: LocalModes,
    }

    impl Session {
        /// What the child drew, escapes stripped.
        fn text(&self) -> String {
            printable(&self.wire)
        }

        /// Every `?2026h` / `?2026l` in order — `true` opens, `false` closes.
        fn blocks(&self) -> Vec<bool> {
            markers(&self.wire)
        }

        fn says(&self, wanted: &str) -> bool {
            self.text().contains(wanted)
        }
    }

    /// Runs one child to completion and collects everything about it.
    ///
    /// `$PHOSPHOR_KEYBOARD` is **removed** unless a caller sets it: this suite
    /// decides which side of `T027`'s negotiation each test exercises, and a
    /// developer who exported the override for their own session must not
    /// silently rewrite the assertions.
    fn run(mode: &str, env: &[(&str, &str)]) -> Session {
        let (master, slave_path) = open_pty();
        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&slave_path)
            .expect("the pty slave opens");
        // Apple's master rejects `TIOCSWINSZ`; the slave takes it on both
        // platforms and is the fd the child asks anyway.
        tcsetwinsize(&slave, SCREEN).expect("the pty takes a window size");

        let exe = std::env::current_exe().expect("this test binary");
        let mut command = Command::new(exe);
        command
            .args([
                "on_a_pty::child_process_body",
                "--exact",
                // Without this libtest buffers the child's panic message, and
                // "the panic lands where the user can read it" is one of the
                // things being asserted.
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(CHILD_ENV, mode)
            .env("TERM", "xterm-256color")
            .env_remove(KEYBOARD_ENV)
            .stdin(Stdio::from(slave.try_clone().expect("the slave clones")))
            .stdout(Stdio::from(slave.try_clone().expect("the slave clones")))
            .stderr(Stdio::from(slave.try_clone().expect("the slave clones")));
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("the child starts");

        // A `Command` keeps its `Stdio` handles after `spawn` — it can be
        // spawned again — so the three slave clones handed to it stay open
        // until the builder itself is dropped, and a master with a live slave
        // never reaches end-of-file. `loop_pty.rs` gets this for free by
        // building the command as a temporary inside the `spawn()` expression;
        // this one has to say so.
        drop(command);

        let wire = Arc::new(Mutex::new(Vec::new()));
        let reader = spawn_reader(Arc::clone(&master), Arc::clone(&wire));

        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            match child.try_wait().expect("the child is waitable") {
                Some(status) => break status,
                None if Instant::now() >= deadline => {
                    child.kill().expect("kill a hung child");
                    break child.wait().expect("reap");
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };

        // **Read the termios before the last slave fd closes, and keep that fd
        // open for exactly this reason.**
        //
        // The obvious shape is to drop every slave fd up front — so the master
        // reaches end-of-file cleanly — and re-open `slave_path` afterwards to
        // ask what state the child left. That version was written, it passed,
        // and it was worthless: with `disable_raw_mode` deleted from
        // `restore_entered`, every assertion below still held. A pts whose last
        // opener has closed is reinitialised to the kernel default when it is
        // next opened, so the re-opened fd reports canonical mode with echo on
        // whether or not the child restored anything.
        //
        // Holding one fd across the child's whole life is what makes the
        // question answerable: this is the same pts, never closed, and its
        // termios is whatever the child left in it.
        let after = tcgetattr(&slave)
            .expect("the pty has a termios")
            .local_modes;

        // Only now — and the reader cannot finish until this happens.
        drop(slave);
        reader.join().expect("the reader thread does not panic");

        Session {
            wire: Arc::try_unwrap(wire)
                .expect("the reader is joined")
                .into_inner()
                .expect("no writer panicked"),
            status,
            after,
        }
    }

    fn open_pty() -> (Arc<File>, PathBuf) {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pty is available");
        grantpt(&master).expect("the pty is granted");
        unlockpt(&master).expect("the pty is unlocked");
        let name = ptsname(&master, Vec::new()).expect("the pty has a name");
        let path = PathBuf::from(OsString::from_vec(name.into_bytes()));
        (Arc::new(File::from(master)), path)
    }

    /// Drains the master until end-of-file, answering the device-attributes
    /// query the first time it appears.
    ///
    /// The answer is a plain VT100's — *"no kitty protocol here"* — because a
    /// terminal that supports it is the case `$PHOSPHOR_KEYBOARD=kitty` covers
    /// without needing different hardware.
    fn spawn_reader(master: Arc<File>, wire: Arc<Mutex<Vec<u8>>>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut answered = false;
            loop {
                // End-of-file reads zero on Apple platforms and fails with
                // `EIO` on Linux; both mean the child is gone.
                let Ok(read @ 1..) = (&*master).read(&mut buffer) else {
                    return;
                };
                let chunk = &buffer[..read];
                let mut held = wire.lock().expect("no other writer panics");
                held.extend_from_slice(chunk);
                if !answered && find(&held, DA1_QUERY).is_some() {
                    answered = true;
                    let _ = (&*master).write_all(DA1_REPLY);
                }
            }
        })
    }

    // -----------------------------------------------------------------------
    // Reading the wire
    // -----------------------------------------------------------------------

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn count(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count()
    }

    /// The marker starting at `at`, if one does. `true` opens, `false` closes.
    fn marker_at(bytes: &[u8], at: usize) -> Option<bool> {
        let rest = &bytes[at..];
        if rest.starts_with(BEGIN) {
            Some(true)
        } else if rest.starts_with(END) {
            Some(false)
        } else {
            None
        }
    }

    /// Every `?2026h` and `?2026l`, in order. `true` opens, `false` closes.
    ///
    /// The two cannot overlap — they differ in one byte and neither is a prefix
    /// of the other — so one left-to-right scan is exact.
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

    /// **The P0, as a predicate.** No opener may be followed by another opener
    /// with no closer between them, and the stream may not end open.
    ///
    /// Surplus *closers* are allowed and are not an oversight — see
    /// [`the_teardown_emits_a_closer_nothing_opened`], which is the test that
    /// names why there is always exactly one.
    fn never_left_open(markers: &[bool]) -> bool {
        let mut open = false;
        for marker in markers {
            if *marker && open {
                return false;
            }
            open = *marker;
        }
        !open
    }

    /// Printable runs, escapes replaced by a space — enough to answer *"is this
    /// word on the frame"*, which is all any assertion here asks. Deliberately
    /// not a terminal emulator; the exact cell grid is Tier 1's job.
    ///
    /// Copied in shape from `loop_pty.rs`'s, because two harnesses that read
    /// frames differently are two harnesses that disagree about a failure.
    fn printable(bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes);
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(character) = chars.next() {
            if character != '\u{1b}' {
                out.push(if character.is_control() {
                    ' '
                } else {
                    character
                });
                continue;
            }
            match chars.next() {
                Some('[') => {
                    for parameter in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&parameter) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(inner) = chars.next() {
                        if inner == '\u{7}' {
                            break;
                        }
                        if inner == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
            out.push(' ');
        }
        out
    }

    // -----------------------------------------------------------------------
    // The synchronized-output wrapper — Design Language §8's P0
    // -----------------------------------------------------------------------

    #[test]
    fn every_frame_a_real_term_draws_is_inside_a_synchronized_block() {
        let session = run("frames", &[("PHOSPHOR_KEYBOARD", "legacy")]);
        assert!(
            session.status.success(),
            "the child failed: {}",
            session.text()
        );

        let blocks = session.blocks();
        assert!(
            never_left_open(&blocks),
            "a synchronized-output block was left open — the emulator holds the \
             last good frame and the editor looks hung. Markers: {blocks:?}"
        );
        // Exactly, not merely well-formed. `never_left_open` tolerates surplus
        // closers on purpose (see `the_teardown_emits_a_closer_nothing_opened`),
        // and a wrapper that closed early and re-closed at the end would satisfy
        // it while drawing every frame outside a block — which is the mutation
        // that got past the first version of this test.
        assert_eq!(
            blocks,
            vec![true, false, true, false, true, false, false],
            "three frames, each its own pair, and one closer at teardown"
        );
    }

    #[test]
    fn frame_content_lands_between_the_two_markers_and_not_outside_them() {
        // The wrapper being present is not the property. The property is that
        // the *frame* is inside it, which a wrapper emitted around nothing
        // would also satisfy.
        let session = run("frames", &[("PHOSPHOR_KEYBOARD", "legacy")]);
        let marker = b"phosphor-term-frame-three";
        let at = find(&session.wire, marker).expect("the child drew its third frame");

        // **The nearest marker on each side**, not the nearest of each kind.
        // Asking for the nearest *open* before and the nearest *close* after
        // is satisfied by `?2026h ?2026l <frame> ?2026l`, where the frame is
        // outside every block — and that is exactly what a planted mutation
        // produced while the first version of this test passed.
        let before = (0..at)
            .rev()
            .find_map(|n| marker_at(&session.wire, n))
            .expect("some marker precedes the frame's bytes");
        let after = (at..session.wire.len())
            .find_map(|n| marker_at(&session.wire, n))
            .expect("some marker follows the frame's bytes");
        assert!(
            before,
            "the marker immediately before the frame's bytes was a CLOSER — the \
             frame was drawn outside the synchronized-output block"
        );
        assert!(
            !after,
            "the marker immediately after the frame's bytes was an OPENER — the \
             frame's block was never closed"
        );
    }

    #[test]
    fn the_teardown_emits_a_closer_nothing_opened() {
        // **A finding, pinned.** `restore_entered` opens with an unconditional
        // `EndSynchronizedUpdate`, deliberately — a `panic = "abort"` build or
        // a signal mid-frame does not unwind, and a terminal stuck inside
        // ?2026 reads as a hang. The consequence is that a clean session emits
        // one more closer than it does openers.
        //
        // That is harmless to a terminal and **not** harmless to a harness
        // that counts frames by counting markers, and this repository has two
        // of those. `phosphor/benches/vm_invocations.rs` counts `?2026h` and
        // is unaffected; `phosphor/tests/loop_pty.rs` counts `?2026l` — its
        // `FRAME` const — and therefore sees one frame that never happened,
        // at shutdown. It gets away with it because the surplus arrives after
        // the last `press`, which is a property of where `ZQ` sits in those
        // tests rather than of the accounting. This is the test that says so
        // out loud, so the next harness that counts closers knows the last one
        // is not a frame.
        let session = run("frames", &[("PHOSPHOR_KEYBOARD", "legacy")]);
        assert_eq!(
            count(&session.wire, END),
            count(&session.wire, BEGIN) + 1,
            "exactly one surplus closer, at teardown"
        );
    }

    // -----------------------------------------------------------------------
    // Restore — the failure that outlives the process
    // -----------------------------------------------------------------------

    #[test]
    fn the_shell_still_echoes_after_the_editor_exits() {
        // If this fails the developer running the suite has a terminal that no
        // longer echoes what they type, which is the exact symptom, so the
        // assertion and the blast radius are the same thing.
        let session = run("frames", &[("PHOSPHOR_KEYBOARD", "legacy")]);

        // Both halves, or the test passes on a child that never entered raw
        // mode at all. The child reads its own termios through fd 0 and draws
        // what it found.
        assert!(
            session.says("termios echo=0 icanon=0"),
            "raw mode was never actually on, so the restore below proves \
             nothing. Drew: {}",
            session.text()
        );
        assert!(
            session.after.contains(LocalModes::ECHO),
            "the terminal was left with echo off — the user's shell is now silent"
        );
        assert!(
            session.after.contains(LocalModes::ICANON),
            "the terminal was left in non-canonical mode — the user's shell \
             no longer waits for a newline"
        );
    }

    #[test]
    fn a_panic_hands_the_terminal_back_before_the_message_lands() {
        let session = run("panic", &[("PHOSPHOR_KEYBOARD", "legacy")]);
        assert!(
            !session.status.success(),
            "the child was supposed to die of a panic"
        );

        assert!(
            session.after.contains(LocalModes::ECHO) && session.after.contains(LocalModes::ICANON),
            "a panic left raw mode on — the user's shell is unusable and the \
             backtrace they need to report is being typed into it invisibly"
        );
        let leave = find(&session.wire, ALT_LEAVE).expect("the alternate screen was left");
        let message = find(&session.wire, b"a widget blew up mid-frame")
            .expect("the panic message reached the terminal");
        assert!(
            leave < message,
            "the panic message was printed to the alternate screen, which is \
             about to vanish and take the message with it"
        );
    }

    #[test]
    fn a_panic_closes_the_block_before_it_switches_screens() {
        // The ordering `restore_entered`'s first line exists for. A terminal
        // inside ?2026 has stopped updating, so switching back to the normal
        // screen while it is frozen shows the user a stale alternate screen
        // and a panic message they cannot see until the closer arrives.
        let session = run("panic", &[("PHOSPHOR_KEYBOARD", "legacy")]);

        let blocks = session.blocks();
        assert!(
            never_left_open(&blocks),
            "the panicking frame's block was never closed: {blocks:?}"
        );

        let last_open = (0..session.wire.len())
            .rfind(|n| session.wire[*n..].starts_with(BEGIN))
            .expect("the panicking frame opened a block");
        let close_after = (last_open..session.wire.len())
            .find(|n| session.wire[*n..].starts_with(END))
            .expect("and something closed it");
        let leave = find(&session.wire, ALT_LEAVE).expect("the alternate screen was left");
        assert!(
            close_after < leave,
            "the alternate screen was left while the terminal was still frozen \
             inside ?2026"
        );
    }

    #[test]
    fn restore_undoes_setup_in_the_order_the_comments_promise() {
        // `restore_entered`'s body is four conditional steps and a comment
        // arguing their order; nothing checked that the wire agreed. Kitty
        // first, then mouse, then — invisibly, between these two — raw mode,
        // then the alternate screen, then the cursor.
        let session = run("frames", &[("PHOSPHOR_KEYBOARD", "kitty")]);

        let pop = find(&session.wire, KITTY_POP).expect("the kitty flags were popped");
        let mouse = find(&session.wire, MOUSE_OFF).expect("mouse capture was disabled");
        let leave = find(&session.wire, ALT_LEAVE).expect("the alternate screen was left");
        let cursor = find(&session.wire, CURSOR_SHOW).expect("the cursor was shown");
        assert!(
            pop < mouse && mouse < leave && leave < cursor,
            "restore ran out of order: kitty {pop}, mouse {mouse}, alt {leave}, \
             cursor {cursor}"
        );
    }

    #[test]
    fn dropping_the_term_restores_exactly_what_restore_does() {
        // `main.rs` calls `Term::restore()`; every error path out of `main`
        // reaches `Drop` instead. Two restore paths that differ is a terminal
        // that comes back only when nothing went wrong.
        let explicit = run("frames", &[("PHOSPHOR_KEYBOARD", "kitty")]);
        let dropped = run("drop", &[("PHOSPHOR_KEYBOARD", "kitty")]);

        for (marker, name) in [
            (KITTY_POP, "the kitty pop"),
            (MOUSE_OFF, "the mouse disable"),
            (ALT_LEAVE, "the alternate-screen leave"),
            (CURSOR_SHOW, "the cursor show"),
            (END, "the synchronized-output closer"),
        ] {
            assert_eq!(
                count(&explicit.wire, marker),
                count(&dropped.wire, marker),
                "{name} differs between `restore()` and `Drop`"
            );
        }
        assert!(dropped.after.contains(LocalModes::ECHO));
        assert!(dropped.after.contains(LocalModes::ICANON));
    }

    // -----------------------------------------------------------------------
    // Capability detection
    // -----------------------------------------------------------------------

    #[test]
    fn forcing_legacy_skips_the_negotiation_rather_than_ignoring_it() {
        // `KeyboardProtocol::from_env_value`'s doc: *"Negotiation is skipped
        // entirely rather than pushed and ignored, so what the editor receives
        // is what a terminal without the protocol would send."* That is a claim
        // about bytes, and it was untested — the unit tests cover the pure
        // function, and a `Term` that pushed the flags anyway would pass every
        // one of them.
        let session = run("report", &[("PHOSPHOR_KEYBOARD", "legacy")]);

        assert!(session.says("caps kb=legacy"));
        assert_eq!(
            count(&session.wire, KITTY_QUERY),
            0,
            "the terminal was asked, so the degradation terminal is not what \
             this run exercised"
        );
        assert_eq!(count(&session.wire, KITTY_PUSH), 0, "the flags were pushed");
        assert_eq!(
            count(&session.wire, KITTY_POP),
            0,
            "flags nobody pushed were popped at teardown"
        );
    }

    #[test]
    fn forcing_kitty_pushes_the_flags_without_asking_first() {
        // The other direction, and the other doc claim: *"For an emulator that
        // supports the protocol but answers the query badly — a multiplexer in
        // the middle is the usual reason. The flags are pushed without asking
        // first."*
        let session = run("report", &[("PHOSPHOR_KEYBOARD", "kitty")]);

        assert!(session.says("caps kb=kitty"));
        assert_eq!(
            count(&session.wire, KITTY_QUERY),
            0,
            "forcing kitty still queried the terminal"
        );
        let push = find(&session.wire, KITTY_PUSH).expect("the flags were pushed");
        let pop = find(&session.wire, KITTY_POP).expect("and popped at teardown");
        assert!(push < pop);
    }

    #[test]
    fn a_terminal_that_answers_plain_vt100_gets_the_legacy_protocol() {
        // The only test here that runs the real query. The reader answers
        // `CSI ? 6 c` — a VT102 with no kitty support — and the child must
        // treat that as a *capability*, not a failure: `Term::new` returns
        // `Ok` and reports `Legacy`.
        //
        // **What this cannot control**, and it is worth knowing before reading
        // a slow run: crossterm writes the query to `/dev/tty` and only falls
        // back to stdout if that open fails, so under an interactive
        // `cargo test` the query can go to the developer's own terminal while
        // the *answer* is read from this pty. The outcome asserted below is the
        // same either way — nobody answers, crossterm times out at two seconds,
        // and legacy is what comes back — which is why this is the one test
        // allowed to depend on it.
        let session = run("report", &[]);

        assert!(session.status.success(), "drew: {}", session.text());
        assert!(
            session.says("caps kb=legacy"),
            "a terminal that says no must be a capability and never an error. \
             Drew: {}",
            session.text()
        );
        assert_eq!(count(&session.wire, KITTY_PUSH), 0);
    }

    #[test]
    fn switching_the_config_off_leaves_those_parts_of_the_terminal_alone() {
        // `TermConfig`'s two knobs, on the wire. Nothing had checked that
        // turning one off *removes* its bytes rather than merely reporting
        // `false`, and a `Capabilities` that lied about mouse capture would
        // give `T081`'s click-to-position a working test and a dead feature.
        let session = run("bare", &[]);

        assert!(session.says("caps kb=legacy mouse=0"));
        assert_eq!(
            count(&session.wire, MOUSE_ON),
            0,
            "mouse capture was enabled"
        );
        assert_eq!(
            count(&session.wire, MOUSE_OFF),
            0,
            "and disabled at teardown"
        );
        assert_eq!(count(&session.wire, KITTY_QUERY), 0);
        assert_eq!(count(&session.wire, KITTY_PUSH), 0);
        // The alternate screen and raw mode are not knobs, and must still be
        // there — otherwise this test would pass on a `Term` that did nothing.
        assert_eq!(count(&session.wire, ALT_ENTER), 1);
        assert!(session.says("termios echo=0 icanon=0"));
    }

    #[test]
    fn the_size_a_term_reports_is_the_terminal_it_is_on() {
        // `Term::size` is on the frame path — `main.rs` calls it once a frame
        // to lay out — and its only test was that it compiled.
        let session = run("report", &[("PHOSPHOR_KEYBOARD", "legacy")]);
        assert!(
            session.says(&format!("size {}x{}", SCREEN.ws_col, SCREEN.ws_row)),
            "drew: {}",
            session.text()
        );
    }

    // -----------------------------------------------------------------------
    // One terminal, one Term
    // -----------------------------------------------------------------------

    #[test]
    fn a_second_term_is_refused_without_disturbing_the_first() {
        // `TermError::AlreadyActive` exists because two `Term`s mean two
        // restore paths for one terminal. The unit test checks its `Display`;
        // what matters is the half no unit test can reach — that the *refusal*
        // is inert. `with_config` takes the `ACTIVE` flag before it enters
        // anything, so the second construction never reaches the failure path
        // that calls `restore_entered`; if it ever did, asking for a second
        // `Term` would tear down the live one's terminal from under it.
        let session = run("second", &[("PHOSPHOR_KEYBOARD", "legacy")]);

        assert!(session.status.success(), "drew: {}", session.text());
        assert!(
            session.says("second term refused"),
            "a second Term was handed out. Drew: {}",
            session.text()
        );
        assert!(
            session.says("first term still draws"),
            "the refusal tore down the terminal the first Term owns. Drew: {}",
            session.text()
        );
        assert!(never_left_open(&session.blocks()), "{:?}", session.blocks());
        // One entry and one leave: the refused construction entered nothing.
        assert_eq!(count(&session.wire, ALT_ENTER), 1);
        assert_eq!(count(&session.wire, ALT_LEAVE), 1);
    }

    // -----------------------------------------------------------------------
    // The child
    // -----------------------------------------------------------------------

    /// One row of the child's report, drawn through the only door there is.
    fn say(term: &mut Term, row: u16, text: &str) {
        let line = text.to_owned();
        term.draw(|frame| {
            let area = frame.area();
            let row = Rect::new(0, row.min(area.height.saturating_sub(1)), area.width, 1);
            frame.render_widget(Paragraph::new(line.as_str()), row);
        })
        .expect("a real terminal takes a frame");
    }

    /// What the pty's line discipline looks like from inside the child.
    fn termios_now() -> String {
        let modes = tcgetattr(std::io::stdin())
            .expect("the child's stdin is a terminal")
            .local_modes;
        format!(
            "termios echo={} icanon={}",
            u8::from(modes.contains(LocalModes::ECHO)),
            u8::from(modes.contains(LocalModes::ICANON)),
        )
    }

    fn caps_line(term: &Term) -> String {
        let caps = term.capabilities();
        format!(
            "caps kb={} mouse={}",
            match caps.keyboard {
                KeyboardProtocol::Kitty => "kitty",
                KeyboardProtocol::Legacy => "legacy",
            },
            u8::from(caps.mouse),
        )
    }

    /// The child, when `$PHOSPHOR_TERM_CHILD` names a mode; a no-op otherwise.
    ///
    /// Every mode reports the same first two rows — the termios it is running
    /// under and the capabilities it negotiated — because every test above
    /// needs at least one of them to know the run was real rather than vacuous.
    #[test]
    fn child_process_body() {
        let Ok(mode) = std::env::var(CHILD_ENV) else {
            return;
        };

        match mode.as_str() {
            "frames" | "drop" => {
                let mut term = Term::new().expect("a pty is a terminal");
                say(&mut term, 0, &termios_now());
                let caps = caps_line(&term);
                say(&mut term, 1, &caps);
                say(&mut term, 2, "phosphor-term-frame-three");
                if mode == "frames" {
                    term.restore().expect("restore reports its own failures");
                }
                // `drop` deliberately falls off the end instead.
            }
            "report" => {
                let mut term = Term::new().expect("a pty is a terminal");
                let size = term.size().expect("the terminal has a size");
                say(&mut term, 0, &termios_now());
                let caps = caps_line(&term);
                say(&mut term, 1, &caps);
                say(
                    &mut term,
                    2,
                    &format!("size {}x{}", size.width, size.height),
                );
                term.restore().expect("restore");
            }
            "bare" => {
                let config = TermConfig {
                    mouse_capture: false,
                    keyboard_enhancement: false,
                };
                let mut term = Term::with_config(config).expect("a pty is a terminal");
                say(&mut term, 0, &termios_now());
                let caps = caps_line(&term);
                say(&mut term, 1, &caps);
                term.restore().expect("restore");
            }
            "second" => {
                let mut term = Term::new().expect("a pty is a terminal");
                let refused = match Term::new() {
                    Err(TermError::AlreadyActive) => "second term refused",
                    Err(TermError::Io(_)) => "second term failed for the wrong reason",
                    Ok(_) => "second term handed out",
                    // `TermError` is `#[non_exhaustive]`; a new variant should
                    // fail this loudly rather than be folded into a pass.
                    Err(_) => "second term failed with an unknown error",
                };
                say(&mut term, 0, &termios_now());
                say(&mut term, 1, refused);
                say(&mut term, 2, "first term still draws");
                term.restore().expect("restore");
            }
            "panic" => {
                let mut term = Term::new().expect("a pty is a terminal");
                say(&mut term, 0, &termios_now());
                let caps = caps_line(&term);
                say(&mut term, 1, &caps);
                // Painted first, so the panic arrives while the emulator is
                // holding a half-written screen — the case that would tear.
                let _ = term.draw(|frame| {
                    frame.render_widget(Paragraph::new("about to fall over"), frame.area());
                    panic!("a widget blew up mid-frame");
                });
                unreachable!("the panic above propagates");
            }
            other => panic!("unknown child mode {other:?}"),
        }
    }
}
