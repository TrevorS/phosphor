#!/usr/bin/env bash
# Version gate for the Tier-2 (VHS) verification harness. `just tapes` runs this
# before touching a single tape. Pixel comparison (V002+) is only meaningful
# against a fixed renderer, so a wrong version fails loudly here rather than
# silently producing a recording that will never pixel-match a committed
# reference. See tapes/README.md for the full rationale and the
# reference-regeneration machine this was pinned against.
#
# Pinned versions are intentionally inlined here rather than sourced from a
# second file: this script is the version gate, so it is the one place that
# should have to change when a pin is deliberately bumped.
#
# ## Two modes, because ffmpeg is not in the path this gate exists to protect
#
# No argument checks all three, and is what `just tapes` and `just tape` run:
# they REGENERATE references, GIFs included, and a GIF's bytes are ffmpeg's.
#
# `pixels` checks vhs and ttyd only, and is what `diff-tapes.sh` runs.
# **Nothing in this repository ever compares a GIF.** `diff-tapes.sh` compares
# `*.png` and only `*.png` — PNG is lossless and `compare -metric AE` is a
# *pixel* metric, so the encoder that wrote the file cannot move the number it
# reports. The renderer whose determinism the header above is about is
# vhs + ttyd + the font; ffmpeg encodes a video nobody diffs.
#
# **Measured before this split was made, on 2026-08-16**, because the claim is
# the whole justification: with ffmpeg 9.0.1 installed against a pin of 8.1.2,
# `1a.tape` was captured twice and the two PNGs differed by `0 (0)` pixels. A
# wrong ffmpeg is not a wrong renderer. Blocking the diff on it cost a `CP-4`
# tapes-diff run that would otherwise have worked — and homebrew no longer
# carries an `ffmpeg@8` at all (checked the same session: 2.8, 4, 5, 6, 7, 9),
# so the pin as written could not be satisfied on the reference machine by any
# ordinary route.
#
# The pin stays for regeneration rather than being deleted: the GIFs are
# tracked, and a library half-encoded by one ffmpeg and half by another is the
# kind of drift `tapes/README.md` exists to prevent — even where nothing
# asserts on it today.
set -euo pipefail

mode="${1:-all}"
case "${mode}" in
all | pixels) ;;
*)
    echo "check-versions.sh: unknown mode '${mode}' (expected 'all' or 'pixels')" >&2
    exit 2
    ;;
esac

REQUIRED_VHS="0.11.0"
REQUIRED_TTYD="1.7.7"
REQUIRED_FFMPEG="8.1.2"

fail() {
  local problem="$1" expected="$2" found="$3"
  echo "phosphor tapes: ${problem}" >&2
  echo >&2
  echo "  expected: ${expected}" >&2
  echo "  found:    ${found}" >&2
  echo >&2
  echo "  install the pinned version and retry — see tapes/README.md for why" >&2
  echo "  these are pinned and where the pins are recorded." >&2
  exit 1
}

check() {
  local name="$1" required="$2" version_cmd="$3"
  if ! command -v "${name}" >/dev/null 2>&1; then
    fail "${name} is not on PATH" "${name} ${required}" "not found"
  fi
  local raw
  raw="$(eval "${version_cmd}" 2>&1 | head -n1)"
  local got
  got="$(printf '%s' "${raw}" | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n1 || true)"
  if [[ "${got}" != "${required}" ]]; then
    fail "wrong ${name} version" "${name} ${required}" "${name} ${got:-unknown}  (raw: ${raw})"
  fi
  echo "${name} ${got}"
}

vhs_v=$(check vhs "${REQUIRED_VHS}" "vhs --version")
ttyd_v=$(check ttyd "${REQUIRED_TTYD}" "ttyd --version")

if [[ "${mode}" == "pixels" ]]; then
    echo "phosphor tapes: versions OK for pixel comparison — ${vhs_v}, ${ttyd_v} (ffmpeg not checked: no PNG goes through it)"
    exit 0
fi

ffmpeg_v=$(check ffmpeg "${REQUIRED_FFMPEG}" "ffmpeg -version")

echo "phosphor tapes: versions OK — ${vhs_v}, ${ttyd_v}, ${ffmpeg_v}"
