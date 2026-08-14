//! **What an undo journal costs on the editor's hot path**, and what a long
//! session costs at startup — the two numbers `T030` designed against and
//! nobody had measured.
//!
//! Run it with `just bench` (or `cargo bench -p phosphor-core`). It prints five
//! tables and a verdict, and asserts the structural half of what it prints.
//!
//! # Why a number here changes something
//!
//! [`journal`]'s header makes three claims that are architecture, not
//! implementation, and each one is a decision somebody could reverse:
//!
//! * *"[`Log::append`] does `write_all` and nothing else"* — one frame per
//!   commit group, and a commit group is roughly one `<esc>`. If an append is
//!   slow enough to feel, the design changes: batching, or a background writer.
//! * *"an `fsync` per undo group is an `fsync` per `<esc>`, which is felt"* —
//!   `felt` is a word. This benchmark's second table is the number behind it,
//!   and it is the number to quote at anyone who proposes moving [`Log::sync`]
//!   onto the append path for durability's sake.
//! * *"a history with ten thousand nodes does not rewrite itself every
//!   keystroke"* — the doubling policy in [`Log::should_compact`]. True, and it
//!   is not the interesting half. **[`undo::History::snapshot`] emits one
//!   `Record::Node` per node and drops none**, so in a session that only types,
//!   a compaction rewrites the whole file and reclaims nothing. That is the
//!   third and fourth tables, and it is `T095`'s argument either way.
//!
//! Nothing here is a microbenchmark of the codec. Every measurement runs
//! against a real file in a scratch directory that removes itself, because the
//! costs that matter — the `write_all` syscall, the `F_FULLFSYNC`, the
//! `rename` — are the ones a memory buffer would hide.
//!
//! # The scale everything is against
//!
//! [`FRAME_BUDGET_MS`]: 16.7 ms, one frame at 60fps. Design Language §8 makes a
//! torn frame a P0, and the editor's loop is single-threaded — so anything on
//! the keystroke path that costs a frame budget *is* a dropped frame. The
//! tables print `per frame` columns for exactly that comparison, and the
//! verdict names the threshold each number would have to reach before somebody
//! should act on it.
//!
//! # The two session shapes
//!
//! A record type nobody writes has no cost, so both shapes are what the binary
//! actually appends (`crates/phosphor/src/main.rs`'s `struct Timeline`):
//!
//! * **typing** — one `Record::Node` per commit group, and nothing else. The
//!   pure case, and the one where compaction has nothing to reclaim.
//! * **walking** — the same, plus a `Record::Cursor` every
//!   [`UNDO_EVERY`] groups (an undo, which `Timeline::goto` journals) and a
//!   `Record::Saved` every [`WRITE_EVERY`] (a write). These are the records a
//!   snapshot *can* collapse, so this arm is what stops the typing arm's result
//!   reading as "compaction is broken".
//!
//! The walking arm also exposes something the typing arm cannot: applying a
//! `Record::Cursor` calls `History::walk_to`, which re-points `redo_child` from
//! the target **back to the root**. That is O(depth), on the way in and again
//! on every fold at startup — so the fifth table is not a formality.
//!
//! # What these numbers are not
//!
//! Wall clock on one machine with one filesystem. The *shapes* are what is
//! asserted — flat versus climbing, O(n) versus O(n²), records-in versus
//! records-out — and the shapes are machine-independent; the absolute
//! microseconds are information for a person deciding where to look. This is
//! the same rule `phosphor-ui/benches/frame_cache.rs` and
//! `phosphor/benches/vm_invocations.rs` are written to, and it is why
//! `just bench` is deliberately not part of `just gate`.
//!
//! Two of the assertions are counts rather than times and hold on any machine:
//! a typing session's compaction reclaims no records, and a walking session's
//! does. Those are the two the `T095` argument rests on.
//!
//! Owned by `harness`.
//!
//! [`journal`]: phosphor_core::journal
//! [`Log::append`]: phosphor_core::journal::Log::append
//! [`Log::should_compact`]: phosphor_core::journal::Log::should_compact
//! [`Log::sync`]: phosphor_core::journal::Log::sync
//! [`undo::History::snapshot`]: phosphor_core::journal::undo::History

#![expect(
    clippy::print_stdout,
    reason = "a benchmark's entire output is stdout; the workspace lint is aimed at the TUI, \
              which must never write to a terminal it is drawing on"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use phosphor_core::journal::UndoLog;
