# fuzz — the parsers that read bytes we did not write

Four cargo-fuzz targets. The test each one had to pass to exist here is narrow
and worth restating, because it is what keeps this directory from becoming four
corpora nobody looks at:

> **Does this code read bytes chosen by someone other than us?**

A fuzzer over our own values is a slower property test. `proptest` already
covers that ground — see `crates/phosphor-core/tests/properties.rs` and
`crates/phosphor-buffer/tests/undo_properties.rs`, which state the laws
`value.rs`, `input/text.rs`, `journal.rs` and the undo tree obey over generated
input. Each target below names what it adds over those, and each names the law
it asserts rather than the lines it colours.

## Running it

```
scripts/fuzz.sh                  # what is here, and how big each corpus is
scripts/fuzz.sh seed             # regenerate seeds/ from real repo files
scripts/fuzz.sh build            # compile every target, run none
scripts/fuzz.sh journal_open     # run until you stop it
scripts/fuzz.sh journal_open 60  # run for 60 seconds
```

**This is not in `just gate`, and must not be.** Fuzzing needs nightly for
`-Zsanitizer` while `/rust-toolchain.toml` pins 1.97.1 for tape determinism; and
a search whose answer is "nothing yet", which gets slower the longer it is
right, has no business failing a build. It is the same call `just coverage` and
`just tapes-diff` already carry. What CI *does* check is structure, in
`scripts/lint-fuzz-targets.sh`: every target declared, sourced, seeded, and
`fuzz/` excluded from the workspace.

`fuzz/rust-toolchain.toml` selects nightly for this directory alone. Nothing at
the repo root sees it — `cd fuzz` is what selects it, which is why the runner
does exactly that.

## The targets

| Target | Reads | Adds over the property suite |
|---|---|---|
| `journal_open` | a journal file, from byte zero | the property test always builds a *valid* header and cuts only in the frames region; this hands the reader bytes that were never a journal |
| `journal_records` | record payloads, framed by the real writer | the deep half — `decode` and `History::apply` — which raw file bytes cannot reach, because coverage feedback cannot solve a CRC-32 |
| `key_notation` | `runtime/*.scm` notation, and `.`'s replay | nothing generated reaches `input/key.rs` today |
| `theme_load` | a user's base16 `.theme` file | nothing generated reaches `phosphor-ui`'s theme loader today |

Two targets were considered and one was dropped:

- **`Value` / `Wire::from_value`** — dropped, and this is a judgement to revisit
  rather than a permanent one. The reasoning: *today* nothing turns bytes into a
  `Value`. The MCP door is `T052` and unbuilt — `crates/phosphor-core/src/registry/mcp.rs`
  emits a schema *description* and its header says `T052` is what owns `rmcp` and
  `serde`. The CLI door converts a clap `ArgMatches`, and it lives in
  `crates/phosphor/src/door.rs`, a binary crate a fuzz target cannot link.
  Meanwhile `properties.rs`'s `any_value_is_decoded_or_refused` and
  `vocabulary_types_round_trip` already state decode-or-refuse and round-trip
  identity over recursively generated `Value`s. So the fuzzer would be a slower
  property test against a door that does not exist. **Add it when `T052` lands**,
  pointed at the JSON boundary rather than at `Value`.

## Seeds, and why they are generated

`seeds/` is tracked; `corpus/` and `artifacts/` are not (`.gitignore` says why).
`scripts/fuzz.sh` copies the seeds into the corpus before every run.

The seeds are *derived*, by `examples/seed.rs`, from files this repo already has:
journals written by the real writer, the string literals in `runtime/*.scm` that
`parse_seq` reads as a short sequence, and the six shipped `.theme` files copied
verbatim. Regenerate with `scripts/fuzz.sh seed`. A hand-built blob would rot
silently the first time a format moved, and the fuzzer would keep reporting green
over bytes it could no longer parse.

## What running them found

Both findings are in `phosphor-core`, which is `spine`'s; each target's header
carries the full diagnosis, and each is filed as a request rather than fixed
here.

1. **`Decoder::u64` accepts non-canonical LEB128**, so the journal codec is not
   injective. Payload `[5, 17, 188, 0]` decodes to `Record::Redo { node: 17,
   child: 60 }`, which encodes to `[5, 17, 60]` — `0xbc 0x00` is `60` written
   with a redundant continuation byte. This makes
   `properties.rs::arbitrary_bytes_decode_or_refuse`'s stated law — *"an `Ok` is
   a record whose own encoding is those same bytes"* — **false**. No writer emits
   one, so it is a false stated law and a format malleability, not data loss.
   `journal_records` now asserts the true, weaker law (a differing re-encoding
   must be a *normalisation*: same record, and a fixed point) so it can keep
   searching, and its header records the strict law and the counterexample.

2. **`notation_of` is not the inverse of `parse_seq`**, which is the law `.`
   (dot-repeat) rides on. Four ASCII characters:
   `parse_seq("< a>")` is three keys, `notation_of` spells them `"<a<>>"`, and
   that reads back as five. `unambiguous` has an arm for `<` (`"<lt>"`) and none
   for `>`, so a bare `>` becomes `"<>>"` — whose inner `>` an *earlier* unclosed
   `<` can claim — and `notation_of` never re-parses after substituting the
   unambiguous spelling, so the wrong fallback is not caught.
   **`key_notation` reproduces this in seconds and is deliberately not weakened
   to get past it.**

`journal_open` (1.3M runs) and `theme_load` (10.5M runs) found nothing.
