//! `T082`'s laws, stated over generated tables rather than the fixtures.
//!
//! `tests/csv.rs` is the acceptance test and stays the readable one: it names a
//! shape a real file arrives in and asserts what it reads as. This file asks
//! the same parser about tables nobody chose, because the failures in a
//! delimited-text parser are at the boundaries *between* fields — a value that
//! is itself the delimiter, a value that ends in a quote, a value that is a
//! bare `\r` in front of a record break — and a generator that emits `[a-z]+`
//! reaches none of them.
//!
//! So [`field`] emits those on purpose. Two of this build's property suites
//! have already been bitten by a generator too tame to reach the bug; the
//! alphabet below is the response, and every entry in it is a case that
//! changes which branch of `quoted_field_at` runs.
//!
//! # Four laws
//!
//! 1. **The writer is the parser's inverse.** For any table, `to_csv` then
//!    `parse` is the identity on values. This is the law the whole module rests
//!    on and the one that makes `write_field`'s quoting rule checkable rather
//!    than argued about.
//! 2. **The written form is a fixed point.** Writing what was just read
//!    produces the same bytes again — so an aligning edit that re-writes a file
//!    cannot make it drift a byte per save.
//! 3. **Spans locate what they claim to.** Every field's span is inside the
//!    source, spans run forward and never overlap, and re-parsing the slice a
//!    span names gives back that field's value. The spans are what an edit and
//!    an inline virtual-text run index with, so a span that is off by one is a
//!    padding run in the middle of a field.
//! 4. **The writer spells RFC 4180.** Laws 1–3 read what [`spell`] wrote, so
//!    none of them can see [`write_field`] at all; this compares the two,
//!    field by field. Without it the quoting rule is checked only against
//!    itself — which is what this file did for a window, because `spell`
//!    called `write_field`.
//!
//! Plus totality, over strings nobody designed: [`parse`] answers for every
//! input. `fuzz/fuzz_targets/csv_parse.rs` states that one over arbitrary
//! *bytes* and for as long as somebody runs it; here it is a cheap gate that
//! runs on every `just test`.
//!
//! `proptest` is `SPIKES.md`'s hygiene choice for exactly this shape of
//! question.

use phosphor_buffer::csv::{Delimiter, parse, write_field};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// The delimiters worth generating: the four named constants, because those
/// are the four a real file uses and each takes a different branch of
/// `write_field`'s quoting rule for the same value.
fn delimiter() -> impl Strategy<Value = Delimiter> {
    prop_oneof![
        Just(Delimiter::COMMA),
        Just(Delimiter::TAB),
        Just(Delimiter::SEMICOLON),
        Just(Delimiter::PIPE),
    ]
}

/// One field, from an alphabet chosen to land on boundaries.
///
/// The `".{0,6}"` arm is the ordinary text; everything else exists because it
/// is a case the parser branches on. The quote-adjacent ones matter most: a
/// value ending in `"` is written as `…""` and its closing quote is then the
/// third of three in a row, which is the shape that breaks a scanner that
/// looks two bytes ahead instead of consuming pairs.
fn field() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-z0-9 ]{0,8}",
        2 => ".{0,6}",
        1 => Just(String::new()),
        1 => Just("   ".to_owned()),
        1 => prop_oneof![
            Just(",".to_owned()),
            Just("\t".to_owned()),
            Just(";".to_owned()),
            Just("|".to_owned()),
        ],
        1 => prop_oneof![
            Just("\"".to_owned()),
            Just("\"\"".to_owned()),
            Just("a\"".to_owned()),
            Just("\"a".to_owned()),
            Just("he said \"hi\"".to_owned()),
        ],
        1 => prop_oneof![
            Just("\n".to_owned()),
            Just("\r".to_owned()),
            Just("\r\n".to_owned()),
            Just("one\ntwo".to_owned()),
            Just("ends\r".to_owned()),
        ],
        1 => prop_oneof![
            Just("名前".to_owned()),
            Just("アリス".to_owned()),
            Just("🙂🙂".to_owned()),
            Just("e\u{0301}".to_owned()),
            Just("\u{feff}bom".to_owned()),
        ],
    ]
}

/// A table: at least one row, each row at least one field.
///
/// Both minimums are the parser's own invariant rather than a convenience — an
/// empty line parses to one empty field, so a row with no fields is not
/// something [`parse`] can produce and not something the writer has to spell.
fn table() -> impl Strategy<Value = Vec<Vec<String>>> {
    prop::collection::vec(prop::collection::vec(field(), 1..5), 1..8)
}

/// The generated table written out — RFC 4180, spelled here.
///
/// Written here rather than borrowed from the crate on purpose: a round-trip
/// law checked against the crate's own writer would pass for a parser and a
/// writer that agreed with each other and disagreed with RFC 4180.
///
/// **Including the quoting rule**, which is the part the RFC actually
/// constrains and the part this file borrowed from `write_field` for a window —
/// so laws 1–3 could not have distinguished a wrong quoting rule that the
/// parser happened to agree with. §2.5–2.7: a field carrying the delimiter, a
/// quote, CR or LF is enclosed in quotes and each quote inside it is doubled;
/// nothing else is quoted.
fn spell(table: &[Vec<String>], delimiter: Delimiter) -> String {
    let mut out = String::new();
    for row in table {
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                out.push(delimiter.as_char());
            }
            out.push_str(&spell_field(value, delimiter));
        }
        out.push('\n');
    }
    out
}

