//! The boot float — what a broken `init.scm` looks like.
//!
//! `T021`'s acceptance criterion is not that the editor survives; it is that
//! the editor survives **and the error is legible**. So this module turns a
//! [`BootReport`] into a [`Float`] carrying the three things you need to act:
//! *which file*, *which line*, and *what Steel said* — with the offending
//! source line underneath it, so the float answers the question instead of
//! sending you to go and look.
//!
//! # It composes; it does not draw
//!
//! The float is `phosphor_core::view::Float` — the protocol, not the widget.
//! `T084` already built the chrome primitive (`phosphor-ui/src/float.rs`) and
//! `T079` interprets this tree into it. Nothing here knows a colour, a border
//! or a width; a second float implementation is exactly what Q12 exists to
//! prevent.
//!
//! # Mood
//!
//! [`Mood::Informational`]. Design Language §4 gives the border three meanings —
//! informational, needs-you, passive — and needs-you is the amber the language
//! reserves for *"questions and permission asks"*, where digits `[1]`-`[n]`
//! answer claude. This float asks nothing and has no options; it reports. The
//! trouble is in the *tone* of the failing rows ([`Tone::Trouble`]), which is
//! where §2 puts failure.
//!
//! **Flagged, not folded in:** there is no float mood for *trouble*, and this is
//! the first surface that would want one. Adding a fourth is a Design Language
//! edit, so it goes to Teej.
//!
//! Owned by `spine`.

use phosphor_core::request::KeySeq;
use phosphor_core::view::{
    Child, Density, Float, FloatHeader, KeyHint, Mood, Node, Run, SpanRow, Tone,
};

use crate::boot::{BootFault, BootReport};

/// How many faults the float lists before it summarises the rest.
///
/// A boot that produced forty faults has one problem, not forty, and a float
/// taller than the screen teaches nothing. The first few are the ones that
/// caused the others.
pub const FAULTS_SHOWN: usize = 6;

/// The boot float, or [`None`] when the boot ran clean.
///
/// A clean boot draws nothing at all — *"cold start invites, never nags"*
/// (TUI Mockups, turn 7). The float exists only when there is something in it.
#[must_use]
pub fn boot_float(report: &BootReport) -> Option<Float> {
    if report.is_clean() {
        return None;
    }

    let mut rows = Vec::new();
    for fault in report.faults.iter().take(FAULTS_SHOWN) {
        if !rows.is_empty() {
            rows.push(SpanRow::default());
        }
        rows.extend(fault_rows(fault));
    }

    let hidden = report.faults.len().saturating_sub(FAULTS_SHOWN);
    if hidden > 0 {
        rows.push(SpanRow::default());
        rows.push(row(vec![Run::new(
            &format!("{hidden} more · :repl to read them"),
            Tone::Meta,
        )]));
    }

    rows.push(SpanRow::default());
    rows.push(row(vec![Run::new(&survival(report), Tone::Meta)]));

    Some(Float {
        mood: Mood::Informational,
        header: Some(FloatHeader {
            left: "◆ steel · boot".to_owned(),
            right: Some(count(report.faults.len(), "fault")),
        }),
        body: Child::new(Node::Spans { rows }),
        footer: Some(Child::new(Node::KeyHints {
            // §4: *"every legal key, always visible"*; §6: primary action
            // first, escape last.
            density: Density::Footer,
            hints: vec![
                hint(":repl", "open the repl"),
                hint(":reload-runtime", "run the boot again"),
                hint("esc", "close"),
            ],
        })),
    })
}

/// One fault, as two or three rows.
fn fault_rows(fault: &BootFault) -> Vec<SpanRow> {
    // `init.scm:12:3 · bad syntax` — §6: the midline dot goes inside a fact.
    let mut rows = vec![row(vec![
        Run::new(&fault.place(), Tone::Trouble),
        Run::new(" · ", Tone::Meta),
        Run::new(fault.label, Tone::Meta),
    ])];

    rows.push(row(vec![Run::new(
        &format!("  {}", fault.message),
        Tone::Text,
    )]));

    if let Some(line) = &fault.source_line {
        rows.push(row(vec![Run::new(
            &format!("  {}", line.trim_end()),
            Tone::Meta,
        )]));
    }

    rows
}

