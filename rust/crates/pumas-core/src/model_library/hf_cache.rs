//! HuggingFace search cache using SQLite.
//!
//! Provides intelligent caching for HuggingFace model searches:
//! - Search result caching with configurable TTL
//! - Model detail caching with lastModified-based invalidation
//! - LRU eviction when cache exceeds size limit
//! - Graceful degradation when database unavailable
//!
//! See CACHING.md for detailed documentation.

use crate::error::{PumasError, Result};
use crate::models::{DownloadOption, HuggingFaceModel};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Default maximum cache size (4GB).
const DEFAULT_MAX_SIZE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Default search TTL (24 hours).
const DEFAULT_SEARCH_TTL_SECONDS: u64 = 24 * 60 * 60;

/// Default lastModified check threshold (24 hours).
const DEFAULT_LAST_MODIFIED_CHECK_SECONDS: u64 = 24 * 60 * 60;

/// Default rate limit window (5 minutes).
const DEFAULT_RATE_LIMIT_WINDOW_SECONDS: u64 = 5 * 60;

/// Configuration for the HuggingFace cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HfCacheConfig {
    /// Maximum cache size in bytes (default: 4GB).
    pub max_size_bytes: u64,
    /// TTL for search results in seconds (default: 24 hours).
    pub search_ttl_seconds: u64,
    /// Threshold before checking lastModified (default: 24 hours).
    pub last_modified_check_threshold: u64,
    /// Whether background refresh is enabled (default: true).
    pub background_refresh_enabled: bool,
    /// Rate limit window in seconds (default: 5 minutes).
    pub rate_limit_window_seconds: u64,
}

impl Default for HfCacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: DEFAULT_MAX_SIZE_BYTES,
            search_ttl_seconds: DEFAULT_SEARCH_TTL_SECONDS,
            last_modified_check_threshold: DEFAULT_LAST_MODIFIED_CHECK_SECONDS,
            background_refresh_enabled: true,
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
        }
    }
}

/// Cached repository details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRepoDetails {
    /// Repository ID.
    pub repo_id: String,
    /// Last modified timestamp from HuggingFace.
    pub last_modified: Option<String>,
    /// Model name.
    pub name: String,
    /// Developer/organization.
    pub developer: String,
    /// Model kind (e.g., "text-generation").
    pub kind: String,
    /// Supported formats.
    pub formats: Vec<String>,
    /// Available quantizations.
    pub quants: Vec<String>,
    /// Download options with sizes.
    pub download_options: Vec<DownloadOption>,
    /// URL to model page.
    pub url: String,
    /// Download count.
    pub downloads: Option<u64>,
    /// Total size in bytes.
    pub total_size_bytes: Option<u64>,
    /// When this was cached.
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

impl From<CachedRepoDetails> for HuggingFaceModel {
    fn from(cached: CachedRepoDetails) -> Self {
        // Compute compatible engines from formats
        let compatible_engines = crate::models::detect_compatible_engines(&cached.formats);

        HuggingFaceModel {
            repo_id: cached.repo_id,
            name: cached.name,
            developer: cached.developer,
            kind: cached.kind,
            formats: cached.formats,
            quants: cached.quants,
            download_options: cached.download_options,
            url: cached.url,
            release_date: cached.last_modified,
            downloads: cached.downloads,
            total_size_bytes: cached.total_size_bytes,
            quant_sizes: None, // Deprecated field
            compatible_engines,
        }
    }
}

/// SQLite-based cache for HuggingFace searches.
pub struct HfSearchCache {
    /// Database connection (wrapped for thread safety).
    conn: Arc<Mutex<Connection>>,
    /// Cache configuration.
    config: HfCacheConfig,
}