/// One field, RFC 4180 §2.5–2.7.
fn spell_field(value: &str, delimiter: Delimiter) -> String {
    if value.contains([delimiter.as_char(), '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn values(source: &str, delimiter: Delimiter) -> Vec<Vec<String>> {
    parse(source, delimiter)
        .rows()
        .iter()
        .map(|row| row.values().map(str::to_owned).collect())
        .collect()
}

// ---------------------------------------------------------------------------
// The laws
// ---------------------------------------------------------------------------

proptest! {
    /// Law 1 — the writer is the parser's inverse.
    #[test]
    fn a_written_table_reads_back_as_itself(
        table in table(),
        delimiter in delimiter(),
    ) {
        let spelled = spell(&table, delimiter);
        prop_assert_eq!(values(&spelled, delimiter), table);
    }

    /// Law 2 — the written form is a fixed point, so a file that is read and
    /// written does not drift.
    #[test]
    fn writing_what_was_read_gives_the_same_bytes(
        table in table(),
        delimiter in delimiter(),
    ) {
        let spelled = spell(&table, delimiter);
        let parsed = parse(&spelled, delimiter);
        prop_assert_eq!(parsed.to_csv(delimiter), spelled);
    }

    /// Law 3 — spans locate what they claim to.
    ///
    /// Ordered, non-overlapping, inside the source, and each one re-parses to
    /// its own field. The last part is the one with teeth: a quoted field's
    /// span includes its quotes, so slicing it and parsing the slice has to
    /// unescape to the same value.
    #[test]
    fn every_span_is_inside_the_source_and_re_parses_to_its_field(
        table in table(),
        delimiter in delimiter(),
    ) {
        let source = spell(&table, delimiter);
        let parsed = parse(&source, delimiter);
        let mut previous_end = 0;
        for row in parsed.rows() {
            prop_assert!(row.span().start >= previous_end);
            prop_assert!(row.span().end <= source.len());
            for cell in row.fields() {
                let span = cell.span();
                prop_assert!(span.start >= previous_end, "spans run backwards");
                prop_assert!(span.end <= source.len(), "a span points past the file");
                prop_assert!(span.start <= span.end, "an inverted span");
                let re_read = parse(&source[span.clone()], delimiter);
                prop_assert_eq!(
                    re_read
                        .rows()
                        .first()
                        .and_then(|r| r.fields().first())
                        .map_or("", |f| f.value()),
                    cell.value(),
                    "the slice a span names is not the field it came from"
                );
                previous_end = span.end;
            }
            previous_end = row.span().end;
        }
    }

    /// Law 4 — the writer spells RFC 4180 and not a private dialect.
    ///
    /// Laws 1–3 all run over [`spell`]'s output, so they say what the *parser*
    /// does with correct CSV. This is the only assertion in the file that looks
    /// at [`write_field`] itself, and it is why the quoting rule above is
    /// written out longhand: a writer that quoted every field, or none, would
    /// still round-trip through this parser and would still be wrong.
    #[test]
    fn write_field_quotes_exactly_what_rfc_4180_quotes(
        value in field(),
        delimiter in delimiter(),
    ) {
        let mut written = String::new();
        write_field(&mut written, &value, delimiter);
        prop_assert_eq!(written, spell_field(&value, delimiter));
    }

    /// Totality — every string is a table, including strings nobody designed.
    ///
    /// `.{0,200}` reaches unbalanced quotes, bare terminators and every arm of
    /// [`field`] in combination. What is asserted is that the answer is
    /// *self-consistent*, because for malformed input there is no second
    /// opinion to compare against — and self-consistency is not nothing: a
    /// lenient reading that did not survive being written out would mean the
    /// file changed the first time anything saved it.
    #[test]
    fn any_string_parses_into_rows_that_agree_with_themselves(
        source in ".{0,200}",
        delimiter in delimiter(),
    ) {
        let parsed = parse(&source, delimiter);
        prop_assert_eq!(parsed.is_empty(), source.is_empty());
        for row in parsed.rows() {
            prop_assert!(!row.fields().is_empty(), "a row with no fields");
            prop_assert!(row.span().end <= source.len());
        }
        // And what it read is stable: writing it out and reading it back gives
        // the same values, whatever lenient reading produced them, and writing
        // *that* changes no byte. Without this, a malformed file would mean one
        // thing on open and another after the first save.
        let spelled = parsed.to_csv(delimiter);
        let again = parse(&spelled, delimiter);
        let before: Vec<Vec<String>> = parsed
            .rows()
            .iter()
            .map(|row| row.values().map(str::to_owned).collect())
            .collect();
        prop_assert_eq!(values(&spelled, delimiter), before);
        prop_assert_eq!(again.to_csv(delimiter), spelled);
    }
}
