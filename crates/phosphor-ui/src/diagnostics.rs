//! Diagnostics on the screen (`T040`) — the bar, the `┊ ■` row, the undercurl.
//!
//! **Nothing here is a new widget.** The state bar is [`crate::gutter`], the row
//! is [`crate::virtual_text`], the undercurl is the vendored fork's `T085`, and
//! this module is the one place that turns *what a server said* into inputs for
//! those three. A second gutter would be the defect this module exists to avoid.
//!
//! # Trouble priority is not decided here
//!
//! §3 fixes the ladder — *"priority: trouble > attention > claude"* — and
//! [`crate::gutter::RegionState::mark`] is the only function that walks it. This module
//! answers a **different** question: *which region state is a diagnostic of this
//! grade*, which is `state`, three arms wide. Composing the two the other way
//! round — a severity that picks its own colour at the drawing — is how a
//! codebase ends up with two ladders that disagree about a row covered by both a
//! warning and an unseen edit.
//!
//! So [`DiagnosticsVm::regions`] hands back [`RegionSpan`]s and stops. The host
//! concatenates them with every *other* source of regions — unseen edits,
//! threads, failures — and calls [`crate::gutter::state_column`] **once**, which is
//! what makes "correct gutter priority against other states" a property of the
//! composition rather than of this file.
//!
//! # Where 6c and §3 disagree, and which one this follows
//!
//! One sentence of §3's prose decides this, not two sources: *"priority:
//! trouble > attention > claude"*. §3's own render draws row 19, *"diagnostic
//! region"*, with the trouble bar — but that row carries no unseen state, so it
//! is silent on the case that matters. Mockup `6c` draws line 64 — the line
//! carrying `■ E0308` — with the **claude** bar, because it sits inside an
//! unseen region claude just wrote. The prose and that drawing cannot both be
//! right on an overlapping row.
//!
//! This follows the prose: the ladder is stated mechanically, `T031` already
//! implements it, and the alternative reading (an inline `■` releases the bar)
//! is not written down anywhere. Flagged rather than folded in — see the `T040`
//! report.
//!
//! # A position the buffer does not have is never drawn on one it does
//!
//! A server's coordinates are against the text *it* last saw, and one keystroke
//! between the publish and the frame is enough to put them past the end. All
//! three outputs follow one rule: **the start decides whether a diagnostic is
//! drawn at all, and the end is clamped to the buffer.** A [`Span`] is
//! `[start, end)`, so the part of a stale one the buffer still has is
//! `[start, buffer end)` — and a start the buffer does not have has no part at
//! all.
//!
//! [`DiagnosticsVm::regions`] and [`DiagnosticsVm::underlines`] drop such a
//! diagnostic; [`DiagnosticsVm::rows`] hands the fork an anchor it cannot
//! place, which [`virtual_text::install`] drops by the same rule. It was two
//! rules until review: the char lookup clamped a stale *line* onto the buffer's
//! last, so a publish about a line that no longer existed drew a trouble-red
//! undercurl under whatever text happened to be down there — and the test that
//! claimed to cover it could not fail, because its fixture ended in a newline
//! and every clamp landed on an empty last line.
//!
//! # What is not built here, and why
//!
//! * `6c` draws `■ E0308: …` **inline, after the code on line 64**. A
//!   [`virtual_text::Row`] is a row of its own: the fork inserts a
//!   `VisualRow::Virtual` *under* the row showing its anchor (`VENDOR.md` patch
//!   8), and end-of-line virtual text is a different patch nobody has written.
//!   `T040`'s own wording — *"`■` rows via `VirtualText`"* — is what this builds.
//! * The `E0308` prefix is the server's diagnostic **code**, and
//!   [`Diagnostic`] has no field for one, so
//!   the row carries the message alone.
//! * `6c`'s summary row (*"1 diagnostic · claude sees what LSP sees"*) routes
//!   `:c fix`, which is `S6`.
//!
//! Owned by `surface`.

use phosphor_core::request::{Diagnostic, Position, Severity, Span};
use phosphor_core::vm::ViewModel;
use ratatui_code_editor::phosphor::cell_style::StyledSpan;
use ratatui_core::style::{Color, Style};

use crate::buffer_view::Editor;
use crate::gutter::{RegionSpan, RegionState};
use crate::theme::Theme;
use crate::virtual_text::{self, Anchor, Run};

/// §2's lexicon: *"✕ ■ session lost · diagnostic"*. The glyph every diagnostic
/// row opens with, and the one `6c`'s statusline counts.
pub const GLYPH: &str = "■";

