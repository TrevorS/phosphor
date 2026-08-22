//! The ACP session client (`T050`) — one Claude Code session per editor per
//! repo.
//!
//! `agent-client-protocol` is the transport; this module is the shape the
//! editor talks to it through, and that shape is deliberately
//! `phosphor-buffer`'s LSP client's: **one thread, one runtime, one child
//! process, and every method returns without waiting for anything.** A modal
//! editor may not block on a network, and the two subsystems that talk to child
//! processes should not be two different answers to that.
//!
//! # What it produces, and what it does not
//!
//! Two Actions, and they are the two `action.rs` files under `T050`:
//! [`SessionAction::TurnBegan`] when a prompt goes out and
//! [`SessionAction::TurnEnded`] when the agent says why it stopped. Both are
//! `Allow` — a producer may emit them — and that is the whole of this task's
//! vocabulary.
//!
//! **Claude's prose is not here.** `session-prose`, `tool-call-started` and the
//! rest of the transcript's verbs are `T054`'s, and the agent is already
//! sending them: every `SessionMessage::SessionMessage` this loop reads is a
//! chunk `T054` will turn into an Action. They are read and dropped, because a
//! channel nobody drains stops the turn — and dropping them on purpose is a
//! different thing from not having thought about them. `T054` replaces the
//! `Dispatch` arm and nothing else here changes.
//!
//! # Why a turn has an id we mint
//!
//! ACP has no turn identifier. A prompt is a request and a stop reason is its
//! response, and the protocol correlates them with a JSON-RPC id that is gone
//! by the time either reaches us. [`TurnId`] is the editor's own — the
//! transcript groups by it (`T054`), the statusline's elapsed clock starts at
//! one (`T051`), and `1b`'s folds are per turn. So the client mints it when the
//! prompt goes out, which is the moment the turn starts as far as anything on
//! screen is concerned.
//!
//! # One session, and what that rules out
//!
//! `T050`'s brief is *"one Claude Code session per editor per repo"*, so
//! [`Session`] holds at most one attached agent and [`Session::attach`]
//! replaces whatever was running. That is the same rule Design Language §9
//! applies to floats — *"opening a second replaces the first"* — for the same
//! reason: two of a thing the statusline has one line for is a lie one of them
//! has to lose.
//!
//! Owned by `agent`.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::{
    ProtocolVersion, v1::CancelNotification, v1::InitializeRequest,
};
use agent_client_protocol::util::MatchDispatch;
use agent_client_protocol::{ByteStreams, Client, Dispatch, SessionMessage};
use phosphor_core::action::{Action, SessionAction};
use phosphor_core::request::{ToolCallId, TurnId};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

// ---------------------------------------------------------------------------
// The two doors out
// ---------------------------------------------------------------------------

/// Where an Action the agent produced goes.
///
/// The same type and the same contract as `phosphor_buffer::lsp::Post`: the
/// `bool` is *"is anyone still listening"*, so a producer whose editor has gone
/// stops rather than spinning.
pub type Post = Arc<dyn Fn(Action) -> bool + Send + Sync>;

/// *Something about the session changed; draw again.*
///
/// A wake carries no Action and could not — a session going from `Starting` to
/// `Attached` mutates nothing the editor owns and has nothing to refuse. It is
/// what keeps the statusline's session state from being correct and stale,
/// which is the exact defect the LSP client's own `Woke` exists for, and §5
/// puts a higher price on it here: *"Session state is always present and
/// truthful."*
pub type Woke = Arc<dyn Fn() + Send + Sync>;

/// A [`Woke`] for a caller with no screen to redraw — every test in this crate.
#[must_use]
pub fn unwatched() -> Woke {
    Arc::new(|| {})
}

// ---------------------------------------------------------------------------
// What to spawn
// ---------------------------------------------------------------------------

/// The agent command, its arguments and its environment.
///
/// **No default and no blessed table.** `phosphor-buffer`'s LSP client ships
/// `blessed()` because a language declaration names a server and the editor
/// knows the languages; there is no equivalent fact about agents, and guessing
/// at `claude-code-acp` on `$PATH` would make *"no session"* indistinguishable
/// from *"the wrong binary"*. The host names the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpec {
    /// The program to run.
    pub command: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// Environment overrides for the child, on top of the editor's own.
    pub env: BTreeMap<String, String>,
}

