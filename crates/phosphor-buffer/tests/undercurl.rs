//! `T085` — undercurl, and the underline it degrades to, from one call site.
//!
//! The capability lives in the fork (`vendor/ratatui-code-editor`, patch 5 in
//! its `VENDOR.md`); this is the proof that runs in *our* CI. The fork is
//! excluded from the workspace, so its own `#[cfg(test)]` module is only
//! reachable by a standalone `cargo test --manifest-path vendor/…` — a build
//! that compiles sixteen tree-sitter grammars and that nothing gates on. The
//! assertions that matter are therefore mirrored here, against the same
//! `grammars-phosphor` build of the fork phosphor actually ships.
//!
//! **What "one call site" means, and why the test does not have one.** The
//! consumer calls [`Editor::set_styled_spans`] once, with
//! [`StyledSpan::undercurl`], and never learns which terminal it is on. This
//! test forces the *terminal* — through `set_underline_capability`, which is an
//! app-layer concern, not a consumer one — and renders that single call site
//! twice. The two renders below are the two terminals in `CP-1`'s matrix.

use phosphor_buffer as _; // the crate under whose manifest this test builds

use ratatui_code_editor::editor::Editor;
use ratatui_code_editor::phosphor::cell_style::{
    StyledSpan, TerminalEnv, UnderlineCapability, decorate_symbol,
};
use ratatui_core::buffer::{Buffer, CellDiffOption, CellWidth};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};
use ratatui_core::widgets::Widget;
use std::num::NonZeroU16;

const CODE: &str = "fn main() {}\n";
/// `main` — chars 3..7 of [`CODE`].
const SPAN: (usize, usize) = (3, 7);
/// `#2a5c44` — Design Language §3, the anchored-region undercurl.
const ANCHOR_UNDERCURL: Color = Color::Rgb(0x2a, 0x5c, 0x44);

/// The single call site, rendered on one terminal.
fn render_on(capability: UnderlineCapability) -> (Buffer, u16) {
    let mut editor = Editor::new("rust", CODE, Vec::new()).expect("editor");

    // ── the call site ────────────────────────────────────────────────────
    editor.set_styled_spans(vec![StyledSpan::undercurl(
        SPAN.0,
        SPAN.1,
        ANCHOR_UNDERCURL,
    )]);
    // ─────────────────────────────────────────────────────────────────────

    // The terminal, which the call site above is not allowed to care about.
    editor.set_underline_capability(Some(capability));

    let area = Rect::new(0, 0, 40, 3);
    let mut buf = Buffer::empty(area);
    (&editor).render(area, &mut buf);
    (buf, editor.get_line_number_width() as u16)
}

fn cell(buf: &Buffer, x: u16) -> (&str, bool) {
    let cell = buf.cell((x, 0)).expect("cell in bounds");
    (cell.symbol(), cell.modifier.contains(Modifier::UNDERLINED))
}

/// The primary terminal: SGR `4:3` wrapped around each glyph in the span, in
/// `#2a5c44`, plus the straight-underline modifier underneath it.
#[test]
fn undercurl_on_the_primary_terminal() {
    let (buf, text_x) = render_on(UnderlineCapability::Undercurl);

    for (offset, glyph) in "main".chars().enumerate() {
        let x = text_x + SPAN.0 as u16 + offset as u16;
        let (symbol, underlined) = cell(&buf, x);
        assert_eq!(
            symbol,
            format!("\u{1b}[4:3m\u{1b}[58;2;42;92;68m{glyph}\u{1b}[59m\u{1b}[4m"),
            "char {offset} of the span carries the curl-on / colour / colour-off / curl-off pair"
        );
        assert!(underlined, "and the modifier the escape upgrades");
    }
}

/// The escape-carrying cell is still **one column wide**.
///
/// Without this, `Buffer::diff` measures the cell by the display width of its
/// symbol — ~30 columns of escape — and the backend skips that many columns of
/// the line. Caught on a real pty, not in a buffer assertion: the buffer was
/// always right and the wire was missing a third of the row.
#[test]
fn an_escaped_cell_stays_one_column_wide() {
    let (buf, text_x) = render_on(UnderlineCapability::Undercurl);
    let one = NonZeroU16::new(1).expect("1 is not zero");

    for offset in 0..4u16 {
        let cell = buf
            .cell((text_x + SPAN.0 as u16 + offset, 0))
            .expect("cell in bounds");
        assert_eq!(cell.diff_option, CellDiffOption::ForcedWidth(one));
        assert_eq!(cell.cell_width(), 1);
    }
}

/// The degradation terminal: nothing is emitted, and what is left is the
/// straight underline the modifier already asked for.
#[test]
fn underline_on_the_degradation_terminal() {
    let (buf, text_x) = render_on(UnderlineCapability::Underline);

    for (offset, glyph) in "main".chars().enumerate() {
        let x = text_x + SPAN.0 as u16 + offset as u16;
        let (symbol, underlined) = cell(&buf, x);
        assert_eq!(
            symbol,
            glyph.to_string(),
            "no escape reaches a terminal that cannot parse it"
        );
        assert!(underlined, "the underline is the whole treatment here");
    }
}

