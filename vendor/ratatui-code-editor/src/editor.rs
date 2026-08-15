use crate::actions::*;
use crate::click::{ClickKind, ClickTracker};
use crate::code::Code;
use crate::code::{EditBatch, Operation};
use crate::code::{RopeGraphemes, grapheme_width_and_chars_len};
use crate::selection::{Selection, SelectionSnap};
use crate::types::{CodeFoldingOptions, DiffOptions, HightlightCache, Theme, VisualRow, LineDiffCache};
// PHOSPHOR PATCH 6 — the row-stream contract, and PATCH 7's marker glyphs.
use crate::types::RowSpan;
// PHOSPHOR PATCH 8 — the `┊` rows interleaved into that stream.
// PHOSPHOR PATCH 11 — a tab's width is a function of the column it starts at.
use crate::phosphor::tabs;
use crate::phosphor::virtual_text::VirtualLine;
use crate::utils;
use crate::view::{View, ViewMode};
use anyhow::{Result, anyhow};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// Represents the text editor, which holds the code buffer, cursor, selection,
/// theme, scroll offsets, highlight cache, clipboard, and user mark intervals.
pub struct Editor {
    /// Code buffer and editing/highlighting logic for the current language
    pub(crate) code: Code,

    /// Current cursor position as a character index in the document
    pub(crate) cursor: usize,

    /// Vertical scroll offset: index of the first visible line
    pub(crate) offset_y: usize,

    /// Horizontal scroll offset in characters (visual columns)
    pub(crate) offset_x: usize,

    /// Syntax theme: mapping of token name to ratatui Style
    pub(crate) theme: Theme,

    /// Current text selection, if any
    pub(crate) selection: Option<Selection>,

    /// Click tracker to detect single/double/triple clicks
    pub(crate) clicks: ClickTracker,

    /// Selection snapping mode (to word, to line, or none)
    pub(crate) selection_snap: SelectionSnap,

    /// Fallback clipboard storage when the system clipboard is unavailable
    pub(crate) clipboard: Option<String>,

    /// User marks for intervals: (start, end, color)
    pub(crate) marks: Option<Vec<(usize, usize, Color)>>,

    /// PHOSPHOR PATCH 5 — cell styles the marks above cannot carry: §3's
    /// undercurl, degrading to a straight underline. Separate from `marks`
    /// rather than an extra tuple field, because the two are replaced by
    /// different owners at different times (region tints `T087`, diagnostics
    /// `T040`) and `set_marks` replaces wholesale.
    pub(crate) styled_spans: Vec<crate::phosphor::cell_style::StyledSpan>,

    /// PHOSPHOR PATCH 5 — `None` asks the environment (the default);
    /// `Some` is an app layer that has negotiated with the terminal itself.
    pub(crate) underline_capability: Option<crate::phosphor::cell_style::UnderlineCapability>,

    /// Syntax highlight cache by intervals to speed up rendering
    pub(crate) highlights_cache: RefCell<HightlightCache>,

    /// Cache for line diff highlights to speed up rendering
    pub(crate) line_diff_cache: RefCell<LineDiffCache>,

    /// Controls whether to highlight occurrences of the word under the cursor
    pub(crate) word_highlight_enabled: bool,

    /// Cache for word highlight ranges: (cursor, list of ranges)
    pub(crate) word_highlight_cache: RefCell<Option<(usize, Vec<(usize, usize)>)>>,

    /// Controls when to show the line numbers
    pub(crate) show_line_numbers: bool,

    /// Controls whether the code-fold gutter is shown and interactive.
    pub(crate) code_folding_options: CodeFoldingOptions,

    /// Controls the left padding before writing the code
    pub(crate) left_code_padding: usize,

    /// PHOSPHOR PATCH 4 — minimum width of the line-number column, in cells.
    /// Defaults to 5, which is the constant this replaces.
    pub(crate) line_number_min_digits: usize,

    /// PHOSPHOR PATCH 7 — whether the fold gutter column is drawn and clickable.
    /// Upstream conflates this with `code_folding_options.enabled`; phosphor
    /// needs folding *without* the column, because `8e` puts the fold marker
    /// inline after the code. `true` is upstream's behaviour.
    pub(crate) fold_gutter_visible: bool,

    /// PHOSPHOR PATCH 7 — whether trailing whitespace is marked with `·`.
    /// Insert-only (`8e`), so the caller turns it on with the mode.
    pub(crate) show_trailing_whitespace: bool,

    /// Current document-to-screen view mode.
    pub(crate) view_mode: ViewMode,

    /// Original code snapshot used for diff and ghost-line highlighting.
    pub(crate) original_code: Option<Code>,

    /// Options for diff and diff-focus modes.
    pub(crate) diff_options: DiffOptions,

    /// Derived view rows and line mappings used for scrolling, rendering, and navigation.
    pub(crate) view: View,
}

impl Editor {
    pub fn new(lang: &str, text: &str, theme: Vec<(&str, &str)>) -> Result<Self> {
        Self::new_with_highlights(lang, text, theme, None)
    }

    pub fn new_with_highlights(
        lang: &str,
        text: &str,
        theme: Vec<(&str, &str)>,
        custom_highlights: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        let code = Code::new(text, lang, custom_highlights.clone())
            .or_else(|_| Code::new(text, "text", custom_highlights))?;

        let theme = Self::build_theme(&theme);
        let highlights_cache = RefCell::new(HashMap::new());
        let line_diff_cache = RefCell::new(HashMap::new());
        let view = View::new(&code, ViewMode::Plain);

        Ok(Self {
            code,
            cursor: 0,
            offset_y: 0,
            offset_x: 0,
            theme,
            selection: None,
            clicks: ClickTracker::new(Duration::from_millis(700)),
            selection_snap: SelectionSnap::None,
            clipboard: None,
            marks: None,
            // PHOSPHOR PATCH 5 — no spans, and the capability unasked until a
            // span exists to draw.
            styled_spans: Vec::new(),
            underline_capability: None,
            highlights_cache,
            line_diff_cache,
            word_highlight_enabled: true,
            word_highlight_cache: RefCell::new(None),
            show_line_numbers: true,
            code_folding_options: CodeFoldingOptions::default(),
            left_code_padding: 2,
            line_number_min_digits: 5,
            // PHOSPHOR PATCH 7 — both default to upstream's behaviour.
            fold_gutter_visible: true,
            show_trailing_whitespace: false,
            view_mode: ViewMode::Plain,
            original_code: None,
            diff_options: DiffOptions::default(),
            view,
        })
    }

