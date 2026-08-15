//! `T022` — the REPL: `6b`, and the primary extension workflow.
//!
//! > *"the editor's internals, queryable live · the same API claude scripts
//! > against in v1.5"* — TUI Mockups `6b`
//!
//! Not a debug tool. The plan calls the REPL the workflow every later phase
//! extends the editor *through*, so this module owns one session — what was
//! typed, what it answered, what is being typed now — and composes it as the
//! view tree `6b` draws.
//!
//! # Liveness is structural, not a feature
//!
//! `T022`'s claim is that a rebind takes effect on the very next keystroke with
//! no restart. Nothing here implements that claim, and that is the point:
//! [`Repl::submit`] evaluates in the **live** VM
//! ([`Runtime::eval`](crate::runtime::Runtime::eval)), the keymap it mutates
//! lives in `runtime/keymaps.scm`, and the host asks that same VM what a key is
//! bound to on every keystroke (`crates/phosphor/src/main.rs`). There is no
//! cached copy of the keymap in Rust, so there is nothing that could go stale
//! and no reload step to forget. A design that needed one would be a `CP-2`
//! failure rather than a slower path.
//!
//! # One evaluator, two front-ends
//!
//! [`Repl::submit`] calls the same [`Runtime::evaluate`] that `--eval` reaches
//! through `door.rs`, and renders it through the same [`crate::answer`]. The two
//! doors cannot disagree about an expression because there is one path, not two
//! kept in step (`T023`).
//!
//! # It composes; it does not draw
//!
//! Same contract as [`crate::float`]: everything here is
//! `phosphor_core::view` — plain data. No colour, no geometry, no ratatui
//! (Q12). `phosphor-ui`'s interpreter (`T079`) turns [`Repl::frame`] into
//! pixels; [`Repl::lines`] is the plain-text form the S1 host bridges through
//! until it lands.
//!
//! # And its statusline is not even composed here
//!
//! `6b` draws a statusline under the session, and `T025` moved that whole
//! composition into `runtime/statusline.scm`: this module says what is *true*
//! about the surface ([`Repl::status_vm`]) and asks
//! ([`Repl::refresh`]). Redefining the composition at this very REPL changes the
//! next frame of this very surface, which is `CP-2`'s manual check performed on
//! itself.
//!
//! Owned by `spine`.

use phosphor_core::action::Outcome;
use phosphor_core::registry::steel::bindings;
use phosphor_core::request::KeySeq;
use phosphor_core::value::Value;
use phosphor_core::view::{
    Axis, Constraint, Emphasis, KeyHint, Node, Run, Slot, SpanRow, Tone, Tree,
};

use crate::answer::{Answered, answered, why};
use crate::convert::{from_steel, string_literal};
use crate::runtime::Runtime;
use crate::status::{ComposeError, StatusVm, compose};

/// `λ ` — Design Language §2's lexicon: *"λ ◆ steel prompt · steel surface"*.
pub const PROMPT: &str = "λ ";

/// `⇒ ` — what the session answered.
pub const ANSWER: &str = "⇒ ";

/// The surface's header, left half: `◆ steel` (§2's steel surface glyph).
pub const HEADER: &str = "◆ steel ";

/// The surface's header, meta half — the command that opened it.
pub const HEADER_META: &str = "· :repl";

/// The chip the statusline draws while the REPL has the frame.
///
/// `6b` draws `REPL` on the steel field, not `NORMAL` on claude-green — the
/// chip is *"a surface name, not only an edit mode"* (`phosphor_core::view`,
/// `ModeChip`), which is the same call `REVIEW` and `DISKDIFF` make.
pub const CHIP: &str = "REPL";

/// What the statusline names instead of a file while the REPL has the frame.
pub const SURFACE: &str = "steel";

/// The global the editor layer declares to say which forms outlive the session.
///
/// Policy, so it is Steel's: `runtime/repl.scm` binds it, and this module only
/// reads it. *"Would two reasonable users want this to differ?"* — yes, so it is
/// not a list in Rust.
pub const PERSISTENT_HEADS: &str = "phosphor/persistent-heads";

