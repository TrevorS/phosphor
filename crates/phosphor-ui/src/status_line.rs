//! The statusline (`T017`) — Design Language §5, shed order §11 as amended by
//! [Q9].
//!
//! > "Three strips of chrome, ever: tab bar (top, appears only with 2+ panes),
//! > **statusline (bottom, always)**, and tmux below it, untouched. Statusline
//! > left→right: mode chip (bg = mode color, the only inverted text on screen),
//! > file + dirty flag, spring, session state, counters. Segments join with a
//! > thin bar │ in meta-gray. **Statusline content is priority-ordered and
//! > truncates from the right — it never wraps; a second line is a bug.**"
//!
//! # A second row is unrepresentable, not merely avoided
//!
//! `T017`'s acceptance is that a property test at widths 40–200 never produces
//! two rows, and the task is explicit that this must be structural. It is:
//! every cell this module writes goes through [`Row`], a private cursor that
//! holds **one** `y` and a half-open `[x, end)` column bound, both fixed at
//! construction. There is no method on it that takes a `y`, and the only
//! constructor clamps the caller's [`Rect`] to `height == 1` and intersects it
//! with the buffer. Composition below decides *what* to drop; it cannot decide
//! to spill, because it never touches the [`Buffer`] itself.
//!
//! That is why the shed ladder can be read as pure layout policy — data, not a
//! safety mechanism. When it runs out of steps (a 400-column path at 40
//! columns) the row simply stops writing at its right edge. Ugly, bounded, one
//! row.
//!
//! # The shed order
//!
//! §11: *"counters → jj → cursor pos → session prose (glyph stays) → mode word
//! (initial stays). The `✻`/`●n` pair is the last thing standing."* [Q9] adds
//! `!` to that set, because a queued ask's only notification is the flag.
//!
//! Shedding is **fit-driven**: a step is applied only when the line does not
//! fit, which is §11's "narrow terminals drop, never squeeze" read literally —
//! nothing is dropped while there is room for it. Mockup `8d` draws the *end*
//! of the ladder (`N`, `retry.rs [+]`, `✻`, `●6`) under an 80-column heading;
//! at a real 80 columns this widget keeps more, because more fits.
//!
//! **`CP-1` settled it in favour of this widget:** shedding stays fit-driven
//! and `8d` is relabelled as illustrating the end of the ladder rather than an
//! 80-column threshold. The *order* was never in question — it is §11's,
//! exactly — only the trigger, and a width-labelled trigger would drop content
//! that fits. Recorded in the plan's amendment table; the mockup itself is
//! edited in the Design project, not here.
//!
//! # What is Steel's — and it is Steel's now
//!
//! The Component Breakdown puts the segment list, order and shed priority in
//! Steel — *"redefine a segment in the REPL and the next frame has it."* `T025`
//! did that: the composed statusline is `runtime/statusline.scm`, it returns a
//! view tree, and `crate::interpret` draws it. **That is the statusline the
//! product has**; this widget is the S1 host's own path until `T026` composes
//! the buffer surface's frame too, and it is what `T018`'s golden frames are
//! captured through.
//!
//! The ladder here is therefore still **data** ([`SHED_ORDER`], overridable via
//! [`StatusLine::with_shed_order`]) rather than a policy branch — the same
//! shape the editor layer's `phosphor/status-ladder` has, one side of the seam
//! each.
//!
//! An earlier version of this paragraph named `T027` as the Steel task. `T027`
//! is the kitty keyboard protocol (`docs/TASKS.md`); the statusline task is
//! `T025`.
//!
//! [Q9]: ../../../docs/IMPLEMENTATION-PLAN.md
//! [`Rect`]: ratatui_core::layout::Rect
//! [`Buffer`]: ratatui_core::buffer::Buffer

use core::time::Duration;
use std::borrow::Cow;

use phosphor_core::vm::ViewModel;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::text::Span;

use crate::theme::Theme;

/// One space between the chip and the file, and one before the right edge.
///
/// The mockups set these in pixels (`padding: 0 10px` at a ~7px cell), which is
/// one to two cells; the grid is the only layout unit (§0), so it is one.
const GAP: &str = " ";

/// §5: *"Segments join with a thin bar │ in meta-gray."*
///
/// Applied **within the counter group only** — `●6 │ jj ✓ │ 12:1`, exactly as
/// `9c` and `8c` draw it. The mode chip needs no bar (its inverted field *is*
/// its boundary), and neither does the seam between session state and the
/// counters: §5's own reference render draws `✻ claude idle` then a plain gap,
/// and `1a`, `9c`, `8c` and `8d` all agree.
///
/// **Teej's `CP-1` ruling**, against §5's prose, which reads as though every
/// segment joins with a bar. The build drew `✻ claude idle │ 6 unseen` and no
/// drawing anywhere does. See [`GAP_AFTER_SESSION`].
const SEP: &str = " │ ";

