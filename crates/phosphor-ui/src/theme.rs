//! The palette (`T010`).
//!
//! **This is the only file in `phosphor-ui` where a colour literal is legal.**
//! `scripts/lint-no-literal-colours.sh` exempts `theme.rs` (and any `theme/`
//! submodule) and nothing else, because Design Language §12 makes the palette
//! the single place colour lives: every widget takes `&Theme` and reads a named
//! field off it. A field missing here becomes an inlined literal in a widget,
//! and the lint rejects that — so completeness is the whole job.
//!
//! # Provenance
//!
//! Every value below is quoted from a design document, with the section that
//! carries it named on the line. Nothing here is approximated, and nothing is
//! invented — with **two exceptions**, both marked `DOC GAP` in place and both
//! flagged for Teej rather than folded in:
//!
//!   * `SyntaxMap::string` / `SyntaxMap::number` — no document assigns a hue to
//!     string or number literals; no mockup contains one. They are mapped onto
//!     the documented *transient* hue (the literal/constant family the mockups
//!     already use for `None` / `Ok` / `Err`) so that no new hex enters the tree.
//!   * `NeutralRamp::bright_text` — §4 and §5 both ask for "bright text" by name
//!     (selection rows, the active tab) but §1's neutral ramp gives it no hex.
//!     The value is read out of the mockups, where it is consistent.
//!
//! # Scope
//!
//! `T010` is the struct plus the §1 encoding, [`Theme::phosphor_dark`]. Loading
//! and actor-hue validation are `T011` ([`load`]); the light variant is `T012`;
//! Catppuccin and Tokyo Night are `T013` (Q7 — Ayu is out, its identity colour
//! is orange and the language reserves orange for attention). The six shipped
//! themes live in `crates/phosphor-ui/themes/*.theme` and are parsed by the same
//! loader a user's own file goes through ([`builtin`]) — there is no second code
//! path for the ones we wrote.

mod builtin;
mod load;

pub use builtin::{BUILTIN_SLUGS, builtin};
pub use load::{FAMILIES, HueFamily, MIN_CHROMA, ThemeError, ThemeErrorKind};

use std::borrow::Cow;

use phosphor_core::view::Tone;
use ratatui_core::style::Color;

/// A hex triple as the design docs write it, e.g. `rgb(0x3ddc97)`.
///
/// Keeping the docs' own notation in the source is deliberate: a reviewer can
/// diff this file against Design Language §1 by eye, which is how `T010`'s
/// acceptance is actually checked.
const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Which end of the lightness axis a theme sits on.
///
/// Not decoration: Design Language §10 makes lightness the theme's to own and
/// hue the contract's, so "light variants deepen, dark variants brighten" is a
/// rule `T011`'s validator applies *per variant*. It needs to be told which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Green-tinted near-black. The v1 default (§10).
    Dark,
    /// Warm paper with deepened hues (§10).
    Light,
}

/// One hue per actor — Design Language §1.
///
/// > "each color names exactly one actor or state, never decoration"
///
/// A theme owns lightness and syntax colours; it **never** owns actor identity
/// (§10). `T011` rejects at load — not warns — any theme that reassigns one of
/// these six hues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorPalette {
    /// `#3ddc97` — claude: his edits, his marks, his voice (§1).
    ///
    /// The load-bearing one. §1: "green always means claude; a green pixel with
    /// another meaning is a bug." §10: "Claude owns the brightest color on
    /// screen" — the property `CP-1` checks by eye in light mode.
    pub claude: Color,
    /// `#82aecd` — you: insert mode, your side of diffs, watches (§1).
    pub you: Color,
    /// `#e0a94e` — attention: waiting, paused, dirty, permission (§1).
    pub attention: Color,
    /// `#d97b6c` — trouble: deletions, failures, disconnects (§1).
    pub trouble: Color,
    /// `#cfa86a` — transient: visual mode, spinners, types (§1).
    pub transient: Color,
    /// `#9ec98c` — steel: repl, functions, scripting (§1).
    pub steel: Color,
}

