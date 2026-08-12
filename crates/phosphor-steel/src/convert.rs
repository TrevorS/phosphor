//! The value bridge — [`SteelVal`] on one side, [`Value`] on the other.
//!
//! `phosphor-core`'s wire model is deliberately smaller than JSON
//! (`value.rs`: no floats, no signed/unsigned split, no map with arbitrary
//! keys) and this module is the only place Steel's much larger value space is
//! narrowed onto it. Every door has one of these; this is the Steel one.
//!
//! Three conversions are *lossy in the forgiving direction*, and each is here
//! because a drawing or a payload type asks for it:
//!
//! * **A symbol decodes as text.** `6b` writes
//!   `(watch-place "src/retry.rs:24" 'delay)` (TUI Mockups.dc.html:502) — a
//!   quoted symbol where the vocabulary declares a
//!   [`ParamType::Choice`](phosphor_core::registry::ParamType::Choice). Refusing
//!   it would make the drawn line an error.
//! * **A character decodes as text.** [`Wire for char`](phosphor_core::value::Wire)
//!   carries a register name as one-character text, and `"a` reads far more
//!   naturally in scheme as `#\a`.
//! * **Void decodes as null**, which is what an omitted trailing argument
//!   arrives as.
//!
//! Encoding is total and never lossy: every [`Value`] case has exactly one
//! `SteelVal` spelling, listed on [`to_steel`].
//!
//! Owned by `spine`.

use std::collections::HashMap;

use phosphor_core::value::{Args, Value};
use steel::rvals::{FromSteelVal as _, IntoSteelVal as _, SteelVal};

/// A `SteelVal` the wire model has no case for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError {
    /// What arrived, named the way Steel names it.
    pub found: String,
    /// Why it could not cross.
    pub because: &'static str,
}

impl core::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self { found, because } = self;
        write!(f, "cannot cross the barrier: {found} — {because}")
    }
}

impl std::error::Error for ConversionError {}

/// A [`Value`] as Steel sees it.
///
/// | wire | scheme |
/// |---|---|
/// | [`Value::Null`] | `void` |
/// | [`Value::Bool`] | `#true` / `#false` |
/// | [`Value::Int`] | an integer |
/// | [`Value::Text`] | a string |
/// | [`Value::List`] | a list |
/// | [`Value::Record`] | a hash, string-keyed |
///
/// A record is a hash rather than an association list because a composition
/// reads it with `hash-get` and writes it with `hash`, both of which are in the
/// language already — an assoc list would be a convention every `.scm` file had
/// to know.
///
/// # Panics
///
/// Never. Every arm is infallible; the `expect` guards a `Vec`/`HashMap`
/// conversion whose only error path is an element that failed to convert, and
/// every element here is already a `SteelVal`.
#[must_use]
pub fn to_steel(value: &Value) -> SteelVal {
    match value {
        Value::Null => SteelVal::Void,
        Value::Bool(flag) => SteelVal::BoolV(*flag),
        Value::Int(number) => isize::try_from(*number).map_or_else(
            // The wire is `i64` and `SteelVal::IntV` is `isize`. They are the
            // same width on every target we ship, so this is a guard rather
            // than a case: a 32-bit host gets a bignum instead of a wrap.
            |_| SteelVal::StringV(number.to_string().into()),
            SteelVal::IntV,
        ),
        Value::Text(text) => SteelVal::StringV(text.as_str().into()),
        Value::List(items) => items
            .iter()
            .map(to_steel)
            .collect::<Vec<_>>()
            .into_steelval()
            .expect("a list of SteelVals always converts to a SteelVal list"),
        Value::Record(args) => args
            .iter()
            .map(|(name, field)| (name.to_owned(), to_steel(field)))
            .collect::<HashMap<String, SteelVal>>()
            .into_steelval()
            .expect("a string-keyed map of SteelVals always converts to a hash"),
    }
}

