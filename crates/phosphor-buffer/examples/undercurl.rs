//! `T085`'s fixture call site — one styled span, on whatever terminal you run it in.
//!
//! ```text
//! cargo run -p phosphor-buffer --example undercurl          # detect
//! PHOSPHOR_UNDERCURL=1 cargo run -p phosphor-buffer --example undercurl   # force curl
//! PHOSPHOR_UNDERCURL=0 cargo run -p phosphor-buffer --example undercurl   # force the fallback
//! TERM=xterm-256color  cargo run -p phosphor-buffer --example undercurl   # V009's degradation env
//! NO_COLOR=1           cargo run -p phosphor-buffer --example undercurl   # V009's other one
//! ```
//!
//! Any key quits. This is the thing to point VHS at for `V002`'s open question
//! — *does undercurl survive capture* — and the thing to open on each of
//! `CP-1`'s four terminals.
//!
//! **What it is not.** Not a consumer of the capability in the product sense:
//! the real ones are `T040` (diagnostics) and `T068` (anchored regions), and
//! they will read these colours from `Theme` rather than writing them down.
//! Literal hexes are legal here — the no-literal-colours lint is
//! `phosphor-ui`'s — and are labelled with the §3 role they stand in for.

use crossterm::event;
use phosphor_term::Term;
use ratatui_code_editor::editor::Editor;
use ratatui_code_editor::phosphor::cell_style::{CellStyle, StyledSpan, UnderlineCapability};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;

/// `#2a5c44` — Design Language §3, the anchored-region undercurl.
const ANCHOR_UNDERCURL: Color = Color::Rgb(0x2a, 0x5c, 0x44);
/// `#d97b6c` — §3's other one: failure / diagnostic.
const FAILURE_UNDERCURL: Color = Color::Rgb(0xd9, 0x7b, 0x6c);
/// `#59635a` — meta, for the legend row. Not part of the demonstration.
const META: Color = Color::Rgb(0x59, 0x63, 0x5a);

const CODE: &str = "\
fn anchored(region: &Region) -> Anchor {
    region.anchor().expect(\"unanchored\")
}
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut editor = Editor::new("rust", CODE, Vec::new())?;

    // ── the call site ────────────────────────────────────────────────────
    // Both spans ask for an undercurl. Neither knows, or can find out from
    // here, which terminal this is: `set_styled_spans` takes the request and
    // the capability decides what reaches the wire.
    editor.set_styled_spans(vec![
        // `anchored` — an anchored region (§3, "tint + undercurl").
        StyledSpan::undercurl(3, 11, ANCHOR_UNDERCURL),
        // `expect("unanchored")` — a diagnostic.
        StyledSpan::undercurl(58, 79, FAILURE_UNDERCURL),
        // ... and one span that asked for a straight underline outright, so
        // the two treatments are visible side by side on a terminal that has
        // both.
        StyledSpan::new(19, 25, CellStyle::underline(ANCHOR_UNDERCURL)),
    ]);
    // ─────────────────────────────────────────────────────────────────────

    let capability = editor.underline_capability();
    let legend = format!(
        " undercurl fixture · {} · TERM={} · any key quits ",
        match capability {
            UnderlineCapability::Undercurl => "SGR 4:3 (undercurl)",
            UnderlineCapability::Underline => "SGR 4 (underline — degraded)",
        },
        std::env::var("TERM").unwrap_or_else(|_| "<unset>".into()),
    );

    let mut term = Term::new()?;
    term.draw(|frame| {
        let area = frame.area();
        let code_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let legend_area = Rect {
            y: area.bottom().saturating_sub(1),
            height: 1,
            ..area
        };
        frame.render_widget(&editor, code_area);
        Line::from(legend.as_str())
            .style(Style::default().fg(META))
            .render(legend_area, frame.buffer_mut());
    })?;

    loop {
        if let event::Event::Key(_) = event::read()? {
            break;
        }
    }
    term.restore()?;
    Ok(())
}
