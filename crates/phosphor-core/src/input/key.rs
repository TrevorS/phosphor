//! One keystroke, and the vim notation it is spelled in.
//!
//! **Text on the wire, a value in here.** `request::KeySeq` is *"a key sequence
//! in vim notation — `"<C-q>"`, `"SPC f"`, `"]u"`"*, and its own header says the
//! parse is `T026`'s: a structured `KeyEvent` on the wire would put crossterm's
//! shape into the MCP schema. So this module is the one parser, and it is the
//! only place that knows `<C-c>` is a control key.
//!
//! Nothing here mentions a terminal. [`Key`] is constructed by the app layer
//! from a `crossterm::event::KeyEvent` (the one crate allowed to name one) and
//! by [`parse_seq`] from text a scheme keymap or an agent wrote, and the two
//! produce the same value — which is what makes `feed-keys` *"exactly as if
//! typed"* rather than a second input path.
//!
//! # Round trip
//!
//! [`Key::notation`] and [`parse`] are inverses for every key this module can
//! spell, and `notation_round_trips` holds them to it. The spellings match what
//! `runtime/keymaps.scm` already contains (`":"`, `"<C-c>"`) — the editor layer
//! is the older writer and it does not move.
//!
//! # `T027` — one key out of two encodings
//!
//! The seam this module left named two things: *"`super`/`hyper` modifiers,
//! which are [`Mods`] bits already declared"* and the event kind. **The first
//! half of the first is not true** — [`Mods`] declares `SUPER` and has no hyper
//! bit, and none is added here: crossterm reports `HYPER` and `META` as
//! modifiers of their own, no surface in the design binds either, and a bit
//! nothing can press is a bit nothing can test. Reading the two producers
//! settled the second, and found a third thing that mattered more than both.
//!
//! **The event kind does not land here, and that is the decision.** [`Key`] is
//! what a keymap is *keyed by* — [`Eq`] and [`Hash`] are the lookup — so a
//! `kind` field would make press, repeat and release three different keys and
//! every binding would have to be written three times. Press and repeat are the
//! same keystroke to a grammar (autorepeat is how `jjjj` is typed), and release
//! is dropped by the loop before the machine sees it (`main.rs`'s `is_press`).
//! The kind stays where it is decided.
//!
//! **A shifted chord arrives in two shapes, and one [`Key`] has to come out of
//! both.** `T014` negotiates `REPORT_ALTERNATE_KEYS`
//! (`phosphor-term/src/lib.rs`), and crossterm 0.29 answers it by *replacing*
//! the code with the shifted character and **clearing the shift bit**
//! (`event/sys/unix/parse.rs:594-606`, read in this session). So
//! <kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> reaches us as `Char('K')` +
//! `CTRL` on a terminal that reports alternates and as `Char('k')` +
//! `CTRL | SHIFT` on one that does not — two spellings of one chord, and a
//! keymap can only be written in one of them. [`Key::new`] folds the first into
//! the second: **a capital under a command modifier is shift plus the base
//! letter.** Both encodings then match the one spelling a keymap uses,
//! `<C-S-k>`, which is `T027`'s acceptance criterion — `ctrl+shift+k`
//! distinguishable from `ctrl+k` — met on the primary terminal *and* on a kitty
//! terminal that answers differently.
//!
//! Only under a command modifier, because [`Mods::normalised`] holds the other
//! half: on a plain letter shift *is* the character, so `A` stays `A` and
//! typing capitals is untouched. And only for ASCII, because the shifted form
//! of anything else is layout-dependent and not recoverable — `!` is not
//! `<S-1>` on every keyboard, so a non-letter chord is spelled by the glyph the
//! terminal actually sent (`<C-!>`), which is vim's own rule.
//!
//! **What the legacy encoding cannot say.** Without the protocol a terminal
//! sends one control byte for <kbd>ctrl</kbd>+<kbd>k</kbd> and
//! <kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> alike: the shift is not lost
//! by us, it never arrives. That is why [`Protocol`] exists and why the
//! degradation lives in the machine ([`Machine::set_protocol`]) rather than
//! here — nothing this module can do invents a bit the wire does not carry.
//!
//! [`Machine::set_protocol`]: super::Machine::set_protocol

