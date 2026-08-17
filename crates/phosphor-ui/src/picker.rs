//! The picker — one widget, many sources (`T045`).
//!
//! A filter line, a matched list, and an optional preview split.
//! [`Node::Picker`](phosphor_core::view::Node::Picker)'s own doc names the
//! sources it is meant to serve: *"unseen, files, inbox, grep, symbols, session
//! adoption and the jj timeline are all this kind with a different source
//! key"*. So nothing here knows what a row means — the source supplies styled
//! runs and this draws them.
//!
//! # Where the parts live
//!
//! ```text
//!   the rows          a picker source (T046, T047) → the store
//!   the matching      nucleo, off-thread, in the binary
//!   the filter TEXT   Node::Picker's `filter` prop, from composition
//!   this file         layout, the shed ladder, and drawing
//! ```
//!
//! **The matcher is not here and cannot be.** `phosphor-ui` takes
//! `ratatui-core` and nothing that reaches a terminal or a thread pool;
//! `scripts/lint-no-app-layer-in-ui.sh` is a *source* lint precisely because
//! Cargo unifies features per crate across the graph. A widget that owned
//! nucleo's threads would also be a widget that outlives a frame, which
//! [`PickerVm`] is the answer to — same seam
//! [`CompletionVm`](crate::float::CompletionVm) already is, for the same
//! reason.
//!
//! # `ratatui-textarea` is deliberately absent, and `T045` asked for it
//!
//! The task says *"`ratatui-textarea` filter line"*. Reading the vocabulary it
//! has to render says otherwise: `Node::Picker` carries
//! **`filter: String` as a prop**, so the filter's text is composition's and
//! arrives fresh every frame. A textarea inside this widget would hold a second
//! copy of that string and would have to be reconciled with the prop on every
//! composition — two maps with one name, which is the exact defect `T041` found
//! in `store::diagnostics` and folded away.
//!
//! What the crate would buy is editing *inside* the filter line — a cursor that
//! moves, a selection, undo. None of that is reachable: keystrokes go to the
//! input machine, and `Node::Picker` has no cursor prop to carry a position
//! back. So it would add a dependency, a feature-unification hazard
//! (`ratatui-textarea`'s default features include `crossterm`, which this crate
//! may not link), and a duplicate source of truth, in exchange for nothing that
//! is currently expressible.
//!
//! **Flagged rather than folded in**, per `CLAUDE.md`: if a cursor inside the
//! filter is wanted, the change is a prop on `Node::Picker` first, and the
//! crate second. Recorded on `T045` in `docs/TASKS.md`.
//!
//! # The shed ladder
//!
//! Design Language §11 — *"narrow terminals drop, never squeeze"*. The preview
//! is the first thing to go, at the width `T045` names:
//!
//! ```text
//!   >= 100 cols   filter · list · preview
//!    < 100 cols   filter · list
//! ```
//!
//! Below [`MIN_LIST`] there is no list either and the widget draws the filter
//! alone, because a one-column list is worse than an honest absence.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
use ratatui_core::style::{Modifier, Style};
use ratatui_core::widgets::Widget;

use phosphor_core::view::Tone;

use crate::theme::Theme;

/// The width at or above which the preview split is drawn.
///
/// `T045`'s own number — *"preview split (dropped under 100 cols)"* — and the
/// only place it is written down.
pub const PREVIEW_AT: u16 = 100;

/// The narrowest a list may be and still be a list.
const MIN_LIST: u16 = 12;

/// How much of the width the preview takes when it is drawn.
const PREVIEW_SHARE: u16 = 40;

// ---------------------------------------------------------------------------
// The ViewModel
// ---------------------------------------------------------------------------

/// One styled run of a row.
///
/// A row is runs rather than a string because `T045`'s own line is *"rows are
/// `Vec<Span>` so agent context renders in actor colours"* — a files-picker row
/// that says *"claude touched this 4 minutes ago"* has to be able to say
/// `claude` in claude's green, and a row that was one string could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunVm {
    /// The text.
    pub text: String,
    /// Which actor or state colours it. [`Tone::Text`] is ordinary prose.
    pub tone: Tone,
    /// Whether this run is part of what the filter matched.
    ///
    /// Drawn bold rather than in a different colour, because §1 is *"each
    /// colour names exactly one actor or state, never decoration"* — a match
    /// highlight is decoration and has no colour of its own to spend.
    pub matched: bool,
}

