//! Delimited text, parsed by hand (`T082`).
//!
//! The one first-class language with no grammar. `tree-sitter-csv` is 2.5 years
//! stale at ~5k downloads, and CSV's surface is *virtual column alignment*
//! rather than syntax highlighting — a column model, not a parse tree. A stale
//! grammar would be a dependency that cannot produce the one thing the surface
//! needs, so `runtime/languages/csv.scm` declares the language with `grammar`
//! void and this module is what stands in its place.
//!
//! Two halves, and the seam between them is a `&str`:
//!
//! * **Here** — bytes to rows of fields. No widths, no cells, no theme.
//! * **`phosphor_ui::csv`** — rows of fields to column widths in terminal
//!   cells. Display width is a rendering question and lives with the renderer,
//!   which is also the only place the width machinery already exists. Not an
//!   intra-doc link: a UI crate is not in this crate's dependency graph and
//!   must not be, which is exactly why the seam is a `&str`.
//!
//! # RFC 4180, and the file in front of you
//!
//! The RFC is the specification and real CSV violates it constantly: Excel
//! writes a BOM, exporters emit LF rather than CRLF, hand-edited files carry a
//! stray quote, and every file is malformed for as long as somebody is halfway
//! through typing a quoted field. **A text editor must render a broken CSV, not
//! refuse it**, so [`parse`] is total: it answers a [`Table`] for every input,
//! it never fails and it never panics. What "malformed" costs is a field whose
//! value is not what a stricter reader would say, and each of those readings is
//! written down below and unit-tested by name.
//!
//! | Input | What this does | Why |
//! |---|---|---|
//! | `a"b` | the quote is data | Excel writes it; a quote that does not open a field never closes one. |
//! | `"abc` (EOF) | the field runs to end of input | It is the state of every quoted field mid-keystroke. Refusing it means the surface dies while you type. |
//! | `"ab"cd` | one field, `abcd` | The alternative is discarding bytes that are in the file, and the row would then be one field short of the row above it. |
//! | ragged rows | kept ragged | The widths are a column model, not a schema. A short row simply stops early. |
//! | a UTF-8 BOM | data, in the first field | Stripping it would make every byte span after it lie by three, and the spans are what an aligning edit indexes with. |
//! | a lone `\r` | data | Terminators are LF and CRLF. A lone CR splitting a record would silently cut a field in half, which is the worse of the two wrong answers. |
//!
//! # Never panic
//!
//! Every scan below is over `source.as_bytes()` and every slice boundary is a
//! position holding an ASCII byte — the delimiter ([`Delimiter::new`] admits
//! ASCII only), `"`, `\r` or `\n`. A UTF-8 continuation byte is never one of
//! those, so a boundary computed this way is always a `char` boundary and no
//! slice here can panic. `fuzz/fuzz_targets/csv_parse.rs` is the standing proof
//! over arbitrary bytes.
//!
//! Owned by `surface`.

use std::borrow::Cow;
use std::ops::Range;

// ---------------------------------------------------------------------------
// The delimiter
// ---------------------------------------------------------------------------

/// What separates two fields on a row.
///
/// ASCII only, and never `"`, `\r` or `\n`. Not squeamishness: the parser
/// splits on byte positions, and a multi-byte delimiter would let a split land
/// inside a UTF-8 sequence, while a delimiter that is also a quote or a
/// terminator makes the grammar ambiguous with itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Delimiter(u8);

impl Delimiter {
    /// `,` — CSV.
    pub const COMMA: Self = Self(b',');
    /// `\t` — TSV, which is the same surface with a wider column.
    pub const TAB: Self = Self(b'\t');
    /// `;` — what a locale using `,` as its decimal separator exports.
    pub const SEMICOLON: Self = Self(b';');
    /// `|` — the log-file dialect.
    pub const PIPE: Self = Self(b'|');