    /// PHOSPHOR PATCH 4 — the width of the line-number column, in cells.
    /// Extracted so `render.rs` and [`Editor::get_line_number_width`] cannot
    /// disagree, and so the minimum is configurable rather than a literal 5.
    pub fn line_number_digits(&self) -> usize {
        let max_line_number = self.code.len_lines().max(1);
        max_line_number
            .to_string()
            .len()
            .max(self.line_number_min_digits)
    }

    // PHOSPHOR PATCH 4 — `pub`, was `pub(crate)`: a consumer composing its own
    // gutter around this widget has to know where the text column starts.
    pub fn get_line_number_width(&self) -> usize {
        let fold_gutter_width = self.fold_gutter_width();
        if self.show_line_numbers {
            self.line_number_digits() + self.left_code_padding + fold_gutter_width
        } else {
            self.left_code_padding + fold_gutter_width
        }
    }

    pub fn focus(&mut self, area: &Rect) {
        self.fit_cursor();
        if self.is_diff_focus_active() {
            self.clamp_cursor_to_focus_rows();
        }
        self.clamp_offset_y();

        let width = area.width as usize;
        let height = area.height as usize;
        let line_number_width = self.get_line_number_width();

        let line = self.code.char_to_line(self.cursor);
        let col = self.cursor - self.code.line_to_char(line);

        let visible_width = width.saturating_sub(line_number_width);
        let visible_height = height;

        // PHOSPHOR PATCH 6 — wrapped text has nowhere to scroll sideways to,
        // so the horizontal offset is pinned at 0 rather than chased.
        let step_size = 10;
        if self.soft_wrap_width().is_some() {
            self.offset_x = 0;
        } else if col < self.offset_x {
            self.offset_x = col.saturating_sub(step_size);
        } else if col >= self.offset_x + visible_width {
            self.offset_x = col.saturating_sub(visible_width.saturating_sub(step_size));
        }

        // PHOSPHOR PATCH 6 — the cursor's *segment*, not the row its line
        // starts on: revealing a wrapped line's head does not reveal a cursor
        // forty cells into it.
        let visual_line = self
            .visual_row_for_position(line, col)
            .unwrap_or(usize::MAX);
        if visual_line == usize::MAX {
            return;
        }

        if visual_line < self.offset_y {
            self.offset_y = visual_line;
        } else if visual_line >= self.offset_y + visible_height {
            self.offset_y = visual_line.saturating_sub(visible_height.saturating_sub(1));
        }
    }

    /// Handles a mouse button press at the given cursor position, updating selection and click state.
    pub fn handle_mouse_down(&mut self, cursor: usize) {
        let kind = self.clicks.register(cursor);
        let (start, end, snap) = match kind {
            ClickKind::Triple => {
                let (line_start, line_end) = self.code.line_boundaries(cursor);
                (line_start, line_end, SelectionSnap::Line { anchor: cursor })
            }
            ClickKind::Double => {
                let (word_start, word_end) = self.code.word_boundaries(cursor);
                (word_start, word_end, SelectionSnap::Word { anchor: cursor })
            }
            ClickKind::Single => (cursor, cursor, SelectionSnap::None),
        };

        self.selection = Some(Selection::from_anchor_and_cursor(start, end));
        self.cursor = end;
        self.selection_snap = snap;
    }

    /// Handles a mouse drag event at the given cursor position, extending the selection.
    pub fn handle_mouse_drag(&mut self, cursor: usize) {
        match self.selection_snap {
            SelectionSnap::Line { anchor } => {
                let (anchor_start, anchor_end) = self.code.line_boundaries(anchor);
                let (cur_start, cur_end) = self.code.line_boundaries(cursor);

                let (sel_start, sel_end, new_cursor) = match cursor.cmp(&anchor) {
                    Ordering::Greater => (anchor_start, cur_end, cur_end), // forward
                    Ordering::Less => (cur_start, anchor_end, cur_start),  // backward
                    Ordering::Equal => (anchor_start, anchor_end, anchor_end),
                };

                self.selection = Some(Selection::from_anchor_and_cursor(sel_start, sel_end));
                self.cursor = new_cursor;
            }
            SelectionSnap::Word { anchor } => {
                let (anchor_start, anchor_end) = self.code.word_boundaries(anchor);
                let (cur_start, cur_end) = self.code.word_boundaries(cursor);

                let (sel_start, sel_end, new_cursor) = match cursor.cmp(&anchor) {
                    Ordering::Greater => (anchor_start, cur_end, cur_end), // forward
                    Ordering::Less => (cur_start, anchor_end, cur_start),  // backward
                    Ordering::Equal => (anchor_start, anchor_end, anchor_end),
                };

                self.selection = Some(Selection::from_anchor_and_cursor(sel_start, sel_end));
                self.cursor = new_cursor;
            }
            SelectionSnap::None => {
                let anchor = self.selection_anchor();
                self.selection = Some(Selection::from_anchor_and_cursor(anchor, cursor));
                self.cursor = cursor;
            }
        }
    }

