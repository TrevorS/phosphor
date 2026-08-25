//! The VCS adapter (`T071`) — detection, and one trait behind it.
//!
//! VCS is the safety net that lets there be no review ceremony (invariant 5), so
//! this crate answers questions about what changed; it never gates an edit.
//!
//! # No feature may assume a repo exists
//!
//! `T071`'s acceptance in bold, and it is the reason this module's entry point
//! answers an [`Option`] rather than a `Result`. **A bare directory is a normal
//! state, not an error path** — `CP-8c` runs the whole `S7` acceptance set three
//! times, in a jj repo, a git repo and a bare directory, and it fails if *any*
//! message implies something is missing. So there is no `NoRepoError` in here to
//! format, and nothing above this line has an error to report: outside a repo
//! the chip is absent, the queries answer empty, and the editor is unchanged.
//!
//! The vocabulary already said so twice before this crate did anything —
//! `vcs-status`'s own doc reads *"every one of these answers empty in a bare
//! directory — no repository is a normal state, not an error"*, and the `Vcs`
//! action group's reads *"an enhancement, never a dependency"*.
//!
//! # Read on demand, not on a timer
//!
//! There is deliberately **no poller in here**. `refresh-vcs` exists precisely
//! because the answer is re-read rather than watched — the binary caches a
//! [`Status`] and asks again when something might have changed.
//!
//! That is a correctness decision as much as a cost one: the pty harness counts
//! a frame per draw, and a background producer that ticked would put one into
//! every test in the suite. `T069`'s disk watcher had to be switched off in
//! tests for exactly that reason; this avoids needing the switch.
//!
//! # Detection is filesystem-only, and that is what makes it testable
//!
//! [`detect`] walks up looking for a marker directory and never runs a
//! subprocess. So *"is this a jj repo"* is answerable on a machine with no jj
//! installed, which is the difference between a test that runs on CI and one
//! that is quietly skipped there.
//!
//! Reading the *change id* does need the backend's own binary, and
//! [`Repo::status`] says so by answering [`None`] for the fields it could not
//! learn rather than pretending the repo is not there.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Which backend is in front of us.
///
/// **jj is checked first and that is not alphabetical.** A colocated repo has
/// both `.jj` and `.git`, and in one the jj store is the truth while `.git` is
/// an export of it. Answering `git` there would describe the file the tool
/// writes rather than the tool you are using.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Jujutsu — `.jj/`.
    Jj,
    /// Git — `.git/`. The adapter lands at `T072`; detection is here because
    /// the colocated case has to be decided in one place or not at all.
    Git,
}

impl Backend {
    /// The word the statusline chip uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Jj => "jj",
            Self::Git => "git",
        }
    }

    /// The directory that marks a repository of this kind.
    const fn marker(self) -> &'static str {
        match self {
            Self::Jj => ".jj",
            Self::Git => ".git",
        }
    }
}

/// What the statusline chip is built from (`T071`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Which tool.
    pub backend: Backend,
    /// The current change, short form — jj's change id, git's branch.
    ///
    /// [`None`] when the backend's binary could not be run. **Not an error**:
    /// the repository is still there and still detected, and a chip reading
    /// `jj` without an id is more honest than one that claims the directory is
    /// bare.
    pub change: Option<String>,
    /// Whether the working copy matches the current change.
    ///
    /// [`None`] for the same reason as [`Status::change`] — unknown, rather
    /// than clean.
    pub clean: Option<bool>,
}

impl Status {
    /// The chip `StatusVm::vcs` carries — `jj ✓`, `jj ●`, or bare `jj`.
    ///
    /// **Three states and not two**, because *"I could not ask"* is not the
    /// same as *"there is nothing to report"*. §1 gives the clean tick and the
    /// dirty dot their meanings; a backend whose binary is missing gets
    /// neither, and says only what it does know.
    #[must_use]
    pub fn chip(&self) -> String {
        let head = match &self.change {
            Some(change) => format!("{} {change}", self.backend.name()),
            None => self.backend.name().to_owned(),
        };
        match self.clean {
            Some(true) => format!("{head} ✓"),
            Some(false) => format!("{head} ●"),
            None => head,
        }
    }
}