    /// The delimiter for `byte`, or `None` if it cannot be one.
    #[must_use]
    pub const fn new(byte: u8) -> Option<Self> {
        match byte {
            b'"' | b'\r' | b'\n' => None,
            0x80..=0xff => None,
            other => Some(Self(other)),
        }
    }

    /// The byte it splits on.
    ///
    /// Private: the scan and [`write_field`] are the only things that want a
    /// byte, and both are in this module. A caller outside it wants
    /// [`Delimiter::as_char`], which is what a renderer can draw.
    const fn byte(self) -> u8 {
        self.0
    }

    /// The same byte as a `char`, for a renderer that has to draw it.
    #[must_use]
    pub const fn as_char(self) -> char {
        self.0 as char
    }
}

/// The delimiter a file most likely uses, from its first line.
///
/// A guess, deliberately: nothing in the file says which byte is the
/// delimiter, and the extension lies as often as it helps (`.csv` files
/// exported by a German Excel are semicolon-delimited). The rule is *the
/// candidate that splits the first record into the most fields*, ties going to
/// the earlier entry in [`Delimiter::COMMA`], [`Delimiter::TAB`],
/// [`Delimiter::SEMICOLON`], [`Delimiter::PIPE`] — which is the order of how
/// often each is right.
///
/// Counting on the *first record* rather than the whole file is what makes
/// this cheap on a large file and correct on a small one: a header row is the
/// one line guaranteed to have every column.
///
/// That sentence was false for a window and the cost was on the open path.
/// This called [`parse`] once per candidate and then read `.rows().first()` —
/// four whole-file parses, each allocating a `Row` per record, all but the
/// first record discarded. Measured on a 6.7 MB export it cost 6.7× the parse
/// it precedes, so the documented open sequence (`sniff` then `parse`) was five
/// parses rather than one. [`first_record_fields`] is the honest version of the
/// same rule, and `benches/csv.rs` table 0 is what keeps it honest: it asserts
/// that this cost does not move when the file grows 16×.
#[must_use]
pub fn sniff(source: &str) -> Delimiter {
    let candidates = [
        Delimiter::COMMA,
        Delimiter::TAB,
        Delimiter::SEMICOLON,
        Delimiter::PIPE,
    ];
    let mut best = Delimiter::COMMA;
    let mut best_fields = 0;
    for candidate in candidates {
        let fields = first_record_fields(source, candidate);
        if fields > best_fields {
            best_fields = fields;
            best = candidate;
        }
    }
    best
}

/// How many fields `delimiter` splits the first record into.
///
/// The same scan [`parse`] runs, stopped at the first record and keeping
/// nothing: no `Vec<Row>`, no `Vec<Field>`, and no visit to byte one of record
/// two. An empty source is zero fields, which is what makes [`sniff`] answer
/// `,` for it.
fn first_record_fields(source: &str, delimiter: Delimiter) -> usize {
    if source.is_empty() {
        return 0;
    }
    let mut at = 0;
    let mut fields = 0;
    loop {
        let (_, next, stop) = field_at(source, at, delimiter);
        fields += 1;
        at = next;
        if stop != Stop::Field {
            return fields;
        }
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// One field: what it says, and where it is.
///
/// [`Field::value`] is the *unescaped* text — the quotes are gone and `""` has
/// become `"` — because that is what gets drawn and what gets measured.
/// [`Field::span`] is the **raw** byte range, quotes included, because that is
/// what an edit or an inline virtual-text run has to index with. Keeping both
/// is the whole reason this is a struct rather than a `Cow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field<'a> {
    value: Cow<'a, str>,
    span: Range<usize>,
}

impl Field<'_> {
    /// The unescaped text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The raw byte range in the source, quotes included.
    ///
    /// **The distinction an inline aligner must not miss.** A column's width is
    /// measured from the **value** (`a,b`, three cells); padding drawn inside
    /// the buffer's own line sits after the **raw** text (`"a,b"`, five cells).
    /// Getting the two the wrong way round pads a quoted field by two cells too
    /// many, and only on the rows that happen to be quoted — which looks like a
    /// rendering glitch rather than an arithmetic error. `span.len() !=
    /// value.len()` is that test, and it lived here as an `is_quoted` method
    /// with no caller outside its own unit test until a reviewer counted.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
}

