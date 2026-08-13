#!/usr/bin/env bash
# V007 — the Tier-2 pixel-diff runner.
#
# Compares a fresh VHS capture of each named screen against the PNG already
# committed at `tapes/artifacts/<id>.png` (git HEAD, not just the working
# tree — see the `git show` call below), and on any difference beyond a
# small tolerance writes a side-by-side diff image and says so. It never
# fails the process for a pixel difference: this is harness's own
# characteristic-failure guard (docs/TEAM.md) written into the script
# itself rather than left to whoever wires it into CI later (`V008`, not
# built yet) remembering `continue-on-error`. See "Exit codes" below.
#
# Usage:
#   tapes/diff-tapes.sh [--no-capture] [id...]
#
#   id...          One or more screen ids (tapes/<id>.tape). Default: every
#                  real tape (tapes/*.tape, `_`-prefixed skipped — same
#                  convention as run-tapes.sh) that has a committed
#                  reference at git HEAD.
#   --no-capture   Skip running `vhs` and compare whatever is already at
#                  `tapes/artifacts/<id>.png` in the working tree. For
#                  re-checking a capture you already made (e.g. by hand, or
#                  via `just tape <id>`), or for driving the comparison
#                  logic directly against a planted image — see this file's
#                  own probe, run once and reverted, described in the V007
#                  section of tapes/README.md.
#
# Why compare against git HEAD and not just "whatever's on disk": every
# real tape's `Screenshot` path is `artifacts/<id>.png`, the same path
# every time (V005's own convention) — so capturing fresh always overwrites
# the file a plain `git diff` would show you were the reference. Reading
# the committed blob straight from git is what makes "the committed
# reference" mean something exact rather than "whatever the last person to
# run this happened to leave lying around".
#
# Exit codes:
#   0   ran cleanly — pixel mismatches (if any) are reported as findings,
#       not failures; see the summary line
#   1   a hard error — a tool is missing, a named id has no `.tape` file, a
#       capture failed outright, or `compare` returned something that isn't
#       a pixel count (a geometry mismatch or similar). Never set for a
#       pixel difference alone.
set -uo pipefail
# Deliberately not `-e`: `compare` exits 1 for "images differ", which is
# this script's normal, expected, non-error path — `set -e` would abort the
# loop on the very first mismatch.
cd "$(dirname "$0")"

USAGE="usage: diff-tapes.sh [--no-capture] [id...]"

no_capture=0
ids=()
for arg in "$@"; do
    case "$arg" in
        --no-capture) no_capture=1 ;;
        -h | --help)
            echo "$USAGE"
            exit 0
            ;;
        -*)
            echo "diff-tapes.sh: unknown flag: $arg" >&2
            echo "$USAGE" >&2
            exit 1
            ;;
        *) ids+=("$arg") ;;
    esac
done

# The comparison toolchain (ImageMagick) is checked for presence only, not
# pinned by version like vhs/ttyd/ffmpeg (tapes/check-versions.sh): it never
# touches a committed reference, only scratch diff output, so a different
# release changes nothing this repo asserts against.
for tool in compare magick; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "diff-tapes.sh: '$tool' not found — install ImageMagick (e.g. brew install imagemagick)" >&2
        exit 1
    fi
done

if [[ "$no_capture" -eq 0 ]]; then
    bash check-versions.sh || exit 1
fi

