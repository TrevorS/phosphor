//! The wire model — one owned, door-neutral value type, and the trait that maps
//! payload types onto it.
//!
//! Invariant 2 says Steel, MCP and the CLI share one vocabulary. They do not
//! share a value representation: Steel has `SteelVal`, MCP has JSON, the CLI has
//! `&str`. [`Value`] is the pivot all three convert through, and [`Wire`] is what
//! makes that conversion *derivable* — a payload type declares its shape once, in
//! [`Wire::TYPE`], and `T020` reads that to emit a JSON Schema, a CLI flag set and
//! a Steel argument list without a second table to keep in step.
//!
//! Deliberately smaller than JSON: no floats, no unsigned/signed split on the
//! wire, no map-with-arbitrary-keys. Every payload in [`crate::action`] and
//! [`crate::query`] fits, and the smaller the model the fewer ways two doors can
//! disagree about the same call. Adding a case here is a contract change, not a
//! convenience — it is `spine`'s call, and every door has to learn it.
//!
//! Owned by `spine` (`TEAM.md`: the `Action` enum, the query vocabulary and the
//! view tree have one writer).

use core::fmt;
use std::path::{Path, PathBuf};

use crate::registry::ParamType;

// ---------------------------------------------------------------------------
// Value
// ---------------------------------------------------------------------------

/// A door-neutral argument or result value.
///
/// Owned throughout: nothing here borrows into the store. A query result is a
/// snapshot, and a Steel composition that holds one across a mutation is holding
/// stale data, never a dangling reference (`query.rs` says why that matters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Value {
    /// Absent. An omitted optional field decodes from this.
    #[default]
    Null,
    /// `#t` / `true` / `--flag`.
    Bool(bool),
    /// Every number. Counts, deltas, ids, line numbers, milliseconds.
    ///
    /// One integer case rather than signed/unsigned pairs: the range check
    /// belongs to the payload type ([`Wire::from_value`]), not to the wire, so a
    /// negative count is one error message in one place.
    Int(i64),
    /// Text, paths, key notation, Steel source, choice tags.
    Text(String),
    /// A homogeneous sequence. Heterogeneity is a [`Value::Record`].
    List(Vec<Value>),
    /// A named record — a struct, or a tagged union carrying its tag in
    /// [`TAG_FIELD`].
    Record(Args),
}

/// The field a tagged union carries its variant name in.
///
/// One constant rather than a convention: MCP schema generation, the Steel
/// binding and the CLI parser all have to agree on this spelling, and they are
/// three files apart.
pub const TAG_FIELD: &str = "kind";

impl Value {
    /// The name of this case, for error messages that have to say what arrived.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Text(_) => "text",
            Self::List(_) => "list",
            Self::Record(_) => "record",
        }
    }

    /// Convenience for the common case of building a tagged union body.
    #[must_use]
    pub fn tagged(tag: &str, fields: Args) -> Self {
        let mut args = Args::new();
        args.set(TAG_FIELD, Self::Text(tag.to_owned()));
        for (name, value) in fields.into_pairs() {
            args.set(&name, value);
        }
        Self::Record(args)
    }

    /// The tag of a tagged union body, if this is one.
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        match self {
            Self::Record(args) => match args.get(TAG_FIELD) {
                Some(Self::Text(tag)) => Some(tag.as_str()),
                _ => None,
            },
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

/// An ordered set of named arguments.
///
/// Ordered, and a `Vec` rather than a map, for three reasons that all show up at
/// `T020`: declaration order is the CLI's positional order and the Steel
/// argument order; equality is stable, so the round-trip test in
/// `tests/vocabulary.rs` can compare two calls directly; and there is no hash
/// dependency at a crate floor that has no dependencies at all.
///
/// Duplicate names are impossible — [`Args::set`] replaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args(Vec<(String, Value)>);

impl Args {
    /// An empty argument set.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Builder form of [`Args::set`].
    #[must_use]
    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.set(name, value);
        self
    }

    /// Sets `name`, replacing any previous value and keeping its position.
    pub fn set(&mut self, name: &str, value: Value) {
        if let Some(slot) = self.0.iter_mut().find(|(key, _)| key == name) {
            slot.1 = value;
        } else {
            self.0.push((name.to_owned(), value));
        }
    }

    /// The raw value of `name`, if present.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    /// Decodes `name` into a payload type.
    ///
    /// A missing field is an error for a required type and `None` for an
    /// `Option`, which is the whole of "optional" as far as the doors are
    /// concerned. Errors are wrapped in [`WireError::Field`] so a door can say
    /// *which* argument was wrong rather than only what was wrong with it.
    ///
    /// # Errors
    ///
    /// [`WireError::Missing`] if the field is absent and required; whatever the
    /// payload type's [`Wire::from_value`] returns otherwise.
    pub fn field<T: Wire>(&self, name: &'static str) -> Result<T, WireError> {
        let raw = self.get(name);
        match raw {
            Some(value) => T::from_value(value).map_err(|source| WireError::Field {
                field: name,
                source: Box::new(source),
            }),
            None if !T::REQUIRED => {
                T::from_value(&Value::Null).map_err(|source| WireError::Field {
                    field: name,
                    source: Box::new(source),
                })
            }
            None => Err(WireError::Missing { field: name }),
        }
    }

    /// Iterates the arguments in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }

    /// Consumes into `(name, value)` pairs, in declaration order.
    pub fn into_pairs(self) -> impl Iterator<Item = (String, Value)> {
        self.0.into_iter()
    }

    /// How many arguments are set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no argument is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(String, Value)> for Args {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        let mut args = Self::new();
        for (name, value) in iter {
            args.set(&name, value);
        }
        args
    }
}

