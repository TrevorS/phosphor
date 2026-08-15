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
//! **Header / body / footer.** [`Float::informational`] and [`Float::needs_you`]
//! take all three. There is no way to build one of those without a header or a
//! footer, so a body cannot land on screen bare. [`Float::passive`] is §4's one
//! documented exception (`T038`) and takes **neither** — see [`Mood::Passive`]
//! for why the header goes too, which §4 does not say and mockup `7c` draws.
//!
//! **The LSP bodies (`T038`, `T039`) live here rather than in a module of their
//! own**, which is what `crate::interpret`'s own table already said would
//! happen: *"the completion list `is` a float in the passive mood … and `T039`
//! renders signature help through the same chrome."* [`CompletionList`] and
//! [`SignatureBody`] are two more [`FloatBody`] implementors and add no second
//! chrome path — the Design Brief's *"table stakes rendered in the same float
//! language"* is a structural claim here, not a stylistic one.
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
//! * **Anchored** ([`Layout::Anchored`], `T038`): the passive float hangs off a
//!   cell instead, sized to its content in *both* axes. §8's band and its
//!   4-column gap are rules for a float that owns the screen; a completion list
//!   that jumped to the middle of the terminal while you typed would be a worse
//!   bug than a narrow one. Mockup `7c` draws it at 44% of the width, beside the
//!   cursor, and that is the only drawing of this float there is.
//! * Padding 1 row / 2 cols — **the rows only where there is chrome to pad away
//!   from.** An anchored float has no header and no footer, and `7c` draws its
//!   first list row against the border, so [`Layout::Anchored`] keeps §8's two
//!   columns and spends no rows.
//! * *"No surface is ever taller than its content"* — height comes from
//!   [`FloatBody::desired_height`], clamped to the area. An anchored float's
//!   *width* comes from [`FloatBody::desired_width`] the same way, up to
//!   [`ANCHORED_WIDTH_PCT`] of the area — see below.
//!
//! # The anchored float's width band (`CP-4`)
//!
//! Reported by hand at `CP-4`: *"we need a max width for completion and hover
//! and stuff — right now it's very dynamic and will go from small to across the
//! screen."* Two complaints, and they are not the same one.
//!
//! **Too wide**, which is [`ANCHORED_WIDTH_PCT`]. §8 gives a float that owns the
//! screen a 60–80% band; §8 says nothing about a float you did not ask for, and
//! the rule this file settles on is that **the passive float's ceiling is the
//! centered band's floor** — a surface that appears while you type is never as
//! wide as one you opened. The measurement that supports the number is stated
//! once, on [`ANCHORED_WIDTH_PCT`], and everything else points at it.
//!
//! **Where this deviates, recorded rather than folded in.** Two places, both
//! read in the tree this session:
//!
//! * The *Component Breakdown*'s `Float` row states one width rule — *"Enforces
//!   the one-float rule (opening replaces), **full-width under 100 cols**,
//!   background dim"* — in the same sentence that names *"passive `#2a3c2e`
//!   no-footer"*. A capped anchored float at 80 columns is not full-width, and
//!   neither is an uncapped one: [`Layout::Anchored`] has never taken §11's
//!   rule, because a completion list that spanned the terminal would be the
//!   *"across the screen"* complaint with a border round it. The cap does not
//!   create that deviation; it narrows it.
//! * The design draws `7c` at one width and the repo captures it at two. At the
//!   drawn width nothing moves. At the repo's own 80-column capture the cap
//!   binds — 61 of 80 columns is 76% of a screen nobody asked for — and the
//!   frame is redrawn. Long content there degrades by *wrapping* rather than by
//!   vanishing, which is the paragraph below.
//!
//! **Too dynamic**, which is [`Float::with_width_floor`]. A float whose right
//! edge moves under the cursor on every keystroke is hard to read even when
//! every width it takes is legal, and the cap alone only bounds the excursion.
//! The fix is monotonicity — grow to fit, never shrink until the session is
//! dismissed — and monotonicity is **memory**, which a widget may not own
//! ([`FloatBody`] is built per frame over a ViewModel). So the floor is a
//! parameter: whoever holds the session across frames carries the widest it has
//! been and hands it back. Unset, it is `0` and the width is `7c`'s
//! content-sized behaviour, capped.
//!
//! **What was considered and rejected**, because both would redraw `7c`:
//! a percentage *floor* (40%, the 20 points under §8's band) inflates every
//! short hover into a half-screen box and scales the wrong way — *"don't take
//! the screen"* is a claim about the screen, *"stay readable"* is a claim about
//! content — and *quantising* the width to a tenth of the area rounds `7c`'s 61
//! columns up to 72, which is a change to the only drawing there is for a
//! reason no drawing can show.
//!
//! **Long content degrades legibly rather than vanishing**, and the two halves
//! of a row answer differently. A completion's *label and detail* truncate: the
//! label is what gets inserted, so it keeps the columns and the meta detail
//! loses them, each with §2's `⋯`. **Prose wraps** — hover, signature
//! documentation and the block under a completion list's rule — on the host's
//! side of the seam, because §11 is *"nothing ever wraps"* and both
//! [`SignatureVm::prose`] and [`CompletionVm::documentation`] are already one
//! string per row for exactly that reason. [`anchored_wrap_cols`] publishes the
//! number to wrap to and [`wrap_prose`] is the wrapping, so a host does not have
//! to reimplement it to agree with the chrome. A line that arrives unwrapped
//! anyway still truncates rather than disappearing — that is the backstop, and
//! it was the only behaviour there was until a review found the published width
//! had no caller.
//!
//! # The body seam
//!
//! Five bodies plug in later, each with its own task: `Picker` (`T045`),
//! `DiffBody` (`T063`), `QuestionBody` (`T059`), `HelpGrid` (`T086`),
//! `ArchDiagram` (`T048`). [`FloatBody`] is deliberately three object-safe
//! methods — *how tall are you at this width*, *how wide do you want to be* and
//! *draw into this rect* — so a body can be a `&dyn` built per frame from its
//! own ViewModel, and so the chrome never needs to know which of them it is
//! holding. [`TextBody`] is the fixture that proves the seam without pre-empting
//! any of them; [`CompletionList`] and [`SignatureBody`] are the first two real
//! ones.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::style::{Color, Style};
use ratatui_core::symbols::line;

use crate::interpret::cells;
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

/// The widest an anchored float may be, as a percentage of its area — **§8's
/// band floor, used as the passive float's ceiling** (`CP-4`).
///
/// The derivation is in the module docs and the short form is: a float that
/// appears while you type is never as wide as one you asked for, so the number
/// §8 gives as the narrowest a centered float may be is the widest this one may
/// be. It is [`WIDTH_PCT_MIN`]'s value and deliberately not that constant —
/// they are the same number for different reasons, and §8 moving its band
/// should not silently move this cap.
///
/// # The measurement, stated once
///
/// This is the only place the numbers live; the module docs, the tests and the
/// golden frames' notes point here rather than restating them, because four
/// copies of an arithmetic claim in prose is four things no lint can reconcile.
///
/// `docs/design/TUI Mockups.dc.html` draws `7c` in a `width:900px` container
/// with the list at `margin-left:290px;width:400px` — **44%**. Transcribed to
/// cells at the 120-column width the golden frames use, its widest row plus six
/// columns of chrome is 61 of 120 — **51%**. Both are under 60, so at the width
/// the design draws it the cap changes nothing, and
/// `crates/phosphor/tests/snapshots/screen_7c__7c.snap` is byte-identical
/// across its introduction.
///
/// **It does redraw the repo's own 80-column capture of the same session**,
/// where 61 columns is 76% of the screen. That capture is a test this repo
/// authored, not a drawing the design published, and what it now shows is the
/// documentation sentence wrapped over two rows instead of laid out over one —
/// see the module docs' deviation list.
pub const ANCHORED_WIDTH_PCT: u16 = 60;

/// §2's *"⋯ elided"*, the mark a truncated row ends in.
///
/// The tree's copy of this glyph is `interpret::glyph_str`'s `Glyph::Elided`
/// arm and a shared constant would mean editing that file, which belongs to
/// another owner (`TEAM.md`). Recorded rather than folded in: if the two ever
/// disagree, §2 is the arbiter and `interpret` is the one with the door.
const ELISION: &str = "⋯";

/// `percent` of `width`, rounded down, **without the intermediate that
/// overflows**.
///
/// The obvious `width * percent / 100` panics in a debug build for every
/// terminal wider than 819 columns at [`WIDTH_PCT_MAX`] (`820 * 80 = 65_600`,
/// past `u16::MAX`) — a real crash on an ultrawide split, and one this file
/// shipped in [`Layout::Centered`]'s arm until a review measured the threshold.
/// Splitting `width` into hundreds and a remainder keeps every product under
/// `655 * 100 + 99 * 100`, and it is **exact** rather than approximate:
/// `w = 100q + r` gives `floor(w·p/100) = q·p + floor(r·p/100)`, which is these
/// two terms.
const fn pct_of(width: u16, percent: u16) -> u16 {
    width / 100 * percent + width % 100 * percent / 100
}

/// The widest an anchored float may be in an area `area_width` columns across —
/// [`ANCHORED_WIDTH_PCT`] of it.
///
/// Public because the cap is a fact about the screen that the host needs before
/// it has a float: see [`anchored_wrap_cols`].
///
/// There is no clamp to `area_width` here and there was one, dead: for any
/// percentage under 100 the answer is strictly smaller than the area for every
/// width from 1 up, and 0 at width 0. A guard that cannot bind reads as a case
/// somebody thought about, so it is worse than nothing.
#[must_use]
pub const fn anchored_max_cols(area_width: u16) -> u16 {
    pct_of(area_width, ANCHORED_WIDTH_PCT)
}

/// The columns of *body* an anchored float offers at its widest — what a host
/// should wrap hover prose to.
///
/// §11 is *"nothing ever wraps"* and this crate honours it literally: a
/// [`SignatureVm::prose`] entry is one screen row. That puts wrapping on the
/// host's side of the seam, and a host cannot wrap to a width nobody told it.
/// This is that width. A line longer than it still renders — truncated with
/// [`ELISION`] — so an unwrapping host degrades rather than breaks.
#[must_use]
pub const fn anchored_wrap_cols(area_width: u16) -> u16 {
    anchored_max_cols(area_width).saturating_sub(2 * (1 + PAD_COLS))
}

