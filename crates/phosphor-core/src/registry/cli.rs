//! The CLI door — one verb per capability, derived from its row.
//!
//! **Nothing here is a list**, exactly as in [`steel`](super::steel) and
//! [`mcp`](super::mcp): [`verb`] is a total function of a [`Capability`].
//! `T023` builds `phosphor <verb> --flag …` by walking [`verbs`] and hands the
//! resulting [`Call`] to the same dispatcher the other two doors use.
//!
//! # Two forms, one door
//!
//! `phosphor --eval '(mark-seen! …)'` is the door's *primitive* form — the plan
//! names it as the CLI door outright, and `V006` drives the editor through it.
//! The generated verbs are sugar over the same [`Call`]: flags in, `Call` out,
//! one dispatcher. Nothing is reachable by a verb that is not reachable by
//! `--eval`.
//!
//! # How a structured argument becomes flags
//!
//! Flattening, because it is derivable and needs no new syntax to be invented
//! and then kept in step with the other two doors:
//!
//! ```text
//! target: Target (a tagged union)   →  --target <arm>            the tag
//!                                      --target.path <PATH>      shared by two arms
//!                                      --target.region.id <…>    tag-qualified
//! at: Position (a record)           →  --at.line <UINT>
//!                                      --at.column <UINT>
//! ```
//!
//! Two arms that declare the same field name **with the same shape** share one
//! flag — `Target`'s `file` and `explicit` arms both carry a `PathBuf`, so
//! `--target.path` serves both and the tag already says which. Where the shapes
//! differ the flag is qualified with the tag: `Target`'s four id-bearing arms
//! carry four *different* id types, so they are `--target.buffer.id`,
//! `--target.region.id`, `--target.anchor.id` and `--target.hunk.id`. A flag
//! never means two shapes.
//!
//! # The one hole, named rather than hidden
//!
//! A **list of records** (`apply-edits`' `edits`, `declare-review-block`'s
//! groups) and [`ParamType::Any`] cannot be flattened into flags — repeating
//! `--edits.text` twice has no way to say which edit each belongs to. Those
//! parameters are listed in [`Verb::unreachable`] and reached through `--eval`,
//! which every one of them can express. The verb is still registered, still
//! carries its other flags, and is still in this door: the parity `T024` checks
//! is about *registration*, and a capability that existed in two doors would be
//! the failure.
//!
//! Owned by `spine`.

use crate::registry::{
    Capability, CapabilityKind, Param, ParamType, Since, UnionVariant, capabilities,
};
use crate::value::{Args, Call, Value};

// ---------------------------------------------------------------------------
// The verb
// ---------------------------------------------------------------------------

/// What a flag's value looks like on the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagValue {
    /// `--before`. Presence is `true`, absence is `false` — which is what a flag
    /// means, and the only place this door supplies a value nobody typed.
    Switch,
    /// `--line <UINT>`. One value, read by [`parse_scalar`].
    One(ParamType),
    /// `--path <PATH>`, repeatable. Each occurrence is one element; zero
    /// occurrences is the empty list.
    Many(ParamType),
    /// `--target <region|file|…>`. Selects a union arm, and decides which of the
    /// arm-qualified flags are read.
    Arm(Vec<&'static str>),
}

impl FlagValue {
    /// The placeholder in `--flag <THIS>`, or [`None`] for a switch.
    #[must_use]
    pub fn value_name(&self) -> Option<String> {
        match self {
            Self::Switch => None,
            Self::One(ty) | Self::Many(ty) => Some(ty.label().to_uppercase()),
            Self::Arm(tags) => Some(tags.join("|")),
        }
    }

    /// The permitted values, for a flag that has a fixed set of them.
    #[must_use]
    pub fn choices(&self) -> Option<&[&'static str]> {
        match self {
            Self::Arm(tags) => Some(tags.as_slice()),
            Self::One(ParamType::Choice(tags)) | Self::Many(ParamType::Choice(tags)) => Some(tags),
            Self::Switch | Self::One(_) | Self::Many(_) => None,
        }
    }
}

