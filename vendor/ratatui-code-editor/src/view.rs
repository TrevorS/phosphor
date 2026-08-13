use crate::code::Code;
// PHOSPHOR PATCH 6 — the wrap engine itself lives under `phosphor/`.
use crate::phosphor::soft_wrap;
// PHOSPHOR PATCH 8 — the placement rule behind `VisualRow::Virtual`.
use crate::phosphor::virtual_text::{self, VirtualLine};
use crate::diff;
use crate::types::{RowSpan, VisualRow};

#[derive(Clone, Copy)]
pub(crate) enum FoldExpandDirection {
    Up,
    Down,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewMode {
    Plain,
    Diff,
    DiffFocus { context_lines: usize },
}

impl ViewMode {
    pub(crate) fn has_diff(self) -> bool {
        !matches!(self, ViewMode::Plain)
    }

    pub(crate) fn is_diff_focus(self) -> bool {
        matches!(self, ViewMode::DiffFocus { .. })
    }
}

#[derive(Default)]
pub(crate) struct View {
    rows: Vec<VisualRow>,
    expanded_hidden_ranges: Vec<(usize, usize)>,
    collapsed_code_folds: Vec<(usize, usize)>,
    /// PHOSPHOR PATCH 6 — text-column width to wrap at, in cells. `None` is
    /// upstream's behaviour: no wrapping, long lines scroll horizontally.
    /// Held here rather than passed to [`View::rebuild`] so every existing
    /// call site keeps its signature.
    soft_wrap: Option<usize>,
    /// PHOSPHOR PATCH 8 — the `┊` rows interleaved into the stream, in the
    /// order they were installed. Held here for the same reason `soft_wrap`
    /// is: every existing `rebuild` call site keeps its signature.
    virtual_lines: Vec<VirtualLine>,
    /// PHOSPHOR PATCH 8 — suppresses the rows without discarding them
    /// (phosphor's `set-virtual-text-visible`). Stored inverted so `Default`
    /// means shown, which is what upstream's derive gives us for free.
    virtual_hidden: bool,
}

impl View {
    pub(crate) fn new(code: &Code, mode: ViewMode) -> Self {
        let mut view = Self::default();
        view.rebuild(code, None, mode);
        view
    }

    pub(crate) fn fold_separator_text(hidden_lines: usize, amount: usize) -> String {
        format!(
            "[+{}up] [+{}down] [show unchanged {} lines]",
            amount, amount, hidden_lines
        )
    }

    pub(crate) fn rows(&self) -> &[VisualRow] {
        &self.rows
    }

    /// PHOSPHOR PATCH 6 — the wrap width, in cells of the text column.
    pub(crate) fn soft_wrap(&self) -> Option<usize> {
        self.soft_wrap
    }

    /// PHOSPHOR PATCH 6 — sets the wrap width. Returns `true` when it changed,
    /// which is the caller's cue to rebuild.
    pub(crate) fn set_soft_wrap(&mut self, width: Option<usize>) -> bool {
        let width = width.filter(|w| *w >= soft_wrap::MIN_WIDTH);
        let changed = width != self.soft_wrap;
        self.soft_wrap = width;
        changed
    }

    /// PHOSPHOR PATCH 8 — the `┊` rows currently installed.
    pub(crate) fn virtual_lines(&self) -> &[VirtualLine] {
        &self.virtual_lines
    }

    /// PHOSPHOR PATCH 8 — replaces the `┊` rows. Returns `true` when the list
    /// actually changed, which is the caller's cue to rebuild.
    pub(crate) fn set_virtual_lines(&mut self, lines: Vec<VirtualLine>) -> bool {
        let changed = lines != self.virtual_lines;
        self.virtual_lines = lines;
        changed
    }

    /// PHOSPHOR PATCH 8 — shows or hides the installed rows without losing
    /// them. Returns `true` when it changed.
    pub(crate) fn set_virtual_visible(&mut self, visible: bool) -> bool {
        let changed = self.virtual_hidden == visible;
        self.virtual_hidden = !visible;
        changed
    }

    /// PHOSPHOR PATCH 8 — whether the installed rows are drawn.
    pub(crate) fn virtual_visible(&self) -> bool {
        !self.virtual_hidden
    }

    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.expanded_hidden_ranges.clear();
        self.collapsed_code_folds.clear();
    }

