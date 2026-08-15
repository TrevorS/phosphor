//! PHOSPHOR PATCH 11 — a tab advances to the next tabstop.
//!
//! Upstream measures every grapheme with `unicode_width`, which answers `1`
//! for `\t` — `unicode-width` 0.2.2's `tables::width_in_str` gives `1` to every
//! `c <= '\u{A0}'` that is not `\n` or `\r` — and the renderer then draws
//! `g.to_string().replace('\t', " ")`. So a tab measured one cell and painted
//! one space, and a file indented with tabs rendered as if it had one space of
//! indent per level. `CP-4` reported it as *"tab only seems to go a space at a
//! time when indenting"*.
//!
//! **A tab's width is not a property of the tab.** It depends on the column it
//! starts at, which is why it cannot be folded into `grapheme_width` and why
//! every measuring walk in this crate has to pass its running column through
//! here. That column is in **display cells**, so a tab after a CJK character
//! advances from the column that character's *two* cells left the caret at —
//! the arithmetic this module exists to keep in one place.

use ropey::RopeSlice;

/// The tabstop a [`crate::code::Code`] uses until something sets one.
///
/// Four rather than eight. `utils::indent` already treats four spaces as the
/// house unit for the languages it names, and a standalone build of this crate
/// should render a tab the width its own indent function would have inserted.
pub const DEFAULT_TAB_WIDTH: usize = 4;

/// Whether `g` is a lone tab — the only grapheme whose width is a function of
/// where it starts.
///
/// A grapheme cluster and not a `char`: the walks that call this iterate
/// clusters, and a cluster beginning with `\t` and continuing into a combining
/// mark is not a tab stop, it is text.
#[must_use]
pub fn is_tab(g: RopeSlice<'_>) -> bool {
    let mut chars = g.chars();
    chars.next() == Some('\t') && chars.next().is_none()
}

/// Cells from `col` to the next tabstop — never zero, so a walk that adds this
/// always makes progress.
///
/// `tab_width` is clamped up to 1 rather than trusted: a stop of zero would
/// divide by zero here and make every measuring loop in the crate spin.
#[must_use]
pub fn stop(col: usize, tab_width: usize) -> usize {
    let width = tab_width.max(1);
    width - (col % width)
}

/// The cells `g` advances the caret by, starting from display column `col`.
///
/// `measured` is what `grapheme_width*` answered, and is returned untouched for
/// everything that is not a tab — so a call site reads as one line laid over
/// the width it already had, which is what keeps this patch's seam small.
#[must_use]
pub fn cells(g: RopeSlice<'_>, measured: usize, col: usize, tab_width: usize) -> usize {
    if is_tab(g) {
        stop(col, tab_width)
    } else {
        measured
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    fn slice(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn a_tab_at_column_zero_spends_the_whole_stop() {
        assert_eq!(stop(0, 4), 4);
    }

    #[test]
    fn a_tab_finishes_the_column_it_starts_in() {
        assert_eq!(stop(1, 4), 3);
        assert_eq!(stop(2, 4), 2);
        assert_eq!(stop(3, 4), 1);
        assert_eq!(stop(4, 4), 4);
    }

    #[test]
    fn a_zero_tab_width_still_advances() {
        assert_eq!(stop(0, 0), 1);
        assert_eq!(stop(7, 0), 1);
    }

    #[test]
    fn only_a_lone_tab_is_a_tab() {
        let rope = slice("\t");
        assert!(is_tab(rope.slice(..)));
        let rope = slice("\t\t");
        assert!(!is_tab(rope.slice(..)));
        let rope = slice(" ");
        assert!(!is_tab(rope.slice(..)));
    }

    /// The two halves of `cells`, at a column where they cannot be confused:
    /// a tab at column 1 spends 3 cells and the measurement it was handed says
    /// 1, so an implementation that ignored either argument fails here.
    #[test]
    fn a_tab_takes_the_stop_and_everything_else_takes_its_measurement() {
        let rope = slice("\t");
        assert_eq!(cells(rope.slice(..), 1, 1, 4), 3);
        let rope = slice("漢");
        assert_eq!(cells(rope.slice(..), 2, 1, 4), 2);
    }
}
