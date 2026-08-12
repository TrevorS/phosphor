//! `T018` — the four golden frames `CP-1` accepts on: `1a`-minus-agent, `9c`,
//! `8c`, `8d`.
//!
//! Each test builds the mockup's own state — its buffer, its viewport, its
//! gutter marks, its statusline ViewModel — renders it into a [`Buffer`] the
//! size of a terminal, and commits the result through `insta`. The serialiser
//! and the reasoning behind its shape are in [`frame_grid`].
//!
//! **Read the `notes` lines at the top of each `.snap`.** They say what the
//! frame is missing and which task owns it, so nobody has to reverse-engineer
//! an absence. Every one of them is a surface that does not exist yet, not a
//! surface that was left out to make the snapshot green.
//!
//! # Reviewing an intentional change
//!
//! ```text
//! cargo insta review          # or: INSTA_FORCE_PASS=0 cargo nextest run
//! ```
//!
//! A snapshot that changed without a mockup changing is a regression. A layout
//! diff in the `text` grid is a geometry change; a diff in `fg`/`bg` with the
//! text grid unchanged is a **palette** change, which at `CP-1` is the more
//! serious of the two.

mod frame_grid;

use frame_grid::Frame;
use phosphor_ui::buffer_view::{
    self, BufferView, Editor, ScrollRequest, StateMark, apply_scroll, editor_area,
};
use phosphor_ui::float::{Float, FloatFooter, FloatHeader, FloatSlot, FooterHint, TextBody};
use phosphor_ui::status_line::{CursorVm, FileVm, Mode, SessionState, StatusLine, StatusLineVm};
use phosphor_ui::theme::Theme;
use ratatui_code_editor::phosphor::cell_style::{StyledSpan, UnderlineCapability};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

/// Mockup `1a`'s file, transcribed: twenty-four lines of `src/retry.rs`.
///
/// **The mockups draw two versions of this file and they differ by one line.**
/// `1a` lists lines 1–24 in full and goes straight from `let mut delay = …`
/// (16) to `for attempt in … {` (17). Turn 8's screens — `8c` at 15–20, `8d` at
/// 17–18, `9c` at 16–19 — all agree with each other and all show
/// `let mut last = None;` at 17, which pushes everything below it down one.
///
/// Neither is wrong; they were drawn at different times. Each frame below
/// therefore uses the file **its own mockup draws**, so a line number in a
/// snapshot is the line number on the screen it is being compared to. See
/// [`RETRY_RS_TURN_8`].
const RETRY_RS_1A: &str = "\
use std::thread;
use std::time::Duration;

use crate::util::jitter;

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

pub fn retry_with_backoff<T, E>(
    mut op: impl FnMut() -> Result<T, E>,
    policy: &RetryPolicy,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt + 1 == policy.max_attempts => return Err(e),
            Err(_) => thread::sleep(jitter(delay)),
        }
        delay = (delay * 2).min(policy.max_delay);
    }
";

/// The same file as turn 8 draws it: `let mut last = None;` at line 17, which
/// is what `8c`, `8d` and `9c` all show. See [`RETRY_RS_1A`].
const RETRY_RS_TURN_8: &str = "\
use std::thread;
use std::time::Duration;

use crate::util::jitter;

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

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
            Err(_) => thread::sleep(jitter(delay)),
        }
        delay = (delay * 2).min(policy.max_delay);
    }
";

/// The half-open character range of a 1-based line, excluding its newline.
///
/// The coordinate space `set_marks_colored` and [`StyledSpan`] both take.
fn line_span(text: &str, line: usize) -> (usize, usize) {
    let start: usize = text
        .lines()
        .take(line - 1)
        .map(|l| l.chars().count() + 1)
        .sum();
    let len = text.lines().nth(line - 1).map_or(0, |l| l.chars().count());
    (start, start + len)
}