/// Which region state a diagnostic of this grade puts on the rows it covers.
///
/// The **grade → state** half; [`crate::gutter::RegionState::mark`] is the
/// **state → tier** half, and it stays the only one. Three arms, because §1 has
/// exactly three roles to draw a diagnostic in and `phosphor-buffer`'s
/// `severity_from_lsp` already collapses LSP's four levels onto them.
///
/// `None` for [`Severity::Info`], and that is a decision rather than an
/// omission: §1 renders info in meta-grey, §3's bar has no meta tier — it is
/// *"unseen/diagnostic/none"* — and a hint that painted the same amber as a
/// warning would make the column say something the palette does not mean. An
/// info diagnostic still gets its row and its underline; it just does not claim
/// the one cell the bar has.
///
/// The [`Severity::Attention`] arm is a **reading**, not a transcription: §3
/// enumerates the bar as three-valued and no mockup draws an amber one. See
/// [`RegionState::Warning`], where the argument for it lives.
#[must_use]
const fn state(severity: Severity) -> Option<RegionState> {
    match severity {
        Severity::Trouble => Some(RegionState::Diagnostic),
        Severity::Attention => Some(RegionState::Warning),
        Severity::Info => None,
    }
}

/// §1's colour for a grade — trouble-red, attention-amber, meta-grey.
///
/// Read from the theme per call, like every other hue in this crate: a value
/// spelled here would be a `T006` violation even if it matched.
fn hue(severity: Severity, theme: &Theme) -> Color {
    match severity {
        Severity::Trouble => theme.actors.trouble,
        Severity::Attention => theme.actors.attention,
        Severity::Info => theme.neutrals.meta,
    }
}

/// One file's diagnostics, as the surface reads them.
///
/// A ViewModel in the [`phosphor_core::vm`] sense — a borrowed, read-only
/// projection with no path back to what produced it. The host builds one per
/// frame from `phosphor_core::store`'s answer for the file the pane is showing;
/// this crate cannot name that module (`T007`) and does not need to, because a
/// [`Diagnostic`] is vocabulary and vocabulary is shared.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticsVm<'a> {
    /// The file's diagnostics, in the order they are drawn — span order, fixed
    /// by the store on ingest so two identical publishes produce the same rows.
    pub diagnostics: &'a [Diagnostic],
}

impl ViewModel for DiagnosticsVm<'_> {}

impl<'a> DiagnosticsVm<'a> {
    /// A projection over one file's set.
    #[must_use]
    pub const fn new(diagnostics: &'a [Diagnostic]) -> Self {
        Self { diagnostics }
    }

    /// The rows each diagnostic covers, as regions for [`crate::gutter::state_column`].
    ///
    /// **Visual rows**, resolved through the same rule `T032` fixed for a
    /// virtual row's anchor: a span starts on the segment showing its start
    /// column and ends on the segment showing its last character, so a
    /// diagnostic on a soft-wrapped line marks the segments it is actually on
    /// rather than the whole line.
    ///
    /// **Never clamped onto a row that is not it.** A start the buffer no
    /// longer has contributes nothing at all, and so does either end inside a
    /// collapsed fold — [`virtual_text::install`]'s rule for a row that cannot
    /// be placed, and a bar drawn on the wrong line is worse than a bar missing
    /// from a line you cannot see. A start the buffer *does* have keeps its
    /// rows even when the end runs off a file that has since been shortened:
    /// the end is clamped, which only ever makes a span cover less. See the
    /// module header. *An error hidden inside a fold is invisible today; see
    /// the `T040` report.*
    ///
    /// Spans are split around `┊` rows: a virtual row is not a line
    /// ([`virtual_text::is_virtual_row`]), so a region covering one would be
    /// claiming to cover more of the buffer than it does.
    #[must_use]
    pub fn regions(&self, editor: &Editor) -> Vec<RegionSpan> {
        let mut regions = Vec::new();
        for diagnostic in self.diagnostics {
            let Some(state) = state(diagnostic.severity) else {
                continue;
            };
            let Some(first) = cell(editor, diagnostic.span.start)
                .and_then(|(line, column)| editor.visual_row_for_position(line, column))
            else {
                continue;
            };
            let (line, column) = end_cell(editor, last_inside(diagnostic.span));
            let Some(last) = editor.visual_row_for_position(line, column) else {
                continue;
            };
            // An inverted span — the two ends crossed during a rewrite — marks
            // its start and nothing else, rather than nothing at all or a range
            // running backwards.
            let last = last.max(first);
            let mut run: Option<core::ops::Range<usize>> = None;
            for row in first..=last {
                if virtual_text::is_virtual_row(editor, row) {
                    regions.extend(run.take().map(|rows| RegionSpan::new(rows, state)));
                } else if let Some(rows) = run.as_mut() {
                    rows.end = row + 1;
                } else {
                    run = Some(row..row + 1);
                }
            }
            regions.extend(run.map(|rows| RegionSpan::new(rows, state)));
        }
        regions
    }

