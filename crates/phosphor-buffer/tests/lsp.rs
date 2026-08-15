//! `T036`'s acceptance tests: the readable ones.
//!
//! Three groups, and the split is deliberate.
//!
//! * **Configuration and conversion** are pure, so they are stated as examples
//!   a reader can check by eye. `tests/lsp_properties.rs` states the same rules
//!   as laws over generated input; this file is where you look to find out what
//!   the rules *are*.
//! * **The client** is exercised against **fake servers built out of `sh`** —
//!   one that answers `initialize` and then publishes a diagnostic, one that
//!   never says anything at all, one that exits immediately, and one that does
//!   not exist. Each is four lines of shell and a file of pre-framed bytes, and
//!   between them they cover every edge in [`ServerState`] that does not need a
//!   real language server.
//! * **rust-analyzer itself** is `tests/lsp_rust_analyzer.rs`, separately,
//!   because it is the one test here that can be skipped.
//!
//! **Why a shell script is a legitimate language server.** The protocol over a
//! pipe is `Content-Length: N\r\n\r\n` and then N bytes of JSON. A server that
//! waits for one line of input and then `cat`s a file of pre-framed responses is
//! a real peer as far as the transport is concerned, and it is *deterministic*,
//! which rust-analyzer is not: no indexing, no version drift, no cargo. The
//! `read` before the `cat` is what orders the exchange — the reply cannot
//! precede the request that it answers.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use phosphor_buffer::lsp::{
    FIRST_CLASS, Failure, LanguageServers, Post, Question, ServerEvent, ServerIdentity, ServerSpec,
    ServerState, blessed, column_from_utf16, diagnostic_from_lsp, file_edits_from_lsp, line_at,
    locations_from_lsp, lsp_types, position_from_lsp, position_to_lsp, severity_from_lsp,
    span_from_lsp, unwatched, utf16_from_column, utf16_len,
};
use phosphor_core::action::{Action, LspAction};
use phosphor_core::request::{
    Diagnostic, FileSpan, LanguageId, LanguageSpec, Position, Severity, Span,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A directory that removes itself. Same shape as `phosphor-core`'s, and for
/// the same reason: no `tempfile` dependency for a test.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path =
            std::env::temp_dir().join(format!("phosphor-lsp-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn made(&self, name: &str) -> PathBuf {
        let path = self.join(name);
        fs::create_dir_all(&path).expect("subdir");
        path
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.join(name);
        fs::write(&path, contents).expect("write");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

/// One LSP frame: the header the specification requires, and the body.
fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

/// The `initialize` response every fake server sends. `id: 0` because
/// `async-lsp`'s first outgoing request is numbered zero
/// (`MainLoop::new` sets `outgoing_id: 0`), and `initialize` is by definition
/// the first request a client sends.
fn initialize_response(name: &str) -> String {
    frame(&format!(
        r#"{{"jsonrpc":"2.0","id":0,"result":{{"capabilities":{{}},"serverInfo":{{"name":"{name}","version":"0.1"}}}}}}"#
    ))
}

/// A server built out of `sh`: wait for the client to say something, pause,
/// write these bytes, then stay alive so the pipe does not close.
///
/// Two details are load-bearing. `sh`'s `read` is byte-at-a-time on a pipe by
/// specification, so it consumes one header line and no more — the client's
/// request is not eaten. And the pause is what makes
/// [`ServerState::Starting`] *observable*: a server that answers instantly is
/// indistinguishable from one that was never starting, and a test that cannot
/// see the intermediate state cannot check that a restart passes through it.
fn fake_server(dir: &TempDir, tag: &str, frames: &str) -> ServerSpec {
    let script = dir.write(&format!("{tag}.frames"), frames);
    ServerSpec::new(tag, "sh")
        .with_args([
            "-c".to_owned(),
            format!(
                "read -r _ ; sleep 0.3 ; cat {} ; exec sleep 30",
                script.display()
            ),
        ])
        .with_ready_timeout(Duration::from_secs(10))
}

/// The sink the host would supply, standing in for `events::Poster::post`.
#[derive(Clone, Debug, Default)]
struct Sink(Arc<Mutex<Vec<Action>>>);

impl Sink {
    fn post(&self) -> Post {
        let seen = Arc::clone(&self.0);
        Arc::new(move |action| {
            seen.lock().expect("sink").push(action);
            true
        })
    }

    fn actions(&self) -> Vec<Action> {
        self.0.lock().expect("sink").clone()
    }
}

/// Polls until `want` holds or the deadline passes, and hands back what it
/// last saw either way.
///
/// **Polling, not blocking on the client.** There is deliberately no
/// `wait_until_ready` on [`LanguageServers`]: the editor never waits for a
/// server, so an API that lets it would be a hole in the property this module
/// is built around. A test polls the same way a frame would.
fn settle(
    servers: &LanguageServers,
    language: &LanguageId,
    want: impl Fn(&ServerState) -> bool,
) -> ServerState {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let state = servers.state(language);
        if want(&state) || Instant::now() > deadline {
            return state;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn language(name: &str) -> LanguageId {
    LanguageId(name.to_owned())
}

// ---------------------------------------------------------------------------
// Blessed, not discovered
// ---------------------------------------------------------------------------

/// The load-bearing half of the task, and the only test that can state it: the
/// command is a name we wrote down, with the terms we chose, and nothing
/// consults the machine to produce it.
#[test]
fn a_blessed_server_is_a_declaration_not_a_lookup() {
    let spec = blessed(&language("rust")).expect("rust is first-class and has a server");
    assert_eq!(spec.command, "rust-analyzer");
    assert!(
        spec.args.is_empty(),
        "rust-analyzer speaks LSP with no flags"
    );
    assert_eq!(
        spec.root_markers,
        vec!["Cargo.toml".to_owned(), "rust-project.json".to_owned()],
        "a rust project's root is a manifest, and we say which"
    );
    assert_eq!(spec.ready_timeout, phosphor_buffer::lsp::READY_TIMEOUT);
}

/// Every first-class language is *answered* — with a server or with an honest
/// `None`. The list is closed, so this is a test that it stays closed.
#[test]
fn every_first_class_language_is_answered_one_way_or_the_other() {
    let serverless = ["steel", "csv", "markdown"];
    assert_eq!(FIRST_CLASS.len(), 12, "the list is short on purpose");
    for name in FIRST_CLASS {
        let spec = blessed(&language(name));
        if serverless.contains(&name) {
            assert!(spec.is_none(), "{name} is declared serverless, not omitted");
        } else {
            let spec = spec.unwrap_or_else(|| panic!("{name} has no blessed server"));
            assert!(!spec.command.is_empty());
            assert_eq!(spec.language.0, name);
        }
    }
}

/// A language nobody declared gets nothing — the second tier, and no guessing.
#[test]
fn an_undeclared_language_gets_no_server_rather_than_a_guess() {
    assert!(blessed(&language("cobol")).is_none());
    assert!(blessed(&language("")).is_none());
}

/// `T037`'s door: a `define-language` declaration replaces the command and
/// keeps what the blessed entry knew about finding a root.
#[test]
fn a_declaration_overrides_the_command_and_inherits_the_root_markers() {
    let declared = LanguageSpec {
        extensions: vec!["rs".to_owned()],
        grammar: Some("rust".to_owned()),
        lsp_command: vec![
            "my-wrapper".to_owned(),
            "--server".to_owned(),
            "rust-analyzer".to_owned(),
        ],
        comment_prefix: Some("//".to_owned()),
    };
    let spec = ServerSpec::from_language_spec(&language("rust"), &declared).expect("a command");
    assert_eq!(spec.command, "my-wrapper");
    assert_eq!(
        spec.args,
        vec!["--server".to_owned(), "rust-analyzer".to_owned()]
    );
    assert_eq!(
        spec.root_markers,
        blessed(&language("rust")).expect("rust").root_markers,
        "swapping the binary does not change where a rust project starts"
    );
}

/// *"Empty means none"* — the second tier, expressed by a declaration rather
/// than by an error.
#[test]
fn a_declaration_with_no_command_is_second_tier() {
    let declared = LanguageSpec {
        extensions: vec!["txt".to_owned()],
        grammar: None,
        lsp_command: Vec::new(),
        comment_prefix: None,
    };
    assert!(ServerSpec::from_language_spec(&language("text"), &declared).is_none());
}

/// Nearest marker wins: a workspace member's own manifest is the root, not the
/// workspace's. Indexing the smaller thing is the point.
#[test]
fn the_project_root_is_the_nearest_marker_not_the_outermost() {
    let dir = TempDir::new("roots");
    fs::write(dir.join("Cargo.toml"), "[workspace]\n").expect("workspace manifest");
    let member = dir.made("member");
    fs::write(member.join("Cargo.toml"), "[package]\n").expect("member manifest");
    let source = dir.made("member/src");
    let file = source.join("lib.rs");
    fs::write(&file, "fn main() {}\n").expect("source");

    let spec = blessed(&language("rust")).expect("rust");
    assert_eq!(spec.root_for(&file).as_deref(), Some(member.as_path()));
    // And a file with no manifest above it has no root, rather than the
    // filesystem's.
    let orphan = TempDir::new("orphan");
    assert_eq!(spec.root_for(&orphan.join("stray.rs")), None);
}

/// A spec with no markers has no opinion, which is different from having a
/// wrong one.
#[test]
fn a_spec_with_no_markers_finds_no_root() {
    let dir = TempDir::new("no-markers");
    assert_eq!(
        ServerSpec::new("json", "x").root_for(&dir.join("a.json")),
        None
    );
}

// ---------------------------------------------------------------------------
// The state machine
// ---------------------------------------------------------------------------

fn identity(name: &str) -> ServerIdentity {
    ServerIdentity {
        name: name.to_owned(),
        version: None,
    }
}

#[test]
fn the_ordinary_life_of_a_server() {
    let state = ServerState::default();
    assert_eq!(state, ServerState::NotStarted);
    let state = state.after(&ServerEvent::Attached);
    assert_eq!(state, ServerState::Starting);
    assert!(state.is_starting());
    let state = state.after(&ServerEvent::Initialized(identity("rust-analyzer")));
    assert!(state.is_ready());
    let state = state.after(&ServerEvent::Failed(Failure::Exited("signal".to_owned())));
    assert_eq!(state.failure(), Some(&Failure::Exited("signal".to_owned())));
    let state = state.after(&ServerEvent::Restarted);
    assert_eq!(state, ServerState::Starting, "restart is a start");
}

/// The edge that only exists because restarts do: the process we killed can
/// still have a reply in flight, and taking it would report `Ready` for
/// something that is not running.
#[test]
fn a_late_initialize_cannot_promote_the_server_it_replaced() {
    let crashed = ServerState::Crashed(Failure::Timeout);
    assert_eq!(
        crashed.after(&ServerEvent::Initialized(identity("ghost"))),
        crashed,
        "a reply from a process we gave up on changes nothing"
    );
    assert_eq!(
        ServerState::Stopped.after(&ServerEvent::Initialized(identity("ghost"))),
        ServerState::Stopped
    );
    assert_eq!(
        ServerState::NotStarted.after(&ServerEvent::Initialized(identity("ghost"))),
        ServerState::NotStarted,
        "nothing was started, so nothing can be ready"
    );
}

/// The other one: an exit we asked for is not a crash, and a status line that
/// says otherwise stops meaning anything.
#[test]
fn an_exit_we_asked_for_is_not_a_crash() {
    let stopped = ServerState::Ready(identity("rust-analyzer")).after(&ServerEvent::Stopped);
    assert_eq!(stopped, ServerState::Stopped);
    assert_eq!(
        stopped.after(&ServerEvent::Failed(Failure::Exited("eof".to_owned()))),
        ServerState::Stopped,
        "the EOF after a shutdown is the shutdown happening"
    );
}

#[test]
fn a_failure_reads_back_as_words_a_user_could_act_on() {
    assert_eq!(
        Failure::Spawn("No such file or directory (os error 2)".to_owned()).to_string(),
        "could not start: No such file or directory (os error 2)"
    );
    assert_eq!(Failure::Timeout.to_string(), "timed out during initialize");
}

// ---------------------------------------------------------------------------
// The UTF-16 seam
// ---------------------------------------------------------------------------

/// The trap, in one assertion: an astral character is **two** UTF-16 units and
/// one column, so every column after it is wrong by one if you pass the
/// server's number through.
#[test]
fn an_astral_character_costs_two_utf16_units_and_one_column() {
    let line = "let x = \"🎉\"; bad";
    // Before the pair, a column is one more than its code unit — the 1-based
    // offset and nothing else. `x` is column 5, unit 4.
    assert_eq!(column_from_utf16(line, 4), 5);
    assert_eq!(utf16_from_column(line, 5), 4);
    // The pair itself: one column, two units.
    assert_eq!(
        utf16_from_column(line, 10),
        9,
        "the emoji starts at unit nine"
    );
    assert_eq!(utf16_from_column(line, 11), 11, "and is two units wide");
    // **After it the two numbers coincide**, because the surrogate's extra unit
    // has eaten the 1-based offset. That is exactly how this bug hides: on the
    // half of the line a test usually looks at, passing the server's number
    // straight through is right.
    let bad = line.chars().position(|glyph| glyph == 'b').expect("bad") as u32;
    assert_eq!(bad, 13);
    assert_eq!(
        column_from_utf16(line, 14),
        14,
        "unit and column agree here"
    );
    assert_ne!(
        column_from_utf16(line, 14),
        15,
        "and `character + 1` — the reading that is right before the pair — is wrong after it"
    );
}

/// A two-byte character is one unit and one column: the bug this codebase
/// already hit was a *byte* count, and this is the assertion that says the
/// conversion is not one.
#[test]
fn a_two_byte_character_is_one_unit_and_one_column() {
    let line = "straße";
    assert_eq!(line.len(), 7, "seven bytes");
    assert_eq!(utf16_len(line_at(line, 0)), 6, "six UTF-16 units");
    assert_eq!(column_from_utf16(line, 6), 7, "six units in, column seven");
}

/// A code unit inside a surrogate pair names no column, so it canonicalises to
/// the character containing it rather than inventing one.
#[test]
fn a_position_inside_a_surrogate_pair_lands_on_its_character() {
    let line = "🎉x";
    assert_eq!(column_from_utf16(line, 0), 1);
    assert_eq!(
        column_from_utf16(line, 1),
        1,
        "the low half is still that character"
    );
    assert_eq!(column_from_utf16(line, 2), 2, "and `x` starts at unit two");
}

/// CRLF: the `\r` is a terminator, not content, and a column at the end of a
/// line must not count it.
#[test]
fn a_crlf_line_has_no_phantom_column() {
    let text = "alpha\r\nbeta\r\n";
    assert_eq!(line_at(text, 0), "alpha");
    assert_eq!(line_at(text, 1), "beta");
    assert_eq!(utf16_len(line_at(text, 0)), 5, "not six");
    assert_eq!(
        position_from_lsp(
            text,
            lsp_types::Position {
                line: 1,
                character: 4
            }
        ),
        Position { line: 2, column: 5 }
    );
}

/// Past the end of a line — and past the end of the text — the excess carries
/// through one-for-one rather than clamping. That is what makes the conversion
/// safe when the text is unknown.
#[test]
fn a_position_past_the_end_carries_its_excess() {
    assert_eq!(column_from_utf16("ab", 5), 6);
    assert_eq!(line_at("one\n", 9), "");
    assert_eq!(
        column_from_utf16(line_at("", 3), 7),
        8,
        "with no text at all, a column is the unit count plus one — exact for ASCII"
    );
}

#[test]
fn a_position_converts_back_to_the_one_the_server_sent() {
    let text = "fn main() {\n    let ß = \"🎉\";\n}\n";
    for line in 0..3_u32 {
        for character in 0..12_u32 {
            let at = lsp_types::Position { line, character };
            let converted = position_from_lsp(text, at);
            let back = position_to_lsp(text, converted);
            assert_eq!(back.line, at.line);
            assert_eq!(
                position_from_lsp(text, back),
                converted,
                "conversion is stable at line {line}, character {character}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// What a server says, in phosphor's vocabulary
// ---------------------------------------------------------------------------

#[test]
fn four_lsp_severities_land_on_the_three_the_design_draws() {
    use lsp_types::DiagnosticSeverity as Lsp;
    assert_eq!(severity_from_lsp(Some(Lsp::ERROR)), Severity::Trouble);
    assert_eq!(severity_from_lsp(Some(Lsp::WARNING)), Severity::Attention);
    assert_eq!(severity_from_lsp(Some(Lsp::INFORMATION)), Severity::Info);
    assert_eq!(severity_from_lsp(Some(Lsp::HINT)), Severity::Info);
    assert_eq!(
        severity_from_lsp(None),
        Severity::Attention,
        "an ungraded diagnostic is not promoted to trouble-red"
    );
}

#[test]
fn a_diagnostic_arrives_with_columns_the_editor_can_use() {
    let text = "let 🎉 = wrong;\n";
    let diagnostic = lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 9,
            },
            end: lsp_types::Position {
                line: 0,
                character: 14,
            },
        },
        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
        message: "cannot find value `wrong`".to_owned(),
        source: Some("rustc".to_owned()),
        ..lsp_types::Diagnostic::default()
    };
    // Character 9 is `w`, and it is column **9** — five characters and one
    // surrogate pair in. The naive readings are both wrong here: `character`
    // alone is 9 only by coincidence of the pair cancelling the 1-based offset,
    // and `character + 1` is 10.
    assert_eq!(
        diagnostic_from_lsp(text, &diagnostic),
        Diagnostic {
            span: Span {
                start: Position { line: 1, column: 9 },
                end: Position {
                    line: 1,
                    column: 14
                },
            },
            severity: Severity::Trouble,
            message: "cannot find value `wrong`".to_owned(),
            source: Some("rustc".to_owned()),
        }
    );
}

/// `apply-workspace-edit` is an `Ask`, so the value it carries has to be the
/// same one every time the same edit arrives — a `HashMap`'s order is not.
#[test]
fn a_workspace_edit_is_sorted_converted_and_repeatable() {
    let dir = TempDir::new("edits");
    let first = dir.write("a.rs", "let ß = 1;\n");
    let second = dir.write("b.rs", "let y = 2;\n");
    let edit = lsp_types::WorkspaceEdit {
        changes: Some(
            [
                (
                    lsp_types::Url::from_file_path(&second).expect("uri"),
                    vec![lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: 0,
                                character: 4,
                            },
                            end: lsp_types::Position {
                                line: 0,
                                character: 5,
                            },
                        },
                        new_text: "z".to_owned(),
                    }],
                ),
                (
                    lsp_types::Url::from_file_path(&first).expect("uri"),
                    vec![lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: 0,
                                character: 4,
                            },
                            end: lsp_types::Position {
                                line: 0,
                                character: 5,
                            },
                        },
                        new_text: "s".to_owned(),
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        ),
        ..lsp_types::WorkspaceEdit::default()
    };
    let read = |path: &Path| fs::read_to_string(path).ok();
    let edits = file_edits_from_lsp(&edit, &read);
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].path, first, "sorted by path, not by hash order");
    assert_eq!(edits[1].path, second);
    assert_eq!(edits[0].edits[0].text, "s");
    assert_eq!(
        edits[0].edits[0].span.start,
        Position { line: 1, column: 5 },
        "`ß` is one column, and one UTF-16 unit"
    );
    assert_eq!(
        file_edits_from_lsp(&edit, &read),
        edits,
        "the same edit converts to the same value every time"
    );
}

/// The refusal recorded in [`file_edits_from_lsp`]'s doc, tested rather than
/// asserted: a `WorkspaceEdit` that also creates or deletes files contributes
/// its *edits* and nothing else.
#[test]
fn creates_and_deletes_inside_a_workspace_edit_are_dropped_not_invented() {
    let dir = TempDir::new("ops");
    let path = dir.write("kept.rs", "x\n");
    let edit = lsp_types::WorkspaceEdit {
        document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
            lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Delete(
                lsp_types::DeleteFile {
                    uri: lsp_types::Url::from_file_path(dir.join("gone.rs")).expect("uri"),
                    options: None,
                },
            )),
            lsp_types::DocumentChangeOperation::Edit(lsp_types::TextDocumentEdit {
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::from_file_path(&path).expect("uri"),
                    version: Some(1),
                },
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: 0,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: 0,
                            character: 1,
                        },
                    },
                    new_text: "y".to_owned(),
                })],
            }),
        ])),
        ..lsp_types::WorkspaceEdit::default()
    };
    let edits = file_edits_from_lsp(&edit, &|path| fs::read_to_string(path).ok());
    assert_eq!(edits.len(), 1, "one file edited, no file deleted");
    assert_eq!(edits[0].path, path);
}

