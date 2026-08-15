//! `T038`/`T039` — screen `7c` **as the host composes it**, at two widths.
//!
//! `CP-4` names *"the `7c` snapshot"* among the things Claude verifies, and one
//! already existed: `crates/phosphor-ui/tests/screen_7c.rs` is the widget's,
//! and it is the frame the mockup was transcribed into. This is the other half
//! of the same pair `3c`, `6b`, `6d` and `8e` each have — the screen drawn the
//! way the **binary** draws it:
//!
//! * the statusline is `runtime/statusline.scm`'s, composed by the editor layer
//!   through `phosphor_steel::status`, rather than `phosphor-ui`'s own
//!   `StatusLine` widget;
//! * the float is a `Node::Completion` inside a passive `view::Float`, drawn by
//!   the interpreter out of a `Resources` — which is the composition
//!   `main::passive_float` performs and `scripts/lint-node-kinds.sh` exists to
//!   insist on;
//! * the server chip is a **`StatusVm` field**, not a `vcs` string standing in
//!   for one. The widget frame's own notes record that substitution — *"vcs is
//!   the statusline slot `7c` puts `rust-analyzer ✓` in … here it is a fixture
//!   string"* — and name `CP-4` as where it stops being one. This is that.
//!
//! # What is a fixture here and what is not
//!
//! The completion session is a fixture, deliberately: a golden frame may not
//! start a language server, and `crates/phosphor/tests/loop_pty.rs` is where a
//! real one answers a real keystroke. What is **not** a fixture is every
//! decision about the frame — the segment order, the shed ladder, the chip's
//! tone and the float's chrome are all read out of the shipped layer and the
//! shipped widgets, so a change to `runtime/statusline.scm` moves this frame.
//!
//! Owned by `spine`.

// `T018`'s golden-frame serialiser, from the crate that owns it.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::request::BufferId;
use phosphor_core::view::{
    Axis, Constraint, Float as ViewFloat, Millis, Mood, Node, SessionState, Slot, Tree,
};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, Cursor, StatusFile, StatusVm};
use phosphor_ui::buffer_view::{self, Editor, ScrollRequest, StateMark, apply_scroll, editor_area};
use phosphor_ui::float::{self, Anchor, CompletionItemVm, CompletionVm, SignatureVm};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// `7c`'s file. The same fixture the widget's frame uses, and for the reason
/// recorded there: the mockup draws a viewport at lines 30–33, so the rows
/// above are a plausible header for the file the picture is a view of.
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
const CURSOR_LINE: u32 = 31;

/// The first visible line, 1-based. `7c`'s gutter reads 30.
const TOP_LINE: u32 = 30;

/// Line 31 ahead of the cursor: the caret sits after `de`, and `de` is the word
/// the server is completing.
const PREFIX: &str = "    let policy = RetryPolicy::";

/// What `7c` draws in the chip: the server's own name and a tick.
///
/// The string is `main::server_chip`'s, for a `ServerState::Ready` whose
/// identity is rust-analyzer's — that function is unit-tested against all five
/// states and a pty test watches a real one change; what this frame adds is
/// *where the statusline puts it*, which is `runtime/statusline.scm`'s answer
/// and not Rust's.
const SERVER_CHIP: &str = "rust-analyzer ✓";

/// The host's frame, as [`Resources`]: one buffer and one completion session.
struct Painted {
    editor: Editor,
    completion: CompletionVm,
}

impl core::fmt::Debug for Painted {
    /// The vendored `Editor` implements none, and [`Resources`] requires one.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Painted").finish_non_exhaustive()
    }
}

impl Resources for Painted {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        Some(&self.editor)
    }

    fn state_marks(&self, _buffer: BufferId) -> &[StateMark] {
        // `7c` is "boring on purpose": no diagnostics, no unseen regions, no
        // agent. An empty column is the truthful one.
        &[]
    }

    fn completion(&self) -> Option<&CompletionVm> {
        Some(&self.completion)
    }

    fn signature(&self) -> Option<&SignatureVm> {
        // `main::passive_float` draws the completion list when both are live;
        // `7c` has only the list.
        None
    }
}

