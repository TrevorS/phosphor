//! `GutterBar` (`T031`) — the 1-cell state column on its own.
//!
//! Draws `Node::Gutter`: one cell per **visual row**, resolved from every
//! region covering that row with the priority Design Language §3 fixes —
//! *trouble > attention > claude-unseen > none* — and degrading to `▎` where a
//! coloured block does not render (§8).
//!
//! The same column already exists *inside* the buffer view, as
//! [`crate::buffer_view::BufferView::state_column`]. What lands here is the
//! resolution behind it, plus the column for surfaces that want it without an
//! editor, which is why the node kind carries a `BufferId` and nothing else.
//!
//! # This module owns the ladder
//!
//! [`RegionState`] is what one region says about the rows it covers;
//! [`RegionState::mark`] is the one place a region's state becomes a tier; and
//! [`resolve`] folds a row's whole region set down to a single [`StateMark`].
//! Nothing else in the crate resolves priority — [`crate::buffer_view`] takes
//! the *answer* and paints it, which is what keeps this a ladder rather than
//! two ladders drifting apart.
//!
//! [`state_column`] is the fixture path: regions as row spans, resolved into
//! the `Vec<StateMark>` a view takes. Real regions arrive from the store at
//! `T041`, and this is the shape they arrive as.
//!
//! # Overlays are not states
//!
//! §7's state machine is *unseen ⇄ seen*, and it calls its overlays — ⚓
//! thread, ◉ watch, ■ diagnostic — orthogonal to it. §3's own render says which
//! of them reach the bar: row 17 (*"unseen — claude wrote, you haven't
//! looked"*) carries the claude hue, row 19 (*"diagnostic region"*) carries
//! trouble, row 18 (*"seen — marker cleared, line is plain"*) carries nothing,
//! and row 20 — *"anchored region — tint + undercurl"* — carries **no bar at
//! all**, saying its anchor with a row tint (`T087`) and an undercurl (`T085`)
//! instead.
//!
//! So a thread or a watch resolves to [`StateMark::None`]. The column is state;
//! an overlay is not one, and the drawing is what settles it.
//!
//! # The degradation is a form, not a second path
//!
//! [`Fill::Block`] is the background-coloured cell every mockup draws;
//! [`Fill::Marker`] is §8's *"markers become `▎`"*, the same hue moved to the
//! foreground for a terminal that cannot paint a background. Both go through
//! [`state_cell`], so there is one function that decides what a mark looks
//! like and the degraded form cannot quietly diverge from the full one.
//!
//! **Which form is a fact about the terminal, and this crate never touches
//! one** (TEAM.md: *"`surface` draws, and never touches a terminal"*). A host
//! drawing a [`GutterBar`] directly chooses; `Node::Gutter` carries no
//! capability prop, so composition draws [`Fill::Block`] until the protocol
//! grows a channel for it. Flagged rather than folded in — the view tree is
//! `spine`'s single writer.
//!
//! Owned by `surface`.

use core::ops::Range;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::widgets::Widget;

use crate::buffer_view::{STATE_BAR_WIDTH, StateMark};
use crate::theme::Theme;

/// §8's degraded marker: *"markers become `▎`"*. `U+258E LEFT ONE QUARTER
/// BLOCK` — one cell, present in default terminal fonts, and the same width as
/// the block it replaces.
pub const MARKER: &str = "▎";

// ---------------------------------------------------------------------------
// The ladder
// ---------------------------------------------------------------------------

