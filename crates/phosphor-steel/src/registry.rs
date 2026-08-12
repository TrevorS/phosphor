//! Installing the vocabulary into the VM — every capability, no table of our
//! own.
//!
//! > *"`T021`'s `phosphor-steel/registry.rs` installs [`bindings`] into the VM;
//! > it holds no table of its own, and the moment it does, invariant 2 is
//! > decorative."* — `phosphor_core::registry`, module docs
//!
//! So [`install`] is a `for` loop over [`bindings`] and [`alias_bindings`], and
//! there is deliberately no way to register a capability from here. Adding one
//! is a row in `action.rs`'s or `query.rs`'s table; this file does not change.
//!
//! # What a binding does when it is called
//!
//! 1. Decodes the positional arguments into the wire model ([`crate::convert`]).
//! 2. Maps them onto named arguments — the calling convention is
//!    `phosphor_core::registry::steel`'s, not ours (`Binding::call`).
//! 3. Turns the call into an [`Action`] or a [`Query`], which is where a wrong
//!    *shape* is caught, with the argument named.
//! 4. Hands it to the [`Host`] — the barrier, and the end of what Steel can
//!    reach.
//!
//! # Refusals are values; errors are errors
//!
//! An Action that is refused answers a **value**: a refusal is a normal state
//! (`action.rs`, `Refusal`), and a `runtime/*.scm` file whose form aborted
//! because a bare directory has no VCS would be the editor breaking on a fact
//! about the world. A *query* that cannot be answered raises instead — you
//! asked for data and there is none, and there is no value that honestly stands
//! for that.
//!
//! Owned by `spine`.

use std::sync::Arc;

use phosphor_core::action::{Action, Outcome, Refusal, Request};
use phosphor_core::query::Query;
use phosphor_core::registry::steel::{Binding, alias_bindings, bindings};
use phosphor_core::registry::{CapabilityKind, Door};
use phosphor_core::request::Actor;
use phosphor_core::value::{Call, Value};
use steel::rerrs::ErrorKind;
use steel::steel_vm::engine::Engine;
use steel::{SteelErr, SteelVal};

use crate::convert::{from_steel, to_steel};
use crate::host::{Host, ReceiptLog};

/// What an Action that produced no value of its own answers.
///
/// `6b` draws `⇒ #ok · persisted to init.scm` (TUI Mockups.dc.html:499). The
/// `#`-sigil is the mockups' own spelling for an opaque handle — `#ok`,
/// `#watch-3`, `#region 4` — so the symbol is the drawing, not an invention.
pub const OK: &str = "#ok";

/// What a refused Action answers: `(#refused "reason")`.
///
/// A two-element list rather than a bare symbol so the reason survives to the
/// REPL and to a composition that wants to branch on it. The reason reads in
/// the product's voice, because it is the same text a float would show.
pub const REFUSED: &str = "#refused";

/// Installs every capability, and every alias, into `engine`.
///
/// Returns how many identifiers were bound — capabilities plus aliases. The
/// count is what `T024`'s parity test compares against
/// [`phosphor_core::registry::capabilities`]; it is returned rather than logged
/// because nothing in this crate may write to the terminal (`lib.rs`).
pub fn install(engine: &mut Engine, host: &Arc<dyn Host>, log: &ReceiptLog) -> usize {
    let mut installed = 0;

    for binding in bindings() {
        let name = binding.name.clone();
        engine.register_value(&name, native(binding, host, log));
        installed += 1;
    }

    // An alias is an extra name for a registered capability, never a
    // capability of its own (`registry::steel`, `AliasBinding`). It resolves to
    // the same row, so it gets the same closure over the same capability.
    for alias in alias_bindings() {
        let Some(binding) = bindings()
            .into_iter()
            .find(|candidate| candidate.capability == alias.capability)
        else {
            // Unreachable: `alias_bindings` panics first, and
            // `tests/vocabulary.rs` proves every alias resolves. Skipping is
            // still the right shape — a boot must not die of a bad alias.
            continue;
        };
        engine.register_value(alias.name, native(binding, host, log));
        installed += 1;
    }

    installed
}

