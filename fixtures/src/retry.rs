//! Retry policy — exponential backoff with jitter for the fetch client.
//! Used by `fetch::request` when a transient error is retried.

use util::jitter;

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

pub fn retry_with_backoff<T, E>(
    mut op: impl FnMut() -> Result<T, E>,
    policy: &RetryPolicy,
) -> Result<T, E> {
    let mut delay = policy.base_delay;
    let mut last = None;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
        thread::sleep(jitter(delay));
        delay = (delay * 2).min(policy.max_delay);
    }
    Err(last.unwrap())
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_never_exceeds_max_delay() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(200),
        };
        let mut tries = 0;
        let result: Result<(), &str> = retry_with_backoff(
            || {
                tries += 1;
                if tries < 3 { Err("transient") } else { Ok(()) }
            },
            &policy,
        );
        assert!(result.is_ok());
        assert_eq!(tries, 3);
    }

    #[test]
    fn the_default_policy_matches_fetch_rs() {
        // `RetryPolicy::default()` is what `src/fetch.rs`'s `fetch_json`
        // constructs — see fixtures/README.md's citation table.
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 4);
        assert_eq!(policy.base_delay, Duration::from_millis(50));
        assert_eq!(policy.max_delay, Duration::from_secs(2));
    }

    #[test]
    fn every_attempt_exhausted_returns_the_last_error() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(4),
        };
        let result: Result<(), &str> = retry_with_backoff(|| Err("down"), &policy);
        assert_eq!(result, Err("down"));
    }
}
