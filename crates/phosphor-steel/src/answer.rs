//! What an [`Outcome`] says — the line after `6b`'s `⇒`, written once.
//!
//! `T023`'s acceptance criterion is *"`phosphor --eval` and the REPL return
//! identical results for the same expression"*, and `T022`'s is that `6b`
//! reproduces. Those two are the same sentence read from two ends, so the
//! rendering lives here rather than at each front-end:
//!
//! ```text
//!   --eval EXPR ─┐                                    ┌─→ door.rs   prints `#ok · persisted to init.scm`
//!                ├─→ Runtime::evaluate ─→ Outcome ─→ answered ─┤
//!   the REPL  ───┘                                    └─→ repl.rs   draws `⇒ ` + head + ` · ` + note
//! ```
//!
//! One evaluator ([`crate::runtime::Runtime::evaluate`]) and one renderer, so
//! *"the REPL and `--eval` agree"* is a property of the arrangement rather than
//! a thing either side has to remember.
//!
//! # Two halves, not one string
//!
//! `6b` draws `⇒ #ok · persisted to init.scm` in two colours — the answer in
//! prose, the note in meta-grey (TUI Mockups.dc.html:499). A single
//! pre-formatted line could not be toned, so [`answered`] keeps them apart and
//! [`line()`](fn@line) is the join the CLI door wants, where there is one colour anyway.
//!
//! # Where this belongs, eventually
//!
//! [`value`] is a scheme writer for `phosphor_core::value::Value` and has
//! nothing to do with Steel; its home is `phosphor-core::value`, beside the type
//! it prints. It sits here because `phosphor-steel` is the highest crate both
//! front-ends already share. **Flagged, not folded in** — moving it is a
//! `phosphor-core` edit.
//!
//! Owned by `spine`.

use phosphor_core::action::{Outcome, Refusal};
use phosphor_core::value::Value;

use crate::registry::{OK, REFUSED};

/// The two halves of the line drawn after `⇒`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// The answer itself: `#ok`, `#watch-3`, `(4 #t)`, `#refused`.
    pub head: String,
    /// The receipt's one line, if it had one: `persisted to init.scm`.
    pub note: Option<String>,
}

impl Answered {
    /// The two halves joined by the midline dot — §6, *"midline dot only
    /// inside a fact"*.
    #[must_use]
    pub fn line(&self) -> String {
        match &self.note {
            Some(note) => format!("{} · {note}", self.head),
            None => self.head.clone(),
        }
    }
}

/// What the editor said, in the shape both front-ends draw.
#[must_use]
pub fn answered(outcome: &Outcome) -> Answered {
    match outcome {
        Outcome::Done(receipt) => Answered {
            head: match &receipt.value {
                // An Action with no value of its own answers `#ok`, the same
                // symbol the Steel door hands back (`registry::OK`).
                Value::Null => OK.to_owned(),
                value => self::value(value),
            },
            note: receipt.note.clone(),
        },
        Outcome::Refused(refusal) => Answered {
            head: REFUSED.to_owned(),
            note: Some(why(refusal)),
        },
    }
}

/// One line, as the CLI door prints it.
#[must_use]
pub fn line(outcome: &Outcome) -> String {
    answered(outcome).line()
}

/// Why an Action did not happen, in the product's voice.
///
/// Design Language §6: lowercase, telegraphic, factual; em dash for cause. The
/// same text reaches a refused Action's `(#refused "…")` value in scheme
/// ([`crate::registry`]), the REPL's `⇒` line and the CLI door's stdout —
/// **written once, here**, because three surfaces phrasing one enum three ways
/// is how a vocabulary stops being one vocabulary.
#[must_use]
pub fn why(refusal: &Refusal) -> String {
    match refusal {
        Refusal::NotYetImplemented { task } => format!("not built yet — {task} builds it"),
        Refusal::FocusRelativeTargetOverMcp => {
            "an agent has no cursor — name the target".to_owned()
        }
        Refusal::DoorDenied { door } => {
            format!(
                "the {} door refuses this — open it in init.scm",
                door.as_str()
            )
        }
        Refusal::NoRepository => "no repository here".to_owned(),
        Refusal::NoSuchTarget => "no such target — it may have moved on".to_owned(),
        Refusal::WouldLoseWork => "unsaved work — force it or save first".to_owned(),
        Refusal::Declined { reason } => reason.clone(),
    }
}