/// [`SEP`] in cells. `│` is one column wide but three bytes long, and the
/// difference is a right-aligned group that starts two columns too far left.
const SEP_WIDTH: u16 = 3;

/// The session-state seam, in cells: a plain [`GAP`], not a [`SEP`].
///
/// `✻ claude idle` then a space then `6 unseen │ jj ✓ │ 12:1`. Every mockup
/// draws it this way and §5's prose implies otherwise; `CP-1` settled it for
/// the drawings. Two columns narrower than a bar, which the right group's
/// width arithmetic has to know about or it starts two columns too far left.
const GAP_AFTER_SESSION: u16 = 1;

/// The editing mode, as the chip spells it.
///
/// The four the design documents draw. `T026` (the input machine, `spine`'s)
/// owns the canonical mode set; when it lands, this maps onto it rather than
/// competing with it — the chip renders a `Mode`, and the ViewModel carries
/// whichever one the machine is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// `NORMAL` — chip field is claude-green (`1a`, `9c`, `8c`, `8e`).
    Normal,
    /// `INSERT` — chip field is you-blue (`7d`).
    Insert,
    /// `VISUAL` — chip field is the transient hue (§1: "transient — visual
    /// mode, spinners, types").
    Visual,
    /// `PAUSED` — chip field is attention-amber (`7e`).
    Paused,
}

impl Mode {
    /// The full word, as drawn at width.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::Paused => "PAUSED",
        }
    }

    /// The initial, which is what survives §11's last shed step: *"mode word
    /// (initial stays)"*. Mockup `8d` draws `N`.
    #[must_use]
    pub const fn initial(self) -> &'static str {
        match self {
            Self::Normal => "N",
            Self::Insert => "I",
            Self::Visual => "V",
            Self::Paused => "P",
        }
    }

    /// The chip's background — an actor colour, never a colour of its own.
    ///
    /// The mapping is recorded on [`crate::theme::Chrome`]: normal → claude,
    /// insert → you, visual → transient, paused → attention.
    #[must_use]
    pub const fn field(self, theme: &Theme) -> Color {
        match self {
            Self::Normal => theme.actors.claude,
            Self::Insert => theme.actors.you,
            Self::Visual => theme.actors.transient,
            Self::Paused => theme.actors.attention,
        }
    }
}

/// The braille spinner, §2: *"Spinner frames: ⠋⠙⠸⠴⠦⠇"*, §8: 80ms/frame.
///
/// The widget is a pure renderer, so the frame index arrives in the ViewModel
/// rather than being counted here — the same reason elapsed time does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Spinner(pub u8);

impl Spinner {
    /// §2's six frames, in order.
    pub const FRAMES: [&'static str; 6] = ["⠋", "⠙", "⠸", "⠴", "⠦", "⠇"];

    /// The frame for this tick. Wraps, so a free-running 80ms counter can be
    /// handed over without bookkeeping.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        Self::FRAMES[(self.0 as usize) % Self::FRAMES.len()]
    }
}

/// The session, as **one enum rendered one way everywhere** (Component
/// Breakdown, `StatusLine`).
///
/// §5: *"Session state is always present and truthful — idle, working+elapsed,
/// waiting, paused, lost."* The glyph/prose/colour split below is what §11's
/// *"session prose (glyph stays)"* shed step needs, and what lets the
/// transcript's seam markers and the dashboard reuse the same vocabulary
/// without redrawing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// No session at all — S1's value, since the ACP client is `T050`. Renders
    /// as nothing: §5 wants the state truthful, and the truth is that there is
    /// no session yet.
    None,
    /// `✻ claude idle` in claude-green (`1a`, `9c`, `8c`).
    Idle,
    /// `⠸ claude working · 0:42` in the transient hue (`3d`, `2c`).
    Working {
        /// Elapsed since the turn started; `None` before the first tick.
        elapsed: Option<Duration>,
        /// Which of §2's six frames to draw.
        spinner: Spinner,
    },
    /// `! claude waiting` in attention-amber (`7a`, `4b`).
    Waiting,
    /// `⏸ claude paused` in attention-amber (`7e`).
    Paused,
    /// `✕ session lost — :reattach` in trouble-red (§5).
    ///
    /// §6 is the authority on the wording over mockup `7b`'s `:ca`, which that
    /// same section names as the counter-example: *"Keyhints spell the whole
    /// command … never cryptic contractions like `:ca` or `:rr`."*
    Lost,
}

