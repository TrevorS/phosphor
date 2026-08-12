//! The shipped themes (`T012`, `T013`).
//!
//! Six files, four palettes, each dark + light:
//!
//! | slug | `T` | where the values come from |
//! |---|---|---|
//! | `phosphor-dark` | `T010` | Design Language §1, verbatim. **The default.** |
//! | `phosphor-light` | `T012` | Mockup `8c` — "warm paper with deepened hues", claude-green `#1a9a62` (§10) |
//! | `catppuccin-mocha` | `T013` | Mockup `9a` left, over the published Catppuccin Mocha palette |
//! | `catppuccin-latte` | `T013` | Mockup `9a` right, over Latte |
//! | `tokyo-night` | `T013` | Published Tokyo Night. [Q7] — Ayu is out |
//! | `tokyo-night-day` | `T013` | Published Tokyo Night Day |
//!
//! Ayu is **not** here and must not be added: its identity colour is orange,
//! which Design Language §1 reserves for `attention`, and Q7 replaced it with
//! Tokyo Night rather than bending the actor contract to fit. Q7 amends three
//! design docs that still name Ayu (Design Brief "Decided since", Design
//! Language §10, the Component Breakdown's `Theme` spec) and supersedes mockup
//! `9b`; `9b` survives only as the *shape* of the Tokyo Night acceptance test —
//! same slice of UI, second palette, actor contract intact.
//!
//! # No second code path
//!
//! Every one of these goes through [`Theme::load`], the same parser and the
//! same validator a user's own file gets. A built-in that could not be loaded
//! by the shipped loader would be a lie in the format's documentation, so the
//! tests below load all six and assert they pass actor-hue validation — which
//! is `T012`'s and `T013`'s *done when*, in the form of a build failure.
//!
//! [Q7]: ../../../../docs/IMPLEMENTATION-PLAN.md

use std::sync::OnceLock;

use super::Theme;

/// One built-in: how a user names it, and the file it is compiled in from.
struct Builtin {
    slug: &'static str,
    source: &'static str,
    cache: OnceLock<Theme>,
}

macro_rules! builtins {
    ($($slug:literal),* $(,)?) => {
        /// The shipped themes, in the order a picker should list them —
        /// the default first, then its light twin, then the mappings.
        pub const BUILTIN_SLUGS: [&str; [$($slug),*].len()] = [$($slug),*];

        static BUILTINS: [Builtin; BUILTIN_SLUGS.len()] = [
            $(Builtin {
                slug: $slug,
                source: include_str!(concat!("../../themes/", $slug, ".theme")),
                cache: OnceLock::new(),
            }),*
        ];
    };
}

builtins! {
    "phosphor-dark",
    "phosphor-light",
    "catppuccin-mocha",
    "catppuccin-latte",
    "tokyo-night",
    "tokyo-night-day",
}

/// Look a shipped theme up by slug — `"catppuccin-mocha"`, `"tokyo-night-day"`.
///
/// `None` for a slug we do not ship; the caller decides whether that is a
/// user's typo or a path to read from disk.
///
/// # Panics
///
/// If a *compiled-in* theme file fails to parse or validate. That is a build
/// defect, not a user error — the tests in this module load all six, so it
/// cannot reach a release.
#[must_use]
pub fn builtin(slug: &str) -> Option<Theme> {
    let entry = BUILTINS.iter().find(|b| b.slug == slug)?;
    Some(
        entry
            .cache
            .get_or_init(|| {
                Theme::load(entry.slug, entry.source).unwrap_or_else(|error| {
                    panic!("built-in theme `{}` does not load: {error}", entry.slug)
                })
            })
            .clone(),
    )
}

impl Theme {
    /// Phosphor light — "warm paper with deepened hues" (§10), mockup `8c`.
    /// Claude-green deepens to `#1a9a62`; same hue, contrast-corrected.
    #[must_use]
    pub fn phosphor_light() -> Self {
        builtin("phosphor-light").expect("shipped")
    }

    /// Catppuccin Mocha — mockup `9a`, left.
    #[must_use]
    pub fn catppuccin_mocha() -> Self {
        builtin("catppuccin-mocha").expect("shipped")
    }

    /// Catppuccin Latte — mockup `9a`, right.
    #[must_use]
    pub fn catppuccin_latte() -> Self {
        builtin("catppuccin-latte").expect("shipped")
    }

    /// Tokyo Night — Q7's replacement for Ayu. No mockup; `9b`'s acceptance
    /// shape with this palette substituted.
    #[must_use]
    pub fn tokyo_night() -> Self {
        builtin("tokyo-night").expect("shipped")
    }

