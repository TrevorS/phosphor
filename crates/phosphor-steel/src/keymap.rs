//! The keymap, asked rather than cached — `T022`'s liveness claim, as a
//! function, and `T033`'s *"every binding lives in `runtime/`"* as a seam.
//!
//! > *"a keybinding redefined at the REPL takes effect on the next frame
//! > without restart"* — `IMPLEMENTATION-PLAN.md`, `S2` acceptance
//!
//! The table is `runtime/keymaps.scm`'s and **there is no copy of it on this
//! side**. [`resolve`] hands the VM a scope and a key sequence in vim notation
//! and reads back what the editor layer says the sequence plays in the grammar.
//! That is the whole mechanism, and it is why the claim needs no invalidation,
//! no reload and no cache-coherence rule: a `(keymap-set! …)` typed at `:repl`
//! mutated the only table there is, one keystroke ago.
//!
//! # Two things a binding can be, and why the difference is load-bearing
//!
//! A scheme binding is either a **role** — plain data naming one arm of
//! [`Role`] — or a **thunk**. The role is what crosses; a closure is what does
//! not, and `request::Binding`'s own header records why. So:
//!
//! * A role comes back as [`Resolution::Role`] and the machine composes with
//!   it. `w` is a motion, so `dw` deletes a word and `2dw` deletes two — the
//!   grammar is the machine's and the vocabulary is the layer's.
//! * A thunk is **run inside the VM** and comes back as [`Resolution::Ran`].
//!   The host learns that arbitrary scheme ran and nothing about what it was,
//!   which is what keeps a `SteelVal` out of the input path.
//!
//! # Stateless, deliberately
//!
//! `T022`'s dispatcher kept its own half-typed sequence, and `T026` gave the
//! machine one too. Two are one too many: the machine already holds the
//! unfinished sequence and hands the whole of it to
//! [`Keymap::resolve`](phosphor_core::input::table::Keymap::resolve), so the
//! layer is asked a complete question every time and has nothing to remember.
//! The `phosphor/press-reset` that kept the two in step is gone with the second
//! copy.
//!
//! # The read side — `T034` and `T086`'s contract
//!
//! [`entries`] is the same table at the other end: every binding, in reading
//! order, with the key sequence and the verb spelled out in full. `T034`'s
//! `KeymapFooter`, the `SPC` grid (`3c`) and `T086`'s `HelpGrid` (`6d`) are
//! `view::Density`'s three arms over exactly this list, so a REPL rebind
//! appears in all three with no extra wiring — the list is read, never cached.
//! [`Entry::hint`] is the projection onto [`KeyHint`], which is the two fields
//! a keymap surface actually draws.
//!
//! # Degradation
//!
//! A runtime tree with no `keymaps.scm` — or one whose forms failed — has no
//! `phosphor/resolve`, so the call raises and every key answers
//! [`Resolution::Unbound`]. The editor is then exactly the editor it was before
//! this module existed rather than one that eats keystrokes, which is the same
//! promise `T021` makes about a broken `init.scm`.
//!
//! Owned by `spine`.

use phosphor_core::input::key::{Key, notation_of};
use phosphor_core::input::table::{self, Goto, Operator, Resolution, Role, Scope};
use phosphor_core::request::{KeySeq, ScrollRequest, SelectionKind, TextObject};
use phosphor_core::value::{Args, Value, Wire};
use phosphor_core::view::KeyHint;
use steel::SteelVal;

use crate::convert::from_steel;
use crate::runtime::Runtime;

/// The editor layer's dispatcher: a scope and a sequence in, a role out.
pub const RESOLVE: &str = "phosphor/resolve";

/// The read side every keymap surface draws from.
pub const ENTRIES: &str = "keymap-entries";

/// The ex-command dispatcher — `:write`, and every unique prefix of it.
pub const EX: &str = "phosphor/ex";

/// The ex table, for `:help`.
pub const EX_ENTRIES: &str = "ex-entries";

/// The layer's canonicaliser — `SPC f` and `<space>f` are one binding.
pub const CANONICAL: &str = "phosphor/keys";

