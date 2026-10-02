//! Ollama-specific version management.
//!
//! Handles Ollama binary downloads and installation.
//! since Ollama is a pre-built binary with no Python dependencies.

use crate::version_manager::progress::ProgressUpdate;
use crate::version_manager::state::VersionState;
use futures::FutureExt;
use pumas_library::config::{AppId, InstallationConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::InstallationStage;
use pumas_library::network::{GitHubAsset, GitHubClient, GitHubRelease};
use pumas_library::{PumasError, Result};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

/// Ollama version manager specialized for binary-only installation.
///
/// Await construction and retain an owner until `shutdown` completes before
/// dropping its storage or stopping the Tokio runtime. Clones share admission
/// and the shutdown receipt; dropping an operation waiter does not cancel it.
#[derive(Clone)]
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
    shutdown_flag: Arc<AtomicBool>,
    activities: Arc<StdMutex<OllamaActivities>>,
}

#[derive(Default)]
struct OllamaActivities {
    tasks: super::InstallationTasks,
    completion: Option<super::InstallationShutdown>,
}

impl OllamaVersionManager {
    /// Create a new Ollama version manager.
    pub async fn new(
        launcher_root: PathBuf,
        metadata_manager: Arc<MetadataManager>,
        github_client: Arc<GitHubClient>,
    ) -> Result<Self> {
        let app_id = AppId::Ollama;
        let state = VersionState::new(&launcher_root, app_id, metadata_manager.clone()).await?;

        Ok(Self {
            launcher_root,
            app_id,
            github_repo: "ollama/ollama".to_string(),
            github_client,
            state: Arc::new(RwLock::new(state)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            install_lock: Arc::new(Mutex::new(())),
            installing_tag: Arc::new(RwLock::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            activities: Arc::new(StdMutex::new(OllamaActivities::default())),
        })
    }

    fn ensure_open(&self) -> Result<()> {
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(PumasError::Other(
                "Ollama version manager is shutting down".into(),
            ));
        }
        Ok(())
    }

    async fn owned_activity(
        &self,
        operation: impl std::future::Future<Output = Result<()>> + Send + 'static,
    ) -> Result<()> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        {
            let mut activities = self
                .activities
                .lock()
                .map_err(|_| PumasError::Other("Ollama activity registry poisoned".into()))?;
            self.ensure_open()?;
            activities.tasks.harvest_finished();
            let task = tokio::spawn(async move {
                let result = operation.await;
                let terminal = result.as_ref().map(|_| ()).map_err(ToString::to_string);
                let _ = sender.send(result);
                terminal
            });
            activities.tasks.tasks.push(task);
        }
        receiver.await.map_err(|error| {
            PumasError::Other(format!("Ollama activity lost its result: {error}"))
        })?
    }

    async fn send_progress(&self, sender: &mpsc::Sender<ProgressUpdate>, update: ProgressUpdate) {
        tokio::select! {
            _ = sender.send(update) => {},
            _ = super::wait_for_install_cancel(self.shutdown_flag.clone()) => {},
        }
    }

