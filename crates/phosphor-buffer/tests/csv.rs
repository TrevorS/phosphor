//! `T082`'s acceptance test: what [`phosphor_buffer::csv`] reads, case by case.
//!
//! The readable half. `tests/csv_properties.rs` states the laws over generated
//! tables and `fuzz/fuzz_targets/csv_parse.rs` states them over arbitrary
//! bytes; this file names the shapes a real CSV actually arrives in, and every
//! **malformed** row of the module header's table appears here as a test whose
//! name is the reading it fixes. That is deliberate: the readings are
//! judgement calls, so each one is a line somebody can disagree with in review
//! rather than a behaviour buried in a state machine.
//!
//! The fixtures under `fixtures/csv/` are the same files `scripts/fuzz.sh seed`
//! turns into the fuzz corpus, so a case added here is a seed there.

use phosphor_buffer::csv::{Delimiter, Table, parse, sniff, write_field};

/// The fields of a table, as owned strings — what nearly every assertion below
/// compares against.
fn values(table: &Table<'_>) -> Vec<Vec<String>> {
    table
        .rows()
        .iter()
        .map(|row| row.values().map(str::to_owned).collect())
        .collect()
}

fn comma(source: &str) -> Vec<Vec<String>> {
    values(&parse(source, Delimiter::COMMA))
}

// ---------------------------------------------------------------------------
// RFC 4180, the parts real files use
// ---------------------------------------------------------------------------

#[test]
fn the_rfc_example_parses_field_for_field() {
    let source = include_str!("fixtures/csv/rfc4180.csv");
    let table = parse(source, Delimiter::COMMA);
    assert_eq!(table.rows().len(), 4);
    assert_eq!(table.columns(), 5);
    assert_eq!(
        values(&table)[1],
        ["1997", "Ford", "E350", "ac, abs, moon", "3000.00"],
        "an embedded comma inside quotes is one field, not three"
    );
    assert_eq!(
        values(&table)[2][2],
        r#"Venture "Extended Edition""#,
        r#"`""` is one quote of data"#
    );
    assert_eq!(values(&table)[2][3], "", "a quoted empty field is empty");
    assert_eq!(
        values(&table)[3][3],
        "MUST SELL!\r\nair, moon roof, loaded",
        "a newline inside quotes stays inside the field"
    );
}

