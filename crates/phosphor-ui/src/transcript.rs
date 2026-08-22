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

use core::num::NonZeroU16;
use phosphor_core::request::{ToolCallId, TurnId};
use phosphor_core::view::Density;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

use ratatui_core::buffer::CellDiffOption;
use ratatui_core::style::{Modifier, Style};
use ratatui_core::text::Line;
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
    /// `✕` — §2's trouble glyph, which a seam wears when the turn did not end
    /// so much as stop. `7b` draws `✕ connection lost mid-turn`.
    pub(super) const TROUBLE: &str = "✕";
    /// `⏸` — §2's paused glyph. `7e` draws `⏸ paused at tool boundary`, and it
    /// is amber rather than red because a pause is a thing *you* did.
    pub(super) const PAUSED: &str = "⏸";
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
    /// Where clicking this row goes — a `file://` URI with the line as its
    /// fragment (`T056`). [`None`] for a call that touches no file, which is
    /// most `bash` rows.
    ///
    /// **A URI and not a path**, because the widget's job is to wrap it in
    /// OSC 8 and it must not be the thing deciding what a path means on this
    /// machine.
    pub link: Option<String>,
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

/// How a turn ended, or stopped (`T054`, `7b` under `T057`, `7e` under `T062`).
///
/// **One row in three tones**, which is what the three mockups draw: `1b` ends a
/// turn with `✻ review ready · retry logic — 2 files, 6 regions` in claude's
/// green, `7b` ends one with `✕ connection lost mid-turn · 14:47` in trouble-red,
/// and `7e` stops one with `⏸ paused at tool boundary · esc · 14:52` in §1's
/// attention-amber. The difference is not a different row — it is the same seam
/// saying a different thing about the same turn.
///
/// **This was a `bool` until `T062` and the third tone is why it is not.**
/// `trouble: bool` was honest about two cases and became a lie at three: a pause
/// is neither claude's nor trouble's, and amber is the palette's word for
/// *waiting on you*.
///
/// The [`detail`](Seam::detail) line is `7b`'s and is the reason this is a
/// struct at all. *"the transcript shows the seam honestly"* is the screen's
/// caption, and honesty here is specifically **what survived**: the sentence
/// under the seam names the disk state, the regions that had already arrived,
/// and the fact that the turn may be incomplete. A seam without it is a
/// statement that something went wrong and no statement about what you still
/// have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seam {
    /// The sentence beside the glyph.
    pub text: String,
    /// What survived, drawn under it in meta. [`None`] for a turn that ended
    /// the ordinary way, which has nothing to reassure anybody about.
    pub detail: Option<String>,
    /// Which of the three this is.
    pub tone: SeamTone,
}

