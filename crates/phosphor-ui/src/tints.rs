//! Region row tints, through the fork's marks API (`T087`).
//!
//! Design Language §3 tints the whole row per region state — `#141d16` anchor,
//! `#26332a` selection-in-float, `#211114` failure — and the bought marks API
//! carries exactly that (colour spans) and nothing else. `T008`'s spike said
//! this was the seam marks *are* good for, and nothing was tasked to build it
//! until now.
//!
//! # Three consequences of the API, and what each one forces
//!
//! **Marks carry no id.** A mark is `(start, end, colour)` and nothing more, so
//! there is no way to ask the editor *"which region is this one"*. Hence
//! [`Tints`]: a side table keyed by offset range, held on this side, which is
//! the *only* thing that knows a mark and a region are the same fact.
//!
//! **`set_marks` replaces wholesale** (`editor.rs:782`). Every seen-state
//! change would re-upload the full set, so this **diffs before uploading** and
//! [`Tints::sync`] is a no-op on a frame where nothing moved. That is what
//! keeps it off the hot path: the loop may call it every frame, and on a file
//! with 500 regions and no news it costs one comparison of a vector it already
//! owns.
//!
//! **The state column and the undercurl are not marks.** Those are `T031` and
//! `T085`, resolved separately and composed per row. This draws the row
//! *ground* and nothing else, which is why it can be a flat list of ranges
//! while the column has a priority ladder.
//!
//! # Offsets are recomputed, never remembered
//!
//! `T087`'s second acceptance is that *"the side table survives an edit that
//! shifts every offset"*. It does, and the reason is that there is nothing to
//! survive: the table holds what was last **uploaded**, and the desired set is
//! recomputed from the store's line-and-column spans against the *current*
//! buffer on every sync. An edit that shifts every offset produces a different
//! desired set, the diff sees it, and one upload follows.
//!
//! Remembering offsets and patching them is the other design, and it is the one
//! that goes wrong: it needs every edit to be observed, and an edit applied by
//! a path that forgot to tell it leaves the tints silently wrong.

use ratatui_code_editor::editor::Editor;
use ratatui_core::style::Color;

use phosphor_core::request::Span;

use crate::gutter::RegionState;
use crate::theme::Theme;

/// The marks currently uploaded to an editor, so a sync can tell whether it has
/// anything to say.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tints {
    uploaded: Vec<(usize, usize, Color)>,
}

impl Tints {
    /// A table that has uploaded nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many marks are currently uploaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.uploaded.len()
    }

    /// Whether none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.uploaded.is_empty()
    }

    /// Bring the editor's marks in line with `regions`, and answer whether that
    /// needed an upload.
    ///
    /// **The answer is the point.** A caller that wants to know whether the
    /// frame changed reads it; `T079`'s cache is the eventual reader. A sync
    /// that uploaded on every call would make that question unanswerable and
    /// the cache useless.
    pub fn sync(
        &mut self,
        editor: &mut Editor,
        theme: &Theme,
        regions: &[(Span, RegionState)],
    ) -> bool {
        let wanted = marks(editor, theme, regions);
        if wanted == self.uploaded {
            return false;
        }
        // `remove_marks` rather than uploading an empty vector: the fork's
        // `marks` is an `Option`, and `Some(vec![])` is a set with nothing in
        // it where `None` is *no marks at all*. They draw the same and only one
        // of them is what "this buffer has no regions" means.
        if wanted.is_empty() {
            editor.remove_marks();
        } else {
            editor.set_marks_colored(wanted.clone());
        }
        self.uploaded = wanted;
        true
    }
}

/// The marks a set of regions wants, in offset order.
///
/// Sorted and deduplicated so the diff in [`Tints::sync`] compares *sets*
/// rather than orderings — the store answers regions in id order, and a
/// declaration that renumbers nothing but arrives second would otherwise look
/// like news.
fn marks(
    editor: &Editor,
    theme: &Theme,
    regions: &[(Span, RegionState)],
) -> Vec<(usize, usize, Color)> {
    let mut out: Vec<(usize, usize, Color)> = regions
        .iter()
        .filter_map(|(span, state)| {
            let colour = tint(theme, *state)?;
            let start = offset(editor, span.start.line, span.start.column)?;
            let end = offset(editor, span.end.line, span.end.column)?;
            (start < end).then_some((start, end, colour))
        })
        .collect();
    out.sort_by_key(|(start, end, _)| (*start, *end));
    out.dedup();
    out
}

/// §3's row tint for a state, or [`None`] for a state that has none.
///
/// **Seen regions get no tint**, which is §3's row 18 — *"seen — marker
/// cleared, line is plain"* — and is the whole visible behaviour of `s`: the
/// ground goes back to the editor's. A tint for seen would make marking
/// something a change of colour rather than a return to normal.
fn tint(theme: &Theme, state: RegionState) -> Option<Color> {
    match state {
        // §3's anchor tint. Claude wrote it and you have not looked.
        RegionState::Unseen | RegionState::Thread | RegionState::Watch => {
            Some(theme.regions.anchor)
        }
        // §3's failure tint, for the states §1 calls trouble. `NeedsYou` is
        // amber rather than red in the *column* — §1's attention tier — but §3
        // gives it no row tint of its own, and inventing a fourth would be a
        // colour the language does not have. It takes the failure ground with
        // the rest, which is what `RegionState::mark` already does one layer
        // up by giving both the same tier.
        RegionState::Diagnostic
        | RegionState::Warning
        | RegionState::Failure
        | RegionState::NeedsYou => Some(theme.regions.failure),
        RegionState::Seen => None,
    }
}