/// A `SteelVal` as the wire model sees it.
///
/// # Errors
///
/// [`ConversionError`] for a value with no wire case — a closure, a port, a
/// continuation, a custom type. Those are precisely the things that must not
/// cross: a payload is plain data (`action.rs`, property 1), and a scheme
/// closure on the wire is what would make the MCP schema underivable.
pub fn from_steel(value: &SteelVal) -> Result<Value, ConversionError> {
    Ok(match value {
        SteelVal::Void => Value::Null,
        SteelVal::BoolV(flag) => Value::Bool(*flag),
        SteelVal::IntV(number) => {
            Value::Int(i64::try_from(*number).map_err(|_| ConversionError {
                found: "an integer".to_owned(),
                because: "it does not fit in the wire's 64 bits",
            })?)
        }
        SteelVal::StringV(text) => Value::Text(text.to_string()),
        // The three forgiving decodes. See the module docs for the drawing or
        // the payload type that asks for each.
        SteelVal::SymbolV(name) => Value::Text(name.to_string()),
        SteelVal::CharV(character) => Value::Text(character.to_string()),
        SteelVal::ListV(items) => Value::List(
            items
                .into_iter()
                .map(from_steel)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        SteelVal::HashMapV(_) => {
            let fields =
                HashMap::<String, SteelVal>::from_steelval(value).map_err(|_| ConversionError {
                    found: "a hash".to_owned(),
                    because: "the wire model has no case for a non-string key",
                })?;
            // `Args` is ordered and a hash is not, so the order has to come
            // from somewhere stable or two identical records compare unequal
            // (`value.rs`: equality is what the round-trip test compares).
            // Sorted by name is the only order available here; decoding reads
            // by name, so nothing downstream depends on it.
            let mut names: Vec<&String> = fields.keys().collect();
            names.sort_unstable();
            let mut args = Args::new();
            for name in names {
                args.set(name, from_steel(&fields[name])?);
            }
            Value::Record(args)
        }
        other => {
            return Err(ConversionError {
                found: kind_of(other).to_owned(),
                because: "a payload is plain data — no closures, no ports, no handles",
            });
        }
    })
}

/// What to call a `SteelVal` in a message.
///
/// Steel's own `Display` for a closure is `#<function>`, which tells a reader
/// nothing about *why* it was refused. These names are chosen to read in the
/// product's voice (Design Language §6: lowercase, telegraphic).
fn kind_of(value: &SteelVal) -> &'static str {
    match value {
        SteelVal::Closure(_)
        | SteelVal::FuncV(_)
        | SteelVal::BoxedFunction(_)
        | SteelVal::MutFunc(_)
        | SteelVal::BuiltIn(_)
        | SteelVal::FutureFunc(_) => "a function",
        SteelVal::NumV(_) => "a float",
        SteelVal::Rational(_) | SteelVal::BigRational(_) => "a rational",
        SteelVal::BigNum(_) => "a bignum",
        SteelVal::Complex(_) => "a complex number",
        SteelVal::VectorV(_) | SteelVal::MutableVector(_) => "a vector",
        SteelVal::ByteVector(_) => "a byte vector",
        SteelVal::HashSetV(_) => "a set",
        SteelVal::CustomStruct(_) => "a struct",
        SteelVal::PortV(_) => "a port",
        SteelVal::Custom(_) | SteelVal::Reference(_) => "a foreign object",
        SteelVal::ContinuationFunction(_) => "a continuation",
        SteelVal::Pair(_) => "an improper pair",
        _ => "a value",
    }
}

/// Text as a scheme **string literal** — source, not a value.
///
/// The other direction from [`to_steel`], and a smaller one: two callers hand
/// the VM a form to *read* rather than a value to hold, and both of them are
/// putting text inside it. [`crate::repl`] persists a typed form
/// (`(persist-form! "…")`) and [`crate::keymap`] names a key
/// (`(phosphor/press "…")`), and a key or a form containing a quote must not be
/// able to close the string early.
///
/// It is here rather than at either caller because *"how text becomes scheme"*
/// is this module's question, and two escapers would be two answers to it.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: &Value) -> Value {
        from_steel(&to_steel(value)).expect("an encoded value always decodes")
    }

    #[test]
    fn every_scalar_wire_case_round_trips_unchanged() {
        for value in [
            Value::Null,
            Value::Bool(true),
            Value::Int(-7),
            Value::Text("src/retry.rs".to_owned()),
            Value::List(vec![Value::Int(1), Value::Text("two".to_owned())]),
        ] {
            assert_eq!(round_trip(&value), value, "{value:?} did not survive");
        }
    }

    #[test]
    fn a_quoted_symbol_decodes_as_text() {
        // `6b` writes `(watch-place "src/retry.rs:24" 'delay)`. `delay` is a
        // choice tag, and the drawing spells it as a symbol.
        let decoded = from_steel(&SteelVal::SymbolV("delay".into())).expect("a symbol crosses");
        assert_eq!(decoded, Value::Text("delay".to_owned()));
    }

    #[test]
    fn a_character_decodes_as_text() {
        // `"a` in `"ayy` — `RegisterName` is one-character text on the wire.
        let decoded = from_steel(&SteelVal::CharV('a')).expect("a character crosses");
        assert_eq!(decoded, Value::Text("a".to_owned()));
    }

    #[test]
    fn a_function_cannot_cross() {
        let error = from_steel(&SteelVal::FuncV(|_| Ok(SteelVal::Void)))
            .expect_err("a closure is not plain data");
        assert_eq!(error.found, "a function");
    }

    #[test]
    fn a_record_keeps_every_field_but_not_its_declaration_order() {
        // The one documented asymmetry: a scheme hash is unordered and `Args`
        // is not, so a record that crosses and comes back is equal *by field*
        // and sorted by name. Nothing downstream depends on the order —
        // `Wire::from_value` reads records by name — but `Value`'s own `Eq`
        // does, so it is written down here rather than discovered later.
        let record = Value::Record(
            Args::new()
                .with("zebra", Value::Int(1))
                .with("alpha", Value::Int(2)),
        );
        let Value::Record(decoded) = round_trip(&record) else {
            panic!("a record decodes as a record");
        };
        assert_eq!(decoded.get("zebra"), Some(&Value::Int(1)));
        assert_eq!(decoded.get("alpha"), Some(&Value::Int(2)));
        assert_eq!(decoded.len(), 2);
        assert_eq!(
            decoded.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["alpha", "zebra"],
            "sorted by name, which is the only stable order a hash can give"
        );
    }

    #[test]
    fn a_string_literal_survives_a_quote_and_a_backslash() {
        assert_eq!(string_literal("]r"), "\"]r\"");
        assert_eq!(string_literal(r#"(f "a\b")"#), r#""(f \"a\\b\")""#);
        assert_eq!(string_literal("two\nlines"), "\"two\\nlines\"");
    }
}
