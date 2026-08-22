//! `TranscriptPane` (`T054`) — screen `1b`, the session stream as a pane.
//!
//! Draws [`phosphor_core::view::Node::Transcript`]: a header, then one block per
//! turn — the prompt line, claude's prose, the tool rows, and how the turn
//! ended. `1b`'s caption is *"session stream as a pane · tool-call folds jump to
//! files · closes back to full buffer"*, and the first three words are the whole
//! contract: **a pane, not a float.** Design Language §9 is why — a float is
//! *"in front of"* something and dims it, and a transcript you are reading
//! beside your code is not in front of it.
//!
//! # What is a prop and what is a door
//!
//! The node carries `follow` and `folded` and nothing else, so the *turns* come
//! through [`Resources::transcript`](crate::interpret::Resources::transcript) —
//! the same division `Node::Picker` already draws. Composition decides whether
//! the pane is on screen and which turns are collapsed; the host decides what
//! the session actually said. A widget crate cannot read a session.
//!
//! # `1b`, read out
//!
//! ```text
//! claude code · acp · 4f2a
//! ❯ add retry with exponential backoff to the fetch layer
//! Adding a RetryPolicy struct and a generic retry_with_backoff helper, then …
//! ▸ edit  src/retry.rs                                            +42 −0
//! ▸ edit  src/fetch.rs                                             +9 −3
//! ▸ bash  cargo test                                        ✓ 34 passed
//! ✻ review ready · retry logic — 2 files, 6 regions
//! ```
//!
//! Every row is one of five kinds and each has one tone: the header is meta,
//! `❯` and the prompt are yours (§1's you-blue), prose is
//! [`prose`](crate::theme::NeutralRamp::prose) — §6 is explicit that *"his prose
//! is `#9aa39a`; facts he produced (diffs, counts) are colored data, not
//! prose"* — a tool row's verb is claude's green with a meta target, and the
//! counts are `+`/`−` in the you-blue and trouble-red the diff already uses.
//!
//! # Streaming, and the two kinds that were waiting for it
//!
//! §8 allows exactly three animations, and two of them are this surface's while
//! a turn is running: the braille spinner and the elapsed counter. They are
//! composed **as nodes** here rather than drawn inline, which is what
//! `scripts/lint-node-kinds.sh` recorded `Node::Spinner` and `Node::Elapsed`
//! against this task for — the statusline reaches them through
//! `Node::Session`'s own arm, and a surface that shows progress *without* a
//! session segment is the one that needs them standalone.
//!
//! # Scale is grouping, not scrolling (§11)
//!
//! A transcript is the design's own example of that rule — *"transcripts by
//! turn"* — so a pane too short for the whole session shows the **newest** turns
//! whole and says how many it dropped, rather than showing every turn cut in
//! half. `follow` is what makes the newest end the one that is kept.
//!
//! Owned by `surface`.

use phosphor_core::request::{ToolCallId, TurnId};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// §2's glyphs, as this surface spends them.
mod glyph {
    /// `❯` — a prompt line. Yours, not claude's.
    pub(super) const PROMPT: &str = "❯";
    /// `▸` — a folded tool row. `1b` draws every one of them closed.
    pub(super) const FOLDED: &str = "▸";
    /// `▾` — an open one.
    pub(super) const OPEN: &str = "▾";
    /// `✻` — claude, and anything he produced. The seam marker's own glyph.
    pub(super) const CLAUDE: &str = "✻";
    /// `⋯` — turns that did not fit (§11's drop, made visible).
    ///
    /// §2's `✓` is deliberately **not** here: `1b` draws `✓ 34 passed` as a
    /// tool call's summary, and a summary is one line the *host* wrote. A
    /// constant here would be this widget deciding what a successful `bash` row
    /// says, which is the agent's sentence and not the surface's.
    pub(super) const ELIDED: &str = "⋯";
}

/// Cells of air at the left of every row. `1b` insets the whole pane.
pub const PAD_COLS: u16 = 1;