    pub(crate) fn toggle_code_fold(
        &mut self,
        code: &Code,
        original: Option<&Code>,
        mode: ViewMode,
        line_idx: usize,
    ) -> bool {
        let Some(range) = code.fold_range_at_start(line_idx) else {
            return false;
        };
        let fold = (range.start_line, range.end_line);
        if let Some(index) = self
            .collapsed_code_folds
            .iter()
            .position(|item| *item == fold)
        {
            self.collapsed_code_folds.swap_remove(index);
        } else {
            self.collapsed_code_folds.push(fold);
        }
        self.rebuild(code, original, mode);
        true
    }

    pub(crate) fn clear_code_folds(&mut self) {
        self.collapsed_code_folds.clear();
    }

    /// PHOSPHOR PATCH 7 — how many lines a fold starting on `line_idx` is
    /// currently hiding, or `None` when there is no collapsed fold there.
    /// The count the `▸⋯ n lines` marker prints (mockup `8e`).
    pub(crate) fn code_fold_hidden_lines(&self, code: &Code, line_idx: usize) -> Option<usize> {
        let range = code.fold_range_at_start(line_idx)?;
        self.collapsed_code_folds
            .contains(&(range.start_line, range.end_line))
            .then(|| range.end_line.saturating_sub(range.start_line))
    }

    pub(crate) fn code_fold_indicator(&self, code: &Code, line_idx: usize) -> Option<bool> {
        code.fold_range_at_start(line_idx).map(|range| {
            self.collapsed_code_folds
                .contains(&(range.start_line, range.end_line))
        })
    }

    pub(crate) fn rebuild(&mut self, code: &Code, original: Option<&Code>, mode: ViewMode) {
        self.collapsed_code_folds.retain(|fold| {
            let (start_line, end_line) = *fold;
            code.has_fold_range(start_line, end_line)
        });

        if !mode.has_diff() {
            self.rows = (0..code.len_lines())
                .filter(|line_idx| {
                    !self
                        .collapsed_code_folds
                        .iter()
                        .any(|(start, end)| *line_idx > *start && *line_idx <= *end)
                })
                .map(|line_idx| VisualRow::Real {
                    line_idx,
                    is_added: false,
                    orig_line_idx: None,
                })
                .collect();
            // PHOSPHOR PATCH 6
            self.rows = soft_wrap::apply(std::mem::take(&mut self.rows), code, self.soft_wrap);
            // PHOSPHOR PATCH 8 — after the wrap, so a virtual row can pick the
            // segment it hangs under rather than the line's first row.
            self.interleave_virtual();
            return;
        }

        let Some(original) = original else {
            self.clear();
            return;
        };

        self.rows = match mode {
            ViewMode::Plain => Vec::new(),
            ViewMode::Diff => {
                self.expanded_hidden_ranges.clear();
                Self::apply_code_folds(
                    diff::compute_diff(code, original),
                    &self.collapsed_code_folds,
                )
            }
            ViewMode::DiffFocus { context_lines } => {
                let full_rows = diff::compute_diff(code, original);
                let rows = Self::focused_diff_rows(
                    &full_rows,
                    context_lines,
                    &self.expanded_hidden_ranges,
                );
                Self::apply_code_folds(rows, &self.collapsed_code_folds)
            }
        };
        // PHOSPHOR PATCH 6
        self.rows = soft_wrap::apply(std::mem::take(&mut self.rows), code, self.soft_wrap);
        // PHOSPHOR PATCH 8 — see the plain-mode branch above.
        self.interleave_virtual();
    }

    /// PHOSPHOR PATCH 8 — the one place `┊` rows enter the stream.
    fn interleave_virtual(&mut self) {
        if self.virtual_hidden || self.virtual_lines.is_empty() {
            return;
        }
        self.rows = virtual_text::apply(std::mem::take(&mut self.rows), &self.virtual_lines);
    }

