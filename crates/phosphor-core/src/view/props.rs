//! The leaves of the view tree — the vocabulary a node's props are drawn from.
//!
//! Split out from [`super`] for the same reason [`crate::request`] is split out
//! of [`crate::action`]: the tree is one enum and one recursion rule, and
//! everything it is *parameterised by* is here. Both halves are `spine`'s.
//!
//! Two of these types are laws rather than conveniences:
//!
//! * [`Tone`] is the only way a node names a colour. There are no RGB values in
//!   this protocol and there can never be one — `scripts/lint-no-literal-colours.sh`
//!   already forbids them in `phosphor-ui` outside `theme.rs`, and a view tree
//!   carrying `#3ddc97` would route around that lint through Steel. A tone names
//!   an *actor or a state*; the theme resolves it (Design Language §1).
//! * [`Glyph`] names Design Language §2's lexicon — *one cell, one concept*. It
//!   is a named vocabulary, not a fence: [`Node::Label`](super::Node::Label)
//!   carries arbitrary text and always will. What it buys is that the common
//!   case cannot drift — a surface that wants `●` asks for [`Glyph::Unseen`],
//!   and a glyph the lexicon does not name is a Design Language change rather
//!   than a string literal in a `.scm` file.
//!
//! Owned by `spine`.

use crate::registry::ParamType;
use crate::request::{BlockId, BufferId, ChangeId, HunkId, KeySeq, PaneKind};
use crate::value::{Value, Wire, WireError, wire_choice, wire_record, wire_union};

// ---------------------------------------------------------------------------
// Colour, glyph, emphasis
// ---------------------------------------------------------------------------

/// A colour role. **The only way this protocol names a colour.**
///
/// Every value here is a role in `phosphor-ui`'s `Theme` — the six actors of
/// Design Language §1 and the neutral ramp beneath them (`theme.rs:85-129`).
/// Composition says *what a thing is*; the theme says what colour that is, and
/// swapping themes cannot change what a green pixel means (§10: *"a theme owns
/// lightness and syntax colours; it never owns actor identity"*).
///
/// **Syntax colours are deliberately absent.** They are the highlighter's, keyed
/// off tree-sitter captures inside `BufferView`; a composition that could pick
/// them would be writing a second highlighter in scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    /// `claude` — his edits, his marks, his voice. Green always means claude.
    Claude,
    /// `you` — insert mode, your side of a diff, watches.
    You,
    /// `attention` — waiting, paused, dirty, permission.
    Attention,
    /// `trouble` — deletions, failures, disconnects.
    Trouble,
    /// `transient` — visual mode, spinners, types.
    Transient,
    /// `steel` — the REPL, functions, scripting.
    Steel,
    /// Ordinary foreground text.
    Text,
    /// Prose — claude's own voice, and any sentence rather than datum.
    Prose,
    /// Meta-grey — separators, hints, secondary facts.
    Meta,
    /// The line-number column's grey.
    LineNumber,
    /// The ground the frame is painted on.
    Ground,
    /// The brightest text the theme has — a selected row, a focused tab.
    BrightText,
    /// What is behind a float. Rust applies it; a node asks for it only when it
    /// is deliberately drawing something recessive.
    Dimmed,
}

wire_choice!(Tone {
    Claude => "claude",
    You => "you",
    Attention => "attention",
    Trouble => "trouble",
    Transient => "transient",
    Steel => "steel",
    Text => "text",
    Prose => "prose",
    Meta => "meta",
    LineNumber => "line-number",
    Ground => "ground",
    BrightText => "bright-text",
    Dimmed => "dimmed",
});

