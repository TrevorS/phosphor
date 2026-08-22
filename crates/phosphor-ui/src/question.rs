//! `QuestionBody` (`T059`) — screen `4a`, claude asking mid-turn.
//!
//! Draws [`phosphor_core::view::Node::Question`]: claude's prose, then the
//! numbered options in §1's attention-amber, one per row. `4a`'s caption is the
//! whole shape of the interaction — *"mid-turn question · quick-answer with
//! digits, prose with `:c`, or ignore until later"* — three ways out, and the
//! third is the one the design is actually built around.
//!
//! # A body, not a float
//!
//! The border, the header and the footer are [`Float`](crate::float)'s, which is
//! `T084`'s whole point: this widget draws the *inside*. So the digit rows are
//! here and `4a`'s `1–3 answer · :c reply · esc later` strip is
//! [`KeyHints`](crate::key_hints) at `Density::Footer`, composed by the host —
//! the same division `Node::Picker` and `Node::Transcript` already use, and the
//! reason a fourth surface costs no new chrome.
//!
//! # Why the digits are a *rendering* and not a keymap
//!
//! *"Digits answer only while it is focused"* is the node's own sentence, and it
//! is enforced where focus lives — in the input layer, against the float that
//! holds it. A widget cannot know whether it is focused and must not try: the
//! two things that would answer a digit are this surface and the buffer's own
//! count prefix, and a widget that decided between them would be a second focus
//! model to disagree with the real one.
//!
//! What this file guarantees is narrower and checkable: **every option drawn
//! carries the digit that answers it**, so a screen offering `[3]` is a screen
//! where `3` is a real answer.

use phosphor_core::request::AskOption;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// Cells of air at the left of every row, matching the float body's inset.
pub const PAD_COLS: u16 = 1;

/// Cells between an option's `[n]` and its label.
const GAP: u16 = 1;

/// One question, as a ViewModel (`T059`).
///
/// Derived from the queue, read-only, rebuilt when it moves — the same contract
/// as every other `Vm` this crate draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestionVm {
    /// What claude is asking, in his own words. Wrapped, not truncated: this is
    /// a paragraph, and `4a` draws two lines of one.
    pub prose: String,
    /// The numbered options, in the order they were given. Empty is a question
    /// with no shortcut — legitimate, and `4a`'s `:c reply` is what answers it.
    pub options: Vec<AskOption>,
}

impl phosphor_core::vm::ViewModel for QuestionVm {}

/// `4a`'s body.
#[derive(Debug, Clone, Copy)]
pub struct QuestionBody<'a> {
    vm: &'a QuestionVm,
    theme: &'a Theme,
}

impl<'a> QuestionBody<'a> {
    /// A body over `vm`.
    #[must_use]
    pub const fn new(vm: &'a QuestionVm, theme: &'a Theme) -> Self {
        Self { vm, theme }
    }

    /// Rows this body wants at `width`.
    ///
    /// **Measured rather than drawn twice.** A float sizes itself before it
    /// paints, so the body has to be able to say how tall it is — and the prose
    /// wraps, so the answer depends on the width it is given.
    #[must_use]
    pub fn desired_height(&self, width: u16) -> u16 {
        let text = width.saturating_sub(2 * PAD_COLS);
        let prose = u16::try_from(self.prose_rows(text).len()).unwrap_or(u16::MAX);
        let options = u16::try_from(self.vm.options.len()).unwrap_or(u16::MAX);
        // The blank row between the question and its answers, and only when
        // there are answers: a question with none is a paragraph, and a
        // paragraph does not end in a separator.
        prose
            .saturating_add(u16::from(options > 0))
            .saturating_add(options)
    }

    fn prose_rows(&self, width: u16) -> Vec<String> {
        let source: Vec<String> = self.vm.prose.lines().map(str::to_owned).collect();
        crate::float::wrap_prose(&source, width)
    }
}

impl Widget for QuestionBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let x = area.x + PAD_COLS;
        let mut y = area.y;

        // §6: claude's prose is the prose neutral, as distinct from the facts
        // he produced — the same tone the transcript gives it, because it is
        // the same voice arriving on a different surface.
        for row in self.prose_rows(area.width.saturating_sub(2 * PAD_COLS)) {
            if y >= area.bottom() {
                return;
            }
            write(
                buf,
                area,
                x,
                y,
                &row,
                Style::new().fg(self.theme.neutrals.prose),
            );
            y += 1;
        }
        if self.vm.options.is_empty() {
            return;
        }
        y += 1;

        for option in &self.vm.options {
            if y >= area.bottom() {
                return;
            }
            // **The bracket and its digit are one string.** `4a` draws `[1]`
            // and the digit is the thing you press; splitting them across two
            // writes is two places for the number to come from.
            let key = format!("[{}]", option.digit);
            let after = write(
                buf,
                area,
                x,
                y,
                &key,
                // §1's attention-amber — *"waiting, paused, dirty,
                // permission"*, and a question waiting on you is all four in
                // spirit.
                Style::new().fg(self.theme.actors.attention),
            );
            write(
                buf,
                area,
                after + GAP,
                y,
                &option.label,
                Style::new().fg(self.theme.neutrals.text),
            );
            y += 1;
        }
    }
}

/// Write `text` at `x` on row `y`, clipped to the area. Returns the column
/// after the last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, y, text, room as usize, style);
    next.min(area.right())
}

/// A readable measure for a paragraph, in columns.
///
/// **Not the screen.** `4a` draws its question across two lines of a float that
/// is not full width, and a question set in one 200-column line is a question
/// nobody reads. The float clamps this down on a narrow screen; nothing clamps
/// it *up*, which is what this constant is.
const PROSE_COLS: u16 = 76;

/// The widest row this body would like, for a float that is sizing itself.
///
/// **The prose counts, up to [`PROSE_COLS`].** An earlier version answered the
/// widest option row only, on the reasoning that prose wraps and options do not
/// — which is true and gave `4a` a float as wide as `show me the failure` with
/// claude's two-line question reflowed into a column. What wraps still has a
/// width it would *like*.
#[must_use]
pub fn natural_width(vm: &QuestionVm) -> u16 {
    let options = vm
        .options
        .iter()
        .map(|option| {
            2 * PAD_COLS + cells(&format!("[{}]", option.digit)) + GAP + cells(&option.label)
        })
        .max()
        .unwrap_or(0);
    let prose = vm
        .prose
        .lines()
        .map(|line| 2 * PAD_COLS + cells(line))
        .max()
        .unwrap_or(0)
        .min(PROSE_COLS);
    options.max(prose)
}
