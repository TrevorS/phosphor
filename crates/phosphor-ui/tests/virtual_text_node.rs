//! `T032` — `┊` rows reach the screen by both roads.
//!
//! The unit tests in `virtual_text.rs` prove placement and the cell; this
//! proves the seams, from the outside, through the crate's public API only:
//!
//! 1. a composed tree naming a `virtual-text` node draws a rail instead of
//!    being reported as an unbuilt primitive, and
//! 2. a `buffer` node over an editor with rows installed draws them
//!    interleaved — **including on a soft-wrapped line**, which is `CP-3`'s
//!    named gate item and the fourth subsystem `T081` touches.
//!
//! It lives out here rather than in `interpret.rs`'s own test module because
//! that file is `spine`'s and this task owns exactly one arm of it.

use phosphor_core::request::{BufferId, RegionId};
use phosphor_core::view::{Child, Emphasis, Glyph, Node, Tone, Tree};
use phosphor_ui::buffer_view::{self, Editor};
use phosphor_ui::interpret::{Interpreter, NoResources, Report, Resources};
use phosphor_ui::soft_wrap;
use phosphor_ui::theme::Theme;
use phosphor_ui::virtual_text::{self, Anchor, Row, Run};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 60,
    height: 16,
};

/// One line short enough to fit and one long enough to wrap three ways at
/// [`AREA`]'s width, which is what "first, middle and last visual row" needs.
const SOURCE: &str = "\
fn main() {}
// a very long trailing comment that has to wrap across three separate visual rows before it finally runs out of words to say and stops
";

/// A host with exactly one buffer. `Editor` is not `Debug`, so the impl
/// [`Resources`] requires is written by hand.
struct OneBuffer(Editor);

impl std::fmt::Debug for OneBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneBuffer")
            .field("rows", &self.0.visual_len_lines())
            .finish()
    }
}

impl Resources for OneBuffer {
    fn editor(&self, buffer: BufferId) -> Option<&Editor> {
        (buffer == BufferId(1)).then_some(&self.0)
    }
}

fn draw(tree: &Tree, resources: &dyn Resources, theme: &Theme) -> (Buffer, Report) {
    let mut buf = Buffer::empty(AREA);
    let report = Interpreter::new(theme, resources).render(tree, AREA, &mut buf);
    (buf, report)
}

