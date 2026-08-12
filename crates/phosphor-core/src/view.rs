//! The view tree — the contract between Steel composition and Rust primitives.
//!
//! Plain data: which primitives, laid out how, with what props. No ratatui types
//! and no Steel types, so neither side owns the protocol (Q12). `phosphor-steel`
//! produces a tree; `phosphor-ui` interprets it into ratatui calls.
//!
//! Only `spine` writes this module (`TEAM.md`'s first single-writer rule names
//! *"the `Action` enum, the query vocabulary, or the view tree"* in one breath).
//!
//! The "no Steel and no ratatui dependency" half of Q12 is not a comment: this
//! crate's `[dependencies]` table is empty, and `scripts/lint-no-store-mutation.sh`
//! fails CI if either ever appears there.
//!
//! # The shape
//!
//! ```text
//! Tree { root, float }
//!   │      │      └── Option<Float> — one slot. Float-over-float is
//!   │      │          unrepresentable, not merely forbidden (§9).
//!   │      └── Node — one enum, ~30 kinds, exhaustive.
//!   │             ├── containers: Split · Line · Shed · Pane · Spring · Spacer
//!   │             ├── primitives: Buffer · Picker · Diff · Transcript · …
//!   │             └── Spans — the one escape hatch (T080).
//!   └── recursion goes through Child, and only through Child.
//! ```
//!
//! # The rule for adding a node kind
//!
//! **A kind exists for every primitive `phosphor-ui` draws, and its props are
//! only what *composition* decides.** Everything else a primitive needs it reads
//! from the store itself. That is why [`Node::Picker`] carries a source, a
//! filter and its columns but not its rows; why [`Node::Buffer`] carries a
//! buffer id but not a viewport; and why [`Node::Question`] carries an
//! [`AskId`] and nothing else. A prop that duplicates
//! store state is a second copy to go stale in.
//!
//! A *new* primitive is a Rust change, by rule (Q12: *"Steel composes
//! primitives; it does not define them"*). The pressure valve is
//! [`Node::Spans`], and it is deliberately one grep-able name.
//!
//! # Why the enum is exhaustive
//!
//! [`crate::action::Action`] is `#[non_exhaustive]`; [`Node`] is not, and the
//! difference is deliberate. `T079`'s interpreter is one match over this enum,
//! and a node kind that reaches it with no arm is a hole in the frame. Leaving
//! it exhaustive means adding a kind **fails to compile** in `phosphor-ui` until
//! someone draws it — which is where the compiler should be shouting. The same
//! argument applies to Steel in reverse and gets no help from the compiler,
//! which is why the tag table is enumerable: see [`Node::TAGS`].
//!
//! # Why the tree crosses a door
//!
//! Every type here implements [`Wire`], so a tree encodes to
//! [`Value`] and back. Two things need that, and one of
//! them is already in the vocabulary:
//!
//! * `phosphor-steel` decodes one wire model rather than writing a second
//!   `SteelVal` walker beside the one `T020` generates for Action payloads;
//! * [`PaneKind::Custom`] — *"a pane whose
//!   contents claude emitted as a view tree"* — is v1.5 and named now, and Q12's
//!   closing argument is that it must be *"same door, no new machinery"*.
//!
//! **The one place the declared type language says `any` is [`Child`].**
//! [`ParamType`] has no recursive case — every other
//! shape in it is finite, and a self-referential `const` does not compile — so
//! the recursion is named by a wrapper whose declared shape is
//! [`ParamType::Any`] and whose codec is
//! [`Node`]'s. Every non-recursive field keeps its real declared type, which
//! means `<Node as Wire>::TYPE` is still a union over every tag with every
//! scalar field described.
//!
//! # What is deliberately *not* in the tree
//!
//! * **Colours.** [`Tone`] names an actor or a state; the theme resolves it. A
//!   view tree carrying `#3ddc97` would route around
//!   `scripts/lint-no-literal-colours.sh` through Steel.
//! * **The viewport.** Invariant 3: a viewport moves only on an explicit
//!   `Action` ([`ScrollRequest`](crate::request::ScrollRequest)). A tree that
//!   could place a viewport would let a redraw scroll the buffer.
//! * **Focus arbitration, dimming, and the one-float rule.** Rust's. The tree
//!   says which pane is focused ([`Node::Pane`]) and what float is open; it does
//!   not get to put two floats on screen or to dim a pane (§9: *"panes never dim
//!   each other"*).
//! * **Frame timing.** The spinner's 80ms and the elapsed counter's 1s are
//!   rendered from a [`Millis`] mark without re-entering the VM — see
//!   [`Node::Spinner`] and `action.rs`'s *"what is deliberately not an Action"*.
//! * **The tmux bar.** Not ours (§5).
//!
//! # Three flagged seams
//!
//! [`SessionState`] and [`Mood`] each already exist on the widget side
//! (`status_line.rs:186`, `float.rs:93`), written first, and [`Constraint`]
//! mirrors ratatui's solver. All three are recorded in their own docs as
//! `surface`-owned duplicates to collapse later; none is folded in here, because
//! deleting a `surface` file's type is not a `spine` edit.

