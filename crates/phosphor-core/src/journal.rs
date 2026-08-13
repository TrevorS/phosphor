//! The on-disk story — `T030`, and the format `T044` inherits rather than
//! negotiates with.
//!
//! Q1 and Q2 both end in the same sentence: *"append-only log with periodic
//! compaction"*, *"the two share one file format and one compaction path"*.
//! Undo persistence arrives two phases before seen-state does, so the format is
//! designed here, once, for both — and this module is named for the shape
//! (a journal) rather than for the first thing stored in it, because `T044`
//! importing `undo_log` to store seen-state would be the wrong name on the
//! import line forever.
//!
//! # The shape: a folded log
//!
//! One idea, applied twice:
//!
//! ```text
//!   append   record ──▶ [PHOSJRNL|fmt|stream][rec][rec][rec] …
//!   open     every record ──▶ fold ──▶ the state
//!   compact  the state ──▶ snapshot ──▶ a fresh log with the same fold
//! ```
//!
//! A [`Folded`] implementation supplies three things — its record type, how a
//! record moves the state, and the minimal record sequence that reproduces the
//! state. Everything else (framing, checksums, torn-tail recovery, atomic
//! compaction, the state directory, the codec) is here and is shared. The law
//! that makes compaction safe is one line, and it is testable generically:
//!
//! > folding a snapshot of a state produces that same state.
//!
//! # Why append-only, and what a crash costs
//!
//! The alternative is writing the whole state on exit, and it fails the only
//! test that matters: `kill -9` does not run exit code. An append-only log
//! loses at most the tail, and the tail is one record.
//!
//! Durability is deliberately two-tier, and the distinction is not academic:
//!
//! * [`Log::append`] does `write_all` and nothing else. The bytes are in the
//!   kernel's page cache, which **survives the process dying** — `kill -9`,
//!   a panic, a `SIGSEGV` — because the file is the kernel's now, not ours.
//!   This is the tier the `T030` acceptance test measures, and it costs no
//!   syscall beyond the write.
//! * [`Log::sync`] is `fsync`, and it is the only thing that survives the
//!   *machine* dying. It is not on the append path on purpose: an `fsync` per
//!   undo group is an `fsync` per `<esc>`, which is felt. Call it at a natural
//!   quiet point.
//!
//! What a crash can leave behind is a **half-written record at the tail** —
//! only at the tail, because appends are sequential. [`Log::open`] therefore
//! reads until the first frame that does not check out, reports what it
//! discarded in a [`Recovery`], and **truncates the file to the last good
//! boundary** so the next append does not land after garbage. A frame carries
//! its own length and a CRC-32 over both, so a short tail, a torn payload and
//! the run of zeros some filesystems leave after a crash are all the same
//! answer: stop here, keep everything before.
//!
//! Compaction is the other half of crash safety. [`Log::compact`] writes a
//! fresh file beside the live one, `fsync`s it, and `rename`s it over the top —
//! atomic on POSIX, so a crash during compaction leaves either the old complete
//! log or the new complete log and never a mixture.
//!
//! # The codec, and the two dependencies this module does not have
//!
//! `SPIKES.md:307` recommends `postcard` for exactly this log, and
//! `SPIKES.md:304` recommends `etcetera` for the XDG paths. Neither is in the
//! root manifest's `[workspace.dependencies]`, and that manifest is `spine`'s;
//! this crate's own says *"Deliberately dependency-free at the floor"*, which is
//! load-bearing rather than tidy. `phosphor-ui` takes `phosphor-core` and
//! `crates/phosphor-ui/Cargo.toml:16-18` records that `phosphor-core` is the
//! **only** `phosphor-*` dependency a widget crate may have, so this crate's
//! dependency line is the widget crate's too — which is why
//! `scripts/lint-no-store-mutation.sh:155-164` fails CI on a `ratatui` or
//! `steel` dependency here. So the codec is ~120 lines of LEB128 and
//! length-prefixed UTF-8, written here, and [`state_home`] is the one function
//! to replace if `etcetera` is ever added.
//!
//! The other candidate was [`crate::value::Value`], which is already a wire
//! model with a round-trip test. Rejected: it is the *door* vocabulary, its
//! records carry their field names as text, and `value.rs` says adding a case
//! is a contract change — which would make every schema change to the doors a
//! change to the disk format. The disk format wants to move for its own
//! reasons, at its own version, which is exactly what Q2 says about the undo
//! format being ours rather than upstream's.
//!
//! # What `T044` gets, and what it supplies
//!
//! Gets: [`Encoder`] / [`Decoder`] and their error type, the framing and CRC,
//! [`Recovery`] and the torn-tail truncation, [`Log`] with append / compact /
//! `compact_if_needed` and its doubling policy, the atomic rewrite,
//! [`state_home`] / [`workspace_dir`] / [`workspace_key`] with Q1's
//! canonical-root keying and its collision marker, and [`Stream::SEEN`], which
//! is reserved here so the two streams cannot collide on disk.
//!
//! Supplies: an `impl Folded for` its own state — a record enum, `apply`, and
//! `snapshot`. That is the whole of `T044`'s persistence surface; the rest of
//! that task is the store's own shape.
//!
//! # The seam with `phosphor-buffer`
//!
//! Q2 splits undo in two and this is the second half. The tree is
//! `phosphor-buffer`'s, and `crates/phosphor-buffer/src/undo.rs:103-153` states
//! what has to be written and read back. [`undo::History`] is that, as plain
//! data: [`undo::Node`] is a field-for-field mirror of that module's `Node`
//! (`undo.rs:558-569`), and [`undo::History::into_parts`] hands back exactly
//! the `(nodes, current, saved)` triple its `UndoTree::from_parts` takes
//! (`undo.rs:989-993`).
//!
//! **The conversion is a field copy and it does not live here**, because
//! `phosphor-core` does not depend on `phosphor-buffer`. That crate's manifest
//! carries the vendored fork, `ropey` and `tree-sitter`, so taking it here would
//! put all three in the floor crate's graph and therefore in `phosphor-ui`'s —
//! the letter of the lint above would pass and its point would not. The
//! conversion belongs to whoever holds both crates, which is the binary.
//!
//! Two things `undo.rs`'s header asks for, and where they are:
//!
//! * **An open group is not persisted.** Nothing here can write one:
//!   [`undo::Record::Node`] is a committed node, and a group that has not been
//!   committed has no node. Half a keystroke sequence is not a state to be
//!   restored into.
//! * **Compaction must not forget the root's text.** `undo.rs:145-153` is
//!   explicit that dropping the oldest nodes moves the root and the compacted
//!   file has to carry the text at the new root. [`Log::compact`] here **drops
//!   no nodes** — it collapses the cursor and save churn, which is where the
//!   records actually accumulate, and every node survives. [`undo::Record::Base`]
//!   exists for the day a truncating compaction does land, and
//!   [`undo::History::base`] is preserved across every rewrite, so the record
//!   that has to be there is already in the format rather than being a format
//!   change later.
//!
//! Owned by `store` (TEAM.md), alongside `store`, `region`, `anchor` and `seen`.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

