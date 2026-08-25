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

/// One entry in the timeline (`T073`, screen `3b`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The short change id — `3b`'s `a4f2`.
    pub id: String,
    /// Whether this is the working copy — `3b`'s `@` against `○`.
    pub working_copy: bool,
    /// Who authored it, as the backend records it.
    ///
    /// **The author's email, not an actor.** `3b` draws `· you` and
    /// `· claude`, and this build cannot honestly produce the second: nothing
    /// creates a change per agent turn yet, so every change in a real
    /// repository is authored by whoever configured jj. Reporting the *recorded*
    /// author is the truthful half — an actor column invented from a guess
    /// would be `3b`'s one claim that no data supports.
    pub author: String,
    /// The first line of the description — `3b`'s `wire ws reconnect`.
    pub description: String,
    /// Lines added and removed — `3b`'s `+11 −18`.
    pub added: u32,
    pub removed: u32,
}

/// One line of a change's diff (`T073`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Unchanged, drawn on both sides.
    Context,
    /// Added by this change.
    Added,
    /// Removed by this change.
    Removed,
}

/// One file's worth of a change's diff (`T073`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeFile {
    /// The path, as the diff names it.
    pub path: String,
    /// The lines, in order.
    pub lines: Vec<(Mark, String)>,
}

/// One entry in `jj op log` — `3b`'s `o full op log` (`T073`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// The short operation id.
    pub id: String,
    /// What jj says it did.
    pub description: String,
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

    /// The timeline, newest first (`T073`).
    ///
    /// **jj only, and an empty answer everywhere else** — the task is *"jj
    /// timeline"* and `3b`'s own subtitle is *"enhancement view, only when jj
    /// is present"*. A git repository is not broken for lacking one; it simply
    /// has nothing to show here, which is `CP-8c`'s rule about absence.
    #[must_use]
    pub fn timeline(&self, limit: Option<u32>) -> Vec<Change> {
        if self.backend != Backend::Jj {
            return Vec::new();
        }
        let mut args = vec![
            "log".to_owned(),
            "--no-graph".to_owned(),
            "--color=never".to_owned(),
            "-r".to_owned(),
            // **`~root()` because jj's root commit is not a change anybody
            // made.** It has no author and no description, so it would draw as
            // a blank row at the bottom of every timeline.
            "all() & ~root()".to_owned(),
            "-T".to_owned(),
            JJ_TIMELINE.to_owned(),
        ];
        if let Some(limit) = limit {
            args.push("-n".to_owned());
            args.push(limit.to_string());
        }
        let out = Command::new("jj")
            .args(&args)
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|out| out.status.success());
        out.map_or_else(Vec::new, |out| {
            read_jj_timeline(&String::from_utf8_lossy(&out.stdout))
        })
    }

    /// One change's diff (`T073`) — `3b`'s `d diff`.
    #[must_use]
    pub fn change_diff(&self, change: &str) -> Vec<ChangeFile> {
        self.jj_text(&["diff", "-r", change, "--git", "--color=never"])
            .map_or_else(Vec::new, |text| read_jj_diff(&text))
    }

    /// The operation log (`T073`) — `3b`'s `o full op log`.
    ///
    /// **This is the *"undo is time travel"* half.** A change is what you
    /// wrote; an operation is what the tool did, including the undos — so the
    /// op log is the only view in which reverting is itself an event you can
    /// see rather than a hole where work used to be.
    #[must_use]
    pub fn operations(&self, limit: Option<u32>) -> Vec<Operation> {
        let mut args = vec![
            "op".to_owned(),
            "log".to_owned(),
            "--no-graph".to_owned(),
            "--color=never".to_owned(),
            "-T".to_owned(),
            JJ_OPERATIONS.to_owned(),
        ];
        if let Some(limit) = limit {
            args.push("-n".to_owned());
            args.push(limit.to_string());
        }
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        self.jj_text(&borrowed)
            .map_or_else(Vec::new, |text| read_jj_operations(&text))
    }

    /// Move the working copy to this change — `3b`'s `↵ edit here` (`T073`).
    ///
    /// **The `Err` carries jj's own words.** A refusal this editor invented
    /// would be a second opinion about a repository it does not own; jj knows
    /// why it would not move and says so better than a paraphrase.
    pub fn edit_at(&self, change: &str) -> Result<String, String> {
        self.jj_run(&["edit", change])
            .map(|()| format!("editing {change}"))
    }

    /// Restore the working copy from this change (`T073`).
    ///
    /// **Not the same verb as `edit_at` and not a synonym for it.** `edit`
    /// moves *where you are*; `restore` brings *what was there* to where you
    /// already are. `3b`'s subtitle calls undo time travel, and these are its
    /// two directions.
    pub fn restore_from(&self, change: &str) -> Result<String, String> {
        self.jj_run(&["restore", "--from", change])
            .map(|()| format!("restored from {change}"))
    }

    /// Run jj for its output, or [`None`] when it will not run.
    fn jj_text(&self, args: &[&str]) -> Option<String> {
        let out = Command::new("jj")
            .args(args)
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|out| out.status.success())?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Run jj for effect, carrying its own complaint back on failure.
    fn jj_run(&self, args: &[&str]) -> Result<(), String> {
        let out = Command::new("jj")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|error| format!("jj: {error}"))?;
        if out.status.success() {
            return Ok(());
        }
        let said = String::from_utf8_lossy(&out.stderr);
        Err(said
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("jj refused")
            .trim()
            .to_owned())
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

