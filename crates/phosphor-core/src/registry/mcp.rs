//! The MCP door — one tool per capability, derived from its row.
//!
//! **Nothing here is a list**, exactly as in [`steel`](super::steel): [`tool`]
//! is a total function of a [`Capability`], so the door cannot be short one.
//! The MCP *server* is `T052` at S6 and has no consumer until then; the schema
//! is generated from S2 anyway, because a door that starts existing three phases
//! late starts by drifting.
//!
//! # Why the schema is plain data and not `serde_json::Value`
//!
//! `phosphor-core` is dependency-free at the floor — the manifest says so and
//! `T078` turns it into a checkpoint item (`docs/TASKS.md`, `CP-2`). [`Schema`]
//! is therefore a small algebraic description of a JSON Schema, not a JSON
//! Schema; `T052`, which owns `rmcp` and therefore owns `serde`, walks it into
//! whatever `rmcp` 3.1.2 wants. That is one match arm per case, and the cases
//! are fixed by [`ParamType`], which is fixed by the vocabulary.
//!
//! # What is deliberately *not* here
//!
//! An **output schema for an Action**. An Action answers an
//! [`Outcome`](crate::action::Outcome), whose shape is the same for every
//! capability and belongs to the server rather than to the row — writing one per
//! capability would be a second definition of `Outcome` to drift from. Queries
//! do get one, because [`QuerySpec::returns`](crate::query::QuerySpec) declares
//! it.
//!
//! Owned by `spine`.

use crate::registry::{
    Capability, CapabilityKind, McpPolicy, Param, ParamType, Since, UnionVariant, capabilities,
};
use crate::value::{TAG_FIELD, Value};

// ---------------------------------------------------------------------------
// The schema language
// ---------------------------------------------------------------------------

/// One property of an [`Schema::Object`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// The JSON property name — the parameter or field name, verbatim, because
    /// that is what [`Args`](crate::value::Args) is keyed by.
    pub name: &'static str,
    /// One line, in the product's voice. This is what an agent reads.
    pub description: &'static str,
    /// Its shape.
    pub schema: Schema,
    /// Whether it appears in the object's `required` list.
    pub required: bool,
}

/// One arm of a [`Schema::OneOf`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    /// The tag, carried in [`TAG_FIELD`].
    pub tag: &'static str,
    /// When this arm applies.
    pub description: &'static str,
    /// The arm's own object schema, including the constant tag property.
    pub schema: Schema,
}

/// A JSON Schema, as far as this vocabulary can reach.
///
/// Every case maps onto one JSON Schema object with no ambiguity, and there is
/// exactly one case per [`ParamType`]. See [`schema`] for the mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schema {
    /// `{"type": "boolean"}`.
    Boolean,
    /// `{"type": "integer"}`, with `minimum` when the payload type is unsigned.
    Integer {
        /// `minimum`, or [`None`] for a signed value.
        minimum: Option<i64>,
    },
    /// `{"type": "integer", "minimum": 0}` for an opaque identifier, with a
    /// description naming what it identifies.
    Id {
        /// What it identifies — `"region"`, `"thread"`.
        of: &'static str,
    },
    /// `{"type": "string"}`, with length bounds and an optional `format`.
    Text {
        /// `minLength`.
        min_length: Option<u32>,
        /// `maxLength`.
        max_length: Option<u32>,
        /// A non-validating hint. `Some("path")` for
        /// [`ParamType::Path`]; consumers that do not know it must ignore it,
        /// which JSON Schema requires of unknown formats anyway.
        format: Option<&'static str>,
    },
    /// `{"type": "string", "enum": [...]}`.
    Enum(&'static [&'static str]),
    /// `{"type": "array", "items": …}`.
    Array {
        /// The element schema.
        items: Box<Schema>,
    },
    /// `{"type": "object", "properties": …, "required": …}`.
    Object {
        /// The properties, in declaration order.
        properties: Vec<Property>,
        /// Whether unknown properties are permitted. `false` everywhere the
        /// shape is ours; `true` only for [`Schema::Unconstrained`]'s neighbours.
        additional_properties: bool,
    },
    /// `{"oneOf": [...]}` over a tagged union.
    OneOf {
        /// The property carrying the tag — always [`TAG_FIELD`], named here so a
        /// generator never spells it a second time.
        discriminator: &'static str,
        /// The arms, in declaration order.
        arms: Vec<Arm>,
    },
    /// `{"type": "object"}` with no constraint — [`ParamType::Any`], whose one
    /// legitimate use is a surface whose parameters belong to the surface.
    Unconstrained,
}

