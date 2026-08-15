//! The boot sequence — and the promise that a broken one still leaves an editor.
//!
//! > *"Surface changes never require restart — `init.scm` is just the REPL
//! > session that runs at boot … and a broken `init.scm` boots the editor anyway
//! > with the error in a float."* — Component Breakdown, *philosophy*
//!
//! [`boot`] is total. It returns a [`BootReport`], never a `Result`: there is no
//! failure mode here that is allowed to stop an editor from existing. A missing
//! runtime directory, an unreadable file, a stray paren and a free identifier
//! are all *findings*, and findings go in a float ([`crate::float`]).
//!
//! # The two granularities, and why both are needed
//!
//! **Per form.** A file is scanned into top-level forms ([`crate::source`]) and
//! each is compiled and run alone. One bad form is one fault; the forms around
//! it still run. Without this, a stray paren on the last line of `init.scm`
//! would discard the whole file including the load order it declares — the
//! silent discard that costs the most and looks the least like a bug.
//!
//! **Per file.** Each file in the load order is loaded independently, so a
//! broken `keymaps.scm` costs you your keymaps and nothing else.
//!
//! There is one limit worth stating rather than discovering: an unclosed `(`
//! is not a broken form, it is a form that has not finished, so every form
//! after it is *inside* it and goes down with it. No scanner can recover
//! those. What it does recover is everything above the mistake, plus the line
//! the unfinished form opened on — which is the line to go and look at.
//!
//! # Where the load order lives
//!
//! In `init.scm`, as data: `(define phosphor/boot-files '("keymaps.scm" …))`.
//! Rust reads that global *after* the last form of `init.scm` has run, then
//! loads each name in turn.
//!
//! It is data rather than a `(load …)` call for one reason worth stating:
//! Steel calls into Rust from inside a running VM, and a binding that loaded a
//! file would be re-entering the engine that is currently executing it. Reading
//! a declared list afterwards gets the same redefinability with no re-entrancy —
//! and it survives a broken form, since a `define` that ran is a `define` Rust
//! can read even if the form after it failed.
//!
//! Owned by `spine`.

use std::fs;
use std::path::{Component, Path, PathBuf};

use phosphor_core::request::Position;
use steel::rerrs::ErrorKind;
use steel::steel_vm::engine::Engine;
use steel::{SteelErr, SteelVal};

use crate::source::{Form, line_and_column, nth_line, top_level_forms};

/// The boot file. Everything else in the load order is named by this one.
pub const INIT: &str = "init.scm";

/// The global `init.scm` declares the load order in.
///
/// Namespaced because it is a name Rust reaches into the VM for; an
/// un-prefixed `boot-files` would be one `(define boot-files …)` in a user's
/// own code away from silently becoming the boot order.
pub const BOOT_FILES: &str = "phosphor/boot-files";

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// One thing that went wrong during boot.
///
/// Carries enough to act on without opening anything else: the file, the
/// position in it, what kind of wrong it is, what Steel said, and the offending
/// line itself. `T021`'s acceptance criterion is a *legible* error float, and a
/// message with no line number is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootFault {
    /// The file, named the way the load order names it — relative to the
    /// runtime root.
    pub file: PathBuf,
    /// Where in the file, when there is a where. Absent for a fault about the
    /// file as a whole (unreadable, or a load order that is not a list).
    pub at: Option<Position>,
    /// What kind of wrong, in two or three lowercase words: `bad syntax`,
    /// `free identifier`, `unreadable`.
    pub label: &'static str,
    /// What Steel said, or what we say when Steel had no part in it.
    pub message: String,
    /// The source line at [`BootFault::at`], for the float to show underneath.
    pub source_line: Option<String>,
}

impl BootFault {
    /// `init.scm:12:3` — the position as every compiler on earth writes it.
    #[must_use]
    pub fn place(&self) -> String {
        let file = self.file.display();
        self.at.map_or_else(
            || file.to_string(),
            |at| format!("{file}:{}:{}", at.line, at.column),
        )
    }
}

/// What one file contributed to the boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootUnit {
    /// The file, relative to the runtime root.
    pub file: PathBuf,
    /// How many top-level forms it holds.
    pub forms: usize,
    /// How many of them ran without a fault.
    pub ran: usize,
}

/// What the boot did, and what it could not do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BootReport {
    /// The runtime root the boot read from, if it found one.
    pub root: Option<PathBuf>,
    /// Each file the boot loaded, in load order.
    pub units: Vec<BootUnit>,
    /// Everything that went wrong, in the order it went wrong.
    pub faults: Vec<BootFault>,
}

