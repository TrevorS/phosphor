//! The editor layer as one object: a VM with the vocabulary installed and the
//! boot already run.
//!
//! [`Runtime::boot`] is **infallible by signature**. That is not defensive
//! programming; it is `T021`'s acceptance criterion expressed in a type — *a
//! syntax error in `init.scm` yields a working editor with a legible error
//! float.* If this returned a `Result`, the first caller to write `?` would
//! delete that property, and no test would notice until someone broke their own
//! `init.scm`.
//!
//! # What the host does with one
//!
//! The wiring lands with the loop (`T026`), and this is the seam it takes:
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use phosphor_steel::host::{Detached, Host};
//! # use phosphor_steel::runtime::Runtime;
//! let host: Arc<dyn Host> = Arc::new(Detached); // the store, once there is one
//! let runtime = Runtime::boot(Runtime::root().as_deref(), host);
//! if let Some(float) = runtime.boot_float() {
//!     // hand it to the one float slot — `phosphor-ui`'s `FloatSlot` (`T084`)
//!     // by way of the view tree (`T079`).
//!     let _ = float;
//! }
//! ```
//!
//! Owned by `spine`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_core::action::{Outcome, Receipt, Refusal};
use phosphor_core::value::Value;
use phosphor_core::view::Float;
use steel::steel_vm::engine::Engine;
use steel::{SteelErr, SteelVal};

use crate::boot::{BootReport, INIT, boot};
use crate::convert::from_steel;
use crate::float::boot_float;
use crate::host::{Host, Logged, ReceiptLog};
use crate::registry::install;
use crate::view::install as install_view;

/// The environment variable that names the runtime tree outright.
pub const RUNTIME_ENV: &str = "PHOSPHOR_RUNTIME";

/// The Steel VM, the vocabulary, and what the boot made of `runtime/`.
pub struct Runtime {
    engine: Engine,
    receipts: ReceiptLog,
    report: BootReport,
}

impl core::fmt::Debug for Runtime {
    /// `Engine` has no `Debug`, and a VM's contents are not what a reader of a
    /// log wants anyway — the boot's findings are.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Runtime")
            .field("root", &self.report.root)
            .field("units", &self.report.units.len())
            .field("faults", &self.report.faults.len())
            .finish_non_exhaustive()
    }
}

impl Runtime {
    /// Builds a VM, installs every capability, and runs the boot sequence.
    ///
    /// `root` is the runtime tree. [`None`] — or a directory with no
    /// `init.scm` — boots an editor with the vocabulary installed and nothing
    /// loaded, which is the correct state for a fresh install and is not a
    /// fault.
    #[must_use]
    pub fn boot(root: Option<&Path>, host: Arc<dyn Host>) -> Self {
        let mut engine = Engine::new();
        let receipts = ReceiptLog::new();
        install(&mut engine, &host, &receipts);
        // What Steel may *say*, beside what it may do: one constructor per node
        // kind, generated from the protocol (Q12, `crate::view`). Installed
        // before the boot, because `init.scm`'s own load order may name a file
        // that composes at load time.
        install_view(&mut engine);

        let report = root.map_or_else(BootReport::default, |root| boot(&mut engine, root));

        Self {
            engine,
            receipts,
            report,
        }
    }

    /// Where the editor layer lives, or [`None`] if nothing looks like it.
    ///
    /// Three places, in order, each with a reason:
    ///
    /// 1. **`$PHOSPHOR_RUNTIME`** — the override. A test, a packager and
    ///    `V006`'s reproducible seeding all need one, and an explicit
    ///    environment variable beats three heuristics.
    /// 2. **`$XDG_CONFIG_HOME/phosphor`** (or `~/.config/phosphor`) — the
    ///    user's own layer. `persist-form` (`6b`'s *"persisted to init.scm"*)
    ///    and `7a`'s always-allow rule both write here.
    /// 3. **`./runtime`** — the checkout, so `cargo run` inside the repo boots
    ///    the editor layer you are editing rather than the one you installed.
    ///
    /// Each candidate has to actually contain an `init.scm`, except the
    /// override, which is taken at its word so that pointing at an empty
    /// directory is a way to boot with nothing loaded.
    #[must_use]
    pub fn root() -> Option<PathBuf> {
        if let Some(named) = std::env::var_os(RUNTIME_ENV) {
            return Some(PathBuf::from(named));
        }

        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|config| config.join("phosphor"));

