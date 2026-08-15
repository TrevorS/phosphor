//! Tooling pass — the editor layer we **ship** boots clean.
//!
//! `T021`'s guarantee is that a broken `init.scm` boots the editor anyway, with
//! the fault in a float and the forms around it still running. That is a
//! promise about *your* layer, and it is exactly why a syntax error in *ours*
//! is invisible: the editor swallows it by design, draws a float most people
//! will `esc` past, and carries on. `just test` never opened the files, and
//! `broken_init.rs` only ever tested a fault planted on purpose.
//!
//! So this asserts the other half — that the shipped tree has no faults at all.
//! It is the difference between "the editor survives a bad layer" and "the
//! layer we hand people is good", and only the first had a test.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::source;

/// The `runtime/` directory as shipped, from this crate's manifest.
fn runtime_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// Every `.scm` we ship, sorted, so a failure names a stable file.
///
/// One level of subdirectory, because the tree grew one: `T037` puts the twelve
/// language declarations in `runtime/languages/`, and `pickers/` is next
/// (`T045`). A flat `read_dir` here would have gone on passing while twelve
/// unscanned files shipped — which is the failure this whole file exists
/// against.
fn shipped() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut directories = vec![runtime_dir()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory)
            .expect("runtime/ is part of the repo")
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.is_dir() && directory == runtime_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|ext| ext == "scm") {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(
        !files.is_empty(),
        "runtime/ has no .scm files — wrong path?"
    );
    files
}

/// A shipped file's name **as the load order spells it** — `keymaps.scm`,
/// `languages/rust.scm`.
fn as_listed(path: &Path) -> String {
    path.strip_prefix(runtime_dir())
        .expect("every shipped file is under runtime/")
        .to_str()
        .expect("a utf-8 path")
        .replace('\\', "/")
}

/// Every shipped file scans: no form left open at end of file.
///
/// The cheap half, and the one that catches a stray paren in a commit. Runs
/// without a VM, so it fails with a file and a line rather than a boot report.
#[test]
fn every_shipped_scm_file_scans() {
    for path in shipped() {
        let text = std::fs::read_to_string(&path).expect("a readable .scm file");
        let (forms, unterminated) = source::top_level_forms(&text);

        if let Some(open) = unterminated {
            let (line, column) = source::line_and_column(&text, open.start);
            panic!(
                "{}:{line}:{column} — a top-level form is never closed.\n\
                 The editor would boot anyway and put this in a float, which is \
                 exactly why it needs a test: nobody reads the float.",
                path.display()
            );
        }

        // `persisted.scm` ships empty on purpose: it is what the REPL appends
        // to, and a shipped default in it would be a decision made for you by
        // nobody. Every other file having no forms means somebody wrote a
        // header and stopped.
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert!(
            !forms.is_empty() || name == "persisted.scm",
            "{} has no top-level forms — a file in the load order that does \
             nothing is one somebody meant to finish",
            path.display()
        );
    }
}

/// The layer boots with **no faults**, which is stronger than scanning.
///
/// A file can scan perfectly and still fail to evaluate — a free identifier, a
/// call with the wrong arity, a name defined after the form that uses it. Only
/// running the boot finds those, and `Runtime::report()` is where `T021` puts
/// them.
#[test]
fn the_shipped_layer_boots_without_a_single_fault() {
    let host: Arc<dyn Host> = Arc::new(Detached);
    let runtime = Runtime::boot(Some(&runtime_dir()), host);
    let report = runtime.report();

    assert!(
        report.is_clean(),
        "the shipped editor layer does not boot clean: {:#?}\n\
         Every one of these would reach a user as a boot float.",
        report.faults
    );
}

/// Every file the load order names exists, and every file present is loaded.
///
/// `init.scm` declares `phosphor/boot-files` as data. A name in it that is not
/// on disk is left out rather than faulting — deliberate, so a boot float does
/// not appear on every start — which means a typo there is silent, and the file
/// it meant to load simply never runs. The reverse is as bad: a `.scm` sitting
/// in `runtime/` that no load order names is dead code that reads as live.
#[test]
fn the_load_order_and_the_directory_agree() {
    let init = std::fs::read_to_string(runtime_dir().join("init.scm")).expect("init.scm ships");

    let listed: Vec<String> = init
        .split_once("phosphor/boot-files")
        .and_then(|(_, rest)| rest.split_once('(').map(|(_, r)| r))
        .and_then(|rest| rest.split_once(')').map(|(inner, _)| inner))
        .map(|inner| {
            inner
                .split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect()
        })
        .expect("init.scm declares phosphor/boot-files as a quoted list of names");

    assert!(!listed.is_empty(), "the load order is empty");

    for name in &listed {
        assert!(
            runtime_dir().join(name).is_file(),
            "the load order names `{name}`, which is not in runtime/. \
             A missing name is skipped silently at boot, so the file never runs \
             and nothing says so."
        );
    }

    for path in shipped() {
        let name = as_listed(&path);
        if name == "init.scm" {
            continue;
        }
        assert!(
            listed.contains(&name),
            "runtime/{name} is not in init.scm's load order, so it never runs. \
             Either add it, or delete it — a file that looks live and is not is \
             worse than no file."
        );
    }
}
