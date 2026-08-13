//! The unknown-key hint (`T035`) — one `┊` line, once per session, never again.
//!
//! Screen `8e` draws it under the code, in meta-gray with the key that missed
//! in amber:
//!
//! ```text
//! ┊ unknown key gq — SPC opens the keymap · :help agent-objects · shown once
//! ```
//!
//! It is the editor's whole onboarding budget. The line names the two doors —
//! the leader and `:help` — and then says *shown once*, which is a promise this
//! module has to keep: [`UnknownKeyHint`] is a latch, and after the first
//! unknown key every later one draws nothing at all.
//!
//! # The latch is the task
//!
//! [`UnknownKeyHint::teach`] answers `Some` exactly once in the life of one
//! value and `None` forever after — for the *same* key, a different key, or any
//! number of them. There is no reset, no "unless the user asked for it" and no
//! second door: [`teach`] is the only constructor of the row, so a caller
//! cannot draw a hint without spending the session's one hint on it.
//!
//! [`teach`]: UnknownKeyHint::teach
//!
//! # Why it has no owner
//!
//! [`phosphor_core::view::Node::VirtualText`]'s `owner` is *"the region it
//! hangs from, absent for an unowned hint"* (`view.rs`), and this is that hint.
//! Nothing in the store put the row there and nothing in the store takes it
//! away, so tagging it with a [`RegionId`] would be inventing a region — and
//! every surface that reads owners ([`crate::virtual_text::owner_at`],
//! [`crate::virtual_text::rows_of`]) would then answer a region id that names
//! nothing.
//!
//! [`RegionId`]: phosphor_core::request::RegionId
//!
//! # Where it sits
//!
//! `8e` draws the row as a **strip** rather than a row of the buffer: the code
//! rows are 20px apart with no padding and the hint's own `div` carries
//! `padding: 6px 0 6px 58px`, so it is set off from the text above it rather
//! than interleaved with it. That is why it is a [`Node::VirtualText`] in a
//! slot of its own and not a [`crate::virtual_text::Row`] installed into the
//! buffer's stream — an installed row would renumber nothing but *would* sit
//! flush against the code, which is not what the drawing shows.
//!
//! # The inset, and the one place the drawing is read rather than copied
//!
//! [`strip`] puts the `┊` at the **code column** — the same column
//! [`crate::virtual_text::install`] gives a row hanging under a whole line, so
//! the two roads cannot diverge visually.
//!
//! `8e` writes the strip's `padding-left` as `58px` while its code text starts
//! at `44px`, which at that mockup's 7.5px cell is column 6 against column ~8.
//! Read literally, the hint would sit two cells right of every other `┊` on
//! screen. Three things say otherwise and they win: §3 is *"indents to code
//! column"*; the buffer's own rows put the rail at 0 indent under a whole
//! line ([`crate::virtual_text::indent_at`]); and `58px` is exactly
//! `3 + 41 + 14`, the sum `1a` adds up to reach *its* code column — the state
//! bar, the number field and the 14px gap after it — whereas `8e`'s own 14px
//! gap is already inside its 41px number field. The `58px` is that sum applied
//! to a gutter that had already spent it. **Flagged, not folded in:** if the two
//! cells were meant, this is one constant.
//!
//! Owned by `surface`.

use phosphor_core::request::KeySeq;
use phosphor_core::view::{Child, Emphasis, Node, Tone};

/// What the row says before the key. `8e`, verbatim.
pub const LEAD: &str = "unknown key ";

/// What it says after the key — the two doors, and the promise this module
/// keeps. `8e`, verbatim, em dash and midline dots included (§6: the em dash is
/// for cause, the dot is inside a fact).
pub const TAUGHT: &str = " — SPC opens the keymap · :help agent-objects · shown once";

/// The session's one unknown-key hint.
///
/// Hold one per session — the host owns it, the way it owns
/// [`crate::frame::FrameCache`]. Copy semantics are deliberate and safe: a copy
/// of a spent latch is spent, and a copy of an unspent one that is then dropped
/// has taught nobody, so the only way to lose the hint is to keep the value
/// that answered `Some`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UnknownKeyHint {
    spent: bool,
}

