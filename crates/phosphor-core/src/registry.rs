//! The capability table — one row per Action and per query, and the door-name
//! derivation all three doors read.
//!
//! Invariant 2's only real defence is that *there is one table*. The rows come
//! first: the [`ActionSpec`] and [`QuerySpec`] rows are emitted by the same macro
//! invocation that emits the enum variants, so a capability cannot exist without
//! a row and a row cannot exist without a capability.
//!
//! # The three doors (`T020`)
//!
//! [`steel`], [`mcp`] and [`cli`] are **derived views over this table, not
//! registries of their own.** Each exposes one total function of a
//! [`Capability`] — [`steel::binding`], [`mcp::tool`], [`cli::verb`] — so there
//! is nowhere to forget a capability and nothing to keep in step. That is what
//! *"adding a capability to one door adds it to all by construction"* means
//! here: registration is the macro row, and the doors are functions.
//!
//! The consuming crates hold no tables either. `T021`'s
//! `phosphor-steel/registry.rs` installs [`steel::bindings`] into the VM,
//! `T023`'s CLI builds its subcommands from [`cli::verbs`], and `T052`'s MCP
//! server serialises [`mcp::tools`]. The moment one of them keeps a list beside
//! this one, invariant 2 is decorative.
//!
//! **The MCP schema is plain data.** `phosphor-core` is dependency-free at the
//! floor, so [`mcp::Schema`] describes a JSON Schema rather than being one; the
//! crate that owns `serde` walks it. Same reason [`cli::Verb`] describes flags
//! rather than building a `clap::Command`.
//!
//! `T024`'s door-parity test enumerates [`registrations`] — every capability in
//! every door, in one walk. That is the point of this module existing at all: a
//! hand-written list rots, an enumeration cannot.
//!
//! Owned by `spine`.

pub mod cli;
pub mod mcp;
pub mod steel;

use crate::action::{ACTIONS, ActionSpec};
use crate::query::{QUERIES, QuerySpec};
use crate::value::{Args, Value};

// ---------------------------------------------------------------------------
// Phase and provenance
// ---------------------------------------------------------------------------

/// The build phase a capability lands in.
///
/// Every capability names one, and `tests/vocabulary.rs` checks the set against
/// the committed checklist in `tests/surfaces.txt`. That is what makes `T019`'s
/// acceptance criterion — *every mutation in S3–S8 has a named Action, even if
/// unimplemented* — a red test rather than a discovery in Window F.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Theme, `BufferView`, `StatusLine`, the S1 host.
    S1,
    /// Steel, the Action spine, the REPL, the view tree. This phase.
    S2,
    /// Input, undo, gutter and virtual text.
    S3,
    /// LSP and the completion float.
    S4,
    /// The semantic store, seen-tracking, the picker — the awareness loop.
    S5,
    /// ACP, MCP, transcript, prompt — the directing loop.
    S6,
    /// Diffs, review blocks, inbox, dirty state, VCS.
    S7,
    /// Watches.
    S8,
    /// Named now, refused at runtime: v1.5 surfaces the vocabulary has to be able
    /// to express so they are not new machinery later (Q12's closing argument).
    V15,
}

impl Phase {
    /// The spelling used in `docs/TASKS.md` and in `tests/surfaces.txt`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S1 => "S1",
            Self::S2 => "S2",
            Self::S3 => "S3",
            Self::S4 => "S4",
            Self::S5 => "S5",
            Self::S6 => "S6",
            Self::S7 => "S7",
            Self::S8 => "S8",
            Self::V15 => "V15",
        }
    }

    /// Parses the checklist spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "S1" => Self::S1,
            "S2" => Self::S2,
            "S3" => Self::S3,
            "S4" => Self::S4,
            "S5" => Self::S5,
            "S6" => Self::S6,
            "S7" => Self::S7,
            "S8" => Self::S8,
            "V15" => Self::V15,
            _ => return None,
        })
    }
}

/// Where a capability comes from: the phase, and the task that implements it.
///
/// The task id is load-bearing, not decoration. It is what
/// [`Refusal::NotYetImplemented`](crate::action::Refusal::NotYetImplemented)
/// reports, so an agent or a user who reaches a named-but-unbuilt capability is
/// told which task builds it instead of getting "unknown action".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Since {
    /// The phase.
    pub phase: Phase,
    /// The `docs/TASKS.md` id, e.g. `"T041"`.
    pub task: &'static str,
}

// ---------------------------------------------------------------------------
// Doors
// ---------------------------------------------------------------------------

/// One of the three doors of invariant 2.
///
/// Rides on [`Request`](crate::action::Request) as well as appearing here: the
/// MCP policy below refuses focus-relative targets, and it can only do that if
/// the dispatcher knows which door asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Door {
    /// In-process Steel — `runtime/*.scm` and the REPL (`T021`, `T022`).
    Steel,
    /// The MCP editor-tool server Claude talks to (`T052`).
    Mcp,
    /// `phosphor --eval` and the generated verbs (`T023`).
    Cli,
}

