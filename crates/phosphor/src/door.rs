//! The CLI door (`T023`) — `phosphor --eval`, and one verb per capability.
//!
//! Invariant 2's third door. Steel is in-process (`T021`), MCP is the agent's
//! (`T052`), and this is the shell's: `phosphor --eval '(mark-seen! …)'`, plus
//! the generated verbs `phosphor mark-seen --target region --target.region.id 3`.
//!
//! # Nothing here is a table
//!
//! Every subcommand, every flag and every argument decoder comes out of
//! [`phosphor_core::registry::cli`], which is a total function of a registry
//! row. This module contains **no list of capabilities** — it walks
//! [`cli::verbs`](phosphor_core::registry::cli::verbs) to build the parser and hands the resulting
//! [`Call`] straight back through [`cli::assemble`](phosphor_core::registry::cli::assemble). Adding an Action to the
//! `actions!` table adds a verb here with no edit, which is the whole of `T020`'s
//! *"by construction"* claim as it lands on this door.
//!
//! The single name this file knows is [`EVAL`], and it is not an exception: it
//! is the capability `--eval` is *sugar for*, resolved through
//! [`registry::lookup`](phosphor_core::registry::lookup) like any other and checked for the shape the sugar
//! assumes ([`eval_call`]). `phosphor --eval EXPR` and `phosphor eval --source
//! EXPR` assemble the same [`Call`], which is a test below and not a comment.
//!
//! # One evaluation path, two front-ends
//!
//! `T023`'s acceptance criterion is *"`--eval` and the REPL return identical
//! results for the same expression"*. That is structural here rather than a
//! thing `T022` has to be careful about:
//!
//! ```text
//!   --eval EXPR ─┐
//!                ├─→ Call{eval, source} ─→ Action::Runtime(Eval) ─→ Request ─→ Evaluate::eval ─→ Outcome
//!   the REPL  ───┘                                (T022 builds the same Request)
//! ```
//!
//! `Action::Runtime(Eval)`'s own registry row says so — *"evaluates scheme
//! source; the CLI door and the REPL are both this"*
//! (`phosphor-core/src/action.rs`). Both front-ends encode to that one Action
//! and both read back an [`Outcome`]; there is no second evaluator to keep in
//! step, because this module has no evaluator at all — it takes one.
//!
//! # The seam, and what is not here yet
//!
//! [`Evaluate`] is that taken evaluator, and **nothing implements it yet**:
//! embedding `steel-core` and booting `init.scm` is `T021`, and `main.rs` passes
//! [`None`] to [`run`]. So `--eval` answers [`Refusal::NotYetImplemented`]
//! naming `T021` — the honest answer, and the same shape every unbuilt
//! capability gives. Saying `T023` there would be the lie: the door is built,
//! the VM it hands source to is not.
//!
//! Wiring it is a `main.rs` edit and nothing else: an adapter implementing this
//! one method over `phosphor-steel`'s engine, passed to [`run`] in place of that
//! `None`. No function in this file changes shape, and the REPL (`T022`) reaches
//! the same runtime through its own front-end — which is what makes *"identical
//! results"* a property of the arrangement rather than a thing to remember.
//!
//! # No terminal
//!
//! This path returns before [`Term`](phosphor_term::Term) is ever constructed:
//! no alternate screen, no raw mode, no synchronized-output wrapper. `V006`
//! seeds tape fixtures through `--eval` with **no test-only backdoor**
//! (`TASKS.md`), which only works if the door runs with stdout on a pipe and no
//! tty at all. `tests/door.rs` runs the built binary with stdout redirected and
//! asserts the output carries no escape sequence.
//!
//! Owned by `spine`.

use std::error::Error;
use std::fmt;
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use phosphor_core::action::{Action, ActionError, Outcome, Refusal, Request, RuntimeAction};
use phosphor_core::query::{Query, QueryError};
use phosphor_core::registry::cli::{
    CliError, FlagValue, Occurrence, Requirement, Verb, assemble, verbs,
};
use phosphor_core::registry::{Capability, CapabilityKind, Door, ParamType, lookup};
use phosphor_core::request::Actor;
use phosphor_core::value::{Call, Value};

/// The capability `--eval` is sugar for.
///
/// The one door name this file spells. It is checked against the registry at
/// call time rather than assumed — see [`eval_call`] — so a rename of the row
/// is a legible error here, not a silently dead flag.
const EVAL: &str = "eval";

