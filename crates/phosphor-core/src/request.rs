//! The payload vocabulary — the nouns every [`Action`](crate::action::Action)
//! and [`Query`](crate::query::Query) is parameterised by.
//!
//! Split out from `action.rs` deliberately: **this module is readable by every
//! crate, including `phosphor-ui`.** `phosphor_core::store` is forbidden there
//! (`scripts/lint-no-store-mutation.sh`) and `phosphor_core::action` should be
//! too — a widget that can *construct* a mutation is one refactor away from
//! applying one. But a widget legitimately needs to name a [`Position`], a
//! [`Span`] or a [`ScrollRequest`] in the ViewModel it renders, and those are
//! here.
//!
//! Every type in this module implements [`Wire`], which is
//! what makes "no Action carries an argument that cannot cross MCP" structural
//! rather than aspirational.
//!
//! Owned by `spine`.

use std::path::PathBuf;

use crate::value::{Wire, wire_choice, wire_record, wire_union};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Declares an opaque id newtype and its wire form (a non-negative integer).
macro_rules! ids {
    ($( $(#[$meta:meta])* $name:ident = $wire:literal ),* $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(pub u64);

            impl Wire for $name {
                const TYPE: crate::registry::ParamType = crate::registry::ParamType::Id($wire);

                fn to_value(&self) -> crate::value::Value {
                    crate::value::Value::Int(i64::try_from(self.0).unwrap_or(i64::MAX))
                }

                fn from_value(
                    value: &crate::value::Value,
                ) -> Result<Self, crate::value::WireError> {
                    u64::from_value(value).map(Self)
                }
            }
        )*
    };
}

ids! {
    /// An open buffer. Not a path: the same file can be open once and renamed.
    BufferId = "buffer",
    /// A pane in the split tree (`T088`).
    PaneId = "pane",
    /// A region — one claude-authored span, in one of the two states of Design
    /// Language §7 (`T041`).
    RegionId = "region",
    /// A resolved anchor: node-tier where the language has a grammar, line +
    /// content fallback where it does not (`T042`, `T043`).
    AnchorId = "anchor",
    /// An anchored exchange (`3a`, `T068`).
    ThreadId = "thread",
    /// A placed watch (`T074`).
    WatchId = "watch",
    /// A queued question or permission ask (Q9, `T060`).
    AskId = "ask",
    /// One hunk of a diff (`T064`).
    HunkId = "hunk",
    /// A claude-declared review block (`T053`).
    BlockId = "block",
    /// A group inside a review block — a directory, or claude's own grouping
    /// (`8b`'s "mechanical" versus "the meat", `T065`).
    GroupId = "group",
    /// One inbox item (`5c`, `T067`).
    InboxId = "inbox-item",
    /// One agent turn (`T054`).
    TurnId = "turn",
    /// One tool call inside a turn (`1b`'s transcript rows, `T056`).
    ToolCallId = "tool-call",
    /// A point in a buffer's undo tree (`T029`).
    CheckpointId = "checkpoint",
}

/// A named text identifier that is not a number: a picker source, a language, a
/// theme slug, a surface.
macro_rules! text_ids {
    ($( $(#[$meta:meta])* $name:ident ),* $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(pub String);

            impl Wire for $name {
                const TYPE: crate::registry::ParamType = crate::registry::ParamType::Text;

                fn to_value(&self) -> crate::value::Value {
                    crate::value::Value::Text(self.0.clone())
                }

                fn from_value(
                    value: &crate::value::Value,
                ) -> Result<Self, crate::value::WireError> {
                    String::from_value(value).map(Self)
                }
            }
        )*
    };
}

text_ids! {
    /// A picker source, defined in Steel (`T046`). A key, not a Rust variant —
    /// redefining one at the REPL re-derives an open picker, which a Rust enum
    /// could not express.
    SourceId,
    /// A language, as `define-language` names it (`T037`).
    LanguageId,
    /// A theme slug — `"phosphor-dark"`, `"tokyo-night"` (`T012`).
    ThemeSlug,
    /// A float surface.
    ///
    /// **A registry key, not a Rust enum**, and this is a decision with a test
    /// behind it: `T048` requires `:arch` to be built entirely from the `spans`
    /// hatch and to add *zero lines* to `phosphor-ui`. A `FloatKind` enum would
    /// make every new Steel surface a Rust edit — the "config file with a Rust
    /// editor hiding behind it" that `CP-2` judges. The cost is that a mistyped
    /// id is a runtime error; it opens the same error float `T021` already
    /// needs for a broken `init.scm`.
    SurfaceId,
    /// A VCS change or commit, as the backend spells it — a jj change id or a
    /// git SHA (`T071`, `T072`).
    ChangeId,
    /// A named register: `"a` in `"ayy`. Text rather than `char` because `"+`
    /// and `"*` exist.
    RegisterName,
}

/// A key sequence in vim notation — `"<C-q>"`, `"SPC f"`, `"]u"`.
///
/// Text on the wire and parsed in Rust at `T026`. A structured `KeyEvent` on the
/// wire would put crossterm's shape into the MCP schema, and the mockups
/// (`3c`, `6b`) already write keys this way in the drawings a user reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeySeq(pub String);

impl Wire for KeySeq {
    const TYPE: crate::registry::ParamType = crate::registry::ParamType::Text;

    fn to_value(&self) -> crate::value::Value {
        crate::value::Value::Text(self.0.clone())
    }

    fn from_value(value: &crate::value::Value) -> Result<Self, crate::value::WireError> {
        String::from_value(value).map(Self)
    }
}

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// A point in a buffer, **1-based in both axes**, as the statusline draws it
/// (`12:1` in `1a`).
///
/// Byte offsets stay inside `phosphor-buffer`, and the UTF-16 conversion the LSP
/// wants stays at that seam (`T036`). A door never sees either: an agent that
/// has to count bytes to name a line is an agent that will get it wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// 1-based line.
    pub line: u32,
    /// 1-based column, in characters.
    pub column: u32,
}

wire_record!(Position {
    line: u32 = "1-based line number",
    column: u32 = "1-based column, counted in characters",
});

/// A half-open span between two [`Position`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// First position inside the span.
    pub start: Position,
    /// First position after it.
    pub end: Position,
}

