//! Base16-style theme loading and actor-hue validation (`T011`).
//!
//! # The format
//!
//! One `key: value` per line; values are `#rrggbb` (the leading `#` is
//! optional, base16 files usually omit it). A line whose first non-blank
//! character is `#` is a comment. **There are no trailing comments** — a
//! value is the rest of its line, trimmed — because a hex triple starts with
//! the comment character and a rule that has to guess is a rule that will
//! guess wrong. Unknown keys, duplicate keys and missing keys are all errors;
//! a theme is a complete palette or it is not a theme.
//!
//! # The rule this file exists for
//!
//! > Design Language §10: *"A theme owns lightness and syntax colors; it never
//! > owns actor identity — claude stays green, attention stays warm, trouble
//! > stays red in every mapping. […] A theme where green means anything else is
//! > rejected, not themed."*
//!
//! Rejected at load, not accepted-and-warned. The actor palette is how you tell
//! at a glance who did what, so a theme that recolours `claude` is not a
//! preference — it is a broken theme, and it fails the same way a syntax error
//! does.
//!
//! ## Where the boundary is drawn, and why there
//!
//! "Reassigns an actor hue" has to be mechanical enough that **Catppuccin and
//! Tokyo Night pass while a red `claude` fails**, so it is three checks, in
//! this order:
//!
//! 1. **Chroma floor** ([`MIN_CHROMA`]). A near-grey has no hue to check —
//!    `#808080` reports hue 0° and would sail through a red-family test for the
//!    wrong reason. Below the floor the colour is rejected as achromatic, with
//!    its own message.
//! 2. **Hue family.** Each actor is locked to one arc of the hue circle.
//!    *Only the angle is the contract*; saturation and lightness are the
//!    theme's, which is exactly what "light variants deepen, dark variants
//!    brighten — same hue, contrast-corrected" means (§10). The arcs are in
//!    [`FAMILIES`], each with the shipped values that pin its ends.
//! 3. **Pairwise distinctness.** Six actors, six *different* colours. Exact RGB
//!    equality only — no perceptual-distance threshold, because that needs a
//!    tolerance nobody can defend and would start rejecting real palettes. This
//!    catches the copy-paste failure (`steel` left equal to `claude`), which is
//!    the one that actually destroys the at-a-glance read.
//!
//! Everything else in a theme file — neutrals, region tints, float chrome,
//! the whole syntax map — is unvalidated. §10 gives those to the theme.
//!
//! **Why families rather than a tolerance around the phosphor value.** A
//! tolerance is the obvious alternative and it is *looser*: admitting Tokyo
//! Night's `#9ece6a` (89°) against phosphor's `#3ddc97` (154°) needs ±65°, a
//! 130°-wide window — wider than the green arc below. The arc is the tighter
//! rule, and unlike a tolerance it has a name a rejection message can say out
//! loud.
//!
//! **Flagged, not folded in:** [Q7](../../../../docs/IMPLEMENTATION-PLAN.md)
//! lists "relaxing validation to hue *families*" among its rejected
//! alternatives. That alternative was a different thing — relaxing the rule so
//! Ayu could slide *attention* within warm to dodge a collision with its own
//! syntax orange, which needs an actor-vs-syntax distinguishability test. This
//! validates actor colours only, never compares them to syntax, and adds no
//! such test. Some banding is unavoidable (Catppuccin's claude is 39° off
//! phosphor's and ships in a mockup), but the overlap with Q7's wording is
//! real and is reported for Teej rather than decided here.

use std::borrow::Cow;
use std::fmt;

use ratatui_core::style::Color;

use super::{
    ActorPalette, Chrome, FloatChrome, NeutralRamp, RegionTints, SyntaxMap, Theme, Variant,
};

/// Minimum chroma — `(max - min) / 255` over the RGB channels — for an actor
/// colour to have a hue worth testing.
///
/// The lowest chroma among the four shipped mappings is `0.235`
/// (`#587539`, tokyo-night-day claude); phosphor's own `steel` is `0.239`.
/// `0.12` clears both by better than 2× and still rejects anything a viewer
/// would read as grey.
pub const MIN_CHROMA: f32 = 0.12;

/// One arc of the hue circle, named.
///
/// `lo` is inclusive, `hi` exclusive, degrees; `lo > hi` wraps through 0°
/// (which is how `red` is expressed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HueFamily {
    /// What a rejection message calls this family.
    pub label: &'static str,
    /// Inclusive lower bound, degrees.
    pub lo: f32,
    /// Exclusive upper bound, degrees.
    pub hi: f32,
}

