//! What the binary owes `T036`–`T040`: the servers, the sink they post
//! through, and the diagnostics they publish.
//!
//! `crates/phosphor-buffer/src/lsp.rs` is the client — one thread, one runtime,
//! one child per language — and its header says where its output goes:
//! *"when a server publishes diagnostics, this module builds
//! `Action::Lsp(IngestDiagnostics { … })` and hands it to the [`Post`] the host
//! supplied"*. [`sink`] is that `Post`, and it is the first producer the event
//! queue (`crate::events`) was built for: `AppEvent::Posted` carried an
//! `expect(dead_code)` whose reason said it *"should disappear when the first
//! producer lands"*, and this is the module that landed.
//!
//! Owned by `spine`, because everything here is the loop's half of the seam
//! rather than the client's.
//!
//! # Why the diagnostics live here and not in `Editing`
//!
//! Two readers, and they are on different sides of the Steel barrier. The
//! gutter reads them per frame off the buffer that is on screen
//! (`phosphor_ui::diagnostics::DiagnosticsVm`); the `diagnostics` **query**
//! answers them to Steel through [`crate::AppHost`], which is `Send + Sync`
//! and behind an `Arc` because a binding runs inside the running VM. One store
//! with two handles is what keeps those two from disagreeing about a file — the
//! alternative is the host answering a query about a set the frame does not
//! draw.
//!
//! # What a server may not do to this editor
//!
//! Nothing here decides that; `crate::deliver` does, by reading each
//! capability's own `McpPolicy` before applying it. It is named here because
//! this is the module that hands a server a way in: a `Post` is a producer
//! door, and `ingest-diagnostics` is `Allow` while every other `Lsp` verb a
//! server could name is `Deny`. So a server can publish what it found and
//! cannot, for instance, open a completion float the user did not ask for.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phosphor_buffer::lsp::{LanguageServers, Post, ServerSpec};
use phosphor_core::action::Action;
use phosphor_core::language::Languages;
use phosphor_core::request::{Diagnostic, LanguageId};
use phosphor_core::value::{Value, Wire as _};

use crate::events;

// ---------------------------------------------------------------------------
// The sink
// ---------------------------------------------------------------------------

/// Which producer an LSP-posted event names itself as.
///
/// `events::Posted::source` exists so an Action the binary does not apply yet
/// says *which subsystem asked for it*; this is the string the whole of `S4`
/// answers to.
pub(crate) const SOURCE: &str = "lsp";

/// The callback the client posts through — the queue, seen from a server's
/// thread.
///
/// `Post` is `Arc<dyn Fn(Action) -> bool + Send + Sync>` and its `bool` is
/// *"is anyone still listening"*, which is exactly what
/// [`events::Poster::post`] answers: a producer whose loop has ended stops
/// rather than spinning. So the two contracts compose with no adaptation
/// beyond naming the source.
pub(crate) fn sink(poster: events::Poster) -> Post {
    std::sync::Arc::new(move |action: Action| {
        poster.post(events::AppEvent::Posted(events::Posted {
            source: SOURCE,
            action,
        }))
    })
}

/// The client's other door: **a server changed state, draw again**.
///
/// Separate from [`sink`] because it carries no `Action` and could not — see
/// `events::AppEvent::Woke`, the variant the queue's header reserved for
/// exactly this and which this is the first producer of.
///
/// What it is *for* is one thing, and the thing is on the statusline: the
/// server chip (`7c`'s `rust-analyzer ✓`, `main::server_chip`). Without a wake
/// the chip is correct and stale — the loop draws when a producer speaks, a
/// state change speaks to nobody, so a server that became ready went on saying
/// `starting …` until the next keystroke and one that failed to spawn said
/// nothing at all until then.
///
/// The answer is ignored, unlike [`sink`]'s: `Poster::post` reports *"is
/// anyone still listening"*, and a `Woke` callback has nothing to stop doing —
/// the client's own drop is what ends it, and a wake nobody is left to draw is
/// one missed frame rather than a leak.
pub(crate) fn waking(poster: events::Poster) -> phosphor_buffer::lsp::Woke {
    std::sync::Arc::new(move || {
        let _listening = poster.post(events::AppEvent::Woke(SOURCE));
    })
}

