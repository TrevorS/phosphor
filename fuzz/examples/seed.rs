//! Writes `fuzz/seeds/<target>/` from real inputs the repo already has.
//!
//! Not a fuzz target — a normal binary, run by `scripts/fuzz.sh seed`, and safe
//! to run at any time: it is idempotent and writes only under `fuzz/seeds/`.
//!
//! # Why a generator rather than checked-in blobs alone
//!
//! A fuzzer starting from an empty corpus spends its first hour rediscovering
//! the file format — `PHOSJRNL`, a 16-byte header, a `u32` length and a `u32`
//! CRC — and never gets past it, because a CRC-32 is not a thing coverage
//! feedback can solve. The corpus is where the format comes from.
//!
//! The seeds are also committed, so they are reviewable. What this binary adds
//! is that they are *derived*: the journals are written by the real writer
//! (`Journal::append` computes its own checksums, so nothing here duplicates
//! the framing), the key notation is every string literal in `runtime/*.scm`,
//! and the themes are the shipped files copied verbatim. Regenerate after
//! changing the format and the corpus follows; a hand-built blob would rot
//! silently and the fuzzer would keep reporting green over a format it could no
//! longer parse.
//!
//! The growing corpus (`fuzz/corpus/`) is *not* committed — `fuzz/.gitignore`
//! says why.

use std::fs;
use std::path::{Path, PathBuf};

use phosphor_core::input::key::parse_seq;
use phosphor_core::journal::{
    Journal, Stream, UndoLog,
    undo::{Caret, CharRange, Edit, Record},
};

/// How long a `runtime/*.scm` string literal may be, in keys, to be seeded into
/// `key_notation`'s corpus. See [`key_notation_seeds`].
const MAX_SEED_KEYS: usize = 8;

fn repo_root() -> PathBuf {
    // The fuzz crate is one level under the repo root, and `CARGO_MANIFEST_DIR`
    // is absolute — so this works from any cwd, which matters because
    // `scripts/fuzz.sh` runs cargo from `fuzz/`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the fuzz crate sits under the repo root")
        .to_path_buf()
}

fn seed_dir(target: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("seeds")
        .join(target);
    fs::create_dir_all(&dir).expect("seeds/ is writable");
    dir
}

fn write(target: &str, name: &str, bytes: &[u8]) {
    let path = seed_dir(target).join(name);
    fs::write(&path, bytes).expect("a seed file is writable");
    println!("  {}  ({} bytes)", path.display(), bytes.len());
}

/// A caret with no selection.
fn at(offset: usize) -> Caret {
    Caret {
        offset,
        selection: None,
    }
}

/// Four histories, chosen to put a different record tag in each seed.
///
/// The tags are what a mutation has to discover: `Origin`, `Base`, `Node`,
/// `Cursor`, `Redo` and `Saved` are six leading varints, and a corpus holding
/// only `Node` teaches the fuzzer one of them.
fn histories() -> Vec<(&'static str, Vec<Record>)> {
    let insert = |offset: usize, text: &str| Edit {
        at: offset,
        removed: String::new(),
        inserted: text.to_owned(),
    };
    let node = |id, parent, edits, before, after| Record::Node {
        id,
        parent,
        edits,
        before,
        after,
    };

    vec![
        // Nothing but a header. The reader's create path, and the shortest
        // input that is a valid journal.
        ("empty", Vec::new()),
        // The ordinary session: a file, two committed groups, a save point.
        (
            "linear",
            vec![
                Record::Origin {
                    path: "/tmp/main.rs".to_owned(),
                },
                node(1, 0, vec![insert(0, "fn main() {}\n")], at(0), at(13)),
                node(2, 1, vec![insert(12, "\n    todo!()")], at(13), at(24)),
                Record::Saved { node: Some(2) },
            ],
        ),
        // A branch: undo out of node 2, edit again, and the redo fix-up a
        // snapshot writes. This is the record stream compaction emits, and the
        // only place `Redo` appears.
        (
            "branched",
            vec![
                Record::Origin {
                    path: "/tmp/notes.md".to_owned(),
                },
                node(1, 0, vec![insert(0, "one")], at(0), at(3)),
                node(2, 1, vec![insert(3, " two")], at(3), at(7)),
                Record::Cursor { to: 1 },
                node(
                    3,
                    1,
                    vec![Edit {
                        at: 0,
                        removed: "one".to_owned(),
                        inserted: "ONE".to_owned(),
                    }],
                    Caret {
                        offset: 0,
                        selection: Some(CharRange { start: 0, end: 3 }),
                    },
                    at(3),
                ),
                Record::Redo { node: 1, child: 2 },
                Record::Saved { node: None },
            ],
        ),
        // A truncating compaction: the only stream with a `Base`, and the one
        // whose root text is not implicit.
        (
            "based",
            vec![
                Record::Origin {
                    path: "/tmp/lib.rs".to_owned(),
                },
                Record::Base {
                    text: "pub fn answer() -> u32 {\n    42\n}\n".to_owned(),
                },
                node(1, 0, vec![insert(33, "\n")], at(33), at(34)),
                Record::Cursor { to: 0 },
            ],
        ),
    ]
}

