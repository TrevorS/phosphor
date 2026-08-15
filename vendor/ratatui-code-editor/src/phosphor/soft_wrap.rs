//! PHOSPHOR PATCH 6 — soft wrap, as a variant of the row stream.
//!
//! Upstream has no wrapping of any kind: a long line scrolls sideways and
//! `VisualRow` has one row per source line. Phosphor's design language wants
//! `↪` continuations that carry no line number (mockup `8e`), and the whole of
//! `T081` is the instruction that this must be **inside** the row stream
//! rather than a layer above it — `View`'s row list is what row↔line mapping,
//! cursor placement, click targeting and (from `T032`) virtual-text placement
//! all read, and a wrap that lives outside it desynchronises all four.
//!
//! So this module owns exactly one thing: turning a `VisualRow::Real` into the
//! run of `VisualRow::Wrapped` segments that replaces it. Everything that
//! *consumes* those rows — `View::row_span`, `View::visual_row_for_position`,
//! the renderer, `Editor::cursor_from_mouse`, `Editor::get_visible_cursor` —
//! reads them through the same helpers it already read `Real` rows through.

use crate::code::{Code, RopeGraphemes, grapheme_width_and_chars_len};
use crate::types::VisualRow;

/// Cells a `↪` continuation row spends on its marker before the text resumes:
/// the glyph plus one space (mockup `8e`).
pub(crate) const CONTINUATION_PREFIX: usize = 2;

/// Narrower than this and a continuation row would have no room for text, so
/// wrapping is refused rather than looping.
pub(crate) const MIN_WIDTH: usize = CONTINUATION_PREFIX + 2;

