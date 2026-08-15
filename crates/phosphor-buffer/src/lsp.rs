//! The language-server client — `T036`.
//!
//! Four things live here, and only the last one needs a thread:
//!
//! 1. **[`ServerSpec`] and [`blessed`]** — *which* server a language gets, and
//!    on what terms. This is the load-bearing half of the task.
//! 2. **[`ServerState`]** — a total state machine over [`ServerEvent`], so that
//!    "is rust-analyzer up?" is a value the render loop reads rather than a
//!    question it asks.
//! 3. **The UTF-16 seam** ([`column_from_utf16`] and its inverse) — the
//!    conversion `phosphor_core::request::Position` already says belongs to this
//!    crate.
//! 4. **[`LanguageServers`]** — one thread, one `tokio` runtime, one child
//!    process per language, and everything it learns handed to the binary's
//!    event queue as an `Action`.
//!
//! Owned by `surface`.
//!
//! # Blessed, not discovered
//!
//! The Component Breakdown makes this a product promise rather than an
//! implementation note: first-class means *"bundled grammar, blessed LSP
//! auto-configured (not just discovered)"*, and the road up for everything else
//! is *"`(define-language ...)` declares a grammar, an LSP command, and locale
//! hooks … the bundled set is just the `define-language` calls we wrote and
//! stand behind."*
//!
//! So there is **no PATH search in this module**, and that is the whole
//! difference. A [`ServerSpec`] names an exact command, its arguments, the
//! markers that identify a project root and how long we are willing to wait for
//! it — and if that command is not installed, the state is
//! [`Failure::Spawn`], which the editor can say out loud. Discovery would
//! instead find *something*, and the failure mode is not hypothetical: on the
//! machine this task was written on, `rust-analyzer` was on `PATH` as a
//! `rustup` shim that answers
//!
//! ```text
//! error: Unknown binary 'rust-analyzer' in official toolchain 'stable-aarch64-apple-darwin'.
//! ```
//!
//! and exits — a hit for `which`, an immediate EOF for a client. A blessed
//! command that fails to start is a [`ServerState::Crashed`] with the reason in
//! it. A discovered one is a mystery.
//!
//! [`blessed`] is the *default* side of that promise and deliberately not the
//! only one: `T037` moves these declarations into `runtime/languages/*.scm`,
//! where they become `define-language` calls, and
//! [`ServerSpec::from_language_spec`] is the door they come back through. The
//! Rust table is what makes rust-analyzer attach today, and what a Steel
//! declaration overrides tomorrow.
//!
//! # The editor stays synchronous
//!
//! `crates/phosphor/src/events.rs` — the one queue — says where the runtime
//! goes: *"`S4`'s LSP client (`T036`) brings its own runtime and calls
//! `Poster::post` from whatever thread it likes; nothing here has to learn
//! that happened."* This module is the other side of that sentence.
//! (`Poster` is `pub(crate)` in the binary, so it is named here and not
//! linked — [`Post`] is this crate's whole view of it.)
//!
//! [`LanguageServers::start`] spawns **one** thread with a `current_thread`
//! `tokio` runtime on it. Every public method on [`LanguageServers`] is a
//! non-blocking send into that thread; nothing on the editor's side ever
//! `await`s, and the only shared state is a `Mutex<HashMap<…>>` of
//! [`ServerState`] that a frame reads and releases. That is Design Language's
//! *"no widget ever blocks"* made structural: a server that hangs hangs its own
//! task, and the worst it can do to a frame is stay [`ServerState::Starting`].
//!
//! **A server that lies is bounded too, and that bound is not free.** The
//! transport's header carries the size of the frame behind it and `async-lsp`
//! allocates that size before reading a byte of it, so `Content-Length:
//! 999999999999999` is an allocation failure — which Rust answers with
//! `abort()`: no unwind, no [`ServerState::Crashed`], the editor gone with the
//! server. Nothing layered above the framing can catch that, so the bound goes
//! underneath it: [`MAX_FRAME_BYTES`], applied by a pass-through reader between
//! the child's pipe and the main loop, which turns those two lines of shell
//! into a [`Failure::Exited`] carrying the number it refused.
//!
//! **The one thing that is not a state is a result.** When a server publishes
//! diagnostics, this module builds `Action::Lsp(IngestDiagnostics { … })` and
//! hands it to the [`Post`] the host supplied — the same capability
//! `events.rs`'s own tests already spell out as what an LSP client will post.
//! Readiness, by contrast, is *not* posted: there is no "server is ready"
//! capability in the registry, and inventing one to carry a wake is the promise
//! `events.rs` explicitly refuses to make. Readiness is [`state`], read on the
//! frame that cares.
//!
//! [`state`]: LanguageServers::state
//!
//! # UTF-16 is a real trap, not a formality
//!
//! LSP's default `positionEncoding` is **UTF-16 code units**, counted from the
//! start of a line. Phosphor's `Position` is *"1-based column, counted in
//! characters"*. Those agree exactly on ASCII, disagree by one per character on
//! anything above U+07FF only if you were counting bytes, and disagree
//! **again** above U+FFFF, where one character is two UTF-16 units. An emoji
//! ahead of the cursor shifts every diagnostic on that line by one column, and
//! the editor already learned the shape of this bug once: `ß` upper-cases to two
//! characters and moved a cursor.
//!
//! So the client declares `PositionEncodingKind::UTF16` in its
//! `initialize` — saying which of the three it means rather than inheriting a
//! default — and converts at exactly one place, [`column_from_utf16`] and
//! [`utf16_from_column`], against the line's own text.
//!
//! **And it reads the answer**, which is what makes the declaration a contract
//! rather than a hope: a server that replies with any other
//! `positionEncoding` — which the specification forbids, having been offered
//! one — is a [`Failure::Protocol`] naming the encoding it chose, because every
//! column it sent afterwards would be silently in the wrong place. A review of
//! `T036` found the declaration made and the reply unread, and the mutation
//! that proves it is now `initialize_params`' own test rather than an
//! assertion nothing makes.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, ready};
use std::time::Duration;

/// The protocol's own types, re-exported.
///
/// Several functions here take and return them ([`span_from_lsp`],
/// [`file_edits_from_lsp`]), so a caller needs the *same* `lsp-types` this
/// crate compiled against — and that version is `async-lsp`'s choice, not ours.
/// Re-exporting is what makes that unambiguous instead of a second dependency
/// line somebody has to keep in step.
pub use async_lsp::lsp_types;
use async_lsp::router::Router;
use async_lsp::{LanguageServer as _, MainLoop, ServerSocket};
use phosphor_core::action::{Action, LspAction};
use phosphor_core::request::{
    Diagnostic, Edit, FileEdits, FileSpan, LanguageId, LanguageSpec, Position, Severity, Span,
};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};

// ---------------------------------------------------------------------------
// Blessed configuration
// ---------------------------------------------------------------------------

/// How long a blessed server has to answer `initialize` before it is treated as
/// crashed.
///
/// Not a guess about machines: `initialize` is the one LSP request a server is
/// required to answer *before* it does any real work — rust-analyzer replies in
/// well under a second on a cold cache and indexes afterwards, reporting
/// progress. A server that has not replied in thirty seconds is not slow, it is
/// wedged, and [`ServerState::Starting`] forever is the state that tells the
/// user nothing. Overridable per language, because a blessed server's terms are
/// ours to set ([`ServerSpec::with_ready_timeout`]).
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// One blessed language server: the exact command, and the terms.
///
/// Every field is a decision rather than a discovery — see the module header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    /// The language this serves, as `define-language` names it.
    pub language: LanguageId,
    /// The program to run. Resolved by the OS against `PATH`, but **never
    /// searched for**: this is a name we chose, and its absence is a reportable
    /// state rather than a reason to look for a substitute.
    pub command: String,
    /// Arguments, in order. Most servers need `--stdio` to speak the protocol
    /// on their standard streams at all.
    pub args: Vec<String>,
    /// Filenames whose presence marks a project root, nearest first
    /// ([`ServerSpec::root_for`]). Empty means the workspace root is the only
    /// answer.
    pub root_markers: Vec<String>,
    /// The server's `initializationOptions` — free-form JSON the server, not
    /// us, gives meaning to. [`None`] sends none at all, which is different
    /// from sending `null`.
    pub initialization_options: Option<serde_json::Value>,
    /// How long `initialize` may take. See [`READY_TIMEOUT`].
    pub ready_timeout: Duration,
}

impl ServerSpec {
    /// A spec with no arguments, no root markers and the default timeout.
    #[must_use]
    pub fn new(language: &str, command: &str) -> Self {
        Self {
            language: LanguageId(language.to_owned()),
            command: command.to_owned(),
            args: Vec::new(),
            root_markers: Vec::new(),
            initialization_options: None,
            ready_timeout: READY_TIMEOUT,
        }
    }

    /// Adds the arguments, in order.
    #[must_use]
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args = args
            .into_iter()
            .map(|arg| arg.as_ref().to_owned())
            .collect();
        self
    }

    /// Adds the filenames that mark a project root.
    #[must_use]
    pub fn with_root_markers<I, S>(mut self, markers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.root_markers = markers
            .into_iter()
            .map(|marker| marker.as_ref().to_owned())
            .collect();
        self
    }

    /// Overrides [`READY_TIMEOUT`] for this server.
    #[must_use]
    pub const fn with_ready_timeout(mut self, timeout: Duration) -> Self {
        self.ready_timeout = timeout;
        self
    }

    /// Sets the `initializationOptions`.
    #[must_use]
    pub fn with_initialization_options(mut self, options: serde_json::Value) -> Self {
        self.initialization_options = Some(options);
        self
    }

    /// The spec a `define-language` declaration asks for (`T037`), or [`None`]
    /// when it declares no server.
    ///
    /// `LanguageSpec::lsp_command` is a single `Vec<String>` — *"the language
    /// server command and its arguments. Empty means none"* — so this is where
    /// that shape is taken apart, and the empty case is the honest second tier
    /// rather than an error.
    ///
    /// Root markers do not come from `LanguageSpec` because it has no field for
    /// them; a declaration that overrides the command keeps whatever
    /// [`blessed`] knows about finding that language's project root, which is
    /// the useful default when someone swaps `rust-analyzer` for a wrapper
    /// script.
    ///
    /// **And a language [`blessed`] has never heard of gets none at all**, so
    /// its server starts rootless ([`root_for`] answers [`None`] on an empty
    /// marker list). That is the thirteenth language's one rough edge and it is
    /// not this function's to fix: the vocabulary a declaration is written in
    /// has no way to say *"a `mix.exs` marks an Elixir project"*, which is a
    /// `LanguageSpec` field and therefore a protocol change. Recorded here
    /// rather than worked around.
    ///
    /// [`root_for`]: Self::root_for
    #[must_use]
    pub fn from_language_spec(language: &LanguageId, spec: &LanguageSpec) -> Option<Self> {
        let (command, args) = spec.lsp_command.split_first()?;
        let blessed_markers = blessed(language).map(|spec| spec.root_markers);
        Some(Self {
            language: language.clone(),
            command: command.clone(),
            args: args.to_vec(),
            root_markers: blessed_markers.unwrap_or_default(),
            initialization_options: None,
            ready_timeout: READY_TIMEOUT,
        })
    }

    /// The project root for a file: the nearest ancestor directory holding one
    /// of [`root_markers`], or [`None`] when no marker is found.
    ///
    /// Nearest wins, which is the rule that makes a workspace member's own
    /// `Cargo.toml` beat the workspace's — the caller decides whether that is
    /// what it wants, and for rust-analyzer it is, because a member root is a
    /// smaller thing to index.
    ///
    /// [`root_markers`]: Self::root_markers
    #[must_use]
    pub fn root_for(&self, file: &Path) -> Option<PathBuf> {
        if self.root_markers.is_empty() {
            return None;
        }
        let start = if file.is_dir() {
            Some(file)
        } else {
            file.parent()
        };
        let mut directory = start;
        while let Some(candidate) = directory {
            if self
                .root_markers
                .iter()
                .any(|marker| candidate.join(marker).exists())
            {
                return Some(candidate.to_path_buf());
            }
            directory = candidate.parent();
        }
        None
    }
}

/// The first-class twelve, in the Component Breakdown's own order.
///
/// Named here because "first-class" is a closed list — *"each one is a
/// maintained product commitment, which is exactly why the list is short"* —
/// and a closed list nothing recomputes is a list that grows by accident.
pub const FIRST_CLASS: [&str; 12] = [
    "typescript",
    "javascript",
    "rust",
    "python",
    "steel",
    "markdown",
    "json",
    "csv",
    "toml",
    "yaml",
    "html",
    "css",
];

/// The server phosphor blesses for a language, or [`None`] when it blesses none.
///
/// **What "blessed" means here, stated so it can be checked:** every command
/// below is one we chose and stand behind, `--stdio` and all, and none of them
/// is the result of looking at a machine to see what was installed. Two of the
/// twelve get [`None`] on purpose rather than by omission — `steel` has no
/// language server in existence, and `csv` gets a hand-tuned surface instead
/// (`T082`, which drops `tree-sitter-csv` for the same reason). Second tier is
/// *honest*, and so is a first-class language with no server.
///
/// **Verification status, because this is a claim about programs on other
/// people's machines.** `rust-analyzer` is the only entry exercised in this
/// crate's tests — `tests/lsp_rust_analyzer.rs` spawns it and asserts it reaches
/// [`ServerState::Ready`]. The other eight are declarations awaiting `CP-4`,
/// whose checklist names tsserver and pyright by name. They are written here
/// rather than left blank because a declaration is what `T037` transcribes into
/// `runtime/languages/*.scm`, and an empty table would make that task invent
/// them.
///
/// **And the transcription is now recomputed.** Since `T037` landed, the only
/// non-test reader of this function is [`ServerSpec::from_language_spec`],
/// which takes `root_markers` and nothing else — the commands themselves live
/// in the `.scm` files, hand-copied. `tests/language_declarations.rs` reads
/// every shipped declaration and fails if its `lsp_command` and the command
/// here disagree, or if the twelve names drift from [`FIRST_CLASS`]. Two tables
/// saying the same thing with nothing checking them is the failure class
/// `CLAUDE.md` keeps a whole lint category for.
#[must_use]
pub fn blessed(language: &LanguageId) -> Option<ServerSpec> {
    let spec = match language.0.as_str() {
        "rust" => ServerSpec::new("rust", "rust-analyzer")
            .with_root_markers(["Cargo.toml", "rust-project.json"]),
        "typescript" => ServerSpec::new("typescript", "typescript-language-server")
            .with_args(["--stdio"])
            .with_root_markers(["tsconfig.json", "package.json"]),
        "javascript" => ServerSpec::new("javascript", "typescript-language-server")
            .with_args(["--stdio"])
            .with_root_markers(["jsconfig.json", "package.json"]),
        "python" => ServerSpec::new("python", "pyright-langserver")
            .with_args(["--stdio"])
            .with_root_markers(["pyproject.toml", "setup.py", "setup.cfg"]),
        "json" => ServerSpec::new("json", "vscode-json-language-server").with_args(["--stdio"]),
        "yaml" => ServerSpec::new("yaml", "yaml-language-server").with_args(["--stdio"]),
        "toml" => ServerSpec::new("toml", "taplo").with_args(["lsp", "stdio"]),
        "html" => ServerSpec::new("html", "vscode-html-language-server").with_args(["--stdio"]),
        "css" => ServerSpec::new("css", "vscode-css-language-server").with_args(["--stdio"]),
        // `markdown` is deliberately serverless for now: the design gives it a
        // bespoke surface (live preview), not a generic one, and no markdown
        // server has been run against this build. `steel` has none in
        // existence; `csv` is `T082`'s hand-tuned parser.
        _ => return None,
    };
    Some(spec)
}

