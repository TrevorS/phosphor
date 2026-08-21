//! `TabBar` (`T089`) — Design Language §5's top chrome strip.
//!
//! Draws [`phosphor_core::view::Node::TabBar`]: one row, one tab per pane, and
//! the pane count at the right. §5 gives it its brief in one clause — *"tab bar
//! (top, appears only with 2+ panes — vim-style but flat: active tab carries a
//! 2px actor-colored top rule and bright text, inactive tabs are meta-gray,
//! unseen counts ride each tab)"* — and §8 gives it its budget: *"Three strips
//! of chrome: tab bar 1 row (only with 2+ panes), statusline 1 row, tmux bar 1
//! row."*
//!
//! **Appearing only at 2+ panes is not this widget's rule.** A strip that
//! decides whether it exists has to be given a row first, and a row given to a
//! strip that draws nothing is a row the buffer lost. So the condition lives
//! where the rows are handed out — `crates/phosphor/src/main.rs`'s `Geometry`,
//! which takes the row off the top of the pane area only when the tree has a
//! second leaf. This file draws whatever it is given; the sibling rule is
//! `Node::Empty`'s, and both are tested where they live.
//!
//! # The two rules that have no row, recorded rather than approximated
//!
//! §5's strip is drawn in the design docs as CSS, and two of its lines are
//! **sub-cell borders**: the strip carries `border-bottom: 1px solid #1d241d`
//! ([`Chrome::tab_bar_rule`]) and the active tab carries `border-top: 2px solid`
//! an actor colour. A terminal cell has no top edge and no bottom edge to
//! paint, and §8 fixes the whole strip at one row, so neither has anywhere to
//! go: a rule of its own would be a second row and §8 spends the budget it
//! would come out of.
//!
//! What the borders *carry* survives, because none of it is only in the border:
//!
//! * **Which tab is active** — [`NeutralRamp::bright_text`] on
//!   [`Chrome::statusline`], against [`NeutralRamp::meta`] on
//!   [`Chrome::tab_bar`] for the rest. That is §5's own second clause and the
//!   mockup's own two colours.
//! * **How many unseen regions ride each tab** — the `●n` counter, in the
//!   claude green, on active and inactive tabs alike.
//!
//! What does not survive is the **actor colour** the top rule carried, and with
//! it the one drawable consequence of [`Tab::kind`]. §7 is why it costs so
//! little: *"Your own edits never create regions: the machine tracks claude
//! only"*, so the `●n` a tab carries is claude's whatever the pane holds, and
//! all three [`PaneKind`]s are claude's work — a buffer he wrote in, the
//! transcript, and a pane he emitted as a view tree. A [`PaneKind`]-to-actor
//! map would therefore be one colour written three ways.
//!
//! Recorded in `docs/OPEN-QUESTIONS.md` rather than folded in, per CLAUDE.md's
//! *"If the design and the build disagree, flag it; do not fold the change
//! in."* The disagreement here is between §5 and §8 rather than between the
//! design and the build, which is why the finding names both.
//!
//! # The pane count is derived, not carried
//!
//! §5's strip ends in `3 panes`, in the line-number grey the mockup gives it
//! (`#414b42`). [`Node::TabBar`] carries no such field and the Component
//! Breakdown's input spec — *"Input: `Vec<TabVM { title, kind, unseen }>`"* —
//! has no room for one. It needs none: composition's contract is one tab per
//! pane, so the count **is** `tabs.len()`, and deriving it here is cheaper than
//! a prop that could disagree with the tabs beside it.
//!
//! # Shedding, when the tabs do not fit
//!
//! §11 is *"narrow terminals drop, never squeeze"*, and §5 is *"never wraps; a
//! second line is a bug"*. Two rungs, in this order:
//!
//! 1. **The count goes first.** It is the only segment whose whole content is
//!    recoverable from what stays on screen — you can count the tabs.
//! 2. **Then the run scrolls to hold the active tab.** Tabs are dropped from
//!    the *left* until the active one's right edge fits, which is what every
//!    tabbed editor does and the only rule under which the strip cannot lose
//!    the thing it exists to point at. Whatever still overruns is clipped at
//!    the right edge by [`write`].
//!
//! An active tab wider than the whole strip is clipped like anything else —
//! there is no rung below "show the active tab".
//!
//! [`Chrome::statusline`]: crate::theme::Chrome::statusline
//! [`Chrome::tab_bar`]: crate::theme::Chrome::tab_bar
//! [`Chrome::tab_bar_rule`]: crate::theme::Chrome::tab_bar_rule
//! [`NeutralRamp::bright_text`]: crate::theme::NeutralRamp::bright_text
//! [`NeutralRamp::meta`]: crate::theme::NeutralRamp::meta
//! [`Node::TabBar`]: phosphor_core::view::Node::TabBar
//! [`PaneKind`]: phosphor_core::request::PaneKind
//!
//! Owned by `surface`.