/// Design Language §2's glyph lexicon — *one cell, one concept*.
///
/// All single-cell, Nerd-Font-free, present in default terminal fonts. The
/// rendering — and the degradation of the ones that need it, `▎` for the state
/// bar and a static `✻` for the spinner (§8) — is `phosphor-ui`'s; this names
/// the concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    /// `✻` — claude, and anything he produced.
    Claude,
    /// `⠸` — working. Animated by [`Node::Spinner`](super::Node::Spinner); this
    /// is the still form, for a degraded terminal or a static caption.
    Working,
    /// `!` — needs you: a question or a permission ask.
    NeedsYou,
    /// `⏸` — paused.
    Paused,
    /// `✱` — changed on disk.
    ChangedOnDisk,
    /// `✕` — session lost.
    SessionLost,
    /// `■` — a diagnostic.
    Diagnostic,
    /// `●` — an unseen count, on a file, a tab or an inbox row.
    Unseen,
    /// `⚓` — an anchor: a thread, or a selection riding the prompt line.
    Anchor,
    /// `┊` — the virtual-text margin rail.
    VirtualRail,
    /// `◉` — a watch.
    Watch,
    /// `⇒` — a value in a watch's stream.
    ValueStream,
    /// `▸` — a closed fold, and the transcript's tool rows.
    FoldClosed,
    /// `▾` — an open fold.
    FoldOpen,
    /// `⋯` — elided content.
    Elided,
    /// `λ` — the steel prompt.
    SteelPrompt,
    /// `◆` — a steel surface.
    SteelSurface,
    /// `❯` — a prompt line, and the source half of a float header.
    Prompt,
    /// `✓` — seen, clean, passed. `jj ✓` and a picker row's seen mark.
    Check,
    /// `↪` — a soft-wrap continuation.
    WrapContinuation,
}

wire_choice!(Glyph {
    Claude => "claude",
    Working => "working",
    NeedsYou => "needs-you",
    Paused => "paused",
    ChangedOnDisk => "changed-on-disk",
    SessionLost => "session-lost",
    Diagnostic => "diagnostic",
    Unseen => "unseen",
    Anchor => "anchor",
    VirtualRail => "virtual-rail",
    Watch => "watch",
    ValueStream => "value-stream",
    FoldClosed => "fold-closed",
    FoldOpen => "fold-open",
    Elided => "elided",
    SteelPrompt => "steel-prompt",
    SteelSurface => "steel-surface",
    Prompt => "prompt",
    Check => "check",
    WrapContinuation => "wrap-continuation",
});

/// How a run of text is weighted.
///
/// Four values, each with exactly one documented use, because the design
/// language is blunt about how little of this there is: *"the mode chip is the
/// only inverted text on screen"* (§5), and undercurl is §3's half of the
/// anchored-region treatment. There is no bold and no italic — a terminal that
/// has them renders them inconsistently, and colour already carries the meaning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Emphasis {
    /// Foreground colour only.
    #[default]
    Plain,
    /// Background and foreground swapped. The mode chip, and nothing else.
    Inverted,
    /// Underlined — an OSC 8 jump link in the transcript.
    Underline,
    /// Undercurled, degrading to underline on a terminal without it (`T085`).
    /// Diagnostics, and the anchored-region treatment of §3.
    Undercurl,
}

wire_choice!(Emphasis {
    Plain => "plain",
    Inverted => "inverted",
    Underline => "underline",
    Undercurl => "undercurl",
});

/// A whole-row background tint (Design Language §3).
///
/// Three, and the palette has exactly three (`theme.rs:142-156`). A row with no
/// tint carries the ground, which is [`Option::None`] rather than a fourth
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tint {
    /// `#141d16` — an anchored region.
    Anchor,
    /// `#26332a` — the selected row inside a float.
    Selection,
    /// `#211114` — a failure.
    Failure,
}

wire_choice!(Tint {
    Anchor => "anchor",
    Selection => "selection",
    Failure => "failure",
});

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Which way a [`Node::Split`](super::Node::Split) divides its area.
///
/// Not [`Direction`](crate::request::Direction), which is a compass bearing for
/// *where a new pane goes* and *which pane focus moves to* — an Action payload.
/// This is the axis of an existing division, and the two are different
/// questions: `split-pane right` produces a `Columns` split, and so does
/// `split-pane left`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Children stack top to bottom; each constraint measures rows.
    Rows,
    /// Children sit left to right; each constraint measures columns.
    Columns,
}