    /// Converts mouse coordinates to a cursor position within the editor area, returning `None` if outside.
    pub fn cursor_from_mouse(&self, mouse_x: u16, mouse_y: u16, area: &Rect) -> Option<usize> {
        let line_number_width = self.get_line_number_width() as u16;

        if mouse_y < area.top()
            || mouse_y >= area.bottom()
            || mouse_x < area.left() + line_number_width
        {
            return None;
        }

        let clicked_visual_row = (mouse_y - area.top()) as usize + self.offset_y;
        let clicked_row = self.line_for_visual_row(clicked_visual_row)?;
        if clicked_row >= self.code.len_lines() {
            return None;
        }

        let clicked_col = (mouse_x - area.left() - line_number_width) as usize;

        let line_start_char = self.code.line_to_char(clicked_row);
        let line_len = self.code.line_len(clicked_row);

        // PHOSPHOR PATCH 6 — on a wrapped row the click is measured inside the
        // row's own span: past the `↪ ` marker, and clamped to the segment
        // rather than to the whole line. Rows the row stream does not own a
        // span for (diff ghosts) keep upstream's offset_x-based mapping.
        let span = self
            .row_span(clicked_visual_row)
            .filter(|span| span.wrapped);
        let clicked_col = match span {
            Some(span) => clicked_col.saturating_sub(span.prefix_cells),
            None => clicked_col,
        };
        let (start_col, end_col) = match span {
            Some(span) => (span.start_col.min(line_len), span.end_col.min(line_len)),
            None => (self.offset_x.min(line_len), line_len),
        };

        let char_start = line_start_char + start_col;
        let char_end = line_start_char + end_col;

        let mut current_col = 0;
        let mut char_idx = start_col;
        let visible_chars = self.code.char_slice(char_start, char_end);
        // PHOSPHOR PATCH 11 — the row's first grapheme sits this far into the
        // line, in cells. `current_col` is relative to the row (it is compared
        // against `clicked_col`, which is too) and a tabstop is absolute, so
        // the walk needs both.
        let base_col = self.code.char_col_to_visual(clicked_row, start_col);
        for g in RopeGraphemes::new(&visible_chars) {
            let (g_width, g_chars) = grapheme_width_and_chars_len(g);
            // PHOSPHOR PATCH 11 — see VENDOR.md.
            let g_width = tabs::cells(g, g_width, base_col + current_col, self.code.tab_width());
            if current_col + g_width > clicked_col {
                break;
            }
            current_col += g_width;
            char_idx += g_chars;
        }

        // PHOSPHOR PATCH 6 — a click past the end of a wrapped row stays on
        // *that* row. Landing on `end_col` of a non-final segment would render
        // the cursor one row lower, on the continuation, which is not the row
        // the user clicked; and the whole-line clamp below is about the end of
        // a line, which a non-final segment is not.
        if let Some(span) = span {
            if !span.is_last_segment && char_idx >= span.end_col {
                char_idx = span.end_col.saturating_sub(1).max(span.start_col);
            }
            return Some(line_start_char + char_idx);
        }

        let line = self
            .code
            .char_slice(line_start_char, line_start_char + line_len);
        // PHOSPHOR PATCH 11 — was a grapheme sum, which measures a tab as one
        // cell and so declared a click *inside* a tab-indented line to be past
        // its end, snapping the cursor to the newline.
        let visual_width = self.code.char_col_to_visual(clicked_row, line_len);

        if clicked_col + self.offset_x >= visual_width {
            let mut end_idx = line.len_chars();
            if end_idx > 0 && line.char(end_idx - 1) == '\n' {
                end_idx -= 1;
            }
            char_idx = end_idx;
        }

        Some(line_start_char + char_idx)
    }

    // PHOSPHOR PATCH 9 — orphaned by the deletion of `Editor::mouse`, which was
    // its only caller. Kept rather than deleted: it is upstream's own working
    // code and the day phosphor wants click-to-fold it is where that starts.
    // See VENDOR.md §9.
    #[allow(dead_code)]
    pub(crate) fn toggle_fold_at_mouse(&mut self, mouse_x: u16, mouse_y: u16, area: &Rect) -> bool {
        if !self.is_code_folding_enabled() {
            return false;
        }

        let line_number_width = self.get_line_number_width();
        let fold_gutter_width = self.fold_gutter_width();
        let Some(fold_gutter_start) = line_number_width.checked_sub(fold_gutter_width) else {
            return false;
        };

        if mouse_x < area.left() + fold_gutter_start as u16
            || mouse_x >= area.left() + line_number_width as u16
            || mouse_y < area.top()
            || mouse_y >= area.bottom()
        {
            return false;
        }

        let visual_row = self.offset_y + (mouse_y - area.top()) as usize;
        let Some(line) = self.line_for_visual_row(visual_row) else {
            return false;
        };

        self.toggle_fold_at_line(line)
    }

    pub fn expand_hidden_diff_at_mouse(&mut self, mouse_x: u16, mouse_y: u16, area: &Rect) -> bool {
        if !self.is_diff_focus_active() {
            return false;
        }

        let line_number_width = self.get_line_number_width() as u16;
        if mouse_y < area.top()
            || mouse_y >= area.bottom()
            || mouse_x < area.left() + line_number_width
        {
            return false;
        }

        let clicked_visual_row = (mouse_y - area.top()) as usize + self.offset_y;
        let text_x = area.left() + line_number_width;
        let clicked_col = mouse_x.saturating_sub(text_x) as usize;
        let visible_width = area.width.saturating_sub(line_number_width) as usize;

        let expanded = self.view.expand_hidden_at_visual_row(
            &self.code,
            self.original_code.as_ref(),
            self.active_view_mode(),
            clicked_visual_row,
            clicked_col,
            visible_width,
            self.diff_options.expand_amount,
        );

        if expanded {
            self.clamp_offset_y();
        }

        expanded
    }

    /// Clears any active selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Extends or starts a selection from the current cursor to `new_cursor`.
    pub fn extend_selection(&mut self, new_cursor: usize) {
        // If there was already a selection, preserve the anchor (start point)
        // otherwise, use the current cursor as the anchor.
        let anchor = self.selection_anchor();
        self.selection = Some(Selection::from_anchor_and_cursor(anchor, new_cursor));
    }

    /// Returns the selection anchor position, or the cursor if no selection exists.
    pub fn selection_anchor(&self) -> usize {
        self.selection
            .as_ref()
            .map(|s| {
                if self.cursor == s.start {
                    s.end
                } else {
                    s.start
                }
            })
            .unwrap_or(self.cursor)
    }

    pub fn apply<A: Action>(&mut self, mut action: A) {
        action.apply(self);
    }

    pub fn set_content(&mut self, content: &str) {
        self.code.tx();
        self.code.set_state_before(self.cursor, self.selection);
        self.code.remove(0, self.code.len());
        self.code.insert(0, content);
        self.code.set_state_after(self.cursor, self.selection);
        self.code.commit();
        self.reset_highlight_cache();
    }

    pub fn set_original_code(&mut self, content: &str) -> Result<()> {
        let mut original = Code::new(content, self.code_ref().lang(), None)
            .or_else(|_| Code::new(content, "text", None))?;
        // PHOSPHOR PATCH 11 — a fresh `Code` starts at the default tabstop, and
        // a ghost row measured against a different stop than the row beside it
        // is a diff whose columns do not line up.
        original.set_tab_width(self.code.tab_width());
        self.highlights_cache.borrow_mut().clear();
        self.line_diff_cache.borrow_mut().clear();
        self.original_code = Some(original);
        self.rebuild_view();
        Ok(())
    }