/// Wraps prose to `cols` cells, greedily, on whitespace — the wrapping
/// [`anchored_wrap_cols`] names the width for.
///
/// **It lives beside the number instead of in the host** because a host that
/// wrapped to its own rule would disagree with the chrome it is wrapping for,
/// and the disagreement would show up as an `⋯` on a line the host believed it
/// had fitted. One function, one width, two callers that cannot drift: the
/// binary's `IngestHover`/`IngestSignatureHelp` arms and `Editing::completions`.
/// It is not the chrome wrapping at draw time — §11 stands, and every
/// [`FloatBody`] here still puts one string on one row.
///
/// **A word longer than `cols` gets a line of its own rather than being cut**:
/// a URL or a `Vec<HashMap<String, Vec<u8>>>` is one token, breaking it mid-way
/// invents a word that is not in the text, and the row it lands on truncates
/// with [`ELISION`] — which is the backstop saying *"there is more of this"*
/// rather than a silent lie. `cols == 0` is a screen with no room for prose and
/// hands the lines back untouched, so a degenerate width cannot loop.
///
/// **A line that already fits is passed through byte for byte**, indentation
/// included. A server sends its doc comment as markdown *source* — fenced code,
/// bullet lists, `# Panics` — and `split_whitespace` would flatten the leading
/// spaces that make those readable. Only a line that overruns is re-flowed, and
/// re-flowing indented source is a known loss recorded rather than solved here:
/// rendering markdown properly is the transcript's job at `S6`
/// (`phosphor_buffer::lsp::Completion`'s own note).
#[must_use]
pub fn wrap_prose(lines: &[String], cols: u16) -> Vec<String> {
    if cols == 0 {
        return lines.to_vec();
    }
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        // An empty line is a paragraph break the server put there; wrapping it
        // away would join two paragraphs that the prose keeps apart — and a
        // line that fits is not this function's business at all.
        if line.trim().is_empty() || cells(line) <= cols {
            out.push(line.clone());
            continue;
        }
        let mut row = String::new();
        let mut used = 0;
        for word in line.split_whitespace() {
            let width = cells(word);
            if used == 0 {
                row.push_str(word);
                used = width;
            } else if used.saturating_add(1).saturating_add(width) <= cols {
                row.push(' ');
                row.push_str(word);
                used = used + 1 + width;
            } else {
                out.push(core::mem::take(&mut row));
                row.push_str(word);
                used = width;
            }
        }
        if !row.is_empty() {
            out.push(row);
        }
    }
    out
}

/// Rows between the top of the area and a centered float, as drawn in `3d`
/// (44px at a 22px line) and `7a` (56px at 20px). Floats sit near the top, not
/// vertically centered — §8 only centers them horizontally.
pub const TOP_MARGIN: u16 = 2;

/// The float's mood, which is the only thing its border colour means (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mood {
    /// `#2a5c44`. Pickers, help, diffs, anything you asked for (`3c`, `3d`,
    /// `8d`).
    Informational,
    /// `#6b5426` with a `#171207` body. Questions and permission asks (`7a`,
    /// `2d`).
    NeedsYou,
    /// `#2a3c2e`, **no footer** — §4's one documented exception, and `T038`'s
    /// whole shape. Completion, signature help and hover (`7c`).
    ///
    /// # It has no header either, which §4 does not say
    ///
    /// §4 names the footer as the exception and is silent about the header.
    /// Mockup `7c` is the only drawing of this float that exists and it has
    /// neither: the first completion row sits against the top border. A header
    /// would have to say something, and the only thing there is to say is the
    /// word the server is completing — which is on the screen, one row up, with
    /// the cursor in it. **Flagged rather than folded in**: the tree agrees
    /// already ([`phosphor_core::view::Float`]'s `header` is an `Option` and its
    /// `footer` doc names this mood), so nothing had to be widened for it.
    ///
    /// # And it does not dim
    ///
    /// §9's dim means *"behind"*. This float is not in front of anything — you
    /// keep typing into the buffer underneath it, which is what *passive*
    /// names, and `7c` draws the code around it at full strength. See
    /// [`Mood::dims`].
    Passive,
}

impl Mood {
    /// The border, and the docked variant's top rule.
    #[must_use]
    pub const fn border(self, theme: &Theme) -> Color {
        match self {
            Self::Informational => theme.float.informational,
            Self::NeedsYou => theme.float.needs_you,
            Self::Passive => theme.float.passive,
        }
    }

    /// The background of the whole float, header and footer included — both
    /// `3d` and `7a` set it on the container, not on the body alone.
    #[must_use]
    pub const fn body(self, theme: &Theme) -> Color {
        match self {
            Self::Informational | Self::Passive => theme.float.body,
            Self::NeedsYou => theme.float.needs_you_body,
        }
    }

    /// The header/body and body/footer rules *inside* the border — and, for
    /// [`Mood::Passive`], the rule [`CompletionList`] draws above its
    /// documentation block.
    ///
    /// `7c` draws that one at `#1d241d` and this is `#242a24`
    /// ([`Theme::chrome`]'s `divider`, *"a float's header/body and body/footer
    /// rules"*). One step apart on the same neutral ramp, and §4 hexes no
    /// internal rule at all. Reusing the field the informational mood already
    /// uses for the same job beats adding a seventh chrome value that five other
    /// themes would have to invent — recorded, not silently reconciled.
    ///
    /// [`Theme::chrome`]: crate::theme::Theme::chrome
    #[must_use]
    pub const fn rule(self, theme: &Theme) -> Color {
        match self {
            Self::Informational | Self::Passive => theme.chrome.divider,
            Self::NeedsYou => theme.float.needs_you_rule,
        }
    }

    /// The header's left half. §4: *"header — source or command · meta right."*
    /// A needs-you header speaks in amber (`7a`: `✻ claude · wants to run`).
    #[must_use]
    pub const fn header_fg(self, theme: &Theme) -> Color {
        match self {
            Self::Informational | Self::Passive => theme.neutrals.text,
            Self::NeedsYou => theme.actors.attention,
        }
    }

    /// Whether the code behind this float dims to `#232823` (§9).
    ///
    /// True for the two moods that take the screen, false for
    /// [`Mood::Passive`] — see that variant. This is the one place §9's rule is
    /// decided, so a third dimming mood is one arm rather than a search.
    #[must_use]
    pub const fn dims(self) -> bool {
        match self {
            Self::Informational | Self::NeedsYou => true,
            Self::Passive => false,
        }
    }
}

/// Which of §8/§11's shapes a float takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    /// 60–80% of width, centered, boxed on all four sides.
    Centered,
    /// Under 100 columns: the full width of the area, docked to its bottom,
    /// with a mood-coloured rule on top and no side or bottom border (`8d`).
    FullWidth,
    /// Hung off an [`Anchor`] and sized to its content in both axes: `T038`'s
    /// completion float (`7c`). Boxed like [`Layout::Centered`], with no header,
    /// no footer and no padding rows.
    ///
    /// **Not width-derived**, which is why [`Layout::for_width`] never returns
    /// it: an anchored float is anchored at 200 columns and at 40.
    ///
    /// **It carries the anchor**, so [`Float::frame`] cannot reach the arm
    /// without one. The cell used to be re-read out of `Float::anchor` inside
    /// the arm, with a comment saying the `None` case was unreachable; a
    /// variant that holds it makes that branch unwritable instead of
    /// unreachable-and-explained.
    Anchored(Anchor),
}

impl Layout {
    /// §11's threshold, and the only place it is decided.
    ///
    /// Answers for a float with no [`Anchor`]; [`Layout::Anchored`] is chosen by
    /// the constructor, not by the width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < FULL_WIDTH_BELOW {
            Self::FullWidth
        } else {
            Self::Centered
        }
    }

    /// Rows of chrome above and below the body, padding included.
    ///
    /// **The footer is not a parameter, because the layout already decides
    /// it.** [`Float::informational`] and [`Float::needs_you`] both require one
    /// and neither can be anchored; [`Float::passive`] is the only constructor
    /// that omits it and it always anchors. So a footerless centered float was
    /// two arms nothing could reach — `cargo llvm-cov` reported zero executions
    /// on both while every other arm was hit.
    const fn chrome_rows(self, has_header: bool) -> u16 {
        // border/rule [+ header + rule + PAD_ROWS] … [PAD_ROWS] [+ rule +
        // footer] [+ border]
        let top = 1 + if has_header { 1 + 1 + PAD_ROWS } else { 0 };
        let bottom = match self {
            Self::Centered => PAD_ROWS + 1 + 1 + 1,
            Self::FullWidth => PAD_ROWS + 1 + 1,
            // §8's padding row has no chrome to hold it off the body here, and
            // `7c` draws the list against the border. See the module docs.
            Self::Anchored(_) => 1,
        };
        top + bottom
    }

    /// Columns of chrome left and right of the body, padding included.
    const fn chrome_cols(self) -> u16 {
        match self {
            Self::Centered | Self::Anchored(_) => 2 * (1 + PAD_COLS),
            Self::FullWidth => 2 * PAD_COLS,
        }
    }
}

/// The cell a passive float hangs off — in the same screen coordinates the
/// float is drawn in.
///
/// The completion the user is typing is *at* a place, and §8's centered band is
/// a rule for floats that own the screen. `7c` puts the list under the word
/// being completed; that word's first cell is what a host should pass.
///
/// **Screen cells, not buffer positions.** This crate cannot map a buffer
/// coordinate to a cell — the gutter width, the viewport and soft wrap all sit
/// between the two — and the host is already doing that arithmetic to draw the
/// cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Anchor {
    /// Column of the cell, absolute.
    pub col: u16,
    /// Row of the cell, absolute. The float lands on the row *below* this one
    /// when there is room, and above it when there is not.
    pub row: u16,
}

