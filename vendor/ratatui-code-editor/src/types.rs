use ratatui_core::style::Style;
use std::collections::HashMap;

// keyword and ratatui style
pub type Theme = HashMap<String, Style>;
// start byte, end byte, style
pub(crate) type Hightlight = (usize, usize, Style);
// source id, start offset, end offset
pub(crate) type HightlightCache = HashMap<(u8, usize, usize), Vec<Hightlight>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LineDiff {
    pub(crate) deletions: Vec<(usize, usize)>,
    pub(crate) additions: Vec<(usize, usize)>,
}

pub(crate) type LineDiffCache = HashMap<(usize, usize), LineDiff>;

/// PHOSPHOR PATCH 6 — what one visual row draws, resolved once so that every
/// consumer of the row stream reads the same answer. See `View::row_span`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSpan {
    /// Source line this row is a slice of.
    pub line_idx: usize,
    /// `0` for an unwrapped line, and for the first segment of a wrapped one.
    pub segment: usize,
    /// First char column drawn, inclusive.
    pub start_col: usize,
    /// Last char column drawn, exclusive.
    pub end_col: usize,
    /// Cells spent before the text — the `↪ ` marker on a continuation row.
    pub prefix_cells: usize,
    /// Whether this row is one segment of a soft-wrapped line. `false` means
    /// the row draws a whole line and honours the horizontal offset.
    pub wrapped: bool,
    /// Whether this row carries the end of its line.
    pub is_last_segment: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VisualRow {
    Real {
        line_idx: usize,
        is_added: bool,
        orig_line_idx: Option<usize>,
    },
    FoldSeparator {
        hidden_lines: usize,
        hidden_start: usize,
        hidden_end: usize,
    },
    GhostDeleted {
        anchor_line: usize,
        original_line_idx: usize,
        curr_line_idx: Option<usize>,
    },
    /// PHOSPHOR PATCH 6 — one segment of a soft-wrapped source line.
    ///
    /// A line only ever becomes `Wrapped` when it does not fit: a line that
    /// fits stays [`VisualRow::Real`], so with wrapping off the row stream is
    /// byte-for-byte what upstream builds. A wrapped line owns a contiguous
    /// run of `segment = 0..n` rows in document order, and their
    /// `[start_col, end_col)` char spans partition the line exactly.
    ///
    /// `segment > 0` carries no line number — it renders `↪` instead.
    Wrapped {
        line_idx: usize,
        segment: usize,
        /// First char column of the line this row draws, inclusive.
        start_col: usize,
        /// Last char column of the line this row draws, exclusive.
        end_col: usize,
        is_added: bool,
        orig_line_idx: Option<usize>,
    },
    /// PHOSPHOR PATCH 8 — a `┊` row hanging under the row that shows its
    /// anchor. Threads, watches, diagnostics and hints all render as this.
    ///
    /// **It carries no `line_idx`, deliberately.** A virtual row is not a
    /// line: it prints no line number, resolves to no source line, and owns
    /// no char span, so inserting one shifts nothing about the numbering of
    /// the rows below it. See `crate::phosphor::virtual_text`.
    Virtual {
        /// Which of the editor's virtual lines this row draws.
        index: usize,
        /// Cells of indent before the `┊`, inherited from the anchor row's
        /// own text start: 0 under a whole line, 2 under a `↪` continuation.
        indent: usize,
    },
}

impl VisualRow {
    pub(crate) fn is_changed(&self) -> bool {
        match self {
            VisualRow::Real { is_added, .. } => *is_added,
            VisualRow::FoldSeparator { .. } => false,
            VisualRow::GhostDeleted { .. } => true,
            // PHOSPHOR PATCH 6
            VisualRow::Wrapped { is_added, .. } => *is_added,
            // PHOSPHOR PATCH 8 — a virtual row is not part of the buffer, so
            // it is never a change to it. It must not pull diff context in.
            VisualRow::Virtual { .. } => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffOptions {
    pub focus_context: usize,
    pub expand_amount: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            focus_context: 3,
            expand_amount: 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldIndicators {
    pub expanded: String,
    pub collapsed: String,
}

impl FoldIndicators {
    pub fn unicode() -> Self {
        Self {
            expanded: "▼".into(),
            collapsed: "▶".into(),
        }
    }
    pub fn ascii() -> Self {
        Self {
            expanded: "v".into(),
            collapsed: ">".into(),
        }
    }
}

impl Default for FoldIndicators {
    fn default() -> Self {
        Self::unicode()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeFoldingOptions {
    pub enabled: bool,
    pub indicators: FoldIndicators,
}

impl Default for CodeFoldingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            indicators: FoldIndicators::default(),
        }
    }
}