/// Splits one source line into `[start_col, end_col)` char spans, each of
/// which fits `width`.
///
/// Breaks at the last space that fits when there is one and hard-breaks
/// mid-word when there is not. The break space stays on the row before it, so
/// the spans **partition** the line: `end_col` of one segment is `start_col`
/// of the next, the first `start_col` is 0, and the last `end_col` is the line
/// length. Continuation rows measure against a width two cells narrower,
/// because that is what the marker costs them.
///
/// A line that fits comes back as a single segment, which is how the caller
/// knows to leave it a `Real` row.
pub(crate) fn segments(code: &Code, line_idx: usize, width: usize) -> Vec<(usize, usize)> {
    let line_len = code.line_len(line_idx);
    if code.char_col_to_visual(line_idx, line_len) <= width {
        return vec![(0, line_len)];
    }

    let line_start = code.line_to_char(line_idx);
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut seg_start = 0usize;
    // PHOSPHOR PATCH 11 — cells this segment starts at, in the *line*.
    // `used` is the segment's own budget and a tabstop is absolute, so a tab on
    // a continuation row has to be measured from where the line has reached
    // rather than from where the row has.
    //
    // Carried across the loop rather than recomputed from the line start,
    // because `char_col_to_visual` is a grapheme walk of the prefix and one per
    // segment makes the rebuild quadratic in the line's own length — the exact
    // shape `phosphor-ui/benches/soft_wrap.rs`'s second table asserts against,
    // which failed at 1454x on one 400000-character line the first time this
    // was written. The segments partition the line (see the doc above), so the
    // next segment starts exactly `spent` cells further along.
    let mut base_col = 0usize;

    while seg_start < line_len {
        let avail = if out.is_empty() {
            width
        } else {
            width.saturating_sub(CONTINUATION_PREFIX)
        }
        .max(1);

        let slice = code.char_slice(line_start + seg_start, line_start + line_len);
        let mut used = 0usize;
        let mut col = seg_start;
        // Both halves of a break point: where it is, and what the row spent to
        // reach it. Breaking at a space discards the graphemes walked past it,
        // so the cells they spent may not be carried.
        let mut last_break: Option<(usize, usize)> = None;
        // Cells the grapheme that did *not* fit would have spent. Only read on
        // the hard-break-at-one-char path below, where it is the whole advance.
        let mut overflowed = 0usize;

        for g in RopeGraphemes::new(&slice) {
            let (g_width, g_chars) = grapheme_width_and_chars_len(g);
            // PHOSPHOR PATCH 11 — see VENDOR.md.
            let g_width = crate::phosphor::tabs::cells(
                g,
                g_width,
                base_col + used,
                code.tab_width(),
            );
            if used + g_width > avail {
                overflowed = g_width;
                break;
            }
            used += g_width;
            col += g_chars;
            if g.chars().next().is_some_and(|c| c == ' ' || c == '\t') {
                last_break = Some((col, used));
            }
        }

        if col >= line_len {
            out.push((seg_start, line_len));
            break;
        }

        let (end, spent) = match last_break {
            Some((brk, cells)) if brk > seg_start && brk < col => (brk, cells),
            // No space to break at, or the only one is where we already are:
            // hard-break, and never zero-width, or this loop would not end.
            _ if col > seg_start => (col, used),
            // Not even one grapheme fits — a tab wider than a continuation
            // row's budget is the reachable case. The forced char takes the
            // cells it overflowed by.
            _ => (seg_start + 1, overflowed),
        };
        out.push((seg_start, end));
        seg_start = end;
        base_col += spent;
    }

    if out.is_empty() {
        out.push((0, line_len));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::Code;

    fn code(text: &str) -> Code {
        Code::new(text, "text", None).unwrap()
    }

    /// **The carry and the walk must agree.** `segments` used to ask
    /// `char_col_to_visual` for each segment's starting column, which is a
    /// grapheme walk of everything before it and made a rebuild quadratic in a
    /// single line's length (`phosphor-ui/benches/soft_wrap.rs`, table 2, which
    /// failed at 1454x). It carries the column instead now, and the carry is
    /// only sound if it lands where the walk would have.
    ///
    /// The line is built so a wrong carry shows up as a different **break**,
    /// not just a different number. At width 8 with a 4-cell stop:
    ///
    /// * row 0 fills to `abcde fg` and breaks at the space, so it ends at
    ///   char 6 having *walked* 8 cells — the two cells past the break are the
    ///   whole point, because they are the gap between "what the row spent" and
    ///   "where the line has reached".
    /// * row 1 starts at cell 6. `fghi` spends 4, putting the tab at cell 10,
    ///   where it spends 2 and exactly fills the continuation row's budget of
    ///   6. So the tab is *in*, and the row ends at char 11.
    ///
    /// Carry the walked 8 instead of the spent 6 and the tab is measured from
    /// cell 12, where it spends 4 and no longer fits: row 1 ends at char 10,
    /// two cells short, with a tab pushed onto the next row that had room.
    #[test]
    fn each_row_measures_its_tabs_from_where_the_line_has_reached() {
        let code = code("abcde fghi\tXY");
        assert_eq!(code.tab_width(), 4);
        assert_eq!(segments(&code, 0, 8), vec![(0, 6), (6, 11), (11, 13)]);
        // The oracle for the carry, stated the slow way: the second row starts
        // 6 cells into the line and the third 12, tab included.
        assert_eq!(code.char_col_to_visual(0, 6), 6);
        assert_eq!(code.char_col_to_visual(0, 11), 12);
    }

    /// A line with no tab cannot tell a right carry from a wrong one — every
    /// grapheme costs what it costs wherever it sits — so this pins the shape
    /// the test above varies from, and would have caught a carry that broke
    /// plain text.
    #[test]
    fn a_line_of_plain_text_breaks_at_its_spaces() {
        let code = code("abcde fghij klmno");
        assert_eq!(segments(&code, 0, 8), vec![(0, 6), (6, 12), (12, 17)]);
    }
}

/// Expands the `Real` rows of a freshly built stream into `Wrapped` runs.
///
/// Fold separators and diff ghosts pass through untouched: a ghost row's text
/// comes from the *original* buffer, and diff is `T063`'s surface rather than
/// this one. A line that fits stays `Real`, so with wrapping off — or on, over
/// a buffer with no long lines — the stream is what upstream builds.
pub(crate) fn apply(rows: Vec<VisualRow>, code: &Code, width: Option<usize>) -> Vec<VisualRow> {
    let Some(width) = width else {
        return rows;
    };

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let VisualRow::Real {
            line_idx,
            is_added,
            orig_line_idx,
        } = row
        else {
            out.push(row);
            continue;
        };

        let spans = segments(code, line_idx, width);
        if spans.len() < 2 {
            out.push(row);
            continue;
        }
        for (segment, (start_col, end_col)) in spans.into_iter().enumerate() {
            out.push(VisualRow::Wrapped {
                line_idx,
                segment,
                start_col,
                end_col,
                is_added,
                orig_line_idx,
            });
        }
    }
    out
}