impl HueFamily {
    /// Is `hue` (degrees, `0.0..360.0`) inside this arc?
    #[must_use]
    pub fn contains(&self, hue: f32) -> bool {
        if self.lo < self.hi {
            hue >= self.lo && hue < self.hi
        } else {
            hue >= self.lo || hue < self.hi
        }
    }
}

impl fmt::Display for HueFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (hue {:.0}°–{:.0}°)", self.label, self.lo, self.hi)
    }
}

/// The six locked arcs, with the shipped values that pin each end.
///
/// Two actors share `amber` and two share the green band on purpose: phosphor's
/// own `attention` (37.4°) and `transient` (36.8°) are the *same* hue, differing
/// only in saturation and lightness, so a rule that demanded six mutually
/// distant hues would reject the reference palette. `claude` gets the narrower
/// green because nothing forces it wider; `steel` gets green-teal because
/// Catppuccin Latte's teal — the only second green that flavour has — sits at
/// 183°.
///
/// | actor | arc | tightest shipped value on each end |
/// |---|---|---|
/// | `claude` | green 70–175 | `#9ece6a` tokyo night 88.8° · `#3ddc97` phosphor 154.0° |
/// | `you` | blue 195–260 | `#82aecd` phosphor 204.8° · `#7aa2f7` tokyo night 220.8° |
/// | `attention` | amber 18–65 | `#8c6c3e` tokyo night day 35.4° · `#f9e2af` mocha 41.4° |
/// | `trouble` | red 335–18 | `#f52a65` tokyo night day 342.6° · `#d97b6c` phosphor 8.3° |
/// | `transient` | amber 18–65 | `#fe640b` latte 22.0° · `#e0a94e` phosphor 37.4° |
/// | `steel` | green-teal 70–195 | `#9ec98c` phosphor 102.3° · `#179299` latte 183.2° |
///
/// Every arc clears its tightest shipped value by at least 8°, and no arc
/// contains another actor's family except where two actors share one.
pub const FAMILIES: [(&str, HueFamily); 6] = [
    (
        "claude",
        HueFamily {
            label: "green",
            lo: 70.0,
            hi: 175.0,
        },
    ),
    (
        "you",
        HueFamily {
            label: "blue",
            lo: 195.0,
            hi: 260.0,
        },
    ),
    (
        "attention",
        HueFamily {
            label: "amber",
            lo: 18.0,
            hi: 65.0,
        },
    ),
    (
        "trouble",
        HueFamily {
            label: "red",
            lo: 335.0,
            hi: 18.0,
        },
    ),
    (
        "transient",
        HueFamily {
            label: "amber",
            lo: 18.0,
            hi: 65.0,
        },
    ),
    (
        "steel",
        HueFamily {
            label: "green-teal",
            lo: 70.0,
            hi: 195.0,
        },
    ),
];

/// Reference values a rejection message quotes back, so the reader can see what
/// "green" is supposed to look like rather than only what it is not.
const REFERENCES: [(&str, [(&str, &str); 3]); 6] = [
    (
        "claude",
        [
            ("#3ddc97", "phosphor dark"),
            ("#a6e3a1", "catppuccin mocha"),
            ("#9ece6a", "tokyo night"),
        ],
    ),
    (
        "you",
        [
            ("#82aecd", "phosphor dark"),
            ("#89b4fa", "catppuccin mocha"),
            ("#7aa2f7", "tokyo night"),
        ],
    ),
    (
        "attention",
        [
            ("#e0a94e", "phosphor dark"),
            ("#f9e2af", "catppuccin mocha"),
            ("#e0af68", "tokyo night"),
        ],
    ),
    (
        "trouble",
        [
            ("#d97b6c", "phosphor dark"),
            ("#f38ba8", "catppuccin mocha"),
            ("#f7768e", "tokyo night"),
        ],
    ),
    (
        "transient",
        [
            ("#cfa86a", "phosphor dark"),
            ("#fab387", "catppuccin mocha"),
            ("#ff9e64", "tokyo night"),
        ],
    ),
    (
        "steel",
        [
            ("#9ec98c", "phosphor dark"),
            ("#94e2d5", "catppuccin mocha"),
            ("#73daca", "tokyo night"),
        ],
    ),
];

