//! `T086` — screen `6d` (`:help agent-objects`) as a **Tier-1 golden frame**.
//!
//! `CP-3` names *"the `6d` snapshot"* among the things Claude verifies, and the
//! plan's `S3` acceptance is *"`6d` renders from the live keymap"*. Tier 1 is
//! the committed cell grid, and the only tier that gates CI (`TASKS.md`'s tier
//! table).
//!
//! # The whole page is composed out of the live table
//!
//! Nothing here is transcribed from the mockup. `6d`'s claim is *"vim's
//! composability with agent-native nouns"*, so the page is built the way the
//! grammar is: a head (`v`, `d`), `i`, and an agent noun — **each of the three
//! found in `runtime/keymaps.scm` by its role**, never by its key. The row's
//! key is those three keys concatenated and its verb is those three verbs, so
//! rebinding any one of them at the REPL moves every row it appears in, which
//! is what `a_repl_rebind_shows_up_in_the_help_grid` asserts.
//!
//! Two consequences worth stating, because they are what "from the live
//! keymap" has to mean to be worth claiming:
//!
//! * The **keys** are whatever plays those roles today. Nothing in this file
//!   spells `v`, `i`, `d`, `u`, `h`, `t` or `b`.
//! * The **words** are `keymaps.scm`'s. `6d` writes *"select inner unseen
//!   region"*; the table says `visual`, `inside`, `unseen region`, and the page
//!   draws what the table says. Inventing the mockup's phrasing here would be
//!   a help page that stops being true the moment a verb changes.
//!
//! # What the shipped table cannot say
//!
//! `6d` draws eight grammar rows. Four of them are compositions of bindings
//! that exist (`viu`, `dih`, and their siblings over the other nouns) and are
//! drawn. The rest name keys nothing binds — `sib` (`s` is *substitute
//! character* in the shipped table, not a mark-seen operator), `]u` / `[u`,
//! `:'<,'>c`, `:g/TODO/c`, `"ay ib` and `q:`. They are listed in the snapshot's
//! own notes with the task that owns each, rather than typed in here as
//! decoration: a help page that lists a key you cannot press is worse than one
//! that is short.
//!
//! # Why it lives in the binary crate
//!
//! Same reason as `screen_6b.rs` and `screen_3c.rs`: a frame composed from
//! Steel and drawn by the interpreter needs `phosphor-steel` **and**
//! `phosphor-ui` at once, and `phosphor-ui` may not have the first
//! (`scripts/lint-no-store-mutation.sh` check 2). The serialiser is `T018`'s,
//! included by path.

// `T018`'s golden-frame serialiser, from the crate that owns it.
#[path = "../../phosphor-ui/tests/frame_grid/mod.rs"]
mod frame_grid;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frame_grid::Frame;
use phosphor_core::input::table::Role;
use phosphor_core::request::{KeySeq, TextObject};
use phosphor_core::view::{
    Axis, Child, Constraint, Density, Float, FloatHeader, KeyHint, Millis, Mood, Node,
    SessionState, Slot, Tree,
};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::keymap::{self, Entry};
use phosphor_steel::runtime::Runtime;
use phosphor_steel::status::{self, StatusVm};
use phosphor_ui::interpret::{Interpreter, NoResources};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// The four agent nouns, in `6d`'s own order (*"u unseen region · h hunk · t
/// thread · b review block"*).
///
/// The **objects**, not their keys: which key plays each is the live table's
/// business, which is the whole point of reading it.
const AGENT_NOUNS: &[TextObject] = &[
    TextObject::UnseenRegion,
    TextObject::Hunk,
    TextObject::Thread,
    TextObject::Block,
];

/// The topic, as the ex line names it (`h[elp]` takes one).
const TOPIC: &str = ":help agent-objects";

fn runtime_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// The shipped editor layer, booted clean.
fn layer() -> Runtime {
    let runtime = Runtime::boot(Some(&runtime_tree()), Arc::new(Detached) as Arc<dyn Host>);
    assert!(
        runtime.report().is_clean(),
        "the shipped layer does not boot: {:?}",
        runtime.report().faults
    );
    runtime
}

/// The first binding in `scope` whose role `wanted` accepts.
fn bound<'a>(
    entries: &'a [Entry],
    scope: &str,
    wanted: impl Fn(&Role) -> bool,
) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|entry| entry.scope == scope && entry.role.as_ref().is_some_and(&wanted))
}

