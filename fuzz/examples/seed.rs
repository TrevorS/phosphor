//! Writes `fuzz/seeds/<target>/` from real inputs the repo already has.
//!
//! Not a fuzz target — a normal binary, run by `scripts/fuzz.sh seed`, and safe
//! to run at any time: it is idempotent and writes only under `fuzz/seeds/`.
//!
//! # Why a generator rather than checked-in blobs alone
//!
//! A fuzzer starting from an empty corpus spends its first hour rediscovering
//! the file format — `PHOSJRNL`, a 16-byte header, a `u32` length and a `u32`
//! CRC — and never gets past it, because a CRC-32 is not a thing coverage
//! feedback can solve. The corpus is where the format comes from.
//!
//! The seeds are also committed, so they are reviewable. What this binary adds
//! is that they are *derived*: the journals are written by the real writer
//! (`Journal::append` computes its own checksums, so nothing here duplicates
//! the framing), the key notation is every string literal in `runtime/*.scm`,
//! and the themes are the shipped files copied verbatim. Regenerate after
//! changing the format and the corpus follows; a hand-built blob would rot
//! silently and the fuzzer would keep reporting green over a format it could no
//! longer parse.
//!
//! The growing corpus (`fuzz/corpus/`) is *not* committed — `fuzz/.gitignore`
//! says why.

use std::fs;
use std::path::{Path, PathBuf};

use phosphor_core::input::key::parse_seq;
use phosphor_core::journal::{
    Journal, Stream, UndoLog,
    undo::{Caret, CharRange, Edit, Record},
};

/// How long a `runtime/*.scm` string literal may be, in keys, to be seeded into
/// `key_notation`'s corpus. See [`key_notation_seeds`].
const MAX_SEED_KEYS: usize = 8;

fn repo_root() -> PathBuf {
    // The fuzz crate is one level under the repo root, and `CARGO_MANIFEST_DIR`
    // is absolute — so this works from any cwd, which matters because
    // `scripts/fuzz.sh` runs cargo from `fuzz/`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the fuzz crate sits under the repo root")
        .to_path_buf()
}

fn seed_dir(target: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("seeds")
        .join(target);
    fs::create_dir_all(&dir).expect("seeds/ is writable");
    dir
}

fn write(target: &str, name: &str, bytes: &[u8]) {
    let path = seed_dir(target).join(name);
    fs::write(&path, bytes).expect("a seed file is writable");
    println!("  {}  ({} bytes)", path.display(), bytes.len());
}

/// A caret with no selection.
fn at(offset: usize) -> Caret {
    Caret {
        offset,
        selection: None,
    }
}

/// Four histories, chosen to put a different record tag in each seed.
///
/// The tags are what a mutation has to discover: `Origin`, `Base`, `Node`,
/// `Cursor`, `Redo` and `Saved` are six leading varints, and a corpus holding
/// only `Node` teaches the fuzzer one of them.
fn histories() -> Vec<(&'static str, Vec<Record>)> {
    let insert = |offset: usize, text: &str| Edit {
        at: offset,
        removed: String::new(),
        inserted: text.to_owned(),
    };
    let node = |id, parent, edits, before, after| Record::Node {
        id,
        parent,
        edits,
        before,
        after,
    };

    vec![
        // Nothing but a header. The reader's create path, and the shortest
        // input that is a valid journal.
        ("empty", Vec::new()),
        // The ordinary session: a file, two committed groups, a save point.
        (
            "linear",
            vec![
                Record::Origin {
                    path: "/tmp/main.rs".to_owned(),
                },
                node(1, 0, vec![insert(0, "fn main() {}\n")], at(0), at(13)),
                node(2, 1, vec![insert(12, "\n    todo!()")], at(13), at(24)),
                Record::Saved { node: Some(2) },
            ],
        ),
        // A branch: undo out of node 2, edit again, and the redo fix-up a
        // snapshot writes. This is the record stream compaction emits, and the
        // only place `Redo` appears.
        (
            "branched",
            vec![
                Record::Origin {
                    path: "/tmp/notes.md".to_owned(),
                },
                node(1, 0, vec![insert(0, "one")], at(0), at(3)),
                node(2, 1, vec![insert(3, " two")], at(3), at(7)),
                Record::Cursor { to: 1 },
                node(
                    3,
                    1,
                    vec![Edit {
                        at: 0,
                        removed: "one".to_owned(),
                        inserted: "ONE".to_owned(),
                    }],
                    Caret {
                        offset: 0,
                        selection: Some(CharRange { start: 0, end: 3 }),
                    },
                    at(3),
                ),
                Record::Redo { node: 1, child: 2 },
                Record::Saved { node: None },
            ],
        ),
        // A truncating compaction: the only stream with a `Base`, and the one
        // whose root text is not implicit.
        (
            "based",
            vec![
                Record::Origin {
                    path: "/tmp/lib.rs".to_owned(),
                },
                Record::Base {
                    text: "pub fn answer() -> u32 {\n    42\n}\n".to_owned(),
                },
                node(1, 0, vec![insert(33, "\n")], at(33), at(34)),
                Record::Cursor { to: 0 },
            ],
        ),
    ]
}

