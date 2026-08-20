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

# `seed_if_needed <tape>` — a fresh store for this capture, when it reads one.
#
# **Per tape, and that is the whole point.** Seeding once per run was the first
# shape and it has an ordering hazard with teeth: `seen-cleared.tape` presses
# `SPC u s`, which is the screen it exists to capture and also a **write** to
# the store every later capture then reads. Measured — `diff-tapes.sh
# seen-cleared 2a` leaves `2a` waiting on a picker row that the region it just
# marked seen no longer draws, and reports a capture failure.
#
# The library survives today only because `seen-cleared` sorts last among the
# seven seeded ids. That is luck, not design: `just tape-diff 2a` after it, or
# any new seeded tape sorting later, brings it back.
#
# `seed-state.sh` is idempotent and clears before it writes, so this is safe to
# call per tape; it costs about two dozen process launches for the captures
# that need one and nothing for the forty that do not.
seed_if_needed() {
    if needs_seed "$1"; then
        bash ./seed-state.sh
    fi
}