impl RunVm {
    /// A plain run of prose.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: Tone::Text,
            matched: false,
        }
    }

    /// The same run in an actor's colour.
    #[must_use]
    pub fn toned(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The same run, marked as matched.
    #[must_use]
    pub fn matched(mut self) -> Self {
        self.matched = true;
        self
    }
}

/// One row of the matched list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowVm {
    /// The styled runs, left to right.
    pub runs: Vec<RunVm>,
}

impl RowVm {
    /// A row from its runs.
    #[must_use]
    pub fn new(runs: Vec<RunVm>) -> Self {
        Self { runs }
    }

    /// How many cells the row wants.
    #[must_use]
    pub fn width(&self) -> usize {
        self.runs.iter().map(|run| run.text.chars().count()).sum()
    }
}

/// What a picker is showing, this frame.
///
/// Lent through [`crate::interpret::Resources::picker`] for the reason
/// [`CompletionVm`](crate::float::CompletionVm) is: a widget crate cannot read
/// the store, and a session outlives a frame while a
/// [`Node`](phosphor_core::view::Node) does not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PickerVm {
    /// The rows that survived the filter, best match first.
    pub rows: Vec<RowVm>,
    /// Which row is selected.
    ///
    /// Out of range selects nothing, which is the honest reading of an empty
    /// list and of a filter that has just excluded everything — the same rule
    /// [`CompletionVm::selected`](crate::float::CompletionVm) follows.
    pub selected: usize,
    /// The preview for the selected row, one string per line.
    ///
    /// §11 is *"nothing ever wraps"*, so a source that wants two rows sends two
    /// strings. Empty draws an empty pane rather than reclaiming the width:
    /// a preview that appeared and vanished as the selection moved would make
    /// the list jump under the cursor.
    pub preview: Vec<String>,
    /// How many rows the source has before filtering.
    ///
    /// Drawn as `matched/total` so *"nothing matched"* and *"there is nothing"*
    /// are distinguishable — the difference between a bad filter and an empty
    /// source, which a bare `0` cannot say.
    pub total: usize,
    /// Whether the matcher is still working.
    ///
    /// `T045`'s criterion is *"it stays responsive filtering a 100k-file
    /// list"*, and responsive means the frame draws while matching is
    /// incomplete. This is what lets it say so instead of showing a stale count
    /// as though it were final.
    pub matching: bool,
}

impl PickerVm {
    /// The selected row, if the selection is in range.
    #[must_use]
    pub fn selection(&self) -> Option<&RowVm> {
        self.rows.get(self.selected)
    }
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// The picker, drawn.
#[derive(Debug)]
pub struct Picker<'a> {
    vm: &'a PickerVm,
    theme: &'a Theme,
    filter: &'a str,
    preview: bool,
}

impl<'a> Picker<'a> {
    /// A picker over `vm`.
    ///
    /// `filter` is [`Node::Picker`](phosphor_core::view::Node::Picker)'s prop
    /// and `preview` is its request — the *request*, not the answer, because
    /// §11's ladder can still drop it.
    #[must_use]
    pub const fn new(vm: &'a PickerVm, theme: &'a Theme, filter: &'a str, preview: bool) -> Self {
        Self {
            vm,
            theme,
            filter,
            preview,
        }
    }

    /// Whether the preview is drawn at this width.
    ///
    /// Both halves matter: the caller has to *want* one, and the terminal has
    /// to be wide enough. Public because the shed ladder is a fact about the
    /// layout that a test and a composition both ask about, and two spellings
    /// of one threshold is how `PREVIEW_AT` would drift.
    #[must_use]
    pub const fn shows_preview(&self, width: u16) -> bool {
        self.preview && width >= PREVIEW_AT
    }
}

impl Widget for Picker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let rows = Layout::new(
            Direction::Vertical,
            [Constraint::Length(1), Constraint::Min(0)],
        )
        .split(area);
        let (filter_row, body) = (rows[0], rows[1]);

        self.filter_line(filter_row, buf);

        if body.is_empty() {
            return;
        }
        if self.shows_preview(area.width) {
            let split = Layout::new(
                Direction::Horizontal,
                [
                    Constraint::Percentage(100 - PREVIEW_SHARE),
                    Constraint::Percentage(PREVIEW_SHARE),
                ],
            )
            .split(body);
            self.list(split[0], buf);
            self.preview_pane(split[1], buf);
        } else if body.width >= MIN_LIST {
            self.list(body, buf);
        }
    }
}

