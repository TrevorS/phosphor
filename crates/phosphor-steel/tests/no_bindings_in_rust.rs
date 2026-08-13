//! `T033`'s acceptance, as a test rather than a sentence: **every binding lives
//! in `runtime/`, none in Rust.**
//!
//! The task's *done when* is one clause long and it is a claim about the whole
//! tree, which is exactly the kind of claim this repo has learned not to take
//! on trust — a `VENDOR.md` once described a licence crisis that three files in
//! the same directory disproved, and every gate passed it because nothing
//! checked prose against reality. So this reads the tree.
//!
//! Four things, each of which fails on a different way of getting it wrong:
//!
//! 1. **Nothing in Rust binds a key.** `Table::bind` is the only way an entry
//!    reaches a keymap, so a `.bind(` outside test code is a binding in Rust.
//! 2. **Every shipped binding resolves.** A role is a list the layer writes and
//!    `phosphor-steel`'s decoder reads; a typo in either makes the key silently
//!    dead. Walking the table through the real decoder is what makes that loud.
//! 3. **The two spellings of a key sequence agree.** `runtime/keymaps.scm`
//!    canonicalises what a person writes and `Key::notation` spells what a
//!    terminal produced. Every key the decoder can produce is put through the
//!    layer's canonicaliser and has to come back unchanged, and every row of
//!    the shipped table has to already be in that form — otherwise a binding is
//!    written that no keystroke can reach, and nothing else would notice.
//!    Driven from the keys, that check only ever sees the canonical spelling,
//!    so a fourth drives it from the **spellings a person writes**: `<C-K>`,
//!    `<S-C-k>` and `<c-k>` are one chord, and the fold onto the one the
//!    machine asks with is what makes all three reachable.
//! 4. **Every ex command names a capability that exists.** `:write` that
//!    decodes to nothing is worse than no `:write`.
//!
//! # What this does *not* claim
//!
//! Not that no Rust implements [`Keymap`](phosphor_core::input::table::Keymap):
//! three do, and they are structure rather than content — a table, the layering
//! rule, and the adapter over the VM. What none of them may contain is an
//! entry.
//!
//! # The exception that is gone
//!
//! This file used to exempt `crates/phosphor-core/src/input/vim.rs` by name —
//! `T026`'s seed table, transcribed into `runtime/keymaps.scm` and unwired, but
//! not deleted, because deleting it was an edit `T033` did not own. It is
//! deleted now, so the exemption is gone with it and the scan is stricter by
//! subtraction: there is no path in the tree that may bind a key in Rust.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_core::input::key::{Code, Key, Mods, Named, notation_of, parse_seq};
use phosphor_core::input::table::{Resolution, Scope};
use phosphor_core::request::KeySeq;
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::keymap::{self, Ex};
use phosphor_steel::runtime::Runtime;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the checkout")
}

fn runtime() -> Runtime {
    let host: Arc<dyn Host> = Arc::new(Detached);
    Runtime::boot(Some(&repo().join("runtime")), host)
}

/// Every `.rs` file under a workspace crate's `src/`, sorted.
///
/// `tests/` is left out on purpose: an integration test is test code by
/// definition, and `vendor/` is not ours.
fn sources() -> Vec<PathBuf> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                into.push(path);
            }
        }
    }

    let mut files = Vec::new();
    for crate_dir in std::fs::read_dir(repo().join("crates"))
        .expect("crates/ is part of the repo")
        .filter_map(Result::ok)
    {
        walk(&crate_dir.path().join("src"), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "no sources found — wrong path?");
    files
}

