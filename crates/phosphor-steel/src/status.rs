//! `T025` — the statusline, composed in Steel.
//!
//! > *"Not just segment order — the statusline is **composed as a view tree
//! > returned from Steel** (Q12): which segments, in what order, with what shed
//! > priority."* — `TASKS.md`, `T025`
//!
//! So this module is deliberately thin, and its thinness is the deliverable.
//! Rust states what is true — [`StatusVm`] — and asks; `runtime/statusline.scm`
//! decides what that means on screen. There is no segment list here, no order,
//! no ladder and no separator rule: all four are in the editor layer, where
//! Design Language §5 and §11 are policy rather than code.
//!
//! # What crosses, and which way
//!
//! ```text
//! store ──▶ StatusVm ──▶ (phosphor/status-line vm) ──▶ Node ──▶ interpreter
//!           ^ facts        ^ the editor layer          ^ the protocol
//! ```
//!
//! The ViewModel goes over as **data** ([`Runtime::call`], never `eval` with the
//! arguments printed into the source: a ViewModel carries a path, and a path
//! carries whatever a filesystem allows).
//!
//! # A layer that composes no statusline
//!
//! Answers [`ComposeError::Unbound`], which is *not* a fault: `crate::keymap`
//! makes the same call about a layer with no dispatcher — *"the editor is then
//! exactly the editor it was before this module existed"*. The caller draws no
//! statusline rather than a Rust one, because a Rust fallback here is precisely
//! the *"config file with a Rust editor hiding behind it"* `CP-2` asks about.
//!
//! Owned by `spine`.

use std::path::PathBuf;

use phosphor_core::value::{Args, Value, Wire};
use phosphor_core::view::{KeyHint, Millis, Node, SessionState};

use crate::convert::to_steel;
use crate::runtime::Runtime;
use crate::view::{self, ViewError};

/// The procedure the editor layer defines: one ViewModel in, one node back.
///
/// Namespaced, like `phosphor/press` and `phosphor/boot-files`, because Rust
/// reaches into the VM for it by name.
pub const COMPOSER: &str = "phosphor/status-line";

/// The file the shipped layer defines it in, for a message that has to say
/// where to look.
pub const FILE: &str = "statusline.scm";

/// The current file, as the statusline knows it (§5: *"file + dirty flag"*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusFile {
    /// The path, as it should read. Contracting it to a basename is a shed step
    /// the layer decides, not a fact about the file.
    pub path: PathBuf,
    /// Whether the buffer has unsaved edits.
    pub dirty: bool,
}

/// `12:1` — where the cursor is (`1a`, `8e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
}

