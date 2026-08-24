//! `T025` and `T080` — composition end to end: scheme in, cells out.
//!
//! # Why this lives in the binary crate
//!
//! The same reason `parity.rs` does. `phosphor-steel` composes and cannot draw
//! — the Steel barrier is that it never sees a renderer
//! (`scripts/lint-the-steel-barrier.sh`) — and `phosphor-ui` draws and cannot
//! compose, because a UI crate's only `phosphor-*` dependency is
//! `phosphor-core`. The binary depends on both, so this is the only place the
//! whole of Q12 is visible at once: *Steel decides which pixels, Rust produces
//! them.*
//!
//! Everything here goes through the shipped editor layer in `runtime/`, not
//! through a fixture. A composition the tests wrote themselves would prove the
//! interpreter and nothing about the editor.
//!
//! # What each half is for
//!
//! * **`T025`** — the statusline is composed in `runtime/statusline.scm`, and
//!   what it draws is Design Language §5 and §11 (as `CP-1` amended them),
//!   asserted against the same rows `phosphor-ui`'s widget tests assert. Then
//!   the whole composition is redefined the way a person would type it, and the
//!   next frame is different.
//! * **`T080`** — a surface the primitive set does not cover, built out of the
//!   `spans` hatch alone, from scheme, with no Rust change and no node kind of
//!   its own. That is `:arch`'s mechanism (`T048`, S5) exercised at S2 by the
//!   surface that can be built now.
//!
//! Owned by `spine`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_core::registry::ParamType;
use phosphor_core::request::KeySeq;
use phosphor_core::value::Wire;
use phosphor_core::view::{KeyHint, Millis, Node, SessionState, SpanRow, Tree};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{Cursor, StatusFile, StatusVm, compose};
use phosphor_steel::view;
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// ---------------------------------------------------------------------------
// The editor layer, booted
// ---------------------------------------------------------------------------

/// The layer this repository ships — `runtime/`, two directories up.
///
/// Behind [`Detached`]: composition reads ViewModels and emits no Actions, so
/// there is nothing here for a store to carry out.
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

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Draws `node` as one row `width` wide and reads the row back as text.
///
/// **Not trimmed**: the last column is the statusline's right margin, and a
/// margin that is not there is exactly the kind of difference this has to see.
fn row_of(node: &Node, width: u16) -> String {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, width, 1);
    let mut buf = Buffer::empty(area);
    let report =
        Interpreter::new(&theme, &NoResources).render(&Tree::new(node.clone()), area, &mut buf);
    assert!(report.deferred.is_empty(), "{report:?}");
    (0..width).map(|x| buf[(x, 0)].symbol()).collect()
}

/// Draws `node` and answers its rows as text, plus the cells themselves.
fn drawn(node: &Node, width: u16, height: u16) -> (Vec<String>, Buffer) {
    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let report =
        Interpreter::new(&theme, &NoResources).render(&Tree::new(node.clone()), area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "this surface needs a primitive that does not exist: {:?}",
        report.deferred
    );
    let rows = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    (rows, buf)
}

// ---------------------------------------------------------------------------
// T025 — the statusline
// ---------------------------------------------------------------------------

/// The ViewModel `9c` draws.
fn screen_9c() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        file: Some(StatusFile {
            path: PathBuf::from("src/retry.rs"),
            dirty: true,
        }),
        session: SessionState::Idle,
        unseen: 6,
        vcs: Some("jj ✓".to_owned()),
        cursor: Some(Cursor { line: 12, col: 1 }),
        ..StatusVm::default()
    }
}

#[test]
fn the_composed_statusline_draws_screen_9c() {
    // The same row `phosphor-ui`'s `screen_9c_reproduces_at_width` asserts of
    // the widget — the mode chip, the file and its flag, then the right-hand
    // group with a plain gap at the session seam and bars only inside the
    // counter group (§5, as `CP-1` settled it against §5's own prose).
    let mut runtime = layer();
    let line = compose(&mut runtime, &screen_9c()).expect("the shipped layer composes");
    let drawn = row_of(&line, 120);

    assert!(drawn.starts_with(" NORMAL  src/retry.rs [+]"), "{drawn:?}");
    assert!(
        drawn.ends_with("✻ claude idle 6 unseen │ jj ✓ │ 12:1 "),
        "{drawn:?}"
    );
    assert!(
        !drawn.contains("claude idle │"),
        "no mockup draws a bar at the session seam: {drawn:?}"
    );
}