/// Cells between a tool row's verb and its target.
const GAP: u16 = 2;

/// One tool call, as a transcript row (`T054`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// The call's id, so a fold and a jump address the same row.
    pub id: ToolCallId,
    /// What it is doing — `read`, `edit`, `bash`.
    pub verb: String,
    /// What it is doing it to. `1b` draws a path for `edit` and a command for
    /// `bash`, which is the same field wearing two hats and is why it is text.
    pub target: Option<String>,
    /// Progress lines, newest last. Drawn only when the row is open.
    pub notes: Vec<String>,
    /// How it ended, or [`None`] while it is still running.
    pub outcome: Option<Outcome>,
}

/// How a tool call ended — the right-hand end of a `1b` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// One line: `✓ 34 passed`, or a file's name.
    pub summary: String,
    /// Lines added, drawn `+42` in the you-blue.
    pub added: u32,
    /// Lines removed, drawn `−0` in trouble-red.
    pub removed: u32,
}

/// One turn (`T054`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// Which turn — what a fold is keyed by, and what `1b`'s rows group under.
    pub id: TurnId,
    /// What started it. [`None`] for a turn the agent began on its own.
    pub prompt: Option<String>,
    /// Claude's prose, accumulated. Streams during Working, which is why it is
    /// one string rather than a list of chunks: a chunk boundary is a fact
    /// about the wire and not about the paragraph.
    pub prose: String,
    /// The tool calls, in the order they started.
    pub calls: Vec<ToolCall>,
    /// How the turn ended, or [`None`] while it is running.
    pub ended: Option<String>,
    /// When it began, for the elapsed counter. Absent for a turn read back
    /// from a log rather than watched.
    pub since: Option<phosphor_core::view::Millis>,
}

/// What the transcript pane draws (`T054`).
///
/// A ViewModel: derived from the session, read-only, rebuilt when it moves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptVm {
    /// `1b`'s header — `claude code · acp · 4f2a`. Empty draws no header,
    /// which is the honest thing with no session.
    pub header: String,
    /// The turns, oldest first.
    pub turns: Vec<Turn>,
}

impl phosphor_core::vm::ViewModel for TranscriptVm {}

/// The transcript, as a pane.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptPane<'a> {
    vm: &'a TranscriptVm,
    theme: &'a Theme,
    follow: bool,
    folded: &'a [TurnId],
    now: phosphor_core::view::Millis,
}

impl<'a> TranscriptPane<'a> {
    /// A pane over `vm`.
    #[must_use]
    pub const fn new(vm: &'a TranscriptVm, theme: &'a Theme) -> Self {
        Self {
            vm,
            theme,
            follow: true,
            folded: &[],
            now: phosphor_core::view::Millis(0),
        }
    }

    /// Whether the newest turn is the one held on screen (`Node::Transcript`).
    #[must_use]
    pub const fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    /// Which turns are collapsed to their prompt line.
    #[must_use]
    pub const fn folded(mut self, folded: &'a [TurnId]) -> Self {
        self.folded = folded;
        self
    }

    /// This frame's clock, for the spinner and the elapsed counter.
    #[must_use]
    pub const fn at(mut self, now: phosphor_core::view::Millis) -> Self {
        self.now = now;
        self
    }

    /// The rows one turn draws, in order.
    ///
    /// Built as data rather than painted directly, because §11's grouping rule
    /// needs to know how tall a turn is *before* deciding which turns fit — and
    /// a widget that measured by drawing would have to draw twice.
    fn rows(&self, turn: &Turn) -> Vec<Row> {
        let mut rows = Vec::new();
        let folded = self.folded.contains(&turn.id);
        if let Some(prompt) = &turn.prompt {
            rows.push(Row::Prompt(prompt.clone(), folded));
        }
        if folded {
            return rows;
        }
        // **Wrapped, not truncated.** Prose is the one thing on this surface
        // that is a paragraph rather than a datum, and §5's *"never wraps"* is
        // a rule about the statusline. A transcript that clipped claude's
        // sentences at the pane edge would be unreadable at any width.
        for line in turn.prose.lines() {
            rows.push(Row::Prose(line.to_owned()));
        }
        for call in &turn.calls {
            rows.push(Row::Call(call.clone()));
            for note in &call.notes {
                rows.push(Row::Note(note.clone()));
            }
        }
        match (&turn.ended, turn.since) {
            (Some(seam), _) => rows.push(Row::Seam(seam.clone())),
            // §8's two animations, and the reason this surface composes them:
            // a turn that is still going says so where it is happening.
            (None, Some(since)) => rows.push(Row::Running(since)),
            (None, None) => {}
        }
        rows
    }
}