/// A [`Value`] written the way the vocabulary is read: scheme.
///
/// The door is a shell interface to a Scheme editor and the REPL is that Scheme,
/// so `(a b c)` and `#t` are the spellings a caller already has from `6b`.
///
/// **Text beginning with `#` prints bare.** `6b`'s own result line is
/// `⇒ (#region 4 fn:use  #region 6-10 struct:RetryPolicy …)`
/// (TUI Mockups.dc.html:494) — the `#`-sigil is the mockups' spelling for an
/// opaque handle, and quoting it would draw `("#region 4 fn:use" …)`. It is also
/// what makes a refused Action's value read as `(#refused "…")` rather than as
/// two strings: `convert::from_steel` narrows a scheme symbol to text
/// deliberately, and this is where that decode is read back.
#[must_use]
pub fn value(value: &Value) -> String {
    match value {
        Value::Null => "#nil".to_owned(),
        Value::Bool(true) => "#t".to_owned(),
        Value::Bool(false) => "#f".to_owned(),
        Value::Int(number) => number.to_string(),
        Value::Text(text) if text.starts_with('#') => text.clone(),
        // Quoted, which is what makes a one-element list distinguishable from a
        // bare word in a pipe.
        Value::Text(text) => format!("{text:?}"),
        Value::List(items) => {
            let written: Vec<String> = items.iter().map(self::value).collect();
            format!("({})", written.join(" "))
        }
        Value::Record(args) => {
            let written: Vec<String> = args
                .iter()
                .map(|(name, field)| format!("{name} {}", self::value(field)))
                .collect();
            format!("(#record {})", written.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::action::Receipt;
    use phosphor_core::value::Args;

    #[test]
    fn a_receipt_renders_as_6b_draws_it() {
        let ok = Outcome::Done(Receipt {
            capability: "set-keybinding",
            value: Value::Null,
            note: Some("persisted to init.scm".to_owned()),
        });
        assert_eq!(
            answered(&ok),
            Answered {
                head: "#ok".to_owned(),
                note: Some("persisted to init.scm".to_owned()),
            }
        );
        assert_eq!(line(&ok), "#ok · persisted to init.scm");

        // `6b` line 503: `⇒ #watch-3 · streaming from next run`.
        let watch = Outcome::Done(Receipt {
            capability: "place-watch",
            value: Value::Text("#watch-3".to_owned()),
            note: Some("streaming from next run".to_owned()),
        });
        assert_eq!(line(&watch), "#watch-3 · streaming from next run");

        assert_eq!(line(&Outcome::Done(Receipt::ok("close-float"))), "#ok");
    }

    #[test]
    fn a_refusal_names_itself_and_then_says_why() {
        let refused = Outcome::Refused(Refusal::NotYetImplemented { task: "T041" });
        assert_eq!(line(&refused), "#refused · not built yet — T041 builds it");
    }

    #[test]
    fn a_handle_prints_bare_and_a_word_prints_quoted() {
        // `6b` line 494 draws `(#region 4 fn:use  …)` — handles unquoted inside
        // a list, which is also how `(#refused "…")` reads back.
        assert_eq!(
            value(&Value::List(vec![
                Value::Text("#region 4 fn:use".to_owned()),
                Value::Text("claude".to_owned()),
            ])),
            "(#region 4 fn:use \"claude\")"
        );
        assert_eq!(
            value(&Value::List(vec![Value::Int(4), Value::Bool(true)])),
            "(4 #t)"
        );
        assert_eq!(
            value(&Value::Record(Args::new().with("id", Value::Int(3)))),
            "(#record id 3)"
        );
    }
}