/// One submitted form and what it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The source as it was typed.
    pub source: String,
    /// What the editor said — `6b`'s `⇒` line, in its two halves.
    pub answered: Answered,
}

/// One REPL session.
///
/// Holds no [`Runtime`]: the VM is the editor's, and a session is a view onto
/// it. That is also what lets `--eval` and the REPL share one runtime without
/// either owning the other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repl {
    entries: Vec<Entry>,
    input: String,
    history: Vec<String>,
    /// How far back the history walk has gone, `None` when it is not walking.
    walked: Option<usize>,
    /// What was being typed when the walk started, so `↓` past the newest entry
    /// gives it back rather than emptying the line.
    draft: String,
    /// The statusline the editor layer last composed for this surface (`T025`),
    /// or `None` before the first [`Repl::refresh`].
    ///
    /// Held rather than asked for per frame because [`Repl::frame`] takes
    /// `&self` and composition needs the VM — and because Q12 says composition
    /// runs at the rate of state change, which is what
    /// [`refresh`](Repl::refresh) is called on.
    status: Option<Node>,
}

impl Repl {
    /// An empty session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What has been typed but not submitted.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Everything submitted so far, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Appends one character to the input line.
    pub fn insert(&mut self, character: char) {
        self.input.push(character);
        self.walked = None;
    }

    /// Deletes the last character, if there is one.
    pub fn backspace(&mut self) {
        self.input.pop();
        self.walked = None;
    }

    /// Clears the input line.
    pub fn clear(&mut self) {
        self.input.clear();
        self.walked = None;
    }

    /// Evaluates the input line and records what it answered.
    ///
    /// Returns [`None`] for a blank line — pressing `↵` at an empty prompt is
    /// not an empty entry.
    ///
    /// Two steps, in this order:
    ///
    /// 1. **Evaluate**, through [`Runtime::evaluate`] — the same call `--eval`
    ///    makes. The value of the last expression is the `⇒` head.
    /// 2. **Persist**, if the editor layer says this form outlives the session
    ///    ([`PERSISTENT_HEADS`]). `6b`'s `⇒ #ok · persisted to init.scm` is
    ///    these two steps: the head from the first, the note from the second.
    ///
    /// The REPL persists rather than `keymap-set!` doing it, because the REPL is
    /// the only place that has the *source text* — a scheme closure cannot be
    /// printed back as the form that made it — and because a boot-time form must
    /// not re-append itself to the file it was just read from.
    pub fn submit(&mut self, runtime: &mut Runtime) -> Option<&Entry> {
        let source = self.input.trim().to_owned();
        if source.is_empty() {
            return None;
        }

        let outcome = runtime.evaluate(&source);
        let mut answered = answered(&outcome);
        if answered.note.is_none() {
            answered.note = self.persist(runtime, &source);
        }

        // An evaluation is the state change composition runs at — and the one
        // that can *be* the redefinition, which is why the statusline is asked
        // for again here rather than invalidated on a revision. A composition
        // that broke keeps the last good line and says so where you can see it.
        if let Err(error) = self.refresh(runtime)
            && answered.note.is_none()
        {
            answered.note = Some(format!("statusline not composed — {error}"));
        }

        self.input.clear();
        self.walked = None;
        self.draft.clear();
        if self.history.last().map(String::as_str) != Some(source.as_str()) {
            self.history.push(source.clone());
        }
        self.entries.push(Entry { source, answered });
        self.entries.last()
    }

