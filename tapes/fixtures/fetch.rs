// `7c`'s call site, parsed by the tree-sitter-rust the editor loads for a
// `.rs` buffer. The tail is load-bearing: `7c-rust.tape` types `7c`'s missing
// line by counting back to the call (`G k k O`).

use serde_json::Value;
use std::time::Duration;

pub struct RetryPolicy {
    delay: Duration,
    attempts: u32,
}

pub async fn fetch_all(urls: &[String]) -> Vec<Result<Value, FetchError>> {
    join_all(urls.iter().map(|u| fetch_json(u))).await
}
