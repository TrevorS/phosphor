//! The on-disk formats, read from files **this build did not write**.
//!
//! # The genre this closes
//!
//! Every other journal test writes a log and reads it back in the same process,
//! with the same encoder on both ends. That proves the codec is self-consistent
//! and proves nothing at all about the thing a user actually depends on: **the
//! store you had yesterday still opens today.** A codec can round-trip
//! perfectly with itself and still have stopped reading every file already on
//! disk — the two failures are indistinguishable to a test that writes its own
//! input.
//!
//! `phosphor-core` had no `include_bytes!` anywhere before this file. The
//! journal carries a magic, a `FORMAT_VERSION` and a per-stream `version`, all
//! of which exist to make an incompatible change *detectable* — and nothing
//! held a byte of an older file to detect it against.
//!
//! It matters most for `SEEN`. Seen-state is the product's thesis: `CP-5` fails
//! if the markers do not change how you read the file, and a marker that does
//! not survive an upgrade is a marker you cannot trust. Losing it is silent —
//! an unreadable log reads as an empty store, which draws as *"nothing here is
//! new"*, which is the one lie this editor must never tell.
//!
//! # Regenerating
//!
//! `PHOSPHOR_WRITE_GOLDEN_JOURNALS=1 cargo test -p phosphor-core --test on_disk_format`
//! rewrites both fixtures from the current writer and passes. **Committing that
//! diff is a decision, not a formality**: a changed golden means files already
//! on disk stop being readable, so the diff is the record of a migration and
//! the review is where someone asks what happens to the stores that exist.
//! This mirrors `PHOSPHOR_WRITE_SURFACES=1` in `vocabulary.rs`, which guards
//! the wire contract for the same reason and in the same shape.
//!
//! # Why the expectation is built in code rather than committed beside it
//!
//! The fixtures are bytes; the assertion is a `Seen` and a `History` built
//! here. So this fails in two different ways, and they mean different things:
//! a file that no longer *decodes* is a framing or codec break, and a file that
//! decodes to the wrong *state* is a semantic one. A committed text rendering
//! of the state would collapse both into "the snapshot moved".

use std::path::{Path, PathBuf};

use phosphor_core::journal::{Folded, Log, UndoLog, undo};
use phosphor_core::request::{Actor, Position, RegionId, Span};
use phosphor_core::store::{self, SeenLog};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The committed bytes. `include_bytes!` rather than a runtime read so a
/// deleted fixture is a compile error — a missing golden that made the test
/// skip would be the same silence this file exists to break.
const SEEN_GOLDEN: &[u8] = include_bytes!("fixtures/seen-v1.journal");
const UNDO_GOLDEN: &[u8] = include_bytes!("fixtures/undo-v1.journal");

/// A directory that removes itself. No `tempfile` dependency: this crate is
/// dependency-free at the floor and a test is not the place to change that —
/// `journal.rs`'s own helper says the same and this is its twin.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "phosphor-golden-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch directory");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Where a regeneration writes.
fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn regenerating() -> bool {
    std::env::var("PHOSPHOR_WRITE_GOLDEN_JOURNALS").is_ok_and(|value| !value.is_empty())
}

// ---------------------------------------------------------------------------
// What each golden holds
// ---------------------------------------------------------------------------

fn span(first: u32, last: u32) -> Span {
    Span {
        start: Position {
            line: first,
            column: 1,
        },
        end: Position {
            line: last,
            column: 1,
        },
    }
}

/// A region an agent touched and a person has not read.
///
/// Deliberately **not** default-everything: a golden whose every field is the
/// type's default cannot tell a codec that dropped a field from one that wrote
/// it. `state` is `Unseen`, the authors differ from each other, and `revisions`
/// is non-zero.
fn regions() -> Vec<store::Region> {
    vec![
        store::Region {
            id: RegionId(1),
            path: PathBuf::from("src/retry.rs"),
            span: span(4, 10),
            author: Actor::Claude,
            declared_by: Actor::Claude,
            state: store::SeenState::Unseen,
            revisions: 2,
            fingerprint: None,
        },
        store::Region {
            id: RegionId(2),
            path: PathBuf::from("src/fetch.rs"),
            span: span(12, 13),
            author: Actor::You,
            declared_by: Actor::Claude,
            state: store::SeenState::Seen,
            revisions: 0,
            fingerprint: None,
        },
    ]
}

