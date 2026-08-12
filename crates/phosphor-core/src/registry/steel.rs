//! The Steel door — one binding per capability, derived from its row.
//!
//! **Nothing here is a list.** [`binding`] is a total function of a
//! [`Capability`], so a capability cannot be missing from this door: there is no
//! place to forget it. `T021`'s `phosphor-steel/registry.rs` walks
//! [`bindings`] and installs each one into the VM; it holds no table of its own,
//! and the moment it does, invariant 2 is decorative.
//!
//! # The calling convention
//!
//! Positional, in declaration order. `6b` fixes that before we get a vote —
//! `(unseen-regions "src/retry.rs")` and `(watch-place "src/retry.rs:24" 'delay)`
//! are positional calls (TUI Mockups.dc.html:493-503).
//!
//! Optionality is expressed by *omitting a trailing run*, not by a keyword. That
//! is why [`Arity::min`] is one past the **last** required parameter rather than
//! the count of required ones: `paste` declares `at`, `register: Option<…>`,
//! `before`, and an optional in the middle cannot be omitted positionally
//! without moving the one after it. Omitted arguments arrive as
//! [`Value::Null`], which is exactly what an `Option<T>` field decodes from
//! ([`Wire::REQUIRED`](crate::value::Wire::REQUIRED)).
//!
//! Owned by `spine`.

use crate::action::ALIASES;
use crate::registry::{Capability, CapabilityKind, Param, Since, capabilities, lookup};
use crate::value::{Args, Call, Value};

// ---------------------------------------------------------------------------
// Arity
// ---------------------------------------------------------------------------

/// How many arguments a binding accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arity {
    /// The fewest a call may pass.
    pub min: usize,
    /// The most — always the declared parameter count.
    pub max: usize,
}

impl Arity {
    /// Whether a call passing `argc` arguments is well-formed.
    #[must_use]
    pub const fn accepts(self, argc: usize) -> bool {
        argc >= self.min && argc <= self.max
    }

    /// Whether every argument is mandatory.
    #[must_use]
    pub const fn is_fixed(self) -> bool {
        self.min == self.max
    }
}

/// The arity of a parameter list — see the module docs for why it is the *last*
/// required parameter and not the count of them.
#[must_use]
pub fn arity(params: &[Param]) -> Arity {
    let min = params
        .iter()
        .rposition(|param| param.required)
        .map_or(0, |index| index + 1);
    Arity {
        min,
        max: params.len(),
    }
}

// ---------------------------------------------------------------------------
// The binding
// ---------------------------------------------------------------------------

/// One capability as the Steel door sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The Scheme identifier: [`Capability::steel_name`], so an Action carries a
    /// `!` and a query does not.
    pub name: String,
    /// The capability's canonical door name, for dispatch back into
    /// [`Action::from_call`](crate::action::Action::from_call).
    pub capability: &'static str,
    /// The domain it groups under in `:help`.
    pub domain: &'static str,
    /// One line, in the product's voice.
    pub doc: &'static str,
    /// Action or query — which is also what the `!` encodes.
    pub kind: CapabilityKind,
    /// The phase and task that implement it.
    pub since: Since,
    /// The parameters, in the order a call passes them.
    pub params: &'static [Param],
    /// How many arguments a call may pass.
    pub arity: Arity,
}

impl Binding {
    /// The signature as `:describe-action` and which-key show it:
    /// `(mark-seen! target)`, with omittable arguments in brackets.
    #[must_use]
    pub fn signature(&self) -> String {
        let mut out = format!("({}", self.name);
        for (index, param) in self.params.iter().enumerate() {
            if index >= self.arity.min {
                let _ = write_arg(&mut out, param.name, true);
            } else {
                let _ = write_arg(&mut out, param.name, false);
            }
        }
        out.push(')');
        out
    }

    /// Maps positional arguments onto named ones.
    ///
    /// The one piece of calling-convention knowledge the Steel door needs, kept
    /// here so `T021` has none of its own. A trailing run of omitted arguments
    /// is filled with [`Value::Null`].
    ///
    /// # Errors
    ///
    /// [`ArityError`] if the count is outside [`Binding::arity`]. A wrong
    /// *shape* is not this function's error — that surfaces from
    /// [`Wire::from_value`](crate::value::Wire::from_value) with the argument
    /// named, which is the better message.
    pub fn args(&self, values: Vec<Value>) -> Result<Args, ArityError> {
        if !self.arity.accepts(values.len()) {
            return Err(ArityError {
                capability: self.capability,
                expected: self.arity,
                got: values.len(),
            });
        }
        let mut args = Args::new();
        let mut values = values.into_iter();
        for param in self.params {
            args.set(param.name, values.next().unwrap_or(Value::Null));
        }
        Ok(args)
    }

    /// A positional call, as a door-neutral [`Call`].
    ///
    /// # Errors
    ///
    /// [`ArityError`], as [`Binding::args`].
    pub fn call(&self, values: Vec<Value>) -> Result<Call, ArityError> {
        Ok(Call {
            name: self.capability.to_owned(),
            args: self.args(values)?,
        })
    }
}

