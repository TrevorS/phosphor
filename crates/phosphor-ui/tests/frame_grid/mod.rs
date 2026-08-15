//! `T018` — the golden-frame serialiser: a rendered [`Buffer`] as committed,
//! diffable text.
//!
//! # Why this shape
//!
//! Tier 1 is *"what we told the terminal to draw"* — exact, diffable, fast
//! (`TASKS.md`'s tier table), and **the only tier that gates CI**. A cell grid
//! of glyphs alone would prove layout and nothing else, and `CP-1` is *"does it
//! look like the mockups"*, which is at least half a question about colour. So
//! a frame serialises as **four aligned grids over the same cells** —
//! `text`, `fg`, `bg`, and `style` when anything is styled — plus a legend.
//!
//! # Colours are named, never hex
//!
//! Every cell's colour is looked up in [`palette`], the flat list of every
//! field a [`Theme`] has, and printed as its **key letter**; the legend spells
//! the key out as `actors.claude` and its hex. Two consequences, both of them
//! the point:
//!
//! * A colour that is on screen but is **not** a `Theme` field prints as `?`
//!   and lands in the legend as `!UNNAMED`. That is a hard failure of Design
//!   Language §12 (*"every widget takes `&Theme` — no literal colors in
//!   components"*) which no grep-based lint can see, because a widget can reach
//!   a literal through a helper. [`Frame::unnamed`] is asserted empty in
//!   `golden_frames.rs`.
//! * Keys are assigned by **position in [`palette`]**, not by order of
//!   appearance — so `a` is `actors.claude` in every frame, every theme, for
//!   ever. A dark frame and a light frame of the same content diff to *nothing*
//!   in the `fg`/`bg` grids, which is exactly the claim `8c` and `9c` make
//!   (*"hue is the contract, lightness is the theme's"*).
//!
//! # `Buffer`, not `TestBackend`
//!
//! The plan says *"ratatui `TestBackend` → cell grid"*. `TestBackend` **is** a
//! `Buffer` plus a `Terminal`, and `Terminal::draw` adds nothing a widget test
//! can observe — while `ratatui` (the full crate, where `TestBackend` lives)
//! would become a dev-dependency of a crate whose whole manifest comment is
//! *"`ratatui-core` only, never `ratatui`"*. The cell grid is identical; the
//! rule stays intact. Flagged in `T018`'s report rather than decided quietly.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::style::{Color, Modifier};

/// Key characters, assigned by index into [`palette`]. 37 fields, so the
/// lowercase alphabet plus eleven capitals.
const KEYS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJK";

/// The cell that carries no colour at all — never written, or written with a
/// style that set only the other channel.
const UNSET: char = '.';

/// A colour on screen that is in no [`Theme`] field. §12's violation, made
/// visible.
const UNNAMED: char = '?';

