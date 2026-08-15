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

            let child = Command::new(binary)
                .arg(file)
                .env("PHOSPHOR_RUNTIME", runtime)
                .env("XDG_STATE_HOME", state)
                .env("XDG_CONFIG_HOME", config_home(state))
                .env("TERM", "xterm-256color")
                .stdin(Stdio::from(slave.try_clone().expect("the slave clones")))
                .stdout(Stdio::from(slave.try_clone().expect("the slave clones")))
                .stderr(Stdio::from(slave))
                .spawn()
                .expect("the shipping binary starts");

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

        /// Writes `keys` and waits for the editor to go quiet, asserting
        /// nothing about frames.
        ///
        /// For a key whose effect is **in the buffer** rather than on a frame
        /// this harness can read. [`printable`]'s own doc says why that is a
        /// real distinction: a frame is a diff, so accepting a completion over
        /// a word that shares a prefix redraws only the suffix — `ault_delay`
        /// reaches the transcript and `let base = default_delay` never does.
        /// The assertion for those belongs on the file, after `:w`.
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
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "phosphor-pty-{name}-{}-{:?}",
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
        // `/` is vim's search. It is bound to the search prompt, which `T058`
        // builds, and the ex line already declines the same capability.
        let before = editor.mark();
        editor.press(b"/");
        let frame = editor.since(before);
        assert!(
            shows(
                &frame,
                "only the ex line exists yet — T058 builds the message and search prompts"
            ),
            "a deferred key names its task on the statusline; frame was: {frame}"
        );

        // `n` walks the search matches, and walking a sequence is
        // `goto-sequence` — declared at `T049` and not applied, so the refusal
        // is derived from that row rather than written anywhere.
        let after = editor.mark();
        editor.press(b"n");
        let frame = editor.since(after);
        assert!(
            shows(&frame, "not built yet — T049 builds it"),
            "the task comes off the capability's own row; frame was: {frame}"
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

    /// **`q` and `m` stopped being silent.** The repair window between `CP-3`
    /// and `S4` gave each a capability that means what the key means —
    /// `set-macro-recording` (`T099`) and `place-anchor` (`T042`) — so both now
    /// decline *by name* instead of resolving to a thunk that draws nothing.
    ///
    /// Through the pty rather than at the machine, because the two halves that
    /// carry this are in different crates and only the loop joins them: the
    /// keymap row is `runtime/keymaps.scm`'s, and for `q` the refusal is
    /// manufactured in `Session::key` — `Machine::apply`'s `SetMacroRecording`
    /// arm is a deliberate no-op, so an `Action::Input` that never reaches
    /// `Editing::apply` would otherwise succeed silently. A test that drove the
    /// machine would pass with the statusline saying nothing.
    #[test]
    fn the_macro_and_mark_keys_decline_by_naming_their_task() {
        let scratch = Scratch::new("named-refusal");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        // **One session each, and that is not fussiness.** The two sentences
        // differ only in `T099` versus `T042`, and ratatui redraws the diff —
        // pressing both in one session emits a frame carrying the two changed
        // cells and nothing else, so the second assertion would read `42` and
        // fail on a build that is correct.

        // `q` — vim's macro record. The task is read off the capability's own
        // row, so this sentence goes stale the day `T099` is ticked and the
        // arm is not replaced, which is the point of asserting the task id.
        let macros = Editor::open(&file, &scratch.state(), &runtime);
        let before = macros.mark();
        macros.press(b"q");
        let frame = macros.since(before);
        assert!(
            shows(&frame, "not built yet — T099 builds it"),
            "q names the task that builds the recorder; frame was: {frame}"
        );
        macros.quit();

        // `m` — set a mark. A mark is an anchor, `place-anchor` is the setter
        // that did not exist until this window, and `T042` is its row's task.
        let marks = Editor::open(&file, &scratch.state(), &runtime);
        let before = marks.mark();
        marks.press(b"m");
        let frame = marks.since(before);
        assert!(
            shows(&frame, "not built yet — T042 builds it"),
            "m names the task that anchors the store; frame was: {frame}"
        );
        marks.quit();
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

            let editor = Editor::open(&file, &scratch.state(), &runtime);
            editor.press(keys);
            editor.press(b"i");
            editor.press(b"|");
            editor.press(b"\x1b");
            editor.press(b":w\r");
            editor.quit();

            assert_eq!(
                fs::read_to_string(&file).expect("the file survives"),
                expected,
                "{name}: `|` marks where the cursor was left"
            );
        }
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
        let server = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/toy_language_server.py")
            .canonicalize()
            .expect("the toy server is beside this file");
        let form = format!(
            "(define-language! \"toy\"\n  (hash \"extensions\" '(\"toy\")\n        \
             \"grammar\" void\n        \"lsp_command\" (list \"python3\" {:?} {mode:?})\n        \
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
    /// `Queue::recv` with no timeout and no tick. The row is
    /// `phosphor_ui::virtual_text`'s and the `■` is §2's lexicon; the state
    /// column beside it is `gutter::state_column`, computed once by the loop.
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
        let frame = editor.press_until(b"", "expected Duration, found u128");
        assert!(
            shows(&frame, "\u{25a0}"),
            "§2's lexicon opens a diagnostic row with a filled square; frame was: {frame}"
        );
        editor.quit();
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
        assert!(
            !shows(&drawn, "denied to a producer"),
            "typing raised a producer refusal; frames were: {drawn}"
        );
        assert!(
            !shows(&drawn, "lsp:"),
            "typing said something about the lsp subsystem at all; frames were: {drawn}"
        );
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
        editor.press_quietly(b"gcgc");
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
    /// It is `T098`'s rule reaching one more key. `runtime/keymaps.scm` argued
    /// the other way and the argument is kept at the row it was wrong about;
    /// what settles it is that `request-references` is not a near-miss for what
    /// `gr` means, it is the verb, so the refusal names the task that builds the
    /// list rather than a task about something else.
    ///
    /// Both halves in one session, the way `a_deferred_key_does_not_spend_the_
    /// session_hint` presses `q` and `Q`: the refusal has to be *readable*, and
    /// a key that is known must not spend `8e`'s one teaching row.
    #[test]
    fn gr_declines_by_naming_the_task_that_builds_the_list() {
        let scratch = Scratch::new("references");
        let runtime = copy_layer(&scratch.path);
        let file = scratch.path.join("sample.txt");
        fs::write(&file, "one\ntwo\n").expect("a fixture");

        let editor = Editor::open(&file, &scratch.state(), &runtime);
        let before = editor.mark();
        editor.press(b"gr");
        let frame = editor.since(before);
        assert!(
            shows(&frame, "not built yet — T047 builds it"),
            "gr names the task that builds the surface a list of places is drawn in; \
             frame was: {frame}"
        );
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
        fn tinted(&self, y: u16) -> Vec<u16> {
            let start = usize::from(y) * usize::from(self.width);
            let row = &self.cells[start..start + usize::from(self.width)];
            let body = &row[row.len() - 1].background;
            (0..self.width)
                .filter(|column| &row[usize::from(*column)].background != body)
                .collect()
        }
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
}