/// The task that embeds the VM this door delegates scheme source to.
///
/// `T023` is the door; `T021` is `steel-core` embedded and booted. Until that
/// lands there is nothing to evaluate *with*, and saying so with the task id is
/// what [`Refusal::NotYetImplemented`] is for: a caller gets *"not built yet —
/// `T021` builds it"* rather than a stub that pretends.
const RUNTIME_TASK: &str = "T021";

// ---------------------------------------------------------------------------
// The evaluator seam
// ---------------------------------------------------------------------------

/// The thing that turns scheme source into an [`Outcome`].
///
/// Deliberately one method wide. The CLI door does not embed a VM, does not know
/// what a Steel value is, and cannot get a different answer from the REPL for
/// the same source — because it is the *same* call into the *same* runtime, made
/// from a different front-end.
pub(crate) trait Evaluate {
    /// Evaluates scheme source, answering what `6b` draws after `⇒`.
    fn eval(&mut self, source: &str) -> Outcome;
}

// ---------------------------------------------------------------------------
// What a door call produced
// ---------------------------------------------------------------------------

/// The result of one call through this door.
///
/// Two cases because the registry has two kinds: an Action answers an
/// [`Outcome`] (it happened, or it was refused), a query answers a
/// [`Value`]. `CapabilityKind` is what chooses, so neither can be reached
/// through the other's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    /// An Action's outcome — what `--eval` and the REPL both produce.
    Acted(Outcome),
    /// A query's result, a snapshot of owned data.
    ///
    /// Nothing constructs this yet, and the compiler saying so is the truthful
    /// state of the read side: a query is a projection of a store snapshot
    /// (`query.rs`), the store is `S5`, and until then every query answers
    /// `Acted(Refused(NotYetImplemented))` naming the task that builds it. The
    /// variant is here rather than added later because the *rendering* differs —
    /// a query answers a value, not a receipt — and that is a shape `T024`'s
    /// parity test reads.
    #[expect(
        dead_code,
        reason = "the read side has no store to project until S5; see above"
    )]
    Read(Value),
}

impl Answer {
    /// Whether the editor did what was asked.
    ///
    /// A refusal is **not an error** (`action.rs`: a bare directory has no VCS),
    /// but it is not a success either, and a shell script that seeded state
    /// through this door has to be able to tell. That is the whole reason this
    /// exists: `V006` builds tape fixtures out of `--eval` calls, and a refusal
    /// that exited `0` would produce a wrong recording silently.
    ///
    /// **Ruled at `§14`: the two routes through this door had to agree, and
    /// they did not.** Measured against the tree rather than reasoned about:
    ///
    /// ```text
    /// phosphor mark-seen --target=cursor        #refused · not built yet …   exit 1
    /// phosphor --eval '(mark-seen! "a.rs:1")'   (#refused "not built yet …") exit 0
    /// ```
    ///
    /// One door, one refusal, two exit codes. The verb route decodes to an
    /// Action and gets [`Outcome::Refused`], which the first arm catches; the
    /// eval route runs *scheme*, and the capability's refusal comes back as the
    /// value that scheme evaluated to — a `(#refused "…")` list inside an
    /// `Outcome::Done`. The evaluation genuinely succeeded, which is why the
    /// shape is right and the exit code was wrong.
    ///
    /// So the eval route now reads its own result. This is not a new contract:
    /// it is `T023`'s existing one, applied to the route that was skipping it.
    ///
    /// **Why now, when `§14` says nothing is wrong today.** Because today is
    /// when it is free. Nothing reads `$?` from this route yet — the parity
    /// walk reads stdout and `scripts/seed-fixtures.sh` matches the refusal
    /// text before it ever consults `code` — and `§14`'s trap springs the day
    /// `T041` lands and refusals start turning into successes. Changing it
    /// while every caller is indifferent is the cheapest this will ever be.
    ///
    /// Distinct from `T100`, which was about the *voice* of a refusal and added
    /// the [`Outcome::Raised`] case for "it ran and raised". This is only the
    /// exit code, and it reads the shape `phosphor_steel::registry::REFUSED`
    /// already defines — the constant whose own doc comment says it is two
    /// elements
    /// "so the reason survives to the REPL and to a composition that wants to
    /// branch on it". This is such a composition.
    fn happened(&self) -> bool {
        match self {
            // `T100` split the second of these off the first. A raise exits
            // non-zero for the same reason a refusal does — nothing happened,
            // and `V006` seeds tape fixtures through this door — but it is a
            // different fact, and `render` now says which.
            Self::Acted(Outcome::Refused(_) | Outcome::Raised(_)) => false,
            Self::Acted(Outcome::Done(receipt)) => !Self::is_refusal(&receipt.value),
            Self::Read(_) => true,
        }
    }