impl SessionState {
    /// The single cell that survives every shed step (§11, [`ShedStep::SessionProse`]).
    #[must_use]
    pub const fn glyph(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Idle => Some("✻"),
            Self::Working { spinner, .. } => Some(spinner.glyph()),
            Self::Waiting => Some("!"),
            Self::Paused => Some("⏸"),
            Self::Lost => Some("✕"),
        }
    }

    /// The words after the glyph, telegraphic and lowercase (§6).
    ///
    /// Public because `T079`'s tree interpreter renders a
    /// `phosphor_core::view::Node::Session` through this same enum rather than
    /// spelling the wording a second time — §5's *"one enum rendered
    /// identically everywhere it appears"* only holds if there is one place the
    /// words live.
    #[must_use]
    pub fn prose(self) -> Option<Cow<'static, str>> {
        match self {
            Self::None => None,
            Self::Idle => Some(Cow::Borrowed("claude idle")),
            Self::Working { elapsed, .. } => Some(match elapsed {
                Some(d) => Cow::Owned(format!("claude working · {}", format_elapsed(d))),
                None => Cow::Borrowed("claude working"),
            }),
            Self::Waiting => Some(Cow::Borrowed("claude waiting")),
            Self::Paused => Some(Cow::Borrowed("claude paused")),
            Self::Lost => Some(Cow::Borrowed("session lost — :reattach")),
        }
    }

    /// The actor colour this state speaks in (§1).
    #[must_use]
    pub const fn colour(self, theme: &Theme) -> Color {
        match self {
            // Never drawn; the caller skips `None` before asking.
            Self::None | Self::Idle => theme.actors.claude,
            Self::Working { .. } => theme.actors.transient,
            Self::Waiting | Self::Paused => theme.actors.attention,
            Self::Lost => theme.actors.trouble,
        }
    }
}

/// `0:42`, and `1:02:03` once a turn passes the hour (mockup `2c`).
///
/// Public for the same reason [`SessionState::prose`] is: `T079` renders
/// `Node::Elapsed` from the frame clock, and a second implementation of this
/// would be a second thing to get wrong at the hour boundary.
#[must_use]
pub fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The file segment: path plus §5's dirty flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVm<'a> {
    /// Repo-relative path, as the mockups draw it (`src/retry.rs`).
    pub path: &'a str,
    /// `[+]`, in attention-amber (§1: "attention — waiting, paused, **dirty**").
    pub dirty: bool,
}

/// `12:1` — the cursor counter (`1a`, `8e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorVm {
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
}

/// Everything the statusline draws, derived from the store.
///
/// A ViewModel in the [`phosphor_core::vm`] sense: read-only, with no path back
/// to a mutation. It is declared here rather than in `phosphor-core` because
/// concrete ViewModels *"land with the surfaces that need them"* (`vm.rs`) and
/// `vm.rs` has no owner in TEAM.md's per-file split — moving it there is a
/// `spine` call, not mine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusLineVm<'a> {
    /// The mode chip — the only inverted text on screen (§5).
    pub mode: Mode,
    /// The current file, or `None` on a surface that has no buffer (mockup
    /// `2d`, the dashboard, draws chip → spring → session → counters).
    pub file: Option<FileVm<'a>>,
    /// Always present and truthful (§5); [`SessionState::None`] at S1.
    pub session: SessionState,
    /// [Q9]'s queued-ask flag: *"It sets the statusline `!` flag immediately and
    /// waits."* Suppressed when the session is already
    /// [`SessionState::Waiting`], whose glyph is that same `!` — drawing it
    /// twice would be a bug, not a stronger signal.
    ///
    /// [Q9]: ../../../docs/IMPLEMENTATION-PLAN.md
    pub ask_pending: bool,
    /// Unseen regions in this file — `6 unseen` at width, `●6` once the
    /// counters shed their words. Zero renders nothing.
    pub unseen: u32,
    /// The VCS chip, e.g. `jj ✓` (`phosphor-vcs`, `T071`). `None` outside a
    /// repo.
    pub vcs: Option<&'a str>,
    /// Cursor position, or `None` where there is no cursor.
    pub cursor: Option<CursorVm>,
}

impl ViewModel for StatusLineVm<'_> {}

impl Default for StatusLineVm<'_> {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            file: None,
            session: SessionState::None,
            ask_pending: false,
            unseen: 0,
            vcs: None,
            cursor: None,
        }
    }
}

/// One rung of §11's ladder.
///
/// Ordered by [`SHED_ORDER`], applied one at a time until the line fits. Each
/// step is a *drop* or a *contraction to a glyph* — never a squeeze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShedStep {
    /// §11's "counters": the counts lose their words but keep their glyphs —
    /// `6 unseen` → `●6`, as drawn wide in `1a`/`8c` and narrow in `9c`/`8d`.
    CounterWords,
    /// `jj ✓` drops.
    Vcs,
    /// `12:1` drops.
    CursorPos,
    /// *"session prose (glyph stays)"* — `✻ claude idle` → `✻`.
    SessionProse,
    /// *"mode word (initial stays)"* — `NORMAL` → `N` (`8d`).
    ModeWord,
    /// The path contracts to its basename — `src/retry.rs` → `retry.rs` (`8d`).
    /// Below §11's ladder, which stops at the mode word.
    FilePath,
    /// The dirty flag `[+]` drops. After the mode word, because `8d` draws `N`
    /// and `[+]` together.
    DirtyFlag,
    /// The file drops entirely. The floor: what remains is the chip and the
    /// last-standing set.
    File,
}