impl BootReport {
    /// Whether the whole boot ran clean.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.faults.is_empty()
    }

    /// How many top-level forms ran.
    #[must_use]
    pub fn forms_ran(&self) -> usize {
        self.units.iter().map(|unit| unit.ran).sum()
    }

    /// How many top-level forms the boot found.
    #[must_use]
    pub fn forms_found(&self) -> usize {
        self.units.iter().map(|unit| unit.forms).sum()
    }
}

// ---------------------------------------------------------------------------
// The sequence
// ---------------------------------------------------------------------------

/// Runs the boot sequence in `engine` against the runtime tree at `root`.
///
/// Total. A runtime root with no [`INIT`] in it is a fresh install, not a
/// fault — *"cold start invites, never nags"* (TUI Mockups, turn 7).
#[must_use]
pub fn boot(engine: &mut Engine, root: &Path) -> BootReport {
    let mut report = BootReport {
        root: Some(root.to_path_buf()),
        ..BootReport::default()
    };

    if !root.join(INIT).is_file() {
        return report;
    }
    load(engine, root, Path::new(INIT), &mut report);

    for file in load_order(engine, &mut report) {
        load(engine, root, &file, &mut report);
    }

    report
}

/// The load order `init.scm` declared, validated.
fn load_order(engine: &Engine, report: &mut BootReport) -> Vec<PathBuf> {
    // Not declared at all is not a fault: `init.scm` may be a file of defaults
    // and nothing else, or the form that declared it may already be in
    // `report.faults`, where saying so twice would help nobody.
    let Ok(value) = engine.extract_value(BOOT_FILES) else {
        return Vec::new();
    };
    let SteelVal::ListV(names) = value else {
        report.faults.push(whole_file_fault(
            INIT,
            "bad boot order",
            format!("{BOOT_FILES} must be a list of file names"),
        ));
        return Vec::new();
    };

    let mut order = Vec::new();
    for name in &names {
        let SteelVal::StringV(name) = name else {
            report.faults.push(whole_file_fault(
                INIT,
                "bad boot order",
                format!("{BOOT_FILES} holds something that is not a file name"),
            ));
            continue;
        };
        let path = PathBuf::from(name.to_string());
        if is_confined(&path) {
            order.push(path);
        } else {
            report.faults.push(whole_file_fault(
                INIT,
                "bad boot order",
                format!("`{}` leaves the runtime directory", path.display()),
            ));
        }
    }
    order
}

/// Whether a load-order entry stays inside the runtime root.
///
/// The editor layer is a tree, not a path into the filesystem. An absolute
/// path or a `..` in the load order is not a capability anyone asked for, and
/// refusing it here costs one function.
fn is_confined(path: &Path) -> bool {
    path.components()
        .all(|part| matches!(part, Component::Normal(_)))
}

/// Loads one file, form by form, recording what ran and what did not.
fn load(engine: &mut Engine, root: &Path, file: &Path, report: &mut BootReport) {
    let source = match fs::read_to_string(root.join(file)) {
        Ok(source) => source,
        Err(error) => {
            report
                .faults
                .push(whole_file_fault(file, "unreadable", error.to_string()));
            report.units.push(BootUnit {
                file: file.to_path_buf(),
                forms: 0,
                ran: 0,
            });
            return;
        }
    };

    let (forms, unterminated) = top_level_forms(&source);
    let mut ran = 0;
    for form in &forms {
        match engine.compile_and_run_raw_program(form.text(&source).to_owned()) {
            Ok(_) => ran += 1,
            Err(error) => report
                .faults
                .push(steel_fault(file, &source, *form, &error)),
        }
    }

    if let Some(open) = unterminated {
        let (line, column) = line_and_column(&source, open.start);
        report.faults.push(BootFault {
            file: file.to_path_buf(),
            at: Some(Position { line, column }),
            label: "unterminated",
            message: format!("this {} is never closed", open.what),
            source_line: nth_line(&source, line).map(str::to_owned),
        });
    }

    report.units.push(BootUnit {
        file: file.to_path_buf(),
        forms: forms.len(),
        ran,
    });
}

