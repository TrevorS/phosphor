//! Where the user's own layer lives — `T101`, and the second half of the
//! decision [`crate::journal`] already made once.
//!
//! Q1 put undo state at `$XDG_STATE_HOME/phosphor/<hash-of-canonical-root>/`
//! and [`journal::state_home`](crate::journal::state_home) is the whole of it.
//! Persistence had no equivalent: `phosphor/persist-file` was a bare name
//! joined to the *runtime root*, which in a dev checkout is the repository, so
//! `CP-4`'s manual test left a `(define-language! "lua" …)` sitting in the
//! tracked `runtime/persisted.scm`. Emacs's equivalent would be writing
//! `custom.el` into `emacs/lisp/`, and it does not.
//!
//! # One root, and a file after it — not a stack
//!
//! ```text
//!   $XDG_CONFIG_HOME/phosphor/    the boot root, when it holds an init.scm
//!                    …/init.scm   read by `phosphor_steel::boot`
//!                    …/*.scm      whatever that init.scm's load order names
//!   the persist file              read after it, by `Layer::load_persisted`
//! ```
//!
//! **There is no layering, and this module must not be read as promising
//! any.** `phosphor_steel::runtime::Runtime::root` is a first-match-wins
//! search — `$PHOSPHOR_RUNTIME`, then the directory above, then `./runtime` —
//! so a hand-written `init.scm` in the config home *replaces* the shipped tree
//! rather than loading on top of it, and an editor booted that way has no
//! keymaps and no way to quit. That is the state of the build, it is the
//! decision `runtime/README.md` records under *"Where this tree is read
//! from"*, and the half that is still open is `OPEN-QUESTIONS.md` §34. An
//! earlier draft of this header drew three stacked layers, which invited
//! exactly the file that bricks the editor.
//!
//! What this module owns is the *directory*: [`config_dir`] is the one
//! resolution, and both `Runtime::root`'s second candidate and
//! `AppHost::persist_target` go through it — so the layer that boots and the
//! file `persist-form!` appends to cannot disagree about where the config home
//! is. They did: `Runtime::root` had its own copy of the walk, without the
//! `is_absolute` filter below, so a relative `XDG_CONFIG_HOME` read the layer
//! from the working directory and wrote under `$HOME/.config`.
//!
//! # Config, not state — the one arguable call
//!
//! `persisted.scm`'s own header promises *"it is yours to edit. delete a line
//! to unbind it"*, and a binding you deliberately kept belongs in the same
//! place as the rest of your dotfiles. `$XDG_STATE_HOME` is for what you would
//! not miss if it were deleted — undo history, seen-state — and a rebind you
//! chose is not that. Emacs makes the same split: `custom.el` sits beside
//! `init.el`, not in a cache.
//!
//! The corollary is that **nothing here hashes a root.** Undo is per-project
//! and keyed on the canonical path for exactly that reason; a keymap is not,
//! and giving each checkout its own keybindings would be a different product.
//!
//! # No `etcetera`, for [`crate::journal`]'s reason
//!
//! `SPIKES.md`'s hygiene table names `etcetera` for XDG paths and this crate's
//! manifest says *"Deliberately dependency-free at the floor"*, which is
//! load-bearing: `phosphor-ui` takes `phosphor-core` and may take nothing else.
//! [`config_dir`] and `journal::state_home` are the two functions that crate
//! would replace.
//!
//! Owned by `spine`.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

/// The directory phosphor owns inside the config home.
///
/// Private, and joined in one place ([`config_dir_in`]). It was `pub` and
/// justified as *"one constant rather than three `join("phosphor")` calls"*
/// when there was only ever the one — a public item with no caller is a
/// promise nobody asked for.
const DIR: &str = "phosphor";

/// There is nowhere to put configuration.
///
/// A struct rather than an enum because there is exactly one way to get here
/// and inventing a path would be worse than saying so — the same call
/// [`crate::journal::Error::NoStateHome`] makes one module over. I/O failures
/// are the caller's: it knows which file it was opening and can name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoConfigHome;

impl fmt::Display for NoConfigHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("neither XDG_CONFIG_HOME nor HOME names an absolute directory")
    }
}

impl std::error::Error for NoConfigHome {}