if [[ ${#ids[@]} -eq 0 ]]; then
    shopt -s nullglob
    for f in *.tape; do
        base="$(basename "$f" .tape)"
        [[ "$base" == _* ]] && continue
        ids+=("$base")
    done
    shopt -u nullglob
fi

if [[ ${#ids[@]} -eq 0 ]]; then
    echo "diff-tapes.sh: no tapes found"
    exit 0
fi

# Fuzz tolerance. `tapes/README.md`'s V002 investigation measured up to
# 1/255 (~0.39%) per-channel drift between an escape code's intended colour
# and what actually lands in a captured PNG, sourced somewhere in the
# headless-Chromium canvas -> PNG path rather than vhs's terminal emulation
# itself. 0.6% comfortably absorbs that noise floor (confirmed this session:
# a same-content, same-tape rerun of `1a.tape` scored AE 0 at this
# tolerance) without hiding a real change — a deliberate one-cell colour
# change scores AE in the hundreds against a 1228x700 capture, five orders
# of magnitude past this floor. See the V007 section of tapes/README.md for
# the worked before/after numbers.
FUZZ="0.6%"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/phosphor-tape-diff.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT

diff_dir="artifacts/_diffs"
mkdir -p "$diff_dir"

pass=0
mismatch=0
skipped=0
hard_error=0

for id in "${ids[@]}"; do
    tape="${id}.tape"
    if [[ ! -f "$tape" ]]; then
        echo "diff-tapes.sh: ${id} — no such tape (${tape} not found)" >&2
        hard_error=1
        continue
    fi

    ref="${work_dir}/${id}.ref.png"
    if ! git show "HEAD:tapes/artifacts/${id}.png" >"$ref" 2>/dev/null; then
        echo "· ${id} — no committed reference yet, nothing to diff against"
        rm -f "$ref"
        skipped=$((skipped + 1))
        continue
    fi

    if [[ "$no_capture" -eq 0 ]]; then
        echo "diff-tapes.sh: capturing ${id}"
        if ! vhs "$tape" >/dev/null 2>&1; then
            echo "x ${id} — vhs capture failed (see \`vhs ${tape}\` directly for the reason)" >&2
            hard_error=1
            continue
        fi
    fi

    fresh="artifacts/${id}.png"
    if [[ ! -f "$fresh" ]]; then
        echo "x ${id} — no capture at ${fresh} (drop --no-capture, or run 'just tape ${id}' first)" >&2
        hard_error=1
        continue
    fi

    # First pass: count only (`null:` — no file written). `compare`'s own
    # exit code is the pass/fail signal (0 similar-within-fuzz, 1
    # dissimilar, 2 error) — trusted directly rather than re-deriving it by
    # parsing the AE number, which already incorporates the fuzz tolerance.
    ae_raw="$(compare -metric AE -fuzz "$FUZZ" "$ref" "$fresh" null: 2>&1)"
    rc=$?
    ae="${ae_raw%% *}"

    if [[ "$rc" -eq 2 ]]; then
        echo "x ${id} — compare could not run (${ae_raw})" >&2
        hard_error=1
        continue
    fi

    if [[ "$rc" -eq 0 ]]; then
        echo "= ${id} — matches committed reference"
        pass=$((pass + 1))
        continue
    fi

    # rc == 1: a real mismatch. Redo with a real output path to also get
    # the highlighted-diff image (one compare call would give us both, but
    # we don't want a diff image on disk for the common pass case above).
    highlight="${work_dir}/${id}.highlight.png"
    compare -fuzz "$FUZZ" "$ref" "$fresh" "$highlight" 2>/dev/null
    # `montage`'s default per-tile caption needs a font, and this ImageMagick
    # install has none configured (`convert -list font` — confirmed empty
    # this session) — it fails even with `-label ''`. `+append` after a
    # `-splice` pad needs no font at all and was proven identical in intent
    # (verified against a planted one-cell colour change — see
    # tapes/README.md's V007 section) — left-to-right order is always
    # committed reference, fresh capture, highlighted diff, documented here
    # since there is no in-image label to say so.
    diff_png="${diff_dir}/${id}.diff.png"
    magick "$ref" "$fresh" "$highlight" -background '#222222' -splice 4x0+0+0 +append "$diff_png"

    echo "x ${id} — MISMATCH (${ae} px beyond ${FUZZ} tolerance) — see ${diff_png}"
    echo "    left to right: committed reference | fresh capture | diff (red = changed)"
    mismatch=$((mismatch + 1))
done

echo
echo "diff-tapes.sh: ${pass} matched, ${mismatch} mismatched, ${skipped} skipped (no reference yet)"
if [[ "$mismatch" -gt 0 ]]; then
    echo "diff-tapes.sh: this is Tier 2 (docs/TASKS.md's three verification tiers) — a"
    echo "  change detector, not a build gate. A mismatch is a request to look at"
    echo "  ${diff_dir}/*.diff.png, not a failure. If the new capture is correct,"
    echo "  'git add tapes/artifacts/<id>.png' and commit it as a reviewed reference"
    echo "  update; if not, 'git checkout -- tapes/artifacts/<id>.png' to discard it."
fi

if [[ "$hard_error" -ne 0 ]]; then
    exit 1
fi
exit 0