/// One binding, as a keymap surface reads it.
///
/// The shape is `runtime/keymaps.scm`'s own header, and it is deliberately
/// four fields rather than a [`Role`]: a surface draws what a key *does*, and
/// the grammar arm it plays is the machine's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Which scope it is bound in — [`Scope::name`]'s spelling.
    pub scope: String,
    /// The sequence, in canonical vim notation.
    pub keys: KeySeq,
    /// What it does, spelled out in full. Design Language §6: a hint says
    /// `:reattach`, never `:ca`.
    pub verb: String,
    /// Whether something longer is bound under these keys — `SPC c` is a group
    /// and `SPC c p` is a leaf. Derived by the layer, never stored, so a group
    /// cannot claim children it does not have.
    pub group: bool,
    /// The role, when the binding is data rather than a thunk. [`None`] for a
    /// thunk, which has nothing a surface could name.
    pub role: Option<Role>,
}

impl Entry {
    /// This binding as the two fields a keymap surface draws.
    #[must_use]
    pub fn hint(&self) -> KeyHint {
        KeyHint {
            key: self.keys.clone(),
            verb: self.verb.clone(),
        }
    }
}

/// One ex command, as `:help` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExCommand {
    /// The whole name — what is displayed, always. Design Language §6:
    /// `:reattach`, never `:ca`.
    pub name: String,
    /// How few characters of it may be **typed**. `write` answers 1, so `:w`
    /// names it; `wq` answers 2, so it has no short form at all. A surface that
    /// wants to draw `w[rite]` has what it needs; one that only draws the name
    /// does not have to know the rule exists.
    pub shortest: usize,
    /// What it does.
    pub verb: String,
}

/// What the layer made of an ex line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ex {
    /// The command named these Actions; the caller applies them.
    Run(Vec<phosphor_core::action::Action>),
    /// The command did the work itself, in scheme.
    Ran,
    /// Several commands start with what was typed and none is shorter.
    Ambiguous,
    /// Nothing starts with it.
    Unknown,
}

/// Asks the live keymap what a sequence plays in `scope`.
///
/// `keys` is the whole unfinished sequence, as the machine holds it; the layer
/// keeps no copy of its own. A thunk binding is **run here**, inside this call,
/// and answers [`Resolution::Ran`].
pub fn resolve(runtime: &mut Runtime, scope: Scope, keys: &[Key]) -> Resolution {
    resolve_seq(runtime, scope, &notation_of(keys))
}

/// The same question, asked with the sequence already spelled.
///
/// The door-facing half: `feed-keys` and `set-keybinding` both name a key as
/// [`KeySeq`] text rather than as keys, and `runtime/keymaps.scm` canonicalises
/// what it is given either way. [`resolve`] is this with the spelling done for
/// it, and is what the input loop calls.
pub fn resolve_seq(runtime: &mut Runtime, scope: Scope, keys: &KeySeq) -> Resolution {
    let args = vec![
        SteelVal::StringV(scope.name().into()),
        SteelVal::StringV(keys.0.as_str().into()),
    ];
    let Ok(answered) = runtime.call(RESOLVE, args) else {
        // No dispatcher, or one that raised. Either way the key is the host's.
        return Resolution::Unbound;
    };
    let Ok(value) = from_steel(&answered) else {
        return Resolution::Unbound;
    };
    resolution(&value)
}

/// The layer's canonical spelling of a key sequence.
///
/// `SPC f` and `<space>f` are one binding, and this is the function that makes
/// them one. Exposed because it is the only way to check, from Rust, that the
/// layer's spelling of a key and [`notation_of`]'s are the same spelling — two
/// parsers that drift leave bindings no keystroke can reach.
pub fn canonical(runtime: &mut Runtime, spelled: &str) -> Option<KeySeq> {
    let args = vec![SteelVal::StringV(spelled.into())];
    let answered = runtime.call(CANONICAL, args).ok()?;
    match from_steel(&answered).ok()? {
        Value::Text(canonical) => Some(KeySeq(canonical)),
        _ => None,
    }
}

/// Every binding, in reading order.
///
/// Empty for a layer with no table — a keymap surface then draws nothing, which
/// is the honest rendering of "nothing is bound".
pub fn entries(runtime: &mut Runtime) -> Vec<Entry> {
    let Ok(answered) = runtime.call(ENTRIES, Vec::new()) else {
        return Vec::new();
    };
    let Ok(Value::List(rows)) = from_steel(&answered) else {
        return Vec::new();
    };
    rows.iter().filter_map(entry).collect()
}