wire_record!(Span {
    start: Position = "first position inside the span",
    end: Position = "first position after the span",
});

/// A file, optionally narrowed to a span — the shape a review block, a
/// diagnostic and a jump target all need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpan {
    /// Workspace-relative path.
    pub path: PathBuf,
    /// The span, or the whole file.
    pub span: Option<Span>,
}

wire_record!(FileSpan {
    path: PathBuf = "workspace-relative path",
    span: Option<Span> = "the span, or absent for the whole file",
});

// ---------------------------------------------------------------------------
// Who asked
// ---------------------------------------------------------------------------

/// Who is asking.
///
/// On the envelope ([`Request`](crate::action::Request)) rather than in a
/// payload, because Design Language §7 is unconditional: *"your own edits never
/// create regions: the machine tracks claude only."* If the actor is not in the
/// dispatch path from the first commit, the region state machine (`T041`) has to
/// infer it, and retrofitting provenance through the whole vocabulary later is
/// the expensive version of this fix.
///
/// It is also an honest record of a real trust surface: any door can *claim* an
/// author (see [`RegionSpec`]), so the store keeps both — who asked, and what
/// was claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Actor {
    /// The person at the keyboard. Blue in every theme.
    You,
    /// The agent. Green in every theme, and the only author whose writes create
    /// regions.
    Claude,
    /// `runtime/*.scm` acting on its own behalf — a hook, a keymap thunk.
    Steel,
    /// A shell invocation of `phosphor`.
    Cli,
    /// The editor itself: disk watchers, LSP pushes, the session transport.
    System,
}

wire_choice!(Actor {
    You => "you",
    Claude => "claude",
    Steel => "steel",
    Cli => "cli",
    System => "system",
});

// ---------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------