/// One record.
///
/// Always at least one field: an empty line is a row holding one empty field,
/// which is what makes [`Table::to_csv`] the exact inverse of [`parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    fields: Vec<Field<'a>>,
    span: Range<usize>,
}

impl<'a> Row<'a> {
    /// The fields, in order.
    #[must_use]
    pub fn fields(&self) -> &[Field<'a>] {
        &self.fields
    }

    /// The record's byte range, terminator excluded.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// The values alone — what a column model measures.
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(Field::value)
    }
}

/// A parsed delimited file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table<'a> {
    rows: Vec<Row<'a>>,
}

impl<'a> Table<'a> {
    /// The records, in order.
    #[must_use]
    pub fn rows(&self) -> &[Row<'a>] {
        &self.rows
    }

    /// True when the source held no records at all — an empty file, and
    /// nothing else. A file holding one newline has one row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The widest row's field count.
    ///
    /// Rows are ragged by design, so this is a maximum rather than a schema:
    /// it is how many columns the surface has to lay out, not how many every
    /// row has.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.rows
            .iter()
            .map(|row| row.fields.len())
            .max()
            .unwrap_or(0)
    }

    /// The table written back out, RFC 4180 style.
    ///
    /// The inverse of [`parse`], and the reason it can be stated as a law:
    /// **every row is terminated**, including the last. Joining rows with a
    /// separator instead would make `[["a"], [""]]` and `[["a"]]` the same
    /// bytes, and a round-trip property that cannot distinguish two tables is
    /// not a round-trip property. `fuzz/fuzz_targets/csv_parse.rs` and
    /// `tests/csv_properties.rs` both rest on this.
    #[must_use]
    pub fn to_csv(&self, delimiter: Delimiter) -> String {
        let mut out = String::with_capacity(self.rows.iter().map(|row| row.span.len() + 1).sum());
        for row in &self.rows {
            for (index, field) in row.fields.iter().enumerate() {
                if index > 0 {
                    out.push(delimiter.as_char());
                }
                write_field(&mut out, field.value(), delimiter);
            }
            out.push('\n');
        }
        out
    }
}

/// Appends one field to `out`, quoted if it has to be.
///
/// Quoted exactly when the value carries the delimiter, a quote or either
/// terminator byte — the four things that would otherwise re-parse as
/// structure. Nothing else is quoted: a value that needs no quotes and gets
/// them would still round-trip, but the file would grow every time it was
/// written.
pub fn write_field(out: &mut String, value: &str, delimiter: Delimiter) {
    let needs_quotes = value
        .bytes()
        .any(|byte| byte == delimiter.byte() || byte == b'"' || byte == b'\r' || byte == b'\n');
    if !needs_quotes {
        out.push_str(value);
        return;
    }
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
}

// ---------------------------------------------------------------------------
// The parser
// ---------------------------------------------------------------------------

/// Why a field ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// A delimiter — another field follows on this row.
    Field,
    /// LF or CRLF — the record is over.
    Record,
    /// End of input.
    Eof,
}

/// Reads `source` as delimited text.
///
/// Total: every input is a table. See the module header for what each
/// malformed shape reads as and why.
#[must_use]
pub fn parse(source: &str, delimiter: Delimiter) -> Table<'_> {
    let mut rows = Vec::new();
    if source.is_empty() {
        return Table { rows };
    }
    let mut at = 0;
    loop {
        let row_start = at;
        let mut fields = Vec::new();
        let stop = loop {
            let (field, next, stop) = field_at(source, at, delimiter);
            fields.push(field);
            at = next;
            if stop != Stop::Field {
                break stop;
            }
        };
        let text_end = fields.last().map_or(row_start, |field| field.span.end);
        rows.push(Row {
            fields,
            span: row_start..text_end,
        });
        // A terminator at the very end of the file closes the last record; it
        // does not open an empty one. That is the difference between `a\n`
        // (one row) and `a\n\n` (two).
        if stop == Stop::Eof || at >= source.len() {
            break;
        }
    }
    Table { rows }
}

