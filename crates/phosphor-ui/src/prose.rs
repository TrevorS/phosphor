//! Claude's prose, as drawable rows (`T055`).
//!
//! One function, two implementations, chosen by the `markdown` feature. The
//! transcript calls it and does not know which one it got — which is the point
//! of the gate, because `T004`'s Q4 guardrail is that *"the plain-text path
//! must stay readable with the gate off"* and a caller that had to branch would
//! be a second place for the two paths to diverge.
//!
//! # Why the plain path is a fallback and not a degradation
//!
//! The prose an ACP agent streams is markdown — headings, fenced code, `*` and
//! `` ` `` — and it is markdown *whether or not anything renders it*. So the
//! honest gate-off rendering is the source, wrapped: a paragraph reads fine
//! with its asterisks showing, and a fenced block reads fine as indented text.
//! What is *not* fine is what the transcript did before this task, which was
//! neither: [`Row::Prose`](crate::transcript) was one row per `\n` and the row
//! was written with `set_stringn`, so a paragraph longer than the pane was cut
//! at the edge. The comment above it said **"Wrapped, not truncated"** and had
//! said so since `T054`; the code split on newlines and never wrapped
//! anything. The tree wins and the comment was the bug.
//!
//! Wrapping is [`crate::float::wrap_prose`], not a fourth copy of the same
//! loop — the same helper the float bodies use, whose own doc block already
//! named this task: *"rendering markdown properly is the transcript's job at
//! `S6`"*.
//!
//! # What the gate buys
//!
//! The vendored fork (`T004`) parses the source and answers styled
//! [`Line`]s — headings in the primary tone, inline code in the code tones,
//! bullets with real markers, fenced blocks set apart. It wraps to the width it
//! is given, so both paths take the same argument and answer the same type, and
//! the transcript's §11 grouping counts the same rows either way.

use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};

use crate::theme::Theme;

/// Claude's prose as rows, wrapped to `width` cells.
///
/// `width` is the room a row has, not the pane's — the transcript insets by
/// [`PAD_COLS`](crate::transcript::PAD_COLS) before calling. Zero hands back
/// the source unwrapped, which is [`wrap_prose`](crate::float::wrap_prose)'s
/// own rule for a screen with no room: a degenerate width must not loop.
#[cfg(not(feature = "markdown"))]
#[must_use]
pub fn lines(prose: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let source: Vec<String> = prose.lines().map(str::to_owned).collect();
    let tone = Style::new().fg(theme.neutrals.prose);
    crate::float::wrap_prose(&source, width)
        .into_iter()
        .map(|row| Line::from(Span::styled(row, tone)))
        .collect()
}

/// Claude's prose as rows, rendered as markdown and wrapped to `width` cells.
///
/// **The fork's renderer, with phosphor's palette bridged into it.** The
/// `RichTextTheme` trait is fifteen colour slots wide and every one of them is
/// answered from [`Theme`] — Design Language §1 is the only source of a colour
/// in this crate, and a default from the fork would be a sixteenth palette
/// nobody chose.
#[cfg(feature = "markdown")]
#[must_use]
pub fn lines(prose: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    use ratatui_markdown::markdown::MarkdownRenderer;

    // Zero means no room; the fork would divide by it. Same rule as the plain
    // path, for the same reason.
    if width == 0 {
        let tone = Style::new().fg(theme.neutrals.prose);
        return prose
            .lines()
            .map(|row| Line::from(Span::styled(row.to_owned(), tone)))
            .collect();
    }
    // One object parses and renders, and it holds the width — so the wrap the
    // parser measures for is the wrap the renderer emits, with no second number
    // to keep in step.
    let renderer = MarkdownRenderer::new(width as usize);
    renderer.render(&renderer.parse(prose), &Palette { theme })
}

/// [`Theme`] as the fork's `RichTextTheme` (`T055`).
///
/// **A bridge and not a palette.** Every method here reads a field of §1's
/// theme; nothing invents a colour, which is what
/// `scripts/lint-no-literal-colours.sh` is about and what a `Color::White`
/// default from the fork would quietly be.
#[cfg(feature = "markdown")]
struct Palette<'a> {
    theme: &'a Theme,
}

