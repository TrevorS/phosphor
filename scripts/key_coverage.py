#!/usr/bin/env python3
"""Every key and every ex command the layer binds is pressed by some test.

# Why this exists

`crates/phosphor/tests/loop_pty.rs` carried the audit in a comment:

    * **Ex commands.** `(ex-entries)` answers **18**. Nine were typed by no
      test at all: `wall`, `wq`, `xit`, `close-buffer`, `transcript`, `inbox`,
      `diff-disk`, `reattach`, `comment`. Three of those are live.
    * **Mouse.** `mouse_actions` handles three kinds — press, drag, wheel —
      and one test pressed the first two. The wheel had nothing.

That survey was real and it found real holes, several of which were then
filled. But it was counted **by hand, once**, and written into prose — which is
the exact shape `doc_claims.py` exists to stop everywhere else in this
repository. The numbers in it are already stale: this script counts 17
`ex-set!` forms where the comment says 18.

A hand-count of coverage rots the first time somebody adds a binding, and it
rots *silently* — a new key with no test looks exactly like an old key with no
test, which is to say like nothing at all.

# What is checked, and what "pressed" is allowed to mean

Bindings come from `runtime/keymaps.scm`: `keymap-set!` forms and the `(list
"keys" …)` rows of its tables. Ex commands come from `ex-set!`, whose vim-style
`w[rite]` spelling means the name is `write` and any prefix from `w` up is
accepted.

A key is **pressed** when some `b"…"` literal in `crates/phosphor/tests/*.rs`
contains its bytes *and that literal is short*. The length bound is the whole
honesty of this check: without it, `H` is "covered" by any test that types
`b"iHello"` into a buffer, and the lint congratulates itself for prose. Six
bytes is long enough for `SPC u s` plus a terminator and short enough that no
sentence qualifies.

Notations that are not literal bytes — `<left>`, `<cr>`, `<tab>` and the rest —
are escape sequences whose spelling depends on the negotiated keyboard
protocol, so a byte match cannot decide them. They are listed in `NAMED` rather
than guessed at, and the list is checked, so it cannot quietly grow.

# It can only shrink

`RECORDED` is the backlog this made visible: bindings that ship and that no
test presses. Entries fail four ways, which is what keeps it from becoming a
place to hide things:

  1. A binding with no test and no `RECORDED` entry — the new gap.
  2. A `RECORDED` entry that *is* now covered — write the test, delete the row.
  3. A `RECORDED` entry naming a binding that no longer exists — a rename that
     left its excuse behind.
  4. `NAMED` disagreeing with the notations actually found.
"""

import glob
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEYMAPS = ROOT / "runtime/keymaps.scm"
TESTS = sorted(glob.glob(str(ROOT / "crates/phosphor/tests/*.rs")))

# A press is a keystroke sequence. Anything longer is text going into a buffer.
PRESS_BYTES = 6

# Notations that are not literal bytes: the escape sequence depends on the
# negotiated keyboard protocol (`T027`), so a byte match cannot decide them.
# Covered instead by the protocol tests in `loop_pty.rs`, which drive both
# sides of the negotiation on one machine.
NAMED = {
    "<S-tab>", "<cr>", "<del>", "<down>", "<end>", "<home>",
    "<left>", "<right>", "<tab>", "<up>",
    # vim's `CTRL-^`, both spellings. A byte match cannot decide these for the
    # opposite reason to the arrow keys: the two notations are **the same
    # byte** (`0x1e`) without the kitty protocol, and which one the editor sees
    # is the terminal's answer rather than the test's. Pressed by
    # `ctrl_caret_goes_to_the_alternate_file_and_back`, which sends that byte.
    "<C-^>", "<C-6>",
}

# Bindings that ship and that no test presses.
#
# **Empty, and it started at thirty-two.** Every row was a key or a command the
# layer binds and nothing had ever pressed — most of them ordinary vim motions,
# which is the half nobody writes a test for because everybody assumes somebody
# did. They are closed in `loop_pty.rs` under *"The keys nothing pressed"*.
#
# A row belongs here when a binding genuinely cannot be pressed by a test yet,
# with the reason. It is not a place to park work: rule 2 fails the moment a
# test does press it, so a row that stops being true stops being green.
RECORDED = {
    # `T058`. Both raise the prompt line and both work — they left the
    # deferred table when the line was built. What has no test is the
    # *keystroke*, and the reason is the harness rather than the editor:
    # `press` counts frames, and a frame that opens the prompt is one the
    # counting does not survive, so every subsequent press runs to its
    # deadline. Probed against a real 100x30 pty and the editor is healthy —
    # it draws the line and takes text into it. `docs/OPEN-QUESTIONS.md` §53
    # carries the reproduction; the test lands with the fix.
    "SPC c p": "T058 — prompt line is built; the pty harness cannot press it yet (§53)",
    "SPC c s": "T058 — same binding, same reason",
}

# A Scheme string literal, escapes included. `"([^"]+)"` was the first version
# and it was wrong twice in the same file: `(list "\"" …)` — the double-quote
# text object — captured a lone backslash, so the lint recorded a binding
# spelled `\` that does not exist while missing the one that does.
STRING = r'"((?:[^"\\]|\\.)*)"'


def uncommented(text):
    """`text` with whole-line Scheme comments removed.

    **`;;` lines hold code that does not run**, and this file has a lot of it —
    worked examples in the prose. The first version of this lint read them as
    bindings and recorded `]r` as an untested key, which is a line inside a
    comment block explaining what `keymap-set!` looks like. A lint that invents
    a gap is worse than one that misses a gap: somebody writes a test for a key
    that is not bound, and it passes for the wrong reason.
    """
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith(";")
    )


