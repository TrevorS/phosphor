//! `Float` — the one chrome primitive (`T084`). Design Language §4, §8, §9.
//!
//! > "§4 — One border style; border color is the float's mood: `#2a5c44`
//! > informational, `#6b5426` needs-you (body `#171207`), `#2a3c2e` passive
//! > (completion — no footer, the exception). Background under a float dims
//! > code to `#232823`. No shadows, no rounded corners beyond the terminal's
//! > cell, no titlebars."
//!
//! > "§9 — At most one float has focus. Opening a second replaces the first;
//! > esc closes top-down. **There is no float-over-float, ever.**"
//!
//! # Three contracts, and how each is held
//!
//! **Header / body / footer.** [`Float`]'s constructors take all three. There
//! is no way to build one without a header or a footer, so a body cannot land
//! on screen bare. (`T038`'s passive completion float is §4's one documented
//! exception and is the reason `footer` is an `Option` *internally* — see
//! [`Float::informational`].)
//!
//! **One float, never two.** [`FloatSlot`] holds `Option<Float>` and is the
//! **only** public path from a `Float` to a [`Buffer`] — [`Float`]'s own
//! renderer is crate-private. Stacking is therefore not something this module
//! prevents; it is something it cannot express. [`FloatSlot::open`] returns the
//! float it displaced, so the replacement is observable rather than silent, and
//! [`FloatSlot::close`] is `esc`: there is only ever one level to close.
//!
//! **Mood.** Border, body, rule and header colours all come off [`Mood`] via
//! `&Theme` (§12). No widget in this crate constructs a colour.
//!
//! # Geometry (§8, §11)
//!
//! * Spans 60–80% of width, centered, never within [`MIN_EDGE_GAP`] columns of
//!   an edge. The target is the top of that band, which is what mockups `3d`
//!   and `7a` draw.
//! * **Full-width under 100 columns** (§11). Mockup `8d` is the only drawing of
//!   that case and it docks the float to the bottom of the buffer area with a
//!   single mood-coloured rule on top and no side or bottom border — the
//!   terminal's own edges are the sides. That is [`Layout::FullWidth`].
//! * Padding 1 row / 2 cols.
//! * *"No surface is ever taller than its content"* — height comes from
//!   [`FloatBody::desired_height`], clamped to the area.
//!
//! # The body seam
//!
//! Five bodies plug in later, each with its own task: `Picker` (`T045`),
//! `DiffBody` (`T063`), `QuestionBody` (`T059`), `HelpGrid` (`T086`),
//! `ArchDiagram` (`T048`). [`FloatBody`] is deliberately two object-safe
//! methods — *how tall are you at this width* and *draw into this rect* — so a
//! body can be a `&dyn` built per frame from its own ViewModel, and so the
//! chrome never needs to know which of the five it is holding. [`TextBody`] is
//! the fixture that proves the seam without pre-empting any of them.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::style::{Color, Style};
use ratatui_core::symbols::line;
use ratatui_core::text::Span;

use crate::theme::Theme;

/// §11: *"floats go full-width"* below this width.
pub const FULL_WIDTH_BELOW: u16 = 100;

/// §8: *"never within 4 cols of an edge."*
pub const MIN_EDGE_GAP: u16 = 4;

/// §8: *"Float padding: 1 row / 2 cols."*
pub const PAD_COLS: u16 = 2;

/// §8: *"Float padding: 1 row / 2 cols."*
pub const PAD_ROWS: u16 = 1;

/// §8: *"Floats span 60–80% of width"* — the bottom of the band.
pub const WIDTH_PCT_MIN: u16 = 60;

/// §8: *"Floats span 60–80% of width"* — the top of the band, and the target.
/// Mockup `3d` draws 80%, `7a` 78%.
pub const WIDTH_PCT_MAX: u16 = 80;

/// Rows between the top of the area and a centered float, as drawn in `3d`
/// (44px at a 22px line) and `7a` (56px at 20px). Floats sit near the top, not
/// vertically centered — §8 only centers them horizontally.
pub const TOP_MARGIN: u16 = 2;

/// The float's mood, which is the only thing its border colour means (§4).
///
/// `T038` adds the third, `Mood::Passive` (`#2a3c2e`, **no footer** — §4's one
/// documented exception). It is deliberately absent rather than stubbed: an
/// unconstructable variant is untestable chrome. The shape it needs is already
/// here — [`Float::footer`] is an `Option`, and every colour decision in this
/// file is one `match` on this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mood {
    /// `#2a5c44`. Pickers, help, diffs, anything you asked for (`3c`, `3d`,
    /// `8d`).
    Informational,
    /// `#6b5426` with a `#171207` body. Questions and permission asks (`7a`,
    /// `2d`).
    NeedsYou,
}