// ---------------------------------------------------------------------------
// Attaching
// ---------------------------------------------------------------------------

/// Starts `language`'s blessed server for a file, if its declaration names one.
///
/// **Everything about *which* server comes from the declaration**, which is
/// `T037`'s whole point: `runtime/languages/rust.scm` says `rust-analyzer`, the
/// `Languages` table records it, and `ServerSpec::from_language_spec` is the
/// door back. There is no Rust table on this path — `lsp::blessed` is consulted
/// only by that function, and only for the root markers a declaration has no
/// field for.
///
/// Answers the **root** it attached at, and [`None`] when the language
/// declares no server (`'()` — `steel`, `markdown` and `csv` ship that way,
/// and it is an honest first-class thing to be). That is not a failure, which
/// is why the caller gets an `Option` rather than a `Result`.
///
/// The root is the nearest ancestor holding one of the spec's markers, and the
/// file's own directory when it holds none. Rootless would be the other
/// choice; a server told nothing about the project indexes nothing, so a
/// directory that is definitely on disk is the better floor.
///
/// **The root is answered because a diagnostic arrives under it.** The client
/// makes a published path workspace-relative when it is inside the root — the
/// capability's own parameter says it carries one — so a host that kept only
/// the absolute path it sent would look up a key that never arrives, and every
/// diagnostic would land in a store nothing reads. [`key_for`] is the other
/// half of that rule and the two are written next to each other for exactly
/// that reason. (Found by pressing no key at all: the pty test for an
/// unsolicited publish drew nothing.)
pub(crate) fn attach(
    servers: &LanguageServers,
    languages: &Languages,
    language: &LanguageId,
    file: &Path,
) -> Option<PathBuf> {
    let spec = ServerSpec::from_language_spec(language, languages.get(language)?)?;
    let root = spec
        .root_for(file)
        .or_else(|| file.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    servers.attach(spec, root.clone());
    Some(root)
}

/// The key a diagnostic about `file` will arrive under.
///
/// The client's own rule, restated once on this side: *"the path is
/// workspace-relative when it is under the root … an absolute path survives
/// for a file outside it"*. A language with no server has no root and keeps its
/// absolute path, which is the same answer for the same reason — nothing will
/// ever publish about it.
pub(crate) fn key_for(file: &Path, root: Option<&Path>) -> PathBuf {
    root.and_then(|root| file.strip_prefix(root).ok())
        .map_or_else(|| file.to_path_buf(), Path::to_path_buf)
}

/// The absolute path a server is told about.
///
/// LSP addresses documents by URI and a relative path has no URI, so this is
/// not a nicety: `didOpen` for `sample.rs` and a publish about
/// `/tmp/x/sample.rs` are two different documents as far as the client's
/// `HashMap<PathBuf, Document>` is concerned, and the diagnostics would land in
/// a file the editor is not showing. A path that cannot be made absolute is
/// handed over as it is, which is the same answer the editor gives it.
pub(crate) fn absolute(file: &Path) -> PathBuf {
    file.canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(file)))
        .unwrap_or_else(|_| file.to_path_buf())
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Every diagnostic the servers have published, by file.
///
/// **Replace, never merge.** `ingest-diagnostics` is declared as *"the
/// diagnostics, replacing that file's set"* and the protocol agrees: a
/// `textDocument/publishDiagnostics` is the whole current set for that file, so
/// a server that fixes an error publishes a shorter list rather than a
/// retraction. Merging would make an error that has been fixed permanent.
#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    by_file: Mutex<BTreeMap<PathBuf, Vec<Diagnostic>>>,
}

impl Diagnostics {
    /// Takes one file's whole set.
    ///
    /// An empty publish **removes the key** rather than storing an empty
    /// vector, so `answer(None)` does not accumulate a row per file anyone has
    /// ever opened.
    pub(crate) fn replace(&self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        let mut by_file = self.lock();
        if diagnostics.is_empty() {
            by_file.remove(&path);
        } else {
            by_file.insert(path, diagnostics);
        }
    }

