//! `T034` — screen `3c` (the leader popup) as a **Tier-1 golden frame**.
//!
//! `CP-3` names *"the `3c` snapshot"* among the things Claude verifies, and
//! Tier 1 is the committed cell grid — *"what we told the terminal to draw.
//! Exact, diffable, fast"* — which is the only tier that gates CI
//! (`TASKS.md`'s tier table).
//!
//! # Everything on this screen is live
//!
//! The six rows are **not** a fixture. They are read out of
//! `runtime/keymaps.scm` through `phosphor_steel::keymap::entries`, which is
//! the same call the widget's product path makes and the same table a
//! keystroke resolves against — so `a_repl_rebind_shows_up_in_the_leader_popup`
//! below rebinds one at the REPL and the frame moves. The statusline is
//! `runtime/statusline.scm`'s, composed by the editor layer (`T025`). The only
//! Rust in the composition is the split itself and the strip's height.
//!
//! # Why it lives in the binary crate
//!
//! Same reason as `screen_6b.rs`: a frame composed from Steel and drawn by the
//! interpreter needs `phosphor-steel` **and** `phosphor-ui` at once, and
//! `phosphor-ui` may not have the first — `scripts/lint-no-store-mutation.sh`
//! check 2 allows it exactly one `phosphor-*` dependency. The serialiser is
//! `T018`'s, included by path rather than copied, so this frame diffs against
//! the `CP-1` frames in the same alphabet.
//!
//! # What this frame is, and is not
//!
//! It is `3c` as `S3` truthfully draws it. Three things the mockup has that the
//! build does not, each named in the snapshot's own notes: the amber `SPC
//! pending` statusline segment (no `StatusVm` field carries it), the dim behind
//! the strip (`3c` draws the code at `#232823`; the dim is the float
//! primitive's and the strip is not a float), and the strip's height, which
//! this test computes in Rust because a composer has no width to compute it
//! from. See the report for the contract request behind the third.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see the module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::request::BufferId;
use phosphor_core::view::{
    Axis, Constraint, Density, KeyHint, Millis, Node, SessionState, Slot, Tree,
};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::keymap;
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, StatusFile, StatusVm};
use phosphor_ui::buffer_view::{self, Editor, ScrollRequest, StateMark, apply_scroll, editor_area};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::key_hints::KeyHints;
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// The leader, as `keymap-entries` spells it. `keymaps.scm`: *"`SPC` is
/// `<space>`, and a space is a separator rather than a key."*
const LEADER: &str = "<space>";

/// `3c`'s file, whole — the mockup draws its first five lines under the strip.
///
/// It closes the item the viewport cuts through, for the reason
/// `phosphor-ui/tests/golden_frames.rs` records at length: a fixture that stops
/// where the *picture* stops does not parse, and the captures on the rows that
/// *are* drawn change.
const RETRY_RS: &str = "\
pub fn retry_with_backoff<T, E>(
    mut op: impl FnMut() -> Result<T, E>,
    policy: &RetryPolicy,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => thread::sleep(jitter(delay)),
        }
        delay = (delay * 2).min(policy.max_delay);
    }
    Err(last.unwrap())
}
";

/// The one buffer this screen has.
///
/// The `Debug` is hand-written because the vendored `Editor` has none and
/// [`Resources`] requires one — the trait wants a host that can say what it is
/// holding, not the whole rope.
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
        // `3c` draws no gutter markers; the store that would answer is `T041`.
        &[]
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

/// What `SPC` opens: every binding one key under the leader, in the order
/// `keymaps.scm` declares them.
///
/// This is which-key's whole question — *what is bound under what I have
/// typed* — asked of the live table. A group (`SPC c`) and a leaf (`SPC t`)
/// read the same here; the widget draws the difference, out of the verb.
fn leader_hints(runtime: &mut Runtime) -> Vec<KeyHint> {
    keymap::entries(runtime)
        .iter()
        .filter(|entry| entry.scope == "normal")
        .filter(|entry| {
            entry.keys.0.strip_prefix(LEADER).is_some_and(|rest| {
                // One key beyond the leader, and not a bracketed chord.
                rest.chars().count() == 1 && !rest.starts_with('<')
            })
        })
        .map(keymap::Entry::hint)
        .collect()
}

/// `3c`'s statusline ViewModel: normal mode, the file, four unseen regions.
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: None,
        file: Some(StatusFile {
            path: PathBuf::from("src/retry.rs"),
            dirty: false,
        }),
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        unseen: 4,
        // No diagnostic on this screen. `2b` is the one that
        // draws `■ N`, and it has no golden frame.
        trouble: 0,
        attention: 0,
        vcs: None,
        server: None,
        cursor: None,
        hints: Vec::new(),
    }
}

