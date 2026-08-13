//! The buffer as the input machine reads it, and where every motion and text
//! object is resolved.
//!
//! # Why the machine reads the buffer at all
//!
//! `Buffer::Delete` takes a [`Span`] and `Buffer::Insert` takes a [`Position`]
//! (`action.rs`), so *"delete a word"* is not sayable in the vocabulary — only
//! *"delete these two positions apart"* is. Something has to turn `dw` into a
//! span, and there are only two candidates: the machine, or whoever applies the
//! Action. It is the machine's, for one reason that decides it — **the same
//! resolution answers `MoveCursor`**. If the host resolved operator spans and
//! the machine resolved nothing, `w` and `dw` would be two implementations of
//! where a word ends, and they would disagree within a window.
//!
//! So [`Text`] is a *read-only* view of one buffer, the machine's only window
//! onto content, and [`cursor_after`] is called from both sides: by the machine
//! to build `dw`'s span, and by the host to apply `MoveCursor`.
//!
//! # What it deliberately cannot answer
//!
//! [`object_span`] returns [`None`] for the four agent nouns — `u` unseen, `h`
//! hunk, `t` thread, `b` block (`6d`). They are **store queries, not syntax**
//! (`request::TextObject`'s own header), and there is no store until `T041`.
//! `T028` binds them, they no-op cleanly here, and `T049` is where they resolve
//! — by giving [`Text`] a neighbour that can answer a region query, not by
//! teaching this file about regions.
//!
//! # Cost
//!
//! [`Text::line`] returns an owned `String`, which is an allocation per line
//! touched. A motion touches one line and a paragraph motion touches until it
//! finds a blank one; at a keystroke's rate that is free, and the alternative —
//! handing out a borrow — makes the trait unimplementable over a rope that
//! stores lines in pieces (`ropey`'s `RopeSlice` is not `&str`).

use crate::request::{Motion, Position, SelectionKind, Span, TextObject};

/// One buffer, read-only, as the input machine sees it.
///
/// Positions are 1-based in both axes, as everywhere else in the vocabulary
/// (`request::Position`). A column may be one past the last character of a line
/// — that is the newline boundary, where `a` at the end of a line puts the
/// cursor, and it is a legal [`Position`].
pub trait Text {
    /// How many lines the buffer has. Never zero: an empty buffer has one
    /// empty line.
    fn lines(&self) -> u32;

    /// One line's text, without its newline. [`None`] past the end.
    fn line(&self, line: u32) -> Option<String>;

    /// Where the cursor is.
    fn cursor(&self) -> Position;

    /// The first line drawn, and how many rows the text area has.
    ///
    /// Only `H`, `M`, `L` and the half-page motions read it. The default is
    /// *"the whole buffer is on screen"*, which is what a headless driver
    /// wants and what those four motions then degrade to.
    fn viewport(&self) -> Viewport {
        Viewport {
            top: 1,
            height: self.lines(),
        }
    }
}

/// What is on screen, in buffer lines.
///
/// Buffer lines rather than visual rows on purpose: a motion moves the *cursor*
/// and the cursor lives in the buffer. Revealing the result is a separate
/// `View::Scroll` request, measured in visual rows by whoever draws (`T081`'s
/// soft wrap is why those two are not the same number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// 1-based line at the top of the text area.
    pub top: u32,
    /// Rows of text area.
    pub height: u32,
}

/// What a character counts as, for word motions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Letters, digits and `_` — vim's "word".
    Word,
    /// Anything else that is not blank.
    Punct,
    /// Spaces, tabs, and the newline at the end of a line.
    Blank,
}

fn class(character: char) -> Class {
    if character.is_alphanumeric() || character == '_' {
        Class::Word
    } else if character.is_whitespace() {
        Class::Blank
    } else {
        Class::Punct
    }
}

/// Characters on a line, not counting the newline.
fn width(text: &dyn Text, line: u32) -> u32 {
    text.line(line).map_or(0, |row| {
        u32::try_from(row.chars().count()).unwrap_or(u32::MAX)
    })
}

