//! `T022`'s acceptance criterion: **screen `6b` reproduces.**
//!
//! The four lines below are transcribed from `docs/design/TUI Mockups.dc.html`
//! (`6b`, lines 492–505) and typed into a real session over the shipped runtime
//! tree. What is asserted here is everything about that screen this phase can be
//! held to: the rows, in the order and with the blank lines the drawing stacks
//! them in; the prompt and answer glyphs; the header; the statusline; and the
//! one line of the four that `S2` can carry out — the rebind, which is the
//! liveness claim.
//!
//! # What `6b` asks for that the vocabulary does not answer yet
//!
//! Checked, not assumed — [`the_session_is_typable_but_the_store_is_s5`] runs
//! each line and records what came back:
//!
//! | line | what happens today |
//! |---|---|
//! | `(unseen-regions "src/retry.rs")` | reaches the registered query; refused, naming `T041` |
//! | `(map region-author (block-regions …))` | `block-regions` is registered; **`region-author` is unbound** |
//! | `(keymap-set! "]r" (lambda () (goto (next-region-by claude))))` | `next-region-by` is registered; **`goto` and `claude` are unbound** (Steel names `claude`), and a lambda's free identifiers are resolved when it is *defined*, so the drawn body cannot be compiled |
//! | `(watch-place "src/retry.rs:24" 'delay)` | the alias resolves to `place-watch` and the string decodes — refused, naming `T077`. Until `§8` this was the one line that failed on *shape*: `anchor` is a `Target` and the drawing passed a string, so the answer was a `TypeMismatch`. The mockup was right and the vocabulary was wrong |
//!
//! `query.rs` already says `region-author` is *"an ordinary accessor over one of
//! those records, free and unregistered"* — so it and `goto` belong in
//! `runtime/`, over records the store returns. **Writing them now would mean
//! inventing the record shape `T041` owns**, so they are reported rather than
//! guessed, and this file is the record.
//!
//! # The colours
//!
//! They are in the tree (`Repl::rows`, `Tone`) and the interpreter paints them
//! (`T079`). The golden frame belongs to
//! `crates/phosphor-ui/tests/golden_frames.rs`, which `spine` may not edit this
//! window — see the report for `CP-2`.

use std::path::Path;
use std::sync::Arc;

use phosphor_core::input::key::parse_seq;
use phosphor_core::input::table::{Resolution, Scope};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::keymap::resolve;
use phosphor_steel::repl::{self, Repl};
use phosphor_steel::runtime::Runtime;

/// `6b`'s session, line by line, exactly as the mockup types it.
const TYPED: &[&str] = &[
    r#"(unseen-regions "src/retry.rs")"#,
    r#"(map region-author (block-regions "retry logic"))"#,
    r#"(keymap-set! "]r" (lambda () (goto (next-region-by claude))))"#,
    r#"(watch-place "src/retry.rs:24" 'delay)"#,
];

fn runtime() -> Runtime {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime");
    let host: Arc<dyn Host> = Arc::new(Detached);
    Runtime::boot(Some(&root), host)
}

fn session() -> (Repl, Runtime) {
    let mut runtime = runtime();
    let mut repl = Repl::new();
    for source in TYPED {
        for character in source.chars() {
            repl.insert(character);
        }
        repl.submit(&mut runtime).expect("a form was typed");
    }
    (repl, runtime)
}

#[test]
fn the_session_stacks_the_way_6b_draws_it() {
    let (repl, _runtime) = session();
    let lines = repl.lines();

    // Four entries of two rows, three blanks between them, and the live prompt
    // directly under the newest entry — `6b` has no blank before the cursor.
    assert_eq!(lines.len(), 4 * 2 + 3 + 1, "{lines:#?}");

    for (index, source) in TYPED.iter().enumerate() {
        let at = index * 3;
        assert_eq!(lines[at], format!("λ {source}"), "entry {index}");
        assert!(
            lines[at + 1].starts_with("⇒ "),
            "entry {index} answered {:?}",
            lines[at + 1]
        );
        if index + 1 < TYPED.len() {
            assert_eq!(lines[at + 2], "", "a blank row separates entries");
        }
    }

    let prompt = lines.last().expect("the live prompt is always drawn");
    assert_eq!(prompt, "λ  ", "an empty input line and the cursor");
}