/// What an Action applies to.
///
/// Late-bound on purpose. `s` in `runtime/keymaps.scm` binds to
/// `(mark-seen! 'selection)` and stays correct as the selection changes; the
/// alternative — query, then act on the result — has a window between the two
/// where the answer goes stale.
///
/// The cost is that four of these arms mean something different depending on
/// where focus is, so **the MCP door refuses them**
/// ([`Target::focus_relative`], [`Refusal::FocusRelativeTargetOverMcp`](crate::action::Refusal::FocusRelativeTargetOverMcp)).
/// An agent has no cursor; letting it act on "the selection" is how an agent
/// silently edits whatever the user happened to be looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Wherever the cursor is. Focus-relative.
    Cursor {},
    /// The live visual selection. Focus-relative.
    Selection {},
    /// The highlighted picker row. Focus-relative.
    PickerRow {},
    /// The highlighted row of the focused float. Focus-relative.
    FloatRow {},
    /// A buffer, whole.
    Buffer {
        /// Which buffer.
        id: BufferId,
    },
    /// A file on disk, whether or not it is open.
    File {
        /// Workspace-relative path.
        path: PathBuf,
    },
    /// A path and a span, named outright. The only arm an agent can always use.
    Explicit {
        /// Workspace-relative path.
        path: PathBuf,
        /// The span.
        span: Span,
    },
    /// One region.
    Region {
        /// Which region.
        id: RegionId,
    },
    /// One anchor and whatever hangs off it.
    Anchor {
        /// Which anchor.
        id: AnchorId,
    },
    /// One diff hunk.
    Hunk {
        /// Which hunk.
        id: HunkId,
    },
    /// A whole review block — `S here marks all 12` (`8b`).
    Block {
        /// Which block.
        id: BlockId,
    },
    /// A group inside a block: a directory, or claude's own grouping.
    Group {
        /// Which group.
        id: GroupId,
    },
    /// One thread.
    Thread {
        /// Which thread.
        id: ThreadId,
    },
    /// One inbox item. `CP-8a` requires unread to *derive* from seen-state, so
    /// this is how an inbox item is marked read — there is no separate Action.
    InboxItem {
        /// Which item.
        id: InboxId,
    },
    /// One watch.
    Watch {
        /// Which watch.
        id: WatchId,
    },
}

wire_union!(Target {
    Cursor => "cursor", "wherever the cursor is — focus-relative, refused over MCP" {},
    Selection => "selection", "the live visual selection — focus-relative, refused over MCP" {},
    PickerRow => "picker-row", "the highlighted picker row — focus-relative, refused over MCP" {},
    FloatRow => "float-row", "the highlighted row of the focused float — focus-relative, refused over MCP" {},
    Buffer => "buffer", "a whole open buffer" {
        id: BufferId = "which buffer",
    },
    File => "file", "a file on disk, open or not" {
        path: PathBuf = "workspace-relative path",
    },
    Explicit => "explicit", "a path and a span, named outright" {
        path: PathBuf = "workspace-relative path",
        span: Span = "the span",
    },
    Region => "region", "one region" {
        id: RegionId = "which region",
    },
    Anchor => "anchor", "one anchor and what hangs off it" {
        id: AnchorId = "which anchor",
    },
    Hunk => "hunk", "one diff hunk" {
        id: HunkId = "which hunk",
    },
    Block => "block", "a whole review block" {
        id: BlockId = "which block",
    },
    Group => "group", "a group inside a review block" {
        id: GroupId = "which group",
    },
    Thread => "thread", "one anchored exchange" {
        id: ThreadId = "which thread",
    },
    InboxItem => "inbox-item", "one inbox item" {
        id: InboxId = "which item",
    },
    Watch => "watch", "one placed watch" {
        id: WatchId = "which watch",
    },
});

impl Target {
    /// Whether this target means something different depending on where focus
    /// is.
    ///
    /// The MCP door refuses these. Not a policy toggle — the answer is a
    /// property of the target itself, so the dispatcher can decide without
    /// consulting a table.
    #[must_use]
    pub const fn focus_relative(&self) -> bool {
        matches!(
            self,
            Self::Cursor {} | Self::Selection {} | Self::PickerRow {} | Self::FloatRow {}
        )
    }
}

/// Which pane an Action lands in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneRef {
    /// The pane that has focus.
    Focused {},
    /// A named pane.
    Id {
        /// Which pane.
        id: PaneId,
    },
    /// The pane in a compass direction from the focused one.
    Direction {
        /// Which way.
        direction: Direction,
    },
    /// The next pane in cycle order.
    Next {},
    /// The previous pane in cycle order.
    Prev {},
}

wire_union!(PaneRef {
    Focused => "focused", "the pane that has focus" {},
    Id => "id", "a named pane" {
        id: PaneId = "which pane",
    },
    Direction => "direction", "the pane in a compass direction from the focused one" {
        direction: Direction = "which way",
    },
    Next => "next", "the next pane in cycle order" {},
    Prev => "prev", "the previous pane in cycle order" {},
});

/// A compass direction, for splits and pane focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

wire_choice!(Direction {
    Up => "up",
    Down => "down",
    Left => "left",
    Right => "right",
});

/// Which way a sequence is walked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    /// The next one after here.
    Next,
    /// The previous one before here.
    Prev,
    /// The first, wherever the cursor is.
    First,
    /// The last.
    Last,
}