// ---------------------------------------------------------------------------
// Call
// ---------------------------------------------------------------------------

/// A capability invocation as a door hands it over: a name and its arguments.
///
/// The one shape all three doors normalise to before anything is constructed.
/// `(mark-seen! 'selection)`, `phosphor/mark-seen {"target": …}` and
/// `phosphor mark-seen --target selection` become the same [`Call`], and
/// [`Action::from_call`](crate::action::Action::from_call) /
/// [`Query::from_call`](crate::query::Query::from_call) take it from there.
///
/// It is also what makes `T024`'s parity test cheap: encode a capability, decode
/// it back, and any door that disagrees with the declared params fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The globally unique door name.
    pub name: String,
    /// Its arguments, in declaration order.
    pub args: Args,
}

impl Call {
    /// A call with no arguments.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            args: Args::new(),
        }
    }

    /// Builder: adds an argument.
    #[must_use]
    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.args.set(name, value);
        self
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a value could not be decoded into a payload type.
///
/// Carries enough to render a legible message at any door: the CLI prints it,
/// the REPL shows it, MCP returns it in the error body. None of them should have
/// to guess which argument was at fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The wrong shape arrived.
    Type {
        /// What the payload type wanted.
        expected: &'static str,
        /// What [`Value::kind`] said turned up.
        found: &'static str,
    },
    /// A required field was absent.
    Missing {
        /// The field's declared name.
        field: &'static str,
    },
    /// A choice or tagged union carried a tag nothing declares.
    Tag {
        /// The tag that arrived.
        got: String,
        /// Every tag the type accepts, in declaration order.
        expected: &'static [&'static str],
    },
    /// In the wire's range but not in the payload type's.
    Range {
        /// What the payload type wanted, e.g. `"a non-negative integer"`.
        expected: &'static str,
        /// The integer that arrived.
        got: i64,
    },
    /// A named field failed. Wraps the cause so the path is recoverable.
    Field {
        /// The failing field's declared name.
        field: &'static str,
        /// Why it failed.
        source: Box<WireError>,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type { expected, found } => write!(f, "expected {expected}, found {found}"),
            Self::Missing { field } => write!(f, "missing required argument `{field}`"),
            Self::Tag { got, expected } => {
                write!(f, "unknown kind `{got}` — expected one of {expected:?}")
            }
            Self::Range { expected, got } => write!(f, "expected {expected}, found {got}"),
            Self::Field { field, source } => write!(f, "argument `{field}`: {source}"),
        }
    }
}

