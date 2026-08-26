//! The wiring pass's proof: **the shipping binary, on a pseudoterminal**.
//!
//! # Why these are not unit tests
//!
//! `S3` finished with four surfaces built, tested and unreachable. The `3c`
//! snapshot passed the whole time — it hand-builds the view tree
//! (`crates/phosphor/tests/screen_3c.rs:27-35` says so in its own module docs)
//! — while pressing `SPC` on a real terminal produced a frame that diffed at
//! zero pixels. A test that composes a tree proves the widget; only a test that
//! presses a key proves the editor.
//!
//! So every test here drives `CARGO_BIN_EXE_phosphor` — the same executable
//! `just install` puts on `$PATH` — through a pty, writes bytes a terminal
//! would write, and reads what came back. Nothing is stubbed and nothing is
//! composed here: the keymap is `runtime/keymaps.scm`'s, the statusline is
//! `runtime/statusline.scm`'s, the frame is the loop's.
//!
//! The harness is `benches/vm_invocations.rs`'s (`T091`), which already drives
//! the binary this way and argues the two things that make it work: a frame is
//! exactly one `ESC [ ? 2026 l`, because `phosphor-term` wraps every frame in a
//! synchronized-output block and nothing may emit one outside it; and the
//! reader must drain continuously or the child blocks on a full output buffer
//! and never reads its input. What is new here is that the transcript is
//! **kept** rather than counted, so a test can ask what was drawn.
//!
//! # What "what was drawn" means
//!
//! `printable` strips the SGR and cursor-motion escapes and keeps the printable
//! runs. That is enough to answer *"is this word on the frame"*, which is the
//! question every test here asks, and it is deliberately not a terminal
//! emulator: the exact cell grid at exact coordinates is Tier 1's job and lives
//! in the `.snap` files.
//!
//! Owned by `spine`.

#[cfg(not(unix))]
#[test]
fn the_loop_is_driven_on_a_pty_on_unix_only() {
    // A pty is a unix object. The wiring itself is platform-independent; what
    // cannot be done here is press a key on a real terminal.
}

#[cfg(unix)]
mod driven {
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::unix::ffi::OsStringExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use phosphor_core::config::config_dir_in;
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    use rustix::termios::{Winsize, tcsetwinsize};

    /// The screen the child lays out at. Wide enough for `3c`'s three-column
    /// grid and `8e`'s hint row without either shedding.
    const SCREEN: Winsize = Winsize {
        ws_row: 30,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    /// The synchronized-output **closer** — one per frame, by `T014`'s
    /// construction (`crates/phosphor-term/src/raw.rs:141-142`, and
    /// `benches/vm_invocations.rs`'s header argues why counting it counts
    /// frames exactly).
    ///
    /// The closer and not the opener, which the benchmark counts: a benchmark
    /// wants to know a frame *started*, and a test wants to read what it drew.
    /// Waiting on `?2026h` returns while the frame's own bytes are still in
    /// flight, and the assertion that follows then reads half a frame.
    const FRAME: &[u8] = b"\x1b[?2026l";

    /// The primary device attributes query terminal setup sends. Answering it
    /// ends keyboard-protocol negotiation immediately rather than after
    /// `crossterm`'s timeout.
    const DA1_QUERY: &[u8] = b"\x1b[c";

    /// A plain VT100 attributes report — "no kitty protocol".
    const DA1_REPLY: &[u8] = b"\x1b[?6c";

    /// `ZQ` — quit, force. `runtime/keymaps.scm` binds it.
    const QUIT: &[u8] = b"ZQ";

    // -----------------------------------------------------------------------
    // The harness
    // -----------------------------------------------------------------------

    /// One child process on a pty, and everything it has drawn.
    struct Editor {
        master: Arc<File>,
        child: std::process::Child,
        transcript: Arc<Mutex<Vec<u8>>>,
        frames: Arc<AtomicU64>,
        /// Frames this harness has asked for and waited on. It tracks `frames`
        /// exactly, and the gap between them is the bug [`Editor::press`]
        /// describes — see its doc comment for why an unaccounted frame is a
        /// failure and not a curiosity.
        accounted: AtomicU64,
        reader: Option<std::thread::JoinHandle<()>>,
    }

    impl std::fmt::Debug for Editor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Editor")
                .field("frames", &self.frames.load(Ordering::Relaxed))
                .finish_non_exhaustive()
        }
    }

    impl Editor {
        /// Starts the shipping binary on `file`, with `state` as its
        /// `$XDG_STATE_HOME`, and waits for the first frame.
        ///
        /// `$XDG_CONFIG_HOME` is set too, and `T101` is why it has to be: the
        /// persisted layer is read from and written to the config home now, so
        /// a child that inherited the developer's would read *their*
        /// `~/.config/phosphor` — and the first test that persisted anything
        /// would write into it. [`config_home`] derives one nothing else
        /// shares from the state home every caller already passes.
        fn open(file: &Path, state: &Path, runtime: &Path) -> Self {
            Self::started(Some(file), state, runtime, &[])
        }

        /// The same editor with **no file argument at all** — `T107`.
        ///
        /// Not a variant of [`Editor::open`] with an empty path: the whole
        /// point is the argv, and a `phosphor ""` would be a file named the
        /// empty string. Until `T107` this command line did not reach the loop
        /// — clap answered *"the following required arguments were not
        /// provided: <FILE>"* and exited `2` — so a test that spawns it and
        /// waits for a frame is the reachability proof, in the sense
        /// `loop_pty`'s own header means it.
        fn bare(state: &Path, runtime: &Path) -> Self {
            Self::started(None, state, runtime, &[])
        }

        /// `phosphor --float informational` — no file either, and a surface in
        /// front of it.
        ///
        /// The **negative** half of `T107`'s notice: the same buffer with no
        /// name, guarded out of saying so because the float explains itself.
        ///
        /// `--float` and not `--repl`, and the difference was measured rather
        /// than assumed. The REPL is drawn over the statusline's row, so it
        /// swallows the notice whether or not the guard is there and a test
        /// written against it passes on a broken build — observed, with the
        /// guard planted out. `T084`'s fixture float is centred and leaves that
        /// row visible: with the guard dropped the last row of the frame reads
        /// `no file — :write <path> creates one`, which is what makes this the
        /// surface that can fail.
        fn floated(state: &Path, runtime: &Path) -> Self {
            Self::started(None, state, runtime, &["--float", "informational"])
        }

        fn started(file: Option<&Path>, state: &Path, runtime: &Path, flags: &[&str]) -> Self {
            Self::started_with(file, state, runtime, flags, &[])
        }

        /// **The editor on a terminal with no colour — §8's degraded path.**
        ///
        /// `NO_COLOR=1` is the condition `phosphor_term::colour_available`
        /// answers on, matched deliberately to <https://no-color.org> and to
        /// `crossterm`'s own rule, so this is the same question the shipping
        /// binary asks rather than a switch invented for a test.
        ///
        /// Exists because `T088`'s collapse verification measured that nothing
        /// headless covered the binding: with the fill defeated in a form that
        /// kept `state_fill` referenced, **1387 tests passed**. The interpreter
        /// half has a unit test; this is the half that reaches it.
        fn degraded(file: &Path, state: &Path, runtime: &Path) -> Self {
            Self::started_with(Some(file), state, runtime, &[], &[("NO_COLOR", "1")])
        }

        /// **The editor standing in `cwd`, with VCS detection running.**
        ///
        /// `CP-8c`'s three passes are built on this: the fixture directory
        /// carries the marker — `.jj`, `.git`, or neither — and detection walks
        /// up from where the child stands.
        fn in_repo_at(file: &Path, state: &Path, runtime: &Path, cwd: &Path) -> Self {
            Self::started_in(
                Some(file),
                state,
                runtime,
                &[],
                &[("PHOSPHOR_VCS", "1")],
                Some(cwd),
            )
        }

        /// **The editor with `T071`'s VCS detection actually running.**
        ///
        /// Every other spawn turns it off — see the `PHOSPHOR_VCS` line in
        /// [`Editor::started_with`] — so the one test that is *about* the chip
        /// turns it back on. The repository it then finds is this checkout,
        /// because the child inherits the runner's working directory; that is
        /// incidental to the test and is why the test asserts the *backend*
        /// rather than any particular branch.
        fn in_a_repo(file: &Path, state: &Path, runtime: &Path) -> Self {
            Self::started_with(Some(file), state, runtime, &[], &[("PHOSPHOR_VCS", "1")])
        }

        /// **The editor with `T069`'s disk watcher actually running.**
        ///
        /// Every other spawn turns it off — see the `PHOSPHOR_WATCH` line in
        /// [`Editor::started_with`] for why — so the two tests that are *about*
        /// the watcher turn it back on here. `degraded` above is the same shape
        /// for the same reason: a producer the suite does not want by default,
        /// named where it is wanted.
        fn watching(file: &Path, state: &Path, runtime: &Path) -> Self {
            Self::started_with(Some(file), state, runtime, &[], &[("PHOSPHOR_WATCH", "1")])
        }

        fn started_with(
            file: Option<&Path>,
            state: &Path,
            runtime: &Path,
            flags: &[&str],
            extra: &[(&str, &str)],
        ) -> Self {
            Self::started_in(file, state, runtime, flags, extra, None)
        }

        /// [`Editor::started_with`], standing in `cwd`.
        ///
        /// **`CP-8c` is why this exists.** That checkpoint runs the whole `S7`
        /// set three times — in a jj repo, a git repo and a bare directory —
        /// and `T071`'s detection walks up from the *working directory*. Every
        /// other spawn lets the child inherit this runner's, which is inside
        /// the phosphor checkout, so without this a test cannot be *in* a
        /// repository of its own choosing.
        fn started_in(
            file: Option<&Path>,
            state: &Path,
            runtime: &Path,
            flags: &[&str],
            extra: &[(&str, &str)],
            cwd: Option<&Path>,
        ) -> Self {
            let binary = PathBuf::from(env!("CARGO_BIN_EXE_phosphor"));
            let (master, slave_path) = open_pty();
            let slave = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&slave_path)
                .expect("the pty slave opens");
            // Apple's master rejects `TIOCSWINSZ`; the slave takes it on both
            // platforms and is the fd the child asks anyway.
            tcsetwinsize(&slave, SCREEN).expect("the pty takes a window size");

            // **The `Command` is scoped, and that is load-bearing.** It owns
            // the three `Stdio` slots, which own the slave fds — so a `Command`
            // that outlives the spawn keeps this side of the pty open, and the
            // rule two comments down ("nothing on this side may hold a slave fd
            // open") quietly stops holding. The symptom is not a failure, it is
            // a **hang**: a child that exits without drawing leaves
            // [`Editor::await_frames`] to time out correctly and then
            // [`Editor::drop`] to block forever in `reader.join()`, because the
            // reader is waiting on an end-of-file that cannot arrive. Measured
            // while planting a mutation against `T107` — restoring the required
            // FILE argument turned four tests that should fail in 30 s into a
            // run that never ended.
            let child = {
                let mut command = Command::new(binary);
                if let Some(file) = file {
                    command.arg(file);
                }
                command.args(flags);
                if let Some(cwd) = cwd {
                    command.current_dir(cwd);
                }
                command
                    .env("PHOSPHOR_RUNTIME", runtime)
                    .env("XDG_STATE_HOME", state)
                    .env("XDG_CONFIG_HOME", config_home(state))
                    .env("TERM", "xterm-256color")
                    // **No VCS chip unless the test asks for one**, and the
                    // reason is the same shape as the watcher below.
                    //
                    // The child inherits *this runner's* working directory,
                    // which is inside the phosphor checkout — so with detection
                    // on, every pty test drew `git worktree-… ●` on its strip:
                    // twenty-four columns of a repository that has nothing to do
                    // with a fixture in `/tmp`. §11 sheds the **server** chip
                    // before the vcs one, so an LSP test lost the chip it was
                    // asserting about, on macOS CI where the path is longest.
                    //
                    // `Editor::in_a_repo` opts back in.
                    .env("PHOSPHOR_VCS", "0")
                    // **No disk watcher unless the test asks for one.**
                    // `press` asserts one frame per key byte and the editor
                    // emits a frame marker on every draw, so an asynchronous
                    // producer attached to every buffer breaks every test that
                    // counts. `Editor::watching` opts back in. Set before
                    // `extra` so that override wins.
                    .env("PHOSPHOR_WATCH", "0")
                    .envs(extra.iter().copied())
                    .stdin(Stdio::from(slave.try_clone().expect("the slave clones")))
                    .stdout(Stdio::from(slave.try_clone().expect("the slave clones")))
                    .stderr(Stdio::from(slave))
                    .spawn()
                    .expect("the shipping binary starts")
            };

            // Nothing on this side may hold a slave fd open, or the master
            // never sees end-of-file when the child exits.
            let transcript = Arc::new(Mutex::new(Vec::new()));
            let frames = Arc::new(AtomicU64::new(0));
            let reader = spawn_reader(
                Arc::clone(&master),
                Arc::clone(&transcript),
                Arc::clone(&frames),
            );

            let editor = Self {
                master,
                child,
                transcript,
                frames,
                accounted: AtomicU64::new(1),
                reader: Some(reader),
            };
            // Raw mode is on by the time the first frame lands, so nothing
            // written after this is echoed into what is being read.
            //
            // One frame, and `accounted` above says one: startup was measured
            // drawing exactly one and then settling — held at 1 after 1.5s
            // idle — so a second frame arriving here is the editor gaining a
            // startup redraw, which every press after it would silently
            // absorb. The first press is where that now surfaces.
            editor.await_frames(1);
            editor
        }

        /// Types `keys` and waits for the frames they produce.
        ///
        /// **One frame per key, and the wait is the assertion.** The loop
        /// draws, blocks on `event::read`, handles what arrived, and draws
        /// again — so a key that produced no frame is a key the loop never saw,
        /// and this times out rather than passing quietly.
        ///
        /// Every sequence these tests write is single-byte keys, so the count
        /// of bytes is the count of frames to wait for; a test that needs a
        /// multi-byte escape would have to say so.
        ///
        /// **That check was one-sided until `§20`, and the missing side is the
        /// one that fails quietly.** Too *few* frames times out, loudly. Too
        /// *many* — one key drawing two — was invisible, and its consequence
        /// lands on the *next* press: `target` is computed from a counter the
        /// surplus has already inflated, so that press returns without its keys
        /// having been handled at all, and the test goes on to assert against a
        /// buffer that never saw them. That is a mechanism for exactly the
        /// symptom `§20` records — a file whose contents are a plausible
        /// mis-sequencing rather than a timeout — and it is load-sensitive,
        /// because whether the surplus frame lands before or after `press`
        /// returns is a scheduling question.
        ///
        /// So the surplus is now accounted for. Each press records the count it
        /// waited for; the next one requires the counter to still be there.
        ///
        /// **Not asserted: that this is what `§20` saw.** It could not be
        /// reproduced — ~400 executions across 30 single-test runs under 20
        /// spinners on 10 cores, and 24 concurrent whole-binary runs, all green
        /// — and every key these tests press was measured drawing exactly one
        /// frame, including `:`, `w`, `\r`, `esc` and `SPC`. This closes the
        /// hole that would produce that symptom and makes the next occurrence
        /// name itself instead of being a mystery a second time.
        fn press(&self, keys: &[u8]) {
            let before = self.frames.load(Ordering::Relaxed);
            let accounted = self.accounted.load(Ordering::Relaxed);
            assert_eq!(
                before,
                accounted,
                "{} frame(s) arrived that no `press` asked for, before typing {:?}. \
                 One frame per key is this harness's whole synchronisation: a key that drew \
                 two means every press after it returns early, and the assertion at the end of \
                 the test reads a buffer that never saw its keys. Last frame: {}",
                before - accounted,
                printable(keys),
                self.tail()
            );

            let target = before + keys.len() as u64;
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            self.await_frames(target);
            self.accounted.store(target, Ordering::Relaxed);
        }

        /// Writes `keys` and waits until `wanted` has been drawn, or fails.
        ///
        /// **The counted-frames discipline cannot be used for anything a
        /// server answers**, and that is a fact about servers rather than a
        /// weakening of the harness: `press` asserts one frame per key because
        /// the terminal is the only producer of a keystroke's frame, and an
        /// LSP answer arrives on its own schedule from another thread. What is
        /// asserted here instead is stronger in the one way that matters — not
        /// *"a frame happened"* but *"this text reached the screen"* — and a
        /// wiring that never answers fails on the deadline rather than
        /// passing quietly.
        ///
        /// The accounting is resynchronised afterwards ([`Editor::settle`]),
        /// so an ordinary `press` may follow.
        fn press_until(&self, keys: &[u8], wanted: &str) -> String {
            let mark = self.mark();
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let drawn = self.since(mark);
                if shows(&drawn, wanted) {
                    // **Settle first, then read again.** The wanted text can
                    // appear in the first half of a frame whose second half is
                    // still on the wire — under load this returned a float
                    // whose documentation block had not been written yet, and
                    // the assertion after it failed on a frame that was about
                    // to be complete. The transcript is re-read afterwards so
                    // what comes back is whole.
                    self.settle();
                    return self.since(mark);
                }
                assert!(
                    Instant::now() < deadline,
                    "{wanted:?} was never drawn after typing {:?}. Drawn since: {drawn}",
                    printable(keys)
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        /// Presses `keys`, then polls the **composed grid** until `wanted` is
        /// on it, and hands back the grid.
        ///
        /// # Why this exists beside [`Editor::press_until`]
        ///
        /// They read two different things and the difference bit three tests
        /// the day this was written. `press_until` scans the *bytes drawn
        /// since* the keys — a delta — so it can only wait for text that is
        /// **newly** written. `J` joining `alpha` and `bravo` writes only
        /// ` bravo` onto a row that already said `alpha`, so `"alpha bravo"` is
        /// on the screen and never in the delta; waiting for it times out at
        /// thirty seconds while the editor sits there having done exactly the
        /// right thing.
        ///
        /// The delta is the right reader for a *notice*, which is written fresh
        /// every time. This is the right reader for **state**: a joined line, a
        /// cursor readout, a marker that was already partly there.
        fn shown_on_grid(&self, keys: &[u8], wanted: &str) -> Screen {
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let screen = self.screen();
                let grid = (0..SCREEN.ws_row)
                    .map(|row| screen.line(row))
                    .collect::<Vec<_>>()
                    .join("\n");
                if shows(&grid, wanted) {
                    self.settle();
                    return self.screen();
                }
                assert!(
                    Instant::now() < deadline,
                    "{wanted:?} never reached the screen after typing {:?}. Screen was:\n{grid}",
                    printable(keys)
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        /// Presses `keys` and waits for `want` on the **statusline**.
        ///
        /// [`Editor::shown_on_grid`] narrowed to the bottom row, and what it
        /// buys is the failure message rather than the wait. A cursor position
        /// that never arrives has two causes and they look identical from a
        /// timeout: the editor did not move, or §11 shed the segment to make
        /// the row fit. The second is not a bug and costs thirty seconds to
        /// find out, so this says which.
        ///
        /// Prefer this over `press_until` for anything positional — that scans
        /// the bytes drawn *since* the keys, and `1:1` becoming `2:1` repaints
        /// one cell.
        fn landed_at(&self, keys: &[u8], want: &str) -> Screen {
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let screen = self.screen();
                let row = statusline(&screen);
                if row.contains(want) {
                    self.settle();
                    return self.screen();
                }
                if Instant::now() >= deadline {
                    // A position segment is `line:column`; §11 drops it whole
                    // rather than squeezing it, so its absence is the tell.
                    let positional = row.split_whitespace().any(|word| {
                        word.contains(':') && word.ends_with(|c: char| c.is_ascii_digit())
                    });
                    let why = if positional {
                        "the statusline has a position and it is not this one — the editor did not move"
                    } else {
                        "the statusline has no position at all: §11 shed it to fit. \
                         The scratch path is the usual reason a 120-column row runs out of room"
                    };
                    panic!(
                        "{want:?} never reached the statusline after typing {:?} — {why}.\n\
                         The row was: {row}",
                        printable(keys)
                    );
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        /// Writes `keys` and waits for the editor to go quiet, asserting
        /// nothing about frames.
        ///
        /// For a key whose effect is **in the buffer** rather than on a frame
        /// this harness can read. [`printable`]'s own doc says why that is a
        /// real distinction: a frame is a diff, so accepting a completion over
        /// a word that shares a prefix redraws only the suffix — `ault_delay`
        /// reaches the transcript and `let base = default_delay` never does.
        /// The assertion for those belongs on the file, after `:w`.
        ///
        /// # This is not a wait for anything asynchronous
        ///
        /// It settles, and [`Editor::settle`] means *no new frame for 250 ms* —
        /// which an operation can satisfy **while still in progress**. Waiting
        /// for a language server is quiet. An `:e` draws the ex line, pauses,
        /// then swaps the buffer. In both cases this returns in the gap, the
        /// next line of the test runs against the old state, and the failure
        /// surfaces somewhere else entirely: a `gcgc` that commented the buffer
        /// you had already left looks exactly like `gc` being broken.
        ///
        /// Four of the five CI failures in the `S4` window were this, written by
        /// three different agents and by the root agent fixing them. Every one
        /// passed locally, because the race needs a machine slow enough to lose
        /// it and a developer's is not.
        ///
        /// **Wait on the thing you mean instead**, in rough order of preference:
        ///
        /// * [`Editor::press_until`] on text that exists *only* after the step
        ///   completed — the target file's contents for a cross-file jump, the
        ///   comment prefix for a `gc`. It is the strongest option because it is
        ///   also the assertion, and it fails with the frames it drew.
        /// * [`Screen::replayed`]'s `row` for a cursor move, when nothing new is
        ///   drawn to match on. `since` cannot see it — that runs the bytes
        ///   through `printable` and strips the escape that carries the answer.
        /// * Nothing on the statusline: it is cached, and its key does not
        ///   include the cursor, so a jump can leave `1:1` on screen. Matching a
        ///   position there hangs for the full timeout.
        fn press_quietly(&self, keys: &[u8]) {
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            self.settle();
        }

        /// Waits for the editor to stop drawing, then takes the frame count as
        /// accounted for.
        ///
        /// A server pushes — diagnostics arrive unasked and a lookup answers
        /// when it answers — so the frame a key produced and the frames an
        /// answer produced cannot be told apart by counting. This is the seam
        /// where a test stops counting and starts reading.
        /// Press `keys` and answer the screen once the editor has finished
        /// reacting to *them*.
        ///
        /// **[`Editor::shown_on_grid`] cannot do this and is not meant to.** It
        /// waits for a needle and then settles, which is exactly right when the
        /// needle is the thing the press produces — and wrong when the needle
        /// was already on screen, because then it matches on the frame *before*
        /// the press and settles against a terminal that has not been asked to
        /// do anything yet.
        ///
        /// `<C-w>v` is that case: two panes on one buffer draw the same text,
        /// so there is no string the split makes appear. Measured — the grid
        /// came back with one pane, and one harmless press later it had two.
        ///
        /// So this waits for a frame to be *drawn* first, then settles. It
        /// makes no claim about what the frame says, which is the caller's to
        /// assert.
        fn after(&self, keys: &[u8]) -> Screen {
            let before = self.frames.load(Ordering::Relaxed);
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            let deadline = Instant::now() + Duration::from_secs(30);
            while self.frames.load(Ordering::Relaxed) == before {
                assert!(
                    Instant::now() < deadline,
                    "the editor drew nothing after {keys:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            self.settle();
            self.screen()
        }

        fn settle(&self) {
            let mut quiet_since = Instant::now();
            let mut last = self.frames.load(Ordering::Relaxed);
            let deadline = Instant::now() + Duration::from_secs(30);
            while quiet_since.elapsed() < Duration::from_millis(250) {
                assert!(
                    Instant::now() < deadline,
                    "the editor never stopped drawing"
                );
                std::thread::sleep(Duration::from_millis(10));
                let now = self.frames.load(Ordering::Relaxed);
                if now != last {
                    last = now;
                    quiet_since = Instant::now();
                }
            }
            self.accounted.store(last, Ordering::Relaxed);
        }

        /// Everything drawn since `mark`, as printable text.
        fn since(&self, mark: usize) -> String {
            let transcript = self.transcript.lock().expect("the reader has not panicked");
            printable(&transcript[mark.min(transcript.len())..])
        }

        /// Every byte the child has written, escapes and all (`T056`).
        ///
        /// **The one reader in this file that does not go through
        /// [`printable`]**, and it exists because there is exactly one kind of
        /// claim a grid cannot carry: an escape sequence occupies no cell.
        /// `printable` strips CSI and OSC on purpose — a test that needled the
        /// raw stream for *text* would be reading a diff renderer's output as
        /// if it were a screen, which is the mistake `OPEN-QUESTIONS.md` §54
        /// records. Text goes through [`Editor::shown_on_grid`]. Sequences come
        /// through here, and nothing else should.
        fn raw(&self) -> String {
            let transcript = self.transcript.lock().expect("the reader has not panicked");
            String::from_utf8_lossy(&transcript).into_owned()
        }

        /// How many bytes have been drawn — the mark [`Editor::since`] takes.
        fn mark(&self) -> usize {
            self.transcript
                .lock()
                .expect("the reader has not panicked")
                .len()
        }

        /// Blocks until the child has drawn `target` frames.
        ///
        /// The timeout prints what *was* drawn, because the common way for
        /// this to fail is a key the loop swallowed — and the frame before it
        /// says which surface swallowed it.
        fn await_frames(&self, target: u64) {
            let deadline = Instant::now() + Duration::from_secs(30);
            while self.frames.load(Ordering::Relaxed) < target {
                assert!(
                    Instant::now() < deadline,
                    "the loop stopped drawing at {} frames, waiting for {target}. Last frame: {}",
                    self.frames.load(Ordering::Relaxed),
                    self.tail()
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        /// Everything drawn so far, replayed onto a grid — see [`Screen`].
        fn screen(&self) -> Screen {
            let transcript = self.transcript.lock().expect("the reader has not panicked");
            Screen::replayed(&transcript)
        }

        /// The tail of the transcript, for a failure message.
        fn tail(&self) -> String {
            let transcript = self.transcript.lock().expect("the reader has not panicked");
            let start = transcript.len().saturating_sub(4096);
            printable(&transcript[start..])
        }

        /// `ZQ`, and the exit status that proves every key before it landed.
        ///
        /// `<esc>` first, always: `Z` typed while a sequence is half-typed
        /// *extends* it — `SPC Z` is unbound, not a quit — so a test that ends
        /// with a popup open would hang on a child that never left.
        fn quit(mut self) {
            self.press(b"\x1b");
            (&*self.master)
                .write_all(QUIT)
                .expect("the child takes the quit");
            let status = self.child.wait().expect("the child exits");
            if let Some(reader) = self.reader.take() {
                reader.join().expect("the reader thread finishes");
            }
            assert!(status.success(), "the shipping binary exited with {status}");
        }

        /// Leave by a route the test chose, and prove the binary agreed.
        ///
        /// [`Editor::quit`] presses `ZQ` because that is the one exit every
        /// test can end with. A test *about* an exit needs to press its own —
        /// `:wq` writes first, `ZQ` throws work away, and the difference is the
        /// thing under test — so this takes the keys and does the rest.
        ///
        /// No `<esc>` first: the keys are the subject here, and prefixing them
        /// would hide a sequence that only works from a clean slate.
        fn leave_by(mut self, keys: &[u8]) {
            (&*self.master)
                .write_all(keys)
                .expect("the child takes the keys");
            let status = self.child.wait().expect("the child exits");
            if let Some(reader) = self.reader.take() {
                reader.join().expect("the reader thread finishes");
            }
            assert!(status.success(), "the shipping binary exited with {status}");
        }

        /// `SIGKILL` — no exit code runs, no destructor, no `fsync`.
        ///
        /// `CP-3` asks for this and `journal.rs` is designed against it: an
        /// append is a `write_all` and nothing more, so the bytes are the
        /// kernel's page cache the moment the call returns and survive the
        /// process dying. What a crash can cost is a torn record at the tail,
        /// which the next open truncates.
        fn kill(mut self) {
            self.child.kill().expect("the child takes a signal");
            let _ = self.child.wait();
            if let Some(reader) = self.reader.take() {
                reader.join().expect("the reader thread finishes");
            }
        }
    }

    impl Drop for Editor {
        fn drop(&mut self) {
            // A failed assertion leaves the child on the alternate screen with
            // its terminal in raw mode. Nothing else has that fd, so killing it
            // is what releases the reader thread.
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
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

    /// Drains the master until end-of-file, keeping everything and answering
    /// the device-attributes query the first time it appears.
    fn spawn_reader(
        master: Arc<File>,
        transcript: Arc<Mutex<Vec<u8>>>,
        frames: Arc<AtomicU64>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut answered = false;
            let mut carry: Vec<u8> = Vec::new();
            loop {
                // End-of-file reads zero on Apple platforms and fails with
                // `EIO` on Linux; both mean the child is gone.
                let Ok(read @ 1..) = (&*master).read(&mut buffer) else {
                    return;
                };
                let chunk = &buffer[..read];
                // Counted over the last few bytes of the previous read as well,
                // so a marker split across two reads is one frame and not none.
                carry.extend_from_slice(chunk);
                frames.fetch_add(count(&carry, FRAME), Ordering::Relaxed);
                if !answered && count(&carry, DA1_QUERY) > 0 {
                    answered = true;
                    let _ = (&*master).write_all(DA1_REPLY);
                }
                let keep = carry.len().saturating_sub(FRAME.len() - 1);
                carry.drain(..keep);
                transcript
                    .lock()
                    .expect("no other writer panics")
                    .extend_from_slice(chunk);
            }
        })
    }

    /// Whether `wanted` was drawn, tolerating cells the terminal did not
    /// redraw.
    ///
    /// **A frame is a diff.** ratatui emits only the cells whose character
    /// *and* style changed, and [`printable`] renders the cursor motion that
    /// skipped one as a space — so a sentence drawn over a row that happened to
    /// share a character loses that character. It bites exactly once, on the
    /// statusline: a notice replaces a row that is holding the file's path, and
    /// the path is a temp directory whose name is different every run. Every
    /// other assertion in this file reads a region that was blank before it.
    ///
    /// So a skipped cell may be a space, and two thirds of the characters have
    /// to be there exactly — which a run of spaces cannot satisfy.
    /// Whether `wanted` appears at least twice — what "two panes on one buffer"
    /// looks like from outside, since both draw the same text.
    ///
    /// Exact rather than [`shows`]'s two-thirds match: a fuzzy count would find
    /// a second copy in the noise of a wide frame, and the whole claim is that
    /// there are two.
    fn twice(frame: &str, wanted: &str) -> bool {
        frame.matches(wanted).count() >= 2
    }

    /// The same, one pane further.
    fn thrice(frame: &str, wanted: &str) -> bool {
        frame.matches(wanted).count() >= 3
    }

    /// A screen as one string, rows joined — [`Editor::shown_on_grid`] answers
    /// a `Screen` and the counting above is over the whole of it.
    fn grid_of(screen: &Screen) -> String {
        (0..SCREEN.ws_row)
            .map(|row| screen.line(row))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn shows(frame: &str, wanted: &str) -> bool {
        let frame: Vec<char> = frame.chars().collect();
        let wanted: Vec<char> = wanted.chars().collect();
        if frame.len() < wanted.len() {
            return false;
        }
        let least = wanted.len() * 2 / 3;
        frame.windows(wanted.len()).any(|window| {
            window
                .iter()
                .zip(&wanted)
                .all(|(drawn, want)| drawn == want || *drawn == ' ')
                && window
                    .iter()
                    .zip(&wanted)
                    .filter(|(drawn, want)| drawn == want)
                    .count()
                    >= least
        })
    }

    fn count(haystack: &[u8], needle: &[u8]) -> u64 {
        haystack
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count() as u64
    }

    /// The printable runs of a terminal byte stream, escapes removed.
    ///
    /// Not an emulator: it answers *"was this word drawn"* and nothing about
    /// where. `ESC [ … final` and `ESC ] … BEL/ST` are dropped whole; anything
    /// else printable is kept, with a space between runs so two cells that
    /// happen to abut across a style change do not read as one word.
    /// The statusline — the bottom row of a composed [`Screen`].
    ///
    /// **Read a position off the grid, never out of a delta.** A statusline
    /// going `1:1` → `2:1` repaints one cell, so the string `"2:1"` is on the
    /// screen and never in the bytes drawn since a keypress. Pair this with
    /// [`Editor::shown_on_grid`], which polls the grid, and not with
    /// [`Editor::press_until`], which scans the delta and will wait out its
    /// full thirty seconds for a line that is already there.
    ///
    /// This was a local closure named `at` in two tests before it was a
    /// function, and `OPEN-QUESTIONS.md` §37 is what it cost to have it in
    /// neither: a `press_until` on a cursor position hung, and the hang was
    /// recorded as a product defect in the statusline's cache.
    fn statusline(screen: &Screen) -> String {
        screen.line(SCREEN.ws_row - 1)
    }

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
                // CSI: parameters and intermediates, then one final byte.
                Some('[') => {
                    for parameter in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&parameter) {
                            break;
                        }
                    }
                }
                // OSC: runs to BEL or ST.
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

    /// The shipped editor layer, copied so a test may append to it.
    fn copy_layer(into: &Path) -> PathBuf {
        let from = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../runtime")
            .canonicalize()
            .expect("the shipped editor layer is where the workspace keeps it");
        let to = into.join("runtime");
        copy_scm_tree(&from, &to);
        to
    }

    /// Every `.scm` under `from`, into `to`, **preserving relative paths**.
    ///
    /// Flat was enough until `T037`. `runtime/` was one directory of files, so a
    /// `read_dir` taking `*.scm` staged the whole layer; `init.scm`'s
    /// `phosphor/boot-files` now names twelve `languages/*.scm`, and a flat copy
    /// stages an `init.scm` asking for files nobody put there. Seven tests here
    /// failed exactly that way, each reporting a boot fault rather than the
    /// missing copy — so the recursion is the fix and this comment is the reason
    /// the next subdirectory does not repeat it.
    fn copy_scm_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("a runtime directory");
        for entry in fs::read_dir(from).expect("the shipped layer") {
            let entry = entry.expect("a readable entry");
            let path = entry.path();
            if path.is_dir() {
                copy_scm_tree(&path, &to.join(entry.file_name()));
            } else if path.extension().is_some_and(|ext| ext == "scm") {
                fs::copy(&path, to.join(entry.file_name())).expect("copy");
            }
        }
    }

    /// A scratch directory that removes itself.
    #[derive(Debug)]
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        /// A scratch tree, under a **short** root — and the shortness is load
        /// bearing.
        ///
        /// `std::env::temp_dir()` is `/tmp` on CI and
        /// `/var/folders/wl/nflr33r52fd_yc7hdl9kw6340000gn/T` on macOS: 48
        /// characters before a name is added, against 4. The statusline is 120
        /// columns and §11 sheds by width, in this order — counters, server,
        /// vcs, **cursor**, session prose, mode word, then the path's
        /// directories. So one fixture drew
        /// `NORMAL  /tmp/…/sample.toy   toy-lsp ✓ │ 2:1` on a runner and
        /// ` N  sample.toy   toy-lsp ✓` here, from a 110-character path,
        /// with the cursor position dropped exactly as §11 says to drop it.
        ///
        /// **That is a flake whether or not anyone has hit it**: any assertion
        /// about statusline content passes on one machine and fails on the
        /// other, and the failure looks like a 30s hang rather than a width
        /// problem. `OPEN-QUESTIONS.md` §37 is what it cost the first time —
        /// recorded as the statusline *lying* about the cursor, which it never
        /// did.
        ///
        /// Canonicalised because `/tmp` is a symlink to `/private/tmp` on
        /// macOS and the editor compares absolute paths: a symlink on one side
        /// and `lsp::absolute` on the other is how `gd` into the file you are
        /// already in concludes it is a different file. Falls back to the
        /// platform temp dir if `/tmp` is not there, because a long path only
        /// costs legibility while a missing one costs the test.
        fn new(name: &str) -> Self {
            let root = fs::canonicalize("/tmp").unwrap_or_else(|_| std::env::temp_dir());
            let path = root.join(format!(
                "ph-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch directory");
            Self { path }
        }

        fn state(&self) -> PathBuf {
            let state = self.path.join("state");
            fs::create_dir_all(&state).expect("a state home");
            state
        }

        /// Where this session's persisted layer lives — `T101`.
        ///
        /// The directory the child will read `persisted.scm` out of, made so a
        /// test can write one *before* the editor starts. Derived through the
        /// product's own [`config_dir_in`] rather than by joining `phosphor`
        /// here, so a test cannot be seeding a directory the binary does not
        /// look in.
        fn persisted(&self) -> PathBuf {
            let dir = config_dir_in(&config_home(&self.state()));
            fs::create_dir_all(&dir).expect("a config home");
            dir
        }
    }

    /// `$XDG_CONFIG_HOME` for a session, derived from its state home.
    ///
    /// One definition, because two would be a test seeding a directory the
    /// child does not read — which is the whole failure mode `T101` was
    /// reported for, one level up. Every caller's state home is
    /// `<scratch>/state`, so its sibling is a config home nothing else shares.
    fn config_home(state: &Path) -> PathBuf {
        state.with_file_name("config")
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // -----------------------------------------------------------------------
    // `R17` — the `SPC` leader popup
    // -----------------------------------------------------------------------

    /// **`CP-3`'s question, made answerable.** *"Is the `SPC` namespace
    /// learnable?"* was unanswerable before this: `SPC` produced a frame that
    /// differed from the one before it at zero pixels.
    #[test]
    fn pressing_space_opens_the_leader_popup() {
        let scratch = Scratch::new("leader");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b" ");
        let frame = editor.since(before);

        assert!(
            frame.contains("SPC"),
            "the leader grid draws its own `SPC ·` title (3c); frame was: {frame}"
        );
        // A group from the shipped table, drawn as `3c` draws it. Not a fixture
        // — `keymap-entries` answered it out of `runtime/keymaps.scm` on this
        // frame.
        assert!(
            frame.contains("+claude"),
            "SPC c is a group in the shipped keymap; frame was: {frame}"
        );

        // And it closes: the machine clears the pending sequence on any
        // resolution that is not `Pending`, so the popup is gone on the next
        // key rather than needing to be dismissed.
        let after = editor.mark();
        editor.press(b"\x1b");
        assert!(
            !editor.since(after).contains("+claude"),
            "the popup goes when the sequence does"
        );
        editor.quit();
    }

    /// **§34, driven: a cold config home with one form in it must not cost the
    /// keyboard.**
    ///
    /// This is the exact state the review found unusable. A config home holding
    /// a single `(set-option! "soft-wrap" #t)` *became* the runtime tree —
    /// `Runtime::root` was a first-match-wins search with the config home
    /// second — so the shipped fifteen files never loaded: an empty statusline,
    /// `:` drawing `┊ unknown key : — SPC opens the keymap`, `ZQ` doing
    /// nothing, and the process had to be killed. With **no boot float and no
    /// fault**, because that one form ran cleanly.
    ///
    /// So the assertions are the three keys that were dead, plus the option
    /// that was the whole reason the file existed. `editor.quit()` is the
    /// fourth: it presses `ZQ` and asserts the child exited, which is the
    /// difference between this test and the reproduction — that one ended in
    /// `kill`.
    ///
    /// A pty test rather than a unit test for this file's own reason: what was
    /// broken was not a value, it was *pressing a key*, and the unit tests
    /// beside `vm` in `main.rs` all passed while it was broken.
    #[test]
    fn a_user_init_scm_costs_nothing_from_the_shipped_keymap() {
        let scratch = Scratch::new("user-layer");
        let runtime = copy_layer(&scratch.path);
        // Written before the child starts, into the directory the product's own
        // `config_dir_in` names — the same seeding `T101`'s persisted tests do,
        // and the reason `Scratch::persisted` exists.
        fs::write(
            scratch.persisted().join("init.scm"),
            "(set-option! \"soft-wrap\" #t)\n",
        )
        .expect("the file a user writes first");

        // One line far wider than the 120-column screen. `TAILMARK` sits past
        // column 114 — the body's width once the six-cell gutter is taken — so
        // it is on the frame only if the line was wrapped, which is only true
        // if the user's one form was read.
        // `.txt` and not `.rs`: a rust file starts a server, whose first state
        // change draws a frame this harness did not press a key for, and one
        // frame per key is its whole synchronisation ([`Editor::press`]).
        let file = scratch.path.join("sample.txt");
        let long = format!("padding {} TAILMARK\n", "x".repeat(140));
        fs::write(&file, long).expect("a fixture with one very wide line");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let opened = editor.since(0);
        assert!(
            opened.contains("TAILMARK"),
            "the user's `(set-option! \"soft-wrap\" #t)` never ran, or ran \
             instead of the shipped layer; frame was: {opened}"
        );

        // `SPC` — the leader popup, composed out of the shipped keymap.
        let before = editor.mark();
        editor.press(b" ");
        let frame = editor.since(before);
        assert!(
            frame.contains("+claude"),
            "the shipped keymap is gone: SPC drew no leader; frame was: {frame}"
        );
        editor.press(b"\x1b");

        // `:` — the ex line. The reproduction's own symptom is the negative
        // assertion here, spelled the way the frame spelled it.
        let before = editor.mark();
        editor.press(b":");
        let frame = editor.since(before);
        assert!(
            !frame.contains("unknown key"),
            "`:` is unbound, which is what §34 saw on a real terminal; frame \
             was: {frame}"
        );
        editor.press(b"\x1b");

        // `ZQ`, and the assertion is that this returns at all.
        editor.quit();
    }

    /// **§34's disclosure half, on the shipping path: a boot that loaded
    /// nothing says so on the first frame.**
    ///
    /// `Layer::note_if_no_layer` is called from `run` and nowhere else, so a
    /// unit test over the method proves the sentence and not the call — and the
    /// state it describes is precisely one nobody would notice was unwired,
    /// because its symptom is *silence*.
    ///
    /// `kill` rather than `quit`, and that is the finding rather than a
    /// convenience: `ZQ` is `runtime/keymaps.scm`'s, the seed table is empty by
    /// construction (`no_bindings_in_rust.rs`), so an editor with no layer
    /// still cannot be quit. What changed is that it now says why.
    #[test]
    fn a_boot_that_found_no_layer_says_so_on_the_first_frame() {
        let scratch = Scratch::new("no-layer");
        // An empty `$PHOSPHOR_RUNTIME` — taken at its word, which is what makes
        // it a way to boot with nothing loaded — and a config home with no
        // `init.scm` in it either.
        let runtime = scratch.path.join("empty-runtime");
        fs::create_dir_all(&runtime).expect("an empty runtime tree");
        let _ = scratch.persisted();
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let frame = editor.since(0);

        assert!(
            shows(&frame, "no editor layer"),
            "an editor with no keymaps drew no fault at all, which is the \
             mystery §34 asked to end; frame was: {frame}"
        );
        assert!(
            shows(&frame, "nothing loaded"),
            "the float names no way out; frame was: {frame}"
        );
        editor.kill();
    }

    /// **The same state, with the file the float told you to write already
    /// written** — §34's own population, and the one a review found still
    /// silent.
    ///
    /// A config home holding one `(set-option! "soft-wrap" #t)` and nothing
    /// shipped underneath it. The form runs, so the boot report has a unit in
    /// it, and the disclosure was guarded on the report being empty — so this
    /// editor applied soft-wrap, drew no statusline, answered `SPC` with
    /// `unknown key <space>`, ignored `ZQ`, and said nothing about any of it.
    /// Measured on a real pty before this test existed; the process had to be
    /// killed.
    ///
    /// Two assertions, and the second is the one the unit test cannot make: the
    /// float has to say something *other* than `write <path>`, because the path
    /// it would name is the file already on disk.
    #[test]
    fn writing_the_file_the_float_asks_for_does_not_buy_silence() {
        let scratch = Scratch::new("wrote-it");
        let runtime = scratch.path.join("empty-runtime");
        fs::create_dir_all(&runtime).expect("an empty runtime tree");
        fs::write(
            scratch.persisted().join("init.scm"),
            "(set-option! \"soft-wrap\" #t)\n",
        )
        .expect("§34's own one-line file");
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let frame = editor.since(0);

        assert!(
            shows(&frame, "no editor layer"),
            "the editor §34 measured — no keymaps, no statusline, no way out — \
             still said nothing; frame was: {frame}"
        );
        assert!(
            shows(&frame, "your init.scm ran over nothing"),
            "the float repeated `write <path>` at somebody who had written it; \
             frame was: {frame}"
        );
        editor.kill();
    }

    /// `T034`'s liveness claim, through the loop: the popup reads the live
    /// table, so a binding written at the REPL is in the next popup with no
    /// wiring of its own.
    ///
    /// Typed at the real `:repl` prompt (`6b`) rather than appended to a file,
    /// because the claim is about a rebind *at runtime* — the copy of the layer
    /// this test runs against never mentions `zebra`.
    #[test]
    fn a_repl_rebind_reaches_the_leader_popup() {
        let scratch = Scratch::new("rebind");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b":repl\r");
        editor.press(b"(keymap-set! \"SPC z\" (key/group) \"zebra\" \"normal\")\r");
        editor.press(b"\x1b");

        let before = editor.mark();
        editor.press(b" ");
        let frame = editor.since(before);
        assert!(
            frame.contains("zebra"),
            "a rebind at the REPL is in the very next popup; frame was: {frame}"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `T101` — explicit persist, and a real config home
    // -----------------------------------------------------------------------

    /// **The whole of `T101`'s first half, in one session.** Ruled by Teej on
    /// 2026-08-14 and it overrides `6b`, which draws a bare `(keymap-set! …)`
    /// answering `⇒ #ok · persisted to init.scm`.
    ///
    /// Three forms, one prompt: the bare rebind is *offered*, the marked one is
    /// *kept*, and `7a`'s rule — a direct call on the capability, which is what
    /// `[2] always allow git push` will be — is written as given. The fourth
    /// assertion is the one `CP-4` earned: nothing lands in the runtime tree,
    /// which in a checkout is the repository.
    #[test]
    fn the_repl_keeps_what_the_verb_marks_and_offers_the_rest() {
        let scratch = Scratch::new("explicit-persist");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");
        let persisted = scratch.persisted().join("persisted.scm");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // Evaluating is evaluating.
        let bare = editor.mark();
        editor.press_until(
            b"(keymap-set! \"gz\" (lambda () (open-repl!)))\r",
            "not persisted",
        );
        assert!(
            !editor.since(bare).contains("persisted to"),
            "a bare config verb is session-only; frames were: {}",
            editor.since(bare)
        );
        assert!(
            !persisted.exists(),
            "and it wrote no file at all, not even an empty one"
        );

        // The verb is the explicit act.
        editor.press_until(
            b"(persist! (keymap-set! \"gy\" (lambda () (open-repl!))))\r",
            "persisted to persisted.scm",
        );

        // `7a`: *"writes (allow \"git push\") to init.scm"*. The capability
        // called directly, which is the call the permission surface will make
        // (`T061`) — no verb, no gate, because pressing a digit was the act.
        editor.press_until(
            b"(persist-form! \"(allow \\\"git push\\\")\")\r",
            "persisted to persisted.scm",
        );
        editor.quit();

        let written = fs::read_to_string(&persisted).expect("the config home holds the layer");
        assert!(
            written.contains(r#"(persist! (keymap-set! "gy""#),
            "the marked form was kept whole: {written:?}"
        );
        assert!(
            written.contains(r#"(allow "git push")"#),
            "7a's rule is written as given, not wrapped: {written:?}"
        );
        assert!(
            !written.contains("\"gz\""),
            "the offered form never reached the file: {written:?}"
        );
        assert!(
            !runtime.join("persisted.scm").exists(),
            "T101: nothing is written into the tree that booted — in a checkout that is the repo"
        );
    }

    /// **And it comes back.** A second process over the same config home, and
    /// the binding is in force on the next key.
    ///
    /// The load order is what this really tests. `init.scm` runs to its last
    /// form *before* Rust reads the load order it declared, so a persisted
    /// `(keymap-set! …)` that loaded any earlier than last would come back as a
    /// free-identifier fault — `keymaps.scm` has not run yet. `T101` took the
    /// file out of `phosphor/boot-files` entirely, so "last" is now
    /// `Layer::load_persisted`'s call site rather than a list position.
    #[test]
    fn a_form_kept_at_the_repl_survives_a_restart_of_the_binary() {
        let scratch = Scratch::new("persist-restart");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let first = Editor::open(&file, &scratch.state(), &runtime);
        first.press_until(b":repl\r", "steel");
        first.press_until(
            b"(persist! (keymap-set! \"gz\" (lambda () (open-repl!))))\r",
            "persisted to persisted.scm",
        );
        first.quit();

        // A fresh process. It has never seen the form; it reads it.
        let second = Editor::open(&file, &scratch.state(), &runtime);
        let before = second.mark();
        second.press(b"gz");
        let frame = second.since(before);
        assert!(
            frame.contains("steel"),
            "the persisted rebind opened the REPL on a fresh process; frame was: {frame}"
        );
        second.quit();
    }

    // -----------------------------------------------------------------------
    // `R18` — the unknown-key hint
    // -----------------------------------------------------------------------

    /// `8e`, and the promise the row itself makes: *shown once*.
    #[test]
    fn an_unbound_key_teaches_once_and_never_again() {
        let scratch = Scratch::new("unknown");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // `Q` is nobody's in the shipped table — the machine turns an unbound
        // key into `App::ShowUnknownKeyHint`.
        let before = editor.mark();
        editor.press(b"Q");
        let taught = editor.since(before);
        assert!(
            taught.contains("unknown key"),
            "the first unbound key teaches (8e); frame was: {taught}"
        );
        assert!(
            taught.contains("shown once"),
            "the row says what it promises; frame was: {taught}"
        );

        // A second unknown key — a different one — draws nothing. That is the
        // whole of `T035`, and it is the half a snapshot of one frame cannot
        // see.
        let after = editor.mark();
        editor.press(b"Q");
        editor.press(b"\x1b");
        assert!(
            !editor.since(after).contains("unknown key"),
            "the session has one hint"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `R19` — folds
    // -----------------------------------------------------------------------

    /// `za` at the cursor, through the loop: the fold closes and the lines
    /// under it leave the frame.
    ///
    /// The ranges are the language's own — `langs/rust/folds.scm`, read by the
    /// fork's `fold_query` — so this is also the first test that the shipped
    /// grammar produces any.
    #[test]
    fn za_closes_the_fold_the_cursor_is_in() {
        let scratch = Scratch::new("folds");
        let runtime = copy_layer(&scratch.path);
        // **This test is about folds, not about rust-analyzer.** A `.rs` file
        // starts whatever `runtime/languages/rust.scm` declares, and on a
        // machine that has that server it changes state on its own schedule,
        // wakes the loop (`events::AppEvent::Woke`) and draws a frame no key
        // asked for — which `press` is entitled to call a bug, and which makes
        // the test behave differently depending on what is installed. So the
        // copied layer redeclares `rust` with no server: turning one off is
        // what `define-language!` is for, and this is the first caller of it.
        fs::write(
            scratch.persisted().join("persisted.scm"),
            "(define-language! \"rust\"\n  (hash \"extensions\" '(\"rs\")\n        \
             \"grammar\" \"rust\"\n        \"lsp_command\" (list)\n        \
             \"comment_prefix\" \"//\"))\n",
        )
        .expect("the config home takes a declaration");
        let file = scratch.path.join("folded.rs");
        fs::write(
            &file,
            "fn outer() {\n    let marker_inside_the_fold = 1;\n    let another = 2;\n}\n",
        )
        .expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b"za");
        let folded = editor.since(before);
        assert!(
            !folded.contains("marker_inside_the_fold"),
            "za hides the fold's body; frame was: {folded}"
        );

        let after = editor.mark();
        editor.press(b"zR");
        assert!(
            editor.since(after).contains("marker_inside_the_fold"),
            "zR opens everything again"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `R2` — undo, across a restart
    // -----------------------------------------------------------------------

    /// **`CP-3`'s survival criterion.** Edit, save, quit, reopen, `u` — and the
    /// file on disk is what it was.
    ///
    /// Two child processes, one journal. The first has no history to restore
    /// and writes one; the second restores it before its first frame and undoes
    /// into it. Nothing in this test knows the journal exists: it is
    /// `$XDG_STATE_HOME` and the file's own path, which is Q1's keying.
    #[test]
    fn undo_survives_quitting_and_reopening() {
        let scratch = Scratch::new("undo-restart");
        let runtime = copy_layer(&scratch.path);
        let state = scratch.state();
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        // Session one: insert, close the group with `<esc>`, save, leave.
        let first = Editor::open(&file, &state, &runtime);
        first.press(b"i");
        first.press(b"ZED");
        first.press(b"\x1b");
        first.press(b":w\r");
        first.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "ZEDone\ntwo\n",
            "the edit reached disk"
        );

        // Session two: a fresh process, a fresh buffer, and a history it did
        // not create.
        let second = Editor::open(&file, &state, &runtime);
        second.press(b"u");
        second.press(b":w\r");
        second.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "one\ntwo\n",
            "undo in the second session walked the first session's history"
        );
    }

    /// **`CP-3`'s other survival criterion: `kill -9`.**
    ///
    /// The `T030` acceptance test proves this for the log in isolation. This
    /// proves it for the *editor*: no exit code runs, no destructor, no
    /// `fsync` — the journal is whatever `write_all` left in the kernel's page
    /// cache — and the next session undoes into it anyway.
    #[test]
    fn undo_survives_a_kill_9() {
        let scratch = Scratch::new("undo-sigkill");
        let runtime = copy_layer(&scratch.path);
        let state = scratch.state();
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "before\n").expect("a fixture");

        let killed = Editor::open(&file, &state, &runtime);
        killed.press(b"i");
        killed.press(b"X");
        killed.press(b"\x1b");
        killed.press(b":w\r");
        killed.kill();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "Xbefore\n"
        );

        let after = Editor::open(&file, &state, &runtime);
        after.press(b"u");
        after.press(b":w\r");
        after.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "before\n",
            "the history a killed process wrote is still a history"
        );
    }

    /// The tree, not the fork's stack: undo, diverge, and the undone branch is
    /// still reachable.
    ///
    /// `vendor/ratatui-code-editor/src/history.rs:19-22` truncates on
    /// divergence — under the fork, typing after an undo destroys what was
    /// undone. `T029`'s tree keeps it, and `<C-r>` after a divergent edit
    /// therefore redoes *the branch just taken* rather than the abandoned one.
    #[test]
    fn undo_and_redo_walk_the_tree_through_the_loop() {
        let scratch = Scratch::new("undo-tree");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "base\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"i");
        editor.press(b"A");
        editor.press(b"\x1b");
        editor.press(b"i");
        editor.press(b"B");
        editor.press(b"\x1b");

        // Two groups, two undos, back to the start.
        editor.press(b"u");
        editor.press(b"u");
        editor.press(b":w\r");
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "base\n",
            "two `<esc>`-closed groups are two undo steps"
        );

        // And forward again. One `<C-r>` per group, which is the same boundary.
        editor.press(b"\x12");
        editor.press(b"\x12");
        editor.press(b":w\r");
        // `BA`, not `AB`: `<esc>` leaves the cursor **on** the last character
        // typed rather than after it, so the second `i` inserts in front of the
        // first `A`. That is vim, and it is what the machine already does — the
        // point of the assertion is the two steps, not the spelling.
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "BAbase\n",
            "redo walks the branch it left"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `R10` — the legacy chord fallback
    // -----------------------------------------------------------------------

    /// `T027`'s degradation, reachable at last.
    ///
    /// `phosphor-core`'s `legacy_chord` retries an unbound `<C-k>` as
    /// `<C-S-k>` — but **only** under `key::Protocol::Legacy`, and nothing ever
    /// told the machine which protocol was negotiated, so on every terminal it
    /// was dead code. The observable difference is which of the two spellings a
    /// chord resolves as, and `$PHOSPHOR_KEYBOARD` forces either side of it
    /// without different hardware.
    ///
    /// Driven through the popup, because that is a surface that says out loud
    /// what sequence the machine is holding: a rebind puts a group under
    /// `<C-S-k>`, and under the legacy protocol a plain `<C-k>` reaches it.
    #[test]
    fn the_legacy_chord_fallback_is_reachable_on_a_legacy_terminal() {
        let scratch = Scratch::new("legacy");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        for (forced, reaches) in [("legacy", true), ("kitty", false)] {
            let editor = Editor::open_forced(&file, &scratch.state(), &runtime, forced);
            editor.press(b":repl\r");
            editor.press(b"(keymap-set! \"<C-S-k>\" (key/group) \"chorded\" \"normal\")\r");
            editor.press(b"(keymap-set! \"<C-S-k> a\" (key/group) \"chord-leaf\" \"normal\")\r");
            editor.press(b"\x1b");

            let before = editor.mark();
            // `<C-k>` — the only spelling a legacy terminal can send.
            editor.press(b"\x0b");
            let frame = editor.since(before);
            assert_eq!(
                frame.contains("chord-leaf"),
                reaches,
                "under PHOSPHOR_KEYBOARD={forced}, <C-k> reaching <C-S-k> should be {reaches}; \
                 frame was: {frame}"
            );
            editor.press(b"\x1b");
            editor.quit();
        }
    }

    // -----------------------------------------------------------------------
    // `T097` — `:help`
    // -----------------------------------------------------------------------

    /// **The one surface the `CP-3` repair pass missed.** `open-help` was
    /// declared, `:h[elp]` was bound, and `main.rs` had no arm — so `6d`
    /// existed as a snapshot and as nothing else. The snapshot
    /// (`crates/phosphor/tests/screen_6d.rs`) hand-builds its tree and says so;
    /// this presses the keys.
    #[test]
    fn help_opens_the_grid_and_closes_on_q() {
        let scratch = Scratch::new("help");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b":help\r");
        let frame = editor.since(before);

        assert!(
            frame.contains(":help"),
            "the float's header is the command that opened it; frame was: {frame}"
        );
        // `open-help`'s own wording: *"absent opens the index"*. Every row is a
        // topic that binds something, counted off the live table on this frame.
        assert!(
            frame.contains("agent-objects"),
            "the index names its topics; frame was: {frame}"
        );
        assert!(
            frame.contains("4 bound"),
            "and counts each one off the live table — the four agent nouns; \
             frame was: {frame}"
        );
        // `6d` draws `q close` in the footer, and it is honest on a grid.
        let after = editor.mark();
        editor.press(b"q");
        assert!(
            !editor.since(after).contains("agent-objects"),
            "q closes the help float, as its own footer promises"
        );
        editor.quit();
    }

    /// `:help <topic>` narrows the same grid — `6d`'s own topic.
    ///
    /// `agent-objects` is not a page of prose kept somewhere: it is the four
    /// `TextObject`s the vocabulary calls agent-native, found in the live
    /// table by their role. So the rows are the shipped verbs and the topic
    /// cannot list a key nothing binds.
    #[test]
    fn help_narrows_to_the_agent_objects_topic() {
        let scratch = Scratch::new("help-topic");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b":help agent-objects\r");
        let frame = editor.since(before);

        for noun in ["unseen region", "hunk", "thread", "review block"] {
            assert!(
                frame.contains(noun),
                "6d's four nouns are the topic; {noun} was missing from: {frame}"
            );
        }
        assert!(
            !frame.contains("toggle the fold here"),
            "a topic narrows — the fold keys are not agent objects; frame was: {frame}"
        );
        editor.press(b"\x1b");
        editor.quit();
    }

    /// `T086`'s liveness claim, through the loop: the grid is composed from
    /// `keymap-entries` when `:help` opens it, so a rebind typed at the REPL is
    /// in the next page with nothing wired for it.
    ///
    /// The copy of the layer this test runs against never mentions `zebra`.
    #[test]
    fn a_repl_rebind_shows_up_in_the_help_grid() {
        let scratch = Scratch::new("help-rebind");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b":repl\r");
        editor.press(b"(keymap-set! \"SPC z\" (key/group) \"zebra\" \"normal\")\r");
        editor.press(b"\x1b");

        let before = editor.mark();
        editor.press(b":help zebra\r");
        let frame = editor.since(before);
        assert!(
            frame.contains("zebra"),
            "a rebind at the REPL is in the very next :help; frame was: {frame}"
        );
        editor.press(b"\x1b");
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `T098` — the deliberately deferred keys
    // -----------------------------------------------------------------------

    /// A key bound to a capability the binary does not apply yet **says which
    /// task builds it**, instead of doing nothing.
    ///
    /// The refusal was always produced — `Editing::act` answers
    /// `NotYetImplemented` off each row's own task id — and `Session::key`
    /// dropped it on the floor, which is why `runtime/keymaps.scm`'s own claim
    /// that pressing a leader leaf *"answers `not built yet — T058 builds it`
    /// rather than nothing at all"* was not true of the running binary.
    #[test]
    fn a_deferred_key_names_the_task_that_builds_it() {
        let scratch = Scratch::new("deferred");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // `/` is vim's search. It is bound to the search prompt, and `T058`
        // built the *ex* and *claude* kinds without it: a search prompt needs
        // somewhere to search, which is the search machinery rather than the
        // line.
        //
        // **It named `T058` until 2026-08-24, and `T058` is ticked** — so the
        // refusal sent a reader to a finished task, which is the whole defect
        // `scripts/lint-refusal-tasks.sh` was written for. `T110` is the
        // machinery, added the same day because nothing in the graph owned it.
        // **Read off the grid, not the delta.** `press`'s byte delta is what
        // ratatui's *diff* renderer emitted, and a settled row arrives in pieces
        // separated by cursor moves — CI caught this one with the frame reading
        // `search is T058's oth  half`, the `er` on the far side of an escape.
        // That is `OPEN-QUESTIONS.md` §54's lesson arriving in a test that
        // predates it.
        editor.press_quietly(b"/");
        let frame = shown_on_grid_text(&editor, "the matcher");
        assert!(
            shows(&frame, "search needs the matcher — T110 builds it"),
            "a deferred key names its task on the statusline; frame was: {frame}"
        );

        // `n` walks the search matches, and walking a sequence is
        // `goto-sequence`.
        //
        // **The task it names moved twice, and the second move is the
        // interesting one.** It went from `T049` to `T058` when
        // `goto-sequence` learned to answer per *sequence* rather than per
        // verb — the refusal getting more precise. It went from `T058` to
        // `T110` on 2026-08-24 for the opposite reason: `T058` had been ticked
        // for three phases, so *"not built yet — T058 builds it"* was pointing
        // a reader at work that shipped. Precision is worth nothing if the
        // address is stale, which is why `scripts/lint-refusal-tasks.sh` now
        // fails when any of these names a ticked task.
        editor.press_quietly(b"n");
        let frame = shown_on_grid_text(&editor, "T110 builds it");
        assert!(
            shows(&frame, "not built yet — T110 builds it"),
            "the task comes off the *sequence*, not the verb; frame was: {frame}"
        );
        editor.quit();
    }

    /// The half `T035`'s ruling is about: `q` is *known and not built*, so it
    /// does not spend the session's one teaching row — and a key that is
    /// genuinely nobody's still does.
    ///
    /// Both in one session on purpose. The latch is per session, so a test that
    /// pressed only `q` could not tell "the hint was not spent" from "the hint
    /// was already gone".
    #[test]
    fn a_deferred_key_does_not_spend_the_session_hint() {
        let scratch = Scratch::new("deferred-hint");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // `q` — vim's macro record, deferred on purpose and bound to say so.
        let before = editor.mark();
        editor.press(b"q");
        assert!(
            !editor.since(before).contains("unknown key"),
            "q is bound, so it is not an unknown key; frame was: {}",
            editor.since(before)
        );

        // `Q` is nobody's in the shipped table, and the hint is still there to
        // be spent on it.
        let after = editor.mark();
        editor.press(b"Q");
        let taught = editor.since(after);
        assert!(
            taught.contains("unknown key"),
            "the session's one hint survives a deferred key; frame was: {taught}"
        );
        assert!(
            taught.contains("shown once"),
            "and it is still 8e's row; frame was: {taught}"
        );
        editor.quit();
    }

    // **`the_macro_key_declines_by_naming_its_task` used to live here**, and its
    // own doc predicted this: *"this sentence goes stale the day `T099` is
    // ticked and the arm is not replaced, which is the point of asserting the
    // task id."* `T099` is ticked and the arm *was* replaced, so `q` no longer
    // declines by naming a task — it records. What took its place is
    // `q_records_a_macro_and_at_plays_it_back`, a round trip rather than a
    // refusal, which is the same shape the mark half took when `T042` landed.
    //
    // Kept as a note rather than deleted silently, because a reader looking for
    // *"where did the macro refusal go"* should find the answer at the site —
    // which is exactly what the mark half's note did for this one.

    /// **`T045`: the picker opens from a keystroke, filters, and closes.**
    ///
    /// `SPC u l` was bound to `open-picker` over the `unseen` source before
    /// this task and answered *"not built yet"*; the binding did not change,
    /// which is the keymap's own rule — *"unimplemented is a value, not an
    /// absence … the binding does not change when the phase lands."*
    ///
    /// **The row count is `0/0` and that is the task boundary, not a failure.**
    /// `T046` is *"Steel picker sources — unseen, files"*, so what supplies
    /// rows does not exist yet. What `T045` owes is a picker that opens,
    /// filters, selects and closes — and the `0/0` is the honest way to draw a
    /// source with nothing behind it, as against pretending to a list.
    #[test]
    fn the_picker_opens_filters_and_closes() {
        let scratch = Scratch::new("picker-open");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `SPC u l` — 3c's `+unseen · list`. Waited on the **count** rather
        // than on the source name: `SPC u` draws the which-key popup, whose
        // `+unseen` row matches the source id and would satisfy a looser
        // needle one keystroke early.
        let opened = editor.press_until(b" ul", "0/0");
        assert!(
            shows(&opened, "0/0"),
            "an empty source draws its count rather than pretending; frame was: {opened}"
        );

        // **The filter line owns every printable key while it is open**, and
        // the proof is the *file* rather than the frame. In normal mode `re`
        // is replace-char — it would turn `one` into `ene`. Reading the buffer
        // is unambiguous where reading the screen is not: ratatui emits only
        // changed cells, so a frame after typing carries `r`, `e`, `t` at three
        // columns and never a whole `> ret` to match on.
        editor.press_quietly(b"ret");

        // `esc` closes top-down (§9) and the buffer is back. Pressed quietly:
        // closing redraws the code that was behind the float, and the
        // statusline does not change, so there is no new text to wait on.
        editor.press_quietly(b"\x1b");
        editor.press_until(b":w\r", "sample.txt");
        editor.quit();

        let after = fs::read_to_string(&file).expect("written");
        assert_eq!(
            after, "one\ntwo\nthree\n",
            "the picker swallowed `ret`; had the machine seen it, `re` would have \
             replaced a character",
        );
    }

    /// **`T046`: a source defined at the REPL opens with no restart.**
    ///
    /// The id is one nothing shipped, so a picker that opens over it can only
    /// be reading what was just defined. Opened through the **door** rather
    /// than a keystroke — `open-picker` is `Allow` on MCP, so this is the path
    /// an agent takes, and it proves the `Intent` seam as well as the registry.
    ///
    /// **Its own test rather than a third act of the one below**, and that is
    /// not tidiness: as one seven-step session it went red under the full
    /// suite's sixteen-way parallelism and green on its own. Each
    /// `press_until` has its own deadline, so a long test is a test whose
    /// budget is the sum of everything before it.
    #[test]
    fn a_source_defined_at_the_repl_opens_with_no_restart() {
        let scratch = Scratch::new("picker-fresh-source");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            b"(define-picker-source! \"scratch\" \
              \"(lambda (args) (view/spans (list (view/span-row (list (view/run \\\"invented-just-now\\\" 'claude 'plain)) void))))\")\r",
            "#ok",
        );
        let fresh = editor.press_until(b"(open-picker! \"scratch\")\r", "invented-just-now");
        editor.quit();

        assert!(
            shows(&fresh, "invented-just-now"),
            "a source defined this session opened without a restart; frame was: {fresh}"
        );
    }

    /// **`T046`: `2a` and `3d` reproduce from a keystroke.**
    ///
    /// One session for both because they read the same store: two regions in
    /// one file are two `unseen` rows and one `files` row, and that difference
    /// *is* `3d`'s activity column.
    #[test]
    fn the_unseen_and_files_pickers_reproduce_from_a_keystroke() {
        let scratch = Scratch::new("picker-sources");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(1, 2), (3, 4)]).as_bytes(), "landed=2");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // `SPC u l` — the shipped `unseen` source, over the store.
        let listed = editor.press_until(b" ul", "2/2");
        assert!(
            shows(&listed, "unseen"),
            "the rows say what state they are in; frame was: {listed}"
        );
        editor.press_quietly(b"\x1b");

        // `3d` — `SPC f`. **The list is the workspace, and the store
        // annotates it**: the caption is *"the file picker carries agent
        // state: unseen counts + activity, not just names"*, and the mockup's
        // own rows include files carrying no activity at all.
        //
        // A picker that *filtered* by the store would show one row here. This
        // asserts the annotation instead — the count on the file that has
        // regions — and the test below asserts the other half, that a
        // workspace with no regions at all still lists.
        let files = editor.press_until(b" f", "unseen");
        editor.quit();

        assert!(
            shows(&files, "unseen"),
            "the file with regions carries its count; frame was: {files}"
        );
    }

    /// **The files picker lists a workspace with no regions in it at all.**
    ///
    /// Reported by Teej testing a normal build: *"file picker has no files in
    /// it — is it really a buffer list not a file list"*. It was neither: the
    /// source listed only paths the **store** had regions for, so an ordinary
    /// session with nothing declared opened an empty picker under a key
    /// labelled *"files"*.
    ///
    /// `3d` is explicit — *"not just names"* — and its own rows carry
    /// `src/main.rs` and `Cargo.toml` with no activity. The store is the
    /// annotation, never the filter.
    ///
    /// Nothing is declared here on purpose. A list that is empty without the
    /// store is the whole of the defect.
    ///
    /// **The workspace is the editor's cwd, not the scratch.** The pty child
    /// inherits this test process's directory, which is the `phosphor` crate —
    /// so what the picker lists is *this crate's* files, and asserting on the
    /// scratch's would be asserting the walk had a bug. `Cargo.toml` is the
    /// needle because every crate directory has one; the walk's own rules
    /// (skipping `target/`, descending, sorting, capping) are unit-tested in
    /// `crate::picker`, where they can be given a directory to walk.
    #[test]
    fn the_files_picker_lists_a_workspace_with_no_regions() {
        let scratch = Scratch::new("files-no-regions");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("alpha.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let listed = editor.press_until(b" f", "Cargo.toml");
        editor.quit();

        assert!(
            shows(&listed, "Cargo.toml"),
            "the picker lists the workspace with an empty store; frame was: {listed}"
        );
        assert!(
            !shows(&listed, "0/0"),
            "and it is not empty, which is the defect this test exists for; \
             frame was: {listed}"
        );
    }

    // -----------------------------------------------------------------------
    // Keys nothing pressed
    // -----------------------------------------------------------------------
    //
    // **A survey, after the file picker shipped a key that refused every row.**
    // That defect was not subtle — a whole keystroke on a shipped surface that
    // no test drove — so the honest follow-up is to ask the same question of
    // every other one. The live keymap answers `(keymap-entries)` with 428
    // bindings; 42 of those are leaves naming a capability, and grepping this
    // file for the bytes that press them found **19**. The tests below are the
    // ones worth having from the other 23.
    //
    // The grammar keys — `h`, `w`, `dw`, `ciw` and the rest — are not in that
    // count and do not want a pty test: they are the input machine's, they are
    // covered exhaustively in `phosphor-core`, and pressing each one here would
    // be re-testing `Machine::feed` through a terminal.

    /// **`J` joins lines, and nothing had pressed it.**
    ///
    /// The one buffer mutation bound to a bare capital that no other test
    /// reaches: `Editing::join` has exactly one Action arm and that arm has
    /// exactly one binding, so this key is the whole of its reachability.
    #[test]
    fn j_joins_the_next_line_onto_this_one() {
        let scratch = Scratch::new("join");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("join.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // On the grid rather than the wire: joining writes ` bravo` onto a row
        // that already said `alpha`, so the sentence this asserts is on the
        // screen and was never in the delta. See `Editor::shown_on_grid`.
        let screen = editor.shown_on_grid(b"J", "alpha bravo");
        let first = screen.line(0);
        let status = screen.line(SCREEN.ws_row - 1);
        editor.quit();

        assert!(
            first.contains("alpha bravo"),
            "the two lines became one, with a space at the seam; row was: {first}"
        );
        assert!(
            status.contains("[+]"),
            "and the buffer says it differs from disk; statusline was: {status}"
        );
    }

    /// **`]u` walks to an unseen region and `<C-o>` walks back** — three keys
    /// no test pressed, and they are one story.
    ///
    /// A region motion is the only thing a user can press that pushes a jump:
    /// `Editing::push_jump` has two callers, `goto_sequence` and `goto_anchor`.
    /// So the jumplist cannot be exercised without one, and testing them apart
    /// would mean inventing a second way onto the list.
    ///
    /// **The jumplist holds anchors, not line numbers**, which is why this is
    /// worth pressing rather than unit-testing: `push_jump` mints an anchor
    /// through the store and `jump` resolves it back, so a `<C-o>` that lands
    /// on the right line has driven `T042`'s ladder end to end from a key.
    #[test]
    fn a_region_motion_pushes_a_jump_and_the_jumplist_walks_back() {
        let scratch = Scratch::new("jumplist");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("walk.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\nsix\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(4, 5)]).as_bytes(), "landed=1");
        editor.press_until(b"(close-repl!)\r", "1    one");

        // The cursor readout is the observable, and it is *state* — so all
        // three of these read the grid rather than the wire.

        // `]u` — the next unseen region, from line 1.
        let moved = editor.shown_on_grid(b"]u", "4:1");
        assert!(
            statusline(&moved).contains("4:1"),
            "]u landed on the region's first line; statusline was: {}",
            statusline(&moved)
        );

        // `<C-o>` — back along the jumplist, to where `]u` was pressed.
        let back = editor.shown_on_grid(b"\x0f", "1:1");
        assert!(
            statusline(&back).contains("1:1"),
            "<C-o> returned to where the jump started; statusline was: {}",
            statusline(&back)
        );

        // `<C-i>` — forward again, the same list read the other way.
        let forward = editor.shown_on_grid(b"\x09", "4:1");
        editor.quit();
        assert!(
            statusline(&forward).contains("4:1"),
            "<C-i> went forward along the same list; statusline was: {}",
            statusline(&forward)
        );
    }

    /// **Every deferred command key says which task builds it** — `T098`'s
    /// claim, over the whole deferred surface rather than the one key it was
    /// written for.
    ///
    /// `runtime/keymaps.scm`'s header promises exactly this: a binding is
    /// *"legible when the capability's phase has not landed — the refusal names
    /// the task"*. Nine keys rely on it and not one was pressed. The failure it
    /// prevents is the one `T098` records: `q` bound to something unbuilt
    /// looked exactly like `q` bound to nothing.
    ///
    /// **A table, so it cannot go stale quietly.** When a task lands its key
    /// stops refusing and this goes red at the row that named it — the same
    /// shape as `scripts/lint-action-arms.sh`'s RECORDED list one layer out, a
    /// record that can only shrink.
    ///
    /// The tasks are not guesses: each is the `since.task` on that capability's
    /// own row in `action.rs`, which is where the refusal reads it from. So
    /// this asserts the sentence a person sees rather than a constant.
    #[test]
    fn a_deferred_binding_names_the_task_that_builds_it() {
        let scratch = Scratch::new("deferred");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("deferred.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `(keys, what it is, the task its refusal must name)`.
        //
        // **The tasks were wrong three ways when this was first written**, from
        // reading the keymap instead of the capability: `SPC c p` and `SPC c s`
        // open a *prompt*, so they answer `T058` and not the session task their
        // group is labelled with; `SPC t` is `set-pane-content`, `T054`; and
        // `SPC r d` is `open-disk-diff`, `T070`, one task along from the reload
        // beside it. Each is the `since` on that capability's own row in
        // `action.rs` — checked there, because the whole point of the assertion
        // is that a user is told the truth.
        // `SPC t` left this table when `T054` built the transcript pane —
        // the row it left behind is what `a_deferred_binding_names_the_task_
        // that_builds_it`'s failure looked like: `T054` was passed as the
        // wanted needle and the frame it got back was `1b` itself, splitting
        // open rather than refusing.
        let deferred: &[(&[u8], &str, &str)] = &[
            (b"?", "search backward", "T110"),
            (b"N", "previous search match", "T110"),
            // The review-block walk. Added when the audit noticed the table had
            // nine rows and the deferred surface had eleven, and re-pointed on
            // 2026-08-24: `goto_sequence` named `T053` for `BlockFile` while
            // `T053` was ticked, so both rows advertised finished work. `T109`
            // is the walk itself, and it owns opening the file — which is what
            // *"next file in the review block"* has always required.
            (b"]b", "next file in the review block", "T109"),
            (b"[b", "previous file in the review block", "T109"),
            // `SPC c p` and `SPC c s` left this table when `T058` built the
            // claude prompt — they raise the line now rather than refusing.
            // `SPC c i` left it when `T062` built the interrupt: it declines by
            // *name* now — `no turn to interrupt` — which is the difference
            // this table exists to make visible, and the row going is the
            // record shrinking as designed.
            // `SPC r r` left this table when `T069` armed `reload-from-disk`,
            // `SPC r d` when `T070` armed `open-disk-diff`, and `SPC j` when
            // `T073` armed `open-timeline`. Each acts now rather than naming a
            // task, which is this record shrinking for the only good reason it
            // can. What is left above is `T109`'s and `T110`'s, and those are
            // open tasks — so every row in this table names work that has
            // genuinely not been done.
        ];
        for (keys, what, task) in deferred {
            // **Pressed, then read off the *grid* — and it took both halves to
            // get here.**
            //
            // A bare `press_quietly` and a grid read raced whatever redrew
            // next, and an early version of this survey reported four working
            // keys as silently broken. So it became `press_until`, which waits.
            //
            // But `press_until` waits on the **byte delta**, which is what
            // ratatui's *diff* renderer emitted — and a settled row arrives in
            // pieces separated by cursor moves. CI caught that on 2026-08-25
            // with the frame reading `search needs the matcher — T 0 buil s
            // it`: every character present, none of them adjacent. The same
            // artifact is recorded one test up, where `T058`'s row came back as
            // `search is T058's oth  half`.
            //
            // `shown_on_grid_text` is both halves at once: it waits for the
            // needle, so nothing races it, and it reads the composed grid, so
            // nothing is split across an escape.
            editor.press_quietly(keys);
            let said = shown_on_grid_text(&editor, task);
            assert!(
                shows(&said, task),
                "{what} is deferred and must say so by name; frame was: {said}"
            );
            editor.press_quietly(b"\x1b");
        }

        // **And the row that left says what took its place.** A key dropping
        // out of this table because its task landed is the good case; a key
        // dropping out because nobody noticed it stopped refusing is the
        // failure the table is for. `SPC c i` with no turn running declines by
        // name, which is `T098`'s claim one rung up: a bound key that cannot
        // act says *what is missing* rather than *which task*.
        let named = editor.press_until(b" ci", "no turn to interrupt");
        assert!(
            shows(&named, "no turn to interrupt"),
            "`SPC c i` is built and refuses by name; frame was: {named}"
        );
        editor.press_quietly(b"\x1b");
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // The rest of the surface
    // -----------------------------------------------------------------------
    //
    // **The same survey, widened past the keymap.** Keys are one way in; the ex
    // line, the mouse and the floats are the others, and each was counted the
    // same way — enumerate what ships, grep for what presses it.
    //
    // * **Ex commands.** `(ex-entries)` answers **18**. Nine were typed by no
    //   test at all: `wall`, `wq`, `xit`, `close-buffer`, `transcript`,
    //   `inbox`, `diff-disk`, `reattach`, `comment`. Three of those are live.
    // * **Mouse.** `mouse_actions` handles three kinds — press, drag, wheel —
    //   and one test pressed the first two. The wheel had nothing.

    // -----------------------------------------------------------------------
    // The keys nothing pressed
    // -----------------------------------------------------------------------
    //
    // **`scripts/lint-key-coverage.sh` counted these, and the tests below are
    // the answer.** The lint recomputes what `loop_pty.rs` used to assert in a
    // comment: every key the layer binds, against every key a test presses. It
    // found thirty-two bindings that ship and that nothing had ever pressed —
    // most of them ordinary vim motions, which is the unglamorous half nobody
    // writes a test for because everybody assumes somebody did.
    //
    // Each of these asserts the **position**, off the statusline, through
    // `Editor::landed_at`. That is only readable because `Scratch` builds under
    // a short root now; the same assertions were impossible while a
    // 110-character temporary path made §11 shed the cursor segment.

    /// `$`, `0` and `^` — the three ends of a line.
    ///
    /// `^` is the one worth having a test for rather than the other two: it is
    /// *first non-blank*, so on an indented line it is neither `0` nor where
    /// the text begins by accident.
    #[test]
    fn the_line_ends_are_three_different_places() {
        let scratch = Scratch::new("line-ends");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("ends.txt");
        fs::write(&file, "    indented line here\nsecond\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        drop(editor.landed_at(b"$", "1:22"));
        drop(editor.landed_at(b"0", "1:1"));
        drop(editor.landed_at(b"^", "1:5"));
        editor.quit();
    }

    /// `{` and `}` by paragraph, `H` and `L` by screen.
    ///
    /// A paragraph here is what vim means by one — the blank line between
    /// blocks — and the screen motions land on the first and last rows the
    /// viewport is showing, which for a file shorter than the window is the
    /// first and last lines of the file.
    #[test]
    fn the_paragraph_and_screen_motions_move_by_block() {
        let scratch = Scratch::new("blocks");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("blocks.txt");
        fs::write(&file, "one\ntwo\n\nthree\nfour\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `}` — forward to the blank line that ends the first block.
        drop(editor.landed_at(b"}", "3:1"));
        // `{` — back to where the file begins.
        drop(editor.landed_at(b"{", "1:1"));
        // `L` — the last row on screen. Line 6, not 5: the fixture ends in a
        // newline, so the buffer has an empty last line and that is the row the
        // viewport bottoms out on.
        drop(editor.landed_at(b"L", "6:1"));
        // `H` — and the first.
        drop(editor.landed_at(b"H", "1:1"));
        editor.quit();
    }

    /// `W` skips punctuation; `%` finds the partner of a bracket.
    ///
    /// `W` is *blank-separated*, which is the whole difference from `w`:
    /// `foo.bar` is three words to `w` and one to `W`.
    #[test]
    fn the_big_word_and_bracket_motions() {
        let scratch = Scratch::new("bigword");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("words.txt");
        fs::write(&file, "foo.bar baz (nested (deep)) end\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `W` from column 1 clears `foo.bar` whole and lands on `baz`.
        drop(editor.landed_at(b"W", "1:9"));
        // `f(` to the outer bracket, then `%` to the one that closes it — past
        // the nested pair, which is what makes this a match rather than a find.
        drop(editor.landed_at(b"f(", "1:13"));
        drop(editor.landed_at(b"%", "1:27"));
        editor.quit();
    }

    /// `F` and `T` search backwards; `;` repeats and `,` reverses.
    ///
    /// The pair is the point. `;` and `,` carry the *last find* as state, so a
    /// test that pressed only one of them would not notice a build that stored
    /// the direction and forgot the character, or the reverse.
    #[test]
    fn the_backward_finds_and_their_repeats() {
        let scratch = Scratch::new("finds");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("finds.txt");
        fs::write(&file, "abcXdefXghi\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // To the end, then back to the nearest `X` behind the cursor.
        drop(editor.landed_at(b"$", "1:11"));
        drop(editor.landed_at(b"FX", "1:8"));
        // `;` — the same find again, still backwards.
        drop(editor.landed_at(b";", "1:4"));
        // `,` — and the other way.
        drop(editor.landed_at(b",", "1:8"));
        // `T` stops one short of what `F` lands on.
        drop(editor.landed_at(b"$", "1:11"));
        drop(editor.landed_at(b"TX", "1:9"));
        editor.quit();
    }

    /// The delimited text objects — `(`, `)` and `"`.
    ///
    /// `(` and `)` name the **same** object, which is the thing worth
    /// asserting: vim treats the open and close bracket as two spellings of one
    /// pair, so `di(` and `di)` take the same text, and a build that bound them
    /// to opening and closing separately would pass any test that pressed only
    /// one.
    #[test]
    fn the_delimited_text_objects_are_what_they_delimit() {
        let scratch = Scratch::new("objects");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("objects.txt");
        fs::write(&file, "let x = (a + b);\nsay \"hello there\";\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `di(` from inside the parentheses.
        editor.press_quietly(b"f(l");
        drop(editor.shown_on_grid(b"di(", "let x = ();"));
        // `di\"` on the next line. `0` first, and that is not decoration: after
        // the edit above the cursor sits mid-line, and `f\"` searches *forward*
        // — from there it finds the **closing** quote, `l` steps past it, and
        // the object then contains nothing. The first draft of this test did
        // exactly that and reported the object as broken.
        editor.press_quietly(b"j0f\"l");
        drop(editor.shown_on_grid(b"di\"", "say \"\";"));
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "let x = ();\nsay \"\";\n",
            "each object took exactly what its delimiters hold"
        );
    }

    /// `)` takes the same object as `(`.
    ///
    /// Its own test rather than a second press in the one above, because the
    /// claim is an *equality* between two keys: run the same edit with the
    /// other spelling and the file has to come back identical.
    #[test]
    fn the_closing_bracket_names_the_same_object_as_the_opening_one() {
        let scratch = Scratch::new("closing");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("closing.txt");
        fs::write(&file, "let x = (a + b);\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press_quietly(b"f(l");
        drop(editor.shown_on_grid(b"di)", "let x = ();"));
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "let x = ();\n",
            "`di)` and `di(` are the same edit — one pair, two spellings"
        );
    }

    /// `gu` and `g~` — the case operators.
    ///
    /// Spelled the way vim spells them — `guu`, `g~~`.
    ///
    /// **This test is why they work.** Written first as `guu` and it left the
    /// file untouched: the keymap bound each operator whole in
    /// operator-pending, so `gugu` doubled and vim's one-key shorthand had
    /// nothing to resolve to. The tails are bound now, in that scope only.
    ///
    /// The fixture is deliberately mixed so lowercase and toggle cannot produce
    /// the same answer — a build that ran one for the other would pass on
    /// `ABC`.
    #[test]
    fn the_case_operators_are_two_different_operators() {
        let scratch = Scratch::new("case");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("case.txt");
        fs::write(&file, "MiXeD CaSe\nsecond\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // **Waited for on the grid, not written and read.** `press_quietly`
        // settles on 250ms of quiet, and an operator's effect can land after
        // that: this test passed here and went red on a macOS runner with the
        // file still `MiXeD CaSe`, because `:w` won the race with the edit it
        // was meant to save. The buffer row showing the new text is the signal
        // that the edit happened; the file then only checks that `:w` wrote
        // what was on screen.
        drop(editor.shown_on_grid(b"guu", "mixed case"));
        editor.press_quietly(b":w\r");
        let lowered = fs::read_to_string(&file).expect("written");
        assert_eq!(lowered, "mixed case\nsecond\n", "`guu` lowercased the line");

        drop(editor.shown_on_grid(b"g~~", "MIXED CASE"));
        editor.press_quietly(b":w\r");
        editor.leave_by(b"ZQ");
        let toggled = fs::read_to_string(&file).expect("written");
        assert_eq!(
            toggled, "MIXED CASE\nsecond\n",
            "`g~~` toggled it, which on an all-lowercase line is all-upper"
        );
    }

    /// **§5's chrome strip has a field, and the ex line is on it.**
    ///
    /// Reported from the running editor as *"the cmd bar doesn't have the
    /// background colour applied"*, and it was true of the whole strip rather
    /// than the command line alone: the statusline had no background either.
    ///
    /// `interpret.rs`'s header had the cause written down the whole time —
    /// *"A `Node::Line` cannot say what ground it is painted on… This
    /// interpreter therefore draws a line transparently, over whatever the
    /// caller painted"* — and nothing painted. The `StatusLine` widget that
    /// `T025` replaced filled the field itself; the composed tree could not ask
    /// for one, and the caller was never told it had inherited the job.
    ///
    /// **The assertion is the escape sequence, not a colour name.** A cell with
    /// no background reports an empty string, which is the exact failure here:
    /// not the wrong colour, *no* colour, leaving the strip on whatever the
    /// terminal's default happens to be. That is invisible on a dark terminal
    /// tuned near `#0c0f0c` and a white band on a light one, which is why it
    /// survived every capture in the library.
    #[test]
    fn the_chrome_strip_is_painted_under_the_statusline_and_the_ex_line() {
        let scratch = Scratch::new("chrome-bg");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("bg.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        let row = SCREEN.ws_row - 1;
        // Column 60 is past the mode chip and the file name, so it is the
        // field itself rather than anything drawn on it.
        let idle = editor.shown_on_grid(b"", "alpha");
        let status_bg = idle.background(row, 60);
        assert!(
            !status_bg.is_empty(),
            "the statusline sits on §5's field, not on the terminal's default"
        );

        // `:` — the ex line takes the same row, and takes the field with it.
        let ex = editor.shown_on_grid(b":", "NORMAL");
        let ex_bg = ex.background(row, 60);
        editor.quit();
        assert_eq!(
            ex_bg, status_bg,
            "the ex line is drawn on the statusline's row and must be on the \
             same field — it is the same strip, not a second one"
        );
    }

    /// **`CTRL-^` — the alternate file, and back again.**
    ///
    /// vim's most-used buffer key and this build did not have it. It is worth a
    /// key rather than a picker row for the reason vim gives: the two files you
    /// move between during one edit are almost never adjacent in any list, so
    /// the thing you want is a toggle, not a search.
    ///
    /// **Pressed as `0x1e`, which is what the terminal actually sends.** Without
    /// the kitty protocol ctrl+6 and ctrl+^ are the same byte and crossterm
    /// decodes it as `^`; with the protocol the terminal distinguishes them and
    /// it arrives as `6`. The keymap binds both spellings, and this harness runs
    /// the legacy encoding — so a build that bound only `<C-6>` would pass a
    /// keymap inspection and do nothing here.
    #[test]
    fn ctrl_caret_goes_to_the_alternate_file_and_back() {
        let scratch = Scratch::new("alternate");
        let runtime = copy_layer(&scratch.path);
        let first = scratch.path.join("first.txt");
        let second = scratch.path.join("second.txt");
        fs::write(&first, "the first file\n").expect("a fixture");
        fs::write(&second, "the second file\n").expect("a sibling");

        let editor = Editor::open(&first, &scratch.state(), &runtime);

        // Nothing has been open but this file, so there is nowhere to go and
        // the key says so rather than doing nothing.
        let refused = editor.press_until(b"\x1e", "no alternate file");
        assert!(
            shows(&refused, "no alternate file"),
            "the first file of a session has no alternate; frame was: {refused}"
        );

        // Open the sibling, which makes `first.txt` the alternate.
        editor.press_until(
            format!(":edit {}\r", second.display()).as_bytes(),
            "the second file",
        );

        // `CTRL-^` — back to where we were. **The grid, not the delta**: the
        // two files share every word but one, so redrawing `first.txt` over
        // `second.txt` rewrites a handful of cells and the sentence being
        // waited for is never newly drawn in full.
        drop(editor.shown_on_grid(b"\x1e", "the first file"));

        // And again — the alternate is a toggle, not a stack.
        drop(editor.shown_on_grid(b"\x1e", "the second file"));
        editor.quit();
    }

    /// A different operator on top of a pending one **aborts**.
    ///
    /// `dc` is not "change" — it is a sequence vim rejects, and this build used
    /// to drop the `d` and wait for `c`'s operand, so a typo became a different
    /// edit rather than nothing. It matters more since the operator tails are
    /// bound in operator-pending: `du` now resolves to an operator where it
    /// used to resolve to nothing, and replacing rather than aborting would
    /// make `du` lowercase a line.
    #[test]
    fn a_second_operator_aborts_the_first_rather_than_replacing_it() {
        let scratch = Scratch::new("abort");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("abort.txt");
        fs::write(&file, "AlphaBeta\nsecond\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `du` — a pending delete, then the lowercase tail. Neither runs.
        editor.press_quietly(b"du");
        // And the editor is back in normal mode, taking keys as keys: `x`
        // deletes one character, which it could not do from operator-pending.
        editor.press_quietly(b"x");
        drop(editor.shown_on_grid(b"", "lphaBeta"));
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "lphaBeta\nsecond\n",
            "`du` did nothing at all — no delete, no lowercase — and left \
             normal mode ready for the next key"
        );
    }

    /// `<` dedents, `C` changes to the end of the line, `O` opens above.
    ///
    /// Three edits that each leave a different kind of trace, run in one
    /// session because the file they leave behind says all three happened.
    #[test]
    fn dedent_change_and_open_above() {
        let scratch = Scratch::new("edits");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("edits.txt");
        fs::write(&file, "        deep\nsecond line\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `<<` — one indent level off line 1.
        editor.press_quietly(b"<<");
        // `C` on line 2 — change to end of line, which is an insert session.
        // Typed with quiet presses: the file is the assertion, and waiting for
        // typed text on a frame is a delta read that can time out on text the
        // screen already shows.
        // `0` before `C`: `<<` leaves the cursor on the first non-blank, so `j`
        // arrives at column 5 and `C` from there keeps `seco`. The first draft
        // wrote `secochanged` and that is what it was telling us.
        editor.press_quietly(b"j0C");
        editor.press_quietly(b"changed");
        editor.press_quietly(b"\x1b");
        // `O` — open above, also an insert session.
        editor.press_quietly(b"O");
        editor.press_quietly(b"above");
        editor.press_quietly(b"\x1b");
        // All three edits are on screen before `:w` is asked to save them.
        drop(editor.shown_on_grid(b"", "above"));
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "    deep\nabove\nchanged\n",
            "dedent took one level, `C` replaced to the line end, `O` put a line above it"
        );
    }

    /// `<C-d>` and `<C-u>` move the **cursor**; `<C-f>` and `<C-b>` move the
    /// **viewport**.
    ///
    /// The four are one test because the distinction between the pairs is the
    /// claim, and it is the same one `the_wheel_scrolls_the_viewport_and_leaves_
    /// the_cursor_alone` makes for the mouse: reading further down a file must
    /// not take your insertion point with it. The keymap agrees — `<C-d>` and
    /// `<C-u>` are `key/motion`, `<C-f>` and `<C-b>` are `key/scroll` — and
    /// nothing had pressed any of them.
    #[test]
    fn the_half_page_keys_move_the_cursor_and_the_page_keys_move_the_view() {
        let scratch = Scratch::new("viewport");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("long.txt");
        // `row-0001` rather than `line 1`, so a needle cannot match its own
        // prefix: `line 1` is inside `line 10`, and a scroll assertion that
        // matched the wrong row would pass for the wrong reason.
        let body = (1..=200)
            .map(|n| format!("row-{n:04}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&file, format!("{body}\n")).expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `<C-d>` — half a page down, and the cursor went with it.
        let down = editor.shown_on_grid(b"\x04", "row-0016");
        let moved = statusline(&down);
        assert!(
            !moved.contains(" 1:1"),
            "`<C-d>` is a motion and must leave line 1; statusline was: {moved}"
        );

        // `<C-u>` — and back to the top.
        drop(editor.landed_at(b"\x15", "1:1"));

        // `<C-f>` — a page down. The first row scrolled off…
        let paged = editor.shown_on_grid(b"\x06", "row-0030");
        let grid = (0..SCREEN.ws_row)
            .map(|row| paged.line(row))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !grid.contains("row-0001"),
            "`<C-f>` scrolled the first row off the screen; grid was:\n{grid}"
        );

        // …and `<C-b>` brings it back. The wait is the assertion: it panics
        // with the grid if `row-0001` never returns.
        drop(editor.shown_on_grid(b"\x02", "row-0001"));
        editor.quit();
    }

    /// `<C-c>` leaves, and leaves **hard** — `quit` with `force` set.
    ///
    /// The keymap binds it to `:quit!` rather than to a cancel, which is worth a
    /// test precisely because the name suggests otherwise: a reader expecting
    /// SIGINT semantics would expect unsaved work to survive it, and it does
    /// not. The buffer is dirty when this presses, so a build that dropped the
    /// `force` would refuse and the child would still be running.
    #[test]
    fn ctrl_c_leaves_even_with_unsaved_work() {
        let scratch = Scratch::new("ctrl-c");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("dirty.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press(b"A");
        editor.press_until(b" edited", "[+]");
        editor.press_quietly(b"\x1b");

        // `leave_by` asserts the child exited successfully. A refusal would
        // leave it running and this would hang rather than pass.
        editor.leave_by(b"\x03");

        let written = fs::read_to_string(&file).expect("the file is still there");
        assert_eq!(
            written, "alpha\n",
            "`<C-c>` is `:quit!` — it leaves without writing, and the edit is gone"
        );
    }

    /// `@` offers a register to play, and an empty one plays nothing.
    ///
    /// **This test asserted that `@` names `T099` until that task built it**,
    /// and the row leaving is the record shrinking as designed — the same shape
    /// `a_deferred_binding_names_the_task_that_builds_it` has one section up.
    /// What replaced it is the claim worth keeping: `@` is a *prefix* now, it
    /// offers the twenty-six registers by name, and playing an untouched one is
    /// a no-op rather than a refusal.
    ///
    /// **An empty register is not an error**, which is the `register` query's
    /// own wording — *"an unset one is empty"* — and the right answer to
    /// *"what does `@z` do before you have recorded anything"*.
    #[test]
    fn at_offers_the_registers_and_an_empty_one_plays_nothing() {
        let scratch = Scratch::new("macros");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("m.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // The prefix draws its children — which is `T034`'s which-key over a
        // table `T099` generated, and proof the twenty-six bindings exist.
        let offered = grid_of(&editor.shown_on_grid(b"@", "play the macro in"));
        assert!(
            shows(&offered, "play the macro in q"),
            "`@` offers the registers by name; grid was:\n{offered}"
        );

        // And `@q` on an untouched register changes nothing.
        editor.press_quietly(b"q");
        let after = grid_of(&editor.shown_on_grid(b"", "alpha"));
        editor.quit();
        assert!(
            shows(&after, "alpha"),
            "an empty register plays nothing; grid was:\n{after}"
        );
    }

    /// `:quit`, `:edit` and `:theme` — the three ex commands that work and that
    /// nothing typed.
    ///
    /// `:quit` had never been pressed by any test in this repository, which is
    /// not as strange as it sounds: [`Editor::quit`] leaves with `ZQ`, so every
    /// one of the hundred-odd sessions here exits by the normal-mode key and
    /// the ex spelling was never exercised.
    #[test]
    fn the_ex_commands_that_open_switch_and_leave() {
        let scratch = Scratch::new("ex-works");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("first.txt");
        let other = scratch.path.join("second.txt");
        fs::write(&file, "the first file\n").expect("a fixture");
        fs::write(&other, "the second file\n").expect("a sibling");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `:edit` — open the sibling, spelled in full rather than as `:e`.
        let opened = editor.press_until(
            format!(":edit {}\r", other.display()).as_bytes(),
            "the second file",
        );
        assert!(
            shows(&opened, "the second file"),
            "`:edit` opened the file it names; frame was: {opened}"
        );

        // `:theme` — **not built**, and it says so by name. Written expecting
        // it to work and corrected by running it: `set-theme` answers *"not
        // built yet — T092 builds it"* even for a slug the layer ships, which
        // is why the theme tapes set the theme on the command line instead.
        //
        // **It named `T012` until 2026-08-24, and `T012` is ticked** — the
        // refusal sent a reader to a task that shipped three phases earlier.
        // `set-theme`'s row is stamped `T092` now, the task whose own *done
        // when* is this exact keystroke, and `scripts/lint-refusal-tasks.sh`
        // is what stops the stamp drifting back.
        let themed = editor.press_until(b":theme phosphor-light\r", "T092");
        assert!(
            shows(&themed, "T092"),
            "`:theme` is deferred and names the task that builds it; frame was: {themed}"
        );

        // `:quit` — and the child exits. `leave_by` fails if it does not.
        editor.leave_by(b":quit\r");
    }

    /// **`CP-8c` — the same editor in a jj repo, a git repo and a bare
    /// directory** (`S7.3`).
    ///
    /// That checkpoint's Claude-half is *"the entire `S7` acceptance set runs
    /// twice — once in a jj repo, once in a bare directory. Plus once in a git
    /// repo"*, and its stance is the thing being checked: **VCS is an
    /// enhancement and its absence is a normal state**. It fails if any surface
    /// is unavailable or any message implies something is missing.
    ///
    /// So this drives one fixture three times, changing **only the marker**,
    /// and asserts two things each pass:
    ///
    /// * the editor is *whole* — the buffer opens, a key edits, `:write` saves.
    ///   None of that is VCS's business and none of it may vary.
    /// * the timeline answers **differently but honestly** in each, because
    ///   `3b` genuinely is *"an enhancement view, only when jj is present"*.
    ///
    /// **No jj is required to run this.** `phosphor_vcs::detect` is
    /// filesystem-only — a `.jj` directory *is* a jj repository as far as
    /// detection is concerned — and `Repo::timeline` answers empty when the
    /// binary will not run. CI has no jj installed, and a test that skipped
    /// itself there would be a checkpoint nobody actually checks. What jj's
    /// real output parses to is held by `phosphor_vcs`'s own fixtures.
    #[test]
    fn cp8c_the_editor_is_whole_in_all_three_repository_kinds() {
        // `(marker, what the timeline should say)` — the whole matrix.
        let passes: &[(Option<&str>, &str)] = &[
            // A bare directory. Not an error, and the sentence says so.
            (None, "no repository here"),
            // Detected, supported, and simply not this view's tool.
            (Some(".git"), "the timeline is jj's"),
            // A jj repository with no jj behind it: the surface opens and is
            // honest about having nothing to show.
            (Some(".jj"), "no changes to show"),
        ];

        for (marker, expected) in passes {
            let scratch = Scratch::new(&format!(
                "cp8c-{}",
                marker.unwrap_or("bare").trim_matches('.')
            ));
            let runtime = copy_layer(&scratch.path);
            // **The repository lives beside the fixture, not around it.** A
            // marker in `scratch.path` is what the child stands in, so
            // detection finds this fixture's answer rather than the checkout's.
            let repo = scratch.path.join("work");
            fs::create_dir_all(&repo).expect("a working directory");
            if let Some(marker) = marker {
                fs::create_dir_all(repo.join(marker)).expect("a marker");
            }
            let file = repo.join("sample.txt");
            fs::write(&file, "alpha\nbeta\n").expect("a fixture");

            let editor = Editor::in_repo_at(&file, &scratch.state(), &runtime, &repo);

            // **The editor is whole.** Nothing here is VCS's business, so
            // nothing here may differ between the three passes — this is the
            // half of `CP-8c` that fails if a feature became unavailable.
            let opened = whole(&editor.screen());
            assert!(
                shows(&opened, "alpha"),
                "[{marker:?}] the buffer opened; frame was: {opened}"
            );
            editor.press_quietly(b"A!");
            editor.press_quietly(b"\x1b");
            // **Settled, not waited on a needle.** `:write` answers `done()`
            // with no note — it redraws and says nothing — so there is no word
            // to wait for and the file itself is the assertion.
            editor.press_quietly(b":write\r");
            assert_eq!(
                fs::read_to_string(&file).expect("the file survives"),
                "alpha!\nbeta\n",
                "[{marker:?}] editing and saving are unaffected by the repository"
            );

            // **And the timeline answers honestly, differently.** Three
            // situations, three sentences — an enhancement that is absent says
            // *what* is absent rather than failing.
            // **Pressed, then waited for on the composed grid.** Two of these
            // three answers are notices and one is a float, and the read has to
            // work for both: a bare `screen()` races the redraw, and
            // `press_until` waits on the byte *delta*, where a settled row
            // arrives split across cursor moves — which is exactly how the jj
            // pass first failed here, and the same artifact the deferred survey
            // records.
            editor.press_quietly(b" j");
            let said = shown_on_grid_text(&editor, expected);
            assert!(
                shows(&said, expected),
                "[{marker:?}] expected {expected:?}; grid was: {said}"
            );
            // **Never apologetic, and never a task id.** `CP-8c`'s own
            // question is whether anything feels degraded; these are the two
            // words that would mean it did.
            assert!(
                !shows(&said, "error") && !shows(&said, "failed"),
                "[{marker:?}] absence is a state, not a failure; frame was: {said}"
            );
            editor.press_quietly(b"\x1b");
            editor.quit();
        }
    }

    /// **`3b` declines by naming the state, in both of the two ways it can**
    /// (`T073`).
    ///
    /// This asserted `T073` until that task landed. What it holds now is the
    /// harder thing, and the one `CP-8c` actually reads: *"does anything feel
    /// degraded or apologetic?"* — a timeline is an **enhancement view, only
    /// when jj is present**, so its absence has to read as a fact rather than a
    /// failure. Both refusals are checked because they are different sentences
    /// about different situations, and collapsing them would be the editor
    /// saying *"no"* without saying *"to what"*.
    ///
    /// Both spellings are pressed — `SPC j` and `:timeline` — because
    /// `lint-key-coverage` counts the key and the ex line separately, and
    /// because a person who learned one should not find the other broken.
    #[test]
    fn the_timeline_declines_by_naming_the_state() {
        let scratch = Scratch::new("ex-timeline");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("t.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        // **No repository.** `Editor::open` sets `PHOSPHOR_VCS=0`, so the
        // editor is in the state `CP-8c` runs its third pass in — a bare
        // directory, where the answer is *"there is nothing here"* rather than
        // *"something went wrong"*.
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let bare = editor.press_until(b" j", "no repository");
        assert!(
            shows(&bare, "no repository here"),
            "outside a repository the timeline names that; frame was: {bare}"
        );
        assert!(
            !shows(&bare, "T073"),
            "and it names no task, because the task landed; frame was: {bare}"
        );
        editor.press_quietly(b"\x1b");
        editor.quit();

        // **A git repository.** Detected, supported, and simply not the tool
        // this view belongs to — which is a third answer again, and the one a
        // git user has to be able to read without concluding the editor is
        // broken.
        let git = Editor::in_a_repo(&file, &scratch.state(), &runtime);
        let wrong = git.press_until(b":timeline\r", "jj");
        assert!(
            shows(&wrong, "the timeline is jj's"),
            "in git the timeline says whose it is; frame was: {wrong}"
        );
        assert!(
            !shows(&wrong, "T073"),
            "and still names no task; frame was: {wrong}"
        );
        git.quit();
    }

    /// `gs` — the mark-seen operator.
    ///
    /// `SPC u s` marks the region under the cursor and has a test; `gs` is the
    /// other spelling and had none. The keymap's own note says why the letter
    /// is `gs` rather than `s`: `s` is vim's substitute and `CP-3` asks that it
    /// stay so.
    ///
    /// `gsj` — the operator over a motion. It marks, and the first reading of
    /// this test said it did not: the mark lands a frame after the keys do, and
    /// the statusline was read too early. The poll below is that lesson.
    ///
    /// The needle is the chip's **absence**. A file with nothing unseen draws
    /// no counter at all rather than `0 unseen` — §11's last-standing set is
    /// `✻` / `●n` / `!`, and `●0` says nothing worth a cell.
    #[test]
    fn gs_marks_a_region_seen() {
        let scratch = Scratch::new("gs");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("marked.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // A region claude declared over lines 1–2, so there is something to
        // mark. The same helper the store tests use.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(1, 2)]).as_bytes(), "landed=1");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // `shown`, not `shown_on_grid` with no keys: this harness accounts one
        // frame per `press`, and `g` opens a which-key group — so the group
        // popup and its dismissal are frames no press asked for. The
        // session-wide wait does not count frames and is the right instrument
        // for a key that draws a menu on the way.
        // **Either spelling** — `ShedStep::CounterWords` is the ladder's first
        // rung, so `1 unseen` becomes `●1` the moment the rest of the row wants
        // the room. The claim is that the region starts unseen, and both
        // spellings make it. See `shown_on_grid_any`.
        let before = shown_on_grid_any(&editor, &["1 unseen", "●1"]);
        assert!(
            before.contains("1 unseen") || before.contains("●1"),
            "the declared region starts unseen; session was: {before}"
        );

        // Polled rather than read once. `press_quietly` settles, but the chip
        // vanishing is a *removal* — there is no new text to settle on, so the
        // frame that drops it can land just after the read.
        editor.press_quietly(b"gsj");
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut after = statusline(&editor.screen());
        while after.contains("unseen") {
            assert!(
                Instant::now() < deadline,
                "`gsj` never cleared the counter; statusline was: {after}"
            );
            std::thread::sleep(Duration::from_millis(10));
            after = statusline(&editor.screen());
        }
        // `leave_by`, not `quit`: `quit` presses `esc` through `press`, which
        // accounts one frame per key — and `g` opened a which-key group, so
        // three frames arrived that no press asked for. `ZQ` leaves without
        // that bookkeeping, and this test's claim is about the store rather
        // than about frame counts.
        editor.leave_by(b"ZQ");
    }

    // -----------------------------------------------------------------------
    // Soak — what a long session grows
    // -----------------------------------------------------------------------
    //
    // **The genre nothing here had.** Every other test in this file presses a
    // handful of keys and asks what the frame says. None of them asks the
    // question a person answers by leaving the editor open all day: does
    // anything grow that should not?
    //
    // It is a real question for this build rather than a generic one. The
    // editor holds a language server as a child process, nucleo's thread pool,
    // an **append-only** undo journal and an append-only seen log, and three
    // of those four are supposed to grow. So the assertion cannot be "memory is
    // flat"; it has to be that growth is *bounded* — that the log compacts, and
    // that nothing accumulates per keystroke without limit.
    //
    // `#[ignore]`d, and run by `just soak`. It drives thousands of keystrokes
    // through a real child process, which is minutes rather than the seconds
    // every other test here costs, and `CLAUDE.md`'s rule about measurements
    // applies: a figure that moves with the machine has no business failing an
    // ordinary build.

    /// The child's resident set size in kilobytes, via `ps`.
    ///
    /// `ps` rather than anything cleverer because this needs one number from
    /// another process on both macOS and Linux, and `rss=` is spelled the same
    /// on both. Answers `None` if the process is gone, which the caller reads
    /// as a failure worth naming rather than a zero worth comparing.
    fn rss_kb(pid: u32) -> Option<u64> {
        let out = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    /// **A long session does not grow without bound.**
    ///
    /// Thousands of keystrokes of `xu` — delete a character, undo it — which
    /// is the cycle that exercises the undo journal hardest: every iteration
    /// appends records and every undo walks the tree.
    ///
    /// **Normal mode only, and no escape byte, which the first draft got
    /// wrong.** It cycled `ix<esc>u`, written to the pty in one blob — and
    /// `\x1b` immediately followed by `u` *is* the prefix of an escape
    /// sequence, so the parser ate both. The editor stayed in insert and typed
    /// four thousand characters onto line 1, which the final motion check
    /// caught by finding `INSERT` and column 4201 on the statusline. A soak
    /// that writes keys faster than a person does must not write any key whose
    /// meaning depends on what follows it.
    ///
    /// **The assertion is a ratio, not a number of bytes.** A resident-set
    /// figure is a wall clock in a costume: it moves with the allocator, the
    /// machine, and whatever else the runner is doing. What does not move is
    /// the shape — a session that accumulates per keystroke has a growth curve
    /// that this ratio catches, and a session that compacts does not. Both
    /// numbers are printed, because the number is the interesting part even
    /// when the assertion passes.
    ///
    /// The editor is asked to draw at the end, and that is half the test: a
    /// process that died at keystroke 3,000 would otherwise report a very
    /// stable resident set.
    /// One batch is written to the pty in a single call; the settle is per
    /// batch. Two thousand `xu` cycles is four thousand keystrokes.
    const CYCLES_PER_BATCH: usize = 100;
    const BATCHES: usize = 20;

    #[test]
    #[ignore = "thousands of keystrokes through a real child — `just soak`"]
    // The numbers are the deliverable. A measurement that only asserts a ratio
    // and prints nothing tells you it passed and never what it measured, which
    // is the half `just bench` exists to give — and `#[expect]` rather than
    // `#[allow]` so deleting the print is a compile error rather than a quiet
    // loss of the output.
    #[expect(clippy::print_stdout, reason = "a measurement reports its numbers")]
    fn a_soak_of_thousands_of_keystrokes_grows_within_a_bound() {
        let scratch = Scratch::new("soak");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("soak.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let pid = editor.child.id();
        editor.press_until(b"i", "INSERT");
        editor.press_quietly(b"\x1b");

        // **Written in batches, and settled between them rather than after
        // every key.** `press_quietly` waits 250ms of quiet, which is right for
        // a test asking what one keystroke did and catastrophic for one asking
        // about five thousand: the first draft of this spent its whole runtime
        // in that wait and never reached an assertion. A batch is written
        // straight to the pty and the settle happens once per batch, which is
        // also closer to what it is meant to imitate — somebody typing, not
        // somebody pausing a quarter-second between characters.
        let batch: Vec<u8> = b"xu".repeat(CYCLES_PER_BATCH);
        let soak = |editor: &Editor| {
            (&*editor.master)
                .write_all(&batch)
                .expect("the child takes the batch");
            editor.settle();
        };

        // Warm up first: the early cycles pay for lazily-built caches, the
        // syntax tree and the journal's first compaction, and charging those to
        // the soak would report startup as a leak.
        soak(&editor);
        let warm = rss_kb(pid).expect("the child is alive after the warmup");

        for _ in 0..BATCHES {
            soak(&editor);
        }
        let after = rss_kb(pid).expect("the child is alive after the soak");

        // Still drawing, which is the half a resident-set figure cannot say.
        // `G` — the last line, which is the empty one after `charlie`'s newline.
        // `landed_at` rather than a grid wait: `charlie` is already on screen,
        // so waiting for it would return without the motion having happened.
        drop(editor.landed_at(b"G", "4:1"));
        editor.quit();

        let cycles = CYCLES_PER_BATCH * BATCHES;
        println!(
            "soak: rss {warm} kB after warmup, {after} kB after {cycles} delete/undo cycles \
             ({} keystrokes)",
            cycles * 2
        );
        assert!(
            after < warm * 3,
            "resident set went {warm} kB -> {after} kB over {cycles} cycles. \
             The journal is append-only and compacts, so growth is expected and \
             unbounded growth is the defect — this is a ratio rather than a byte \
             count for the reason the benchmarks give about figures that move \
             with the machine"
        );
    }

    /// **The wheel scrolls, and the cursor stays where you left it.**
    ///
    /// `mouse_actions` answers a wheel with `View::Scroll` and nothing else,
    /// which is the whole claim: a viewport move is not a cursor move. It is
    /// the mouse half of *"nothing moves unless you asked"* — reading further
    /// down a file must not take your insertion point with it, or every wheel
    /// nudge while reading is an edit in the wrong place waiting to happen.
    ///
    /// Nothing had turned a wheel. The one mouse test in this file presses and
    /// drags, and those are the two kinds that *do* move the cursor, so the
    /// invariant this asserts had no test pulling the other way.
    #[test]
    fn the_wheel_scrolls_the_viewport_and_leaves_the_cursor_alone() {
        let scratch = Scratch::new("wheel");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("long.txt");
        let body = (1..=60)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&file, format!("{body}\n")).expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        assert!(
            editor.screen().line(0).contains("line 1"),
            "the file opens at the top"
        );

        // SGR reporting spells the wheel 64 (up) and 65 (down); `?1006h` is
        // what `mouse` writes, and it is the mode the editor asks for.
        let down = editor.shown_on_grid(&mouse(65, 10, 5, true), "line 4");
        assert!(
            down.line(0).contains("line 4"),
            "one notch moved the viewport down; row was: {}",
            down.line(0)
        );
        assert!(
            down.line(SCREEN.ws_row - 1).contains("1:1"),
            "and the cursor did not come with it — a wheel is not a motion; \
             statusline was: {}",
            down.line(SCREEN.ws_row - 1)
        );

        // And back, by the same amount, so the notch is symmetric.
        let up = editor.shown_on_grid(&mouse(64, 10, 5, true), "line 1");
        editor.quit();
        assert!(
            up.line(0).contains("line 1"),
            "a notch up undid a notch down; row was: {}",
            up.line(0)
        );
    }

    /// **`:wq` writes the buffer and leaves** — the commonest exit in vim, and
    /// nothing typed it.
    ///
    /// The two halves are one Action list — `save-buffer` then `quit` — and
    /// `CP-4` found by hand that the *order* of their refusals was wrong, which
    /// is the whole reason `Session::key` takes the first refusal rather than
    /// the last. That fix was pressed through `ZZ` and never through `:wq`,
    /// although `submit_ex` and the keymap build the same list. This is the ex
    /// half of it.
    ///
    /// **The assertion is on disk**, not on a frame: a `:wq` that quit without
    /// writing would leave a green frame and a lost edit, which is exactly the
    /// failure worth catching.
    #[test]
    fn wq_writes_the_buffer_and_leaves() {
        let scratch = Scratch::new("wq");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("wq.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // `J` makes the buffer differ from disk, so a write has something to do
        // and a quit has something to refuse if it happens first.
        editor.shown_on_grid(b"J", "alpha bravo");
        editor.leave_by(b":wq\r");

        let written = fs::read_to_string(&file).expect("the file survives");
        assert_eq!(
            written, "alpha bravo\n",
            "the join reached disk before the editor left"
        );
    }

    /// `:xit` is vim's other spelling of the same thing, and it is a separate
    /// row in the ex table — so it is a separate way for the list to be wrong.
    #[test]
    fn xit_is_the_same_exit_by_another_name() {
        let scratch = Scratch::new("xit");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("xit.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.shown_on_grid(b"J", "alpha bravo");
        editor.leave_by(b":xit\r");

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "alpha bravo\n",
            ":xit wrote what :wq writes"
        );
    }

    /// `:wall` — save every buffer, and do not leave.
    ///
    /// **This drives the shipping binary, which opens one buffer**, so what it
    /// can hold is that the one on screen is written and the editor stays. The
    /// *every* half is a question about `Buffers` and is asserted where two of
    /// them can be built — `wall_writes_past_a_buffer_it_cannot_write` in
    /// `main.rs`, which is also where the case that matters lives: a buffer
    /// that cannot be written does not stop the ones after it.
    #[test]
    fn wall_writes_without_leaving() {
        let scratch = Scratch::new("wall");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("wall.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.shown_on_grid(b"J", "alpha bravo");
        // The dirty marker going out is the observable: `[+]` means *differs
        // from disk*, so it clears exactly when the write lands.
        let saved = editor.shown_on_grid(b":wall\r", "wall.txt");
        let status = saved.line(SCREEN.ws_row - 1);
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "alpha bravo\n",
            ":wall wrote the buffer"
        );
        assert!(
            !status.contains("[+]"),
            "and the editor stopped calling it dirty; statusline was: {status}"
        );
    }

    /// **`<C-v>` in the picker opens the row in a new split**, which is Teej's
    /// ruling at `T088`'s entry: telescope's `<CR>` opens in the current
    /// window, `<C-v>` vertical, `<C-x>` horizontal.
    ///
    /// It declined with *"one pane until T088 splits it"* until step 12. What
    /// this can hold from the outside is the half a screen shows: the editor
    /// takes the key, does not refuse it, and ends up looking at the file the
    /// row named. That the *tree* gained a leaf is asserted where two panes can
    /// be built without a terminal — `a_split_puts_the_new_pane_on_the_side_it_was_told`
    /// in `main.rs`.
    ///
    /// **Not a new pane by default** is the other half of the ruling, and
    /// `<CR>` is what proves it: a picker that split on every accept would make
    /// finding a file a window-management decision, which is the thing those
    /// defaults exist to avoid.
    #[test]
    fn control_v_in_the_picker_opens_the_row_in_a_split() {
        let scratch = Scratch::new("picker-split");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        // **`Cargo.toml` is the needle for the reason the tab-cycle test gives
        // it**: the `files` source lists the editor's cwd, which is the crate
        // directory, so a file written into the scratch tree is not in it.
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(b"(open-picker! \"files\")\r", "Cargo.toml");
        editor.press_until(b"Cargo", "Cargo.toml");
        let split = editor.press_until(b"\x16", "[package]");
        editor.quit();

        assert!(
            shows(&split, "[package]"),
            "the row opened and the split is showing that file's first line; \
             frame was: {split}"
        );
        assert!(
            !shows(&split, "T088"),
            "and it no longer declines by naming the task that builds it; \
             frame was: {split}"
        );
    }

    /// **The `<C-w>` window keys, on a real terminal.**
    ///
    /// `T088` shipped four capabilities with arms and nothing bound to them —
    /// its acceptance asked for arms and a query and never for keys, so for a
    /// while the only way a person could make a split was the files picker's
    /// `<C-v>`. `scripts/lint-capability-bindings.sh` is what stops the next
    /// one shipping unreachable; this is what proves these ones are not.
    ///
    /// **One test for the whole set, deliberately.** Each key is one call into
    /// machinery that is unit-tested against the tree directly — split, focus,
    /// close and resize all have their own assertions in `main.rs` over two and
    /// three panes. What only a terminal can say is that the *bindings* reach
    /// them, and that is one question asked eleven times.
    ///
    /// **The prefix is pressed on its own**, and that is not padding: `<C-w>`
    /// raises `3c`'s which-key grid, so a single `press_until(b"\x17v", …)`
    /// matches the *popup* frame and returns before the split has happened.
    /// The first press waits for the grid the popup draws, the second for the
    /// screen after it.
    #[test]
    fn the_window_keys_split_focus_resize_and_close() {
        let scratch = Scratch::new("window-keys");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // The prefix draws the grid, and the grid is the list of these keys.
        let hints = editor.shown_on_grid(b"\x17", "split right");
        assert!(
            shows(&grid_of(&hints), "focus the next pane"),
            "the which-key grid lists what `<C-w>` can do"
        );
        editor.shown_on_grid(b"\x1b", "NORMAL");

        // **Two panes on one buffer draw the same text twice**, which is the
        // observable a terminal has and a unit test does not.
        //
        // **Both bytes in one literal**, and that is not cosmetic: `<C-w> v` is
        // *one* notation, `key_coverage.py` spells it `\x17v`, and a test that
        // pressed the prefix and the key separately would cover neither. It
        // matches the popup frame first — `shown_on_grid` settles after
        // matching, so the screen it answers is the one after the split.
        let split = grid_of(&editor.after(b"\x17v"));
        assert!(
            twice(&split, "alpha"),
            "<C-w>v put a second pane beside the first; grid was: {split}"
        );

        // **Left and right, in a layout that has a left and a right.** The
        // vertical pair is tested after the horizontal split below, and the
        // separation is the point: `<C-w>k` with nothing above refuses, which
        // is correct and is what `T098`'s rule makes a key say out loud.
        for keys in [&b"\x17h"[..], b"\x17l", b"\x17w", b"\x17W"] {
            let moved = grid_of(&editor.after(keys));
            assert!(
                !moved.contains("no such"),
                "a focus key refused; grid was: {moved}"
            );
            assert!(
                twice(&moved, "alpha"),
                "and both panes are still drawn; grid was: {moved}"
            );
        }

        // Resize both ways.
        for keys in [&b"\x17+"[..], b"\x17-"] {
            let sized = grid_of(&editor.after(keys));
            assert!(
                twice(&sized, "alpha"),
                "a resize kept both panes; grid was: {sized}"
            );
        }

        // A third pane below, which is what gives `j` and `k` somewhere to go.
        let stacked = grid_of(&editor.after(b"\x17s"));
        assert!(
            thrice(&stacked, "alpha"),
            "<C-w>s stacked a third; grid was: {stacked}"
        );

        for keys in [&b"\x17k"[..], b"\x17j"] {
            let moved = grid_of(&editor.after(keys));
            assert!(
                !moved.contains("no such"),
                "a vertical focus key refused in a layout that has one; grid \
                 was: {moved}"
            );
        }

        let closed = grid_of(&editor.after(b"\x17c"));
        assert!(
            !closed.contains("no such"),
            "closing a pane refused; grid was: {closed}"
        );
        assert!(
            twice(&closed, "alpha") && !thrice(&closed, "alpha"),
            "and there are two left; grid was: {closed}"
        );

        // The ex spellings of the same two.
        editor.after(b":split\r");
        let vsplit = grid_of(&editor.after(b":vsplit\r"));
        editor.quit();

        assert!(
            thrice(&vsplit, "alpha"),
            ":vsplit is <C-w>v under its other name; grid was: {vsplit}"
        );
    }

    /// **`T057`: `7d` and `5d` reproduce, and `:start-session` starts a session.**
    ///
    /// The dashboard is one screen with two data shapes — `7d` is *"session /
    /// none running"* and `5d` is the same surface with discovery's list under
    /// it. Discovery answers empty until v1.5's tmux control mode, so what a
    /// terminal can show today is `7d`, and `5d`'s branch is written in
    /// `runtime/dashboard.scm` against the day it is not.
    ///
    /// **Built entirely from the spans hatch**, like `:arch` — the second proof
    /// that `Node::Spans` is sufficient for a real surface, and a better one,
    /// because these rows change with the session where the diagram's numbers
    /// only change with the store.
    ///
    /// **`:start-session` is read off the composed grid and not off the byte delta**, and
    /// that distinction is the whole of `OPEN-QUESTIONS.md` §54. The verb was
    /// recorded there as reaching the client and never finishing the handshake,
    /// on the evidence of a probe that searched the raw pty stream for
    /// `claude idle`. It finishes: diff rendering writes a settled statusline in
    /// pieces separated by cursor moves, so the needle is on the screen and
    /// never in the bytes. [`Editor::shown_on_grid`] already says which reader
    /// is which; the probe was not using it.
    #[test]
    fn the_dashboard_reproduces_screen_7d_and_cn_starts_a_session() {
        let scratch = Scratch::new("dashboard");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // **`7d`: cold start.** No session, and the surface says so in a
        // sentence rather than a glyph — the strip has no room and `7d` does.
        let cold = editor.press_until(b":dashboard\r", "none running");
        assert!(
            shows(&cold, "none running"),
            "`7d` states what it found; session was: {cold}"
        );
        assert!(
            // Needled on the words, not the spacing: adjacent `view/run`s are
            // laid out with a cell between them, so `:start-session start claude` in the
            // source is `:start-session  start claude` on the grid.
            shows(&cold, "start claude"),
            "and offers `7d`'s three verbs; session was: {cold}"
        );
        editor.press_quietly(b"\x1b");

        // **`:reattach` with nothing to reattach to declines by name.** `7b`'s
        // remedy needs a session to have existed; `T057` built the verb, so it
        // no longer names a task — it says what is missing.
        let nothing = editor.press_until(b":reattach\r", "no session to reattach");
        assert!(
            shows(&nothing, "no session to reattach"),
            "the remedy says what it needs; session was: {nothing}"
        );

        // **`:start-session` — the verb `7d` names.** The one-off form of
        // `(set-option! "agent-command" …)`, and `7d` draws it because a
        // dashboard whose only remedy is an option is not a remedy.
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");
        let started = format!(":start-session python3 {} turn\r", agent.display());
        // **The notice first, then the state.** `claude attached` is written
        // fresh on the whole statusline row and holds it; the strip underneath
        // only becomes readable once a keystroke has retired the notice, which
        // is why this is two waits and not one.
        editor.shown_on_grid(started.as_bytes(), "claude attached");
        let running = editor.shown_on_grid(b"\x1bj", "claude idle");
        assert!(
            shows(&grid_of(&running), "claude idle"),
            "`:start-session` attaches and the strip settles to idle; grid was:\n{}",
            grid_of(&running)
        );

        // And `7d` redraws from the live session rather than from what it said
        // the first time — `none running` is gone.
        let warm = editor.shown_on_grid(b":dashboard\r", "phosphor");
        let warm = grid_of(&warm);
        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
        assert!(
            !shows(&warm, "none running"),
            "the dashboard follows the session it started; grid was:\n{warm}"
        );
    }

    /// **`T056`: the OSC 8 sequence reaches a real terminal.**
    ///
    /// The task's *Done when* is *"clicking a tool row jumps to the file and
    /// range, on the primary terminal"*, and the click is Tier 3 —
    /// `docs/TASKS.md`'s own table says *"links may render, but nothing can
    /// click one"*, which is why `CP-6` asks Teej to press it inside tmux. What
    /// a test can hold this to is everything up to the press: that the bytes
    /// are emitted, that the URI names the file and the line the agent gave,
    /// and that the whole sequence arrives in one piece.
    ///
    /// **Read off the raw stream on purpose.** Every other assertion in this
    /// file reads the composed grid, because a grid is what a person sees —
    /// this one is about bytes a grid *cannot* show, since an escape sequence
    /// occupies no cell. §54 is the entry about using the wrong reader; this is
    /// the case where the byte stream is the right one.
    #[test]
    fn a_tool_row_emits_its_osc_8_link_to_the_terminal() {
        let scratch = Scratch::new("osc8");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} turn\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // A turn, so there is a tool row — the fixture's `turn` mode edits
        // `/tmp/toy/src/retry.rs` at line 19 and says so in `locations`.
        editor.press_quietly(b":claude add retry with backoff\r");
        shown(&editor, "claude idle");
        editor.shown_on_grid(b" t", "retry.rs");

        let bytes = editor.raw();
        assert!(
            bytes.contains("\u{1b}]8;;file:///tmp/toy/src/retry.rs#L19\u{1b}\\"),
            "the opener names the file and the line the agent gave"
        );
        // **The closer, and specifically that it follows the target text.** An
        // opener alone is the failure this whole design is arranged against: a
        // link that never closes runs on across everything drawn after it.
        assert!(
            bytes.contains("src/retry.rs\u{1b}]8;;\u{1b}\\"),
            "and the sequence closes immediately after the text it wraps"
        );

        editor.leave_by(b"ZQ");
    }

    /// **`T059`: screen `4a` reproduces, and its digits answer.**
    ///
    /// *"mid-turn question · quick-answer with digits, prose with `:c`, or
    /// ignore until later"*. The float, the wrapped prose, the amber `[n]`
    /// column, and the digit that closes it.
    ///
    /// **The producer is `:ask`**, an ex command in `runtime/asks.scm`. The
    /// real one is the agent, over whatever a question turns out to be on the
    /// ACP wire — `T060`'s queue and `T061`'s permission flow — and a screen
    /// nothing can put on screen is a screen nothing can check.
    #[test]
    fn screen_4a_reproduces_and_its_digits_answer() {
        let scratch = Scratch::new("question");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // A session, because `4a`'s strip says `! claude waiting` and there is
        // nothing to be waiting *for* without one.
        let started = format!(":start-session python3 {} mute\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        let asked = grid_of(&editor.shown_on_grid(
            b":ask cargo test fails on trunk before my change. Bisect it first,               or stay scoped to the retry work?|bisect trunk|stay scoped|show me the failure\r",
            "needs input",
        ));
        assert!(
            shows(&asked, "bisect trunk"),
            "`4a`'s options are drawn; grid was:\n{asked}"
        );
        assert!(
            shows(&asked, "[3] show me the failure"),
            "each with the digit that answers it; grid was:\n{asked}"
        );
        // **Wrapped, not one long row.** `4a` draws the question over two
        // lines, and a float as wide as claude's longest sentence is the thing
        // `QuestionBody::desired_height` takes a width for.
        let head = asked
            .lines()
            .position(|row| row.contains("Bisect it first"))
            .expect("the question is on screen");
        let tail = asked
            .lines()
            .position(|row| row.contains("work?"))
            .expect("and so is the end of it");
        assert!(
            tail > head,
            "the prose wraps rather than running off the float; grid was:\n{asked}"
        );
        // §5's `!`. **Waiting outranks working**, which is the point of the
        // state: what it means is *the next move is yours*.
        assert!(
            shows(&asked, "claude waiting"),
            "and the strip says whose move it is; grid was:\n{asked}"
        );

        // **A digit no option carries declines by name.** A float that ate the
        // key would be indistinguishable from one that had not noticed.
        let unoffered = editor.press_until(b"7", "no option 7");
        assert!(
            shows(&unoffered, "no option 7"),
            "an unoffered digit says so; session was: {unoffered}"
        );

        // And the one that is offered answers, which closes the float.
        let answered = editor.press_until(b"2", "answered 2");
        assert!(
            shows(&answered, "answered 2"),
            "the digit answers; session was: {answered}"
        );
        let after = grid_of(&editor.shown_on_grid(b"", "alpha"));
        assert!(
            !shows(&after, "needs input"),
            "and the float that asked is gone; grid was:\n{after}"
        );
        // **The `!` goes with it**, because the strip and the float read one
        // map. A `!` that outlived its question is §5's *"always truthful"*
        // failing in the one moment it is being read.
        assert!(
            !shows(&after, "claude waiting"),
            "and the strip stops waiting; grid was:\n{after}"
        );

        editor.leave_by(b"ZQ");
    }

    /// **`T060`: a question arriving while something else holds focus waits.**
    ///
    /// Q9, in one sentence: *"a question arriving while another float holds
    /// focus sets the statusline `!` and waits. Surfaces when nothing else
    /// holds focus."* Both halves, in order — and the acceptance's own wording
    /// for the first is that asking while a picker is open **destroys
    /// nothing**.
    ///
    /// **Asked from the REPL and not from `:ask`**, because the point is that
    /// the ask arrives while a surface is up, and an ex line typed at a picker
    /// goes into the picker's filter. The REPL is a surface that can also run
    /// the producer, which makes it the one place a pty can stage this.
    #[test]
    fn a_question_that_arrives_behind_a_surface_waits_for_it() {
        let scratch = Scratch::new("ask-queue");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // Needled on the *receipt* — `enqueue-ask` answers the id it minted, so
        // an agent can name the ask when the answer comes back. `steel` is in
        // the REPL header, already drawn, and never in the delta.
        editor.press_until(
            b"(enqueue-ask! \"deploy to prod?\" (list (hash \"digit\" 1 \"label\" \"go\")))\r",
            "⇒ 1",
        );

        // **It waited.** The REPL still owns the screen, and the question has
        // not painted over what was being typed into it.
        let behind = grid_of(&editor.shown_on_grid(b"", "REPL"));
        assert!(
            !shows(&behind, "needs input"),
            "the question waits behind the surface; grid was:\n{behind}"
        );
        // And it said so. §5's `!` is the whole notification a queued ask gets,
        // which is why `StatusLineVm::ask_pending` carries Q9's sentence in its
        // own doc — and why it read `false` from the binary until this task.
        assert!(
            shows(&behind, "deploy to prod?") || behind.contains('!'),
            "and the strip carries the flag; grid was:\n{behind}"
        );

        // **And it surfaces when nothing else holds focus.**
        let surfaced = grid_of(&editor.shown_on_grid(b"(close-repl!)\r", "needs input"));
        assert!(
            shows(&surfaced, "deploy to prod?"),
            "the question comes up on its own; grid was:\n{surfaced}"
        );
        assert!(
            shows(&surfaced, "[1] go"),
            "with its options intact — nothing was destroyed; grid was:\n{surfaced}"
        );

        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T099`: `q` records a macro and `@` plays it back.**
    ///
    /// The task's own *Done when*, in the running binary: `q<reg>` records,
    /// `@<reg>` replays through `feed-keys`, and the `register` query reads the
    /// same register back.
    ///
    /// **The wall this had been stuck behind was one line of scheme.**
    /// `runtime/keymaps.scm` recorded that *"a keymap cannot ask a query"* — and
    /// asking was always fine. `phosphor/resolve` called a function binding and
    /// **discarded its answer**, so a thunk could open a float but could not run
    /// an Action. It honours a role now, and `@`'s thunk reads the register at
    /// press time and hands the keys to `feed-keys`, which is the shape `T099`
    /// described when it added the query for exactly this.
    #[test]
    fn q_records_a_macro_and_at_plays_it_back() {
        let scratch = Scratch::new("macros");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `qa` starts, `x` is the macro, `qa` stops. **The `qa` that stops is
        // not part of what it stopped** — a recording that included its own
        // terminator would turn itself off half way through the first replay.
        editor.press_quietly(b"qa");
        editor.shown_on_grid(b"x", "lpha");
        editor.press_quietly(b"qa");

        // The register holds it, read back through the door.
        editor.press_until(b":repl\r", "steel");
        let held = editor.press_until(b"(list (register \"a\") (recording))\r", "⇒");
        assert!(
            shows(&held, "(\"x\" \"\")"),
            "the register holds the macro and nothing is recording; session was: {held}"
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // **And it plays.** `alpha` lost its `a` to the recording; `@a` takes
        // the `l`.
        let played = grid_of(&editor.shown_on_grid(b"@a", "pha"));
        editor.leave_by(b"ZQ");
        assert!(
            shows(&played, "pha"),
            "`@a` replayed the macro; grid was:\n{played}"
        );
    }

    /// **`T096`: `set-soft-wrap` toggles wrapping, both ways.**
    ///
    /// The verb was declared by `T081`, generated into Steel, MCP and the CLI,
    /// and applied by nothing — *"a capability that the doors advertise and that
    /// does nothing is worse than one that is absent"*, which is the task's own
    /// sentence and what `scripts/lint-action-arms.sh` has said on every run
    /// since.
    ///
    /// **Both ways is the assertion that matters.** Turning wrapping on is one
    /// line; turning it off needs the loop to *unwrap* a rope it already
    /// wrapped, and without that the toggle works exactly once.
    ///
    /// **And it was reachable from no door at all.** Arming it in `Editing::act`
    /// made it a *key's* verb; every door lands in `AppHost::apply`, which does
    /// not fall through — so `(set-soft-wrap! …)` at the REPL went on answering
    /// `not built yet — T081 builds it` with the arm sitting right there.
    #[test]
    fn set_soft_wrap_wraps_a_long_line_and_unwraps_it_again() {
        let scratch = Scratch::new("soft-wrap-verb");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("long.txt");
        // Wider than the 120-column harness, in one line, so wrapping is the
        // only thing that can put its tail on the screen.
        let long = "alpha ".repeat(40);
        fs::write(&file, format!("{long}\n")).expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Off by default — `init.scm` sets `soft-wrap` false, and the tail of
        // the line is off the right edge rather than on a second row.
        let flat = grid_of(&editor.shown_on_grid(b"", "alpha"));
        assert!(
            !flat.contains('↪'),
            "no continuation mark before the verb runs; grid was:\n{flat}"
        );

        // **`:wrap`, the way vim spells it**, rather than the REPL — the verb
        // is reachable from all three doors and the one a *person* has is the
        // one worth pressing.
        let wrapped = grid_of(&editor.shown_on_grid(b":wrap\r", "↪"));
        assert!(
            wrapped.contains('↪'),
            "`:wrap` wrapped it; grid was:\n{wrapped}"
        );

        // **And off again.** This is the half that did not work: the option
        // moved and the rope did not, so a buffer stayed wrapped until it was
        // reopened.
        editor.press_quietly(b":nowrap\r");
        let flat_again = grid_of(&editor.shown_on_grid(b"", "alpha"));
        editor.leave_by(b"ZQ");
        assert!(
            !flat_again.contains('↪'),
            "and unwrapped it again; grid was:\n{flat_again}"
        );
    }

    /// **`T062`: screen `7e` reproduces — `esc` stops the turn at a boundary.**
    ///
    /// *"`esc` pauses at the next tool boundary · steer, resume, or abort · the
    /// seam is recorded"*, and the acceptance's own words: **from a keystroke**,
    /// and it **reaches the next tool boundary**.
    ///
    /// **The `dawdle` fixture mode exists for this and only this.** `esc` has to
    /// arrive while a turn is running and *before* the agent's next tool call;
    /// every other mode reaches that boundary in microseconds, so a test would
    /// pass or fail on scheduling. `dawdle` puts two seconds between the prose
    /// and the call, which is the window `7e` is about.
    #[test]
    fn screen_7e_reproduces_and_esc_stops_at_the_next_tool_boundary() {
        let scratch = Scratch::new("interrupt");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} dawdle\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // **Waited for, not slept through.** The turn has to be *running* when
        // `esc` lands, and `claude working` on the strip is the editor saying
        // so — which is what `SessionState::Working` is for.
        editor.press_quietly(b":claude rewrite the retry loop\r");
        editor.shown_on_grid(b"", "claude working");

        // The keystroke. It says what it is going to do, because the pause has
        // not happened yet — the boundary may be a second away, and an `esc`
        // with nothing on the strip reads as a key that did nothing.
        let asked = editor.press_until(b"\x1b", "pausing at the next");
        assert!(
            shows(&asked, "pausing at the next tool boundary"),
            "`esc` says what it asked for; session was: {asked}"
        );

        // **And then it reaches one.** Nothing is pressed to make this happen:
        // the agent gets there on its own schedule and the boundary is where
        // the editor stops.
        // **A harmless key first.** A notice borrows the whole statusline row,
        // so `pausing at the next tool boundary` sits where the session chip
        // goes until a keystroke retires it — waiting for `claude paused` with
        // the notice still up is a thirty-second deadline on a screen that is
        // already right.
        let paused = grid_of(&editor.shown_on_grid(b"j", "claude paused"));
        assert!(
            shows(&paused, "claude paused"),
            "the strip says nothing is moving; grid was:\n{paused}"
        );

        let stopped = grid_of(&editor.shown_on_grid(b" t", "paused at tool boundary"));
        assert!(
            shows(&stopped, "acp · paused"),
            "`7e`'s header says what is running, and nothing is; grid was:\n{stopped}"
        );
        // **The held call, drawn and not run.** A pause you cannot see the edge
        // of is indistinguishable from a hang; this row is the boundary made
        // visible.
        assert!(
            shows(&stopped, "next: edit"),
            "the call it stopped before is on screen; grid was:\n{stopped}"
        );
        assert!(
            shows(&stopped, "paused at tool boundary"),
            "and the seam is recorded; grid was:\n{stopped}"
        );
        // **The pause outranks the stop reason.** The toy agent finishes its
        // turn regardless — it does not honour `session/cancel`, which a real
        // one would — and `✻ EndTurn` used to overwrite this seam, leaving a
        // screen that had forgotten the pause it was still in.
        assert!(
            !shows(&stopped, "EndTurn"),
            "and is not overwritten by the turn ending; grid was:\n{stopped}"
        );
        // `7e`'s three ways on.
        assert!(
            shows(&stopped, "steer and resume") && shows(&stopped, "abandon the turn"),
            "with the three ways on from here; grid was:\n{stopped}"
        );

        // `:abort` — the held call does not run, and the seam says so.
        let abandoned = editor.press_until(b":abort\r", "turn abandoned");
        assert!(
            shows(&abandoned, "turn abandoned"),
            "aborting says so; session was: {abandoned}"
        );

        editor.leave_by(b"ZQ");
    }

    /// **`T062`: the other two ways on from a boundary — steer, and resume.**
    ///
    /// `:abort` is proven beside `7e` above; these are its siblings, and they
    /// differ in exactly one thing each. **`:resume` runs the held call**, so
    /// the row moves out of `next:` and into the transcript proper. **`:steer`
    /// does that *and* sends the correction**, which is what makes it steering
    /// rather than a note — the agent gets a prompt and what it does next is a
    /// turn that heard you.
    #[test]
    fn steering_and_resuming_both_run_the_held_call_and_only_one_speaks() {
        let scratch = Scratch::new("steer");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} dawdle\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // Pause once, resume, and the held call is a call again.
        editor.press_quietly(b":claude first go\r");
        editor.shown_on_grid(b"", "claude working");
        editor.press_until(b"\x1b", "pausing at the next");
        editor.shown_on_grid(b"j", "claude paused");
        let resumed = editor.press_until(b":resume\r", "resumed");
        assert!(
            shows(&resumed, "resumed"),
            "`:resume` says so; session was: {resumed}"
        );
        let after = grid_of(&editor.shown_on_grid(b" t", "retry.rs"));
        assert!(
            !shows(&after, "next: edit"),
            "and the held call is no longer held; grid was:\n{after}"
        );
        assert!(
            !shows(&after, "paused at tool boundary"),
            "and the pause seam is gone with it; grid was:\n{after}"
        );
        editor.press_quietly(b"\x1b");

        // **Pause again and steer.** The correction goes out as a prompt, which
        // is observable: the toy agent echoes what it heard.
        editor.press_quietly(b":claude second go\r");
        editor.shown_on_grid(b"", "claude working");
        editor.press_until(b"\x1b", "pausing at the next");
        editor.shown_on_grid(b"j", "claude paused");
        let steered = editor.press_until(b":steer leave the tests alone\r", "steered");
        assert!(
            shows(&steered, "steered — carrying on"),
            "`:steer` says so; session was: {steered}"
        );
        let heard = grid_of(&editor.shown_on_grid(b" t", "leave the tests alone"));
        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
        // **The agent heard it**, which is the whole difference from `:resume`.
        assert!(
            shows(&heard, "heard: leave the tests alone"),
            "the correction reached the agent; grid was:\n{heard}"
        );
    }

    /// **`T061`: screen `7a` reproduces, and always-allow writes a rule.**
    ///
    /// *"consequential command · exact invocation shown · always-allow writes a
    /// legible rule"*, and the acceptance's two halves: **readable by a human**
    /// and **takes effect next time**.
    ///
    /// **The rule is in the option's own label**, which is one better than the
    /// mockup: `7a` puts `2 writes (allow "git push")` in the footer, and this
    /// puts it on the thing you are pressing. A legible rule is one you read
    /// before you agree to it.
    #[test]
    fn screen_7a_reproduces_and_always_allow_writes_a_legible_rule() {
        let scratch = Scratch::new("permission");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let asked = grid_of(
            &editor.shown_on_grid(b":permit git push origin retry-backoff\r", "wants to run"),
        );
        // **The invocation, as it will run.** A permission ask that paraphrased
        // what it was asking about would be asking you to trust the paraphrase.
        assert!(
            shows(&asked, "$ git push origin retry-backoff"),
            "`7a` shows the exact invocation; grid was:\n{asked}"
        );
        assert!(
            shows(&asked, "allow once") && shows(&asked, "deny"),
            "with `7a`'s three answers; grid was:\n{asked}"
        );
        // **The rule, spelled before you press it.** The verb is the first two
        // words — `git push` from the whole command line — because a rule as
        // specific as the invocation would never match again.
        assert!(
            shows(&asked, "always allow git push"),
            "and the rule it would write; grid was:\n{asked}"
        );

        // `[2]` — always. The float closes and the strip says what happened.
        let granted = editor.press_until(b"2", "allowing git push");
        assert!(
            shows(&granted, "allowing git push from now on"),
            "the grant says what it did; session was: {granted}"
        );

        // **Takes effect next time**, which here is the next invocation of the
        // same verb: a rule that already permits it is not a question, and that
        // is checked on the path that would otherwise ask.
        editor.press_quietly(b":permit git push origin somewhere-else\r");
        let quiet = grid_of(&editor.shown_on_grid(b"j", "1:1"));
        assert!(
            !shows(&quiet, "wants to run"),
            "a rule that covers it asks nothing; grid was:\n{quiet}"
        );

        // **And it is readable.** `:allowed` is the audit — a permission
        // surface whose grants are invisible is one you stop trusting.
        let listed = grid_of(&editor.shown_on_grid(b":allowed\r", "always allowed"));
        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
        assert!(
            shows(&listed, "git push"),
            "the written rule reads back; grid was:\n{listed}"
        );

        // **And it reached disk**, which is what makes *next time* mean the
        // next session rather than the next keystroke. `T101` put
        // machine-written forms in the config home; `7a` still draws
        // `init.scm`, and the entry in `TASKS.md` records that.
        let persisted = scratch.persisted().join("persisted.scm");
        let written = fs::read_to_string(&persisted).unwrap_or_default();
        assert!(
            written.contains("(allow \"git push\")"),
            "the rule is a form a person can read; file was {written:?}"
        );
    }

    /// **`T060`: a workspace edit reaches a file no pane was showing.**
    ///
    /// The arm this queue owed a task that is not its own.
    /// `scripts/lint-action-arms.sh` has named `apply-workspace-edit` on every
    /// run for two windows: `T036` built the reading half and the applying half
    /// was blocked twice — nowhere to put the question, and files that are not
    /// open.
    ///
    /// **`OPEN-QUESTIONS.md` §47's rules, exercised.** The file edited here is
    /// never opened in a pane, so the buffer that receives the edit is one
    /// nothing is pointing at — the container `T088` shipped and this task
    /// inherited. It is dirty afterwards, and `:wall` is what writes it, which
    /// is the same two steps a rename you typed yourself would take.
    #[test]
    fn a_workspace_edit_reaches_a_file_no_pane_is_showing() {
        let scratch = Scratch::new("workspace-edit");
        let runtime = copy_layer(&scratch.path);
        let here = scratch.path.join("sample.txt");
        fs::write(&here, "alpha\n").expect("a fixture");
        let elsewhere = scratch.path.join("untouched.txt");
        fs::write(&elsewhere, "before\n").expect("a second fixture");

        let editor = Editor::open(&here, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // Line 1 columns 1..7 — `before` — replaced. Spans are line/column, and
        // the end is exclusive, which is what `1 7` says here.
        let form = format!(
            "(apply-workspace-edit! (list (hash \"path\" \"{}\" \"edits\" \
             (list (hash \"span\" (hash \"start\" (hash \"line\" 1 \"column\" 1) \
             \"end\" (hash \"line\" 1 \"column\" 7)) \"text\" \"after\")))))\r",
            elsewhere.display()
        );
        editor.press_until(form.as_bytes(), "⇒");

        // **It does not apply on the way in, and that is the rating doing its
        // job.** `apply-workspace-edit` is the one `Lsp` capability rated
        // `Ask`, and the rating is about the *action* rather than the door: a
        // rename arriving from Steel needs the same yes as one from a server.
        // So closing the REPL surfaces the question rather than the edit.
        let asked = grid_of(&editor.shown_on_grid(b"(close-repl!)\r", "needs input"));
        assert!(
            shows(&asked, "let it"),
            "the edit became a question; grid was:\n{asked}"
        );
        assert!(
            shows(&asked, "steel"),
            "which says who wants it; grid was:\n{asked}"
        );
        assert!(
            fs::read_to_string(&elsewhere).expect("the file is there") == "before\n",
            "and nothing was applied while it was being asked"
        );

        // `[1]` — let it. **Only now** does the edit run.
        editor.press_until(b"1", "answered 1");

        // **`:wall` writes it**, which is §47's second rule answered out loud:
        // a rename whose files were not written by `:wall` is the surprise, not
        // the safety.
        // **`:wall` says nothing when it succeeds** — its notice is the list of
        // buffers it *could not* write — so this is pressed quietly and settled
        // rather than waited on. A needle on a sentence the command does not
        // emit is a thirty-second deadline dressed as an assertion.
        editor.press_quietly(b":wall\r");
        editor.shown_on_grid(b"", "NORMAL");
        editor.leave_by(b"ZQ");

        let written = fs::read_to_string(&elsewhere).expect("the file is still there");
        assert!(
            written.contains("after"),
            "the edit reached a file no pane was showing; file was {written:?}"
        );
        assert!(
            !written.contains("before"),
            "and replaced what was there; file was {written:?}"
        );
    }

    /// **`T060`: `esc` defers, `]!` brings it back, and the `!` outlives both.**
    ///
    /// `4a`'s third way out — *"you answer when you get a chance, same
    /// philosophy as unseen"*. The queue has to converge for this to be a
    /// feature rather than a loop: without the deferral set, `esc` closes the
    /// float and the very next pass finds the same head still pending and
    /// raises it again.
    #[test]
    fn a_deferred_question_stays_queued_until_the_bracket_bang_recalls_it() {
        let scratch = Scratch::new("ask-defer");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.shown_on_grid(b":ask migrate the schema?|now|later\r", "needs input");

        // `esc later`. The float goes and does not come back on its own.
        // **Needled on `2:1`, which is false before the keys and true after.**
        // `1:1` was already on the grid, so waiting for it returned the frame
        // *before* esc had been processed and the assertion below read a stale
        // screen — a green test would have proved nothing and a red one, as
        // here, proves the wrong thing. `shown_on_grid` cannot wait for an
        // absence, so the wait is on what the keystroke makes true.
        // **Two writes, not one.** `\x1b` immediately followed by `j` in the
        // same write is the terminal's ESC-prefix ambiguity and arrives as
        // `<A-j>` — the editor said so on its own hint row, which is how this
        // was found.
        editor.press_quietly(b"\x1b");
        let after = grid_of(&editor.shown_on_grid(b"j", "2:1"));
        assert!(
            !shows(&after, "needs input"),
            "esc puts the question away; grid was:\n{after}"
        );
        // **And the `!` is still there**, because deferring is a fact about the
        // screen and the question is still pending. A flag that vanished when
        // you pushed something back would be the editor forgetting for you.
        assert!(
            after.lines().any(|row| row.contains('!')),
            "the strip still says a question is waiting; grid was:\n{after}"
        );

        // `]!`, pressed as one literal so `key_coverage.py` can see it.
        let recalled = grid_of(&editor.shown_on_grid(b"]!", "needs input"));
        assert!(
            shows(&recalled, "migrate the schema?"),
            "`]!` brings back what you pushed aside; grid was:\n{recalled}"
        );

        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T059`: a digit over a buffer is still vim's count prefix.**
    ///
    /// The other half of *"digits answer only while it is focused"*, and the
    /// half a test of the float cannot see. `2` with no question on screen has
    /// to reach the input machine unchanged — the arm that answers is gated on
    /// a float holding the screen *and* showing an ask, and a gate with one
    /// condition too few would quietly take every digit in normal mode.
    #[test]
    fn a_digit_with_no_question_on_screen_is_still_a_count() {
        let scratch = Scratch::new("count-prefix");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\ndelta\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // `3j` is three lines down. If the digit had been swallowed, `j` alone
        // would land on line 2.
        let moved = grid_of(&editor.shown_on_grid(b"3j", "4:1"));
        assert!(
            shows(&moved, "4:1"),
            "the count reached the motion; grid was:\n{moved}"
        );

        // **And a digit over a float that is not a question is not an answer
        // either.** This is the condition a test of the buffer cannot reach:
        // `Surface::Float` holds the screen, so a gate that only checked *which
        // surface* would try to answer `AskId(0)` and decline by name. Planted
        // and caught — with `Shell::asked` removed from the gate, the two tests
        // above both still passed.
        let over = editor.press_until(b":arch\r", "substrate");
        assert!(
            shows(&over, "substrate"),
            "a float that is not a question is open; session was: {over}"
        );
        // **Marked, then pressed, because the right answer draws nothing.** A
        // digit at a float that has no use for it is a key the editor ignores,
        // and `press_until` waiting on a frame that never comes is a
        // thirty-second deadline rather than a failure. `esc` is what redraws,
        // and the delta from before the digit is what carries the notice the
        // defect would have written.
        let mark = editor.mark();
        editor.press_quietly(b"1");
        editor.shown_on_grid(b"\x1b", "NORMAL");
        let pressed = editor.since(mark);
        editor.leave_by(b"ZQ");
        assert!(
            !shows(&pressed, "no option 1"),
            "and a digit at it is not an answer to anything; session was: {pressed}"
        );
    }

    /// **`T056`: `goto-location` opens the file at the position it names.**
    ///
    /// The verb the link is *for*, and the one thing about `T056` a keyboard
    /// can drive. Its three callers are a picker accept, a tool row click and
    /// an OSC 8 link — a terminal resolves the third itself, and none of them
    /// is a key — so it is recorded as `EMITTED` in
    /// `scripts/lint-capability-bindings.sh` and reached here through the door
    /// a pty *can* drive, which is the same argument `T053`'s block test makes.
    #[test]
    fn goto_location_opens_the_file_at_the_position_it_names() {
        let scratch = Scratch::new("goto-location");
        let runtime = copy_layer(&scratch.path);
        let here = scratch.path.join("sample.txt");
        fs::write(&here, "alpha\n").expect("a fixture");
        let there = scratch.path.join("elsewhere.txt");
        fs::write(&there, "one\ntwo\nthree\n").expect("a second fixture");

        let editor = Editor::open(&here, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(goto-location! \"{}\" (hash \"line\" 3 \"column\" 1) (key/focused-pane))\r",
            there.display()
        );
        // **No `(close-repl!)` after this, and that is not an omission.**
        // Opening a file replaces what the pane holds, so the REPL is gone by
        // the time the form has answered — and a `(close-repl!)` typed after it
        // is eleven normal-mode keys, of which `o` is *open a line* and the
        // rest are insert. The editor then sits in INSERT, where `ZQ` is two
        // more characters, and the test hangs in `leave_by`'s untimed
        // `child.wait()`. Measured: one stuck child per run.
        //
        // **The position is the point of the verb**, not garnish on naming a
        // file — `open-file`'s `at` is optional and this one's is what a jump
        // link carries. So the readout is the assertion: landing at the top of
        // the right file would be the wrong answer drawn convincingly.
        // **Needled on the file's contents, not on its name.** The REPL echoes
        // the form as you type it, path and all, so waiting for
        // `elsewhere.txt` matches the *echo* on the frame before anything has
        // opened — and then `ZQ` goes into a REPL that is still up, which is
        // another stuck child. `three` is on line 3 of the target and nowhere
        // in the form.
        editor.press_until(form.as_bytes(), "#ok");
        // **`q`, the REPL's own footer key, and one keystroke exactly.** The
        // float is over the pane the file lands in, so nothing is visible until
        // it is gone — and `(close-repl!)` typed here is eleven normal-mode
        // keys the moment the float closes under them, of which `o` is *open a
        // line*. That left the editor in INSERT, where `ZQ` is two more
        // characters and `leave_by`'s untimed `child.wait()` never returns.
        // Measured: one stuck child per run.
        let landed = grid_of(&editor.shown_on_grid(b"q", "three"));
        editor.leave_by(b"ZQ");
        assert!(
            shows(&landed, "elsewhere.txt"),
            "the named file is open; grid was:\n{landed}"
        );
        assert!(
            shows(&landed, "3:1"),
            "and the cursor is on the line it named; grid was:\n{landed}"
        );
    }

    /// **`T055`: claude's prose wraps rather than stopping at the pane edge.**
    ///
    /// The task's guardrail is *"the plain-text path must stay readable with
    /// the gate off"*, and this is the default build, so this is that path. A
    /// paragraph wider than the pane used to be one row written with
    /// `set_stringn` — cut at the edge, with the sentence's end simply gone —
    /// under a comment claiming it was *"Wrapped, not truncated"*. The comment
    /// had been there since `T054` and nothing measured it.
    ///
    /// **Needled on the paragraph's last word.** A wrap check that looked at
    /// row widths would pass against a renderer that truncated every row; the
    /// only thing that distinguishes wrapping from clipping is whether the tail
    /// is on the screen at all.
    #[test]
    fn claudes_prose_wraps_in_the_transcript_rather_than_clipping() {
        let scratch = Scratch::new("prose-wrap");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} turn\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // Wider than the 120-column harness, so the tail is off the row unless
        // something wrapped it. The toy agent answers `heard: <prompt>`, which
        // makes the prose row this sentence plus a prefix.
        let paragraph = "adding a retry policy struct and a generic backoff              helper then wiring the whole fetch layer through it so that every              call site inherits the same jittered schedule finis";
        assert!(
            paragraph.len() > usize::from(SCREEN.ws_col),
            "the fixture has to overrun the pane or it proves nothing"
        );
        editor.press_quietly(format!(":claude {paragraph}\r").as_bytes());

        let read = grid_of(&editor.shown_on_grid(b" t", "jittered"));
        assert!(
            shows(&read, "finis"),
            "the paragraph's last word is on screen; grid was:\n{read}"
        );
        // And it is on a *different row* from its beginning, which is what
        // wrapping means and what a wider pane would hide.
        let first = read
            .lines()
            .position(|row| row.contains("adding a retry"))
            .expect("the paragraph starts somewhere");
        let last = read
            .lines()
            .position(|row| row.contains("finis"))
            .expect("and ends somewhere");
        assert!(
            last > first,
            "the tail is on a later row than the head; grid was:\n{read}"
        );

        editor.leave_by(b"ZQ");
    }

    /// **`T057`: screen `7b` reproduces — the transcript shows the seam.**
    ///
    /// `7b`'s caption is *"acp gone mid-turn · editing never blocks · the
    /// transcript shows the seam honestly"*, and the third clause is what this
    /// asserts. The agent takes a prompt, says one thing, and dies before any
    /// stop reason — the `drop` fixture mode, which exists for this and only
    /// this, because `deaf` and `linger` both die with no turn running and so
    /// can produce no seam at all.
    ///
    /// **The seam is not typed.** `:seam` is the manual form for a pause or a
    /// resume, which nothing observes; a connection going while a turn is open
    /// is observed by the loop, so the row appears without a keystroke asking
    /// for it. A transcript whose honesty depended on the reader remembering to
    /// request it would not be the thing the caption claims.
    #[test]
    fn the_transcript_shows_screen_7bs_seam_when_the_agent_dies_mid_turn() {
        let scratch = Scratch::new("seam-7b");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} drop\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // The prompt starts a turn; the agent answers once and goes. Both the
        // prose and the drop arrive with nothing else pressed.
        editor.shown_on_grid(b":claude fix the retry loop\r", "session lost");

        // **`SPC t`, pressed as one literal.** `scripts/key_coverage.py` reads
        // the bytes a test sends, so a leader binding split across two presses
        // is a binding nothing proves anybody can reach.
        let seam = grid_of(&editor.shown_on_grid(b" t", "connection lost"));
        assert!(
            shows(&seam, "connection lost mid-turn"),
            "`7b`'s seam is in the transcript; grid was:\n{seam}"
        );
        assert!(
            shows(&seam, "disk state preserved"),
            "and says what survived it; grid was:\n{seam}"
        );
        assert!(
            shows(&seam, "turn may be incomplete"),
            "and does not overclaim the turn; grid was:\n{seam}"
        );
        // The header keeps saying whose stream this was. Blanking it would make
        // a transcript you are reading *after* a drop anonymous, which is the
        // one moment the header is load-bearing.
        assert!(
            shows(&seam, "acp · disconnected"),
            "the header follows the session; grid was:\n{seam}"
        );
        // `7b`'s footer offers the session back. Spelled in full, not as the
        // mockup's `:ca` — Design Language §6 and the drawing disagree, and
        // `OPEN-QUESTIONS.md` §55 records it rather than folding it in.
        //
        // **Read off the footer's own row, not off the grid.** A bare needle on
        // `reattach` went green with the hint deleted, because the statusline
        // three rows below already says `✕ session lost — :reattach` — the
        // remedy was on screen and the footer was empty, and the assertion
        // could not tell those apart. `new session` is the `:start-session` hint and
        // nothing else on the screen draws it, so it locates the strip; the row
        // it is on is then the row that has to carry the rest.
        let footer = seam
            .lines()
            .find(|row| row.contains("start a new one"))
            .unwrap_or_else(|| panic!("`7b`'s footer is not on screen; grid was:\n{seam}"));
        assert!(
            footer.contains("reattach") && footer.contains("close"),
            "and offers the remedy beside the way out; footer row was: {footer:?}"
        );

        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T057`: screen `2d` reproduces — the dashboard opened mid-task.**
    ///
    /// *"what you see attaching to a repo where claude's been busy · state, not
    /// splash"*. Same surface as `7d`, three differences: the session row names
    /// what is running, an `unseen` row appears because there is something to
    /// be unseen, and the footer leads with `]u` instead of `:start-session` — you did not
    /// come back to start a session, you came back to the work.
    ///
    /// **Two of `2d`'s five rows are absent and that is the honest rendering.**
    /// `vcs jj · trunk@a4f2 · clean` is `vcs-status` (`T071`) and
    /// `last cargo test ✓ 34 passed` is the timeline (`T073`); both answer
    /// `NotYetImplemented` today, so `runtime/dashboard.scm` draws neither. A
    /// row reading `vcs —` would be the file claiming to have looked.
    #[test]
    fn the_dashboard_reproduces_screen_2d_when_claude_has_been_busy() {
        let scratch = Scratch::new("dashboard-2d");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("retry.rs");
        fs::write(&file, "one\ntwo\nthree\nfour\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let started = format!(":start-session python3 {} mute\r", agent.display());
        editor.shown_on_grid(started.as_bytes(), "claude attached");

        // Claude has been busy: one review block over two spans in this file,
        // declared through the registry the same way `T053`'s test does.
        editor.press_until(b":repl\r", "steel");
        // A record per span (`T066`, §59): where it is, and what it replaced.
        let form = format!(
            "(declare-review-block! \"retry logic\" (list (hash \"path\" \"{}\" \
             \"spans\" (list (hash \"span\" (hash \"start\" (hash \"line\" 1 \"column\" 1) \
             \"end\" (hash \"line\" 2 \"column\" 1))) \
             (hash \"span\" (hash \"start\" (hash \"line\" 3 \"column\" 1) \
             \"end\" (hash \"line\" 4 \"column\" 1)))) \
             \"annotation\" \"the meat\")) \"the whole change\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "the whole change");
        editor.press_until(b"(close-repl!)\r", "review ready");

        let busy = grid_of(&editor.shown_on_grid(b":dashboard\r", "unseen"));
        assert!(
            shows(&busy, "2 regions in 1 file"),
            "`2d`'s unseen row counts what arrived; grid was:\n{busy}"
        );
        assert!(
            shows(&busy, "retry logic, review ready"),
            "and names the block it belongs to; grid was:\n{busy}"
        );
        assert!(
            !shows(&busy, "none running"),
            "the session row is not `7d`'s any more; grid was:\n{busy}"
        );
        assert!(
            shows(&busy, "next unseen"),
            "and `2d`'s footer leads with the work; grid was:\n{busy}"
        );

        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T057`: the editor stays usable through a mid-turn drop.**
    ///
    /// The task's emphasis — *"editing never blocks on session trouble"* — and
    /// the half of it a terminal can answer. The agent dies while a turn is
    /// running; the statusline says so, and the buffer still takes keys.
    ///
    /// **Not asserted by watching the session state alone.** `T051`'s test
    /// already proves the strip follows a drop with no keystroke. What is
    /// proven here is the other side: that the *editing* path is untouched by
    /// it, which is only visible by editing.
    #[test]
    fn editing_survives_a_session_dying_mid_turn() {
        let scratch = Scratch::new("survives-drop");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(set-option! \"agent-command\" \"python3 {} linger\")\r",
            agent.display()
        );
        editor.press_until(form.as_bytes(), "()");
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        shown(&editor, "claude idle");

        // The agent goes on its own schedule. Nothing is pressed to make it.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if grid_of(&editor.screen()).contains("session lost") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the session never dropped; grid was: {}",
                grid_of(&editor.screen())
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        // **And the editor is an editor.** `o` opens a line, `x` types into it,
        // `esc` leaves insert — none of which has anything to do with a
        // session, which is exactly the claim.
        editor.press_quietly(b"ox\x1b");
        let edited = shown(&editor, "x");
        editor.leave_by(b"ZQ");
        assert!(
            shows(&edited, "alpha"),
            "the buffer is still there after the drop; session was: {edited}"
        );
    }

    /// **`T058`: screen `1c` reproduces from a keystroke.**
    ///
    /// The task's *Done when* — *"pressing `:` in the running binary raises the
    /// line, anchor chip included"* — and `1c`'s caption is the mechanism:
    /// *"visual-select, hit the prompt — file & range ride along
    /// automatically"*.
    ///
    /// **`esc` before `ZQ`, and that is not politeness.** `leave_by` does a
    /// `child.wait()` with no timeout, so a `ZQ` typed while the prompt is open
    /// becomes *text on the prompt line* and the child never exits — the test
    /// hangs in its own teardown, long after its assertions have passed. That
    /// cost `docs/OPEN-QUESTIONS.md` §53 an entry blaming the harness for a bug
    /// in the test that was reading it.
    #[test]
    fn the_prompt_line_reproduces_screen_1c_from_a_keystroke() {
        let scratch = Scratch::new("prompt-anchor");
        let runtime = copy_layer(&scratch.path);
        // **`.txt`, not `.rs`.** A Rust file attaches a grammar and a language
        // server, and both draw frames of their own — `press` asserts one frame
        // per key, so the first press after `V` fails on a surplus that has
        // nothing to do with the prompt. `1c` shows `src/retry.rs`; what this
        // test is about is the chip naming *whatever* file rode along.
        let file = scratch.path.join("retry.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // Lines 1 to 3 — `1c`'s three lines, started where the cursor already
        // is. **`V` after a motion draws two frames** and `press` asserts one
        // per key, so the sequence is the one
        // `v_line_selects_whole_lines` already proves against this harness:
        // select first, move after.
        editor.press(b"V");
        editor.press(b"j");
        editor.press(b"j");
        assert!(
            shows(&editor.tail(), "V-LINE"),
            "the selection is live; row was: {}",
            editor.tail()
        );

        // **The keystroke.** `:` in visual mode carries the selection along —
        // the binding is `open-prompt` with `key/at-selection`, and resolving
        // it to a range *when the prompt opens* is what makes the chip name
        // something that stays true after the selection is gone.
        editor.press(b":");
        // `tail()` is the last *non-empty* row, which is the prompt's — `1c`
        // puts it above the statusline, so the two rows are read separately
        // below.
        let row = editor.screen().line(SCREEN.ws_row - 2);

        // No trailing space in the needle: `⚓` is double-width, so the grid
        // carries a continuation cell after it rather than the space the
        // string has.
        assert!(
            row.contains('⚓'),
            "the anchor chip is on screen; row was: {row:?}"
        );
        assert!(
            row.contains("retry.txt:1–3"),
            "and it names the file and the range that rode along; row was: {row:?}"
        );

        // **The statusline is still there, on its own row.** `1c` draws the
        // prompt *below* it — asserted as rows rather than as text, which is
        // the claim the layout actually makes.
        let screen = editor.screen();
        let last = screen.line(SCREEN.ws_row - 1);
        assert!(
            !last.contains('⚓') && !last.trim().is_empty(),
            "the statusline keeps its own row under the prompt; row was: {last:?}"
        );

        // **Two `esc`s.** The first closes the prompt and leaves the selection
        // live; the second leaves visual mode. `ZQ` typed in either of those
        // states is not a quit, and `leave_by` waits on the child with no
        // timeout — see this test's own note above.
        editor.press(b"\x1b");
        editor.press(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T058`: `SPC c p` and `SPC c s` raise the same line.**
    ///
    /// `3c`'s `+claude · prompt · steer`, both `open-prompt` with
    /// `kind: claude`. They named `T058` on the notice row until the line was
    /// built; this is what replaced that row in the deferred table.
    #[test]
    fn the_claude_prompt_raises_from_a_keystroke() {
        let scratch = Scratch::new("claude-prompt");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // One literal, three keys: `press` waits for one frame per byte, and
        // `scripts/key_coverage.py` reads the notation `SPC c p` out of the
        // sequence — pressed as three separate calls it is three keys the lint
        // cannot see as one binding.
        editor.press(b" cp");
        let raised = editor.tail();
        assert!(
            !shows(&raised, "T058"),
            "the line is built, so nothing names the task; row was: {raised:?}"
        );
        assert!(
            raised.contains(':'),
            "and the prompt is up; row was: {raised:?}"
        );

        editor.press(b"\x1b");
        editor.press(b" cs");
        let steered = editor.tail();
        assert!(
            !shows(&steered, "T058"),
            "steer raises the same line; row was: {steered:?}"
        );

        editor.press(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`T065`'s acceptance: `8b` is navigable.**
    ///
    /// Declare a block, `:review` it, and the float draws the grouped tree with
    /// its counts. Then `za` folds the highlighted group and `j` moves the
    /// highlight — which is the whole of what *navigable* means for a screen
    /// whose own footer reads `za fold · s seen · S group seen · q`.
    ///
    /// **The rows are a live query behind a snapshot float.** The float is
    /// composed once, at `:review`; the counts on it are rebuilt every frame
    /// off the same store the gutter reads. So the numbers here are the store's
    /// answer and not a copy taken at open time.
    #[test]
    fn a_review_block_opens_as_a_navigable_tree() {
        let scratch = Scratch::new("review-tree");
        let runtime = copy_layer(&scratch.path);
        // **Two files in one directory**, because one is not a group and a
        // fixture with one cannot tell a count of what a row *holds* from a
        // count of what it *drew*. A planted `files = 1` passed against a
        // one-file fixture and is caught by this one.
        let dir = scratch.path.join("src");
        fs::create_dir_all(&dir).expect("a directory");
        let file = dir.join("fetch.txt");
        let other = dir.join("retry.txt");
        for path in [&file, &other] {
            fs::write(
                path,
                "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
            )
            .expect("a fixture");
        }

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // **A changed span carries what it replaced** (`T066`, §59). `was`
        // absent is a pure insertion, which `4b` draws as `+` lines and no `−`.
        let span = |from: u32, to: u32, was: &str| {
            // **Absent, not `#false`.** An `Option<String>` on the wire is a
            // field that is *there* or *is not*; `#false` is a boolean and the
            // decoder says so — `expected text, found bool`.
            let replaced = if was.is_empty() {
                String::new()
            } else {
                format!(" \"was\" \"{was}\"")
            };
            format!(
                "(hash \"span\" (hash \"start\" (hash \"line\" {from} \"column\" 1) \
                 \"end\" (hash \"line\" {to} \"column\" 1)){replaced})"
            )
        };
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" (list {} {}) \"annotation\" \"the meat\") \
                   (hash \"path\" \"{}\" \"spans\" (list {}) \"annotation\" \"mechanical\")) \
             \"the whole change\")\r",
            file.display(),
            span(1, 2, ""),
            span(5, 6, "was five"),
            other.display(),
            span(3, 4, ""),
        );
        editor.press_until(form.as_bytes(), "the whole change");
        editor.press_until(b"(close-repl!)\r", "review ready");

        // **Bare `:review` means the newest block.** A person who has just read
        // `review ready · retry logic` has one block in mind and no id for it.
        let open = editor.shown_on_grid(b":review\r", "review — ✻ retry logic");
        let body = whole(&open);
        assert!(
            body.contains("2 file(s) · 3 region(s)"),
            "the header counts what the store holds:\n{body}"
        );
        // **Three levels, which is what `8b` is.** A directory row with the
        // count under it, the file nested inside carrying claude's note, and the
        // hunk ranges under that. The counts are the store's, this instant.
        // **`3` and `2`, and neither is the number of rows drawn.** The
        // directory holds three unseen hunks across two files; a row that
        // counted what it drew would say `2 files` by luck and `●2` by mistake.
        assert!(
            body.contains("●3 unseen · 2 files"),
            "the directory row counts what is under it:\n{body}"
        );
        // **Relative to the row above it.** `src/` is drawn once, on the
        // directory row; a file row repeating it would be the path twice.
        assert!(
            body.contains("▾   fetch.txt  ●2  the meat"),
            "the file row is nested, chipped and annotated:\n{body}"
        );
        assert!(
            body.contains("retry.txt  ●1  mechanical"),
            "and its neighbour is under the same directory:\n{body}"
        );
        // **`@@ 1`, not `@@ 1–2`.** A span from line 1 to line 2 column 1 is
        // half-open and covers one line — `Span`'s own reading — and `4b`
        // writes a one-line hunk as `@@ 4`.
        assert!(
            body.contains("@@ 1") && body.contains("@@ 5"),
            "and the hunks are under the file:\n{body}"
        );

        // **`T066`, §59: the two sides.** The after-side is the file's text
        // *now*, read live; the before-side is what claude said it replaced.
        assert!(
            body.contains("− was five"),
            "what claude replaced is a minus:\n{body}"
        );
        assert!(
            body.contains("+ five"),
            "and what is there now is a plus:\n{body}"
        );
        // **A hunk that replaced nothing draws no minus**, which `4b` draws
        // too: its `@@ 4` is one `+` line and no `−` at all. So *"claude did
        // not say"* and *"it removed nothing"* look identical, which is the
        // truthful reading rather than a fallback.
        let inserted = body
            .lines()
            .skip_while(|row| !row.contains("@@ 1"))
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        assert!(
            inserted.contains("+ one") && !inserted.contains('−'),
            "a pure insertion is additions only: {inserted:?}"
        );
        // `8b`'s footer, spelled whole — Design Language §6.
        assert!(body.contains("za"), "the footer teaches the keys:\n{body}");
        assert!(body.contains("fold"), "{body}");

        // **`za` folds the highlighted group.** The `z` is swallowed and the
        // `a` folds — two-key sequences belong to the input machine and the
        // machine is not running over a float.
        let folded = whole(&editor.shown_on_grid(b"za", "▸ "));
        // Row 0 is the directory — the highlight starts there and `za` folds it.
        assert!(folded.contains("▸ "), "the arrow closed:\n{folded}");

        // And it toggles back, which is what makes it a fold rather than a
        // hide.
        let open_again = whole(&editor.shown_on_grid(b"za", "▾ "));
        assert!(open_again.contains("▾ "), "{open_again}");

        // **`:annotate` writes claude's sentence onto the row you are on**, and
        // takes no id — `8b`'s *"mechanical"* against *"the meat"*, typed by a
        // person rather than declared by an agent. That the ex line opens at all
        // over this float is the assertion underneath: a surface that owned
        // every key would be the one place its own two commands could not be
        // typed.
        //
        // **Pressed quietly and then read**, not `shown_on_grid` — the needle
        // would be the text of the command itself, which is on the ex line the
        // moment it is typed and true a frame before it runs. `press_quietly`
        // settles, so the grid after it is the grid the command produced.
        // Onto a *file* row, so the highlight goes there first — a directory is
        // the host's grouping and carries no store id to annotate.
        editor.press_quietly(b"j");
        editor.press_quietly(b":annotate handler signatures\r");
        let annotated = whole(&editor.screen());
        assert!(
            annotated.contains("fetch.txt  ●2  handler signatures"),
            "the file row carries the new note:\n{annotated}"
        );
        assert!(
            !annotated.contains("the meat"),
            "and replaces the old one rather than appending:\n{annotated}"
        );
        assert!(
            annotated.contains("retry.txt  ●1  mechanical"),
            "and its neighbour is untouched:\n{annotated}"
        );

        // **`:grouping flat` keeps the files and drops the scaffolding** —
        // `8d`'s answer at 80 columns. The group row goes; what it held stays.
        editor.press_quietly(b":grouping flat\r");
        let flat = whole(&editor.screen());
        assert!(
            !flat.contains("●3 unseen · 2 files"),
            "the directory row is gone:\n{flat}"
        );
        assert!(
            flat.contains("handler signatures") && flat.contains("mechanical"),
            "and both files it held are still drawn, with their notes:\n{flat}"
        );
        assert!(
            flat.contains("src/fetch.txt"),
            "spelled whole, because there is no row above them now:\n{flat}"
        );
        assert!(
            flat.contains("review — ✻ retry logic"),
            "the header is not a group row:\n{flat}"
        );

        // **`q` closes it and the buffer takes keys again**, which is one claim
        // and needs one keystroke to prove: `G` moves the cursor, which only
        // the buffer does.
        //
        // Not `press_until(b"q", "NORMAL")` — the first version was that, and
        // it hung for thirty-four seconds. `press_until` reads the *byte
        // delta*, and closing a float redraws the rows it covered while leaving
        // the statusline exactly as it was, so `NORMAL` is on screen and not in
        // what arrived. The needle has to be something the keystroke *changes*.
        editor.press_quietly(b"q");
        let closed = whole(&editor.shown_on_grid(b"G", "11:1"));
        assert!(!closed.contains("za fold"), "the float is gone:\n{closed}");

        // **Every surface closed before `ZQ`** — `leave_by` does a
        // `child.wait()` with no timeout.
        editor.leave_by(b"ZQ");
    }

    /// **`4b`'s after-side is the buffer, not the disk copy** (`T066`, §59).
    ///
    /// A hunk with an unsaved edit under it shows what is on screen, not what
    /// was last written — the same ruling `AppHost::parse` makes for anchors
    /// applied here, where the two can actually differ.
    #[test]
    fn a_hunks_after_side_is_the_open_buffer_not_the_disk_copy() {
        let scratch = Scratch::new("review-live");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1)))))) \
             \"one region\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "one region");
        editor.press_until(b"(close-repl!)\r", "review ready");

        // Edit line 2 without saving — the buffer and the disk copy now
        // disagree, and only one of them is what the review is *of*.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"jcc");
        editor.press_quietly(b"UNSAVED\x1b");

        let opened = whole(&editor.shown_on_grid(b":review\r", "review — ✻ retry logic"));
        assert!(
            opened.contains("+ UNSAVED"),
            "the hunk shows the unsaved edit:\n{opened}"
        );
        assert!(
            !opened.contains("+ two"),
            "and not the line that is only on disk:\n{opened}"
        );

        editor.press_quietly(b"q");
        editor.leave_by(b"ZQ");
    }

    /// **One surface, one session — `gh` while a review is open closes it.**
    ///
    /// A probe found this before a test did: keys `review_key` does not name
    /// fall through to the buffer's own keymap even while a float holds the
    /// screen, so `gh` fires with the review still open. If `shell.review`
    /// stayed set, the next `s` would read the review's selected row instead of
    /// the peek's hunk — which is `4b`'s `S`, not `2b`'s `s`, answering a key
    /// pressed on `2b`'s screen.
    #[test]
    fn opening_a_peek_while_a_review_is_open_replaces_it_not_layers_it() {
        let scratch = Scratch::new("one-session");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\nsix\nseven\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // **Two hunks, on purpose.** With one, marking through the review's
        // stale session and marking through the peek land on the same region —
        // indistinguishable. `s` on the review's `selected` row (a directory,
        // row 0) marks *every* hunk under it; `s` on the peek marks only the
        // one it opened. The first version of this test had one hunk and could
        // not tell the two paths apart — a bug that fails to fail is worse
        // than no test.
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1))) \
                         (hash \"span\" (hash \"start\" (hash \"line\" 5 \"column\" 1) \
                                          \"end\" (hash \"line\" 6 \"column\" 1)))))) \
             \"two regions\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "two regions");
        editor.press_until(b"(close-repl!)\r", "review ready");

        // Cursor onto the *second* hunk (line 5) before the review opens — the
        // review's own navigation does not move the buffer's cursor, and its
        // `selected` starts at row 0, the directory — a different row entirely.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"4j");
        // **`shown_on_grid`, not a quiet press and a grid read.** Opening this
        // float runs scheme — `review/body` and `review/footer` compose through
        // the VM — and `press_quietly` settles the *bytes*, which is not the
        // same as waiting for the surface to exist. Reading `screen()` straight
        // after is a race that this machine wins and CI loses.
        //
        // **It lost three times before anyone looked.** The runs for `T066`,
        // `T067` and `T068` were all red on exactly this assertion while
        // `just gate` was green locally each time, because CI shards nextest
        // across four slower workers and the float had not drawn yet. A local
        // gate is not a CI result, and three commits went out on that
        // assumption.
        let opened = whole(&editor.shown_on_grid(b":review\r", "]] next file"));
        assert!(
            opened.contains("]] next file"),
            "the review is the screen right now, selected at row 0:\n{opened}"
        );

        // `gh`, with the review still recorded as open.
        let peeked = whole(&editor.shown_on_grid(b"gh", "claude changed"));
        assert!(
            !peeked.contains("]] next file"),
            "the review's footer is gone — it was replaced, not layered:\n{peeked}"
        );
        assert!(
            peeked.contains("@@ 5"),
            "and it opened at the cursor's hunk, not the review's row 0:\n{peeked}"
        );

        // **The proof that matters.** If `shell.review` survived, `s` would hit
        // `review_key`'s guard first — row 0 is the directory, and `s` on an
        // unwidened directory row marks every hunk under it, both of them. A
        // correctly cleared session marks only the peek's one hunk.
        editor.press_quietly(b"s");
        editor.press_quietly(b"q");
        editor.press_until(b":repl\r", "steel");
        let read = editor.press_until(
            b"(map (lambda (h) (hash-ref h \"seen\")) (hunks 0))\r",
            "(#f #t)",
        );
        assert!(
            shows(&read, "(#f #t)"),
            "only the second hunk — the peek's — is marked:\n{read}"
        );

        editor.press_until(b"(close-repl!)\r", "NORMAL");
        editor.leave_by(b"ZQ");
    }

    /// **`5c`: one list of everything claude said, and unread derives.**
    ///
    /// `T067`'s two claims in one press. The list is a *merge* — a declared
    /// review block and a posted note are two different stores and one screen —
    /// and `CP-8a` asks that unread come from seen-state rather than a copy, so
    /// the proof is that **marking the block's hunks seen makes its inbox row
    /// read** with nothing subscribing to anything.
    #[test]
    fn the_inbox_merges_what_claude_said_and_unread_derives_from_seen_state() {
        let scratch = Scratch::new("inbox");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1)))))) \
             \"one region\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "one region");
        // The receipt is the note's id — `⇒ 2`, which is `InboxSource::Note(0)`
        // encoded. Needled on it because "steel" was already on screen from the
        // banner and would have matched a frame before this ran.
        editor.press_until(
            b"(notify! \"info\" \"bumped tokio to 1.41 for sleep jitter\")\r",
            "⇒ 2",
        );
        editor.press_until(b"(close-repl!)\r", "review ready");

        // **`5c`'s strip says how much is waiting, before the float is open.**
        // Two unread — the block and the note — counted from the same three
        // sources the float merges, so the two cannot disagree.
        //
        // `j` first, and it is not decoration: a notice borrows the *whole*
        // statusline row (vim's placement, and `Chrome`'s), and the frame this
        // arrives on is still showing `review ready · retry logic`. A harmless
        // key gives the row back before anything on it can be read.
        let strip = whole(&editor.shown_on_grid(b"j", "inbox 2 unread"));
        assert!(strip.contains("inbox 2 unread"), "{strip}");

        // **Both stores, one list.**
        let opened = whole(&editor.shown_on_grid(b":inbox\r", "everything claude said"));
        assert!(
            opened.contains("✻ review ready"),
            "the block is a row:\n{opened}"
        );
        assert!(
            opened.contains("· note") && opened.contains("bumped tokio to 1.41"),
            "and so is the note:\n{opened}"
        );
        // Newest first — the note arrived after the block.
        let note_at = opened.find("bumped tokio").expect("the note is drawn");
        let block_at = opened.find("review ready").expect("the block is drawn");
        assert!(
            note_at < block_at,
            "newest first, which is the only order an inbox has:\n{opened}"
        );
        // Neither has been read.
        assert!(!opened.contains("seen ✓"), "nothing is read yet:\n{opened}");
        // `5c`'s footer, spelled whole — Design Language §6.
        assert!(
            opened.contains("open") && opened.contains("mark seen"),
            "{opened}"
        );

        // **The `CP-8a` proof.** Mark the block's one hunk seen from the
        // *buffer* — `gsih`, nothing to do with the inbox — and its inbox row
        // must go read on its own, because the row's `unread` is computed from
        // the same regions the gutter draws.
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b"gg");
        editor.press_quietly(b"j");
        editor.press_quietly(b"gsih");
        let after = whole(&editor.shown_on_grid(b":inbox\r", "everything claude said"));
        assert!(
            after.contains("seen ✓"),
            "the block's row went read with nothing subscribing:\n{after}"
        );
        // And the note did **not** — it has a bit of its own and nobody touched
        // it. Without this the assertion above would pass on a screen that had
        // marked everything.
        let note_row = after
            .lines()
            .find(|row| row.contains("bumped tokio"))
            .unwrap_or_default();
        assert!(
            !note_row.contains("seen ✓"),
            "the note is untouched: {note_row:?}"
        );
        // And the strip agrees — one left, not two.
        editor.press_quietly(b"\x1b");
        let after_strip = whole(&editor.shown_on_grid(b"j", "inbox 1 unread"));
        assert!(
            after_strip.contains("inbox 1 unread"),
            "the strip counts the same merge the float does:\n{after_strip}"
        );

        // **And a note is marked read through the same verb**, against an
        // `inbox-item` target — the one inbox row that is not a region, taken
        // before the scope machinery because a note has no file and no span.
        //
        // This half is also what makes the strip's count *testable*: with only
        // the block ever marked, `every row` and `every unread row` are the
        // same number, and a planted `notes.len()` passed. Marking the note is
        // what separates them.
        editor.press_quietly(b"\x1b");
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            b"(mark-seen! (hash \"kind\" \"inbox-item\" \"id\" 2))\r",
            "⇒ 1",
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        let read = whole(&editor.shown_on_grid(b":inbox\r", "everything claude said"));
        let note_now = read
            .lines()
            .find(|row| row.contains("bumped tokio"))
            .unwrap_or_default();
        assert!(
            note_now.contains("seen ✓"),
            "the note reads as seen: {note_now:?}"
        );
        editor.press_quietly(b"\x1b");
        // Nothing unread left, so §11's segment draws nothing at all rather
        // than `inbox 0 unread` — the same rule `unseen` follows beside it.
        editor.press_quietly(b"j");
        let empty = whole(&editor.screen());
        // **`unread`, not `inbox`.** The scratch directory is named after this
        // test, so the file path on the strip contains the word `inbox` — and
        // the first version of this assertion failed on its own fixture's name
        // while the segment was correctly absent.
        assert!(
            !empty.contains("unread"),
            "an empty count draws nothing:\n{empty}"
        );

        editor.leave_by(b"ZQ");
    }

    /// **`3a`: your comment and claude's reply, as virtual text under the
    /// region.**
    ///
    /// The mockup's own three claims, in one press each: the exchange hangs
    /// under the anchored line as `┊` rows, the two sides are told apart by
    /// *which door they came through* rather than by a field, and the
    /// statusline counts the conversation you are still in.
    #[test]
    fn a_thread_draws_both_sides_under_the_line_it_is_anchored_to() {
        let scratch = Scratch::new("thread");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("retry.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\nsix\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `:comment` anchors at the cursor — line 3.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"jj");
        let mine =
            whole(&editor.shown_on_grid(b":comment collapse these arms\r", "collapse these arms"));
        // **`you · `, not `⚓ you`.** `⚓` is double-width; ratatui writes it and
        // marks the cell it covers as skipped, and `Screen::replayed` has no
        // model for that — so the *test grid* keeps whatever was in that cell
        // before and reads `⚓r you`. The byte stream is correct and so is the
        // terminal. Needling past the covered cell asserts the row without
        // asserting the harness's own gap.
        assert!(mine.contains("you \u{b7} "), "your side names you:\n{mine}");

        // **Claude's side arrives the same way yours does — same verb, same
        // store, different door.** The actor is *which applier ran* and not a
        // parameter, which is the one thing §7 rules out being settable: the
        // machine can only track claude if the two sides cannot be forged.
        //
        // Through the REPL rather than `:reply`, and that is the assertion: a
        // repl call is a **door** call, so this proves `AppHost`'s arm labels
        // it `claude` while `:reply` below proves the loop's labels it `you`.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            "(reply-to-thread! 0 \"collapsed - error carried in `last`\")\r".as_bytes(),
            "\u{21d2} ",
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        let both = whole(&editor.screen());
        assert!(
            both.contains("claude \u{b7} "),
            "and claude's names him, from the door he came through:\n{both}"
        );
        assert!(
            both.contains("collapsed - error carried in"),
            "with what he said:\n{both}"
        );
        // Both rows hang off the same anchor, on the `┊` rail `T032` built.
        assert!(
            both.matches('\u{250a}').count() >= 2,
            "two rows, one rail:\n{both}"
        );

        // **§3's row 20 is a tint *and* an undercurl, and the tint is what
        // says the anchor is a whole line.** The exchange draws either way —
        // rows hang at `span.start` regardless — so a point anchor is
        // invisible in the rows and visible here: a zero-width span produces
        // no marks at all (`Tints::marks` needs `start < end`), and this row
        // would come back untinted. A planted point anchor passed every other
        // assertion in this test.
        //
        // Row 2 is line 3, the anchored one: rows are 0-based and the buffer
        // starts at the top of the screen.
        let anchored = editor.screen().tinted(2);
        assert!(
            !anchored.is_empty(),
            "the anchored line carries §3's row tint: {:?}",
            editor.screen().line(2)
        );

        // **`1 thread`, and it is the conversation you are still in.** A
        // harmless key first: a notice borrows the whole statusline row.
        let strip = whole(&editor.shown_on_grid(b"j", "1 thread"));
        assert!(strip.contains("1 thread"), "{strip}");

        // **`:reply` is the keyboard's half, and it labels the reply *yours*.**
        // Same verb, same store, other applier — the pair that makes the actor
        // a fact about the door rather than about a parameter.
        let mine_again = whole(&editor.shown_on_grid(b":reply 0 good catch\r", "good catch"));
        assert_eq!(
            mine_again.matches("you \u{b7} ").count(),
            2,
            "two of the three rows are yours:\n{mine_again}"
        );
        assert_eq!(
            mine_again.matches("claude \u{b7} ").count(),
            1,
            "and one is his:\n{mine_again}"
        );

        // **Resolving takes it off the count and leaves the exchange.** `3a`'s
        // own subtitle is that the record of *why* a line looks the way it does
        // outlives the conversation — so the rows stay and the claim on your
        // attention goes.
        editor.press_quietly(b":resolve 0\r");
        editor.press_quietly(b"k");
        let resolved = whole(&editor.screen());
        assert!(
            !resolved.contains("1 thread"),
            "resolved stops claiming your attention:\n{resolved}"
        );
        assert!(
            resolved.contains("collapse these arms"),
            "and the exchange is still there:\n{resolved}"
        );

        // **`:broadcast` puts one message on every matching line** — `6d`'s
        // `:g/TODO/c` without the `/pattern/` grammar. Two lines contain `o`
        // in this fixture's first three (`one`, `two`), so this is a claim
        // about *many* anchors rather than one.
        editor.press_quietly(b":broadcast four sweep this\r");
        let swept = whole(&editor.screen());
        assert!(
            swept.contains("sweep this"),
            "the broadcast landed on its match:\n{swept}"
        );

        // **`:unthread` is the verb that loses something, and it is the only
        // one of the four that does.** `:resolve` above kept the rows; this
        // takes them.
        editor.press_quietly(b":unthread 0\r");
        editor.press_quietly(b"j");
        let gone = whole(&editor.screen());
        assert!(
            !gone.contains("collapse these arms"),
            "the exchange is deleted, not resolved:\n{gone}"
        );

        editor.leave_by(b"ZQ");
    }

    /// **Three kinds, one clock: newest first across blocks and notes alike.**
    ///
    /// **This fixture's shape is the whole test, and it took two tries to get
    /// right.** `store::Shared::arrivals` exists so blocks and notes can be
    /// ordered against each other; `BlockId` and a note's own counter mint
    /// independently and are not comparable. Proving that needs a fixture where
    /// the two disagree *and* the disagreement changes the drawn order:
    ///
    /// * With **one** block, `BlockId(0)` and arrival `0` are the same number.
    ///   A planted `block.id.0` drew an identical screen.
    /// * With **one note between two blocks**, the wrong key gives the note and
    ///   the second block the same value — and a stable sort happens to break
    ///   that tie the right way. Still identical.
    /// * With **two notes**, the keys finally diverge: correct puts the second
    ///   block on top, wrong puts the newest note there.
    ///
    /// So the assertion below is on the *first* row, which is the only position
    /// the two orderings disagree about.
    #[test]
    fn the_inbox_orders_blocks_and_notes_on_one_shared_clock() {
        let scratch = Scratch::new("inbox-order");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let block = |title: &str, line: u32, note: &str| {
            format!(
                "(declare-review-block! \"{title}\" \
                 (list (hash \"path\" \"{}\" \"spans\" \
                       (list (hash \"span\" (hash \"start\" (hash \"line\" {line} \"column\" 1) \
                                              \"end\" (hash \"line\" {} \"column\" 1)))))) \
                 \"{note}\")\r",
                file.display(),
                line + 1,
            )
        };
        // Oldest first, so the drawn order must come out reversed. Two notes
        // between the blocks — see the doc above for why one is not enough.
        editor.press_until(block("retry logic", 2, "first").as_bytes(), "first");
        editor.press_until(
            b"(notify! \"info\" \"bumped tokio to 1.41\")\r",
            "\u{21d2} ",
        );
        editor.press_until(
            b"(notify! \"info\" \"pinned the toolchain\")\r",
            "\u{21d2} ",
        );
        editor.press_until(block("ws reconnect", 4, "later").as_bytes(), "later");
        editor.press_until(b"(close-repl!)\r", "review ready");

        let all = whole(&editor.shown_on_grid(b":inbox\r", "ws reconnect"));
        let newest = all.find("ws reconnect").expect("the second block is drawn");
        let note_two = all.find("pinned the toolchain").expect("the newer note");
        let note_one = all.find("bumped tokio").expect("the older note");
        let oldest = all.find("retry logic").expect("the first block is drawn");
        assert!(
            newest < note_two,
            "the block declared last is the first row — the position the two \
             orderings disagree about:\n{all}"
        );
        assert!(
            note_two < note_one && note_one < oldest,
            "and the rest fall in arrival order behind it:\n{all}"
        );

        editor.press_quietly(b"\x1b");
        editor.leave_by(b"ZQ");
    }

    /// **`j`/`k`/`s`/`↵` inside `5c`, aimed at the row under the highlight.**
    ///
    /// `view/spans` is a snapshot — `layer.surface`'s own doc — so there is no
    /// live door into it the way `Resources::diff` gives the review float;
    /// navigating recomposes the inbox with a new `selected` on every key. The
    /// proof this test is for is that the *right* row moves: `s` on the
    /// highlighted row and not on row 0 regardless of where the highlight is,
    /// and `↵` on a block opens the review it names.
    #[test]
    fn the_inbox_navigates_and_opens_what_the_highlighted_row_names() {
        let scratch = Scratch::new("inbox-nav");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1)))))) \
             \"one region\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "one region");
        editor.press_until(
            b"(notify! \"info\" \"bumped tokio to 1.41 for sleep jitter\")\r",
            "⇒ 2",
        );
        editor.press_until(b"(close-repl!)\r", "review ready");

        // Row 0 is the note (newest); row 1 is the block. `j` moves onto the
        // block without touching the note.
        let opened = whole(&editor.shown_on_grid(b":inbox\r", "everything claude said"));
        assert!(
            opened.contains("bumped tokio"),
            "the note is drawn:\n{opened}"
        );
        editor.press_quietly(b"j");
        editor.press_quietly(b"s");
        let marked = whole(&editor.screen());
        let block_row = marked
            .lines()
            .find(|row| row.contains("review ready"))
            .unwrap_or_default();
        let note_row = marked
            .lines()
            .find(|row| row.contains("bumped tokio"))
            .unwrap_or_default();
        assert!(
            block_row.contains("seen ✓"),
            "`s` acted on the highlighted row, the block: {block_row:?}"
        );
        assert!(
            !note_row.contains("seen ✓"),
            "and not on row 0, the note: {note_row:?}"
        );

        // **`↵` on the block opens the review it names.** `k` back onto it —
        // `s` did not move the highlight — then `Enter`.
        editor.press_quietly(b"\r");
        let review = whole(&editor.screen());
        assert!(
            review.contains("]] next file"),
            "the review opened, replacing the inbox:\n{review}"
        );

        editor.press_quietly(b"q");
        editor.leave_by(b"ZQ");
    }

    /// **`2b`'s acceptance: `gh` opens a peek without leaving the buffer.**
    ///
    /// Screens `4b` and `2b` are one function apart — `peek_vm` reuses
    /// `hunk_lines` exactly, so what proves `4b`'s two sides proves `2b`'s. What
    /// this test is *for* is the two claims `4b` cannot make: the peek opens at
    /// the hunk under the *cursor*, and `s` inside it marks that one hunk and
    /// closes nothing.
    #[test]
    fn gh_opens_a_hunk_peek_at_the_cursor_without_leaving_the_buffer() {
        let scratch = Scratch::new("hunk-peek");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(
            &file,
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        )
        .expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1)) \
                                \"was\" \"was two\") \
                         (hash \"span\" (hash \"start\" (hash \"line\" 5 \"column\" 1) \
                                          \"end\" (hash \"line\" 6 \"column\" 1)))))) \
             \"two regions\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "two regions");
        editor.press_until(b"(close-repl!)\r", "review ready");

        // Cursor onto line 2 — the *first* declared span, not the second.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"j");
        let opened = whole(&editor.shown_on_grid(b"gh", "− was two"));
        assert!(
            opened.contains("+ two"),
            "the after-side is line 2, read live:\n{opened}"
        );
        assert!(
            !opened.contains("+ five") && !opened.contains("@@ 5"),
            "and not the other hunk — this one is at the cursor:\n{opened}"
        );
        // **`2b`'s float is three lines tall and has no header** — `T063`'s
        // documented empty case, spent here.
        assert!(
            !opened.contains("review — ✻"),
            "a peek is not a review:\n{opened}"
        );
        assert!(
            opened.contains("s") && opened.contains("mark seen"),
            "{opened}"
        );
        assert!(opened.contains("close"), "{opened}");

        // `s` marks this hunk and does **not** close the float — `4b`'s `s`
        // doesn't either, and a screen that closed on its own primary verb
        // would make "mark the next one" cost a second `gh`.
        editor.press_quietly(b"s");
        let marked = whole(&editor.screen());
        assert!(
            marked.contains("seen"),
            "the peek shows it is now seen:\n{marked}"
        );
        assert!(
            marked.contains("− was two"),
            "and the float is still open:\n{marked}"
        );

        // **The buffer was never left.** `q` closes the peek and the cursor is
        // still where `gh` found it — line 2, column 1 — which `G` moving away
        // from it proves by contrast.
        editor.press_quietly(b"q");
        let closed = whole(&editor.shown_on_grid(b"G", "10:1"));
        assert!(
            !closed.contains("mark seen"),
            "the float is gone:\n{closed}"
        );
        editor.leave_by(b"ZQ");
    }

    /// **`4b`'s four keys: `]]`, `za`, `s` and `S`.**
    ///
    /// One review, four presses, and each one is a claim about *scope*: `]]`
    /// lands on the next file, `za` collapses one hunk, `s` marks the row you
    /// are on, and `S` widens by one level to what that row is inside. The
    /// counts on the screen are the store's, so they are also the assertion.
    #[test]
    fn the_review_footers_keys_each_act_on_their_own_scope() {
        let scratch = Scratch::new("review-keys");
        let runtime = copy_layer(&scratch.path);
        let dir = scratch.path.join("src");
        fs::create_dir_all(&dir).expect("a directory");
        let first = dir.join("fetch.txt");
        let second = dir.join("retry.txt");
        for path in [&first, &second] {
            fs::write(
                path,
                "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
            )
            .expect("a fixture");
        }

        let editor = Editor::open(&first, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let span = |from: u32, to: u32| {
            format!(
                "(hash \"span\" (hash \"start\" (hash \"line\" {from} \"column\" 1) \
                 \"end\" (hash \"line\" {to} \"column\" 1)))"
            )
        };
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" (list {} {})) \
                   (hash \"path\" \"{}\" \"spans\" (list {}))) \
             \"three regions\")\r",
            first.display(),
            span(1, 2),
            span(5, 6),
            second.display(),
            span(3, 4),
        );
        editor.press_until(form.as_bytes(), "three regions");
        editor.press_until(b"(close-repl!)\r", "review ready");
        editor.press_quietly(b":review\r");

        // Row 0 is the directory. `]]` from there lands on the *first* file.
        editor.press_quietly(b"]]");
        // **Down onto the file's one hunk, and `s` there marks *that hunk*,
        // not the file it belongs to.** `fetch.txt` has one region declared
        // here — `span(1, 2)` — so this is the row directly under the file,
        // and the assertion below is only true if `s` read the hunk row and
        // not the nearer file row above it.
        editor.press_quietly(b"j");
        editor.press_quietly(b"s");
        let one = whole(&editor.screen());
        assert!(
            one.contains("fetch.txt  ●1"),
            "`s` on a hunk row marks one hunk, leaving the file's other one:\n{one}"
        );
        assert!(
            one.contains("retry.txt  ●1"),
            "and not its neighbour's:\n{one}"
        );
        assert!(
            one.contains("●2 unseen · 2 files"),
            "so the directory has two left, not one:\n{one}"
        );

        // `s` on the *file* row now marks what is left of it — the second hunk.
        editor.press_quietly(b"k");
        editor.press_quietly(b"s");
        let both = whole(&editor.screen());
        assert!(
            both.contains("fetch.txt  ●0"),
            "`s` on a file row marks the rest of its hunks:\n{both}"
        );

        // **`]]` again lands on the second file and does not wrap.** A review is
        // a list you work down; `]]` off the end that came back to the top would
        // lose your place in the one screen whose job is keeping it.
        editor.press_quietly(b"]]");
        editor.press_quietly(b"S");
        let all = whole(&editor.screen());
        // **`seen ✓ · 2 files`, not `●0 unseen`.** A group with nothing left to
        // read says so rather than counting zero — `8b`'s own rule, and the
        // reason `File::unseen` is an `Option`. The first version of this
        // assertion asked for the count and the screen was right.
        assert!(
            all.contains("seen ✓ · 2 files"),
            "`S` widens to what the row is inside:\n{all}"
        );
        assert!(
            all.contains("3 seen ✓"),
            "and the header agrees, because both read the same store:\n{all}"
        );

        // **A seen hunk folds itself**, which is §11 applied to your own
        // progress rather than to the change's size — `4b` draws
        // `@@ 9–14 · tests  ⋯ folded · 6 lines  seen ✓`.
        assert!(
            all.contains("⋯ folded"),
            "what you have read collapses:\n{all}"
        );

        editor.press_quietly(b"q");
        let closed = whole(&editor.shown_on_grid(b"G", "11:1"));
        assert!(
            !closed.contains("]] next file"),
            "the float is gone:\n{closed}"
        );
        editor.leave_by(b"ZQ");
    }

    /// **`T064`'s acceptance, through the keyboard.** Marking one hunk seen
    /// leaves the rest unseen.
    ///
    /// `gsih` — *mark inner hunk seen* — is the sentence the operator ruling of
    /// 2026-08-12 was written for, and until this task `h` parsed and selected
    /// nothing, so the whole phrase was a no-op that looked like a binding. The
    /// keymap's own comment said so, naming a task.
    ///
    /// **Counted through `unseen-count` rather than off the statusline**, which
    /// is not squeamishness about rendering: `2 unseen` is the first rung of the
    /// shed ladder and becomes `●2` the moment the row is tight, so an assertion
    /// on the words is an assertion about how wide the terminal happened to be.
    /// The count is the claim; the chip is a drawing of it.
    #[test]
    fn marking_one_hunk_seen_leaves_the_rest_of_the_block_unseen() {
        let scratch = Scratch::new("hunk-seen");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        // Ten lines, so three spans have room to be separate.
        fs::write(
            &file,
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        )
        .expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // A record per span (`T066`, §59) — where it is, and what it replaced.
        // `was` is absent throughout: this test is about seen-state, not sides.
        let span = |from: u32, to: u32| {
            format!(
                "(hash \"span\" (hash \"start\" (hash \"line\" {from} \"column\" 1) \
                 \"end\" (hash \"line\" {to} \"column\" 1)))"
            )
        };
        let form = format!(
            "(declare-review-block! \"retry logic\" (list (hash \"path\" \"{}\" \
             \"spans\" (list {} {} {}) \"annotation\" \"the meat\")) \"three regions\")\r",
            file.display(),
            span(1, 2),
            span(5, 6),
            span(9, 10),
        );
        editor.press_until(form.as_bytes(), "three regions");

        let before = editor.press_until(b"(unseen-count)\r", "3");
        assert!(shows(&before, "3"), "three declared; session was: {before}");

        editor.press_until(b"(close-repl!)\r", "review ready");

        // Line 5 is inside the second span and inside nothing else.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"4j");
        // **One literal.** `key_coverage.py` reads the bytes a test presses and
        // matches them against the bound sequences, so `gsih` split across two
        // calls is four keys it cannot see as a binding. `press_quietly`
        // because `g` opens a which-key popup on the way and `press` asserts
        // one frame per key byte.
        editor.press_quietly(b"gsih");

        let after = editor.press_until(b":repl\r", "steel");
        assert!(!after.is_empty());
        // Two left, and the one that went is the one the cursor was in.
        let counted = editor.press_until(b"(unseen-count)\r", "2");
        assert!(
            shows(&counted, "2"),
            "one hunk marked, two still unseen; session was: {counted}"
        );

        // **And the hunk rows say *which*** — the query `T064` answers, mapped
        // down to the one field this is about. A bare `(hunks 0)` prints three
        // records with their spans nested inside, which is three hundred
        // characters through an eighty-column repl pane: the needle would be
        // waiting for text the screen cannot show, and the answer would be
        // right.
        //
        // `#f` and `#t`, not `#false` and `#true` — steel *reads* the long
        // spelling and *prints* the short one, and the first version of this
        // needle waited thirty-five seconds for a string the repl was never
        // going to draw.
        let listed = editor.press_until(
            b"(map (lambda (h) (hash-ref h \"seen\")) (hunks 0))\r",
            "(#f #t #f)",
        );
        assert!(
            shows(&listed, "(#f #t #f)"),
            "the middle hunk and only it; session was: {listed}"
        );

        editor.press_until(b"(close-repl!)\r", "NORMAL");
        editor.leave_by(b"ZQ");
    }

    /// **`T053`: a declared block becomes markers and a notification.**
    ///
    /// The task's *Done when* is two things and they land in two places: the
    /// spans become §7 unseen regions in the store — the same ones the gutter
    /// draws and the statusline counts, because `declare_block` goes through
    /// the same `declare` `declare-regions` does — and the block itself becomes
    /// a sentence on the notice row.
    ///
    /// **The notification is why `Intent::Say` exists.** A door answers its
    /// *caller*: `Receipt::note` reaches the shell that ran the verb or the
    /// agent that called the tool. A review block is news to the person at the
    /// terminal, who made no call at all, so it needed a way from the far side
    /// of the VM onto §6's notice row.
    ///
    /// Declared through the REPL rather than over MCP because the point is that
    /// it is *routed through the registry* — one capability, three doors — and
    /// the Steel one is the one a pty can drive.
    #[test]
    fn a_declared_review_block_becomes_markers_and_a_notice() {
        let scratch = Scratch::new("review-block");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("retry.rs");
        fs::write(&file, "one\ntwo\nthree\nfour\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // Two spans in one file, with claude's own annotation for the group.
        // **`spans` carries a record per span now** (`T066`, §59): where the
        // change is, and what it replaced. `was` is absent on both here, which
        // is a pure insertion — the shape `4b`'s own `@@ 4` has.
        let form = format!(
            "(declare-review-block! \"retry logic\" (list (hash \"path\" \"{}\" \
             \"spans\" (list (hash \"span\" (hash \"start\" (hash \"line\" 1 \"column\" 1) \
             \"end\" (hash \"line\" 2 \"column\" 1))) \
             (hash \"span\" (hash \"start\" (hash \"line\" 3 \"column\" 1) \
             \"end\" (hash \"line\" 4 \"column\" 1)))) \
             \"annotation\" \"the meat\")) \"the whole change\")\r",
            file.display()
        );
        // The receipt is the block's id with claude's own annotation as the
        // note — `Receipt::note` reaching the caller, which is exactly the
        // channel a *person* at the terminal is not on. Hence the notice below.
        editor.press_until(form.as_bytes(), "the whole change");

        // The query sees it, with its groups and their annotations. Needled on
        // the *group's* note rather than the block's title: the record prints
        // in field order and the title sits past the right edge of an 80-column
        // repl pane, so a needle on it would be waiting for a string the screen
        // cannot show.
        let listed = editor.press_until(b"(review-blocks)\r", "the meat");
        assert!(
            shows(&listed, "the whole change"),
            "and the block's own annotation; session was: {listed}"
        );

        // **Not `"NORMAL"`.** A notice borrows the whole statusline row — vim's
        // own placement, and `Chrome`'s — so the mode chip is not on screen on
        // the frame this is waiting for. Waiting for the notice itself is both
        // the correct needle and the assertion.
        let said = editor.press_until(b"(close-repl!)\r", "review ready");

        // **The notification**, in `1b`'s own words — the seam sentence, on the
        // notice row, reaching the person who made no call. It was parked while
        // the REPL owned the frame and lands on the first frame that has a
        // notice row.
        //
        // **This assertion is the whole reason `Intent::Say` is testable.** The
        // first version of this test asserted markers and the query only, and
        // went green with the notification deleted — a test named for a notice
        // that never looked at one.
        assert!(
            shows(&said, "review ready · retry logic"),
            "the notice names the block; session was: {said}"
        );
        assert!(
            shows(&said, "1 file(s), 2 region(s)"),
            "and counts what landed; session was: {said}"
        );

        // **The markers.** Two spans declared, so the statusline counts two —
        // the same store the gutter draws from.
        //
        // One harmless key first: a notice holds the statusline row until the
        // next keystroke, so the counter has nowhere to be drawn until the
        // sentence above has been read.
        editor.press_quietly(b"0");
        // **Either spelling, and CI is what taught this.** The counter's *word*
        // is `SHED_ORDER`'s first rung — the very first thing the strip gives up
        // — so whether it reads `2 unseen` or `●2` depends on how much room the
        // rest of the row wants. On a runner where the language-server chip is
        // drawn and the temp path is long, the word goes and the count stays;
        // locally it fit, and the test asserted the wide spelling for months.
        // The claim is *two regions became unseen*, and both spellings make it.
        let counted = shown_on_grid_any(&editor, &["2 unseen", "●2"]);
        assert!(
            counted.contains("2 unseen") || counted.contains("●2"),
            "the block's spans became unseen regions; session was: {counted}"
        );
        editor.leave_by(b"ZQ");
    }

    /// **`T054`: screen `1b` reproduces from a keystroke.**
    ///
    /// The task's *Done when* names the keystroke specifically — *"the binding
    /// that opens the pane opens it in the running binary"* — because `T016`
    /// and `Density::Help` both shipped a surface a golden frame proved and no
    /// key could reach. So this presses `SPC t` and reads the screen.
    ///
    /// **A split, not a takeover.** `1b` keeps the code above the transcript,
    /// which is why the binding is `split-pane` with `kind: transcript` rather
    /// than `set-pane-content` — one call, because the capability takes what the
    /// new pane holds. `:transcript` is the other one, and it is the *same*
    /// capability the row says it is: it turns this pane into the transcript
    /// and `:transcript buffer` puts it back.
    ///
    /// Every row of `1b` that a toy agent can produce is asserted: the header,
    /// the `❯` prompt line, claude's prose, and a tool row. The `+42 −0` counts
    /// are not — ACP does not carry them, `1b` draws them from a diff, and
    /// `T063` is the task that supplies one.
    #[test]
    fn the_transcript_pane_reproduces_screen_1b_from_a_keystroke() {
        let scratch = Scratch::new("transcript");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(set-option! \"agent-command\" \"python3 {} turn\")\r",
            agent.display()
        );
        editor.press_until(form.as_bytes(), "()");
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        shown(&editor, "claude idle");

        // A turn, so there is something to transcribe.
        editor.press_quietly(b":claude add retry with backoff\r");
        shown(&editor, "claude idle");

        // **The keystroke.** `SPC t` splits below and focuses the new pane.
        let opened = grid_of(&editor.after(b" t"));
        editor.leave_by(b"ZQ");

        // `1b`'s header — what is running, the protocol, the session's tail.
        assert!(
            opened.contains("claude code · acp"),
            "the transcript header is `1b`'s; grid was: {opened}"
        );
        // The prompt line, and it is what was actually asked.
        assert!(
            opened.contains("❯ add retry with backoff"),
            "the prompt line carries what you said; grid was: {opened}"
        );
        // Claude's prose.
        assert!(
            opened.contains("heard: add retry with backoff"),
            "claude's prose is on the pane; grid was: {opened}"
        );
        // A tool row — `1b`'s `▸ edit  src/retry.rs`.
        assert!(
            opened.contains("edit") && opened.contains("src/retry.rs"),
            "the tool call became a row; grid was: {opened}"
        );
        // And the code is still there, because this is a split.
        assert!(
            opened.contains("alpha"),
            "`1b` keeps the buffer above the transcript; grid was: {opened}"
        );
    }

    /// **`T051`: the statusline is never stale — it changes with no keystroke.**
    ///
    /// The half of *"always present and truthful"* (§5) that a keystroke-driven
    /// test cannot reach. The editor draws when something tells it to, and a
    /// session dropping tells nobody: without `Session`'s wake, the strip would
    /// go on saying `claude idle` for a session that had died, until the user
    /// happened to press a key. That is *correct and stale*, which is exactly
    /// what §5 forbids.
    ///
    /// **Nothing is pressed after the session attaches**, and the fixture mode
    /// is what makes that mean something. The first version used `deaf`, which
    /// exits the instant it has answered `initialize` — inside the keystrokes
    /// that set it up. The editor had therefore already redrawn before the
    /// quiet phase began, and the test **passed with the wake removed**.
    /// `linger` answers the handshake and exits two seconds later, so the drop
    /// lands while nobody is typing, which is the only arrangement in which the
    /// poll below is evidence.
    #[test]
    fn the_statusline_follows_a_session_that_dies_with_no_keystroke() {
        let scratch = Scratch::new("stale");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(set-option! \"agent-command\" \"python3 {} linger\")\r",
            agent.display()
        );
        editor.press_until(form.as_bytes(), "()");
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        // Attached first, so what the poll below waits for is the *drop* and
        // not the attach.
        shown(&editor, "claude idle");

        // From here on, **no key is pressed**. The screen has to move anyway.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let seen = grid_of(&editor.screen());
            if seen.contains("session lost") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the editor never noticed the session go, with no keystroke to \
                 make it look; grid was: {seen}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        editor.leave_by(b"ZQ");
    }

    /// **`T051`: the `session` query answers what the statusline drew.**
    ///
    /// §5's *"one enum rendered identically everywhere it appears"* is only
    /// true if there is one derivation, so the loop composes the state once per
    /// frame and publishes **that value** — the query and the strip cannot
    /// disagree, because there is nothing for them to disagree with.
    #[test]
    fn the_session_query_answers_the_state_the_strip_drew() {
        let scratch = Scratch::new("session-query");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // Before any session: the query answers, and answers `none`.
        let quiet = editor.press_until(b"(session)\r", "none");
        assert!(
            shows(&quiet, "none"),
            "a session query with no session answers `none` rather than \
             raising; session was: {quiet}"
        );

        let form = format!(
            "(set-option! \"agent-command\" \"python3 {} turn\")\r",
            agent.display()
        );
        editor.press_until(form.as_bytes(), "()");

        // Attached: the query says `idle` and names the agent's own session.
        let attached = editor.press_until(b"(session)\r", "idle");
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        assert!(
            shows(&attached, "toy-session-1"),
            "the query carries the agent's own session id — `5d`'s adoption \
             picker is what that is for; session was: {attached}"
        );

        // And it is the same word the strip drew.
        let strip = shown(&editor, "claude idle");
        editor.leave_by(b"ZQ");
        assert!(
            shows(&strip, "claude idle"),
            "the strip and the query disagree; session was: {strip}"
        );
    }

    /// **`T052`: `apply-edits` is a batch, and one `u` is all of it.**
    ///
    /// The clause `T052`'s *Done when* singles out — *"a batch applied as one
    /// undo group, which is the shape an agent writes through"* — pressed in
    /// the running binary. An agent that rewrote two call sites is one keystroke
    /// away from before it, not two.
    ///
    /// **The undo half of this passes for free, and that was measured rather
    /// than assumed.** Deleting the arm's `begin`/`commit` pair and re-running
    /// left it green: the group boundary is the input machine's
    /// (`History::CommitUndoGroup`, per `Timeline::close`), so every edit made
    /// while applying one Action is already one group. The assertion stays
    /// because it is the task's acceptance — a future change to where the
    /// boundary is drawn has to keep it true — but it is not evidence about
    /// this arm, and a test whose failure mode nobody has seen is worth saying
    /// so about.
    ///
    /// **It is also the test that proves scheme reaches the rope.** The first
    /// version of this failed on `#refused · not built yet — T052 builds it`
    /// with the arm already written: `AppHost::apply` is the VM's applier and
    /// `Editing::act` is the loop's, and nothing joined them, so no
    /// buffer-domain capability had ever been reachable from `:repl`.
    /// `Intent::Act` is that join.
    ///
    /// **The two edits are on one line, and that is the whole of what makes
    /// the ordering testable.** A `Span` is line-and-column and is resolved
    /// against the document as it stands, so two edits on *different* lines
    /// survive either order — the first draft of this test used exactly that
    /// pair and **passed with the sort planted front-to-back**. Replacing five
    /// columns with three moves everything after it on that row, so a
    /// front-to-back walk writes the second edit three columns late: a wrong
    /// *result* rather than a crash, which is why this asserts the text.
    #[test]
    fn apply_edits_is_one_undo_group() {
        let scratch = Scratch::new("apply-edits");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha bravo\ncharlie\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // `alpha bravo` becomes `one two` — two edits on one row, declared in
        // reading order, which is the order an agent produces them in and the
        // order that is wrong to apply in. `alpha` is columns 1..6 and `bravo`
        // is columns 7..12 **of the line as it was read**; once `alpha` is
        // three characters, `bravo` is at 5..10 and a front-to-back walk
        // writes over the wrong three columns.
        let form = concat!(
            "(apply-edits! (list ",
            "(hash \"span\" (hash \"start\" (hash \"line\" 1 \"column\" 1) ",
            "\"end\" (hash \"line\" 1 \"column\" 6)) \"text\" \"one\") ",
            "(hash \"span\" (hash \"start\" (hash \"line\" 1 \"column\" 7) ",
            "\"end\" (hash \"line\" 1 \"column\" 12)) \"text\" \"two\")))\r"
        );
        // `#ok`, not `#done` with a value: `AppHost::apply` answers the
        // scheme caller the moment it posts the intent — see `Intent::Act` on
        // why the real outcome arrives on the notice row instead.
        editor.press_until(form.as_bytes(), "#ok");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        let edited = grid_of(&editor.after(b"0"));
        assert!(
            edited.contains("one two"),
            "both edits landed, in the right columns — a front-to-back walk \
             writes the second one three columns late; grid was: {edited}"
        );
        assert!(
            !edited.contains("alpha") && !edited.contains("bravo"),
            "and none of the old row survives; grid was: {edited}"
        );

        // **One `u`.** Two undo steps would leave half the row rewritten.
        let undone = grid_of(&editor.after(b"u"));
        editor.quit();
        assert!(
            undone.contains("alpha bravo"),
            "one `u` undid the whole batch; grid was: {undone}"
        );
        assert!(
            !undone.contains("one") && !undone.contains("two"),
            "and not just half of it — the batch is one undo group; grid was: \
             {undone}"
        );
    }

    /// **`T050`: a session attaches and a turn completes — in the running
    /// binary.**
    ///
    /// `phosphor-agent`'s own tests prove the client; this proves the *editor*
    /// has one. Three things only a terminal can answer: that `agent-command`
    /// spawns an agent, that §5's session segment is the client's report rather
    /// than the `SessionState::None` the statusline hardcoded until now, and
    /// that `:claude` starts a turn a person can watch.
    ///
    /// **The `slow` fixture mode exists for the middle frame.** `turn` answers
    /// in microseconds, so `idle`, `working` and `idle` would all land inside
    /// one frame and the test could assert none of them. Two seconds is long
    /// enough to photograph and short enough not to dominate the suite.
    ///
    /// The agent is the same `toy_acp_agent.py` the client's tests drive,
    /// reached across the crate boundary rather than copied: two spellings of
    /// one fixture is how the two would come to disagree about the protocol.
    #[test]
    fn a_session_attaches_and_a_turn_completes_in_the_editor() {
        let scratch = Scratch::new("acp-session");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");
        let agent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../phosphor-agent/tests/fixtures/toy_acp_agent.py")
            .canonicalize()
            .expect("the toy agent is in the sibling crate");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");

        // **No session before the option names one.** §5's segment is
        // *"always present and truthful"*, and the truth here is that there is
        // nothing to be present about.
        let before = statusline(&editor.screen());
        assert!(
            !before.contains("claude"),
            "something claimed a session before one was asked for: {before}"
        );

        // `(set-option! …)` — the whole attach door, which is why a live agent
        // is one REPL line away rather than a task away.
        let form = format!(
            "(set-option! \"agent-command\" \"python3 {} slow\")\r",
            agent.display()
        );
        editor.press_until(form.as_bytes(), "()");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // Attached and between turns is `idle`.
        let idle = shown(&editor, "claude idle");
        assert!(
            shows(&idle, "claude idle"),
            "the session never attached; session was: {idle}"
        );

        // **A turn, watched.** `:claude` is `send-message`, and the fixture
        // holds the stop reason for two seconds so `working` is a frame that
        // exists.
        editor.press_quietly(b":claude what is 2 + 2?\r");
        let working = shown(&editor, "claude working");
        assert!(
            shows(&working, "claude working"),
            "the turn never showed as working; session was: {working}"
        );

        // And it ends. Polled rather than read once: the counter going back to
        // `idle` is a *removal* of the working segment, so the frame that does
        // it can land just after a read.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut after = statusline(&editor.screen());
        while after.contains("working") {
            assert!(
                Instant::now() < deadline,
                "the turn never ended; statusline was: {after}"
            );
            std::thread::sleep(Duration::from_millis(20));
            after = statusline(&editor.screen());
        }
        assert!(
            after.contains("idle"),
            "the turn ended into something other than idle: {after}"
        );
        editor.leave_by(b"ZQ");
    }

    /// **An anchored message is refused by name rather than sent without its
    /// anchor**, because claude answering about the wrong thing with nothing on
    /// screen to say the range went missing is the worse failure.
    ///
    /// The refusal is `T058`'s, which is the task that builds the line the
    /// anchor comes from.
    #[test]
    fn a_message_with_an_anchor_names_the_task_that_owes_it() {
        let scratch = Scratch::new("acp-anchor");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let said = editor.press_until(
            b"(send-message! \"hello\" (list (hash \"kind\" \"cursor\")))\r",
            "T058",
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        editor.quit();

        assert!(
            shows(&said, "T058"),
            "an anchored message was not refused by name; session was: {said}"
        );
    }

    /// **`T089`: the tab bar appears on the second pane and never on the
    /// first, and its counts are the store's.**
    ///
    /// Both halves of the task's *Done when*, in the running binary, because
    /// both are claims a unit test cannot make. `crate::tab_bar`'s own tests
    /// prove the strip reads the way §5 draws it; what only a terminal can
    /// answer is whether the strip is *there* — the condition lives in
    /// `Geometry::take_tab_bar` and in `compose_tabs`, one of which spends a
    /// row and the other of which fills it, and a widget test can see neither.
    ///
    /// **The absence check is about the top *row*, not about the text**, and
    /// the first draft of this test got that wrong. It asked only whether the
    /// word `panes` was anywhere on screen, so a `Geometry::take_tab_bar` that
    /// spent the row at one pane **passed it** — `compose_tabs` answers
    /// `Node::Empty` there, so nothing draws into the row and the strip is
    /// invisible while the buffer is silently a line shorter. Planted and
    /// measured; the guard is in two places because it does two things, and a
    /// test that watches only the ink can see only one of them.
    ///
    /// So the assertion is that row zero *is* the buffer at one pane and *is*
    /// the strip at two. `contains` and not `shows`: `shows` is a two-thirds
    /// fuzzy match, so `!shows(frame, …)` is nearly always false — the trap
    /// `the_window_keys_split_focus_resize_and_close` paid for first.
    ///
    /// `●2` is the needle for the counts because nothing else on this frame
    /// draws it: the state column's marker is `▎` (`gutter::MARKER`) and the
    /// statusline spells its own count `2 unseen` at this width, contracting to
    /// `●2` only after §11's ladder has taken the word — which needs a much
    /// narrower terminal than this one.
    #[test]
    fn the_tab_bar_appears_on_the_second_pane_and_never_on_the_first() {
        let scratch = Scratch::new("tab-bar");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Two regions claude declared, so every tab of this file has a count to
        // carry and the store has somewhere to be tracked from.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(1, 1), (3, 3)]).as_bytes(), "landed=2");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // **One pane: no strip at all**, and the row it would have taken is
        // still the buffer's. `0` is a harmless key — the cursor is already at
        // the start of the line — pressed only to have a frame to read.
        let alone = editor.after(b"0");
        let top = alone.line(0);
        let alone = grid_of(&alone);
        assert!(
            top.contains("alpha"),
            "the top row is not the buffer's first line, so §5's strip took it \
             at one pane; row was: {top:?}"
        );
        assert!(
            !alone.contains("panes"),
            "§5's strip appeared at one pane; grid was: {alone}"
        );

        // **Two panes: the strip, one tab each, both carrying the store's
        // count.** `<C-w> v` is one notation and both bytes go in one literal —
        // `key_coverage.py` spells it `\x17v`.
        let split = editor.after(b"\x17v");
        let top = split.line(0);
        let split = grid_of(&split);
        assert!(
            top.contains("2 panes"),
            "the strip is not on the top row of the second pane's frame; row \
             was: {top:?}"
        );
        assert!(
            twice(&split, "sample.txt ●2"),
            "each pane did not get a tab carrying the store's two unseen \
             regions; grid was: {split}"
        );

        // **The count tracks the store rather than the frame it first drew
        // on.** `gs` is the mark-seen operator (`s` is vim's substitute and
        // `CP-3` asks that it stay so); `gsj` marks the region under the cursor
        // seen, and both tabs have to say so — one store, two readers.
        //
        // Polled rather than read once, for `gs_marks_a_region_seen`'s reason:
        // the counter dropping is a *removal*, so there is no new text to
        // settle on and the frame that drops it can land just after a read.
        editor.press_quietly(b"gsj");
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut after = grid_of(&editor.screen());
        while twice(&after, "●2") {
            assert!(
                Instant::now() < deadline,
                "the tabs never followed the store; grid was: {after}"
            );
            std::thread::sleep(Duration::from_millis(10));
            after = grid_of(&editor.screen());
        }
        assert!(
            twice(&after, "sample.txt ●1"),
            "and the count they settled on is the one that is left; grid was: \
             {after}"
        );

        // **Closing back to one pane takes the strip away again**, which is the
        // half of *"never on the first"* that a session which never split
        // cannot test: the row has to come *back*.
        let closed = editor.after(b"\x17c");
        let top = closed.line(0);
        let closed = grid_of(&closed);
        // `leave_by`, not `quit`: `gs` opened a which-key group on the way, so
        // frames arrived that no press asked for and `quit`'s per-key frame
        // accounting would trip on them.
        editor.leave_by(b"ZQ");

        assert!(
            !closed.contains("panes"),
            "the strip outlived the second pane; grid was: {closed}"
        );
        assert!(
            top.contains("alpha"),
            "and the row went back to the buffer; row was: {top:?}"
        );
    }

    /// **`:close-buffer` on the only buffer says what to type instead.**
    ///
    /// It used to answer *"one buffer, one pane — :quit leaves; T088 gives a
    /// buffer somewhere to close to"* and sat in the deferred table above,
    /// naming its task. `T088`'s step 8 built it, so the row went red and left
    /// — and this is what took its place, because the lint that noticed
    /// `:close-buffer` was suddenly unpressed is right to: a command nothing
    /// types is a command nothing checks.
    ///
    /// The answer is no longer about a task. Closing the only buffer would
    /// leave a pane with nothing in it, and `:quit` is the verb for leaving, so
    /// the refusal names the command that does what you meant.
    #[test]
    fn close_buffer_on_the_only_buffer_names_quit_instead() {
        let scratch = Scratch::new("close-only");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("only.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let said = editor.press_until(b":close-buffer\r", "the only buffer");
        editor.quit();

        assert!(
            shows(&said, "the only buffer — :quit leaves"),
            "closing the last buffer names the command that leaves; frame was: {said}"
        );
        assert!(
            !shows(&said, "T088"),
            "and it no longer names a task, because the task landed; frame was: {said}"
        );
    }

    /// **Every deferred ex command names the task that builds it** — the ex
    /// line's half of `a_deferred_binding_names_the_task_that_builds_it`.
    ///
    /// Five of the eighteen answer a refusal rather than doing something, and
    /// none of them was typed by anything. The same table shape, for the same
    /// reason: when a task lands its command stops refusing and the row that
    /// named it goes red, so this can only shrink.
    ///
    /// **`close-buffer` left this table at `T088`'s step 8**, which is the
    /// table doing its job: it declined while there was one buffer and named
    /// the task, the task landed, and the row went red. It closes now, so
    /// there is no refusal here to name anything.
    #[test]
    fn a_deferred_ex_command_names_the_task_that_builds_it() {
        let scratch = Scratch::new("ex-deferred");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("ex.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `(command, the task its refusal must name)`, each read off that
        // capability's own row in `action.rs`.
        // `:transcript` left this table with `SPC t` — same task, same
        // capability, same reason.
        // `:inbox` left this table when `T067` built `5c` — it opens a float
        // now, which is the third command to graduate off this list and the
        // reason the list is written as data rather than as prose.
        // `:comment` left this table when `T068` built `3a` — it starts a
        // thread now. That is the fourth command to graduate off this list in
        // as many tasks, which is the reason the list is written as data: each
        // departure is one line, and the line that stays is the claim.
        // **The table is empty, and that is the claim now.** `:reattach` left
        // when `T057` built the lifecycle, `:comment` when `T068` built `3a`,
        // and `:diff-disk` — the last row — when `T070` built `5b`. Every ex
        // command this editor registers is live.
        //
        // A shrink-only list that reaches zero has to say something or become a
        // loop over nothing that passes forever. So the claim inverts: the one
        // command that left most recently is pressed, and it must decline **by
        // name** rather than by task. That is the same rung `SPC c i` is held
        // to one test up — *"a bound key that cannot act says what is missing
        // rather than which task"* — and it is what a graduated command owes.
        let deferred: &[(&str, &str)] = &[];
        for (command, task) in deferred {
            let said = editor.press_until(format!(":{command}\r").as_bytes(), task);
            assert!(
                shows(&said, task),
                ":{command} is not built and must say which task builds it; \
                 frame was: {said}"
            );
        }

        // `:diff-disk` on a buffer that agrees with disk. Not a task id, not
        // silence — the reason.
        let agreed = editor.press_until(b":diff-disk\r", "already agree");
        assert!(
            shows(&agreed, "already agree"),
            "`:diff-disk` is built and declines by name; frame was: {agreed}"
        );
        assert!(
            !shows(&agreed, "T070"),
            "and it names no task, because the task landed; frame was: {agreed}"
        );
        editor.quit();
    }

    /// **`zc`, `zo` and `zM` — the fold keys that are not `za` or `zR`.**
    ///
    /// `za` toggles and `zR` opens everything, and those two were the whole of
    /// what any test pressed. The other three are the *explicit* forms — close
    /// this one, open this one, close everything — and each is a distinct arm
    /// (`set-fold` with `folded`/`unfolded`, and `fold-all`). A toggle passing
    /// says nothing about whether the explicit pair are wired to the right
    /// states, which is exactly the mixup a toggle cannot expose.
    ///
    /// The layer redeclares `rust` with no server, for the reason
    /// `za_closes_the_fold_the_cursor_is_in` gives at length: a `.rs` file
    /// otherwise starts whatever is installed, which draws frames no key asked
    /// for.
    #[test]
    fn zc_zo_and_zm_are_the_explicit_folds() {
        let scratch = Scratch::new("folds-explicit");
        let runtime = copy_layer(&scratch.path);
        fs::write(
            scratch.persisted().join("persisted.scm"),
            "(define-language! \"rust\"\n  (hash \"extensions\" '(\"rs\")\n        \
             \"grammar\" \"rust\"\n        \"lsp_command\" (list)\n        \
             \"comment_prefix\" \"//\"))\n",
        )
        .expect("the config home takes a declaration");
        let file = scratch.path.join("folded.rs");
        fs::write(
            &file,
            "fn outer() {\n    let marker_inside_the_fold = 1;\n}\n\
             fn second() {\n    let other_marker = 2;\n}\n",
        )
        .expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let grid = |screen: &Screen| {
            (0..SCREEN.ws_row)
                .map(|row| screen.line(row))
                .collect::<Vec<_>>()
                .join("\n")
        };

        // `zc` closes, and unlike `za` it is not a toggle: pressing it on an
        // already-closed fold leaves it closed.
        editor.press(b"zc");
        assert!(
            !shows(&grid(&editor.screen()), "marker_inside_the_fold"),
            "zc closed the fold the cursor is in"
        );
        editor.press(b"zc");
        assert!(
            !shows(&grid(&editor.screen()), "marker_inside_the_fold"),
            "and pressing it again did not re-open it, which is what makes it \
             not a toggle"
        );

        // `zo` opens the same one.
        let opened = editor.shown_on_grid(b"zo", "marker_inside_the_fold");
        assert!(
            grid(&opened).contains("marker_inside_the_fold"),
            "zo opened the fold zc closed"
        );

        // `zM` closes every fold — the half `zR` mirrors, and the one no test
        // pressed while `zR` had one.
        editor.press(b"zM");
        let all = grid(&editor.screen());
        editor.quit();
        assert!(
            !shows(&all, "marker_inside_the_fold") && !shows(&all, "other_marker"),
            "zM closed both folds, not just the one under the cursor; \
             screen was:\n{all}"
        );
    }

    /// **`[u` and `SPC u n` — the two ways into the unseen walk that `]u` is
    /// not.**
    ///
    /// One capability, three bindings, one of them pressed. That is the shape
    /// the `<C-i>` defect had: a second spelling nobody exercised, unreachable
    /// for a reason the first spelling could not show.
    ///
    /// **Three regions, and the walk stops on the middle one.** Two is not
    /// enough and the first draft of this test used two: `Next` wraps —
    /// `find(|line| *line > here).unwrap_or(lines[0])` — so from the *last*
    /// region a forwards seek lands on the first, which is exactly where
    /// backwards lands too. A planted `Seek::Prev => Next` passed it. From the
    /// middle of three the two answers differ, and the same plant fails.
    #[test]
    fn the_other_two_ways_into_the_unseen_walk() {
        let scratch = Scratch::new("unseen-walk");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("walk.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\nsix\nseven\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            declare(&file, &[(2, 3), (4, 5), (6, 7)]).as_bytes(),
            "landed=3",
        );
        editor.press_until(b"(close-repl!)\r", "1    one");

        // `SPC u n` — `3c`'s `+unseen · next`, the same capability `]u` names.
        let next = editor.shown_on_grid(b" un", "2:1");
        assert!(
            statusline(&next).contains("2:1"),
            "SPC u n walked to the first region; statusline was: {}",
            statusline(&next)
        );

        // Forward again with the bracket spelling, onto the middle one — the
        // only place from which backwards and forwards disagree.
        let further = editor.shown_on_grid(b"]u", "4:1");
        assert!(
            statusline(&further).contains("4:1"),
            "]u walked to the second; statusline was: {}",
            statusline(&further)
        );

        // `[u` — backwards, the arm with no coverage at all. A `Prev` wired to
        // `Next` would answer `6:1` here.
        let back = editor.shown_on_grid(b"[u", "2:1");
        editor.quit();
        assert!(
            statusline(&back).contains("2:1"),
            "[u walked back to the first — the seek is a direction, not a \
             synonym; statusline was: {}",
            statusline(&back)
        );
    }

    /// **Every surface you can open, you can escape.**
    ///
    /// Eight `Surface` variants ship; `Boot` and `Fixture` are startup and
    /// scaffolding, and `Buffer` is what the others sit on. The rest are things
    /// a key opens over your file, and the one property all of them owe you is
    /// a way back out — an editor that can trap you on a float is a modal
    /// editor with a hole in it.
    ///
    /// **Nothing asserted this.** `esc` is pressed after a picker in three
    /// tests, always as *cleanup* before the next assertion and never as the
    /// thing under test; help's dismissal is tested through `q`, which its own
    /// footer documents, and not through `esc`. A surface shipped without an
    /// escape would have been caught by nothing, and a table is what stops the
    /// next one being added without one.
    #[test]
    fn every_surface_you_can_open_you_can_escape() {
        let scratch = Scratch::new("escapable");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("s.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\n").expect("a fixture");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // `(what opens it, a string only that surface draws)`.
        let surfaces: &[(&str, &[u8], &str)] = &[
            ("the REPL", b":repl\r", "steel"),
            ("the help float", b":help\r", "agent-objects"),
            ("the files picker", b" f", "Cargo.toml"),
        ];
        for (what, open, marker) in surfaces {
            editor.shown_on_grid(open, marker);
            editor.press_quietly(b"\x1b");
            // Polled, because a dismissal is the *absence* of something and
            // `shown_on_grid` can only wait for a presence.
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut dismissed = false;
            let mut last = String::new();
            while Instant::now() < deadline {
                let screen = editor.screen();
                last = (0..SCREEN.ws_row)
                    .map(|row| screen.line(row))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !shows(&last, marker) {
                    dismissed = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(dismissed, "esc did not close {what}; screen was:\n{last}");
            assert!(
                shows(&last, "charlie"),
                "and the buffer is underneath again; screen was:\n{last}"
            );
        }
        editor.quit();
    }

    /// **`↵` on a file row opens the file** — the half that was never pressed.
    ///
    /// Reported by Teej at a real terminal: every row of the file picker
    /// declined with
    ///
    /// ```text
    /// that row does not name a place — sources write `path:line` first
    /// ```
    ///
    /// which was `accept_picker`'s own doc comment asserting an invariant
    /// nothing checked. `8a`'s rows *are* `path:line` and three of the four
    /// sources write it — but `3d`'s file rows are bare names by design, so
    /// the one source that follows the mockup was the one that could not be
    /// accepted.
    ///
    /// **Nothing in this repository had ever pressed `↵` on a picker row.**
    /// `grep_rows_carry_the_store_and_tab_cycles_the_source`'s own summary line
    /// says *"tab, and `↵` opens"* and its body stops after the tab; every
    /// other picker test asserts on the list and presses escape. A whole
    /// keystroke on a shipped surface, described in a doc comment and covered
    /// by nothing — which is exactly the shape of defect a human at a terminal
    /// finds first.
    ///
    /// Both spellings are pressed here, in one session, because the fix is
    /// that there are two: the file row carries no line and the grep row does.
    #[test]
    fn enter_on_a_picker_row_opens_it_whichever_way_the_row_is_spelled() {
        let scratch = Scratch::new("picker-accept");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("alpha.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // **`3d`, a bare name.** The workspace is the editor's cwd — the
        // `phosphor` crate, since the pty child inherits this process's
        // directory — so `Cargo.toml` is a row, and it is the needle for the
        // reason the test above gives: every crate directory has one.
        editor.press_until(b" f", "Cargo.toml");
        let opened = editor.press_until(b"Cargo.toml\r", "[package]");
        assert!(
            !shows(&opened, "does not name a place"),
            "a file row is a place; frame was: {opened}"
        );
        assert!(
            shows(&opened, "[package]"),
            "and pressing it opened the file, not a refusal; frame was: {opened}"
        );

        // **`8a`, a `path:line`.** Back to the fixture, then grep it — one row
        // per buffer line — and accept the row naming line 3. The cursor
        // landing there is the half a bare name cannot carry, so this is what
        // says the position is still honoured.
        editor.press_until(format!(":e {}\r", file.display()).as_bytes(), "three");
        editor.press_until(b":repl\r", "steel");
        editor.press_until(b"(open-picker! \"grep\")\r", "3/3");
        editor.press_until(b"three\r", "1    one");
        let landed = editor.screen();
        editor.quit();

        // The statusline's cursor readout: `alpha.txt:3` is the row that was
        // accepted, and line 3 is where a target carrying a position puts you.
        let status = landed.line(SCREEN.ws_row - 1);
        assert!(
            status.contains("3:1"),
            "a `path:line` row still lands on its line; statusline was: {status}"
        );
    }

    /// **`T047`: `8a` from a keystroke — grep rows and tab.**
    ///
    /// `8a` draws `src/retry.rs:9  ●  pub max_delay: Duration,` and its caption
    /// is *"results know who touched them"*, so the row carries the store's
    /// unseen dot beside the line.
    ///
    /// **This summary read *"tab, and `↵` opens"* and the body never pressed
    /// `↵`.** Nothing else did either, in any test, which is how the file
    /// picker shipped unable to accept a single row — see
    /// `enter_on_a_picker_row_opens_it_whichever_way_the_row_is_spelled`, which
    /// is where that keystroke lives now. A summary line describing a keystroke
    /// the body does not press is worth exactly as much as the coverage behind
    /// it.
    #[test]
    fn grep_rows_carry_the_store_and_tab_cycles_the_source() {
        let scratch = Scratch::new("picker-grep");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(2, 2)]).as_bytes(), "landed=1");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // The grep source is first in `phosphor/picker-sources`, so it is what
        // a door-opened picker over `grep` shows: one row per buffer line.
        editor.press_until(b":repl\r", "steel");
        let grepped = editor.press_until(b"(open-picker! \"grep\")\r", "3/3");
        assert!(
            shows(&grepped, "bravo"),
            "a grep row carries the line's text; frame was: {grepped}"
        );
        assert!(
            shows(&grepped, "●"),
            "and the store's unseen marker for the line a region covers; frame was: {grepped}"
        );

        // **Tab cycles.** `grep` → `files`, and the two draw different things:
        // grep's rows are this buffer's three lines, files' are the workspace.
        // `Cargo.toml` is the needle because it is in the editor's cwd (the
        // crate directory) and cannot be a grep row.
        let cycled = editor.press_until(b"\t", "Cargo.toml");
        editor.quit();

        assert!(
            shows(&cycled, "Cargo.toml"),
            "tab moved to the next source in the layer's order; frame was: {cycled}"
        );
    }

    /// **`T048`: `:arch` reproduces `6a`, over the live store, adding zero
    /// lines to `phosphor-ui`.**
    ///
    /// The third clause is the interesting one and it is checked by
    /// construction rather than by this test: every row of `runtime/arch.scm`
    /// is `view/spans`, and `scripts/lint-one-escape-hatch.sh` proves that is
    /// the only custom-draw path there is. A Rust primitive for this screen
    /// would have to appear in `phosphor-ui` to be drawn at all.
    ///
    /// What a test *can* hold is the second clause — that it reflects the
    /// **actual** store. So the same command runs twice with a declaration in
    /// between, and the number in the box moves.
    #[test]
    fn arch_draws_the_live_store_from_the_spans_hatch() {
        let scratch = Scratch::new("arch");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // Empty store: the box says so rather than saying nothing.
        let empty = editor.press_until(b":arch\r", "0 unseen");
        assert!(
            shows(&empty, "semantic store"),
            "6a's centre box; frame was: {empty}"
        );
        assert!(
            shows(&empty, "one API, two callers"),
            "and its caption; frame was: {empty}"
        );
        // **`esc`, not `q`.** This float's footer said `q close` and `q` never
        // closed it — `closes_surface` gives `q` to the help grid alone. The
        // float stayed open and the next `:repl` opened over it, which is why
        // the test passed. T099 made the stray `q` audible by turning it into a
        // register prefix that ate the `:` after it.
        editor.press_quietly(b"\x1b");

        // Two regions, and the same command drawn again.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(1, 1), (3, 3)]).as_bytes(), "landed=2");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        let filled = editor.press_until(b":arch\r", "2 unseen");
        editor.quit();

        assert!(
            shows(&filled, "2 unseen"),
            "the diagram is a query, not a drawing; frame was: {filled}"
        );
    }

    /// **`T049`: `viu` selects an unseen region.**
    ///
    /// `6d`'s agent nouns *"parse here and resolve at `T049`"* — the keymap's
    /// own words, written when `viu` selected nothing. It selects now, over the
    /// same store the gutter draws from, so the noun and the marker cannot
    /// disagree.
    ///
    /// **Read off the file, not the frame.** A selection is a highlight and a
    /// highlight is hard to assert on; what is unambiguous is what an operator
    /// over it *does*. So this deletes the object — `diu` — and reads what is
    /// left, which is three lines only if the noun covered exactly the region.
    #[test]
    fn viu_selects_the_unseen_region_under_the_cursor() {
        let scratch = Scratch::new("agent-nouns");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // **Lines 2 and 3, not 2 through 4.** The declaration is
        // `[2:1, 4:1)` and a span is half-open (`store::region::overlaps` —
        // *"touching is not overlapping"*), so line 4 column 1 is the first
        // position *after* the region. Asserting `one\nfive\n` here is what a
        // first draft did, and the span convention is the answer rather than
        // the bug.
        editor.press_until(declare(&file, &[(2, 4)]).as_bytes(), "landed=1");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // Cursor onto line 3, inside the region, then delete the noun.
        editor.press_quietly(b"jj");
        editor.press_quietly(b"diu");
        editor.press_until(b":w\r", "sample.txt");
        editor.quit();

        let after = fs::read_to_string(&file).expect("written");
        assert_eq!(
            after, "one\nfour\nfive\n",
            "`diu` took the region's lines and nothing else — linewise, 2 and 3",
        );
    }

    /// **`dih` is the mockup's own answer to "revert claude's edit"** — the
    /// design docs' words, verbatim: `TUI Mockups.dc.html`'s `6d` draws
    /// `dih  delete inner hunk — revert claude's edit, plain vim delete`.
    ///
    /// **Why `RevertHunk` — the richer wire capability with a before-side —
    /// stays unbuilt.** It would restore what claude's edit replaced; `dih` only
    /// deletes what is there now, which is *"plain vim delete"* in the mockup's
    /// own words and not a restore. No screen draws a key for the richer verb —
    /// `4b`'s footer has no revert key at all, and `2b`'s `u undo (jj)` is
    /// `T073`'s, a different verb over a different store. `dih` is what every
    /// mockup that mentions reverting actually asks for, and this is where that
    /// gets read rather than assumed.
    #[test]
    fn a_declared_hunks_dih_reverts_it_plain_vim_delete_style() {
        let scratch = Scratch::new("dih-reverts");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("fetch.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let form = format!(
            "(declare-review-block! \"retry logic\" \
             (list (hash \"path\" \"{}\" \"spans\" \
                   (list (hash \"span\" (hash \"start\" (hash \"line\" 2 \"column\" 1) \
                                          \"end\" (hash \"line\" 3 \"column\" 1)) \
                                \"was\" \"was two\")))) \
             \"one region\")\r",
            file.display()
        );
        editor.press_until(form.as_bytes(), "one region");
        editor.press_until(b"(close-repl!)\r", "review ready");

        editor.press_quietly(b"gg");
        editor.press_quietly(b"j");
        editor.press_quietly(b"dih");
        editor.press_until(b":w\r", "fetch.txt");
        editor.quit();

        let after = fs::read_to_string(&file).expect("written");
        // **Deleted, not restored** — `dih` takes the hunk's *current* line
        // and nothing replaces it, which is what makes it *"plain vim delete"*
        // rather than the richer verb. `was two` — what claude's edit
        // replaced — is nowhere in this file; a real revert would put it back.
        assert_eq!(
            after, "one\nthree\nfour\nfive\n",
            "the hunk's current line is gone; `was two` was never written back"
        );
    }

    /// **Two of the three nouns still have no store; `dih` now does and this
    /// fixture has nothing declared for it to find** (`T049`, `T064`, `T066`).
    ///
    /// `6d` draws four. `T064` built hunks and `dih` resolves through it now —
    /// see `a_declared_hunks_dih_reverts_it_plain_vim_delete_style` for the
    /// case where a hunk *is* declared. `T068` threads and `T053`'s review
    /// blocks give `dit`/`dib` no store to speak of yet, so those two are still
    /// the true *"select nothing, rather than selecting something wrong"* case
    /// this test names. This paragraph used to say `dih` was the same as the
    /// other two; it was, until `T064` built the store under it.
    #[test]
    fn the_nouns_without_a_store_select_nothing() {
        let scratch = Scratch::new("agent-nouns-unbuilt");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_quietly(b"dih");
        editor.press_quietly(b"dit");
        editor.press_quietly(b"dib");
        editor.press_until(b":w\r", "sample.txt");
        editor.quit();

        let after = fs::read_to_string(&file).expect("written");
        assert_eq!(
            after, "one\ntwo\nthree\n",
            "none of the three took anything"
        );
    }

    /// **`T044`: seen-state survives `kill -9`.**
    ///
    /// The task asks for *"survives restart and `kill -9`"*, and `kill -9` is
    /// the harder half by a long way — no exit code runs, no destructor, no
    /// `fsync`. `journal.rs` is designed against exactly this: an append is a
    /// `write_all` and nothing more, so the bytes belong to the kernel's page
    /// cache the moment the call returns and outlive the process. What a crash
    /// can cost is a torn record at the tail, which the next open truncates.
    ///
    /// Both sessions share one [`Scratch`], so they share one `XDG_STATE_HOME`
    /// and therefore one workspace journal. That is the whole mechanism under
    /// test: **two processes, one store.**
    #[test]
    fn seen_state_survives_a_kill_nine() {
        let scratch = Scratch::new("seen-persist");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        // Session one: three regions, one of them marked seen, then killed.
        let first = Editor::open(&file, &scratch.state(), &runtime);
        first.press_until(b":repl\r", "steel");
        first.press_until(
            declare(&file, &[(1, 2), (3, 4), (5, 5)]).as_bytes(),
            "landed=3",
        );
        first.press_until(COUNT, "unseen=3");
        first.press_until(
            format!(
                "(string-append \"marked=\" (number->string (mark-seen! (hash \"kind\" \
                 \"explicit\" \"path\" \"{}\" \"span\" (hash \"start\" (hash \"line\" 3 \
                 \"column\" 1) \"end\" (hash \"line\" 4 \"column\" 1))))))\r",
                file.display()
            )
            .as_bytes(),
            "marked=1",
        );
        first.press_until(COUNT, "unseen=2");
        // **No clean quit.** `SIGKILL`, which is the criterion.
        first.kill();

        // Session two: a fresh process, the same workspace.
        let second = Editor::open(&file, &scratch.state(), &runtime);
        second.press_until(b":repl\r", "steel");
        second.press_until(COUNT, "unseen=2");
        let drawn = second.press_until(
            b"(string-append \"seen=\" (number->string (seen-count)))\r",
            "seen=1",
        );
        second.quit();

        assert!(
            shows(&drawn, "seen=1"),
            "the region marked before the kill is still marked; frame was: {drawn}"
        );
    }

    /// **`T042`'s keystroke criterion, end to end.** `m{a-z}` writes a mark,
    /// `` `{a-z} `` reads it back, and the proof is the *file* rather than the
    /// statusline — a cursor that says it is on line 1 and edits line 5 is
    /// exactly the class of defect a position assertion cannot see.
    ///
    /// `m` alone is a prefix now (26 rows, generated in `runtime/keymaps.scm`),
    /// so pressing it waits rather than acting. That is why this presses `ma`.
    #[test]
    fn a_mark_is_set_and_returned_to() {
        let scratch = Scratch::new("mark-round-trip");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\ndelta\necho\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Mark line 1, walk to the bottom, come back, and delete a character.
        editor.press(b"ma");
        editor.press(b"G");
        editor.press(b"`a");
        editor.press(b"x");
        editor.press_until(b":w\r", "sample.txt");
        editor.quit();

        let after = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            after, "lpha\nbravo\ncharlie\ndelta\necho\n",
            "`a returned to the marked line, so x took alpha's first character",
        );
    }

    /// **`'` goes to the line, `` ` `` goes to the column** — the whole of what
    /// separates the two keys, and only one of them was pressed.
    ///
    /// `goto_anchor` takes an `exact` flag and the keymap generates 26 rows for
    /// each spelling, so this is one boolean with 52 bindings over it. The test
    /// above presses `` `a ``; the only `'` in this file presses `'z` and asserts
    /// the **refusal** for a mark that was never set. So the success path of `'`
    /// — and with it the flag's `false` arm — had nothing.
    ///
    /// The mark is set at a column well inside the line, which is what makes
    /// the two answers different: `` ` `` returns to that column and `'` returns
    /// to column 1. A fixture marked at column 1 would pass either way.
    #[test]
    fn quote_returns_to_the_line_and_backtick_to_the_column() {
        let scratch = Scratch::new("mark-column");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("marks.txt");
        fs::write(&file, "alpha\nbravo\ncharlie\ndelta\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Line 3, column 5 — `charlie`'s `i`. Marked there.
        editor.press(b"jj");
        editor.press(b"llll");
        editor.press(b"mq");
        let marked = editor.screen().line(SCREEN.ws_row - 1);
        assert!(
            marked.contains("3:5"),
            "the mark is set away from column 1, or this proves nothing; \
             statusline was: {marked}"
        );

        // Away, then back the exact way.
        editor.press(b"G");
        let exact = editor.shown_on_grid(b"`q", "3:5");
        assert!(
            exact.line(SCREEN.ws_row - 1).contains("3:5"),
            "backtick returned to the column the mark was written at; \
             statusline was: {}",
            exact.line(SCREEN.ws_row - 1)
        );

        // And back the line way, which must land in column 1.
        editor.press(b"G");
        let line = editor.shown_on_grid(b"'q", "3:1");
        editor.quit();
        assert!(
            line.line(SCREEN.ws_row - 1).contains("3:1"),
            "quote returned to the line and not the column; statusline was: {}",
            line.line(SCREEN.ws_row - 1)
        );
    }

    /// **`ZZ` writes and leaves**, and `CP-4` found a defect in it by hand
    /// while nothing pressed it.
    ///
    /// It is `save-buffer` then `quit` — the same Action list `:wq` builds —
    /// and the fix that came out of `CP-4` was about which of the two refusals
    /// a user is shown: on a buffer with no name it said *"unsaved work — force
    /// it or save first"* and swallowed *"no file name — :write <path>"*, which
    /// is the half that says what to type. `Session::key` takes the **first**
    /// refusal because of it.
    ///
    /// That fix has a test through `:wq` (`wq_writes_the_buffer_and_leaves`)
    /// and had none through the key, although the key is where it was found.
    #[test]
    fn zz_writes_the_buffer_and_leaves() {
        let scratch = Scratch::new("zz-exit");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("zz.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.shown_on_grid(b"J", "alpha bravo");
        editor.leave_by(b"ZZ");

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "alpha bravo\n",
            "ZZ wrote before it left — the whole difference from ZQ"
        );
    }

    /// And the other half of that pair: `ZQ` throws the edit away.
    ///
    /// Asserted because it is the one exit every other test in this file ends
    /// with — `Editor::quit` presses it — so a `ZQ` that quietly started
    /// writing would make every one of those tests lie about the file it left
    /// behind, and nothing would say so.
    #[test]
    fn zq_leaves_without_writing() {
        let scratch = Scratch::new("zq-exit");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("zq.txt");
        fs::write(&file, "alpha\nbravo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.shown_on_grid(b"J", "alpha bravo");
        editor.leave_by(b"ZQ");

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "alpha\nbravo\n",
            "ZQ left the file alone, which is what makes it the harness's exit"
        );
    }

    /// **`T043`'s criterion: markers work correctly on an extensionless file
    /// with no grammar.**
    ///
    /// The file is called `deploy`, has no extension, and nothing in the
    /// bundled ten parses it — so the node tier never applies and the line tier
    /// is the *only* thing holding the marker on. That is the task's whole
    /// argument: *"the floor, not a degraded extra"*, and it is what makes an
    /// unseen marker a store feature rather than a language feature
    /// (invariant 4).
    ///
    /// The proof is that the region's span **moves** when text is inserted
    /// above it. A positional region — what the store did before this task —
    /// would still claim line 2 while the code it described sat on line 4.
    #[test]
    fn a_marker_on_a_grammar_free_file_survives_the_edit_that_moves_it() {
        let scratch = Scratch::new("no-grammar-marker");
        let runtime = copy_layer(&scratch.path);
        // No extension, and `run_the_thing --now` is the line the marker is on.
        let file = scratch.path.join("deploy");
        fs::write(&file, "#!/bin/sh\nrun_the_thing --now\nexit 0\n").expect("a fixture");

        let line_of = "(string-append \"line=\" (number->string (hash-ref (hash-ref (hash-ref \
             (car (regions)) \"span\") \"start\") \"line\")))\r";

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(2, 2)]).as_bytes(), "landed=1");
        editor.press_until(line_of.as_bytes(), "line=2");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // Two lines inserted above it, by duplicating the shebang twice.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"yy");
        editor.press_quietly(b"P");
        editor.press_quietly(b"P");
        // `reanchor` reads the file, so the buffer has to be on disk first.
        editor.press_until(b":w\r", "deploy");

        editor.press_until(b":repl\r", "steel");
        let reanchored = format!("(reanchor! \"{}\")\r", file.display());
        editor.press_until(reanchored.as_bytes(), "moved");
        editor.press_until(line_of.as_bytes(), "line=4");
        editor.quit();

        let after = fs::read_to_string(&file).expect("written");
        assert_eq!(
            after, "#!/bin/sh\n#!/bin/sh\n#!/bin/sh\nrun_the_thing --now\nexit 0\n",
            "the fixture edit is what the assertion above depends on",
        );
    }

    /// The other half: a mark that was never set declines by name rather than
    /// jumping somewhere plausible.
    #[test]
    fn an_unset_mark_says_so() {
        let scratch = Scratch::new("mark-unset");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let drawn = editor.press_until(b"'z", "no mark z");
        editor.quit();

        assert!(
            shows(&drawn, "no mark z"),
            "an unset mark is declined by name; frame was: {drawn}"
        );
    }

    /// **`B1` at the loop level: `3x` on a three-character line takes all
    /// three.** It used to take two.
    ///
    /// `crates/phosphor-core/tests/input.rs`'s
    /// `a_counted_fused_operator_takes_the_last_character_of_the_line` proves
    /// the rule against `Machine`; nothing proved it against the *binary*
    /// after the scratch driver that did was deleted. So this presses the keys
    /// and reads the file: `l` is exclusive and stops at the end of the line,
    /// and the end of the line for an *operand* is the boundary past the last
    /// character rather than the last character itself.
    #[test]
    fn a_counted_delete_takes_the_last_character_of_the_line() {
        let scratch = Scratch::new("counted-delete");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "abc\nkeep\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"3x");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "\nkeep\n",
            "3x took the whole line — `c` left behind is the defect B1 named"
        );
    }

    /// **`B2` at the loop level: an operator lands the cursor at the start of
    /// what it touched.**
    ///
    /// `crates/phosphor-core/tests/input.rs`'s
    /// `an_operator_lands_the_cursor_at_the_start_of_what_it_touched` proves it
    /// against `Machine`. Proving it against the binary needs the cursor to be
    /// *observable*, and the frame is not where to read it — so the next key
    /// reads it instead: `i` inserts wherever the cursor ended up, and the
    /// saved file says where that was.
    ///
    /// `gUiw` from the middle of `alpha` uppercases the word and lands on its
    /// first column, so the `X` goes in front. A cursor left where it started
    /// would spell `ALXPHA beta`.
    /// **`§21`: `~` advances past what it wrote, not by one character.**
    ///
    /// `text::cased` is not character-count preserving —
    /// `to_uppercase('\u{df}')` is `"SS"` — and `~` is the one case operator
    /// that *advances*, so it was the one that could land inside its own
    /// output. Measured before the fix, through this same harness:
    /// `~` on `\u{df}xy` gave `S|Sxy` and `~~` gave `Ss|xy`, the second toggle
    /// re-casing the second `S` instead of moving on to `x`.
    ///
    /// The cursor is read the way `an_operator_leaves_the_cursor_where_the_
    /// next_key_can_prove_it` reads it: `i` inserts wherever it ended up, and
    /// the saved file says where that was. The ASCII rows are the regression
    /// half — a fix that moved the cursor differently when nothing grew would
    /// break every `~` a person actually types.
    #[test]
    fn a_case_change_that_grows_leaves_the_cursor_past_what_it_wrote() {
        for (name, keys, source, expected) in [
            ("one toggle", &b"~"[..], "\u{df}xy\n", "SS|xy\n"),
            (
                "twice, and the second lands on x",
                &b"~~"[..],
                "\u{df}xy\n",
                "SSX|y\n",
            ),
            ("at end of line", &b"~"[..], "\u{df}\n", "SS|\n"),
            ("ascii is unchanged", &b"~"[..], "abc\n", "A|bc\n"),
            // `gU` was never affected and must stay that way: an operator lands
            // at the *start* of what it touched, and a start does not move when
            // an end does.
            (
                "gU still lands at the start",
                &b"gUiw"[..],
                "stra\u{df}e beta\n",
                "|STRASSE beta\n",
            ),
        ] {
            let scratch = Scratch::new("case-grows");
            let runtime = copy_layer(&scratch.path);
            let file = scratch.path.join("sample.txt");
            fs::write(&file, source).expect("a fixture");

            // **`press_quietly`, and the assertion below is why.** What this
            // proves is the *file* after `:w`, which `quit` guarantees by
            // waiting for the child to exit — no frame is read, so counting
            // them buys nothing and costs a flake. `gUiw` is four bytes in one
            // `press`, and `g` opens a which-key popup on the way: the
            // one-frame-per-key contract was never true of it, and CI is where
            // that came due.
            let editor = Editor::open(&file, &scratch.state(), &runtime);
            editor.press_quietly(keys);
            editor.press_quietly(b"i");
            editor.press_quietly(b"|");
            editor.press_quietly(b"\x1b");
            editor.press_quietly(b":w\r");
            editor.quit();

            assert_eq!(
                fs::read_to_string(&file).expect("the file survives"),
                expected,
                "{name}: `|` marks where the cursor was left"
            );
        }
    }

    /// **Enter and space still work in a buffer that has no language server**,
    /// which is most buffers and every buffer before `S4`.
    ///
    /// Not a duplicate of the two guard tests over the toy server: those run
    /// with a float open and unchosen, and this one runs where there is no
    /// completion machinery at all — no server, no session, nothing for
    /// `accept-completion` to look at. Both reach the same `otherwise` arm and
    /// the second is the one that would have gone unnoticed, because a
    /// regression here breaks *typing* rather than completion and no completion
    /// test would see it.
    ///
    /// This file has no other test that presses <kbd>enter</kbd> in insert
    /// mode: every `\r` in it terminates an ex line, which is a different
    /// scope.
    #[test]
    fn enter_and_space_are_still_text_in_a_buffer_with_no_server() {
        let scratch = Scratch::new("insert-text-keys");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"A");
        editor.press_quietly(b" beta\rgamma");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "alpha beta\ngamma\n",
            "space typed a space and enter split the line, with no server in sight"
        );
    }

    /// **A Rust generic types forwards** — `CP-4`, in the first minute of being
    /// the user.
    ///
    /// `phosphor/prefix?` compared canonical key spellings with `starts-with?`,
    /// on characters. The character `<` spells itself, so it was a string
    /// prefix of `<space>`, `<esc>`, `<C-x>` and every other bracketed row in
    /// the insert scope: the machine answered `Pending`, held the key, and
    /// flushed the batch as text when the sequence died — with every Action in
    /// the batch built against one stale cursor, so they came out reversed.
    /// Typed one character at a time into the running binary, `a<u8>b` wrote
    /// `a8>bu<` and `pub fn probe() { let v: Vec<u8> = Vec::new(); v.` wrote
    /// `…let v: Vec8> = Vec::new(); v.u<`.
    ///
    /// **This is the outcome test, and it needs both halves broken to go red**
    /// — measured, by planting each mutation on its own and then together.
    /// `runtime/keymaps.scm` no longer calls `<` a prefix, so no insert
    /// sequence is pending and the flush never sees more than one key;
    /// `Machine::insert_keys` walks the position across a batch, so a batch is
    /// in order even when one forms. Either fix alone rescues this string. The
    /// isolating tests are `phosphor-steel`'s
    /// `a_printable_character_is_not_a_prefix_of_a_bracketed_binding` and
    /// `phosphor-core`'s `an_unbound_sequence_in_insert_mode_types_its_keys_in_order`,
    /// and each of those was watched going red on its own mutation. With both
    /// planted this reads `let v: Vec8> = Vec::new();u<`, which is what a user
    /// gets.
    ///
    /// The fixture is `.txt` and the text is Rust: the defect is the keymap's,
    /// so no server is wanted here — and a `.rs` file starts `rust-analyzer`,
    /// whose unasked frames break the counted-frames discipline this test is
    /// leaning on.
    #[test]
    fn a_rust_generic_types_forwards_in_insert_mode() {
        let scratch = Scratch::new("insert-angle-brackets");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"i");
        editor.press(b"let v: Vec<u8> = Vec::new();");
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "let v: Vec<u8> = Vec::new();\n",
            "every character landed where it was typed, in the order it was typed"
        );
    }

    /// **Enter in insert mode still scrolls, which is the regression binding
    /// `<cr>` nearly shipped** (`CP-4` review).
    ///
    /// A `<cr>` with no float open is an `accept-completion` whose `otherwise`
    /// is a newline, and `moves_cursor` did not name that Action — so
    /// `Editing::apply` skipped the reveal and every newline past the last
    /// visible row walked the cursor off the bottom of the screen. On the
    /// installed binary at 80x24 the statusline read `31:1` over a viewport
    /// still showing lines 1..23.
    ///
    /// **Read off the replayed grid, not off the transcript.** A frame is a
    /// diff, and typing a word at the end of a line redraws one cell per key —
    /// [`Editor::press_until`] on `"deep"` fails even when it *is* on screen,
    /// because the four characters arrive in four frames with cursor moves
    /// between them (observed here before this test was rewritten). [`Screen`]
    /// replays the whole stream onto a grid the size of [`SCREEN`], so
    /// *"which row is this word on"* is answerable and *"it is on no row"* is
    /// the failure. The file read afterwards pins what was typed as well as
    /// where it landed.
    #[test]
    fn enter_in_insert_mode_still_scrolls_the_viewport_after_it() {
        let scratch = Scratch::new("insert-enter-reveal");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "top\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"A");
        // Forty, against a thirty-row [`SCREEN`]: the cursor ends ten rows
        // below anything the opening frame could show.
        editor.press_quietly(&[b'\r'; 40]);

        // **Asserted before another key is typed**, and that is not fussiness:
        // any ordinary insert is a `Buffer::Insert`, which `moves_cursor` has
        // always named — so typing one letter first reveals the cursor and
        // hides the defect completely. Measured: with the `Action::Lsp` arm
        // removed, a version of this test that typed a word before looking
        // passed.
        let screen = editor.screen();
        let rows: Vec<String> = (0..SCREEN.ws_row).map(|y| screen.line(y)).collect();
        assert!(
            !rows.iter().any(|row| row.contains("top")),
            "the viewport never followed the newlines: line 1 is still drawn with the \
             cursor on line 41. Rows: {rows:#?}"
        );

        editor.press_quietly(b"deep");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            format!("top{}deep\n", "\n".repeat(40)),
            "forty newlines and a word, all of them where they were typed"
        );
    }

    /// **`R` is still vim's `R`** — the other half of the same `CP-4` finding.
    ///
    /// `Scope::of` folds `EditMode::Replace` into the insert scope, so the
    /// `<space>` and `<cr>` rows bind in replace mode too, where the loop's
    /// typing gate (`EditMode::Insert`) guarantees no float can ever be open.
    /// The `otherwise` fall-through therefore fires on every one of them, and
    /// while it spliced rather than overwrote, `R` was quietly `i`: this same
    /// script wrote `XY Zdef` and left the `d` alive.
    ///
    /// Nothing else in this file presses `R`, and the machine-level tests drive
    /// their own table rather than `runtime/keymaps.scm` — which is why a
    /// keymap row changed the meaning of a mode with every gate green.
    #[test]
    fn replace_mode_still_overwrites_with_space_bound_in_the_insert_scope() {
        let scratch = Scratch::new("replace-overwrites");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "abcdef\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"R");
        editor.press(b"XY Z");
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "XY Zef\n",
            "four keys replaced four characters — the space overwrote `c`, it did not push it right"
        );
    }

    #[test]
    fn an_operator_leaves_the_cursor_where_the_next_key_can_prove_it() {
        let scratch = Scratch::new("operator-cursor");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "alpha beta\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"ll");
        editor.press(b"gUiw");
        editor.press(b"i");
        editor.press(b"X");
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "XALPHA beta\n",
            "the operator landed the cursor at the start of the word it changed"
        );
    }

    // -----------------------------------------------------------------------
    // `T104` — a tab renders at a tabstop, and `<tab>` does something
    // -----------------------------------------------------------------------

    /// `<tab>`, once, in the running binary. One byte, one frame.
    const TAB: &[u8] = b"\t";

    /// Where a character sits on the drawn row, or `None` if it is not there.
    ///
    /// Columns are absolute screen columns and the gutter is deliberately not
    /// subtracted: the assertions below compare two rows of *one* frame against
    /// each other, so whatever the gutter costs cancels, and a test that
    /// hardcoded its width would break on a change that has nothing to do with
    /// tabs.
    fn column_of(screen: &Screen, row: u16, character: char) -> Option<usize> {
        screen
            .line(row)
            .chars()
            .position(|drawn| drawn == character)
    }

    /// **The report, through the shipping binary.** *"tab only seems to go a
    /// space at a time when indenting"* — a file whose lines begin with a real
    /// `\t` drew one column of indent per level, because the renderer replaced
    /// every tab with a single space.
    ///
    /// The assertion is a **comparison between rows of one frame**, which is
    /// what makes it a tabstop test rather than an arithmetic one: row 0 is
    /// four literal spaces and row 1 is one `\t`, so a build that draws tabs
    /// correctly puts both `x`s in the same column and the old build put them
    /// three apart. Row 2 is the same tab after three characters — a *fixed*
    /// four-space expansion would push it to column 7 where the stop is 4.
    #[test]
    fn a_file_of_tabs_draws_at_the_tabstop() {
        let scratch = Scratch::new("tab-render");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "    x\n\tx\nabc\tx\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let screen = editor.screen();
        let spaces = column_of(&screen, 0, 'x').expect("row 0 draws its x");
        let tabbed = column_of(&screen, 1, 'x').expect("row 1 draws its x");
        let after_abc = column_of(&screen, 2, 'x').expect("row 2 draws its x");
        editor.quit();

        assert_eq!(
            tabbed,
            spaces,
            "a leading tab and four leading spaces have to reach the same column; \
             rows were {:?} and {:?}",
            screen.line(0),
            screen.line(1)
        );
        assert_eq!(
            after_abc,
            spaces,
            "a tab three characters in finishes the column it starts in rather than \
             adding a fixed four; row was {:?}",
            screen.line(2)
        );
    }

    /// **`<tab>` in insert mode advances to the tabstop**, read off the saved
    /// file rather than off the frame — `press_quietly`'s doc says why a frame
    /// is the wrong place to look for an edit, and a file is unambiguous about
    /// *how many* spaces landed.
    ///
    /// Two presses at two columns, because one cannot tell a tabstop from a
    /// substitution: at column 0 a stop and a fixed four-space expansion agree,
    /// and at column 2 they do not.
    ///
    /// This also proves the key is **bound**. Unbound it reaches
    /// `Machine::insert_key`'s literal `"\t"` arm and the file holds a tab
    /// character, which is neither of the strings below.
    #[test]
    fn tab_in_insert_mode_advances_to_the_tabstop() {
        let scratch = Scratch::new("tab-insert");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "ab\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // At the end of `ab`: two cells to the stop.
        editor.press(b"A");
        editor.press(TAB);
        editor.press(b"x");
        editor.press(b"\x1b");
        // And at the start of the line: a whole stop.
        editor.press(b"I");
        editor.press(TAB);
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "    ab  x\n",
            "<tab> types the cells left to the next stop, not a fixed count"
        );
    }

    /// **Cells, not characters.** `漢` is one `char` and two columns, so the
    /// stop after it is two cells away. A column counted in `char`s types three
    /// spaces here, which is the confusion this repo has shipped three bugs
    /// from — and it is invisible in every ASCII test above.
    #[test]
    fn a_tab_after_cjk_advances_by_cells_in_the_running_binary() {
        let scratch = Scratch::new("tab-cjk");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "漢\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"A");
        editor.press(TAB);
        editor.press(b"x");
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "漢  x\n",
            "the stop after a two-cell character is two cells away, not three"
        );
    }

    /// **`R` is still vim's `R` when the key is `<tab>`** — the third key this
    /// window has had to teach it, after `<space>` and `<cr>`.
    ///
    /// Same seam as `replace_mode_still_overwrites_with_space_bound_in_the_
    /// insert_scope` above and the same reason it needs a pty: `Scope::of`
    /// folds `EditMode::Replace` into the insert scope, so the `<tab>` row
    /// binds in `R` too, and only `runtime/keymaps.scm` driving the running
    /// binary says so.
    ///
    /// The expected text is `nvim -u NONE`'s, with
    /// `set expandtab tabstop=4 softtabstop=0`, run this session: `Rx<Tab>`
    /// over `abcdefgh` gives `x···cdefgh` — the tab spends the three cells left
    /// to the stop and consumes one character doing it, so the line grows by
    /// two. A version that spliced leaves the `b` alive and reads
    /// `x   bcdefgh`.
    #[test]
    fn replace_mode_still_overwrites_with_tab_bound_in_the_insert_scope() {
        let scratch = Scratch::new("replace-overwrites-tab");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "abcdefgh\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b"R");
        editor.press(b"x");
        editor.press(TAB);
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "x   cdefgh\n",
            "the tab overwrote `b` and spent the three cells to the stop; it did not push \
             the rest of the line right"
        );
    }

    /// **`>` shifts by the same unit `<tab>` types, and a declaration beats the
    /// option.**
    ///
    /// `>` had an arm and a binding since `T026` and **nothing had ever pressed
    /// it** — `TASKS.md`'s `T104` entry records that no test in
    /// `crates/phosphor/tests/` or `crates/phosphor-core/tests/` types one,
    /// which is how the unit stayed hardcoded inside the vendored fork through
    /// a green gate. This is the first press.
    ///
    /// **Two buffers, one session, one option value**, which is what makes it a
    /// precedence test: `start.txt` is claimed by no declaration and takes
    /// `init.scm`'s four, and `sample.zz` is claimed by a `define-language!`
    /// typed into this session's REPL that says two. A build reading one source
    /// gives the same answer twice.
    ///
    /// The declaration road is `a_language_declared_at_the_repl_is_live_in_the_
    /// same_session`'s, and so is the way it waits: `press_until` on text that
    /// exists only after the step completed, never `press_quietly` — an `:e`
    /// draws the ex line, pauses, then swaps the buffer, and settling returns
    /// in the gap.
    #[test]
    fn the_shift_operator_shifts_by_the_unit_a_declaration_named() {
        let scratch = Scratch::new("shift-unit");
        let runtime = copy_layer(&scratch.path);
        let plain = scratch.path.join("start.txt");
        fs::write(&plain, "nothing to do with it\n").expect("a fixture");
        let declared = scratch.path.join("sample.zz");
        fs::write(&declared, "local x = 1\n").expect("a fixture");

        let editor = Editor::open(&plain, &scratch.state(), &runtime);
        // No declaration claims `.txt`, so this is the global answer. Counted
        // presses, not `press_until`: a frame is a **diff**, and a shift
        // redraws only the columns that moved — the whole shifted line is
        // never on the wire to match against. `.txt` declares no server, so
        // nothing else is drawing and one frame per key holds.
        editor.press(b">");
        editor.press(b">");
        editor.press(b":w\r");

        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            b"(define-language! \"zz\" (hash \"extensions\" (list \"zz\") \
              \"grammar\" void \"lsp_command\" (list) \"comment_prefix\" void \
              \"indent\" \"  \"))\r",
            "#ok",
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");
        editor.press_until(
            format!(":e {}\r", declared.display()).as_bytes(),
            "local x = 1",
        );
        editor.press(b">");
        editor.press(b">");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&plain).expect("the file survives"),
            "    nothing to do with it\n",
            "a buffer no declaration claims took init.scm's four"
        );
        assert_eq!(
            fs::read_to_string(&declared).expect("the file survives"),
            "  local x = 1\n",
            "and the declaration's own two beat it, in the same session"
        );
    }

    /// **The option is live from the REPL, not only at boot.**
    ///
    /// `T037` shipped a bug where a table was read once and cached, and
    /// `T101`'s review caught the same shape a second time — so the assertion
    /// worth making is not *"the option works"* but *"the option works after
    /// the editor has already drawn frames with the old value"*. The first
    /// `>>` here is against the shipped four; the second is against eight, set
    /// by typing at the REPL in the same session. A width read once at boot
    /// puts four on both lines.
    #[test]
    fn the_tab_width_is_live_from_the_repl() {
        let scratch = Scratch::new("tab-width-live");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "a\nb\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press(b">");
        editor.press(b">");

        editor.press_until(b":repl\r", "steel");
        editor.press_until(b"(set-option! \"tab-width\" 8)\r", "#ok");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        editor.press(b"j");
        editor.press(b">");
        editor.press(b">");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "    a\n        b\n",
            "the second shift took the eight typed at the REPL"
        );
    }

    impl Editor {
        /// [`Editor::open`] with `$PHOSPHOR_KEYBOARD` forced, which is the only
        /// way to test both sides of `T027` on one machine.
        fn open_forced(file: &Path, state: &Path, runtime: &Path, keyboard: &str) -> Self {
            let binary = PathBuf::from(env!("CARGO_BIN_EXE_phosphor"));
            let (master, slave_path) = open_pty();
            let slave = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&slave_path)
                .expect("the pty slave opens");
            tcsetwinsize(&slave, SCREEN).expect("the pty takes a window size");

            let child = Command::new(binary)
                .arg(file)
                .env("PHOSPHOR_RUNTIME", runtime)
                .env("XDG_STATE_HOME", state)
                .env("XDG_CONFIG_HOME", config_home(state))
                .env("PHOSPHOR_KEYBOARD", keyboard)
                .env("TERM", "xterm-256color")
                .stdin(Stdio::from(slave.try_clone().expect("the slave clones")))
                .stdout(Stdio::from(slave.try_clone().expect("the slave clones")))
                .stderr(Stdio::from(slave))
                .spawn()
                .expect("the shipping binary starts");

            let transcript = Arc::new(Mutex::new(Vec::new()));
            let frames = Arc::new(AtomicU64::new(0));
            let reader = spawn_reader(
                Arc::clone(&master),
                Arc::clone(&transcript),
                Arc::clone(&frames),
            );
            let editor = Self {
                master,
                child,
                transcript,
                frames,
                accounted: AtomicU64::new(1),
                reader: Some(reader),
            };
            editor.await_frames(1);
            editor
        }
    }

    // -----------------------------------------------------------------------
    // `S4` — the language server, from a keystroke
    // -----------------------------------------------------------------------

    /// The toy server, as `runtime/languages/` would declare it.
    ///
    /// **Declared, not configured.** This is the same `define-language!` the
    /// shipped twelve are, appended to the **config home's** `persisted.scm` —
    /// which `T101` moved out of the runtime tree and which the binary loads
    /// after the whole boot order, and which is what the REPL writes into. So
    /// every one of these tests is `CP-4`'s manual half run from a pty: a
    /// thirteenth language, its server, and its comment syntax, with no Rust
    /// in the path.
    ///
    /// `mode` picks which half of the server runs; see the fixture's header for
    /// why one process cannot do both under a frame-counting harness.
    fn declare_toy(scratch: &Scratch, mode: &str) {
        declare_toy_logging(scratch, mode, None);
    }

    /// The same declaration, with the server told to record every completion
    /// request it receives.
    ///
    /// **Only the debounce test passes a log**, and the reason is that the
    /// thing it tests is invisible in a frame: a 250ms pause and no pause at
    /// all draw the same float, and what separates them is how many times the
    /// server was asked. Every other toy test passes [`None`] and the fixture
    /// writes nothing.
    fn declare_toy_logging(scratch: &Scratch, mode: &str, log: Option<&Path>) {
        let server = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/toy_language_server.py")
            .canonicalize()
            .expect("the toy server is beside this file");
        // `.to_string()` before `{:?}` for the same reason the server path
        // below does it: `Path::Display`'s Debug is not its Display, and a
        // quoted-and-escaped *string* is what the scheme list needs.
        let recording = log.map_or_else(String::new, |log| {
            format!(" {:?}", log.display().to_string())
        });
        let form = format!(
            "(define-language! \"toy\"\n  (hash \"extensions\" '(\"toy\")\n        \
             \"grammar\" void\n        \"lsp_command\" (list \"python3\" {:?} {mode:?}{recording})\n        \
             \"comment_prefix\" \";\"))\n",
            server.display().to_string(),
        );
        let persisted = scratch.persisted().join("persisted.scm");
        let mut existing = fs::read_to_string(&persisted).unwrap_or_default();
        existing.push_str(&form);
        fs::write(&persisted, existing).expect("the config home takes a declaration");
    }

    /// Waits until `wanted` has been drawn **at any point this session**, then
    /// settles and hands back everything drawn.
    ///
    /// **Not [`Editor::press_until`], and the difference is a race that one
    /// cannot avoid.** That waits for text to appear *since a mark*, which is
    /// right for a key's own frame and wrong for a chip that changes in place:
    /// a frame is a diff, so `starting …` becoming `✓` repaints the glyph and
    /// skips the name beside it. Whether the name is ever redrawn after the
    /// mark depends on whether the server won its race with the first frame —
    /// a fact about process startup rather than about the editor, and the
    /// reason this exists. It cost a `just gate` run to find, on a test that
    /// had passed a dozen times.
    ///
    /// `contains` rather than `shows`: the callers here match strings they
    /// wrote themselves, and a fuzzy match on a one-character needle answers
    /// yes to a row of spaces.
    /// Waits for `wanted` on the **composed grid** and hands the grid back as
    /// text.
    ///
    /// [`Editor::shown_on_grid`] without a keystroke, flattened. The pair with
    /// [`shown`] below is the same division `Editor::press_until` and
    /// `Editor::shown_on_grid` draw and for the same reason: a *notice* is
    /// written fresh and lives in the delta, and anything that has *settled* —
    /// a statusline segment, a counter, a refusal that is still on the row —
    /// lives on the grid and may reach the delta in pieces.
    fn shown_on_grid_text(editor: &Editor, wanted: &str) -> String {
        shown_on_grid_any(editor, &[wanted])
    }

    /// The same, satisfied by **any** of `wanted`.
    ///
    /// **For text the shed ladder can respell**, which is a real category and
    /// not a convenience: `§11`'s rungs mean a counter reads `2 unseen` where
    /// there is room and `●2` where there is not, and *which* depends on how
    /// much the rest of the row wants — the language-server chip, the length of
    /// a temp path. A test that needles one spelling is asserting a fact about
    /// the runner. Both spellings are the same claim.
    fn shown_on_grid_any(editor: &Editor, wanted: &[&str]) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let grid = grid_of(&editor.screen());
            if wanted.iter().any(|needle| shows(&grid, needle)) {
                editor.settle();
                return grid_of(&editor.screen());
            }
            assert!(
                Instant::now() < deadline,
                "none of {wanted:?} reached the grid. Grid was:\n{grid}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn shown(editor: &Editor, wanted: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let session = editor.since(0);
            if session.contains(wanted) {
                editor.settle();
                return editor.since(0);
            }
            assert!(
                Instant::now() < deadline,
                "{wanted:?} was never drawn. Drawn this session: {session}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until the toy server has answered `initialize`, and resynchronises
    /// the frame count.
    ///
    /// **The counted-frames discipline and an unsolicited producer cannot both
    /// be true**, and since the repair pass there is a second unsolicited
    /// producer: a server changing state wakes the loop
    /// (`events::AppEvent::Woke`) so the statusline's chip cannot go stale. Two
    /// frames therefore arrive on their own between `open` and the first
    /// keystroke — `starting …` and `toy ✓` — and [`Editor::press`] is entitled
    /// to call that a bug, because for a *keystroke* it is.
    ///
    /// So every `S4` test that counts frames waits here first. What makes that
    /// possible rather than a sleep is the chip itself: `toy ✓` is the server's
    /// own `serverInfo.name` out of its `initialize` reply, so this is *"the
    /// server is up"* stated by the editor rather than guessed at by the test.
    fn ready(editor: &Editor) {
        drop(shown(editor, "toy-lsp \u{2713}"));
    }

    /// A scratch tree with the toy language declared and one `.toy` file in it.
    fn toy(name: &str, mode: &str, contents: &str) -> (Scratch, PathBuf, PathBuf) {
        let scratch = Scratch::new(name);
        let runtime = copy_layer(&scratch.path);
        declare_toy(&scratch, mode);
        let file = scratch.path.join("sample.toy");
        fs::write(&file, contents).expect("a fixture");
        (scratch, runtime, file)
    }

    /// **`T047`'s other half: `gr` fills the picker from a real server.**
    ///
    /// Every hop is the real one — `runtime/keymaps.scm`'s `gr`, the
    /// `request-references` capability, `LanguageServers::ask` over a pipe to a
    /// real process, the answer arriving on another thread, the `references`
    /// slot, an `open-picker` posted from the callback, and the shipped
    /// `references` source drawing what it was handed. Break any one and this
    /// goes red.
    ///
    /// **Three places in two files**, which is what separates this from `gd`:
    /// one place could be answered by opening it, and `8a` exists because a
    /// list needs a surface.
    #[test]
    fn gr_fills_the_picker_from_a_real_server() {
        let (scratch, runtime, file) = toy("gr-references", "definition", "one\ntwo\nthree\n");
        fs::write(scratch.path.join("target.toy"), "a\nb\n").expect("the sibling");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        drop(shown(&editor, "toy-lsp \u{2713}"));

        // `gr` — "what uses this".
        let listed = editor.press_until(b"gr", "3/3");
        editor.quit();

        // **`3/3` is the assertion.** The rows carry absolute paths — a
        // `Scratch` is under the system temp directory and the editor's cwd is
        // the repo, so `key_for` cannot strip the prefix — and at this float's
        // width they truncate before the filename. That is correct behaviour
        // (§11: clip, never wrap) and it makes the row text a fact about the
        // tempdir. The count is a fact about the server: three places in, three
        // rows out.
        assert!(
            shows(&listed, "3/3"),
            "the server's three places became three picker rows; frame was: {listed}"
        );
        assert!(
            shows(&listed, "references"),
            "drawn by the references source, named in the float header; frame was: {listed}"
        );
    }

    /// **A reference row lands on the column the server named, not on 1.**
    ///
    /// Reported from the running editor: `gr` puts you on the right *line* and
    /// the wrong *column*. Nothing here could have caught it — every place the
    /// toy server answers starts at character 0 but one, and what the picker
    /// tests assert is the statusline's `3:1`, which is also what a dropped
    /// column draws.
    ///
    /// So the assertion is the **file**, for the reason
    /// `a_jump_inside_the_open_file_moves_the_cursor_and_keeps_the_edits`
    /// gives: `x` takes the character under the cursor, and which character
    /// that was is a fact on disk rather than a diff on a frame. Line 3 is
    /// `wxyzQrst` and the server's second place is character 4 — so a cursor
    /// on the column it named deletes the `Q`, and one snapped to the start of
    /// the line deletes the `w`. The two files differ in their first byte.
    #[test]
    fn a_reference_row_lands_on_the_column_the_server_named() {
        let (scratch, runtime, file) = toy("gr-column", "definition", "one\ntwo\nwxyzQrst\n");
        fs::write(scratch.path.join("target.toy"), "a\nb\n").expect("the sibling");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        drop(shown(&editor, "toy-lsp \u{2713}"));
        editor.press_until(b"gr", "3/3");

        // The three places are `sample.toy` line 1, `sample.toy` line 3 and
        // `target.toy` line 2. `:3` filters rather than moving the selection,
        // matching the picker tests above, and it is unambiguous here: a
        // scratch path holds no colon, so the only `3` that can follow one is
        // a line number.
        // **`<C-n>` rather than a filter**, which is what the second row needs
        // and nothing more. This typed `:3` at first and waited for `1/3`, and
        // it went red on CI showing `3/3…` — the matcher had not finished, and
        // `picker.rs`'s notify was a no-op so nothing drew again. That is fixed
        // and this still should not go through the matcher: a test about which
        // *column* a row carries has no business depending on how a fuzzy
        // filter scores a temporary directory's name.
        //
        // `picker_key` binds `<C-n>`/`<C-p>` and `Down`/`Up` to
        // `matcher.select(±1)`, all four synchronous, and the rows are sorted
        // by path — so one press is the second place the server named.
        editor.press_until(b"\x0e\r", "wxyzQrst");
        editor.press_quietly(b"x");
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "one\ntwo\nwxyzrst\n",
            "`x` took the character at the column the server named"
        );
    }

    /// The same claim for `gd`, which does not go through a picker at all.
    ///
    /// `request-definition` posts an `open-file` carrying the whole position,
    /// so this half has a column to lose somewhere else entirely — the two
    /// paths share nothing below `FileSpan`. Worth its own test for that
    /// reason: a fix for one says nothing about the other.
    #[test]
    fn gd_lands_on_the_column_the_server_named() {
        let (scratch, runtime, file) = toy(
            "gd-column",
            "definition-column",
            "the first line\nwxyzQrst\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);

        // `gd` is asynchronous and `x` is not, so the terminal's own cursor is
        // the signal that the answer landed — the same wait, and the same
        // reason, as the same-file jump test further down.
        let before = editor.screen().row;
        editor.press_quietly(b"gd");
        let deadline = Instant::now() + Duration::from_secs(30);
        while editor.screen().row == before {
            assert!(
                Instant::now() < deadline,
                "the jump never moved the cursor off row {before}. Last frame: {}",
                editor.tail()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        editor.press_quietly(b"x");
        editor.press_quietly(b":w\r");
        editor.quit();

        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "the first line\nwxyzrst\n",
            "`x` took the character at the column the server named"
        );
    }

    /// **`T038`'s *done when*, verbatim:** *"screen `7c`'s completion
    /// reproduces **from a keystroke** — typing in insert mode in the running
    /// binary raises the float"*.
    ///
    /// Nothing here presses a completion key. One character is typed into the
    /// buffer, and everything between that character and the frame is real:
    /// the loop notices the edit stream moved in insert mode against a server
    /// that is ready, `LanguageServers` asks it over a pipe to a real process,
    /// the answer comes back on another thread through `crate::events`' queue,
    /// and the composed `Node::Completion` is drawn by the interpreter through
    /// the host's own `Resources`. Break any one of those and this goes red;
    /// nothing here builds a tree.
    #[test]
    fn typing_in_insert_mode_raises_the_completion_float() {
        let (scratch, runtime, file) = toy(
            "typing",
            "completion",
            "let retry = RetryPolicy\nlet base = de\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"j");
        // `A` — append at the end of the line, which is an insert session and
        // not yet an edit.
        editor.press(b"A");

        // **Asked for once explicitly, and this is how the test waits for the
        // child process to finish `initialize`.** A `<C-x>` is queued into the
        // server's own task and answered when it is ready; a *typed* trigger
        // is edge-triggered on the edit, so one that landed a millisecond
        // early would be dropped and this test would hang on a wiring that
        // works. Dismissed again, so what follows starts from a closed float.
        editor.press_until(b"\x18", "default_delay");
        editor.press_quietly(b"\x05");

        // …and this is the edit that raises it. One character, no completion
        // key.
        let frame = editor.press_until(b"f", "default_delay");
        assert!(
            shows(&frame, "fn() -> RetryPolicy"),
            "`7c` draws a meta detail column right of the labels; frame was: {frame}"
        );
        assert!(
            shows(&frame, "3 attempts"),
            "the selected row's documentation sits under a rule; frame was: {frame}"
        );
        editor.quit();
    }

    /// The float is not only drawn — it is **driven**, by the keys `7c` cannot
    /// show because a passive float has no footer.
    ///
    /// `<C-x>` asks, `<C-n>` moves the selection, `<C-y>` accepts. The proof
    /// the selection moved is the *prose*: the block under the rule is per
    /// item, so a selection that did not move would leave the first row's
    /// sentence on screen. The proof `<C-y>` accepted the **selected** row and
    /// not a fixed one is the buffer — `index 0` means the selection, and a
    /// literal row number would have written `default()`.
    #[test]
    fn the_completion_keys_move_the_selection_and_write_the_buffer() {
        let (scratch, runtime, file) = toy("accept", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "3 attempts");
        editor.press_until(b"\x0e", "The base delay");
        // Accepted quietly: the line it writes shares its first ten characters
        // with the line that was there, so the frame is a diff of the suffix
        // alone. The file is the assertion.
        editor.press_quietly(b"\x19");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "let base = default_delay\n",
            "the accepted row replaced the prefix under the cursor, and nothing else"
        );
        editor.quit();
    }

    /// **`<C-p>` walks the list back**, and only `<C-n>` had ever been pressed.
    ///
    /// `move-completion` takes a signed delta and these two keys are the two
    /// signs — one arm, and half of it unexercised. The prose under the rule is
    /// per item for the reason the test above gives, so it is also what says a
    /// selection came *back*: `<C-n>` then `<C-p>` has to leave the first row's
    /// sentence on screen, and a `<C-p>` wired to the wrong sign would leave
    /// the third.
    #[test]
    fn ctrl_p_walks_the_completion_list_back() {
        let (scratch, runtime, file) = toy("completion-prev", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "3 attempts");
        editor.press_until(b"\x0e", "The base delay");
        let back = editor.press_until(b"\x10", "3 attempts");
        editor.press_quietly(b"\x1b");
        editor.quit();
        assert!(
            shows(&back, "3 attempts"),
            "<C-p> put the first row's prose back, so the delta's sign is \
             wired both ways; frame was: {back}"
        );
    }

    /// **`CP-4`, verbatim:** *"i like being able to hit space to select and put
    /// a space after or enter to select without a space after"*.
    ///
    /// Four tests, because the two keys have two behaviours each and the pair
    /// nobody asked for is the pair that decides whether the keys are usable.
    /// `T038`'s float is raised by **typing**, so it is open for most of the
    /// time you are in insert mode: a `<space>` that accepted whatever was
    /// highlighted would complete a word every time you finished one, and
    /// `<cr>` would stop making newlines. So each key only acts on a row the
    /// user steered to with `<C-n>`, and otherwise types what it always typed.
    ///
    /// This one is the fall-through: **the float is open and untouched**, which
    /// is the state a typist is in for most of a word.
    ///
    /// The assertion is the file rather than a frame, for [`printable`]'s
    /// reason: a frame is a diff, and a space typed at the end of a line
    /// redraws one cell that was already blank.
    #[test]
    fn space_types_a_space_while_the_completion_float_is_untouched() {
        let (scratch, runtime, file) = toy("space-through", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        // The float is up — and stays up, unchosen, under the space.
        editor.press_until(b"\x18", "default_delay");
        editor.press_quietly(b" ");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "let base = def \n",
            "space over an unchosen list types a space — it does not accept"
        );
    }

    /// …and once a row **has** been chosen, the same key accepts it and leaves
    /// a space behind, which is the half Teej asked for.
    ///
    /// The row is the *second* one, so a host that accepted a fixed row would
    /// write `default()` and this would read it.
    #[test]
    fn space_accepts_a_chosen_completion_and_leaves_a_space() {
        let (scratch, runtime, file) = toy("space-accept", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "3 attempts");
        // The prose under the rule is per item, so this is the wait for the
        // selection having actually moved rather than for the key having been
        // written to the pty.
        editor.press_until(b"\x0e", "The base delay");
        editor.press_quietly(b" ");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "let base = default_delay \n",
            "space accepted the row `<C-n>` chose, and left one space after it"
        );
    }

    /// The same pair for `<cr>`, whose fall-through is a newline — and losing
    /// newlines in insert mode is the failure that makes this guard worth its
    /// complexity.
    #[test]
    fn enter_makes_a_newline_while_the_completion_float_is_untouched() {
        let (scratch, runtime, file) = toy("enter-through", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "default_delay");
        editor.press_quietly(b"\r");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "let base = def\n\n",
            "enter over an unchosen list still splits the line"
        );
    }

    /// …and on a chosen row it accepts with **no** space, which is the whole
    /// difference between the two keys.
    ///
    /// Read against the `<space>` test above rather than alone: the same
    /// keystrokes with the same fixture differ by exactly one trailing space,
    /// so `then` is pinned from both sides.
    #[test]
    fn enter_accepts_a_chosen_completion_with_no_space_after() {
        let (scratch, runtime, file) = toy("enter-accept", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "3 attempts");
        editor.press_until(b"\x0e", "The base delay");
        editor.press_quietly(b"\r");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "let base = default_delay\n",
            "enter accepted rather than splitting the line, and added nothing after"
        );
    }

    /// **`<tab>` steps the list, and this is the sequence `CP-4` reported.**
    ///
    /// Teej, at the keyboard, on a float that was up with nothing highlighted:
    /// *"in this form i should be able to hit tab or something to select"*, and
    /// *"enter or space doesnt accept"*. Both halves are one fact — nothing had
    /// been **chosen**, so `select = false` held and the two accept keys
    /// correctly fell through — and the missing piece was a comfortable key to
    /// choose with. Helix binds `Tab` to the same `move_down()` as `C-n` and its
    /// menu cursor starts at `None`, so the first press lands on row 0; that is
    /// what this presses.
    ///
    /// The `<cr>` afterwards is the point rather than a detail: it is the key
    /// the report says *"doesn't accept"*, and after one `<tab>` it does.
    #[test]
    fn tab_steps_the_completion_list_and_then_enter_accepts() {
        let (scratch, runtime, file) = toy("tab-step", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "default_delay");
        // One `<tab>`: from nothing chosen to row 0, exactly as `<C-n>` would.
        editor.press_until(TAB, "The base delay");
        editor.press_quietly(b"\r");
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "let base = default_delay\n",
            "<tab> chose the first row and <cr> took it"
        );
    }

    /// **A burst of typing costs one request, not one per character.**
    ///
    /// `CP-4`, from Teej at the keyboard: *"completion seemed to take longer
    /// than it should have"*. The cause was that there was no timer — the only
    /// gate was one-request-in-flight, so typing sent a request, waited a whole
    /// server round trip, sent the next, and every list drawn was about a
    /// prefix the cursor had already left. [`COMPLETION_DEBOUNCE`] is helix's
    /// 250ms, and what it changes is invisible in a frame: the float looks the
    /// same either way. So this counts what reached the server.
    ///
    /// **Nine characters in one write**, which is the burst the `Outstanding`
    /// docs describe reproducing against a real rust-analyzer. Written as one
    /// `press` so the pty delivers them together and the loop coalesces them,
    /// which is what human typing looks like from in here.
    ///
    /// The assertion is `<= 2` rather than `== 1`, and the bound is what makes
    /// it a debounce test rather than a timing test: one request is the
    /// intended outcome, and a second is legitimate if the deadline happened to
    /// fall mid-burst on a loaded machine.
    ///
    /// **Watched going red on `COMPLETION_DEBOUNCE = 0`, and it does not fail
    /// on the count — it fails on the wait, which is the report itself.** With
    /// no pause the nine edits each ask as fast as the round trip allows, every
    /// answer is about a prefix the cursor has already left, the `at` guard in
    /// the ingest arm drops it, and `default_delay` is *never drawn for the
    /// word that was typed* — the 30s `press_until` deadline is what goes off.
    /// That is exactly *"completion seemed to take longer than it should have"*
    /// reproduced: not a slow server, a list that never catches up. The count
    /// below is the tighter statement of the same thing, and it is what stops
    /// this passing again for some other reason.
    #[test]
    fn a_burst_of_typing_asks_the_server_once_rather_than_once_per_character() {
        let scratch = Scratch::new("debounce");
        let runtime = copy_layer(&scratch.path);
        let asked = scratch.path.join("completion-requests.log");
        declare_toy_logging(&scratch, "completion", Some(&asked));
        let file = scratch.path.join("sample.toy");
        fs::write(&file, "let base = d\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        // Wait for the server to be through `initialize`, then start counting
        // from a known point: this explicit ask is one request and it is
        // subtracted below.
        editor.press_until(b"\x18", "default_delay");
        editor.press_quietly(b"\x05");
        let explicit = fs::read_to_string(&asked).map_or(0, |log| log.lines().count());

        // The burst. One write, nine characters — and every one of them keeps
        // the word a prefix of `default_delay`, because the *editor* narrows
        // the server's answer (`phosphor_buffer::lsp::narrow`) and a burst that
        // typed past the item would empty the list and close the float, which
        // is a different test failing for a reason that is not the debounce.
        editor.press_until(b"efault_de", "default_delay");
        editor.press_quietly(b"\x1b");
        editor.quit();

        let total = fs::read_to_string(&asked)
            .expect("the server recorded what it was asked")
            .lines()
            .count();
        let burst = total - explicit;
        assert!(
            burst >= 1,
            "the burst still raises a list — {burst} requests for nine characters"
        );
        assert!(
            burst <= 2,
            "nine characters typed together are one pause and so one ask; \
             got {burst} requests, which is a request per keystroke rather than \
             a request per pause"
        );
    }

    /// …and with **no** list open the same key still types one indent level.
    ///
    /// This is the half that makes the fall-through a fall-through rather than
    /// a handover: `move-completion`'s `otherwise` names `insert-indent`, and a
    /// binding that had simply been reassigned to the float would leave `<tab>`
    /// dead in every buffer without a server — which is most of them.
    ///
    /// The fixture is the toy server's language with the cursor somewhere no
    /// completion is offered, so the difference from
    /// `tab_in_insert_mode_advances_to_the_tabstop` is that a server **is**
    /// attached here. That test proves the key in a buffer with no server at
    /// all; this one proves the fall-through is reached rather than the float
    /// merely being absent.
    #[test]
    fn tab_with_no_completion_list_open_types_one_indent_level() {
        let (scratch, runtime, file) = toy("tab-fallthrough", "completion", "ab\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        // `I` puts the cursor at column 0 with no word behind it, so the
        // typing trigger's floor is not met and no float is ever raised.
        editor.press(b"I");
        editor.press(TAB);
        editor.press(b"\x1b");
        editor.press(b":w\r");
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("the buffer was written"),
            "    ab\n",
            "<tab> with no list open types the cells left to the next stop"
        );
    }

    /// **The decoration, from a keystroke** — the `CP-4` half that was *"do we
    /// have plans to decorate the auto complete with things like src and meta
    /// info about each item"*.
    ///
    /// Every column here is a field the protocol has always sent and this
    /// editor used to throw away, and each one has to survive a different part
    /// of the path: `cnst` is `CompletionItemKind` `21` through
    /// `completions_from_lsp`'s twenty-five-arm mapper and
    /// `CompletionKind::abbreviation`; `retry::policy` is
    /// `labelDetails.description`, which the fixture **withholds** unless the
    /// client announced `labelDetailsSupport` — so this is also the test that
    /// the announcement in `initialize_params` is really sent.
    ///
    /// Deprecation is not asserted here and cannot be: it is a style, and a
    /// pty transcript run through [`printable`] has no styles in it. The
    /// golden frame at `crates/phosphor-ui/tests/screen_7c.rs` is where that
    /// lives.
    #[test]
    fn the_completion_list_draws_the_kind_and_source_columns() {
        let (scratch, runtime, file) = toy("decorated", "completion", "let base = de\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        let frame = editor.press_until(b"\x18", "default_delay");
        assert!(
            shows(&frame, "cnst"),
            "`default_delay` is `CompletionItemKind` 21, which draws as `cnst`; frame was: {frame}"
        );
        assert!(
            shows(&frame, "retry::policy"),
            "`labelDetails.description` is the `src` column, and the fixture only sends it \
             to a client that announced `labelDetailsSupport`; frame was: {frame}"
        );
        editor.quit();
    }

    /// `<C-e>` dismisses, which is the other half of a float with no footer —
    /// and the half that would otherwise be a list you cannot get rid of
    /// without leaving insert mode.
    #[test]
    fn control_e_dismisses_the_completion_float() {
        let (scratch, runtime, file) = toy("dismiss", "completion", "let base = def\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "default_delay");

        let mark = editor.mark();
        editor.press_quietly(b"\x05");
        // Every cell the list held is redrawn as the code behind it, so the
        // labels are gone from the frames drawn *after* the key.
        let after = editor.since(mark);
        assert!(
            !shows(&after, "default_delay"),
            "the list is still on screen after `<C-e>`; frames were: {after}"
        );
        editor.quit();
    }

    /// `T039` — signature help and hover are one surface, reached by two keys.
    ///
    /// `<C-s>` in insert asks what the call takes; `K` in normal asks what is
    /// under the cursor, which is the meaning vim's `K` already had.
    #[test]
    fn signature_help_and_hover_reach_the_same_passive_float() {
        let (scratch, runtime, file) = toy("signature", "completion", "retry(\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        // The active parameter is drawn in its own style, and `printable`
        // puts a space between style runs — so the assertion is the run, not
        // the whole line. `how many times` is the documentation under the rule,
        // which is what makes this the signature *body* and not a bare label.
        let frame = editor.press_until(b"\x13", "policy: RetryPolicy");
        assert!(
            shows(&frame, "how many times, and how far apart"),
            "frame was: {frame}"
        );
        editor.press(b"\x1b");
        editor.press_until(b"K", "a toy hover answer");
        editor.quit();
    }

    /// **Signature help survives the argument you type under it**, and is
    /// dismissed by leaving the insert session that raised it.
    ///
    /// The rule was *"the next key is the answer to have you read it"* for both
    /// features at once. That is right for hover in normal mode and exactly
    /// backwards for signature help: `7c` is captioned *"lsp completion +
    /// signature help"*, the float exists to be read **while the arguments are
    /// typed**, and at `CP-4` the first character of the first argument cleared
    /// it — `<C-s>` inside `add(` drew `fn add(left: i32, right: i32)` with
    /// `left: i32` in the active tone, and typing `1` left an empty frame.
    ///
    /// **Read as an erasure, not as a presence.** A frame is a diff: a float
    /// that is still up is not redrawn, so *"is it still there"* cannot be
    /// asked directly. What a dismissal does is repaint the rows underneath —
    /// so the fixture puts a word there and the assertion is that the word does
    /// not come back. The token is the **tail** of that word, because the float
    /// hangs off the cursor's column and repaints only the cells it covered.
    #[test]
    fn signature_help_survives_the_argument_being_typed_under_it() {
        let (scratch, runtime, file) = toy(
            "signature-typing",
            "completion",
            "retry(\nUNDERNEATH_THE_FLOAT\nand another line\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x13", "policy: RetryPolicy");

        // A digit matches no completion label, so nothing else opens over it —
        // this is the signature float alone.
        let mark = editor.mark();
        editor.press_quietly(b"1");
        editor.settle();
        let typed = editor.since(mark);
        assert!(
            !shows(&typed, "THE_FLOAT"),
            "the row under the float was repainted, so the float was dismissed \
             by the argument it exists to help type; frames were: {typed}"
        );

        // …and `esc` — the key that ends the insert session — does dismiss it.
        let closed = editor.press_until(b"\x1b", "THE_FLOAT");
        assert!(
            !shows(&closed, "policy: RetryPolicy"),
            "the signature is still on screen after leaving insert; frames were: {closed}"
        );
        editor.quit();
    }

    /// `T040` — a **real** publish, arriving unasked on the queue, reaching the
    /// screen.
    ///
    /// Nothing presses a key for this one and that is the property: a server
    /// publishes when it has something, and the editor was parked in
    /// `Queue::recv` with no timeout and no tick. The `■` is §2's lexicon; the
    /// state column beside it is `gutter::state_column`, computed once by the
    /// loop.
    ///
    /// **What arrives unasked is the count, and the sentence is one motion
    /// away.** This test asserted the row itself until `CP-4`, where eleven
    /// cascade parse errors from a half-typed line stacked eleven rows and
    /// pushed the code off the screen. `RowPolicy` bounds them to the cursor's
    /// line, so the file's diagnostic — the fixture puts it on line 2 and the
    /// cursor opens on line 1 — reaches the screen through the statusline's
    /// `■ n` rather than through a row.
    ///
    /// So both halves are pressed here, in order, and they are the argument
    /// that this is quieting rather than hiding: the glyph with **no key at
    /// all**, and then the server's own sentence after a single `j`.
    ///
    /// **The sentinel is `■` and not `■1`, and the reason is a finding worth
    /// keeping.** This harness opens its fixture by absolute path, and that
    /// path is a 110-character temp directory — so at this terminal's 120
    /// columns the statusline is genuinely out of room, §11's ladder does its
    /// job, and the *count* is one of the things it gives up. Probed rather
    /// than guessed: the row reads
    /// `" N  /var/folders/…/sample.toy  │ toy-lsp ✓"`, with even the mode word
    /// already contracted to `N`.
    ///
    /// That is the ladder working, not a defect — `file` is the last rung, so
    /// a monstrous path outlives everything by design. It does mean **this
    /// test cannot assert the count**, and asserting it here is what a first
    /// draft did. The count's own visibility is asserted where width is
    /// controllable, in `compose.rs`'s
    /// `the_diagnostic_counters_never_take_a_second_row`, at 80, 120 and 200
    /// columns against a path a person might actually have.
    #[test]
    fn a_published_diagnostic_reaches_the_screen_with_nobody_asking() {
        let (scratch, runtime, file) = toy(
            "diagnostics",
            "diagnostics",
            "let retry = RetryPolicy\nbase = 3\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // No key is pressed. The editor is idle and the server pushes into the
        // same queue the keyboard uses.
        let frame = editor.press_until(b"", "\u{25a0}");
        assert!(
            !shows(&frame, "expected Duration, found u128"),
            "the cursor is on line 1 and the diagnostic is on line 2, so the \
             sentence has not interrupted anything yet; frame was: {frame}"
        );

        // …and one motion onto its line is what makes it speak.
        let arrived = editor.press_until(b"j", "expected Duration, found u128");
        assert!(
            shows(&arrived, "\u{25a0}"),
            "§2's lexicon opens a diagnostic row with a filled square; frame was: {arrived}"
        );
        editor.quit();
    }

    /// **Eleven cascade errors on one line do not bury the file** — `CP-4`,
    /// in the running binary.
    ///
    /// The screenshot that produced this: a half-typed `path:` in
    /// `crates/phosphor/src/main.rs`, rust-analyzer answering with eleven
    /// `Syntax Error: expected …` diagnostics, and every one drawn as its own
    /// row. `phosphor-ui` bounds them and is unit-tested for it; this is the
    /// half that proves the **host** passes a bounded policy rather than the
    /// default-constructed one, which is the composition defect this repo has
    /// shipped four times.
    #[test]
    fn a_pile_of_diagnostics_on_one_line_does_not_bury_the_buffer() {
        let (scratch, runtime, file) = toy(
            "diagnostic-pile",
            "diagnostic-cascade",
            "let retry = RetryPolicy\nbase = 3\nlet tail = 9\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Onto the line the cascade is about, so the rows are drawn at all.
        let frame = editor.press_until(b"j", "expected COMMA");
        let rows = frame
            .lines()
            .filter(|line| line.contains('\u{25a0}') && line.contains("expected"))
            .count();
        assert!(
            rows <= 4,
            "three rows and an overflow at most, not one per error; \
             {rows} were drawn. Frame was: {frame}"
        );
        assert!(
            shows(&frame, "more here"),
            "and the ones that did not draw are said, not swallowed; \
             frame was: {frame}"
        );
        // The buffer is still on screen, which is the whole complaint.
        assert!(
            shows(&frame, "let tail = 9"),
            "the code below the cascade survived it; frame was: {frame}"
        );
        editor.quit();
    }

    /// **`T040`'s *done when*, and the two words it waited three windows for:
    /// *"against other states"*.**
    ///
    /// The criterion is *"a file with real errors shows correct gutter priority
    /// against other states"*, and until `T041` landed the store there was
    /// exactly **one** source of regions — so the claim had nothing to be
    /// correct against and the task entry says so at length. `T042` and `T087`
    /// gave the column two more, and the loop concatenates diagnostic regions
    /// with the store's before calling `gutter::state_column` **once**. This is
    /// the proof that was missing.
    ///
    /// **One row carries both.** The toy server publishes its error on buffer
    /// line 2; the declaration covers lines 2 **and** 3, so line 2 is trouble
    /// *and* claude-unseen while line 3 is claude-unseen alone. §3's ladder is
    /// *trouble > attention > claude*, so line 2's bar must be the one it
    /// already was.
    ///
    /// **Read as a colour, because that is what a state bar is.** The mark is a
    /// one-cell background in column 0 — `buffer_view`'s
    /// `the_state_bar_is_column_zero_and_carries_the_actor_hue` is the unit
    /// half — so a glyph assertion would be asserting nothing. No literal
    /// appears here: the assertions are *equal to what it was* and *different
    /// from its neighbour*, which is the whole of what a ladder claims and
    /// leaves §1 owning the hues.
    ///
    /// The second assertion is what keeps the first from being vacuous. If the
    /// two states drew the same colour, "unchanged" would pass while proving
    /// nothing.
    #[test]
    fn a_diagnostic_outranks_an_unseen_region_on_the_same_row() {
        let (scratch, runtime, file) = toy(
            "gutter-priority",
            "diagnostics",
            "let retry = RetryPolicy\nbase = 3\nlet tail = 9\nlet end = 0\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // The statusline's count is the signal the diagnostic landed — the
        // inline row needs the cursor on its line and the cursor stays on line
        // 1 throughout, which is also what keeps a cursor-row tint off the two
        // rows being read.
        editor.press_until(b"", "\u{25a0}");
        let trouble = editor.screen().background(1, 0);
        let plain = editor.screen().background(2, 0);
        assert_ne!(
            trouble, plain,
            "the diagnostic painted line 2's state bar in the first place"
        );

        // Lines 2 and 3 — spans are half-open, so `[2:1, 4:1)` is those two.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(2, 4)]).as_bytes(), "landed=1");
        // **Not `"NORMAL"`, which every other `close-repl!` here waits for.**
        // This session's statusline carries a server chip and an unseen count,
        // so §11's ladder contracts the mode to `N` and the word is never
        // drawn. The buffer's own first line is the honest settle: closing the
        // float repaints the rows underneath it, which is exactly the repaint
        // being read.
        editor.press_until(b"(close-repl!)\r", "let retry = RetryPolicy");

        let after = editor.screen();
        let both = after.background(1, 0);
        let unseen = after.background(2, 0);
        editor.quit();

        assert_eq!(
            both, trouble,
            "an unseen region arriving on a row that already had a diagnostic \
             does not lower it: §3 is trouble > attention > claude"
        );
        assert_ne!(
            unseen, plain,
            "line 3 took the declaration, so the two rows differ by their sets \
             and not by whether anything arrived"
        );
        assert_ne!(
            both, unseen,
            "and the two states are distinguishable on screen, which is what \
             makes the assertion above mean something"
        );
    }

    /// `T036` — `gd`, and the arm it needed that nothing had noticed was
    /// missing.
    ///
    /// A definition is a **place**, not text about a place, so it does not come
    /// back through a float: the client answers `Vec<FileSpan>`, the host turns
    /// the first into an `open-file` with a position — which the client could
    /// not, because a `PaneRef` is knowledge it does not have — and the loop
    /// opens it.
    ///
    /// **`open-file`'s `at` was being dropped**, and every caller so far was
    /// `:edit <path>`, which has no opinion about where the cursor lands. So
    /// the assertion is on the *line*, not on the file: the target's second
    /// line is what a jump that honours the position lands on, and its first is
    /// what one that does not lands on.
    #[test]
    fn gd_opens_the_file_the_server_named_at_the_line_it_named() {
        let (scratch, runtime, file) = toy("definition", "completion", "retry\n");
        fs::write(
            scratch.path.join("target.toy"),
            "the first line of the target\nthe definition is on this line\n",
        )
        .expect("a fixture to jump to");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press_until(b"gd", "the definition is on this line");
        // `x` deletes the character under the cursor, which is the only way
        // this harness can ask *where is the cursor* — the answer is in the
        // file it writes.
        editor.press_quietly(b"x");
        editor.press_quietly(b":w\r");
        let written =
            fs::read_to_string(scratch.path.join("target.toy")).expect("the target was written");
        assert_eq!(
            written, "the first line of the target\nhe definition is on this line\n",
            "the cursor landed on the line the server named, not on the first one"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // T041 — the store, and §7's one state machine
    // -----------------------------------------------------------------------

    /// One `declare-regions!` form for `file`, covering `from..to` by line.
    ///
    /// The path is the **absolute** one this harness opens with, and that is
    /// the point rather than an accident: `store::key_for` normalises a
    /// declaration and a lookup the same way, so the two agree whatever form
    /// arrives. A test that declared a relative path here would prove the store
    /// works only for a door that happened to spell it the way the editor did.
    fn declare(file: &Path, spans: &[(u32, u32)]) -> String {
        let regions = spans
            .iter()
            .map(|(from, to)| {
                format!(
                    "(hash \"path\" \"{}\" \"span\" (hash \"start\" (hash \"line\" {from} \
                     \"column\" 1) \"end\" (hash \"line\" {to} \"column\" 1)) \"author\" \
                     \"claude\")",
                    file.display()
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "(string-append \"landed=\" (number->string (declare-regions! (list {regions}))))\r"
        )
    }

    /// `(unseen-count)`, spelled so the answer is a sentinel rather than a
    /// digit.
    ///
    /// `press_until` searches the frame for a substring, and a bare `3` is on
    /// the screen already — it is a line number, a column, a diagnostic count.
    /// `unseen=3` is on the screen only because this query answered it.
    const COUNT: &[u8] = b"(string-append \"unseen=\" (number->string (unseen-count)))\r";

    /// **`T041`, end to end on the shipping binary: a region declared is a
    /// region counted, and a keystroke clears it.**
    ///
    /// Every hop is the real one — `runtime/keymaps.scm`'s `SPC u s`, the input
    /// machine, `Editing::act`, the shared store, and the `unseen-count` query
    /// coming back out through the Steel door. Nothing here composes a
    /// ViewModel or hand-builds a region, which is what `loop_pty`'s own header
    /// means by *"a test that presses a key proves the editor"*.
    ///
    /// The two halves that could not be proved separately are that the *door*
    /// and the *keyboard* reach the same store. `declare-regions!` arrives
    /// through `AppHost::apply` and `SPC u s` through `Editing::act` — two
    /// dispatchers, deliberately, because only one of them has an editor to
    /// resolve `cursor` with — so a build where they held two stores would pass
    /// every unit test in the repository and fail this line.
    #[test]
    fn a_region_declared_at_the_repl_is_counted_and_a_keystroke_clears_it() {
        let scratch = Scratch::new("regions");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // Three regions: lines 2, 3 and 5, each half-open over its own line.
        editor.press_until(
            declare(&file, &[(2, 3), (3, 4), (5, 6)]).as_bytes(),
            "landed=3",
        );
        editor.press_until(COUNT, "unseen=3");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // Line 1, which no region covers. `s` there has to answer *nothing
        // here* rather than mark the nearest thing — a store that widened a
        // miss to the file would read as working right up until it marked five
        // regions the user never looked at.
        editor.press_quietly(b" us");
        editor.press_until(b":repl\r", "steel");
        editor.press_until(COUNT, "unseen=3");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // Line 3, which the second region covers.
        editor.press_quietly(b"jj");
        editor.press_quietly(b" us");
        editor.press_until(b":repl\r", "steel");
        editor.press_until(COUNT, "unseen=2");

        // **And claude revising it puts it back.** §7's third edge, which is
        // the one a count alone cannot see: this re-declares the *same* span,
        // so a store that treated every declaration as new would answer
        // `landed=1` here and `unseen=3` below — the right numbers for the
        // wrong reason, with four regions in a file that has three.
        editor.press_until(declare(&file, &[(3, 4)]).as_bytes(), "landed=1");
        editor.press_until(COUNT, "unseen=3");
        editor.press_until(
            b"(string-append \"total=\" (number->string (+ (unseen-count) (seen-count))))\r",
            "total=3",
        );
        editor.quit();
    }

    /// **§41 — `s` on a line with no region says so.**
    ///
    /// The count stays the value a door reads; what changed is that a *person*
    /// gets a sentence. A `Done` never reaches the notice row
    /// (`answer::trouble` answers `None`), so before this ruling `SPC u s` on a
    /// line no region covers was indistinguishable from the key being unbound —
    /// correct behaviour that reads as a bug, which is the one failure mode a
    /// test can catch and a person cannot argue with.
    /// **`:refresh-vcs` re-reads the repository and says what it found**
    /// (`T071`).
    ///
    /// **The child inherits this runner's working directory**, which is inside
    /// the phosphor checkout — so the repository it detects is a real one and
    /// the chip is a real answer rather than a fixture's. That is deliberate:
    /// a fixture repo would test `detect` against a directory this test made,
    /// and `detect` is already held to that in `phosphor_vcs`'s own suite
    /// against a bare directory, a nested one and a colocated one.
    ///
    /// What this adds is the half those cannot reach: that a person can type
    /// the command, that it reaches an arm, and that the arm answers by naming
    /// the backend rather than by naming a task.
    #[test]
    fn refresh_vcs_re_reads_the_repository() {
        let scratch = Scratch::new("refresh-vcs");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::in_a_repo(&file, &scratch.state(), &runtime);
        // `git`, because a git worktree marks itself with a `.git` *file*
        // rather than a directory — which `detect` handles, and which is the
        // shape this very checkout has.
        let said = editor.press_until(b":refresh-vcs\r", "git");
        assert!(
            shows(&said, "git"),
            "`:refresh-vcs` names the backend it found; frame was: {said}"
        );
        assert!(
            !shows(&said, "T071"),
            "and it names no task, because the task landed; frame was: {said}"
        );
        editor.quit();
    }

    /// **`SPC r d` draws your buffer against what is on disk** (`T070`, `5b`).
    ///
    /// The fixture is built so a *merge* would be visible: the buffer has a
    /// line disk does not and disk has a line the buffer does not. A surface
    /// that showed both as though they were one file would be the auto-merge
    /// this task's own line forbids, and it would look plausible.
    #[test]
    fn spc_r_d_draws_your_buffer_against_disk() {
        let scratch = Scratch::new("diskdiff");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("both.txt");
        fs::write(&file, "shared one\nmine only\nshared two\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Claude rewrites it underneath, replacing the middle line.
        fs::write(&file, "shared one\ntheirs only\nshared two\n").expect("the write underneath");

        let shown = whole(&editor.shown_on_grid(b" rd", "theirs only"));
        // **The header names which side is which**, which is the one thing two
        // columns cannot say about themselves.
        assert!(
            shows(&shown, "disk ⟷ buffer"),
            "`5b`'s header names both sides; frame was: {shown}"
        );
        assert!(
            shows(&shown, "mine only") && shows(&shown, "theirs only"),
            "both versions are drawn, neither is merged away; frame was: {shown}"
        );

        // **And each is on its own side**, which is the assertion that can
        // actually fail. `DiffBody` puts the *removed* side on the left — its
        // own words: *"a row with text on the left and nothing on the right is
        // a deletion"* — and `5b` draws `buffer · yours` left against
        // `disk · claude` right, so the buffer has to be the diff's *from*
        // side. Swap the two arguments to `similar::TextDiff::from_lines` and
        // you get a perfectly correct diff of the wrong two things: both lines
        // still appear, both assertions above still pass, and the columns are
        // backwards. Nothing but a position check sees it.
        let grid = editor.screen();
        let side = |needle: &str| -> usize {
            (0..SCREEN.ws_row)
                .find_map(|row| grid.line(row).find(needle).map(|at| (row, at)))
                .unwrap_or_else(|| panic!("{needle} is on the screen somewhere"))
                .1
        };
        let half = usize::from(SCREEN.ws_col) / 2;
        assert!(
            side("mine only") < half,
            "your buffer is the left column ({} of {half}); frame was: {shown}",
            side("mine only")
        );
        assert!(
            side("theirs only") >= half,
            "claude's disk copy is the right column ({} of {half}); frame was: {shown}",
            side("theirs only")
        );
        // §5's chip is the surface, not the edit mode — `5b` draws `DISKDIFF`.
        assert!(
            shows(&shown, "diskdiff") || shows(&shown, "DISKDIFF"),
            "the strip names the surface; frame was: {shown}"
        );
        editor.press_quietly(b"\x1b");
        editor.quit();
    }

    /// **`:take-disk` takes all of disk and none of yours** (`T070`).
    ///
    /// The *"no auto-merge"* half of the acceptance, asserted as an absence:
    /// the line that was only ever in the buffer has to be **gone**. A merge
    /// would keep it and still look like a plausible file, which is exactly why
    /// this is asserted rather than eyeballed.
    #[test]
    fn take_disk_takes_all_of_disk_and_none_of_yours() {
        let scratch = Scratch::new("take-disk");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("both.txt");
        fs::write(&file, "shared one\nmine only\nshared two\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        fs::write(&file, "shared one\ntheirs only\nshared two\n").expect("the write underneath");
        editor.press_until(b" rd", "theirs only");

        let taken = whole(&editor.shown_on_grid(b":take-disk\r", "took what"));
        assert!(
            !shows(&taken, "mine only"),
            "nothing of yours survives a `:take-disk`; frame was: {taken}"
        );
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "shared one\ntheirs only\nshared two\n",
            "and disk is untouched — taking it is not writing it"
        );
        editor.quit();
    }

    /// **`:keep-mine` writes your buffer over what claude wrote** (`T070`).
    ///
    /// The mirror of the one above, and the same absence: claude's line has to
    /// be gone from **disk**, because keeping yours means yours is what the
    /// file now says.
    #[test]
    fn keep_mine_writes_your_buffer_over_what_claude_wrote() {
        let scratch = Scratch::new("keep-mine");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("both.txt");
        fs::write(&file, "shared one\nmine only\nshared two\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        fs::write(&file, "shared one\ntheirs only\nshared two\n").expect("the write underneath");
        editor.press_until(b" rd", "theirs only");
        editor.press_until(b":keep-mine\r", "kept your");

        let written = fs::read_to_string(&file).expect("the file survives");
        assert_eq!(
            written, "shared one\nmine only\nshared two\n",
            "your buffer is what the file says now"
        );
        assert!(
            !written.contains("theirs only"),
            "and nothing of claude's was merged into it"
        );
        editor.quit();
    }

    /// **`:ask-claude` with no agent declines rather than choosing** (`T070`).
    ///
    /// The third exit is the one that resolves nothing by itself, so with
    /// nobody to ask it must say so — and it must **not** quietly fall back to
    /// one of the other two. An editor that picked a side because the agent was
    /// missing would be the auto-merge wearing a different hat.
    #[test]
    fn ask_claude_with_no_agent_declines_rather_than_choosing() {
        let scratch = Scratch::new("ask-claude");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("both.txt");
        fs::write(&file, "shared one\nmine only\nshared two\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        fs::write(&file, "shared one\ntheirs only\nshared two\n").expect("the write underneath");
        editor.press_until(b" rd", "theirs only");

        let asked = editor.press_until(b":ask-claude\r", "nobody to ask");
        assert!(
            shows(&asked, "nobody to ask"),
            "no agent is a refusal by name; frame was: {asked}"
        );
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "shared one\ntheirs only\nshared two\n",
            "and neither side was taken while nobody was listening"
        );
        editor.press_quietly(b"\x1b");
        editor.quit();
    }

    /// **`SPC r r` takes what is on disk** (`T069`).
    ///
    /// The narrow half of `1d`: the offer, accepted. No watcher is involved —
    /// `reload-from-disk` reads the file itself — so this test says nothing
    /// about timing and everything about whether the verb works.
    ///
    /// `.txt` on purpose: a `.rs` fixture attaches a grammar and a language
    /// server, and both draw frames of their own that `press` would charge to
    /// the next keystroke.
    #[test]
    fn spc_r_r_takes_what_is_on_disk() {
        let scratch = Scratch::new("reload");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "before one\nbefore two\n").expect("a fixture");

        let editor = Editor::watching(&file, &scratch.state(), &runtime);
        let opened = whole(&editor.screen());
        assert!(
            shows(&opened, "before one"),
            "the buffer opened on what was there; frame was: {opened}"
        );

        // Somebody else writes the file. Claude, a formatter, `git checkout` —
        // the editor cannot tell and does not need to.
        // **Longer than it was, and that is what makes the test able to fail.**
        // With a same-length rewrite, "keep the line" and "jump to the end of
        // the splice" both land on line 3 and the planted defect passes. The
        // fixture has to be able to tell the two apart.
        fs::write(
            &file,
            "after one\nafter two\nafter three\nafter four\nafter five\n",
        )
        .expect("the write underneath");

        // **`SPC r r`, pressed as one literal.** `scripts/key_coverage.py`
        // reads the bytes a test sends, so a binding typed in pieces is a
        // binding it cannot see — which is the whole reason this test exists:
        // the lint asked for it by name when `reload-from-disk` stopped
        // refusing.
        // Somewhere that is not the top, so a reload that dumped the cursor
        // **The expected place is written down, not captured.** Two earlier
        // versions of this assertion read the strip *before* the reload and
        // compared: the first used `Screen::row`, which is the terminal's
        // cursor and does not move under a planted `set_cursor(len)`; the
        // second read the strip but captured it after `press_quietly` had
        // already settled somewhere other than where this test assumed. Both
        // passed against the defect they existed to catch.
        //
        // `1d` draws the buffer position on the strip (`5:9`). This fixture is
        // three lines — two of text and the one a trailing newline makes — so
        // `2j` is line 3, and a reload that keeps the *line* leaves it there
        // while one that jumps to the end of its own splice does not.
        editor.press_quietly(b"2j");
        let place = "3:1";

        let reloaded = whole(&editor.shown_on_grid(b" rr", "after one"));
        // **The text changed and the cursor did not.** `1d`'s caption is
        // *"nothing moves unless you asked"*, and what you asked for was the
        // text: a reload that also threw you to the end of the file would be
        // answering a question nobody put.
        assert!(
            statusline(&editor.screen()).contains(place),
            "a reload replaces the text and leaves the cursor at {place}; strip was: {}",
            statusline(&editor.screen())
        );
        assert!(
            shows(&reloaded, "after one") && shows(&reloaded, "after two"),
            "`SPC r r` took what was on disk; frame was: {reloaded}"
        );
        assert!(
            !shows(&reloaded, "before one"),
            "and the old text is gone rather than merged; frame was: {reloaded}"
        );
        editor.quit();
    }

    /// **Invariant 3, which is the whole of `1d`** (`T069`).
    ///
    /// *"Buffer holds stable; nothing moves unless you asked — indicate, offer
    /// to refresh."* So the assertion is not that the editor recovered
    /// gracefully. It is that **nothing moved**: same cursor row, same cursor
    /// column, same text under it, while the file changed underneath.
    ///
    /// The `✱` is asserted in the same breath, because an editor that held
    /// perfectly still and said nothing would pass the first half of this
    /// screen and fail the point of it.
    ///
    /// **`bursts` is read through `disk-state` rather than counted on screen**,
    /// which is the only place the debouncing claim is observable: `T069`'s
    /// entry calls debouncing load-bearing, and one save has to move that
    /// number by exactly one. A raw `notify` would report the truncate and the
    /// write separately and this would read 2.
    #[test]
    fn a_disk_change_under_the_buffer_moves_nothing() {
        let scratch = Scratch::new("holds-still");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("held.txt");
        let body: String = (1..=40).map(|n| format!("line {n}\n")).collect();
        fs::write(&file, &body).expect("a fixture");

        let editor = Editor::watching(&file, &scratch.state(), &runtime);
        // Somewhere that is neither the top nor the default, so a viewport
        // that reset to either would be visible.
        editor.press_quietly(b"gg");
        editor.press_quietly(b"12j");
        editor.press_quietly(b"4l");

        // **The strip's `line:column`, written down, and *waited* for.**
        //
        // Three things this had to learn, each found by a failure rather than
        // by reading it. `Screen::row` is the *terminal's* cursor and survives
        // a planted `set_cursor(len)`, so an assertion on it passes against the
        // defect it exists to catch — `1d` draws the buffer position on the
        // strip (`5:9`) and that is the honest measure.
        //
        // **Capturing** the expectation is the second trap: reading the strip
        // straight after the three quiet presses returned `1:1` on CI, because
        // they had not landed on it yet, and the test then compared a stale
        // reading to a correct one and failed claiming the cursor had moved.
        // `gg` then `12j` then `4l` is line 13 column 5, so say so.
        //
        // And **waiting** is the third: writing the value down is not enough if
        // the screen is read before it arrives. `shown_on_grid` with no keys
        // waits for the needle, which is the same tool the `✱` below uses.
        let place = "13:5";
        let before = editor.shown_on_grid(b"", place);
        let text = whole(&before);

        // Claude writes the file underneath. Different length as well as
        // different content, so a viewport clamped to the new length would
        // move even if nothing else did.
        let rewritten: String = (1..=12).map(|n| format!("rewritten {n}\n")).collect();
        fs::write(&file, &rewritten).expect("the write underneath");

        // **Wait for the `✱`, not for a duration.** The watcher debounces for
        // 250ms and the filesystem takes what it takes; a sleep here would be
        // the flake this harness exists to avoid.
        let noticed = whole(&editor.shown_on_grid(b"", "disk changed"));
        assert!(
            shows(&noticed, "✱"),
            "§2's glyph is what the strip spends on this; frame was: {noticed}"
        );

        // **The two halves of invariant 3, asserted separately** so a failure
        // says which one broke.
        let after = editor.screen();
        assert!(
            statusline(&after).contains(place),
            "the cursor did not move while the file changed underneath it — \
             was at {place}, strip is now: {}",
            statusline(&after)
        );
        assert!(
            whole(&after).contains("line 13"),
            "the buffer still holds what it held — the reload was offered, not taken"
        );
        assert!(
            !whole(&after).contains("rewritten"),
            "and nothing from disk leaked into it"
        );
        // The text is unchanged apart from the strip, which is what changed on
        // purpose. Compared on the buffer rows alone for that reason.
        // **The row-by-row comparison is gone, and its absence is the
        // finding.**
        //
        // It compared the first twenty rows before and after, and it passed
        // for the wrong reason: `runtime/disk.scm` raised on compose, so no
        // float was ever drawn and the rows matched trivially. With the box
        // fixed it draws — and it is **much wider than `1d`'s**, starting
        // around column thirteen rather than sitting in the top-right corner,
        // because `view/float` produces one size and the mockup draws a small
        // notice. So the comparison could only be kept by asserting that the
        // notice never appears, which is the opposite of this screen.
        //
        // What invariant 3 actually claims is held by the three assertions
        // above and below: the cursor did not move (`13:5` on the strip), the
        // buffer still holds `line 13`, and nothing from disk leaked into it.
        // The geometry gap is recorded at OPEN-QUESTIONS.md §65.
        let _ = &text;

        // **`1d`'s box is actually drawn, and this is the assertion that was
        // missing.** `T069` shipped `runtime/disk.scm` with two faults that
        // only fire when a float *composes* — `view/run` passed two arguments
        // where it takes three, and `view/spans` passed bare runs where it
        // takes `view/span-row`s. Both survived the whole task, because every
        // assertion was about the strip and the buffer and nothing ever opened
        // the box. `CP-8c`'s matrix found them by opening a different float.
        //
        // Asserted on the *grid* rather than the delta: a float is composed
        // once and then sits there, so what matters is that it is on screen.
        let box_shown = whole(&editor.shown_on_grid(b"", "changed on disk"));
        assert!(
            shows(&box_shown, "✱ changed on disk"),
            "`1d`'s box says what happened; grid was: {box_shown}"
        );
        assert!(
            shows(&box_shown, ":reload") && shows(&box_shown, ":diff-disk"),
            "…and offers both ways out, spelled whole (§61); grid was: {box_shown}"
        );

        // **And the editor goes quiet with the box up.**
        //
        // `press_quietly` settles — it waits for 250ms with no frame — so a
        // loop that keeps redrawing fails here on the deadline. That is the
        // claim: this test's own watcher is live, and a version of `1d` that
        // recomposed per frame would never go quiet while a change is pending.
        //
        // **`press` was too strict for it and CI said so.** That helper asserts
        // *exactly one* frame per key byte, and under inotify the watcher
        // legitimately delivers one more — 93 against 92. One extra frame is
        // not a spin; requiring none was asserting that this test's own
        // producer never speaks, which is the opposite of what it is for.
        editor.press_quietly(b"j");

        // **The burst count is asserted in `store.rs`, not here**, and that
        // is a flake this test had before it had a name: querying `disk-state`
        // right after the `✱` appears races the debouncer's own window, so the
        // number read 1 on one run and 2 on the next. It caught a planted
        // `bursts += 2` once and missed it once, which makes it worse than no
        // assertion — a test that reports pass or fail depending on the
        // machine teaches you to ignore it.
        //
        // What is deterministic is the counter itself, and
        // `one_delivery_is_one_burst` in `crate::store` holds that with no
        // clock in it. What is *not* deterministic anywhere is how long a
        // filesystem takes to tell us, which is `notify-debouncer-full`'s
        // property rather than this build's.
        editor.quit();
    }

    #[test]
    fn marking_seen_where_there_is_no_region_says_so() {
        let scratch = Scratch::new("no-region");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // No region has been declared at all, so every line is a miss.
        editor.press_until(b" us", "no region here");

        // And it is *not* a refusal — the store did what was asked and found
        // nothing. A refusal would make `S` over an already-seen block an
        // error, which is the reason §41 ruled against one.
        let mark = editor.mark();
        editor.press_quietly(b" us");
        let frame = editor.since(mark);
        assert!(
            !shows(&frame, "refused"),
            "a miss is a receipt, not a refusal; frame was: {frame}"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `T042` / `CP-5` — `6c`, an anchor followed through a rewrite
    // -----------------------------------------------------------------------

    /// `6c`'s file: two functions, and the marked line belongs to the second.
    ///
    /// **Every line is distinct**, and that is deliberate rather than
    /// incidental. `resolve`'s two tiers can disagree when a file holds the
    /// same text twice — `store/region.rs`'s §43 tests are built on exactly
    /// that — and this test is not about which tier answered. It is about
    /// whether the marker followed the rewrite at all, so the fixture is one
    /// where both tiers agree and only *following* can be observed.
    const REFACTOR_RS: &str = "\
fn helper(base: u64) -> u64 {
    base.saturating_mul(2)
}

fn retry_with_backoff(base: u64) -> u64 {
    let delay = capped(helper(base));
    delay
}
";

    /// Reads one region's start line back out through the door, as a sentinel.
    ///
    /// `at=N` rather than a bare number for the reason [`COUNT`] gives: a digit
    /// is on the screen already — it is a line number, a column, a count — and
    /// `at=10` is on screen only because this query answered it.
    fn region_line(file: &Path) -> String {
        format!(
            "(string-append \"at=\" (number->string (hash-ref (hash-ref (hash-ref \
             (car (unseen-regions \"{}\")) \"span\") \"start\") \"line\")))\r",
            file.display()
        )
    }

    /// **`CP-5`'s owed criterion, and the last one it had: *"anchor-survival
    /// across a real refactor (`6c`)"*, end to end on the shipping binary.**
    ///
    /// The checkpoint's own record called this *"partly"* met and said why: the
    /// tier ladder is covered by unit tests and a property test carries an
    /// anchor through an insertion, but **nothing followed a marker through a
    /// rewrite in the running editor**. That is the gap this closes, and it is
    /// a different claim from every test above it — those prove `resolve` is
    /// correct given a snapshot, and this proves the editor actually takes one
    /// and hands it over when the text underneath a region changes.
    ///
    /// # Every hop is the real one
    ///
    /// The region arrives through `declare-regions!` on `AppHost`'s side of the
    /// door. The rewrite is **four keystroke sequences into the live buffer** —
    /// `gg`, then `O` and a typed comment, four times — going through the input
    /// machine, the operator table and the vendored editor's rope. `reanchor`
    /// is the door verb, which reads `Editing::snapshot` off that same rope,
    /// including whatever the grammar has resolved for it. The answer comes
    /// back out through `unseen-regions`.
    ///
    /// Nothing here builds a `Snapshot`, and that is the point: a
    /// hand-built one is what every existing test of this ladder uses, and a
    /// host that took a *wrong* snapshot — the wrong text, stale syntax, an
    /// off-by-one in the line indexing — would pass all of them.
    ///
    /// # What the numbers are
    ///
    /// The region covers line 6, `let delay = capped(helper(base));`, inside
    /// `retry_with_backoff`. Four lines go in above line 1, so the same text is
    /// line 10 afterwards. A marker that did not follow reads `at=6`; one that
    /// followed reads `at=10`; one that was lost reads `at=6` as well, which is
    /// why the file content is checked too — a rewrite that silently did
    /// nothing would otherwise look like a marker that correctly held still.
    #[test]
    fn an_unseen_marker_follows_its_construct_through_a_rewrite_in_the_running_editor() {
        let scratch = Scratch::new("refactor-6c");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("refactor.rs");
        fs::write(&file, REFACTOR_RS).expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(6, 7)]).as_bytes(), "landed=1");
        editor.press_until(region_line(&file).as_bytes(), "at=6");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // The refactor: four lines of new code above everything, typed.
        editor.press_quietly(b"gg");
        for _ in 0..4 {
            editor.press_quietly(b"O// added above you\x1b");
        }

        // The rewrite really happened — see the doc comment on why this is
        // checked rather than assumed.
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert!(
            written.starts_with("// added above you\n// added above you\n"),
            "four lines went in above the file; it now reads:\n{written}"
        );
        assert!(
            written.contains("\n    let delay = capped(helper(base));\n"),
            "and the marked line itself is untouched:\n{written}"
        );

        // The ladder, run by the editor over its own buffer.
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            format!("(reanchor! \"{}\")\r", file.display()).as_bytes(),
            "moved",
        );
        editor.press_until(region_line(&file).as_bytes(), "at=10");

        // And the marker is still a marker: following a rewrite must not
        // consume it. `CP-5`'s thesis is that the gutter pulls your eye to
        // what you have not read, and a region that quietly went seen on a
        // reanchor would empty the gutter every time an agent edited above you.
        editor.press_until(COUNT, "unseen=1");
        editor.quit();
    }

    /// **§8's degradation, end to end on the shipping binary — the half nothing
    /// covered.**
    ///
    /// When colour is gone a state marker becomes `▎`
    /// (`gutter::state_cell`'s `Fill::Marker` arm), because a bar drawn as one
    /// cell of *background* and no glyph comes out blank on a terminal that
    /// drops the escape — and `CP-5`'s thesis is the markers changing how you
    /// read the file. On that terminal there would be none to read.
    ///
    /// # Why this test exists rather than a unit test
    ///
    /// The chain is six links and five of them were already covered.
    /// `T088`'s collapse verification planted a defeat of the **sixth** — the
    /// binding in `draw` where `phosphor_term::colour_available()` reaches the
    /// interpreter — in a form that kept `state_fill` referenced, and measured
    /// that **all 1387 tests still passed**. `draw` has one caller and nothing
    /// headless reaches it, so the only thing covering that link was a Tier-2
    /// capture, and CI's Tier-2 job captures one screen and does not compare it.
    ///
    /// So this presses the whole chain through a real child on a real pty with
    /// `NO_COLOR=1` set, which is the condition the binary itself asks about.
    #[test]
    fn a_terminal_with_no_colour_draws_the_marker_as_a_glyph() {
        let scratch = Scratch::new("no-colour");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\nfour\nfive\n").expect("a fixture");

        let editor = Editor::degraded(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(declare(&file, &[(2, 3)]).as_bytes(), "landed=1");
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // The marker is in the state column, which is column 0 of the frame.
        // Waiting for it on the grid rather than reading one frame: the
        // declaration lands through the door and the redraw follows it.
        let screen = editor.shown_on_grid(b"", "▎");
        let column: String = (0..SCREEN.ws_row)
            .map(|row| screen.line(row).chars().next().unwrap_or(' '))
            .collect();
        assert!(
            column.contains('▎'),
            "§8 says the marker degrades to `▎` when colour is gone; column 0 \
             reads {column:?}",
        );
        editor.quit();
    }

    /// **§43 — a float surface the editor layer registered, opened by a door.**
    ///
    /// `T093`. `open-float` has always taken a `SurfaceId` documented as *"a
    /// registry key, not a Rust enum"*, and until this ruling **nothing created
    /// an entry and no verb could** — the whole vocabulary had two `define-*`
    /// capabilities and neither was a surface.
    ///
    /// Every hop is the real one: `define-float-surface!` binds a procedure in
    /// the running VM, `open-float!` calls it and puts what it answered on
    /// screen, and the body is scheme this test wrote — no Rust knows the word
    /// `scouted`. That is `T048`'s acceptance rehearsed one task early: a
    /// surface that adds zero lines to `phosphor-ui`.
    /// `6b`-style source, as one line, defining a surface whose body is a
    /// `spans` row. Kept beside the test because the escaping is the fiddly
    /// part: this is scheme inside a Rust string inside a scheme string.
    const SCOUT_SURFACE: &[u8] = br#"(define-float-surface! "scout" "(lambda (a) (view/float (quote informational) void (view/spans (list (view/span-row (list (view/run \"scouted here\" (quote text) (quote plain))) void))) void))")"#;

    #[test]
    fn a_float_surface_defined_in_scheme_opens_from_a_door() {
        let scratch = Scratch::new("float-surface");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        // A procedure of one argument, as the capability's own doc says.
        // The body is `Node::Spans` — `T048`'s escape hatch, and the same one
        // the boot float uses. Signatures were read off the door rather than
        // guessed: `phosphor --eval` names the arity of anything spelled wrong.
        // A raw string cannot hold a `\r`, and a terminal submits on one — a
        // trailing newline types into the REPL and waits for a form that never
        // closes, which is what a truncated frame looks like from the outside.
        editor.press_until(&[SCOUT_SURFACE, b"\r"].concat(), "#ok");
        // **Waited on the float, not on `#ok`.** Opening replaces the REPL with
        // the surface, so the receipt is drawn over before it can be read —
        // which is the verb working, and is exactly what §9's *"opening a
        // second replaces the first"* looks like from the outside.
        editor.press_until(b"(open-float! \"scout\" (hash))\r", "scouted here");

        // §9: `esc` closes top-down.
        let mark = editor.mark();
        editor.press_quietly(b"\x1b");
        let frame = editor.since(mark);
        assert!(
            !shows(&frame, "scouted here"),
            "esc left the float open; frame was: {frame}"
        );
        editor.quit();
    }

    /// **An id that is not a name is refused rather than interpolated.**
    ///
    /// `define-float-surface` is `Allow` on MCP and its id is built into a
    /// `define` form, so an unchecked one is scheme injection from an agent.
    #[test]
    fn a_surface_id_that_is_not_a_name_is_refused() {
        let scratch = Scratch::new("bad-surface-id");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            b"(define-float-surface! \"x) (displayln 1) (define y\" \"(lambda (a) a)\")\r",
            "is not a surface name",
        );
        editor.quit();
    }

    /// **§7's other rule, through the door: your own edits never create
    /// regions.**
    ///
    /// *"the machine tracks claude only"*, and a declaration claiming anyone
    /// else is a no-op the store records rather than an error — so the receipt
    /// says how many were dropped instead of refusing the whole batch. A door
    /// that got a bare `#ok` back would have no way to learn the rule.
    #[test]
    fn a_declaration_that_is_not_claudes_creates_no_region() {
        let scratch = Scratch::new("not-claudes");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        let mine = format!(
            "(declare-regions! (list (hash \"path\" \"{}\" \"span\" (hash \"start\" \
             (hash \"line\" 1 \"column\" 1) \"end\" (hash \"line\" 2 \"column\" 1)) \
             \"author\" \"you\")))\r",
            file.display()
        );
        editor.press_until(mine.as_bytes(), "only claude's writes become regions");
        editor.press_until(COUNT, "unseen=0");
        editor.quit();
    }

    /// `T037`'s locale hook, from a keystroke: `gc` uses the prefix the
    /// **declaration** named, and nothing in Rust knows what it is.
    ///
    /// The toy language declares `;`. A Rust comment table would have to answer
    /// `//` or nothing for a `.toy` file, so this cannot pass by accident.
    #[test]
    fn gc_comments_with_the_prefix_the_declaration_named() {
        let (scratch, runtime, file) = toy("comment", "completion", "base = 3\nbase = 4\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        // `gcgc`, not `gcc`. Doubling an operator is a lookup in
        // operator-pending — the rule that makes `dd` linewise — so the
        // doubled form of a two-key operator is the two keys again. `gcc` is
        // `gc` followed by `c`, which is a *different* operator.
        editor.press(b"g");
        editor.press(b"c");
        editor.press(b"g");
        editor.press(b"c");
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "; base = 3\nbase = 4\n",
            "the prefix came from `define-language!`, and only the cursor's line moved"
        );
        editor.quit();
    }

    /// **The list is narrowed to the word being typed**, and the server is not
    /// what narrows it.
    ///
    /// The fixture answers *the same three items whatever the prefix is* —
    /// its own header says so, and a real server behaves the same way for the
    /// same reason: the protocol says the client filters. At `CP-4` nothing
    /// did, so one `.` against rust-analyzer drew a float over rows 0–28 of 30
    /// with `strict_mul` selected.
    ///
    /// The assertion that matters is the **negative** one: `defaults_for`
    /// alone survives a typed `defaults`, and `default_delay` — which the
    /// server sent, in the same answer — is not on the frame.
    #[test]
    fn the_list_is_narrowed_to_the_prefix_the_server_was_never_told_about() {
        let (scratch, runtime, file) = toy("narrow", "completion", "let base = de\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        // Warm: `<C-x>` is answered once the server is ready, so what follows
        // is not racing `initialize`. All three rows match `de`.
        let wide = editor.press_until(b"\x18", "defaults_for");
        assert!(
            shows(&wide, "default_delay"),
            "every row matching `de` is offered; frame was: {wide}"
        );
        // Dismissed, so what follows is a float drawn from nothing rather than
        // a diff of one — `press_quietly`'s own doc explains why a shrinking
        // list cannot be read off a partial redraw.
        editor.press_quietly(b"\x05");

        // …and typing the rest of the word narrows it, with no completion key
        // pressed at all.
        let narrow = editor.press_until(b"faults", "defaults_for");
        assert!(
            !shows(&narrow, "default_delay"),
            "a row that cannot become `defaults` is still on screen; frames were: {narrow}"
        );
        assert!(
            !shows(&narrow, "Duration"),
            "…nor its detail column; frames were: {narrow}"
        );
        editor.quit();
    }

    /// **Typing does not paint a refusal.** `CP-4` found `lsp: denied to a
    /// producer — only the keyboard asks for this` on the statusline during
    /// ordinary typing, in four independent runs against a real
    /// rust-analyzer — the last content frame of a session ending in that
    /// notice.
    ///
    /// The cause was one slot for one outstanding request: the insert-mode
    /// trigger asks per edit, the newest answer replaced the oldest in the
    /// slot, and every superseded answer arrived unrecognised and was rated
    /// `Deny` as though a server had pushed it. `Outstanding` counts instead.
    ///
    /// A burst is what reproduces it, and **`<C-x>` is interleaved into it on
    /// purpose**: the trigger now asks one at a time, so typing alone no longer
    /// overlaps two requests, while an explicit ask always may — it is the key
    /// that says *ask now*, and it is what keeps this test pointed at the
    /// counting rather than at the gate in front of it. Verified by planting
    /// the old one-slot behaviour (`*owed = 0` in `Outstanding::answers`),
    /// which puts the notice back on the frame verbatim.
    #[test]
    fn a_burst_of_typing_never_says_the_editor_denied_something() {
        let (scratch, runtime, file) = toy("burst", "completion", "let base = de\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b"\x18", "default_delay");
        editor.press_quietly(b"\x05");

        // **Written as one burst**, which is the whole reproduction: a key at a
        // time, each waited on, is ten separate round trips and never overlaps
        // two requests. This is what typing at speed looks like on the wire.
        let mark = editor.mark();
        editor.press_quietly(b"f\x18a\x18u\x18lts_for");
        editor.settle();
        let drawn = editor.since(mark);
        // **The other two notices `deliver` can paint**, spelled out rather
        // than caught with a short `"lsp:"` prefix. That prefix was here and
        // was a flake: `shows` is a fuzzy matcher — a space in the frame
        // matches any wanted character, and only two thirds need to match
        // exactly — so over four characters it needs just two, and the
        // statusline's own `toy-lsp ` chip matched `lsp:` every time a redraw
        // put the chip inside the captured window. Whether it lands there is
        // timing, which is why this passed alone and failed under load.
        //
        // A needle short enough to be ambiguous is worse than no needle: it
        // fails on a correct build and, being fuzzy, could equally pass over a
        // real notice. These three are the whole set `deliver` can produce —
        // the two policy refusals and the vocabulary's own — and each is long
        // enough that two thirds of it cannot come from a chip.
        for notice in [
            "denied to a producer",
            "needs an ask first",
            "not built yet",
        ] {
            assert!(
                !shows(&drawn, notice),
                "typing painted {notice:?}; frames were: {drawn}"
            );
        }
        editor.quit();
    }

    /// **`gd` may not discard unsaved work**, and until `CP-4` it did: the
    /// open-file arm re-read the target from disk with no dirty guard, so a
    /// jump out of an edited buffer threw the edit away with no prompt and no
    /// notice.
    ///
    /// `close-buffer` and `quit` both raise `WouldLoseWork`; this is the third
    /// verb that can lose a buffer, and the first one a *keystroke* reaches
    /// without an ex line.
    #[test]
    fn a_jump_out_of_a_dirty_buffer_refuses_rather_than_discarding_it() {
        let (scratch, runtime, file) = toy("dirty-gd", "completion", "retry\n");
        fs::write(
            scratch.path.join("target.toy"),
            "the first line of the target\nthe definition is on this line\n",
        )
        .expect("a fixture to jump to");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        // Dirty it, and prove it is dirty: `[+]` is the statusline's own word
        // for unsaved work.
        editor.press(b"A");
        editor.press_until(b" edited", "[+]");
        editor.press_quietly(b"\x1b");

        let refused = editor.press_until(b"gd", "unsaved work");
        assert!(
            !shows(&refused, "the definition is on this line"),
            "the jump happened anyway; frames were: {refused}"
        );
        // And the edit is still there to be written.
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "retry edited\n",
            "the edit survived the refused jump"
        );
        editor.quit();
    }

    /// **`OPEN-QUESTIONS.md` §37's experiment, run — two motions on one buffer.**
    ///
    /// §37 records *"the statusline can say `1:1` while the cursor is on line
    /// 2"* as a product defect, blames *"a cache whose key does not include the
    /// cursor"*, and refuses to widen that key on a guess: *"measure first —
    /// two runs, `j` then `gd` on the same buffer"*. These are those two runs,
    /// and the difference between them is the whole question — `j` is handled
    /// on the keystroke, `gd` arrives through the event queue.
    ///
    /// **It reads the grid, and that is the point.** The evidence §37 was
    /// written from is a `press_until` that hung for 30s waiting for `2:1`, and
    /// `press_until` scans the bytes drawn *since* a mark. A statusline going
    /// `1:1` → `2:1` repaints **one cell**, so `"2:1"` is on the screen and
    /// never in the delta — the trap [`Editor::shown_on_grid`] exists for.
    #[test]
    fn the_statusline_says_where_the_cursor_is_after_a_motion_and_after_a_jump() {
        let (scratch, runtime, file) = toy(
            "statusline-position",
            "definition-column",
            "the first line\nwxyzQrst\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);

        // (1) `j` — a motion handled on the keystroke itself.
        let moved = editor.landed_at(b"j", "2:1");
        assert!(
            statusline(&moved).contains("2:1"),
            "`j` moved to line 2 and the statusline says so; it read: {}",
            statusline(&moved)
        );

        // (2) `gd` — a jump arriving through the event queue, from a real
        // server, landing on a column the statusline has never drawn. `2:5` is
        // a sharper needle than `2:1`: line **and** column, neither of them a
        // value already on screen.
        let home = editor.landed_at(b"k", "1:1");
        assert!(
            statusline(&home).contains("1:1"),
            "back at the top; statusline read: {}",
            statusline(&home)
        );
        let jumped = editor.landed_at(b"gd", "2:5");
        editor.quit();
        assert!(
            statusline(&jumped).contains("2:5"),
            "`gd` landed on line 2 column 5 and the statusline says so; it read: {}",
            statusline(&jumped)
        );
    }

    /// The same jump into the file you are **already in** does not re-read it,
    /// so it cannot lose anything — which is `gd`'s common case and the one
    /// the guard above must not break.
    #[test]
    fn a_jump_inside_the_open_file_moves_the_cursor_and_keeps_the_edits() {
        let (scratch, runtime, file) = toy(
            "same-file",
            "definition-here",
            "the first line\nthe definition is on this line\n",
        );
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");
        editor.press_until(b" edited", "[+]");
        editor.press_quietly(b"\x1b");

        // The server answers with *this* file, line 2. **The file is the
        // assertion, not the frame**: a jump inside one buffer moves the
        // cursor and nothing else, and a frame is a diff — `1:21` becoming
        // `2:1` on the statusline redraws two cells that no fuzzy match can
        // tell from the row it replaced. `x` is how this harness asks where the
        // cursor is, and the answer is in the file it writes.
        // **`gd` is asynchronous and `x` is not**, so the two race and the loser
        // is silent: `x` deletes wherever the cursor still is. CI caught it —
        // the file came back `"the first line edite\n…"`, the `d` taken from
        // line 1, instead of `"…edited\nhe definition…"`, the `t` taken from
        // line 2. The same test passing and failing for the same reason on two
        // machines.
        //
        // Three ways to wait were tried before this one, and each failed for a
        // reason worth keeping:
        //
        // * [`Editor::settle`] cannot work here at all — [`Editor::press_quietly`]
        //   already calls it, and *waiting for a server is quiet*. It returns
        //   during the wait, which is the race unchanged.
        // * A text match on the statusline reading `2:1` hangs for the full 30 s.
        //   The position is never redrawn: the statusline is cached, and its key
        //   does not include the cursor. Only `1:1` appears in the whole session.
        // * [`Editor::since`] cannot see it either way — it runs the bytes
        //   through `printable`, so the cursor-position escape that *does* carry
        //   the answer is stripped before any matcher gets to it.
        //
        // [`Screen`] replays those escapes onto a grid and keeps `row`, so the
        // terminal's own cursor is the signal — the one thing here that moves
        // when the answer lands and not when the request is sent.
        let before = editor.screen().row;
        editor.press_quietly(b"gd");
        let deadline = Instant::now() + Duration::from_secs(30);
        while editor.screen().row == before {
            assert!(
                Instant::now() < deadline,
                "the jump never moved the cursor off row {before}. Last frame: {}",
                editor.tail()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        editor.press_quietly(b"x");
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "the first line edited\nhe definition is on this line\n",
            "the cursor moved to the line the server named and the edit survived"
        );
        editor.quit();
    }

    /// **`T037`'s criterion, in one session:** *"a 13th language can be added
    /// from the REPL with no Rust change"* — with no restart either.
    ///
    /// The loop read the language table **once, at boot**, and the comment
    /// above that line claimed a language declared at `:repl` was a fact about
    /// the next file opened. It was not: `:e` is the next file opened, and at
    /// `CP-4` a language declared at the REPL commented nothing until the
    /// binary was restarted on the same layer, at which point it worked. Every
    /// test that ticked the task called `AppHost::languages` freshly and was
    /// structurally unable to see it.
    ///
    /// So: declare at the REPL, `:e` a file with that extension, and press the
    /// key whose whole meaning comes from the declaration.
    #[test]
    fn a_language_declared_at_the_repl_is_live_in_the_same_session() {
        let scratch = Scratch::new("live-language");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.zz");
        fs::write(&file, "local x = 1\n").expect("a fixture");
        let opened = scratch.path.join("start.txt");
        fs::write(&opened, "nothing to do with it\n").expect("a fixture");

        let editor = Editor::open(&opened, &scratch.state(), &runtime);
        editor.press_until(b":repl\r", "steel");
        editor.press_until(
            b"(define-language! \"zz\" (hash \"extensions\" (list \"zz\") \
              \"grammar\" void \"lsp_command\" (list) \"comment_prefix\" \"--\"))\r",
            "#ok",
        );
        editor.press_until(b"(close-repl!)\r", "NORMAL");

        // **Wait for the file to be on screen, not for the editor to go quiet.**
        // `press_quietly` settles, and settling is a 250 ms window with no new
        // frame — which an `:e` can satisfy *while still opening*, because the
        // ex line draws, then there is a gap, then the buffer swaps. CI lost
        // that race: `gcgc` commented the buffer that was still open
        // (`start.txt`), `:w` wrote `sample.zz` untouched, and the assertion
        // read `"local x = 1\n"` — the fixture, unchanged, which looks like
        // `gc` not working rather than like `:e` not having finished.
        //
        // The fixture's own text is the signal, the same way
        // `gd_opens_the_file_the_server_named_at_the_line_it_named` waits on its
        // target's text: it is on screen only once the swap has happened, and
        // `start.txt` says something else entirely so there is nothing to
        // confuse it with.
        editor.press_until(format!(":e {}\r", file.display()).as_bytes(), "local x = 1");
        // **Wait for the comment on the screen, not for the write.** Waiting on
        // the open alone was not enough: CI failed twice more with the fixture
        // written back unchanged, which says `gcgc` ran before something it
        // needed — and the buffer's text being drawn does not prove the
        // language was attached to it, because a `zz` declares no server and so
        // puts nothing on the statusline to wait for.
        //
        // Asserting the *drawn* row is the stronger claim anyway: a comment the
        // user cannot see is not a comment, and `:w` after it can no longer
        // race what it is writing. It is also self-diagnosing — if the prefix
        // never arrives, `press_until` fails with the frames, which is the
        // thing four red CI runs could not tell me.
        editor.press_until(b"gcgc", "-- local x = 1");
        editor.press_quietly(b":w\r");
        let written = fs::read_to_string(&file).expect("the buffer was written");
        assert_eq!(
            written, "-- local x = 1\n",
            "the prefix came from a declaration typed into this session's own REPL"
        );
        editor.quit();
    }

    /// **`7c`'s `rust-analyzer ✓`, and the half of it that matters more:** a
    /// server that could not start says so.
    ///
    /// `ServerState` was complete, tested and read by exactly one call site —
    /// the insert-mode trigger's `is_ready()` — so `Crashed`, `Failure` and
    /// `ServerIdentity` reached nobody. At `CP-4`, with a server failing to
    /// initialize, the editor drew the buffer and the statusline and said
    /// nothing, forever: no float, no notice, no refusal, through two `<C-x>`
    /// presses over forty seconds.
    ///
    /// The declaration here names a program that does not exist, so the
    /// sentence on the row is the OS's own — which is what `Failure::Spawn`
    /// says it carries the message for.
    #[test]
    fn a_server_that_cannot_start_says_so_on_the_statusline() {
        let scratch = Scratch::new("no-server");
        let runtime = copy_layer(&scratch.path);
        let form = "(define-language! \"toy\"\n  (hash \"extensions\" '(\"toy\")\n        \
                    \"grammar\" void\n        \
                    \"lsp_command\" (list \"phosphor-no-such-language-server\")\n        \
                    \"comment_prefix\" \";\"))\n";
        fs::write(scratch.persisted().join("persisted.scm"), form)
            .expect("the config home takes it");
        let file = scratch.path.join("sample.toy");
        fs::write(&file, "base = 3\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // **No key is pressed for this.** The spawn fails on the client's
        // runtime thread after the first frame is already on screen, and this
        // loop draws when a producer speaks — so what proves the wake
        // (`events::AppEvent::Woke`) is that the chip changes with the editor
        // idle. Without it the row would say `starting …` until a keystroke.
        // Read against the whole session rather than the last frame: the name
        // was drawn by the `starting …` chip and a frame is a diff, so the
        // failure repaints the glyph and skips the name it is not changing.
        let whole = shown(&editor, "\u{2717}");
        assert!(
            whole.contains("phosphor-no-such-language-server"),
            "the chip never named the program that could not be started; \
             session was: {whole}"
        );
        assert!(
            !whole.contains("no language server for this buffer"),
            "…and it is not the second-tier sentence, which would be a lie about \
             a language that declares one; session was: {whole}"
        );
        editor.quit();
    }

    /// The same chip, on a server that **is** serving: `7c` draws
    /// `rust-analyzer ✓` and the name is the server's own, out of its
    /// `initialize` reply, rather than the command that was run.
    ///
    /// The declaration runs `python3`; the fixture calls itself `toy`. A chip
    /// built from the command would read `python3 ✓`.
    #[test]
    fn a_ready_server_draws_the_name_it_gave_itself() {
        let (scratch, runtime, file) = toy("chip", "completion", "base = 3\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // No key is pressed: the chip arrives on the wake the state change
        // posts, which is what makes it the answer to *"is the server up"*
        // every other `S4` test waits on.
        let frame = shown(&editor, "toy-lsp \u{2713}");
        assert!(
            !frame.contains("python3 \u{2713}"),
            "the chip is the command rather than the name the server gave itself; \
             frame was: {frame}"
        );
        editor.quit();
    }

    /// `T036` — `:restart-server`, and the sentence a bare one answers with.
    ///
    /// The refusal is the half worth pressing: an ex command that silently did
    /// nothing when it could not tell which server you meant is the failure
    /// `T098` is about, one surface over.
    #[test]
    fn restarting_a_server_names_it_and_a_bare_restart_says_why() {
        let (scratch, runtime, file) = toy("restart", "completion", "base = 3\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press_until(b":restart-server toy\r", "restarting toy");
        editor.press_until(b":restart-server\r", "which language");
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `CP-4` — the completion floor
    // -----------------------------------------------------------------------

    /// Sets `completion-min-chars` in the copied layer, the way a person's own
    /// editor layer would.
    ///
    /// Appended to the config home's `persisted.scm`, which the binary loads
    /// **after the whole boot order** (`T101`) — so this is an override of the
    /// shipped default rather than a replacement of it, which is the
    /// arrangement a real user has and the only one that proves the option is
    /// read at all.
    fn set_completion_floor(scratch: &Scratch, least: i64) {
        let persisted = scratch.persisted().join("persisted.scm");
        let mut existing = fs::read_to_string(&persisted).unwrap_or_default();
        existing.push_str(&format!("(set-option! \"completion-min-chars\" {least})\n"));
        fs::write(&persisted, existing).expect("the config home takes an option");
    }

    /// **`CP-4`'s first finding: the list fired at zero characters.** *"there
    /// should be a configurable min num char before firing the menu, right now
    /// its 0"*.
    ///
    /// The shipped floor is two, and the two keystrokes below it are the ones
    /// worth pressing. A **space** is an edit, in insert mode, against a ready
    /// server — every gate the trigger had — and it leaves nothing to filter
    /// on, so the answer was the server's whole table. The **first letter** of
    /// an identifier is the widest list that letter has.
    ///
    /// The negative assertion is the test. `press_quietly` settles — it returns
    /// only after the editor has drawn nothing for 250ms — and the toy server
    /// is a local pipe answering in constants, so a request that was made would
    /// have been answered and drawn well inside that window. Verified by
    /// planting a floor of `0`, which puts all three labels on the frame after
    /// the space.
    #[test]
    fn typing_does_not_raise_the_list_until_the_word_is_two_characters() {
        let (scratch, runtime, file) = toy("floor-shipped", "completion", "let base =\n");
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        // `A` — an insert session at the end of the line, and not yet an edit.
        editor.press(b"A");

        let mark = editor.mark();
        editor.press_quietly(b" ");
        editor.press_quietly(b"d");
        let quiet = editor.since(mark);
        assert!(
            !shows(&quiet, "default_delay"),
            "the list came up on a space and a single letter; frames were: {quiet}"
        );
        assert!(
            !shows(&quiet, "defaults_for"),
            "…nor any other row of it; frames were: {quiet}"
        );

        // The second character of the word is the floor, and the very next
        // keystroke raises it — the floor delays the list, it does not need a
        // key to get it back.
        let raised = editor.press_until(b"e", "default_delay");
        assert!(
            shows(&raised, "defaults_for"),
            "the whole list is up at two characters; frame was: {raised}"
        );
        editor.quit();
    }

    /// **The floor is the editor layer's, which is what *configurable* means
    /// here.** No Rust in the path: one `(set-option! …)` in the layer, and the
    /// same keystrokes behave differently.
    ///
    /// Four rather than three, so the value cannot be confused with the shipped
    /// default by an off-by-one — three characters are silent and the fourth
    /// raises the list.
    #[test]
    fn the_completion_floor_is_the_editor_layers_own() {
        let (scratch, runtime, file) = toy("floor-four", "completion", "let base =\n");
        set_completion_floor(&scratch, 4);
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");

        let mark = editor.mark();
        editor.press_quietly(b" def");
        let quiet = editor.since(mark);
        assert!(
            !shows(&quiet, "default_delay"),
            "three characters raised a list the layer set the floor at four; \
             frames were: {quiet}"
        );

        let raised = editor.press_until(b"a", "default_delay");
        assert!(
            shows(&raised, "defaults_for"),
            "…and the fourth did not raise it; frame was: {raised}"
        );
        editor.quit();
    }

    /// **`<C-x>` ignores the floor, because asking is asking.**
    ///
    /// The two halves are pressed against the *same* prefix on purpose. One
    /// character with a floor of four is silent when it was typed and answers
    /// when it was asked for, so what this separates is the *path* rather than
    /// the length — which is the whole of the ruling: the floor lives in the
    /// loop's typed-trigger and not in `Editing::act`, so every door that sends
    /// `request-completion` — the key, the CLI, MCP — is unaffected by a
    /// preference about typing.
    #[test]
    fn an_explicit_completion_key_ignores_the_floor() {
        let (scratch, runtime, file) = toy("floor-explicit", "completion", "let base =\n");
        set_completion_floor(&scratch, 4);
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");

        let mark = editor.mark();
        editor.press_quietly(b" d");
        assert!(
            !shows(&editor.since(mark), "default_delay"),
            "typing one character raised a list floored at four; frames were: {}",
            editor.since(mark)
        );

        let asked = editor.press_until(b"\x18", "default_delay");
        assert!(
            shows(&asked, "defaults_for"),
            "`<C-x>` on the same one character answered nothing; frame was: {asked}"
        );
        editor.quit();
    }

    /// **A trigger character asks where the floor would not** (`CP-4` review).
    ///
    /// The floor is measured on `Editing::prefix_len`, which counts identifier
    /// characters — so `foo.` measures **zero** and the shipped floor of two
    /// made `.`-completion, the most common completion moment in a dotted
    /// language, unreachable by typing. The server is the one that knows: this
    /// asks its `completionProvider.triggerCharacters`, which the toy fixture
    /// advertises as `.` and `::`.
    ///
    /// Two halves, and the first is what makes the second mean something: a
    /// floor of four is set, so a single `d` is silent — the floor is still the
    /// floor — and then a `.` raises the list on a prefix of zero.
    #[test]
    fn a_trigger_character_raises_the_list_under_the_floor() {
        let (scratch, runtime, file) = toy("trigger-dot", "completion", "let base =\n");
        set_completion_floor(&scratch, 4);
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        ready(&editor);
        editor.press(b"A");

        let mark = editor.mark();
        editor.press_quietly(b" d");
        assert!(
            !shows(&editor.since(mark), "default_delay"),
            "one character raised a list floored at four; frames were: {}",
            editor.since(mark)
        );

        let raised = editor.press_until(b".", "default_delay");
        assert!(
            shows(&raised, "defaults_for"),
            "a `.` the server named as a trigger raised nothing; frame was: {raised}"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `CP-4` — `gr`, and a path with nothing behind it
    // -----------------------------------------------------------------------

    /// **`CP-4`'s second finding: `gr` was unbound.** *"why is gr unbound it
    /// should show uses of that thing right"*.
    ///
    /// It is `T098`'s rule reaching one more key, and the half of it that
    /// survives `T047` is the second: **a key that is known must not spend
    /// `8e`'s one teaching row**, whatever it goes on to do.
    ///
    /// The first half asserted the refusal — *"not built yet — T047 builds
    /// it"* — and `T047` built it, so that assertion moved rather than
    /// loosened: `gr_fills_the_picker_from_a_real_server` presses the same
    /// key against a real server and reads the list it produces. Kept as a
    /// separate test because what it holds is not about references at all:
    /// it is that a *bound* key is not an unknown one, and a build where
    /// `gr` stopped working would still have to not lie about that.
    #[test]
    fn gr_is_bound_and_does_not_spend_the_session_hint() {
        let scratch = Scratch::new("references");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        // No language server for a `.txt`, so `gr` has nothing to ask —
        // which is the case that could most easily read as *unbound*.
        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b"gr");
        let frame = editor.since(before);
        assert!(
            !frame.contains("unknown key"),
            "gr is bound, so it is not an unknown key and does not spend the session's \
             one hint; frame was: {frame}"
        );
        editor.quit();
    }

    /// **`CP-4`'s third finding: `:e` on a path that does not exist refused.**
    /// Teej typed `:e /tmp/x.lua` and got *"x.lua didnt exist"* — no buffer, no
    /// way to create the file.
    ///
    /// The whole chain is asserted rather than the notice alone, because the
    /// notice is the cheap half: a buffer with nothing on disk behind it still
    /// has to know its language, still has to reach a server, and above all has
    /// to be *writable*, or the empty buffer is a worse answer than the refusal
    /// it replaced.
    ///
    /// So: the file is never created, the extension is the only thing anyone
    /// knows about it, and the proof that the declaration was adopted is that
    /// typing two characters into an empty new buffer raises a list the toy
    /// server answered — which needs the language table, the attach, and the
    /// `didOpen` that carries the buffer's own (empty) text rather than disk's.
    /// Then `:w` creates the file.
    #[test]
    fn a_path_with_nothing_behind_it_opens_as_a_writable_new_buffer() {
        let (scratch, runtime, opened) = toy("new-file", "completion", "base = 3\n");
        let fresh = scratch.path.join("second.toy");
        assert!(!fresh.exists(), "the fixture is a path with nothing at it");

        let editor = Editor::open(&opened, &scratch.state(), &runtime);
        ready(&editor);
        // [`Editor::press_until`] rather than settle-then-assert: an `:e` can
        // satisfy a 250 ms quiet window *while still opening* — the ex line
        // draws, there is a gap, then the buffer swaps — so reading the frames
        // straight after `press_quietly` races the thing being asserted. Its
        // timeout IS the assertion here, which is why there is no `assert!`
        // under it repeating the same claim.
        editor.press_until(format!(":e {}\r", fresh.display()).as_bytes(), "new file");

        editor.press(b"i");
        let raised = editor.press_until(b"de", "default_delay");
        assert!(
            shows(&raised, "defaults_for"),
            "the new buffer never reached the server the extension declares; \
             frame was: {raised}"
        );
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b":w\r");
        assert_eq!(
            fs::read_to_string(&fresh).expect("`:w` created the file"),
            "de",
            "the buffer wrote itself to the path that had nothing behind it"
        );
        editor.quit();
    }

    /// The same buffer, keyed on the same journal **across a restart** — which
    /// is the half a new file makes easy to get wrong.
    ///
    /// `Timeline` keys on `std::fs::canonicalize`, which needs the file to
    /// exist; a new buffer has nothing to resolve. Keyed on the name alone the
    /// second session would hash a different path and open an empty history,
    /// and nothing would say so — the file would be fine and the undo would
    /// simply not be there. So: type, save, quit, reopen, `u`, and the file
    /// goes back.
    ///
    /// **One character, and that is not arbitrary.** Undoing a group of *two
    /// or more* edits whose highest offset is past the length the buffer ends
    /// up at panics the binary in the vendored fork — `Code::notify_changes`
    /// (`vendor/ratatui-code-editor/src/code.rs`) computes `point(edit.start)`
    /// for every edit in the batch against the rope as it is *after* all of
    /// them, so `char_to_line` is handed an index the rope no longer has. It is
    /// not this change's doing and it is not new-file-specific — `abc` with no
    /// trailing newline, `A`, `xy`, `<esc>`, `u` reproduces it on a file that
    /// has been on disk all along — but an empty buffer is the shortest way
    /// there, so this test stays inside one edit rather than encoding a crash.
    /// Reported at `CP-4`; the fix is `surface`'s, in the fork.
    #[test]
    fn a_new_files_history_survives_the_first_save_and_a_restart() {
        let scratch = Scratch::new("new-file-undo");
        let runtime = copy_layer(&scratch.path);
        let state = scratch.state();
        let file = scratch.path.join("fresh.txt");

        let first = Editor::open(&file, &state, &runtime);
        first.press(b"i");
        first.press(b"x");
        first.press(b"\x1b");
        first.press_quietly(b":w\r");
        assert_eq!(
            fs::read_to_string(&file).expect("`:w` created the file"),
            "x"
        );
        first.quit();

        // A second process, on the same journal. `u` walks into a history the
        // first session wrote before the file existed.
        let second = Editor::open(&file, &state, &runtime);
        second.press(b"u");
        second.press_quietly(b":w\r");
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "",
            "the history the first session wrote was keyed on a path the second \
             session could not find"
        );
        second.quit();
    }

    /// **A path whose *directory* does not exist is still a refusal**, and that
    /// is the line the new-buffer rule stops at: an empty buffer that `:w`
    /// cannot write costs whatever was typed into it, which is strictly worse
    /// than the keystroke a refusal costs.
    ///
    /// The second half is what proves nothing opened. The notice alone could
    /// not: a swapped-in empty buffer would leave the original file untouched
    /// too. So the buffer is edited and written, and the bytes that land are
    /// the *original* file's — which only happens if `:e` never swapped it.
    #[test]
    fn a_path_whose_directory_is_missing_is_still_a_refusal() {
        let scratch = Scratch::new("no-directory");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");
        let nowhere = scratch.path.join("nope").join("x.txt");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // Same reason as the sibling above — wait for the refusal rather than
        // for quiet. The negative assertion below is still worth making, and
        // needs the frames this returns.
        let drawn = editor.press_until(
            format!(":e {}\r", nowhere.display()).as_bytes(),
            "no directory",
        );
        assert!(
            !shows(&drawn, "new file"),
            "a path with no directory was opened as a new buffer; frames were: {drawn}"
        );

        // `x` deletes a character from whatever buffer is open, and `:w` writes
        // it back to whatever file that buffer came from.
        editor.press_quietly(b"x");
        editor.press_quietly(b":w\r");
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "ne\ntwo\n",
            "the refused `:e` swapped the buffer anyway"
        );
        assert!(
            !nowhere.exists(),
            "nothing was created under a directory that does not exist"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // Visual mode — `CP-4`
    // -----------------------------------------------------------------------

    /// The transcript replayed onto a grid, **because a frame is a diff**.
    ///
    /// [`printable`] answers *"is this word on the frame"* and cannot answer
    /// *"is this cell still highlighted"* — the two questions differ exactly
    /// where a selection is concerned. Leaving visual mode changes no text at
    /// all, so the frame that clears the highlight and the frame that fails to
    /// clear it contain the same characters; the difference is entirely in the
    /// SGR runs around them, and it is *cumulative*, since a cell nobody
    /// rewrote keeps whatever the last frame that touched it said. Asserting on
    /// one frame's bytes therefore cannot fail — a selection left standing is a
    /// frame that simply never mentions those cells.
    ///
    /// So this replays the whole transcript and keeps, per cell, the character
    /// and the background it was written with. It is deliberately the smallest
    /// terminal that can answer the question: cursor positioning (`CSI …H`),
    /// the SGR background, and printable runs, which is all `ratatui`'s
    /// backend emits — measured on this binary's own output, where the only
    /// other sequences are the private mode sets terminal setup makes once.
    /// Everything else is skipped the way [`printable`] skips it.
    #[derive(Debug)]
    struct Screen {
        width: u16,
        cells: Vec<Cell>,
        row: u16,
        column: u16,
        background: String,
    }

    /// One cell: what was written, and the background it was written on.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Cell {
        character: char,
        background: String,
    }

    impl Default for Cell {
        fn default() -> Self {
            Self {
                character: ' ',
                background: String::new(),
            }
        }
    }

    impl Screen {
        /// A grid the size of [`SCREEN`], with `bytes` played onto it.
        fn replayed(bytes: &[u8]) -> Self {
            let width = SCREEN.ws_col;
            let height = SCREEN.ws_row;
            let mut screen = Self {
                width,
                cells: vec![Cell::default(); usize::from(width) * usize::from(height)],
                row: 0,
                column: 0,
                background: String::new(),
            };
            screen.feed(bytes);
            screen
        }

        fn feed(&mut self, bytes: &[u8]) {
            let text = String::from_utf8_lossy(bytes);
            let mut chars = text.chars().peekable();
            while let Some(character) = chars.next() {
                if character != '\u{1b}' {
                    match character {
                        '\r' => self.column = 0,
                        '\n' => {
                            self.row = self.row.saturating_add(1);
                            self.column = 0;
                        }
                        printable if !printable.is_control() => self.put(printable),
                        _ => {}
                    }
                    continue;
                }
                match chars.next() {
                    Some('[') => {
                        let mut parameters = String::new();
                        let mut final_byte = '\0';
                        for byte in chars.by_ref() {
                            if ('\u{40}'..='\u{7e}').contains(&byte) {
                                final_byte = byte;
                                break;
                            }
                            parameters.push(byte);
                        }
                        self.csi(&parameters, final_byte);
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
            }
        }

        fn csi(&mut self, parameters: &str, final_byte: char) {
            // A private sequence (`?2026h`) is never one of the two below.
            if parameters.starts_with('?') {
                return;
            }
            match final_byte {
                'H' | 'f' => {
                    let mut halves = parameters.split(';');
                    let row = Self::number(halves.next()).max(1);
                    let column = Self::number(halves.next()).max(1);
                    self.row = row - 1;
                    self.column = column - 1;
                }
                'm' => self.sgr(parameters),
                _ => {}
            }
        }

        fn number(field: Option<&str>) -> u16 {
            field
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse().ok())
                .unwrap_or(1)
        }

        /// The background half of an SGR run. Foreground, bold and the rest are
        /// no part of the question this screen exists to answer.
        fn sgr(&mut self, parameters: &str) {
            let fields: Vec<&str> = parameters.split(';').collect();
            let mut index = 0;
            while index < fields.len() {
                match fields[index] {
                    "" | "0" | "49" => self.background.clear(),
                    "48" => {
                        let width = match fields.get(index + 1) {
                            Some(&"2") => 5,
                            Some(&"5") => 3,
                            _ => 1,
                        };
                        self.background =
                            fields[index..(index + width).min(fields.len())].join(";");
                        index += width - 1;
                    }
                    _ => {}
                }
                index += 1;
            }
        }

        fn put(&mut self, character: char) {
            if self.column >= self.width {
                self.row = self.row.saturating_add(1);
                self.column = 0;
            }
            let at = usize::from(self.row) * usize::from(self.width) + usize::from(self.column);
            if let Some(cell) = self.cells.get_mut(at) {
                cell.character = character;
                cell.background.clone_from(&self.background);
            }
            self.column = self.column.saturating_add(1);
        }

        /// Row `y` as text, trailing blanks trimmed.
        fn line(&self, y: u16) -> String {
            let start = usize::from(y) * usize::from(self.width);
            let row = &self.cells[start..start + usize::from(self.width)];
            row.iter()
                .map(|cell| cell.character)
                .collect::<String>()
                .trim_end()
                .to_owned()
        }

        /// The columns of row `y` drawn on a background other than the body's.
        ///
        /// The body background is read off the row's **last** column, which is
        /// the honest reference rather than a colour literal: no test here
        /// writes a line anywhere near 120 characters, so the far right of a
        /// row is never selected and never anything but background.
        /// The background of one cell, as the escape sequence that set it.
        ///
        /// Opaque on purpose: no test here should know what colour `trouble`
        /// *is* — Design Language §1 owns that and `scripts/lint-no-colours.sh`
        /// keeps the literals in `theme.rs`. What a test can honestly say is
        /// that two cells match, or do not, which is all a priority ladder
        /// claims.
        fn background(&self, y: u16, x: u16) -> String {
            let at = usize::from(y) * usize::from(self.width) + usize::from(x);
            self.cells
                .get(at)
                .map(|cell| cell.background.clone())
                .unwrap_or_default()
        }

        fn tinted(&self, y: u16) -> Vec<u16> {
            let start = usize::from(y) * usize::from(self.width);
            let row = &self.cells[start..start + usize::from(self.width)];
            let body = &row[row.len() - 1].background;
            (0..self.width)
                .filter(|column| &row[usize::from(*column)].background != body)
                .collect()
        }
    }

    /// Every row of a screen, joined — for an assertion about the *body* rather
    /// than about which row something landed on.
    ///
    /// A float's rows move with its height, and a test that pinned one would be
    /// asserting the float's geometry while claiming to assert its contents.
    fn whole(screen: &Screen) -> String {
        (0..SCREEN.ws_row)
            .map(|row| screen.line(row))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A file whose lines are short, distinct, and the same length — so a
    /// column count is readable straight off [`Screen::tinted`].
    fn ten_by_ten(name: &str) -> (Scratch, PathBuf, PathBuf) {
        let scratch = Scratch::new(name);
        let file = scratch.path.join("ten.txt");
        fs::write(&file, "abcdefghij\nklmnopqrst\nuvwxyz0123\n").expect("a fixture file");
        let runtime = copy_layer(&scratch.path);
        (scratch, file, runtime)
    }

    /// The mouse, as an emulator with SGR reporting (`?1006h`) spells it.
    fn mouse(button: u8, column: u16, row: u16, down: bool) -> Vec<u8> {
        format!(
            "\x1b[<{button};{column};{row}{}",
            if down { 'M' } else { 'm' }
        )
        .into_bytes()
    }

    /// **`CP-4`'s reported finding, on the shipping binary.** *"escape gets out
    /// of visual mode but doesnt clear the selection."*
    ///
    /// Nothing in this repository pressed `v` before this test, so the whole of
    /// visual mode was unguarded — which is why the finding could only be found
    /// by hand. The typed path turns out to be correct and this is what says so
    /// from now on.
    #[test]
    fn v_selects_under_the_cursor_and_esc_takes_the_highlight_with_it() {
        let (scratch, file, runtime) = ten_by_ten("visual-esc");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press(b"v");
        let one = editor.screen().tinted(0);
        assert_eq!(one.len(), 1, "`v` selects the character under the cursor");

        editor.press(b"ll");
        assert_eq!(
            editor.screen().tinted(0),
            [one[0], one[0] + 1, one[0] + 2],
            "two `l`s in visual mode reach three characters"
        );

        editor.press(b"\x1b");
        assert!(
            editor.screen().tinted(0).is_empty(),
            "esc left the selection drawn: {:?}",
            editor.screen().line(0)
        );
        assert!(
            shows(&editor.tail(), "NORMAL"),
            "esc left the mode chip alone: {}",
            editor.tail()
        );
        editor.quit();
    }

    /// **The highlight and the operand were one character apart** (`CP-4`).
    ///
    /// `v l l` drew two cells and `d` deleted three, because `SelectRange` went
    /// through `span_between` — inclusive of the character under the cursor —
    /// and `ExtendSelection` went through the fork's half-open
    /// `extend_selection`. Whichever of the two is right, they cannot disagree:
    /// a highlight that is not the operand is a lie the editor tells at every
    /// keystroke.
    #[test]
    fn the_highlight_is_exactly_what_the_operator_takes() {
        let (scratch, file, runtime) = ten_by_ten("visual-operand");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press(b"vll");
        let highlighted = editor.screen().tinted(0);
        assert_eq!(highlighted.len(), 3, "three characters are drawn selected");

        editor.press_quietly(b"d");
        assert!(
            editor.screen().line(0).ends_with("defghij"),
            "the highlight covered three characters and the delete took a \
             different number: {:?}",
            editor.screen().line(0)
        );
        editor.quit();
    }

    /// `V` is linewise on the screen as well as under the operator (`CP-4`).
    ///
    /// It drew a single cell while `V d` deleted the whole line — the same
    /// disagreement as the test above, one selection kind over.
    #[test]
    fn shift_v_highlights_whole_lines() {
        let (scratch, file, runtime) = ten_by_ten("visual-line");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press(b"V");
        let first = editor.screen().tinted(0);
        assert_eq!(first.len(), 10, "`V` selects the line, not a character");
        assert!(
            editor.screen().tinted(1).is_empty(),
            "`V` reached a line the cursor is not on"
        );
        assert!(shows(&editor.tail(), "V-LINE"), "{}", editor.tail());

        editor.press(b"j");
        assert_eq!(editor.screen().tinted(0), first);
        assert_eq!(
            editor.screen().tinted(1),
            first,
            "`j` in `V` takes the whole of the second line too"
        );

        editor.press_quietly(b"d");
        assert!(
            editor.screen().line(0).ends_with("uvwxyz0123"),
            "`V j d` took something other than the two whole lines it drew: {:?}",
            editor.screen().line(0)
        );
        editor.quit();
    }

    /// `<C-v>` reaches blockwise mode and `<esc>` leaves it.
    ///
    /// **Deliberately not asserted: the shape of the block.** The fork's
    /// `Selection` is one offset range, so a column selection is drawn as a run
    /// through the intervening line ends — reported at `CP-4` rather than
    /// blessed here, because a test written to the current drawing would have
    /// to be deleted by whoever fixes it. What is guarded is the half that is
    /// this file's business: the key reaches the machine, the chip says so, and
    /// `<esc>` leaves nothing behind.
    #[test]
    fn ctrl_v_reaches_blockwise_mode_and_esc_leaves_it() {
        let (scratch, file, runtime) = ten_by_ten("visual-block");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        editor.press(b"\x16");
        assert!(shows(&editor.tail(), "V-BLOCK"), "{}", editor.tail());
        editor.press(b"jl");
        assert!(
            !editor.screen().tinted(1).is_empty(),
            "the block never reached the second row"
        );

        editor.press(b"\x1b");
        assert!(shows(&editor.tail(), "NORMAL"), "{}", editor.tail());
        for row in 0..3 {
            assert!(
                editor.screen().tinted(row).is_empty(),
                "esc left row {row} highlighted"
            );
        }
        editor.quit();
    }

    /// **A pointer selection is a visual selection** (`CP-4`).
    ///
    /// This is the reported finding's other half and the one that reproduced: a
    /// drag built a highlight straight in the editor, so the machine never
    /// entered visual mode, `<esc>` had nothing it believed it had selected,
    /// and the highlight stayed for the rest of the session — no key cleared
    /// it, and only another click did.
    #[test]
    fn a_drag_selects_in_visual_mode_and_esc_clears_what_it_drew() {
        let (scratch, file, runtime) = ten_by_ten("visual-drag");
        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // Row 1 of the screen, over the text column: the gutter is six cells,
        // so the file's first character is column 7 in the emulator's 1-based
        // spelling.
        editor.press_quietly(&mouse(0, 7, 1, true));
        editor.press_quietly(&mouse(32, 11, 1, true));
        editor.press_quietly(&mouse(0, 11, 1, false));

        let dragged = editor.screen().tinted(0);
        assert_eq!(
            dragged.len(),
            5,
            "a press at the first character and a drag to the fifth selects five"
        );
        assert!(
            shows(&editor.tail(), "VISUAL"),
            "a drag left the mode chip in normal: {}",
            editor.tail()
        );

        editor.press(b"\x1b");
        assert!(
            editor.screen().tinted(0).is_empty(),
            "esc left the drag's highlight drawn"
        );
        editor.quit();
    }

    // -----------------------------------------------------------------------
    // `T107` — a buffer with no file
    // -----------------------------------------------------------------------

    /// **`phosphor` with no argument opens an editor**, and the first row of
    /// chrome says what turns the buffer into a file.
    ///
    /// The reachability half is the whole test and it is why this is a pty test
    /// rather than a parse test: until `T107` this argv never reached the loop
    /// at all — clap refused it and the process exited `2` — so *"a frame was
    /// drawn"* is the claim, and typing into that
    /// frame and finding the bytes on disk is the proof it is an editor and not
    /// a picture of one.
    ///
    /// **What a generator cannot produce here** is the first line. `no file —
    /// :write <path> creates one` is composed by `main::no_file` and drawn on
    /// the notice row; nothing in the layer, the keymap or the ViewModel names
    /// it, and no other command line puts it on a frame.
    #[test]
    fn a_bare_phosphor_opens_a_buffer_and_says_what_would_give_it_a_file() {
        let scratch = Scratch::new("t107-bare");
        let runtime = copy_layer(&scratch.path);
        let editor = Editor::bare(&scratch.state(), &runtime);

        assert!(
            shows(&editor.tail(), "no file — :write <path> creates one"),
            "the first frame said nothing about having no file: {}",
            editor.tail()
        );

        let file = scratch.path.join("typed.txt");
        editor.press(b"i");
        editor.press(b"alpha");
        editor.press(b"\x1b");
        editor.press(format!(":w {}\r", file.display()).as_bytes());
        editor.quit();

        assert_eq!(
            fs::read_to_string(&file).expect("`:write <path>` created the file"),
            "alpha",
            "what was typed into a buffer with no name reached the path it was given"
        );
    }

    /// **A surface that explains itself is not told it has no file** — the
    /// negative half of `T107`'s notice, and the half nothing asserted.
    ///
    /// The guard is `matches!(surface, Surface::Buffer)` beside
    /// `editing.file.is_none()`, argued at length in two doc comments
    /// (*"`--repl`, the boot float and the `--float` fixture — all of which
    /// explain themselves — stay silent"*) and, until this, checked by nothing:
    /// `a_bare_phosphor_opens_a_buffer_and_says_what_would_give_it_a_file`
    /// presses the positive case only, and dropping the guard left the whole
    /// suite green. `--repl` is the same buffer with the same `file: None`, so
    /// the *only* thing standing between it and the notice is the surface.
    ///
    /// The float's own footer is waited for first, so this is an assertion
    /// about a frame that was drawn — *"the notice is absent"* is worth nothing
    /// on a frame that never arrived. [`Editor::floated`] says why the fixture
    /// float is the surface that can fail here and `--repl` is not.
    ///
    /// **This bites:** drop `&& matches!(surface, Surface::Buffer)` from the
    /// notice in `run` and the last row of this frame reads `no file — :write
    /// <path> creates one` under a float that is already explaining itself.
    #[test]
    fn a_float_over_a_nameless_buffer_says_nothing_about_the_missing_file() {
        let scratch = Scratch::new("t107-float-silent");
        let runtime = copy_layer(&scratch.path);
        let editor = Editor::floated(&scratch.state(), &runtime);

        let first = editor.tail();
        assert!(
            shows(&first, "esc close"),
            "the fixture float never drew, so there is no frame to find the notice absent \
             from: {first}"
        );
        assert!(
            !shows(&first, "no file"),
            "the float explains itself and was told about its missing file anyway: {first}"
        );
        editor.quit();
    }

    /// **An editor you cannot leave is the worst version of this feature.**
    ///
    /// `ZQ` at a buffer with no file and unsaved work in it, which is the state
    /// a bare `phosphor` is in the moment anybody types. Unsaved is the half
    /// that matters: `quit` refuses on `WouldLoseWork` and `ZQ` is the forcing
    /// spelling, so a scratch buffer that had no forced exit would be a trap
    /// with no way out at all — there is no file to `:write` to.
    ///
    /// [`Editor::quit`] asserts the exit status, so a child still on the
    /// alternate screen fails here rather than hanging the suite.
    #[test]
    fn a_bare_phosphor_with_unsaved_work_is_still_quittable() {
        let scratch = Scratch::new("t107-quit");
        let runtime = copy_layer(&scratch.path);
        let editor = Editor::bare(&scratch.state(), &runtime);

        editor.press(b"i");
        editor.press(b"unsaved");
        editor.quit();
    }

    /// **`:write` with nothing to write to refuses in this editor's voice**,
    /// which is §6's — lowercase, telegraphic, and naming the whole command
    /// rather than a contraction.
    ///
    /// The refusal itself predates `T107` (`Editing::write` has always answered
    /// it) and was unreachable from a bare `phosphor`, because a bare
    /// `phosphor` did not run. What is pinned here is that it is what the user
    /// meets — not clap's *"the following required arguments were not
    /// provided"*, which is what the same mistake used to produce one layer
    /// earlier and in a different voice.
    #[test]
    fn write_with_no_path_refuses_by_naming_the_command_that_would_work() {
        let scratch = Scratch::new("t107-refusal");
        let runtime = copy_layer(&scratch.path);
        let editor = Editor::bare(&scratch.state(), &runtime);

        editor.press(b"i");
        editor.press(b"x");
        editor.press(b"\x1b");
        let drawn = editor.press_until(b":w\r", "no file name — :write <path>");
        assert!(
            !shows(&drawn, "required arguments"),
            "the editor answered in clap's voice: {drawn}"
        );
        editor.quit();
    }

    /// **`:write <path>` gives a scratch buffer a history, not only a file.**
    ///
    /// Two child processes and one journal, the same shape as
    /// [`undo_survives_quitting_and_reopening`] — except that the first session
    /// starts with *no journal at all*, because a buffer with no file has
    /// nothing to key one on. `Timeline::attach` opens one at the moment the
    /// buffer gains a name and seeds it with the tree the scratch session
    /// already built, so the second session undoes into edits made before the
    /// file existed.
    ///
    /// **This is the test the whole `Timeline::attach` path exists for, and it
    /// is the one that bites.** Deleting the `attach` call leaves every other
    /// `T107` test green — the file is still written, the editor still quits,
    /// the refusal is unchanged — and this one fails with `alpha` still on
    /// disk, because the second session restores nothing and `u` has no history
    /// to walk.
    ///
    /// The two-step undo is deliberate: `ZED` is one group and `alpha` is
    /// another, so a single `u` proves the *newest* node survived and the
    /// second proves the seed carried the node underneath it rather than only
    /// the save.
    #[test]
    fn a_scratch_buffer_written_to_a_path_undoes_into_it_after_a_restart() {
        let scratch = Scratch::new("t107-history");
        let runtime = copy_layer(&scratch.path);
        let state = scratch.state();
        let file = scratch.path.join("grown.txt");

        // Session one: two insert groups typed into a buffer with no file, then
        // the write that gives it one.
        let first = Editor::bare(&state, &runtime);
        first.press(b"i");
        first.press(b"alpha");
        first.press(b"\x1b");
        first.press(b"A");
        first.press(b"ZED");
        first.press(b"\x1b");
        first.press(format!(":w {}\r", file.display()).as_bytes());
        first.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file exists"),
            "alphaZED",
            "both groups reached disk"
        );

        // Session two: a fresh process on the file the scratch buffer became,
        // undoing into a history no session of *this* file ever recorded.
        let second = Editor::open(&file, &state, &runtime);
        second.press(b"u");
        second.press(b":w\r");
        second.press(b"u");
        second.press(b":w\r");
        second.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "",
            "the second session walked the scratch buffer's own history"
        );
    }

    /// **A `:write <path>` that overwrites a file replaces that file's undo
    /// history, and says so.**
    ///
    /// This test was written to pin the opposite behaviour and **found the
    /// defect instead**, which is why it is here in this shape. Leaving the
    /// existing journal alone sounds like the conservative answer — it is
    /// somebody's undo history and this is only a save — and what it actually
    /// produces is a tree describing bytes the write has just replaced. Undo in
    /// the next session then applies the inverse of an edit against text that
    /// no longer exists: measured, `owned\n` with one saved edit, written over
    /// by a scratch buffer holding `new`, reopened, `u` — and the buffer became
    /// **`ew`**.
    ///
    /// So the seed replaces it, the third session below undoes into *this*
    /// buffer's history rather than into the old one, and the row says a
    /// history went. Both halves are asserted because either alone passes over
    /// the bug: the notice alone would pass with the stale journal still there,
    /// and the undo alone would pass with the replacement done silently.
    ///
    /// This is also the only test that reaches `Editing::note` at all, and
    /// therefore the only one that proves the loop drains it — delete the drain
    /// and the row stays blank while everything else about the write still
    /// works.
    #[test]
    fn writing_over_a_file_replaces_the_undo_history_that_was_under_it() {
        let scratch = Scratch::new("t107-occupied");
        let runtime = copy_layer(&scratch.path);
        let state = scratch.state();
        let file = scratch.path.join("owned.txt");
        fs::write(&file, "owned\n").expect("a fixture");

        // A session on the file itself, which is what puts a journal under its
        // key. Nothing about this session is scratch.
        let owner = Editor::open(&file, &state, &runtime);
        owner.press(b"i");
        owner.press(b"X");
        owner.press(b"\x1b");
        owner.press(b":w\r");
        owner.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "Xowned\n"
        );

        // …and then a nameless buffer told to write over it.
        let scratchpad = Editor::bare(&state, &runtime);
        scratchpad.press(b"i");
        scratchpad.press(b"new");
        scratchpad.press(b"\x1b");
        scratchpad.press_until(
            format!(":w {}\r", file.display()).as_bytes(),
            "undo history replaced",
        );
        scratchpad.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "new",
            "the write itself is an ordinary write"
        );

        // A third session on the file: `u` walks the *scratch* buffer's history
        // — one insert of `new` into an empty buffer — and not the owner's,
        // whose single edit would have deleted a character it never wrote.
        let again = Editor::open(&file, &state, &runtime);
        again.press(b"u");
        again.press(b":w\r");
        again.quit();
        assert_eq!(
            fs::read_to_string(&file).expect("the file survives"),
            "",
            "undo walked the history the write installed, not the one it invalidated"
        );
    }

    // -----------------------------------------------------------------------
    // `T094` — the editor layer, reloaded
    // -----------------------------------------------------------------------

    /// **A broken file leaves the previous layer standing** (`T094`).
    ///
    /// The requirement that shapes the whole implementation: the new runtime is
    /// built *beside* the old one and only swapped in if its boot produced no
    /// fault. Reloading in place and repairing on failure cannot work — half
    /// the load order has already run by the time the fault appears, and there
    /// is nothing to roll back to.
    ///
    /// **What is asserted is that the editor still edits.** A float saying
    /// something went wrong is easy; an editor that still has your buffer, your
    /// cursor and a working keymap behind that float is the claim. So this
    /// types after the failed reload and checks the text moved.
    #[test]
    fn a_broken_reload_leaves_the_editor_that_was_working() {
        let scratch = Scratch::new("reload-broken");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);

        // An unbalanced paren: the file parses as far as this and then stops,
        // which is the ordinary way a hand-edited layer breaks.
        let keymaps = runtime.join("keymaps.scm");
        let mut layer = fs::read_to_string(&keymaps).expect("the copied layer");
        layer.push_str("\n(ex-set! \"broken\" \"unbalanced\n");
        fs::write(&keymaps, layer).expect("the layer is writable");

        editor.press_quietly(b":reload\r");

        // The editor is still an editor: `x` deletes a character, which needs
        // the keymap the reload just failed to replace. If the broken layer had
        // been swapped in, there would be no `x`.
        editor.press_quietly(b"\x1b");
        editor.press_quietly(b"x");
        let after = editor.screen();
        let grid = (0..SCREEN.ws_row)
            .map(|row| after.line(row))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            grid.contains("ne") && !grid.contains("one"),
            "the previous layer is still running, so `x` still deletes: {grid}"
        );

        editor.leave_by(b":q!\r");
    }

    // -----------------------------------------------------------------------
    // `T092` — the theme, switched without a restart
    // -----------------------------------------------------------------------

    /// **`:theme <slug>` redraws in the new palette** (`T092`).
    ///
    /// The theme was an immutable local baked into each `Editor` at
    /// construction and every widget takes a `&Theme`, so switching it is a
    /// *rebuild path* rather than an arm: the loop hands the new palette to
    /// every open buffer and invalidates the frame cache in the same beat.
    /// `--theme <slug>` already worked, and `:theme` answered a refusal —
    /// Teej's ruling on 2026-08-13 was that the ex command stays bound *"but
    /// only if something is going to close it"*, and this is that something.
    ///
    /// **It asserts the colour, not the text.** A theme switch changes no
    /// characters at all, so every text assertion in this file would pass
    /// against an editor that ignored the command completely. The background
    /// escape sequence the terminal was actually sent is the only honest
    /// witness, and comparing it before and after is what makes this a test:
    /// the *ground* of `phosphor-dark` and of `tokyo-night` are different
    /// colours, so a `:theme` that did nothing leaves the two readings equal.
    #[test]
    fn switching_theme_redraws_the_buffer_in_the_new_ground() {
        let scratch = Scratch::new("theme-switch");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        // The far right of a row is never selected and never anything but
        // background — the same reference `Screen::tinted` takes.
        let before = editor.screen().background(0, SCREEN.ws_col - 1);

        // The switch is accepted before the palette is looked at, so a failure
        // below says *"the redraw did not happen"* rather than *"something
        // about themes did not work"*.
        let said = editor.shown_on_grid(b":theme tokyo-night\r", "theme tokyo-night");
        let notice = (0..SCREEN.ws_row)
            .map(|row| said.line(row))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            notice.contains("theme tokyo-night"),
            "the switch is accepted and says so: {notice}"
        );
        // **One more key, and the reason is the loop's order rather than a
        // weakness in the claim.** The frame is composed near the top of a pass
        // and the Action's ask is drained near the bottom, so the palette that
        // changes during pass N is the one pass N+1 draws with — and a pass
        // happens when something arrives. In a session that is invisible,
        // because the next thing that arrives is you typing. Here it has to be
        // asked for. `j` is the smallest thing that certainly *produces* one —
        // `<esc>` in normal mode can be a no-op, and a key that draws no frame
        // would leave this waiting for a redraw that never comes.
        editor.press_quietly(b"j");
        let after = editor.screen().background(0, SCREEN.ws_col - 1);

        assert_ne!(
            before, after,
            "the buffer is drawn on the new theme's ground without a restart"
        );

        editor.leave_by(b":q!\r");
    }

    /// **An unknown theme is refused, and the editor keeps the one it has**
    /// (`T092`).
    ///
    /// The slug is resolved in the Action's arm rather than in the loop, so
    /// `:theme nonesuch`, an MCP call and a CLI verb all answer one sentence.
    /// What this adds is the half the door cannot prove: that a refused switch
    /// leaves the palette alone rather than half-applying it.
    #[test]
    fn an_unknown_theme_is_refused_and_changes_nothing() {
        let scratch = Scratch::new("theme-unknown");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.screen().background(0, SCREEN.ws_col - 1);

        let said = editor.shown_on_grid(b":theme nonesuch\r", "no theme called");
        let notice = (0..SCREEN.ws_row)
            .map(|row| said.line(row))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            notice.contains("no theme called nonesuch"),
            "the refusal names what was asked for: {notice}"
        );
        assert_eq!(
            before,
            editor.screen().background(0, SCREEN.ws_col - 1),
            "a refused switch leaves the palette it had"
        );

        editor.leave_by(b":q!\r");
    }
}
