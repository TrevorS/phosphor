//! `T091`'s acceptance criterion, as a runnable artifact: **real Steel
//! invocations flat while frames climb, on the loop that ships.**
//!
//! Run it with `just bench` (or `cargo bench -p phosphor`). It prints two
//! tables and a verdict, and asserts the structural half of what it prints.
//!
//! # Why this exists next to `T079`
//!
//! `phosphor-ui/benches/frame_cache.rs` proves the frame cache with a *Rust*
//! composer, because `phosphor-ui` may not depend on `phosphor-steel` —
//! `scripts/lint-no-store-mutation.sh` check 2 allows it exactly one
//! `phosphor-*` dependency. Its own header is careful about what that costs:
//! the control arm is a floor, not a lie. But nothing counted a real
//! `steel-core` invocation against frames drawn, and no in-crate harness could:
//! the binary is the only crate with `phosphor-steel` and `phosphor-ui` at
//! once, and the loop is `main.rs`'s, which is not a library.
//!
//! So this does not rebuild the loop. **It runs the shipping binary** —
//! `CARGO_BIN_EXE_phosphor`, the same executable `just install` puts on
//! `$PATH` — on a pseudoterminal, and counts two things from outside it.
//!
//! # The two counters, and why neither is a hook in the loop
//!
//! `T091` is a measurement, not a change to the loop, so neither number is
//! read through anything added for the measurement. Both were already exposed,
//! by contracts that exist for other reasons:
//!
//! * **Frames** — `phosphor-term` wraps every frame in a synchronized-output
//!   block, and `T014`'s acceptance criterion is that no frame can be emitted
//!   outside it (the writer is private to `phosphor_term`'s `raw` module; its
//!   own tests hold *one `?2026h` per frame*). So counting `ESC [ ? 2026 h` in
//!   the terminal's byte stream counts frames, exactly, with nothing added to
//!   the binary to make it countable.
//! * **Steel invocations** — Rust reaches into the VM for
//!   `phosphor/status-line` (`T025`) and `phosphor/resolve` (`T022`, `T033`)
//!   **by name, on every call**. That is the editor layer's liveness claim, and
//!   it is also the instrument: a copy of `runtime/` with a counting wrapper
//!   appended to `persisted.scm` — which loads last — redefines both, ticks a
//!   byte into a file, and delegates to what the shipped layer defined. The
//!   composition that runs is the shipped composition.
//!
//! The layer is *copied* rather than edited: `PHOSPHOR_RUNTIME`
//! (`phosphor_steel::runtime::RUNTIME_ENV`) points the child at the copy, so
//! the tree in the repo is untouched and what is measured is whatever
//! `runtime/*.scm` says today.
//!
//! # The two arms
//!
//! Both drive the shipping loop; they differ in what the events mean.
//!
//! * **quiet** — a focus event (`ESC [ I`) per rung step. The loop's own
//!   `_ => {}` arm swallows it and redraws on the next turn, so the frame count
//!   climbs with the ladder while nothing the statusline reads has moved. This
//!   is the arm the claim is about.
//! * **moving** — `j`/`k`, alternating. Every event moves the cursor, so
//!   `StatusVm` differs, the revision advances and the composer runs. This arm
//!   is the control that stops the first one being vacuous: a cache that had
//!   simply stopped composing would also read "flat".
//!
//! # A finding the second counter makes visible
//!
//! **The composer is cached; the keymap deliberately is not.** `phosphor/resolve`
//! is asked of the VM on *every keystroke* and never cached — that is `T022`'s
//! liveness claim, restated at the top of `runtime/keymaps.scm` ("it asks this
//! file, on every keystroke, and never caches the answer"). So in the moving
//! arm both counters track keystrokes, and the cache is doing its job on the
//! only one of the two it is allowed to. Counting a single "VM invocations"
//! number without that split would have made the claim look weaker than it is,
//! and hidden which door the frame cache actually guards.
//!
//! # What the numbers are not
//!
//! The wall-clock column is the whole child process, startup included — a fixed
//! per-run cost (terminal setup negotiates the keyboard protocol before the
//! first frame) that does not divide out cleanly, so `ms/frame` is offered as
//! an order of magnitude and nothing finer. The counting wrapper itself writes
//! and flushes a byte per invocation, which the moving arm pays once per
//! keystroke and the quiet arm pays once. And the child lays out at this pty's
//! size unless it inherits a controlling terminal, in which case `crossterm`
//! reads that one's geometry instead (`terminal/sys/unix.rs`'s `window_size`
//! opens `/dev/tty` first). Neither counter depends on the geometry.
//!
//! Owned by `spine`.

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

