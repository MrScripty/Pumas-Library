//! SQLite-based unified cache implementation.

use super::traits::{CacheBackend, CacheConfig, CacheEntry, CacheMeta, CacheStats};
use crate::error::{PumasError, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::debug;

/// SQLite-based cache backend.
///
/// Provides namespace-isolated caching with a single shared database.
/// Thread-safe via internal mutex on the connection.
pub struct SqliteCache {
    /// Database connection (wrapped for thread safety).
    conn: Arc<Mutex<Connection>>,
    /// Cache configuration.
    config: CacheConfig,
}

impl SqliteCache {
    /// Create a new cache at the specified database path.
    ///
    /// Creates the database and tables if they don't exist.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        Self::with_config(db_path, CacheConfig::default())
    }

    /// Create a new cache with custom configuration.
    pub fn with_config(db_path: impl AsRef<Path>, config: CacheConfig) -> Result<Self> {
        let db_path = db_path.as_ref();

        // Create parent directory if needed
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create cache directory: {}", e),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        let conn = Connection::open(db_path).map_err(|e| PumasError::Database {
            message: format!("Failed to open cache database: {}", e),
            source: Some(e),
        })?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| PumasError::Database {
                message: format!("Failed to set pragmas: {}", e),
                source: Some(e),
            })?;

        let cache = Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        };

        cache.init_schema()?;

        Ok(cache)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        conn.execute_batch(
            r#"
            -- Generic cache entries table
            CREATE TABLE IF NOT EXISTS cache_entries (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                cached_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                last_accessed TEXT NOT NULL,
                PRIMARY KEY (namespace, key)
            );

            -- Index for expiration queries
            CREATE INDEX IF NOT EXISTS idx_cache_expires
                ON cache_entries(namespace, expires_at);

            -- Index for LRU eviction
            CREATE INDEX IF NOT EXISTS idx_cache_accessed
                ON cache_entries(last_accessed);

            -- Index for size queries
            CREATE INDEX IF NOT EXISTS idx_cache_size
                ON cache_entries(namespace, size_bytes);

            -- Namespace metadata table
            CREATE TABLE IF NOT EXISTS cache_namespaces (
                namespace TEXT PRIMARY KEY,
                entry_count INTEGER DEFAULT 0,
                total_size_bytes INTEGER DEFAULT 0,
                last_modified TEXT,
                last_cleanup TEXT
            );

            -- Cache configuration table
            CREATE TABLE IF NOT EXISTS cache_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to initialize cache schema: {}", e),
            source: Some(e),
        })?;

        // Store default config values
        let defaults = [
            (
                "default_ttl_seconds",
                self.config.default_ttl.as_secs().to_string(),
            ),
            ("max_size_bytes", self.config.max_size_bytes.to_string()),
            ("enable_eviction", self.config.enable_eviction.to_string()),
        ];

        for (key, value) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO cache_config (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to set config {}: {}", key, e),
                source: Some(e),
            })?;
        }

        Ok(())
    }

    /// Get current configuration from database.
    pub fn get_config(&self) -> Result<CacheConfig> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let get_value = |key: &str, default: &str| -> String {
            conn.query_row(
                "SELECT value FROM cache_config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| default.to_string())
        };

        let default_ttl_secs: u64 = get_value(
            "default_ttl_seconds",
            &CacheConfig::DEFAULT_TTL_SECS.to_string(),
        )
        .parse()
        .unwrap_or(CacheConfig::DEFAULT_TTL_SECS);
        let max_size_bytes: u64 = get_value(
            "max_size_bytes",
            &CacheConfig::DEFAULT_MAX_SIZE_BYTES.to_string(),
        )
        .parse()
        .unwrap_or(CacheConfig::DEFAULT_MAX_SIZE_BYTES);
        let enable_eviction: bool = get_value("enable_eviction", "true").parse().unwrap_or(true);

        Ok(CacheConfig {
            default_ttl: Duration::from_secs(default_ttl_secs),
            max_size_bytes,
            enable_eviction,
        })
    }

    /// Update configuration.
    pub fn set_config(&self, config: &CacheConfig) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let values = [
            (
                "default_ttl_seconds",
                config.default_ttl.as_secs().to_string(),
            ),
            ("max_size_bytes", config.max_size_bytes.to_string()),
            ("enable_eviction", config.enable_eviction.to_string()),
        ];

        for (key, value) in values {
            conn.execute(
                "INSERT OR REPLACE INTO cache_config (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to update config {}: {}", key, e),
                source: Some(e),
            })?;
        }

        Ok(())
    }

    /// Update namespace metadata after modifications.
    fn update_namespace_meta(&self, conn: &Connection, namespace: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Calculate entry count and total size
        let (count, size): (i64, i64) = conn
            .query_row(
                r#"
                SELECT COUNT(*), COALESCE(SUM(size_bytes), 0)
                FROM cache_entries
                WHERE namespace = ?1
                "#,
                params![namespace],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));

        conn.execute(
            r#"
            INSERT INTO cache_namespaces (namespace, entry_count, total_size_bytes, last_modified)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(namespace) DO UPDATE SET
                entry_count = ?2,
                total_size_bytes = ?3,
                last_modified = ?4
            "#,
            params![namespace, count, size, now],
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to update namespace metadata: {}", e),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Check and perform eviction if needed.
    fn check_eviction(&self) -> Result<()> {
        let config = self.get_config()?;
        if !config.enable_eviction || config.max_size_bytes == 0 {
            return Ok(());
        }

        let stats = self.get_stats()?;
        if stats.total_size_bytes > config.max_size_bytes {
            let evicted = self.evict_to_size(config.max_size_bytes)?;
            if evicted > 0 {
                debug!("Evicted {} entries to stay under size limit", evicted);
            }
        }

        Ok(())
    }
}

