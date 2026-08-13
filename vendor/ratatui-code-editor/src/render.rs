use crate::code::{Code, RopeGraphemes, grapheme_width_and_bytes_len, grapheme_width_and_chars_len};
use crate::editor::Editor;
// PHOSPHOR PATCH 5 — see VENDOR.md.
use crate::phosphor::cell_style;
// PHOSPHOR PATCH 6 — the cell budget a `↪` continuation spends before its text.
use crate::phosphor::soft_wrap::CONTINUATION_PREFIX;
use crate::types::VisualRow;
use crate::view::View;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::widgets::Widget;

// PHOSPHOR PATCH 6 / 7 — the glyphs screen `8e` adds, all from Design Language
// §2's lexicon: `↪` soft-wrap continuation, `▸` fold closed, `⋯` elided, and
// the midline dot for whitespace. Single-cell and Nerd-Font-free, per §2.
const WRAP_INDICATOR: &str = "↪ ";
const FOLD_CLOSED: &str = "▸";
const FOLD_ELIDED: &str = "⋯";
const WHITESPACE_MARK: &str = "·";

// PHOSPHOR PATCH 8 — §2's `┊` virtual-margin rail, plus the space after it.
// Two cells, the same budget `↪ ` spends, so a virtual row and a continuation
// row start their text in the same column.
const VIRTUAL_RAIL: &str = "┊ ";