        [config, Some(PathBuf::from("runtime"))]
            .into_iter()
            .flatten()
            .find(|candidate| candidate.join(INIT).is_file())
    }

    /// What the boot did, and what it could not do.
    #[must_use]
    pub const fn report(&self) -> &BootReport {
        &self.report
    }

    /// The float describing the boot's faults, or [`None`] if it ran clean.
    #[must_use]
    pub fn boot_float(&self) -> Option<Float> {
        boot_float(&self.report)
    }

    /// Evaluates source in the live VM.
    ///
    /// The one entry point `T022`'s REPL and `T023`'s `--eval` both take, so
    /// *"`--eval` and the REPL return identical results for the same
    /// expression"* is true because there is one path, not because two are kept
    /// in step.
    ///
    /// Whole-source, not form-by-form: an expression a person typed is one
    /// unit, and a parse error in it belongs in their face rather than in a
    /// report. The boot's per-form isolation ([`crate::boot`]) is for files
    /// nobody is watching run.
    ///
    /// # Errors
    ///
    /// Whatever Steel says, unchanged. The REPL prints it; nothing here
    /// interprets it.
    pub fn eval(&mut self, source: &str) -> Result<Vec<SteelVal>, SteelErr> {
        self.engine.compile_and_run_raw_program(source.to_owned())
    }

    /// Evaluates source and answers an [`Outcome`] — the shape both front-ends
    /// of `Action::Runtime(Eval)` want.
    ///
    /// `crates/phosphor/src/door.rs` declares an `Evaluate` trait with exactly
    /// this signature and passes `None` for it until `T021` lands, so wiring
    /// `--eval` (`T023`) and the REPL (`T022`) to a real VM is one adapter over
    /// this method. It lives here rather than in the binary because the
    /// `SteelVal` → [`Value`] narrowing is this crate's job and nothing above it
    /// should have to know a `SteelVal` exists.
    ///
    /// The value is the last expression's; the note is the last Action's, which
    /// is what draws `6b`'s `⇒ #ok · persisted to init.scm`
    /// (TUI Mockups.dc.html:499) — the two halves of that line come from two
    /// places, and this is where they meet.
    ///
    /// **Flagged seam:** [`Outcome`] has no case for *"it ran and raised"* —
    /// `Done` or `Refused`, and a Steel error is neither. It comes back as
    /// [`Refusal::Declined`] carrying Steel's own text, which reads correctly
    /// after `⇒` but is not what `Declined` means. Adding an arm is an
    /// `action.rs` edit, so it is reported rather than made here.
    #[must_use]
    pub fn evaluate(&mut self, source: &str) -> Outcome {
        let result = self.eval(source);
        let note = self
            .take_receipts()
            .into_iter()
            .rev()
            .find_map(|logged| match logged.outcome {
                Outcome::Done(receipt) => receipt.note,
                Outcome::Refused(_) => None,
            });

        match result {
            Ok(values) => Outcome::Done(Receipt {
                capability: "eval",
                value: values.last().map_or(Value::Null, from_steel_or_opaque),
                note,
            }),
            Err(error) => Outcome::Refused(Refusal::Declined {
                reason: error.to_string(),
            }),
        }
    }

    /// The outcomes of every Action applied since this was last called.
    ///
    /// `6b`'s `⇒ #ok · persisted to init.scm` — the value comes back from
    /// [`Runtime::eval`], the note comes from here.
    #[must_use]
    pub fn take_receipts(&self) -> Vec<Logged> {
        self.receipts.take()
    }

    /// Reads a global out of the VM.
    ///
    /// # Errors
    ///
    /// Steel's own error when nothing is bound to that name.
    pub fn global(&self, name: &str) -> Result<SteelVal, SteelErr> {
        self.engine.extract_value(name)
    }

    /// Calls a procedure the editor layer defined, with values Rust already
    /// holds.
    ///
    /// The read side of composition (`T025`): a ViewModel goes in as data and a
    /// view tree comes back. **Not `eval` with the arguments printed into the
    /// source** — a ViewModel carries a path, and a path carries whatever a
    /// filesystem allows; rendering one into scheme source would make a file
    /// name able to change the form being evaluated.
    ///
    /// # Errors
    ///
    /// Steel's own error: nothing is bound to that name, it is not callable, or
    /// the call raised.
    pub fn call(&mut self, name: &str, args: Vec<SteelVal>) -> Result<SteelVal, SteelErr> {
        self.engine.call_function_by_name_with_args(name, args)
    }
}

