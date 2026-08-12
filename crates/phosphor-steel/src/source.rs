//! Splitting a `.scm` file into top-level forms — the granularity the boot
//! fails at.
//!
//! # Why this exists at all
//!
//! `T021`'s requirement is that *a broken `init.scm` boots the editor anyway.*
//! Handing a whole file to
//! [`Engine::compile_and_run_raw_program`](steel::steel_vm::engine::Engine::compile_and_run_raw_program)
//! makes the *file* the unit of failure: one stray paren on line 40 discards
//! the thirty-nine good forms above it, and the load order the file declares
//! goes with them. That is the silent-discard failure the task names, one level
//! down from the one everybody thinks of.
//!
//! So the unit is the **top-level form**. A file is scanned into forms here,
//! each is compiled and run on its own, and a form that fails is named in the
//! boot float while the rest of the file still runs. Boot faults become as
//! local as the mistake that caused them.
//!
//! # Why a scanner and not Steel's parser
//!
//! A parser stops at the first error. Recovering from it means knowing where
//! the *next* form starts, which is exactly the balancing this scanner does and
//! nothing more — it never interprets, and the forms it hands back go to Steel
//! unaltered. It is a delimiter scanner, deliberately: strings, line comments,
//! nestable `#|` block comments, `#;` datum comments, character literals
//! (`#\(` is not an open paren) and the quote family.
//!
//! Owned by `spine`.

/// One top-level form: a byte range of the source.
///
/// A range rather than a `String` because the caller needs the offset anyway —
/// a Steel error's span is relative to whatever was compiled, so the file
/// position is `form.start + span.start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Form {
    /// Where it starts, in bytes.
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
}

impl Form {
    /// This form's text.
    ///
    /// # Panics
    ///
    /// Never for a form this module produced: both ends land on character
    /// boundaries by construction.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// Something opened and never closed.
///
/// Reported separately from the forms before it, because the forms before it
/// *ran*. A file ending mid-string is one fault, not a dead file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unterminated {
    /// Where the thing that never closed began.
    pub start: usize,
    /// What it was: `"form"`, `"string"`, or `"block comment"`.
    pub what: &'static str,
}

/// Splits `source` into top-level forms, plus whatever was left open at the end.
///
/// The forms come back even when something was left open — everything before
/// the unterminated thing is complete and runnable, which is the whole point.
#[must_use]
pub fn top_level_forms(source: &str) -> (Vec<Form>, Option<Unterminated>) {
    let bytes = source.as_bytes();
    let mut forms = Vec::new();
    let mut at = 0;

    loop {
        at = match skip_atmosphere(bytes, at) {
            Ok(next) => next,
            Err(open) => return (forms, Some(open)),
        };
        if at >= bytes.len() {
            return (forms, None);
        }
        match datum_end(bytes, at) {
            Ok(end) => {
                forms.push(Form { start: at, end });
                at = end;
            }
            Err(open) => return (forms, Some(open)),
        }
    }
}

/// The 1-based line and column of a byte offset.
///
/// The column counts characters, not bytes: it is shown to a person next to
/// their own source line.
#[must_use]
pub fn line_and_column(source: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(source.len());
    let before = &source[..floor_boundary(source, offset)];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rfind('\n')
        .map_or(before, |newline| &before[newline + 1..])
        .chars()
        .count()
        + 1;
    (truncate(line), truncate(column))
}

/// The text of a 1-based line, without its terminator.
#[must_use]
pub fn nth_line(source: &str, line: u32) -> Option<&str> {
    let index = usize::try_from(line).ok()?.checked_sub(1)?;
    source.lines().nth(index)
}

// ---------------------------------------------------------------------------
// The scanner
// ---------------------------------------------------------------------------

/// Whitespace and comments — everything between two forms.
fn skip_atmosphere(bytes: &[u8], mut at: usize) -> Result<usize, Unterminated> {
    loop {
        let Some(byte) = bytes.get(at).copied() else {
            return Ok(at);
        };
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => at += 1,
            b';' => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            }
            b'#' if bytes.get(at + 1) == Some(&b'|') => at = block_comment_end(bytes, at)?,
            _ => return Ok(at),
        }
    }
}