/// When a flag must be supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// Always — a required parameter, or a required field of one.
    Always,
    /// Never; omitting it means the argument is absent.
    Optional,
    /// Required only when another flag selected one of these union arms. `clap`
    /// spells this `required_if_eq_any`.
    WithArm {
        /// The flag carrying the tag.
        flag: String,
        /// The arms that need this one.
        tags: Vec<&'static str>,
    },
}

/// One command-line flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    /// The long name, without `--`: `"target"`, `"target.id"`, `"at.line"`.
    /// Kebab-case, because a parameter is a Rust field and the command line is
    /// not.
    pub long: String,
    /// One line, in the product's voice.
    pub help: &'static str,
    /// The value's shape.
    pub value: FlagValue,
    /// When it must be supplied.
    pub requirement: Requirement,
}

/// One capability as the CLI door sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verb {
    /// The subcommand: [`Capability::cli_verb`], the door name verbatim.
    pub verb: &'static str,
    /// One line, in the product's voice — the subcommand's `about`.
    pub about: &'static str,
    /// The domain, for grouping the help output.
    pub domain: &'static str,
    /// Action or query. A query prints its result; an Action prints its
    /// [`Outcome`](crate::action::Outcome).
    pub kind: CapabilityKind,
    /// The phase and task that implement it.
    pub since: Since,
    /// The flags, in declaration order, parents before children.
    pub flags: Vec<Flag>,
    /// Parameters that cannot be expressed as flags — see the module docs. Empty
    /// for all but a handful.
    pub unreachable: Vec<&'static str>,
}

impl Verb {
    /// Whether this verb needs `--eval` to be called with every argument it
    /// declares.
    #[must_use]
    pub fn needs_eval(&self) -> bool {
        !self.unreachable.is_empty()
    }

    /// The flag with this long name, if any.
    #[must_use]
    pub fn flag(&self, long: &str) -> Option<&Flag> {
        self.flags.iter().find(|flag| flag.long == long)
    }
}

/// This capability's CLI verb. Total — see the module docs.
#[must_use]
pub fn verb(capability: &Capability) -> Verb {
    let mut flags = Vec::new();
    let mut unreachable = Vec::new();
    for param in capability.params {
        let requirement = if param.required {
            Requirement::Always
        } else {
            Requirement::Optional
        };
        if flatten(
            &kebab(param.name),
            param.doc,
            &param.ty,
            &requirement,
            &mut flags,
        )
        .is_err()
        {
            unreachable.push(param.name);
        }
    }
    Verb {
        verb: capability.cli_verb(),
        about: capability.doc,
        domain: capability.domain,
        kind: capability.kind,
        since: capability.since,
        flags,
        unreachable,
    }
}

/// Every capability's verb, in registry order.
#[must_use]
pub fn verbs() -> Vec<Verb> {
    capabilities().iter().map(verb).collect()
}

// ---------------------------------------------------------------------------
// Flattening
// ---------------------------------------------------------------------------

/// A shape with no flag form. Carried as an error so [`verb`] can record the
/// parameter in [`Verb::unreachable`] rather than pretend.
struct NoFlagForm;