/// An editor over `source`, configured with the phosphor contract and scrolled
/// so `top_line` (1-based) is the first row.
///
/// The scroll goes through [`apply_scroll`] rather than the vendored core's own
/// `focus()` — invariant 3 is that the viewport moves only when something asks,
/// and a fixture is not an exception.
fn editor(theme: &Theme, source: &str, top_line: usize, area: Rect) -> Editor {
    let mut editor = Editor::new("rust", source, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    // Both halves of `T085`'s degradation path are reachable; a snapshot must
    // not depend on the `TERM` of whoever runs it, so the capability is stated
    // rather than detected. `V009`'s tape is where the *other* half is shown.
    editor.set_underline_capability(Some(UnderlineCapability::Undercurl));
    apply_scroll(&mut editor, ScrollRequest::ToRow(top_line - 1), area);
    editor
}

/// Design Language §3's anchored-region treatment — **tint + undercurl** — over
/// one whole line, which is what `9c` and `8c` draw on line 18.
///
/// Both halves are the vendored fork's own APIs (`VENDOR.md` patches 4 and 5).
/// The *widget* that decides which lines get this is `T087`, S5; these are
/// fixture coordinates standing in for the store query, exactly as the
/// statusline's counters below stand in for `T041`.
fn anchor_line(editor: &mut Editor, theme: &Theme, source: &str, line: usize) {
    let (start, end) = line_span(source, line);
    editor.set_marks_colored(vec![(start, end, theme.regions.anchor)]);
    editor.set_styled_spans(vec![StyledSpan::undercurl(
        start,
        end,
        theme.regions.anchor_undercurl,
    )]);
}

/// A state column with `ClaudeUnseen` on the given 1-based lines and nothing
/// anywhere else.
///
/// §3's unseen marker. It is a store query in the product (`T041`, S5) and a
/// fixture here — `BufferView` takes the resolved column as a slice, so no
/// surface is being faked, only supplied.
fn unseen(lines: &[usize]) -> Vec<StateMark> {
    let rows = lines.iter().copied().max().unwrap_or(0);
    let mut column = vec![StateMark::None; rows];
    for line in lines {
        column[line - 1] = StateMark::ClaudeUnseen;
    }
    column
}

/// Buffer area above, statusline on the last row — the layout every mockup in
/// this checkpoint has.
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

// ---------------------------------------------------------------------------
// 1a — "Default view — code-first", minus the agent surfaces
// ---------------------------------------------------------------------------

#[test]
fn screen_1a_minus_agent() {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 100, 25);
    let (body, status) = split(area);

    let editor = editor(&theme, RETRY_RS_1A, 1, body);
    // The mockup's own gutter: lines 4, 6-10 and 12-24 carry the unseen bar.
    let marks = unseen(&[
        4, 6, 7, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
    ]);

    let vm = StatusLineVm {
        mode: Mode::Normal,
        file: Some(FileVm {
            path: "src/retry.rs",
            dirty: false,
        }),
        session: SessionState::Idle,
        ask_pending: false,
        unseen: 6,
        vcs: Some("jj ✓"),
        cursor: Some(CursorVm { line: 12, col: 1 }),
    };

    let mut buf = Buffer::empty(area);
    BufferView::new(&editor, &theme)
        .state_column(&marks)
        .render(body, &mut buf);
    StatusLine::new(&vm, &theme).render(status, &mut buf);

    let frame = Frame {
        screen: "1a-minus-agent",
        theme_label: "phosphor dark",
        theme: &theme,
        notes: &[
            "BufferView (T015) + state column + StatusLine (T017), full width.",
            "MINUS the agent surfaces, as CP-1 asks: the `✻ claude · review ready`",
            "  notification is S6 (T054/T057) and is absent, not stubbed.",
            "State-column marks and the `6 unseen` counter are fixture values;",
            "  their source is the store (T041, S5).",
            "KNOWN DIFFERENCE, row 11: the mockup paints `retry_with_backoff` in",
            "  syntax.function; it is neutrals.text here. The vendored renderer runs",
            "  the highlight query one line at a time, and rust's `(function_item",
            "  name: (identifier) @function)` needs a range that spans the whole",
            "  item — so a *definition* name loses its capture while every call site",
            "  (`op`, `sleep`, `jitter`) keeps it. Fork-level; see T018's report.",
        ],
    };
    insta::assert_snapshot!("1a-minus-agent", frame.to_text(&buf));
    assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));
}

// ---------------------------------------------------------------------------
// 9c — "Original — phosphor (working name)": both variants, same content
// ---------------------------------------------------------------------------