/// One drawn row of the transcript.
#[derive(Debug, Clone)]
enum Row {
    /// `❯ add retry with exponential backoff …`, and whether its turn is folded.
    Prompt(String, bool),
    /// One line of claude's prose.
    Prose(String),
    /// `▸ edit  src/retry.rs                     +42 −0`.
    Call(ToolCall),
    /// A progress line under an open call.
    Note(String),
    /// `✻ review ready · retry logic — 2 files, 6 regions`.
    Seam(String),
    /// The spinner and elapsed counter of a turn still running.
    Running(phosphor_core::view::Millis),
}

impl Widget for TranscriptPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let meta = Style::new().fg(self.theme.neutrals.meta);

        let mut y = area.y;
        if !self.vm.header.is_empty() {
            write(buf, area, area.x + PAD_COLS, y, &self.vm.header, meta);
            y += 1;
        }

        // **§11: scale is grouping, not scrolling.** The turns that fit are
        // kept whole from the newest end, and the count of the rest is one row
        // — the same shape `key_hints`' `Density::Help` body uses, and the same
        // reason: a list that stopped mid-turn would read as a transcript that
        // ended there.
        let room = usize::from(area.bottom().saturating_sub(y));
        let blocks: Vec<Vec<Row>> = self.vm.turns.iter().map(|turn| self.rows(turn)).collect();
        let (from, dropped) = fit(&blocks, room, self.follow);

        if dropped > 0 && y < area.bottom() {
            write(
                buf,
                area,
                area.x + PAD_COLS,
                y,
                &format!("{} {dropped} earlier turn(s)", glyph::ELIDED),
                meta,
            );
            y += 1;
        }
        for block in blocks.iter().skip(from) {
            for row in block {
                if y >= area.bottom() {
                    return;
                }
                self.row(row, area, y, buf);
                y += 1;
            }
        }
    }
}

