//! `T030`'s acceptance criterion, as tests.
//!
//! *Done when: undo history survives a clean restart **and** a `kill -9`*
//! (`docs/TASKS.md`, `T030`), and both halves are `CP-3` gate items. The second
//! is what makes the format append-only rather than write-on-exit, so it is
//! tested with a real process and a real `SIGKILL` — [`Child::kill`] sends
//! signal 9 on Unix — rather than by dropping a struct. Dropping a struct
//! proves that `Drop` was not load-bearing; it does not produce the tail a
//! crash produces.
//!
//! # How the process tests work
//!
//! There is no second binary to spawn and none is needed: a test binary can
//! re-execute itself. [`child_process_body`] is a normal test that returns
//! immediately unless `PHOSPHOR_JOURNAL_CHILD` names a mode, and the process
//! tests spawn `current_exe()` with that variable set plus
//! `--exact child_process_body`. Under `just test` it costs nothing; under a
//! parent it is the child.
//!
//! Three modes, and each is a different thing that can happen to a session:
//!
//! * `park` — commit a history, then deliberately write five bytes of a sixth
//!   frame and stop. Nothing is `fsync`ed, on purpose: what survives is what a
//!   plain `write_all` left in the page cache, which is the claim the append
//!   path rests on. Those five bytes are the half-written record at the tail, put
//!   there on purpose because a single `write_all` of a small frame is not
//!   interruptible on a local filesystem: the kernel takes the whole frame or
//!   none of it. The tail a crash leaves comes from a write that spans
//!   syscalls or from a filesystem that persists a prefix, and this constructs
//!   it rather than racing for it. The `SIGKILL` that follows is real.
//! * `flood` — commit a history, then append forever. The kill lands at an
//!   arbitrary point, and what must be true is that everything committed before
//!   it is intact.
//! * `exit` — commit a history, `fsync`, and exit normally. The clean-restart
//!   half, and the one place [`Log::sync`] is exercised.
//!
//! # Why every test makes its own directory
//!
//! State lives under the XDG state dir, and `SPIKES.md`'s hygiene table says
//! `cargo-nextest` is the runner *because* tests that touch it go flaky under a
//! shared process. Each test here builds a temporary directory, passes it as an
//! explicit state home ([`journal::workspace_dir_in`]) or as `XDG_STATE_HOME` on
//! a child's `Command`, and removes it on drop. Nothing here can see the user's
//! real state directory, and `std::env::set_var` is never called — it is
//! `unsafe` in edition 2024 and this workspace denies `unsafe_code`.