/// §11's ladder, in order, with `8d`'s file steps below it.
///
/// The last-standing set — `✻` / `●n` / `!` ([Q9]) — appears nowhere in it, and
/// that is the point: no step can remove those three.
///
/// [Q9]: ../../../docs/IMPLEMENTATION-PLAN.md
pub const SHED_ORDER: &[ShedStep] = &[
    ShedStep::CounterWords,
    ShedStep::Vcs,
    ShedStep::CursorPos,
    ShedStep::SessionProse,
    ShedStep::ModeWord,
    ShedStep::FilePath,
    ShedStep::DirtyFlag,
    ShedStep::File,
];

/// Which rungs have been applied so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Shed {
    counter_words: bool,
    vcs: bool,
    cursor: bool,
    session_prose: bool,
    mode_word: bool,
    file_path: bool,
    dirty: bool,
    file: bool,
}

impl Shed {
    fn apply(&mut self, step: ShedStep) {
        match step {
            ShedStep::CounterWords => self.counter_words = true,
            ShedStep::Vcs => self.vcs = true,
            ShedStep::CursorPos => self.cursor = true,
            ShedStep::SessionProse => self.session_prose = true,
            ShedStep::ModeWord => self.mode_word = true,
            ShedStep::FilePath => self.file_path = true,
            ShedStep::DirtyFlag => self.dirty = true,
            ShedStep::File => self.file = true,
        }
    }
}

/// A run of text with one style. The unit the row writer consumes.
#[derive(Debug, Clone)]
struct Piece<'a> {
    text: Cow<'a, str>,
    style: Style,
    /// Join this piece to the one before it with a plain [`GAP`] instead of
    /// [`SEP`]. Set on the first counter, so the session-state seam reads the
    /// way every mockup draws it. See [`GAP_AFTER_SESSION`].
    gap_before: bool,
}

impl<'a> Piece<'a> {
    fn new(text: impl Into<Cow<'a, str>>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            gap_before: false,
        }
    }

    /// Display width in cells, grapheme- and East-Asian-aware (the same
    /// measurement `Buffer::set_stringn` writes with).
    fn width(&self) -> u16 {
        u16::try_from(Span::raw(self.text.as_ref()).width()).unwrap_or(u16::MAX)
    }
}

fn sum_width(pieces: &[Piece<'_>]) -> u16 {
    pieces
        .iter()
        .fold(0u16, |acc, p| acc.saturating_add(p.width()))
}

/// The statusline widget (`T017`).
///
/// Takes `&Theme` (§12) and a ViewModel, and owns no state — the shed decision
/// is recomputed from the width every frame, so nothing can go stale.
#[derive(Debug, Clone, Copy)]
pub struct StatusLine<'a> {
    vm: &'a StatusLineVm<'a>,
    theme: &'a Theme,
    shed_order: &'a [ShedStep],
}

impl<'a> StatusLine<'a> {
    /// Compose against [`SHED_ORDER`].
    #[must_use]
    pub const fn new(vm: &'a StatusLineVm<'a>, theme: &'a Theme) -> Self {
        Self {
            vm,
            theme,
            shed_order: SHED_ORDER,
        }
    }

    /// Override the ladder — this widget's half of `T025`'s seam, for a host
    /// that has a shed order and is not composing through the view tree.
    #[must_use]
    pub const fn with_shed_order(mut self, order: &'a [ShedStep]) -> Self {
        self.shed_order = order;
        self
    }

