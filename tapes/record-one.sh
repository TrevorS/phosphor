#!/usr/bin/env bash
# Records exactly one tape — what `just tape <id>` runs.
#
# The recipe used to be `cd tapes && vhs "<id>.tape"` inline in the justfile,
# and that is why this file exists rather than one more line there: the
# environment a capture must run in is `tape-env.sh`'s, three entry points need
# it, and a justfile recipe is the one of the three that cannot `source`
# anything. Same cwd as `run-tapes.sh` (`tapes/`), so a tape's relative paths —
# `Source "_config.tape"`, `Screenshot "artifacts/<id>.png"` — resolve
# identically whichever way it was regenerated.
set -euo pipefail
cd "$(dirname "$0")"

if [[ $# -ne 1 ]]; then
  echo "usage: tapes/record-one.sh <id>" >&2
  exit 2
fi

# shellcheck source=tape-env.sh
source ./tape-env.sh

exec vhs "$1.tape"
