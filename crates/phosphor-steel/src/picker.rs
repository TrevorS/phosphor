//! Picker sources, defined in Steel (`T046`).
//!
//! Deliberately the **same three parts** as [`crate::float`]'s surface
//! registry, because they are the same problem: a door names an id, the editor
//! layer owns a procedure under a reserved global, and the host only calls it.
//! A prefix, a validator, a `define` form, and one call site.
//!
//! ```text
//!   define-picker-source  →  (define phosphor/picker-source/<id> <body>)
//!   open-picker "<id>"    →  call it, decode a `spans` node, take its rows
//! ```
//!
//! # Why a source answers a `spans` node rather than a row type of its own
//!
//! `T080`'s escape hatch is already *"styled rows straight from Steel"*, and
//! that is exactly what a picker row is. Inventing a second vocabulary for the
//! same shape would mean a second constructor for scheme to learn, a second
//! decoder here, and two places for *"a row is runs, left to right"* to drift
//! apart.
//!
//! It also keeps the barrier honest. A [`SpanRow`] is `phosphor-core`'s, so
//! this module answers a core type and never names `phosphor-ui` — which
//! `scripts/lint-the-steel-barrier.sh` enforces. The binary is what turns a
//! `SpanRow` into the widget's `RowVm`, at the same seam it turns everything
//! else.
//!
//! # A picker is a live query, and a float is a snapshot
//!
//! `define-picker-source`'s own doc says *"an open picker re-derives from
//! it"*, where `open-float` composes once. That difference is the caller's, not
//! this module's: [`rows`] is a plain call and running it again is what
//! re-deriving means.
//!
//! Owned by `spine`.

use phosphor_core::view::{Node, SpanRow};

/// The prefix a registered source's procedure is bound under.
///
/// One namespace, and the same shape as [`crate::float::SURFACE_PREFIX`] and
/// [`crate::status::COMPOSER`]: the editor layer owns a global and the host
/// only calls it.
pub const SOURCE_PREFIX: &str = "phosphor/picker-source/";

/// The global a source's procedure lives at.
#[must_use]
pub fn source_global(id: &str) -> String {
    format!("{SOURCE_PREFIX}{id}")
}

/// Whether an id may be built into a global name.
///
/// **A door supplies this string** and it is interpolated into a `define`
/// form, so it is validated rather than trusted — `define-picker-source` is
/// rated `Allow` for MCP, so without this an id of `x) (displayln "owned"` is
/// scheme injection. The rule is [`crate::float::valid_surface_id`]'s, and it
/// is *called* rather than copied: two spellings of one validation is how the
/// weaker one gets found by somebody else.
#[must_use]
pub fn valid_source_id(id: &str) -> bool {
    crate::float::valid_surface_id(id)
}

/// The form `define-picker-source` evaluates.
///
/// The body is a procedure of one argument — the args hash `open-picker`
/// carries — and binding it to a global is what makes a redefinition at the
/// REPL take effect with no restart. That is `T046`'s own criterion: *"a source
/// added from the REPL appears with no restart."*
#[must_use]
pub fn define_form(id: &str, body: &str) -> String {
    format!("(define {} {body})", source_global(id))
}

/// Calls a registered source and decodes the rows it answered.
///
/// The same three steps as [`crate::float::surface`] and
/// [`crate::status::compose`]: check the global, call it, decode.
///
/// # Errors
///
/// [`SourceError`], one variant per way it can fail to produce rows.
pub fn rows(
    runtime: &mut crate::runtime::Runtime,
    id: &str,
    args: &phosphor_core::value::Value,
) -> Result<Vec<SpanRow>, SourceError> {
    let global = source_global(id);
    if runtime.global(&global).is_err() {
        return Err(SourceError::Unknown(id.to_owned()));
    }
    let answered = runtime
        .call(&global, vec![crate::convert::to_steel(args)])
        .map_err(|error| SourceError::Raised(error.to_string()))?;
    match crate::view::node(&answered).map_err(SourceError::NotRows)? {
        Node::Spans { rows } => Ok(rows),
        // Named by its tag rather than by a list here, so a node kind added to
        // the protocol cannot quietly become a valid picker source.
        other => Err(SourceError::NotSpans(other.tag().to_owned())),
    }
}

/// Why a source did not produce rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// The id is not one an editor layer has defined.
    Unknown(String),
    /// It is defined and raised. Carries Steel's own text.
    Raised(String),
    /// It answered something that is not a view node at all.
    NotRows(crate::view::ViewError),
    /// It answered a node, and not a `spans` one.
    NotSpans(String),
    /// The id could not be a global name — see [`valid_source_id`].
    BadId(String),
}

impl core::fmt::Display for SourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // §6's voice: say what to do, not what went wrong.
            Self::Unknown(id) => write!(
                f,
                "no picker source `{id}` — (define-picker-source! …) makes one"
            ),
            Self::Raised(why) => write!(f, "{why}"),
            Self::NotRows(error) => write!(f, "{error}"),
            Self::NotSpans(tag) => write!(
                f,
                "a picker source answers (view/spans …), and this one answered a `{tag}`"
            ),
            Self::BadId(id) => write!(
                f,
                "`{id}` cannot name a picker source — letters, digits, - and _"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_global_is_the_prefix_and_the_id() {
        assert_eq!(source_global("unseen"), "phosphor/picker-source/unseen");
    }

    /// The validator is `float`'s, called rather than copied — so this asserts
    /// the *shared* rule and would fail if someone forked it.
    #[test]
    fn an_id_that_could_inject_scheme_is_refused() {
        assert!(valid_source_id("unseen"));
        assert!(valid_source_id("files-2"));
        assert!(valid_source_id("a_b"));

        assert!(!valid_source_id(""));
        assert!(!valid_source_id("x) (displayln \"owned\""));
        assert!(!valid_source_id("has space"));
        assert!(!valid_source_id("dots.are.out"));
    }

    #[test]
    fn the_define_form_binds_the_body_to_the_global() {
        assert_eq!(
            define_form("unseen", "(lambda (args) 1)"),
            "(define phosphor/picker-source/unseen (lambda (args) 1))"
        );
    }

    #[test]
    fn every_error_says_what_to_do() {
        let said = SourceError::Unknown("nope".to_owned()).to_string();
        assert!(said.contains("define-picker-source"), "{said}");

        let said = SourceError::NotSpans("picker".to_owned()).to_string();
        assert!(said.contains("view/spans"), "{said}");

        let said = SourceError::BadId("a b".to_owned()).to_string();
        assert!(said.contains("letters, digits"), "{said}");
    }
}
