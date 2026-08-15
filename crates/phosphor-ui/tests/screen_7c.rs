//! `T038` / `T039` — screen `7c`, *"just editing"*, as a golden frame.
//!
//! `CP-4` accepts on this one: *"lsp completion + signature help · no agent
//! anywhere · boring on purpose"*. So the frame's job is partly to show what is
//! **not** there — no claude green, no unseen counter, no dim behind the float.
//! The Design Brief's sentence for this screen is the acceptance criterion:
//! *"LSP completion, signature help, snippets — table stakes rendered in the
//! same float language. No agent chrome anywhere near it."*
//!
//! Same serialiser and the same review loop as `tests/golden_frames.rs`
//! (`cargo insta review`, or `just review`); a separate file because that one is
//! `T018`'s and carries `CP-1`'s four frames, and because this frame is
//! composed out of a passive float rather than a `BufferView` and a statusline
//! alone.
//!
//! The seam tests below the frame are the other half: a picture proves the
//! widget and says nothing about whether anything reaches it — which is exactly
//! what `scripts/lint-node-kinds.sh` exists to catch — so the tree-composed
//! path is exercised through the public API too, with the session behind
//! [`Resources`] the way the host will supply it.

mod frame_grid;

use frame_grid::Frame;
use phosphor_core::request::BufferId;
use phosphor_core::view::{Float as ViewFloat, Mood, Node, Tree};
use phosphor_ui::buffer_view::{self, BufferView, Editor, ScrollRequest, apply_scroll};
use phosphor_ui::float::{
    Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody, FloatSlot,
    MAX_DOC_ROWS, MAX_ITEM_ROWS, SignatureVm, anchored_max_cols,
};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::status_line::{CursorVm, FileVm, Mode, SessionState, StatusLine, StatusLineVm};
use phosphor_ui::theme::Theme;
use proptest::prelude::*;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

/// An anchored float's chrome: a border column and `float::PAD_COLS` padding
/// columns, each side. Spelled once here because three laws below are stated
/// against it and `6` in three places is three things to keep in step.
const CHROME_COLS: u16 = 6;

/// `7c`'s file, `src/fetch.rs`.
///
/// **Lines 1–29 are not in the mockup and neither is any other screen's copy of
/// this file.** `7c` draws a viewport at 30–33 and a fixture that started at
/// line 1 would put `30` in the gutter next to `use`, which is the one thing a
/// golden frame is for. The preamble is therefore a plausible header for the
/// file the picture is a view of — the same judgement `golden_frames.rs` records
/// for its trailing `}` — and it is entirely **above** the rows this frame
/// renders. What it changes is the parse, and through it the syntax colours on
/// the rows that are drawn.
const FETCH_RS: &str = "\
use std::collections::HashMap;
use std::time::Duration;

use futures::future::join_all;
use serde_json::Value;
use crate::retry::RetryPolicy;

#[derive(Debug)]
pub enum FetchError {
    Timeout,
    Status(u16),
    Body(String),
}

pub struct Client {
    base: String,
    headers: HashMap<String, String>,
    timeout: Duration,
}

async fn fetch_json(url: &str) -> Result<Value, FetchError> {
    let body = get(url).await.map_err(FetchError::Body)?;
    serde_json::from_str(&body).map_err(|_| FetchError::Body(url.to_owned()))
}

async fn get(url: &str) -> Result<String, String> {
    Err(url.to_owned())
}

pub async fn fetch_all(urls: &[String]) -> Vec<Result<Value, FetchError>> {
    let policy = RetryPolicy::de
    join_all(urls.iter().map(|u| fetch_json(u))).await
}
";

/// The line `7c` puts the cursor on, 1-based.
const CURSOR_LINE: usize = 31;

/// The first visible line, 1-based. `7c`'s gutter reads 30.
const TOP_LINE: usize = 30;

/// Characters of line 31 before the cursor: `    let policy = RetryPolicy::de`
/// with the caret after `de`, and `de` is what the server is completing.
const PREFIX: &str = "    let policy = RetryPolicy::";

