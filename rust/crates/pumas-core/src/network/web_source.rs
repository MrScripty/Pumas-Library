//! WebSource trait system for extensible network source management.
//!
//! Provides a unified interface for web sources (GitHub, HuggingFace, etc.)
//! that integrates with NetworkManager for connectivity checking and circuit
//! breaking.

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
}

/// Main trait for web sources that can be registered with NetworkManager.
///
/// Implement this trait to integrate a new web source with the network
/// management system. This provides connectivity checking and circuit
/// breaker integration. Sources own their cache reads; no shared path
/// substitutes stale cache data for a failed request.
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
    fn test_cache_strategy_ttl() {
        assert_eq!(MockSource.default_ttl(), Duration::from_secs(300));
    }
}