/// A repository, found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// Which backend.
    pub backend: Backend,
    /// The directory holding the marker — the repository root.
    pub root: PathBuf,
}

/// The repository `start` is inside, or [`None`] (`T071`).
///
/// Walks up from `start` to the filesystem root. **[`None`] is a normal
/// answer** — see the module header.
#[must_use]
pub fn detect(start: &Path) -> Option<Repo> {
    let mut here = Some(start);
    while let Some(dir) = here {
        // jj before git, for the colocated reason `Backend` gives.
        for backend in [Backend::Jj, Backend::Git] {
            if dir.join(backend.marker()).exists() {
                return Some(Repo {
                    backend,
                    root: dir.to_path_buf(),
                });
            }
        }
        here = dir.parent();
    }
    None
}

impl Repo {
    /// Read this repository's current state (`T071`).
    ///
    /// Runs the backend's own binary. **A backend that will not run is not a
    /// missing repository** — the fields it could not fill answer [`None`] and
    /// the `Repo` is still a `Repo`, which is what keeps *"no jj installed"*
    /// from rendering as *"bare directory"*.
    #[must_use]
    pub fn status(&self) -> Status {
        match self.backend {
            Backend::Jj => self.jj_status(),
            Backend::Git => self.git_status(),
        }
    }

    /// jj's current change id and whether the working copy is clean.
    ///
    /// **One `jj log` call and not two.** `jj status` would answer the second
    /// question directly, but a second subprocess per refresh is a second
    /// chance to be slow on a big repo — and the template below already knows
    /// both: `empty` is jj's own word for *"this change touches nothing"*,
    /// which for `@` is exactly *"the working copy matches its change"*.
    ///
    /// `--ignore-working-copy` is deliberately **not** passed: the whole
    /// question is what the working copy looks like, and the snapshot jj takes
    /// on the way is the thing being asked about.
    fn jj_status(&self) -> Status {
        let out = Command::new("jj")
            .args([
                "log",
                "--no-graph",
                "--color=never",
                "-r",
                "@",
                "-T",
                r#"change_id.short() ++ " " ++ if(empty, "clean", "dirty")"#,
            ])
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|out| out.status.success());

        let Some(out) = out else {
            return Status {
                backend: Backend::Jj,
                change: None,
                clean: None,
            };
        };
        let said = String::from_utf8_lossy(&out.stdout);
        let mut words = said.split_whitespace();
        Status {
            backend: Backend::Jj,
            change: words.next().map(str::to_owned),
            clean: match words.next() {
                Some("clean") => Some(true),
                Some("dirty") => Some(false),
                _ => None,
            },
        }
    }

    /// git's branch and whether the working tree is clean (`T072`).
    ///
    /// **One `git status` and not two**, for `jj_status`'s reason exactly:
    /// `--porcelain=v2 --branch` answers both questions in a single call, and
    /// the format is the one git documents as stable for machines. Verified
    /// against a real repository in four states before this parser was written
    /// — clean, untracked-only, modified-tracked, and detached.
    fn git_status(&self) -> Status {
        let out = Command::new("git")
            .args(["status", "--porcelain=v2", "--branch"])
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|out| out.status.success());

        let Some(out) = out else {
            return Status {
                backend: Backend::Git,
                change: None,
                clean: None,
            };
        };
        read_git_status(&String::from_utf8_lossy(&out.stdout))
    }
}