/// The screen: code, the leader strip, the statusline.
///
/// The strip's height comes from the widget, because at `Density::Grid` it
/// depends on the width — the grid packs into as many columns as fit. A
/// composer in Steel has no width, so this is the one number Rust supplies;
/// `key_hints.rs` records the seam and the report asks `spine` for the
/// constraint that would close it.
fn tree(runtime: &mut Runtime, theme: &Theme, width: u16) -> Tree {
    let hints = leader_hints(runtime);
    assert!(
        !hints.is_empty(),
        "the leader tree is `T033`'s and it ships"
    );
    let strip = KeyHints::new(&hints, Density::Grid, theme).desired_height(width);
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
            Slot::new(
                Constraint::Cells {
                    cells: u32::from(strip),
                },
                Node::KeyHints {
                    density: Density::Grid,
                    hints,
                },
            ),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
}

/// What this frame is missing, and which task owns each absence.
///
/// Goes into the `.snap` itself, on `T018`'s rule: *"nobody has to
/// reverse-engineer an absence."* Every line was checked against the tree in
/// the session that wrote it.
const NOTES: &[&str] = &[
    "The six rows are read live from runtime/keymaps.scm through",
    "  keymap::entries — not a fixture. A REPL rebind moves this frame, which",
    "  is what a_repl_rebind_shows_up_in_the_leader_popup asserts.",
    "3c draws the code behind the strip dimmed (#232823). Dimming is the float",
    "  primitive's, and §9 defines it as \"behind a float\"; the leader strip is",
    "  a row slot, not a float — it has no header, no internal rules and no",
    "  footer, which is the chrome Float would impose. Flagged, not folded in.",
    "3c's statusline carries an amber `SPC pending` segment. No StatusVm field",
    "  says a leader sequence is half-typed, so the composed statusline cannot",
    "  draw one (T026 owns the machine that would know; the field is spine's).",
    "The strip's height is computed in Rust: at Density::Grid the row count",
    "  depends on the width, and a Steel composer has no width. See the report.",
    "No gutter markers and no session segment: both are store/session queries",
    "  (T041, T050), and 3c draws neither.",
];

/// Renders `tree` into a terminal-sized buffer over `resources`.
fn render(tree: &Tree, theme: &Theme, resources: &dyn Resources, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, resources).render(tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`3c` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// The screen at `width`, drawn through the live layer.
fn screen(runtime: &mut Runtime, theme: &Theme, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut editor = Editor::new("rust", RETRY_RS, Vec::new()).expect("rust editor");
    buffer_view::configure(&mut editor, theme);
    // Row 1, not row 0: `ScrollRequest` is `phosphor_core::request`'s now
    // (`R7-ScrollRequest`), and the vocabulary counts visual rows from 1
    // because a person types them. The top of the file is what this asks for.
    apply_scroll(
        &mut editor,
        ScrollRequest::ToRow { row: 1 },
        editor_area(area),
    );
    let resources = OneBuffer(editor);
    let tree = tree(runtime, theme, width);
    render(&tree, theme, &resources, area)
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
fn screen_3c_draws() {
    golden("3c", 120);
}

/// The same screen at 80 columns — where the grid gives up columns rather than
/// entries (§11: *"narrow terminals drop, never squeeze"*), so every binding is
/// still on screen.
#[test]
fn screen_3c_draws_at_80_columns() {
    golden("3c-80", 80);
}

/// `T034`'s second acceptance half: *"a REPL rebind shows up in it."*
///
/// No reload, no invalidation, no second boot — the popup is a function of the
/// table, and the table is what the REPL just changed.
#[test]
fn a_repl_rebind_shows_up_in_the_leader_popup() {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let before = rows(&screen(&mut runtime, &theme, 120, 24));
    assert!(
        before.iter().any(|row| row.contains("jj timeline")),
        "the shipped leader draws `SPC j`:\n{}",
        before.join("\n")
    );

    // Two rebinds, both of the kind `6b` types: one that renames an existing
    // leaf, one that adds a key nothing was bound to.
    let outcome = runtime.evaluate(
        r#"(keymap-set! "SPC j" (key/run (key/cmd "open-timeline")) ":timeline — agent turns are changes")"#,
    );
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );
    let outcome = runtime.evaluate(r#"(keymap-set! "SPC g" (lambda () 1) "grep")"#);
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );

    let after = rows(&screen(&mut runtime, &theme, 120, 24));
    let drawn = after.join("\n");
    assert!(
        drawn.contains(":timeline — agent turns are changes"),
        "the rebound verb is on screen:\n{drawn}"
    );
    assert!(
        !drawn.contains("jj timeline"),
        "and the old one is not:\n{drawn}"
    );
    assert!(drawn.contains("g  grep"), "a new binding appears:\n{drawn}");
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