fn write_arg(out: &mut String, name: &str, optional: bool) -> core::fmt::Result {
    use core::fmt::Write as _;
    if optional {
        write!(out, " [{name}]")
    } else {
        write!(out, " {name}")
    }
}

/// This capability's Steel binding. Total — see the module docs.
#[must_use]
pub fn binding(capability: &Capability) -> Binding {
    Binding {
        name: capability.steel_name(),
        capability: capability.name,
        domain: capability.domain,
        doc: capability.doc,
        kind: capability.kind,
        since: capability.since,
        params: capability.params,
        arity: arity(capability.params),
    }
}

/// Every capability's binding, in registry order.
#[must_use]
pub fn bindings() -> Vec<Binding> {
    capabilities().iter().map(binding).collect()
}

// ---------------------------------------------------------------------------
// Aliases
// ---------------------------------------------------------------------------

/// A second Scheme identifier for a capability.
///
/// Only the Steel door has these, and only because a *drawing* has one: `6b`
/// writes `(watch-place …)`, which is mutating with no bang and noun-first
/// against the rest of the vocabulary. An alias is an extra name for a
/// registered capability, never a capability that exists in one door — so it
/// does not weaken the parity `T024` checks. The alias is spelled **verbatim**,
/// without the `!` the naming rule would add, because the drawing has no `!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasBinding {
    /// The alternative identifier, exactly as drawn.
    pub name: &'static str,
    /// The canonical capability's door name.
    pub capability: &'static str,
    /// The canonical Scheme identifier it forwards to.
    pub canonical: String,
    /// Why it exists.
    pub reason: &'static str,
}

/// Every alias binding, from [`ALIASES`].
///
/// # Panics
///
/// If an alias names a capability that does not exist —
/// `tests/vocabulary.rs` already proves it cannot.
#[must_use]
pub fn alias_bindings() -> Vec<AliasBinding> {
    ALIASES
        .iter()
        .map(|alias| {
            let capability =
                lookup(alias.canonical).expect("every alias resolves to a registered capability");
            AliasBinding {
                name: alias.alias,
                capability: capability.name,
                canonical: capability.steel_name(),
                reason: alias.reason,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A call passed the wrong number of arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArityError {
    /// The capability being called.
    pub capability: &'static str,
    /// What it accepts.
    pub expected: Arity,
    /// What arrived.
    pub got: usize,
}

impl core::fmt::Display for ArityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self {
            capability,
            expected,
            got,
        } = self;
        if expected.is_fixed() {
            write!(
                f,
                "`{capability}` takes {} argument(s), got {got}",
                expected.min
            )
        } else {
            write!(
                f,
                "`{capability}` takes {} to {} arguments, got {got}",
                expected.min, expected.max
            )
        }
    }
}

impl std::error::Error for ArityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_action_takes_a_bang_and_a_query_does_not() {
        let mark_seen = binding(&lookup("mark-seen").expect("registered"));
        assert_eq!(mark_seen.name, "mark-seen!");
        let unseen = binding(&lookup("unseen-regions").expect("registered"));
        assert_eq!(unseen.name, "unseen-regions");
    }

    #[test]
    fn a_trailing_optional_may_be_omitted() {
        // `yank` declares target, register: Option<…> — the optional is last.
        let yank = binding(&lookup("yank").expect("registered"));
        assert_eq!(yank.arity, Arity { min: 1, max: 2 });
        let args = yank
            .args(vec![Value::Text("x".to_owned())])
            .expect("one argument is enough");
        assert_eq!(args.get("register"), Some(&Value::Null));
    }

    #[test]
    fn an_optional_in_the_middle_stays_mandatory() {
        // `paste` declares at, register: Option<…>, before — omitting the
        // middle one positionally would move `before`, so it cannot be omitted.
        let paste = binding(&lookup("paste").expect("registered"));
        assert!(paste.arity.is_fixed(), "{:?}", paste.arity);
        assert_eq!(paste.arity.min, paste.params.len());
    }

    #[test]
    fn arity_is_checked_before_shape() {
        let mark_seen = binding(&lookup("mark-seen").expect("registered"));
        let error = mark_seen
            .args(vec![])
            .expect_err("one argument is required");
        assert_eq!(error.got, 0);
    }

    #[test]
    fn every_capability_has_a_binding_and_a_signature() {
        for capability in capabilities() {
            let binding = binding(&capability);
            assert!(!binding.name.is_empty());
            assert!(binding.signature().starts_with('('));
            assert!(binding.arity.max >= binding.arity.min);
        }
    }

    #[test]
    fn the_drawn_alias_resolves_to_the_canonical_binding() {
        let aliases = alias_bindings();
        let watch_place = aliases
            .iter()
            .find(|alias| alias.name == "watch-place")
            .expect("6b's spelling is aliased");
        assert_eq!(watch_place.canonical, "place-watch!");
    }

    #[test]
    fn a_zero_argument_capability_binds_as_a_thunk() {
        let clear = binding(&lookup("clear-selection").expect("registered"));
        assert_eq!(clear.arity, Arity { min: 0, max: 0 });
        assert_eq!(clear.signature(), "(clear-selection!)");
        assert!(clear.args(vec![]).expect("no arguments").is_empty());
    }
}