#[test]
fn a_span_converts_both_of_its_ends() {
    let span = span_from_lsp(
        "abc\ndé\n",
        lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 1,
            },
            end: lsp_types::Position {
                line: 1,
                character: 2,
            },
        },
    );
    assert_eq!(
        span,
        Span {
            start: Position { line: 1, column: 2 },
            end: Position { line: 2, column: 3 },
        }
    );
}

// ---------------------------------------------------------------------------
// The client, against servers made of `sh`
// ---------------------------------------------------------------------------

/// The whole pipeline in one test: a process is spawned, `initialize` is
/// answered, the state becomes `Ready`, and an unsolicited `publishDiagnostics`
/// arrives at the host as `Action::Lsp(IngestDiagnostics)` — the capability
/// `crates/phosphor/src/events.rs`'s own tests already name as what an LSP
/// client posts.
#[test]
fn a_server_reports_ready_and_its_diagnostics_reach_the_queue_as_an_action() {
    let dir = TempDir::new("ready");
    let source = dir.write("main.rs", "let 🎉 = wrong;\n");
    let uri = lsp_types::Url::from_file_path(&source).expect("uri");
    let frames = format!(
        "{}{}",
        initialize_response("fake-analyzer"),
        frame(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{uri}","diagnostics":[{{"range":{{"start":{{"line":0,"character":9}},"end":{{"line":0,"character":14}}}},"severity":1,"source":"rustc","message":"cannot find value"}}]}}}}"#
        ))
    );
    let spec = fake_server(&dir, "rust", &frames);
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());

    // Before attaching, so the text is recorded before any diagnostic can
    // arrive — `LanguageServers::open` records on this thread for exactly this
    // reason.
    servers.open(
        &language("rust"),
        source.clone(),
        "let 🎉 = wrong;\n".to_owned(),
    );
    servers.attach(spec, dir.path.clone());

    let state = settle(&servers, &language("rust"), ServerState::is_ready);
    assert_eq!(
        state,
        ServerState::Ready(ServerIdentity {
            name: "fake-analyzer".to_owned(),
            version: Some("0.1".to_owned()),
        }),
        "the server named itself in its initialize response"
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    while sink.actions().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let actions = sink.actions();
    assert_eq!(actions.len(), 1, "one publish, one Action");
    match &actions[0] {
        Action::Lsp(LspAction::IngestDiagnostics { path, diagnostics }) => {
            assert_eq!(
                path,
                Path::new("main.rs"),
                "workspace-relative, as declared"
            );
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].severity, Severity::Trouble);
            assert_eq!(
                diagnostics[0].span.start,
                Position { line: 1, column: 9 },
                "converted against the text the client recorded, not passed through"
            );
        }
        other => panic!("expected IngestDiagnostics, got {other:?}"),
    }
}