use core::fmt;

use crate::request::KeySeq;

/// A key that is not a character.
///
/// Spelled lowercase inside angle brackets — `<esc>`, `<cr>`, `<pagedown>` —
/// because that is how `runtime/keymaps.scm` and the mockups already write them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Named {
    /// `<esc>`.
    Esc,
    /// `<cr>` — return.
    Enter,
    /// `<tab>`.
    Tab,
    /// `<bs>` — backspace.
    Backspace,
    /// `<del>` — forward delete.
    Delete,
    /// `<ins>`.
    Insert,
    /// `<left>`.
    Left,
    /// `<right>`.
    Right,
    /// `<up>`.
    Up,
    /// `<down>`.
    Down,
    /// `<home>`.
    Home,
    /// `<end>`.
    End,
    /// `<pageup>`.
    PageUp,
    /// `<pagedown>`.
    PageDown,
    /// `<f1>` … `<f12>`.
    Function(u8),
}

impl Named {
    /// The name between the brackets, without the modifiers.
    ///
    /// [`Named::Function`] has no fixed string, so it answers [`None`] and
    /// [`Key::notation`] spells it from the number.
    #[must_use]
    pub const fn word(self) -> Option<&'static str> {
        let word = match self {
            Self::Esc => "esc",
            Self::Enter => "cr",
            Self::Tab => "tab",
            Self::Backspace => "bs",
            Self::Delete => "del",
            Self::Insert => "ins",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "pageup",
            Self::PageDown => "pagedown",
            Self::Function(_) => return None,
        };
        Some(word)
    }

    /// The key a bracketed word names, if any.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        let named = match word {
            "esc" | "escape" => Self::Esc,
            "cr" | "enter" | "return" => Self::Enter,
            "tab" => Self::Tab,
            "bs" | "backspace" => Self::Backspace,
            "del" | "delete" => Self::Delete,
            "ins" | "insert" => Self::Insert,
            "left" => Self::Left,
            "right" => Self::Right,
            "up" => Self::Up,
            "down" => Self::Down,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            other => {
                let number = other.strip_prefix('f')?.parse::<u8>().ok()?;
                if (1..=12).contains(&number) {
                    Self::Function(number)
                } else {
                    return None;
                }
            }
        };
        Some(named)
    }
}

/// What was pressed, before the modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Code {
    /// A character, including `<space>`, which spells itself.
    Char(char),
    /// Everything else.
    Named(Named),
}

/// The modifiers held with a key.
///
/// A hand-rolled bit set rather than `bitflags`: `phosphor-core` is
/// deliberately dependency-free at the floor (its manifest), and four bits do
/// not justify the first entry in that table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Mods(u8);

impl Mods {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// `<C-…>`.
    pub const CTRL: Self = Self(1);
    /// `<A-…>` — alt / meta.
    pub const ALT: Self = Self(2);
    /// `<S-…>`.
    pub const SHIFT: Self = Self(4);
    /// `<D-…>` — super / command. Unreachable without the kitty protocol, and
    /// declared here because `T027` is what makes it reachable.
    pub const SUPER: Self = Self(8);

    /// The union of two sets.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every bit of `other` is set here.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether nothing is held.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Drops the bits a character already encodes.
    ///
    /// **The rule `T027` inherits.** A terminal reports `A` as shift + `a` or
    /// as `A` depending on the protocol, and a keymap written `A` must match
    /// both. So shift is dropped from a character key that is holding nothing
    /// else — the character carries it — and kept everywhere else, which is
    /// what leaves `<C-S-k>` distinguishable from `<C-k>`.
    #[must_use]
    pub const fn normalised(self, code: Code) -> Self {
        if matches!(code, Code::Char(_)) && self.0 == Self::SHIFT.0 {
            Self::NONE
        } else {
            self
        }
    }

