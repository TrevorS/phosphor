#!/usr/bin/env bash
# Do the docs still describe this repository?
#
# A thin wrapper so the check joins the `scripts/lint-*.sh` glob that `just lint`
# and CI already run; the checks themselves are in `doc_claims.py`, because they
# are graph arithmetic and bash is the wrong tool for it.
#
# What it verifies, and why each one is here: see that file's module docstring.
# Every check corresponds to a claim that had already gone stale in this repo and
# was found by hand rather than by anything mechanical.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v python3 >/dev/null 2>&1; then
    echo "lint-doc-claims: FAILED — python3 not found; this lint cannot verify anything without it"
    exit 1
fi

exec python3 scripts/doc_claims.py