/// The character at a position; the newline boundary reads as `\n`.
fn at(text: &dyn Text, position: Position) -> Option<char> {
    let row = text.line(position.line)?;
    let column = position.column.checked_sub(1)? as usize;
    Some(row.chars().nth(column).unwrap_or('\n'))
}

/// The next character boundary, crossing lines.
fn forward(text: &dyn Text, position: Position) -> Option<Position> {
    if position.column <= width(text, position.line) {
        return Some(Position {
            column: position.column + 1,
            ..position
        });
    }
    (position.line < text.lines()).then_some(Position {
        line: position.line + 1,
        column: 1,
    })
}

/// The previous character boundary, crossing lines.
fn backward(text: &dyn Text, position: Position) -> Option<Position> {
    if position.column > 1 {
        return Some(Position {
            column: position.column - 1,
            ..position
        });
    }
    (position.line > 1).then(|| Position {
        line: position.line - 1,
        column: width(text, position.line - 1) + 1,
    })
}

/// Puts a position inside the buffer, with the column on a character.
///
/// The newline boundary is legal for insertion and not for a normal-mode
/// cursor, so this clamps to the last character — the entries that want the
/// boundary ([`end_of_line`]) ask for it by name.
#[must_use]
pub fn clamp(text: &dyn Text, position: Position) -> Position {
    let line = position.line.clamp(1, text.lines().max(1));
    let last = width(text, line).max(1);
    Position {
        line,
        column: position.column.clamp(1, last),
    }
}

/// The newline boundary of a line — where `A` and `o` insert.
#[must_use]
pub fn end_of_line(text: &dyn Text, line: u32) -> Position {
    Position {
        line: line.clamp(1, text.lines().max(1)),
        column: width(text, line) + 1,
    }
}

/// The first non-blank column of a line, 1-based.
#[must_use]
pub fn first_non_blank(text: &dyn Text, line: u32) -> Position {
    let row = text.line(line).unwrap_or_default();
    let column = row
        .chars()
        .position(|character| !character.is_whitespace())
        .map_or(1, |index| u32::try_from(index).unwrap_or(0) + 1);
    Position { line, column }
}

/// Where `motion`, repeated `count` times, leaves the cursor.
///
/// **Both callers of the vim grammar meet here** (module header): the host
/// applying `MoveCursor` and the machine building an operator's span. Total —
/// an unresolvable motion answers where it started rather than erroring, so a
/// keystroke never becomes a fault.
#[must_use]
pub fn cursor_after(text: &dyn Text, from: Position, motion: Motion, count: u32) -> Position {
    let count = count.max(1);
    let mut position = from;
    for _ in 0..count {
        position = step(text, position, motion);
    }
    match motion {
        // The vertical motions keep the column and clamp it to the new line.
        Motion::LineUp | Motion::LineDown | Motion::HalfPageUp | Motion::HalfPageDown => {
            clamp(text, position)
        }
        _ => position,
    }
}

/// One application of a motion. One arm per motion, deliberately: a table
/// reads better than a dispatch through twenty functions.
fn step(text: &dyn Text, from: Position, motion: Motion) -> Position {
    let lines = text.lines().max(1);
    let view = text.viewport();
    match motion {
        Motion::CharLeft => Position {
            column: from.column.saturating_sub(1).max(1),
            ..from
        },
        Motion::CharRight => clamp(
            text,
            Position {
                column: from.column + 1,
                ..from
            },
        ),
        Motion::LineUp => Position {
            line: from.line.saturating_sub(1).max(1),
            ..from
        },
        Motion::LineDown => Position {
            line: (from.line + 1).min(lines),
            ..from
        },
        Motion::WordForward => word_forward(text, from),
        Motion::WordBackward => word_backward(text, from),
        Motion::WordEnd => word_end(text, from),
        Motion::LineStart => Position { column: 1, ..from },
        Motion::FirstNonBlank => first_non_blank(text, from.line),
        Motion::LineEnd => Position {
            column: width(text, from.line).max(1),
            ..from
        },
        Motion::BufferStart => first_non_blank(text, 1),
        Motion::BufferEnd => first_non_blank(text, lines),
        Motion::ParagraphForward => paragraph(text, from, true),
        Motion::ParagraphBackward => paragraph(text, from, false),
        Motion::MatchingBracket => matching_bracket(text, from).unwrap_or(from),
        Motion::ScreenTop => first_non_blank(text, view.top.clamp(1, lines)),
        Motion::ScreenMiddle => first_non_blank(text, (view.top + view.height / 2).clamp(1, lines)),
        Motion::ScreenBottom => first_non_blank(
            text,
            (view.top + view.height.saturating_sub(1)).clamp(1, lines),
        ),
        Motion::HalfPageDown => Position {
            line: (from.line + (view.height / 2).max(1)).min(lines),
            ..from
        },
        Motion::HalfPageUp => Position {
            line: from.line.saturating_sub((view.height / 2).max(1)).max(1),
            ..from
        },
        // No search prompt until `T033`'s ex commands, and a motion that
        // invented a destination would be worse than one that stays put.
        Motion::SearchNext | Motion::SearchPrev => from,
    }
}