wire_choice!(Seek {
    Next => "next",
    Prev => "prev",
    First => "first",
    Last => "last",
});

// ---------------------------------------------------------------------------
// The viewport
// ---------------------------------------------------------------------------

/// The only thing that can move a viewport.
///
/// **Invariant 3 in one type.** Every arm is a caller saying what it wants; none
/// is a side effect of drawing. `T026` deleted the second writer — the vendored
/// core's `focus()` moved the viewport on every keystroke, and `Editor::input`
/// is gone — so this is now the only thing that moves one.
///
/// **This is the only definition.** `phosphor-ui`'s `buffer_view::ScrollRequest`
/// was a second copy of this shape, written first, and was collapsed to a
/// `pub use` of this type during the `CP-3` repairs. The 1-based-`u32` here to
/// 0-based-`usize` conversion the widget needs happens in exactly one place,
/// `Viewport::scrolled`'s `index_of`, rather than at a host boundary. The
/// canonical definition belongs here because `phosphor-core` cannot depend on
/// `phosphor-ui`, and `request` is importable from a widget where `action` is
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollRequest {
    /// Relative, in rows. Negative scrolls towards the top of the buffer.
    Rows {
        /// How many rows.
        rows: i64,
    },
    /// Relative, in screenfuls.
    Pages {
        /// How many screenfuls.
        pages: i64,
    },
    /// Relative, in columns. Negative scrolls left.
    Columns {
        /// How many columns.
        columns: i64,
    },
    /// Absolute: put this visual row at the top.
    ToRow {
        /// 1-based visual row.
        row: u32,
    },
    /// The first screenful.
    ToTop {},
    /// The last screenful.
    ToBottom {},
    /// Bring `row` inside the viewport with at least `margin` rows of context,
    /// moving as little as possible — and not at all if it is already there.
    ///
    /// **This is the whole of "follow the cursor", and it is a request.**
    RevealRow {
        /// 1-based visual row to reveal.
        row: u32,
        /// Rows of context to keep on the side it enters from.
        margin: u32,
    },
}

wire_union!(ScrollRequest {
    Rows => "rows", "relative, in rows; negative scrolls towards the top" {
        rows: i64 = "how many rows",
    },
    Pages => "pages", "relative, in screenfuls" {
        pages: i64 = "how many screenfuls",
    },
    Columns => "columns", "relative, in columns; negative scrolls left" {
        columns: i64 = "how many columns",
    },
    ToRow => "to-row", "absolute: put this visual row at the top" {
        row: u32 = "1-based visual row",
    },
    ToTop => "to-top", "the first screenful" {},
    ToBottom => "to-bottom", "the last screenful" {},
    RevealRow => "reveal-row", "bring a row into view, moving as little as possible" {
        row: u32 = "1-based visual row",
        margin: u32 = "rows of context to keep",
    },
});

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// The input machine's mode (`T026`).
///
/// **Not the statusline chip.** `phosphor-ui`'s `status_line::Mode`
/// (`status_line.rs:107-118`) has four values — Normal, Insert, Visual, Paused —
/// and the mockups draw at least four more labels in the same inverted chip:
/// `V-LINE` (`TUI Mockups.dc.html:1311`), `REVIEW` (`:207`, `:753`), `DISKDIFF`
/// (`:633`), `REPL` (`:507`). Two of those are surfaces and one (`PAUSED`) is a
/// *session* state. So the chip renders a **surface label**, of which this enum
/// is one source, and `set-mode` must never be the writer of `PAUSED`.
/// Widening the chip is a `surface` edit — flagged at `T019`, not folded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    /// Keys are commands.
    Normal,
    /// Keys are text.
    Insert,
    /// Characterwise selection.
    VisualChar,
    /// Linewise selection — the chip's `V-LINE`.
    VisualLine,
    /// Blockwise selection.
    VisualBlock,
    /// Replace.
    Replace,
    /// An operator is waiting for a motion or a text object — `SPC pending`
    /// (`3c`).
    OperatorPending,
}

wire_choice!(EditMode {
    Normal => "normal",
    Insert => "insert",
    VisualChar => "visual-char",
    VisualLine => "visual-line",
    VisualBlock => "visual-block",
    Replace => "replace",
    OperatorPending => "operator-pending",
});