#![cfg(unix)]

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use phosphor_core::journal::{
    self, DecodeError, Decoder, Encoder, FoldError, Folded, Log, Recovery, Stream, UndoLog, undo,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A directory that removes itself. No `tempfile` dependency: this crate is
/// dependency-free at the floor and a test is not the place to change that.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "phosphor-journal-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn made(&self, name: &str) -> PathBuf {
        let path = self.join(name);
        fs::create_dir_all(&path).expect("subdir");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

fn edit(at: usize, removed: &str, inserted: &str) -> undo::Edit {
    undo::Edit {
        at,
        removed: removed.to_string(),
        inserted: inserted.to_string(),
    }
}

fn caret(offset: usize) -> undo::Caret {
    undo::Caret {
        offset,
        selection: None,
    }
}

/// The session every process test writes: three edits, one of them on a branch
/// taken after an undo, and a save.
///
/// Deliberately branchy. A linear history round-trips through anything; the
/// pointer that says *which* branch a redo takes is the one a naive format
/// loses, and `undo.rs:869-883` is explicit that a restored tree falling back to
/// the newest child is a real behaviour difference.
fn session(origin: &str) -> Vec<undo::Record> {
    vec![
        undo::Record::Origin {
            path: origin.to_string(),
        },
        undo::Record::Node {
            id: 1,
            parent: 0,
            edits: vec![edit(0, "", "hello")],
            before: caret(0),
            after: caret(5),
        },
        undo::Record::Node {
            id: 2,
            parent: 1,
            edits: vec![edit(5, "", " world")],
            before: caret(5),
            after: caret(11),
        },
        // undo back to node 1, then type something else: the divergence the
        // vendored fork's stack would have destroyed.
        undo::Record::Cursor { to: 1 },
        undo::Record::Node {
            id: 3,
            parent: 1,
            edits: vec![edit(5, "", "?")],
            before: caret(5),
            after: caret(6),
        },
        undo::Record::Saved { node: Some(3) },
    ]
}

/// What [`session`] folds to, asserted field by field.
fn assert_session(history: &undo::History, origin: &str) {
    assert_eq!(history.origin(), Some(origin), "origin");
    assert_eq!(history.nodes().len(), 4, "root plus three nodes");
    assert_eq!(history.current(), 3, "the branch we ended on");
    assert_eq!(history.saved(), Some(3), "saved at the branch");

    let nodes = history.nodes();
    assert_eq!(nodes[0].parent, None, "the root has no parent");
    assert_eq!(nodes[0].change, None, "the root has no change");
    assert_eq!(nodes[0].children, vec![1], "the root's children");
    assert_eq!(nodes[0].redo_child, Some(1), "the root's live branch");

    assert_eq!(nodes[1].parent, Some(0));
    assert_eq!(nodes[1].children, vec![2, 3], "both branches survive");
    assert_eq!(
        nodes[1].redo_child,
        Some(3),
        "the branch taken after the undo, not the newest by accident"
    );

    let change = nodes[2].change.as_ref().expect("node 2 has a change");
    assert_eq!(change.edits, vec![edit(5, "", " world")]);
    assert_eq!(change.before, caret(5));
    assert_eq!(change.after, caret(11));
    assert_eq!(nodes[2].children, Vec::<u64>::new());
    assert_eq!(nodes[2].redo_child, None);

    assert_eq!(nodes[3].parent, Some(1));
    let change = nodes[3].change.as_ref().expect("node 3 has a change");
    assert_eq!(change.edits, vec![edit(5, "", "?")]);
}

fn write_session(log: &mut UndoLog, origin: &str) {
    for record in session(origin) {
        log.append(record).expect("append");
    }
}

// ---------------------------------------------------------------------------
// The codec
// ---------------------------------------------------------------------------

#[test]
fn every_primitive_round_trips() {
    let mut out = Encoder::new();
    out.u64(0);
    out.u64(127);
    out.u64(128);
    out.u64(u64::MAX);
    out.usize(usize::MAX);
    out.bool(true);
    out.bool(false);
    out.str("");
    out.str("héllo ↪ 世界");
    out.option_u64(None);
    out.option_u64(Some(9));
    out.seq_len(3);
    out.bool(true);
    out.bool(false);
    out.bool(true);
    let bytes = out.finish();

    let mut input = Decoder::new(&bytes);
    assert_eq!(input.u64().expect("0"), 0);
    assert_eq!(input.u64().expect("127"), 127);
    assert_eq!(input.u64().expect("128"), 128);
    assert_eq!(input.u64().expect("max"), u64::MAX);
    assert_eq!(input.usize().expect("usize"), usize::MAX);
    assert!(input.bool().expect("true"));
    assert!(!input.bool().expect("false"));
    assert_eq!(input.str().expect("empty"), "");
    assert_eq!(input.str().expect("unicode"), "héllo ↪ 世界");
    assert_eq!(input.option_u64().expect("none"), None);
    assert_eq!(input.option_u64().expect("some"), Some(9));
    assert_eq!(input.seq_len().expect("len"), 3);
    assert!(input.bool().expect("item"));
    assert!(!input.bool().expect("item"));
    assert!(input.bool().expect("item"));
    input.finish().expect("fully consumed");
}

#[test]
fn a_truncated_value_is_an_error_not_a_guess() {
    let mut out = Encoder::new();
    out.str("hello");
    let bytes = out.finish();

    let mut input = Decoder::new(&bytes[..3]);
    assert_eq!(input.str(), Err(DecodeError::UnexpectedEnd));
}

#[test]
fn a_sequence_longer_than_the_record_is_refused_before_it_allocates() {
    let mut out = Encoder::new();
    out.seq_len(usize::MAX);
    let bytes = out.finish();

    let mut input = Decoder::new(&bytes);
    assert!(matches!(input.seq_len(), Err(DecodeError::TooLong { .. })));
}

#[test]
fn trailing_bytes_are_a_schema_bug_and_say_so() {
    let mut out = Encoder::new();
    out.u64(1);
    out.u64(2);
    let bytes = out.finish();

    let mut input = Decoder::new(&bytes);
    assert_eq!(input.u64().expect("first"), 1);
    assert_eq!(input.finish(), Err(DecodeError::Trailing { extra: 1 }));
}

#[test]
fn every_undo_record_round_trips() {
    let records = [
        undo::Record::Origin {
            path: "/tmp/a.rs".to_string(),
        },
        undo::Record::Base {
            text: "fn main() {}\n".to_string(),
        },
        undo::Record::Node {
            id: 7,
            parent: 3,
            edits: vec![edit(0, "old", "new"), edit(12, "", "x")],
            before: undo::Caret {
                offset: 4,
                selection: Some(undo::CharRange { start: 1, end: 9 }),
            },
            after: caret(13),
        },
        undo::Record::Cursor { to: 2 },
        undo::Record::Redo { node: 1, child: 2 },
        undo::Record::Saved { node: None },
        undo::Record::Saved { node: Some(4) },
    ];

    for record in records {
        let mut out = Encoder::new();
        undo::History::encode(&record, &mut out);
        let bytes = out.finish();
        assert_eq!(
            undo::History::decode(&bytes).expect("decodes"),
            record,
            "round trip"
        );
    }
}

#[test]
fn an_unknown_record_tag_names_itself() {
    let mut out = Encoder::new();
    out.u64(99);
    let bytes = out.finish();
    assert_eq!(
        undo::History::decode(&bytes),
        Err(DecodeError::UnknownRecord { tag: 99 })
    );
}

// ---------------------------------------------------------------------------
// The fold
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_history_is_a_tree_at_the_root() {
    let history = undo::History::default();
    assert_eq!(history.nodes().len(), 1);
    assert_eq!(history.current(), undo::ROOT);
    assert_eq!(
        history.saved(),
        Some(undo::ROOT),
        "just opened the file means the buffer matches disk"
    );
}

#[test]
fn the_fold_reproduces_the_tree_the_records_describe() {
    let mut history = undo::History::default();
    for record in session("/tmp/a.rs") {
        history.apply(record).expect("applies");
    }
    assert_session(&history, "/tmp/a.rs");
}

#[test]
fn into_parts_hands_back_what_from_parts_takes() {
    let mut history = undo::History::default();
    for record in session("/tmp/a.rs") {
        history.apply(record).expect("applies");
    }
    let (nodes, current, saved) = history.into_parts();
    assert_eq!(nodes.len(), 4);
    assert_eq!(current, 3);
    assert_eq!(saved, Some(3));
}

#[test]
fn a_node_out_of_creation_order_is_refused() {
    let mut history = undo::History::default();
    let record = undo::Record::Node {
        id: 4,
        parent: 0,
        edits: vec![edit(0, "", "x")],
        before: caret(0),
        after: caret(1),
    };
    assert_eq!(
        history.apply(record),
        Err(FoldError::OutOfOrder {
            found: 4,
            expected: 1
        })
    );
}

#[test]
fn a_parent_that_is_not_before_its_child_is_refused() {
    let mut history = undo::History::default();
    let record = undo::Record::Node {
        id: 1,
        parent: 1,
        edits: vec![edit(0, "", "x")],
        before: caret(0),
        after: caret(1),
    };
    assert_eq!(
        history.apply(record),
        Err(FoldError::BadParent { id: 1, parent: 1 }),
        "this is the invariant that makes dropping a torn tail safe"
    );
}

#[test]
fn a_cursor_or_save_naming_a_missing_node_is_refused() {
    let mut history = undo::History::default();
    assert_eq!(
        history.apply(undo::Record::Cursor { to: 9 }),
        Err(FoldError::UnknownNode { id: 9 })
    );
    assert_eq!(
        history.apply(undo::Record::Saved { node: Some(9) }),
        Err(FoldError::UnknownNode { id: 9 })
    );
}

#[test]
fn a_redo_pointer_that_is_not_a_child_is_refused() {
    let mut history = undo::History::default();
    for record in session("/tmp/a.rs") {
        history.apply(record).expect("applies");
    }
    assert_eq!(
        history.apply(undo::Record::Redo { node: 2, child: 1 }),
        Err(FoldError::BadRedoChild { id: 2, child: 1 })
    );
}

#[test]
fn a_second_origin_for_a_different_file_is_refused() {
    let mut history = undo::History::default();
    history
        .apply(undo::Record::Origin {
            path: "/tmp/a.rs".to_string(),
        })
        .expect("first");
    assert_eq!(
        history.apply(undo::Record::Origin {
            path: "/tmp/b.rs".to_string()
        }),
        Err(FoldError::WrongOrigin),
        "a hash collision must not silently merge two files' histories"
    );
}

/// The law the whole compaction path rests on.
#[test]
fn folding_a_snapshot_reproduces_the_state() {
    let mut history = undo::History::default();
    for record in session("/tmp/a.rs") {
        history.apply(record).expect("applies");
    }
    // Extra churn, so the snapshot has something to collapse and the redo
    // pointers have somewhere to go wrong.
    for record in [
        undo::Record::Cursor { to: 2 },
        undo::Record::Cursor { to: 0 },
        undo::Record::Cursor { to: 2 },
        undo::Record::Saved { node: None },
    ] {
        history.apply(record).expect("applies");
    }

    let mut replayed = undo::History::default();
    for record in history.snapshot() {
        replayed.apply(record).expect("snapshot replays");
    }
    assert_eq!(replayed, history, "fold(snapshot(state)) == state");
}

// ---------------------------------------------------------------------------
// Framing and recovery
// ---------------------------------------------------------------------------

#[test]
fn a_log_reopens_with_everything_it_was_given() {
    let tmp = TempDir::new("reopen");
    let path = tmp.join("undo.journal");

    {
        let (mut log, recovery) = UndoLog::open(&path).expect("create");
        assert_eq!(recovery, Recovery::default(), "a new log lost nothing");
        write_session(&mut log, "/tmp/a.rs");
    }

    let (log, recovery) = UndoLog::open(&path).expect("reopen");
    assert!(recovery.is_clean(), "clean close, clean open");
    assert_eq!(recovery.records, 6);
    assert_session(log.state(), "/tmp/a.rs");
}

#[test]
fn a_journal_of_the_wrong_stream_is_refused_at_the_header() {
    let tmp = TempDir::new("stream");
    let path = tmp.join("undo.journal");
    drop(UndoLog::open(&path).expect("create"));

    let opened = Log::<undo::History>::open(&path);
    assert!(opened.is_ok(), "the right stream still opens");

    let err = journal::Journal::open(&path, Stream::SEEN).expect_err("wrong stream");
    let message = err.to_string();
    assert!(
        message.contains("holds undo/1"),
        "the error says what it found: {message}"
    );
}

#[test]
fn a_file_that_is_not_a_journal_is_refused() {
    let tmp = TempDir::new("notajournal");
    let path = tmp.join("undo.journal");
    fs::write(&path, b"this is not a journal at all").expect("write");
    let err = UndoLog::open(&path).expect_err("refused");
    assert!(err.to_string().contains("not a phosphor journal"));
}

#[test]
fn a_half_written_record_at_the_tail_is_dropped_and_truncated_away() {
    let tmp = TempDir::new("torn");
    let path = tmp.join("undo.journal");

    {
        let (mut log, _) = UndoLog::open(&path).expect("create");
        write_session(&mut log, "/tmp/a.rs");
        log.sync().expect("sync");
    }
    let intact = fs::metadata(&path).expect("stat").len();

    // Five bytes of a sixth frame: a length prefix and one stray byte.
    let mut file = OpenOptions::new().append(true).open(&path).expect("append");
    file.write_all(&[0xff, 0xff, 0x00, 0x00, 0x7f])
        .expect("torn tail");
    drop(file);

    let (mut log, recovery) = UndoLog::open(&path).expect("recovers");
    assert_eq!(recovery.discarded_bytes, 5, "exactly the torn tail");
    assert_eq!(recovery.records, 6, "and everything before it");
    assert_session(log.state(), "/tmp/a.rs");
    assert_eq!(
        fs::metadata(&path).expect("stat").len(),
        intact,
        "the file is truncated to the last good boundary"
    );

    // And the log is writable again — the point of truncating rather than
    // merely stopping the read.
    log.append(undo::Record::Cursor { to: 2 }).expect("append");
    drop(log);
    let (log, recovery) = UndoLog::open(&path).expect("reopen");
    assert!(recovery.is_clean());
    assert_eq!(log.state().current(), 2);
}

#[test]
fn a_corrupted_payload_stops_the_read_at_that_record() {
    let tmp = TempDir::new("corrupt");
    let path = tmp.join("undo.journal");

    {
        let (mut log, _) = UndoLog::open(&path).expect("create");
        write_session(&mut log, "/tmp/a.rs");
    }

    // Flip a bit inside the last record's payload. The CRC covers the length
    // and the payload, so this is indistinguishable from a torn write and gets
    // the same answer: keep the prefix.
    let mut bytes = fs::read(&path).expect("read");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&path, &bytes).expect("write");

    let (log, recovery) = UndoLog::open(&path).expect("recovers");
    assert!(!recovery.is_clean(), "the corruption was noticed");
    assert_eq!(
        recovery.records, 5,
        "five records survive, the sixth does not"
    );
    assert_eq!(
        log.state().saved(),
        Some(0),
        "the save was the record that went; the rest of the history stands"
    );
    assert_eq!(log.state().current(), 3);
}