fn word_forward(text: &dyn Text, from: Position) -> Position {
    let mut position = from;
    if let Some(start) = at(text, position)
        && class(start) != Class::Blank
    {
        let start = class(start);
        while let Some(next) = forward(text, position) {
            if at(text, next).map(class) == Some(start) {
                position = next;
            } else {
                position = next;
                break;
            }
        }
    }
    while at(text, position).map(class) == Some(Class::Blank) {
        match forward(text, position) {
            Some(next) => position = next,
            None => break,
        }
    }
    position
}

fn word_backward(text: &dyn Text, from: Position) -> Position {
    let Some(mut position) = backward(text, from) else {
        return from;
    };
    while at(text, position).map(class) == Some(Class::Blank) {
        match backward(text, position) {
            Some(previous) => position = previous,
            None => return position,
        }
    }
    let run = at(text, position).map(class);
    while let Some(previous) = backward(text, position) {
        if at(text, previous).map(class) == run {
            position = previous;
        } else {
            break;
        }
    }
    position
}

fn word_end(text: &dyn Text, from: Position) -> Position {
    let Some(mut position) = forward(text, from) else {
        return from;
    };
    while at(text, position).map(class) == Some(Class::Blank) {
        match forward(text, position) {
            Some(next) => position = next,
            None => return position,
        }
    }
    let run = at(text, position).map(class);
    while let Some(next) = forward(text, position) {
        if at(text, next).map(class) == run {
            position = next;
        } else {
            break;
        }
    }
    position
}

fn is_blank_line(text: &dyn Text, line: u32) -> bool {
    text.line(line).is_none_or(|row| row.trim().is_empty())
}

fn paragraph(text: &dyn Text, from: Position, forwards: bool) -> Position {
    let lines = text.lines().max(1);
    let mut line = from.line;
    let mut seen_text = false;
    loop {
        let next = if forwards {
            if line >= lines {
                return Position {
                    line: lines,
                    column: 1,
                };
            }
            line + 1
        } else {
            if line <= 1 {
                return Position { line: 1, column: 1 };
            }
            line - 1
        };
        line = next;
        if is_blank_line(text, line) {
            if seen_text {
                return Position { line, column: 1 };
            }
        } else {
            seen_text = true;
        }
    }
}

/// The `(`, `[` or `{` family a character belongs to, and which way it points.
fn bracket(character: char) -> Option<(char, char, bool)> {
    let pair = match character {
        '(' => ('(', ')', true),
        ')' => ('(', ')', false),
        '[' => ('[', ']', true),
        ']' => ('[', ']', false),
        '{' => ('{', '}', true),
        '}' => ('{', '}', false),
        '<' => ('<', '>', true),
        '>' => ('<', '>', false),
        _ => return None,
    };
    Some(pair)
}

/// One step along the buffer, in whichever direction a bracket search runs.
type Walk = fn(&dyn Text, Position) -> Option<Position>;