/// What one region says about the rows it covers.
///
/// Design Language §12 gives this widget its input as `Vec<RegionState>` with
/// *"priority resolution baked in"*. The vocabulary is §7's state machine and
/// its overlays, plus §1's two non-claude tiers; [`mark`](RegionState::mark) is
/// where each one meets §3's ladder.
///
/// Fixtures until `T041`. Nothing here is a store type — a region *id*, its
/// anchors and its lifetime are the store's, and this enum is only the part the
/// column has to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionState {
    /// Claude wrote it and you have not looked yet — §7's `unseen`, §3's row
    /// 17. The claude hue, and the whole thesis of the gutter.
    Unseen,
    /// The marker is cleared: §3's row 18, *"seen — marker cleared, line is
    /// plain"*. §7 makes seen-state the only mutable flag the user owns, and
    /// this is what it looks like from the column.
    Seen,
    /// ⚓ a thread anchored to the region — one of §7's orthogonal overlays,
    /// drawn as a row tint and an undercurl (§3's row 20), never as a bar.
    Thread,
    /// ◉ a watch over the region (§7's overlays, §2's lexicon). An overlay, so
    /// the column says nothing about it.
    Watch,
    /// ■ a diagnostic over the region — §7's third overlay, and the one §3's
    /// row 19 *does* draw in the bar, because a diagnostic is trouble (§1).
    Diagnostic,
    /// A failure: §1's trouble is *"deletions, failures, disconnects"*, and §3
    /// gives a failed region its own row tint. Same tier as a diagnostic.
    Failure,
    /// Waiting on you — §2's `!` is *"needs you — question or permission"* and
    /// §1's attention is *"waiting, paused, dirty, permission"*.
    NeedsYou,
}

impl RegionState {
    /// Every state, in the enum's own order.
    ///
    /// Exhaustive by construction: a variant added without a line here is a
    /// compile error at the array's length, and `T031`'s acceptance —
    /// *"priority resolution unit-tested across all overlap combinations"* —
    /// enumerates the power set of this array in every order.
    pub const ALL: [Self; 7] = [
        Self::Unseen,
        Self::Seen,
        Self::Thread,
        Self::Watch,
        Self::Diagnostic,
        Self::Failure,
        Self::NeedsYou,
    ];

    /// The tier this state contributes to the column. **The one place a
    /// region's state becomes a mark.**
    #[must_use]
    pub const fn mark(self) -> StateMark {
        match self {
            // Overlays and a cleared marker say nothing in column 1 — see the
            // module header, and §3's rows 18 and 20.
            Self::Seen | Self::Thread | Self::Watch => StateMark::None,
            Self::Unseen => StateMark::ClaudeUnseen,
            Self::NeedsYou => StateMark::Attention,
            Self::Diagnostic | Self::Failure => StateMark::Trouble,
        }
    }
}

/// A region's state over a span of visual rows.
///
/// The rows are half-open and live in the same coordinate space as
/// [`crate::buffer_view::Viewport::top_row`] — visual rows, so folds and
/// soft-wrap continuations are already counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpan {
    /// Which visual rows the region covers.
    pub rows: Range<usize>,
    /// What it says about them.
    pub state: RegionState,
}

impl RegionSpan {
    /// A region in `state` covering `rows`.
    #[must_use]
    pub const fn new(rows: Range<usize>, state: RegionState) -> Self {
        Self { rows, state }
    }
}

/// **The ladder.** A row's whole region set, resolved to one mark.
///
/// §3, verbatim: *"priority: trouble > attention > claude"*. Written as a fold
/// over [`raise`], so it is a maximum rather than a scan — the answer cannot
/// depend on the order regions arrive in, which matters because the store has
/// no ordering to promise and a row's set is a set.
///
/// An empty set is [`StateMark::None`]: a row nothing covers has nothing to
/// say, which is the ladder's floor rather than a special case.
#[must_use]
pub fn resolve<I>(states: I) -> StateMark
where
    I: IntoIterator<Item = RegionState>,
{
    states
        .into_iter()
        .map(RegionState::mark)
        .fold(StateMark::None, raise)
}

