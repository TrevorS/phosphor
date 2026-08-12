//! `T022` — screen `6b` as a **Tier-1 golden frame**.
//!
//! `CP-2` names *"the `6b` snapshot"* among the things Claude verifies, and
//! Tier 1 is the committed cell grid — *"what we told the terminal to draw.
//! Exact, diffable, fast"* — which is the only tier that gates CI
//! (`TASKS.md`'s tier table). Before this file there were two weaker things and
//! no baseline: assertions in `phosphor-steel/tests/screen_6b.rs`, which prove
//! rows and glyphs but cannot see a palette regression, and
//! `tapes/artifacts/6b.png`, which is Tier 2 and is a change *detector*.
//!
//! # Why it lives in the binary crate
//!
//! `6b` is composed by Steel and drawn by the interpreter, so a test of it
//! needs `phosphor-steel` **and** `phosphor-ui` at once. `phosphor-ui` may not
//! have the first: `scripts/lint-no-store-mutation.sh` check 2 allows it
//! exactly one `phosphor-*` dependency and it is `phosphor-core`. The binary is
//! the only crate that sees both, which is the same reason `parity.rs` is here.
//!
//! The serialiser is `T018`'s, included by path rather than copied. One
//! serialiser means `6b` diffs against the `CP-1` frames in the same alphabet —
//! same colour key letters, same legend — and a second copy would drift from it
//! silently. `phosphor-ui` owns that file; this crate only reads it.
//!
//! # What this frame is, and is not
//!
//! It is the screen as `S2` truthfully draws it, refusals and all. `6b`'s
//! mockup answers four lines with `⇒ #ok · …`; three of them cannot answer that
//! until `T041` gives them a store and `T033` binds `goto`, and the per-line
//! table in `phosphor-steel/tests/screen_6b.rs` says which is which. Snapshotting
//! the truth is the point: when `T041` lands, this frame moves, and the diff is
//! the evidence that it did.

// `T018`'s golden-frame serialiser, from the crate that owns it. Not copied:
// see the module docs.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::Path;
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::view::Tree;
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::repl::Repl;
use phosphor_steel::runtime::Runtime;
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// `6b`'s session, line by line, exactly as the mockup types it.
///
/// The same four lines as `phosphor-steel/tests/screen_6b.rs`'s `TYPED`. Kept
/// as its own copy on purpose: that file asserts what each line *answers*, this
/// one asserts what the screen *looks like*, and a shared fixture would let one
/// of them be changed to suit the other.
const TYPED: &[&str] = &[
    r#"(unseen-regions "src/retry.rs")"#,
    r#"(map region-author (block-regions "retry logic"))"#,
    r#"(keymap-set! "]r" (lambda () (goto (next-region-by claude))))"#,
    r#"(watch-place "src/retry.rs:24" 'delay)"#,
];

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

/// `6b`'s REPL, with its four lines typed and answered.
fn session() -> (Repl, Runtime) {
    let mut runtime = layer();
    let mut repl = Repl::new();
    for source in TYPED {
        for character in source.chars() {
            repl.insert(character);
        }
        repl.submit(&mut runtime).expect("a form was typed");
    }
    (repl, runtime)
}

/// What this frame is missing, and which task owns each absence.
///
/// Goes into the `.snap` itself, on `T018`'s rule: *"nobody has to
/// reverse-engineer an absence."* Every line was checked against the tree in
/// the session that wrote it.
const NOTES: &[&str] = &[
    "`6b` draws `⇒ #ok · …` for all four lines. Three cannot answer that at",
    "  S2 and the fourth is a shape gap; the per-line table in",
    "  phosphor-steel/tests/screen_6b.rs says which is which.",
    "`unseen-regions` is registered and refuses, naming T041 — there is no",
    "  store to query (S5).",
    "`region-author`, `goto` and `claude` are unbound. They are editor-layer",
    "  names over records T041 returns; writing them now would invent the",
    "  record shape (T033, T041).",
    "`watch-place` resolves as an alias and then fails on shape: the row's",
    "  anchor is a Target and the mockup passes a string (T077).",
    "The λ prompt is drawn steel `#9ec98c`, not claude green. Design Language",
    "  line 53 draws `λ ◆` in steel captioned \"steel prompt\"; 6b draws it in",
    "  claude green. Teej ruled for the lexicon — the mockup is the bug.",
    "The footer promises `q close`, as drawn. `q` types and `esc` closes;",
    "  making the hint true needs modes (T026).",
];

/// Renders `tree` into a terminal-sized buffer.
fn render(tree: &Tree, theme: &Theme, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(theme, &NoResources).render(tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`6b` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// Commits one width as a golden frame.
fn golden(name: &'static str, width: u16) {
    let (repl, _runtime) = session();
    let theme = Theme::phosphor_dark();
    let buf = render(&repl.frame(), &theme, width, 24);
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
fn screen_6b_draws() {
    golden("6b", 120);
}

/// The same screen at 80 columns — the width the shed ladder is written for,
/// and the one nearly every checkpoint names.
#[test]
fn screen_6b_draws_at_80_columns() {
    golden("6b-80", 80);
}
