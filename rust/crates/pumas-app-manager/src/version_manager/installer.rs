//! Version installation with progress reporting.
//!
//! Handles downloading, extracting, and setting up new versions.

use pumas_core::config::{AppId, InstallationConfig, PathsConfig};
use pumas_core::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_core::models::InstallationStage;
use pumas_core::network::GitHubRelease;
use crate::version_manager::progress::{InstallationProgressTracker, ProgressUpdate};
use pumas_core::{PumasError, Result};
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
    pub async fn install_version(
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

        // Create HTTP client
        let client = reqwest::Client::builder()
            .timeout(InstallationConfig::URL_FETCH_TIMEOUT)
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
