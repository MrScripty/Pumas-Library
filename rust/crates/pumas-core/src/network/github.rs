//! GitHub API client with releases fetching and caching.
//!
//! Provides:
//! - GitHub releases API integration
//! - Three-tier caching: in-memory → disk → network
//! - Offline-first strategy with stale data fallback
//! - Rate limit handling

use crate::config::{AppId, NetworkConfig};
use crate::models::{CacheStatus, GitHubReleasesCache};
use crate::network::client::HttpClient;
use crate::network::retry::{retry_async, RetryConfig};
use crate::{PumasError, Result};
use chrono::{DateTime, Utc};
use mini_moka::sync::Cache;
use reqwest::StatusCode;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Re-export for convenience
pub use crate::models::{GitHubAsset, GitHubRelease};

/// Cache for GitHub releases.
pub struct ReleasesCache {
    /// In-memory cache with TTL.
    memory_cache: Cache<String, Vec<GitHubRelease>>,
    /// Path to disk cache.
    cache_dir: PathBuf,
    /// Default TTL for cache entries.
    default_ttl: Duration,
}

impl ReleasesCache {
    /// Create a new releases cache.
    pub fn new(cache_dir: PathBuf, ttl: Duration) -> Self {
        Self {
            memory_cache: Cache::builder()
                .time_to_live(ttl)
                .max_capacity(10)
                .build(),
            cache_dir,
            default_ttl: ttl,
        }
    }

    /// Get releases from memory cache.
    pub fn get_memory(&self, key: &str) -> Option<Vec<GitHubRelease>> {
        self.memory_cache.get(&key.to_string())
    }

    /// Store releases in memory cache.
    pub fn set_memory(&self, key: &str, releases: Vec<GitHubRelease>) {
        self.memory_cache.insert(key.to_string(), releases);
    }

