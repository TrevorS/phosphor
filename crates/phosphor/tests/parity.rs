//! `T024` — the door-parity test.
//!
//! One walk over [`registrations`], and at every row all three doors are
//! *exercised* rather than counted. Enumeration is the point: a hand-written
//! list of capabilities rots the first time someone adds one, and the registry
//! is the only list that cannot.
//!
//! # Why this lives in the binary crate
//!
//! `crates/phosphor` is the only crate that can see all three doors at once.
//! `phosphor-core` holds the table and generates the MCP schema but embeds no
//! VM and is not a program; `phosphor-steel` has the VM but no CLI. The binary
//! depends on both **and** is the CLI door — reached here as a process through
//! `CARGO_BIN_EXE_phosphor`, the path cargo hands an integration test, so
//! nothing is assumed about `$PATH` or about a build having been installed.
//!
//! # What "reachable" means at `S2`
//!
//! `T024` is explicit that the answer differs per door before `S6`, and this
//! file asserts exactly what exists now (`docs/TASKS.md`, `T024`):
//!
//! * **Steel** — end to end. A real [`Runtime`], the whole vocabulary
//!   installed, and one scheme call per capability built from that capability's
//!   own canonical example. The call has to reach the [`Host`] carrying *that*
//!   capability's name; a binding that decoded to a neighbour would fail here.
//! * **CLI** — end to end, as a process. The verb's own generated flags are
//!   filled from the same canonical examples, the binary runs with no terminal,
//!   and the line on stdout has to be the refusal naming **that row's** task.
//!   That id is read off the registry (`door.rs` derives it from
//!   `action.spec().since.task`), so a verb that dispatched to the wrong
//!   capability prints the wrong task and fails.
//! * **MCP** — the schema is generated and well-formed, because the *server* is
//!   `T052` and there is no consumer yet. Well-formed here is not "a JSON
//!   object": the schema must offer one property per declared parameter in
//!   declaration order, mark exactly the required ones required, pin every
//!   union arm's discriminator to its own tag, refuse undeclared properties,
//!   and — the load-bearing one — the tool's canonical `example` must
//!   **validate against the tool's own input schema** and then **decode into
//!   the capability**. An empty-property object would fail all three.
//!
//! `T052` upgrades the MCP third to a live round-trip by replacing the body of
//! [`mcp_door`] with a server call. The shape of this file does not change: one
//! enumeration, one check per [`Door`], a `match` that a fourth door would
//! break at compile time.
//!
//! # What this catches that the type system does not
//!
//! Each door is a total function of a row (`phosphor_core::registry`), so a
//! capability cannot be *absent* from a door — and `scripts/lint-one-registry.sh`
//! catches the other half, a door that grows a table or a special case of its
//! own. What is left, and what this file is for, is a capability that is
//! registered at a door and not *usable* through it: an identifier that is not
//! bound, a verb whose flags cannot reassemble the call, a schema an agent
//! cannot construct an argument from. Those are the shapes a one-door Action
//! actually takes once registration is by construction.
//!
//! Owned by `spine`.

use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

use phosphor_core::action::{Action, Outcome, Receipt, Request, RuntimeAction};
use phosphor_core::query::{Answer, Answers, Query, QueryError, Revision};
use phosphor_core::registry::cli::{FlagValue, Verb};
use phosphor_core::registry::mcp::{Schema, Tool};
use phosphor_core::registry::steel::{Binding, alias_bindings};
use phosphor_core::registry::{
    Capability, CapabilityKind, Door, ParamType, Registration, capabilities, lookup, mcp,
    registrations, sample,
};
use phosphor_core::value::{TAG_FIELD, Value};
use phosphor_steel::host::Host;
use phosphor_steel::runtime::Runtime;

/// Whether this capability's implementation *is* a runtime.
///
/// One capability does not answer a task id at the CLI door: `eval`'s
/// implementation is the VM, and `T022` wired `main.rs` to hand the door one.
/// So it answers whatever the VM made of the source — since `T100` a `#raised`
/// line, because the canonical example is not a defined identifier — rather
/// than `not built yet — … builds it`, and the door check reads that instead.
///
/// Detected structurally — the Action that is a runtime — rather than by naming
/// the capability, so this stays one exception in the arrangement and not one in
/// a list.
fn is_the_vm(capability: &Capability) -> bool {
    matches!(
        Action::from_call(capability.name, &capability.sample_args()),
        Ok(Action::Runtime(RuntimeAction::Eval { .. }))
    )
}