impl Picker<'_> {
    /// `> filter                                   12/100k`
    fn filter_line(&self, area: Rect, buf: &mut Buffer) {
        let prompt = Style::new().fg(self.theme.neutrals.meta);
        let text = Style::new().fg(self.theme.neutrals.text);
        let mut x = write(buf, area, area.x, "> ", prompt);
        x = write(buf, area, x, self.filter, text);

        // The cursor, as a block on the cell after the text. Drawn rather than
        // placed: `phosphor-ui` cannot reach a terminal, so it cannot ask for a
        // real cursor — and a picker whose filter line had no cursor at all
        // would read as not focused.
        if x < area.right()
            && let Some(cell) = buf.cell_mut((x, area.y))
        {
            cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
        }

        let count = if self.vm.matching {
            format!("{}/{}…", self.vm.rows.len(), self.vm.total)
        } else {
            format!("{}/{}", self.vm.rows.len(), self.vm.total)
        };
        let width = u16::try_from(count.chars().count()).unwrap_or(u16::MAX);
        if area.width > width {
            write(
                buf,
                area,
                area.right().saturating_sub(width),
                &count,
                Style::new().fg(self.theme.neutrals.meta),
            );
        }
    }

    fn list(&self, area: Rect, buf: &mut Buffer) {
        // The window slides only far enough to hold the selection, so a
        // selection that has not moved does not scroll the list under it.
        let height = usize::from(area.height);
        if height == 0 {
            return;
        }
        // **Clamped to the last row before the window is computed**, and the
        // test that found this is `selection_out_of_range_selects_nothing…`: an
        // out-of-range selection scrolled the window past the end and drew a
        // blank list. Out of range selects nothing — that is
        // [`PickerVm::selected`]'s documented rule — but it must not also mean
        // *shows* nothing, which is a filter that has just excluded the
        // selection and is the ordinary case rather than an error.
        let last = self.vm.rows.len().saturating_sub(1);
        let anchor = self.vm.selected.min(last);
        let first = anchor.saturating_sub(height.saturating_sub(1));
        for (offset, row) in self.vm.rows.iter().skip(first).take(height).enumerate() {
            let y = area.y + u16::try_from(offset).unwrap_or(u16::MAX);
            let index = first + offset;
            let selected = index == self.vm.selected;
            if selected {
                let ground = Style::new().bg(self.theme.regions.selection);
                for x in area.x..area.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(ground);
                    }
                }
            }
            let mut x = area.x;
            for run in &row.runs {
                if x >= area.right() {
                    break;
                }
                let mut style = Style::new().fg(self.theme.tone(run.tone));
                if selected {
                    style = style.bg(self.theme.regions.selection);
                }
                if run.matched {
                    style = style.add_modifier(Modifier::BOLD);
                }
                x = write(
                    buf,
                    Rect {
                        y,
                        height: 1,
                        ..area
                    },
                    x,
                    &run.text,
                    style,
                );
            }
        }
    }

    fn preview_pane(&self, area: Rect, buf: &mut Buffer) {
        let style = Style::new().fg(self.theme.neutrals.text);
        for (offset, line) in self
            .vm
            .preview
            .iter()
            .take(usize::from(area.height))
            .enumerate()
        {
            let y = area.y + u16::try_from(offset).unwrap_or(u16::MAX);
            write(
                buf,
                Rect {
                    y,
                    height: 1,
                    ..area
                },
                area.x,
                line,
                style,
            );
        }
    }
}

