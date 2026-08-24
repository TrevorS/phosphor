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
//! (`request::TextObject`'s own header). `T028` binds them, they no-op cleanly
//! here, and `T049` is where they resolve — by giving [`Text`] a neighbour that
//! can answer a region query, not by teaching this file about regions.
//!
//! This read *"there is no store until `T041`"*. There is one now, and the
//! sentence above is why that changed nothing here: the reason these four
//! answer [`None`] was never that the store was missing, it is that this file
//! is syntax and they are not.
//!
//! # Cost
//!
//! [`Text::line`] returns an owned `String`, which is an allocation per line
//! touched. A motion touches one line and a paragraph motion touches until it
//! finds a blank one; at a keystroke's rate that is free, and the alternative —
//! handing out a borrow — makes the trait unimplementable over a rope that
//! stores lines in pieces (`ropey`'s `RopeSlice` is not `&str`).

use crate::request::{CaseChange, Motion, Position, SelectionKind, Span, TextObject};

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

    /// The unseen region covering a position, if the store holds one
    /// (`T049`).
    ///
    /// **The seam `6d`'s agent nouns were waiting for**, and it is here rather
    /// than as a parameter on [`object_span`] because this trait is already
    /// *"what the machine may ask about the buffer"* — the machine holds one of
    /// these and nothing else. A parameter would have to be threaded through
    /// every caller including three test helpers, to carry a fact only one
    /// object reads.
    ///
    /// Defaults to [`None`], which is what a headless driver answers and what
    /// `viu` then degrades to: selecting nothing rather than selecting wrongly.
    /// The binary's implementation reads the same store the gutter draws from,
    /// so the noun and the marker cannot disagree.
    ///
    /// **Only the unseen region, of `6d`'s four.** A hunk needs `T063`, a
    /// thread `T068` and a review block `T053` — none of those stores exists,
    /// so their objects still answer `None` and say which task builds them.
    fn unseen_at(&self, at: Position) -> Option<Span> {
        let _ = at;
        None
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
///
/// The four find motions need a character, which this form does not have; they
/// stay put here and answer through [`cursor_after_with_target`] instead. See
/// [`Motion`]'s own header for why the character does not ride on the tag.
#[must_use]
pub fn cursor_after(text: &dyn Text, from: Position, motion: Motion, count: u32) -> Position {
    cursor_after_with_target(text, from, motion, count, None)
}

/// [`cursor_after`], with the character `f`, `F`, `t` and `T` search for.
///
/// The machine holds that character between the `f` and the key that names it
/// and passes it here; every other motion ignores it.
#[must_use]
pub fn cursor_after_with_target(
    text: &dyn Text,
    from: Position,
    motion: Motion,
    count: u32,
    target: Option<char>,
) -> Position {
    let count = count.max(1);
    if is_find(motion) {
        // Counted as a whole rather than stepped: `3ta` is *the third* `a`,
        // then one back — stepping a till motion would stall on the character
        // it already stopped before.
        return match target {
            Some(character) => find_char(text, from, motion, character, count),
            None => from,
        };
    }
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

/// Whether this motion is one of the four that need a character.
///
/// `;` and `,` are not among them: the machine resolves them to the find they
/// repeat *before* asking, because the last find is state the machine holds and
/// this module is a view of one buffer with no memory.
#[must_use]
pub const fn is_find(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::FindCharForward
            | Motion::FindCharBackward
            | Motion::TillCharForward
            | Motion::TillCharBackward
    )
}

/// `text` with its letters upper-cased, lower-cased or toggled.
///
/// **One definition of what `~` means**, so the host applying
/// `Buffer::SetCase` and any test driving it cannot differ. Non-letters are
/// untouched, and a letter with no other case is itself.
#[must_use]
pub fn cased(text: &str, case: CaseChange) -> String {
    text.chars()
        .flat_map(|character| {
            let cased: Box<dyn Iterator<Item = char>> = match case {
                CaseChange::Upper => Box::new(character.to_uppercase()),
                CaseChange::Lower => Box::new(character.to_lowercase()),
                CaseChange::Toggle if character.is_lowercase() => {
                    Box::new(character.to_uppercase())
                }
                CaseChange::Toggle if character.is_uppercase() => {
                    Box::new(character.to_lowercase())
                }
                CaseChange::Toggle => Box::new(core::iter::once(character)),
            };
            cased
        })
        .collect()
}

/// `f`, `F`, `t`, `T` — the one place the off-by-one that separates them lives.
///
/// **On this line only**, which is vim's rule and the reason `dt)` is safe at
/// the end of a line. A count that cannot be met leaves the cursor where it
/// started: the whole motion fails, rather than stopping at the second of three
/// asked-for occurrences.
fn find_char(
    text: &dyn Text,
    from: Position,
    motion: Motion,
    target: char,
    count: u32,
) -> Position {
    let row: Vec<char> = match text.line(from.line) {
        Some(row) => row.chars().collect(),
        None => return from,
    };
    let forwards = matches!(motion, Motion::FindCharForward | Motion::TillCharForward);
    // 0-based, so the arithmetic below is the same in both directions.
    let mut at = (from.column as usize).saturating_sub(1);
    for _ in 0..count.max(1) {
        let found = if forwards {
            (at + 1..row.len()).find(|index| row[*index] == target)
        } else {
            (0..at).rev().find(|index| row[*index] == target)
        };
        match found {
            Some(index) => at = index,
            None => return from,
        }
    }
    // `t` stops one short of what `f` lands on, in whichever direction it ran.
    let column = match motion {
        Motion::TillCharForward => at,
        Motion::TillCharBackward => at + 2,
        _ => at + 1,
    };
    Position {
        line: from.line,
        column: u32::try_from(column).unwrap_or(1).max(1),
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
        Motion::WordForward => word_forward(text, from, false),
        Motion::WordBackward => word_backward(text, from, false),
        Motion::WordEnd => word_end(text, from, false),
        Motion::BigWordForward => word_forward(text, from, true),
        Motion::BigWordBackward => word_backward(text, from, true),
        Motion::BigWordEnd => word_end(text, from, true),
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
        // Reached only through a door: the machine resolves a find to its
        // character before it asks, and `;` to the find it repeats.
        Motion::FindCharForward
        | Motion::FindCharBackward
        | Motion::TillCharForward
        | Motion::TillCharBackward
        | Motion::RepeatFind
        | Motion::RepeatFindReverse => from,
    }
}

/// What a character counts as, with `W`'s coarser reading as an option.
///
/// **The one difference between `w` and `W`**, and it is a classifier rather
/// than a second walk: a big word is *anything not blank*, so punctuation stops
/// being a run of its own. [`word_object`] already read the buffer this way for
/// `iW`; the motions now share the rule instead of restating it.
fn classify(character: char, big: bool) -> Class {
    match class(character) {
        Class::Punct if big => Class::Word,
        other => other,
    }
}

fn word_forward(text: &dyn Text, from: Position, big: bool) -> Position {
    let mut position = from;
    if let Some(start) = at(text, position)
        && classify(start, big) != Class::Blank
    {
        let start = classify(start, big);
        while let Some(next) = forward(text, position) {
            if at(text, next).map(|c| classify(c, big)) == Some(start) {
                position = next;
            } else {
                position = next;
                break;
            }
        }
    }
    while at(text, position).map(|c| classify(c, big)) == Some(Class::Blank) {
        match forward(text, position) {
            Some(next) => position = next,
            None => break,
        }
    }
    position
}

fn word_backward(text: &dyn Text, from: Position, big: bool) -> Position {
    let Some(mut position) = backward(text, from) else {
        return from;
    };
    while at(text, position).map(|c| classify(c, big)) == Some(Class::Blank) {
        match backward(text, position) {
            Some(previous) => position = previous,
            None => return position,
        }
    }
    let run = at(text, position).map(|c| classify(c, big));
    while let Some(previous) = backward(text, position) {
        if at(text, previous).map(|c| classify(c, big)) == run {
            position = previous;
        } else {
            break;
        }
    }
    position
}

fn word_end(text: &dyn Text, from: Position, big: bool) -> Position {
    let Some(mut position) = forward(text, from) else {
        return from;
    };
    while at(text, position).map(|c| classify(c, big)) == Some(Class::Blank) {
        match forward(text, position) {
            Some(next) => position = next,
            None => return position,
        }
    }
    let run = at(text, position).map(|c| classify(c, big));
    while let Some(next) = forward(text, position) {
        if at(text, next).map(|c| classify(c, big)) == run {
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

/// `l` as an *operand*, which reaches one place `l` as a motion may not.
///
/// **The newline boundary is the limit here, and the last character is the
/// limit in [`step`].** vim spells `x` as `dl` and `~` as `g~l` with
/// `'notildeop'` off (`vim91/doc/change.txt:31-33`, `:315-318`), so `3x` on a
/// three-character line has to delete three characters — and it cannot, if the
/// operand stops where a normal-mode cursor stops. `l` is exclusive
/// (`motion.txt:189`) and left-right motions "stop at the first column and at
/// the end of the line" (`motion.txt:170-171`); for an operator, *the end of
/// the line* is the boundary past the last character, not the character itself.
///
/// It stays on one line, which is vim's default `'whichwrap'`: `5x` at two
/// characters from the end takes those two and does not join the next line.
/// Fixing it here rather than per-key is what makes `x`, `X`, `s`, `~` and
/// `d3l` agree — they are one rule wearing five spellings.
fn char_right_operand(text: &dyn Text, from: Position, count: u32) -> Position {
    Position {
        column: from
            .column
            .saturating_add(count.max(1))
            .min(width(text, from.line).saturating_add(1)),
        ..from
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
    motion_span_with_target(text, from, motion, count, None)
}

/// [`motion_span`], with the character `f`, `F`, `t` and `T` search for.
///
/// [`None`] when the find does not land — `dfx` with no `x` on the line leaves
/// the operator's operand unresolved, which the machine turns into a cancelled
/// `d` rather than a delete of something else.
#[must_use]
pub fn motion_span_with_target(
    text: &dyn Text,
    from: Position,
    motion: Motion,
    count: u32,
    target: Option<char>,
) -> Option<(Span, SelectionKind)> {
    if matches!(
        motion,
        Motion::SearchNext | Motion::SearchPrev | Motion::RepeatFind | Motion::RepeatFindReverse
    ) {
        return None;
    }
    let to = if motion == Motion::CharRight {
        char_right_operand(text, from, count)
    } else {
        cursor_after_with_target(text, from, motion, count, target)
    };
    // A find that did not land is not a span of zero characters: `dfx` with no
    // `x` ahead of the cursor deletes nothing at all.
    if is_find(motion) && to == from {
        return None;
    }
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
    // A span of no characters is not an operand. `x` on an empty line, `X` and
    // `d0` in column 1: vim beeps at each and changes nothing, and the
    // alternative here is a `Delete` of nothing that still closes an undo group
    // — an empty step in `T029`'s tree, and a `.` that repeats it.
    if start == end {
        return None;
    }
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

/// Whether an operator takes the character its motion lands on.
///
/// The forward finds are inclusive and the backward ones are not, which is
/// vim's rule and is what makes `dfx` swallow the `x` while `dFx` deletes back
/// *to* it and leaves the character under the cursor alone.
const fn inclusive(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::WordEnd
            | Motion::BigWordEnd
            | Motion::LineEnd
            | Motion::MatchingBracket
            | Motion::FindCharForward
            | Motion::TillCharForward
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
        // `T049`. `viu` — *"select the unseen region under the cursor"* —
        // through [`Text::unseen_at`], which is the seam this file was missing
        // when it said so. Linewise, because a region is a span of *rows* and
        // §7 tints whole rows: a characterwise selection of one would put the
        // cursor mid-line and leave the rest of the row out of the operator.
        //
        // `inner` and `around` are the same span. A region has no delimiters to
        // be inside or outside of, and inventing a one-row margin for `vau`
        // would be a second meaning nobody asked for.
        TextObject::UnseenRegion => text
            .unseen_at(at_position)
            .map(|span| (span, SelectionKind::Line)),
        // A markup tag needs the grammar (`T037`); the other three need a store
        // that does not exist yet — hunks are `T064`, threads `T068`, review
        // blocks `T053`. `T063` drew the hunk *widget* and is ticked, which
        // moves nothing here: a hunk this can select is one with an id and a
        // seen bit, and that is the store `T064` builds. They stay `None`
        // rather than guessing, which is what makes `vih` select nothing
        // instead of selecting something wrong.
        TextObject::Tag | TextObject::Hunk | TextObject::Thread | TextObject::Block => None,
    }
}

fn word_object(
    text: &dyn Text,
    from: Position,
    inner: bool,
    big: bool,
) -> Option<(Span, SelectionKind)> {
    let run = classify(at(text, from)?, big);
    let mut start = from;
    while let Some(previous) = backward(text, start) {
        if previous.line != start.line || at(text, previous).map(|c| classify(c, big)) != Some(run)
        {
            break;
        }
        start = previous;
    }
    let mut end = from;
    while let Some(next) = forward(text, end) {
        if next.line != end.line || at(text, next).map(|c| classify(c, big)) != Some(run) {
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