    /// Whether any of ctrl, alt or super is held — *"this is a command, not
    /// text"*.
    ///
    /// The boundary both halves of `T027`'s rule are drawn at: shift folds into
    /// the character below it and stays a modifier above it.
    #[must_use]
    pub const fn commanding(self) -> bool {
        self.0 & (Self::CTRL.0 | Self::ALT.0 | Self::SUPER.0) != 0
    }
}

/// Which keyboard encoding the keystrokes are arriving in.
///
/// The editor's copy of `phosphor_term::KeyboardProtocol`, which is what
/// negotiation settled (`T014`). Two enums for one fact because
/// `phosphor-core` is dependency-free at the floor and never names a terminal;
/// the host maps one to the other in the same line that builds the machine.
///
/// It changes exactly one thing — [`Protocol::Legacy`] turns on the chord
/// fallback in [`Machine::set_protocol`] — and the default is
/// [`Protocol::Kitty`] on purpose: a machine nobody configured behaves as
/// though nothing is lost, so the fallback can only ever be switched *on* by a
/// host that has been told the terminal loses information. The other default
/// would make an unconfigured machine guess, and guessing wrong fires the wrong
/// binding on a terminal that was telling the truth.
///
/// [`Machine::set_protocol`]: super::Machine::set_protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Protocol {
    /// The kitty keyboard protocol is active: a modifier the user held is a
    /// modifier we were told about.
    #[default]
    Kitty,
    /// The traditional encoding. `<C-k>` and `<C-S-k>` are the same byte on the
    /// wire, so they are the same keystroke here.
    Legacy,
}

/// One keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// What was pressed.
    pub code: Code,
    /// What was held with it.
    pub mods: Mods,
}

impl Key {
    /// A key with no modifiers.
    #[must_use]
    pub const fn plain(code: Code) -> Self {
        Self {
            code,
            mods: Mods::NONE,
        }
    }

    /// A character with no modifiers — the common case in a keymap.
    #[must_use]
    pub const fn char(character: char) -> Self {
        Self::plain(Code::Char(character))
    }

    /// A named key with no modifiers.
    #[must_use]
    pub const fn named(named: Named) -> Self {
        Self::plain(Code::Named(named))
    }

    /// A key with modifiers, in the one spelling a keymap is written in.
    ///
    /// **The whole of `T027`'s decode rule, in the one constructor both
    /// producers use** — the app layer's `crossterm::KeyEvent` conversion
    /// (`main.rs`'s `decode`) and [`parse_bracketed`] below. Two steps, and the
    /// module header argues both:
    ///
    /// 1. A capital under ctrl, alt or super is folded to shift plus the base
    ///    letter, because a terminal reporting alternate keys sends the shifted
    ///    character *instead of* the shift bit.
    /// 2. [`Mods::normalised`] then drops a lone shift from a character, because
    ///    there the character already carries it.
    ///
    /// One consequence worth knowing when writing a keymap: `<C-K>` and
    /// `<C-S-k>` are the same key, and neither is `<C-k>`. Vim treats the first
    /// and the third as one; a terminal that can tell them apart is newer than
    /// that rule, and telling them apart is the point of the task.
    #[must_use]
    pub const fn new(code: Code, mods: Mods) -> Self {
        let (code, mods) = match code {
            Code::Char(character) if character.is_ascii_uppercase() && mods.commanding() => (
                Code::Char(character.to_ascii_lowercase()),
                mods.with(Mods::SHIFT),
            ),
            _ => (code, mods),
        };
        Self {
            code,
            mods: mods.normalised(code),
        }
    }