/// One field, starting at `at`.
///
/// Returns the field, the offset the next one starts at (past the delimiter or
/// terminator), and why this one ended.
fn field_at(source: &str, at: usize, delimiter: Delimiter) -> (Field<'_>, usize, Stop) {
    if source.as_bytes().get(at) == Some(&b'"') {
        quoted_field_at(source, at, delimiter)
    } else {
        plain_field_at(source, at, delimiter)
    }
}

/// A field with no opening quote — everything up to the next structural byte.
///
/// The loop finds the boundary and the field is built once below it, which is
/// the shape [`quoted_field_at`] already used for its own exits.
fn plain_field_at(source: &str, at: usize, delimiter: Delimiter) -> (Field<'_>, usize, Stop) {
    let bytes = source.as_bytes();
    let mut end = at;
    let (next, stop) = loop {
        if end >= bytes.len() {
            break (end, Stop::Eof);
        }
        match bytes[end] {
            byte if byte == delimiter.byte() => break (end + 1, Stop::Field),
            b'\n' => break (end + 1, Stop::Record),
            // CRLF terminates; a lone CR is data (module header).
            b'\r' if bytes.get(end + 1) == Some(&b'\n') => break (end + 2, Stop::Record),
            _ => end += 1,
        }
    };
    (
        Field {
            value: Cow::Borrowed(&source[at..end]),
            span: at..end,
        },
        next,
        stop,
    )
}

/// A field opening with `"`.
///
/// Borrows when it can — a quoted field with no `""` and nothing after its
/// closing quote is a slice of the source, which is the common case and the
/// reason a 200 MB file does not become 200 MB of `String`s.
fn quoted_field_at(source: &str, at: usize, delimiter: Delimiter) -> (Field<'_>, usize, Stop) {
    let bytes = source.as_bytes();
    let open = at + 1;
    let mut scan = open;
    let mut value: Option<String> = None;
    let mut piece_start = open;

    while scan < bytes.len() {
        if bytes[scan] != b'"' {
            scan += 1;
            continue;
        }
        if bytes.get(scan + 1) == Some(&b'"') {
            // `""` — one quote of data, and the field can no longer be a slice.
            let text = value.get_or_insert_with(String::new);
            text.push_str(&source[piece_start..scan]);
            text.push('"');
            scan += 2;
            piece_start = scan;
            continue;
        }
        // The closing quote. Whatever follows it before the next structural
        // byte is data appended to this field (module header).
        let closed = scan;
        let (tail, next, stop) = plain_field_at(source, closed + 1, delimiter);
        let span = at..tail.span.end;
        let value = match (value, tail.value.is_empty()) {
            (None, true) => Cow::Borrowed(&source[open..closed]),
            (None, false) => {
                let mut text = source[open..closed].to_owned();
                text.push_str(tail.value());
                Cow::Owned(text)
            }
            (Some(mut text), _) => {
                text.push_str(&source[piece_start..closed]);
                text.push_str(tail.value());
                Cow::Owned(text)
            }
        };
        return (Field { value, span }, next, stop);
    }

    // Unterminated: the field runs to end of input.
    let value = match value {
        None => Cow::Borrowed(&source[open..]),
        Some(mut text) => {
            text.push_str(&source[piece_start..]);
            Cow::Owned(text)
        }
    };
    (
        Field {
            value,
            span: at..source.len(),
        },
        source.len(),
        Stop::Eof,
    )
}