/// One mark per visual row, `rows` long, from the regions covering them.
///
/// The fixture path `T031` ships with and the shape `T041` fills in: the store
/// answers *which regions cover which rows*, and this is what turns that into
/// the column [`crate::buffer_view::BufferView::state_column`] and
/// [`GutterBar`] both take. Spans are clamped to `rows`, so a region that
/// outlives an edit cannot write past the end of the buffer.
#[must_use]
pub fn state_column(regions: &[RegionSpan], rows: usize) -> Vec<StateMark> {
    let mut column = vec![StateMark::None; rows];
    for region in regions {
        let mark = region.state.mark();
        let start = region.rows.start.min(rows);
        let end = region.rows.end.min(rows);
        for cell in column.get_mut(start..end).unwrap_or_default() {
            *cell = raise(*cell, mark);
        }
    }
    column
}

/// The higher of two marks on §3's ladder.
///
/// Commutative and associative by construction, which is what makes
/// [`resolve`] order-independent.
#[must_use]
const fn raise(current: StateMark, next: StateMark) -> StateMark {
    if rank(next) > rank(current) {
        next
    } else {
        current
    }
}

/// A mark's rung. Written as an exhaustive match rather than read off the
/// enum's discriminants, so a fifth mark cannot be given a place in the ladder
/// by accident — it has to be given one here.
const fn rank(mark: StateMark) -> u8 {
    match mark {
        StateMark::None => 0,
        StateMark::ClaudeUnseen => 1,
        StateMark::Attention => 2,
        StateMark::Trouble => 3,
    }
}

// ---------------------------------------------------------------------------
// The cell
// ---------------------------------------------------------------------------

/// How the state bar draws a mark.
///
/// §8 names one degradation for this column and no others, so this is a pair
/// rather than a scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Fill {
    /// A background-coloured cell — what every mockup draws (§3, `1a`).
    #[default]
    Block,
    /// [`MARKER`] in the mark's own hue, for a terminal that cannot paint a
    /// background (§8).
    Marker,
}

/// What one row's cell is: its symbol and its style.
///
/// **The one place a [`StateMark`] becomes something drawable**, both forms
/// through the same match. Public because the column is drawn in two places —
/// here and inside [`crate::buffer_view::BufferView`] — and one of them
/// currently has a private copy of the hue lookup; see this task's report.
#[must_use]
pub fn state_cell(mark: StateMark, theme: &Theme, fill: Fill) -> (&'static str, Style) {
    let ground = theme.neutrals.ground;
    match (fill, mark) {
        // Nothing to say: ground, in both forms. Written rather than skipped,
        // so a stale symbol from whatever drew underneath cannot survive in
        // the gutter (the reason `BufferView` writes this cell too).
        (_, StateMark::None) => (" ", Style::new().bg(ground)),
        (Fill::Block, mark) => (" ", Style::new().bg(hue(mark, theme))),
        (Fill::Marker, mark) => (MARKER, Style::new().fg(hue(mark, theme)).bg(ground)),
    }
}

/// §1's actor hues, and ground for "nothing" so the bar is invisible rather
/// than absent.
fn hue(mark: StateMark, theme: &Theme) -> Color {
    match mark {
        StateMark::None => theme.neutrals.ground,
        StateMark::ClaudeUnseen => theme.actors.claude,
        StateMark::Attention => theme.actors.attention,
        StateMark::Trouble => theme.actors.trouble,
    }
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// The 1-cell state column (`T031`).
///
/// Takes `&Theme` and already-resolved marks, indexed by **visual row**, and
/// owns no state of its own. Rows past the end of the slice draw
/// [`StateMark::None`], so a column shorter than the area is legal and means
/// "nothing down there" rather than "undefined".
///
/// It writes exactly [`STATE_BAR_WIDTH`] cells per row and never touches the
/// rest of the area: a gutter beside something else cannot paint over its
/// neighbour, whatever constraint composition gave it.
#[derive(Debug, Clone, Copy)]
pub struct GutterBar<'a> {
    marks: &'a [StateMark],
    theme: &'a Theme,
    top_row: usize,
    fill: Fill,
}