/// The neutral ramp — Design Language §1, plus the one neutral §4/§5 name but
/// never give a hex to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeutralRamp {
    /// `#0c0f0c` — ground (§1). The editor background, and the inverted
    /// foreground of the statusline mode chip (§5).
    pub ground: Color,
    /// `#c6cec6` — text (§1). Default buffer foreground.
    pub text: Color,
    /// `#9aa39a` — prose (§1). Claude's sentences, as distinct from the facts
    /// he produced: §6, "His prose is `#9aa39a`; facts he produced (diffs,
    /// counts) are colored data, not prose."
    pub prose: Color,
    /// `#59635a` — meta (§1). Keyhints, counters, hunk headers, the `│`
    /// statusline separator, comments, fold summaries.
    pub meta: Color,
    /// `#414b42` — line numbers (§1). §3 pins this: "Column 2: line numbers,
    /// **always** `#414b42`." Also the `↪` soft-wrap continuation glyph and the
    /// `~` past-end-of-buffer rows (mockups `8e`, `1d`).
    pub line_numbers: Color,
    /// `#232823` — dimmed-under-float (§1). §9: "Dimming means 'behind.' Code
    /// under a float renders at `#232823`; panes never dim each other."
    pub dimmed_under_float: Color,
    /// `#e8f0e8` — **DOC GAP.** §4 ("selection row gets `#26332a` + bright
    /// text") and §5 ("active tab carries a 2px actor-colored top rule and
    /// bright text") both require a neutral brighter than [`text`], and §1's
    /// ramp does not list one. Read out of the mockups, where it is used
    /// consistently for exactly those two jobs (`8d`, `5c`, and §5's own
    /// tab-bar strip). Escalated rather than folded in.
    ///
    /// [`text`]: NeutralRamp::text
    pub bright_text: Color,
}

/// Row-level region treatment — Design Language §3.
///
/// > "Region highlights tint the whole row (`#141d16` anchor, `#26332a`
/// > selection-in-float, `#211114` failure)."
///
/// The two undercurl entries are the other half of §3's anchored-region
/// treatment ("tint + undercurl"). The vendored marks API is colour-only, so
/// `T085` adds the cell-style capability to the fork; the colours it draws with
/// are these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionTints {
    /// `#141d16` — anchored region, row tint (§3).
    pub anchor: Color,
    /// `#2a5c44` — anchored region, undercurl (§3 "tint + undercurl"; the hue
    /// is the informational mood, as drawn in mockups `9c` and `8c`).
    pub anchor_undercurl: Color,
    /// `#26332a` — selection-in-float row tint (§3). §4 pairs it with
    /// [`NeutralRamp::bright_text`].
    pub selection: Color,
    /// `#211114` — failure row tint (§3). Also the background of the
    /// insert-only trailing-whitespace mark (`8e`).
    pub failure: Color,
    /// `#d97b6c` — failure/diagnostic undercurl (§3; the trouble hue, as drawn
    /// in mockup `4c`).
    pub failure_undercurl: Color,
}

/// Float chrome — Design Language §4.
///
/// > "One border style; border color is the float's mood: `#2a5c44`
/// > informational, `#6b5426` needs-you (body `#171207`), `#2a3c2e` passive
/// > (completion — no footer, the exception)."
///
/// Consumed by `T084`, the one chrome primitive. Body backgrounds for the
/// informational and passive moods are not given a hex in §4 — only the
/// needs-you body is — so [`body`] is read out of the mockups, where every
/// informational and passive float uses the same value (`3c`, `3d`, `5c`).
///
/// [`body`]: FloatChrome::body
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatChrome {
    /// `#2a5c44` — informational border (§4).
    pub informational: Color,
    /// `#6b5426` — needs-you border (§4).
    pub needs_you: Color,
    /// `#171207` — needs-you body background (§4).
    pub needs_you_body: Color,
    /// `#3d3418` — the header/footer rule *inside* a needs-you float. §4 gives
    /// floats one border style; the internal rule follows the mood, and the
    /// amber-dark variant is drawn in mockups `7a` / `2d`.
    pub needs_you_rule: Color,
    /// `#2a3c2e` — passive border (§4). The completion float: no footer, the
    /// documented exception.
    pub passive: Color,
    /// `#101410` — body background for the informational and passive moods.
    /// See the type-level note; §4 hexes only the needs-you body.
    pub body: Color,
}