/// Every ex command, in reading order.
pub fn ex_entries(runtime: &mut Runtime) -> Vec<ExCommand> {
    let Ok(answered) = runtime.call(EX_ENTRIES, Vec::new()) else {
        return Vec::new();
    };
    let Ok(Value::List(rows)) = from_steel(&answered) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let row = record(row)?;
            let name = text(&row, "name")?;
            let shortest = match row.get("shortest") {
                Some(Value::Int(least)) => usize::try_from(*least).unwrap_or(name.len()),
                _ => name.len(),
            };
            Some(ExCommand {
                name,
                shortest,
                verb: text(&row, "verb").unwrap_or_default(),
            })
        })
        .collect()
}

/// Runs one ex line — the text after the `:`, without it.
///
/// Abbreviation is the layer's rule and lives with the table (`keymaps.scm`):
/// an exact name wins, otherwise the shortest command the text is a prefix of,
/// otherwise it is [`Ex::Ambiguous`]. Nothing on this side knows a command name.
pub fn ex(runtime: &mut Runtime, line: &str) -> Ex {
    let args = vec![SteelVal::StringV(line.into())];
    let Ok(answered) = runtime.call(EX, args) else {
        return Ex::Unknown;
    };
    let Ok(value) = from_steel(&answered) else {
        return Ex::Unknown;
    };
    match &value {
        Value::Text(word) if word == "ran" => Ex::Ran,
        Value::Text(word) if word == "ambiguous" => Ex::Ambiguous,
        Value::List(_) => match role(&value) {
            Some(Role::Run(actions)) => Ex::Run(actions),
            // A command that answered some other role has nothing to apply on
            // its own: a role is composed with a count and an operator, and an
            // ex line has neither.
            _ => Ex::Unknown,
        },
        _ => Ex::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// The three words and one list [`RESOLVE`] can answer.
fn resolution(value: &Value) -> Resolution {
    match value {
        Value::Text(word) if word == "ran" => Resolution::Ran,
        Value::Text(word) if word == "pending" => Resolution::Pending,
        Value::List(_) => role(value).map_or(Resolution::Unbound, Resolution::Role),
        _ => Resolution::Unbound,
    }
}

/// One row of [`ENTRIES`].
fn entry(row: &Value) -> Option<Entry> {
    let row = record(row)?;
    Some(Entry {
        scope: text(&row, "scope")?,
        keys: KeySeq(text(&row, "keys")?),
        verb: text(&row, "verb").unwrap_or_default(),
        group: matches!(row.get("group"), Some(Value::Bool(true))),
        role: row.get("role").and_then(role),
    })
}

/// A role descriptor: a list whose head names the arm.
///
/// Total and forgiving — an unknown head, a missing argument or a payload the
/// wire model refuses answers [`None`], and the binding is then unbound rather
/// than wrong. `no_bindings_in_rust.rs` walks the shipped table through here,
/// so a typo in `keymaps.scm` is a failing test rather than a dead key.
fn role(value: &Value) -> Option<Role> {
    let Value::List(items) = value else {
        return None;
    };
    let head = match items.first()? {
        Value::Text(head) => head.as_str(),
        _ => return None,
    };
    let rest = &items[1..];
    let built = match head {
        "motion" => Role::Motion(choice(rest.first()?)?),
        "goto" => Role::Goto(match word(rest.first()?)? {
            "first" => Goto::First,
            "last" => Goto::Last,
            _ => return None,
        }),
        "operator" => Role::Operator(operator(rest.first()?)?),
        "fused" => Role::Fused {
            operator: operator(rest.first()?)?,
            motion: choice(rest.get(1)?)?,
        },
        "object" => Role::Object {
            object: choice::<TextObject>(rest.first()?)?,
            delimiter: rest.get(1).and_then(word).and_then(|d| d.chars().next()),
        },
        "inner" => Role::Inner,
        "around" => Role::Around,
        "enter" => Role::Enter(match word(rest.first()?)? {
            "before" => table::Entry::Before,
            "after" => table::Entry::After,
            "line-start" => table::Entry::LineStart,
            "line-end" => table::Entry::LineEnd,
            "open-below" => table::Entry::OpenBelow,
            "open-above" => table::Entry::OpenAbove,
            "replace" => table::Entry::Replace,
            _ => return None,
        }),
        "select" => Role::Select(choice::<SelectionKind>(rest.first()?)?),
        "paste" => Role::Paste {
            before: flag(rest.first()?),
        },
        "history" => Role::History {
            redo: flag(rest.first()?),
        },
        "scroll" => Role::Scroll(ScrollRequest::from_value(rest.first()?).ok()?),
        "repeat" => Role::Repeat,
        "escape" => Role::Escape,
        "register" => Role::Register,
        "run" => Role::Run(rest.iter().map(call).collect::<Option<Vec<_>>>()?),
        _ => return None,
    };
    Some(built)
}

/// One capability call: `("quit" #hash(("force" . #true)))`.
fn call(value: &Value) -> Option<phosphor_core::action::Action> {
    let Value::List(parts) = value else {
        return None;
    };
    let name = word(parts.first()?)?;
    let args = match parts.get(1) {
        Some(value) => record(value)?,
        None => Args::new(),
    };
    phosphor_core::action::Action::from_call(name, &args).ok()
}

/// An operator name, which is this module's vocabulary rather than the wire's —
/// [`Operator`] is a grammar role and never rides in a payload.
fn operator(value: &Value) -> Option<Operator> {
    let operator = match word(value)? {
        "delete" => Operator::Delete,
        "change" => Operator::Change,
        "yank" => Operator::Yank,
        "indent" => Operator::Indent,
        "dedent" => Operator::Dedent,
        "toggle-comment" => Operator::ToggleComment,
        _ => return None,
    };
    Some(operator)
}

/// A payload type spelled as its wire tag — `"word-forward"`.
fn choice<T: Wire>(value: &Value) -> Option<T> {
    T::from_value(value).ok()
}

fn word(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text.as_str()),
        _ => None,
    }
}