fn runtime_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// The shipped editor layer, booted clean.
fn layer() -> Runtime {
    let runtime = Runtime::boot(Some(&runtime_tree()), Arc::new(Detached) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    runtime
}

/// One row as the **mockup** draws it — see the twin of this function in
/// `crates/phosphor-ui/tests/screen_7c.rs` for why every field is spelled and
/// `..CompletionItemVm::default()` is refused: a struct-update took `T106`'s
/// two new columns silently, and left this frame byte-identical across the
/// change it exists to police.
fn item(label: &str, detail: &str) -> CompletionItemVm {
    CompletionItemVm {
        label: label.to_owned(),
        detail: Some(detail.to_owned()),
        kind: None,
        source: None,
        deprecated: false,
    }
}

/// `7c`'s list: the three rows a typed `de` leaves, and the selected row's one
/// line of prose.
///
/// **Three rows is the filter, not the server.** A server answers with
/// everything that could go at that position — `phosphor_buffer::lsp::narrow`
/// is what turns that into this, and it is the reason this float is four rows
/// tall instead of covering the screen.
fn session(anchor: Anchor, width: u16) -> CompletionVm {
    CompletionVm {
        items: vec![
            item("default()", "fn() -> RetryPolicy"),
            item("default_delay", "Duration"),
            item("deserialize", "fn(D) -> Result<Self>"),
        ],
        selected: 0,
        // **Wrapped the way the host wraps it**, which is what makes this the
        // binary's frame rather than the widget's. `Editing::completions` runs
        // the documentation through `float::wrap_prose` at
        // `float::anchored_wrap_cols` before it ever reaches a ViewModel, so a
        // fixture that handed the widget one long line would be a picture of a
        // code path the binary does not take — and at 80 columns it is the
        // difference between the mockup's sentence over two rows and the same
        // sentence cut off at `200ms⋯`.
        documentation: float::wrap_prose(
            &["Returns the policy with 3 attempts, 200ms base, 1s cap.".to_owned()],
            float::anchored_wrap_cols(width),
        ),
        anchor,
        // Content-sized: this frame is one answer, and the anti-thrash floor is
        // a fact about a session that has already been drawn wider.
        width_floor: 0,
    }
}

/// `7c`'s statusline ViewModel — insert mode, a dirty file, a ready server, and
/// nothing else. *"No agent anywhere."*
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "insert".to_owned(),
        surface: None,
        file: Some(StatusFile {
            path: PathBuf::from("src/fetch.rs"),
            dirty: true,
        }),
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        unseen: 0,
        vcs: None,
        server: Some(SERVER_CHIP.to_owned()),
        cursor: Some(Cursor {
            line: CURSOR_LINE,
            col: 34,
        }),
        hints: Vec::new(),
    }
}

/// The screen: the buffer, the composed statusline, and the passive float over
/// both — which is the tree `main::passive_float` builds.
fn tree(runtime: &mut Runtime) -> Tree {
    let status = status::compose(runtime, &status_vm()).expect("runtime/statusline.scm composes");
    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(
                Constraint::Fill { weight: 1 },
                Node::Buffer {
                    buffer: BufferId(1),
                    soft_wrap: false,
                },
            ),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
    .with_float(ViewFloat::new(Mood::Passive, Node::Completion {}))
}

/// What this frame is missing, and which task owns each absence.
const NOTES: &[&str] = &[
    "The statusline is composed by runtime/statusline.scm, not by a widget: the",
    "  segment order, the shed ladder and the chip's tone are all the editor",
    "  layer's. `rust-analyzer ✓` is a StatusVm field (T036) rather than the vcs",
    "  slot standing in for one, which is what the widget's own 7c frame records",
    "  as owed at CP-4.",
    "The completion session is a fixture here — a golden frame may not start a",
    "  language server. Three rows is what the typed prefix leaves; the filter",
    "  is phosphor_buffer::lsp::narrow, and loop_pty.rs presses the keys that",
    "  prove it against a real server process.",
    "7c draws `src/fetch.rs [+]` and the running editor draws the path as the",
    "  user typed it, absolute included. Repo-relative is phosphor-vcs's answer",
    "  (T071); this frame states the mockup's spelling because the ViewModel is",
    "  a path and the shed ladder's basename rung is what contracts it.",
    "The float's documentation rule is chrome.divider (#242a24); 7c specifies",
    "  #1d241d. One step apart on the same neutral ramp, and §4 hexes no",
    "  internal rule — recorded in float::Mood::rule, not folded in.",
    "The selection tint and the documentation rule cover the body's rows, which",
    "  sit two columns inside the border; 7c runs both edge to edge. A FloatBody",
    "  is handed an area inside the padding and must clip to it.",
    "The documentation is wrapped by the host, not by the widget: §11 is",
    "  \"nothing ever wraps\" and float::anchored_wrap_cols publishes the width",
    "  Editing::completions wraps to. At 120 columns the mockup's sentence fits",
    "  one row and nothing moves; at 80 the cap (ANCHORED_WIDTH_PCT) binds and",
    "  the same sentence takes two.",
    "No cursor: a golden frame is a Buffer, and the insert caret 7c draws after",
    "  `de` is the terminal's, placed by the host.",
    "Lines 1-29 of the fixture are not in any mockup — see the widget's frame,",
    "  which records the same judgement about the same file.",
];