wire_choice!(Axis {
    Rows => "rows",
    Columns => "columns",
});

/// How much of a split's area one child gets.
///
/// Deliberately the same five shapes ratatui's own solver takes, named
/// independently: `phosphor-core` has no ratatui dependency and must not grow
/// one (Q12), so this is a mirror the interpreter (`T079`) converts at the seam.
/// A sixth shape here that ratatui cannot express would be a protocol that
/// cannot be drawn.
///
/// Counted in `u32` rather than the `u16` a terminal coordinate actually is,
/// because the wire model declares one unsigned width ([`crate::value`]) and
/// narrowing at the seam is a saturating cast the interpreter makes once. A
/// protocol that grew a wire case to save two bytes would be paying in the
/// wrong currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constraint {
    /// Exactly this many cells.
    Cells {
        /// Rows or columns, per the split's [`Axis`].
        cells: u32,
    },
    /// At least this many cells.
    Min {
        /// The floor.
        cells: u32,
    },
    /// At most this many cells.
    Max {
        /// The ceiling.
        cells: u32,
    },
    /// This percentage of the area.
    Percent {
        /// 0–100.
        percent: u32,
    },
    /// Whatever is left, shared between the `Fill` children in proportion to
    /// their weights.
    Fill {
        /// This child's share.
        weight: u32,
    },
}

wire_union!(Constraint {
    Cells => "cells", "exactly this many rows or columns" {
        cells: u32 = "how many cells",
    },
    Min => "min", "at least this many rows or columns" {
        cells: u32 = "the floor, in cells",
    },
    Max => "max", "at most this many rows or columns" {
        cells: u32 = "the ceiling, in cells",
    },
    Percent => "percent", "this share of the area" {
        percent: u32 = "0-100",
    },
    Fill => "fill", "whatever is left, shared by weight between the fill children" {
        weight: u32 = "this child's share of the remainder",
    },
});

/// One child of a [`Node::Split`](super::Node::Split), with the area it asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// How much of the split's area this child gets.
    pub constraint: Constraint,
    /// What goes in it.
    pub child: super::Child,
}

wire_record!(Slot {
    constraint: Constraint = "how much of the split's area this child gets",
    child: super::Child = "the node that fills it",
});

impl Slot {
    /// A slot holding `node` under `constraint`.
    #[must_use]
    pub fn new(constraint: Constraint, node: super::Node) -> Self {
        Self {
            constraint,
            child: super::Child::new(node),
        }
    }
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// A point on the host's monotonic clock, in milliseconds.
///
/// The two time-derived nodes ([`Node::Spinner`](super::Node::Spinner),
/// [`Node::Elapsed`](super::Node::Elapsed)) take one of these rather than a
/// frame counter or a pre-formatted string, and `action.rs`'s *"what is
/// deliberately not an Action"* section is why: a spinner frame and a 1s tick
/// have no actor and nothing to refuse, so they are not mutations, so they do
/// not bump a [`Revision`](crate::query::Revision), so they must not re-enter
/// the VM. Rust reads the clock each frame and renders the difference from this
/// mark; the tree that named the mark stays valid and cached (`T079`).
///
/// Not [`std::time::Instant`]: this crosses a door (an agent-built pane may
/// carry one), and an `Instant` has no wire form. The epoch is the app's own —
/// whatever `phosphor-term` starts its clock at — because only differences are
/// ever rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Millis(pub u64);

impl Wire for Millis {
    const TYPE: ParamType = ParamType::Uint;

    fn to_value(&self) -> Value {
        Value::Int(i64::try_from(self.0).unwrap_or(i64::MAX))
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        u64::from_value(value).map(Self)
    }
}

// ---------------------------------------------------------------------------
// Styled text
// ---------------------------------------------------------------------------

/// One styled run of text.
///
/// The unit the `spans` escape hatch is built from, and the reason the hatch is
/// *one* grep-able name: a surface drawn out of [`Run`]s is a surface with no
/// primitive of its own, which is exactly the thing to look at when a
/// frame-budget regression appears (Q12's accepted cost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The text. Rendered as-is; the interpreter measures it in grapheme
    /// clusters and never wraps it.
    pub text: String,
    /// Its colour role.
    pub tone: Tone,
    /// Its weight.
    pub emphasis: Emphasis,
}