fn flatten(
    base: &str,
    help: &'static str,
    ty: &ParamType,
    requirement: &Requirement,
    out: &mut Vec<Flag>,
) -> Result<(), NoFlagForm> {
    match ty {
        ParamType::Bool => {
            // A switch cannot distinguish "absent" from "false", so an optional
            // boolean takes a value instead.
            let value = if matches!(requirement, Requirement::Optional) {
                FlagValue::One(ParamType::Bool)
            } else {
                FlagValue::Switch
            };
            push(out, base, help, value, requirement.clone());
            Ok(())
        }
        ParamType::Int
        | ParamType::Uint
        | ParamType::Text
        | ParamType::Char
        | ParamType::Path
        | ParamType::Choice(_)
        | ParamType::Id(_) => {
            push(out, base, help, FlagValue::One(*ty), requirement.clone());
            Ok(())
        }
        ParamType::List(inner) if is_scalar(inner) => {
            push(
                out,
                base,
                help,
                FlagValue::Many(**inner),
                // A repeated flag expresses the empty list by not appearing, so
                // it is never itself required.
                Requirement::Optional,
            );
            Ok(())
        }
        ParamType::Record(fields) => {
            for field in *fields {
                flatten(
                    &join(base, field.name),
                    field.doc,
                    &field.ty,
                    &narrow(requirement, field.required),
                    out,
                )?;
            }
            Ok(())
        }
        ParamType::Union(variants) => {
            push(
                out,
                base,
                help,
                FlagValue::Arm(tags_of(variants)),
                requirement.clone(),
            );
            for variant in *variants {
                for field in variant.fields {
                    let name = arm_field(base, variants, variant, field);
                    let field_requirement =
                        if field.required && !matches!(requirement, Requirement::Optional) {
                            Requirement::WithArm {
                                flag: base.to_owned(),
                                tags: vec![variant.tag],
                            }
                        } else {
                            Requirement::Optional
                        };
                    flatten(&name, field.doc, &field.ty, &field_requirement, out)?;
                }
            }
            Ok(())
        }
        ParamType::List(_) | ParamType::Any => Err(NoFlagForm),
    }
}

/// Adds a flag, merging with one that is already there.
///
/// Two arms of a union that share a field share a flag, so this is reached for
/// every union with a common field — `--target.id` is pushed four times and must
/// come out once, needed by whichever arms declare it.
fn push(out: &mut Vec<Flag>, long: &str, help: &'static str, value: FlagValue, req: Requirement) {
    if let Some(existing) = out.iter_mut().find(|flag| flag.long == long) {
        existing.requirement = merge(existing.requirement.clone(), req);
        return;
    }
    out.push(Flag {
        long: long.to_owned(),
        help,
        value,
        requirement: req,
    });
}