/// The screen at `width`, drawn through the live layer.
fn screen(runtime: &mut Runtime, theme: &Theme, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let (body, _status) = split(area);

    let mut editor = Editor::new("rust", FETCH_RS, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    apply_scroll(
        &mut editor,
        ScrollRequest::ToRow { row: TOP_LINE },
        editor_area(body),
    );

    // The word being completed starts here: the code column, plus the prefix
    // ahead of it on the row. This is the arithmetic the host does to place the
    // cursor, which is why an `Anchor` is in screen cells.
    let anchor = Anchor::new(
        body.x
            + buffer_view::gutter_width(&editor)
            + u16::try_from(PREFIX.chars().count()).expect("a column"),
        body.y + u16::try_from(CURSOR_LINE - TOP_LINE).expect("a row"),
    );
    let painted = Painted {
        editor,
        completion: session(anchor, width),
    };

    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, &painted).render(&tree(runtime), area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`7c` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// Buffer area above, statusline on the last row — the same split the loop
/// makes.
const fn split(area: Rect) -> (Rect, Rect) {
    let body = Rect {
        height: area.height - 1,
        ..area
    };
    let status = Rect {
        y: area.y + body.height,
        height: 1,
        ..area
    };
    (body, status)
}

/// Commits one width as a golden frame.
fn golden(name: &'static str, width: u16) {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let buf = screen(&mut runtime, &theme, width, 24);
    let frame = Frame {
        screen: name,
        theme_label: "phosphor-dark",
        theme: &theme,
        notes: NOTES,
    };

    // §12's other half, which no grep lint can reach: a colour on screen that
    // no `Theme` field names.
    assert!(
        frame.unnamed(&buf).is_empty(),
        "colours on screen that no Theme field names: {:?}",
        frame.unnamed(&buf)
    );

    insta::assert_snapshot!(name, frame.to_text(&buf));
}

/// The screen, at the width the `CP-1` golden frames use.
#[test]
fn screen_7c_draws() {
    golden("7c", 120);
}

/// The same screen at 80 columns. The float is **anchored**, not centered, so
/// §11's full-width rule does not apply to it — what changes is that it slides
/// left along the edge rather than spilling off it, and that the statusline
/// sheds.
///
/// # And, since `CP-4`, that the width cap binds here
///
/// `float::ANCHORED_WIDTH_PCT` is 60, so this float stops at 48 columns where
/// the session asks for 61. **This capture is the only frame in the repo the
/// cap redraws** — the design draws `7c` at one width, 900px, and the
/// 120-column frame above it is byte-identical across the cap's introduction.
/// The mockup's documentation sentence is still drawn whole, over two rows
/// instead of one, because the host wraps prose to the columns the float will
/// give it (`Editing::wrapped`) and this fixture wraps the same way.
///
/// Flagged rather than folded in, per `CLAUDE.md`: this is a repo capture
/// disagreeing with a repo change, not the build disagreeing with a drawing.
#[test]
fn screen_7c_draws_at_80_columns() {
    golden("7c-80", 80);
}

/// **The chip is the layer's decision, not Rust's** — and this is the
/// assertion that keeps `7c`'s statusline from being a picture of a hardcoded
/// row.
///
/// `status-order-set!` is `statusline.scm`'s own idiom for reordering a side;
/// dropping the counters group takes the server chip with it, because the chip
/// is a member of that group rather than a segment Rust places.
#[test]
fn the_server_chip_is_a_segment_the_editor_layer_places() {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let before = rows(&screen(&mut runtime, &theme, 120, 24)).join("\n");
    assert!(before.contains(SERVER_CHIP), "{before}");

    let outcome = runtime.evaluate("(status-order-set! 'right '(session))");
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );

    let after = rows(&screen(&mut runtime, &theme, 120, 24)).join("\n");
    assert!(
        !after.contains(SERVER_CHIP),
        "the chip survived a statusline that no longer asks for its group:\n{after}"
    );
}

/// A buffer's rows as trimmed text.
fn rows(buf: &Buffer) -> Vec<String> {
    (buf.area.y..buf.area.bottom())
        .map(|y| {
            (buf.area.x..buf.area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}
