//! The fetch client — the second file in `retry.rs`'s review block.
//!
//! `fetch_json` below, and the `RetryPolicy::default()`/`retry_with_backoff`
//! call inside it, match `docs/design/TUI Mockups.dc.html`'s `2b` (hunk peek)
//! and `3b` (jj timeline) screens byte-for-byte — both render this exact
//! function. See `fixtures/README.md`'s citation table.

use crate::retry::{RetryPolicy, retry_with_backoff};

pub async fn fetch_json(url: &str) -> Result<Value, FetchError> {
    let policy = RetryPolicy::default();
    let resp = retry_with_backoff(|| client.get(url).send(), &policy)?;
    resp.json().await.map_err(FetchError::Decode)
}

#[derive(Debug)]
pub enum FetchError {
    Transport(std::io::Error),
    Decode(serde_json::Error),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "transport: {error}"),
            Self::Decode(error) => write!(f, "decode: {error}"),
        }
    }
}

impl From<std::io::Error> for FetchError {
    fn from(error: std::io::Error) -> Self {
        Self::Transport(error)
    }
}