    /// Appends the form to `init.scm` when the editor layer says it should
    /// outlive the session, and answers the note that goes beside `⇒`.
    ///
    /// **The persist Action's own outcome, not the evaluation's.** A refused
    /// Action is a *value* at the Steel door (`registry::outcome_value`), so the
    /// call succeeds either way and only the receipt says what happened. A
    /// refusal must not eat the answer — the rebind still took effect — but it
    /// must not be silent either: a write that did not happen is exactly the
    /// thing a person needs told.
    fn persist(&self, runtime: &mut Runtime, source: &str) -> Option<String> {
        if !persistent(runtime, source) {
            return None;
        }
        let call = format!("(persist-form! {})", string_literal(source));
        let _ = runtime.eval(&call);
        match &runtime.take_receipts().last()?.outcome {
            Outcome::Done(receipt) => receipt.note.clone(),
            Outcome::Refused(refusal) => Some(format!("not persisted — {}", why(refusal))),
            // `T100`. Same sentence opener, because what a reader needs is the
            // same fact — the write did not happen — and the half after the
            // dash is the enum's own, not a second phrasing of it.
            Outcome::Raised(raised) => Some(format!("not persisted — {}", raised.why())),
        }
    }

    /// Walks the history. Positive `delta` goes back, per `repl-history`'s own
    /// row (*"how far back, negative goes forward"*).
    pub fn history(&mut self, delta: i64) {
        if self.history.is_empty() {
            return;
        }
        let depth = match self.walked {
            None => {
                if delta <= 0 {
                    return;
                }
                self.draft = self.input.clone();
                0i64
            }
            Some(depth) => i64::try_from(depth).unwrap_or(i64::MAX),
        } + delta;

        let last = i64::try_from(self.history.len()).unwrap_or(i64::MAX);
        if depth <= 0 {
            self.walked = None;
            self.input = core::mem::take(&mut self.draft);
            return;
        }
        let depth = depth.min(last);
        let index = usize::try_from(last - depth).unwrap_or(0);
        self.input = self.history[index].clone();
        self.walked = Some(usize::try_from(depth).unwrap_or(0));
    }

    /// Completes the identifier under the cursor against the vocabulary.
    ///
    /// `6b`'s footer promises *"tab complete"*. The candidates are
    /// [`bindings`] — the registry, walked, so the REPL completes exactly what
    /// the three doors expose and a capability added to `action.rs` is
    /// completable with no edit here.
    ///
    /// A unique match completes; several complete as far as they agree, which
    /// is the shell behaviour everyone already has.
    pub fn complete(&mut self) {
        let start = token_start(&self.input);
        let token = &self.input[start..];
        if token.is_empty() {
            return;
        }
        let mut names = bindings()
            .into_iter()
            .map(|binding| binding.name)
            .filter(|name| name.starts_with(token))
            .collect::<Vec<String>>();
        names.sort_unstable();
        names.dedup();

        let Some(shortest) = names.first() else {
            return;
        };
        let common = names.iter().skip(1).fold(shortest.clone(), |common, name| {
            common_prefix(&common, name)
        });
        if common.len() > token.len() {
            self.input.truncate(start);
            self.input.push_str(&common);
        }
        self.walked = None;
    }

    // -----------------------------------------------------------------------
    // The view
    // -----------------------------------------------------------------------

    /// What is true about this surface, for the editor layer to compose from.
    ///
    /// `6b`'s statusline in facts: the REPL has the frame, `steel` stands where
    /// a file would, and the surface teaches three keys. Everything else is the
    /// truthful S2 answer — no session, no counters, no cursor.
    #[must_use]
    pub fn status_vm(&self) -> StatusVm {
        StatusVm {
            mode: CHIP.to_ascii_lowercase(),
            surface: Some(SURFACE.to_owned()),
            hints: hints(),
            ..StatusVm::default()
        }
    }

