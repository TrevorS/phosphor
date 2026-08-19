#!/usr/bin/env python3
"""A language server that exists so `tests/loop_pty.rs` can press a key.

`T038`'s *done when* is "screen 7c's completion reproduces **from a keystroke**"
and `T040`'s is "a file with real errors shows correct gutter priority". Both
are statements about the running binary talking to a real server over a real
pipe, and neither can be proved by a test that hands `Editing` an Action: that
substitution is exactly the defect `T016` and `T097` were paid for.

rust-analyzer is the honest server and the wrong one to gate CI on — it must be
installed, it indexes a whole crate graph, and it decides for itself what to
offer. This speaks the same protocol over the same transport and answers in
constants, so the assertion can be `frame contains "default_delay"` rather than
"something appeared".

WHAT IS DELIBERATELY REAL. The framing (`Content-Length` headers), the
JSON-RPC envelope, `initialize`/`initialized`/`shutdown`, the UTF-16 position
encoding declaration the client checks the reply of, and `didOpen` sync. The
editor's client cannot tell this apart from a real server, which is the point.

WHAT IT DOES NOT DO. Parse anything. Positions in the answers below are
constants, and the completion list is the same three items whatever the prefix
is. A server that filtered would make the test about this file's matcher.

WHICH BEHAVIOUR is chosen by argv, because a pty test counts frames and an
unsolicited push arrives on its own schedule:

    completion   answer lookups; publish nothing
    diagnostics  publish one error on didOpen; answer nothing
    diagnostic-cascade
                 publish ELEVEN errors on one line on didOpen — the parse
                 cascade `CP-4` reported, for the row policy that bounds it
    definition-here
                 answer `definition` with *this* document rather than a
                 sibling — `gd` into the file you are already editing, which is
                 the common case and the one that used to re-read the buffer
                 from disk; answer no lookups, so no float competes for the
                 frame
    definition-column
                 the same jump, at a **non-zero character**. Every other answer
                 in this file starts at character 0, where a column carried
                 correctly and a column dropped land on the same cell — so no
                 test here could tell them apart, and the references picker
                 shipped dropping it

`7c`'s own labels and detail column are used for the completion items so the
frame a test reads is the frame the mockup draws.

A SECOND ARGUMENT, optional, is a path to append one line to per
`textDocument/completion` received. It exists for the debounce
(`COMPLETION_DEBOUNCE`): *how long the editor waits* is not visible in any
frame, and the only honest question to ask about it is how many requests a
burst of typing produced. Absent, nothing is written and this file behaves
exactly as it did.
"""

import json
import sys

MODE = sys.argv[1] if len(sys.argv) > 1 else "completion"
REQUEST_LOG = sys.argv[2] if len(sys.argv) > 2 else None


def record(method):
    """Append one line naming a request, when a log was asked for.

    Opened per write and closed again rather than held: the reader is another
    process watching the file grow, and a buffered handle in this one would
    make the count a fact about flushing.
    """
    if not REQUEST_LOG:
        return
    with open(REQUEST_LOG, "a", encoding="utf-8") as log:
        log.write(method + "\n")


def read_message():
    """One `Content-Length`-framed JSON-RPC message, or None at end of input."""
    length = None
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.strip()
        if not line:
            break
        name, _, value = line.decode("ascii", "replace").partition(":")
        if name.strip().lower() == "content-length":
            length = int(value.strip())
    if length is None:
        return None
    return json.loads(sys.stdin.buffer.read(length))


def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()


def reply(request_id, result):
    send({"jsonrpc": "2.0", "id": request_id, "result": result})


