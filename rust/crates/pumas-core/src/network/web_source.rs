//! WebSource trait system for extensible network source management.
//!
//! Provides a unified interface for web sources (GitHub, HuggingFace, etc.)
//! that integrates with NetworkManager for connectivity checking, circuit
//! breaking, and caching strategies.

use async_trait::async_trait;
use std::time::Duration;

/// Unique identifier for a web source.
///
/// Implement this trait to identify your web source and its domains.
pub trait WebSourceId: Send + Sync {
    /// Returns the unique source identifier (e.g., "github", "huggingface").
    fn id(&self) -> &'static str;

    /// Returns the domains this source connects to.
    ///
    /// Used for circuit breaker tracking per domain.
    fn domains(&self) -> &[&'static str];
}

/// Cache behavior configuration for a web source.
pub trait CacheStrategy: Send + Sync {
    /// Default time-to-live for cache entries.
    fn default_ttl(&self) -> Duration;

    /// Whether to use stale cache data when network is unavailable.
    ///
    /// Defaults to true for offline-first behavior.
    fn allow_stale_on_offline(&self) -> bool {
        true
    }

    /// Maximum age for stale cache data before it's considered unusable.
    ///
    /// `None` means no limit - any stale data is acceptable when offline.
    fn max_stale_age(&self) -> Option<Duration> {
        None
    }
}

/// Main trait for web sources that can be registered with NetworkManager.
///
/// Implement this trait to integrate a new web source with the network
/// management system. This provides automatic connectivity checking,
/// circuit breaker integration, and cache fallback behavior.
#[async_trait]
pub trait WebSource: WebSourceId + CacheStrategy + Send + Sync {
    /// Check if this source has any cached data for a given key.
    ///
    /// This should check both memory and disk caches.
    fn has_cache(&self, key: &str) -> bool;

    /// Check if cached data is still fresh (not stale/expired).
    fn is_cache_fresh(&self, key: &str) -> bool;

    /// Called when the network becomes available after being offline.
    ///
    /// Sources can use this to trigger background cache refresh.
    async fn on_network_restored(&self) {}

    /// Called when circuit breaker opens for this source's domain.
    ///
    /// Sources can use this for logging or metrics.
    fn on_circuit_open(&self, _domain: &str) {}

    /// Called periodically for optional background cache refresh.
    ///
    /// Return Ok(()) if no refresh needed or refresh succeeded.
    async fn background_refresh(&self) -> crate::Result<()> {
        Ok(())
    }
}

/// Wrapper to make any WebSource into an Arc<dyn WebSource>.
///
/// This is useful for storing heterogeneous sources in collections.
pub type DynWebSource = std::sync::Arc<dyn WebSource>;

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSource;

    impl WebSourceId for MockSource {
        fn id(&self) -> &'static str {
            "mock"
        }

        fn domains(&self) -> &[&'static str] {
            &["mock.example.com"]
        }
    }

    impl CacheStrategy for MockSource {
        fn default_ttl(&self) -> Duration {
            Duration::from_secs(300)
        }
    }

    #[async_trait]
    impl WebSource for MockSource {
        fn has_cache(&self, _key: &str) -> bool {
            false
        }

        fn is_cache_fresh(&self, _key: &str) -> bool {
            false
        }
    }

    #[test]
    fn test_web_source_id() {
        let source = MockSource;
        assert_eq!(source.id(), "mock");
        assert_eq!(source.domains(), &["mock.example.com"]);
    }

    #[test]
    fn test_cache_strategy_defaults() {
        let source = MockSource;
        assert!(source.allow_stale_on_offline());
        assert!(source.max_stale_age().is_none());
    }
}