/// Real journal files, plus the two damaged shapes a crash produces.
fn journal_open_seeds(scratch: &Path) {
    println!("seeds/journal_open:");
    for (name, records) in histories() {
        let path = scratch.join(format!("{name}.journal"));
        let _ = fs::remove_file(&path);
        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in records {
            log.append(record).expect("a writer's record is appendable");
        }
        log.sync().expect("fsync");
        drop(log);
        let bytes = fs::read(&path).expect("the journal is readable");

        // The torn tail and the zero-run are the two shapes `journal.rs`'s
        // header names. Seeding them rather than waiting for a mutation to
        // produce one costs two files and saves the fuzzer the discovery.
        if bytes.len() > 24 {
            let cut = bytes.len() - 5;
            write("journal_open", &format!("{name}-torn"), &bytes[..cut]);
            let mut zeroed = bytes[..cut].to_vec();
            zeroed.extend_from_slice(&[0; 5]);
            write("journal_open", &format!("{name}-zeros"), &zeroed);
        }
        write("journal_open", name, &bytes);
    }
}

/// The same records as `[len: u8][payload]` runs — `journal_records`'s input.
fn journal_records_seeds(scratch: &Path) {
    println!("seeds/journal_records:");
    for (name, records) in histories() {
        let path = scratch.join(format!("{name}.records"));
        let _ = fs::remove_file(&path);
        let (mut log, _) = UndoLog::open(&path).expect("a fresh journal opens");
        for record in records {
            log.append(record).expect("appendable");
        }
        log.sync().expect("fsync");
        drop(log);

        // Read the payloads back through the reader rather than re-encoding
        // them here: the seed is then exactly what the framing hands a decoder.
        let (_, payloads, _) = Journal::open(&path, Stream::UNDO).expect("it reopens");
        let mut out = Vec::new();
        for payload in &payloads {
            let Ok(len) = u8::try_from(payload.len()) else {
                // A payload past 255 cannot be spelled in this target's input
                // format. None of the histories above produces one; if one ever
                // does, the seed is short rather than wrong.
                continue;
            };
            out.push(len);
            out.extend_from_slice(payload);
        }
        write("journal_records", name, &out);
    }
}