// ---------------------------------------------------------------------------
// The state machine
// ---------------------------------------------------------------------------

/// Why a server is not serving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The command could not be started at all — not installed, not executable,
    /// or the OS refused. Carries the OS's own words, because *"no such file or
    /// directory"* is the answer the user needs and the one discovery would
    /// have hidden.
    Spawn(String),
    /// The process ended, or its pipes did. A server that exits after
    /// `initialize` has crashed; one that exits after we asked it to is
    /// [`ServerState::Stopped`], and [`ServerState::after`] is what keeps those
    /// apart.
    Exited(String),
    /// The server spoke, but not the protocol — a malformed frame, or an error
    /// response to `initialize`.
    Protocol(String),
    /// `initialize` did not answer inside [`ServerSpec::ready_timeout`].
    Timeout,
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(why) => write!(formatter, "could not start: {why}"),
            Self::Exited(why) => write!(formatter, "exited: {why}"),
            Self::Protocol(why) => write!(formatter, "protocol error: {why}"),
            Self::Timeout => formatter.write_str("timed out during initialize"),
        }
    }
}

/// What a server called itself in its `initialize` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerIdentity {
    /// `serverInfo.name` — `"rust-analyzer"`.
    pub name: String,
    /// `serverInfo.version`, when it gave one.
    pub version: Option<String>,
}

/// Where a language's server is, as the render loop sees it.
///
/// Five states, and the interesting edges are the two that are *not* obvious:
/// a late `initialize` response cannot promote a server we already abandoned,
/// and an exit we asked for is not a crash. Both are in [`after`].
///
/// [`after`]: Self::after
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ServerState {
    /// Nothing has been attached for this language. The state a language with
    /// no blessed server stays in forever, and the honest one.
    #[default]
    NotStarted,
    /// The process is up, or going up; `initialize` has not answered.
    Starting,
    /// `initialize` answered and `initialized` was sent. The server is usable.
    Ready(ServerIdentity),
    /// It died, or never lived. Carries why.
    Crashed(Failure),
    /// We asked it to stop and it did. Distinct from [`Crashed`] because the
    /// editor should say nothing about it.
    ///
    /// [`Crashed`]: Self::Crashed
    Stopped,
}

/// Something that happened to a server.
///
/// Deliberately *not* the same enum as [`ServerState`]: the transition is where
/// the rules live, and a state that is also an event has nowhere to put them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// A process was spawned for this language.
    Attached,
    /// `initialize` came back.
    Initialized(ServerIdentity),
    /// It failed, at any point.
    Failed(Failure),
    /// A restart was asked for — `restart-language-server`.
    Restarted,
    /// A stop was asked for, and taken.
    Stopped,
}

impl ServerState {
    /// The state after `event`. **Total, and closed over all five states.**
    ///
    /// Three rules do all the work, and each exists because the obvious version
    /// is wrong:
    ///
    /// * **[`Initialized`] only promotes [`Starting`].** A restart kills the old
    ///   process, but a reply already in flight still arrives, and a client that
    ///   took it would report [`Ready`] for a process that no longer exists. A
    ///   second `Initialized` on a server that is *already* [`Ready`] leaves it
    ///   ready and keeps the identity it has — no promotion happens, so the rule
    ///   holds, and the property that states it says *"nothing becomes ready
    ///   except from `Starting`"* rather than *"nothing is ready unless it just
    ///   came from `Starting`"*.
    /// * **[`Failed`] does not reach [`Stopped`].** We asked the server to exit;
    ///   the EOF that follows is the exit happening, not a crash, and showing a
    ///   crash for a clean shutdown is how a status line loses its meaning.
    /// * **[`Restarted`] returns to [`Starting`] from anywhere**, including
    ///   [`NotStarted`], because `restart-language-server` on a server that
    ///   never started is a start.
    ///
    /// [`Initialized`]: ServerEvent::Initialized
    /// [`Starting`]: Self::Starting
    /// [`Ready`]: Self::Ready
    /// [`Failed`]: ServerEvent::Failed
    /// [`Stopped`]: Self::Stopped
    /// [`Restarted`]: ServerEvent::Restarted
    /// [`NotStarted`]: Self::NotStarted
    #[must_use]
    pub fn after(&self, event: &ServerEvent) -> Self {
        match (self, event) {
            (_, ServerEvent::Attached | ServerEvent::Restarted) => Self::Starting,
            (Self::Starting, ServerEvent::Initialized(identity)) => Self::Ready(identity.clone()),
            // A reply for a process we already gave up on.
            (_, ServerEvent::Initialized(_)) => self.clone(),
            (Self::Stopped, ServerEvent::Failed(_)) => Self::Stopped,
            (_, ServerEvent::Failed(failure)) => Self::Crashed(failure.clone()),
            (_, ServerEvent::Stopped) => Self::Stopped,
        }
    }

    /// Whether a request may be sent.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Whether the editor is waiting on this server. What a spinner would bind
    /// to, once one animates.
    #[must_use]
    pub const fn is_starting(&self) -> bool {
        matches!(self, Self::Starting)
    }

    /// Why it is not serving, when that is the reason.
    #[must_use]
    pub const fn failure(&self) -> Option<&Failure> {
        match self {
            Self::Crashed(failure) => Some(failure),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// The UTF-16 seam
// ---------------------------------------------------------------------------

/// The text of one 0-based line, as LSP counts lines, with its terminator
/// removed.
///
/// A `\r` left on the end would add a phantom column at the end of every line
/// of a CRLF file, which is the off-by-one this function exists to not have.
/// Past the end of the text the answer is `""` rather than a panic — a server
/// may report a diagnostic on a line we have since deleted.
///
/// **It splits on `\n` and strips one trailing `\r`: LF and CRLF, and nothing
/// else.** The specification also allows a lone `\r` as a terminator, and this
/// deliberately treats one as content. Two reasons, and the second is the real
/// one: classic-Mac line endings have been extinct for two decades and no
/// blessed server emits them — while a *stray* `\r` inside a line does occur,
/// in files with mixed endings, and splitting on it would renumber every line
/// after it. Wrong line numbers on a file that opens fine is a worse failure
/// than a stray control character counted as a column.
///
/// The consequence, since it is observable: in a CRLF file whose line content
/// genuinely ends in `\r`, exactly one `\r` is removed and the other is kept.
/// That is the correct reading of `"a\r\r\n"` under this rule, and
/// `tests/lsp_properties.rs` excludes it from the terminator law rather than
/// pretending the ambiguity is not the file format's.
#[must_use]
pub fn line_at(text: &str, line: u32) -> &str {
    text.split('\n')
        .nth(line as usize)
        .map_or("", |line| line.strip_suffix('\r').unwrap_or(line))
}

/// A line's length in UTF-16 code units — what an LSP `character` past the end
/// of the line is measured against.
#[must_use]
pub fn utf16_len(line: &str) -> u32 {
    u32::try_from(line.chars().map(char::len_utf16).sum::<usize>()).unwrap_or(u32::MAX)
}

/// LSP's 0-based UTF-16 `character` → phosphor's 1-based character column.
///
/// **Total for every input, and that is the point.** Three cases, all of which
/// happen:
///
/// * inside the line — the answer is the number of characters before that code
///   unit, plus one;
/// * **inside a surrogate pair** — a server may name the second half of an
///   astral character (an emoji, a rare CJK glyph). There is no column there, so
///   the answer is the column of the character containing it. Converting back
///   gives that character's first unit, so the pair *canonicalises* rather than
///   round-tripping, and [`utf16_from_column`] documents the same rule from the
///   other side;
/// * **past the end** — the excess is carried through one-for-one, so a
///   position beyond the line stays beyond it by the same amount. This is what
///   makes the conversion safe when the line's text is unknown: against `""`
///   every column comes back as `character + 1`, which is exactly right for
///   ASCII and wrong by a known amount rather than silently clamped to 1.
///
/// # The last addition saturates, and that is a panic that shipped
///
/// `character` is a `u32` off the wire, so `character == u32::MAX` is a value a
/// server can send and `serde` will accept. Against any all-BMP line — `""`
/// included — the carried-through excess is `character + 1`, which **overflows
/// `u32`**: an arithmetic panic in every build with overflow checks on, which
/// is every developer build and every test run. It happens on the runtime's
/// task, so the editor loses diagnostics from that server and learns nothing
/// about why. `F5` (`fuzz/fuzz_targets/lsp_wire.rs`) reaches it from a
/// `publishDiagnostics` frame; `a_wire_position_at_the_u32_ceiling_does_not_overflow`
/// pins it.
///
/// Saturating is the honest degradation: the excess is carried through until it
/// cannot be, and a column at `u32::MAX` is already past every line this editor
/// can hold.
#[must_use]
pub fn column_from_utf16(line: &str, character: u32) -> u32 {
    let mut units = 0_u32;
    let mut column = 1_u32;
    for glyph in line.chars() {
        let width = u32::try_from(glyph.len_utf16()).unwrap_or(1);
        if character < units + width {
            return column;
        }
        units += width;
        column += 1;
    }
    column.saturating_add(character - units)
}

/// Phosphor's 1-based character column → LSP's 0-based UTF-16 `character`.
///
/// The exact inverse of [`column_from_utf16`] on every column that names a
/// character: for all `column >= 1`,
/// `column_from_utf16(line, utf16_from_column(line, column)) == column`. The
/// other direction holds for every `character` that starts a character and
/// canonicalises the ones that do not — see [`column_from_utf16`].
///
/// Column 0 does not exist (columns are 1-based), and is treated as column 1
/// rather than underflowing.
///
/// The final addition saturates for [`column_from_utf16`]'s reason, one
/// direction over: a column past the end of a line holding astral characters
/// carries an excess measured in *units*, so `units + (wanted - seen)` exceeds
/// `u32::MAX` for a column near the ceiling — two emoji and `column ==
/// u32::MAX` is enough. This side is not fed by the wire, which is why it is a
/// hardening rather than a fix, and it is here because leaving one half of a
/// pair overflowing is how the next reader concludes the pattern is safe.
#[must_use]
pub fn utf16_from_column(line: &str, column: u32) -> u32 {
    let wanted = column.saturating_sub(1);
    let mut units = 0_u32;
    let mut seen = 0_u32;
    for glyph in line.chars() {
        if seen == wanted {
            return units;
        }
        units += u32::try_from(glyph.len_utf16()).unwrap_or(1);
        seen += 1;
    }
    units.saturating_add(wanted - seen)
}

/// An LSP position against the text it points into.
#[must_use]
pub fn position_from_lsp(text: &str, at: lsp_types::Position) -> Position {
    Position {
        line: at.line.saturating_add(1),
        column: column_from_utf16(line_at(text, at.line), at.character),
    }
}

/// A phosphor position, as the server counts.
#[must_use]
pub fn position_to_lsp(text: &str, at: Position) -> lsp_types::Position {
    let line = at.line.saturating_sub(1);
    lsp_types::Position {
        line,
        character: utf16_from_column(line_at(text, line), at.column),
    }
}

/// An LSP range against the text it points into.
#[must_use]
pub fn span_from_lsp(text: &str, range: lsp_types::Range) -> Span {
    Span {
        start: position_from_lsp(text, range.start),
        end: position_from_lsp(text, range.end),
    }
}

// ---------------------------------------------------------------------------
// What a server says, in phosphor's vocabulary
// ---------------------------------------------------------------------------

/// LSP severity → the three the design draws.
///
/// LSP has four levels and Design Language §1 has three colours, so `HINT` and
/// `INFORMATION` both land on [`Severity::Info`] — meta-grey, *"something
/// happened"*. An **omitted** severity is `Attention` rather than `Trouble`:
/// the specification leaves it to the client, and a diagnostic nobody graded is
/// worth your eyes without being called an error in trouble-red.
#[must_use]
pub fn severity_from_lsp(severity: Option<lsp_types::DiagnosticSeverity>) -> Severity {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => Severity::Trouble,
        Some(lsp_types::DiagnosticSeverity::INFORMATION | lsp_types::DiagnosticSeverity::HINT) => {
            Severity::Info
        }
        _ => Severity::Attention,
    }
}

/// One diagnostic, converted against the text it applies to.
///
/// `text` is that file's contents as the client last saw them — see
/// [`LanguageServers::open`]. Passing `""` is legal and degrades exactly as
/// [`column_from_utf16`] describes.
#[must_use]
pub fn diagnostic_from_lsp(text: &str, diagnostic: &lsp_types::Diagnostic) -> Diagnostic {
    Diagnostic {
        span: span_from_lsp(text, diagnostic.range),
        severity: severity_from_lsp(diagnostic.severity),
        message: diagnostic.message.clone(),
        source: diagnostic.source.clone(),
    }
}

/// A server's `WorkspaceEdit`, flattened into what `apply-workspace-edit`
/// carries — *"a rename, a code action, a format"*.
///
/// `text_of` hands back a file's current contents, which the conversion needs
/// because a `TextEdit`'s range is in UTF-16 units; a file it cannot answer for
/// converts as [`column_from_utf16`] describes against `""`.
///
/// Both shapes are read, because servers disagree about which to send:
/// `changes` (the older map) and `documentChanges` (versioned, which
/// rust-analyzer uses). Creates, renames and deletes inside `documentChanges`
/// are **dropped**, and that is a deliberate refusal rather than an oversight —
/// `FileEdits` describes edits to files that exist, and inventing a file
/// deletion out of a shape the vocabulary cannot express is precisely the kind
/// of mutation an `Ask` capability exists to keep visible.
///
/// The result is sorted by path so the same edit always produces the same
/// `Action`: `changes` is a `HashMap`, and an ask that reorders itself between
/// two runs is an ask nobody can review.
#[must_use]
pub fn file_edits_from_lsp(
    edit: &lsp_types::WorkspaceEdit,
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Vec<FileEdits> {
    let mut out: Vec<FileEdits> = Vec::new();
    let mut push = |uri: &lsp_types::Url, edits: &[lsp_types::TextEdit]| {
        let Ok(path) = uri.to_file_path() else {
            return;
        };
        let text = text_of(&path).unwrap_or_default();
        let converted = edits
            .iter()
            .map(|edit| Edit {
                span: span_from_lsp(&text, edit.range),
                text: edit.new_text.clone(),
            })
            .collect::<Vec<_>>();
        out.push(FileEdits {
            path,
            edits: converted,
        });
    };

    if let Some(changes) = &edit.changes {
        for (uri, edits) in changes {
            push(uri, edits);
        }
    }
    match &edit.document_changes {
        Some(lsp_types::DocumentChanges::Edits(edits)) => {
            for edit in edits {
                let plain = edit
                    .edits
                    .iter()
                    .map(|edit| match edit {
                        lsp_types::OneOf::Left(edit) => edit.clone(),
                        lsp_types::OneOf::Right(annotated) => annotated.text_edit.clone(),
                    })
                    .collect::<Vec<_>>();
                push(&edit.text_document.uri, &plain);
            }
        }
        Some(lsp_types::DocumentChanges::Operations(operations)) => {
            for operation in operations {
                if let lsp_types::DocumentChangeOperation::Edit(edit) = operation {
                    let plain = edit
                        .edits
                        .iter()
                        .map(|edit| match edit {
                            lsp_types::OneOf::Left(edit) => edit.clone(),
                            lsp_types::OneOf::Right(annotated) => annotated.text_edit.clone(),
                        })
                        .collect::<Vec<_>>();
                    push(&edit.text_document.uri, &plain);
                }
            }
        }
        None => {}
    }
    out.sort_by(|left, right| left.path.cmp(&right.path));
    out
}

/// One `Location` as the file-and-span shape phosphor names places with.
///
/// `text_of` supplies the *target* file's contents, which is a different file
/// from the one the question was asked about — go-to-definition's whole point.
/// A file `text_of` has nothing for converts as [`column_from_utf16`] describes
/// against `""`: exact for ASCII, off by the number of astral characters before
/// the column otherwise.
///
/// # That gap is closed at the caller, and this is where the decision is
/// recorded
///
/// `T036` left it open — *"closing it means reading the target from disk, and
/// that is a decision about blocking a runtime thread on IO, not a conversion
/// detail"* — and `T038` made it: **read it, off the blocking pool, before
/// converting.** [`answer`] pre-loads every target file it has no text for
/// through [`read_bounded`], so by the time this function runs `text_of`
/// answers for the target as well as for the open buffers. The premise the
/// deferral rested on is false: `tokio::fs` does not block the runtime thread,
/// it moves the read to the blocking pool, and the request this is answering is
/// already `await`ing a server.
///
/// The function itself stays total and stays honest about `None` — a file that
/// was deleted between the server indexing it and us reading it still converts,
/// approximately, rather than losing the line number as well.
#[must_use]
pub fn file_span_from_lsp(
    location: &lsp_types::Location,
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Option<FileSpan> {
    let path = location.uri.to_file_path().ok()?;
    let text = text_of(&path).unwrap_or_default();
    Some(FileSpan {
        span: Some(span_from_lsp(&text, location.range)),
        path,
    })
}

/// Every `Location` a `textDocument/definition` answer names, in the order the
/// server gave them and before any conversion.
///
/// The response has three shapes and servers use all three — one location, a
/// list, or a list of `LocationLink`s (which rust-analyzer sends when the
/// client advertises link support). A client that reads only the first would
/// silently do nothing against half the ecosystem.
///
/// Separate from [`locations_from_lsp`] because **two callers need the shapes
/// flattened and only one of them wants them converted**: [`answer`] reads the
/// target paths out of a response so it can load their text before converting
/// against it. One match, two users, so the three shapes cannot fall out of step.
#[must_use]
pub fn locations_of(response: &lsp_types::GotoDefinitionResponse) -> Vec<lsp_types::Location> {
    match response {
        lsp_types::GotoDefinitionResponse::Scalar(location) => vec![location.clone()],
        lsp_types::GotoDefinitionResponse::Array(locations) => locations.clone(),
        lsp_types::GotoDefinitionResponse::Link(links) => links
            .iter()
            .map(|link| lsp_types::Location {
                uri: link.target_uri.clone(),
                range: link.target_selection_range,
            })
            .collect(),
    }
}

/// How a server wants `didChange`, out of its `initialize` reply (`T038`).
///
/// **`FULL` when the server said nothing**, which is the conservative reading:
/// a full replacement is a legal change under every sync kind that accepts
/// changes at all, and the alternative — assuming incremental — would send a
/// range to a server that does not read ranges.
#[must_use]
pub fn sync_kind(
    capability: Option<&lsp_types::TextDocumentSyncCapability>,
) -> lsp_types::TextDocumentSyncKind {
    match capability {
        Some(lsp_types::TextDocumentSyncCapability::Kind(kind)) => *kind,
        Some(lsp_types::TextDocumentSyncCapability::Options(options)) => options
            .change
            .unwrap_or(lsp_types::TextDocumentSyncKind::FULL),
        None => lsp_types::TextDocumentSyncKind::FULL,
    }
}

/// The `didChange` content event for a whole-document edit, in the shape this
/// server asked for (`T038`).
///
/// **The editor only ever has the whole text**, and the two shapes are two ways
/// of saying so:
///
/// * `FULL` — one event with no range, which the specification defines as
///   replacing the document.
/// * `INCREMENTAL` — one event whose range covers the *entire previous
///   document*, which is a legal incremental edit and says the same thing. This
///   is what makes the client correct against rust-analyzer and every other
///   server that declares `Incremental`: sending a range-less event to one of
///   those is off-specification, however widely it happens to be tolerated.
/// * `NONE` — `None`. The server asked not to be told, so it is not told.
///
/// `previous` is what the range is measured against: the server's own copy,
/// which is [`Shared::revise`]'s answer and is therefore never in doubt — a
/// change to a document this client never opened is sent as a `didOpen`
/// instead ([`LanguageServers::change`]), so there is no "previous unknown"
/// case for this function to invent an empty document for.
#[must_use]
pub fn change_event(
    kind: lsp_types::TextDocumentSyncKind,
    text: &str,
    previous: &str,
) -> Option<lsp_types::TextDocumentContentChangeEvent> {
    match kind {
        lsp_types::TextDocumentSyncKind::NONE => None,
        lsp_types::TextDocumentSyncKind::INCREMENTAL => {
            Some(lsp_types::TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: end_of(previous),
                }),
                range_length: None,
                text: text.to_owned(),
            })
        }
        // `FULL`, and anything a future protocol version adds: say the whole
        // document, which is the one shape that cannot be misread.
        _ => Some(lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_owned(),
        }),
    }
}