    /// Asks the editor layer for this surface's statusline (`T025`).
    ///
    /// Called on a state change, never per frame (Q12). A layer that composes no
    /// statusline draws none — `crate::status`'s degradation, not an error. A
    /// composition that *raised* keeps the last good line, which is the same
    /// rule `phosphor-ui`'s `FrameCache::try_update` applies to a whole frame:
    /// a broken redefinition must not blank the chrome.
    ///
    /// # Errors
    ///
    /// [`ComposeError`] when the layer's composer raised or answered something
    /// that is not a view tree. The caller has something to show a person; the
    /// frame is unaffected either way.
    pub fn refresh(&mut self, runtime: &mut Runtime) -> Result<(), ComposeError> {
        match compose(runtime, &self.status_vm()) {
            Ok(node) => {
                self.status = Some(node);
                Ok(())
            }
            // A layer with no statusline is a layer with no statusline.
            Err(ComposeError::Unbound) => {
                self.status = Some(Node::Empty {});
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// The session's rows, exactly as `6b` stacks them.
    ///
    /// Entries are separated by a blank row and the live prompt follows the
    /// newest one directly — which is the drawing (TUI Mockups.dc.html:492-505),
    /// where three blanks separate four entries and none precedes the cursor.
    #[must_use]
    pub fn rows(&self) -> Vec<SpanRow> {
        let mut rows = Vec::new();
        for entry in &self.entries {
            if !rows.is_empty() {
                rows.push(SpanRow::default());
            }
            rows.push(row(vec![
                Run::new(PROMPT, Tone::Steel),
                Run::new(&entry.source, Tone::Text),
            ]));

            let mut answer = vec![Run::new(
                &format!("{ANSWER}{}", entry.answered.head),
                Tone::Prose,
            )];
            if let Some(note) = &entry.answered.note {
                answer.push(Run::new(&format!(" · {note}"), Tone::Meta));
            }
            rows.push(row(answer));
        }

        rows.push(row(vec![
            Run::new(PROMPT, Tone::Steel),
            Run::new(&self.input, Tone::Text),
            Run {
                text: " ".to_owned(),
                tone: Tone::Text,
                emphasis: Emphasis::Inverted,
            },
        ]));
        rows
    }

    /// The surface: the header row and the session under it.
    ///
    /// A full-frame surface rather than a float. Design Language §8 fixes floats
    /// at *"60–80% of width, centered"* and `6b` draws the REPL edge to edge
    /// with `REPL` in the statusline chip — the same shape `5b`'s `DISKDIFF` and
    /// `8b`'s `REVIEW` are drawn in, and not the shape `7a`'s float is.
    #[must_use]
    pub fn surface(&self) -> Node {
        Node::split(
            Axis::Rows,
            [
                Slot::new(
                    Constraint::Cells { cells: 1 },
                    Node::line([
                        Node::Label {
                            text: HEADER.to_owned(),
                            tone: Tone::Text,
                            emphasis: Emphasis::Plain,
                        },
                        Node::Label {
                            text: HEADER_META.to_owned(),
                            tone: Tone::Meta,
                            emphasis: Emphasis::Plain,
                        },
                    ]),
                ),
                Slot::new(
                    Constraint::Fill { weight: 1 },
                    Node::Spans { rows: self.rows() },
                ),
            ],
        )
    }

    /// The whole of `6b`: the surface, and the statusline under it.
    ///
    /// **The statusline is the editor layer's** (`T025`,
    /// `runtime/statusline.scm`), taken from the last
    /// [`refresh`](Repl::refresh).
    ///
    /// [`seed_status_line`] stands in for the frames before the first one, and
    /// it is a bridge with a demolition date rather than a fallback: this method
    /// takes `&self` because the S1 host calls it that way (`T090`), and
    /// composition needs the VM. `T026`'s loop composes per state change and
    /// hands the tree in, at which point the seed goes with the rest of that
    /// file's event handling. It is held to the layer's own output by
    /// `the_seed_says_what_the_layer_says`.
    #[must_use]
    pub fn frame(&self) -> Tree {
        let status = self.status.clone().unwrap_or_else(seed_status_line);
        Tree::new(Node::split(
            Axis::Rows,
            [
                Slot::new(Constraint::Fill { weight: 1 }, self.surface()),
                Slot::new(Constraint::Cells { cells: 1 }, status),
            ],
        ))
    }

    /// The session as plain text, one string per row.
    ///
    /// The bridge the S1 host draws through until `T079`'s interpreter can walk
    /// [`Repl::frame`]. It carries no tone, which is exactly why it is
    /// temporary — and why it is text rather than a second interpreter.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.rows()
            .iter()
            .map(|row| {
                row.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect()
    }
}

/// `6b`'s statusline before the editor layer has been asked for one.
///
/// **Not a second composition, and not a fallback with an opinion** — the
/// decisions (which segments, in what order, what the chip says) are
/// `runtime/statusline.scm`'s, and this exists only because [`Repl::frame`]
/// cannot reach the VM. It says exactly what the shipped layer says for
/// [`Repl::status_vm`], and a test fails if the two ever disagree.
///
/// The keys are a [`Node::Label`] rather than [`Node::KeyHints`] for the same
/// reason the layer's are: the interpreter draws a keymap surface only inside a
/// float footer until `T034` builds the widget, and a deferred node draws
/// nothing at all.
fn seed_status_line() -> Node {
    let keys = hints()
        .iter()
        .map(|hint| format!("{} {}", hint.key.0, hint.verb))
        .collect::<Vec<_>>()
        // §6: the midline dot goes inside a fact, and a hint row is one fact.
        .join(" · ");
    let gapped = |node: Node| Node::line([Node::Spacer { cells: 1 }, node]);
    Node::line([
        Node::ModeChip {
            label: CHIP.to_owned(),
            tone: Tone::Steel,
        },
        gapped(Node::Label {
            text: SURFACE.to_owned(),
            tone: Tone::Text,
            emphasis: Emphasis::Plain,
        }),
        Node::Spring {},
        Node::Label {
            text: keys,
            tone: Tone::Meta,
            emphasis: Emphasis::Plain,
        },
        // Every mockup leaves one column between the last segment and the edge.
        Node::Spacer { cells: 1 },
    ])
}

/// `C-c buffer · tab complete · q close` — `6b`'s own hints, in its own order.
///
/// **Flagged, not folded in:** the drawing promises `q close` on a surface whose
/// body is a text input, where `q` is a character you are typing. The two cannot
/// both be true until the REPL has modes (`T026`), so the host closes on `esc`
/// (Design Language §9, *"esc closes top-down"*) and this hint is left as drawn
/// rather than quietly rewritten.
#[must_use]
pub fn hints() -> Vec<KeyHint> {
    [("C-c", "buffer"), ("tab", "complete"), ("q", "close")]
        .into_iter()
        .map(|(key, verb)| KeyHint {
            key: KeySeq(key.to_owned()),
            verb: verb.to_owned(),
        })
        .collect()
}

/// Whether the editor layer declared this form's head worth persisting.
///
/// Rust finds the head; Steel decides what the heads are. A form whose head is
/// not declared — `(+ 1 2)`, a query, anything exploratory — is session-only,
/// which is what keeps `init.scm` a file of decisions rather than a transcript.
fn persistent(runtime: &Runtime, source: &str) -> bool {
    let Some(head) = head(source) else {
        return false;
    };
    let Ok(declared) = runtime.global(PERSISTENT_HEADS) else {
        return false;
    };
    let Ok(Value::List(heads)) = from_steel(&declared) else {
        return false;
    };
    heads
        .iter()
        .any(|declared| matches!(declared, Value::Text(name) if name == head))
}

/// The head of a form: `(keymap-set! …)` → `keymap-set!`.
fn head(source: &str) -> Option<&str> {
    let rest = source.trim_start().strip_prefix('(')?;
    let end = rest
        .find(|character: char| character.is_whitespace() || character == '(' || character == ')')
        .unwrap_or(rest.len());
    Some(&rest[..end]).filter(|head| !head.is_empty())
}

/// Where the identifier under the cursor starts.
fn token_start(input: &str) -> usize {
    input
        .rfind(|character: char| {
            character.is_whitespace() || character == '(' || character == ')' || character == '\''
        })
        .map_or(0, |at| {
            at + input[at..].chars().next().map_or(1, char::len_utf8)
        })
}

/// The longest prefix two names share.
fn common_prefix(left: &str, right: &str) -> String {
    left.char_indices()
        .zip(right.chars())
        .take_while(|((_, l), r)| l == r)
        .map(|((_, l), _)| l)
        .collect()
}

fn row(runs: Vec<Run>) -> SpanRow {
    SpanRow { runs, tint: None }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use super::*;
    use crate::host::{Detached, Host};

    fn runtime() -> Runtime {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime");
        let host: Arc<dyn Host> = Arc::new(Detached);
        Runtime::boot(Some(&root), host)
    }

    /// Everything a row says, flattened.
    fn text(repl: &Repl) -> String {
        repl.lines().join("\n")
    }

    #[test]
    fn a_submitted_form_is_echoed_with_its_answer() {
        let mut repl = Repl::new();
        let mut runtime = runtime();
        for character in "(+ 1 2)".chars() {
            repl.insert(character);
        }
        let entry = repl.submit(&mut runtime).expect("a form was typed");
        assert_eq!(entry.source, "(+ 1 2)");
        assert_eq!(entry.answered.head, "3");
        assert!(repl.input().is_empty(), "submitting clears the line");
    }

    #[test]
    fn a_blank_line_is_not_an_entry() {
        let mut repl = Repl::new();
        let mut runtime = runtime();
        repl.insert(' ');
        assert!(repl.submit(&mut runtime).is_none());
        assert!(repl.entries().is_empty());
    }

    #[test]
    fn a_steel_error_lands_in_the_session_rather_than_ending_it() {
        let mut repl = Repl::new();
        let mut runtime = runtime();
        for character in "(".chars() {
            repl.insert(character);
        }
        let entry = repl.submit(&mut runtime).expect("a form was typed");
        // `T100`: `#raised`, not `#refused`. Nothing declined this — the form
        // ran and blew up, and `6b`'s `⇒` line now says which of the two
        // happened instead of calling both a refusal.
        assert_eq!(entry.answered.head, crate::answer::RAISED);
        assert_eq!(
            entry.answered.note.as_deref(),
            Some("cannot parse — Unexpected EOF"),
            "the whole sentence, so steel's `Error: Parse:` envelope coming back fails here"
        );
    }

    #[test]
    fn the_history_walks_back_and_forward_and_gives_the_draft_back() {
        let mut repl = Repl::new();
        let mut runtime = runtime();
        for source in ["(+ 1 1)", "(+ 2 2)"] {
            for character in source.chars() {
                repl.insert(character);
            }
            repl.submit(&mut runtime);
        }
        repl.insert('x');

        repl.history(1);
        assert_eq!(repl.input(), "(+ 2 2)");
        repl.history(1);
        assert_eq!(repl.input(), "(+ 1 1)");
        repl.history(1);
        assert_eq!(repl.input(), "(+ 1 1)", "the walk stops at the oldest");
        repl.history(-1);
        assert_eq!(repl.input(), "(+ 2 2)");
        repl.history(-1);
        assert_eq!(
            repl.input(),
            "x",
            "walking past the newest gives the draft back"
        );
    }

    #[test]
    fn tab_completes_against_the_registry_and_never_a_table() {
        let mut repl = Repl::new();
        for character in "(unseen-reg".chars() {
            repl.insert(character);
        }
        repl.complete();
        assert_eq!(repl.input(), "(unseen-regions");

        // Several candidates complete as far as they agree.
        let mut repl = Repl::new();
        for character in "(close-".chars() {
            repl.insert(character);
        }
        repl.complete();
        assert!(
            repl.input().starts_with("(close-"),
            "a shared prefix, not a guess: {}",
            repl.input()
        );
    }

    #[test]
    fn the_session_stacks_exactly_as_6b_draws_it() {
        let mut repl = Repl::new();
        let mut runtime = runtime();
        for source in ["(+ 1 1)", "(+ 2 2)"] {
            for character in source.chars() {
                repl.insert(character);
            }
            repl.submit(&mut runtime);
        }

        let lines = repl.lines();
        assert_eq!(lines[0], "λ (+ 1 1)");
        assert_eq!(lines[1], "⇒ 2");
        assert_eq!(lines[2], "", "a blank row separates entries");
        assert_eq!(lines[3], "λ (+ 2 2)");
        assert_eq!(lines[4], "⇒ 4");
        assert_eq!(
            lines[5], "λ  ",
            "the live prompt follows the newest entry with no blank"
        );
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn the_prompt_is_the_steel_glyph_and_the_cursor_is_the_only_inverted_run() {
        let repl = Repl::new();
        let rows = repl.rows();
        let prompt = rows.last().expect("the live prompt is always drawn");
        assert_eq!(prompt.runs[0].text, PROMPT);
        assert_eq!(
            prompt.runs[0].tone,
            Tone::Steel,
            "§2: `λ ◆ steel prompt · steel surface`"
        );
        assert_eq!(
            prompt.runs.last().expect("a cursor").emphasis,
            Emphasis::Inverted
        );
    }

    /// Everything a tree says, in reading order — the chip's word, each label,
    /// each path. Enough to compare two compositions without comparing their
    /// wrappers.
    fn says(node: &Node) -> Vec<String> {
        let mut out = Vec::new();
        match node {
            Node::Line { children } => {
                for child in children {
                    out.extend(says(child.node()));
                }
            }
            // The rung is not part of what the line *says* — only of what it
            // gives up when it cannot say all of it.
            Node::Shed { child, .. } => out.extend(says(child.node())),
            Node::ModeChip { label, tone } => out.push(format!("[{label}/{tone:?}]")),
            Node::Label { text, tone, .. } => out.push(format!("{text}/{tone:?}")),
            Node::Spacer { .. } => out.push(" ".to_owned()),
            Node::Spring {} => out.push("<spring>".to_owned()),
            other => out.push(other.tag().to_owned()),
        }
        out
    }

    /// The statusline row out of a frame — `6b`'s bottom slot.
    fn status_of(tree: &Tree) -> Node {
        let Node::Split { slots, .. } = &tree.root else {
            panic!("6b stacks a surface and a statusline");
        };
        slots[1].child.node().clone()
    }

    #[test]
    fn the_frame_is_the_surface_over_the_statusline() {
        let repl = Repl::new();
        let tree = repl.frame();
        assert!(tree.float.is_none(), "the REPL is a surface, not a float");

        let Node::Split { axis, slots } = &tree.root else {
            panic!("6b stacks a surface and a statusline");
        };
        assert_eq!(*axis, Axis::Rows);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[1].constraint, Constraint::Cells { cells: 1 });

        let Node::Line { children } = slots[1].child.node() else {
            panic!("the statusline is one line that never wraps (§5)");
        };
        let Node::ModeChip { label, tone } = children[0].node() else {
            panic!("the chip is first (§5)");
        };
        assert_eq!(label, CHIP);
        assert_eq!(*tone, Tone::Steel, "6b draws the chip on the steel field");
        assert!(matches!(children[2].node(), Node::Spring {}));
    }

    #[test]
    fn the_statusline_is_the_editor_layers_from_the_first_refresh() {
        let mut runtime = runtime();
        let mut repl = Repl::new();
        repl.refresh(&mut runtime)
            .expect("the shipped layer composes");

        let tree = repl.frame();
        let Node::Split { slots, .. } = &tree.root else {
            panic!("6b stacks a surface and a statusline");
        };
        let said = says(slots[1].child.node()).join(" ");
        assert!(said.contains("[REPL/Steel]"), "{said}");
        assert!(said.contains("steel/Text"), "{said}");
        assert!(
            said.contains("C-c buffer · tab complete · q close/Meta"),
            "{said}"
        );
    }

    #[test]
    fn the_seed_says_what_the_layer_says() {
        // The seed exists because `frame` takes `&self` and composition needs
        // the VM (see `seed_status_line`). It is allowed to be a bridge; it is
        // not allowed to be a second opinion.
        let mut runtime = runtime();
        let composed = compose(&mut runtime, &Repl::new().status_vm())
            .expect("the shipped layer composes the REPL's statusline");
        assert_eq!(says(&seed_status_line()), says(&composed));
    }

    #[test]
    fn redefining_the_whole_composition_changes_the_next_frame() {
        // `T025`'s acceptance criterion, typed the way a person would type it.
        let mut runtime = runtime();
        let mut repl = Repl::new();
        repl.refresh(&mut runtime).expect("composes");
        let before = repl.frame();

        for character in
            "(define (phosphor/status-line vm) (view/label \"λ only\" 'steel 'plain))".chars()
        {
            repl.insert(character);
        }
        repl.submit(&mut runtime).expect("a form was typed");

        let after = repl.frame();
        assert_ne!(before, after, "the next frame has it");
        let Node::Split { slots, .. } = &after.root else {
            panic!("6b stacks a surface and a statusline");
        };
        let Node::Label { text, .. } = slots[1].child.node() else {
            panic!("the whole composition was replaced");
        };
        assert_eq!(text, "λ only");
    }

    #[test]
    fn a_broken_composition_keeps_the_last_good_line_and_says_so() {
        let mut runtime = runtime();
        let mut repl = Repl::new();
        repl.refresh(&mut runtime).expect("composes");
        let good = status_of(&repl.frame());

        for character in "(define (phosphor/status-line vm) (car '()))".chars() {
            repl.insert(character);
        }
        let entry = repl.submit(&mut runtime).expect("a form was typed");
        assert!(
            entry
                .answered
                .note
                .as_deref()
                .is_some_and(|note| note.contains("statusline not composed")),
            "{:?}",
            entry.answered
        );
        assert_eq!(
            status_of(&repl.frame()),
            good,
            "a broken redefinition blanks nothing"
        );
    }

    #[test]
    fn a_layer_with_no_statusline_draws_none_rather_than_a_rust_one() {
        let host: Arc<dyn Host> = Arc::new(Detached);
        let mut bare = Runtime::boot(None, host);
        let mut repl = Repl::new();
        repl.refresh(&mut bare)
            .expect("no statusline is not a fault");

        let Node::Split { slots, .. } = &repl.frame().root else {
            panic!("the surface is still there");
        };
        assert!(matches!(slots[1].child.node(), Node::Empty {}));
    }

    #[test]
    fn the_header_names_the_surface_and_the_command_that_opened_it() {
        let repl = Repl::new();
        let Node::Split { slots, .. } = repl.surface() else {
            panic!("the surface is a header over the session");
        };
        let Node::Line { children } = slots[0].child.node() else {
            panic!("the header is one line");
        };
        let Node::Label { text, tone, .. } = children[0].node() else {
            panic!("`◆ steel`");
        };
        assert_eq!(text, HEADER);
        assert_eq!(*tone, Tone::Text);
        let Node::Label { text, tone, .. } = children[1].node() else {
            panic!("`· :repl`");
        };
        assert_eq!(text, HEADER_META);
        assert_eq!(*tone, Tone::Meta, "§4: source or command · meta right");
    }

    #[test]
    fn the_head_of_a_form_is_read_off_the_text() {
        assert_eq!(
            head("(keymap-set! \"]r\" (lambda () 1))"),
            Some("keymap-set!")
        );
        assert_eq!(
            head("  (set-option! \"soft-wrap\" #t)"),
            Some("set-option!")
        );
        assert_eq!(head("(+ 1 2)"), Some("+"));
        assert_eq!(head("42"), None);
        assert_eq!(head("()"), None);
    }

    #[test]
    fn a_form_the_layer_does_not_declare_is_session_only() {
        let runtime = runtime();
        assert!(
            !persistent(&runtime, "(+ 1 2)"),
            "arithmetic does not belong in init.scm"
        );
        assert!(
            persistent(&runtime, "(keymap-set! \"]r\" (lambda () 1))"),
            "runtime/repl.scm declares keymap-set! persistent"
        );
    }

    #[test]
    fn a_session_with_nothing_in_it_still_draws_the_prompt() {
        let repl = Repl::new();
        assert_eq!(text(&repl), "λ  ");
    }
}
