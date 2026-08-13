# Retry logic — reading notes

V006 fixture: Markdown (T037's first-class twelve), and the vendored
`ratatui-markdown` fork's (`T004`, `T055`) plain-text-vs-rendered path.

## Why exponential backoff

A fixed delay between retries means every caller retrying the same failure
wakes on the same schedule — a thundering herd against whatever just came
back up. Doubling the delay each attempt spreads that out; jitter spreads it
further.

## Open questions

- Does `max_attempts: 1` need a special case, or does the loop bound already
  cover it? (See the thread anchored at `src/retry.rs:19-21`.)
- Should `Client::request_json` share one `Jitter` across requests, or build
  a fresh one per call?

```rust
let mut delay = policy.base_delay;
delay = (delay * 2).min(policy.max_delay);
```

> The cap has to apply identically whether or not jitter is in the picture —
> that is what this review block is actually about.
