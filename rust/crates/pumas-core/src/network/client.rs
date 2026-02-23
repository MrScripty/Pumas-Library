//! HTTP client with rate limiting awareness.
//!
//! Provides a wrapper around reqwest with:
//! - Rate limit tracking from response headers
//! - Automatic throttling when approaching limits
//! - Configurable timeouts
//! - User-agent management

use crate::config::NetworkConfig;
use crate::{PumasError, Result};
use reqwest::{header, Client, Response, StatusCode};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Rate limit state extracted from response headers.
#[derive(Debug, Clone, Default)]
pub struct RateLimitState {
    /// Remaining requests allowed.
    pub remaining: Option<u64>,
    /// Total request limit.
    pub limit: Option<u64>,
    /// Unix timestamp when the rate limit resets.
    pub reset: Option<u64>,
}

impl RateLimitState {
    /// Check if we should throttle requests.
    pub fn should_throttle(&self) -> bool {
        match (self.remaining, self.limit) {
            (Some(remaining), Some(limit)) if limit > 0 => {
                // Throttle when below 20% of limit (more conservative than before)
                let threshold = (limit as f64 * 0.2) as u64;
                remaining < threshold.max(2)
            }
            _ => false,
        }
    }

    /// Check if we are completely rate limited (no remaining requests).
    pub fn is_exhausted(&self) -> bool {
        matches!(self.remaining, Some(0))
    }

    /// Get time until rate limit resets.
    pub fn time_until_reset(&self) -> Option<Duration> {
        self.reset.and_then(|reset| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if reset > now {
                Some(Duration::from_secs(reset - now))
            } else {
                None
            }
        })
    }
}

/// HTTP client with rate limiting awareness.
pub struct HttpClient {
    client: Client,
    /// Rate limit state (shared for thread safety).
    rate_limit_remaining: AtomicI64,
    rate_limit_limit: AtomicU64,
    rate_limit_reset: AtomicU64,
    /// Throttle delay when rate limited.
    throttle_delay: Duration,
}

impl HttpClient {
    /// Create a new HTTP client with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_timeout(NetworkConfig::REQUEST_TIMEOUT)
    }

    /// Create a new HTTP client with a custom default timeout.
    pub fn with_timeout(timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("Pumas-Library/1.0")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })?;

        Ok(Self {
            client,
            rate_limit_remaining: AtomicI64::new(-1),
            rate_limit_limit: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
            throttle_delay: Duration::from_secs(2), // Increased from 500ms for more effective throttling
        })
    }

    /// Get a reference to the underlying reqwest client.
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Get the current rate limit state.
    pub fn rate_limit_state(&self) -> RateLimitState {
        let remaining = self.rate_limit_remaining.load(Ordering::SeqCst);
        RateLimitState {
            remaining: if remaining >= 0 {
                Some(remaining as u64)
            } else {
                None
            },
            limit: {
                let limit = self.rate_limit_limit.load(Ordering::SeqCst);
                if limit > 0 {
                    Some(limit)
                } else {
                    None
                }
            },
            reset: {
                let reset = self.rate_limit_reset.load(Ordering::SeqCst);
                if reset > 0 {
                    Some(reset)
                } else {
                    None
                }
            },
        }
    }

    /// Make a GET request.
    pub async fn get(&self, url: &str) -> Result<Response> {
        self.maybe_throttle().await;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("GET {} failed: {}", url, e),
                cause: Some(e.to_string()),
            })?;

        self.update_rate_limits(&response);
        self.check_response_status(response, url).await
    }

    /// Make a GET request with custom headers.
    pub async fn get_with_headers(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<Response> {
        self.maybe_throttle().await;

        let mut request = self.client.get(url);
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("GET {} failed: {}", url, e),
            cause: Some(e.to_string()),
        })?;

        self.update_rate_limits(&response);
        self.check_response_status(response, url).await
    }

    /// Make a HEAD request.
    pub async fn head(&self, url: &str) -> Result<Response> {
        self.maybe_throttle().await;

        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("HEAD {} failed: {}", url, e),
                cause: Some(e.to_string()),
            })?;

        self.update_rate_limits(&response);
        self.check_response_status(response, url).await
    }

    /// Make a POST request with JSON body.
    pub async fn post_json<T: serde::Serialize>(&self, url: &str, body: &T) -> Result<Response> {
        self.maybe_throttle().await;

        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("POST {} failed: {}", url, e),
                cause: Some(e.to_string()),
            })?;

        self.update_rate_limits(&response);
        self.check_response_status(response, url).await
    }

    /// Check if an HTTP status code indicates a retryable error.
    pub fn is_retryable_status(status: StatusCode) -> bool {
        matches!(
            status.as_u16(),
            408 | 429 | 500 | 502 | 503 | 504
        )
    }

    /// Check if an HTTP status code indicates a permanent failure.
    pub fn is_permanent_failure(status: StatusCode) -> bool {
        matches!(status.as_u16(), 400 | 401 | 403 | 404)
    }

    // Internal methods

    async fn maybe_throttle(&self) {
        let state = self.rate_limit_state();

        // If rate limit is exhausted (remaining=0), wait until reset
        if state.is_exhausted() {
            if let Some(wait_time) = state.time_until_reset() {
                // Cap wait time to 60 seconds to avoid blocking too long
                let capped_wait = wait_time.min(Duration::from_secs(60));
                warn!(
                    "Rate limit exhausted, waiting {:?} until reset (full reset in {:?})",
                    capped_wait, wait_time
                );
                tokio::time::sleep(capped_wait).await;
                return;
            }
        }

        // Standard throttle for low remaining
        if state.should_throttle() {
            warn!(
                "Rate limit approaching (remaining: {:?}/{:?}), throttling for {:?}",
                state.remaining, state.limit, self.throttle_delay
            );
            tokio::time::sleep(self.throttle_delay).await;
        }
    }

    fn update_rate_limits(&self, response: &Response) {
        let headers = response.headers();

        // X-RateLimit-Remaining
        if let Some(remaining) = headers.get("X-RateLimit-Remaining") {
            if let Ok(value) = remaining.to_str() {
                if let Ok(num) = value.parse::<i64>() {
                    self.rate_limit_remaining.store(num, Ordering::SeqCst);
                }
            }
        }

        // X-RateLimit-Limit
        if let Some(limit) = headers.get("X-RateLimit-Limit") {
            if let Ok(value) = limit.to_str() {
                if let Ok(num) = value.parse::<u64>() {
                    self.rate_limit_limit.store(num, Ordering::SeqCst);
                }
            }
        }

        // X-RateLimit-Reset
        if let Some(reset) = headers.get("X-RateLimit-Reset") {
            if let Ok(value) = reset.to_str() {
                if let Ok(num) = value.parse::<u64>() {
                    self.rate_limit_reset.store(num, Ordering::SeqCst);
                }
            }
        }

        // Log rate limit status
        let remaining = self.rate_limit_remaining.load(Ordering::SeqCst);
        let limit = self.rate_limit_limit.load(Ordering::SeqCst);
        if remaining >= 0 && limit > 0 {
            debug!("Rate limit: {}/{}", remaining, limit);
        }
    }

    async fn check_response_status(&self, response: Response, url: &str) -> Result<Response> {
        let status = response.status();

        if status.is_success() {
            return Ok(response);
        }

        // Handle rate limiting specifically
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());

            return Err(PumasError::RateLimited {
                service: extract_domain(url),
                retry_after_secs: retry_after,
            });
        }

        // Return the response for other error codes (caller may want to handle them)
        Ok(response)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

