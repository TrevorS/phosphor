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
}

# Bindings that ship and that nothing presses. Shrink this by writing tests.
RECORDED = {
    # Motions with no test of their own. Cheap to close, and the reason they
    # are here is that the survey that found them stopped at the leader keys.
    "$": "end-of-line motion",
    "^": "first non-blank motion",
    "0": "start-of-line motion",
    "{": "paragraph back",
    "}": "paragraph forward",
    "(": "sentence back",
    ")": "sentence forward",
    "H": "screen top",
    "L": "screen bottom",
    "W": "WORD forward",
    "%": "matching bracket",
    ";": "repeat the last find",
    ",": "repeat the last find, reversed",
    "F": "find back on the line",
    "T": "till back on the line",
    "<": "dedent",
    "\\": "unbound prefix in the shipped map",
    # Operators and edits.
    "C": "change to end of line",
    "O": "open a line above",
    "@": "run a register",
    "gs": "sort — no test presses it",
    "gu": "lowercase operator",
    "g~": "swap case operator",
    # Viewport and scroll.
    "<C-b>": "page back",
    "<C-f>": "page forward",
    "<C-d>": "half page down",
    "<C-u>": "half page up",
    "<C-c>": "cancel — the pty harness leaves with ZQ instead",
    # Store.
    "]r": "next region — `]u` is tested, this one is not",
    # Ex commands nothing types.
    ":quit": "the harness leaves with `ZQ`, never `:q`",
    ":edit": "used only through `:e <path>` inside other tests' setup",
    ":close-buffer": "refuses until T088 splits the pane",
    ":theme": "the theme tapes set it on the command line instead",
    ":transcript": "deferred — T054",
    ":timeline": "deferred — T073",
    ":inbox": "deferred — T059",
    ":diff-disk": "deferred — T070",
    ":reattach": "deferred — T062",
    ":comment": "deferred — T060",
}


def bindings():
    """Every key notation and ex command name the layer binds."""
    text = KEYMAPS.read_text()
    keys = set(re.findall(r'\(keymap-set!\s+"([^"]+)"', text))
    keys |= set(re.findall(r'^\s*\(list\s+"([^"]+)"', text, re.M))

    commands = {}
    for spec in re.findall(r'\(ex-set!\s+"([^"]+)"', text):
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


def pressed():
    """Every byte string a test writes to the pty."""
    out = set()
    for path in TESTS:
        for match in re.finditer(r'b"((?:[^"\\]|\\.)*)"', pathlib.Path(path).read_text()):
            try:
                out.add(match.group(1).encode().decode("unicode_escape").encode("latin-1"))
            except (UnicodeDecodeError, UnicodeEncodeError):
                pass
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

    for name, least in sorted(commands.items()):
        hit = False
        for press in presses:
            for typed in re.findall(r":([a-z-]+)", press.decode("latin-1")):
                if name.startswith(typed) and len(typed) >= least:
                    hit = True
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