    /// Get releases from disk cache.
    pub fn get_disk(&self, key: &str) -> Option<GitHubReleasesCache> {
        let path = self.disk_cache_path(key);
        if !path.exists() {
            return None;
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(cache) => Some(cache),
                Err(e) => {
                    warn!("Failed to parse disk cache {}: {}", path.display(), e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to read disk cache {}: {}", path.display(), e);
                None
            }
        }
    }

    /// Store releases in disk cache.
    pub fn set_disk(&self, key: &str, releases: &[GitHubRelease]) -> Result<()> {
        let path = self.disk_cache_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create cache directory: {}", e),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        let cache = GitHubReleasesCache {
            last_fetched: Utc::now().to_rfc3339(),
            ttl: self.default_ttl.as_secs(),
            releases: releases.to_vec(),
        };

        let contents = serde_json::to_string_pretty(&cache)?;
        std::fs::write(&path, contents).map_err(|e| PumasError::Io {
            message: format!("Failed to write disk cache: {}", e),
            path: Some(path),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Check if disk cache is valid (not expired).
    pub fn is_disk_cache_valid(&self, cache: &GitHubReleasesCache) -> bool {
        if let Ok(last_fetched) = DateTime::parse_from_rfc3339(&cache.last_fetched) {
            let age = Utc::now().signed_duration_since(last_fetched);
            age.num_seconds() < cache.ttl as i64
        } else {
            false
        }
    }

    /// Get cache status for a key.
    pub fn get_status(&self, key: &str, is_fetching: bool) -> CacheStatus {
        let disk_cache = self.get_disk(key);
        let has_cache = disk_cache.is_some();
        let is_valid = disk_cache
            .as_ref()
            .map(|c| self.is_disk_cache_valid(c))
            .unwrap_or(false);

        let (age_seconds, last_fetched, releases_count) = if let Some(cache) = disk_cache {
            let age = DateTime::parse_from_rfc3339(&cache.last_fetched)
                .map(|t| Utc::now().signed_duration_since(t).num_seconds() as u64)
                .ok();
            (age, Some(cache.last_fetched), Some(cache.releases.len() as u32))
        } else {
            (None, None, None)
        };

        CacheStatus {
            has_cache,
            is_valid,
            is_fetching,
            age_seconds,
            last_fetched,
            releases_count,
        }
    }

    /// Invalidate cache for a key.
    pub fn invalidate(&self, key: &str) {
        self.memory_cache.invalidate(&key.to_string());
        let path = self.disk_cache_path(key);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    fn disk_cache_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filename
        let safe_key = key.replace('/', "-");
        self.cache_dir.join(format!("github-releases-{}.json", safe_key))
    }
}

/// GitHub API client.
pub struct GitHubClient {
    http: Arc<HttpClient>,
    cache: ReleasesCache,
    /// Whether we're currently fetching releases.
    is_fetching: AtomicBool,
    /// Lock for coordinating fetches.
    fetch_lock: RwLock<()>,
}

impl GitHubClient {
    /// Create a new GitHub client.
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        let http = HttpClient::new()?;
        Ok(Self {
            http: Arc::new(http),
            cache: ReleasesCache::new(cache_dir, NetworkConfig::GITHUB_RELEASES_TTL),
            is_fetching: AtomicBool::new(false),
            fetch_lock: RwLock::new(()),
        })
    }

    /// Create a new GitHub client with custom TTL.
    pub fn with_ttl(cache_dir: PathBuf, ttl: Duration) -> Result<Self> {
        let http = HttpClient::new()?;
        Ok(Self {
            http: Arc::new(http),
            cache: ReleasesCache::new(cache_dir, ttl),
            is_fetching: AtomicBool::new(false),
            fetch_lock: RwLock::new(()),
        })
    }

    /// Get releases for a repository (offline-first strategy).
    ///
    /// Order of operations:
    /// 1. Check in-memory cache (instant)
    /// 2. Check disk cache if valid (fast path)
    /// 3. Return stale disk cache if available (offline support)
    /// 4. Network fetch only if force_refresh=true
    pub async fn get_releases(
        &self,
        repo: &str,
        force_refresh: bool,
    ) -> Result<Vec<GitHubRelease>> {
        let cache_key = repo.to_string();

        // 1. Check in-memory cache (unless force refresh)
        if !force_refresh {
            if let Some(releases) = self.cache.get_memory(&cache_key) {
                debug!("GitHub releases cache hit (memory) for {}", repo);
                return Ok(releases);
            }
        }

        // 2. Check disk cache
        if let Some(disk_cache) = self.cache.get_disk(&cache_key) {
            let is_valid = self.cache.is_disk_cache_valid(&disk_cache);

            if !force_refresh && is_valid {
                // Valid disk cache - use it and populate memory cache
                debug!("GitHub releases cache hit (disk) for {}", repo);
                self.cache.set_memory(&cache_key, disk_cache.releases.clone());
                return Ok(disk_cache.releases);
            }

            // 3. Stale cache available - try network, fall back to stale
            if !force_refresh {
                debug!("GitHub releases cache stale for {}, trying network", repo);
                let fetch_result: Result<Vec<GitHubRelease>> = self.fetch_releases_from_network(repo).await;
                match fetch_result {
                    Ok(releases) => {
                        self.cache.set_memory(&cache_key, releases.clone());
                        let _ = self.cache.set_disk(&cache_key, &releases);
                        return Ok(releases);
                    }
                    Err(e) => {
                        warn!(
                            "Network fetch failed for {}, using stale cache: {}",
                            repo, e
                        );
                        self.cache.set_memory(&cache_key, disk_cache.releases.clone());
                        return Ok(disk_cache.releases);
                    }
                }
            }
        }

        // 4. No cache or force refresh - fetch from network
        let releases: Vec<GitHubRelease> = self.fetch_releases_from_network(repo).await?;
        self.cache.set_memory(&cache_key, releases.clone());
        let _ = self.cache.set_disk(&cache_key, &releases);
        Ok(releases)
    }

    /// Get releases for an app by its ID.
    pub async fn get_releases_for_app(
        &self,
        app_id: AppId,
        force_refresh: bool,
    ) -> Result<Vec<GitHubRelease>> {
        self.get_releases(app_id.github_repo(), force_refresh).await
    }

    /// Get the latest non-prerelease release.
    pub async fn get_latest_release(
        &self,
        repo: &str,
        force_refresh: bool,
    ) -> Result<Option<GitHubRelease>> {
        let releases: Vec<GitHubRelease> = self.get_releases(repo, force_refresh).await?;
        Ok(releases.into_iter().find(|r| !r.prerelease))
    }

    /// Get a specific release by tag.
    pub async fn get_release_by_tag(
        &self,
        repo: &str,
        tag: &str,
        force_refresh: bool,
    ) -> Result<Option<GitHubRelease>> {
        let releases: Vec<GitHubRelease> = self.get_releases(repo, force_refresh).await?;
        Ok(releases.into_iter().find(|r| r.tag_name == tag))
    }

    /// Get cache status for a repository.
    pub fn get_cache_status(&self, repo: &str) -> CacheStatus {
        self.cache.get_status(repo, self.is_fetching.load(Ordering::SeqCst))
    }

    /// Invalidate cache for a repository.
    pub fn invalidate_cache(&self, repo: &str) {
        self.cache.invalidate(repo);
    }

    // Internal methods

    async fn fetch_releases_from_network(&self, repo: &str) -> Result<Vec<GitHubRelease>> {
        // Acquire fetch lock to prevent concurrent fetches for the same repo
        let _lock = self.fetch_lock.write().await;
        self.is_fetching.store(true, Ordering::SeqCst);

        let result = self.do_fetch_releases(repo).await;

        self.is_fetching.store(false, Ordering::SeqCst);
        result
    }

    async fn do_fetch_releases(&self, repo: &str) -> Result<Vec<GitHubRelease>> {
        let mut all_releases = Vec::new();
        let per_page = NetworkConfig::GITHUB_RELEASES_PER_PAGE;
        let max_pages = NetworkConfig::GITHUB_RELEASES_MAX_PAGES;

        for page in 1..=max_pages {
            let url = format!(
                "{}/repos/{}/releases?per_page={}&page={}",
                NetworkConfig::GITHUB_API_BASE,
                repo,
                per_page,
                page
            );

            let retry_config = RetryConfig::new()
                .with_max_attempts(3)
                .with_base_delay(Duration::from_secs(2));

            let http = self.http.clone();
            let url_clone = url.clone();

            let (result, stats) = retry_async(
                &retry_config,
                || {
                    let http = http.clone();
                    let url = url_clone.clone();
                    async move {
                        let headers = vec![
                            ("Accept".to_string(), "application/vnd.github.v3+json".to_string()),
                        ];
                        http.get_with_headers(&url, &headers).await
                    }
                },
                |e| e.is_retryable(),
            )
            .await;

            if stats.attempts > 1 {
                debug!(
                    "GitHub API request succeeded after {} attempts",
                    stats.attempts
                );
            }

            let response = result?;
            let status = response.status();

            if status == StatusCode::FORBIDDEN {
                // Rate limited - check if we have stale cache
                return Err(PumasError::RateLimited {
                    service: "GitHub".to_string(),
                    retry_after_secs: None,
                });
            }

            if !status.is_success() {
                return Err(PumasError::GitHubApi {
                    message: format!("GitHub API returned {}", status),
                    status_code: Some(status.as_u16()),
                });
            }

            let releases: Vec<GitHubRelease> = response.json().await.map_err(|e| {
                PumasError::Json {
                    message: format!("Failed to parse GitHub releases: {}", e),
                    source: None,
                }
            })?;

            let count = releases.len();
            all_releases.extend(releases);

            // If we got fewer than per_page, we've reached the end
            if count < per_page as usize {
                break;
            }
        }

        info!(
            "Fetched {} releases from GitHub for {}",
            all_releases.len(),
            repo
        );
        Ok(all_releases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_client() -> (GitHubClient, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let client = GitHubClient::new(temp_dir.path().to_path_buf()).unwrap();
        (client, temp_dir)
    }

    #[test]
    fn test_releases_cache_disk() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ReleasesCache::new(temp_dir.path().to_path_buf(), Duration::from_secs(3600));

        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: "Release 1.0.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: vec![],
            html_url: "https://github.com/test/repo/releases/v1.0.0".to_string(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }];

        // Save to disk
        cache.set_disk("test/repo", &releases).unwrap();

        // Read back
        let cached = cache.get_disk("test/repo").unwrap();
        assert_eq!(cached.releases.len(), 1);
        assert_eq!(cached.releases[0].tag_name, "v1.0.0");
        assert!(cache.is_disk_cache_valid(&cached));
    }

    #[test]
    fn test_releases_cache_memory() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ReleasesCache::new(temp_dir.path().to_path_buf(), Duration::from_secs(3600));

        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: "Release 1.0.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: vec![],
            html_url: "https://github.com/test/repo/releases/v1.0.0".to_string(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }];

        cache.set_memory("test/repo", releases.clone());
        let cached = cache.get_memory("test/repo").unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].tag_name, "v1.0.0");
    }

    #[test]
    fn test_cache_status() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ReleasesCache::new(temp_dir.path().to_path_buf(), Duration::from_secs(3600));

        // No cache
        let status = cache.get_status("test/repo", false);
        assert!(!status.has_cache);
        assert!(!status.is_valid);

        // Add cache
        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: "Release 1.0.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: vec![],
            html_url: "https://github.com/test/repo/releases/v1.0.0".to_string(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }];
        cache.set_disk("test/repo", &releases).unwrap();

        let status = cache.get_status("test/repo", false);
        assert!(status.has_cache);
        assert!(status.is_valid);
        assert_eq!(status.releases_count, Some(1));
    }

    #[test]
    fn test_cache_invalidate() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ReleasesCache::new(temp_dir.path().to_path_buf(), Duration::from_secs(3600));

        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: "Release 1.0.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: vec![],
            html_url: "https://github.com/test/repo/releases/v1.0.0".to_string(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }];

        cache.set_memory("test/repo", releases.clone());
        cache.set_disk("test/repo", &releases).unwrap();

        cache.invalidate("test/repo");

        assert!(cache.get_memory("test/repo").is_none());
        assert!(cache.get_disk("test/repo").is_none());
    }

    #[tokio::test]
    async fn test_client_creation() {
        let (client, _temp) = create_test_client();
        let status = client.get_cache_status("test/repo");
        assert!(!status.has_cache);
        assert!(!status.is_fetching);
    }
}
