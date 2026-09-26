//! Network utilities for HTTP operations, retries, and resilience.
//!
//! This module provides:
//! - Retry logic with exponential backoff and jitter
//! - Circuit breaker pattern for network resilience
//! - HTTP client with rate limiting awareness
//! - GitHub API integration
//! - Download manager with progress tracking
//! - NetworkManager for centralized connectivity management
//! - WebSource traits for extensible web source registration

pub mod activity;
mod circuit_breaker;
mod client;
mod download;
mod github;
mod manager;
mod retry;
mod web_source;

pub use activity::{
    ActivityError, ActivityUpdate, CopyableSource, MeasurementBasis, MeasurementCoverage,
    NetworkActivityRegistry, OperationId, TransferActivity, TransferDirection, TransferId,
    TransferState,
};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerStats, CircuitState,
};
pub use client::{HttpClient, RateLimitState};
pub use download::{DownloadManager, DownloadProgress};
pub use github::{GitHubAsset, GitHubClient, GitHubRelease, ReleasesCache};
pub use manager::{ConnectivityConfig, ConnectivityState, NetworkManager, NetworkStatus};
pub use retry::{retry_async, RetryConfig, RetryStats};
pub use web_source::{CacheStrategy, DynWebSource, WebSource, WebSourceId};
