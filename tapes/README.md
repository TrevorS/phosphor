# Tapes — the Tier-2 verification harness

VHS-captured PTY recordings: real terminal escape sequences → PNG frames / GIFs,
compared against committed references. Proves what actually appeared on screen,
which Tier-1 snapshot tests (ratatui `TestBackend`) structurally cannot — see
`docs/TASKS.md`'s "three verification tiers" for the full split.

**No real tapes exist yet.** `V002`–`V005` (Window B) add the first ones and the
per-screen convention (`tapes/<id>.tape` → `artifacts/<id>.png`) they follow.
This phase (`V001`) is the harness those tapes will run inside: pinned tool
versions, the version gate, and the `just tapes` entry point.

## Pinned versions

Pixel comparison is only meaningful against a fixed renderer — a font
substitution, a `ttyd` rendering change, or a `vhs` release that reflows
padding all produce a diff that means nothing. So every tool in the capture
pipeline is pinned by exact version, and `just tapes` refuses to run against
anything else.

| tool | pinned version | checked by |
|---|---|---|
| `vhs` | `0.11.0` | `tapes/check-versions.sh` |
| `ttyd` | `1.7.7` | `tapes/check-versions.sh` |
| `ffmpeg` | `8.1.2` | `tapes/check-versions.sh` |

`ttyd`/`ffmpeg` are pinned to what was actually installed and verified working
on the reference machine below, per SPIKES.md's instruction to pin what we
have rather than an arbitrary version. Bumping any of the three is a
deliberate, one-line edit to `tapes/check-versions.sh`, done alongside
re-regenerating every reference image — never a silent drift.

## Reference-regeneration machine

References (once V002+ exist) are only comparable to a fresh capture if both
came off the same renderer. Regenerate them only on hardware matching this
profile, and update this record — not just the images — the day that changes:

- **Model:** Mac mini (Mac16,10), Apple M4, arm64
- **OS:** macOS 26.5 (build 25F71), Darwin 25.5.0
- **Toolchain:** whatever `rust-toolchain.toml` pins at the root (currently
  `1.93.1`) — the binary being captured must be built the same way every time,
  per SPIKES.md's reasoning for pinning it at all

## Font

VHS's own built-in default (unpinned) is a ten-font CSS fallback chain headed
by `JetBrains Mono` (`vhs.go`, `defaultFontFamily`: `JetBrains Mono, DejaVu
Sans Mono, Menlo, Bitstream Vera Sans Mono, Inconsolata, Roboto Mono, Hack,
Consolas, ui-monospace, monospace` + an `Apple Symbols` fallback). That is
exactly the nondeterminism this file exists to remove: neither of the first
two fonts is installed on the reference machine above, so the *unpinned*
default silently resolves to `Menlo` here today, and would silently resolve to
`JetBrains Mono` on any machine that happens to have it — same tape, different
pixels, no diff that explains why.

**Pinned font: `Menlo`**, at whatever size `V002`'s column calibration lands
on. Menlo ships with every Mac (`/System/Library/Fonts/Menlo.ttc` — confirmed
present on the reference machine, zero install cost) and is what the unpinned
default already falls through to here, so pinning it costs nothing and matches
today's captures if any exist before `_config.tape` is written explicitly.

This becomes a `Set FontFamily "Menlo"` line in `tapes/_config.tape` (`V003`,
Window B — `Source`d by every tape). Recorded here first because V001's brief
is where the pin belongs; V003 is where it becomes code.

## Convention: every tape gets a `Require`

Every `.tape` file must open with a `Require <program>` line for each external
binary the tape's *content* drives — most obviously `Require phosphor`, and
anything else a tape shells out to. VHS's own `Require` command
(`vhs new`'s generated header) checks the program is on `$PATH` before running
a single frame and fails the tape immediately if it's missing, rather than
producing an empty or silently-wrong recording that only fails a pixel diff
much later. `vhs`, `ttyd`, and `ffmpeg` themselves are not `Require`d per-tape
— they're checked once, by version, before any tape runs (see `just tapes`
below) — because a tape doesn't invoke them by name, `vhs` invokes the tape.

## Layout

```
tapes/
  README.md            this file
  check-versions.sh     the version gate `just tapes` runs first
  run-tapes.sh           runs every tapes/*.tape (currently none)
```

`_`-prefixed files (`_config.tape`, `_dimensions.tape`, …) are `V002`/`V003`
additions: shared fragments other tapes `Source`, not standalone recordings.
`run-tapes.sh` already skips them by convention, ahead of any of them existing.

## Running

```
just tapes
```

Version-checks `vhs`, `ttyd`, and `ffmpeg` first; fails loudly and legibly if
any is missing or the wrong version (see `tapes/check-versions.sh` — the
message names what's expected and what was found). Once versions check out,
records every `tapes/*.tape`. Against today's empty library that's a quiet,
successful no-op — there is nothing to regenerate until Window B.
