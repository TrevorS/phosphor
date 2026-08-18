//! `T036`'s acceptance criterion and `CP-4`'s, against the real things:
//! rust-analyzer, typescript-language-server and pyright-langserver each
//! attach and report ready.
//!
//! # One file, three servers, and why it grew
//!
//! This was `lsp_rust_analyzer.rs` and covered one. `CP-4`'s checklist asks for
//! all three, and its own mechanical half recorded the gap in the plainest
//! terms available: *"`typescript`, `javascript` and `python` declare
//! `typescript-language-server` and `pyright-langserver`, and **nothing
//! automated has ever attached to either**"*. The `rootUri` defect at `T036` is
//! what that cost — two of the twelve shipped with a server that could not
//! initialize, and a human running the binary is what found it.
//!
//! `docker/lsp.Dockerfile` is where all three exist at once. Run it with
//! `just lsp-docker`.
//!
//! # These tests skip, and the skip is the interesting part
//!
//! **CI has no rust-analyzer, and a test that reddens CI for a missing tool is
//! worse than no test** — it trains everyone to ignore a red build. So this one
//! probes for the server and returns without asserting when it is not usable,
//! printing why. `tests/lsp.rs` covers every edge in the client with fake
//! servers made of `sh`, which need nothing installed; what is *only* provable
//! here is that a real language server, with its real handshake, reaches
//! [`ServerState::Ready`].
//!
//! **The probe runs the binary rather than looking for it, and that is the same
//! lesson the module is built on.** On the machine `T036` was written on,
//! `rust-analyzer` was on `PATH` — as a symlink to `rustup`, which answers
//!
//! ```text
//! error: Unknown binary 'rust-analyzer' in official toolchain 'stable-aarch64-apple-darwin'.
//! ```
//!
//! and exits 1. `which rust-analyzer` succeeded, `Command::new` succeeded, and
//! the client saw an immediate EOF. A presence check would have "found" a
//! server that cannot serve; running `--version` is what tells the two apart,
//! and it costs one process.
//!
//! **It is also cwd-sensitive, which is why the probe runs in the fixture.**
//! The same shim resolves a *different* toolchain depending on the directory it
//! is started in, so a probe that passes in the repository and a spawn that
//! fails in a temp directory is a reachable state. Both run in the same place.

// The skip message is the deliverable when this test skips: a silent skip and a
// missing test are indistinguishable in a log. `print_stdout` is denied
// workspace-wide because a stray print corrupts the TUI frame; a test binary
// owns its own stdout, as `grammar_abi.rs` records for the same reason.
#![allow(clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use phosphor_buffer::lsp::{LanguageServers, ServerSpec, ServerState, blessed, unwatched};
use phosphor_core::action::Action;
use phosphor_core::request::LanguageId;