/// A result value on the wire, or its own printed form when it has no wire case.
///
/// A `(lambda () …)` is a perfectly good thing for an expression to answer at
/// the REPL and a perfectly bad thing to put in an `Outcome` payload. Steel's
/// own display of it is what `6b` would print after `⇒`, so that is what
/// crosses — as text, which is honest about what it is.
fn from_steel_or_opaque(value: &SteelVal) -> Value {
    from_steel(value).unwrap_or_else(|_| Value::Text(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Detached;

    fn detached(root: Option<&Path>) -> Runtime {
        Runtime::boot(root, Arc::new(Detached))
    }

    #[test]
    fn a_runtime_with_no_root_still_has_the_whole_vocabulary() {
        let mut runtime = detached(None);
        assert!(runtime.report().is_clean());
        assert!(runtime.boot_float().is_none());
        let out = runtime
            .eval("(reload-runtime!)")
            .expect("every capability is installed with or without a boot file");
        assert!(!out.is_empty());
    }

    #[test]
    fn the_shipped_runtime_tree_boots_clean() {
        // `runtime/init.scm` is the editor layer this repo ships. If it stops
        // booting cleanly, the float is right and this test is the alarm.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime");
        let runtime = detached(Some(&root));
        assert!(
            runtime.report().is_clean(),
            "runtime/init.scm does not boot: {:?}",
            runtime.report().faults
        );
        assert!(
            runtime.report().forms_ran() > 0,
            "runtime/init.scm ran no forms at all"
        );
    }

    #[test]
    fn eval_is_the_same_path_the_repl_and_the_cli_will_take() {
        let mut runtime = detached(None);
        let out = runtime.eval("(+ 1 2)").expect("arithmetic");
        assert_eq!(out.last(), Some(&SteelVal::IntV(3)));
    }

    #[test]
    fn evaluate_answers_the_outcome_the_cli_door_wants() {
        let mut runtime = detached(None);
        let Outcome::Done(receipt) = runtime.evaluate("(+ 1 2)") else {
            panic!("an expression that evaluated is not a refusal");
        };
        assert_eq!(receipt.capability, "eval");
        assert_eq!(receipt.value, Value::Int(3));
    }

    #[test]
    fn evaluate_reports_a_steel_error_rather_than_panicking() {
        let mut runtime = detached(None);
        let Outcome::Refused(Refusal::Declined { reason }) = runtime.evaluate("(") else {
            panic!("a broken expression comes back as a refusal carrying steel's text");
        };
        assert!(!reason.is_empty());
    }

    #[test]
    fn evaluate_answers_an_uncrossable_value_as_its_printed_form() {
        let mut runtime = detached(None);
        let Outcome::Done(receipt) = runtime.evaluate("(lambda () 1)") else {
            panic!("a lambda is a fine thing to answer at the repl");
        };
        assert!(matches!(receipt.value, Value::Text(_)), "{receipt:?}");
    }

    #[test]
    fn receipts_carry_the_note_the_value_cannot() {
        let mut runtime = detached(None);
        runtime
            .eval("(close-float!)")
            .expect("a refusal is a value");
        let receipts = runtime.take_receipts();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].capability, "close-float");
        assert!(runtime.take_receipts().is_empty());
    }
}