    /// Tokyo Night Day — the light half of the pair, and half of why Q7 chose
    /// Tokyo Night: it is a real light variant rather than an afterthought.
    #[must_use]
    pub fn tokyo_night_day() -> Self {
        builtin("tokyo-night-day").expect("shipped")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Variant;

    /// `T012` + `T013`, both *done when*s: all six load, and loading is what
    /// runs actor-hue validation.
    #[test]
    fn all_six_shipped_themes_load_and_pass_validation() {
        for slug in BUILTIN_SLUGS {
            let theme = builtin(slug).unwrap_or_else(|| panic!("`{slug}` is not registered"));
            assert!(!theme.name.is_empty(), "{slug} has no name");
        }
    }

    #[test]
    fn the_data_file_and_the_const_fn_are_the_same_palette() {
        // phosphor dark exists twice — as `Theme::phosphor_dark()` (T010's
        // reference encoding, diffable against §1 by eye) and as a theme file.
        // Neither is allowed to drift.
        assert_eq!(
            Theme::phosphor_dark(),
            builtin("phosphor-dark").expect("shipped")
        );
    }

    #[test]
    fn dark_is_the_default() {
        // §10: "Phosphor (dark + light) ships as the v1 default."
        assert_eq!(Theme::default(), Theme::phosphor_dark());
        assert_eq!(BUILTIN_SLUGS[0], "phosphor-dark");
    }

    #[test]
    fn each_family_ships_a_dark_and_a_light() {
        let pairs = [
            (Theme::phosphor_dark(), Theme::phosphor_light()),
            (Theme::catppuccin_mocha(), Theme::catppuccin_latte()),
            (Theme::tokyo_night(), Theme::tokyo_night_day()),
        ];
        for (dark, light) in pairs {
            assert_eq!(dark.variant, Variant::Dark, "{} dark", dark.name);
            assert_eq!(light.variant, Variant::Light, "{} light", light.name);
            assert_eq!(dark.name, light.name, "a pair shares one name");
        }
    }

    #[test]
    fn ayu_is_not_shipped() {
        // Q7. Named here so a future "let's add the third mapping back" has to
        // delete an assertion that says why not.
        assert!(!BUILTIN_SLUGS.iter().any(|s| s.contains("ayu")));
    }

    #[test]
    fn the_planted_bad_theme_is_rejected() {
        // `CP-1` verifies this by hand; here it is as a build failure.
        let source = include_str!("../../themes/fixtures/claude-is-red.theme");
        let error = Theme::load("fixtures/claude-is-red.theme", source)
            .expect_err("a red claude must not load");
        let message = error.to_string();
        assert!(message.contains("`claude`"), "{message}");
        assert!(message.contains("green"), "{message}");
        assert!(message.contains("#d9534f"), "{message}");
    }

    #[test]
    fn claude_green_deepens_on_light_rather_than_moving() {
        use ratatui_core::style::Color;
        // §10, the whole thesis of the light variant: "claude-green is #3ddc97
        // on dark, #1a9a62 on light — same hue, contrast-corrected."
        assert_eq!(
            Theme::phosphor_dark().actors.claude,
            Color::Rgb(0x3d, 0xdc, 0x97)
        );
        assert_eq!(
            Theme::phosphor_light().actors.claude,
            Color::Rgb(0x1a, 0x9a, 0x62)
        );
    }

    #[test]
    fn the_mockup_values_survived_the_transcription() {
        use ratatui_core::style::Color;
        // Spot-check the values a reader can read straight off a mockup, so a
        // regenerated theme file that quietly moved one fails here rather than
        // at CP-1. Mockup `8c` (phosphor light) and `9a` (catppuccin).
        let light = Theme::phosphor_light();
        assert_eq!(light.neutrals.ground, Color::Rgb(0xf4, 0xf2, 0xec));
        assert_eq!(light.neutrals.line_numbers, Color::Rgb(0xb0, 0xaa, 0x98));
        assert_eq!(light.chrome.statusline, Color::Rgb(0xe8, 0xe4, 0xd8));
        assert_eq!(light.regions.anchor, Color::Rgb(0xe6, 0xef, 0xe6));

        let mocha = Theme::catppuccin_mocha();
        assert_eq!(mocha.actors.claude, Color::Rgb(0xa6, 0xe3, 0xa1));
        assert_eq!(mocha.neutrals.ground, Color::Rgb(0x1e, 0x1e, 0x2e));
        assert_eq!(mocha.neutrals.line_numbers, Color::Rgb(0x45, 0x47, 0x5a));
        assert_eq!(mocha.syntax.keyword, Color::Rgb(0xcb, 0xa6, 0xf7));

        let latte = Theme::catppuccin_latte();
        assert_eq!(latte.actors.claude, Color::Rgb(0x40, 0xa0, 0x2b));
        assert_eq!(latte.neutrals.ground, Color::Rgb(0xef, 0xf1, 0xf5));
    }

    #[test]
    fn the_mode_chip_is_inverted_in_every_theme() {
        // §5: the mode chip is "the only inverted text on screen" — its
        // foreground is the theme's ground on an actor-coloured field. Every
        // mockup that shows one does this, so it is a mapping rule, not a value.
        for slug in BUILTIN_SLUGS {
            let theme = builtin(slug).expect("shipped");
            assert_eq!(
                theme.chrome.mode_chip_fg, theme.neutrals.ground,
                "{slug}: mode chip foreground is not ground"
            );
        }
    }
}
