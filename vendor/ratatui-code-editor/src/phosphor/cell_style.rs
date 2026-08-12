//! PHOSPHOR PATCH 5 — cell styles the marks API cannot carry, with a
//! degradation path. `T085`.
//!
//! Upstream's marks are `(start, end, Color)` and nothing else — a background
//! tint, no style and no priority (`editor.rs`, and `SPIKES.md` seam 1). Design
//! Language §3 draws an anchored region as **"tint + undercurl"**, so the tint
//! is upstream's job and the undercurl is this module's.
//!
//! # Why this is not just a `Modifier`
//!
//! `ratatui_core::style::Modifier` has nine bits and none of them is undercurl;
//! it has `UNDERLINED` (SGR 4) and stops there. A curly underline is SGR `4:3`,
//! a sub-parameter form no ratatui backend emits, so there is no `Style` that
//! means "curly". The only channel from a `Buffer` cell to the terminal that
//! carries arbitrary bytes is the cell's **symbol**, which every backend writes
//! verbatim (`Print(cell.symbol())`) — so the SGR pair rides there, wrapped
//! around the glyph, self-contained per cell so a partial redraw stays correct.
//!
//! # The degradation path is the point
//!
//! A consumer asks for [`Underline::Curl`] and never learns which terminal it
//! is on. [`UnderlineCapability`] resolves that once, and a span always sets
//! `Modifier::UNDERLINED` on the cell — so on a terminal without undercurl the
//! escape is simply never emitted and what is left is a straight underline.
//! Degradation is the absence of an addition, not a second code path.
//!
//! Design Language §8: *"markers become `▎`, undercurl becomes underline,
//! spinner becomes a static `✻`."*

use ratatui_core::buffer::{Buffer, CellDiffOption, CellWidth};
use ratatui_core::style::{Color, Modifier, Style};
use std::num::NonZeroU16;
use std::sync::OnceLock;

/// How the line under a styled span is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Underline {
    /// No underline. The default, so a `CellStyle` that carries only a colour
    /// draws nothing.
    #[default]
    None,
    /// Straight underline — SGR 4. Present on every terminal in the matrix.
    Straight,
    /// Curly underline — SGR `4:3`, degraded to [`Underline::Straight`] where
    /// the terminal does not have it. This is what a consumer asks for; it is
    /// never asked to know whether it will get it.
    Curl,
}

/// A cell-level style, applied over a character range by [`StyledSpan`].
///
/// Deliberately not a `ratatui` `Style`: this is the set of properties that
/// either has no `Style` representation (curly underline) or whose `Style`
/// representation is feature-gated in `ratatui-core` (`underline_color`, behind
/// the `underline-color` feature, which this crate does not enable and cannot
/// rely on a consumer enabling). Everything else a caller wants on a cell it
/// already has through the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CellStyle {
    /// Which underline to draw.
    pub underline: Underline,
    /// The underline's colour — SGR 58, emitted only alongside an undercurl.
    ///
    /// `None` leaves the underline in the cell's foreground colour, which is
    /// what the degradation path always gets: a terminal without SGR `4:3`
    /// almost never has SGR 58 either, and phosphor does not send colour bytes
    /// a terminal has not advertised.
    pub color: Option<Color>,
}

impl CellStyle {
    /// The anchored-region and diagnostic treatment: a curly underline in
    /// `color`, degrading to a straight underline. Design Language §3.
    pub const fn undercurl(color: Color) -> Self {
        Self { underline: Underline::Curl, color: Some(color) }
    }

    /// A straight underline in `color`, on every terminal. For a consumer that
    /// wants the flat treatment rather than the degraded one.
    pub const fn underline(color: Color) -> Self {
        Self { underline: Underline::Straight, color: Some(color) }
    }
}

/// A half-open character range `[start, end)` carrying a [`CellStyle`].
///
/// Offsets are character indices into the document — the same coordinates
/// `Editor::set_marks` uses, so a consumer holding a region's range can hand
/// the same numbers to both halves of §3's "tint + undercurl".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct StyledSpan {
    /// First character in the span.
    pub start: usize,
    /// One past the last character in the span.
    pub end: usize,
    /// What to draw over it.
    pub style: CellStyle,
}

