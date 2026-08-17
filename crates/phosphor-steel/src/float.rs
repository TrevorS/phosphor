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

/// Whether the ex line this float's footer teaches actually exists.
///
/// §4 asks a footer for *"every legal key, always visible"*, and the reading
/// that matters is **legal**. `:repl` and `:reload-runtime` are two ex commands,
/// `:` is bound in `runtime/keymaps.scm`, and the one state this float is
/// guaranteed to open in — nothing loaded an editor layer at all (`OPEN-QUESTIONS.md`
/// §34) — is the state where that file did not run. Driven on a pty in exactly
/// that state: `esc` closed the float, because Rust handles it; pressing `:`
/// changed nothing on the frame. A footer teaching two keys that cannot be
/// typed is worse than a short one, and it is worst in front of the reader who
/// has no other way to find out what is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExLine {
    /// An editor layer loaded, so `:` reaches the command table.
    Bound,
    /// Nothing loaded one. `esc` is the only key the footer can honestly name.
    Unbound,
}

/// The boot float, or [`None`] when the boot ran clean.
///
/// A clean boot draws nothing at all — *"cold start invites, never nags"*
/// (TUI Mockups, turn 7). The float exists only when there is something in it.
#[must_use]
pub fn boot_float(report: &BootReport, ex: ExLine) -> Option<Float> {
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
        // The same rule as the footer: an ex command is only an instruction
        // where `:` is bound. Without it the count is all there is to say.
        let rest = match ex {
            ExLine::Bound => format!("{hidden} more · :repl to read them"),
            ExLine::Unbound => format!("{hidden} more"),
        };
        rows.push(row(vec![Run::new(&rest, Tone::Meta)]));
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
            // first, escape last. See [`ExLine`] for why the first two are not
            // always legal.
            density: Density::Footer,
            hints: match ex {
                ExLine::Bound => vec![
                    hint(":repl", "open the repl"),
                    hint(":reload-runtime", "run the boot again"),
                    hint("esc", "close"),
                ],
                ExLine::Unbound => vec![hint("esc", "close")],
            },
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

// ---------------------------------------------------------------------------
// Surfaces — the registry `open-float` names (`T093`, §43)
// ---------------------------------------------------------------------------

/// The prefix a registered surface's procedure is bound under.
///
/// One namespace, and it is the same shape as [`crate::status::COMPOSER`]'s:
/// the editor layer owns a global and the host only calls it. A surface is
/// `phosphor/float-surface/<id>`, so `(open-float "arch")` reaches
/// `phosphor/float-surface/arch`.
pub const SURFACE_PREFIX: &str = "phosphor/float-surface/";

/// The global a surface's procedure lives at.
#[must_use]
pub fn surface_global(id: &str) -> String {
    format!("{SURFACE_PREFIX}{id}")
}

/// Whether an id may be built into a global name.
///
/// **A door supplies this string**, and it is interpolated into a `define`
/// form, so it is validated rather than trusted: without this, an id of
/// `x) (displayln "owned"` is scheme injection through a capability rated
/// `Allow` for MCP. Letters, digits, `-` and `_`, and non-empty — which is
/// every id any mockup writes (`arch`, `unseen`, `files`).
#[must_use]
pub fn valid_surface_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The form `define-float-surface` evaluates.
///
/// The body is *"a procedure of one argument"* — the args hash `open-float`
/// carries — and binding it to a global is what makes a redefinition at the
/// REPL take effect without a restart, the same property `define-language`
/// has and for the same reason.
#[must_use]
pub fn define_form(id: &str, body: &str) -> String {
    format!("(define {} {body})", surface_global(id))
}

/// Calls a registered surface and decodes what it answered.
///
/// The whole of `open-float`'s Steel half, and deliberately the same three
/// steps as [`crate::status::compose`]: check the global, call it, decode. A
/// surface that raises leaves the caller to decide what stays on screen —
/// which for a float is *nothing opens*, unlike the statusline where the last
/// good line is kept.
///
/// # Errors
///
/// [`SurfaceError`], one variant per way it can fail to produce a float.
pub fn surface(
    runtime: &mut crate::runtime::Runtime,
    id: &str,
    args: &phosphor_core::value::Value,
) -> Result<Float, SurfaceError> {
    let global = surface_global(id);
    if runtime.global(&global).is_err() {
        return Err(SurfaceError::Unknown(id.to_owned()));
    }
    let answered = runtime
        .call(&global, vec![crate::convert::to_steel(args)])
        .map_err(|error| SurfaceError::Raised(error.to_string()))?;
    crate::view::float(&answered).map_err(SurfaceError::NotAFloat)
}

/// Why a surface did not produce a float.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceError {
    /// The id is not one an editor layer has defined.
    Unknown(String),
    /// It is defined and raised. Carries Steel's own text.
    Raised(String),
    /// It answered something that is not a float.
    NotAFloat(crate::view::ViewError),
    /// The id could not be a global name — see [`valid_surface_id`].
    BadId(String),
}