/// Draws the main editor view in the provided area using the ratatui rendering buffer.
///
/// Renders visible [`VisualRow`]s, including fold separators and deleted diff rows.
/// Added and deleted rows receive a diff background before syntax highlighting is
/// applied. Selections and user marks are then drawn over real editor rows.
///
/// # Arguments
///
/// * `self` - The `Editor` instance (as reference) to render.
/// * `area` - The rectangular area on the terminal to draw within.
/// * `buf` - The ratatui `Buffer` that represents the screen cells to draw to.
///
impl Widget for &Editor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let code = self.code_ref();
        // PHOSPHOR PATCH 4 — one source of truth for the digit count, so this
        // and `get_line_number_width()` cannot disagree about where text starts.
        let line_number_digits = self.line_number_digits();
        let line_number_width = self.get_line_number_width();
        let fold_gutter_width = self.fold_gutter_width();
        let total_visual_lines = self.visual_len_lines();
        let mut draw_y = area.top();

        // PHOSPHOR PATCH 4 — both were hardcoded. They now come from the theme
        // map like every other colour in this function, falling back to the
        // previous constants when the theme does not carry the key.
        let line_number_style = Style::default()
            .fg(self.theme_style("line_number").fg.unwrap_or(Color::DarkGray));
        let default_text_style = Style::default()
            .fg(self.theme_style("default_text").fg.unwrap_or(Color::White));

        let diff_added_bg = self.theme_style("diff_added").bg
            .or(self.theme_style("diff_added").fg)
            .unwrap_or(Color::Rgb(1, 125, 78));
        let diff_added_word_bg = self.theme_style("diff_added_word").bg
            .or(self.theme_style("diff_added_word").fg)
            .unwrap_or(Color::Rgb(19, 163, 111));
        let diff_deleted_bg = self.theme_style("diff_deleted").bg
            .or(self.theme_style("diff_deleted").fg)
            .unwrap_or(Color::Rgb(217, 75, 75));
        let diff_deleted_word_bg = self.theme_style("diff_deleted_word").bg
            .or(self.theme_style("diff_deleted_word").fg)
            .unwrap_or(Color::Rgb(248, 99, 99));

        let word_highlights = self.word_highlight_ranges();
        let word_highlight_bg = self.theme_style("word_highlight").bg
            .or(self.theme_style("word_highlight").fg)
            .unwrap_or(Color::Rgb(48, 54, 64));

        let fold_separator_style = Style::default().fg(Color::DarkGray);

        // PHOSPHOR PATCH 6 / 7 — the three glyph styles screen `8e` adds, read
        // from the theme map like everything else in this function and falling
        // back to the line-number colour, so a standalone build still renders.
        let wrap_indicator_style = Style::default().fg(self
            .theme_style("wrap_indicator")
            .fg
            .unwrap_or(Color::DarkGray));
        let fold_marker_style = Style::default().fg(self
            .theme_style("fold_marker")
            .fg
            .unwrap_or(Color::DarkGray));
        let trailing_whitespace_style = self.theme_style("trailing_whitespace");

        // PHOSPHOR PATCH 8 — the `┊` rail. Its runs carry their own styles;
        // only the rail glyph is the fork's to colour.
        let virtual_rail_style = Style::default().fg(self
            .theme_style("virtual_rail")
            .fg
            .unwrap_or(Color::DarkGray));

        // PHOSPHOR PATCH 5 — §3's undercurl, which marks cannot carry. Resolved
        // once per frame, not once per cell: the answer cannot change mid-frame.
        let styled_spans = self.styled_spans();
        let underline_capability = self.underline_capability();

        // draw lines, syntax highlighting, selection and marks in a single unified loop
        for visual_row_idx in self.offset_y..total_visual_lines {
            if draw_y >= area.bottom() {
                break;
            }

            let row = match self.visual_row(visual_row_idx) {
                Some(row) => row,
                None => break,
            };

            if let VisualRow::FoldSeparator { hidden_lines, .. } = &row {
                if self.show_line_numbers {
                    buf.set_string(
                        area.left(),
                        draw_y,
                        &format!("{:>width$}", "...", width = line_number_digits),
                        line_number_style,
                    );
                }
                let text_x = area.left() + line_number_width as u16;
                let text =
                    View::fold_separator_text(*hidden_lines, self.diff_options.expand_amount);
                let width = (area.width as usize).saturating_sub(line_number_width);
                let visible_text = text.chars().take(width).collect::<String>();
                if text_x < area.right() {
                    buf.set_string(text_x, draw_y, &visible_text, fold_separator_style);
                }
            } else if let VisualRow::Virtual { index, indent } = &row {
                // PHOSPHOR PATCH 8 — a `┊` row: the line-number column stays
                // blank because a virtual row is not a line, and the rail
                // starts at the anchor row's own text column (Design Language
                // §3, "virtual text indents to code column"), which on a `↪`
                // continuation is `indent` cells further in.
                if self.show_line_numbers {
                    buf.set_string(
                        area.left(),
                        draw_y,
                        &format!("{:>width$}", " ", width = line_number_digits),
                        line_number_style,
                    );
                }
                let mut x = area.left() + (line_number_width + indent) as u16;
                if x < area.right() {
                    let (next, _) = buf.set_stringn(
                        x,
                        draw_y,
                        VIRTUAL_RAIL,
                        (area.right() - x) as usize,
                        virtual_rail_style,
                    );
                    x = next.min(area.right());
                }
                if let Some(line) = self.virtual_lines().get(*index) {
                    for run in &line.runs {
                        if x >= area.right() {
                            break;
                        }
                        let (next, _) = buf.set_stringn(
                            x,
                            draw_y,
                            &run.text,
                            (area.right() - x) as usize,
                            run.style,
                        );
                        x = next.min(area.right());
                    }
                }
            } else {
                let (line_idx, is_added, is_ghost, partner_line_idx) = match &row {
                    VisualRow::Real { line_idx, is_added, orig_line_idx } => (*line_idx, *is_added, false, *orig_line_idx),
                    VisualRow::GhostDeleted {
                        original_line_idx, curr_line_idx, ..
                    } => (*original_line_idx, false, true, *curr_line_idx),
                    // PHOSPHOR PATCH 6 — a soft-wrap segment is a slice of a
                    // real line and draws like one; only its number column and
                    // its char span differ.
                    VisualRow::Wrapped { line_idx, is_added, orig_line_idx, .. } => (*line_idx, *is_added, false, *orig_line_idx),
                    _ => unreachable!(),
                };
                // PHOSPHOR PATCH 6 — (segment, start_col, end_col), or `None`
                // for a row that is not one segment of a wrapped line.
                let wrap = match &row {
                    VisualRow::Wrapped { segment, start_col, end_col, .. } => {
                        Some((*segment, *start_col, *end_col))
                    }
                    _ => None,
                };
                let source_code = if is_ghost {
                    self.original_code.as_ref().unwrap_or(code)
                } else {
                    code
                };

                // 1. Draw line numbers
                if self.show_line_numbers {
                    // PHOSPHOR PATCH 6 — `8e`: "carries no line number — the
                    // gutter stays honest". Only segment 0 is numbered.
                    let numbered = !is_ghost && wrap.is_none_or(|(segment, ..)| segment == 0);
                    let line_number = if numbered {
                        format!("{:>width$}", line_idx + 1, width = line_number_digits)
                    } else {
                        format!("{:>width$}", " ", width = line_number_digits)
                    };
                    buf.set_string(area.left(), draw_y, &line_number, line_number_style);
                }
                if !is_ghost {
                    if let Some(collapsed) = self.code_fold_indicator(line_idx) {
                        let indicator = if collapsed {
                            &self.code_folding_options.indicators.collapsed
                        } else {
                            &self.code_folding_options.indicators.expanded
                        };
                        buf.set_string(
                            area.left() + (line_number_width - fold_gutter_width) as u16,
                            draw_y,
                            indicator,
                            line_number_style,
                        );
                    }
                }

                let mut text_x = area.left() + line_number_width as u16;
                let mut width = (area.width as usize).saturating_sub(line_number_width);

                let line_len = source_code.line_len(line_idx);
                // PHOSPHOR PATCH 6 — a wrapped row draws the char span the row
                // stream gave it, not a window onto the whole line, and a
                // continuation row spends its first cells on `↪ `.
                let (start_col, end_col) = match wrap {
                    Some((segment, seg_start, seg_end)) => {
                        if segment > 0 {
                            if text_x < area.right() {
                                buf.set_string(text_x, draw_y, WRAP_INDICATOR, wrap_indicator_style);
                            }
                            text_x = text_x.saturating_add(CONTINUATION_PREFIX as u16);
                            width = width.saturating_sub(CONTINUATION_PREFIX);
                        }
                        (seg_start.min(line_len), seg_end.min(line_len))
                    }
                    None => {
                        let start = self.offset_x.min(line_len);
                        (start, (start + width).min(line_len))
                    }
                };

                // PHOSPHOR PATCH 7 — first column of the line's trailing
                // whitespace run; `usize::MAX` when the row has none to mark.
                let trailing_start = if self.show_trailing_whitespace() && !is_ghost {
                    trailing_whitespace_start(source_code, line_idx, line_len)
                } else {
                    usize::MAX
                };

                let line_start_char = source_code.line_to_char(line_idx);
                let char_slice_start = line_start_char + start_col;
                let char_slice_end = line_start_char + end_col;
                let visible_chars = source_code.char_slice(char_slice_start, char_slice_end);

                let start_byte = source_code.char_to_byte(char_slice_start);
                let end_byte = source_code.char_to_byte(char_slice_end);

                let line_end_char = line_start_char + line_len;
                let line_word_highlights: Vec<(usize, usize)> = if is_ghost {
                    Vec::new()
                } else {
                    word_highlights
                        .iter()
                        .filter(|&&(start, end)| start < line_end_char && end > line_start_char)
                        .cloned()
                        .collect()
                };

                // Fetch highlights
                let highlights = if code.is_highlight() {
                    if is_ghost {
                        self.highlight_interval_original(start_byte, end_byte, &self.theme)
                    } else {
                        self.highlight_interval(start_byte, end_byte, &self.theme)
                    }
                } else {
                    Vec::new()
                };

                // Fetch intra-line diff highlights on the fly from cache
                let intra_highlights = partner_line_idx.map(|partner_idx| {
                    if is_ghost {
                        self.get_line_diff(line_idx, partner_idx, true)
                    } else {
                        self.get_line_diff(partner_idx, line_idx, false)
                    }
                });

                // Base style background color
                let base_bg = match is_ghost {
                    true => Some(diff_deleted_bg),
                    false if is_added => Some(diff_added_bg),
                    false => None,
                };

                let mut x = 0;
                let mut byte_idx_in_rope = start_byte;
                let mut char_col = start_col;

                // 3. Single loop over the graphemes of the line
                for g in RopeGraphemes::new(&visible_chars) {
                    let (g_width, g_bytes) = grapheme_width_and_bytes_len(g);
                    let (_, g_chars) = grapheme_width_and_chars_len(g);

                    if x >= width {
                        break;
                    }

                    let start_x = text_x + x as u16;

                    // Check if current character falls within an intra-line highlight range
                    let is_word_highlight = intra_highlights.as_ref().map_or(false, |ranges| {
                        ranges.iter().any(|&(start, end)| char_col >= start && char_col < end)
                    });

                    let active_bg = if is_word_highlight {
                        if is_ghost { Some(diff_deleted_word_bg) } else { Some(diff_added_word_bg) }
                    } else {
                        base_bg
                    };

                    // Compose style
                    let mut style = if let Some(bg) = active_bg {
                        Style::default().bg(bg)
                    } else {
                        default_text_style
                    };

                    // Layer A: Syntax highlights
                    for &(start, end, s) in &highlights {
                        if start <= byte_idx_in_rope && byte_idx_in_rope < end {
                            style = style.patch(s);
                            if let Some(bg) = active_bg {
                                style = style.bg(bg); // Keep active diff background
                            }
                            break;
                        }
                    }

                    let global_char_idx = line_start_char + char_col;

                    if !is_ghost {
                        // Layer D: Word Highlight
                        let is_in_word_highlight = line_word_highlights.iter().any(|&(start, end)| {
                            global_char_idx >= start && global_char_idx < end
                        });
                        if is_in_word_highlight {
                            style = style.bg(word_highlight_bg);
                        }

                        // Layer B: Selection
                        if let Some(selection) = self.selection
                            && !selection.is_empty()
                        {
                            let start = selection.start.min(selection.end);
                            let end = selection.start.max(selection.end);
                            if global_char_idx >= start && global_char_idx < end {
                                style = style.bg(Color::DarkGray);
                            }
                        }

                        // Layer C: Marks
                        if let Some(ref marks) = self.marks {
                            for &(m_start, m_end, m_color) in marks {
                                if global_char_idx >= m_start && global_char_idx < m_end {
                                    style = style.bg(m_color);
                                }
                            }
                        }
                    }

                    // PHOSPHOR PATCH 5 — Layer E: styled spans. Ghost rows are
                    // the original buffer, whose char offsets are not the ones
                    // a span was built against, so they take none.
                    let cell_style = if is_ghost {
                        None
                    } else {
                        cell_style::span_at(styled_spans, global_char_idx)
                    };
                    if let Some(cell_style) = cell_style {
                        style = cell_style::patch_style(style, cell_style);
                    }

                    // PHOSPHOR PATCH 7 — `8e`: trailing whitespace shows as a
                    // midline dot on the failure tint, in INSERT only. Patched
                    // over everything else so it survives a diff background.
                    let is_trailing_ws = char_col >= trailing_start;
                    if is_trailing_ws {
                        style = style.patch(trailing_whitespace_style);
                    }

                    // Draw character
                    let display_g = if is_trailing_ws {
                        WHITESPACE_MARK.to_string()
                    } else {
                        g.to_string().replace('\t', " ")
                    };
                    if start_x < area.right() {
                        buf.set_string(start_x, draw_y, &display_g, style);
                        // PHOSPHOR PATCH 5 — undercurl has no ratatui
                        // `Modifier`, so where the terminal has it the SGR pair
                        // rides in the cell's symbol. A no-op everywhere else,
                        // which is the whole degradation path.
                        if let Some(cell_style) = cell_style {
                            cell_style::decorate_cell(
                                buf,
                                start_x,
                                draw_y,
                                cell_style,
                                underline_capability,
                            );
                        }
                    }

                    x = x.saturating_add(g_width);
                    byte_idx_in_rope += g_bytes;
                    char_col += g_chars;
                }

                // PHOSPHOR PATCH 7 — `8e`: a collapsed fold marks its header
                // line inline, after the code — `▸⋯ 13 lines` in meta-gray —
                // rather than in a gutter column no mockup draws. The fold
                // itself is upstream's: the hidden lines are already absent
                // from the row stream.
                if !is_ghost
                    && wrap.is_none_or(|(_, _, seg_end)| seg_end >= line_len)
                    && x < width
                    && let Some(hidden) = self.fold_hidden_lines(line_idx)
                {
                    let marker = format!(
                        " {}{} {} line{}",
                        FOLD_CLOSED,
                        FOLD_ELIDED,
                        hidden,
                        if hidden == 1 { "" } else { "s" }
                    );
                    let marker_x = text_x + x as u16;
                    if marker_x < area.right() {
                        let room = (width - x).min((area.right() - marker_x) as usize);
                        let visible = marker.chars().take(room).collect::<String>();
                        buf.set_string(marker_x, draw_y, &visible, fold_marker_style);
                    }
                }

                // 4. Fill remaining width with background if needed
                if let Some(bg) = base_bg
                    && x < width
                    && text_x + (x as u16) < area.right()
                {
                    let fill_x = text_x + (x as u16);
                    let fill_width = width - x;
                    buf.set_string(
                        fill_x,
                        draw_y,
                        &" ".repeat(fill_width),
                        Style::default().bg(bg),
                    );
                }
            }
            draw_y += 1;
        }
    }
}

/// PHOSPHOR PATCH 7 — first char column of `line_idx`'s trailing whitespace
/// run, or `usize::MAX` when the line does not end in any. A line that is
/// nothing but whitespace is trailing whitespace from column 0, which is what
/// vim's `trail` listchar does and what `8e` asks for.
fn trailing_whitespace_start(code: &Code, line_idx: usize, line_len: usize) -> usize {
    if line_len == 0 {
        return usize::MAX;
    }
    let line_start = code.line_to_char(line_idx);
    let slice = code.char_slice(line_start, line_start + line_len);
    let mut col = 0usize;
    let mut trailing_start = 0usize;
    for c in slice.chars() {
        col += 1;
        if c != ' ' && c != '\t' {
            trailing_start = col;
        }
    }
    if trailing_start == line_len {
        usize::MAX
    } else {
        trailing_start
    }
}