impl Mood {
    /// The border, and the docked variant's top rule.
    #[must_use]
    pub const fn border(self, theme: &Theme) -> Color {
        match self {
            Self::Informational => theme.float.informational,
            Self::NeedsYou => theme.float.needs_you,
        }
    }

    /// The background of the whole float, header and footer included — both
    /// `3d` and `7a` set it on the container, not on the body alone.
    #[must_use]
    pub const fn body(self, theme: &Theme) -> Color {
        match self {
            Self::Informational => theme.float.body,
            Self::NeedsYou => theme.float.needs_you_body,
        }
    }

    /// The header/body and body/footer rules *inside* the border.
    #[must_use]
    pub const fn rule(self, theme: &Theme) -> Color {
        match self {
            Self::Informational => theme.chrome.divider,
            Self::NeedsYou => theme.float.needs_you_rule,
        }
    }

    /// The header's left half. §4: *"header — source or command · meta right."*
    /// A needs-you header speaks in amber (`7a`: `✻ claude · wants to run`).
    #[must_use]
    pub const fn header_fg(self, theme: &Theme) -> Color {
        match self {
            Self::Informational => theme.neutrals.text,
            Self::NeedsYou => theme.actors.attention,
        }
    }
}

/// Which of §8/§11's two shapes a float takes at this width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    /// 60–80% of width, centered, boxed on all four sides.
    Centered,
    /// Under 100 columns: the full width of the area, docked to its bottom,
    /// with a mood-coloured rule on top and no side or bottom border (`8d`).
    FullWidth,
}

impl Layout {
    /// §11's threshold, and the only place it is decided.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < FULL_WIDTH_BELOW {
            Self::FullWidth
        } else {
            Self::Centered
        }
    }

    /// Rows of chrome above and below the body, padding included.
    const fn chrome_rows(self, has_footer: bool) -> u16 {
        // border/rule + header + rule + PAD_ROWS … PAD_ROWS [+ rule + footer]
        let top = match self {
            Self::Centered => 1 + 1 + 1 + PAD_ROWS,
            Self::FullWidth => 1 + 1 + 1 + PAD_ROWS,
        };
        let bottom = match (self, has_footer) {
            (Self::Centered, true) => PAD_ROWS + 1 + 1 + 1,
            (Self::Centered, false) => PAD_ROWS + 1,
            (Self::FullWidth, true) => PAD_ROWS + 1 + 1,
            (Self::FullWidth, false) => PAD_ROWS,
        };
        top + bottom
    }

    /// Columns of chrome left and right of the body, padding included.
    const fn chrome_cols(self) -> u16 {
        match self {
            Self::Centered => 2 * (1 + PAD_COLS),
            Self::FullWidth => 2 * PAD_COLS,
        }
    }
}

/// §4: *"header — source or command · meta right."*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatHeader<'a> {
    /// The source or command — `❯ files`, `✻ claude · wants to run`.
    pub left: &'a str,
    /// Right-aligned meta, in meta-gray. Dropped before the left half when the
    /// two cannot both fit.
    pub right: Option<&'a str>,
}

impl<'a> FloatHeader<'a> {
    /// A header with no meta half.
    #[must_use]
    pub const fn new(left: &'a str) -> Self {
        Self { left, right: None }
    }

    /// Add the right-aligned meta half.
    #[must_use]
    pub const fn meta(mut self, right: &'a str) -> Self {
        self.right = Some(right);
        self
    }
}

/// One footer hint: the key, and the verb it performs.
///
/// §6: *"Keyhints spell the whole command … never cryptic contractions."*
/// `esc` carries no verb — every mockup draws it bare, and §6 puts it last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterHint<'a> {
    /// The key as typed: `↵`, `s`, `esc`.
    pub key: &'a str,
    /// What it does: `open`, `mark seen`. `None` for a bare key.
    pub verb: Option<&'a str>,
}

impl<'a> FooterHint<'a> {
    /// `↵ open`.
    #[must_use]
    pub const fn new(key: &'a str, verb: &'a str) -> Self {
        Self {
            key,
            verb: Some(verb),
        }
    }

    /// A bare key — in practice only `esc`.
    #[must_use]
    pub const fn bare(key: &'a str) -> Self {
        Self { key, verb: None }
    }
}