    /// Close mutation admission, request installation cancellation, and drain
    /// admitted install/removal activities before draining state mutations.
    /// Started work is joined, including work whose waiter was cancelled.
    /// Repeated calls retain failures, including task panics. Existing network
    /// waits and filesystem settlement may delay shutdown; elapsed time never
    /// substitutes for completion. Cancelled shutdown waiters may safely retry.
    pub async fn shutdown(&self) -> Result<()> {
        let completion = {
            let mut activities = self
                .activities
                .lock()
                .map_err(|_| PumasError::Other("Ollama activity registry poisoned".into()))?;
            if let Some(completion) = &activities.completion {
                completion.clone()
            } else {
                self.shutdown_flag.store(true, Ordering::SeqCst);
                self.cancel_flag.store(true, Ordering::SeqCst);
                let registered = std::mem::take(&mut activities.tasks);
                let owner = self.clone();
                let supervisor = tokio::spawn(async move {
                    let mut failures = registered.failures;
                    for task in registered.tasks {
                        match task.await {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => failures.push(error),
                            Err(error) => failures.push(error.to_string()),
                        }
                    }
                    if let Err(error) = owner.state.write().await.shutdown_mutations().await {
                        failures.push(error.to_string());
                    }
                    if failures.is_empty() {
                        Ok(())
                    } else {
                        Err(Arc::new(failures.join("; ")))
                    }
                });
                let completion = async move {
                    supervisor
                        .await
                        .unwrap_or_else(|error| Err(Arc::new(error.to_string())))
                }
                .boxed()
                .shared();
                activities.completion = Some(completion.clone());
                completion
            }
        };
        completion
            .await
            .map_err(|error| PumasError::Other(format!("Ollama shutdown failed: {error}")))
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

        debug!("Selecting Ollama asset for {}-{}", desired_os, desired_arch);

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
            if (cfg!(windows) && name_lower.ends_with(".exe"))
                || name_lower.ends_with(".tar.gz")
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
            warn!(
                "No suitable Ollama asset found for {}-{}",
                desired_os, desired_arch
            );
        }

