//! Does `steel-core 0.8.2` actually embed, evaluate, and call back into Rust?
//!
//! The same shape of question `T083` asked of the grammars, and asked for the
//! same reason: `phosphor-steel` declares `steel-core` and nothing yet *uses*
//! it, so `cargo build` proves the pin resolves and nothing more. Every task in
//! Window C (`T020`–`T025`) assumes this VM embeds, evaluates `runtime/*.scm`,
//! and can have Rust functions registered into it — and [Q5] pins it exactly
//! *because* it is pre-1.0, which is the version of "we depend on this" that
//! deserves a check before six tasks are built on top.
//!
//! Deliberately minimal. This is not `T020` — no Action bindings, no registry,
//! no `runtime/` loading. It answers three questions and stops:
//!
//! 1. Can a VM be constructed at all?
//! 2. Does it evaluate an expression and give back a value Rust can read?
//! 3. Can Rust register a value the Scheme side then uses? That is the
//!    direction the whole editor layer depends on — Steel calling into us.
//!
//! [Q5]: ../../../docs/IMPLEMENTATION-PLAN.md

use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;

/// A VM exists, and arithmetic survives the round trip.
#[test]
fn the_vm_embeds_and_evaluates() {
    let mut vm = Engine::new();
    let out = vm
        .compile_and_run_raw_program("(+ 1 2)")
        .expect("steel-core 0.8.2 failed to compile and run `(+ 1 2)`");

    assert_eq!(out.last(), Some(&SteelVal::IntV(3)), "got {out:?}");
}

/// Steel-dialect forms the editor layer will actually be written in — `define`
/// and `lambda`, then calling the result. `runtime/init.scm` is made of this.
#[test]
fn define_and_lambda_work_the_way_runtime_scm_will_need() {
    let mut vm = Engine::new();
    let out = vm
        .compile_and_run_raw_program("(define (double x) (* 2 x)) (double 21)")
        .expect("steel-core 0.8.2 failed on a define + application");

    assert_eq!(out.last(), Some(&SteelVal::IntV(42)), "got {out:?}");
}

/// **The direction that matters.** Invariant 1 puts the editor layer in Steel
/// over a Rust core, so the crossing that has to work is Rust handing something
/// in and Scheme using it. If this ever fails, `T022`'s bindings are the thing
/// to redesign, not the caller.
#[test]
fn rust_can_register_a_value_that_scheme_then_uses() {
    let mut vm = Engine::new();
    vm.register_value("phosphor/answer", SteelVal::IntV(42));

    let out = vm
        .compile_and_run_raw_program("(+ phosphor/answer 1)")
        .expect("steel-core 0.8.2 failed to see a registered value");

    assert_eq!(out.last(), Some(&SteelVal::IntV(43)), "got {out:?}");
}
