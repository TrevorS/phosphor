//! `F1a` — the journal reader, over arbitrary file bytes.
//!
//! `T030`'s acceptance criterion is that a `kill -9` mid-append costs the tail
//! and nothing else. `journal.rs`'s own header states it as *"reads until the
//! first frame that does not check out … truncates the file to the last good
//! boundary"*. The input to that claim is **a file**, and a file is bytes
//! nobody in this repo chose: a crash can land inside a length field, inside a
//! CRC, inside a multi-byte character in a payload, and some filesystems leave
//! a run of zeros rather than a short file.
//!
//! # What this adds over the property test
//!
//! `crates/phosphor-core/tests/properties.rs`'s `any_truncation_recovers_a_prefix`
//! already covers the shape a crash produces: *write a real log, cut it
//! somewhere, append garbage*. It is the stronger statement about that shape,
//! because it knows what the writer wrote and so can assert the survivors are a
//! **prefix**.
//!
//! What it cannot generate is a file that was never a log. Its header is always
//! `read_header`-valid, its frames up to the cut always check out, and its cut
//! is always in the frames region — the property says so in its own comment
//! (*"the header is a separate contract"*). This target hands `Log::open` the
//! whole file, from byte zero, with no structure at all. The seed corpus
//! (`seeds/journal_open/`) is real journals written by the real writer, so the
//! fuzzer starts from valid files and mutates *toward* the garbage rather than
//! spending its first hour rediscovering `PHOSJRNL`.
//!
//! # The law
//!
//! Three claims, and each is the reader's own sentence rather than a restatement
//! of its code:
//!
//! 1. **It answers.** Never a panic, for any bytes. `Log::open` is on the path
//!    from "open a file" to "draw a buffer", so a panic here is an editor that
//!    will not start until the user deletes a file they cannot see.
//! 2. **Recovery lands on a boundary.** If the open succeeded, the file left on
//!    disk is a *clean* journal: opening it a second time succeeds, discards
//!    nothing, and folds to the same state. That is what "truncates to the last
//!    good boundary" means, made mechanical — an off-by-one that left half a
//!    frame behind would pass the first open and fail the second.
//! 3. **The bytes are accounted for.** What is left on disk plus what recovery
//!    says it discarded is exactly what it was handed. This is the half that
//!    says the next append cannot land after garbage.
//!
//! # What is deliberately not asserted
//!
//! **That the recovered state survives compaction.** It does not, in general,
//! and that is known rather than suspected: `properties.rs`'s
//! `a_hand_written_redo_on_the_cursor_path_does_not_survive_compaction` measured
//! the boundary — the fold accepts strictly more than any writer emits, and
//! `History::snapshot` round-trips only what a writer emits. A file is exactly
//! where a non-writer-emitted record can come from, so asserting it here would
//! rediscover a documented boundary on the first interesting input and report a
//! finding somebody already wrote down.

#![no_main]

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use phosphor_core::journal::UndoLog;

/// One file, reused for the life of the process.
///
/// A `TempDir` per iteration is two syscalls and an `unlink` per input, which at
/// libFuzzer's exec rate is most of the run. libFuzzer is in-process and single
/// threaded by default, so one path is safe; the pid keeps two concurrent
/// `cargo fuzz run` invocations apart.
fn scratch() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("phosphor-fuzz-journal-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("the fuzzer needs a scratch directory");
        dir.join("undo.journal")
    })
}

fuzz_target!(|data: &[u8]| {
    let path = scratch();

    // `create(true).truncate(true)` rather than `remove_file` + create: an
    // empty file is a *valid* input (`Journal::open` writes a header into one),
    // and a missing file is a different input again. Both are reachable — the
    // first when `data` is empty, the second never, which is the one case the
    // dedicated unit tests already cover.
    {
        let mut file = fs::File::create(path).expect("the scratch file is writable");
        file.write_all(data).expect("the scratch file is writable");
    }
    let handed = data.len() as u64;

    // Claim 1: it answers. An `Err` is a correct answer — a file that is not a
    // journal, is a journal of the wrong stream, or holds a record this schema
    // cannot decode are all `Error` cases the reader declares.
    let Ok((log, recovery)) = UndoLog::open(path) else {
        return;
    };
    let state = log.state().clone();
    let left = fs::metadata(path).expect("the file exists").len();

    // Claim 3: the bytes are accounted for. Only when the file had bytes —
    // `Journal::open` *writes* a header into an empty file, so `left` there is
    // 16 and `handed` is 0, which is the reader creating rather than recovering.
    if handed > 0 {
        assert_eq!(
            left + recovery.discarded_bytes,
            handed,
            "recovery neither kept nor discarded {} bytes of a {handed}-byte file",
            handed as i64 - (left + recovery.discarded_bytes) as i64
        );
    }
    assert_eq!(
        log.journal().byte_len(),
        left,
        "the journal's byte count disagrees with the file it just truncated"
    );
    assert_eq!(
        log.journal().records(),
        recovery.records,
        "the journal's record count disagrees with the recovery it just reported"
    );
    drop(log);

    // Claim 2: recovery lands on a boundary. Whatever came in, what is on disk
    // now is a journal that needs no recovery.
    let (again, second) = UndoLog::open(path).expect("a recovered journal reopens");
    assert!(
        second.is_clean(),
        "reopening a recovered journal discarded another {} bytes — \
         the first open did not truncate to a frame boundary",
        second.discarded_bytes
    );
    assert_eq!(
        second.records, recovery.records,
        "the reopen read back a different number of records"
    );
    assert!(
        again.state() == &state,
        "the reopen folded to a different state than the recovery did"
    );
});
