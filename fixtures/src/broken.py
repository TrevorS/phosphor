# fixtures/src/broken.py — deliberately invalid syntax.
#
# V006 fixture: "a file with a syntax error" (the S3 preamble's own wording).
# Two unclosed parens below are not typos to fix — tree-sitter-python should
# produce ERROR nodes here, which is the point: unseen markers and gutter
# state (T031/T043) have to degrade honestly on a file that will never
# highlight cleanly, and diagnostics (T040, S4) need a file that fails before
# a language server even gets involved. Do not "fix" this file.

import time


def retry_with_backoff(policy, attempt:
    """Retry a callable using policy's backoff schedule."""
    delay = policy.base_delay
    for _ in range(policy.max_attempts):
        try:
            return attempt()
        except TransientError:
            time.sleep(delay
            delay = min(delay * 2, policy.max_delay)
    raise RuntimeError("exhausted retries")
