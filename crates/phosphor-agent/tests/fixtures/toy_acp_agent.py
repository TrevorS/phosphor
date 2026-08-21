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

  `turn`      the whole exchange above. The happy path.
  `mute`      answers `initialize` and `session/new`, then never answers a
              prompt. The client must stay attached and stay responsive; a
              turn that never ends is not a hang.
  `deaf`      exits immediately after `initialize`. The connection drops
              mid-session, which is `7b`'s seam and `Failure::Dropped`.
  `gibberish` answers `initialize` with a line that is not JSON at all.

Nothing here imports the ACP SDK on purpose: a fixture built from the same
library as the code under test proves the two agree with each other rather than
with the protocol.
"""

import json
import sys

SESSION = "toy-session-1"


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
            notify(
                "session/update",
                {
                    "sessionId": params.get("sessionId", SESSION),
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": f"heard: {spoken}"},
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