/// The string literals in `runtime/*.scm` that could be a key sequence.
///
/// Every literal, filtered — not every literal, and not only the ones in a
/// `keymap-set!` position. Recognising that position means parsing Scheme;
/// taking all 374 of them means seeding a corpus with capability names and
/// paragraphs of Scheme docstrings, which is three hundred redundant starting
/// points for a parser that dispatches on one character at a time.
///
/// [`MAX_SEED_KEYS`] is the filter, and it is the parser's own judgement rather
/// than a length in bytes: a string is kept if `parse_seq` reads it as a short
/// sequence. `"gsib"` and `"<C-w>v"` stay; a sentence does not.
fn key_notation_seeds(root: &Path) {
    println!("seeds/key_notation:");
    let mut seen: Vec<String> = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(root.join("runtime"))
        .expect("runtime/ exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "scm"))
        .collect();
    entries.sort();

    for path in entries {
        let source = fs::read_to_string(&path).expect("a runtime file is readable");
        // A hand-rolled scan rather than a regex: this is a two-state machine
        // and the crate has no regex dependency to borrow.
        let mut chars = source.chars();
        while let Some(ch) = chars.next() {
            if ch != '"' {
                continue;
            }
            let mut literal = String::new();
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                literal.push(ch);
            }
            let keys = parse_seq(&literal).map_or(usize::MAX, |keys| keys.len());
            if !literal.is_empty() && keys <= MAX_SEED_KEYS && !seen.contains(&literal) {
                seen.push(literal);
            }
        }
    }

    // Plus the shapes `parse_seq`'s own header calls out, in case no runtime
    // file happens to spell one: the leader, a bracketed key, the unclosed
    // bracket that cost `.` its correctness, and the `S`/`P`/`C` collision
    // `notation_of` respells.
    for extra in ["SPC f", "<C-w>v", "<<", "<nope>", "SPC<esc>", "]u", "3dd"] {
        if !seen.iter().any(|s| s == extra) {
            seen.push(extra.to_owned());
        }
    }

    for (i, literal) in seen.iter().enumerate() {
        write("key_notation", &format!("{i:03}"), literal.as_bytes());
    }
}

/// The CSV fixtures, each prefixed with the delimiter byte the target reads.
///
/// `crates/phosphor-buffer/tests/fixtures/csv/` is already the case list —
/// `tests/csv.rs` asserts what every one of those files parses to — so the
/// corpus is that list rather than a second one that drifts away from it. The
/// prefix is `csv_parse`'s input framing: byte 0 is the delimiter, so a `.tsv`
/// fixture seeds a tab and the fuzzer starts from a corpus that already knows
/// the delimiter is a parameter.
///
/// Plus [`EXTRA_CSV_SEEDS`] — malformed one-liners short enough that a mutation
/// would have to be lucky to rediscover them inside a 200-byte file.
fn csv_parse_seeds(root: &Path) {
    println!("seeds/csv_parse:");
    let dir = root.join("crates/phosphor-buffer/tests/fixtures/csv");
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("the csv fixture directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .expect("a fixture has a name")
            .to_string_lossy()
            .into_owned();
        let delimiter = if name.ends_with(".tsv") { b'\t' } else { b',' };
        let mut bytes = vec![delimiter];
        bytes.extend_from_slice(&fs::read(&path).expect("a fixture is readable"));
        write("csv_parse", &name, &bytes);
    }
    for (name, body) in EXTRA_CSV_SEEDS {
        let mut bytes = vec![b','];
        bytes.extend_from_slice(body.as_bytes());
        write("csv_parse", name, &bytes);
    }
}

/// The pathological one-liners, each named for what it is.
///
/// Every entry is a branch of `quoted_field_at` or a boundary of `parse`.
/// `three-quotes` is the shape that breaks a scanner looking two bytes ahead
/// instead of consuming pairs; `blank-lines` is the difference between one
/// trailing terminator and two, which is the whole of `to_csv`'s inverse law.
const EXTRA_CSV_SEEDS: [(&str, &str); 8] = [
    ("bare-quote", "\""),
    ("three-quotes", "\"\"\""),
    ("unterminated", "a,\"never closed"),
    ("tail-after-quote", "\"ab\"cd,x"),
    ("blank-lines", "a\n\n\n"),
    ("crlf-mix", "a,b\r\nc,d\ne,f\r\n"),
    ("lone-cr", "a\rb,c"),
    ("all-delimiters", ",\t;|\n,\t;|"),
];