/// A Steel error, placed in the file rather than in the form.
///
/// A form is compiled on its own, so the span Steel reports is an offset into
/// *that form*. Adding the form's own offset is what turns it back into a
/// position a person can jump to.
///
/// **Public because the persisted layer is loaded outside this crate.** `T101`
/// moved `persisted.scm` into the config home, so `crates/phosphor/src/main.rs`
/// runs it form by form itself — and its own copy of these six lines kept
/// Steel's CamelCase kind in the message that [`message`] strips, which put two
/// voices in one boot float. One constructor is what stops that recurring.
#[must_use]
pub fn steel_fault(file: &Path, source: &str, form: Form, error: &SteelErr) -> BootFault {
    let offset = error
        .span()
        .map_or(form.start, |span| form.start + span.start() as usize);
    let (line, column) = line_and_column(source, offset.min(source.len()));

    BootFault {
        file: file.to_path_buf(),
        at: Some(Position { line, column }),
        label: label(error.kind()),
        message: message(error),
        source_line: nth_line(source, line).map(str::to_owned),
    }
}

/// A fault about a file rather than about a place in it.
fn whole_file_fault(file: impl AsRef<Path>, label: &'static str, message: String) -> BootFault {
    BootFault {
        file: file.as_ref().to_path_buf(),
        at: None,
        label,
        message,
        source_line: None,
    }
}

/// Steel's error kind, spelled the way the editor talks (Design Language §6:
/// lowercase, telegraphic).
const fn label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::ArityMismatch => "wrong arity",
        ErrorKind::FreeIdentifier => "free identifier",
        ErrorKind::TypeMismatch => "wrong type",
        ErrorKind::UnexpectedToken => "unexpected token",
        ErrorKind::ContractViolation => "contract violation",
        ErrorKind::BadSyntax => "bad syntax",
        ErrorKind::ConversionError => "conversion",
        ErrorKind::Io => "io",
        ErrorKind::Parse => "parse",
        ErrorKind::Infallible => "impossible",
        ErrorKind::Generic => "error",
    }
}

