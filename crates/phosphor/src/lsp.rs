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
//! # Where the diagnostics went
//!
//! This module held them until `T041`, in a `BTreeMap<PathBuf, Vec<Diagnostic>>`
//! behind a `Mutex` with its own `replace`/`of`/`answer` — written at `T040`
//! because the map that should have held them,
//! `phosphor_core::store::diagnostics`, has no lock and this binary needed one.
//! So the documented store and the real one were two maps with one name, and
//! that module had **no importer at all**.
//!
//! They are in [`crate::store`] now, beside the regions and behind the same
//! revision, and the argument this header used to make for two handles is made
//! there instead — it was always an argument about the *store*, not about the
//! LSP.
//!
//! # What a server may not do to this editor
//!
//! Nothing here decides that; `crate::deliver` does, by reading each
//! capability's own `McpPolicy` before applying it. It is named here because
//! this is the module that hands a server a way in: a `Post` is a producer
//! door, and `ingest-diagnostics` is `Allow` while every other `Lsp` verb a
//! server could name is `Deny`. So a server can publish what it found and
//! cannot, for instance, open a completion float the user did not ask for.

use std::path::{Path, PathBuf};

use phosphor_buffer::lsp::{LanguageServers, Post, ServerSpec};
use phosphor_core::action::Action;
use phosphor_core::language::Languages;
use phosphor_core::request::LanguageId;

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