use phosphor_core::view::Tab;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// Cells of air either side of a tab's label.
///
/// §5's strip draws `padding: 0 14px` on every tab at an ~6.6px cell — two
/// cells and a sliver, and the sliver is not a cell.
pub const PAD_COLS: u16 = 2;

/// Cells between a tab's title and its `●n` counter. The mockup separates them
/// with a single space.
const COUNTER_GAP: u16 = 1;

/// The strip's right inset, before the pane count. §5 draws
/// `padding-right: 10px`.
const RIGHT_INSET: u16 = 1;

/// §2's unseen glyph, which is what rides a tab.
const UNSEEN: &str = "●";

/// The word after the pane count. Plural always, because the strip does not
/// exist at one pane — see the module docs.
const PANES: &str = " panes";

/// §5's top chrome strip, over the tabs composition handed it.
///
/// Built per frame and thrown away — it borrows the tabs and the theme and owns
/// nothing.
#[derive(Debug, Clone, Copy)]
pub struct TabBar<'a> {
    tabs: &'a [Tab],
    theme: &'a Theme,
}

impl<'a> TabBar<'a> {
    /// A strip over `tabs`, left to right.
    #[must_use]
    pub const fn new(tabs: &'a [Tab], theme: &'a Theme) -> Self {
        Self { tabs, theme }
    }

    /// What one tab reads, and how wide it is with its air.
    fn label(tab: &Tab) -> String {
        let mut text = tab.title.clone();
        if tab.unseen > 0 {
            for _ in 0..COUNTER_GAP {
                text.push(' ');
            }
            text.push_str(UNSEEN);
            text.push_str(&tab.unseen.to_string());
        }
        text
    }

    /// The whole width one tab occupies, air included.
    fn width(label: &str) -> u16 {
        PAD_COLS
            .saturating_add(cells(label))
            .saturating_add(PAD_COLS)
    }
}

impl Widget for TabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() || self.tabs.is_empty() {
            return;
        }

        // §5's field, painted before anything sits on it. The same contract the
        // statusline's ground is drawn under in `main.rs`'s `draw`: a
        // `Node::Line` cannot say what ground it is painted on, so the surface
        // that *is* the strip paints its own.
        let ground = Style::new()
            .fg(self.theme.neutrals.meta)
            .bg(self.theme.chrome.tab_bar);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_symbol(" ").set_style(ground);
            }
        }

        let labels: Vec<String> = self.tabs.iter().map(TabBar::label).collect();
        let widths: Vec<u16> = labels.iter().map(|label| Self::width(label)).collect();
        let total: u16 = widths
            .iter()
            .copied()
            .fold(0u16, |sum, width| sum.saturating_add(width));

        // Rung one: the count, which is only drawn when the tabs leave room for
        // it whole. A truncated `3 pan` is worse than no count at all — §11
        // drops, and this is the drop.
        let count = format!("{}{PANES}", self.tabs.len());
        let count_width = cells(&count).saturating_add(RIGHT_INSET);
        let counted = total.saturating_add(count_width) <= area.width;
        if counted {
            let x = area.right().saturating_sub(count_width);
            write(
                buf,
                area,
                x,
                &count,
                Style::new()
                    .fg(self.theme.neutrals.line_numbers)
                    .bg(self.theme.chrome.tab_bar),
            );
        }

        // Rung two: drop from the left until the active tab's right edge fits.
        // The room is the whole strip once the count has gone, which is why
        // this rung is measured after that one.
        let room = if counted {
            area.width.saturating_sub(count_width)
        } else {
            area.width
        };
        let first = first_shown(&widths, self.tabs, room);

        let mut x = area.x;
        for (index, label) in labels.iter().enumerate().skip(first) {
            let tab = &self.tabs[index];
            let style = if tab.active {
                Style::new()
                    .fg(self.theme.neutrals.bright_text)
                    .bg(self.theme.chrome.statusline)
            } else {
                ground
            };
            // The air is the tab's, so the active tab's field runs under its
            // padding too — §5 draws the background on the whole tab, not on
            // its letters.
            let end = x.saturating_add(widths[index]).min(area.right());
            for cell in x..end {
                buf[(cell, area.y)].set_symbol(" ").set_style(style);
            }
            write(buf, area, x.saturating_add(PAD_COLS), label, style);
            x = end;
            if x >= area.right() {
                break;
            }
        }
    }
}

