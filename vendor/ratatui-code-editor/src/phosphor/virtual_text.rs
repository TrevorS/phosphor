//! PHOSPHOR PATCH 8 — virtual text, as a variant of the row stream.
//!
//! Upstream has no notion of a row that is not a line. Phosphor's design
//! language wants `┊`-prefixed rows hanging from a region — threads, watches,
//! diagnostics and the once-per-session unknown-key hint all render through the
//! same primitive (`T032`) — and `T081` already settled the shape this has to
//! take: `View`'s row list is what row↔line mapping, cursor placement, click
//! targeting and virtual-text placement all read, so a virtual row that lives
//! *above* that list desynchronises the other three.
//!
//! So the row is a [`VisualRow::Virtual`] variant like `Wrapped` is, and this
//! module owns exactly one thing: deciding which row each virtual line hangs
//! under. Everything that consumes rows reads it through the same helpers it
//! already read `Real` and `Wrapped` rows through.
//!
//! # What a virtual row is not
//!
//! It carries **no `line_idx`**. That is the whole of "a virtual row never
//! shifts the buffer's own line numbering": the renderer prints a number from
//! the row's `line_idx`, and a row with none prints none, so inserting one
//! between two lines leaves every number below it exactly where it was.
//! `View::line_for_visual_row` and `View::row_span` answer `None` for it, which
//! is what makes a click on one resolve to no cursor.
//!
//! # The anchor is a position, not a line
//!
//! [`VirtualLine::col`] exists so a wrapped line places correctly: the row a
//! virtual line hangs under is the *segment* showing `(line_idx, col)`, found
//! the same way [`crate::view::View::visual_row_for_position`] finds it, and
//! the indent it inherits is that segment's own text start — 0 on a whole line
//! or a first segment, [`CONTINUATION_PREFIX`] on a `↪` continuation. Design
//! Language §3 is *"virtual text indents to code column"*, and on a
//! continuation row the code column is two cells further in.
//!
//! **Anchors are not maintained here.** A virtual line names a buffer position
//! and the caller re-installs the list when its anchors move (`T042`/`T043`);
//! a line naming a position the stream does not show — inside a collapsed fold,
//! or past the end of the buffer — is dropped from the stream rather than
//! clamped somewhere it was not asked to be.

use ratatui_core::style::Style;

use crate::phosphor::soft_wrap::CONTINUATION_PREFIX;
use crate::types::VisualRow;

/// One styled piece of a virtual row's text.
///
/// Styles arrive already resolved. The fork has no palette and must not grow
/// one: `phosphor-ui` builds these from its `Theme`, which is the only place
/// a colour is allowed to come from.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualRun {
    pub text: String,
    pub style: Style,
}

impl VirtualRun {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// One `┊` row, and the buffer position it hangs from.
///
/// The rail glyph is **not** in `runs` — the renderer draws it, so every
/// virtual row is prefixed identically and no caller can forget it.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualLine {
    /// Source line the row hangs under, 0-based.
    pub line_idx: usize,
    /// Char column within that line. Only matters when the line is wrapped,
    /// where it picks the segment; a column past the line's end lands on the
    /// last segment.
    pub col: usize,
    /// Opaque owner tag — phosphor's `RegionId`, which this fork never
    /// interprets. `None` for an unowned row, such as the unknown-key hint.
    pub owner: Option<u64>,
    /// What the row says, after the rail.
    pub runs: Vec<VirtualRun>,
}

impl VirtualLine {
    /// A row hanging from the start of `line_idx`.
    pub fn new(line_idx: usize, runs: Vec<VirtualRun>) -> Self {
        Self {
            line_idx,
            col: 0,
            owner: None,
            runs,
        }
    }

    /// Hangs from `col` of the line rather than its start — the wrapped-line
    /// case, where the two are different rows.
    pub fn at_col(mut self, col: usize) -> Self {
        self.col = col;
        self
    }

    /// Tags the row with the region that owns it.
    pub fn owned_by(mut self, owner: u64) -> Self {
        self.owner = Some(owner);
        self
    }
}

/// Inserts a [`VisualRow::Virtual`] under each line's anchor row.
///
/// Runs **after** [`crate::phosphor::soft_wrap::apply`], so the segments a
/// wrapped line owns already exist to choose between. Lines whose anchor row
/// is not in the stream are dropped. Several rows anchored to the same row keep
/// the order they were given in, which is what lets a thread render as an
/// exchange rather than a set.
pub(crate) fn apply(rows: Vec<VisualRow>, lines: &[VirtualLine]) -> Vec<VisualRow> {
    if lines.is_empty() || rows.is_empty() {
        return rows;
    }

    let mut hangers: Vec<Vec<VisualRow>> = vec![Vec::new(); rows.len()];
    for (index, line) in lines.iter().enumerate() {
        if let Some((row, indent)) = anchor_row(&rows, line) {
            hangers[row].push(VisualRow::Virtual { index, indent });
        }
    }

    let mut out = Vec::with_capacity(rows.len() + lines.len());
    for (idx, row) in rows.into_iter().enumerate() {
        out.push(row);
        out.append(&mut hangers[idx]);
    }
    out
}

/// The row `line` hangs under, and the indent it inherits from it.
///
/// The same rule [`crate::view::View::visual_row_for_position`] uses, over a
/// stream that has no virtual rows in it yet: the first segment whose
/// `end_col` is past the anchor column, or the line's last segment when the
/// column is past its end.
fn anchor_row(rows: &[VisualRow], line: &VirtualLine) -> Option<(usize, usize)> {
    let mut found: Option<(usize, usize)> = None;
    for (idx, row) in rows.iter().enumerate() {
        match row {
            // A line that fits is one row, and its text starts at the text
            // column: no indent to inherit.
            VisualRow::Real { line_idx, .. } if *line_idx == line.line_idx => {
                return Some((idx, 0));
            }
            VisualRow::Wrapped {
                line_idx,
                segment,
                end_col,
                ..
            } if *line_idx == line.line_idx => {
                let indent = if *segment == 0 { 0 } else { CONTINUATION_PREFIX };
                found = Some((idx, indent));
                if line.col < *end_col {
                    return found;
                }
            }
            // Past the run this line owns: the anchor column is past its end,
            // so the last segment is the answer.
            _ if found.is_some() => break,
            _ => {}
        }
    }
    found
}
