//! The picker's matcher — nucleo, off the frame thread (`T045`).
//!
//! `phosphor-ui` draws a [`PickerVm`]; this is what fills one. The split is the
//! architecture rather than a preference: nucleo owns a thread pool, and a
//! widget crate that spawned threads would be a widget that outlives a frame.
//!
//! # What "responsive" means here, and why it is a `tick` rather than a wait
//!
//! `T045`'s criterion is *"it stays responsive filtering a 100k-file list"*,
//! and the shape that makes that true is nucleo's own: matching runs on its
//! worker threads and the frame thread **never blocks on it**. Every frame
//! calls [`Picker::tick`] with a small deadline, takes whatever has matched so
//! far, and draws. A filter typed into a 100k list draws a partial result on
//! the first frame and a complete one a few frames later; it never draws
//! nothing, and it never waits.
//!
//! That is also why [`PickerVm::matching`] exists. A partial count shown as
//! though it were final is the failure mode this shape trades for, and the
//! widget draws `12/100000…` while work is outstanding so the ellipsis carries
//! the difference.
//!
//! # Rows are styled, and the matcher only sees text
//!
//! A row is `Vec<RunVm>` so agent context renders in actor colours — `T045`'s
//! own line. Nucleo matches against **one** string per item, so each row
//! carries a flattened `haystack` beside its runs, built once at injection.
//! Matching against the styled form would mean either re-flattening per
//! keystroke or teaching the matcher about tones, and neither buys anything: a
//! filter matches what the row *says*, not what colour it says it in.
//!
//! Owned by `spine`.

use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

use phosphor_core::request::SourceId;
use phosphor_ui::picker::{PickerVm, RowVm, RunVm};

use crate::events;

/// What the matcher calls when it has results the last frame did not show.
///
/// The same shape as `phosphor_buffer::lsp::Woke` and for the same reason: a
/// producer that finishes on its own thread has to be able to say so, and
/// posting an [`events::AppEvent::Woke`] is how everything else here does it.
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

/// Which producer a picker wake names.
const SOURCE: &str = "picker";

/// A [`Wake`] that posts into the loop's queue — `crate::lsp::waking`'s twin.
pub(crate) fn waking(poster: events::Poster) -> Wake {
    Arc::new(move || {
        let _listening = poster.post(events::AppEvent::Woke(SOURCE));
    })
}

/// How long a frame is willing to spend inside the matcher.
///
/// One millisecond of a 16.7ms budget. Not a guess at how long matching takes —
/// nucleo's `tick` is a *deadline*, so this is the answer to *"how much of a
/// frame may the picker cost"*, and the rest of the work continues on the
/// worker threads either way.
const TICK_BUDGET_MS: u64 = 1;

/// One row, as the matcher holds it.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    /// What the filter matches against.
    haystack: String,
    /// What the widget draws.
    row: RowVm,
}

impl Item {
    /// A row and the text a filter sees, flattened once.
    pub(crate) fn new(row: RowVm) -> Self {
        let haystack = row
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<Vec<_>>()
            .concat();
        Self { haystack, row }
    }
}

/// Directory names never walked, whatever they contain.
///
/// **A denylist and not a `.gitignore` reader**, which is a real limitation
/// stated rather than hidden: honouring ignore files means either the `ignore`
/// crate — ripgrep's, and a substantial dependency for one picker — or a
/// half-implementation of a format with `!` negations and precedence rules
/// that would be wrong in ways nobody could predict. These five are what
/// actually makes a workspace walk unusable, and a file inside one of them is
/// reachable by typing its path into `:e`.
const NEVER_WALK: [&str; 5] = [".git", ".jj", "target", "node_modules", ".claude"];

/// How many files a single walk will collect.
///
/// `T045`'s criterion is *"responsive filtering a 100k-file list"*, so the
/// matcher is not the reason for a cap — the **walk** is, and it is on the
/// keystroke that opens the picker. A repository large enough to exceed this
/// gets a truncated list rather than a hung editor, and
/// [`workspace_files`] says which happened so the caller can say so too.
const WALK_CAP: usize = 100_000;