#[test]
fn compaction_shrinks_the_log_and_keeps_the_state() {
    let tmp = TempDir::new("compact");
    let path = tmp.join("undo.journal");

    let (mut log, _) = UndoLog::open(&path).expect("create");
    write_session(&mut log, "/tmp/a.rs");
    for _ in 0..500 {
        log.append(undo::Record::Cursor { to: 2 }).expect("append");
        log.append(undo::Record::Cursor { to: 3 }).expect("append");
    }
    let before = log.journal().byte_len();
    assert!(log.should_compact(), "a thousand cursor moves is doubling");

    let expected = log.state().clone();
    assert!(log.compact_if_needed().expect("compact"), "it compacted");
    assert_eq!(log.state(), &expected, "compaction is not a mutation");
    assert!(
        log.journal().byte_len() < before,
        "the point of compaction: {} < {before}",
        log.journal().byte_len()
    );
    assert!(!log.should_compact(), "and it is not due again immediately");

    drop(log);
    let (log, recovery) = UndoLog::open(&path).expect("reopen");
    assert!(recovery.is_clean());
    assert_eq!(log.state(), &expected, "and it reads back the same");
}

#[test]
fn a_crash_during_compaction_leaves_the_old_log_whole() {
    let tmp = TempDir::new("compact-crash");
    let path = tmp.join("undo.journal");

    {
        let (mut log, _) = UndoLog::open(&path).expect("create");
        write_session(&mut log, "/tmp/a.rs");
        log.sync().expect("sync");
    }

    // The sibling a compaction writes before its rename. Its presence is what a
    // crash mid-compaction leaves; the rename is what makes it invisible.
    let orphan = tmp.join("undo.journal.compacting");
    fs::write(&orphan, b"half a compaction").expect("write");

    let (log, recovery) = UndoLog::open(&path).expect("reopen");
    assert!(recovery.is_clean(), "the live log was never touched");
    assert_session(log.state(), "/tmp/a.rs");
}