/// A file's non-test source, as `(line number, text)`.
///
/// Every crate in this workspace puts its unit tests in one `#[cfg(test)] mod
/// tests { … }` at the end of the file, so "before the first column-0
/// `#[cfg(test)]`" is the whole of the non-test source. That convention is
/// **checked rather than assumed**: a second one, or one that is not a test
/// module, fails here rather than quietly shrinking what is scanned.
fn outside_tests(path: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).expect("a readable source file");
    let lines: Vec<&str> = text.lines().collect();
    let marks: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "#[cfg(test)]")
        .map(|(at, _)| at)
        .collect();
    assert!(
        marks.len() <= 1,
        "{}: more than one top-level #[cfg(test)] — this scan assumes one test \
         module per file, so a second one would hide source from it",
        path.display()
    );
    if let Some(at) = marks.first() {
        assert!(
            lines.get(at + 1).is_some_and(|next| *next == "mod tests {"),
            "{}:{}: a top-level #[cfg(test)] that is not a test module",
            path.display(),
            at + 1
        );
    }
    let end = marks.first().copied().unwrap_or(lines.len());
    lines[..end]
        .iter()
        .enumerate()
        .map(|(at, line)| (at + 1, (*line).to_owned()))
        .collect()
}

/// **The acceptance criterion.** No Rust outside a test binds a key.
#[test]
fn no_rust_source_binds_a_key() {
    let repo = repo();
    let mut offences = Vec::new();

    for path in sources() {
        let relative = path
            .strip_prefix(&repo)
            .unwrap_or(&path)
            .display()
            .to_string();

        for (line, text) in outside_tests(&path) {
            let trimmed = text.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if !(text.contains(".bind(") || text.contains(".unbind(")) {
                continue;
            }
            offences.push(format!("{relative}:{line}: {}", trimmed.trim_end()));
        }
    }

    assert!(
        offences.is_empty(),
        "a keymap entry was added in Rust. Every binding lives in runtime/ \
         (T033); move it to runtime/keymaps.scm, where a REPL rebind can reach \
         it and which-key can read it.\n{}",
        offences.join("\n")
    );
}

/// Every shipped binding resolves to something the machine can act on.
///
/// A role that the decoder cannot read answers [`Resolution::Unbound`], which
/// on a real keyboard is a key that does nothing and says nothing. This is the
/// only place the whole table is put through the real decoder.
///
/// It *runs* a thunk binding, by construction — resolving one is running it.
/// The shipped table has none today, and a layer that adds one is asking for it
/// to be run when the key is pressed anyway.
#[test]
fn every_shipped_binding_resolves() {
    let mut runtime = runtime();
    let entries = keymap::entries(&mut runtime);
    assert!(
        entries.len() > 100,
        "the shipped keymap is {} entries — the transcription is not complete",
        entries.len()
    );

    let mut dead = Vec::new();
    for entry in &entries {
        let Some(scope) = scope_named(&entry.scope) else {
            dead.push(format!("{} — no such scope", entry.scope));
            continue;
        };
        if keymap::resolve_seq(&mut runtime, scope, &entry.keys) == Resolution::Unbound {
            dead.push(format!(
                "{} {} ({}) — the role does not decode",
                entry.scope, entry.keys.0, entry.verb
            ));
        }
    }
    assert!(dead.is_empty(), "{}", dead.join("\n"));
}

/// The scope a name spells, or [`None`].
fn scope_named(name: &str) -> Option<Scope> {
    let scope = match name {
        "normal" => Scope::Normal,
        "insert" => Scope::Insert,
        "visual" => Scope::Visual,
        "operator-pending" => Scope::OperatorPending,
        "object" => Scope::Object,
        _ => return None,
    };
    Some(scope)
}