use phosphor_core::journal::undo::{Caret, Edit, NodeId, ROOT, Record};

/// One frame at 60fps, in milliseconds. **The only scale in this file that
/// means anything**: the editor's loop is single-threaded, so a keystroke that
/// spends this long in the journal has spent a whole frame not drawing.
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;

/// Appends per timed block in the first table. Large enough that the block's
/// own timing overhead is noise, small enough that eight blocks still show a
/// trend if there is one.
const BLOCK: u64 = 4_096;

/// Blocks. Eight of them is an 8x growth in log length across the table, which
/// is what makes "flat" distinguishable from "grows with the log".
const BLOCKS: u64 = 8;

/// Records in the durability table. Small on purpose: an `fsync` arm at
/// [`BLOCK`] scale would dominate the whole run, and the contrast is visible in
/// three digits.
const SYNCED: u64 = 256;

/// Commit groups per rung of the compaction and startup ladder. A 16x climb —
/// enough that O(n) and O(n^2) are different answers rather than different
/// noise.
const LADDER: [u64; 5] = [1_024, 2_048, 4_096, 8_192, 16_384];

/// One undo in this many commit groups, in the walking arm. Roughly a
/// vim user who backs out of one edit in eight.
const UNDO_EVERY: u64 = 8;

/// One write in this many commit groups, in the walking arm.
const WRITE_EVERY: u64 = 64;

/// Characters a commit group inserts. A group is a whole insert session
/// (`i` … `<esc>`), not a keystroke, so a dozen characters is the honest
/// middle — and the payload size is what the `write_all` actually costs.
const GROUP_TEXT: &str = "retry_count ";

fn main() {
    let scratch = Scratch::new();

    println!("phosphor · B1 the undo journal — append, durability, compaction, startup");
    println!(
        "  frame budget  {FRAME_BUDGET_MS:.1} ms at 60fps — the loop is single-threaded, so a \
         keystroke that costs this costs a frame"
    );
    println!("  scratch       {}", scratch.path.display());
    println!(
        "  commit group  one Record::Node, one edit, {} chars inserted",
        GROUP_TEXT.chars().count()
    );
    println!();

    let appends = append_ladder(&scratch.path.join("append.undo"));
    let durability = durability(&scratch.path);

    let mut typing = Vec::new();
    let mut walking = Vec::new();
    for groups in LADDER {
        typing.push(rung(Shape::Typing, groups, &scratch.path));
        walking.push(rung(Shape::Walking, groups, &scratch.path));
    }
    let policy = policy_probe(&scratch.path.join("policy.undo"));

    append_table(&appends);
    durability_table(&durability);
    compaction_table(
        "compaction — typing: every commit group is a node, nothing else",
        &typing,
    );
    compaction_table(
        "compaction — walking: the same, plus an undo in 8 and a write in 64",
        &walking,
    );
    startup_table(&typing, &walking);
    verdict(&appends, &durability, &typing, &walking, policy);
}

// ---------------------------------------------------------------------------
// 1 · The append path
// ---------------------------------------------------------------------------

/// One timed block of appends.
#[derive(Debug, Clone, Copy)]
struct Block {
    /// Records already in the log when the block started.
    before: u64,
    /// What the block took, for [`BLOCK`] appends.
    elapsed: Duration,
    /// Bytes the log grew by, so the payload size is visible next to the cost.
    bytes: u64,
}

impl Block {
    fn micros_per_append(&self) -> f64 {
        self.elapsed.as_secs_f64() * 1e6 / BLOCK as f64
    }

    fn bytes_per_record(&self) -> f64 {
        self.bytes as f64 / BLOCK as f64
    }

    /// How many commit groups fit in one frame at this cost. The number to
    /// compare against "how fast can a human press `<esc>`".
    fn appends_per_frame(&self) -> f64 {
        FRAME_BUDGET_MS * 1e3 / self.micros_per_append()
    }
}