/// §4: *"footer — every legal key, always visible."*
///
/// The data, not the layout: `T034`'s `KeymapFooter` takes this over and feeds
/// it from the live keymap, so Steel rebinds appear here without this file
/// changing. Until then it renders `key verb · key verb · esc`, which is what
/// `3d`, `7a` and `8d` draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatFooter<'a> {
    /// Primary action first, `esc` last (§6).
    pub hints: &'a [FooterHint<'a>],
}

impl<'a> FloatFooter<'a> {
    /// Wrap a hint list.
    #[must_use]
    pub const fn new(hints: &'a [FooterHint<'a>]) -> Self {
        Self { hints }
    }
}

/// What plugs into a float's body.
///
/// Object-safe on purpose: a float holds `&dyn FloatBody`, so the five real
/// bodies never need a common enum and the chrome never needs to know which
/// one it has. Implementors **must** clip to the `area` they are handed; it is
/// already intersected with the float's frame and the buffer.
pub trait FloatBody: core::fmt::Debug {
    /// How many rows this body wants at `width`. The float clamps it to what
    /// the screen has — §8: *"No surface is ever taller than its content."*
    fn desired_height(&self, width: u16) -> u16;

    /// Draw into `area`, which is already inset by the border and the 2-column
    /// padding. `mood` is passed because a body on the amber needs-you ground
    /// picks different foregrounds than one on the informational ground.
    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, mood: Mood);
}

/// The fixture body: plain lines in the neutral text colour, no wrapping
/// (§11: *"Nothing ever wraps"*).
///
/// It exists so `T084`'s contract can be exercised — in tests, in a tape, and
/// by hand at `CP-1` — before any of the five real bodies is written. It is
/// also the degenerate case each of them must still look like: a rectangle of
/// rows that clips to its area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBody<'a> {
    /// One row each, in order.
    pub lines: &'a [&'a str],
}

impl<'a> TextBody<'a> {
    /// Wrap a slice of lines.
    #[must_use]
    pub const fn new(lines: &'a [&'a str]) -> Self {
        Self { lines }
    }
}

impl FloatBody for TextBody<'_> {
    fn desired_height(&self, _width: u16) -> u16 {
        u16::try_from(self.lines.len()).unwrap_or(u16::MAX)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, _mood: Mood) {
        let style = Style::new().fg(theme.neutrals.text);
        for (i, text) in self.lines.iter().enumerate() {
            let Ok(dy) = u16::try_from(i) else { break };
            if dy >= area.height {
                break;
            }
            buf.set_stringn(area.x, area.y + dy, text, area.width as usize, style);
        }
    }
}

/// The one chrome primitive: header / body / footer inside a mood border.
///
/// Built per frame from a ViewModel and handed to a [`FloatSlot`], which is the
/// only thing that can draw it.
#[derive(Debug, Clone, Copy)]
pub struct Float<'a> {
    mood: Mood,
    header: FloatHeader<'a>,
    /// `None` is reachable only from `T038`'s passive constructor, which does
    /// not exist yet. Every float you can build today has a footer.
    footer: Option<FloatFooter<'a>>,
    body: &'a dyn FloatBody,
}

impl<'a> Float<'a> {
    /// An informational float — `#2a5c44` border, `#101410` body (`3c`, `3d`,
    /// `8d`). Pickers, help, diffs: anything you asked for.
    #[must_use]
    pub const fn informational(
        header: FloatHeader<'a>,
        body: &'a dyn FloatBody,
        footer: FloatFooter<'a>,
    ) -> Self {
        Self {
            mood: Mood::Informational,
            header,
            footer: Some(footer),
            body,
        }
    }

    /// A needs-you float — `#6b5426` border, `#171207` body (`7a`, `2d`).
    /// Questions and permission asks.
    ///
    /// [Q9] governs *when* one of these appears: an ask queues and sets the
    /// statusline `!` flag rather than replacing whatever float is open, which
    /// is what keeps §9's one-float rule from destroying a picker under the
    /// user. That is the caller's decision; this type only draws.
    ///
    /// [Q9]: ../../../docs/IMPLEMENTATION-PLAN.md
    #[must_use]
    pub const fn needs_you(
        header: FloatHeader<'a>,
        body: &'a dyn FloatBody,
        footer: FloatFooter<'a>,
    ) -> Self {
        Self {
            mood: Mood::NeedsYou,
            header,
            footer: Some(footer),
            body,
        }
    }

    /// This float's mood.
    #[must_use]
    pub const fn mood(&self) -> Mood {
        self.mood
    }