impl std::error::Error for WireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Field { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Wire
// ---------------------------------------------------------------------------

/// A payload type that can cross a door.
///
/// Three things at once, and that is the point — they cannot drift apart because
/// they are one impl:
///
/// * [`Wire::TYPE`] — the declared shape. `T020` turns this into a JSON Schema
///   for the MCP tool and a flag set for the CLI verb.
/// * [`Wire::to_value`] / [`Wire::from_value`] — the conversion the doors run.
/// * [`Wire::REQUIRED`] — `false` only for `Option<T>`, which is how optionality
///   reaches the schema without a second annotation.
///
/// **Every field of every [`crate::action::Action`] and [`crate::query::Query`]
/// variant implements this.** That is what makes "no variant whose arguments
/// cannot be expressed over MCP" a compile-time fact rather than a review note.
/// A payload needing a Steel closure would break it — so bindings carry source
/// text or a capability name instead (see [`crate::request::Binding`]).
pub trait Wire: Sized {
    /// The declared shape, for schema generation.
    const TYPE: ParamType;

    /// Whether a door must supply this argument. `false` for `Option<T>`.
    const REQUIRED: bool = true;

    /// Encodes into the wire model.
    fn to_value(&self) -> Value;

    /// Decodes from the wire model.
    ///
    /// # Errors
    ///
    /// [`WireError`] describing the mismatch. Implementations do not panic on
    /// hostile input: every door is reachable by something we do not control.
    fn from_value(value: &Value) -> Result<Self, WireError>;
}

impl Wire for bool {
    const TYPE: ParamType = ParamType::Bool;

    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Bool(flag) => Ok(*flag),
            other => Err(WireError::Type {
                expected: "a boolean",
                found: other.kind(),
            }),
        }
    }
}

impl Wire for i64 {
    const TYPE: ParamType = ParamType::Int;

    fn to_value(&self) -> Value {
        Value::Int(*self)
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Int(number) => Ok(*number),
            other => Err(WireError::Type {
                expected: "an integer",
                found: other.kind(),
            }),
        }
    }
}

/// `u32` and `u64` decode through the same range check; the wire is signed.
macro_rules! wire_unsigned {
    ($($t:ty),* $(,)?) => {
        $(
            impl Wire for $t {
                const TYPE: ParamType = ParamType::Uint;

                fn to_value(&self) -> Value {
                    Value::Int(i64::try_from(*self).unwrap_or(i64::MAX))
                }

                fn from_value(value: &Value) -> Result<Self, WireError> {
                    match value {
                        Value::Int(number) => Self::try_from(*number).map_err(|_| {
                            WireError::Range {
                                expected: concat!("a value in range for ", stringify!($t)),
                                got: *number,
                            }
                        }),
                        other => Err(WireError::Type {
                            expected: "a non-negative integer",
                            found: other.kind(),
                        }),
                    }
                }
            }
        )*
    };
}

wire_unsigned!(u32, u64);

impl Wire for String {
    const TYPE: ParamType = ParamType::Text;

    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Text(text) => Ok(text.clone()),
            other => Err(WireError::Type {
                expected: "text",
                found: other.kind(),
            }),
        }
    }
}

impl Wire for char {
    const TYPE: ParamType = ParamType::Char;

    fn to_value(&self) -> Value {
        Value::Text(self.to_string())
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Text(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(only), None) => Ok(only),
                    _ => Err(WireError::Type {
                        expected: "exactly one character",
                        found: "text",
                    }),
                }
            }
            other => Err(WireError::Type {
                expected: "exactly one character",
                found: other.kind(),
            }),
        }
    }
}

/// Paths cross as text.
///
/// `to_value` is lossy on a non-UTF-8 path, which is the right failure here: the
/// three doors are JSON, a scheme string and a shell argument, and none of them
/// can carry the bytes faithfully anyway. Nothing in the vocabulary round-trips a
/// path it did not receive as text.
impl Wire for PathBuf {
    const TYPE: ParamType = ParamType::Path;

    fn to_value(&self) -> Value {
        Value::Text(self.to_string_lossy().into_owned())
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Text(text) => Ok(Self::from(Path::new(text))),
            other => Err(WireError::Type {
                expected: "a path",
                found: other.kind(),
            }),
        }
    }
}

impl<T: Wire> Wire for Option<T> {
    const TYPE: ParamType = T::TYPE;
    const REQUIRED: bool = false;

    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::Null, Wire::to_value)
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Null => Ok(None),
            present => T::from_value(present).map(Some),
        }
    }
}

impl<T: Wire> Wire for Vec<T> {
    const TYPE: ParamType = ParamType::List(&T::TYPE);