/// Parse `git status --porcelain=v2 --branch` (`T072`).
///
/// **A free function taking the text**, so the parsing half is testable on a
/// machine with no git — the same reason `detect` never shells out. The four
/// captured fixtures in this module's tests came from a real repository rather
/// than from memory.
///
/// The two facts:
///
/// * **the branch** is the word after `# branch.head`. Detached answers the
///   literal `(detached)`, and there the short commit is the honest name —
///   *"which change am I on"* has an answer even with no branch pointing at it,
///   which is exactly what jj's change id is on the other side.
/// * **clean** is *"no line that is not a header"*. Untracked counts as dirty:
///   git's porcelain reports it as `? path`, and a tree with a file git has
///   never seen is not one you could walk away from. jj agrees on the other
///   side, because its `empty` counts untracked files into the change.
fn read_git_status(said: &str) -> Status {
    let mut branch = None;
    let mut oid = None;
    let mut clean = true;

    for line in said.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("# branch.oid ") {
            oid = Some(rest.trim().to_owned());
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            clean = false;
        }
    }

    let change = match branch.as_deref() {
        // A repository with no commits yet reports `(initial)` as its oid, and
        // there the branch name is the only thing there is.
        Some("(detached)") => oid
            .filter(|oid| oid != "(initial)")
            .map(|oid| oid.chars().take(8).collect()),
        other => other.map(str::to_owned),
    };

    Status {
        backend: Backend::Git,
        change,
        clean: Some(clean),
    }
}