    /// Where this float lands inside `area` — public because click routing and
    /// the dim both need it, and because it is the testable half of §8's
    /// geometry rules.
    #[must_use]
    pub fn frame(&self, area: Rect) -> Rect {
        let layout = Layout::for_width(area.width);
        let width = match layout {
            Layout::FullWidth => area.width,
            Layout::Centered => {
                let target = area.width * WIDTH_PCT_MAX / 100;
                let floor = area.width * WIDTH_PCT_MIN / 100;
                // The 4-column rule wins over the band when they disagree,
                // which they only can on a very narrow "centered" area.
                let capped = area.width.saturating_sub(2 * MIN_EDGE_GAP);
                target.min(capped).max(floor.min(capped))
            }
        };

        let body_width = width.saturating_sub(layout.chrome_cols());
        let content = self.body.desired_height(body_width);
        let height = layout
            .chrome_rows(self.footer.is_some())
            .saturating_add(content)
            .min(area.height);

        match layout {
            Layout::FullWidth => Rect {
                x: area.x,
                y: area.bottom().saturating_sub(height),
                width,
                height,
            },
            Layout::Centered => {
                let x = area.x + (area.width - width) / 2;
                let y = if area.y + TOP_MARGIN + height <= area.bottom() {
                    area.y + TOP_MARGIN
                } else {
                    area.bottom().saturating_sub(height).max(area.y)
                };
                Rect {
                    x,
                    y,
                    width,
                    height,
                }
            }
        }
    }

    /// Draw. Crate-private: the only public route to a screen is
    /// [`FloatSlot::render`], and that is what makes float-over-float
    /// unrepresentable rather than merely discouraged.
    fn render_into(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let frame = self.frame(area).intersection(buf.area);
        if frame.is_empty() {
            return;
        }
        let layout = Layout::for_width(area.width);
        let mut canvas = Canvas { buf, rect: frame };

        // Ground first: §4 puts the mood's body colour behind the whole float,
        // header and footer included (both 3d and 7a set it on the container).
        let ground = Style::new()
            .fg(theme.neutrals.text)
            .bg(self.mood.body(theme));
        for y in frame.y..frame.bottom() {
            canvas.fill_row(y, ground);
        }

        let border = Style::new()
            .fg(self.mood.border(theme))
            .bg(self.mood.body(theme));
        let rule = Style::new()
            .fg(self.mood.rule(theme))
            .bg(self.mood.body(theme));
        let meta = Style::new()
            .fg(theme.neutrals.meta)
            .bg(self.mood.body(theme));

        // Rows, top to bottom. Each is skipped if the frame was clamped short
        // of it — a float never grows to fit its chrome.
        let inset = match layout {
            Layout::Centered => 1,
            Layout::FullWidth => 0,
        };
        let text_x = frame.x + inset + PAD_COLS;
        let text_w = frame.width.saturating_sub(2 * (inset + PAD_COLS));

        match layout {
            Layout::Centered => canvas.box_border(frame, border),
            // 8d: one mood-coloured rule on top, no sides, no bottom.
            Layout::FullWidth => canvas.hrule(frame.y, frame.x, frame.width, border, false),
        }

        let header_y = frame.y + 1;
        canvas.header(text_x, header_y, text_w, self.header, self.mood, theme);

        let rule_y = frame.y + 2;
        canvas.hrule(rule_y, frame.x, frame.width, rule, inset == 1);

        let footer_rows = if self.footer.is_some() { 2 } else { 0 };
        let body_top = rule_y + 1 + PAD_ROWS;
        let body_bottom = frame
            .bottom()
            .saturating_sub(inset + PAD_ROWS + footer_rows);
        if body_bottom > body_top {
            let inner = Rect {
                x: text_x,
                y: body_top,
                width: text_w,
                height: body_bottom - body_top,
            }
            .intersection(frame);
            if !inner.is_empty() {
                self.body.render(inner, buf, theme, self.mood);
            }
        }

        if let Some(footer) = self.footer {
            let footer_y = frame.bottom().saturating_sub(1 + inset);
            let footer_rule_y = footer_y.saturating_sub(1);
            if footer_rule_y > rule_y {
                canvas = Canvas { buf, rect: frame };
                canvas.hrule(footer_rule_y, frame.x, frame.width, rule, inset == 1);
                canvas.footer(text_x, footer_y, text_w, footer, meta);
            }
        }
    }
}