/// A cursor motion. Counts ride on the Action, not here.
///
/// # The four that need a character, and why it does not ride here
///
/// `f`, `F`, `t` and `T` each take the character to search for, and this stays
/// a payload-free [`wire_choice`] anyway. A payload-carrying arm would make
/// [`crate::registry::ParamType::Choice`] the wrong type for `motion` and break
/// the CLI's flag value and the MCP schema's enum in the same edit — so the
/// character rides the way `SelectObject`'s delimiter already does: **beside**
/// the motion, not inside it. The input machine holds it between the `f` and
/// the key that names it ([`super::input::Machine`]), which is the same shape
/// as `"` naming a register.
///
/// A find motion asked of [`super::input::text::cursor_after`] with no
/// character stays where it started, exactly as [`Motion::SearchNext`] does
/// with no search state: a motion that invented a destination would be worse
/// than one that does not move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One character left.
    CharLeft,
    /// One character right.
    CharRight,
    /// One line up.
    LineUp,
    /// One line down.
    LineDown,
    /// Start of the next word.
    WordForward,
    /// Start of the previous word.
    WordBackward,
    /// End of the current word.
    WordEnd,
    /// `W` — start of the next blank-separated word.
    BigWordForward,
    /// `B` — start of the previous blank-separated word.
    BigWordBackward,
    /// `E` — end of the current blank-separated word.
    BigWordEnd,
    /// `f` — forward to the next occurrence of a character, landing on it.
    FindCharForward,
    /// `F` — back to the previous occurrence, landing on it.
    FindCharBackward,
    /// `t` — forward to just *before* the next occurrence.
    TillCharForward,
    /// `T` — back to just *after* the previous occurrence.
    TillCharBackward,
    /// `;` — the last find again, in the direction it was made.
    RepeatFind,
    /// `,` — the last find again, the other way.
    RepeatFindReverse,
    /// First column.
    LineStart,
    /// First non-blank column.
    FirstNonBlank,
    /// Last column.
    LineEnd,
    /// First line of the buffer.
    BufferStart,
    /// Last line of the buffer.
    BufferEnd,
    /// Next paragraph.
    ParagraphForward,
    /// Previous paragraph.
    ParagraphBackward,
    /// The bracket matching the one under the cursor.
    MatchingBracket,
    /// Top row of the viewport. Moves the cursor, never the viewport.
    ScreenTop,
    /// Middle row of the viewport.
    ScreenMiddle,
    /// Bottom row of the viewport.
    ScreenBottom,
    /// Half a screen down.
    HalfPageDown,
    /// Half a screen up.
    HalfPageUp,
    /// The next search match.
    SearchNext,
    /// The previous search match.
    SearchPrev,
}

wire_choice!(Motion {
    CharLeft => "char-left",
    CharRight => "char-right",
    LineUp => "line-up",
    LineDown => "line-down",
    WordForward => "word-forward",
    WordBackward => "word-backward",
    WordEnd => "word-end",
    BigWordForward => "big-word-forward",
    BigWordBackward => "big-word-backward",
    BigWordEnd => "big-word-end",
    FindCharForward => "find-char-forward",
    FindCharBackward => "find-char-backward",
    TillCharForward => "till-char-forward",
    TillCharBackward => "till-char-backward",
    RepeatFind => "repeat-find",
    RepeatFindReverse => "repeat-find-reverse",
    LineStart => "line-start",
    FirstNonBlank => "first-non-blank",
    LineEnd => "line-end",
    BufferStart => "buffer-start",
    BufferEnd => "buffer-end",
    ParagraphForward => "paragraph-forward",
    ParagraphBackward => "paragraph-backward",
    MatchingBracket => "matching-bracket",
    ScreenTop => "screen-top",
    ScreenMiddle => "screen-middle",
    ScreenBottom => "screen-bottom",
    HalfPageDown => "half-page-down",
    HalfPageUp => "half-page-up",
    SearchNext => "search-next",
    SearchPrev => "search-prev",
});

/// A text object — what `i`/`a` select over.
///
/// The last four are `6d`'s agent nouns, and they are the reason a text object
/// is a *store query* rather than a syntax rule: `u` is "the unseen region",
/// which only the store knows. `T028` parses them; `T049` resolves them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    /// `w`.
    Word,
    /// `W`.
    BigWord,
    /// `s`.
    Sentence,
    /// `p`.
    Paragraph,
    /// `(`, `[`, `{`, `"`, `'` — the delimiter rides in the Action.
    Delimited,
    /// `t` in the HTML sense: a markup tag.
    Tag,
    /// `u` — the unseen region under the cursor (`6d`).
    UnseenRegion,
    /// `h` — the diff hunk under the cursor (`6d`).
    Hunk,
    /// `t` in the agent sense: the thread anchored here. Disambiguated from
    /// [`TextObject::Tag`] by the language, per `6d`.
    Thread,
    /// `b` — the review block this file belongs to (`6d`).
    Block,
}