/// The three strips of chrome — Design Language §5.
///
/// > "Three strips of chrome, ever: tab bar (top, appears only with 2+ panes),
/// > statusline (bottom, always), and tmux below it, untouched."
///
/// tmux gets no field: §5 says untouched, so it is not ours to colour.
/// The mode chip's background is an *actor* colour chosen per mode
/// (`NORMAL` → claude, `INSERT` → you, `VISUAL` → transient, `PAUSED` →
/// attention), which is `T017`'s mapping over [`ActorPalette`], not a colour of
/// its own. Its foreground is [`mode_chip_fg`].
///
/// [`mode_chip_fg`]: Chrome::mode_chip_fg
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    /// `#1a201a` — statusline background (§5), and the active tab's background
    /// in the tab bar.
    pub statusline: Color,
    /// `#0c0f0c` — mode-chip foreground: ground on an actor-coloured field.
    /// §5 calls the chip "the only inverted text on screen."
    pub mode_chip_fg: Color,
    /// `#0c0f0c` — tab-bar background (§5); the tab bar sits on ground.
    pub tab_bar: Color,
    /// `#1d241d` — the rule under the tab bar (§5).
    pub tab_bar_rule: Color,
    /// `#242a24` — in-surface divider: the pane split, and a float's
    /// header/body and body/footer rules (§4's anatomy, mockups `4b`, `8d`).
    pub divider: Color,
}

/// Syntax highlighting — the base16-style map (`T010`, Component Breakdown's
/// `Theme` spec).
///
/// Design Language §1 covers three of these outright, because the actor palette
/// *is* partly a syntax map: transient is "types", steel is "functions".
/// `keyword` is not named in §1 but is drawn in the you-blue in every mockup
/// that shows code (`1a`, `9c`, `8d`, `8e`, `4c`, `1d`) — read out, not chosen.
///
/// A theme owns these (§10: "A theme owns lightness and syntax colors; it never
/// owns actor identity"), which is why they are a separate struct from
/// [`ActorPalette`]: `T011`'s validator locks the actors and leaves this free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxMap {
    /// Default foreground for anything unclassified — `#c6cec6`, the neutral
    /// text value. Identifiers, operators, punctuation and attributes all
    /// render plain in the mockups.
    pub text: Color,
    /// `#82aecd` — keywords (`let`, `mut`, `for`, `in`, `match`, `pub`, `fn`,
    /// `impl`, `return`, `await`, `mod`, `use`). Mockups `1a` / `9c` / `8e`.
    pub keyword: Color,
    /// `#cfa86a` — types. §1, verbatim: "transient — visual mode, spinners,
    /// types".
    pub ty: Color,
    /// `#9ec98c` — functions. §1, verbatim: "steel — repl, functions,
    /// scripting". Mockup `1a` renders `retry_with_backoff` in it.
    pub function: Color,
    /// `#cfa86a` — constants and enum constructors (`None`, `Ok`, `Err`),
    /// drawn in the transient hue in `1a` / `9c` / `8d`.
    pub constant: Color,
    /// `#cfa86a` — **DOC GAP.** No design document and no mockup contains a
    /// string literal, so no hue is specified. Mapped onto the documented
    /// transient hue with the rest of the literal family rather than inventing
    /// a value; escalated for Teej.
    pub string: Color,
    /// `#cfa86a` — **DOC GAP.** Same as [`string`]: numeric literals appear
    /// only as bare `0` and `2` in the mockups, rendered plain, which does not
    /// settle whether numbers have a hue. Mapped to transient; escalated.
    ///
    /// [`string`]: SyntaxMap::string
    pub number: Color,
    /// `#59635a` — comments, in the meta neutral. Mockup `8e`'s doc comment and
    /// its soft-wrap continuation.
    pub comment: Color,
}

/// The palette. Every widget in `phosphor-ui` takes `&Theme` (Design Language
/// §12) and reads a named field; no widget constructs a colour.
///
/// The grouping mirrors the design document's own sections so the two can be
/// diffed by eye: [`actors`] and [`neutrals`] are §1, [`regions`] is §3,
/// [`float`] is §4, [`chrome`] is §5, and [`syntax`] is the base16-style map
/// the Component Breakdown's `Theme` spec asks for.
///
/// [`actors`]: Theme::actors
/// [`neutrals`]: Theme::neutrals
/// [`regions`]: Theme::regions
/// [`float`]: Theme::float
/// [`chrome`]: Theme::chrome
/// [`syntax`]: Theme::syntax
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Theme name as a user names it — `"phosphor"`, `"catppuccin"`. The
    /// flavour is [`variant`], so Mocha and Latte are both `"catppuccin"`; the
    /// pair is addressed by slug (`catppuccin-mocha`) in [`builtin`].
    ///
    /// [`Cow`] rather than `&'static str` because `T011` loads themes from
    /// disk, where the name is not known at compile time. Built-ins stay
    /// `Cow::Borrowed`, so [`Theme::phosphor_dark`] is still a `const fn`.
    ///
    /// [`variant`]: Theme::variant
    pub name: Cow<'static, str>,
    /// Which end of the lightness axis this variant sits on (§10).
    pub variant: Variant,
    /// §1 — one hue per actor. Locked by `T011`'s validator.
    pub actors: ActorPalette,
    /// §1 — the neutral ramp.
    pub neutrals: NeutralRamp,
    /// §3 — row tints and undercurl.
    pub regions: RegionTints,
    /// §4 — float mood borders and bodies.
    pub float: FloatChrome,
    /// §5 — statusline and tab bar.
    pub chrome: Chrome,
    /// The base16-style syntax map. Theme-owned (§10).
    pub syntax: SyntaxMap,
}

