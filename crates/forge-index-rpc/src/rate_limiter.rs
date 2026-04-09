//! Per-chain token bucket rate limiter.

use tokio::time::Instant;

/// Thread-safe token bucket rate limiter.
pub struct RateLimiter {
    inner: tokio::sync::Mutex<TokenBucket>,
}

struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_rps: u32) -> Self {
        Self {
            tokens: max_rps as f64,
            max_tokens: max_rps as f64,
            refill_rate: max_rps as f64,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = (now - self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn time_until_available(&mut self) -> std::time::Duration {
        self.refill();
        if self.tokens >= 1.0 {
            std::time::Duration::ZERO
        } else {
            let deficit = 1.0 - self.tokens;
            std::time::Duration::from_secs_f64(deficit / self.refill_rate)
        }
    }
}

impl RateLimiter {
    /// Creates a new rate limiter with the given maximum requests per second.
    pub fn new(max_requests_per_second: u32) -> Self {
        Self {
            inner: tokio::sync::Mutex::new(TokenBucket::new(max_requests_per_second)),
        }
    }

    /// Acquires a single token, waiting if necessary until one is available.
    pub async fn acquire(&self) {
        loop {
            let wait_time = {
                let mut bucket = self.inner.lock().await;
                if bucket.try_acquire() {
                    return;
                }
                bucket.time_until_available()
            };
            tokio::time::sleep(wait_time).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rate_limiter_with_limit_2_per_sec_10_calls_takes_at_least_4_seconds() {
        tokio::time::pause();

        let limiter = RateLimiter::new(2);
        let start = Instant::now();

        for _ in 0..10 {
            limiter.acquire().await;
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed >= std::time::Duration::from_secs(4),
            "Expected at least 4s, got {:?}",
            elapsed
        );
    }
}