/// An 8-bit RGB triple, as a theme file writes it.
type Rgb = (u8, u8, u8);

/// A parsed slot: its value, and the line it came from (for error messages).
type Slotted = Option<(Rgb, usize)>;

// ── the key table ────────────────────────────────────────────────────────────

macro_rules! slots {
    ($($variant:ident => $key:literal),* $(,)?) => {
        /// One colour-carrying key in a theme file.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Slot { $($variant),* }

        /// Every colour key, in file order. Also the "you are missing these"
        /// list — a theme must set all of them.
        const SLOT_KEYS: &[&str] = &[$($key),*];

        impl Slot {
            fn from_key(key: &str) -> Option<Self> {
                match key { $($key => Some(Self::$variant),)* _ => None }
            }
        }
    };
}

slots! {
    Claude => "actor.claude",
    You => "actor.you",
    Attention => "actor.attention",
    Trouble => "actor.trouble",
    Transient => "actor.transient",
    Steel => "actor.steel",

    Ground => "neutral.ground",
    Text => "neutral.text",
    Prose => "neutral.prose",
    Meta => "neutral.meta",
    LineNumbers => "neutral.line_numbers",
    DimmedUnderFloat => "neutral.dimmed_under_float",
    BrightText => "neutral.bright_text",

    RegionAnchor => "region.anchor",
    RegionAnchorUndercurl => "region.anchor_undercurl",
    RegionSelection => "region.selection",
    RegionFailure => "region.failure",
    RegionFailureUndercurl => "region.failure_undercurl",

    FloatInformational => "float.informational",
    FloatNeedsYou => "float.needs_you",
    FloatNeedsYouBody => "float.needs_you_body",
    FloatNeedsYouRule => "float.needs_you_rule",
    FloatPassive => "float.passive",
    FloatBody => "float.body",

    ChromeStatusline => "chrome.statusline",
    ChromeModeChipFg => "chrome.mode_chip_fg",
    ChromeTabBar => "chrome.tab_bar",
    ChromeTabBarRule => "chrome.tab_bar_rule",
    ChromeDivider => "chrome.divider",

    SyntaxText => "syntax.text",
    SyntaxKeyword => "syntax.keyword",
    SyntaxType => "syntax.type",
    SyntaxFunction => "syntax.function",
    SyntaxConstant => "syntax.constant",
    SyntaxString => "syntax.string",
    SyntaxNumber => "syntax.number",
    SyntaxComment => "syntax.comment",
}

/// The six actor slots, paired with their entry in [`FAMILIES`] by position.
const ACTOR_SLOTS: [Slot; 6] = [
    Slot::Claude,
    Slot::You,
    Slot::Attention,
    Slot::Trouble,
    Slot::Transient,
    Slot::Steel,
];

// ── errors ───────────────────────────────────────────────────────────────────

