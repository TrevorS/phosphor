//! `T040` — screen `6c`, *"anchors survive the rewrite"*, as a golden frame.
//!
//! `CP-4` accepts on *"diagnostic gutter priority vs other region states"*, and
//! this is that picture: one row carrying a diagnostic **and** an unseen claude
//! edit, two rows above it carrying the unseen edit alone, a thread's `┊` rows
//! between them carrying nothing, and an anchored region's tint and undercurl
//! over the line the thread hangs from. Every state §3 draws in the bar, and
//! both of the overlays it does not, on one screen.
//!
//! Same serialiser and the same review loop as `tests/golden_frames.rs`
//! (`cargo insta review`, or `just review`); a separate file because that one is
//! `T018`'s and carries `CP-1`'s four frames.
//!
//! **What this frame is composed of is the point.** The diagnostic's three
//! contributions — region spans, `┊` rows, undercurl spans — are merged with a
//! thread's before they reach the editor, because `virtual_text::install` and
//! `Editor::set_styled_spans` each replace the whole list. A host that installed
//! diagnostics *after* threads would silently delete the threads, and the merge
//! below is the shape that does not.

mod frame_grid;

use frame_grid::Frame;
use phosphor_core::request::{Diagnostic, Position, Severity, Span};
use phosphor_ui::buffer_view::{
    self, BufferView, Editor, ScrollRequest, apply_scroll, gutter_width,
};
use phosphor_ui::diagnostics::{DiagnosticsVm, RowPolicy};
use phosphor_ui::gutter::{self, RegionSpan, RegionState};
use phosphor_ui::status_line::{CursorVm, FileVm, Mode, SessionState, StatusLine, StatusLineVm};
use phosphor_ui::theme::Theme;
use phosphor_ui::virtual_text::{self, Anchor, Run};
use ratatui_code_editor::phosphor::cell_style::{StyledSpan, UnderlineCapability};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

/// `6c`'s file. The mockup draws a viewport at lines 61–66, so the fixture is a
/// plausible `src/retry.rs` whose `impl RetryPolicy` block lands exactly there
/// — the same judgement `golden_frames.rs` records for its trailing `}` and
/// `screen_7c.rs` for its preamble: a mockup is a viewport onto a file, and a
/// fixture that starts where the picture starts puts the wrong numbers in the
/// gutter. [`the_fixture_puts_the_impl_where_6c_draws_it`] holds it in place.
const RETRY_RS: &str = "\
use std::thread;
use std::time::Duration;

use crate::util::jitter;

/// How often to retry, and how long to wait between attempts.
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(1),
        }
    }
}

/// Runs `op` until it succeeds or the policy runs out of attempts.
pub fn retry_with_backoff<T, E>(
    mut op: impl FnMut() -> Result<T, E>,
    policy: &RetryPolicy,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    let mut last = None;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt + 1 == policy.max_attempts => return Err(e),
            Err(e) => {
                last = Some(e);
                thread::sleep(jitter(delay));
            }
        }
        delay = policy.next_delay(attempt);
    }
    match last {
        Some(e) => Err(e),
        None => unreachable!(\"the loop returns on the last attempt\"),
    }
}

/// The same walk, without the sleep — used by the tests.
pub fn attempts(policy: &RetryPolicy) -> Vec<Duration> {
    let mut out = Vec::new();
    let mut delay = policy.base_delay;
    for _ in 0..policy.max_attempts {
        out.push(delay);
        delay = (delay * 2).min(policy.max_delay);
    }
    out
}

// claude restructured the file: the delay arithmetic moved into a method of
// its own, and the thread anchored to the old arms followed the node.

impl RetryPolicy {
    pub fn next_delay(&self, attempt: u32) -> Duration {
        let exp = self.base_delay * 2u32.pow(attempt);
        jitter(exp.min(self.max_delay))
    }
}
";

/// `impl RetryPolicy {`, 1-based — `6c`'s line 61.
const IMPL_LINE: usize = 61;

/// `pub fn next_delay(…)`, the line the thread is anchored to.
const ANCHOR_LINE: usize = 62;

/// `jitter(exp.min(self.max_delay))`, the line carrying `6c`'s `E0308`.
const TROUBLE_LINE: usize = 64;

/// The three lines claude wrote and you have not looked at: the signature it
/// hoisted and the two lines of body it moved.
const UNSEEN_LINES: [usize; 3] = [62, 63, 64];

