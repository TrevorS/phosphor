//! `T034` / `T086` — `Node::KeyHints` reaches the widget, at all three
//! densities.
//!
//! The unit tests in `key_hints.rs` prove the layout and the spelling; this
//! proves the seam, from the outside and through the crate's public API only: a
//! composed tree naming a keymap surface draws it instead of being reported as
//! an unbuilt primitive, and a float over one sizes itself to the grid rather
//! than collapsing to chrome.
//!
//! It lives out here rather than in `interpret.rs`'s own test module because
//! that file is `spine`'s and this task owns exactly its `Node::KeyHints` arms.

use phosphor_core::request::KeySeq;
use phosphor_core::view::{
    Axis, Child, Constraint, Density, Float, FloatHeader, KeyHint, Mood, Node, Slot, Tree,
};
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 24,
};

fn hint(key: &str, verb: &str) -> KeyHint {
    KeyHint {
        key: KeySeq(key.to_owned()),
        verb: verb.to_owned(),
    }
}

fn draw(tree: &Tree) -> (Buffer, Vec<&'static str>) {
    let theme = Theme::phosphor_dark();
    let mut buf = Buffer::empty(AREA);
    let report = Interpreter::new(&theme, &NoResources).render(tree, AREA, &mut buf);
    (buf, report.deferred)
}

fn text(buf: &Buffer) -> String {
    (AREA.y..AREA.bottom())
        .map(|y| {
            (AREA.x..AREA.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `3c`: the leader strip in a row slot of its own, above where a statusline
/// goes.
#[test]
fn a_leader_grid_draws_where_composition_puts_it() {
    let tree = Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(Constraint::Fill { weight: 1 }, Node::Empty {}),
            Slot::new(
                Constraint::Cells { cells: 4 },
                Node::KeyHints {
                    density: Density::Grid,
                    hints: vec![
                        hint("<space>c", "+claude · prompt · steer · interrupt"),
                        hint("<space>t", "transcript"),
                    ],
                },
            ),
        ],
    ));
    let (buf, deferred) = draw(&tree);
    let drawn = text(&buf);
    assert!(deferred.is_empty(), "{deferred:?}");
    assert!(drawn.contains("SPC ·"), "{drawn}");
    assert!(
        drawn.contains("c  +claude  prompt · steer · interrupt"),
        "{drawn}"
    );
    assert!(drawn.contains("t  transcript"), "{drawn}");
}

/// `6d`: the `:help` body, and the reason `Ctx::height` needed an arm — a float
/// asks its body how tall it is, and a body that answers zero draws nothing at
/// all.
#[test]
fn a_help_body_gives_the_float_its_height() {
    let hints: Vec<KeyHint> = (0..6)
        .map(|index| hint(&format!("v{index}u"), "select inner unseen region"))
        .collect();
    let tree = Tree::new(Node::Empty {}).with_float(Float {
        mood: Mood::Informational,
        header: Some(FloatHeader::new(":help agent-objects")),
        body: Child::new(Node::KeyHints {
            density: Density::Help,
            hints,
        }),
        footer: Some(Child::new(Node::KeyHints {
            density: Density::Footer,
            hints: vec![hint("q", "close")],
        })),
    });
    let (buf, deferred) = draw(&tree);
    let drawn = text(&buf);
    assert!(deferred.is_empty(), "{deferred:?}");
    assert!(drawn.contains(":help agent-objects"), "{drawn}");
    assert!(drawn.contains("q close"), "{drawn}");
    assert_eq!(
        drawn.matches("select inner unseen region").count(),
        6,
        "every row of the body is on screen, not just the ones chrome left room \
         for by accident:\n{drawn}"
    );
}

/// A footer outside a float is the same row the float chrome draws — one
/// string, two routes to it.
#[test]
fn a_footer_outside_a_float_reads_the_same_as_one_inside() {
    let hints = vec![hint("↵", "open"), hint("s", "mark seen"), hint("esc", "")];
    let bare = Tree::new(Node::KeyHints {
        density: Density::Footer,
        hints: hints.clone(),
    });
    let framed = Tree::new(Node::Empty {}).with_float(Float {
        mood: Mood::Informational,
        header: Some(FloatHeader::new("❯ unseen")),
        body: Child::new(Node::Empty {}),
        footer: Some(Child::new(Node::KeyHints {
            density: Density::Footer,
            hints,
        })),
    });

    let (bare_buf, bare_deferred) = draw(&bare);
    let (framed_buf, framed_deferred) = draw(&framed);
    assert!(bare_deferred.is_empty(), "{bare_deferred:?}");
    assert!(framed_deferred.is_empty(), "{framed_deferred:?}");
    let row = "↵ open · s mark seen · esc";
    assert!(text(&bare_buf).contains(row));
    assert!(text(&framed_buf).contains(row));
}