impl<'a> GutterBar<'a> {
    /// A column over `marks`, painted with `theme`, from the top of the buffer
    /// and at full fidelity.
    #[must_use]
    pub const fn new(marks: &'a [StateMark], theme: &'a Theme) -> Self {
        Self {
            marks,
            theme,
            top_row: 0,
            fill: Fill::Block,
        }
    }

    /// Which visual row the first drawn row is.
    ///
    /// The same number as [`crate::buffer_view::Viewport::top_row`] — a column
    /// beside a buffer view is handed that buffer's, so the two scroll
    /// together. It is a parameter and not a read of anything: rendering
    /// cannot move a viewport (invariant 3).
    #[must_use]
    pub const fn top_row(mut self, row: usize) -> Self {
        self.top_row = row;
        self
    }

    /// Draw §8's degraded form instead of the coloured block.
    #[must_use]
    pub const fn fill(mut self, fill: Fill) -> Self {
        self.fill = fill;
        self
    }

    /// The mark at a visual row. Past the end of the column: nothing.
    #[must_use]
    pub fn mark_at(&self, visual_row: usize) -> StateMark {
        self.marks.get(visual_row).copied().unwrap_or_default()
    }
}

impl Widget for GutterBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let width = STATE_BAR_WIDTH.min(area.width);
        for dy in 0..area.height {
            let visual_row = self.top_row.saturating_add(dy as usize);
            let (symbol, style) = state_cell(self.mark_at(visual_row), self.theme, self.fill);
            for dx in 0..width {
                buf.set_string(area.x + dx, area.y + dy, symbol, style);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer_view::{BufferView, Editor, configure};

    fn theme() -> Theme {
        Theme::phosphor_dark()
    }

    // -- the ladder ---------------------------------------------------------

    /// Every state's tier, as a table read off §3 and §7 rather than off the
    /// implementation. The mapping the exhaustive test below then assumes.
    #[test]
    fn each_state_lands_on_the_tier_the_language_gives_it() {
        for (state, expected) in [
            (RegionState::Unseen, StateMark::ClaudeUnseen),
            (RegionState::Seen, StateMark::None),
            (RegionState::Thread, StateMark::None),
            (RegionState::Watch, StateMark::None),
            (RegionState::Diagnostic, StateMark::Trouble),
            (RegionState::Failure, StateMark::Trouble),
            (RegionState::NeedsYou, StateMark::Attention),
        ] {
            assert_eq!(state.mark(), expected, "{state:?}");
        }
        // And the table is the whole enum, not a sample of it.
        assert_eq!(RegionState::ALL.len(), 7);
    }

    /// §3's ladder, said a second way: the highest tier anything in the set
    /// carries wins. Deliberately not a fold — an independent oracle for the
    /// exhaustive test, so the two would have to be wrong the same way.
    fn ladder(states: &[RegionState]) -> StateMark {
        for tier in [
            StateMark::Trouble,
            StateMark::Attention,
            StateMark::ClaudeUnseen,
        ] {
            if states.iter().any(|state| state.mark() == tier) {
                return tier;
            }
        }
        StateMark::None
    }

    /// Every ordering of `items`.
    fn orderings(items: &[RegionState]) -> Vec<Vec<RegionState>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let head = rest.remove(i);
            for mut tail in orderings(&rest) {
                tail.insert(0, head);
                out.push(tail);
            }
        }
        out
    }

    /// **`T031`'s acceptance, exhaustively.** Every subset of the seven states
    /// — all 128 of them — in every order, which is 13 700 sequences in total.
    ///
    /// The space is small enough to enumerate, so it is enumerated rather than
    /// sampled: a row is covered by whichever regions happen to cover it, in
    /// whatever order the store hands them over, and every one of those cases
    /// is one of these.
    #[test]
    fn priority_resolves_across_every_overlap_combination() {
        let all = RegionState::ALL;
        let mut sequences = 0usize;
        let mut subsets = 0usize;

        for bits in 0..(1u32 << all.len()) {
            let subset: Vec<RegionState> = all
                .iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, state)| *state)
                .collect();
            subsets += 1;

            let expected = ladder(&subset);
            for ordering in orderings(&subset) {
                sequences += 1;
                assert_eq!(
                    resolve(ordering.iter().copied()),
                    expected,
                    "{ordering:?} must resolve to {expected:?}"
                );
            }
        }

        assert_eq!(subsets, 128, "every subset of the seven states");
        assert_eq!(sequences, 13_700, "every subset, in every order");
    }

    #[test]
    fn a_row_nothing_covers_has_nothing_to_say() {
        assert_eq!(resolve([]), StateMark::None);
    }

    #[test]
    fn repeating_a_state_changes_nothing() {
        // Two regions in the same state overlapping one row is the common
        // case, not an edge case: claude wrote twice and you looked at
        // neither.
        for a in RegionState::ALL {
            for b in RegionState::ALL {
                let once = resolve([a, b]);
                assert_eq!(resolve([a, b, a, b, a]), once, "{a:?} + {b:?}");
                assert_eq!(resolve([b, a]), once, "{a:?} + {b:?} is a set");
            }
        }
    }

    #[test]
    fn an_overlay_never_reaches_the_bar() {
        // §3's row 20: an anchored region draws tint + undercurl and an empty
        // state cell. A thread, a watch and a cleared marker are all that,
        // together and apart.
        for states in [
            vec![RegionState::Thread],
            vec![RegionState::Watch],
            vec![RegionState::Seen],
            vec![RegionState::Seen, RegionState::Thread, RegionState::Watch],
        ] {
            assert_eq!(
                resolve(states.iter().copied()),
                StateMark::None,
                "{states:?}"
            );
        }
        // And an overlay cannot suppress a state either — the ladder only ever
        // raises.
        assert_eq!(
            resolve([RegionState::Thread, RegionState::Unseen, RegionState::Watch]),
            StateMark::ClaudeUnseen
        );
    }

    #[test]
    fn trouble_beats_attention_beats_claude() {
        // The ladder read as three pairwise claims, which is how §3 words it.
        assert_eq!(
            resolve([RegionState::Unseen, RegionState::NeedsYou]),
            StateMark::Attention
        );
        assert_eq!(
            resolve([RegionState::NeedsYou, RegionState::Diagnostic]),
            StateMark::Trouble
        );
        assert_eq!(
            resolve([RegionState::Unseen, RegionState::Failure]),
            StateMark::Trouble
        );
    }

    // -- regions onto rows --------------------------------------------------

    #[test]
    fn spans_resolve_per_row_where_they_overlap() {
        //  rows:  0  1  2  3  4  5
        //  unseen    [-----]
        //  needs        [--]
        //  diag            [-----]
        let column = state_column(
            &[
                RegionSpan::new(1..4, RegionState::Unseen),
                RegionSpan::new(2..4, RegionState::NeedsYou),
                RegionSpan::new(3..6, RegionState::Diagnostic),
            ],
            6,
        );
        assert_eq!(
            column,
            vec![
                StateMark::None,
                StateMark::ClaudeUnseen,
                StateMark::Attention,
                StateMark::Trouble,
                StateMark::Trouble,
                StateMark::Trouble,
            ]
        );
    }

    #[test]
    fn a_span_past_the_end_is_clamped_rather_than_a_panic() {
        // A region that outlived an edit that shortened the file. The third is
        // inverted — `end` before `start`, which is what a span whose two
        // anchors crossed during a rewrite looks like — and it must draw
        // nothing rather than panic. Built field-by-field because the literal
        // form is a clippy error in a test, which is the lint doing its job.
        let inverted = RegionSpan {
            rows: Range { start: 4, end: 2 },
            state: RegionState::Failure,
        };
        let column = state_column(
            &[
                RegionSpan::new(0..999, RegionState::Unseen),
                RegionSpan::new(90..95, RegionState::Diagnostic),
                inverted,
            ],
            3,
        );
        assert_eq!(column, vec![StateMark::ClaudeUnseen; 3]);
        assert!(state_column(&[], 0).is_empty());
    }

    // -- the widget ---------------------------------------------------------

    fn render(marks: &[StateMark], theme: &Theme, fill: Fill, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        GutterBar::new(marks, theme)
            .fill(fill)
            .render(area, &mut buf);
        buf
    }

    const EVERY_MARK: [StateMark; 4] = [
        StateMark::None,
        StateMark::ClaudeUnseen,
        StateMark::Attention,
        StateMark::Trouble,
    ];

    #[test]
    fn the_column_is_one_coloured_cell_per_row() {
        let theme = theme();
        let area = Rect::new(0, 0, 4, 4);
        let buf = render(&EVERY_MARK, &theme, Fill::Block, area);

        let expected = [
            theme.neutrals.ground,
            theme.actors.claude,
            theme.actors.attention,
            theme.actors.trouble,
        ];
        for (y, want) in expected.into_iter().enumerate() {
            let cell = &buf[(0, y as u16)];
            assert_eq!(cell.symbol(), " ", "the bar is a colour, not a glyph");
            assert_eq!(cell.bg, want, "row {y}");
        }
    }

    #[test]
    fn the_hues_are_the_themes_and_are_read_per_frame() {
        // A second theme with unmistakably different values in those fields
        // proves the widget reads the fields rather than agreeing with them by
        // coincidence. The substitutes are other *theme* colours, so no value
        // enters this file (`T006` would reject one that did).
        let mut recoloured = theme();
        recoloured.actors.claude = recoloured.actors.steel;
        recoloured.actors.attention = recoloured.actors.you;
        recoloured.actors.trouble = recoloured.actors.transient;

        let area = Rect::new(0, 0, 1, 4);
        let buf = render(&EVERY_MARK, &recoloured, Fill::Block, area);
        assert_eq!(buf[(0, 1)].bg, recoloured.actors.steel);
        assert_eq!(buf[(0, 2)].bg, recoloured.actors.you);
        assert_eq!(buf[(0, 3)].bg, recoloured.actors.transient);
    }

    #[test]
    fn the_degraded_form_is_the_marker_in_the_same_hue() {
        // §8: "markers become ▎". Same hue, moved to the foreground; a row
        // with nothing to say stays blank rather than becoming a grey marker.
        let theme = theme();
        let area = Rect::new(0, 0, 1, 4);
        let buf = render(&EVERY_MARK, &theme, Fill::Marker, area);

        assert_eq!(buf[(0, 0)].symbol(), " ");
        assert_eq!(buf[(0, 0)].bg, theme.neutrals.ground);
        for (y, want) in [
            (1u16, theme.actors.claude),
            (2, theme.actors.attention),
            (3, theme.actors.trouble),
        ] {
            assert_eq!(buf[(0, y)].symbol(), MARKER, "row {y}");
            assert_eq!(buf[(0, y)].fg, want, "row {y}");
            assert_eq!(buf[(0, y)].bg, theme.neutrals.ground, "row {y}");
        }
    }

    #[test]
    fn the_two_forms_say_the_same_thing() {
        // The degradation is a form of one decision, not a second decision:
        // whatever the block draws in the background, the marker draws in the
        // foreground, mark for mark.
        let theme = theme();
        for mark in EVERY_MARK {
            let (block_symbol, block) = state_cell(mark, &theme, Fill::Block);
            let (marker_symbol, marker) = state_cell(mark, &theme, Fill::Marker);
            if mark == StateMark::None {
                assert_eq!(block_symbol, marker_symbol);
                assert_eq!(block.bg, marker.bg);
            } else {
                assert_eq!(marker_symbol, MARKER);
                assert_eq!(block.bg, marker.fg, "{mark:?}");
            }
        }
    }

    #[test]
    fn the_column_scrolls_with_the_viewport_it_is_given() {
        let theme = theme();
        let marks = [
            StateMark::None,
            StateMark::None,
            StateMark::Trouble,
            StateMark::ClaudeUnseen,
        ];
        let area = Rect::new(0, 0, 1, 2);
        let mut buf = Buffer::empty(area);
        GutterBar::new(&marks, &theme)
            .top_row(2)
            .render(area, &mut buf);
        assert_eq!(buf[(0, 0)].bg, theme.actors.trouble);
        assert_eq!(buf[(0, 1)].bg, theme.actors.claude);
    }

    #[test]
    fn rows_past_the_end_of_the_column_are_ground() {
        let theme = theme();
        let area = Rect::new(0, 0, 1, 5);
        let buf = render(&[StateMark::Trouble], &theme, Fill::Block, area);
        assert_eq!(buf[(0, 0)].bg, theme.actors.trouble);
        for y in 1..area.height {
            assert_eq!(buf[(0, y)].bg, theme.neutrals.ground, "row {y}");
        }
        // …and so is a viewport scrolled past the end entirely.
        let mut buf = Buffer::empty(area);
        GutterBar::new(&[StateMark::Trouble], &theme)
            .top_row(usize::MAX - 1)
            .render(area, &mut buf);
        for y in 0..area.height {
            assert_eq!(buf[(0, y)].bg, theme.neutrals.ground, "row {y}");
        }
    }

    #[test]
    fn the_column_writes_one_cell_and_no_more() {
        // Handed four columns, it paints one. A gutter beside something else
        // cannot paint over its neighbour.
        let theme = theme();
        let area = Rect::new(2, 1, 4, 3);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 5));
        for x in 0..8 {
            for y in 0..5 {
                buf[(x, y)].set_symbol("#");
            }
        }
        GutterBar::new(&[StateMark::Trouble; 3], &theme).render(area, &mut buf);
        for y in 0..5u16 {
            for x in 0..8u16 {
                let painted = x == 2 && (1..4).contains(&y);
                assert_eq!(
                    buf[(x, y)].symbol(),
                    if painted { " " } else { "#" },
                    "cell ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn a_degenerate_area_does_not_panic() {
        let theme = theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 4));
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(3, 3, 1, 1),
            Rect::new(0, 0, 40, 40),
            Rect::new(9, 9, 4, 4),
        ] {
            GutterBar::new(&EVERY_MARK, &theme).render(area, &mut buf);
        }
    }

    // -- the seam with `BufferView` -----------------------------------------

    /// The two paints agree, cell for cell.
    ///
    /// [`crate::buffer_view::BufferView`] keeps a private hue lookup of its
    /// own — a `StateMark` becomes a colour in two files today, which is one
    /// too many. Until they are collapsed (see this task's report), this test
    /// is what stops them drifting: change either and it fails.
    #[test]
    fn the_column_agrees_with_the_one_inside_the_buffer_view() {
        let theme = theme();
        let mut editor =
            Editor::new("rust", "one\ntwo\nthree\nfour\n", Vec::new()).expect("editor");
        configure(&mut editor, &theme);
        let area = Rect::new(0, 0, 20, 4);

        let mut view = Buffer::empty(area);
        BufferView::new(&editor, &theme)
            .state_column(&EVERY_MARK)
            .render(area, &mut view);

        let mut bar = Buffer::empty(area);
        GutterBar::new(&EVERY_MARK, &theme).render(area, &mut bar);

        for y in 0..area.height {
            assert_eq!(bar[(0, y)].symbol(), view[(0, y)].symbol(), "row {y}");
            assert_eq!(bar[(0, y)].bg, view[(0, y)].bg, "row {y}");
            assert_eq!(bar[(0, y)].fg, view[(0, y)].fg, "row {y}");
        }
    }
}
