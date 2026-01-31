//! Version installation with progress reporting.
//!
//! Handles downloading, extracting, and setting up new versions.

use pumas_library::config::{AppId, InstallationConfig, PathsConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::InstallationStage;
use pumas_library::network::{GitHubAsset, GitHubRelease};
use crate::version_manager::progress::{InstallationProgressTracker, ProgressUpdate};
use pumas_library::{PumasError, Result};
use chrono::Utc;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Handles version installation.
pub struct VersionInstaller {
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Metadata manager.
    metadata_manager: Arc<MetadataManager>,
    /// Progress tracker.
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    /// Cancellation flag.
    cancel_flag: Arc<AtomicBool>,
}

impl VersionInstaller {
    /// Create a new version installer.
    pub fn new(
        launcher_root: PathBuf,
        app_id: AppId,
        metadata_manager: Arc<MetadataManager>,
        progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            launcher_root,
            app_id,
            metadata_manager,
            progress_tracker,
            cancel_flag,
        }
    }

    /// Install a version from a GitHub release.
    /// Dispatches to app-specific installation method based on app_id.
    pub async fn install_version(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        match self.app_id {
            AppId::Ollama => self.install_ollama_binary(tag, release, progress_tx).await,
            AppId::ComfyUI | _ => self.install_python_app(tag, release, progress_tx).await,
        }
    }

    /// Install a Python-based app (ComfyUI) from source with venv and dependencies.
    async fn install_python_app(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Starting installation of version {}", tag);

        // Create log file
        let log_dir = self.logs_dir();
        std::fs::create_dir_all(&log_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create logs directory: {}", e),
            path: Some(log_dir.clone()),
            source: Some(e),
        })?;
        let log_path = log_dir.join(format!(
            "install-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        // Initialize progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                release.total_size,
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

        // Determine download URL
        let download_url = release
            .zipball_url
            .as_ref()
            .or(release.tarball_url.as_ref())
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "No download URL available for release".to_string(),
            })?;

        let is_tarball = release.tarball_url.is_some() && release.zipball_url.is_none();

        // Create temp directory
        let temp_dir = self.launcher_root.join("temp");
        std::fs::create_dir_all(&temp_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create temp directory: {}", e),
            path: Some(temp_dir.clone()),
            source: Some(e),
        })?;

        let archive_ext = if is_tarball { "tar.gz" } else { "zip" };
        let archive_path = temp_dir.join(format!("{}.{}", tag, archive_ext));
        let extract_dir = temp_dir.join(format!("extract-{}", tag));

        // Clean up any previous failed attempts
        if archive_path.exists() {
            let _ = std::fs::remove_file(&archive_path);
        }
        if extract_dir.exists() {
            let _ = std::fs::remove_dir_all(&extract_dir);
        }

        // Execute installation steps
        let result = self
            .do_install(
                tag,
                release,
                download_url,
                is_tarball,
                &archive_path,
                &extract_dir,
                &progress_tx,
            )
            .await;

        // Cleanup temp files
        let _ = std::fs::remove_file(&archive_path);
        let _ = std::fs::remove_dir_all(&extract_dir);

        // Update progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.complete_installation(result.is_ok());
        }

        result
    }

    /// Install Ollama binary from pre-built release assets.
    /// Unlike Python apps, Ollama is distributed as a pre-compiled binary.
    async fn install_ollama_binary(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Starting Ollama binary installation for {}", tag);

        // Select platform-appropriate asset (e.g., ollama-linux-amd64.tgz)
        let asset = self.select_ollama_asset(&release.assets)?;
        let download_url = &asset.download_url;
        let total_size = asset.size;
        let asset_name = asset.name.clone();

        info!(
            "Selected Ollama asset: {} ({} bytes)",
            asset_name, total_size
        );

        // Create log file
        let log_dir = self.logs_dir();
        std::fs::create_dir_all(&log_dir).ok();
        let log_path = log_dir.join(format!(
            "install-ollama-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        // Start progress tracking with actual asset size from GitHub
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                Some(total_size),
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

        // Use download cache directory to avoid re-downloading on reinstalls
        let cache_downloads = self.launcher_root.join("launcher-data").join("cache").join("downloads");
        std::fs::create_dir_all(&cache_downloads).map_err(|e| PumasError::Io {
            message: format!("Failed to create download cache directory: {}", e),
            path: Some(cache_downloads.clone()),
            source: Some(e),
        })?;

        let archive_path = cache_downloads.join(&asset_name);

        // Check if we have a valid cached download
        let cache_valid = if archive_path.exists() {
            match std::fs::metadata(&archive_path) {
                Ok(meta) if meta.len() == total_size => {
                    info!("Using cached download: {} ({} bytes)", asset_name, total_size);
                    true
                }
                Ok(meta) => {
                    info!("Cached download size mismatch ({} != {}), re-downloading", meta.len(), total_size);
                    let _ = std::fs::remove_file(&archive_path);
                    false
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&archive_path);
                    false
                }
            }
        } else {
            false
        };

        // Download binary asset (skip if cache is valid)
        let result = self
            .do_ollama_install(
                tag,
                release,
                download_url,
                total_size,
                &asset_name,
                &archive_path,
                cache_valid,
                &progress_tx,
            )
            .await;

        // Keep cached download on success, remove on failure
        if result.is_err() {
            let _ = std::fs::remove_file(&archive_path);
        }

        // Update progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.complete_installation(result.is_ok());
        }

        result
    }

    /// Execute Ollama installation steps.
    async fn do_ollama_install(
        &self,
        tag: &str,
        release: &GitHubRelease,
        download_url: &str,
        total_size: u64,
        asset_name: &str,
        archive_path: &PathBuf,
        cache_valid: bool,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        // Check cancellation
        self.check_cancelled()?;

        // Step 1: Download binary asset (skip if using cache)
        if cache_valid {
            // Update progress to show we're using cache
            {
                let mut tracker = self.progress_tracker.write().await;
                tracker.update_stage(InstallationStage::Download, 100.0, Some("Using cached download"));
            }
            let _ = progress_tx
                .send(ProgressUpdate::Download {
                    downloaded_bytes: total_size,
                    total_bytes: Some(total_size),
                    speed_bytes_per_sec: None,
                })
                .await;
        } else {
            self.download_archive(download_url, archive_path, progress_tx)
                .await?;
        }

        // Check cancellation
        self.check_cancelled()?;

        // Step 2: Create version directory
        let version_dir = self.versions_dir().join(tag);
        std::fs::create_dir_all(&version_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create version directory: {}", e),
            path: Some(version_dir.clone()),
            source: Some(e),
        })?;

        // Step 3: Extract binary from archive
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Extract, 0.0, Some("Extracting binary..."));
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Extract,
                message: "Extracting binary...".to_string(),
            })
            .await;

        self.extract_ollama_binary(archive_path, &version_dir, asset_name)?;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Extract, 100.0, Some("Extraction complete"));
        }

        // Check cancellation
        self.check_cancelled()?;

        // Step 4: Finalize (no venv, no deps - just mark as installed)
        self.finalize_ollama_installation(tag, release, &version_dir, progress_tx)
            .await?;

        info!("Ollama installation of {} completed successfully", tag);
        Ok(())
    }

    /// Select the appropriate Ollama binary asset for the current platform.
    /// Uses exact matching to avoid selecting variant builds (ROCm, Jetpack, etc.).
    fn select_ollama_asset<'a>(&self, assets: &'a [GitHubAsset]) -> Result<&'a GitHubAsset> {
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

        assets
            .iter()
            .find(|a| exact_patterns.iter().any(|p| a.name == *p))
            .ok_or_else(|| PumasError::InstallationFailed {
                message: format!(
                    "No Ollama binary found for {}-{}. Looking for: {:?}. Available assets: {:?}",
                    os,
                    arch,
                    exact_patterns,
                    assets.iter().map(|a| &a.name).collect::<Vec<_>>()
                ),
            })
    }

    /// Extract Ollama binary from archive format.
    /// Ollama releases are distributed as:
    /// - Linux: ollama-linux-amd64.tar.zst (Zstandard compressed tar, current format)
    /// - Linux (legacy): ollama-linux-amd64.tgz (gzip compressed tar)
    /// - macOS: ollama-darwin-arm64.tar.zst
    /// - Windows: ollama-windows-amd64.zip (containing ollama.exe)
    fn extract_ollama_binary(
        &self,
        archive_path: &PathBuf,
        version_dir: &PathBuf,
        asset_name: &str,
    ) -> Result<()> {
        info!("Extracting Ollama binary from {}", asset_name);

        if asset_name.ends_with(".tar.zst") {
            // Extract tar.zst (Zstandard compressed tar - current Ollama format)
            self.extract_tar_zst(archive_path, version_dir)?;
        } else if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            // Extract tar.gz (legacy format)
            self.extract_tarball(archive_path, version_dir)?;
        } else if asset_name.ends_with(".zip") {
            // Extract zip
            self.extract_zip(archive_path, version_dir)?;
        } else {
            // Raw binary (e.g., ollama-linux-amd64 without extension)
            let binary_name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
            let dest = version_dir.join(binary_name);
            std::fs::copy(archive_path, &dest).map_err(|e| PumasError::Io {
                message: format!("Failed to copy binary: {}", e),
                path: Some(dest.clone()),
                source: Some(e),
            })?;
        }

        // Find and make the binary executable on Unix
        self.finalize_ollama_binary(version_dir)?;

        info!("Ollama binary extraction complete");
        Ok(())
    }

    /// Extract a .tar.zst archive (Zstandard compressed tar).
    fn extract_tar_zst(&self, archive_path: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
        info!("Extracting tar.zst archive to {}", dest_dir.display());

        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        let decoder = zstd::Decoder::new(BufReader::new(file)).map_err(|e| PumasError::Io {
            message: format!("Failed to create zstd decoder: {}", e),
            path: Some(archive_path.clone()),
            source: Some(std::io::Error::other(e)),
        })?;

        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to extract tar.zst: {}", e),
            path: Some(dest_dir.clone()),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Find the ollama binary in the extracted directory and make it executable.
    fn finalize_ollama_binary(&self, version_dir: &PathBuf) -> Result<()> {
        // Ollama archives typically extract to bin/ollama or just ollama
        let possible_paths = [
            version_dir.join("bin").join("ollama"),
            version_dir.join("ollama"),
        ];

        let binary_path = possible_paths.iter().find(|p| p.exists());

        #[cfg(unix)]
        if let Some(binary) = binary_path {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(binary)
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to get binary metadata: {}", e),
                    path: Some(binary.clone()),
                    source: Some(e),
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(binary, perms).map_err(|e| PumasError::Io {
                message: format!("Failed to set binary permissions: {}", e),
                path: Some(binary.clone()),
                source: Some(e),
            })?;
            info!("Set executable permissions on {}", binary.display());
        }

        Ok(())
    }

    /// Finalize Ollama installation (create metadata, no Python/venv).
    async fn finalize_ollama_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        version_dir: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Finalizing Ollama installation for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Setup, 0.0, Some("Finalizing installation..."));
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            })
            .await;

        // Find the download URL for metadata
        let download_url = release.assets.iter()
            .find(|a| a.name.contains("linux") || a.name.contains("darwin") || a.name.contains("windows"))
            .map(|a| a.download_url.clone());

        // Create metadata entry (no Python version for Ollama)
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version: None, // Ollama is a Go binary, no Python
            git_commit: None,
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url,
            size: release.archive_size,
            requirements_hash: None,
            dependencies_installed: Some(true), // No dependencies needed
        };

        // Save metadata
        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Setup, 100.0, Some("Installation complete"));
        }
        let _ = progress_tx
            .send(ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            })
            .await;

        info!("Ollama installation of {} finalized", tag);
        Ok(())
    }

    async fn do_install(
        &self,
        tag: &str,
        release: &GitHubRelease,
        download_url: &str,
        is_tarball: bool,
        archive_path: &PathBuf,
        extract_dir: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        // Check cancellation
        self.check_cancelled()?;

        // Step 1: Download
        self.download_archive(download_url, archive_path, progress_tx)
            .await?;

        // Check cancellation
        self.check_cancelled()?;

        // Step 2: Extract
        self.extract_archive(archive_path, extract_dir, is_tarball, progress_tx)
            .await?;

        // Check cancellation
        self.check_cancelled()?;

        // Step 3: Move to final location
        let version_dir = self.versions_dir().join(tag);
        self.move_to_final_location(extract_dir, &version_dir)
            .await?;

        // Check cancellation
        self.check_cancelled()?;

        // Step 4: Create virtual environment
        self.create_venv(tag, &version_dir, progress_tx).await?;

        // Check cancellation
        self.check_cancelled()?;

        // Step 5: Install dependencies
        self.install_deps(tag, &version_dir, progress_tx).await?;

        // Check cancellation
        self.check_cancelled()?;

        // Step 6: Setup and finalize
        self.finalize_installation(tag, release, &version_dir, progress_tx)
            .await?;

        info!("Installation of {} completed successfully", tag);
        Ok(())
    }

    async fn download_archive(
        &self,
        url: &str,
        archive_path: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Downloading archive from {}", url);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Download, 0.0, Some("Starting download..."));
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Download,
                message: "Starting download...".to_string(),
            })
            .await;

        // Create HTTP client with appropriate timeouts for large downloads
        // - connect_timeout: time to establish connection (15s is fine)
        // - NO overall timeout: downloads can take a long time for large files (1.6 GB+)
        let client = reqwest::Client::builder()
            .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })?;

        // Start download with retry
        let mut response = None;
        for attempt in 1..=InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS {
            match client.get(url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        response = Some(resp);
                        break;
                    } else {
                        warn!(
                            "Download attempt {} failed with status {}",
                            attempt,
                            resp.status()
                        );
                    }
                }
                Err(e) => {
                    warn!("Download attempt {} failed: {}", attempt, e);
                    if attempt == InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS {
                        return Err(PumasError::Network {
                            message: format!("Download failed after {} attempts: {}", attempt, e),
                            cause: Some(e.to_string()),
                        });
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        }

        let response = response.ok_or_else(|| PumasError::Network {
            message: "Download failed - no successful response".to_string(),
            cause: None,
        })?;

        let total_size = response.content_length();

        // Create output file
        let mut file = File::create(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to create archive file: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        // Download with progress
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            // Check cancellation
            self.check_cancelled()?;

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Error reading download chunk: {}", e),
                cause: Some(e.to_string()),
            })?;

            file.write_all(&chunk).map_err(|e| PumasError::Io {
                message: format!("Failed to write to archive: {}", e),
                path: Some(archive_path.clone()),
                source: Some(e),
            })?;

            downloaded += chunk.len() as u64;

            // Calculate speed
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                Some(downloaded as f64 / elapsed)
            } else {
                None
            };

            // Update progress
            {
                let mut tracker = self.progress_tracker.write().await;
                tracker.update_download_progress(downloaded, total_size, speed);
            }

            let _ = progress_tx
                .send(ProgressUpdate::Download {
                    downloaded_bytes: downloaded,
                    total_bytes: total_size,
                    speed_bytes_per_sec: speed,
                })
                .await;
        }

        // Add to completed items
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.add_completed_item("archive", "archive", Some(downloaded));
        }

        info!("Download complete: {} bytes", downloaded);
        Ok(())
    }

    async fn extract_archive(
        &self,
        archive_path: &PathBuf,
        extract_dir: &PathBuf,
        is_tarball: bool,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Extracting archive to {}", extract_dir.display());

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Extract, 0.0, Some("Extracting archive..."));
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Extract,
                message: "Extracting archive...".to_string(),
            })
            .await;

        std::fs::create_dir_all(extract_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create extract directory: {}", e),
            path: Some(extract_dir.clone()),
            source: Some(e),
        })?;

        if is_tarball {
            self.extract_tarball(archive_path, extract_dir)?;
        } else {
            self.extract_zip(archive_path, extract_dir)?;
        }

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Extract, 100.0, Some("Extraction complete"));
        }
        let _ = progress_tx
            .send(ProgressUpdate::Extract {
                progress_percent: 100.0,
            })
            .await;

        info!("Extraction complete");
        Ok(())
    }

    fn extract_zip(&self, archive_path: &PathBuf, extract_dir: &PathBuf) -> Result<()> {
        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open zip archive: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        let mut archive = zip::ZipArchive::new(file).map_err(|e| PumasError::InstallationFailed {
            message: format!("Invalid zip archive: {}", e),
        })?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to read zip entry {}: {}", i, e),
            })?;

            let outpath = match file.enclosed_name() {
                Some(path) => extract_dir.join(path),
                None => continue,
            };

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
            } else {
                if let Some(parent) = outpath.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                            message: format!("Failed to create parent directory: {}", e),
                            path: Some(parent.to_path_buf()),
                            source: Some(e),
                        })?;
                    }
                }

                let mut outfile = File::create(&outpath).map_err(|e| PumasError::Io {
                    message: format!("Failed to create file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| PumasError::Io {
                    message: format!("Failed to extract file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
            }

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
                }
            }
        }

        Ok(())
    }

    fn extract_tarball(&self, archive_path: &PathBuf, extract_dir: &PathBuf) -> Result<()> {
        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open tarball: {}", e),
            path: Some(archive_path.clone()),
            source: Some(e),
        })?;

        let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(decoder);

        archive.unpack(extract_dir).map_err(|e| PumasError::InstallationFailed {
            message: format!("Failed to extract tarball: {}", e),
        })?;

        Ok(())
    }

    async fn move_to_final_location(
        &self,
        extract_dir: &PathBuf,
        version_dir: &PathBuf,
    ) -> Result<()> {
        info!(
            "Moving extracted files to {}",
            version_dir.display()
        );

        // GitHub archives typically wrap content in a single directory
        // Find the actual source directory
        let entries: Vec<_> = std::fs::read_dir(extract_dir)
            .map_err(|e| PumasError::Io {
                message: format!("Failed to read extract directory: {}", e),
                path: Some(extract_dir.clone()),
                source: Some(e),
            })?
            .filter_map(|e| e.ok())
            .collect();

        let source_dir = if entries.len() == 1 && entries[0].path().is_dir() {
            entries[0].path()
        } else {
            extract_dir.clone()
        };

        // Ensure versions directory exists
        if let Some(parent) = version_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create versions directory: {}", e),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        // Remove existing version directory if present
        if version_dir.exists() {
            std::fs::remove_dir_all(version_dir).map_err(|e| PumasError::Io {
                message: format!("Failed to remove existing version directory: {}", e),
                path: Some(version_dir.clone()),
                source: Some(e),
            })?;
        }

        // Move (rename) the directory
        std::fs::rename(&source_dir, version_dir).map_err(|e| {
            // If rename fails (cross-device), fall back to copy + delete
            debug!("Rename failed, falling back to copy: {}", e);
            if let Err(copy_err) = self.copy_dir_recursive(&source_dir, version_dir) {
                return copy_err;
            }
            if let Err(rm_err) = std::fs::remove_dir_all(&source_dir) {
                warn!("Failed to remove source after copy: {}", rm_err);
            }
            PumasError::Io {
                message: "Moved via copy".to_string(),
                path: None,
                source: None,
            }
        }).or_else(|e| {
            if e.to_string().contains("Moved via copy") {
                Ok(())
            } else {
                Err(e)
            }
        })?;

        Ok(())
    }

    fn copy_dir_recursive(&self, src: &PathBuf, dst: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(dst).map_err(|e| PumasError::Io {
            message: format!("Failed to create directory: {}", e),
            path: Some(dst.clone()),
            source: Some(e),
        })?;

        for entry in std::fs::read_dir(src).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", e),
            path: Some(src.clone()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read entry: {}", e),
                path: Some(src.clone()),
                source: Some(e),
            })?;

            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path).map_err(|e| PumasError::Io {
                    message: format!("Failed to copy file: {}", e),
                    path: Some(src_path.clone()),
                    source: Some(e),
                })?;
            }
        }

        Ok(())
    }

    async fn create_venv(
        &self,
        tag: &str,
        version_dir: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Creating virtual environment for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Venv,
                0.0,
                Some("Creating virtual environment..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Venv,
                message: "Creating virtual environment...".to_string(),
            })
            .await;

        let venv_dir = version_dir.join("venv");

        // Create venv using Python
        let output = tokio::process::Command::new("python3")
            .args(["-m", "venv", venv_dir.to_string_lossy().as_ref()])
            .current_dir(version_dir)
            .output()
            .await
            .map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to create virtual environment: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::InstallationFailed {
                message: format!("Virtual environment creation failed: {}", stderr),
            });
        }

        // Ensure pip is up to date
        let venv_python = venv_dir.join("bin").join("python");
        let _ = tokio::process::Command::new(&venv_python)
            .args(["-m", "ensurepip", "--upgrade"])
            .output()
            .await;

        let _ = tokio::process::Command::new(&venv_python)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await;

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Venv, 100.0, Some("Virtual environment created"));
        }
        let _ = progress_tx
            .send(ProgressUpdate::Venv {
                message: "Virtual environment created".to_string(),
            })
            .await;

        info!("Virtual environment created");
        Ok(())
    }

    async fn install_deps(
        &self,
        tag: &str,
        version_dir: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Installing dependencies for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Dependencies,
                0.0,
                Some("Installing dependencies..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Dependencies,
                message: "Installing dependencies...".to_string(),
            })
            .await;

        let requirements_path = version_dir.join("requirements.txt");
        if !requirements_path.exists() {
            info!("No requirements.txt found, skipping dependency installation");
            return Ok(());
        }

        let venv_python = version_dir.join("venv").join("bin").join("python");

        // Set up pip environment
        let pip_cache_dir = self.pip_cache_dir();
        std::fs::create_dir_all(&pip_cache_dir).ok();

        // Install requirements
        let mut cmd = tokio::process::Command::new(&venv_python);
        cmd.args([
            "-m",
            "pip",
            "install",
            "-r",
            requirements_path.to_string_lossy().as_ref(),
        ])
        .env("PIP_CACHE_DIR", &pip_cache_dir)
        .current_dir(version_dir);

        let output = cmd.output().await.map_err(|e| PumasError::InstallationFailed {
            message: format!("Failed to run pip install: {}", e),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Dependency installation failed: {}", stderr);
            return Err(PumasError::DependencyFailed {
                message: stderr.to_string(),
            });
        }

        // Install global required packages
        let global_packages = ["setproctitle"];
        for pkg in &global_packages {
            let _ = tokio::process::Command::new(&venv_python)
                .args(["-m", "pip", "install", pkg])
                .env("PIP_CACHE_DIR", &pip_cache_dir)
                .output()
                .await;
        }

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Dependencies,
                100.0,
                Some("Dependencies installed"),
            );
        }

        info!("Dependencies installed");
        Ok(())
    }

    async fn finalize_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        version_dir: &PathBuf,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Finalizing installation for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Setup, 0.0, Some("Finalizing installation..."));
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            })
            .await;

        // Get Python version
        let venv_python = version_dir.join("venv").join("bin").join("python");
        let python_version = if venv_python.exists() {
            let output = tokio::process::Command::new(&venv_python)
                .args(["--version"])
                .output()
                .await
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string());
            output
        } else {
            None
        };

        // Create metadata entry
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version,
            git_commit: None, // Could extract from git log if needed
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url: release.zipball_url.clone().or(release.tarball_url.clone()),
            size: release.total_size,
            requirements_hash: None, // Could compute if needed
            dependencies_installed: Some(true),
        };

        // Save metadata
        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(InstallationStage::Setup, 100.0, Some("Installation complete"));
        }
        let _ = progress_tx
            .send(ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            })
            .await;

        info!("Installation of {} finalized", tag);
        Ok(())
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.cancel_flag.load(Ordering::SeqCst) {
            Err(PumasError::InstallationFailed {
                message: "Installation cancelled by user".to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(self.app_id.versions_dir_name())
    }

    fn logs_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join(PathsConfig::LOGS_DIR_NAME)
    }

    fn pip_cache_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME)
            .join(PathsConfig::PIP_CACHE_DIR_NAME)
    }

    fn slugify_tag(&self, tag: &str) -> String {
        tag.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_tag() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let metadata_manager = Arc::new(MetadataManager::new(temp_dir.path()));
        let progress_tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            temp_dir.path().to_path_buf(),
        )));

        let installer = VersionInstaller::new(
            temp_dir.path().to_path_buf(),
            AppId::ComfyUI,
            metadata_manager,
            progress_tracker,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(installer.slugify_tag("v1.0.0"), "v100");
        assert_eq!(installer.slugify_tag("v1.0.0-beta.1"), "v100-beta1");
    }
}