    pub fn clear_original_code(&mut self) {
        self.highlights_cache.borrow_mut().clear();
        self.line_diff_cache.borrow_mut().clear();
        self.original_code = None;
        self.rebuild_view();
        self.clamp_offset_y();
    }

    pub fn has_diff(&self) -> bool {
        self.view_mode.has_diff() && self.original_code.is_some()
    }

    pub fn set_diff_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.view_mode == ViewMode::Plain {
                self.view_mode = ViewMode::Diff;
            }
        } else {
            self.view_mode = ViewMode::Plain;
        }
        self.rebuild_view();
        self.clamp_offset_y();
    }

    pub fn is_diff_enabled(&self) -> bool {
        self.view_mode.has_diff()
    }

    pub fn set_diff_focus_enabled(&mut self, enabled: bool) {
        self.view_mode = if enabled {
            ViewMode::DiffFocus {
                context_lines: self.diff_options.focus_context,
            }
        } else if self.view_mode.has_diff() {
            ViewMode::Diff
        } else {
            ViewMode::Plain
        };
        self.rebuild_view();
        self.clamp_cursor_to_focus_rows();
        self.clamp_offset_y();
    }

    pub fn toggle_diff_focus(&mut self) {
        self.set_diff_focus_enabled(!self.view_mode.is_diff_focus());
    }

    pub fn set_diff_focus_context(&mut self, context_lines: usize) {
        self.diff_options.focus_context = context_lines;
        if self.view_mode.is_diff_focus() {
            self.view_mode = ViewMode::DiffFocus { context_lines };
        }
        self.rebuild_view();
        self.clamp_cursor_to_focus_rows();
        self.clamp_offset_y();
    }

    pub fn set_diff_expand_amount(&mut self, amount: usize) {
        self.diff_options.expand_amount = amount;
        self.rebuild_view();
    }

    pub fn set_diff_options(&mut self, options: DiffOptions) {
        self.diff_options = options;
        if self.view_mode.is_diff_focus() {
            self.view_mode = ViewMode::DiffFocus {
                context_lines: options.focus_context,
            };
        }
        self.rebuild_view();
        self.clamp_cursor_to_focus_rows();
        self.clamp_offset_y();
    }

    pub fn diff_options(&self) -> DiffOptions {
        self.diff_options
    }

    pub fn apply_batch(&mut self, batch: &EditBatch) {
        self.code.tx();

        if let Some(state) = &batch.state_before {
            self.code.set_state_before(state.offset, state.selection);
        }
        if let Some(state) = &batch.state_after {
            self.code.set_state_after(state.offset, state.selection);
        }

        for edit in &batch.edits {
            match edit.operation {
                Operation::Insert => {
                    self.code.insert(edit.start, &edit.text);
                }
                Operation::Remove => {
                    self.code
                        .remove(edit.start, edit.start + edit.text.chars().count());
                }
            }
        }
        self.code.commit();
        self.reset_highlight_cache();
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.fit_cursor();
    }

    /// Toggles the Rust Tree-sitter fold that begins on `line_idx`.
    pub fn toggle_fold_at_line(&mut self, line_idx: usize) -> bool {
        if !self.code_folding_options.enabled {
            return false;
        }
        let top_line = self.line_for_visual_row(self.offset_y);
        let toggled = self.view.toggle_code_fold(
            &self.code,
            self.original_code.as_ref(),
            self.active_view_mode(),
            line_idx,
        );
        if toggled {
            if let Some(top_line) = top_line {
                let visual_line = self.visual_line_idx(top_line);
                if visual_line != usize::MAX {
                    self.offset_y = visual_line;
                }
            }
            self.clamp_offset_y();
        }
        toggled
    }

    pub fn toggle_fold_at_cursor(&mut self) -> bool {
        let line_idx = self.code.char_to_line(self.cursor);
        self.toggle_fold_at_line(line_idx)
    }

    pub fn set_code_folding_enabled(&mut self, enabled: bool) {
        self.code_folding_options.enabled = enabled;
        if !enabled {
            self.view.clear_code_folds();
        }
        self.rebuild_view();
    }

    pub fn is_code_folding_enabled(&self) -> bool {
        self.code_folding_options.enabled
    }

    pub fn set_code_folding_options(&mut self, options: CodeFoldingOptions) {
        let enabled = options.enabled;
        self.code_folding_options = options;
        if !enabled {
            self.view.clear_code_folds();
        }
        self.rebuild_view();
    }

    pub fn code_folding_options(&self) -> CodeFoldingOptions {
        self.code_folding_options.clone()
    }

    pub(crate) fn fold_gutter_width(&self) -> usize {
        // PHOSPHOR PATCH 7 — folding can be on with the column off.
        if !self.code_folding_options.enabled || !self.fold_gutter_visible {
            return 0;
        }
        self.code_folding_options
            .indicators
            .expanded
            .width()
            .max(self.code_folding_options.indicators.collapsed.width())
            + 1
    }

    pub(crate) fn code_fold_indicator(&self, line_idx: usize) -> Option<bool> {
        // PHOSPHOR PATCH 7 — this is the *gutter* glyph and nothing else, so
        // it follows the gutter rather than folding. Without the added
        // condition, hiding the column would draw the indicator over the first
        // cell of the text instead of dropping it.
        (self.code_folding_options.enabled && self.fold_gutter_visible)
            .then(|| self.view.code_fold_indicator(&self.code, line_idx))
            .flatten()
    }

    pub fn fit_cursor(&mut self) {
        // make sure cursor is not out of bounds
        let len = self.code.len_chars();
        self.cursor = self.cursor.min(len);

        // make sure cursor is not out of bounds on the line
        let (row, col) = self.code.point(self.cursor);
        if col > self.code.line_len(row) {
            self.cursor = self.code.line_to_char(row) + self.code.line_len(row);
        }
    }

    pub fn scroll_up(&mut self) {
        if self.offset_y > 0 {
            self.offset_y -= 1;
        }
    }

    pub fn scroll_down(&mut self, area_height: usize) {
        let len_lines = self.visual_len_lines();
        if self.offset_y < len_lines.saturating_sub(area_height) {
            self.offset_y += 1;
        }
    }

    pub fn build_theme(theme: &Vec<(&str, &str)>) -> Theme {
        theme
            .into_iter()
            .map(|(name, hex)| {
                let (r, g, b) = utils::rgb(hex);
                let color = Color::Rgb(r, g, b);
                let style = match *name {
                    "diff_added"
                    | "diff_added_word"
                    | "diff_deleted"
                    | "diff_deleted_word"
                    | "word_highlight" => Style::default().bg(color),
                    _ => Style::default().fg(color),
                };
                (name.to_string(), style)
            })
            .collect()
    }

    pub(crate) fn theme_style(&self, key: &str) -> Style {
        self.theme.get(key).cloned().unwrap_or_default()
    }

    pub fn get_content(&self) -> String {
        self.code.get_content()
    }

    pub fn get_content_slice(&self, start: usize, end: usize) -> String {
        self.code.slice(start, end)
    }

    pub fn get_cursor(&self) -> usize {
        self.cursor
    }

    // PHOSPHOR PATCH 3 — see VENDOR.md. `arboard` is now behind the `clipboard`
    // feature. Both functions already had an in-editor fallback for the case where
    // `arboard` fails at runtime; with the feature off that fallback is simply the
    // only path, so behaviour on a headless machine is unchanged and nothing links
    // X11/Wayland.
    pub fn set_clipboard(&mut self, text: &str) -> Result<()> {
        #[cfg(feature = "clipboard")]
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(text.to_string()))
            .unwrap_or_else(|_| self.clipboard = Some(text.to_string()));
        #[cfg(not(feature = "clipboard"))]
        {
            self.clipboard = Some(text.to_string());
        }
        Ok(())
    }

    pub fn get_clipboard(&self) -> Result<String> {
        #[cfg(feature = "clipboard")]
        return arboard::Clipboard::new()
            .and_then(|mut c| c.get_text())
            .ok()
            .or_else(|| self.clipboard.clone())
            .ok_or_else(|| anyhow!("cant get clipboard"));
        #[cfg(not(feature = "clipboard"))]
        return self
            .clipboard
            .clone()
            .ok_or_else(|| anyhow!("cant get clipboard"));
    }

    pub fn set_marks(&mut self, marks: Vec<(usize, usize, &str)>) {
        self.marks = Some(
            marks
                .into_iter()
                .map(|(start, end, color)| {
                    let (r, g, b) = utils::rgb(color);
                    (start, end, Color::Rgb(r, g, b))
                })
                .collect(),
        );
    }

    /// PHOSPHOR PATCH 4 — as [`Editor::set_marks`], for a caller that already
    /// owns `ratatui` colours. `set_marks` parses hex strings, and formatting a
    /// `Color` back to hex to get in here is a round-trip that only `Color::Rgb`
    /// survives.
    pub fn set_marks_colored(&mut self, marks: Vec<(usize, usize, Color)>) {
        self.marks = Some(marks);
    }

    pub fn remove_marks(&mut self) {
        self.marks = None;
    }

    /// PHOSPHOR PATCH 5 — the one call site for §3's undercurl.
    ///
    /// Replaces the styled spans wholesale, as `set_marks` does. A span asks
    /// for [`Underline::Curl`]; whether that reaches the terminal as SGR `4:3`
    /// or as a straight underline is [`UnderlineCapability`]'s business, not
    /// the caller's.
    ///
    /// [`Underline::Curl`]: crate::phosphor::cell_style::Underline::Curl
    /// [`UnderlineCapability`]: crate::phosphor::cell_style::UnderlineCapability
    pub fn set_styled_spans(&mut self, spans: Vec<crate::phosphor::cell_style::StyledSpan>) {
        self.styled_spans = spans;
    }

    /// PHOSPHOR PATCH 5 — clears every styled span.
    pub fn clear_styled_spans(&mut self) {
        self.styled_spans.clear();
    }

    /// PHOSPHOR PATCH 5 — the spans currently drawn.
    pub fn styled_spans(&self) -> &[crate::phosphor::cell_style::StyledSpan] {
        &self.styled_spans
    }

    /// PHOSPHOR PATCH 5 — which underline the terminal gets, detected from the
    /// environment unless [`Editor::set_underline_capability`] has said
    /// otherwise.
    pub fn underline_capability(&self) -> crate::phosphor::cell_style::UnderlineCapability {
        self.underline_capability
            .unwrap_or_else(crate::phosphor::cell_style::UnderlineCapability::detect)
    }

    /// PHOSPHOR PATCH 5 — overrides the detected capability. `None` restores
    /// detection. This is the terminal, not the call site: a test forces both
    /// halves of the degradation path through it, and an app layer that has
    /// negotiated capabilities itself can hand down what it learned.
    pub fn set_underline_capability(
        &mut self,
        capability: Option<crate::phosphor::cell_style::UnderlineCapability>,
    ) {
        self.underline_capability = capability;
    }

    pub fn has_marks(&self) -> bool {
        self.marks.is_some()
    }

    pub fn get_marks(&self) -> Option<&Vec<(usize, usize, Color)>> {
        self.marks.as_ref()
    }

    pub fn get_selection_text(&mut self) -> Option<String> {
        if let Some(selection) = &self.selection
            && !selection.is_empty()
        {
            let text = self.code.slice(selection.start, selection.end);
            return Some(text);
        }
        None
    }

    pub fn get_selection(&mut self) -> Option<Selection> {
        return self.selection;
    }

    pub fn set_selection(&mut self, selection: Option<Selection>) {
        self.selection = selection;
    }

    pub fn set_offset_y(&mut self, offset_y: usize) {
        self.offset_y = offset_y.min(self.visual_len_lines().saturating_sub(1));
    }

    pub fn set_offset_x(&mut self, offset_x: usize) {
        self.offset_x = offset_x;
    }

    pub fn get_offset_y(&self) -> usize {
        self.offset_y
    }

    pub fn get_offset_x(&self) -> usize {
        self.offset_x
    }

    // PHOSPHOR PATCH 4 — `pub`, was `pub(crate)`: the number of rows the widget
    // will draw is what a caller clamps its own scrolling against.
    pub fn visual_len_lines(&self) -> usize {
        self.view
            .visual_len_lines(&self.code, self.active_view_mode())
    }

    pub(crate) fn line_for_visual_row(&self, visual_row: usize) -> Option<usize> {
        self.view
            .line_for_visual_row(&self.code, self.active_view_mode(), visual_row)
    }

    // ------------------------------------------------------------------
    // PHOSPHOR PATCH 6 — soft wrap. The row stream is the single source of
    // truth for row<->line mapping, cursor placement, click targeting and
    // (from `T032`) virtual-text placement; everything below reads it.
    // ------------------------------------------------------------------

    /// Wraps text at `width` cells of the *text column* — the area width minus
    /// [`Editor::get_line_number_width`]. `None` restores upstream behaviour:
    /// long lines do not wrap and scroll horizontally instead.
    ///
    /// Rebuilds the row stream when the width changes, and nothing else: it
    /// does not move the viewport and does not touch the cursor.
    pub fn set_soft_wrap(&mut self, width: Option<usize>) {
        if self.view.set_soft_wrap(width) {
            self.rebuild_view();
        }
    }

    /// The wrap width, or `None` when wrapping is off.
    pub fn soft_wrap_width(&self) -> Option<usize> {
        self.view.soft_wrap()
    }

    // ------------------------------------------------------------------
    // PHOSPHOR PATCH 11 — the tabstop.
    // ------------------------------------------------------------------

    /// Sets how many cells a `\t` advances to, for this buffer and for the
    /// original it is diffed against.
    ///
    /// Rebuilds the row stream when the stop changes, and nothing else: a
    /// wider tab moves the column every wrap point is measured at, so a stale
    /// stream would wrap in the wrong place until the next resize. Same shape
    /// and same reason as [`Editor::set_soft_wrap`].
    ///
    /// The diff original is set alongside because a ghost row is drawn against
    /// the *live* buffer's stop ([`crate::render`]), and a ghost whose own
    /// `Code` measured tabs differently would place its columns somewhere the
    /// renderer does not draw them.
    pub fn set_tab_width(&mut self, tab_width: usize) {
        let changed = self.code.set_tab_width(tab_width);
        if let Some(original) = self.original_code.as_mut() {
            original.set_tab_width(tab_width);
        }
        if changed {
            self.rebuild_view();
        }
    }

    /// How many cells a `\t` advances to.
    pub fn tab_width(&self) -> usize {
        self.code.tab_width()
    }

    /// What a visual row draws — see [`RowSpan`]. `None` for rows that are not
    /// a slice of the current buffer (fold separators, diff ghosts).
    pub fn row_span(&self, visual_row: usize) -> Option<RowSpan> {
        self.view
            .row_span(&self.code, self.active_view_mode(), visual_row)
    }

    /// The visual row showing `char_col` of `line_idx` — the wrapped-line
    /// answer to [`Editor::visual_line_idx`], which returns the row a line
    /// *starts* on. A column on a segment boundary belongs to the later row.
    pub fn visual_row_for_position(&self, line_idx: usize, char_col: usize) -> Option<usize> {
        self.view
            .visual_row_for_position(self.active_view_mode(), line_idx, char_col)
    }

    /// The visual row the cursor is on, segments included.
    pub fn visual_row_for_cursor(&self) -> Option<usize> {
        let (line, col) = self.code.point(self.cursor);
        self.visual_row_for_position(line, col)
    }

    /// The char offset at `visual_col` cells into `visual_row`, clamped to the
    /// row's own span. `None` for a row that draws no buffer text.
    pub fn cursor_at_visual_row_col(&self, visual_row: usize, visual_col: usize) -> Option<usize> {
        let span = self.row_span(visual_row)?;
        let line_start = self.code.line_to_char(span.line_idx);
        let base = self.code.char_col_to_visual(span.line_idx, span.start_col);
        let col = self
            .code
            .visual_to_char_col(span.line_idx, base + visual_col)
            .clamp(span.start_col, span.end_col);
        Some(line_start + col)
    }

    /// One visual row up (`-1`) or down (`1`) from the cursor, keeping its
    /// column within the row. `None` when wrapping is off — which is what
    /// makes `MoveUp`/`MoveDown` fall through to their upstream, line-wise
    /// behaviour — or when there is no such row.
    pub fn soft_wrap_row_step(&self, delta: isize) -> Option<usize> {
        self.soft_wrap_width()?;
        let from = self.visual_row_for_cursor()?;
        let mut to = from.checked_add_signed(delta)?;
        if to >= self.visual_len_lines() {
            return None;
        }
        // PHOSPHOR PATCH 8 — a virtual row holds no cursor, so vertical motion
        // passes over it rather than stalling on it. Stepping in the direction
        // of travel is what keeps `j` on a row with a thread under it from
        // being a no-op.
        let step: isize = if delta < 0 { -1 } else { 1 };
        while self.virtual_row_indent(to).is_some() {
            to = to.checked_add_signed(step)?;
            if to >= self.visual_len_lines() {
                return None;
            }
        }
        let span = self.row_span(from)?;
        let (line, col) = self.code.point(self.cursor);
        let visual_col = self
            .code
            .char_col_to_visual(line, col)
            .saturating_sub(self.code.char_col_to_visual(span.line_idx, span.start_col));
        self.cursor_at_visual_row_col(to, visual_col)
    }

    // ------------------------------------------------------------------
    // PHOSPHOR PATCH 8 — virtual text. Rows in the same stream, so the four
    // subsystems above cannot disagree with them about where a row is.
    // ------------------------------------------------------------------

    /// Replaces the `┊` rows hanging from this buffer, in draw order.
    ///
    /// Rebuilds the row stream when the list changes, and nothing else: it
    /// does not move the viewport and does not touch the cursor. A row whose
    /// anchor the stream does not show — inside a collapsed fold, or past the
    /// end of the buffer — is simply not drawn.
    pub fn set_virtual_lines(&mut self, lines: Vec<VirtualLine>) {
        if self.view.set_virtual_lines(lines) {
            self.rebuild_view();
        }
    }

    /// Removes every `┊` row.
    pub fn clear_virtual_lines(&mut self) {
        self.set_virtual_lines(Vec::new());
    }

    /// The `┊` rows currently installed, drawn or not.
    pub fn virtual_lines(&self) -> &[VirtualLine] {
        self.view.virtual_lines()
    }

    /// Shows or hides every `┊` row without discarding the list.
    pub fn set_virtual_text_visible(&mut self, visible: bool) {
        if self.view.set_virtual_visible(visible) {
            self.rebuild_view();
        }
    }

    /// Whether the `┊` rows are drawn.
    pub fn virtual_text_visible(&self) -> bool {
        self.view.virtual_visible()
    }

    /// The virtual line a visual row draws, or `None` when the row is buffer
    /// text. **The predicate every row-indexed column needs**: a state bar or
    /// a gutter built per visual row has to leave these rows alone, because a
    /// virtual row is not a line and marking one would be a lie about how many
    /// there are.
    pub fn virtual_line_at(&self, visual_row: usize) -> Option<&VirtualLine> {
        match self.visual_row(visual_row)? {
            VisualRow::Virtual { index, .. } => self.virtual_lines().get(index),
            _ => None,
        }
    }

    /// Cells of indent before the `┊` on a virtual row — 0 under a whole line,
    /// 2 under a `↪` continuation. `None` when the row is not a virtual one.
    pub fn virtual_row_indent(&self, visual_row: usize) -> Option<usize> {
        match self.visual_row(visual_row)? {
            VisualRow::Virtual { indent, .. } => Some(indent),
            _ => None,
        }
    }

    /// PHOSPHOR PATCH 7 — how many lines the collapsed fold starting on
    /// `line_idx` hides, or `None` when there is none. The `n` in `▸⋯ n lines`.
    pub fn fold_hidden_lines(&self, line_idx: usize) -> Option<usize> {
        self.code_folding_options
            .enabled
            .then(|| self.view.code_fold_hidden_lines(&self.code, line_idx))
            .flatten()
    }

    pub(crate) fn visual_row(&self, visual_row: usize) -> Option<VisualRow> {
        if self.has_diff() || !self.view.rows().is_empty() {
            self.view.rows().get(visual_row).cloned()
        } else if visual_row < self.code.len_lines() {
            Some(VisualRow::Real {
                line_idx: visual_row,
                is_added: false,
                orig_line_idx: None,
            })
        } else {
            None
        }
    }

    pub(crate) fn visual_line_idx(&self, line_idx: usize) -> usize {
        self.view
            .visual_row_for_line(self.active_view_mode(), line_idx)
            .unwrap_or(usize::MAX)
    }

    pub fn code_mut(&mut self) -> &mut Code {
        &mut self.code
    }

    pub fn code_ref(&self) -> &Code {
        &self.code
    }

    /// Set the change callback function for handling document changes
    pub fn set_change_callback(
        &mut self,
        callback: Box<dyn Fn(Vec<(usize, usize, usize, usize, String)>)>,
    ) {
        self.code.set_change_callback(callback);
    }

    pub fn highlight_interval(
        &self,
        start: usize,
        end: usize,
        theme: &Theme,
    ) -> Vec<(usize, usize, Style)> {
        let mut cache = self.highlights_cache.borrow_mut();
        let key = (0, start, end);
        if let Some(v) = cache.get(&key) {
            return v.clone();
        }

        let highlights = self.code.highlight_interval(start, end, theme);
        cache.insert(key, highlights.clone());
        highlights
    }

    pub fn highlight_interval_original(
        &self,
        start: usize,
        end: usize,
        theme: &Theme,
    ) -> Vec<(usize, usize, Style)> {
        let Some(original) = &self.original_code else {
            return Vec::new();
        };
        let mut cache = self.highlights_cache.borrow_mut();
        let key = (1, start, end);
        if let Some(v) = cache.get(&key) {
            return v.clone();
        }

        let highlights = original.highlight_interval(start, end, theme);
        cache.insert(key, highlights.clone());
        highlights
    }

    pub fn word_highlight_ranges(&self) -> Vec<(usize, usize)> {
        if !self.word_highlight_enabled {
            return Vec::new();
        }

        let mut cache = self.word_highlight_cache.borrow_mut();
        if let Some((cached_cursor, cached_ranges)) = &*cache {
            if *cached_cursor == self.cursor {
                return cached_ranges.clone();
            }
        }

        let (start, end) = self.code.word_boundaries(self.cursor);
        if start == end {
            *cache = Some((self.cursor, Vec::new()));
            return Vec::new();
        }

        let word = self.code.slice(start, end);
        if word.is_empty() {
            *cache = Some((self.cursor, Vec::new()));
            return Vec::new();
        }

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
        if !word.chars().next().map_or(false, is_word_char) {
            *cache = Some((self.cursor, Vec::new()));
            return Vec::new();
        }

        let mut ranges = Vec::new();
        let word_chars: Vec<char> = word.chars().collect();
        let word_len = word_chars.len();
        let content = &self.code.content;
        let total_chars = content.len_chars();

        if total_chars >= word_len {
            for line_idx in 0..content.len_lines() {
                let line = content.line(line_idx);
                let line_len_chars = line.len_chars();
                if line_len_chars < word_len {
                    continue;
                }

                let line_chars: Vec<char> = line.chars().collect();
                let line_start_char = content.line_to_char(line_idx);

                for i in 0..=(line_chars.len() - word_len) {
                    if line_chars[i..(i + word_len)] == word_chars {
                        let prev_ok = i == 0 || !is_word_char(line_chars[i - 1]);
                        let next_idx = i + word_len;
                        let next_ok = next_idx >= line_chars.len() || !is_word_char(line_chars[next_idx]);
                        if prev_ok && next_ok {
                            ranges.push((line_start_char + i, line_start_char + next_idx));
                        }
                    }
                }
            }
        }

        *cache = Some((self.cursor, ranges.clone()));
        ranges
    }

    pub fn set_word_highlight_enabled(&mut self, enabled: bool) {
        self.word_highlight_enabled = enabled;
        if !enabled {
            self.word_highlight_cache.borrow_mut().take();
        }
    }

    pub fn word_highlight_enabled(&self) -> bool {
        self.word_highlight_enabled
    }

    pub fn reset_highlight_cache(&mut self) {
        self.highlights_cache.borrow_mut().clear();
        self.line_diff_cache.borrow_mut().clear();
        self.word_highlight_cache.borrow_mut().take();
        self.rebuild_view();
    }

    pub fn get_line_diff(
        &self,
        orig_idx: usize,
        curr_idx: usize,
        is_ghost: bool,
    ) -> Vec<(usize, usize)> {
        let Some(orig_code) = &self.original_code else {
            return Vec::new();
        };

        let mut cache = self.line_diff_cache.borrow_mut();
        let key = (orig_idx, curr_idx);

        let diff = cache.entry(key).or_insert_with(|| {
            crate::diff::compute_line_diff(orig_code, orig_idx, &self.code, curr_idx)
        });

        if is_ghost {
            diff.deletions.clone()
        } else {
            diff.additions.clone()
        }
    }
}