/// **The one-float rule, as a type** (§9).
///
/// Holds at most one float, because it holds an `Option`. [`open`] replaces and
/// hands back what it displaced; [`close`] is `esc`. There is no `push`, no
/// stack, and no renderer anywhere in this crate that takes more than one
/// float — so *"no float-over-float, ever"* is a property of the API rather
/// than a rule someone has to remember.
///
/// The slot is built per frame from whatever the composition layer says is
/// open; *which* float that is, is the view tree's state ([Q12], `spine`), not
/// this crate's — widgets own no state.
///
/// [`open`]: FloatSlot::open
/// [`close`]: FloatSlot::close
/// [Q12]: ../../../docs/IMPLEMENTATION-PLAN.md
#[derive(Debug, Clone, Copy, Default)]
pub struct FloatSlot<'a> {
    occupant: Option<Float<'a>>,
}

impl<'a> FloatSlot<'a> {
    /// Nothing open.
    #[must_use]
    pub const fn empty() -> Self {
        Self { occupant: None }
    }

    /// A slot with one float in it.
    #[must_use]
    pub const fn with(float: Float<'a>) -> Self {
        Self {
            occupant: Some(float),
        }
    }

    /// §9: *"Opening a second replaces the first."* Returns the displaced
    /// float, so a caller that wants to know it destroyed something can.
    pub fn open(&mut self, float: Float<'a>) -> Option<Float<'a>> {
        self.occupant.replace(float)
    }

    /// §9: *"esc closes top-down"* — and there is only ever one level.
    pub fn close(&mut self) -> Option<Float<'a>> {
        self.occupant.take()
    }

    /// Whether anything is open. Focus routing reads this: §9's *"keystrokes go
    /// where they went before the float appeared."*
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.occupant.is_some()
    }

    /// The open float, if any.
    #[must_use]
    pub const fn occupant(&self) -> Option<&Float<'a>> {
        self.occupant.as_ref()
    }

    /// Dim `area` and draw the float over it — the only public way a [`Float`]
    /// reaches a [`Buffer`].
    ///
    /// §9: *"Dimming means 'behind.' Code under a float renders at `#232823`;
    /// panes never dim each other."* The dim recolours foregrounds and leaves
    /// backgrounds alone, which is exactly what mockups `3c`/`3d`/`7a` draw:
    /// the code is still there, in `#232823`, on the same ground.
    ///
    /// An empty slot draws nothing at all — no dim, no frame.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let Some(float) = self.occupant.as_ref() else {
            return;
        };
        let dim = area.intersection(buf.area);
        if dim.is_empty() {
            return;
        }
        buf.set_style(dim, Style::new().fg(theme.neutrals.dimmed_under_float));
        float.render_into(area, buf, theme);
    }
}

/// A clipped drawing surface. Every write checks the frame, so a clamped float
/// (one whose content is taller than the screen) draws a prefix of itself
/// rather than panicking or spilling.
#[derive(Debug)]
struct Canvas<'b> {
    buf: &'b mut Buffer,
    rect: Rect,
}

