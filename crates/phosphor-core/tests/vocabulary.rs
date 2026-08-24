//! `T019`'s acceptance criterion, as tests.
//!
//! *Done when: every mutation in phases S3–S8 has a named Action, even if
//! unimplemented* (`docs/TASKS.md`, `T019`). That sentence is only worth
//! anything if something checks it, so three things here do:
//!
//! 1. **[`checklist_covers_the_registry`] / [`registry_covers_the_checklist`]** —
//!    the vocabulary against the committed checklist in `tests/surfaces.txt`,
//!    both directions. Adding a capability without recording it, or deleting one
//!    the checklist still claims, is a failing test.
//! 2. **[`every_mutating_task_in_s3_to_s8_has_a_capability`]** — the checklist
//!    against `docs/TASKS.md` itself. This is the one that catches the failure
//!    the criterion is really about: a whole *task* — a surface — with no verb
//!    in the vocabulary. Tasks that legitimately have none are listed in
//!    [`NO_CAPABILITY`] with a reason each, so "nothing to add here" is a
//!    decision on the record rather than an omission.
//! 3. **[`every_capability_round_trips`]** — every registered capability
//!    encoded from its own declared parameters and decoded back. That is what
//!    proves each variant's decoder agrees with the schema `T020` will generate
//!    from the same row, for all of them, without 150 hand-written examples.
//!
//! # Regenerating the checklist
//!
//! `PHOSPHOR_WRITE_SURFACES=1 cargo nextest run -p phosphor-core` rewrites
//! `tests/surfaces.txt` from the registry and then passes. Commit the diff — it
//! is the record of what the vocabulary gained or lost, and reviewing it is the
//! point.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use phosphor_core::action::{ACTIONS, ALIASES, Action};
use phosphor_core::query::{QUERIES, Query};
use phosphor_core::registry::{Capability, CapabilityKind, McpPolicy, Param, ParamType, sample};
use phosphor_core::value::Value;
use phosphor_core::{registry, request};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Tasks in S3–S8 that legitimately add no capability, and why.
///
/// Every entry is a task that draws, decodes or persists something already in
/// the vocabulary. If you are adding a task here, the question to answer first
/// is *"what verb does a user or an agent name to reach this?"* — if there is
/// one, it belongs in the enum instead.
const NO_CAPABILITY: &[(&str, &str)] = &[
    (
        "T027",
        "kitty keyboard protocol — a decoder in phosphor-term; it produces the same keys the \
         input machine already names",
    ),
    (
        "T030",
        "undo persistence in phosphor-core — the journal writing and replaying itself, on \
         `T044`'s precedent one line down: persistence is where a verb lands, not a verb. It \
         held `compact-history` until 2026-08-24, when that row was re-stamped to `T095`. The \
         stamp is the address a refusal prints — *\"not built yet — T030 builds it\"* — and \
         `T030` is ticked, so it named finished work; `T095` is the task whose own text is \
         *\"`journal.rs` implements compaction and proves it under a real `SIGKILL`, and \
         nothing triggers it\"*. See scripts/lint-refusal-tasks.sh",
    ),
    (
        "T031",
        "GutterBar — a widget. It renders region, diagnostic and VCS state the store already \
         holds; nothing about it is a mutation",
    ),
    (
        "T034",
        "KeymapFooter / which-key — a widget over the `keymap` query",
    ),
    (
        "T043",
        "line + content fallback anchoring — the second tier of `reanchor`, not a second verb",
    ),
    (
        "T044",
        "seen-state persistence — the store writing its own state to disk; `mark-seen` is the \
         verb, this is where it lands",
    ),
    (
        "T052",
        "the MCP server — it is *generated from* the registry (T020); a capability of its own \
         would be circular",
    ),
    (
        "T055",
        "markdown prose behind the gate — rendering of transcript chunks that `session-prose` \
         already delivers",
    ),
    (
        "T063",
        "DiffBody — a widget, and the same answer `T031` got. It draws rows a host hands it \
         through `Resources::diff`; it computes no diff, holds no hunk and mutates nothing. \
         The three capabilities that used to cite this task all moved on the day it was \
         ticked, because a refusal saying \"T063 builds it\" would name a task that is done: \
         `set-diff-mode` and `expand-diff-context` act on a diff that is on screen and went \
         to `T066` beside `open-review-block`, and `hunks` answers \"a block's hunks, with \
         each one's seen state\" — which is `T064`'s sentence, not this one's",
    ),
    (
        "T072",
        "git adapter — the same VCS trait as jj behind the same capabilities",
    ),
    (
        "T087",
        "region tints via a marks side table — a rendering detail of regions the store already \
         has",
    ),
    ("T089", "TabBar — a widget over the `panes` query"),
    (
        "T091",
        "real VM invocations measured in the binary — a measurement of the loop, not a verb \
         anyone names. Nothing about it is reachable from a door",
    ),
];

