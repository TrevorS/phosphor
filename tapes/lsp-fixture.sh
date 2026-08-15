#!/usr/bin/env bash
# Builds the scratch tree an `S4` tape drives, and swaps one line of one
# language declaration to point at a server that answers in constants.
#
# **Why a fixture server and not rust-analyzer.** `CP-4`'s VHS list asks for
# the completion float over real code in three languages. A tape is a pixel
# reference, and rust-analyzer decides for itself what to offer and when: the
# list depends on how far indexing has got, which depends on the machine. A
# tape that raced indexing would produce a different frame every capture,
# which is the flake `V006` exists to prevent for agent surfaces and the exact
# reason `docs/TEAM.md` says a red Tier-2 build teaches the team to ignore the
# harness. `crates/phosphor/tests/fixtures/toy_language_server.py` was written
# for the `S4` pty tests and answers `7c`'s own three labels, its detail column
# and its one row of prose — so the tape shows `7c`'s *shape*, drawn by the
# shipping binary over a real pipe to a real process, and shows it identically
# on every run. What is deliberately real: the framing, the JSON-RPC envelope,
# `initialize`/`didOpen`, and the client's UTF-16 check. See that file's own
# header.
#
# **What this script does NOT swap.** Only the `lsp_command` line, and it is
# edited in the *copied* layer rather than written out here, so the grammar,
# the extensions and the comment prefix stay whatever `runtime/languages/`
# says today. That is what keeps "over real code" true: the buffer is parsed
# and highlighted by the same tree-sitter grammar the editor ships for that
# language, and a declaration that changes upstream changes the capture rather
# than silently disagreeing with it. The visible consequence is the statusline
# chip — `toy-lsp ✓`, the server's own `serverInfo.name`, where `7c` draws
# `rust-analyzer ✓`. Left visible on purpose: a capture that hid which server
# answered would be the dishonest version of this.
#
# **Why a scratch $PHOSPHOR_RUNTIME.** `Runtime::root()`
# (`crates/phosphor-steel/src/runtime.rs`) falls back to `./runtime`, which is
# the tracked tree, and `harness` does not own `runtime/**` (`docs/TEAM.md`).
# Same rule `broken-init.tape` follows, for the same reason.
#
# Usage:
#   lsp-fixture.sh <scratch> <language> <mode> <fixture>...
#
#   scratch    names /tmp/phosphor-tape-<scratch>, wiped and rebuilt
#   language   a name in runtime/languages/ — the declaration whose
#              `lsp_command` is repointed (`rust`, `typescript`, `python`)
#   mode       the toy server's argv: `completion` or `diagnostics`. One
#              process cannot do both — an unsolicited publish arrives on its
#              own schedule, which is the fixture header's own reasoning and
#              is doubly true for a tape counting on a settled frame.
#   fixture    one or more files under tapes/fixtures/, copied in flat
#
# Prints the scratch directory. Every caller `cd`s into it, so the statusline
# draws the file's own name rather than a /tmp path (`runtime/statusline.scm`,
# `status/file` — the path contracts to its basename only when the row is too
# narrow, and at 120 columns it is not).
set -euo pipefail
cd "$(dirname "$0")"

if [ "$#" -lt 4 ]; then
    echo "usage: lsp-fixture.sh <scratch> <language> <mode> <fixture>..." >&2
    exit 1
fi

scratch="/tmp/phosphor-tape-$1"
language="$2"
mode="$3"
shift 3

case "$mode" in
completion | diagnostics) ;;
*)
    echo "lsp-fixture.sh: mode must be 'completion' or 'diagnostics', got '$mode'" >&2
    exit 1
    ;;
esac

server="$(cd ../crates/phosphor/tests/fixtures && pwd)/toy_language_server.py"
if [ ! -f "$server" ]; then
    echo "lsp-fixture.sh: the toy server is not at $server" >&2
    exit 1
fi

rm -rf "$scratch"
mkdir -p "$scratch"
cp -R ../runtime "$scratch/runtime"

declaration="$scratch/runtime/languages/$language.scm"
if [ ! -f "$declaration" ]; then
    echo "lsp-fixture.sh: no declaration for '$language' in runtime/languages/" >&2
    exit 1
fi

# Exactly one line, or the edit below is guessing. A declaration that grew a
# second mention of `lsp_command` (a comment, say) would otherwise be rewritten
# in a place nobody looked at, and the tape would fail as a timeout rather than
# as this sentence.
matches=$(grep -c '"lsp_command"' "$declaration")
if [ "$matches" -ne 1 ]; then
    echo "lsp-fixture.sh: $declaration has $matches lines matching \"lsp_command\", expected 1" >&2
    exit 1
fi

awk -v line="        \"lsp_command\" (list \"python3\" \"$server\" \"$mode\")" \
    '/"lsp_command"/ { print line; next } { print }' \
    "$declaration" >"$declaration.swapped"
mv "$declaration.swapped" "$declaration"

for fixture in "$@"; do
    if [ ! -f "fixtures/$fixture" ]; then
        echo "lsp-fixture.sh: no fixture at tapes/fixtures/$fixture" >&2
        exit 1
    fi
    cp "fixtures/$fixture" "$scratch/$fixture"
done

echo "$scratch"