/// `9c` draws dark and light **side by side over identical code** — the claim
/// is that the actor contract survives the lightness flip. So the snapshot
/// carries both renders, one after the other, in one file: the `fg`/`bg` grids
/// are keyed by theme *field*, so the two halves are byte-identical there and
/// any drift in which field paints what shows up as a diff.
///
/// They are two renders rather than two panes because panes are `T088`,
/// three windows away; drawing them side by side would be inventing a layout.
#[test]
fn screen_9c() {
    let mut out = String::new();
    for (label, theme) in [
        ("phosphor dark", Theme::phosphor_dark()),
        ("phosphor light", Theme::phosphor_light()),
    ] {
        let area = Rect::new(0, 0, 61, 5);
        let (body, status) = split(area);

        let mut editor = editor(&theme, RETRY_RS_TURN_8, 16, body);
        anchor_line(&mut editor, &theme, RETRY_RS_TURN_8, 18);
        let marks = unseen(&[16, 17]);

        let vm = StatusLineVm {
            mode: Mode::Normal,
            file: Some(FileVm {
                path: "src/retry.rs",
                dirty: true,
            }),
            session: SessionState::Idle,
            ask_pending: false,
            unseen: 6,
            vcs: Some("jj ✓"),
            cursor: None,
        };

        let mut buf = Buffer::empty(area);
        BufferView::new(&editor, &theme)
            .state_column(&marks)
            .render(body, &mut buf);
        StatusLine::new(&vm, &theme).render(status, &mut buf);

        let frame = Frame {
            screen: "9c",
            theme_label: label,
            theme: &theme,
            notes: &[
                "Lines 16-19 with the anchored region on 18: tint + undercurl (§3),",
                "  through the fork's set_marks_colored and StyledSpan (T085).",
                "MINUS the two `┊ ⚓ you` / `┊ ✻ claude` annotation rows, which are",
                "  VirtualText (T032, S3) and do not exist yet.",
                "The mockup tints the whole row including the gutter; the marks API",
                "  tints only the characters it covers. T087 owns that difference.",
                "The mockup draws `●6 │ jj ✓`; at 61 columns the ladder still has",
                "  room for `6 unseen`, and contracts it at 64. Order agrees, width",
                "  does not — see the `shed-ladder` snapshot.",
            ],
        };
        out.push_str(&frame.to_text(&buf));
        out.push('\n');
        assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));
    }
    insta::assert_snapshot!("9c", out);
}

// ---------------------------------------------------------------------------
// 8c — "Light theme — the palette holds"
// ---------------------------------------------------------------------------

#[test]
fn screen_8c() {
    let theme = Theme::phosphor_light();
    let area = Rect::new(0, 0, 100, 7);
    let (body, status) = split(area);

    let mut editor = editor(&theme, RETRY_RS_TURN_8, 15, body);
    anchor_line(&mut editor, &theme, RETRY_RS_TURN_8, 18);
    let marks = unseen(&[16, 17]);

    let vm = StatusLineVm {
        mode: Mode::Normal,
        file: Some(FileVm {
            path: "src/retry.rs",
            dirty: false,
        }),
        session: SessionState::Idle,
        ask_pending: false,
        unseen: 2,
        vcs: Some("jj ✓"),
        cursor: None,
    };

    let mut buf = Buffer::empty(area);
    BufferView::new(&editor, &theme)
        .state_column(&marks)
        .render(body, &mut buf);
    StatusLine::new(&vm, &theme).render(status, &mut buf);

    let frame = Frame {
        screen: "8c",
        theme_label: "phosphor light",
        theme: &theme,
        notes: &[
            "Lines 15-20, wide, on warm paper. The acceptance question is whether",
            "  claude-green is still the brightest thing on screen (§10, Q7).",
            "MINUS the VirtualText annotation rows (T032, S3), as in 9c.",
        ],
    };
    insta::assert_snapshot!("8c", frame.to_text(&buf));
    assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));
}

// ---------------------------------------------------------------------------
// 8d — "80 columns — drop, don't squeeze"
// ---------------------------------------------------------------------------