fn main() {
    #[cfg(unix)]
    measured::main();
    #[cfg(not(unix))]
    println!("T091: skipped — the measurement drives the shipping binary through a pty");
}

#[cfg(unix)]
mod measured {
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::unix::ffi::OsStringExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    use rustix::termios::{Winsize, tcsetwinsize};

    /// The screen the child lays out at — `9c`'s proportions, and `T079`'s, so
    /// the two benchmarks are drawing the same amount of chrome.
    const SCREEN: Winsize = Winsize {
        ws_row: 40,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    /// Events per run. 25 to 800 is a 32× climb, the same shape as `T079`'s
    /// frame-rate ladder; the claim is that the VM counters do not move across
    /// it.
    const LADDER: [u64; 6] = [25, 50, 100, 200, 400, 800];

    /// `ESC [ I` — a focus event. `crossterm` decodes it whether or not focus
    /// reporting was negotiated, and the loop's last match arm swallows it:
    /// *"A resize redraws from the new size on the next turn of the loop; so
    /// does everything else this arm swallows (focus, paste)"*. One turn of the
    /// loop, one frame, nothing touched.
    const QUIET: &[u8] = b"\x1b[I";

    /// `ZQ` — quit, force. `runtime/keymaps.scm` binds it; the loop breaks
    /// before drawing again, so the last event of every run is also the proof
    /// that every event before it was consumed.
    const QUIT: &[u8] = b"ZQ";

    /// The synchronized-output opener. One per frame, by `T014`'s construction.
    const FRAME: &[u8] = b"\x1b[?2026h";

    /// Bytes of the previous read the next one is scanned with, so a marker
    /// split across two reads is still found. One less than the longest needle.
    const OVERLAP: usize = FRAME.len() - 1;

    /// Events written before waiting for their frames. 64 events is at most 192
    /// bytes, comfortably inside a terminal's input queue.
    const CHUNK: u64 = 64;

    /// The primary device attributes query terminal setup sends. Answering it
    /// ends the keyboard-protocol negotiation immediately instead of after
    /// `crossterm`'s timeout; the answer says "no kitty protocol", which is the
    /// legacy path and the one where a keystroke is one event.
    const DA1_QUERY: &[u8] = b"\x1b[c";

    /// What we answer it with — a plain VT100 attributes report.
    const DA1_REPLY: &[u8] = b"\x1b[?6c";

    /// The file the child opens. `main.rs` is the loop being measured, so the
    /// editor holds its own event loop open while the frames are counted — and
    /// it is a real Rust buffer, tree-sitter highlighting and all, rather than
    /// a fixture sized to make the numbers look good.
    const FIXTURE: &str = include_str!("../src/main.rs");

    /// Which events a run feeds the loop.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Arm {
        /// Focus events: frames, and nothing else.
        Quiet,
        /// `j`/`k`: every event moves the cursor, so every event is a state
        /// change.
        Moving,
    }

    impl Arm {
        /// The byte sequence for one event at position `n`.
        fn event(self, n: u64) -> &'static [u8] {
            match self {
                Self::Quiet => QUIET,
                Self::Moving if n.is_multiple_of(2) => b"j",
                Self::Moving => b"k",
            }
        }

