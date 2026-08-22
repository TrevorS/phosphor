#!/usr/bin/env python3
"""A toy ACP agent, for `T050`'s tests.

The sibling of `crates/phosphor/tests/fixtures/toy_language_server.py`, and
here for the same three reasons: a real agent needs a network, a subscription
and a model whose answers are not the same twice, and none of those belong in a
test that asks whether *the client* works.

It speaks the stable ACP wire form — newline-delimited JSON-RPC 2.0 on
stdin/stdout — and answers exactly three requests:

  * `initialize`     -> protocol version 1 and empty capabilities
  * `session/new`    -> a fixed session id
  * `session/prompt` -> one `session/update` notification carrying the prompt
                        back as agent prose, then `{"stopReason": "end_turn"}`

Modes, chosen by `argv[1]`, each standing for one thing the client has to
survive:

  `turn`      the whole exchange above, plus one tool call that starts and
              completes. The happy path.
  `mute`      answers `initialize` and `session/new`, then never answers a
              prompt. The client must stay attached and stay responsive; a
              turn that never ends is not a hang.
  `slow`      the whole exchange, with the stop reason held back for
              `SLOW_SECONDS`. The only mode in which "claude is working" is
              observable on a screen: `turn` answers in microseconds, so a test
              watching the statusline would see `idle`, `working` and `idle`
              inside one frame and could assert none of them.
  `deaf`      exits immediately after `initialize`. The connection drops
              mid-session, which is `7b`'s seam and `Failure::Dropped`.
  `linger`    answers the handshake, then exits `SLOW_SECONDS` later. The mode
              a *staleness* test needs: `deaf` dies inside the keystrokes that
              set it up, so the editor has already redrawn by the time such a
              test stops typing, and it passes with the wake removed. Measured
              — the planted defect went green.
  `drop`      answers the handshake, then dies **mid-turn**: a prompt gets one
              chunk of prose and the process exits before any stop reason. This
              is screen `7b` exactly — "acp gone mid-turn" — and it is the one
              mode where the turn is still open at the moment the connection
              goes. `deaf` and `linger` both die with nothing running, so
              neither can produce a seam.
  `gibberish` answers `initialize` with a line that is not JSON at all.

Nothing here imports the ACP SDK on purpose: a fixture built from the same
library as the code under test proves the two agree with each other rather than
with the protocol.
"""

import json
import os
import sys
import threading
import time

SESSION = "toy-session-1"

#: How long `slow` holds a turn open. Long enough for a pty test to see a frame
#: drawn during it, short enough not to dominate the suite.
SLOW_SECONDS = 2.0


def send(message: dict) -> None:
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


def result(request_id, payload: dict) -> None:
    send({"jsonrpc": "2.0", "id": request_id, "result": payload})


def notify(method: str, params: dict) -> None:
    send({"jsonrpc": "2.0", "method": method, "params": params})


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "turn"

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue

        method = message.get("method")
        request_id = message.get("id")

        # A notification: nothing to answer.
        if request_id is None:
            continue

        if method == "initialize":
            if mode == "gibberish":
                sys.stdout.write("this is not json\n")
                sys.stdout.flush()
                continue
            result(
                request_id,
                {
                    "protocolVersion": 1,
                    "agentCapabilities": {},
                    "authMethods": [],
                    "agentInfo": {"name": "toy-acp-agent", "version": "0.0.1"},
                },
            )
            if mode == "deaf":
                return 0
            if mode == "linger":
                # Answer `session/new` first if it is already queued, then go.
                # A thread, so the read loop keeps serving until the moment it
                # exits — the point is that the drop lands while nobody is
                # typing, not that nothing was answered.
                threading.Timer(SLOW_SECONDS, lambda: os._exit(0)).start()
            continue

        if method == "session/new":
            result(request_id, {"sessionId": SESSION})
            continue

        if method == "session/prompt":
            if mode == "mute":
                continue
            params = message.get("params") or {}
            spoken = " ".join(
                block.get("text", "")
                for block in params.get("prompt") or []
                if isinstance(block, dict)
            )
            if mode == "slow":
                time.sleep(SLOW_SECONDS)
            session_id = params.get("sessionId", SESSION)
            notify(
                "session/update",
                {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": f"heard: {spoken}"},
                    },
                },
            )
            if mode == "drop":
                # Prose out, then gone — no stop reason, ever. `os._exit` and
                # not `return`, so nothing gets a chance to flush a tidy
                # goodbye the client could mistake for one.
                sys.stdout.flush()
                os._exit(0)
            # A tool call that starts and completes — the row `1b` is mostly
            # made of, and the only way to prove the client turns one into
            # `tool-call-started` / `tool-call-completed` rather than dropping
            # it beside the prose.
            notify(
                "session/update",
                {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "call-1",
                        "title": "src/retry.rs",
                        "kind": "edit",
                        "status": "pending",
                        # `T056`'s jump link. ACP keeps the *title* — what the
                        # row says — apart from `locations`, which is where a
                        # file actually is, and a real agent's title is a
                        # sentence rather than a path. The two look alike here
                        # only because `1b` happens to draw a path.
                        "locations": [{"path": "/tmp/toy/src/retry.rs", "line": 19}],
                    },
                },
            )
            notify(
                "session/update",
                {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": "call-1",
                        "title": "src/retry.rs",
                        "status": "completed",
                    },
                },
            )
            result(request_id, {"stopReason": "end_turn"})
            continue

        # Anything else: an empty result rather than an error. A client that
        # asked something this fixture does not model should not be failed by
        # the fixture's own narrowness.
        result(request_id, {})

    return 0


if __name__ == "__main__":
    sys.exit(main())