CAPABILITIES = {
    # The one the client reads back and refuses on. See `lsp.rs`'s header.
    "positionEncoding": "utf-16",
    # 1 is TextDocumentSyncKind.Full — the whole document per change, which is
    # what the editor has to give.
    "textDocumentSync": 1,
    # `CP-4`'s review: the editor's typing gate is a floor on *identifier*
    # prefixes, and `foo.` has a prefix of zero — so a server that means "ask me
    # here" says it with these, and the editor reads them off `initialize`.
    # One of each shape is enough to prove the plumbing, including a
    # multi-character trigger, which is compared as a suffix and not as a key.
    "completionProvider": {"triggerCharacters": [".", "::"]},
    "hoverProvider": True,
    "signatureHelpProvider": {"triggerCharacters": ["("]},
    "definitionProvider": True,
    # `T047` — `gr`. Announced so the client asks; the handler is below.
    "referencesProvider": True,
}

# `7c`, verbatim: three labels, a meta detail column, and one row of prose —
# plus the three fields the protocol has always sent and this editor used to
# throw away. `kind` is `CompletionItemKind`; `labelDetails.description` is the
# "src" column, which the client asks for by announcing `labelDetailsSupport`
# and which a server is entitled to withhold from one that does not;
# `tags: [1]` is `Deprecated`.
#
# One row carries each so a frame assertion can name it: `default` is a
# function from `retry::policy`, `default_delay` is a constant with no source,
# and `defaults_for` is deprecated. A list where every row looked the same
# would pass a renderer that drew the first row's decoration on all three.
COMPLETIONS = [
    {
        "label": "default",
        "kind": 3,
        "labelDetails": {"description": "retry::policy"},
        "detail": "fn() -> RetryPolicy",
        "insertText": "default()",
        "documentation": "Returns the policy with 3 attempts, 200ms base, 1s cap.",
    },
    {
        "label": "default_delay",
        "kind": 21,
        "detail": "Duration",
        "insertText": "default_delay",
        "documentation": "The base delay every attempt is measured from.",
    },
    {
        "label": "defaults_for",
        "kind": 3,
        "tags": [1],
        "detail": "fn(Kind) -> RetryPolicy",
        "insertText": "defaults_for",
        "documentation": "The policy this kind of request ships with.",
    },
]


# The eleven a real rust-analyzer answered with when Teej half-typed `path:`
# at CP-4, transcribed from the report — repeats included, because the repeats
# are the point. A parse cascade is one parser resynchronising, not eleven
# findings, and it is what `diagnostic-rows` exists to bound.
CASCADE = [
    "Syntax Error: expected type",
    "Syntax Error: expected COMMA",
    "Syntax Error: expected field declaration",
    "Syntax Error: expected COMMA",
    "Syntax Error: expected COLON",
    "Syntax Error: expected R_PAREN",
    "Syntax Error: expected COMMA",
    "Syntax Error: expected field declaration",
    "Syntax Error: expected COMMA",
    "Syntax Error: expected field declaration",
    "Syntax Error: expected COMMA",
]


def publish(uri):
    if MODE == "diagnostic-cascade":
        # All eleven on ONE line, which is what a half-typed line produces.
        found = [
            {
                "range": {
                    "start": {"line": 1, "character": 0},
                    "end": {"line": 1, "character": 5},
                },
                "severity": 1,
                "message": message,
                "source": "toy",
            }
            for message in CASCADE
        ]
    else:
        found = [
            {
                # Line 1, columns 1..5, zero-based and half-open as LSP
                # counts them. The fixture puts a word there.
                "range": {
                    "start": {"line": 1, "character": 0},
                    "end": {"line": 1, "character": 5},
                },
                "severity": 1,
                "message": "expected Duration, found u128",
                "source": "toy",
            }
        ]
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "diagnostics": found},
        }
    )


def offered(announced):
    """The completion list, minus anything the client did not ask for.

    **A real server withholds `labelDetails` from a client that did not
    announce `completionItem.labelDetailsSupport`**, which is what makes the
    announcement worth testing: the field is `@since 3.17` and the
    specification makes sending it conditional on exactly that flag. So this
    fixture withholds it too. Drop `label_details_support` from
    `initialize_params` in `crates/phosphor-buffer/src/lsp.rs` and the `src`
    column goes off the screen, which is a pty test failing rather than a
    silently emptier float.
    """
    if announced:
        return COMPLETIONS
    return [
        {key: value for key, value in item.items() if key != "labelDetails"}
        for item in COMPLETIONS
    ]