impl SessionSpec {
    /// A spec that runs `command` with no arguments.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    /// The same spec with `args`.
    #[must_use]
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// The same spec with one environment variable set for the child.
    #[must_use]
    pub fn with_env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }
}

// ---------------------------------------------------------------------------
// What state it is in
// ---------------------------------------------------------------------------

/// Why a session is not running.
///
/// Three, and they are three different things to say on a statusline: the
/// editor could not start the process at all, the process would not speak the
/// protocol, or it was speaking and stopped. §6's voice is *state, then the
/// remedy*, and only the third one has `:reattach` as its remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The child would not spawn — no such command, or no permission.
    Spawn(String),
    /// It spawned and the handshake did not complete.
    Handshake(String),
    /// It was attached and the connection ended.
    Dropped(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(why) => write!(formatter, "session would not start — {why}"),
            Self::Handshake(why) => write!(formatter, "session would not speak acp — {why}"),
            Self::Dropped(why) => write!(formatter, "session lost — {why}"),
        }
    }
}

/// Where the session is, as the host may observe it.
///
/// **Not `view::SessionState`, and deliberately.** That enum is what §5 draws —
/// Idle, Working, Waiting, Paused, Lost, None — and it is a statement about the
/// *turn*, which is `T051`'s to map. This one is a statement about the
/// *connection*, which is the only thing this client knows. Collapsing them
/// here would put a rendering decision inside a transport.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Life {
    /// Nothing has been attached.
    #[default]
    None,
    /// The child is spawning and the handshake has not finished.
    Starting,
    /// Attached, with the agent's session id.
    Attached {
        /// What the agent called the session — opaque, and carried so a host
        /// can say it out loud when a session is adopted (`5d`).
        session: String,
    },
    /// It stopped, and why.
    Lost(Failure),
}

// ---------------------------------------------------------------------------
// The handle
// ---------------------------------------------------------------------------

/// What the runtime thread is asked to do.
#[derive(Debug)]
enum Ask {
    Attach {
        spec: Box<SessionSpec>,
        cwd: PathBuf,
    },
    Prompt(String),
    /// `7e`'s `esc` — tell the agent to stop the turn it is running (`T062`).
    ///
    /// **Over the wire and not only in the editor.** A client-side pause that
    /// stopped *drawing* the agent's work while the agent went on doing it
    /// would be a strip saying `⏸ claude paused` about something that is not,
    /// which is §5's *"always truthful"* failing in the moment it matters most.
    Interrupt,
    Stop,
}

/// State the runtime thread writes and the host reads.
struct Shared {
    life: Mutex<Life>,
    /// The next turn id. Monotonic for the life of the process, never reused —
    /// a transcript that reused one would fold two turns into one row.
    next_turn: Mutex<u64>,
    /// The agent's tool-call names, mapped to this editor's ids — see
    /// [`Shared::name`].
    names: Mutex<BTreeMap<String, ToolCallId>>,
    post: Post,
    woke: Woke,
}

impl Shared {
    /// Records a transition and wakes the frame, **once per change**.
    ///
    /// Idempotent by construction: a repeated state is not a transition, and a
    /// wake per redundant event is a redraw per redundant event.
    fn record(&self, life: Life) {
        let mut held = self
            .life
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *held == life {
            return;
        }
        *held = life;
        drop(held);
        (self.woke)();
    }

    /// This editor's number for an agent's tool-call name.
    ///
    /// **ACP names a call with a string and the vocabulary with an id**, and
    /// neither side can adopt the other's: `request.rs`'s ids are *"an opaque
    /// non-negative integer"* by construction — every one of the fourteen is —
    /// and an agent's call name is whatever it wants. So the seam is a map, and
    /// it lives here because the client is the only thing that sees both.
    ///
    /// Stable for the life of the session, so `tool-call-progress` and
    /// `tool-call-completed` reach the row `tool-call-started` created.
    fn name(&self, called: &str) -> ToolCallId {
        let mut names = self
            .names
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(known) = names.get(called) {
            return *known;
        }
        let id = ToolCallId(names.len() as u64);
        names.insert(called.to_owned(), id);
        id
    }