impl Anchor {
    /// An anchor at `(col, row)`.
    #[must_use]
    pub const fn new(col: u16, row: u16) -> Self {
        Self { col, row }
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

    /// How many columns this body wants, chrome excluded.
    ///
    /// **Only [`Layout::Anchored`] asks.** A centered float takes §8's 60–80%
    /// band and a full-width one takes the terminal, so for those two the answer
    /// changes nothing; an anchored float is sized to its content in both axes
    /// and has nothing else to go on. Required rather than defaulted because a
    /// body that cannot say how wide it is is a body that draws a completion
    /// list six columns wide the first time someone anchors it.
    fn desired_width(&self) -> u16;

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

    fn desired_width(&self) -> u16 {
        self.lines.iter().copied().map(cells).max().unwrap_or(0)
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

// ---------------------------------------------------------------------------
// The LSP bodies (`T038`, `T039`)
// ---------------------------------------------------------------------------

/// Two spaces between the label column and the detail column, which is what
/// `7c` draws: its three labels are 9, 13 and 11 cells and every detail starts
/// at column 15.
const DETAIL_GAP: u16 = 2;

/// The most rows of completions a list draws, however many it holds.
///
/// **A passive float is the one float you did not ask for**, so it is the one
/// that may not take the screen: §9's *"not in front of anything"* is a claim
/// about the code staying readable underneath, and a list sized to its content
/// breaks it the moment a server answers in earnest. Measured at `CP-4` before
/// this existed: one `.` against rust-analyzer drew rows 0–28 of a 30-row
/// terminal and the entire editor was behind it.
///
/// Ten is the number `pumheight` is usually set to in the editor this one is
/// modelled on — vim's own default is *"as much room as there is"*, which is
/// exactly the behaviour measured above. The list still scrolls:
/// [`CompletionList::scroll`] keeps the selected row on screen, which is what
/// makes a cap a *window* rather than a truncation. `7c` draws three.
pub const MAX_ITEM_ROWS: u16 = 10;

/// The most rows of documentation under the rule.
///
/// `7c` draws **one**. A real server sends the item's whole doc comment as
/// markdown *source* — `# Panics`, fenced code, the lot — and rendering that is
/// the transcript's job at `S6` (`phosphor_buffer::lsp::Completion`'s own
/// note). Fourteen rows of it under a three-row list is a float that is mostly
/// not the list, so the block is a summary here and stops at four.
pub const MAX_DOC_ROWS: u16 = 4;

/// The most rows of prose a signature or hover float draws, for the same reason
/// [`MAX_DOC_ROWS`] exists one surface over.
///
/// Larger, because for hover the prose **is** the answer — there is no list
/// above it competing for the rows.
///
/// **It counts rows after [`wrap_prose`], which is a trade and not an
/// oversight.** A host that wraps turns one 130-column server line into three
/// rows, so a long hover now runs out of *rows* where it used to run out of
/// *columns*. That is the better end to lose from: eight whole rows say more
/// than six rows each cut at 66 columns with an `⋯` no key reveals, and the
/// remainder is a scroll (`T045`'s picker chrome) rather than a wider float.
/// Recorded because the number was chosen before anything wrapped.
pub const MAX_PROSE_ROWS: u16 = 8;

/// One row of the completion list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionItemVm {
    /// What the server would insert — `default()`, `default_delay` (`7c`).
    pub label: String,
    /// The type or shape, right of the label column in meta-grey:
    /// `fn() -> RetryPolicy`, `Duration`. Absent for a server that sends none.
    pub detail: Option<String>,
}

/// The completion session, as one frame needs it (`T038`).
///
/// **Owned, and handed out by reference** through
/// [`crate::interpret::Resources`], unlike [`crate::status_line::StatusLineVm`]
/// which the caller builds at the call site. A trait method cannot return a
/// borrowing view of data it does not own, and the host already holds the
/// session — so the host holds one of these and lends it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionVm {
    /// The items, in the order the server ranked them.
    pub items: Vec<CompletionItemVm>,
    /// Which row is selected. Out of range selects nothing, which is the honest
    /// reading of an empty list and of a session that has been filtered down.
    pub selected: usize,
    /// The selected item's documentation, one row per line — §11 is *"nothing
    /// ever wraps"*, so a host that wants two rows sends two strings. `7c`
    /// draws one: *"Returns the policy with 3 attempts, 200ms base, 1s cap."*
    pub documentation: Vec<String>,
    /// The cell the list hangs off: the first cell of the word being completed.
    pub anchor: Anchor,
    /// **The widest this session has been**, in body columns — what
    /// [`Float::with_width_floor`] is handed, and the anti-thrash half of
    /// `CP-4`. `0` is content-sizing.
    ///
    /// On the ViewModel because the ViewModel **is** the session: monotonicity
    /// is memory, a [`FloatBody`] is built per frame and may not own any, and
    /// the host that lends this through [`crate::interpret::Resources`] is the
    /// one thing that outlives a frame. It shipped as a [`Float`] parameter with
    /// no non-test caller — built, tested and uncomposed — and the field is what
    /// gives the number somewhere to live between two frames.
    pub width_floor: u16,
}

/// `7c`'s completion list: labels, a meta detail column, and the selected
/// item's documentation under a rule.
///
/// # What is drawn where the mockup differs
///
/// The selection tint (§3/§4: `#26332a` + bright text) covers the body's rows,
/// which are inset from the border by §8's two padding columns; `7c` runs it
/// edge to edge inside the border. Same for the documentation rule. A body is
/// handed an area *inside* the padding and *must* clip to it — that is
/// [`FloatBody`]'s contract, and `crate::interpret`'s existing
/// [`phosphor_core::view::Tint`] rows already tint exactly this rectangle. The
/// alternative is a body that can paint the chrome's columns, which is a bigger
/// hole than a two-column inset.
#[derive(Debug, Clone, Copy)]
pub struct CompletionList<'a> {
    vm: &'a CompletionVm,
}

impl<'a> CompletionList<'a> {
    /// The list over one session.
    #[must_use]
    pub const fn new(vm: &'a CompletionVm) -> Self {
        Self { vm }
    }

    /// Cells from the start of a row to the start of the detail column.
    fn label_col(&self) -> u16 {
        let widest = self
            .vm
            .items
            .iter()
            .map(|item| cells(&item.label))
            .max()
            .unwrap_or(0);
        widest.saturating_add(DETAIL_GAP)
    }

    /// Rows the documentation block occupies, rule included; zero when there is
    /// none.
    ///
    /// **And zero when there are no items**, which is not the same statement:
    /// documentation belongs to the *selected* completion, so a session with
    /// none is a closed session however much prose came with it. Without this a
    /// list that filtered down to nothing would leave a two-row float of orphan
    /// prose beside the cursor.
    fn doc_rows(&self) -> u16 {
        if self.vm.documentation.is_empty() || self.vm.items.is_empty() {
            0
        } else {
            u16::try_from(self.vm.documentation.len())
                .unwrap_or(u16::MAX)
                .min(MAX_DOC_ROWS)
                .saturating_add(1)
        }
    }

    /// The first item on screen when the list is taller than the room it has.
    ///
    /// **The selected row is always on screen**, which is the whole reason this
    /// is not just `0`: a completion list is scrolled with `ctrl-n` and a
    /// selection that walked off the bottom would leave the user steering
    /// something invisible.
    ///
    /// **Clamped against the list as well as against the rows**, because
    /// [`CompletionVm::selected`] declares an out-of-range selection legal
    /// (*"a session that has been filtered down"*). Scrolling to an item that
    /// is not there drew a float sized for every item with **none** of them in
    /// it — blank rows, a rule and the documentation, beside the cursor.
    fn scroll(&self, rows: u16) -> usize {
        let rows = rows as usize;
        let last = self.vm.items.len().saturating_sub(1);
        let selected = self.vm.selected.min(last);
        if rows == 0 || selected < rows {
            0
        } else {
            selected + 1 - rows
        }
    }
}

impl FloatBody for CompletionList<'_> {
    fn desired_height(&self, _width: u16) -> u16 {
        u16::try_from(self.vm.items.len())
            .unwrap_or(u16::MAX)
            .min(MAX_ITEM_ROWS)
            .saturating_add(self.doc_rows())
    }

    fn desired_width(&self) -> u16 {
        let label_col = self.label_col();
        self.vm
            .items
            .iter()
            .map(|item| {
                item.detail
                    .as_deref()
                    // An empty detail is no detail: counting the gap in front
                    // of one buys two columns of nothing, and for a list whose
                    // labels are empty too that is the whole float.
                    .filter(|detail| !detail.is_empty())
                    .map_or(0, |detail| label_col.saturating_add(cells(detail)))
                    .max(cells(&item.label))
            })
            // Only the rows that will be drawn: a float widened by the
            // twentieth line of a doc comment it does not show is wider than
            // anything on it.
            .chain(
                self.vm
                    .documentation
                    .iter()
                    .take(MAX_DOC_ROWS as usize)
                    .map(|line| cells(line)),
            )
            .max()
            .unwrap_or(0)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, mood: Mood) {
        let ground = mood.body(theme);
        // The documentation gives way to the items, never the other way round:
        // a one-row float shows the selected completion, not its prose.
        //
        // **And a rule with nothing under it is not documentation.** One row
        // left after the clamp is the rule alone — a separator separating one
        // thing from nothing — so the block goes rather than its content.
        let doc_rows = match self.doc_rows().min(area.height.saturating_sub(1)) {
            0 | 1 => 0,
            rows => rows,
        };
        let item_rows = area.height - doc_rows;
        let label_col = self.label_col();

        let mut canvas = Canvas { buf, rect: area };
        let first = self.scroll(item_rows);
        for dy in 0..item_rows {
            let Some(item) = self.vm.items.get(first + dy as usize) else {
                break;
            };
            let y = area.y + dy;
            let selected = first + dy as usize == self.vm.selected;
            // §4: "selection row gets #26332a + bright text".
            let (fg, bg) = if selected {
                (theme.neutrals.bright_text, theme.regions.selection)
            } else {
                (theme.neutrals.text, ground)
            };
            canvas.fill_row(y, Style::new().bg(bg));
            // **The label keeps the columns and the detail loses them**, which
            // is §11's *"drop, never squeeze"* applied to a row rather than to
            // a screen: the label is the text that gets inserted and the detail
            // is meta about it. So the label is elided only when it alone
            // overruns the float, and a label column wider than the whole body
            // takes every detail off the row rather than shaving both.
            canvas.text_elided(
                area.x,
                y,
                label_col.min(area.width),
                &item.label,
                Style::new().fg(fg).bg(bg),
            );
            if let Some(detail) = &item.detail
                && label_col < area.width
            {
                canvas.text_elided(
                    area.x + label_col,
                    y,
                    area.width - label_col,
                    detail,
                    Style::new().fg(theme.neutrals.meta).bg(bg),
                );
            }
        }

        if doc_rows == 0 {
            return;
        }
        let rule_y = area.y + item_rows;
        canvas.hrule(
            rule_y,
            area.x,
            area.width,
            Style::new().fg(mood.rule(theme)).bg(ground),
            false,
        );
        // Meta-grey, as `7c` draws it. §6's `#9aa39a` prose is claude's voice
        // and this is a server's.
        let prose = Style::new().fg(theme.neutrals.meta).bg(ground);
        for (i, line) in self.vm.documentation.iter().enumerate() {
            let Ok(dy) = u16::try_from(i) else { break };
            if dy + 1 >= doc_rows {
                break;
            }
            canvas.text_elided(area.x, rule_y + 1 + dy, area.width, line, prose);
        }
    }
}

