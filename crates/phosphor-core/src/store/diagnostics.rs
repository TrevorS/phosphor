//! Where `ingest-diagnostics` lands (`T040`).
//!
//! **This is not the store, and it is deliberately too small to become one.**
//! It holds one set per file and nothing else — no ids, no anchors, no
//! lifetime, no seen-state.
//!
//! # `T041` folded it in, and the fold was the point
//!
//! This header used to end *"when `T041` lands, this map is what folds into
//! it"*. It has: [`crate::store::Store`] owns this beside
//! [`crate::store::region::Regions`], behind one
//! [`Revision`](crate::query::Revision) that both move, so a publish and a
//! `mark-seen` are the same kind of news to a cache.
//!
//! What the fold actually fixed was worse than untidiness. Nothing outside this
//! module imported it: `crates/phosphor/src/lsp.rs` had its own
//! `BTreeMap<PathBuf, Vec<Diagnostic>>` with its own `replace`/`of`/`answer`,
//! written at `T040` because it needed a `Mutex` and this crate holds no locks.
//! Two maps with one name, and the documented one dead.
//!
//! A diagnostic is still not a *region*. `6c` wants it anchored to a
//! tree-sitter node and surviving the rewrite that moved it, and that is
//! `T042`; what it has today is an owner — the region covering its line — which
//! is what makes its virtual-text rail collapsible.
//!
//! # Why the order is fixed here rather than at the drawing
//!
//! A server sends its diagnostics in whatever order it found them, and
//! rust-analyzer's order for one file is not stable between two publishes of
//! the same errors. Every consumer of this map turns it into something a person
//! reads in order — `T040`'s `┊ ■` rows hang under their lines in list order,
//! and the `diagnostics` query answers a list. A list that reshuffles between
//! two identical publishes is a diff nobody can read, so the sort is done once,
//! on the way in, where there is exactly one of it. Same argument
//! `phosphor-buffer`'s `file_edits_from_lsp` records for `WorkspaceEdit`.
//!
//! Owned by `store`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::request::Diagnostic;

/// Every file's diagnostics, as the servers last published them.
///
/// The read side of the `diagnostics` query and the write side of
/// `ingest-diagnostics`, which is the whole of what this type is for. Keyed by
/// the path the capability carries — workspace-relative where the file is under
/// the root, absolute where it is not (`phosphor-buffer`'s `ingest`) — and this
/// module never interprets it, so a host that mixes the two forms sees two
/// files. That is the host's contract to keep, not something this can check.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    by_file: BTreeMap<PathBuf, Vec<Diagnostic>>,
}