#[test]
fn the_composed_statusline_sheds_in_section_11_order() {
    // The ladder as widths, the way `T018`'s `shed-ladder` snapshot reads it:
    // for each thing the ladder gives up, the narrowest width that still shows
    // it. Shedding is fit-driven, so those thresholds have to fall in exactly
    // §11's order.
    let mut runtime = layer();
    let line = compose(&mut runtime, &screen_9c()).expect("composes");

    let at = |width: u16| row_of(&line, width);
    let threshold = |needle: &str| -> u16 {
        (4u16..=200)
            .find(|width| at(*width).contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} is never drawn at any width"))
    };

    let ladder = [
        ("6 unseen", threshold("6 unseen")),
        ("jj ✓", threshold("jj ✓")),
        ("12:1", threshold("12:1")),
        ("claude idle", threshold("claude idle")),
        ("NORMAL", threshold("NORMAL")),
        ("src/", threshold("src/")),
        ("[+]", threshold("[+]")),
        ("retry.rs", threshold("retry.rs")),
    ];
    for pair in ladder.windows(2) {
        let ((first, wide_at), (then, narrow_at)) = (pair[0], pair[1]);
        assert!(
            wide_at > narrow_at,
            "{first} must shed before {then}, but they go at {wide_at} and {narrow_at}"
        );
    }

    // §11 + Q9: what is left at the bottom of the ladder is the chip and the
    // last-standing set. The words are gone; the glyphs are not.
    let floor = at(ladder[3].1 - 1);
    assert!(
        floor.contains('✻') && !floor.contains("claude idle"),
        "{floor:?}"
    );
    assert!(floor.contains("●6"), "{floor:?}");
}

#[test]
fn the_last_standing_set_survives_every_width() {
    let mut runtime = layer();
    let vm = StatusVm {
        file: Some(StatusFile {
            path: PathBuf::from("a/very/deeply/nested/path/that/will/never/fit.rs"),
            dirty: true,
        }),
        ask_pending: true,
        threads: 0,
        inbox_unread: 0,
        ..screen_9c()
    };
    let line = compose(&mut runtime, &vm).expect("composes");
    for width in 24..=200u16 {
        let drawn = row_of(&line, width);
        assert!(drawn.contains('✻'), "width {width}: {drawn:?}");
        assert!(
            drawn.contains("●6") || drawn.contains("6 unseen"),
            "width {width}: {drawn:?}"
        );
        assert!(drawn.contains('!'), "width {width}: {drawn:?}");
        assert!(
            drawn.starts_with(" N"),
            "§5: the chip is always visible — width {width}: {drawn:?}"
        );
    }
}

/// **The sweep's 80-column claim, with the diagnostic counters present.**
///
/// `docs/TASKS.md`'s recurring sweep asks the same question at every checkpoint
/// — *"Nothing wraps. A second statusline row is a bug"* — and the fixture
/// every other test in this file uses (`screen_9c`) takes
/// `..StatusVm::default()`, so `trouble` and `attention` are **zero** in all of
/// them. They were zero in every width test in the repository on the day the
/// counters were added, which means the segments this build gained at `CP-4`
/// had no width coverage at all.
///
/// **They are also the only counters that cannot contract.** `unseen` sheds
/// `6 unseen` → `●6` on §11's first rung; `runtime/statusline.scm` passes
/// `void` for *both* forms of the diagnostic counters deliberately — `■ 3`
/// needs no noun — so the rung is a no-op for them and they occupy their cells
/// at every width down to the floor. That is the shape worth a test rather
/// than a comment: two segments that never shrink, on a row that must stay one
/// row.
///
/// Eleven is `CP-4`'s own number, from the cascade that started all of this.
#[test]
fn the_diagnostic_counters_never_take_a_second_row() {
    let mut runtime = layer();
    let vm = StatusVm {
        trouble: 11,
        attention: 3,
        ..screen_9c()
    };
    let line = compose(&mut runtime, &vm).expect("composes");

    for width in 24..=200u16 {
        let drawn = row_of(&line, width);
        assert!(
            !drawn.contains('\n'),
            "§11: nothing wraps — width {width}: {drawn:?}"
        );
        // The chip is §5's floor and outlives every rung, counters included.
        assert!(
            drawn.starts_with(" N"),
            "width {width}: the chip went — {drawn:?}"
        );
    }

    // At width the count is drawn, and it is the file's whole set rather than
    // whatever the rows happened to show (`RowPolicy` bounds those).
    //
    // **80 and 120 are asserted by name**, not just "some wide width": 80 is
    // the sweep's own number and 120 is what `loop_pty.rs` gives its terminal,
    // so a count that survived only at 200 would be a count nobody ever sees.
    for width in [80u16, 120, 200] {
        let drawn = row_of(&line, width);
        assert!(
            drawn.contains("■11"),
            "the trouble count is gone at {width}: {drawn:?}"
        );
        assert!(
            drawn.contains("■3"),
            "the attention count is gone at {width}: {drawn:?}"
        );
    }

    // And they shed with the counter group rather than outliving it: `unseen`
    // is on the same rung, so a width that has dropped `●6` has dropped these
    // too. Read off the composition rather than assumed — the alternative is a
    // row whose last surviving segment is a number nobody can act on.
    let floor = (4u16..=200)
        .find(|width| row_of(&line, *width).contains("●6"))
        .expect("the unseen counter is drawn at some width");
    let below = row_of(&line, floor - 1);
    assert!(
        !below.contains("■11") && !below.contains("■3"),
        "the diagnostic counters outlived the counter rung at width {}: {below:?}",
        floor - 1
    );
}