fn matching_bracket(text: &dyn Text, from: Position) -> Option<Position> {
    // `%` starts at the first bracket at or after the cursor *on this line* —
    // vim's rule, and the reason `%` on an indented `if (` works.
    let mut start = from;
    let (open, close, opening) = loop {
        match at(text, start).and_then(bracket) {
            Some(found) => break found,
            None => {
                if start.column > width(text, start.line) {
                    return None;
                }
                start = forward(text, start)?;
            }
        }
    };
    let (want, step): (char, Walk) = if opening {
        (close, forward)
    } else {
        (open, backward)
    };
    let mine = if opening { open } else { close };
    let mut depth = 0_u32;
    let mut position = start;
    loop {
        let character = at(text, position)?;
        if character == mine {
            depth += 1;
        } else if character == want {
            depth -= 1;
            if depth == 0 {
                return Some(position);
            }
        }
        position = step(text, position)?;
    }
}

/// The span an operator covers when `motion` is its operand.
///
/// [`None`] when the motion has no span — a search with no search state, and
/// anything that would run off the end of the buffer.
#[must_use]
pub fn motion_span(
    text: &dyn Text,
    from: Position,
    motion: Motion,
    count: u32,
) -> Option<(Span, SelectionKind)> {
    if matches!(motion, Motion::SearchNext | Motion::SearchPrev) {
        return None;
    }
    let to = cursor_after(text, from, motion, count);
    let kind = linewise(motion);
    if kind == SelectionKind::Line {
        let (first, last) = (from.line.min(to.line), from.line.max(to.line));
        return Some((line_span(text, first, last), SelectionKind::Line));
    }
    let (start, end) = if (to.line, to.column) < (from.line, from.column) {
        (to, from)
    } else {
        (from, to)
    };
    // `e`, `%` and `$` take the character they land on; `w`, `b`, `0` stop
    // short of it. That is vim's inclusive/exclusive split and the only place
    // it lives.
    let end = if inclusive(motion) {
        forward(text, end).unwrap_or(end)
    } else {
        end
    };
    Some((Span { start, end }, SelectionKind::Char))
}

/// A whole-line span, `first`..=`last`, ending at the start of the line after.
#[must_use]
pub fn line_span(text: &dyn Text, first: u32, last: u32) -> Span {
    let lines = text.lines().max(1);
    let first = first.clamp(1, lines);
    let last = last.clamp(first, lines);
    let end = if last < lines {
        Position {
            line: last + 1,
            column: 1,
        }
    } else {
        end_of_line(text, last)
    };
    Span {
        start: Position {
            line: first,
            column: 1,
        },
        end,
    }
}

const fn linewise(motion: Motion) -> SelectionKind {
    match motion {
        Motion::LineUp
        | Motion::LineDown
        | Motion::BufferStart
        | Motion::BufferEnd
        | Motion::ParagraphForward
        | Motion::ParagraphBackward
        | Motion::ScreenTop
        | Motion::ScreenMiddle
        | Motion::ScreenBottom
        | Motion::HalfPageUp
        | Motion::HalfPageDown => SelectionKind::Line,
        _ => SelectionKind::Char,
    }
}

const fn inclusive(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::WordEnd | Motion::LineEnd | Motion::MatchingBracket
    )
}

/// The span a text object covers, or [`None`] if this object cannot answer.
///
/// The four agent nouns are the deliberate [`None`]s — see the module header.
#[must_use]
pub fn object_span(
    text: &dyn Text,
    at_position: Position,
    object: TextObject,
    inner: bool,
    count: u32,
    delimiter: Option<char>,
) -> Option<(Span, SelectionKind)> {
    let _ = count;
    match object {
        TextObject::Word => word_object(text, at_position, inner, false),
        TextObject::BigWord => word_object(text, at_position, inner, true),
        TextObject::Sentence => sentence_object(text, at_position, inner),
        TextObject::Paragraph => paragraph_object(text, at_position, inner),
        TextObject::Delimited => delimited_object(text, at_position, inner, delimiter?),
        // `T028` binds these; `T049` resolves them. A markup tag needs the
        // grammar (`T037`), and the other four need the store (`T041`).
        TextObject::Tag
        | TextObject::UnseenRegion
        | TextObject::Hunk
        | TextObject::Thread
        | TextObject::Block => None,
    }
}