// ---------------------------------------------------------------------------
// Where state lives — Q1
// ---------------------------------------------------------------------------

#[test]
fn a_workspace_directory_is_keyed_on_the_canonical_root() {
    let tmp = TempDir::new("xdg");
    let home = tmp.made("state");
    let root = tmp.made("workspace");

    let dir = journal::workspace_dir_in(&home, &root).expect("dir");
    assert!(dir.starts_with(home.join("phosphor")), "under phosphor/");
    assert_eq!(
        dir,
        journal::workspace_dir_in(&home, &root).expect("again"),
        "and it is stable"
    );

    let canonical = fs::canonicalize(&root).expect("canonical");
    assert_eq!(
        dir.file_name().and_then(|name| name.to_str()),
        Some(journal::workspace_key(&canonical).as_str()),
        "the directory name is the key"
    );
    assert_eq!(
        fs::read_to_string(dir.join("root")).expect("marker"),
        canonical.to_string_lossy(),
        "and it records which root it belongs to"
    );

    let other = tmp.made("elsewhere");
    assert_ne!(
        journal::workspace_dir_in(&home, &other).expect("second root"),
        dir,
        "a different root gets a different bucket"
    );
}

#[test]
fn the_key_is_ours_and_stable() {
    // Not `DefaultHasher`, which is explicitly unstable across releases: a
    // toolchain bump would silently orphan every user's state. Pinned by a
    // literal so a change to the hash is a failing test rather than a mystery.
    assert_eq!(journal::key(b""), "cbf29ce484222325");
    assert_eq!(journal::key(b"phosphor"), "cf438874015577ce");
    assert_eq!(journal::key(b"phosphor").len(), 16);
}