/// An editor over `source`, scrolled so `top_line` is the first row. Same
/// configuration `golden_frames.rs` uses and for the same reasons — the scroll
/// goes through [`apply_scroll`] rather than the vendored core's `focus()`,
/// because invariant 3 says the viewport moves only when something asks.
fn editor(theme: &Theme, source: &str, top_line: usize, area: Rect) -> Editor {
    let mut editor = Editor::new("rust", source, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    let top_line = u32::try_from(top_line).expect("a line number");
    apply_scroll(&mut editor, ScrollRequest::ToRow { row: top_line }, area);
    editor
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

fn item(label: &str, detail: &str) -> CompletionItemVm {
    CompletionItemVm {
        label: label.to_owned(),
        detail: Some(detail.to_owned()),
    }
}

/// `7c`'s completion session, transcribed: three items, the first selected, and
/// the documentation line under the rule.
fn session(anchor: Anchor) -> CompletionVm {
    CompletionVm {
        items: vec![
            item("default()", "fn() -> RetryPolicy"),
            item("default_delay", "Duration"),
            item("deserialize", "fn(D) -> Result<Self>"),
        ],
        selected: 0,
        documentation: vec!["Returns the policy with 3 attempts, 200ms base, 1s cap.".to_owned()],
        anchor,
        width_floor: 0,
    }
}

// ---------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------

#[test]
fn screen_7c() {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 120, 12);
    let (body, status) = split(area);
    let editor = editor(&theme, FETCH_RS, TOP_LINE, body);

    // The word being completed starts here: the code column, plus the prefix
    // ahead of it on the row, on the row the cursor is on. This is the
    // arithmetic the host does to draw the cursor, which is why `Anchor` is in
    // screen cells and not in buffer coordinates.
    //
    // **`gutter_width` already counts the state bar**, which is what
    // `editor_area` insets by — `STATE_BAR_WIDTH + STATE_BAR_GAP`, in both — so
    // adding the two re-counts it and puts the float two columns right of the
    // word. It matched the mockup only because `de` happens to be two cells
    // wide. The three assertions under the snapshot are what hold it now; this
    // expression is what the handoff hands the wiring agent.
    let code_x = body.x + buffer_view::gutter_width(&editor);
    let anchor = Anchor::new(
        code_x + u16::try_from(PREFIX.chars().count()).expect("a column"),
        body.y + u16::try_from(CURSOR_LINE - TOP_LINE).expect("a row"),
    );
    let vm = session(anchor);
    let list = CompletionList::new(&vm);

    let statusline = StatusLineVm {
        mode: Mode::Insert,
        file: Some(FileVm {
            path: "src/fetch.rs",
            dirty: true,
        }),
        // `7c` has no session line at all — "no agent anywhere".
        session: SessionState::None,
        ask_pending: false,
        unseen: 0,
        vcs: Some("rust-analyzer ✓"),
        cursor: Some(CursorVm { line: 31, col: 34 }),
    };

    let mut buf = Buffer::empty(area);
    BufferView::new(&editor, &theme).render(body, &mut buf);
    FloatSlot::with(Float::passive(&list, anchor)).render(body, &mut buf, &theme);
    StatusLine::new(&statusline, &theme).render(status, &mut buf);

    let frame = Frame {
        screen: "7c",
        theme_label: "phosphor dark",
        theme: &theme,
        notes: &[
            "The passive float (T038): #2a3c2e border, no footer — §4's one",
            "  documented exception — and no header either, which §4 does not say and",
            "  7c draws. Mood::Passive carries both.",
            "NOTHING IS DIMMED. §9's dim means 'behind'; this float is not in front",
            "  of anything, because you are still typing into the code under it. The",
            "  code around it is at full strength, as 7c draws it.",
            "The float is anchored to the word being completed rather than centered in",
            "  §8's 60-80% band, and is sized to its content. 7c is the only drawing",
            "  of this float and that is what it draws.",
            "The selection tint (#26332a + bright text) and the documentation rule",
            "  cover the body's rows, which sit two columns inside the border; 7c runs",
            "  both edge to edge. A FloatBody is handed an area inside the padding and",
            "  must clip to it — the same rectangle a Tint::Selection span row gets.",
            "The documentation rule is chrome.divider (#242a24); 7c draws #1d241d.",
            "  One step apart on the same ramp, and §4 hexes no internal rule.",
            "vcs is the statusline slot 7c puts `rust-analyzer ✓` in, and in THIS frame",
            "  it is a fixture string, exactly as 8d's `jj ✓` is: StatusLineVm is the",
            "  widget's own ViewModel and has no server field. It stopped being a",
            "  stand-in at CP-4 — status::StatusVm carries `server`, main::server_chip",
            "  builds it out of ServerState, and crates/phosphor/tests/screen_7c.rs is",
            "  the same screen with the statusline composed by runtime/statusline.scm.",
            "7c draws the insert cursor as an inverted cell after `de` on line 31 and",
            "  this frame has none: the cursor is the terminal's, placed by the host",
            "  through Frame::set_cursor_position, and a golden frame is a Buffer.",
            "  The same absence golden_frames.rs records for the Picker's query block.",
            "Lines 1-29 of the fixture are not in any mockup — see FETCH_RS.",
        ],
    };
    insta::assert_snapshot!("7c", frame.to_text(&buf));
    assert!(frame.unnamed(&buf).is_empty(), "{:?}", frame.unnamed(&buf));

    // **The float hangs off the word, not two cells right of it.** The anchor
    // cell still holds the `d` of `de` — the float lands on the row below — and
    // the frame's top-left corner is directly under it.
    assert_eq!(buf[(anchor.col, anchor.row)].symbol(), "d");
    assert_eq!(buf[(anchor.col - 1, anchor.row)].symbol(), ":");
    assert_eq!(buf[(anchor.col, anchor.row + 1)].symbol(), "┌");
}

// ---------------------------------------------------------------------------
// The seam: composed, not hand-built
// ---------------------------------------------------------------------------

/// A host that has the LSP session and no buffers.
#[derive(Debug, Default)]
struct Host {
    completion: Option<CompletionVm>,
    signature: Option<SignatureVm>,
}

impl Resources for Host {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        None
    }

    fn completion(&self) -> Option<&CompletionVm> {
        self.completion.as_ref()
    }

    fn signature(&self) -> Option<&SignatureVm> {
        self.signature.as_ref()
    }
}