impl UnknownKeyHint {
    /// A session that has not taught anything yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { spent: false }
    }

    /// Whether the session's one hint has been spent.
    ///
    /// Diagnostic — nothing needs to ask before calling
    /// [`teach`](Self::teach), which answers `None` on its own.
    #[must_use]
    pub const fn is_spent(&self) -> bool {
        self.spent
    }

    /// The hint for `key`, the first time, and `None` every time after.
    ///
    /// **The whole task is the second half.** A second unknown key — the same
    /// one or another one — draws nothing, which is what *"shown once"* on the
    /// row itself promises.
    ///
    /// The key is drawn as the sequence spells itself. See the report's note on
    /// `<space>`: the canonical → drawn mapping lives in
    /// [`crate::key_hints`] and is not public there yet.
    pub fn teach(&mut self, key: &KeySeq) -> Option<Node> {
        if self.spent {
            return None;
        }
        self.spent = true;
        Some(row(key))
    }
}

/// The hint, placed: `8e`'s strip, with its `┊` at the code column of a
/// `gutter`-wide gutter. See the module docs for why the code column and not
/// two cells past it.
///
/// Takes the node [`UnknownKeyHint::teach`] answered rather than building one,
/// so placing a hint still costs a latch.
#[must_use]
pub fn strip(hint: Node, gutter: u16) -> Node {
    Node::line([
        Node::Spacer {
            cells: u32::from(gutter),
        },
        hint,
    ])
}