    /// Filters out rows whose source lines fall inside any collapsed fold range.
    /// `GhostDeleted` rows use `anchor_line` which is `line_idx + 1`, hence the `+1` adjustment.
    fn apply_code_folds(rows: Vec<VisualRow>, folds: &[(usize, usize)]) -> Vec<VisualRow> {
        rows.into_iter()
            .filter(|row| match row {
                VisualRow::Real { line_idx, .. } => !folds
                    .iter()
                    .any(|(start, end)| *line_idx > *start && *line_idx <= *end),
                VisualRow::GhostDeleted { anchor_line, .. } => !folds
                    .iter()
                    .any(|(start, end)| *anchor_line > *start + 1 && *anchor_line <= *end + 1),
                VisualRow::FoldSeparator { .. } => true,
                // PHOSPHOR PATCH 6 — folds are applied before wrapping, so
                // this arm is unreachable today; it filters like `Real` so it
                // stays correct if the two ever swap order.
                VisualRow::Wrapped { line_idx, .. } => !folds
                    .iter()
                    .any(|(start, end)| *line_idx > *start && *line_idx <= *end),
                // PHOSPHOR PATCH 8 — virtual rows are interleaved after folds
                // are applied, so this arm is unreachable today. It keeps them
                // if the two ever swap order; a virtual line whose anchor is
                // inside a collapsed fold is dropped by `virtual_text::apply`,
                // which cannot find an anchor row for it.
                VisualRow::Virtual { .. } => true,
            })
            .collect()
    }

    pub(crate) fn expand_hidden_at_visual_row(
        &mut self,
        code: &Code,
        original: Option<&Code>,
        mode: ViewMode,
        visual_row: usize,
        clicked_col: usize,
        visible_width: usize,
        amount: usize,
    ) -> bool {
        if !matches!(mode, ViewMode::DiffFocus { .. }) {
            return false;
        }

        let row = match self.rows.get(visual_row) {
            Some(row) => row,
            None => return false,
        };

        let &VisualRow::FoldSeparator {
            hidden_lines,
            hidden_start,
            hidden_end,
        } = row
        else {
            return false;
        };

        if hidden_start > hidden_end || amount == 0 {
            return false;
        }

        let Some(direction) =
            Self::fold_expand_direction_for_click(hidden_lines, clicked_col, visible_width, amount)
        else {
            return false;
        };

        let (start, end) = match direction {
            FoldExpandDirection::Up => {
                let end = hidden_start + amount - 1;
                (hidden_start, end.min(hidden_end))
            }
            FoldExpandDirection::Down => {
                let start = hidden_end.saturating_sub(amount.saturating_sub(1));
                (hidden_start.max(start), hidden_end)
            }
            FoldExpandDirection::All => (hidden_start, hidden_end),
        };

        self.add_expanded_hidden_range(start, end);
        self.rebuild(code, original, mode);
        true
    }

    fn fold_expand_direction_for_click(
        hidden_lines: usize,
        clicked_col: usize,
        visible_width: usize,
        amount: usize,
    ) -> Option<FoldExpandDirection> {
        if clicked_col >= visible_width {
            return None;
        }

        let up_label = format!("[+{}up]", amount);
        let down_label = format!("[+{}down]", amount);
        let full_text = Self::fold_separator_text(hidden_lines, amount);

        let up_len = up_label.chars().count();
        let down_len = down_label.chars().count();
        let full_text_len = full_text.chars().count();
        let visible_text_len = visible_width.min(full_text_len);

        let up_end = up_len;
        let down_start = up_end + 1;
        let down_end = down_start + down_len;
        let show_start = down_end + 1;

        if clicked_col < up_end.min(visible_text_len) {
            return Some(FoldExpandDirection::Up);
        }
        if clicked_col >= down_start && clicked_col < down_end.min(visible_text_len) {
            return Some(FoldExpandDirection::Down);
        }
        if clicked_col >= show_start && clicked_col < visible_text_len {
            return Some(FoldExpandDirection::All);
        }

        None
    }

    pub(crate) fn visual_len_lines(&self, code: &Code, mode: ViewMode) -> usize {
        if mode.has_diff() || !self.rows.is_empty() {
            return self.rows.len().max(1);
        }
        code.len_lines().max(1)
    }

    pub(crate) fn line_for_visual_row(
        &self,
        code: &Code,
        mode: ViewMode,
        visual_row: usize,
    ) -> Option<usize> {
        let last = code.len_lines().saturating_sub(1);
        if !mode.has_diff() && self.rows.is_empty() {
            return Some(visual_row.min(last));
        }

        self.rows.get(visual_row).and_then(|row| match row {
            VisualRow::Real { line_idx, .. } => Some(*line_idx),
            VisualRow::GhostDeleted { anchor_line, .. } => {
                Some(anchor_line.saturating_sub(1).min(last))
            }
            VisualRow::FoldSeparator { .. } => None,
            // PHOSPHOR PATCH 6 — every segment maps back to its source line.
            VisualRow::Wrapped { line_idx, .. } => Some(*line_idx),
            // PHOSPHOR PATCH 8 — a virtual row is not a line. `None` is what
            // makes a click on one resolve to no cursor, and what keeps it out
            // of every line-wise motion.
            VisualRow::Virtual { .. } => None,
        })
    }