/// Real journal files, plus the two damaged shapes a crash produces.
fn journal_open_seeds(scratch: &Path) {
    println!("seeds/journal_open:");
    for (name, records) in histories() {
        let path = scratch.join(format!("{name}.journal"));
        let _ = fs::remove_file(&path);
        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in records {
            log.append(record).expect("a writer's record is appendable");
        }
        log.sync().expect("fsync");
        drop(log);
        let bytes = fs::read(&path).expect("the journal is readable");

        // The torn tail and the zero-run are the two shapes `journal.rs`'s
        // header names. Seeding them rather than waiting for a mutation to
        // produce one costs two files and saves the fuzzer the discovery.
        if bytes.len() > 24 {
            let cut = bytes.len() - 5;
            write("journal_open", &format!("{name}-torn"), &bytes[..cut]);
            let mut zeroed = bytes[..cut].to_vec();
            zeroed.extend_from_slice(&[0; 5]);
            write("journal_open", &format!("{name}-zeros"), &zeroed);
        }
        write("journal_open", name, &bytes);
    }
}

/// The same records as `[len: u8][payload]` runs — `journal_records`'s input.
fn journal_records_seeds(scratch: &Path) {
    println!("seeds/journal_records:");
    for (name, records) in histories() {
        let path = scratch.join(format!("{name}.records"));
        let _ = fs::remove_file(&path);
        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in records {
            log.append(record).expect("appendable");
        }
        log.sync().expect("fsync");
        drop(log);

        // Read the payloads back through the reader rather than re-encoding
        // them here: the seed is then exactly what the framing hands a decoder.
        let (_, payloads, _) = Journal::open(&path, Stream::UNDO).expect("it reopens");
        let mut out = Vec::new();
        for payload in &payloads {
            let Ok(len) = u8::try_from(payload.len()) else {
                // A payload past 255 cannot be spelled in this target's input
                // format. None of the histories above produces one; if one ever
                // does, the seed is short rather than wrong.
                continue;
            };
            out.push(len);
            out.extend_from_slice(payload);
        }
        write("journal_records", name, &out);
    }
}

/// The string literals in `runtime/*.scm` that could be a key sequence.
///
/// Every literal, filtered — not every literal, and not only the ones in a
/// `keymap-set!` position. Recognising that position means parsing Scheme;
/// taking all 374 of them means seeding a corpus with capability names and
/// paragraphs of Scheme docstrings, which is three hundred redundant starting
/// points for a parser that dispatches on one character at a time.
///
/// [`MAX_SEED_KEYS`] is the filter, and it is the parser's own judgement rather
/// than a length in bytes: a string is kept if `parse_seq` reads it as a short
/// sequence. `"gsib"` and `"<C-w>v"` stay; a sentence does not.
fn key_notation_seeds(root: &Path) {
    println!("seeds/key_notation:");
    let mut seen: Vec<String> = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(root.join("runtime"))
        .expect("runtime/ exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "scm"))
        .collect();
    entries.sort();

    for path in entries {
        let source = fs::read_to_string(&path).expect("a runtime file is readable");
        // A hand-rolled scan rather than a regex: this is a two-state machine
        // and the crate has no regex dependency to borrow.
        let mut chars = source.chars();
        while let Some(ch) = chars.next() {
            if ch != '"' {
                continue;
            }
            let mut literal = String::new();
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                literal.push(ch);
            }
            let keys = parse_seq(&literal).map_or(usize::MAX, |keys| keys.len());
            if !literal.is_empty() && keys <= MAX_SEED_KEYS && !seen.contains(&literal) {
                seen.push(literal);
            }
        }
    }

    // Plus the shapes `parse_seq`'s own header calls out, in case no runtime
    // file happens to spell one: the leader, a bracketed key, the unclosed
    // bracket that cost `.` its correctness, and the `S`/`P`/`C` collision
    // `notation_of` respells.
    for extra in ["SPC f", "<C-w>v", "<<", "<nope>", "SPC<esc>", "]u", "3dd"] {
        if !seen.iter().any(|s| s == extra) {
            seen.push(extra.to_owned());
        }
    }

    for (i, literal) in seen.iter().enumerate() {
        write("key_notation", &format!("{i:03}"), literal.as_bytes());
    }
}

/// The shipped themes, verbatim.
fn theme_load_seeds(root: &Path) {
    println!("seeds/theme_load:");
    let dir = root.join("crates/phosphor-ui/themes");
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("the themes directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "theme"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .expect("a theme file has a name")
            .to_string_lossy()
            .into_owned();
        let bytes = fs::read(&path).expect("a theme file is readable");
        write("theme_load", &name, &bytes);
    }
}

fn main() {
    let root = repo_root();
    let scratch = std::env::temp_dir().join("phosphor-fuzz-seed");
    fs::create_dir_all(&scratch).expect("a scratch directory");

    journal_open_seeds(&scratch);
    journal_records_seeds(&scratch);
    key_notation_seeds(&root);
    theme_load_seeds(&root);

    let _ = fs::remove_dir_all(&scratch);
    println!("\nseeds written under {}/seeds", env!("CARGO_MANIFEST_DIR"));
}