/// The row policy this frame draws under — **the shipped default**, with the
/// cursor where `6c` puts it.
///
/// **That it is the default is the finding, not a convenience.** `RowPolicy`
/// was added because `CP-4` reported eleven cascade rows burying the code, and
/// its default draws the cursor's line only. This screen has its cursor on
/// line 64 (`CursorVm { line: 64, .. }` below) and that is exactly the line
/// carrying `E0308` — so the mockup the design drew and the default the build
/// now ships are the same picture, and this frame is what proves it. A policy
/// spelled `Everywhere` here would have made this file pass while saying
/// nothing about what a person actually sees.
///
/// `- 1` because [`TROUBLE_LINE`] is 1-based, as `line_span` below documents,
/// and `virtual_text::Anchor::line` is 0-based.
fn at_the_trouble_line() -> RowPolicy {
    RowPolicy {
        cursor: TROUBLE_LINE - 1,
        ..RowPolicy::default()
    }
}

/// The half-open character range of a 1-based line, excluding its newline —
/// the coordinate space `set_marks_colored` and [`StyledSpan`] both take.
fn line_span(text: &str, line: usize) -> (usize, usize) {
    let start: usize = text
        .lines()
        .take(line - 1)
        .map(|l| l.chars().count() + 1)
        .sum();
    let len = text.lines().nth(line - 1).map_or(0, |l| l.chars().count());
    (start, start + len)
}

/// `6c`'s diagnostic: the type mismatch on the `jitter` call.
///
/// The span is the call expression, which is what the mockup undercurls. The
/// `E0308` in the mockup's text is the server's diagnostic **code**, and
/// `phosphor_core::request::Diagnostic` has no field for one — see the note
/// under the snapshot.
fn e0308() -> Diagnostic {
    Diagnostic {
        span: Span {
            start: Position {
                line: TROUBLE_LINE as u32,
                column: 9,
            },
            end: Position {
                line: TROUBLE_LINE as u32,
                column: 40,
            },
        },
        severity: Severity::Trouble,
        message: "expected Duration, found u128".to_owned(),
        source: Some("rust-analyzer".to_owned()),
    }
}

/// `6c`'s thread, as three `┊` rows under the line it followed. A fixture:
/// threads are `T068`, and what this frame needs from one is that it occupies
/// rows and says nothing in the bar.
fn thread_rows(theme: &Theme) -> Vec<virtual_text::Row> {
    let anchor = Anchor::line(ANCHOR_LINE - 1);
    [
        (
            "⚓ thread · was retry_with_backoff:19-21 · followed node fn:next_delay",
            theme.neutrals.meta,
        ),
        (
            "you: collapse these arms — use the shared backoff helper",
            theme.neutrals.meta,
        ),
        (
            "✻ claude: collapsed — and hoisted into next_delay during the split",
            theme.actors.claude,
        ),
    ]
    .into_iter()
    .map(|(text, colour)| {
        virtual_text::Row::new(
            anchor,
            vec![Run::new(text, ratatui_core::style::Style::new().fg(colour))],
        )
    })
    .collect()
}

/// The unseen edit, as regions over visual rows.
///
/// A fixture for what `T041` answers, and it goes through the same
/// [`RegionSpan`] the diagnostic's regions do — which is the whole point: the
/// ladder resolves one row's *set*, whoever contributed each member.
fn unseen(editor: &Editor) -> Vec<RegionSpan> {
    UNSEEN_LINES
        .iter()
        .filter_map(|line| editor.visual_row_for_position(line - 1, 0))
        .map(|row| RegionSpan::new(row..row + 1, RegionState::Unseen))
        .collect()
}

/// Buffer area above, statusline on the last row.
fn split(area: Rect) -> (Rect, Rect) {
    let body = Rect {
        height: area.height - 1,
        ..area
    };
    let status = Rect {
        y: area.bottom() - 1,
        height: 1,
        ..area
    };
    (body, status)
}

/// The screen row showing a 1-based source line.
fn row_of(editor: &Editor, body: Rect, line: usize) -> u16 {
    let row = editor
        .visual_row_for_position(line - 1, 0)
        .expect("the line is in the stream");
    body.y + u16::try_from(row - editor.get_offset_y()).expect("a row on screen")
}