/// The shipped themes, verbatim.
fn theme_load_seeds(root: &Path) {
    println!("seeds/theme_load:");
    let dir = root.join("crates/phosphor-ui/themes");
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("the themes directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "theme"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .expect("a theme file has a name")
            .to_string_lossy()
            .into_owned();
        let bytes = fs::read(&path).expect("a theme file is readable");
        write("theme_load", &name, &bytes);
    }
}

/// One LSP frame: the header a server writes, and the body it counted.
///
/// The length comes from the body rather than from a literal for
/// `journal_records`'s reason — a hand-written count is a second implementation
/// of the framing, and getting it wrong would silently seed the corpus with the
/// malformation the pathological seeds already cover deliberately.
fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

/// A JSON-RPC notification, as a server sends it.
fn notification(method: &str, params: &serde_json::Value) -> String {
    frame(&serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string())
}

/// A JSON-RPC response to request `id`.
fn response(id: i64, result: &serde_json::Value) -> String {
    frame(&serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
}

/// Real traffic, plus the malformations a mutation would have to be lucky to
/// find — `lsp_wire`'s corpus.
///
/// The real half is **derived**, in the sense `journal_open`'s is: every body
/// is a real `lsp_types` value serialized by the same `serde` impls `async-lsp`
/// writes with, so a field that moves upstream moves the seeds with it. What is
/// hand-written is the malformed half, and it has to be: a malformation is
/// exactly a shape no serializer produces. Same call as `EXTRA_CSV_SEEDS`.
fn lsp_wire_seeds() {
    use phosphor_buffer::lsp::lsp_types;

    println!("seeds/lsp_wire:");

    let range = |line: u32, from: u32, to: u32| lsp_types::Range {
        start: lsp_types::Position {
            line,
            character: from,
        },
        end: lsp_types::Position {
            line,
            character: to,
        },
    };
    let diagnostics = |items: Vec<lsp_types::Diagnostic>| {
        notification(
            "textDocument/publishDiagnostics",
            &serde_json::to_value(lsp_types::PublishDiagnosticsParams {
                uri: lsp_types::Url::parse("file:///tmp/main.rs").expect("a file url"),
                diagnostics: items,
                version: None,
            })
            .expect("diagnostics serialize"),
        )
    };
    let diagnostic = |range: lsp_types::Range, message: &str| lsp_types::Diagnostic {
        range,
        message: message.to_owned(),
        ..lsp_types::Diagnostic::default()
    };

    // rust-analyzer's opening move, and the one field `sync_kind` reads.
    let ready = lsp_types::InitializeResult {
        capabilities: lsp_types::ServerCapabilities {
            text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                lsp_types::TextDocumentSyncKind::INCREMENTAL,
            )),
            completion_provider: Some(lsp_types::CompletionOptions::default()),
            hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
            ..lsp_types::ServerCapabilities::default()
        },
        server_info: Some(lsp_types::ServerInfo {
            name: "rust-analyzer".to_owned(),
            version: Some("1.0.0".to_owned()),
        }),
    };
    write(
        "lsp_wire",
        "initialize-result",
        response(
            0,
            &serde_json::to_value(&ready).expect("a capability set serializes"),
        )
        .as_bytes(),
    );

    // Two diagnostics, one of them with an astral character before the column
    // so the UTF-16 seam is in the corpus from the first exec.
    write(
        "lsp_wire",
        "diagnostics",
        diagnostics(vec![
            lsp_types::Diagnostic {
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                source: Some("rustc".to_owned()),
                ..diagnostic(range(0, 4, 8), "cannot find value `🦀` in this scope")
            },
            diagnostic(range(1, 0, 0), "unused import"),
        ])
        .as_bytes(),
    );

    // The reproducer. `character` is a `u32` on the wire, so `u32::MAX`
    // deserialises — and `column_from_utf16` carried the excess through with a
    // plain `+`. See `lsp_wire.rs`'s header; this file is why the target finds
    // it in one exec rather than never.
    write(
        "lsp_wire",
        "diagnostics-ceiling",
        diagnostics(vec![diagnostic(range(0, u32::MAX, u32::MAX), "")]).as_bytes(),
    );

    // An empty list is how a server says "this file is clean", and it is the
    // frame the editor sees most often.
    write(
        "lsp_wire",
        "diagnostics-empty",
        diagnostics(Vec::new()).as_bytes(),
    );

    let completions = lsp_types::CompletionList {
        is_incomplete: true,
        items: vec![
            lsp_types::CompletionItem {
                label: "default".to_owned(),
                sort_text: Some("ffffffff7fffffffdefault".to_owned()),
                detail: Some("fn() -> Self".to_owned()),
                documentation: Some(lsp_types::Documentation::MarkupContent(
                    lsp_types::MarkupContent {
                        kind: lsp_types::MarkupKind::Markdown,
                        value: "Returns the \"default value\".\n\n# Examples\n".to_owned(),
                    },
                )),
                ..lsp_types::CompletionItem::default()
            },
            lsp_types::CompletionItem {
                label: "deserialize".to_owned(),
                filter_text: Some("deserialize".to_owned()),
                insert_text: Some("deserialize(${1:deserializer})".to_owned()),
                ..lsp_types::CompletionItem::default()
            },
        ],
    };
    write(
        "lsp_wire",
        "completion-list",
        response(
            1,
            &serde_json::to_value(lsp_types::CompletionResponse::List(completions))
                .expect("a completion list serializes"),
        )
        .as_bytes(),
    );

    // Both parameter-label shapes in one signature: offsets (UTF-16 units into
    // the label, which `parameter_range` converts) and the text form it has to
    // find instead.
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "fn add(left: u32, right: u32) -> u32".to_owned(),
            documentation: None,
            parameters: Some(vec![
                lsp_types::ParameterInformation {
                    label: lsp_types::ParameterLabel::LabelOffsets([7, 17]),
                    documentation: None,
                },
                lsp_types::ParameterInformation {
                    label: lsp_types::ParameterLabel::Simple("right: u32".to_owned()),
                    documentation: None,
                },
            ]),
            active_parameter: Some(1),
        }],
        active_signature: Some(0),
        active_parameter: Some(0),
    };
    write(
        "lsp_wire",
        "signature-help",
        response(
            2,
            &serde_json::to_value(&help).expect("a signature serializes"),
        )
        .as_bytes(),
    );

    let hover = lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: "```rust\nfn main()\n```\n\n---\n\nThe entry point.\n".to_owned(),
        }),
        range: None,
    };
    write(
        "lsp_wire",
        "hover-markup",
        response(
            3,
            &serde_json::to_value(&hover).expect("a hover serializes"),
        )
        .as_bytes(),
    );

    // The `LocationLink` shape, which rust-analyzer sends whenever the client
    // advertises link support — and this client does.
    let places = lsp_types::GotoDefinitionResponse::Link(vec![lsp_types::LocationLink {
        origin_selection_range: None,
        target_uri: lsp_types::Url::parse("file:///tmp/lib.rs").expect("a file url"),
        target_range: lsp_types::Range::default(),
        target_selection_range: lsp_types::Range::default(),
    }]);
    write(
        "lsp_wire",
        "definition-link",
        response(
            4,
            &serde_json::to_value(&places).expect("a link serializes"),
        )
        .as_bytes(),
    );

    let edit = lsp_types::WorkspaceEdit {
        changes: Some(
            [(
                lsp_types::Url::parse("file:///tmp/main.rs").expect("a file url"),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::default(),
                    new_text: "renamed".to_owned(),
                }],
            )]
            .into_iter()
            .collect(),
        ),
        document_changes: None,
        change_annotations: None,
    };
    write(
        "lsp_wire",
        "workspace-edit",
        response(5, &serde_json::to_value(&edit).expect("an edit serializes")).as_bytes(),
    );

    // A session, not a message: four frames back to back is the only seed that
    // teaches the fuzzer a body *ends*, which is the whole of `FrameScan`'s job.
    let session = format!(
        "{}{}{}{}",
        response(0, &serde_json::to_value(&ready).expect("serializes")),
        notification(
            "window/logMessage",
            &serde_json::json!({"type": 3, "message": "loading"})
        ),
        diagnostics(vec![diagnostic(range(0, 0, 1), "x")]),
        response(3, &serde_json::to_value(&hover).expect("serializes")),
    );
    write("lsp_wire", "session", session.as_bytes());

    for (name, body) in EXTRA_LSP_SEEDS {
        write("lsp_wire", name, body.as_bytes());
    }

    // A long header line, past `MAX_HEADER_BYTES` (8 KiB) — the slow abort
    // `FrameScan::push` refuses. Generated rather than written out because
    // eight thousand `x`s is not a reviewable literal.
    let mut forever = String::from("Content-Length: ");
    forever.push_str(&"9".repeat(9_000));
    write("lsp_wire", "header-forever", forever.as_bytes());

    // EOF at every offset of the shortest legal frame. A server that dies
    // mid-message stops at *some* byte, and which byte decides whether the
    // client is waiting on a header, a length, a blank line or a body — four
    // states this walks through one at a time. Twenty-two files, each of them
    // trivially reviewable, and cheaper than hoping a mutation truncates.
    let shortest = frame("{}");
    for cut in 0..shortest.len() {
        write(
            "lsp_wire",
            &format!("eof-{cut:02}"),
            shortest[..cut].as_bytes(),
        );
    }
}