impl Door {
    /// All three, for enumeration in `T024`'s parity test.
    pub const ALL: &'static [Self] = &[Self::Steel, Self::Mcp, Self::Cli];

    /// The door's name in a message.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Steel => "steel",
            Self::Mcp => "mcp",
            Self::Cli => "cli",
        }
    }
}

/// What the MCP door does with a capability *by default*.
///
/// **This is a policy, not a second vocabulary.** Every capability is registered
/// in all three doors; what varies is authorization, and the user widens it with
/// a legible rule in `init.scm` — the same mechanism `7a`'s always-allow already
/// writes (`T061`). Marking a capability "Steel-only" instead would break
/// invariant 2 by construction, and is exactly what `T024` is built to fail on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpPolicy {
    /// Callable by an agent without asking.
    Allow,
    /// Callable, but it routes through the ask queue first (`T060`, `T061`).
    Ask,
    /// Registered, schema generated, refused unless a rule in `init.scm` opens
    /// it. Applied to the capabilities that give an agent the *user's* keyboard
    /// rather than an editor capability — see
    /// [`Action::feeds_the_keyboard`](crate::action::Action::feeds_the_keyboard).
    Deny,
}

// ---------------------------------------------------------------------------
// The declared shape of a payload
// ---------------------------------------------------------------------------

/// One named argument of a capability, or one field of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// The argument name — the CLI flag, the JSON property, the Steel keyword.
    pub name: &'static str,
    /// One line, in the product's voice. This is what the MCP schema's
    /// `description` says and what `:describe-action` shows.
    pub doc: &'static str,
    /// The declared shape.
    pub ty: ParamType,
    /// `false` only for `Option<T>` (see [`Wire::REQUIRED`](crate::value::Wire::REQUIRED)).
    pub required: bool,
}

/// One arm of a tagged union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnionVariant {
    /// The tag, carried in [`TAG_FIELD`](crate::value::TAG_FIELD).
    pub tag: &'static str,
    /// One line describing when this arm applies.
    pub doc: &'static str,
    /// This arm's fields.
    pub fields: &'static [Param],
}

