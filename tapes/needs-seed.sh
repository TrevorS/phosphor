#!/usr/bin/env bash
# Does this capture need a seeded store? `source`d, never executed.
#
# Derived from the tape itself and never a list kept here. A `CP-5` screen
# launches the editor in `../fixtures` — the tree `fixtures/seed/plan.scm`
# seeds, and the only reason a capture needs a store at all. Every other tape
# in the library draws `tapes/fixtures/`, which no seed touches, and wants the
# empty store `tape-env.sh` just made.
#
# A list of "the tapes that need seeding" is the thing that rots the first time
# somebody adds one, which is `lint-fuzz-targets.sh`'s argument about
# `[[bin]]` entries and `parity.rs`'s about capabilities. The tape says what it
# needs by what it does.
#
# Seeding costs about two dozen process launches, so this is what keeps a
# single `just tape 1a` from paying for a store it will not read.

# `needs_seed <tape>...` — true when any named tape starts the editor in the
# seeded workspace.
needs_seed() {
    grep -lq 'cd \.\./fixtures' "$@" 2>/dev/null
}
