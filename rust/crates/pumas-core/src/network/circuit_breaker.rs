//! Circuit breaker pattern for network resilience.
//!
//! Implements the circuit breaker pattern to prevent cascading failures:
//! - CLOSED: Normal operation, requests flow through
//! - OPEN: Failing, requests are rejected immediately
//! - HALF_OPEN: Testing recovery, limited requests allowed

use crate::config::NetworkConfig;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests flow through.
    Closed,
    /// Failing - requests are rejected immediately.
    Open,
    /// Testing recovery - limited requests allowed.
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "CLOSED"),
            CircuitState::Open => write!(f, "OPEN"),
            CircuitState::HalfOpen => write!(f, "HALF_OPEN"),
        }
    }
}

/// Configuration for circuit breaker behavior.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit.
    pub failure_threshold: u32,
    /// Time to wait before attempting recovery.
    pub recovery_timeout: Duration,
    /// Maximum number of test requests in half-open state.
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: NetworkConfig::CIRCUIT_BREAKER_RECOVERY_TIMEOUT,
            half_open_max_calls: 1,
        }
    }
}

/// Circuit breaker for protecting against cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    /// Current state of the circuit.
    state: RwLock<CircuitState>,
    /// Consecutive failure count (reset on success).
    failure_count: AtomicU32,
    /// Total failure count (lifetime).
    total_failures: AtomicU64,
    /// Total success count (lifetime).
    total_successes: AtomicU64,
    /// When the circuit was opened.
    opened_at: RwLock<Option<Instant>>,
    /// Number of test calls made in half-open state.
    half_open_calls: AtomicU32,
    /// Domain this circuit breaker protects.
    domain: String,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration.
    pub fn new(domain: impl Into<String>) -> Self {
        Self::with_config(domain, CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration.
    pub fn with_config(domain: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            total_failures: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            opened_at: RwLock::new(None),
            half_open_calls: AtomicU32::new(0),
            domain: domain.into(),
        }
    }

    /// Get the current state of the circuit.
    pub fn state(&self) -> CircuitState {
        self.maybe_transition_to_half_open();
        *self.state.read().unwrap()
    }

    /// Check if a request should be allowed through.
    pub fn allow_request(&self) -> bool {
        self.maybe_transition_to_half_open();

        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // Allow limited test requests
                let calls = self.half_open_calls.fetch_add(1, Ordering::SeqCst);
                calls < self.config.half_open_max_calls
            }
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        self.total_successes.fetch_add(1, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);

        let state = *self.state.read().unwrap();
        if state == CircuitState::HalfOpen {
            // Recovery successful - close the circuit
            self.transition_to_closed();
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        self.total_failures.fetch_add(1, Ordering::SeqCst);
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => {
                if failures >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Recovery failed - reopen the circuit
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Get statistics about this circuit breaker.
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            domain: self.domain.clone(),
            state: self.state(),
            failure_count: self.failure_count.load(Ordering::SeqCst),
            total_failures: self.total_failures.load(Ordering::SeqCst),
            total_successes: self.total_successes.load(Ordering::SeqCst),
            time_in_state: self.time_in_current_state(),
        }
    }

    /// Reset the circuit breaker to closed state.
    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        *self.opened_at.write().unwrap() = None;
        *self.state.write().unwrap() = CircuitState::Closed;
        info!("Circuit breaker for {} reset to CLOSED", self.domain);
    }

    // Internal state transitions

    fn transition_to_open(&self) {
        let mut state = self.state.write().unwrap();
        if *state != CircuitState::Open {
            *state = CircuitState::Open;
            *self.opened_at.write().unwrap() = Some(Instant::now());
            self.half_open_calls.store(0, Ordering::SeqCst);
            warn!(
                "Circuit breaker for {} opened after {} failures",
                self.domain,
                self.failure_count.load(Ordering::SeqCst)
            );
        }
    }

    fn transition_to_half_open(&self) {
        let mut state = self.state.write().unwrap();
        if *state == CircuitState::Open {
            *state = CircuitState::HalfOpen;
            self.half_open_calls.store(0, Ordering::SeqCst);
            debug!("Circuit breaker for {} entering HALF_OPEN", self.domain);
        }
    }

    fn transition_to_closed(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        *self.opened_at.write().unwrap() = None;
        info!("Circuit breaker for {} recovered to CLOSED", self.domain);
    }

    fn maybe_transition_to_half_open(&self) {
        let state = *self.state.read().unwrap();
        if state != CircuitState::Open {
            return;
        }

        let opened_at = *self.opened_at.read().unwrap();
        if let Some(opened) = opened_at {
            if opened.elapsed() >= self.config.recovery_timeout {
                self.transition_to_half_open();
            }
        }
    }

    fn time_in_current_state(&self) -> Duration {
        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => Duration::ZERO,
            CircuitState::Open | CircuitState::HalfOpen => self
                .opened_at
                .read()
                .unwrap()
                .map(|t| t.elapsed())
                .unwrap_or(Duration::ZERO),
        }
    }
}

/// Statistics about a circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub domain: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub total_failures: u64,
    pub total_successes: u64,
    pub time_in_state: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new("test.com");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config("test.com", config);

        // Record failures up to threshold
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config("test.com", config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // Resets count
        cb.record_failure();
        cb.record_failure();
        // Should still be closed (only 2 consecutive failures)
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::with_config("test.com", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(15));

        // Should transition to half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_request()); // First test request allowed

        // Success in half-open closes the circuit
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::with_config("test.com", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(15));

        // Transition to half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Failure in half-open reopens
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_stats() {
        let cb = CircuitBreaker::new("test.com");
        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let stats = cb.stats();
        assert_eq!(stats.domain, "test.com");
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.failure_count, 1);
    }

    #[test]
    fn test_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config("test.com", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Reset
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }
}