/// The position one past the last character of `text`, in LSP's own units.
///
/// Not [`line_at`], deliberately: that strips a trailing `\r` so a column is
/// measured against the line's *content*, and the end of a document has to
/// count every unit that is actually there or an incremental replacement leaves
/// a stray carriage return behind.
fn end_of(text: &str) -> lsp_types::Position {
    let lines = text.split('\n').count();
    let last = text.rsplit('\n').next().unwrap_or("");
    lsp_types::Position {
        line: u32::try_from(lines.saturating_sub(1)).unwrap_or(u32::MAX),
        character: utf16_len(last),
    }
}

/// A completion response as the float draws it (`T038`).
///
/// Both shapes: servers answer with a bare array or with a `CompletionList`,
/// and rust-analyzer sends the second. **The wire order is kept here and
/// re-ranked by [`narrow`]** — `sortText` and `filterText` are carried on
/// [`Completion`] for exactly that, because a transport that sorted would have
/// to know what the user has typed and it does not.
#[must_use]
pub fn completions_from_lsp(response: &lsp_types::CompletionResponse) -> Vec<Completion> {
    let items = match response {
        lsp_types::CompletionResponse::Array(items) => items.as_slice(),
        lsp_types::CompletionResponse::List(list) => list.items.as_slice(),
    };
    items
        .iter()
        .map(|item| Completion {
            label: item.label.clone(),
            detail: item.detail.clone().or_else(|| {
                item.label_details
                    .as_ref()
                    .and_then(|details| details.detail.clone())
            }),
            documentation: item
                .documentation
                .as_ref()
                .map(documentation_lines)
                .unwrap_or_default(),
            insert: item
                .insert_text
                .clone()
                .unwrap_or_else(|| item.label.clone()),
            // Both default to the label, which is the specification's own rule
            // for each: *"the label is used"* when the field is absent.
            filter: item
                .filter_text
                .clone()
                .unwrap_or_else(|| item.label.clone()),
            sort: item.sort_text.clone().unwrap_or_else(|| item.label.clone()),
        })
        .collect()
}

/// The rows of `items` that `prefix` could still become, in the order the
/// server wants them shown (`T038`).
///
/// **The client filters; the server does not.** That is not a nicety of this
/// build, it is how the protocol is written: a server answers the whole set
/// that could go at a position and re-runs the request only when its
/// `CompletionList` says `isIncomplete`. Without this, one `.` against
/// rust-analyzer answers several hundred rows and the float — which sizes
/// itself to its content — covers the code being typed into. Observed at
/// `CP-4` on a 100×30 terminal: rows 0–28 of 30, the selected row `strict_mul`,
/// against a buffer whose cursor was after `add(1, 2).`.
///
/// **A prefix match, deliberately, and case-insensitively.** `7c` is the
/// specification for this screen and it draws `default()`, `default_delay` and
/// `deserialize` under a typed `de` — three rows that share a prefix, not a
/// fuzzy subsequence ranking. Fuzzy matching is a judgement about ordering that
/// belongs with the Picker's scorer (`T045`) rather than being invented twice;
/// what is here is the floor that makes the float usable and it is written to
/// be replaced.
///
/// **`sortText` decides ties and everything else.** It is the protocol's
/// re-ranking hook — rust-analyzer emits `ffffffff7fffffffdefault` style keys
/// whose order has nothing to do with the label's — so a client that ignored it
/// draws a list in an order the server did not choose, which is what
/// `completions_from_lsp`'s own header used to defer. The sort is **stable**,
/// so two rows with equal `sortText` keep the wire order.
///
/// An empty `prefix` matches everything, which is right: `<C-x>` on a fresh
/// word asks *what can go here* and the answer is the whole set.
#[must_use]
pub fn narrow(items: Vec<Completion>, prefix: &str) -> Vec<Completion> {
    let wanted = prefix.to_lowercase();
    let mut kept: Vec<Completion> = items
        .into_iter()
        .filter(|item| item.filter.to_lowercase().starts_with(&wanted))
        .collect();
    kept.sort_by(|left, right| left.sort.cmp(&right.sort));
    kept
}

/// The signature a `signatureHelp` answer is *about*, with its active parameter
/// resolved to a character range (`T039`).
///
/// `None` when the server sent no signatures, which is how it says *"you are
/// not inside a call"*.
///
/// **The active parameter is per-signature first, per-help second.** LSP 3.16
/// added `SignatureInformation::active_parameter` precisely because the
/// top-level field cannot describe an overload set, and a client that read only
/// the top-level one highlights the wrong argument on any server that sets both.
#[must_use]
pub fn signature_from_lsp(help: &lsp_types::SignatureHelp) -> Option<Signature> {
    let index = help.active_signature.unwrap_or(0) as usize;
    let signature = help
        .signatures
        .get(index)
        .or_else(|| help.signatures.first())?;
    let active = signature
        .active_parameter
        .or(help.active_parameter)
        .and_then(|active| signature.parameters.as_ref()?.get(active as usize))
        .and_then(|parameter| parameter_range(&signature.label, &parameter.label));
    Some(Signature {
        label: signature.label.clone(),
        active,
        documentation: signature
            .documentation
            .as_ref()
            .map(documentation_lines)
            .unwrap_or_default(),
    })
}

/// A parameter's place inside its signature, as a **character** range.
///
/// The two shapes a server may use, and both need converting:
///
/// * `LabelOffsets` are **UTF-16 code units into the signature label** — the
///   same trap the module header is written about, one field over, and
///   [`column_from_utf16`] is already the conversion. A client that used them
///   as character indices would highlight the wrong span of any signature with
///   a non-ASCII identifier in it.
/// * `Simple` is the parameter's own text, which has to be *found* in the
///   label. First occurrence, because that is all the protocol gives; a
///   parameter whose text also appears in an earlier type would highlight the
///   type. Recorded rather than papered over — the offsets form exists to fix
///   exactly this and servers that care use it.
fn parameter_range(label: &str, parameter: &lsp_types::ParameterLabel) -> Option<(usize, usize)> {
    match parameter {
        lsp_types::ParameterLabel::LabelOffsets([start, end]) => {
            let start = column_from_utf16(label, *start).saturating_sub(1) as usize;
            let end = column_from_utf16(label, *end).saturating_sub(1) as usize;
            (start < end).then_some((start, end))
        }
        lsp_types::ParameterLabel::Simple(text) => {
            let byte = label.find(text.as_str())?;
            let start = label[..byte].chars().count();
            Some((start, start + text.chars().count()))
        }
    }
}

/// Hover contents as rows of prose, whichever of the three shapes arrived.
///
/// Markdown is handed over as its source text: §11 is *"nothing ever wraps"*
/// and a hover float is rows, so the rendering question belongs to the
/// transcript's markdown body (`S6`, and it is optional there for `Q4`'s
/// reason). The empty result is how a caller learns there was nothing to say.
#[must_use]
pub fn hover_prose(contents: &lsp_types::HoverContents) -> Vec<String> {
    match contents {
        lsp_types::HoverContents::Scalar(one) => prose_lines(marked_string(one)),
        lsp_types::HoverContents::Array(many) => many
            .iter()
            .flat_map(|one| prose_lines(marked_string(one)))
            .collect(),
        lsp_types::HoverContents::Markup(markup) => prose_lines(&markup.value),
    }
}

/// Server prose as float rows: trailing whitespace trimmed, blank rows dropped.
///
/// **A blank row is not content**, and a float's height is its content (§8) —
/// so a hover whose markdown puts an empty line between two sentences is two
/// rows, not three. The rule is the same for hover prose and for a completion's
/// documentation, which is why it is one function and not two.
fn prose_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .map(str::to_owned)
        .filter(|line| !line.is_empty())
        .collect()
}