#[cfg(feature = "markdown")]
impl ratatui_markdown::theme::RichTextTheme for Palette<'_> {
    // The fork caches by generation; phosphor's theme is rebuilt rather than
    // mutated, so there is one generation and nothing to invalidate.
    fn generation(&self) -> ratatui_markdown::theme::Generation {
        ratatui_markdown::theme::Generation::default()
    }

    fn get_text_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.prose
    }
    fn get_muted_text_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.meta
    }
    // A heading is claude's, like the seam marker and the prose itself.
    fn get_primary_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.claude
    }
    fn get_popup_selected_background(&self) -> ratatui_core::style::Color {
        self.theme.chrome.statusline
    }
    fn get_border_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.meta
    }
    fn get_focused_border_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.you
    }
    fn get_secondary_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.text
    }
    fn get_info_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.you
    }
    // The JSON slots belong to the fork's tree view, which the transcript does
    // not draw. Answered anyway, from the palette, because the trait requires
    // them and a `Color::Rgb` here would be the literal the lint forbids.
    fn get_json_key_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.you
    }
    fn get_json_string_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.claude
    }
    fn get_json_number_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.transient
    }
    fn get_json_bool_color(&self) -> ratatui_core::style::Color {
        self.theme.actors.transient
    }
    fn get_json_null_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.meta
    }
    fn get_accent_yellow(&self) -> ratatui_core::style::Color {
        self.theme.actors.transient
    }
    fn get_background_color(&self) -> ratatui_core::style::Color {
        self.theme.neutrals.ground
    }
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::theme::Theme;

    /// One paragraph, longer than any pane it will be drawn in.
    const PARAGRAPH: &str = "Adding a RetryPolicy struct and a generic \
        retry_with_backoff helper, then wiring the fetch layer through it so \
        every call site inherits the same jittered exponential schedule.";

    fn width_of(line: &ratatui_core::text::Line<'_>) -> u16 {
        u16::try_from(line.width()).unwrap_or(u16::MAX)
    }

    fn drawn(rows: &[ratatui_core::text::Line<'_>]) -> String {
        rows.iter()
            .map(ratatui_core::text::Line::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// **The task's own guardrail: the plain path stays readable.**
    ///
    /// `T004` gated the fork *"so the plain-text path stays the fallback"*, and
    /// a fallback that cuts a sentence at the pane edge is not one. Every row
    /// has to fit the width it was given, and every word has to survive.
    #[test]
    fn a_paragraph_wraps_to_the_width_it_was_given() {
        let theme = Theme::phosphor_dark();
        let rows = lines(PARAGRAPH, 40, &theme);

        assert!(rows.len() > 1, "a long paragraph is more than one row");
        for row in &rows {
            assert!(
                width_of(row) <= 40,
                "no row overruns the width; row was {:?} at {} cells",
                row.to_string(),
                width_of(row)
            );
        }
        // **Nothing is lost, which is the half a width check cannot see.** A
        // wrapper that truncated every row to 40 would satisfy the loop above
        // and fail this.
        let all = drawn(&rows).replace('\n', " ");
        for word in PARAGRAPH.split_whitespace() {
            assert!(
                all.contains(word),
                "{word:?} survived the wrap; drawn was {all:?}"
            );
        }
    }

    /// A width of zero is a pane with no room, and must not loop or panic —
    /// [`crate::float::wrap_prose`]'s own rule, and the same one on both sides
    /// of the gate.
    #[test]
    fn no_room_is_answered_rather_than_divided_by() {
        let theme = Theme::phosphor_dark();
        let rows = lines(PARAGRAPH, 0, &theme);
        assert_eq!(rows.len(), 1, "one source line in, one row out");
    }

    /// **Behind the gate, markdown is markdown.** The source's `#` and `**` are
    /// syntax and stop being text; without the gate they stay text, and both
    /// are honest renderings of the same stream. This is the half `just hack`
    /// runs and the default gate cannot.
    #[cfg(feature = "markdown")]
    #[test]
    fn a_heading_renders_as_a_heading_rather_than_as_its_source() {
        let theme = Theme::phosphor_dark();
        let rows = lines("# Retry logic\n\nWired **through** fetch.", 40, &theme);
        let all = drawn(&rows);

        assert!(
            all.contains("Retry logic"),
            "the heading's words are drawn; was {all:?}"
        );
        assert!(
            !all.contains("**"),
            "and its emphasis markers are not; was {all:?}"
        );
    }

    /// The same source with the gate **off** keeps its markers, and that is not
    /// a bug — it is the fallback being the source rather than a half-parse.
    #[cfg(not(feature = "markdown"))]
    #[test]
    fn without_the_gate_the_source_is_the_rendering() {
        let theme = Theme::phosphor_dark();
        let rows = lines("# Retry logic\n\nWired **through** fetch.", 40, &theme);
        let all = drawn(&rows);
        assert!(
            all.contains("# Retry logic") && all.contains("**through**"),
            "the plain path draws what arrived; was {all:?}"
        );
    }
}