// ---------------------------------------------------------------------------
// The recording host
// ---------------------------------------------------------------------------

/// A [`Host`] that answers everything and remembers what arrived.
///
/// Not a mock of the store: the point is only *which capability crossed the
/// barrier*, which is the thing a door can get wrong. Answering rather than
/// refusing keeps every capability on one path, so a failure is a parity
/// failure and never the `Detached` host's own refusal.
#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<&'static str>>);

impl Recorder {
    fn record(&self, capability: &'static str) {
        if let Ok(mut log) = self.0.lock() {
            log.push(capability);
        }
    }

    fn take(&self) -> Vec<&'static str> {
        self.0
            .lock()
            .map(|mut log| core::mem::take(&mut *log))
            .unwrap_or_default()
    }
}

impl Answers for Recorder {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        self.record(query.spec().name);
        Ok(Answer {
            value: Value::Null,
            revision: Revision::default(),
        })
    }
}

impl Host for Recorder {
    fn apply(&self, request: &Request) -> Outcome {
        let capability = request.action.spec().name;
        self.record(capability);
        Outcome::Done(Receipt::ok(capability))
    }
}

// ---------------------------------------------------------------------------
// The three doors
// ---------------------------------------------------------------------------

/// The doors, opened once and reused across the enumeration.
struct Doors {
    runtime: Runtime,
    seen: Arc<Recorder>,
}

impl Doors {
    fn open() -> Self {
        let seen = Arc::new(Recorder::default());
        let host: Arc<dyn Host> = Arc::clone(&seen) as Arc<dyn Host>;
        // `None` for the runtime tree: the vocabulary is installed either way
        // (`phosphor_steel::runtime`), and reading `$PHOSPHOR_RUNTIME` or the
        // user's config here would make what this test proves depend on the
        // machine it runs on.
        Self {
            runtime: Runtime::boot(None, host),
            seen,
        }
    }

    /// One capability at one door.
    ///
    /// A `match` on [`Door`] rather than three loops, so a fourth door is a
    /// compile error here and not a third of the vocabulary nobody checked.
    fn check(&mut self, door: Door, registration: &Registration) -> Result<(), String> {
        let capability = &registration.capability;
        match door {
            Door::Steel => self.steel_door(capability, &registration.steel),
            Door::Mcp => mcp_door(capability, &registration.mcp),
            Door::Cli => cli_door(capability, &registration.cli),
        }
    }