/// `6d`'s grammar, composed from the live table.
///
/// One row per (head, noun): the head's key and verb, `i`'s, and the noun's.
/// A head or a noun nobody binds contributes no row rather than a row with a
/// hole in it.
fn agent_object_hints(runtime: &mut Runtime) -> Vec<KeyHint> {
    let entries = keymap::entries(runtime);
    let Some(inner) = bound(&entries, "operator-pending", |role| {
        matches!(role, Role::Inner)
    }) else {
        return Vec::new();
    };

    // `6d`'s illustrations: select one, delete one — *"revert claude's edit,
    // plain vim delete"* — and **mark one seen**, which is the row `6d` draws as
    // `sib` and this build spells `gsib`.
    //
    // **The third head is named and the first two are not**, and that asymmetry
    // is the point rather than an inconsistency. `Select` and `Operator` take
    // whatever plays the role, because `6d`'s claim about them is *"whatever
    // your select and delete are"*. Mark-seen is not any operator — it is the
    // one the screen is about — so asking for the first `Operator` found it
    // `d` and drew `dib` twice, with no slot left for `gsib` however the keymap
    // was written. `T086`'s own entry recorded that as the outstanding item.
    let heads = [
        bound(&entries, "normal", |role| matches!(role, Role::Select(_))),
        bound(&entries, "normal", |role| matches!(role, Role::Operator(_))),
        bound(&entries, "normal", |role| {
            matches!(
                role,
                Role::Operator(phosphor_core::input::table::Operator::MarkSeen)
            )
        }),
    ];

    let mut hints = Vec::new();
    for head in heads.into_iter().flatten() {
        for noun in AGENT_NOUNS {
            let Some(object) = bound(
                &entries,
                "object",
                |role| matches!(role, Role::Object { object, .. } if object == noun),
            ) else {
                continue;
            };
            hints.push(KeyHint {
                key: KeySeq(format!("{}{}{}", head.keys.0, inner.keys.0, object.keys.0)),
                verb: format!("{} {} {}", head.verb, inner.verb, object.verb),
            });
        }
    }
    hints
}

/// `6d`'s statusline: the help surface, and no file.
fn status_vm() -> StatusVm {
    StatusVm {
        mode: "normal".to_owned(),
        surface: Some("help".to_owned()),
        file: None,
        session: SessionState::None,
        since: None::<Millis>,
        ask_pending: false,
        unseen: 0,
        // No diagnostic on this screen. `2b` is the one that
        // draws `■ N`, and it has no golden frame.
        trouble: 0,
        attention: 0,
        vcs: None,
        server: None,
        cursor: None,
        hints: Vec::new(),
    }
}

/// The screen: an empty pane, the statusline, and the `:help` float over both.
fn tree(runtime: &mut Runtime) -> Tree {
    let hints = agent_object_hints(runtime);
    assert!(
        hints.len() == 12,
        "the shipped table binds three heads, `i`, and four agent nouns: {hints:?}"
    );
    let status = status::compose(runtime, &status_vm()).expect("runtime/statusline.scm composes");

    Tree::new(Node::split(
        Axis::Rows,
        [
            Slot::new(Constraint::Fill { weight: 1 }, Node::Empty {}),
            Slot::new(Constraint::Cells { cells: 1 }, status),
        ],
    ))
    .with_float(Float {
        mood: Mood::Informational,
        header: Some(FloatHeader::new(TOPIC)),
        body: Child::new(Node::KeyHints {
            density: Density::Help,
            hints,
        }),
        footer: Some(Child::new(Node::KeyHints {
            density: Density::Footer,
            hints: vec![KeyHint {
                key: KeySeq("q".to_owned()),
                verb: "close".to_owned(),
            }],
        })),
    })
}

/// What this frame is missing, and which task owns each absence.
///
/// `T018`'s rule: *"nobody has to reverse-engineer an absence."* Every line was
/// checked against the tree in the session that wrote it.
const NOTES: &[&str] = &[
    "Every row is composed from runtime/keymaps.scm by role — the head, `i`,",
    "  and the noun — so the keys are whatever plays those roles today and the",
    "  words are the table's own. 6d writes \"select inner unseen region\"; the",
    "  shipped verbs say \"visual inside unseen region\".",
    "Three of 6d's grammar rows are drawn now and were not when this frame was",
    "  first written; the notes below are what is true against the tree today,",
    "  not when the snapshot was first accepted:",
    "  `sib` is `gsib`. Teej ruled on 2026-08-12 that `s` stays vim's substitute",
    "    and mark-seen moves to `gs` (runtime/keymaps.scm binds it as an",
    "    operator), so 6d's own sentence is the thing that is wrong — recorded",
    "    as an amendment in docs/README.md, not folded into the .dc.html.",
    "  `]u` / `[u` are bound, to `goto-sequence` over unseen regions. They",
    "    resolve and decline by name until the store lands (T041/T049).",
    "  `:'<,'>c` runs. The ex line grew a range grammar, so the selection range",
    "    is read rather than swallowed into a command name, and `c[omment]` is",
    "    bound — `:c` resolves to it, `cl[aude]` needing two letters. This note",
    "    said *there is still no `:c` command* until 2026-08-23, which had been",
    "    false since the comment verb landed: the frame did not move, so insta",
    "    passed the prose with it.",
    "  `\"ay ib`, `q:` — register-into-prompt and command history, still T058.",
    "6d draws its noun letters in you-blue and the sequences in claude green;",
    "  view::KeyHint carries no distinction, so every key draws in the claude",
    "  hue, as 3c draws the namespace. Flagged, not folded in.",
    "6d's `nouns  u unseen region  h hunk …` header line is four objects on one",
    "  row. The objects are here, in the verbs; an inline label + pairs row is a",
    "  composed Spans row rather than a keymap surface, and composition is",
    "  runtime/'s (spine).",
    "6d draws this as a full-width surface with `q close` in the statusline;",
    "  TASKS.md T086 and the Component Breakdown both call HelpGrid a Float",
    "  body, so it is drawn as one — centered at 120, docked at 80 (§11).",
    "At 80 the docked float covers the statusline: interpret.rs hands the float",
    "  the whole screen rect, while 8d (and T084's own tests) dock it above the",
    "  statusline because the host passes the body rect. The tree cannot say",
    "  where a float may land; that is spine's, and it is in the report.",
    "The agent nouns render here and resolve at T049 (Q8), so pressing one of",
    "  these sequences selects nothing yet.",
];