/// The layer's canonical spelling and `Key::notation` are the same spelling.
///
/// Two parsers: `phosphor/keys` turns what a person wrote into the form the
/// table is keyed by, and `Key::notation` turns a keystroke into the form the
/// machine asks with. If they drift, a binding is written that no keystroke can
/// ever reach, and nothing else would notice — the key would simply do nothing.
///
/// **Driven from the keys, not from a spelling.** Every key the decoder can
/// produce is spelled and handed to the layer, and the layer must hand it back
/// unchanged. That is the whole reachability property, and it does not go
/// through `key::parse_seq` — which cannot spell a bare `<` at all (see this
/// task's report), so a check written the other way round would be checking the
/// weaker of the two parsers.
#[test]
fn the_layer_spells_every_key_the_machine_can_produce() {
    let mut runtime = runtime();
    let mut keys: Vec<Key> = (0x20_u8..0x7f)
        .map(|byte| Key::char(char::from(byte)))
        .collect();
    for named in [
        Named::Esc,
        Named::Enter,
        Named::Tab,
        Named::Backspace,
        Named::Delete,
        Named::Insert,
        Named::Left,
        Named::Right,
        Named::Up,
        Named::Down,
        Named::Home,
        Named::End,
        Named::PageUp,
        Named::PageDown,
        Named::Function(1),
        Named::Function(12),
    ] {
        keys.push(Key::named(named));
    }
    for mods in [Mods::CTRL, Mods::ALT, Mods::CTRL.with(Mods::SHIFT)] {
        keys.push(Key::new(Code::Char('k'), mods));
        keys.push(Key::new(Code::Named(Named::Left), mods));
    }

    for key in keys {
        let spelled = key.notation();
        assert_eq!(
            keymap::canonical(&mut runtime, &spelled),
            Some(KeySeq(spelled.clone())),
            "the layer does not spell {spelled:?} the way the machine does, so \
             a binding on it could never be reached"
        );
    }
}

/// Every shipped binding is already in canonical form.
///
/// Together with the test above this is reachability: the layer answers to
/// exactly what the machine asks with, and every row in the table is spelled
/// that way.
#[test]
fn every_shipped_binding_is_spelled_the_way_a_keystroke_arrives() {
    let mut runtime = runtime();
    for entry in keymap::entries(&mut runtime) {
        assert_eq!(
            keymap::canonical(&mut runtime, &entry.keys.0).as_ref(),
            Some(&entry.keys),
            "{} is not a fixed point of the layer's own canonicaliser",
            entry.keys.0
        );
    }
}

/// **R12.** Every spelling of one chord folds onto the one the machine asks
/// with.
///
/// The test above drives from the keys the decoder produces, so it can only
/// ever see the *canonical* spelling — which is why it could not catch this:
/// the layer copied a bracketed key verbatim, so `<C-K>`, `<S-C-k>` and
/// `<c-k>` were three bindings, and `T027` made the machine always ask with
/// `<C-S-k>`. Three keys nothing could ever press, and nothing red.
///
/// So this drives from the *spellings a person writes*. The right-hand side is
/// never a string this test invented: it is a [`Key`] spelled by the machine's
/// own [`notation_of`], which is the exact text `keymap::resolve` asks with.
#[test]
fn every_spelling_of_a_chord_folds_onto_the_one_the_machine_asks_with() {
    let mut runtime = runtime();

    let equivalents: &[(&[&str], Key)] = &[
        // `T027`'s chord, in the five ways it gets written.
        (
            &["<C-S-k>", "<C-K>", "<S-C-k>", "<c-s-k>", "<c-K>"],
            Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT)),
        ),
        // …and the one it must stay distinguishable from.
        (&["<C-k>", "<c-k>"], Key::new(Code::Char('k'), Mods::CTRL)),
        // Order, on a key that was never a character. `M-` is vim's other
        // spelling of alt.
        (
            &["<A-S-left>", "<S-A-left>", "<M-S-left>"],
            Key::new(Code::Named(Named::Left), Mods::ALT.with(Mods::SHIFT)),
        ),
        // A named key's aliases and its case.
        (
            &["<esc>", "<Esc>", "<ESC>", "<escape>"],
            Key::named(Named::Esc),
        ),
        (
            &["<cr>", "<CR>", "<enter>", "<return>"],
            Key::named(Named::Enter),
        ),
        (&["<f5>", "<F5>"], Key::named(Named::Function(5))),
        // Shift folds into a plain character, so neither of these is bracketed
        // once it has been read.
        (&["<S-a>", "<s-a>", "a"], Key::char('a')),
        (&["<S-A>", "A"], Key::char('A')),
        (&["<lt>", "<LT>"], Key::char('<')),
        (&["<w>", "w"], Key::char('w')),
        // The leader, both ways `3c` and the machine spell it.
        (&["SPC", "<space>", "<Space>"], Key::char(' ')),
    ];

    for (spellings, key) in equivalents {
        let asked = notation_of(&[*key]);
        for spelled in *spellings {
            assert_eq!(
                keymap::canonical(&mut runtime, spelled).as_ref(),
                Some(&asked),
                "the layer spells {spelled:?} differently from the machine, so \
                 a binding written that way could never be reached"
            );
        }
    }

    // A bracketed word that names no key is **not** folded, and that is right
    // rather than a gap: rust reads `<nope>` as the six characters it is, so
    // verbatim already is the spelling the machine asks with.
    let literal = parse_seq("<nope>").expect("a spelling this test wrote");
    assert_eq!(literal.len(), 6);
    assert_eq!(
        keymap::canonical(&mut runtime, "<nope>"),
        Some(notation_of(&literal))
    );
}