#[test]
fn two_roots_in_one_bucket_is_loud() {
    let tmp = TempDir::new("collision");
    let home = tmp.made("state");
    let root = tmp.made("workspace");

    let dir = journal::workspace_dir_in(&home, &root).expect("dir");
    fs::write(dir.join("root"), b"/somewhere/else").expect("plant");

    let err = journal::workspace_dir_in(&home, &root).expect_err("collision");
    let message = err.to_string();
    assert!(
        message.contains("/somewhere/else"),
        "the error names the occupant: {message}"
    );
}

/// **The `S4` flake, as a test.** `RootCollision { occupant: "" }`, twice, on
/// two different process tests that both pass alone.
///
/// The empty occupant is the whole diagnosis. A bucket is claimed by writing
/// the canonical root into `<dir>/root`, and `fs::write` is create-truncate-
/// write: between the truncate and the write the file exists and says nothing.
/// A reader in that window read an empty marker, compared it against its own
/// root, and reported a collision with a root that never existed.
///
/// It is **not** two tests sharing a bucket — every test here builds its own
/// `TempDir` and its own state home. The two racers are the parent and the
/// child *inside one test*: [`ChildSession::start`] spawns the child, the child
/// calls `journal::workspace_dir` on the root it was given, and the parent
/// calls [`child_paths`] on the same root a moment later. Two processes, one
/// root, one marker — which is exactly what two phosphor windows on one
/// repository are, so the fix belongs in `journal.rs` and not here.
#[test]
fn a_marker_caught_mid_write_is_not_an_occupant() {
    let tmp = TempDir::new("torn-marker");
    let home = tmp.made("state");
    let root = tmp.made("workspace");

    let dir = journal::workspace_dir_in(&home, &root).expect("dir");
    fs::write(dir.join("root"), b"").expect("plant what the racer saw");

    let again = journal::workspace_dir_in(&home, &root)
        .expect("an empty marker is unclaimed, not occupied");
    assert_eq!(again, dir);
    assert_eq!(
        fs::read_to_string(dir.join("root")).expect("marker"),
        fs::canonicalize(&root)
            .expect("canonical")
            .to_string_lossy(),
        "and the arrival claims the bucket rather than failing"
    );
}