fn merge(left: Requirement, right: Requirement) -> Requirement {
    match (left, right) {
        (Requirement::Always, _) | (_, Requirement::Always) => Requirement::Always,
        (
            Requirement::WithArm { flag, mut tags },
            Requirement::WithArm {
                flag: other,
                tags: more,
            },
        ) if flag == other => {
            for tag in more {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            Requirement::WithArm { flag, tags }
        }
        (Requirement::WithArm { flag, tags }, _) | (_, Requirement::WithArm { flag, tags }) => {
            Requirement::WithArm { flag, tags }
        }
        (Requirement::Optional, Requirement::Optional) => Requirement::Optional,
    }
}

/// A field of a nested record is never *more* required than its parent.
fn narrow(parent: &Requirement, field_required: bool) -> Requirement {
    if field_required {
        parent.clone()
    } else {
        Requirement::Optional
    }
}

/// The flag name for one field of one arm — shared with the other arms that
/// declare the same name and shape, tag-qualified where they disagree.
fn arm_field(
    base: &str,
    variants: &'static [UnionVariant],
    variant: &'static UnionVariant,
    field: &'static Param,
) -> String {
    let conflicts = variants.iter().any(|other| {
        other.tag != variant.tag
            && other
                .fields
                .iter()
                .any(|candidate| candidate.name == field.name && candidate.ty != field.ty)
    });
    if conflicts {
        join(&join(base, variant.tag), field.name)
    } else {
        join(base, field.name)
    }
}

fn tags_of(variants: &'static [UnionVariant]) -> Vec<&'static str> {
    variants.iter().map(|variant| variant.tag).collect()
}

fn is_scalar(ty: &ParamType) -> bool {
    matches!(
        ty,
        ParamType::Bool
            | ParamType::Int
            | ParamType::Uint
            | ParamType::Text
            | ParamType::Char
            | ParamType::Path
            | ParamType::Choice(_)
            | ParamType::Id(_)
    )
}

fn join(base: &str, name: &str) -> String {
    format!("{base}.{}", kebab(name))
}

/// A Rust field name as a command-line flag.
fn kebab(name: &str) -> String {
    name.replace('_', "-")
}

// ---------------------------------------------------------------------------
// Reading values back
// ---------------------------------------------------------------------------

/// One flag as it appeared on the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// The long name, without `--`.
    pub long: String,
    /// Its value, or [`None`] for a switch.
    pub value: Option<String>,
}

impl Occurrence {
    /// A flag with a value.
    #[must_use]
    pub fn valued(long: &str, value: &str) -> Self {
        Self {
            long: long.to_owned(),
            value: Some(value.to_owned()),
        }
    }

    /// A switch.
    #[must_use]
    pub fn switch(long: &str) -> Self {
        Self {
            long: long.to_owned(),
            value: None,
        }
    }
}

/// Builds the door-neutral call from the flags a command line supplied.
///
/// The whole of `T023`'s argument handling: the CLI needs no knowledge of the
/// vocabulary's types, because the walk here is the same one [`verb`] used to
/// emit the flags.
///
/// # Errors
///
/// [`CliError`], naming the flag. A missing required flag, an unparseable value,
/// an unknown union arm, an unknown flag, or a parameter that only `--eval` can
/// express.
pub fn assemble(capability: &Capability, occurrences: &[Occurrence]) -> Result<Call, CliError> {
    let generated = verb(capability);
    for occurrence in occurrences {
        if generated.flag(&occurrence.long).is_none() {
            return Err(CliError::UnknownFlag {
                verb: generated.verb,
                flag: occurrence.long.clone(),
            });
        }
    }
    let mut args = Args::new();
    for param in capability.params {
        let value = read(&kebab(param.name), &param.ty, param.required, occurrences)?;
        args.set(param.name, value);
    }
    Ok(Call {
        name: capability.name.to_owned(),
        args,
    })
}

fn occurrences_of<'a>(long: &str, all: &'a [Occurrence]) -> Vec<&'a Occurrence> {
    all.iter()
        .filter(|occurrence| occurrence.long == long)
        .collect()
}

fn any_under(prefix: &str, all: &[Occurrence]) -> bool {
    let prefix = format!("{prefix}.");
    all.iter()
        .any(|occurrence| occurrence.long.starts_with(&prefix))
}