/// **The property the design calls "no widget ever blocks", tested as a
/// property of this API rather than asserted in a comment.**
///
/// The server never answers anything. Every call the editor makes must return
/// in about the time a function call takes — not in the time the server takes —
/// and the state must be an honest `Starting` until the timeout turns it into a
/// reason.
#[test]
fn a_hung_server_never_blocks_the_editor_and_ends_up_a_reason() {
    let dir = TempDir::new("hung");
    let spec = ServerSpec::new("rust", "sh")
        .with_args(["-c", "exec sleep 30"])
        .with_ready_timeout(Duration::from_millis(200));
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());

    let start = Instant::now();
    servers.attach(spec, dir.path.clone());
    for _ in 0..1_000 {
        drop(servers.state(&language("rust")));
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(150),
        "attach plus a thousand state reads took {elapsed:?}; the editor waited on a server"
    );

    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    assert_eq!(
        state,
        ServerState::Crashed(Failure::Timeout),
        "a server that never answers is wedged, and says so"
    );
    assert!(sink.actions().is_empty(), "a hung server posts nothing");

    // And dropping is not a hang either: the child is `kill_on_drop`, so this
    // returns rather than waiting out the `sleep 30`.
    let start = Instant::now();
    drop(servers);
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "drop waited on the child"
    );
}

