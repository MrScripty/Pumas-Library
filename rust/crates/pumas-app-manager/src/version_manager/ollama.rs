//! Ollama-specific version management.
//!
//! Handles Ollama binary downloads and installation - simpler than ComfyUI
//! since Ollama is a pre-built binary with no Python dependencies.

use pumas_library::config::{AppId, InstallationConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::InstallationStage;
use pumas_library::network::{GitHubAsset, GitHubClient, GitHubRelease};
use crate::version_manager::progress::ProgressUpdate;
use crate::version_manager::state::VersionState;
use pumas_library::{PumasError, Result};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

/// Ollama version manager specialized for binary-only installation.
pub struct OllamaVersionManager {
    /// Root directory for launcher data.
    launcher_root: PathBuf,
    /// App ID (always Ollama).
    app_id: AppId,
    /// GitHub repository for Ollama.
    github_repo: String,
    /// GitHub client for fetching releases.
    github_client: Arc<GitHubClient>,
    /// Version state tracking.
    state: Arc<RwLock<VersionState>>,
    /// Cancellation flag.
    cancel_flag: Arc<AtomicBool>,
    /// Installation lock.
    install_lock: Arc<Mutex<()>>,
    /// Currently installing tag.
    installing_tag: Arc<RwLock<Option<String>>>,
}

impl OllamaVersionManager {
    /// Create a new Ollama version manager.
    pub async fn new(
        launcher_root: PathBuf,
        metadata_manager: Arc<MetadataManager>,
        github_client: Arc<GitHubClient>,
    ) -> Result<Self> {
        let app_id = AppId::Ollama;
        let state = VersionState::new(
            &launcher_root,
            app_id,
            metadata_manager.clone(),
        )
        .await?;

        Ok(Self {
            launcher_root,
            app_id,
            github_repo: "ollama/ollama".to_string(),
            github_client,
            state: Arc::new(RwLock::new(state)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            install_lock: Arc::new(Mutex::new(())),
            installing_tag: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the versions directory.
    fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(self.app_id.versions_dir_name())
    }

    /// Get the version directory for a specific tag.
    fn version_path(&self, tag: &str) -> PathBuf {
        self.versions_dir().join(tag)
    }

    /// Get the binary name for current platform.
    fn binary_name() -> &'static str {
        if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        }
    }

    /// Check if a version is complete (has binary).
    pub fn is_version_complete(&self, tag: &str) -> bool {
        let version_path = self.version_path(tag);
        if !version_path.exists() {
            return false;
        }
        let binary_path = version_path.join(Self::binary_name());
        binary_path.exists()
    }

    /// Select the best asset for the current platform.
    fn select_asset(release: &GitHubRelease) -> Option<&GitHubAsset> {
        let system = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        // Map architecture names
        let desired_arch = match arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => arch,
        };

        // Map OS names
        let desired_os = if system.starts_with("win") {
            "windows"
        } else {
            system
        };

        debug!(
            "Selecting Ollama asset for {}-{}",
            desired_os, desired_arch
        );

        // Score each asset and find the best match
        let mut best_asset: Option<&GitHubAsset> = None;
        let mut best_score = 0;

        for asset in &release.assets {
            let name_lower = asset.name.to_lowercase();
            let mut score = 0;

            // OS match
            if name_lower.contains(desired_os) {
                score += 2;
            }

            // Architecture match
            if name_lower.contains(desired_arch) {
                score += 2;
            }

            // Prefer certain formats
            if cfg!(windows) && name_lower.ends_with(".exe") {
                score += 1;
            } else if name_lower.ends_with(".tar.gz")
                || name_lower.ends_with(".tgz")
                || name_lower.ends_with(".zip")
                || name_lower.ends_with(".tar.zst")
            {
                score += 1;
            }

            // Skip source archives
            if name_lower.contains("source") || name_lower.contains("src") {
                continue;
            }

            if score > best_score {
                best_score = score;
                best_asset = Some(asset);
            }
        }

        if let Some(asset) = best_asset {
            info!(
                "Selected Ollama asset: {} (score: {})",
                asset.name, best_score
            );
        } else {
            warn!("No suitable Ollama asset found for {}-{}", desired_os, desired_arch);
        }

        best_asset
    }

    /// Install an Ollama version.
    pub async fn install_version(
        &self,
        tag: &str,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        // Acquire installation lock
        let _lock = self.install_lock.lock().await;

        // Set installing tag
        {
            let mut installing = self.installing_tag.write().await;
            *installing = Some(tag.to_string());
        }

        // Reset cancel flag
        self.cancel_flag.store(false, Ordering::SeqCst);

        let result = self
            .install_version_internal(tag, progress_tx.clone())
            .await;

        // Clear installing tag
        {
            let mut installing = self.installing_tag.write().await;
            *installing = None;
        }

        // Send completion status
        if let Some(tx) = progress_tx {
            let _ = tx
                .send(ProgressUpdate::Completed {
                    success: result.is_ok(),
                })
                .await;
        }

        result
    }

    async fn install_version_internal(
        &self,
        tag: &str,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        info!("Installing Ollama version {}", tag);

        // Create version directory
        let version_path = self.version_path(tag);
        std::fs::create_dir_all(&version_path).map_err(|e| PumasError::Io {
            message: format!("Failed to create version directory: {}", e),
            path: Some(version_path.clone()),
            source: Some(e),
        })?;

        // Send stage update
        if let Some(ref tx) = progress_tx {
            let _ = tx
                .send(ProgressUpdate::StageChanged {
                    stage: InstallationStage::Download,
                    message: format!("Fetching release {}", tag),
                })
                .await;
        }

        // Fetch releases and find the matching one
        let releases = self
            .github_client
            .get_releases(&self.github_repo, false)
            .await?;

        let release = releases
            .iter()
            .find(|r| r.tag_name == tag)
            .ok_or_else(|| PumasError::VersionNotFound { tag: tag.to_string() })?;

        // Select appropriate asset
        let asset = Self::select_asset(release).ok_or_else(|| PumasError::InstallationFailed {
            message: "No suitable Ollama binary found for this platform".to_string(),
        })?;

        // Check for cancellation
        self.check_cancelled()?;

        // Download the asset
        let download_url = &asset.download_url;
        let archive_path = version_path.join(&asset.name);

        if let Some(ref tx) = progress_tx {
            let _ = tx
                .send(ProgressUpdate::StageChanged {
                    stage: InstallationStage::Download,
                    message: format!("Downloading {}", asset.name),
                })
                .await;
        }

        self.download_file(download_url, &archive_path, progress_tx.clone())
            .await?;

        // Check for cancellation
        self.check_cancelled()?;

        // Extract and set up
        if let Some(ref tx) = progress_tx {
            let _ = tx
                .send(ProgressUpdate::StageChanged {
                    stage: InstallationStage::Extract,
                    message: "Extracting binary".to_string(),
                })
                .await;
        }

        self.extract_binary(&archive_path, &version_path).await?;

        // Clean up archive
        if archive_path.exists() {
            std::fs::remove_file(&archive_path).ok();
        }

        // Record in metadata
        if let Some(ref tx) = progress_tx {
            let _ = tx
                .send(ProgressUpdate::StageChanged {
                    stage: InstallationStage::Setup,
                    message: "Recording installation".to_string(),
                })
                .await;
        }

        // Create metadata and update state
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: chrono::Utc::now().to_rfc3339(),
            python_version: None, // No Python for Ollama
            release_tag: tag.to_string(),
            git_commit: None,
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url: Some(download_url.clone()),
            size: Some(asset.size),
            requirements_hash: None,
            dependencies_installed: Some(true), // No dependencies for Ollama
        };

        {
            let mut state = self.state.write().await;
            state.add_installed_version(tag, metadata)?;
        }

        info!("Ollama {} installed successfully", tag);
        Ok(())
    }

    /// Download a file with progress reporting.
    async fn download_file(
        &self,
        url: &str,
        dest: &PathBuf,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        use std::io::Write;

        let client = reqwest::Client::builder()
            .timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })?;

        let response = client.get(url).send().await.map_err(|e| PumasError::Network {
            message: format!("Download failed: {}", e),
            cause: Some(e.to_string()),
        })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!("Download failed with status: {}", response.status()),
                cause: None,
            });
        }

        let total_size = response.content_length();
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut file = std::fs::File::create(dest).map_err(|e| PumasError::Io {
            message: format!("Failed to create file: {}", e),
            path: Some(dest.clone()),
            source: Some(e),
        })?;

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            self.check_cancelled()?;

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Error reading download: {}", e),
                cause: Some(e.to_string()),
            })?;

            file.write_all(&chunk).map_err(|e| PumasError::Io {
                message: format!("Failed to write: {}", e),
                path: Some(dest.clone()),
                source: Some(e),
            })?;

            downloaded += chunk.len() as u64;

            if let Some(ref tx) = progress_tx {
                let _ = tx
                    .send(ProgressUpdate::Download {
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: None,
                    })
                    .await;
            }
        }

        Ok(())
    }

    /// Extract the binary from archive.
    async fn extract_binary(&self, archive_path: &PathBuf, version_path: &PathBuf) -> Result<()> {
        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Determine archive type
        if archive_name.ends_with(".zip") {
            self.extract_zip(archive_path, version_path)?;
        } else if archive_name.ends_with(".tar.gz")
            || archive_name.ends_with(".tgz")
            || archive_name.ends_with(".tar.zst")
        {
            self.extract_tarball(archive_path, version_path)?;
        } else if archive_name.ends_with(".exe") || archive_name == "ollama" {
            // Direct binary, just move it
            let dest = version_path.join(Self::binary_name());
            std::fs::rename(archive_path, &dest).map_err(|e| PumasError::Io {
                message: format!("Failed to move binary: {}", e),
                path: Some(dest),
                source: Some(e),
            })?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: format!("Unknown archive format: {}", archive_name),
            });
        }

        // Find and finalize binary
        self.finalize_binary(version_path)?;

        Ok(())
    }

    fn extract_zip(&self, archive_path: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
        use std::io::Read;
        let file = std::fs::File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        let mut archive = zip::ZipArchive::new(file).map_err(|e| PumasError::InstallationFailed {
            message: format!("Failed to read zip: {}", e),
        })?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to read zip entry: {}", e),
            })?;

            let outpath = dest_dir.join(file.name());

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath).ok();
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| PumasError::Io {
                    message: format!("Failed to create file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).map_err(|e| PumasError::Io {
                    message: format!("Failed to read from archive: {}", e),
                    path: None,
                    source: Some(e),
                })?;
                std::io::Write::write_all(&mut outfile, &contents).map_err(|e| PumasError::Io {
                    message: format!("Failed to write file: {}", e),
                    path: Some(outpath),
                    source: Some(e),
                })?;
            }
        }

        Ok(())
    }

    fn extract_tarball(&self, archive_path: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
        use std::io::BufReader;

        let file = std::fs::File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Handle different compression formats
        if archive_name.ends_with(".tar.zst") {
            // zstd compression
            let decoder = zstd::stream::Decoder::new(BufReader::new(file))
                .map_err(|e| PumasError::InstallationFailed {
                    message: format!("Failed to create zstd decoder: {}", e),
                })?;
            let mut archive = tar::Archive::new(decoder);
            archive.unpack(dest_dir).map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to extract tar.zst: {}", e),
            })?;
        } else {
            // gzip compression
            let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
            let mut archive = tar::Archive::new(decoder);
            archive.unpack(dest_dir).map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to extract tarball: {}", e),
            })?;
        }

        Ok(())
    }

    /// Find and set up the binary in the version directory.
    fn finalize_binary(&self, version_path: &PathBuf) -> Result<()> {
        let binary_name = Self::binary_name();
        let final_path = version_path.join(binary_name);

        // If binary already in place, just make it executable
        if final_path.exists() {
            #[cfg(unix)]
            {
                let mut perms = std::fs::metadata(&final_path)
                    .map_err(|e| PumasError::Io {
                        message: format!("Failed to get permissions: {}", e),
                        path: Some(final_path.clone()),
                        source: Some(e),
                    })?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&final_path, perms).map_err(|e| PumasError::Io {
                    message: format!("Failed to set permissions: {}", e),
                    path: Some(final_path),
                    source: Some(e),
                })?;
            }
            return Ok(());
        }

        // Search for binary in extracted directories
        let binary = self.find_binary_recursive(version_path)?;
        if let Some(found) = binary {
            std::fs::rename(&found, &final_path).map_err(|e| PumasError::Io {
                message: format!("Failed to move binary: {}", e),
                path: Some(final_path.clone()),
                source: Some(e),
            })?;

            #[cfg(unix)]
            {
                let mut perms = std::fs::metadata(&final_path)
                    .map_err(|e| PumasError::Io {
                        message: format!("Failed to get permissions: {}", e),
                        path: Some(final_path.clone()),
                        source: Some(e),
                    })?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&final_path, perms).map_err(|e| PumasError::Io {
                    message: format!("Failed to set permissions: {}", e),
                    path: Some(final_path),
                    source: Some(e),
                })?;
            }

            // Clean up extracted directories
            self.cleanup_extracted_dirs(version_path)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: "Could not find Ollama binary in archive".to_string(),
            });
        }

        Ok(())
    }

    fn find_binary_recursive(&self, dir: &PathBuf) -> Result<Option<PathBuf>> {
        let binary_name = Self::binary_name();

        for entry in std::fs::read_dir(dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", e),
            path: Some(dir.clone()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read entry: {}", e),
                path: Some(dir.clone()),
                source: Some(e),
            })?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == binary_name || name == "ollama" || name == "ollama.exe" {
                        return Ok(Some(path));
                    }
                }
            } else if path.is_dir() {
                if let Some(found) = self.find_binary_recursive(&path)? {
                    return Ok(Some(found));
                }
            }
        }

        Ok(None)
    }

    fn cleanup_extracted_dirs(&self, version_path: &PathBuf) -> Result<()> {
        let binary_name = Self::binary_name();

        for entry in std::fs::read_dir(version_path).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", e),
            path: Some(version_path.clone()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read entry: {}", e),
                path: Some(version_path.clone()),
                source: Some(e),
            })?;
            let path = entry.path();

            // Skip the binary itself
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == binary_name {
                        continue;
                    }
                }
            }

            // Remove directories
            if path.is_dir() {
                std::fs::remove_dir_all(&path).ok();
            }
        }

        Ok(())
    }

    /// Cancel ongoing installation.
    pub fn cancel_installation(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Err(PumasError::InstallationCancelled);
        }
        Ok(())
    }

    /// Get installed versions.
    pub async fn get_installed_versions(&self) -> Vec<String> {
        let state = self.state.read().await;
        state.get_installed_tags()
    }

    /// Get active version.
    pub async fn get_active_version(&self) -> Option<String> {
        let state = self.state.read().await;
        state.get_active_version()
    }

    /// Set active version.
    pub async fn set_active_version(&self, tag: &str) -> Result<()> {
        let mut state = self.state.write().await;
        state.set_active_version(tag)?;
        Ok(())
    }

    /// Get default version.
    pub async fn get_default_version(&self) -> Option<String> {
        let state = self.state.read().await;
        state.get_default_version()
    }

    /// Set default version.
    pub async fn set_default_version(&self, tag: Option<&str>) -> Result<()> {
        let mut state = self.state.write().await;
        state.set_default_version(tag)?;
        Ok(())
    }

    /// Uninstall a version.
    pub async fn uninstall_version(&self, tag: &str) -> Result<()> {
        let version_path = self.version_path(tag);

        if version_path.exists() {
            std::fs::remove_dir_all(&version_path).map_err(|e| PumasError::Io {
                message: format!("Failed to remove version directory: {}", e),
                path: Some(version_path),
                source: Some(e),
            })?;
        }

        // Update state (this also removes from metadata)
        {
            let mut state = self.state.write().await;
            state.remove_installed_version(tag)?;
        }

        info!("Ollama {} uninstalled", tag);
        Ok(())
    }

    /// Get the binary path for a version.
    pub fn get_binary_path(&self, tag: &str) -> PathBuf {
        self.version_path(tag).join(Self::binary_name())
    }

    /// Get available releases from GitHub.
    pub async fn get_available_releases(&self, force_refresh: bool) -> Result<Vec<GitHubRelease>> {
        self.github_client.get_releases(&self.github_repo, force_refresh).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_name() {
        let name = OllamaVersionManager::binary_name();
        #[cfg(windows)]
        assert_eq!(name, "ollama.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "ollama");
    }
}