    /// Draw into the **first row** of `area`.
    ///
    /// Extra height is ignored, not used: see the module header. A caller that
    /// hands over three rows gets one statusline and two untouched rows.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let Some(mut row) = Row::new(buf, area, self.theme.chrome.statusline) else {
            return;
        };

        // Fit the ladder, then draw. `budget` is what the two groups may share:
        // the row minus one column of right margin.
        let budget = row.width().saturating_sub(1);
        let mut shed = Shed::default();
        let (mut left, mut right) = self.compose(shed);
        for step in self.shed_order {
            if group_width(&left, &right) <= budget {
                break;
            }
            shed.apply(*step);
            let composed = self.compose(shed);
            left = composed.0;
            right = composed.1;
        }

        for piece in &left {
            row.write(&piece.text, piece.style);
        }

        // Right-aligned, but never on top of the left group: the spring is
        // whatever is left over, and it collapses to nothing before the right
        // group starts losing cells off its own right edge.
        let right_width = joined_width(&right);
        let right_start = row
            .end()
            .saturating_sub(1)
            .saturating_sub(right_width)
            .max(row.cursor());
        row.skip_to(right_start);
        let sep_style = Style::new().fg(self.theme.neutrals.meta);
        for (i, piece) in right.iter().enumerate() {
            if i > 0 {
                row.write(if piece.gap_before { GAP } else { SEP }, sep_style);
            }
            row.write(&piece.text, piece.style);
        }
    }

    /// Build the two groups at the given shed level. Left → right within each.
    fn compose(&self, shed: Shed) -> (Vec<Piece<'a>>, Vec<Piece<'a>>) {
        let (vm, theme) = (self.vm, self.theme);
        let text = Style::new().fg(theme.neutrals.text);

        // ── left ─────────────────────────────────────────────────────────────
        //
        // The chip is the ONLY piece in this file that sets a background. §5:
        // "the only inverted text on screen." Keeping the single `.bg(...)`
        // call here is what makes that checkable by grep as well as by test.
        let word = if shed.mode_word {
            vm.mode.initial()
        } else {
            vm.mode.word()
        };
        let chip = Style::new()
            .fg(theme.chrome.mode_chip_fg)
            .bg(vm.mode.field(theme));
        let mut left = vec![Piece::new(format!(" {word} "), chip)];

        if let Some(file) = vm.file
            && !shed.file
        {
            let path = if shed.file_path {
                basename(file.path)
            } else {
                file.path
            };
            left.push(Piece::new(GAP, text));
            left.push(Piece::new(path, text));
            if file.dirty && !shed.dirty {
                left.push(Piece::new(GAP, text));
                left.push(Piece::new("[+]", Style::new().fg(theme.actors.attention)));
            }
        }

        // ── right ────────────────────────────────────────────────────────────
        let mut right = Vec::new();

        if let Some(glyph) = vm.session.glyph() {
            let style = Style::new().fg(vm.session.colour(theme));
            let body = match (shed.session_prose, vm.session.prose()) {
                (false, Some(prose)) => Cow::Owned(format!("{glyph} {prose}")),
                _ => Cow::Borrowed(glyph),
            };
            right.push(Piece::new(body, style));
        }

        // Q9's flag. Never doubled up with Waiting's own `!`.
        if vm.ask_pending && vm.session != SessionState::Waiting {
            right.push(Piece::new("!", Style::new().fg(theme.actors.attention)));
        }

        // The counters are their own group: bars inside it, a plain gap where it
        // meets session state. `right` so far holds only session pieces, so the
        // first counter is the seam — whichever counter that turns out to be
        // once shedding has had its say.
        let session_pieces = right.len();

        if vm.unseen > 0 {
            let n = vm.unseen;
            let counter = if shed.counter_words {
                format!("●{n}")
            } else {
                format!("{n} unseen")
            };
            right.push(Piece::new(counter, Style::new().fg(theme.neutrals.meta)));
        }

        if let Some(vcs) = vm.vcs
            && !shed.vcs
        {
            right.push(Piece::new(vcs, Style::new().fg(theme.neutrals.meta)));
        }

        if let Some(cursor) = vm.cursor
            && !shed.cursor
        {
            right.push(Piece::new(
                format!("{}:{}", cursor.line, cursor.col),
                Style::new().fg(theme.neutrals.meta),
            ));
        }

        // Only when session state is actually present: with nothing before it,
        // the first counter has no seam to soften.
        if session_pieces > 0
            && let Some(first_counter) = right.get_mut(session_pieces)
        {
            first_counter.gap_before = true;
        }

        (left, right)
    }
}

/// `src/retry.rs` → `retry.rs` (`8d`).
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Width of the right group including the joins between its pieces — a `│` for
/// most, a plain gap at the session seam ([`GAP_AFTER_SESSION`]).
fn joined_width(right: &[Piece<'_>]) -> u16 {
    right
        .iter()
        .enumerate()
        .fold(sum_width(right), |acc, (i, piece)| {
            if i == 0 {
                acc
            } else if piece.gap_before {
                acc.saturating_add(GAP_AFTER_SESSION)
            } else {
                acc.saturating_add(SEP_WIDTH)
            }
        })
}

/// What the two groups need, with one column of spring between them.
fn group_width(left: &[Piece<'_>], right: &[Piece<'_>]) -> u16 {
    let spring = u16::from(!right.is_empty());
    sum_width(left)
        .saturating_add(spring)
        .saturating_add(joined_width(right))
}

/// **The reason a second row cannot happen.**
///
/// One `y`, fixed at construction. One `[x, end)` column window, fixed at
/// construction. `write` advances the cursor and clamps to `end`; there is no
/// way to address another line and no way to widen the window. Composition
/// hands this text; it never sees the [`Buffer`].
#[derive(Debug)]
struct Row<'b> {
    buf: &'b mut Buffer,
    y: u16,
    x: u16,
    end: u16,
}