/// The row itself — meta prose around the key that missed, in amber.
///
/// Private on purpose: it is the latch's payload, and a public spelling of it
/// would be a second door onto a surface whose entire contract is that there is
/// only one.
fn row(key: &KeySeq) -> Node {
    let meta = |text: &str| Node::Label {
        text: text.to_owned(),
        tone: Tone::Meta,
        emphasis: Emphasis::Plain,
    };
    Node::VirtualText {
        owner: None,
        content: Child::new(Node::line([
            meta(LEAD),
            // §1: amber is attention, and a key that resolved to nothing is
            // exactly that. `8e` draws `gq` in `#e0a94e`.
            Node::Label {
                text: key.0.clone(),
                tone: Tone::Attention,
                emphasis: Emphasis::Plain,
            },
            meta(TAUGHT),
        ])),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{LEAD, TAUGHT, UnknownKeyHint, strip};
    use crate::interpret::{Interpreter, NoResources};
    use crate::theme::Theme;
    use crate::virtual_text::RAIL_PREFIX;
    use phosphor_core::request::KeySeq;
    use phosphor_core::view::{Node, Tree};
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 3,
    };

    fn key(seq: &str) -> KeySeq {
        KeySeq(seq.to_owned())
    }

    /// Draws whatever composition produced, through the interpreter — the same
    /// road the frame takes.
    fn draw(node: Node) -> Buffer {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(AREA);
        let report =
            Interpreter::new(&theme, &NoResources).render(&Tree::new(node), AREA, &mut buf);
        assert!(report.deferred.is_empty(), "{report:?}");
        buf
    }

    fn text(buf: &Buffer, y: u16) -> String {
        (AREA.x..AREA.right())
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    // -- the latch ----------------------------------------------------------

    /// **The task, both halves.** The first unknown key teaches; the second
    /// does not, and neither does any after it.
    #[test]
    fn the_hint_fires_once_and_then_never_again() {
        let mut session = UnknownKeyHint::new();
        assert!(!session.is_spent());

        assert!(session.teach(&key("gq")).is_some(), "the first key teaches");
        assert!(session.is_spent());

        assert!(session.teach(&key("gq")).is_none(), "the same key again");
        assert!(session.teach(&key("zz")).is_none(), "a different key");
        for _ in 0..100 {
            assert!(session.teach(&key("q")).is_none());
        }
    }

    /// A fresh session teaches again — the latch is per session, and `new` is
    /// what starts one.
    #[test]
    fn a_new_session_gets_its_own_hint() {
        let mut first = UnknownKeyHint::new();
        assert!(first.teach(&key("gq")).is_some());
        let mut second = UnknownKeyHint::default();
        assert!(second.teach(&key("gq")).is_some());
        assert!(first.teach(&key("gq")).is_none(), "and the first is spent");
    }

    // -- the row -------------------------------------------------------------

    /// `8e`'s line, cell for cell, with the rail the primitive draws.
    #[test]
    fn the_row_says_what_8e_says() {
        let mut session = UnknownKeyHint::new();
        let hint = session.teach(&key("gq")).expect("the first key");
        let buf = draw(hint);
        assert_eq!(
            text(&buf, 0),
            "┊ unknown key gq — SPC opens the keymap · :help agent-objects · shown once"
        );
        assert_eq!(text(&buf, 1), "", "§11: nothing ever wraps");
    }

    /// §3's *"meta-gray with colored spans"*: the prose and the rail are meta,
    /// the key that missed is amber (§1 — attention).
    #[test]
    fn the_key_is_the_only_thing_that_is_not_meta() {
        let theme = Theme::phosphor_dark();
        let mut session = UnknownKeyHint::new();
        let buf = draw(session.teach(&key("gq")).expect("the first key"));
        let drawn = text(&buf, 0);
        // Cells, not bytes: `┊` and `—` are multi-byte and `str::find` would
        // answer an offset that is not a column.
        let column = |needle: &str| -> u16 {
            let at = drawn.find(needle).expect("drawn");
            u16::try_from(drawn[..at].chars().count()).expect("on screen")
        };

        assert_eq!(buf[(0, 0)].fg, theme.neutrals.meta, "the rail");
        assert_eq!(buf[(column("unknown"), 0)].fg, theme.neutrals.meta);
        let at = column("gq");
        assert_eq!(buf[(at, 0)].symbol(), "g");
        assert_eq!(buf[(at, 0)].fg, theme.actors.attention);
        assert_eq!(buf[(at + 1, 0)].fg, theme.actors.attention);
        assert_eq!(buf[(column("SPC"), 0)].fg, theme.neutrals.meta);
    }

    /// The row is unowned. `8e`'s hint hangs from no region, so nothing that
    /// reads owners can be handed one that names nothing.
    #[test]
    fn the_row_carries_no_owner() {
        let mut session = UnknownKeyHint::new();
        let hint = session.teach(&key("gq")).expect("the first key");
        assert!(
            matches!(hint, Node::VirtualText { owner: None, .. }),
            "{hint:?}"
        );
    }

    /// The key is drawn, whatever it is — a chord reads as the resolver spells
    /// it, so the hint cannot silently teach the wrong key.
    #[test]
    fn whatever_missed_is_what_the_row_names() {
        for spelled in ["gq", "<C-q>", "Z"] {
            let mut session = UnknownKeyHint::new();
            let buf = draw(session.teach(&key(spelled)).expect("the first key"));
            assert_eq!(text(&buf, 0), format!("┊ {LEAD}{spelled}{TAUGHT}"));
        }
    }

    // -- placement -----------------------------------------------------------

    /// The rail sits at the code column — the same column
    /// [`crate::virtual_text::install`] gives a row under a whole line — and
    /// the text starts [`RAIL_PREFIX`] cells past it.
    #[test]
    fn the_strip_puts_its_rail_at_the_code_column() {
        let gutter = 6; // every two-digit mockup, `8e` included
        let mut session = UnknownKeyHint::new();
        let hint = session.teach(&key("gq")).expect("the first key");
        let buf = draw(strip(hint, gutter));

        assert_eq!(buf[(gutter, 0)].symbol(), "┊");
        assert_eq!(buf[(gutter - 1, 0)].symbol(), " ");
        assert_eq!(buf[(gutter + RAIL_PREFIX, 0)].symbol(), "u");
    }

    /// A strip narrower than its row clips at the right edge rather than
    /// wrapping, and a gutter wider than the screen does not panic.
    #[test]
    fn a_strip_with_no_room_clips_rather_than_wrapping() {
        let mut session = UnknownKeyHint::new();
        let hint = session.teach(&key("gq")).expect("the first key");
        let buf = draw(strip(hint, u16::MAX));
        assert_eq!(text(&buf, 0), "");
        assert_eq!(text(&buf, 1), "");
    }
}