impl Canvas<'_> {
    fn put(&mut self, x: u16, y: u16, symbol: &str, style: Style) {
        if self.rect.contains(Position::new(x, y)) {
            self.buf[(x, y)].set_symbol(symbol).set_style(style);
        }
    }

    fn text(&mut self, x: u16, y: u16, max_width: u16, text: &str, style: Style) {
        if !self.rect.contains(Position::new(x, y)) || max_width == 0 {
            return;
        }
        let room = self.rect.right().saturating_sub(x).min(max_width);
        self.buf.set_stringn(x, y, text, room as usize, style);
    }

    fn fill_row(&mut self, y: u16, style: Style) {
        for x in self.rect.x..self.rect.right() {
            self.put(x, y, " ", style);
        }
    }

    /// A horizontal rule spanning `[x, x + width)`. `caps` replaces the two end
    /// cells with `├`/`┤`, which is what a rule inside a boxed float needs and
    /// a docked one (`8d`, no side borders) does not.
    fn hrule(&mut self, y: u16, x: u16, width: u16, style: Style, caps: bool) {
        for dx in 0..width {
            self.put(x + dx, y, line::HORIZONTAL, style);
        }
        if caps && width >= 2 {
            self.put(x, y, line::VERTICAL_RIGHT, style);
            self.put(x + width - 1, y, line::VERTICAL_LEFT, style);
        }
    }

    /// §4: one border style, square corners (*"no rounded corners beyond the
    /// terminal's cell"*).
    fn box_border(&mut self, frame: Rect, style: Style) {
        let (l, r) = (frame.x, frame.right() - 1);
        let (t, b) = (frame.y, frame.bottom() - 1);
        for x in l..=r {
            self.put(x, t, line::HORIZONTAL, style);
            self.put(x, b, line::HORIZONTAL, style);
        }
        for y in t..=b {
            self.put(l, y, line::VERTICAL, style);
            self.put(r, y, line::VERTICAL, style);
        }
        self.put(l, t, line::TOP_LEFT, style);
        self.put(r, t, line::TOP_RIGHT, style);
        self.put(l, b, line::BOTTOM_LEFT, style);
        self.put(r, b, line::BOTTOM_RIGHT, style);
    }

    /// §4: *"header — source or command · meta right."* The meta half is
    /// dropped, never squeezed, when both cannot fit.
    fn header(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        header: FloatHeader<'_>,
        mood: Mood,
        theme: &Theme,
    ) {
        let bg = mood.body(theme);
        let left = Style::new().fg(mood.header_fg(theme)).bg(bg);
        let meta = Style::new().fg(theme.neutrals.meta).bg(bg);

        let right_w = header
            .right
            .map_or(0, |r| u16::try_from(Span::raw(r).width()).unwrap_or(0));
        let left_w = u16::try_from(Span::raw(header.left).width()).unwrap_or(0);

        // One space minimum between the two halves, or the meta half goes.
        let fits = header.right.is_some() && left_w + right_w < width;
        let left_room = if fits { width - right_w - 1 } else { width };
        self.text(x, y, left_room, header.left, left);
        if let Some(right) = header.right
            && fits
        {
            self.text(x + width - right_w, y, right_w, right, meta);
        }
    }

    /// `↵ open · s mark seen · esc` — §6's midline dot inside a fact, primary
    /// first, escape last. All of it meta-gray, as `3d`, `7a` and `8d` draw it.
    fn footer(&mut self, x: u16, y: u16, width: u16, footer: FloatFooter<'_>, style: Style) {
        let mut text = String::new();
        for (i, hint) in footer.hints.iter().enumerate() {
            if i > 0 {
                text.push_str(" · ");
            }
            text.push_str(hint.key);
            if let Some(verb) = hint.verb {
                text.push(' ');
                text.push_str(verb);
            }
        }
        self.text(x, y, width, &text, style);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Float, FloatFooter, FloatHeader, FloatSlot, FooterHint, Layout, MIN_EDGE_GAP, Mood,
        TextBody, WIDTH_PCT_MAX, WIDTH_PCT_MIN,
    };
    use crate::theme::Theme;
    use ratatui_core::buffer::{Buffer, Cell};
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Color;

    const HINTS: &[FooterHint<'static>] = &[
        FooterHint::new("↵", "open"),
        FooterHint::new("s", "mark seen"),
        FooterHint::bare("esc"),
    ];

    const BODY: TextBody<'static> = TextBody::new(&[
        "▸ src/retry.rs:6–10   +RetryPolicy struct",
        "  src/retry.rs:12–24  +retry_with_backoff",
    ]);

    fn area() -> Rect {
        Rect::new(0, 0, 120, 30)
    }

    fn informational() -> Float<'static> {
        Float::informational(
            FloatHeader::new("❯ unseen").meta("2 files · 6 regions"),
            &BODY,
            FloatFooter::new(HINTS),
        )
    }

    fn needs_you() -> Float<'static> {
        Float::needs_you(
            FloatHeader::new("✻ claude · wants to run"),
            &BODY,
            FloatFooter::new(HINTS),
        )
    }

    fn draw(float: Float<'_>, area: Rect) -> (Buffer, Rect) {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(Rect::new(0, 0, area.width, area.height));
        let frame = float.frame(area);
        FloatSlot::with(float).render(area, &mut buf, &theme);
        (buf, frame)
    }

    /// A row with its side borders and padding stripped — what is *in* it.
    fn bare(buf: &Buffer, frame: Rect, dy: u16) -> String {
        row(buf, frame, dy)
            .trim_matches(|c| c == '\u{2502}' || c == ' ')
            .to_string()
    }

    fn row(buf: &Buffer, frame: Rect, dy: u16) -> String {
        (frame.x..frame.right())
            .map(|x| buf[(x, frame.y + dy)].symbol())
            .collect()
    }

    #[test]
    fn informational_draws_the_documented_chrome() {
        let theme = Theme::phosphor_dark();
        let (buf, frame) = draw(informational(), area());

        // §4: one border style, mood colour, square corners.
        assert_eq!(buf[(frame.x, frame.y)].symbol(), "┌");
        assert_eq!(buf[(frame.x, frame.y)].fg, theme.float.informational);
        assert_eq!(buf[(frame.right() - 1, frame.bottom() - 1)].symbol(), "┘");
        // The mood's body colour is behind all of it.
        assert_eq!(buf[(frame.x + 5, frame.y + 1)].bg, theme.float.body);

        // header · rule · pad · body · pad · rule · footer
        assert!(row(&buf, frame, 1).contains("❯ unseen"));
        assert!(row(&buf, frame, 1).contains("2 files · 6 regions"));
        assert!(row(&buf, frame, 2).starts_with('├'));
        assert_eq!(
            bare(&buf, frame, 3),
            "",
            "the pad row carries only the border"
        );
        assert!(row(&buf, frame, 4).contains("+RetryPolicy struct"));
        let footer_row = frame.height - 2;
        assert!(row(&buf, frame, footer_row).contains("↵ open · s mark seen · esc"));
        assert!(row(&buf, frame, footer_row - 1).starts_with('├'));
    }

    #[test]
    fn needs_you_draws_the_amber_mood() {
        let theme = Theme::phosphor_dark();
        let (buf, frame) = draw(needs_you(), area());
        assert_eq!(buf[(frame.x, frame.y)].fg, theme.float.needs_you);
        assert_eq!(
            buf[(frame.x + 3, frame.y + 1)].bg,
            theme.float.needs_you_body
        );
        // The header speaks amber; the internal rule is the dark-amber variant.
        assert_eq!(buf[(frame.x + 3, frame.y + 1)].fg, theme.actors.attention);
        assert_eq!(
            buf[(frame.x + 3, frame.y + 2)].fg,
            theme.float.needs_you_rule
        );
        assert!(row(&buf, frame, 1).contains("✻ claude · wants to run"));
    }

    #[test]
    fn both_moods_keep_the_header_body_footer_contract() {
        for float in [informational(), needs_you()] {
            let (buf, frame) = draw(float, area());
            assert!(!row(&buf, frame, 1).trim().is_empty(), "header");
            assert!(row(&buf, frame, 4).contains("retry.rs"), "body");
            assert!(row(&buf, frame, frame.height - 2).contains("esc"), "footer");
        }
    }

    #[test]
    fn opening_a_second_float_replaces_the_first() {
        // §9's one-float rule, and T084's acceptance criterion.
        let mut slot = FloatSlot::empty();
        assert!(!slot.is_open());
        assert!(slot.open(informational()).is_none());
        let displaced = slot.open(needs_you());
        assert_eq!(
            displaced.map(|f| f.mood()),
            Some(Mood::Informational),
            "the first float is handed back, not stacked"
        );
        assert_eq!(slot.occupant().map(Float::mood), Some(Mood::NeedsYou));

        // And on screen: one frame, in the second float's mood.
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
        slot.render(area(), &mut buf, &theme);
        let frame = needs_you().frame(area());
        assert_eq!(buf[(frame.x, frame.y)].fg, theme.float.needs_you);
        assert!(row(&buf, frame, 1).contains("wants to run"));
        assert!(!row(&buf, frame, 1).contains("unseen"));
    }

    #[test]
    fn esc_closes_and_there_is_only_one_level() {
        let mut slot = FloatSlot::with(informational());
        assert!(slot.close().is_some());
        assert!(slot.close().is_none(), "nothing underneath, ever");
        assert!(!slot.is_open());
    }

    #[test]
    fn an_empty_slot_draws_nothing_not_even_the_dim() {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
        FloatSlot::empty().render(area(), &mut buf, &theme);
        let touched = (0..120)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .any(|(x, y)| buf[(x, y)].fg != Color::Reset || buf[(x, y)].symbol() != " ");
        assert!(!touched);
    }

    #[test]
    fn the_background_under_a_float_dims() {
        // §9: code under a float renders at the theme's dimmed colour.
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
        for x in 0..120u16 {
            buf[(x, 0)].set_symbol("x").set_fg(theme.neutrals.text);
        }
        FloatSlot::with(informational()).render(area(), &mut buf, &theme);
        // Row 0 is above a float that starts at TOP_MARGIN.
        assert_eq!(buf[(0, 0)].fg, theme.neutrals.dimmed_under_float);
        assert_eq!(buf[(0, 0)].symbol(), "x", "dimming recolours, never erases");
    }

    #[test]
    fn centered_geometry_holds_across_the_wide_band() {
        for width in FULL..=200u16 {
            let area = Rect::new(0, 0, width, 30);
            let frame = informational().frame(area);
            assert_eq!(Layout::for_width(width), Layout::Centered);
            let pct = u32::from(frame.width) * 100 / u32::from(width);
            assert!(
                (u32::from(WIDTH_PCT_MIN)..=u32::from(WIDTH_PCT_MAX)).contains(&pct),
                "width {width}: {pct}% is outside §8's 60–80 band"
            );
            assert!(frame.x >= MIN_EDGE_GAP, "width {width}: too near the left");
            assert!(
                frame.right() + MIN_EDGE_GAP <= width,
                "width {width}: too near the right"
            );
            // Centered to within the odd column.
            assert!(frame.x.abs_diff(width - frame.right()) <= 1);
        }
    }

    const FULL: u16 = super::FULL_WIDTH_BELOW;

    #[test]
    fn under_100_cols_the_float_goes_full_width() {
        // §11 + mockup 8d: full width, docked to the bottom, one mood rule on
        // top, no side or bottom border.
        let theme = Theme::phosphor_dark();
        for width in 40..FULL {
            let area = Rect::new(0, 0, width, 20);
            assert_eq!(Layout::for_width(width), Layout::FullWidth);
            let (buf, frame) = draw(informational(), area);
            assert_eq!(frame.x, 0);
            assert_eq!(frame.width, width);
            assert_eq!(frame.bottom(), 20, "docked to the bottom of the area");
            assert_eq!(buf[(0, frame.y)].symbol(), "─");
            assert_eq!(buf[(0, frame.y)].fg, theme.float.informational);
            assert_ne!(buf[(0, frame.y + 1)].symbol(), "│", "no side border");
        }
    }

    #[test]
    fn padding_is_one_row_and_two_cols() {
        // §8's spacing unit, checked against the frame rather than the mockup's
        // pixels: 2 columns from the border to the text, 1 blank row above the
        // body and 1 below it.
        let (buf, frame) = draw(informational(), area());
        let header = row(&buf, frame, 1);
        assert!(header.starts_with("│  ❯ unseen"), "{header:?}");
        // The pad rows carry the border and nothing else.
        assert_eq!(bare(&buf, frame, 3), "", "pad row above the body");
        assert_eq!(
            bare(&buf, frame, frame.height - 4),
            "",
            "pad row below the body"
        );
    }

    #[test]
    fn a_float_is_never_taller_than_its_content() {
        // §8. Two body lines: 2 borders + header + rule + 2 pad + 2 body +
        // rule + footer = 10.
        let frame = informational().frame(area());
        assert_eq!(frame.height, 10);
    }

    #[test]
    fn a_short_area_clamps_instead_of_spilling() {
        // Ten rows of chrome and content into a six-row area: the float is
        // clamped, not spilled, and the rows it does draw are still its own.
        let area = Rect::new(0, 0, 120, 6);
        let (buf, frame) = draw(informational(), area);
        assert!(frame.height <= 6 && frame.bottom() <= 6);
        for y in frame.bottom()..6 {
            for x in 0..120u16 {
                assert_eq!(buf[(x, y)].symbol(), " ", "row {y} is below the float");
            }
        }
        assert!(row(&buf, frame, 1).contains("❯ unseen"), "header survives");
    }

    #[test]
    fn the_header_meta_half_drops_before_it_squeezes() {
        // §11: drop, never squeeze.
        let float = Float::informational(
            FloatHeader::new(
                "❯ a source name long enough that the meta half cannot also fit on the row",
            )
            .meta("2 files · 6 regions"),
            &BODY,
            FloatFooter::new(HINTS),
        );
        let area = Rect::new(0, 0, 100, 30);
        let (buf, frame) = draw(float, area);
        let header = row(&buf, frame, 1);
        assert!(
            header.contains("long enough that the meta half"),
            "{header:?}"
        );
        assert!(!header.contains("6 regions"), "{header:?}");
    }

    #[test]
    fn cells_outside_the_frame_are_only_ever_dimmed() {
        let theme = Theme::phosphor_dark();
        let (buf, frame) = draw(informational(), area());
        for y in 0..30u16 {
            for x in 0..120u16 {
                let inside = frame.contains(ratatui_core::layout::Position::new(x, y));
                if !inside {
                    let cell: &Cell = &buf[(x, y)];
                    assert_eq!(cell.symbol(), " ");
                    assert_eq!(cell.fg, theme.neutrals.dimmed_under_float);
                }
            }
        }
    }
}