#[test]
fn the_session_is_typable_but_the_store_is_s5() {
    // Every line of `6b` is *typable* — none of them ends the session, which is
    // the property that matters for a REPL. What each one answers is the module
    // table above, checked here so the table cannot rot.
    let (repl, _runtime) = session();
    let answers: Vec<String> = repl
        .entries()
        .iter()
        .map(|entry| entry.answered.line())
        .collect();

    // A registered query with no store: refused, naming the task that builds it.
    assert!(answers[0].contains("T041"), "{answers:#?}");
    // The accessor `query.rs` says is free and unregistered — and is not yet
    // written, because the record it accesses is `T041`'s.
    assert!(answers[1].contains("region-author"), "{answers:#?}");
    // Neither `goto` nor `claude` is bound — `goto-anchor`, `goto-sequence`
    // and `goto-location` are the registered names, and the actor identifiers
    // have no binding at all. Steel names the first it reaches.
    assert!(answers[2].contains("claude"), "{answers:#?}");
    // `6b` draws `(watch-place "src/retry.rs:24" …)`, and this line used to
    // assert the *shape gap*: the alias resolved, the row's `anchor` was a
    // `Target`, and a string could not be one — so the answer was Steel's
    // `TypeMismatch` naming `place-watch`, and `contains("place-watch")` was
    // reading an error message.
    //
    // `§8` ruled the mockup right and the vocabulary wrong. `path:line` is a
    // `Target` spelling now, so this line does what it is drawn doing: the
    // alias resolves, the string decodes, the call reaches the dispatcher, and
    // what comes back is the editor declining in its own voice rather than the
    // VM rejecting an argument.
    //
    // The assertion moved with it, and is stronger for it — naming the task is
    // something only a *decoded* call can do, where the old one passed on any
    // message that happened to mention the capability.
    assert!(answers[3].contains("T077"), "{answers:#?}");
    assert!(
        !answers[3].contains("TypeMismatch"),
        "the drawn line reaches the dispatcher now; a shape error means `§8` regressed: {answers:#?}"
    );
}

#[test]
fn the_rebind_is_live_on_the_very_next_key() {
    // `6b`'s third line is the one this phase owns, and its claim is not the
    // text of the answer — it is that the binding is in force immediately.
    // The drawn body cannot compile yet (see the module docs), so the body here
    // is one that exists; the *mechanism* under test is identical.
    let mut runtime = runtime();
    let mut repl = Repl::new();

    // `]` itself is a prefix in the shipped table — `]u` walks the unseen
    // regions (`6d`) — so the key this proves the rebind on is the whole
    // sequence `6b` types, which nothing binds until the form runs.
    assert_eq!(pressed(&mut runtime, "]r"), Resolution::Unbound);

    for character in r#"(keymap-set! "]r" (lambda () (open-repl!)))"#.chars() {
        repl.insert(character);
    }
    let entry = repl.submit(&mut runtime).expect("a form was typed");

    // `6b`: `⇒ #ok · persisted to init.scm`. The head is the rebind's; the note
    // is the persist step's receipt. `Detached` refuses `persist-form` — the
    // host that writes the file is the binary's (`crates/phosphor/src/main.rs`,
    // covered there against a scratch tree) — so what is proven here is that the
    // REPL *asked*, and said what happened either way.
    assert_eq!(entry.answered.head, "#ok");
    let note = entry.answered.note.as_deref().expect("the REPL persisted");
    assert!(
        note.contains("persisted to init.scm") || note.starts_with("not persisted —"),
        "the persist step ran and said what happened: {note:?}"
    );

    // No reload, no second boot, nothing invalidated.
    assert_eq!(pressed(&mut runtime, "]"), Resolution::Pending);
    assert_eq!(pressed(&mut runtime, "]r"), Resolution::Ran);
}

/// One question for the live keymap, spelled the way a keymap is written.
///
/// `T033` made the layer stateless and the whole sequence the unit of a
/// lookup — the machine holds the unfinished keys and hands them over, so a
/// second copy on the scheme side could only disagree with it.
fn pressed(runtime: &mut Runtime, keys: &str) -> Resolution {
    let keys = parse_seq(keys).expect("a spelling this test wrote");
    resolve(runtime, Scope::Normal, &keys)
}

#[test]
fn the_frame_carries_the_header_and_the_statusline_6b_draws() {
    let (repl, _runtime) = session();
    let tree = repl.frame();

    // `◆ steel · :repl` — §4's *"source or command · meta right"*.
    assert_eq!(
        format!("{}{}", repl::HEADER, repl::HEADER_META),
        "◆ steel · :repl"
    );
    // `REPL` on the steel field, `steel` where a file would be, and the three
    // keys the footer teaches.
    assert_eq!(repl::CHIP, "REPL");
    assert_eq!(repl::SURFACE, "steel");
    let hints: Vec<String> = repl::hints()
        .iter()
        .map(|hint| format!("{} {}", hint.key.0, hint.verb))
        .collect();
    assert_eq!(hints.join(" · "), "C-c buffer · tab complete · q close");

    assert!(
        tree.float.is_none(),
        "the REPL takes the frame; §8 puts a float at 60-80% of width and `6b` \
         draws it edge to edge with its own chip"
    );
}