/// Everything the statusline could say, and nothing about how it says it.
///
/// **Facts only.** There is no `Mode` enum here and no chip colour: the mode is
/// a name ([`StatusVm::mode`]) and the layer maps it to a word and an actor,
/// because *"would two reasonable users want this to differ?"* is yes for the
/// word and yes for whether a surface gets a chip of its own.
///
/// **Flagged seam, not folded in.** `phosphor-ui`'s `status_line::StatusLineVm`
/// is the same facts on the widget side, written first, and
/// `phosphor_core::vm` is where one canonical ViewModel would live. Three
/// spellings of one ViewModel will drift; collapsing them deletes a
/// `surface`-owned type, which is a request rather than an edit `spine` makes
/// here — the same call `view/props.rs` records for `SessionState` and `Mood`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusVm {
    /// What the chip names: an edit mode (`normal`, `insert`) or a surface
    /// (`repl`, `review`, `diskdiff`). Free text; the layer decides the word,
    /// the actor field and the initial it contracts to.
    pub mode: String,
    /// The surface's own name, where a file would go — `6b` draws `steel`.
    pub surface: Option<String>,
    /// The current file, or [`None`] on a surface that has no buffer (`2d`).
    pub file: Option<StatusFile>,
    /// Always present and truthful (§5); [`SessionState::None`] until `T050`.
    pub session: SessionState,
    /// When the current turn started, for the elapsed counter.
    pub since: Option<Millis>,
    /// [Q9]'s queued-ask flag — the only notification a queued ask gets.
    ///
    /// [Q9]: ../../../docs/IMPLEMENTATION-PLAN.md
    pub ask_pending: bool,
    /// Unseen regions in this file. Zero draws nothing.
    pub unseen: u32,
    /// Diagnostics in this file, by grade. Zero of a grade draws nothing.
    ///
    /// **§3's third diagnostic surface, and it was missing for a whole
    /// window.** `2b` draws `■ 1` beside `1 thread · 2 unseen` and nothing
    /// computed it: before this field neither this struct nor
    /// `runtime/statusline.scm` mentioned a diagnostic at all, so the only
    /// place a file's error count appeared was as one inline row per
    /// diagnostic — which is what made eleven of them land on one screen at
    /// `CP-4`. The count is what lets `phosphor_ui::diagnostics::RowPolicy`
    /// quiet the rows without hiding anything.
    ///
    /// Three counters because §1 gives each grade its own hue, and a merged
    /// total would be a number with no honest colour.
    pub trouble: u32,
    /// Attention-grade diagnostics in this file.
    pub attention: u32,
    /// The VCS chip, e.g. `jj ✓` (`T071`). [`None`] outside a repo.
    pub vcs: Option<String>,
    /// The language-server chip, e.g. `rust-analyzer ✓` (`7c`, `T036`).
    /// [`None`] for a buffer whose language declares no server — which is an
    /// honest first-class thing to be, so it draws nothing rather than a
    /// crossed-out anything.
    ///
    /// **A sentence, not a state.** Whether a crashed server says
    /// `rust-analyzer ✗` or the OS's own words is the host's judgement about
    /// what the user needs; this type is facts, and the fact is *what the
    /// statusline should say about the server*. Until this existed
    /// `ServerState::Crashed` was read by nothing at all and a server that
    /// could not start was completely silent — `Failure::Spawn` carries *"no
    /// such file or directory"* precisely so it can be said, and it reached
    /// nobody.
    pub server: Option<String>,
    /// Where the cursor is, or [`None`] on a surface with no cursor.
    pub cursor: Option<Cursor>,
    /// Keys this surface teaches — `6b`'s `C-c buffer · tab complete · q close`.
    pub hints: Vec<KeyHint>,
}

impl Default for StatusVm {
    /// The truthful S2 statusline: normal mode, no file, no session, no
    /// counters. Every field a lie would have to be written in explicitly.
    fn default() -> Self {
        Self {
            mode: "normal".to_owned(),
            surface: None,
            file: None,
            session: SessionState::None,
            since: None,
            ask_pending: false,
            unseen: 0,
            trouble: 0,
            attention: 0,
            vcs: None,
            server: None,
            cursor: None,
            hints: Vec::new(),
        }
    }
}

impl StatusVm {
    /// The ViewModel as the editor layer reads it: a hash, keyed by these names.
    ///
    /// Written out rather than derived because `wire_record!` is
    /// `phosphor-core`'s own macro and this type is not a payload — it crosses
    /// no door, it is one side of one call.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let file = self.file.as_ref().map_or(Value::Null, |file| {
            Value::Record(
                Args::new()
                    .with("path", file.path.to_value())
                    .with("dirty", file.dirty.to_value()),
            )
        });
        let cursor = self.cursor.map_or(Value::Null, |cursor| {
            Value::Record(
                Args::new()
                    .with("line", cursor.line.to_value())
                    .with("col", cursor.col.to_value()),
            )
        });

        Value::Record(
            Args::new()
                .with("mode", self.mode.to_value())
                .with("surface", self.surface.to_value())
                .with("file", file)
                .with("session", self.session.to_value())
                .with("since", self.since.to_value())
                .with("ask_pending", self.ask_pending.to_value())
                .with("unseen", self.unseen.to_value())
                .with("trouble", self.trouble.to_value())
                .with("attention", self.attention.to_value())
                .with("vcs", self.vcs.to_value())
                .with("server", self.server.to_value())
                .with("cursor", cursor)
                .with("hints", self.hints.to_value()),
        )
    }
}

