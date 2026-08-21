//! What the binary owes `T050`: the session, the sink it posts through, and
//! the state the statusline reads off it.
//!
//! `crates/phosphor-agent/src/session.rs` is the client — one thread, one
//! runtime, one child process — and this is the loop's half of that seam, the
//! same way [`crate::lsp`] is the loop's half of the language servers'. The two
//! modules are deliberately the same shape: a [`Post`] that names its source, a
//! wake that carries no Action, and an attach helper that decides *which*
//! process from something the editor layer said.
//!
//! Owned by `spine`.
//!
//! # What an agent may not do to this editor
//!
//! Nothing here decides that; [`crate::deliver`] does, by reading each
//! capability's own policy before applying it. It is named here because this is
//! the module that hands an agent a way in. `turn-began` and `turn-ended` are
//! `Allow` — a producer may say a turn started — and that is the whole of what
//! this door carries today.

use std::sync::Arc;

use phosphor_agent::session::{Post, SessionSpec, Woke};
use phosphor_core::action::Action;

use crate::events;

/// Which producer a session-posted event names itself as.
pub(crate) const SOURCE: &str = "acp";

/// The option that names the agent to run.
///
/// **An option and not a capability**, which is what makes a live session one
/// REPL line away rather than a task away: `(set-option! "agent-command" "npx
/// @zed-industries/claude-code-acp")` and the next frame attaches. `T057` owns
/// the lifecycle *verbs* — attach, adopt, reattach, `5d`'s picker — and this is
/// the floor under them, not a substitute: an option cannot say *which of
/// several running sessions*, and that is the question `T057` exists to answer.
pub(crate) const COMMAND: &str = "agent-command";

/// The callback the client posts through — the queue, seen from the session's
/// thread.
///
/// The same contract [`crate::lsp::sink`] has, and the same two lines: `Post`'s
/// `bool` is *"is anyone still listening"*, which is exactly what
/// [`events::Poster::post`] answers.
pub(crate) fn sink(poster: events::Poster) -> Post {
    Arc::new(move |action: Action| {
        poster.post(events::AppEvent::Posted(events::Posted {
            source: SOURCE,
            action,
        }))
    })
}

/// The client's other door: **the session changed, draw again**.
///
/// Design Language §5 puts a higher price on this than the LSP's twin: *"Session
/// state is always present and truthful."* A session that dropped while nobody
/// pressed a key would go on saying `attached` until the next keystroke, and
/// truthful is exactly what that is not.
pub(crate) fn waking(poster: events::Poster) -> Woke {
    Arc::new(move || {
        let _listening = poster.post(events::AppEvent::Woke(SOURCE));
    })
}

/// The agent command, split the way a shell would split it.
///
/// **Whitespace, and no quoting.** `npx @zed-industries/claude-code-acp` and
/// `python3 agent.py --verbose` are the shapes this has to carry, and neither
/// needs quotes; a command that does can be built with `(set-option! …)` from a
/// list once `T057`'s vocabulary exists. Guessing at shell quoting here would
/// be a parser nobody asked for, and a wrong one is worse than none — see
/// `lint-door-callers.sh` on what a script that mis-parses an answer costs.
///
/// [`None`] when the option is absent or blank, which is *"no session"* and is
/// an honest first-class thing to be.
pub(crate) fn spec_from(command: &str) -> Option<SessionSpec> {
    let mut words = command.split_whitespace();
    let program = words.next()?;
    Some(SessionSpec::new(program).with_args(words))
}

#[cfg(test)]
mod tests {
    use super::spec_from;

    #[test]
    fn a_command_splits_into_a_program_and_its_arguments() {
        let spec = spec_from("npx @zed-industries/claude-code-acp").expect("a command");
        assert_eq!(spec.command, "npx");
        assert_eq!(
            spec.args,
            vec!["@zed-industries/claude-code-acp".to_owned()]
        );
    }

    #[test]
    fn a_bare_program_takes_no_arguments() {
        let spec = spec_from("claude-code-acp").expect("a command");
        assert_eq!(spec.command, "claude-code-acp");
        assert!(spec.args.is_empty());
    }

    /// Blank is *"no session"*, and so is whitespace — an option set to `" "`
    /// by a config that built the string from parts should not spawn a shell's
    /// idea of an empty command.
    #[test]
    fn nothing_to_run_is_no_session() {
        assert!(spec_from("").is_none());
        assert!(spec_from("   ").is_none());
    }
}