/// Write `text` at `x`, clipped to `area`, and answer the next free column.
///
/// One truncating writer rather than `Line::render`, for the reason §11 gives:
/// *"nothing ever wraps"*, and a row that ran past its area would wrap into the
/// row below it.
fn write(buf: &mut Buffer, area: Rect, x: u16, text: &str, style: Style) -> u16 {
    let mut at = x;
    for character in text.chars() {
        if at >= area.right() {
            break;
        }
        if let Some(cell) = buf.cell_mut((at, area.y)) {
            cell.set_symbol(&character.to_string());
            cell.set_style(style);
        }
        at += 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;

    fn vm(rows: usize, total: usize) -> PickerVm {
        PickerVm {
            rows: (0..rows)
                .map(|index| RowVm::new(vec![RunVm::text(format!("row{index}"))]))
                .collect(),
            selected: 0,
            preview: vec!["a preview line".to_owned()],
            total,
            matching: false,
        }
    }

    fn drawn(vm: &PickerVm, filter: &str, preview: bool, width: u16, height: u16) -> String {
        let theme = builtin("phosphor-dark").expect("the shipped theme");
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        Picker::new(vm, &theme, filter, preview).render(area, &mut buf);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map_or(" ".to_owned(), |cell| cell.symbol().to_owned())
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_filter_line_carries_the_prompt_the_text_and_the_count() {
        let screen = drawn(&vm(3, 40), "ret", false, 40, 6);
        let first = screen.lines().next().expect("a first row");

        assert!(first.starts_with("> ret"), "prompt and filter: {first:?}");
        assert!(first.trim_end().ends_with("3/40"), "the count: {first:?}");
    }

    /// An empty source draws `0/0` rather than nothing. *"Nothing matched"* and
    /// *"there is nothing"* are different facts and a blank line says neither.
    #[test]
    fn an_empty_source_still_draws_its_count() {
        let screen = drawn(&PickerVm::default(), "", false, 40, 4);
        assert!(
            screen.lines().next().is_some_and(|row| row.contains("0/0")),
            "screen was:\n{screen}"
        );
    }

    /// While the matcher is still running the count carries an ellipsis, so a
    /// partial result is not read as a final one.
    #[test]
    fn a_running_matcher_says_so_in_the_count() {
        let mut partial = vm(2, 100_000);
        partial.matching = true;
        let screen = drawn(&partial, "x", false, 40, 4);

        assert!(
            screen.contains("2/100000…"),
            "an unfinished count is marked: {screen}"
        );
    }

    #[test]
    fn the_rows_are_drawn_under_the_filter_line() {
        let screen = drawn(&vm(3, 3), "", false, 40, 6);
        let rows: Vec<&str> = screen.lines().collect();

        assert!(rows[1].starts_with("row0"));
        assert!(rows[2].starts_with("row1"));
        assert!(rows[3].starts_with("row2"));
    }

    /// §11's ladder — *"narrow terminals drop, never squeeze"*. `T045`'s own
    /// number is 100 columns.
    #[test]
    fn the_preview_drops_below_a_hundred_columns() {
        let vm = vm(2, 2);
        let theme = builtin("phosphor-dark").expect("the shipped theme");

        let wide = Picker::new(&vm, &theme, "", true);
        assert!(wide.shows_preview(PREVIEW_AT));
        assert!(wide.shows_preview(PREVIEW_AT + 40));

        let narrow = Picker::new(&vm, &theme, "", true);
        assert!(!narrow.shows_preview(PREVIEW_AT - 1));

        // And a caller that never wanted one does not get one at any width.
        let never = Picker::new(&vm, &theme, "", false);
        assert!(!never.shows_preview(PREVIEW_AT + 200));
    }

    #[test]
    fn the_preview_is_drawn_on_the_right_when_it_is_shown() {
        let screen = drawn(&vm(2, 2), "", true, 120, 4);
        assert!(
            screen.contains("a preview line"),
            "the preview pane draws: {screen}"
        );
        // And is absent one column below the threshold.
        let narrow = drawn(&vm(2, 2), "", true, PREVIEW_AT - 1, 4);
        assert!(!narrow.contains("a preview line"), "dropped: {narrow}");
    }

    /// A row longer than the area is clipped, never wrapped — §11 again, and
    /// the reason this file has its own `write`.
    #[test]
    fn a_long_row_is_clipped_rather_than_wrapped() {
        let long = PickerVm {
            rows: vec![RowVm::new(vec![RunVm::text("x".repeat(200))])],
            total: 1,
            ..PickerVm::default()
        };
        let screen = drawn(&long, "", false, 20, 3);
        let rows: Vec<&str> = screen.lines().collect();

        assert_eq!(rows[1].trim_end().len(), 20, "clipped to the width");
        assert!(
            rows[2].trim().is_empty(),
            "and did not wrap into the next row: {screen}"
        );
    }

    /// The window slides only far enough to hold the selection, so a list
    /// taller than its area still shows the selected row.
    #[test]
    fn a_selection_past_the_bottom_scrolls_into_view() {
        let mut deep = vm(20, 20);
        deep.selected = 19;
        let screen = drawn(&deep, "", false, 40, 4);

        assert!(
            screen.contains("row19"),
            "the selected row is on screen: {screen}"
        );
    }

    #[test]
    fn selection_out_of_range_selects_nothing_rather_than_panicking() {
        let mut past = vm(2, 2);
        past.selected = 99;

        assert!(past.selection().is_none());
        // And it still draws.
        let screen = drawn(&past, "", false, 40, 4);
        assert!(screen.contains("row"), "still drew rows: {screen}");
    }

    #[test]
    fn a_zero_sized_area_draws_nothing_rather_than_panicking() {
        let theme = builtin("phosphor-dark").expect("the shipped theme");
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
        Picker::new(&vm(3, 3), &theme, "x", true).render(area, &mut buf);
    }
}