/// One datum, including any `'`, `` ` ``, `,`, `,@` or `#;` prefixing it.
fn datum_end(bytes: &[u8], start: usize) -> Result<usize, Unterminated> {
    let mut at = start;
    loop {
        at = skip_atmosphere(bytes, at)?;
        let Some(byte) = bytes.get(at).copied() else {
            // A prefix with nothing after it.
            return Err(Unterminated {
                start,
                what: "form",
            });
        };
        match byte {
            b'\'' | b'`' => at += 1,
            b',' => {
                at += 1;
                if bytes.get(at) == Some(&b'@') {
                    at += 1;
                }
            }
            // `#;` comments out the datum that follows it, so the two are one
            // unit: dropping the comment without the datum it governs would
            // change what runs.
            b'#' if bytes.get(at + 1) == Some(&b';') => at += 2,
            b'#' if bytes.get(at + 1) == Some(&b'\\') => return Ok(char_literal_end(bytes, at)),
            b'(' | b'[' | b'{' => return compound_end(bytes, at),
            b'"' => return string_end(bytes, at),
            _ => return Ok(atom_end(bytes, at)),
        }
    }
}

/// A bracketed form, from its opener to its matching closer.
fn compound_end(bytes: &[u8], start: usize) -> Result<usize, Unterminated> {
    let mut depth = 0_usize;
    let mut at = start;
    loop {
        let Some(byte) = bytes.get(at).copied() else {
            return Err(Unterminated {
                start,
                what: "form",
            });
        };
        match byte {
            b'(' | b'[' | b'{' => {
                depth += 1;
                at += 1;
            }
            b')' | b']' | b'}' => {
                at += 1;
                depth -= 1;
                if depth == 0 {
                    return Ok(at);
                }
            }
            b'"' => at = string_end(bytes, at)?,
            b';' => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            }
            b'#' if bytes.get(at + 1) == Some(&b'|') => at = block_comment_end(bytes, at)?,
            // `#\(` is a character, not an opener. Without this the scanner
            // would count it and every form after it would be off by one.
            b'#' if bytes.get(at + 1) == Some(&b'\\') => at = char_literal_end(bytes, at),
            _ => at += 1,
        }
    }
}

/// A string literal, from its opening quote past its closing one.
fn string_end(bytes: &[u8], start: usize) -> Result<usize, Unterminated> {
    let mut at = start + 1;
    while let Some(byte) = bytes.get(at).copied() {
        match byte {
            b'\\' => at += 2,
            b'"' => return Ok(at + 1),
            _ => at += 1,
        }
    }
    Err(Unterminated {
        start,
        what: "string",
    })
}

/// A `#| … |#` comment, which nests.
fn block_comment_end(bytes: &[u8], start: usize) -> Result<usize, Unterminated> {
    let mut depth = 0_usize;
    let mut at = start;
    loop {
        let Some(byte) = bytes.get(at).copied() else {
            return Err(Unterminated {
                start,
                what: "block comment",
            });
        };
        if byte == b'#' && bytes.get(at + 1) == Some(&b'|') {
            depth += 1;
            at += 2;
        } else if byte == b'|' && bytes.get(at + 1) == Some(&b'#') {
            at += 2;
            depth -= 1;
            if depth == 0 {
                return Ok(at);
            }
        } else {
            at += 1;
        }
    }
}

/// A `#\x` literal, including the named ones (`#\space`, `#\newline`).
fn char_literal_end(bytes: &[u8], start: usize) -> usize {
    let mut at = start + 2;
    let Some(byte) = bytes.get(at).copied() else {
        return at;
    };
    at += utf8_width(byte);
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-') {
        at += 1;
    }
    at
}

/// A symbol, number or `#t`/`#f`, up to the next delimiter.
fn atom_end(bytes: &[u8], start: usize) -> usize {
    let mut at = start;
    while at < bytes.len() && !is_delimiter(bytes[at]) {
        at += 1;
    }
    // A lone delimiter — a stray `)`, say. Consume it so the scan advances;
    // Steel will have the last word on what it means.
    at.max(start + 1).min(bytes.len())
}

const fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | 0x0b
            | 0x0c
            | b'('
            | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'"'
            | b';'
            | b'\''
            | b'`'
            | b','
    )
}

const fn utf8_width(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// The nearest character boundary at or below `offset`.
///
/// A Steel span is a byte range and nothing guarantees it lands on one.
fn floor_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Line and column numbers are for reading; a file that overflows `u32` of
/// either has a worse problem than a truncated number.
fn truncate(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<&str> {
        let (forms, open) = top_level_forms(source);
        assert!(open.is_none(), "unexpected {open:?} in {source:?}");
        forms.iter().map(|form| form.text(source)).collect()
    }

    #[test]
    fn forms_are_split_one_per_top_level_datum() {
        assert_eq!(
            texts("(define x 1)\n(define y 2)\n"),
            ["(define x 1)", "(define y 2)"]
        );
    }

    #[test]
    fn comments_are_atmosphere_and_never_a_form() {
        assert_eq!(
            texts("; the boot session\n(define x 1) ; trailing\n#| block\n   comment |#\n(f)"),
            ["(define x 1)", "(f)"]
        );
    }

    #[test]
    fn block_comments_nest() {
        assert_eq!(texts("#| outer #| inner |# still |# (f)"), ["(f)"]);
    }

    #[test]
    fn parens_inside_a_string_do_not_count() {
        assert_eq!(
            texts(r#"(set-option! "a )( b" #t) (f)"#),
            [r#"(set-option! "a )( b" #t)"#, "(f)"]
        );
    }

    #[test]
    fn an_escaped_quote_does_not_end_a_string() {
        assert_eq!(
            texts(r#"(f "say \"hi\"") (g)"#),
            [r#"(f "say \"hi\"")"#, "(g)"]
        );
    }

    #[test]
    fn a_character_literal_is_not_a_paren() {
        // `#\(` and `#\;` would each throw the scan off by one.
        assert_eq!(
            texts(r"(f #\( #\; #\space) (g)"),
            [r"(f #\( #\; #\space)", "(g)"]
        );
    }

    #[test]
    fn a_quote_prefix_belongs_to_the_form_it_prefixes() {
        assert_eq!(
            texts("'(a b) `(c ,d ,@e) (f)"),
            ["'(a b)", "`(c ,d ,@e)", "(f)"]
        );
    }

    #[test]
    fn a_datum_comment_takes_the_datum_with_it() {
        assert_eq!(texts("#;(dead form) (live)"), ["#;(dead form)", "(live)"]);
    }

    #[test]
    fn the_forms_before_an_unclosed_one_still_come_back() {
        // The whole reason this module exists: everything above the mistake ran.
        let source = "(define x 1)\n(define y 2)\n(oops\n";
        let (forms, open) = top_level_forms(source);
        assert_eq!(
            forms.iter().map(|f| f.text(source)).collect::<Vec<_>>(),
            ["(define x 1)", "(define y 2)"]
        );
        let open = open.expect("the third form never closed");
        assert_eq!(open.what, "form");
        assert_eq!(line_and_column(source, open.start), (3, 1));
    }

    #[test]
    fn an_unterminated_string_is_named_as_one() {
        let (_, open) = top_level_forms("(f \"never closed");
        assert_eq!(open.expect("unterminated").what, "string");
    }

    #[test]
    fn an_unterminated_block_comment_is_named_as_one() {
        let (_, open) = top_level_forms("(f)\n#| and then nothing");
        assert_eq!(open.expect("unterminated").what, "block comment");
    }

    #[test]
    fn positions_are_one_based_in_both_axes_and_count_characters() {
        let source = "(a)\n(λx 2)\n";
        let (forms, _) = top_level_forms(source);
        assert_eq!(line_and_column(source, forms[1].start), (2, 1));
        // `λ` is two bytes; the column after it is 2, not 3.
        let inside = forms[1].start + 1 + 'λ'.len_utf8();
        assert_eq!(line_and_column(source, inside), (2, 3));
        assert_eq!(nth_line(source, 2), Some("(λx 2)"));
    }

    #[test]
    fn an_empty_file_has_no_forms_and_no_fault() {
        assert_eq!(top_level_forms("\n  ; nothing\n"), (Vec::new(), None));
    }
}