/// Why a theme file was rejected.
///
/// Every variant carries enough to print a line the reader can act on without
/// opening the design docs; see the [`fmt::Display`] impl.
///
/// Not `Eq`: two variants carry a measured `f32` (hue, chroma).
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeErrorKind {
    /// A non-comment, non-blank line with no `:`.
    NotAKeyValue { text: String },
    /// A key the loader does not know.
    UnknownKey { key: String },
    /// A key set twice.
    DuplicateKey { key: String, first_line: usize },
    /// A value that is not `#rrggbb`.
    BadColour { key: String, value: String },
    /// `variant:` was neither `dark` nor `light`.
    BadVariant { value: String },
    /// `name:` was empty.
    EmptyName,
    /// Keys the file never set.
    MissingKeys { keys: Vec<&'static str> },
    /// An actor colour sits outside its locked arc — the `T011` rejection.
    ActorHue {
        actor: &'static str,
        family: HueFamily,
        value: String,
        hue: f32,
        looks_like: &'static str,
    },
    /// An actor colour has no usable hue.
    ActorAchromatic {
        actor: &'static str,
        value: String,
        chroma: f32,
    },
    /// Two actors were given the same colour.
    ActorCollision {
        actor: &'static str,
        other: &'static str,
        value: String,
    },
}

/// A theme file that could not be loaded, with where in it the problem is.
///
/// [`kind`] is boxed so that `Result<Theme, ThemeError>` stays cheap to
/// return: the error variants carry enough context to write a legible message
/// (family arcs, reference values, the missing-key list) and inlining that into
/// every `Ok` path is exactly what `clippy::result_large_err` is for.
///
/// [`kind`]: ThemeError::kind
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError {
    /// Where the source came from — a path, or a name for built-in sources.
    pub origin: String,
    /// 1-based line, or `None` for whole-file problems (missing keys).
    pub line: Option<usize>,
    /// What went wrong.
    pub kind: Box<ThemeErrorKind>,
}

impl std::error::Error for ThemeError {}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: ", self.origin)?,
            None => write!(f, "{}: ", self.origin)?,
        }
        match &*self.kind {
            ThemeErrorKind::NotAKeyValue { text } => write!(
                f,
                "expected `key: value`, found `{text}`\n    \
                 (a comment must start the line with `#`; there are no trailing comments)"
            ),
            ThemeErrorKind::UnknownKey { key } => write!(
                f,
                "unknown key `{key}`\n    \
                 a theme sets exactly the {} keys the palette has, no more",
                SLOT_KEYS.len() + 2
            ),
            ThemeErrorKind::DuplicateKey { key, first_line } => {
                write!(f, "`{key}` is set twice — first at line {first_line}")
            }
            ThemeErrorKind::BadColour { key, value } => write!(
                f,
                "`{key}` is not a colour: `{value}`\n    \
                 expected six hex digits, with or without a leading `#` (e.g. `#3ddc97`)"
            ),
            ThemeErrorKind::BadVariant { value } => write!(
                f,
                "`variant` must be `dark` or `light`, found `{value}`\n    \
                 (Design Language §10 — lightness is the theme's, and the validator \
                 needs to be told which end it is on)"
            ),
            ThemeErrorKind::EmptyName => write!(f, "`name` must not be empty"),
            ThemeErrorKind::MissingKeys { keys } => {
                write!(f, "incomplete palette — {} key(s) never set:", keys.len())?;
                for key in keys {
                    write!(f, "\n    {key}")?;
                }
                write!(
                    f,
                    "\n    a missing field becomes an inlined literal in a widget, \
                     which the T006 lint rejects — so a partial theme is not a theme"
                )
            }
            ThemeErrorKind::ActorHue {
                actor,
                family,
                value,
                hue,
                looks_like,
            } => {
                write!(
                    f,
                    "actor `{actor}` must be {family} — found {value}, which is \
                     {looks_like} (hue {hue:.0}°).\n    \
                     Hue is the contract; saturation and lightness are yours \
                     (Design Language §10: \"a theme owns lightness and syntax colors; \
                     it never owns actor identity\").\n    \
                     `{actor}` in the shipped themes:"
                )?;
                for (name, refs) in REFERENCES {
                    if name == *actor {
                        for (hex, theme) in refs {
                            write!(f, "\n      {hex}  {theme}")?;
                        }
                    }
                }
                Ok(())
            }
            ThemeErrorKind::ActorAchromatic {
                actor,
                value,
                chroma,
            } => write!(
                f,
                "actor `{actor}` has no hue to check: {value} is effectively grey \
                 (chroma {chroma:.2}, floor {MIN_CHROMA:.2}).\n    \
                 Every actor names a colour someone has to recognise at a glance; \
                 a neutral cannot do that job."
            ),
            ThemeErrorKind::ActorCollision {
                actor,
                other,
                value,
            } => write!(
                f,
                "actors `{actor}` and `{other}` are both {value}.\n    \
                 The actor palette is how you tell at a glance who did what \
                 (Design Language §1) — two actors cannot share one colour."
            ),
        }
    }
}

// ── colour maths ─────────────────────────────────────────────────────────────

/// Hue in degrees, `0.0..360.0`. Returns `0.0` for a grey — meaningless, which
/// is why [`chroma`] is checked first.
fn hue_degrees(rgb: Rgb) -> f32 {
    let (r, g, b) = (
        f32::from(rgb.0) / 255.0,
        f32::from(rgb.1) / 255.0,
        f32::from(rgb.2) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta <= f32::EPSILON {
        return 0.0;
    }
    let hue = if max <= r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max <= g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    if hue < 0.0 { hue + 360.0 } else { hue }
}

/// `(max - min) / 255` over the channels — "how far from grey".
fn chroma(rgb: Rgb) -> f32 {
    let max = rgb.0.max(rgb.1).max(rgb.2);
    let min = rgb.0.min(rgb.1).min(rgb.2);
    f32::from(max - min) / 255.0
}

/// A plain-English name for a hue, **for error prose only**.
///
/// Deliberately not the same table as [`FAMILIES`]: this one partitions the
/// whole circle so every rejected colour gets a name, whereas `FAMILIES` is the
/// contract and has gaps no actor may sit in. Nothing branches on this.
fn describe_hue(hue: f32) -> &'static str {
    match hue {
        h if !(15.0..345.0).contains(&h) => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 160.0 => "green",
        h if h < 195.0 => "teal",
        h if h < 255.0 => "blue",
        h if h < 300.0 => "violet",
        _ => "magenta",
    }
}