    /// A refusal the VM answered as data, rather than one the dispatcher
    /// returned as an [`Outcome`].
    fn is_refusal(value: &Value) -> bool {
        matches!(
            value,
            Value::List(items)
                if matches!(
                    items.first(),
                    Some(Value::Text(head)) if head == phosphor_steel::registry::REFUSED
                )
        )
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a command line could not become a call, or a call an Action.
///
/// Distinct from a [`Refusal`], exactly as `action.rs` draws the line: this is a
/// *malformed* request, not a well-formed one the editor declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DoorError {
    /// No capability has that name.
    Unknown {
        /// What was asked for.
        name: String,
    },
    /// The flags did not assemble into a call.
    Flags(CliError),
    /// The call did not decode into an Action.
    Action(ActionError),
    /// The call did not decode into a query.
    Query(QueryError),
    /// `--eval`'s sugar assumes the `eval` row takes one required text
    /// parameter. If that stops being true, this says so instead of silently
    /// sending the source to the wrong argument.
    EvalShape,
}

impl fmt::Display for DoorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { name } => write!(f, "no such capability `{name}`"),
            Self::Flags(error) => write!(f, "{error}"),
            Self::Action(error) => write!(f, "{error}"),
            Self::Query(error) => write!(f, "{error}"),
            Self::EvalShape => write!(
                f,
                "`--eval` is sugar for the `{EVAL}` capability, which no longer takes one \
                 required text argument"
            ),
        }
    }
}

impl Error for DoorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Flags(error) => Some(error),
            Self::Action(error) => Some(error),
            Self::Query(error) => Some(error),
            Self::Unknown { .. } | Self::EvalShape => None,
        }
    }
}

// ---------------------------------------------------------------------------
// The parser
// ---------------------------------------------------------------------------

/// The host's own parser, extended with one subcommand per capability.
///
/// **One parser, not two.** The root manifest's `clap` entry says `T090` is its
/// first consumer and *"`T023` takes the same parser when `--eval` lands"*; this
/// takes `Cli::command()` and adds to it, so `--theme`, `--float` and the file
/// argument keep one definition.
///
/// The verbs are hidden from the top-level help rather than absent from it: 215
/// subcommands would bury the four flags a person actually types, and a door you
/// discover by name (`phosphor <verb> --help`) is the shape every multi-verb CLI
/// already has. [`after_help`] says so in one line, with the count derived.
pub(crate) fn parser(host: Command) -> Command {
    let generated = verbs();
    let note = after_help(&generated);
    host.subcommand_negates_reqs(true)
        .args_conflicts_with_subcommands(true)
        .after_help(note)
        .subcommands(generated.iter().map(subcommand))
}

/// One capability's subcommand: its flags, in declaration order.
fn subcommand(verb: &Verb) -> Command {
    let mut command = Command::new(verb.verb)
        .about(verb.about)
        .hide(true)
        .args(verb.flags.iter().map(argument));
    if verb.needs_eval() {
        command = command.after_help(format!(
            "`{}` has arguments with no flag form ({}) — call it with `--eval`.",
            verb.verb,
            verb.unreachable.join(", ")
        ));
    }
    command
}

/// One flag, as the registry declared it.
///
/// Every value stays a `String` here on purpose: parsing is
/// [`cli::parse_scalar`](phosphor_core::registry::cli::parse_scalar)'s, and a
/// second parser in clap's value-parser slot is a second answer to *"what is a
/// `--line`"*. Choices are advertised through the value name for the same
/// reason — [`FlagValue::value_name`] already spells `<REGION|FILE|…>`.
fn argument(flag: &phosphor_core::registry::cli::Flag) -> Arg {
    let mut arg = Arg::new(flag.long.clone())
        .long(flag.long.clone())
        .help(flag.help);
    arg = match &flag.value {
        FlagValue::Switch => arg.action(ArgAction::SetTrue),
        FlagValue::One(_) | FlagValue::Arm(_) => arg.action(ArgAction::Set),
        FlagValue::Many(_) => arg.action(ArgAction::Append),
    };
    if let Some(name) = flag.value.value_name() {
        arg = arg.value_name(name);
    }
    match &flag.requirement {
        Requirement::Always => arg.required(true),
        Requirement::Optional => arg,
        Requirement::WithArm { flag: on, tags } => {
            arg.required_if_eq_any(tags.iter().map(|tag| (on.clone(), *tag)))
        }
    }
}

