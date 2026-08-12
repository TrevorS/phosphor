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
        let mut last_break: Option<usize> = None;

        for g in RopeGraphemes::new(&slice) {
            let (g_width, g_chars) = grapheme_width_and_chars_len(g);
            if used + g_width > avail {
                break;
            }
            used += g_width;
            col += g_chars;
            if g.chars().next().is_some_and(|c| c == ' ' || c == '\t') {
                last_break = Some(col);
            }
        }

        if col >= line_len {
            out.push((seg_start, line_len));
            break;
        }

        let end = match last_break {
            Some(brk) if brk > seg_start && brk < col => brk,
            // No space to break at, or the only one is where we already are:
            // hard-break, and never zero-width, or this loop would not end.
            _ => col.max(seg_start + 1),
        };
        out.push((seg_start, end));
        seg_start = end;
    }

    if out.is_empty() {
        out.push((0, line_len));
    }
    out
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
