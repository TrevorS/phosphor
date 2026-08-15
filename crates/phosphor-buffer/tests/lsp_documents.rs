//! `T038`'s document sync, and the `T038`/`T039` lookups.
//!
//! `T036` shipped `didOpen` and nothing after it, and recorded why that is not
//! good enough: *"a completion request against a stale server copy returns
//! completions for text the user is no longer looking at."* This file is the
//! other half — `didChange` with a rising version, `didClose`, and the three
//! requests the passive float draws.
//!
//! Same fake-server technique as `tests/lsp.rs` (see its header for why a shell
//! script is a legitimate language server), with one addition: **the server
//! keeps its stdin.** `cat >> log` after the canned frames turns everything the
//! client says into a file, which is the only way to assert that a notification
//! was *sent* — a notification has no reply, so nothing else can observe it.
//!
//! The conversions are pure and are stated as examples here rather than as
//! round trips through a process.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use phosphor_buffer::lsp::{
    Completion, Insight, LanguageServers, Lookup, Post, Question, ServerSpec, ServerState,
    change_event, completions_from_lsp, hover_prose, lsp_types, signature_from_lsp, sync_kind,
    unwatched,
};
use phosphor_core::action::Action;
use phosphor_core::request::{LanguageId, Position};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A directory that removes itself. Same shape as `tests/lsp.rs`'s, and for the
/// same reason: no `tempfile` dependency for a test.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "phosphor-docs-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, contents).expect("write");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

/// An `initialize` reply that declares `textDocumentSync`, which is the field
/// `T038` reads: `2` is `Incremental`, `1` is `Full`, `0` is `None`.
fn initialize_response(sync: u8) -> String {
    frame(&format!(
        r#"{{"jsonrpc":"2.0","id":0,"result":{{"capabilities":{{"textDocumentSync":{sync}}},"serverInfo":{{"name":"scribe","version":"0.1"}}}}}}"#
    ))
}

/// A server that answers `initialize` and then writes everything the client
/// says into `log`.
///
/// **The log is appended, not truncated**, so a restart's second process writes
/// under the first one's transcript and the whole conversation — both
/// processes' — is one file in order. The restart test below is what needs
/// that; every other test here spawns one process and cannot tell.
fn logging_server(dir: &TempDir, sync: u8) -> (ServerSpec, PathBuf) {
    let frames = dir.write("frames", &initialize_response(sync));
    let log = dir.path.join("stdin.log");
    let spec = ServerSpec::new("rust", "sh")
        .with_args([
            "-c".to_owned(),
            format!(
                "read -r _ ; sleep 0.3 ; cat {} ; cat >> {}",
                frames.display(),
                log.display()
            ),
        ])
        .with_ready_timeout(Duration::from_secs(10));
    (spec, log)
}

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
}

fn language(name: &str) -> LanguageId {
    LanguageId(name.to_owned())
}