/// The one line that tells a reader the hidden two thirds of this CLI exist.
fn after_help(generated: &[Verb]) -> String {
    let actions = generated
        .iter()
        .filter(|verb| verb.kind == CapabilityKind::Action)
        .count();
    format!(
        "The CLI door: {} capability verbs ({actions} actions, {} queries) are also accepted, one \
         per registered Action and query. `phosphor <verb> --help` describes one; \
         `phosphor --eval '(<verb> …)'` is the same capability through Steel.",
        generated.len(),
        generated.len() - actions,
    )
}

// ---------------------------------------------------------------------------
// argv → Call
// ---------------------------------------------------------------------------

/// The call a matched subcommand asked for.
///
/// The walk is [`cli::assemble`](phosphor_core::registry::cli::assemble)'s, not this file's: clap hands back the flags
/// that were supplied, and the registry reassembles them into the same
/// door-neutral [`Call`] Steel and MCP produce.
///
/// # Errors
///
/// [`DoorError::Unknown`] for a verb no capability declares (unreachable through
/// clap, reachable through a test), or [`DoorError::Flags`] for a value that is
/// not the shape its parameter declared.
pub(crate) fn call(verb: &str, matches: &ArgMatches) -> Result<Call, DoorError> {
    let capability = capability(verb)?;
    let generated = phosphor_core::registry::cli::verb(&capability);
    assemble(&capability, &occurrences(&generated, matches)).map_err(DoorError::Flags)
}

/// The flags clap actually saw, in the registry's declaration order.
fn occurrences(verb: &Verb, matches: &ArgMatches) -> Vec<Occurrence> {
    let mut out = Vec::new();
    for flag in &verb.flags {
        match flag.value {
            FlagValue::Switch => {
                if matches.get_flag(&flag.long) {
                    out.push(Occurrence::switch(&flag.long));
                }
            }
            FlagValue::One(_) | FlagValue::Arm(_) => {
                if let Some(value) = matches.get_one::<String>(&flag.long) {
                    out.push(Occurrence::valued(&flag.long, value));
                }
            }
            FlagValue::Many(_) => {
                for value in matches.get_many::<String>(&flag.long).into_iter().flatten() {
                    out.push(Occurrence::valued(&flag.long, value));
                }
            }
        }
    }
    out
}

/// `--eval EXPR`, as the call `phosphor eval --source EXPR` makes.
///
/// The parameter name is read off the registry row rather than spelled here, so
/// the sugar cannot drift from the capability it is sugar for.
///
/// # Errors
///
/// [`DoorError::Unknown`] if the row is gone, [`DoorError::EvalShape`] if it no
/// longer takes exactly one required text argument.
pub(crate) fn eval_call(source: &str) -> Result<Call, DoorError> {
    let capability = capability(EVAL)?;
    let param = match capability.params {
        [only] => only,
        _ => return Err(DoorError::EvalShape),
    };
    if !param.required || !matches!(param.ty, ParamType::Text) {
        return Err(DoorError::EvalShape);
    }
    Ok(Call::new(capability.name).with(param.name, Value::Text(source.to_owned())))
}