    /// The Steel third: the binding is bound, callable, and reaches the host.
    fn steel_door(&mut self, capability: &Capability, binding: &Binding) -> Result<(), String> {
        self.runtime
            .global(&binding.name)
            .map_err(|error| format!("`{}` is not bound in the VM — {error}", binding.name))?;

        let source = scheme_call(capability, binding)?;
        let _ = self.seen.take();

        match self.runtime.evaluate(&source) {
            Outcome::Done(_) => {}
            // A call that did not run: a wrong arity, an argument the barrier
            // refused, an identifier that is not a procedure. `T100` made this
            // its own case — it used to arrive as `Refusal::Declined`, which
            // meant this walk could not tell a raise from a rule saying no, and
            // the two arms below were one guess apart.
            Outcome::Raised(raised) => {
                return Err(format!("`{source}` did not run — {}", raised.why()));
            }
            Outcome::Refused(refusal) => {
                return Err(format!("`{source}` was refused — {refusal:?}"));
            }
        }

        let arrived = self.seen.take();
        if arrived != vec![capability.name] {
            return Err(format!(
                "`{source}` reached the host as {arrived:?}, not as `{}`",
                capability.name
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Steel: a call built from the capability's own canonical example
// ---------------------------------------------------------------------------

/// The scheme call this capability's own example makes.
fn scheme_call(capability: &Capability, binding: &Binding) -> Result<String, String> {
    let args = capability.sample_args();
    let mut source = format!("({}", binding.name);
    for param in capability.params {
        let value = args
            .get(param.name)
            .ok_or_else(|| format!("no canonical example for `{}`", param.name))?;
        source.push(' ');
        source.push_str(&scheme(value).map_err(|why| format!("`{}` {why}", param.name))?);
    }
    source.push(')');
    Ok(source)
}

/// A wire [`Value`] as scheme source, in the spelling `convert.rs` documents:
/// a record is a hash, a list is a list, and there is nothing else to know.
fn scheme(value: &Value) -> Result<String, String> {
    Ok(match value {
        // Reachable only from an empty `Choice` or `Union`, which is a shape
        // with no callable example — at any door.
        Value::Null => return Err("has no canonical example".to_owned()),
        Value::Bool(true) => "#true".to_owned(),
        Value::Bool(false) => "#false".to_owned(),
        Value::Int(number) => number.to_string(),
        Value::Text(text) => quote(text),
        Value::List(items) => {
            let mut out = "(list".to_owned();
            for item in items {
                out.push(' ');
                out.push_str(&scheme(item)?);
            }
            out.push(')');
            out
        }
        Value::Record(fields) => {
            let mut out = "(hash".to_owned();
            for (name, field) in fields.iter() {
                out.push(' ');
                out.push_str(&quote(name));
                out.push(' ');
                out.push_str(&scheme(field)?);
            }
            out.push(')');
            out
        }
    })
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// MCP: generated, and well-formed enough for an agent to call
// ---------------------------------------------------------------------------

/// The MCP third. `T052` replaces the body with a live round-trip; the
/// signature is what the enumeration depends on.
fn mcp_door(capability: &Capability, tool: &Tool) -> Result<(), String> {
    // The generator's own definition of well-formed first — it lives beside the
    // generator so the two cannot describe different things. Everything after
    // this is what an MCP *client* additionally needs.
    mcp::check(tool).map_err(|error| error.to_string())?;

    if tool.name != capability.mcp_name() {
        return Err(format!(
            "the tool is named `{}`, the row is `{}`",
            tool.name,
            capability.mcp_name()
        ));
    }
    if tool.capability != capability.name {
        return Err(format!(
            "the tool dispatches back to `{}`, not to `{}`",
            tool.capability, capability.name
        ));
    }

    let reads = capability.kind == CapabilityKind::Query;
    if tool.read_only != reads {
        return Err(format!(
            "`readOnlyHint` is {} for a {:?}",
            tool.read_only, capability.kind
        ));
    }
    if tool.output_schema.is_some() != reads {
        return Err(format!(
            "a {:?} {} a result schema",
            capability.kind,
            if reads { "needs" } else { "must not declare" }
        ));
    }

    let Schema::Object {
        properties,
        additional_properties,
    } = &tool.input_schema
    else {
        return Err("the input schema is not an object".to_owned());
    };
    if *additional_properties {
        return Err("the input schema accepts properties the vocabulary never declared".to_owned());
    }

    // One property per declared parameter, in declaration order. This is the
    // assertion an empty-properties object fails: "is an object" is not the
    // same as "describes this capability".
    let declared: Vec<&str> = capability.params.iter().map(|param| param.name).collect();
    let offered: Vec<&str> = properties.iter().map(|property| property.name).collect();
    if declared != offered {
        return Err(format!(
            "the schema offers {offered:?} for the parameters {declared:?}"
        ));
    }

    let required: Vec<&str> = capability
        .params
        .iter()
        .filter(|param| param.required)
        .map(|param| param.name)
        .collect();
    if tool.input_schema.required() != required {
        return Err(format!(
            "the schema requires {:?}, the row requires {required:?}",
            tool.input_schema.required()
        ));
    }

    shape(&tool.name, &tool.input_schema)?;
    if let Some(output) = &tool.output_schema {
        shape(&format!("{}::result", tool.name), output)?;
    }

    // The example an agent reads has to be an example of *this* schema...
    validate(&tool.name, &tool.example, &tool.input_schema)?;

    // ...and the editor has to accept it. Without this the schema could
    // describe a call nothing decodes, which is the failure an agent discovers
    // at runtime and nobody discovers in CI.
    let Value::Record(args) = &tool.example else {
        return Err("the example is not a property bag".to_owned());
    };
    match capability.kind {
        CapabilityKind::Action => Action::from_call(capability.name, args)
            .map(|_| ())
            .map_err(|error| format!("the example does not decode — {error}")),
        CapabilityKind::Query => Query::from_call(capability.name, args)
            .map(|_| ())
            .map_err(|error| format!("the example does not decode — {error}")),
    }
}

/// The structural requirements a client has beyond [`mcp::check`].
fn shape(at: &str, schema: &Schema) -> Result<(), String> {
    match schema {
        Schema::Object { properties, .. } => {
            for property in properties {
                shape(&format!("{at}::{}", property.name), &property.schema)?;
            }
            Ok(())
        }
        Schema::OneOf {
            discriminator,
            arms,
        } => {
            // The tag rides in `kind` on the wire (`value::TAG_FIELD`), and a
            // schema that named it anything else would describe a call the
            // decoder rejects.
            if *discriminator != TAG_FIELD {
                return Err(format!(
                    "`{at}` discriminates on `{discriminator}`, the wire carries `{TAG_FIELD}`"
                ));
            }
            for arm in arms {
                let at = format!("{at}::{}", arm.tag);
                let Schema::Object { properties, .. } = &arm.schema else {
                    return Err(format!("`{at}` is not an object"));
                };
                // A `oneOf` an agent cannot pick an arm of is not callable: the
                // discriminator must be present, required, and pinned to this
                // arm's tag alone.
                let tag = properties
                    .iter()
                    .find(|property| property.name == *discriminator)
                    .ok_or_else(|| format!("`{at}` does not carry `{discriminator}`"))?;
                if !tag.required {
                    return Err(format!("`{at}` does not require `{discriminator}`"));
                }
                if !matches!(tag.schema, Schema::Enum(tags) if tags == [arm.tag]) {
                    return Err(format!(
                        "`{at}` does not pin `{discriminator}` to `{}`",
                        arm.tag
                    ));
                }
                shape(&at, &arm.schema)?;
            }
            Ok(())
        }
        Schema::Array { items } => shape(&format!("{at}[]"), items),
        Schema::Text {
            min_length,
            max_length,
            ..
        } => match (min_length, max_length) {
            (Some(low), Some(high)) if low > high => {
                Err(format!("`{at}` accepts no string at all"))
            }
            _ => Ok(()),
        },
        Schema::Enum(_) | Schema::Boolean | Schema::Integer { .. } | Schema::Id { .. } => Ok(()),
        // `ParamType::Any`'s one legitimate use is a surface whose parameters
        // belong to the surface (`registry::ParamType::Any`).
        Schema::Unconstrained => Ok(()),
    }
}

/// Whether a value is an instance of a schema.
///
/// A validator rather than an eyeball: the tool's own `example` is checked
/// against the tool's own `input_schema`, so a schema that describes something
/// other than the capability it belongs to fails even when it is a perfectly
/// valid JSON Schema.
fn validate(at: &str, value: &Value, schema: &Schema) -> Result<(), String> {
    match (schema, value) {
        (Schema::Boolean, Value::Bool(_)) => Ok(()),
        (Schema::Integer { minimum }, Value::Int(number)) => match minimum {
            Some(floor) if number < floor => Err(format!("`{at}`: {number} is below {floor}")),
            _ => Ok(()),
        },
        (Schema::Id { .. }, Value::Int(number)) if *number >= 0 => Ok(()),
        (
            Schema::Text {
                min_length,
                max_length,
                ..
            },
            Value::Text(text),
        ) => {
            let length = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
            if min_length.is_some_and(|low| length < low)
                || max_length.is_some_and(|high| length > high)
            {
                Err(format!("`{at}`: {text:?} is outside the declared length"))
            } else {
                Ok(())
            }
        }
        (Schema::Enum(tags), Value::Text(text)) => {
            if tags.contains(&text.as_str()) {
                Ok(())
            } else {
                Err(format!("`{at}`: {text:?} is not one of {tags:?}"))
            }
        }
        (Schema::Array { items }, Value::List(elements)) => {
            for (index, element) in elements.iter().enumerate() {
                validate(&format!("{at}[{index}]"), element, items)?;
            }
            Ok(())
        }
        (
            Schema::Object {
                properties,
                additional_properties,
            },
            Value::Record(fields),
        ) => {
            for property in properties {
                let at = format!("{at}::{}", property.name);
                match fields.get(property.name) {
                    Some(Value::Null) | None if property.required => {
                        return Err(format!("`{at}` is required and absent"));
                    }
                    Some(Value::Null) | None => {}
                    Some(field) => validate(&at, field, &property.schema)?,
                }
            }
            if !*additional_properties {
                for (name, _) in fields.iter() {
                    if !properties.iter().any(|property| property.name == name) {
                        return Err(format!("`{at}` carries undeclared `{name}`"));
                    }
                }
            }
            Ok(())
        }
        (
            Schema::OneOf {
                discriminator,
                arms,
            },
            Value::Record(fields),
        ) => {
            let Some(Value::Text(tag)) = fields.get(discriminator) else {
                return Err(format!("`{at}` carries no `{discriminator}`"));
            };
            let arm = arms
                .iter()
                .find(|arm| arm.tag == tag.as_str())
                .ok_or_else(|| format!("`{at}`: `{tag}` names no arm"))?;
            validate(&format!("{at}::{tag}"), value, &arm.schema)
        }
        (Schema::Unconstrained, Value::Record(_)) => Ok(()),
        (schema, value) => Err(format!("`{at}`: {value:?} does not satisfy {schema:?}")),
    }
}

// ---------------------------------------------------------------------------
// CLI: the built binary, run as a process
// ---------------------------------------------------------------------------

/// The CLI third: the generated verb, filled from the same canonical examples,
/// run as a program with no terminal.
fn cli_door(capability: &Capability, verb: &Verb) -> Result<(), String> {
    if verb.verb != capability.name {
        return Err(format!(
            "the verb is `{}`, the row is `{}`",
            verb.verb, capability.name
        ));
    }

    let mut argv: Vec<String> = vec![verb.verb.to_owned()];
    for flag in &verb.flags {
        // `--flag=value`, not `--flag value`. The vocabulary has signed
        // parameters — `indent`'s and `scroll`'s deltas — and clap reads a bare
        // `-1` as an option, so the separated form is not a general way to call
        // this door. Reported to `T023` rather than worked around silently.
        match flag_text(&flag.value) {
            Some(text) => argv.push(format!("--{}={text}", flag.long)),
            None => argv.push(format!("--{}", flag.long)),
        }
    }

    let output = run(&argv);
    let printed = String::from_utf8_lossy(&output.stdout).into_owned();
    let diagnostics = String::from_utf8_lossy(&output.stderr).into_owned();

    // The one capability that answers no task id: its implementation is the VM
    // (`is_the_vm`), and since `T022` the door has one. The canonical example
    // for its source parameter is the word `sample`, which is not a defined
    // identifier — so the VM *raises*, and a raise is the proof that the source
    // reached a runtime at all.
    //
    // `T100` sharpened this from `#refused ·` to the exact sentence. The old
    // form passed on any refusal that named no task, which is what a door with
    // no runtime at all would print for `eval` too — `not built yet — T021
    // builds it` was excluded by name, but `Refusal::Declined` carrying
    // anything else was not.
    if is_the_vm(capability) {
        const UNBOUND: &str = "#raised · unbound identifier — ";
        return if printed.starts_with(UNBOUND) {
            Ok(())
        } else {
            Err(format!(
                "`phosphor {}` answered {printed:?} / {diagnostics:?}, which is not a VM's answer",
                argv.join(" ")
            ))
        };
    }

    let expected = format!(
        "#refused · not built yet — {} builds it\n",
        task_of(capability)
    );

    if printed == expected {
        // The line names this row's own task, which `door.rs` reads off the
        // decoded Action — so the verb reached the dispatcher as this
        // capability and not as a neighbour.
        return Ok(());
    }

    // The registry names the parameters with no flag form and routes them to
    // `--eval` (`registry::cli`, `Verb::unreachable`). A verb that says so,
    // naming one of its own unreachable parameters and printing no result, has
    // been reached end to end — it is the answer the door is supposed to give.
    if verb.needs_eval() && printed.is_empty() {
        let named = verb
            .unreachable
            .iter()
            .any(|name| diagnostics.contains(&name.replace('_', "-")));
        if named && diagnostics.contains("--eval") {
            return Ok(());
        }
    }

    Err(format!(
        "`phosphor {}` answered {printed:?} / {diagnostics:?}, expected {expected:?}",
        argv.join(" ")
    ))
}

/// The task id the CLI door reports for this capability.
///
/// Every capability answers its own row's task — `door.rs` derives it from the
/// decoded Action rather than listing it — except the one whose implementation
/// *is* a runtime. See [`is_the_vm`].
fn task_of(capability: &Capability) -> &'static str {
    capability.since.task
}

/// The text a flag's value takes, from the same canonical example every other
/// door is checked with. [`None`] for a switch, which carries no value.
fn flag_text(value: &FlagValue) -> Option<String> {
    match value {
        FlagValue::Switch => None,
        FlagValue::One(ty) | FlagValue::Many(ty) => Some(scalar_text(ty)),
        // The tag flag selects an arm; `sample` picks the first variant, so
        // picking the first tag here keeps the three doors on one example.
        FlagValue::Arm(tags) => tags.first().map(|tag| (*tag).to_owned()),
    }
}

fn scalar_text(ty: &ParamType) -> String {
    match sample(ty) {
        Value::Bool(flag) => flag.to_string(),
        Value::Int(number) => number.to_string(),
        Value::Text(text) => text,
        // Unreachable: `cli::verb` only emits `One`/`Many` for scalars, and
        // every scalar's sample is one of the three above. A shape that stops
        // being scalar shows up as an unparseable value rather than silently.
        other => format!("{other:?}"),
    }
}

fn run(argv: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_phosphor"))
        .args(argv)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs")
}

// ---------------------------------------------------------------------------
// The enumeration
// ---------------------------------------------------------------------------

/// The registry's shape, which every walk below assumes and none of them
/// re-checks.
///
/// Split out with the three walks at `§26`. It is the assertion that makes an
/// empty or half-registered table fail loudly rather than letting three tests
/// pass over nothing.
#[test]
fn the_registry_is_one_row_per_capability_at_three_doors() {
    let registered = registrations();
    assert_eq!(
        registered.len(),
        capabilities().len(),
        "a registration is one capability at all three doors"
    );
    assert!(
        !registered.is_empty(),
        "an empty registry would pass every walk below"
    );
    assert_eq!(
        Door::ALL.len(),
        3,
        "invariant 2 is three doors, and there is one walk per door below. \
         A fourth door needs a fourth walk — `Doors::check`'s `match` will not \
         compile without one, and this says so before the compiler does."
    );
}

/// One door, every capability. The body of the three walks below.
///
/// **Why there are three walks and not one loop over `Door::ALL`.** This was a
/// single test until `§26`, and it took **176.1 s of a 182.5 s** suite — every
/// other test in the repository finished inside the remaining ~6 seconds.
/// `nextest` isolates tests per process and can run hundreds concurrently, but
/// it cannot split one test *function*, so that was a floor under `just gate`
/// that no parallelism removed, paid once per agent in a concurrent window.
///
/// Splitting per door was **not** expected to be the fix — a split bounds the
/// run at the largest third, and if one door owns nearly all of the time the
/// speedup rounds to nothing. It was expected to be the cheapest way to buy the
/// measurement, because three tests print three numbers. Read those numbers
/// before optimising anything here; the entry at `docs/OPEN-QUESTIONS.md`'s
/// `§26` records what they turned out to be.
///
/// Nothing about the checking changed. `Doors::check` is still a `match` on
/// [`Door`], so a fourth door is still a compile error rather than a third of
/// the vocabulary nobody walked.
fn walk(door: Door) {
    let registered = registrations();
    let mut doors = Doors::open();
    let mut failures = Vec::new();

    for registration in &registered {
        if let Err(why) = doors.check(door, registration) {
            failures.push(format!("{} — {why}", registration.capability.name));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of the {} {} checks failed:\n{}",
        failures.len(),
        registered.len(),
        door.as_str(),
        failures.join("\n")
    );
}

#[test]
fn every_capability_is_reachable_at_the_steel_door() {
    walk(Door::Steel);
}

#[test]
fn every_capability_is_reachable_at_the_mcp_door() {
    walk(Door::Mcp);
}

/// The CLI door, and the only walk that does not go through [`Doors::check`].
///
/// **This third is the whole cost, and the split is what proved it.** Measured
/// the moment the three walks existed: Steel **1.19 s**, MCP **1.14 s**, CLI
/// **157.62 s**. The guess recorded at `§26` — that the Steel door owned the
/// time, because it is one parse/compile/eval per capability — was wrong by two
/// orders of magnitude, which is the argument for measuring before optimising
/// stated as a fact rather than as advice.
///
/// The reason is structural and not a defect: [`cli_door`] is the only door
/// whose check *spawns the shipping binary*, because a CLI door with an exit
/// code and an argv is not a function you can call in-process. So this walk is
/// 212 process launches, each one booting the Steel layer on the way up.
///
/// They are also completely independent of one another — separate processes,
/// null stdin, no shared state, nothing written — so they are run across lanes
/// rather than end to end. That is a genuine wall-clock fix rather than a
/// rearrangement: the work per capability is unchanged and every one of them
/// still runs.
///
/// This is why the walk calls [`cli_door`] directly. The `match` on [`Door`] in
/// [`Doors::check`] still exists and is still what the other two walks go
/// through, so a fourth door remains a compile error there — and
/// `the_registry_is_one_row_per_capability_at_three_doors` fails first, saying
/// a fourth walk is owed.
#[test]
fn every_capability_is_reachable_at_the_cli_door() {
    let registered = registrations();
    let lanes = std::thread::available_parallelism()
        .map_or(4, |cores| cores.get())
        .clamp(1, registered.len().max(1));
    let per_lane = registered.len().div_ceil(lanes).max(1);

    let failures: Vec<String> = std::thread::scope(|scope| {
        let lanes: Vec<_> = registered
            .chunks(per_lane)
            .map(|slice| {
                scope.spawn(move || {
                    slice
                        .iter()
                        .filter_map(|registration| {
                            cli_door(&registration.capability, &registration.cli)
                                .err()
                                .map(|why| format!("{} — {why}", registration.capability.name))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        lanes
            .into_iter()
            .flat_map(|lane| lane.join().expect("a lane spawns the binary and no more"))
            .collect()
    });

    assert!(
        failures.is_empty(),
        "{} of the {} cli checks failed:\n{}",
        failures.len(),
        registered.len(),
        failures.join("\n")
    );
}

#[test]
fn an_alias_is_an_extra_name_and_never_a_fourth_registration() {
    // `6b` draws `(watch-place …)` for a row named `place-watch`. An alias that
    // resolved to nothing, or to a name outside the table, would be a
    // Steel-only capability wearing a different hat — exactly what the parity
    // above is for.
    let doors = Doors::open();
    for alias in alias_bindings() {
        let capability = lookup(alias.capability)
            .unwrap_or_else(|| panic!("`{}` names no registered row", alias.name));
        assert_eq!(alias.canonical, capability.steel_name());
        doors
            .runtime
            .global(alias.name)
            .unwrap_or_else(|error| panic!("`{}` is not bound in the VM — {error}", alias.name));
    }
}

// ---------------------------------------------------------------------------
// The planted violations
// ---------------------------------------------------------------------------
//
// `CP-2` asks for "a planted one-door-only Action fails CI". A capability
// cannot be absent from a door — each door is a total function of a row — so
// the plantable failure is a capability that is registered at a door and not
// usable through it. One per door, each doctoring a real registration, because
// a check nobody has watched bite is a check nobody should trust.

#[test]
fn a_capability_the_steel_door_never_bound_is_caught() {
    const ABSENT: &str = "mark-read";
    assert!(
        lookup(ABSENT).is_none(),
        "the planted name has to be one the registry does not declare"
    );
    let mark_seen = lookup("mark-seen").expect("registered");
    let planted = Capability {
        name: ABSENT,
        ..mark_seen
    };
    let mut doors = Doors::open();
    let error = doors
        .steel_door(&planted, &phosphor_core::registry::steel::binding(&planted))
        .expect_err("an unbound identifier is not a reachable capability");
    assert!(error.contains("not bound"), "{error}");
}

#[test]
fn a_schema_that_is_an_object_with_no_properties_is_caught() {
    // The lazy check this test exists to be better than: `{"type": "object"}`
    // is well-formed JSON Schema and useless to an agent.
    let mark_seen = lookup("mark-seen").expect("registered");
    let mut planted = mcp::tool(&mark_seen);
    planted.input_schema = Schema::Object {
        properties: Vec::new(),
        additional_properties: false,
    };
    let error = mcp_door(&mark_seen, &planted).expect_err("a bare object describes nothing");
    assert!(error.contains("offers []"), "{error}");
}

#[test]
fn a_schema_whose_example_it_does_not_describe_is_caught() {
    // The subtler one: every property is present and documented, and the
    // example still is not an instance of it.
    let mark_seen = lookup("mark-seen").expect("registered");
    let mut planted = mcp::tool(&mark_seen);
    let Schema::Object { properties, .. } = &mut planted.input_schema else {
        panic!("an input schema is an object");
    };
    properties[0].schema = Schema::Text {
        min_length: None,
        max_length: None,
        format: None,
    };
    let error = mcp_door(&mark_seen, &planted).expect_err("the example is not text");
    assert!(error.contains("does not satisfy"), "{error}");
}

#[test]
fn a_verb_the_binary_does_not_accept_is_caught() {
    let mark_seen = lookup("mark-seen").expect("registered");
    let mut planted = phosphor_core::registry::cli::verb(&mark_seen);
    planted.verb = "mark-seen";
    // The verb name is right and the flags are wrong: one that the parser does
    // not declare, which is what a door drifting from the table looks like from
    // the outside.
    planted.flags[0].long = "targt".to_owned();
    let error = cli_door(&mark_seen, &planted).expect_err("the binary has no `--targt`");
    assert!(error.contains("expected"), "{error}");
}

#[test]
fn a_verb_that_answers_for_another_capability_is_caught() {
    // The failure a hand-written parity list cannot see: the verb runs, prints
    // a well-formed refusal, and it is the wrong capability's. The task id in
    // the line is what tells them apart.
    let close = lookup("close-float").expect("registered");
    let planted = Capability {
        since: phosphor_core::registry::Since {
            task: "T999",
            ..close.since
        },
        ..close
    };
    let error = cli_door(&planted, &phosphor_core::registry::cli::verb(&close))
        .expect_err("the printed task id is the row's, not a constant");
    assert!(error.contains("T999"), "{error}");
}

// ---------------------------------------------------------------------------
// The third of the CLI door that a production change would complete
// ---------------------------------------------------------------------------

/// The capabilities whose flags cannot express one argument reach the CLI door
/// through `--eval` — *"nothing is reachable by a verb that is not reachable by
/// `--eval`"* (`phosphor_core::registry::cli`). This asserts that sentence.
///
/// **Un-ignored by `T022`**, which is the production change this asked for:
/// `main.rs` now hands `door::run` a `Vm` over
/// `phosphor_steel::runtime::Runtime::evaluate`, so `--eval` answers what the VM
/// made of the expression instead of one refusal for all of them.
///
/// The check is deliberately about *reach*, not rendering: the door prints
/// whatever the VM answered, and at `S2` the host behind it refuses each
/// capability by naming its own row's task — which is the per-capability thing
/// there is to observe. A capability the `S2` host **carries out** answers `#ok`
/// and is exempt, because "it happened" is a stronger proof of reach than a
/// refusal is.
#[test]
fn the_eval_route_reaches_what_no_flag_can_express() {
    let mut failures = Vec::new();
    for registration in registrations() {
        if !registration.cli.needs_eval() {
            continue;
        }
        let capability = &registration.capability;
        let source = match scheme_call(capability, &registration.steel) {
            Ok(source) => source,
            Err(why) => {
                failures.push(format!("{} — {why}", capability.name));
                continue;
            }
        };
        let output = run(&["--eval".to_owned(), source.clone()]);
        let printed = String::from_utf8_lossy(&output.stdout).into_owned();
        // **Not answered yet** is what has to name a task; anything else was
        // carried out and reached the VM and then the host, which is more than
        // the refusal below proves.
        //
        // This read `printed.trim() == "#ok"` until `T041`, which is the same
        // check for a build where every carried-out capability answered
        // `Value::Null`. `mark-seen` answers *how many regions were in scope* —
        // a number is the composable answer for something a script calls — so
        // the narrow form started failing on capabilities that had just been
        // built. The claim was never about the shape of a success.
        let unanswered = printed.contains("refused") || printed.contains("raised");
        if !unanswered {
            // Carried out.
        } else if !printed.contains(capability.since.task) {
            failures.push(format!(
                "{} — `--eval {source}` answered {printed:?}, which names no task of its own",
                capability.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `§14` — one door, one refusal, one exit code.
///
/// The two routes into the CLI door disagreed: a verb decodes to an Action and
/// gets `Outcome::Refused`, while `--eval` runs scheme and the refusal comes
/// back as the *value* the scheme evaluated to, inside a successful
/// `Outcome::Done`. So the same refusal exited `1` one way and `0` the other,
/// and a script that seeded state through this door could not tell.
///
/// Both spellings below are the same capability refusing for the same reason.
/// The eval one also happens to be the `§8` spelling — a plain `path:line`
/// where a tagged target used to be required — so this pins both rulings at the
/// shipping binary rather than at a unit boundary.
///
/// **It was `mark-seen` until `T041` built it.** `place-watch` keeps both
/// claims — a `Target` argument, so the `path:line` spelling is still what the
/// eval route exercises — and is `S8`/`T077`, so it stays a refusal for as long
/// as this test is about refusals.
#[test]
fn the_two_cli_routes_agree_on_what_a_refusal_exits() {
    let verb = run(&[
        "place-watch".to_owned(),
        "--anchor=cursor".to_owned(),
        "--expr=delay".to_owned(),
    ]);
    let eval = run(&[
        "--eval".to_owned(),
        "(place-watch! \"src/retry.rs:24\" \"delay\")".to_owned(),
    ]);

    for (route, output) in [("verb", &verb), ("eval", &eval)] {
        let printed = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            printed.contains("refused"),
            "the {route} route was supposed to refuse; it printed {printed:?}"
        );
        assert!(
            !output.status.success(),
            "the {route} route refused and still exited 0 — a caller checking `$?` \
             cannot tell that nothing happened"
        );
    }

    assert_eq!(
        verb.status.code(),
        eval.status.code(),
        "one door, one refusal, one exit code"
    );
}