impl StyledSpan {
    /// A span with an explicit style.
    pub const fn new(start: usize, end: usize, style: CellStyle) -> Self {
        Self { start, end, style }
    }

    /// **The call site.** One verb, both terminals: `undercurl` where the
    /// terminal has SGR `4:3`, a straight underline where it does not.
    pub const fn undercurl(start: usize, end: usize, color: Color) -> Self {
        Self::new(start, end, CellStyle::undercurl(color))
    }

    /// Does this span cover `char_idx`?
    pub const fn contains(&self, char_idx: usize) -> bool {
        self.start <= char_idx && char_idx < self.end
    }
}

/// The last span covering `char_idx`, or `None`.
///
/// Last, not first: spans are applied in the order the caller supplied them,
/// so a later span wins — the same rule upstream's marks loop uses.
pub fn span_at(spans: &[StyledSpan], char_idx: usize) -> Option<&CellStyle> {
    spans.iter().rev().find(|span| span.contains(char_idx)).map(|span| &span.style)
}

/// What the terminal can draw under a cell.
///
/// One question, asked once, in one place. Consumers never see it: they ask for
/// [`Underline::Curl`] and this decides what actually reaches the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnderlineCapability {
    /// SGR `4:3` is honoured — the primary terminal (Ghostty / kitty / WezTerm).
    Undercurl,
    /// Straight underline only — the degradation target (Terminal.app), a
    /// multiplexer, `NO_COLOR`, or any terminal that has not said otherwise.
    Underline,
}

impl UnderlineCapability {
    /// The process's capability, resolved once and cached.
    ///
    /// Cached because it is read per styled cell per frame and the answer
    /// cannot change without a new process: `TERM` is fixed at exec, and the
    /// override below is read at the same moment. A caller that needs a
    /// different answer — a test, or an app layer that has negotiated with the
    /// terminal itself — sets it explicitly through
    /// `Editor::set_underline_capability` instead.
    pub fn detect() -> Self {
        static DETECTED: OnceLock<UnderlineCapability> = OnceLock::new();
        *DETECTED.get_or_init(|| {
            let var = |name: &str| std::env::var(name).ok();
            let (undercurl, no_color, term, term_program) = (
                var("PHOSPHOR_UNDERCURL"),
                var("NO_COLOR"),
                var("TERM"),
                var("TERM_PROGRAM"),
            );
            Self::resolve(&TerminalEnv {
                phosphor_undercurl: undercurl.as_deref(),
                no_color: no_color.as_deref(),
                term: term.as_deref(),
                term_program: term_program.as_deref(),
            })
        })
    }

