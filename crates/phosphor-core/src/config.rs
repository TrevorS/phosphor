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
//! # A stack of three, and the order is a call site
//!
//! ```text
//!   $PHOSPHOR_RUNTIME, or ./runtime     the shipped tree: init.scm, then the
//!                                       whole load order it declares
//!   $XDG_CONFIG_HOME/phosphor/init.scm  yours. hand-written. runs on top
//!                    …/persisted.scm    machine-written by `persist!`. last
//! ```
//!
//! **Each layer loads after the one above it, and none replaces another.**
//! Emacs's model — shipped lisp, then your `init.el` — which is the argument
//! `T101` was decided on and the one Teej ruled on again on 2026-08-14
//! (`OPEN-QUESTIONS.md` §34). A user's file may therefore *remove* as well as
//! add: `keymap-remove!` is already in the vocabulary and already in the
//! persistable set, so both directions cost no new verb.
//!
//! Until that ruling the config home was `Runtime::root`'s second *candidate*,
//! so an `init.scm` here **became** the runtime tree and the shipped fifteen
//! files never loaded — measured on the built binary: an empty statusline, `:`
//! drawing `unknown key :`, `ZQ` doing nothing, and no boot float, because the
//! user's one form ran cleanly. `Runtime::root` names two candidates now and
//! the config home is not one of them; `crates/phosphor/src/main.rs`'s `vm`
//! is where the three above are stacked, in that order, by three calls.
//!
//! **`$PHOSPHOR_RUNTIME` still replaces the shipped tree**, and that is not the
//! same seam: it points the binary at a scratch layer, which is what every pty
//! test and every tape does. A config home is not a runtime root, and reading
//! one directory as both is the whole of what §34 measured.
//!
//! What this module owns is the *directory*: [`config_dir`] is the one
//! resolution, **called once per process**, and `AppHost::user_layer`,
//! `AppHost::persist_target` and `AppHost::config_home` are three joins and a
//! read onto the one path it answers — so the file you hand-write and the file
//! `persist-form!` appends to cannot disagree about where the config home is.
//! (That sentence was narrowly false for one revision: `main.rs`'s `run` called
//! `config_dir()` a second time to hand the boot float a directory the host
//! already held. No drift was possible — one function, one process — but a
//! claim that holds only by coincidence is one a later edit gets to break in
//! silence, so `AppHost::config_home` exists and `run` reads it.)
//! They did, when `Runtime::root` kept its own copy of the walk without the
//! `is_absolute` filter below: a relative `XDG_CONFIG_HOME` read the layer from
//! the working directory and wrote under `$HOME/.config`.
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

/// `~/.config/phosphor/init.scm` — a path as a **float** should say it.
///
/// The boot float names the file a fault came from, and once a user's own
/// `init.scm` layers over the shipped one (§34) there are two files with that
/// name: naming both `init.scm` puts a reader in front of a fault with no way
/// to tell whose file broke, which is the legibility §34 is about. The whole
/// path answers that, and the leading `$HOME` is what stops it being an answer
/// — `AppHost::persist`'s note already refuses to *"put somebody's `$HOME` on a
/// screenshot"*, and a float is likelier to be screenshotted than a receipt.
///
/// Purely cosmetic: nothing opens what this returns. A path that is not under
/// `$HOME` comes back unchanged, so the abbreviation can never be the reason a
/// reader cannot find the file.
#[must_use]
pub fn abbreviated(path: &Path) -> PathBuf {
    abbreviate(path, std::env::var_os("HOME"))
}

/// [`abbreviated`] over a value rather than over the environment, for
/// [`home_from`]'s reason: the environment read is then one line with nothing
/// in it to get wrong, and every branch has a test.
fn abbreviate(path: &Path, home: Option<OsString>) -> PathBuf {
    // A relative `$HOME` is ignored here for the same reason `home_from`
    // ignores one: it would match a prefix of a relative path by accident.
    let Some(home) = home.map(PathBuf::from).filter(|home| home.is_absolute()) else {
        return path.to_path_buf();
    };
    path.strip_prefix(&home)
        .map_or_else(|_| path.to_path_buf(), |rest| Path::new("~").join(rest))
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

    /// The case the boot float draws: two `init.scm`s, and the reader has to
    /// be able to tell which one faulted.
    #[test]
    fn a_path_under_home_is_abbreviated_for_a_reader() {
        assert_eq!(
            abbreviate(
                Path::new("/home/teej/.config/phosphor/init.scm"),
                Some("/home/teej".into())
            ),
            PathBuf::from("~/.config/phosphor/init.scm")
        );
    }

    /// The abbreviation may never be the reason a path stops being findable, so
    /// everything it does not recognise comes back byte for byte.
    #[test]
    fn a_path_outside_home_is_left_exactly_as_it_is() {
        let elsewhere = Path::new("/etc/phosphor/init.scm");
        assert_eq!(
            abbreviate(elsewhere, Some("/home/teej".into())),
            elsewhere.to_path_buf()
        );
        assert_eq!(abbreviate(elsewhere, None), elsewhere.to_path_buf());
        // A prefix that matches by *characters* and not by path components:
        // `/home/teejan` is not under `/home/teej`, and `strip_prefix` is what
        // makes that true rather than a `starts_with` on the string.
        let sibling = Path::new("/home/teejan/notes.scm");
        assert_eq!(
            abbreviate(sibling, Some("/home/teej".into())),
            sibling.to_path_buf()
        );
    }

    /// A relative `$HOME` is ignored on this side too — the same call
    /// `home_from` makes, for the same reason.
    #[test]
    fn a_relative_home_abbreviates_nothing() {
        let path = Path::new("/home/teej/.config/phosphor/init.scm");
        assert_eq!(abbreviate(path, Some(".".into())), path.to_path_buf());
    }

    #[test]
    fn the_directory_is_the_home_plus_one_component() {
        assert_eq!(
            config_dir_in(Path::new("/home/teej/.config")),
            PathBuf::from("/home/teej/.config/phosphor")
        );
    }
}