const TREE_AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 20,
};

fn draw_tree(tree: &Tree, host: &Host) -> (Buffer, Vec<&'static str>) {
    let theme = Theme::phosphor_dark();
    let mut buf = Buffer::empty(TREE_AREA);
    let report = Interpreter::new(&theme, host).render(tree, TREE_AREA, &mut buf);
    (buf, report.deferred)
}

fn text(buf: &Buffer) -> String {
    (TREE_AREA.y..TREE_AREA.bottom())
        .map(|y| {
            (TREE_AREA.x..TREE_AREA.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// **The reachability half.** A `completion` node inside a passive float is what
/// the shipped editor will compose; this proves the tag draws rather than
/// landing in `Report::deferred`, which is what it did until `T038`.
#[test]
fn a_composed_completion_float_draws_the_session() {
    let host = Host {
        completion: Some(session(Anchor::new(12, 3))),
        signature: None,
    };
    let tree =
        Tree::new(Node::Empty {}).with_float(ViewFloat::new(Mood::Passive, Node::Completion {}));
    let (buf, deferred) = draw_tree(&tree, &host);
    assert!(deferred.is_empty(), "{deferred:?}");
    let drawn = text(&buf);
    assert!(drawn.contains("default()"), "{drawn}");
    assert!(drawn.contains("fn(D) -> Result<Self>"), "{drawn}");
    assert!(drawn.contains("Returns the policy"), "{drawn}");
}

/// `T039`'s half of the same seam.
#[test]
fn a_composed_signature_float_draws_through_the_same_chrome() {
    let host = Host {
        completion: None,
        signature: Some(SignatureVm {
            label: Some("fn fetch_json(url: &str) -> Result<Value, FetchError>".to_owned()),
            active: Some((14, 23)),
            prose: vec!["one request, deserialised".to_owned()],
            anchor: Anchor::new(8, 5),
            width_floor: 0,
        }),
    };
    let tree =
        Tree::new(Node::Empty {}).with_float(ViewFloat::new(Mood::Passive, Node::Signature {}));
    let (buf, deferred) = draw_tree(&tree, &host);
    assert!(deferred.is_empty(), "{deferred:?}");
    let drawn = text(&buf);
    assert!(drawn.contains("fn fetch_json(url: &str)"), "{drawn}");
    assert!(drawn.contains("one request, deserialised"), "{drawn}");
    // Same border as the completion list: one float language, not two.
    let theme = Theme::phosphor_dark();
    assert_eq!(buf[(8, 6)].fg, theme.float.passive);
}

/// A `completion` node **outside** a float is still a completion list. A node
/// kind that only worked in one wrapper would be a lie about the protocol.
#[test]
fn a_completion_node_composed_bare_is_still_a_list() {
    let host = Host {
        completion: Some(session(Anchor::new(0, 0))),
        signature: None,
    };
    let (buf, deferred) = draw_tree(&Tree::new(Node::Completion {}), &host);
    assert!(deferred.is_empty(), "{deferred:?}");
    assert!(text(&buf).contains("default()"), "{}", text(&buf));
}

// ---------------------------------------------------------------------------
// Laws
// ---------------------------------------------------------------------------

prop_compose! {
    /// A completion session of arbitrary shape, including the degenerate ones:
    /// no items, no details, a selection past the end, wide characters.
    ///
    /// **The strings run past any terminal on purpose** (`CP-4`): the finding
    /// was a float that *"will go from small to across the screen"*, and a
    /// generator whose widest row fit in 120 columns could never have produced
    /// it. A 240-cell `detail` and a 200-cell documentation line are what a
    /// server actually sends; the cap law below is stated against them.
    fn any_session()(
        labels in prop::collection::vec("[a-z_]{0,40}|名前|🙂x", 0..12),
        details in prop::collection::vec(prop::option::of("[a-zA-Z<>() ,:-]{0,120}"), 0..12),
        selected in 0usize..14,
        docs in prop::collection::vec("[a-z .]{0,200}", 0..3),
        col in 0u16..200,
        row in 0u16..60,
    ) -> CompletionVm {
        CompletionVm {
            items: labels
                .iter()
                .enumerate()
                .map(|(i, label)| CompletionItemVm {
                    label: label.clone(),
                    detail: details.get(i).cloned().flatten(),
                })
                .collect(),
            selected,
            documentation: docs,
            anchor: Anchor::new(col, row),
            width_floor: 0,
        }
    }
}

proptest! {
    /// **The law every float has to keep and this one is the first that could
    /// break**: an anchored float is placed from a number the host supplies, so
    /// an anchor near the edge, a list wider than the terminal or a terminal
    /// three cells tall must still leave every written cell inside the area.
    #[test]
    fn a_passive_float_never_draws_outside_its_area(
        vm in any_session(),
        width in 3u16..140,
        height in 2u16..40,
    ) {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, height);
        // A buffer larger than the area, so a float that spilled would be
        // visible rather than clipped by the buffer and silently correct.
        let mut buf = Buffer::empty(Rect::new(0, 0, width + 8, height + 4));
        let list = CompletionList::new(&vm);
        let float = Float::passive(&list, vm.anchor);
        let frame = float.frame(area);
        FloatSlot::with(float).render(area, &mut buf, &theme);

        prop_assert!(frame.right() <= area.right(), "{frame:?} in {area:?}");
        prop_assert!(frame.bottom() <= area.bottom(), "{frame:?} in {area:?}");
        for y in 0..height + 4 {
            for x in 0..width + 8 {
                let touched = buf[(x, y)].symbol() != " ";
                prop_assert!(
                    !touched || frame.contains(ratatui_core::layout::Position::new(x, y)),
                    "cell ({x}, {y}) is outside {frame:?}"
                );
            }
        }
    }

    /// **Chrome rows are conserved**: whatever the session, a passive float that
    /// is drawn at all is its *shown* content plus exactly two border rows —
    /// never taller than that (§8), never shorter than its chrome.
    ///
    /// *Shown* is the caps, added at `CP-4`: a real server answers with the
    /// whole scope and a float sized to the answer covered a 30-row terminal,
    /// so the list is a window of [`MAX_ITEM_ROWS`] and the documentation a
    /// summary of [`MAX_DOC_ROWS`]. The law is stated against those rather than
    /// against `len()`, which is what makes it a law about the float instead of
    /// a restatement of the two `min` calls.
    ///
    /// *Drawn at all* is the whole of the other half: a session with no items,
    /// or one whose every label, detail and doc line is empty, has no content
    /// in one axis or the other and draws nothing — a bordered box with nothing
    /// in it is an artifact beside the cursor, in columns exactly as in rows.
    #[test]
    fn chrome_is_two_rows_and_six_columns(vm in any_session()) {
        let area = Rect::new(0, 0, 200, 80);
        let list = CompletionList::new(&vm);
        let frame = Float::passive(&list, vm.anchor).frame(area);
        if vm.items.is_empty() || list.desired_width() == 0 {
            prop_assert_eq!(frame.height, 0, "a session with no content draws nothing");
            prop_assert_eq!(frame.width, 0);
            return Ok(());
        }
        let items = vm.items.len().min(MAX_ITEM_ROWS as usize);
        let docs = if vm.documentation.is_empty() {
            0
        } else {
            vm.documentation.len().min(MAX_DOC_ROWS as usize) + 1
        };
        prop_assert_eq!(usize::from(frame.height), items + docs + 2);
        prop_assert!(frame.height <= 2 + MAX_ITEM_ROWS + MAX_DOC_ROWS + 1,
            "and never more, whatever the server sent: {frame:?}");
        // Six columns of chrome under `CP-4`'s cap — the same shape as the row
        // law above, where *shown* content is what the float is sized to.
        prop_assert_eq!(
            frame.width,
            (list.desired_width() + CHROME_COLS).min(anchored_max_cols(area.width))
        );
    }

    /// **`CP-4`'s cap, as a law rather than as three examples.** No session, at
    /// any terminal width, may hand the anchored float more than
    /// [`anchored_max_cols`] — which is the whole of the finding's first half,
    /// *"it will go … across the screen"*.
    ///
    /// Stated over the *drawn* frame and not over the arithmetic, and swept
    /// across widths rather than fixed at two, because the two golden frames
    /// already pin 120 and 80 and the failure this is for is a rounding step
    /// nobody looked at — a float one column over the cap at width 37.
    ///
    /// **What it deliberately does not claim**: that the cap is 60%. It used to
    /// carry a second assertion spelled in `ANCHORED_WIDTH_PCT`, which is a
    /// claim about the arithmetic agreeing with itself — a review moved the
    /// constant to 61 and it stayed green while five other tests reddened. The
    /// value is pinned by literal in `float_width::the_cap_at_both_widths`,
    /// which is the right division of labour: **that** test owns the number and
    /// this one owns the behaviour at every width in between. The second
    /// assertion here is now the biting half of the same claim — a float that
    /// *wants* more than the cap gets exactly the cap, so a rounding step that
    /// left it a column short would be as red as one that left it a column
    /// over.
    #[test]
    fn an_anchored_float_never_takes_more_than_the_cap(
        vm in any_session(),
        width in 8u16..200,
        floor in 0u16..300,
    ) {
        let area = Rect::new(0, 0, width, 40);
        let list = CompletionList::new(&vm);
        let frame = Float::passive(&list, vm.anchor)
            .with_width_floor(floor)
            .frame(area);
        let cap = anchored_max_cols(width);
        prop_assert!(frame.width <= cap, "{frame:?} over the {cap}-column cap at {width}");
        let wanted = list.desired_width().max(floor).saturating_add(CHROME_COLS);
        if frame.width > 0 && wanted >= cap {
            prop_assert_eq!(
                frame.width, cap,
                "a session asking for {} columns at width {} should take the whole cap",
                wanted, width
            );
        }
    }

    /// **The thrash law**: a session's floor **is reached**, and is a floor and
    /// nothing else. It widens the float to what it asks for, it never shrinks
    /// it, it never lifts it over the cap, and it never puts a box beside the
    /// cursor that a session with nothing in it would not have earned.
    ///
    /// The **first** of those is what [`Float::with_width_floor`] is *for*, and
    /// it is the one this test did not state. Every other clause here is
    /// satisfied by `held == bare`, so the version that shipped stayed green
    /// under a floor made a no-op (`.max(self.width_floor.min(0))`) — a test
    /// named for the fix, unable to fail on it, while two unit tests one file
    /// over caught it. It now asserts the width **exactly**: the wider of the
    /// content and the floor, plus chrome, under the cap.
    #[test]
    fn a_width_floor_only_ever_widens(vm in any_session(), floor in 0u16..300) {
        let area = Rect::new(0, 0, 120, 40);
        let cap = anchored_max_cols(area.width);
        let list = CompletionList::new(&vm);
        let bare = Float::passive(&list, vm.anchor).frame(area);
        let held = Float::passive(&list, vm.anchor).with_width_floor(floor).frame(area);

        if bare.width == 0 {
            prop_assert_eq!(held.width, 0, "a floor does not resurrect a dead session");
            return Ok(());
        }
        // The law, stated whole. Every clause below is a reading of it, kept
        // because each names a different way of getting it wrong.
        prop_assert_eq!(
            held.width,
            list.desired_width().max(floor).saturating_add(CHROME_COLS).min(cap),
            "a floor of {} over content of {}", floor, list.desired_width()
        );
        prop_assert!(held.width >= bare.width, "{held:?} narrower than {bare:?}");
        prop_assert!(held.width <= cap);
        // And the two agree exactly whenever the floor asks for nothing the
        // content did not already ask for.
        if floor <= list.desired_width() {
            prop_assert_eq!(held.width, bare.width);
        }
    }

    /// The selected row is on screen for every selection, at every height — the
    /// property the `ctrl-n` scroll exists for.
    ///
    /// Stated as *"exactly one row carries the selection tint"* rather than as
    /// *"the label is somewhere in the text"*, because a row read back out of a
    /// `Buffer` carries the continuation cell of every wide grapheme (`名前`
    /// comes back as `名 前`) and a text search would fail on CJK for a reason
    /// that has nothing to do with the widget. The tint is the thing the user
    /// actually steers by.
    ///
    /// An out-of-range selection is excluded because it tints nothing by
    /// construction (`CompletionVm::selected`) — what it must **not** do is
    /// scroll the list off its own float, which
    /// `float.rs::a_selection_out_of_range_tints_nothing_and_still_draws_the_list`
    /// states directly.
    #[test]
    fn the_selection_is_always_on_screen(vm in any_session(), height in 3u16..24) {
        prop_assume!(!vm.items.is_empty());
        // Folded into range rather than assumed into it: `any_session` draws a
        // selection past the end deliberately, and rejecting those ran the
        // generator out of budget ("too many global rejects") at high case
        // counts instead of testing anything.
        let vm = CompletionVm { selected: vm.selected % vm.items.len(), ..vm };
        let list = CompletionList::new(&vm);
        // Nothing to steer, and nothing drawn — the case above states it.
        prop_assume!(list.desired_width() > 0);

        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, 120, height);
        let mut buf = Buffer::empty(area);
        let float = Float::passive(&list, Anchor::new(0, 0));
        let frame = float.frame(area);
        FloatSlot::with(float).render(area, &mut buf, &theme);

        let tinted: Vec<u16> = (frame.y..frame.bottom())
            .filter(|&y| buf[(frame.x + 3, y)].bg == theme.regions.selection)
            .collect();
        prop_assert_eq!(tinted.len(), 1, "{:?} rows tinted in {:?}", tinted, frame);
    }
}
