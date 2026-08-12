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
set -euo pipefail

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
ffmpeg_v=$(check ffmpeg "${REQUIRED_FFMPEG}" "ffmpeg -version")

echo "phosphor tapes: versions OK — ${vhs_v}, ${ttyd_v}, ${ffmpeg_v}"
