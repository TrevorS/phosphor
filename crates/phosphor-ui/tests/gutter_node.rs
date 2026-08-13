//! `T031` — `Node::Gutter` reaches the widget.
//!
//! The unit tests in `gutter.rs` prove the ladder and the cell; this proves the
//! seam, from the outside, through the crate's public API only: a composed tree
//! naming a gutter draws the column instead of being reported as an unbuilt
//! primitive.
//!
//! It lives out here rather than in `interpret.rs`'s own test module because
//! that file is `spine`'s and this task owns exactly one arm of it.

use phosphor_core::request::BufferId;
use phosphor_core::view::{Axis, Constraint, Node, Slot, Tree};
use phosphor_ui::buffer_view::{Editor, StateMark};
use phosphor_ui::gutter::{RegionSpan, RegionState, state_column};
use phosphor_ui::interpret::{Interpreter, Resources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// A host with a state column and no editors — `Node::Gutter`'s reason for
/// existing, which is the column *without* an editor behind it.
#[derive(Debug)]
struct Marks(Vec<StateMark>);

impl Resources for Marks {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        None
    }

    fn state_marks(&self, _buffer: BufferId) -> &[StateMark] {
        &self.0
    }
}

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 12,
    height: 4,
};

#[test]
fn a_gutter_node_draws_the_resolved_column() {
    let theme = Theme::phosphor_dark();
    // Three overlapping regions over four rows, resolved the way a host will
    // resolve them once `T041` answers with real ones.
    let marks = Marks(state_column(
        &[
            RegionSpan::new(0..3, RegionState::Unseen),
            RegionSpan::new(1..3, RegionState::NeedsYou),
            RegionSpan::new(2..4, RegionState::Diagnostic),
        ],
        4,
    ));
    let tree = Tree::new(Node::split(
        Axis::Columns,
        [
            Slot::new(
                Constraint::Cells { cells: 1 },
                Node::Gutter {
                    buffer: BufferId(1),
                },
            ),
            Slot::new(Constraint::Fill { weight: 1 }, Node::Empty {}),
        ],
    ));

    let mut buf = Buffer::empty(AREA);
    let report = Interpreter::new(&theme, &marks).render(&tree, AREA, &mut buf);

    assert!(
        report.deferred.is_empty(),
        "the gutter is built: {report:?}"
    );
    for (y, want) in [
        theme.actors.claude,
        theme.actors.attention,
        theme.actors.trouble,
        theme.actors.trouble,
    ]
    .into_iter()
    .enumerate()
    {
        let y = u16::try_from(y).expect("four rows");
        assert_eq!(buf[(0, y)].bg, want, "row {y}");
    }
}

#[test]
fn a_gutter_for_a_buffer_the_host_does_not_have_draws_ground() {
    let theme = Theme::phosphor_dark();
    let tree = Tree::new(Node::Gutter {
        buffer: BufferId(9),
    });
    let mut buf = Buffer::empty(AREA);
    let report = Interpreter::new(&theme, &Marks(Vec::new())).render(&tree, AREA, &mut buf);

    assert!(report.deferred.is_empty(), "{report:?}");
    for y in 0..AREA.height {
        assert_eq!(buf[(0, y)].bg, theme.neutrals.ground, "row {y}");
        assert_eq!(buf[(0, y)].symbol(), " ", "row {y}");
    }
}