/// Renders the tree at `width`.
fn screen(runtime: &mut Runtime, theme: &Theme, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let tree = tree(runtime);
    let report = Interpreter::new(theme, &NoResources).render(&tree, area, &mut buf);
    assert!(
        report.deferred.is_empty(),
        "`6d` needs a primitive that does not exist: {:?}",
        report.deferred
    );
    buf
}

/// Commits one width as a golden frame.
fn golden(name: &'static str, width: u16) {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    let buf = screen(&mut runtime, &theme, width, 24);
    let frame = Frame {
        screen: name,
        theme_label: "phosphor-dark",
        theme: &theme,
        notes: NOTES,
    };

    // §12's other half, which no grep lint can reach: a colour on screen that
    // no `Theme` field names.
    assert!(
        frame.unnamed(&buf).is_empty(),
        "colours on screen that no Theme field names: {:?}",
        frame.unnamed(&buf)
    );

    insta::assert_snapshot!(name, frame.to_text(&buf));
}

/// The screen, at the width the `CP-1` golden frames use.
#[test]
fn screen_6d_draws() {
    golden("6d", 120);
}

/// The same screen at 80 columns, where §11 sends the float full-width — which
/// is also the shape `6d` itself draws.
#[test]
fn screen_6d_draws_at_80_columns() {
    golden("6d-80", 80);
}

/// `T086`'s second acceptance half: *"a REPL rebind shows up in it."*
///
/// The rebind here is a **noun**, which is the strongest form of the claim: the
/// page is built by role, so moving `unseen-region` onto another key moves
/// every row it composes into, and renaming it renames every verb.
#[test]
fn a_repl_rebind_shows_up_in_the_help_grid() {
    let mut runtime = layer();
    let theme = Theme::phosphor_dark();
    // **Taller than `6d`'s own frame, and deliberately.** The grid grew a third
    // head when `T086` made mark-seen one, so twelve grammar rows do not fit a
    // 24-row screen and the float says so — `KeyHints` at `Density::Help` spends
    // its last row on the count it dropped. That is the shipped behaviour and it
    // is honest; it is just not this test's subject, which is that a *rebind*
    // reaches the grid at all.
    let before = rows(&screen(&mut runtime, &theme, 120, 40)).join("\n");
    assert!(before.contains("visual inside unseen region"), "{before}");

    // Move the unseen-region object from `u` to `U`, and say so differently.
    let outcome = runtime.evaluate(r#"(keymap-remove! "u" "object")"#);
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );
    let outcome = runtime.evaluate(
        r#"(keymap-set! "U" (key/object "unseen-region") "region claude wrote" "object")"#,
    );
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );

    let after = rows(&screen(&mut runtime, &theme, 120, 40)).join("\n");
    // **The key and the verb, not the spacing between them.** The gap is the
    // key column's padding, which is the width of the *longest* key in the
    // table — so `T086` adding `gsib` moved it from two spaces to three and
    // this assertion failed on a grid that was entirely correct. What the test
    // is about is that the rebind reached the row.
    let row = after
        .lines()
        .find(|line| line.contains("viU"))
        .unwrap_or_else(|| panic!("the rebound key is on screen:\n{after}"));
    assert!(
        row.contains("visual inside region claude wrote"),
        "and its verb is beside it; row was: {row:?}"
    );
    assert!(!after.contains("viu"), "and the old key is not:\n{after}");
}

/// A buffer's rows as trimmed text.
fn rows(buf: &Buffer) -> Vec<String> {
    (buf.area.y..buf.area.bottom())
        .map(|y| {
            (buf.area.x..buf.area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}