/// The `rustup` shim, generalised: a command that is not there is a state with
/// the operating system's own words in it, not a silent absence.
#[test]
fn a_server_that_is_not_installed_is_a_crash_with_a_reason() {
    let dir = TempDir::new("missing");
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(
        ServerSpec::new("rust", "phosphor-no-such-language-server"),
        dir.path.clone(),
    );
    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    match state.failure() {
        Some(Failure::Spawn(why)) => assert!(
            !why.is_empty(),
            "the reason is the OS's, and it is what the user needs to read"
        ),
        other => panic!("expected a spawn failure, got {other:?}"),
    }
}

/// A server that starts and dies — the shim's actual behaviour — is a crash,
/// and it does not wait for the readiness timeout to say so.
#[test]
fn a_server_that_exits_immediately_is_a_crash_before_the_timeout() {
    let dir = TempDir::new("dead");
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(
        ServerSpec::new("rust", "sh")
            .with_args(["-c", "exit 1"])
            .with_ready_timeout(Duration::from_secs(30)),
        dir.path.clone(),
    );
    let start = Instant::now();
    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    assert!(
        matches!(state.failure(), Some(Failure::Exited(_))),
        "expected an exit, got {state:?}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "the main loop noticed EOF rather than waiting out the readiness timeout"
    );
}