    /// The resolution rules, as a pure function of the environment. Every
    /// branch is testable without a terminal; [`TerminalEnv::from_process`] is
    /// the only part that reads the world.
    ///
    /// In order:
    ///
    /// 1. **`PHOSPHOR_UNDERCURL`** — `1`/`true`/`on`/`always`/`force` forces
    ///    undercurl, `0`/`false`/`off`/`never` forces the fallback. The escape
    ///    hatch for a terminal the table below gets wrong, and how `CP-1`'s
    ///    four-terminal pass sees both treatments without a rebuild.
    /// 2. **`NO_COLOR`**, non-empty → fallback. `V009` pairs `NO_COLOR=1` with
    ///    the degradation path, so in phosphor it selects the whole degraded
    ///    treatment, not the colours alone.
    /// 3. **`TERM` unset, empty, or `dumb`** → fallback.
    /// 4. **`TERM` names a terminal with `Smulx`** (kitty, ghostty, wezterm,
    ///    foot, contour, alacritty, rio) → undercurl. Matched as a substring so
    ///    `xterm-kitty` and `xterm-ghostty` land here.
    /// 5. **`TERM` starts with `screen`/`tmux`** → fallback. Passthrough of
    ///    `4:3` needs tmux ≥ 3.4 *and* a `terminal-features` entry; assuming it
    ///    paints garbage inside every multiplexer that has not been configured.
    ///    Decided **before** `TERM_PROGRAM`, because tmux inside iTerm2 reports
    ///    both and it is the multiplexer that has to carry the escape.
    /// 6. **`TERM_PROGRAM`** (`ghostty`, `WezTerm`, `iTerm.app`, `vscode`,
    ///    `Rio`) → undercurl. **Teej's `CP-1` ruling moved this ahead of the
    ///    plain-family rule below.** iTerm2 and VS Code both ship
    ///    `TERM=xterm-256color` and both support `4:3`, so the old order —
    ///    `TERM` is always the authority — made `SMULX_PROGRAMS`' `iterm.app`
    ///    and `vscode` entries unreachable and degraded two capable terminals
    ///    for nothing.
    /// 7. **`TERM` names a plain family** (`xterm*`, `vt*`, `linux`, `ansi`,
    ///    `rxvt*`, `Eterm*`, `cons*`, `dtterm*`, `nsterm*`) → fallback.
    ///    A degradation capture must now force the path explicitly with
    ///    `PHOSPHOR_UNDERCURL=0` rather than leaning on `TERM`, which is what
    ///    `tapes/_undercurl-check-forced-underline.tape` already does and what
    ///    `V009` should do when it lands.
    /// 8. Anything else → fallback. **The allowlist points one way on
    ///    purpose:** the cost of missing undercurl on a capable terminal is a
    ///    flat underline, and the cost of sending `4:3` to a terminal that
    ///    mis-parses sub-parameters is visible garbage in the buffer.
    pub fn resolve(env: &TerminalEnv<'_>) -> Self {
        if let Some(forced) = env.forced() {
            return forced;
        }
        if env.no_color.is_some_and(|v| !v.is_empty()) {
            return Self::Underline;
        }

        let term = env.term.unwrap_or_default();
        if term.is_empty() || term == "dumb" {
            return Self::Underline;
        }
        let term = term.to_ascii_lowercase();

        const SMULX_TERMS: [&str; 7] =
            ["kitty", "ghostty", "wezterm", "foot", "contour", "alacritty", "rio"];
        if SMULX_TERMS.iter().any(|name| term.contains(name)) {
            return Self::Undercurl;
        }

        // Multiplexers are decided before `TERM_PROGRAM` on purpose: tmux inside
        // iTerm2 reports `TERM=screen-256color` *and* `TERM_PROGRAM=iTerm.app`,
        // and it is the multiplexer that has to pass `4:3` through.
        const MULTIPLEXERS: [&str; 2] = ["screen", "tmux"];
        if MULTIPLEXERS.iter().any(|name| term.starts_with(name)) {
            return Self::Underline;
        }

        const SMULX_PROGRAMS: [&str; 5] = ["ghostty", "wezterm", "iterm.app", "vscode", "rio"];
        let program = env.term_program.unwrap_or_default().to_ascii_lowercase();
        if SMULX_PROGRAMS.contains(&program.as_str()) {
            return Self::Undercurl;
        }

        const PLAIN_FAMILIES: [&str; 9] =
            ["xterm", "vt", "linux", "ansi", "rxvt", "eterm", "cons", "dtterm", "nsterm"];
        if PLAIN_FAMILIES.iter().any(|name| term.starts_with(name)) {
            return Self::Underline;
        }

        Self::Underline
    }
}

/// The environment [`UnderlineCapability::resolve`] reads, lifted out so the
/// rules are a pure function and [`UnderlineCapability::detect`] — which reads
/// the real process — is one caller of them rather than the only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalEnv<'a> {
    /// `PHOSPHOR_UNDERCURL` — the override.
    pub phosphor_undercurl: Option<&'a str>,
    /// `NO_COLOR` — <https://no-color.org>, and `V009`'s degradation tape.
    pub no_color: Option<&'a str>,
    /// `TERM`. The authority.
    pub term: Option<&'a str>,
    /// `TERM_PROGRAM`. Consulted only when `TERM` says nothing.
    pub term_program: Option<&'a str>,
}

impl TerminalEnv<'_> {
    fn forced(&self) -> Option<UnderlineCapability> {
        let raw = self.phosphor_undercurl?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "on" | "yes" | "always" | "force" => {
                Some(UnderlineCapability::Undercurl)
            }
            "0" | "false" | "off" | "no" | "never" => Some(UnderlineCapability::Underline),
            _ => None,
        }
    }
}

