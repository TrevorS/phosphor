//! `T023` — the CLI door, run as a process.
//!
//! `door.rs`'s unit tests prove the two front-ends assemble one call. These
//! prove the two things that are only true of a *program*: that the door needs
//! no terminal, and that the same expression through either front-end produces
//! the same bytes on stdout and the same exit status.
//!
//! Why a process test rather than another unit test. `V006` seeds tape fixtures
//! by shelling out to `phosphor --eval`, with **no test-only backdoor**
//! (`TASKS.md`), and a tape runs with stdout on a pipe inside `ttyd`. An
//! in-process test cannot catch the failure that would actually bite there —
//! entering the alternate screen, or taking raw mode, before printing — because
//! the thing it would catch is a side effect on a terminal the test does not
//! have. Running the binary with stdout on a file does catch it: the escape
//! sequences land in the file.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// The expression both front-ends are asked for.
///
/// Deliberately one the editor cannot answer yet — `unseen-regions` is a query
/// and the store is `T041` — because what is under test is that the two doors
/// agree, and they have to agree about a refusal exactly as much as about a
/// result. `6b` types this very line.
const EXPR: &str = "(unseen-regions \"src/retry.rs\")";

fn phosphor() -> Command {
    Command::new(env!("CARGO_BIN_EXE_phosphor"))
}

fn run(args: &[&str]) -> Output {
    phosphor()
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs")
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("phosphor-door-{name}-{}", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

#[test]
fn the_door_prints_without_a_terminal() {
    // stdout is a file, stdin is closed, and nothing here is a tty. If the door
    // ever entered the alternate screen or negotiated the kitty protocol before
    // printing, the escape sequences would be in this file.
    let out = scratch("redirect");
    let err = scratch("redirect-err");
    let status = phosphor()
        .args(["--eval", EXPR])
        .stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(&out).expect("create")))
        .stderr(Stdio::from(fs::File::create(&err).expect("create")))
        .status()
        .expect("the binary runs");

    let printed = fs::read_to_string(&out).expect("read");
    let diagnostics = fs::read_to_string(&err).expect("read");
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(&err);

    assert!(diagnostics.is_empty(), "unexpected stderr: {diagnostics:?}");

    assert!(!printed.is_empty(), "the door printed nothing");
    assert!(
        !printed.contains('\u{1b}'),
        "the door wrote an escape sequence to a redirected stdout: {printed:?}"
    );
    assert!(printed.ends_with('\n'), "one line, newline-terminated");
    // A refusal is not an error, but it is not a success either — `V006` seeds
    // state through this door and has to be able to tell.
    assert!(!status.success());
}

#[test]
fn the_flag_and_the_verb_answer_identically() {
    // `T023`'s acceptance criterion, as a program: two front-ends, one
    // evaluation path. Bytes, not "equivalent".
    let sugar = run(&["--eval", EXPR]);
    let verb = run(&["eval", "--source", EXPR]);

    assert_eq!(
        String::from_utf8_lossy(&sugar.stdout),
        String::from_utf8_lossy(&verb.stdout)
    );
    assert_eq!(sugar.status.code(), verb.status.code());
    assert!(sugar.stderr.is_empty(), "a refusal is not a diagnostic");
}

#[test]
fn a_refusal_names_the_task_that_builds_it() {
    // Naming the task is what `Refusal::NotYetImplemented` is for — the caller
    // learns what to wait for instead of getting "unknown action".
    //
    // **`T022` wired the VM in**, so this is no longer the door's own refusal
    // for want of a runtime: the source reaches Steel, the binding reaches the
    // host, and the host refuses the *query* by naming `T041`. That the task id
    // survives the round trip is the whole point.
    let out = run(&["--eval", EXPR]);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        printed.starts_with("#refused · "),
        "unexpected answer: {printed:?}"
    );
    assert!(
        printed.contains("T041"),
        "the refusal lost the task that builds it: {printed:?}"
    );
}

#[test]
fn a_generated_verb_is_reachable_end_to_end() {
    // `T024`'s CLI third: the verb parses, assembles a call, decodes into the
    // Action, and answers. Every flag here came out of the registry row.
    let out = run(&["mark-seen", "--target", "region", "--target.region.id", "3"]);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        printed.starts_with("#refused · T041 "),
        "unexpected answer: {printed:?}"
    );
}

#[test]
fn a_malformed_call_is_a_diagnostic_and_prints_no_result() {
    // A malformed call is not a refusal: nothing goes to stdout, so a shell
    // pipeline never mistakes an error for an answer.
    let out = run(&["mark-seen", "--target", "region"]);
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "stdout carried a result for a call that never ran"
    );
    assert!(!out.stderr.is_empty(), "the error went nowhere");
}

#[test]
fn the_host_still_needs_a_file_and_the_door_does_not() {
    let bare = run(&[]);
    assert!(!bare.status.success(), "no file and no expression is usage");

    let file = scratch("host").with_extension("rs");
    fs::write(&file, "fn main() {}\n").expect("write");
    // Not run — opening it would take the terminal. What is under test is that
    // the *parser* accepts the host's line unchanged now that 208 subcommands
    // sit beside it, which `--help` exercises without drawing a frame.
    let help = run(&["--help"]);
    let _ = fs::remove_file(&file);
    assert!(help.status.success());
    let printed = String::from_utf8_lossy(&help.stdout);
    assert!(printed.contains("--theme"), "the host's flags survived");
    assert!(printed.contains("--eval"), "the door is documented");
}
