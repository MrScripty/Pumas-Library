//! Download management for HuggingFace models.
//!
//! Handles multi-file downloads with progress tracking, pause/resume,
//! cancellation, retry with resume, and crash recovery via persistence.

use super::types::{DownloadCompletionCallback, DownloadCompletionInfo, DownloadState, FileToDownload, HF_HUB_BASE};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::download_store::{DownloadPersistence, PersistedDownload};
use crate::model_library::types::{DownloadRequest, DownloadStatus, ModelDownloadProgress};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Regular (non-LFS) files that should be automatically fetched alongside
/// weight files. These are config/tokenizer files needed by inference engines.
/// Matched by filename (the last path component).
const AUXILIARY_FILE_PATTERNS: &[&str] = &[
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "generation_config.json",
    "special_tokens_map.json",
    "tokenizer.model",
    "vocab.json",
    "merges.txt",
    "added_tokens.json",
    "preprocessor_config.json",
    "chat_template.jinja",
    "model.safetensors.index.json",
];

/// Select auxiliary config/tokenizer files from a repo's regular (non-LFS) file list.
fn select_auxiliary_files(regular_files: &[String]) -> Vec<String> {
    regular_files
        .iter()
        .filter(|path| {
            let filename = path.rsplit('/').next().unwrap_or(path);
            AUXILIARY_FILE_PATTERNS.iter().any(|pattern| filename == *pattern)
        })
        .cloned()
        .collect()
}