impl Editor {

    fn clamp_offset_y(&mut self) {
        self.offset_y = self.offset_y.min(self.visual_len_lines().saturating_sub(1));
    }

    pub(crate) fn prev_line(&self, line_idx: usize) -> Option<usize> {
        self.view.prev_line(self.active_view_mode(), line_idx)
    }

    pub(crate) fn next_line(&self, line_idx: usize) -> Option<usize> {
        self.view
            .next_line(&self.code, self.active_view_mode(), line_idx)
    }

    pub(crate) fn is_diff_focus_active(&self) -> bool {
        self.has_diff() && self.view_mode.is_diff_focus()
    }

    pub(crate) fn clamp_cursor_to_focus_rows(&mut self) {
        let clear_selection = self.is_diff_focus_active();
        let (cursor_line, cursor_char_col) = self.code.point(self.cursor);
        if self.line_visible(cursor_line) {
            return;
        }

        let current_visual_col = self.code.char_col_to_visual(cursor_line, cursor_char_col);
        let Some(target_line) = self.nearest_focus_real_line(cursor_line) else {
            return;
        };
        let target_start = self.code.line_to_char(target_line);
        let target_len = self.code.line_len(target_line);
        let target_col = self
            .code
            .visual_to_char_col(target_line, current_visual_col)
            .min(target_len);

        self.cursor = target_start + target_col;
        if clear_selection {
            self.clear_selection();
        }
    }