/// The same race, run rather than reasoned about: several openers of one fresh
/// root at once, none of which may see a half-claimed bucket.
#[test]
fn openers_of_one_root_never_see_a_half_claimed_bucket() {
    let tmp = TempDir::new("marker-race");
    let home = tmp.made("state");
    // Fresh root per round: the claim happens once per bucket, so the window
    // this is aimed at is only open on the first opener of each one.
    for round in 0..64 {
        let root = tmp.made(&format!("workspace-{round}"));
        std::thread::scope(|scope| {
            let racers: Vec<_> = (0..4)
                .map(|_| scope.spawn(|| journal::workspace_dir_in(&home, &root)))
                .collect();
            for racer in racers {
                racer
                    .join()
                    .expect("the opener did not panic")
                    .expect("one root, one bucket, no collision");
            }
        });
    }
}

#[test]
fn one_journal_per_file() {
    let tmp = TempDir::new("undo-path");
    let dir = tmp.made("state");
    let a = journal::undo_path(&dir, Path::new("/w/src/a.rs"));
    let b = journal::undo_path(&dir, Path::new("/w/src/b.rs"));
    assert_ne!(a, b, "compacting one file does not rewrite another");
    assert!(a.starts_with(dir.join("undo")));
    assert_eq!(a.extension().and_then(|ext| ext.to_str()), Some("journal"));
}