impl HfSearchCache {
    /// Create a new cache at the specified path.
    ///
    /// Creates the database and tables if they don't exist.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        Self::with_config(db_path, HfCacheConfig::default())
    }

    /// Create a new cache with custom configuration.
    pub fn with_config(db_path: impl AsRef<Path>, config: HfCacheConfig) -> Result<Self> {
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

        let cache = Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        };

        cache.init_schema()?;
        cache.init_config()?;

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
            -- Search results cache
            CREATE TABLE IF NOT EXISTS search_cache (
                query_normalized TEXT NOT NULL,
                kind TEXT,
                result_limit INTEGER NOT NULL,
                result_offset INTEGER NOT NULL,
                result_repo_ids TEXT NOT NULL,
                searched_at TEXT NOT NULL,
                PRIMARY KEY (query_normalized, kind, result_limit, result_offset)
            );

            CREATE INDEX IF NOT EXISTS idx_search_cache_time
                ON search_cache(searched_at);

            -- Repository details cache
            CREATE TABLE IF NOT EXISTS repo_details (
                repo_id TEXT PRIMARY KEY,
                last_modified TEXT,
                name TEXT NOT NULL,
                developer TEXT NOT NULL,
                kind TEXT NOT NULL,
                formats TEXT NOT NULL,
                quants TEXT NOT NULL,
                download_options TEXT,
                url TEXT NOT NULL,
                downloads INTEGER,
                total_size_bytes INTEGER,
                cached_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                data_size_bytes INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_repo_last_accessed
                ON repo_details(last_accessed);
            CREATE INDEX IF NOT EXISTS idx_repo_last_modified
                ON repo_details(last_modified);

            -- Configuration table
            CREATE TABLE IF NOT EXISTS cache_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to initialize schema: {}", e),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Initialize default configuration values.
    fn init_config(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let defaults = [
            ("max_size_bytes", self.config.max_size_bytes.to_string()),
            (
                "search_ttl_seconds",
                self.config.search_ttl_seconds.to_string(),
            ),
            (
                "last_modified_check_threshold",
                self.config.last_modified_check_threshold.to_string(),
            ),
            (
                "background_refresh_enabled",
                self.config.background_refresh_enabled.to_string(),
            ),
            (
                "rate_limit_window_seconds",
                self.config.rate_limit_window_seconds.to_string(),
            ),
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

    /// Get current configuration.
    pub fn get_config(&self) -> Result<HfCacheConfig> {
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

        Ok(HfCacheConfig {
            max_size_bytes: get_value("max_size_bytes", &DEFAULT_MAX_SIZE_BYTES.to_string())
                .parse()
                .unwrap_or(DEFAULT_MAX_SIZE_BYTES),
            search_ttl_seconds: get_value(
                "search_ttl_seconds",
                &DEFAULT_SEARCH_TTL_SECONDS.to_string(),
            )
            .parse()
            .unwrap_or(DEFAULT_SEARCH_TTL_SECONDS),
            last_modified_check_threshold: get_value(
                "last_modified_check_threshold",
                &DEFAULT_LAST_MODIFIED_CHECK_SECONDS.to_string(),
            )
            .parse()
            .unwrap_or(DEFAULT_LAST_MODIFIED_CHECK_SECONDS),
            background_refresh_enabled: get_value("background_refresh_enabled", "true")
                .parse()
                .unwrap_or(true),
            rate_limit_window_seconds: get_value(
                "rate_limit_window_seconds",
                &DEFAULT_RATE_LIMIT_WINDOW_SECONDS.to_string(),
            )
            .parse()
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECONDS),
        })
    }

    /// Update configuration.
    pub fn set_config(&self, config: &HfCacheConfig) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let values = [
            ("max_size_bytes", config.max_size_bytes.to_string()),
            ("search_ttl_seconds", config.search_ttl_seconds.to_string()),
            (
                "last_modified_check_threshold",
                config.last_modified_check_threshold.to_string(),
            ),
            (
                "background_refresh_enabled",
                config.background_refresh_enabled.to_string(),
            ),
            (
                "rate_limit_window_seconds",
                config.rate_limit_window_seconds.to_string(),
            ),
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

    /// Normalize a search query for cache lookup.
    pub fn normalize_query(query: &str) -> String {
        query.trim().to_lowercase()
    }

    /// Get cached search results if available and fresh.
    pub fn get_search_results(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Option<Vec<HuggingFaceModel>>> {
        let query_normalized = Self::normalize_query(query);
        let config = self.get_config()?;

        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        // Check search cache
        let cached: Option<(String, String)> = conn
            .query_row(
                r#"
                SELECT result_repo_ids, searched_at
                FROM search_cache
                WHERE query_normalized = ?1
                  AND (kind = ?2 OR (kind IS NULL AND ?2 IS NULL))
                  AND result_limit = ?3
                  AND result_offset = ?4
                "#,
                params![query_normalized, kind, limit as i64, offset as i64],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query search cache: {}", e),
                source: Some(e),
            })?;

        let (repo_ids_json, searched_at_str) = match cached {
            Some(c) => c,
            None => return Ok(None),
        };

        // Check if search is still fresh
        let searched_at = chrono::DateTime::parse_from_rfc3339(&searched_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_e| PumasError::Database {
                message: "Invalid searched_at timestamp".to_string(),
                source: None,
            })?;

        let age = chrono::Utc::now()
            .signed_duration_since(searched_at)
            .num_seconds() as u64;

        if age > config.search_ttl_seconds {
            debug!(
                "Search cache expired for '{}' (age: {}s, ttl: {}s)",
                query_normalized, age, config.search_ttl_seconds
            );
            return Ok(None);
        }

        // Parse repo IDs
        let repo_ids: Vec<String> = serde_json::from_str(&repo_ids_json).map_err(|e| {
            PumasError::Database {
                message: format!("Failed to parse repo IDs: {}", e),
                source: None,
            }
        })?;

        // Get details for each repo
        let mut models = Vec::with_capacity(repo_ids.len());
        let now = chrono::Utc::now().to_rfc3339();

        for repo_id in &repo_ids {
            if let Some(details) = self.get_repo_details_internal(&conn, repo_id, &now)? {
                models.push(details.into());
            }
        }

        debug!(
            "Cache hit for search '{}': {} models",
            query_normalized,
            models.len()
        );

        Ok(Some(models))
    }

    /// Internal helper to get repo details without locking.
    fn get_repo_details_internal(
        &self,
        conn: &Connection,
        repo_id: &str,
        now: &str,
    ) -> Result<Option<CachedRepoDetails>> {
        let row: Option<(
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            Option<i64>,
            Option<i64>,
            String,
        )> = conn
            .query_row(
                r#"
                SELECT repo_id, last_modified, name, developer, kind,
                       formats, quants, download_options, url,
                       downloads, total_size_bytes, cached_at
                FROM repo_details
                WHERE repo_id = ?1
                "#,
                params![repo_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query repo details: {}", e),
                source: Some(e),
            })?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Update last_accessed
        let _ = conn.execute(
            "UPDATE repo_details SET last_accessed = ?1 WHERE repo_id = ?2",
            params![now, repo_id],
        );

        let formats: Vec<String> =
            serde_json::from_str(&row.5).unwrap_or_default();
        let quants: Vec<String> =
            serde_json::from_str(&row.6).unwrap_or_default();
        let download_options: Vec<DownloadOption> = row
            .7
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let cached_at = chrono::DateTime::parse_from_rfc3339(&row.11)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        Ok(Some(CachedRepoDetails {
            repo_id: row.0,
            last_modified: row.1,
            name: row.2,
            developer: row.3,
            kind: row.4,
            formats,
            quants,
            download_options,
            url: row.8,
            downloads: row.9.map(|d| d as u64),
            total_size_bytes: row.10.map(|s| s as u64),
            cached_at,
        }))
    }

    /// Get cached repo details if available.
    pub fn get_repo_details(&self, repo_id: &str) -> Result<Option<CachedRepoDetails>> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let now = chrono::Utc::now().to_rfc3339();
        self.get_repo_details_internal(&conn, repo_id, &now)
    }

    /// Check if cached repo needs refresh based on lastModified.
    ///
    /// Returns true if:
    /// - No cache exists
    /// - Cache is older than threshold AND search result has newer lastModified
    pub fn needs_refresh(
        &self,
        repo_id: &str,
        search_last_modified: Option<&str>,
    ) -> Result<bool> {
        let config = self.get_config()?;
        let cached = self.get_repo_details(repo_id)?;

        let cached = match cached {
            Some(c) => c,
            None => return Ok(true), // No cache, needs fetch
        };

        // Check age threshold
        let age = chrono::Utc::now()
            .signed_duration_since(cached.cached_at)
            .num_seconds() as u64;

        if age < config.last_modified_check_threshold {
            return Ok(false); // Cache is fresh enough
        }

        // Compare lastModified timestamps
        match (cached.last_modified.as_ref(), search_last_modified) {
            (Some(cached_lm), Some(search_lm)) => {
                // Parse and compare
                let cached_dt = chrono::DateTime::parse_from_rfc3339(cached_lm).ok();
                let search_dt = chrono::DateTime::parse_from_rfc3339(search_lm).ok();

                match (cached_dt, search_dt) {
                    (Some(c), Some(s)) => Ok(s > c), // Refresh if search is newer
                    _ => Ok(true), // Can't compare, refresh to be safe
                }
            }
            (None, Some(_)) => Ok(true), // We have no lastModified, refresh
            _ => Ok(false),              // No new lastModified info, keep cache
        }
    }

    /// Cache search results.
    pub fn cache_search_results(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
        offset: usize,
        repo_ids: &[String],
    ) -> Result<()> {
        let query_normalized = Self::normalize_query(query);
        let repo_ids_json = serde_json::to_string(repo_ids).map_err(|e| PumasError::Json {
            message: format!("Failed to serialize repo IDs: {}", e),
            source: None,
        })?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO search_cache
            (query_normalized, kind, result_limit, result_offset, result_repo_ids, searched_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                query_normalized,
                kind,
                limit as i64,
                offset as i64,
                repo_ids_json,
                now
            ],
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to cache search results: {}", e),
            source: Some(e),
        })?;

        debug!(
            "Cached search '{}' with {} results",
            query_normalized,
            repo_ids.len()
        );

        Ok(())
    }

    /// Cache repository details.
    pub fn cache_repo_details(&self, model: &HuggingFaceModel) -> Result<()> {
        self.cache_repo_details_with_options(model, &model.download_options)
    }

    /// Cache repository details with explicit download options.
    pub fn cache_repo_details_with_options(
        &self,
        model: &HuggingFaceModel,
        download_options: &[DownloadOption],
    ) -> Result<()> {
        let formats_json = serde_json::to_string(&model.formats).unwrap_or_else(|_| "[]".into());
        let quants_json = serde_json::to_string(&model.quants).unwrap_or_else(|_| "[]".into());
        let download_options_json =
            serde_json::to_string(download_options).unwrap_or_else(|_| "[]".into());

        let now = chrono::Utc::now().to_rfc3339();

        // Estimate data size for LRU tracking
        let data_size = formats_json.len()
            + quants_json.len()
            + download_options_json.len()
            + model.repo_id.len()
            + model.name.len()
            + model.developer.len()
            + model.kind.len()
            + model.url.len()
            + 200; // Base overhead

        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO repo_details
            (repo_id, last_modified, name, developer, kind, formats, quants,
             download_options, url, downloads, total_size_bytes,
             cached_at, last_accessed, data_size_bytes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                model.repo_id,
                model.release_date,
                model.name,
                model.developer,
                model.kind,
                formats_json,
                quants_json,
                download_options_json,
                model.url,
                model.downloads.map(|d| d as i64),
                model.total_size_bytes.map(|s| s as i64),
                now,
                now,
                data_size as i64
            ],
        )
        .map_err(|e| PumasError::Database {
            message: format!("Failed to cache repo details: {}", e),
            source: Some(e),
        })?;

        debug!("Cached repo details for '{}'", model.repo_id);

        // Check if eviction is needed (async-friendly: don't block)
        drop(conn);
        let _ = self.check_and_evict();

        Ok(())
    }

    /// Check cache size and evict if necessary.
    pub fn check_and_evict(&self) -> Result<usize> {
        let config = self.get_config()?;

        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        // Get current cache size
        let current_size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(data_size_bytes), 0) FROM repo_details",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if (current_size as u64) <= config.max_size_bytes {
            return Ok(0);
        }

        let excess = current_size as u64 - config.max_size_bytes;
        debug!(
            "Cache size {}MB exceeds limit {}MB, evicting...",
            current_size / 1_000_000,
            config.max_size_bytes / 1_000_000
        );

        // Get oldest accessed entries to evict
        let mut stmt = conn
            .prepare(
                r#"
                SELECT repo_id, data_size_bytes
                FROM repo_details
                ORDER BY last_accessed ASC
                "#,
            )
            .map_err(|e| PumasError::Database {
                message: format!("Failed to prepare eviction query: {}", e),
                source: Some(e),
            })?;

        let entries: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| PumasError::Database {
                message: format!("Failed to query for eviction: {}", e),
                source: Some(e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        // Evict until under limit
        let mut evicted = 0u64;
        let mut evicted_count = 0;

        for (repo_id, size) in entries {
            if evicted >= excess {
                break;
            }

            conn.execute("DELETE FROM repo_details WHERE repo_id = ?1", params![repo_id])
                .ok();

            evicted += size as u64;
            evicted_count += 1;
        }

        // Clean up orphaned search cache entries
        conn.execute(
            r#"
            DELETE FROM search_cache
            WHERE NOT EXISTS (
                SELECT 1 FROM repo_details
                WHERE repo_details.repo_id IN (
                    SELECT value FROM json_each(search_cache.result_repo_ids)
                )
            )
            "#,
            [],
        )
        .ok();

        debug!("Evicted {} entries ({}MB)", evicted_count, evicted / 1_000_000);

        Ok(evicted_count)
    }

    /// Get cache statistics.
    pub fn get_stats(&self) -> Result<CacheStats> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        let search_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM search_cache", [], |row| row.get(0))
            .unwrap_or(0);

        let repo_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM repo_details", [], |row| row.get(0))
            .unwrap_or(0);

        let total_size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(data_size_bytes), 0) FROM repo_details",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        drop(conn);
        let config = self.get_config()?;

        Ok(CacheStats {
            search_count: search_count as usize,
            repo_count: repo_count as usize,
            total_size_bytes: total_size as u64,
            max_size_bytes: config.max_size_bytes,
        })
    }

    /// Clear all cached data.
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| PumasError::Database {
            message: format!("Failed to lock database: {}", e),
            source: None,
        })?;

        conn.execute("DELETE FROM search_cache", [])
            .map_err(|e| PumasError::Database {
                message: format!("Failed to clear search cache: {}", e),
                source: Some(e),
            })?;

        conn.execute("DELETE FROM repo_details", [])
            .map_err(|e| PumasError::Database {
                message: format!("Failed to clear repo details: {}", e),
                source: Some(e),
            })?;

        debug!("Cleared all cache data");

        Ok(())
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cached searches.
    pub search_count: usize,
    /// Number of cached repositories.
    pub repo_count: usize,
    /// Total cache size in bytes.
    pub total_size_bytes: u64,
    /// Maximum allowed size in bytes.
    pub max_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (TempDir, HfSearchCache) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_cache.sqlite");
        let cache = HfSearchCache::new(&db_path).unwrap();
        (temp_dir, cache)
    }

    fn create_test_model(repo_id: &str) -> HuggingFaceModel {
        HuggingFaceModel {
            repo_id: repo_id.to_string(),
            name: "Test Model".to_string(),
            developer: "TestDev".to_string(),
            kind: "text-generation".to_string(),
            formats: vec!["gguf".to_string()],
            quants: vec!["Q4_K_M".to_string()],
            download_options: vec![DownloadOption {
                quant: "Q4_K_M".to_string(),
                size_bytes: Some(4_000_000_000),
                file_group: None,
            }],
            url: format!("https://huggingface.co/{}", repo_id),
            release_date: Some("2024-01-15T10:00:00Z".to_string()),
            downloads: Some(1000),
            total_size_bytes: Some(4_000_000_000),
            quant_sizes: None,
            compatible_engines: vec![],
        }
    }

    #[test]
    fn test_cache_creation() {
        let (_temp, cache) = create_test_cache();
        let config = cache.get_config().unwrap();
        assert_eq!(config.max_size_bytes, DEFAULT_MAX_SIZE_BYTES);
    }

    #[test]
    fn test_query_normalization() {
        assert_eq!(HfSearchCache::normalize_query("  Llama GGUF  "), "llama gguf");
        assert_eq!(HfSearchCache::normalize_query("TEST"), "test");
    }

    #[test]
    fn test_cache_repo_details() {
        let (_temp, cache) = create_test_cache();
        let model = create_test_model("test/model");

        cache.cache_repo_details(&model).unwrap();

        let cached = cache.get_repo_details("test/model").unwrap();
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.name, "Test Model");
        assert_eq!(cached.download_options.len(), 1);
    }

    #[test]
    fn test_cache_search_results() {
        let (_temp, cache) = create_test_cache();

        // Cache some models first
        cache
            .cache_repo_details(&create_test_model("test/model1"))
            .unwrap();
        cache
            .cache_repo_details(&create_test_model("test/model2"))
            .unwrap();

        // Cache search results
        let repo_ids = vec!["test/model1".to_string(), "test/model2".to_string()];
        cache
            .cache_search_results("test query", None, 25, 0, &repo_ids)
            .unwrap();

        // Retrieve
        let results = cache
            .get_search_results("test query", None, 25, 0)
            .unwrap();
        assert!(results.is_some());
        assert_eq!(results.unwrap().len(), 2);
    }

    #[test]
    fn test_needs_refresh() {
        let (_temp, cache) = create_test_cache();
        let model = create_test_model("test/model");
        cache.cache_repo_details(&model).unwrap();

        // Same lastModified should not need refresh
        let needs = cache
            .needs_refresh("test/model", Some("2024-01-15T10:00:00Z"))
            .unwrap();
        assert!(!needs);

        // Non-existent model needs refresh
        let needs = cache.needs_refresh("nonexistent", None).unwrap();
        assert!(needs);
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