    /// One `┊ ■ message` row per diagnostic, hung from where it applies.
    ///
    /// Unowned ([`virtual_text::Row::owner`] is `None`): a region id is the
    /// store's and there are no regions until `T041`, at which point a
    /// diagnostic's row is owned by the region anchored to its node — which is
    /// what makes `6c`'s *"the thread followed its tree-sitter node"* true of a
    /// diagnostic too.
    ///
    /// The message is flattened to one line. A server is free to send newlines —
    /// rust-analyzer does, on a type mismatch with notes — and a row is one
    /// row: the runs go into the fork's row stream verbatim, so a `\n` in one
    /// would be a glyph in the middle of the buffer rather than a second row.
    /// §11 is *"nothing ever wraps"*, and the drawing clips what does not fit.
    ///
    /// **The whole row takes the grade's hue**, glyph and message together.
    /// §3's default for virtual text is *"meta-gray with colored spans"*, and
    /// `6c`'s own inline draw of this exact diagnostic — `■ E0308: expected
    /// Duration, found u128` — is one colour end to end. The drawing wins where
    /// it is specific, which is the same rule the rest of this file follows.
    #[must_use]
    pub fn rows(&self, theme: &Theme) -> Vec<virtual_text::Row> {
        self.diagnostics
            .iter()
            .map(|diagnostic| {
                let text = format!("{GLYPH} {}", one_line(&diagnostic.message));
                let style = Style::new().fg(hue(diagnostic.severity, theme));
                virtual_text::Row::new(anchor_of(diagnostic.span), vec![Run::new(text, style)])
            })
            .collect()
    }

