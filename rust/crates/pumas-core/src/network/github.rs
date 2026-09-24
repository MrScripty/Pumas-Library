//! GitHub API client with releases fetching and caching.
//!
//! Provides:
//! - GitHub releases API integration
//! - Three-tier caching: in-memory → disk → network
//! - Offline-first strategy with stale data fallback
//! - Rate limit handling

use super::web_source::{CacheStrategy, WebSource, WebSourceId};
use crate::config::{AppId, NetworkConfig};
use crate::models::{CacheStatus, GitHubReleasesCache};
use crate::network::client::HttpClient;
use crate::network::retry::{retry_async, RetryConfig};
use crate::{PumasError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mini_moka::sync::Cache;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
#[cfg(test)]
use tokio::sync::Notify;
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{debug, info, warn};

// Re-export for convenience
pub use crate::models::{GitHubAsset, GitHubRelease};

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredReleasesCache {
    #[serde(flatten)]
    snapshot: GitHubReleasesCache,
    #[serde(default)]
    listing_complete: bool,
}

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
            memory_cache: Cache::builder().time_to_live(ttl).max_capacity(10).build(),
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
        self.get_disk_entry(key).map(|entry| entry.snapshot)
    }

    fn get_disk_entry(&self, key: &str) -> Option<StoredReleasesCache> {
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

        let contents = serde_json::to_string_pretty(&StoredReleasesCache {
            snapshot: cache,
            listing_complete: true,
        })?;
        std::fs::write(&path, contents).map_err(|e| PumasError::Io {
            message: format!("Failed to write disk cache: {}", e),
            path: Some(path),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Get releases from disk cache without blocking the async runtime.
    pub async fn get_disk_async(&self, key: &str) -> Option<GitHubReleasesCache> {
        self.get_disk_entry_async(key)
            .await
            .map(|entry| entry.snapshot)
    }

    async fn get_disk_entry_async(&self, key: &str) -> Option<StoredReleasesCache> {
        let path = self.disk_cache_path(key);

        match fs::read_to_string(&path).await {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(cache) => Some(cache),
                Err(e) => {
                    warn!("Failed to parse disk cache {}: {}", path.display(), e);
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                warn!("Failed to read disk cache {}: {}", path.display(), e);
                None
            }
        }
    }

    /// Store releases in disk cache without blocking the async runtime.
    pub async fn set_disk_async(&self, key: &str, releases: &[GitHubRelease]) -> Result<()> {
        let path = self.disk_cache_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| PumasError::Io {
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

        let contents = serde_json::to_string_pretty(&StoredReleasesCache {
            snapshot: cache,
            listing_complete: true,
        })?;
        fs::write(&path, contents)
            .await
            .map_err(|e| PumasError::Io {
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

    /// Get cache status for a key without blocking the async runtime.
    pub async fn get_status_async(&self, key: &str, is_fetching: bool) -> CacheStatus {
        let disk_entry = self.get_disk_entry_async(key).await;
        let has_cache = disk_entry.is_some();
        let is_valid = disk_entry
            .as_ref()
            .map(|entry| {
                self.is_disk_cache_valid(&entry.snapshot) && !may_be_legacy_truncated(key, entry)
            })
            .unwrap_or(false);

        let (age_seconds, last_fetched, releases_count) = if let Some(entry) = disk_entry {
            let cache = entry.snapshot;
            let age = DateTime::parse_from_rfc3339(&cache.last_fetched)
                .map(|t| Utc::now().signed_duration_since(t).num_seconds() as u64)
                .ok();
            (
                age,
                Some(cache.last_fetched),
                Some(cache.releases.len() as u32),
            )
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
        self.cache_dir
            .join(format!("github-releases-{}.json", safe_key))
    }
}

/// Cloneable failure details for callers sharing one fetch. The owner keeps its
/// original PumasError; followers retain the actionable public category.
#[derive(Clone)]
enum FetchFailure {
    RateLimited {
        service: String,
        retry_after_secs: Option<u64>,
    },
    GitHubApi {
        message: String,
        status_code: Option<u16>,
    },
    Network {
        message: String,
        cause: Option<String>,
    },
    Other(String),
}

impl FetchFailure {
    fn from_error(error: &PumasError) -> Self {
        match error {
            PumasError::RateLimited {
                service,
                retry_after_secs,
            } => Self::RateLimited {
                service: service.clone(),
                retry_after_secs: *retry_after_secs,
            },
            PumasError::GitHubApi {
                message,
                status_code,
            } => Self::GitHubApi {
                message: message.clone(),
                status_code: *status_code,
            },
            PumasError::Network { message, cause } => Self::Network {
                message: message.clone(),
                cause: cause.clone(),
            },
            _ => Self::Other(error.to_string()),
        }
    }

    fn into_error(self) -> PumasError {
        match self {
            Self::RateLimited {
                service,
                retry_after_secs,
            } => PumasError::RateLimited {
                service,
                retry_after_secs,
            },
            Self::GitHubApi {
                message,
                status_code,
            } => PumasError::GitHubApi {
                message,
                status_code,
            },
            Self::Network { message, cause } => PumasError::Network { message, cause },
            Self::Other(message) => PumasError::Network {
                message,
                cause: None,
            },
        }
    }
}

type FetchResult = std::result::Result<Vec<GitHubRelease>, FetchFailure>;

struct FetchingReset<'a>(&'a AtomicBool);

impl Drop for FetchingReset<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// Before exhaustive Torch pagination, branch versions could stop after one
// or ten full pages. New caches mark completion even at those exact counts.
fn may_be_legacy_truncated(repo: &str, entry: &StoredReleasesCache) -> bool {
    repo == AppId::Torch.github_repo()
        && !entry.listing_complete
        && matches!(entry.snapshot.releases.len(), 100 | 1000)
}

const TORCH_RELEASES_MAX_PAGES: u32 = 20;
const TORCH_RELEASES_DEADLINE: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
struct ReleasePagePolicy {
    max_pages: u32,
    require_complete: bool,
    deadline: Option<Duration>,
}

fn page_policy(repo: &str) -> ReleasePagePolicy {
    if repo == AppId::Torch.github_repo() {
        ReleasePagePolicy {
            max_pages: TORCH_RELEASES_MAX_PAGES,
            require_complete: true,
            deadline: Some(TORCH_RELEASES_DEADLINE),
        }
    } else {
        ReleasePagePolicy {
            max_pages: NetworkConfig::GITHUB_RELEASES_MAX_PAGES,
            require_complete: false,
            deadline: None,
        }
    }
}

/// Torch is complete only after a short page; other repos retain the historical
/// page budget. An exact multiple needs one final empty request for proof.
async fn collect_release_pages<F, Fut>(
    per_page: usize,
    policy: ReleasePagePolicy,
    mut fetch_page: F,
) -> Result<Vec<GitHubRelease>>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<Vec<GitHubRelease>>>,
{
    let collect = async {
        let mut all_releases = Vec::new();
        for page in 1..=policy.max_pages {
            let releases = fetch_page(page).await?;
            let count = releases.len();
            all_releases.extend(releases);
            if count < per_page {
                return Ok(all_releases);
            }
            if page == policy.max_pages {
                if policy.require_complete {
                    return Err(PumasError::GitHubApi {
                        message: format!(
                            "Torch release listing exceeded the {page}-page budget before its end"
                        ),
                        status_code: None,
                    });
                }
                return Ok(all_releases);
            }
        }
        Err(PumasError::GitHubApi {
            message: "GitHub release page budget is zero".into(),
            status_code: None,
        })
    };
    if let Some(deadline) = policy.deadline {
        tokio::time::timeout(deadline, collect)
            .await
            .map_err(|_| PumasError::GitHubApi {
                message: "Torch release listing timed out before its end".into(),
                status_code: None,
            })?
    } else {
        collect.await
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
    /// Pending fetch operations - allows request coalescing.
    /// When a fetch is in progress, other callers subscribe to receive the same result.
    pending_fetches: Mutex<HashMap<String, watch::Receiver<Option<FetchResult>>>>,
    #[cfg(test)]
    follower_joined: Notify,
}

struct LlamaCppReleaseVariant {
    id: &'static str,
    label: &'static str,
    sort_order: u8,
}

impl LlamaCppReleaseVariant {
    fn new(id: &'static str, label: &'static str, sort_order: u8) -> Self {
        Self {
            id,
            label,
            sort_order,
        }
    }
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
            pending_fetches: Mutex::new(HashMap::new()),
            #[cfg(test)]
            follower_joined: Notify::new(),
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
            pending_fetches: Mutex::new(HashMap::new()),
            #[cfg(test)]
            follower_joined: Notify::new(),
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
        if let Some(disk_entry) = self.cache.get_disk_entry_async(&cache_key).await {
            let legacy_truncated = may_be_legacy_truncated(repo, &disk_entry);
            let disk_cache = disk_entry.snapshot;
            let is_valid = self.cache.is_disk_cache_valid(&disk_cache);

            if !force_refresh && is_valid && !legacy_truncated {
                // Valid disk cache - use it and populate memory cache
                debug!("GitHub releases cache hit (disk) for {}", repo);
                self.cache
                    .set_memory(&cache_key, disk_cache.releases.clone());
                return Ok(disk_cache.releases);
            }

            // 3. Stale or possibly truncated cache: refresh, then use only a
            // listing known to be complete if the network fails.
            if !force_refresh {
                debug!("GitHub releases cache requires refresh for {}", repo);
                let fetch_result: Result<Vec<GitHubRelease>> =
                    self.fetch_releases_from_network(repo).await;
                match fetch_result {
                    Ok(releases) => {
                        self.cache.set_memory(&cache_key, releases.clone());
                        let _ = self.cache.set_disk_async(&cache_key, &releases).await;
                        return Ok(releases);
                    }
                    Err(e) if !legacy_truncated => {
                        warn!(
                            "Network fetch failed for {}, using stale cache: {}",
                            repo, e
                        );
                        self.cache
                            .set_memory(&cache_key, disk_cache.releases.clone());
                        return Ok(disk_cache.releases);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // 4. No cache or force refresh - fetch from network
        let releases: Vec<GitHubRelease> = self.fetch_releases_from_network(repo).await?;
        self.cache.set_memory(&cache_key, releases.clone());
        let _ = self.cache.set_disk_async(&cache_key, &releases).await;
        Ok(releases)
    }

    /// Get releases for an app by its ID, enriched with platform-specific archive sizes.
    ///
    /// Fetches releases from GitHub (with caching) and populates `archive_size`
    /// from platform-matched assets (e.g., selects the correct Ollama binary).
    pub async fn get_releases_for_app(
        &self,
        app_id: AppId,
        force_refresh: bool,
    ) -> Result<Vec<GitHubRelease>> {
        let releases = self
            .get_releases(app_id.github_repo(), force_refresh)
            .await?;

        if app_id == AppId::LlamaCpp {
            return Ok(Self::expand_llama_cpp_release_variants(releases));
        }

        let mut releases = releases;

        // Populate archive_size from platform-matched assets
        Self::populate_archive_sizes(&mut releases, app_id);

        Ok(releases)
    }

    /// Populate archive_size from platform-matched release assets.
    /// For Ollama, this selects the binary for the current platform (e.g., ollama-linux-amd64.tgz).
    fn populate_archive_sizes(releases: &mut [GitHubRelease], app_id: AppId) {
        match app_id {
            AppId::Ollama => {
                for release in releases.iter_mut() {
                    if let Some(asset) = Self::find_ollama_asset_for_platform(&release.assets) {
                        release.archive_size = Some(asset.size);
                    }
                }
            }
            _ => {
                // Default: use the largest asset as the archive size
                for release in releases.iter_mut() {
                    if let Some(largest) = release.assets.iter().max_by_key(|a| a.size) {
                        release.archive_size = Some(largest.size);
                    }
                }
            }
        }
    }

    /// Find the Ollama binary asset matching the current platform.
    /// Uses exact matching to avoid selecting variant builds (ROCm, Jetpack, etc.).
    fn find_ollama_asset_for_platform(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
        let os = std::env::consts::OS;
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => std::env::consts::ARCH,
        };

        // Exact patterns for standard binaries (excludes -rocm, -jetpack variants)
        let exact_patterns = [
            format!("ollama-{}-{}.tar.zst", os, arch), // Primary (current format)
            format!("ollama-{}-{}.tgz", os, arch),     // Legacy format
            format!("ollama-{}-{}.tar.gz", os, arch),  // Legacy format
            format!("ollama-{}-{}.zip", os, arch),     // Windows
        ];

        assets.iter().find(|a| exact_patterns.contains(&a.name))
    }

    fn expand_llama_cpp_release_variants(releases: Vec<GitHubRelease>) -> Vec<GitHubRelease> {
        let os = std::env::consts::OS;
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => std::env::consts::ARCH,
        };
        releases
            .into_iter()
            .flat_map(|release| {
                Self::expand_llama_cpp_release_variants_for_platform(release, os, arch)
            })
            .collect()
    }

    fn expand_llama_cpp_release_variants_for_platform(
        release: GitHubRelease,
        os: &str,
        arch: &str,
    ) -> Vec<GitHubRelease> {
        let platform = match os {
            "linux" => "ubuntu",
            "macos" => "macos",
            "windows" => "win",
            _ => os,
        };
        let mut variants = release
            .assets
            .iter()
            .filter_map(|asset| {
                let variant = Self::llama_cpp_asset_variant(asset, os, platform, arch)?;
                Some((variant, asset.clone()))
            })
            .collect::<Vec<_>>();
        variants.sort_by_key(|(variant, _)| variant.sort_order);
        variants
            .into_iter()
            .map(|(variant, asset)| {
                let mut variant_release = release.clone();
                variant_release.tag_name = format!("{}+{}", release.tag_name, variant.id);
                variant_release.name = format!("{} ({})", release.tag_name, variant.label);
                variant_release.assets = vec![asset.clone()];
                variant_release.archive_size = Some(asset.size);
                variant_release.total_size = Some(asset.size);
                variant_release
            })
            .collect()
    }

    fn llama_cpp_asset_variant(
        asset: &GitHubAsset,
        os: &str,
        platform: &str,
        arch: &str,
    ) -> Option<LlamaCppReleaseVariant> {
        let name = asset.name.to_ascii_lowercase();
        if !Self::is_llama_cpp_platform_archive(&name, platform, arch) {
            return None;
        }
        if Self::is_llama_cpp_cpu_asset_name(&name) {
            return Some(LlamaCppReleaseVariant::new("cpu", "CPU", 0));
        }
        if name.contains("vulkan") {
            return Some(LlamaCppReleaseVariant::new("vulkan", "Vulkan", 10));
        }
        if name.contains("rocm") {
            return Some(LlamaCppReleaseVariant::new("rocm", "ROCm", 20));
        }
        if name.contains("cuda-13") {
            return Some(LlamaCppReleaseVariant::new("cuda-13", "CUDA 13", 30));
        }
        if name.contains("cuda-12") || name.contains("cudart") {
            return Some(LlamaCppReleaseVariant::new("cuda-12", "CUDA 12", 31));
        }
        if name.contains("hip") && os == "windows" {
            return Some(LlamaCppReleaseVariant::new("hip", "HIP", 40));
        }
        if name.contains("sycl-fp32") {
            return Some(LlamaCppReleaseVariant::new("sycl-fp32", "SYCL FP32", 50));
        }
        if name.contains("sycl-fp16") {
            return Some(LlamaCppReleaseVariant::new("sycl-fp16", "SYCL FP16", 51));
        }
        if name.contains("sycl") {
            return Some(LlamaCppReleaseVariant::new("sycl", "SYCL", 52));
        }
        None
    }

    fn is_llama_cpp_cpu_asset_name(name: &str) -> bool {
        let excluded = [
            "source",
            "cudart",
            "cuda",
            "vulkan",
            "rocm",
            "hip",
            "sycl",
            "metal",
            "kompute",
            "opencl",
            "musl",
            "openvino",
            "openeuler",
            "kleidiai",
        ];
        !excluded.iter().any(|pattern| name.contains(pattern))
    }

    fn is_llama_cpp_platform_archive(name: &str, platform: &str, arch: &str) -> bool {
        name.starts_with("llama-")
            && name.contains("-bin-")
            && name.contains(platform)
            && name.contains(arch)
            && (name.ends_with(".zip") || name.ends_with(".tar.gz") || name.ends_with(".tgz"))
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
    pub async fn get_cache_status(&self, repo: &str) -> CacheStatus {
        self.cache
            .get_status_async(repo, self.is_fetching.load(Ordering::SeqCst))
            .await
    }

    /// Invalidate cache for a repository.
    pub fn invalidate_cache(&self, repo: &str) {
        self.cache.invalidate(repo);
    }

    // Internal methods

    /// Fetch releases from network with request coalescing.
    ///
    /// If a fetch is already in progress for this repo, wait for and return
    /// the same result instead of making a duplicate request.
    async fn fetch_releases_from_network(&self, repo: &str) -> Result<Vec<GitHubRelease>> {
        self.coalesced_fetch(repo, || self.do_fetch_releases(repo))
            .await
    }

    async fn coalesced_fetch<F, Fut>(&self, repo: &str, fetch: F) -> Result<Vec<GitHubRelease>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<GitHubRelease>>>,
    {
        let cache_key = repo.to_string();
        let tx = loop {
            // Elect the owner while holding the same map lock used for lookup.
            let election = {
                let mut pending = self.pending_fetches.lock().await;
                if pending
                    .get(&cache_key)
                    .is_some_and(|receiver| receiver.has_changed().is_err())
                {
                    pending.remove(&cache_key);
                }
                if let Some(receiver) = pending.get(&cache_key) {
                    Err(receiver.clone())
                } else {
                    let (tx, rx) = watch::channel(None);
                    pending.insert(cache_key.clone(), rx);
                    Ok(tx)
                }
            };
            match election {
                Ok(tx) => break tx,
                Err(mut rx) => {
                    debug!(
                        "Coalescing request for {} - waiting for in-flight fetch",
                        repo
                    );
                    #[cfg(test)]
                    self.follower_joined.notify_one();
                    loop {
                        if let Some(result) = rx.borrow().as_ref() {
                            return result.clone().map_err(FetchFailure::into_error);
                        }
                        if rx.changed().await.is_err() {
                            // The elected owner was cancelled. Retry election.
                            break;
                        }
                    }
                }
            }
        };

        // Acquire fetch lock and do the actual fetch
        let _lock = self.fetch_lock.write().await;
        self.is_fetching.store(true, Ordering::SeqCst);
        let _fetching_reset = FetchingReset(&self.is_fetching);

        let result = fetch().await;

        // Convert result to FetchResult and broadcast
        let fetch_result: FetchResult = result
            .as_ref()
            .map(|r| r.clone())
            .map_err(FetchFailure::from_error);

        // Broadcast the result to any waiting callers
        let _ = tx.send(Some(fetch_result));

        // Clean up the pending entry
        {
            let mut pending = self.pending_fetches.lock().await;
            pending.remove(&cache_key);
        }

        result
    }

    async fn do_fetch_releases(&self, repo: &str) -> Result<Vec<GitHubRelease>> {
        let all_releases = collect_release_pages(
            NetworkConfig::GITHUB_RELEASES_PER_PAGE as usize,
            page_policy(repo),
            |page| self.fetch_release_page(repo, page),
        )
        .await?;
        info!(
            "Fetched {} releases from GitHub for {}",
            all_releases.len(),
            repo
        );
        Ok(all_releases)
    }

    async fn fetch_release_page(&self, repo: &str, page: u32) -> Result<Vec<GitHubRelease>> {
        let per_page = NetworkConfig::GITHUB_RELEASES_PER_PAGE;
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
                    let headers = vec![(
                        "Accept".to_string(),
                        "application/vnd.github.v3+json".to_string(),
                    )];
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

        if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
            // Rate limited - extract retry information from headers
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                // Also check X-RateLimit-Reset as fallback
                .or_else(|| {
                    response
                        .headers()
                        .get("X-RateLimit-Reset")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .and_then(|reset| {
                            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
                            Some(reset.saturating_sub(now))
                        })
                });

            warn!(
                "GitHub rate limited ({}), retry after: {:?} seconds",
                status, retry_after
            );

            return Err(PumasError::RateLimited {
                service: "GitHub".to_string(),
                retry_after_secs: retry_after,
            });
        }

        if !status.is_success() {
            return Err(PumasError::GitHubApi {
                message: format!("GitHub API returned {}", status),
                status_code: Some(status.as_u16()),
            });
        }

        response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse GitHub releases: {}", e),
            source: None,
        })
    }
}

// === WebSource trait implementations ===

impl WebSourceId for GitHubClient {
    fn id(&self) -> &'static str {
        "github"
    }

    fn domains(&self) -> &[&'static str] {
        &["api.github.com"]
    }
}

impl CacheStrategy for GitHubClient {
    fn default_ttl(&self) -> Duration {
        NetworkConfig::GITHUB_RELEASES_TTL
    }

    fn allow_stale_on_offline(&self) -> bool {
        true
    }

    fn max_stale_age(&self) -> Option<Duration> {
        // Allow stale data up to 7 days old
        Some(Duration::from_secs(7 * 24 * 60 * 60))
    }
}

#[async_trait]
impl WebSource for GitHubClient {
    fn has_cache(&self, key: &str) -> bool {
        self.cache.get_memory(key).is_some() || self.cache.get_disk(key).is_some()
    }

    fn is_cache_fresh(&self, key: &str) -> bool {
        // Memory cache is always fresh (managed by TTL internally)
        if self.cache.get_memory(key).is_some() {
            return true;
        }

        // Check disk cache validity
        self.cache
            .get_disk_entry(key)
            .map(|entry| {
                self.cache.is_disk_cache_valid(&entry.snapshot)
                    && !may_be_legacy_truncated(key, &entry)
            })
            .unwrap_or(false)
    }

    async fn on_network_restored(&self) {
        debug!("GitHub source: network restored, background refresh may occur");
        // The existing caching logic will handle refresh on next request
    }

    fn on_circuit_open(&self, domain: &str) {
        warn!("GitHub source: circuit breaker opened for {}", domain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tempfile::TempDir;

    #[tokio::test]
    async fn coalesced_callers_share_one_owner_and_rate_limit_details() {
        let (client, _root) = create_test_client();
        let client = Arc::new(client);
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let start_barrier = Arc::new(tokio::sync::Barrier::new(3));
        let mut callers = Vec::new();
        for _ in 0..2 {
            let client = client.clone();
            let started = started.clone();
            let release = release.clone();
            let calls = calls.clone();
            let start_barrier = start_barrier.clone();
            callers.push(tokio::spawn(async move {
                start_barrier.wait().await;
                client
                    .coalesced_fetch("pytorch/pytorch", move || async move {
                        calls.fetch_add(1, AtomicOrdering::SeqCst);
                        started.notify_one();
                        release.notified().await;
                        Err(PumasError::RateLimited {
                            service: "GitHub".into(),
                            retry_after_secs: Some(37),
                        })
                    })
                    .await
            }));
        }
        start_barrier.wait().await;
        tokio::time::timeout(Duration::from_secs(2), started.notified())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), client.follower_joined.notified())
            .await
            .unwrap();
        release.notify_one();

        for caller in callers {
            let result = caller.await.unwrap();
            assert!(matches!(
                result,
                Err(PumasError::RateLimited {
                    service,
                    retry_after_secs: Some(37),
                }) if service == "GitHub"
            ));
        }
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelled_fetch_owner_allows_waiter_to_take_over() {
        let (client, _root) = create_test_client();
        let client = Arc::new(client);
        let started = Arc::new(Notify::new());
        let owner = {
            let client = client.clone();
            let started = started.clone();
            tokio::spawn(async move {
                client
                    .coalesced_fetch("pytorch/pytorch", move || async move {
                        started.notify_one();
                        std::future::pending::<Result<Vec<GitHubRelease>>>().await
                    })
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(2), started.notified())
            .await
            .unwrap();
        let waiter = {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .coalesced_fetch("pytorch/pytorch", || async {
                        Ok(vec![github_release(vec![])])
                    })
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(2), client.follower_joined.notified())
            .await
            .unwrap();
        owner.abort();
        let releases = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(releases.len(), 1);
        assert!(!client.is_fetching.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn closed_completed_pending_channel_does_not_satisfy_new_fetch() {
        let (client, _root) = create_test_client();
        let mut old_release = github_release(vec![]);
        old_release.tag_name = "old".into();
        for stale in [
            Ok(vec![old_release]),
            Err(FetchFailure::RateLimited {
                service: "GitHub".into(),
                retry_after_secs: Some(99),
            }),
        ] {
            let (tx, rx) = watch::channel(Some(stale));
            let prior_waiter = rx.clone();
            client
                .pending_fetches
                .lock()
                .await
                .insert("pytorch/pytorch".into(), rx);
            drop(tx);

            let calls = Arc::new(AtomicUsize::new(0));
            let calls_for_fetch = calls.clone();
            let releases = tokio::time::timeout(
                Duration::from_secs(2),
                client.coalesced_fetch("pytorch/pytorch", move || async move {
                    calls_for_fetch.fetch_add(1, AtomicOrdering::SeqCst);
                    let mut release = github_release(vec![]);
                    release.tag_name = "new".into();
                    Ok(vec![release])
                }),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
            assert_eq!(releases[0].tag_name, "new");
            assert!(prior_waiter.borrow().is_some());
        }
    }

    #[test]
    fn coalesced_github_api_status_survives_projection() {
        let error = PumasError::GitHubApi {
            message: "GitHub API returned 503".into(),
            status_code: Some(503),
        };
        assert!(matches!(
            FetchFailure::from_error(&error).into_error(),
            PumasError::GitHubApi {
                status_code: Some(503),
                ..
            }
        ));
    }

    #[tokio::test]
    async fn release_pagination_continues_past_page_ten_and_stops_on_short_page() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let releases =
            collect_release_pages(100, page_policy(AppId::Torch.github_repo()), |page| {
                let seen = seen.clone();
                async move {
                    seen.lock().unwrap().push(page);
                    let count = if page <= 11 { 100 } else { 1 };
                    Ok((0..count)
                        .map(|index| {
                            let mut release = github_release(vec![]);
                            release.tag_name = format!("v{page}.{index}.0");
                            release
                        })
                        .collect())
                }
            })
            .await
            .unwrap();

        assert_eq!(releases.len(), 1101);
        assert_eq!(releases.last().unwrap().tag_name, "v12.0.0");
        assert_eq!(*seen.lock().unwrap(), (1..=12).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn release_pagination_returns_error_instead_of_partial_listing() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let result = collect_release_pages(1, page_policy(AppId::Torch.github_repo()), |page| {
            let seen = seen.clone();
            async move {
                seen.lock().unwrap().push(page);
                if page == 11 {
                    return Err(PumasError::RateLimited {
                        service: "GitHub".into(),
                        retry_after_secs: Some(60),
                    });
                }
                Ok(vec![github_release(vec![])])
            }
        })
        .await;

        assert!(matches!(result, Err(PumasError::RateLimited { .. })));
        assert_eq!(*seen.lock().unwrap(), (1..=11).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn non_torch_listing_keeps_first_page_budget() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let releases = collect_release_pages(1, page_policy("ggml-org/llama.cpp"), |page| {
            let seen = seen.clone();
            async move {
                seen.lock().unwrap().push(page);
                Ok(vec![github_release(vec![])])
            }
        })
        .await
        .unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(*seen.lock().unwrap(), vec![1]);
    }

    #[tokio::test]
    async fn torch_page_budget_and_deadline_fail_without_partial_listing() {
        let budget = collect_release_pages(1, page_policy(AppId::Torch.github_repo()), |_| async {
            Ok(vec![github_release(vec![])])
        })
        .await;
        let error = budget.unwrap_err();
        assert!(matches!(&error, PumasError::GitHubApi { .. }));
        assert!(error.to_string().contains("page budget"));

        let policy = ReleasePagePolicy {
            max_pages: 20,
            require_complete: true,
            deadline: Some(Duration::from_millis(1)),
        };
        let timeout = collect_release_pages(1, policy, |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(vec![github_release(vec![])])
        })
        .await;
        let error = timeout.unwrap_err();
        assert!(matches!(&error, PumasError::GitHubApi { .. }));
        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn marked_legacy_cap_sizes_are_reused_offline() {
        for size in [100, 1000] {
            let (client, _root) = create_test_client();
            let repo = AppId::Torch.github_repo();
            let releases = vec![github_release(vec![]); size];
            client.cache.set_disk(repo, &releases).unwrap();
            let entry = client.cache.get_disk_entry(repo).unwrap();
            assert!(entry.listing_complete);
            assert!(!may_be_legacy_truncated(repo, &entry));
            let cached = client.get_releases(repo, false).await.unwrap();
            assert_eq!(cached.len(), size);
            assert!(client.cache.get_memory(repo).is_some());
        }
    }

    #[tokio::test]
    async fn old_unmarked_legacy_cap_sizes_require_refresh() {
        for size in [100, 1000] {
            let (client, _root) = create_test_client();
            let repo = AppId::Torch.github_repo();
            let snapshot = GitHubReleasesCache {
                last_fetched: Utc::now().to_rfc3339(),
                ttl: 3600,
                releases: vec![github_release(vec![]); size],
            };
            let path = client.cache.disk_cache_path(repo);
            std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
            let entry = client.cache.get_disk_entry(repo).unwrap();
            assert!(!entry.listing_complete);
            assert!(may_be_legacy_truncated(repo, &entry));
            assert!(!client.get_cache_status(repo).await.is_valid);
        }
    }

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

    #[tokio::test]
    async fn test_cache_status() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ReleasesCache::new(temp_dir.path().to_path_buf(), Duration::from_secs(3600));

        // No cache
        let status = cache.get_status_async("test/repo", false).await;
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

        let status = cache.get_status_async("test/repo", false).await;
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

    #[test]
    fn llama_cpp_release_expansion_exposes_cpu_and_gpu_runtime_variants() {
        let release = github_release(vec![
            github_asset("llama-b9082-bin-ubuntu-x64.tar.gz", 1),
            github_asset("llama-b9082-bin-ubuntu-vulkan-x64.tar.gz", 2),
            github_asset("llama-b9082-bin-ubuntu-rocm-7.2-x64.tar.gz", 3),
            github_asset("llama-b9082-bin-ubuntu-sycl-fp32-x64.tar.gz", 4),
            github_asset("llama-b9082-bin-ubuntu-sycl-fp16-x64.tar.gz", 5),
        ]);

        let variants =
            GitHubClient::expand_llama_cpp_release_variants_for_platform(release, "linux", "x64");
        let tags = variants
            .iter()
            .map(|release| release.tag_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            tags,
            vec![
                "b9082+cpu",
                "b9082+vulkan",
                "b9082+rocm",
                "b9082+sycl-fp32",
                "b9082+sycl-fp16",
            ]
        );
        assert_eq!(variants[1].archive_size, Some(2));
        assert_eq!(
            variants[1].assets[0].name,
            "llama-b9082-bin-ubuntu-vulkan-x64.tar.gz"
        );
    }

    #[test]
    fn llama_cpp_release_expansion_ignores_other_platform_assets() {
        let release = github_release(vec![
            github_asset("llama-b9082-bin-ubuntu-x64.tar.gz", 1),
            github_asset("llama-b9082-bin-win-vulkan-x64.zip", 2),
        ]);

        let variants =
            GitHubClient::expand_llama_cpp_release_variants_for_platform(release, "linux", "x64");

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].tag_name, "b9082+cpu");
    }

    fn github_asset(name: &str, size: u64) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            size,
            download_url: format!("https://example.invalid/{name}"),
            content_type: None,
        }
    }

    fn github_release(assets: Vec<GitHubAsset>) -> GitHubRelease {
        GitHubRelease {
            tag_name: "b9082".to_string(),
            name: "b9082".to_string(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets,
            html_url: "https://github.com/ggml-org/llama.cpp/releases/tag/b9082".to_string(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let (client, _temp) = create_test_client();
        let status = client.get_cache_status("test/repo").await;
        assert!(!status.has_cache);
        assert!(!status.is_fetching);
    }
}