    /// PHOSPHOR PATCH 6 — the char span `[start_col, end_col)` a row draws,
    /// with the source line it belongs to and the cells its marker costs.
    ///
    /// `None` for rows that are not a slice of the current buffer — fold
    /// separators, and diff ghosts, whose text comes from the original.
    /// **This is the row-stream contract**: cursor placement, click targeting
    /// and (at `T032`) virtual-text placement all resolve a row through this
    /// one function, so they cannot disagree about what a row shows.
    pub(crate) fn row_span(
        &self,
        code: &Code,
        mode: ViewMode,
        visual_row: usize,
    ) -> Option<RowSpan> {
        if !mode.has_diff() && self.rows.is_empty() {
            let line_idx = visual_row.min(code.len_lines().saturating_sub(1));
            return Some(RowSpan {
                line_idx,
                segment: 0,
                start_col: 0,
                end_col: code.line_len(line_idx),
                prefix_cells: 0,
                wrapped: false,
                is_last_segment: true,
            });
        }

        match self.rows.get(visual_row)? {
            VisualRow::Real { line_idx, .. } => Some(RowSpan {
                line_idx: *line_idx,
                segment: 0,
                start_col: 0,
                end_col: code.line_len(*line_idx),
                prefix_cells: 0,
                wrapped: false,
                is_last_segment: true,
            }),
            VisualRow::Wrapped {
                line_idx,
                segment,
                start_col,
                end_col,
                ..
            } => Some(RowSpan {
                line_idx: *line_idx,
                segment: *segment,
                start_col: *start_col,
                end_col: *end_col,
                prefix_cells: if *segment == 0 {
                    0
                } else {
                    soft_wrap::CONTINUATION_PREFIX
                },
                wrapped: true,
                is_last_segment: *end_col >= code.line_len(*line_idx),
            }),
            // PHOSPHOR PATCH 8 — `Virtual` joins the two rows that draw no
            // slice of the buffer. It owns no char span, so no column resolves
            // to it and it is never a cursor target.
            VisualRow::FoldSeparator { .. }
            | VisualRow::GhostDeleted { .. }
            | VisualRow::Virtual { .. } => None,
        }
    }

    pub(crate) fn visual_row_for_line(&self, mode: ViewMode, line_idx: usize) -> Option<usize> {
        if !mode.has_diff() && self.rows.is_empty() {
            return Some(line_idx);
        }

        // PHOSPHOR PATCH 6 — a wrapped line's first segment is the row that
        // carries its number, so it is the row the line is "at".
        self.rows.iter().position(|row| match row {
            VisualRow::Real { line_idx: idx, .. } => *idx == line_idx,
            VisualRow::Wrapped {
                line_idx: idx,
                segment,
                ..
            } => *idx == line_idx && *segment == 0,
            _ => false,
        })
    }

    /// PHOSPHOR PATCH 6 — the row that shows `char_col` of `line_idx`.
    ///
    /// Differs from [`View::visual_row_for_line`] only for wrapped lines,
    /// where it picks the segment rather than the line's first row. A column
    /// sitting exactly on a segment boundary belongs to the *later* row, which
    /// is where a cursor typed up to the wrap point appears.
    pub(crate) fn visual_row_for_position(
        &self,
        mode: ViewMode,
        line_idx: usize,
        char_col: usize,
    ) -> Option<usize> {
        let first = self.visual_row_for_line(mode, line_idx)?;
        let mut last = first;
        for (offset, row) in self.rows.iter().enumerate().skip(first) {
            match row {
                VisualRow::Wrapped {
                    line_idx: idx,
                    end_col,
                    ..
                } if *idx == line_idx => {
                    last = offset;
                    if char_col < *end_col {
                        return Some(offset);
                    }
                }
                // PHOSPHOR PATCH 8 — a virtual row anchored to an early
                // segment sits *between* the segments of its own line. It is
                // not a row of this line and it does not end the run, so it is
                // skipped rather than treated as the end of the wrap. Without
                // this arm every column past the first virtual row resolves to
                // the wrong segment, which is the desync `T081` warned about.
                VisualRow::Virtual { .. } => {}
                _ if offset > first => break,
                _ => {}
            }
        }
        Some(last)
    }

    pub(crate) fn line_visible(&self, mode: ViewMode, line_idx: usize) -> bool {
        self.visual_row_for_line(mode, line_idx).is_some()
    }

