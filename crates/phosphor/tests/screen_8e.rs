//! `T035` — screen `8e` (the unknown-key hint) as a **Tier-1 golden frame**.
//!
//! `CP-3` names *"the `8e` snapshot"* among the things Claude verifies, and
//! Tier 1 is the committed cell grid — *"what we told the terminal to draw.
//! Exact, diffable, fast"* — which is the only tier that gates CI
//! (`TASKS.md`'s tier table).
//!
//! # Both halves are frames, because both halves are the task
//!
//! `T035` is *"one virtual-text line naming `SPC` and `:help`, once per
//! session, never again"*, and `CP-3`'s VHS list asks for the hint **firing and
//! then not firing again**. So the negative case is captured here too:
//! [`screen_8e_teaches_nothing_the_second_time`] draws the identical screen
//! after the session's one hint has been spent, and the `┊` row is gone from
//! the grid. A latch that leaked would show up as a diff rather than as a
//! silence.
//!
//! # Why it lives in the binary crate
//!
//! Same reason as `screen_3c.rs` and `screen_6b.rs`: the statusline is composed
//! by `runtime/statusline.scm`, so the frame needs `phosphor-steel` **and**
//! `phosphor-ui` at once, and `phosphor-ui` may not have the first —
//! `scripts/lint-no-store-mutation.sh` check 2 allows it exactly one
//! `phosphor-*` dependency. The serialiser is `T018`'s, included by path rather
//! than copied, so this frame diffs against the `CP-1` frames in the same
//! alphabet.
//!
//! # What this frame is, and is not
//!
//! `8e` draws five buffer rows — a collapsed fold, a blank, a softly wrapped
//! comment and a line with insert-only whitespace marks — and the hint under
//! them. Four things differ from the drawing, each named in the snapshot's own
//! notes and each checked against the tree in this session: the second item has
//! to be a real declaration for the fixture to parse, the comment's wrap point
//! is width-driven rather than drawn, a wrapped comment does not repeat its
//! `//`, and the `← INSERT only` beside the whitespace marks is the mockup
//! annotating itself, not a cell on screen.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see the module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::request::{BufferId, KeySeq};
use phosphor_core::view::{Axis, Constraint, Millis, Node, SessionState, Slot, Tree};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, Cursor, StatusFile, StatusVm};
use phosphor_ui::buffer_view::{
    self, Editor, ScrollRequest, StateMark, apply_scroll, editor_area, gutter_width,
};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::soft_wrap::{self, EditMode};
use phosphor_ui::theme::Theme;
use phosphor_ui::unknown_key::{self, UnknownKeyHint};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// `8e`'s file — `src/fetch.rs`, whole.
///
/// Three of its line numbers are load-bearing, because the mockup draws them:
///
/// * **12** opens `retry_with_backoff`, whose fold hides **13 lines** — the
///   fork prints `end_line - start_line` (`view.rs`, `code_fold_hidden_lines`),
///   so the closing brace has to land on 25 and does.
/// * **27** is the long comment that wraps.
/// * **28** is the line with two trailing spaces, marked in INSERT only.
///
/// **26 is a declaration where `8e` draws a blank line, and that is deliberate.**
/// The mockup's line 28 is indented four cells, so it is inside a block; its
/// line 12 is a fold that ends before 26; and a fold's hidden run is contiguous
/// from its header, so no single fold can both end at 25 and leave 28 inside
/// its own function. Something has to open a block between 25 and 28, and
/// `golden_frames.rs`'s rule is that a fixture parses or the captures on the
/// rows that *are* drawn change. It opens on 26, which costs the blank and
/// keeps 27 and 28 exactly where `8e` puts them.
const FETCH_RS: &str = "\
use std::time::Duration;
use reqwest::Response;

use crate::error::FetchError;

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
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap())
}
pub async fn decode(resp: Response) -> Result<Payload, FetchError> {
// long doc comment wraps softly and the continuation row carries no line number — the gutter stays honest
    resp.json().await.map_err(FetchError::Decode)
}
";

/// The two trailing spaces `8e` marks on line 28, and the text they follow.
///
/// Added at runtime rather than written into [`FETCH_RS`]: trailing whitespace
/// inside a source literal is exactly the thing an editor, a formatter or a
/// pre-commit hook strips without asking, and a fixture whose point is trailing
/// whitespace cannot be one that tooling may silently repair.
const MARKED_LINE: &str = "    resp.json().await.map_err(FetchError::Decode)";

/// The fold `8e` draws collapsed, 0-based: `pub fn retry_with_backoff<T, E>(`.
const FOLD_HEADER: usize = 11;

/// Lines it hides — the `13` in `▸⋯ 13 lines`.
const HIDDEN_LINES: usize = 13;

/// The key that missed, as `8e` draws it.
const MISSED: &str = "gq";