/// Signature help and hover, which are one surface (`T039`).
///
/// **One type for two features, on purpose.** `textDocument/signatureHelp`
/// answers with a callable and the parameter you are inside; `textDocument/hover`
/// answers with prose about what is under the cursor. Both render as *"an
/// optional line of code, then prose"* in a passive float at the cursor, and a
/// second near-identical type would only make the two look different on screen.
/// Hover sets [`label`] to `None`; signature help sets it and usually sends no
/// prose.
///
/// [`label`]: SignatureVm::label
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignatureVm {
    /// The callable as the server spells it —
    /// `fn fetch_json(url: &str) -> Result<Value, FetchError>`. `None` for
    /// hover.
    pub label: Option<String>,
    /// The active parameter, as a **character** range into [`label`] —
    /// `(start, end)`, half-open, the way LSP's own parameter labels are given
    /// once converted off UTF-16.
    ///
    /// Characters rather than bytes or cells because that is the only unit the
    /// host can hand over without knowing this widget's font metrics; the
    /// conversion to columns is one `Span::width` here, and it is why a CJK
    /// parameter name lands in the right place.
    ///
    /// [`label`]: SignatureVm::label
    pub active: Option<(usize, usize)>,
    /// Documentation or hover prose, one row per line (§11: nothing wraps).
    pub prose: Vec<String>,
    /// The cell this hangs off — the cursor.
    pub anchor: Anchor,
    /// The widest this session has been, in body columns. Same contract and
    /// same reason as [`CompletionVm::width_floor`].
    pub width_floor: u16,
}

/// The body [`SignatureVm`] draws through: the signature line, then a rule,
/// then prose. Any part that is absent takes no rows.
#[derive(Debug, Clone, Copy)]
pub struct SignatureBody<'a> {
    vm: &'a SignatureVm,
}

impl<'a> SignatureBody<'a> {
    /// The body over one signature-help or hover answer.
    #[must_use]
    pub const fn new(vm: &'a SignatureVm) -> Self {
        Self { vm }
    }

    /// Whether the rule between the signature and the prose is drawn — only
    /// when there is something on both sides of it.
    const fn has_rule(&self) -> bool {
        self.vm.label.is_some() && !self.vm.prose.is_empty()
    }
}