fn text(row: &Args, name: &str) -> Option<String> {
    match row.get(name) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn record(value: &Value) -> Option<Args> {
    match value {
        Value::Record(args) => Some(args.clone()),
        _ => None,
    }
}

/// Scheme's `#false` and `void` both mean "no"; anything else means yes.
fn flag(value: &Value) -> bool {
    matches!(value, Value::Bool(true))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::*;
    use crate::host::{Detached, Host};
    use phosphor_core::input::key::parse_seq;
    use phosphor_core::request::Motion;

    fn tree() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime")
    }

    fn runtime() -> Runtime {
        let host: Arc<dyn Host> = Arc::new(Detached);
        Runtime::boot(Some(&tree()), host)
    }

    fn keys(spelled: &str) -> Vec<Key> {
        parse_seq(spelled).expect("a spelling these tests wrote")
    }

    fn ask(runtime: &mut Runtime, scope: Scope, spelled: &str) -> Resolution {
        resolve(runtime, scope, &keys(spelled))
    }

    #[test]
    fn the_shipped_layer_answers_roles_not_only_thunks() {
        let mut runtime = runtime();
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "w"),
            Resolution::Role(Role::Motion(Motion::WordForward))
        );
        assert_eq!(
            ask(&mut runtime, Scope::OperatorPending, "d"),
            Resolution::Role(Role::Operator(Operator::Delete))
        );
        assert_eq!(
            ask(&mut runtime, Scope::Object, "u"),
            Resolution::Role(Role::Object {
                object: TextObject::UnseenRegion,
                delimiter: None,
            }),
            "6d's agent nouns are bound here, and resolve at T049"
        );
    }

    #[test]
    fn the_leader_is_pending_until_it_names_something() {
        let mut runtime = runtime();
        assert_eq!(ask(&mut runtime, Scope::Normal, "SPC"), Resolution::Pending);
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "SPC c"),
            Resolution::Pending,
            "3c draws +claude as a group, so SPC c has to wait for its leaf"
        );
        assert!(matches!(
            ask(&mut runtime, Scope::Normal, "SPC c i"),
            Resolution::Role(Role::Run(_))
        ));
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "SPC z"),
            Resolution::Unbound
        );
    }

    #[test]
    fn spc_and_a_space_are_one_binding() {
        // `Key::notation` spells the leader `<space>` and `3c` spells it `SPC`.
        // The layer canonicalises, so the table can be written either way and a
        // keystroke finds it.
        let mut runtime = runtime();
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "<space>c i"),
            ask(&mut runtime, Scope::Normal, "SPC c i")
        );
    }

    #[test]
    fn a_thunk_runs_in_the_vm_and_answers_ran() {
        let mut runtime = runtime();
        assert_eq!(ask(&mut runtime, Scope::Normal, "g"), Resolution::Pending);
        let outcome = runtime.evaluate(r#"(keymap-set! "gz" (lambda () 1) "count to one")"#);
        assert!(matches!(outcome, phosphor_core::action::Outcome::Done(_)));
        assert_eq!(ask(&mut runtime, Scope::Normal, "gz"), Resolution::Ran);
    }

    #[test]
    fn a_rebind_is_in_force_on_the_very_next_key() {
        // `T022`'s claim, at the level this crate can hold it: no reload, no
        // second boot, no invalidation — the next press sees it.
        let mut runtime = runtime();
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "w"),
            Resolution::Role(Role::Motion(Motion::WordForward))
        );
        let outcome = runtime.evaluate(r#"(keymap-set! "w" (key/motion "line-down") "down")"#);
        assert!(matches!(outcome, phosphor_core::action::Outcome::Done(_)));
        assert_eq!(
            ask(&mut runtime, Scope::Normal, "w"),
            Resolution::Role(Role::Motion(Motion::LineDown))
        );
        let _ = runtime.evaluate(r#"(keymap-remove! "w")"#);
        assert_eq!(ask(&mut runtime, Scope::Normal, "w"), Resolution::Unbound);
    }

    #[test]
    fn a_layer_without_a_table_leaves_every_key_to_the_host() {
        let host: Arc<dyn Host> = Arc::new(Detached);
        let mut runtime = Runtime::boot(None, host);
        assert_eq!(ask(&mut runtime, Scope::Normal, "w"), Resolution::Unbound);
        assert!(entries(&mut runtime).is_empty());
        assert_eq!(ex(&mut runtime, "write"), Ex::Unknown);
    }

    #[test]
    fn the_read_side_carries_the_leader_grid() {
        let mut runtime = runtime();
        let entries = entries(&mut runtime);
        let leader = entries
            .iter()
            .find(|entry| entry.keys.0 == "<space>c")
            .expect("3c's +claude group");
        assert!(leader.group, "something longer is bound under SPC c");
        let leaf = entries
            .iter()
            .find(|entry| entry.keys.0 == "<space>ci")
            .expect("3c's interrupt leaf");
        assert!(!leaf.group);
        // Design Language §6: a keyhint spells the whole command.
        assert!(leaf.hint().verb.starts_with(":interrupt"));
    }

    #[test]
    fn an_ex_command_is_typed_the_way_vim_abbreviates_one() {
        let mut runtime = runtime();
        // `w[rite]`: every prefix from the first character on names it, and
        // `wall` does not get in the way because it declares two.
        for typed in ["w", "wr", "wri", "writ", "write"] {
            assert_eq!(
                ex(&mut runtime, typed),
                ex(&mut runtime, "write"),
                "{typed}"
            );
        }
        assert_eq!(ex(&mut runtime, "wa"), ex(&mut runtime, "wall"));
        assert_eq!(ex(&mut runtime, "nosuchthing"), Ex::Unknown);
        // `t` is three commands deep, and none of them answers to one letter.
        assert_eq!(ex(&mut runtime, "t"), Ex::Unknown);
        assert_ne!(ex(&mut runtime, "th"), Ex::Unknown);
        // `:repl` does its own work through the Steel door.
        assert_eq!(ex(&mut runtime, "repl"), Ex::Ran);
    }

    #[test]
    fn a_bang_is_a_flag_rather_than_part_of_the_name() {
        // `:q` refuses on unsaved work and `:q!` does not, and the difference
        // is one argument to one capability — not two commands.
        let mut runtime = runtime();
        assert_ne!(ex(&mut runtime, "q"), Ex::Unknown);
        assert_ne!(
            ex(&mut runtime, "q!"),
            ex(&mut runtime, "q"),
            "the bang has to reach the Action, or `:q!` is a lie"
        );
        assert_ne!(ex(&mut runtime, "q!"), Ex::Unknown);
    }

    #[test]
    fn every_ex_command_is_listed_by_its_whole_name() {
        let mut runtime = runtime();
        let listed = ex_entries(&mut runtime);
        let write = listed
            .iter()
            .find(|command| command.name == "write")
            .expect(":write ships");
        assert_eq!(write.shortest, 1, "`w[rite]` may be typed as one letter");
        assert!(
            listed.iter().all(|command| !command.verb.is_empty()),
            "a command with no verb has nothing for :help to draw"
        );
        assert!(
            listed
                .iter()
                .all(|command| command.shortest <= command.name.len()),
            "a command you cannot type in full is not a command"
        );
    }
}