/// The phases `T019`'s acceptance criterion covers.
const REQUIRED_PHASES: &[&str] = &["S3", "S4", "S5", "S6", "S7", "S8"];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn surfaces_path() -> PathBuf {
    crate_dir().join("tests/surfaces.txt")
}

fn tasks_path() -> PathBuf {
    crate_dir().join("../../docs/TASKS.md")
}

/// One checklist line: phase, task, capability name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
    phase: String,
    task: String,
    name: String,
}

fn registry_entries() -> Vec<Entry> {
    registry::capabilities()
        .into_iter()
        .map(|cap| Entry {
            phase: cap.since.phase.as_str().to_owned(),
            task: cap.since.task.to_owned(),
            name: cap.name.to_owned(),
        })
        .collect()
}

fn render_checklist(entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort();

    let mut out = String::new();
    out.push_str(
        "# T019 — the committed surface checklist.\n\
         #\n\
         # One line per registered capability: <phase> <task> <name>. Sorted, so a\n\
         # diff reads as \"what the vocabulary gained or lost\" and nothing else.\n\
         #\n\
         # tests/vocabulary.rs checks this file against the registry in both\n\
         # directions, and checks the task column against docs/TASKS.md — so a\n\
         # phase that grows a surface with no verb behind it is a red test rather\n\
         # than a discovery in Window F.\n\
         #\n\
         # Regenerate with PHOSPHOR_WRITE_SURFACES=1 cargo nextest run -p phosphor-core,\n\
         # then read the diff before committing it.\n\n",
    );
    for entry in sorted {
        let _ = writeln!(out, "{} {} {}", entry.phase, entry.task, entry.name);
    }
    out
}

fn read_checklist() -> Vec<Entry> {
    let path = surfaces_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let phase = parts.next().expect("a checklist line starts with a phase");
            let task = parts
                .next()
                .unwrap_or_else(|| panic!("no task in `{line}`"));
            let name = parts
                .next()
                .unwrap_or_else(|| panic!("no name in `{line}`"));
            assert!(
                parts.next().is_none(),
                "checklist line has trailing junk: `{line}`"
            );
            Entry {
                phase: phase.to_owned(),
                task: task.to_owned(),
                name: name.to_owned(),
            }
        })
        .collect()
}

/// Regenerates `tests/surfaces.txt` when asked, so the checklist is cheap to
/// keep honest and impossible to forget.
fn maybe_rewrite_checklist(entries: &[Entry]) {
    if std::env::var_os("PHOSPHOR_WRITE_SURFACES").is_none() {
        return;
    }
    let path = surfaces_path();
    std::fs::write(&path, render_checklist(entries))
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

/// Every `T0xx` task id in `docs/TASKS.md`, grouped by the `## S<n>` heading it
/// sits under.
fn tasks_by_phase() -> BTreeMap<String, BTreeSet<String>> {
    let path = tasks_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} — this test reads the task breakdown on purpose, so the \
             vocabulary cannot silently fall behind it: {error}",
            path.display()
        )
    });

    let mut phases: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            current = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            continue;
        }
        let Some(rest) = line
            .strip_prefix("- [ ] **")
            .or_else(|| line.strip_prefix("- [x] **"))
        else {
            continue;
        };
        let id: String = rest
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if id.starts_with('T') && id.len() == 4 && !current.is_empty() {
            phases.entry(current.clone()).or_default().insert(id);
        }
    }
    phases
}