    fn to_value(&self) -> Value {
        Value::List(self.iter().map(Wire::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::List(items) => items.iter().map(T::from_value).collect(),
            other => Err(WireError::Type {
                expected: "a list",
                found: other.kind(),
            }),
        }
    }
}

/// Free-form arguments, for the one place the vocabulary deliberately stops
/// short: a surface's own parameters ([`crate::request::SurfaceId`]).
impl Wire for Args {
    const TYPE: ParamType = ParamType::Any;

    fn to_value(&self) -> Value {
        Value::Record(self.clone())
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        match value {
            Value::Record(args) => Ok(args.clone()),
            Value::Null => Ok(Self::new()),
            other => Err(WireError::Type {
                expected: "a record",
                found: other.kind(),
            }),
        }
    }
}

impl Wire for Value {
    const TYPE: ParamType = ParamType::Any;

    fn to_value(&self) -> Value {
        self.clone()
    }

    fn from_value(value: &Value) -> Result<Self, WireError> {
        Ok(value.clone())
    }
}

// ---------------------------------------------------------------------------
// The three derive macros
// ---------------------------------------------------------------------------

/// Implements [`Wire`] for an already-declared C-like enum, as a tag string.
///
/// The tags are the door spelling — kebab-case, because that is what the Steel
/// bindings, the CLI flags and the mockups all use.
macro_rules! wire_choice {
    (
        $ty:ident {
            $( $variant:ident => $tag:literal ),* $(,)?
        }
    ) => {
        impl $crate::value::Wire for $ty {
            const TYPE: $crate::registry::ParamType =
                $crate::registry::ParamType::Choice(&[$($tag),*]);

            fn to_value(&self) -> $crate::value::Value {
                $crate::value::Value::Text(
                    match self { $( Self::$variant => $tag ),* }.to_owned(),
                )
            }

            fn from_value(
                value: &$crate::value::Value,
            ) -> ::core::result::Result<Self, $crate::value::WireError> {
                match value {
                    $crate::value::Value::Text(tag) => match tag.as_str() {
                        $( $tag => Ok(Self::$variant), )*
                        other => Err($crate::value::WireError::Tag {
                            got: other.to_owned(),
                            expected: &[$($tag),*],
                        }),
                    },
                    other => Err($crate::value::WireError::Type {
                        expected: "one of a fixed set of names",
                        found: other.kind(),
                    }),
                }
            }
        }
    };
}

/// Implements [`Wire`] for an already-declared struct with named fields.
macro_rules! wire_record {
    (
        $ty:ident {
            $( $field:ident : $fty:ty = $doc:literal ),* $(,)?
        }
    ) => {
        impl $crate::value::Wire for $ty {
            const TYPE: $crate::registry::ParamType =
                $crate::registry::ParamType::Record(&[
                    $( $crate::registry::Param {
                        name: stringify!($field),
                        doc: $doc,
                        ty: <$fty as $crate::value::Wire>::TYPE,
                        required: <$fty as $crate::value::Wire>::REQUIRED,
                    } ),*
                ]);

            fn to_value(&self) -> $crate::value::Value {
                let mut args = $crate::value::Args::new();
                $( args.set(
                    stringify!($field),
                    $crate::value::Wire::to_value(&self.$field),
                ); )*
                $crate::value::Value::Record(args)
            }

            fn from_value(
                value: &$crate::value::Value,
            ) -> ::core::result::Result<Self, $crate::value::WireError> {
                match value {
                    $crate::value::Value::Record(args) => Ok(Self {
                        $( $field: args.field(stringify!($field))? ),*
                    }),
                    other => Err($crate::value::WireError::Type {
                        expected: concat!("a ", stringify!($ty), " record"),
                        found: other.kind(),
                    }),
                }
            }
        }
    };
}

/// Implements [`Wire`] for an already-declared enum whose variants carry named
/// fields, as a record tagged by [`TAG_FIELD`].
///
/// Every variant must be brace-form, including the empty ones (`Cursor {}`).
/// Uniform shape beats terse declarations here: one match arm pattern works for
/// all of them, and the generated code has no special cases to get wrong.
/// The `text = …` parser a union gets when it declares none.
///
/// Generic in the return type so the macro can call it unconditionally: the
/// binding it feeds is annotated `Option<Self>`, which is what fixes `T`.
pub(crate) fn no_text_spelling<T>(_text: &str) -> Option<T> {
    None
}

macro_rules! wire_union {
    // No plain-text spelling — the ordinary case. Delegates rather than
    // duplicating the body, so the two arms cannot drift.
    (
        $ty:ident {
            $($body:tt)*
        }
    ) => {
        $crate::value::wire_union!($ty, text = $crate::value::no_text_spelling {
            $($body)*
        });
    };

    (
        $ty:ident, text = $parser:path {
            $(
                $variant:ident => $tag:literal, $vdoc:literal {
                    $( $field:ident : $fty:ty = $doc:literal ),* $(,)?
                }
            ),* $(,)?
        }
    ) => {
        impl $crate::value::Wire for $ty {
            const TYPE: $crate::registry::ParamType =
                $crate::registry::ParamType::Union(&[
                    $( $crate::registry::UnionVariant {
                        tag: $tag,
                        doc: $vdoc,
                        fields: &[
                            $( $crate::registry::Param {
                                name: stringify!($field),
                                doc: $doc,
                                ty: <$fty as $crate::value::Wire>::TYPE,
                                required: <$fty as $crate::value::Wire>::REQUIRED,
                            } ),*
                        ],
                    } ),*
                ]);

            fn to_value(&self) -> $crate::value::Value {
                match self {
                    $(
                        Self::$variant { $( $field ),* } => {
                            #[allow(unused_mut, reason = "a fieldless union arm sets nothing")]
                            let mut args = $crate::value::Args::new();
                            $( args.set(
                                stringify!($field),
                                $crate::value::Wire::to_value($field),
                            ); )*
                            $crate::value::Value::tagged($tag, args)
                        }
                    ),*
                }
            }

            fn from_value(
                value: &$crate::value::Value,
            ) -> ::core::result::Result<Self, $crate::value::WireError> {
                // The optional plain-text spelling, tried before the tagged
                // shape and never instead of it. A union that declares none
                // gets `no_text_spelling`, which is `None` for every input, so
                // this costs one branch and changes no behaviour there.
                if let $crate::value::Value::Text(text) = value {
                    let spelled: ::core::option::Option<Self> = $parser(text);
                    if let ::core::option::Option::Some(target) = spelled {
                        return ::core::result::Result::Ok(target);
                    }
                }
                let $crate::value::Value::Record(args) = value else {
                    return Err($crate::value::WireError::Type {
                        expected: concat!("a tagged ", stringify!($ty), " record"),
                        found: value.kind(),
                    });
                };
                let tag = value.tag().ok_or($crate::value::WireError::Missing {
                    field: $crate::value::TAG_FIELD,
                })?;
                match tag {
                    $(
                        $tag => Ok(Self::$variant {
                            $( $field: args.field(stringify!($field))? ),*
                        }),
                    )*
                    other => Err($crate::value::WireError::Tag {
                        got: other.to_owned(),
                        expected: &[$($tag),*],
                    }),
                }
            }
        }
    };
}

pub(crate) use {wire_choice, wire_record, wire_union};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_replace_keeps_position() {
        let mut args = Args::new()
            .with("a", Value::Int(1))
            .with("b", Value::Int(2));
        args.set("a", Value::Int(3));
        let names: Vec<_> = args.iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(args.get("a"), Some(&Value::Int(3)));
    }

    #[test]
    fn missing_required_field_is_named() {
        let args = Args::new();
        let error = args.field::<String>("path").unwrap_err();
        assert_eq!(error, WireError::Missing { field: "path" });
    }

    #[test]
    fn missing_optional_field_is_none() {
        let args = Args::new();
        assert_eq!(args.field::<Option<String>>("path").unwrap(), None);
    }

    #[test]
    fn field_errors_carry_the_field_name() {
        let args = Args::new().with("count", Value::Text("three".to_owned()));
        let error = args.field::<u32>("count").unwrap_err();
        let WireError::Field { field, .. } = error else {
            panic!("expected a field-scoped error, got {error:?}");
        };
        assert_eq!(field, "count");
    }

    #[test]
    fn negative_into_unsigned_is_a_range_error() {
        let error = u32::from_value(&Value::Int(-1)).unwrap_err();
        assert!(matches!(error, WireError::Range { .. }));
    }

    #[test]
    fn optional_round_trips_through_null() {
        let empty: Option<u32> = None;
        assert_eq!(empty.to_value(), Value::Null);
        assert_eq!(Option::<u32>::from_value(&Value::Null).unwrap(), None);
    }
}