/// One capability as a scheme procedure.
fn native(binding: Binding, host: &Arc<dyn Host>, log: &ReceiptLog) -> SteelVal {
    let host = Arc::clone(host);
    let log = log.clone();

    SteelVal::anonymous_boxed_function(Arc::new(move |args: &[SteelVal]| {
        let call = decode(&binding, args)?;
        match binding.kind {
            CapabilityKind::Action => apply(&host, &log, &call),
            CapabilityKind::Query => answer(&host, &call),
        }
    }))
}

/// Positional scheme arguments to a door-neutral call.
fn decode(binding: &Binding, args: &[SteelVal]) -> Result<Call, SteelErr> {
    let values = args
        .iter()
        .map(from_steel)
        .collect::<Result<Vec<Value>, _>>()
        .map_err(|error| {
            SteelErr::new(
                ErrorKind::TypeMismatch,
                format!("{} — {error}", binding.name),
            )
        })?;

    binding
        .call(values)
        .map_err(|error| SteelErr::new(ErrorKind::ArityMismatch, error.to_string()))
}

/// Applies an Action and answers what it produced.
fn apply(host: &Arc<dyn Host>, log: &ReceiptLog, call: &Call) -> Result<SteelVal, SteelErr> {
    let action = Action::from_call(&call.name, &call.args)
        .map_err(|error| SteelErr::new(ErrorKind::TypeMismatch, error.to_string()))?;

    // `Actor::Steel` — *"`runtime/*.scm` acting on its own behalf"*
    // (`request.rs`). A keymap thunk you pressed is still Steel asking; the
    // store keeps who asked, and Design Language §7 is unconditional that your
    // own edits create no regions.
    let request = Request::new(Actor::Steel, Door::Steel, action);
    let outcome = host.apply(&request);
    let capability = request.action.spec().name;
    let answer = outcome_value(&outcome);
    log.push(capability, outcome);
    Ok(answer)
}

/// Answers a query, or raises.
fn answer(host: &Arc<dyn Host>, call: &Call) -> Result<SteelVal, SteelErr> {
    let query = Query::from_call(&call.name, &call.args)
        .map_err(|error| SteelErr::new(ErrorKind::TypeMismatch, error.to_string()))?;

    host.answer(&query)
        .map(|answer| to_steel(&answer.value))
        .map_err(|error| SteelErr::new(ErrorKind::Generic, error.to_string()))
}

/// An [`Outcome`] as scheme sees it — see [`OK`] and [`REFUSED`].
fn outcome_value(outcome: &Outcome) -> SteelVal {
    match outcome {
        Outcome::Done(receipt) => match &receipt.value {
            Value::Null => SteelVal::SymbolV(OK.into()),
            value => to_steel(value),
        },
        Outcome::Refused(refusal) => SteelVal::ListV(
            [
                SteelVal::SymbolV(REFUSED.into()),
                SteelVal::StringV(refusal_text(refusal).into()),
            ]
            .into_iter()
            .collect(),
        ),
    }
}