    fn mint(&self) -> TurnId {
        let mut next = self
            .next_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.saturating_add(1);
        TurnId(id)
    }
}

/// The editor's session with an agent.
///
/// One thread, one runtime, one child. Every method returns without waiting.
pub struct Session {
    shared: Arc<Shared>,
    /// [`Option`] for [`Drop`]'s sake, exactly as the LSP client's is: the
    /// supervisor ends when the last sender is gone, so the sender has to be
    /// destroyed before the join and a field cannot be moved out of
    /// `&mut self`.
    asks: Option<UnboundedSender<Ask>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("life", &self.life())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Starts the runtime thread. Nothing is spawned until [`Session::attach`].
    #[must_use]
    pub fn start(post: Post, woke: Woke) -> Self {
        let shared = Arc::new(Shared {
            life: Mutex::new(Life::None),
            next_turn: Mutex::new(0),
            names: Mutex::new(BTreeMap::new()),
            post,
            woke,
        });
        let (asks, receiver) = unbounded_channel();
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("phosphor-acp".to_owned())
                .spawn(move || supervise(&shared, receiver))
                .ok()
        };
        Self {
            shared,
            asks: Some(asks),
            thread,
        }
    }

    fn send(&self, ask: Ask) {
        if let Some(asks) = &self.asks {
            drop(asks.send(ask));
        }
    }

    /// Spawns `spec`'s agent, rooted at `cwd`, replacing whatever was running.
    ///
    /// Returns immediately; the session is [`Life::Starting`] from the next
    /// observation until it is not.
    pub fn attach(&self, spec: SessionSpec, cwd: PathBuf) {
        self.shared.record(Life::Starting);
        self.send(Ask::Attach {
            spec: Box::new(spec),
            cwd,
        });
    }

    /// Asks the agent to stop the turn it is running (`7e`, `T062`).
    ///
    /// Returns immediately. **The turn does not end here** — ACP's own note is
    /// that an agent may send final updates after a cancel — and what ends it
    /// is the stop reason arriving like any other.
    pub fn interrupt(&self) {
        self.send(Ask::Interrupt);
    }

    /// Sends a prompt, beginning a turn.
    ///
    /// **Queued rather than refused when nothing is attached.** The channel is
    /// unbounded and the supervisor drains it in order, so a prompt typed
    /// while the handshake is still going out arrives after it. A refusal here
    /// would be a race the user loses by typing quickly.
    pub fn prompt(&self, body: impl Into<String>) {
        self.send(Ask::Prompt(body.into()));
    }

    /// Ends the session and leaves the runtime thread idle.
    pub fn stop(&self) {
        self.send(Ask::Stop);
    }

    /// Where the session is, right now.
    #[must_use]
    pub fn life(&self) -> Life {
        self.shared
            .life
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The sender first, so the supervisor's `recv` returns `None` and the
        // loop ends; then the join. Reversing these deadlocks.
        drop(self.asks.take());
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

// ---------------------------------------------------------------------------
// The runtime thread
// ---------------------------------------------------------------------------

/// What this client calls itself in the ACP handshake.
const CLIENT_NAME: &str = "phosphor";

/// One thread, one current-thread runtime, one child at a time.
fn supervise(shared: &Arc<Shared>, mut asks: UnboundedReceiver<Ask>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        // **The outer loop is what makes `attach` replace rather than add.**
        // `serve` owns the connection for as long as the session lives and
        // returns when it is asked to stop, when the agent goes, or when a
        // second `attach` arrives — and then this loop is back here, holding
        // the next `Attach` it was handed.
        let mut pending = asks.recv().await;
        while let Some(ask) = pending.take() {
            match ask {
                Ask::Attach { spec, cwd } => {
                    pending = serve(*spec, cwd, shared, &mut asks).await;
                }
                // A prompt or a stop with nothing attached: there is no session
                // to send it to and no state to change. Dropped rather than
                // queued forever, which is what `Life::None` already says.
                Ask::Prompt(_) | Ask::Interrupt | Ask::Stop => pending = asks.recv().await,
            }
        }
    });
}

