//! `T036`'s acceptance criterion, against the real thing: *"rust-analyzer
//! attaches and reports ready."*
//!
//! # This test skips, and the skip is the interesting part
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

/// A crate small enough that rust-analyzer answers `initialize` before it has
/// anything to index. Written fresh so the test does not depend on the state of
/// the repository it is run from.
fn fixture() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let root = std::env::temp_dir().join(format!("phosphor-lsp-ra-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).expect("fixture dirs");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phosphor-lsp-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(left: i32, right: i32) -> i32 {\n    left + right\n}\n",
    )
    .expect("fixture source");
    root
}

/// Whether the blessed command can actually serve, asked by running it.
fn usable(spec: &ServerSpec, root: &Path) -> Result<String, String> {
    let output = Command::new(&spec.command)
        .arg("--version")
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{} could not be started: {error}", spec.command))?;
    if !output.status.success() {
        return Err(format!(
            "`{} --version` exited {}: {}",
            spec.command,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The acceptance criterion, in one test.
#[test]
fn rust_analyzer_attaches_and_reports_ready() {
    let root = fixture();
    let language = LanguageId("rust".to_owned());
    let spec = blessed(&language).expect("rust is first-class and has a blessed server");

    let version = match usable(&spec, &root) {
        Ok(version) => version,
        Err(why) => {
            println!("SKIP  T036 acceptance — no usable rust-analyzer: {why}");
            drop(std::fs::remove_dir_all(&root));
            return;
        }
    };
    println!("T036 acceptance — against {version}");

    // The root is found the way the editor will find it: from the file, by the
    // markers the blessed spec declares.
    let source = root.join("src/lib.rs");
    let found = spec
        .root_for(&source)
        .expect("the fixture's Cargo.toml is the root");
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
                identity.name, "rust-analyzer",
                "the server named itself in its initialize response"
            );
            assert!(
                identity.version.is_some(),
                "rust-analyzer reports a version, and reading it is how a bug report gets one"
            );
        }
        other => {
            drop(std::fs::remove_dir_all(&root));
            panic!("rust-analyzer did not reach Ready: {other:?}");
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
    std::thread::sleep(Duration::from_secs(1));
    let after = servers.state(&language);
    drop(std::fs::remove_dir_all(&root));
    assert!(
        after.is_ready(),
        "rust-analyzer was ready and then was not: {after:?}"
    );
    assert!(
        posted.lock().expect("sink").iter().all(|action| matches!(
            action,
            Action::Lsp(phosphor_core::action::LspAction::IngestDiagnostics { .. })
        )),
        "the only Action an LSP client posts today is IngestDiagnostics"
    );
}