fn read(base: &str, ty: &ParamType, required: bool, all: &[Occurrence]) -> Result<Value, CliError> {
    match ty {
        ParamType::Bool => match occurrences_of(base, all).last() {
            Some(occurrence) => match &occurrence.value {
                Some(text) => {
                    parse_scalar(&ParamType::Bool, text).map_err(|expected| CliError::BadValue {
                        flag: base.to_owned(),
                        expected,
                        got: text.clone(),
                    })
                }
                None => Ok(Value::Bool(true)),
            },
            None if required => Ok(Value::Bool(false)),
            None => Ok(Value::Null),
        },
        ParamType::Int
        | ParamType::Uint
        | ParamType::Text
        | ParamType::Char
        | ParamType::Path
        | ParamType::Choice(_)
        | ParamType::Id(_) => match occurrences_of(base, all).last() {
            Some(occurrence) => {
                let text = occurrence
                    .value
                    .as_ref()
                    .ok_or_else(|| CliError::MissingValue {
                        flag: base.to_owned(),
                    })?;
                parse_scalar(ty, text).map_err(|expected| CliError::BadValue {
                    flag: base.to_owned(),
                    expected,
                    got: text.clone(),
                })
            }
            None if required => Err(CliError::MissingFlag {
                flag: base.to_owned(),
            }),
            None => Ok(Value::Null),
        },
        ParamType::List(inner) if is_scalar(inner) => {
            let mut items = Vec::new();
            for occurrence in occurrences_of(base, all) {
                let text = occurrence
                    .value
                    .as_ref()
                    .ok_or_else(|| CliError::MissingValue {
                        flag: base.to_owned(),
                    })?;
                items.push(
                    parse_scalar(inner, text).map_err(|expected| CliError::BadValue {
                        flag: base.to_owned(),
                        expected,
                        got: text.clone(),
                    })?,
                );
            }
            if items.is_empty() && !required {
                return Ok(Value::Null);
            }
            Ok(Value::List(items))
        }
        ParamType::Record(fields) => {
            if !required && !any_under(base, all) {
                return Ok(Value::Null);
            }
            let mut args = Args::new();
            for field in *fields {
                let value = read(
                    &join(base, field.name),
                    &field.ty,
                    required && field.required,
                    all,
                )?;
                args.set(field.name, value);
            }
            Ok(Value::Record(args))
        }
        ParamType::Union(variants) => {
            let Some(occurrence) = occurrences_of(base, all).last().copied() else {
                return if required {
                    Err(CliError::MissingFlag {
                        flag: base.to_owned(),
                    })
                } else {
                    Ok(Value::Null)
                };
            };
            let tag = occurrence
                .value
                .as_ref()
                .ok_or_else(|| CliError::MissingValue {
                    flag: base.to_owned(),
                })?;
            let variant = variants
                .iter()
                .find(|variant| variant.tag == *tag)
                .ok_or_else(|| CliError::UnknownArm {
                    flag: base.to_owned(),
                    got: tag.clone(),
                    expected: tags_of(variants),
                })?;
            let mut fields = Args::new();
            for field in variant.fields {
                let name = arm_field(base, variants, variant, field);
                let value = read(&name, &field.ty, field.required, all)?;
                fields.set(field.name, value);
            }
            Ok(Value::tagged(variant.tag, fields))
        }
        ParamType::List(_) | ParamType::Any => {
            if !required && occurrences_of(base, all).is_empty() && !any_under(base, all) {
                Ok(Value::Null)
            } else {
                Err(CliError::NeedsEval {
                    at: base.to_owned(),
                })
            }
        }
    }
}