/// Every named colour in a [`Theme`], flattened, **in a fixed order**.
///
/// The order is the key assignment, so it is part of the committed snapshots:
/// inserting a field in the middle rekeys every frame below it. Append, don't
/// insert — and if a field is ever removed, expect the churn and review it.
///
/// The six groups are `theme.rs`'s own structs in their declaration order:
/// actors, neutrals, regions, float, chrome, syntax.
#[must_use]
pub(crate) fn palette(theme: &Theme) -> Vec<(&'static str, Color)> {
    vec![
        ("actors.claude", theme.actors.claude),
        ("actors.you", theme.actors.you),
        ("actors.attention", theme.actors.attention),
        ("actors.trouble", theme.actors.trouble),
        ("actors.transient", theme.actors.transient),
        ("actors.steel", theme.actors.steel),
        ("neutrals.ground", theme.neutrals.ground),
        ("neutrals.text", theme.neutrals.text),
        ("neutrals.prose", theme.neutrals.prose),
        ("neutrals.meta", theme.neutrals.meta),
        ("neutrals.line_numbers", theme.neutrals.line_numbers),
        (
            "neutrals.dimmed_under_float",
            theme.neutrals.dimmed_under_float,
        ),
        ("neutrals.bright_text", theme.neutrals.bright_text),
        ("regions.anchor", theme.regions.anchor),
        ("regions.anchor_undercurl", theme.regions.anchor_undercurl),
        ("regions.selection", theme.regions.selection),
        ("regions.failure", theme.regions.failure),
        ("regions.failure_undercurl", theme.regions.failure_undercurl),
        ("float.informational", theme.float.informational),
        ("float.needs_you", theme.float.needs_you),
        ("float.needs_you_body", theme.float.needs_you_body),
        ("float.needs_you_rule", theme.float.needs_you_rule),
        ("float.passive", theme.float.passive),
        ("float.body", theme.float.body),
        ("chrome.statusline", theme.chrome.statusline),
        ("chrome.mode_chip_fg", theme.chrome.mode_chip_fg),
        ("chrome.tab_bar", theme.chrome.tab_bar),
        ("chrome.tab_bar_rule", theme.chrome.tab_bar_rule),
        ("chrome.divider", theme.chrome.divider),
        ("syntax.text", theme.syntax.text),
        ("syntax.keyword", theme.syntax.keyword),
        ("syntax.ty", theme.syntax.ty),
        ("syntax.function", theme.syntax.function),
        ("syntax.constant", theme.syntax.constant),
        ("syntax.string", theme.syntax.string),
        ("syntax.number", theme.syntax.number),
        ("syntax.comment", theme.syntax.comment),
    ]
}

/// `#rrggbb` for a truecolor value — `Color`'s own `Display`, lowercased to
/// match how the design docs write hex.
///
/// A non-truecolor value spells itself (`Reset`, `DarkGray`, or a bare index)
/// and, being in no `Theme` field, lands in the legend as `!UNNAMED` — which is
/// the correct outcome: the design language is truecolor, and a 256-colour cell
/// on a phosphor surface is a finding, not a detail to normalise away.
///
/// Deliberately not written as a `match` on `Color::Rgb`:
/// `scripts/lint-no-literal-colours.sh` greps this crate for that spelling and
/// makes no exception for a test, which is the right default — so the round
/// trip goes through `Display` and never names a variant.
fn hex(colour: Color) -> String {
    colour.to_string().to_ascii_lowercase()
}

/// One cell, decomposed: the glyph the terminal will print, and whether the
/// undercurl escape (`T085`) is wrapped around it.
struct Decomposed {
    glyph: String,
    curl: bool,
    /// The SGR-58 colour as `#rrggbb`, read straight off the wire bytes — no
    /// `Color` is reconstructed, so this file never names a colour variant.
    underline_colour: Option<String>,
}

/// Splits the SGR wrapper `T085` rides in the cell **symbol** back off the
/// glyph — `ESC[4:3m` `ESC[58;2;r;g;bm` glyph `ESC[59m` `ESC[4m`.
///
/// Without this a single undercurled cell would put ~30 bytes of escape into
/// the text grid and misalign the row; with it the curl shows up in the `style`
/// grid as `~`, which is the readable form and still proves the escape is on
/// the wire.
fn decompose(symbol: &str) -> Decomposed {
    let mut glyph = String::new();
    let mut curl = false;
    let mut underline_colour = None;
    let mut chars = symbol.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            glyph.push(ch);
            continue;
        }
        let mut seq = String::new();
        for ch in chars.by_ref() {
            if ch == 'm' {
                break;
            }
            seq.push(ch);
        }
        if seq == "[4:3" {
            curl = true;
        } else if let Some(rest) = seq.strip_prefix("[58;2;") {
            let parts: Vec<&str> = rest.split(';').collect();
            if let [r, g, b] = parts[..]
                && let (Ok(r), Ok(g), Ok(b)) = (r.parse::<u8>(), g.parse::<u8>(), b.parse::<u8>())
            {
                underline_colour = Some(format!("#{r:02x}{g:02x}{b:02x}"));
            }
        }
    }
    Decomposed {
        glyph,
        curl,
        underline_colour,
    }
}

