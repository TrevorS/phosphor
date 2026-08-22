//! `PromptLine` (`T058`) — screen `1c`, the `:` line and its anchor chip.
//!
//! Draws [`phosphor_core::view::Node::Prompt`]: an optional `⚓` chip naming
//! what the message is about, the typed text, and a block cursor at the end.
//!
//! **This is the demolition `docs/OPEN-QUESTIONS.md` §13 scheduled.** That
//! ruling let the ex row be drawn from `Node::Line` and `Node::Label` in the
//! binary — *"scaffolding with a demolition date"* — because `phosphor-ui`
//! deferred `prompt` to this task. The scaffolding's own comment in `main.rs`
//! named the date; this is it.
//!
//! # `1c`, read out
//!
//! ```text
//! ⚓ src/retry.rs:19–21  :c collapse these arms — use the shared backoff helper
//! ```
//!
//! The chip is claude-green inside a `#2a5c44` border — the informational float
//! mood, which is what a chip *is*: a small bordered statement of fact. The text
//! after it is ordinary [`text`](crate::theme::NeutralRamp::text), and the
//! cursor is one inverted cell, because §5 reserves inversion for the mode chip
//! and a caret is not a chip.
//!
//! # The chip's border is two cells, not a box
//!
//! A terminal cannot draw `1c`'s rounded 1px rule around a run of text without
//! spending a row above and below it. What it can spend is **one cell either
//! side**: `▏` and `▕` in the border's own colour, which reads as an enclosure
//! at a glance and costs the row nothing. Recorded here rather than approximated
//! silently — the alternative was a full `Block`, which would make a one-row
//! surface three.
//!
//! # Where the row goes is not this widget's business
//!
//! `1c` draws the prompt **below** the statusline, which every other mockup's
//! prompt does not do because no other mockup draws one. The binary's
//! `Geometry` decides: an anchored prompt gets its own row, an unanchored one
//! borrows the statusline the way vim does. Both hand this widget one `Rect`.
//!
//! Owned by `surface`.

use phosphor_core::request::{FileSpan, PromptKind};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// `⚓` — §2's anchor, and what a chip is built around.
///
/// **No space after it in the format string.** The glyph is double-width, so
/// the cell after it is its own continuation — writing a space too puts three
/// columns between the anchor and the path where `1c` shows one gap. Measured
/// against the buffer: `▏⚓  src/retry.rs` before, `▏⚓ src/retry.rs` after.
const ANCHOR: &str = "⚓";

/// The chip's left and right edges. See the module docs on why a border is two
/// cells rather than a box.
const CHIP_LEFT: &str = "▏";
const CHIP_RIGHT: &str = "▕";

/// Cells between the chip and the text.
const GAP: u16 = 1;

/// The `:` line, with whatever rides along.
#[derive(Debug, Clone, Copy)]
pub struct PromptLine<'a> {
    kind: PromptKind,
    text: &'a str,
    anchor: Option<&'a FileSpan>,
    theme: &'a Theme,
}

impl<'a> PromptLine<'a> {
    /// A prompt of `kind` over `text`.
    #[must_use]
    pub const fn new(kind: PromptKind, text: &'a str, theme: &'a Theme) -> Self {
        Self {
            kind,
            text,
            anchor: None,
            theme,
        }
    }

    /// What the message is about — `1c`'s selection, riding along.
    #[must_use]
    pub const fn anchor(mut self, anchor: Option<&'a FileSpan>) -> Self {
        self.anchor = anchor;
        self
    }

    /// The chip's text: `⚓ src/retry.rs:19–21`.
    ///
    /// **An en dash between the lines, and it is `1c`'s.** §6 keeps the em dash
    /// for cause — *"session lost — :reattach"* — so a *range* uses the shorter
    /// one, which is what the mockup draws and what every style guide means by
    /// a span of numbers.
    ///
    /// One line is not a range: a chip reading `19–19` says the same thing as
    /// `19` and takes three more cells.
    fn chip(anchor: &FileSpan) -> String {
        let path = anchor.path.display();
        // **A `FileSpan`'s span is optional**, and a chip for a whole file is a
        // real thing: `1c` anchors a range because a visual selection made one,
        // and `:c` with no selection anchors the file you are in.
        let Some(span) = anchor.span else {
            return format!("{ANCHOR}{path}");
        };
        let (from, to) = (span.start.line, span.end.line);
        if from == to {
            format!("{ANCHOR}{path}:{from}")
        } else {
            format!("{ANCHOR}{path}:{from}–{to}")
        }
    }

    /// What the typed line reads, prefix included.
    ///
    /// The prefix is the *kind*, not a character the user typed: `:` opened an
    /// ex line and `:c` a message, and the mockups draw both with the prefix
    /// on screen. Search is vim's `/`.
    fn typed(&self) -> String {
        let prefix = match self.kind {
            PromptKind::Ex => ":",
            PromptKind::Claude => ":c ",
            PromptKind::Search => "/",
        };
        format!("{prefix}{}", self.text)
    }
}