// ---------------------------------------------------------------------------
// Restart and `kill -9` — the two halves of the acceptance criterion
// ---------------------------------------------------------------------------

/// The clean-restart half, across a real process boundary.
#[test]
fn history_survives_a_clean_restart() {
    let mut session = ChildSession::start("exit");
    let status = session.child_waits().expect("child exits");
    assert!(status.success(), "the child exited normally: {status}");
    session.assert_history();
}

/// The `kill -9` half, with a half-written record on disk at the moment of
/// death. See this file's header for why the tail is constructed rather than
/// raced for; the kill itself is a real `SIGKILL` to a real process.
#[test]
fn history_survives_a_kill_9_with_a_torn_tail() {
    let mut session = ChildSession::start("park");
    session.wait_for_ready();
    let status = session.kill_9();
    assert_eq!(status.signal(), Some(9), "a real SIGKILL: {status}");

    let (log, recovery) = UndoLog::open(&session.journal).expect("reopen after the kill");
    assert_eq!(recovery.discarded_bytes, 5, "the half-written record");
    assert_eq!(recovery.records, 6, "and nothing else");
    assert_session(log.state(), &session.origin);
}

/// The same kill, landing at an arbitrary point in a stream of appends rather
/// than at a place the test chose.
#[test]
fn history_survives_a_kill_9_mid_append() {
    let mut session = ChildSession::start("flood");
    session.wait_for_ready();
    sleep(Duration::from_millis(150));
    let status = session.kill_9();
    assert_eq!(status.signal(), Some(9), "a real SIGKILL: {status}");

    let (log, _) = UndoLog::open(&session.journal).expect("reopen after the kill");
    let history = log.state();
    assert_eq!(history.origin(), Some(session.origin.as_str()));
    assert_eq!(history.nodes().len(), 4, "every committed node is there");
    assert_eq!(history.saved(), Some(3), "and the save with them");
    assert!(
        history.current() <= 3,
        "whatever survived past the commit is a valid prefix"
    );
}

// ---------------------------------------------------------------------------
// The child
// ---------------------------------------------------------------------------

/// A spawned copy of this test binary, and the state directory it writes into.
#[derive(Debug)]
struct ChildSession {
    _dir: TempDir,
    child: Child,
    ready: PathBuf,
    journal: PathBuf,
    origin: String,
}

impl ChildSession {
    fn start(mode: &str) -> Self {
        let dir = TempDir::new(mode);
        let home = dir.made("state");
        let root = dir.made("workspace");
        let file = root.join("edited.rs");
        fs::write(&file, b"hello world\n").expect("the file being edited");

        let ready = dir.join("ready");
        let exe = std::env::current_exe().expect("test binary");
        let child = Command::new(exe)
            .args(["child_process_body", "--exact", "--test-threads", "1"])
            .env("PHOSPHOR_JOURNAL_CHILD", mode)
            .env("XDG_STATE_HOME", &home)
            .env("PHOSPHOR_JOURNAL_ROOT", &root)
            .env("PHOSPHOR_JOURNAL_FILE", &file)
            .env("PHOSPHOR_JOURNAL_READY", &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the child");

        let (journal, origin) = child_paths(&home, &root, &file);
        Self {
            _dir: dir,
            child,
            ready,
            journal,
            origin,
        }
    }

    fn wait_for_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if self.ready.exists() {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("the child exited before it was ready: {status}");
            }
            sleep(Duration::from_millis(10));
        }
        panic!("the child never became ready");
    }

    fn kill_9(&mut self) -> std::process::ExitStatus {
        self.child.kill().expect("SIGKILL");
        self.child.wait().expect("reap")
    }

    fn child_waits(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
    }

    fn assert_history(&self) {
        let (log, recovery) = UndoLog::open(&self.journal).expect("reopen");
        assert!(recovery.is_clean(), "a clean exit leaves a clean log");
        assert_session(log.state(), &self.origin);
    }
}