/// Runs one session until it ends, and answers whatever ended it.
///
/// [`Some`] is an [`Ask`] the session loop took off the channel and could not
/// serve — an `Attach` for a *different* agent — which the supervisor above
/// then acts on. [`None`] is *the channel is closed*, which is the editor
/// leaving.
async fn serve(
    spec: SessionSpec,
    cwd: PathBuf,
    shared: &Arc<Shared>,
    asks: &mut UnboundedReceiver<Ask>,
) -> Option<Ask> {
    // **The child is ours, and that is the whole reason this is not
    // `AcpAgent`.** The SDK's own component spawns the process and keeps the
    // handle, which is convenient right up until the agent dies: the session's
    // update channel holds its own sender, so `read_update` waits forever on a
    // channel that will never close, and a session whose agent had exited went
    // on reporting `Attached`. Measured — `an_agent_that_dies_mid_session_is_a_drop`
    // timed out at thirty seconds against `AcpAgent`.
    //
    // Holding the `Child` makes *"the agent is gone"* an event this loop can
    // select on. It also makes a spawn failure a `Result` instead of a string
    // to classify, which is what [`classify`] used to do and no longer has to.
    //
    // `tokio::process` for the spawn and `tokio_util::compat` for the seam,
    // which is the same pair `phosphor-buffer`'s LSP client uses and for the
    // same reason: the SDK reads `futures::io`, tokio hands out `tokio::io`.
    let mut child = match tokio::process::Command::new(&spec.command)
        .args(&spec.args)
        .envs(&spec.env)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        // **The agent's stderr goes nowhere on purpose.** This process owns a
        // terminal in raw mode; a child writing to the inherited stderr would
        // paint over the frame.
        .stderr(std::process::Stdio::null())
        // The editor leaving must not leave an agent behind.
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            shared.record(Life::Lost(Failure::Spawn(error.to_string())));
            return asks.recv().await;
        }
    };
    let (Some(to_agent), Some(from_agent)) = (child.stdin.take(), child.stdout.take()) else {
        shared.record(Life::Lost(Failure::Spawn(
            "the agent has no stdin or stdout".to_owned(),
        )));
        return asks.recv().await;
    };
    let transport = ByteStreams::new(
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(to_agent),
        tokio_util::compat::TokioAsyncReadCompatExt::compat(from_agent),
    );

    // *The agent exited*, as a future this loop can wait on beside the others.
    let (gone, mut went) = tokio::sync::oneshot::channel();
    let watching = tokio::spawn(async move {
        let status = child.wait().await;
        drop(gone.send(status));
    });

    // **What the session loop wants to hand back**, filled in before it
    // returns. A closure that returned it directly would have to name it in
    // `connect_with`'s error type, and the transport's errors and ours are not
    // the same thing.
    let mut carried: Option<Ask> = None;

    let outcome = Client
        .builder()
        .name(CLIENT_NAME)
        .connect_with(transport, async |cx| {
            cx.send_request(InitializeRequest::new(ProtocolVersion::V1))
                .block_task()
                .await?;
            let mut session = cx.build_session(&cwd).block_task().start_session().await?;
            shared.record(Life::Attached {
                session: session.session_id().to_string(),
            });

            // The turn in flight, if there is one, and the prompts waiting
            // behind it.
            //
            // **One turn at a time, enforced rather than assumed.** An ACP
            // prompt is a request and its stop reason is the response, so two
            // in flight are two responses this loop cannot tell apart — and
            // the first version of it did not try: it overwrote `turn` on the
            // second `send_prompt` and emitted one `turn-ended` for two
            // `turn-began`s, leaving a transcript row that never closes.
            // Caught by `each_turn_gets_its_own_id`, which sends two prompts
            // without waiting, because that is what a person typing quickly
            // does.
            let mut turn: Option<TurnId> = None;
            let mut queued: VecDeque<String> = VecDeque::new();
            loop {
                // The next turn starts here and nowhere else, so *"a prompt
                // goes out only when nothing is running"* is one line rather
                // than a rule every arm has to remember.
                if turn.is_none()
                    && let Some(body) = queued.pop_front()
                {
                    session.send_prompt(&body)?;
                    // **After the send, not before.** A turn that failed to go
                    // out is not a turn, and a transcript row for one would be
                    // a row nothing ever ends.
                    let began = shared.mint();
                    turn = Some(began);
                    (shared.post)(Action::Session(SessionAction::TurnBegan {
                        turn: began,
                        prompt: Some(body),
                    }));
                }
                // **The borrow ends with the `select!`.** `read_update` takes
                // `&mut session` for as long as its future lives, so the arm
                // that answers an `Ask` — which needs the same `&mut` to send a
                // prompt — cannot run inside the macro. Both arms therefore
                // produce a value and the work happens after it, which is the
                // shape that compiles and the reason it is written this way.
                let step = tokio::select! {
                    ask = asks.recv() => Step::Asked(ask),
                    update = session.read_update() => Step::Heard(Box::new(update)),
                    // **The third branch, and the one the SDK cannot give.**
                    // A dead agent produces no update and closes no channel;
                    // it just stops answering. Without this the session stays
                    // `Attached` forever and §5's *"always truthful"* is a
                    // sentence about a strip that has stopped being one.
                    _ = &mut went => Step::Gone,
                };
                match step {
                    Step::Gone => {
                        return Err(
                            agent_client_protocol::Error::internal_error().data("the agent exited")
                        );
                    }
                    Step::Asked(None | Some(Ask::Stop)) => return Ok(()),
                    Step::Asked(Some(attach @ Ask::Attach { .. })) => {
                        carried = Some(attach);
                        return Ok(());
                    }
                    Step::Asked(Some(Ask::Prompt(body))) => queued.push_back(body),
                    // **`session/cancel`, and the queue emptied with it.** A
                    // prompt still waiting behind the interrupted turn is one
                    // you asked for before you changed your mind; sending it
                    // anyway would be the editor arguing with `esc`.
                    Step::Asked(Some(Ask::Interrupt)) => {
                        queued.clear();
                        if turn.is_some() {
                            cx.send_notification(CancelNotification::new(
                                session.session_id().clone(),
                            ))?;
                        }
                    }
                    Step::Heard(heard) => match *heard {
                        Err(error) => return Err(error),
                        Ok(SessionMessage::StopReason(reason)) => {
                            // `take`, so a stop reason that arrives twice ends
                            // one turn. The protocol should not send two; a
                            // client that would emit two `turn-ended` for one
                            // turn is a client whose transcript can disagree
                            // with itself.
                            if let Some(ended) = turn.take() {
                                (shared.post)(Action::Session(SessionAction::TurnEnded {
                                    turn: ended,
                                    summary: Some(format!("{reason:?}")),
                                }));
                            }
                        }
                        // `T054` — what the agent is saying, as it says it.
                        Ok(SessionMessage::SessionMessage(dispatch)) => {
                            // The turn a chunk belongs to is the one running:
                            // ACP correlates a prompt with its response and
                            // says nothing about turns, so the editor's own id
                            // is the only answer — see the module header.
                            if let Some(running) = turn {
                                transcribe(shared, running, dispatch).await;
                            }
                        }
                        // **Last, and that placement is the whole of it.** This
                        // arm exists because `SessionMessage` is
                        // `#[non_exhaustive]` — the protocol reserving room —
                        // and it was written *above* the two real arms first,
                        // where a wildcard silently ate every notification. The
                        // transcript came out with a prompt line and no prose.
                        Ok(_) => {}
                    },
                }
            }
        })
        .await;

    watching.abort();
    match outcome {
        Ok(()) => shared.record(Life::None),
        // **Every failure past the spawn is a drop.** The spawn is answered
        // above, off the `Result` the OS gave rather than off a string, so
        // there is nothing left here to classify: an agent that started and
        // then stopped talking is `7b`'s seam whatever the transport called it.
        Err(error) => shared.record(Life::Lost(Failure::Dropped(error.to_string()))),
    }
    // A carried `Attach` outranks the channel: it is already off it.
    match carried {
        Some(ask) => Some(ask),
        None => asks.recv().await,
    }
}