fn settle(servers: &LanguageServers, language: &LanguageId) -> ServerState {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let state = servers.state(language);
        if state.is_ready() || Instant::now() > deadline {
            return state;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Polls the log until it contains `want`, then hands the whole thing back.
fn logged(log: &PathBuf, want: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let text = fs::read_to_string(log).unwrap_or_default();
        if text.contains(want) || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// didChange / didClose
// ---------------------------------------------------------------------------

/// **The whole of `T038`'s document half, observed on the wire.** Two edits
/// after the open, each with the version one higher than the last, because a
/// server that sees the same version twice is entitled to ignore the second.
#[test]
fn every_change_is_sent_with_the_next_version() {
    let dir = TempDir::new("versions");
    let source = dir.write("main.rs", "let x = 1;\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    // Attached first, and only then told about the file: a `didOpen` for a
    // language with no server running has nowhere to go.
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    servers.open(&language("rust"), source.clone(), "let x = 1;\n".to_owned());
    servers.change(
        &language("rust"),
        source.clone(),
        "let xy = 1;\n".to_owned(),
    );
    servers.change(
        &language("rust"),
        source.clone(),
        "let xyz = 1;\n".to_owned(),
    );

    let text = logged(&log, "let xyz = 1;");
    assert!(text.contains("textDocument/didOpen"), "{text}");
    assert_eq!(
        text.matches("textDocument/didChange").count(),
        2,
        "two edits, two notifications:\n{text}"
    );
    assert!(text.contains(r#""version":2"#), "{text}");
    assert!(text.contains(r#""version":3"#), "{text}");
    // The server declared `Full`, so the change carries no range at all.
    assert!(!text.contains(r#""range""#), "{text}");
}

/// A server that syncs incrementally is told the same thing in its own shape:
/// one edit spanning the whole previous document. Sending it a range-less
/// change would be off-specification, however widely it is tolerated.
#[test]
fn an_incremental_server_gets_a_range_over_the_whole_previous_document() {
    let dir = TempDir::new("incremental");
    let source = dir.write("main.rs", "one\ntwo\n");
    let (spec, log) = logging_server(&dir, 2);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    servers.open(&language("rust"), source.clone(), "one\ntwo\n".to_owned());
    servers.change(&language("rust"), source, "one\ntwo\nthree\n".to_owned());

    let text = logged(&log, "textDocument/didChange");
    // `"one\ntwo\n"` is three lines under `split('\n')` — the third is empty —
    // so the end of the document is line 2, character 0. The fields come out
    // in `serde`'s alphabetical order, which is what is on the wire.
    assert!(
        text.contains(
            r#""range":{"end":{"character":0,"line":2},"start":{"character":0,"line":0}}"#
        ),
        "{text}"
    );
}

/// `didClose`, and the record going with it: after a close the client has
/// disclaimed the file, so it must not keep converting columns against a copy
/// it no longer owns.
#[test]
fn closing_tells_the_server_and_forgets_the_text() {
    let dir = TempDir::new("close");
    let source = dir.write("main.rs", "let x = 1;\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    servers.open(&language("rust"), source.clone(), "let x = 1;\n".to_owned());
    assert_eq!(servers.text_of(&source).as_deref(), Some("let x = 1;\n"));
    servers.close(&language("rust"), &source);

    assert_eq!(servers.text_of(&source), None, "the record goes too");
    let text = logged(&log, "textDocument/didClose");
    assert!(text.contains("textDocument/didClose"), "{text}");
}

/// **A restarted server is told about the documents the client is holding**,
/// before it is told anything else.
///
/// The failure without this is silent and total: `restart-language-server`
/// spawned a new process, and the first thing it heard about the open file was
/// a `didChange` at version 3 for a document it had never been sent. Every
/// completion, signature and hover after that is answered against a document
/// the server does not have, and nothing on screen says so.
///
/// The version continues rather than restarting, because the client's record
/// does: a `didOpen` at 1 after the client had reached 3 would make the next
/// edit look stale.
#[test]
fn restarting_reopens_every_document_that_language_holds() {
    let dir = TempDir::new("restart");
    let source = dir.write("main.rs", "let x = 1;\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    servers.open(&language("rust"), source.clone(), "let x = 1;\n".to_owned());
    servers.change(
        &language("rust"),
        source.clone(),
        "let xy = 1;\n".to_owned(),
    );
    let first = logged(&log, r#""version":2"#);
    assert_eq!(first.matches("textDocument/didOpen").count(), 1, "{first}");

    servers.restart(&language("rust"));
    // Waited for rather than raced: with the edit sent immediately the replay
    // would carry it already, and this test would be asserting that the client
    // is fast rather than that the server is told.
    let reopened = logged(&log, "let xy = 1;\\n\",");
    assert_eq!(
        reopened.matches("textDocument/didOpen").count(),
        2,
        "the restarted process is told about the file again:\n{reopened}"
    );
    // The replayed open carries the version the client is on — 2, after one
    // edit — and the text as it is now, not as it was when the buffer opened.
    let second = reopened
        .rfind("textDocument/didOpen")
        .expect("the second didOpen");
    assert!(reopened[second..].contains(r#""version":2"#), "{reopened}");
    assert!(reopened[second..].contains("let xy = 1;"), "{reopened}");

    servers.change(&language("rust"), source, "let xyz = 1;\n".to_owned());
    let text = logged(&log, r#""version":3"#);
    let changed = text
        .rfind("textDocument/didChange")
        .expect("the last change");
    assert!(second < changed, "the open comes first:\n{text}");
    assert!(text[changed..].contains(r#""version":3"#), "{text}");
}

/// The same hole from the other side: the editor opens a buffer and *then*
/// discovers which server serves it, so an attach has to replay too.
#[test]
fn attaching_after_a_file_is_open_tells_the_new_server_about_it() {
    let dir = TempDir::new("attach-after-open");
    let source = dir.write("main.rs", "let x = 1;\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.open(&language("rust"), source, "let x = 1;\n".to_owned());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    let text = logged(&log, "textDocument/didOpen");
    assert!(text.contains("let x = 1;"), "{text}");
}

/// And only that language's: a python server has no business hearing about a
/// rust buffer, and the document map is keyed by path.
#[test]
fn a_replay_carries_only_the_languages_own_documents() {
    let dir = TempDir::new("replay-scope");
    let rust = dir.write("main.rs", "let x = 1;\n");
    let python = dir.write("main.py", "x = 1\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.open(&language("rust"), rust, "let x = 1;\n".to_owned());
    servers.open(&language("python"), python, "x = 1\n".to_owned());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    let text = logged(&log, "textDocument/didOpen");
    assert!(text.contains("main.rs"), "{text}");
    assert!(
        !text.contains("main.py"),
        "python is not this server's:\n{text}"
    );
}

/// The text the conversion uses is the *current* one, the moment `change`
/// returns — the same promise `open` makes and for the same reason: a server
/// can publish diagnostics against the new text before our own next message
/// lands.
#[test]
fn the_recorded_text_is_current_the_moment_change_returns() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    let path = PathBuf::from("/nowhere/main.rs");
    servers.open(&language("rust"), path.clone(), "let x = 1;\n".to_owned());
    servers.change(&language("rust"), path.clone(), "let 名 = 1;\n".to_owned());
    assert_eq!(servers.text_of(&path).as_deref(), Some("let 名 = 1;\n"));
}

/// A change to a file nobody opened records it rather than dropping the text
/// the conversion needs.
#[test]
fn a_change_to_an_unopened_file_records_it() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    let path = PathBuf::from("/nowhere/other.rs");
    servers.change(&language("rust"), path.clone(), "fn main() {}\n".to_owned());
    assert_eq!(servers.text_of(&path).as_deref(), Some("fn main() {}\n"));
}

/// **And it reaches the server as the `didOpen` it should have been.**
/// `didChange` is defined only for an open document, and `close` has always
/// guarded the same case in the other direction — it sends nothing for a path
/// it was not holding — so the asymmetry was a choice this code made in one
/// direction only.
#[test]
fn a_change_to_an_unopened_file_is_sent_as_the_open_it_should_have_been() {
    let dir = TempDir::new("change-first");
    let source = dir.write("main.rs", "let x = 1;\n");
    let (spec, log) = logging_server(&dir, 1);

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    // No `open` at all.
    servers.change(&language("rust"), source, "let x = 2;\n".to_owned());

    let text = logged(&log, "let x = 2;");
    assert!(text.contains("textDocument/didOpen"), "{text}");
    assert!(
        !text.contains("textDocument/didChange"),
        "a didChange for a document the server has no copy of:\n{text}"
    );
}

/// Closing a file nobody opened is a no-op. The editor closes buffers it opened
/// before any server attached.
#[test]
fn closing_an_unopened_file_is_not_an_error() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.close(&language("rust"), &PathBuf::from("/nowhere/never.rs"));
    assert_eq!(servers.text_of(&PathBuf::from("/nowhere/never.rs")), None);
}

// ---------------------------------------------------------------------------
// The target file the client never opened — `T036`'s recorded gap
// ---------------------------------------------------------------------------

/// **`T036` left this open and `T038` closed it.** Go-to-definition lands in a
/// file the client has never opened, and its columns arrive in UTF-16 units;
/// converting them against `""` is exact for ASCII and off by one per astral
/// character otherwise. The target here is `let 🙂 = 1;` and the answer names
/// unit 6 — the space after the emoji.
///
/// **Column 6 with the file read, column 7 without it**, which is what makes
/// this a test rather than a demonstration: revert the pre-load in `answer` and
/// the assertion below moves by exactly the number of astral characters ahead
/// of the column.
#[test]
fn a_definition_in_an_unopened_file_converts_against_that_files_text() {
    let dir = TempDir::new("target");
    let source = dir.write("main.rs", "let x = 1;\n");
    // Never handed to `LanguageServers::open`: the client learns this file
    // exists from the server's answer and from nowhere else.
    let target = dir.write("target.rs", "let 🙂 = 1;\n");
    let uri = lsp_types::Url::from_file_path(&target).expect("uri");

    let hello = initialize_response(1);
    let answer = frame(&format!(
        r#"{{"jsonrpc":"2.0","id":1,"result":[{{"uri":"{uri}","range":{{"start":{{"line":0,"character":6}},"end":{{"line":0,"character":7}}}}}}]}}"#
    ));
    let script = dir.write("definition.frames", &format!("{hello}{answer}"));
    let spec = ServerSpec::new("rust", "sh")
        .with_args([
            "-c".to_owned(),
            format!(
                "read -r _ ; sleep 0.3 ; head -c {} {} ; sleep 1 ; tail -c +{} {} ; exec sleep 30",
                hello.len(),
                script.display(),
                hello.len() + 1,
                script.display(),
            ),
        ])
        .with_ready_timeout(Duration::from_secs(10));

    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    servers.open(&language("rust"), source.clone(), "let x = 1;\n".to_owned());
    servers.attach(spec, dir.path.clone());
    assert!(settle(&servers, &language("rust")).is_ready());

    let (sender, receiver) = std::sync::mpsc::channel();
    servers.ask(
        &language("rust"),
        Question::Definition,
        source,
        Position { line: 1, column: 5 },
        Arc::new(move |places| drop(sender.send(places))),
    );
    let places = receiver
        .recv_timeout(Duration::from_secs(20))
        .expect("exactly one answer, always");
    let span = places
        .first()
        .expect("one place")
        .span
        .expect("a span")
        .start;
    assert_eq!(
        span,
        Position { line: 1, column: 6 },
        "🙂 is two UTF-16 units and one column; against \"\" this reads 7"
    );
}

// ---------------------------------------------------------------------------
// The change event, as a function
// ---------------------------------------------------------------------------

#[test]
fn a_full_server_is_told_the_document_and_nothing_else() {
    let change =
        change_event(lsp_types::TextDocumentSyncKind::FULL, "new", "old").expect("a change");
    assert_eq!(change.range, None);
    assert_eq!(change.text, "new");
}

#[test]
fn a_server_that_wants_no_sync_is_told_nothing() {
    assert!(change_event(lsp_types::TextDocumentSyncKind::NONE, "new", "old").is_none());
}

/// The range is measured in **UTF-16 code units**, like every other position in
/// the protocol. An emoji on the last line is two units, and a range that
/// counted characters would leave half of it behind.
#[test]
fn the_incremental_range_counts_utf16_units_on_the_last_line() {
    let change = change_event(lsp_types::TextDocumentSyncKind::INCREMENTAL, "x", "a\n🙂b")
        .expect("a change");
    let range = change.range.expect("a range");
    assert_eq!(range.start, lsp_types::Position::new(0, 0));
    assert_eq!(range.end, lsp_types::Position::new(1, 3), "🙂 is two units");
}

/// An empty previous document is an empty range — the shape a change against a
/// document the server has, but which is empty, takes.
#[test]
fn an_empty_previous_document_is_an_empty_range() {
    let change =
        change_event(lsp_types::TextDocumentSyncKind::INCREMENTAL, "x", "").expect("a change");
    assert_eq!(
        change.range.expect("a range").end,
        lsp_types::Position::new(0, 0)
    );
}

#[test]
fn a_server_that_says_nothing_about_sync_is_treated_as_full() {
    assert_eq!(sync_kind(None), lsp_types::TextDocumentSyncKind::FULL);
    assert_eq!(
        sync_kind(Some(&lsp_types::TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions {
                change: Some(lsp_types::TextDocumentSyncKind::INCREMENTAL),
                ..lsp_types::TextDocumentSyncOptions::default()
            }
        ))),
        lsp_types::TextDocumentSyncKind::INCREMENTAL
    );
}

proptest! {
    /// **The law an incremental change has to keep**: its range starts at the
    /// top of the document and ends past its last unit, whatever is in it —
    /// astral characters, CRLF, no trailing newline, nothing at all. A range
    /// that fell short would leave a tail of the old document behind, and the
    /// server's copy would diverge silently from the buffer's.
    #[test]
    fn an_incremental_range_covers_every_unit_of_the_previous_document(
        previous in "(\\PC|\n|\r\n|🙂|名){0,60}",
    ) {
        let change = change_event(
            lsp_types::TextDocumentSyncKind::INCREMENTAL,
            "replaced",
            previous.as_str(),
        )
        .expect("a change");
        let range = change.range.expect("a range");
        prop_assert_eq!(range.start, lsp_types::Position::new(0, 0));

        let lines: Vec<&str> = previous.split('\n').collect();
        let last = u32::try_from(lines.len() - 1).expect("a line");
        prop_assert_eq!(range.end.line, last);
        let units: u32 = lines[lines.len() - 1]
            .chars()
            .map(|c| u32::try_from(c.len_utf16()).expect("units"))
            .sum();
        prop_assert_eq!(range.end.character, units);
    }
}

// ---------------------------------------------------------------------------
// The three lookups
// ---------------------------------------------------------------------------

fn item(label: &str) -> lsp_types::CompletionItem {
    lsp_types::CompletionItem {
        label: label.to_owned(),
        ..lsp_types::CompletionItem::default()
    }
}

/// Servers answer with a bare array or with a `CompletionList`, and
/// rust-analyzer sends the second. A client that read only one would be blind
/// to half the ecosystem — the same shape `locations_from_lsp` already had to
/// learn.
#[test]
fn a_completion_response_is_read_in_both_of_its_shapes() {
    let array = lsp_types::CompletionResponse::Array(vec![item("default()")]);
    let list = lsp_types::CompletionResponse::List(lsp_types::CompletionList {
        is_incomplete: true,
        items: vec![item("default()")],
    });
    assert_eq!(completions_from_lsp(&array), completions_from_lsp(&list));
    assert_eq!(completions_from_lsp(&array).len(), 1);
}

/// `7c`'s row: a label, the detail column, documentation, and what would be
/// typed. `insertText` wins over the label when the server sends one.
#[test]
fn a_completion_carries_the_columns_7c_draws() {
    let response = lsp_types::CompletionResponse::Array(vec![lsp_types::CompletionItem {
        label: "default()".to_owned(),
        detail: Some("fn() -> RetryPolicy".to_owned()),
        documentation: Some(lsp_types::Documentation::String(
            "Returns the policy with 3 attempts.\n\n200ms base.".to_owned(),
        )),
        insert_text: Some("default()".to_owned()),
        ..lsp_types::CompletionItem::default()
    }]);
    assert_eq!(
        completions_from_lsp(&response),
        vec![Completion {
            label: "default()".to_owned(),
            detail: Some("fn() -> RetryPolicy".to_owned()),
            // The blank line between the two sentences is dropped: a float's
            // height is its content, and a blank row is not content.
            documentation: vec![
                "Returns the policy with 3 attempts.".to_owned(),
                "200ms base.".to_owned(),
            ],
            insert: "default()".to_owned(),
            // Neither field was sent, so both are the label — the
            // specification's own rule for each (`narrow`).
            filter: "default()".to_owned(),
            sort: "default()".to_owned(),
        }]
    );
}

#[test]
fn a_completion_with_no_insert_text_inserts_its_label() {
    let response = lsp_types::CompletionResponse::Array(vec![item("deserialize")]);
    assert_eq!(completions_from_lsp(&response)[0].insert, "deserialize");
}

fn parameter(label: lsp_types::ParameterLabel) -> lsp_types::ParameterInformation {
    lsp_types::ParameterInformation {
        label,
        documentation: None,
    }
}

/// **The UTF-16 trap, one field over from the one the module header is about.**
/// `LabelOffsets` are code units into the signature label, so a client that
/// used them as character indices highlights the wrong span of any signature
/// with a non-ASCII identifier in it.
#[test]
fn a_parameter_offset_is_converted_off_utf16() {
    let label = "fn 送る(🙂: A, body: B)";
    // `body` starts at character 14 and at UTF-16 unit 15 — the emoji is two
    // units and one character.
    let units_before_body: u32 = label
        .chars()
        .take_while(|&c| c != 'b')
        .map(|c| u32::try_from(c.len_utf16()).expect("units"))
        .sum();
    let chars_before_body = label.chars().take_while(|&c| c != 'b').count();
    assert_ne!(
        usize::try_from(units_before_body).expect("units"),
        chars_before_body,
        "the fixture has to actually disagree, or this test proves nothing"
    );

    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: label.to_owned(),
            documentation: None,
            parameters: Some(vec![parameter(lsp_types::ParameterLabel::LabelOffsets([
                units_before_body,
                units_before_body + 7,
            ]))]),
            active_parameter: Some(0),
        }],
        active_signature: Some(0),
        active_parameter: None,
    };
    let signature = signature_from_lsp(&help).expect("a signature");
    assert_eq!(signature.label, label);
    assert_eq!(
        signature.active,
        Some((chars_before_body, chars_before_body + 7))
    );
}

/// The `Simple` shape, which is the parameter's own text and has to be found.
#[test]
fn a_simple_parameter_label_is_located_in_the_signature() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "fn fetch_json(url: &str) -> Value".to_owned(),
            documentation: Some(lsp_types::Documentation::String("one request".to_owned())),
            parameters: Some(vec![parameter(lsp_types::ParameterLabel::Simple(
                "url: &str".to_owned(),
            ))]),
            active_parameter: None,
        }],
        active_signature: None,
        active_parameter: Some(0),
    };
    let signature = signature_from_lsp(&help).expect("a signature");
    assert_eq!(signature.active, Some((14, 23)));
    assert_eq!(signature.documentation, vec!["one request".to_owned()]);
}

/// LSP 3.16 put the active parameter on the *signature* precisely because the
/// top-level field cannot describe an overload set. A client that read only the
/// top-level one highlights the wrong argument on every server that sets both.
#[test]
fn the_signatures_own_active_parameter_wins() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "f(a, b)".to_owned(),
            documentation: None,
            parameters: Some(vec![
                parameter(lsp_types::ParameterLabel::Simple("a".to_owned())),
                parameter(lsp_types::ParameterLabel::Simple("b".to_owned())),
            ]),
            active_parameter: Some(1),
        }],
        active_signature: Some(0),
        active_parameter: Some(0),
    };
    assert_eq!(
        signature_from_lsp(&help).expect("a signature").active,
        Some((5, 6))
    );
}

#[test]
fn no_signatures_is_no_signature() {
    let help = lsp_types::SignatureHelp {
        signatures: Vec::new(),
        active_signature: Some(0),
        active_parameter: Some(0),
    };
    assert!(signature_from_lsp(&help).is_none());
}

#[test]
fn hover_reads_all_three_of_its_shapes() {
    let markup = lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
        kind: lsp_types::MarkupKind::Markdown,
        value: "```rust\nfn f()\n```\n\na function".to_owned(),
    });
    assert_eq!(
        hover_prose(&markup),
        vec!["```rust", "fn f()", "```", "a function"]
    );

    let scalar =
        lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String("plain".to_owned()));
    assert_eq!(hover_prose(&scalar), vec!["plain".to_owned()]);

    let array = lsp_types::HoverContents::Array(vec![
        lsp_types::MarkedString::LanguageString(lsp_types::LanguageString {
            language: "rust".to_owned(),
            value: "fn f()".to_owned(),
        }),
        lsp_types::MarkedString::String("a function".to_owned()),
    ]);
    assert_eq!(hover_prose(&array), vec!["fn f()", "a function"]);
}

/// The contract that makes [`LanguageServers::look_up`] usable from a
/// keystroke without a timer at the call site: **exactly one answer**, and
/// `Nothing` when there is no server to ask.
#[test]
fn a_lookup_with_no_server_is_answered_with_nothing() {
    let sink = Sink::default();
    let servers = LanguageServers::start(sink.post(), unwatched());
    let (sender, receiver) = std::sync::mpsc::channel();
    servers.look_up(
        &language("rust"),
        Lookup::Completion,
        PathBuf::from("/nowhere/main.rs"),
        Position { line: 1, column: 1 },
        Arc::new(move |insight| drop(sender.send(insight))),
    );
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("exactly one answer, always"),
        Insight::Nothing
    );
}

/// And when the client is dropped mid-flight, which is the path
/// `Answer`'s `Drop` exists for.
#[test]
fn a_lookup_outliving_its_client_is_still_answered() {
    let sink = Sink::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    {
        let servers = LanguageServers::start(sink.post(), unwatched());
        servers.look_up(
            &language("rust"),
            Lookup::Hover,
            PathBuf::from("/nowhere/main.rs"),
            Position { line: 1, column: 1 },
            Arc::new(move |insight| drop(sender.send(insight))),
        );
    }
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("exactly one answer, always"),
        Insight::Nothing
    );
}