pub mod props;

pub use props::{
    Axis, Constraint, Density, DiffSource, Emphasis, FloatHeader, Glyph, KeyHint, Millis, Mood,
    PickerColumn, Run, SessionState, Slot, SpanRow, Tab, Tint, Tone,
};

use std::path::PathBuf;

use crate::registry::ParamType;
use crate::request::{
    AskId, BufferId, DiffMode, FileSpan, Grouping, PaneId, PaneKind, PromptKind, RegionId,
    SourceId, TurnId, WatchId,
};
use crate::value::{Value, Wire, WireError, wire_record, wire_union};

// ---------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------

/// One frame's declarative description of the screen.
///
/// Not `#[non_exhaustive]`, and not opaque: `phosphor-steel` constructs these
/// and `phosphor-ui` destructures them, which is the whole job. A protocol whose
/// two ends cannot name its fields is a protocol with a third party in it.
///
/// **The float is a slot, not a node.** Design Language §9 is unconditional —
/// *"opening a second replaces the first; there is no float-over-float, ever"* —
/// and a rule the type system enforces cannot be broken by a composition written
/// at 2am. It also keeps a [`Float`]'s chrome out of the tiling tree, where it
/// would have to be laid out like a pane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    /// Everything that tiles: panes, chrome strips, the statusline.
    pub root: Node,
    /// The one float, if one is open. Q9's queued asks are *not* here until they
    /// surface — the queue is a store query, and a pending ask that nothing has
    /// room for renders as the statusline's `!` flag alone.
    pub float: Option<Float>,
}

wire_record!(Tree {
    root: Node = "everything that tiles: panes, chrome strips, the statusline",
    float: Option<Float> = "the one open float, or absent",
});

impl Tree {
    /// A tree with no float.
    #[must_use]
    pub const fn new(root: Node) -> Self {
        Self { root, float: None }
    }

    /// The same tree with `float` open over it.
    #[must_use]
    pub fn with_float(mut self, float: Float) -> Self {
        self.float = Some(float);
        self
    }
}

/// The one chrome primitive (§4): header · body · footer, and a mood border.
///
/// A struct rather than a [`Node`] kind because of where it may appear: exactly
/// one, exactly at [`Tree::float`]. The geometry — 60–80% of width, centered,
/// never within 4 cols of an edge, full-width under 100 columns, 1 row / 2 cols
/// of padding, the ground behind it dimmed — is `phosphor-ui`'s and is not
/// expressible here on purpose (§8, `float.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Float {
    /// The border colour's only meaning.
    pub mood: Mood,
    /// *"source or command · meta right"*.
    pub header: Option<FloatHeader>,
    /// The body: a picker, a diff, a question, a grid, or [`Node::Spans`].
    pub body: Child,
    /// *"every legal key, always visible"* — usually
    /// [`Node::KeyHints`] at [`Density::Footer`]. Absent for
    /// [`Mood::Passive`], §4's one documented exception.
    pub footer: Option<Child>,
}

wire_record!(Float {
    mood: Mood = "informational, needs-you, or passive",
    header: Option<FloatHeader> = "the source or command, and right-aligned meta",
    body: Child = "the float's body",
    footer: Option<Child> = "the key hints, absent only for the passive mood",
});