/// Writes the primitives a record is made of.
///
/// Schema-driven, not self-describing: a record's tag says what follows, so
/// nothing on disk repeats a field name. The reader is [`Decoder`] and the two
/// are written to be read side by side — every method here has exactly one
/// counterpart there.
#[derive(Debug, Default, Clone)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    /// An empty encoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The bytes written so far.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// How many bytes have been written.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    /// LEB128. Small numbers cost one byte, which is what an undo log is made
    /// of — offsets, ids and lengths are nearly all under 128.
    pub fn u64(&mut self, mut value: u64) {
        loop {
            let byte = u8::try_from(value & 0x7f).unwrap_or(0);
            value >>= 7;
            if value == 0 {
                self.bytes.push(byte);
                return;
            }
            self.bytes.push(byte | 0x80);
        }
    }

    /// A `usize`, as a [`Encoder::u64`]. Char offsets are `usize` in the engine
    /// and `u64` on disk; a 32-bit host reading a file written by a 64-bit one
    /// fails loudly in [`Decoder::usize`] rather than silently truncating.
    pub fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    /// One byte, `0` or `1`.
    pub fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    /// Length-prefixed UTF-8.
    pub fn str(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    /// A present-flag then the value, so [`None`] costs one byte.
    pub fn option_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.u64(value);
            }
            None => self.bool(false),
        }
    }

    /// How many items follow. Same encoding as a [`Encoder::usize`]; a separate
    /// name because [`Decoder::seq_len`] bounds it against what is left.
    pub fn seq_len(&mut self, len: usize) {
        self.usize(len);
    }
}

/// Reads what [`Encoder`] wrote.
#[derive(Debug)]
pub struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Decoder<'a> {
    /// A decoder over one record's payload.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// How many bytes are left unread.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(DecodeError::UnexpectedEnd)?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    /// One LEB128 integer.
    ///
    /// # Errors
    ///
    /// [`DecodeError::UnexpectedEnd`] on a truncated varint,
    /// [`DecodeError::Overflow`] on one that does not fit in a `u64` — which a
    /// corrupt byte can produce and an allocation must never be sized from.
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        let mut value: u64 = 0;
        for shift in (0..64).step_by(7) {
            let byte = *self.take(1)?.first().ok_or(DecodeError::UnexpectedEnd)?;
            let part = u64::from(byte & 0x7f);
            value |= part
                .checked_shl(shift)
                .filter(|shifted| shifted >> shift == part)
                .ok_or(DecodeError::Overflow)?;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(DecodeError::Overflow)
    }

    /// One integer, as a `usize`.
    ///
    /// # Errors
    ///
    /// Whatever [`Decoder::u64`] returns, plus [`DecodeError::Overflow`] if the
    /// value does not fit this host's `usize`.
    pub fn usize(&mut self) -> Result<usize, DecodeError> {
        usize::try_from(self.u64()?).map_err(|_| DecodeError::Overflow)
    }

    /// One boolean.
    ///
    /// # Errors
    ///
    /// [`DecodeError::UnexpectedEnd`], or [`DecodeError::BadBool`] for any byte
    /// that is not `0` or `1` — a cheap corruption detector in the middle of a
    /// record whose CRC already passed is worth having, because it says which
    /// field went wrong.
    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match *self.take(1)?.first().ok_or(DecodeError::UnexpectedEnd)? {
            0 => Ok(false),
            1 => Ok(true),
            byte => Err(DecodeError::BadBool { byte }),
        }
    }

    /// One length-prefixed UTF-8 string.
    ///
    /// # Errors
    ///
    /// [`DecodeError::UnexpectedEnd`] if the length runs past the record,
    /// [`DecodeError::Utf8`] if the bytes are not UTF-8.
    pub fn str(&mut self) -> Result<String, DecodeError> {
        let len = self.usize()?;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError::Utf8)
    }

    /// One optional integer.
    ///
    /// # Errors
    ///
    /// Whatever [`Decoder::bool`] and [`Decoder::u64`] return.
    pub fn option_u64(&mut self) -> Result<Option<u64>, DecodeError> {
        if self.bool()? {
            Ok(Some(self.u64()?))
        } else {
            Ok(None)
        }
    }

    /// A sequence length, bounded by what is left.
    ///
    /// Every item costs at least one byte, so a length larger than the bytes
    /// remaining is corruption, and this is where that is caught — before a
    /// `Vec::with_capacity` sized from it.
    ///
    /// # Errors
    ///
    /// [`DecodeError::TooLong`] when the length exceeds the bytes remaining.
    pub fn seq_len(&mut self) -> Result<usize, DecodeError> {
        let len = self.usize()?;
        if len > self.remaining() {
            return Err(DecodeError::TooLong {
                want: len,
                remaining: self.remaining(),
            });
        }
        Ok(len)
    }

    /// Asserts the record is fully consumed.
    ///
    /// Trailing bytes mean the writer and the reader disagree about the shape
    /// of this record, which is a schema bug rather than a corruption, and it
    /// is worth failing on rather than ignoring.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Trailing`] with the number of bytes left over.
    pub fn finish(self) -> Result<(), DecodeError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(DecodeError::Trailing {
                extra: self.remaining(),
            })
        }
    }
}

/// Why a record's payload would not decode.
///
/// Deliberately number-only: it is carried inside [`Error`], and an error type
/// that is cheap to return is one nobody is tempted to `unwrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The record ended in the middle of a value.
    UnexpectedEnd,
    /// A varint does not fit a `u64`, or a value does not fit this host's
    /// `usize`.
    Overflow,
    /// A byte that should have been `0` or `1` was not.
    BadBool {
        /// What was there.
        byte: u8,
    },
    /// A string's bytes are not UTF-8.
    Utf8,
    /// A sequence claims more items than the record has bytes.
    TooLong {
        /// The claimed length.
        want: usize,
        /// Bytes left in the record.
        remaining: usize,
    },
    /// The record's leading tag is not one this schema version knows.
    UnknownRecord {
        /// The tag that was there.
        tag: u64,
    },
    /// The record decoded with bytes left over.
    Trailing {
        /// How many.
        extra: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => f.write_str("record ended mid-value"),
            Self::Overflow => f.write_str("integer does not fit"),
            Self::BadBool { byte } => write!(f, "expected 0 or 1, found {byte}"),
            Self::Utf8 => f.write_str("string is not utf-8"),
            Self::TooLong { want, remaining } => {
                write!(
                    f,
                    "sequence of {want} in a record with {remaining} bytes left"
                )
            }
            Self::UnknownRecord { tag } => write!(f, "unknown record tag {tag}"),
            Self::Trailing { extra } => write!(f, "{extra} bytes left after the record"),
        }
    }
}

impl std::error::Error for DecodeError {}

// ---------------------------------------------------------------------------
// Framing
// ---------------------------------------------------------------------------

/// What every phosphor journal starts with.
const MAGIC: [u8; 8] = *b"PHOSJRNL";

