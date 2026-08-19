#!/usr/bin/env bash
# The environment every VHS capture runs in. `source`d, never executed —
# it exports, and a subshell's exports reach nothing.
#
# # Why this file exists
#
# `_config.tape` removes every source of nondeterminism *inside* the emulated
# terminal — font, geometry, cursor blink, theme, typing speed. It cannot
# remove the one outside it: the recording machine's own `$XDG_CONFIG_HOME`.
# Since §34 a config-home `init.scm` layers over the shipped tree on every
# start, and since `T101` a `persisted.scm` there does too, so `just tapes` on
# an operator's laptop loaded that operator's editor config into every Tier-2
# capture. The tapes set `PHOSPHOR_RUNTIME=../runtime` and nothing else, and
# `grep -rn XDG_CONFIG_HOME tapes/ scripts/ justfile` returned nothing before
# this file — the pty harness sets one (`loop_pty.rs`'s `config_home`) and so
# does `benches/vm_invocations.rs`, which is what left the tapes as the only
# uncovered surface. `CP-4` reads the capture library as a change detector, so
# a screen that moves because somebody edited their own `init.scm` is a false
# positive nobody could explain.
#
# # Why the environment and not a `Set` line
#
# `_config.tape` holds only `Set` lines on purpose — a bare `Source` of it must
# not end vhs's before-first-command window, or the per-tape `Set Width` after
# it stops taking effect — and vhs 0.11 has no `Env` command at all (checked
# against `vhs manual`'s command list). What it does have is plain inheritance:
# vhs spawns ttyd, ttyd spawns the shell, and the shell sees the environment
# vhs was started in. Verified by running a probe tape whose only command was
# `env | grep XDG_CONFIG_HOME > cfg.txt`, with this variable exported around it
# — the file came back holding the exported value.
#
# Every entry point that runs `vhs` sources this: `run-tapes.sh`,
# `diff-tapes.sh`, and `record-one.sh` behind `just tape <id>`.

# A scratch config home, emptied every run. Not a path inside the repository:
# some tapes evaluate `persist!`, which creates the directory and appends to a
# file in it, and a capture run may not leave a new untracked file in the tree
# it is capturing. Absolute, because `phosphor_core::config` ignores a relative
# `XDG_CONFIG_HOME` per the XDG spec and would fall back to `$HOME/.config` —
# which is the operator's, the exact thing this removes.
PHOSPHOR_TAPES_CONFIG_HOME="${TMPDIR:-/tmp}/phosphor-tapes-config"
rm -rf "${PHOSPHOR_TAPES_CONFIG_HOME}"
mkdir -p "${PHOSPHOR_TAPES_CONFIG_HOME}"
export XDG_CONFIG_HOME="${PHOSPHOR_TAPES_CONFIG_HOME}"

# ── The same hole, one variable over ──────────────────────────────────────
#
# This file closed `XDG_CONFIG_HOME` and left `XDG_STATE_HOME` open, and that
# was correct exactly until `T041`/`T044` landed a store that persists. Since
# then the seen-state journal lives under `$XDG_STATE_HOME/phosphor/<hash of
# the canonical workspace root>/seen.journal`, so a capture of any screen the
# store feeds — the gutter's unseen markers, the unseen picker, the files
# picker's activity column — read whatever store the recording machine
# happened to have. `grep -rn XDG_STATE_HOME tapes/` returned nothing before
# this block: every tape inherited the operator's.
#
# That is the same false positive the paragraph above describes, and worse in
# one way: an `init.scm` is something an operator knows they wrote, and a
# journal is a file they have never heard of, written by an editor session
# they have forgotten.
#
# Scratch and emptied every run, for the config home's reasons. Absolute for
# its reason too — `phosphor_core::config`'s XDG handling ignores a relative
# one and falls back to `$HOME`, which is the operator's.
#
# **Empty is the right default.** Most of the library draws files under
# `tapes/fixtures/`, which no seed touches, so those screens want a store with
# nothing in it and now provably get one. A screen that needs *seeded* state —
# `CP-5`'s — asks for it with `tapes/seed-state.sh`, which fills this same home
# from `fixtures/seed/plan.scm` and nowhere else.
PHOSPHOR_TAPES_STATE_HOME="${TMPDIR:-/tmp}/phosphor-tapes-state"
rm -rf "${PHOSPHOR_TAPES_STATE_HOME}"
mkdir -p "${PHOSPHOR_TAPES_STATE_HOME}"
export XDG_STATE_HOME="${PHOSPHOR_TAPES_STATE_HOME}"
