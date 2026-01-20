//! Network utilities for HTTP operations, retries, and resilience.
//!
//! This module provides:
//! - Retry logic with exponential backoff and jitter
//! - Circuit breaker pattern for network resilience
//! - HTTP client with rate limiting awareness
//! - GitHub API integration
//! - Download manager with progress tracking

mod circuit_breaker;
mod client;
mod download;
mod github;
mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use client::{HttpClient, RateLimitState};
pub use download::{DownloadManager, DownloadProgress};
pub use github::{GitHubClient, GitHubRelease, GitHubAsset, ReleasesCache};
pub use retry::{RetryConfig, RetryStats, retry_async};