/// A project small enough that its server answers `initialize` before it has
/// anything to index, written fresh so no test depends on the state of the
/// repository it is run from.
///
/// **Each one carries the root marker its own blessed spec declares** — a
/// `Cargo.toml`, a `tsconfig.json`, a `pyproject.toml` — because `root_for` is
/// half of what is under test: a server handed the wrong root is the `T036`
/// defect, and it is not visible from the outside until something asks it a
/// question.
fn fixture(language: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let root = std::env::temp_dir().join(format!(
        "phosphor-lsp-{language}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).expect("fixture dirs");
    match language {
        "rust" => {
            std::fs::write(
                root.join("Cargo.toml"),
                "[package]\nname = \"phosphor-lsp-fixture\"\nversion = \"0.0.0\"\n\
                 edition = \"2021\"\n",
            )
            .expect("fixture manifest");
            std::fs::write(
                root.join("src/lib.rs"),
                "pub fn add(left: i32, right: i32) -> i32 {\n    left + right\n}\n",
            )
            .expect("fixture source");
        }
        "typescript" => {
            // `tsconfig.json` rather than `package.json`: both are root markers
            // for this spec, and the one that makes tsserver treat the
            // directory as a *project* is this one.
            std::fs::write(root.join("tsconfig.json"), "{ \"include\": [\"src\"] }\n")
                .expect("fixture tsconfig");
            std::fs::write(
                root.join("src/lib.ts"),
                "export function add(left: number, right: number): number {\n\
                 \x20   return left + right;\n}\n",
            )
            .expect("fixture source");
            // **The server refuses to initialize without a `typescript` it can
            // resolve from the workspace**, and finding that out is what this
            // test is for. Verbatim, the first time it ran:
            //
            // ```text
            // Could not find a valid TypeScript installation. Please ensure
            // that the "typescript" dependency is installed in the workspace or
            // that a valid `tsserver.path` is specified. Exiting.
            // ```
            //
            // This comment used to say the server *"does not consult the global
            // install"*. It does — a globally installed server resolves
            // `typescript` as its **sibling** in the npm global root, which the
            // container proved by reaching `Ready` with no `node_modules` at
            // all. The reason the machine this was written on still fails is
            // one version number, not one search path: its global `typescript`
            // is 7.0.2, the native port, which ships no `lib/tsserver.js`.
            //
            // So the fixture links a *known-good* `typescript` in, which is
            // what a real project's `node_modules` would hold and what makes
            // this test say the same thing on every machine. The link is
            // `usable_typescript`'s, called by the harness — which is what
            // turns "no typescript this server can drive" into a skip naming
            // the reason rather than a `Crashed(Protocol(…))` panic.
        }
        "python" => {
            std::fs::write(
                root.join("pyproject.toml"),
                "[project]\nname = \"phosphor-lsp-fixture\"\nversion = \"0.0.0\"\n",
            )
            .expect("fixture pyproject");
            std::fs::write(
                root.join("src/lib.py"),
                "def add(left: int, right: int) -> int:\n    return left + right\n",
            )
            .expect("fixture source");
        }
        other => panic!("no fixture shape for {other}"),
    }
    root
}

/// Put a `typescript` where `typescript-language-server` will look for it, and
/// say whether the one found can actually drive it.
///
/// Asks npm for the global root rather than guessing a path, so a developer's
/// machine and `docker/lsp.Dockerfile` both work without either being
/// special-cased.
///
/// # `typescript` being installed is not enough, and this is why the container
/// exists
///
/// The machine this was written on has **typescript 7.0.2** installed globally
/// — the native-port rewrite, which ships `tsgo` and has no `lib/tsserver.js`
/// at all. `typescript-language-server` 5.3.0 drives the classic `tsserver.js`
/// and cannot use it, so on that host the server starts, answers `initialize`
/// with an error, and lands in `Crashed`:
///
/// ```text
/// Could not find a valid TypeScript installation. Please ensure that the
/// "typescript" dependency is installed in the workspace or that a valid
/// `tsserver.path` is specified. Exiting.
/// ```
///
/// A *version* check here would be guessing at a compatibility matrix that
/// changes without us. Checking for the file the server actually loads is the
/// question it will ask itself, so that is what this asks.
fn usable_typescript(root: &Path) -> Result<(), String> {
    let output = Command::new("npm")
        .arg("root")
        .arg("-g")
        .output()
        .map_err(|error| format!("npm could not be run: {error}"))?;
    if !output.status.success() {
        return Err("`npm root -g` failed".to_owned());
    }
    let global = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    let from = global.join("typescript");
    if !from.join("lib/tsserver.js").is_file() {
        return Err(format!(
            "{} has no lib/tsserver.js — typescript-language-server drives the classic \
             tsserver, and typescript 7's native port does not ship one",
            from.display()
        ));
    }
    let modules = root.join("node_modules");
    std::fs::create_dir_all(&modules).map_err(|error| format!("node_modules: {error}"))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(&from, modules.join("typescript"))
        .map_err(|error| format!("linking typescript into the fixture: {error}"))?;
    Ok(())
}

/// The file a fixture's server is asked about.
fn source_of(language: &str, root: &Path) -> PathBuf {
    match language {
        "rust" => root.join("src/lib.rs"),
        "typescript" => root.join("src/lib.ts"),
        "python" => root.join("src/lib.py"),
        other => panic!("no source for {other}"),
    }
}

/// Whether the blessed command can actually serve, asked by running it.
///
/// # `--version` exiting non-zero is not always a broken server
///
/// `pyright-langserver --version` exits **1**, saying
///
/// ```text
/// Error: Connection input stream is not set. Use arguments of
/// createConnection or set command line parameters: '--node-ipc', '--stdio'
/// or '--socket={number}'
/// ```
///
/// which is not a failure to run — it is the server parsing its arguments and
/// objecting that no transport was named. It is *proof* the binary is real,
/// which is exactly what this probe is for, and it took a red test to notice.
///
/// The distinction that matters is the one the module header records: a rustup
/// shim answers *"Unknown binary"* and exits 1 too, so the exit code alone
/// cannot separate them. What separates them is **who is complaining** — a
/// server that talks about its own transport has already started.
fn usable(spec: &ServerSpec, root: &Path) -> Result<String, String> {
    let output = Command::new(&spec.command)
        .arg("--version")
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{} could not be started: {error}", spec.command))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    if stderr.contains("Connection input stream is not set") {
        return Ok(format!("{} (spoke up about its transport)", spec.command));
    }
    Err(format!(
        "`{} --version` exited {}: {stderr}",
        spec.command, output.status
    ))
}

/// The acceptance criterion, for one language.
///
/// `named` is what the server calls **itself** in its `initialize` response,
/// which is not required to be the command and is worth asserting for that
/// reason: `7c`'s statusline chip draws *"the name a server gives itself"*, so
/// this is the only place that claim is checked against a real handshake.
///
/// **All three happen to match their command, and that was not the guess.**
/// This parameter was added believing `pyright-langserver` would answer
/// `pyright` — the npm package's name, and what its CLI sibling is called. It
/// answers `pyright-langserver`. The parameter stays: three servers agreeing
/// today is a fact about three servers, not a rule, and a fourth that
/// disagreed would otherwise be found by a confusing statusline rather than
/// here.
fn attaches_and_reports_ready(language: &str, named: &str, versioned: bool) {
    let root = fixture(language);
    let language = LanguageId(language.to_owned());
    let spec = blessed(&language).expect("a first-class language has a blessed server");

    // A server can be perfectly installed and still have nothing to run: see
    // `usable_typescript`. Checked before the probe so the skip names the
    // *reason*, which a `Crashed(Protocol(…))` panic would bury.
    if language.0 == "typescript"
        && let Err(why) = usable_typescript(&root)
    {
        println!("SKIP  CP-4 acceptance — typescript-language-server has nothing to drive: {why}");
        drop(std::fs::remove_dir_all(&root));
        return;
    }

    let version = match usable(&spec, &root) {
        Ok(version) => version,
        Err(why) => {
            println!("SKIP  CP-4 acceptance — no usable {}: {why}", spec.command);
            drop(std::fs::remove_dir_all(&root));
            return;
        }
    };
    println!("CP-4 acceptance — {} against {version}", spec.command);
    // Kept before `attach` consumes the spec, so a failure can still name the
    // command that produced it.
    let command = spec.command.clone();

    // The root is found the way the editor will find it: from the file, by the
    // markers the blessed spec declares.
    let source = source_of(&language.0, &root);
    let found = spec
        .root_for(&source)
        .expect("the fixture carries this spec's own root marker");
    assert_eq!(found, root);

    let posted: Arc<Mutex<Vec<Action>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&posted);
    let servers = LanguageServers::start(
        Arc::new(move |action| {
            sink.lock().expect("sink").push(action);
            true
        }),
        unwatched(),
    );
    servers.open(
        &language,
        source.clone(),
        std::fs::read_to_string(&source).expect("fixture source"),
    );
    servers.attach(spec, found);

    let deadline = Instant::now() + Duration::from_secs(60);
    let state = loop {
        let state = servers.state(&language);
        if state.is_ready() || state.failure().is_some() || Instant::now() > deadline {
            break state;
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    match &state {
        ServerState::Ready(identity) => {
            assert_eq!(
                identity.name, named,
                "the server named itself in its initialize response"
            );
            // **Only one of the three sends one, and nothing on screen needs
            // it.** rust-analyzer reports a version; `typescript-language-server`
            // and `pyright-langserver` both send none, which is a server's
            // choice and not a client defect — and
            // `server_chip` draws `identity.name` alone, so a missing version
            // costs nothing a person would see. Asserted per server rather
            // than dropped, because the one that *does* send a version
            // regressing to silence is a real change and this is the only
            // place that would notice.
            assert_eq!(
                identity.version.is_some(),
                versioned,
                "{named} changed its mind about reporting a version"
            );
        }
        other => {
            drop(std::fs::remove_dir_all(&root));
            panic!("{command} did not reach Ready: {other:?}");
        }
    }

    // **And it stays attached, which is a different claim.** A real server
    // talks after `initialize` — `$/progress`, `window/logMessage`,
    // `client/registerCapability` — and a client that mishandles any of it
    // drops to `Crashed` seconds after reporting `Ready`.
    //
    // This second is a *weak* check and is labelled as one: with
    // `router`'s `unhandled_notification` catch-all deleted, this test still
    // passed, because rust-analyzer sends nothing but `$/progress` in that
    // window on a two-function crate — and `$/progress` is the one prefix
    // `async-lsp`'s default tolerates. The test that actually holds that line
    // is `a_servers_chatter_does_not_take_the_client_down` in `tests/lsp.rs`,
    // where the fake sends exactly the notification that would break it.
    // **This parameter used to have a `stays: false` case, and closing it is
    // what the container was built for.** `typescript-language-server` reached
    // `Ready` and was gone inside the second with `Exited("the underlying
    // channel reached EOF")`, recorded at `T036` as a finding with closing
    // stdin as the suspected mechanism.
    //
    // Stdin was a red herring, and the reason it looked right is worth keeping:
    // an identical handshake driven from a node script survives, and adding a
    // stdin close to that script kills the server instantly — a true fact about
    // the server that had nothing to do with the crash. What the script also
    // did was never *answer* the `window/workDoneProgress/create` request, and
    // a request left hanging is survivable where an error answer is not. The
    // client announced `window.workDoneProgress: true` and then refused the one
    // request that capability invites; `router` is the fix and
    // `tests/lsp.rs::a_capability_we_announced_is_a_request_we_answer` is the
    // version of this that needs no node.
    //
    // The finding was found by piping the server's stderr, which the client
    // used to discard — see `LastWords`. It said, in full:
    //
    // ```text
    // ResponseError: phosphor's LSP client answers no requests yet
    //     at handleResponse (typescript-language-server/lib/cli.mjs:4305:40)
    // ```
    std::thread::sleep(Duration::from_secs(1));
    let after = servers.state(&language);
    drop(std::fs::remove_dir_all(&root));
    assert!(
        after.is_ready(),
        "{named} was ready and then was not: {after:?}"
    );
    assert!(
        posted.lock().expect("sink").iter().all(|action| matches!(
            action,
            Action::Lsp(phosphor_core::action::LspAction::IngestDiagnostics { .. })
        )),
        "the only Action an LSP client posts today is IngestDiagnostics"
    );
}

/// `T036`'s own acceptance, and the one server this repository could already
/// prove.
#[test]
fn rust_analyzer_attaches_and_reports_ready() {
    attaches_and_reports_ready("rust", "rust-analyzer", true);
}

/// **`CP-4`, first of the two it was missing.** `typescript.scm` records that
/// this is the tsserver *wrapper* rather than tsserver itself, because tsserver
/// speaks its own protocol and not LSP — so what attaches here is what the
/// declaration actually names.
#[test]
fn typescript_language_server_attaches_and_reports_ready() {
    attaches_and_reports_ready("typescript", "typescript-language-server", false);
}

/// **`CP-4`, second of the two.** It names itself `pyright-langserver` — the
/// command, not the npm package — which is the opposite of what the harness
/// above was written expecting. Recorded there.
#[test]
fn pyright_attaches_and_reports_ready() {
    attaches_and_reports_ready("python", "pyright-langserver", false);
}

// **A test that was written here and deleted, because the container refuted the
// finding it was asserting.**
//
// `T036` recorded that *"a `.ts` file in a directory with no `node_modules`
// gets a crashed server"* because *"resolution walks up from the workspace and
// never consults the global install"*. The first half is true on the machine it
// was found on. The second half is false, and this file is where that was
// proved: the assertion reached `Ready` in the container, and
//
// ```text
// require.resolve("typescript", { paths: ["…/typescript-language-server/lib"] })
//   → /usr/lib/node_modules/typescript/lib/typescript.js
// ```
//
// says why — a globally installed server finds a globally installed
// `typescript` as its **sibling**, which is exactly what node's resolution is
// supposed to do.
//
// So the crash on the original host has one cause, not two: its global
// `typescript` is 7.0.2, the native port, which ships no `lib/tsserver.js` for
// the server to drive. `usable_typescript` already asks that question the right
// way, by looking for the file rather than comparing versions. The finding is
// corrected at `T036`.
//
// What the fix in `LastWords` buys here is real and is tested where it can be:
// the failure now carries the server's own sentence rather than `the underlying
// channel reached EOF`. `tests/lsp.rs::a_crash_carries_what_the_server_said_on_
// the_way_out` states that with a fake, so it holds on every machine instead of
// only on one.