wire_record!(Run {
    text: String = "the text of this run",
    tone: Tone = "which actor or state colours it",
    emphasis: Emphasis = "plain, inverted, underlined or undercurled",
});

impl Run {
    /// A plain run in `tone`.
    #[must_use]
    pub fn new(text: &str, tone: Tone) -> Self {
        Self {
            text: text.to_owned(),
            tone,
            emphasis: Emphasis::Plain,
        }
    }
}

/// One row of the `spans` hatch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpanRow {
    /// Left to right.
    pub runs: Vec<Run>,
    /// A whole-row background, or the ground.
    pub tint: Option<Tint>,
}

wire_record!(SpanRow {
    runs: Vec<Run> = "the styled runs of this row, left to right",
    tint: Option<Tint> = "a whole-row background tint, or absent for the ground",
});

// ---------------------------------------------------------------------------
// Chrome
// ---------------------------------------------------------------------------

/// The session's state, as the statusline and the transcript both render it.
///
/// Design Language §5: *"`SessionState` is ONE enum rendered identically
/// everywhere it appears"* — so it is one enum *in the protocol*, and a surface
/// that draws it differently is a bug the type cannot express.
///
/// **Flagged seam, not folded in.** `phosphor-ui`'s `status_line::SessionState`
/// (`status_line.rs:186`) is the same enum, written first, on the widget side.
/// Two definitions of one type will drift; the canonical one belongs here, since
/// `phosphor-core` cannot depend on `phosphor-ui` and this is the module both
/// Steel and the interpreter read. Collapsing them deletes a `surface`-owned
/// definition, so it is a request to `surface` rather than an edit `spine`
/// makes — the same call `request::ScrollRequest` recorded for the same reason.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// No session. Renders nothing at all, and is not an error.
    #[default]
    None,
    /// Attached, nothing in flight.
    Idle,
    /// A turn is running. Pair it with `since` for the elapsed counter.
    Working,
    /// Claude asked something and is waiting (Q9's queued ask sets the flag).
    Waiting,
    /// Paused at a tool boundary.
    Paused,
    /// The transport dropped. Trouble on the statusline never blocks editing.
    Lost,
}

wire_choice!(SessionState {
    None => "none",
    Idle => "idle",
    Working => "working",
    Waiting => "waiting",
    Paused => "paused",
    Lost => "lost",
});

/// One tab of the tab bar (`T089`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    /// What it says — a path, or `transcript`.
    pub title: String,
    /// What the pane holds; the tab's actor colour follows it.
    pub kind: PaneKind,
    /// Unseen regions in it. Zero renders no counter.
    pub unseen: u32,
    /// Whether this is the focused pane: a 2-cell actor-coloured top rule and
    /// bright text, against meta-grey for the rest.
    pub active: bool,
}

wire_record!(Tab {
    title: String = "the tab's label",
    kind: PaneKind = "what the pane holds",
    unseen: u32 = "unseen regions in it; zero draws no counter",
    active: bool = "whether this is the focused pane",
});

/// One entry of a keymap surface.
///
/// The same datum at all three densities of [`Density`] — Design Language §12:
/// *"KeymapFooter … also renders the which-key grid — same data, two
/// densities"*, and `T086`'s `HelpGrid` is the third.
///
/// The verb spells the whole command (§6: *"never cryptic contractions like
/// `:ca`"*); abbreviation is a typing affordance, never a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
    /// The key, in vim notation.
    pub key: KeySeq,
    /// What it does, spelled out.
    pub verb: String,
}

wire_record!(KeyHint {
    key: KeySeq = "the key sequence, in vim notation",
    verb: String = "what it does, spelled out in full",
});

/// Which density a keymap surface renders at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Density {
    /// A float's footer strip: primary action first, `esc` last (§4).
    Footer,
    /// The `SPC` leader grid (`3c`).
    Grid,
    /// The `:help` float body — a full grid with grammar (`6d`, `T086`).
    Help,
}

