// One header line, and it is load-bearing — see `diagnostics.tape`'s header.
const DEFAULT_DELAY: u128 = 200;

pub struct RetryPolicy {
    delay: Duration,
    attempts: u32,
}

pub fn shipped() -> RetryPolicy {
    RetryPolicy {
        delay: DEFAULT_DELAY,
        attempts: 3,
    }
}