/// What Steel said, with its own prefixes taken off.
///
/// `SteelErr`'s `Display` is `Error: <Kind>: <message>`
/// (`steel-core-0.8.2/src/rerrs.rs:33-36`) and exposes no accessor for the
/// message alone. The kind is already [`label`]'s job, and repeating it in
/// CamelCase inside the float would be the wrong voice twice.
///
/// The leading capital comes off too (Design Language §6: *"lowercase,
/// telegraphic"*). That is the whole edit — the words are Steel's. Rewriting
/// its diagnostics into our own would mean maintaining a translation table
/// against a pre-1.0 crate, and a message that does not match what the same
/// error prints at the REPL is worse than a capital letter.
fn message(error: &SteelErr) -> String {
    let text = error.to_string();
    let text = text.strip_prefix("Error: ").unwrap_or(&text);
    let kind = format!("{:?}: ", error.kind());
    let text = text.strip_prefix(&kind).unwrap_or(text).trim();

    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::host::{Detached, Host, ReceiptLog};
    use crate::registry::install;

    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "phosphor-boot-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("a temp runtime tree");
            Self(root)
        }

        fn write(&self, name: &str, contents: &str) -> &Self {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("a temp runtime subdirectory");
            }
            fs::write(path, contents).expect("a temp runtime file");
            self
        }

        fn boot(&self) -> (Engine, BootReport) {
            let mut engine = Engine::new();
            let host: Arc<dyn Host> = Arc::new(Detached);
            install(&mut engine, &host, &ReceiptLog::new());
            let report = boot(&mut engine, &self.0);
            (engine, report)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_runtime_root_is_not_a_fault() {
        let mut engine = Engine::new();
        let report = boot(&mut engine, Path::new("/nowhere/phosphor-does-not-exist"));
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(report.forms_found(), 0);
    }

    #[test]
    fn a_clean_init_runs_every_form() {
        let tree = Tree::new("clean");
        tree.write(INIT, "(define a 1)\n(define b 2)\n");
        let (engine, report) = tree.boot();
        assert!(report.is_clean(), "{report:?}");
        assert_eq!((report.forms_found(), report.forms_ran()), (2, 2));
        assert_eq!(engine.extract_value("b").ok(), Some(SteelVal::IntV(2)));
    }

    #[test]
    fn a_broken_form_costs_that_form_and_nothing_else() {
        // The load-bearing half of `T021`, at the granularity that matters.
        let tree = Tree::new("broken-form");
        tree.write(
            INIT,
            "(define before 1)\n(no-such-procedure 1 2)\n(define after 3)\n",
        );
        let (engine, report) = tree.boot();

        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.forms_found(), 3);
        assert_eq!(report.forms_ran(), 2);
        assert_eq!(
            engine.extract_value("before").ok(),
            Some(SteelVal::IntV(1)),
            "the form above the mistake ran"
        );
        assert_eq!(
            engine.extract_value("after").ok(),
            Some(SteelVal::IntV(3)),
            "the form below the mistake ran"
        );
    }

    #[test]
    fn a_missing_closer_swallows_what_follows_and_says_where_it_started() {
        // The honest limit of per-form isolation: an unclosed `(` is not a
        // broken form, it is a form that has not finished, so everything after
        // it is inside it. Nothing can recover the swallowed forms — what can
        // be recovered is the forms *above* it, and the line to go and look at.
        let tree = Tree::new("unclosed");
        tree.write(INIT, "(define before 1)\n(define oops\n(define after 3)\n");
        let (engine, report) = tree.boot();

        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.faults[0].label, "unterminated");
        assert_eq!(
            report.faults[0].at.expect("a position").line,
            2,
            "named at the line the form opened on"
        );
        assert_eq!(
            engine.extract_value("before").ok(),
            Some(SteelVal::IntV(1)),
            "the forms above it still ran"
        );
        assert!(engine.extract_value("after").is_err());
    }

    #[test]
    fn a_fault_names_the_file_the_line_and_what_was_wrong() {
        let tree = Tree::new("legible");
        tree.write(INIT, "(define a 1)\n(define b (+ 1 nonesuch))\n");
        let (_, report) = tree.boot();

        let fault = report.faults.first().expect("one fault");
        assert_eq!(fault.file, Path::new(INIT));
        let at = fault.at.expect("a fault inside a form has a position");
        assert_eq!(at.line, 2, "{fault:?}");
        assert!(fault.place().starts_with("init.scm:2:"), "{fault:?}");
        assert!(!fault.message.is_empty(), "{fault:?}");
        assert!(!fault.message.starts_with("Error:"), "{fault:?}");
        assert_eq!(
            fault.source_line.as_deref(),
            Some("(define b (+ 1 nonesuch))")
        );
    }

    #[test]
    fn the_load_order_is_read_from_init_and_followed() {
        let tree = Tree::new("order");
        tree.write(INIT, "(define phosphor/boot-files '(\"keymaps.scm\"))")
            .write("keymaps.scm", "(define bound 1)");
        let (engine, report) = tree.boot();

        assert!(report.is_clean(), "{report:?}");
        assert_eq!(
            report
                .units
                .iter()
                .map(|unit| unit.file.display().to_string())
                .collect::<Vec<_>>(),
            [INIT, "keymaps.scm"]
        );
        assert_eq!(engine.extract_value("bound").ok(), Some(SteelVal::IntV(1)));
    }

    #[test]
    fn a_broken_file_in_the_load_order_does_not_stop_the_next_one() {
        let tree = Tree::new("order-broken");
        tree.write(INIT, "(define phosphor/boot-files '(\"a.scm\" \"b.scm\"))")
            .write("a.scm", "(define broken")
            .write("b.scm", "(define fine 1)");
        let (engine, report) = tree.boot();

        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.faults[0].file, Path::new("a.scm"));
        assert_eq!(engine.extract_value("fine").ok(), Some(SteelVal::IntV(1)));
    }

    #[test]
    fn a_load_order_that_survived_a_broken_form_is_still_read() {
        // The reason the order is data read afterwards rather than a call: a
        // form that ran is readable even when the form beside it did not.
        let tree = Tree::new("order-survives");
        tree.write(INIT, "(define phosphor/boot-files '(\"a.scm\"))\n(oops\n")
            .write("a.scm", "(define loaded 1)");
        let (engine, report) = tree.boot();

        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(engine.extract_value("loaded").ok(), Some(SteelVal::IntV(1)));
    }

    #[test]
    fn a_load_order_entry_may_not_leave_the_runtime_tree() {
        let tree = Tree::new("escape");
        tree.write(INIT, "(define phosphor/boot-files '(\"../etc/passwd\"))");
        let (_, report) = tree.boot();
        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.faults[0].label, "bad boot order");
    }

    #[test]
    fn a_missing_file_in_the_load_order_is_a_fault() {
        let tree = Tree::new("missing");
        tree.write(INIT, "(define phosphor/boot-files '(\"gone.scm\"))");
        let (_, report) = tree.boot();
        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.faults[0].label, "unreadable");
    }

    #[test]
    fn a_load_order_that_is_not_a_list_is_a_fault_rather_than_a_panic() {
        let tree = Tree::new("not-a-list");
        tree.write(INIT, "(define phosphor/boot-files 3)");
        let (_, report) = tree.boot();
        assert_eq!(report.faults.len(), 1, "{report:?}");
        assert_eq!(report.faults[0].label, "bad boot order");
        assert!(report.faults[0].at.is_none());
    }
}