impl Float {
    /// A float in `mood` around `body`, with no header and no footer.
    #[must_use]
    pub fn new(mood: Mood, body: Node) -> Self {
        Self {
            mood,
            header: None,
            body: Child::new(body),
            footer: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Recursion
// ---------------------------------------------------------------------------

/// A nested node — **the one place this protocol recurses.**
///
/// A newtype rather than a bare `Box<Node>` so the recursion has a name and a
/// single [`Wire`] impl to carry the one honest `any` in the schema (see the
/// module docs). Anything holding children holds these, so "how deep can a tree
/// go" has exactly one answer to give the interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Child(pub Box<Node>);

impl Child {
    /// Wraps a node.
    #[must_use]
    pub fn new(node: Node) -> Self {
        Self(Box::new(node))
    }

    /// The node inside.
    #[must_use]
    pub fn node(&self) -> &Node {
        &self.0
    }
}

impl From<Node> for Child {
    fn from(node: Node) -> Self {
        Self::new(node)
    }
}

/// Declared as [`ParamType::Any`] because the type language has no recursive
/// case and a self-referential `const` does not compile. The codec is
/// [`Node`]'s, so nothing about the *value* is loosened — only its description.
impl Wire for Child {
    const TYPE: ParamType = ParamType::Any;

    fn to_value(&self) -> Value {
        self.0.to_value()
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        Node::from_value(value).map(Self::new)
    }
}

// ---------------------------------------------------------------------------
// The macro
// ---------------------------------------------------------------------------

/// Declares the node kinds: variants, rustdoc, wire codec and the tag table,
/// from one table.
///
/// The same discipline as `actions!` and `queries!` — one row emits everything,
/// so a kind cannot exist without a tag and a tag cannot exist without a kind.
macro_rules! nodes {
    (
        $(
            $variant:ident = $tag:literal, $doc:literal {
                $( $field:ident : $fty:ty = $fdoc:literal ),* $(,)?
            }
        )*
    ) => {
        /// One node of the view tree.
        ///
        /// Exhaustive on purpose — see the module docs. Every kind is either a
        /// container Steel composes with, a primitive `phosphor-ui` draws, or
        /// [`Node::Spans`].
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Node {
            $(
                #[doc = $doc]
                #[doc = ""]
                #[doc = concat!("Tag on the wire: `", $tag, "`.")]
                $variant {
                    $(
                        #[doc = $fdoc]
                        $field: $fty,
                    )*
                },
            )*
        }

        wire_union!(Node {
            $(
                $variant => $tag, $doc {
                    $( $field: $fty = $fdoc ),*
                }
            ),*
        });

        impl Node {
            /// Every node kind's wire tag, in declaration order.
            ///
            /// The Rust side gets exhaustiveness from the compiler; Steel and an
            /// agent-emitted tree get it from here. `T024`'s enumeration
            /// discipline, applied to the protocol rather than the vocabulary: a
            /// hand-written list of node kinds in a `.scm` file or a schema
            /// would rot, and this cannot.
            pub const TAGS: &'static [&'static str] = &[$($tag),*];

            /// This node's wire tag.
            ///
            /// A match, not a re-encode: `T079` walks a tree per state change
            /// and a benchmark walks one per frame, and neither should be
            /// cloning every `String` in the tree to find out what it is
            /// looking at.
            #[must_use]
            pub const fn tag(&self) -> &'static str {
                match self {
                    $( Self::$variant { .. } => $tag, )*
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// The node kinds
// ---------------------------------------------------------------------------

nodes! {
    // -- containers ---------------------------------------------------------

    Empty = "empty", "draws nothing. What an absent surface composes to, so a \
        composition never has to return a sentinel" {}

    Split = "split", "divides its area along an axis and gives each child a \
        share of it. Panes, chrome strips, and the frame itself" {
        axis: Axis = "rows or columns",
        slots: Vec<Slot> = "the children, each with the area it asks for",
    }

    Line = "line", "one row of nodes at their natural widths, left to right. \
        Never wraps: a second row is a bug (Design Language §5), so the \
        interpreter sheds and truncates instead" {
        children: Vec<Child> = "the nodes of this row, left to right",
    }

    Spring = "spring", "expands to fill whatever a line has left over. The gap \
        between the file and the session state in the statusline" {}

    Spacer = "spacer", "a fixed run of air, in cells" {
        cells: u32 = "how many cells of air",
    }

    Shed = "shed", "marks a child as droppable when its line does not fit. \
        Ascending priority is the order of the ladder, and a child with no shed \
        wrapper never drops — which is exactly how the last-standing set of \
        Design Language §11 is written" {
        priority: u32 = "lower sheds first",
        contracted: Option<Child> = "a narrower form to try before dropping it entirely",
        child: Child = "the node this governs",
    }

    Pane = "pane", "one pane of the split tree. Panes never dim each other (§9); \
        only floats dim what is behind them" {
        pane: PaneId = "which pane",
        holds: PaneKind = "a buffer, the transcript, or a claude-built surface",
        focused: bool = "whether keystrokes go here",
        child: Child = "its contents",
    }

    // -- chrome -------------------------------------------------------------

    TabBar = "tab-bar", "the top chrome strip. Appears only at 2+ panes (§5)" {
        tabs: Vec<Tab> = "the tabs, left to right",
    }

    ModeChip = "mode-chip", "the inverted chip at the left of the statusline — \
        the only inverted text on screen (§5). The label is a surface name, not \
        only an edit mode: REVIEW, DISKDIFF and REPL are drawn in the same chip" {
        label: String = "the surface or mode name",
        tone: Tone = "the chip's background role",
    }

    FileLabel = "file-label", "a path and its dirty flag. The basename \
        contraction of `8d` is a shed step, not a prop" {
        path: PathBuf = "the path, as it should read",
        dirty: bool = "whether to draw `[+]` in attention-amber",
    }

    Session = "session", "the session state, rendered identically everywhere it \
        appears (§5). Always present and truthful" {
        state: SessionState = "idle, working, waiting, paused, lost, or none",
        since: Option<Millis> = "when the current turn started, for the elapsed counter",
        prose: bool = "false contracts it to its glyph — §11's shed step",
    }

    Counter = "counter", "a glyph and a number, with an optional word. `6 \
        unseen` at width and `●6` once the counters have shed their words" {
        glyph: Glyph = "what is being counted",
        count: u32 = "how many; zero draws nothing at all",
        label: Option<String> = "the word, dropped by the first rung of the ladder",
        tone: Tone = "which actor or state colours it",
    }

    Divider = "divider", "the thin bar `│` between statusline segments, in \
        meta-grey (§6). Structure, not decoration — it joins the counter group \
        and nothing else (CP-1)" {}

    // -- text ---------------------------------------------------------------

    Label = "label", "one run of text in one tone" {
        text: String = "the text",
        tone: Tone = "which actor or state colours it",
        emphasis: Emphasis = "plain, inverted, underlined or undercurled",
    }

    Glyph = "glyph", "one cell out of Design Language §2's lexicon" {
        glyph: Glyph = "which concept",
        tone: Tone = "which actor or state colours it",
    }

    Spans = "spans", "**the escape hatch.** Styled rows straight from Steel, for \
        a surface the primitive set does not cover — `:arch` is built entirely \
        from this and adds zero lines to phosphor-ui (T048, T080). One \
        grep-able name, so a frame-budget regression has exactly one place to \
        look" {
        rows: Vec<SpanRow> = "the rows, top to bottom",
    }

    // -- time-derived -------------------------------------------------------

    Spinner = "spinner", "the braille spinner, 80ms a frame (§8). Rust animates \
        it from the mark without re-entering the VM: a frame tick is not an \
        Action, so it bumps no revision and invalidates no cached tree" {
        since: Millis = "when the thing being waited on started",
    }

    Elapsed = "elapsed", "a counter that ticks once a second, drawn `0:31` or \
        `31:34`. Same contract as the spinner: Rust re-renders, the tree stays \
        cached" {
        since: Millis = "when the clock started",
    }

    // -- buffer surfaces ----------------------------------------------------

    Buffer = "buffer", "a BufferView: the 3-column contract, tree-sitter \
        highlighting, region tints, virtual-text rows. It carries no viewport — \
        invariant 3 puts the viewport behind an Action, and a redraw may never \
        move it" {
        buffer: BufferId = "which buffer",
        soft_wrap: bool = "whether long lines wrap with `↪` continuations (off by default)",
    }

    Gutter = "gutter", "the 1-cell state column alone, resolved per row with \
        priority trouble > attention > claude-unseen > none. Part of every \
        BufferView; a node kind of its own for the surfaces that want the column \
        without the editor" {
        buffer: BufferId = "whose rows to resolve",
    }

    VirtualText = "virtual-text", "a `┊`-prefixed row hanging from a region, \
        indented to the code column. Threads, watches, diagnostics and the \
        once-per-session unknown-key hint all render through this" {
        owner: Option<RegionId> = "the region it hangs from, absent for an unowned hint",
        content: Child = "what the row says",
    }

    // -- float bodies and panes ---------------------------------------------

    Picker = "picker", "a filter line, a matched list and an optional preview \
        split. One widget, many sources: unseen, files, inbox, grep, symbols, \
        session adoption and the jj timeline are all this kind with a different \
        source key" {
        source: SourceId = "which `define-picker-source` supplies the rows",
        filter: String = "the current filter text",
        columns: Vec<PickerColumn> = "which row fields are shown, how wide, in whose colour",
        preview: bool = "whether to ask for the preview split; dropped under 100 columns",
    }

    Diff = "diff", "added and deleted lines with expandable unchanged spans, \
        per-hunk seen state, and directory grouping. The review block, the hunk \
        peek and `:diff-disk` are one kind with a different source" {
        source: DiffSource = "what is being diffed",
        mode: DiffMode = "unified or side-by-side",
        grouping: Grouping = "by directory, or one flat list",
    }

    Question = "question", "prose, amber digit options `[1]`-`[n]`, and the \
        full command in the footer. Digits answer only while it is focused" {
        ask: AskId = "the queued ask this is showing",
    }

    Transcript = "transcript", "the turn list: prompt lines, prose, tool rows, \
        seam markers, review-ready lines. A pane, not a float — it splits, holds \
        focus like a window, and survives float churn (§9)" {
        follow: bool = "whether to stay pinned to the newest row while a turn streams",
        folded: Vec<TurnId> = "turns collapsed to one row — scale is grouping, not scrolling",
    }

    Prompt = "prompt", "the `:` line, with a `⚓` anchor chip when a selection \
        rides along. Routes to command parse or to a message for claude" {
        prompt: PromptKind = "ex, a message to claude, or search",
        text: String = "what has been typed so far",
        anchor: Option<FileSpan> = "the selection riding along, if any",
    }

    KeyHints = "key-hints", "a keymap surface at one of three densities, read \
        from the live keymap so a REPL rebind appears with no extra wiring. A \
        float footer, the `SPC` leader grid, and the `:help` body are the same \
        data three ways" {
        density: Density = "footer, leader grid, or the help body",
        hints: Vec<KeyHint> = "the entries, in the order they should read",
    }

    Completion = "completion", "the LSP completion list, in a passive float. \
        Takes no props: there is one active completion session and the store \
        holds it — composition decides only where it goes" {}

    Signature = "signature", "LSP signature help. Same contract as the \
        completion list; hover prose renders as a Label or Spans body instead" {}

    Watch = "watch", "one watch's `◉ ⇒` value stream with its run-provenance \
        line. This node only formats — it renders through a virtual-text row, \
        and the values arrive over the session" {
        watch: WatchId = "which watch",
    }
}

impl Default for Node {
    fn default() -> Self {
        Self::Empty {}
    }
}

impl Node {
    /// A line of children.
    #[must_use]
    pub fn line(children: impl IntoIterator<Item = Self>) -> Self {
        Self::Line {
            children: children.into_iter().map(Child::new).collect(),
        }
    }

    /// A split of already-constrained slots.
    #[must_use]
    pub fn split(axis: Axis, slots: impl IntoIterator<Item = Slot>) -> Self {
        Self::Split {
            axis,
            slots: slots.into_iter().collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::request::{BlockId, KeySeq};
    use crate::value::Args;

    /// One of every node kind.
    ///
    /// [`every_node_kind_round_trips`] checks this list against [`Node::TAGS`],
    /// so a kind added without a sample is a failing test rather than an
    /// untested arm.
    fn samples() -> Vec<Node> {
        vec![
            Node::Empty {},
            Node::split(
                Axis::Rows,
                [Slot::new(
                    Constraint::Fill { weight: 1 },
                    Node::Buffer {
                        buffer: BufferId(1),
                        soft_wrap: false,
                    },
                )],
            ),
            Node::line([Node::Spring {}]),
            Node::Spring {},
            Node::Spacer { cells: 2 },
            Node::Shed {
                priority: 3,
                contracted: Some(Child::new(Node::Glyph {
                    glyph: Glyph::Check,
                    tone: Tone::Claude,
                })),
                child: Child::new(Node::Label {
                    text: "jj ✓".to_owned(),
                    tone: Tone::Meta,
                    emphasis: Emphasis::Plain,
                }),
            },
            Node::Pane {
                pane: PaneId(2),
                holds: PaneKind::Transcript,
                focused: true,
                child: Child::new(Node::Transcript {
                    follow: true,
                    folded: vec![TurnId(9)],
                }),
            },
            Node::TabBar {
                tabs: vec![Tab {
                    title: "src/retry.rs".to_owned(),
                    kind: PaneKind::Buffer,
                    unseen: 3,
                    active: true,
                }],
            },
            Node::ModeChip {
                label: "NORMAL".to_owned(),
                tone: Tone::Text,
            },
            Node::FileLabel {
                path: PathBuf::from("src/retry.rs"),
                dirty: true,
            },
            Node::Session {
                state: SessionState::Working,
                since: Some(Millis(1_000)),
                prose: true,
            },
            Node::Counter {
                glyph: Glyph::Unseen,
                count: 6,
                label: Some("unseen".to_owned()),
                tone: Tone::Claude,
            },
            Node::Divider {},
            Node::Label {
                text: "connection lost mid-turn".to_owned(),
                tone: Tone::Trouble,
                emphasis: Emphasis::Plain,
            },
            Node::Glyph {
                glyph: Glyph::Claude,
                tone: Tone::Claude,
            },
            Node::Spans {
                rows: vec![SpanRow {
                    runs: vec![Run::new("store", Tone::Steel)],
                    tint: Some(Tint::Selection),
                }],
            },
            Node::Spinner {
                since: Millis(1_700),
            },
            Node::Elapsed {
                since: Millis(1_700),
            },
            Node::Buffer {
                buffer: BufferId(1),
                soft_wrap: true,
            },
            Node::Gutter {
                buffer: BufferId(1),
            },
            Node::VirtualText {
                owner: Some(RegionId(4)),
                content: Child::new(Node::Watch { watch: WatchId(5) }),
            },
            Node::Picker {
                source: SourceId("files".to_owned()),
                filter: "retry".to_owned(),
                columns: vec![PickerColumn {
                    field: "path".to_owned(),
                    constraint: Constraint::Fill { weight: 1 },
                    tone: Tone::Text,
                }],
                preview: true,
            },
            Node::Diff {
                source: DiffSource::ReviewBlock { block: BlockId(7) },
                mode: DiffMode::Unified,
                grouping: Grouping::Directory,
            },
            Node::Question { ask: AskId(8) },
            Node::Transcript {
                follow: false,
                folded: Vec::new(),
            },
            Node::Prompt {
                prompt: PromptKind::Claude,
                text: "make the retry backoff configurable".to_owned(),
                anchor: Some(FileSpan {
                    path: PathBuf::from("src/retry.rs"),
                    span: None,
                }),
            },
            Node::KeyHints {
                density: Density::Footer,
                hints: vec![KeyHint {
                    key: KeySeq("s".to_owned()),
                    verb: "mark seen".to_owned(),
                }],
            },
            Node::Completion {},
            Node::Signature {},
            Node::Watch { watch: WatchId(5) },
        ]
    }

    #[test]
    fn every_node_kind_round_trips() {
        let mut covered = BTreeSet::new();
        for node in samples() {
            let encoded = node.to_value();
            let tag = encoded.tag().expect("a node encodes as a tagged record");
            covered.insert(tag.to_owned());
            assert_eq!(
                Node::from_value(&encoded).expect("a node decodes from its own encoding"),
                node,
                "`{tag}` did not survive the round trip"
            );
        }

        let declared: BTreeSet<String> = Node::TAGS.iter().map(|tag| (*tag).to_owned()).collect();
        assert_eq!(
            declared, covered,
            "samples() and Node::TAGS disagree — every node kind needs one sample"
        );
    }

    #[test]
    fn nesting_round_trips_through_child() {
        let tree = Tree::new(Node::split(
            Axis::Rows,
            [
                Slot::new(
                    Constraint::Fill { weight: 1 },
                    Node::Pane {
                        pane: PaneId(1),
                        holds: PaneKind::Buffer,
                        focused: true,
                        child: Child::new(Node::Buffer {
                            buffer: BufferId(1),
                            soft_wrap: false,
                        }),
                    },
                ),
                Slot::new(
                    Constraint::Cells { cells: 1 },
                    Node::line([
                        Node::ModeChip {
                            label: "NORMAL".to_owned(),
                            tone: Tone::Text,
                        },
                        Node::Spring {},
                        Node::Session {
                            state: SessionState::Idle,
                            since: None,
                            prose: true,
                        },
                    ]),
                ),
            ],
        ))
        .with_float(Float::new(
            Mood::Informational,
            Node::Spans {
                rows: vec![SpanRow::default()],
            },
        ));

        let encoded = tree.to_value();
        assert_eq!(Tree::from_value(&encoded).expect("a tree decodes"), tree);
    }

    #[test]
    fn the_empty_tree_draws_nothing() {
        assert_eq!(
            Tree::default(),
            Tree {
                root: Node::Empty {},
                float: None,
            }
        );
    }

    #[test]
    fn an_unknown_kind_names_every_tag_it_could_have_been() {
        let stray = Value::tagged("buffer-view", Args::new());
        let error = Node::from_value(&stray).expect_err("`buffer-view` is not a node kind");
        let WireError::Tag { got, expected } = error else {
            panic!("expected a tag error, got {error:?}");
        };
        assert_eq!(got, "buffer-view");
        assert_eq!(expected, Node::TAGS);
    }

    #[test]
    fn tags_are_unique_and_kebab_case() {
        let unique: BTreeSet<&&str> = Node::TAGS.iter().collect();
        assert_eq!(unique.len(), Node::TAGS.len(), "two node kinds share a tag");
        for tag in Node::TAGS {
            assert!(
                tag.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "`{tag}` is not kebab-case, which the doors and the mockups both are"
            );
        }
    }

    #[test]
    fn the_declared_shape_is_a_union_over_every_tag() {
        let ParamType::Union(variants) = <Node as Wire>::TYPE else {
            panic!("a node's declared shape is a tagged union");
        };
        let declared: Vec<&str> = variants.iter().map(|variant| variant.tag).collect();
        assert_eq!(declared, Node::TAGS);
    }

    /// Every union tag reachable from `ty`, with the field names of its arms.
    fn unions(ty: &ParamType, found: &mut Vec<(&'static str, &'static str)>) {
        match ty {
            ParamType::Union(variants) => {
                for variant in *variants {
                    for param in variant.fields {
                        found.push((variant.tag, param.name));
                        unions(&param.ty, found);
                    }
                }
            }
            ParamType::Record(params) => {
                for param in *params {
                    unions(&param.ty, found);
                }
            }
            ParamType::List(inner) => unions(inner, found),
            _ => {}
        }
    }

    /// A field named `kind` on a tagged union silently overwrites its own tag.
    ///
    /// Not hypothetical: `Node::Pane` and `Node::Prompt` were both written with
    /// a `kind` field, `Value::tagged` set the tag first and the field replaced
    /// it, and a pane holding the transcript encoded as a *transcript node*.
    /// Both round-trip tests caught it, but only because a sample happened to
    /// carry one — hence this, which cannot be dodged by an unlucky fixture.
    #[test]
    fn no_union_arm_shadows_the_tag_field() {
        let mut found = Vec::new();
        unions(&<Node as Wire>::TYPE, &mut found);
        unions(&<Tree as Wire>::TYPE, &mut found);
        let offenders: Vec<_> = found
            .iter()
            .filter(|(_, field)| *field == crate::value::TAG_FIELD)
            .collect();
        assert!(
            offenders.is_empty(),
            "these union arms declare a `{}` field, which overwrites their own tag: {offenders:?}",
            crate::value::TAG_FIELD,
        );
    }

    #[test]
    fn recursion_is_the_only_place_the_schema_says_any() {
        // `Child` is the wrapper the module docs name; nothing else may loosen
        // its declared type, or the schema stops describing the protocol.
        assert_eq!(<Child as Wire>::TYPE, ParamType::Any);
        assert_ne!(<Node as Wire>::TYPE, ParamType::Any);
    }

    /// [`Node::tag`] is a match and [`Wire::to_value`] is an encode; they are
    /// two ways of saying the same thing, and drift between them is how a decode
    /// error names the wrong kind.
    #[test]
    fn a_node_reports_the_same_tag_it_encodes_with() {
        for node in samples() {
            assert_eq!(node.to_value().tag(), Some(node.tag()));
        }
    }
}
