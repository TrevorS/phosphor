# fuzz — the parsers that read bytes we did not write

Six cargo-fuzz targets. The test each one had to pass to exist here is narrow
and worth restating, because it is what keeps this directory from becoming six
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
| `csv_parse` | a delimited file, delimiter and all | the parse and the column model together, against bytes no writer produced |
| `lsp_wire` | a language server's stdout | the only input here written by **a program we did not write**; nothing generated reaches `lsp.rs`'s framing or its decode path today |

`csv_parse` was missing from this table — and from the count above, which read
"four" — from the commit that added it (`0c12f68`) until this one, while it was
declared, sourced, seeded and running the whole time. The lint checks that a
target exists; nothing checks that this file mentions it, which is worth
knowing before trusting a count in prose.

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
`parse_seq` reads as a short sequence, the six shipped `.theme` files copied
verbatim, and the CSV fixtures `tests/csv.rs` already asserts against. `lsp_wire`
has no file to derive from — a language server's stdout is not in this repo — so
its real half is derived from the *types* instead: every body is a real
`lsp_types` value serialized by the same `serde` impls `async-lsp` writes with,
which moves with the protocol crate the same way a journal seed moves with the
journal writer. Regenerate with `scripts/fuzz.sh seed`. A hand-built blob would
rot silently the first time a format moved, and the fuzzer would keep reporting
green over bytes it could no longer parse.

Two targets keep hand-written seeds as well, and both are the same case:
`EXTRA_CSV_SEEDS` and `EXTRA_LSP_SEEDS` are *malformations*, and a malformation
is precisely the shape no serializer produces. They are short enough to read and
each is named for the question it asks.

## What running them found

Three findings. The first two are in `phosphor-core`, which is `spine`'s, and
are filed as requests rather than fixed here; the third is in
`phosphor-buffer`'s LSP client and is fixed. Each target's header carries the
full diagnosis.

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

3. **`column_from_utf16` overflowed on a position a server can legally send.**
   `Position::character` is a `u32`, so `4294967295` deserialises, and the
   *past the end* case carried the excess through as `column + (character -
   units)` — `1 + u32::MAX` against any all-BMP line, `""` included. `attempt to
   add with overflow` on the LSP task in every build with overflow checks on:
   the editor keeps running and stops receiving diagnostics from that server,
   with nothing said about why. One `publishDiagnostics` frame does it.
   **Fixed** — the addition saturates —
   `crates/phosphor-buffer/src/lsp.rs::column_from_utf16`, pinned by
   `lsp::tests::a_wire_position_at_the_u32_ceiling_does_not_overflow`, with
   `seeds/lsp_wire/diagnostics-ceiling` keeping the reproducer in the corpus.
   The same shape was hardened in `utf16_from_column`, which the wire does not
   feed.

`journal_open` (1.3M runs) and `theme_load` (10.5M runs) found nothing.

`lsp_wire` reached the third finding **from its own seed corpus**, before
libFuzzer had mutated anything — which is the argument for seeding written out
as a result: the fix's absence was proven by reverting it and watching the run
die on `seeds/lsp_wire/diagnostics-ceiling` in under a second. With the fix in,
5.28M runs in 601 seconds found nothing further, and no law tripped: not the
read-size invariance, not the desync check against `async-lsp`'s own framing
rules, not decode totality.
