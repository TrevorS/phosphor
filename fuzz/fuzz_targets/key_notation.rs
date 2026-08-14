//! `F3` — key notation, and the round trip `.` depends on.
//!
//! `key::parse_seq` reads two things nobody in this repo chose. The obvious one
//! is `runtime/keymaps.scm`, which is user-authored and whose first line says so
//! — *"every binding in the editor is in this file"* — and which a user rewrites
//! at the `:repl` while the editor is running. The one that bites is the other:
//! `Machine::last_change` records the keys of a command, spells them with
//! [`notation_of`], and replays them through [`parse_seq`] when you press `.`.
//!
//! That round trip has already been a shipped defect. `parse_seq`'s own header
//! records it: an unclosed `<` used to be [`None`], so **`.` silently did
//! nothing after any command starting with `<`** — `<<` is dedent twice, and the
//! spelling of it could not be read back. The fallback that fixed it (a `<` that
//! opens nothing is the character) is exactly the kind of rule that is right for
//! the cases somebody thought of, and this target is the question of whether
//! those were all of them.
//!
//! # The laws
//!
//! 1. **`parse_seq` is total.** It answers `Some` for every string. The
//!    function is written to be — there is no `None` return left in it — and a
//!    reader who assumes otherwise is reading `parse`'s signature. Stated so
//!    that reintroducing a refusal is a fuzz failure rather than a silent `.`.
//! 2. **`notation_of` is a right inverse of `parse_seq`.** For any keys,
//!    `parse_seq(notation_of(keys)) == keys`. This is the law `.` rides on.
//!    `notation_of` already checks it *per key* as it builds, respelling a key
//!    that would be misread; this asserts it for the whole sequence, which is
//!    the statement the per-key check is trying to reach.
//! 3. **Notation is a normal form.** Spelling a parse and parsing it again is a
//!    fixed point: `notation_of(parse_seq(notation_of(k))) == notation_of(k)`.
//!    A `.` that replays a command *twice* must produce the same keys the
//!    second time.
//! 4. **`parse` is `parse_seq` at length one**, in both directions — the one
//!    place a caller is entitled to assume a single key, and the place a keymap
//!    entry lands.
//!
//! # This target fails today, in seconds, and that is the point
//!
//! Law 2 is **false**. The reproducer is four ASCII characters:
//!
//! ```text
//! parse_seq("< a>")      = [ '<', 'a', '>' ]        (3 keys)
//! notation_of(those)     = "<a<>>"
//! parse_seq("<a<>>")     = [ '<', 'a', '<', '>', '>' ]   (5 keys)
//! ```
//!
//! The mechanism is a gap in `unambiguous`, which has an arm for `<` (`"<lt>"`)
//! and none for `>`, so a bare `>` is respelled as `"<>>"` — and the `>` inside
//! that spelling is the same character that closes a bracket, so an **earlier**
//! unclosed `<` finds it and swallows the respelling. `notation_of`'s doc claims
//! a bracketed form *"cannot be [misread]: a bracket is consumed whole"*; it can,
//! because the respelling changes how a character *before* it parses.
//!
//! Compounding it: `notation_of` re-parses the accumulation after appending a
//! key's plain spelling, but **does not re-parse after substituting the
//! unambiguous one**, so a wrong fallback is never caught.
//!
//! `<gt>` beside the `<lt>` arm fixes this input — `parse_bracketed` already
//! reads `"gt"` — and re-checking after the substitution is what makes the
//! claim a property rather than a habit. Both are in `phosphor-core`, which is
//! `spine`'s, and this run files it as a `CONTRACT` rather than fixing it.
//!
//! **The assertion is deliberately not weakened to let the target keep
//! searching.** A fuzz target relaxed to accommodate the bug it found is the
//! coverage-floor failure in miniature: the number goes green and stops meaning
//! anything. Until `key.rs` is fixed this target reproduces in seconds and
//! searches no further, which is the correct amount of pressure.
//!
//! # Not covered by the property suite
//!
//! `crates/phosphor-core/tests/properties.rs` covers `input/text.rs` — motions,
//! objects, case changes — and does not touch `input/key.rs`. There is no
//! generated coverage of this parser at all today; the bracket/literal fallback
//! is proven by the unit tests in `key.rs` itself.
//!
//! Bytes are read as UTF-8 and non-UTF-8 inputs are dropped, which loses
//! nothing: `parse_seq` takes a `&str`, so the crate boundary above it — Steel
//! strings and recorded keystrokes — has already made that guarantee.

#![no_main]

use libfuzzer_sys::fuzz_target;
use phosphor_core::input::key::{notation_of, parse, parse_seq};
use phosphor_core::request::KeySeq;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // Law 1 — total.
    let keys = parse_seq(text).expect("parse_seq refused a string; it has no refusal left in it");

    // Law 4, one direction: `parse` is `parse_seq` at length one.
    match parse(text) {
        Some(single) => assert_eq!(
            keys,
            vec![single],
            "parse answered one key for a string parse_seq reads as {} keys",
            keys.len()
        ),
        None => assert_ne!(
            keys.len(),
            1,
            "parse refused a string that spells exactly one key"
        ),
    }

    // Law 2 — `notation_of` is a right inverse.
    let spelled: KeySeq = notation_of(&keys);
    let reparsed = parse_seq(&spelled.0).expect("parse_seq refused its own spelling");
    assert_eq!(
        reparsed,
        keys,
        "{text:?} parsed to {} keys, spelled as {:?}, and read back as {} keys — \
         `.` would replay the wrong command",
        keys.len(),
        spelled.0,
        reparsed.len()
    );

    // Law 3 — notation is a normal form.
    assert_eq!(
        notation_of(&reparsed).0,
        spelled.0,
        "spelling {:?} twice gave two different strings",
        spelled.0
    );

    // Law 4, the other direction.
    if keys.len() == 1 {
        assert_eq!(
            parse(&spelled.0),
            keys.first().copied(),
            "a one-key sequence did not read back through `parse`"
        );
    }
});