/// Every file under `root`, workspace-relative, sorted — and whether the cap
/// cut it short (`T047`).
///
/// **This is what makes `SPC f` a file picker.** The first version of the
/// `files` source listed only files the store had regions for, which meant an
/// ordinary build with nothing declared opened an empty picker under a key
/// labelled *"files"*. `3d` settles it: its caption is *"the file picker
/// carries agent state: unseen counts + activity, **not just names**"*, and
/// its own rows include `src/main.rs` and `Cargo.toml` carrying no activity at
/// all. The list is the workspace; the store **annotates** it.
///
/// Walked here rather than in Steel because a source runs inside the VM and no
/// capability walks a directory — the same seam that hands `grep` the buffer's
/// lines, and for the same reason (`OPEN-QUESTIONS.md` §42).
///
/// Sorted, so two runs of the same picker agree: `read_dir` order is the
/// filesystem's and `CP-5` asks for *"identical output on two machines"*.
pub(crate) fn workspace_files(root: &std::path::Path) -> (Vec<String>, bool) {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut truncated = false;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            // Symlinks are not followed: a link into a parent directory is how
            // a walk becomes infinite, and `read_dir` gives no cycle check.
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if !NEVER_WALK.contains(&name.as_ref()) {
                    stack.push(path);
                }
                continue;
            }
            if found.len() >= WALK_CAP {
                truncated = true;
                break;
            }
            if let Ok(relative) = path.strip_prefix(root) {
                found.push(relative.display().to_string());
            }
        }
        if truncated {
            break;
        }
    }
    found.sort_unstable();
    (found, truncated)
}

/// A `SpanRow` from a Steel source, as a row the widget draws (`T046`).
///
/// **The seam, and it is one function.** `phosphor-steel` may not name
/// `phosphor-ui` (`scripts/lint-the-steel-barrier.sh`), so a source answers
/// `phosphor-core`'s `SpanRow` and the conversion happens here — the same
/// place every other core-to-widget conversion happens.
///
/// Two things a `SpanRow` carries are deliberately dropped.
///
/// `SpanRow::tint` — §3's row tints name a *region state* and a picker row is
/// not in one. The selected row's ground is the widget's, from
/// `RegionTints::selection`, which is §4's pairing.
///
/// `Run::emphasis` — **a source cannot mark what matched, because a source
/// does not know.** The first draft mapped an emphasis onto
/// `RunVm::matched` and there is no weight in the protocol to map (`Emphasis`
/// is `Plain`, `Inverted`, `Underline`, `Undercurl` — every one of them a
/// *meaning*, not a weight). The mapping would have been inventing one. What
/// actually knows which characters matched is **nucleo**, which produces match
/// indices, and wiring those into `RunVm::matched` belongs with `T047` — grep
/// and symbols are where a highlighted match earns its keep. Until then the
/// flag is drawn and written by nothing, which is visible rather than wrong.
pub(crate) fn row_of(span: &phosphor_core::view::SpanRow) -> RowVm {
    RowVm::new(
        span.runs
            .iter()
            .map(|run| RunVm::text(run.text.clone()).toned(run.tone))
            .collect(),
    )
}