#[test]
fn screen_8d() {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 80, 13);
    let (body, status) = split(area);

    let editor = editor(&theme, RETRY_RS_TURN_8, 17, body);
    let marks = unseen(&[17]);

    // The picker's rows, as `8d` draws them. The body is `TextBody` — the
    // fixture body `T084` shipped to prove the seam — because `Picker` is
    // `T045`, S5. What that costs is visible in the snapshot: every row is one
    // colour, so the mockup's selected row, its dimmed second row and its meta
    // third row are all `neutrals.text` here.
    let rows = [
        "▸ src/retry.rs:6–10   +RetryPolicy struct",
        "  src/retry.rs:12–24  +retry_with_backoff",
        "  no preview under 100 cols · ↵ opens",
    ];
    let body_fixture = TextBody::new(&rows);
    let hints = [
        FooterHint::new("↵", "open"),
        FooterHint::new("s", "mark seen"),
        FooterHint::bare("esc"),
    ];
    let float = Float::informational(
        FloatHeader::new("❯ unseen"),
        &body_fixture,
        FloatFooter::new(&hints),
    );

    let vm = StatusLineVm {
        mode: Mode::Normal,
        file: Some(FileVm {
            path: "src/retry.rs",
            dirty: true,
        }),
        session: SessionState::Idle,
        ask_pending: false,
        unseen: 6,
        vcs: Some("jj ✓"),
        cursor: Some(CursorVm { line: 18, col: 1 }),
    };

    let mut buf = Buffer::empty(area);
    BufferView::new(&editor, &theme)
        .state_column(&marks)
        .render(body, &mut buf);
    FloatSlot::with(float).render(body, &mut buf, &theme);
    StatusLine::new(&vm, &theme).render(status, &mut buf);

    let frame = Frame {
        screen: "8d",
        theme_label: "phosphor dark",
        theme: &theme,
        notes: &[
            "80 columns: the float goes full-width and docks to the bottom of the",
            "  buffer area (§11, Layout::FullWidth).",
            "THE STATUSLINE DOES NOT SHED AT 80. The mockup draws the ladder's floor",
            "  (`N` · `retry.rs [+]` · `✻` · `●6`), which this VM reaches at width 24.",
            "  The whole ladder is the `shed-ladder` snapshot; the order matches 8d's",
            "  caption exactly, only the widths differ from the drawing.",
            "Body is TextBody, T084's fixture body — Picker is T045, S5. The",
            "  mockup's per-row picker colouring is therefore flat here.",
            "The header's query cursor block is the Picker's too, and is absent.",
            "FloatSlot dims the code behind the float to neutrals.dimmed_under_float",
            "  (§9). Mockup 8d draws those two rows undimmed; 3c/3d/7a draw the dim.",
            "The float body is painted float.body; mockup 8d leaves the docked float",
            "  on neutrals.ground and only 8a's centered float paints a body colour.",
        ],
    };
    insta::assert_snapshot!("8d", frame.to_text(&buf));
    assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));
}

// ---------------------------------------------------------------------------
// The geometry the frames above assert by picture, asserted by number
// ---------------------------------------------------------------------------

/// The gutter is where every column in every frame is measured from, so it gets
/// an assertion of its own rather than living only inside a picture.
#[test]
fn the_gutter_is_six_cells_and_the_editor_starts_two_in() {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 100, 24);
    let editor = editor(&theme, RETRY_RS_1A, 1, area);
    assert_eq!(buffer_view::gutter_width(&editor), 6);
    assert_eq!(editor_area(area).x, 2);
}

/// `8d`'s caption spells the ladder out — *"counters → jj → cursor pos →
/// session prose (glyph stays) → mode word (initial stays)"* — and draws its
/// floor: `N`, `retry.rs [+]`, `✻`, `●6`.
///
/// This walks the widths down and records the first width at which each rung is
/// gone, so `CP-1` can see the whole ladder as numbers next to the one width
/// the `8d` frame above captures. It is the mechanical half of the width sweep
/// `V003` records as pictures.
#[test]
fn the_shed_ladder_matches_the_order_8d_documents() {
    let theme = Theme::phosphor_dark();
    let vm = StatusLineVm {
        mode: Mode::Normal,
        file: Some(FileVm {
            path: "src/retry.rs",
            dirty: true,
        }),
        session: SessionState::Idle,
        ask_pending: false,
        unseen: 6,
        vcs: Some("jj ✓"),
        cursor: Some(CursorVm { line: 18, col: 1 }),
    };

    let mut ladder = String::new();
    let mut previous = String::new();
    for width in (20..=120).rev() {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        StatusLine::new(&vm, &theme).render(area, &mut buf);
        let row: String = (0..width).map(|x| buf[(x, 0)].symbol()).collect();
        let row = row.trim_end().to_owned();
        if row != previous {
            ladder.push_str(&format!("{width:>3} │{row}│\n"));
            previous = row;
        }
    }
    insta::assert_snapshot!("shed-ladder", ladder);
}