/// The half of a [`CellStyle`] that ratatui can express: the underline bit.
///
/// Set for both capabilities — it *is* the fallback, and on the primary
/// terminal it is what [`decorate_cell`] upgrades in place. Underline colour is
/// not set here; see [`CellStyle::color`].
pub fn patch_style(style: Style, cell: &CellStyle) -> Style {
    match cell.underline {
        Underline::None => style,
        Underline::Straight | Underline::Curl => style.add_modifier(Modifier::UNDERLINED),
    }
}

/// Wraps the glyph already written at `(x, y)` in the SGR pair that makes its
/// underline curly, when — and only when — the terminal has said it can.
///
/// The pair is `ESC[4:3m` … `ESC[4m`: turn the underline curly, draw, put it
/// back to straight. Straight is what the cell's `Modifier::UNDERLINED` already
/// told the backend, so the terminal is left in exactly the state the backend
/// believes it is in and no neighbouring cell inherits the curl. `ESC[58;…m` /
/// `ESC[59m` colour the underline by the same rule, in crossterm's byte form.
///
/// **`ForcedWidth` is not optional.** `Buffer::diff` measures a cell by the
/// display width of its symbol and skips that many columns; a symbol carrying
/// ~30 bytes of escape measures ~30 cells wide, and the backend would silently
/// drop the next 30 columns of the line. `CellDiffOption::ForcedWidth` exists
/// for exactly this — *"escape sequences will have some computed width that
/// does not match what is written to the screen"* — and pins the cell back to
/// the width of the glyph inside it.
///
/// A no-op for [`UnderlineCapability::Underline`] — nothing is emitted and the
/// straight underline from [`patch_style`] is the whole treatment.
pub fn decorate_cell(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    cell: &CellStyle,
    capability: UnderlineCapability,
) {
    if cell.underline != Underline::Curl || capability != UnderlineCapability::Undercurl {
        return;
    }
    let Some(target) = buf.cell_mut((x, y)) else {
        return;
    };
    let width = NonZeroU16::new(target.symbol().cell_width()).unwrap_or(NonZeroU16::MIN);
    let decorated = decorate_symbol(target.symbol(), cell.color);
    target.set_symbol(&decorated);
    target.set_diff_option(CellDiffOption::ForcedWidth(width));
}

/// [`decorate_cell`]'s string half, separated so the bytes can be asserted
/// without a `Buffer`.
pub fn decorate_symbol(symbol: &str, color: Option<Color>) -> String {
    let mut out = String::with_capacity(symbol.len() + 24);
    out.push_str("\u{1b}[4:3m");
    let colored = color.and_then(sgr_underline_color);
    if let Some(ref sgr) = colored {
        out.push_str(sgr);
    }
    out.push_str(symbol);
    if colored.is_some() {
        out.push_str("\u{1b}[59m");
    }
    out.push_str("\u{1b}[4m");
    out
}