wire_choice!(TextObject {
    Word => "word",
    BigWord => "big-word",
    Sentence => "sentence",
    Paragraph => "paragraph",
    Delimited => "delimited",
    Tag => "tag",
    UnseenRegion => "unseen-region",
    Hunk => "hunk",
    Thread => "thread",
    Block => "block",
});

/// Characterwise, linewise or blockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// Characterwise.
    Char,
    /// Linewise — `V-LINE`.
    Line,
    /// Blockwise.
    Block,
}

wire_choice!(SelectionKind {
    Char => "char",
    Line => "line",
    Block => "block",
});

/// What a case change does to the letters it covers — `gU`, `gu`, `~`.
///
/// A payload rather than three capabilities: the three differ in one word and a
/// door that had to pick between `upper-case` and `toggle-case` by name would
/// be three schemas for one edit. It is a *buffer* mutation and not a
/// [`Motion`] — `gu` takes a motion as its operand, the way `d` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseChange {
    /// `gU` — every letter upper-cased.
    Upper,
    /// `gu` — every letter lower-cased.
    Lower,
    /// `g~` and `~` — each letter to its other case.
    Toggle,
}

wire_choice!(CaseChange {
    Upper => "upper",
    Lower => "lower",
    Toggle => "toggle",
});

// ---------------------------------------------------------------------------
// Steel-facing payloads
// ---------------------------------------------------------------------------

/// What a key is bound to.
///
/// **No Steel value ever appears in an Action payload.** `6b` binds
/// `(keymap-set! "]r" (lambda () (goto (next-region-by claude))))`, and a
/// closure cannot cross MCP or a shell. So a binding is either a named
/// capability with its arguments, or scheme *source text* the Steel door
/// evaluates. Without this rule three capabilities are Steel-only on day one and
/// invariant 2 has a hole in it before the MCP server exists.
///
/// The cost is real and worth stating: an agent or a script writing a keymap
/// writes scheme source, and the CLI cannot type-check it before evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// A registered capability, by door name, with its arguments.
    Capability {
        /// The door name, e.g. `"mark-seen"`.
        name: String,
        /// Its arguments.
        args: crate::value::Args,
    },
    /// Scheme source, evaluated by the Steel door when the key fires.
    Source {
        /// The source text.
        source: String,
    },
}

wire_union!(Binding {
    Capability => "capability", "a registered capability, by door name" {
        name: String = "the door name",
        args: crate::value::Args = "its arguments",
    },
    Source => "source", "scheme source text, evaluated when the key fires" {
        source: String = "the source text",
    },
});

// ---------------------------------------------------------------------------
// Store-facing payloads
// ---------------------------------------------------------------------------

/// A region as it is declared, before the store gives it an id.
///
/// `author` is *claimed*; the store also records who asked
/// ([`Actor`] on the envelope). Design Language §7: only claude-authored writes
/// create regions, so a declaration claiming [`Actor::You`] is a no-op the store
/// records rather than an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpec {
    /// Workspace-relative path.
    pub path: PathBuf,
    /// The span it covers.
    pub span: Span,
    /// Who is claimed to have written it.
    pub author: Actor,
}

wire_record!(RegionSpec {
    path: PathBuf = "workspace-relative path",
    span: Span = "the span the region covers",
    author: Actor = "who is claimed to have written it",
});

/// One file's contribution to a review block (`T053`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileGroup {
    /// Workspace-relative path.
    pub path: PathBuf,
    /// The spans that changed.
    pub spans: Vec<Span>,
    /// Claude's own annotation for this group — `8b`'s "mechanical" versus "the
    /// meat".
    pub annotation: Option<String>,
}

wire_record!(FileGroup {
    path: PathBuf = "workspace-relative path",
    spans: Vec<Span> = "the spans that changed",
    annotation: Option<String> = "claude's note about this group",
});

/// One option in a queued ask (`4a`'s amber `[1]`–`[n]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskOption {
    /// The digit that answers it, 1-based.
    pub digit: u32,
    /// The label, in the product's voice.
    pub label: String,
}