/// Appends [`BLOCKS`] x [`BLOCK`] records to one log, timing each block.
///
/// The claim is that [`Log::append`] is O(1) in the length of the log it is
/// appending to — it seeks nothing and reads nothing — so the per-append cost
/// of the last block must not track the log's size.
fn append_ladder(path: &Path) -> Vec<Block> {
    let (mut log, _recovery) = UndoLog::open(path).expect("a fresh journal opens");
    let mut out = Vec::with_capacity(BLOCKS as usize);
    let mut id: NodeId = 1;

    for _ in 0..BLOCKS {
        let before = log.journal().records();
        let bytes_before = log.journal().byte_len();
        let started = Instant::now();
        for _ in 0..BLOCK {
            log.append(node(id)).expect("the append lands");
            id += 1;
        }
        let elapsed = started.elapsed();
        out.push(Block {
            before,
            elapsed,
            bytes: log.journal().byte_len() - bytes_before,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// 2 · The two durability tiers
// ---------------------------------------------------------------------------

/// One durability tier, measured over [`SYNCED`] appends.
#[derive(Debug, Clone, Copy)]
struct Tier {
    name: &'static str,
    elapsed: Duration,
}

impl Tier {
    fn micros_per_append(&self) -> f64 {
        self.elapsed.as_secs_f64() * 1e6 / SYNCED as f64
    }

    fn appends_per_frame(&self) -> f64 {
        FRAME_BUDGET_MS * 1e3 / self.micros_per_append()
    }
}

/// The shipping tier against the one the header rejected.
///
/// `Log::sync` is `File::sync_all`, which is `F_FULLFSYNC` on Apple platforms
/// and `fsync` elsewhere — the only call that survives the machine dying, and
/// the one this module deliberately keeps off the append path.
fn durability(dir: &Path) -> [Tier; 2] {
    let plain = {
        let (mut log, _recovery) = UndoLog::open(&dir.join("plain.undo")).expect("it opens");
        let started = Instant::now();
        for id in 1..=SYNCED {
            log.append(node(id)).expect("the append lands");
        }
        started.elapsed()
    };
    let synced = {
        let (mut log, _recovery) = UndoLog::open(&dir.join("synced.undo")).expect("it opens");
        let started = Instant::now();
        for id in 1..=SYNCED {
            log.append(node(id)).expect("the append lands");
            log.sync().expect("the fsync lands");
        }
        started.elapsed()
    };
    [
        Tier {
            name: "write_all only — what ships",
            elapsed: plain,
        },
        Tier {
            name: "write_all + fsync per group",
            elapsed: synced,
        },
    ]
}

// ---------------------------------------------------------------------------
// 3 · Compaction and startup, against session length
// ---------------------------------------------------------------------------

/// What a session did, beyond typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Nothing but committed groups.
    Typing,
    /// Groups, undos and writes — the records a snapshot can collapse.
    Walking,
}

/// One rung: build a session's log, fold it back, then compact it.
#[derive(Debug, Clone, Copy)]
struct Rung {
    groups: u64,
    records_before: u64,
    records_after: u64,
    bytes_before: u64,
    bytes_after: u64,
    /// `Log::open` — the fold every startup pays.
    open: Duration,
    /// `Log::compact` — the rewrite, its `fsync` and its `rename`.
    compact: Duration,
    /// Whether the doubling policy would have fired at this length.
    would_compact: bool,
}

impl Rung {
    /// Records the rewrite did not carry over. **The whole T095 question.**
    const fn reclaimed(&self) -> u64 {
        self.records_before.saturating_sub(self.records_after)
    }

    fn reclaimed_percent(&self) -> f64 {
        if self.records_before == 0 {
            return 0.0;
        }
        self.reclaimed() as f64 * 100.0 / self.records_before as f64
    }

    fn compact_millis(&self) -> f64 {
        self.compact.as_secs_f64() * 1e3
    }

    fn open_millis(&self) -> f64 {
        self.open.as_secs_f64() * 1e3
    }

    /// Cost per record rewritten. Flat across the ladder means O(n); climbing
    /// with the ladder means worse.
    fn compact_micros_per_record(&self) -> f64 {
        self.compact.as_secs_f64() * 1e6 / self.records_after.max(1) as f64
    }

    /// Cost per record folded. The same test, for the startup path.
    fn open_micros_per_record(&self) -> f64 {
        self.open.as_secs_f64() * 1e6 / self.records_before.max(1) as f64
    }

    /// How many frames the startup fold occupies.
    fn open_frames(&self) -> f64 {
        self.open_millis() / FRAME_BUDGET_MS
    }
}

fn rung(shape: Shape, groups: u64, dir: &Path) -> Rung {
    let path = dir.join(format!(
        "{}-{groups}.undo",
        match shape {
            Shape::Typing => "typing",
            Shape::Walking => "walking",
        }
    ));
    // A rung is a fresh file: a leftover from a previous run would be folded in
    // and every count below would be a lie about this session.
    let _ = fs::remove_file(&path);

    let records_before;
    let bytes_before;
    {
        let (mut log, _recovery) = UndoLog::open(&path).expect("a fresh journal opens");
        log.append(Record::Origin {
            path: format!("/phosphor/bench/{groups}.rs"),
        })
        .expect("the origin lands");

        // Node ids are dense and in creation order, so the group's number *is*
        // its id — `apply` rejects any other (`FoldError::OutOfOrder`).
        let mut current: NodeId = ROOT;
        for group in 1..=groups {
            log.append(branching(group, current))
                .expect("the group lands");
            current = group;
            if shape == Shape::Walking {
                if group.is_multiple_of(UNDO_EVERY) {
                    // An undo: back one step, which is what `Timeline::goto`
                    // journals. The next group then branches from there.
                    let to = parent_of(current);
                    log.append(Record::Cursor { to }).expect("the undo lands");
                    current = to;
                }
                if group.is_multiple_of(WRITE_EVERY) {
                    log.append(Record::Saved {
                        node: Some(current),
                    })
                    .expect("the write lands");
                }
            }
        }
        log.sync()
            .expect("the session is flushed before it is read back");
        records_before = log.journal().records();
        bytes_before = log.journal().byte_len();
    }

    // Reopened from cold, which is what a startup does: read every frame,
    // decode it, fold it.
    let started = Instant::now();
    let (mut log, _recovery) = UndoLog::open(&path).expect("the session folds back");
    let open = started.elapsed();
    assert_eq!(
        log.journal().records(),
        records_before,
        "the fold read back a different number of records than were written"
    );
    let would_compact = log.should_compact();

    let started = Instant::now();
    log.compact().expect("the compaction lands");
    let compact = started.elapsed();

    Rung {
        groups,
        records_before,
        records_after: log.journal().records(),
        bytes_before,
        bytes_after: log.journal().byte_len(),
        open,
        compact,
        would_compact,
    }
}

/// The parent of a node in the chain this bench builds, never below the root.
const fn parent_of(id: NodeId) -> NodeId {
    if id <= 1 { ROOT } else { id - 1 }
}

// ---------------------------------------------------------------------------
// 4 · When the doubling policy actually fires
// ---------------------------------------------------------------------------

/// A session length for the policy probe. Any length does; the answer is a
/// ratio, not a size.
const POLICY_GROUPS: u64 = 4_096;

/// What [`Log::should_compact`] asks of a session that has just started.
#[derive(Debug, Clone, Copy)]
struct Policy {
    /// Records already on disk when the file was opened.
    on_disk: u64,
    /// Groups appended after opening before the policy first said yes.
    appended: u64,
    /// Whether it ever said yes at all.
    fired: bool,
}

/// Reopens a session's log and counts the commit groups it takes before
/// [`Log::should_compact`] says yes.
///
/// **The policy is stated against the snapshot, and the snapshot is taken at
/// open.** `Log::open` sets its denominator to `state.snapshot().len()`, which
/// for an undo history is one record per node — so a freshly opened log is
/// already "as short as a snapshot would make it" and the policy is false at
/// the first keystroke of every session. It becomes true only after the session
/// has roughly doubled what it inherited, in that one process.
///
/// That is worth a number rather than a paragraph, because it decides what
/// `T095` is even for: if the policy cannot fire inside a normal session, then
/// wiring `compact-history` to it wires it to nothing.
fn policy_probe(path: &Path) -> Policy {
    let _ = fs::remove_file(path);
    {
        let (mut log, _recovery) = UndoLog::open(path).expect("a fresh journal opens");
        for group in 1..=POLICY_GROUPS {
            log.append(node(group)).expect("the group lands");
        }
        log.sync().expect("the session is flushed");
    }

    let (mut log, _recovery) = UndoLog::open(path).expect("the session folds back");
    let on_disk = log.journal().records();
    // A ceiling, not a guess: the policy is a doubling, so three times what is
    // on disk is past it by any reading. Reaching it without a `true` means the
    // policy is not a doubling any more, which is a finding rather than a hang.
    let ceiling = on_disk * 3 + 8;
    let mut appended = 0;
    let mut id = on_disk + 1;
    while !log.should_compact() && appended < ceiling {
        log.append(node(id)).expect("the group lands");
        id += 1;
        appended += 1;
    }
    Policy {
        on_disk,
        appended,
        fired: log.should_compact(),
    }
}

// ---------------------------------------------------------------------------
// The records
// ---------------------------------------------------------------------------

/// A committed group on a straight chain — the typing case.
fn node(id: NodeId) -> Record {
    branching(id, parent_of(id))
}

/// A committed group reached from `parent`.
///
/// One edit inserting [`GROUP_TEXT`], and a caret on both sides, which is
/// exactly what `main.rs`'s `journalled` builds from a `Change`.
fn branching(id: NodeId, parent: NodeId) -> Record {
    let at = id as usize * GROUP_TEXT.len();
    Record::Node {
        id,
        parent,
        edits: vec![Edit {
            at,
            removed: String::new(),
            inserted: GROUP_TEXT.to_owned(),
        }],
        before: Caret {
            offset: at,
            selection: None,
        },
        after: Caret {
            offset: at + GROUP_TEXT.len(),
            selection: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn append_table(blocks: &[Block]) {
    println!("append — one write_all per commit group, as the log grows");
    println!("    records in log    appends    µs/append    bytes/record    appends per frame");
    for block in blocks {
        println!(
            "  {:>16}    {:>7}    {:>9.2}    {:>12.1}    {:>17.0}",
            block.before,
            BLOCK,
            block.micros_per_append(),
            block.bytes_per_record(),
            block.appends_per_frame(),
        );
    }
    println!();
}

fn durability_table(tiers: &[Tier; 2]) {
    println!("durability — the two tiers journal.rs deliberately splits");
    println!("    tier                              µs/append    appends per frame");
    for tier in tiers {
        println!(
            "  {:<32}    {:>9.2}    {:>17.0}",
            tier.name,
            tier.micros_per_append(),
            tier.appends_per_frame(),
        );
    }
    println!(
        "  fsync costs {:.0}x an append, and it would land on every <esc>",
        tiers[1].micros_per_append() / tiers[0].micros_per_append().max(f64::MIN_POSITIVE),
    );
    println!();
}

fn compaction_table(title: &str, rungs: &[Rung]) {
    println!("{title}");
    println!(
        "    groups    records in    records out    reclaimed    KiB in    KiB out    ms/compact  \
         µs/record    policy"
    );
    for rung in rungs {
        println!(
            "  {:>8}    {:>10}    {:>11}    {:>8.1}%    {:>6.0}    {:>7.0}    {:>10.2}  {:>9.2}    \
             {}",
            rung.groups,
            rung.records_before,
            rung.records_after,
            rung.reclaimed_percent(),
            rung.bytes_before as f64 / 1024.0,
            rung.bytes_after as f64 / 1024.0,
            rung.compact_millis(),
            rung.compact_micros_per_record(),
            if rung.would_compact {
                "would fire"
            } else {
                "-"
            },
        );
    }
    println!();
}

fn startup_table(typing: &[Rung], walking: &[Rung]) {
    println!("startup — Log::open decodes and folds every record before the buffer is drawn");
    println!("    groups    typing ms    µs/record    frames    walking ms    µs/record    frames");
    for (a, b) in typing.iter().zip(walking) {
        println!(
            "  {:>8}    {:>9.2}    {:>9.2}    {:>6.1}    {:>10.2}    {:>9.2}    {:>6.1}",
            a.groups,
            a.open_millis(),
            a.open_micros_per_record(),
            a.open_frames(),
            b.open_millis(),
            b.open_micros_per_record(),
            b.open_frames(),
        );
    }
    println!();
}

/// The ratio of the largest value to the smallest — the shape test. A cost that
/// is O(1) in the ladder's parameter gives ~1; one that is O(n) gives the
/// ladder's own climb.
///
/// Undirected on purpose, because the assertions below only ask whether a cost
/// *moved*: a per-record cost that falls as the ladder climbs (page cache
/// warming, a fixed `fsync` amortised over more records) is not the O(n²) this
/// is watching for, and treating a fall as a climb would be the wrong alarm.
/// Where the *direction* is the finding — the startup fold — use [`growth`].
fn spread(values: &[f64]) -> f64 {
    let max = values.iter().copied().fold(f64::MIN, f64::max);
    let min = values.iter().copied().fold(f64::MAX, f64::min);
    max / min.max(f64::MIN_POSITIVE)
}

/// Last over first — a *directed* ratio. Above 1 the cost climbs with the
/// ladder, below 1 it falls.
///
/// The startup table needs this and [`spread`] will not do: typing's per-record
/// fold falls 5x across the ladder and walking's climbs 5x, and an undirected
/// ratio reports the same number for both.
fn growth(values: &[f64]) -> f64 {
    values[values.len() - 1] / values[0].max(f64::MIN_POSITIVE)
}

fn verdict(blocks: &[Block], tiers: &[Tier; 2], typing: &[Rung], walking: &[Rung], policy: Policy) {
    let append_costs: Vec<f64> = blocks.iter().map(Block::micros_per_append).collect();
    let append_spread = spread(&append_costs);
    // The ladder grows 16x; O(n^2) compaction would put ~16x into this number
    // and O(n) puts ~1x. 8 is the midpoint on a log scale, and it is the widest
    // a machine's noise has any business being.
    let compact_spread = spread(
        &typing
            .iter()
            .map(Rung::compact_micros_per_record)
            .collect::<Vec<_>>(),
    );
    let open_typing_growth = growth(
        &typing
            .iter()
            .map(Rung::open_micros_per_record)
            .collect::<Vec<_>>(),
    );
    let open_walking_growth = growth(
        &walking
            .iter()
            .map(Rung::open_micros_per_record)
            .collect::<Vec<_>>(),
    );

    let append_flat = append_spread < 8.0;
    let compact_linear = compact_spread < 8.0;
    // The two claims that are counts, not times.
    let typing_reclaims_nothing = typing.iter().all(|rung| rung.reclaimed_percent() < 5.0);
    let walking_reclaims = walking.iter().all(|rung| rung.reclaimed() > 0);

    let last_typing = typing.last().expect("the ladder is not empty");
    let last_walking = walking.last().expect("the ladder is not empty");
    let slowest_append = append_costs.iter().copied().fold(f64::MIN, f64::max);

    println!("verdict");
    println!(
        "  append           {:.2} µs at the head of the log, {:.2} µs {} records in ({:.1}x \
         spread — {})",
        append_costs[0],
        append_costs[append_costs.len() - 1],
        blocks[blocks.len() - 1].before,
        append_spread,
        if append_flat { "FLAT" } else { "CLIMBS" },
    );
    println!(
        "                   {:.0} commit groups fit in one frame. Act on this if it ever falls \
         below ~60,",
        FRAME_BUDGET_MS * 1e3 / slowest_append,
    );
    println!(
        "                   which is the point where one <esc> costs a measurable slice of a \
         frame."
    );
    println!(
        "  fsync per group  {:.0} µs — {:.0}x an append, {:.1}% of a frame budget each.",
        tiers[1].micros_per_append(),
        tiers[1].micros_per_append() / tiers[0].micros_per_append().max(f64::MIN_POSITIVE),
        tiers[1].micros_per_append() / (FRAME_BUDGET_MS * 1e3) * 100.0,
    );
    println!(
        "                   This is the number behind journal.rs's \"which is felt\". Keep sync() \
         off the append path."
    );
    println!(
        "  compaction       O(n): {:.2} µs/record at {} groups, {:.2} at {} ({:.1}x over a 16x \
         climb — {})",
        typing[0].compact_micros_per_record(),
        typing[0].groups,
        last_typing.compact_micros_per_record(),
        last_typing.groups,
        compact_spread,
        if compact_linear { "LINEAR" } else { "WORSE" },
    );
    println!(
        "                   and it costs {:.1} ms at {} groups — {:.1} frames, on whichever \
         keystroke trips the policy.",
        last_typing.compact_millis(),
        last_typing.groups,
        last_typing.compact_millis() / FRAME_BUDGET_MS,
    );
    println!();
    println!("  the T095 question, as counts rather than timings:");
    println!(
        "    typing   {} records in, {} out — the log gets {} record(s) LONGER. A snapshot emits",
        last_typing.records_before,
        last_typing.records_after,
        last_typing
            .records_after
            .saturating_sub(last_typing.records_before),
    );
    println!(
        "             one Record::Node per node and drops none, then adds its own Cursor and \
         Saved:"
    );
    println!(
        "             {:.0} KiB rewritten, {:.0} KiB saved, {:.1} ms spent.",
        last_typing.bytes_before as f64 / 1024.0,
        (last_typing
            .bytes_before
            .saturating_sub(last_typing.bytes_after)) as f64
            / 1024.0,
        last_typing.compact_millis(),
    );
    println!(
        "    walking  {} in, {} out — {:.1}% reclaimed. Compaction works; what it collapses is",
        last_walking.records_before,
        last_walking.records_after,
        last_walking.reclaimed_percent(),
    );
    println!("             Cursor and Saved churn, and there is no undo history without nodes.");
    println!(
        "    policy   should_compact() was {} on a log of {} records reopened cold, and needed {} \
         more",
        if policy.appended == 0 {
            "already true"
        } else {
            "false"
        },
        policy.on_disk,
        policy.appended,
    );
    println!(
        "             group(s) in that same process before it {}. Log::open takes its denominator",
        if policy.fired {
            "said yes"
        } else {
            "would say yes — it never did"
        },
    );
    println!(
        "             from snapshot().len(), so every session starts already as short as a \
         compaction"
    );
    println!(
        "             would make it. Wiring compact-history to this policy wires it to a session \
         that"
    );
    println!("             has doubled its own inherited history — not to a long one.");
    println!();
    println!(
        "  startup          {:.2} ms to fold {} typing records ({:.1} frames), {:.2} ms to fold \
         {} walking ones ({:.1} frames)",
        last_typing.open_millis(),
        last_typing.records_before,
        last_typing.open_frames(),
        last_walking.open_millis(),
        last_walking.records_before,
        last_walking.open_frames(),
    );
    println!(
        "                   per record, first rung to last: typing {:.2}x (FALLS), walking \
         {:.2}x ({}).",
        open_typing_growth,
        open_walking_growth,
        if open_walking_growth > 2.0 {
            "CLIMBS"
        } else {
            "flat"
        },
    );
    println!(
        "                   Record::Cursor's fold is History::walk_to, which re-points redo_child \
         from the"
    );
    println!(
        "                   target back to the root — O(depth) per undo, so a session's startup \
         is O(n·depth)"
    );
    println!(
        "                   in the undos it contains. Typing folds at a flat cost per record; \
         walking does not."
    );
    println!(
        "                   Act on it when opening a file costs a visible pause — say 100 ms, six \
         frames."
    );
    println!(
        "                   NOT asserted, deliberately: the day somebody makes walk_to \
         incremental this line"
    );
    println!(
        "                   improves, and a benchmark that fails when the code gets better is a \
         change detector."
    );
    println!();
    println!(
        "  B1: {}",
        if append_flat && compact_linear && typing_reclaims_nothing && walking_reclaims {
            "PASS — append is flat in log length, compaction is linear, and a typing session's \
             compaction reclaims nothing"
        } else {
            "FAIL — see the tables above"
        }
    );

    assert!(
        append_flat,
        "append cost tracked the length of the log it was appending to ({append_spread:.1}x over \
         an 8x growth); Journal::append seeks nothing and reads nothing, so this is a regression \
         in the append path, not in this benchmark"
    );
    assert!(
        compact_linear,
        "compaction cost per record climbed {compact_spread:.1}x over a 16x growth in session \
         length; snapshot() and rewrite() are both one pass, so this is superlinear where it \
         should not be"
    );
    assert!(
        typing_reclaims_nothing,
        "a typing session's compaction reclaimed records. undo::History::snapshot emits one \
         Record::Node per node and drops none, so it cannot — unless the snapshot changed, in \
         which case T095's argument changed with it and this line is the place to say so"
    );
    assert!(
        walking_reclaims,
        "a walking session's compaction reclaimed nothing either, so the line above measures a \
         compaction that does nothing at all rather than one with nothing to do"
    );
}

// ---------------------------------------------------------------------------
// The scratch directory
// ---------------------------------------------------------------------------

/// A directory under the system temp that removes itself.
///
/// Never `journal::state_home()`: a measurement must not leave undo journals in
/// the user's real `$XDG_STATE_HOME`, and `T091`'s benchmark makes the same
/// call for the same reason.
#[derive(Debug)]
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "phosphor-b1-{}-{}",
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