// ---------------------------------------------------------------------------
// The checklist
// ---------------------------------------------------------------------------

#[test]
fn checklist_covers_the_registry() {
    let entries = registry_entries();
    maybe_rewrite_checklist(&entries);
    let committed: BTreeSet<Entry> = read_checklist().into_iter().collect();

    let missing: Vec<&Entry> = entries
        .iter()
        .filter(|entry| !committed.contains(*entry))
        .collect();

    assert!(
        missing.is_empty(),
        "{} capability/-ies are registered but not in tests/surfaces.txt: {missing:#?}\n\
         Add the lines, or regenerate with PHOSPHOR_WRITE_SURFACES=1 and read the diff.",
        missing.len()
    );
}

#[test]
fn registry_covers_the_checklist() {
    let entries: BTreeSet<Entry> = registry_entries().into_iter().collect();
    let committed = read_checklist();

    let stale: Vec<&Entry> = committed
        .iter()
        .filter(|entry| !entries.contains(*entry))
        .collect();

    assert!(
        stale.is_empty(),
        "tests/surfaces.txt claims {} capability/-ies the registry does not have: {stale:#?}\n\
         A renamed capability shows up here twice — once stale, once missing.",
        stale.len()
    );
}

#[test]
fn every_mutating_task_in_s3_to_s8_has_a_capability() {
    let committed = read_checklist();
    let covered: BTreeSet<&str> = committed.iter().map(|entry| entry.task.as_str()).collect();
    let exempt: BTreeSet<&str> = NO_CAPABILITY.iter().map(|(task, _)| *task).collect();
    let phases = tasks_by_phase();

    let mut uncovered: Vec<String> = Vec::new();
    for phase in REQUIRED_PHASES {
        let tasks = phases.get(*phase).unwrap_or_else(|| {
            panic!("docs/TASKS.md has no `## {phase}` section — has the phase list changed?")
        });
        for task in tasks {
            if !covered.contains(task.as_str()) && !exempt.contains(task.as_str()) {
                uncovered.push(format!("{phase} {task}"));
            }
        }
    }

    assert!(
        uncovered.is_empty(),
        "these S3–S8 tasks have no capability in the vocabulary and no entry in \
         NO_CAPABILITY: {uncovered:#?}\n\
         T019's acceptance criterion is that every mutation in S3–S8 has a named Action, \
         even if unimplemented. Either name one, or record here why the task needs none.",
    );
}

/// Tasks a capability may name that `docs/TASKS.md` does not define.
///
/// One entry, and it is a version rather than a task: `v1.5` is post-1.0 work
/// the breakdown deliberately stops short of. Anything else here would be a
/// capability parked under an id nobody can look up.
const NOT_IN_TASKS: &[&str] = &["v1.5"];

/// Every capability's `Since.task` names a task that exists.
///
/// `surfaces.txt`'s header claims this check runs, and until this test it did
/// not: `tasks_by_phase()` was read for the S3–S8 coverage direction and for
/// the exemption list, and nothing looked at the checklist's own task column.
/// A capability could cite `T999` — or keep citing a task after it was
/// renumbered — and every gate would pass it.
///
/// Which is the defect class `CLAUDE.md` names: prose asserting a check that
/// is not performed, in the one file whose job is to describe the check.
#[test]
fn every_capability_names_a_task_that_exists() {
    let phases = tasks_by_phase();
    let all: BTreeSet<&String> = phases.values().flatten().collect();

    let mut dangling: Vec<String> = Vec::new();
    for capability in registry::capabilities() {
        let task = capability.since.task;
        if NOT_IN_TASKS.contains(&task) {
            continue;
        }
        if !all.contains(&task.to_owned()) {
            dangling.push(format!("{} cites {task}", capability.name));
        }
    }

    assert!(
        dangling.is_empty(),
        "these capabilities name a task docs/TASKS.md does not define: {dangling:#?}\n\
         A task id in the registry is a claim about the breakdown. Either the task was \
         renumbered and the row is stale, or the id was invented.",
    );
}