/// SGR 58 for a colour, in the form crossterm writes it (`58;2;r;g;b` /
/// `58;5;n`) — the encoding the terminals in the matrix are known to accept,
/// because it is the one every ratatui app already sends them.
///
/// `None` for a colour with no unambiguous SGR-58 encoding (`Color::Reset`);
/// the underline then takes the cell's foreground, which is the correct
/// no-opinion result.
fn sgr_underline_color(color: Color) -> Option<String> {
    let body = match color {
        Color::Rgb(r, g, b) => format!("2;{r};{g};{b}"),
        Color::Indexed(i) => format!("5;{i}"),
        Color::Black => "5;0".into(),
        Color::Red => "5;1".into(),
        Color::Green => "5;2".into(),
        Color::Yellow => "5;3".into(),
        Color::Blue => "5;4".into(),
        Color::Magenta => "5;5".into(),
        Color::Cyan => "5;6".into(),
        Color::Gray => "5;7".into(),
        Color::DarkGray => "5;8".into(),
        Color::LightRed => "5;9".into(),
        Color::LightGreen => "5;10".into(),
        Color::LightYellow => "5;11".into(),
        Color::LightBlue => "5;12".into(),
        Color::LightMagenta => "5;13".into(),
        Color::LightCyan => "5;14".into(),
        Color::White => "5;15".into(),
        Color::Reset => return None,
    };
    Some(format!("\u{1b}[58;{body}m"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(term: &'static str) -> TerminalEnv<'static> {
        TerminalEnv { term: Some(term), ..TerminalEnv::default() }
    }

    #[test]
    fn term_names_the_capability() {
        for term in ["xterm-kitty", "xterm-ghostty", "wezterm", "foot-extra", "alacritty"] {
            assert_eq!(UnderlineCapability::resolve(&env(term)), UnderlineCapability::Undercurl);
        }
        for term in ["xterm-256color", "xterm", "vt100", "linux", "dumb", "", "screen-256color"] {
            assert_eq!(UnderlineCapability::resolve(&env(term)), UnderlineCapability::Underline);
        }
    }

    #[test]
    fn term_program_rescues_a_plain_term_but_never_a_multiplexer() {
        // Teej's CP-1 ruling. iTerm2 and VS Code both ship
        // `TERM=xterm-256color` and both support `4:3`, so consulting the
        // program only after the plain-family rule made those two entries
        // unreachable and degraded them for nothing. Cased as the real
        // variable is, to cover the lowercasing too.
        for program in ["iTerm.app", "vscode", "ghostty"] {
            let capable = TerminalEnv {
                term: Some("xterm-256color"),
                term_program: Some(program),
                ..TerminalEnv::default()
            };
            assert_eq!(UnderlineCapability::resolve(&capable), UnderlineCapability::Undercurl);
        }

        // tmux inside iTerm2 reports both, and the multiplexer still wins:
        // passthrough needs tmux >= 3.4 plus a `terminal-features` entry, and
        // assuming it paints garbage in every unconfigured session.
        let multiplexed = TerminalEnv {
            term: Some("screen-256color"),
            term_program: Some("iTerm.app"),
            ..TerminalEnv::default()
        };
        assert_eq!(UnderlineCapability::resolve(&multiplexed), UnderlineCapability::Underline);

        // A degradation capture now forces the path explicitly instead of
        // leaning on TERM — what the forced-underline tape already does.
        let forced = TerminalEnv {
            phosphor_undercurl: Some("0"),
            term: Some("xterm-256color"),
            term_program: Some("iTerm.app"),
            ..TerminalEnv::default()
        };
        assert_eq!(UnderlineCapability::resolve(&forced), UnderlineCapability::Underline);
    }

    #[test]
    fn no_color_and_the_override() {
        let no_color =
            TerminalEnv { no_color: Some("1"), ..env("xterm-kitty") };
        assert_eq!(UnderlineCapability::resolve(&no_color), UnderlineCapability::Underline);

        let forced_on =
            TerminalEnv { phosphor_undercurl: Some("1"), ..no_color };
        assert_eq!(UnderlineCapability::resolve(&forced_on), UnderlineCapability::Undercurl);

        let forced_off =
            TerminalEnv { phosphor_undercurl: Some("never"), ..env("xterm-kitty") };
        assert_eq!(UnderlineCapability::resolve(&forced_off), UnderlineCapability::Underline);
    }

    #[test]
    fn the_sgr_pair_restores_what_the_backend_believes() {
        let curl = CellStyle::undercurl(Color::Rgb(0x2a, 0x5c, 0x44));
        assert_eq!(
            decorate_symbol("x", curl.color),
            "\u{1b}[4:3m\u{1b}[58;2;42;92;68mx\u{1b}[59m\u{1b}[4m"
        );
        assert_eq!(decorate_symbol("x", None), "\u{1b}[4:3mx\u{1b}[4m");
    }

    #[test]
    fn the_later_span_wins() {
        let spans = [
            StyledSpan::undercurl(0, 10, Color::Rgb(1, 1, 1)),
            StyledSpan::undercurl(4, 6, Color::Rgb(2, 2, 2)),
        ];
        assert_eq!(span_at(&spans, 5).and_then(|s| s.color), Some(Color::Rgb(2, 2, 2)));
        assert_eq!(span_at(&spans, 8).and_then(|s| s.color), Some(Color::Rgb(1, 1, 1)));
        assert_eq!(span_at(&spans, 10), None);
    }
}