impl CacheBackend for SqliteCache {
    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_entry(namespace, key)
            .map(|opt| opt.map(|e| e.value))
    }

    fn get_entry(&self, namespace: &str, key: &str) -> Result<Option<CacheEntry>> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let row: Option<(Vec<u8>, String, String, i64, String)> = conn
            .query_row(
                r#"
                SELECT value, cached_at, expires_at, size_bytes, last_accessed
                FROM cache_entries
                WHERE namespace = ?1 AND key = ?2 AND expires_at > ?3
                "#,
                params![namespace, key, now_str],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query cache entry: {}", e),
                source: Some(e),
            })?;

        let (value, cached_at_str, expires_at_str, size_bytes, last_accessed_str) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Update last_accessed timestamp
        let _ = conn.execute(
            "UPDATE cache_entries SET last_accessed = ?1 WHERE namespace = ?2 AND key = ?3",
            params![now_str, namespace, key],
        );

        let cached_at = DateTime::parse_from_rfc3339(&cached_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);
        let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);
        let last_accessed = DateTime::parse_from_rfc3339(&last_accessed_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        Ok(Some(CacheEntry {
            value,
            cached_at,
            expires_at,
            size_bytes: size_bytes as u64,
            last_accessed,
        }))
    }

    fn set(&self, namespace: &str, key: &str, value: &[u8], ttl: Duration) -> Result<()> {
        let expires_at = Utc::now() + chrono::Duration::from_std(ttl).unwrap_or_default();
        self.set_with_expiry(namespace, key, value, expires_at)
    }

    fn set_with_expiry(
        &self,
        namespace: &str,
        key: &str,
        value: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let now = Utc::now().to_rfc3339();
        let expires_str = expires_at.to_rfc3339();
        let size_bytes = value.len() as i64;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO cache_entries
            (namespace, key, value, cached_at, expires_at, size_bytes, last_accessed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![namespace, key, value, now, expires_str, size_bytes, now],
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to set cache entry: {}", e),
            source: Some(e),
        })?;

        self.update_namespace_meta(&conn, namespace)?;

        // Release lock before checking eviction
        drop(conn);
        let _ = self.check_eviction();

        Ok(())
    }

    fn invalidate(&self, namespace: &str, key: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let deleted = conn
            .execute(
                "DELETE FROM cache_entries WHERE namespace = ?1 AND key = ?2",
                params![namespace, key],
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to invalidate cache entry: {}", e),
                source: Some(e),
            })?;

        if deleted > 0 {
            self.update_namespace_meta(&conn, namespace)?;
        }

        Ok(deleted > 0)
    }

    fn invalidate_namespace(&self, namespace: &str) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let deleted = conn
            .execute(
                "DELETE FROM cache_entries WHERE namespace = ?1",
                params![namespace],
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to invalidate namespace: {}", e),
                source: Some(e),
            })?;

        // Update or remove namespace metadata
        conn.execute(
            "DELETE FROM cache_namespaces WHERE namespace = ?1",
            params![namespace],
        )
        .ok();

        debug!(
            "Invalidated {} entries from namespace '{}'",
            deleted, namespace
        );

        Ok(deleted)
    }

    fn is_valid(&self, namespace: &str, key: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let now = Utc::now().to_rfc3339();

        let exists: bool = conn
            .query_row(
                r#"
                SELECT 1 FROM cache_entries
                WHERE namespace = ?1 AND key = ?2 AND expires_at > ?3
                LIMIT 1
                "#,
                params![namespace, key, now],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| PumasError::Database {
                message: format!("Failed to check cache validity: {}", e),
                source: Some(e),
            })?
            .unwrap_or(false);

        Ok(exists)
    }

    fn get_namespace_meta(&self, namespace: &str) -> Result<Option<CacheMeta>> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let row: Option<(i64, i64, Option<String>, Option<String>)> = conn
            .query_row(
                r#"
                SELECT entry_count, total_size_bytes, last_modified, last_cleanup
                FROM cache_namespaces
                WHERE namespace = ?1
                "#,
                params![namespace],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| PumasError::Database {
                message: format!("Failed to get namespace metadata: {}", e),
                source: Some(e),
            })?;

        let (entry_count, total_size_bytes, last_modified_str, last_cleanup_str) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let last_modified = last_modified_str
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let last_cleanup = last_cleanup_str
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(Some(CacheMeta {
            namespace: namespace.to_string(),
            entry_count: entry_count as usize,
            total_size_bytes: total_size_bytes as u64,
            last_modified,
            last_cleanup,
        }))
    }

    fn get_stats(&self) -> Result<CacheStats> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        // Get overall stats
        let (total_entries, total_size): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM cache_entries",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));

        // Get per-namespace stats
        let mut stmt = conn
            .prepare(
                r#"
                SELECT namespace, entry_count, total_size_bytes, last_modified, last_cleanup
                FROM cache_namespaces
                ORDER BY namespace
                "#,
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to prepare namespace stats query: {}", e),
                source: Some(e),
            })?;

        let namespaces: Vec<CacheMeta> = stmt
            .query_map([], |row| {
                let namespace: String = row.get(0)?;
                let entry_count: i64 = row.get(1)?;
                let total_size_bytes: i64 = row.get(2)?;
                let last_modified_str: Option<String> = row.get(3)?;
                let last_cleanup_str: Option<String> = row.get(4)?;

                Ok(CacheMeta {
                    namespace,
                    entry_count: entry_count as usize,
                    total_size_bytes: total_size_bytes as u64,
                    last_modified: last_modified_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_cleanup: last_cleanup_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            })
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query namespace stats: {}", e),
                source: Some(e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);
        drop(conn);
        let config = self.get_config()?;

        Ok(CacheStats {
            total_entries: total_entries as usize,
            total_size_bytes: total_size as u64,
            max_size_bytes: config.max_size_bytes,
            namespace_count: namespaces.len(),
            namespaces,
        })
    }

    fn cleanup_expired(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let now = Utc::now().to_rfc3339();

        // Get affected namespaces before deletion
        let mut stmt = conn
            .prepare("SELECT DISTINCT namespace FROM cache_entries WHERE expires_at <= ?1")
            .map_err(|e| PumasError::Database {
                message: format!("Failed to prepare cleanup query: {}", e),
                source: Some(e),
            })?;

        let affected_namespaces: Vec<String> = stmt
            .query_map(params![now], |row| row.get(0))
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query expired entries: {}", e),
                source: Some(e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        // Delete expired entries
        let deleted = conn
            .execute(
                "DELETE FROM cache_entries WHERE expires_at <= ?1",
                params![now],
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to cleanup expired entries: {}", e),
                source: Some(e),
            })?;

        // Update namespace metadata
        for namespace in affected_namespaces {
            self.update_namespace_meta(&conn, &namespace)?;
        }

        // Update last_cleanup timestamp for affected namespaces
        let cleanup_time = now;
        conn.execute(
            "UPDATE cache_namespaces SET last_cleanup = ?1",
            params![cleanup_time],
        )
        .ok();

        if deleted > 0 {
            debug!("Cleaned up {} expired cache entries", deleted);
        }

        Ok(deleted)
    }

    fn evict_to_size(&self, max_bytes: u64) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        // Get current total size
        let current_size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if (current_size as u64) <= max_bytes {
            return Ok(0);
        }

        let excess = current_size as u64 - max_bytes;
        debug!(
            "Cache size {}MB exceeds limit {}MB, evicting...",
            current_size / 1_000_000,
            max_bytes / 1_000_000
        );

        // Get entries ordered by last_accessed (LRU)
        let mut stmt = conn
            .prepare(
                r#"
                SELECT namespace, key, size_bytes
                FROM cache_entries
                ORDER BY last_accessed ASC
                "#,
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to prepare eviction query: {}", e),
                source: Some(e),
            })?;

        let entries: Vec<(String, String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query for eviction: {}", e),
                source: Some(e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        // Evict until under limit
        let mut evicted_bytes = 0u64;
        let mut evicted_count = 0;
        let mut affected_namespaces: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for (namespace, key, size) in entries {
            if evicted_bytes >= excess {
                break;
            }

            conn.execute(
                "DELETE FROM cache_entries WHERE namespace = ?1 AND key = ?2",
                params![namespace, key],
            )
            .ok();

            evicted_bytes += size as u64;
            evicted_count += 1;
            affected_namespaces.insert(namespace);
        }

        // Update namespace metadata for affected namespaces
        for namespace in affected_namespaces {
            self.update_namespace_meta(&conn, &namespace)?;
        }

        debug!(
            "Evicted {} entries ({}MB)",
            evicted_count,
            evicted_bytes / 1_000_000
        );

        Ok(evicted_count)
    }

    fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        conn.execute("DELETE FROM cache_entries", [])
            .map_err(|e| PumasError::Database {
                message: format!("Failed to clear cache entries: {}", e),
                source: Some(e),
            })?;

        conn.execute("DELETE FROM cache_namespaces", [])
            .map_err(|e| PumasError::Database {
                message: format!("Failed to clear namespace metadata: {}", e),
                source: Some(e),
            })?;

        debug!("Cleared all cache data");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (TempDir, SqliteCache) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_cache.sqlite");
        let cache = SqliteCache::new(&db_path).unwrap();
        (temp_dir, cache)
    }

    #[test]
    fn test_cache_creation() {
        let (_temp, cache) = create_test_cache();
        let config = cache.get_config().unwrap();
        assert_eq!(config.max_size_bytes, CacheConfig::DEFAULT_MAX_SIZE_BYTES);
    }

    #[test]
    fn test_set_and_get() {
        let (_temp, cache) = create_test_cache();

        cache
            .set("test_ns", "key1", b"hello world", Duration::from_secs(3600))
            .unwrap();

        let value = cache.get("test_ns", "key1").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"hello world");
    }

    #[test]
    fn test_expiration() {
        let (_temp, cache) = create_test_cache();

        // Set with very short TTL (already expired)
        let expired_at = Utc::now() - chrono::Duration::seconds(1);
        cache
            .set_with_expiry("test_ns", "expired_key", b"old data", expired_at)
            .unwrap();

        let value = cache.get("test_ns", "expired_key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_invalidate() {
        let (_temp, cache) = create_test_cache();

        cache
            .set("test_ns", "key1", b"data1", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("test_ns", "key2", b"data2", Duration::from_secs(3600))
            .unwrap();

        assert!(cache.is_valid("test_ns", "key1").unwrap());

        let deleted = cache.invalidate("test_ns", "key1").unwrap();
        assert!(deleted);

        assert!(!cache.is_valid("test_ns", "key1").unwrap());
        assert!(cache.is_valid("test_ns", "key2").unwrap());
    }

    #[test]
    fn test_invalidate_namespace() {
        let (_temp, cache) = create_test_cache();

        cache
            .set("ns1", "key1", b"data1", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("ns1", "key2", b"data2", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("ns2", "key1", b"data3", Duration::from_secs(3600))
            .unwrap();

        let deleted = cache.invalidate_namespace("ns1").unwrap();
        assert_eq!(deleted, 2);

        assert!(!cache.is_valid("ns1", "key1").unwrap());
        assert!(!cache.is_valid("ns1", "key2").unwrap());
        assert!(cache.is_valid("ns2", "key1").unwrap());
    }

    #[test]
    fn test_namespace_isolation() {
        let (_temp, cache) = create_test_cache();

        cache
            .set("ns1", "shared_key", b"value1", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("ns2", "shared_key", b"value2", Duration::from_secs(3600))
            .unwrap();

        let v1 = cache.get("ns1", "shared_key").unwrap().unwrap();
        let v2 = cache.get("ns2", "shared_key").unwrap().unwrap();

        assert_eq!(v1, b"value1");
        assert_eq!(v2, b"value2");
    }

    #[test]
    fn test_stats() {
        let (_temp, cache) = create_test_cache();

        cache
            .set("ns1", "key1", b"12345", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("ns1", "key2", b"67890", Duration::from_secs(3600))
            .unwrap();
        cache
            .set("ns2", "key1", b"abcde", Duration::from_secs(3600))
            .unwrap();

        let stats = cache.get_stats().unwrap();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.total_size_bytes, 15);
        assert_eq!(stats.namespace_count, 2);
    }

    #[test]
    fn test_cleanup_expired() {
        let (_temp, cache) = create_test_cache();

        // Add some expired entries
        let past = Utc::now() - chrono::Duration::seconds(100);
        cache
            .set_with_expiry("test_ns", "old1", b"data", past)
            .unwrap();
        cache
            .set_with_expiry("test_ns", "old2", b"data", past)
            .unwrap();

        // Add some valid entries
        cache
            .set("test_ns", "new1", b"data", Duration::from_secs(3600))
            .unwrap();

        let cleaned = cache.cleanup_expired().unwrap();
        assert_eq!(cleaned, 2);

        assert!(!cache.is_valid("test_ns", "old1").unwrap());
        assert!(!cache.is_valid("test_ns", "old2").unwrap());
        assert!(cache.is_valid("test_ns", "new1").unwrap());
    }

    #[test]
    fn test_config_update() {
        let (_temp, cache) = create_test_cache();

        let mut config = cache.get_config().unwrap();
        config.max_size_bytes = 1_000_000_000; // 1GB
        cache.set_config(&config).unwrap();

        let updated = cache.get_config().unwrap();
        assert_eq!(updated.max_size_bytes, 1_000_000_000);
    }
}