    pub(crate) fn prev_line(&self, mode: ViewMode, line_idx: usize) -> Option<usize> {
        if !mode.has_diff() && self.rows.is_empty() {
            return line_idx.checked_sub(1);
        }

        self.rows.iter().rev().find_map(|row| {
            // PHOSPHOR PATCH 6 — `Wrapped` rows are real lines too; without
            // this arm, up/down motion would skip every wrapped line.
            let idx = match row {
                VisualRow::Real { line_idx: idx, .. }
                | VisualRow::Wrapped { line_idx: idx, .. } => *idx,
                _ => return None,
            };
            (idx < line_idx).then_some(idx)
        })
    }

    pub(crate) fn next_line(&self, code: &Code, mode: ViewMode, line_idx: usize) -> Option<usize> {
        if !mode.has_diff() && self.rows.is_empty() {
            let next = line_idx + 1;
            return (next < code.len_lines()).then_some(next);
        }

        self.rows.iter().find_map(|row| {
            // PHOSPHOR PATCH 6 — see `prev_line`.
            let idx = match row {
                VisualRow::Real { line_idx: idx, .. }
                | VisualRow::Wrapped { line_idx: idx, .. } => *idx,
                _ => return None,
            };
            (idx > line_idx).then_some(idx)
        })
    }

    pub(crate) fn nearest_line(
        &self,
        code: &Code,
        mode: ViewMode,
        line_idx: usize,
    ) -> Option<usize> {
        let prev = self.prev_line(mode, line_idx);
        let next = self.next_line(code, mode, line_idx);

        match (prev, next) {
            (Some(prev), Some(next)) => {
                if line_idx - prev <= next - line_idx {
                    Some(prev)
                } else {
                    Some(next)
                }
            }
            (Some(prev), None) => Some(prev),
            (None, Some(next)) => Some(next),
            (None, None) => None,
        }
    }

    fn focused_diff_rows(
        rows: &[VisualRow],
        context_lines: usize,
        expanded_hidden_ranges: &[(usize, usize)],
    ) -> Vec<VisualRow> {
        // Build a visibility mask: changed rows plus configured context around them.
        let mut include = vec![false; rows.len()];

        for (idx, row) in rows.iter().enumerate() {
            if row.is_changed() {
                let start = idx.saturating_sub(context_lines);
                let end = (idx + context_lines + 1).min(rows.len());
                for should_include in include.iter_mut().take(end).skip(start) {
                    *should_include = true;
                }
            }
        }

        // Keep user-expanded hidden regions visible even if they are outside default context.
        for &(start, end) in expanded_hidden_ranges {
            let start = start.min(rows.len().saturating_sub(1));
            let end = end.min(rows.len().saturating_sub(1));
            if start > end {
                continue;
            }
            for should_include in include.iter_mut().take(end + 1).skip(start) {
                *should_include = true;
            }
        }

        let mut result = Vec::new();
        let mut last_included_idx = None;

        for (idx, &is_included) in include.iter().enumerate() {
            if is_included {
                if let Some(last) = last_included_idx {
                    if idx > last + 1 {
                        result.push(VisualRow::FoldSeparator {
                            hidden_lines: idx - last - 1,
                            hidden_start: last + 1,
                            hidden_end: idx - 1,
                        });
                    }
                } else if idx > 0 {
                    result.push(VisualRow::FoldSeparator {
                        hidden_lines: idx,
                        hidden_start: 0,
                        hidden_end: idx - 1,
                    });
                }
                result.push(rows[idx].clone());
                last_included_idx = Some(idx);
            }
        }

        if let Some(last) = last_included_idx {
            if last + 1 < rows.len() {
                result.push(VisualRow::FoldSeparator {
                    hidden_lines: rows.len() - last - 1,
                    hidden_start: last + 1,
                    hidden_end: rows.len() - 1,
                });
            }
        } else {
            return rows.to_vec();
        }

        result
    }

    fn add_expanded_hidden_range(&mut self, start: usize, end: usize) {
        self.expanded_hidden_ranges
            .push((start.min(end), start.max(end)));
        self.expanded_hidden_ranges.sort_by_key(|(s, _)| *s);

        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in self.expanded_hidden_ranges.drain(..) {
            if let Some((_, last_end)) = merged.last_mut()
                && s <= *last_end + 1
            {
                *last_end = (*last_end).max(e);
            } else {
                merged.push((s, e));
            }
        }
        self.expanded_hidden_ranges = merged;
    }
}