#[test]
fn a_working_session_animates_without_being_recomposed() {
    // The composed tree carries a *mark*, not a rendered string, so the elapsed
    // counter and the spinner move from one composition (Q12, `FrameCache`).
    let mut runtime = layer();
    let vm = StatusVm {
        session: SessionState::Working,
        since: Some(Millis(0)),
        ..StatusVm::default()
    };
    let line = compose(&mut runtime, &vm).expect("composes");

    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 60, 1);
    let mut seen = Vec::new();
    for frame in 0..4u64 {
        let mut buf = Buffer::empty(area);
        Interpreter::new(&theme, &NoResources)
            .at(Millis(frame * 1_000))
            .render(&Tree::new(line.clone()), area, &mut buf);
        seen.push(
            (0..area.width)
                .map(|x| buf[(x, 0)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned(),
        );
    }
    assert!(seen[0].contains("claude working"), "{:?}", seen[0]);
    assert!(seen[3].contains("0:03"), "{:?}", seen[3]);
    assert_ne!(seen[0], seen[3], "one composition, four different frames");
}

#[test]
fn redefining_the_whole_composition_changes_the_next_frame() {
    // `T025`'s acceptance criterion, drawn: type a new composition, draw again,
    // read the row. No reload, no restart, nothing invalidated — the editor
    // layer *is* the statusline.
    let mut runtime = layer();
    let before = row_of(&compose(&mut runtime, &screen_9c()).expect("composes"), 120);
    assert!(before.contains("6 unseen"), "{before:?}");

    runtime
        .eval(
            r#"(define (phosphor/status-line vm)
                 (view/line (list (view/mode-chip "PHOSPHOR" 'you)
                                  (view/spring)
                                  (view/label "one line, redefined live" 'steel 'plain)
                                  (view/spacer 1))))"#,
        )
        .expect("a redefinition is an ordinary form");

    let after = row_of(&compose(&mut runtime, &screen_9c()).expect("composes"), 120);
    assert!(after.starts_with(" PHOSPHOR "), "{after:?}");
    assert!(after.ends_with("one line, redefined live "), "{after:?}");
}

#[test]
fn replacing_one_segment_leaves_the_rest_of_the_line_alone() {
    // The other half of the same claim: the composition is a set of *named*
    // decisions, so a person can replace the chip without knowing how the rest
    // of the line is assembled. `status-set!` rather than a second `define`,
    // for the reason `runtime/statusline.scm`'s header gives: a `define` binds
    // the name for forms compiled after it, and the composition is already
    // compiled.
    let mut runtime = layer();
    runtime
        .eval(
            r#"(status-set! 'chip
                 (lambda (vm)
                   (list (status/segment #false #false (view/mode-chip "λ" 'steel) 'none))))"#,
        )
        .expect("a segment is replaced by name");

    let drawn = row_of(&compose(&mut runtime, &screen_9c()).expect("composes"), 120);
    assert!(drawn.starts_with(" λ  src/retry.rs [+]"), "{drawn:?}");
    assert!(drawn.contains("6 unseen │ jj ✓"), "{drawn:?}");
}

#[test]
fn dropping_a_segment_from_the_order_drops_it_from_the_line() {
    let mut runtime = layer();
    runtime
        .eval("(status-order-set! 'right '(session))")
        .expect("the order is the layer's");

    let drawn = row_of(&compose(&mut runtime, &screen_9c()).expect("composes"), 120);
    assert!(drawn.contains("✻ claude idle"), "{drawn:?}");
    assert!(!drawn.contains("unseen"), "{drawn:?}");
    assert!(!drawn.contains("jj ✓"), "{drawn:?}");
}

#[test]
fn reordering_the_ladder_changes_what_goes_first() {
    // §11's order is data in the editor layer. Put the file at the top of the
    // ladder and the file is what goes, at a width where it used to survive.
    let mut runtime = layer();
    let width = 60;
    let before = row_of(
        &compose(&mut runtime, &screen_9c()).expect("composes"),
        width,
    );
    assert!(before.contains("retry.rs"), "{before:?}");

    runtime
        .eval("(status-ladder-set! '(file counter-words vcs cursor session-prose mode-word))")
        .expect("the ladder is the layer's");

    let after = row_of(
        &compose(&mut runtime, &screen_9c()).expect("composes"),
        width,
    );
    assert!(!after.contains("retry.rs"), "{after:?}");
    assert!(
        after.contains("6 unseen"),
        "the rest of the line is unharmed: {after:?}"
    );
}

// ---------------------------------------------------------------------------
// T080 — the escape hatch
// ---------------------------------------------------------------------------

/// A surface the primitive set does not cover, composed in scheme out of
/// `view/spans` and nothing else.
///
/// Deliberately shaped like `:arch` (`T048`, Q11): a heading, an indented tree
/// of names, a selected row, and a footnote in meta — the things a diagram of
/// the store needs and no `phosphor-ui` primitive draws. `T048` builds the real
/// one over a store query; what is under test here is that it will need no Rust.
const A_SURFACE_WITH_NO_PRIMITIVE: &str = r#"
(define (arch/row indent text tone tint)
  (view/span-row (list (view/run indent 'meta 'plain)
                       (view/run text tone 'plain))
                 tint))

(define (arch/sketch)
  (view/spans
   (list (arch/row "" "the semantic store" 'steel void)
         (arch/row "" "" 'meta void)
         (arch/row "  " "regions      every surface is a query over these" 'claude void)
         (arch/row "  " "seen-state   line-based fallback on any file" 'you 'selection)
         (arch/row "  " "threads      anchored to a node, or to a line" 'meta void)
         (arch/row "" "" 'meta void)
         (arch/row "" "one store · three doors" 'meta void))))
"#;

#[test]
fn a_surface_with_no_primitive_is_built_from_spans_alone() {
    let mut runtime = layer();
    runtime
        .eval(A_SURFACE_WITH_NO_PRIMITIVE)
        .expect("the hatch needs no Rust change");
    let answered = runtime.eval("(arch/sketch)").expect("it composes");
    let node = view::node(answered.last().expect("a value")).expect("a view tree");

    // One node kind, and it is the hatch. Nothing else was needed and nothing
    // else was reached for.
    assert_eq!(node.tag(), "spans");
    let Node::Spans { rows } = &node else {
        panic!("the hatch is `spans`");
    };
    assert_eq!(rows.len(), 7);

    let (drawn, buf) = drawn(&node, 60, 7);
    assert_eq!(drawn[0], "the semantic store");
    assert_eq!(
        drawn[2],
        "  regions      every surface is a query over these"
    );
    assert_eq!(drawn[6], "one store · three doors");

    // Styled, not just placed: the tones and the row tint reached the cells.
    let theme = Theme::phosphor_dark();
    assert_eq!(buf[(0, 0)].fg, theme.actors.steel);
    assert_eq!(buf[(2, 2)].fg, theme.actors.claude);
    assert_eq!(buf[(2, 3)].fg, theme.actors.you);
    assert_eq!(
        buf[(59, 3)].bg,
        theme.regions.selection,
        "a row tint paints the whole row, not only the text on it"
    );
    assert_eq!(buf[(0, 6)].fg, theme.neutrals.meta);
}

#[test]
fn the_hatch_is_the_only_node_kind_that_takes_styled_rows() {
    // `scripts/lint-one-escape-hatch.sh` holds this against the protocol's
    // source; this holds it against the protocol's own declared schema, which
    // is what a composition actually sees.
    let rows = ParamType::List(&<SpanRow as Wire>::TYPE);
    let hatches: Vec<&str> = view::constructors()
        .into_iter()
        .filter(|constructor| {
            constructor.tag.is_some() && constructor.params.iter().any(|param| param.ty == rows)
        })
        .map(|constructor| constructor.tag.unwrap_or_default())
        .collect();
    assert_eq!(hatches, ["spans"]);
}

#[test]
fn a_float_body_can_be_the_hatch_and_the_chrome_is_still_rusts() {
    // Q12's line, at the seam a person would test it at: the *contents* are
    // scheme, the border, the padding and the geometry are not.
    let mut runtime = layer();
    let answered = runtime
        .eval(
            r#"(view/tree
                 (view/empty)
                 (view/float 'informational
                             (view/float-header "◆ steel · a surface" "2 rows")
                             (view/spans (list (view/span-row (list (view/run "composed here" 'steel 'plain)) void)
                                               (view/span-row (list (view/run "drawn there" 'meta 'plain)) void)))
                             (view/key-hints 'footer (list (view/key-hint "esc" "close")))))"#,
        )
        .expect("a whole frame composes");
    let wire = phosphor_steel::convert::from_steel(answered.last().expect("a value"))
        .expect("a tree crosses");
    let tree = <Tree as Wire>::from_value(&wire).expect("a tree decodes");

    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let report = Interpreter::new(&theme, &NoResources).render(&tree, area, &mut buf);
    assert!(report.deferred.is_empty(), "{report:?}");

    let text = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("◆ steel · a surface"), "{text}");
    assert!(text.contains("composed here"), "{text}");
    assert!(text.contains("esc close"), "{text}");
    // The rules, the padding and the full-width docking under 100 columns are
    // the chrome primitive's (`T084`, §8) — the composition named a mood and a
    // body and nothing else.
    assert!(text.contains('─'), "{text}");
    assert!(
        text.contains("  composed here"),
        "the body is padded by the float, not by the composition:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// The REPL's own surface
// ---------------------------------------------------------------------------

#[test]
fn the_repl_draws_the_statusline_the_layer_composed_for_it() {
    // `6b`, end to end: the surface is `phosphor-steel`'s, the statusline is
    // the editor layer's, and one interpreter draws both.
    let mut runtime = layer();
    let mut repl = phosphor_steel::repl::Repl::new();
    repl.refresh(&mut runtime).expect("the layer composes");

    let theme = Theme::phosphor_dark();
    let area = Rect::new(0, 0, 80, 6);
    let mut buf = Buffer::empty(area);
    Interpreter::new(&theme, &NoResources).render(&repl.frame(), area, &mut buf);

    let bottom: String = (0..area.width)
        .map(|x| buf[(x, area.height - 1)].symbol())
        .collect();
    assert!(bottom.starts_with(" REPL  steel"), "{bottom:?}");
    assert!(
        bottom
            .trim_end()
            .ends_with("C-c buffer · tab complete · q close"),
        "{bottom:?}"
    );
    assert_eq!(
        buf[(1, area.height - 1)].fg,
        theme.chrome.mode_chip_fg,
        "the chip is inverted, and the tone came from the layer"
    );
    assert_eq!(buf[(1, area.height - 1)].bg, theme.actors.steel);
}

#[test]
fn the_repls_own_view_model_is_facts_and_nothing_else() {
    let vm = phosphor_steel::repl::Repl::new().status_vm();
    assert_eq!(vm.mode, "repl");
    assert_eq!(vm.surface.as_deref(), Some("steel"));
    assert_eq!(vm.session, SessionState::None, "S2 has no session to claim");
    assert_eq!(vm.unseen, 0);
    assert_eq!(
        vm.hints.first(),
        Some(&KeyHint {
            key: KeySeq("C-c".to_owned()),
            verb: "buffer".to_owned(),
        })
    );
}
