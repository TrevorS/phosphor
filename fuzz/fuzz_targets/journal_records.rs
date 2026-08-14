//! `F1b` — the journal's payload half: the codec and the fold, past the CRC.
//!
//! # Why this is a second target rather than more of `journal_open`
//!
//! Because a coverage-guided fuzzer cannot solve a CRC-32. `journal_open` hands
//! the reader a whole file, which is exactly right for the framing contract —
//! and it means that after the first mutated byte in a payload, every frame
//! from there on fails `scan`'s checksum and the input never reaches
//! `Folded::decode` or `History::apply` at all. The deep half of the reader is
//! unreachable from raw file bytes by construction, and a target that cannot
//! reach the code it names is a corpus nobody will look at.
//!
//! So this target writes its frames through the **real writer** —
//! `Journal::append`, which computes the length and the checksum itself — and
//! lets the fuzzer choose only the payloads. No CRC is duplicated here: getting
//! one wrong would silently reproduce the problem this target exists to solve.
//!
//! # Input
//!
//! A run of `[len: u8][len bytes]`, up to [`MAX_RECORDS`]. A one-byte length is
//! the point: a fuzzer flips it and the record boundary moves, which is the
//! mutation that matters, and it makes a seed file something a person can read
//! with `xxd` (`seeds/journal_records/` holds the payloads of real journals in
//! exactly this framing).
//!
//! # The laws
//!
//! 1. **Framing round-trips.** What `append` wrote, `Journal::open` reads back
//!    byte for byte, in order, with nothing discarded. This is the claim the
//!    CRC exists to support and it is asserted here rather than assumed,
//!    because everything below depends on the payload the decoder sees being
//!    the payload the writer wrote.
//! 2. **The codec normalises rather than corrupts.** `History::decode` answers
//!    `Ok` or a `DecodeError` for any bytes, never a panic — and where the
//!    re-encoding of an `Ok` differs from the bytes it came from, it must still
//!    mean the same record and be a fixed point. See *"The stronger law is
//!    false"* below for why this is not the obvious statement.
//! 3. **The fold answers.** `History::apply` is where a decoded record meets
//!    node indices — `walk_to` follows parent links, `get` bounds-checks an id
//!    against `nodes.len()` — and its input is a record that survived a CRC,
//!    which after a real crash means a record some *other* version of this
//!    program wrote. `FoldError` is the right answer to all of it; a panic is
//!    an editor that will not open a file.
//! 4. **A folded log reopens clean**, exactly as in `journal_open`.
//!
//! # The stronger law is false, and this target is how we know
//!
//! The obvious law — *an `Ok` re-encodes to the bytes it came from* — is what
//! `properties.rs`'s `arbitrary_bytes_decode_or_refuse` asserts, in those words:
//! *"an `Ok` is a record whose own encoding is those same bytes"*. **It is not
//! true**, and this target found the counterexample in under two minutes:
//!
//! ```text
//! payload [5, 17, 188, 0]  decodes to  Record::Redo { node: 17, child: 60 }
//! which encodes to  [5, 17, 60]
//! ```
//!
//! `Encoder::u64` is minimal LEB128 and `Decoder::u64` accepts *any* LEB128:
//! `0xbc 0x00` is `60` written with a redundant continuation byte, so two byte
//! strings decode to one record and the codec is not injective. It reaches
//! every field on the wire, because `usize`, `seq_len` and `option_u64` all
//! route through `u64`.
//!
//! Nothing this build writes is affected — the encoder never emits a
//! non-minimal varint — so this is a false *stated law* and a format
//! malleability rather than data loss. The fix belongs in `Decoder::u64`
//! (refuse a non-minimal encoding, the way a checksummed format has to if its
//! bytes are to mean one thing), and it is `spine`'s to make in
//! `phosphor-core`; it is filed as a `CONTRACT` from this run, with the
//! four-byte reproducer above as the regression test.
//!
//! Until then the assertion below is the *true* law, split so it keeps its
//! teeth: a re-encoding that differs is allowed only if it is a **normalisation**
//! — it decodes back to the same record, and encoding it again changes nothing.
//! A re-encoding that means something else, or that fails to decode at all, is
//! still a failure here.