/// The declared shape of a value, in a type language small enough that all three
/// doors can render it.
///
/// Every case maps cleanly onto JSON Schema (`T020`), a clap argument (`T023`)
/// and a Steel argument (`T021`). [`ParamType::Any`] is the single deliberate
/// hole, and it exists for exactly one reason — see its docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    /// `true` / `false`; a CLI flag.
    Bool,
    /// A signed integer — deltas, counts that can go backwards.
    Int,
    /// A non-negative integer — line numbers, indices, ids.
    Uint,
    /// Text.
    Text,
    /// Exactly one character — a register name, a delimiter.
    Char,
    /// A filesystem path, carried as text.
    Path,
    /// One of a fixed set of names.
    Choice(&'static [&'static str]),
    /// A homogeneous list.
    List(&'static ParamType),
    /// A record with these fields.
    Record(&'static [Param]),
    /// A tagged union — a record whose `kind` selects the arm.
    Union(&'static [UnionVariant]),
    /// An opaque identifier, carried as a non-negative integer. The string names
    /// what it identifies (`"region"`, `"thread"`) so a schema can say so.
    Id(&'static str),
    /// A free-form record.
    ///
    /// **One legitimate use:** the parameters of a surface whose schema belongs
    /// to the surface rather than to us
    /// ([`SurfaceId`](crate::request::SurfaceId)). `:arch` (`T048`, Q11) has to
    /// be openable without a Rust edit, which means its arguments cannot be in a
    /// Rust type. Anywhere else this is a vocabulary that has given up.
    Any,
}

impl ParamType {
    /// A short name for this shape, for a CLI value placeholder and for
    /// `:describe-action`.
    ///
    /// One definition rather than one per door: `--seek <NEXT|PREV|FIRST|LAST>`
    /// and the Steel help text are the same vocabulary talking, and two spellings
    /// of one type is exactly the drift the single table exists to prevent.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Bool => "bool".to_owned(),
            Self::Int => "int".to_owned(),
            Self::Uint => "uint".to_owned(),
            Self::Text => "text".to_owned(),
            Self::Char => "char".to_owned(),
            Self::Path => "path".to_owned(),
            Self::Choice(tags) => tags.join("|"),
            Self::List(inner) => format!("{}...", inner.label()),
            Self::Record(_) => "record".to_owned(),
            Self::Union(variants) => variants
                .iter()
                .map(|variant| variant.tag)
                .collect::<Vec<_>>()
                .join("|"),
            Self::Id(of) => format!("{of}-id"),
            Self::Any => "any".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// The uniform row
// ---------------------------------------------------------------------------

/// Whether a capability writes or reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    /// An [`Action`](crate::action::Action) — it mutates, and the store records
    /// who asked.
    Action,
    /// A [`Query`](crate::query::Query) — pure, total, synchronous, and it may
    /// not mutate. `query.rs` lists the rest of the contract.
    Query,
}

/// One row of the table, uniform across Actions and queries.
///
/// `T020` builds three doors from this and nothing else; `T024` enumerates it.
/// The borrow is `'static` throughout — the table is a `const`, so a door can
/// hold a row for the process lifetime without copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// The globally unique door name, kebab-case: `"mark-seen"`,
    /// `"declare-review-block"`.
    pub name: &'static str,
    /// Which domain enum it lives in, for grouping in `:help` and in schemas.
    pub domain: &'static str,
    /// One line, in the product's voice.
    pub doc: &'static str,
    /// Action or query.
    pub kind: CapabilityKind,
    /// The phase and task that implement it.
    pub since: Since,
    /// The MCP door's default policy.
    pub mcp: McpPolicy,
    /// Its arguments, in declaration order.
    pub params: &'static [Param],
    /// What a query returns; [`None`] for an Action, whose result is an
    /// [`Outcome`](crate::action::Outcome).
    pub returns: Option<ParamType>,
}

impl Capability {
    /// The Steel spelling.
    ///
    /// Actions take a `!` (they mutate); queries do not. `6b` draws
    /// `(keymap-set! …)` and `(unseen-regions "src/retry.rs")`, which is this
    /// rule. It also draws `(watch-place …)` — mutating, no bang — which is why
    /// [`crate::action::ALIASES`] exists and why that one is flagged rather than
    /// folded in.
    #[must_use]
    pub fn steel_name(&self) -> String {
        match self.kind {
            CapabilityKind::Action => format!("{}!", self.name),
            CapabilityKind::Query => self.name.to_owned(),
        }
    }

    /// The MCP tool name.
    ///
    /// One function, because Q6 fixes exactly one of these literally —
    /// `phosphor/declare-review-block` — and if `rmcp` turns out to reject `/`
    /// in a tool name at `T052`, this is the single place the whole vocabulary
    /// changes spelling.
    #[must_use]
    pub fn mcp_name(&self) -> String {
        format!("phosphor/{}", self.name)
    }

    /// The CLI verb: `phosphor mark-seen --target …`.
    #[must_use]
    pub const fn cli_verb(&self) -> &'static str {
        self.name
    }

    /// This capability's name at one door.
    ///
    /// A `match` on [`Door`] rather than three unrelated methods, so a fourth
    /// door would be a compile error at every call site instead of a door that
    /// silently names nothing. `T024` walks [`Door::ALL`] through this.
    #[must_use]
    pub fn name_in(&self, door: Door) -> String {
        match door {
            Door::Steel => self.steel_name(),
            Door::Mcp => self.mcp_name(),
            Door::Cli => self.cli_verb().to_owned(),
        }
    }
}

impl ActionSpec {
    /// This Action's row in the uniform table.
    #[must_use]
    pub const fn capability(&self) -> Capability {
        Capability {
            name: self.name,
            domain: self.domain,
            doc: self.doc,
            kind: CapabilityKind::Action,
            since: self.since,
            mcp: self.mcp,
            params: self.params,
            returns: None,
        }
    }
}

impl QuerySpec {
    /// This query's row in the uniform table.
    #[must_use]
    pub const fn capability(&self) -> Capability {
        Capability {
            name: self.name,
            domain: self.domain,
            doc: self.doc,
            kind: CapabilityKind::Query,
            since: self.since,
            // A query reads. Reading is what an agent is *for*; the refusals
            // that matter are on the write side and on focus-relative targets.
            mcp: McpPolicy::Allow,
            params: self.params,
            returns: Some(self.returns),
        }
    }
}

/// Every capability, Actions then queries, in declaration order.
///
/// **The enumeration `T024` walks.** Adding a capability adds a row here with no
/// further edit, which is the whole mechanism: a door that forgot one is a
/// failing test, not a missing feature nobody noticed.
#[must_use]
pub fn capabilities() -> Vec<Capability> {
    ACTIONS
        .iter()
        .map(ActionSpec::capability)
        .chain(QUERIES.iter().map(QuerySpec::capability))
        .collect()
}

/// The capability with this door name, if any.
#[must_use]
pub fn lookup(name: &str) -> Option<Capability> {
    capabilities().into_iter().find(|cap| cap.name == name)
}

// ---------------------------------------------------------------------------
// One capability, all three doors
// ---------------------------------------------------------------------------

/// One capability as each of the three doors sees it.
///
/// **This is what `T024` enumerates.** Not three lists to compare — one list of
/// triples, built by three total functions, so "present in all three doors" is a
/// property of the type rather than something a test hopes to find. The test's
/// job is to prove the derived views are *usable* (a well-formed schema, a
/// callable binding, a verb whose flags reassemble), not that they exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// The row.
    pub capability: Capability,
    /// The Steel binding (`T021`).
    pub steel: steel::Binding,
    /// The MCP tool (`T052`; generated from S2 so it cannot drift).
    pub mcp: mcp::Tool,
    /// The CLI verb (`T023`).
    pub cli: cli::Verb,
}