fn word_object(
    text: &dyn Text,
    from: Position,
    inner: bool,
    big: bool,
) -> Option<(Span, SelectionKind)> {
    let classify = |character: char| {
        if big {
            if character.is_whitespace() {
                Class::Blank
            } else {
                Class::Word
            }
        } else {
            class(character)
        }
    };
    let run = classify(at(text, from)?);
    let mut start = from;
    while let Some(previous) = backward(text, start) {
        if previous.line != start.line || at(text, previous).map(classify) != Some(run) {
            break;
        }
        start = previous;
    }
    let mut end = from;
    while let Some(next) = forward(text, end) {
        if next.line != end.line || at(text, next).map(classify) != Some(run) {
            break;
        }
        end = next;
    }
    let mut end = forward(text, end).unwrap_or(end);
    if !inner {
        // `aw` takes the whitespace after the word, and only after it — vim
        // falls back to the whitespace before when there is none after, which
        // is a refinement this does not have and `CP-3` will find if it matters.
        while at(text, end).map(class) == Some(Class::Blank) && end.line == from.line {
            match forward(text, end) {
                Some(next) => end = next,
                None => break,
            }
        }
    }
    Some((Span { start, end }, SelectionKind::Char))
}

fn paragraph_object(text: &dyn Text, from: Position, inner: bool) -> Option<(Span, SelectionKind)> {
    let lines = text.lines().max(1);
    let mut first = from.line;
    while first > 1 && !is_blank_line(text, first - 1) {
        first -= 1;
    }
    let mut last = from.line;
    while last < lines && !is_blank_line(text, last + 1) {
        last += 1;
    }
    if !inner {
        while last < lines && is_blank_line(text, last + 1) {
            last += 1;
        }
    }
    Some((line_span(text, first, last), SelectionKind::Line))
}

fn sentence_object(text: &dyn Text, from: Position, inner: bool) -> Option<(Span, SelectionKind)> {
    let row = text.line(from.line)?;
    let characters: Vec<char> = row.chars().collect();
    let index = (from.column as usize)
        .saturating_sub(1)
        .min(characters.len());
    let ends = |at: usize| matches!(characters.get(at), Some('.' | '!' | '?'));
    let mut start = index;
    while start > 0
        && !(ends(start - 1) && characters.get(start).is_some_and(|c| c.is_whitespace()))
    {
        start -= 1;
    }
    while characters.get(start).is_some_and(|c| c.is_whitespace()) {
        start += 1;
    }
    let mut end = index;
    while end < characters.len() && !ends(end) {
        end += 1;
    }
    if end < characters.len() {
        end += 1;
    }
    if !inner {
        while characters.get(end).is_some_and(|c| c.is_whitespace()) {
            end += 1;
        }
    }
    Some((
        Span {
            start: Position {
                line: from.line,
                column: u32::try_from(start).unwrap_or(0) + 1,
            },
            end: Position {
                line: from.line,
                column: u32::try_from(end).unwrap_or(0) + 1,
            },
        },
        SelectionKind::Char,
    ))
}

fn delimited_object(
    text: &dyn Text,
    from: Position,
    inner: bool,
    delimiter: char,
) -> Option<(Span, SelectionKind)> {
    let (open, close) = match delimiter {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' => ('{', '}'),
        '<' | '>' => ('<', '>'),
        quote => (quote, quote),
    };
    let (start, end) = if open == close {
        quoted(text, from, open)?
    } else {
        enclosing(text, from, open, close)?
    };
    let span = if inner {
        Span {
            start: forward(text, start)?,
            end,
        }
    } else {
        Span {
            start,
            end: forward(text, end)?,
        }
    };
    Some((span, SelectionKind::Char))
}

/// The innermost `open`/`close` pair containing (or starting at) the cursor.
fn enclosing(
    text: &dyn Text,
    from: Position,
    open: char,
    close: char,
) -> Option<(Position, Position)> {
    let mut start = from;
    let mut depth = 0_u32;
    loop {
        match at(text, start) {
            Some(character) if character == open => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Some(character) if character == close && start != from => depth += 1,
            _ => {}
        }
        start = backward(text, start)?;
    }
    let mut end = start;
    let mut depth = 0_u32;
    loop {
        match at(text, end) {
            Some(character) if character == open => depth += 1,
            Some(character) if character == close => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            None => return None,
            _ => {}
        }
        end = forward(text, end)?;
    }
    Some((start, end))
}

