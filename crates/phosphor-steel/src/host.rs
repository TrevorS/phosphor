//! The barrier — what Steel is allowed to reach, and nothing else.
//!
//! > *"Safety comes from the barrier, not from ceremony: Steel can only emit
//! > Actions and read ViewModels, so live redefinition can misconfigure but
//! > never corrupt a buffer."* — Component Breakdown, *philosophy*
//!
//! That sentence is this trait. A [`Host`] has exactly two methods — apply a
//! [`Request`], answer a [`Query`] — because those are the only two verbs the
//! sentence allows. There is no third method, no handle to the store, and
//! nothing that can hand a `&mut Buffer` across (`lib.rs`).
//!
//! [`Answers`] is `phosphor-core`'s, reused rather than re-declared: the read
//! side already had one trait and a second would be a second vocabulary.
//! `spine` owns the write side's seam because no such trait exists in
//! `phosphor-core` yet — it belongs there once the store applies Actions
//! (`T041`), and that move is a `phosphor-core` edit, so it is flagged rather
//! than made here.
//!
//! # Why `&self` and not `&mut self`
//!
//! Steel calls a binding *while the VM is running*, from inside a `Fn` that
//! Steel requires to be `Send + Sync + 'static`
//! (`steel-core-0.8.2/src/values/functions.rs:512-517`). A `&mut` host would
//! have to be re-entrantly borrowed out of the same object that owns the
//! engine. The interior mutability is the binary's, where the store already
//! lives.
//!
//! Owned by `spine`.

use std::sync::{Arc, Mutex};

use phosphor_core::action::{Outcome, Refusal, Request};
use phosphor_core::query::{Answer, Answers, Query, QueryError};

/// What the Steel door is allowed to reach.
///
/// Implemented by the binary once the loop owns a store. Until then
/// [`Detached`] stands in, and the whole editor still boots — which is the
/// property `T021` exists to prove.
pub trait Host: Answers + Send + Sync + 'static {
    /// Applies one request and reports what happened.
    ///
    /// Total: a [`Request`] the host will not carry out comes back as
    /// [`Outcome::Refused`], never as an error. A refusal is a normal state
    /// (`action.rs`, [`Refusal`]) — an agent that may not move your cursor and
    /// a capability whose phase has not landed are the same shape of answer.
    fn apply(&self, request: &Request) -> Outcome;
}

/// The host before there is a loop: everything is registered, nothing is built.
///
/// Not a mock and not test scaffolding — it is the truthful answer at `S2`.
/// Every capability is in the registry from `T019`, and
/// [`Refusal::NotYetImplemented`] names the task that builds each one, so a
/// boot against this host produces legible refusals rather than missing
/// bindings. `runtime/init.scm` runs end to end against it.
#[derive(Debug, Default, Clone, Copy)]
pub struct Detached;

impl Answers for Detached {
    fn answer(&self, query: &Query) -> Result<Answer, QueryError> {
        Err(QueryError::NotYetImplemented {
            task: query.spec().since.task,
        })
    }
}

impl Host for Detached {
    fn apply(&self, request: &Request) -> Outcome {
        Outcome::Refused(Refusal::NotYetImplemented {
            task: request.action.spec().since.task,
        })
    }
}

/// One door call and what it produced.
///
/// The [`Outcome`] alone cannot carry this: [`Outcome::Refused`] does not name
/// the capability that was refused, and `6b` draws the capability's own answer
/// with a note beside it — `⇒ #ok · persisted to init.scm`
/// (TUI Mockups.dc.html:499).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    /// The capability's canonical door name.
    pub capability: &'static str,
    /// What applying it produced.
    pub outcome: Outcome,
}

/// Where the outcomes of a form's Actions are kept until someone reads them.
///
/// A binding returns the Action's *value* to scheme — that is what a
/// composition computes with. The note beside it (`· persisted to init.scm`)
/// is chrome, and chrome that a `(map …)` over the result would have to step
/// around is chrome in the wrong place. So it rides out of band, and `T022`'s
/// REPL drains this after each evaluation to draw the `⇒` line.
///
/// A `Mutex` rather than a `RefCell` because Steel requires the binding
/// closures to be `Send + Sync`.
#[derive(Debug, Clone, Default)]
pub struct ReceiptLog(Arc<Mutex<Vec<Logged>>>);

impl ReceiptLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one outcome.
    ///
    /// Silent if the lock is poisoned. A panicking binding must not take the
    /// editor with it, and a lost `⇒` note is the smallest possible casualty.
    pub fn push(&self, capability: &'static str, outcome: Outcome) {
        if let Ok(mut log) = self.0.lock() {
            log.push(Logged {
                capability,
                outcome,
            });
        }
    }

    /// Takes everything recorded since the last drain.
    #[must_use]
    pub fn take(&self) -> Vec<Logged> {
        self.0
            .lock()
            .map(|mut log| core::mem::take(&mut *log))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::action::{Action, RuntimeAction};
    use phosphor_core::registry::Door;
    use phosphor_core::request::Actor;

    #[test]
    fn a_detached_host_refuses_by_naming_the_task() {
        let action = Action::Runtime(RuntimeAction::ReloadRuntime {});
        let request = Request::new(Actor::Steel, Door::Steel, action);
        let Outcome::Refused(Refusal::NotYetImplemented { task }) = Detached.apply(&request) else {
            panic!("a detached host refuses everything, legibly");
        };
        assert_eq!(task, "T094");
    }

    #[test]
    fn the_log_drains_once() {
        let log = ReceiptLog::new();
        log.push(
            "reload-runtime",
            Outcome::Refused(Refusal::NotYetImplemented { task: "T094" }),
        );
        assert_eq!(log.take().len(), 1);
        assert!(log.take().is_empty(), "a drain empties the log");
    }
}