        best_asset
    }

    /// Install an Ollama version.
    pub async fn install_version(
        &self,
        tag: &str,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        let owner = self.clone();
        let tag = tag.to_owned();
        self.owned_activity(async move { owner.install_version_owned(&tag, progress_tx).await })
            .await
    }

    async fn install_version_owned(
        &self,
        tag: &str,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        // Acquire installation lock
        let _lock = self.install_lock.lock().await;

        // Set installing tag
        {
            let mut installing = self.installing_tag.write().await;
            let _admission = self
                .activities
                .lock()
                .map_err(|_| PumasError::Other("Ollama activity registry poisoned".into()))?;
            self.ensure_open()?;
            *installing = Some(tag.to_string());
            self.cancel_flag.store(false, Ordering::SeqCst);
        }

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
            self.send_progress(
                &tx,
                ProgressUpdate::Completed {
                    success: result.is_ok(),
                },
            )
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
        fs::create_dir_all(&version_path)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create version directory: {}", e),
                path: Some(version_path.clone()),
                source: Some(e),
            })?;

        // Send stage update
        if let Some(ref tx) = progress_tx {
            self.send_progress(
                tx,
                ProgressUpdate::StageChanged {
                    stage: InstallationStage::Download,
                    message: format!("Fetching release {}", tag),
                },
            )
            .await;
        }

        // Fetch releases and find the matching one
        let releases = self
            .github_client
            .get_releases(&self.github_repo, false)
            .await?;

        let release = releases.iter().find(|r| r.tag_name == tag).ok_or_else(|| {
            PumasError::VersionNotFound {
                tag: tag.to_string(),
            }
        })?;

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
            self.send_progress(
                tx,
                ProgressUpdate::StageChanged {
                    stage: InstallationStage::Download,
                    message: format!("Downloading {}", asset.name),
                },
            )
            .await;
        }

        self.download_file(download_url, &archive_path, progress_tx.clone())
            .await?;

        // Check for cancellation
        self.check_cancelled()?;

        // Extract and set up
        if let Some(ref tx) = progress_tx {
            self.send_progress(
                tx,
                ProgressUpdate::StageChanged {
                    stage: InstallationStage::Extract,
                    message: "Extracting binary".to_string(),
                },
            )
            .await;
        }

        self.extract_binary(&archive_path, &version_path).await?;

        // Clean up archive
        if path_exists(&archive_path).await.unwrap_or(false) {
            let _ = fs::remove_file(&archive_path).await;
        }

        // Record in metadata
        if let Some(ref tx) = progress_tx {
            self.send_progress(
                tx,
                ProgressUpdate::StageChanged {
                    stage: InstallationStage::Setup,
                    message: "Recording installation".to_string(),
                },
            )
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
        dest: &Path,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
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
        let mut file = fs::File::create(dest)
            .await
            .map_err(|e| PumasError::io_with_path(e, dest))?;

        let transfer: Result<()> = async {
            use futures::StreamExt;
            while let Some(chunk) = stream.next().await {
                self.check_cancelled()?;

                let chunk = chunk.map_err(|e| PumasError::Network {
                    message: format!("Error reading download: {}", e),
                    cause: Some(e.to_string()),
                })?;

                file.write_all(&chunk)
                    .await
                    .map_err(|e| PumasError::io_with_path(e, dest))?;

                downloaded += chunk.len() as u64;

                if let Some(ref tx) = progress_tx {
                    self.send_progress(
                        tx,
                        ProgressUpdate::Download {
                            downloaded_bytes: downloaded,
                            total_bytes: total_size,
                            speed_bytes_per_sec: None,
                        },
                    )
                    .await;
                }
            }

            Ok(())
        }
        .await;
        // Settle this activity's existing file writes before its terminal
        // receipt, including cancellation/error exits. Dropping Tokio File
        // alone can leave blocking writes active after wrapper shutdown.
        let settlement = file
            .flush()
            .await
            .map_err(|error| PumasError::io_with_path(error, dest));
        drop(file.into_std().await);
        match (transfer, settlement) {
            (result, Ok(())) => result,
            (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(settlement)) => Err(PumasError::Other(format!(
                "{error}; Ollama file settlement failed: {settlement}"
            ))),
        }
    }

    /// Extract the binary from archive.
    async fn extract_binary(&self, archive_path: &Path, version_path: &Path) -> Result<()> {
        let archive_path = archive_path.to_path_buf();
        let version_path = version_path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            Self::extract_binary_blocking(&archive_path, &version_path)
        })
        .await
        .map_err(|e| PumasError::InstallationFailed {
            message: format!("Ollama extraction task failed: {}", e),
        })?
    }

    fn extract_binary_blocking(archive_path: &Path, version_path: &Path) -> Result<()> {
        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if archive_name.ends_with(".zip") {
            Self::extract_zip(archive_path, version_path)?;
        } else if archive_name.ends_with(".tar.gz")
            || archive_name.ends_with(".tgz")
            || archive_name.ends_with(".tar.zst")
        {
            Self::extract_tarball(archive_path, version_path)?;
        } else if archive_name.ends_with(".exe") || archive_name == "ollama" {
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

        Self::finalize_binary(version_path)
    }

    fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use std::io::Read;
        let file = std::fs::File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to read zip: {}", e),
            })?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| PumasError::InstallationFailed {
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
                file.read_to_end(&mut contents)
                    .map_err(|e| PumasError::Io {
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

    fn extract_tarball(archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use std::io::BufReader;

        let file = std::fs::File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Handle different compression formats
        if archive_name.ends_with(".tar.zst") {
            // zstd compression
            let decoder = zstd::stream::Decoder::new(BufReader::new(file)).map_err(|e| {
                PumasError::InstallationFailed {
                    message: format!("Failed to create zstd decoder: {}", e),
                }
            })?;
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(dest_dir)
                .map_err(|e| PumasError::InstallationFailed {
                    message: format!("Failed to extract tar.zst: {}", e),
                })?;
        } else {
            // gzip compression
            let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(dest_dir)
                .map_err(|e| PumasError::InstallationFailed {
                    message: format!("Failed to extract tarball: {}", e),
                })?;
        }

        Ok(())
    }

    /// Find and set up the binary in the version directory.
    fn finalize_binary(version_path: &Path) -> Result<()> {
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
        let binary = Self::find_binary_recursive(version_path)?;
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
            Self::cleanup_extracted_dirs(version_path)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: "Could not find Ollama binary in archive".to_string(),
            });
        }

        Ok(())
    }

    fn find_binary_recursive(dir: &Path) -> Result<Option<PathBuf>> {
        let binary_name = Self::binary_name();

        for entry in std::fs::read_dir(dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", e),
            path: Some(dir.to_path_buf()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read entry: {}", e),
                path: Some(dir.to_path_buf()),
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
                if let Some(found) = Self::find_binary_recursive(&path)? {
                    return Ok(Some(found));
                }
            }
        }

        Ok(None)
    }

    fn cleanup_extracted_dirs(version_path: &Path) -> Result<()> {
        let binary_name = Self::binary_name();

        for entry in std::fs::read_dir(version_path).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", e),
            path: Some(version_path.to_path_buf()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read entry: {}", e),
                path: Some(version_path.to_path_buf()),
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
        self.ensure_open()?;
        state.set_active_version(tag).await?;
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
        self.ensure_open()?;
        state.set_default_version(tag).await?;
        Ok(())
    }

    /// Uninstall a version.
    pub async fn uninstall_version(&self, tag: &str) -> Result<()> {
        let owner = self.clone();
        let tag = tag.to_owned();
        self.owned_activity(async move { owner.uninstall_version_owned(&tag).await })
            .await
    }

    async fn uninstall_version_owned(&self, tag: &str) -> Result<()> {
        let _install = self.install_lock.lock().await;
        self.ensure_open()?;
        let version_path = self.version_path(tag);

        if path_exists(&version_path).await? {
            fs::remove_dir_all(&version_path)
                .await
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to remove version directory: {}", e),
                    path: Some(version_path),
                    source: Some(e),
                })?;
        }

        // Update state (this also removes from metadata)
        {
            let mut state = self.state.write().await;
            state.remove_installed_version(tag).await?;
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
        self.github_client
            .get_releases(&self.github_repo, force_refresh)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_manager() -> (OllamaVersionManager, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let metadata = Arc::new(MetadataManager::new(root.path()));
        metadata.ensure_directories().unwrap();
        let github = Arc::new(GitHubClient::new(root.path().join("launcher-data/cache")).unwrap());
        let manager = OllamaVersionManager::new(root.path().to_owned(), metadata, github)
            .await
            .unwrap();
        std::fs::create_dir_all(manager.version_path("v1")).unwrap();
        std::fs::write(manager.get_binary_path("v1"), "complete").unwrap();
        manager
            .state
            .write()
            .await
            .add_installed_version(
                "v1",
                InstalledVersionMetadata {
                    path: "v1".into(),
                    release_tag: "v1".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        (manager, root)
    }

    fn single_blocking_worker_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap()
    }

    async fn occupy_blocking_worker() -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>)
    {
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = tokio::task::spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        entered_rx.await.unwrap();
        (release_tx, worker)
    }

    #[test]
    fn ollama_wrapper_shutdown_drains_cancelled_state_mutation_and_retains_outcome() {
        single_blocking_worker_runtime().block_on(async {
            for fail_mutation in [false, true] {
                let (manager, root) = test_manager().await;
                let state_owner = manager.state.read().await.mutation_tasks();
                let (release, blocker) = occupy_blocking_worker().await;
                let selecting = manager.clone();
                let waiter =
                    tokio::spawn(async move { selecting.set_default_version(Some("v1")).await });
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while !state_owner.has_active_tasks() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                waiter.abort();
                assert!(waiter.await.unwrap_err().is_cancelled());
                if fail_mutation {
                    let path = root
                        .path()
                        .join("launcher-data/metadata/versions-ollama.json");
                    std::fs::remove_file(&path).unwrap();
                    std::fs::create_dir(&path).unwrap();
                }
                let owner = manager.clone();
                let mut shutdown = tokio::spawn(async move { owner.shutdown().await });
                assert!(
                    tokio::time::timeout(std::time::Duration::from_millis(30), &mut shutdown)
                        .await
                        .is_err()
                );
                shutdown.abort();
                assert!(shutdown.await.unwrap_err().is_cancelled());
                release.send(()).unwrap();
                blocker.await.unwrap();
                let outcome = manager.shutdown().await.map_err(|error| error.to_string());
                assert_eq!(outcome.is_err(), fail_mutation);
                if fail_mutation {
                    assert!(outcome
                        .as_ref()
                        .unwrap_err()
                        .contains("versions-ollama.json"));
                } else {
                    assert_eq!(
                        MetadataManager::new(root.path())
                            .load_versions(Some(AppId::Ollama))
                            .unwrap()
                            .default_version
                            .as_deref(),
                        Some("v1")
                    );
                }
                assert_eq!(
                    manager.shutdown().await.map_err(|error| error.to_string()),
                    outcome
                );
                assert!(manager.set_active_version("v1").await.is_err());
                assert!(manager.set_default_version(None).await.is_err());
                assert!(manager.install_version("late", None).await.is_err());
                assert!(manager.uninstall_version("v1").await.is_err());
                assert!(manager.get_binary_path("v1").exists());
                assert!(!manager.version_path("late").exists());
            }
        });
    }

    #[test]
    fn ollama_wrapper_shutdown_settles_removal_after_waiter_cancellation() {
        single_blocking_worker_runtime().block_on(async {
            for fail_removal in [false, true] {
                let (manager, root) = test_manager().await;
                if fail_removal {
                    std::fs::remove_dir_all(manager.version_path("v1")).unwrap();
                    std::fs::write(manager.version_path("v1"), "retained").unwrap();
                }
                let (release, blocker) = occupy_blocking_worker().await;
                let removing = manager.clone();
                let waiter = tokio::spawn(async move { removing.uninstall_version("v1").await });
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while manager.install_lock.try_lock().is_ok() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                waiter.abort();
                assert!(waiter.await.unwrap_err().is_cancelled());
                let shutdown = manager.shutdown();
                tokio::pin!(shutdown);
                assert!(
                    tokio::time::timeout(std::time::Duration::from_millis(30), &mut shutdown)
                        .await
                        .is_err()
                );
                assert!(manager.version_path("v1").exists());
                release.send(()).unwrap();
                blocker.await.unwrap();
                let outcome = shutdown.await.map_err(|error| error.to_string());
                assert_eq!(outcome.is_err(), fail_removal);
                assert_eq!(manager.version_path("v1").exists(), fail_removal);
                assert_eq!(
                    MetadataManager::new(root.path())
                        .get_installed_version("v1", Some(AppId::Ollama))
                        .unwrap()
                        .is_some(),
                    fail_removal
                );
                assert_eq!(
                    manager.shutdown().await.map_err(|error| error.to_string()),
                    outcome
                );
                assert!(manager.uninstall_version("v1").await.is_err());
            }
        });
    }

    #[tokio::test]
    async fn ollama_wrapper_shutdown_rejects_queued_install_before_cancellation_reset() {
        let (manager, _root) = test_manager().await;
        let held = manager.install_lock.lock().await;
        let installing = manager.clone();
        let waiter = tokio::spawn(async move { installing.install_version("queued", None).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while manager.activities.lock().unwrap().tasks.tasks.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        let shutdown = manager.shutdown();
        tokio::pin!(shutdown);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), &mut shutdown)
                .await
                .is_err()
        );
        drop(held);
        let error = shutdown.await.unwrap_err().to_string();
        assert!(error.contains("shutting down"));
        assert!(manager.cancel_flag.load(Ordering::SeqCst));
        assert!(!manager.version_path("queued").exists());
        assert_eq!(manager.shutdown().await.unwrap_err().to_string(), error);
        assert!(manager.install_version("late", None).await.is_err());
    }

    #[test]
    fn test_binary_name() {
        let name = OllamaVersionManager::binary_name();
        #[cfg(windows)]
        assert_eq!(name, "ollama.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "ollama");
    }
}