impl HuggingFaceClient {
    /// Restore persisted downloads from disk.
    ///
    /// Called during startup to recover paused/errored downloads from a previous session.
    /// Only restores entries whose `.part` file still exists on disk.
    pub async fn restore_persisted_downloads(&self) {
        let persistence = match &self.persistence {
            Some(p) => p,
            None => return,
        };

        let entries = persistence.load_all();
        if entries.is_empty() {
            return;
        }

        info!("Restoring {} persisted downloads", entries.len());
        let mut downloads = self.downloads.write().await;

        for entry in entries {
            // For multi-file downloads, check if any file or .part exists.
            // For single-file downloads (legacy), check the primary .part file.
            let all_filenames = if entry.filenames.is_empty() {
                vec![entry.filename.clone()]
            } else {
                entry.filenames.clone()
            };

            let has_any_file = all_filenames.iter().any(|f| {
                let part = entry.dest_dir.join(format!(
                    "{}{}",
                    f,
                    crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
                ));
                let completed = entry.dest_dir.join(f);
                part.exists() || completed.exists()
            });

            if !has_any_file {
                info!(
                    "Removing stale persisted download {} (no files on disk)",
                    entry.download_id
                );
                let _ = persistence.remove(&entry.download_id);
                continue;
            }

            // Sum bytes from completed files + current .part file for progress
            let downloaded_bytes: u64 = all_filenames
                .iter()
                .map(|f| {
                    let completed = entry.dest_dir.join(f);
                    let part = entry.dest_dir.join(format!(
                        "{}{}",
                        f,
                        crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
                    ));
                    if completed.exists() {
                        std::fs::metadata(&completed).map(|m| m.len()).unwrap_or(0)
                    } else if part.exists() {
                        std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0)
                    } else {
                        0
                    }
                })
                .sum();

            // Log status transitions for visibility
            match entry.status {
                DownloadStatus::Queued | DownloadStatus::Downloading => {
                    info!(
                        "Download {} was {:?} at shutdown, marking as Paused for resume",
                        entry.download_id, entry.status
                    );
                }
                _ => {}
            }

            let state = DownloadState::from_persisted(&entry, downloaded_bytes);

            info!(
                "Restoring download {}: {} ({} bytes on disk, status {:?})",
                entry.download_id, entry.repo_id, downloaded_bytes, state.status
            );

            downloads.insert(entry.download_id.clone(), state);
        }
    }

    /// Start a model download (supports multi-file models).
    ///
    /// Returns a download ID for tracking progress.
    /// For multi-shard models or "all files" requests, all files are downloaded
    /// sequentially under a single download ID.
    pub async fn start_download(
        &self,
        request: &DownloadRequest,
        dest_dir: &Path,
    ) -> Result<String> {
        let download_id = uuid::Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Get file info
        let tree = self.get_repo_files(&request.repo_id).await?;

        // Resolve files to download
        let files: Vec<FileToDownload> = if let Some(ref f) = request.filename {
            // Specific file requested
            let lfs = tree.lfs_files.iter().find(|lf| lf.filename == *f);
            vec![FileToDownload {
                filename: f.clone(),
                size: lfs.map(|l| l.size),
                sha256: lfs.map(|l| l.sha256.clone()),
            }]
        } else if let Some(ref quant) = request.quant {
            // All files matching this quantization (handles sharded models)
            let matching: Vec<FileToDownload> = tree
                .lfs_files
                .iter()
                .filter(|f| f.filename.contains(quant.as_str()))
                .map(|f| FileToDownload {
                    filename: f.filename.clone(),
                    size: Some(f.size),
                    sha256: Some(f.sha256.clone()),
                })
                .collect();
            if matching.is_empty() {
                return Err(PumasError::ModelNotFound {
                    model_id: format!("{}:{}", request.repo_id, quant),
                });
            }
            matching
        } else {
            // All LFS files in the repo
            if tree.lfs_files.is_empty() {
                return Err(PumasError::ModelNotFound {
                    model_id: request.repo_id.clone(),
                });
            }
            tree.lfs_files
                .iter()
                .map(|f| FileToDownload {
                    filename: f.filename.clone(),
                    size: Some(f.size),
                    sha256: Some(f.sha256.clone()),
                })
                .collect()
        };

        // SHA256 of the primary (largest) weight file for import metadata
        // (must be computed before auxiliary files are appended)
        let primary_file = files.iter().max_by_key(|f| f.size.unwrap_or(0));
        let known_sha256 = primary_file.and_then(|f| f.sha256.clone());

        // Append auxiliary config/tokenizer files from the repo's regular file list
        let auxiliary = select_auxiliary_files(&tree.regular_files);
        if !auxiliary.is_empty() {
            info!(
                "Including {} auxiliary config file(s) for {}",
                auxiliary.len(),
                request.repo_id
            );
        }
        let mut files = files;
        for aux_filename in &auxiliary {
            files.push(FileToDownload {
                filename: aux_filename.clone(),
                size: None,
                sha256: None,
            });
        }

        // Total bytes across all files (sum known sizes; auxiliary files
        // lack LFS size metadata but are small enough to not materially
        // affect progress accuracy)
        let total_bytes: Option<u64> = {
            let known_sum: u64 = files.iter().filter_map(|f| f.size).sum();
            if known_sum > 0 { Some(known_sum) } else { None }
        };
        let first_filename = files[0].filename.clone();

        let pause_flag = Arc::new(AtomicBool::new(false));

        // Create download state
        let state = DownloadState {
            download_id: download_id.clone(),
            repo_id: request.repo_id.clone(),
            status: DownloadStatus::Queued,
            progress: 0.0,
            downloaded_bytes: 0,
            total_bytes,
            speed: 0.0,
            cancel_flag: cancel_flag.clone(),
            pause_flag: pause_flag.clone(),
            error: None,
            dest_dir: dest_dir.to_path_buf(),
            filename: first_filename.clone(),
            files: files.clone(),
            files_completed: 0,
            download_request: Some(request.clone()),
            known_sha256: known_sha256.clone(),
        };

        self.downloads
            .write()
            .await
            .insert(download_id.clone(), state);

        // Persist download metadata for crash recovery
        if let Some(ref persistence) = self.persistence {
            let _ = persistence.save(&PersistedDownload {
                download_id: download_id.clone(),
                repo_id: request.repo_id.clone(),
                filename: first_filename.clone(),
                filenames: files.iter().map(|f| f.filename.clone()).collect(),
                dest_dir: dest_dir.to_path_buf(),
                total_bytes,
                status: DownloadStatus::Queued,
                download_request: request.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: known_sha256.clone(),
            });
        }

        info!(
            "Starting download {} for {} ({} file{})",
            download_id, request.repo_id, files.len(),
            if files.len() == 1 { "" } else { "s" }
        );

        // Spawn download task (uses download_client which has no total timeout)
        let client = self.download_client.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.clone();
        let repo_id = request.repo_id.clone();
        let dest_dir = dest_dir.to_path_buf();
        let persistence = self.persistence.clone();
        let completion_callback = self.completion_callback.clone();
        let auth_header = self.auth_header_value().await;

        tokio::spawn(async move {
            let result = Self::run_download(
                client,
                downloads.clone(),
                &download_id_clone,
                &repo_id,
                &files,
                &dest_dir,
                cancel_flag,
                pause_flag,
                persistence.clone(),
                completion_callback,
                auth_header,
            )
            .await;

            if let Err(e) = result {
                // DownloadPaused is not a real error -- status already set by run_download
                if matches!(e, PumasError::DownloadPaused) {
                    info!("Download paused for {}", repo_id);
                    // Persistence already updated in run_download
                    return;
                }
                error!("Download failed for {}: {}", repo_id, e);
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(&download_id_clone) {
                    state.status = DownloadStatus::Error;
                    state.error = Some(e.to_string());
                }
                // Update persistence with error status (preserve for resume)
                if let Some(ref persistence) = persistence {
                    if let Ok(mut entries) = Ok::<Vec<_>, ()>(persistence.load_all()) {
                        if let Some(entry) = entries.iter_mut().find(|d| d.download_id == download_id_clone) {
                            entry.status = DownloadStatus::Error;
                            let _ = persistence.save(entry);
                        }
                    }
                }
            }
        });

        Ok(download_id)
    }

    /// Run the download in the background with retry and resume support.
    ///
    /// Downloads all files sequentially. Files that already exist on disk
    /// (from a previous partial download) are skipped automatically.
    async fn run_download(
        client: reqwest::Client,
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_id: &str,
        repo_id: &str,
        files: &[FileToDownload],
        dest_dir: &Path,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        persistence: Option<Arc<DownloadPersistence>>,
        completion_callback: Option<DownloadCompletionCallback>,
        auth_header: Option<String>,
    ) -> Result<()> {
        use crate::config::NetworkConfig;
        use crate::network::RetryConfig;

        // Update status to downloading
        {
            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::Downloading;
            }
        }

        std::fs::create_dir_all(dest_dir)?;

        let retry_config = RetryConfig::new()
            .with_max_attempts(NetworkConfig::HF_DOWNLOAD_MAX_RETRIES)
            .with_base_delay(NetworkConfig::HF_DOWNLOAD_RETRY_BASE_DELAY);

        // Download each file sequentially
        let mut bytes_offset: u64 = 0;

        for (file_idx, file_info) in files.iter().enumerate() {
            let filename = &file_info.filename;
            let dest_path = dest_dir.join(filename);
            let part_path = dest_dir.join(format!(
                "{}{}",
                filename,
                NetworkConfig::DOWNLOAD_TEMP_SUFFIX
            ));

            // Skip files that already exist (completed from previous run)
            if dest_path.exists() {
                let existing_size = tokio::fs::metadata(&dest_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                bytes_offset += existing_size;
                info!(
                    "Skipping already-downloaded file {}/{} ({} bytes)",
                    repo_id, filename, existing_size
                );

                // Update state
                {
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.files_completed = file_idx + 1;
                        state.downloaded_bytes = bytes_offset;
                        if let Some(total) = state.total_bytes {
                            state.progress = bytes_offset as f32 / total as f32;
                        }
                    }
                }
                continue;
            }

            // Update current filename in state
            {
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.filename = filename.clone();
                }
            }

            let url = format!("{}/{}/resolve/main/{}", HF_HUB_BASE, repo_id, filename);

            let mut last_error: Option<PumasError> = None;

            let mut file_completed = false;
            for attempt in 0..retry_config.max_attempts {
                // Check cancellation before each attempt
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = tokio::fs::remove_file(&part_path).await;
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.status = DownloadStatus::Cancelled;
                    }
                    if let Some(ref persistence) = persistence {
                        let _ = persistence.remove(download_id);
                    }
                    return Err(PumasError::DownloadCancelled);
                }

                // Check pause before each attempt
                if pause_flag.load(Ordering::Relaxed) {
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.status = DownloadStatus::Paused;
                    }
                    if let Some(ref persistence) = persistence {
                        Self::persist_status_update(
                            persistence,
                            download_id,
                            DownloadStatus::Paused,
                        );
                    }
                    return Err(PumasError::DownloadPaused);
                }

                // Determine resume offset from existing .part file
                let resume_from_byte = tokio::fs::metadata(&part_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);

                if attempt > 0 {
                    warn!(
                        "Retry {}/{} for {}/{} (resuming from byte {})",
                        attempt + 1,
                        retry_config.max_attempts,
                        repo_id,
                        filename,
                        resume_from_byte
                    );

                    // Reset status to Downloading for the retry
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.status = DownloadStatus::Downloading;
                        state.error = None;
                    }
                }

                match Self::download_attempt(
                    &client,
                    &downloads,
                    download_id,
                    &url,
                    &part_path,
                    file_info.size,
                    resume_from_byte,
                    bytes_offset,
                    &cancel_flag,
                    &pause_flag,
                    auth_header.as_deref(),
                )
                .await
                {
                    Ok(_) => {
                        // Rename .part to final path atomically
                        tokio::fs::rename(&part_path, &dest_path).await.map_err(
                            |e| PumasError::DownloadFailed {
                                url: url.clone(),
                                message: format!("Failed to rename temp file: {}", e),
                            },
                        )?;

                        file_completed = true;
                        break;
                    }
                    Err(e) => {
                        // Paused -- .part preserved, not a real error
                        if matches!(e, PumasError::DownloadPaused) {
                            if let Some(ref persistence) = persistence {
                                Self::persist_status_update(
                                    persistence,
                                    download_id,
                                    DownloadStatus::Paused,
                                );
                            }
                            return Err(e);
                        }

                        if !e.is_retryable() || cancel_flag.load(Ordering::Relaxed) {
                            if cancel_flag.load(Ordering::Relaxed) {
                                let _ = tokio::fs::remove_file(&part_path).await;
                                if let Some(ref persistence) = persistence {
                                    let _ = persistence.remove(download_id);
                                }
                            }
                            return Err(e);
                        }

                        warn!(
                            "Download attempt {}/{} failed for {}/{}: {}",
                            attempt + 1,
                            retry_config.max_attempts,
                            repo_id,
                            filename,
                            e
                        );
                        last_error = Some(e);

                        if attempt + 1 < retry_config.max_attempts {
                            let delay = retry_config.calculate_delay(attempt);
                            debug!("Waiting {:?} before retry", delay);
                            tokio::time::sleep(delay).await;
                        }
                    }
                }
            }

            if !file_completed {
                return Err(last_error.unwrap_or_else(|| PumasError::DownloadFailed {
                    url,
                    message: "All retry attempts exhausted".to_string(),
                }));
            }

            // File completed -- use actual file size for accurate offset
            let actual_size = tokio::fs::metadata(&dest_path)
                .await
                .map(|m| m.len())
                .unwrap_or(file_info.size.unwrap_or(0));
            bytes_offset += actual_size;
            {
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.files_completed = file_idx + 1;
                    state.downloaded_bytes = bytes_offset;
                }
            }

            info!(
                "File {}/{} complete ({}/{})",
                repo_id,
                filename,
                file_idx + 1,
                files.len()
            );
        }

        // All files completed -- update status and fire callback
        let completion_info = {
            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::Completed;
                state.progress = 1.0;
                state.files_completed = files.len();

                state.download_request.as_ref().map(|req| {
                    // Use the primary (largest) filename for the completion info
                    let primary_filename = files
                        .iter()
                        .max_by_key(|f| f.size.unwrap_or(0))
                        .map(|f| f.filename.clone())
                        .unwrap_or_else(|| state.filename.clone());

                    DownloadCompletionInfo {
                        download_id: download_id.to_string(),
                        dest_dir: state.dest_dir.clone(),
                        filename: primary_filename,
                        filenames: files.iter().map(|f| f.filename.clone()).collect(),
                        download_request: req.clone(),
                        known_sha256: state.known_sha256.clone(),
                    }
                })
            } else {
                None
            }
        };

        // Invoke completion callback for in-place import
        if let (Some(ref callback), Some(info)) = (&completion_callback, completion_info) {
            callback(info);
        }

        // Remove from persistence -- download is done
        if let Some(ref persistence) = persistence {
            let _ = persistence.remove(download_id);
        }

        Ok(())
    }

    /// Execute a single download attempt, optionally resuming from a byte offset.
    ///
    /// `file_size_expected` is the expected size of this individual file.
    /// `bytes_offset` is bytes already downloaded from previous files in a multi-file download.
    /// Overall progress is calculated as `(bytes_offset + file_downloaded) / overall_total`.
    async fn download_attempt(
        client: &reqwest::Client,
        downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
        download_id: &str,
        url: &str,
        part_path: &Path,
        file_size_expected: Option<u64>,
        resume_from_byte: u64,
        bytes_offset: u64,
        cancel_flag: &Arc<AtomicBool>,
        pause_flag: &Arc<AtomicBool>,
        auth_header: Option<&str>,
    ) -> Result<()> {
        use futures::StreamExt;

        let mut request = client.get(url);
        if let Some(auth) = auth_header {
            request = request.header("Authorization", auth);
        }
        if resume_from_byte > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from_byte));
            info!("Resuming download from byte {}", resume_from_byte);
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("Download request failed: {}", e),
            cause: Some(e.to_string()),
        })?;

        let status = response.status();

        // Check for non-success responses (but 206 Partial Content is expected for resume)
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(PumasError::DownloadFailed {
                url: url.to_string(),
                message: format!("HTTP {}", status),
            });
        }

        // Determine if we're actually resuming
        let is_resuming =
            resume_from_byte > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if resume_from_byte > 0 && !is_resuming {
            warn!("Server does not support Range requests, restarting from zero");
        }

        // Per-file total for completeness verification
        let file_total = if is_resuming {
            file_size_expected
        } else {
            response.content_length().or(file_size_expected)
        };

        // Open file: append for resume, create for fresh start
        let mut file = if is_resuming {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(part_path)
                .await?
        } else {
            tokio::fs::File::create(part_path).await?
        };

        let mut downloaded: u64 = if is_resuming { resume_from_byte } else { 0 };
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                let _ = tokio::fs::remove_file(part_path).await;

                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Cancelled;
                }

                return Err(PumasError::DownloadCancelled);
            }

            if pause_flag.load(Ordering::Relaxed) {
                file.flush().await?;
                drop(file);
                // Preserve .part file for resume

                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Paused;
                }

                return Err(PumasError::DownloadPaused);
            }

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Download stream error: {}", e),
                cause: Some(e.to_string()),
            })?;

            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Update overall progress (bytes_offset accounts for completed files)
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            };

            let overall_downloaded = bytes_offset + downloaded;

            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.downloaded_bytes = overall_downloaded;
                state.speed = speed;
                state.progress = if let Some(total) = state.total_bytes {
                    overall_downloaded as f32 / total as f32
                } else {
                    0.0
                };
            }
        }

        file.flush().await?;
        drop(file);

        // Verify this file's download completeness
        if let Some(total) = file_total {
            if downloaded != total {
                return Err(PumasError::Network {
                    message: format!(
                        "Incomplete download: got {} of {} bytes",
                        downloaded, total
                    ),
                    cause: None,
                });
            }
        }

        Ok(())
    }

    /// Helper: update status in persistence store (best-effort).
    fn persist_status_update(
        persistence: &DownloadPersistence,
        download_id: &str,
        status: DownloadStatus,
    ) {
        let entries = persistence.load_all();
        if let Some(mut entry) = entries.into_iter().find(|d| d.download_id == download_id) {
            entry.status = status;
            let _ = persistence.save(&entry);
        }
    }

    /// Get download progress.
    pub async fn get_download_progress(&self, download_id: &str) -> Option<ModelDownloadProgress> {
        let downloads = self.downloads.read().await;
        downloads.get(download_id).map(|state| ModelDownloadProgress {
            download_id: state.download_id.clone(),
            repo_id: Some(state.repo_id.clone()),
            status: state.status,
            progress: Some(state.progress),
            downloaded_bytes: Some(state.downloaded_bytes),
            total_bytes: state.total_bytes,
            speed: Some(state.speed),
            eta_seconds: if state.speed > 0.0 && state.total_bytes.is_some() {
                let remaining = state.total_bytes.unwrap() - state.downloaded_bytes;
                Some(remaining as f64 / state.speed)
            } else {
                None
            },
            error: state.error.clone(),
        })
    }

    /// Cancel a download.
    pub async fn cancel_download(&self, download_id: &str) -> Result<bool> {
        let downloads = self.downloads.read().await;
        if let Some(state) = downloads.get(download_id) {
            state.cancel_flag.store(true, Ordering::Relaxed);
            // Remove from persistence -- cancelled downloads don't survive restart
            if let Some(ref persistence) = self.persistence {
                let _ = persistence.remove(download_id);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all downloads (active, paused, completed, etc.).
    pub async fn list_downloads(&self) -> Vec<ModelDownloadProgress> {
        let downloads = self.downloads.read().await;
        downloads
            .values()
            .map(|state| ModelDownloadProgress {
                download_id: state.download_id.clone(),
                repo_id: Some(state.repo_id.clone()),
                status: state.status,
                progress: Some(state.progress),
                downloaded_bytes: Some(state.downloaded_bytes),
                total_bytes: state.total_bytes,
                speed: Some(state.speed),
                eta_seconds: if state.speed > 0.0 && state.total_bytes.is_some() {
                    let remaining = state.total_bytes.unwrap().saturating_sub(state.downloaded_bytes);
                    Some(remaining as f64 / state.speed)
                } else {
                    None
                },
                error: state.error.clone(),
            })
            .collect()
    }

    /// Pause an active download. Preserves the `.part` file for later resume.
    pub async fn pause_download(&self, download_id: &str) -> Result<bool> {
        let downloads = self.downloads.read().await;
        if let Some(state) = downloads.get(download_id) {
            if state.status == DownloadStatus::Downloading
                || state.status == DownloadStatus::Queued
            {
                state.pause_flag.store(true, Ordering::Relaxed);
                drop(downloads);
                // Set transitional Pausing status
                let mut downloads = self.downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Pausing;
                }
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Resume a paused or errored download from its `.part` file.
    pub async fn resume_download(&self, download_id: &str) -> Result<bool> {
        let (repo_id, files, dest_dir, cancel_flag, pause_flag) = {
            let mut downloads = self.downloads.write().await;
            let state = match downloads.get_mut(download_id) {
                Some(s) => s,
                None => return Ok(false),
            };

            if state.status != DownloadStatus::Paused && state.status != DownloadStatus::Error {
                return Ok(false);
            }

            // Reset flags and status for re-download
            state.pause_flag.store(false, Ordering::Relaxed);
            state.cancel_flag.store(false, Ordering::Relaxed);
            state.status = DownloadStatus::Queued;
            state.error = None;
            state.speed = 0.0;

            (
                state.repo_id.clone(),
                state.files.clone(),
                state.dest_dir.clone(),
                state.cancel_flag.clone(),
                state.pause_flag.clone(),
            )
        };

        // Update persistence to Queued status
        if let Some(ref persistence) = self.persistence {
            Self::persist_status_update(persistence, download_id, DownloadStatus::Queued);
        }

        // Re-spawn the download task
        let client = self.download_client.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.to_string();
        let persistence = self.persistence.clone();
        let completion_callback = self.completion_callback.clone();
        let auth_header = self.auth_header_value().await;

        tokio::spawn(async move {
            let result = Self::run_download(
                client,
                downloads.clone(),
                &download_id_clone,
                &repo_id,
                &files,
                &dest_dir,
                cancel_flag,
                pause_flag,
                persistence.clone(),
                completion_callback,
                auth_header,
            )
            .await;

            if let Err(e) = result {
                if matches!(e, PumasError::DownloadPaused) {
                    info!("Download paused for {}", repo_id);
                    return;
                }
                error!("Download failed for {}: {}", repo_id, e);
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(&download_id_clone) {
                    state.status = DownloadStatus::Error;
                    state.error = Some(e.to_string());
                }
                // Update persistence with error status
                if let Some(ref persistence) = persistence {
                    Self::persist_status_update(persistence, &download_id_clone, DownloadStatus::Error);
                }
            }
        });

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_auxiliary_files_filters_correctly() {
        let regular_files = vec![
            ".gitattributes".to_string(),
            "README.md".to_string(),
            "config.json".to_string(),
            "tokenizer.json".to_string(),
            "tokenizer_config.json".to_string(),
            "generation_config.json".to_string(),
            "special_tokens_map.json".to_string(),
            "modeling_trado.py".to_string(),
            "tokenizer.model".to_string(),
            "vocab.json".to_string(),
            "merges.txt".to_string(),
            "added_tokens.json".to_string(),
            "preprocessor_config.json".to_string(),
            "chat_template.jinja".to_string(),
            "model.safetensors.index.json".to_string(),
        ];

        let selected = select_auxiliary_files(&regular_files);
        assert_eq!(selected.len(), 12);
        assert!(selected.contains(&"config.json".to_string()));
        assert!(selected.contains(&"tokenizer.json".to_string()));
        assert!(selected.contains(&"tokenizer_config.json".to_string()));
        assert!(selected.contains(&"generation_config.json".to_string()));
        assert!(selected.contains(&"special_tokens_map.json".to_string()));
        assert!(selected.contains(&"tokenizer.model".to_string()));
        assert!(selected.contains(&"vocab.json".to_string()));
        assert!(selected.contains(&"merges.txt".to_string()));
        assert!(selected.contains(&"added_tokens.json".to_string()));
        assert!(selected.contains(&"preprocessor_config.json".to_string()));
        assert!(selected.contains(&"chat_template.jinja".to_string()));
        assert!(selected.contains(&"model.safetensors.index.json".to_string()));
        assert!(!selected.contains(&"README.md".to_string()));
        assert!(!selected.contains(&".gitattributes".to_string()));
        assert!(!selected.contains(&"modeling_trado.py".to_string()));
    }

    #[test]
    fn test_select_auxiliary_files_empty_input() {
        let selected = select_auxiliary_files(&[]);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_select_auxiliary_files_no_matches() {
        let regular_files = vec![
            ".gitattributes".to_string(),
            "README.md".to_string(),
            "modeling_sdar.py".to_string(),
        ];
        let selected = select_auxiliary_files(&regular_files);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_select_auxiliary_files_ignores_subdirectory_paths() {
        let regular_files = vec![
            "subdir/config.json".to_string(),
            "tokenizer.json".to_string(),
        ];
        let selected = select_auxiliary_files(&regular_files);
        // Both should match — the subdirectory path matches by filename component
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&"subdir/config.json".to_string()));
        assert!(selected.contains(&"tokenizer.json".to_string()));
    }
}