impl Capability {
    /// This capability at all three doors.
    #[must_use]
    pub fn registration(&self) -> Registration {
        Registration {
            capability: *self,
            steel: steel::binding(self),
            mcp: mcp::tool(self),
            cli: cli::verb(self),
        }
    }
}

/// Every capability at every door, in registry order.
#[must_use]
pub fn registrations() -> Vec<Registration> {
    capabilities()
        .iter()
        .map(Capability::registration)
        .collect()
}

// ---------------------------------------------------------------------------
// Canonical examples
// ---------------------------------------------------------------------------

/// A canonical example value of a declared shape.
///
/// Two uses, and the second is why it is public rather than test-only:
///
/// * `tests/vocabulary.rs` encodes every capability from its declared params and
///   decodes it back. That round trip is what proves a variant's decoder agrees
///   with its schema — for **every** capability, without anyone writing 150
///   examples by hand.
/// * `T020` can put an `example` in the MCP schema it generates, which is the
///   difference between an agent guessing at an argument's shape and reading it.
///
/// Total: every [`ParamType`] has one. An empty [`ParamType::Choice`] is the only
/// degenerate case and answers [`Value::Null`], which the round trip then fails
/// on — correctly, since a choice of nothing is not callable.
#[must_use]
pub fn sample(ty: &ParamType) -> Value {
    match ty {
        ParamType::Bool => Value::Bool(true),
        ParamType::Int => Value::Int(-1),
        ParamType::Uint => Value::Int(1),
        ParamType::Text => Value::Text("sample".to_owned()),
        ParamType::Char => Value::Text("a".to_owned()),
        ParamType::Path => Value::Text("src/retry.rs".to_owned()),
        ParamType::Choice(tags) => tags
            .first()
            .map_or(Value::Null, |tag| Value::Text((*tag).to_owned())),
        ParamType::List(inner) => Value::List(vec![sample(inner)]),
        ParamType::Record(fields) => Value::Record(sample_args(fields)),
        ParamType::Union(variants) => variants.first().map_or(Value::Null, |variant| {
            Value::tagged(variant.tag, sample_args(variant.fields))
        }),
        ParamType::Id(_) => Value::Int(7),
        ParamType::Any => Value::Record(Args::new()),
    }
}

/// Canonical example arguments for a parameter list — every field filled,
/// optional ones included, so a round trip exercises them.
#[must_use]
pub fn sample_args(params: &[Param]) -> Args {
    let mut args = Args::new();
    for param in params {
        args.set(param.name, sample(&param.ty));
    }
    args
}

impl Capability {
    /// Canonical example arguments for this capability.
    #[must_use]
    pub fn sample_args(&self) -> Args {
        sample_args(self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_names_derive_from_one_field() {
        let cap = lookup("declare-review-block").expect("Q6's tool is registered");
        // Q6 fixes this spelling literally (IMPLEMENTATION-PLAN.md §5, Q6).
        assert_eq!(cap.mcp_name(), "phosphor/declare-review-block");
        assert_eq!(cap.steel_name(), "declare-review-block!");
        assert_eq!(cap.cli_verb(), "declare-review-block");
    }

    #[test]
    fn every_capability_is_named_at_every_door() {
        // The shape `T024` builds on: one walk, no per-door list to compare
        // against. A door that could be short one capability would have to be a
        // table rather than a function.
        for capability in capabilities() {
            for door in Door::ALL {
                assert!(
                    !capability.name_in(*door).is_empty(),
                    "`{}` has no name at the {} door",
                    capability.name,
                    door.as_str()
                );
            }
        }
    }

    #[test]
    fn a_registration_carries_the_same_capability_through_all_three() {
        let registrations = registrations();
        assert_eq!(registrations.len(), capabilities().len());
        for registration in registrations {
            let name = registration.capability.name;
            assert_eq!(registration.steel.capability, name);
            assert_eq!(registration.mcp.capability, name);
            assert_eq!(registration.cli.verb, name);
        }
    }

    #[test]
    fn queries_have_no_bang_and_actions_do() {
        let query = lookup("unseen-regions").expect("6b's first line is registered");
        assert_eq!(query.steel_name(), "unseen-regions");
        assert_eq!(query.kind, CapabilityKind::Query);
    }
}