/// The text out of a deprecated `MarkedString`, either shape.
fn marked_string(one: &lsp_types::MarkedString) -> &str {
    match one {
        lsp_types::MarkedString::String(text) => text,
        lsp_types::MarkedString::LanguageString(block) => &block.value,
    }
}

/// `Documentation` as rows, either shape — through [`prose_lines`], which is
/// the same rule hover text takes.
fn documentation_lines(documentation: &lsp_types::Documentation) -> Vec<String> {
    prose_lines(match documentation {
        lsp_types::Documentation::String(text) => text.as_str(),
        lsp_types::Documentation::MarkupContent(markup) => markup.value.as_str(),
    })
}

/// Every place a `textDocument/definition` answer names, in the order the
/// server gave them.
#[must_use]
pub fn locations_from_lsp(
    response: &lsp_types::GotoDefinitionResponse,
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Vec<FileSpan> {
    locations_of(response)
        .iter()
        .filter_map(|location| file_span_from_lsp(location, text_of))
        .collect()
}

// ---------------------------------------------------------------------------
// The client
// ---------------------------------------------------------------------------

/// Where a finished piece of work goes: the binary's event queue, as a closure.
///
/// The host supplies one wrapping `events::Poster::post`; this crate cannot name
/// that type, and should not — `Poster` is `pub(crate)` in the binary, and an
/// `Action` is the whole vocabulary either side needs. The [`bool`] is
/// `Poster::post`'s own answer, *"is anyone still listening"*, so a client whose
/// editor has exited learns it rather than queueing into nothing.
pub type Post = Arc<dyn Fn(Action) -> bool + Send + Sync>;

/// *Something about a server changed; draw again.*
///
/// **A wake, not a message**, and separate from [`Post`] because it carries no
/// [`Action`] and could not: a server going from `Starting` to `Ready` mutates
/// nothing the editor owns, has no actor and nothing to refuse, so it is one of
/// the things `phosphor_core::action`'s *"what is deliberately not an Action"*
/// list is about. The binary's queue has a variant for exactly this.
///
/// Called on the client's runtime thread, once per state **transition** — not
/// per event, because the transition rules make several events idempotent
/// (`ServerState::after`) and a wake per redundant event is a redraw per
/// redundant event.
///
/// Without it the editor's picture of a server is correct and stale: nothing
/// else in this process changes when a server dies, so a loop that draws only
/// when a producer speaks would show `starting …` for a server that failed to
/// spawn until the user happened to press a key. [`unwatched`] is the honest
/// answer for a caller that draws nothing.
pub type Woke = Arc<dyn Fn() + Send + Sync>;

/// A [`Woke`] for a caller with no screen to redraw — every test in this crate,
/// and any embedder that polls [`LanguageServers::state`] instead.
#[must_use]
pub fn unwatched() -> Woke {
    Arc::new(|| {})
}

/// Where the answer to a *question* goes — `request-definition`,
/// `request-references`.
///
/// **Not a [`Post`], and the difference is the point.** A diagnostic becomes an
/// `Action` because `ingest-diagnostics` exists; a list of places does not,
/// because no capability in the registry carries one. `open-file` is the
/// closest, and it takes a `PaneRef` — *which pane* is knowledge this crate
/// does not have and must not guess. So the client hands the places back to the
/// caller that asked for them, and the host turns one into an `Action` with the
/// pane it knows about. Inventing a capability to avoid the callback would be
/// the promise `events.rs` refuses to make, one layer down.
///
/// Called on the runtime thread, except on the one path where there is no
/// runtime thread to call it on — see [`LanguageServers::ask`]. Whatever it
/// does must be quick and must not block; posting into the queue is quick.
pub type Locations = Arc<dyn Fn(Vec<FileSpan>) + Send + Sync>;

/// One question's one answer — given, or given empty when this is dropped.
///
/// **The contract [`LanguageServers::ask`] states is kept by this type rather
/// than by every path remembering to keep it**, and it is here because the
/// first review of `T036` found three paths that did not. A question travels
/// through two channels and a spawned task, and each of those has a drop:
///
/// * the supervisor's map still holds the sender of a server task that has
///   ended — a failed spawn, an EOF, a stop — so the send fails and returns the
///   command;
/// * `Attach` and `Restart` `abort()` the task that was going to read it;
/// * the supervisor itself ends when [`LanguageServers`] is dropped, and the
///   commands still queued go with it.
///
/// On every one of those the callback used to be discarded silently, which
/// meant `gd` against an uninstalled server — the state this module's whole
/// thesis is about — left its caller waiting forever. A `Drop` that answers is
/// the difference between a promise and a comment: there is no path that
/// destroys this value without calling `then` exactly once.
/// **Generic over what is answered** since `T038`: [`Lookup`] answers in
/// [`Insight`] rather than in places, and a second copy of this type would be a
/// second chance to get the drop wrong. `T::default()` is the empty answer —
/// `Vec::new()` for places, [`Insight::Nothing`] for a lookup.
struct Answer<T: Default> {
    /// [`None`] once answered, which is what makes "exactly once" true from
    /// both [`give`](Self::give) and [`Drop`].
    then: Option<Arc<dyn Fn(T) + Send + Sync>>,
}

impl<T: Default> Answer<T> {
    /// Wraps the host's callback.
    fn new(then: Arc<dyn Fn(T) + Send + Sync>) -> Self {
        Self { then: Some(then) }
    }

    /// Answers, now. Consuming, so a second answer is not a thing that compiles.
    fn give(mut self, found: T) {
        if let Some(then) = self.then.take() {
            then(found);
        }
    }
}

impl<T: Default> Drop for Answer<T> {
    fn drop(&mut self) {
        if let Some(then) = self.then.take() {
            then(T::default());
        }
    }
}

/// Which question was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// `textDocument/definition` — `request-definition`.
    Definition,
    /// `textDocument/references`, including the declaration —
    /// `request-references`.
    References,
}

// ---------------------------------------------------------------------------
// What the surfaces ask for (`T038`, `T039`)
// ---------------------------------------------------------------------------

/// Which *"tell me about this place"* request was made (`T038`, `T039`).
///
/// Separate from [`Question`], which answers in places. These three answer in
/// [`Insight`] — text about the place rather than another place — and they
/// share a request path because they share a shape: one position, one server,
/// one reply, drawn in one passive float.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    /// `textDocument/completion` — what could go here (`T038`).
    Completion,
    /// `textDocument/signatureHelp` — what the call you are inside takes
    /// (`T039`).
    SignatureHelp,
    /// `textDocument/hover` — what the thing under the cursor is (`T039`).
    Hover,
}

/// One completion the server offered (`T038`).
///
/// `phosphor-ui`'s `CompletionVm` is the drawing half of this and is a
/// different type on purpose: a UI crate's only phosphor dependency may be
/// `phosphor-core` (`crates/phosphor-ui/Cargo.toml`), so the host maps one to
/// the other, exactly as it already does for the statusline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    /// What the list shows — `default()`, `default_delay` (`7c`).
    pub label: String,
    /// The type or shape, drawn in meta-grey right of the label.
    pub detail: Option<String>,
    /// Documentation, one row per line (§11: nothing wraps). Markdown arrives
    /// as its source text; rendering it is the transcript's job at `S6` and a
    /// completion float is one row of prose in `7c`.
    pub documentation: Vec<String>,
    /// What to type when this is accepted.
    ///
    /// **`insertText` or the label, and not `textEdit`.** A `textEdit` carries
    /// its own range, which can start before the cursor and can be a snippet
    /// with tab stops; honouring one is an edit, not a string, and belongs
    /// with whatever Action applies it. Recorded here rather than silently
    /// half-done — a client that read `textEdit`'s `newText` and ignored its
    /// range would insert the *right* text in the *wrong* place, which is worse
    /// than not reading it.
    pub insert: String,
    /// What [`narrow`] matches the typed prefix against — `filterText`, or the
    /// label where the server sent none.
    ///
    /// A separate field because the two differ where it matters: a server may
    /// label a row `default() (RetryPolicy)` and filter it as `default`, and a
    /// client that matched the label would drop the row the moment you typed
    /// the character it was suggesting.
    pub filter: String,
    /// What [`narrow`] orders by — `sortText`, or the label where the server
    /// sent none. Never shown.
    pub sort: String,
}

/// One signature, as signature help gives it (`T039`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Signature {
    /// The callable as the server spells it.
    pub label: String,
    /// The active parameter as a **character** range into [`label`], converted
    /// off LSP's UTF-16 offsets here so no surface above has to know that
    /// encoding exists. See the module header.
    ///
    /// [`label`]: Signature::label
    pub active: Option<(usize, usize)>,
    /// Documentation for the active parameter, or for the signature.
    pub documentation: Vec<String>,
}

/// What a [`Lookup`] answered.
///
/// [`Insight::Nothing`] is the default and covers every way of not answering —
/// no server, no reply inside the timeout, a reply with no content — so a
/// caller has one arm for *"there is nothing to show"* rather than three.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Insight {
    /// `T038`'s list, in the order the server ranked it.
    Completions(Vec<Completion>),
    /// `T039`'s signature help: the active signature, if the server named one.
    Signature(Box<Signature>),
    /// `T039`'s hover prose, one row per line.
    Hover(Vec<String>),
    /// Nothing to show.
    #[default]
    Nothing,
}

/// Where a [`Lookup`]'s answer goes.
///
/// Same contract as [`Locations`] and kept by the same type: **exactly one
/// call, on every path**, including a server that never replies and a task
/// aborted by a restart.
pub type Insights = Arc<dyn Fn(Insight) + Send + Sync>;

/// The version a `didOpen` claims. Every `didChange` sends one more.
const FIRST_VERSION: i32 = 1;

/// One document as the client knows it: whose server it belongs to, what that
/// server was last told, and which version we called it.
///
/// **The version is not decoration.** `didChange` carries a version and the
/// specification says it increases; a server that sees 1, 1, 1 is entitled to
/// treat later changes as stale and answer completions for text the user is no
/// longer looking at, which is the exact failure `T038` closes.
///
/// **The language is not decoration either.** It is what makes a replay
/// possible: a restarted server has to be told about *its* documents and no
/// others, and the map is keyed by path (see [`Shared::documents`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Document {
    language: LanguageId,
    text: String,
    /// `didOpen` sends [`FIRST_VERSION`] and every `didChange` sends one more.
    version: i32,
}

/// State every server task shares with the editor's side.
struct Shared {
    states: Mutex<HashMap<LanguageId, ServerState>>,
    /// `completionProvider.triggerCharacters`, per language, as the server
    /// answered `initialize` (`T038`).
    ///
    /// **Beside [`ServerState`] rather than inside [`ServerState::Ready`]**:
    /// `Ready` carries the server's *identity*, which the statusline chip draws
    /// and five call sites match on, and a second payload there would make every
    /// one of them a place to keep updated. This is one map with one reader.
    triggers: Mutex<HashMap<LanguageId, Vec<String>>>,
    /// Every document handed to a server, keyed by absolute path. The client
    /// keeps the text because it is the only thing that can convert a UTF-16
    /// column, and the version because `didChange` needs one (`T038`).
    documents: Mutex<HashMap<PathBuf, Document>>,
    post: Post,
    woke: Woke,
}

impl Shared {
    /// Applies one [`ServerEvent`] to a language's state.
    fn record(&self, language: &LanguageId, event: &ServerEvent) {
        let mut states = self.states.lock().unwrap_or_else(|poison| {
            // A poisoned lock here means a server task panicked mid-update.
            // The map is a map of states, not an invariant across entries, so
            // the honest move is to keep going rather than take the editor down
            // with the task — `CatchUnwindLayer` is async-lsp's version of the
            // same judgement.
            poison.into_inner()
        });
        let current = states.get(language).cloned().unwrap_or_default();
        let next = current.after(event);
        let moved = next != current;
        states.insert(language.clone(), next);
        // Outside the lock would be tidier and is wrong: `record` is called
        // from the runtime thread and the wake is what makes the editor redraw,
        // so a caller that took the lock again to read the state it just set
        // would be the ordering this avoids. The callback posts into a channel
        // and returns; it does not draw.
        drop(states);
        if moved {
            (self.woke)();
        }
    }

    fn state(&self, language: &LanguageId) -> ServerState {
        self.states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(language)
            .cloned()
            .unwrap_or_default()
    }

    fn text_of(&self, path: &Path) -> Option<String> {
        self.documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(path)
            .map(|document| document.text.clone())
    }

    /// Records what this server says asks for a completion list.
    ///
    /// Written on every `initialize` answer, empty list included, so a restart
    /// onto a server that advertises fewer characters cannot leave the old set
    /// behind.
    fn set_triggers(&self, language: &LanguageId, triggers: Vec<String>) {
        self.triggers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(language.clone(), triggers);
    }

    fn triggers(&self, language: &LanguageId) -> Vec<String> {
        self.triggers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(language)
            .cloned()
            .unwrap_or_default()
    }

    /// Records `text` for `path` and hands back what to send.
    ///
    /// **One lock, one decision.** The version has to be read and written
    /// without anything in between, or two edits in the same frame can both
    /// claim the same number — which is the one thing a version is for.
    fn revise(&self, language: &LanguageId, path: &Path, text: String) -> Revision {
        let mut documents = self
            .documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match documents.get_mut(path) {
            Some(document) => {
                let previous = core::mem::replace(&mut document.text, text);
                document.version = document.version.saturating_add(1);
                Revision::Changed {
                    previous,
                    version: document.version,
                }
            }
            // A change to a document nobody opened is recorded as the open it
            // should have been — and *sent* as one too. The alternative is
            // dropping the text the conversion needs on the floor, or sending
            // a `didChange` for a document the server has no copy of, which the
            // specification does not define.
            None => {
                documents.insert(
                    path.to_path_buf(),
                    Document {
                        language: language.clone(),
                        text,
                        version: FIRST_VERSION,
                    },
                );
                Revision::Opened
            }
        }
    }

    /// Every document this client holds for `language`, as a fresh server has
    /// to be told about them: path, text, and the version the client is on.
    ///
    /// The map is keyed by path because that is how every other caller reaches
    /// it; this is the one place that wants it the other way round, and a list
    /// built once per restart is cheaper than a second index kept in step.
    fn documents_of(&self, language: &LanguageId) -> Vec<(PathBuf, String, i32)> {
        self.documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(_, document)| document.language == *language)
            .map(|(path, document)| (path.clone(), document.text.clone(), document.version))
            .collect()
    }
}

/// What [`Shared::revise`] decided, which is also what goes on the wire.
///
/// Two outcomes rather than an `Option<String>`, because they are two different
/// notifications: `didChange` is defined only for a document the server has
/// been sent a `didOpen` for.
enum Revision {
    /// The client had this document; here is what the server's copy said before
    /// the edit, and the version this change claims.
    Changed { previous: String, version: i32 },
    /// The client did not, so this is the open it should have been.
    Opened,
}