impl Widget for PromptLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        // **No ground of its own**, unlike `tab_bar` and the leader grid.
        // Those two *are* their strip and nothing else paints there; a prompt
        // borrows a row the caller has already painted — §5's statusline field
        // today, `1c`'s ground if it ever gets a row of its own — and painting
        // over it would replace a field the caller chose. `interpret.rs`'s own
        // header states the rule: a node cannot say what ground it sits on, so
        // it draws transparently over whatever the caller painted.
        //
        // Found by `the_chrome_strip_is_painted_under_the_statusline_and_the_ex_line`,
        // which asserts the ex row's background matches the statusline's: this
        // widget painted `neutrals.ground` over it and the two stopped
        // matching.
        let ground = Style::new().fg(self.theme.neutrals.text);

        let mut x = area.x;
        if let Some(anchor) = self.anchor {
            let edge = Style::new().fg(self.theme.float.informational);
            let inside = Style::new().fg(self.theme.actors.claude);
            x = write(buf, area, x, CHIP_LEFT, edge);
            x = write(buf, area, x, &Self::chip(anchor), inside);
            x = write(buf, area, x, CHIP_RIGHT, edge);
            x = x.saturating_add(GAP);
        }

        let typed = self.typed();
        let after = write(buf, area, x, &typed, ground);
        // The caret: one inverted cell at the end, and only if it fits. §5
        // reserves inversion for the mode chip *as a chip*; a one-cell caret is
        // not a second one, and it is what `1c` draws.
        if after < area.right() {
            // The caret is the one thing that *does* set a background: it is
            // an inverted cell by definition, and inverting needs both halves.
            buf[(after, area.y)].set_symbol(" ").set_style(
                Style::new()
                    .fg(self.theme.neutrals.ground)
                    .bg(self.theme.neutrals.text),
            );
        }
    }
}

/// Write `text` at `x` on `area`'s row, clipped. Answers the column after it.
fn write(buf: &mut Buffer, area: Rect, x: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, area.y, text, room as usize, style);
    next.min(area.right())
}

/// How many rows an anchored prompt needs. Always one — §5 never wraps, and a
/// prompt too long for its row scrolls its text rather than growing.
///
/// Public because the binary's `Geometry` has to reserve the row before the
/// tree is composed, and a second constant there would be a second answer.
#[must_use]
pub const fn rows() -> u16 {
    1
}

/// The widest a chip can be before it is worth eliding. Not enforced here —
/// `write` clips — and named so a caller that wants to shorten a path knows
/// what it is aiming at.
#[must_use]
pub fn chip_width(anchor: &FileSpan) -> u16 {
    cells(&PromptLine::chip(anchor)) + 2
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::PromptLine;
    use crate::theme::Theme;
    use phosphor_core::request::{FileSpan, Position, PromptKind, Span};
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::widgets::Widget;

    fn anchor(from: u32, to: u32) -> FileSpan {
        FileSpan {
            path: "src/retry.rs".into(),
            span: Some(Span {
                start: Position {
                    line: from,
                    column: 1,
                },
                end: Position {
                    line: to,
                    column: 1,
                },
            }),
        }
    }

    fn draw(line: PromptLine<'_>, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        line.render(area, &mut buf);
        (0..width).map(|x| buf[(x, 0u16)].symbol()).collect()
    }

    /// **`1c`, read out.** The chip names the file and the range that rode
    /// along, and the message follows it.
    #[test]
    fn an_anchored_prompt_draws_the_chip_then_the_line() {
        let theme = Theme::phosphor_dark();
        let held = anchor(19, 21);
        let drawn = draw(
            PromptLine::new(PromptKind::Claude, "collapse these arms", &theme).anchor(Some(&held)),
            72,
        );
        assert!(
            drawn.contains("src/retry.rs:19–21") && drawn.contains('⚓'),
            "the chip is `1c`'s: {drawn:?}"
        );
        assert!(
            drawn.contains(":c collapse these arms"),
            "and the message follows it, prefixed by its kind: {drawn:?}"
        );
    }

    /// One line is not a range — `19–19` says what `19` says and costs three
    /// more cells.
    #[test]
    fn one_line_is_a_line_and_not_a_range() {
        let theme = Theme::phosphor_dark();
        let held = anchor(19, 19);
        let drawn = draw(
            PromptLine::new(PromptKind::Ex, "", &theme).anchor(Some(&held)),
            48,
        );
        assert!(drawn.contains("src/retry.rs:19"), "{drawn:?}");
        assert!(!drawn.contains("19–"), "{drawn:?}");
    }

    /// **An unanchored prompt is just the line**, which is every `:` this
    /// editor has had until now and what vim draws.
    #[test]
    fn an_unanchored_prompt_draws_no_chip() {
        let theme = Theme::phosphor_dark();
        let drawn = draw(PromptLine::new(PromptKind::Ex, "write", &theme), 32);
        assert!(!drawn.contains('⚓'), "{drawn:?}");
        assert!(drawn.starts_with(":write"), "{drawn:?}");
    }

    /// A row too narrow for the chip clips rather than wrapping — §5's *"never
    /// wraps; a second line is a bug"* holds for a prompt too.
    #[test]
    fn a_narrow_row_clips_rather_than_wrapping() {
        let theme = Theme::phosphor_dark();
        let held = anchor(19, 21);
        let drawn = draw(
            PromptLine::new(PromptKind::Ex, "a very long command indeed", &theme)
                .anchor(Some(&held)),
            12,
        );
        assert_eq!(
            drawn.chars().count(),
            12,
            "one row, twelve cells: {drawn:?}"
        );
    }
}