    pub(crate) fn line_visible(&self, line_idx: usize) -> bool {
        self.view.line_visible(self.active_view_mode(), line_idx)
    }

    fn nearest_focus_real_line(&self, line_idx: usize) -> Option<usize> {
        self.view
            .nearest_line(&self.code, self.active_view_mode(), line_idx)
    }

    /// calculates visible cursor position
    pub fn get_visible_cursor(&self, area: &Rect) -> Option<(u16, u16)> {
        let line_number_width = self.get_line_number_width();

        let (cursor_line, cursor_char_col) = self.code.point(self.cursor);
        // PHOSPHOR PATCH 6 — the cursor sits on its *segment*, and measures its
        // column from that segment's start rather than from the line's.
        let cursor_visual_line = self
            .visual_row_for_position(cursor_line, cursor_char_col)
            .unwrap_or(usize::MAX);
        let wrap_span = self.row_span(cursor_visual_line).filter(|span| span.wrapped);

        if cursor_visual_line >= self.offset_y
            && cursor_visual_line < self.offset_y + area.height as usize
        {
            let max_x = (area.width as usize).saturating_sub(line_number_width);
            // PHOSPHOR PATCH 6 — wrapped rows have no horizontal offset; the
            // segment's own start is what the cursor is relative to.
            let start_col = match wrap_span {
                Some(span) => span.start_col,
                None => self.offset_x,
            };
            let prefix_cells = wrap_span.map_or(0, |span| span.prefix_cells);

            // PHOSPHOR PATCH 11 — both were grapheme sums, which measure a tab
            // as one cell; `Code::char_col_to_visual` is the same walk with the
            // tabstop in it. Routed through it rather than repeating the rule
            // here, so the column the cursor is *drawn* at cannot disagree with
            // the column every motion computes.
            let cursor_visual_col = self.code.char_col_to_visual(cursor_line, cursor_char_col);
            let offset_visual_col = self.code.char_col_to_visual(cursor_line, start_col);

            let relative_visual_col = cursor_visual_col.saturating_sub(offset_visual_col);
            // PHOSPHOR PATCH 6 — past the `↪ ` marker on a continuation row.
            let visible_x = (relative_visual_col + prefix_cells).min(max_x);

            let cursor_x = area.left() + (line_number_width + visible_x) as u16;
            let cursor_y = area.top() + (cursor_visual_line - self.offset_y) as u16;

            if cursor_x < area.right() && cursor_y < area.bottom() {
                return Some((cursor_x, cursor_y));
            }
        }

        return None;
    }