/// What the editor's side asks the runtime thread to do.
///
/// **Named fields, like every [`Action`] variant in `phosphor-core`**, and for
/// the reason that vocabulary chose them: a five-field tuple needs a prose
/// comment to say which `String` is the new text and which is the old one, and
/// a comment is not what the compiler checks.
///
/// [`Action`]: phosphor_core::action::Action
enum Command {
    Attach {
        spec: Box<ServerSpec>,
        root: PathBuf,
    },
    Open {
        language: LanguageId,
        path: PathBuf,
        text: String,
    },
    /// `T038`.
    Change {
        language: LanguageId,
        path: PathBuf,
        /// What the buffer says now.
        text: String,
        /// What the server's copy said before this edit — the range an
        /// incremental change is measured against.
        previous: String,
        /// The version this change claims. Rises by one per edit.
        version: i32,
    },
    Close {
        language: LanguageId,
        path: PathBuf,
    },
    Ask {
        language: LanguageId,
        question: Question,
        path: PathBuf,
        at: Position,
        answer: Answer<Vec<FileSpan>>,
    },
    Look {
        language: LanguageId,
        lookup: Lookup,
        path: PathBuf,
        at: Position,
        answer: Answer<Insight>,
    },
    Restart(LanguageId),
    Stop(LanguageId),
}

/// What one server's task is asked to do, once it is up. The same vocabulary as
/// [`Command`] with the language stripped: a task serves exactly one.
enum ServerCommand {
    Open {
        path: PathBuf,
        text: String,
        /// [`FIRST_VERSION`] for a document the editor just opened, and
        /// whatever the client is on for one replayed into a restarted server.
        version: i32,
    },
    Change {
        path: PathBuf,
        text: String,
        previous: String,
        version: i32,
    },
    Close {
        path: PathBuf,
    },
    Ask {
        question: Question,
        path: PathBuf,
        at: Position,
        answer: Answer<Vec<FileSpan>>,
    },
    Look {
        lookup: Lookup,
        path: PathBuf,
        at: Position,
        answer: Answer<Insight>,
    },
    Stop,
}

/// Every language server this editor is running.
///
/// One thread, one runtime, one child process per language. Every method here
/// returns without waiting for anything: see the module header.
pub struct LanguageServers {
    shared: Arc<Shared>,
    /// [`Option`] for exactly one reason, and it is [`Drop`]'s: the supervisor
    /// loop ends when the **last** sender is gone, so the sender has to be
    /// destroyed before the join and a field cannot be moved out of `&mut self`.
    /// A `downgrade()` that is discarded looks like it closes the channel and
    /// does not — the strong sender is still in the struct, and the join
    /// deadlocks.
    commands: Option<UnboundedSender<Command>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl fmt::Debug for LanguageServers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let states = self
            .shared
            .states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        formatter
            .debug_struct("LanguageServers")
            .field("states", &*states)
            .finish_non_exhaustive()
    }
}

impl LanguageServers {
    /// Starts the runtime thread. Nothing is spawned until [`attach`].
    ///
    /// [`attach`]: Self::attach
    #[must_use]
    pub fn start(post: Post, woke: Woke) -> Self {
        let shared = Arc::new(Shared {
            states: Mutex::new(HashMap::new()),
            triggers: Mutex::new(HashMap::new()),
            documents: Mutex::new(HashMap::new()),
            post,
            woke,
        });
        let (commands, receiver) = unbounded_channel();
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("phosphor-lsp".to_owned())
                .spawn(move || supervise(&shared, receiver))
                .ok()
        };
        Self {
            shared,
            commands: Some(commands),
            thread,
        }
    }

    /// Hands one command to the runtime thread, and never waits.
    fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            drop(commands.send(command));
        }
    }

    /// Starts `spec`'s server, rooted at `root`. Replaces whatever was running
    /// for that language.
    ///
    /// Returns immediately; the server is [`ServerState::Starting`] from the
    /// next observation until it is not.
    pub fn attach(&self, spec: ServerSpec, root: PathBuf) {
        self.send(Command::Attach {
            spec: Box::new(spec),
            root,
        });
    }

    /// Tells a language's server about a file and its contents — `didOpen`.
    ///
    /// **The text is recorded here, on the caller's thread, before anything is
    /// sent.** It is the only thing that can turn a server's UTF-16 columns
    /// back into columns (see the module header), and a server can publish
    /// diagnostics the instant it hears about a file — so recording it on the
    /// runtime thread would be a race between our own two messages, won
    /// silently and in the wrong direction. [`text_of`] is true the moment this
    /// returns.
    ///
    /// A language with no server running records the text anyway. That is not
    /// a special case: the conversion is the same one when a server attaches
    /// later, and the alternative is a document the client cannot describe.
    ///
    /// [`text_of`]: Self::text_of
    pub fn open(&self, language: &LanguageId, path: PathBuf, text: String) {
        self.shared
            .documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                path.clone(),
                Document {
                    language: language.clone(),
                    text: text.clone(),
                    version: FIRST_VERSION,
                },
            );
        self.send(Command::Open {
            language: language.clone(),
            path,
            text,
        });
    }

    /// Tells a language's server that a file changed — `didChange` (`T038`).
    ///
    /// **This is the whole of `T038`'s document-sync half, and it is why the
    /// completion float can be trusted.** `T036` sent `didOpen` and nothing
    /// after it, so every request against a file the user had typed into asked
    /// about the text as it was when the buffer opened. Completions for a
    /// prefix that is no longer there is not a stale-looking list; it is a
    /// *wrong* one, and nothing on screen says so.
    ///
    /// Takes the **whole** text, because that is what the editor has. Whether
    /// the server is told so as a full replacement or as one edit spanning the
    /// old document is negotiated per server from its `textDocumentSync`
    /// capability — see [`change_event`] — and neither shape needs the caller
    /// to know which happened.
    ///
    /// The text is recorded here, on the caller's thread, before anything is
    /// sent, for the reason [`open`] gives.
    ///
    /// **A change to a document this client never opened is sent as the
    /// `didOpen` it should have been**, not as a `didChange`: the specification
    /// defines `didChange` only for an open document, and [`close`] has always
    /// guarded the same case in the other direction (it sends nothing for a
    /// path it was not holding). The editor reaches this when a buffer was
    /// opened before any server attached.
    ///
    /// [`open`]: Self::open
    /// [`close`]: Self::close
    pub fn change(&self, language: &LanguageId, path: PathBuf, text: String) {
        let command = match self.shared.revise(language, &path, text.clone()) {
            Revision::Changed { previous, version } => Command::Change {
                language: language.clone(),
                path,
                text,
                previous,
                version,
            },
            Revision::Opened => Command::Open {
                language: language.clone(),
                path,
                text,
            },
        };
        self.send(command);
    }

    /// Tells a language's server the file is no longer ours — `didClose`
    /// (`T038`).
    ///
    /// After this the server falls back to what is on disk, which is the
    /// specification's own rule and the reason the record goes too: a client
    /// that kept its copy would keep converting columns against a document it
    /// has disclaimed.
    ///
    /// Closing a file that was never opened is a no-op, not an error. The
    /// editor closes buffers it opened before a server attached.
    pub fn close(&self, language: &LanguageId, path: &Path) {
        let known = self
            .shared
            .documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(path)
            .is_some();
        if known {
            self.send(Command::Close {
                language: language.clone(),
                path: path.to_path_buf(),
            });
        }
    }

    /// Asks a server where a symbol is defined, or what refers to it —
    /// `request-definition` and `request-references`.
    ///
    /// Returns immediately, like everything else here. `then` is called when
    /// the answer arrives, with an empty list when the server has no answer,
    /// has crashed, was stopped, was never started, or does not reply inside
    /// [`ServerSpec::ready_timeout`] — **a question always gets exactly one
    /// answer, so a caller never has to time itself out.**
    ///
    /// That promise is [`Answer`]'s, structurally, and not a rule each path
    /// keeps: every way of destroying a question answers it. The one visible
    /// consequence is *which thread* calls `then` — the runtime thread in every
    /// ordinary case, but the caller's own thread when there is no runtime
    /// thread left to reach (the client is being dropped, or its thread never
    /// spawned). A callback that only posts into a queue, which is what this
    /// type exists for, does not care.
    pub fn ask(
        &self,
        language: &LanguageId,
        question: Question,
        path: PathBuf,
        at: Position,
        then: Locations,
    ) {
        self.send(Command::Ask {
            language: language.clone(),
            question,
            path,
            at,
            answer: Answer::new(then),
        });
    }

    /// Asks a server about the place under the cursor — completion (`T038`),
    /// signature help and hover (`T039`).
    ///
    /// The same promise [`ask`] makes and for the same structural reason:
    /// **exactly one answer per request**, [`Insight::Nothing`] when there is
    /// nothing to say, so a surface never has to time itself out. What comes
    /// back is text about a place rather than another place, which is the only
    /// difference and the reason this is not a fourth [`Question`].
    ///
    /// [`ask`]: Self::ask
    pub fn look_up(
        &self,
        language: &LanguageId,
        lookup: Lookup,
        path: PathBuf,
        at: Position,
        then: Insights,
    ) {
        self.send(Command::Look {
            language: language.clone(),
            lookup,
            path,
            at,
            answer: Answer::new(then),
        });
    }

    /// `restart-language-server`. Kills the process and starts a new one from
    /// the same spec.
    pub fn restart(&self, language: &LanguageId) {
        self.send(Command::Restart(language.clone()));
    }

    /// Asks a server to shut down. The state becomes [`ServerState::Stopped`],
    /// not [`ServerState::Crashed`].
    pub fn stop(&self, language: &LanguageId) {
        self.send(Command::Stop(language.clone()));
    }

    /// Where a language's server is. **The editor's only question**, and it
    /// takes a mutex for the length of a `HashMap` lookup.
    #[must_use]
    pub fn state(&self, language: &LanguageId) -> ServerState {
        self.shared.state(language)
    }

    /// The text this client last sent to a server for `path`.
    #[must_use]
    pub fn text_of(&self, path: &Path) -> Option<String> {
        self.shared.text_of(path)
    }

    /// What this language's server says raises a completion list on its own —
    /// `completionProvider.triggerCharacters` (`T038`). Typically `.` and the
    /// scope operator in a dotted language; empty is legal and common.
    ///
    /// Empty until `initialize` answers, and for every server that advertises
    /// none. **The editor needs these because its typing gate is a floor on
    /// identifier prefixes**: `foo.` has a prefix of zero word characters, so
    /// without the server's own list the most common completion moment in a
    /// dotted language is unreachable except by `<C-x>`.
    #[must_use]
    pub fn completion_triggers(&self, language: &LanguageId) -> Vec<String> {
        self.shared.triggers(language)
    }
}

impl Drop for LanguageServers {
    /// Ends the runtime thread and every child with it.
    ///
    /// Dropping the sender ends [`supervise`]'s loop, which drops the runtime,
    /// which drops every task, which drops every `Child` — and each was spawned
    /// `kill_on_drop`, so a hung server is killed rather than outliving the
    /// editor. Joining is bounded by that and by nothing else: no task is
    /// awaited on the way out.
    fn drop(&mut self) {
        // Order matters and is the whole of this function: the sender goes
        // first, because that is what `supervise`'s `recv().await` is waiting
        // on, and joining before it would wait forever.
        self.commands = None;
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

/// The runtime thread: one `current_thread` runtime, one supervisor loop.
fn supervise(shared: &Arc<Shared>, mut commands: UnboundedReceiver<Command>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        let mut running: HashMap<LanguageId, UnboundedSender<ServerCommand>> = HashMap::new();
        let mut specs: HashMap<LanguageId, (ServerSpec, PathBuf)> = HashMap::new();
        let mut tasks: HashMap<LanguageId, tokio::task::JoinHandle<()>> = HashMap::new();

        while let Some(command) = commands.recv().await {
            match command {
                Command::Attach { spec, root } => {
                    let language = spec.language.clone();
                    specs.insert(language.clone(), ((*spec).clone(), root.clone()));
                    if let Some(task) = tasks.remove(&language) {
                        task.abort();
                    }
                    let (sender, receiver) = unbounded_channel();
                    replay(shared, &language, &sender);
                    running.insert(language.clone(), sender);
                    let shared = Arc::clone(shared);
                    tasks.insert(
                        language,
                        tokio::spawn(async move { serve(*spec, root, shared, receiver).await }),
                    );
                }
                Command::Restart(language) => {
                    let Some((spec, root)) = specs.get(&language).cloned() else {
                        continue;
                    };
                    if let Some(task) = tasks.remove(&language) {
                        task.abort();
                    }
                    shared.record(&language, &ServerEvent::Restarted);
                    let (sender, receiver) = unbounded_channel();
                    replay(shared, &language, &sender);
                    running.insert(language.clone(), sender);
                    let shared = Arc::clone(shared);
                    tasks.insert(
                        language,
                        tokio::spawn(async move { serve(spec, root, shared, receiver).await }),
                    );
                }
                // The text is already recorded — `LanguageServers::open` does
                // it before sending, and says why. The same silence in all
                // three notification arms when no server is running: a language
                // with no server has nothing to be told, and the record stands.
                Command::Open {
                    language,
                    path,
                    text,
                } => forward(
                    &mut running,
                    &language,
                    ServerCommand::Open {
                        path,
                        text,
                        version: FIRST_VERSION,
                    },
                ),
                // `T038`.
                Command::Change {
                    language,
                    path,
                    text,
                    previous,
                    version,
                } => forward(
                    &mut running,
                    &language,
                    ServerCommand::Change {
                        path,
                        text,
                        previous,
                        version,
                    },
                ),
                Command::Close { language, path } => {
                    forward(&mut running, &language, ServerCommand::Close { path });
                }
                // A question to a language with no server running is answered
                // with nothing, here, rather than dropped. `ask`'s contract is
                // one answer per question, and "there is no server" is an
                // answer a caller can act on. **Not `forward`**: these two carry
                // an `Answer`, and the empty one differs per question type — so
                // the shape stays written out twice rather than made generic
                // over a thing that has two inhabitants.
                Command::Ask {
                    language,
                    question,
                    path,
                    at,
                    answer,
                } => {
                    let unsent = match running.get(&language) {
                        Some(sender) => sender
                            .send(ServerCommand::Ask {
                                question,
                                path,
                                at,
                                answer,
                            })
                            .err(),
                        None => {
                            answer.give(Vec::new());
                            None
                        }
                    };
                    if let Some(unsent) = unsent {
                        // **The entry outlived its task.** A server that failed
                        // to spawn, hit EOF or was stopped leaves its sender
                        // here with nothing reading it, and this is the arm a
                        // question then takes — the one the review found
                        // silently dropping the callback. Dropping the command
                        // handed back *is* the answer (see `Answer`), and the
                        // entry goes with it so the next question takes the
                        // `None` arm directly.
                        drop(unsent);
                        running.remove(&language);
                    }
                }
                // The same three paths as `Ask`, one answer type over. Dropping
                // the command handed back *is* the answer — see `Answer`.
                Command::Look {
                    language,
                    lookup,
                    path,
                    at,
                    answer,
                } => {
                    let unsent = match running.get(&language) {
                        Some(sender) => sender
                            .send(ServerCommand::Look {
                                lookup,
                                path,
                                at,
                                answer,
                            })
                            .err(),
                        None => {
                            answer.give(Insight::Nothing);
                            None
                        }
                    };
                    if let Some(unsent) = unsent {
                        drop(unsent);
                        running.remove(&language);
                    }
                }
                // The entry is removed rather than kept: after a stop there is
                // no server for this language until something attaches one, and
                // a sender nobody reads is exactly how a question stops getting
                // an answer.
                Command::Stop(language) => {
                    if let Some(sender) = running.remove(&language) {
                        drop(sender.send(ServerCommand::Stop));
                    }
                }
            }
        }
    });
}