    /// The same keystroke with shift held.
    ///
    /// What a [`Protocol::Legacy`] terminal could not tell us: it sends one
    /// byte for `<C-k>` and `<C-S-k>`, so this is how the machine asks the
    /// second question after the first comes back unbound. Goes through
    /// [`Key::new`], so a plain character stays itself — shift on `a` is `A`,
    /// not a modifier.
    #[must_use]
    pub const fn shifted(self) -> Self {
        Self::new(self.code, self.mods.with(Mods::SHIFT))
    }

    /// The character this key would type, or [`None`] if it types nothing.
    ///
    /// Control and super are commands, never text; alt is left out for the same
    /// reason (`<A-x>` is a binding, not an `x`). Shift is already in the
    /// character.
    #[must_use]
    pub const fn typed(self) -> Option<char> {
        match self.code {
            Code::Char(character)
                if !self.mods.has(Mods::CTRL)
                    && !self.mods.has(Mods::ALT)
                    && !self.mods.has(Mods::SUPER) =>
            {
                Some(character)
            }
            _ => None,
        }
    }

    /// This key in vim notation.
    #[must_use]
    pub fn notation(self) -> String {
        let bare = match self.code {
            Code::Char(' ') => "space".to_owned(),
            Code::Char(character) if self.mods.is_empty() => return character.to_string(),
            Code::Char(character) => character.to_string(),
            Code::Named(Named::Function(number)) => format!("f{number}"),
            Code::Named(named) => named.word().unwrap_or("?").to_owned(),
        };
        let mut spelled = String::from("<");
        for (bit, prefix) in [
            (Mods::CTRL, "C-"),
            (Mods::ALT, "A-"),
            (Mods::SHIFT, "S-"),
            (Mods::SUPER, "D-"),
        ] {
            if self.mods.has(bit) {
                spelled.push_str(prefix);
            }
        }
        spelled.push_str(&bare);
        spelled.push('>');
        spelled
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.notation())
    }
}

/// Parses one key in vim notation, or [`None`] if it spells nothing.
#[must_use]
pub fn parse(text: &str) -> Option<Key> {
    let mut keys = parse_seq(text)?;
    if keys.len() == 1 { keys.pop() } else { None }
}

/// Parses a whole sequence — `"]u"`, `"SPC f"`, `"<C-w>v"`.
///
/// Three token shapes, in this order:
///
/// * `<…>` — one bracketed key, modifiers first.
/// * `SPC` — the leader as `3c` and the Design Language spell it. Uppercase and
///   exactly three characters, so a keymap can still bind the letters `S`, `P`
///   and `C` beside it.
/// * anything else — one character.
///
/// A run of ASCII spaces separates tokens and is never itself a key; `<space>`
/// is. Answers [`None`] on an unclosed bracket or an unknown bracketed word,
/// because a keymap entry nobody can press is a typo worth reporting rather
/// than a binding worth keeping.
#[must_use]
pub fn parse_seq(text: &str) -> Option<Vec<Key>> {
    let chars: Vec<char> = text.chars().collect();
    let mut keys = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            ' ' => at += 1,
            '<' => {
                let close = chars[at..].iter().position(|&c| c == '>')? + at;
                let inside: String = chars[at + 1..close].iter().collect();
                keys.push(parse_bracketed(&inside)?);
                at = close + 1;
            }
            _ if chars[at..].starts_with(&['S', 'P', 'C']) => {
                keys.push(Key::char(' '));
                at += 3;
            }
            character => {
                keys.push(Key::char(character));
                at += 1;
            }
        }
    }
    Some(keys)
}

/// A whole [`KeySeq`] as keys, for `feed-keys`.
#[must_use]
pub fn parse_key_seq(seq: &KeySeq) -> Option<Vec<Key>> {
    parse_seq(&seq.0)
}

