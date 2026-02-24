use std::future::Future;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;

/// Configuration for retry behaviour with exponential back-off.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries, 1 = one retry, etc.).
    pub max_retries: u32,
    /// Initial delay before the first retry, in milliseconds.
    pub initial_delay_ms: u64,
    /// Multiplier applied to the delay after each retry.
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 1,
            initial_delay_ms: 3000,
            backoff_factor: 2.0,
        }
    }
}

/// Execute an async operation with retries according to the given policy.
///
/// The `operation` closure is called repeatedly until it succeeds, is
/// cancelled, or all retries are exhausted.  Between attempts the function
/// sleeps with exponential back-off, but the sleep is interruptible by the
/// cancellation token.
///
/// # Type parameters
///
/// * `F`   - A closure that returns a future producing `Result<T, E>`.
/// * `Fut` - The future type returned by `F`.
/// * `T`   - The success type.
/// * `E`   - The error type.
pub async fn execute_with_retry<F, Fut, T, E>(
    policy: &RetryPolicy,
    cancel_token: &CancellationToken,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay_ms = policy.initial_delay_ms;

    for attempt in 0..=policy.max_retries {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                // If this was the last allowed attempt, propagate the error.
                if attempt == policy.max_retries {
                    return Err(err);
                }

                // Check cancellation before sleeping.
                if cancel_token.is_cancelled() {
                    return Err(err);
                }

                tracing::warn!(
                    attempt = attempt + 1,
                    max = policy.max_retries,
                    delay_ms = delay_ms,
                    "operation failed, retrying after delay"
                );

                // Sleep is interruptible by cancellation.
                tokio::select! {
                    _ = sleep(Duration::from_millis(delay_ms)) => {}
                    _ = cancel_token.cancelled() => {
                        // Cancelled during retry delay — return last error.
                        return Err(err);
                    }
                }

                delay_ms = (delay_ms as f64 * policy.backoff_factor) as u64;
            }
        }
    }

    // Unreachable: the loop always returns on the last iteration.
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn token() -> CancellationToken {
        CancellationToken::new()
    }

    #[tokio::test]
    async fn test_succeeds_first_try() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_delay_ms: 10,
            backoff_factor: 2.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        let result: Result<&str, &str> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok("ok")
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retries_then_succeeds() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay_ms: 10,
            backoff_factor: 1.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        let result: Result<&str, &str> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err("fail")
                } else {
                    Ok("recovered")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_exhausts_retries() {
        let policy = RetryPolicy {
            max_retries: 1,
            initial_delay_ms: 10,
            backoff_factor: 1.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        let result: Result<&str, &str> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err("always fails")
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), "always fails");
        // initial attempt + 1 retry = 2
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cancellation_stops_retries() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_delay_ms: 50,
            backoff_factor: 1.0,
        };

        let cancel = CancellationToken::new();
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let cancel_clone = cancel.clone();

        let result: Result<&str, &str> = execute_with_retry(&policy, &cancel, || {
            let c = c.clone();
            let cancel_clone = cancel_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // Cancel after first failure, during the retry delay.
                    cancel_clone.cancel();
                }
                Err("fail")
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), "fail");
        // Should have stopped after at most 2 attempts (cancelled during delay).
        assert!(counter.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 1);
        assert_eq!(policy.initial_delay_ms, 3000);
        assert!((policy.backoff_factor - 2.0).abs() < f64::EPSILON);
    }
}