/// The template [`Repo::timeline`] asks jj for (`T073`).
///
/// Six tab-separated fields per change, in `3b`'s own reading order. Verified
/// against a real repository before the parser was written — the same rule
/// `T072`'s four captured `git status` states follow.
const JJ_TIMELINE: &str = concat!(
    r#"if(current_working_copy, "@", "o") ++ "\t" ++ "#,
    r#"change_id.short(4) ++ "\t" ++ "#,
    r#"author.email() ++ "\t" ++ "#,
    r#"diff.stat().total_added() ++ "\t" ++ "#,
    r#"diff.stat().total_removed() ++ "\t" ++ "#,
    r#"description.first_line() ++ "\n""#,
);

/// Parse [`JJ_TIMELINE`]'s output (`T073`).
///
/// **A free function over the text**, so the parsing half runs on a machine
/// with no jj — `T071`'s rule for detection and `T072`'s for `git status`.
///
/// **A row that does not have six fields is skipped rather than guessed at.**
/// jj's own root commit has no author and no description, and while `~root()`
/// filters it, a template that ever changes shape should lose a row rather
/// than produce one with fields shifted along by one.
fn read_jj_timeline(said: &str) -> Vec<Change> {
    said.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let marker = fields.next()?;
            let id = fields.next()?;
            let author = fields.next()?;
            let added = fields.next()?;
            let removed = fields.next()?;
            let description = fields.next()?;
            if id.is_empty() {
                return None;
            }
            Some(Change {
                id: id.to_owned(),
                working_copy: marker == "@",
                author: author.to_owned(),
                description: description.to_owned(),
                // A count that will not parse is zero rather than a dropped
                // row: the change is real either way and `+0 −0` is the honest
                // rendering of *"nothing measured"*.
                added: added.parse().unwrap_or(0),
                removed: removed.parse().unwrap_or(0),
            })
        })
        .collect()
}

/// The template [`Repo::operations`] asks jj for (`T073`).
const JJ_OPERATIONS: &str = r#"id.short(4) ++ "\t" ++ description ++ "\n""#;

/// Parse [`JJ_OPERATIONS`]'s output (`T073`).
///
/// **jj's root operation is dropped**, the way the timeline drops the root
/// commit: it has an id of zeroes and no description, and it is not something
/// anybody did.
fn read_jj_operations(said: &str) -> Vec<Operation> {
    said.lines()
        .filter_map(|line| {
            let (id, description) = line.split_once('\t')?;
            if id.is_empty() || description.trim().is_empty() {
                return None;
            }
            Some(Operation {
                id: id.to_owned(),
                description: description.to_owned(),
            })
        })
        .collect()
}

