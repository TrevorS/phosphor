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

/// The door stays quiet on a machine that has never run anything.
///
/// This is [`the_door_prints_without_a_terminal`] with the one variable that
/// test cannot see: a `HOME` with no `.local/share` in it. `steel-core`'s
/// `Engine::new` wants a home directory and writes `Unable to create steel home
/// directory …` **to stderr** when it cannot make one, and it cannot make one
/// when the parent is missing too.
///
/// It reached CI before it reached us, which is the whole reason this test
/// exists: macOS ships `~/.local/share`, so every local run passed, and a
/// GitHub runner does not, so `the_door_prints_without_a_terminal` failed on
/// Linux only. A test whose result depends on which developer ran it is not a
/// test — so this one brings its own `HOME` and gets the same answer anywhere.
///
/// `XDG_DATA_HOME` and `STEEL_HOME` are cleared as well as `HOME` set, because
/// either would send Steel somewhere that already exists and the probe would
/// pass without proving anything.
#[test]
fn the_door_stays_quiet_on_a_home_that_has_never_run_anything() {
    let home = scratch("empty-home");
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(&home).expect("a fresh home");

    let printed = phosphor()
        .args(["--eval", "(+ 1 2)"])
        .stdin(Stdio::null())
        .env_remove("XDG_DATA_HOME")
        .env_remove("STEEL_HOME")
        .env("HOME", &home)
        .output()
        .expect("the binary runs");

    let diagnostics = String::from_utf8_lossy(&printed.stderr).into_owned();
    let _ = fs::remove_dir_all(&home);

    assert!(
        diagnostics.is_empty(),
        "a fresh HOME made the door talk to stderr, which in the TUI is a torn \
         frame (Design Language §8): {diagnostics:?}"
    );
    assert_eq!(String::from_utf8_lossy(&printed.stdout).trim(), "3");
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
fn an_unanswerable_query_raises_and_keeps_the_task_that_builds_it() {
    // Naming the task is what `QueryError::NotYetImplemented` is for — the
    // caller learns what to wait for instead of getting "unknown query".
    //
    // **`T022` wired the VM in**, so the source reaches Steel and the binding
    // reaches the host. **`T100` fixed what came back.** A query that cannot be
    // answered *raises* (`phosphor-steel`'s `registry.rs`: refusals are values,
    // errors are errors), and until this task there was no `Outcome` case for
    // that — so the line said `#refused` and carried Steel's own envelope
    // around a sentence that was already in Design Language §6's voice:
    //
    //     #refused · Error: Generic: not built yet — T041 builds it
    //
    // Asserted as the whole line rather than a `starts_with` plus a `contains`,
    // which is the pair that let the envelope live here unnoticed.
    let out = run(&["--eval", EXPR]);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert_eq!(printed, "#raised · not built yet — T041 builds it\n");
}

#[test]
fn a_generated_verb_is_reachable_end_to_end() {
    // `T024`'s CLI third: the verb parses, assembles a call, decodes into the
    // Action, and answers. Every flag here came out of the registry row.
    let out = run(&["mark-seen", "--target", "region", "--target.region.id", "3"]);
    let printed = String::from_utf8_lossy(&out.stdout);
    // The whole line, from the real binary. `T100` collapsed this door's own
    // phrasing into `Refusal::why`, so what a shell sees here is byte-for-byte
    // what the REPL and a float show — asserted rather than assumed, because
    // "they agree" is the claim that was false before.
    assert_eq!(
        printed, "#refused · not built yet — T041 builds it\n",
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
    // the *parser* accepts the host's line unchanged now that 215 subcommands
    // sit beside it, which `--help` exercises without drawing a frame.
    let help = run(&["--help"]);
    let _ = fs::remove_file(&file);
    assert!(help.status.success());
    let printed = String::from_utf8_lossy(&help.stdout);
    assert!(printed.contains("--theme"), "the host's flags survived");
    assert!(printed.contains("--eval"), "the door is documented");
}

/// A relative `XDG_CONFIG_HOME` is ignored, on the *read* side too — `T101`.
///
/// The window that added `phosphor_core::config` filtered on `is_absolute`
/// there (*"resolving one against the working directory would put a user's
/// keymap wherever they happened to launch from"*) and left
/// `Runtime::root`'s own copy of the same walk unfiltered. So the layer booted
/// from `./cfg/phosphor` while `persist-form!` wrote under `$HOME/.config` —
/// the split `AppHost::persist_target` exists to prevent, arrived at from the
/// other end.
///
/// A process test because the resolution reads the environment and
/// `std::env::set_var` is `unsafe` in edition 2024; a child gets its own.
#[test]
fn a_relative_xdg_config_home_boots_no_layer_at_all() {
    let home = scratch("relative-config");
    let _ = fs::remove_dir_all(&home);
    let layer = home.join("cfg").join("phosphor");
    fs::create_dir_all(&layer).expect("a layer under a relative config home");
    fs::write(layer.join("init.scm"), "(displayln \"RELATIVE-LAYER\")\n").expect("an init.scm");

    let printed = phosphor()
        .args(["--eval", "(+ 1 2)"])
        .stdin(Stdio::null())
        // `current_dir` is the whole test: `cfg` resolves against it, and the
        // checkout candidate — `./runtime` — does not exist here.
        .current_dir(&home)
        .env_remove("PHOSPHOR_RUNTIME")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", "cfg")
        .output()
        .expect("the binary runs");

    let out = String::from_utf8_lossy(&printed.stdout).into_owned();
    let _ = fs::remove_dir_all(&home);

    assert!(
        !out.contains("RELATIVE-LAYER"),
        "a relative XDG_CONFIG_HOME was resolved against the working \
         directory on the read side: {out:?}"
    );
    assert_eq!(out.trim(), "3");
}

/// **`OPEN-QUESTIONS.md` §34's reproduction, as a test.**
///
/// The measurement that opened §34 was this command, run against a config home
/// holding the single file `phosphor_core::config`'s header used to draw — an
/// `init.scm` with one `(set-option! "soft-wrap" #t)` in it:
///
/// ```text
/// phosphor --eval '(length phosphor/boot-files)'
/// #raised · unbound identifier — Cannot reference an identifier before its definition
/// ```
///
/// Nothing shipped had loaded. `Runtime::root` was a first-match-wins search
/// with the config home second, so that one file *became* the runtime tree —
/// and driven through a pty it was an editor with an empty statusline, `:`
/// drawing `unknown key :` and `ZQ` doing nothing, with **no boot float**,
/// because the user's one form ran cleanly.
///
/// Two claims, in one process because they are one question. `user-ran` is
/// visible, so the user's file ran; `ZQ` resolves to the shipped `quit`, so
/// `runtime/keymaps.scm` ran too — and `keymap-set!` is defined *there*, which
/// is why a user's file loading after the tree rather than instead of it is
/// what makes both true at once.
///
/// A process test for [`a_relative_xdg_config_home_boots_no_layer_at_all`]'s
/// reason: the resolution reads the environment, and `std::env::set_var` is
/// `unsafe` in edition 2024.
#[test]
fn a_config_home_layers_over_the_shipped_tree() {
    let config = scratch("layering");
    let _ = fs::remove_dir_all(&config);
    let layer = config.join("phosphor");
    fs::create_dir_all(&layer).expect("a config home");
    fs::write(
        layer.join("init.scm"),
        "(set-option! \"soft-wrap\" #t)\n(define user-ran 7)\n",
    )
    .expect("the file a user writes first");

    // The checkout, so the tree under test is the *shipped* one rather than
    // whatever the developer running this has in `$PHOSPHOR_RUNTIME`.
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let printed = phosphor()
        .args([
            "--eval",
            "(list user-ran (phosphor/resolve \"normal\" \"ZQ\"))",
        ])
        .stdin(Stdio::null())
        .current_dir(&checkout)
        .env_remove("PHOSPHOR_RUNTIME")
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .expect("the binary runs");

    let out = String::from_utf8_lossy(&printed.stdout).into_owned();
    let _ = fs::remove_dir_all(&config);

    assert!(
        out.starts_with("(7 "),
        "the user's own init.scm never ran: {out:?}"
    );
    assert!(
        out.contains("\"run\"") && out.contains("\"quit\""),
        "the shipped keymap did not survive a user's init.scm — §34's whole \
         defect, and the state a person could not quit out of: {out:?}"
    );
}
