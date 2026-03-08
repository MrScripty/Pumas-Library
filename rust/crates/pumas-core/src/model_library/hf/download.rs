//! Download management for HuggingFace models.
//!
//! Handles multi-file downloads with progress tracking, pause/resume,
//! cancellation, retry with resume, and crash recovery via persistence.

use super::types::{
    AuxFilesCompleteCallback, AuxFilesCompleteInfo, DownloadCompletionCallback,
    DownloadCompletionInfo, DownloadState, FileToDownload, HF_HUB_BASE,
};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::download_store::{DownloadPersistence, PersistedDownload};
use crate::model_library::sharding;
use crate::model_library::types::{DownloadRequest, DownloadStatus, ModelDownloadProgress};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Regular (non-LFS) filenames that should be automatically fetched alongside
/// weight files.  These are config/tokenizer files needed by inference engines.
/// Matched by filename (the last path component).
const AUXILIARY_FILE_PATTERNS: &[&str] = &[
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "generation_config.json",
    "special_tokens_map.json",
    "tokenizer.model",
    "spiece.model",
    "vocab.json",
    "merges.txt",
    "added_tokens.json",
    "preprocessor_config.json",
    "chat_template.jinja",
    "model.safetensors.index.json",
    "scheduler_config.json",
    "model_index.json",
];

/// Select auxiliary config/tokenizer files from a repo's regular (non-LFS) file list.
fn select_auxiliary_files(regular_files: &[String]) -> Vec<String> {
    regular_files
        .iter()
        .filter(|path| {
            let filename = path.rsplit('/').next().unwrap_or(path);
            AUXILIARY_FILE_PATTERNS.contains(&filename)
        })
        .cloned()
        .collect()
}

/// Enhanced auxiliary selection that is scope-aware.
///
/// In addition to the base auxiliary patterns, this also includes:
/// - Non-weight LFS files (images, READMEs, etc.) from the full repo
/// - Subdirectory config files whose directory overlaps with selected weight files
/// - Shard index JSON files (`*.index.json`) in directories containing selected weights
fn select_auxiliary_files_for_download(
    regular_files: &[String],
    all_lfs_files: &[crate::model_library::types::LfsFileInfo],
    weight_files: &[FileToDownload],
) -> Vec<FileToDownload> {
    // Collect directory prefixes from selected weight files.
    let weight_dirs: HashSet<&str> = weight_files
        .iter()
        .filter_map(|f| f.filename.rsplit_once('/').map(|(dir, _)| dir))
        .collect();

    // Already-selected weight filenames (to avoid duplicating them in aux).
    let weight_names: HashSet<&str> = weight_files.iter().map(|f| f.filename.as_str()).collect();

    let mut aux: Vec<FileToDownload> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. Regular (non-LFS) auxiliary files — root-level by pattern.
    for path in regular_files {
        let filename = path.rsplit('/').next().unwrap_or(path);
        let is_root = !path.contains('/');
        let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

        let include = if is_root {
            // Root-level: match by auxiliary pattern.
            AUXILIARY_FILE_PATTERNS.contains(&filename)
        } else if weight_dirs.contains(dir) {
            // Subdirectory that has selected weight files: include configs.
            AUXILIARY_FILE_PATTERNS.contains(&filename) || filename.ends_with(".index.json")
        } else {
            // Always include globally-useful files regardless of directory.
            filename == "model_index.json" || filename == "scheduler_config.json"
        };

        if include && seen.insert(path.clone()) {
            aux.push(FileToDownload {
                filename: path.clone(),
                size: None,
                sha256: None,
            });
        }
    }

    // 2. Non-weight LFS files — always included (images, READMEs, etc.).
    let (_weight_groups, non_weight_lfs) = sharding::group_weight_files(all_lfs_files);
    for lf in &non_weight_lfs {
        if !weight_names.contains(lf.filename.as_str()) && seen.insert(lf.filename.clone()) {
            aux.push(FileToDownload {
                filename: lf.filename.clone(),
                size: Some(lf.size),
                sha256: Some(lf.sha256.clone()),
            });
        }
    }

    aux
}

fn retry_limit(max_attempts: u32) -> Option<u32> {
    if max_attempts == 0 {
        None
    } else {
        Some(max_attempts)
    }
}

fn retry_limit_display(limit: Option<u32>) -> String {
    match limit {
        Some(limit) => limit.to_string(),
        None => "unlimited".to_string(),
    }
}

