#!/usr/bin/env bash
# Runs every top-level tapes/*.tape file through `vhs`. Called by `just tapes`
# after tapes/check-versions.sh has already passed.
#
# Files starting with `_` (e.g. the shared `_config.tape` / `_dimensions.tape`
# that V002/V003 add) are convention for `Source`d fragments, not standalone
# tapes, and are skipped.
#
# **Every tape runs, and the run still fails.** This used to stop at the first
# broken tape under `set -e` — "fail loud, don't paper over a partial success" —
# with the note that it was worth revisiting once some tapes passed and others
# did not. That day arrived and nobody revisited: `6b.tape` waited on a word
# `§8` had stopped drawing, `6b` sorts fifth in the glob, and the twenty-odd
# tapes after it — including all six of `CP-4`'s — had not been recorded for
# three commits. One broken tape hid the library.
#
# So a failure is collected rather than fatal: the exit status is still
# non-zero and the names are repeated at the end, which is the "fail loud" the
# original wanted without the "and stop looking" it also bought.
#
# An empty library (no *.tape files at all, `_`-prefixed or not) is still
# success, not a no-op error — see the branch below.
set -euo pipefail
cd "$(dirname "$0")"

# The capture environment — a scratch `$XDG_CONFIG_HOME`, so a recording does
# not load the operator's own `init.scm` into every screen. See tape-env.sh.
# shellcheck source=tape-env.sh
source ./tape-env.sh

shopt -s nullglob
candidates=(*.tape)
shopt -u nullglob

to_run=()
if (( ${#candidates[@]} > 0 )); then
  for f in "${candidates[@]}"; do
    case "${f}" in
      _*) continue ;;
      *) to_run+=("${f}") ;;
    esac
  done
fi

if [[ ${#to_run[@]} -eq 0 ]]; then
  echo "phosphor tapes: library is empty — nothing to record"
  exit 0
fi

failed=()
for tape in "${to_run[@]}"; do
  echo "phosphor tapes: recording ${tape}"
  if ! vhs "${tape}"; then
    failed+=("${tape}")
  fi
done

if (( ${#failed[@]} > 0 )); then
  echo "phosphor tapes: ${#failed[@]} of ${#to_run[@]} tapes failed to record:"
  printf 'phosphor tapes:   %s\n' "${failed[@]}"
  exit 1
fi

echo "phosphor tapes: ${#to_run[@]} tapes recorded"