/// Reads one scalar from its command-line text.
///
/// # Errors
///
/// The expected shape, as a phrase for the message. The caller has the flag name
/// and the text, so this returns only the half it knows.
pub fn parse_scalar(ty: &ParamType, text: &str) -> Result<Value, &'static str> {
    match ty {
        ParamType::Bool => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("`true` or `false`"),
        },
        ParamType::Int => text
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| "an integer"),
        ParamType::Uint | ParamType::Id(_) => text
            .parse::<u64>()
            .ok()
            .and_then(|number| i64::try_from(number).ok())
            .map(Value::Int)
            .ok_or("a non-negative integer"),
        ParamType::Text => Ok(Value::Text(text.to_owned())),
        ParamType::Char => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(_), None) => Ok(Value::Text(text.to_owned())),
                _ => Err("exactly one character"),
            }
        }
        ParamType::Path => {
            if text.is_empty() {
                Err("a path")
            } else {
                Ok(Value::Text(text.to_owned()))
            }
        }
        ParamType::Choice(tags) => {
            if tags.contains(&text) {
                Ok(Value::Text(text.to_owned()))
            } else {
                Err("one of the listed choices")
            }
        }
        ParamType::List(_) | ParamType::Record(_) | ParamType::Union(_) | ParamType::Any => {
            Err("a value with a flag form")
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a command line could not be turned into a [`Call`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// A required flag was not supplied.
    MissingFlag {
        /// Which one.
        flag: String,
    },
    /// A flag that takes a value was supplied without one.
    MissingValue {
        /// Which one.
        flag: String,
    },
    /// A value could not be read as its declared shape.
    BadValue {
        /// Which flag.
        flag: String,
        /// What was expected, as a phrase.
        expected: &'static str,
        /// What arrived.
        got: String,
    },
    /// A union tag that names no arm.
    UnknownArm {
        /// Which flag.
        flag: String,
        /// What arrived.
        got: String,
        /// The arms that exist.
        expected: Vec<&'static str>,
    },
    /// A flag this verb does not declare.
    UnknownFlag {
        /// The verb.
        verb: &'static str,
        /// What arrived.
        flag: String,
    },
    /// A parameter with no flag form — reach it through `--eval`.
    NeedsEval {
        /// The flag path it would have had.
        at: String,
    },
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingFlag { flag } => write!(f, "`--{flag}` is required"),
            Self::MissingValue { flag } => write!(f, "`--{flag}` takes a value"),
            Self::BadValue {
                flag,
                expected,
                got,
            } => write!(f, "`--{flag}` wants {expected}, got `{got}`"),
            Self::UnknownArm {
                flag,
                got,
                expected,
            } => write!(
                f,
                "`--{flag}` has no `{got}`; it is one of {}",
                expected.join(", ")
            ),
            Self::UnknownFlag { verb, flag } => write!(f, "`{verb}` has no `--{flag}`"),
            Self::NeedsEval { at } => {
                write!(f, "`--{at}` has no flag form — call it with `--eval`")
            }
        }
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::registry::lookup;

    fn call(name: &str, occurrences: &[Occurrence]) -> Result<Call, CliError> {
        assemble(&lookup(name).expect("registered"), occurrences)
    }

    /// Whether a shape contains, anywhere inside it, something with no flag
    /// form — a list of non-scalars, or an open record.
    fn has_no_flag_form(ty: &ParamType) -> bool {
        match ty {
            ParamType::Any => true,
            ParamType::List(inner) => !is_scalar(inner) || has_no_flag_form(inner),
            ParamType::Record(fields) => fields.iter().any(|field| has_no_flag_form(&field.ty)),
            ParamType::Union(variants) => variants
                .iter()
                .flat_map(|variant| variant.fields)
                .any(|field| has_no_flag_form(&field.ty)),
            ParamType::Bool
            | ParamType::Int
            | ParamType::Uint
            | ParamType::Text
            | ParamType::Char
            | ParamType::Path
            | ParamType::Choice(_)
            | ParamType::Id(_) => false,
        }
    }

    #[test]
    fn a_union_flattens_into_a_tag_and_its_fields() {
        let mark_seen = verb(&lookup("mark-seen").expect("registered"));
        let target = mark_seen.flag("target").expect("the tag flag exists");
        assert!(matches!(target.value, FlagValue::Arm(_)));
        assert_eq!(target.requirement, Requirement::Always);

        // `path` is `PathBuf` in both the `file` and `explicit` arms — one
        // shape, so one flag, needed when either arm is selected.
        let path = mark_seen
            .flag("target.path")
            .expect("two arms share a path field");
        let Requirement::WithArm { flag, tags } = &path.requirement else {
            panic!("an arm's field is conditionally required");
        };
        assert_eq!(flag, "target");
        assert!(
            tags.contains(&"file") && tags.contains(&"explicit"),
            "{tags:?}"
        );

        // `id` is declared by four arms as four *different* id types, so it is
        // tag-qualified: a flag never means two shapes.
        assert!(mark_seen.flag("target.id").is_none());
        assert!(mark_seen.flag("target.region.id").is_some());
        assert!(mark_seen.flag("target.buffer.id").is_some());
    }

    #[test]
    fn a_flattened_call_round_trips_into_an_action() {
        let call = call(
            "mark-seen",
            &[
                Occurrence::valued("target", "region"),
                Occurrence::valued("target.region.id", "3"),
            ],
        )
        .expect("a region target is expressible as flags");
        let action = Action::from_call(&call.name, &call.args).expect("it decodes");
        assert_eq!(action.to_call(), call);
    }

    #[test]
    fn a_record_flattens_by_dotted_flags() {
        let insert = verb(&lookup("insert").expect("registered"));
        assert!(insert.flag("at.line").is_some(), "{:?}", insert.flags);
        let call = call(
            "insert",
            &[
                Occurrence::valued("at.line", "4"),
                Occurrence::valued("at.column", "0"),
                Occurrence::valued("text", "hello"),
            ],
        )
        .expect("a position is expressible as flags");
        Action::from_call(&call.name, &call.args).expect("it decodes");
    }

    #[test]
    fn a_missing_required_flag_names_itself() {
        let error = call("mark-seen", &[]).expect_err("target is required");
        assert_eq!(
            error,
            CliError::MissingFlag {
                flag: "target".to_owned()
            }
        );
    }

    #[test]
    fn an_unknown_arm_lists_the_arms() {
        let error = call("mark-seen", &[Occurrence::valued("target", "nowhere")])
            .expect_err("`nowhere` is not an arm");
        assert!(matches!(error, CliError::UnknownArm { .. }), "{error:?}");
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        let error = call("mark-seen", &[Occurrence::valued("targt", "region")])
            .expect_err("a typo is not a silent no-op");
        assert!(matches!(error, CliError::UnknownFlag { .. }), "{error:?}");
    }

    #[test]
    fn a_switch_absent_is_false() {
        let call = call(
            "paste",
            &[
                Occurrence::valued("at", "cursor"),
                // `before` omitted.
            ],
        )
        .expect("paste's optional register may be omitted");
        assert_eq!(call.args.get("before"), Some(&Value::Bool(false)));
        assert_eq!(call.args.get("register"), Some(&Value::Null));
        Action::from_call(&call.name, &call.args).expect("it decodes");
    }

    #[test]
    fn a_switch_present_is_true() {
        let call = call(
            "paste",
            &[
                Occurrence::valued("at", "cursor"),
                Occurrence::switch("before"),
            ],
        )
        .expect("before is a switch");
        assert_eq!(call.args.get("before"), Some(&Value::Bool(true)));
    }

    #[test]
    fn every_capability_has_a_verb_with_unique_flags() {
        for capability in capabilities() {
            let generated = verb(&capability);
            assert_eq!(generated.verb, capability.name);
            let mut seen: Vec<&str> = Vec::new();
            for flag in &generated.flags {
                assert!(
                    !seen.contains(&flag.long.as_str()),
                    "`{}` declares `--{}` twice",
                    generated.verb,
                    flag.long
                );
                seen.push(&flag.long);
                assert!(!flag.help.trim().is_empty());
            }
        }
    }

    #[test]
    fn the_eval_only_hole_is_structural_rather_than_arbitrary() {
        // Every parameter with no flag form is a list of non-scalars or an
        // explicitly open shape. If this ever fails, flattening lost a case it
        // used to cover rather than the vocabulary gaining an awkward one.
        for capability in capabilities() {
            let generated = verb(&capability);
            for name in &generated.unreachable {
                let param = capability
                    .params
                    .iter()
                    .find(|param| param.name == *name)
                    .expect("an unreachable parameter is one of the declared ones");
                assert!(
                    has_no_flag_form(&param.ty),
                    "`{}`'s `{name}` has no flag form for no structural reason",
                    generated.verb
                );
            }
        }
    }

    #[test]
    fn an_eval_only_parameter_says_so() {
        let apply = lookup("apply-edits").expect("registered");
        let generated = verb(&apply);
        assert!(generated.needs_eval());
        let error = assemble(&apply, &[]).expect_err("a list of records has no flag form");
        assert!(matches!(error, CliError::NeedsEval { .. }), "{error:?}");
    }
}