/// Turns one `session/update` into the Actions the transcript is built from.
///
/// **Prose and tool calls only.** The protocol carries plans, modes, available
/// commands and token usage as well; each of those is a surface this build has
/// not drawn, and inventing an Action for one would be vocabulary nobody asked
/// for. What is here is exactly what `1b` shows.
///
/// A thought chunk is prose too. §6 draws no distinction — *"his prose is
/// `#9aa39a`"* — and an agent that thinks out loud is still saying something;
/// hiding it would make the transcript disagree with what the agent believes it
/// told you.
async fn transcribe(shared: &Arc<Shared>, turn: TurnId, dispatch: Dispatch) {
    use agent_client_protocol::schema::v1::{
        ContentBlock, ContentChunk, SessionNotification, SessionUpdate, ToolCallStatus,
    };

    let shared = Arc::clone(shared);
    let matched = MatchDispatch::new(dispatch)
        .if_notification(async move |notification: SessionNotification| {
            match notification.update {
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(text),
                    ..
                })
                | SessionUpdate::AgentThoughtChunk(ContentChunk {
                    content: ContentBlock::Text(text),
                    ..
                }) => {
                    (shared.post)(Action::Session(SessionAction::SessionProse {
                        turn,
                        chunk: text.text,
                    }));
                }
                SessionUpdate::ToolCall(call) => {
                    // **`T056` — the first location, and only the first.** ACP
                    // carries a *list* of files a call touches; `1b` draws one
                    // row per call and a row has one link. The rest are not
                    // dropped so much as unaddressed: a row that could jump to
                    // three places needs somewhere to put the choice, which is
                    // a surface this screen does not have.
                    let place = call.locations.first();
                    (shared.post)(Action::Session(SessionAction::ToolCallStarted {
                        turn,
                        call: shared.name(&call.tool_call_id.0),
                        // The agent's own word for what it is doing. `1b` draws
                        // `edit`, `bash`, `read`; the protocol calls that the
                        // *kind* and puts the sentence in the title, so the
                        // kind is the verb and the title is the target.
                        verb: format!("{:?}", call.kind).to_lowercase(),
                        target: Some(call.title),
                        path: place.map(|at| at.path.display().to_string()),
                        line: place.and_then(|at| at.line),
                    }));
                }
                SessionUpdate::ToolCallUpdate(update) => {
                    let named = shared.name(&update.tool_call_id.0);
                    match update.fields.status {
                        // A finished call, with whatever it had to say. The
                        // counts are zero because ACP does not carry them —
                        // `1b` draws `+42 −0` from a *diff*, which is `T063`'s
                        // to supply, and a guess here would be a number on
                        // screen that came from nowhere.
                        Some(ToolCallStatus::Completed | ToolCallStatus::Failed) => {
                            (shared.post)(Action::Session(SessionAction::ToolCallCompleted {
                                call: named,
                                summary: update.fields.title.unwrap_or_default(),
                                added: 0,
                                removed: 0,
                            }));
                        }
                        _ => {
                            if let Some(note) = update.fields.title {
                                (shared.post)(Action::Session(SessionAction::ToolCallProgress {
                                    call: named,
                                    note,
                                }));
                            }
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })
        .await;
    // A dispatch that is not a session notification is not this client's
    // business — the connection has already answered it.
    drop(matched.otherwise_ignore());
}

/// One iteration of the session loop, so the `&mut session` borrow ends with
/// the `select!` that made it.
enum Step {
    Asked(Option<Ask>),
    /// The child process exited.
    Gone,
    /// Boxed because a `SessionMessage` carries a whole `Dispatch` and the
    /// other variant is a pointer — clippy's `large_enum_variant`, and it is
    /// right: this value is built once per protocol message.
    Heard(Box<Result<SessionMessage, agent_client_protocol::Error>>),
}
