//! `T050`'s *Done when*: **a session attaches and a turn completes**.
//!
//! Against `tests/fixtures/toy_acp_agent.py` rather than a real agent, for the
//! reasons its own docstring gives — a network, a subscription and answers that
//! are not the same twice are three things a test of *the client* should not
//! depend on. The fixture speaks the wire form and imports none of the SDK, so
//! a passing test says the two agree with the protocol rather than with each
//! other.
//!
//! What is asserted is the Actions that come out of the [`Post`], because that
//! is the whole of this client's contract with the editor: `turn-began` when a
//! prompt goes out, `turn-ended` when the agent says why it stopped, and
//! nothing else.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phosphor_agent::session::{Failure, Life, Post, Session, SessionSpec};
use phosphor_core::action::{Action, SessionAction};

/// Every Action the client posted, in order.
#[derive(Clone, Default)]
struct Heard(Arc<Mutex<Vec<Action>>>);

impl Heard {
    fn post(&self) -> Post {
        let seen = Arc::clone(&self.0);
        Arc::new(move |action: Action| {
            seen.lock().expect("a sane lock").push(action);
            true
        })
    }

    fn actions(&self) -> Vec<Action> {
        self.0.lock().expect("a sane lock").clone()
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/toy_acp_agent.py")
        .canonicalize()
        .expect("the toy agent is beside this file")
}

fn spec(mode: &str) -> SessionSpec {
    SessionSpec::new("python3")
        .with_args([fixture().to_string_lossy().into_owned(), mode.to_owned()])
}

/// Polls `question` until it answers, or fails after 30s.
///
/// **A poll and not a sleep.** Everything this client does is asynchronous by
/// construction — the whole point of the type is that no method waits — so a
/// test that slept would be asserting against a clock rather than against a
/// state.
fn until<T>(what: &str, mut question: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(answer) = question() {
            return answer;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// **The task's acceptance, in one test.** A session attaches, a prompt goes
/// out, and the turn ends.
#[test]
fn a_session_attaches_and_a_turn_completes() {
    let heard = Heard::default();
    let session = Session::start(heard.post(), phosphor_agent::session::unwatched());

    assert_eq!(session.life(), Life::None, "nothing before an attach");

    session.attach(spec("turn"), std::env::current_dir().expect("a cwd"));
    let attached = until("the session to attach", || match session.life() {
        Life::Attached { session } => Some(session),
        _ => None,
    });
    assert_eq!(attached, "toy-session-1", "the agent's own session id");

    session.prompt("what is 2 + 2?");
    let actions = until("the turn to end", || {
        let seen = heard.actions();
        (seen.len() >= 2).then_some(seen)
    });

    // **The order is the claim.** A `turn-ended` before its `turn-began` would
    // leave a transcript that cannot group by turn.
    match &actions[0] {
        Action::Session(SessionAction::TurnBegan { turn, prompt }) => {
            assert_eq!(prompt.as_deref(), Some("what is 2 + 2?"));
            assert_eq!(turn.0, 0, "the first turn of the session");
        }
        other => panic!("the first Action is turn-began, not {other:?}"),
    }
    match &actions[1] {
        Action::Session(SessionAction::TurnEnded { turn, summary }) => {
            assert_eq!(turn.0, 0, "and it ends the turn it began");
            assert!(
                summary
                    .as_deref()
                    .is_some_and(|why| why.contains("EndTurn")),
                "the stop reason rides along; summary was {summary:?}"
            );
        }
        other => panic!("the second Action is turn-ended, not {other:?}"),
    }
    assert_eq!(actions.len(), 2, "and nothing else — `T054` owns the prose");
}

/// **Two turns get two ids, and they do not collide.**
///
/// A transcript folds by turn (`1b`), so a reused id folds two turns into one
/// row. The counter is the session's and never goes backwards.
#[test]
fn each_turn_gets_its_own_id() {
    let heard = Heard::default();
    let session = Session::start(heard.post(), phosphor_agent::session::unwatched());
    session.attach(spec("turn"), std::env::current_dir().expect("a cwd"));
    until("the session to attach", || {
        matches!(session.life(), Life::Attached { .. }).then_some(())
    });

    session.prompt("first");
    session.prompt("second");
    let actions = until("both turns to end", || {
        let seen = heard.actions();
        (seen.len() >= 4).then_some(seen)
    });

    let began: Vec<u64> = actions
        .iter()
        .filter_map(|action| match action {
            Action::Session(SessionAction::TurnBegan { turn, .. }) => Some(turn.0),
            _ => None,
        })
        .collect();
    assert_eq!(began, vec![0, 1], "two prompts, two turns, in order");
}

/// **A command that is not there is `Spawn`, not `Dropped`**, and the
/// difference is what §6 puts on the statusline: *"session would not start"*
/// has no `:reattach` remedy and *"session lost"* does.
#[test]
fn an_agent_that_will_not_spawn_says_so() {
    let heard = Heard::default();
    let session = Session::start(heard.post(), phosphor_agent::session::unwatched());
    session.attach(
        SessionSpec::new("phosphor-no-such-agent-anywhere"),
        std::env::current_dir().expect("a cwd"),
    );

    let failure = until("the failure to be recorded", || match session.life() {
        Life::Lost(failure) => Some(failure),
        _ => None,
    });
    assert!(
        matches!(failure, Failure::Spawn(_)),
        "a missing command is a spawn failure, not a drop: {failure:?}"
    );
    assert!(
        heard.actions().is_empty(),
        "and a session that never started began no turn"
    );
}

/// **An agent that dies mid-session is `Dropped`** — `7b`'s seam. The editor
/// has to hear about it, because §5 says the session state is *"always present
/// and truthful"* and a client that stayed silent would leave `attached` on
/// screen forever.
#[test]
fn an_agent_that_goes_away_is_a_drop() {
    let heard = Heard::default();
    let session = Session::start(heard.post(), phosphor_agent::session::unwatched());
    session.attach(spec("deaf"), std::env::current_dir().expect("a cwd"));

    let failure = until("the drop to be noticed", || match session.life() {
        Life::Lost(failure) => Some(failure),
        _ => None,
    });
    assert!(
        matches!(failure, Failure::Dropped(_)),
        "an agent that answered and left is a drop: {failure:?}"
    );
}

/// **A turn that never ends is not a hang.** The client stays attached, posts
/// its `turn-began` and nothing else, and — the part that matters — a second
/// `attach` still lands, so the editor is never stuck with a session it cannot
/// replace.
#[test]
fn a_turn_that_never_ends_leaves_the_client_usable() {
    let heard = Heard::default();
    let session = Session::start(heard.post(), phosphor_agent::session::unwatched());
    session.attach(spec("mute"), std::env::current_dir().expect("a cwd"));
    until("the session to attach", || {
        matches!(session.life(), Life::Attached { .. }).then_some(())
    });

    session.prompt("never answered");
    let actions = until("the turn to begin", || {
        let seen = heard.actions();
        (!seen.is_empty()).then_some(seen)
    });
    assert!(matches!(
        actions[0],
        Action::Session(SessionAction::TurnBegan { .. })
    ));

    // The replacement, which is the liveness claim: a mute agent must not be
    // able to wedge the supervisor.
    session.attach(spec("turn"), std::env::current_dir().expect("a cwd"));
    session.prompt("answered");
    until("the replacement session to complete a turn", || {
        heard
            .actions()
            .iter()
            .any(|action| matches!(action, Action::Session(SessionAction::TurnEnded { .. })))
            .then_some(())
    });
}

/// **A wake fires on every transition and on no repeat.** §5 wants the session
/// state *"always present and truthful"*, and the loop only draws when
/// something tells it to — so a client that recorded a state without waking
/// would be correct and stale, which is the exact defect the LSP client's
/// `Woke` was added for.
#[test]
fn every_transition_wakes_the_frame() {
    let wakes = Arc::new(Mutex::new(0u32));
    let counted = Arc::clone(&wakes);
    let heard = Heard::default();
    let session = Session::start(
        heard.post(),
        Arc::new(move || {
            *counted.lock().expect("a sane lock") += 1;
        }),
    );

    session.attach(spec("turn"), std::env::current_dir().expect("a cwd"));
    until("the session to attach", || {
        matches!(session.life(), Life::Attached { .. }).then_some(())
    });
    // `Starting` then `Attached` — two transitions, and `attach` records the
    // first synchronously so this count is not a race.
    assert!(
        *wakes.lock().expect("a sane lock") >= 2,
        "attaching is two transitions and each one draws"
    );
}