fn retry_exhausted(
    attempt: u32,
    limit: Option<u32>,
    elapsed: Duration,
    max_elapsed: Duration,
) -> bool {
    let attempts_exhausted = limit.is_some_and(|max_attempts| attempt >= max_attempts);
    let elapsed_exhausted = max_elapsed > Duration::ZERO && elapsed >= max_elapsed;
    attempts_exhausted || elapsed_exhausted
}

fn retry_exhausted_message(
    attempt: u32,
    limit: Option<u32>,
    elapsed: Duration,
    last_error: &str,
) -> String {
    let limit_text = limit
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unlimited".to_string());
    format!(
        "Retry budget exhausted after {} attempt(s) (limit {}, elapsed {:.1}s). Last error: {}",
        attempt,
        limit_text,
        elapsed.as_secs_f64(),
        last_error
    )
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

        // Resolve weight files to download.
        // Priority: filenames (explicit list) > filename (single) > quant (substring) > all.
        let files: Vec<FileToDownload> =
            if request.bundle_format == Some(crate::models::BundleFormat::DiffusersDirectory) {
                tree.lfs_files
                    .iter()
                    .map(|f| FileToDownload {
                        filename: f.filename.clone(),
                        size: Some(f.size),
                        sha256: Some(f.sha256.clone()),
                    })
                    .collect()
            } else if let Some(ref fnames) = request.filenames {
                // Explicit file list from grouped file selection
                let name_set: HashSet<&str> = fnames.iter().map(|s| s.as_str()).collect();
                let matching: Vec<FileToDownload> = tree
                    .lfs_files
                    .iter()
                    .filter(|f| name_set.contains(f.filename.as_str()))
                    .map(|f| FileToDownload {
                        filename: f.filename.clone(),
                        size: Some(f.size),
                        sha256: Some(f.sha256.clone()),
                    })
                    .collect();
                if matching.is_empty() {
                    return Err(PumasError::ModelNotFound {
                        model_id: format!("{}:{} files", request.repo_id, fnames.len()),
                    });
                }
                matching
            } else if let Some(ref f) = request.filename {
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

        // Prepend auxiliary files so they download first.
        // When an explicit file list (filenames) is used, apply scope-aware
        // auxiliary selection that includes non-weight LFS files and
        // directory-scoped configs.  Otherwise fall back to the basic
        // pattern-only selection.
        let mut aux_files = if request.filenames.is_some() {
            select_auxiliary_files_for_download(&tree.regular_files, &tree.lfs_files, &files)
        } else {
            let auxiliary = select_auxiliary_files(&tree.regular_files);
            auxiliary
                .into_iter()
                .map(|aux_filename| FileToDownload {
                    filename: aux_filename,
                    size: None,
                    sha256: None,
                })
                .collect()
        };
        if !aux_files.is_empty() {
            info!(
                "Including {} auxiliary file(s) for {}",
                aux_files.len(),
                request.repo_id
            );
        }
        aux_files.extend(files);
        let mut files = aux_files;

        // Allow additional downloads for the same repo directory by removing
        // filenames already tracked by queued/running/paused downloads.
        // If everything requested is already tracked, return that existing ID.
        let (tracked_filenames, tracked_download_id) = {
            let downloads = self.downloads.read().await;
            let mut tracked = HashSet::new();
            let mut tracked_id: Option<String> = None;

            for (id, state) in downloads.iter() {
                if state.dest_dir != dest_dir
                    || state.cancel_flag.load(Ordering::Relaxed)
                    || !matches!(
                        state.status,
                        DownloadStatus::Queued
                            | DownloadStatus::Downloading
                            | DownloadStatus::Pausing
                            | DownloadStatus::Paused
                            | DownloadStatus::Cancelling
                    )
                {
                    continue;
                }

                if tracked_id.is_none() {
                    tracked_id = Some(id.clone());
                }

                tracked.extend(state.files.iter().map(|f| f.filename.clone()));
            }

            (tracked, tracked_id)
        };

        if !tracked_filenames.is_empty() {
            let before = files.len();
            files.retain(|f| !tracked_filenames.contains(&f.filename));

            if files.is_empty() {
                if let Some(id) = tracked_download_id {
                    info!(
                        "Skipping duplicate download for {}: already tracked as {}",
                        request.repo_id, id
                    );
                    return Ok(id);
                }
            } else if before != files.len() {
                info!(
                    "Skipping {} already-tracked file(s) for {}",
                    before - files.len(),
                    request.repo_id
                );
            }
        }

        // Total bytes across all files (sum known sizes; auxiliary files
        // lack LFS size metadata but are small enough to not materially
        // affect progress accuracy)
        let total_bytes: Option<u64> = {
            let known_sum: u64 = files.iter().filter_map(|f| f.size).sum();
            if known_sum > 0 {
                Some(known_sum)
            } else {
                None
            }
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
            retry_attempt: 0,
            retry_limit: None,
            retrying: false,
            next_retry_delay_seconds: None,
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

        // Ensure destination exists so early metadata projection can be written
        // immediately at download start.
        std::fs::create_dir_all(dest_dir)?;

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

        // Fire early callback immediately so metadata is available in SQLite
        // as soon as the download is created (before file transfer begins).
        if let Some(ref callback) = self.aux_complete_callback {
            callback(AuxFilesCompleteInfo {
                download_id: download_id.clone(),
                dest_dir: dest_dir.to_path_buf(),
                filenames: files.iter().map(|f| f.filename.clone()).collect(),
                download_request: request.clone(),
                total_bytes,
            });
        }

        // Write marker file with repo_id so interrupted downloads can be recovered
        // even if downloads.json is lost (e.g. crash before persistence flush).
        let marker_path = dest_dir.join(".pumas_download");
        if let Err(e) = std::fs::write(
            &marker_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": request.repo_id,
                "family": request.family,
                "official_name": request.official_name,
                "model_type": request.model_type,
                "bundle_format": request.bundle_format,
                "pipeline_class": request.pipeline_class,
            }))
            .unwrap_or_default(),
        ) {
            info!("Failed to write download marker (non-fatal): {}", e);
        }

        info!(
            "Starting download {} for {} ({} file{})",
            download_id,
            request.repo_id,
            files.len(),
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
        let aux_complete_callback = self.aux_complete_callback.clone();
        let auth_header = self.auth_header_value().await;
        let dest_lock = self.destination_lock(&dest_dir).await;

        tokio::spawn(async move {
            // Serialize downloads targeting the same destination directory.
            let _destination_guard = dest_lock.lock().await;

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
                aux_complete_callback,
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
                        if let Some(entry) = entries
                            .iter_mut()
                            .find(|d| d.download_id == download_id_clone)
                        {
                            entry.status = DownloadStatus::Error;
                            let _ = persistence.save(entry);
                        }
                    }
                }
            }
        });

        Ok(download_id)
    }

    async fn destination_lock(&self, dest_dir: &Path) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.dest_locks.write().await;
        locks
            .entry(dest_dir.to_path_buf())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Run the download in the background with retry and resume support.
    ///
    /// Downloads all files sequentially. Files that already exist on disk
    /// (from a previous partial download) are skipped automatically.
    #[allow(clippy::too_many_arguments)]
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
        aux_complete_callback: Option<AuxFilesCompleteCallback>,
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

        let max_attempts = NetworkConfig::hf_download_max_retries();
        let retry_limit = retry_limit(max_attempts);
        let max_retry_elapsed = NetworkConfig::hf_download_max_retry_elapsed();
        let retry_config = RetryConfig::new()
            .with_max_attempts(max_attempts.max(1))
            .with_base_delay(NetworkConfig::HF_DOWNLOAD_RETRY_BASE_DELAY);

        // Download each file sequentially
        let mut bytes_offset: u64 = 0;
        let mut aux_callback_fired = false;

        for (file_idx, file_info) in files.iter().enumerate() {
            let filename = &file_info.filename;
            let dest_path = dest_dir.join(filename);
            let part_path = dest_dir.join(format!(
                "{}{}",
                filename,
                NetworkConfig::DOWNLOAD_TEMP_SUFFIX
            ));

            // Ensure parent directory exists (needed for subdirectory files
            // like transformer/model.safetensors in diffusion repos)
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

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

            // Fire aux-complete callback at the boundary between auxiliary and weight files.
            // Auxiliary files have size: None (non-LFS), weight files have size: Some (LFS).
            if !aux_callback_fired && file_info.size.is_some() {
                aux_callback_fired = true;
                if let Some(ref callback) = aux_complete_callback {
                    let info = {
                        let downloads = downloads.read().await;
                        downloads.get(download_id).and_then(|state| {
                            state
                                .download_request
                                .as_ref()
                                .map(|req| AuxFilesCompleteInfo {
                                    download_id: download_id.to_string(),
                                    dest_dir: state.dest_dir.clone(),
                                    filenames: files.iter().map(|f| f.filename.clone()).collect(),
                                    download_request: req.clone(),
                                    total_bytes: state.total_bytes,
                                })
                        })
                    };
                    if let Some(info) = info {
                        callback(info);
                    }
                }
            }

            // Update current filename in state
            {
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.filename = filename.clone();
                    state.retry_attempt = 0;
                    state.retry_limit = retry_limit;
                    state.retrying = false;
                    state.next_retry_delay_seconds = None;
                }
            }

            let url = format!("{}/{}/resolve/main/{}", HF_HUB_BASE, repo_id, filename);

            let mut last_error: Option<PumasError> = None;

            let mut file_completed = false;
            let mut attempt: u32 = 0;
            let retry_started = Instant::now();
            loop {
                attempt += 1;
                {
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.retry_attempt = attempt;
                        state.retry_limit = retry_limit;
                        state.retrying = false;
                        state.next_retry_delay_seconds = None;
                    }
                }

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

                if attempt > 1 {
                    warn!(
                        "Retry {}/{} for {}/{} (resuming from byte {})",
                        attempt,
                        retry_limit_display(retry_limit),
                        repo_id,
                        filename,
                        resume_from_byte
                    );

                    // Reset status to Downloading for the retry
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.status = DownloadStatus::Downloading;
                        state.error = None;
                        state.retry_attempt = attempt;
                        state.retry_limit = retry_limit;
                        state.retrying = false;
                        state.next_retry_delay_seconds = None;
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
                        tokio::fs::rename(&part_path, &dest_path)
                            .await
                            .map_err(|e| PumasError::DownloadFailed {
                                url: url.clone(),
                                message: format!("Failed to rename temp file: {}", e),
                            })?;

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
                            attempt,
                            retry_limit_display(retry_limit),
                            repo_id,
                            filename,
                            e
                        );
                        let error_text = e.to_string();
                        last_error = Some(e);

                        let elapsed = retry_started.elapsed();
                        if retry_exhausted(attempt, retry_limit, elapsed, max_retry_elapsed) {
                            break;
                        }

                        let delay = retry_config.calculate_delay(attempt.saturating_sub(1));
                        let limit_text = retry_limit_display(retry_limit);
                        let next_attempt = attempt + 1;
                        {
                            let mut downloads = downloads.write().await;
                            if let Some(state) = downloads.get_mut(download_id) {
                                state.retry_attempt = attempt;
                                state.retry_limit = retry_limit;
                                state.retrying = true;
                                state.next_retry_delay_seconds = Some(delay.as_secs_f64());
                                state.error = Some(format!(
                                    "Transient network error, retrying attempt {}/{} in {:.1}s: {}",
                                    next_attempt,
                                    limit_text,
                                    delay.as_secs_f64(),
                                    error_text
                                ));
                            }
                        }
                        debug!("Waiting {:?} before retry", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }

            if !file_completed {
                let elapsed = retry_started.elapsed();
                if let Some(last_error) = last_error {
                    let detail = retry_exhausted_message(
                        attempt,
                        retry_limit,
                        elapsed,
                        &last_error.to_string(),
                    );
                    return Err(PumasError::DownloadFailed {
                        url,
                        message: detail,
                    });
                }
                return Err(PumasError::DownloadFailed {
                    url,
                    message: "Download stopped before completion".to_string(),
                });
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
                    state.retry_attempt = 0;
                    state.retrying = false;
                    state.next_retry_delay_seconds = None;
                    state.error = None;
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
    #[allow(clippy::too_many_arguments)]
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
        let is_resuming = resume_from_byte > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
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
                    message: format!("Incomplete download: got {} of {} bytes", downloaded, total),
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
        downloads
            .get(download_id)
            .map(|state| ModelDownloadProgress {
                download_id: state.download_id.clone(),
                repo_id: Some(state.repo_id.clone()),
                model_name: state
                    .download_request
                    .as_ref()
                    .map(|request| request.official_name.clone()),
                model_type: state
                    .download_request
                    .as_ref()
                    .and_then(|request| request.model_type.clone()),
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
                retry_attempt: Some(state.retry_attempt),
                retry_limit: state.retry_limit,
                retrying: Some(state.retrying),
                next_retry_delay_seconds: state.next_retry_delay_seconds,
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
                model_name: state
                    .download_request
                    .as_ref()
                    .map(|request| request.official_name.clone()),
                model_type: state
                    .download_request
                    .as_ref()
                    .and_then(|request| request.model_type.clone()),
                status: state.status,
                progress: Some(state.progress),
                downloaded_bytes: Some(state.downloaded_bytes),
                total_bytes: state.total_bytes,
                speed: Some(state.speed),
                eta_seconds: if state.speed > 0.0 && state.total_bytes.is_some() {
                    let remaining = state
                        .total_bytes
                        .unwrap()
                        .saturating_sub(state.downloaded_bytes);
                    Some(remaining as f64 / state.speed)
                } else {
                    None
                },
                retry_attempt: Some(state.retry_attempt),
                retry_limit: state.retry_limit,
                retrying: Some(state.retrying),
                next_retry_delay_seconds: state.next_retry_delay_seconds,
                error: state.error.clone(),
            })
            .collect()
    }

    /// Find the download ID whose destination directory matches `dest_dir`.
    pub async fn find_download_id_by_dest_dir(&self, dest_dir: &Path) -> Option<String> {
        let downloads = self.downloads.read().await;
        downloads
            .values()
            .find(|state| state.dest_dir == dest_dir)
            .map(|state| state.download_id.clone())
    }

    /// Get the current in-memory status for a download ID.
    pub async fn get_download_status(&self, download_id: &str) -> Option<DownloadStatus> {
        let downloads = self.downloads.read().await;
        downloads.get(download_id).map(|state| state.status)
    }

    /// Relocate a tracked download destination directory.
    ///
    /// Updates both in-memory state and persisted download metadata so resume
    /// continues from the new path after migration/reclassification moves.
    pub async fn relocate_download_destination(
        &self,
        download_id: &str,
        new_dest_dir: &Path,
        new_model_type: Option<&str>,
        new_family: Option<&str>,
    ) -> Result<bool> {
        {
            let mut downloads = self.downloads.write().await;
            let Some(state) = downloads.get_mut(download_id) else {
                return Ok(false);
            };
            state.dest_dir = new_dest_dir.to_path_buf();
            if let Some(request) = state.download_request.as_mut() {
                if let Some(model_type) = new_model_type {
                    request.model_type = Some(model_type.to_string());
                }
                if let Some(family) = new_family {
                    request.family = family.to_string();
                }
            }
        }

        if let Some(ref persistence) = self.persistence {
            let mut persisted = persistence.load_all();
            if let Some(entry) = persisted
                .iter_mut()
                .find(|entry| entry.download_id == download_id)
            {
                entry.dest_dir = new_dest_dir.to_path_buf();
                if let Some(model_type) = new_model_type {
                    entry.download_request.model_type = Some(model_type.to_string());
                }
                if let Some(family) = new_family {
                    entry.download_request.family = family.to_string();
                }
                persistence.save(entry)?;
            }
        }

        Ok(true)
    }

    /// Pause an active download. Preserves the `.part` file for later resume.
    pub async fn pause_download(&self, download_id: &str) -> Result<bool> {
        let downloads = self.downloads.read().await;
        if let Some(state) = downloads.get(download_id) {
            if state.status == DownloadStatus::Downloading || state.status == DownloadStatus::Queued
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
        let aux_complete_callback = self.aux_complete_callback.clone();
        let auth_header = self.auth_header_value().await;
        let dest_lock = self.destination_lock(&dest_dir).await;

        tokio::spawn(async move {
            let _destination_guard = dest_lock.lock().await;

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
                aux_complete_callback,
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
                    Self::persist_status_update(
                        persistence,
                        &download_id_clone,
                        DownloadStatus::Error,
                    );
                }
            }
        });

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    #[test]
    fn test_retry_limit_zero_means_unlimited() {
        assert_eq!(retry_limit(0), None);
        assert_eq!(retry_limit(4), Some(4));
    }

    #[test]
    fn test_retry_exhausted_by_attempt_limit() {
        let exhausted = retry_exhausted(
            3,
            Some(3),
            Duration::from_secs(10),
            Duration::from_secs(120),
        );
        assert!(exhausted);
    }

    #[test]
    fn test_retry_exhausted_by_elapsed_budget() {
        let exhausted =
            retry_exhausted(2, None, Duration::from_secs(121), Duration::from_secs(120));
        assert!(exhausted);
    }

    #[tokio::test]
    async fn test_relocate_download_destination_updates_state_and_persistence() {
        let tmp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(tmp.path()).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(tmp.path()));
        client.set_persistence(persistence.clone());

        let download_id = "dl-relocate".to_string();
        let old_dest = tmp.path().join("old");
        let new_dest = tmp.path().join("new");
        std::fs::create_dir_all(&old_dest).unwrap();

        let request = DownloadRequest {
            repo_id: "owner/model".to_string(),
            family: "oldfam".to_string(),
            official_name: "Model".to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: None,
            pipeline_tag: Some("text-generation".to_string()),
            bundle_format: None,
            pipeline_class: None,
        };

        persistence
            .save(&PersistedDownload {
                download_id: download_id.clone(),
                repo_id: "owner/model".to_string(),
                filename: "model.safetensors".to_string(),
                filenames: vec!["model.safetensors".to_string()],
                dest_dir: old_dest.clone(),
                total_bytes: Some(1024),
                status: DownloadStatus::Paused,
                download_request: request.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
            })
            .unwrap();

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/model".to_string(),
                    status: DownloadStatus::Paused,
                    progress: 0.5,
                    downloaded_bytes: 512,
                    total_bytes: Some(1024),
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 0,
                    retry_limit: None,
                    retrying: false,
                    next_retry_delay_seconds: None,
                    dest_dir: old_dest.clone(),
                    filename: "model.safetensors".to_string(),
                    files: vec![FileToDownload {
                        filename: "model.safetensors".to_string(),
                        size: Some(1024),
                        sha256: None,
                    }],
                    files_completed: 0,
                    download_request: Some(request.clone()),
                    known_sha256: None,
                },
            );
        }

        assert!(client
            .relocate_download_destination(
                &download_id,
                &new_dest,
                Some("reranker"),
                Some("forturne"),
            )
            .await
            .unwrap());

        let status = client.get_download_status(&download_id).await;
        assert_eq!(status, Some(DownloadStatus::Paused));
        let found = client.find_download_id_by_dest_dir(&new_dest).await;
        assert_eq!(found.as_deref(), Some(download_id.as_str()));

        let entry = persistence
            .load_all()
            .into_iter()
            .find(|entry| entry.download_id == download_id)
            .unwrap();
        assert_eq!(entry.dest_dir, new_dest);
        assert_eq!(
            entry.download_request.model_type.as_deref(),
            Some("reranker")
        );
        assert_eq!(entry.download_request.family, "forturne");
    }

    #[tokio::test]
    async fn test_list_downloads_includes_model_type_and_name() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let download_id = "dl-progress".to_string();

        let request = DownloadRequest {
            repo_id: "owner/model".to_string(),
            family: "owner".to_string(),
            official_name: "Model Display Name".to_string(),
            model_type: Some("reranker".to_string()),
            quant: None,
            filename: None,
            filenames: None,
            pipeline_tag: Some("text-ranking".to_string()),
            bundle_format: None,
            pipeline_class: None,
        };

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/model".to_string(),
                    status: DownloadStatus::Paused,
                    progress: 0.25,
                    downloaded_bytes: 256,
                    total_bytes: Some(1024),
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 2,
                    retry_limit: Some(5),
                    retrying: true,
                    next_retry_delay_seconds: Some(4.0),
                    dest_dir: tmp.path().join("owner-model"),
                    filename: "model.safetensors".to_string(),
                    files: vec![FileToDownload {
                        filename: "model.safetensors".to_string(),
                        size: Some(1024),
                        sha256: None,
                    }],
                    files_completed: 0,
                    download_request: Some(request),
                    known_sha256: None,
                },
            );
        }

        let list = client.list_downloads().await;
        let progress = list
            .into_iter()
            .find(|item| item.download_id == download_id)
            .expect("download progress should be present");
        assert_eq!(progress.model_type.as_deref(), Some("reranker"));
        assert_eq!(progress.model_name.as_deref(), Some("Model Display Name"));
        assert_eq!(progress.retry_attempt, Some(2));
        assert_eq!(progress.retry_limit, Some(5));
        assert_eq!(progress.retrying, Some(true));
        assert_eq!(progress.next_retry_delay_seconds, Some(4.0));
    }

    #[tokio::test]
    async fn test_destination_lock_reuses_same_mutex_for_same_path() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let a = tmp.path().join("llm/owner/model");
        let b = tmp.path().join("llm/owner/model");
        let c = tmp.path().join("llm/owner/other-model");

        let lock_a = client.destination_lock(&a).await;
        let lock_b = client.destination_lock(&b).await;
        let lock_c = client.destination_lock(&c).await;

        assert!(Arc::ptr_eq(&lock_a, &lock_b));
        assert!(!Arc::ptr_eq(&lock_a, &lock_c));
    }
}
