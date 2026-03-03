use std::future::Future;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;

use crate::error::Retryable;

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
/// 不可重试的错误（如认证失败）会立即返回，不浪费重试次数。
pub async fn execute_with_retry<F, Fut, T, E>(
    policy: &RetryPolicy,
    cancel_token: &CancellationToken,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug + Retryable,
{
    let mut delay_ms = policy.initial_delay_ms;

    for attempt in 0..=policy.max_retries {
        let err = match operation().await {
            Ok(value) => return on_retry_success(value, attempt),
            Err(err) => err,
        };
        if !should_retry(&err, attempt, policy, cancel_token) {
            return Err(err);
        }
        log_retry(attempt, policy.max_retries, delay_ms, &err);
        if wait_retry_delay(delay_ms, cancel_token).await {
            return Err(err);
        }
        delay_ms = next_delay(delay_ms, policy.backoff_factor);
    }

    unreachable!()
}

fn on_retry_success<T, E>(value: T, attempt: u32) -> Result<T, E> {
    if attempt > 0 {
        tracing::info!(attempt = attempt + 1, "Operation succeeded after retry");
    }
    Ok(value)
}

fn should_retry<E: Retryable>(
    err: &E,
    attempt: u32,
    policy: &RetryPolicy,
    cancel_token: &CancellationToken,
) -> bool {
    if !err.is_retryable() || attempt == policy.max_retries || cancel_token.is_cancelled() {
        return false;
    }
    true
}

fn log_retry<E: std::fmt::Debug>(attempt: u32, max_retries: u32, delay_ms: u64, err: &E) {
    tracing::warn!(
        attempt = attempt + 1,
        max = max_retries,
        delay_ms,
        error = ?err,
        "operation failed, retrying after delay"
    );
}

async fn wait_retry_delay(delay_ms: u64, cancel_token: &CancellationToken) -> bool {
    tokio::select! {
        _ = sleep(Duration::from_millis(delay_ms)) => false,
        _ = cancel_token.cancelled() => true,
    }
}

fn next_delay(delay_ms: u64, backoff_factor: f64) -> u64 {
    (delay_ms as f64 * backoff_factor) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 测试用的可重试错误。
    #[derive(Debug, PartialEq)]
    struct TestError(&'static str);

    impl Retryable for TestError {
        fn is_retryable(&self) -> bool {
            true
        }
    }

    /// 测试用的不可重试错误。
    #[derive(Debug, PartialEq)]
    struct NonRetryableError(&'static str);

    impl Retryable for NonRetryableError {
        fn is_retryable(&self) -> bool {
            false
        }
    }

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

        let result: Result<&str, TestError> = execute_with_retry(&policy, &token(), || {
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

        let result: Result<&str, TestError> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(TestError("fail"))
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

        let result: Result<&str, TestError> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(TestError("always fails"))
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), TestError("always fails"));
        // initial attempt + 1 retry = 2
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_non_retryable_error_skips_retry() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay_ms: 10,
            backoff_factor: 1.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        let result: Result<&str, NonRetryableError> = execute_with_retry(&policy, &token(), || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(NonRetryableError("auth failed"))
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), NonRetryableError("auth failed"));
        // 不可重试 → 只执行 1 次，不重试。
        assert_eq!(counter.load(Ordering::SeqCst), 1);
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

        let result: Result<&str, TestError> = execute_with_retry(&policy, &cancel, || {
            let c = c.clone();
            let cancel_clone = cancel_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // Cancel after first failure, during the retry delay.
                    cancel_clone.cancel();
                }
                Err(TestError("fail"))
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), TestError("fail"));
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