/// Degradation is the *absence* of an addition: outside the symbol, the two
/// terminals get byte-identical cells. Nothing about the fallback is a second
/// code path that could drift.
#[test]
fn the_two_renders_differ_only_in_the_symbol() {
    let (curl, _) = render_on(UnderlineCapability::Undercurl);
    let (flat, _) = render_on(UnderlineCapability::Underline);

    assert_eq!(curl.area, flat.area);
    for (curled, plain) in curl.content.iter().zip(flat.content.iter()) {
        assert_eq!(curled.fg, plain.fg);
        assert_eq!(curled.bg, plain.bg);
        assert_eq!(curled.modifier, plain.modifier);
        let expected = if curled.symbol() == plain.symbol() {
            plain.symbol().to_string()
        } else {
            decorate_symbol(plain.symbol(), Some(ANCHOR_UNDERCURL))
        };
        assert_eq!(
            curled.symbol(),
            expected,
            "the curled symbol is the plain one with the SGR pair around it"
        );
    }
}

/// A span touches only its own range — the neighbours stay unstyled on both
/// terminals.
#[test]
fn nothing_outside_the_span_is_underlined() {
    for capability in [
        UnderlineCapability::Undercurl,
        UnderlineCapability::Underline,
    ] {
        let (buf, text_x) = render_on(capability);
        for x in [text_x, text_x + 2, text_x + 7, text_x + 9] {
            let (symbol, underlined) = cell(&buf, x);
            assert!(!underlined, "{capability:?}: column {x} is outside 3..7");
            assert!(
                !symbol.contains('\u{1b}'),
                "{capability:?}: no escape outside the span"
            );
        }
    }
}

/// The detection table, at the rows `CP-1` and `V009` land on. `TERM` is the
/// authority; `TERM_PROGRAM` only answers when `TERM` said nothing; `NO_COLOR`
/// degrades; `PHOSPHOR_UNDERCURL` overrides everything.
#[test]
fn the_capability_is_detected_from_the_environment() {
    let env = |term: &'static str| TerminalEnv {
        term: Some(term),
        ..TerminalEnv::default()
    };
    let resolve = UnderlineCapability::resolve;

    // Primary terminals (the matrix's Ghostty / kitty / WezTerm).
    assert_eq!(
        resolve(&env("xterm-ghostty")),
        UnderlineCapability::Undercurl
    );
    assert_eq!(resolve(&env("xterm-kitty")), UnderlineCapability::Undercurl);
    assert_eq!(resolve(&env("wezterm")), UnderlineCapability::Undercurl);

    // Degradation target, multiplexer, and V009's two tape environments.
    assert_eq!(
        resolve(&env("xterm-256color")),
        UnderlineCapability::Underline
    );
    assert_eq!(
        resolve(&env("tmux-256color")),
        UnderlineCapability::Underline
    );
    assert_eq!(
        resolve(&TerminalEnv {
            no_color: Some("1"),
            ..env("xterm-ghostty")
        }),
        UnderlineCapability::Underline,
    );

    // TERM_PROGRAM now outranks a plain TERM family — Teej's CP-1 ruling.
    // iTerm2 and VS Code both ship TERM=xterm-256color and both support 4:3,
    // so the old "TERM is always the authority" order degraded two capable
    // terminals for nothing.
    for program in ["ghostty", "iTerm.app", "vscode"] {
        assert_eq!(
            resolve(&TerminalEnv {
                term_program: Some(program),
                ..env("xterm-256color")
            }),
            UnderlineCapability::Undercurl,
        );
    }

    // A multiplexer still wins over the program: tmux inside iTerm2 reports
    // both, and passthrough of 4:3 has to be configured to work.
    assert_eq!(
        resolve(&TerminalEnv {
            term_program: Some("iTerm.app"),
            ..env("tmux-256color")
        }),
        UnderlineCapability::Underline,
    );

    // A degradation capture therefore forces the path rather than leaning on
    // TERM — which is what tapes/_undercurl-check-forced-underline.tape does,
    // and what V009 should do when it lands.
    assert_eq!(
        resolve(&TerminalEnv {
            phosphor_undercurl: Some("0"),
            term_program: Some("ghostty"),
            ..env("xterm-256color")
        }),
        UnderlineCapability::Underline,
    );

    // The override, which is how CP-1 sees both treatments on one terminal.
    assert_eq!(
        resolve(&TerminalEnv {
            phosphor_undercurl: Some("1"),
            ..env("xterm-256color")
        }),
        UnderlineCapability::Undercurl,
    );
    assert_eq!(
        resolve(&TerminalEnv {
            phosphor_undercurl: Some("0"),
            ..env("xterm-ghostty")
        }),
        UnderlineCapability::Underline,
    );
}