/// Why an Action did not happen, in the product's voice.
///
/// Design Language §6: lowercase, telegraphic, factual; em dash for cause.
/// This text ends up in the REPL's `⇒` line, in the CLI door's stdout and in a
/// float, so it is written once — in [`crate::answer::why`], which `T022` moved
/// it to when the second surface appeared. This function is the scheme door's
/// call into it, kept as a name so `outcome_value` reads the same as before.
fn refusal_text(refusal: &Refusal) -> String {
    crate::answer::why(refusal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Detached;
    use phosphor_core::registry::capabilities;

    fn engine() -> Engine {
        let mut engine = Engine::new();
        let host: Arc<dyn Host> = Arc::new(Detached);
        install(&mut engine, &host, &ReceiptLog::new());
        engine
    }

    #[test]
    fn every_capability_is_bound_and_the_count_is_the_registry_plus_aliases() {
        let mut engine = Engine::new();
        let host: Arc<dyn Host> = Arc::new(Detached);
        let installed = install(&mut engine, &host, &ReceiptLog::new());
        assert_eq!(installed, capabilities().len() + alias_bindings().len());

        // Bound, not merely counted: each name resolves to a procedure.
        for binding in bindings() {
            let value = engine
                .extract_value(&binding.name)
                .unwrap_or_else(|_| panic!("`{}` is not bound", binding.name));
            assert!(
                matches!(value, SteelVal::BoxedFunction(_)),
                "`{}` is bound to something that is not callable",
                binding.name
            );
        }
    }

    #[test]
    fn a_refused_action_answers_a_value_rather_than_raising() {
        let mut engine = engine();
        let out = engine
            .compile_and_run_raw_program("(reload-runtime!)")
            .expect("a refusal is a value, not an error");
        let Some(SteelVal::ListV(pair)) = out.last() else {
            panic!("a refusal answers `(#refused \"…\")`, got {out:?}");
        };
        let items: Vec<SteelVal> = pair.into_iter().cloned().collect();
        assert_eq!(items[0], SteelVal::SymbolV(REFUSED.into()));
        let SteelVal::StringV(reason) = &items[1] else {
            panic!("the reason is text");
        };
        assert!(reason.contains("T021"), "{reason}");
    }

    #[test]
    fn a_query_with_no_answer_raises() {
        let mut engine = engine();
        engine
            .compile_and_run_raw_program(r#"(unseen-regions "src/retry.rs")"#)
            .expect_err("a query with nothing to say raises rather than answering a sentinel");
    }

    #[test]
    fn the_wrong_number_of_arguments_is_caught_before_the_host() {
        let mut engine = engine();
        let error = engine
            .compile_and_run_raw_program("(mark-seen!)")
            .expect_err("`mark-seen!` takes one argument");
        assert_eq!(error.kind(), ErrorKind::ArityMismatch);
    }

    #[test]
    fn the_drawn_alias_is_callable_under_its_drawn_name() {
        // `6b` draws `(watch-place "src/retry.rs:24" 'delay)` — no bang,
        // noun-first. The *name* is what the alias exists for and is what this
        // proves. The drawn *arguments* do not decode: `place-watch`'s first
        // parameter is a `Target`, a tagged record, and the drawing passes
        // `"path:line"` text. That gap is `phosphor-core`'s to close (a `Target`
        // that parses from `path:line`) and is reported rather than papered
        // over here — `ALIASES` already flags the same line for Teej.
        let mut engine = engine();
        engine
            .compile_and_run_raw_program(
                r#"(watch-place (hash "kind" "file" "path" "src/retry.rs") "delay")"#,
            )
            .expect("the drawn spelling is callable");
    }

    #[test]
    fn a_tagged_record_crosses_as_a_hash() {
        // How a composition writes a `Target`: the union's tag rides in
        // `kind` (`value.rs`, `TAG_FIELD`), and a hash is how scheme spells a
        // record.
        let mut engine = engine();
        engine
            .compile_and_run_raw_program(r#"(mark-seen! (hash "kind" "region" "id" 3))"#)
            .expect("a tagged record decodes into a payload");
    }

    #[test]
    fn a_closure_cannot_be_passed_across_the_barrier() {
        let mut engine = engine();
        let error = engine
            .compile_and_run_raw_program("(set-option! \"soft-wrap\" (lambda () 1))")
            .expect_err("a payload is plain data");
        assert_eq!(error.kind(), ErrorKind::TypeMismatch);
    }

    #[test]
    fn an_applied_action_is_logged_with_its_capability() {
        let log = ReceiptLog::new();
        let mut engine = Engine::new();
        let host: Arc<dyn Host> = Arc::new(Detached);
        install(&mut engine, &host, &log);
        engine
            .compile_and_run_raw_program("(reload-runtime!)")
            .expect("a refusal is a value");
        let logged = log.take();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].capability, "reload-runtime");
    }
}
