//! Retry logic with exponential backoff and jitter.
//!
//! Provides configurable retry behavior for network operations with:
//! - Exponential backoff (delay doubles each attempt)
//! - Optional jitter to prevent thundering herd
//! - Customizable retry predicates
//! - Statistics tracking

use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first one).
    pub max_attempts: u32,
    /// Initial delay between retries.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Exponential base (typically 2.0 for doubling).
    pub exponential_base: f64,
    /// Whether to add random jitter to delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            exponential_base: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of attempts.
    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the base delay.
    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Set the maximum delay cap.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Enable or disable jitter.
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate the delay for a given attempt number (0-indexed).
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: base * (exponential_base ^ attempt)
        let multiplier = self.exponential_base.powi(attempt as i32);
        let delay_secs = self.base_delay.as_secs_f64() * multiplier;
        let capped_secs = delay_secs.min(self.max_delay.as_secs_f64());

        let final_secs = if self.jitter {
            // Decorrelated jitter: multiply delay by random factor between 0.5 and 1.5
            // This keeps the average delay the same while adding randomness to prevent
            // thundering herd, without allowing near-zero delays
            let mut rng = rand::rng();
            let jitter_factor = rng.random_range(0.5..1.5);
            (capped_secs * jitter_factor).min(self.max_delay.as_secs_f64())
        } else {
            capped_secs
        };

        Duration::from_secs_f64(final_secs)
    }
}

/// Statistics about a retry operation.
#[derive(Debug, Clone, Default)]
pub struct RetryStats {
    /// Number of attempts made.
    pub attempts: u32,
    /// Total delay accumulated.
    pub total_delay: Duration,
    /// Whether the operation ultimately succeeded.
    pub success: bool,
    /// Last error message if failed.
    pub last_error: Option<String>,
}

/// Retry an async operation with exponential backoff.
///
/// # Arguments
///
/// * `config` - Retry configuration
/// * `operation` - Async function that returns a Result
/// * `should_retry` - Predicate to determine if an error is retryable
///
/// # Returns
///
/// A tuple of (Result, RetryStats)
pub async fn retry_async<F, Fut, T, E>(
    config: &RetryConfig,
    mut operation: F,
    should_retry: impl Fn(&E) -> bool,
) -> (Result<T, E>, RetryStats)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut stats = RetryStats::default();

    for attempt in 0..config.max_attempts {
        stats.attempts = attempt + 1;

        match operation().await {
            Ok(value) => {
                stats.success = true;
                if attempt > 0 {
                    debug!("Operation succeeded after {} attempts", attempt + 1);
                }
                return (Ok(value), stats);
            }
            Err(e) => {
                stats.last_error = Some(e.to_string());

                // Check if we should retry
                if !should_retry(&e) {
                    debug!("Error is not retryable: {}", e);
                    return (Err(e), stats);
                }

                // Check if we have more attempts
                if attempt + 1 >= config.max_attempts {
                    warn!(
                        "All {} retry attempts exhausted. Last error: {}",
                        config.max_attempts, e
                    );
                    return (Err(e), stats);
                }

                // Calculate and apply delay
                let delay = config.calculate_delay(attempt);
                stats.total_delay += delay;

                warn!(
                    "Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt + 1,
                    config.max_attempts,
                    e,
                    delay
                );

                tokio::time::sleep(delay).await;
            }
        }
    }

    // Should not reach here, but just in case
    unreachable!("Retry loop should have returned")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_delay_calculation_no_jitter() {
        let config = RetryConfig::new()
            .with_base_delay(Duration::from_secs(1))
            .with_jitter(false);

        // First attempt: 1 * 2^0 = 1s
        assert_eq!(config.calculate_delay(0), Duration::from_secs(1));
        // Second attempt: 1 * 2^1 = 2s
        assert_eq!(config.calculate_delay(1), Duration::from_secs(2));
        // Third attempt: 1 * 2^2 = 4s
        assert_eq!(config.calculate_delay(2), Duration::from_secs(4));
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig::new()
            .with_base_delay(Duration::from_secs(10))
            .with_max_delay(Duration::from_secs(30))
            .with_jitter(false);

        // 10 * 2^3 = 80s, but capped at 30s
        assert_eq!(config.calculate_delay(3), Duration::from_secs(30));
    }

    #[test]
    fn test_delay_with_jitter() {
        let config = RetryConfig::new()
            .with_base_delay(Duration::from_secs(2))
            .with_jitter(true);

        // With decorrelated jitter, delay should be between 0.5x and 1.5x the base
        // For attempt 0 with base 2s: expected range is 1s to 3s
        for _ in 0..20 {
            let delay = config.calculate_delay(0);
            // Jitter factor is 0.5 to 1.5, so delay should be 1s to 3s
            assert!(
                delay >= Duration::from_secs(1) && delay <= Duration::from_secs(3),
                "Delay {:?} should be between 1s and 3s",
                delay
            );
        }

        // For attempt 1 with base 2s: 2 * 2^1 = 4s, range is 2s to 6s
        for _ in 0..20 {
            let delay = config.calculate_delay(1);
            assert!(
                delay >= Duration::from_secs(2) && delay <= Duration::from_secs(6),
                "Delay {:?} should be between 2s and 6s",
                delay
            );
        }
    }

    #[tokio::test]
    async fn test_retry_succeeds_first_try() {
        let config = RetryConfig::new().with_max_attempts(3);

        let (result, stats) =
            retry_async(&config, || async { Ok::<_, String>(42) }, |_: &String| true).await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(stats.attempts, 1);
        assert!(stats.success);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_base_delay(Duration::from_millis(10))
            .with_jitter(false);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let (result, stats) = retry_async(
            &config,
            || {
                let counter = counter_clone.clone();
                async move {
                    let count = counter.fetch_add(1, Ordering::SeqCst);
                    if count < 2 {
                        Err("temporary failure".to_string())
                    } else {
                        Ok(42)
                    }
                }
            },
            |_: &String| true,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(stats.attempts, 3);
        assert!(stats.success);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_base_delay(Duration::from_millis(10))
            .with_jitter(false);

        let (result, stats) = retry_async(
            &config,
            || async { Err::<i32, _>("always fails".to_string()) },
            |_: &String| true,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(stats.attempts, 3);
        assert!(!stats.success);
        assert_eq!(stats.last_error, Some("always fails".to_string()));
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let config = RetryConfig::new().with_max_attempts(3);

        let (result, stats) = retry_async(
            &config,
            || async { Err::<i32, _>("permanent failure".to_string()) },
            |e: &String| !e.contains("permanent"),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(stats.attempts, 1); // Only one attempt for non-retryable
        assert!(!stats.success);
    }
}