wire_record!(AskOption {
    digit: u32 = "the digit that answers it, 1-based",
    label: String = "the option's label",
});

/// How urgently an inbox item wants to be read (`5c`).
///
/// Three levels rather than syslog's eight: the palette has exactly three roles
/// to render them in (Design Language §1 — meta, attention-amber, trouble-red),
/// and a severity with no colour is a severity nobody can see. `T067` owns the
/// final naming; this is the vocabulary it starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Something happened. Meta-grey.
    Info,
    /// Worth your eyes. Attention-amber.
    Attention,
    /// Something is wrong. Trouble-red.
    Trouble,
}

wire_choice!(Severity {
    Info => "info",
    Attention => "attention",
    Trouble => "trouble",
});

/// A diagnostic, as the LSP hands it over (`T040`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Where.
    pub span: Span,
    /// How bad.
    pub severity: Severity,
    /// What the server said.
    pub message: String,
    /// Which server or linter said it.
    pub source: Option<String>,
}

wire_record!(Diagnostic {
    span: Span = "where the diagnostic applies",
    severity: Severity = "how bad it is",
    message: String = "what the server said",
    source: Option<String> = "which server or linter said it",
});

/// One edit in a replayable batch (`T029`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// What it replaces.
    pub span: Span,
    /// What replaces it. Empty is a deletion.
    pub text: String,
}

wire_record!(Edit {
    span: Span = "the span being replaced",
    text: String = "the replacement; empty deletes",
});

/// How a diff is drawn (`T063`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    /// One column.
    Unified,
    /// Two columns. Dropped below the width the mockups fix.
    SideBySide,
}

wire_choice!(DiffMode {
    Unified => "unified",
    SideBySide => "side-by-side",
});

/// How a review block's files are grouped (`T065`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    /// By directory — scale is grouping, not scrolling.
    Directory,
    /// One flat list.
    Flat,
}

wire_choice!(Grouping {
    Directory => "directory",
    Flat => "flat",
});

/// A fold's new state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldState {
    /// Collapsed.
    Folded,
    /// Expanded.
    Unfolded,
    /// The other one.
    Toggle,
}

wire_choice!(FoldState {
    Folded => "folded",
    Unfolded => "unfolded",
    Toggle => "toggle",
});

/// How far a permission grant reaches (`7a`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantScope {
    /// This invocation only.
    Once,
    /// Write a rule to `init.scm` — `7a`'s legible always-allow.
    Always,
}

wire_choice!(GrantScope {
    Once => "once",
    Always => "always",
});

/// What a picker row does when accepted (`8a`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptHow {
    /// `↵ open` — open it in the focused pane.
    Open,
    /// Open it in a new split.
    Split,
    /// `C-q all to quickfix` — every row, into the quickfix list.
    ///
    /// **Drawn once (`TUI Mockups.dc.html:180`) and named in no task.** The
    /// vocabulary carries it because the drawing does; whether `T047` builds it
    /// or it is cut is Teej's call, flagged at `T019`.
    Quickfix,
}

wire_choice!(AcceptHow {
    Open => "open",
    Split => "split",
    Quickfix => "quickfix",
});

/// What a session seam is (`7b`, `7e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamKind {
    /// The turn paused at a tool boundary.
    Paused,
    /// The transport dropped.
    Lost,
    /// It came back.
    Resumed,
}

wire_choice!(SeamKind {
    Paused => "paused",
    Lost => "lost",
    Resumed => "resumed",
});

/// How a disk conflict is resolved (`5b`). **No auto-merge**, ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskExit {
    /// Take what is on disk, losing the unsaved buffer.
    TakeDisk,
    /// Keep the buffer, leaving disk alone.
    KeepMine,
    /// Hand both to claude and ask.
    AskClaude,
}

wire_choice!(DiskExit {
    TakeDisk => "take-disk",
    KeepMine => "keep-mine",
    AskClaude => "ask-claude",
});

/// What a pane holds (`T088`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    /// A buffer.
    Buffer,
    /// The transcript — a pane, not a float (Design Language §9).
    Transcript,
    /// A pane whose contents claude emitted as a view tree. v1.5; named now so
    /// it is not new machinery later (Q12).
    Custom,
}

wire_choice!(PaneKind {
    Buffer => "buffer",
    Transcript => "transcript",
    Custom => "custom",
});