#[test]
fn exemptions_are_real_tasks_with_reasons() {
    let phases = tasks_by_phase();
    let all: BTreeSet<&String> = phases.values().flatten().collect();

    for (task, reason) in NO_CAPABILITY {
        assert!(
            all.contains(&(*task).to_owned()),
            "NO_CAPABILITY names `{task}`, which is not a task in docs/TASKS.md"
        );
        assert!(
            reason.len() > 20,
            "`{task}` is exempt with no real reason recorded"
        );
    }
}

// ---------------------------------------------------------------------------
// The registry itself
// ---------------------------------------------------------------------------

#[test]
fn door_names_are_globally_unique() {
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for cap in registry::capabilities() {
        if let Some(previous) = seen.insert(cap.name, cap.domain) {
            panic!(
                "`{}` is registered twice — in `{previous}` and in `{}`. Door names are flat \
                 and global (Q6 fixes phosphor/declare-review-block literally), so two \
                 capabilities cannot share one.",
                cap.name, cap.domain
            );
        }
    }

    for alias in ALIASES {
        assert!(
            !seen.contains_key(alias.alias),
            "alias `{}` collides with a canonical capability name",
            alias.alias
        );
        assert!(
            seen.contains_key(alias.canonical),
            "alias `{}` points at `{}`, which is not registered",
            alias.alias,
            alias.canonical
        );
    }
}

#[test]
fn every_capability_round_trips() {
    for cap in registry::capabilities() {
        let args = cap.sample_args();
        match cap.kind {
            CapabilityKind::Action => {
                let action = Action::from_call(cap.name, &args).unwrap_or_else(|error| {
                    panic!(
                        "`{}` cannot be decoded from its own schema: {error}",
                        cap.name
                    )
                });
                assert_eq!(action.name(), cap.name);
                assert_eq!(action.domain(), cap.domain);
                assert_eq!(action.spec().name, cap.name);
                let call = action.to_call();
                assert_eq!(call.name, cap.name);
                assert_eq!(
                    call.args, args,
                    "`{}` does not re-encode to what it decoded from",
                    cap.name
                );
            }
            CapabilityKind::Query => {
                let query = Query::from_call(cap.name, &args).unwrap_or_else(|error| {
                    panic!(
                        "`{}` cannot be decoded from its own schema: {error}",
                        cap.name
                    )
                });
                assert_eq!(query.name(), cap.name);
                assert_eq!(query.domain(), cap.domain);
                assert_eq!(query.spec().name, cap.name);
                let call = query.to_call();
                assert_eq!(call.name, cap.name);
                assert_eq!(
                    call.args, args,
                    "`{}` does not re-encode to what it decoded from",
                    cap.name
                );
            }
        }
    }
}

#[test]
fn every_capability_is_documented_and_named_for_three_doors() {
    for cap in registry::capabilities() {
        assert!(!cap.doc.is_empty(), "`{}` has no doc line", cap.name);
        assert!(
            cap.name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "`{}` is not kebab-case; the CLI verb, the JSON tool name and the Steel binding \
             all derive from it",
            cap.name
        );
        assert!(
            !cap.name.starts_with('-') && !cap.name.ends_with('-'),
            "`{}` has a dangling dash",
            cap.name
        );
        assert_eq!(cap.mcp_name(), format!("phosphor/{}", cap.name));
        assert_eq!(cap.cli_verb(), cap.name);
        assert_eq!(
            cap.steel_name(),
            match cap.kind {
                CapabilityKind::Action => format!("{}!", cap.name),
                CapabilityKind::Query => cap.name.to_owned(),
            }
        );
        check_params(cap.name, cap.params);
    }
}

fn check_params(capability: &str, params: &[Param]) {
    let mut seen = BTreeSet::new();
    for param in params {
        assert!(
            !param.doc.is_empty(),
            "`{capability}`'s `{}` has no doc line — it becomes the MCP schema's description",
            param.name
        );
        assert!(
            seen.insert(param.name),
            "`{capability}` declares `{}` twice",
            param.name
        );
        check_type(capability, param.name, &param.ty);
    }
}