    /// One file's set, cloned out from behind the lock.
    ///
    /// Cloned because the frame holds it across a `&mut Editor` borrow —
    /// `DiagnosticsVm::rows` installs virtual text — and a guard held that long
    /// would be a lock held across a redraw.
    pub(crate) fn of(&self, path: &Path) -> Vec<Diagnostic> {
        self.lock().get(path).cloned().unwrap_or_default()
    }

    /// The `diagnostics` query's answer: every diagnostic, or one file's.
    ///
    /// Each record is the [`Diagnostic`] itself with its `path` added, because
    /// the query may answer for every file at once and a record that did not
    /// say which file it was about would be unreadable in that shape.
    pub(crate) fn answer(&self, only: Option<&Path>) -> Vec<Value> {
        let by_file = self.lock();
        by_file
            .iter()
            .filter(|(path, _)| only.is_none_or(|wanted| wanted == path.as_path()))
            .flat_map(|(path, diagnostics)| {
                diagnostics.iter().map(move |diagnostic| {
                    let mut args = phosphor_core::value::Args::new()
                        .with("path", Value::Text(path.display().to_string()));
                    if let Value::Record(fields) = diagnostic.to_value() {
                        for (field, value) in fields.into_pairs() {
                            args.set(&field, value);
                        }
                    }
                    Value::Record(args)
                })
            })
            .collect()
    }

    /// The map, with a poisoned lock read through rather than panicked on —
    /// a diagnostic set is not worth taking the editor down for.
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PathBuf, Vec<Diagnostic>>> {
        self.by_file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use phosphor_core::request::{Position, Severity, Span};

    use super::Diagnostics;

    fn diagnostic(message: &str) -> phosphor_core::request::Diagnostic {
        phosphor_core::request::Diagnostic {
            span: Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 2 },
            },
            severity: Severity::Trouble,
            message: message.to_owned(),
            source: Some("rust-analyzer".to_owned()),
        }
    }

    /// The protocol's rule: a publish is the whole current set, so the second
    /// one is not added to the first.
    #[test]
    fn a_second_publish_replaces_the_first_rather_than_adding_to_it() {
        let store = Diagnostics::default();
        let path = std::path::PathBuf::from("/tmp/a.rs");
        store.replace(
            path.clone(),
            vec![diagnostic("first"), diagnostic("second")],
        );
        store.replace(path.clone(), vec![diagnostic("only")]);
        let held = store.of(&path);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].message, "only");
    }

    /// A server that fixed everything publishes an empty list, and the file
    /// stops being one the query has anything to say about.
    #[test]
    fn an_empty_publish_clears_the_file_out_of_the_query() {
        let store = Diagnostics::default();
        let path = std::path::PathBuf::from("/tmp/a.rs");
        store.replace(path.clone(), vec![diagnostic("boom")]);
        assert_eq!(store.answer(None).len(), 1);
        store.replace(path.clone(), Vec::new());
        assert!(store.of(&path).is_empty());
        assert!(
            store.answer(None).is_empty(),
            "an empty set must not leave a row behind"
        );
    }

    /// The query narrows by path, and every record says which file it is about.
    #[test]
    fn the_query_narrows_to_one_file_and_every_record_names_its_file() {
        let store = Diagnostics::default();
        store.replace("/tmp/a.rs".into(), vec![diagnostic("a")]);
        store.replace("/tmp/b.rs".into(), vec![diagnostic("b")]);
        assert_eq!(store.answer(None).len(), 2);

        let one = store.answer(Some(std::path::Path::new("/tmp/b.rs")));
        assert_eq!(one.len(), 1);
        let phosphor_core::value::Value::Record(fields) = &one[0] else {
            panic!("a diagnostic answers as a record");
        };
        assert_eq!(
            fields.get("path"),
            Some(&phosphor_core::value::Value::Text("/tmp/b.rs".to_owned())),
        );
        assert_eq!(
            fields.get("message"),
            Some(&phosphor_core::value::Value::Text("b".to_owned())),
        );
    }
}