    pub fn show_line_numbers(&mut self, show: bool) {
        self.show_line_numbers = show
    }

    pub fn set_left_code_padding(&mut self, char_count: usize) {
        self.left_code_padding = char_count
    }

    /// PHOSPHOR PATCH 4 — sets the minimum width of the line-number column.
    /// The default is 5, the constant this replaced.
    pub fn set_line_number_min_digits(&mut self, digits: usize) {
        self.line_number_min_digits = digits
    }

    /// PHOSPHOR PATCH 4 — replaces the syntax theme with an already-built one.
    /// [`Editor::new`] takes `Vec<(&str, &str)>` of hex strings; a caller that
    /// already owns `ratatui` styles has no way in short of formatting them
    /// back to hex. Invalidates the highlight cache, which bakes the styles in.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.reset_highlight_cache();
    }

    /// PHOSPHOR PATCH 7 — adds one entry to the theme map without replacing it.
    /// The renderer's non-syntax keys (`line_number`, `default_text`,
    /// `fold_marker`, `wrap_indicator`, `trailing_whitespace`) are not
    /// tree-sitter captures, so a consumer that owns them has no reason to
    /// rebuild the whole syntax map to change one.
    pub fn set_theme_key(&mut self, key: &str, style: Style) {
        self.theme.insert(key.to_string(), style);
        self.reset_highlight_cache();
    }

    /// PHOSPHOR PATCH 7 — shows or hides the fold gutter column, independently
    /// of whether folding works. `true` is upstream's behaviour.
    pub fn set_fold_gutter_visible(&mut self, visible: bool) {
        self.fold_gutter_visible = visible;
    }

    /// PHOSPHOR PATCH 7 — marks trailing whitespace with `·`. Off by default;
    /// `8e` shows it in INSERT only.
    pub fn set_show_trailing_whitespace(&mut self, show: bool) {
        self.show_trailing_whitespace = show;
    }

    /// PHOSPHOR PATCH 7 — whether trailing whitespace is currently marked.
    pub fn show_trailing_whitespace(&self) -> bool {
        self.show_trailing_whitespace
    }

    fn active_view_mode(&self) -> ViewMode {
        if self.original_code.is_some() {
            self.view_mode
        } else {
            ViewMode::Plain
        }
    }

    pub(crate) fn rebuild_view(&mut self) {
        self.view.rebuild(
            &self.code,
            self.original_code.as_ref(),
            self.active_view_mode(),
        );
    }
}
