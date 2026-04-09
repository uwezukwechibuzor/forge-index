//! Retry policy with exponential backoff and jitter.

use crate::error::RpcError;
use std::time::Duration;
use tokio_retry::strategy::ExponentialBackoff;

const MAX_RETRIES: usize = 4; // 4 retries = 5 total attempts
const BASE_DELAY_MS: u64 = 1000;
const MAX_DELAY: Duration = Duration::from_secs(30);

/// Adds ±10% jitter to a duration.
fn add_jitter(duration: Duration) -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let factor = 0.9 + (nanos as f64 / u32::MAX as f64) * 0.2;
    duration.mul_f64(factor)
}

/// Executes `f` with retry logic.
///
/// - Max 5 total attempts (1 initial + 4 retries)
/// - Exponential backoff: 1s, 2s, 4s, 8s (capped at 30s) with ±10% jitter
/// - Retries on transport errors, rate limits, and timeouts
/// - Does NOT retry on decode errors
pub async fn with_retry<F, Fut, T>(method: &str, chain_id: u64, mut f: F) -> Result<T, RpcError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, RpcError>>,
{
    let delays: Vec<Duration> = ExponentialBackoff::from_millis(BASE_DELAY_MS)
        .factor(2)
        .max_delay(MAX_DELAY)
        .map(add_jitter)
        .take(MAX_RETRIES)
        .collect();

    // First attempt
    match f().await {
        Ok(v) => return Ok(v),
        Err(e) if !e.is_retryable() => return Err(e),
        Err(e) => {
            tracing::warn!(
                method = method,
                chain_id = chain_id,
                attempt = 1,
                error = %e,
                "RPC call failed, will retry"
            );
        }
    }

    // Retry attempts
    for (i, delay) in delays.iter().enumerate() {
        tracing::warn!(
            method = method,
            chain_id = chain_id,
            attempt = i + 2,
            delay_ms = delay.as_millis() as u64,
            "Retrying RPC call"
        );
        tokio::time::sleep(*delay).await;

        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if !e.is_retryable() => return Err(e),
            Err(e) => {
                if i + 2 >= (MAX_RETRIES + 1) {
                    // Last attempt
                    return Err(RpcError::MaxRetriesExceeded {
                        method: method.to_string(),
                        attempts: (MAX_RETRIES + 1) as u32,
                    });
                }
                tracing::warn!(
                    method = method,
                    chain_id = chain_id,
                    attempt = i + 2,
                    error = %e,
                    "RPC call failed, will retry"
                );
            }
        }
    }

    Err(RpcError::MaxRetriesExceeded {
        method: method.to_string(),
        attempts: (MAX_RETRIES + 1) as u32,
    })
}