/// The sequence back as one [`KeySeq`], for a receipt or a which-key row.
#[must_use]
pub fn notation_of(keys: &[Key]) -> KeySeq {
    KeySeq(keys.iter().map(|key| key.notation()).collect())
}

/// The inside of a `<…>` token.
fn parse_bracketed(inside: &str) -> Option<Key> {
    let mut mods = Mods::NONE;
    let mut rest = inside;
    loop {
        let (bit, tail) = match rest.split_at_checked(2) {
            Some(("C-", tail)) => (Mods::CTRL, tail),
            Some(("A-" | "M-", tail)) => (Mods::ALT, tail),
            Some(("S-", tail)) => (Mods::SHIFT, tail),
            Some(("D-", tail)) => (Mods::SUPER, tail),
            _ => break,
        };
        mods = mods.with(bit);
        rest = tail;
    }
    let lowered = rest.to_ascii_lowercase();
    let code = match lowered.as_str() {
        "space" => Code::Char(' '),
        "lt" => Code::Char('<'),
        "gt" => Code::Char('>'),
        "bslash" => Code::Char('\\'),
        _ => {
            let mut characters = rest.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Code::Char(character),
                _ => Code::Named(Named::from_word(&lowered)?),
            }
        }
    };
    Some(Key::new(code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation_round_trips() {
        for key in [
            Key::char('q'),
            Key::char(']'),
            Key::char(' '),
            Key::char('A'),
            Key::new(Code::Char('c'), Mods::CTRL),
            Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT)),
            Key::named(Named::Esc),
            Key::named(Named::Enter),
            Key::named(Named::Function(5)),
            Key::new(Code::Named(Named::Left), Mods::ALT),
        ] {
            let spelled = key.notation();
            assert_eq!(parse(&spelled), Some(key), "{spelled}");
        }
    }

    #[test]
    fn the_spellings_the_editor_layer_already_uses() {
        // `runtime/keymaps.scm` binds `":"`, and `6b`'s footer names `C-c`.
        // Those two are older than this module and it is this module that has
        // to match them.
        assert_eq!(parse(":"), Some(Key::char(':')));
        assert_eq!(parse("<C-c>"), Some(Key::new(Code::Char('c'), Mods::CTRL)));
        assert_eq!(parse("<esc>"), Some(Key::named(Named::Esc)));
        assert_eq!(parse("<space>"), Some(Key::char(' ')));
        assert_eq!(Key::char(' ').notation(), "<space>");
    }

    #[test]
    fn a_sequence_is_read_the_way_a_keymap_is_written() {
        assert_eq!(parse_seq("]u"), Some(vec![Key::char(']'), Key::char('u')]));
        // `3c`'s leader, both spellings.
        assert_eq!(
            parse_seq("SPC f"),
            Some(vec![Key::char(' '), Key::char('f')])
        );
        assert_eq!(
            parse_seq("<space>f"),
            Some(vec![Key::char(' '), Key::char('f')])
        );
        assert_eq!(
            parse_seq("<C-w>v"),
            Some(vec![Key::new(Code::Char('w'), Mods::CTRL), Key::char('v')])
        );
        // Unclosed and unknown are refused rather than silently dropped.
        assert_eq!(parse_seq("<C-w"), None);
        assert_eq!(parse_seq("<nope>"), None);
    }

    #[test]
    fn shift_is_part_of_the_character_and_a_modifier_everywhere_else() {
        // The rule `T027` inherits: a terminal that reports `A` as shift + `a`
        // and one that reports `A` must produce the same key…
        assert_eq!(Key::new(Code::Char('A'), Mods::SHIFT), Key::char('A'));
        // …and `ctrl+shift+k` must stay distinguishable from `ctrl+k`, which is
        // `T027`'s own acceptance criterion.
        assert_ne!(
            Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT)),
            Key::new(Code::Char('k'), Mods::CTRL)
        );
        assert_eq!(
            Key::new(Code::Named(Named::Tab), Mods::SHIFT).notation(),
            "<S-tab>"
        );
    }

    #[test]
    fn the_two_encodings_of_a_shifted_chord_are_one_key() {
        // `T027`'s acceptance, at the level this module owns it. Kitty with
        // REPORT_ALTERNATE_KEYS puts the shifted character in the code and
        // clears the shift bit (crossterm `event/sys/unix/parse.rs:594-606`);
        // kitty without it, and the CSI-u path generally, sets the bit and
        // leaves the base letter. Both are `<C-S-k>`.
        let reported_as_alternate = Key::new(Code::Char('K'), Mods::CTRL);
        let reported_as_modifier = Key::new(Code::Char('k'), Mods::CTRL.with(Mods::SHIFT));
        assert_eq!(reported_as_alternate, reported_as_modifier);
        assert_eq!(reported_as_alternate.notation(), "<C-S-k>");
        // …and neither is ctrl+k, which is the criterion itself.
        assert_ne!(
            reported_as_alternate,
            Key::new(Code::Char('k'), Mods::CTRL),
            "ctrl+shift+k must stay distinguishable from ctrl+k"
        );
        // A keymap may spell it either way and reach the same binding.
        assert_eq!(parse("<C-S-k>"), Some(reported_as_alternate));
        assert_eq!(parse("<C-K>"), Some(reported_as_alternate));
    }

    #[test]
    fn folding_stops_where_the_character_carries_the_shift() {
        // Typing capitals is untouched: no command modifier, no fold.
        assert_eq!(Key::new(Code::Char('A'), Mods::SHIFT), Key::char('A'));
        assert_eq!(Key::char('A').notation(), "A");
        // Alt keeps its own shift, because ESC-prefixed input carries the case
        // even under the legacy encoding — `<A-K>` is alt+shift+k either way.
        assert_eq!(
            Key::new(Code::Char('K'), Mods::ALT).notation(),
            "<A-S-k>",
            "a capital under alt is shift too"
        );
        // Non-ASCII and punctuation are spelled by the glyph the terminal sent:
        // the shifted form of a key is layout-dependent and not recoverable.
        assert_eq!(
            Key::new(Code::Char('!'), Mods::CTRL).notation(),
            "<C-!>",
            "punctuation keeps the glyph — vim's rule"
        );
        assert_eq!(Key::new(Code::Char('Ä'), Mods::CTRL).notation(), "<C-Ä>");
        // A named key was never a character, so shift stays a modifier on it.
        assert_eq!(
            Key::new(Code::Named(Named::Tab), Mods::SHIFT).notation(),
            "<S-tab>"
        );
    }

    #[test]
    fn shifted_is_the_question_a_legacy_terminal_makes_the_machine_ask() {
        assert_eq!(
            Key::new(Code::Char('k'), Mods::CTRL).shifted(),
            parse("<C-S-k>").expect("a spelling this test wrote")
        );
        // Idempotent, so a retry cannot walk further away from the key pressed.
        let chord = Key::new(Code::Char('k'), Mods::CTRL).shifted();
        assert_eq!(chord.shifted(), chord);
        // And on a plain letter it is still just the letter.
        assert_eq!(Key::char('a').shifted(), Key::char('a'));
    }

    #[test]
    fn the_unconfigured_protocol_assumes_nothing_is_lost() {
        // The default decides what an unconfigured machine does, and the only
        // safe answer is "no fallback": the fallback can fire the wrong binding
        // on a terminal that was telling the truth.
        assert_eq!(Protocol::default(), Protocol::Kitty);
    }

    #[test]
    fn only_a_key_that_types_something_types_something() {
        assert_eq!(Key::char('a').typed(), Some('a'));
        assert_eq!(Key::char(' ').typed(), Some(' '));
        assert_eq!(Key::new(Code::Char('a'), Mods::CTRL).typed(), None);
        assert_eq!(Key::named(Named::Enter).typed(), None);
    }
}
