#!/usr/bin/env bash
# Runs every top-level tapes/*.tape file through `vhs`. Called by `just tapes`
# after tapes/check-versions.sh has already passed.
#
# Files starting with `_` (e.g. the shared `_config.tape` / `_dimensions.tape`
# that V002/V003 add) are convention for `Source`d fragments, not standalone
# tapes, and are skipped.
#
# There are no real tapes yet — V002-V005 (Window B) add the first ones and the
# per-screen convention they follow. Until then this is intentionally quiet:
# an empty library is success, not a no-op error.
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