    /// An undercurl under every diagnostic's span, in its grade's colour.
    ///
    /// `T085` owns both halves of this: the caller asks for
    /// [`StyledSpan::undercurl`] and never learns which terminal it is on —
    /// SGR `4:3` where the terminal has it, a straight underline where it does
    /// not (§8). There is no second code path here to keep in step.
    ///
    /// **A zero-width span is widened to one character.** *"expected `;`"*
    /// arrives as an empty range at the position the character is missing from,
    /// and an underline under nothing is nothing — the row would be the only
    /// sign, on a line the eye has no reason to go to. Widened *where there is
    /// a character to widen onto*: a position at the very end of the buffer has
    /// none, and that span draws nothing rather than reaching past the last
    /// character.
    ///
    /// In span order, and **not one per diagnostic**: a diagnostic whose start
    /// the buffer does not have contributes none, which is the module header's
    /// rule and the same one [`Self::regions`] follows. A caller that needs to
    /// know which span is whose has to carry that itself rather than index the
    /// two lists together.
    #[must_use]
    pub fn underlines(&self, editor: &Editor, theme: &Theme) -> Vec<StyledSpan> {
        let chars = editor.code_ref().len_chars();
        self.diagnostics
            .iter()
            .filter_map(|diagnostic| {
                let start = char_of(editor, cell(editor, diagnostic.span.start)?);
                let end = char_of(editor, end_cell(editor, diagnostic.span.end))
                    .max(start.saturating_add(1))
                    .min(chars.max(start));
                Some(StyledSpan::undercurl(
                    start,
                    end,
                    hue(diagnostic.severity, theme),
                ))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Positions — the vocabulary's coordinates, in the buffer's
// ---------------------------------------------------------------------------

/// A [`Position`] as the fork counts: 0-based line, 0-based char column.
///
/// The vocabulary is 1-based in both (`request::Position`) and the buffer is
/// 0-based in both, and this is the only place the two meet. A `0` from a
/// producer that counted from zero saturates to the first line rather than
/// wrapping to the last one, which is what `saturating_sub` is doing here.
const fn zero_based(at: Position) -> (usize, usize) {
    (
        at.line.saturating_sub(1) as usize,
        at.column.saturating_sub(1) as usize,
    )
}

/// The last position *inside* a half-open span.
///
/// [`Span`] ends at the first position after it, so a span covering all of line
/// 12 ends at line 13 column 1 — and asking which row *that* is on marks a line
/// the diagnostic does not touch. Column 1 of a line means "the end of the line
/// before"; anything else steps back one column.
fn last_inside(span: Span) -> Position {
    if span.end.column > 1 {
        Position {
            line: span.end.line,
            column: span.end.column - 1,
        }
    } else {
        Position {
            line: span.end.line.saturating_sub(1).max(span.start.line),
            // Past any line's end, which
            // [`Editor::visual_row_for_position`] resolves to its last segment.
            column: u32::MAX,
        }
    }
}

/// Where a diagnostic's row hangs: its span's start.
const fn anchor_of(span: Span) -> Anchor {
    let (line, column) = zero_based(span.start);
    Anchor::at(line, column)
}

/// `at` as a buffer cell, or `None` when the buffer has no such line.
///
/// **The start half of the module header's rule.** Never clamped: a start the
/// buffer does not have is a diagnostic about text that is gone, and the last
/// line of the buffer is not where it belongs.
fn cell(editor: &Editor, at: Position) -> Option<(usize, usize)> {
    let (line, column) = zero_based(at);
    (line < editor.code_ref().len_lines()).then_some((line, column))
}

/// The same for a span's **end**, which is clamped rather than dropped.
///
/// A span is `[start, end)`, so the part of a stale one a shortened buffer
/// still has ends where the buffer does. Clamping only ever shortens a span,
/// which is why it is safe here and not on a start. The sentinel column is what
/// both consumers read as *"the end of that line"* — [`char_of`] takes the
/// line's own length and `Editor::visual_row_for_position` its last segment.
fn end_cell(editor: &Editor, at: Position) -> (usize, usize) {
    let last = editor.code_ref().len_lines().saturating_sub(1);
    let (line, column) = zero_based(at);
    if line > last {
        (last, usize::MAX)
    } else {
        (line, column)
    }
}

/// A buffer cell as a character offset into the document — the coordinate
/// [`StyledSpan`] speaks.
///
/// The column is clamped to the line's own length excluding its newline, so a
/// position past the end of a line that still exists lands on that line's end
/// rather than inside the next one. The **line** is not clamped here and must
/// not be: both callers hand it one the buffer has — [`cell`] checked, or
/// [`end_cell`] clamped — because `line_to_char` panics past the end, and a
/// clamp hidden down here is what drew a stale diagnostic on the last line.
fn char_of(editor: &Editor, (line, column): (usize, usize)) -> usize {
    let code = editor.code_ref();
    code.line_to_char(line) + column.min(code.line_len(line))
}

/// A server's message as one row's worth of text: every run of whitespace
/// becomes a single space.
fn one_line(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use phosphor_core::request::Severity;
    use ratatui_code_editor::phosphor::cell_style::{Underline, UnderlineCapability};
    use ratatui_core::layout::Rect;

    use super::*;
    use crate::buffer_view::StateMark;
    use crate::gutter;
    use crate::{buffer_view, soft_wrap};

    /// `6c`'s file, around the lines it draws: `next_delay` with the type
    /// mismatch on the `jitter` call.
    const RETRY_RS: &str = "\
impl RetryPolicy {
    pub fn next_delay(&self, attempt: u32) -> Duration {
        let exp = self.base_delay * 2u32.pow(attempt);
        jitter(exp.min(self.max_delay))
    }
}
";

    /// The `jitter(…)` line, 1-based — `6c`'s line 64.
    const TROUBLE_LINE: u32 = 4;

    /// One line long enough to wrap several times at the width the boundary
    /// test renders — the only geometry in which a span's half-open end and an
    /// inclusive one differ.
    const LONG_LINE: &str = "\
let message = format!(\"retry {} of {} after {:?}, last error was {}\", attempt, max, delay, err);
let next = delay * 2;
";

    fn theme() -> Theme {
        Theme::phosphor_dark()
    }

    fn configured(theme: &Theme, source: &str) -> Editor {
        let mut editor = Editor::new("rust", source, Vec::new()).expect("rust editor");
        buffer_view::configure(&mut editor, theme);
        soft_wrap::configure(&mut editor, theme);
        virtual_text::configure(&mut editor, theme);
        editor
    }

    fn editor(theme: &Theme) -> Editor {
        configured(theme, RETRY_RS)
    }

    /// The same file with its trailing newline gone, so the last line carries
    /// text instead of being empty.
    ///
    /// **The stale-publish tests are vacuous without this geometry.** With a
    /// trailing newline the rope's last line is empty, so anything clamped onto
    /// it lands on `len_chars()` and a span collapses to zero width whatever
    /// the code did with the line number — which is exactly how the first
    /// version of those tests passed against a clamp that drew a stale
    /// diagnostic under real text.
    fn unterminated(theme: &Theme) -> Editor {
        configured(theme, RETRY_RS.trim_end_matches('\n'))
    }

    fn span(line: u32, columns: core::ops::Range<u32>) -> Span {
        Span {
            start: Position {
                line,
                column: columns.start,
            },
            end: Position {
                line,
                column: columns.end,
            },
        }
    }

    fn diagnostic(span: Span, severity: Severity, message: &str) -> Diagnostic {
        Diagnostic {
            span,
            severity,
            message: message.to_owned(),
            source: Some("rust-analyzer".to_owned()),
        }
    }

    /// `6c`'s own diagnostic: an error on the `jitter` call.
    fn e0308() -> Diagnostic {
        diagnostic(
            span(TROUBLE_LINE, 9..38),
            Severity::Trouble,
            "expected Duration, found u128",
        )
    }

    // -- the grade ladder ---------------------------------------------------

    #[test]
    fn each_grade_lands_on_the_state_the_language_gives_it() {
        assert_eq!(state(Severity::Trouble), Some(RegionState::Diagnostic));
        assert_eq!(state(Severity::Attention), Some(RegionState::Warning));
        assert_eq!(
            state(Severity::Info),
            None,
            "§3's bar is unseen/diagnostic/none and has no meta tier"
        );
    }

    #[test]
    fn a_grade_is_drawn_in_its_own_role_and_reads_the_theme() {
        // A second theme with unmistakably different values in those fields
        // proves the lookup reads the fields rather than agreeing with them by
        // coincidence — the same argument `gutter.rs` makes, and no colour
        // enters this file (`T006` would reject one).
        let mut recoloured = theme();
        recoloured.actors.trouble = recoloured.actors.claude;
        recoloured.actors.attention = recoloured.actors.steel;
        recoloured.neutrals.meta = recoloured.actors.you;
        assert_eq!(
            hue(Severity::Trouble, &recoloured),
            recoloured.actors.claude
        );
        assert_eq!(
            hue(Severity::Attention, &recoloured),
            recoloured.actors.steel
        );
        assert_eq!(hue(Severity::Info, &recoloured), recoloured.actors.you);
    }

    // -- the bar ------------------------------------------------------------

    /// The state column a set of diagnostics produces on their own.
    fn column(editor: &Editor, diagnostics: &[Diagnostic]) -> Vec<StateMark> {
        let vm = DiagnosticsVm::new(diagnostics);
        gutter::state_column(&vm.regions(editor), editor.visual_len_lines())
    }

    #[test]
    fn an_error_marks_its_own_row_and_no_other() {
        let theme = theme();
        let editor = editor(&theme);
        let marks = column(&editor, &[e0308()]);
        for (row, mark) in marks.iter().enumerate() {
            let expected = if row + 1 == TROUBLE_LINE as usize {
                StateMark::Trouble
            } else {
                StateMark::None
            };
            assert_eq!(*mark, expected, "row {row}");
        }
    }

    #[test]
    fn a_span_across_lines_marks_every_row_it_covers_and_stops_at_its_end() {
        let theme = theme();
        let editor = editor(&theme);
        // Lines 2-4 (1-based), ending at column 1 of line 5 — the half-open
        // form a server sends for "this whole block". Line 5 must stay clean.
        let across = diagnostic(
            Span {
                start: Position { line: 2, column: 5 },
                end: Position { line: 5, column: 1 },
            },
            Severity::Trouble,
            "mismatched types",
        );
        let marks = column(&editor, &[across]);
        assert_eq!(
            marks[..5],
            [
                StateMark::None,
                StateMark::Trouble,
                StateMark::Trouble,
                StateMark::Trouble,
                StateMark::None,
            ]
        );
    }

    #[test]
    fn a_warning_is_amber_and_a_hint_says_nothing_in_the_bar() {
        let theme = theme();
        let editor = editor(&theme);
        let warning = diagnostic(span(2, 5..12), Severity::Attention, "unused variable");
        let hint = diagnostic(span(3, 9..12), Severity::Info, "consider `let _`");
        let marks = column(&editor, &[warning, hint]);
        assert_eq!(marks[1], StateMark::Attention);
        assert_eq!(marks[2], StateMark::None, "meta has no tier in the bar");
    }

    #[test]
    fn an_error_outranks_the_unseen_marker_on_the_same_row() {
        // **`T040`'s acceptance, and `6c`'s overlap**: the diagnostic line is
        // one claude just wrote and has not been looked at. §3's ladder is
        // trouble > attention > claude, so the row goes red — and the rows
        // around it keep the claude hue.
        let theme = theme();
        let editor = editor(&theme);
        let set = [e0308()];
        let vm = DiagnosticsVm::new(&set);
        let mut regions = vec![RegionSpan::new(1..4, RegionState::Unseen)];
        regions.extend(vm.regions(&editor));
        let marks = gutter::state_column(&regions, editor.visual_len_lines());
        assert_eq!(
            marks[..4],
            [
                StateMark::None,
                StateMark::ClaudeUnseen,
                StateMark::ClaudeUnseen,
                StateMark::Trouble,
            ]
        );
    }

    #[test]
    fn every_other_region_state_meets_a_diagnostic_on_one_row() {
        // Each of the eight states, overlapped with the same error on the same
        // row. `gutter.rs` proves the ladder over its whole power set; this
        // proves that what this module contributes enters that ladder — the
        // seam, once per state.
        let theme = theme();
        let editor = editor(&theme);
        let row = TROUBLE_LINE as usize - 1;
        let set = [e0308()];
        let vm = DiagnosticsVm::new(&set);
        for state in RegionState::ALL {
            let mut regions = vec![RegionSpan::new(row..row + 1, state)];
            regions.extend(vm.regions(&editor));
            let marks = gutter::state_column(&regions, editor.visual_len_lines());
            assert_eq!(
                marks[row],
                StateMark::Trouble,
                "an error must survive {state:?} on the same row"
            );
        }
    }

    #[test]
    fn a_diagnostic_the_stream_does_not_show_marks_nothing() {
        // A publish against a file the buffer has since shortened. Not clamped
        // to the last line — a bar on the wrong line is worse than no bar.
        let theme = theme();
        let editor = editor(&theme);
        let set = [diagnostic(span(4_000, 1..8), Severity::Trouble, "gone")];
        assert!(DiagnosticsVm::new(&set).regions(&editor).is_empty());
        assert!(
            column(&editor, &set)
                .iter()
                .all(|mark| *mark == StateMark::None)
        );
    }

    /// A span the buffer has outrun at **one** end still says what it can about
    /// the other.
    ///
    /// A span is `[start, end)`, so the part of a stale one the buffer still
    /// has is `[start, buffer end)`. Line 3 is on screen and the diagnostic
    /// covers it, whatever the server thought line 4 000 was; dropping the
    /// whole span would leave an error on a visible line unmarked, which is
    /// what this did until review.
    #[test]
    fn a_span_running_off_the_end_of_the_buffer_marks_what_is_left_of_it() {
        let theme = theme();
        let editor = unterminated(&theme);
        let set = [diagnostic(
            Span {
                start: Position { line: 3, column: 9 },
                end: Position {
                    line: 4_000,
                    column: 1,
                },
            },
            Severity::Trouble,
            "unclosed delimiter",
        )];

        let marks = column(&editor, &set);
        assert_eq!(marks[..2], [StateMark::None, StateMark::None], "{marks:?}");
        assert!(
            marks[2..].iter().all(|mark| *mark == StateMark::Trouble),
            "from its start to the end of the buffer: {marks:?}"
        );

        // And the undercurl runs to the buffer's last character rather than
        // stopping one short of it on a clamped empty line.
        let chars = editor.code_ref().len_chars();
        let spans = DiagnosticsVm::new(&set).underlines(&editor, &theme);
        assert_eq!(spans[0].start, editor.code_ref().line_to_char(2) + 8);
        assert_eq!(spans[0].end, chars);
        assert!(spans[0].contains(chars - 1), "{:?}", spans[0]);
    }

    #[test]
    fn a_virtual_row_inside_a_span_carries_no_bar() {
        // §3's column is indexed by visual row and a `┊` row is not a line, so
        // a region covering one would claim more of the buffer than it covers.
        // The row splits the region in two rather than swallowing it.
        let theme = theme();
        let mut editor = editor(&theme);
        virtual_text::install(
            &mut editor,
            &[virtual_text::Row::new(
                Anchor::line(1),
                vec![Run::prose(
                    "⚓ thread · was retry_with_backoff:19-21",
                    &theme,
                )],
            )],
        );
        let across = diagnostic(
            Span {
                start: Position { line: 2, column: 5 },
                end: Position { line: 4, column: 9 },
            },
            Severity::Trouble,
            "mismatched types",
        );
        let set = [across];
        let vm = DiagnosticsVm::new(&set);
        let regions = vm.regions(&editor);
        assert_eq!(
            regions.len(),
            2,
            "the `┊` row splits the region: {regions:?}"
        );
        let marks = gutter::state_column(&regions, editor.visual_len_lines());
        // Rows: 0 `impl`, 1 `pub fn`, 2 the `┊` row, 3 `let exp`, 4 `jitter`.
        assert_eq!(
            marks[..5],
            [
                StateMark::None,
                StateMark::Trouble,
                StateMark::None,
                StateMark::Trouble,
                StateMark::Trouble,
            ]
        );
    }

    #[test]
    fn a_span_ending_on_a_wrap_boundary_stops_at_the_segment_it_covers() {
        // **The half-open end, on the one geometry that can see it.** A span
        // ending at column `n` covers up to `n - 1`, and on a soft-wrapped line
        // column `n` is the *first* column of the next segment (`T032`: "a
        // column sitting exactly on a segment boundary belongs to the later
        // row"). Reading the end as inclusive marks a second row for a
        // diagnostic that never reaches it — invisible on an unwrapped file,
        // which is why this test wraps one.
        let theme = theme();
        let mut editor = Editor::new("rust", LONG_LINE, Vec::new()).expect("rust editor");
        buffer_view::configure(&mut editor, &theme);
        soft_wrap::configure(&mut editor, &theme);
        soft_wrap::wrap_to(&mut editor, Rect::new(0, 0, 40, 10));
        let first = editor.row_span(0).expect("the first segment");
        assert!(first.wrapped, "the fixture must wrap: {first:?}");
        assert!(editor.visual_len_lines() > 2, "and onto more than one row");

        // Exactly the first segment: 1-based, half-open, so the end is one past
        // its last column — which is the next segment's first.
        let boundary = diagnostic(
            span(1, 1..u32::try_from(first.end_col).expect("a column") + 1),
            Severity::Trouble,
            "mismatched types",
        );
        let set = [boundary];
        let marks = column(&editor, &set);
        assert_eq!(marks[0], StateMark::Trouble, "the segment it covers");
        assert_eq!(
            marks[1],
            StateMark::None,
            "and not the one that starts where it ends"
        );
    }

    /// **The `u32::MAX` sentinel, on the one geometry where it decides
    /// anything.**
    ///
    /// A span ending at column 1 of the next line ends at *"past the end of the
    /// line before"* (`last_inside`), and on a soft-wrapped line that has to
    /// resolve to the line's **last** segment. The fork gets there by falling
    /// through its segment walk rather than by a rule of its own — `View::
    /// visual_row_for_position` returns its running `last` when no segment's
    /// `end_col` is past the column — so a fork change that clamped an
    /// over-large column to the first segment would quietly mark one row of a
    /// line the diagnostic covers three rows of, and every unwrapped test in
    /// this file would stay green.
    #[test]
    fn a_span_ending_at_the_next_line_covers_every_segment_of_a_wrapped_one() {
        let theme = theme();
        let mut editor = configured(&theme, LONG_LINE);
        soft_wrap::wrap_to(&mut editor, Rect::new(0, 0, 40, 10));
        let segments: Vec<usize> = (0..editor.visual_len_lines())
            .filter(|row| editor.row_span(*row).is_some_and(|span| span.line_idx == 0))
            .collect();
        assert!(
            segments.len() >= 3,
            "the fixture must wrap three ways: {segments:?}"
        );

        // The whole of line 1, half-open: a server sends "this line" as ending
        // at column 1 of the line after it.
        let whole = diagnostic(
            Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 2, column: 1 },
            },
            Severity::Trouble,
            "mismatched types",
        );
        let marks = column(&editor, &[whole]);
        for row in &segments {
            assert_eq!(marks[*row], StateMark::Trouble, "segment at row {row}");
        }
        assert_eq!(
            marks[segments.last().expect("segments") + 1],
            StateMark::None,
            "and stops before line 2"
        );
    }

    // -- the row ------------------------------------------------------------

    #[test]
    fn the_row_is_the_glyph_and_the_message_in_the_grades_colour() {
        let theme = theme();
        let set = [e0308()];
        let rows = DiagnosticsVm::new(&set).rows(&theme);
        let [row] = rows.as_slice() else {
            panic!("one diagnostic, one row: {rows:?}");
        };
        assert_eq!(row.owner, None, "a region id is `T041`'s to give");
        assert_eq!(row.anchor, Anchor::at(TROUBLE_LINE as usize - 1, 8));
        let [run] = row.runs.as_slice() else {
            panic!("{:?}", row.runs);
        };
        assert_eq!(run.text, "■ expected Duration, found u128");
        assert_eq!(run.style.fg, Some(theme.actors.trouble));
    }

    #[test]
    fn a_message_with_newlines_in_it_is_still_one_row() {
        // rust-analyzer sends these on a type mismatch with notes. A `\n` in a
        // run is a glyph in the row stream, not a second row.
        let theme = theme();
        let wrapped = diagnostic(
            span(TROUBLE_LINE, 9..38),
            Severity::Trouble,
            "expected Duration,\n   found u128\nnote: in this expansion",
        );
        let set = [wrapped];
        let rows = DiagnosticsVm::new(&set).rows(&theme);
        let text = &rows[0].runs[0].text;
        assert_eq!(
            text,
            "■ expected Duration, found u128 note: in this expansion"
        );
        assert!(!text.contains('\n'));
    }

    #[test]
    fn the_rows_hang_under_the_lines_they_belong_to() {
        // The half `T032` owns, exercised end to end: installed rows land in
        // the fork's stream under their own anchors, and the buffer's line
        // numbering is untouched.
        let theme = theme();
        let mut editor = editor(&theme);
        let lines = editor.code_ref().len_lines();
        let set = [e0308()];
        let rows = DiagnosticsVm::new(&set).rows(&theme);
        virtual_text::install(&mut editor, &rows);
        assert_eq!(editor.code_ref().len_lines(), lines, "no line was added");
        assert!(virtual_text::is_virtual_row(&editor, TROUBLE_LINE as usize));
    }

    // -- the undercurl ------------------------------------------------------

    #[test]
    fn the_undercurl_covers_the_span_in_the_grades_colour() {
        let theme = theme();
        let editor = editor(&theme);
        let set = [e0308()];
        let spans = DiagnosticsVm::new(&set).underlines(&editor, &theme);
        let [span] = spans.as_slice() else {
            panic!("{spans:?}");
        };
        let line_start = editor.code_ref().line_to_char(TROUBLE_LINE as usize - 1);
        assert_eq!(span.start, line_start + 8);
        assert_eq!(span.end, line_start + 37);
        assert_eq!(span.style.underline, Underline::Curl);
        assert_eq!(span.style.color, Some(theme.actors.trouble));
    }

    #[test]
    fn a_zero_width_diagnostic_is_widened_to_one_character() {
        // "expected `;`" arrives as an empty range at the position the
        // character is missing from. An underline under nothing is nothing.
        let theme = theme();
        let editor = editor(&theme);
        let set = [diagnostic(span(3, 9..9), Severity::Trouble, "expected `;`")];
        let spans = DiagnosticsVm::new(&set).underlines(&editor, &theme);
        assert_eq!(spans[0].end, spans[0].start + 1);
    }

    /// **A publish that raced a keystroke is dropped by all three surfaces.**
    ///
    /// The fixture's last line carries text, and that is the whole of why this
    /// test can fail: [`unterminated`] says what the terminated one hides. The
    /// version this replaces asserted `start <= chars`, `end <= chars` and
    /// `!contains(chars - 1)` against `RETRY_RS` — three claims that are true
    /// of a wrong answer as well as a right one, because the clamped position
    /// landed on an empty last line and collapsed the span to zero width.
    #[test]
    fn a_position_past_the_end_of_the_buffer_is_dropped_by_every_surface() {
        let theme = theme();
        let mut editor = unterminated(&theme);
        let code = editor.code_ref();
        assert!(
            code.line_len(code.len_lines() - 1) > 0,
            "the fixture's last line must carry text or this test cannot fail"
        );

        let set = [diagnostic(span(4_000, 1..6), Severity::Trouble, "stale")];
        let vm = DiagnosticsVm::new(&set);
        assert!(vm.regions(&editor).is_empty(), "no bar");
        assert!(
            vm.underlines(&editor, &theme).is_empty(),
            "and no undercurl on whatever text is at the end of the buffer"
        );

        // The third surface: `rows` hands the fork an anchor it cannot place,
        // and `install` drops it by the same rule rather than hanging the row
        // off the last line.
        let before = editor.visual_len_lines();
        virtual_text::install(&mut editor, &vm.rows(&theme));
        assert_eq!(editor.visual_len_lines(), before, "and no `┊ ■` row");
    }

    /// **Both halves of `T085`'s degradation path, on the same frame.**
    ///
    /// The primary terminal gets the SGR `4:3` pair wrapped around the glyph;
    /// the degradation terminal gets the same cell with nothing added, and the
    /// straight underline `patch_style` always sets is the whole treatment.
    /// Neither is a second code path in this crate — the call is
    /// [`StyledSpan::undercurl`] in both, which is the property being checked.
    #[test]
    fn the_undercurl_degrades_to_an_underline_without_a_second_path() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;
        use ratatui_core::style::Modifier;
        use ratatui_core::widgets::Widget;

        use crate::buffer_view::{BufferView, gutter_width};

        let theme = theme();
        let area = Rect::new(0, 0, 60, 8);
        let set = [e0308()];
        // Two editors over the same text rather than one cloned: `Editor` is
        // not `Clone`, and the point is that the *same* span list reaches both.
        let mut curly = editor(&theme);
        let mut flat = editor(&theme);
        let spans = DiagnosticsVm::new(&set).underlines(&curly, &theme);
        curly.set_styled_spans(spans.clone());
        flat.set_styled_spans(spans);
        curly.set_underline_capability(Some(UnderlineCapability::Undercurl));
        flat.set_underline_capability(Some(UnderlineCapability::Underline));

        let cell = |editor: &Editor| {
            let mut buf = Buffer::empty(area);
            BufferView::new(editor, &theme).render(area, &mut buf);
            let x = gutter_width(editor) + 8;
            buf[(x, TROUBLE_LINE as u16 - 1)].clone()
        };

        let curled = cell(&curly);
        let straight = cell(&flat);
        assert!(
            curled.symbol().contains("\u{1b}[4:3m"),
            "the primary terminal gets the curl: {:?}",
            curled.symbol()
        );
        assert_eq!(
            straight.symbol(),
            "j",
            "the degradation terminal gets the glyph and nothing else"
        );
        for cell in [&curled, &straight] {
            assert!(
                cell.modifier.contains(Modifier::UNDERLINED),
                "both terminals underline: {cell:?}"
            );
        }
    }
}
