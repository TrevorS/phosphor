#!/usr/bin/env bash
# Every key and every ex command the layer binds is pressed by some test.
#
# The logic is `scripts/key_coverage.py`; this is the wrapper that puts it in
# `just lint`'s glob, the same shape `lint-doc-claims.sh` uses for
# `doc_claims.py`.
#
# It closes the last hand-counted audit in the repository. `loop_pty.rs` carried
# the key, ex-command and mouse survey in a comment — a real audit that found
# real holes, counted once and never recomputed. Its own numbers had already
# drifted: it says `(ex-entries)` answers 18 where the layer binds 17.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

exec python3 scripts/key_coverage.py
