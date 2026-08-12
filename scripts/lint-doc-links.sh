#!/usr/bin/env bash
# Tooling pass — structural lint: the doc comments' cross-references resolve.
#
# This codebase cross-references itself constantly: `action.rs`'s header alone
# carries a dozen intra-doc links, and the reasoning threaded through those
# headers is what stops the next agent redesigning something that was decided
# on purpose. A link is how that reasoning stays reachable.
#
# Nothing checked them until this script, because `cargo doc` was never run —
# not by `just build`, not by `just clippy`, not by CI. A broken link was
# silent. Running it for the first time found **eight**, two of them introduced
# hours earlier by the `CP-2` repair, which removed two imports and left the
# module header pointing at them.
#
# That is the `VENDOR.md` defect class again, one layer down: prose asserting a
# relationship the tree no longer has. `CLAUDE.md`'s rule is that a claim about
# a file is only worth making if you read the file; a dead link is a claim that
# a name exists, and this is the check that a name still does.
#
# What is denied and what is not lives in `[workspace.lints.rustdoc]` in
# `Cargo.toml`, next to the clippy and rustc tables, so all three are read
# together. This script's job is to make `cargo doc` actually run, with warnings
# promoted, which is the part no other recipe does.
#
# Deliberate limits: `--no-deps` (upstream docs are not ours to fix), and the
# vendored forks are outside `[workspace] members` so they are not reached —
# the same seam `just vendor-diff` guards.
#
# Exit 0 = every link resolves, exit 1 = at least one does not.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "lint-doc-links: cargo not on PATH" >&2
    exit 1
fi

# `-D warnings` promotes the `warn`-level rustdoc lints too, so an
# `unescaped_backticks` or an `invalid_html_tags` fails here rather than
# accumulating. `--no-deps` keeps it to code we wrote.
output=$(RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet 2>&1) || {
    echo "$output"
    echo
    echo "lint-doc-links: FAILED — a doc comment references something that does not resolve."
    echo "  Fix the link, or use the full path (\`phosphor_core::registry::cli::verbs\`)"
    echo "  when the item lives in another crate. Disambiguate a name that is both a"
    echo "  function and a macro with \`fn@name\`."
    exit 1
}

links=$(grep -rho '\[`[^`]*`\]' crates --include='*.rs' | wc -l | tr -d ' ')
echo "lint-doc-links: clean — ${links} intra-doc links across crates/ all resolve"