#![no_main]

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use phosphor_core::journal::{Encoder, Folded, Journal, Stream, UndoLog, undo};

/// Frames per input.
///
/// libFuzzer's default `max_len` is 4096, so an input of nothing but zero-length
/// records would otherwise write four thousand frames and spend the whole exec
/// in `write_all`. The interesting failures are between the first few records,
/// not the four-thousandth.
const MAX_RECORDS: usize = 64;

/// One file, reused for the life of the process — see `journal_open` for why.
fn scratch() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("phosphor-fuzz-records-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("the fuzzer needs a scratch directory");
        dir.join("undo.journal")
    })
}

/// Splits the input into payloads on its one-byte length prefixes.
///
/// A trailing length that promises more than is left takes what is left, rather
/// than dropping it: that is the byte pattern a truncated file has, and
/// discarding it would make the last record of every input uninteresting.
fn payloads(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() && out.len() < MAX_RECORDS {
        let want = usize::from(data[at]);
        at += 1;
        let end = (at + want).min(data.len());
        out.push(&data[at..end]);
        at = end;
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let path = scratch();
    let payloads = payloads(data);

    // A fresh file each iteration: `Journal::open` writes the header into an
    // empty one, so this is the real create path rather than a hand-built
    // header.
    let _ = fs::remove_file(path);
    {
        let (mut journal, read_back, recovery) =
            Journal::open(path, Stream::UNDO).expect("a fresh journal opens");
        assert!(read_back.is_empty() && recovery.is_clean());
        for payload in &payloads {
            journal
                .append(payload)
                .expect("a payload under 256 bytes fits");
        }
        journal.sync().expect("fsync");
    }

    // Law 1 — framing round-trips.
    let (_, read_back, recovery) = Journal::open(path, Stream::UNDO).expect("the journal reopens");
    assert!(
        recovery.is_clean(),
        "a journal this process just wrote came back with {} discarded bytes",
        recovery.discarded_bytes
    );
    assert_eq!(
        read_back.len(),
        payloads.len(),
        "wrote {} frames, read back {}",
        payloads.len(),
        read_back.len()
    );
    for (wrote, read) in payloads.iter().zip(&read_back) {
        assert_eq!(
            *wrote,
            read.as_slice(),
            "a frame did not survive its own CRC"
        );
    }

    // Law 2 — the codec normalises rather than corrupts.
    for payload in &read_back {
        let Ok(record) = undo::History::decode(payload) else {
            continue;
        };
        let mut encoder = Encoder::new();
        undo::History::encode(&record, &mut encoder);
        let spelled = encoder.finish();
        if &spelled == payload {
            continue;
        }
        // The re-encoding differs — allowed only as a normalisation. See the
        // header: a non-minimal LEB128 varint is the one known way to get here.
        let again = undo::History::decode(&spelled).unwrap_or_else(|error| {
            panic!("{record:?} encodes to bytes the decoder refuses: {error}")
        });
        assert!(
            again == record,
            "{payload:?} decoded to {record:?}, which re-encodes to bytes \
             meaning {again:?} — a normalisation may not change the record"
        );
        let mut encoder = Encoder::new();
        undo::History::encode(&again, &mut encoder);
        assert_eq!(
            encoder.finish(),
            spelled,
            "encoding {record:?} is not a fixed point"
        );
    }

    // Laws 3 and 4 — the fold answers, and a folded log reopens clean.
    let Ok((log, _)) = UndoLog::open(path) else {
        return;
    };
    let state = log.state().clone();
    drop(log);
    let (again, second) = UndoLog::open(path).expect("a folded journal reopens");
    assert!(
        second.is_clean(),
        "reopening discarded {} bytes from a journal nothing had torn",
        second.discarded_bytes
    );
    assert!(
        again.state() == &state,
        "two opens of the same file folded to different states"
    );
});