impl Theme {
    /// A [`Tone`] resolved against this theme.
    ///
    /// **The only route from the protocol to a colour.** There are no RGB
    /// values in the view tree and there can never be one
    /// (`view/props.rs`, `scripts/lint-no-literal-colours.sh`), so every widget
    /// that draws a toned run comes through here.
    ///
    /// It lives on the theme rather than on the interpreter because a *second*
    /// widget needed it — `T045`'s picker draws rows of toned runs — and a
    /// private copy in each would be the shape of drift this file exists to
    /// prevent. `crate::interpret` delegates to it.
    #[must_use]
    pub const fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Claude => self.actors.claude,
            Tone::You => self.actors.you,
            Tone::Attention => self.actors.attention,
            Tone::Trouble => self.actors.trouble,
            Tone::Transient => self.actors.transient,
            Tone::Steel => self.actors.steel,
            Tone::Text => self.neutrals.text,
            Tone::Prose => self.neutrals.prose,
            Tone::Meta => self.neutrals.meta,
            Tone::LineNumber => self.neutrals.line_numbers,
            Tone::Ground => self.neutrals.ground,
            Tone::BrightText => self.neutrals.bright_text,
            Tone::Dimmed => self.neutrals.dimmed_under_float,
        }
    }

    /// Phosphor dark — Design Language §1 encoded verbatim.
    ///
    /// §10 makes this the v1 default: "green-tinted near-black with phosphor
    /// green for claude." It is also the *reference* instance for `T010`'s
    /// acceptance — the test at the bottom of this file walks §1's twelve
    /// values and asserts each one has a home here.
    ///
    /// The light variant is [`Theme::phosphor_light`] (`T012`), which deepens
    /// these hues — claude-green goes to `#1a9a62` (§10, mockup `8c`).
    ///
    /// This `const fn` and `themes/phosphor-dark.theme` are two encodings of
    /// the same palette, and `builtin::tests` asserts they agree field for
    /// field. Edit one and the other fails the build.
    #[must_use]
    pub const fn phosphor_dark() -> Self {
        Self {
            name: Cow::Borrowed("phosphor"),
            variant: Variant::Dark,
            actors: ActorPalette {
                claude: rgb(0x3ddc97),
                you: rgb(0x82aecd),
                attention: rgb(0xe0a94e),
                trouble: rgb(0xd97b6c),
                transient: rgb(0xcfa86a),
                steel: rgb(0x9ec98c),
            },
            neutrals: NeutralRamp {
                ground: rgb(0x0c0f0c),
                text: rgb(0xc6cec6),
                prose: rgb(0x9aa39a),
                meta: rgb(0x59635a),
                line_numbers: rgb(0x414b42),
                dimmed_under_float: rgb(0x232823),
                bright_text: rgb(0xe8f0e8),
            },
            regions: RegionTints {
                anchor: rgb(0x141d16),
                anchor_undercurl: rgb(0x2a5c44),
                selection: rgb(0x26332a),
                failure: rgb(0x211114),
                failure_undercurl: rgb(0xd97b6c),
            },
            float: FloatChrome {
                informational: rgb(0x2a5c44),
                needs_you: rgb(0x6b5426),
                needs_you_body: rgb(0x171207),
                needs_you_rule: rgb(0x3d3418),
                passive: rgb(0x2a3c2e),
                body: rgb(0x101410),
            },
            chrome: Chrome {
                statusline: rgb(0x1a201a),
                mode_chip_fg: rgb(0x0c0f0c),
                tab_bar: rgb(0x0c0f0c),
                tab_bar_rule: rgb(0x1d241d),
                divider: rgb(0x242a24),
            },
            syntax: SyntaxMap {
                text: rgb(0xc6cec6),
                keyword: rgb(0x82aecd),
                ty: rgb(0xcfa86a),
                function: rgb(0x9ec98c),
                constant: rgb(0xcfa86a),
                string: rgb(0xcfa86a),
                number: rgb(0xcfa86a),
                comment: rgb(0x59635a),
            },
        }
    }
}