/// The one buffer this screen has.
///
/// The `Debug` is hand-written because the vendored `Editor` has none and
/// [`Resources`] requires one.
struct OneBuffer(Editor);

impl core::fmt::Debug for OneBuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OneBuffer").finish_non_exhaustive()
    }
}

impl Resources for OneBuffer {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        Some(&self.0)
    }

    fn state_marks(&self, _buffer: BufferId) -> &[StateMark] {
        // `8e` draws no gutter markers; the store that would answer is `T041`.
        &[]
    }
}

/// The shipped editor layer, booted clean.
fn layer() -> Runtime {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime");
    let runtime = Runtime::boot(Some(&root), Arc::new(Detached) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    runtime
}

/// `8e`'s statusline: normal mode, the file, an idle session, the cursor on 27.
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: None,
        file: Some(StatusFile {
            path: PathBuf::from("src/fetch.rs"),
            dirty: false,
        }),
        session: SessionState::Idle,
        since: None::<Millis>,
        ask_pending: false,
        unseen: 0,
        vcs: None,
        cursor: Some(Cursor { line: 27, col: 1 }),
        hints: Vec::new(),
    }
}

/// `8e`'s buffer: the fold collapsed, the whitespace marks on, wrapped to the
/// screen, scrolled so the fold header is the top row.
fn buffer(theme: &Theme, area: Rect) -> Editor {
    let source = FETCH_RS.replace(&format!("{MARKED_LINE}\n"), &format!("{MARKED_LINE}  \n"));
    assert!(
        source.contains("Decode)  \n"),
        "line 28 has to carry the two trailing spaces `8e` marks"
    );
    let mut editor = Editor::new("rust", &source, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    // `T016`'s three details and `T081`'s wrap, in the order `soft_wrap.rs`
    // requires: `configure` after `buffer_view::configure`, never before.
    soft_wrap::configure(&mut editor, theme);
    soft_wrap::set_mode(&mut editor, EditMode::Insert);
    assert!(
        editor.toggle_fold_at_line(FOLD_HEADER),
        "line {} opens a fold",
        FOLD_HEADER + 1
    );
    assert_eq!(
        editor.fold_hidden_lines(FOLD_HEADER),
        Some(HIDDEN_LINES),
        "`8e` draws `▸⋯ {HIDDEN_LINES} lines`"
    );
    let inner = editor_area(area);
    soft_wrap::wrap_to(&mut editor, inner);
    apply_scroll(&mut editor, ScrollRequest::ToRow(FOLD_HEADER), inner);
    editor
}

/// The screen: the code, `8e`'s hint strip when there is one, the statusline.
///
/// **The hint has no slot of its own when it is not being shown.** A session
/// that has spent its hint composes a two-slot split, so the row the strip
/// would have taken goes back to the buffer — which is what "never again" looks
/// like on screen.
fn tree(runtime: &mut Runtime, hint: Option<Node>, gutter: u16) -> Tree {
    let status = status::compose(runtime, &status_vm()).expect("runtime/statusline.scm composes");
    let mut slots = vec![Slot::new(
        Constraint::Fill { weight: 1 },
        Node::Buffer {
            buffer: BufferId(1),
            soft_wrap: true,
        },
    )];
    if let Some(hint) = hint {
        slots.push(Slot::new(
            Constraint::Cells { cells: 1 },
            unknown_key::strip(hint, gutter),
        ));
    }
    slots.push(Slot::new(Constraint::Cells { cells: 1 }, status));
    Tree::new(Node::split(Axis::Rows, slots))
}

/// What this frame is missing, and which task owns each absence.
///
/// Goes into the `.snap` itself, on `T018`'s rule: *"nobody has to
/// reverse-engineer an absence."* Every line was checked against the tree in
/// the session that wrote it.
const NOTES: &[&str] = &[
    "The hint is the once-per-session unknown-key line (T035). It is a",
    "  Node::VirtualText with no owner — `8e` sets it off from the code with",
    "  its own padding, so it is a strip above the statusline rather than a",
    "  row installed in the buffer's stream.",
    "8e draws line 26 blank. The fixture declares there instead: line 28 is",
    "  indented, so it is inside a block, and a fold's hidden run is contiguous",
    "  from its header — nothing can both end the fold at 25 and leave 28 inside",
    "  the same function. See FETCH_RS.",
    "8e wraps line 27 after `the continuation row`. Wrapping here is",
    "  width-driven (T081), so at 120 columns the comment fits on one row and",
    "  no `↪` is drawn; at 80 it wraps where the text column runs out.",
    "8e's continuation row repeats `// `. A soft wrap is a wrap, not a re-emit:",
    "  the continuation carries the next words of the same comment and the `↪`",
    "  stands in for the line number (T081).",
    "8e writes `← INSERT only` beside the whitespace marks. That is the mockup",
    "  annotating itself; the cells on screen are the two `··` marks, which are",
    "  drawn because the buffer is in INSERT (T016).",
    "No gutter markers and no session counters: both are store queries (T041),",
    "  and 8e draws neither.",
];

/// Renders `tree` into a terminal-sized buffer over `resources`.
fn render(tree: &Tree, theme: &Theme, resources: &dyn Resources, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, resources).render(tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`8e` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// The screen at `width`, for a session in whatever state `session` is in.
///
/// The session is threaded in rather than made here, so the second frame is the
/// *same* composition asked a second time — the only difference between the two
/// snapshots is that the latch has already answered once.
fn screen(
    runtime: &mut Runtime,
    session: &mut UnknownKeyHint,
    theme: &Theme,
    width: u16,
    height: u16,
) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let resources = OneBuffer(buffer(theme, area));
    let hint = session.teach(&KeySeq(MISSED.to_owned()));
    let tree = tree(runtime, hint, gutter_width(&resources.0));
    render(&tree, theme, &resources, area)
}

/// Commits one width as a golden frame.
///
/// Seven rows: `8e`'s five buffer rows, its hint, its statusline. Sized to the
/// drawing the way `golden_frames.rs` sizes `8c` to its own six, because a
/// mockup is a viewport onto a file and padding it out with `~` rows would
/// compare the frame against something nobody drew.
fn golden(name: &'static str, width: u16, session: &mut UnknownKeyHint) {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let buf = screen(&mut runtime, session, &theme, width, 7);
    let frame = Frame {
        screen: name,
        theme_label: "phosphor-dark",
        theme: &theme,
        notes: NOTES,
    };

    // §12, and the half of it no grep-based lint can reach: a colour on screen
    // that is not a `Theme` field. `CP-1` asserts this on every golden frame.
    assert!(
        frame.unnamed(&buf).is_empty(),
        "colours on screen that no Theme field names: {:?}",
        frame.unnamed(&buf)
    );

    insta::assert_snapshot!(name, frame.to_text(&buf));
}

/// The screen, at the width the `CP-1` golden frames use.
#[test]
fn screen_8e_draws() {
    golden("8e", 120, &mut UnknownKeyHint::new());
}

/// The same screen at 80 columns — where the comment on line 27 runs out of
/// text column and takes its `↪` continuation, which is what `8e` draws.
#[test]
fn screen_8e_draws_at_80_columns() {
    golden("8e-80", 80, &mut UnknownKeyHint::new());
}

/// **`T035`'s second half, as a frame.** The same session, asked again: the
/// hint is gone, the buffer has the row back, and nothing else on screen moved.
#[test]
fn screen_8e_teaches_nothing_the_second_time() {
    let mut session = UnknownKeyHint::new();
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();

    let first = rows(&screen(&mut runtime, &mut session, &theme, 120, 7));
    assert!(
        first.iter().any(|row| row.contains("┊ unknown key gq")),
        "the first unknown key teaches:\n{}",
        first.join("\n")
    );
    assert!(session.is_spent());

    golden("8e-taught", 120, &mut session);
}

/// And the row is gone whatever the next key is — the latch is the session's,
/// not the key's.
#[test]
fn a_different_key_does_not_get_a_second_hint() {
    let mut session = UnknownKeyHint::new();
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();

    let _ = screen(&mut runtime, &mut session, &theme, 120, 7);
    for next in ["gq", "zz", "<C-q>"] {
        let hint = session.teach(&KeySeq(next.to_owned()));
        assert!(hint.is_none(), "{next} taught a second time");
    }

    let after = rows(&screen(&mut runtime, &mut session, &theme, 120, 7));
    assert!(
        !after.iter().any(|row| row.contains('┊')),
        "no rail survives the latch:\n{}",
        after.join("\n")
    );
}

/// The fold, the wrap and the whitespace marks `8e` draws, asserted rather than
/// only snapshotted — a snapshot says *what changed*, and these say *what it
/// is*.
#[test]
fn the_screen_draws_8es_three_text_details() {
    let mut session = UnknownKeyHint::new();
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let narrow = rows(&screen(&mut runtime, &mut session, &theme, 80, 7));

    assert!(
        narrow[0].contains("pub fn retry_with_backoff<T, E>(")
            && narrow[0].ends_with("▸⋯ 13 lines"),
        "the collapsed fold marks its header inline:\n{}",
        narrow.join("\n")
    );
    assert!(
        narrow.iter().any(|row| row.trim_start().starts_with('↪')),
        "line 27 wraps at 80 columns:\n{}",
        narrow.join("\n")
    );
    assert!(
        narrow.iter().any(|row| row.contains("··")),
        "the trailing spaces are marked in INSERT:\n{}",
        narrow.join("\n")
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
