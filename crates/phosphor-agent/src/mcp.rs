//! The MCP server (`T052`) — invariant 2's third door, and the agent's.
//!
//! **Nothing here is a list.** `phosphor_core::registry::mcp::tool` is a total
//! function of a capability row, so [`tools`] is one `map` over
//! [`capabilities`] and this module contains no capability name at all. Adding
//! an Action to the `actions!` table adds a tool here with no edit, which is
//! `T020`'s *"by construction"* claim as it lands on this door —
//! `scripts/lint-one-registry.sh` holds it.
//!
//! # Where the schema is turned into JSON, and why it is here
//!
//! `phosphor-core` is dependency-free at the floor, so its
//! [`Schema`] is *a small algebraic
//! description of* a JSON Schema rather than a JSON Schema. Its own module
//! header names this crate as the one that walks it: *"`T052`, which owns
//! `rmcp` and therefore owns `serde`, walks it into whatever `rmcp` 3.1.2
//! wants. That is one match arm per case, and the cases are fixed by
//! `ParamType`, which is fixed by the vocabulary."* [`schema_json`] is that
//! walk, and it is exhaustive on purpose — a new `ParamType` breaks it at
//! compile time rather than emitting an object an agent cannot construct an
//! argument from.
//!
//! # What this crate does *not* decide
//!
//! Whether a tool call is allowed, and what it does. Both are the binary's:
//! [`Editor`] is one method wide, and `crates/phosphor/src/door.rs`'s
//! `answer` is what implements it — the same path the CLI door runs, which is
//! what makes *"the same capability works from Steel and CLI"* structural
//! rather than a thing to keep in step. A policy check belongs in `deliver`,
//! beside every other door's.
//!
//! Owned by `agent`.

use std::borrow::Cow;
use std::sync::Arc;

use phosphor_core::registry::mcp::{Schema, Tool as Row, tool};
use phosphor_core::registry::{Capability, capabilities};
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, JsonObject,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServiceExt};

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// What an editor does with a tool call.
///
/// One method wide, deliberately — the same width `door::Evaluate` is, and for
/// the same reason: this crate owns the transport and the schema, and the
/// binary owns what a capability *means*. Anything wider would be this crate
/// learning about the editor.
///
/// `Err` is a call that never reached the editor — a name no row has, arguments
/// that do not decode. `Ok` is what the editor said, refusals included: a
/// refusal is an answer, not a transport failure, and an agent that could not
/// tell them apart would retry the wrong ones.
pub trait Editor: Send + Sync + 'static {
    /// Runs `capability` with `args` and answers what the editor said.
    ///
    /// # Errors
    ///
    /// The call was malformed: an unknown tool, or arguments the vocabulary
    /// cannot decode.
    fn call(&self, capability: &str, args: &JsonObject) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// The tool list
// ---------------------------------------------------------------------------

/// Every capability, as an MCP tool.
///
/// The order is the registry's, which is the vocabulary's declaration order,
/// which means a diff of this list reads like a diff of `actions!`.
#[must_use]
pub fn tools() -> Vec<Tool> {
    capabilities().iter().map(|row| wire(&tool(row))).collect()
}

/// The registry row for an MCP tool name, or [`None`].
///
/// The inverse of [`Capability::mcp_name`], done by asking every row rather
/// than by parsing the `phosphor/` prefix off the name: the prefix is the
/// registry's to spell and a second speller here is a second thing to get
/// wrong.
#[must_use]
pub fn row_for(name: &str) -> Option<Capability> {
    capabilities()
        .iter()
        .find(|row| row.mcp_name() == name)
        .copied()
}

/// One row, in `rmcp`'s shape.
fn wire(row: &Row) -> Tool {
    // Through `rmcp`'s own constructors: both types are `#[non_exhaustive]`, so
    // a struct literal here would stop compiling the first time the protocol
    // grows a field — which is the whole reason the attribute is on them.
    let mut built = Tool::new(
        Cow::Owned(row.name.clone()),
        Cow::Borrowed(row.description),
        Arc::new(schema_json(&row.input_schema)),
    );
    built.output_schema = row
        .output_schema
        .as_ref()
        .map(|schema| Arc::new(schema_json(schema)));
    // A query may not mutate, so this is derived rather than declared — see
    // `registry::mcp::Tool::read_only`.
    built.annotations = Some(ToolAnnotations::new().read_only(row.read_only));
    built
}