impl<'b> Row<'b> {
    /// Clamp `area` to its first row, intersect it with the buffer, and paint
    /// the statusline field across it.
    ///
    /// Returns `None` when nothing of it is on screen — a zero-width or
    /// zero-height area, or one that starts past the buffer.
    fn new(buf: &'b mut Buffer, area: Rect, field: Color) -> Option<Self> {
        let one_row = Rect {
            height: area.height.min(1),
            ..area
        };
        let area = one_row.intersection(buf.area);
        if area.is_empty() {
            return None;
        }
        let (y, x, end) = (area.y, area.x, area.right());
        buf.set_string(x, y, " ".repeat((end - x) as usize), Style::new().bg(field));
        Some(Self { buf, y, x, end })
    }

    fn width(&self) -> u16 {
        self.end - self.x
    }

    fn cursor(&self) -> u16 {
        self.x
    }

    fn end(&self) -> u16 {
        self.end
    }

    /// Move the cursor forward. Never backward — a piece already written can
    /// never be overwritten by a later one.
    fn skip_to(&mut self, x: u16) {
        self.x = x.clamp(self.x, self.end);
    }

    /// Write at the cursor, clipped to the row's right edge.
    fn write(&mut self, text: &str, style: Style) {
        let remaining = self.end.saturating_sub(self.x);
        if remaining == 0 {
            return;
        }
        let (x, _) = self
            .buf
            .set_stringn(self.x, self.y, text, remaining as usize, style);
        self.x = x.min(self.end);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CursorVm, FileVm, Mode, SessionState, ShedStep, Spinner, StatusLine, StatusLineVm,
    };
    use crate::theme::Theme;
    use core::time::Duration;
    use proptest::prelude::*;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;

    /// A buffer wider and taller than the area we hand the widget, so an
    /// overflow in either axis is visible as a touched cell.
    const CANVAS: Rect = Rect {
        x: 0,
        y: 0,
        width: 220,
        height: 5,
    };

    /// The area is a single row at y == 2, inset by 2 columns, unless a test
    /// deliberately hands over more height.
    fn area(width: u16, height: u16) -> Rect {
        Rect {
            x: 2,
            y: 2,
            width,
            height,
        }
    }

    fn render(vm: &StatusLineVm<'_>, width: u16, height: u16) -> Buffer {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(CANVAS);
        StatusLine::new(vm, &theme).render(area(width, height), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16, x: u16, width: u16) -> String {
        (x..x + width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
    }

    /// Every cell that is not inside `{y == 2, 2 <= x < 2 + width}`.
    fn cells_outside(buf: &Buffer, width: u16) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for y in CANVAS.y..CANVAS.bottom() {
            for x in CANVAS.x..CANVAS.right() {
                let inside = y == 2 && (2..2 + width).contains(&x);
                let cell = &buf[(x, y)];
                let untouched = cell.symbol() == " "
                    && cell.fg == ratatui_core::style::Color::Reset
                    && cell.bg == ratatui_core::style::Color::Reset;
                if !inside && !untouched {
                    out.push((x, y));
                }
            }
        }
        out
    }

    fn full_vm() -> StatusLineVm<'static> {
        StatusLineVm {
            mode: Mode::Normal,
            file: Some(FileVm {
                path: "src/retry.rs",
                dirty: true,
            }),
            session: SessionState::Idle,
            ask_pending: false,
            unseen: 6,
            vcs: Some("jj ✓"),
            cursor: Some(CursorVm { line: 12, col: 1 }),
        }
    }

    #[test]
    fn screen_9c_reproduces_at_width() {
        // 9c: NORMAL chip · src/retry.rs [+] · spring · ✻ claude idle · ●6 · jj ✓
        //
        // Note the seam: a plain gap after `claude idle`, bars only inside the
        // counter group. That is `9c`'s own markup — session state is one span
        // with padding, `●6 │ jj ✓` is the next — and `1a`, `8c` and `8d` agree.
        // Teej's CP-1 ruling, against §5's prose. See `GAP_AFTER_SESSION`.
        let buf = render(&full_vm(), 120, 1);
        let line = row_text(&buf, 2, 2, 120);
        assert!(line.starts_with(" NORMAL  src/retry.rs [+]"), "{line:?}");
        assert!(
            line.ends_with("✻ claude idle 6 unseen │ jj ✓ │ 12:1 "),
            "{line:?}"
        );
        assert!(
            !line.contains("claude idle │"),
            "no mockup draws a bar at the session seam: {line:?}"
        );
    }

