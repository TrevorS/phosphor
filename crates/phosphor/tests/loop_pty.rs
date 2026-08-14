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
        fs::create_dir_all(&to).expect("a runtime directory");
        for entry in fs::read_dir(&from).expect("the shipped layer") {
            let entry = entry.expect("a readable entry");
            if entry.path().extension().is_some_and(|ext| ext == "scm") {
                fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
            }
        }
        to
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
}