/// A 1-based line and column as the editor's character offset.
///
/// [`None`] past the end of the buffer, which is what a region whose file has
/// been truncated under it answers — and dropping it is right: a mark on a row
/// that is not there would be a tint on nothing.
fn offset(editor: &Editor, line: u32, column: u32) -> Option<usize> {
    let code = editor.code_ref();
    let row = usize::try_from(line.checked_sub(1)?).ok()?;
    if row >= code.len_lines() {
        return None;
    }
    let col = usize::try_from(column.saturating_sub(1)).ok()?;
    Some(code.offset(row, col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;
    use phosphor_core::request::Position;

    fn editor(text: &str) -> Editor {
        Editor::new("text", text, Vec::new()).expect("an editor")
    }

    fn span(from: u32, to: u32) -> Span {
        Span {
            start: Position {
                line: from,
                column: 1,
            },
            end: Position {
                line: to,
                column: 1,
            },
        }
    }

    fn theme() -> Theme {
        builtin("phosphor-dark").expect("the shipped theme")
    }

    #[test]
    fn an_unseen_region_uploads_a_tint() {
        let mut held = editor("one\ntwo\nthree\n");
        let mut tints = Tints::new();

        assert!(tints.sync(&mut held, &theme(), &[(span(1, 3), RegionState::Unseen)]));
        assert_eq!(tints.len(), 1);
    }

    /// The diff is the whole reason this type exists: `set_marks` replaces
    /// wholesale, so a caller that may run every frame must be able to do
    /// nothing.
    #[test]
    fn syncing_the_same_regions_twice_uploads_once() {
        let mut held = editor("one\ntwo\nthree\n");
        let mut tints = Tints::new();
        let regions = [(span(1, 3), RegionState::Unseen)];

        assert!(
            tints.sync(&mut held, &theme(), &regions),
            "the first upload"
        );
        assert!(
            !tints.sync(&mut held, &theme(), &regions),
            "and nothing on a frame with no news"
        );
    }

    /// `s` — the visible behaviour of marking something seen is that the ground
    /// goes back to the editor's.
    #[test]
    fn marking_seen_removes_the_tint() {
        let mut held = editor("one\ntwo\nthree\n");
        let mut tints = Tints::new();
        tints.sync(&mut held, &theme(), &[(span(1, 3), RegionState::Unseen)]);

        assert!(tints.sync(&mut held, &theme(), &[(span(1, 3), RegionState::Seen)]));
        assert!(tints.is_empty(), "seen has no tint — §3's row 18");
    }

    /// The store answers in id order and a set is a set. Two orderings of the
    /// same regions must not look like news, or every declaration would
    /// re-upload.
    #[test]
    fn the_order_regions_arrive_in_is_not_news() {
        let mut held = editor("one\ntwo\nthree\nfour\n");
        let mut tints = Tints::new();
        let a = (span(1, 2), RegionState::Unseen);
        let b = (span(3, 4), RegionState::Unseen);

        assert!(tints.sync(&mut held, &theme(), &[a, b]));
        assert!(
            !tints.sync(&mut held, &theme(), &[b, a]),
            "the same two regions, the other way round"
        );
    }

    /// `T087`'s second acceptance. Nothing is remembered about offsets, so an
    /// edit that shifts every one of them produces a different desired set and
    /// exactly one upload.
    #[test]
    fn the_table_survives_an_edit_that_shifts_every_offset() {
        let mut held = editor("one\ntwo\nthree\n");
        let mut tints = Tints::new();
        let regions = [(span(2, 3), RegionState::Unseen)];
        tints.sync(&mut held, &theme(), &regions);
        let before = tints.clone();

        // A longer first line moves every offset after it.
        let mut widened = editor("one-much-longer\ntwo\nthree\n");
        assert!(
            tints.sync(&mut widened, &theme(), &regions),
            "the offsets moved, so the marks did"
        );
        assert_ne!(tints, before, "and the table now holds the new ones");
    }

    #[test]
    fn a_region_past_the_end_is_dropped_rather_than_drawn_on_nothing() {
        let mut held = editor("one\n");
        let mut tints = Tints::new();

        assert!(!tints.sync(&mut held, &theme(), &[(span(40, 44), RegionState::Unseen)]));
        assert!(tints.is_empty());
    }

    #[test]
    fn a_zero_width_region_is_not_a_mark() {
        let mut held = editor("one\ntwo\n");
        let mut tints = Tints::new();

        assert!(!tints.sync(&mut held, &theme(), &[(span(1, 1), RegionState::Unseen)]));
    }

    #[test]
    fn every_region_state_resolves_to_a_tint_or_deliberately_to_none() {
        let theme = theme();
        for state in [
            RegionState::Unseen,
            RegionState::Thread,
            RegionState::Watch,
            RegionState::Diagnostic,
            RegionState::Warning,
            RegionState::Failure,
            RegionState::NeedsYou,
        ] {
            assert!(tint(&theme, state).is_some(), "{state:?} has a tint");
        }
        assert!(tint(&theme, RegionState::Seen).is_none());
    }
}
