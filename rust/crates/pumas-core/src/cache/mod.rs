//! Unified cache abstraction for Pumas.
//!
//! Provides a generic caching layer that can be used by different subsystems:
//! - HuggingFace search/repository caching
//! - GitHub release caching
//! - Plugin configuration caching
//!
//! All caches share a single SQLite database with namespace-based isolation.

mod sqlite;
mod traits;

pub use sqlite::SqliteCache;
pub use traits::{CacheBackend, CacheConfig, CacheEntry, CacheMeta, CacheStats};