/// `restart-language-server`, end to end: a running server is replaced, and the
/// state passes through `Starting` on the way.
#[test]
fn a_restart_replaces_a_running_server() {
    let dir = TempDir::new("restart");
    let spec = fake_server(&dir, "rust", &initialize_response("fake-analyzer"));
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust"), ServerState::is_ready).is_ready());

    servers.restart(&language("rust"));
    let state = settle(&servers, &language("rust"), ServerState::is_starting);
    assert_eq!(
        state,
        ServerState::Starting,
        "a restart goes back through Starting, never through Crashed"
    );
    assert!(
        settle(&servers, &language("rust"), ServerState::is_ready).is_ready(),
        "and comes back up"
    );
}

/// A stop is not a crash, all the way through the real client: the EOF that
/// follows `exit` must not overwrite `Stopped`.
#[test]
fn stopping_a_server_leaves_it_stopped_and_not_crashed() {
    let dir = TempDir::new("stop");
    // This fake **exits a second after it answers**, so the pipe closes while
    // the state is already `Stopped` — which is the case the rule exists for,
    // and the one a server that stayed alive would leave untested: the main
    // loop would never report EOF, and `Stopped` would survive by never being
    // challenged. Timing rather than a reply to `shutdown`, because a `sh`
    // `read` cannot reliably see a request that has no trailing newline.
    let script = dir.write("stop.frames", &initialize_response("fake-analyzer"));
    let spec = ServerSpec::new("rust", "sh")
        .with_args([
            "-c".to_owned(),
            format!("read -r _ ; sleep 0.3 ; cat {} ; sleep 1", script.display()),
        ])
        .with_ready_timeout(Duration::from_secs(10));
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust"), ServerState::is_ready).is_ready());

    servers.stop(&language("rust"));
    let stopped = settle(&servers, &language("rust"), |state| {
        matches!(state, ServerState::Stopped)
    });
    assert_eq!(stopped, ServerState::Stopped);
    // Outlast the server's own exit, so the transport reports EOF *after* the
    // stop was recorded. Without the `Stopped` rule in `ServerState::after`,
    // that EOF becomes `Crashed(Exited)` and this assertion is what says so.
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(servers.state(&language("rust")), ServerState::Stopped);
}