impl Default for Theme {
    /// Phosphor dark. §10: "Phosphor (dark + light) ships as the v1 default".
    fn default() -> Self {
        Self::phosphor_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, rgb};
    use ratatui_core::style::Color;

    /// Design Language §1, transcribed: the twelve values the section lists,
    /// each with the field it must live in.
    ///
    /// This is `T010`'s acceptance criterion — "every colour in the language has
    /// a named field" — turned into something that fails a build instead of
    /// needing a careful reader. Six actors, six neutrals, twelve distinct
    /// values, no duplicates within the section.
    const SECTION_1: &[(&str, u32)] = &[
        // "1 · Color — one hue per actor"
        ("actors.claude", 0x3ddc97),
        ("actors.you", 0x82aecd),
        ("actors.attention", 0xe0a94e),
        ("actors.trouble", 0xd97b6c),
        ("actors.transient", 0xcfa86a),
        ("actors.steel", 0x9ec98c),
        // "Neutrals: … ground, … text, … prose, … meta, … line nums,
        //  … dimmed-under-float."
        ("neutrals.ground", 0x0c0f0c),
        ("neutrals.text", 0xc6cec6),
        ("neutrals.prose", 0x9aa39a),
        ("neutrals.meta", 0x59635a),
        ("neutrals.line_numbers", 0x414b42),
        ("neutrals.dimmed_under_float", 0x232823),
    ];

    fn field(theme: &Theme, path: &str) -> Color {
        match path {
            "actors.claude" => theme.actors.claude,
            "actors.you" => theme.actors.you,
            "actors.attention" => theme.actors.attention,
            "actors.trouble" => theme.actors.trouble,
            "actors.transient" => theme.actors.transient,
            "actors.steel" => theme.actors.steel,
            "neutrals.ground" => theme.neutrals.ground,
            "neutrals.text" => theme.neutrals.text,
            "neutrals.prose" => theme.neutrals.prose,
            "neutrals.meta" => theme.neutrals.meta,
            "neutrals.line_numbers" => theme.neutrals.line_numbers,
            "neutrals.dimmed_under_float" => theme.neutrals.dimmed_under_float,
            other => panic!("no accessor for {other}"),
        }
    }

    #[test]
    fn every_section_1_colour_has_a_named_field() {
        let theme = Theme::phosphor_dark();
        for (path, hex) in SECTION_1 {
            assert_eq!(
                field(&theme, path),
                rgb(*hex),
                "Design Language §1 value #{hex:06x} is not what `{path}` holds"
            );
        }
    }

    #[test]
    fn section_1_is_twelve_distinct_values() {
        // Guards the transcription itself: if a future edit drops or duplicates
        // a row of §1, the count stops matching the document.
        let mut values: Vec<u32> = SECTION_1.iter().map(|(_, hex)| *hex).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), 12, "§1 lists 6 actor hues and 6 neutrals");
    }

    #[test]
    fn rgb_splits_the_hex_the_way_the_docs_write_it() {
        assert_eq!(rgb(0x3ddc97), Color::Rgb(0x3d, 0xdc, 0x97));
        assert_eq!(rgb(0x0c0f0c), Color::Rgb(0x0c, 0x0f, 0x0c));
    }

    #[test]
    fn claude_is_green_and_nothing_else_is_that_green() {
        // §1: "green always means claude; a green pixel with another meaning is
        // a bug." The full check is `T011`'s actor-hue validation; this is the
        // cheap structural half — claude's exact value is claude's alone.
        let t = Theme::phosphor_dark();
        let claude = t.actors.claude;
        for other in [
            t.actors.you,
            t.actors.attention,
            t.actors.trouble,
            t.actors.transient,
            t.actors.steel,
        ] {
            assert_ne!(other, claude, "an actor hue collides with claude-green");
        }
    }

    #[test]
    fn the_default_theme_is_phosphor_dark() {
        // §10: phosphor dark is the v1 default.
        assert_eq!(Theme::default(), Theme::phosphor_dark());
    }
}