impl Drop for ChildSession {
    fn drop(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
    }
}

/// Both sides compute the journal's location the same way — through Q1's
/// keying, from the same state home and root — so the test is exercising the
/// real path resolution rather than agreeing on a filename.
fn child_paths(home: &Path, root: &Path, file: &Path) -> (PathBuf, String) {
    let dir = journal::workspace_dir_in(home, root).expect("workspace dir");
    let canonical = fs::canonicalize(file).expect("canonical file");
    let path = journal::undo_path(&dir, &canonical);
    (path, canonical.to_string_lossy().to_string())
}

/// The child, when `PHOSPHOR_JOURNAL_CHILD` names a mode; a no-op otherwise.
///
/// Re-executing the test binary is what lets a `SIGKILL` test exist at all
/// without a second crate or a shell script — and a mode that is a plain test
/// costs one no-op per suite run.
#[test]
fn child_process_body() {
    let Ok(mode) = std::env::var("PHOSPHOR_JOURNAL_CHILD") else {
        return;
    };

    let home = PathBuf::from(std::env::var_os("XDG_STATE_HOME").expect("state home"));
    let root = PathBuf::from(std::env::var_os("PHOSPHOR_JOURNAL_ROOT").expect("root"));
    let file = PathBuf::from(std::env::var_os("PHOSPHOR_JOURNAL_FILE").expect("file"));
    let ready = PathBuf::from(std::env::var_os("PHOSPHOR_JOURNAL_READY").expect("ready"));

    // The child resolves its own state directory from `XDG_STATE_HOME`, which
    // is what `state_home()` reads — so the environment variable path is under
    // test here, in the one place a test may set one.
    let dir = journal::workspace_dir(&root).expect("workspace dir");
    assert_eq!(dir, journal::workspace_dir_in(&home, &root).expect("same"));

    let canonical = fs::canonicalize(&file).expect("canonical");
    let path = journal::undo_path(&dir, &canonical);
    let (mut log, _) = UndoLog::open(&path).expect("open");
    for record in session(&canonical.to_string_lossy()) {
        log.append(record).expect("append");
    }
    if mode == "exit" {
        // The clean half syncs, because that is what a quiet point does. The
        // two kill modes deliberately do not: what they prove is that a plain
        // `write_all` is already enough to survive the process dying, which is
        // the claim the append path rests on.
        log.sync().expect("fsync");
    }

    if mode == "park" {
        // The half-written record, put there deliberately: see the header.
        let mut file = OpenOptions::new().append(true).open(&path).expect("append");
        file.write_all(&[0xff, 0xff, 0x00, 0x00, 0x7f])
            .expect("tail");
    }

    mark_ready(&ready);

    match mode.as_str() {
        // Nothing more to do; the parent kills this process where it stands.
        "park" => park(),
        // Append until killed. Cursor records only, so whatever survives past
        // the committed session is still a valid history.
        "flood" => {
            let deadline = Instant::now() + Duration::from_secs(60);
            while Instant::now() < deadline {
                log.append(undo::Record::Cursor { to: 2 }).expect("append");
                log.append(undo::Record::Cursor { to: 3 }).expect("append");
                sleep(Duration::from_millis(1));
            }
        }
        // A clean exit: the restart half.
        _ => {}
    }
}

fn mark_ready(ready: &Path) {
    let tmp = ready.with_extension("tmp");
    fs::write(&tmp, b"ready").expect("ready marker");
    fs::rename(&tmp, ready).expect("publish the marker");
}

/// Waits to be killed, with a ceiling so a failed parent cannot leave this
/// process behind.
fn park() {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        sleep(Duration::from_millis(20));
    }
}