impl Schema {
    /// The `required` list of an object schema, in declaration order.
    #[must_use]
    pub fn required(&self) -> Vec<&'static str> {
        match self {
            Self::Object { properties, .. } => properties
                .iter()
                .filter(|property| property.required)
                .map(|property| property.name)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// The schema for a declared shape. Total over [`ParamType`].
#[must_use]
pub fn schema(ty: &ParamType) -> Schema {
    match ty {
        ParamType::Bool => Schema::Boolean,
        ParamType::Int => Schema::Integer { minimum: None },
        ParamType::Uint => Schema::Integer { minimum: Some(0) },
        ParamType::Text => Schema::Text {
            min_length: None,
            max_length: None,
            format: None,
        },
        ParamType::Char => Schema::Text {
            min_length: Some(1),
            max_length: Some(1),
            format: None,
        },
        ParamType::Path => Schema::Text {
            min_length: Some(1),
            max_length: None,
            format: Some("path"),
        },
        ParamType::Choice(tags) => Schema::Enum(tags),
        ParamType::List(inner) => Schema::Array {
            items: Box::new(schema(inner)),
        },
        ParamType::Record(fields) => object(fields),
        ParamType::Union(variants) => Schema::OneOf {
            discriminator: TAG_FIELD,
            arms: variants.iter().map(arm).collect(),
        },
        ParamType::Id(of) => Schema::Id { of },
        ParamType::Any => Schema::Unconstrained,
    }
}

/// An object schema over a parameter list.
#[must_use]
pub fn object(params: &'static [Param]) -> Schema {
    Schema::Object {
        properties: params.iter().map(property).collect(),
        additional_properties: false,
    }
}

fn property(param: &'static Param) -> Property {
    Property {
        name: param.name,
        description: param.doc,
        schema: schema(&param.ty),
        required: param.required,
    }
}

/// One arm of a union, with its tag pinned to a single-value enum.
///
/// The tag is a property of the arm rather than a sibling of `oneOf` because
/// that is the form every JSON Schema validator agrees on, and because
/// [`Value::tagged`](crate::value::Value::tagged) puts it in the record.
fn arm(variant: &'static UnionVariant) -> Arm {
    let tag = Property {
        name: TAG_FIELD,
        description: "which arm this is",
        schema: Schema::Enum(core::slice::from_ref(&variant.tag)),
        required: true,
    };
    let properties = core::iter::once(tag)
        .chain(variant.fields.iter().map(property))
        .collect();
    Arm {
        tag: variant.tag,
        description: variant.doc,
        schema: Schema::Object {
            properties,
            additional_properties: false,
        },
    }
}

// ---------------------------------------------------------------------------
// The tool
// ---------------------------------------------------------------------------

/// One capability as the MCP door sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// The MCP tool name: [`Capability::mcp_name`], `phosphor/`-prefixed.
    pub name: String,
    /// The canonical door name, for dispatch back into
    /// [`Action::from_call`](crate::action::Action::from_call).
    pub capability: &'static str,
    /// One line, in the product's voice — the tool description an agent reads.
    pub description: &'static str,
    /// The domain, for grouping a 150-tool list into something legible.
    pub domain: &'static str,
    /// `true` for a query. The `readOnlyHint` annotation, and derivable rather
    /// than declared: a query may not mutate (`query.rs` says why).
    pub read_only: bool,
    /// What the door does with it before an `init.scm` rule widens it.
    pub policy: McpPolicy,
    /// The phase and task that implement it — what
    /// [`Refusal::NotYetImplemented`](crate::action::Refusal::NotYetImplemented)
    /// reports.
    pub since: Since,
    /// Always [`Schema::Object`].
    pub input_schema: Schema,
    /// The result shape for a query; [`None`] for an Action — see the module
    /// docs.
    pub output_schema: Option<Schema>,
    /// A canonical example call, from [`sample_args`](crate::registry::sample_args).
    /// Always a [`Value::Record`]. The difference between an agent guessing at
    /// an argument's shape and reading it.
    pub example: Value,
}

/// This capability's MCP tool. Total — see the module docs.
#[must_use]
pub fn tool(capability: &Capability) -> Tool {
    Tool {
        name: capability.mcp_name(),
        capability: capability.name,
        description: capability.doc,
        domain: capability.domain,
        read_only: capability.kind == CapabilityKind::Query,
        policy: capability.mcp,
        since: capability.since,
        input_schema: object(capability.params),
        output_schema: capability.returns.as_ref().map(schema),
        example: Value::Record(capability.sample_args()),
    }
}

/// Every capability's tool, in registry order.
#[must_use]
pub fn tools() -> Vec<Tool> {
    capabilities().iter().map(tool).collect()
}

// ---------------------------------------------------------------------------
// Well-formedness
// ---------------------------------------------------------------------------

/// Why a generated tool would not be a usable MCP tool.
///
/// `T024`'s MCP third is *"the schema is generated and well-formed for every
/// capability"* — this is the definition of well-formed it checks against, kept
/// beside the generator so the two cannot describe different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// The tool name is not `phosphor/`-prefixed, or the suffix is empty.
    ToolName {
        /// What was generated.
        name: String,
    },
    /// A description an agent cannot act on.
    EmptyDescription {
        /// Where — the tool name, or `tool::property`.
        at: String,
    },
    /// The top level of an input schema must be an object; an agent sends a
    /// property bag.
    InputNotAnObject,
    /// Two properties of one object share a name, so one is unreachable.
    DuplicateProperty {
        /// Where.
        at: String,
        /// The repeated name.
        name: &'static str,
    },
    /// An enum or a `oneOf` with nothing in it is not callable.
    EmptyChoice {
        /// Where.
        at: String,
    },
    /// Two arms of one union share a tag, so the discriminator does not
    /// discriminate.
    DuplicateArm {
        /// Where.
        at: String,
        /// The repeated tag.
        tag: &'static str,
    },
}

