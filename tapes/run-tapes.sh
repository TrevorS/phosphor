#!/usr/bin/env bash
# Runs every top-level tapes/*.tape file through `vhs`. Called by `just tapes`
# after tapes/check-versions.sh has already passed.
#
# Files starting with `_` (e.g. the shared `_config.tape` / `_dimensions.tape`
# that V002/V003 add) are convention for `Source`d fragments, not standalone
# tapes, and are skipped.
#
# `set -euo pipefail` means the first tape that fails stops the run right
# there — deliberate (fail loud, don't paper over a broken capture with a
# partial "success"), but it does mean `just tapes` won't tell you about
# tape #7's problem until #1-6 pass. Not a concern while every real tape is
# expected to fail (see tapes/README.md's "Screen library convention" —
# Window B ships the harness before Window C ships anything for it to
# capture); worth revisiting if that ever hides real regressions once some
# tapes pass and others don't.
#
# An empty library (no *.tape files at all, `_`-prefixed or not) is still
# success, not a no-op error — see the branch below.
set -euo pipefail
cd "$(dirname "$0")"

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

for tape in "${to_run[@]}"; do
  echo "phosphor tapes: recording ${tape}"
  vhs "${tape}"
done