def unescape(literal):
    """A Scheme string literal's value — `\\"` is one double quote."""
    return literal.replace('\\"', '"').replace("\\\\", "\\")


def bindings():
    """Every key notation and ex command name the layer binds."""
    text = uncommented(KEYMAPS.read_text())
    keys = {unescape(k) for k in re.findall(r"\(keymap-set!\s+" + STRING, text)}
    keys |= {unescape(k) for k in re.findall(r"^\s*\(list\s+" + STRING, text, re.M)}

    commands = {}
    for spec in map(unescape, re.findall(r"\(ex-set!\s+" + STRING, text)):
        match = re.match(r"^([a-z-]*)\[([a-z-]+)\]$", spec)
        full, short = (match.group(1) + match.group(2), match.group(1)) if match else (spec, spec)
        commands[full] = max(1, len(short))
    return keys, commands


def as_bytes(notation):
    """The literal bytes a test would press, or None for a named key."""
    out = b""
    for token in notation.split(" "):
        if token in ("SPC", "<space>"):
            # Both spellings are one literal byte, so neither needs `NAMED`.
            out += b" "
        elif match := re.fullmatch(r"<C-([a-z])>", token):
            out += bytes([ord(match.group(1)) - 96])
        elif re.fullmatch(r"<[^>]+>", token):
            return None
        elif all(32 <= ord(char) < 127 for char in token):
            out += token.encode()
        else:
            return None
    return out


def code_lines(path):
    """A test file's code lines, comments dropped.

    **Quotes are paired within a line and never across one**, which is the only
    version of this that works. A file-wide scan drifts the moment it meets an
    unpaired quote — this repository's doc comments quote the specification
    constantly, and the code has char literals like `\'"\'` — after which every
    literal it reports is an artefact of where the drift began. Measured:
    `"transcript"` and `"comment"` sit in the same array literal, four lines
    apart, and a file-wide scan found only the second.

    Whole-line `//` only. A `//` inside a string is not a comment, and telling
    those apart needs a parser — but a line that *starts* with `//` is a comment
    in every Rust file here.
    """
    return [
        line
        for line in pathlib.Path(path).read_text().splitlines()
        if not line.lstrip().startswith("//")
    ]


def pressed():
    """Every byte string a test writes to the pty."""
    out = set()
    for path in TESTS:
        for line in code_lines(path):
            for match in re.finditer(r'b"((?:[^"\\]|\\.)*)"', line):
                try:
                    out.add(match.group(1).encode().decode("unicode_escape").encode("latin-1"))
                except (UnicodeDecodeError, UnicodeEncodeError):
                    pass
    return out


def literals():
    """Every plain string literal in the tests.

    **Not every press is a byte literal**, which the first version of this lint
    assumed and was wrong about six times over. `loop_pty.rs` drives the
    deferred ex commands from a table of names —

        let deferred: &[(&str, &str)] = &[("transcript", "T054"), …];
        editor.press_until(format!(":{command}\\r").as_bytes(), task);

    — so the colon is in a format string and the name is a plain `"…"`. The
    lint reported all six as untyped while a test was typing them, which is the
    failure mode that matters most here: a coverage check that invents gaps
    sends somebody to write a test that already exists.

    A whole literal is required, never a substring, so `"theme"` counts and a
    sentence mentioning the theme does not.
    """
    out = set()
    for path in TESTS:
        for line in code_lines(path):
            for match in re.finditer(r'(?<!b)"((?:[^"\\]|\\.)*)"', line):
                out.add(match.group(1))
    return out


def main():
    keys, commands = bindings()
    presses = pressed()
    problems = []

    found_named = {key for key in keys if as_bytes(key) is None}
    if found_named != NAMED:
        for extra in sorted(found_named - NAMED):
            problems.append(
                f"{extra} is a named key and is not in NAMED — add it with the "
                f"reason a byte match cannot decide it"
            )
        for gone in sorted(NAMED - found_named):
            problems.append(f"NAMED lists {gone}, which the layer no longer binds")

    uncovered = set()
    for key in sorted(keys - found_named):
        wanted = as_bytes(key)
        hit = any(
            press == wanted or (len(press) <= PRESS_BYTES and wanted in press)
            for press in presses
        )
        if not hit:
            uncovered.add(key)

    quoted = literals()
    # Every string a test could be typing: byte literals as written, and plain
    # ones because a command is as often built by `format!(":edit {}", path)`.
    typed_text = [press.decode("latin-1") for press in presses] + sorted(quoted)
    for name, least in sorted(commands.items()):
        # Typed with its colon, in a literal of either kind…
        hit = any(
            name.startswith(typed) and len(typed) >= least
            for text in typed_text
            for typed in re.findall(r":([a-z-]+)", text)
        )
        # …or named on its own, for a `format!(":{command}")` to put a colon on.
        hit = hit or name in quoted
        if not hit:
            uncovered.add(f":{name}")

    for gap in sorted(uncovered - set(RECORDED)):
        problems.append(f"{gap} is bound and no test presses it — write one, or record it")

    for recorded in sorted(set(RECORDED) - uncovered):
        known = recorded[1:] in commands if recorded.startswith(":") else recorded in keys
        if known:
            problems.append(
                f"RECORDED lists {recorded}, which a test now presses — delete the row"
            )
        else:
            problems.append(f"RECORDED lists {recorded}, which the layer no longer binds")

    if problems:
        print("lint-key-coverage: FAILED")
        for problem in problems:
            print(f"  {problem}")
        return 1

    print(
        f"lint-key-coverage: clean — {len(keys)} key notations "
        f"({len(found_named)} named), {len(commands)} ex commands, "
        f"{len(RECORDED)} recorded gaps"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