/// Whose seam it is (`T062`).
///
/// Not [`Tone`](phosphor_core::view::Tone), which is the whole palette: a seam
/// has exactly three readings and naming them is what stops a fourth being
/// invented at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeamTone {
    /// `1b` — the turn ended and claude produced something. `✻`, claude-green.
    #[default]
    Ended,
    /// `7e` — you stopped it. `⏸`, attention-amber.
    Paused,
    /// `7b` — it stopped. `✕`, trouble-red.
    Trouble,
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
    pub ended: Option<Seam>,
    /// The call the agent was about to make when you paused it (`7e`, `T062`).
    ///
    /// **Drawn as `▸ next: edit tests/ws_test.rs` and not run.** *"`esc` pauses
    /// at the next tool boundary"* is a promise about *where* it stops, and a
    /// pause you cannot see the edge of is indistinguishable from a hang — this
    /// row is what makes the boundary a thing on screen rather than a claim in
    /// a doc.
    pub next: Option<ToolCall>,
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
    /// The footer strip — `1b`'s `↵ jump to file · q close`, and `7b`'s
    /// `:reattach · :cn · q close` when the session is gone.
    ///
    /// **A prop, not a policy.** What a transcript offers depends on whether
    /// there is a session to offer it about, and that is the host's question;
    /// a widget that decided it would be deciding when `:reattach` is worth
    /// suggesting. Empty draws no strip, which is what
    /// [`KeyHints::desired_height`](crate::key_hints::KeyHints::desired_height)
    /// already says an empty table means.
    pub hints: Vec<phosphor_core::view::KeyHint>,
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
    fn rows(&self, turn: &Turn, width: u16) -> Vec<Row> {
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
        //
        // **This comment was false until `T055`.** It said what it says now and
        // the loop under it was `turn.prose.lines()` — one row per `\n`, drawn
        // with `set_stringn`, which cuts at the pane edge. The tree won and the
        // comment was the bug. [`crate::prose::lines`] is where the two paths
        // are now, and it wraps in both.
        rows.extend(
            crate::prose::lines(&turn.prose, width, self.theme)
                .into_iter()
                .map(Row::Prose),
        );
        for call in &turn.calls {
            rows.push(Row::Call(call.clone()));
            for note in &call.notes {
                rows.push(Row::Note(note.clone()));
            }
        }
        // Before the seam, because that is the order `7e` draws them in and the
        // order they happened: the agent reached for something, and the pause
        // caught it there.
        if let Some(next) = &turn.next {
            rows.push(Row::Next(next.clone()));
        }
        match (&turn.ended, turn.since) {
            (Some(seam), _) => {
                rows.push(Row::Seam(seam.clone()));
                // Measured as its own row, because §11 fits turns by height and
                // a detail line that appeared only at paint time would make
                // every seam one row taller than the grouping believed.
                if let Some(detail) = &seam.detail {
                    rows.push(Row::Note(detail.clone()));
                }
            }
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
    /// One line of claude's prose, styled — rendered markdown behind the gate
    /// and the wrapped source without it (`T055`).
    Prose(Line<'static>),
    /// `▸ edit  src/retry.rs                     +42 −0`.
    Call(ToolCall),
    /// A progress line under an open call.
    Note(String),
    /// `✻ review ready · retry logic — 2 files, 6 regions`, or `7b`'s
    /// `✕ connection lost mid-turn`.
    Seam(Seam),
    /// The spinner and elapsed counter of a turn still running.
    Running(phosphor_core::view::Millis),
    /// `▸ next: edit tests/ws_test.rs` — held, not run (`7e`).
    Next(ToolCall),
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

        // **The footer is taken off the top of the room, before grouping.**
        // `1b` and `7b` both draw one, and a strip painted over the last turn
        // after the fitting had already promised it would fit is exactly the
        // *"a list that stopped mid-turn"* failure §11 is about — one row
        // lower down. `KeyHints` answers zero for an empty table, so a
        // transcript with nothing to offer loses no room to this.
        let footer = crate::key_hints::KeyHints::new(&self.vm.hints, Density::Footer, self.theme);
        let strip = footer.desired_height(area.width).min(area.height);
        let floor = area.bottom().saturating_sub(strip);

        // **§11: scale is grouping, not scrolling.** The turns that fit are
        // kept whole from the newest end, and the count of the rest is one row
        // — the same shape `key_hints`' `Density::Help` body uses, and the same
        // reason: a list that stopped mid-turn would read as a transcript that
        // ended there.
        let room = usize::from(floor.saturating_sub(y));
        // The room a *row* has, which is what prose wraps to — the pane inset
        // on both sides, not the pane.
        let text_width = area.width.saturating_sub(2 * PAD_COLS);
        let blocks: Vec<Vec<Row>> = self
            .vm
            .turns
            .iter()
            .map(|turn| self.rows(turn, text_width))
            .collect();
        let (from, dropped) = fit(&blocks, room, self.follow);

        if dropped > 0 && y < floor {
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
                if y >= floor {
                    break;
                }
                self.row(row, area, y, buf);
                y += 1;
            }
        }

        if strip > 0 {
            footer.render(
                Rect {
                    x: area.x,
                    y: floor,
                    width: area.width,
                    height: strip,
                },
                buf,
            );
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
            // produced — and the tones inside a rendered heading or a fenced
            // block, which the row carries rather than the caller choosing.
            Row::Prose(line) => {
                let room = area.right().saturating_sub(x);
                buf.set_line(x, y, line, room);
            }
            Row::Call(call) => self.call(call, area, y, buf),
            // **The same row as a call, prefixed and dimmed.** It is a tool
            // call in every respect except that it has not happened, so drawing
            // it as anything else would be inventing a second row shape for the
            // same fact. `next:` is the whole of the difference and `7e` writes
            // exactly that.
            Row::Next(call) => {
                let meta = Style::new().fg(self.theme.neutrals.meta);
                let after = write(buf, area, x, y, glyph::FOLDED, meta);
                let after = write(buf, area, after + 1, y, "next:", meta);
                let after = write(buf, area, after + 1, y, &call.verb, meta);
                if let Some(target) = &call.target {
                    write(buf, area, after + GAP, y, target, meta);
                }
            }
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
            // The seam is claude's, glyph and all — unless it is trouble's,
            // in which case §2's `✕` and the trouble tone, and both move
            // together because a red sentence behind a green glyph is the kind
            // of half-truth §5 spends its rules on.
            Row::Seam(seam) => {
                let (mark, tone) = match seam.tone {
                    SeamTone::Ended => (glyph::CLAUDE, self.theme.actors.claude),
                    SeamTone::Paused => (glyph::PAUSED, self.theme.actors.attention),
                    SeamTone::Trouble => (glyph::TROUBLE, self.theme.actors.trouble),
                };
                let after = write(buf, area, x, y, mark, Style::new().fg(tone));
                write(buf, area, after + 1, y, &seam.text, Style::new().fg(tone));
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
            let style = Style::new().fg(self.theme.neutrals.text);
            match &call.link {
                Some(uri) => link(buf, area, after + GAP, y, target, uri, style),
                None => {
                    write(buf, area, after + GAP, y, target, style);
                }
            }
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
/// Write `text` at `x` as an OSC 8 hyperlink to `uri` (`T056`).
///
/// # Why the whole link lives in one cell
///
/// OSC 8 is **stateful**: `ESC]8;;uri ST` opens a link and everything printed
/// until `ESC]8;; ST` belongs to it. Ratatui paints by diffing two cell grids
/// and emitting only the cells that changed, so an opener and a closer in
/// separate cells are two independent decisions — and the frame where the URI
/// changes but the last character does not prints the opener, skips the closer,
/// and leaves the link running across everything drawn after it. That is not a
/// rare race; it is what happens the first time claude edits a different file
/// whose name ends in the same letter.
///
/// So the entire sequence — opener, text, closer — is the symbol of a **single**
/// cell, which the diff can only emit or skip whole. The cells it visually
/// covers are marked [`CellDiffOption::Skip`] so nothing paints over the text
/// mid-sequence, and the anchor carries [`CellDiffOption::ForcedWidth`] because
/// its symbol measures dozens of columns wide and occupies as many as the text
/// does. **Ratatui 0.30.1 added these two options for exactly this**, and says
/// so: *"prevent the buffer from overwriting a cell that is covered by something
/// from an escape sequence, such as graphics or links."*
///
/// The text is underlined, which is
/// [`Emphasis::Underline`](phosphor_core::view::Emphasis)'s own definition —
/// *"an OSC 8 jump link in the transcript"* — and the only affordance a link
/// has on a surface where hovering costs nothing and clicking is the verb.
///
/// **A width of zero, or no room, writes nothing rather than half a sequence.**
/// An opener with no closer is the one failure mode that escapes the pane it was
/// drawn in.
fn link(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, uri: &str, style: Style) {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return;
    }
    let room = area.right() - x;
    let width = cells(text).min(room);
    let Some(forced) = NonZeroU16::new(width) else {
        return;
    };
    // **Clipped before the sequence is built**, so the closer is always the end
    // of what is written. Truncating the finished string is the one thing that
    // must never happen here: the tail of it is the closer, and a link that is
    // cut short is a link that never closes.
    let shown: String = {
        let mut kept = String::new();
        let mut used = 0;
        for character in text.chars() {
            let mut glyph = [0u8; 4];
            let next = used + cells(character.encode_utf8(&mut glyph));
            if next > width {
                break;
            }
            kept.push(character);
            used = next;
        }
        kept
    };
    let style = style.add_modifier(Modifier::UNDERLINED);
    buf[(x, y)]
        .set_symbol(&format!("{OSC8}{uri}{ST}{shown}{OSC8}{ST}"))
        .set_style(style)
        .set_diff_option(CellDiffOption::ForcedWidth(forced));
    for covered in (x + 1)..(x + width) {
        buf[(covered, y)]
            .set_style(style)
            .set_diff_option(CellDiffOption::Skip);
    }
}

/// OSC 8's opener, up to the URI. `\x1b]8;;`.
const OSC8: &str = "\x1b]8;;";
/// The string terminator that closes an OSC. `\x1b\\` — ST, not BEL: both are
/// accepted and ST is what the specification writes.
const ST: &str = "\x1b\\";

fn write(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, y, text, room as usize, style);
    next.min(area.right())
}

#[cfg(test)]
mod tests {
    use super::{PAD_COLS, ToolCall, TranscriptPane, TranscriptVm, Turn};
    use crate::theme::Theme;
    use phosphor_core::request::{ToolCallId, TurnId};
    use ratatui_core::buffer::{Buffer, CellDiffOption};
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Modifier;
    use ratatui_core::widgets::Widget;

    fn one_call(link: Option<&str>) -> TranscriptVm {
        TranscriptVm {
            header: String::new(),
            turns: vec![Turn {
                next: None,
                id: TurnId(1),
                prompt: None,
                prose: String::new(),
                calls: vec![ToolCall {
                    id: ToolCallId(1),
                    verb: "edit".to_owned(),
                    target: Some("src/retry.rs".to_owned()),
                    link: link.map(str::to_owned),
                    notes: Vec::new(),
                    outcome: None,
                }],
                ended: None,
                since: None,
            }],
            hints: Vec::new(),
        }
    }

    fn drawn(vm: &TranscriptVm, width: u16) -> Buffer {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, 4);
        let mut buf = Buffer::empty(area);
        TranscriptPane::new(vm, &theme).render(area, &mut buf);
        buf
    }

    /// Where the target starts: past the pad, the fold glyph, a space, `edit`,
    /// and the two-cell gap.
    const TARGET_X: u16 = PAD_COLS + 1 + 1 + 4 + super::GAP;

    /// **`T056`: the bytes, exactly.**
    ///
    /// The click itself is Tier 3 and stays `CP-6`'s — a capture cannot press a
    /// mouse — so what a test can hold this to is the sequence, and the sequence
    /// is either right or it is a link to nowhere. Opener, URI, terminator,
    /// text, empty opener, terminator; the whole of it in **one** cell.
    #[test]
    fn a_tool_row_with_a_location_is_one_osc_8_cell() {
        let buf = drawn(&one_call(Some("file:///tmp/toy/src/retry.rs#L19")), 60);
        let anchor = &buf[(TARGET_X, 0)];

        assert_eq!(
            anchor.symbol(),
            "\x1b]8;;file:///tmp/toy/src/retry.rs#L19\x1b\\src/retry.rs\x1b]8;;\x1b\\",
            "the anchor carries the whole sequence"
        );
        // **The width it declares is the width it covers**, not the width its
        // symbol measures — which is sixty-odd columns of escape.
        assert!(
            matches!(anchor.diff_option, CellDiffOption::ForcedWidth(width) if width.get() == 12),
            "and declares the twelve columns `src/retry.rs` occupies; was {:?}",
            anchor.diff_option
        );
        assert!(
            anchor.modifier.contains(Modifier::UNDERLINED),
            "a link is underlined — `Emphasis::Underline`'s own definition"
        );
    }

    /// **The cells the link covers are skipped, all of them.**
    ///
    /// This is the assertion that keeps OSC 8's statefulness from escaping the
    /// row: a covered cell the diff was still willing to paint would print over
    /// the middle of the sequence, and what follows a half-written opener is
    /// linked until something closes it.
    #[test]
    fn every_cell_the_link_covers_is_skipped() {
        let buf = drawn(&one_call(Some("file:///tmp/toy/src/retry.rs")), 60);
        for x in (TARGET_X + 1)..(TARGET_X + 12) {
            assert!(
                matches!(buf[(x, 0)].diff_option, CellDiffOption::Skip),
                "column {x} is covered by the link and must not be painted"
            );
        }
        // And the column *after* the link is ordinary again — a run of skips
        // that overshot would silently blank whatever came next.
        assert!(
            matches!(buf[(TARGET_X + 12, 0)].diff_option, CellDiffOption::None),
            "the column after the link is drawable again"
        );
    }

    /// **A call with no file is not a link**, which is most `bash` rows. Drawn
    /// as plain text, with no sequence and nothing skipped.
    #[test]
    fn a_call_that_touches_no_file_is_plain_text() {
        let buf = drawn(&one_call(None), 60);
        assert_eq!(buf[(TARGET_X, 0)].symbol(), "s", "the target is just text");
        assert!(
            !buf[(TARGET_X, 0)].modifier.contains(Modifier::UNDERLINED),
            "and is not underlined, because there is nowhere to go"
        );
    }

    /// **A pane too narrow for the target clips the text and never the
    /// sequence.** The closer is the last thing written; a link cut short is
    /// the one failure that escapes the pane it was drawn in.
    #[test]
    fn a_narrow_pane_clips_the_text_and_still_closes_the_link() {
        let buf = drawn(
            &one_call(Some("file:///tmp/toy/src/retry.rs")),
            TARGET_X + 5,
        );
        let symbol = buf[(TARGET_X, 0)].symbol();
        assert!(
            symbol.ends_with("\x1b]8;;\x1b\\"),
            "the sequence closes; symbol was {symbol:?}"
        );
        assert!(
            symbol.contains("\x1b\\src/r\x1b]8;;"),
            "and the text inside it is what fits; symbol was {symbol:?}"
        );
    }
}