#[test]
fn the_fixture_puts_the_impl_where_6c_draws_it() {
    let lines: Vec<&str> = RETRY_RS.lines().collect();
    assert_eq!(lines[IMPL_LINE - 1], "impl RetryPolicy {");
    assert!(lines[ANCHOR_LINE - 1].contains("pub fn next_delay"));
    assert!(lines[TROUBLE_LINE - 1].contains("jitter(exp.min(self.max_delay))"));
    // The span `e0308` claims is exactly that call, so the undercurl in the
    // snapshot is under the expression and not under the indent.
    let (start, _) = line_span(RETRY_RS, TROUBLE_LINE);
    let call: String = RETRY_RS.chars().skip(start + 8).take(31).collect();
    assert_eq!(call, "jitter(exp.min(self.max_delay))");
}

#[test]
fn screen_6c() {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 100, 12);
    let (body, status) = split(area);

    let mut editor = Editor::new("rust", RETRY_RS, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, &theme);
    virtual_text::configure(&mut editor, &theme);
    // Stated rather than detected: a snapshot must not depend on the `TERM` of
    // whoever runs it. The other half of `T085`'s path is asserted in
    // `phosphor_ui::diagnostics`' own tests and captured by `V009`'s tape.
    editor.set_underline_capability(Some(UnderlineCapability::Undercurl));

    let diagnostics = [e0308()];
    let vm = DiagnosticsVm::new(&diagnostics);

    // **The merge.** Both lists replace, so both are built once, in draw order:
    // the thread's rows first, so `6c`'s exchange sits directly under the
    // signature it is about.
    let mut rows = thread_rows(&theme);
    rows.extend(vm.rows(&theme, &at_the_trouble_line()));
    virtual_text::install(&mut editor, &rows);

    // §3's anchored region: tint + undercurl over the whole line (row 20),
    // merged with the diagnostic's undercurl the same way.
    let (anchor_start, anchor_end) = line_span(RETRY_RS, ANCHOR_LINE);
    editor.set_marks_colored(vec![(anchor_start, anchor_end, theme.regions.anchor)]);
    let mut spans = vec![StyledSpan::undercurl(
        anchor_start,
        anchor_end,
        theme.regions.anchor_undercurl,
    )];
    spans.extend(vm.underlines(&editor, &theme));
    editor.set_styled_spans(spans);

    apply_scroll(
        &mut editor,
        ScrollRequest::ToRow {
            row: u32::try_from(IMPL_LINE).expect("a line number"),
        },
        body,
    );

    // The column, resolved **once**, over every region covering each row —
    // which is what makes the priority a property of the composition.
    let mut regions = unseen(&editor);
    regions.extend(vm.regions(&editor));
    let marks = gutter::state_column(&regions, editor.visual_len_lines());

    let statusline = StatusLineVm {
        mode: Mode::Normal,
        file: Some(FileVm {
            path: "src/retry.rs",
            dirty: false,
        }),
        session: SessionState::Idle,
        ask_pending: false,
        unseen: 2,
        vcs: None,
        cursor: Some(CursorVm { line: 64, col: 9 }),
    };

    let mut buf = Buffer::empty(area);
    BufferView::new(&editor, &theme)
        .state_column(&marks)
        .render(body, &mut buf);
    StatusLine::new(&statusline, &theme).render(status, &mut buf);

    let frame = Frame {
        screen: "6c",
        theme_label: "phosphor dark",
        theme: &theme,
        notes: &[
            "THE ROW THIS FRAME EXISTS FOR is line 64: an unseen claude edit and a",
            "  diagnostic on the same row. §3's ladder is 'trouble > attention >",
            "  claude', so the bar is trouble-red — 6c itself draws that row's bar",
            "  green, which is the one place the mockup and §3 disagree. Flagged in",
            "  the T040 report, not folded in.",
            "Lines 62-63 carry the same unseen edit with no diagnostic, and are",
            "  claude-green; the ┊ rows between them carry no bar at all, because an",
            "  overlay is not a state (§3 rows 18 and 20).",
            "The ■ row is a row of its own. 6c draws `■ E0308: …` INLINE, after the",
            "  code on line 64; the fork inserts a virtual row UNDER the row showing",
            "  its anchor (VENDOR.md patch 8) and end-of-line virtual text is a patch",
            "  nobody has written. T040's own wording is '■ rows via VirtualText'.",
            "`E0308:` is the server's diagnostic code and request::Diagnostic has no",
            "  field for one, so the row carries the message alone.",
            "The statusline has no `■ 1`. StatusLineVm carries no diagnostic count",
            "  (T025 composes the line in Steel from that VM), so the count is a",
            "  contract for whoever owns it — see the T040 report.",
            "6c's closing summary row ('1 diagnostic · claude sees what LSP sees —",
            "  :c fix routes it with the anchor attached') routes a verb that lands",
            "  at S6.",
            "The thread's three rows and the anchor's tint are fixtures for T068 and",
            "  T087, standing in for the store exactly as golden_frames.rs's own",
            "  counters do.",
        ],
    };
    insta::assert_snapshot!("6c", frame.to_text(&buf));
    assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));

    // -- the acceptance, read off the cells ---------------------------------

    let bar = |line: usize| buf[(body.x, row_of(&editor, body, line))].bg;
    assert_eq!(
        bar(TROUBLE_LINE),
        theme.actors.trouble,
        "the diagnostic outranks the unseen edit on its own row"
    );
    for line in [ANCHOR_LINE, 63] {
        assert_eq!(
            bar(line),
            theme.actors.claude,
            "line {line} carries the unseen edit alone"
        );
    }
    assert_eq!(
        bar(IMPL_LINE),
        theme.neutrals.ground,
        "a row nothing covers says nothing"
    );

    // The `┊` rows: no bar, whatever is around them.
    let first_thread_row = row_of(&editor, body, ANCHOR_LINE) + 1;
    for dy in 0..3 {
        let y = first_thread_row + dy;
        assert_eq!(buf[(body.x, y)].bg, theme.neutrals.ground, "row {y}");
        assert_eq!(
            buf[(body.x + gutter_width(&editor), y)].symbol(),
            "┊",
            "row {y} is a virtual row"
        );
    }

    // The diagnostic's own row hangs under line 64 and says what the server
    // said, in trouble-red.
    let message_row = row_of(&editor, body, TROUBLE_LINE) + 1;
    let x = body.x + gutter_width(&editor);
    let text: String = (x..body.right())
        .map(|x| buf[(x, message_row)].symbol())
        .collect();
    assert_eq!(text.trim_end(), "┊ ■ expected Duration, found u128");
    assert_eq!(
        buf[(x + 2, message_row)].fg,
        theme.actors.trouble,
        "the ■ is the grade's colour"
    );

    // And the undercurl is under the call, not under the indent.
    let trouble_row = row_of(&editor, body, TROUBLE_LINE);
    let call_x = x + 8;
    assert!(
        buf[(call_x, trouble_row)].symbol().contains("\u{1b}[4:3m"),
        "{:?}",
        buf[(call_x, trouble_row)].symbol()
    );
    assert_eq!(
        buf[(call_x - 1, trouble_row)].symbol(),
        " ",
        "the indent is not undercurled"
    );
}

