//! The keymap, asked rather than cached — `T022`'s liveness claim, as a
//! function.
//!
//! > *"a keybinding redefined at the REPL takes effect on the next frame
//! > without restart"* — `IMPLEMENTATION-PLAN.md`, `S2` acceptance
//!
//! The table is `runtime/keymaps.scm`'s and **there is no copy of it on this
//! side**. [`press`] hands the VM one key in vim notation and reads back what
//! the editor layer did with it. That is the whole mechanism, and it is why the
//! claim needs no invalidation, no reload and no cache-coherence rule: a
//! `(keymap-set! …)` typed at `:repl` mutated the only table there is, one
//! keystroke ago.
//!
//! # Why the host may not interpret a binding
//!
//! [`Press`] carries *what happened*, never *what is bound*. A binding is a
//! scheme closure; handing one to Rust would put a `SteelVal` in the input path
//! and make the keymap a thing two sides both understand — which is exactly how
//! a cached copy gets born. The host learns `handled`, `pending` or `unbound`,
//! and `unbound` is the only one it may act on.
//!
//! # Degradation
//!
//! A runtime tree with no `keymaps.scm` — or one whose forms failed — has no
//! `phosphor/press`, so the call raises and every key answers
//! [`Press::Unbound`]. The editor is then exactly the editor it was before this
//! module existed rather than one that eats keystrokes, which is the same
//! promise `T021` makes about a broken `init.scm`.
//!
//! Owned by `spine`.

use phosphor_core::action::Outcome;
use phosphor_core::value::Value;

use crate::convert::string_literal;
use crate::runtime::Runtime;

/// The editor layer's dispatcher: one key in, one symbol out.
pub const PRESS: &str = "phosphor/press";

/// Drops an unfinished sequence.
pub const RESET: &str = "phosphor/press-reset";

/// What the editor layer did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// A binding ran. The host does nothing further with this key.
    Handled,
    /// The keys so far are a prefix of something bound — `]` while `]r` exists.
    /// The host does nothing further either, and the next key continues the
    /// sequence.
    Pending,
    /// Nothing in the layer wants it; the host may have its own use for it.
    Unbound,
}

/// Asks the live keymap what to do with one keystroke.
///
/// `key` is vim notation as the editor layer spells it: `q`, `]`, `<C-c>`,
/// `<esc>`. Encoding a terminal event into it is the app layer's job (it owns
/// crossterm) and `T026`'s to own properly.
pub fn press(runtime: &mut Runtime, key: &str) -> Press {
    let call = format!("({PRESS} {})", string_literal(key));
    match runtime.evaluate(&call) {
        Outcome::Done(receipt) => match &receipt.value {
            // `convert::from_steel` narrows a scheme symbol to text, so the
            // three answers arrive as their own names.
            Value::Text(answer) if answer == "handled" => Press::Handled,
            Value::Text(answer) if answer == "pending" => Press::Pending,
            _ => Press::Unbound,
        },
        // No dispatcher, or one that raised. Either way the key is the host's.
        Outcome::Refused(_) => Press::Unbound,
    }
}

/// Drops whatever sequence was in progress.
pub fn reset(runtime: &mut Runtime) {
    let _ = runtime.evaluate(&format!("({RESET})"));
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::*;
    use crate::host::{Detached, Host};

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

    #[test]
    fn the_shipped_layer_binds_the_repl_and_nothing_else() {
        let mut runtime = runtime();
        assert_eq!(press(&mut runtime, ":"), Press::Handled);
        assert_eq!(press(&mut runtime, "q"), Press::Unbound);
    }

    #[test]
    fn a_rebind_is_in_force_on_the_very_next_key() {
        // `T022`'s claim, at the level this crate can hold it: no reload, no
        // second boot, no invalidation — the next press sees it.
        let mut runtime = runtime();
        assert_eq!(press(&mut runtime, "g"), Press::Unbound);

        let outcome = runtime.evaluate("(keymap-set! \"g\" (lambda () 1))");
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");

        assert_eq!(press(&mut runtime, "g"), Press::Handled);

        let _ = runtime.evaluate("(keymap-remove! \"g\")");
        assert_eq!(press(&mut runtime, "g"), Press::Unbound);
    }

    #[test]
    fn an_unfinished_sequence_is_pending_and_resets() {
        let mut runtime = runtime();
        let _ = runtime.evaluate("(keymap-set! \"]r\" (lambda () 1))");
        assert_eq!(press(&mut runtime, "]"), Press::Pending);
        assert_eq!(press(&mut runtime, "r"), Press::Handled);

        assert_eq!(press(&mut runtime, "]"), Press::Pending);
        reset(&mut runtime);
        assert_eq!(
            press(&mut runtime, "r"),
            Press::Unbound,
            "a reset sequence does not complete itself later"
        );
    }

    #[test]
    fn a_layer_without_a_dispatcher_leaves_every_key_to_the_host() {
        let host: Arc<dyn Host> = Arc::new(Detached);
        let mut runtime = Runtime::boot(None, host);
        assert_eq!(press(&mut runtime, ":"), Press::Unbound);
    }
}