impl core::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // §6's voice: say what to do, not what went wrong.
            Self::Unknown(id) => {
                write!(
                    f,
                    "no float surface `{id}` — (define-float-surface! …) makes one"
                )
            }
            Self::Raised(why) => write!(f, "{why}"),
            Self::NotAFloat(error) => write!(f, "{error}"),
            Self::BadId(id) => write!(f, "`{id}` is not a surface name — letters, digits, - and _"),
        }
    }
}

impl std::error::Error for SurfaceError {}

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
        assert!(boot_float(&report(Vec::new()), ExLine::Bound).is_none());
    }

    #[test]
    fn the_float_carries_the_file_the_line_and_the_message() {
        let float = boot_float(
            &report(vec![fault(12, "unexpected end of file")]),
            ExLine::Bound,
        )
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
        let float = boot_float(&report(vec![fault(2, "boom")]), ExLine::Bound)
            .expect("a fault opens the float");
        assert!(text(&float).contains("3 of 4 forms ran · the editor is up"));
    }

    #[test]
    fn the_header_counts_the_faults_and_the_footer_teaches_the_keys() {
        let float = boot_float(&report(vec![fault(1, "a"), fault(2, "b")]), ExLine::Bound)
            .expect("two faults, one float");
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

    /// **A footer may not teach a key that does not exist** — the state
    /// `OPEN-QUESTIONS.md` §34's float is guaranteed to open in.
    ///
    /// Nothing loaded an editor layer, so `:` is unbound and the two ex
    /// commands beside `esc` are instructions a reader cannot carry out. Driven
    /// on a pty in exactly that state before this existed: `esc` closed the
    /// float, `:` changed nothing on the frame.
    ///
    /// The `Bound` half is asserted too, in the same test, because the bug this
    /// guards against on *that* side is silently teaching nothing: a footer
    /// trimmed to `esc` for every boot fault would pass a test that only looked
    /// at the unbound case.
    #[test]
    fn a_boot_with_no_editor_layer_teaches_only_the_key_rust_handles() {
        let hints = |ex| {
            let float = boot_float(&report(vec![fault(1, "boom")]), ex).expect("a fault, a float");
            let Node::KeyHints { hints, .. } = float.footer.expect("a footer").node().clone()
            else {
                panic!("the footer is a keymap surface");
            };
            hints
                .iter()
                .map(|hint| hint.key.0.clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(hints(ExLine::Unbound), vec!["esc".to_owned()]);
        assert_eq!(
            hints(ExLine::Bound),
            vec![
                ":repl".to_owned(),
                ":reload-runtime".to_owned(),
                "esc".to_owned()
            ]
        );
    }

    #[test]
    fn a_long_list_of_faults_is_summarised_rather_than_scrolled() {
        let faults: Vec<BootFault> = (1..=FAULTS_SHOWN + 3)
            .map(|line| fault(u32::try_from(line).expect("small"), "boom"))
            .collect();
        let float =
            boot_float(&report(faults.clone()), ExLine::Bound).expect("faults open the float");
        assert!(
            text(&float).contains("3 more · :repl to read them"),
            "{}",
            text(&float)
        );

        // The same row without an ex line to send the reader to. The count is
        // still worth saying; the instruction is not.
        let float = boot_float(&report(faults), ExLine::Unbound).expect("faults open the float");
        let body = text(&float);
        assert!(body.contains("3 more"), "{body}");
        assert!(!body.contains(":repl"), "{body}");
    }

    #[test]
    fn the_mood_is_informational_because_it_asks_nothing() {
        let float = boot_float(&report(vec![fault(1, "boom")]), ExLine::Bound)
            .expect("a fault opens the float");
        assert_eq!(float.mood, Mood::Informational);
    }
}