/// `$XDG_CONFIG_HOME`, or `$HOME/.config`.
///
/// Private for [`DIR`]'s reason: [`config_dir`] is its only caller, and a
/// config home with no `phosphor/` on the end is not a path anything above
/// this crate has a use for.
///
/// # Errors
///
/// [`NoConfigHome`] when neither variable names an absolute directory.
fn config_home() -> Result<PathBuf, NoConfigHome> {
    home_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

/// [`config_home`] over two values rather than over the environment.
///
/// Split out **so the failure has a test**. `std::env::set_var` is `unsafe` in
/// edition 2024 and this workspace denies `unsafe_code`, so a test cannot
/// unset `HOME` in-process; `journal::state_home` has the same shape and the
/// same no-state-home arm, and that arm has never been executed by anything.
/// The environment read is then one line with nothing in it to get wrong.
fn home_from(xdg: Option<OsString>, home: Option<OsString>) -> Result<PathBuf, NoConfigHome> {
    // Relative is treated as unset, not as relative-to-cwd: the XDG spec says
    // a relative value is invalid and must be ignored, and resolving one
    // against the working directory would put a user's keymap wherever they
    // happened to launch from.
    if let Some(dir) = xdg.map(PathBuf::from).filter(|dir| dir.is_absolute()) {
        return Ok(dir);
    }
    if let Some(home) = home.map(PathBuf::from).filter(|home| home.is_absolute()) {
        return Ok(home.join(".config"));
    }
    Err(NoConfigHome)
}

/// `$XDG_CONFIG_HOME/phosphor/`.
///
/// **Resolved, not created.** A cold start that persists nothing should leave
/// no trace — *"cold start invites, never nags"* — so the directory is made by
/// whoever first writes into it, and a missing one reads as *nothing has been
/// persisted yet* rather than as a fault.
///
/// # Errors
///
/// Whatever `config_home` returns.
pub fn config_dir() -> Result<PathBuf, NoConfigHome> {
    Ok(config_dir_in(&config_home()?))
}

/// [`config_dir`] under an explicit config home.
///
/// The explicit form is what tests and the pty harness use: a child process
/// gets `XDG_CONFIG_HOME` set on its `Command`, which is safe, and an
/// in-process test joins the same `phosphor/` here rather than spelling it
/// again.
#[must_use]
pub fn config_dir_in(config_home: &Path) -> PathBuf {
    config_home.join(DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_config_home_wins_when_it_is_absolute() {
        assert_eq!(
            home_from(Some("/xdg".into()), Some("/home/teej".into())),
            Ok(PathBuf::from("/xdg"))
        );
    }

    #[test]
    fn home_is_the_fallback_and_carries_dot_config() {
        assert_eq!(
            home_from(None, Some("/home/teej".into())),
            Ok(PathBuf::from("/home/teej/.config"))
        );
    }

    /// A relative `XDG_CONFIG_HOME` is invalid per the spec, and resolving it
    /// against the working directory is the bug this whole module exists to
    /// stop: it would put a persisted keybinding wherever phosphor was
    /// launched from, which is the repository again.
    #[test]
    fn a_relative_xdg_config_home_is_ignored_rather_than_resolved() {
        assert_eq!(
            home_from(Some("config".into()), Some("/home/teej".into())),
            Ok(PathBuf::from("/home/teej/.config"))
        );
    }

    #[test]
    fn a_relative_home_is_ignored_too() {
        assert_eq!(
            home_from(Some("also/relative".into()), Some(".".into())),
            Err(NoConfigHome)
        );
    }

    /// **The arm the refusal in `crates/phosphor/src/main.rs` names.** With
    /// neither variable set there is nowhere to write, and `AppHost::persist`
    /// says so rather than guessing — which is the case `T101` inherited from
    /// the old *"no runtime tree to write to"* and kept.
    #[test]
    fn with_neither_variable_there_is_nowhere_to_put_configuration() {
        assert_eq!(home_from(None, None), Err(NoConfigHome));
        assert_eq!(
            NoConfigHome.to_string(),
            "neither XDG_CONFIG_HOME nor HOME names an absolute directory"
        );
    }

    #[test]
    fn the_directory_is_the_home_plus_one_component() {
        assert_eq!(
            config_dir_in(Path::new("/home/teej/.config")),
            PathBuf::from("/home/teej/.config/phosphor")
        );
    }
}