/// The malformed frames, each named for what it is.
///
/// Every one is a review question `T036` was asked and could not answer from
/// the code: a header that never ends, a length that lies in each direction, a
/// length that is not a number, no length at all, two lengths, a body that is
/// not JSON, JSON that is not a message, an id nothing asked for, a lone `\r`,
/// and UTF-8 that a read boundary can fall inside. The target varies the read
/// size itself, which is what turns the last one into a split.
const EXTRA_LSP_SEEDS: [(&str, &str); 13] = [
    ("truncated-header", "Content-Len"),
    ("length-lies-long", "Content-Length: 40\r\n\r\n{}"),
    (
        "length-lies-short",
        "Content-Length: 1\r\n\r\n{\"jsonrpc\":\"2.0\"}",
    ),
    ("length-zero", "Content-Length: 0\r\n\r\n"),
    ("length-negative", "Content-Length: -1\r\n\r\n{}"),
    ("length-absurd", "Content-Length: 999999999999999\r\n\r\n"),
    (
        "length-missing",
        "Content-Type: application/vscode-jsonrpc\r\n\r\n{}",
    ),
    (
        "length-duplicated",
        "Content-Length: 2\r\nContent-Length: 16\r\n\r\n{\"jsonrpc\":\"2\"}\r\n",
    ),
    ("body-not-json", "Content-Length: 5\r\n\r\nhello"),
    ("json-not-a-message", "Content-Length: 7\r\n\r\n[1,2,3]"),
    (
        "id-nobody-asked",
        "Content-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":9999,\"result\":null}",
    ),
    ("lone-cr", "Content-Length: 2\rContent-Length: 2\r\n\r\n{}"),
    (
        "utf8-astral",
        "Content-Length: 26\r\n\r\n{\"jsonrpc\":\"2.0\",\"m\":\"🦀\"}",
    ),
];

fn main() {
    let root = repo_root();
    let scratch = std::env::temp_dir().join("phosphor-fuzz-seed");
    fs::create_dir_all(&scratch).expect("a scratch directory");

    journal_open_seeds(&scratch);
    journal_records_seeds(&scratch);
    key_notation_seeds(&root);
    theme_load_seeds(&root);
    csv_parse_seeds(&root);
    lsp_wire_seeds();

    let _ = fs::remove_dir_all(&scratch);
    println!("\nseeds written under {}/seeds", env!("CARGO_MANIFEST_DIR"));
}