/// The framing and codec version — bumped when a frame's shape changes, never
/// when a record's does. That is [`Stream::version`]'s job.
const FORMAT_VERSION: u16 = 1;

/// Magic, format version, stream kind, stream version, two reserved bytes.
const HEADER_LEN: u64 = 16;

/// A frame's fixed prefix: a `u32` length and a `u32` CRC.
const FRAME_PREFIX: usize = 8;

/// Which schema a journal holds, and at which version.
///
/// Written into the header so opening a seen-state log as an undo log is an
/// error at the first byte rather than a decode failure fifty records in — the
/// two live in the same directory under Q1's keying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stream {
    /// Which schema. Stable forever; a new kind is a new number.
    pub kind: u16,
    /// That schema's own version. A reader refuses a version it does not know.
    pub version: u16,
}

impl Stream {
    /// `T030`'s stream — a buffer's undo history.
    pub const UNDO: Self = Self {
        kind: 1,
        version: 1,
    };

    /// `T044`'s stream, reserved here so the two cannot collide on disk.
    pub const SEEN: Self = Self {
        kind: 2,
        version: 1,
    };
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind {
            1 => "undo",
            2 => "seen",
            _ => "unknown",
        };
        write!(f, "{name}/{}", self.version)
    }
}

/// What a crash cost, as read back by [`Log::open`].
///
/// Reported rather than logged: the caller decides whether losing the tail of a
/// session is worth telling the user about, and `phosphor-core` has no opinion
/// about surfaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recovery {
    /// Bytes after the last good frame, discarded and truncated away.
    pub discarded_bytes: u64,
    /// Frames read back intact.
    pub records: u64,
}

impl Recovery {
    /// Whether the file ended on a frame boundary — no crash, or a crash
    /// between records.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.discarded_bytes == 0
    }
}

/// CRC-32, the IEEE polynomial, reflected — the same one `gzip` and `png` use.
///
/// Built at compile time so the table costs nothing at runtime, and hand-rolled
/// for the reason in this module's header: nothing may be added to this crate's
/// dependency line.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 0 {
                value >> 1
            } else {
                0xedb8_8320 ^ (value >> 1)
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
};

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for part in parts {
        for byte in *part {
            let index = usize::from((crc as u8) ^ *byte);
            crc = CRC_TABLE[index] ^ (crc >> 8);
        }
    }
    !crc
}

/// The file: a header, then frames, and nothing else.
///
/// Payloads are opaque here. [`Log`] is what pairs one with a [`Folded`] state;
/// this type exists separately because framing and folding fail for different
/// reasons and testing them apart is how you find out which.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    file: File,
    stream: Stream,
    records: u64,
    bytes: u64,
}