def main():
    label_details = False
    while True:
        message = read_message()
        if message is None:
            return
        method = message.get("method")
        request_id = message.get("id")

        if method == "initialize":
            label_details = bool(
                message["params"]
                .get("capabilities", {})
                .get("textDocument", {})
                .get("completion", {})
                .get("completionItem", {})
                .get("labelDetailsSupport")
            )
            reply(
                request_id,
                {
                    "capabilities": CAPABILITIES,
                    # Deliberately not `toy`, which is the language, and not
                    # `python3`, which is the command: the statusline chip is
                    # specified to draw the name a server gives itself, and a
                    # fixture whose three names collide could not tell.
                    "serverInfo": {"name": "toy-lsp", "version": "0"},
                },
            )
        elif method == "shutdown":
            reply(request_id, None)
        elif method == "exit":
            return
        elif method == "textDocument/didOpen":
            if MODE in ("diagnostics", "diagnostic-cascade"):
                publish(message["params"]["textDocument"]["uri"])
        elif method == "textDocument/definition":
            # A place in a *different* file, so the jump is observable: the
            # editor has to open something and put the cursor in it. The
            # sibling is named off the document's own URI, because this process
            # is told nothing about the directory it was started in.
            uri = message["params"]["textDocument"]["uri"]
            here = MODE in ("definition-here", "definition-column")
            # Character 0 for every mode but one. `definition-column` is the
            # only answer in this file that names a column at all, because a
            # column is the one part of a place a client can drop while still
            # looking right.
            character = 4 if MODE == "definition-column" else 0
            reply(
                request_id,
                {
                    "uri": uri if here else uri.rsplit("/", 1)[0] + "/target.toy",
                    "range": {
                        "start": {"line": 1, "character": character},
                        "end": {"line": 1, "character": character},
                    },
                },
            )
        elif method == "textDocument/references":
            # `T047`. **Three places, in two files**, because that is what
            # separates a working references picker from a working *definition*
            # jump: one place could be drawn by opening it, and the whole point
            # of `8a` is that a list needs a surface. Two of them are in the
            # document itself and one is in a sibling, so a row that named the
            # wrong file would be visible.
            uri = message["params"]["textDocument"]["uri"]
            sibling = uri.rsplit("/", 1)[0] + "/target.toy"
            reply(
                request_id,
                []
                if MODE == "references-none"
                else [
                    {
                        "uri": uri,
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 3},
                        },
                    },
                    {
                        "uri": uri,
                        "range": {
                            "start": {"line": 2, "character": 4},
                            "end": {"line": 2, "character": 7},
                        },
                    },
                    {
                        "uri": sibling,
                        "range": {
                            "start": {"line": 1, "character": 0},
                            "end": {"line": 1, "character": 3},
                        },
                    },
                ],
            )
        elif method == "textDocument/completion":
            record(method)
            reply(
                request_id,
                offered(label_details) if MODE == "completion" else [],
            )
        elif method == "textDocument/hover":
            reply(
                request_id,
                {"contents": {"kind": "plaintext", "value": "a toy hover answer"}}
                if MODE == "completion"
                else None,
            )
        elif method == "textDocument/signatureHelp":
            reply(
                request_id,
                {
                    "signatures": [
                        {
                            "label": "fn retry(policy: RetryPolicy) -> Result<(), Error>",
                            "parameters": [{"label": [8, 28]}],
                            "documentation": "how many times, and how far apart",
                        }
                    ],
                    "activeSignature": 0,
                    "activeParameter": 0,
                }
                if MODE == "completion"
                else None,
            )
        elif request_id is not None:
            # Every other request gets a null answer rather than silence: a
            # server that never replies makes a test time out instead of fail.
            reply(request_id, None)


if __name__ == "__main__":
    main()