/// [`Schema`] as a JSON Schema object.
///
/// **Answers an object rather than a `Value`, because every case is one.** The
/// first version returned `serde_json::Value` and had an `object_of` beside it
/// with a `_ => JsonObject::new()` arm for the case that was not — and there is
/// no such case. A test written to press that arm passed a `Schema::Boolean`
/// and got `{"type": "boolean"}`, which is an object; the branch was
/// unreachable and the test asserting it was asserting a falsehood. The type
/// says the true thing now and there is no branch to be wrong about.
///
/// Exhaustive on the schema, and the exhaustiveness is the point: a case added
/// to [`Schema`] stops this compiling instead of quietly emitting `{}` for it.
fn schema_json(schema: &Schema) -> JsonObject {
    // **Property order matters and is not decoration.** `parity.rs` asserts
    // the offered properties equal the declared parameters *as a sequence*.
    // `serde_json`'s `Map` is a `BTreeMap` unless `preserve_order` is on — and
    // it **is** on across this workspace, because the ACP crates ask for it
    // (`docs/OPEN-QUESTIONS.md` §51). So insertion order is what ships, which
    // is declaration order. Recorded because the two facts are three
    // dependencies apart and neither one mentions the other.
    use serde_json::{Value as Json, json};
    let mut out = JsonObject::new();
    match schema {
        Schema::Boolean => {
            out.insert("type".to_owned(), json!("boolean"));
        }
        Schema::Integer { minimum } => {
            out.insert("type".to_owned(), json!("integer"));
            if let Some(least) = minimum {
                out.insert("minimum".to_owned(), json!(least));
            }
        }
        // The description is the whole of what makes an opaque id legible to an
        // agent: `{"type": "integer"}` says nothing, and *"a region"* says what
        // to go and find one of.
        Schema::Id { of } => {
            out.insert("type".to_owned(), json!("integer"));
            out.insert("minimum".to_owned(), json!(0));
            out.insert(
                "description".to_owned(),
                json!(format!("an opaque {of} id")),
            );
        }
        Schema::Text {
            min_length,
            max_length,
            format,
        } => {
            out.insert("type".to_owned(), json!("string"));
            if let Some(least) = min_length {
                out.insert("minLength".to_owned(), json!(least));
            }
            if let Some(most) = max_length {
                out.insert("maxLength".to_owned(), json!(most));
            }
            if let Some(hint) = format {
                out.insert("format".to_owned(), json!(hint));
            }
        }
        Schema::Enum(choices) => {
            out.insert("type".to_owned(), json!("string"));
            out.insert("enum".to_owned(), json!(choices));
        }
        Schema::Array { items } => {
            out.insert("type".to_owned(), json!("array"));
            out.insert("items".to_owned(), Json::Object(schema_json(items)));
        }
        Schema::Object {
            properties,
            additional_properties,
        } => {
            let mut fields = JsonObject::new();
            let mut required = Vec::new();
            for property in properties {
                let mut described = schema_json(&property.schema);
                described.insert("description".to_owned(), json!(property.description));
                fields.insert(property.name.to_owned(), Json::Object(described));
                if property.required {
                    required.push(property.name);
                }
            }
            out.insert("type".to_owned(), json!("object"));
            out.insert("properties".to_owned(), Json::Object(fields));
            out.insert("required".to_owned(), json!(required));
            out.insert(
                "additionalProperties".to_owned(),
                json!(additional_properties),
            );
        }
        Schema::OneOf {
            discriminator,
            arms,
        } => {
            let described: Vec<Json> = arms
                .iter()
                .map(|arm| {
                    let mut map = schema_json(&arm.schema);
                    map.insert("description".to_owned(), json!(arm.description));
                    Json::Object(map)
                })
                .collect();
            out.insert("oneOf".to_owned(), Json::Array(described));
            out.insert(
                "discriminator".to_owned(),
                json!({"propertyName": discriminator}),
            );
        }
        // *"An agent-built surface's parameters belong to the surface"* — the
        // one place the vocabulary declines to say a shape.
        Schema::Unconstrained => {
            out.insert("type".to_owned(), json!("object"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The server
// ---------------------------------------------------------------------------

/// What this server calls itself in the MCP handshake.
const SERVER_NAME: &str = "phosphor";

/// The MCP server over one editor.
///
/// Cloneable and cheap: `rmcp` wants the handler by value and the editor is
/// behind an [`Arc`], so a clone is a refcount rather than a second editor.
#[derive(Debug, Clone)]
pub struct Server<E> {
    editor: Arc<E>,
}

impl<E: Editor> Server<E> {
    /// A server over `editor`.
    pub fn new(editor: E) -> Self {
        Self {
            editor: Arc::new(editor),
        }
    }

    /// Serves MCP on this process's stdin and stdout until the client goes.
    ///
    /// # Errors
    ///
    /// The transport failed — the client's pipe closed mid-message, or the
    /// handshake did not complete.
    pub async fn serve_stdio(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let running = self.serve(rmcp::transport::stdio()).await?;
        running.waiting().await?;
        Ok(())
    }
}

impl<E: Editor> ServerHandler for Server<E> {
    fn get_info(&self) -> ServerInfo {
        // Built field by field off `Default`, not as a literal: `ServerInfo`
        // and `Implementation` are both `#[non_exhaustive]`, which is the
        // protocol reserving room to grow.
        // Assigned field by field off `Default`, never as a literal — not
        // even with `..`: `ServerInfo` and `Implementation` are both
        // `#[non_exhaustive]`, which is the protocol reserving room to grow and
        // is a rule about the *expression*, not about the fields named in it.
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info.name = SERVER_NAME.to_owned();
        info.server_info.version = env!("CARGO_PKG_VERSION").to_owned();
        info.instructions = Some(
            "The editor's own vocabulary. Every tool is one capability; a \
                 refusal naming a task means the capability is declared and not \
                 built yet."
                .to_owned(),
        );
        info
    }

    /// Every tool, every time.
    ///
    /// **Unpaginated on purpose.** The list is the registry and the registry is
    /// a `const` slice — there is nothing to stream and no cursor that could
    /// mean anything, so a page-two request would have to invent an empty
    /// answer. A client that sends a cursor gets the whole list, which is the
    /// truthful response to *"show me the rest"* when there is no rest.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let empty = JsonObject::new();
        let args = request.arguments.as_ref().unwrap_or(&empty);
        // **A malformed call is a tool error and a refusal is not.** A
        // refusal is what the editor *said* — `T054 is not built yet` is an
        // answer — while an unknown tool or an argument that will not decode
        // never reached it. An agent that could not tell them apart would retry
        // the ones that can never work and give up on the ones that will work
        // as soon as a task lands.
        Ok(CallToolResponse::Complete(
            match self.editor.call(&request.name, args) {
                Ok(said) => CallToolResult::success(vec![ContentBlock::text(said)]),
                Err(why) => CallToolResult::error(vec![ContentBlock::text(why)]),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Schema, row_for, schema_json, tools};
    use phosphor_core::registry::capabilities;

    /// **The list is the registry, and nothing here is a list.**
    #[test]
    fn every_capability_is_a_tool() {
        let offered = tools();
        assert_eq!(offered.len(), capabilities().len());
        for (tool, row) in offered.iter().zip(capabilities()) {
            assert_eq!(tool.name, row.mcp_name(), "in the registry's own order");
        }
    }

    /// The inverse holds, which is what dispatch depends on.
    #[test]
    fn a_tool_name_finds_its_row() {
        for row in capabilities() {
            let found = row_for(&row.mcp_name()).expect("every tool has a row");
            assert_eq!(found.name, row.name);
        }
        assert!(row_for("phosphor/no-such-tool").is_none());
        // Not the door name, which is what a client that stripped the prefix
        // itself would send.
        assert!(row_for("mark-seen").is_none());
    }

    /// **Declaration order survives the walk**, which `parity.rs` asserts as a
    /// sequence — see [`super::object`]'s note on why that is three
    /// dependencies away from being an accident.
    #[test]
    fn properties_are_offered_in_declaration_order() {
        for tool in tools() {
            let row = row_for(&tool.name).expect("a row");
            let properties = tool.input_schema.get("properties").expect("an object");
            let offered: Vec<&str> = properties
                .as_object()
                .expect("properties is an object")
                .keys()
                .map(String::as_str)
                .collect();
            let declared: Vec<&str> = row.params.iter().map(|param| param.name).collect();
            assert_eq!(offered, declared, "for {}", tool.name);
        }
    }

    /// Every input schema is an object that refuses what the vocabulary never
    /// declared — the property an agent's malformed call fails on.
    #[test]
    fn an_input_schema_is_a_closed_object() {
        for tool in tools() {
            assert_eq!(
                tool.input_schema.get("type").and_then(|kind| kind.as_str()),
                Some("object"),
                "for {}",
                tool.name
            );
            assert_eq!(
                tool.input_schema
                    .get("additionalProperties")
                    .and_then(serde_json::Value::as_bool),
                Some(false),
                "for {}",
                tool.name
            );
        }
    }

    /// **Every case answers something a client can read**, which is what
    /// makes the object return type honest rather than merely convenient.
    ///
    /// This replaced a test that pressed an unreachable fallback — see
    /// [`schema_json`]'s own note. A scalar is `{"type": …}` and a union is
    /// `{"oneOf": …}`; nothing answers `{}`.
    #[test]
    fn every_schema_case_answers_a_shape() {
        let cases = [
            Schema::Boolean,
            Schema::Integer { minimum: None },
            Schema::Id { of: "region" },
            Schema::Text {
                min_length: None,
                max_length: None,
                format: None,
            },
            Schema::Enum(&["one", "two"]),
            Schema::Array {
                items: Box::new(Schema::Boolean),
            },
            Schema::Unconstrained,
        ];
        for case in cases {
            let drawn = schema_json(&case);
            assert!(
                drawn.contains_key("type") || drawn.contains_key("oneOf"),
                "{case:?} answered {drawn:?}"
            );
        }
    }

    /// The two scalar shapes that carry more than a type.
    #[test]
    fn an_id_says_what_it_identifies_and_a_bound_survives() {
        let id = schema_json(&Schema::Id { of: "region" });
        assert_eq!(id["type"], "integer");
        assert_eq!(id["minimum"], 0);
        assert!(
            id["description"]
                .as_str()
                .is_some_and(|it| it.contains("region")),
            "an opaque integer says what to go and find one of: {id:?}"
        );

        let counted = schema_json(&Schema::Integer { minimum: Some(0) });
        assert_eq!(counted["minimum"], 0);
        let signed = schema_json(&Schema::Integer { minimum: None });
        assert!(signed.get("minimum").is_none(), "a delta has no floor");
    }
}