/// A server talks about things the client did not ask for, and **`async-lsp`'s
/// `Router` breaks the main loop by default on any notification it has no
/// handler for** — `$/`-prefixed ones excepted. `window/logMessage` is not
/// `$/`-prefixed and rust-analyzer sends it, so without the catch-all in
/// `router` a working server takes the client down as a protocol error moments
/// after reporting ready.
///
/// The fake sends exactly that, then a diagnostic, so the assertion is not just
/// "still alive" but "still *working* afterwards".
#[test]
fn a_servers_chatter_does_not_take_the_client_down() {
    let dir = TempDir::new("chatter");
    let source = dir.write("main.rs", "let x = 1;\n");
    let uri = lsp_types::Url::from_file_path(&source).expect("uri");
    let frames = format!(
        "{}{}{}",
        initialize_response("chatty"),
        frame(
            r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"server started"}}"#
        ),
        frame(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{uri}","diagnostics":[]}}}}"#
        ))
    );
    let spec = fake_server(&dir, "rust", &frames);
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.open(&language("rust"), source, "let x = 1;\n".to_owned());
    servers.attach(spec, dir.path.clone());

    assert!(settle(&servers, &language("rust"), ServerState::is_ready).is_ready());
    let deadline = Instant::now() + Duration::from_secs(20);
    while sink.actions().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        sink.actions().len(),
        1,
        "the diagnostic behind the log message still arrived"
    );
    assert!(
        servers.state(&language("rust")).is_ready(),
        "a log message is not a protocol error"
    );
}