/// The line that makes the float honest: the editor is up, and by how much.
fn survival(report: &BootReport) -> String {
    format!(
        "{} of {} ran · the editor is up",
        report.forms_ran(),
        count(report.forms_found(), "form"),
    )
}

/// `1 fault` / `3 faults` — §6: a number beats an adjective.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn row(runs: Vec<Run>) -> SpanRow {
    SpanRow { runs, tint: None }
}

fn hint(key: &str, verb: &str) -> KeyHint {
    KeyHint {
        key: KeySeq(key.to_owned()),
        verb: verb.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::boot::BootUnit;
    use phosphor_core::request::Position;

    fn fault(line: u32, message: &str) -> BootFault {
        BootFault {
            file: PathBuf::from("init.scm"),
            at: Some(Position { line, column: 3 }),
            label: "bad syntax",
            message: message.to_owned(),
            source_line: Some("(define oops".to_owned()),
        }
    }

    fn report(faults: Vec<BootFault>) -> BootReport {
        BootReport {
            root: Some(PathBuf::from("runtime")),
            units: vec![BootUnit {
                file: PathBuf::from("init.scm"),
                forms: 4,
                ran: 3,
            }],
            faults,
        }
    }

    /// Everything the float says, flattened — what a reader would see.
    fn text(float: &Float) -> String {
        let Node::Spans { rows } = float.body.node() else {
            panic!("the boot float's body is the spans hatch");
        };
        rows.iter()
            .map(|row| {
                row.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_clean_boot_draws_nothing() {
        assert!(boot_float(&report(Vec::new())).is_none());
    }

    #[test]
    fn the_float_carries_the_file_the_line_and_the_message() {
        let float = boot_float(&report(vec![fault(12, "unexpected end of file")]))
            .expect("a fault opens the float");
        let body = text(&float);
        assert!(body.contains("init.scm:12:3"), "{body}");
        assert!(body.contains("unexpected end of file"), "{body}");
        assert!(body.contains("bad syntax"), "{body}");
        assert!(
            body.contains("(define oops"),
            "the offending line is shown\n{body}"
        );
    }

    #[test]
    fn the_float_says_the_editor_is_up() {
        let float = boot_float(&report(vec![fault(2, "boom")])).expect("a fault opens the float");
        assert!(text(&float).contains("3 of 4 forms ran · the editor is up"));
    }

    #[test]
    fn the_header_counts_the_faults_and_the_footer_teaches_the_keys() {
        let float =
            boot_float(&report(vec![fault(1, "a"), fault(2, "b")])).expect("two faults, one float");
        let header = float.header.expect("the boot float has a header");
        assert_eq!(header.left, "◆ steel · boot");
        assert_eq!(header.right.as_deref(), Some("2 faults"));

        let Some(footer) = &float.footer else {
            panic!("only the passive mood may go without a footer (§4)");
        };
        let Node::KeyHints { density, hints } = footer.node() else {
            panic!("the footer is a keymap surface");
        };
        assert_eq!(*density, Density::Footer);
        assert_eq!(hints.last().expect("at least one hint").key.0, "esc");
    }

    #[test]
    fn a_long_list_of_faults_is_summarised_rather_than_scrolled() {
        let faults = (1..=FAULTS_SHOWN + 3)
            .map(|line| fault(u32::try_from(line).expect("small"), "boom"))
            .collect();
        let float = boot_float(&report(faults)).expect("faults open the float");
        assert!(
            text(&float).contains("3 more · :repl to read them"),
            "{}",
            text(&float)
        );
    }

    #[test]
    fn the_mood_is_informational_because_it_asks_nothing() {
        let float = boot_float(&report(vec![fault(1, "boom")])).expect("a fault opens the float");
        assert_eq!(float.mood, Mood::Informational);
    }
}