fn parse_hex(value: &str) -> Option<Rgb> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

fn hex_of(rgb: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

// ── the loader ───────────────────────────────────────────────────────────────

impl Theme {
    /// Parse a base16-style theme, then validate its actor hues.
    ///
    /// `origin` is only ever used to build error messages — pass the path the
    /// source came from, or a stable name for a compiled-in theme.
    ///
    /// # Errors
    ///
    /// Returns the *first* problem found, in file order: syntax, then unknown
    /// or duplicate keys, then bad values, then missing keys, then the actor
    /// contract. One legible error beats a list nobody reads to the end.
    pub fn load(origin: &str, source: &str) -> Result<Self, ThemeError> {
        let mut name: Option<String> = None;
        let mut variant: Option<Variant> = None;
        let mut slots: [Slotted; SLOT_KEYS.len()] = [None; SLOT_KEYS.len()];

        let err = |line: usize, kind: ThemeErrorKind| ThemeError {
            origin: origin.to_owned(),
            line: Some(line),
            kind: Box::new(kind),
        };

        for (i, raw) in source.lines().enumerate() {
            let line = i + 1;
            let text = raw.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }
            let Some((key, value)) = text.split_once(':') else {
                return Err(err(
                    line,
                    ThemeErrorKind::NotAKeyValue {
                        text: text.to_owned(),
                    },
                ));
            };
            let (key, value) = (key.trim(), value.trim());

            match key {
                "name" => {
                    if value.is_empty() {
                        return Err(err(line, ThemeErrorKind::EmptyName));
                    }
                    name = Some(value.to_owned());
                }
                "variant" => {
                    variant = Some(match value {
                        "dark" => Variant::Dark,
                        "light" => Variant::Light,
                        other => {
                            return Err(err(
                                line,
                                ThemeErrorKind::BadVariant {
                                    value: other.to_owned(),
                                },
                            ));
                        }
                    });
                }
                _ => {
                    let Some(slot) = Slot::from_key(key) else {
                        return Err(err(
                            line,
                            ThemeErrorKind::UnknownKey {
                                key: key.to_owned(),
                            },
                        ));
                    };
                    if let Some((_, first_line)) = slots[slot as usize] {
                        return Err(err(
                            line,
                            ThemeErrorKind::DuplicateKey {
                                key: key.to_owned(),
                                first_line,
                            },
                        ));
                    }
                    let Some(rgb) = parse_hex(value) else {
                        return Err(err(
                            line,
                            ThemeErrorKind::BadColour {
                                key: key.to_owned(),
                                value: value.to_owned(),
                            },
                        ));
                    };
                    slots[slot as usize] = Some((rgb, line));
                }
            }
        }

        let mut missing: Vec<&'static str> = Vec::new();
        if name.is_none() {
            missing.push("name");
        }
        if variant.is_none() {
            missing.push("variant");
        }
        for (i, key) in SLOT_KEYS.iter().enumerate() {
            if slots[i].is_none() {
                missing.push(key);
            }
        }
        if !missing.is_empty() {
            return Err(ThemeError {
                origin: origin.to_owned(),
                line: None,
                kind: Box::new(ThemeErrorKind::MissingKeys { keys: missing }),
            });
        }

        validate_actors(origin, &slots)?;

        let at = |slot: Slot| {
            let (rgb, _) = slots[slot as usize].expect("checked above");
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        };

        Ok(Self {
            name: Cow::Owned(name.expect("checked above")),
            variant: variant.expect("checked above"),
            actors: ActorPalette {
                claude: at(Slot::Claude),
                you: at(Slot::You),
                attention: at(Slot::Attention),
                trouble: at(Slot::Trouble),
                transient: at(Slot::Transient),
                steel: at(Slot::Steel),
            },
            neutrals: NeutralRamp {
                ground: at(Slot::Ground),
                text: at(Slot::Text),
                prose: at(Slot::Prose),
                meta: at(Slot::Meta),
                line_numbers: at(Slot::LineNumbers),
                dimmed_under_float: at(Slot::DimmedUnderFloat),
                bright_text: at(Slot::BrightText),
            },
            regions: RegionTints {
                anchor: at(Slot::RegionAnchor),
                anchor_undercurl: at(Slot::RegionAnchorUndercurl),
                selection: at(Slot::RegionSelection),
                failure: at(Slot::RegionFailure),
                failure_undercurl: at(Slot::RegionFailureUndercurl),
            },
            float: FloatChrome {
                informational: at(Slot::FloatInformational),
                needs_you: at(Slot::FloatNeedsYou),
                needs_you_body: at(Slot::FloatNeedsYouBody),
                needs_you_rule: at(Slot::FloatNeedsYouRule),
                passive: at(Slot::FloatPassive),
                body: at(Slot::FloatBody),
            },
            chrome: Chrome {
                statusline: at(Slot::ChromeStatusline),
                mode_chip_fg: at(Slot::ChromeModeChipFg),
                tab_bar: at(Slot::ChromeTabBar),
                tab_bar_rule: at(Slot::ChromeTabBarRule),
                divider: at(Slot::ChromeDivider),
            },
            syntax: SyntaxMap {
                text: at(Slot::SyntaxText),
                keyword: at(Slot::SyntaxKeyword),
                ty: at(Slot::SyntaxType),
                function: at(Slot::SyntaxFunction),
                constant: at(Slot::SyntaxConstant),
                string: at(Slot::SyntaxString),
                number: at(Slot::SyntaxNumber),
                comment: at(Slot::SyntaxComment),
            },
        })
    }
}