/// Why the statusline was not composed.
///
/// Three cases and they are handled differently, which is why they are three:
/// an unbound composer is a layer that draws no statusline, a raise is a
/// redefinition someone is in the middle of getting wrong, and a value that is
/// not a tree is a composition that answered the wrong thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeError {
    /// The editor layer defines no [`COMPOSER`]. Not a fault.
    Unbound,
    /// It raised. Carries Steel's own text.
    Raised(String),
    /// It answered something that is not a view tree.
    NotATree(ViewError),
}

impl core::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unbound => write!(f, "no statusline — {COMPOSER} is not defined ({FILE})"),
            Self::Raised(why) => write!(f, "{why}"),
            Self::NotATree(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ComposeError {}

/// Asks the editor layer what the statusline says about `vm`.
///
/// One call per state change, never one per frame — `phosphor-ui`'s
/// `FrameCache` is the other half of that rule (Q12), and this function is
/// deliberately cheap enough that the rule is about the VM rather than about
/// this code.
///
/// # Errors
///
/// [`ComposeError`], which the caller keeps the last good frame through: a
/// broken redefinition must never blank the chrome.
pub fn compose(runtime: &mut Runtime, vm: &StatusVm) -> Result<Node, ComposeError> {
    if runtime.global(COMPOSER).is_err() {
        return Err(ComposeError::Unbound);
    }
    let answered = runtime
        .call(COMPOSER, vec![to_steel(&vm.to_value())])
        .map_err(|error| ComposeError::Raised(error.to_string()))?;
    view::node(&answered).map_err(ComposeError::NotATree)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::*;
    use crate::host::{Detached, Host};
    use phosphor_core::request::KeySeq;
    use phosphor_core::view::{Glyph, Tone};

    fn runtime() -> Runtime {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime");
        let host: Arc<dyn Host> = Arc::new(Detached);
        Runtime::boot(Some(&root), host)
    }

    /// The ViewModel `9c` draws: normal mode, a dirty file, an idle session, six
    /// unseen, a clean repo, a cursor.
    fn screen_9c() -> StatusVm {
        StatusVm {
            mode: "normal".to_owned(),
            file: Some(StatusFile {
                path: PathBuf::from("src/retry.rs"),
                dirty: true,
            }),
            session: SessionState::Idle,
            unseen: 6,
            vcs: Some("jj ✓".to_owned()),
            cursor: Some(Cursor { line: 12, col: 1 }),
            ..StatusVm::default()
        }
    }

    /// Every node in a tree, depth first.
    fn walk(node: &Node, out: &mut Vec<Node>) {
        out.push(node.clone());
        match node {
            Node::Line { children } => {
                for child in children {
                    walk(child.node(), out);
                }
            }
            Node::Shed {
                contracted, child, ..
            } => {
                if let Some(contracted) = contracted {
                    walk(contracted.node(), out);
                }
                walk(child.node(), out);
            }
            _ => {}
        }
    }

    fn nodes(node: &Node) -> Vec<Node> {
        let mut out = Vec::new();
        walk(node, &mut out);
        out
    }

    #[test]
    fn the_shipped_layer_composes_the_line_section_5_describes() {
        let mut runtime = runtime();
        let line = compose(&mut runtime, &screen_9c()).expect("the shipped layer composes");
        assert!(
            matches!(line, Node::Line { .. }),
            "§5: one row, never two — {line:?}"
        );

        let all = nodes(&line);
        // The chip, and the file it names.
        assert!(all.iter().any(|node| matches!(
            node,
            Node::ModeChip { label, tone } if label == "NORMAL" && *tone == Tone::Claude
        )));
        assert!(all.iter().any(|node| matches!(
            node,
            Node::FileLabel { path, .. } if path == Path::new("src/retry.rs")
        )));
        // The right-hand group, in §5's own vocabulary rather than as text.
        assert!(all.iter().any(
            |node| matches!(node, Node::Session { state, .. } if *state == SessionState::Idle)
        ));
        assert!(all.iter().any(|node| matches!(
            node,
            Node::Counter { glyph, count, .. } if *glyph == Glyph::Unseen && *count == 6
        )));
        assert!(
            all.iter().any(|node| matches!(node, Node::Divider {})),
            "the counter group joins with a bar (§5, as CP-1 amended it)"
        );
        assert!(all.iter().any(|node| matches!(node, Node::Spring {})));
    }

    /// What a rung governs, in one word: enough to say *which* segment it is.
    fn what(node: &Node) -> Option<String> {
        match node {
            Node::Label { text, .. } => Some(text.clone()),
            Node::FileLabel { path, .. } => Some(path.display().to_string()),
            Node::ModeChip { label, .. } => Some(label.clone()),
            Node::Counter { count, label, .. } => Some(match label {
                Some(word) => format!("{count} {word}"),
                None => format!("●{count}"),
            }),
            Node::Session { .. } => Some("session".to_owned()),
            _ => None,
        }
    }

    #[test]
    fn the_ladder_is_the_layers_and_ascends_in_section_11_order() {
        // §11: counters -> jj -> cursor pos -> session prose -> mode word, with
        // `8d`'s three file steps below it. The *order* is what this holds; what
        // each rung does to the pixels is `phosphor-ui`'s and is tested there.
        let mut runtime = runtime();
        let line = compose(&mut runtime, &screen_9c()).expect("composes");

        let mut ladder: Vec<(u32, String)> = Vec::new();
        for node in nodes(&line) {
            let Node::Shed {
                priority, child, ..
            } = node
            else {
                continue;
            };
            let governs = nodes(child.node())
                .iter()
                .find_map(what)
                .expect("a rung governs something drawable");
            ladder.push((priority, governs));
        }
        ladder.sort_by_key(|(priority, _)| *priority);

        let rungs: Vec<u32> = ladder.iter().map(|(priority, _)| *priority).collect();
        let mut unique = rungs.clone();
        unique.dedup();
        assert_eq!(rungs, unique, "two segments cannot share a rung");

        let order: Vec<&str> = ladder.iter().map(|(_, what)| what.as_str()).collect();
        assert_eq!(
            order,
            [
                // §11, in its own words.
                "6 unseen",
                "jj ✓",
                "12:1",
                "session",
                "NORMAL",
                // `8d`'s file steps: the path contracts to its basename, the
                // flag goes, the file goes. The last rung wraps the first —
                // which is why it reaches the contracted form's `retry.rs`.
                "src/retry.rs",
                "[+]",
                "retry.rs",
            ],
        );
    }

    #[test]
    fn the_last_standing_set_carries_no_rung_that_drops_it() {
        // §11 + Q9: `✻` / `●n` / `!` survive every step. In the protocol that is
        // "either unwrapped, or wrapped by a rung that only contracts".
        let mut runtime = runtime();
        let vm = StatusVm {
            ask_pending: true,
            session: SessionState::Idle,
            ..screen_9c()
        };
        let line = compose(&mut runtime, &vm).expect("composes");

        for node in nodes(&line) {
            let Node::Shed {
                contracted, child, ..
            } = &node
            else {
                continue;
            };
            if contracted.is_some() {
                continue;
            }
            let inner = nodes(child.node());
            assert!(
                !inner
                    .iter()
                    .any(|node| matches!(node, Node::Session { .. })),
                "a rung that drops must not take the session glyph with it"
            );
            assert!(
                !inner
                    .iter()
                    .any(|node| matches!(node, Node::Counter { .. })),
                "a rung that drops must not take the unseen counter with it"
            );
            assert!(
                !inner
                    .iter()
                    .any(|node| matches!(node, Node::ModeChip { .. })),
                "§5: the chip is always visible"
            );
        }
    }

    #[test]
    fn a_surface_gets_its_own_chip_and_its_own_keys() {
        // `6b`: `REPL` on the steel field, `steel` where a file would go, and the
        // surface's keys on the right.
        let mut runtime = runtime();
        let vm = StatusVm {
            mode: "repl".to_owned(),
            surface: Some("steel".to_owned()),
            hints: vec![KeyHint {
                key: KeySeq("C-c".to_owned()),
                verb: "buffer".to_owned(),
            }],
            ..StatusVm::default()
        };
        let line = compose(&mut runtime, &vm).expect("composes");
        let all = nodes(&line);

        assert!(all.iter().any(|node| matches!(
            node,
            Node::ModeChip { label, tone } if label == "REPL" && *tone == Tone::Steel
        )));
        assert!(
            all.iter().any(
                |node| matches!(node, Node::Label { text, .. } if text.contains("C-c buffer"))
            )
        );
    }

    #[test]
    fn nothing_true_of_the_state_is_drawn_when_it_is_absent() {
        // A truthful statusline with nothing to say says nothing: no session, no
        // counters, no vcs, no cursor — the S2 default.
        let mut runtime = runtime();
        let line = compose(&mut runtime, &StatusVm::default()).expect("composes");
        let all = nodes(&line);
        assert!(!all.iter().any(|node| matches!(node, Node::Counter { .. })));
        assert!(!all.iter().any(|node| matches!(node, Node::Divider {})));
        assert!(
            !all.iter()
                .any(|node| matches!(node, Node::FileLabel { .. }))
        );
        assert!(
            all.iter().any(|node| matches!(node, Node::ModeChip { .. })),
            "the chip is the one segment that is always there (§5)"
        );
    }

    #[test]
    fn a_redefined_composition_is_in_force_on_the_next_call() {
        // `T025`'s acceptance, at the seam: no reload, no invalidation, no copy
        // of the composition on this side to go stale.
        let mut runtime = runtime();
        let before = compose(&mut runtime, &screen_9c()).expect("composes");
        assert!(matches!(before, Node::Line { .. }));

        runtime
            .eval(
                "(define (phosphor/status-line vm) \
                   (view/label \"nothing but this\" 'meta 'plain))",
            )
            .expect("a redefinition is an ordinary form");

        let after = compose(&mut runtime, &screen_9c()).expect("composes");
        let Node::Label { text, .. } = after else {
            panic!("the whole composition was replaced, not a segment of it");
        };
        assert_eq!(text, "nothing but this");
    }

    #[test]
    fn a_layer_with_no_statusline_says_so_rather_than_drawing_one() {
        let host: Arc<dyn Host> = Arc::new(Detached);
        let mut bare = Runtime::boot(None, host);
        assert_eq!(
            compose(&mut bare, &StatusVm::default()),
            Err(ComposeError::Unbound)
        );
    }

    #[test]
    fn a_composition_that_raises_is_reported_with_steels_own_text() {
        let mut runtime = runtime();
        runtime
            .eval("(define (phosphor/status-line vm) (car '()))")
            .expect("a redefinition is an ordinary form");
        let error = compose(&mut runtime, &StatusVm::default()).expect_err("it raises");
        assert!(matches!(error, ComposeError::Raised(_)), "{error:?}");
    }

    #[test]
    fn a_composition_that_answers_the_wrong_thing_names_what_it_answered() {
        let mut runtime = runtime();
        runtime
            .eval("(define (phosphor/status-line vm) 42)")
            .expect("a redefinition is an ordinary form");
        let error = compose(&mut runtime, &StatusVm::default()).expect_err("42 is not a tree");
        assert!(matches!(error, ComposeError::NotATree(_)), "{error:?}");
    }

    #[test]
    fn the_view_model_crosses_as_the_hash_the_layer_reads() {
        let value = screen_9c().to_value();
        let Value::Record(args) = &value else {
            panic!("a ViewModel is a record");
        };
        assert_eq!(args.get("mode"), Some(&Value::Text("normal".to_owned())));
        assert_eq!(args.get("unseen"), Some(&Value::Int(6)));
        assert_eq!(args.get("ask_pending"), Some(&Value::Bool(false)));
        let Some(Value::Record(file)) = args.get("file") else {
            panic!("the file is a record of its own");
        };
        assert_eq!(
            file.get("path"),
            Some(&Value::Text("src/retry.rs".to_owned()))
        );
        assert_eq!(
            StatusVm::default().to_value(),
            StatusVm::default().to_value(),
            "encoding is a function of the ViewModel and nothing else"
        );
    }
}