fn check_type(capability: &str, param: &str, ty: &ParamType) {
    match ty {
        ParamType::Choice(tags) => assert!(
            !tags.is_empty(),
            "`{capability}`'s `{param}` is a choice of nothing"
        ),
        ParamType::List(inner) => check_type(capability, param, inner),
        ParamType::Record(fields) => check_params(capability, fields),
        ParamType::Union(variants) => {
            assert!(
                !variants.is_empty(),
                "`{capability}`'s `{param}` is a union of nothing"
            );
            for variant in *variants {
                assert!(
                    !variant.doc.is_empty(),
                    "`{capability}`'s `{param}` has an undocumented `{}` arm",
                    variant.tag
                );
                check_params(capability, variant.fields);
            }
        }
        _ => {}
    }
}

#[test]
fn the_users_keyboard_is_denied_to_agents_by_default() {
    for spec in ACTIONS {
        if spec.domain == "input" {
            assert_eq!(
                spec.mcp,
                McpPolicy::Deny,
                "`{}` hands an agent the user's keyboard and must be Deny by default; a rule \
                 in init.scm opens it (T061)",
                spec.name
            );
        }
    }

    let eval = ACTIONS
        .iter()
        .find(|spec| spec.name == "eval")
        .expect("the CLI door's own capability is registered");
    assert_eq!(eval.mcp, McpPolicy::Deny);
}

#[test]
fn focus_relative_targets_are_identifiable_for_the_mcp_refusal() {
    // The MCP door refuses these; that decision has to be answerable from the
    // target alone, without consulting a table.
    let focus_relative = [
        request::Target::Cursor {},
        request::Target::Selection {},
        request::Target::PickerRow {},
        request::Target::FloatRow {},
    ];
    for target in focus_relative {
        assert!(target.focus_relative(), "{target:?} should be refused");
    }
    assert!(
        !request::Target::Region {
            id: request::RegionId(1)
        }
        .focus_relative()
    );
}

#[test]
fn the_vocabulary_reaches_every_phase_it_claims_to() {
    let entries = registry_entries();
    for phase in REQUIRED_PHASES {
        assert!(
            entries.iter().any(|entry| entry.phase == *phase),
            "nothing in the vocabulary lands at {phase} — T019 designs for all the mockups, \
             not just S1's"
        );
    }
    assert!(
        !QUERIES.is_empty() && !ACTIONS.is_empty(),
        "both halves of the API exist"
    );
}

#[test]
fn samples_are_total_over_the_type_language() {
    // Every shape the vocabulary uses has a canonical example, which is what the
    // round-trip test above rests on. A ParamType with no sample would make that
    // test silently weaker rather than fail, so check it directly.
    for cap in registry::capabilities() {
        for param in cap.params {
            let value = sample(&param.ty);
            assert_ne!(
                value,
                Value::Null,
                "`{}`'s `{}` has no canonical example",
                cap.name,
                param.name
            );
        }
        if let Some(returns) = cap.returns {
            let _: Value = sample(&returns);
        }
    }
}

#[test]
fn the_checklist_file_is_the_shape_the_tests_assume() {
    let path = surfaces_path();
    assert!(
        Path::new(&path).exists(),
        "tests/surfaces.txt is missing — it is the committed half of T019's acceptance \
         criterion, not a cache"
    );
    let entries = read_checklist();
    assert!(entries.len() > 100, "the checklist looks truncated");
    let mut sorted = entries.clone();
    sorted.sort();
    assert_eq!(
        entries, sorted,
        "tests/surfaces.txt is not sorted; regenerate it with PHOSPHOR_WRITE_SURFACES=1"
    );
}

#[test]
fn capability_lookup_finds_what_the_doors_will_look_up() {
    let cap: Capability = registry::lookup("mark-seen").expect("s is the whole awareness loop");
    assert_eq!(cap.kind, CapabilityKind::Action);
    assert_eq!(cap.since.task, "T041");
    assert!(registry::lookup("mark-read").is_none());
}