impl core::fmt::Display for Malformed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ToolName { name } => write!(f, "`{name}` is not a `phosphor/` tool name"),
            Self::EmptyDescription { at } => write!(f, "`{at}` has no description"),
            Self::InputNotAnObject => write!(f, "the input schema is not an object"),
            Self::DuplicateProperty { at, name } => {
                write!(f, "`{at}` declares `{name}` twice")
            }
            Self::EmptyChoice { at } => write!(f, "`{at}` offers no choices"),
            Self::DuplicateArm { at, tag } => write!(f, "`{at}` declares the arm `{tag}` twice"),
        }
    }
}

impl std::error::Error for Malformed {}

/// Checks a generated tool against [`Malformed`].
///
/// # Errors
///
/// The first problem found, named with enough context to fix it.
pub fn check(tool: &Tool) -> Result<(), Malformed> {
    let suffix = tool
        .name
        .strip_prefix("phosphor/")
        .ok_or_else(|| Malformed::ToolName {
            name: tool.name.clone(),
        })?;
    if suffix.is_empty() {
        return Err(Malformed::ToolName {
            name: tool.name.clone(),
        });
    }
    if tool.description.trim().is_empty() {
        return Err(Malformed::EmptyDescription {
            at: tool.name.clone(),
        });
    }
    if !matches!(tool.input_schema, Schema::Object { .. }) {
        return Err(Malformed::InputNotAnObject);
    }
    check_schema(&tool.name, &tool.input_schema)?;
    if let Some(output) = &tool.output_schema {
        check_schema(&format!("{}::result", tool.name), output)?;
    }
    Ok(())
}