/// `request-definition`, end to end: the question goes out, the answer comes
/// back as the file-and-span shape the host turns into an `open-file`.
///
/// The fake answers on a timer rather than on the content of the request, for
/// the reason `fake_server` gives — and the timer is comfortably after the
/// question, which the test sends as soon as the server is ready.
#[test]
fn a_definition_question_is_answered_with_a_place() {
    let dir = TempDir::new("definition");
    let source = dir.write("main.rs", "let x = 1;\n");
    let uri = lsp_types::Url::from_file_path(&source).expect("uri");
    let script = dir.write(
        "definition.frames",
        &format!(
            "{}{}",
            initialize_response("fake-analyzer"),
            // `id: 1` — `initialize` was 0, and this is the next request the
            // client sends.
            frame(&format!(
                r#"{{"jsonrpc":"2.0","id":1,"result":[{{"uri":"{uri}","range":{{"start":{{"line":0,"character":4}},"end":{{"line":0,"character":5}}}}}}]}}"#
            ))
        ),
    );
    let spec = ServerSpec::new("rust", "sh")
        .with_args([
            "-c".to_owned(),
            format!(
                "read -r _ ; sleep 0.3 ; head -c {} {} ; sleep 1 ; tail -c +{} {} ; exec sleep 30",
                initialize_response("fake-analyzer").len(),
                script.display(),
                initialize_response("fake-analyzer").len() + 1,
                script.display(),
            ),
        ])
        .with_ready_timeout(Duration::from_secs(10));

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.open(&language("rust"), source.clone(), "let x = 1;\n".to_owned());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust"), ServerState::is_ready).is_ready());

    let (sender, receiver) = std::sync::mpsc::channel();
    servers.ask(
        &language("rust"),
        Question::Definition,
        source.clone(),
        Position { line: 1, column: 5 },
        Arc::new(move |places| drop(sender.send(places))),
    );
    let places = receiver
        .recv_timeout(Duration::from_secs(20))
        .expect("exactly one answer, always");
    assert_eq!(
        places,
        vec![FileSpan {
            path: source,
            span: Some(Span {
                start: Position { line: 1, column: 5 },
                end: Position { line: 1, column: 6 },
            }),
        }]
    );
}

/// The contract that makes [`LanguageServers::ask`] usable without a timer at
/// the call site: **a question always gets exactly one answer**, even when
/// there is no server to ask.
#[test]
fn a_question_with_no_server_is_answered_with_nothing() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    let (sender, receiver) = std::sync::mpsc::channel();
    servers.ask(
        &language("rust"),
        Question::References,
        PathBuf::from("/nowhere/main.rs"),
        Position { line: 1, column: 1 },
        Arc::new(move |places| drop(sender.send(places))),
    );
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("an answer, not a silence"),
        Vec::new(),
        "no server is an answer, and the caller is told rather than left waiting"
    );
}

/// The three shapes a `definition` response comes in. A client that reads only
/// the first does nothing against half the ecosystem, silently.
#[test]
fn all_three_shapes_of_a_definition_answer_are_read() {
    let dir = TempDir::new("shapes");
    let path = dir.write("a.rs", "let x = 1;\n");
    let uri = lsp_types::Url::from_file_path(&path).expect("uri");
    let range = lsp_types::Range {
        start: lsp_types::Position {
            line: 0,
            character: 4,
        },
        end: lsp_types::Position {
            line: 0,
            character: 5,
        },
    };
    let location = lsp_types::Location {
        uri: uri.clone(),
        range,
    };
    let read = |path: &Path| fs::read_to_string(path).ok();
    let expected = vec![FileSpan {
        path: path.clone(),
        span: Some(Span {
            start: Position { line: 1, column: 5 },
            end: Position { line: 1, column: 6 },
        }),
    }];

    assert_eq!(
        locations_from_lsp(
            &lsp_types::GotoDefinitionResponse::Scalar(location.clone()),
            &read
        ),
        expected
    );
    assert_eq!(
        locations_from_lsp(
            &lsp_types::GotoDefinitionResponse::Array(vec![location]),
            &read
        ),
        expected
    );
    assert_eq!(
        locations_from_lsp(
            &lsp_types::GotoDefinitionResponse::Link(vec![lsp_types::LocationLink {
                origin_selection_range: None,
                target_uri: uri,
                target_range: range,
                target_selection_range: range,
            }]),
            &read
        ),
        expected,
        "a LocationLink is what rust-analyzer sends when link support is advertised"
    );
}

/// A language nobody attached is `NotStarted` — not an error, not a panic, and
/// not a lookup.
#[test]
fn a_language_with_no_server_is_not_started() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    assert_eq!(servers.state(&language("steel")), ServerState::NotStarted);
    assert_eq!(servers.text_of(Path::new("/nowhere")), None);
}

// ---------------------------------------------------------------------------
// The promises a review found broken
// ---------------------------------------------------------------------------

/// Asks a question and waits for the one answer `ask` promises, or [`None`]
/// when the promise was not kept.
///
/// The wait is what makes the assertion meaningful: a callback that is never
/// called and a callback called with nothing are the same *value* at the call
/// site, and only the second one is an answer.
fn asked(servers: &LanguageServers, language: &LanguageId, path: &Path) -> Option<Vec<FileSpan>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    servers.ask(
        language,
        Question::Definition,
        path.to_path_buf(),
        Position { line: 1, column: 1 },
        Arc::new(move |places| drop(sender.send(places))),
    );
    receiver.recv_timeout(Duration::from_secs(10)).ok()
}