/// A rebind written the way a person writes it reaches the key they pressed.
///
/// The consequence of the fold, at the REPL — which is where a spelling that
/// is not the canonical one actually gets typed (`6b`).
#[test]
fn a_rebind_written_in_any_spelling_is_reachable() {
    let mut runtime = runtime();
    let chord = Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT));
    let asked = KeySeq(notation_of(&[chord]).0);

    assert_eq!(
        keymap::resolve_seq(&mut runtime, Scope::Normal, &asked),
        Resolution::Unbound
    );
    let outcome = runtime.evaluate(r#"(keymap-set! "<C-K>" (key/motion "line-down") "down")"#);
    assert!(
        matches!(outcome, phosphor_core::action::Outcome::Done(_)),
        "{outcome:?}"
    );
    assert!(
        matches!(
            keymap::resolve_seq(&mut runtime, Scope::Normal, &asked),
            Resolution::Role(_)
        ),
        "a binding written `<C-K>` answers the `<C-S-k>` the machine asks with"
    );
}

/// Every ex command names a capability that exists, with arguments it accepts.
#[test]
fn every_ex_command_decodes() {
    let mut runtime = runtime();
    let commands = keymap::ex_entries(&mut runtime);
    assert!(
        commands.len() > 5,
        "the ex table is {} deep",
        commands.len()
    );

    for command in &commands {
        // The argument is what a person would type after the name; an empty
        // one is the shape every command has to survive.
        assert_ne!(
            keymap::ex(&mut runtime, &command.name),
            Ex::Unknown,
            ":{} is listed and does not run",
            command.name
        );
        // Design Language §6: a command is displayed whole. Abbreviation is
        // about the keyboard, and this is the label.
        assert!(
            !command.name.starts_with(':'),
            ":{} carries its own colon — the prompt draws that",
            command.name
        );
    }
}

/// Screen `3c`: the leader popup's six rows, from the live table.
///
/// The mockup is the spec for what the surface *says*, and the data behind it
/// is this task's. `T034` draws these; what is asserted here is that they are
/// there to draw, in the order `3c` draws them.
#[test]
fn the_leader_tree_is_the_six_rows_3c_draws() {
    let mut runtime = runtime();
    let rows: Vec<String> = keymap::entries(&mut runtime)
        .into_iter()
        .filter(|entry| {
            entry.scope == "normal"
                && entry.keys.0.starts_with("<space>")
                && entry.keys.0.len() == "<space>".len() + 1
        })
        .map(|entry| entry.keys.0)
        .collect();
    assert_eq!(
        rows,
        vec![
            "<space>c", "<space>u", "<space>t", "<space>r", "<space>j", "<space>f",
        ],
        "3c draws c, u, t, r, j, f in that order"
    );

    // A group waits for its leaf; a leaf does not.
    assert_eq!(
        keymap::resolve_seq(&mut runtime, Scope::Normal, &KeySeq("SPC c".to_owned())),
        Resolution::Pending
    );
}