fn check_schema(at: &str, schema: &Schema) -> Result<(), Malformed> {
    match schema {
        Schema::Object { properties, .. } => {
            let mut seen: Vec<&'static str> = Vec::with_capacity(properties.len());
            for property in properties {
                if seen.contains(&property.name) {
                    return Err(Malformed::DuplicateProperty {
                        at: at.to_owned(),
                        name: property.name,
                    });
                }
                seen.push(property.name);
                if property.description.trim().is_empty() {
                    return Err(Malformed::EmptyDescription {
                        at: format!("{at}::{}", property.name),
                    });
                }
                check_schema(&format!("{at}::{}", property.name), &property.schema)?;
            }
            Ok(())
        }
        Schema::OneOf { arms, .. } => {
            if arms.is_empty() {
                return Err(Malformed::EmptyChoice { at: at.to_owned() });
            }
            let mut seen: Vec<&'static str> = Vec::with_capacity(arms.len());
            for arm in arms {
                if seen.contains(&arm.tag) {
                    return Err(Malformed::DuplicateArm {
                        at: at.to_owned(),
                        tag: arm.tag,
                    });
                }
                seen.push(arm.tag);
                check_schema(&format!("{at}::{}", arm.tag), &arm.schema)?;
            }
            Ok(())
        }
        Schema::Enum(tags) => {
            if tags.is_empty() {
                Err(Malformed::EmptyChoice { at: at.to_owned() })
            } else {
                Ok(())
            }
        }
        Schema::Array { items } => check_schema(&format!("{at}[]"), items),
        Schema::Boolean | Schema::Integer { .. } | Schema::Id { .. } | Schema::Text { .. } => {
            Ok(())
        }
        Schema::Unconstrained => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::lookup;

    #[test]
    fn every_generated_tool_is_well_formed() {
        for tool in tools() {
            check(&tool).unwrap_or_else(|error| panic!("{}: {error}", tool.name));
        }
    }

    #[test]
    fn a_union_becomes_a_discriminated_one_of() {
        let mark_seen = tool(&lookup("mark-seen").expect("registered"));
        let Schema::Object { properties, .. } = &mark_seen.input_schema else {
            panic!("an input schema is an object");
        };
        let target = properties
            .iter()
            .find(|property| property.name == "target")
            .expect("mark-seen takes a target");
        let Schema::OneOf {
            discriminator,
            arms,
        } = &target.schema
        else {
            panic!("Target is a tagged union");
        };
        assert_eq!(*discriminator, TAG_FIELD);
        let region = arms
            .iter()
            .find(|arm| arm.tag == "region")
            .expect("a region is a target");
        let Schema::Object { properties, .. } = &region.schema else {
            panic!("an arm is an object");
        };
        assert_eq!(properties[0].name, TAG_FIELD);
        assert_eq!(properties[0].schema, Schema::Enum(&["region"]));
        assert!(properties[0].required);
    }

    #[test]
    fn a_query_declares_a_result_and_an_action_does_not() {
        assert!(
            tool(&lookup("unseen-regions").expect("registered"))
                .output_schema
                .is_some()
        );
        assert!(
            tool(&lookup("mark-seen").expect("registered"))
                .output_schema
                .is_none()
        );
    }

    #[test]
    fn a_query_is_read_only_and_allowed() {
        let unseen = tool(&lookup("unseen-regions").expect("registered"));
        assert!(unseen.read_only);
        assert_eq!(unseen.policy, McpPolicy::Allow);
    }

    #[test]
    fn the_users_keyboard_is_denied_at_this_door() {
        // action.rs's `feeds_the_keyboard` decides this; the schema is still
        // generated, which is the point — a denied capability is registered and
        // openable by a rule the user wrote, not absent (T061).
        let eval = tool(&lookup("eval").expect("registered"));
        assert_eq!(eval.policy, McpPolicy::Deny);
        check(&eval).expect("a denied tool is still a well-formed tool");
    }

    #[test]
    fn an_unsigned_parameter_carries_its_floor() {
        assert_eq!(
            schema(&ParamType::Uint),
            Schema::Integer { minimum: Some(0) }
        );
        assert_eq!(schema(&ParamType::Int), Schema::Integer { minimum: None });
    }

    #[test]
    fn every_example_is_a_record() {
        for tool in tools() {
            assert!(
                matches!(tool.example, Value::Record(_)),
                "{}'s example is not a property bag",
                tool.name
            );
        }
    }

    #[test]
    fn a_malformed_tool_is_caught() {
        // The generator cannot produce this; the checker is what T024 leans on,
        // so prove it bites rather than trusting that it would.
        let mut planted = tool(&lookup("mark-seen").expect("registered"));
        planted.name = "mark-seen".to_owned();
        assert!(matches!(check(&planted), Err(Malformed::ToolName { .. })));
    }
}