    #[test]
    fn the_mode_chip_is_the_only_inverted_text() {
        // §5: "mode chip (bg = mode color, the only inverted text on screen)".
        let theme = Theme::phosphor_dark();
        let vm = StatusLineVm {
            session: SessionState::Working {
                elapsed: Some(Duration::from_secs(42)),
                spinner: Spinner(2),
            },
            ask_pending: true,
            ..full_vm()
        };
        let buf = render(&vm, 120, 1);
        let inverted: Vec<u16> = (2..122)
            .filter(|x| buf[(*x, 2)].bg != theme.chrome.statusline)
            .collect();
        assert_eq!(
            inverted,
            (2..10).collect::<Vec<_>>(),
            "exactly one contiguous run of inverted cells, and it is the chip"
        );
        assert!(
            buf[(2, 2)].bg == theme.actors.claude,
            "the chip's field is the mode's actor colour"
        );
    }

    #[test]
    fn extra_height_is_ignored_not_used() {
        // The widget is handed three rows and must use one.
        let buf = render(&full_vm(), 120, 3);
        assert_eq!(cells_outside(&buf, 120), Vec::new());
    }

    #[test]
    fn the_ladder_sheds_in_section_11_order() {
        // Stated as thresholds rather than hand-picked widths: for each thing
        // the ladder can drop, the narrowest width that still shows it. Because
        // shedding is fit-driven and monotonic in the width, those thresholds
        // must fall in exactly the documented order — which checks every rung at
        // once, and keeps checking it when a segment's text changes length.
        let vm = full_vm();
        let at = |w: u16| row_text(&render(&vm, w, 1), 2, 2, w);
        let threshold = |needle: &str| -> u16 {
            (4u16..=200)
                .find(|w| at(*w).contains(needle))
                .unwrap_or_else(|| panic!("{needle:?} is never drawn at any width"))
        };

        let wide = at(120);
        assert!(
            wide.contains("NORMAL") && wide.contains("src/retry.rs [+]"),
            "{wide:?}"
        );
        assert!(wide.contains("6 unseen") && wide.contains("jj ✓") && wide.contains("12:1"));

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

        // And what the words contract to is there once the words are not.
        let narrow = at(ladder[3].1 - 1);
        assert!(
            narrow.contains('✻') && !narrow.contains("claude idle"),
            "{narrow:?}"
        );
        assert!(narrow.contains("●6"), "{narrow:?}");
    }

    #[test]
    fn the_last_standing_set_survives_every_step() {
        // §11 + Q9: ✻ / ●n / ! are the last three standing.
        let vm = StatusLineVm {
            mode: Mode::Normal,
            file: Some(FileVm {
                path: "a/very/deeply/nested/path/that/will/never/fit.rs",
                dirty: true,
            }),
            session: SessionState::Idle,
            ask_pending: true,
            unseen: 6,
            vcs: Some("jj ✓"),
            cursor: Some(CursorVm { line: 120, col: 44 }),
        };
        let line = row_text(&render(&vm, 40, 1), 2, 2, 40);
        assert!(line.contains('✻'), "{line:?}");
        assert!(line.contains("●6"), "{line:?}");
        assert!(line.contains('!'), "{line:?}");
    }

    #[test]
    fn a_queued_ask_never_doubles_the_waiting_glyph() {
        let vm = StatusLineVm {
            session: SessionState::Waiting,
            ask_pending: true,
            unseen: 0,
            vcs: None,
            cursor: None,
            ..full_vm()
        };
        let line = row_text(&render(&vm, 60, 1), 2, 2, 60);
        assert_eq!(line.matches('!').count(), 1, "{line:?}");
    }

    #[test]
    fn a_custom_shed_order_is_honoured() {
        // The Steel seam: the ladder is data.
        let vm = full_vm();
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(CANVAS);
        StatusLine::new(&vm, &theme)
            .with_shed_order(&[ShedStep::File])
            .render(area(40, 1), &mut buf);
        let line = row_text(&buf, 2, 2, 40);
        assert!(!line.contains("retry.rs"), "{line:?}");
        assert!(line.contains("6 unseen"), "counters kept: {line:?}");
    }

    // ── the property ─────────────────────────────────────────────────────────

    prop_compose! {
        fn any_session()(
            kind in 0u8..6,
            secs in 0u64..500_000,
            frame in any::<u8>(),
            has_elapsed in any::<bool>(),
        ) -> SessionState {
            match kind {
                0 => SessionState::None,
                1 => SessionState::Idle,
                2 => SessionState::Working {
                    elapsed: has_elapsed.then(|| Duration::from_secs(secs)),
                    spinner: Spinner(frame),
                },
                3 => SessionState::Waiting,
                4 => SessionState::Paused,
                _ => SessionState::Lost,
            }
        }
    }

