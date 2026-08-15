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

/// What an evaluation that ran and then raised answers: `#raised · why`.
///
/// **Not beside [`OK`] and [`REFUSED`] in [`crate::registry`], and the reason is
/// the rule that file states:** *refusals are values; errors are errors.* Those
/// two are symbols scheme receives and can branch on. A raise never becomes a
/// value — it unwinds — so `#raised` exists only where an [`Outcome`] is being
/// *drawn*, which is here. `T100`.
pub const RAISED: &str = "#raised";

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
        // A refusal that came back through scheme rather than through the
        // `Outcome`. `6b` drew `⇒ (#refused "not built yet — T077 builds it")`
        // for `(watch-place …)` while the three lines above it drew
        // `⇒ #raised · …`, because the door turns a refused Action into a
        // *value* and the last value is what a REPL prints. Both halves are
        // right and the join was not: a reader gets a receipt here, in the one
        // voice §6 allows, and scheme still receives the list it can branch on.
        Outcome::Done(receipt) if refused(&receipt.value).is_some() => Answered {
            head: REFUSED.to_owned(),
            note: refused(&receipt.value).map(str::to_owned),
        },
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
        // `T100`. A raise is neither of the two above and now says so: the head
        // is its own sigil, and the sentence is the enum's, exactly as a
        // refusal's is.
        Outcome::Raised(raised) => Answered {
            head: RAISED.to_owned(),
            note: Some(raised.why()),
        },
    }
}

/// The reason inside the door's own `(#refused "…")`, or [`None`].
///
/// Exactly the two-element shape [`REFUSED`] documents and
/// `registry::outcome_value` builds — a longer list, or a list that merely
/// starts with the word, is somebody's data and is printed as data.
fn refused(value: &Value) -> Option<&str> {
    let Value::List(items) = value else {
        return None;
    };
    let [Value::Text(head), Value::Text(reason)] = items.as_slice() else {
        return None;
    };
    (head == REFUSED).then_some(reason.as_str())
}

/// One line, as the CLI door prints it.
#[must_use]
pub fn line(outcome: &Outcome) -> String {
    answered(outcome).line()
}

/// The one line to show when an [`Outcome`] is not a success, or [`None`].
///
/// Design Language §5: *trouble on the statusline never blocks editing* — so
/// the surfaces that reduce an outcome to a notice want a sentence or nothing,
/// never a receipt. The binary has three of them (an ex line, a producer's
/// posted Action, a keymap form arriving from the CLI or MCP door) and each one
/// carried its own two-arm `match` on [`Outcome::Refused`] alone.
///
/// **`T100` is why this exists rather than a fourth arm in each.** Adding
/// [`Outcome::Raised`] made two of those three a compile error and left the
/// third — an `if let Outcome::Refused(…)` — compiling and silently dropping a
/// raise on the floor. One function is what stops the next case doing that
/// again: it cannot be exhaustive in one place and lossy in another.
#[must_use]
pub fn trouble(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Done(_) => None,
        Outcome::Refused(refusal) => Some(why(refusal)),
        Outcome::Raised(raised) => Some(raised.why()),
    }
}

/// Why an Action did not happen, in the product's voice.
///
/// **A delegate, and deliberately branchless.** The phrasing is
/// [`Refusal::why`], hung on the enum it describes so that a second voice is a
/// `match` somebody has to write on purpose — `OPEN-QUESTIONS.md` §9, where
/// this function and `door.rs`'s own copy said *"not built yet — `T041` builds
/// it"* and *"`T041` builds this"* about one value. `T100` collapsed them.
///
/// It stays as a name because the scheme door ([`crate::registry`]), the REPL
/// and the ex line's diagnostic all reach the voice through it, and repointing
/// those is an edit to files `T100` did not hold. Nothing is lost by the extra
/// hop: there is one implementation, and it is not here. **Do not grow a
/// `match` in this function** — that is precisely the defect that was fixed.
#[must_use]
pub fn why(refusal: &Refusal) -> String {
    refusal.why()
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

    /// The same refusal arriving the other way round — as the *value* the
    /// Steel door hands back — reads identically.
    ///
    /// `6b` drew `⇒ (#refused "not built yet — T077 builds it")` for
    /// `(watch-place …)` while the three lines above it drew `⇒ #raised · …`,
    /// because a refused Action is a value at the door and the last value is
    /// what a REPL prints. Both halves were right; the join put Steel's shape
    /// in front of a reader.
    #[test]
    fn a_refusal_that_came_back_as_a_value_reads_as_a_receipt() {
        let door = |value| {
            Outcome::Done(Receipt {
                capability: "eval",
                value,
                note: None,
            })
        };
        let refusal = Value::List(vec![
            Value::Text("#refused".to_owned()),
            Value::Text("not built yet — T077 builds it".to_owned()),
        ]);
        assert_eq!(
            line(&door(refusal)),
            "#refused · not built yet — T077 builds it"
        );

        // Somebody's own data is data. A two-element list whose head is not the
        // door's symbol, and a list of the right head but the wrong length, are
        // both printed as scheme — otherwise `answered` would be guessing at
        // any list that mentioned the word.
        assert_eq!(
            line(&door(Value::List(vec![
                Value::Text("#region 4".to_owned()),
                Value::Text("fn:use".to_owned()),
            ]))),
            "(#region 4 \"fn:use\")"
        );
        assert_eq!(
            line(&door(Value::List(vec![Value::Text("#refused".to_owned())]))),
            "(#refused)"
        );
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