/// The first tab to draw, so the active one's right edge lands inside `room`.
///
/// Zero whenever everything fits, which is the common case and the one worth
/// being free. A run with no active tab starts at zero as well: composition
/// always marks one, and guessing on its behalf would hide the bug.
fn first_shown(widths: &[u16], tabs: &[Tab], room: u16) -> usize {
    let Some(active) = tabs.iter().position(|tab| tab.active) else {
        return 0;
    };
    let mut first = 0;
    loop {
        let through: u16 = widths[first..=active]
            .iter()
            .copied()
            .fold(0u16, |sum, width| sum.saturating_add(width));
        if through <= room || first == active {
            return first;
        }
        first += 1;
    }
}

/// Write `text` at `x` on `area`'s first row, clipped to the area.
///
/// The same shape `key_hints` and `interpret` write with, and for the same
/// reason: [`Buffer::set_stringn`] clamps `x` and never `y`, so the clip has to
/// be the caller's.
fn write(buf: &mut Buffer, area: Rect, x: u16, text: &str, style: Style) {
    if area.is_empty() || x >= area.right() {
        return;
    }
    let room = area.right() - x;
    buf.set_stringn(x, area.y, text, room as usize, style);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::TabBar;
    use crate::theme::Theme;
    use phosphor_core::request::PaneKind;
    use phosphor_core::view::Tab;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::widgets::Widget;

    fn tab(title: &str, unseen: u32, active: bool) -> Tab {
        Tab {
            title: title.to_owned(),
            kind: PaneKind::Buffer,
            unseen,
            active,
        }
    }

    /// §5's own strip: two files with three unseen apiece and the transcript,
    /// the first of them focused.
    fn strip() -> Vec<Tab> {
        vec![
            tab("src/retry.rs", 3, true),
            tab("src/fetch.rs", 3, false),
            Tab {
                title: "transcript".to_owned(),
                kind: PaneKind::Transcript,
                unseen: 0,
                active: false,
            },
        ]
    }

    fn draw(tabs: &[Tab], width: u16) -> Buffer {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        TabBar::new(tabs, &theme).render(area, &mut buf);
        buf
    }

    fn row(buf: &Buffer) -> String {
        (buf.area.x..buf.area.right())
            .map(|x| buf[(x, 0)].symbol())
            .collect::<String>()
    }

    /// **§5's strip, cell for cell.** Two cells of air either side of every
    /// tab, the `●n` counters riding the two that have unseen work, the
    /// transcript carrying none, and the pane count at the right.
    #[test]
    fn the_strip_reads_the_way_section_five_draws_it() {
        let drawn = row(&draw(&strip(), 80));
        assert_eq!(
            drawn.trim_end(),
            "  src/retry.rs ●3    src/fetch.rs ●3    transcript                      3 panes"
        );
        // The count sits against the right edge, one cell in.
        assert!(drawn.ends_with("3 panes "), "{drawn:?}");
    }

    /// **The active tab is the only one on the statusline's field**, and the
    /// only one in bright text. That pair is what §5 leaves a terminal after
    /// the top rule turns out to have no row — see the module docs.
    #[test]
    fn the_active_tab_is_bright_on_its_own_field_and_the_rest_are_meta() {
        let theme = Theme::phosphor_dark();
        let buf = draw(&strip(), 80);
        // `src/retry.rs` runs from 0 to `2 + 12 + 3 + 2` = 19 inclusive of its
        // air; `src/fetch.rs` starts at 19.
        let active = &buf[(4u16, 0u16)];
        assert_eq!(active.fg, theme.neutrals.bright_text);
        assert_eq!(active.bg, theme.chrome.statusline);
        // Its air is on the same field — §5 paints the tab, not its letters.
        assert_eq!(buf[(0u16, 0u16)].bg, theme.chrome.statusline);
        assert_eq!(buf[(18u16, 0u16)].bg, theme.chrome.statusline);

        let inactive = &buf[(23u16, 0u16)];
        assert_eq!(inactive.fg, theme.neutrals.meta);
        assert_eq!(inactive.bg, theme.chrome.tab_bar);
        // And so is the ground either side of the whole run.
        assert_eq!(buf[(70u16, 0u16)].bg, theme.chrome.tab_bar);
    }

    /// A tab with nothing unseen draws no counter at all — §5's `●n` is a
    /// count, and `Node::Counter`'s own rule is that *"zero draws nothing at
    /// all"*.
    #[test]
    fn a_tab_with_no_unseen_work_draws_no_counter() {
        let drawn = row(&draw(&[tab("a.rs", 0, true), tab("b.rs", 0, false)], 40));
        assert!(!drawn.contains('●'), "{drawn:?}");
        assert_eq!(drawn.trim_end(), "  a.rs    b.rs                  2 panes");
    }

    /// §11's first rung. The count is the only segment whose whole content is
    /// recoverable from what stays — you can count the tabs — so it is what
    /// goes when the tabs need the room.
    #[test]
    fn the_pane_count_is_the_first_thing_shed() {
        let wide = row(&draw(&strip(), 80));
        assert!(wide.contains("3 panes"), "{wide:?}");
        // The three tabs are 19 + 19 + 14 = 52 cells; `3 panes` plus its inset
        // is 8. At 58 there is room for the tabs and not for both.
        let narrow = row(&draw(&strip(), 58));
        assert!(!narrow.contains("panes"), "{narrow:?}");
        assert!(narrow.contains("transcript"), "{narrow:?}");
        // Never a truncated word — the rung drops, it does not squeeze.
        assert!(!narrow.contains("pan"), "{narrow:?}");
    }

    /// §11's second rung, and the rule the strip exists for: whatever else
    /// goes, the tab you are looking at does not.
    #[test]
    fn a_narrow_strip_scrolls_to_hold_the_active_tab() {
        let mut tabs = strip();
        tabs[0].active = false;
        tabs[2].active = true;
        // Room for the transcript's 14 cells and one of the files' 19, not all
        // three.
        let drawn = row(&draw(&tabs, 34));
        assert!(drawn.contains("transcript"), "{drawn:?}");
        assert!(!drawn.contains("retry"), "{drawn:?}");
        assert!(drawn.contains("fetch"), "{drawn:?}");
    }

    /// The last rung is the right edge, and an active tab wider than the whole
    /// strip has nothing below it to drop. It is clipped like any other text —
    /// §5's *"never wraps; a second line is a bug"* holds either way.
    #[test]
    fn an_active_tab_wider_than_the_strip_is_clipped_not_wrapped() {
        let tabs = vec![
            tab("a/very/long/path/to/somewhere.rs", 0, true),
            tab("b.rs", 0, false),
        ];
        let buf = draw(&tabs, 12);
        assert_eq!(buf.area.height, 1);
        assert_eq!(row(&buf), "  a/very/lon");
    }

    /// A strip with no tabs is the frame where composition answered
    /// `Node::Empty` and the geometry gave no row — but the widget is still
    /// reachable through a hand-built tree, and drawing nothing is what it owes
    /// that caller. Not even its ground, which would be a strip.
    #[test]
    fn no_tabs_draws_nothing_at_all() {
        let buf = draw(&[], 40);
        assert_eq!(row(&buf), " ".repeat(40));
        let theme = Theme::phosphor_dark();
        assert_ne!(buf[(0u16, 0u16)].bg, theme.chrome.tab_bar);
    }

    /// Zero-width, zero-height and a rect past the buffer all have to be
    /// survivable: `Buffer::set_stringn` clamps `x` and never `y`, and a rect
    /// that outlives its buffer panics the editor.
    #[test]
    fn a_degenerate_area_is_survivable() {
        let theme = Theme::phosphor_dark();
        let tabs = strip();
        for area in [
            Rect::new(0, 0, 0, 1),
            Rect::new(0, 0, 40, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(38, 0, 40, 1),
        ] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
            TabBar::new(&tabs, &theme).render(area, &mut buf);
        }
    }
}