impl TranscriptPane<'_> {
    /// One row, in its own tones.
    fn row(&self, row: &Row, area: Rect, y: u16, buf: &mut Buffer) {
        let x = area.x + PAD_COLS;
        match row {
            // §1's you-blue: a prompt is yours.
            Row::Prompt(text, folded) => {
                let mark = if *folded {
                    glyph::FOLDED
                } else {
                    glyph::PROMPT
                };
                let after = write(
                    buf,
                    area,
                    x,
                    y,
                    mark,
                    Style::new().fg(self.theme.actors.you),
                );
                write(
                    buf,
                    area,
                    after + 1,
                    y,
                    text,
                    Style::new().fg(self.theme.neutrals.text),
                );
            }
            // §6: *"his prose is `#9aa39a`"*, as distinct from the facts he
            // produced.
            Row::Prose(text) => {
                write(
                    buf,
                    area,
                    x,
                    y,
                    text,
                    Style::new().fg(self.theme.neutrals.prose),
                );
            }
            Row::Call(call) => self.call(call, area, y, buf),
            Row::Note(note) => {
                write(
                    buf,
                    area,
                    x + GAP,
                    y,
                    note,
                    Style::new().fg(self.theme.neutrals.meta),
                );
            }
            // The seam is claude's, glyph and all.
            Row::Seam(text) => {
                let after = write(
                    buf,
                    area,
                    x,
                    y,
                    glyph::CLAUDE,
                    Style::new().fg(self.theme.actors.claude),
                );
                write(
                    buf,
                    area,
                    after + 1,
                    y,
                    text,
                    Style::new().fg(self.theme.actors.claude),
                );
            }
            Row::Running(since) => {
                let elapsed = self.now.0.saturating_sub(since.0);
                // The same eighty-millisecond cadence §8 allows and
                // `Interpreter::session` already spins the statusline at —
                // reached through the same `Spinner`, so the two cannot drift
                // into two rhythms.
                let frame = elapsed / crate::interpret::SPINNER_PERIOD_MS;
                let spun = crate::status_line::Spinner(
                    u8::try_from(frame % crate::status_line::Spinner::FRAMES.len() as u64)
                        .unwrap_or(0),
                );
                let after = write(
                    buf,
                    area,
                    x,
                    y,
                    spun.glyph(),
                    Style::new().fg(self.theme.actors.transient),
                );
                write(
                    buf,
                    area,
                    after + 1,
                    y,
                    &crate::status_line::format_elapsed(std::time::Duration::from_millis(elapsed)),
                    Style::new().fg(self.theme.neutrals.meta),
                );
            }
        }
    }

    /// `▸ edit  src/retry.rs                                    +42 −0`.
    ///
    /// The counts are right-aligned, which is what `1b` draws and what makes a
    /// column of them readable: a reader scanning for *"what changed a lot"*
    /// reads down the right edge.
    fn call(&self, call: &ToolCall, area: Rect, y: u16, buf: &mut Buffer) {
        let x = area.x + PAD_COLS;
        let open = !call.notes.is_empty();
        let mark = if open { glyph::OPEN } else { glyph::FOLDED };
        let after = write(
            buf,
            area,
            x,
            y,
            mark,
            Style::new().fg(self.theme.neutrals.meta),
        );
        let after = write(
            buf,
            area,
            after + 1,
            y,
            &call.verb,
            Style::new().fg(self.theme.actors.claude),
        );
        if let Some(target) = &call.target {
            write(
                buf,
                area,
                after + GAP,
                y,
                target,
                Style::new().fg(self.theme.neutrals.text),
            );
        }
        let Some(outcome) = &call.outcome else {
            return;
        };
        // Right-aligned, built as one string so the measurement and the write
        // cannot disagree about its width.
        let counts = if outcome.added == 0 && outcome.removed == 0 {
            String::new()
        } else {
            format!("+{} −{}", outcome.added, outcome.removed)
        };
        let tail = if counts.is_empty() {
            outcome.summary.clone()
        } else if outcome.summary.is_empty() {
            counts
        } else {
            format!("{}  {counts}", outcome.summary)
        };
        if tail.is_empty() {
            return;
        }
        let at = area
            .right()
            .saturating_sub(PAD_COLS)
            .saturating_sub(cells(&tail));
        write(
            buf,
            area,
            at.max(after + GAP),
            y,
            &tail,
            Style::new().fg(self.theme.neutrals.meta),
        );
    }
}

/// Which turn to start at, and how many were dropped.
///
/// §11's grouping applied to a list of *blocks*: whole turns, from the end that
/// `follow` says matters. A single turn taller than the pane is still started —
/// there is no rung below *"show the turn you are watching"*, exactly as
/// `tab_bar`'s ladder has none below *"show the active tab"*.
fn fit(blocks: &[Vec<Row>], room: usize, follow: bool) -> (usize, usize) {
    if blocks.is_empty() || room == 0 {
        return (0, 0);
    }
    if !follow {
        return (0, 0);
    }
    // One row of the budget buys the truth about the rest, and only when there
    // is a rest.
    let mut used = 0;
    let mut from = blocks.len();
    for (index, block) in blocks.iter().enumerate().rev() {
        let would = used + block.len();
        if would > room && index + 1 < blocks.len() {
            break;
        }
        used = would;
        from = index;
    }
    (from, from)
}

/// Write `text` at `(x, y)`, clipped to `area`. Answers the column after the
/// last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, y, text, room as usize, style);
    next.min(area.right())
}