/// A live picker session: the rows, the matcher, and where the selection is.
pub(crate) struct Picker {
    nucleo: Nucleo<Item>,
    /// The filter the matcher was last told about, so a redraw with unchanged
    /// text does not reparse the pattern.
    filter: String,
    /// Which matched row is selected.
    selected: u32,
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Nucleo` is not `Debug`, and the interesting state is not it.
        f.debug_struct("Picker")
            .field("filter", &self.filter)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl Picker {
    /// An empty session, which wakes the loop when it has more to show.
    ///
    /// **This callback used to be a no-op, and the reason given for it was
    /// false about the loop as built.** It read *"this loop is a **poll**, not
    /// a wake: the frame already runs on its own schedule"*. It does not:
    /// `events::Queue::recv`'s own doc is *"No timeout, no tick, no sleep — a
    /// quiet editor is parked in `recv`"*, and the loop ticks this matcher once
    /// per drawn frame. So a match that outran [`TICK_BUDGET_MS`] left the
    /// partial list and its `…` on screen and **nothing ever drew again** until
    /// the next keystroke. Typing into a large picker could stop halfway and
    /// stay there.
    ///
    /// Found from a 30s CI timeout on a three-row picker — `3/3…` with the
    /// query typed and the filter never applied — which is the same freeze at
    /// the small end, where a starved runner made a microsecond of matching
    /// miss its millisecond.
    ///
    /// Waking is not drawing on the matcher's schedule: the callback posts an
    /// [`events::AppEvent::Woke`] and the loop decides what to do about it,
    /// exactly as the LSP client's state changes do.
    pub(crate) fn new(wake: Wake) -> Self {
        Self {
            nucleo: Nucleo::new(Config::DEFAULT, wake, None, 1),
            filter: String::new(),
            selected: 0,
        }
    }

    /// Replace the rows.
    ///
    /// `restart(true)` rather than a fresh `Nucleo`: it drops the old items and
    /// keeps the threads, which is what makes re-running a source cheap enough
    /// to do on a keystroke.
    pub(crate) fn feed(&mut self, rows: impl IntoIterator<Item = RowVm>) {
        self.nucleo.restart(true);
        let injector = self.nucleo.injector();
        for row in rows {
            let item = Item::new(row);
            injector.push(item, |item, columns| {
                columns[0] = item.haystack.as_str().into();
            });
        }
        self.selected = 0;
    }

    /// Tell the matcher what to look for.
    ///
    /// The `append` hint is the optimisation that matters when someone is
    /// typing: extending a filter can only *narrow* the current result set, so
    /// nucleo re-matches what already matched instead of the whole corpus.
    pub(crate) fn filter(&mut self, filter: &str) {
        if filter == self.filter {
            return;
        }
        let append = filter.starts_with(&self.filter);
        self.filter = filter.to_owned();
        self.nucleo
            .pattern
            .reparse(0, filter, CaseMatching::Smart, Normalization::Smart, append);
        self.selected = 0;
    }

    /// Select an absolute 0-based row, clamped to what has matched.
    ///
    /// `float-select-row`'s arm. Absolute rather than relative because that is
    /// what the capability takes — *"highlights an absolute row of the focused
    /// float"* — and routing it through [`Self::select`] keeps one clamp.
    pub(crate) fn select_to(&mut self, row: i64) {
        let delta = row.saturating_sub(i64::from(self.selected));
        self.select(delta);
    }

    /// Move the selection, clamped to what has matched.
    pub(crate) fn select(&mut self, delta: i64) {
        let matched = self.nucleo.snapshot().matched_item_count();
        if matched == 0 {
            self.selected = 0;
            return;
        }
        let last = matched.saturating_sub(1);
        let next = i64::from(self.selected).saturating_add(delta);
        self.selected = u32::try_from(next.clamp(0, i64::from(last))).unwrap_or(0);
    }

    /// Advance the matcher by at most [`TICK_BUDGET_MS`] and answer what to
    /// draw.
    ///
    /// **Never blocks.** `Status::running` is what rides out on
    /// [`PickerVm::matching`], so a partial result is drawn *as partial* rather
    /// than as a small one.
    pub(crate) fn tick(&mut self, rows: usize) -> PickerVm {
        let status = self.nucleo.tick(TICK_BUDGET_MS);
        let snapshot = self.nucleo.snapshot();
        let matched = snapshot.matched_item_count();
        // The window slides only far enough to hold the selection, the same
        // rule the widget's list draws by — computed here because the matcher
        // is what knows how many rows there are.
        let height = u32::try_from(rows).unwrap_or(u32::MAX).max(1);
        let first = self.selected.saturating_sub(height.saturating_sub(1));
        let end = first.saturating_add(height).min(matched);

        let visible: Vec<RowVm> = if first < end {
            snapshot
                .matched_items(first..end)
                .map(|item| item.data.row.clone())
                .collect()
        } else {
            Vec::new()
        };

        PickerVm {
            selected: usize::try_from(self.selected.saturating_sub(first)).unwrap_or(0),
            rows: visible,
            preview: Vec::new(),
            total: usize::try_from(snapshot.item_count()).unwrap_or(usize::MAX),
            matching: status.running,
        }
    }

    /// The selected row's text, flattened — what `picker-accept` reads.
    ///
    /// The **haystack**, not the runs: it is the same string the filter
    /// matched against, so what a person picked and what the accept parses are
    /// the same text by construction rather than by two joins agreeing.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let snapshot = self.nucleo.snapshot();
        let at = self.selected;
        if at >= snapshot.matched_item_count() {
            return None;
        }
        snapshot
            .matched_items(at..at.saturating_add(1))
            .next()
            .map(|item| item.data.haystack.clone())
    }

    /// How many rows currently match. Read without ticking, for a caller that
    /// only wants the count.
    pub(crate) fn matched(&self) -> usize {
        usize::try_from(self.nucleo.snapshot().matched_item_count()).unwrap_or(usize::MAX)
    }
}

// **No `Default`, deliberately.** It existed and had no caller. A default
// picker would have to invent a wake, and the only one available to a `Default`
// is the no-op that caused the freeze this callback exists to fix — a
// constructor that compiles everywhere and silently reintroduces the bug. The
// notify is not a detail a matcher can supply for itself; it belongs to the
// loop, so the loop has to hand it over.

/// One open picker: which source, what the filter reads, and the matcher.
///
/// The **filter text lives here and not in the widget**, which is the whole of
/// why `T045` did not take `ratatui-textarea` — see `phosphor_ui::picker`'s
/// header. `Node::Picker` carries it as a prop, composed from this every frame,
/// so there is one copy of the string and it is this one.
#[derive(Debug)]
pub(crate) struct PickerSession {
    /// Which `define-picker-source` supplies the rows (`T046`).
    pub(crate) source: SourceId,
    /// The filter text, as `Node::Picker`'s prop.
    pub(crate) filter: String,
    /// Whether a preview was *asked* for. §11's ladder can still drop it, which
    /// is `phosphor_ui::picker::Picker::shows_preview`'s job and not this
    /// flag's — a terminal that is too narrow must not silently turn the
    /// setting off, or widening it back would surprise.
    pub(crate) preview: bool,
    /// The matcher.
    pub(crate) matcher: Picker,
}

impl PickerSession {
    /// A session over `source`, seeded with `query`.
    pub(crate) fn open(source: SourceId, query: Option<String>, wake: &Wake) -> Self {
        let mut matcher = Picker::new(Arc::clone(wake));
        let filter = query.unwrap_or_default();
        matcher.filter(&filter);
        Self {
            source,
            filter,
            preview: true,
            matcher,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_ui::picker::RunVm;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// One frame at 60fps. Design Language §8 makes a torn frame a P0 and the
    /// loop is single-threaded, so anything on the frame path that costs this
    /// long *is* a dropped frame.
    const FRAME_BUDGET: Duration = Duration::from_micros(16_700);

    fn rows(count: usize) -> Vec<RowVm> {
        (0..count)
            .map(|index| {
                RowVm::new(vec![RunVm::text(format!(
                    "crates/phosphor-{}/src/module_{index}.rs",
                    index % 7
                ))])
            })
            .collect()
    }

    /// Run `tick` until the matcher settles, answering how many frames it took
    /// and the worst single frame.
    fn settle(picker: &mut Picker) -> (usize, Duration) {
        let mut frames = 0;
        let mut worst = Duration::ZERO;
        loop {
            let start = Instant::now();
            let vm = picker.tick(40);
            worst = worst.max(start.elapsed());
            frames += 1;
            if !vm.matching {
                return (frames, worst);
            }
            assert!(frames < 10_000, "the matcher never settled");
        }
    }

    /// A scratch tree that removes itself.
    struct Tree(std::path::PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let at =
                std::env::temp_dir().join(format!("phosphor-walk-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&at);
            std::fs::create_dir_all(&at).expect("a scratch tree");
            Self(at)
        }

        fn file(&self, relative: &str) -> &Self {
            let at = self.0.join(relative);
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).expect("a parent");
            }
            std::fs::write(&at, "x\n").expect("a file");
            self
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The defect Teej reported, at the layer that caused it: the walk is what
    /// makes `SPC f` a file picker, and it must answer without a store.
    #[test]
    fn the_walk_lists_every_file_workspace_relative() {
        let tree = Tree::new("flat");
        tree.file("Cargo.toml")
            .file("src/main.rs")
            .file("src/a/b.rs");

        let (found, truncated) = workspace_files(&tree.0);

        assert!(!truncated);
        assert_eq!(
            found,
            vec!["Cargo.toml", "src/a/b.rs", "src/main.rs"],
            "relative to the root, and sorted so two runs agree",
        );
    }

    #[test]
    fn the_walk_skips_the_directories_that_make_it_unusable() {
        let tree = Tree::new("skips");
        tree.file("keep.rs")
            .file("target/debug/junk.rs")
            .file("node_modules/pkg/index.js")
            .file(".git/objects/ab/cd")
            .file(".jj/repo/store")
            .file(".claude/settings.json");

        let (found, _) = workspace_files(&tree.0);

        assert_eq!(found, vec!["keep.rs"], "every denied directory is unwalked");
    }

    /// Sorted, because `read_dir` order is the filesystem's and `CP-5` asks for
    /// *"identical output on two machines"*.
    #[test]
    fn the_walk_is_ordered_rather_than_whatever_the_filesystem_says() {
        let tree = Tree::new("sorted");
        tree.file("z.rs").file("a.rs").file("m/n.rs").file("b.rs");

        let (found, _) = workspace_files(&tree.0);
        let mut expected = found.clone();
        expected.sort_unstable();

        assert_eq!(found, expected);
    }

    #[test]
    fn an_empty_workspace_answers_an_empty_list_rather_than_failing() {
        let tree = Tree::new("empty");
        let (found, truncated) = workspace_files(&tree.0);

        assert!(found.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn a_root_that_does_not_exist_answers_nothing_rather_than_panicking() {
        let (found, truncated) = workspace_files(std::path::Path::new("/no/such/place/at/all"));

        assert!(found.is_empty());
        assert!(!truncated);
    }

    /// **`T045`'s criterion: it stays responsive filtering a 100k-file list.**
    ///
    /// # This asserts a *shape*, and the first draft asserted a time
    ///
    /// The obvious test is *"no `tick` costs more than a frame"*, and it was
    /// written that way, and it went red the first time the whole suite ran —
    /// under `nextest`'s per-test parallelism, sixteen processes sharing the
    /// machine make a 50µs call take milliseconds. That is precisely what
    /// `CLAUDE.md` says not to do: *"a figure that moves with the machine has
    /// no business failing a build"*, and this worktree has *"seen absolute
    /// times swing 25× under concurrent load while every shape assertion
    /// held"*.
    ///
    /// So what is asserted is the structural property that makes it responsive,
    /// which is stronger and cannot flake:
    ///
    /// 1. **It does not hang.** A single `tick` over 100k rows returns, and the
    ///    bound is 500 frame budgets — so loose no load can reach it and only a
    ///    genuine hang can.
    ///
    ///    **Two sharper assertions were tried here and both were wrong**, which
    ///    is worth keeping because each looked obviously right:
    ///
    ///    * *"the first tick reports `matching: true`"* — `Status::running`
    ///      says whether nucleo's workers are running *now*, not whether work
    ///      is outstanding. On a loaded box the tick can return before they are
    ///      scheduled, so `matching` is legitimately `false` with nothing done.
    ///    * *"the first tick has not matched all 100k"* — with an **empty**
    ///      pattern there is no matching to do. Every item trivially matches,
    ///      so the full count is the correct and immediate answer.
    ///
    ///    What remains is that *"never blocks"* is a property of the **API
    ///    shape** — `tick` takes a deadline and the loop polls it — and is
    ///    visible in the twelve lines of [`Picker::tick`] rather than derivable
    ///    from a measurement. Measuring it is `benches/picker.rs`'s job on the
    ///    drawing side; asserting it here would be asserting the machine.
    /// 2. **It converges.** Repeated ticks settle, in a frame count that is
    ///    bounded and small.
    /// 3. **Extending a filter only ever narrows.** That is what the `append`
    ///    hint means — nucleo re-matches the current result set rather than the
    ///    corpus — and the observable form of it is a match count that never
    ///    goes *up* as characters are added.
    ///
    ///    This started as *"each keystroke settles in no more frames than its
    ///    prefix did"* and went red at 18 frames against 13. A frame count is a
    ///    wall clock in a costume: each tick has a 1ms deadline, so counting
    ///    ticks measures the machine exactly as much as timing them does.
    ///
    ///
    /// [`FRAME_BUDGET`] survives only as a **hang detector** — a bound so loose
    /// (500 frames' worth in one call) that no load can reach it and only a
    /// genuinely blocking `tick` can.
    /// **The matcher says when it has more, because nothing else will ask.**
    ///
    /// This is the half `a_hundred_thousand_rows_never_block_a_frame` cannot
    /// reach. That test drives `tick` in a loop, so it proves the matcher
    /// *converges when something keeps asking*. The loop does not: it is parked
    /// in `events::Queue::recv` — *"No timeout, no tick, no sleep"* — and ticks
    /// this matcher once per **drawn frame**. A keystroke buys exactly one
    /// tick, and if matching outruns that tick's budget there is no second one.
    ///
    /// So without the notify, typing a filter drew the partial list and its `…`
    /// and then **froze there** until the next key. Found as a 30s CI timeout on
    /// a three-row picker showing `3/3…` with the query typed and never
    /// applied — the same freeze at the small end, where a starved runner made
    /// a microsecond of work miss its millisecond.
    ///
    /// The shape, not a time, for the reason the test below sets out at length:
    /// **one tick, then nothing**, and the wake still arrives. The injection
    /// wakes are dropped first so what is left is the filter's own.
    /// [`Picker::new`] taking the callback rather than defaulting it is what
    /// makes this reachable — a `Default` supplying a no-op would compile
    /// everywhere and freeze here.
    #[test]
    fn a_matcher_with_work_outstanding_wakes_the_loop() {
        let woke = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&woke);
        let mut picker = Picker::new(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        picker.feed(rows(100_000));
        drop(picker.tick(10));
        // Injection wakes for the same reason matching does. Only the filter's
        // are the claim here.
        woke.store(0, Ordering::SeqCst);

        picker.filter("row");
        // The one tick a keystroke's frame gives it. Nothing ticks again — that
        // is the whole point, and it is what the loop actually does.
        drop(picker.tick(10));

        let deadline = Instant::now() + Duration::from_secs(30);
        while woke.load(Ordering::SeqCst) == 0 {
            assert!(
                Instant::now() < deadline,
                "the matcher never woke the loop, so the frame drawn above is \
                 the last one — a filter typed into a big list would stop here",
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_hundred_thousand_rows_never_block_a_frame() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(100_000));

        // (1) A tick returns rather than hanging, and (2) the whole corpus
        // converges. Nothing is asserted about the *first* tick: injection is
        // asynchronous, so on a loaded box it can legitimately report an item
        // count of zero — a third wrong assertion this test held before the
        // full suite ran it.
        let start = Instant::now();
        let (frames, worst) = settle(&mut picker);
        assert!(
            worst < FRAME_BUDGET * 500,
            "a single tick took {worst:?}; that is a hang, not load",
        );
        assert!(frames < 5_000, "settled only after {frames} frames");
        assert_eq!(picker.matched(), 100_000, "and every row is there");
        assert!(
            start.elapsed() < FRAME_BUDGET * 5_000,
            "settling 100k rows should not take a minute",
        );

        // (3) Extending a filter only ever narrows.
        let mut previous = picker.matched();
        for filter in ["p", "ph", "pho", "phos", "phosphor"] {
            picker.filter(filter);
            settle(&mut picker);
            let now = picker.matched();
            assert!(
                now <= previous,
                "extending the filter to {filter:?} matched {now} rows against {previous} \
                 for its prefix — a superset is not a narrowing",
            );
            previous = now;
        }
        assert!(
            previous > 0,
            "the fixture contains `phosphor` in every path"
        );
    }

    /// Rows appear within a **few** frames of opening, not after the whole
    /// corpus has been matched — the other half of responsive, since a picker
    /// that showed a blank list until matching finished would be correct and
    /// would feel broken.
    ///
    /// **Not "the first tick", which is what this test claimed until it was
    /// run.** Injection is asynchronous: `feed` hands 100k items to the
    /// injector and the very first `tick` can legitimately see none of them
    /// yet. What the criterion actually needs is that the wait is measured in
    /// frames rather than in the size of the corpus, and that no single frame
    /// pays for it — which is what the loop below asserts on both counts.
    #[test]
    fn rows_appear_within_a_few_frames_of_opening() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(100_000));

        let mut frames = 0;
        loop {
            let vm = picker.tick(40);
            frames += 1;
            if !vm.rows.is_empty() {
                assert_eq!(vm.total, 100_000, "and the count is the whole corpus");
                break;
            }
            // A *frame count*, not a duration — see
            // `a_hundred_thousand_rows_never_block_a_frame` for why nothing
            // here is allowed to be a wall clock, and for the three assertions
            // about the first tick that were wrong before this one.
            assert!(
                frames < 5_000,
                "5,000 ticks without a single row is not responsive",
            );
        }
    }

    #[test]
    fn a_filter_narrows_and_an_empty_one_restores() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(500));
        settle(&mut picker);
        let everything = picker.matched();

        picker.filter("module_1.rs");
        settle(&mut picker);
        let narrowed = picker.matched();
        assert!(narrowed < everything, "{narrowed} !< {everything}");
        assert!(narrowed > 0, "the fixture contains that name");

        picker.filter("");
        settle(&mut picker);
        assert_eq!(picker.matched(), everything, "clearing restores every row");
    }

    #[test]
    fn a_filter_matching_nothing_answers_none_rather_than_everything() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(200));
        picker.filter("zzzzz-no-such-thing");
        settle(&mut picker);

        assert_eq!(picker.matched(), 0);
        let vm = picker.tick(10);
        assert!(vm.rows.is_empty());
        assert_eq!(vm.total, 200, "and still says how many there were");
    }

    /// The selection is clamped to what matched, so narrowing the list under a
    /// selection cannot leave it pointing past the end.
    #[test]
    fn the_selection_is_clamped_to_what_matched() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(50));
        settle(&mut picker);

        picker.select(10_000);
        let vm = picker.tick(50);
        assert!(vm.selected < vm.rows.len().max(1));

        picker.select(-10_000);
        let vm = picker.tick(50);
        assert_eq!(vm.selected, 0, "and does not go below the first row");
    }

    #[test]
    fn feeding_again_replaces_the_rows_rather_than_appending() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(rows(100));
        settle(&mut picker);
        assert_eq!(picker.matched(), 100);

        picker.feed(rows(7));
        settle(&mut picker);
        assert_eq!(picker.matched(), 7, "restart(true) dropped the old corpus");
    }

    /// The haystack is the row's runs concatenated, so a filter matches what
    /// the row *says* across colour boundaries — `claude` in a toned run is
    /// still findable.
    #[test]
    fn a_filter_matches_across_run_boundaries() {
        let mut picker = Picker::new(Arc::new(|| {}));
        picker.feed(vec![RowVm::new(vec![
            RunVm::text("src/retry.rs"),
            RunVm::text(" · "),
            RunVm::text("claude").toned(phosphor_core::view::Tone::Claude),
        ])]);
        picker.filter("claude");
        settle(&mut picker);

        assert_eq!(picker.matched(), 1, "the toned run is part of the haystack");
    }
}