/// Hands one notification to a language's server, and drops the entry when the
/// task behind it is gone.
///
/// **The entry can outlive its task** — a server that failed to spawn, hit EOF
/// or was stopped leaves a sender here with nothing reading it — so every send
/// is also the check. Written once because it was written three times, and the
/// third copy is where a missing `running.remove` would have hidden.
fn forward(
    running: &mut HashMap<LanguageId, UnboundedSender<ServerCommand>>,
    language: &LanguageId,
    command: ServerCommand,
) {
    let gone = running
        .get(language)
        .is_some_and(|sender| sender.send(command).is_err());
    if gone {
        running.remove(language);
    }
}

/// Tells a **fresh** server about every document the client already holds for
/// its language, before it is handed anything else.
///
/// **Without this a restart is silent data loss.** `restart-language-server`
/// aborted the task and spawned a new process, and the first thing that process
/// heard about an open file was a `didChange` at version 2 for a document it
/// had never been sent — so every completion, signature and hover afterwards
/// was answered against a document the server did not have, with nothing on
/// screen saying so. `attach` has the same hole from the other side: the editor
/// opens a buffer and *then* discovers which server serves it.
///
/// The commands queue on the new task's channel and are read after
/// `initialize`, which is what makes this correct rather than racy: [`drive`]
/// does not touch the receiver until the server has replied.
fn replay(shared: &Shared, language: &LanguageId, sender: &UnboundedSender<ServerCommand>) {
    for (path, text, version) in shared.documents_of(language) {
        // The client's version continues rather than restarting at 1: the
        // records are not reset, so the next `didChange` claims one more than
        // this, and a server that saw 1 after 5 would be entitled to call the
        // next edit stale.
        //
        // An edit made *while* the restart is in flight is read here or is
        // still behind us in the command queue — either the replay already
        // carries it, in which case the `didChange` that follows repeats the
        // version with the same text, or it does not and the change is
        // genuinely newer. Both leave the server holding the buffer's text,
        // which is the only thing this is for.
        drop(sender.send(ServerCommand::Open {
            path,
            text,
            version,
        }));
    }
}

// ---------------------------------------------------------------------------
// The transport, bounded
// ---------------------------------------------------------------------------
//
// Everything in this section is `pub` so that `F5`
// (`fuzz/fuzz_targets/lsp_wire.rs`) reaches the **shipping** scanner rather
// than a copy of it — a fuzz target over a reimplementation proves things about
// the reimplementation. The two constants are values, not settings: exporting
// the number a client refuses at lets a test name the boundary, and still
// leaves no way for a host to raise it, which is the property the module header
// asks for.

/// The largest frame a server may declare, in bytes.
///
/// **This number exists because the alternative is `abort()`.** `async-lsp`
/// reads `Content-Length: N` and then does `vec![0u8; N]` before a byte of the
/// body arrives, with no bound on `N` — so `Content-Length: 999999999999999` is
/// an allocation failure, and Rust's answer to one is to abort the process: no
/// unwind, no [`ServerState::Crashed`], no editor. A review of `T036` found it
/// by writing the two-line server that does it, and
/// `tests/lsp.rs::an_absurd_content_length_is_a_crash_and_not_an_abort` is that
/// server.
///
/// 64 MiB rather than something snug, because the failure this bounds is
/// "absurd", not "large": a workspace-wide rename or a semantic-tokens response
/// for a generated file is legitimately megabytes, and a client that refused
/// one would be a worse bug than the one being fixed. What it rules out is a
/// declaration no honest server makes.
pub const MAX_FRAME_BYTES: u64 = 64 * 1024 * 1024;

/// The longest header line this client will accumulate before refusing.
///
/// The same shape one field over: `async-lsp` reads a header line into a
/// `String` with `read_line`, which is unbounded, so a server that writes
/// forever without a newline is the slow version of the same abort. LSP headers
/// are two fields and neither is long.
pub const MAX_HEADER_BYTES: usize = 8 * 1024;

/// A reader that reads the frame headers as they go past, and refuses one that
/// declares more than [`MAX_FRAME_BYTES`].
///
/// It is a byte-for-byte pass-through: the bytes reach `async-lsp` unchanged
/// and this only ever *fails* the read, which the main loop reports as the
/// error that ends the server. Refusing before the allocation is the whole
/// point, so the check has to be here — under the framing, above the pipe —
/// rather than in any code we could add on top of it.
#[derive(Debug)]
pub struct Bounded<R> {
    inner: R,
    scan: FrameScan,
}

impl<R> Bounded<R> {
    /// Wraps a reader — a child's stdout in the client, a `Cursor` in `F5`.
    pub const fn new(inner: R) -> Self {
        Self {
            inner,
            scan: FrameScan::new(),
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Bounded<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buffer.filled().len();
        ready!(Pin::new(&mut this.inner).poll_read(context, buffer))?;
        this.scan.inspect(&buffer.filled()[before..])?;
        Poll::Ready(Ok(()))
    }
}

/// Where the reader is in the stream of frames.
///
/// The one thing it must get exactly right is the *body*: a JSON payload can
/// contain the text `Content-Length: 999999999999999` inside a string, and a
/// scanner that read bodies as headers would kill a working server over a
/// diagnostic message. So bodies are counted, not parsed — `left` bytes are
/// skipped wholesale — and the count comes from the same header field
/// `async-lsp` will `read_exact` with, which keeps the two in step by
/// construction.
///
/// **Public for `F5`, which states that claim as a law rather than taking it.**
/// The target frames the same bytes by `async-lsp` 0.2.4's own rules and
/// asserts this scanner is at a frame boundary wherever the framer is; see
/// [`mid_frame`](Self::mid_frame), which is the whole of what it needs to look
/// at.
#[derive(Debug)]
pub struct FrameScan {
    /// The header line being accumulated, bounded by [`MAX_HEADER_BYTES`].
    line: Vec<u8>,
    /// The `Content-Length` of the frame whose headers are being read.
    declared: Option<u64>,
    /// Body bytes still to skip.
    left: u64,
}

impl Default for FrameScan {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameScan {
    /// A scanner at the start of a stream, expecting headers.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            line: Vec::new(),
            declared: None,
            left: 0,
        }
    }

    /// Is there a body still being skipped?
    ///
    /// `false` means the next byte is a header byte — the one observable that
    /// says whether this scanner and `async-lsp`'s framer agree about where a
    /// frame ends, which is the desync `F5` searches for.
    #[must_use]
    pub const fn mid_frame(&self) -> bool {
        self.left > 0
    }

    /// Reads the bytes that just went past. Chunk boundaries are arbitrary —
    /// this is a stream, so a header may arrive one byte at a time.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::InvalidData`] for a `Content-Length` past
    /// [`MAX_FRAME_BYTES`] or a header line past [`MAX_HEADER_BYTES`] — the two
    /// declarations that would otherwise be an allocation the process cannot
    /// survive. Nothing else is refused: a malformed header is `async-lsp`'s to
    /// reject, with a better message than this type has.
    pub fn inspect(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            if self.left > 0 {
                let taken = usize::try_from(self.left)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                self.left -= u64::try_from(taken).unwrap_or(self.left);
                bytes = &bytes[taken..];
                continue;
            }
            match bytes.iter().position(|byte| *byte == b'\n') {
                Some(end) => {
                    self.push(&bytes[..=end])?;
                    bytes = &bytes[end + 1..];
                    self.finish_line()?;
                }
                None => {
                    self.push(bytes)?;
                    bytes = &[];
                }
            }
        }
        Ok(())
    }

    fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.line.len().saturating_add(bytes.len()) > MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("a header line longer than {MAX_HEADER_BYTES} bytes, and no frame yet"),
            ));
        }
        self.line.extend_from_slice(bytes);
        Ok(())
    }

    /// A complete header line. The blank one ends the headers and starts the
    /// body — which is where the declared length becomes a length we skip, and
    /// where a refusal has to happen before `async-lsp` reads the same field.
    fn finish_line(&mut self) -> io::Result<()> {
        let line = std::mem::take(&mut self.line);
        let line = line.strip_suffix(b"\n").unwrap_or(&line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            self.left = self.declared.take().unwrap_or(0);
            return Ok(());
        }
        let Some(colon) = line.iter().position(|byte| *byte == b':') else {
            // Not a header at all. `async-lsp` rejects it as a protocol error,
            // which is the right answer and not this type's to give.
            return Ok(());
        };
        if !line[..colon].eq_ignore_ascii_case(b"content-length") {
            return Ok(());
        }
        // An unparseable length is left alone for the same reason: `async-lsp`
        // parses the same bytes and fails the same way, with a better message.
        let Some(declared) = std::str::from_utf8(&line[colon + 1..])
            .ok()
            .map(str::trim)
            .and_then(|value| value.parse::<u64>().ok())
        else {
            return Ok(());
        };
        if declared > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Content-Length: {declared} is past this client's {MAX_FRAME_BYTES}-byte frame limit"
                ),
            ));
        }
        self.declared = Some(declared);
        Ok(())
    }
}

/// One server, from spawn to exit.
///
/// The `select!` is the shape that matters: `served` is async-lsp's main loop
/// reading the child's stdout, and `driven` is our conversation with it. They
/// run together, and **whichever finishes first ends the server** — a crash
/// (the main loop hitting EOF) does not wait for a request to time out, and a
/// stop does not wait for the pipe to close.
async fn serve(
    spec: ServerSpec,
    root: PathBuf,
    shared: Arc<Shared>,
    commands: UnboundedReceiver<ServerCommand>,
) {
    shared.record(&spec.language, &ServerEvent::Attached);

    let mut child = match tokio::process::Command::new(&spec.command)
        .args(&spec.args)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // A server's stderr is its own log. Inheriting it would write over the
        // frame — the terminal belongs to the editor, and Design Language §8's
        // "torn frame = P0" is not negotiable for a diagnostic message.
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            shared.record(
                &spec.language,
                &ServerEvent::Failed(Failure::Spawn(error.to_string())),
            );
            return;
        }
    };
    let (Some(stdout), Some(stdin)) = (child.stdout.take(), child.stdin.take()) else {
        shared.record(
            &spec.language,
            &ServerEvent::Failed(Failure::Spawn("no pipes on the child".to_owned())),
        );
        return;
    };

    let (mainloop, socket) = MainLoop::new_client(|_server| router(&shared, &root));
    // `Bounded` is between the pipe and the framing, and nowhere else can be:
    // see `MAX_FRAME_BYTES`.
    let served = mainloop.run_buffered(Bounded::new(stdout).compat(), stdin.compat_write());
    let driven = drive(socket, &spec, &root, &shared, commands);
    tokio::pin!(served, driven);

    tokio::select! {
        outcome = &mut served => {
            // The pipe closed. `record` refuses to call this a crash if we had
            // already asked the server to stop.
            let why = match outcome {
                Ok(()) => "the server closed its side".to_owned(),
                Err(error) => error.to_string(),
            };
            shared.record(&spec.language, &ServerEvent::Failed(Failure::Exited(why)));
        }
        () = &mut driven => {}
    }
}

