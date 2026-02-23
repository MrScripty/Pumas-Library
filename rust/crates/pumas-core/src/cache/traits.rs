//! Cache backend trait and types.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for cache behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CacheConfig {
    /// Default TTL for cache entries.
    pub default_ttl: Duration,
    /// Maximum cache size in bytes (0 = unlimited).
    pub max_size_bytes: u64,
    /// Whether to enable LRU eviction when max size is reached.
    pub enable_eviction: bool,
}

impl CacheConfig {
    /// Default time-to-live for cache entries (1 day).
    pub const DEFAULT_TTL_SECS: u64 = 86_400;
    /// Default maximum cache size (4 GB).
    pub const DEFAULT_MAX_SIZE_BYTES: u64 = 4_294_967_296;
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(CacheConfig::DEFAULT_TTL_SECS),
            max_size_bytes: CacheConfig::DEFAULT_MAX_SIZE_BYTES,
            enable_eviction: true,
        }
    }
}

/// A cached entry with metadata.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The cached data as bytes.
    pub value: Vec<u8>,
    /// When the entry was cached.
    pub cached_at: DateTime<Utc>,
    /// When the entry expires.
    pub expires_at: DateTime<Utc>,
    /// Size of the cached data in bytes.
    pub size_bytes: u64,
    /// When the entry was last accessed.
    pub last_accessed: DateTime<Utc>,
}

/// Metadata about a cache namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    /// Namespace name.
    pub namespace: String,
    /// Number of entries in this namespace.
    pub entry_count: usize,
    /// Total size of all entries in bytes.
    pub total_size_bytes: u64,
    /// When the namespace was last modified.
    pub last_modified: Option<DateTime<Utc>>,
    /// When the last cleanup was performed.
    pub last_cleanup: Option<DateTime<Utc>>,
}

/// Cache statistics across all namespaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of entries across all namespaces.
    pub total_entries: usize,
    /// Total size of all cached data in bytes.
    pub total_size_bytes: u64,
    /// Maximum allowed size in bytes.
    pub max_size_bytes: u64,
    /// Number of namespaces.
    pub namespace_count: usize,
    /// Per-namespace statistics.
    pub namespaces: Vec<CacheMeta>,
}

/// Generic cache backend trait.
///
/// Provides namespace-isolated key-value storage with TTL support.
/// All operations are synchronous to match rusqlite's API.
pub trait CacheBackend: Send + Sync {
    /// Get cached data by key.
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>>;

    /// Get cached data with full entry metadata.
    fn get_entry(&self, namespace: &str, key: &str) -> Result<Option<CacheEntry>>;

    /// Set cached data with TTL.
    ///
    /// Overwrites any existing entry with the same key.
    fn set(&self, namespace: &str, key: &str, value: &[u8], ttl: Duration) -> Result<()>;

    /// Set cached data with explicit expiration time.
    fn set_with_expiry(
        &self,
        namespace: &str,
        key: &str,
        value: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<()>;

    /// Invalidate (delete) a specific key.
    fn invalidate(&self, namespace: &str, key: &str) -> Result<bool>;

    /// Invalidate all keys in a namespace.
    fn invalidate_namespace(&self, namespace: &str) -> Result<usize>;

    /// Check if a cache entry exists and is valid (not expired).
    fn is_valid(&self, namespace: &str, key: &str) -> Result<bool>;

    /// Get metadata for a namespace.
    fn get_namespace_meta(&self, namespace: &str) -> Result<Option<CacheMeta>>;

    /// Get overall cache statistics.
    fn get_stats(&self) -> Result<CacheStats>;

    /// Remove expired entries from all namespaces.
    ///
    /// Returns the number of entries removed.
    fn cleanup_expired(&self) -> Result<usize>;

    /// Evict entries until cache is under the size limit.
    ///
    /// Uses LRU (least recently accessed) eviction strategy.
    /// Returns the number of entries evicted.
    fn evict_to_size(&self, max_bytes: u64) -> Result<usize>;

    /// Clear all cached data across all namespaces.
    fn clear_all(&self) -> Result<()>;
}