/// The seen log's records, in the order a session would have appended them.
///
/// Includes a **tombstone**, because a `Gone` is the record whose absence is
/// invisible: a log that dropped it folds to a store with a region that came
/// back from the dead, and every count on screen is one too high.
fn seen_records() -> Vec<store::persist::Record> {
    let mut out: Vec<store::persist::Record> = regions()
        .into_iter()
        .map(|region| store::persist::Record::Region(Box::new(region)))
        .collect();
    out.push(store::persist::Record::RegionGone(RegionId(3)));
    out.push(store::persist::Record::Minted {
        regions: 3,
        anchors: 0,
    });
    out
}

/// The undo log's records: an origin and two edits on one branch.
fn undo_records() -> Vec<undo::Record> {
    vec![
        undo::Record::Origin {
            path: "src/retry.rs".to_owned(),
        },
        undo::Record::Node {
            id: 1,
            parent: 0,
            edits: vec![undo::Edit {
                at: 0,
                removed: String::new(),
                inserted: "hello".to_owned(),
            }],
            before: undo::Caret {
                offset: 0,
                selection: None,
            },
            after: undo::Caret {
                offset: 5,
                selection: None,
            },
        },
        undo::Record::Cursor { to: 1 },
        undo::Record::Saved { node: Some(1) },
    ]
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// Writes `records` into a fresh log at `path` and closes it.
///
/// `Log::open` carries the stream in `F::STREAM`, so the header this writes is
/// the one the type declares — which is the property the regeneration path
/// needs: a golden must be a file the shipping writer would actually produce.
fn write<F: Folded>(path: &Path, records: Vec<F::Record>) {
    let _ = std::fs::remove_file(path);
    let (mut log, _) = Log::<F>::open(path).expect("a fresh log opens");
    for record in records {
        log.append(record).expect("append");
    }
}

/// **`SEEN` — a store written by an older build still reads.**
#[test]
fn a_committed_seen_log_still_opens_and_folds_to_what_it_held() {
    let dir = TempDir::new("seen");
    let path = dir.path.join("seen.journal");

    if regenerating() {
        write::<store::Seen>(&fixture_path("seen-v1.journal"), seen_records());
    }

    std::fs::write(&path, SEEN_GOLDEN).expect("the golden lands on disk");
    let (log, recovery) = SeenLog::open(&path).expect("a committed seen log opens");

    assert!(
        recovery.is_clean(),
        "the committed golden is not a torn file; recovery said {recovery:?}"
    );

    let state = log.state();
    let expected = regions();
    assert_eq!(
        state.regions.len(),
        expected.len(),
        "both regions survived the round trip through disk"
    );
    for region in expected {
        assert_eq!(
            state.regions.get(&region.id),
            Some(&region),
            "region {:?} folded back field for field",
            region.id
        );
    }
    assert!(
        !state.regions.contains_key(&RegionId(3)),
        "the tombstone still buries its region — without it every count is one high"
    );
    assert_eq!(
        state.minted_regions, 3,
        "minted ids survive, so a restart cannot reissue a retired one"
    );
}

/// **`UNDO` — a history written by an older build still reads.**
#[test]
fn a_committed_undo_log_still_opens_and_folds_to_what_it_held() {
    let dir = TempDir::new("undo");
    let path = dir.path.join("undo.journal");

    if regenerating() {
        write::<undo::History>(&fixture_path("undo-v1.journal"), undo_records());
    }

    std::fs::write(&path, UNDO_GOLDEN).expect("the golden lands on disk");
    let (log, recovery) = UndoLog::open(&path).expect("a committed undo log opens");

    assert!(
        recovery.is_clean(),
        "the committed golden is not a torn file; recovery said {recovery:?}"
    );

    let history = log.state();
    assert_eq!(history.nodes().len(), 2, "the root plus the one edit");
    assert_eq!(history.current(), 1, "and the branch it ended on");
    assert_eq!(history.saved(), Some(1), "the save point came back too");
}

/// **The header is checked, and this is what makes the two above meaningful.**
///
/// A reader that ignored the version bytes would pass every assertion in this
/// file for as long as the record layout happened to stay compatible, and then
/// silently misread the first file written after it did not. So this asserts
/// the refusal directly: the same bytes, one stream field changed, must fail to
/// open rather than fold into something plausible.
#[test]
fn a_log_from_an_unknown_stream_version_is_refused_rather_than_guessed_at() {
    let dir = TempDir::new("version");
    let path = dir.path.join("seen.journal");

    let mut bytes = SEEN_GOLDEN.to_vec();
    // Magic (8), format version (2), stream kind (2), stream version (2).
    bytes[12] = bytes[12].wrapping_add(1);
    std::fs::write(&path, &bytes).expect("the doctored golden lands on disk");

    assert!(
        SeenLog::open(&path).is_err(),
        "a stream version this build does not know must be an error at the header, \
         not a decode failure fifty records in — or an empty store that reads as \
         'nothing here is new'"
    );
}