impl FloatBody for SignatureBody<'_> {
    fn desired_height(&self, _width: u16) -> u16 {
        let label = u16::from(self.vm.label.is_some());
        let prose = u16::try_from(self.vm.prose.len())
            .unwrap_or(u16::MAX)
            .min(MAX_PROSE_ROWS);
        label
            .saturating_add(prose)
            .saturating_add(u16::from(self.has_rule()))
    }

    fn desired_width(&self) -> u16 {
        self.vm
            .label
            .iter()
            .chain(self.vm.prose.iter().take(MAX_PROSE_ROWS as usize))
            .map(|line| cells(line))
            .max()
            .unwrap_or(0)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, mood: Mood) {
        let ground = mood.body(theme);
        let mut canvas = Canvas { buf, rect: area };
        let mut y = area.y;

        if let Some(label) = &self.vm.label {
            let plain = Style::new().fg(theme.neutrals.text).bg(ground);
            canvas.fill_row(y, Style::new().bg(ground));
            canvas.text_elided(area.x, y, area.width, label, plain);
            // The active parameter, repainted in place. §4's "bright text" is
            // the emphasis this language has; the row is not re-laid out, so a
            // wide character in the parameter cannot shift the rest of the line.
            if let Some((start, end)) = self.vm.active {
                let before: String = label.chars().take(start).collect();
                let inside: String = label
                    .chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect();
                let x = area.x.saturating_add(cells(&before));
                if !inside.is_empty() && x < area.right() {
                    // Elided like the line it sits in: a signature truncated at
                    // the cap must not have its emphasis painted over the `⋯`
                    // that says the line goes on.
                    canvas.text_elided(
                        x,
                        y,
                        area.right() - x,
                        &inside,
                        Style::new().fg(theme.neutrals.bright_text).bg(ground),
                    );
                }
            }
            y += 1;
        }

        if self.has_rule() && y < area.bottom() {
            canvas.hrule(
                y,
                area.x,
                area.width,
                Style::new().fg(mood.rule(theme)).bg(ground),
                false,
            );
            y += 1;
        }

        let prose = Style::new().fg(theme.neutrals.meta).bg(ground);
        for line in &self.vm.prose {
            if y >= area.bottom() {
                break;
            }
            canvas.fill_row(y, Style::new().bg(ground));
            canvas.text_elided(area.x, y, area.width, line, prose);
            y += 1;
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
    /// `None` only from [`Float::passive`] — see [`Mood::Passive`].
    header: Option<FloatHeader<'a>>,
    /// `None` only from [`Float::passive`], §4's documented exception.
    footer: Option<FloatFooter<'a>>,
    body: &'a dyn FloatBody,
    /// `Some` exactly when the mood is [`Mood::Passive`], which is what makes
    /// [`Layout::Anchored`] unreachable from the other two constructors.
    anchor: Option<Anchor>,
    /// Body columns this float may not go under — [`Float::with_width_floor`].
    /// `0` from every constructor, which is content-sizing.
    width_floor: u16,
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
            header: Some(header),
            footer: Some(footer),
            body,
            anchor: None,
            width_floor: 0,
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
            header: Some(header),
            footer: Some(footer),
            body,
            anchor: None,
            width_floor: 0,
        }
    }

    /// `T038`'s passive float — `#2a3c2e` border, `#101410` body, **no header
    /// and no footer**, hung off `anchor` and sized to its content (`7c`).
    ///
    /// The completion list, signature help and hover all take this shape;
    /// [`CompletionList`] and [`SignatureBody`] are the bodies. §4's exception
    /// and §9's dim are both decided on [`Mood`], not here.
    #[must_use]
    pub const fn passive(body: &'a dyn FloatBody, anchor: Anchor) -> Self {
        Self {
            mood: Mood::Passive,
            header: None,
            footer: None,
            body,
            anchor: Some(anchor),
            width_floor: 0,
        }
    }

    /// **The anti-thrash knob** (`CP-4`): body columns this float may not go
    /// under, so an anchored float can grow to fit and never shrink under the
    /// cursor.
    ///
    /// `CP-4` reported the passive float as *"very dynamic — it will go from
    /// small to across the screen."* [`ANCHORED_WIDTH_PCT`] answers the second
    /// half; this answers the first. Vim recomputes its popup width on every
    /// redraw and jitters for exactly this reason (its `pumwidth` is a floor,
    /// not a memory); the editor whose suggest widget feels steady keeps one
    /// width for the life of the session. Monotonic is that behaviour with no
    /// setting to tune.
    ///
    /// **It is a parameter here and a field on the session**, which is the
    /// architecture rather than a preference: widgets own no state and a
    /// [`FloatBody`] is built per frame, so *"the widest this session has
    /// been"* has to live where the session does. It arrives as
    /// [`CompletionVm::width_floor`] / [`SignatureVm::width_floor`] and
    /// `crate::interpret` applies it at the one place a passive float is built.
    /// Unset it is `0`, which is content-sizing, so a host that never widens
    /// loses the steadiness and nothing else.
    ///
    /// It shipped with **no** non-test caller and a module doc that described
    /// the wiring in the present tense — `TEAM.md`'s *"built, tested, ticked and
    /// uncomposed"*, one layer below where `scripts/lint-action-arms.sh` can
    /// see. `float_is_held_to_the_widest_the_session_has_been` in
    /// `crate::interpret`'s tests is the composition half, so the next time it
    /// comes loose a test says so.
    ///
    /// **It never beats the cap**: [`Float::frame`] clamps after applying it,
    /// so a floor measured on a wide terminal cannot span a narrow one, and a
    /// session with nothing in it still draws nothing.
    #[must_use]
    pub const fn with_width_floor(mut self, body_cols: u16) -> Self {
        self.width_floor = body_cols;
        self
    }

    /// This float's mood.
    #[must_use]
    pub const fn mood(&self) -> Mood {
        self.mood
    }

    /// Which of §8/§11's shapes this float takes in an area `width` wide.
    #[must_use]
    pub const fn layout(&self, width: u16) -> Layout {
        match self.anchor {
            Some(anchor) => Layout::Anchored(anchor),
            None => Layout::for_width(width),
        }
    }

    /// Where this float lands inside `area` — public because click routing and
    /// the dim both need it, and because it is the testable half of §8's
    /// geometry rules.
    #[must_use]
    pub fn frame(&self, area: Rect) -> Rect {
        let layout = self.layout(area.width);
        // **`body_cols` and `width` come out of one match**, because they are
        // one decision. It used to be two, the first existing only to hoist the
        // anchored arm's measurement out for the emptiness guard forty lines
        // below — and a second `match` on the same scrutinee is a place for the
        // two to disagree.
        //
        // **Only the anchored arm measures**, which is
        // [`FloatBody::desired_width`]'s own contract: asking anyway would run
        // a `Picker`'s measurement over every row of a list for a number no
        // other layout reads. The other two answer `0`, and the guard below
        // reads that as *"this layout does not size to its content"* rather
        // than as *"empty"* — it only tests `body_cols` inside the anchored arm.
        let (body_cols, width) = match layout {
            Layout::FullWidth => (0, area.width),
            Layout::Centered => {
                let target = pct_of(area.width, WIDTH_PCT_MAX);
                let floor = pct_of(area.width, WIDTH_PCT_MIN);
                // The 4-column rule wins over the band when they disagree,
                // which they only can on a very narrow "centered" area.
                let capped = area.width.saturating_sub(2 * MIN_EDGE_GAP);
                (0, target.min(capped).max(floor.min(capped)))
            }
            // Content plus chrome, held above the session's floor and under
            // `CP-4`'s cap. §8's *band* is still not a floor here — the cap is
            // its floor value read as this float's ceiling, which is a
            // different claim and the module docs argue it.
            Layout::Anchored(_) => {
                let body_cols = self.body.desired_width();
                let width = body_cols
                    .max(self.width_floor)
                    .saturating_add(layout.chrome_cols())
                    .min(anchored_max_cols(area.width));
                (body_cols, width)
            }
        };

        let body_width = width.saturating_sub(layout.chrome_cols());
        let content = self.body.desired_height(body_width);
        let height = layout
            .chrome_rows(self.header.is_some())
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
            // **An anchored float with nothing in it is not a float, in either
            // axis.** The other two layouts collapse to their chrome, which is
            // right for a surface the user asked for and wrong for one that
            // appears while they type: a 6×2 empty box beside the cursor is an
            // artifact, not a message. A completion session with no items
            // closes; until it does, this draws nothing.
            //
            // **The column half of that rule was missing** and a property test
            // found it: a session whose every label is empty has content rows
            // and zero content *columns*, and drew a 6×3 bordered box with
            // nothing in it. `body_width == 0` is the same statement one axis
            // over, and it also covers an area too narrow for the chrome.
            //
            // **`body_cols`, not `body_width`, is the one that means "empty".**
            // A [`Float::with_width_floor`] wide enough makes `body_width`
            // positive for a session with nothing in it, and the box that rule
            // exists to prevent would be back — wider than before.
            Layout::Anchored(_) if content == 0 || body_cols == 0 || body_width == 0 => Rect {
                x: area.x,
                y: area.y,
                width: 0,
                height: 0,
            },
            Layout::Anchored(anchor) => {
                // Clamped, not centered: an anchored float slides along the
                // edge rather than jumping away from the word it belongs to.
                let x = anchor
                    .col
                    .min(area.right().saturating_sub(width))
                    .max(area.x);
                let below = anchor.row.saturating_add(1);
                // The last row the float can start on and still fit.
                let last = area.bottom().saturating_sub(height).max(area.y);
                let y = if below.saturating_add(height) <= area.bottom() {
                    below
                } else if anchor.row >= area.y.saturating_add(height) {
                    // No room under the cursor: flip above it, which is what
                    // every completion list does on the last rows of a screen.
                    anchor.row - height
                } else {
                    last
                };
                // **And then clamped, which is not belt and braces.** An anchor
                // is a number the host computed; one that is off the area
                // entirely — a stale cursor, a resize between compose and draw
                // — sends the "flip above" branch above or below the screen.
                // A property test found exactly that: row 32 in a two-row area
                // placed the frame at y=30. A float never draws outside its
                // area, whatever it was told.
                let y = y.clamp(area.y, last);
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
        let layout = self.layout(area.width);
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
            Layout::Centered | Layout::Anchored(_) => 1,
            Layout::FullWidth => 0,
        };
        let text_x = frame.x + inset + PAD_COLS;
        let text_w = frame.width.saturating_sub(2 * (inset + PAD_COLS));

        match layout {
            Layout::Centered | Layout::Anchored(_) => canvas.box_border(frame, border),
            // 8d: one mood-coloured rule on top, no sides, no bottom.
            Layout::FullWidth => canvas.hrule(frame.y, frame.x, frame.width, border, false),
        }

        // The header, and the rule under it, are one unit: no header, no rule,
        // and the body starts one row under the border. `7c`.
        let body_top = match self.header {
            Some(header) => {
                let header_y = frame.y + 1;
                canvas.header(text_x, header_y, text_w, header, self.mood, theme);
                let rule_y = frame.y + 2;
                canvas.hrule(rule_y, frame.x, frame.width, rule, inset == 1);
                rule_y + 1 + PAD_ROWS
            }
            None => frame.y + 1,
        };

        let footer_rows = if self.footer.is_some() { 2 } else { 0 };
        let pad_below = match layout {
            Layout::Centered | Layout::FullWidth => PAD_ROWS,
            Layout::Anchored(_) => 0,
        };
        let body_bottom = frame
            .bottom()
            .saturating_sub(inset + pad_below + footer_rows);
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
            // One row above the body's first is still legal — the rule replaces
            // the padding row a clamped float has already lost.
            if footer_rule_y + 1 >= body_top {
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
    /// **[`Mood::Passive`] does not dim** ([`Mood::dims`]): `7c` draws the code
    /// around the completion list at full strength, because you are still
    /// typing into it.
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
        if float.mood().dims() {
            buf.set_style(dim, Style::new().fg(theme.neutrals.dimmed_under_float));
        }
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

    /// [`Canvas::text`], but a row too long for its room ends in §2's `⋯`
    /// instead of stopping mid-word.
    ///
    /// **The cap makes this reachable, so it is not cosmetic.** Before `CP-4`
    /// an anchored float grew to whatever it held and a clipped row only
    /// happened on a terminal narrower than a completion; now every row of a
    /// server's answer meets a ceiling, and a `detail` that stops dead is
    /// indistinguishable from one the server sent short. The mark says *there
    /// is more*.
    ///
    /// **The prefix is measured by [`Buffer::set_stringn`] itself** rather than
    /// by counting characters here: it walks graphemes and stops before one
    /// would straddle the edge, so a CJK or emoji row loses a whole character
    /// and not half of one. Whatever cell that leaves short is filled before
    /// the mark goes down, so the mark is always in the last column and never
    /// on top of a continuation cell.
    fn text_elided(&mut self, x: u16, y: u16, max_width: u16, text: &str, style: Style) {
        if !self.rect.contains(Position::new(x, y)) || max_width == 0 {
            return;
        }
        let room = self.rect.right().saturating_sub(x).min(max_width);
        if cells(text) <= room {
            self.text(x, y, room, text, style);
            return;
        }
        let last = x + room - 1;
        let (next, _) = self.buf.set_stringn(x, y, text, (room - 1) as usize, style);
        for gap in next..last {
            self.put(gap, y, " ", style);
        }
        self.put(last, y, ELISION, style);
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

        let right_w = header.right.map_or(0, cells);
        let left_w = cells(header.left);

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
        Anchor, CompletionItemVm, CompletionList, CompletionVm, ELISION, Float, FloatBody,
        FloatFooter, FloatHeader, FloatSlot, FooterHint, Layout, MIN_EDGE_GAP, Mood, PAD_COLS,
        SignatureBody, SignatureVm, TextBody, WIDTH_PCT_MAX, WIDTH_PCT_MIN, anchored_wrap_cols,
        wrap_prose,
    };
    use crate::theme::Theme;
    use ratatui_core::buffer::{Buffer, Cell};
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Color;
    use ratatui_core::text::Span;

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

    // -- T038 / T039: the passive float ------------------------------------

    fn completion(items: &[(&str, Option<&str>)], selected: usize, docs: &[&str]) -> CompletionVm {
        CompletionVm {
            items: items
                .iter()
                .map(|(label, detail)| CompletionItemVm {
                    label: (*label).to_owned(),
                    detail: detail.map(str::to_owned),
                })
                .collect(),
            selected,
            documentation: docs.iter().map(|line| (*line).to_owned()).collect(),
            anchor: Anchor::new(0, 0),
            width_floor: 0,
        }
    }

    /// `7c`'s list, as `7c` draws it.
    fn screen_7c() -> CompletionVm {
        completion(
            &[
                ("default()", Some("fn() -> RetryPolicy")),
                ("default_delay", Some("Duration")),
                ("deserialize", Some("fn(D) -> Result<Self>")),
            ],
            0,
            &["Returns the policy with 3 attempts, 200ms base, 1s cap."],
        )
    }

    #[test]
    fn the_passive_float_is_7cs_border_with_no_header_and_no_footer() {
        let theme = Theme::phosphor_dark();
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(30, 6)), area());

        // §4: `#2a3c2e` passive.
        assert_eq!(buf[(frame.x, frame.y)].symbol(), "┌");
        assert_eq!(buf[(frame.x, frame.y)].fg, theme.float.passive);
        assert_eq!(buf[(frame.x + 3, frame.y + 1)].bg, theme.regions.selection);

        // Three items, a rule and a documentation row, between two borders.
        assert_eq!(frame.height, 2 + 3 + 1 + 1);
        assert!(row(&buf, frame, 1).contains("default()      fn() -> RetryPolicy"));
        assert!(row(&buf, frame, 2).contains("default_delay  Duration"));
        assert!(row(&buf, frame, 3).contains("deserialize    fn(D) -> Result<Self>"));
        assert!(row(&buf, frame, 4).contains('─'));
        assert!(row(&buf, frame, 5).contains("Returns the policy"));

        // No header row, and no footer row: every row between the borders is
        // the body's. §4's exception, plus `7c`'s missing header.
        let last = row(&buf, frame, frame.height - 1);
        assert!(last.starts_with('└'), "{last:?}");
        assert!(!last.contains("esc"), "{last:?}");
    }

    /// **`CP-4`'s defect, as an assertion about rows.** A real server answers
    /// with the whole scope — hundreds of rows — and a float sized to its
    /// content covered rows 0–28 of a 30-row terminal, hiding the code being
    /// typed into. The cap makes the list a window; [`CompletionList::scroll`]
    /// is what keeps the selection inside it.
    #[test]
    fn a_long_list_is_a_window_over_the_items_rather_than_the_whole_screen() {
        let labels: Vec<String> = (0..200).map(|n| format!("item_{n:03}")).collect();
        let items: Vec<(&str, Option<&str>)> =
            labels.iter().map(|label| (label.as_str(), None)).collect();
        let vm = completion(&items, 0, &[]);
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 100, 30);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(4, 2)), area);

        assert_eq!(
            frame.height,
            2 + super::MAX_ITEM_ROWS,
            "two borders and the cap, not two hundred rows"
        );
        assert!(
            frame.height < area.height / 2,
            "and the code underneath is still readable: {frame:?}"
        );

        // The 191st row is not on screen, and the first is.
        assert!(row(&buf, frame, 1).contains("item_000"));
        assert!(
            !(1..frame.height - 1).any(|dy| row(&buf, frame, dy).contains("item_190")),
            "the window shows the selection's end of the list"
        );
    }

    /// The selection stays inside the window, which is what makes the cap a
    /// scroll rather than a truncation — `<C-n>` past the tenth row would
    /// otherwise steer something invisible.
    #[test]
    fn the_selected_row_is_on_screen_however_far_down_the_list_it_is() {
        let labels: Vec<String> = (0..200).map(|n| format!("item_{n:03}")).collect();
        let items: Vec<(&str, Option<&str>)> =
            labels.iter().map(|label| (label.as_str(), None)).collect();
        let vm = completion(&items, 190, &[]);
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(
            Float::passive(&body, Anchor::new(4, 2)),
            Rect::new(0, 0, 100, 30),
        );
        assert!(
            (1..frame.height - 1).any(|dy| row(&buf, frame, dy).contains("item_190")),
            "the selected row is drawn"
        );
    }

    /// A server's documentation is markdown **source** — fences, `#` headings,
    /// the lot — and `7c` draws one line of prose. Fourteen rows of it is a
    /// float that is mostly not the completion list.
    #[test]
    fn a_doc_comment_the_length_of_a_readme_is_summarised_to_four_rows() {
        let prose: Vec<String> = (0..40).map(|n| format!("doc line {n}")).collect();
        let docs: Vec<&str> = prose.iter().map(String::as_str).collect();
        let vm = completion(&[("default()", Some("fn() -> RetryPolicy"))], 0, &docs);
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(
            Float::passive(&body, Anchor::new(4, 2)),
            Rect::new(0, 0, 100, 30),
        );
        assert_eq!(
            frame.height,
            2 + 1 + 1 + super::MAX_DOC_ROWS,
            "borders, one item, the rule, and the cap"
        );
        assert!(row(&buf, frame, 3).contains("doc line 0"));
        assert!(
            !(1..frame.height - 1).any(|dy| row(&buf, frame, dy).contains("doc line 9")),
            "and the tenth row is not drawn"
        );
    }

    /// Hover's whole answer is prose, so its cap is larger — and it is still a
    /// cap, because rust-analyzer's hover on a standard-library method is a
    /// page of markdown.
    #[test]
    fn hover_prose_stops_before_it_owns_the_screen() {
        let prose: Vec<String> = (0..40).map(|n| format!("hover line {n}")).collect();
        let vm = SignatureVm {
            label: None,
            active: None,
            prose,
            anchor: Anchor::new(4, 2),
            width_floor: 0,
        };
        let body = SignatureBody::new(&vm);
        let frame = Float::passive(&body, Anchor::new(4, 2)).frame(Rect::new(0, 0, 100, 30));
        assert_eq!(frame.height, 2 + super::MAX_PROSE_ROWS);
    }

    #[test]
    fn the_passive_float_hangs_off_its_anchor() {
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let anchor = Anchor::new(30, 6);
        let frame = Float::passive(&body, anchor).frame(area());
        assert_eq!(frame.x, 30, "left edge at the anchor column");
        assert_eq!(frame.y, 7, "the row under the anchor");
        // Sized to content, and the documentation row is the widest thing in
        // `7c` — 55 cells — not the longest completion. Plus two borders and
        // §8's two padding columns each side.
        assert_eq!(frame.width, 55 + 6);
    }

    // -- CP-4: the width band ---------------------------------------------

    /// **`CP-4`'s finding, as an assertion about columns.** Reported by hand:
    /// *"we need a max width for completion and hover and stuff — right now
    /// it's very dynamic and will go from small to across the screen."* One
    /// `detail` from a real server took the terminal; the cap is 60% of it, at
    /// both widths this repo takes frames at.
    #[test]
    fn one_long_detail_no_longer_takes_the_whole_screen() {
        let detail = "fn(&mut Formatter<'_>, RetryPolicy, Duration, &[String], &mut Client) \
                      -> Result<Vec<Result<Value, FetchError>>, FetchError>";
        let vm = completion(&[("default", Some(detail))], 0, &[]);
        let body = CompletionList::new(&vm);
        // Wider than either terminal on its own, which is what made this a
        // finding rather than a preference.
        assert!(body.desired_width() > 120, "{}", body.desired_width());

        for (width, cap) in [(120u16, 72u16), (80, 48)] {
            let area = Rect::new(0, 0, width, 20);
            let frame = Float::passive(&body, Anchor::new(2, 2)).frame(area);
            assert_eq!(super::anchored_max_cols(width), cap);
            assert_eq!(frame.width, cap, "at {width} columns");
            assert!(
                area.width - frame.width >= 2 * MIN_EDGE_GAP,
                "and the code you are typing into is still there: {frame:?}"
            );
        }
    }

    /// The same cap over the other passive body — hover, where the prose *is*
    /// the answer and there is no list above it to compete for the columns.
    ///
    /// **This is the backstop, not the plan.** The line here arrives unwrapped,
    /// which is what a host that has not run [`wrap_prose`] sends; the float
    /// truncates it with §2's mark rather than dropping it. What the shipping
    /// binary sends is wrapped — `Editing::wrapped`, and
    /// `hover_prose_is_wrapped_to_the_float_it_will_be_drawn_in` over in the
    /// host is the test for that half.
    #[test]
    fn a_hover_paragraph_is_capped_at_both_widths() {
        let vm = SignatureVm {
            label: None,
            active: None,
            prose: vec![
                "returns the policy this client retries with; attempts, base delay and \
                 cap all come from the config file and none of them is negotiable."
                    .to_owned(),
            ],
            anchor: Anchor::new(0, 0),
            width_floor: 0,
        };
        let body = SignatureBody::new(&vm);
        for width in [120u16, 80] {
            let area = Rect::new(0, 0, width, 20);
            let (buf, frame) = draw(Float::passive(&body, Anchor::new(3, 3)), area);
            assert_eq!(frame.width, super::anchored_max_cols(width));
            // Truncated, not vanished: §2's mark is in the last body column.
            let prose = bare(&buf, frame, 1);
            assert!(prose.starts_with("returns the policy"), "{prose:?}");
            assert!(prose.ends_with('⋯'), "{prose:?}");
        }
    }

    /// **At the width the design draws it, the cap redraws nothing.** The
    /// measurement is [`ANCHORED_WIDTH_PCT`]'s and is not restated here; this is
    /// it executed — `7c`'s list is still sized to its content at 120 columns,
    /// which is what `the_passive_float_hangs_off_its_anchor` asserts one test
    /// up and what the `screen_7c` golden frame commits.
    ///
    /// It says nothing about 80 columns, where the cap does bind. That case is
    /// `crates/phosphor/tests/screen_7c.rs`'s second frame and the deviation is
    /// recorded on the constant.
    #[test]
    fn the_cap_is_above_everything_7c_draws() {
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let frame = Float::passive(&body, Anchor::new(30, 6)).frame(area());
        assert_eq!(frame.width, 61);
        assert!(frame.width < super::anchored_max_cols(area().width));
        assert!(
            u32::from(frame.width) * 100 / u32::from(area().width)
                < u32::from(super::ANCHORED_WIDTH_PCT)
        );
    }

    /// **The thrash rule** (`CP-4`'s *"very dynamic"* half): an anchored float
    /// grows to fit and does not shrink under the cursor while the session
    /// lives.
    ///
    /// The first two frames are what the finding describes — one keystroke
    /// filters the list and the right edge jumps 30 columns left. The third is
    /// the same keystroke with the session's floor carried across, which is
    /// what [`Float::with_width_floor`] is for and who holds it.
    #[test]
    fn a_width_floor_grows_to_fit_and_never_shrinks() {
        let wide = completion(
            &[(
                "with_a_rather_long_name",
                Some("fn(D) -> Result<Self, Error>"),
            )],
            0,
            &[],
        );
        let narrow = completion(&[("with_a", None)], 0, &[]);
        let (wide_body, narrow_body) = (CompletionList::new(&wide), CompletionList::new(&narrow));
        let at = Anchor::new(4, 4);

        let grown = Float::passive(&wide_body, at).frame(area()).width;
        let shrunk = Float::passive(&narrow_body, at).frame(area()).width;
        assert!(
            grown > shrunk + 30,
            "the frames the finding describes: {grown} then {shrunk}"
        );

        let held = Float::passive(&narrow_body, at)
            .with_width_floor(wide_body.desired_width())
            .frame(area())
            .width;
        assert_eq!(held, grown, "the right edge does not move under the cursor");
    }

    /// A floor is a floor, not an override: it may not span the screen, and it
    /// may not put a bordered box beside the cursor with nothing in it — the
    /// two rules `CP-4` and the property tests already bought.
    #[test]
    fn a_width_floor_beats_neither_the_cap_nor_the_empty_session() {
        let vm = completion(&[("x", None)], 0, &[]);
        let body = CompletionList::new(&vm);
        for width in [120u16, 80] {
            let area = Rect::new(0, 0, width, 20);
            let frame = Float::passive(&body, Anchor::new(0, 0))
                .with_width_floor(u16::MAX)
                .frame(area);
            assert_eq!(frame.width, super::anchored_max_cols(width));
        }

        let empty = completion(&[], 0, &["orphan prose".repeat(4).as_str()]);
        let empty_body = CompletionList::new(&empty);
        let frame = Float::passive(&empty_body, Anchor::new(0, 0))
            .with_width_floor(40)
            .frame(area());
        assert_eq!(frame.width, 0, "a floor does not resurrect a dead session");
        assert_eq!(frame.height, 0);

        // **And the column half of that rule, which is the one a floor can
        // break.** A session whose every label is empty has rows and no
        // columns; sizing the float off the floor rather than off the content
        // would put a 46-column bordered box with nothing in it beside the
        // cursor — the artifact the zero-width rule exists to prevent, wider
        // than it was before the floor existed.
        let blank = completion(&[("", None), ("", None)], 0, &[]);
        let blank_body = CompletionList::new(&blank);
        assert_eq!(blank_body.desired_width(), 0);
        let frame = Float::passive(&blank_body, Anchor::new(0, 0))
            .with_width_floor(40)
            .frame(area());
        assert_eq!(frame.width, 0, "and a floor does not fill an empty one");
        assert_eq!(frame.height, 0);
    }

    /// **[`wrap_prose`] fits, and every row it produces survives the float.**
    ///
    /// Stated over the drawn buffer rather than over the returned strings,
    /// because the claim that matters is *"no `⋯` and nothing lost"* — a
    /// wrapper that agreed with the arithmetic and disagreed with the chrome
    /// would pass a string-length assertion and still elide on screen.
    #[test]
    fn wrapped_prose_fits_the_float_it_was_wrapped_for() {
        let source = "returns the policy this client retries with; attempts, base delay \
                      and cap all come from the config file and none of them is negotiable."
            .to_owned();
        for width in [80u16, 120, 200] {
            let wrapped = wrap_prose(core::slice::from_ref(&source), anchored_wrap_cols(width));
            assert!(wrapped.len() > 1, "at {width}: {wrapped:?}");
            let vm = SignatureVm {
                label: None,
                active: None,
                prose: wrapped.clone(),
                anchor: Anchor::new(0, 0),
                width_floor: 0,
            };
            let body = SignatureBody::new(&vm);
            let area = Rect::new(0, 0, width, 24);
            let (buf, frame) = draw(Float::passive(&body, Anchor::new(1, 1)), area);
            // Row 0 of the frame is the border; the body starts at 1.
            let drawn: Vec<String> = (1..=wrapped.len())
                .map(|row| bare(&buf, frame, u16::try_from(row).expect("a row")))
                .collect();
            for (row, text) in drawn.iter().enumerate() {
                assert!(
                    !text.ends_with(ELISION),
                    "at {width}, row {row} was elided: {text:?}"
                );
            }
            // And the words are all still there, in order.
            assert_eq!(
                drawn.join(" ").split_whitespace().collect::<Vec<_>>(),
                source.split_whitespace().collect::<Vec<_>>(),
                "at {width}"
            );
        }
    }

    /// The two shapes a greedy wrapper gets wrong: a token wider than the
    /// column, and a blank line.
    ///
    /// A long token gets a row of its own rather than being cut mid-way —
    /// breaking `Vec<HashMap<String, Vec<u8>>>` invents a word that is not in
    /// the text, where a truncated row's `⋯` says *"there is more of this"*. A
    /// blank line is a paragraph break the server put there and wrapping it away
    /// joins two paragraphs the prose keeps apart.
    #[test]
    fn wrap_prose_breaks_neither_a_long_token_nor_a_paragraph() {
        let long = "Vec<HashMap<String,Vec<u8>>>".to_owned();
        let wrapped = wrap_prose(
            &[format!("takes a {long} and"), String::new(), long.clone()],
            10,
        );
        assert_eq!(
            wrapped,
            vec![
                "takes a".to_owned(),
                long.clone(),
                "and".to_owned(),
                String::new(),
                long,
            ]
        );
        // A width of zero is a screen with no room for prose at all: hand the
        // lines back rather than loop looking for a column that is not there.
        let lines = vec!["a b c".to_owned()];
        assert_eq!(wrap_prose(&lines, 0), lines);
    }

    /// **A centered float does not panic on a wide terminal.**
    ///
    /// `area.width * WIDTH_PCT_MAX / 100` overflows `u16` at 820 columns — a
    /// debug-build panic on an ultrawide or a tall split, and one this file
    /// shipped. [`pct_of`] is the arithmetic that does not, and 819/820 is the
    /// exact step, so a rewrite that reintroduces the product is caught by the
    /// first of the two rather than by nothing.
    #[test]
    fn a_centered_float_survives_a_terminal_wider_than_u16_arithmetic() {
        let body = TextBody::new(&["one line"]);
        for width in [819u16, 820, 1092, 1093, u16::MAX] {
            let area = Rect::new(0, 0, width, 20);
            let frame = Float::informational(
                FloatHeader::new("❯ files"),
                &body,
                FloatFooter::new(&[FooterHint::bare("esc")]),
            )
            .frame(area);
            assert_eq!(
                frame.width,
                super::pct_of(width, WIDTH_PCT_MAX),
                "at {width} columns"
            );
            assert!(frame.right() <= area.right(), "{frame:?} at {width}");
        }
    }

    /// The number [`anchored_wrap_cols`] publishes is the number the float
    /// actually offers, so a host that wraps hover prose to it gets no `⋯` —
    /// §11's *"nothing ever wraps"* with the wrapping on the host's side of the
    /// seam, which is where [`SignatureVm::prose`] already puts it.
    ///
    /// One cell more is truncated, which is the other half of the claim: the
    /// published width is exact, not advisory.
    #[test]
    fn the_published_wrap_width_is_the_width_the_float_offers() {
        for width in [80u16, 120, 200] {
            let cols = anchored_wrap_cols(width);
            for (len, mark) in [(cols, false), (cols + 1, true)] {
                let vm = SignatureVm {
                    label: None,
                    active: None,
                    prose: vec!["x".repeat(len as usize)],
                    anchor: Anchor::new(0, 0),
                    width_floor: 0,
                };
                let body = SignatureBody::new(&vm);
                let area = Rect::new(0, 0, width, 10);
                let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area);
                assert_eq!(
                    row(&buf, frame, 1).contains('⋯'),
                    mark,
                    "{len} cells at {width} columns"
                );
            }
        }
    }

    /// **Truncation is measured in cells, not characters** — the confusion this
    /// repo has shipped three bugs from. A CJK label cut at the cap loses a
    /// whole character and the mark still lands in the last column, with no
    /// half-drawn glyph and no row a cell too long.
    #[test]
    fn truncation_counts_cells_and_never_splits_a_wide_grapheme() {
        // 8 cells in 4 characters, against 6 columns of body at 20 wide.
        let vm = completion(&[("名前名前", Some("Duration"))], 0, &[]);
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 20, 10);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area);
        assert_eq!(frame.width, 12);

        // One symbol per *cell*, so a wide grapheme's continuation cell reads
        // back as a blank — `名前` is `名 前`, which is what makes this a
        // measurement in cells rather than in characters.
        let drawn = row(&buf, frame, 1);
        assert_eq!(
            drawn.chars().count(),
            frame.width as usize,
            "the row is exactly the float wide: {drawn:?}"
        );
        assert!(drawn.starts_with("│  名 前"), "{drawn:?}");
        // The third `名` went whole rather than leaving half a glyph, and the
        // mark is in the last body column with §8's padding outside it.
        assert!(drawn.ends_with("⋯  │"), "{drawn:?}");
        assert_eq!(drawn.matches('名').count(), 1, "{drawn:?}");
        // The detail is off the row entirely, not sharing the six columns.
        assert!(!drawn.contains('D'), "{drawn:?}");
    }

    #[test]
    fn a_passive_float_with_no_room_below_flips_above_the_anchor() {
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 120, 12);
        let frame = Float::passive(&body, Anchor::new(4, 11)).frame(area);
        assert_eq!(frame.bottom(), 11, "sits on the rows above the cursor");
        assert!(frame.y < 11);
    }

    #[test]
    fn a_passive_float_slides_along_the_right_edge_rather_than_spilling() {
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 60, 30);
        let frame = Float::passive(&body, Anchor::new(58, 2)).frame(area);
        assert_eq!(frame.right(), 60);
        assert!(frame.x < 58);
        // §8's 4-column gap is a centered float's rule, not this one's.
        assert!(frame.width <= area.width);
    }

    #[test]
    fn a_passive_float_narrower_than_its_content_truncates_rather_than_wraps() {
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 20, 30);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 1)), area);
        // **12, not 20.** Before `CP-4` this float took the whole 20-column
        // area; the cap is 60% of it, so eight columns of code stay visible on
        // the narrowest screen there is.
        assert_eq!(frame.width, 12);
        assert_eq!(frame.width, super::anchored_max_cols(area.width));
        // §11: nothing ever wraps. One row per item, still.
        assert_eq!(frame.height, 2 + 3 + 1 + 1);
        // Six body columns: five of the label and §2's mark. The row says
        // *there is more* rather than stopping mid-word, and the detail column
        // is gone rather than sharing the six — §11's *"drop, never squeeze."*
        assert_eq!(bare(&buf, frame, 1), "defau⋯", "{:?}", row(&buf, frame, 1));
        assert!(!row(&buf, frame, 1).contains("fn()"));
    }

    #[test]
    fn an_empty_completion_list_draws_nothing_at_all() {
        let theme = Theme::phosphor_dark();
        let vm = completion(&[], 0, &[]);
        let body = CompletionList::new(&vm);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
        FloatSlot::with(Float::passive(&body, Anchor::new(10, 10))).render(
            area(),
            &mut buf,
            &theme,
        );
        let touched = (0..120)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .any(|(x, y)| buf[(x, y)].symbol() != " ");
        assert!(
            !touched,
            "an empty session is not a 6x2 box beside the cursor"
        );
    }

    #[test]
    fn one_item_is_one_row_between_two_borders() {
        let vm = completion(&[("x", None)], 0, &[]);
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(2, 2)), area());
        assert_eq!(frame.height, 3);
        assert_eq!(frame.width, 1 + 6);
        assert!(row(&buf, frame, 1).contains('x'));
    }

    #[test]
    fn a_list_longer_than_the_screen_keeps_the_selection_visible() {
        // The failure this prevents: `ctrl-n` past the bottom row steers a
        // selection nobody can see.
        let items: Vec<(String, Option<String>)> =
            (0..40).map(|i| (format!("item_{i}"), None)).collect();
        let borrowed: Vec<(&str, Option<&str>)> = items
            .iter()
            .map(|(label, _)| (label.as_str(), None))
            .collect();
        let area = Rect::new(0, 0, 60, 10);
        for selected in [0usize, 5, 39] {
            let vm = completion(&borrowed, selected, &[]);
            let body = CompletionList::new(&vm);
            let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area);
            assert!(frame.height <= area.height);
            let drawn = (0..frame.height)
                .map(|dy| row(&buf, frame, dy))
                .collect::<Vec<_>>()
                .join("\n");
            let want = format!("item_{selected}");
            // `item_1` is a prefix of `item_10`, so the match has to be exact.
            assert!(
                drawn
                    .lines()
                    .any(|line| line.split_whitespace().any(|word| word == want)),
                "selection {selected} is off screen:\n{drawn}"
            );
        }
    }

    #[test]
    fn a_wide_label_moves_the_detail_column_by_cells_not_by_chars() {
        // The `ß` class of bug, one surface over: a CJK identifier is two cells
        // per character and a detail column measured in `char`s would land
        // inside it.
        let vm = completion(
            &[("名前", Some("String")), ("x", Some("u8"))],
            0,
            &["日本語のドキュメント"],
        );
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area());
        // `名前` is two characters and **four cells**, so the detail column is
        // at 4 + 2, not at 2 + 2. Asserted by cell, because a row read back as
        // a string carries the continuation cell of every wide grapheme.
        let text_x = frame.x + 1 + PAD_COLS;
        assert_eq!(buf[(text_x, frame.y + 1)].symbol(), "名");
        assert_eq!(buf[(text_x + 6, frame.y + 1)].symbol(), "S");
        assert_eq!(buf[(text_x, frame.y + 2)].symbol(), "x");
        assert_eq!(buf[(text_x + 6, frame.y + 2)].symbol(), "u");
        // And the float is wide enough for the widest *cell* count, which is
        // the documentation row at 20 cells.
        assert_eq!(frame.width, 20 + 6);
    }

    #[test]
    fn the_selection_row_is_bright_text_on_the_selection_tint() {
        // §4: "selection row gets #26332a + bright text".
        let theme = Theme::phosphor_dark();
        let vm = completion(&[("a", None), ("b", None)], 1, &[]);
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area());
        let x = frame.x + 1 + PAD_COLS;
        assert_eq!(buf[(x, frame.y + 1)].fg, theme.neutrals.text);
        assert_eq!(buf[(x, frame.y + 1)].bg, theme.float.body);
        assert_eq!(buf[(x, frame.y + 2)].fg, theme.neutrals.bright_text);
        assert_eq!(buf[(x, frame.y + 2)].bg, theme.regions.selection);
    }

    #[test]
    fn a_selection_out_of_range_tints_nothing_and_still_draws_the_list() {
        // `CompletionVm::selected` calls an out-of-range selection legal — "a
        // session that has been filtered down" — and the float has to keep
        // showing what it is sized for. It scrolled past the end instead: a
        // float of blank rows, a rule and the documentation, beside the cursor.
        let theme = Theme::phosphor_dark();
        let vm = completion(
            &[("alpha", None), ("beta", None), ("gamma", None)],
            7,
            &["doc"],
        );
        let body = CompletionList::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 1)), area());
        assert_eq!(frame.height, 2 + 3 + 1 + 1);
        assert!(
            row(&buf, frame, 1).contains("alpha"),
            "{:?}",
            row(&buf, frame, 1)
        );
        assert!(row(&buf, frame, 2).contains("beta"));
        assert!(row(&buf, frame, 3).contains("gamma"));
        for dy in 1..=3 {
            assert_eq!(
                buf[(frame.x + 3, frame.y + dy)].bg,
                theme.float.body,
                "row {dy} is tinted"
            );
        }
    }

    #[test]
    fn a_list_with_nothing_to_say_is_not_a_float_either() {
        // The column half of "an anchored float with nothing in it is not a
        // float": items with empty labels have rows and no columns, and the
        // float used to be exactly `chrome_cols()` wide — a 6x3 bordered box
        // beside the cursor with nothing in it. Found by
        // `screen_7c::the_selection_is_always_on_screen`, which went red once.
        let theme = Theme::phosphor_dark();
        let vm = completion(&[("", None), ("", None)], 0, &[]);
        let body = CompletionList::new(&vm);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
        FloatSlot::with(Float::passive(&body, Anchor::new(4, 4))).render(area(), &mut buf, &theme);
        let touched = (0..120)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .any(|(x, y)| buf[(x, y)].symbol() != " ");
        assert!(!touched, "an empty-labelled session is not a 6x3 box");
    }

    #[test]
    fn a_documentation_rule_never_appears_without_a_row_under_it() {
        // One row left after the clamp is the rule alone — a separator
        // separating one thing from nothing. The items take the row instead.
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 60, 4);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area);
        assert_eq!(frame.height, 4);
        for dy in 1..3 {
            let row = row(&buf, frame, dy);
            assert!(!row.contains('─'), "row {dy} is a rule: {row:?}");
        }
        assert!(row(&buf, frame, 1).contains("default()"));
        assert!(row(&buf, frame, 2).contains("default_delay"));
    }

    #[test]
    fn the_rule_sits_between_a_signature_and_its_prose_and_only_there() {
        // `T039`'s float is "the signature line, then a rule, then prose", and
        // the rule is a row `desired_height` reserves: deleting it left every
        // other test in this crate green.
        let theme = Theme::phosphor_dark();
        let both = SignatureVm {
            label: Some("fn get(url: &str)".to_owned()),
            active: None,
            prose: vec!["one request".to_owned()],
            anchor: Anchor::new(0, 0),
            width_floor: 0,
        };
        let body = SignatureBody::new(&both);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area());
        assert_eq!(frame.height, 2 + 1 + 1 + 1, "label, rule, prose");
        assert!(row(&buf, frame, 1).contains("fn get(url: &str)"));
        let rule = row(&buf, frame, 2);
        assert!(rule.contains('─'), "{rule:?}");
        assert_eq!(
            buf[(frame.x + 3, frame.y + 2)].fg,
            Mood::Passive.rule(&theme)
        );
        assert!(row(&buf, frame, 3).contains("one request"));

        // And nowhere else: one side of it empty is one row shorter, with no
        // rule anywhere between the borders.
        for vm in [
            SignatureVm {
                prose: Vec::new(),
                ..both.clone()
            },
            SignatureVm {
                label: None,
                ..both.clone()
            },
        ] {
            let body = SignatureBody::new(&vm);
            let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area());
            assert_eq!(frame.height, 3);
            assert!(!row(&buf, frame, 1).contains('─'), "{vm:?}");
        }
    }

    #[test]
    fn the_documentation_gives_way_to_the_items_when_rows_run_out() {
        // A one-row area for the body: the selected completion wins, because a
        // list showing only prose is a list you cannot steer.
        let vm = screen_7c();
        let body = CompletionList::new(&vm);
        let area = Rect::new(0, 0, 120, 3);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area);
        assert_eq!(frame.height, 3);
        assert!(row(&buf, frame, 1).contains("default()"));
        assert!(!row(&buf, frame, 1).contains("Returns the policy"));
    }

    #[test]
    fn hover_is_a_signature_body_with_no_signature() {
        let theme = Theme::phosphor_dark();
        let vm = SignatureVm {
            label: None,
            active: None,
            prose: vec!["a retry policy".to_owned(), "3 attempts".to_owned()],
            anchor: Anchor::new(0, 0),
            width_floor: 0,
        };
        let body = SignatureBody::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(5, 5)), area());
        assert_eq!(frame.height, 2 + 2, "no signature row, no rule");
        assert!(row(&buf, frame, 1).contains("a retry policy"));
        assert_eq!(buf[(frame.x + 3, frame.y + 1)].fg, theme.neutrals.meta);
    }

    #[test]
    fn the_active_parameter_is_measured_in_cells_too() {
        let theme = Theme::phosphor_dark();
        let label = "fn 送る(あて: Addr, body: Body)";
        let start = label.chars().position(|c| c == 'b').expect("the parameter");
        let vm = SignatureVm {
            label: Some(label.to_owned()),
            active: Some((start, start + "body: Body".chars().count())),
            prose: Vec::new(),
            anchor: Anchor::new(0, 0),
            width_floor: 0,
        };
        let body = SignatureBody::new(&vm);
        let (buf, frame) = draw(Float::passive(&body, Anchor::new(0, 0)), area());
        assert_eq!(frame.height, 3, "one signature row, no prose, no rule");
        let text_x = frame.x + 1 + PAD_COLS;
        let before: String = label.chars().take(start).collect();
        let x = text_x + u16::try_from(Span::raw(&before).width()).expect("cells");
        assert_eq!(buf[(x, frame.y + 1)].symbol(), "b");
        assert_eq!(buf[(x, frame.y + 1)].fg, theme.neutrals.bright_text);
        // The character before it is still plain — the run is exactly the
        // parameter, not the rest of the line.
        assert_eq!(buf[(x - 1, frame.y + 1)].fg, theme.neutrals.text);
    }

    #[test]
    fn the_other_two_moods_dim_the_code_and_the_passive_one_does_not() {
        // §9, asserted where it is observable — over the same buffer, through
        // the slot, rather than as three assertions over a three-arm `const
        // fn` with no logic in it.
        let theme = Theme::phosphor_dark();
        let vm = screen_7c();
        let list = CompletionList::new(&vm);
        let cases = [
            (informational(), true),
            (needs_you(), true),
            (Float::passive(&list, Anchor::new(30, 6)), false),
        ];
        for (float, dims) in cases {
            let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
            for x in 0..120u16 {
                buf[(x, 0)].set_symbol("x").set_fg(theme.neutrals.text);
            }
            FloatSlot::with(float).render(area(), &mut buf, &theme);
            let dimmed = buf[(0, 0)].fg == theme.neutrals.dimmed_under_float;
            assert_eq!(dimmed, dims, "{:?} dims the code", float.mood());
        }
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