/// Parse `jj diff --git` (`T073`).
///
/// **A free function over the text**, so the parsing half needs no jj — the
/// rule `T071`'s detection, `T072`'s `git status` and `T073`'s timeline all
/// follow.
///
/// **Headers are dropped and hunk boundaries are not drawn.** `index`, `---`
/// and `+++` say nothing a reader of `3b` needs, and `@@` lines are jj's own
/// coordinates rather than content. What is kept is the three kinds of line
/// that *are* the diff.
fn read_jj_diff(said: &str) -> Vec<ChangeFile> {
    let mut files: Vec<ChangeFile> = Vec::new();
    for line in said.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = rest.split_once(" b/").map_or(rest, |(left, _)| left);
            files.push(ChangeFile {
                path: path.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        // Order matters: `+++` and `---` start with `+` and `-`.
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        if line.starts_with("index ") || line.starts_with("new file") || line.starts_with("deleted")
        {
            continue;
        }
        let mark = match line.chars().next() {
            Some('+') => Mark::Added,
            Some('-') => Mark::Removed,
            Some(' ') => Mark::Context,
            _ => continue,
        };
        file.lines.push((mark, line[1..].to_owned()));
    }
    files
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

    /// **The timeline, captured from a real repository.**
    ///
    /// The fixture is the literal output of the template in
    /// `crates/phosphor-vcs/src/lib.rs`, recorded before this parser existed.
    #[test]
    fn the_timeline_reads_six_fields_a_change() {
        let rows = super::read_jj_timeline(
            "@\tvmsp\ttrevor@strieber.org\t1\t0\tretry logic\n\
             o\tovkk\ttrevor@strieber.org\t88\t3\tscaffold fetch module\n",
        );
        assert_eq!(rows.len(), 2);

        // **Newest first, and the working copy is the first row** — `3b` draws
        // `@ a4f2` at the top and `○` beneath it.
        assert!(rows[0].working_copy);
        assert!(!rows[1].working_copy);
        assert_eq!(rows[0].id, "vmsp");
        assert_eq!(rows[0].description, "retry logic");
        assert_eq!((rows[1].added, rows[1].removed), (88, 3));
        assert_eq!(rows[1].author, "trevor@strieber.org");
    }

    /// **A short row is skipped rather than guessed at.**
    ///
    /// If the template ever changes shape, losing a row is recoverable and
    /// producing one with its fields shifted along by one is not — a change id
    /// that is really an email is the kind of wrong that reaches a screen
    /// looking plausible.
    #[test]
    fn a_row_with_the_wrong_shape_is_dropped() {
        let rows = super::read_jj_timeline(
            "@\tvmsp\tme@example.com\t1\t0\tfine\n\
             zzzz\n\
             \t\t\t\t\t\n\
             o\tovkk\tme@example.com\t2\t1\tsomething\tspare\n\
             o\tabcd\tme@example.com\t3\t2\n\
             o\tefgh\tme@example.com\t2\t1\talso fine\n",
        );
        // **The five-field row is the one that matters.** A row that is one
        // field *short* is the case a lenient parser turns into a change whose
        // description is really its removed-count — fields shifted along by
        // one, and entirely plausible on screen. A row with a spare field is
        // kept, because the six this parser needs are all present and jj
        // gaining a seventh should not blank the timeline.
        assert_eq!(rows.len(), 3, "three rows have all six fields");
        assert_eq!(rows[0].id, "vmsp");
        assert_eq!(rows[1].id, "ovkk");
        assert_eq!(rows[2].id, "efgh");
        assert!(
            rows.iter().all(|row| row.id != "abcd"),
            "the five-field row is dropped rather than shifted"
        );
    }

    /// **A change's diff, captured from a real repository.**
    ///
    /// The literal output of `jj diff -r <id> --git`, recorded before this
    /// parser existed — `T072`'s rule about writing fixtures from memory.
    #[test]
    fn a_change_diff_keeps_the_lines_and_drops_the_headers() {
        let files = super::read_jj_diff(
            "diff --git a/b.txt b/b.txt\n\
             new file mode 100644\n\
             index 0000000000..ef49dd86a6\n\
             --- /dev/null\n\
             +++ b/b.txt\n\
             @@ -0,0 +1,1 @@\n\
             +more\n",
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "b.txt");
        // **One line kept out of seven.** `index`, `---`, `+++` and `@@` are
        // coordinates rather than content, and `---`/`+++` in particular would
        // otherwise read as a removed and an added line.
        assert_eq!(
            files[0].lines,
            vec![(super::Mark::Added, "more".to_owned())]
        );
    }

    /// **`---` and `+++` are headers, not a removal and an addition.**
    ///
    /// The trap this parser is one `starts_with` away from: both begin with the
    /// character that marks a changed line, so a naïve match turns every file
    /// header into two phantom edits — and the result looks entirely plausible
    /// on screen.
    ///
    /// **The context line's leading space is written `\x20`** because Rust's
    /// `\` line-continuation eats the newline *and* the indentation after it —
    /// so ` kept` written naturally arrives as `kept`, the parser sees no
    /// space, and the test fails against correct code. It did, once.
    #[test]
    fn a_file_header_is_not_two_changed_lines() {
        let files = super::read_jj_diff(
            "diff --git a/a.txt b/a.txt\n\
             --- a/a.txt\n\
             +++ b/a.txt\n\
             @@ -1,2 +1,2 @@\n\
             -was\n\
             +now\n\
             \x20kept\n",
        );
        assert_eq!(
            files[0].lines,
            vec![
                (super::Mark::Removed, "was".to_owned()),
                (super::Mark::Added, "now".to_owned()),
                (super::Mark::Context, "kept".to_owned()),
            ]
        );
    }

    /// **The op log, captured, with jj's root operation dropped.**
    ///
    /// That row has an id of zeroes and no description; it is not something
    /// anybody did, the same way the timeline's root commit is not a change.
    #[test]
    fn the_op_log_drops_the_root_operation() {
        let ops = super::read_jj_operations(
            "63f0\tsnapshot working copy\n\
             58e1\tnew empty commit\n\
             79a0\tadd workspace 'default'\n\
             0000\t\n",
        );
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].id, "63f0");
        assert_eq!(ops[0].description, "snapshot working copy");
        assert!(ops.iter().all(|op| op.id != "0000"));
    }

    /// **A git repository has no timeline, and that is not a failure.**
    ///
    /// `3b` is *"an enhancement view, only when jj is present"*. `CP-8c` runs
    /// the whole `S7` set in a git repo and fails if any message implies
    /// something is missing — so this answers empty rather than refusing.
    ///
    /// **The `backend != Jj` guard inside `timeline` is an equivalent mutant to
    /// this test, and is named rather than covered.** Removing it does not
    /// change what a git repository answers: `jj log` fails in a directory with
    /// no `.jj`, `jj_text` filters on the exit status, and the result is the
    /// same empty vector. What the guard actually buys is *not spawning a
    /// process to be told so* — a cost property, which no assertion about the
    /// return value can distinguish. Planting its removal is caught by nothing
    /// here, and inventing a test that watched for a subprocess would be
    /// testing the guard rather than the behaviour.
    #[test]
    fn a_git_repo_has_an_empty_timeline() {
        let dir = scratch("git-timeline");
        std::fs::create_dir_all(dir.join(".git")).expect("a git marker");
        let found = detect(&dir).expect("a repo");
        assert!(found.timeline(None).is_empty());
        std::fs::remove_dir_all(&dir).ok();
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