fn row(buf: &Buffer, y: u16) -> String {
    (AREA.x..AREA.right())
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// The fixture, configured the way a host configures a buffer, wrapped to
/// [`AREA`].
fn editor(theme: &Theme) -> Editor {
    let mut editor = Editor::new("rust", SOURCE, vec![]).expect("fixture parses");
    buffer_view::configure(&mut editor, theme);
    soft_wrap::configure(&mut editor, theme);
    virtual_text::configure(&mut editor, theme);
    soft_wrap::wrap_to(&mut editor, AREA);
    editor
}

/// The visual rows a 0-based source line occupies, in order.
fn segments(editor: &Editor, line: usize) -> Vec<usize> {
    (0..editor.visual_len_lines())
        .filter(|row| {
            editor
                .row_span(*row)
                .is_some_and(|span| span.line_idx == line)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1 — the node
// ---------------------------------------------------------------------------

#[test]
fn a_virtual_text_node_draws_a_rail_instead_of_deferring() {
    let theme = Theme::phosphor_dark();
    let tree = Tree::new(Node::VirtualText {
        owner: Some(RegionId(4)),
        content: Child::new(Node::line([
            Node::Glyph {
                glyph: Glyph::Anchor,
                tone: Tone::You,
            },
            Node::Label {
                text: " you · 2m".to_owned(),
                tone: Tone::You,
                emphasis: Emphasis::Plain,
            },
            Node::Spacer { cells: 2 },
            Node::Label {
                text: "cap check?".to_owned(),
                tone: Tone::Meta,
                emphasis: Emphasis::Plain,
            },
        ])),
    });

    let (buf, report) = draw(&tree, &NoResources, &theme);
    assert!(
        report.deferred.is_empty(),
        "the primitive is built now: {report:?}"
    );
    // `⚓` is two cells wide, so the cell after it is the wide-grapheme filler
    // and the row reads with a doubled space. That is the terminal's arithmetic
    // and not a stray space in the composition.
    assert_eq!(row(&buf, 0), "┊ ⚓  you · 2m  cap check?");
    assert_eq!(buf[(0, 0)].fg, theme.neutrals.meta, "§3: the rail is meta");
    assert_eq!(buf[(2, 0)].fg, theme.actors.you);
    assert_eq!(row(&buf, 1), "", "§11: a row is one row");
}

#[test]
fn an_unowned_row_draws_exactly_like_an_owned_one() {
    // `8e`'s unknown-key hint has no region behind it (`T035`). The owner is a
    // tag for whoever placed the row, not something the drawing depends on.
    let theme = Theme::phosphor_dark();
    let content = || {
        Child::new(Node::Label {
            text: "unknown key gq — SPC opens the keymap".to_owned(),
            tone: Tone::Meta,
            emphasis: Emphasis::Plain,
        })
    };
    let (owned, _) = draw(
        &Tree::new(Node::VirtualText {
            owner: Some(RegionId(7)),
            content: content(),
        }),
        &NoResources,
        &theme,
    );
    let (unowned, _) = draw(
        &Tree::new(Node::VirtualText {
            owner: None,
            content: content(),
        }),
        &NoResources,
        &theme,
    );
    assert_eq!(owned, unowned);
    assert_eq!(row(&owned, 0), "┊ unknown key gq — SPC opens the keymap");
}

// ---------------------------------------------------------------------------
// 2 — the buffer's own rows, on a wrapped line
// ---------------------------------------------------------------------------

/// **`CP-3`'s gate item, end to end.** A row anchored to a column in the middle
/// of a wrapped line is drawn under *that segment*, and every line number on
/// screen is what it was before the row existed.
#[test]
fn a_row_lands_on_the_right_segment_of_a_wrapped_line() {
    let theme = Theme::phosphor_dark();
    let tree = Tree::new(Node::Buffer {
        buffer: BufferId(1),
        soft_wrap: true,
    });

    let bare = OneBuffer(editor(&theme));
    let rows = segments(&bare.0, 1);
    assert!(
        rows.len() >= 3,
        "the fixture must wrap three ways: {rows:?}"
    );
    let (before, _) = draw(&tree, &bare, &theme);

    let middle = bare.0.row_span(rows[1]).expect("a middle segment");
    let mut host = OneBuffer(editor(&theme));
    virtual_text::install(
        &mut host.0,
        &[Row::new(
            Anchor::at(1, (middle.start_col + middle.end_col) / 2),
            vec![Run::new(
                "1 diagnostic · claude sees what LSP sees",
                Style::new().fg(theme.actors.trouble),
            )],
        )
        .owned_by(RegionId(2))],
    );
    let (after, _) = draw(&tree, &host, &theme);

    // The row is immediately under the middle segment …
    let hung = rows[1] + 1;
    assert!(row(&after, hung as u16).contains("┊ 1 diagnostic"));
    assert_eq!(virtual_text::rows_of(&host.0, RegionId(2)), vec![hung]);

    // … the rows above it are untouched …
    for y in 0..hung as u16 {
        assert_eq!(row(&before, y), row(&after, y), "row {y} moved");
    }
    // … and the row below it is the row that used to be there, not a renumber.
    assert_eq!(row(&before, hung as u16), row(&after, hung as u16 + 1));
}

#[test]
fn a_row_never_shifts_a_line_number() {
    let theme = Theme::phosphor_dark();
    let tree = Tree::new(Node::Buffer {
        buffer: BufferId(1),
        soft_wrap: true,
    });
    let numbers = |buf: &Buffer| -> Vec<String> {
        (0..AREA.height)
            .map(|y| {
                row(buf, y)
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_owned()
            })
            .filter(|word| word.parse::<u32>().is_ok())
            .collect()
    };

    let bare = OneBuffer(editor(&theme));
    let (before, _) = draw(&tree, &bare, &theme);

    let mut host = OneBuffer(editor(&theme));
    virtual_text::install(
        &mut host.0,
        &[
            Row::new(Anchor::line(0), vec![Run::prose("first", &theme)]),
            Row::new(Anchor::line(1), vec![Run::prose("second", &theme)]),
            Row::new(Anchor::line(1), vec![Run::prose("third", &theme)]),
        ],
    );
    let (after, _) = draw(&tree, &host, &theme);

    assert_eq!(
        numbers(&before),
        numbers(&after),
        "a virtual row is not a line, so nothing was renumbered"
    );
    assert!(
        row(&after, 1).starts_with("      ┊ first"),
        "{:?}",
        row(&after, 1)
    );
}