/// One screen, ready to serialise.
pub(crate) struct Frame<'a> {
    /// The mockup id this reproduces — the snapshot's name.
    pub(crate) screen: &'a str,
    /// Human-readable theme label for the header line.
    pub(crate) theme_label: &'a str,
    /// The theme every colour is resolved against.
    pub(crate) theme: &'a Theme,
    /// What is in the frame, and — more importantly — what is **not**, because
    /// it does not exist yet. One line each; they go into the snapshot so a
    /// reader of the `.snap` never has to guess whether an absence is a bug.
    pub(crate) notes: &'a [&'a str],
}

impl Frame<'_> {
    /// Every colour in `buf` that is not a field of the theme.
    ///
    /// Empty is the contract (§12). Non-empty is a widget that reached a colour
    /// without going through `&Theme` — which the grep lint cannot see.
    #[must_use]
    pub(crate) fn unnamed(&self, buf: &Buffer) -> Vec<String> {
        let named = self.palette_index();
        let mut found: Vec<String> = Vec::new();
        for y in buf.area.top()..buf.area.bottom() {
            for x in buf.area.left()..buf.area.right() {
                let cell = &buf[(x, y)];
                for colour in [cell.fg, cell.bg] {
                    if colour == Color::Reset || named.contains_key(&hex(colour)) {
                        continue;
                    }
                    let spelled = hex(colour);
                    if !found.contains(&spelled) {
                        found.push(spelled);
                    }
                }
            }
        }
        found
    }

    /// hex → (key char, every field name carrying that value).
    fn palette_index(&self) -> BTreeMap<String, (char, Vec<&'static str>)> {
        let mut index: BTreeMap<String, (char, Vec<&'static str>)> = BTreeMap::new();
        for (position, (name, colour)) in palette(self.theme).into_iter().enumerate() {
            let key = KEYS.get(position).map_or(UNNAMED, |byte| char::from(*byte));
            index
                .entry(hex(colour))
                .and_modify(|entry| entry.1.push(name))
                .or_insert((key, vec![name]));
        }
        index
    }

    /// The frame as committed text.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one linear document builder; splitting it would scatter the \
                  snapshot's layout across four functions that are only ever \
                  called in this order"
    )]
    pub(crate) fn to_text(&self, buf: &Buffer) -> String {
        let area = buf.area;
        let index = self.palette_index();
        let width = area.width as usize;

        let mut text_rows: Vec<String> = Vec::new();
        let mut fg_rows: Vec<String> = Vec::new();
        let mut bg_rows: Vec<String> = Vec::new();
        let mut style_rows: Vec<String> = Vec::new();
        let mut used: Vec<char> = Vec::new();
        let mut curl_colours: Vec<String> = Vec::new();
        let mut any_style = false;
        let mut any_struck = false;

        let key_of = |colour: Color, used: &mut Vec<char>| -> char {
            if colour == Color::Reset {
                return UNSET;
            }
            let key = index.get(&hex(colour)).map_or(UNNAMED, |entry| entry.0);
            if !used.contains(&key) {
                used.push(key);
            }
            key
        };

        for y in area.top()..area.bottom() {
            let (mut text, mut fgs, mut bgs, mut styles) = (
                String::with_capacity(width),
                String::with_capacity(width),
                String::with_capacity(width),
                String::with_capacity(width),
            );
            for x in area.left()..area.right() {
                let cell = &buf[(x, y)];
                let decomposed = decompose(cell.symbol());
                if decomposed.glyph.is_empty() {
                    // A wide glyph's trailing cell. Marked, not skipped, so the
                    // grids stay column-aligned with the text.
                    text.push('\u{2219}');
                } else {
                    text.push_str(&decomposed.glyph);
                }
                fgs.push(key_of(cell.fg, &mut used));
                bgs.push(key_of(cell.bg, &mut used));

                let style = if decomposed.curl {
                    if let Some(colour) = decomposed.underline_colour
                        && !curl_colours.contains(&colour)
                    {
                        curl_colours.push(colour);
                    }
                    '~'
                } else if cell.modifier.contains(Modifier::UNDERLINED) {
                    '_'
                } else if cell.modifier.contains(Modifier::CROSSED_OUT) {
                    any_struck = true;
                    's'
                } else if cell.modifier.contains(Modifier::REVERSED) {
                    'r'
                } else if cell.modifier.contains(Modifier::BOLD) {
                    'b'
                } else if cell.modifier.is_empty() {
                    UNSET
                } else {
                    '*'
                };
                any_style |= style != UNSET;
                styles.push(style);
            }
            text_rows.push(text);
            fg_rows.push(fgs);
            bg_rows.push(bgs);
            style_rows.push(styles);
        }

        let mut out = String::new();
        let _ = writeln!(
            out,
            "screen {} · {} · {}x{} cells",
            self.screen, self.theme_label, area.width, area.height
        );
        for note in self.notes {
            let _ = writeln!(out, "  {note}");
        }

        let ruler_tens: String = (0..area.width)
            .map(|x| {
                if x % 10 == 0 {
                    char::from(b'0' + u8::try_from((x / 10) % 10).unwrap_or(0))
                } else {
                    ' '
                }
            })
            .collect();
        let ruler_ones: String = (0..area.width)
            .map(|x| char::from(b'0' + u8::try_from(x % 10).unwrap_or(0)))
            .collect();

        let ruler_tens = ruler_tens.trim_end().to_owned();
        let grid = |title: &str, rows: &[String], out: &mut String| {
            let _ = writeln!(out, "\n{title}");
            let _ = writeln!(out, "     {ruler_tens}");
            let _ = writeln!(out, "     {ruler_ones}");
            for (y, row) in rows.iter().enumerate() {
                let _ = writeln!(out, "{y:>3} │{row}│");
            }
        };

        grid("text", &text_rows, &mut out);
        grid("fg", &fg_rows, &mut out);
        grid("bg", &bg_rows, &mut out);
        if any_style {
            grid("style", &style_rows, &mut out);
        }

        let _ = writeln!(out, "\nlegend");
        for (position, (name, colour)) in palette(self.theme).into_iter().enumerate() {
            let key = KEYS.get(position).map_or(UNNAMED, |byte| char::from(*byte));
            if !used.contains(&key) {
                continue;
            }
            let spelled = hex(colour);
            let Some((_, names)) = index.get(&spelled) else {
                continue;
            };
            // Only the first field of a shared value prints; the rest are its
            // aliases on that one line.
            if names.first() != Some(&name) {
                continue;
            }
            let _ = writeln!(out, "  {key}  {spelled}  {}", names.join(" = "));
        }
        if used.contains(&UNSET) {
            let _ = writeln!(out, "  {UNSET}  —        no colour set on this channel");
        }
        for colour in self.unnamed(buf) {
            let _ = writeln!(
                out,
                "  {UNNAMED}  {colour}  !UNNAMED — not a Theme field (Design Language §12)"
            );
        }
        if any_style {
            let _ = writeln!(out, "\nstyle");
            let _ = writeln!(
                out,
                "  ~  undercurl — SGR 4:3 wrapped around the glyph, plus Modifier::UNDERLINED"
            );
            let _ = writeln!(out, "  _  Modifier::UNDERLINED alone (the degraded path)");
            // **Conditional where the two above are unconditional**, and the
            // asymmetry is deliberate: `~` and `_` are one degradation path and
            // have been in every committed legend since `T018`, so making them
            // conditional now would rewrite frames that did not change. A line
            // added later has to earn its place on the frames that use it, or
            // the first new treatment churns forty snapshots to say nothing.
            if any_struck {
                let _ = writeln!(
                    out,
                    "  s  Modifier::CROSSED_OUT — a deprecated completion, which also \
                     recedes one step down §1's neutral ramp so a terminal that ignores \
                     SGR 9 still says so"
                );
            }
            for spelled in &curl_colours {
                let name = index
                    .get(spelled)
                    .map_or("!UNNAMED", |entry| entry.1.first().unwrap_or(&"!UNNAMED"));
                let _ = writeln!(out, "     underline colour {spelled}  {name}");
            }
        }
        out
    }
}