/// **The most reachable state there is** — a blessed server that is not
/// installed — and the one the *"exactly one answer"* contract was silently
/// false in: the supervisor kept the dead task's sender, so the question went
/// into a channel with no receiver and the callback was dropped on the floor.
///
/// `a_question_with_no_server_is_answered_with_nothing` asks about a language
/// that was *never* attached, which is the other branch, and why the suite was
/// green while `gd` after a failed attach would hang its caller forever.
#[test]
fn a_question_after_a_failed_spawn_still_gets_its_one_answer() {
    let dir = TempDir::new("ask-after-spawn-failure");
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(
        ServerSpec::new("rust", "phosphor-no-such-language-server"),
        dir.path.clone(),
    );
    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    assert!(
        matches!(state.failure(), Some(Failure::Spawn(_))),
        "expected a spawn failure to ask into, got {state:?}"
    );

    let file = dir.write("main.rs", "let x = 1;\n");
    assert_eq!(
        asked(&servers, &language("rust"), &file),
        Some(Vec::new()),
        "a crashed server is an answer — nothing — and not a silence"
    );
    // And again, because the first question is also what prunes the dead
    // sender: a contract that holds once is not a contract.
    assert_eq!(
        asked(&servers, &language("rust"), &file),
        Some(Vec::new()),
        "the second question is answered too"
    );
}

/// The same promise on the other reachable path: we asked the server to stop,
/// so its task is gone, and a question that arrives afterwards is answered with
/// nothing rather than dropped.
#[test]
fn a_question_after_a_stop_still_gets_its_one_answer() {
    let dir = TempDir::new("ask-after-stop");
    let spec = fake_server(&dir, "rust", &initialize_response("fake-analyzer"));
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust"), ServerState::is_ready).is_ready());

    servers.stop(&language("rust"));
    assert_eq!(
        settle(&servers, &language("rust"), |state| matches!(
            state,
            ServerState::Stopped
        )),
        ServerState::Stopped
    );

    let file = dir.write("main.rs", "let x = 1;\n");
    assert_eq!(
        asked(&servers, &language("rust"), &file),
        Some(Vec::new()),
        "a stopped server is an answer, and the caller is told rather than left waiting"
    );
}

/// **A malformed header must not take the editor with it.**
///
/// The transport's header carries the size of the frame that follows and
/// `async-lsp` allocates it before reading a byte, so a server that declares a
/// petabyte is an allocation failure — which Rust turns into `abort()`: no
/// unwind, no `Crashed`, no editor. This test is the two-line server that does
/// it, and before the bound it failed by killing the whole test process rather
/// than by asserting.
#[test]
fn an_absurd_content_length_is_a_crash_and_not_an_abort() {
    let dir = TempDir::new("absurd-frame");
    let spec = ServerSpec::new("rust", "sh")
        .with_args([
            "-c",
            "read -r _ ; printf 'Content-Length: 999999999999999\\r\\n\\r\\n' ; exec sleep 30",
        ])
        .with_ready_timeout(Duration::from_secs(10));
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());

    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    match state.failure() {
        Some(Failure::Exited(why)) => assert!(
            why.contains("Content-Length"),
            "the reason names the frame that was refused, got {why:?}"
        ),
        other => panic!("expected the frame to be refused, got {other:?}"),
    }
    assert!(
        sink.actions().is_empty(),
        "nothing inside an unreadable frame reaches the queue"
    );
}

/// A server that answers `initialize` with a `positionEncoding` we did not
/// offer is a protocol failure, not a ready server.
///
/// Every column this module converts is a UTF-16 code unit and the client says
/// so in its `initialize`. A server replying `utf-8` — which the specification
/// forbids, having been offered only one — would have every column silently
/// wrong on the first non-ASCII line, which is the bug class the module header
/// is written against. Reading the answer is what makes the declaration a
/// contract rather than a hope.
#[test]
fn a_server_that_answers_in_another_encoding_is_a_protocol_failure() {
    let dir = TempDir::new("encoding");
    let response = frame(
        r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{"positionEncoding":"utf-8"},"serverInfo":{"name":"utf8-analyzer"}}}"#,
    );
    let spec = fake_server(&dir, "rust", &response);
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());

    let state = settle(&servers, &language("rust"), |state| {
        state.failure().is_some()
    });
    match state.failure() {
        Some(Failure::Protocol(why)) => assert!(
            why.contains("utf-8"),
            "the reason names the encoding the server chose, got {why:?}"
        ),
        other => panic!("expected a protocol failure, got {other:?}"),
    }
}

/// The other half of that rule: a server that *agrees*, out loud, is ready.
/// `a_server_reports_ready_and_its_diagnostics_reach_the_queue_as_an_action`
/// covers the third case — a server that says nothing, which the specification
/// makes UTF-16 by default.
#[test]
fn a_server_that_agrees_about_the_encoding_is_ready() {
    let dir = TempDir::new("encoding-agrees");
    let response = frame(
        r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{"positionEncoding":"utf-16"},"serverInfo":{"name":"utf16-analyzer"}}}"#,
    );
    let spec = fake_server(&dir, "rust", &response);
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(
        settle(&servers, &language("rust"), ServerState::is_ready).is_ready(),
        "the encoding we asked for is not a reason to refuse a server"
    );
}