#[test]
fn embedded_newlines_and_escaped_quotes_survive_lf_terminators() {
    let source = include_str!("fixtures/csv/embedded.csv");
    assert_eq!(
        comma(source),
        vec![
            vec!["id", "note"],
            vec!["1", "line one\nline two"],
            vec!["2", r#"he said "hello""#],
            vec!["3", "plain"],
        ]
    );
}

#[test]
fn crlf_and_lf_may_be_mixed_in_one_file() {
    assert_eq!(
        comma("a,b\r\nc,d\ne,f\r\n"),
        vec![vec!["a", "b"], vec!["c", "d"], vec!["e", "f"]]
    );
}

/// A lone `\r` is data — the module header's reading. Splitting on it would cut
/// a field in half in every file that ever passed through a Mac Classic export
/// or a mangled paste, and the halves would then be silently misaligned.
#[test]
fn a_lone_carriage_return_is_data() {
    assert_eq!(comma("a\rb,c\n"), vec![vec!["a\rb", "c"]]);
}

// ---------------------------------------------------------------------------
// Terminators, and the difference one newline makes
// ---------------------------------------------------------------------------

#[test]
fn a_trailing_newline_does_not_add_an_empty_record() {
    assert_eq!(comma("a,b\n"), vec![vec!["a", "b"]]);
    assert_eq!(comma("a,b"), vec![vec!["a", "b"]]);
}

#[test]
fn a_blank_line_is_a_record_holding_one_empty_field() {
    assert_eq!(comma("a\n\nb\n"), vec![vec!["a"], vec![""], vec!["b"]]);
    assert_eq!(comma("\n"), vec![vec![""]]);
}

#[test]
fn an_empty_file_has_no_rows_at_all() {
    let table = parse(include_str!("fixtures/csv/empty.csv"), Delimiter::COMMA);
    assert!(table.is_empty());
    assert_eq!(table.columns(), 0);
    assert!(parse("", Delimiter::COMMA).is_empty());
}

#[test]
fn one_byte_is_one_row_of_one_field() {
    assert_eq!(
        comma(include_str!("fixtures/csv/one-byte.csv")),
        vec![vec!["a"]]
    );
}

#[test]
fn a_trailing_delimiter_leaves_an_empty_field_behind_it() {
    assert_eq!(comma("a,\n"), vec![vec!["a", ""]]);
    assert_eq!(comma(",\n"), vec![vec!["", ""]]);
}

// ---------------------------------------------------------------------------
// Ragged rows and whitespace
// ---------------------------------------------------------------------------

#[test]
fn ragged_rows_are_kept_ragged_and_columns_is_the_widest() {
    let table = parse(include_str!("fixtures/csv/ragged.csv"), Delimiter::COMMA);
    assert_eq!(
        values(&table),
        vec![
            vec!["a", "b", "c"],
            vec!["1"],
            vec![""],
            vec!["2", "3", "4", "5", "6"],
            vec!["7", "8"],
        ]
    );
    assert_eq!(table.columns(), 5, "columns is a maximum, not a schema");
}

/// Leading and trailing spaces are part of the field, quoted or not. RFC 4180
/// §2.4 says so, and the alternative — trimming — would change what the file
/// says while claiming to be a renderer.
#[test]
fn a_whitespace_only_field_keeps_its_whitespace() {
    assert_eq!(comma(r#"" ",   ,"#), vec![vec![" ", "   ", ""]]);
}

// ---------------------------------------------------------------------------
// Malformed — one test per row of the module header's table
// ---------------------------------------------------------------------------

#[test]
fn a_quote_inside_an_unquoted_field_is_data() {
    assert_eq!(comma(r#"a"b,plain"#), vec![vec![r#"a"b"#, "plain"]]);
}

#[test]
fn text_after_a_closing_quote_joins_the_field() {
    assert_eq!(comma(r#""ab"cd,tail"#), vec![vec!["abcd", "tail"]]);
}

#[test]
fn an_unterminated_quote_runs_to_the_end_of_the_file() {
    assert_eq!(
        comma("a,\"never closed\nand the rest"),
        vec![vec!["a", "never closed\nand the rest"]],
        "the state of every quoted field while it is being typed"
    );
}

#[test]
fn a_byte_order_mark_stays_in_the_field_it_is_in() {
    let table = parse("\u{feff}id,name\n", Delimiter::COMMA);
    assert_eq!(values(&table)[0][0], "\u{feff}id");
    assert_eq!(
        table.rows()[0].fields()[0].span(),
        0..5,
        "the span counts the BOM's three bytes, so an edit indexed by it lands"
    );
}

#[test]
fn the_malformed_fixture_parses_rather_than_refusing() {
    let table = parse(include_str!("fixtures/csv/malformed.csv"), Delimiter::COMMA);
    assert_eq!(table.rows().len(), 5);
    assert_eq!(values(&table)[0], [r#"a"b"#, "plain"]);
    assert_eq!(values(&table)[1], ["abcd", "tail"]);
    assert_eq!(values(&table)[2], [" ", "   "]);
    assert_eq!(values(&table)[3], ["\u{feff}bom", "yes"]);
    assert_eq!(values(&table)[4], ["never closed,and the rest of the file"]);
}

// ---------------------------------------------------------------------------
// Delimiters that are not commas
// ---------------------------------------------------------------------------

#[test]
fn a_tab_delimited_file_is_the_same_surface() {
    let source = include_str!("fixtures/csv/people.tsv");
    let table = parse(source, Delimiter::TAB);
    assert_eq!(
        values(&table),
        vec![
            vec!["name", "role", "notes"],
            vec!["ada", "analyst", "tab\tinside"],
            vec!["grace", "admiral", ""],
        ]
    );
}

#[test]
fn a_comma_is_data_under_a_semicolon_delimiter() {
    assert_eq!(
        values(&parse("a,b;c\n", Delimiter::SEMICOLON)),
        vec![vec!["a,b", "c"]]
    );
}

#[test]
fn a_delimiter_that_would_make_the_grammar_ambiguous_is_refused() {
    assert_eq!(Delimiter::new(b'"'), None);
    assert_eq!(Delimiter::new(b'\n'), None);
    assert_eq!(Delimiter::new(b'\r'), None);
    assert_eq!(
        Delimiter::new(0xff),
        None,
        "a byte no ASCII split can land on"
    );
    assert_eq!(Delimiter::new(b','), Some(Delimiter::COMMA));
}

#[test]
fn sniff_picks_the_byte_that_splits_the_header_widest() {
    assert_eq!(sniff("a,b,c\n1,2,3\n"), Delimiter::COMMA);
    assert_eq!(sniff("a\tb\tc\n"), Delimiter::TAB);
    assert_eq!(sniff("a;b;c;d\n"), Delimiter::SEMICOLON);
    assert_eq!(sniff("a|b|c\n"), Delimiter::PIPE);
    assert_eq!(sniff(""), Delimiter::COMMA, "nothing to go on");
    assert_eq!(
        sniff("no delimiters here\n"),
        Delimiter::COMMA,
        "one field under every candidate — the tie goes to the commonest"
    );
}

/// `sniff` reads the **first record and stops**, which its doc claims and its
/// body did not do for a window: it ran four whole-file parses and threw all
/// but the header away.
///
/// The header wins even when every later record would vote the other way. That
/// is the rule ("a header row is the one line guaranteed to have every
/// column") and it is also the only way to observe the early exit from outside.
#[test]
fn sniff_reads_the_first_record_and_stops() {
    assert_eq!(
        sniff("a,b\n1;2;3;4;5\n6;7;8;9;10\n"),
        Delimiter::COMMA,
        "the header splits widest on `,`; the body's semicolons are not counted"
    );
    assert_eq!(
        sniff("h1;h2;h3\nx,y\n"),
        Delimiter::SEMICOLON,
        "and the reverse — nothing about the answer depends on record two"
    );
}

/// A record is not a line: a quoted field carries its own newlines, so the
/// first *record* of `"a\nb",c` is two fields and not one.
#[test]
fn sniff_counts_a_record_not_a_line() {
    assert_eq!(sniff("\"a\nb\",c,d\ne;f;g;h\n"), Delimiter::COMMA);
}

/// An unterminated quote means the first record runs to end of input — the
/// state every CSV is in while somebody is typing. `sniff` must answer rather
/// than loop or panic.
#[test]
fn sniff_answers_for_a_file_that_is_one_open_quote() {
    assert_eq!(sniff("\"abc"), Delimiter::COMMA);
    assert_eq!(
        sniff("\"abc\ndef;g;h"),
        Delimiter::COMMA,
        "one field, no vote"
    );
}

// ---------------------------------------------------------------------------
// Spans — what an aligning edit indexes with
// ---------------------------------------------------------------------------

#[test]
fn a_field_span_covers_its_raw_text_quotes_included() {
    let source = r#"a,"b,c",d"#;
    let table = parse(source, Delimiter::COMMA);
    let fields = table.rows()[0].fields();
    assert_eq!(fields[0].span(), 0..1);
    assert_eq!(&source[fields[1].span()], r#""b,c""#);
    assert_eq!(fields[1].value(), "b,c");
    // The raw text is wider than the value exactly when quotes were removed —
    // the arithmetic an inline aligner has to get the right way round
    // (`Field::span`'s doc).
    assert_eq!(fields[1].span().len(), fields[1].value().len() + 2);
    assert_eq!(fields[0].span().len(), fields[0].value().len());
    assert_eq!(fields[2].span(), 8..9);
}

#[test]
fn a_row_span_stops_before_its_terminator() {
    let source = "ab,cd\r\nef\n";
    let table = parse(source, Delimiter::COMMA);
    assert_eq!(&source[table.rows()[0].span()], "ab,cd");
    assert_eq!(&source[table.rows()[1].span()], "ef");
}

// ---------------------------------------------------------------------------
// The writer, and the round trip it makes statable
// ---------------------------------------------------------------------------

#[test]
fn a_field_is_quoted_exactly_when_it_would_re_parse_as_structure() {
    let mut out = String::new();
    for (value, expected) in [
        ("plain", "plain"),
        ("", ""),
        ("  ", "  "),
        ("a,b", r#""a,b""#),
        (r#"q"q"#, r#""q""q""#),
        ("two\nlines", "\"two\nlines\""),
        ("cr\r", "\"cr\r\""),
    ] {
        out.clear();
        write_field(&mut out, value, Delimiter::COMMA);
        assert_eq!(out, expected, "writing {value:?}");
    }
}

#[test]
fn a_tab_needs_no_quotes_under_a_comma_and_does_under_a_tab() {
    let mut out = String::new();
    write_field(&mut out, "a\tb", Delimiter::COMMA);
    assert_eq!(out, "a\tb");
    out.clear();
    write_field(&mut out, "a\tb", Delimiter::TAB);
    assert_eq!(out, "\"a\tb\"");
}

#[test]
fn every_fixture_round_trips_through_the_writer() {
    for (name, source, delimiter) in [
        (
            "rfc4180.csv",
            include_str!("fixtures/csv/rfc4180.csv"),
            Delimiter::COMMA,
        ),
        (
            "embedded.csv",
            include_str!("fixtures/csv/embedded.csv"),
            Delimiter::COMMA,
        ),
        (
            "ragged.csv",
            include_str!("fixtures/csv/ragged.csv"),
            Delimiter::COMMA,
        ),
        (
            "wide.csv",
            include_str!("fixtures/csv/wide.csv"),
            Delimiter::COMMA,
        ),
        (
            "malformed.csv",
            include_str!("fixtures/csv/malformed.csv"),
            Delimiter::COMMA,
        ),
        (
            "people.tsv",
            include_str!("fixtures/csv/people.tsv"),
            Delimiter::TAB,
        ),
        (
            "empty.csv",
            include_str!("fixtures/csv/empty.csv"),
            Delimiter::COMMA,
        ),
        (
            "one-byte.csv",
            include_str!("fixtures/csv/one-byte.csv"),
            Delimiter::COMMA,
        ),
    ] {
        let once = parse(source, delimiter);
        let spelled = once.to_csv(delimiter);
        let twice = parse(&spelled, delimiter);
        assert_eq!(
            values(&once),
            values(&twice),
            "{name} does not survive a write and a re-read"
        );
        assert_eq!(
            spelled,
            twice.to_csv(delimiter),
            "{name}'s written form is not a fixed point"
        );
    }
}