/// Extract domain from a URL.
pub fn extract_domain(url: &str) -> String {
    url::Url::parse(url)
        .map(|u| u.host_str().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_state_throttle() {
        let state = RateLimitState {
            remaining: Some(15),
            limit: Some(100),
            reset: None,
        };
        assert!(state.should_throttle()); // 15 < 20% of 100

        let state = RateLimitState {
            remaining: Some(25),
            limit: Some(100),
            reset: None,
        };
        assert!(!state.should_throttle()); // 25 >= 20% of 100
    }

    #[test]
    fn test_rate_limit_state_exhausted() {
        let state = RateLimitState {
            remaining: Some(0),
            limit: Some(60),
            reset: None,
        };
        assert!(state.is_exhausted());

        let state = RateLimitState {
            remaining: Some(1),
            limit: Some(60),
            reset: None,
        };
        assert!(!state.is_exhausted());
    }

    #[test]
    fn test_rate_limit_state_no_throttle_without_data() {
        let state = RateLimitState::default();
        assert!(!state.should_throttle());
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://api.github.com/repos/foo/bar"),
            "api.github.com"
        );
        assert_eq!(
            extract_domain("https://huggingface.co/models"),
            "huggingface.co"
        );
        assert_eq!(extract_domain("invalid-url"), "unknown");
    }

    #[test]
    fn test_retryable_status_codes() {
        assert!(HttpClient::is_retryable_status(StatusCode::REQUEST_TIMEOUT));
        assert!(HttpClient::is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(HttpClient::is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(HttpClient::is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(HttpClient::is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(HttpClient::is_retryable_status(StatusCode::GATEWAY_TIMEOUT));

        assert!(!HttpClient::is_retryable_status(StatusCode::OK));
        assert!(!HttpClient::is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!HttpClient::is_retryable_status(StatusCode::FORBIDDEN));
    }

    #[test]
    fn test_permanent_failure_status_codes() {
        assert!(HttpClient::is_permanent_failure(StatusCode::BAD_REQUEST));
        assert!(HttpClient::is_permanent_failure(StatusCode::UNAUTHORIZED));
        assert!(HttpClient::is_permanent_failure(StatusCode::FORBIDDEN));
        assert!(HttpClient::is_permanent_failure(StatusCode::NOT_FOUND));

        assert!(!HttpClient::is_permanent_failure(StatusCode::OK));
        assert!(!HttpClient::is_permanent_failure(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = HttpClient::new().unwrap();
        assert_eq!(client.rate_limit_state().remaining, None);
    }

    #[tokio::test]
    async fn test_client_with_timeout() {
        let client = HttpClient::with_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(client.rate_limit_state().remaining, None);
    }
}