/// **Why the frame above merges its two row sources before installing either.**
///
/// [`virtual_text::install`] replaces the whole list, so a host that installed
/// the diagnostic's rows on their own after a thread's would delete the thread —
/// silently, because both calls succeed and the second frame simply has fewer
/// rows in it. This is that composition, wrong way and right way, on `6c`'s own
/// two sources.
///
/// The claim about `install` itself lives with `install`
/// (`phosphor_ui::virtual_text::tests::a_second_install_replaces_the_rows_the_first_one_left`);
/// what this adds is that `6c`'s picture depends on it.
#[test]
fn a_second_install_would_delete_the_threads_rows_which_is_why_they_are_merged() {
    let theme = Theme::phosphor_dark();
    let diagnostics = [e0308()];
    let vm = DiagnosticsVm::new(&diagnostics);

    let editor = |install: &dyn Fn(&mut Editor)| {
        let mut editor = Editor::new("rust", RETRY_RS, Vec::new()).expect("rust editor");
        buffer_view::configure(&mut editor, &theme);
        virtual_text::configure(&mut editor, &theme);
        install(&mut editor);
        editor
    };
    let virtual_rows = |editor: &Editor| {
        (0..editor.visual_len_lines())
            .filter(|row| virtual_text::is_virtual_row(editor, *row))
            .count()
    };

    // The frame's shape: one list, built from both sources.
    let merged = editor(&|editor| {
        let mut rows = thread_rows(&theme);
        rows.extend(vm.rows(&theme, &at_the_trouble_line()));
        virtual_text::install(editor, &rows);
    });
    assert_eq!(
        virtual_rows(&merged),
        4,
        "6c's three thread rows and the diagnostic's one"
    );

    // The shape a host reaches for first, and what it costs.
    let replaced = editor(&|editor| {
        virtual_text::install(editor, &thread_rows(&theme));
        virtual_text::install(editor, &vm.rows(&theme, &at_the_trouble_line()));
    });
    assert_eq!(
        virtual_rows(&replaced),
        1,
        "the second install replaces rather than adds, so the thread is gone"
    );
}