/// Which sequence a navigation Action walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sequence {
    /// Unseen regions — `]u` / `[u`.
    UnseenRegion,
    /// Diff hunks.
    Hunk,
    /// Files inside a review block — `]]` (`4b`).
    BlockFile,
    /// Diagnostics.
    Diagnostic,
    /// Threads.
    Thread,
    /// Pending asks — `]!` (Q9).
    Ask,
    /// Search matches.
    SearchMatch,
    /// Jumplist entries.
    Jump,
}

wire_choice!(Sequence {
    UnseenRegion => "unseen-region",
    Hunk => "hunk",
    BlockFile => "block-file",
    Diagnostic => "diagnostic",
    Thread => "thread",
    Ask => "ask",
    SearchMatch => "search-match",
    Jump => "jump",
});

/// Which prompt the `:` line is (`T058`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// The ex line — `:w`, `:transcript`, `:diff-disk`.
    Ex,
    /// A message to claude — `:claude`, drawn `:c` in the mockups.
    Claude,
    /// Search.
    Search,
}

wire_choice!(PromptKind {
    Ex => "ex",
    Claude => "claude",
    Search => "search",
});

/// One file's worth of edits, applied together (`T036`'s code actions, rename
/// and format all lower to this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdits {
    /// Workspace-relative path.
    pub path: PathBuf,
    /// The edits, in the order the server gave them.
    pub edits: Vec<Edit>,
}

wire_record!(FileEdits {
    path: PathBuf = "workspace-relative path",
    edits: Vec<Edit> = "the edits, in order",
});

/// What `define-language` declares (`T037`).
///
/// A record rather than free-form arguments: this is the userspace road *up*
/// from a second-tier language to a first-class one, and a schema an agent can
/// read is the difference between "add a language" being a documented capability
/// and being folklore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSpec {
    /// File extensions, without the dot.
    pub extensions: Vec<String>,
    /// The tree-sitter grammar's name or path. Absent means second tier — no
    /// node anchoring, no watches, and the editor says so honestly.
    pub grammar: Option<String>,
    /// The language server command and its arguments. Empty means none.
    pub lsp_command: Vec<String>,
    /// The line-comment prefix, for `toggle-comment`.
    pub comment_prefix: Option<String>,
}

wire_record!(LanguageSpec {
    extensions: Vec<String> = "file extensions, without the dot",
    grammar: Option<String> = "tree-sitter grammar name or path; absent means second tier",
    lsp_command: Vec<String> = "language server command and arguments; empty means none",
    comment_prefix: Option<String> = "line-comment prefix, for toggle-comment",
});

/// Narrows a query or a navigation Action to a subset of regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionFilter {
    /// Only regions by this author. `6b`'s `(next-region-by claude)`.
    pub author: Option<Actor>,
    /// Only unseen ones.
    pub unseen_only: bool,
    /// Only inside this target.
    pub within: Option<Target>,
}

wire_record!(RegionFilter {
    author: Option<Actor> = "only regions by this author",
    unseen_only: bool = "only unseen regions",
    within: Option<Target> = "only regions inside this target",
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Value, Wire};

    #[test]
    fn focus_relative_targets_are_the_four_we_refuse() {
        assert!(Target::Cursor {}.focus_relative());
        assert!(Target::Selection {}.focus_relative());
        assert!(Target::PickerRow {}.focus_relative());
        assert!(Target::FloatRow {}.focus_relative());
        assert!(!Target::Region { id: RegionId(1) }.focus_relative());
        assert!(
            !Target::Explicit {
                path: PathBuf::from("src/retry.rs"),
                span: Span {
                    start: Position { line: 6, column: 1 },
                    end: Position {
                        line: 10,
                        column: 1
                    },
                },
            }
            .focus_relative()
        );
    }

    #[test]
    fn a_target_round_trips_through_the_wire() {
        let target = Target::Explicit {
            path: PathBuf::from("src/retry.rs"),
            span: Span {
                start: Position { line: 6, column: 1 },
                end: Position {
                    line: 10,
                    column: 1,
                },
            },
        };
        let encoded = target.to_value();
        assert_eq!(encoded.tag(), Some("explicit"));
        assert_eq!(Target::from_value(&encoded).unwrap(), target);
    }

    #[test]
    fn an_unknown_tag_names_what_it_accepts() {
        let bogus = Value::tagged("everything", crate::value::Args::new());
        let error = Target::from_value(&bogus).unwrap_err();
        let crate::value::WireError::Tag { got, expected } = error else {
            panic!("expected a tag error");
        };
        assert_eq!(got, "everything");
        assert!(expected.contains(&"region"));
    }
}