/// The registry row for a door name, following [`ALIASES`](phosphor_core::action::ALIASES).
///
/// `6b` draws `(watch-place …)` while the naming rule says `place-watch`; the
/// alias lives in the vocabulary, so resolving through it here costs this door
/// no knowledge of which names are aliased.
fn capability(name: &str) -> Result<Capability, DoorError> {
    lookup(Action::canonical_name(name)).ok_or_else(|| DoorError::Unknown {
        name: name.to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Call → Answer
// ---------------------------------------------------------------------------

/// Runs one call and answers what the editor said.
///
/// # Errors
///
/// [`DoorError`] for a malformed call. A *refused* call is not an error — it is
/// an [`Answer`].
pub(crate) fn answer(call: &Call, runtime: Option<&mut dyn Evaluate>) -> Result<Answer, DoorError> {
    let capability = capability(&call.name)?;
    match capability.kind {
        CapabilityKind::Action => {
            let action =
                Action::from_call(capability.name, &call.args).map_err(DoorError::Action)?;
            let request = Request::new(Actor::Cli, Door::Cli, action);
            Ok(Answer::Acted(apply(&request, runtime)))
        }
        CapabilityKind::Query => {
            let query = Query::from_call(capability.name, &call.args).map_err(DoorError::Query)?;
            // The read side needs the store, which is `S5`. Until then a query
            // answers the same refusal an Action does, naming the task that
            // builds it — `T041` and its neighbours.
            Ok(Answer::Acted(not_yet(query.spec().since.task)))
        }
    }
}

/// The one dispatcher this door has, and it is not a match on capability names.
///
/// At `S2` there is no store, so every Action that touches one answers
/// [`Refusal::NotYetImplemented`] carrying **its own row's task id** — derived,
/// not listed, which is why 215 capabilities need no table here. The single
/// special case is the one capability whose implementation *is* a runtime rather
/// than a store: scheme source needs a VM, and the VM is [`Evaluate`].
fn apply(request: &Request, runtime: Option<&mut dyn Evaluate>) -> Outcome {
    match (&request.action, runtime) {
        (Action::Runtime(RuntimeAction::Eval { source }), Some(runtime)) => runtime.eval(source),
        (Action::Runtime(RuntimeAction::Eval { .. }), None) => not_yet(RUNTIME_TASK),
        (action, _) => not_yet(action.spec().since.task),
    }
}

/// Named in the vocabulary, not built yet — and here is the task that builds it.
const fn not_yet(task: &'static str) -> Outcome {
    Outcome::Refused(Refusal::NotYetImplemented { task })
}

// ---------------------------------------------------------------------------
// Answer → one line of stdout
// ---------------------------------------------------------------------------

/// One line, in `6b`'s shape.
///
/// The REPL draws `⇒ #ok · persisted to init.scm` and
/// `⇒ #watch-3 · streaming from next run` (TUI Mockups `6b`). The `⇒` is REPL
/// chrome; the body is this. A receipt with no value is `#ok`, a note follows
/// after ` · `, and a refusal says which of `Refusal`'s cases it was — including
/// the task id, because *"not built yet — `T041` builds it"* is the answer that
/// tells a caller what to do next.
///
/// **This door does not phrase an outcome at all** — `T100`, finished. It used
/// to carry its own `match`, spelling the `#ok` sigil, the midline dot and the
/// `#refused` head a second time beside [`phosphor_steel::answer::answered`]'s;
/// the two agreed only because a test compared them byte for byte, which is a
/// convention with a guard rather than a structure. There is now one `match` on
/// [`Outcome`] behind both front-ends, so the REPL's `⇒` line and this door's
/// stdout cannot drift without somebody deleting this call — and `#raised`, the
/// case `T100` added, needed no edit here at all.
pub(crate) fn render(answer: &Answer) -> String {
    match answer {
        Answer::Acted(outcome) => phosphor_steel::answer::line(outcome),
        Answer::Read(value) => write_value(value),
    }
}

/// A [`Value`] written the way the vocabulary is read: scheme.
///
/// **One writer, both doors.** `T022` moved it to
/// [`phosphor_steel::answer::value`] when the REPL became a second surface
/// printing the same values: `phosphor --eval '(set-option! …)'` and `6b`'s `⇒`
/// line are the same text because they are the same function, and the
/// `#`-sigil rule that keeps `#ok` from printing as `"#ok"` lives in one place.
///
/// **And now the refusals too.** This file used to carry its own `why`, saying
/// `T041 builds this` where the Steel door and the REPL said `not built yet —
/// T041 builds it` — one enum in two voices, flagged here and ruled into
/// `T100`. That function is gone rather than repointed: the phrasing is
/// [`Refusal::why`], on the enum, so the doors cannot drift again without
/// somebody writing a second `match` on purpose.
fn write_value(value: &Value) -> String {
    phosphor_steel::answer::value(value)
}

// ---------------------------------------------------------------------------
// The front end
// ---------------------------------------------------------------------------

/// Runs one call, prints one line, and answers the process's exit code.
///
/// **No terminal is touched on this path.** It is called before `main` builds a
/// [`Term`](phosphor_term::Term), so there is no alternate screen to restore and
/// nothing to corrupt by writing to stdout — which is why this is the one
/// function in the binary that prints a result rather than a diagnostic.
///
/// # Errors
///
/// [`DoorError`] for a malformed call, reported by `main` on stderr.
#[expect(
    clippy::print_stdout,
    reason = "the CLI door is the one legitimate writer to stdout; the TUI is not running here"
)]
pub(crate) fn run(call: &Call, runtime: Option<&mut dyn Evaluate>) -> Result<ExitCode, DoorError> {
    let answer = answer(call, runtime)?;
    println!("{}", render(&answer));
    Ok(if answer.happened() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// What is worth proving here is that this door is *derived* and that its two
// front-ends are one path. Everything else about it — that a `--line` wants an
// integer, that a union arm picks its fields — is `cli.rs`'s, tested there
// against the registry rather than re-tested against clap here.

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use phosphor_core::action::{Raised, Receipt};
    use phosphor_core::registry::capabilities;
    use phosphor_core::value::Args;

    use super::*;

    /// A runtime that answers with the source it was given, so two front-ends
    /// can be compared without `T021`.
    struct Echo;

    impl Evaluate for Echo {
        fn eval(&mut self, source: &str) -> Outcome {
            Outcome::Done(Receipt {
                capability: EVAL,
                value: Value::Text(source.to_owned()),
                note: Some("evaluated".to_owned()),
            })
        }
    }

    fn parse(argv: &[&str]) -> ArgMatches {
        parser(Command::new("phosphor"))
            .try_get_matches_from(argv)
            .expect("argv parses")
    }

    #[test]
    fn the_eval_flag_and_the_eval_verb_assemble_the_same_call() {
        // `T023`'s acceptance criterion, at the level this crate can hold it:
        // the two front-ends are the same call, so they cannot disagree about
        // what was asked before anything evaluates it.
        let source = "(mark-seen! (region 3))";
        let matches = parse(&["phosphor", EVAL, "--source", source]);
        let (verb, sub) = matches.subcommand().expect("a verb matched");

        assert_eq!(
            eval_call(source).expect("the sugar assembles"),
            call(verb, sub).expect("the verb assembles"),
        );
    }

    #[test]
    fn both_front_ends_get_the_same_outcome_from_one_runtime() {
        let source = "(+ 1 2)";
        let matches = parse(&["phosphor", EVAL, "--source", source]);
        let (verb, sub) = matches.subcommand().expect("a verb matched");

        let sugar = answer(&eval_call(source).unwrap(), Some(&mut Echo)).unwrap();
        let generated = answer(&call(verb, sub).unwrap(), Some(&mut Echo)).unwrap();

        assert_eq!(sugar, generated);
        assert_eq!(render(&sugar), format!("{source:?} · evaluated"));
    }

    #[test]
    fn every_capability_is_a_verb_and_the_parser_has_no_extras() {
        let command = parser(Command::new("phosphor"));
        let generated: Vec<&str> = command
            .get_subcommands()
            .map(|sub| sub.get_name())
            .collect();
        let registered: Vec<&str> = capabilities().iter().map(|cap| cap.name).collect();

        // Equality both ways: a verb the registry does not declare would be a
        // second registry, which is exactly what `lint-one-registry.sh` and
        // `T024` exist to make impossible.
        assert_eq!(generated.len(), registered.len());
        for name in registered {
            assert!(generated.contains(&name), "`{name}` has no verb");
        }
    }

    #[test]
    fn a_verbs_flags_are_the_registry_rows_flags() {
        // The one shape that would let this file grow a table: flags spelled
        // here rather than read. Checked against a capability with a union, a
        // record and a switch in one row.
        let matches = parse(&[
            "phosphor",
            "mark-seen",
            "--target",
            "region",
            "--target.region.id",
            "3",
        ]);
        let (verb, sub) = matches.subcommand().expect("a verb matched");
        let assembled = call(verb, sub).expect("assembles");

        assert_eq!(assembled.name, "mark-seen");
        let action = Action::from_call(&assembled.name, &assembled.args).expect("decodes");
        assert_eq!(action.name(), "mark-seen");
    }

    #[test]
    fn an_unbuilt_capability_names_the_task_that_builds_it() {
        let matches = parse(&[
            "phosphor",
            "mark-seen",
            "--target",
            "region",
            "--target.region.id",
            "3",
        ]);
        let (verb, sub) = matches.subcommand().expect("a verb matched");
        let answered = answer(&call(verb, sub).unwrap(), None).unwrap();

        let task = lookup("mark-seen").expect("registered").since.task;
        assert_eq!(
            answered,
            Answer::Acted(Outcome::Refused(Refusal::NotYetImplemented { task })),
            "the refusal is read off the row, not written here"
        );
        // The one voice, phrased by `Refusal::why` and not by this door
        // (`T100`). Asserted as the whole line rather than a `contains`, so a
        // door that grew a second phrasing fails here.
        assert_eq!(
            render(&answered),
            format!("#refused · not built yet — {task} builds it")
        );
        assert!(!answered.happened(), "a refusal must not exit zero");
    }

    #[test]
    fn eval_without_a_runtime_names_the_task_that_embeds_one() {
        // `T023` is this door; `T021` is the VM it delegates to. Saying `T023`
        // here would be the lie — the door exists.
        let answered = answer(&eval_call("(+ 1 2)").unwrap(), None).unwrap();
        assert_eq!(
            render(&answered),
            format!("#refused · not built yet — {RUNTIME_TASK} builds it")
        );
    }

    #[test]
    fn a_query_answers_its_own_row_rather_than_an_action_error() {
        let call = Call::new("unseen-regions").with("path", Value::Text("src/retry.rs".to_owned()));
        let answered = answer(&call, None).expect("a well-formed query");
        let task = lookup("unseen-regions").expect("registered").since.task;
        assert_eq!(
            render(&answered),
            format!("#refused · not built yet — {task} builds it")
        );
    }

    /// Every case of [`Refusal`], so the checks below are over the enum and not
    /// over the three someone thought of.
    ///
    /// The `match` is the guard: it names each variant once and the compiler
    /// rejects it the day an eighth appears, so this list cannot go stale
    /// quietly — the same shape `interpret.rs` uses for its deferred node
    /// kinds.
    fn every_refusal() -> Vec<Refusal> {
        let all = vec![
            Refusal::NotYetImplemented { task: "T041" },
            Refusal::FocusRelativeTargetOverMcp,
            Refusal::DoorDenied { door: Door::Mcp },
            Refusal::NoRepository,
            Refusal::NoSuchTarget,
            Refusal::WouldLoseWork,
            Refusal::Declined {
                reason: "a rule said no".to_owned(),
            },
        ];
        for refusal in &all {
            match refusal {
                Refusal::NotYetImplemented { .. }
                | Refusal::FocusRelativeTargetOverMcp
                | Refusal::DoorDenied { .. }
                | Refusal::NoRepository
                | Refusal::NoSuchTarget
                | Refusal::WouldLoseWork
                | Refusal::Declined { .. } => {}
            }
        }
        all
    }

    #[test]
    fn one_enum_value_is_one_sentence_and_this_is_the_sentence() {
        // `T100`'s *done when*, and the reason it was a task: this door used to
        // say `T041 builds this` where `phosphor-steel` said
        // `not built yet — T041 builds it`.
        //
        // This replaces a test that compared the two front-ends to each other.
        // That test can no longer fail — `render` calls
        // `phosphor_steel::answer::line`, so it would be comparing a function
        // with itself — and a pair that agree are still free to agree on the
        // wrong words. So the expectation is written out. `every_refusal`'s own
        // `match` is what stops the list going stale: an eighth variant is a
        // compile error there and a missing row here.
        let expected = [
            (
                Refusal::NotYetImplemented { task: "T041" },
                "#refused · not built yet — T041 builds it",
            ),
            (
                Refusal::FocusRelativeTargetOverMcp,
                "#refused · an agent has no cursor — name the target",
            ),
            (
                Refusal::DoorDenied { door: Door::Mcp },
                "#refused · the mcp door refuses this — open it in init.scm",
            ),
            (Refusal::NoRepository, "#refused · no repository here"),
            (
                Refusal::NoSuchTarget,
                "#refused · no such target — it was dropped or closed",
            ),
            (
                Refusal::WouldLoseWork,
                "#refused · unsaved work — force it or save first",
            ),
            (
                Refusal::Declined {
                    reason: "a rule said no".to_owned(),
                },
                "#refused · a rule said no",
            ),
        ];

        for refusal in every_refusal() {
            let (_, line) = expected
                .iter()
                .find(|(candidate, _)| *candidate == refusal)
                .unwrap_or_else(|| panic!("`{refusal:?}` has no sentence in this table"));
            assert_eq!(&render(&Answer::Acted(Outcome::Refused(refusal))), line);
        }
        assert_eq!(expected.len(), every_refusal().len());
    }

    #[test]
    fn a_raise_is_not_a_refusal_and_the_door_says_so() {
        // `T100`'s other half. Before it, `phosphor --eval '(car 5)'` printed
        // `#refused · Error: TypeMismatch: car expected a list or pair, found: 5`
        // — the wrong head, because nothing declined anything, and Steel's own
        // envelope inside a line Design Language §6 owns.
        let raised = Answer::Acted(Outcome::Raised(Raised {
            kind: Some("wrong type"),
            message: "car expected a list or pair, found: 5".to_owned(),
        }));
        assert_eq!(
            render(&raised),
            "#raised · wrong type — car expected a list or pair, found: 5"
        );
        assert!(!raised.happened(), "a raise must not exit zero");

        // The kind is optional because a `QueryError` is already a finished
        // sentence — the case the task was reported from.
        let query = Answer::Acted(Outcome::Raised(Raised {
            kind: None,
            message: "not built yet — T041 builds it".to_owned(),
        }));
        assert_eq!(render(&query), "#raised · not built yet — T041 builds it");
    }

    #[test]
    fn no_refusal_leaks_a_type_name_or_shouts() {
        // Design Language §6 — *lowercase, telegraphic, factual*. What would
        // break it here is a Rust identifier reaching a reader, a capitalised
        // opener, or the exclamation §6 names outright ("Oops! Something went
        // wrong"). A task id stays uppercase on purpose: `T041` is a name the
        // caller looks up, like `:reattach` in §6's own example.
        //
        // `Declined`'s text belongs to whichever rule, hook or user wrote it,
        // so what this asserts for that case is the fixture above and not the
        // product. Every other case is the product.
        const LEAKS: [&str; 5] = ["Refusal", "Outcome", "Steel", "Error:", "::"];
        for refusal in every_refusal() {
            let line = refusal.why();
            assert!(!line.is_empty(), "{refusal:?} says nothing");
            for leak in LEAKS {
                assert!(!line.contains(leak), "`{line}` leaks `{leak}`");
            }
            assert!(
                !line.starts_with(char::is_uppercase),
                "`{line}` opens capitalised"
            );
            assert!(
                !line.ends_with('.') && !line.contains('!'),
                "`{line}` is a sentence, not a fact"
            );
        }
    }

    #[test]
    fn an_alias_reaches_its_canonical_row() {
        // `6b` draws `(watch-place …)`; the row is `place-watch`. The alias
        // lives in the vocabulary, so the door needs no knowledge of it.
        let call = Call::new("watch-place")
            .with("anchor", Value::tagged("file", Args::new()))
            .with("expr", Value::Text("delay".to_owned()));
        let error = answer(&call, None).unwrap_err();
        assert!(
            !matches!(error, DoorError::Unknown { .. }),
            "the alias resolved: {error}"
        );
    }

    #[test]
    fn a_name_no_capability_declares_is_an_error_not_a_refusal() {
        let error = answer(&Call::new("mark-read"), None).unwrap_err();
        assert_eq!(
            error,
            DoorError::Unknown {
                name: "mark-read".to_owned()
            }
        );
    }

    #[test]
    fn a_receipt_renders_as_6b_draws_it() {
        let ok = Answer::Acted(Outcome::Done(Receipt {
            capability: "set-keybinding",
            value: Value::Null,
            note: Some("persisted to init.scm".to_owned()),
        }));
        assert_eq!(render(&ok), "#ok · persisted to init.scm");

        let bare = Answer::Acted(Outcome::Done(Receipt::ok("close-float")));
        assert_eq!(render(&bare), "#ok");

        let list = Answer::Acted(Outcome::Done(Receipt {
            capability: "regions",
            value: Value::List(vec![Value::Int(4), Value::Bool(true)]),
            note: None,
        }));
        assert_eq!(render(&list), "(4 #t)");
    }

    #[test]
    fn the_host_flags_survive_the_extension() {
        // One parser, not two: adding 215 subcommands must not cost `--theme`
        // or the file argument, and a subcommand must not demand the file.
        let matches = parser(crate::Cli::command())
            .try_get_matches_from(["phosphor", "--theme", "tokyo-night", "src/main.rs"])
            .expect("the host's own line still parses");
        assert!(matches.subcommand().is_none());

        parser(crate::Cli::command())
            .try_get_matches_from(["phosphor", "close-float"])
            .expect("a verb does not need the FILE argument");
    }
}