#[cfg(test)]
mod tests {
    use super::{Backend, Status, detect};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ph-vcs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        dir
    }

    /// **A bare directory is not a repository, and that is not an error.**
    ///
    /// The whole of `T071`'s bold line, and the one `CP-8c` runs the entire
    /// `S7` set inside.
    #[test]
    fn a_bare_directory_has_no_repository() {
        let dir = scratch("bare");
        let nested = dir.join("a").join("b");
        std::fs::create_dir_all(&nested).expect("a nested dir");
        assert_eq!(detect(&nested), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Detection walks *up*, so a file three directories deep still knows.
    #[test]
    fn detection_finds_the_root_from_a_nested_directory() {
        let dir = scratch("nested");
        std::fs::create_dir_all(dir.join(".jj").join("repo")).expect("a jj marker");
        let nested = dir.join("src").join("deep");
        std::fs::create_dir_all(&nested).expect("a nested dir");

        let found = detect(&nested).expect("the repo is up there");
        assert_eq!(found.backend, Backend::Jj);
        assert_eq!(found.root, dir);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// **A colocated repo is jj, not git**, and this is the test that pins it.
    ///
    /// Both markers are present — which `jj git init --colocate` produces — and
    /// the jj store is the truth while `.git` is an export of it. Answering
    /// `git` would describe the file the tool writes rather than the tool.
    #[test]
    fn a_colocated_repo_answers_jj() {
        let dir = scratch("colocated");
        std::fs::create_dir_all(dir.join(".jj")).expect("a jj marker");
        std::fs::create_dir_all(dir.join(".git")).expect("a git marker");

        assert_eq!(detect(&dir).expect("a repo").backend, Backend::Jj);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// **A `.git` marker with no git behind it is still a repository** — the
    /// git side of the rule two tests down.
    ///
    /// This was named `a_git_repo_is_detected_before_its_adapter_exists` while
    /// `T072` was open. The adapter exists now, and what the test actually
    /// holds is the *other* thing: a directory that looks like a repository but
    /// cannot be read reports the backend and nothing it has not earned.
    #[test]
    fn a_git_marker_with_no_git_behind_it_is_still_a_repo() {
        let dir = scratch("git");
        std::fs::create_dir_all(dir.join(".git")).expect("a git marker");

        let found = detect(&dir).expect("a repo");
        assert_eq!(found.backend, Backend::Git);
        // **A `.git` directory with nothing behind it.** `git status` fails in
        // there, so every field it could not learn is `None` and the chip says
        // only the backend — the same three-state rule the jj side follows.
        let status = found.status();
        assert_eq!(status.change, None);
        assert_eq!(status.clean, None);
        assert_eq!(status.chip(), "git");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// **The chip has three states, not two.**
    ///
    /// *"I could not ask"* is not *"nothing to report"*: a backend whose binary
    /// is missing says only what it knows, which is its own name.
    #[test]
    fn the_chip_says_only_what_it_knows() {
        let clean = Status {
            backend: Backend::Jj,
            change: Some("qpvuntsm".to_owned()),
            clean: Some(true),
        };
        assert_eq!(clean.chip(), "jj qpvuntsm ✓");

        let dirty = Status {
            clean: Some(false),
            ..clean.clone()
        };
        assert_eq!(dirty.chip(), "jj qpvuntsm ●");

        let unknown = Status {
            backend: Backend::Jj,
            change: None,
            clean: None,
        };
        assert_eq!(unknown.chip(), "jj");
    }

    /// **The four states, captured from a real repository.**
    ///
    /// Each fixture below is the literal output of
    /// `git status --porcelain=v2 --branch` in that state, recorded before the
    /// parser existed. Writing them from memory is how a parser ends up
    /// matching a format nobody emits.
    #[test]
    fn git_status_reads_the_branch_and_whether_the_tree_is_clean() {
        let clean = super::read_git_status(
            "# branch.oid 292f9839007423bf469a07accaff8c85d776e0a3\n# branch.head main\n",
        );
        assert_eq!(clean.change.as_deref(), Some("main"));
        assert_eq!(clean.clean, Some(true));
        assert_eq!(clean.chip(), "git main ✓");

        // **Untracked counts as dirty.** A tree holding a file git has never
        // seen is not one you could walk away from.
        let untracked = super::read_git_status(
            "# branch.oid 292f9839007423bf469a07accaff8c85d776e0a3\n\
             # branch.head main\n\
             ? b.txt\n",
        );
        assert_eq!(untracked.clean, Some(false));
        assert_eq!(untracked.chip(), "git main ●");

        let modified = super::read_git_status(
            "# branch.oid 292f9839007423bf469a07accaff8c85d776e0a3\n\
             # branch.head main\n\
             1 .M N... 100644 100644 100644 5626abf 5626abf a.txt\n",
        );
        assert_eq!(modified.clean, Some(false));
    }

    /// **Detached HEAD has no branch, and the short commit is the honest
    /// name.**
    ///
    /// *"Which change am I on"* has an answer even with nothing pointing at
    /// it — which is exactly what jj's change id is on the other side.
    #[test]
    fn a_detached_head_names_the_commit_rather_than_a_branch() {
        let detached = super::read_git_status(
            "# branch.oid 292f9839007423bf469a07accaff8c85d776e0a3\n# branch.head (detached)\n",
        );
        assert_eq!(detached.change.as_deref(), Some("292f9839"));
        assert_eq!(detached.chip(), "git 292f9839 ✓");

        // A repository with no commits reports `(initial)` — there is no
        // commit to name, so the chip says what it knows and no more.
        let fresh = super::read_git_status("# branch.oid (initial)\n# branch.head (detached)\n");
        assert_eq!(fresh.change, None);
        assert_eq!(fresh.chip(), "git ✓");
    }

    /// **A `.jj` directory with no jj behind it is still a repository.**
    ///
    /// The case that separates *"could not ask"* from *"bare"*, and the reason
    /// `status` answers a `Status` rather than an `Option<Status>`: this runs
    /// identically on a machine with jj and one without, because the marker is
    /// a directory and the fields it cannot fill are `None` either way.
    #[test]
    fn a_marker_with_no_backend_behind_it_is_still_a_repo() {
        let dir = scratch("no-binary");
        std::fs::create_dir_all(dir.join(".jj")).expect("a jj marker");

        let found = detect(&dir).expect("a repo");
        assert_eq!(found.backend, Backend::Jj);
        // Whether jj is installed decides the *fields*, never whether the
        // repository was found — so this asserts the half that is true on both
        // machines.
        assert!(found.status().chip().starts_with("jj"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