impl Journal {
    /// Opens or creates a journal, reads every intact record, and truncates a
    /// torn tail.
    ///
    /// The parent directory is created if it is missing, so a caller does not
    /// have to know whether this is the first run.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] for anything the filesystem refuses, [`Error::NotAJournal`]
    /// for a file that is not one, and [`Error::WrongStream`] /
    /// [`Error::WrongFormat`] for one that is a journal of the wrong kind or a
    /// version this build does not know.
    pub fn open(path: &Path, stream: Stream) -> Result<(Self, Vec<Vec<u8>>, Recovery), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|source| Error::io(path, source))?;

        let len = file
            .metadata()
            .map_err(|source| Error::io(path, source))?
            .len();

        if len == 0 {
            file.write_all(&header_bytes(stream))
                .map_err(|source| Error::io(path, source))?;
            return Ok((
                Self {
                    path: path.to_path_buf(),
                    file,
                    stream,
                    records: 0,
                    bytes: HEADER_LEN,
                },
                Vec::new(),
                Recovery::default(),
            ));
        }

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|source| Error::io(path, source))?;
        read_header(path, &data, stream)?;

        let (records, good_end) = scan(&data);
        let discarded = data.len().saturating_sub(good_end) as u64;
        if discarded > 0 {
            file.set_len(good_end as u64)
                .map_err(|source| Error::io(path, source))?;
        }
        file.seek(SeekFrom::Start(good_end as u64))
            .map_err(|source| Error::io(path, source))?;

        let recovery = Recovery {
            discarded_bytes: discarded,
            records: records.len() as u64,
        };
        Ok((
            Self {
                path: path.to_path_buf(),
                file,
                stream,
                records: recovery.records,
                bytes: good_end as u64,
            },
            records,
            recovery,
        ))
    }

    /// Appends one record.
    ///
    /// One `write_all` of one frame, and no `fsync` — see the durability
    /// paragraph in this module's header for what that does and does not
    /// survive.
    ///
    /// # Errors
    ///
    /// [`Error::RecordTooLarge`] for a payload past `u32::MAX`, [`Error::Io`]
    /// for a failed write. A failed write leaves the file truncated to the last
    /// good boundary by the *next* [`Journal::open`], so a partial frame here is
    /// not a corruption, it is a tail.
    pub fn append(&mut self, payload: &[u8]) -> Result<(), Error> {
        let len = u32::try_from(payload.len())
            .map_err(|_| Error::RecordTooLarge { len: payload.len() })?;
        let len_bytes = len.to_le_bytes();
        let crc = crc32(&[&len_bytes, payload]);

        let mut frame = Vec::with_capacity(FRAME_PREFIX + payload.len());
        frame.extend_from_slice(&len_bytes);
        frame.extend_from_slice(&crc.to_le_bytes());
        frame.extend_from_slice(payload);

        self.file
            .write_all(&frame)
            .map_err(|source| Error::io(&self.path, source))?;
        self.records += 1;
        self.bytes += frame.len() as u64;
        Ok(())
    }

    /// `fsync`. The only thing that survives the machine dying.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the flush fails.
    pub fn sync(&self) -> Result<(), Error> {
        self.file
            .sync_all()
            .map_err(|source| Error::io(&self.path, source))
    }

    /// Replaces the file's contents with `payloads`, atomically.
    ///
    /// Writes a sibling, `fsync`s it, then `rename`s over this path. A crash
    /// leaves either the whole old file or the whole new one; `rename` on POSIX
    /// has no third outcome.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] for any step, [`Error::RecordTooLarge`] for a payload past
    /// `u32::MAX`.
    pub fn rewrite(&mut self, payloads: &[Vec<u8>]) -> Result<(), Error> {
        let name = self
            .path
            .file_name()
            .map(|name| {
                let mut name = name.to_os_string();
                name.push(".compacting");
                name
            })
            .ok_or_else(|| Error::io(&self.path, io::Error::from(io::ErrorKind::InvalidInput)))?;
        let tmp = self.path.with_file_name(name);

        {
            let mut fresh = File::create(&tmp).map_err(|source| Error::io(&tmp, source))?;
            fresh
                .write_all(&header_bytes(self.stream))
                .map_err(|source| Error::io(&tmp, source))?;
            let mut bytes = HEADER_LEN;
            for payload in payloads {
                let len = u32::try_from(payload.len())
                    .map_err(|_| Error::RecordTooLarge { len: payload.len() })?;
                let len_bytes = len.to_le_bytes();
                let crc = crc32(&[&len_bytes, payload]);
                fresh
                    .write_all(&len_bytes)
                    .and_then(|()| fresh.write_all(&crc.to_le_bytes()))
                    .and_then(|()| fresh.write_all(payload))
                    .map_err(|source| Error::io(&tmp, source))?;
                bytes += (FRAME_PREFIX + payload.len()) as u64;
            }
            fresh.sync_all().map_err(|source| Error::io(&tmp, source))?;
            self.bytes = bytes;
        }

        fs::rename(&tmp, &self.path).map_err(|source| Error::io(&self.path, source))?;

        // The rename is durable once the directory entry is, and only an fsync
        // of the directory guarantees that. Best-effort: a platform that
        // refuses to open a directory for reading still gets the atomicity,
        // which is the property this method is for.
        if let Some(parent) = self.path.parent()
            && let Ok(dir) = File::open(parent)
        {
            drop(dir.sync_all());
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|source| Error::io(&self.path, source))?;
        file.seek(SeekFrom::End(0))
            .map_err(|source| Error::io(&self.path, source))?;
        self.file = file;
        self.records = payloads.len() as u64;
        Ok(())
    }

    /// How many records the file holds.
    #[must_use]
    pub fn records(&self) -> u64 {
        self.records
    }

    /// How many bytes it occupies, header included.
    #[must_use]
    pub fn byte_len(&self) -> u64 {
        self.bytes
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn header_bytes(stream: Stream) -> [u8; HEADER_LEN as usize] {
    let mut header = [0u8; HEADER_LEN as usize];
    header[..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[10..12].copy_from_slice(&stream.kind.to_le_bytes());
    header[12..14].copy_from_slice(&stream.version.to_le_bytes());
    header
}

fn read_header(path: &Path, data: &[u8], want: Stream) -> Result<(), Error> {
    if data.len() < HEADER_LEN as usize || data[..8] != MAGIC {
        return Err(Error::NotAJournal {
            path: path.to_path_buf(),
        });
    }
    let format = u16::from_le_bytes([data[8], data[9]]);
    if format != FORMAT_VERSION {
        return Err(Error::WrongFormat {
            path: path.to_path_buf(),
            found: format,
            expected: FORMAT_VERSION,
        });
    }
    let found = Stream {
        kind: u16::from_le_bytes([data[10], data[11]]),
        version: u16::from_le_bytes([data[12], data[13]]),
    };
    if found != want {
        return Err(Error::WrongStream {
            path: path.to_path_buf(),
            found,
            expected: want,
        });
    }
    Ok(())
}

/// Reads frames until one does not check out. Returns the payloads and the
/// offset of the first byte that is not part of an intact frame.
fn scan(data: &[u8]) -> (Vec<Vec<u8>>, usize) {
    let mut records = Vec::new();
    let mut at = HEADER_LEN as usize;
    while at < data.len() {
        if data.len() - at < FRAME_PREFIX {
            break;
        }
        let len_bytes = [data[at], data[at + 1], data[at + 2], data[at + 3]];
        let crc = u32::from_le_bytes([data[at + 4], data[at + 5], data[at + 6], data[at + 7]]);
        let len = u32::from_le_bytes(len_bytes) as usize;
        let start = at + FRAME_PREFIX;
        let Some(end) = start.checked_add(len).filter(|end| *end <= data.len()) else {
            break;
        };
        let payload = &data[start..end];
        if crc32(&[&len_bytes, payload]) != crc {
            break;
        }
        records.push(payload.to_vec());
        at = end;
    }
    (records, at)
}

// ---------------------------------------------------------------------------
// The folded log
// ---------------------------------------------------------------------------

/// A state that is the fold of a record stream.
///
/// The three methods are the whole contract between a feature and this module,
/// and one law binds them:
///
/// ```text
///   fold(snapshot(state)) == state
/// ```
///
/// [`Log::compact`] rewrites the file as `snapshot(state)`, so a `snapshot`
/// that loses something loses it permanently and silently. Test the law.
pub trait Folded: Default + fmt::Debug {
    /// One entry in the log.
    type Record: fmt::Debug;

    /// Which schema this is, and at what version.
    const STREAM: Stream;

    /// Writes a record's payload — the tag and its fields.
    fn encode(record: &Self::Record, out: &mut Encoder);

    /// Reads one back.
    ///
    /// # Errors
    ///
    /// A [`DecodeError`] for a payload this schema version cannot read.
    fn decode(bytes: &[u8]) -> Result<Self::Record, DecodeError>;

    /// Moves the state by one record, or refuses it.
    ///
    /// Called before the record reaches disk, so a rejected record is never
    /// written; and called again for every record on the way back in, which is
    /// what makes a log that was valid when written valid when read.
    ///
    /// # Errors
    ///
    /// A [`FoldError`] when the record does not fit the state it arrives at.
    fn apply(&mut self, record: Self::Record) -> Result<(), FoldError>;

    /// The shortest record sequence that folds back to this state.
    fn snapshot(&self) -> Vec<Self::Record>;
}

/// The compaction floor: below this many records, compaction is not worth a
/// rewrite.
const COMPACT_FLOOR: u64 = 256;

/// A [`Folded`] state and the journal it is folded from.
///
/// This is the type a feature holds. [`Log::append`] moves both halves and
/// keeps them in step; [`Log::state`] is the read side.
#[derive(Debug)]
pub struct Log<F: Folded> {
    journal: Journal,
    state: F,
    /// Records the last snapshot took — the denominator of the doubling
    /// policy in [`Log::should_compact`].
    compacted_at: u64,
}

impl<F: Folded> Log<F> {
    /// Opens the journal at `path`, folds every intact record into a state, and
    /// reports what a crash cost.
    ///
    /// # Errors
    ///
    /// Whatever [`Journal::open`] returns, plus [`Error::Decode`] and
    /// [`Error::Fold`] naming the record that would not read back. Both are
    /// hard failures rather than a silent truncation: a log that decodes for
    /// four hundred records and then does not is a schema bug, and continuing
    /// past it would restore a buffer into a state its own history disagrees
    /// with.
    pub fn open(path: &Path) -> Result<(Self, Recovery), Error> {
        let (journal, payloads, recovery) = Journal::open(path, F::STREAM)?;
        let mut state = F::default();
        for payload in &payloads {
            let record = F::decode(payload).map_err(|source| Error::Decode {
                path: path.to_path_buf(),
                source,
            })?;
            state.apply(record).map_err(|source| Error::Fold {
                path: path.to_path_buf(),
                source,
            })?;
        }
        let compacted_at = state.snapshot().len() as u64;
        Ok((
            Self {
                journal,
                state,
                compacted_at,
            },
            recovery,
        ))
    }

    /// The folded state.
    pub fn state(&self) -> &F {
        &self.state
    }

    /// The journal underneath, for its counters and its path.
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Applies a record to the state and appends it to the log.
    ///
    /// In that order: a record the state refuses never reaches disk. The
    /// reverse — a write that fails after the state moved — leaves memory ahead
    /// of the file, which the next [`Log::open`] corrects by reading the file.
    ///
    /// # Errors
    ///
    /// [`Error::Fold`] if the state refuses the record, or [`Error::Io`] if the
    /// write fails.
    pub fn append(&mut self, record: F::Record) -> Result<(), Error> {
        let mut encoder = Encoder::new();
        F::encode(&record, &mut encoder);
        let payload = encoder.finish();
        self.state.apply(record).map_err(|source| Error::Fold {
            path: self.journal.path.clone(),
            source,
        })?;
        self.journal.append(&payload)
    }

    /// `fsync` — see [`Journal::sync`].
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the flush fails.
    pub fn sync(&self) -> Result<(), Error> {
        self.journal.sync()
    }

    /// Whether the log has doubled since the last snapshot.
    ///
    /// The policy, in full: not below [`COMPACT_FLOOR`] records, and above it
    /// when the file holds twice what a snapshot would. That is self-tuning —
    /// a history with ten thousand nodes does not rewrite itself every
    /// keystroke — and it is checked rather than timed, because a timer in the
    /// store is a second clock to reason about.
    #[must_use]
    pub fn should_compact(&self) -> bool {
        let records = self.journal.records();
        records >= COMPACT_FLOOR && records >= 2 * self.compacted_at.max(1)
    }

    /// Rewrites the log as a snapshot of the state.
    ///
    /// This is `Action::History(CompactHistory)`'s implementation — the sweep
    /// Q1 and Q2 share (`crates/phosphor-core/src/action.rs:482-485`).
    ///
    /// # Errors
    ///
    /// Whatever [`Journal::rewrite`] returns.
    pub fn compact(&mut self) -> Result<(), Error> {
        let payloads: Vec<Vec<u8>> = self
            .state
            .snapshot()
            .iter()
            .map(|record| {
                let mut encoder = Encoder::new();
                F::encode(record, &mut encoder);
                encoder.finish()
            })
            .collect();
        self.journal.rewrite(&payloads)?;
        self.compacted_at = payloads.len() as u64;
        Ok(())
    }

    /// Compacts if [`Log::should_compact`] says so. Returns whether it did.
    ///
    /// # Errors
    ///
    /// Whatever [`Log::compact`] returns.
    pub fn compact_if_needed(&mut self) -> Result<bool, Error> {
        if self.should_compact() {
            self.compact()?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Why a record does not fit the state it arrived at.
///
/// The undo tree's four restore invariants (`phosphor-buffer/src/undo.rs:128-138`)
/// are checked here, on the way in *and* on the way back out, which is what
/// makes a truncated log safe rather than merely short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldError {
    /// A node arrived with an id that is not the next one. Ids are dense and in
    /// creation order; a gap means a record was lost from the middle, which an
    /// append-only log cannot do and a corrupt one can.
    OutOfOrder {
        /// The id the record carried.
        found: u64,
        /// The id it had to have.
        expected: u64,
    },
    /// A record names a node that is not in the log.
    UnknownNode {
        /// The id it named.
        id: u64,
    },
    /// A node's parent is not smaller than the node — the invariant that makes
    /// dropping a torn tail safe.
    BadParent {
        /// The node.
        id: u64,
        /// Its claimed parent.
        parent: u64,
    },
    /// A redo pointer names something that is not a child of that node.
    BadRedoChild {
        /// The node.
        id: u64,
        /// The child it claimed.
        child: u64,
    },
    /// A second origin record disagrees with the first — this journal is
    /// another file's history, arrived at through a hash collision or a stale
    /// path.
    WrongOrigin,
}

impl fmt::Display for FoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfOrder { found, expected } => {
                write!(f, "node {found} arrived where node {expected} was due")
            }
            Self::UnknownNode { id } => write!(f, "node {id} is not in this history"),
            Self::BadParent { id, parent } => {
                write!(
                    f,
                    "node {id} claims parent {parent}, which is not before it"
                )
            }
            Self::BadRedoChild { id, child } => {
                write!(
                    f,
                    "node {id} claims redo child {child}, which is not its child"
                )
            }
            Self::WrongOrigin => f.write_str("this journal belongs to a different file"),
        }
    }
}

impl std::error::Error for FoldError {}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Anything that can go wrong reading or writing persisted state.
#[derive(Debug)]
pub enum Error {
    /// The filesystem refused.
    Io {
        /// What was being read or written.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The file exists and is not a phosphor journal.
    NotAJournal {
        /// Which file.
        path: PathBuf,
    },
    /// A journal written by a build whose framing this one does not know.
    WrongFormat {
        /// Which file.
        path: PathBuf,
        /// What it says.
        found: u16,
        /// What this build writes.
        expected: u16,
    },
    /// A journal of the wrong kind, or of a schema version this build does not
    /// know.
    WrongStream {
        /// Which file.
        path: PathBuf,
        /// What it holds.
        found: Stream,
        /// What was asked for.
        expected: Stream,
    },
    /// A record's payload would not decode.
    Decode {
        /// Which file.
        path: PathBuf,
        /// Why.
        source: DecodeError,
    },
    /// A record decoded but does not fit the state.
    Fold {
        /// Which file.
        path: PathBuf,
        /// Why.
        source: FoldError,
    },
    /// A payload larger than a frame's length field.
    RecordTooLarge {
        /// How large.
        len: usize,
    },
    /// Neither `XDG_STATE_HOME` nor `HOME` is set to an absolute path, so there
    /// is nowhere to put state.
    NoStateHome,
    /// Two workspace roots hashed to the same directory. Loud rather than
    /// silent: the alternative is one repository reading another's history.
    RootCollision {
        /// The shared directory.
        dir: PathBuf,
        /// The root that got there first.
        occupant: PathBuf,
        /// The root that arrived second.
        arrival: PathBuf,
    },
}

impl Error {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NotAJournal { path } => {
                write!(f, "{}: not a phosphor journal", path.display())
            }
            Self::WrongFormat {
                path,
                found,
                expected,
            } => write!(
                f,
                "{}: journal format {found}, this build writes {expected}",
                path.display()
            ),
            Self::WrongStream {
                path,
                found,
                expected,
            } => write!(f, "{}: holds {found}, expected {expected}", path.display()),
            Self::Decode { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Fold { path, source } => write!(f, "{}: {source}", path.display()),
            Self::RecordTooLarge { len } => {
                write!(f, "record of {len} bytes is past the frame limit")
            }
            Self::NoStateHome => {
                f.write_str("neither XDG_STATE_HOME nor HOME names an absolute directory")
            }
            Self::RootCollision {
                dir,
                occupant,
                arrival,
            } => write!(
                f,
                "{} already holds state for {}, not {}",
                dir.display(),
                occupant.display(),
                arrival.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Fold { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Where state lives — Q1
// ---------------------------------------------------------------------------

/// The file that records which workspace root a state directory belongs to.
const ROOT_MARKER: &str = "root";

/// `$XDG_STATE_HOME`, or `$HOME/.local/state`.
///
/// Q1 puts phosphor's state at `$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/`,
/// keyed on the path and never on VCS identity, which is what makes one code
/// path serve a jj repo and a bare directory.
///
/// `SPIKES.md:304` names `etcetera` for this and it is not in the workspace
/// manifest; this function is the whole of what it would replace.
///
/// # Errors
///
/// [`Error::NoStateHome`] when neither variable names an absolute directory.
pub fn state_home() -> Result<PathBuf, Error> {
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME") {
        let dir = PathBuf::from(dir);
        if dir.is_absolute() {
            return Ok(dir);
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if home.is_absolute() {
            return Ok(home.join(".local").join("state"));
        }
    }
    Err(Error::NoStateHome)
}

/// FNV-1a, 64-bit, as sixteen hex digits.
///
/// Not cryptographic and not trying to be — it buckets a path into a directory
/// name, and [`workspace_dir_in`] catches a collision by writing the path it
/// belongs to alongside. Hand-rolled for the reason in the header, and stable
/// by construction: `DefaultHasher` would have been shorter and is explicitly
/// not stable across releases, which would silently orphan every user's state
/// on a toolchain bump.
#[must_use]
pub fn key(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The directory name for a workspace root, from its canonical path.
#[must_use]
pub fn workspace_key(canonical_root: &Path) -> String {
    key(canonical_root.to_string_lossy().as_bytes())
}

/// `$XDG_STATE_HOME/phosphor/<key>/`, created, with its root marker written.
///
/// # Errors
///
/// Whatever [`state_home`] and [`workspace_dir_in`] return.
pub fn workspace_dir(root: &Path) -> Result<PathBuf, Error> {
    let home = state_home()?;
    workspace_dir_in(&home, root)
}

/// [`workspace_dir`] under an explicit state home.
///
/// The explicit form exists because a test may not set an environment variable:
/// `std::env::set_var` is `unsafe` in edition 2024 and this workspace denies
/// `unsafe_code`. Tests pass a temporary directory here; a spawned child
/// process gets `XDG_STATE_HOME` set on its `Command`, which is safe.
///
/// # Errors
///
/// [`Error::Io`] if the root does not exist or the directory cannot be made,
/// [`Error::RootCollision`] if another root already owns this bucket.
///
/// # Two openers of one root
///
/// **The claim is a `rename`, not a `write`, and an empty marker is not an
/// occupant.** Both halves close one race, and the race is not hypothetical:
/// `S3` left an intermittent `RootCollision { occupant: "" }` in the journal
/// tests, and an empty occupant is a marker that was *read while it was being
/// written* — `fs::write` is create-truncate-write, so between the truncate and
/// the write the file exists and says nothing.
///
/// Two phosphor windows on one repository are exactly that reader and that
/// writer, so this was a product bug that the tests happened to catch: the
/// second window would refuse to open, naming a root that never existed. A
/// `rename` on POSIX has no third outcome, so a reader now sees either no
/// marker or the whole of one; treating an empty one as unclaimed is what
/// clears a marker already torn on disk by an older build.
pub fn workspace_dir_in(state_home: &Path, root: &Path) -> Result<PathBuf, Error> {
    let canonical = fs::canonicalize(root).map_err(|source| Error::io(root, source))?;
    let dir = state_home.join("phosphor").join(workspace_key(&canonical));
    fs::create_dir_all(&dir).map_err(|source| Error::io(&dir, source))?;

    let marker = dir.join(ROOT_MARKER);
    let mine = canonical.to_string_lossy().to_string();
    match fs::read(&marker) {
        Ok(bytes) if !bytes.is_empty() => {
            let occupant = String::from_utf8_lossy(&bytes).to_string();
            if occupant != mine {
                return Err(Error::RootCollision {
                    dir,
                    occupant: PathBuf::from(occupant),
                    arrival: canonical,
                });
            }
        }
        // An empty marker, or none: unclaimed either way.
        Ok(_) => claim(&marker, mine.as_bytes())?,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            claim(&marker, mine.as_bytes())?;
        }
        Err(source) => return Err(Error::io(&marker, source)),
    }

    Ok(dir)
}

/// Distinguishes one claim's sibling from another's inside a process; the
/// process id does it between processes.
static CLAIM_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Puts `root` in the marker so that no reader can see it half-written.
///
/// A sibling then a `rename`, which is [`Log::rewrite`]'s trick at a smaller
/// scale and for the same reason. The sibling's name carries the process id and
/// a counter, because two claimants renaming *the same* temporary file would
/// leave the second one renaming a path that is no longer there.
///
/// Two claimants both landing a rename is fine and is the ordinary case: they
/// write the same bytes. What stays best-effort is the collision check itself —
/// two *different* roots claiming one bucket in the same instant can both
/// succeed, as they could before. That needs a 64-bit FNV-1a collision and a
/// coincidence of timing; the case this detects is the persistent one, a bucket
/// already owned when the next session opens it.
fn claim(marker: &Path, root: &[u8]) -> Result<(), Error> {
    let sequence = CLAIM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = marker
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .ok_or_else(|| Error::io(marker, io::Error::from(io::ErrorKind::InvalidInput)))?;
    name.push(format!(".claiming.{}.{sequence}", std::process::id()));
    let tmp = marker.with_file_name(name);
    fs::write(&tmp, root).map_err(|source| Error::io(&tmp, source))?;
    fs::rename(&tmp, marker).map_err(|source| Error::io(marker, source))
}

/// Where one file's undo history lives inside a workspace's state directory.
///
/// One journal per file rather than one per workspace: compacting one file's
/// history does not rewrite every other file's, and opening a file reads only
/// its own log. The file's own path goes inside as [`undo::Record::Origin`], so
/// a hash collision here is caught the same way one in [`workspace_dir_in`] is.
#[must_use]
pub fn undo_path(workspace_dir: &Path, canonical_file: &Path) -> PathBuf {
    workspace_dir.join("undo").join(format!(
        "{}.journal",
        key(canonical_file.to_string_lossy().as_bytes())
    ))
}

// ---------------------------------------------------------------------------
// The undo schema
// ---------------------------------------------------------------------------

/// `T030`'s schema: a buffer's undo tree, as the sequence of mutations that
/// built it.
///
/// The log records what the tree *did*, not what it *is* — a node created, the
/// cursor moved, a save marked — because those are exactly the mutators
/// `phosphor-buffer`'s `UndoTree` has, and a log of mutations cannot disagree
/// with the tree the way a periodically-written copy of its state can.
pub mod undo {
    use super::{DecodeError, Decoder, Encoder, FoldError, Folded, Stream};

    /// A point in the undo tree.
    ///
    /// The same opaque non-negative integer as `phosphor_buffer::undo::NodeId`
    /// and [`crate::request::CheckpointId`] — `undo.rs:168-175` says why all
    /// three are one number. Ids are dense and in creation order, so
    /// [`NodeId`] `0` is the root and a node's parent is always smaller.
    pub type NodeId = u64;

    /// The root, and the state the buffer was in when the history started.
    pub const ROOT: NodeId = 0;

    /// A half-open range of char offsets.
    ///
    /// Mirrors `phosphor_buffer::undo::CharRange` (`undo.rs:198-204`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct CharRange {
        /// First char in the range.
        pub start: usize,
        /// First char after it.
        pub end: usize,
    }

    /// Where the cursor and selection were.
    ///
    /// Mirrors `phosphor_buffer::undo::Caret` (`undo.rs:234-240`). Persisted on
    /// both sides of every change because undo restores it, and a step that got
    /// the text right and the cursor wrong is not exact.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Caret {
        /// Char offset of the cursor.
        pub offset: usize,
        /// The selection, if there was one.
        pub selection: Option<CharRange>,
    }

    /// One replacement: at char offset `at`, `removed` becomes `inserted`.
    ///
    /// Mirrors `phosphor_buffer::undo::Edit` (`undo.rs:285-294`), including the
    /// rule that makes multi-edit changes readable: `at` is against the text as
    /// it stands after every earlier edit in the same [`Change`].
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct Edit {
        /// Char offset, after every earlier edit in the same change.
        pub at: usize,
        /// What was there.
        pub removed: String,
        /// What replaces it.
        pub inserted: String,
    }

    /// One undo step: the edits of a single group, and the caret on both sides.
    ///
    /// Mirrors `phosphor_buffer::undo::Change` (`undo.rs:458-466`).
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct Change {
        /// In application order.
        pub edits: Vec<Edit>,
        /// Where the caret was before the group.
        pub before: Caret,
        /// Where it ended up.
        pub after: Caret,
    }

    /// One state of the buffer, and how it was reached.
    ///
    /// A field-for-field mirror of `phosphor_buffer::undo::Node`
    /// (`undo.rs:558-569`), `children` included — that field is *not* encoded,
    /// because the parent links carry it and `UndoTree::from_parts` recomputes
    /// it anyway (`undo.rs:979-982`). It is here so the conversion at the seam
    /// is a copy rather than a rebuild.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Node {
        /// The state this one was reached from. [`None`] only for the root.
        pub parent: Option<NodeId>,
        /// States reached from here, in creation order. Derived, not stored.
        pub children: Vec<NodeId>,
        /// Which child a redo takes — the branch most recently created or
        /// walked.
        pub redo_child: Option<NodeId>,
        /// The change that turns the parent's text into this one's. [`None`]
        /// only for the root.
        pub change: Option<Change>,
    }

    impl Node {
        fn root() -> Self {
            Self {
                parent: None,
                children: Vec::new(),
                redo_child: None,
                change: None,
            }
        }
    }

    /// One entry in an undo journal.
    ///
    /// Six records, and between them they say everything an `UndoTree` can be
    /// asked to hand over. The tags are stable: a new record is a new tag and a
    /// bump of [`Stream::UNDO`]'s version, never a reshuffle.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Record {
        /// Which file this history belongs to. Written once, at creation, and
        /// checked on every open — the file-level half of Q1's collision guard.
        Origin {
            /// The file's canonical path.
            path: String,
        },
        /// The text at the root node.
        ///
        /// Absent from a journal that has never been truncated, because the
        /// root is *"the buffer as it was when this tree started"* and that is
        /// implicit. `undo.rs:145-153` requires it the moment a compaction
        /// drops the oldest nodes: without it the surviving history replays
        /// into garbage. Nothing writes one yet — see this module's parent
        /// header — and it is in the format so that day is not a format change.
        Base {
            /// The text at the root.
            text: String,
        },
        /// A committed undo group, and the node it became.
        ///
        /// The only record an open group can never produce: a group that has
        /// not been committed has no node.
        Node {
            /// The new node's id. Must be the next one.
            id: NodeId,
            /// What it was reached from.
            parent: NodeId,
            /// The edits of the group, in application order.
            edits: Vec<Edit>,
            /// Where the caret was before.
            before: Caret,
            /// Where it ended up.
            after: Caret,
        },
        /// The buffer is now at this node — an undo, a redo, or a walk to a
        /// checkpoint.
        Cursor {
            /// Where it went.
            to: NodeId,
        },
        /// A redo pointer that the record stream alone would not reproduce.
        ///
        /// Written only by [`History::snapshot`]. During a session the pointer
        /// is derived exactly as the tree derives it — a new node becomes its
        /// parent's redo child, and a cursor move re-points every node on the
        /// path it walks (`undo.rs:811-828`, `undo.rs:891-956`) — but a
        /// snapshot lists nodes in id order, which would leave every branch
        /// point pointing at its newest child. This record is the fix-up, and
        /// it is emitted only where the two differ.
        Redo {
            /// The branch point.
            node: NodeId,
            /// The child a redo should take.
            child: NodeId,
        },
        /// The node the file on disk matches, or [`None`] if it matches none.
        Saved {
            /// Which node.
            node: Option<NodeId>,
        },
    }

    const TAG_ORIGIN: u64 = 1;
    const TAG_BASE: u64 = 2;
    const TAG_NODE: u64 = 3;
    const TAG_CURSOR: u64 = 4;
    const TAG_REDO: u64 = 5;
    const TAG_SAVED: u64 = 6;

    fn put_caret(out: &mut Encoder, caret: Caret) {
        out.usize(caret.offset);
        match caret.selection {
            Some(range) => {
                out.bool(true);
                out.usize(range.start);
                out.usize(range.end);
            }
            None => out.bool(false),
        }
    }

    fn get_caret(input: &mut Decoder<'_>) -> Result<Caret, DecodeError> {
        let offset = input.usize()?;
        let selection = if input.bool()? {
            Some(CharRange {
                start: input.usize()?,
                end: input.usize()?,
            })
        } else {
            None
        };
        Ok(Caret { offset, selection })
    }

    /// The fold of an undo journal: an undo tree, minus the text.
    ///
    /// `UndoTree::from_parts` takes exactly what [`History::into_parts`] hands
    /// back, and validates the same four invariants this fold enforces on the
    /// way in (`undo.rs:128-138`). Both check, because the log is the older of
    /// the two and a file outlives the process that wrote it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct History {
        origin: Option<String>,
        base: Option<String>,
        nodes: Vec<Node>,
        current: NodeId,
        saved: Option<NodeId>,
    }

    impl Default for History {
        /// A history at the root, with the buffer matching disk — the same
        /// starting state as `UndoTree::new` (`undo.rs:657-663`).
        fn default() -> Self {
            Self {
                origin: None,
                base: None,
                nodes: vec![Node::root()],
                current: ROOT,
                saved: Some(ROOT),
            }
        }
    }

    impl History {
        /// The file this history belongs to, once a [`Record::Origin`] has
        /// been recorded.
        #[must_use]
        pub fn origin(&self) -> Option<&str> {
            self.origin.as_deref()
        }

        /// The text at the root, if a truncating compaction ever wrote one.
        #[must_use]
        pub fn base(&self) -> Option<&str> {
            self.base.as_deref()
        }

        /// Every node, indexed by [`NodeId`]. Never empty — index `0` is the
        /// root.
        #[must_use]
        pub fn nodes(&self) -> &[Node] {
            &self.nodes
        }

        /// Where the buffer is.
        #[must_use]
        pub fn current(&self) -> NodeId {
            self.current
        }

        /// The node the file on disk matches.
        #[must_use]
        pub fn saved(&self) -> Option<NodeId> {
            self.saved
        }

        /// What `UndoTree::from_parts` takes, in its order
        /// (`undo.rs:989-993`). The conversion of each [`Node`] is a field
        /// copy; see this module's parent header for why it does not live here.
        #[must_use]
        pub fn into_parts(self) -> (Vec<Node>, NodeId, Option<NodeId>) {
            (self.nodes, self.current, self.saved)
        }

        fn get(&self, id: NodeId) -> Result<usize, FoldError> {
            usize::try_from(id)
                .ok()
                .filter(|index| *index < self.nodes.len())
                .ok_or(FoldError::UnknownNode { id })
        }

        /// Re-points every node on the path from the root to `to`, which is
        /// what both halves of `UndoTree::goto` do as they walk
        /// (`undo.rs:920-924`, `undo.rs:947-951`).
        fn walk_to(&mut self, to: NodeId) -> Result<(), FoldError> {
            let mut at = to;
            while let Some(parent) = self.nodes[self.get(at)?].parent {
                let index = self.get(parent)?;
                self.nodes[index].redo_child = Some(at);
                at = parent;
            }
            self.current = to;
            Ok(())
        }
    }

    impl Folded for History {
        type Record = Record;

        const STREAM: Stream = Stream::UNDO;

        fn encode(record: &Self::Record, out: &mut Encoder) {
            match record {
                Record::Origin { path } => {
                    out.u64(TAG_ORIGIN);
                    out.str(path);
                }
                Record::Base { text } => {
                    out.u64(TAG_BASE);
                    out.str(text);
                }
                Record::Node {
                    id,
                    parent,
                    edits,
                    before,
                    after,
                } => {
                    out.u64(TAG_NODE);
                    out.u64(*id);
                    out.u64(*parent);
                    out.seq_len(edits.len());
                    for edit in edits {
                        out.usize(edit.at);
                        out.str(&edit.removed);
                        out.str(&edit.inserted);
                    }
                    put_caret(out, *before);
                    put_caret(out, *after);
                }
                Record::Cursor { to } => {
                    out.u64(TAG_CURSOR);
                    out.u64(*to);
                }
                Record::Redo { node, child } => {
                    out.u64(TAG_REDO);
                    out.u64(*node);
                    out.u64(*child);
                }
                Record::Saved { node } => {
                    out.u64(TAG_SAVED);
                    out.option_u64(*node);
                }
            }
        }

        fn decode(bytes: &[u8]) -> Result<Self::Record, DecodeError> {
            let mut input = Decoder::new(bytes);
            let record = match input.u64()? {
                TAG_ORIGIN => Record::Origin { path: input.str()? },
                TAG_BASE => Record::Base { text: input.str()? },
                TAG_NODE => {
                    let id = input.u64()?;
                    let parent = input.u64()?;
                    let count = input.seq_len()?;
                    let mut edits = Vec::with_capacity(count);
                    for _ in 0..count {
                        edits.push(Edit {
                            at: input.usize()?,
                            removed: input.str()?,
                            inserted: input.str()?,
                        });
                    }
                    let before = get_caret(&mut input)?;
                    let after = get_caret(&mut input)?;
                    Record::Node {
                        id,
                        parent,
                        edits,
                        before,
                        after,
                    }
                }
                TAG_CURSOR => Record::Cursor { to: input.u64()? },
                TAG_REDO => Record::Redo {
                    node: input.u64()?,
                    child: input.u64()?,
                },
                TAG_SAVED => Record::Saved {
                    node: input.option_u64()?,
                },
                tag => return Err(DecodeError::UnknownRecord { tag }),
            };
            input.finish()?;
            Ok(record)
        }

        fn apply(&mut self, record: Self::Record) -> Result<(), FoldError> {
            match record {
                Record::Origin { path } => match &self.origin {
                    Some(existing) if *existing != path => return Err(FoldError::WrongOrigin),
                    _ => self.origin = Some(path),
                },
                Record::Base { text } => self.base = Some(text),
                Record::Node {
                    id,
                    parent,
                    edits,
                    before,
                    after,
                } => {
                    let expected = self.nodes.len() as u64;
                    if id != expected {
                        return Err(FoldError::OutOfOrder {
                            found: id,
                            expected,
                        });
                    }
                    if parent >= id {
                        return Err(FoldError::BadParent { id, parent });
                    }
                    let index = self.get(parent)?;
                    self.nodes[index].children.push(id);
                    self.nodes[index].redo_child = Some(id);
                    self.nodes.push(Node {
                        parent: Some(parent),
                        children: Vec::new(),
                        redo_child: None,
                        change: Some(Change {
                            edits,
                            before,
                            after,
                        }),
                    });
                    self.current = id;
                }
                Record::Cursor { to } => {
                    self.get(to)?;
                    self.walk_to(to)?;
                }
                Record::Redo { node, child } => {
                    let index = self.get(node)?;
                    if !self.nodes[index].children.contains(&child) {
                        return Err(FoldError::BadRedoChild { id: node, child });
                    }
                    self.nodes[index].redo_child = Some(child);
                }
                Record::Saved { node } => {
                    if let Some(id) = node {
                        self.get(id)?;
                    }
                    self.saved = node;
                }
            }
            Ok(())
        }

        fn snapshot(&self) -> Vec<Self::Record> {
            let mut out = Vec::with_capacity(self.nodes.len() + 4);
            if let Some(path) = &self.origin {
                out.push(Record::Origin { path: path.clone() });
            }
            if let Some(text) = &self.base {
                out.push(Record::Base { text: text.clone() });
            }
            for (index, node) in self.nodes.iter().enumerate().skip(1) {
                let (Some(parent), Some(change)) = (node.parent, node.change.as_ref()) else {
                    continue;
                };
                out.push(Record::Node {
                    id: index as NodeId,
                    parent,
                    edits: change.edits.clone(),
                    before: change.before,
                    after: change.after,
                });
            }
            // Replaying the nodes above leaves every branch point pointing at
            // its newest child; the trailing cursor move re-points the path to
            // `current`. Everything else needs saying outright.
            for (index, node) in self.nodes.iter().enumerate() {
                if let Some(child) = node.redo_child
                    && node.children.last() != Some(&child)
                {
                    out.push(Record::Redo {
                        node: index as NodeId,
                        child,
                    });
                }
            }
            out.push(Record::Cursor { to: self.current });
            out.push(Record::Saved { node: self.saved });
            out
        }
    }
}

/// An undo journal — [`Log`] over [`undo::History`].
pub type UndoLog = Log<undo::History>;
