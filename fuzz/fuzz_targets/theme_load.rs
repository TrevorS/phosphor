//! `F4` — the theme loader, over a user's file.
//!
//! `Theme::load` is the only parser in the build whose input is a file the user
//! wrote by hand, in a format borrowed from somebody else's ecosystem: base16
//! files are downloaded, generated, and edited in the middle. Its own header
//! says *"Unknown keys, duplicate keys and missing keys are all errors; a theme
//! is a complete palette or it is not a theme"*, and it declares nineteen error
//! variants — a parser that has clearly thought about malformed input. A fuzzer
//! is how you find out whether it thought of everything.
//!
//! The interesting inputs are not random bytes; they are a *valid theme with
//! something wrong with it*. So `seeds/theme_load/` holds the six shipped
//! `.theme` files verbatim, and libFuzzer mutates outward from them.
//!
//! # Where the prey is
//!
//! Four `expect`s, each reasoning about what an earlier pass already rejected —
//! `slots[slot as usize].expect("checked above")` in `Theme::load`,
//! `.expect("missing keys already rejected")` and two `.expect("checked")` in
//! `validate_actors`. Every one of them is a claim that the missing-key sweep
//! ran first and covered exactly these slots, and that claim is about the
//! interaction of two tables (`SLOT_KEYS`, `ACTOR_SLOTS`) with a parse loop.
//!
//! And the validator does float arithmetic on parsed values — `chroma`,
//! `hue_degrees`, `HueFamily::contains` on an arc that may wrap through 0° —
//! then formats the results into a message. A hue of `NaN` compares false
//! against every bound, which is a rejection rather than a panic, but it is a
//! rejection whose *message* says the colour looks like nothing.
//!
//! # The laws
//!
//! 1. **It answers.** `Ok` or a `ThemeError`, for any source, never a panic. A
//!    panic here is an editor that will not start because of a file the user
//!    can fix but cannot see.
//! 2. **A rejection points at a line that exists.** `ThemeError::line` is
//!    documented as *"1-based line, or `None` for whole-file problems"*. An
//!    error naming line 47 of a twelve-line file is a real defect — it is what
//!    an off-by-one in the enumerate loop produces, and it is invisible to a
//!    test that only checks the variant.
//! 3. **The message renders.** `Display` is where the float values reach a
//!    formatter, and it must produce something non-empty for every rejection.
//! 4. **An accepted theme has six distinct actors.** This is Design Language
//!    §10's contract restated on the *output* rather than trusted from the
//!    check that is supposed to enforce it — the copy-paste failure the loader's
//!    header calls *"the one that actually destroys the at-a-glance read"*.
//!
//! # Not covered by the property suite
//!
//! Nothing generated reaches this file. `crates/phosphor-ui`'s only `proptest!`
//! block is `status_line.rs`'s width property; the theme loader's tests are the
//! hand-written ones in `load.rs` itself and the shipped-theme checks in
//! `builtin.rs`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use phosphor_ui::theme::{Theme, ThemeErrorKind};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    // Law 1 — it answers.
    let theme = match Theme::load("fuzz.theme", source) {
        Ok(theme) => theme,
        Err(error) => {
            // Law 2 — a rejection points at a line that exists.
            if let Some(line) = error.line {
                let lines = source.lines().count();
                assert!(
                    line >= 1 && line <= lines,
                    "{:?} blamed line {line} of a source with {lines} lines",
                    error.kind
                );
            } else {
                // The documented `None` case is the whole-file one. Stated so
                // that a per-line problem losing its line is a failure rather
                // than a quietly worse message.
                assert!(
                    matches!(*error.kind, ThemeErrorKind::MissingKeys { .. }),
                    "{:?} is a per-line problem reported without a line",
                    error.kind
                );
            }
            // Law 3 — the message renders.
            let rendered = error.to_string();
            assert!(
                !rendered.trim().is_empty(),
                "{:?} rendered to nothing",
                error.kind
            );
            return;
        }
    };

    // Law 4 — six actors, six colours.
    let actors = [
        ("claude", theme.actors.claude),
        ("you", theme.actors.you),
        ("attention", theme.actors.attention),
        ("trouble", theme.actors.trouble),
        ("transient", theme.actors.transient),
        ("steel", theme.actors.steel),
    ];
    for (i, (first, a)) in actors.iter().enumerate() {
        for (second, b) in actors.iter().skip(i + 1) {
            assert!(
                a != b,
                "an accepted theme gives {first} and {second} the same colour"
            );
        }
    }
    assert!(
        !theme.name.trim().is_empty(),
        "an accepted theme has an empty name"
    );
});