/// The client's own conversation: initialize, then whatever the editor asks.
async fn drive(
    mut socket: ServerSocket,
    spec: &ServerSpec,
    root: &Path,
    shared: &Arc<Shared>,
    mut commands: UnboundedReceiver<ServerCommand>,
) {
    // How this server wants `didChange`, read out of its `initialize` reply and
    // kept here rather than in `Shared` — it is one server's answer, this task
    // is that server's, and nothing on the editor's side has a use for it.
    // Uninitialised on purpose: every arm below that does not set it returns,
    // so the compiler proves there is no path to a command loop that guessed.
    let sync;
    let initialize = socket.initialize(initialize_params(spec, root));
    match tokio::time::timeout(spec.ready_timeout, initialize).await {
        Err(_elapsed) => {
            shared.record(&spec.language, &ServerEvent::Failed(Failure::Timeout));
            return;
        }
        Ok(Err(error)) => {
            shared.record(
                &spec.language,
                &ServerEvent::Failed(Failure::Protocol(error.to_string())),
            );
            return;
        }
        Ok(Ok(result)) => {
            // **The answer to the offer, read.** The client declares UTF-16 and
            // nothing else, so the specification leaves a server two legal
            // replies: that same kind, or silence (which *is* UTF-16, the
            // protocol's default). Anything else is a server counting columns
            // in units this module cannot convert — every diagnostic on a
            // non-ASCII line silently in the wrong place, which is the exact
            // bug class the module header is written against. A refusal with
            // the encoding named in it is the legible version of that, and a
            // review found this unchecked: the declaration was made and the
            // reply was never looked at.
            if let Some(encoding) = &result.capabilities.position_encoding
                && *encoding != lsp_types::PositionEncodingKind::UTF16
            {
                shared.record(
                    &spec.language,
                    &ServerEvent::Failed(Failure::Protocol(format!(
                        "server chose positionEncoding {:?}; phosphor's client offered only {:?}",
                        encoding.as_str(),
                        lsp_types::PositionEncodingKind::UTF16.as_str()
                    ))),
                );
                return;
            }
            sync = sync_kind(result.capabilities.text_document_sync.as_ref());
            // **What asks for a list, in the server's own words** (`T038`).
            // Read here because `initialize` is the only place it is said, and
            // recorded rather than acted on: the editor's typed-completion gate
            // is a floor on identifier prefixes, and a `.` is not an identifier
            // — without this, `foo.` measured a prefix of zero and member
            // completion was unreachable by typing in every language.
            shared.set_triggers(
                &spec.language,
                result
                    .capabilities
                    .completion_provider
                    .as_ref()
                    .and_then(|provider| provider.trigger_characters.clone())
                    .unwrap_or_default(),
            );
            if socket.initialized(lsp_types::InitializedParams {}).is_err() {
                shared.record(
                    &spec.language,
                    &ServerEvent::Failed(Failure::Exited("gone before initialized".to_owned())),
                );
                return;
            }
            let identity = result.server_info.map_or_else(
                || ServerIdentity {
                    name: spec.command.clone(),
                    version: None,
                },
                |info| ServerIdentity {
                    name: info.name,
                    version: info.version,
                },
            );
            shared.record(&spec.language, &ServerEvent::Initialized(identity));
        }
    }

    while let Some(command) = commands.recv().await {
        match command {
            ServerCommand::Open {
                path,
                text,
                version,
            } => {
                let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
                    continue;
                };
                let opened = socket.did_open(lsp_types::DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri,
                        language_id: spec.language.0.clone(),
                        version,
                        text,
                    },
                });
                if opened.is_err() {
                    return;
                }
            }
            // `T038`. A server that asked for no synchronisation at all is told
            // nothing — `change_event` answers `None` and this sends nothing
            // rather than a notification the server declared it does not want.
            ServerCommand::Change {
                path,
                text,
                previous,
                version,
            } => {
                let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
                    continue;
                };
                let Some(change) = change_event(sync, &text, &previous) else {
                    continue;
                };
                let changed = socket.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
                    content_changes: vec![change],
                });
                if changed.is_err() {
                    return;
                }
            }
            ServerCommand::Close { path } => {
                let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
                    continue;
                };
                let closed = socket.did_close(lsp_types::DidCloseTextDocumentParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                });
                if closed.is_err() {
                    return;
                }
            }
            // **Spawned rather than awaited here, and that is the same rule one
            // layer in.** A request waits for a server, and this loop is what
            // makes the *next* command — a `didOpen`, a stop — reachable. A
            // definition that took thirty seconds would otherwise hold up the
            // stop that was meant to end it.
            ServerCommand::Ask {
                question,
                path,
                at,
                answer: then,
            } => {
                drop(tokio::spawn(answer(
                    socket.clone(),
                    Arc::clone(shared),
                    spec.ready_timeout,
                    question,
                    path,
                    at,
                    then,
                )));
            }
            // Spawned for the same reason, and it matters more here: a
            // completion request happens on a keystroke, so a slow one must
            // never be in the way of the next one.
            ServerCommand::Look {
                lookup,
                path,
                at,
                answer: then,
            } => {
                drop(tokio::spawn(look_up(
                    socket.clone(),
                    Arc::clone(shared),
                    spec.ready_timeout,
                    lookup,
                    path,
                    at,
                    then,
                )));
            }
            ServerCommand::Stop => {
                // Recorded *before* the requests, so the EOF that follows is
                // read as the stop happening rather than as a crash. This is
                // the ordering `ServerState::after`'s `Stopped` rule exists to
                // protect, and the only place that can get it right.
                shared.record(&spec.language, &ServerEvent::Stopped);
                let shutdown = tokio::time::timeout(spec.ready_timeout, socket.shutdown(()));
                drop(shutdown.await);
                drop(socket.exit(()));
                // **And then we wait rather than return.** Returning here would
                // end `serve`, drop the `Child` and SIGKILL a server that was
                // in the middle of doing what we asked — `kill_on_drop` is the
                // backstop, not the mechanism. The wait ends the moment the
                // server closes its side, because that completes the main loop
                // and `serve`'s `select!` takes the other branch; the timeout
                // is only for a server that ignores `exit`.
                tokio::time::sleep(spec.ready_timeout).await;
                return;
            }
        }
    }
}

/// One question, asked and answered.
///
/// **Exactly one call to the callback, on every path**, including a server that
/// never replies, a path that has no `file:` URL, and this task being aborted
/// mid-flight by a restart — the last of those is [`Answer`]'s `Drop` rather
/// than a line here, which is the only way to cover a path that does not run.
/// That is what lets a caller treat the callback as the answer rather than as a
/// maybe, and it is why the timeout is here rather than at the call site.
async fn answer(
    mut socket: ServerSocket,
    shared: Arc<Shared>,
    patience: Duration,
    question: Question,
    path: PathBuf,
    at: Position,
    then: Answer<Vec<FileSpan>>,
) {
    let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
        then.give(Vec::new());
        return;
    };
    let text = shared.text_of(&path).unwrap_or_default();
    let position = lsp_types::TextDocumentPositionParams {
        text_document: lsp_types::TextDocumentIdentifier { uri },
        position: position_to_lsp(&text, at),
    };

    let places = match question {
        Question::Definition => {
            let request = socket.definition(lsp_types::GotoDefinitionParams {
                text_document_position_params: position,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            });
            match tokio::time::timeout(patience, request).await {
                Ok(Ok(Some(response))) => locations_of(&response),
                _ => Vec::new(),
            }
        }
        Question::References => {
            let request = socket.references(lsp_types::ReferenceParams {
                text_document_position: position,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
                // The declaration is one of the references as far as a reader is
                // concerned; excluding it makes `gr` on a definition answer
                // "nothing", which reads as a broken feature rather than as a
                // protocol option.
                context: lsp_types::ReferenceContext {
                    include_declaration: true,
                },
            });
            match tokio::time::timeout(patience, request).await {
                Ok(Ok(Some(locations))) => locations,
                _ => Vec::new(),
            }
        }
    };

    // **The target files, read before the columns are converted** — `T036`'s
    // recorded gap, closed at `T038`. Only files the client has no text for,
    // only files this server just named, and bounded; see `read_bounded`.
    let mut targets: HashMap<PathBuf, String> = HashMap::new();
    for location in &places {
        let Ok(target) = location.uri.to_file_path() else {
            continue;
        };
        if shared.text_of(&target).is_some() || targets.contains_key(&target) {
            continue;
        }
        if let Some(text) = read_bounded(&target).await {
            targets.insert(target, text);
        }
    }
    let text_of = move |path: &Path| shared.text_of(path).or_else(|| targets.get(path).cloned());

    then.give(
        places
            .iter()
            .filter_map(|location| file_span_from_lsp(location, &text_of))
            .collect(),
    );
}

/// The largest target file this client will read to convert a column.
///
/// The same judgement as [`MAX_FRAME_BYTES`], one door over: the paths read
/// here are **named by the server**, so a server that answered
/// go-to-definition with `/dev/zero` would otherwise be handing the editor an
/// unbounded read. 8 MiB is far past any source file and far short of a
/// problem, and a file over it converts against `""` — the behaviour that was
/// there before this read existed.
const MAX_TARGET_BYTES: u64 = 8 * 1024 * 1024;

/// A file's text, off the blocking pool and bounded, or `None`.
///
/// `None` for a file that does not exist, is not readable, is not UTF-8, or is
/// over [`MAX_TARGET_BYTES`] — every one of which is *"convert against `""`"*,
/// which is what this code did for all files before it.
///
/// **`spawn_blocking` and `std::fs`, not `tokio::fs`**, and the difference is
/// only a manifest: `tokio`'s `fs` feature is not in this workspace's six
/// (root `Cargo.toml`, *"`process`, `io-util`, `rt`, `sync`, `time`,
/// `macros`"*), that manifest is not this crate's to edit, and `tokio::fs` is
/// `spawn_blocking` around `std::fs` underneath in any case. The property that
/// matters — the runtime thread is not blocked while a file is read — is the
/// same one, reached without a contract change.
async fn read_bounded(path: &Path) -> Option<String> {
    use std::io::Read as _;

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path).ok()?;
        let mut text = String::new();
        // `take` bounds the read itself rather than checking a length first: a
        // length check is a TOCTOU, and a named pipe has no length at all.
        io::BufReader::new(file.take(MAX_TARGET_BYTES + 1))
            .read_to_string(&mut text)
            .ok()?;
        (u64::try_from(text.len()).unwrap_or(u64::MAX) <= MAX_TARGET_BYTES).then_some(text)
    })
    .await
    .ok()
    .flatten()
}

/// One [`Lookup`], asked and answered (`T038`, `T039`).
///
/// **Exactly one call to the callback on every path**, the same as [`answer`]
/// and by the same mechanism: [`Answer`]'s `Drop`.
async fn look_up(
    mut socket: ServerSocket,
    shared: Arc<Shared>,
    patience: Duration,
    lookup: Lookup,
    path: PathBuf,
    at: Position,
    then: Answer<Insight>,
) {
    let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
        then.give(Insight::Nothing);
        return;
    };
    let text = shared.text_of(&path).unwrap_or_default();
    let position = lsp_types::TextDocumentPositionParams {
        text_document: lsp_types::TextDocumentIdentifier { uri },
        position: position_to_lsp(&text, at),
    };

    let found = match lookup {
        Lookup::Completion => {
            let request = socket.completion(lsp_types::CompletionParams {
                text_document_position: position,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
                context: None,
            });
            match tokio::time::timeout(patience, request).await {
                Ok(Ok(Some(response))) => Insight::Completions(completions_from_lsp(&response)),
                _ => Insight::Nothing,
            }
        }
        Lookup::SignatureHelp => {
            let request = socket.signature_help(lsp_types::SignatureHelpParams {
                text_document_position_params: position,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                context: None,
            });
            match tokio::time::timeout(patience, request).await {
                Ok(Ok(Some(help))) => signature_from_lsp(&help)
                    .map_or(Insight::Nothing, |signature| {
                        Insight::Signature(Box::new(signature))
                    }),
                _ => Insight::Nothing,
            }
        }
        Lookup::Hover => {
            let request = socket.hover(lsp_types::HoverParams {
                text_document_position_params: position,
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            });
            match tokio::time::timeout(patience, request).await {
                Ok(Ok(Some(hover))) => {
                    let prose = hover_prose(&hover.contents);
                    if prose.is_empty() {
                        Insight::Nothing
                    } else {
                        Insight::Hover(prose)
                    }
                }
                _ => Insight::Nothing,
            }
        }
    };
    then.give(found);
}

/// What we tell a server about ourselves.
///
/// `position_encoding` is the field worth reading twice: the specification's
/// default is UTF-16 and a client that says nothing gets it, but *saying* it is
/// what makes the conversion in this module a contract instead of an
/// assumption. A server that offers UTF-8 will not silently switch to it.
///
/// # `rootUri` is deprecated and is sent anyway
///
/// The specification marked it deprecated in 3.6 in favour of
/// `workspaceFolders`, and **typescript-language-server 5.3.0 refuses to
/// initialize without it**: `{"code":-32603,"message":"Request initialize
/// failed with message: Could not find a valid TypeScript installation…"}`,
/// with `typescript` installed in `node_modules` of the very folder that was
/// sent as a workspace folder. Isolated at `CP-4` against the real binary — the
/// same params with `rootUri` set initialize fine — so this is not a guess
/// about the server's reason, it is the field it reads to find `node_modules`.
/// Two of the twelve first-class declarations (`typescript`, `javascript`) had
/// no working server until this line, which is what makes sending a deprecated
/// field the smaller of the two costs.
fn initialize_params(spec: &ServerSpec, root: &Path) -> lsp_types::InitializeParams {
    let uri = lsp_types::Url::from_file_path(root).ok();
    let folders = uri.clone().map(|uri| {
        vec![lsp_types::WorkspaceFolder {
            uri,
            name: root
                .file_name()
                .map_or_else(|| "root".to_owned(), |name| name.to_string_lossy().into()),
        }]
    });
    #[expect(
        deprecated,
        reason = "typescript-language-server 5.3.0 fails `initialize` without it — see above"
    )]
    lsp_types::InitializeParams {
        root_uri: uri,
        workspace_folders: folders,
        initialization_options: spec.initialization_options.clone(),
        capabilities: lsp_types::ClientCapabilities {
            general: Some(lsp_types::GeneralClientCapabilities {
                position_encodings: Some(vec![lsp_types::PositionEncodingKind::UTF16]),
                ..lsp_types::GeneralClientCapabilities::default()
            }),
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                publish_diagnostics: Some(
                    lsp_types::PublishDiagnosticsClientCapabilities::default(),
                ),
                // `T038` / `T039`. **Announced, because a server is entitled to
                // answer nothing to a request the client never said it could
                // use** — several skip building a completion index until they
                // see this. The defaults are the plain forms: no snippets (a
                // `textEdit` is not applied — see `Completion::insert`), no
                // resolve round-trip, and markdown is not claimed, so a server
                // that can answer in plain text will.
                synchronization: Some(lsp_types::TextDocumentSyncClientCapabilities::default()),
                completion: Some(lsp_types::CompletionClientCapabilities::default()),
                signature_help: Some(lsp_types::SignatureHelpClientCapabilities::default()),
                hover: Some(lsp_types::HoverClientCapabilities::default()),
                ..lsp_types::TextDocumentClientCapabilities::default()
            }),
            window: Some(lsp_types::WindowClientCapabilities {
                work_done_progress: Some(true),
                ..lsp_types::WindowClientCapabilities::default()
            }),
            ..lsp_types::ClientCapabilities::default()
        },
        ..lsp_types::InitializeParams::default()
    }
}

/// What arrives unasked-for.
///
/// Only one server-to-client message has an `Action` to become, and it is the
/// one `events.rs`'s tests already name: `publishDiagnostics` →
/// `Action::Lsp(IngestDiagnostics)`, posted with the source `"lsp"` by whatever
/// the host wrapped in [`Post`].
///
/// **Both catch-alls are set, and the notification one is not optional.**
/// `Router`'s default breaks the main loop on any notification it does not
/// know, `$/`-prefixed ones excepted — and rust-analyzer's very first message
/// after `initialize` is often `window/logMessage`. The default would take a
/// working server down as a protocol error.
fn router(shared: &Arc<Shared>, root: &Path) -> Router<()> {
    let mut router = Router::new(());
    let for_diagnostics = Arc::clone(shared);
    let root = root.to_path_buf();
    router
        .notification::<lsp_types::notification::PublishDiagnostics>(move |(), params| {
            if let Some(action) = ingest(&for_diagnostics, &root, &params) {
                (for_diagnostics.post)(action);
            }
            ControlFlow::Continue(())
        })
        .unhandled_notification(|(), _| ControlFlow::Continue(()))
        .unhandled_request(|(), _| async move {
            Err(async_lsp::ResponseError::new(
                async_lsp::ErrorCode::METHOD_NOT_FOUND,
                "phosphor's LSP client answers no requests yet",
            ))
        });
    router
}