/// A quoted run on the cursor's own line — quotes do not nest and do not wrap.
fn quoted(text: &dyn Text, from: Position, quote: char) -> Option<(Position, Position)> {
    let row = text.line(from.line)?;
    let columns: Vec<u32> = row
        .chars()
        .enumerate()
        .filter(|(_, character)| *character == quote)
        .map(|(index, _)| u32::try_from(index).unwrap_or(0) + 1)
        .collect();
    for pair in columns.chunks_exact(2) {
        let (open, close) = (pair[0], pair[1]);
        if from.column <= close {
            return Some((
                Position {
                    line: from.line,
                    column: open,
                },
                Position {
                    line: from.line,
                    column: close,
                },
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer of lines, which is every fixture this module needs.
    #[derive(Debug)]
    struct Lines {
        rows: Vec<String>,
        cursor: Position,
    }

    impl Lines {
        fn new(text: &str, line: u32, column: u32) -> Self {
            Self {
                rows: text.split('\n').map(str::to_owned).collect(),
                cursor: Position { line, column },
            }
        }
    }

    impl Text for Lines {
        fn lines(&self) -> u32 {
            u32::try_from(self.rows.len()).unwrap_or(1).max(1)
        }

        fn line(&self, line: u32) -> Option<String> {
            self.rows.get((line as usize).checked_sub(1)?).cloned()
        }

        fn cursor(&self) -> Position {
            self.cursor
        }
    }

    fn at_position(line: u32, column: u32) -> Position {
        Position { line, column }
    }

    #[test]
    fn word_motions_walk_runs_of_one_class() {
        let text = Lines::new("let value = foo_bar(1);\nnext line", 1, 1);
        assert_eq!(
            cursor_after(&text, at_position(1, 1), Motion::WordForward, 1),
            at_position(1, 5)
        );
        // Punctuation is its own run: `=` then `foo_bar` then `(`.
        assert_eq!(
            cursor_after(&text, at_position(1, 5), Motion::WordForward, 2),
            at_position(1, 13)
        );
        assert_eq!(
            cursor_after(&text, at_position(1, 1), Motion::WordEnd, 1),
            at_position(1, 3)
        );
        assert_eq!(
            cursor_after(&text, at_position(1, 5), Motion::WordBackward, 1),
            at_position(1, 1)
        );
        // A word motion crosses the line boundary rather than stopping at it.
        assert_eq!(
            cursor_after(&text, at_position(1, 23), Motion::WordForward, 1),
            at_position(2, 1)
        );
    }

    #[test]
    fn a_vertical_motion_keeps_the_column_and_clamps_it() {
        let text = Lines::new("a longer line\nshort\nanother long line", 1, 9);
        assert_eq!(
            cursor_after(&text, at_position(1, 9), Motion::LineDown, 1),
            at_position(2, 5),
            "the column clamps to the shorter line"
        );
    }

    #[test]
    fn the_bracket_motion_matches_across_lines() {
        let text = Lines::new("fn f(a: u8) {\n    g(a);\n}", 1, 13);
        assert_eq!(
            cursor_after(&text, at_position(1, 13), Motion::MatchingBracket, 1),
            at_position(3, 1)
        );
        assert_eq!(
            cursor_after(&text, at_position(3, 1), Motion::MatchingBracket, 1),
            at_position(1, 13)
        );
        // `%` finds the first bracket after the cursor on the line.
        assert_eq!(
            cursor_after(&text, at_position(2, 1), Motion::MatchingBracket, 1),
            at_position(2, 8)
        );
    }

    #[test]
    fn an_operators_span_is_inclusive_only_where_vim_says_so() {
        let text = Lines::new("alpha beta gamma", 1, 1);
        // `dw` stops short of the character it lands on…
        let (span, kind) = motion_span(&text, at_position(1, 1), Motion::WordForward, 1).unwrap();
        assert_eq!(span.end, at_position(1, 7));
        assert_eq!(kind, SelectionKind::Char);
        // …`de` takes it.
        let (span, _) = motion_span(&text, at_position(1, 1), Motion::WordEnd, 1).unwrap();
        assert_eq!(span.end, at_position(1, 6));
        // `d$` takes the last character too.
        let (span, _) = motion_span(&text, at_position(1, 12), Motion::LineEnd, 1).unwrap();
        assert_eq!(span.end, at_position(1, 17));
    }

    #[test]
    fn a_linewise_span_swallows_the_whole_line_and_its_newline() {
        let text = Lines::new("one\ntwo\nthree\nfour\nfive", 2, 1);
        let (span, kind) = motion_span(&text, at_position(2, 1), Motion::LineDown, 2).unwrap();
        assert_eq!(kind, SelectionKind::Line);
        assert_eq!(span.start, at_position(2, 1));
        assert_eq!(
            span.end,
            at_position(5, 1),
            "a linewise span ends at the start of the line after it, so the newline goes too"
        );
        // The last line has no newline to take, so the span ends at its end.
        let (span, _) = motion_span(&text, at_position(5, 1), Motion::LineDown, 1).unwrap();
        assert_eq!(span.end, at_position(5, 5));
    }

    #[test]
    fn a_delimited_object_is_inner_or_around() {
        let text = Lines::new("call(alpha, beta)", 1, 8);
        let (inner, _) = object_span(
            &text,
            at_position(1, 8),
            TextObject::Delimited,
            true,
            1,
            Some('('),
        )
        .unwrap();
        assert_eq!(inner.start, at_position(1, 6));
        assert_eq!(inner.end, at_position(1, 17));
        let (around, _) = object_span(
            &text,
            at_position(1, 8),
            TextObject::Delimited,
            false,
            1,
            Some('('),
        )
        .unwrap();
        assert_eq!(around.start, at_position(1, 5));
        assert_eq!(around.end, at_position(1, 18));
    }

    #[test]
    fn a_quoted_object_does_not_wrap_to_the_next_line() {
        let text = Lines::new("let s = \"hello\";\nlet t = 1;", 1, 11);
        let (inner, _) = object_span(
            &text,
            at_position(1, 11),
            TextObject::Delimited,
            true,
            1,
            Some('"'),
        )
        .unwrap();
        assert_eq!(inner.start, at_position(1, 10));
        assert_eq!(inner.end, at_position(1, 15));
    }

    #[test]
    fn a_word_object_is_the_run_under_the_cursor() {
        let text = Lines::new("alpha beta gamma", 1, 8);
        let (inner, _) =
            object_span(&text, at_position(1, 8), TextObject::Word, true, 1, None).unwrap();
        assert_eq!(
            (inner.start, inner.end),
            (at_position(1, 7), at_position(1, 11))
        );
        let (around, _) =
            object_span(&text, at_position(1, 8), TextObject::Word, false, 1, None).unwrap();
        assert_eq!(around.end, at_position(1, 12), "aw takes the space after");
    }

    #[test]
    fn the_agent_nouns_answer_nothing_rather_than_guessing() {
        // `T028` binds them and `T049` resolves them; a span invented here
        // would be a region that does not exist.
        let text = Lines::new("anything", 1, 1);
        for object in [
            TextObject::UnseenRegion,
            TextObject::Hunk,
            TextObject::Thread,
            TextObject::Block,
        ] {
            assert!(object_span(&text, at_position(1, 1), object, true, 1, None).is_none());
        }
    }

    #[test]
    fn a_paragraph_object_stops_at_the_blank_line() {
        let text = Lines::new("one\ntwo\n\nthree", 1, 1);
        let (span, kind) = object_span(
            &text,
            at_position(1, 1),
            TextObject::Paragraph,
            true,
            1,
            None,
        )
        .unwrap();
        assert_eq!(kind, SelectionKind::Line);
        assert_eq!(span.start, at_position(1, 1));
        assert_eq!(span.end, at_position(3, 1));
    }
}