    /// **`T060`: Q9's `!` survives the shed at 40 columns.**
    ///
    /// The acceptance in its own words. The flag is *the only notification a
    /// queued ask gets* — a question waits rather than interrupting, so a strip
    /// that dropped the `!` to fit would be an editor that had stopped
    /// mentioning it at all. `SHED_ORDER` has no rung for it, and this is the
    /// test that says so at the width where every rung has been spent.
    ///
    /// **Adversarially wide inputs**, because a `!` that survives 40 columns
    /// with a short path proves nothing about one that has to shed everything
    /// first.
    #[test]
    fn q9s_flag_survives_every_rung_of_the_shed() {
        let vm = StatusLineVm {
            file: Some(FileVm {
                path: "a/very/deeply/nested/path/far/longer/than/any/terminal.rs",
                dirty: true,
            }),
            // **Not `Waiting`.** That state's own glyph *is* `!`, and the flag
            // is suppressed beside it — so drawing this case at `Idle` is what
            // makes the `!` on screen the flag's and nobody else's.
            session: SessionState::Idle,
            ask_pending: true,
            unseen: 999,
            vcs: Some("jj ✓ 3 ahead"),
            cursor: Some(CursorVm {
                line: 12_345,
                col: 678,
            }),
            ..full_vm()
        };
        for width in 40..=60 {
            let line = row_text(&render(&vm, width, 1), 2, 2, width);
            assert!(
                line.contains('!'),
                "the queued-ask flag survives {width} columns; row was {line:?}"
            );
        }

        // And it is not there when nothing is queued — otherwise the loop above
        // would pass against a strip that always drew one.
        let quiet = StatusLineVm {
            ask_pending: false,
            ..vm
        };
        let line = row_text(&render(&quiet, 40, 1), 2, 2, 40);
        assert!(
            !line.contains('!'),
            "and only when one is; row was {line:?}"
        );
    }

    #[test]
    fn every_width_from_40_to_200_is_one_row() {
        // The deterministic half of `T017`'s criterion: the property below
        // samples the width range, this one walks all 161 of them, against the
        // ViewModels most likely to overflow — a path far longer than the
        // terminal, and counters at their widest.
        let long = "a/very/deeply/nested/path/that/is/far/longer/than/any/terminal/is/wide.rs";
        let adversarial = [
            StatusLineVm {
                file: Some(FileVm {
                    path: long,
                    dirty: true,
                }),
                session: SessionState::Lost,
                ask_pending: true,
                unseen: u32::MAX,
                vcs: Some("jj ✓ 3 ahead"),
                cursor: Some(CursorVm {
                    line: u32::MAX,
                    col: u32::MAX,
                }),
                ..full_vm()
            },
            StatusLineVm::default(),
            full_vm(),
        ];
        for vm in &adversarial {
            for width in 40..=200u16 {
                assert_eq!(
                    cells_outside(&render(vm, width, 3), width),
                    Vec::new(),
                    "width {width}"
                );
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1024, ..ProptestConfig::default() })]

        /// `T017`'s acceptance criterion: **widths 40 to 200, never two rows.**
        ///
        /// Stated as "no cell outside the one-row area is touched", which also
        /// catches an overflow past the right edge — the other way a statusline
        /// spills. The area is deliberately given `height` up to 3 so the clamp
        /// is exercised, and is inset from the buffer on all sides so a write
        /// in any direction is visible.
        #[test]
        fn never_two_rows(
            width in 40u16..=200,
            height in 1u16..=3,
            mode in prop_oneof![
                Just(Mode::Normal), Just(Mode::Insert),
                Just(Mode::Visual), Just(Mode::Paused),
            ],
            path in prop::option::of("[\\PC]{0,120}"),
            dirty in any::<bool>(),
            session in any_session(),
            ask_pending in any::<bool>(),
            unseen in any::<u32>(),
            vcs in prop::option::of("[\\PC]{0,40}"),
            cursor in prop::option::of((any::<u32>(), any::<u32>())),
        ) {
            let vm = StatusLineVm {
                mode,
                file: path.as_deref().map(|path| FileVm { path, dirty }),
                session,
                ask_pending,
                unseen,
                vcs: vcs.as_deref(),
                cursor: cursor.map(|(line, col)| CursorVm { line, col }),
            };
            let buf = render(&vm, width, height);
            prop_assert_eq!(cells_outside(&buf, width), Vec::new());
        }

        /// The chip is always visible and always the only inverted run, at
        /// every width in the same range — the invariant §5 states and the one
        /// a shed ladder is most likely to break.
        #[test]
        fn the_chip_survives_every_width(
            width in 40u16..=200,
            session in any_session(),
            unseen in any::<u32>(),
        ) {
            let theme = Theme::phosphor_dark();
            let vm = StatusLineVm {
                mode: Mode::Insert,
                session,
                unseen,
                ..full_vm()
            };
            let buf = render(&vm, width, 1);
            prop_assert_eq!(buf[(2, 2)].bg, theme.actors.you);
            let inverted: Vec<u16> = (2..2 + width)
                .filter(|x| buf[(*x, 2)].bg != theme.chrome.statusline)
                .collect();
            let contiguous: Vec<u16> = (2..2 + inverted.len() as u16).collect();
            prop_assert_eq!(inverted, contiguous);
        }
    }
}