        fn title(self) -> &'static str {
            match self {
                Self::Quiet => "quiet — the loop turns and nothing the statusline reads moves",
                Self::Moving => {
                    "moving — every event moves the cursor, so every event is a state change"
                }
            }
        }
    }

    /// One row of the table.
    #[derive(Debug, Clone, Copy)]
    struct Measurement {
        /// Events written to the terminal, not counting the quit.
        events: u64,
        /// `?2026h` blocks in the child's output — frames, exactly.
        frames: u64,
        /// `phosphor/status-line` invocations. **The number the claim is
        /// about.**
        compositions: u64,
        /// `phosphor/resolve` invocations — the uncached door, for contrast.
        resolutions: u64,
        /// The whole child process, startup included.
        wall: Duration,
    }

    impl Measurement {
        /// Every real invocation of the editor layer this run made.
        const fn vm_total(&self) -> u64 {
            self.compositions + self.resolutions
        }

        /// Order of magnitude only — see the module header.
        fn millis_per_frame(&self) -> f64 {
            self.wall.as_secs_f64() * 1e3 / self.frames as f64
        }
    }

    pub(super) fn main() {
        let binary = PathBuf::from(env!("CARGO_BIN_EXE_phosphor"));
        let layer = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../runtime")
            .canonicalize()
            .expect("the shipped editor layer is where the workspace keeps it");

        println!("phosphor · T091 real VM invocations, in the shipping binary");
        println!("  binary  {}", binary.display());
        println!(
            "  layer   {} (copied per run, with a counting wrapper appended to persisted.scm)",
            layer.display()
        );
        println!(
            "  screen  {}x{} · buffer crates/phosphor/src/main.rs",
            SCREEN.ws_col, SCREEN.ws_row
        );
        println!();

        let mut quiet = Vec::new();
        let mut moving = Vec::new();
        for events in LADDER {
            quiet.push(run(Arm::Quiet, events, &binary, &layer));
            moving.push(run(Arm::Moving, events, &binary, &layer));
        }

        table(Arm::Quiet.title(), &quiet);
        table(Arm::Moving.title(), &moving);
        verdict(&quiet, &moving);
    }

    // -----------------------------------------------------------------------
    // The run
    // -----------------------------------------------------------------------

    /// One child process: boot it on an instrumented copy of the layer, feed it
    /// `events` events and then `ZQ`, and count what came back.
    fn run(arm: Arm, events: u64, binary: &Path, layer: &Path) -> Measurement {
        let scratch = Scratch::new();
        let compositions = scratch.path.join("compositions");
        let resolutions = scratch.path.join("resolutions");
        let runtime = scratch.path.join("runtime");
        copy_layer(layer, &runtime);
        instrument(&runtime, &compositions, &resolutions);

        let fixture = scratch.path.join("main.rs");
        fs::write(&fixture, FIXTURE).expect("the fixture buffer is written");

        let (master, slave_path) = open_pty();
        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&slave_path)
            .expect("the pty slave opens");
        // The size goes on the far end. Apple's master rejects `TIOCSWINSZ`
        // (`ENOTTY`); the slave takes it on both platforms, and it is the fd the
        // child will be asking anyway.
        tcsetwinsize(&slave, SCREEN).expect("the pty takes a window size");

        let started = Instant::now();
        let mut child = Command::new(binary)
            .arg(&fixture)
            .env("PHOSPHOR_RUNTIME", &runtime)
            // The wiring pass gave the loop an undo journal, keyed on the
            // workspace and the file (`main.rs`'s `Timeline`). A measurement
            // must not leave state in the user's real `$XDG_STATE_HOME`, so it
            // goes in the scratch that removes itself — and the journal's
            // per-keystroke cost is then measured rather than skipped.
            .env("XDG_STATE_HOME", scratch.path.join("state"))
            .env("TERM", "xterm-256color")
            .stdin(Stdio::from(slave.try_clone().expect("the slave clones")))
            .stdout(Stdio::from(slave.try_clone().expect("the slave clones")))
            .stderr(Stdio::from(slave))
            .spawn()
            .expect("the shipping binary starts");

        // Nothing on this side may hold a slave fd open, or the master never
        // sees end-of-file when the child exits and the reader never finishes.
        let drawn = Arc::new(AtomicU64::new(0));
        let reader = spawn_reader(Arc::clone(&master), Arc::clone(&drawn));

        // Raw mode is on by the time the first frame lands, so nothing written
        // after this point is echoed back into the stream being counted.
        await_frames(&drawn, 1, 0);

        // **Paced against the frames, not written in one go.** A terminal's
        // input queue is small — 1 KiB on Apple platforms — and a writer that
        // overruns it does not block politely: the far end drops what does not
        // fit, so the `ZQ` at the end of a long run never arrives and the child
        // waits on a key that was thrown away. Writing a chunk and then waiting
        // for its frames keeps the queue shallow, and makes the pacing itself
        // an assertion: the loop draws exactly one frame per event, so a chunk
        // that does not produce its frames is a failure rather than a stall.
        let mut written = 0;
        while written < events {
            let chunk = (events - written).min(CHUNK);
            let mut bytes = Vec::with_capacity(chunk as usize * QUIET.len());
            for n in written..written + chunk {
                bytes.extend_from_slice(arm.event(n));
            }
            (&*master)
                .write_all(&bytes)
                .expect("the child takes the events");
            written += chunk;
            await_frames(&drawn, written + 1, written);
        }
        (&*master)
            .write_all(QUIT)
            .expect("the child takes the quit");

        let status = child.wait().expect("the child exits");
        let wall = started.elapsed();
        reader.join().expect("the reader thread finishes");
        assert!(status.success(), "the shipping binary exited with {status}");

        Measurement {
            events,
            frames: drawn.load(Ordering::Relaxed),
            compositions: ticks(&compositions),
            resolutions: ticks(&resolutions),
            wall,
        }
    }

    /// Drains the master until end-of-file, counting frames as they go past and
    /// answering the device-attributes query the first time it appears.
    ///
    /// Draining is not optional: the child blocks once the terminal's output
    /// buffer fills, and a blocked child never reads the events written to it.
    ///
    /// Counted incrementally rather than by rescanning, over each read plus the
    /// last [`OVERLAP`] bytes of the one before it, so a marker split across two
    /// reads is still one frame and never two. Only the tail is kept: the run
    /// needs the count, not the transcript.
    fn spawn_reader(master: Arc<File>, drawn: Arc<AtomicU64>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut region: Vec<u8> = Vec::new();
            let mut answered = false;
            loop {
                // On end-of-file this reads zero on Apple platforms and fails
                // with `EIO` on Linux; both mean the child is gone.
                let Ok(read @ 1..) = (&*master).read(&mut buffer) else {
                    return;
                };
                region.extend_from_slice(&buffer[..read]);
                drawn.fetch_add(count(&region, FRAME), Ordering::Relaxed);
                if !answered && count(&region, DA1_QUERY) > 0 {
                    answered = true;
                    let _ = (&*master).write_all(DA1_REPLY);
                }
                let keep = region.len().saturating_sub(OVERLAP);
                region.drain(..keep);
            }
        })
    }

    /// Blocks until the child has drawn `target` frames.
    fn await_frames(drawn: &Arc<AtomicU64>, target: u64, written: u64) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let frames = drawn.load(Ordering::Relaxed);
            if frames >= target {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the shipping binary drew {frames} frames for {written} events, and then stopped"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // -----------------------------------------------------------------------
    // The instrumented layer
    // -----------------------------------------------------------------------

    /// Copies every `*.scm` in the shipped layer into `into`.
    ///
    /// Whole-directory rather than a listed set: `init.scm`'s
    /// `phosphor/boot-files` grows as the editor layer does, and a benchmark
    /// that named the files would quietly stop measuring the ones added after
    /// it was written.
    fn copy_layer(from: &Path, into: &Path) {
        fs::create_dir_all(into).expect("the scratch layer directory is created");
        let entries = fs::read_dir(from).unwrap_or_else(|error| {
            panic!("the shipped layer is at {}: {error}", from.display());
        });
        let mut copied = 0u32;
        for entry in entries {
            let path = entry.expect("the layer directory is readable").path();
            if path.extension().is_some_and(|kind| kind == "scm") {
                let name = path.file_name().expect("a file has a name");
                fs::copy(&path, into.join(name)).expect("the layer file is copied");
                copied += 1;
            }
        }
        assert!(copied > 0, "no *.scm found in {}", from.display());
    }

    /// Appends the counting wrapper to the copied `persisted.scm`.
    ///
    /// `persisted.scm` loads **last** (`init.scm`'s `phosphor/boot-files`, and
    /// its own header: *"it loads last, and that is the whole reason it exists"*),
    /// so these two definitions are the ones Rust finds when it asks for the
    /// names. Each delegates to the value the shipped layer bound, so what runs
    /// per invocation is the shipped composition plus one byte.
    ///
    /// If the layer ever stops making these the last definitions, the counter
    /// concerned reads zero in both arms and [`verdict`]'s liveness assert
    /// fails, rather than a flat line being reported for something nothing
    /// measured.
    fn instrument(runtime: &Path, compositions: &Path, resolutions: &Path) {
        let scheme = format!(
            "\n\
             ;; T091 — the counting wrapper, appended to a *copy* of the shipped layer.\n\
             (define phosphor/t091-composer phosphor/status-line)\n\
             (define phosphor/t091-resolver phosphor/resolve)\n\
             (define phosphor/t091-compositions (open-output-file \"{compositions}\"))\n\
             (define phosphor/t091-resolutions (open-output-file \"{resolutions}\"))\n\
             (define (phosphor/t091-tick port)\n\
             \x20 (write-string \".\" port)\n\
             \x20 (flush-output-port port))\n\
             (define (phosphor/status-line vm)\n\
             \x20 (phosphor/t091-tick phosphor/t091-compositions)\n\
             \x20 (phosphor/t091-composer vm))\n\
             (define (phosphor/resolve scope keys)\n\
             \x20 (phosphor/t091-tick phosphor/t091-resolutions)\n\
             \x20 (phosphor/t091-resolver scope keys))\n",
            compositions = scheme_path(compositions),
            resolutions = scheme_path(resolutions),
        );
        let persisted = runtime.join("persisted.scm");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&persisted)
            .unwrap_or_else(|error| panic!("{} is appendable: {error}", persisted.display()));
        file.write_all(scheme.as_bytes())
            .expect("the wrapper is appended");
    }

    /// A path as a scheme string literal. The temp directory is ours and has no
    /// quote or backslash in it; this is the assertion that says so rather than
    /// an escaper nobody would exercise.
    fn scheme_path(path: &Path) -> String {
        let text = path.to_str().expect("the scratch path is UTF-8").to_owned();
        assert!(
            !text.contains(['"', '\\']),
            "the scratch path needs escaping: {text}"
        );
        text
    }

    /// One byte per invocation, so the file's length is the count.
    fn ticks(path: &Path) -> u64 {
        fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // The terminal
    // -----------------------------------------------------------------------

    /// A pseudoterminal, and the path of its far end. The caller opens that end
    /// and puts [`SCREEN`] on it.
    ///
    /// `rustix` rather than raw `ioctl`: the workspace denies `unsafe_code`, and
    /// a benchmark is not a reason to make an exception.
    fn open_pty() -> (Arc<File>, PathBuf) {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pty is available");
        grantpt(&master).expect("the pty is granted");
        unlockpt(&master).expect("the pty is unlocked");
        let name = ptsname(&master, Vec::new()).expect("the pty has a name");
        let path = PathBuf::from(OsString::from_vec(name.into_bytes()));
        (Arc::new(File::from(master)), path)
    }

    /// Occurrences of `needle`, non-overlapping being irrelevant for the
    /// sequences counted here.
    fn count(haystack: &[u8], needle: &[u8]) -> u64 {
        haystack
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count() as u64
    }

    /// A scratch directory that removes itself.
    #[derive(Debug)]
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "phosphor-t091-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("the scratch directory is created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // -----------------------------------------------------------------------
    // Output
    // -----------------------------------------------------------------------

    fn table(title: &str, rows: &[Measurement]) {
        println!("{title}");
        println!(
            "    events    frames    compositions    resolutions    vm total    ms/frame    wall s"
        );
        for row in rows {
            println!(
                "  {:>8}    {:>6}    {:>12}    {:>11}    {:>8}    {:>8.2}    {:>6.2}",
                row.events,
                row.frames,
                row.compositions,
                row.resolutions,
                row.vm_total(),
                row.millis_per_frame(),
                row.wall.as_secs_f64(),
            );
        }
        println!();
    }

    fn verdict(quiet: &[Measurement], moving: &[Measurement]) {
        let first = quiet.first().expect("the ladder is not empty");
        let last = quiet.last().expect("the ladder is not empty");
        let moved = moving.last().expect("the ladder is not empty");

        let climbed = quiet.windows(2).all(|pair| pair[1].frames > pair[0].frames);
        let compositions_flat = quiet
            .iter()
            .all(|row| row.compositions == first.compositions);
        let resolutions_flat = quiet.iter().all(|row| row.resolutions == first.resolutions);
        // **Both** counters, because either one stuck at zero would read as a
        // flat line above without measuring anything.
        let live = moving.windows(2).all(|pair| {
            pair[1].compositions > pair[0].compositions && pair[1].resolutions > pair[0].resolutions
        });

        println!("verdict");
        println!(
            "  frames             {} → {}   ({}× climb)",
            first.frames,
            last.frames,
            last.frames / first.frames.max(1),
        );
        println!(
            "  compositions       {} → {}   ({})",
            first.compositions,
            last.compositions,
            if compositions_flat { "FLAT" } else { "MOVED" },
        );
        println!(
            "  resolutions        {} → {}   ({})",
            first.resolutions,
            last.resolutions,
            if resolutions_flat { "FLAT" } else { "MOVED" },
        );
        println!(
            "  vm invocations     {} → {}   over {} frames",
            first.vm_total(),
            last.vm_total(),
            last.frames,
        );
        println!(
            "  one composition per {:.0} frames at the top of the ladder",
            last.frames as f64 / last.compositions.max(1) as f64,
        );
        println!();
        println!(
            "  the same loop, when the events do move state: {} events → {} compositions,",
            moved.events, moved.compositions,
        );
        println!(
            "  {} resolutions over {} frames — the composer is cached, the keymap is asked",
            moved.resolutions, moved.frames,
        );
        println!("  on every keystroke and never cached (T022's liveness claim, keymaps.scm).");
        println!();
        println!("  (this is the shipping binary on a pty, not a harness around the parts:");
        println!("   frames are T014's synchronized-output blocks, and the invocations are the");
        println!("   editor layer counting itself through the redefinition it already allows)");
        println!();
        println!(
            "  T091: {}",
            if climbed && compositions_flat && resolutions_flat && live {
                "PASS — real Steel invocations flat while frames climb"
            } else {
                "FAIL — see the tables above"
            }
        );

        assert!(climbed, "frames did not climb with the ladder");
        assert!(
            compositions_flat,
            "the composer ran a different number of times when only the frame count changed"
        );
        assert!(
            resolutions_flat,
            "the keymap was asked a different number of times when no key was pressed"
        );
        assert!(
            live,
            "the moving arm did not run the editor layer more often than the quiet one — a \
             counter is measuring nothing, so the flat line above is vacuous"
        );
    }
}