/// One `publishDiagnostics` as the Action it becomes, or [`None`] when it names
/// a file no path can be made of.
///
/// The path is workspace-relative when it is under the root, which is what the
/// capability's own parameter says it carries; an absolute path survives for a
/// file outside it, because dropping the diagnostic would be worse than an
/// unusual path.
fn ingest(
    shared: &Arc<Shared>,
    root: &Path,
    params: &lsp_types::PublishDiagnosticsParams,
) -> Option<Action> {
    let absolute = params.uri.to_file_path().ok()?;
    let text = shared.text_of(&absolute).unwrap_or_default();
    let path = absolute
        .strip_prefix(root)
        .map_or_else(|_| absolute.clone(), Path::to_path_buf);
    Some(Action::Lsp(LspAction::IngestDiagnostics {
        path,
        diagnostics: params
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic_from_lsp(&text, diagnostic))
            .collect(),
    }))
}

/// The three things `tests/lsp.rs` cannot reach: what we *say* in `initialize`,
/// the answer guarantee as a law about a value, and the frame scanner as a
/// function over bytes.
///
/// They are here rather than in `tests/` because each is private, and each is
/// private for a reason — a client that let a host rewrite its `initialize` or
/// raise its own frame limit would be back to the bugs these close.
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use proptest::prelude::*;

    use super::*;

    /// **The mutation this test exists for**: `UTF16` → `UTF8` in
    /// [`initialize_params`] left all 82 of this crate's tests green while
    /// changing what rust-analyzer does — it honours the declaration, so every
    /// column on a non-ASCII line would arrive as a byte offset and be
    /// converted as if it were a code-unit offset. The headline safety claim in
    /// the module header had no test at all; this is it.
    #[test]
    fn the_client_says_utf16_and_says_it_out_loud() {
        let params = initialize_params(&ServerSpec::new("rust", "rust-analyzer"), Path::new("/"));
        let general = params.capabilities.general.expect("general capabilities");
        assert_eq!(
            general.position_encodings,
            Some(vec![lsp_types::PositionEncodingKind::UTF16]),
            "one encoding is offered, and it is the one this module converts"
        );
    }

    /// The other half of the `serde_json` dependency's justification: a spec's
    /// `initializationOptions` is what reaches the server, unchanged and
    /// untouched, and [`None`] sends the field not at all rather than `null`.
    #[test]
    fn initialization_options_reach_the_server_or_are_absent() {
        let options = serde_json::json!({ "cargo": { "features": "all" } });
        let spec =
            ServerSpec::new("rust", "rust-analyzer").with_initialization_options(options.clone());
        assert_eq!(
            initialize_params(&spec, Path::new("/")).initialization_options,
            Some(options),
            "free-form JSON the server gives meaning to, passed through"
        );
        assert_eq!(
            initialize_params(&ServerSpec::new("rust", "rust-analyzer"), Path::new("/"))
                .initialization_options,
            None,
            "no options is no field, which is not the same as `null`"
        );
    }

    /// **`rootUri` is deprecated and two of the twelve declarations do not
    /// initialize without it** — see [`initialize_params`]. Asserted as an
    /// equality against the folder's own URI rather than as `is_some`, because
    /// the failure that produced this test was a *`null`* `rootUri` beside a
    /// correct `workspaceFolders`, and a server reads the two independently.
    #[test]
    fn the_root_is_sent_both_ways_because_tsserver_reads_the_deprecated_one() {
        let root = std::env::temp_dir();
        let params = initialize_params(&ServerSpec::new("typescript", "tsserver"), &root);
        let expected = lsp_types::Url::from_file_path(&root).expect("a temp dir has a file URI");
        #[expect(deprecated, reason = "the field under test")]
        let sent = params.root_uri;
        assert_eq!(sent.as_ref(), Some(&expected));
        assert_eq!(
            params
                .workspace_folders
                .as_ref()
                .and_then(|folders| folders.first())
                .map(|folder| &folder.uri),
            Some(&expected),
            "and the modern field still says the same place",
        );
    }

    /// One completion, spelled the way a server does when the four strings
    /// differ from each other.
    fn offered(label: &str, filter: &str, sort: &str) -> Completion {
        Completion {
            label: label.to_owned(),
            detail: None,
            documentation: Vec::new(),
            insert: label.to_owned(),
            filter: filter.to_owned(),
            sort: sort.to_owned(),
        }
    }

    /// `7c`'s own list, which is the specification for this: `de` typed, three
    /// rows out of a scope full of them.
    ///
    /// **What the filter drops is what makes the float usable.** Without it a
    /// real `.` covers the screen — `CP-4` measured 29 rows of 30 — so the
    /// assertion that matters is the negative one.
    #[test]
    fn a_typed_prefix_narrows_the_list_to_the_rows_it_could_still_become() {
        let items = vec![
            offered("strict_mul", "strict_mul", "a"),
            offered("default()", "default", "c"),
            offered("deserialize", "deserialize", "b"),
            offered("default_delay", "default_delay", "d"),
            offered("to_string", "to_string", "e"),
        ];
        let kept = narrow(items, "de");
        let labels: Vec<&str> = kept.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["deserialize", "default()", "default_delay"],
            "only the rows `de` could become, in `sortText` order"
        );
    }

    /// `sortText` is the server's ranking and the label is not. rust-analyzer's
    /// keys are opaque strings whose order has nothing to do with the label's,
    /// so a list sorted by label is a list the server did not choose.
    #[test]
    fn the_order_is_sort_text_and_not_the_label_or_the_wire() {
        let items = vec![
            offered("zeta", "zeta", "0001"),
            offered("alpha", "alpha", "0003"),
            offered("mid", "mid", "0002"),
        ];
        let labels: Vec<String> = narrow(items, "")
            .into_iter()
            .map(|item| item.label)
            .collect();
        assert_eq!(labels, vec!["zeta", "mid", "alpha"]);
    }

    /// The two fields a client must not confuse. A row labelled with more than
    /// its identifier still matches on `filterText`, which is the case a
    /// label-matching filter drops exactly when the user has typed enough for
    /// it to be the only answer.
    #[test]
    fn filter_text_matches_where_the_label_would_not() {
        let items = vec![offered("default() (RetryPolicy)", "default", "a")];
        assert_eq!(narrow(items.clone(), "default").len(), 1);
        assert!(
            narrow(items, "default(").is_empty(),
            "and the label's own punctuation is not what is matched",
        );
    }

    /// Case-insensitive, because a typed `De` is still asking for `default`;
    /// and an empty prefix is `<C-x>` on a fresh word, which asks for
    /// everything.
    #[test]
    fn matching_ignores_case_and_an_empty_prefix_keeps_the_whole_set() {
        let items = vec![offered("Default", "Default", "a"), offered("de", "de", "b")];
        assert_eq!(narrow(items.clone(), "dE").len(), 2);
        assert_eq!(narrow(items, "").len(), 2);
    }

    /// Absent `filterText`/`sortText` mean the label, which is the
    /// specification's rule for both — and the case every server that sends
    /// neither relies on.
    #[test]
    fn a_server_that_sends_neither_field_is_filtered_and_sorted_by_its_labels() {
        let response = lsp_types::CompletionResponse::Array(vec![
            lsp_types::CompletionItem {
                label: "beta".to_owned(),
                ..lsp_types::CompletionItem::default()
            },
            lsp_types::CompletionItem {
                label: "alpha".to_owned(),
                ..lsp_types::CompletionItem::default()
            },
            lsp_types::CompletionItem {
                label: "other".to_owned(),
                ..lsp_types::CompletionItem::default()
            },
        ]);
        let items = completions_from_lsp(&response);
        assert_eq!(items[0].filter, "beta");
        assert_eq!(items[0].sort, "beta");
        let labels: Vec<String> = narrow(items, "a")
            .into_iter()
            .map(|item| item.label)
            .collect();
        assert_eq!(labels, vec!["alpha"]);
    }

    /// Counts how many times a callback ran, which is the whole question.
    fn counted(count: &Arc<AtomicUsize>) -> Locations {
        let count = Arc::clone(count);
        Arc::new(move |_places| {
            count.fetch_add(1, Ordering::SeqCst);
        })
    }

    /// `ask`'s contract as a law about the value that carries it: **exactly
    /// one**, whichever way the value ends.
    #[test]
    fn an_answer_is_given_exactly_once_however_it_ends() {
        let given = Arc::new(AtomicUsize::new(0));
        Answer::new(counted(&given)).give(vec![FileSpan {
            path: PathBuf::from("/a.rs"),
            span: None,
        }]);
        assert_eq!(given.load(Ordering::SeqCst), 1, "answered once, by giving");

        let dropped = Arc::new(AtomicUsize::new(0));
        drop(Answer::new(counted(&dropped)));
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            1,
            "a question destroyed unanswered is still answered — this is the \
             promise the supervisor's dead senders were breaking"
        );
    }

    /// **The reproducer `F5` reported**, one 222-byte frame
    /// (`fuzz/seeds/lsp_wire/diagnostics-ceiling`, wrapped here):
    ///
    /// ```text
    /// Content-Length: 222\r\n\r\n
    /// {"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":
    ///  {"diagnostics":[{"message":"","range":
    ///    {"end":{"character":4294967295,"line":0},
    ///     "start":{"character":4294967295,"line":0}}}],
    ///   "uri":"file:///tmp/main.rs"}}
    /// ```
    ///
    /// `character` is a `u32` on the wire, so `u32::MAX` deserialises, and
    /// [`column_from_utf16`] carried the excess through with `column +
    /// (character - units)` — `1 + u32::MAX` against an all-BMP line.
    /// `attempt to add with overflow`, on the LSP task, in every build with
    /// overflow checks on. The editor kept running and simply stopped getting
    /// diagnostics from that server, which is the failure mode that takes
    /// longest to notice.
    ///
    /// Both directions are asserted because the same shape is in
    /// [`utf16_from_column`], where two astral characters are enough to lift
    /// `units` above `seen` and push the sum past the ceiling.
    #[test]
    fn a_wire_position_at_the_u32_ceiling_does_not_overflow() {
        assert_eq!(column_from_utf16("", u32::MAX), u32::MAX);
        assert_eq!(column_from_utf16("abc", u32::MAX), u32::MAX);
        assert_eq!(utf16_from_column("🦀🦀", u32::MAX), u32::MAX);

        // And the whole path a server reaches it by, which is the one that
        // matters: a range is two positions and neither may take the thread
        // down.
        let span = span_from_lsp(
            "fn main() {}\n",
            lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: u32::MAX,
                },
                end: lsp_types::Position {
                    line: u32::MAX,
                    character: u32::MAX,
                },
            },
        );
        assert_eq!(span.start.column, u32::MAX);
        assert_eq!(
            span.end.line,
            u32::MAX,
            "a line saturates rather than wraps"
        );
    }

    /// The scanner counts bodies rather than reading them, so a payload that
    /// *contains* a header cannot be mistaken for one. A diagnostic quoting an
    /// absurd `Content-Length` is a plausible message; killing the server over
    /// it would be this fix causing the bug it prevents.
    #[test]
    fn a_body_is_skipped_not_parsed() {
        let body = r#"{"m":"Content-Length: 999999999999999"}"#;
        let mut scan = FrameScan::new();
        scan.inspect(format!("Content-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
            .expect("a body is not a header");
        // And the scanner is back on headers for the next frame.
        assert_eq!(scan.left, 0);
    }

    #[test]
    fn an_absurd_frame_is_refused_before_it_is_allocated() {
        let mut scan = FrameScan::new();
        let refused = scan
            .inspect(b"Content-Length: 999999999999999\r\n\r\n")
            .expect_err("a petabyte is not a frame");
        assert_eq!(refused.kind(), io::ErrorKind::InvalidData);
        assert!(
            refused.to_string().contains("999999999999999"),
            "the refusal names the number, got {refused}"
        );

        // The boundary, both sides of it: the limit itself is a frame.
        let mut scan = FrameScan::new();
        assert!(
            scan.inspect(format!("Content-Length: {MAX_FRAME_BYTES}\r\n\r\n").as_bytes())
                .is_ok()
        );
        let mut scan = FrameScan::new();
        assert!(
            scan.inspect(format!("Content-Length: {}\r\n\r\n", MAX_FRAME_BYTES + 1).as_bytes())
                .is_err()
        );
    }

    /// A server that writes forever without a newline is the slow version of
    /// the same abort — `async-lsp` reads a header line unbounded.
    #[test]
    fn a_header_line_that_never_ends_is_refused_too() {
        let mut scan = FrameScan::new();
        let mut outcome = Ok(());
        for _ in 0..64 {
            outcome = scan.inspect(&[b'x'; 256]);
            if outcome.is_err() {
                break;
            }
        }
        assert!(
            outcome.is_err(),
            "{MAX_HEADER_BYTES} bytes of header with no newline is not a header"
        );
    }

    /// Frames as they actually arrive: in chunks a pipe chose, not the ones a
    /// message was written in.
    fn framed(bodies: Vec<String>) -> Vec<u8> {
        let mut out = Vec::new();
        for body in bodies {
            out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
            out.extend_from_slice(body.as_bytes());
        }
        out
    }

    proptest! {
        /// **The law: chunk boundaries do not exist.** The scanner sees a
        /// stream, so a header can arrive one byte at a time and a body can end
        /// mid-chunk; any well-formed sequence of frames must pass whatever way
        /// it is cut up, and must finish with no body outstanding. This is the
        /// property that makes the fast body-skip safe, and it is generated
        /// because the boundary that breaks it is the one nobody writes by
        /// hand.
        #[test]
        fn well_formed_frames_pass_however_they_are_chunked(
            bodies in prop::collection::vec("[ -~]{0,64}", 1..6),
            chunk in 1_usize..17,
        ) {
            let bytes = framed(bodies);
            let mut scan = FrameScan::new();
            for piece in bytes.chunks(chunk) {
                scan.inspect(piece).expect("a well-formed frame");
            }
            prop_assert_eq!(scan.left, 0, "every body was accounted for exactly");
        }

        /// And the refusal is not a chunking accident either: an absurd
        /// declaration is refused however it is cut, including one byte at a
        /// time, because the number is only read once the line ends.
        #[test]
        fn an_absurd_declaration_is_refused_however_it_is_chunked(chunk in 1_usize..9) {
            let bytes = b"Content-Length: 999999999999999\r\n\r\n";
            let mut scan = FrameScan::new();
            let refused = bytes
                .chunks(chunk)
                .try_fold((), |(), piece| scan.inspect(piece));
            prop_assert!(refused.is_err());
        }
    }
}