wire_choice!(Density {
    Footer => "footer",
    Grid => "grid",
    Help => "help",
});

// ---------------------------------------------------------------------------
// Float chrome
// ---------------------------------------------------------------------------

/// A float's mood, which is the only thing its border colour means (§4).
///
/// **Flagged seam**, on the same terms as [`SessionState`]: `float.rs:93` is the
/// widget-side enum and carries two of these — `T038` adds `Passive`. The
/// protocol names all three from the start, because a mood the tree cannot say
/// is a float Steel cannot open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mood {
    /// `#2a5c44` — pickers, help, diffs: anything you asked for.
    Informational,
    /// `#6b5426` with a `#171207` body — questions and permission asks.
    NeedsYou,
    /// `#2a3c2e` — completion. **No footer**, §4's one documented exception
    /// (`T038`).
    Passive,
}

wire_choice!(Mood {
    Informational => "informational",
    NeedsYou => "needs-you",
    Passive => "passive",
});

/// A float's header: *"source or command · meta right"* (§4).
///
/// Two strings rather than two nodes. The header is text by contract, its colour
/// comes from the mood (`float.rs:133-138`), and the drop rule when both halves
/// cannot fit — meta first — is the widget's, not composition's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatHeader {
    /// The source or command: `❯ files`, `✻ claude · wants to run`.
    pub left: String,
    /// Right-aligned meta, in meta-grey. Dropped before the left half.
    pub right: Option<String>,
}

wire_record!(FloatHeader {
    left: String = "the source or command, on the left",
    right: Option<String> = "right-aligned meta, dropped first when it cannot fit",
});

impl FloatHeader {
    /// A header with no meta half.
    #[must_use]
    pub fn new(left: &str) -> Self {
        Self {
            left: left.to_owned(),
            right: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Body parameters
// ---------------------------------------------------------------------------

/// One column of a picker's list.
///
/// Q12 puts *"picker columns"* in the composition layer explicitly, so a source
/// supplies fields and the tree decides which of them are shown, how wide, and
/// in whose colour — which is how `2a`'s files picker grows activity columns
/// without a Rust edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerColumn {
    /// The field of the row record to render.
    pub field: String,
    /// How much width it gets.
    pub constraint: Constraint,
    /// Its colour role.
    pub tone: Tone,
}

wire_record!(PickerColumn {
    field: String = "which field of the row record to render",
    constraint: Constraint = "how much width the column gets",
    tone: Tone = "which actor or state colours it",
});

/// What a [`Node::Diff`](super::Node::Diff) is showing.
///
/// A union rather than four optional ids, so *"a diff of nothing"* and *"a diff
/// of two things at once"* are both unrepresentable. The four arms are the four
/// surfaces the mockups draw: a review block (`4b`), one hunk peeked at (`2b`),
/// the unsaved buffer against what claude wrote to disk (`5b`), and a change out
/// of the VCS timeline (`3b`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSource {
    /// A claude-declared review block (`T053`).
    ReviewBlock {
        /// The block.
        block: BlockId,
    },
    /// One hunk, peeked (`T066`).
    Hunk {
        /// The hunk.
        hunk: HunkId,
    },
    /// Your unsaved buffer against disk — `:diff-disk` (`T070`).
    Disk {
        /// The buffer whose disk copy moved.
        buffer: BufferId,
    },
    /// A VCS change (`T073`).
    Change {
        /// The change or commit, as the backend spells it.
        change: ChangeId,
    },
}

wire_union!(DiffSource {
    ReviewBlock => "review-block", "a claude-declared review block" {
        block: BlockId = "the block",
    },
    Hunk => "hunk", "one hunk, peeked" {
        hunk: HunkId = "the hunk",
    },
    Disk => "disk", "your unsaved buffer against what is on disk" {
        buffer: BufferId = "the buffer whose disk copy moved",
    },
    Change => "change", "a change out of the VCS timeline" {
        change: ChangeId = "the change or commit id",
    },
});