impl Diagnostics {
    /// Nothing published yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// **The arm of `ingest-diagnostics`.** Replaces `path`'s whole set.
    ///
    /// Replacing rather than merging is the capability's own wording —
    /// *"the diagnostics, replacing that file's set"* — and it is also what LSP
    /// means: `publishDiagnostics` is the full set for that file, so a server
    /// that fixed an error re-sends the file without it. Merging would make
    /// every fixed error permanent.
    ///
    /// **An empty publish removes the file.** That is how a server says *"this
    /// file is clean now"*, and keeping the key would leave a file listed as
    /// having diagnostics with none in it, plus a map that only ever grows over
    /// a session.
    pub fn ingest(&mut self, path: PathBuf, mut diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            self.by_file.remove(&path);
            return;
        }
        // Stable, so two diagnostics on the same span keep the order the server
        // put them in — that order is the server's own judgement about which it
        // considers primary, and there is nothing better to break the tie with.
        diagnostics.sort_by_key(|diagnostic| diagnostic.span);
        self.by_file.insert(path, diagnostics);
    }

    /// One file's diagnostics, ordered by span. Empty for a file nothing has
    /// published about, which is the same answer as a clean file and means the
    /// same thing to every caller.
    #[must_use]
    pub fn of(&self, path: &Path) -> &[Diagnostic] {
        self.by_file.get(path).map_or(&[], Vec::as_slice)
    }

    /// Every file with diagnostics, in path order — the `diagnostics` query
    /// with its `path` parameter absent.
    pub fn files(&self) -> impl Iterator<Item = (&Path, &[Diagnostic])> {
        self.by_file
            .iter()
            .map(|(path, diagnostics)| (path.as_path(), diagnostics.as_slice()))
    }

    /// Diagnostics across every file — the count `6c`'s statusline draws as
    /// `■ 1`.
    ///
    /// Zero is *"no file has any"*, so nothing here needs an `is_empty` of its
    /// own; there was one, with no caller and no contract asking for one.
    #[must_use]
    pub fn total(&self) -> usize {
        self.by_file.values().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{Position, Severity, Span};

    fn at(line: u32, column: u32) -> Position {
        Position { line, column }
    }

    fn diagnostic(line: u32, message: &str) -> Diagnostic {
        Diagnostic {
            span: Span {
                start: at(line, 1),
                end: at(line, 9),
            },
            severity: Severity::Trouble,
            message: message.to_owned(),
            source: Some("rust-analyzer".to_owned()),
        }
    }

    fn path() -> PathBuf {
        PathBuf::from("src/retry.rs")
    }

    fn messages(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    #[test]
    fn a_publish_replaces_the_files_set_rather_than_adding_to_it() {
        // The server fixed one error and re-sent the file. Merging would make
        // the fixed one permanent, which is the bug this arm exists not to
        // have.
        let mut diagnostics = Diagnostics::new();
        diagnostics.ingest(
            path(),
            vec![diagnostic(12, "E0308"), diagnostic(19, "E0425")],
        );
        diagnostics.ingest(path(), vec![diagnostic(19, "E0425")]);
        assert_eq!(messages(diagnostics.of(&path())), ["E0425"]);
        assert_eq!(diagnostics.total(), 1);
    }

    #[test]
    fn an_empty_publish_is_how_a_server_says_the_file_is_clean() {
        let mut diagnostics = Diagnostics::new();
        diagnostics.ingest(path(), vec![diagnostic(12, "E0308")]);
        diagnostics.ingest(path(), Vec::new());
        assert!(diagnostics.of(&path()).is_empty());
        assert_eq!(
            diagnostics.files().count(),
            0,
            "the file is gone, not merely empty"
        );
        assert_eq!(diagnostics.total(), 0);
        // Which is also the answer for a file nothing was ever published about:
        // the same `map_or(&[], …)` arm, so one assertion covers both and a
        // test that only built a `Diagnostics::new()` would be restating
        // `#[derive(Default)]` on a `BTreeMap`.
        assert!(diagnostics.of(Path::new("src/fetch.rs")).is_empty());
    }

    #[test]
    fn the_answer_is_ordered_by_span_whatever_order_the_server_sent() {
        // The property the `┊` rows and the query both rest on: the same set
        // published twice in two orders answers the same way twice.
        let mut forwards = Diagnostics::new();
        forwards.ingest(
            path(),
            vec![
                diagnostic(4, "first"),
                diagnostic(12, "second"),
                diagnostic(64, "third"),
            ],
        );
        let mut backwards = Diagnostics::new();
        backwards.ingest(
            path(),
            vec![
                diagnostic(64, "third"),
                diagnostic(4, "first"),
                diagnostic(12, "second"),
            ],
        );
        assert_eq!(
            messages(forwards.of(&path())),
            ["first", "second", "third"],
            "sorted by where they are, not by when they arrived"
        );
        assert_eq!(forwards, backwards);
    }

    #[test]
    fn two_diagnostics_on_one_span_keep_the_servers_own_order() {
        // A stable sort, so the server's judgement about which of two is the
        // primary one survives. Nothing else could break this tie.
        let mut diagnostics = Diagnostics::new();
        diagnostics.ingest(
            path(),
            vec![diagnostic(12, "primary"), diagnostic(12, "note")],
        );
        assert_eq!(messages(diagnostics.of(&path())), ["primary", "note"]);
    }

    #[test]
    fn files_are_answered_in_path_order_with_their_own_sets() {
        let mut diagnostics = Diagnostics::new();
        diagnostics.ingest(PathBuf::from("src/retry.rs"), vec![diagnostic(12, "E0308")]);
        diagnostics.ingest(
            PathBuf::from("src/fetch.rs"),
            vec![diagnostic(30, "E0061"), diagnostic(31, "E0599")],
        );
        let files: Vec<_> = diagnostics
            .files()
            .map(|(path, set)| (path.to_string_lossy().into_owned(), set.len()))
            .collect();
        assert_eq!(
            files,
            [
                ("src/fetch.rs".to_owned(), 2),
                ("src/retry.rs".to_owned(), 1)
            ]
        );
        assert_eq!(diagnostics.total(), 3);
    }
}