/// The three checks, in the order the module docs give them.
fn validate_actors(origin: &str, slots: &[Slotted]) -> Result<(), ThemeError> {
    let err = |line: usize, kind: ThemeErrorKind| ThemeError {
        origin: origin.to_owned(),
        line: Some(line),
        kind: Box::new(kind),
    };

    for (i, slot) in ACTOR_SLOTS.iter().enumerate() {
        let (rgb, line) = slots[*slot as usize].expect("missing keys already rejected");
        let (actor, family) = FAMILIES[i];
        let c = chroma(rgb);
        if c < MIN_CHROMA {
            return Err(err(
                line,
                ThemeErrorKind::ActorAchromatic {
                    actor,
                    value: hex_of(rgb),
                    chroma: c,
                },
            ));
        }
        let hue = hue_degrees(rgb);
        if !family.contains(hue) {
            return Err(err(
                line,
                ThemeErrorKind::ActorHue {
                    actor,
                    family,
                    value: hex_of(rgb),
                    hue,
                    looks_like: describe_hue(hue),
                },
            ));
        }
    }

    for (i, first) in ACTOR_SLOTS.iter().enumerate() {
        let (a, _) = slots[*first as usize].expect("checked");
        for (offset, second) in ACTOR_SLOTS.iter().skip(i + 1).enumerate() {
            let (b, line) = slots[*second as usize].expect("checked");
            if a == b {
                return Err(err(
                    line,
                    ThemeErrorKind::ActorCollision {
                        actor: FAMILIES[i + 1 + offset].0,
                        other: FAMILIES[i].0,
                        value: hex_of(a),
                    },
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete, valid file, so a test can mutate one line of it.
    const GOOD: &str = include_str!("../../themes/phosphor-dark.theme");

    fn with_line_replaced(key: &str, value: &str) -> String {
        GOOD.lines()
            .map(|l| {
                if l.starts_with(&format!("{key}:")) {
                    format!("{key}: {value}")
                } else {
                    l.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn hue_matches_the_values_the_arcs_were_drawn_from() {
        // Spot-checks against the table in FAMILIES' doc comment. If the hue
        // maths drifts, the arcs stop meaning what they say.
        let cases = [
            ("#3ddc97", 154.0),
            ("#9ece6a", 88.8),
            ("#82aecd", 204.8),
            ("#f52a65", 342.6),
            ("#179299", 183.2),
        ];
        for (hex, expected) in cases {
            let got = hue_degrees(parse_hex(hex).unwrap());
            assert!(
                (got - expected).abs() < 0.2,
                "{hex}: hue {got:.1}°, expected {expected:.1}°"
            );
        }
    }

    #[test]
    fn a_red_claude_is_rejected_and_the_error_is_legible() {
        // T011's acceptance criterion, exactly as TASKS.md words it.
        let source = with_line_replaced("actor.claude", "#d9534f");
        let error = Theme::load("bad.theme", &source).expect_err("a red claude must not load");
        let message = error.to_string();

        assert!(matches!(
            *error.kind,
            ThemeErrorKind::ActorHue {
                actor: "claude",
                ..
            }
        ));
        // "legible" is not a vibe: the message must name the actor, the
        // expected hue family, and the offending value.
        assert!(message.contains("`claude`"), "{message}");
        assert!(message.contains("green (hue 70°–175°)"), "{message}");
        assert!(message.contains("#d9534f"), "{message}");
        let claude_line = GOOD
            .lines()
            .position(|l| l.starts_with("actor.claude:"))
            .expect("fixture has a claude line")
            + 1;
        assert!(
            message.contains(&format!("bad.theme:{claude_line}")),
            "{message}"
        );
    }

    #[test]
    fn every_actor_is_locked_not_just_claude() {
        // Magenta (312°) sits in no actor's arc, so one value tests all six.
        for (actor, _) in FAMILIES {
            let source = with_line_replaced(&format!("actor.{actor}"), "#c72fa8");
            let error =
                Theme::load("bad.theme", &source).expect_err("a magenta actor must not load");
            assert!(
                matches!(*error.kind, ThemeErrorKind::ActorHue { actor: a, .. } if a == actor),
                "{actor}: {error}"
            );
        }
    }

    #[test]
    fn a_grey_actor_is_rejected_as_achromatic_not_as_the_wrong_hue() {
        // #808080 reports hue 0.0 and would otherwise "pass" as trouble-red.
        let source = with_line_replaced("actor.trouble", "#808080");
        let error = Theme::load("bad.theme", &source).expect_err("grey is not an actor colour");
        assert!(matches!(
            *error.kind,
            ThemeErrorKind::ActorAchromatic {
                actor: "trouble",
                ..
            }
        ));
        assert!(error.to_string().contains("effectively grey"));
    }

    #[test]
    fn two_actors_may_not_share_a_colour() {
        let source = with_line_replaced("actor.steel", "#3ddc97");
        let error = Theme::load("bad.theme", &source).expect_err("steel == claude must not load");
        assert!(matches!(*error.kind, ThemeErrorKind::ActorCollision { .. }));
    }

    #[test]
    fn saturation_and_lightness_are_the_themes_to_move() {
        // The other half of the rule: a much darker, much duller claude with
        // the same hue is a legitimate restyle and must load.
        let source = with_line_replaced("actor.claude", "#1d4a35");
        Theme::load("ok.theme", &source).expect("same hue, different lightness — allowed");
    }

    #[test]
    fn an_incomplete_palette_names_what_is_missing() {
        let source = GOOD
            .lines()
            .filter(|l| !l.starts_with("float.passive:") && !l.starts_with("syntax.number:"))
            .collect::<Vec<_>>()
            .join("\n");
        let error = Theme::load("short.theme", &source).expect_err("partial theme");
        let message = error.to_string();
        assert!(message.contains("float.passive"), "{message}");
        assert!(message.contains("syntax.number"), "{message}");
    }

    #[test]
    fn unknown_and_duplicate_keys_are_errors() {
        let extra = format!("{GOOD}\nactor.jeff: #3ddc97\n");
        assert!(matches!(
            *Theme::load("x.theme", &extra).unwrap_err().kind,
            ThemeErrorKind::UnknownKey { .. }
        ));
        let twice = format!("{GOOD}\nactor.claude: #3ddc97\n");
        assert!(matches!(
            *Theme::load("x.theme", &twice).unwrap_err().kind,
            ThemeErrorKind::DuplicateKey { .. }
        ));
    }

    #[test]
    fn a_hex_value_is_not_mistaken_for_a_comment() {
        // The one real hazard in the format: values start with `#`.
        let theme = Theme::load("x.theme", GOOD).expect("loads");
        assert_eq!(theme.actors.claude, Color::Rgb(0x3d, 0xdc, 0x97));
    }
}
