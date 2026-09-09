//! Download management for HuggingFace models.
//!
//! Handles multi-file downloads with progress tracking, pause/resume,
//! cancellation, retry with resume, and crash recovery via persistence.

use super::lifecycle::{
    CancelPredecessor, DestinationDomain, DestinationExecutionOwner, DownloadTaskOwner,
    PreparedTask as OwnedPreparedTask, ProjectionOutcome, ProjectionSettlement,
    ProjectionTransition, TaskContext, TaskObservation, TaskRole, TaskTerminal,
};
use super::types::{
    AuxFilesCompleteCallback, AuxFilesCompleteInfo, DownloadCompletionCallback,
    DownloadCompletionInfo, DownloadDestination, DownloadState, FileToDownload, HF_HUB_BASE,
};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::download_store::{
    DownloadAdmissionDomain, DownloadAdmissionRequest, DownloadAdmissionTransition,
    DownloadPersistence, LifecycleCleanupDisposition, LifecycleQuarantine,
    LifecycleQuarantineDomain, PersistedDownload, PersistedDownloadInventory,
};
use crate::model_library::partial_download::{
    finalize_download_artifact_with_files, infer_expected_sizes_with_files,
};
use crate::model_library::sharding;
use crate::model_library::types::{DownloadRequest, DownloadStatus, ModelDownloadProgress};
use crate::model_library::SelectedArtifactIdentity;
use crate::model_library::{RecoveryDownloadAdmission, VerifiedDownloadRecovery};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex as TokioMutex, OwnedMutexGuard, RwLock};
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

const DOWNLOAD_UPDATE_CURSOR_PREFIX: &str = "download:";
const DOWNLOAD_PROGRESS_PUBLISH_INTERVAL: Duration = Duration::from_millis(500);
const DOWNLOAD_SHUTDOWN_INTERRUPTED: &str = "Download interrupted by library shutdown";

struct PendingDownloadPublication {
    notification: crate::models::ModelDownloadUpdateNotification,
    completed: tokio::sync::oneshot::Sender<()>,
}

#[derive(Default)]
struct DownloadPublicationState {
    queue: VecDeque<PendingDownloadPublication>,
    draining: bool,
}

#[cfg(test)]
type DownloadDispatchObserver = Arc<dyn Fn(&crate::models::ModelDownloadSnapshot) + Send + Sync>;

pub(crate) struct DownloadPublicationOwner {
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    revision: Arc<AtomicU64>,
    updates: broadcast::Sender<crate::models::ModelDownloadUpdateNotification>,
    capture: TokioMutex<()>,
    state: std::sync::Mutex<DownloadPublicationState>,
    #[cfg(test)]
    dispatch_observer: std::sync::Mutex<Option<DownloadDispatchObserver>>,
}

impl DownloadPublicationOwner {
    pub(super) fn new(
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        revision: Arc<AtomicU64>,
        updates: broadcast::Sender<crate::models::ModelDownloadUpdateNotification>,
    ) -> Self {
        Self {
            downloads,
            revision,
            updates,
            capture: TokioMutex::new(()),
            state: std::sync::Mutex::new(DownloadPublicationState::default()),
            #[cfg(test)]
            dispatch_observer: std::sync::Mutex::new(None),
        }
    }

    async fn publish(&self) {
        let (snapshot, completed, becomes_drainer) = {
            let capture = self.capture.lock().await;
            let next_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
            let snapshot = build_download_snapshot_from_parts(&self.downloads, next_revision).await;
            let notification = crate::models::ModelDownloadUpdateNotification {
                cursor: snapshot.cursor.clone(),
                snapshot: snapshot.clone(),
                stale_cursor: false,
                snapshot_required: false,
            };
            let (completed_sender, completed) = tokio::sync::oneshot::channel();
            let becomes_drainer = {
                let mut state = self
                    .state
                    .lock()
                    .expect("HF download-publication owner lock poisoned");
                state.queue.push_back(PendingDownloadPublication {
                    notification,
                    completed: completed_sender,
                });
                if state.draining {
                    false
                } else {
                    state.draining = true;
                    true
                }
            };
            drop(capture);
            (snapshot, completed, becomes_drainer)
        };

        #[cfg(test)]
        {
            let observer = self
                .dispatch_observer
                .lock()
                .expect("HF download-dispatch observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer(&snapshot);
            }
        }
        #[cfg(not(test))]
        let _ = snapshot;

        if becomes_drainer {
            self.drain();
        }
        let _ = completed.await;
    }

    fn drain(&self) {
        loop {
            let pending = {
                let mut state = self
                    .state
                    .lock()
                    .expect("HF download-publication owner lock poisoned");
                match state.queue.pop_front() {
                    Some(pending) => Some(pending),
                    None => {
                        state.draining = false;
                        None
                    }
                }
            };
            let Some(pending) = pending else {
                return;
            };
            let _ = self.updates.send(pending.notification);
            let _ = pending.completed.send(());
        }
    }

    #[cfg(test)]
    fn set_dispatch_observer_for_test(&self, observer: Option<DownloadDispatchObserver>) {
        *self
            .dispatch_observer
            .lock()
            .expect("HF download-dispatch observer lock poisoned") = observer;
    }
}

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

/// Resolve the exact producer-bound recovery set against the current remote
/// tree. Recovery never widens this set with repository auxiliary files and
/// never accepts a partial intersection.
fn resolve_exact_recovery_files(
    tree: &crate::model_library::RepoFileTree,
    bound_files: &[String],
) -> Option<Vec<FileToDownload>> {
    bound_files
        .iter()
        .map(|filename| {
            if let Some(file) = tree
                .lfs_files
                .iter()
                .find(|file| file.filename == *filename)
            {
                return Some(FileToDownload {
                    filename: filename.clone(),
                    size: Some(file.size),
                    sha256: Some(file.sha256.clone()),
                });
            }
            tree.regular_files
                .iter()
                .any(|file| file == filename)
                .then(|| FileToDownload {
                    filename: filename.clone(),
                    size: None,
                    sha256: None,
                })
        })
        .collect()
}

enum DownloadFile {
    Ambient(Arc<std::sync::Mutex<std::fs::File>>),
    Recovery(Arc<std::sync::Mutex<std::fs::File>>),
}

struct DownloadStartSetup {
    marker_contents: String,
}

fn serialize_download_marker(
    request: &DownloadRequest,
    selected_filenames: Vec<String>,
    evidence: Option<&crate::models::HuggingFaceEvidence>,
) -> Result<String> {
    let selected_artifact =
        SelectedArtifactIdentity::from_download_request(request, Some(selected_filenames));
    serde_json::to_string_pretty(&serde_json::json!({
        "repo_id": request.repo_id,
        "family": request.family,
        "official_name": request.official_name,
        "model_type": request.model_type,
        "pipeline_tag": request.pipeline_tag,
        "bundle_format": request.bundle_format,
        "pipeline_class": request.pipeline_class,
        "selected_artifact": selected_artifact,
        "huggingface_evidence": evidence,
    }))
    .map_err(|error| PumasError::Json {
        message: "failed to serialize download marker".into(),
        source: Some(error),
    })
}

#[derive(Clone)]
struct CancellationPersistence {
    store: Arc<DownloadPersistence>,
    download_id: String,
    domain: LifecycleQuarantineDomain,
    admission_attempt: Option<String>,
    revoked_snapshot: Option<PersistedDownload>,
}

fn validate_restore_inventory(inventory: &PersistedDownloadInventory) -> Result<()> {
    // Strict inventory binds Verified Recovery cleanup to durable revocation
    // and its exact retained admission. Only terminal settlement is permitted.
    if inventory.queue_admissions.iter().any(|(id, admission)| {
        admission.domain == DownloadAdmissionDomain::Recovery
            && !inventory.quarantines.get(id).is_some_and(|quarantine| {
                quarantine.domain == LifecycleQuarantineDomain::Recovery
                    && quarantine.disposition == LifecycleCleanupDisposition::Verified
            })
    }) {
        return Err(PumasError::Validation {
            field: "download_recovery".into(),
            message: "Active recovery custody requires explicit reconciliation before restore"
                .into(),
        });
    }
    if !inventory.hidden_admissions.is_empty()
        || inventory
            .quarantines
            .values()
            .any(|quarantine| quarantine.disposition != LifecycleCleanupDisposition::Verified)
    {
        return Err(PumasError::Other(
            "Download restore requires unresolved admission or quarantine reconciliation".into(),
        ));
    }
    Ok(())
}

impl CancellationPersistence {
    fn begin(&self, sticky_failure: bool) -> Result<Option<LifecycleQuarantine>> {
        let inventory = self.store.load_lifecycle_inventory_strict()?;
        let existing = inventory.quarantines.get(&self.download_id);
        // A cancelled recovery transition may have revoked the ordinary row
        // without promoting the runtime destination. Select the store domain
        // after draining that transition. Existence is not durable proof:
        // begin_lifecycle_quarantine still validates the tombstone below.
        let domain =
            if self.revoked_snapshot.is_some() && self.store.is_revoked(&self.download_id)? {
                LifecycleQuarantineDomain::Recovery
            } else {
                self.domain
            };
        if existing.is_some_and(|quarantine| quarantine.domain != domain) {
            return Err(PumasError::Validation {
                field: "download_cleanup".into(),
                message: "Cancellation quarantine domain changed".into(),
            });
        }
        let snapshot = existing
            .map(|quarantine| quarantine.snapshot.clone())
            .or_else(|| {
                inventory
                    .downloads
                    .into_iter()
                    .find(|snapshot| snapshot.download_id == self.download_id)
            })
            .or_else(|| self.revoked_snapshot.clone());
        let Some(snapshot) = snapshot else {
            if self.admission_attempt.is_some() {
                return Err(PumasError::Validation {
                    field: "download_cleanup".into(),
                    message: "Admitted cancellation requires a retained cleanup snapshot".into(),
                });
            }
            return Ok(None);
        };
        let sticky_failure =
            sticky_failure || existing.is_some_and(|quarantine| quarantine.sticky_failure);
        // The inventory projection can hide terminal publication phases.
        // Only the store's confirmed result decides whether cleanup repeats.
        self.store
            .begin_lifecycle_quarantine(
                &snapshot,
                domain,
                sticky_failure,
                self.admission_attempt.as_deref(),
            )
            .map(Some)
    }

    fn mark_failed(&self) -> Result<()> {
        if self
            .store
            .mark_lifecycle_quarantine_failed(&self.download_id)?
        {
            Ok(())
        } else {
            Err(PumasError::Other(
                "Cancellation quarantine failure was not recorded".into(),
            ))
        }
    }

    fn finish(&self, quarantine: Option<&LifecycleQuarantine>) -> Result<()> {
        if let Some(quarantine) = quarantine {
            if quarantine.sticky_failure
                && !self.store.verify_lifecycle_quarantine(&self.download_id)?
            {
                return Err(PumasError::Other(
                    "Cancellation quarantine cleanup was not verified".into(),
                ));
            }
            if let Some(attempt) = self.admission_attempt.as_ref() {
                if !self
                    .store
                    .settle_queue_admission(&self.download_id, attempt)?
                {
                    return Err(PumasError::Other(
                        "Download queue settlement was not confirmed".into(),
                    ));
                }
            } else if !quarantine.sticky_failure
                && !self
                    .store
                    .remove_clean_lifecycle_quarantine(&self.download_id)?
            {
                return Err(PumasError::Other(
                    "Clean cancellation quarantine was not removed".into(),
                ));
            }
            return Ok(());
        }
        if let Some(attempt) = self.admission_attempt.as_ref() {
            if !self
                .store
                .settle_queue_admission(&self.download_id, attempt)?
            {
                return Err(PumasError::Other(
                    "Download queue settlement was not confirmed".into(),
                ));
            }
        }
        Ok(())
    }
}

struct PreparedDownloadTask {
    #[cfg(test)]
    download_base_url: Option<String>,
    client: reqwest::Client,
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    download_publications: Arc<DownloadPublicationOwner>,
    destination_executions: Arc<DestinationExecutionOwner>,
    download_id: String,
    repo_id: String,
    files: Vec<FileToDownload>,
    destination: DownloadDestination,
    configured_root: Option<crate::model_library::download_recovery::DownloadDestinationRoot>,
    cancel_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    completion_callback: Option<DownloadCompletionCallback>,
    aux_complete_callback: Option<AuxFilesCompleteCallback>,
    download_importer: Option<Arc<crate::model_library::ModelImporter>>,
    /// Status updates are intentionally disabled for verified recovery work.
    persistence: Option<Arc<DownloadPersistence>>,
    /// Terminal cleanup remains required for both ambient and recovery work.
    terminal_cleanup_persistence: Option<Arc<DownloadPersistence>>,
    auth_header: Option<String>,
    destination_lock: Arc<TokioMutex<()>>,
    start_setup: Option<DownloadStartSetup>,
    persist_queued_status: bool,
    restore_finalization: Option<DownloadStatus>,
}

enum RestoredFinalizationError {
    Operation(PumasError),
    Import(PumasError),
}

impl From<PumasError> for RestoredFinalizationError {
    fn from(error: PumasError) -> Self {
        Self::Operation(error)
    }
}

fn download_completion_info(state: &DownloadState) -> Option<DownloadCompletionInfo> {
    state
        .download_request
        .as_ref()
        .map(|request| DownloadCompletionInfo {
            download_id: state.download_id.clone(),
            dest_dir: state.dest_dir.clone(),
            filename: state
                .files
                .iter()
                .max_by_key(|file| file.size.unwrap_or(0))
                .map(|file| file.filename.clone())
                .unwrap_or_else(|| state.filename.clone()),
            filenames: state
                .files
                .iter()
                .map(|file| file.filename.clone())
                .collect(),
            download_request: request.clone(),
            known_sha256: state.known_sha256.clone(),
            huggingface_evidence: state.huggingface_evidence.clone(),
        })
}

async fn import_completed_download(
    importer: &Option<Arc<crate::model_library::ModelImporter>>,
    context: &TaskContext,
    info: Option<DownloadCompletionInfo>,
) -> Result<()> {
    let Some(importer) = importer.clone() else {
        return Ok(());
    };
    let info = info.ok_or_else(|| PumasError::Config {
        message: "Download import requires completion metadata".into(),
    })?;
    context
        .run_fallible_async_named("finalize downloaded model import", move || async move {
            importer
                .finalize_downloaded_directory(&info)
                .await
                .map(|_| ())
        })
        .await
        .map_err(|error| {
            PumasError::Other(format!("Download import observation failed: {error}"))
        })?
}

impl PreparedDownloadTask {
    async fn finalize_restored(
        &self,
        context: &TaskContext,
        initial_status: DownloadStatus,
    ) -> std::result::Result<(), RestoredFinalizationError> {
        let mut destination_guard = Some(self.destination_lock.clone().lock_owned().await);
        let (attempt, total_bytes) = {
            let mut states = self.downloads.write().await;
            let state = current_worker_state(
                &mut states,
                &self.download_id,
                context,
                &[DownloadStatus::Queued, DownloadStatus::Downloading],
            )?;
            let attempt = state
                .admission
                .as_ref()
                .ok_or_else(|| PumasError::Config {
                    message: "Restore finalization requires durable admission".into(),
                })?
                .attempt_id
                .clone();
            state.status = DownloadStatus::Downloading;
            (attempt, state.total_bytes)
        };
        let complete = context
            .run_fallible_blocking_named("finalize restored download files", {
                let destination = self.destination.capability().clone();
                let filenames = self
                    .files
                    .iter()
                    .map(|file| file.filename.clone())
                    .collect::<Vec<_>>();
                move || -> Result<bool> {
                    let sizes =
                        infer_expected_sizes_with_files(&destination, &filenames, total_bytes)?;
                    Ok(
                        finalize_download_artifact_with_files(&destination, &filenames, &sizes)?
                            .complete,
                    )
                }
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Restore finalization owner failed: {error}"))
            })??;
        if !matches!(context.drain_blocking().await, Ok(0)) {
            return Err(
                PumasError::Other("Restore finalization effects did not drain".into()).into(),
            );
        }
        {
            let mut states = self.downloads.write().await;
            let state = current_worker_state(
                &mut states,
                &self.download_id,
                context,
                &[DownloadStatus::Downloading],
            )?;
            if !complete {
                state.status = initial_status;
                state.task_registered = false;
                return Err(PumasError::DownloadPaused.into());
            }
            // The file set is committed. A later pause must not interrupt its
            // exact durable settlement; cancellation still owns replacement.
            state.files_completed = state.files.len();
        }
        let info = self
            .downloads
            .read()
            .await
            .get(&self.download_id)
            .and_then(download_completion_info);
        drop(destination_guard.take());
        import_completed_download(&self.download_importer, context, info)
            .await
            .map_err(RestoredFinalizationError::Import)?;
        destination_guard = Some(self.destination_lock.clone().lock_owned().await);
        {
            let mut states = self.downloads.write().await;
            current_worker_state(
                &mut states,
                &self.download_id,
                context,
                &[DownloadStatus::Downloading],
            )?;
        }
        let persistence = self.persistence.clone().ok_or_else(|| PumasError::Config {
            message: "Restore finalization persistence is unavailable".into(),
        })?;
        let id = self.download_id.clone();
        let settled = context
            .run_fallible_blocking_named("settle restored download admission", move || {
                persistence.settle_queue_admission(&id, &attempt)
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Restore settlement owner failed: {error}"))
            })??;
        if !settled || !matches!(context.drain_blocking().await, Ok(0)) {
            return Err(PumasError::Other(
                "Restore finalization settlement was not confirmed".into(),
            )
            .into());
        }
        {
            let mut states = self.downloads.write().await;
            let state = current_worker_state(
                &mut states,
                &self.download_id,
                context,
                &[DownloadStatus::Downloading],
            )?;
            state.status = DownloadStatus::Completed;
            state.progress = 1.0;
            state.task_registered = false;
            state.speed = 0.0;
        }
        drop(destination_guard);
        publish_download_snapshot_from_parts(&self.download_publications).await;
        Ok(())
    }

    fn prepare_owned(
        self,
        owner: &Arc<DownloadTaskOwner>,
        role: TaskRole,
        protected_context: Option<TaskContext>,
    ) -> Result<OwnedPreparedTask> {
        let download_id = self.download_id.clone();
        owner.prepare(download_id, role, move |task_context| async move {
            let task_context = protected_context.as_ref().map_or_else(
                || task_context.clone(),
                |source| task_context.inherit_root_grant(source),
            );
            drop(protected_context);
            // This worker owns and projects its failures; ordinary starts do
            // not have a second consumer for the settled internal outcome.
            let _ = self.run_owned(task_context).await;
        })
    }

    async fn run_owned(self, mut task_context: TaskContext) -> Result<bool> {
        use futures::FutureExt;

        let destination_path = self.destination.identity();
        let destination_domain = self.destination.domain();
        let waiting_context = task_context.without_root_grant();
        let acquired = {
            let wait = async {
                tokio::select! {
                    biased;
                    _ = waiting_context.pause_requested(&self.pause_flag) => None,
                    acquired = self.destination_executions.wait_for_turn(
                        &destination_path,
                        waiting_context.download_id(),
                        destination_domain,
                        waiting_context.generation(),
                    ) => Some(acquired),
                }
            };
            tokio::pin!(wait);
            match futures::poll!(&mut wait) {
                std::task::Poll::Ready(acquired) => acquired,
                std::task::Poll::Pending => {
                    // Only an actual queue wait relinquishes admission custody.
                    task_context = task_context.without_root_grant();
                    wait.await
                }
            }
        };
        if acquired == Some(false) {
            return Ok(false);
        }
        let protected = async {
            if let Some(root) = self.configured_root.clone() {
                task_context = task_context.with_root_grant(root).await?;
            }
            task_context = task_context
                .with_root_grant(self.destination.capability().execution_root())
                .await?;
            if acquired == Some(true) {
                self.validate_execution(&task_context).await?;
            }
            Ok::<_, PumasError>(())
        }
        .await;
        if let Err(error) = protected {
            HuggingFaceClient::project_execution_refusal(
                &self.downloads,
                &self.download_publications,
                &self.download_id,
                &task_context,
                TaskRole::Worker,
                &error,
            )
            .await;
            return Err(error);
        }
        let mut restore_import_failed = false;
        let outcome =
            if let (Some(true), Some(initial_status)) = (acquired, self.restore_finalization) {
                match AssertUnwindSafe(self.finalize_restored(&task_context, initial_status))
                    .catch_unwind()
                    .await
                {
                    Ok(Err(RestoredFinalizationError::Import(error))) => {
                        restore_import_failed = true;
                        Ok(Err(error))
                    }
                    Ok(result) => Ok(result.map_err(|error| match error {
                        RestoredFinalizationError::Operation(error)
                        | RestoredFinalizationError::Import(error) => error,
                    })),
                    Err(panic) => Err(panic),
                }
            } else if acquired == Some(true) {
                AssertUnwindSafe(HuggingFaceClient::run_download(
                    self.client,
                    self.downloads.clone(),
                    self.download_publications.clone(),
                    &self.download_id,
                    &self.repo_id,
                    &self.files,
                    &self.destination,
                    self.cancel_flag,
                    self.pause_flag,
                    self.persistence.clone(),
                    self.terminal_cleanup_persistence.clone(),
                    self.aux_complete_callback,
                    self.download_importer,
                    self.auth_header,
                    task_context.clone(),
                    self.destination_lock,
                    self.start_setup,
                    self.persist_queued_status,
                    #[cfg(test)]
                    self.download_base_url,
                ))
                .catch_unwind()
                .await
            } else {
                Ok(Err(if acquired.is_none() {
                    PumasError::DownloadPaused
                } else {
                    PumasError::DownloadCancelled
                }))
            };
        let mut result = match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        };
        if matches!(result, Err(PumasError::DownloadPaused)) {
            let pause_still_needs_settlement = {
                let states = self.downloads.read().await;
                states
                    .get(&self.download_id)
                    .is_some_and(|state| state.status == DownloadStatus::Pausing)
                    && task_context.is_current_role(TaskRole::Worker)
            };
            if pause_still_needs_settlement {
                let mut no_destination_guard = None;
                result = HuggingFaceClient::settle_worker_pause(
                    &self.downloads,
                    &self.download_publications,
                    &self.download_id,
                    &task_context,
                    self.persistence.as_ref(),
                    &mut no_destination_guard,
                )
                .await;
            }
        }
        // Paused and failed downloads retain destination authority for their
        // resumable artifacts. Only verified successful completion releases
        // the reservation; cancellation transfers it to its finalizer.
        if acquired == Some(true) && result.is_ok() {
            let completion_info = if self.completion_callback.is_some() {
                self.downloads
                    .read()
                    .await
                    .get(&self.download_id)
                    .and_then(download_completion_info)
            } else {
                None
            };
            self.destination_executions.release(
                &destination_path,
                task_context.download_id(),
                destination_domain,
                task_context.generation(),
            );
            task_context = task_context.without_root_grant();
            if let (Some(callback), Some(info)) =
                (self.completion_callback.clone(), completion_info)
            {
                let notification = task_context
                    .run_fallible_blocking_named("invoke download-completion callback", move || {
                        std::panic::catch_unwind(AssertUnwindSafe(|| callback(info)))
                            .map_err(|_| "download completion callback panicked".to_string())
                    })
                    .await;
                if !matches!(notification, Ok(Ok(()))) {
                    warn!("Download completion notification failed after terminal settlement: {notification:?}");
                }
            }
        }

        // Terminal Error persistence is registered before the final owner
        // drain. No nested effect may be created after that drain or after the
        // terminal snapshot is published.
        let persist_error_ok = if result.as_ref().is_err_and(|error| {
            !matches!(
                error,
                PumasError::DownloadPaused | PumasError::DownloadCancelled
            )
        }) {
            if let Some(ref persistence) = self.persistence {
                let admission_attempt = self
                    .downloads
                    .read()
                    .await
                    .get(&self.download_id)
                    .and_then(|state| {
                        state
                            .admission
                            .as_ref()
                            .map(|admission| admission.attempt_id.clone())
                    });
                matches!(
                    HuggingFaceClient::persist_status_update_owned(
                        &task_context,
                        persistence.clone(),
                        self.download_id.clone(),
                        DownloadStatus::Error,
                        admission_attempt,
                    )
                    .await,
                    Ok(true)
                )
            } else {
                true
            }
        } else {
            true
        };
        let nested_failures = task_context.drain_blocking().await.unwrap_or(1);

        let mut error_projected = false;
        if let Err(error) = &result {
            if matches!(error, PumasError::DownloadPaused) {
                info!("Download paused for {}", self.repo_id);
                return Ok(false);
            }
            if matches!(error, PumasError::DownloadCancelled) {
                info!("Download cancelled for {}", self.repo_id);
                return Ok(false);
            }
            error!("Download failed for {}: {}", self.repo_id, error);
            let projected = {
                let mut download_states = self.downloads.write().await;
                current_worker_state(
                    &mut download_states,
                    &self.download_id,
                    &task_context,
                    &[
                        DownloadStatus::Queued,
                        DownloadStatus::Downloading,
                        DownloadStatus::Pausing,
                    ],
                )
                .map(|state| {
                    state.status = DownloadStatus::Error;
                    state.error = Some(error.to_string());
                    state.task_registered = false;
                    if !persist_error_ok || nested_failures > 0 {
                        state.lifecycle_failure_unverified = true;
                    }
                })
                .is_ok()
            };
            if projected {
                publish_download_snapshot_from_parts(&self.download_publications).await;
            }
            error_projected = projected;
        }
        if nested_failures > 0 {
            error!(
                "Download task for {} observed {} nested blocking failure(s)",
                self.repo_id, nested_failures
            );
            if result.is_ok() {
                return Err(PumasError::Other(
                    "Download completion effects did not drain".into(),
                ));
            }
        }
        if restore_import_failed && persist_error_ok && error_projected {
            return Ok(false);
        }
        result.map(|()| true)
    }

    async fn validate_execution(&self, context: &TaskContext) -> Result<()> {
        let Some(store) = self.terminal_cleanup_persistence.clone() else {
            return Ok(());
        };
        let attempt = self
            .downloads
            .read()
            .await
            .get(&self.download_id)
            .and_then(|state| {
                state
                    .admission
                    .as_ref()
                    .map(|entry| entry.attempt_id.clone())
            });
        // Newly ticket-authorized recovery never acquired an ordinary durable
        // admission. Its bound destination capability remains its authority.
        if attempt.is_none() && self.destination.is_recovery() {
            return Ok(());
        }
        let attempt = attempt.ok_or_else(|| PumasError::Config {
            message: "Download execution requires its retained admission attempt".into(),
        })?;
        let destination = self.destination.clone();
        let download_id = self.download_id.clone();
        let files = self
            .files
            .iter()
            .map(|file| file.filename.clone())
            .collect::<Vec<_>>();
        context
            .run_fallible_blocking_named("validate current download execution", move || {
                store.validate_queue_execution(
                    &download_id,
                    &attempt,
                    if destination.is_recovery() {
                        DownloadAdmissionDomain::Recovery
                    } else {
                        DownloadAdmissionDomain::Ambient
                    },
                    &destination.persisted_identity()?,
                    &files,
                )
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!(
                    "Download execution validation observation failed: {error}"
                ))
            })?
    }
}

impl DownloadDestination {
    async fn prepare(&self, task_context: &TaskContext) -> Result<()> {
        match self {
            Self::Managed(destination) => {
                let destination = destination.clone();
                recovery_filesystem_operation(
                    task_context,
                    "prepare ambient destination",
                    move || destination.prepare(),
                )
                .await
            }
            Self::Recovery(destination) => {
                let destination = destination.clone();
                recovery_filesystem_operation(task_context, "preflight", move || {
                    destination.preflight(&[])
                })
                .await
            }
        }
    }

    async fn prepare_file(&self, task_context: &TaskContext, filename: &str) -> Result<()> {
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                recovery_filesystem_operation(task_context, "create file parent", move || {
                    destination.create_parent(&filename)
                })
                .await
            }
        }
    }

    async fn file_len(&self, task_context: &TaskContext, filename: &str) -> Result<Option<u64>> {
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                recovery_filesystem_operation(task_context, "inspect download file", move || {
                    destination.file_len(&filename)
                })
                .await
            }
        }
    }

    async fn part_len(&self, task_context: &TaskContext, filename: &str) -> Result<Option<u64>> {
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                recovery_filesystem_operation(
                    task_context,
                    "inspect partial download file",
                    move || destination.part_len(&filename),
                )
                .await
            }
        }
    }

    async fn remove_part(&self, task_context: &TaskContext, filename: &str) -> Result<()> {
        let operation = if self.is_recovery() {
            "remove partial download file"
        } else {
            "remove ambient partial download file"
        };
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                #[cfg(test)]
                let inject_failure = task_context.should_fail_blocking_operation(operation);
                recovery_filesystem_operation(task_context, operation, move || {
                    #[cfg(test)]
                    if inject_failure {
                        return Err(std::io::Error::other(
                            "injected recovery partial cleanup failure",
                        ));
                    }
                    destination.remove_part(&filename)
                })
                .await
            }
        }
    }

    async fn finalize_complete_part_file(
        &self,
        task_context: &TaskContext,
        filename: &str,
        expected_size: Option<u64>,
    ) -> Result<bool> {
        let Some(expected_size) = expected_size else {
            return Ok(false);
        };
        if self.part_len(task_context, filename).await? != Some(expected_size) {
            return Ok(false);
        }
        self.rename_part_to_file(task_context, filename).await?;
        info!(
            "Finalized fully downloaded partial file {} ({} bytes)",
            filename, expected_size
        );
        Ok(true)
    }

    async fn rename_part_to_file(&self, task_context: &TaskContext, filename: &str) -> Result<()> {
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                recovery_filesystem_operation(
                    task_context,
                    "promote partial download file",
                    move || destination.rename_part_to_file(&filename),
                )
                .await
            }
        }
    }

    async fn open_part(
        &self,
        task_context: &TaskContext,
        filename: &str,
        append: bool,
    ) -> Result<DownloadFile> {
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                let filename = filename.to_string();
                let file = recovery_filesystem_operation(
                    task_context,
                    "open partial download file",
                    move || destination.open_part(&filename, append),
                )
                .await?;
                let file = Arc::new(std::sync::Mutex::new(file));
                Ok(if self.is_recovery() {
                    DownloadFile::Recovery(file)
                } else {
                    DownloadFile::Ambient(file)
                })
            }
        }
    }

    async fn remove_marker(&self, task_context: &TaskContext) -> Result<()> {
        let operation = if matches!(self, Self::Managed(_)) {
            "remove ambient download marker"
        } else {
            "remove download marker"
        };
        match self {
            Self::Recovery(destination) | Self::Managed(destination) => {
                let destination = destination.clone();
                recovery_filesystem_operation(task_context, operation, move || {
                    destination.remove_marker()
                })
                .await
            }
        }
    }

    async fn write_marker(&self, task_context: &TaskContext, contents: String) -> Result<()> {
        match self {
            Self::Managed(destination) => {
                let destination = destination.clone();
                let marker =
                    serde_json::from_str::<serde_json::Value>(&contents).map_err(|error| {
                        PumasError::Json {
                            message: "Invalid download marker".into(),
                            source: Some(error),
                        }
                    })?;
                match task_context
                    .run_fallible_blocking_named("write managed download marker", move || {
                        destination.write_marker(&marker)
                    })
                    .await
                {
                    Ok(Ok(crate::metadata::AtomicPublication::Durable)) => Ok(()),
                    Ok(Ok(crate::metadata::AtomicPublication::PublishedDurabilityUnknown {
                        error,
                    }))
                    | Ok(Ok(crate::metadata::AtomicPublication::VisibilityUnknown {
                        error, ..
                    })) => Err(error),
                    Ok(Err(failure)) => Err(failure.into_error()),
                    Err(error) => Err(PumasError::Other(format!(
                        "Download marker owner failed: {error}"
                    ))),
                }
            }
            Self::Recovery(_) => Err(PumasError::Other(
                "recovery work cannot create ambient download authority".to_string(),
            )),
        }
    }
}

impl DownloadFile {
    async fn write_all(&mut self, task_context: &TaskContext, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Ambient(file) => {
                let file = file.clone();
                let bytes = bytes.to_vec();
                recovery_filesystem_operation(
                    task_context,
                    "write ambient partial download file",
                    move || {
                        let mut file = file
                            .lock()
                            .map_err(|_| std::io::Error::other("ambient file lock was poisoned"))?;
                        std::io::Write::write_all(&mut *file, &bytes)
                    },
                )
                .await
            }
            Self::Recovery(file) => {
                let file = file.clone();
                let bytes = bytes.to_vec();
                recovery_filesystem_operation(
                    task_context,
                    "write partial download file",
                    move || {
                        let mut file = file.lock().map_err(|_| {
                            std::io::Error::other("recovery file lock was poisoned")
                        })?;
                        std::io::Write::write_all(&mut *file, &bytes)
                    },
                )
                .await
            }
        }
    }

    async fn flush(&mut self, task_context: &TaskContext) -> Result<()> {
        match self {
            Self::Ambient(file) => {
                let file = file.clone();
                recovery_filesystem_operation(
                    task_context,
                    "flush ambient partial download file",
                    move || {
                        let mut file = file
                            .lock()
                            .map_err(|_| std::io::Error::other("ambient file lock was poisoned"))?;
                        std::io::Write::flush(&mut *file)
                    },
                )
                .await
            }
            Self::Recovery(file) => {
                let file = file.clone();
                recovery_filesystem_operation(
                    task_context,
                    "flush partial download file",
                    move || {
                        let mut file = file.lock().map_err(|_| {
                            std::io::Error::other("recovery file lock was poisoned")
                        })?;
                        std::io::Write::flush(&mut *file)
                    },
                )
                .await
            }
        }
    }
}

async fn recovery_filesystem_operation<T, F>(
    task_context: &TaskContext,
    operation: &'static str,
    function: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> std::io::Result<T> + Send + 'static,
{
    task_context
        .run_fallible_blocking_named(operation, function)
        .await
        .map_err(|error| {
            PumasError::Other(format!(
                "download recovery filesystem capability task failed during {operation}: {error}"
            ))
        })?
        .map_err(|error| {
            PumasError::Other(format!(
                "download recovery filesystem capability unavailable during {operation}: {error}"
            ))
        })
}

enum RecoveryContext {
    Exact { download_id: String },
    Mismatch,
    Missing,
}

enum RecoveryLaunchPlan {
    Existing { download_id: String },
    New { download_id: String },
}

impl RecoveryLaunchPlan {
    fn download_id(&self) -> &str {
        match self {
            Self::Existing { download_id } | Self::New { download_id } => download_id,
        }
    }
}

fn recovery_selection_matches(
    state: &DownloadState,
    repo_id: &str,
    expected_files: &BTreeSet<&str>,
) -> bool {
    let tracked_selection = state.download_request.as_ref().map(|request| {
        let selected_filenames = state
            .huggingface_evidence
            .as_ref()
            .and_then(|evidence| evidence.selected_filenames.clone());
        SelectedArtifactIdentity::from_download_request(request, selected_filenames)
            .selected_filenames
    });
    let tracked_files: BTreeSet<&str> = tracked_selection
        .as_ref()
        .filter(|files| !files.is_empty())
        .map(|files| files.iter().map(String::as_str).collect())
        .unwrap_or_else(|| {
            state
                .files
                .iter()
                .map(|file| file.filename.as_str())
                .collect()
        });
    state.repo_id == repo_id && tracked_files == *expected_files
}

fn recovery_context(
    downloads: &HashMap<String, DownloadState>,
    identity: &crate::model_library::download_recovery::DestinationIdentity,
    repo_id: &str,
    files: &[String],
) -> RecoveryContext {
    let expected_files: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let mut exact = None;
    let mut mismatch = false;

    for state in downloads
        .values()
        .filter(|state| state.matches_destination(identity))
    {
        if recovery_selection_matches(state, repo_id, &expected_files) && exact.is_none() {
            exact = Some(state.download_id.clone());
        } else {
            mismatch = true;
        }
    }

    if mismatch {
        RecoveryContext::Mismatch
    } else if let Some(download_id) = exact {
        RecoveryContext::Exact { download_id }
    } else {
        RecoveryContext::Missing
    }
}

fn admitted_existing_recovery(
    state: &DownloadState,
    task: Option<&super::lifecycle::TaskSnapshot>,
) -> Option<RecoveryDownloadAdmission> {
    let download_id = state.download_id.clone();
    match state.status {
        DownloadStatus::Queued
        | DownloadStatus::Downloading
        | DownloadStatus::Pausing
        | DownloadStatus::Cancelling
            if state.recovery_destination().is_some()
                && state.task_registered
                && task.is_some_and(|task| {
                    task.started
                        && !task.finished
                        && matches!(task.role, TaskRole::Worker | TaskRole::CancelFinalizer)
                }) =>
        {
            Some(RecoveryDownloadAdmission::Attached {
                download_id,
                status: state.status,
            })
        }
        DownloadStatus::Completed => {
            Some(RecoveryDownloadAdmission::AlreadyCompleted { download_id })
        }
        DownloadStatus::Cancelled => {
            Some(RecoveryDownloadAdmission::AlreadyCancelled { download_id })
        }
        _ => None,
    }
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

fn selected_artifact_id_for_state(state: &DownloadState) -> Option<String> {
    let request = state.download_request.as_ref()?;
    let request_has_explicit_file_scope =
        request.filename.is_some() || request.filenames.is_some() || request.quant.is_some();
    let selected_filenames = if state.files.is_empty() || !request_has_explicit_file_scope {
        None
    } else {
        Some(
            state
                .files
                .iter()
                .map(|file| file.filename.clone())
                .collect(),
        )
    };

    Some(SelectedArtifactIdentity::from_download_request(request, selected_filenames).artifact_id)
}

fn download_update_cursor(revision: u64) -> String {
    format!("{DOWNLOAD_UPDATE_CURSOR_PREFIX}{revision}")
}

fn parse_download_update_cursor(cursor: &str) -> Option<u64> {
    cursor
        .strip_prefix(DOWNLOAD_UPDATE_CURSOR_PREFIX)
        .and_then(|value| value.parse::<u64>().ok())
}

fn progress_from_state(state: &DownloadState) -> ModelDownloadProgress {
    ModelDownloadProgress {
        download_id: state.download_id.clone(),
        library_model_id: state
            .destination
            .as_ref()
            .map(|destination| destination.capability().library_model_id()),
        repo_id: Some(state.repo_id.clone()),
        selected_artifact_id: selected_artifact_id_for_state(state),
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
    }
}

async fn build_download_snapshot_from_parts(
    downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
    revision: u64,
) -> crate::models::ModelDownloadSnapshot {
    let downloads = downloads.read().await;
    let mut progresses = downloads
        .values()
        .map(progress_from_state)
        .collect::<Vec<_>>();
    progresses.sort_by(|left, right| left.download_id.cmp(&right.download_id));

    crate::models::ModelDownloadSnapshot {
        cursor: download_update_cursor(revision),
        revision,
        downloads: progresses,
    }
}

async fn publish_download_snapshot_from_parts(publications: &Arc<DownloadPublicationOwner>) {
    publications.publish().await;
}

#[allow(clippy::too_many_arguments)]
async fn publish_worker_snapshot_and_revalidate(
    publications: &Arc<DownloadPublicationOwner>,
    downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
    download_id: &str,
    task_context: &TaskContext,
    destination: &DownloadDestination,
    destination_lock: &Arc<TokioMutex<()>>,
    destination_guard: &mut Option<OwnedMutexGuard<()>>,
    expected_statuses: &[DownloadStatus],
) -> Result<()> {
    drop(destination_guard.take());
    publish_download_snapshot_from_parts(publications).await;
    *destination_guard = Some(destination_lock.clone().lock_owned().await);

    let revalidated = {
        let mut states = downloads.write().await;
        current_worker_state(&mut states, download_id, task_context, expected_statuses).map(
            |state| {
                !state.cancel_flag.load(Ordering::Relaxed)
                    && state.matches_destination(&destination.identity())
            },
        )
    };
    match revalidated {
        Ok(true) => Ok(()),
        Ok(false) | Err(PumasError::DownloadCancelled) => Err(PumasError::DownloadCancelled),
        Err(error) => Err(error),
    }
}

/// Returns mutable state only while this exact task generation still owns the
/// Worker role and the state is in one of the caller's expected phases.
///
/// Callers hold the download-state write lock while this checks the task
/// owner. Cancellation and replacement take those locks in the same order, so
/// a successful check and the immediately following state projection are one
/// atomic ownership decision.
fn current_worker_state<'a>(
    states: &'a mut HashMap<String, DownloadState>,
    download_id: &str,
    task_context: &TaskContext,
    expected: &[DownloadStatus],
) -> Result<&'a mut DownloadState> {
    if !task_context.is_current_role(TaskRole::Worker) {
        return Err(PumasError::DownloadCancelled);
    }
    let state = states
        .get_mut(download_id)
        .ok_or(PumasError::DownloadCancelled)?;
    if !expected.contains(&state.status) {
        if state.status == DownloadStatus::Pausing
            && expected.contains(&DownloadStatus::Downloading)
        {
            return Err(PumasError::DownloadPaused);
        }
        return Err(PumasError::DownloadCancelled);
    }
    Ok(state)
}

async fn project_worker_retry_reset(
    downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
    download_id: &str,
    task_context: &TaskContext,
    attempt: u32,
    retry_limit: Option<u32>,
) -> Result<()> {
    #[cfg(test)]
    task_context.observe_worker_projection("retry-reset");
    let mut states = downloads.write().await;
    let state = current_worker_state(
        &mut states,
        download_id,
        task_context,
        &[DownloadStatus::Downloading],
    )?;
    state.status = DownloadStatus::Downloading;
    state.error = None;
    state.retry_attempt = attempt;
    state.retry_limit = retry_limit;
    state.retrying = false;
    state.next_retry_delay_seconds = None;
    Ok(())
}

pub(super) async fn project_download_shutdown(
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    publications: Arc<DownloadPublicationOwner>,
) -> Result<()> {
    {
        let mut states = downloads.write().await;
        for state in states.values_mut() {
            if matches!(
                state.status,
                DownloadStatus::Queued
                    | DownloadStatus::Downloading
                    | DownloadStatus::Pausing
                    | DownloadStatus::Cancelling
            ) {
                state.status = DownloadStatus::Error;
                state.error = Some(DOWNLOAD_SHUTDOWN_INTERRUPTED.to_string());
                state.speed = 0.0;
                state.retrying = false;
                state.next_retry_delay_seconds = None;
                state.task_registered = false;
            }
        }
    }
    // The lifecycle owner calls this only after all invocation, worker and
    // projector effects have drained. Durable recovery truth remains untouched.
    publications.publish().await;
    Ok(())
}

impl HuggingFaceClient {
    async fn project_execution_refusal(
        downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
        publications: &Arc<DownloadPublicationOwner>,
        download_id: &str,
        context: &TaskContext,
        role: TaskRole,
        error: &PumasError,
    ) {
        let _ = context.drain_blocking().await;
        let changed = {
            let mut states = downloads.write().await;
            if !context.is_current_role(role) {
                return;
            }
            states.get_mut(download_id).is_some_and(|state| {
                if matches!(
                    state.status,
                    DownloadStatus::Completed | DownloadStatus::Cancelled
                ) {
                    return false;
                }
                // Refusal establishes no durable mutation or settlement authority.
                state.status = DownloadStatus::Error;
                state.error = Some(error.to_string());
                state.task_registered = false;
                state.lifecycle_failure_unverified = true;
                true
            })
        };
        if changed {
            publish_download_snapshot_from_parts(publications).await;
            let _ = context.complete_transferred_projection(true);
        }
    }

    /// Permanently close download admission and observe owned work to completion.
    /// Cancelling one waiter does not cancel the shared drain or its result.
    pub async fn shutdown_downloads(&self) -> Result<()> {
        let downloads = self.downloads.clone();
        let publications = self.download_publications.clone();
        self.download_tasks
            .shutdown(move || project_download_shutdown(downloads, publications))
            .await
    }

    async fn reconcile_download_reads(&self) {
        if self.destination_root.is_none() {
            return;
        }
        let client = self.clone_for_invocation();
        let result = self
            .run_download_invocation(move |context| async move {
                let context = client.protect_download_mutation(&context).await?;
                client.reconcile_inactive_active_downloads(&context).await;
                Ok(())
            })
            .await;
        if let Err(error) = result {
            if !matches!(
                error,
                PumasError::DownloadLifecycleClosed | PumasError::DownloadRootBusy
            ) {
                warn!("Download read reconciliation owner failed: {error}");
            }
        }
    }

    pub(crate) async fn inspect_recovery_model_directory(
        &self,
        library_root: std::path::PathBuf,
        record: crate::index::ModelRecord,
    ) -> Result<Option<std::path::PathBuf>> {
        self.run_download_invocation(move |context| async move {
            context
                .run_fallible_blocking_named("inspect recovery model directory", move || {
                    crate::model_library::canonical_managed_model_dir(&library_root, &record)
                })
                .await
                .map_err(|error| {
                    PumasError::Other(format!("Recovery inspection owner failed: {error}"))
                })?
        })
        .await
    }

    pub(crate) async fn verify_recovery_model_snapshot(
        &self,
        library_root: std::path::PathBuf,
        record: crate::index::ModelRecord,
        token: crate::model_library::DownloadRecoveryToken,
    ) -> Result<crate::model_library::DownloadRecoveryVerification> {
        self.run_download_invocation(move |context| async move {
            context
                .run_fallible_blocking_named("verify recovery model snapshot", move || {
                    crate::model_library::verify_download_recovery_ticket(
                        &library_root,
                        &record,
                        &token,
                    )
                })
                .await
                .map_err(|error| {
                    PumasError::Other(format!("Recovery verification owner failed: {error}"))
                })?
        })
        .await
    }

    async fn project_finished_task_observation(
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_publications: Arc<DownloadPublicationOwner>,
        task_context: TaskContext,
        download_id: String,
        observation: Option<TaskObservation>,
    ) -> ProjectionOutcome {
        #[cfg(test)]
        task_context.observe_projection("finished-task");
        let Some(observation) = observation else {
            let changed = {
                let mut states = downloads.write().await;
                if !task_context.is_current_role(TaskRole::TerminalProjection) {
                    return ProjectionOutcome::RolledBack;
                }
                states.get_mut(&download_id).is_some_and(|state| {
                    state.status = DownloadStatus::Error;
                    state.error = Some("download task outcome was unavailable".to_string());
                    state.task_registered = false;
                    state.lifecycle_failure_unverified = true;
                    true
                })
            };
            if changed {
                publish_download_snapshot_from_parts(&download_publications).await;
            }
            return ProjectionOutcome::Failed;
        };

        let failed =
            observation.terminal == TaskTerminal::Panicked || observation.nested_failures > 0;
        let changed = {
            let mut states = downloads.write().await;
            if !task_context.is_current_role(TaskRole::TerminalProjection) {
                return ProjectionOutcome::RolledBack;
            }
            let Some(state) = states.get_mut(&download_id) else {
                return ProjectionOutcome::Committed;
            };
            let unfinished_finalizer = observation.role == TaskRole::CancelFinalizer
                && state.status == DownloadStatus::Cancelling;
            let unfinished_worker = observation.role == TaskRole::Worker
                && matches!(
                    state.status,
                    DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Pausing
                );
            let unreported_lifecycle_failure = failed
                && !matches!(
                    state.status,
                    DownloadStatus::Completed | DownloadStatus::Cancelled
                );
            if unfinished_worker || unfinished_finalizer || unreported_lifecycle_failure {
                state.status = DownloadStatus::Error;
                state.error = Some("download task ended without a verified terminal state".into());
                state.task_registered = false;
                state.lifecycle_failure_unverified = true;
                true
            } else {
                false
            }
        };

        if failed {
            warn!(
                "Observed failed {:?} owner for download {} ({} nested failure(s))",
                observation.role, download_id, observation.nested_failures
            );
        }
        if changed {
            publish_download_snapshot_from_parts(&download_publications).await;
        }
        ProjectionOutcome::Committed
    }

    async fn project_terminal_projection_failure(
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_publications: Arc<DownloadPublicationOwner>,
        task_context: TaskContext,
        download_id: String,
        detail: &'static str,
    ) -> ProjectionOutcome {
        let changed = {
            let mut states = downloads.write().await;
            if !task_context.is_current_role(TaskRole::TerminalProjection) {
                return ProjectionOutcome::RolledBack;
            }
            states.get_mut(&download_id).is_some_and(|state| {
                state.status = DownloadStatus::Error;
                state.error = Some(detail.to_string());
                state.task_registered = false;
                state.lifecycle_failure_unverified = true;
                true
            })
        };
        if changed {
            warn!("Terminal projection failed for download {}", download_id);
            publish_download_snapshot_from_parts(&download_publications).await;
            ProjectionOutcome::Failed
        } else {
            ProjectionOutcome::RolledBack
        }
    }

    async fn project_inactive_download(
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_publications: Arc<DownloadPublicationOwner>,
        persistence: Option<Arc<DownloadPersistence>>,
        task_context: TaskContext,
        download_id: String,
        persist: bool,
    ) -> ProjectionOutcome {
        #[cfg(test)]
        task_context.observe_projection("inactive-task");
        let persistence_ok = if persist {
            if let Some(persistence) = persistence {
                let persisted_id = download_id.clone();
                let attempt = downloads.read().await.get(&download_id).and_then(|state| {
                    state
                        .admission
                        .as_ref()
                        .map(|entry| entry.attempt_id.clone())
                });
                matches!(
                    task_context
                        .run_fallible_blocking_named("persist reconciliation pause", move || {
                            if let Some(attempt) = attempt {
                                persistence.update_admitted_status(
                                    &persisted_id,
                                    &attempt,
                                    DownloadStatus::Paused,
                                )
                            } else {
                                Err(PumasError::Config {
                                    message: "Ordinary reconciliation requires durable admission"
                                        .into(),
                                })
                            }
                        },)
                        .await,
                    Ok(Ok(true))
                )
            } else {
                true
            }
        } else {
            true
        };

        let failed = {
            let mut states = downloads.write().await;
            if !task_context.is_current_role(TaskRole::TerminalProjection) {
                return ProjectionOutcome::RolledBack;
            }
            let Some(state) = states.get_mut(&download_id) else {
                return ProjectionOutcome::RolledBack;
            };
            if !matches!(
                state.status,
                DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Pausing
            ) || !state.task_registered
            {
                return ProjectionOutcome::RolledBack;
            }
            if persistence_ok {
                state.status = DownloadStatus::Paused;
                state.speed = 0.0;
                state.retrying = false;
                state.next_retry_delay_seconds = None;
                state.task_registered = false;
                false
            } else {
                state.status = DownloadStatus::Error;
                state.error =
                    Some("failed to persist inactive-download reconciliation".to_string());
                state.task_registered = false;
                state.lifecycle_failure_unverified = true;
                true
            }
        };

        if failed {
            warn!(
                "Failed to persist inactive download {} reconciliation",
                download_id
            );
        } else {
            warn!(
                "Marking inactive download {} as paused because no task is running",
                download_id
            );
        }
        publish_download_snapshot_from_parts(&download_publications).await;
        if failed {
            ProjectionOutcome::Failed
        } else {
            ProjectionOutcome::Committed
        }
    }

    async fn observe_finished_download_tasks(&self) {
        self.download_tasks.rescue_abandoned();
        let candidate_ids = self.download_tasks.finished_or_projecting_ids();
        for download_id in candidate_ids {
            let transition = {
                // Fixed hierarchy: state first, lifecycle second. The finished
                // predecessor is replaced by a projector without an ownerless
                // ABA window; the start gate remains closed under both locks.
                let states = self.downloads.write().await;
                let snapshot = self.download_tasks.snapshot(&download_id);
                let inherit_failure = states.get(&download_id).is_some_and(|state| {
                    snapshot
                        .as_ref()
                        .is_some_and(|snapshot| match snapshot.role {
                            TaskRole::Worker => matches!(
                                state.status,
                                DownloadStatus::Queued
                                    | DownloadStatus::Downloading
                                    | DownloadStatus::Pausing
                            ),
                            TaskRole::CancelFinalizer => state.status == DownloadStatus::Cancelling,
                            _ => false,
                        })
                });
                let downloads = self.downloads.clone();
                let download_publications = self.download_publications.clone();
                let projected_id = download_id.clone();
                let fallback_downloads = self.downloads.clone();
                let fallback_publications = self.download_publications.clone();
                let fallback_id = download_id.clone();
                self.download_tasks.begin_finished_projection(
                    &download_id,
                    inherit_failure,
                    move |task_context, observation| async move {
                        Self::project_finished_task_observation(
                            downloads,
                            download_publications,
                            task_context,
                            projected_id,
                            observation,
                        )
                        .await
                    },
                    move |task_context| async move {
                        Self::project_terminal_projection_failure(
                            fallback_downloads,
                            fallback_publications,
                            task_context,
                            fallback_id,
                            "download terminal projection panicked",
                        )
                        .await
                    },
                )
            };
            let Ok(transition) = transition else { return };
            let ticket = match transition {
                ProjectionTransition::Started(projector)
                | ProjectionTransition::Existing(projector) => projector.start(),
                ProjectionTransition::NotReady => continue,
            };
            let _ = ticket.wait().await;
            while self.download_tasks.settle_projection(&ticket) == ProjectionSettlement::Pending {
                tokio::task::yield_now().await;
            }
        }
    }

    async fn reconcile_inactive_active_downloads(&self, context: &TaskContext) {
        self.observe_finished_download_tasks().await;

        let missing_registered_tasks = {
            let running_task_ids = self
                .download_tasks
                .ids()
                .into_iter()
                .filter(|download_id| {
                    self.download_tasks
                        .snapshot(download_id)
                        .is_some_and(|snapshot| snapshot.started && !snapshot.finished)
                })
                .collect::<HashSet<_>>();
            let downloads = self.downloads.read().await;

            downloads
                .iter()
                .filter_map(|(download_id, state)| {
                    let is_active = matches!(
                        state.status,
                        DownloadStatus::Queued
                            | DownloadStatus::Downloading
                            | DownloadStatus::Pausing
                    );
                    let has_running_task = running_task_ids.contains(download_id);

                    if is_active && state.task_registered && !has_running_task {
                        Some((download_id.clone(), state.recovery_destination().is_some()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        if missing_registered_tasks.is_empty() {
            return;
        }

        let projectors = {
            let mut downloads = self.downloads.write().await;
            let mut projectors = Vec::new();
            for (download_id, is_recovery) in &missing_registered_tasks {
                if let Some(state) = downloads.get_mut(download_id) {
                    let has_current_owner = self.download_tasks.snapshot(download_id).is_some();
                    if matches!(
                        state.status,
                        DownloadStatus::Queued
                            | DownloadStatus::Downloading
                            | DownloadStatus::Pausing
                    ) && state.task_registered
                        && !has_current_owner
                    {
                        let states = self.downloads.clone();
                        let publications = self.download_publications.clone();
                        let persistence = self.persistence.clone();
                        let projected_id = download_id.clone();
                        let persist = !*is_recovery;
                        let fallback_states = self.downloads.clone();
                        let fallback_publications = self.download_publications.clone();
                        let fallback_id = download_id.clone();
                        let protected_context = context.clone();
                        let prepared = self.download_tasks.prepare_projection(
                            download_id.clone(),
                            move |task_context, _| async move {
                                let task_context =
                                    task_context.inherit_root_grant(&protected_context);
                                Self::project_inactive_download(
                                    states,
                                    publications,
                                    persistence,
                                    task_context,
                                    projected_id,
                                    persist,
                                )
                                .await
                            },
                            move |task_context| async move {
                                Self::project_terminal_projection_failure(
                                    fallback_states,
                                    fallback_publications,
                                    task_context,
                                    fallback_id,
                                    "inactive-download reconciliation panicked",
                                )
                                .await
                            },
                        );
                        let Ok(prepared) = prepared else { continue };
                        if let Ok(projector) =
                            self.download_tasks.install_projection_gated(prepared)
                        {
                            projectors.push(projector);
                        }
                    }
                }
            }
            projectors
        };
        self.download_tasks.rescue_abandoned();

        if projectors.is_empty() {
            return;
        }

        let tickets = projectors
            .into_iter()
            .map(super::lifecycle::InstalledProjection::start)
            .collect::<Vec<_>>();
        for ticket in tickets {
            let _ = ticket.wait().await;
            while self.download_tasks.settle_projection(&ticket) == ProjectionSettlement::Pending {
                tokio::task::yield_now().await;
            }
        }
    }

    /// Restore persisted downloads from disk.
    ///
    /// Returns an error for corrupt or unresolved authoritative inventory;
    /// these failures must not be interpreted as an empty download list.
    ///
    /// Reconciles durable admission inventory and restores queue-owned entries
    /// in recorded order, including entries that have not written any bytes.
    /// Old tracking formats require explicit conversion before restoration.
    /// Invalid or unresolved authoritative inventory is returned as an error.
    pub async fn restore_persisted_downloads(&self) -> Result<Vec<DownloadCompletionInfo>> {
        let client = self.clone_for_invocation();
        self.run_download_invocation(move |context| async move {
            let context = client.protect_download_mutation(&context).await?;
            client.restore_persisted_downloads_admitted(&context).await
        })
        .await
    }

    async fn restore_persisted_downloads_admitted(
        &self,
        context: &TaskContext,
    ) -> Result<Vec<DownloadCompletionInfo>> {
        let persistence = match &self.persistence {
            Some(p) => p.clone(),
            None => return Ok(Vec::new()),
        };

        let protected_context = context.clone();
        let inventory = self
            .run_download_invocation(move |context| async move {
                let context = context.inherit_root_grant(&protected_context);
                drop(protected_context);
                context
                    .run_fallible_blocking_named("reconcile download restore inventory", move || {
                        persistence.reconcile_lifecycle_inventory_strict()?;
                        let inventory = persistence.load_lifecycle_inventory_strict()?;
                        validate_restore_inventory(&inventory)?;
                        // A crash can leave durable Verified cleanup immediately
                        // before its exact queue settlement. No filesystem replay
                        // or new admission is authorized by this terminal proof.
                        for id in inventory.quarantines.keys() {
                            if let Some(admission) = inventory.queue_admissions.get(id) {
                                if !persistence.settle_queue_admission(id, &admission.attempt_id)? {
                                    return Err(PumasError::Validation {
                                        field: "download_cleanup".into(),
                                        message: "Verified cleanup queue settlement was not confirmed".into(),
                                    });
                                }
                            }
                        }
                        let inventory = persistence.load_lifecycle_inventory_strict()?;
                        validate_restore_inventory(&inventory)?;
                        if inventory.quarantines.keys().any(|id| inventory.queue_admissions.contains_key(id)) {
                            return Err(PumasError::Validation {
                                field: "download_cleanup".into(),
                                message: "Cleanup queue custody remains active after reconciliation".into(),
                            });
                        }
                        Ok(inventory)
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Download restore owner failed: {error}"))
                    })?
            })
            .await?;
        let verified_failures = inventory
            .quarantines
            .values()
            .filter(|quarantine| quarantine.domain == LifecycleQuarantineDomain::Ambient)
            .map(|quarantine| DownloadState::from_verified_cleanup(&quarantine.snapshot))
            .collect::<Vec<_>>();
        let mut entries = inventory
            .downloads
            .into_iter()
            .map(|entry| {
                let admission = inventory
                    .queue_admissions
                    .get(&entry.download_id)
                    .cloned()
                    .ok_or_else(|| PumasError::Validation {
                        field: "download_admission".into(),
                        message: "Current download snapshot requires durable admission".into(),
                    })?;
                Ok((entry, admission))
            })
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by_key(|(_, admission)| admission.position.ordinal);

        let mut restored_entries = Vec::new();

        for (entry, admission) in entries {
            let root = self
                .destination_root
                .clone()
                .ok_or_else(|| PumasError::Config {
                    message: "Download restore destination authority unavailable".into(),
                })?;
            let target = entry.dest_dir.clone();
            let destination = context
                .run_fallible_blocking_named("resolve restored download destination", move || {
                    root.resolve(&target)
                })
                .await
                .map_err(|error| {
                    PumasError::Other(format!("Download restore authority owner failed: {error}"))
                })??;
            {
                if admission.domain != DownloadAdmissionDomain::Ambient {
                    return Err(PumasError::Other(
                        "Recovery admission requires ticket reconciliation".into(),
                    ));
                }
                let expected = admission.destination.clone();
                let files = admission.execution_files.clone();
                let (destination, downloaded_bytes) = context
                    .run_fallible_blocking_named(
                        "inspect restored download destination",
                        move || -> Result<_> {
                            if destination.persisted_identity()? != expected {
                                return Err(PumasError::Other(
                                    "Persisted download destination identity changed".into(),
                                ));
                            }
                            let mut bytes = 0u64;
                            for file in files {
                                bytes = bytes
                                    .checked_add(
                                        destination
                                            .file_len(&file)?
                                            .or(destination.part_len(&file)?)
                                            .unwrap_or(0),
                                    )
                                    .ok_or_else(|| {
                                        PumasError::Other("Download byte count overflow".into())
                                    })?;
                            }
                            Ok((destination, bytes))
                        },
                    )
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!(
                            "Download restore authority owner failed: {error}"
                        ))
                    })??;
                restored_entries.push((
                    entry,
                    downloaded_bytes,
                    Some(super::types::AdmittedDownload {
                        attempt_id: admission.attempt_id.clone(),
                    }),
                    DownloadDestination::Managed(destination),
                ));
            }
        }

        if restored_entries.is_empty() && verified_failures.is_empty() {
            return Ok(Vec::new());
        }

        info!("Restoring {} persisted downloads", restored_entries.len());
        let restored_ids = restored_entries
            .iter()
            .map(|(entry, _, _, _)| entry.download_id.clone())
            .collect::<Vec<_>>();
        let mut downloads = self.downloads.write().await;

        for state in verified_failures {
            downloads.entry(state.download_id.clone()).or_insert(state);
        }
        for (entry, downloaded_bytes, admission, destination) in restored_entries {
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

            let identity = destination.identity();
            let mut state = DownloadState::from_persisted(&entry, downloaded_bytes, destination);
            state.admission = admission;

            info!(
                "Restoring download {}: {} ({} bytes on disk, status {:?})",
                entry.download_id, entry.repo_id, downloaded_bytes, state.status
            );

            if !self.destination_executions.reserve_dormant(
                identity,
                entry.download_id.clone(),
                DestinationDomain::Ambient,
            ) {
                continue;
            }
            downloads.insert(entry.download_id.clone(), state);
        }
        drop(downloads);
        self.publish_download_snapshot().await;
        let mut completed = Vec::new();
        for id in restored_ids {
            if let Some(info) = self.finalize_restored_download(context, &id).await? {
                completed.push(info);
            }
        }
        Ok(completed)
    }

    async fn finalize_restored_download(
        &self,
        protected_context: &TaskContext,
        download_id: &str,
    ) -> Result<Option<DownloadCompletionInfo>> {
        let (info, files, destination, admission, initial_status) = {
            let states = self.downloads.read().await;
            let Some(state) = states.get(download_id) else {
                return Ok(None);
            };
            if !matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                || state.lifecycle_failure_unverified
                || self.download_tasks.contains(download_id)
            {
                return Ok(None);
            }
            let Some(destination) = state.destination.clone() else {
                return Ok(None);
            };
            if !self.destination_executions.is_first(
                &destination.identity(),
                download_id,
                DestinationDomain::Ambient,
            ) {
                return Ok(None);
            }
            let info = DownloadCompletionInfo {
                download_id: download_id.into(),
                dest_dir: state.dest_dir.clone(),
                filename: state.filename.clone(),
                filenames: state
                    .files
                    .iter()
                    .map(|file| file.filename.clone())
                    .collect(),
                download_request: state.download_request.clone().ok_or_else(|| {
                    PumasError::Config {
                        message: "Restore finalization request is unavailable".into(),
                    }
                })?,
                known_sha256: state.known_sha256.clone(),
                huggingface_evidence: state.huggingface_evidence.clone(),
            };
            (
                info,
                state.files.clone(),
                destination,
                state.admission.clone(),
                state.status,
            )
        };
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let mut prepared_download = self
            .prepare_download_task(
                download_id.into(),
                info.download_request.repo_id.clone(),
                files,
                destination.clone(),
                cancel_flag.clone(),
                pause_flag.clone(),
                None,
                None,
                self.persistence.clone(),
                self.persistence.clone(),
            )
            .await;
        prepared_download.restore_finalization = Some(initial_status);
        let (finished_sender, finished) = tokio::sync::oneshot::channel();
        let result_states = self.downloads.clone();
        let protected_context = protected_context.clone();
        let prepared = self.download_tasks.prepare(
            download_id.into(),
            TaskRole::Worker,
            move |context| async move {
                let context = context.inherit_root_grant(&protected_context);
                drop(protected_context);
                let result = prepared_download.run_owned(context.clone()).await;
                if result.as_ref().is_ok_and(|complete| *complete) {
                    let mut states = result_states.write().await;
                    if context.is_current_role(TaskRole::Worker)
                        && states
                            .get(&info.download_id)
                            .is_some_and(|state| state.status == DownloadStatus::Completed)
                    {
                        states.remove(&info.download_id);
                    }
                }
                // The exact worker outcome survives UI-state clearing and
                // normal owner retirement. Never infer this result by ID.
                let _ = finished_sender.send(result.map(|complete| complete.then_some(info)));
            },
        )?;
        let installed = {
            let mut states = self.downloads.write().await;
            let Some(state) = states.get_mut(download_id) else {
                return Ok(None);
            };
            if !matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                || state.lifecycle_failure_unverified
                || self.download_tasks.contains(download_id)
                || !state.matches_destination(&destination.identity())
                || state.admission.as_ref().map(|value| &value.attempt_id)
                    != admission.as_ref().map(|value| &value.attempt_id)
                || !self.destination_executions.is_first(
                    &destination.identity(),
                    download_id,
                    DestinationDomain::Ambient,
                )
            {
                return Ok(None);
            }
            let Ok(installed) = self.download_tasks.install_gated(prepared) else {
                return Ok(None);
            };
            if !self.destination_executions.reserve(
                destination.identity(),
                download_id.into(),
                DestinationDomain::Ambient,
                installed.generation().clone(),
            ) {
                drop(installed);
                drop(states);
                self.download_tasks.rescue_abandoned();
                return Ok(None);
            }
            state.cancel_flag = cancel_flag;
            state.pause_flag = pause_flag;
            state.status = DownloadStatus::Queued;
            state.task_registered = true;
            installed
        };
        let generation = installed.generation().clone();
        installed.start();
        let result = finished
            .await
            .map_err(|_| PumasError::Other("Restore finalization owner did not finish".into()))?;
        while self
            .download_tasks
            .generation_is_current(download_id, &generation)
            && self
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| !task.finished)
        {
            tokio::task::yield_now().await;
        }
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("restore-finalization-result", download_id);
        self.observe_finished_download_tasks().await;
        self.publish_download_snapshot().await;
        result
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
        remote_evidence: Option<crate::models::HuggingFaceEvidence>,
    ) -> Result<String> {
        let client = self.clone_for_invocation();
        let request = request.clone();
        let dest_dir = dest_dir.to_path_buf();
        self.run_download_invocation(move |context| async move {
            client
                .start_download_admitted(&context, &request, &dest_dir, remote_evidence)
                .await
        })
        .await
    }

    async fn start_download_admitted(
        &self,
        context: &TaskContext,
        request: &DownloadRequest,
        dest_dir: &Path,
        remote_evidence: Option<crate::models::HuggingFaceEvidence>,
    ) -> Result<String> {
        let protected_context = self.protect_download_mutation(context).await?;
        let context = &protected_context;
        let root = self
            .destination_root
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: "Download destination authority is unavailable".into(),
            })?;
        let persistence = self.persistence.clone().ok_or_else(|| PumasError::Config {
            message: "Durable download admission is unavailable".into(),
        })?;
        let requested_destination = dest_dir.to_path_buf();
        let destination = context
            .run_fallible_blocking_named("resolve download destination", move || {
                root.resolve(&requested_destination)
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Download authority resolution failed: {error}"))
            })??;
        let dest_dir = destination.display_path();
        self.observe_finished_download_tasks().await;

        let download_id = uuid::Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Get file info
        let metadata_client = self.clone_for_invocation();
        let repo_id = request.repo_id.clone();
        let tree = context
            .run_fallible_async_named("load download repository files", move || async move {
                metadata_client.get_repo_files(&repo_id).await
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Download metadata observation failed: {error}"))
            })??;

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
        let requested_payload_files = files
            .iter()
            .map(|file| file.filename.clone())
            .collect::<Vec<_>>();
        aux_files.extend(files);
        let files = aux_files;

        // Await every fallible/cancellable prerequisite before acquiring the
        // state/task admission critical section. From this point onward task
        // construction is pure until the gated owner and state are committed
        // together.
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("prepare-download-task", &download_id);
        let auth_header = self.auth_header_value().await;
        let destination_lock = self.destination_lock(&destination.identity()).await;
        let pause_flag = Arc::new(AtomicBool::new(false));

        let (admission_sender, admission_receiver) = tokio::sync::oneshot::channel();
        let (admission_completed, admission_completion) = tokio::sync::watch::channel(false);
        let admission_identity = super::lifecycle::PendingAdmissionIdentity {
            destination: destination.identity(),
            repo_id: request.repo_id.clone(),
            files: files
                .iter()
                .map(|file| (file.filename.clone(), file.size, file.sha256.clone()))
                .collect(),
        };
        let installed = {
            let downloads = self.downloads.write().await;
            if let Some((existing_id, mut completion)) =
                self.download_tasks.pending_admission(&admission_identity)
            {
                drop(downloads);
                completion
                    .wait_for(|completed| *completed)
                    .await
                    .map_err(|_| {
                        PumasError::Other("Concurrent download admission failed".into())
                    })?;
                let downloads = self.downloads.read().await;
                if downloads.get(&existing_id).is_some_and(|state| {
                    state.admission.is_some()
                        && matches!(
                            state.status,
                            DownloadStatus::Queued
                                | DownloadStatus::Downloading
                                | DownloadStatus::Pausing
                        )
                        && !state.cancel_flag.load(Ordering::Relaxed)
                }) {
                    return Ok(existing_id);
                }
                return Err(PumasError::Other(
                    "Concurrent download admission no longer active".into(),
                ));
            }

            // Inactive resumable states still own their partial files and the
            // shared marker. Restore those dormant reservations before the
            // new admission takes its ordered position.
            let mut dormant_ids = downloads
                .iter()
                .filter(|(_, state)| {
                    state.matches_destination(&destination.identity())
                        && matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                })
                .map(|(id, state)| {
                    (
                        id.clone(),
                        if state.recovery_destination().is_some() {
                            DestinationDomain::Recovery
                        } else {
                            DestinationDomain::Ambient
                        },
                    )
                })
                .collect::<Vec<_>>();
            dormant_ids.sort_by(|left, right| left.0.cmp(&right.0));
            for (dormant_id, domain) in dormant_ids {
                self.destination_executions.reserve_dormant(
                    destination.identity(),
                    dormant_id,
                    domain,
                );
            }

            // Revalidate destination/file dedupe at the exact commit point.
            // A concurrent start may have appeared after the earlier scan.
            if let Some(existing_id) = downloads.iter().find_map(|(id, state)| {
                let same_files = state.files.len() == files.len()
                    && state.files.iter().zip(&files).all(|(left, right)| {
                        left.filename == right.filename
                            && left.size == right.size
                            && left.sha256 == right.sha256
                    });
                let exact_started_worker = self.download_tasks.snapshot(id).is_some_and(|task| {
                    task.role == TaskRole::Worker && task.started && !task.outer_finished
                });
                (state.matches_destination(&destination.identity())
                    && state.repo_id == request.repo_id
                    && same_files
                    && !state.cancel_flag.load(Ordering::Relaxed)
                    && matches!(
                        state.status,
                        DownloadStatus::Queued
                            | DownloadStatus::Downloading
                            | DownloadStatus::Pausing
                    )
                    && state.recovery_destination().is_none()
                    && !state.ambient_authority_blocked
                    && exact_started_worker)
                    .then(|| id.clone())
            }) {
                return Ok(existing_id);
            }

            let known_sum: u64 = files.iter().filter_map(|file| file.size).sum();
            let total_bytes = (known_sum > 0).then_some(known_sum);
            let first_filename = files[0].filename.clone();
            let final_filenames = files
                .iter()
                .map(|file| file.filename.clone())
                .collect::<Vec<_>>();
            let known_sha256 = files
                .iter()
                .filter(|file| file.size.is_some())
                .max_by_key(|file| file.size.unwrap_or(0))
                .and_then(|file| file.sha256.clone());
            let mut huggingface_evidence = remote_evidence.clone();
            if let Some(ref mut evidence) = huggingface_evidence {
                Self::enrich_huggingface_evidence_for_download(
                    evidence,
                    &tree,
                    request,
                    &final_filenames,
                );
            }
            let persisted_download = self.persistence.as_ref().map(|_| PersistedDownload {
                download_id: download_id.clone(),
                repo_id: request.repo_id.clone(),
                filename: first_filename.clone(),
                filenames: final_filenames,
                dest_dir: dest_dir.to_path_buf(),
                total_bytes,
                status: DownloadStatus::Queued,
                download_request: request.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: known_sha256.clone(),
                huggingface_evidence: huggingface_evidence.clone(),
            });
            let marker_contents = serialize_download_marker(
                request,
                requested_payload_files.clone(),
                huggingface_evidence.as_ref(),
            )?;
            let admission_snapshot = persisted_download
                .clone()
                .ok_or_else(|| PumasError::Other("Durable download snapshot unavailable".into()))?;
            let execution_files = files.iter().map(|file| file.filename.clone()).collect();
            let prepared_download = PreparedDownloadTask {
                #[cfg(test)]
                download_base_url: self.download_base_url.clone(),
                client: self.download_client.clone(),
                downloads: self.downloads.clone(),
                download_publications: self.download_publications.clone(),
                destination_executions: self.destination_executions.clone(),
                download_id: download_id.clone(),
                repo_id: request.repo_id.clone(),
                files: files.clone(),
                destination: DownloadDestination::Managed(destination.clone()),
                cancel_flag: cancel_flag.clone(),
                pause_flag: pause_flag.clone(),
                completion_callback: self.completion_callback.clone(),
                aux_complete_callback: self.aux_complete_callback.clone(),
                download_importer: self.download_importer.clone(),
                configured_root: self.destination_root.clone(),
                persistence: self.persistence.clone(),
                terminal_cleanup_persistence: self.persistence.clone(),
                auth_header,
                destination_lock,
                start_setup: Some(DownloadStartSetup { marker_contents }),
                persist_queued_status: false,
                restore_finalization: None,
            };
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
                task_registered: true,
                lifecycle_failure_unverified: false,
                dest_dir: dest_dir.to_path_buf(),
                ambient_authority_blocked: false,
                admission: None,
                revoked_snapshot: None,
                destination: Some(DownloadDestination::Managed(destination.clone())),
                filename: first_filename,
                files: files.clone(),
                files_completed: 0,
                download_request: Some(request.clone()),
                known_sha256: known_sha256.clone(),
                huggingface_evidence,
            };
            let owner = self.download_tasks.clone();
            let states = self.downloads.clone();
            let executions = self.destination_executions.clone();
            let held_destination = destination.clone();
            let attempt_id = uuid::Uuid::new_v4().to_string();
            let protected_context = context.clone();
            let prepared = self.download_tasks.prepare(
                download_id.clone(),
                TaskRole::AdmissionTransition,
                move |task_context| async move {
                    let task_context = task_context.inherit_root_grant(&protected_context);
                    drop(protected_context);
                    let attempt = attempt_id.clone();
                    let admission_destination = held_destination.clone();
                    #[cfg(test)]
                    let admission_observer = owner.clone();
                    let outcome = task_context
                        .run_fallible_blocking_named(
                            "durably admit download",
                            move || -> Result<_> {
                                let admission_request = DownloadAdmissionRequest {
                                    snapshot: admission_snapshot,
                                    domain: DownloadAdmissionDomain::Ambient,
                                    destination: admission_destination.persisted_identity()?,
                                    requested_payload_files,
                                    execution_files,
                                };
                                let before = persistence.load_lifecycle_inventory_strict()?;
                                if before.hidden_admissions.values().any(|hidden| {
                                    hidden.request.destination == admission_request.destination
                                }) {
                                    return Err(PumasError::Other(
                                        "Destination has an unresolved durable admission".into(),
                                    ));
                                }
                                #[cfg(test)]
                                admission_observer.observe_ambient_admission("admission-inventory-checked", &admission_request.snapshot.download_id);
                                let transition =
                                    persistence.admit_download(&attempt, &admission_request)?;
                                let inventory = persistence.load_lifecycle_inventory_strict()?;
                                if let DownloadAdmissionTransition::Durable { admission, .. } = &transition {
                                    if inventory.hidden_admissions.values().any(|hidden|
                                        hidden.request.destination == admission_request.destination
                                            && hidden.position.ordinal < admission.position.ordinal)
                                    {
                                        return Err(PumasError::Other("Destination has an unresolved earlier durable admission".into()));
                                    }
                                }
                                Ok((transition, inventory, admission_request.destination))
                            },
                        )
                        .await;
                    let confirmed = match outcome {
                        Ok(Ok((transition, inventory, identity))) => transition.into_result().map(|_| (inventory, identity)),
                        Ok(Err(error)) => Err(error),
                        Err(error) => Err(PumasError::Other(format!(
                            "Download admission owner failed: {error}"
                        ))),
                    };
                    let (inventory, identity) = match confirmed {
                        Ok(confirmed) => confirmed,
                        Err(error) => {
                            let _ = admission_sender.send(Err(error));
                            let _ = task_context.drain_blocking().await;
                            return;
                        }
                    };
                    {
                        let mut states = states.write().await;
                        let mut predecessors = inventory
                            .queue_admissions
                            .iter()
                            .filter(|(_, entry)| entry.destination == identity)
                            .collect::<Vec<_>>();
                        predecessors.sort_by_key(|(_, entry)| entry.position.ordinal);
                        for (id, entry) in predecessors {
                            executions.reserve_dormant(
                                held_destination.identity(),
                                id.clone(),
                                match entry.domain {
                                    DownloadAdmissionDomain::Ambient => DestinationDomain::Ambient,
                                    DownloadAdmissionDomain::Recovery => {
                                        DestinationDomain::Recovery
                                    }
                                },
                            );
                        }
                        if !owner.promote_admission(
                            task_context.download_id(),
                            task_context.generation(),
                        ) {
                            let _ = admission_sender.send(Err(PumasError::Other(
                                "Download admission lost its owner".into(),
                            )));
                            return;
                        }
                        if !executions.reserve(
                            held_destination.identity(),
                            task_context.download_id().to_string(),
                            DestinationDomain::Ambient,
                            task_context.generation().clone(),
                        ) {
                            let _ = admission_sender.send(Err(PumasError::Other("Download destination reservation was refused".into())));
                            return;
                        }
                        let mut state = state;
                        state.admission = Some(super::types::AdmittedDownload {
                            attempt_id,
                        });
                        states.insert(state.download_id.clone(), state);
                    }
                    let _ = admission_sender.send(Ok(()));
                    admission_completed.send_replace(true);
                    drop(admission_completed);
                    let _ = prepared_download.run_owned(task_context).await;
                },
            )?;
            match self.download_tasks.install_gated(prepared) {
                Ok(installed) => {
                    self.download_tasks.bind_pending_admission(
                        &download_id,
                        installed.generation(),
                        admission_identity,
                        admission_completion,
                    );
                    Some(installed)
                }
                Err(rejected) => {
                    drop(rejected);
                    None
                }
            }
        };
        let Some(installed) = installed else {
            self.download_tasks.rescue_abandoned();
            return Err(PumasError::Other(
                "download task owner collision during admission".to_string(),
            ));
        };
        installed.start();
        admission_receiver.await.map_err(|error| {
            PumasError::Other(format!("Download admission owner ended: {error}"))
        })??;
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("ordinary-start-started", &download_id);
        self.publish_download_snapshot().await;

        info!(
            "Starting download {} for {} ({} file{})",
            download_id,
            request.repo_id,
            files.len(),
            if files.len() == 1 { "" } else { "s" }
        );

        Ok(download_id)
    }

    async fn destination_lock(
        &self,
        identity: &crate::model_library::download_recovery::DestinationIdentity,
    ) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.dest_locks.write().await;
        locks
            .entry(identity.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    #[allow(clippy::too_many_arguments)]
    async fn prepare_download_task(
        &self,
        download_id: String,
        repo_id: String,
        files: Vec<FileToDownload>,
        destination: DownloadDestination,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        completion_callback: Option<DownloadCompletionCallback>,
        aux_complete_callback: Option<AuxFilesCompleteCallback>,
        persistence: Option<Arc<DownloadPersistence>>,
        terminal_cleanup_persistence: Option<Arc<DownloadPersistence>>,
    ) -> PreparedDownloadTask {
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("prepare-download-task", &download_id);
        let auth_header = self.auth_header_value().await;
        let destination_lock = self.destination_lock(&destination.identity()).await;
        // Ticket recovery grants only its existing bound-file effects. It does
        // not acquire the builder's ordinary metadata/import mutation policy.
        let download_importer = if destination.is_recovery() {
            None
        } else {
            self.download_importer.clone()
        };
        PreparedDownloadTask {
            #[cfg(test)]
            download_base_url: self.download_base_url.clone(),
            client: self.download_client.clone(),
            downloads: self.downloads.clone(),
            download_publications: self.download_publications.clone(),
            destination_executions: self.destination_executions.clone(),
            download_id,
            repo_id,
            files,
            destination,
            configured_root: self.destination_root.clone(),
            cancel_flag,
            pause_flag,
            completion_callback,
            aux_complete_callback,
            download_importer,
            persistence,
            terminal_cleanup_persistence,
            auth_header,
            destination_lock,
            start_setup: None,
            persist_queued_status: false,
            restore_finalization: None,
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    async fn spawn_download_task(
        &self,
        download_id: String,
        repo_id: String,
        files: Vec<FileToDownload>,
        destination: DownloadDestination,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        completion_callback: Option<DownloadCompletionCallback>,
        aux_complete_callback: Option<AuxFilesCompleteCallback>,
        persistence: Option<Arc<DownloadPersistence>>,
    ) -> bool {
        let destination_path = destination.identity();
        let destination_domain = destination.domain();
        let prepared = self
            .prepare_download_task(
                download_id.clone(),
                repo_id,
                files,
                destination,
                cancel_flag,
                pause_flag,
                completion_callback,
                aux_complete_callback,
                persistence.clone(),
                persistence,
            )
            .await;
        let prepared = prepared
            .prepare_owned(&self.download_tasks, TaskRole::Worker, None)
            .unwrap();
        let installed = {
            let mut downloads = self.downloads.write().await;
            if let Some(state) = downloads.get_mut(&download_id) {
                if state.status == DownloadStatus::Queued {
                    if let Ok(installed) = self.download_tasks.install_gated(prepared) {
                        self.destination_executions.reserve(
                            destination_path,
                            download_id.clone(),
                            destination_domain,
                            installed.generation().clone(),
                        );
                        state.task_registered = true;
                        Some(installed)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        let Some(installed) = installed else {
            self.download_tasks.rescue_abandoned();
            return false;
        };
        installed.start();
        true
    }

    #[cfg(test)]
    async fn remove_stale_part_for_completed_file(dest_path: &Path, part_path: &Path) {
        let final_exists = tokio::fs::try_exists(dest_path).await.unwrap_or(false);
        let part_exists = tokio::fs::try_exists(part_path).await.unwrap_or(false);
        if final_exists && part_exists {
            match tokio::fs::remove_file(part_path).await {
                Ok(()) => info!(
                    "Removed stale partial file {} because final file exists",
                    part_path.display()
                ),
                Err(err) => warn!(
                    "Failed to remove stale partial file {}: {}",
                    part_path.display(),
                    err
                ),
            }
        }
    }

    #[cfg(test)]
    async fn finalize_complete_part_file(
        dest_path: &Path,
        part_path: &Path,
        expected_size: Option<u64>,
    ) -> Result<bool> {
        let Some(expected_size) = expected_size else {
            return Ok(false);
        };
        let Ok(metadata) = tokio::fs::metadata(part_path).await else {
            return Ok(false);
        };
        if !metadata.is_file() || metadata.len() != expected_size {
            return Ok(false);
        }

        tokio::fs::rename(part_path, dest_path)
            .await
            .map_err(|error| PumasError::DownloadFailed {
                url: part_path.display().to_string(),
                message: format!("Failed to finalize fully downloaded temp file: {error}"),
            })?;
        info!(
            "Finalized fully downloaded partial file {} ({} bytes)",
            dest_path.display(),
            expected_size
        );
        Ok(true)
    }

    /// Run the download in the background with retry and resume support.
    ///
    /// Downloads all files sequentially. Files that already exist on disk
    /// (from a previous partial download) are skipped automatically.
    #[allow(clippy::too_many_arguments)]
    async fn settle_worker_pause(
        downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
        download_publications: &Arc<DownloadPublicationOwner>,
        download_id: &str,
        task_context: &TaskContext,
        persistence: Option<&Arc<DownloadPersistence>>,
        destination_guard: &mut Option<OwnedMutexGuard<()>>,
    ) -> Result<()> {
        let persistence_ok = if let Some(persistence) = persistence {
            let persistence = persistence.clone();
            let persisted_id = download_id.to_string();
            let attempt = downloads.read().await.get(download_id).and_then(|state| {
                state
                    .admission
                    .as_ref()
                    .map(|entry| entry.attempt_id.clone())
            });
            matches!(
                task_context
                    .run_fallible_blocking_named("persist download pause", move || {
                        if let Some(attempt) = attempt {
                            persistence.update_admitted_status(
                                &persisted_id,
                                &attempt,
                                DownloadStatus::Paused,
                            )
                        } else {
                            Err(PumasError::Config {
                                message: "Ordinary pause requires durable admission".into(),
                            })
                        }
                    })
                    .await,
                Ok(Ok(true))
            )
        } else {
            true
        };

        let projected = {
            let mut states = downloads.write().await;
            let state = current_worker_state(
                &mut states,
                download_id,
                task_context,
                &[DownloadStatus::Downloading, DownloadStatus::Pausing],
            )?;
            state.speed = 0.0;
            state.task_registered = false;
            if persistence_ok {
                state.status = DownloadStatus::Paused;
                state.retrying = false;
                state.next_retry_delay_seconds = None;
            } else {
                state.status = DownloadStatus::Error;
                state.error = Some("failed to persist download pause".to_string());
                state.lifecycle_failure_unverified = true;
            }
            true
        };

        drop(destination_guard.take());
        if projected {
            publish_download_snapshot_from_parts(download_publications).await;
        }
        if persistence_ok {
            Err(PumasError::DownloadPaused)
        } else {
            Err(PumasError::Other(
                "failed to persist download pause".to_string(),
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_download(
        client: reqwest::Client,
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_publications: Arc<DownloadPublicationOwner>,
        download_id: &str,
        repo_id: &str,
        files: &[FileToDownload],
        destination: &DownloadDestination,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        persistence: Option<Arc<DownloadPersistence>>,
        terminal_cleanup_persistence: Option<Arc<DownloadPersistence>>,
        aux_complete_callback: Option<AuxFilesCompleteCallback>,
        download_importer: Option<Arc<crate::model_library::ModelImporter>>,
        auth_header: Option<String>,
        task_context: TaskContext,
        destination_lock: Arc<TokioMutex<()>>,
        start_setup: Option<DownloadStartSetup>,
        persist_queued_status: bool,
        #[cfg(test)] download_base_url: Option<String>,
    ) -> Result<()> {
        use crate::config::NetworkConfig;
        use crate::network::RetryConfig;

        #[cfg(test)]
        task_context.observe_worker_projection("worker-entry");
        let mut destination_guard = Some(destination_lock.clone().lock_owned().await);

        // Start only while the task registration still owns the queued state.
        // A concurrent cancellation may have won after registration but before
        // this future was first polled.
        let start_disposition = {
            let mut downloads = downloads.write().await;
            current_worker_state(
                &mut downloads,
                download_id,
                &task_context,
                &[DownloadStatus::Queued, DownloadStatus::Pausing],
            )
            .ok()
            .and_then(|state| {
                if state.cancel_flag.load(Ordering::Relaxed) {
                    return None;
                }
                if state.pause_flag.load(Ordering::Relaxed) {
                    state.status = DownloadStatus::Pausing;
                    return Some(true);
                }
                state.status = DownloadStatus::Downloading;
                Some(false)
            })
        };
        let Some(pause_before_start) = start_disposition else {
            return Err(PumasError::DownloadCancelled);
        };
        if pause_before_start {
            #[cfg(test)]
            task_context.observe_worker_projection("pause-before-destination");
            return Self::settle_worker_pause(
                &downloads,
                &download_publications,
                download_id,
                &task_context,
                persistence.as_ref(),
                &mut destination_guard,
            )
            .await;
        }
        publish_worker_snapshot_and_revalidate(
            &download_publications,
            &downloads,
            download_id,
            &task_context,
            destination,
            &destination_lock,
            &mut destination_guard,
            &[DownloadStatus::Downloading],
        )
        .await?;

        destination.prepare(&task_context).await?;

        if let Some(start_setup) = start_setup {
            destination
                .write_marker(&task_context, start_setup.marker_contents)
                .await?;
        }
        if persist_queued_status {
            let persistence = persistence.clone().ok_or_else(|| {
                PumasError::Other(
                    "download resume lost its configured persistence owner".to_string(),
                )
            })?;
            let persisted_id = download_id.to_string();
            let attempt = downloads
                .read()
                .await
                .get(download_id)
                .and_then(|state| state.admission.as_ref())
                .map(|admission| admission.attempt_id.clone());
            if let Some(attempt) = attempt.as_ref() {
                let marker = task_context
                    .run_fallible_blocking_named("read admitted resume marker", {
                        let persistence = persistence.clone();
                        let persisted_id = persisted_id.clone();
                        let attempt = attempt.clone();
                        let destination = destination.clone();
                        move || -> Result<String> {
                            let identity = destination.persisted_identity()?;
                            let inventory = persistence.load_lifecycle_inventory_strict()?;
                            let admission = inventory
                                .queue_admissions
                                .get(&persisted_id)
                                .filter(|admission| {
                                    admission.attempt_id == attempt
                                        && admission.domain == DownloadAdmissionDomain::Ambient
                                        && admission.destination == identity
                                })
                                .ok_or_else(|| PumasError::Validation {
                                    field: "download_admission".into(),
                                    message: "Resume marker requires its exact active admission"
                                        .into(),
                                })?;
                            let snapshot = inventory
                                .downloads
                                .iter()
                                .find(|snapshot| snapshot.download_id == persisted_id)
                                .ok_or_else(|| PumasError::Validation {
                                    field: "download_admission".into(),
                                    message: "Resume marker snapshot is unavailable".into(),
                                })?;
                            serialize_download_marker(
                                &snapshot.download_request,
                                admission.requested_payload_files.clone(),
                                snapshot.huggingface_evidence.as_ref(),
                            )
                        }
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Download resume marker owner failed: {error}"))
                    })??;
                destination.write_marker(&task_context, marker).await?;
            }
            match task_context
                .run_fallible_blocking_named("persist admitted download resume", move || {
                    if let Some(attempt) = attempt {
                        persistence.update_admitted_status(
                            &persisted_id,
                            &attempt,
                            DownloadStatus::Queued,
                        )
                    } else {
                        Err(PumasError::Config {
                            message: "Ordinary resume requires durable admission".into(),
                        })
                    }
                })
                .await
            {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => {
                    return Err(PumasError::Other(
                        "persisted download resume row was unavailable".to_string(),
                    ));
                }
                Ok(Err(error)) => return Err(error),
                Err(error) => {
                    return Err(PumasError::Other(format!(
                        "failed to observe admitted download resume persistence: {error}"
                    )));
                }
            }
        }

        // A pause requested while a recovery preflight is in blocking I/O
        // must settle before any further filesystem or network operation.
        // The generation/role check prevents an old Worker from overwriting a
        // concurrently installed cancellation finalizer.
        if pause_flag.load(Ordering::Relaxed) {
            #[cfg(test)]
            task_context.observe_worker_projection("pause-after-preflight");
            return Self::settle_worker_pause(
                &downloads,
                &download_publications,
                download_id,
                &task_context,
                persistence.as_ref(),
                &mut destination_guard,
            )
            .await;
        }

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

            // Ensure parent directory exists (needed for subdirectory files
            // like transformer/model.safetensors in diffusion repos)
            destination.prepare_file(&task_context, filename).await?;

            // Skip files that already exist (completed from previous run)
            if let Some(existing_size) = destination.file_len(&task_context, filename).await? {
                if destination
                    .part_len(&task_context, filename)
                    .await?
                    .is_some()
                {
                    if let Err(error) = destination.remove_part(&task_context, filename).await {
                        warn!(
                            "Failed to remove stale partial file for {}/{}: {}",
                            repo_id, filename, error
                        );
                    }
                }
                bytes_offset += existing_size;
                info!(
                    "Skipping already-downloaded file {}/{} ({} bytes)",
                    repo_id, filename, existing_size
                );

                // Update state
                #[cfg(test)]
                task_context.observe_worker_projection("before-existing-file-projection");
                {
                    let mut downloads = downloads.write().await;
                    let state = current_worker_state(
                        &mut downloads,
                        download_id,
                        &task_context,
                        &[DownloadStatus::Downloading],
                    )?;
                    state.files_completed = file_idx + 1;
                    state.downloaded_bytes = bytes_offset;
                    if let Some(total) = state.total_bytes {
                        state.progress = bytes_offset as f32 / total as f32;
                    }
                }
                #[cfg(test)]
                if file_idx + 1 == files.len() {
                    task_context.observe_worker_projection("terminal-cleanup-committed");
                }
                publish_worker_snapshot_and_revalidate(
                    &download_publications,
                    &downloads,
                    download_id,
                    &task_context,
                    destination,
                    &destination_lock,
                    &mut destination_guard,
                    &[DownloadStatus::Downloading],
                )
                .await?;
                continue;
            }

            // Fire aux-complete callback at the boundary between auxiliary and weight files.
            // Auxiliary files have size: None (non-LFS), weight files have size: Some (LFS).
            if !aux_callback_fired && file_info.size.is_some() {
                aux_callback_fired = true;
                if aux_complete_callback.is_some() || download_importer.is_some() {
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
                                    huggingface_evidence: state.huggingface_evidence.clone(),
                                })
                        })
                    };
                    if let Some(info) = info {
                        drop(destination_guard.take());
                        if let Some(importer) = download_importer.clone() {
                            let import_info = info.clone();
                            task_context
                                .run_fallible_async_named(
                                    "persist auxiliary download metadata",
                                    move || async move {
                                        importer.upsert_download_metadata_stub(&import_info).await
                                    },
                                )
                                .await
                                .map_err(|error| {
                                    PumasError::Other(format!(
                                        "Auxiliary metadata observation failed: {error}"
                                    ))
                                })??;
                        }
                        let callback_outcome = if let Some(callback) = aux_complete_callback.clone()
                        {
                            task_context
                                .run_fallible_blocking_named(
                                    "invoke auxiliary-files-complete callback",
                                    move || {
                                        std::panic::catch_unwind(AssertUnwindSafe(|| {
                                            callback(info)
                                        }))
                                        .map_err(|_| {
                                            "auxiliary-files-complete callback panicked".to_string()
                                        })
                                    },
                                )
                                .await
                        } else {
                            Ok(Ok(()))
                        };
                        destination_guard = Some(destination_lock.clone().lock_owned().await);

                        let still_current = {
                            let mut download_states = downloads.write().await;
                            current_worker_state(
                                &mut download_states,
                                download_id,
                                &task_context,
                                &[DownloadStatus::Downloading],
                            )
                            .is_ok_and(|state| {
                                !state.cancel_flag.load(Ordering::Relaxed)
                                    && state.matches_destination(&destination.identity())
                            })
                        };
                        if !still_current {
                            return Err(PumasError::DownloadCancelled);
                        }
                        match callback_outcome {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                return Err(PumasError::Other(format!(
                                    "auxiliary-files-complete callback failed: {error}"
                                )));
                            }
                            Err(error) => {
                                return Err(PumasError::Other(format!(
                                    "failed to observe auxiliary-files-complete callback: {error}"
                                )));
                            }
                        }
                    }
                }
            }

            // Update current filename in state
            {
                let mut downloads = downloads.write().await;
                let state = current_worker_state(
                    &mut downloads,
                    download_id,
                    &task_context,
                    &[DownloadStatus::Downloading],
                )?;
                state.filename = filename.clone();
                state.retry_attempt = 0;
                state.retry_limit = retry_limit;
                state.retrying = false;
                state.next_retry_delay_seconds = None;
            }
            publish_worker_snapshot_and_revalidate(
                &download_publications,
                &downloads,
                download_id,
                &task_context,
                destination,
                &destination_lock,
                &mut destination_guard,
                &[DownloadStatus::Downloading],
            )
            .await?;

            let download_base = HF_HUB_BASE;
            #[cfg(test)]
            let download_base = download_base_url.as_deref().unwrap_or(download_base);
            let url = format!("{}/{}/resolve/main/{}", download_base, repo_id, filename);

            let mut last_error: Option<PumasError> = None;

            let mut file_completed = false;
            let mut attempt: u32 = 0;
            let retry_started = Instant::now();
            loop {
                attempt += 1;
                {
                    let mut downloads = downloads.write().await;
                    let state = current_worker_state(
                        &mut downloads,
                        download_id,
                        &task_context,
                        &[DownloadStatus::Downloading],
                    )?;
                    state.retry_attempt = attempt;
                    state.retry_limit = retry_limit;
                    state.retrying = false;
                    state.next_retry_delay_seconds = None;
                }

                // Check cancellation before each attempt
                #[cfg(test)]
                task_context.observe_cancellation_check();
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = destination.remove_part(&task_context, filename).await;
                    // `cancel_download` has already generation-replaced this
                    // worker. Its caller-independent finalizer exclusively
                    // owns terminal state, persistence cleanup, and recovery
                    // capability release after observing this worker.
                    return Err(PumasError::DownloadCancelled);
                }

                // Check pause before each attempt
                if pause_flag.load(Ordering::Relaxed) {
                    #[cfg(test)]
                    task_context.observe_worker_projection("pause-before-attempt");
                    return Self::settle_worker_pause(
                        &downloads,
                        &download_publications,
                        download_id,
                        &task_context,
                        persistence.as_ref(),
                        &mut destination_guard,
                    )
                    .await;
                }

                // Determine resume offset from existing .part file
                let resume_from_byte = destination
                    .part_len(&task_context, filename)
                    .await?
                    .unwrap_or(0);

                if destination
                    .finalize_complete_part_file(&task_context, filename, file_info.size)
                    .await?
                {
                    file_completed = true;
                    break;
                }

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
                    project_worker_retry_reset(
                        &downloads,
                        download_id,
                        &task_context,
                        attempt,
                        retry_limit,
                    )
                    .await?;
                    publish_worker_snapshot_and_revalidate(
                        &download_publications,
                        &downloads,
                        download_id,
                        &task_context,
                        destination,
                        &destination_lock,
                        &mut destination_guard,
                        &[DownloadStatus::Downloading],
                    )
                    .await?;
                }

                match Self::download_attempt(
                    &client,
                    &downloads,
                    &download_publications,
                    &destination_lock,
                    &mut destination_guard,
                    download_id,
                    &url,
                    destination,
                    filename,
                    file_info.size,
                    resume_from_byte,
                    bytes_offset,
                    &cancel_flag,
                    &pause_flag,
                    persistence.as_ref(),
                    auth_header.as_deref(),
                    &task_context,
                )
                .await
                {
                    Ok(_) => {
                        #[cfg(test)]
                        task_context.observe_worker_projection("before-rename-pause-check");
                        if pause_flag.load(Ordering::Relaxed) {
                            #[cfg(test)]
                            task_context.observe_worker_projection("pause-before-rename");
                            return Self::settle_worker_pause(
                                &downloads,
                                &download_publications,
                                download_id,
                                &task_context,
                                persistence.as_ref(),
                                &mut destination_guard,
                            )
                            .await;
                        }
                        // Rename .part to final path atomically
                        destination
                            .rename_part_to_file(&task_context, filename)
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
                            return Err(e);
                        }

                        if !e.is_retryable() || cancel_flag.load(Ordering::Relaxed) {
                            if cancel_flag.load(Ordering::Relaxed) {
                                let _ = destination.remove_part(&task_context, filename).await;
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
                            let state = current_worker_state(
                                &mut downloads,
                                download_id,
                                &task_context,
                                &[DownloadStatus::Downloading],
                            )?;
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
                        publish_worker_snapshot_and_revalidate(
                            &download_publications,
                            &downloads,
                            download_id,
                            &task_context,
                            destination,
                            &destination_lock,
                            &mut destination_guard,
                            &[DownloadStatus::Downloading],
                        )
                        .await?;
                        debug!("Waiting {:?} before retry", delay);
                        tokio::select! {
                            biased;
                            _ = task_context.pause_requested(&pause_flag) => return Err(PumasError::DownloadPaused),
                            _ = tokio::time::sleep(delay) => {}
                        }
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
            let actual_size = destination
                .file_len(&task_context, filename)
                .await?
                .unwrap_or(file_info.size.unwrap_or(0));
            bytes_offset += actual_size;
            {
                let mut downloads = downloads.write().await;
                let state = current_worker_state(
                    &mut downloads,
                    download_id,
                    &task_context,
                    &[DownloadStatus::Downloading],
                )?;
                state.files_completed = file_idx + 1;
                state.downloaded_bytes = bytes_offset;
                state.retry_attempt = 0;
                state.retrying = false;
                state.next_retry_delay_seconds = None;
                state.error = None;
            }
            #[cfg(test)]
            if file_idx + 1 == files.len() {
                task_context.observe_worker_projection("terminal-cleanup-committed");
            }
            publish_worker_snapshot_and_revalidate(
                &download_publications,
                &downloads,
                download_id,
                &task_context,
                destination,
                &destination_lock,
                &mut destination_guard,
                &[DownloadStatus::Downloading],
            )
            .await?;

            info!(
                "File {}/{} complete ({}/{})",
                repo_id,
                filename,
                file_idx + 1,
                files.len()
            );
        }

        // Remove the marker through the same destination authority before
        // releasing a recovery capability from state. If this fails, the
        // download remains recoverable instead of becoming a false success.
        destination.remove_marker(&task_context).await?;

        let completion_info = downloads
            .read()
            .await
            .get(download_id)
            .and_then(download_completion_info);
        drop(destination_guard.take());
        import_completed_download(&download_importer, &task_context, completion_info).await?;
        destination_guard = Some(destination_lock.clone().lock_owned().await);
        {
            let mut states = downloads.write().await;
            current_worker_state(
                &mut states,
                download_id,
                &task_context,
                &[DownloadStatus::Downloading],
            )?;
        }

        // A durable queue release is an assertion about completed effects.
        // Observe their joins (including retained failures) before writing it.
        match task_context.drain_blocking().await {
            Ok(0) => {}
            Ok(failures) => {
                return Err(PumasError::Other(format!(
                    "Download effects failed before settlement: {failures}"
                )))
            }
            Err(error) => {
                return Err(PumasError::Other(format!(
                    "Download effect drain failed before settlement: {error}"
                )))
            }
        }

        // Persistence cleanup is part of successful completion. It is
        // registered with the same task owner and must finish before the final
        // drain, Completed projection, or recovery-capability release.
        let completion_admission = downloads.read().await.get(download_id).and_then(|state| {
            state
                .admission
                .as_ref()
                .map(|entry| entry.attempt_id.clone())
        });
        if let Some(persistence) = terminal_cleanup_persistence
            .as_ref()
            .filter(|_| completion_admission.is_some() || !destination.is_recovery())
        {
            let persistence = persistence.clone();
            let persisted_id = download_id.to_string();
            let attempt = completion_admission;
            match task_context
                .run_fallible_blocking_named("remove completed persisted download", move || {
                    if let Some(attempt) = attempt {
                        if persistence.settle_queue_admission(&persisted_id, &attempt)? {
                            Ok(())
                        } else {
                            Err(PumasError::Other(
                                "Completed download queue settlement was not confirmed".into(),
                            ))
                        }
                    } else {
                        Err(PumasError::Config {
                            message: "Ordinary completion requires durable admission".into(),
                        })
                    }
                })
                .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => return Err(error),
                Err(error) => {
                    return Err(PumasError::Other(format!(
                        "failed to observe completed-download persistence cleanup: {error}"
                    )));
                }
            }
        }

        let nested_failures = task_context.drain_blocking().await.map_err(|error| {
            PumasError::Other(format!(
                "failed to drain recovery filesystem operations before completion: {error}"
            ))
        })?;
        if nested_failures > 0 {
            return Err(PumasError::Other(format!(
                "recovery filesystem operations completed with {nested_failures} task failure(s)"
            )));
        }

        // All files completed -- update status and fire callback
        {
            let mut downloads = downloads.write().await;
            let state = current_worker_state(
                &mut downloads,
                download_id,
                &task_context,
                &[DownloadStatus::Downloading],
            )?;
            state.status = DownloadStatus::Completed;
            state.progress = 1.0;
            state.files_completed = files.len();
            state.task_registered = false;
            state.destination = None;
            state.lifecycle_failure_unverified = false;
        }
        drop(destination_guard.take());
        publish_download_snapshot_from_parts(&download_publications).await;

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
        download_publications: &Arc<DownloadPublicationOwner>,
        destination_lock: &Arc<TokioMutex<()>>,
        destination_guard: &mut Option<OwnedMutexGuard<()>>,
        download_id: &str,
        url: &str,
        destination: &DownloadDestination,
        filename: &str,
        file_size_expected: Option<u64>,
        resume_from_byte: u64,
        bytes_offset: u64,
        cancel_flag: &Arc<AtomicBool>,
        pause_flag: &Arc<AtomicBool>,
        persistence: Option<&Arc<DownloadPersistence>>,
        auth_header: Option<&str>,
        task_context: &TaskContext,
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

        let response = tokio::select! {
            biased;
            _ = task_context.pause_requested(pause_flag) => return Err(PumasError::DownloadPaused),
            response = request.send() => response,
        }
        .map_err(|e| PumasError::Network {
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
        let mut file = destination
            .open_part(task_context, filename, is_resuming)
            .await?;

        let mut downloaded: u64 = if is_resuming { resume_from_byte } else { 0 };
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();
        let mut last_publish = Instant::now();

        loop {
            let chunk = tokio::select! {
                biased;
                _ = task_context.pause_requested(pause_flag) => {
                    file.flush(task_context).await?;
                    return Err(PumasError::DownloadPaused);
                }
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else {
                break;
            };
            #[cfg(test)]
            task_context.observe_cancellation_check();
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                let _ = destination.remove_part(task_context, filename).await;
                // Terminal cancellation belongs to the generation-replacing
                // finalizer, which observes this worker and its nested work.
                return Err(PumasError::DownloadCancelled);
            }

            if pause_flag.load(Ordering::Relaxed) {
                file.flush(task_context).await?;
                drop(file);
                // Preserve .part file for resume

                #[cfg(test)]
                task_context.observe_worker_projection("pause-during-stream");
                return Self::settle_worker_pause(
                    downloads,
                    download_publications,
                    download_id,
                    task_context,
                    persistence,
                    destination_guard,
                )
                .await;
            }

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Download stream error: {}", e),
                cause: Some(e.to_string()),
            })?;

            file.write_all(task_context, &chunk).await?;
            downloaded += chunk.len() as u64;

            // Update overall progress (bytes_offset accounts for completed files)
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            };

            let overall_downloaded = bytes_offset + downloaded;

            let mut download_states = downloads.write().await;
            let state = current_worker_state(
                &mut download_states,
                download_id,
                task_context,
                &[DownloadStatus::Downloading],
            )?;
            state.downloaded_bytes = overall_downloaded;
            state.speed = speed;
            state.progress = if let Some(total) = state.total_bytes {
                overall_downloaded as f32 / total as f32
            } else {
                0.0
            };
            drop(download_states);

            if last_publish.elapsed() >= DOWNLOAD_PROGRESS_PUBLISH_INTERVAL {
                publish_worker_snapshot_and_revalidate(
                    download_publications,
                    downloads,
                    download_id,
                    task_context,
                    destination,
                    destination_lock,
                    destination_guard,
                    &[DownloadStatus::Downloading],
                )
                .await?;
                last_publish = Instant::now();
            }
        }

        file.flush(task_context).await?;
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

    async fn persist_status_update_owned(
        task_context: &TaskContext,
        persistence: Arc<DownloadPersistence>,
        download_id: String,
        status: DownloadStatus,
        admission_attempt: Option<String>,
    ) -> Result<bool> {
        match task_context
            .run_fallible_blocking_named("persist download status", move || {
                if let Some(attempt) = admission_attempt {
                    persistence.update_admitted_status(&download_id, &attempt, status)
                } else {
                    Err(PumasError::Config {
                        message: "Ordinary status persistence requires durable admission".into(),
                    })
                }
            })
            .await
        {
            Ok(result) => result,
            Err(error) => Err(PumasError::Other(format!(
                "failed to observe persisted download status: {error}"
            ))),
        }
    }

    async fn persisted_download_is_revoked(
        context: &TaskContext,
        persistence: Arc<DownloadPersistence>,
        download_id: String,
    ) -> Result<bool> {
        context
            .run_fallible_blocking_named("inspect persisted recovery authority", move || {
                persistence.is_revoked(&download_id)
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!(
                    "Failed to join persisted recovery authority check: {error}"
                ))
            })?
    }

    /// Get download progress.
    pub async fn get_download_progress(&self, download_id: &str) -> Option<ModelDownloadProgress> {
        self.reconcile_download_reads().await;
        let downloads = self.downloads.read().await;
        downloads.get(download_id).map(progress_from_state)
    }

    /// Cancel a download.
    pub async fn cancel_download(&self, download_id: &str) -> Result<bool> {
        let client = self.clone_for_invocation();
        let download_id = download_id.to_string();
        self.run_download_invocation(move |context| async move {
            let context = client.protect_download_mutation(&context).await?;
            client
                .cancel_download_admitted(&context, &download_id)
                .await
        })
        .await
    }

    async fn cancel_download_admitted(
        &self,
        context: &TaskContext,
        download_id: &str,
    ) -> Result<bool> {
        let finalizer = {
            let mut download_states = self.downloads.write().await;
            let Some(state) = download_states.get_mut(download_id) else {
                return Ok(false);
            };
            if matches!(
                state.status,
                DownloadStatus::Completed | DownloadStatus::Cancelled
            ) {
                return Ok(false);
            }
            if state.status == DownloadStatus::Cancelling
                && state.task_registered
                && self
                    .download_tasks
                    .snapshot(download_id)
                    .is_some_and(|task| {
                        task.started && task.role == TaskRole::CancelFinalizer && !task.finished
                    })
            {
                return Ok(true);
            }

            let worker_requires_terminal_projection = matches!(
                state.status,
                DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Pausing
            );
            let unverified_lifecycle_failure = state.lifecycle_failure_unverified;
            let admission_attempt = state
                .admission
                .as_ref()
                .map(|admission| admission.attempt_id.clone());
            let cleanup_destination =
                state
                    .destination
                    .clone()
                    .ok_or_else(|| PumasError::Config {
                        message: "Download cancellation authority is unavailable".into(),
                    })?;
            let cleanup_identity = cleanup_destination.identity();
            let cleanup_domain = cleanup_destination.domain();
            let cleanup_files = state
                .files
                .iter()
                .map(|file| file.filename.clone())
                .collect::<Vec<_>>();
            let downloads = self.downloads.clone();
            let download_publications = self.download_publications.clone();
            let destination_executions = self.destination_executions.clone();
            let persistence = self.persistence.clone();
            let finalizer_id = download_id.to_string();
            let cancellation_persistence = persistence.map(|store| CancellationPersistence {
                store,
                download_id: finalizer_id.clone(),
                domain: match cleanup_domain {
                    DestinationDomain::Ambient => LifecycleQuarantineDomain::Ambient,
                    DestinationDomain::Recovery => LifecycleQuarantineDomain::Recovery,
                },
                admission_attempt,
                revoked_snapshot: state.revoked_snapshot.clone(),
            });
            let protected_context = context.clone();
            let configured_root = self.destination_root.clone();
            let transition = self.download_tasks.begin_cancel(
                download_id,
                move |task_context, predecessor| async move {
                    use futures::FutureExt;
                    let mut task_context = task_context.inherit_root_grant(&protected_context);
                    drop(protected_context);

                    let waiting_identity = cleanup_destination.identity();
                    let waiting_generation = task_context.generation().clone();
                    let turn = destination_executions
                        .wait_for_turn(
                            &waiting_identity,
                            &finalizer_id,
                            cleanup_domain,
                            &waiting_generation,
                        );
                    tokio::pin!(turn);
                    let acquired = match futures::poll!(&mut turn) {
                        std::task::Poll::Ready(acquired) => acquired,
                        std::task::Poll::Pending => {
                            // The predecessor has already drained before this
                            // finalizer runs. A queued destination owns no effects.
                            task_context = task_context.without_root_grant();
                            turn.await
                        }
                    };
                    if !acquired {
                        return;
                    }
                    let protected = async {
                        if let Some(root) = configured_root {
                            task_context = task_context.with_root_grant(root).await?;
                        }
                        task_context.with_root_grant(cleanup_destination.capability().execution_root()).await
                    }.await;
                    let task_context = match protected {
                        Ok(context) => context,
                        Err(error) => {
                            Self::project_execution_refusal(
                                &downloads, &download_publications, &finalizer_id,
                                &task_context, TaskRole::CancelFinalizer, &error,
                            ).await;
                            return;
                        }
                    };
                    let destination_identity = cleanup_destination.identity();
                    let finalizer_generation = task_context.generation().clone();
                    let finalizer_outcome = std::panic::AssertUnwindSafe(async {
                        let predecessor_failed = matches!(
                            &predecessor,
                            CancelPredecessor::Observed(observation)
                                if observation.role == TaskRole::CancelFinalizer
                                    || (observation.role == TaskRole::Worker
                                        && observation.outer_finished_before_replacement
                                        && worker_requires_terminal_projection)
                                    || observation.terminal == TaskTerminal::Panicked
                                    || observation.nested_failures > 0
                        );
                        let quarantine = if let Some(persistence) = cancellation_persistence.as_ref() {
                            let persistence = persistence.clone();
                            task_context.run_fallible_blocking_named("quarantine download before cancellation", move || persistence.begin(unverified_lifecycle_failure || predecessor_failed)).await
                                .map_err(|error| PumasError::Other(format!("Cancellation quarantine owner failed: {error}"))).and_then(|result| result)
                        } else { Ok(None) };
                        let quarantine_failed = quarantine.is_err();
                        let quarantine = quarantine.ok().flatten();
                        let already_verified = quarantine.as_ref().is_some_and(|quarantine| quarantine.disposition == LifecycleCleanupDisposition::Verified);
                        let sticky_failure = unverified_lifecycle_failure || predecessor_failed || quarantine.as_ref().is_some_and(|quarantine| quarantine.sticky_failure);
                        let mut filesystem_cleanup_failed = false;
                        if !quarantine_failed && !already_verified {
                        for filename in &cleanup_files {
                            if cleanup_destination
                                .remove_part(&task_context, filename)
                                .await
                                .is_err()
                            {
                                filesystem_cleanup_failed = true;
                            }
                        }
                        if cleanup_destination
                            .remove_marker(&task_context)
                            .await
                            .is_err()
                        {
                            filesystem_cleanup_failed = true;
                        }
                        }

                        let effect_drain_failed = !matches!(task_context.drain_blocking().await, Ok(0));
                        let persistence_cleanup_failed = if !quarantine_failed && !filesystem_cleanup_failed && !effect_drain_failed {
                            if let Some(persistence) = cancellation_persistence.as_ref() {
                                let persistence = persistence.clone();
                                let quarantine = quarantine.clone();
                                !matches!(
                                    task_context
                                        .run_fallible_blocking_named(
                                            "remove persisted download during cancellation",
                                            move || persistence.finish(quarantine.as_ref()),
                                        )
                                        .await,
                                    Ok(Ok(()))
                                )
                            } else {
                                false
                            }
                        } else {
                            quarantine_failed
                        };
                        if quarantine.is_some() && (filesystem_cleanup_failed || effect_drain_failed || persistence_cleanup_failed) {
                            if let Some(persistence) = cancellation_persistence.as_ref() {
                                let persistence = persistence.clone();
                                let _ = task_context.run_fallible_blocking_named("mark cancelled download cleanup failure", move || persistence.mark_failed()).await;
                            }
                        }
                        // Join registered finalizer blocking work before exposing
                        // Cancelled or releasing the recovery capability.
                        let finalizer_drain = task_context.drain_blocking().await;
                        let finalizer_failed = effect_drain_failed || !matches!(finalizer_drain, Ok(0));
                        let cleanup_verified = !filesystem_cleanup_failed
                            && !persistence_cleanup_failed
                            && !finalizer_failed
                            && !quarantine_failed;
                        let failed = sticky_failure
                            || filesystem_cleanup_failed
                            || persistence_cleanup_failed
                            || finalizer_failed;

                        let projected = {
                            let mut states = downloads.write().await;
                            if let Some(state) = states.get_mut(&finalizer_id) {
                                if state.status == DownloadStatus::Cancelling {
                                    state.status = if failed {
                                        DownloadStatus::Error
                                    } else {
                                        DownloadStatus::Cancelled
                                    };
                                    state.task_registered = false;
                                    if failed {
                                        state.lifecycle_failure_unverified = true;
                                        state.error = Some(
                                            "download cancellation could not verify terminal cleanup"
                                                .to_string(),
                                        );
                                    } else {
                                        state.destination = None;
                                    }
                                    Some(failed)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };
                        let projected_failure = projected == Some(true);
                        #[cfg(test)]
                        if projected_failure {
                            task_context.observe_projection("cancel-finalizer-error");
                        }
                        publish_download_snapshot_from_parts(&download_publications).await;
                        let _ = task_context.complete_transferred_projection(projected_failure);
                        // Lifecycle provenance is intentionally sticky: a retry
                        // can verify idempotent cleanup and release destination
                        // authority without rewriting Error to Cancelled.
                        projected.is_some() && cleanup_verified
                    })
                    .catch_unwind()
                    .await;

                    match finalizer_outcome {
                        Ok(true) => {
                            destination_executions.release(
                                &destination_identity,
                                &finalizer_id,
                                cleanup_domain,
                                &finalizer_generation,
                            );
                        }
                        Ok(false) => {}
                        Err(payload) => std::panic::resume_unwind(payload),
                    }
                },
            );
            let finalizer = match transition? {
                super::lifecycle::CancelTransition::Started(finalizer)
                | super::lifecycle::CancelTransition::Existing(finalizer) => finalizer,
                super::lifecycle::CancelTransition::AlreadyRunning => return Ok(true),
            };
            let reservation_bound = self.destination_executions.reserve(
                cleanup_identity,
                download_id.to_string(),
                cleanup_domain,
                finalizer.generation().clone(),
            );
            if !reservation_bound {
                drop(finalizer);
                None
            } else {
                state.cancel_flag.store(true, Ordering::Relaxed);
                state.status = DownloadStatus::Cancelling;
                state.speed = 0.0;
                state.task_registered = true;
                Some(finalizer)
            }
        };
        let Some(finalizer) = finalizer else {
            self.download_tasks.rescue_abandoned();
            return Ok(false);
        };
        finalizer.start();
        self.publish_download_snapshot().await;
        Ok(true)
    }

    /// List all downloads (active, paused, completed, etc.).
    pub async fn list_downloads(&self) -> Vec<ModelDownloadProgress> {
        self.reconcile_download_reads().await;
        let downloads = self.downloads.read().await;
        downloads.values().map(progress_from_state).collect()
    }

    /// Snapshot all tracked downloads with a monotonic cursor.
    pub async fn download_snapshot(&self) -> crate::models::ModelDownloadSnapshot {
        self.reconcile_download_reads().await;
        let revision = self.download_revision.load(Ordering::SeqCst);
        build_download_snapshot_from_parts(&self.downloads, revision).await
    }

    /// Subscribe to download-state updates.
    pub fn subscribe_download_updates(
        &self,
    ) -> broadcast::Receiver<crate::models::ModelDownloadUpdateNotification> {
        self.download_updates.subscribe()
    }

    /// Build the recovery notification needed after a snapshot cursor.
    pub fn download_notification_since(
        &self,
        cursor: Option<&str>,
        snapshot: crate::models::ModelDownloadSnapshot,
    ) -> Option<crate::models::ModelDownloadUpdateNotification> {
        let requested = cursor.and_then(parse_download_update_cursor);
        let stale_cursor = cursor.is_some() && requested.is_none();
        let snapshot_required = requested
            .map(|revision| revision < snapshot.revision)
            .unwrap_or(true);

        if !snapshot_required && !stale_cursor {
            return None;
        }

        Some(crate::models::ModelDownloadUpdateNotification {
            cursor: snapshot.cursor.clone(),
            snapshot,
            stale_cursor,
            snapshot_required,
        })
    }

    async fn publish_download_snapshot(&self) {
        publish_download_snapshot_from_parts(&self.download_publications).await;
    }

    /// Find the download ID whose destination directory matches `dest_dir`.
    pub async fn find_download_id_by_dest_dir(&self, dest_dir: &Path) -> Option<String> {
        let client = self.clone_for_invocation();
        let path = dest_dir.to_path_buf();
        self.run_download_invocation(move |context| async move {
            let Some(root) = client.destination_root.clone() else {
                return Ok(None);
            };
            let destination = context
                .run_fallible_blocking_named("resolve download lookup destination", move || {
                    root.resolve(&path)
                })
                .await
                .map_err(|error| {
                    PumasError::Other(format!("Download lookup owner failed: {error}"))
                })??;
            let downloads = client.downloads.read().await;
            Ok(downloads
                .values()
                .find(|state| state.matches_destination(&destination.identity()))
                .map(|state| state.download_id.clone()))
        })
        .await
        .ok()
        .flatten()
    }

    /// Atomically admit a producer-verified partial-download recovery.
    ///
    /// This is the sole mutation owner for recovery: it either attaches to an
    /// exact tracked context, resumes that exact context, or inserts a new
    /// exact-set task while holding the download-state write lock. It never
    /// delegates to the generic destination-only dedupe path.
    pub(crate) async fn admit_recovery_download(
        &self,
        verified: &VerifiedDownloadRecovery,
        model_type: Option<String>,
    ) -> Result<RecoveryDownloadAdmission> {
        let client = self.clone_for_invocation();
        let verified = verified.clone();
        self.run_download_invocation(move |context| async move {
            client
                .admit_recovery_download_admitted(&context, &verified, model_type)
                .await
        })
        .await
    }

    async fn admit_recovery_download_admitted(
        &self,
        context: &TaskContext,
        verified: &VerifiedDownloadRecovery,
        model_type: Option<String>,
    ) -> Result<RecoveryDownloadAdmission> {
        let protected_context = self.protect_download_mutation(context).await?;
        let protected_context = protected_context
            .with_root_grant(verified.destination.execution_root())
            .await?;
        let context = &protected_context;
        self.observe_finished_download_tasks().await;
        let dest_dir = verified.destination.display_path();

        let destination = verified.destination.clone();
        let bound_files = verified.files.clone();
        let protected_context = context.clone();
        if self
            .run_download_invocation(move |task_context| async move {
                let task_context = task_context.inherit_root_grant(&protected_context);
                drop(protected_context);
                recovery_filesystem_operation(
                    &task_context,
                    "recovery admission preflight",
                    move || destination.preflight(&bound_files),
                )
                .await
            })
            .await
            .is_err()
        {
            return Ok(RecoveryDownloadAdmission::CapabilityUnavailable);
        }

        let metadata_client = self.clone_for_invocation();
        let repo_id = verified.repo_id.clone();
        let tree = context
            .run_fallible_async_named("load recovery repository files", move || async move {
                metadata_client.get_repo_files(&repo_id).await
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Recovery metadata observation failed: {error}"))
            })??;
        let Some(files) = resolve_exact_recovery_files(&tree, &verified.files) else {
            return Ok(RecoveryDownloadAdmission::BoundFilesUnavailable);
        };
        let Some((family, official_name)) = verified.repo_id.split_once('/') else {
            return Ok(RecoveryDownloadAdmission::ContextMismatch);
        };
        let request = DownloadRequest {
            repo_id: verified.repo_id.clone(),
            family: family.to_string(),
            official_name: official_name.to_string(),
            model_type,
            quant: None,
            filename: None,
            filenames: Some(verified.files.clone()),
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        let first_filename = files
            .first()
            .expect("verified recovery file set is nonempty")
            .filename
            .clone();
        let total_bytes = match files
            .iter()
            .filter_map(|file| file.size)
            .try_fold(0_u64, u64::checked_add)
        {
            Some(total) => (total > 0).then_some(total),
            None => return Ok(RecoveryDownloadAdmission::BoundFilesUnavailable),
        };
        let known_sha256 = files
            .iter()
            .max_by_key(|file| file.size.unwrap_or(0))
            .and_then(|file| file.sha256.clone());

        let launch_plan = {
            let downloads = self.downloads.read().await;
            match recovery_context(
                &downloads,
                &verified.destination.identity(),
                &verified.repo_id,
                &verified.files,
            ) {
                RecoveryContext::Exact { download_id, .. } => {
                    let state = downloads
                        .get(&download_id)
                        .expect("recovery context was resolved from this map");
                    let task = self.download_tasks.snapshot(&download_id);
                    if let Some(admission) = admitted_existing_recovery(state, task.as_ref()) {
                        return Ok(admission);
                    }
                    if !matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error) {
                        return Ok(RecoveryDownloadAdmission::ContextMismatch);
                    }
                    RecoveryLaunchPlan::Existing { download_id }
                }
                RecoveryContext::Mismatch => {
                    return Ok(RecoveryDownloadAdmission::ContextMismatch);
                }
                RecoveryContext::Missing => RecoveryLaunchPlan::New {
                    download_id: uuid::Uuid::new_v4().to_string(),
                },
            }
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let prepared_download = self
            .prepare_download_task(
                launch_plan.download_id().to_string(),
                verified.repo_id.clone(),
                files.clone(),
                DownloadDestination::Recovery(verified.destination.clone()),
                cancel_flag.clone(),
                pause_flag.clone(),
                None,
                None,
                None,
                self.persistence.clone(),
            )
            .await;

        if let RecoveryLaunchPlan::Existing { download_id } = &launch_plan {
            let admission_identity = super::lifecycle::PendingAdmissionIdentity {
                destination: verified.destination.identity(),
                repo_id: verified.repo_id.clone(),
                files: files
                    .iter()
                    .map(|file| (file.filename.clone(), file.size, file.sha256.clone()))
                    .collect(),
            };
            let (admission_completed, admission_completion) = tokio::sync::watch::channel(false);
            let transition_download_id = download_id.clone();
            let transition_repo_id = verified.repo_id.clone();
            let transition_bound_files = verified.files.clone();
            let transition_destination = verified.destination.clone();
            let transition_files = files.clone();
            let transition_first_filename = first_filename.clone();
            let transition_request = request.clone();
            let transition_known_sha256 = known_sha256.clone();
            let transition_cancel_flag = cancel_flag.clone();
            let transition_pause_flag = pause_flag.clone();
            let downloads = self.downloads.clone();
            let download_publications = self.download_publications.clone();
            let destination_executions = self.destination_executions.clone();
            let persistence = self.persistence.clone();
            let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
            let protected_context = context.clone();
            let prepared_transition = self.download_tasks.prepare(
                download_id.clone(),
                TaskRole::RecoveryTransition,
                move |task_context| async move {
                    let task_context = task_context.inherit_root_grant(&protected_context);
                    drop(protected_context);
                    let result = async {
                        if let Some(persistence) = persistence {
                            let persisted_id = transition_download_id.clone();
                            let inspection_store = persistence.clone();
                            let snapshot = task_context
                                .run_fallible_blocking_named(
                                    "capture persisted recovery snapshot",
                                    move || {
                                        Ok::<_, PumasError>(
                                            inspection_store
                                                .load_lifecycle_inventory_strict()?
                                                .downloads
                                                .into_iter()
                                                .find(|snapshot| {
                                                    snapshot.download_id == persisted_id
                                                }),
                                        )
                                    },
                                )
                                .await
                                .map_err(|error| {
                                    PumasError::Other(format!(
                                        "download recovery persistence task failed: {error}"
                                    ))
                                })??;
                            let (snapshot, admission_attempt) = {
                                let mut states = downloads.write().await;
                                if !task_context.is_current_role(TaskRole::RecoveryTransition) {
                                    return Ok(RecoveryDownloadAdmission::ContextMismatch);
                                }
                                let Some(state) = states.get_mut(&transition_download_id) else {
                                    return Ok(RecoveryDownloadAdmission::ContextMismatch);
                                };
                                if !state.matches_destination(&transition_destination.identity()) {
                                    return Ok(RecoveryDownloadAdmission::ContextMismatch);
                                }
                                let snapshot = snapshot.or_else(|| state.revoked_snapshot.clone());
                                if snapshot.is_some() {
                                    state.revoked_snapshot = snapshot.clone();
                                }
                                (snapshot, state.admission.as_ref().map(|entry| entry.attempt_id.clone()))
                            };
                            let persisted_id = transition_download_id.clone();
                            task_context
                                .run_fallible_blocking_named(
                                    "revoke persisted recovery authority",
                                    move || match (admission_attempt, snapshot) {
                                        (Some(attempt), Some(snapshot)) => persistence.revoke_admitted_for_recovery(
                                            &persisted_id, &attempt, &snapshot)?.into_result(),
                                        (None, None) => persistence.revoke(&persisted_id),
                                        _ => Err(PumasError::Validation {
                                            field: "download_recovery".into(),
                                            message: "Tracked recovery requires its exact admitted snapshot".into(),
                                        }),
                                    },
                                )
                                .await
                                .map_err(|error| {
                                    PumasError::Other(format!(
                                        "download recovery persistence task failed: {error}"
                                    ))
                                })??;
                        }

                        {
                            let mut states = downloads.write().await;
                            let exact = matches!(
                                recovery_context(
                                    &states,
                                    &transition_destination.identity(),
                                    &transition_repo_id,
                                    &transition_bound_files,
                                ),
                                RecoveryContext::Exact {
                                    download_id: current_id
                                } if current_id == transition_download_id
                            );
                            let inactive =
                                states.get(&transition_download_id).is_some_and(|state| {
                                    matches!(
                                        state.status,
                                        DownloadStatus::Paused | DownloadStatus::Error
                                    )
                                });
                            if !exact || !inactive || !task_context.promote_role(TaskRole::Worker) {
                                return Ok(RecoveryDownloadAdmission::ContextMismatch);
                            }
                            let reservation_promoted = destination_executions.promote_domain(
                                &transition_destination.identity(),
                                &transition_download_id,
                                DestinationDomain::Recovery,
                                DestinationDomain::Recovery,
                                task_context.generation(),
                            ) || destination_executions.promote_domain(
                                &transition_destination.identity(),
                                &transition_download_id,
                                DestinationDomain::Ambient,
                                DestinationDomain::Recovery,
                                task_context.generation(),
                            );
                            if !reservation_promoted {
                                return Ok(RecoveryDownloadAdmission::ContextMismatch);
                            }
                            let state = states
                                .get_mut(&transition_download_id)
                                .expect("exact recovery context remains present");
                            state.pause_flag = transition_pause_flag;
                            state.cancel_flag = transition_cancel_flag;
                            state.status = DownloadStatus::Queued;
                            state.error = None;
                            state.speed = 0.0;
                            state.retry_attempt = 0;
                            state.retrying = false;
                            state.next_retry_delay_seconds = None;
                            state.task_registered = true;
                            state.destination =
                                Some(DownloadDestination::Recovery(transition_destination));
                            state.filename = transition_first_filename;
                            state.files = transition_files;
                            state.files_completed = 0;
                            state.total_bytes = total_bytes;
                            state.download_request = Some(transition_request);
                            state.known_sha256 = transition_known_sha256;
                            state.huggingface_evidence = None;
                        }
                        publish_download_snapshot_from_parts(&download_publications).await;
                        Ok(RecoveryDownloadAdmission::Resumed {
                            download_id: transition_download_id.clone(),
                        })
                    }
                    .await;

                    let run_worker =
                        matches!(&result, Ok(RecoveryDownloadAdmission::Resumed { .. }));
                    let _ = result_sender.send(result);
                    if run_worker {
                        // Only a durably committed state/domain handoff is joinable.
                        admission_completed.send_replace(true);
                        let _ = prepared_download.run_owned(task_context).await;
                    }
                },
            )?;

            let mut pending_admission = None;
            let mut existing_admission = None;
            let transition_install = {
                let mut states = self.downloads.write().await;
                let exact = matches!(
                    recovery_context(&states, &verified.destination.identity(), &verified.repo_id, &verified.files),
                    RecoveryContext::Exact {
                        download_id: current_id
                    } if current_id == *download_id
                );
                let inactive = states.get(download_id).is_some_and(|state| {
                    matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                });
                if exact {
                    // Recheck under installation's state lock: another caller
                    // may have committed or installed since launch planning.
                    existing_admission = states.get(download_id).and_then(|state| {
                        admitted_existing_recovery(
                            state,
                            self.download_tasks.snapshot(download_id).as_ref(),
                        )
                    });
                    pending_admission = self
                        .download_tasks
                        .pending_recovery_admission(download_id, &admission_identity);
                }
                if exact && inactive && pending_admission.is_none() && existing_admission.is_none()
                {
                    if let Ok(installed) = self.download_tasks.install_gated(prepared_transition) {
                        let current_domain = if states
                            .get(download_id)
                            .is_some_and(|state| state.recovery_destination().is_some())
                        {
                            DestinationDomain::Recovery
                        } else {
                            DestinationDomain::Ambient
                        };
                        let reserved = self.destination_executions.reserve(
                            verified.destination.identity(),
                            download_id.clone(),
                            current_domain,
                            installed.generation().clone(),
                        );
                        if reserved {
                            self.download_tasks.bind_pending_admission(
                                download_id,
                                installed.generation(),
                                admission_identity,
                                admission_completion,
                            );
                            states
                                .get_mut(download_id)
                                .expect("exact recovery state remains present")
                                .ambient_authority_blocked = true;
                            let generation = installed.generation().clone();
                            Some((generation, installed))
                        } else {
                            drop(installed);
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    drop(prepared_transition);
                    None
                }
            };
            let Some((transition_generation, installed_transition)) = transition_install else {
                self.download_tasks.rescue_abandoned();
                if let Some(admission) = existing_admission {
                    return Ok(admission);
                }
                if let Some((generation, mut completion)) = pending_admission {
                    completion
                        .wait_for(|completed| *completed)
                        .await
                        .map_err(|_| {
                            PumasError::Other("Concurrent recovery admission did not commit".into())
                        })?;
                    #[cfg(test)]
                    self.download_tasks
                        .observe_ambient_admission("recovery-handoff-observed", download_id);
                    let states = self.downloads.read().await;
                    // A committed worker can release its capability and finish before
                    // this waiter is polled again. Its existing release record proves
                    // only this terminal observation, never a new attachment.
                    let generation_current = self
                        .download_tasks
                        .generation_is_current(download_id, &generation);
                    let terminal_proof = generation_current
                        || self.destination_executions.was_released(
                            &verified.destination.identity(),
                            download_id,
                            DestinationDomain::Recovery,
                        );
                    if terminal_proof {
                        if let Some(state) = states.get(download_id).filter(|state| {
                            state.download_id == *download_id
                                && recovery_selection_matches(
                                    state,
                                    &verified.repo_id,
                                    &verified.files.iter().map(String::as_str).collect(),
                                )
                                && matches!(
                                    state.status,
                                    DownloadStatus::Completed | DownloadStatus::Cancelled
                                )
                        }) {
                            if let Some(admission) = admitted_existing_recovery(state, None) {
                                return Ok(admission);
                            }
                        }
                    }
                    let exact = matches!(recovery_context(&states, &verified.destination.identity(), &verified.repo_id, &verified.files),
                        RecoveryContext::Exact { download_id: current_id } if current_id == *download_id);
                    if exact && generation_current {
                        if let Some(admission) = states.get(download_id).and_then(|state| {
                            admitted_existing_recovery(
                                state,
                                self.download_tasks.snapshot(download_id).as_ref(),
                            )
                        }) {
                            return Ok(admission);
                        }
                    }
                }
                return Ok(RecoveryDownloadAdmission::ContextMismatch);
            };
            installed_transition.start();

            let result = result_receiver.await.map_err(|_| {
                PumasError::Other(
                    "download recovery transition ended without a terminal result".to_string(),
                )
            })?;
            if !matches!(&result, Ok(RecoveryDownloadAdmission::Resumed { .. })) {
                while self
                    .download_tasks
                    .generation_is_current(download_id, &transition_generation)
                {
                    // Route the transition through the same owner-held
                    // TerminalProjection as every other finished task. A
                    // semantic revocation failure is archived by the nested
                    // blocking owner and must not be discarded here.
                    self.observe_finished_download_tasks().await;
                    tokio::task::yield_now().await;
                }
            }
            return result;
        }

        let RecoveryLaunchPlan::New { download_id } = launch_plan else {
            unreachable!("existing recovery returned from its transition owner")
        };
        let prepared = prepared_download.prepare_owned(
            &self.download_tasks,
            TaskRole::Worker,
            Some(context.clone()),
        )?;
        let installed = {
            let mut downloads = self.downloads.write().await;
            if matches!(
                recovery_context(
                    &downloads,
                    &verified.destination.identity(),
                    &verified.repo_id,
                    &verified.files
                ),
                RecoveryContext::Missing
            ) {
                if let Ok(installed) = self.download_tasks.install_gated(prepared) {
                    if !self.destination_executions.reserve(
                        verified.destination.identity(),
                        download_id.clone(),
                        DestinationDomain::Recovery,
                        installed.generation().clone(),
                    ) {
                        drop(installed);
                        drop(downloads);
                        self.download_tasks.rescue_abandoned();
                        return Ok(RecoveryDownloadAdmission::ContextMismatch);
                    }
                    downloads.insert(
                        download_id.clone(),
                        DownloadState {
                            download_id: download_id.clone(),
                            repo_id: verified.repo_id.clone(),
                            status: DownloadStatus::Queued,
                            progress: 0.0,
                            downloaded_bytes: 0,
                            total_bytes,
                            speed: 0.0,
                            cancel_flag,
                            pause_flag,
                            error: None,
                            retry_attempt: 0,
                            retry_limit: None,
                            retrying: false,
                            next_retry_delay_seconds: None,
                            task_registered: true,
                            lifecycle_failure_unverified: false,
                            dest_dir: dest_dir.to_path_buf(),
                            ambient_authority_blocked: true,
                            admission: None,
                            revoked_snapshot: None,
                            destination: Some(DownloadDestination::Recovery(
                                verified.destination.clone(),
                            )),
                            filename: first_filename,
                            files,
                            files_completed: 0,
                            download_request: Some(request),
                            known_sha256,
                            huggingface_evidence: None,
                        },
                    );
                    Some(installed)
                } else {
                    None
                }
            } else {
                None
            }
        };
        let Some(installed) = installed else {
            self.download_tasks.rescue_abandoned();
            return Ok(RecoveryDownloadAdmission::ContextMismatch);
        };
        installed.start();
        self.publish_download_snapshot().await;
        Ok(RecoveryDownloadAdmission::Recovered { download_id })
    }

    /// Get the current in-memory status for a download ID.
    pub async fn get_download_status(&self, download_id: &str) -> Option<DownloadStatus> {
        let downloads = self.downloads.read().await;
        downloads.get(download_id).map(|state| state.status)
    }

    /// Pause an active download. Preserves the `.part` file for later resume.
    pub async fn pause_download(&self, download_id: &str) -> Result<bool> {
        let client = self.clone_for_invocation();
        let download_id = download_id.to_string();
        self.run_download_invocation(move |context| async move {
            let _context = client.protect_download_mutation(&context).await?;
            client.pause_download_admitted(&download_id).await
        })
        .await
    }

    async fn pause_download_admitted(&self, download_id: &str) -> Result<bool> {
        let generation = {
            let mut downloads = self.downloads.write().await;
            let Some(generation) = self.download_tasks.active_worker_generation(download_id) else {
                return Ok(false);
            };
            let Some(state) = downloads.get_mut(download_id) else {
                return Ok(false);
            };
            if !matches!(
                state.status,
                DownloadStatus::Downloading | DownloadStatus::Queued
            ) || state.files_completed >= state.files.len()
            {
                return Ok(false);
            }
            state.pause_flag.store(true, Ordering::Release);
            state.status = DownloadStatus::Pausing;
            generation
        };
        generation.wake_pause();
        self.publish_download_snapshot().await;
        Ok(true)
    }

    /// Resume a paused or errored download from its `.part` file.
    pub async fn resume_download(&self, download_id: &str) -> Result<bool> {
        let client = self.clone_for_invocation();
        let download_id = download_id.to_string();
        self.run_download_invocation(move |context| async move {
            client
                .resume_download_admitted(&context, &download_id)
                .await
        })
        .await
    }

    async fn resume_download_admitted(
        &self,
        context: &TaskContext,
        download_id: &str,
    ) -> Result<bool> {
        let protected_context = self.protect_download_mutation(context).await?;
        let context = &protected_context;
        self.observe_finished_download_tasks().await;
        if self
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.role == TaskRole::RecoveryTransition)
        {
            return Ok(false);
        }

        let recovery_resume = {
            let downloads = self.downloads.read().await;
            downloads.get(download_id).and_then(|state| {
                if matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error) {
                    state.recovery_destination().cloned().map(|destination| {
                        (state.repo_id.clone(), state.files.clone(), destination)
                    })
                } else {
                    None
                }
            })
        };

        if let Some((repo_id, files, recovery_destination)) = recovery_resume {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let pause_flag = Arc::new(AtomicBool::new(false));
            let prepared_download = self
                .prepare_download_task(
                    download_id.to_string(),
                    repo_id.clone(),
                    files.clone(),
                    DownloadDestination::Recovery(recovery_destination.clone()),
                    cancel_flag.clone(),
                    pause_flag.clone(),
                    None,
                    None,
                    None,
                    self.persistence.clone(),
                )
                .await;
            let installed = {
                let mut downloads = self.downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    let same_files = state.files.len() == files.len()
                        && state.files.iter().zip(&files).all(|(left, right)| {
                            left.filename == right.filename
                                && left.size == right.size
                                && left.sha256 == right.sha256
                        });
                    let same_recovery = state.repo_id == repo_id
                        && same_files
                        && state.recovery_destination().is_some_and(|destination| {
                            destination.identity() == recovery_destination.identity()
                        });
                    if matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                        && same_recovery
                        && !self.download_tasks.contains(download_id)
                    {
                        let prepared = prepared_download.prepare_owned(
                            &self.download_tasks,
                            TaskRole::Worker,
                            Some(context.clone()),
                        )?;
                        if let Ok(installed) = self.download_tasks.install_gated(prepared) {
                            if !self.destination_executions.reserve(
                                recovery_destination.identity(),
                                download_id.to_string(),
                                DestinationDomain::Recovery,
                                installed.generation().clone(),
                            ) {
                                drop(installed);
                                drop(downloads);
                                self.download_tasks.rescue_abandoned();
                                return Ok(false);
                            }
                            state.pause_flag = pause_flag;
                            state.cancel_flag = cancel_flag;
                            state.status = DownloadStatus::Queued;
                            state.error = None;
                            state.speed = 0.0;
                            state.task_registered = true;
                            Some(installed)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            let Some(installed) = installed else {
                self.download_tasks.rescue_abandoned();
                return Ok(false);
            };
            installed.start();
            #[cfg(test)]
            self.download_tasks
                .observe_ambient_admission("resume-started", download_id);
            self.publish_download_snapshot().await;
            return Ok(true);
        }

        if let Some(persistence) = self.persistence.clone() {
            if Self::persisted_download_is_revoked(context, persistence, download_id.to_string())
                .await?
            {
                return Ok(false);
            }
        }
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("resume", download_id);

        let (repo_id, files, admission, destination) = {
            let downloads = self.downloads.read().await;
            let Some(state) = downloads.get(download_id) else {
                return Ok(false);
            };
            if state.ambient_authority_blocked
                || !matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                || state.recovery_destination().is_some()
            {
                return Ok(false);
            }
            if state.admission.is_none() || self.persistence.is_none() {
                return Err(PumasError::Config {
                    message: "Ordinary resume requires current durable admission".into(),
                });
            }
            (
                state.repo_id.clone(),
                state.files.clone(),
                state.admission.clone(),
                state
                    .destination
                    .clone()
                    .ok_or_else(|| PumasError::Config {
                        message: "Download resume authority is unavailable".into(),
                    })?,
            )
        };
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let mut prepared_download = self
            .prepare_download_task(
                download_id.to_string(),
                repo_id.clone(),
                files.clone(),
                destination.clone(),
                cancel_flag.clone(),
                pause_flag.clone(),
                self.completion_callback.clone(),
                self.aux_complete_callback.clone(),
                self.persistence.clone(),
                self.persistence.clone(),
            )
            .await;
        prepared_download.persist_queued_status = self.persistence.is_some();
        let installed = {
            let mut downloads = self.downloads.write().await;
            let Some(state) = downloads.get_mut(download_id) else {
                return Ok(false);
            };
            let same_files = state.files.len() == files.len()
                && state.files.iter().zip(&files).all(|(left, right)| {
                    left.filename == right.filename
                        && left.size == right.size
                        && left.sha256 == right.sha256
                });
            let same_context = state.repo_id == repo_id
                && state.matches_destination(&destination.identity())
                && state.admission.as_ref().map(|value| &value.attempt_id)
                    == admission.as_ref().map(|value| &value.attempt_id)
                && same_files
                && state.recovery_destination().is_none();
            if matches!(state.status, DownloadStatus::Paused | DownloadStatus::Error)
                && !state.ambient_authority_blocked
                && same_context
                && !self.download_tasks.contains(download_id)
            {
                let prepared = prepared_download.prepare_owned(
                    &self.download_tasks,
                    TaskRole::Worker,
                    Some(context.clone()),
                )?;
                match self.download_tasks.install_gated(prepared) {
                    Ok(installed) => {
                        if !self.destination_executions.reserve(
                            destination.identity(),
                            download_id.to_string(),
                            DestinationDomain::Ambient,
                            installed.generation().clone(),
                        ) {
                            drop(installed);
                            drop(downloads);
                            self.download_tasks.rescue_abandoned();
                            return Ok(false);
                        }
                        state.pause_flag = pause_flag;
                        state.cancel_flag = cancel_flag;
                        state.status = DownloadStatus::Queued;
                        state.error = None;
                        state.speed = 0.0;
                        state.task_registered = true;
                        Some(installed)
                    }
                    Err(rejected) => {
                        drop(rejected);
                        None
                    }
                }
            } else {
                None
            }
        };
        let Some(installed) = installed else {
            self.download_tasks.rescue_abandoned();
            return Ok(false);
        };
        installed.start();
        #[cfg(test)]
        self.download_tasks
            .observe_ambient_admission("resume-started", download_id);
        self.publish_download_snapshot().await;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    fn admit_snapshot_fixture(
        store: &DownloadPersistence,
        snapshot: &PersistedDownload,
        destination: &crate::model_library::DownloadRecoveryDestination,
    ) -> String {
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let files = if snapshot.filenames.is_empty() {
            vec![snapshot.filename.clone()]
        } else {
            snapshot.filenames.clone()
        };
        let request = DownloadAdmissionRequest {
            snapshot: snapshot.clone(),
            domain: DownloadAdmissionDomain::Ambient,
            destination: destination.persisted_identity().unwrap(),
            requested_payload_files: files.clone(),
            execution_files: files,
        };
        store
            .admit_download(&attempt_id, &request)
            .unwrap()
            .into_result()
            .unwrap();
        attempt_id
    }

    fn persist_state_fixture(store: &DownloadPersistence, state: &mut DownloadState) {
        let attempt_id = admit_snapshot_fixture(
            store,
            &persisted_recovery_test_state(state),
            state.destination.as_ref().unwrap().capability(),
        );
        state.admission = Some(super::super::types::AdmittedDownload { attempt_id });
    }

    fn admit_snapshot_at_root(
        store: &DownloadPersistence,
        snapshot: &PersistedDownload,
        root: &Path,
    ) -> String {
        let held = crate::model_library::download_recovery::DownloadDestinationRoot::open(root)
            .unwrap()
            .resolve(&snapshot.dest_dir)
            .unwrap();
        admit_snapshot_fixture(store, snapshot, &held)
    }

    fn revoke_state_fixture(store: &DownloadPersistence, state: &mut DownloadState) {
        let snapshot = persisted_recovery_test_state(state);
        store
            .revoke_admitted_for_recovery(
                &state.download_id,
                &state.admission.as_ref().unwrap().attempt_id,
                &snapshot,
            )
            .unwrap()
            .into_result()
            .unwrap();
        state.revoked_snapshot = Some(snapshot);
    }

    fn destination_identity(
        client: &HuggingFaceClient,
        path: &Path,
    ) -> crate::model_library::download_recovery::DestinationIdentity {
        client
            .destination_root
            .as_ref()
            .expect("fixture must configure its download root")
            .resolve(path)
            .unwrap()
            .identity()
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn restored_admitted_root_alias_blocks_a_canonical_successor() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let alias = temp.path().join("library-alias");
        let destination = root.join("model");
        std::fs::create_dir_all(&destination).unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        std::fs::write(destination.join("first.gguf.part"), b"old").unwrap();
        // Stops an incorrectly admitted successor before any network access.
        std::fs::create_dir(destination.join(".pumas_download")).unwrap();
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        client.configure_download_destination_root(&root).unwrap();
        let head_id = uuid::Uuid::new_v4().to_string();
        admit_snapshot_at_root(
            client.persistence.as_ref().unwrap(),
            &PersistedDownload {
                download_id: head_id.clone(),
                repo_id: "acme/first".into(),
                filename: "first.gguf".into(),
                filenames: vec!["first.gguf".into()],
                dest_dir: alias.join("model"),
                total_bytes: Some(8),
                status: DownloadStatus::Paused,
                download_request: recovery_test_request("acme/first", &["first.gguf".into()]),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
                huggingface_evidence: None,
            },
            &root,
        );
        client.restore_persisted_downloads().await.unwrap();
        assert_eq!(
            client.get_download_status(&head_id).await,
            Some(DownloadStatus::Paused)
        );
        cache_repo_tree(
            &client,
            "acme/second",
            vec![LfsFileInfo {
                filename: "second.gguf".into(),
                size: 8,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let (entered, entered_receiver) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered));
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "prepare ambient destination" {
                    if let Some(sender) = entered.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                }
            })));
        let successor = client
            .start_download(
                &recovery_test_request("acme/second", &["second.gguf".into()]),
                &destination,
                None,
            )
            .await
            .unwrap();
        let bypassed = tokio::time::timeout(Duration::from_millis(250), entered_receiver)
            .await
            .is_ok();
        client.download_tasks.set_blocking_observer(None);
        assert!(
            !bypassed,
            "a supported root alias must not bypass the restored dormant owner"
        );
        assert_eq!(
            client.get_download_status(&successor).await,
            Some(DownloadStatus::Queued)
        );
        assert_eq!(
            std::fs::read(destination.join("first.gguf.part")).unwrap(),
            b"old"
        );
        std::fs::remove_dir(destination.join(".pumas_download")).unwrap();
        // Known final bytes keep this queue-order oracle independent of HTTP.
        std::fs::write(destination.join("second.gguf"), b"complete").unwrap();
        assert!(client.cancel_download(&head_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(&head_id).await == Some(DownloadStatus::Cancelled)
                    && client.get_download_status(&successor).await
                        == Some(DownloadStatus::Completed)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("exact incumbent cancellation must release the canonical successor");
        assert!(!destination.join("first.gguf.part").exists());
    }
    #[tokio::test]
    async fn queued_pause_preserves_destination_and_restarts_at_its_fifo_position() {
        assert_queued_pause_resume_preserves_marker(true, false).await;
    }

    #[tokio::test]
    async fn queued_pause_resumes_with_its_marker_in_the_same_client() {
        assert_queued_pause_resume_preserves_marker(false, false).await;
    }

    #[tokio::test]
    async fn restored_implicit_selection_preserves_queued_marker_until_its_turn() {
        assert_queued_pause_resume_preserves_marker(true, true).await;
    }

    async fn assert_queued_pause_resume_preserves_marker(restart: bool, implicit: bool) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("model");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("first.gguf.part"), b"old").unwrap();
        let marker = br#"{"repo_id":"acme/first","sentinel":"untouched"}"#;
        std::fs::write(destination.join(".pumas_download"), marker).unwrap();
        let client = configured_download_client(temp.path().join("cache")).unwrap();
        let head = uuid::Uuid::new_v4().to_string();
        admit_snapshot_at_root(
            client.persistence.as_ref().unwrap(),
            &PersistedDownload {
                download_id: head.clone(),
                repo_id: "acme/first".into(),
                filename: "first.gguf".into(),
                filenames: vec!["first.gguf".into()],
                dest_dir: destination.clone(),
                total_bytes: Some(8),
                status: DownloadStatus::Paused,
                download_request: recovery_test_request("acme/first", &["first.gguf".into()]),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
                huggingface_evidence: None,
            },
            temp.path(),
        );
        client.restore_persisted_downloads().await.unwrap();
        cache_repo_tree(
            &client,
            "acme/second",
            vec![LfsFileInfo {
                filename: "second.gguf".into(),
                size: 8,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let mut successor_request = recovery_test_request("acme/second", &["second.gguf".into()]);
        if implicit {
            successor_request.filename = None;
            successor_request.filenames = None;
        }
        let successor = client
            .start_download(&successor_request, &destination, None)
            .await
            .unwrap();
        assert_eq!(
            client.get_download_status(&successor).await,
            Some(DownloadStatus::Queued)
        );
        assert!(client.pause_download(&successor).await.unwrap());
        let paused = tokio::time::timeout(Duration::from_millis(500), async {
            while client.get_download_status(&successor).await != Some(DownloadStatus::Paused) {
                tokio::task::yield_now().await;
            }
        })
        .await;
        if paused.is_err() {
            client.cancel_download(&successor).await.unwrap();
            client.cancel_download(&head).await.unwrap();
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while client
                .download_tasks
                .snapshot(&successor)
                .is_some_and(|task| !task.finished)
                || client
                    .download_tasks
                    .snapshot(&head)
                    .is_some_and(|task| !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            paused.is_ok(),
            "queued pause must settle while the dormant head retains its claim"
        );
        assert_eq!(
            std::fs::read(destination.join(".pumas_download")).unwrap(),
            marker
        );
        assert_eq!(
            std::fs::read(destination.join("first.gguf.part")).unwrap(),
            b"old"
        );
        assert!(!destination.join("second.gguf.part").exists());
        let mut restarted = if restart {
            drop(client);
            let restored = configured_download_client(temp.path().join("cache")).unwrap();
            restored.restore_persisted_downloads().await.unwrap();
            restored
        } else {
            client
        };
        assert_eq!(
            restarted.get_download_status(&successor).await,
            Some(DownloadStatus::Paused)
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        restarted.set_test_download_base_url(format!("http://{}", listener.local_addr().unwrap()));
        let (requested_sender, mut requested) = tokio::sync::oneshot::channel();
        let (release_sender, release) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                assert!(headers.len() < 4096);
                headers.push(socket.read_u8().await.unwrap());
            }
            requested_sender.send(()).unwrap();
            release.await.unwrap();
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ncomplete",
                )
                .await
                .unwrap();
        });
        assert!(restarted.resume_download(&successor).await.unwrap());
        assert_eq!(
            restarted.get_download_status(&successor).await,
            Some(DownloadStatus::Queued)
        );
        let bypassed = tokio::time::timeout(Duration::from_millis(250), &mut requested)
            .await
            .is_ok();
        let unchanged_marker = std::fs::read(destination.join(".pumas_download")).ok();
        assert!(restarted.cancel_download(&head).await.unwrap());
        if !bypassed {
            tokio::time::timeout(Duration::from_secs(3), requested)
                .await
                .unwrap()
                .unwrap();
        }
        let resumed_marker = std::fs::read(destination.join(".pumas_download"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        release_sender.send(()).unwrap();
        server.await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while restarted.get_download_status(&successor).await != Some(DownloadStatus::Completed)
                || restarted
                    .download_tasks
                    .snapshot(&successor)
                    .is_some_and(|task| !task.finished)
                || restarted
                    .download_tasks
                    .snapshot(&head)
                    .is_some_and(|task| !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            restarted.get_download_status(&head).await,
            Some(DownloadStatus::Cancelled)
        );
        assert!(
            !bypassed,
            "resumed successor must wait for the dormant head"
        );
        assert_eq!(unchanged_marker.as_deref(), Some(marker.as_slice()));
        let resumed_marker = resumed_marker.expect("successor must publish its marker before HTTP");
        assert_eq!(resumed_marker["repo_id"], "acme/second");
        assert_eq!(
            resumed_marker["selected_artifact"]["selected_filenames"],
            serde_json::json!(["second.gguf"])
        );
    }

    #[tokio::test]
    async fn transferred_partial_survives_interrupted_response_and_fresh_owner_cancel() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 4096, "fixture request exceeded header limit");
                header.push(socket.read_u8().await.unwrap());
            }
            String::from_utf8(header).unwrap()
        }
        let temp = TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (interrupt, interrupted) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut socket)
                .await
                .starts_with("GET /acme/model/resolve/main/weights.gguf HTTP/1.1"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\npartial",
                )
                .await
                .unwrap();
            interrupted.await.unwrap();
            socket.shutdown().await.unwrap();
            drop(socket);
            // A terminal HTTP refusal ends retries after the interrupted body.
            let (mut retry, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut retry)
                .await
                .to_ascii_lowercase()
                .contains("range: bytes=7-"));
            retry
                .write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            retry.shutdown().await.unwrap();
        });
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        client.download_base_url = Some(format!("http://{address}"));
        *client.auth_token.write().await = None;
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 12,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let destination = temp.path().join("library/model");
        let id = client
            .start_download(
                &recovery_test_request("acme/model", &["weights.gguf".into()]),
                &destination,
                None,
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if client
                    .list_downloads()
                    .await
                    .iter()
                    .any(|entry| entry.download_id == id && entry.downloaded_bytes == Some(7))
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        interrupt.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if client
                    .list_downloads()
                    .await
                    .iter()
                    .any(|entry| entry.download_id == id && entry.status == DownloadStatus::Error)
                    && !client.download_tasks.contains(&id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(
            std::fs::read(destination.join("weights.gguf.part")).unwrap(),
            b"partial"
        );
        assert!(!destination.join("weights.gguf").exists());
        drop(client);
        let restarted = configured_download_client(temp.path().join("cache")).unwrap();
        assert!(restarted
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        let progress = restarted
            .list_downloads()
            .await
            .into_iter()
            .find(|entry| entry.download_id == id)
            .unwrap();
        assert_eq!(progress.downloaded_bytes, Some(7));
        assert_eq!(progress.total_bytes, Some(12));
        assert!(restarted.cancel_download(&id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if restarted.get_download_status(&id).await == Some(DownloadStatus::Cancelled) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!destination.join("weights.gguf.part").exists());
        assert!(!destination.join(".pumas_download").exists());
        drop(restarted);
        let after_cancel = configured_download_client(temp.path().join("cache")).unwrap();
        assert!(after_cancel
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        assert!(after_cancel.list_downloads().await.is_empty());
    }

    #[tokio::test]
    async fn pause_settles_while_response_headers_are_stalled_without_losing_partial() {
        assert_pause_settles_during_stalled_response(StalledResponse::Headers).await;
    }

    #[tokio::test]
    async fn pause_settles_while_response_body_is_stalled_without_losing_transferred_bytes() {
        assert_pause_settles_during_stalled_response(StalledResponse::Body).await;
    }

    #[tokio::test]
    async fn pause_settles_during_retry_backoff_without_another_request() {
        assert_pause_settles_during_stalled_response(StalledResponse::Retry).await;
    }

    #[tokio::test]
    async fn cancellation_drains_stalled_body_pause_persistence_without_stale_paused() {
        assert_pause_settles_during_stalled_response(StalledResponse::CancelDuringPause).await;
    }

    #[tokio::test]
    async fn immediately_resuming_stalled_body_pause_transfers_remaining_range() {
        assert_pause_settles_during_stalled_response(StalledResponse::ImmediateResume).await;
    }

    #[derive(Clone, Copy, Debug)]
    enum StalledResponse {
        Headers,
        Body,
        Retry,
        CancelDuringPause,
        ImmediateResume,
    }

    async fn assert_pause_settles_during_stalled_response(stall: StalledResponse) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let stall_body = !matches!(stall, StalledResponse::Headers);
        let retry = matches!(stall, StalledResponse::Retry);
        let temp = TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (requested_sender, requested) = tokio::sync::oneshot::channel();
        let (release_sender, release) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                assert!(headers.len() < 4096);
                headers.push(socket.read_u8().await.unwrap());
            }
            assert!(String::from_utf8(headers)
                .unwrap()
                .to_ascii_lowercase()
                .contains("range: bytes=3-"));
            if stall_body {
                socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 3-7/8\r\nConnection: close\r\n\r\nde").await.unwrap();
            }
            requested_sender.send(()).unwrap();
            if retry {
                drop(socket);
                release.await.unwrap();
                return tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err();
            }
            release.await.unwrap();
            if !stall_body {
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
            }
            true
        });
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        client.set_test_download_base_url(format!("http://{address}"));
        *client.auth_token.write().await = None;
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 8,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let destination = temp.path().join("model");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("weights.gguf.part"), b"abc").unwrap();
        let id = client
            .start_download(
                &recovery_test_request("acme/model", &["weights.gguf".into()]),
                &destination,
                None,
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), requested)
            .await
            .unwrap()
            .unwrap();
        if stall_body {
            tokio::time::timeout(Duration::from_secs(3), async {
                while !client.list_downloads().await.iter().any(|entry| {
                    entry.download_id == id
                        && entry.downloaded_bytes == Some(5)
                        && (!retry || entry.retrying == Some(true))
                }) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        if matches!(stall, StalledResponse::CancelDuringPause) {
            let (entered_sender, entered) = tokio::sync::oneshot::channel();
            let entered_sender = std::sync::Mutex::new(Some(entered_sender));
            let (continue_sender, continue_receiver) = std::sync::mpsc::channel();
            let continue_receiver = std::sync::Mutex::new(continue_receiver);
            client
                .download_tasks
                .set_blocking_observer(Some(Arc::new(move |operation| {
                    if operation == "persist download pause" {
                        if let Some(sender) = entered_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = continue_receiver.lock().unwrap().recv();
                        }
                    }
                })));
            assert!(client.pause_download(&id).await.unwrap());
            let reached = tokio::time::timeout(Duration::from_secs(3), entered).await;
            if reached.is_err() {
                let _ = continue_sender.send(());
            }
            reached.unwrap().unwrap();
            assert!(client.cancel_download(&id).await.unwrap());
            assert_eq!(
                client.get_download_status(&id).await,
                Some(DownloadStatus::Cancelling)
            );
            continue_sender.send(()).unwrap();
            release_sender.send(()).unwrap();
            assert!(server.await.unwrap());
            tokio::time::timeout(Duration::from_secs(3), async {
                while client.get_download_status(&id).await != Some(DownloadStatus::Cancelled)
                    || client
                        .download_tasks
                        .snapshot(&id)
                        .is_some_and(|task| !task.finished)
                {
                    assert_ne!(
                        client.get_download_status(&id).await,
                        Some(DownloadStatus::Paused)
                    );
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            client.download_tasks.set_blocking_observer(None);
            let mut restarted = HuggingFaceClient::new(temp.path().join("restarted")).unwrap();
            restarted
                .configure_download_destination_root(temp.path())
                .unwrap();
            restarted.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
            restarted.restore_persisted_downloads().await.unwrap();
            assert!(restarted.list_downloads().await.is_empty());
            return;
        }
        assert!(client.pause_download(&id).await.unwrap());
        let paused = tokio::time::timeout(Duration::from_millis(500), async {
            while client.get_download_status(&id).await != Some(DownloadStatus::Paused) {
                tokio::task::yield_now().await;
            }
        })
        .await;
        if paused.is_ok() && matches!(stall, StalledResponse::ImmediateResume) {
            // Do not drain the paused generation first: public Paused is the
            // promise that callers may immediately request a successor.
            assert_resumed_partial_completes(&mut client, &id, &destination).await;
            release_sender.send(()).unwrap();
            assert!(server.await.unwrap());
            return;
        }
        release_sender.send(()).unwrap();
        let no_extra_request = server.await.unwrap();
        if paused.is_err() {
            client.cancel_download(&id).await.unwrap();
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while client
                .download_tasks
                .snapshot(&id)
                .is_some_and(|task| !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            paused.is_ok(),
            "pause must settle without waiting for {stall:?}"
        );
        assert!(
            no_extra_request,
            "paused retry must not send another request"
        );
        assert_eq!(
            std::fs::read(destination.join("weights.gguf.part")).unwrap(),
            if stall_body {
                b"abcde".as_slice()
            } else {
                b"abc".as_slice()
            }
        );
        let mut restarted = HuggingFaceClient::new(temp.path().join("restarted")).unwrap();
        restarted
            .configure_download_destination_root(temp.path())
            .unwrap();
        restarted.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        restarted.restore_persisted_downloads().await.unwrap();
        assert_eq!(
            restarted.get_download_status(&id).await,
            Some(DownloadStatus::Paused)
        );
        if stall_body {
            assert_resumed_partial_completes(&mut restarted, &id, &destination).await;
        }
    }

    async fn assert_resumed_partial_completes(
        client: &mut HuggingFaceClient,
        id: &str,
        destination: &Path,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        client.set_test_download_base_url(format!("http://{}", listener.local_addr().unwrap()));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                assert!(headers.len() < 4096);
                headers.push(socket.read_u8().await.unwrap());
            }
            assert!(String::from_utf8(headers)
                .unwrap()
                .to_ascii_lowercase()
                .contains("range: bytes=5-"));
            socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 5-7/8\r\nConnection: close\r\n\r\nfgh").await.unwrap();
        });
        assert!(client.resume_download(id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(id).await != Some(DownloadStatus::Completed)
                || client
                    .download_tasks
                    .snapshot(id)
                    .is_some_and(|task| !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(
            std::fs::read(destination.join("weights.gguf")).unwrap(),
            b"abcdefgh"
        );
    }
    #[tokio::test]
    async fn admitted_error_resumes_with_exact_authority_before_and_after_restart() {
        for restart in [false, true] {
            let temp = TempDir::new().unwrap();
            let mut client = configured_download_client(temp.path().join("cache")).unwrap();
            let destination = temp.path().join("library/model");
            std::fs::create_dir_all(destination.join(".pumas_download")).unwrap();
            cache_repo_tree(
                &client,
                "acme/model",
                vec![LfsFileInfo {
                    filename: "weights.gguf".into(),
                    size: 4,
                    sha256: "a".repeat(64),
                }],
                Vec::new(),
            );
            let id = client
                .start_download(
                    &recovery_test_request("acme/model", &["weights.gguf".to_string()]),
                    &destination,
                    None,
                )
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if client.list_downloads().await.iter().any(|entry| {
                        entry.download_id == id && entry.status == DownloadStatus::Error
                    }) && !client.download_tasks.contains(&id)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            std::fs::remove_dir(destination.join(".pumas_download")).unwrap();
            if restart {
                drop(client);
                client = configured_download_client(temp.path().join("cache")).unwrap();
                client.restore_persisted_downloads().await.unwrap();
            }
            std::fs::write(destination.join("weights.gguf"), b"done").unwrap();
            assert!(client.resume_download(&id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if client.list_downloads().await.iter().any(|entry| {
                        entry.download_id == id && entry.status == DownloadStatus::Completed
                    }) {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("resuming an admitted request must use its exact persistence attempt");
            assert!(!client
                .persistence
                .as_ref()
                .unwrap()
                .load_lifecycle_inventory_strict()
                .unwrap()
                .queue_admissions
                .contains_key(&id));
        }
    }
    #[tokio::test]
    async fn admission_rechecks_hidden_predecessors_after_its_store_transaction() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        let destination = temp.path().join("library/model");
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;
        let (checked_sender, checked) = tokio::sync::oneshot::channel();
        let checked_sender = std::sync::Mutex::new(Some(checked_sender));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = std::sync::Mutex::new(release);
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new(move |operation, _| {
                if operation == "admission-inventory-checked" {
                    if let Some(sender) = checked_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            })));
        let start = tokio::spawn({
            let client = client.clone();
            let request = request.clone();
            let destination = destination.clone();
            async move { client.start_download(&request, &destination, None).await }
        });
        tokio::time::timeout(Duration::from_secs(2), checked)
            .await
            .unwrap()
            .unwrap();
        let independent_store = DownloadPersistence::new(temp.path());
        let predecessor = DownloadAdmissionRequest {
            snapshot: PersistedDownload {
                download_id: uuid::Uuid::new_v4().to_string(),
                repo_id: "acme/model".into(),
                filename: "weights.gguf".into(),
                filenames: vec!["weights.gguf".into()],
                dest_dir: destination.clone(),
                total_bytes: Some(4),
                status: DownloadStatus::Queued,
                download_request: request,
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: Some("a".repeat(64)),
                huggingface_evidence: None,
            },
            domain: DownloadAdmissionDomain::Ambient,
            destination: client
                .destination_root
                .as_ref()
                .unwrap()
                .resolve(&destination)
                .unwrap()
                .persisted_identity()
                .unwrap(),
            requested_payload_files: vec!["weights.gguf".into()],
            execution_files: vec!["weights.gguf".into()],
        };
        assert!(matches!(
            independent_store
                .admit_download(&uuid::Uuid::new_v4().to_string(), &predecessor)
                .unwrap(),
            DownloadAdmissionTransition::Durable { .. }
        ));
        release_sender.send(()).unwrap();
        let refused = start.await.unwrap().is_err();
        assert!(
            refused,
            "a newly observed unresolved predecessor must block Worker promotion"
        );
        assert!(client.downloads.read().await.is_empty());
        assert!(!destination.exists());
        drop(guard);
    }
    #[test]
    fn admitted_terminal_settlement_waits_for_effect_drain() {
        for cancel in [false, true] {
            let temp = TempDir::new().unwrap();
            let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
            cache_repo_tree(
                &client,
                "acme/model",
                vec![LfsFileInfo {
                    filename: "weights.gguf".into(),
                    size: 4,
                    sha256: "a".repeat(64),
                }],
                Vec::new(),
            );
            let destination = temp.path().join("library/model");
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(destination.join("weights.gguf"), b"done").unwrap();
            let (id_sender, id_receiver) = std::sync::mpsc::channel();
            let (entered_sender, entered) = std::sync::mpsc::channel();
            let entered_sender = std::sync::Mutex::new(Some(entered_sender));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = std::sync::Mutex::new(release);
            client
                .download_tasks
                .set_drain_observer(Some(Arc::new(move || {
                    if let Some(sender) = entered_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                })));
            let worker = std::thread::spawn({
                let client = client.clone();
                move || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(async {
                            let physical_guard = client
                                .destination_lock(&destination_identity(&client, &destination))
                                .await
                                .lock_owned()
                                .await;
                            let id = client
                                .start_download(
                                    &recovery_test_request(
                                        "acme/model",
                                        &["weights.gguf".to_string()],
                                    ),
                                    &destination,
                                    None,
                                )
                                .await
                                .unwrap();
                            id_sender.send(id.clone()).unwrap();
                            if cancel {
                                client.cancel_download(&id).await.unwrap();
                            }
                            drop(physical_guard);
                            tokio::time::timeout(Duration::from_secs(2), async {
                                loop {
                                    if client.list_downloads().await.iter().any(|entry| {
                                        entry.download_id == id
                                            && entry.status
                                                == if cancel {
                                                    DownloadStatus::Cancelled
                                                } else {
                                                    DownloadStatus::Completed
                                                }
                                    }) {
                                        break;
                                    }
                                    tokio::task::yield_now().await;
                                }
                            })
                            .await
                            .unwrap();
                        })
                }
            });
            let id = id_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            entered.recv_timeout(Duration::from_secs(2)).unwrap();
            let still_owned = client
                .persistence
                .as_ref()
                .unwrap()
                .load_lifecycle_inventory_strict()
                .unwrap()
                .queue_admissions
                .contains_key(&id);
            release_sender.send(()).unwrap();
            client.download_tasks.set_drain_observer(None);
            worker.join().unwrap();
            assert!(
                still_owned,
                "durable release must wait for effect drain for cancel={cancel}"
            );
        }
    }
    #[tokio::test]
    async fn unconfigured_download_refuses_before_remote_or_destination_effects() {
        let temp = tempfile::TempDir::new().unwrap();
        let client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("missing");
        assert!(matches!(
            client.start_download(&request, &destination, None).await,
            Err(PumasError::Config { .. })
        ));
        assert!(!destination.exists());
        assert!(client.list_downloads().await.is_empty());
    }

    #[tokio::test]
    async fn malformed_restore_inventory_is_an_error_and_preserves_store_bytes() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        std::fs::write(&path, b"not-json").unwrap();
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        assert!(matches!(
            client.restore_persisted_downloads().await,
            Err(PumasError::Json { .. })
        ));
        assert!(client.list_downloads().await.is_empty());
        assert_eq!(std::fs::read(path).unwrap(), b"not-json");
    }
    #[tokio::test]
    async fn restore_rejects_old_tracking_formats_without_migrating_or_touching_files() {
        for version in [None, Some(1), Some(2), Some(3)] {
            let temp = TempDir::new().unwrap();
            let client = configured_download_client(temp.path().join("cache")).unwrap();
            let destination = temp.path().join("model");
            std::fs::create_dir(&destination).unwrap();
            std::fs::write(destination.join("weights.gguf.part"), b"partial").unwrap();
            std::fs::write(destination.join(".pumas_download"), b"marker").unwrap();
            let mut document = serde_json::json!({"downloads": {}});
            if let Some(version) = version {
                document["schema_version"] = version.into();
            }
            let bytes = serde_json::to_vec(&document).unwrap();
            let tracking = temp.path().join("downloads.json");
            std::fs::write(&tracking, &bytes).unwrap();
            assert!(matches!(client.restore_persisted_downloads().await,
                Err(PumasError::Validation { field, .. }) if field == "downloads.schema_version"));
            assert!(client.list_downloads().await.is_empty());
            assert_eq!(std::fs::read(&tracking).unwrap(), bytes);
            assert_eq!(
                std::fs::read(destination.join("weights.gguf.part")).unwrap(),
                b"partial"
            );
            assert_eq!(
                std::fs::read(destination.join(".pumas_download")).unwrap(),
                b"marker"
            );
        }
    }

    fn recovery_test_client(cache_dir: PathBuf, library_root: &Path) -> HuggingFaceClient {
        std::fs::create_dir_all(library_root).unwrap();
        let mut client = HuggingFaceClient::new(cache_dir).unwrap();
        client
            .configure_download_destination_root(library_root)
            .unwrap();
        client
    }

    fn configured_download_client(cache_dir: impl Into<PathBuf>) -> Result<HuggingFaceClient> {
        let cache_dir = cache_dir.into();
        let root = if cache_dir.file_name().is_some_and(|name| name == "cache") {
            cache_dir.parent().unwrap().to_path_buf()
        } else {
            cache_dir.clone()
        };
        std::fs::create_dir_all(&root).unwrap();
        let mut client = HuggingFaceClient::new(cache_dir)?;
        client.configure_download_destination_root(&root)?;
        client.set_persistence(Arc::new(DownloadPersistence::new(&root)));
        Ok(client)
    }
    #[tokio::test]
    async fn admitted_download_survives_restart_and_cancels_with_exact_queue_settlement() {
        let temp = tempfile::TempDir::new().unwrap();
        let library = temp.path().join("library");
        let destination = library.join("model");
        std::fs::create_dir_all(destination.join(".pumas_download")).unwrap();
        std::fs::write(destination.join("weights.gguf.part"), b"x").unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(&library)
            .unwrap();
        client.set_persistence(persistence.clone());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;
        let id = client
            .start_download(&request, &destination, None)
            .await
            .unwrap();
        let inventory = persistence.load_lifecycle_inventory_strict().unwrap();
        assert!(
            inventory.queue_admissions.contains_key(&id),
            "returned download ID must already have confirmed durable admission"
        );
        drop(guard);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client
                    .list_downloads()
                    .await
                    .iter()
                    .any(|entry| entry.download_id == id && entry.status == DownloadStatus::Error)
                    && !client.download_tasks.contains(&id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(client);
        std::fs::remove_dir(destination.join(".pumas_download")).unwrap();
        let reopened = Arc::new(DownloadPersistence::new(temp.path()));
        let mut restarted = configured_download_client(temp.path().join("cache")).unwrap();
        restarted
            .configure_download_destination_root(&library)
            .unwrap();
        restarted.set_persistence(reopened.clone());
        restarted.restore_persisted_downloads().await.unwrap();
        assert!(restarted
            .list_downloads()
            .await
            .iter()
            .any(|entry| entry.download_id == id));
        assert!(restarted.cancel_download(&id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if restarted.list_downloads().await.iter().any(|entry| {
                    entry.download_id == id && entry.status == DownloadStatus::Cancelled
                }) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let inventory = reopened.load_lifecycle_inventory_strict().unwrap();
        assert!(!inventory.queue_admissions.contains_key(&id));
        assert!(!destination.join("weights.gguf.part").exists());
    }
    use super::*;
    use crate::model_library::{
        issue_download_recovery_ticket, verify_download_recovery_ticket,
        DownloadRecoveryVerification, LfsFileInfo, RepoFileTree,
    };
    use crate::ModelRecord;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    fn verified_recovery(
        library_root: &Path,
        repo_id: &str,
        files: &[&str],
    ) -> VerifiedDownloadRecovery {
        let model_dir = library_root.join("llm/acme/model");
        std::fs::create_dir_all(&model_dir).unwrap();
        crate::model_library::download_recovery::DownloadDestinationRoot::open(library_root)
            .unwrap();
        let record = ModelRecord {
            id: "llm/acme/model".to_string(),
            path: model_dir.display().to_string(),
            cleaned_name: "model".to_string(),
            official_name: "Model".to_string(),
            model_type: "llm".to_string(),
            tags: Vec::new(),
            hashes: HashMap::new(),
            metadata: json!({
                "download_incomplete": true,
                "repo_id": repo_id,
                "selected_artifact_files": files,
            }),
            updated_at: "2026-09-03T00:00:00Z".to_string(),
        };
        let ticket = issue_download_recovery_ticket(library_root, &record)
            .unwrap()
            .unwrap();
        let token = crate::model_library::DownloadRecoveryToken::parse(ticket.token()).unwrap();
        match verify_download_recovery_ticket(library_root, &record, &token).unwrap() {
            DownloadRecoveryVerification::Verified(verified) => verified,
            _ => panic!("recovery fixture must verify"),
        }
    }

    fn cache_repo_tree(
        client: &HuggingFaceClient,
        repo_id: &str,
        lfs_files: Vec<LfsFileInfo>,
        regular_files: Vec<String>,
    ) {
        let tree = RepoFileTree {
            repo_id: repo_id.to_string(),
            lfs_files,
            regular_files,
            cached_at: chrono::Utc::now().to_rfc3339(),
            last_modified: None,
            cache_version: 2,
        };
        std::fs::write(
            client.get_cache_path(repo_id, "files"),
            serde_json::to_vec(&tree).unwrap(),
        )
        .unwrap();
    }

    fn recovery_test_request(repo_id: &str, files: &[String]) -> DownloadRequest {
        let (family, official_name) = repo_id.split_once('/').unwrap();
        DownloadRequest {
            repo_id: repo_id.to_string(),
            family: family.to_string(),
            official_name: official_name.to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: Some(files.to_vec()),
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        }
    }

    fn recovery_test_state(
        verified: &VerifiedDownloadRecovery,
        download_id: &str,
        status: DownloadStatus,
        task_registered: bool,
    ) -> DownloadState {
        let files = verified
            .files
            .iter()
            .map(|filename| FileToDownload {
                filename: filename.clone(),
                size: Some(4),
                sha256: None,
            })
            .collect::<Vec<_>>();
        DownloadState {
            download_id: download_id.to_string(),
            repo_id: verified.repo_id.clone(),
            status,
            progress: 0.5,
            downloaded_bytes: 2,
            total_bytes: Some(4),
            speed: 0.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            error: None,
            retry_attempt: 0,
            retry_limit: None,
            retrying: false,
            next_retry_delay_seconds: None,
            task_registered,
            lifecycle_failure_unverified: false,
            dest_dir: verified.destination.display_path().to_path_buf(),
            ambient_authority_blocked: false,
            admission: None,
            revoked_snapshot: None,
            destination: Some(DownloadDestination::Recovery(verified.destination.clone())),
            filename: verified.files[0].clone(),
            files,
            files_completed: 0,
            download_request: Some(recovery_test_request(&verified.repo_id, &verified.files)),
            known_sha256: None,
            huggingface_evidence: None,
        }
    }

    #[test]
    fn progress_library_identity_comes_only_from_the_bound_destination() {
        let temp = TempDir::new().unwrap();
        let verified =
            verified_recovery(temp.path(), "different-publisher/model", &["weights.gguf"]);
        let mut state = recovery_test_state(
            &verified,
            "identity-download",
            DownloadStatus::Paused,
            false,
        );
        assert_eq!(
            progress_from_state(&state).library_model_id.as_deref(),
            Some("llm/acme/model")
        );
        state.make_managed_for_test();
        assert_eq!(
            progress_from_state(&state).library_model_id.as_deref(),
            Some("llm/acme/model")
        );
        let snapshot = persisted_recovery_test_state(&state);
        let restored =
            DownloadState::from_persisted(&snapshot, 2, state.destination.clone().unwrap());
        assert_eq!(
            progress_from_state(&restored).library_model_id.as_deref(),
            Some("llm/acme/model")
        );
        state.destination = None;
        assert!(
            progress_from_state(&state).library_model_id.is_none(),
            "ambient path and repository labels cannot invent association"
        );
    }

    fn persisted_recovery_test_state(state: &DownloadState) -> PersistedDownload {
        PersistedDownload {
            download_id: state.download_id.clone(),
            repo_id: state.repo_id.clone(),
            filename: state.filename.clone(),
            filenames: state
                .files
                .iter()
                .map(|file| file.filename.clone())
                .collect(),
            dest_dir: state.dest_dir.clone(),
            total_bytes: state.total_bytes,
            status: state.status,
            download_request: state.download_request.clone().unwrap(),
            created_at: "2026-09-03T00:00:00Z".to_string(),
            known_sha256: None,
            huggingface_evidence: None,
        }
    }

    fn install_promotable_recovery_transition(
        client: &Arc<HuggingFaceClient>,
        download_id: &str,
    ) -> (
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let (promote_sender, promote_receiver) = tokio::sync::oneshot::channel();
        let (promoted_sender, promoted_receiver) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::RecoveryTransition,
                move |context| async move {
                    let _ = promote_receiver.await;
                    assert!(context.promote_role(TaskRole::Worker));
                    let _ = promoted_sender.send(());
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        (promote_sender, promoted_receiver)
    }

    #[tokio::test]
    async fn ordinary_start_invalid_destination_is_refused_before_admission() {
        let temp = TempDir::new().unwrap();
        let client = configured_download_client(temp.path().join("cache")).unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("not-a-directory");
        std::fs::write(&destination, b"occupied").unwrap();

        assert!(client
            .start_download(&request, &destination, None)
            .await
            .is_err());
        assert!(client.list_downloads().await.is_empty());
        assert!(client.download_tasks.ids().is_empty());
        assert_eq!(std::fs::read(destination).unwrap(), b"occupied");
    }

    #[tokio::test]
    async fn concurrent_same_destination_starts_commit_one_owner_and_one_id() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = Arc::new(recovery_test_request(
            "acme/model",
            &["weights.gguf".to_string()],
        ));
        let destination = temp.path().join("library").join("model");
        let destination_guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;
        let admissions = Arc::new(std::sync::Barrier::new(2));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let admissions = admissions.clone();
                move |operation, _| {
                    if operation == "prepare-download-task" {
                        admissions.wait();
                    }
                }
            })));

        let start = |client: Arc<HuggingFaceClient>| {
            let request = request.clone();
            let destination = destination.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(client.start_download(&request, &destination, None))
                    .unwrap()
            })
        };
        let first = start(client.clone());
        let second = start(client.clone());
        let first_id = first.join().expect("first start should settle");
        let second_id = second.join().expect("second start should settle");

        assert_eq!(first_id, second_id);
        let downloads = client.downloads.read().await;
        assert_eq!(downloads.len(), 1);
        let state = downloads.get(&first_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.task_registered);
        drop(downloads);
        assert!(client.download_tasks.contains(&first_id));

        client.download_tasks.set_ambient_admission_observer(None);
        drop(destination_guard);
        assert!(client.cancel_download(&first_id).await.unwrap());
    }

    #[tokio::test]
    async fn cancelling_a_waiting_destination_owner_cannot_clean_the_incumbent() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/first",
            vec![LfsFileInfo {
                filename: "first.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        cache_repo_tree(
            &client,
            "acme/second",
            vec![LfsFileInfo {
                filename: "second.gguf".to_string(),
                size: 4,
                sha256: "b".repeat(64),
            }],
            Vec::new(),
        );
        let first_request = recovery_test_request("acme/first", &["first.gguf".to_string()]);
        let second_request = recovery_test_request("acme/second", &["second.gguf".to_string()]);
        let destination = temp.path().join("library").join("shared-model");
        std::fs::create_dir_all(&destination).unwrap();
        let physical_guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;

        let first_id = client
            .start_download(&first_request, &destination, None)
            .await
            .unwrap();
        let second_id = client
            .start_download(&second_request, &destination, None)
            .await
            .unwrap();
        assert_ne!(first_id, second_id);
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            2
        );
        std::fs::write(destination.join("first.gguf.part"), b"first").unwrap();
        std::fs::write(destination.join("second.gguf.part"), b"second").unwrap();
        std::fs::write(destination.join(".pumas_download"), b"incumbent").unwrap();

        assert!(client.cancel_download(&second_id).await.unwrap());
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(
            client.downloads.read().await[&second_id].status,
            DownloadStatus::Cancelling
        );
        assert_eq!(
            std::fs::read(destination.join("first.gguf.part")).unwrap(),
            b"first"
        );
        assert_eq!(
            std::fs::read(destination.join("second.gguf.part")).unwrap(),
            b"second"
        );
        assert_eq!(
            std::fs::read(destination.join(".pumas_download")).unwrap(),
            b"incumbent"
        );

        assert!(client.cancel_download(&first_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let states = client.downloads.read().await;
                let terminal = [first_id.as_str(), second_id.as_str()].iter().all(|id| {
                    states
                        .get(*id)
                        .is_some_and(|state| state.status == DownloadStatus::Cancelled)
                });
                drop(states);
                if terminal
                    && !client.download_tasks.contains(&first_id)
                    && !client.download_tasks.contains(&second_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("incumbent cleanup must release the queued finalizer in FIFO order");
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            0
        );
        assert!(!destination.join("first.gguf.part").exists());
        assert!(!destination.join("second.gguf.part").exists());
        assert!(!destination.join(".pumas_download").exists());
        drop(physical_guard);
    }

    #[tokio::test]
    async fn partial_overlap_gets_a_truthful_full_request_behind_the_incumbent() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![
                LfsFileInfo {
                    filename: "a.gguf".to_string(),
                    size: 4,
                    sha256: "a".repeat(64),
                },
                LfsFileInfo {
                    filename: "b.gguf".to_string(),
                    size: 8,
                    sha256: "b".repeat(64),
                },
            ],
            Vec::new(),
        );
        let first_request = recovery_test_request("acme/model", &["a.gguf".to_string()]);
        let full_request =
            recovery_test_request("acme/model", &["a.gguf".to_string(), "b.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let physical_guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;

        let first_id = client
            .start_download(&first_request, &destination, None)
            .await
            .unwrap();
        let full_id = client
            .start_download(&full_request, &destination, None)
            .await
            .unwrap();
        assert_ne!(first_id, full_id);
        let states = client.downloads.read().await;
        let full = &states[&full_id];
        assert_eq!(
            full.files
                .iter()
                .map(|file| file.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["a.gguf", "b.gguf"]
        );
        assert_eq!(full.known_sha256.clone(), Some("b".repeat(64)));
        assert_eq!(
            full.download_request
                .as_ref()
                .and_then(|request| request.filenames.as_ref())
                .cloned(),
            Some(vec!["a.gguf".to_string(), "b.gguf".to_string()])
        );
        drop(states);
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            2
        );

        assert!(client.cancel_download(&full_id).await.unwrap());
        assert!(client.cancel_download(&first_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let states = client.downloads.read().await;
                let terminal = [first_id.as_str(), full_id.as_str()].iter().all(|id| {
                    states
                        .get(*id)
                        .is_some_and(|state| state.status == DownloadStatus::Cancelled)
                });
                drop(states);
                if terminal {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            0
        );
        drop(physical_guard);
    }

    #[tokio::test]
    async fn ordinary_start_never_reuses_or_crosses_live_recovery_authority() {
        for requested in [
            vec!["a.gguf".to_string()],
            vec!["a.gguf".to_string(), "b.gguf".to_string()],
            vec!["b.gguf".to_string()],
        ] {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            std::fs::create_dir_all(&library_root).unwrap();
            let mut client = configured_download_client(temp.path().join("cache")).unwrap();
            client
                .configure_download_destination_root(&library_root)
                .unwrap();
            let client = Arc::new(client);
            cache_repo_tree(
                &client,
                "acme/model",
                vec![
                    LfsFileInfo {
                        filename: "a.gguf".to_string(),
                        size: 4,
                        sha256: "a".repeat(64),
                    },
                    LfsFileInfo {
                        filename: "b.gguf".to_string(),
                        size: 8,
                        sha256: "b".repeat(64),
                    },
                ],
                Vec::new(),
            );
            let verified = verified_recovery(&library_root, "acme/model", &["a.gguf"]);
            let recovery_id = "held-recovery";
            client.downloads.write().await.insert(
                recovery_id.to_string(),
                recovery_test_state(&verified, recovery_id, DownloadStatus::Paused, false),
            );
            let marker = verified.destination.display_path().join(".pumas_download");
            std::fs::write(&marker, b"recovery-marker").unwrap();
            let marker_before = std::fs::read(&marker).unwrap();
            let request = recovery_test_request("acme/model", &requested);

            let ordinary_id = client
                .start_download(&request, verified.destination.display_path(), None)
                .await
                .unwrap();
            assert_ne!(ordinary_id, recovery_id);
            let states = client.downloads.read().await;
            assert!(states[recovery_id].recovery_destination().is_some());
            assert_eq!(states[recovery_id].status, DownloadStatus::Paused);
            assert_eq!(states[&ordinary_id].status, DownloadStatus::Queued);
            assert!(states[&ordinary_id].recovery_destination().is_none());
            drop(states);
            assert_eq!(
                client
                    .destination_executions
                    .claim_count(&verified.destination.identity()),
                2
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
            assert_eq!(std::fs::read(&marker).unwrap(), marker_before);
            assert!(!verified
                .destination
                .display_path()
                .join("b.gguf.part")
                .exists());

            assert!(client.cancel_download(&ordinary_id).await.unwrap());
            tokio::time::sleep(Duration::from_millis(25)).await;
            assert_eq!(std::fs::read(&marker).unwrap(), marker_before);
            assert!(client.cancel_download(recovery_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    client.observe_finished_download_tasks().await;
                    let states = client.downloads.read().await;
                    let terminal = [recovery_id, ordinary_id.as_str()].iter().all(|id| {
                        states
                            .get(*id)
                            .is_some_and(|state| state.status == DownloadStatus::Cancelled)
                    });
                    drop(states);
                    if terminal {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("recovery cleanup must release the queued Ambient finalizer");
            assert_eq!(
                client
                    .destination_executions
                    .claim_count(&verified.destination.identity()),
                0
            );
        }
    }

    #[tokio::test]
    async fn incumbent_disappearance_cannot_return_a_stale_id_or_drop_overlap_files() {
        for second_files in [
            vec!["a.gguf".to_string()],
            vec!["a.gguf".to_string(), "b.gguf".to_string()],
        ] {
            let temp = TempDir::new().unwrap();
            let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
            cache_repo_tree(
                &client,
                "acme/model",
                vec![
                    LfsFileInfo {
                        filename: "a.gguf".to_string(),
                        size: 4,
                        sha256: "a".repeat(64),
                    },
                    LfsFileInfo {
                        filename: "b.gguf".to_string(),
                        size: 8,
                        sha256: "b".repeat(64),
                    },
                ],
                Vec::new(),
            );
            let destination = temp.path().join("library").join("model");
            let physical_guard = client
                .destination_lock(&destination_identity(&client, &destination))
                .await
                .lock_owned()
                .await;
            let first_request = recovery_test_request("acme/model", &["a.gguf".to_string()]);
            let first_id = client
                .start_download(&first_request, &destination, None)
                .await
                .unwrap();

            let (scanned_sender, scanned) = std::sync::mpsc::channel();
            let scanned_sender = Arc::new(std::sync::Mutex::new(Some(scanned_sender)));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = Arc::new(std::sync::Mutex::new(release));
            client
                .download_tasks
                .set_ambient_admission_observer(Some(Arc::new({
                    let scanned_sender = scanned_sender.clone();
                    let release = release.clone();
                    move |operation, _| {
                        if operation == "prepare-download-task" {
                            if let Some(sender) = scanned_sender.lock().unwrap().take() {
                                sender.send(()).unwrap();
                                release.lock().unwrap().recv().unwrap();
                            }
                        }
                    }
                })));
            let expected_second_files = second_files.clone();
            let (second_id_sender, second_id_receiver) = std::sync::mpsc::channel();
            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            let second = std::thread::spawn({
                let client = client.clone();
                let destination = destination.clone();
                move || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(async move {
                            let second_id = client
                                .start_download(
                                    &recovery_test_request("acme/model", &second_files),
                                    &destination,
                                    None,
                                )
                                .await
                                .unwrap();
                            second_id_sender.send(second_id).unwrap();
                            let _ = shutdown_receiver.await;
                        });
                }
            });
            scanned
                .recv_timeout(Duration::from_secs(1))
                .expect("second admission must pause before its exact commit");
            assert!(client.cancel_download(&first_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                while client.downloads.read().await[&first_id].status != DownloadStatus::Cancelled {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            release_sender.send(()).unwrap();
            let second_id = second_id_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("second admission must return its new ID");
            assert_ne!(second_id, first_id);
            let states = client.downloads.read().await;
            assert_eq!(
                states[&second_id]
                    .files
                    .iter()
                    .map(|file| file.filename.clone())
                    .collect::<Vec<_>>(),
                expected_second_files
            );
            drop(states);
            client.download_tasks.set_ambient_admission_observer(None);
            assert!(client.cancel_download(&second_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                while client.downloads.read().await[&second_id].status != DownloadStatus::Cancelled
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                client
                    .destination_executions
                    .claim_count(&destination_identity(&client, &destination)),
                0
            );
            let _ = shutdown_sender.send(());
            second.join().unwrap();
            drop(physical_guard);
        }
    }

    #[tokio::test]
    async fn ordinary_start_install_collision_never_commits_state_or_starts_rejected_work() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let collided_id = Arc::new(std::sync::Mutex::new(None::<String>));
        let owner = client.download_tasks.clone();
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let collided_id = collided_id.clone();
                move |operation, download_id| {
                    if operation == "prepare-download-task" {
                        *collided_id.lock().unwrap() = Some(download_id.to_string());
                        let collision = owner
                            .prepare(
                                download_id.to_string(),
                                TaskRole::RecoveryTransition,
                                |_| async {},
                            )
                            .unwrap();
                        owner.install_gated(collision).unwrap().start();
                    }
                }
            })));
        let rejected_work_started = Arc::new(AtomicBool::new(false));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let rejected_work_started = rejected_work_started.clone();
            move |operation| {
                if operation == "prepare ambient destination" {
                    rejected_work_started.store(true, Ordering::SeqCst);
                }
            }
        })));

        assert!(client
            .start_download(&request, &destination, None)
            .await
            .is_err());
        let collided_id = collided_id.lock().unwrap().clone().unwrap();
        assert!(!client.downloads.read().await.contains_key(&collided_id));
        assert!(!rejected_work_started.load(Ordering::SeqCst));
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.download_tasks.contains(&collided_id) {
                client.observe_finished_download_tasks().await;
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("collision owner and rejected prepared task must both settle");
        client.download_tasks.set_ambient_admission_observer(None);
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn cancelling_start_caller_after_commit_cannot_detach_the_started_worker() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let destination_guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;
        let publication_guard = client.download_publications.capture.lock().await;
        let (committed_sender, committed_receiver) = tokio::sync::oneshot::channel();
        let committed_sender = Arc::new(std::sync::Mutex::new(Some(committed_sender)));
        let committed_id = Arc::new(std::sync::Mutex::new(None::<String>));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let committed_sender = committed_sender.clone();
                let committed_id = committed_id.clone();
                move |operation, download_id| {
                    if operation == "ordinary-start-started" {
                        *committed_id.lock().unwrap() = Some(download_id.to_string());
                        if let Some(sender) = committed_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                        }
                    }
                }
            })));

        let start = {
            let client = client.clone();
            let destination = destination.clone();
            tokio::spawn(async move { client.start_download(&request, &destination, None).await })
        };
        tokio::time::timeout(Duration::from_secs(1), committed_receiver)
            .await
            .expect("start should synchronously start its committed owner")
            .unwrap();
        start.abort();
        let _ = start.await;
        drop(publication_guard);

        let download_id = committed_id.lock().unwrap().clone().unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(&download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.task_registered);
        drop(downloads);
        assert!(client
            .download_tasks
            .snapshot(&download_id)
            .is_some_and(|task| {
                task.role == TaskRole::Worker && task.started && !task.outer_finished
            }));

        client.download_tasks.set_ambient_admission_observer(None);
        drop(destination_guard);
        assert!(client.cancel_download(&download_id).await.unwrap());
    }

    #[tokio::test]
    async fn cancelling_start_caller_before_commit_leaves_no_state_or_owner() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let (preparing_sender, preparing_receiver) = tokio::sync::oneshot::channel();
        let preparing_sender = Arc::new(std::sync::Mutex::new(Some(preparing_sender)));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let preparing_sender = preparing_sender.clone();
                move |operation, _| {
                    if operation == "prepare-download-task" {
                        if let Some(sender) = preparing_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                        }
                    }
                }
            })));
        let auth_guard = client.auth_token.write().await;
        let start = {
            let client = client.clone();
            tokio::spawn(async move { client.start_download(&request, &destination, None).await })
        };
        tokio::time::timeout(Duration::from_secs(1), preparing_receiver)
            .await
            .expect("start should reach cancellable pre-admission auth preparation")
            .unwrap();
        assert!(client.downloads.read().await.is_empty());
        assert!(client.download_tasks.ids().is_empty());
        start.abort();
        let _ = start.await;
        drop(auth_guard);
        assert!(client.downloads.read().await.is_empty());
        assert!(client.download_tasks.ids().is_empty());
        client.download_tasks.set_ambient_admission_observer(None);
    }

    #[tokio::test]
    async fn failing_initial_persistence_is_owned_and_never_publishes_a_marker_or_live_row() {
        let temp = TempDir::new().unwrap();
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let (save_started_sender, save_started) = tokio::sync::oneshot::channel();
        let save_started_sender = Arc::new(std::sync::Mutex::new(Some(save_started_sender)));
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let release_receiver = Arc::new(std::sync::Mutex::new(Some(release_receiver)));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let save_started_sender = save_started_sender.clone();
            let release_receiver = release_receiver.clone();
            move |operation| {
                if operation == "durably admit download" {
                    if let Some(sender) = save_started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        release_receiver
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap()
                            .recv()
                            .unwrap();
                    }
                }
            }
        })));

        let client = Arc::new(client);
        let start = tokio::spawn({
            let client = client.clone();
            let destination = destination.clone();
            async move { client.start_download(&request, &destination, None).await }
        });
        tokio::time::timeout(Duration::from_secs(1), save_started)
            .await
            .expect("owned admission should reach persistence before returning")
            .unwrap();
        assert!(!start.is_finished());
        assert!(client.downloads.read().await.is_empty());
        assert_eq!(client.download_tasks.ids().len(), 1);
        assert!(!destination.join(".pumas_download").exists());
        let store_path = temp.path().join("downloads.json");
        std::fs::write(&store_path, b"not-json").unwrap();
        release_sender.send(()).unwrap();
        assert!(matches!(start.await.unwrap(), Err(PumasError::Json { .. })));

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                if client.download_tasks.ids().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("failed admission must settle without a public active state");
        assert!(client.downloads.read().await.is_empty());
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            0
        );
        assert!(!destination.join(".pumas_download").exists());
        std::fs::remove_file(store_path).unwrap();
        assert!(persistence.load_all().is_empty());
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn marker_creation_failure_persists_error_before_terminal_publication() {
        let temp = TempDir::new().unwrap();
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        std::fs::create_dir_all(destination.join(".pumas_download")).unwrap();
        let callback_count = Arc::new(AtomicUsize::new(0));
        client.set_completion_callback(Arc::new({
            let callback_count = callback_count.clone();
            move |_| {
                callback_count.fetch_add(1, Ordering::SeqCst);
            }
        }));
        let client = Arc::new(client);

        let (persist_started_sender, persist_started) = tokio::sync::oneshot::channel();
        let persist_started_sender = Arc::new(std::sync::Mutex::new(Some(persist_started_sender)));
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let release_receiver = Arc::new(std::sync::Mutex::new(Some(release_receiver)));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let persist_started_sender = persist_started_sender.clone();
            let release_receiver = release_receiver.clone();
            move |operation| {
                if operation == "persist download status" {
                    if let Some(sender) = persist_started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        release_receiver
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap()
                            .recv()
                            .unwrap();
                    }
                }
            }
        })));
        let mut updates = client.subscribe_download_updates();

        let download_id = client
            .start_download(&request, &destination, None)
            .await
            .expect("marker creation failure occurs in admitted work");
        tokio::time::timeout(Duration::from_secs(1), persist_started)
            .await
            .expect("terminal Error persistence must be owner-registered")
            .unwrap();
        let state = client.downloads.read().await;
        assert!(matches!(
            state[&download_id].status,
            DownloadStatus::Queued | DownloadStatus::Downloading
        ));
        assert!(state[&download_id].task_registered);
        drop(state);
        while let Ok(notification) = updates.try_recv() {
            assert!(!notification.snapshot.downloads.iter().any(|download| {
                download.download_id == download_id && download.status == DownloadStatus::Error
            }));
        }
        assert_eq!(persistence.load_all()[0].status, DownloadStatus::Queued);

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let state = client.downloads.read().await;
                let settled = state.get(&download_id).is_some_and(|state| {
                    state.status == DownloadStatus::Error && !state.task_registered
                });
                drop(state);
                if settled && !client.download_tasks.contains(&download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("marker failure must settle only after Error persistence");
        assert_eq!(persistence.load_all()[0].status, DownloadStatus::Error);
        assert!(destination.join(".pumas_download").is_dir());
        assert!(!destination.join("weights.gguf.part").exists());
        assert_eq!(callback_count.load(Ordering::SeqCst), 0);
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            1
        );
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn missing_resume_row_fails_closed_without_late_queued_persistence() {
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let library_root = temp.path().join("library");
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ambient-resume-missing-row";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (persist_started_sender, persist_started) = tokio::sync::oneshot::channel();
        let persist_started_sender = Arc::new(std::sync::Mutex::new(Some(persist_started_sender)));
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let release_receiver = Arc::new(std::sync::Mutex::new(Some(release_receiver)));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let persist_started_sender = persist_started_sender.clone();
            let release_receiver = release_receiver.clone();
            move |operation| {
                if operation == "persist admitted download resume" {
                    if let Some(sender) = persist_started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        release_receiver
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap()
                            .recv()
                            .unwrap();
                    }
                }
            }
        })));

        assert!(client.resume_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(1), persist_started)
            .await
            .expect("resumed Worker must own the Queued persistence update")
            .unwrap();
        let attempt_id = client.downloads.read().await[download_id]
            .admission
            .as_ref()
            .unwrap()
            .attempt_id
            .clone();
        assert!(persistence
            .settle_queue_admission(download_id, &attempt_id)
            .unwrap());
        release_sender.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let state = client.downloads.read().await;
                let settled = state.get(download_id).is_some_and(|state| {
                    state.status == DownloadStatus::Error
                        && state.lifecycle_failure_unverified
                        && !state.task_registered
                });
                drop(state);
                if settled && !client.download_tasks.contains(download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("missing resume row must settle sticky Error");
        assert!(persistence.load_all().is_empty());
        assert_eq!(
            client
                .destination_executions
                .claim_count(&verified.destination.identity()),
            1,
            "unverified Error must park its destination reservation"
        );
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn ordinary_worker_panic_is_observed_and_projects_sticky_error() {
        let temp = TempDir::new().unwrap();
        let client = configured_download_client(temp.path().join("cache")).unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        client
            .download_tasks
            .set_worker_projection_observer(Some(Arc::new(|projection| {
                if projection == "worker-entry" {
                    panic!("ordinary Worker panic sentinel");
                }
            })));
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let download_id = client
            .start_download(&request, &destination, None)
            .await
            .expect("the Worker panic occurs after atomic admission");

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let failed = client
                    .downloads
                    .read()
                    .await
                    .get(&download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && state.lifecycle_failure_unverified
                            && !state.task_registered
                    });
                if failed && !client.download_tasks.contains(&download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("panicked ordinary Worker must settle through terminal projection");
        assert!(!destination.exists());
        client.download_tasks.set_worker_projection_observer(None);
    }

    #[tokio::test]
    async fn ordinary_start_aux_callback_panic_is_owned_once_and_fails_closed() {
        let temp = TempDir::new().unwrap();
        let mut client = configured_download_client(temp.path().join("cache")).unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let callback_count = Arc::new(AtomicU64::new(0));
        client.set_aux_complete_callback(Arc::new({
            let callback_count = callback_count.clone();
            move |_| {
                callback_count.fetch_add(1, Ordering::SeqCst);
                panic!("ordinary auxiliary callback panic sentinel");
            }
        }));
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let destination = temp.path().join("library").join("model");
        let download_id = client
            .start_download(&request, &destination, None)
            .await
            .expect("callback belongs to the admitted Worker");

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let failed = client
                    .downloads
                    .read()
                    .await
                    .get(&download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && state.lifecycle_failure_unverified
                            && !state.task_registered
                    });
                if failed && !client.download_tasks.contains(&download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("auxiliary callback panic must settle sticky Error");
        assert_eq!(callback_count.load(Ordering::SeqCst), 1);
        assert!(!destination.join("weights.gguf.part").exists());
    }

    #[test]
    fn exact_recovery_selection_requires_every_bound_file_and_never_adds_auxiliary_files() {
        let tree = RepoFileTree {
            repo_id: "acme/model".to_string(),
            lfs_files: vec![LfsFileInfo {
                filename: "weights-1.gguf".to_string(),
                size: 10,
                sha256: "a".repeat(64),
            }],
            regular_files: vec!["config.json".to_string()],
            cached_at: "2026-09-03T00:00:00Z".to_string(),
            last_modified: None,
            cache_version: 2,
        };

        assert!(resolve_exact_recovery_files(
            &tree,
            &["weights-1.gguf".to_string(), "weights-2.gguf".to_string()]
        )
        .is_none());
        let files = resolve_exact_recovery_files(&tree, &["weights-1.gguf".to_string()])
            .expect("the complete bound set is available");
        assert_eq!(
            files
                .iter()
                .map(|file| file.filename.as_str())
                .collect::<Vec<_>>(),
            ["weights-1.gguf"]
        );
    }

    #[tokio::test]
    async fn recovery_admission_refuses_an_incomplete_remote_set_without_task_or_target_write() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let cache = temp.path().join("cache");
        let mut client = HuggingFaceClient::new(&cache).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let verified = verified_recovery(
            &library_root,
            "acme/model",
            &["weights-1.gguf", "weights-2.gguf"],
        );
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights-1.gguf".to_string(),
                size: 10,
                sha256: "a".repeat(64),
            }],
            vec!["config.json".to_string()],
        );

        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::BoundFilesUnavailable
        ));
        assert!(client.downloads.read().await.is_empty());
        assert!(client.download_tasks.is_empty());
        assert!(!verified
            .destination
            .display_path()
            .join(".pumas_download")
            .exists());
        assert!(!verified
            .destination
            .display_path()
            .join("weights-1.gguf.part")
            .exists());
    }

    #[tokio::test]
    async fn recovery_admission_refuses_remote_size_overflow_without_task_or_target_write() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let cache = temp.path().join("cache");
        let mut client = HuggingFaceClient::new(&cache).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let verified = verified_recovery(
            &library_root,
            "acme/model",
            &["weights-1.gguf", "weights-2.gguf"],
        );
        cache_repo_tree(
            &client,
            "acme/model",
            vec![
                LfsFileInfo {
                    filename: "weights-1.gguf".to_string(),
                    size: u64::MAX,
                    sha256: "a".repeat(64),
                },
                LfsFileInfo {
                    filename: "weights-2.gguf".to_string(),
                    size: 1,
                    sha256: "b".repeat(64),
                },
            ],
            Vec::new(),
        );

        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::BoundFilesUnavailable
        ));
        assert!(client.downloads.read().await.is_empty());
        assert!(client.download_tasks.is_empty());
        assert!(!verified
            .destination
            .display_path()
            .join(".pumas_download")
            .exists());
        assert!(!verified
            .destination
            .display_path()
            .join("weights-1.gguf.part")
            .exists());
    }

    #[tokio::test]
    async fn concurrent_unrelated_recovery_context_cannot_attach_or_filter() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let client = Arc::new(client);
        let first = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let second = verified_recovery(&library_root, "other/model", &["other.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 10,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        cache_repo_tree(
            &client,
            "other/model",
            vec![LfsFileInfo {
                filename: "other.gguf".to_string(),
                size: 10,
                sha256: "b".repeat(64),
            }],
            Vec::new(),
        );

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let run = |verified: VerifiedDownloadRecovery| {
            let client = client.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
                    .unwrap()
            })
        };
        let first_task = run(first);
        let second_task = run(second);
        barrier.wait().await;
        let outcomes = [first_task.await.unwrap(), second_task.await.unwrap()];

        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, RecoveryDownloadAdmission::Recovered { .. }))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, RecoveryDownloadAdmission::ContextMismatch))
                .count(),
            1
        );
        assert_eq!(client.downloads.read().await.len(), 1);
    }

    #[tokio::test]
    async fn recovery_admission_atomically_registers_task_with_held_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;

        let download_id = match client
            .admit_recovery_download(&verified, Some("llm".to_string()))
            .await
            .unwrap()
        {
            RecoveryDownloadAdmission::Recovered { download_id } => download_id,
            _ => panic!("new recovery must be admitted"),
        };
        let downloads = client.downloads.read().await;
        let state = downloads.get(&download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.recovery_destination().is_some());
        assert!(state.task_registered);
        assert!(client.download_tasks.contains(&download_id));
        drop(downloads);

        assert!(client.cancel_download(&download_id).await.unwrap());
        drop(destination_guard);
    }

    #[tokio::test]
    async fn same_recovery_ticket_waits_for_durable_revocation_before_attaching() {
        let (returned_early, first, second) = overlapping_recovery_admission(false, false).await;
        assert!(
            !returned_early,
            "same-context admission returned before durable handoff"
        );
        assert!(
            matches!(first.unwrap(), RecoveryDownloadAdmission::Resumed { download_id } if download_id == "same-ticket-durable-handoff")
        );
        assert!(
            matches!(second.unwrap(), RecoveryDownloadAdmission::Attached { download_id, status: DownloadStatus::Queued } if download_id == "same-ticket-durable-handoff")
        );
    }

    #[tokio::test]
    async fn failed_recovery_transition_never_attaches_same_ticket_waiter() {
        let (returned_early, first, second) = overlapping_recovery_admission(true, false).await;
        assert!(!returned_early);
        assert!(first.is_err());
        assert!(second.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recovery_waiter_observes_completion_after_committed_generation_is_released() {
        let (returned_early, first, second) = overlapping_recovery_admission(false, true).await;
        assert!(!returned_early);
        assert!(matches!(
            first.unwrap(),
            RecoveryDownloadAdmission::Resumed { .. }
        ));
        assert!(
            matches!(second.unwrap(), RecoveryDownloadAdmission::AlreadyCompleted { download_id } if download_id == "same-ticket-durable-handoff")
        );
    }

    async fn overlapping_recovery_admission(
        fail_revocation: bool,
        complete_before_join: bool,
    ) -> (
        bool,
        Result<RecoveryDownloadAdmission>,
        Result<RecoveryDownloadAdmission>,
    ) {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "same-ticket-durable-handoff";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.into(), state);
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("weights.gguf.part"),
            b"done",
        )
        .unwrap();
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let mut destination_guard = Some(destination_lock.lock().await);

        let (started, started_rx) = tokio::sync::oneshot::channel();
        let started = Arc::new(std::sync::Mutex::new(Some(started)));
        let (release, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "revoke persisted recovery authority" {
                    if let Some(started) = started.lock().unwrap().take() {
                        let _ = started.send(());
                        release_rx.lock().unwrap().recv().unwrap();
                        assert!(
                            !fail_revocation,
                            "injected revocation task failure before publication"
                        );
                    }
                }
            })));
        let (waiter_observed, waiter_reached) = tokio::sync::oneshot::channel();
        let (waiter_release, waiter_gate) = std::sync::mpsc::channel();
        if complete_before_join {
            let waiter_observed = std::sync::Mutex::new(Some(waiter_observed));
            let waiter_gate = std::sync::Mutex::new(waiter_gate);
            client
                .download_tasks
                .set_ambient_admission_observer(Some(Arc::new(move |operation, _| {
                    if operation == "recovery-handoff-observed" {
                        if let Some(observed) = waiter_observed.lock().unwrap().take() {
                            let _ = observed.send(());
                            let _ = waiter_gate.lock().unwrap().recv();
                        }
                    }
                })));
        }
        let client = Arc::new(client);
        let first = tokio::spawn({
            let client = client.clone();
            let verified = verified.clone();
            async move {
                client
                    .admit_recovery_download(&verified, Some("llm".into()))
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(2), started_rx)
            .await
            .expect("first recovery must reach durable revocation")
            .unwrap();
        // Establish that the second admission is waiting before revocation
        // commits. Its owned producer is gated separately from caller polling.
        let mut second = Box::pin(client.admit_recovery_download(&verified, Some("llm".into())));
        let early = tokio::time::timeout(Duration::from_millis(100), &mut second).await;
        let returned_before_commit = early.is_ok();
        release.send(()).unwrap();
        let first_result = first.await.unwrap();
        if complete_before_join {
            tokio::time::timeout(Duration::from_secs(2), waiter_reached)
                .await
                .expect("second admission must observe the committed handoff")
                .unwrap();
            drop(destination_guard.take());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let status = client.get_download_status(download_id).await;
                    if status == Some(DownloadStatus::Completed)
                        && !client.download_tasks.contains(download_id)
                    {
                        break;
                    }
                    client.observe_finished_download_tasks().await;
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("committed worker must complete before the held waiter inspects state");
            waiter_release.send(()).unwrap();
        }
        let second_result = match early {
            Ok(result) => result,
            Err(_) => second.await,
        };
        client.download_tasks.set_blocking_observer(None);
        client.download_tasks.set_ambient_admission_observer(None);
        drop(destination_guard);
        client.cancel_download(download_id).await.unwrap();
        (returned_before_commit, first_result, second_result)
    }

    #[tokio::test]
    async fn cancelling_recovery_admission_before_commit_leaves_no_state_or_task() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let auth_guard = client.auth_token.write().await;
        let admission = {
            let client = client.clone();
            let verified = verified.clone();
            tokio::spawn(async move {
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
            })
        };

        let _ = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if !client.downloads.read().await.is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        admission.abort();
        let _ = admission.await;
        drop(auth_guard);

        assert!(client.downloads.read().await.is_empty());
        assert!(client.download_tasks.is_empty());
    }

    #[tokio::test]
    async fn cancelling_recovery_admission_after_commit_keeps_registered_owner() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;
        let admission = {
            let client = client.clone();
            let verified = verified.clone();
            tokio::spawn(async move {
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
            })
        };
        let download_id = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let downloads = client.downloads.read().await;
                if let Some((download_id, state)) = downloads.iter().next() {
                    if state.task_registered {
                        break download_id.clone();
                    }
                }
                drop(downloads);
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("recovery commit should synchronously register its worker");
        admission.abort();
        let _ = admission.await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(&download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.recovery_destination().is_some());
        assert!(state.task_registered);
        drop(downloads);
        assert!(client
            .download_tasks
            .snapshot(&download_id)
            .is_some_and(|task| !task.finished));

        assert!(client.cancel_download(&download_id).await.unwrap());
        drop(destination_guard);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(&download_id).await == Some(DownloadStatus::Cancelled)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancel finalizer should settle the committed recovery");
    }

    #[tokio::test]
    async fn recovery_attach_requires_registered_capability_backed_owner() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "attach-owner";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Queued, false),
        );
        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::ContextMismatch
        ));

        {
            let mut downloads = client.downloads.write().await;
            downloads.get_mut(download_id).unwrap().task_registered = true;
        }
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::Attached { download_id: id, status: DownloadStatus::Queued }
                if id == download_id
        ));

        {
            let mut downloads = client.downloads.write().await;
            downloads
                .get_mut(download_id)
                .unwrap()
                .make_managed_for_test();
        }
        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::ContextMismatch
        ));
        assert!(client.cancel_download(download_id).await.unwrap());
    }

    #[tokio::test]
    async fn tracked_recovery_resume_replaces_ambient_path_with_held_nonpersistent_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "tracked-recovery";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;

        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::Resumed { download_id: id } if id == download_id
        ));
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.recovery_destination().is_some());
        assert!(state.task_registered);
        drop(downloads);
        assert!(persistence.load_all().is_empty());
        assert!(DownloadPersistence::new(temp.path()).load_all().is_empty());

        assert!(client.cancel_download(download_id).await.unwrap());
        drop(destination_guard);
    }

    #[tokio::test]
    async fn recovery_revocation_reserves_actual_download_from_generic_resume() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "reserved-revocation";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let lock_file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(temp.path().join(".downloads.lock"))
            .unwrap();
        lock_file.lock().unwrap();
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;
        let admission = {
            let client = client.clone();
            let verified = verified.clone();
            tokio::spawn(async move {
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
            })
        };

        tokio::time::timeout(Duration::from_secs(2), async {
            while client.download_tasks.snapshot(download_id).is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("recovery revocation should install its transition owner");
        let reservation = client.download_tasks.snapshot(download_id);

        let resume = tokio::time::timeout(
            Duration::from_millis(100),
            client.resume_download(download_id),
        )
        .await;
        lock_file.unlock().unwrap();

        assert!(reservation
            .is_some_and(|task| { task.role == TaskRole::RecoveryTransition && !task.finished }));
        assert!(matches!(resume, Ok(Ok(false))));

        assert!(matches!(
            admission.await.unwrap().unwrap(),
            RecoveryDownloadAdmission::Resumed { download_id: id } if id == download_id
        ));
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| { task.role == TaskRole::Worker && !task.finished }));
        assert!(client.cancel_download(download_id).await.unwrap());
        drop(destination_guard);
    }

    #[tokio::test]
    async fn generic_resume_rechecks_transition_installed_after_its_initial_snapshot() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "resume-stale-transition-check";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        let mut downloads_guard = client.downloads.write().await;
        downloads_guard.insert(download_id.to_string(), state);

        let (checked_sender, checked) = tokio::sync::oneshot::channel();
        let checked_sender = Arc::new(std::sync::Mutex::new(Some(checked_sender)));
        client.download_tasks.set_snapshot_observer(Some(Arc::new({
            let checked_sender = checked_sender.clone();
            move |observed_id, snapshot| {
                if observed_id == download_id && snapshot.is_none() {
                    if let Some(sender) = checked_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                }
            }
        })));
        let resume = {
            let client = client.clone();
            tokio::spawn(async move { client.resume_download(download_id).await })
        };
        checked.await.unwrap();
        client.download_tasks.set_snapshot_observer(None);
        let (promote, promoted) = install_promotable_recovery_transition(&client, download_id);
        drop(downloads_guard);

        assert!(!resume.await.unwrap().unwrap());
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Paused);
        assert!(!state.task_registered);
        drop(downloads);
        promote.send(()).unwrap();
        promoted.await.unwrap();
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| { task.role == TaskRole::Worker && !task.finished }));
        assert!(client.cancel_download(download_id).await.unwrap());
    }

    #[tokio::test]
    async fn generic_resume_refuses_after_transition_durably_revokes_and_disappears() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "resume-after-revoked-transition";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (admission_sender, admission_reached) = std::sync::mpsc::channel();
        let admission_sender = Arc::new(std::sync::Mutex::new(Some(admission_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let admission_sender = admission_sender.clone();
                let release = release.clone();
                move |operation, observed_id| {
                    if operation == "resume" && observed_id == download_id {
                        if let Some(sender) = admission_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let transition_client = client.clone();
        let transition_persistence = persistence.clone();
        let transition_thread = std::thread::spawn(move || {
            admission_reached.recv().unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let prepared = transition_client
                    .download_tasks
                    .prepare(
                        download_id.to_string(),
                        TaskRole::RecoveryTransition,
                        |_| async {},
                    )
                    .unwrap();
                let installed = {
                    let mut downloads = transition_client.downloads.write().await;
                    let installed = transition_client
                        .download_tasks
                        .install_gated(prepared)
                        .unwrap();
                    downloads
                        .get_mut(download_id)
                        .unwrap()
                        .ambient_authority_blocked = true;
                    installed
                };
                installed.start();
                let (attempt_id, snapshot) = {
                    let downloads = transition_client.downloads.read().await;
                    let state = &downloads[download_id];
                    (
                        state.admission.as_ref().unwrap().attempt_id.clone(),
                        persisted_recovery_test_state(state),
                    )
                };
                tokio::task::spawn_blocking(move || {
                    transition_persistence
                        .revoke_admitted_for_recovery(download_id, &attempt_id, &snapshot)?
                        .into_result()
                })
                .await
                .unwrap()
                .unwrap();
                loop {
                    if transition_client
                        .download_tasks
                        .observe_finished(download_id)
                        .await
                        .is_some()
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                assert!(!transition_client.download_tasks.contains(download_id));
                release_sender.send(()).unwrap();
            });
        });

        assert!(!client.resume_download(download_id).await.unwrap());
        transition_thread.join().unwrap();
        client.download_tasks.set_ambient_admission_observer(None);
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Paused);
        assert!(!state.task_registered);
        assert!(state.ambient_authority_blocked);
        drop(downloads);
        assert!(persistence.is_revoked(download_id).unwrap());
    }

    #[tokio::test]
    async fn dropped_recovery_caller_cannot_detach_revocation_to_worker_handoff() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "dropped-revocation-caller";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let lock_file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(temp.path().join(".downloads.lock"))
            .unwrap();
        lock_file.lock().unwrap();
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;
        let admission = {
            let client = client.clone();
            let verified = verified.clone();
            tokio::spawn(async move {
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.role == TaskRole::RecoveryTransition && !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actual download ID must own the blocked revocation");

        admission.abort();
        let _ = admission.await;
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| { task.role == TaskRole::RecoveryTransition && !task.finished }));
        lock_file.unlock().unwrap();

        tokio::time::timeout(Duration::from_secs(2), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.role == TaskRole::Worker && !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("caller-independent transition must promote to its worker");
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.task_registered);
        assert!(state.recovery_destination().is_some());
        drop(downloads);
        assert!(client.cancel_download(download_id).await.unwrap());
        drop(destination_guard);
    }

    #[tokio::test]
    async fn cancellation_during_recovery_revocation_is_owned_until_terminal() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "cancel-revocation";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let lock_file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(temp.path().join(".downloads.lock"))
            .unwrap();
        lock_file.lock().unwrap();
        let admission = {
            let client = client.clone();
            let verified = verified.clone();
            tokio::spawn(async move {
                client
                    .admit_recovery_download(&verified, Some("llm".to_string()))
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.role == TaskRole::RecoveryTransition && !task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("revocation must be owned under the actual download ID");

        assert!(client.cancel_download(download_id).await.unwrap());
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| { task.role == TaskRole::CancelFinalizer && !task.finished }));
        lock_file.unlock().unwrap();
        assert!(admission.await.unwrap().is_err());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let downloads = client.downloads.read().await;
                let state = downloads.get(download_id).unwrap();
                if state.status == DownloadStatus::Cancelled {
                    assert!(!state.task_registered);
                    assert!(state.recovery_destination().is_none());
                    break;
                }
                drop(downloads);
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("caller-independent finalizer must settle after revocation drains");
        assert!(persistence.load_all().is_empty());
    }

    #[tokio::test]
    async fn failed_strict_revocation_leaves_ambient_state_and_target_unchanged() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "revocation-failure";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(temp.path().join("downloads.json"), b"not-json").unwrap();

        assert!(client
            .admit_recovery_download(&verified, Some("llm".to_string()))
            .await
            .is_err());
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.ambient_authority_blocked);
        drop(downloads);
        assert!(client.download_tasks.is_empty());
        assert!(!verified
            .destination
            .display_path()
            .join("weights.gguf.part")
            .exists());

        // Remove the injected corruption so finalizer cleanup is otherwise
        // healthy; the sticky transition failure alone must keep cancellation
        // fail-closed.
        std::fs::remove_file(temp.path().join("downloads.json")).unwrap();
        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let settled = client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && state.lifecycle_failure_unverified
                            && !state.task_registered
                    });
                if settled {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("later cancellation must preserve unresolved transition failure");
    }

    #[tokio::test]
    async fn revoked_ambient_state_cannot_resume_without_fresh_recovery_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "revoked-ambient";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        revoke_state_fixture(
            &persistence,
            client.downloads.write().await.get_mut(download_id).unwrap(),
        );

        assert!(!client.resume_download(download_id).await.unwrap());
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Paused);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
        drop(downloads);
        assert!(client.download_tasks.is_empty());
    }

    #[tokio::test]
    async fn recovery_completion_clears_capability_without_callbacks_or_persistence() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let completion_called = Arc::new(AtomicBool::new(false));
        let completion_observer = completion_called.clone();
        client.set_completion_callback(Arc::new(move |_| {
            completion_observer.store(true, Ordering::SeqCst);
        }));
        let aux_called = Arc::new(AtomicBool::new(false));
        let aux_observer = aux_called.clone();
        client.set_aux_complete_callback(Arc::new(move |_| {
            aux_observer.store(true, Ordering::SeqCst);
        }));

        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            vec!["config.json".to_string()],
        );
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("weights.gguf.part"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();
        let mut updates = client.subscribe_download_updates();

        let download_id = match client
            .admit_recovery_download(&verified, Some("llm".to_string()))
            .await
            .unwrap()
        {
            RecoveryDownloadAdmission::Recovered { download_id } => download_id,
            _ => panic!("a new exact recovery must be admitted"),
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let update = updates.recv().await.unwrap();
                if update.snapshot.downloads.iter().any(|download| {
                    download.download_id == download_id
                        && download.status == DownloadStatus::Completed
                }) {
                    break;
                }
            }
        })
        .await
        .expect("byte-complete recovery should finish without network access");

        let downloads = client.downloads.read().await;
        let state = downloads.get(&download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Completed);
        assert!(!state.task_registered);
        assert!(state.recovery_destination().is_none());
        assert_eq!(
            state
                .files
                .iter()
                .map(|file| file.filename.as_str())
                .collect::<Vec<_>>(),
            ["weights.gguf"]
        );
        drop(downloads);
        assert!(!completion_called.load(Ordering::SeqCst));
        assert!(!aux_called.load(Ordering::SeqCst));
        assert!(persistence.load_all().is_empty());
        assert!(!temp.path().join("downloads.json").exists());
        assert_eq!(
            std::fs::read(verified.destination.display_path().join("weights.gguf")).unwrap(),
            b"done"
        );
        assert!(!verified
            .destination
            .display_path()
            .join(".pumas_download")
            .exists());
    }

    #[tokio::test]
    async fn tracked_recovery_completion_waits_for_persisted_cleanup_before_capability_release() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "tracked-recovery-completion";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("weights.gguf.part"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let (cleanup_sender, cleanup_started) = tokio::sync::oneshot::channel();
        let cleanup_sender = Arc::new(std::sync::Mutex::new(Some(cleanup_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let cleanup_sender = cleanup_sender.clone();
            let release = release.clone();
            move |operation| {
                if operation == "remove completed persisted download" {
                    if let Some(sender) = cleanup_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            }
        })));

        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::Resumed { download_id: resumed }
                if resumed == download_id
        ));
        tokio::time::timeout(Duration::from_secs(2), cleanup_started)
            .await
            .expect("verified recovery must reach owned persistence cleanup")
            .unwrap();
        {
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert_ne!(state.status, DownloadStatus::Completed);
            assert!(state.task_registered);
            assert!(state.recovery_destination().is_some());
        }
        assert!(persistence.load_all().is_empty());
        assert!(persistence.is_revoked(download_id).unwrap());

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(download_id).await != Some(DownloadStatus::Completed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Completed must follow persistence cleanup");
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert!(!state.task_registered);
        assert!(state.recovery_destination().is_none());
        drop(downloads);
        assert!(persistence.load_all().is_empty());
        assert!(persistence.is_revoked(download_id).unwrap());
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn tracked_recovery_persistence_cleanup_failure_retains_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let download_id = "tracked-recovery-cleanup-failure";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("weights.gguf.part"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let downloads_path = temp.path().join("downloads.json");
        let corrupted = Arc::new(AtomicBool::new(false));
        let corrupted_in_observer = corrupted.clone();
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "remove completed persisted download"
                    && !corrupted_in_observer.swap(true, Ordering::SeqCst)
                {
                    std::fs::write(&downloads_path, b"not-json").unwrap();
                }
            })));

        assert!(matches!(
            client
                .admit_recovery_download(&verified, Some("llm".to_string()))
                .await
                .unwrap(),
            RecoveryDownloadAdmission::Resumed { .. }
        ));
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let failed = client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && !state.task_registered
                            && state.recovery_destination().is_some()
                    });
                if failed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("persistence cleanup failure must not publish Completed");
        client.observe_finished_download_tasks().await;
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_some());
        assert!(state.lifecycle_failure_unverified);
        drop(downloads);
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn tracked_recovery_cleanup_retry_preserves_revoked_snapshot_and_failure_history() {
        assert_tracked_recovery_cleanup_snapshot(false).await;
    }

    #[tokio::test]
    async fn cancellation_after_durable_revocation_preserves_snapshot_before_handoff() {
        assert_tracked_recovery_cleanup_snapshot(true).await;
    }

    async fn assert_tracked_recovery_cleanup_snapshot(cancel_before_handoff: bool) {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let destination = verified.destination.display_path();
        std::fs::write(destination.join("weights.gguf.part"), b"abc").unwrap();
        std::fs::write(destination.join(".pumas_download"), b"{}").unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        let original = PersistedDownload {
            download_id: "tracked-revoked-snapshot".into(),
            repo_id: "acme/model".into(),
            filename: "weights.gguf".into(),
            filenames: vec!["weights.gguf".into()],
            dest_dir: destination.to_path_buf(),
            total_bytes: Some(8),
            status: DownloadStatus::Paused,
            download_request: recovery_test_request("acme/model", &["weights.gguf".into()]),
            created_at: "2021-02-03T04:05:06Z".into(),
            known_sha256: Some("b".repeat(64)),
            huggingface_evidence: None,
        };
        admit_snapshot_fixture(&persistence, &original, &verified.destination);
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        client.set_persistence(persistence);
        client.restore_persisted_downloads().await.unwrap();
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 8,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let client = Arc::new(client);
        // The worker cannot reach remote transfer while the public recovery
        // admission and cancellation exchange ownership.
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let transfer_guard = destination_lock.lock().await;
        let fail_once = Arc::new(AtomicBool::new(true));
        client
            .download_tasks
            .set_blocking_failure_observer(Some(Arc::new(move |operation| {
                matches!(
                    operation,
                    "remove partial download file" | "remove ambient partial download file"
                ) && fail_once.swap(false, Ordering::SeqCst)
            })));
        let cancel_thread = if cancel_before_handoff {
            let (ready_sender, ready) = std::sync::mpsc::channel();
            let ready_sender = std::sync::Mutex::new(Some(ready_sender));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = std::sync::Mutex::new(release);
            client
                .download_tasks
                .set_blocking_result_observer(Some(Arc::new(move |operation| {
                    if operation == "revoke persisted recovery authority" {
                        if let Some(sender) = ready_sender.lock().unwrap().take() {
                            sender.send(()).unwrap();
                            release.lock().unwrap().recv().unwrap();
                        }
                    }
                })));
            let cancel_client = client.clone();
            let id = original.download_id.clone();
            Some(std::thread::spawn(move || {
                ready.recv_timeout(Duration::from_secs(3)).unwrap();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async move {
                        assert!(cancel_client.cancel_download(&id).await.unwrap());
                        release_sender.send(()).unwrap();
                        tokio::time::timeout(Duration::from_secs(3), async {
                            while cancel_client.get_download_status(&id).await
                                == Some(DownloadStatus::Cancelling)
                                || cancel_client
                                    .download_tasks
                                    .snapshot(&id)
                                    .is_some_and(|task| !task.finished)
                            {
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                    });
            }))
        } else {
            None
        };
        let admission = client
            .admit_recovery_download(&verified, Some("llm".into()))
            .await;
        if cancel_before_handoff {
            assert!(!matches!(
                admission,
                Ok(RecoveryDownloadAdmission::Resumed { .. })
            ));
        } else {
            assert!(
                matches!(admission.unwrap(), RecoveryDownloadAdmission::Resumed { download_id } if download_id == original.download_id)
            );
            assert!(client.cancel_download(&original.download_id).await.unwrap());
        }
        drop(transfer_guard);
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if client.get_download_status(&original.download_id).await
                    == Some(DownloadStatus::Error)
                    && client
                        .download_tasks
                        .snapshot(&original.download_id)
                        .is_none_or(|task| task.finished)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("failed cleanup must settle without releasing resumable authority");
        if let Some(cancel_thread) = cancel_thread {
            cancel_thread.join().unwrap();
        }
        client.download_tasks.set_blocking_result_observer(None);
        assert_eq!(
            std::fs::read(destination.join("weights.gguf.part")).unwrap(),
            b"abc"
        );
        let reopened_store = Arc::new(DownloadPersistence::new(temp.path()));
        let inventory = reopened_store.load_lifecycle_inventory_strict().unwrap();
        let quarantine = &inventory.quarantines[&original.download_id];
        assert_eq!(quarantine.domain, LifecycleQuarantineDomain::Recovery);
        assert_eq!(quarantine.disposition, LifecycleCleanupDisposition::Pending);
        assert!(quarantine.sticky_failure);
        let mut expected_quarantine = serde_json::to_value(&original).unwrap();
        expected_quarantine["status"] = serde_json::to_value(DownloadStatus::Error).unwrap();
        assert_eq!(serde_json::to_value(&quarantine.snapshot).unwrap(), expected_quarantine,
            "quarantine must preserve the exact pre-revocation fields, not refreshed remote metadata or a new timestamp");
        let mut reopened = HuggingFaceClient::new(temp.path().join("reopened")).unwrap();
        reopened
            .configure_download_destination_root(&library_root)
            .unwrap();
        reopened.set_persistence(reopened_store);
        assert!(reopened.restore_persisted_downloads().await.is_err());
        client.download_tasks.set_blocking_failure_observer(None);
        assert!(client.cancel_download(&original.download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if client.get_download_status(&original.download_id).await
                    == Some(DownloadStatus::Error)
                    && client
                        .download_tasks
                        .snapshot(&original.download_id)
                        .is_none_or(|task| task.finished)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cleanup retry must settle while retaining Error history");
        assert!(!destination.join("weights.gguf.part").exists());
        reopened.restore_persisted_downloads().await.unwrap();
        assert!(reopened.list_downloads().await.is_empty());
        assert_eq!(
            client.get_download_status(&original.download_id).await,
            Some(DownloadStatus::Error)
        );
    }

    #[tokio::test]
    async fn ambient_completion_callback_follows_persisted_cleanup() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ambient-completion-order";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("weights.gguf"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_in_task = callback_called.clone();
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let callback_saw_released_destination = Arc::new(AtomicBool::new(false));
        let callback_saw_released_destination_in_task = callback_saw_released_destination.clone();
        let callback_reentered_terminal_cancel = Arc::new(AtomicBool::new(false));
        let callback_reentered_terminal_cancel_in_task = callback_reentered_terminal_cancel.clone();
        let callback_runtime = tokio::runtime::Handle::current();
        let callback_client = Arc::new(client);
        let callback_client_in_task = callback_client.clone();
        let callback_download_id = download_id.to_string();
        let callback: DownloadCompletionCallback = Arc::new(move |_| {
            callback_saw_released_destination_in_task
                .store(destination_lock.try_lock().is_ok(), Ordering::SeqCst);
            let client = callback_client_in_task.clone();
            let download_id = callback_download_id.clone();
            let (cancelled_sender, cancelled_receiver) = std::sync::mpsc::channel();
            callback_runtime.spawn(async move {
                let cancelled = client.cancel_download(&download_id).await.unwrap();
                let _ = cancelled_sender.send(cancelled);
            });
            let cancelled = cancelled_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("terminal callback must be able to reenter cancellation");
            callback_reentered_terminal_cancel_in_task.store(!cancelled, Ordering::SeqCst);
            callback_called_in_task.store(true, Ordering::SeqCst);
        });
        let client = callback_client;
        let (cleanup_sender, cleanup_started) = tokio::sync::oneshot::channel();
        let cleanup_sender = Arc::new(std::sync::Mutex::new(Some(cleanup_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let cleanup_sender = cleanup_sender.clone();
            let release = release.clone();
            move |operation| {
                if operation == "remove completed persisted download" {
                    if let Some(sender) = cleanup_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            }
        })));

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    Some(callback),
                    None,
                    Some(persistence.clone()),
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), cleanup_started)
            .await
            .expect("ambient worker must reach persistence cleanup")
            .unwrap();
        assert!(!callback_called.load(Ordering::SeqCst));
        assert_ne!(
            client.get_download_status(download_id).await,
            Some(DownloadStatus::Completed)
        );

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(download_id).await != Some(DownloadStatus::Completed)
                || !callback_called.load(Ordering::SeqCst)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("ambient completion must follow persistence cleanup");
        assert!(callback_called.load(Ordering::SeqCst));
        assert!(callback_saw_released_destination.load(Ordering::SeqCst));
        assert!(callback_reentered_terminal_cancel.load(Ordering::SeqCst));
        assert!(persistence.load_all().is_empty());
        client.download_tasks.set_blocking_observer(None);
    }

    async fn imported_download_fixture(
        root: &Path,
    ) -> (
        Arc<crate::model_library::ModelLibrary>,
        HuggingFaceClient,
        PathBuf,
        DownloadRequest,
    ) {
        let library = Arc::new(
            crate::model_library::ModelLibrary::new(root.join("library"))
                .await
                .unwrap(),
        );
        let destination = library.build_model_path("vision", "acme", "model");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("model.onnx"), b"data").unwrap();
        let mut client = HuggingFaceClient::new(root.join("cache")).unwrap();
        client
            .configure_download_destination_root(library.library_root())
            .unwrap();
        client.set_persistence(Arc::new(DownloadPersistence::new(root)));
        client.set_download_importer(Arc::new(crate::model_library::ModelImporter::new(
            library.clone(),
        )));
        let mut request = recovery_test_request("acme/model", &["model.onnx".into()]);
        request.model_type = Some("vision".into());
        request.pipeline_tag = Some("image-classification".into());
        cache_repo_tree(
            &client,
            &request.repo_id,
            vec![LfsFileInfo {
                filename: "model.onnx".into(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        (library, client, destination, request)
    }

    #[tokio::test]
    async fn busy_root_refuses_mutation_but_preserves_idle_runtime_reads() {
        let temp = TempDir::new().unwrap();
        let (library, client, destination, request) = imported_download_fixture(temp.path()).await;
        let root = crate::model_library::download_recovery::DownloadDestinationRoot::open(
            library.library_root(),
        )
        .unwrap();
        // Configuring an idle client must not acquire exclusion.
        let grant = root.try_acquire_execution_grant().unwrap();
        assert!(matches!(
            client.start_download(&request, &destination, None).await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(matches!(
            client.restore_persisted_downloads().await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(matches!(
            client.pause_download("absent").await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(matches!(
            client.resume_download("absent").await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(matches!(
            client.cancel_download("absent").await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(client.list_downloads().await.is_empty());
        assert!(client.get_download_progress("absent").await.is_none());
        assert!(!destination.join(".pumas_download").exists());
        assert!(library.load_metadata(&destination).unwrap().is_none());
        assert!(client
            .persistence
            .as_ref()
            .unwrap()
            .load_lifecycle_inventory_strict()
            .unwrap()
            .queue_admissions
            .is_empty());
        assert_eq!(
            std::fs::read(destination.join("model.onnx")).unwrap(),
            b"data"
        );
        drop(grant);
        assert!(client
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        client.shutdown_downloads().await.unwrap();
        assert!(root.try_acquire_execution_grant().is_ok());
    }

    #[tokio::test]
    async fn real_import_precedes_completion_and_holds_destination_successor() {
        let temp = TempDir::new().unwrap();
        let (library, client, destination, request) = imported_download_fixture(temp.path()).await;
        let (entered, ready) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered));
        let (release, held) = std::sync::mpsc::channel();
        let held = std::sync::Mutex::new(held);
        let import_destination = destination.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            if import_destination.join(".pumas_download").exists() {
                return;
            }
            let entered = entered.lock().unwrap().take();
            if let Some(entered) = entered {
                let _ = entered.send(());
                let _ = held.lock().unwrap().recv();
            }
        })));
        let first = client
            .start_download(&request, &destination, None)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        let status_while_held = client.get_download_status(&first).await;
        let inventory = client
            .persistence
            .as_ref()
            .unwrap()
            .load_lifecycle_inventory_strict()
            .unwrap();
        let index_while_held = library
            .index()
            .get(&library.get_model_id(&destination).unwrap())
            .unwrap();
        let metadata_while_held = library.load_metadata(&destination).unwrap();
        let marker_while_held = destination.join(".pumas_download").exists();
        let competing_root =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(
                library.library_root(),
            )
            .unwrap();
        let contender_blocked = matches!(
            competing_root.try_acquire_execution_grant(),
            Err(PumasError::DownloadRootBusy)
        );
        let independent_destination = library.build_model_path("vision", "acme", "independent");
        std::fs::create_dir_all(&independent_destination).unwrap();
        std::fs::write(independent_destination.join("model.onnx.part"), b"data").unwrap();
        let independent = client
            .start_download(&request, &independent_destination, None)
            .await
            .unwrap();
        // Auxiliary metadata and final import share the existing metadata writer.
        // Root exclusion must still allow this destination's preceding marker write.
        let independent_progressed = tokio::time::timeout(Duration::from_secs(3), async {
            while !independent_destination.join(".pumas_download").exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_ok();
        let independent_state = client
            .downloads
            .read()
            .await
            .get(&independent)
            .map(|state| (state.status, state.error.clone()));
        std::fs::write(destination.join("successor.onnx"), b"next").unwrap();
        let mut successor_request = request.clone();
        successor_request.repo_id = "acme/successor".into();
        successor_request.filename = Some("successor.onnx".into());
        successor_request.filenames = Some(vec!["successor.onnx".into()]);
        cache_repo_tree(
            &client,
            &successor_request.repo_id,
            vec![LfsFileInfo {
                filename: "successor.onnx".into(),
                size: 4,
                sha256: "b".repeat(64),
            }],
            Vec::new(),
        );
        let successor_prepared = Arc::new(AtomicBool::new(false));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let prepared = successor_prepared.clone();
            move |operation| {
                if operation == "prepare ambient destination" {
                    prepared.store(true, Ordering::SeqCst);
                }
            }
        })));
        let successor = client
            .start_download(&successor_request, &destination, None)
            .await
            .unwrap();
        client.pause_download(&successor).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(&successor).await != Some(DownloadStatus::Paused) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let successor_started_early = successor_prepared.load(Ordering::SeqCst);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(&first).await != Some(DownloadStatus::Completed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(status_while_held, Some(DownloadStatus::Downloading));
        assert!(
            contender_blocked,
            "real final import must retain physical exclusion"
        );
        assert!(
            independent_progressed,
            "same-client different destinations must overlap: {independent_state:?}"
        );
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(&independent).await != Some(DownloadStatus::Completed)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("second import must finish after metadata writer release");
        assert!(inventory.queue_admissions.contains_key(&first));
        assert!(index_while_held.is_none());
        assert!(metadata_while_held.is_none());
        assert!(!marker_while_held, "the barrier must hold final import");
        assert!(!successor_started_early);
        let metadata = library.load_metadata(&destination).unwrap().unwrap();
        assert_eq!(metadata.repo_id.as_deref(), Some("acme/model"));
        assert_eq!(metadata.match_source.as_deref(), Some("download"));
        assert!(library
            .index()
            .get(&library.get_model_id(&destination).unwrap())
            .unwrap()
            .is_some());
        let inventory = client
            .persistence
            .as_ref()
            .unwrap()
            .load_lifecycle_inventory_strict()
            .unwrap();
        assert!(!inventory.queue_admissions.contains_key(&first));
        assert!(inventory.queue_admissions.contains_key(&successor));
        client.download_tasks.set_blocking_observer(None);
        library.set_metadata_write_notifier(None);
        client.shutdown_downloads().await.unwrap();
    }

    #[tokio::test]
    async fn held_completion_notification_allows_real_destination_successor() {
        let temp = TempDir::new().unwrap();
        let (library, mut client, destination, request) =
            imported_download_fixture(temp.path()).await;
        let (entered, ready) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered));
        let (release, held) = std::sync::mpsc::channel();
        let held = std::sync::Mutex::new(held);
        client.set_completion_callback(Arc::new(move |_| {
            if let Some(entered) = entered.lock().unwrap().take() {
                let _ = entered.send(());
                let _ = held.lock().unwrap().recv();
            }
        }));
        let first = client
            .start_download(&request, &destination, None)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        let metadata_before_successor = library.load_metadata(&destination).unwrap().unwrap();
        let indexed_before_successor = library
            .index()
            .get(&library.get_model_id(&destination).unwrap())
            .unwrap();
        let inventory_before_successor = client
            .persistence
            .as_ref()
            .unwrap()
            .load_lifecycle_inventory_strict()
            .unwrap();
        let competing_root =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(
                library.library_root(),
            )
            .unwrap();
        let notification_released_root = competing_root.try_acquire_execution_grant().is_ok();
        std::fs::write(destination.join("successor.onnx"), b"next").unwrap();
        let mut successor_request = request.clone();
        successor_request.repo_id = "acme/successor".into();
        successor_request.filename = Some("successor.onnx".into());
        successor_request.filenames = Some(vec!["successor.onnx".into()]);
        cache_repo_tree(
            &client,
            &successor_request.repo_id,
            vec![LfsFileInfo {
                filename: "successor.onnx".into(),
                size: 4,
                sha256: "b".repeat(64),
            }],
            Vec::new(),
        );
        let successor = client
            .start_download(&successor_request, &destination, None)
            .await
            .unwrap();
        let progressed = tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(&successor).await != Some(DownloadStatus::Completed) {
                tokio::task::yield_now().await;
            }
        })
        .await;
        let shutdown = client.shutdown_downloads();
        tokio::pin!(shutdown);
        let shutdown_pending = futures::poll!(&mut shutdown).is_pending();
        release.send(()).unwrap();
        progressed
            .expect("logical release must let the successor complete before notification returns");
        assert!(
            shutdown_pending,
            "shutdown must retain the held notification"
        );
        assert_eq!(
            metadata_before_successor.repo_id.as_deref(),
            Some("acme/model")
        );
        assert!(indexed_before_successor.is_some());
        assert!(
            notification_released_root,
            "notification must not retain physical exclusion"
        );
        assert!(!inventory_before_successor
            .queue_admissions
            .contains_key(&first));
        assert_eq!(
            client.get_download_status(&first).await,
            Some(DownloadStatus::Completed)
        );
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn cancellation_and_shutdown_drain_real_import_before_settlement() {
        for final_import in [false, true] {
            for (shutdown, fail_import) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let temp = TempDir::new().unwrap();
                let (library, client, destination, request) =
                    imported_download_fixture(temp.path()).await;
                if !final_import {
                    // A missing weight reaches auxiliary metadata before HTTP;
                    // cancellation/shutdown at that barrier prevents any request.
                    std::fs::remove_file(destination.join("model.onnx")).unwrap();
                }
                let client = Arc::new(client);
                let (entered, ready) = tokio::sync::oneshot::channel();
                let entered = std::sync::Mutex::new(Some(entered));
                let (release, held) = std::sync::mpsc::channel();
                let held = std::sync::Mutex::new(held);
                let import_destination = destination.clone();
                library.set_metadata_write_notifier(Some(Arc::new(move |_| {
                    if final_import && import_destination.join(".pumas_download").exists() {
                        return;
                    }
                    if let Some(entered) = entered.lock().unwrap().take() {
                        let _ = entered.send(());
                        let _ = held.lock().unwrap().recv();
                        assert!(!fail_import, "injected real importer write panic");
                    }
                })));
                let download_id = client
                    .start_download(&request, &destination, None)
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(3), ready)
                    .await
                    .unwrap()
                    .unwrap();
                let waiter = if shutdown {
                    let client = client.clone();
                    Some(tokio::spawn(
                        async move { client.shutdown_downloads().await },
                    ))
                } else {
                    assert!(client.cancel_download(&download_id).await.unwrap());
                    None
                };
                if shutdown {
                    while !client.download_tasks.is_closed() {
                        tokio::task::yield_now().await;
                    }
                }
                let before = client
                    .persistence
                    .as_ref()
                    .unwrap()
                    .load_lifecycle_inventory_strict()
                    .unwrap();
                let waiting = waiter.as_ref().is_none_or(|waiter| !waiter.is_finished());
                let status = client.get_download_status(&download_id).await;
                let marker_while_held = destination.join(".pumas_download").exists();
                let competing_root =
                    crate::model_library::download_recovery::DownloadDestinationRoot::open(
                        library.library_root(),
                    )
                    .unwrap();
                let contender_blocked = matches!(
                    competing_root.try_acquire_execution_grant(),
                    Err(PumasError::DownloadRootBusy)
                );
                release.send(()).unwrap();
                if let Some(waiter) = waiter {
                    let outcome = tokio::time::timeout(Duration::from_secs(3), waiter)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(outcome.is_err(), fail_import);
                } else {
                    tokio::time::timeout(Duration::from_secs(3), async {
                        while client.get_download_status(&download_id).await
                            == Some(DownloadStatus::Cancelling)
                        {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                }
                assert!(waiting);
                assert!(
                    contender_blocked,
                    "cancellation and shutdown must retain importer exclusion"
                );
                assert_eq!(marker_while_held, !final_import);
                assert!(before.queue_admissions.contains_key(&download_id));
                assert!(!before.quarantines.contains_key(&download_id));
                assert_eq!(
                    status,
                    Some(if shutdown {
                        DownloadStatus::Downloading
                    } else {
                        DownloadStatus::Cancelling
                    })
                );
                assert_eq!(
                    client.get_download_status(&download_id).await,
                    Some(if shutdown || fail_import {
                        DownloadStatus::Error
                    } else {
                        DownloadStatus::Cancelled
                    })
                );
                if final_import {
                    assert_eq!(
                        std::fs::read(destination.join("model.onnx")).unwrap(),
                        b"data"
                    );
                } else {
                    assert!(!destination.join("model.onnx").exists());
                }
                library.set_metadata_write_notifier(None);
                if !shutdown {
                    assert_eq!(client.shutdown_downloads().await.is_err(), fail_import);
                }
            }
        }
    }

    #[tokio::test]
    async fn auxiliary_callback_runs_without_destination_lease_and_cancel_stops_continuation() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(
            &library_root,
            "acme/model",
            &["config.json", "weights.gguf"],
        );
        let download_id = "aux-callback-reentrant-cancel";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        state.files = vec![
            FileToDownload {
                filename: "config.json".to_string(),
                size: None,
                sha256: None,
            },
            FileToDownload {
                filename: "weights.gguf".to_string(),
                size: Some(4),
                sha256: None,
            },
        ];
        state.filename = "config.json".to_string();
        state.total_bytes = Some(4);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("config.json"),
            b"{}",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let callback_saw_released_destination = Arc::new(AtomicBool::new(false));
        let callback_saw_released_destination_in_task = callback_saw_released_destination.clone();
        let callback_client = client.clone();
        let callback_download_id = download_id.to_string();
        let callback_runtime = tokio::runtime::Handle::current();
        let callback: AuxFilesCompleteCallback = Arc::new(move |_| {
            callback_saw_released_destination_in_task
                .store(destination_lock.try_lock().is_ok(), Ordering::SeqCst);
            let client = callback_client.clone();
            let download_id = callback_download_id.clone();
            let (cancelled_sender, cancelled_receiver) = std::sync::mpsc::channel();
            callback_runtime.spawn(async move {
                let cancelled = client.cancel_download(&download_id).await.unwrap();
                let _ = cancelled_sender.send(cancelled);
            });
            let cancelled = cancelled_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("reentrant cancellation must return while callback owns no guard");
            assert!(cancelled);
        });

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    Some(callback),
                    None,
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(download_id).await == Some(DownloadStatus::Cancelled)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("reentrant cancellation must settle the auxiliary callback worker");

        assert!(callback_saw_released_destination.load(Ordering::SeqCst));
        assert!(!verified
            .destination
            .display_path()
            .join("weights.gguf.part")
            .exists());
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn auxiliary_callback_panic_survives_cancelled_waiter_and_fails_closed() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(
            &library_root,
            "acme/model",
            &["config.json", "weights.gguf"],
        );
        let download_id = "aux-callback-panic-cancel";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.files = vec![
            FileToDownload {
                filename: "config.json".to_string(),
                size: None,
                sha256: None,
            },
            FileToDownload {
                filename: "weights.gguf".to_string(),
                size: Some(4),
                sha256: None,
            },
        ];
        state.filename = "config.json".to_string();
        state.total_bytes = Some(4);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("config.json"),
            b"{}",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let (result_sender, result_ready) = std::sync::mpsc::channel();
        let result_sender = Arc::new(std::sync::Mutex::new(Some(result_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_blocking_result_observer(Some(Arc::new({
                let result_sender = result_sender.clone();
                let release = release.clone();
                move |operation| {
                    if operation == "invoke auxiliary-files-complete callback" {
                        if let Some(sender) = result_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));

        let (during_sender, during_receiver) = tokio::sync::oneshot::channel();
        let (settled_sender, settled_receiver) = tokio::sync::oneshot::channel();
        let cancel_client = client.clone();
        let cancel_thread = std::thread::spawn(move || {
            result_ready
                .recv_timeout(Duration::from_secs(2))
                .expect("callback semantic failure must be recorded before result delivery");
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    assert!(cancel_client.cancel_download(download_id).await.unwrap());
                    let during = {
                        let states = cancel_client.downloads.read().await;
                        let state = states.get(download_id).unwrap();
                        (
                            state.status,
                            state.task_registered,
                            state.recovery_destination().is_some(),
                        )
                    };
                    let _ = during_sender.send(during);
                    release_sender.send(()).unwrap();
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while cancel_client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| state.status == DownloadStatus::Cancelling)
                        {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    let _ = settled_sender.send(());
                });
        });
        let callback: AuxFilesCompleteCallback = Arc::new(|_| {
            panic!("auxiliary callback panic sentinel");
        });
        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    Some(callback),
                    None,
                )
                .await
        );
        assert_eq!(
            during_receiver.await.unwrap(),
            (DownloadStatus::Cancelling, true, true)
        );
        settled_receiver.await.unwrap();
        cancel_thread.join().unwrap();
        client.download_tasks.set_blocking_result_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(!state.task_registered);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.recovery_destination().is_some());
    }

    #[tokio::test]
    async fn completion_callback_panic_is_observed_without_rolling_back_completed() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "completion-callback-panic";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("weights.gguf"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_in_task = callback_called.clone();
        let callback: DownloadCompletionCallback = Arc::new(move |_| {
            assert!(destination_lock.try_lock().is_ok());
            callback_called_in_task.store(true, Ordering::SeqCst);
            panic!("completion callback panic sentinel");
        });

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    Some(callback),
                    None,
                    None,
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            while !callback_called.load(Ordering::SeqCst)
                || !client
                    .download_tasks
                    .snapshot(download_id)
                    .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("completion callback panic must be caught and owner-observed");
        let _ = client.list_downloads().await;
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Completed);
        assert!(!state.task_registered);
        assert!(!state.lifecycle_failure_unverified);
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn ambient_completion_cleanup_failure_is_error_without_success_callback() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ambient-completion-failure";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("weights.gguf"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();

        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_in_task = callback_called.clone();
        let callback: DownloadCompletionCallback = Arc::new(move |_| {
            callback_called_in_task.store(true, Ordering::SeqCst);
        });
        let downloads_path = temp.path().join("downloads.json");
        let corrupted = Arc::new(AtomicBool::new(false));
        let corrupted_in_observer = corrupted.clone();
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "remove completed persisted download"
                    && !corrupted_in_observer.swap(true, Ordering::SeqCst)
                {
                    std::fs::write(&downloads_path, b"not-json").unwrap();
                }
            })));

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    Some(callback),
                    None,
                    Some(persistence),
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(download_id).await != Some(DownloadStatus::Error) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("ambient persistence cleanup failure must project Error");
        assert!(!callback_called.load(Ordering::SeqCst));
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let _ = client.list_downloads().await;
                if client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| state.lifecycle_failure_unverified)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("finished cleanup failure must become sticky lifecycle provenance");
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert!(state.lifecycle_failure_unverified);
        assert!(!state.task_registered);
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn ambient_marker_cleanup_failure_is_error_and_retains_persisted_recovery_truth() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ambient-marker-cleanup-failure";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        let files = state.files.clone();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("weights.gguf"),
            b"done",
        )
        .unwrap();
        let marker = verified.destination.display_path().join(".pumas_download");
        std::fs::create_dir(&marker).unwrap();
        let callback_called = Arc::new(AtomicBool::new(false));
        let callback: DownloadCompletionCallback = Arc::new({
            let callback_called = callback_called.clone();
            move |_| callback_called.store(true, Ordering::SeqCst)
        });

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    Some(callback),
                    None,
                    Some(persistence.clone()),
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let failed = client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && state.lifecycle_failure_unverified
                            && !state.task_registered
                    });
                if failed && !client.download_tasks.contains(download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("marker cleanup failure must settle sticky Error");
        assert!(marker.is_dir());
        assert!(!callback_called.load(Ordering::SeqCst));
        let persisted = persistence.load_all();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].download_id, download_id);
        assert_eq!(persisted[0].status, DownloadStatus::Error);
    }

    #[tokio::test]
    async fn recovery_error_and_inactive_pause_retain_capability_without_persistence() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let pausing_id = "recovery-pausing";
        client.downloads.write().await.insert(
            pausing_id.to_string(),
            recovery_test_state(&verified, pausing_id, DownloadStatus::Downloading, true),
        );
        let pausing_worker = client
            .download_tasks
            .prepare(pausing_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(pausing_worker)
            .unwrap()
            .start();
        assert!(client.pause_download(pausing_id).await.unwrap());
        let downloads = client.downloads.read().await;
        let state = downloads.get(pausing_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Pausing);
        assert!(state.recovery_destination().is_some());
        drop(downloads);
        assert!(client.cancel_download(pausing_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(pausing_id).await != Some(DownloadStatus::Cancelled) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("test pause owner should settle before reusing the destination");

        let paused_id = "recovery-paused";
        client.downloads.write().await.insert(
            paused_id.to_string(),
            recovery_test_state(&verified, paused_id, DownloadStatus::Downloading, true),
        );

        let progress = client
            .list_downloads()
            .await
            .into_iter()
            .find(|download| download.download_id == paused_id)
            .unwrap();
        assert_eq!(progress.status, DownloadStatus::Paused);
        let downloads = client.downloads.read().await;
        let state = downloads.get(paused_id).unwrap();
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
        drop(downloads);
        assert!(persistence.load_all().is_empty());
        assert!(!temp.path().join("downloads.json").exists());

        let error_id = "recovery-error";
        client.downloads.write().await.insert(
            error_id.to_string(),
            recovery_test_state(&verified, error_id, DownloadStatus::Queued, false),
        );
        std::fs::remove_dir(verified.destination.display_path()).unwrap();
        std::fs::write(verified.destination.display_path(), b"not a directory").unwrap();
        let (cancel_flag, pause_flag) = {
            let downloads = client.downloads.read().await;
            let error_state = downloads.get(error_id).unwrap();
            (
                error_state.cancel_flag.clone(),
                error_state.pause_flag.clone(),
            )
        };
        let spawned = client
            .spawn_download_task(
                error_id.to_string(),
                verified.repo_id.clone(),
                vec![FileToDownload {
                    filename: "weights.gguf".to_string(),
                    size: Some(4),
                    sha256: None,
                }],
                DownloadDestination::Recovery(verified.destination.clone()),
                cancel_flag,
                pause_flag,
                None,
                None,
                None,
            )
            .await;
        assert!(spawned);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(error_id).await == Some(DownloadStatus::Error) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("capability failure should become a typed download error");
        assert!(client
            .downloads
            .read()
            .await
            .get(error_id)
            .unwrap()
            .recovery_destination()
            .is_some());
        assert!(persistence.load_all().is_empty());
    }

    #[tokio::test]
    async fn ordinary_resume_preserves_ambient_callbacks_and_persistence_contract() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let completion_called = Arc::new(AtomicBool::new(false));
        let completion_observer = completion_called.clone();
        client.set_completion_callback(Arc::new(move |_| {
            completion_observer.store(true, Ordering::SeqCst);
        }));
        let aux_called = Arc::new(AtomicBool::new(false));
        let aux_observer = aux_called.clone();
        client.set_aux_complete_callback(Arc::new(move |_| {
            aux_observer.store(true, Ordering::SeqCst);
        }));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let download_id = "ordinary-resume";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("weights.gguf.part"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();
        let mut updates = client.subscribe_download_updates();

        assert!(client.resume_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let update = updates.recv().await.unwrap();
                if update.snapshot.downloads.iter().any(|download| {
                    download.download_id == download_id
                        && download.status == DownloadStatus::Completed
                }) {
                    break;
                }
            }
        })
        .await
        .expect("byte-complete ordinary resume should finish without network access");

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if completion_called.load(Ordering::SeqCst) && persistence.load_all().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("ordinary completion callback and persistence cleanup should finish");
        assert!(aux_called.load(Ordering::SeqCst));
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Completed);
        assert!(state.recovery_destination().is_none());
    }

    #[tokio::test]
    async fn cancelling_ordinary_resume_before_commit_preserves_paused_state() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancelled-ordinary-resume";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (preparing_sender, preparing_receiver) = tokio::sync::oneshot::channel();
        let preparing_sender = Arc::new(std::sync::Mutex::new(Some(preparing_sender)));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let preparing_sender = preparing_sender.clone();
                move |operation, observed_id| {
                    if operation == "prepare-download-task" && observed_id == download_id {
                        if let Some(sender) = preparing_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                        }
                    }
                }
            })));
        let auth_guard = client.auth_token.write().await;
        let resume = {
            let client = client.clone();
            tokio::spawn(async move { client.resume_download(download_id).await })
        };
        tokio::time::timeout(Duration::from_secs(1), preparing_receiver)
            .await
            .expect("resume should reach its pre-admission preparation")
            .unwrap();
        assert_eq!(
            client.get_download_status(download_id).await,
            Some(DownloadStatus::Paused)
        );
        resume.abort();
        let _ = resume.await;
        drop(auth_guard);
        client.download_tasks.set_ambient_admission_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Paused);
        assert!(!state.task_registered);
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn cancelling_resume_caller_after_commit_keeps_started_owner_in_both_modes() {
        for recovery in [false, true] {
            let temp = TempDir::new().unwrap();
            let mode = if recovery { "recovery" } else { "ambient" };
            let library_root = temp.path().join(mode).join("library");
            let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
            let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
            client
                .configure_download_destination_root(&library_root)
                .unwrap();
            let persistence = Arc::new(DownloadPersistence::new(temp.path()));
            client.set_persistence(persistence.clone());
            let client = Arc::new(client);
            let download_id = format!("cancelled-{mode}-resume-after-commit");
            let mut state =
                recovery_test_state(&verified, &download_id, DownloadStatus::Paused, false);
            if !recovery {
                state.make_managed_for_test();
                persist_state_fixture(&persistence, &mut state);
            }
            client
                .downloads
                .write()
                .await
                .insert(download_id.clone(), state);
            let destination_guard = client
                .destination_lock(&verified.destination.identity())
                .await
                .lock_owned()
                .await;
            let publication_guard = client.download_publications.capture.lock().await;
            let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
            let started_sender = Arc::new(std::sync::Mutex::new(Some(started_sender)));
            let observed_id = download_id.clone();
            client
                .download_tasks
                .set_ambient_admission_observer(Some(Arc::new(move |operation, download_id| {
                    if operation == "resume-started" && download_id == observed_id {
                        if let Some(sender) = started_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                        }
                    }
                })));
            let resume = {
                let client = client.clone();
                let download_id = download_id.clone();
                tokio::spawn(async move { client.resume_download(&download_id).await })
            };
            tokio::time::timeout(Duration::from_secs(1), started_receiver)
                .await
                .expect("resume should synchronously start its committed owner")
                .unwrap();
            resume.abort();
            let _ = resume.await;
            drop(publication_guard);

            let downloads = client.downloads.read().await;
            let state = downloads.get(&download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Queued);
            assert!(state.task_registered);
            drop(downloads);
            assert!(client
                .download_tasks
                .snapshot(&download_id)
                .is_some_and(|task| {
                    task.role == TaskRole::Worker && task.started && !task.outer_finished
                }));
            client.download_tasks.set_ambient_admission_observer(None);
            drop(destination_guard);
            assert!(client.cancel_download(&download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                while client.get_download_status(&download_id).await
                    != Some(DownloadStatus::Cancelled)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("owned resume must remain cancellable after its caller is aborted");
        }
    }

    #[tokio::test]
    async fn pause_cannot_overwrite_concurrent_completion() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "pause-complete";
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, false);
        state.progress = 1.0;
        let mut downloads = client.downloads.write().await;
        downloads.insert(download_id.to_string(), state);
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let pause_task = {
            let client = client.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                client.pause_download(download_id).await.unwrap()
            })
        };
        barrier.wait().await;
        tokio::task::yield_now().await;
        let state = downloads.get_mut(download_id).unwrap();
        state.status = DownloadStatus::Completed;
        state.make_managed_for_test();
        drop(downloads);

        assert!(!pause_task.await.unwrap());
        assert_eq!(
            client.get_download_status(download_id).await,
            Some(DownloadStatus::Completed)
        );
    }

    #[tokio::test]
    async fn missing_admission_refuses_destination_work_even_when_pause_races() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "pause-before-destination-missing-row";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let files = state.files.clone();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;
        let destination_work = Arc::new(AtomicU64::new(0));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let destination_work = destination_work.clone();
            move |operation| {
                if operation == "prepare ambient destination" {
                    destination_work.fetch_add(1, Ordering::SeqCst);
                }
            }
        })));

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    Some(persistence.clone()),
                )
                .await
        );
        // Admission validation precedes the destination mutex. It may reject
        // before the pause request can attach; neither order may authorize work.
        if !client.pause_download(download_id).await.unwrap() {
            assert_eq!(
                client.get_download_status(download_id).await,
                Some(DownloadStatus::Error)
            );
        }
        drop(destination_guard);

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                if client.get_download_status(download_id).await == Some(DownloadStatus::Error)
                    && !client.download_tasks.contains(download_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("missing durable row must settle the pause as a sticky error");
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(!state.task_registered);
        assert!(state.lifecycle_failure_unverified);
        drop(downloads);
        assert_eq!(destination_work.load(Ordering::SeqCst), 0);
        assert!(persistence.load_all().is_empty());
        assert!(!temp.path().join("downloads.json").exists());
        assert!(!verified
            .destination
            .display_path()
            .join(".pumas_download")
            .exists());
        assert!(!verified
            .destination
            .display_path()
            .join("weights.gguf.part")
            .exists());
        assert!(!verified
            .destination
            .display_path()
            .join("weights.gguf")
            .exists());
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn pause_persistence_error_is_sticky_and_never_projects_paused() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "pause-corrupt-persistence";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        let files = state.files.clone();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let destination_guard = client
            .destination_lock(&verified.destination.identity())
            .await
            .lock_owned()
            .await;
        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    Some(persistence),
                )
                .await
        );
        assert!(client.pause_download(download_id).await.unwrap());
        let store_path = temp.path().join("downloads.json");
        std::fs::write(&store_path, b"not-json").unwrap();
        drop(destination_guard);

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let failed = client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| {
                        state.status == DownloadStatus::Error
                            && state.lifecycle_failure_unverified
                            && !state.task_registered
                    });
                if failed && !client.download_tasks.contains(download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("pause persistence error must settle sticky Error");
        assert_ne!(
            client.get_download_status(download_id).await,
            Some(DownloadStatus::Paused)
        );
        std::fs::remove_file(store_path).unwrap();
    }

    #[tokio::test]
    async fn pause_rejects_absent_gated_finished_and_non_worker_owners() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        assert!(!client.pause_download("absent").await.unwrap());

        let gated_id = "pause-gated-worker";
        client.downloads.write().await.insert(
            gated_id.to_string(),
            recovery_test_state(&verified, gated_id, DownloadStatus::Queued, true),
        );
        let gated = client
            .download_tasks
            .prepare(gated_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        let gated = client.download_tasks.install_gated(gated).unwrap();
        assert!(!client.pause_download(gated_id).await.unwrap());
        assert_eq!(
            client.get_download_status(gated_id).await,
            Some(DownloadStatus::Queued)
        );
        drop(gated);
        client.download_tasks.rescue_abandoned();
        client.downloads.write().await.remove(gated_id);

        let finished_id = "pause-finished-worker";
        client.downloads.write().await.insert(
            finished_id.to_string(),
            recovery_test_state(&verified, finished_id, DownloadStatus::Downloading, true),
        );
        let finished = client
            .download_tasks
            .prepare(finished_id.to_string(), TaskRole::Worker, |_| async {})
            .unwrap();
        client
            .download_tasks
            .install_gated(finished)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(finished_id)
                .is_some_and(|task| task.outer_finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!client.pause_download(finished_id).await.unwrap());
        client.observe_finished_download_tasks().await;

        let transition_id = "pause-recovery-transition";
        client.downloads.write().await.insert(
            transition_id.to_string(),
            recovery_test_state(&verified, transition_id, DownloadStatus::Downloading, true),
        );
        let transition = client
            .download_tasks
            .prepare(
                transition_id.to_string(),
                TaskRole::RecoveryTransition,
                |_| async { std::future::pending::<()>().await },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(transition)
            .unwrap()
            .start();
        assert!(!client.pause_download(transition_id).await.unwrap());
        assert_eq!(
            client.get_download_status(transition_id).await,
            Some(DownloadStatus::Downloading)
        );
        assert!(client.cancel_download(transition_id).await.unwrap());
    }

    #[derive(Clone, Copy)]
    #[repr(u8)]
    enum PauseAdmissionStage {
        Unobserved = 0,
        RuntimeThreadEntered = 1,
        CallingStart = 2,
        PreparingTask = 3,
        InventoryChecked = 4,
        WorkerStarted = 5,
        StartReturned = 6,
    }

    struct PauseAdmissionObservation(std::sync::atomic::AtomicU8);

    impl PauseAdmissionObservation {
        fn new() -> Self {
            Self(std::sync::atomic::AtomicU8::new(
                PauseAdmissionStage::Unobserved as u8,
            ))
        }

        fn record(&self, stage: PauseAdmissionStage) {
            self.0.store(stage as u8, Ordering::Relaxed);
        }

        fn observe(&self, operation: &str) {
            let stage = match operation {
                "prepare-download-task" => PauseAdmissionStage::PreparingTask,
                "admission-inventory-checked" => PauseAdmissionStage::InventoryChecked,
                "ordinary-start-started" => PauseAdmissionStage::WorkerStarted,
                _ => return,
            };
            self.record(stage);
        }

        fn describe(&self, runtime_finished: bool) -> String {
            // Diagnostic only: one atomic read, no owner lock, filesystem read,
            // or join. This is the last observation, not proof of a pending phase.
            let stage = match self.0.load(Ordering::Relaxed) {
                0 => "unobserved",
                1 => "runtime-thread-entered",
                2 => "calling-start",
                3 => "prepare-download-task",
                4 => "admission-inventory-checked",
                5 => "ordinary-start-started",
                6 => "start-returned",
                _ => unreachable!("only PauseAdmissionStage values are recorded"),
            };
            format!(
                "last observed admission stage={stage}; worker runtime finished={runtime_finished}"
            )
        }
    }

    #[test]
    fn pause_admission_diagnostics_report_observations_without_waiting_for_workers() {
        let observation = PauseAdmissionObservation::new();
        assert_eq!(
            observation.describe(false),
            "last observed admission stage=unobserved; worker runtime finished=false"
        );
        observation.observe("prepare-download-task");
        observation.observe("unrelated-event");
        assert_eq!(
            observation.describe(false),
            "last observed admission stage=prepare-download-task; worker runtime finished=false"
        );
        observation.observe("admission-inventory-checked");
        assert_eq!(observation.describe(true), "last observed admission stage=admission-inventory-checked; worker runtime finished=true");
        observation.record(PauseAdmissionStage::StartReturned);
        assert_eq!(
            observation.describe(true),
            "last observed admission stage=start-returned; worker runtime finished=true"
        );
    }

    #[tokio::test]
    async fn pause_after_a_worker_check_is_settled_by_the_same_generation() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
        cache_repo_tree(
            &client,
            "acme/model",
            vec![LfsFileInfo {
                filename: "weights.gguf".to_string(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let destination = temp.path().join("library").join("model");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("weights.gguf"), b"done").unwrap();
        let physical_guard = client
            .destination_lock(&destination_identity(&client, &destination))
            .await
            .lock_owned()
            .await;
        let (reached_sender, reached) = std::sync::mpsc::channel();
        let reached_sender = Arc::new(std::sync::Mutex::new(Some(reached_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_worker_projection_observer(Some(Arc::new({
                let reached_sender = reached_sender.clone();
                let release = release.clone();
                move |projection| {
                    if projection == "before-existing-file-projection" {
                        if let Some(sender) = reached_sender.lock().unwrap().take() {
                            sender.send(()).unwrap();
                            release.lock().unwrap().recv().unwrap();
                        }
                    }
                }
            })));
        let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
        let (id_sender, id_receiver) = std::sync::mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let admission = Arc::new(PauseAdmissionObservation::new());
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let admission = admission.clone();
                move |operation, _| admission.observe(operation)
            })));
        let worker_runtime = std::thread::spawn({
            let client = client.clone();
            let destination = destination.clone();
            let admission = admission.clone();
            move || {
                admission.record(PauseAdmissionStage::RuntimeThreadEntered);
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async move {
                        admission.record(PauseAdmissionStage::CallingStart);
                        let download_id = client
                            .start_download(&request, &destination, None)
                            .await
                            .unwrap();
                        admission.record(PauseAdmissionStage::StartReturned);
                        id_sender.send(download_id).unwrap();
                        let _ = shutdown_receiver.await;
                    });
            }
        });
        let download_id = id_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_else(|error| {
                panic!(
                    "start must return its admitted ID: {error:?}; {}",
                    admission.describe(worker_runtime.is_finished())
                )
            });
        drop(physical_guard);
        reached
            .recv_timeout(Duration::from_secs(1))
            .expect("Worker must reach the after-check projection seam");
        assert!(client.pause_download(&download_id).await.unwrap());
        release_sender.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                client.observe_finished_download_tasks().await;
                let settled =
                    client
                        .downloads
                        .read()
                        .await
                        .get(&download_id)
                        .is_some_and(|state| {
                            state.status == DownloadStatus::Paused && !state.task_registered
                        });
                if settled && !client.download_tasks.contains(&download_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the Worker that lost the projection race must settle Paused");
        assert_eq!(
            client
                .destination_executions
                .claim_count(&destination_identity(&client, &destination)),
            1
        );
        client.download_tasks.set_worker_projection_observer(None);
        let _ = shutdown_sender.send(());
        worker_runtime.join().unwrap();
    }

    #[tokio::test]
    async fn pause_is_rejected_after_final_file_commit_and_during_cleanup() {
        for block_at in [
            "terminal-cleanup-committed",
            "remove ambient download marker",
        ] {
            let temp = TempDir::new().unwrap();
            let client = Arc::new(configured_download_client(temp.path().join("cache")).unwrap());
            cache_repo_tree(
                &client,
                "acme/model",
                vec![LfsFileInfo {
                    filename: "weights.gguf".to_string(),
                    size: 4,
                    sha256: "a".repeat(64),
                }],
                Vec::new(),
            );
            let destination = temp.path().join("library").join("model");
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(destination.join("weights.gguf"), b"done").unwrap();
            let physical_guard = client
                .destination_lock(&destination_identity(&client, &destination))
                .await
                .lock_owned()
                .await;
            let (reached_sender, reached) = std::sync::mpsc::channel();
            let reached_sender = Arc::new(std::sync::Mutex::new(Some(reached_sender)));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = Arc::new(std::sync::Mutex::new(Some(release)));
            if block_at == "terminal-cleanup-committed" {
                client
                    .download_tasks
                    .set_worker_projection_observer(Some(Arc::new({
                        let reached_sender = reached_sender.clone();
                        let release = release.clone();
                        move |projection| {
                            if projection == "terminal-cleanup-committed" {
                                if let Some(sender) = reached_sender.lock().unwrap().take() {
                                    sender.send(()).unwrap();
                                    release.lock().unwrap().take().unwrap().recv().unwrap();
                                }
                            }
                        }
                    })));
            } else {
                client.download_tasks.set_blocking_observer(Some(Arc::new({
                    let reached_sender = reached_sender.clone();
                    let release = release.clone();
                    move |operation| {
                        if operation == "remove ambient download marker" {
                            if let Some(sender) = reached_sender.lock().unwrap().take() {
                                sender.send(()).unwrap();
                                release.lock().unwrap().take().unwrap().recv().unwrap();
                            }
                        }
                    }
                })));
            }
            let request = recovery_test_request("acme/model", &["weights.gguf".to_string()]);
            let (id_sender, id_receiver) = std::sync::mpsc::channel();
            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            let admission = Arc::new(PauseAdmissionObservation::new());
            client
                .download_tasks
                .set_ambient_admission_observer(Some(Arc::new({
                    let admission = admission.clone();
                    move |operation, _| admission.observe(operation)
                })));
            let worker_runtime = std::thread::spawn({
                let client = client.clone();
                let destination = destination.clone();
                let admission = admission.clone();
                move || {
                    admission.record(PauseAdmissionStage::RuntimeThreadEntered);
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(async move {
                            admission.record(PauseAdmissionStage::CallingStart);
                            let download_id = client
                                .start_download(&request, &destination, None)
                                .await
                                .unwrap();
                            admission.record(PauseAdmissionStage::StartReturned);
                            id_sender.send(download_id).unwrap();
                            let _ = shutdown_receiver.await;
                        });
                }
            });
            let download_id = id_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("start must return its admitted ID: {error:?}; cleanup fixture={block_at}; {}", admission.describe(worker_runtime.is_finished())));
            drop(physical_guard);
            reached
                .recv_timeout(Duration::from_secs(1))
                .expect("Worker must reach terminal cleanup");
            assert!(!client.pause_download(&download_id).await.unwrap());
            assert_eq!(
                client.downloads.read().await[&download_id].status,
                DownloadStatus::Downloading
            );
            release_sender.send(()).unwrap();

            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    client.observe_finished_download_tasks().await;
                    if client.downloads.read().await[&download_id].status
                        == DownloadStatus::Completed
                        && !client.download_tasks.contains(&download_id)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("terminal cleanup must complete without a stranded Pausing state");
            assert_eq!(
                client
                    .destination_executions
                    .claim_count(&destination_identity(&client, &destination)),
                0
            );
            let _ = shutdown_sender.send(());
            worker_runtime.join().unwrap();
        }
    }

    #[tokio::test]
    async fn recovery_resume_and_cancel_overlap_ends_cancelled_without_registered_task() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "resume-cancel";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Paused, false),
        );
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let resume = {
            let client = client.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                client.resume_download(download_id).await.unwrap()
            })
        };
        let cancel = {
            let client = client.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                client.cancel_download(download_id).await.unwrap()
            })
        };
        barrier.wait().await;
        let _ = resume.await.unwrap();
        assert!(cancel.await.unwrap());

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(download_id).await == Some(DownloadStatus::Cancelled)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("caller-independent cancellation should reach its terminal state");

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
        drop(downloads);
        client.observe_finished_download_tasks().await;
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn cancelling_cancel_future_cannot_detach_recovery_finalizer() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-finalizer";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (worker_started, started) = tokio::sync::oneshot::channel();
        let (release_worker, worker_release) = std::sync::mpsc::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |task_context| async move {
                    let _ = task_context
                        .run_blocking(move || {
                            let _ = worker_started.send(());
                            let _ = worker_release.recv();
                        })
                        .await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        started.await.unwrap();

        let cancellation = {
            let client = client.clone();
            tokio::spawn(async move { client.cancel_download(download_id).await })
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if client.get_download_status(download_id).await == Some(DownloadStatus::Cancelling)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancel should enter its owned terminal transition");
        cancellation.abort();
        let _ = cancellation.await;

        {
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert!(state.task_registered);
            assert!(state.recovery_destination().is_some());
        }
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| !task.finished));
        assert!(client.cancel_download(download_id).await.unwrap());
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| !task.finished));

        release_worker.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let downloads = client.downloads.read().await;
                let state = downloads.get(download_id).unwrap();
                if state.status == DownloadStatus::Cancelled
                    && !state.task_registered
                    && state.recovery_destination().is_none()
                {
                    break;
                }
                drop(downloads);
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the caller-independent finalizer should settle cancellation");
    }

    #[tokio::test]
    async fn recovery_finalizer_drains_each_capability_mutation_before_terminal() {
        #[derive(Clone, Copy)]
        enum Operation {
            CreateParent,
            TruncatePart,
            WritePart,
            FlushPart,
            RemovePart,
            RenamePart,
            RemoveMarker,
        }

        for (index, operation) in [
            Operation::CreateParent,
            Operation::TruncatePart,
            Operation::WritePart,
            Operation::FlushPart,
            Operation::RemovePart,
            Operation::RenamePart,
            Operation::RemoveMarker,
        ]
        .into_iter()
        .enumerate()
        {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            let client = Arc::new(recovery_test_client(
                temp.path().join("cache"),
                &library_root,
            ));
            let verified = verified_recovery(&library_root, "acme/model", &["nested/weights.gguf"]);
            let destination = DownloadDestination::Recovery(verified.destination.clone());
            let filename = "nested/weights.gguf";
            std::fs::create_dir_all(verified.destination.display_path().join("nested")).unwrap();
            match operation {
                Operation::RemovePart | Operation::RenamePart => {
                    std::fs::write(
                        verified
                            .destination
                            .display_path()
                            .join("nested/weights.gguf.part"),
                        b"partial",
                    )
                    .unwrap();
                }
                Operation::RemoveMarker => {
                    std::fs::write(
                        verified.destination.display_path().join(".pumas_download"),
                        b"{}",
                    )
                    .unwrap();
                }
                Operation::CreateParent
                | Operation::TruncatePart
                | Operation::WritePart
                | Operation::FlushPart => {}
            }

            let expected_label = match operation {
                Operation::CreateParent => "create file parent",
                Operation::TruncatePart => "open partial download file",
                Operation::WritePart => "write partial download file",
                Operation::FlushPart => "flush partial download file",
                Operation::RemovePart => "remove partial download file",
                Operation::RenamePart => "promote partial download file",
                Operation::RemoveMarker => "remove download marker",
            };
            let (started_sender, started) = tokio::sync::oneshot::channel();
            let started_sender = Arc::new(std::sync::Mutex::new(Some(started_sender)));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = Arc::new(std::sync::Mutex::new(release));
            client.download_tasks.set_blocking_observer(Some(Arc::new({
                let started_sender = started_sender.clone();
                let release = release.clone();
                move |label| {
                    if label == expected_label {
                        if let Some(sender) = started_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));

            let download_id = format!("mutation-{index}");
            client.downloads.write().await.insert(
                download_id.clone(),
                recovery_test_state(&verified, &download_id, DownloadStatus::Downloading, true),
            );
            let prepared = client
                .download_tasks
                .prepare(
                    download_id.clone(),
                    TaskRole::Worker,
                    move |task_context| async move {
                        let _ = match operation {
                            Operation::CreateParent => {
                                destination.prepare_file(&task_context, filename).await
                            }
                            Operation::TruncatePart => destination
                                .open_part(&task_context, filename, false)
                                .await
                                .map(drop),
                            Operation::WritePart => {
                                match destination.open_part(&task_context, filename, false).await {
                                    Ok(mut file) => {
                                        file.write_all(&task_context, b"sentinel").await
                                    }
                                    Err(error) => Err(error),
                                }
                            }
                            Operation::FlushPart => {
                                match destination.open_part(&task_context, filename, false).await {
                                    Ok(mut file) => file.flush(&task_context).await,
                                    Err(error) => Err(error),
                                }
                            }
                            Operation::RemovePart => {
                                destination.remove_part(&task_context, filename).await
                            }
                            Operation::RenamePart => {
                                destination
                                    .rename_part_to_file(&task_context, filename)
                                    .await
                            }
                            Operation::RemoveMarker => {
                                destination.remove_marker(&task_context).await
                            }
                        };
                    },
                )
                .unwrap();
            client
                .download_tasks
                .install_gated(prepared)
                .unwrap()
                .start();
            started.await.unwrap();

            assert!(client.cancel_download(&download_id).await.unwrap());
            tokio::task::yield_now().await;
            {
                let downloads = client.downloads.read().await;
                let state = downloads.get(&download_id).unwrap();
                assert_eq!(state.status, DownloadStatus::Cancelling);
                assert!(state.recovery_destination().is_some());
                assert!(state.task_registered);
            }
            release_sender.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let downloads = client.downloads.read().await;
                    let state = downloads.get(&download_id).unwrap();
                    if state.status == DownloadStatus::Cancelled {
                        assert!(state.recovery_destination().is_none());
                        assert!(!state.task_registered);
                        break;
                    }
                    drop(downloads);
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("terminal callback must follow the capability mutation drain");
            let snapshot = || {
                [
                    verified.destination.display_path().join(filename),
                    verified
                        .destination
                        .display_path()
                        .join(format!("{filename}.part")),
                    verified.destination.display_path().join(".pumas_download"),
                ]
                .map(|path| std::fs::read(path).ok())
            };
            let terminal_snapshot = snapshot();
            tokio::task::yield_now().await;
            assert_eq!(
                snapshot(),
                terminal_snapshot,
                "no mutation may outlive terminal state"
            );
            client.download_tasks.set_blocking_observer(None);
        }
    }

    #[tokio::test]
    async fn ambient_finalizer_drains_inflight_write_before_cleanup_and_terminal() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ambient-held-write-cancel";
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true);
        state.make_managed_for_test();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let destination = DownloadDestination::Managed(verified.destination.clone());
        let (started_sender, started) = tokio::sync::oneshot::channel();
        let started_sender = Arc::new(std::sync::Mutex::new(Some(started_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let started_sender = started_sender.clone();
            let release = release.clone();
            move |operation| {
                if operation == "write ambient partial download file" {
                    if let Some(sender) = started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            }
        })));
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |task_context| async move {
                    let mut file = destination
                        .open_part(&task_context, "weights.gguf", false)
                        .await
                        .unwrap();
                    file.write_all(&task_context, b"stale-after-cancel")
                        .await
                        .unwrap();
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(2), started)
            .await
            .unwrap()
            .unwrap();

        assert!(client.cancel_download(download_id).await.unwrap());
        {
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Cancelling);
            assert!(state.task_registered);
        }
        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(download_id).await != Some(DownloadStatus::Cancelled) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("ambient terminal must follow the owner-visible write and cleanup");
        let part = verified
            .destination
            .display_path()
            .join("weights.gguf.part");
        assert!(!part.exists());
        tokio::task::yield_now().await;
        assert!(
            !part.exists(),
            "no ambient mutation may outlive terminal state"
        );
        client.download_tasks.set_blocking_observer(None);
    }

    #[tokio::test]
    async fn recovery_worker_panic_cannot_publish_cancelled_or_release_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "worker-panic-cancel";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                panic!("worker failure sentinel");
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn recovery_nested_panic_cannot_publish_cancelled_or_release_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "nested-panic-cancel";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (nested_finished_sender, nested_finished) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_blocking(|| panic!("nested failure sentinel"))
                        .await;
                    let _ = nested_finished_sender.send(());
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        nested_finished.await.unwrap();

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn cancellation_cleanup_failure_is_not_reported_as_cancelled() {
        let temp = TempDir::new().unwrap();
        let blocked_root = temp.path().join("not-a-directory");
        std::fs::write(&blocked_root, b"sentinel").unwrap();
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client.set_persistence(Arc::new(DownloadPersistence::new(&blocked_root)));
        let download_id = "cleanup-failure-cancel";
        let library_root = temp.path().join("library");
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true);
        state.make_managed_for_test();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn panicked_cancel_finalizer_is_reconciled_to_error() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "panicked-finalizer";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Cancelling, true),
        );
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::CancelFinalizer,
                |_| async { panic!("finalizer failure sentinel") },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let _ = client.list_downloads().await;
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn repeated_public_cancel_repairs_finished_and_panicked_finalizers() {
        for panicked in [false, true] {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            let client = recovery_test_client(temp.path().join("cache"), &library_root);
            let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
            let download_id = if panicked {
                "repeat-panicked-finalizer"
            } else {
                "repeat-finished-finalizer"
            };
            client.downloads.write().await.insert(
                download_id.to_string(),
                recovery_test_state(&verified, download_id, DownloadStatus::Cancelling, true),
            );
            let prepared = client
                .download_tasks
                .prepare(
                    download_id.to_string(),
                    TaskRole::CancelFinalizer,
                    move |_| async move {
                        assert!(!panicked, "finalizer failure sentinel");
                    },
                )
                .unwrap();
            client
                .download_tasks
                .install_gated(prepared)
                .unwrap()
                .start();
            tokio::time::timeout(Duration::from_secs(1), async {
                while !client
                    .download_tasks
                    .snapshot(download_id)
                    .is_some_and(|task| task.finished)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();

            assert!(client.cancel_download(download_id).await.unwrap());
            let expected = DownloadStatus::Error;
            tokio::time::timeout(Duration::from_secs(2), async {
                while client.get_download_status(download_id).await != Some(expected) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("repeat public cancel must settle the finished predecessor");
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert!(!state.task_registered);
            assert!(state.lifecycle_failure_unverified);
            assert!(state.recovery_destination().is_some());
            drop(downloads);
            assert!(client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| { task.role == TaskRole::CancelFinalizer && task.finished }));
        }
    }

    #[tokio::test]
    async fn direct_cancel_preserves_finished_active_worker_obligation() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-finished-active-worker";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (finish_sender, finish) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |_| async move {
                    let _ = finish.await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();

        let (replacement_reached_sender, replacement_reached) = std::sync::mpsc::channel();
        let replacement_reached_sender =
            Arc::new(std::sync::Mutex::new(Some(replacement_reached_sender)));
        let (release_replacement_sender, release_replacement) = std::sync::mpsc::channel();
        let release_replacement = Arc::new(std::sync::Mutex::new(release_replacement));
        client
            .download_tasks
            .set_cancel_replacement_observer(Some(Arc::new({
                let replacement_reached_sender = replacement_reached_sender.clone();
                let release_replacement = release_replacement.clone();
                move || {
                    if let Some(sender) = replacement_reached_sender.lock().unwrap().take() {
                        sender.send(()).unwrap();
                        release_replacement.lock().unwrap().recv().unwrap();
                    }
                }
            })));
        let cancel_client = client.clone();
        let cancel = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let result = cancel_client.cancel_download(download_id).await;
                    tokio::time::timeout(Duration::from_secs(2), async {
                        loop {
                            let terminal = cancel_client
                                .downloads
                                .read()
                                .await
                                .get(download_id)
                                .is_some_and(|state| !state.task_registered);
                            if terminal {
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .expect("the cancellation runtime must drive its finalizer to terminal");
                    result
                })
        });
        replacement_reached
            .recv_timeout(Duration::from_secs(2))
            .expect("cancel must reach the exact pre-replacement seam");
        assert!(!client.download_tasks.outer_finished_for_test(download_id));
        finish_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client.download_tasks.outer_finished_for_test(download_id) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Worker must finish after state capture but before lifecycle replacement");
        release_replacement_sender.send(()).unwrap();
        assert!(cancel.join().unwrap().unwrap());
        client.download_tasks.set_cancel_replacement_observer(None);

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let failed_closed =
                    client
                        .downloads
                        .read()
                        .await
                        .get(download_id)
                        .is_some_and(|state| {
                            state.status == DownloadStatus::Error
                                && !state.task_registered
                                && state.lifecycle_failure_unverified
                                && state.recovery_destination().is_some()
                        });
                if failed_closed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("direct cancel must retain the finished Worker's unresolved obligation");
    }

    #[tokio::test]
    async fn cancel_drains_held_nested_work_after_outer_finished_and_preserves_obligation() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-outer-finished-nested-held";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (nested_started_sender, nested_started) = tokio::sync::oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |context| async move {
                    context
                        .register_blocking_without_wait_for_test("held after outer", move || {
                            let _ = nested_started_sender.send(());
                            let _ = release.recv();
                        })
                        .unwrap();
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), nested_started)
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client.download_tasks.outer_finished_for_test(download_id) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Worker outer must finish before cancellation replacement");

        assert!(client.cancel_download(download_id).await.unwrap());
        {
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Cancelling);
            assert!(state.task_registered);
            assert!(state.recovery_destination().is_some());
        }

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let failed_closed =
                    client
                        .downloads
                        .read()
                        .await
                        .get(download_id)
                        .is_some_and(|state| {
                            state.status == DownloadStatus::Error
                                && !state.task_registered
                                && state.lifecycle_failure_unverified
                                && state.recovery_destination().is_some()
                        });
                if failed_closed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("held nested work must drain before fail-closed settlement");
    }

    #[tokio::test]
    async fn finished_active_worker_is_projected_to_sticky_error_without_cancel() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "project-finished-active-worker";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, false),
        );
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {})
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let _ = client.list_downloads().await;
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.recovery_destination().is_some());
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn cancel_superseding_held_terminal_projection_inherits_state_obligation() {
        for (role, status, download_id) in [
            (
                TaskRole::Worker,
                DownloadStatus::Downloading,
                "cancel-held-worker-projection",
            ),
            (
                TaskRole::CancelFinalizer,
                DownloadStatus::Cancelling,
                "cancel-held-finalizer-projection",
            ),
        ] {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            let client = Arc::new(recovery_test_client(
                temp.path().join("cache"),
                &library_root,
            ));
            let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
            client.downloads.write().await.insert(
                download_id.to_string(),
                recovery_test_state(&verified, download_id, status, false),
            );
            let prepared = client
                .download_tasks
                .prepare(download_id.to_string(), role, |_| async {})
                .unwrap();
            client
                .download_tasks
                .install_gated(prepared)
                .unwrap()
                .start();
            tokio::time::timeout(Duration::from_secs(1), async {
                while !client
                    .download_tasks
                    .snapshot(download_id)
                    .is_some_and(|task| task.finished)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();

            let (projection_sender, projection_reached) = std::sync::mpsc::channel();
            let projection_sender = Arc::new(std::sync::Mutex::new(Some(projection_sender)));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = Arc::new(std::sync::Mutex::new(release));
            client
                .download_tasks
                .set_projection_observer(Some(Arc::new({
                    let projection_sender = projection_sender.clone();
                    let release = release.clone();
                    move |projection| {
                        if projection == "finished-task" {
                            if let Some(sender) = projection_sender.lock().unwrap().take() {
                                let _ = sender.send(());
                                let _ = release.lock().unwrap().recv();
                            }
                        }
                    }
                })));
            let listing_client = client.clone();
            let listing = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(listing_client.list_downloads())
            });
            projection_reached
                .recv_timeout(Duration::from_secs(2))
                .expect("finished owner must enter its terminal projector");
            assert!(client.cancel_download(download_id).await.unwrap());
            release_sender.send(()).unwrap();
            listing.join().unwrap();

            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let failed_closed =
                        client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| {
                                state.status == DownloadStatus::Error
                                    && !state.task_registered
                                    && state.lifecycle_failure_unverified
                                    && state.recovery_destination().is_some()
                            });
                    if failed_closed {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("cancel must inherit the superseded projection's sticky obligation");

            tokio::time::timeout(Duration::from_secs(2), async {
                while client.download_tasks.contains(download_id)
                    || client.download_tasks.outstanding_retired_for_test() != 0
                {
                    let _ = client.list_downloads().await;
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("the replacement finalizer must be observed and settled");
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Error);
            assert!(!state.task_registered);
            assert!(state.lifecycle_failure_unverified);
            assert!(state.recovery_destination().is_some());
            drop(downloads);
            assert_eq!(client.download_tasks.outstanding_retired_for_test(), 0);
            client.download_tasks.set_projection_observer(None);
        }
    }

    #[tokio::test]
    async fn public_cancel_acknowledges_transferred_failure_after_error_projection() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-acknowledges-transferred-projection";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Paused, false),
        );

        let (fallback_reached_sender, fallback_reached) = tokio::sync::oneshot::channel();
        let (_fallback_release_sender, fallback_release) = tokio::sync::oneshot::channel::<()>();
        let prepared = client
            .download_tasks
            .prepare_projection(
                download_id.to_string(),
                |_, _| async {
                    panic!("terminal projection panic sentinel");
                },
                move |_| async move {
                    let _ = fallback_reached_sender.send(());
                    let _ = fallback_release.await;
                    ProjectionOutcome::RolledBack
                },
            )
            .unwrap();
        let ticket = client
            .download_tasks
            .install_projection_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), fallback_reached)
            .await
            .expect("projection panic must enter its owned fallback")
            .unwrap();
        assert!(!ticket.failure_projected_for_test());

        let (error_reached_sender, error_reached) = std::sync::mpsc::channel();
        let error_reached_sender = Arc::new(std::sync::Mutex::new(Some(error_reached_sender)));
        let (release_error_sender, release_error) = std::sync::mpsc::channel();
        let release_error = Arc::new(std::sync::Mutex::new(release_error));
        client
            .download_tasks
            .set_projection_observer(Some(Arc::new({
                let error_reached_sender = error_reached_sender.clone();
                let release_error = release_error.clone();
                move |projection| {
                    if projection == "cancel-finalizer-error" {
                        if let Some(sender) = error_reached_sender.lock().unwrap().take() {
                            sender.send(()).unwrap();
                            release_error.lock().unwrap().recv().unwrap();
                        }
                    }
                }
            })));

        let cancel_client = client.clone();
        let finalizer_ticket = ticket.clone();
        let cancel = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    assert!(cancel_client.cancel_download(download_id).await.unwrap());
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while !finalizer_ticket.settled_for_test() {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .expect("finalizer must settle transferred projection custody");
                });
        });
        tokio::task::spawn_blocking(move || {
            error_reached
                .recv_timeout(Duration::from_secs(2))
                .expect("finalizer must project Error before acknowledging the old cell");
        })
        .await
        .unwrap();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Superseded);
        assert!(!ticket.failure_projected_for_test());
        assert!(!ticket.settled_for_test());
        {
            let states = client.downloads.read().await;
            let state = states.get(download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Error);
            assert!(state.lifecycle_failure_unverified);
            assert!(state.recovery_destination().is_some());
        }

        release_error_sender.send(()).unwrap();
        cancel.join().unwrap();
        assert!(ticket.failure_projected_for_test());
        assert!(ticket.settled_for_test());
        client.download_tasks.set_projection_observer(None);
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.download_tasks.contains(download_id) {
                let _ = client.list_downloads().await;
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("acknowledging finalizer must be observed without residual owner");
        let states = client.downloads.read().await;
        let state = states.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.recovery_destination().is_some());
    }

    #[tokio::test]
    async fn panicked_terminal_projection_projects_once_settles_and_stays_fail_closed() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "panicked-terminal-projector";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        state.error = Some("handled worker outcome".to_string());
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {})
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let projection_calls = Arc::new(AtomicUsize::new(0));
        let projection_calls_in_task = projection_calls.clone();
        client
            .download_tasks
            .set_projection_observer(Some(Arc::new(move |projection| {
                if projection == "finished-task" {
                    projection_calls_in_task.fetch_add(1, Ordering::SeqCst);
                    panic!("terminal projection panic sentinel");
                }
            })));
        let (first, second) = tokio::join!(client.list_downloads(), client.list_downloads());
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        client.download_tasks.set_projection_observer(None);
        assert_eq!(projection_calls.load(Ordering::SeqCst), 1);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.recovery_destination().is_some());
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            while client
                .downloads
                .read()
                .await
                .get(download_id)
                .is_some_and(|state| state.status == DownloadStatus::Cancelling)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.lifecycle_failure_unverified);
        assert!(state.recovery_destination().is_some());
    }

    #[tokio::test]
    async fn cancelled_finished_observer_keeps_owner_until_atomic_state_projection() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "stale-failed-observation";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Queued, true),
        );
        let failed = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                panic!("stale worker failure sentinel")
            })
            .unwrap();
        client.download_tasks.install_gated(failed).unwrap().start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let (projection_sender, projection_reached) = std::sync::mpsc::channel();
        let projection_sender = Arc::new(std::sync::Mutex::new(Some(projection_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_projection_observer(Some(Arc::new({
                let projection_sender = projection_sender.clone();
                let release = release.clone();
                move |projection| {
                    if projection == "finished-task" {
                        if let Some(sender) = projection_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let (abort_sender, abort_receiver) = std::sync::mpsc::channel();
        let (cancelled_sender, cancelled_receiver) = std::sync::mpsc::channel();
        let (terminal_sender, terminal_receiver) = std::sync::mpsc::channel();
        let listing_client = client.clone();
        let listing = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let request_client = listing_client.clone();
                let request = tokio::spawn(async move { request_client.list_downloads().await });
                abort_sender.send(request.abort_handle()).unwrap();
                assert!(request.await.unwrap_err().is_cancelled());
                // The second executor thread acknowledges both public waiter
                // cancellation and invocation retirement while the projector
                // remains blocked on the first thread.
                tokio::time::timeout(Duration::from_secs(2), async {
                    while listing_client.download_tasks.outstanding_retired_for_test() > 0 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                cancelled_sender.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    while !listing_client
                        .download_tasks
                        .snapshot(download_id)
                        .is_some_and(|task| {
                            task.role == TaskRole::TerminalProjection && task.finished
                        })
                    {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                terminal_sender.send(()).unwrap();
            });
        });
        projection_reached
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        abort_receiver.recv().unwrap().abort();
        cancelled_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.role == TaskRole::TerminalProjection && task.started));
        release_sender.send(()).unwrap();
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        listing.join().unwrap();
        assert!(client.download_tasks.contains(download_id));
        client.download_tasks.set_projection_observer(None);
        let _ = client.list_downloads().await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(!state.task_registered);
        assert!(state.recovery_destination().is_some());
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn concurrent_finished_observers_cannot_apply_predecessor_to_successor() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "concurrent-finished-observers";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Queued, true),
        );
        let failed = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                panic!("predecessor failure sentinel")
            })
            .unwrap();
        client.download_tasks.install_gated(failed).unwrap().start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let projection_count = Arc::new(AtomicUsize::new(0));
        let (first_sender, first_reached) = std::sync::mpsc::channel();
        let first_sender = Arc::new(std::sync::Mutex::new(Some(first_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_projection_observer(Some(Arc::new({
                let first_sender = first_sender.clone();
                let release = release.clone();
                let projection_count = projection_count.clone();
                move |projection| {
                    if projection == "finished-task" {
                        projection_count.fetch_add(1, Ordering::SeqCst);
                        let sender = first_sender.lock().unwrap().take();
                        if let Some(sender) = sender {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let first_client = client.clone();
        let first = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(first_client.observe_finished_download_tasks());
        });
        first_reached.recv_timeout(Duration::from_secs(2)).unwrap();

        let second_client = client.clone();
        let second = tokio::spawn(async move { second_client.list_downloads().await });
        tokio::task::yield_now().await;
        assert_eq!(projection_count.load(Ordering::SeqCst), 1);
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.role == TaskRole::TerminalProjection));
        let rejected = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        assert!(client.download_tasks.install_gated(rejected).is_err());
        client.download_tasks.rescue_abandoned();
        release_sender.send(()).unwrap();
        first.join().unwrap();
        second.await.unwrap();
        client.download_tasks.set_projection_observer(None);

        let successor = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        let installed = {
            let mut downloads = client.downloads.write().await;
            let state = downloads.get_mut(download_id).unwrap();
            state.status = DownloadStatus::Queued;
            state.error = None;
            state.task_registered = true;
            client.download_tasks.install_gated(successor).unwrap()
        };
        installed.start();

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.error.is_none());
        assert!(state.task_registered);
        drop(downloads);
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.role == TaskRole::Worker && !task.finished));
        assert!(client.cancel_download(download_id).await.unwrap());
    }

    #[tokio::test]
    async fn public_cancel_without_predecessor_owner_settles_truthfully() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = recovery_test_client(temp.path().join("cache"), &library_root);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-without-predecessor";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, false),
        );

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let terminal = client
                    .downloads
                    .read()
                    .await
                    .get(download_id)
                    .is_some_and(|state| state.status == DownloadStatus::Cancelled);
                if terminal {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let _ = client.list_downloads().await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn ownerless_cancel_cleans_all_bound_artifacts_before_terminal_in_both_modes() {
        for recovery in [false, true] {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
            std::fs::create_dir_all(&library_root).unwrap();
            client
                .configure_download_destination_root(&library_root)
                .unwrap();
            let persistence = Arc::new(DownloadPersistence::new(temp.path()));
            client.set_persistence(persistence.clone());
            let verified = verified_recovery(
                &library_root,
                "acme/model",
                &["one.gguf", "nested/two.gguf"],
            );
            let download_id = if recovery {
                "ownerless-recovery-cleanup"
            } else {
                "ownerless-ambient-cleanup"
            };
            let mut state = recovery_test_state(
                &verified,
                download_id,
                if recovery {
                    DownloadStatus::Paused
                } else {
                    DownloadStatus::Downloading
                },
                false,
            );
            if !recovery {
                state.make_managed_for_test();
            }
            persist_state_fixture(&persistence, &mut state);
            if recovery {
                revoke_state_fixture(&persistence, &mut state);
                state.status = DownloadStatus::Downloading;
            }
            client
                .downloads
                .write()
                .await
                .insert(download_id.to_string(), state);

            std::fs::create_dir_all(verified.destination.display_path().join("nested")).unwrap();
            for filename in ["one.gguf", "nested/two.gguf"] {
                std::fs::write(
                    verified
                        .destination
                        .display_path()
                        .join(format!("{filename}.part")),
                    b"partial",
                )
                .unwrap();
            }
            std::fs::write(
                verified.destination.display_path().join(".pumas_download"),
                b"{}",
            )
            .unwrap();

            let expected = if recovery {
                "remove partial download file"
            } else {
                "remove ambient partial download file"
            };
            let (started_sender, started) = tokio::sync::oneshot::channel();
            let started_sender = Arc::new(std::sync::Mutex::new(Some(started_sender)));
            let (release_sender, release) = std::sync::mpsc::channel();
            let release = Arc::new(std::sync::Mutex::new(release));
            client.download_tasks.set_blocking_observer(Some(Arc::new({
                let started_sender = started_sender.clone();
                let release = release.clone();
                move |operation| {
                    if operation == expected {
                        if let Some(sender) = started_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));

            assert!(client.cancel_download(download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(1), started)
                .await
                .expect("the finalizer must own partial cleanup")
                .unwrap();
            let competing_root =
                crate::model_library::download_recovery::DownloadDestinationRoot::open(
                    &library_root,
                )
                .unwrap();
            let cleanup_excluded_contender = matches!(
                competing_root.try_acquire_execution_grant(),
                Err(PumasError::DownloadRootBusy)
            );
            {
                let downloads = client.downloads.read().await;
                let state = downloads.get(download_id).unwrap();
                assert_eq!(state.status, DownloadStatus::Cancelling);
                assert!(state.task_registered);
                assert_eq!(state.recovery_destination().is_some(), recovery);
            }
            release_sender.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                while client.get_download_status(download_id).await
                    != Some(DownloadStatus::Cancelled)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("terminal cancellation must wait for all cleanup");
            assert!(
                cleanup_excluded_contender,
                "actual partial removal must retain root exclusion"
            );

            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert!(!state.task_registered);
            assert!(state.recovery_destination().is_none());
            drop(downloads);
            for filename in ["one.gguf", "nested/two.gguf"] {
                assert!(!verified
                    .destination
                    .display_path()
                    .join(format!("{filename}.part"))
                    .exists());
            }
            assert!(!verified
                .destination
                .display_path()
                .join(".pumas_download")
                .exists());
            assert!(persistence.load_all().is_empty());
            assert_eq!(persistence.is_revoked(download_id).unwrap(), recovery);
            client.download_tasks.set_blocking_observer(None);
            client.shutdown_downloads().await.unwrap();
            assert!(competing_root.try_acquire_execution_grant().is_ok());
        }
    }

    #[tokio::test]
    async fn dropped_client_and_shutdown_waiter_retain_root_through_actual_cleanup() {
        for explicit_shutdown in [false, true] {
            let temp = TempDir::new().unwrap();
            let (client, destination) = admitted_download_fixture(temp.path()).await;
            let competing_root =
                crate::model_library::download_recovery::DownloadDestinationRoot::open(temp.path())
                    .unwrap();
            let (entered, ready) = tokio::sync::oneshot::channel();
            let entered = std::sync::Mutex::new(Some(entered));
            let (release, held) = std::sync::mpsc::channel();
            let held = std::sync::Mutex::new(held);
            client
                .download_tasks
                .set_blocking_observer(Some(Arc::new(move |operation| {
                    if operation == "remove ambient partial download file" {
                        let entered = entered.lock().unwrap().take();
                        if let Some(entered) = entered {
                            let _ = entered.send(());
                            let _ = held.lock().unwrap().recv();
                        }
                    }
                })));
            assert!(client.cancel_download("admitted-cleanup").await.unwrap());
            tokio::time::timeout(Duration::from_secs(3), ready)
                .await
                .unwrap()
                .unwrap();
            let weak_client = Arc::downgrade(&client);
            let weak_owner = Arc::downgrade(&client.download_tasks);
            if explicit_shutdown {
                let shutdown_client = client.clone();
                let shutdown =
                    tokio::spawn(async move { shutdown_client.shutdown_downloads().await });
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !client.download_tasks.is_closed() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                shutdown.abort();
                assert!(shutdown.await.unwrap_err().is_cancelled());
            }
            drop(client);
            // Only production shutdown/effect observers may retain ownership now.
            let client_released = weak_client.strong_count() == 0;
            let held_payload = std::fs::read(destination.join("weights.gguf.part")).unwrap();
            let excluded = matches!(
                competing_root.try_acquire_execution_grant(),
                Err(PumasError::DownloadRootBusy)
            );
            release.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                while weak_owner.strong_count() != 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("production observers must drain after the actual cleanup closure exits");
            assert!(
                client_released,
                "the test must not keep a strong client alive"
            );
            assert!(
                excluded,
                "client/waiter drop must not unlock a running remove closure"
            );
            assert_eq!(held_payload, b"abc");
            assert!(!destination.join("weights.gguf.part").exists());
            assert!(competing_root.try_acquire_execution_grant().is_ok());
        }
    }

    #[tokio::test]
    async fn ownerless_cancel_cleanup_failure_is_fail_closed_in_both_modes() {
        for recovery in [false, true] {
            let temp = TempDir::new().unwrap();
            let library_root = temp.path().join("library");
            let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
            std::fs::create_dir_all(&library_root).unwrap();
            client
                .configure_download_destination_root(&library_root)
                .unwrap();
            let persistence = Arc::new(DownloadPersistence::new(temp.path()));
            client.set_persistence(persistence.clone());
            let verified = verified_recovery(
                &library_root,
                "acme/model",
                &["blocked.gguf", "cleaned.gguf"],
            );
            let download_id = if recovery {
                "ownerless-recovery-failure"
            } else {
                "ownerless-ambient-failure"
            };
            let mut state = recovery_test_state(
                &verified,
                download_id,
                if recovery {
                    DownloadStatus::Paused
                } else {
                    DownloadStatus::Downloading
                },
                false,
            );
            if !recovery {
                state.make_managed_for_test();
            }
            let original = persisted_recovery_test_state(&state);
            persist_state_fixture(&persistence, &mut state);
            if recovery {
                revoke_state_fixture(&persistence, &mut state);
                state.revoked_snapshot = Some(original.clone());
                state.status = DownloadStatus::Downloading;
            }
            client
                .downloads
                .write()
                .await
                .insert(download_id.to_string(), state);

            std::fs::write(
                verified
                    .destination
                    .display_path()
                    .join("blocked.gguf.part"),
                b"scanner-visible partial",
            )
            .unwrap();
            std::fs::write(
                verified
                    .destination
                    .display_path()
                    .join("cleaned.gguf.part"),
                b"partial",
            )
            .unwrap();
            std::fs::write(
                verified.destination.display_path().join(".pumas_download"),
                b"{}",
            )
            .unwrap();

            let failed_once = Arc::new(AtomicBool::new(false));
            let failed_once_in_observer = failed_once.clone();
            let failure_operation = if recovery {
                "remove partial download file"
            } else {
                "remove ambient partial download file"
            };
            client
                .download_tasks
                .set_blocking_failure_observer(Some(Arc::new(move |operation| {
                    operation == failure_operation
                        && !failed_once_in_observer.swap(true, Ordering::SeqCst)
                })));

            assert!(client.cancel_download(download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let failed =
                        client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| {
                                state.status == DownloadStatus::Error
                                    && state.lifecycle_failure_unverified
                                    && !state.task_registered
                            });
                    if failed {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("cleanup failure must settle as Error");
            let state = client.downloads.read().await;
            assert_eq!(
                state[download_id].recovery_destination().is_some(),
                recovery
            );
            drop(state);
            assert!(verified
                .destination
                .display_path()
                .join("blocked.gguf.part")
                .is_file());
            assert!(!verified
                .destination
                .display_path()
                .join("cleaned.gguf.part")
                .exists());
            assert!(!verified
                .destination
                .display_path()
                .join(".pumas_download")
                .exists());
            if recovery {
                assert!(persistence.load_all().is_empty());
                assert!(persistence.is_revoked(download_id).unwrap());
            } else {
                assert!(persistence.load_all().is_empty());
                assert!(!persistence.is_revoked(download_id).unwrap());
            }
            let inventory = persistence.load_lifecycle_inventory_strict().unwrap();
            let quarantine = &inventory.quarantines[download_id];
            assert_eq!(
                quarantine.domain,
                if recovery {
                    LifecycleQuarantineDomain::Recovery
                } else {
                    LifecycleQuarantineDomain::Ambient
                }
            );
            assert_eq!(quarantine.disposition, LifecycleCleanupDisposition::Pending);
            assert!(quarantine.sticky_failure);
            let mut expected = serde_json::to_value(&original).unwrap();
            expected["status"] = serde_json::to_value(DownloadStatus::Error).unwrap();
            assert_eq!(
                serde_json::to_value(&quarantine.snapshot).unwrap(),
                expected
            );
            let mut reopened = HuggingFaceClient::new(temp.path().join("reopened")).unwrap();
            reopened
                .configure_download_destination_root(&library_root)
                .unwrap();
            reopened.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
            assert!(reopened.restore_persisted_downloads().await.is_err());
            client.download_tasks.set_blocking_failure_observer(None);

            assert_eq!(
                client
                    .destination_executions
                    .claim_count(&verified.destination.identity()),
                1,
                "failed cleanup must park destination authority"
            );
            assert!(client.cancel_download(download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let settled =
                        client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| {
                                state.status == DownloadStatus::Error
                                    && state.lifecycle_failure_unverified
                                    && !state.task_registered
                            });
                    if settled
                        && client
                            .destination_executions
                            .claim_count(&verified.destination.identity())
                            == 0
                    {
                        break;
                    }
                    let _ = client.get_download_status(download_id).await;
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("successful retry must settle sticky Error and release authority");

            let state = client.downloads.read().await;
            assert_eq!(state[download_id].status, DownloadStatus::Error);
            assert!(state[download_id].lifecycle_failure_unverified);
            assert_eq!(
                state[download_id].recovery_destination().is_some(),
                recovery
            );
            drop(state);
            assert!(!verified
                .destination
                .display_path()
                .join("blocked.gguf.part")
                .exists());
            assert!(persistence.load_all().is_empty());
            assert_eq!(persistence.is_revoked(download_id).unwrap(), recovery);
            reopened.restore_persisted_downloads().await.unwrap();
            if recovery {
                assert!(reopened.list_downloads().await.is_empty());
            } else {
                assert_eq!(
                    reopened.get_download_status(download_id).await,
                    Some(DownloadStatus::Error)
                );
                assert!(!reopened.resume_download(download_id).await.unwrap());
                assert!(reopened.cancel_download(download_id).await.is_err());
            }
        }
    }

    #[tokio::test]
    async fn replaced_worker_cannot_publish_cancelled_before_its_finalizer() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-ready-worker";
        let state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (check_sender, check_reached) = std::sync::mpsc::channel();
        let check_sender = Arc::new(std::sync::Mutex::new(Some(check_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_cancellation_check_observer(Some(Arc::new({
                let check_sender = check_sender.clone();
                let release = release.clone();
                move || {
                    if let Some(sender) = check_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            })));
        let (state_sender, state_receiver) = tokio::sync::oneshot::channel();
        let (settled_sender, settled_receiver) = tokio::sync::oneshot::channel();
        let cancel_client = client.clone();
        let cancel_thread = std::thread::spawn(move || {
            check_reached.recv().unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                assert!(cancel_client.cancel_download(download_id).await.unwrap());
                let state_during_cancel = {
                    let downloads = cancel_client.downloads.read().await;
                    let state = downloads.get(download_id).unwrap();
                    (
                        state.status,
                        state.recovery_destination().is_some(),
                        state.task_registered,
                    )
                };
                let _ = state_sender.send(state_during_cancel);
                release_sender.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let settled = cancel_client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| state.status != DownloadStatus::Cancelling);
                        if settled {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let _ = settled_sender.send(());
            });
        });
        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    vec![FileToDownload {
                        filename: "weights.gguf".to_string(),
                        size: Some(4),
                        sha256: None,
                    }],
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        let state_during_cancel = state_receiver.await.unwrap();
        cancel_thread.join().unwrap();
        assert_eq!(
            state_during_cancel,
            (DownloadStatus::Cancelling, true, true)
        );
        settled_receiver.await.unwrap();
        client.download_tasks.set_cancellation_check_observer(None);
        let _ = client.list_downloads().await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn replaced_worker_cannot_publish_paused_after_its_finalizer() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancel-ready-pause-worker";
        let state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (projection_sender, projection_reached) = std::sync::mpsc::channel();
        let projection_sender = Arc::new(std::sync::Mutex::new(Some(projection_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_worker_projection_observer(Some(Arc::new({
                let projection_sender = projection_sender.clone();
                let release = release.clone();
                let pause_flag = pause_flag.clone();
                move |projection| {
                    if projection == "worker-entry" {
                        pause_flag.store(true, Ordering::Relaxed);
                    }
                    if projection == "pause-before-destination" {
                        if let Some(sender) = projection_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let (state_sender, state_receiver) = tokio::sync::oneshot::channel();
        let (settled_sender, settled_receiver) = tokio::sync::oneshot::channel();
        let cancel_client = client.clone();
        let cancel_thread = std::thread::spawn(move || {
            projection_reached.recv().unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                assert!(cancel_client.cancel_download(download_id).await.unwrap());
                let state_during_cancel = {
                    let downloads = cancel_client.downloads.read().await;
                    let state = downloads.get(download_id).unwrap();
                    (
                        state.status,
                        state.recovery_destination().is_some(),
                        state.task_registered,
                    )
                };
                let _ = state_sender.send(state_during_cancel);
                release_sender.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let settled = cancel_client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| state.status != DownloadStatus::Cancelling);
                        if settled {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let _ = settled_sender.send(());
            });
        });
        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    vec![FileToDownload {
                        filename: "weights.gguf".to_string(),
                        size: Some(4),
                        sha256: None,
                    }],
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        assert_eq!(
            state_receiver.await.unwrap(),
            (DownloadStatus::Cancelling, true, true)
        );
        settled_receiver.await.unwrap();
        cancel_thread.join().unwrap();
        client.download_tasks.set_worker_projection_observer(None);
        let _ = client.list_downloads().await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn semantic_recovery_failure_survives_cancel_before_receiver_consumes_result() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["nested/weights.gguf"]);
        let download_id = "cancel-semantic-failure";
        let state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("nested"),
            b"not a dir",
        )
        .unwrap();

        let (result_sender, result_ready) = std::sync::mpsc::channel();
        let result_sender = Arc::new(std::sync::Mutex::new(Some(result_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_blocking_result_observer(Some(Arc::new({
                let result_sender = result_sender.clone();
                let release = release.clone();
                move |operation| {
                    if operation == "create file parent" {
                        if let Some(sender) = result_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));

        let (state_sender, state_receiver) = tokio::sync::oneshot::channel();
        let (settled_sender, settled_receiver) = tokio::sync::oneshot::channel();
        let cancel_client = client.clone();
        let cancel_thread = std::thread::spawn(move || {
            result_ready.recv().unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                assert!(cancel_client.cancel_download(download_id).await.unwrap());
                let state_during_cancel = {
                    let downloads = cancel_client.downloads.read().await;
                    let state = downloads.get(download_id).unwrap();
                    (
                        state.status,
                        state.recovery_destination().is_some(),
                        state.task_registered,
                    )
                };
                let _ = state_sender.send(state_during_cancel);
                release_sender.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let settled = cancel_client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| state.status != DownloadStatus::Cancelling);
                        if settled {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let _ = settled_sender.send(());
            });
        });

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    vec![FileToDownload {
                        filename: "nested/weights.gguf".to_string(),
                        size: Some(4),
                        sha256: None,
                    }],
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        assert_eq!(
            state_receiver.await.unwrap(),
            (DownloadStatus::Cancelling, true, true)
        );
        settled_receiver.await.unwrap();
        cancel_thread.join().unwrap();
        client.download_tasks.set_blocking_result_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn retry_and_pause_projections_require_current_worker_and_expected_status() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(HuggingFaceClient::new(temp.path().join("cache")).unwrap());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "worker-projection-generation";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (context_sender, context_receiver) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context_sender.send(context);
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        let stale_worker = context_receiver.await.unwrap();

        {
            let mut downloads = client.downloads.write().await;
            assert!(matches!(
                current_worker_state(
                    &mut downloads,
                    download_id,
                    &stale_worker,
                    &[DownloadStatus::Queued],
                ),
                Err(PumasError::DownloadCancelled)
            ));
        }
        let transition = client
            .download_tasks
            .begin_cancel(download_id, |_, _| async {
                std::future::pending::<()>().await;
            })
            .unwrap();
        let super::super::lifecycle::CancelTransition::Started(finalizer) = transition else {
            panic!("the worker should be replaced by a finalizer");
        };
        finalizer.start();
        let mut downloads = client.downloads.write().await;
        assert!(matches!(
            current_worker_state(
                &mut downloads,
                download_id,
                &stale_worker,
                &[DownloadStatus::Downloading],
            ),
            Err(PumasError::DownloadCancelled)
        ));
    }

    #[tokio::test]
    async fn retry_reset_projection_cannot_overwrite_replacement_finalizer() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "retry-reset-replacement";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );
        let (context_sender, context_receiver) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(
                download_id.to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context_sender.send(context.clone());
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        let worker_context = context_receiver.await.unwrap();

        let (projection_sender, projection_reached) = std::sync::mpsc::channel();
        let projection_sender = Arc::new(std::sync::Mutex::new(Some(projection_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_worker_projection_observer(Some(Arc::new({
                let projection_sender = projection_sender.clone();
                let release = release.clone();
                move |projection| {
                    if projection == "retry-reset" {
                        if let Some(sender) = projection_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let (state_sender, state_receiver) = tokio::sync::oneshot::channel();
        let (settled_sender, settled_receiver) = tokio::sync::oneshot::channel();
        let cancel_client = client.clone();
        let cancel_thread = std::thread::spawn(move || {
            projection_reached.recv().unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                assert!(cancel_client.cancel_download(download_id).await.unwrap());
                let state_during_cancel = {
                    let downloads = cancel_client.downloads.read().await;
                    let state = downloads.get(download_id).unwrap();
                    (
                        state.status,
                        state.recovery_destination().is_some(),
                        state.task_registered,
                    )
                };
                let _ = state_sender.send(state_during_cancel);
                release_sender.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let settled = cancel_client
                            .downloads
                            .read()
                            .await
                            .get(download_id)
                            .is_some_and(|state| state.status != DownloadStatus::Cancelling);
                        if settled {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let _ = settled_sender.send(());
            });
        });

        assert!(matches!(
            project_worker_retry_reset(
                &client.downloads,
                download_id,
                &worker_context,
                2,
                Some(4),
            )
            .await,
            Err(PumasError::DownloadCancelled)
        ));
        assert_eq!(
            state_receiver.await.unwrap(),
            (DownloadStatus::Cancelling, true, true)
        );
        settled_receiver.await.unwrap();
        cancel_thread.join().unwrap();
        client.download_tasks.set_worker_projection_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Cancelled);
        assert!(state.recovery_destination().is_none());
        assert!(!state.task_registered);
    }

    #[tokio::test]
    async fn recovery_pause_during_blocking_preflight_retains_owner_and_capability() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "pause-blocking-preflight";
        let state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let (started_sender, started) = tokio::sync::oneshot::channel();
        let started_sender = Arc::new(std::sync::Mutex::new(Some(started_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client.download_tasks.set_blocking_observer(Some(Arc::new({
            let started_sender = started_sender.clone();
            let release = release.clone();
            move |label| {
                if label == "preflight" {
                    if let Some(sender) = started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                    let _ = release.lock().unwrap().recv();
                }
            }
        })));

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    vec![FileToDownload {
                        filename: "weights.gguf".to_string(),
                        size: Some(4),
                        sha256: None,
                    }],
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        started.await.unwrap();

        assert!(client.pause_download(download_id).await.unwrap());
        {
            let downloads = client.downloads.read().await;
            let state = downloads.get(download_id).unwrap();
            assert_eq!(state.status, DownloadStatus::Pausing);
            assert!(state.recovery_destination().is_some());
            assert!(state.task_registered);
        }
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| !task.finished));

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let downloads = client.downloads.read().await;
                let state = downloads.get(download_id).unwrap();
                if state.status == DownloadStatus::Paused && !state.task_registered {
                    assert!(state.recovery_destination().is_some());
                    break;
                }
                drop(downloads);
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("pause should settle only after the owned preflight operation returns");
        client.observe_finished_download_tasks().await;
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn cancelling_recovery_resume_before_commit_preserves_paused_state() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "cancelled-resume";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Paused, false),
        );
        let auth_guard = client.auth_token.write().await;
        let resume = {
            let client = client.clone();
            tokio::spawn(async move { client.resume_download(download_id).await })
        };

        let _ = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if client.get_download_status(download_id).await == Some(DownloadStatus::Queued) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        resume.abort();
        let _ = resume.await;
        drop(auth_guard);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Paused);
        assert!(state.recovery_destination().is_some());
        assert!(!state.task_registered);
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));
    }

    #[tokio::test]
    async fn cancelling_recovery_resume_after_commit_keeps_registered_owner() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "post-commit-resume";
        client.downloads.write().await.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Paused, false),
        );
        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let destination_guard = destination_lock.lock().await;
        let resume = {
            let client = client.clone();
            tokio::spawn(async move { client.resume_download(download_id).await })
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let downloads = client.downloads.read().await;
                let state = downloads.get(download_id).unwrap();
                if state.status == DownloadStatus::Queued && state.task_registered {
                    break;
                }
                drop(downloads);
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("recovery resume should atomically register its worker");
        resume.abort();
        let _ = resume.await;

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Queued);
        assert!(state.recovery_destination().is_some());
        assert!(state.task_registered);
        drop(downloads);
        assert!(client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| !task.finished));

        assert!(client.cancel_download(download_id).await.unwrap());
        drop(destination_guard);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if client.get_download_status(download_id).await == Some(DownloadStatus::Cancelled)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancel finalizer should settle the resumed recovery");
    }

    #[test]
    fn persisted_download_restore_never_recreates_recovery_authority() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let state = recovery_test_state(&verified, "persisted", DownloadStatus::Paused, false);
        let restored = DownloadState::from_persisted(
            &persisted_recovery_test_state(&state),
            2,
            DownloadDestination::Managed(verified.destination.clone()),
        );

        assert!(restored.recovery_destination().is_none());
        assert_eq!(restored.status, DownloadStatus::Paused);
    }

    #[tokio::test]
    async fn shutdown_closes_mutations_and_reads_without_reconciling_held_work() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(HuggingFaceClient::new(temp.path().join("cache")).unwrap());
        let verified = verified_recovery(
            &temp.path().join("library"),
            "acme/model",
            &["weights.gguf"],
        );
        for (id, status) in [
            ("active", DownloadStatus::Downloading),
            ("queued", DownloadStatus::Queued),
            ("pausing", DownloadStatus::Pausing),
            ("cancelling", DownloadStatus::Cancelling),
            ("paused", DownloadStatus::Paused),
            ("completed", DownloadStatus::Completed),
            ("cancelled", DownloadStatus::Cancelled),
            ("error", DownloadStatus::Error),
        ] {
            client
                .downloads
                .write()
                .await
                .insert(id.into(), recovery_test_state(&verified, id, status, true));
        }
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let path = temp.path().join("owned-preparation");
        let operation = tokio::spawn({
            let client = client.clone();
            let path = path.clone();
            async move {
                client
                    .run_download_invocation(move |context| async move {
                        context
                            .run_fallible_blocking_named("held shutdown preparation", move || {
                                let _ = entered.send(());
                                held.recv().unwrap();
                                std::fs::write(path, b"owned until observed")?;
                                Ok::<_, PumasError>(())
                            })
                            .await
                            .map_err(|error| PumasError::Other(error.to_string()))?
                    })
                    .await
            }
        });
        started.await.unwrap();
        let shutdown = tokio::spawn({
            let client = client.clone();
            async move { client.shutdown_downloads().await }
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !client.download_tasks.is_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let progress = client.get_download_progress("active").await.unwrap();
        let listed = client.list_downloads().await;
        let snapshot = client.download_snapshot().await;
        let pending = !shutdown.is_finished();
        let request = recovery_test_request(&verified.repo_id, &verified.files);
        let closed_results = [
            client.pause_download("active").await.map(|_| ()),
            client.resume_download("paused").await.map(|_| ()),
            client.cancel_download("active").await.map(|_| ()),
            client
                .start_download(&request, temp.path(), None)
                .await
                .map(|_| ()),
            client.restore_persisted_downloads().await.map(|_| ()),
            client
                .admit_recovery_download(&verified, None)
                .await
                .map(|_| ()),
        ];
        // Always release actual blocking work before assertions can fail.
        release.send(()).unwrap();
        let _ = operation.await.unwrap();
        shutdown.await.unwrap().unwrap();
        assert!(pending);
        assert_eq!(progress.status, DownloadStatus::Downloading);
        assert_eq!(
            listed
                .iter()
                .find(|row| row.download_id == "active")
                .unwrap()
                .status,
            DownloadStatus::Downloading
        );
        assert_eq!(
            snapshot
                .downloads
                .iter()
                .find(|row| row.download_id == "active")
                .unwrap()
                .status,
            DownloadStatus::Downloading
        );
        assert!(closed_results
            .into_iter()
            .all(|result| matches!(result, Err(PumasError::DownloadLifecycleClosed))));
        assert_eq!(std::fs::read(path).unwrap(), b"owned until observed");
        for id in ["active", "queued", "pausing", "cancelling"] {
            let row = client.get_download_progress(id).await.unwrap();
            assert_eq!(row.status, DownloadStatus::Error);
            assert_eq!(row.error.as_deref(), Some(DOWNLOAD_SHUTDOWN_INTERRUPTED));
            assert_eq!(row.downloaded_bytes, Some(2));
        }
        for (id, expected) in [
            ("paused", DownloadStatus::Paused),
            ("completed", DownloadStatus::Completed),
            ("cancelled", DownloadStatus::Cancelled),
            ("error", DownloadStatus::Error),
        ] {
            assert_eq!(client.get_download_status(id).await, Some(expected));
        }
        client.shutdown_downloads().await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_shutdown_waiter_retains_failure_for_later_waiters() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(HuggingFaceClient::new(temp.path()).unwrap());
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let operation = tokio::spawn({
            let client = client.clone();
            async move {
                client
                    .run_download_invocation(move |context| async move {
                        context
                            .run_fallible_blocking_named(
                                "failed held shutdown preparation",
                                move || {
                                    let _ = entered.send(());
                                    held.recv().unwrap();
                                    Err::<(), _>(PumasError::Other(
                                        "held preparation failed".into(),
                                    ))
                                },
                            )
                            .await
                            .map_err(|error| PumasError::Other(error.to_string()))?
                    })
                    .await
            }
        });
        started.await.unwrap();
        let first_waiter = tokio::spawn({
            let client = client.clone();
            async move { client.shutdown_downloads().await }
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !client.download_tasks.is_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        first_waiter.abort();
        assert!(first_waiter.await.unwrap_err().is_cancelled());
        let second_waiter = tokio::spawn({
            let client = client.clone();
            async move { client.shutdown_downloads().await }
        });
        let pending = !second_waiter.is_finished();
        release.send(()).unwrap();
        let _ = operation.await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(2), second_waiter)
            .await
            .unwrap()
            .unwrap();
        let repeated = client.shutdown_downloads().await;
        assert!(pending);
        assert!(
            matches!(result, Err(PumasError::DownloadShutdownFailed { failures }) if failures > 0)
        );
        assert_eq!(format!("{result:?}"), format!("{repeated:?}"));
    }

    #[tokio::test]
    async fn client_drop_releases_state_and_unstarted_task_recovery_authority() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let baseline = verified.destination.authority_strong_count();
        let download_id = "drop-recovery";
        let lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let lock_guard = lock.lock().await;
        let state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    vec![FileToDownload {
                        filename: "weights.gguf".to_string(),
                        size: Some(4),
                        sha256: None,
                    }],
                    DownloadDestination::Recovery(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        assert!(verified.destination.authority_strong_count() > baseline);

        drop(client);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if verified.destination.authority_strong_count() == baseline {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("aborted unstarted task and state should release their authority");
        drop(lock_guard);
        assert!(!verified
            .destination
            .display_path()
            .join("weights.gguf.part")
            .exists());
    }

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
    async fn recovery_context_requires_one_exact_repo_destination_and_file_set() {
        let temp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(temp.path()).unwrap();
        let destination = temp.path().join("llm/acme/model");
        let root =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(temp.path())
                .unwrap();
        let held = root.resolve(&destination).unwrap();
        let request = DownloadRequest {
            repo_id: "acme/model".to_string(),
            family: "acme".to_string(),
            official_name: "Model".to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: Some(vec![
                "weights-2.gguf".to_string(),
                "weights-1.gguf".to_string(),
            ]),
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        client.downloads.write().await.insert(
            "download-1".to_string(),
            DownloadState {
                download_id: "download-1".to_string(),
                repo_id: "acme/model".to_string(),
                status: DownloadStatus::Paused,
                progress: 0.5,
                downloaded_bytes: 1,
                total_bytes: Some(2),
                speed: 0.0,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                pause_flag: Arc::new(AtomicBool::new(false)),
                error: None,
                retry_attempt: 0,
                retry_limit: None,
                retrying: false,
                next_retry_delay_seconds: None,
                task_registered: false,
                lifecycle_failure_unverified: false,
                dest_dir: destination.clone(),
                ambient_authority_blocked: false,
                admission: None,
                revoked_snapshot: None,
                destination: Some(DownloadDestination::Managed(held.clone())),
                filename: "weights-1.gguf".to_string(),
                files: vec![FileToDownload {
                    filename: "weights-1.gguf".to_string(),
                    size: Some(2),
                    sha256: None,
                }],
                files_completed: 0,
                download_request: Some(request),
                known_sha256: None,
                huggingface_evidence: None,
            },
        );

        let downloads = client.downloads.read().await;
        assert!(matches!(
            recovery_context(
                &downloads,
                &held.identity(),
                "acme/model",
                &["weights-1.gguf".to_string(), "weights-2.gguf".to_string()],
            ),
            RecoveryContext::Exact { download_id } if download_id == "download-1"
        ));
        for (repo_id, files) in [
            (
                "acme/other",
                vec!["weights-1.gguf".to_string(), "weights-2.gguf".to_string()],
            ),
            ("acme/model", vec!["weights-1.gguf".to_string()]),
        ] {
            assert!(matches!(
                recovery_context(&downloads, &held.identity(), repo_id, &files),
                RecoveryContext::Mismatch
            ));
        }
        assert!(matches!(
            recovery_context(
                &downloads,
                &root.resolve(&temp.path().join("other")).unwrap().identity(),
                "acme/model",
                &["weights-1.gguf".to_string(), "weights-2.gguf".to_string()],
            ),
            RecoveryContext::Missing
        ));
        drop(downloads);

        let unrelated_request = DownloadRequest {
            repo_id: "other/model".to_string(),
            family: "other".to_string(),
            official_name: "Model".to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: Some(vec!["other.gguf".to_string()]),
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        client.downloads.write().await.insert(
            "unrelated".to_string(),
            DownloadState {
                download_id: "unrelated".to_string(),
                repo_id: "other/model".to_string(),
                status: DownloadStatus::Downloading,
                progress: 0.0,
                downloaded_bytes: 0,
                total_bytes: Some(1),
                speed: 0.0,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                pause_flag: Arc::new(AtomicBool::new(false)),
                error: None,
                retry_attempt: 0,
                retry_limit: None,
                retrying: false,
                next_retry_delay_seconds: None,
                task_registered: false,
                lifecycle_failure_unverified: false,
                dest_dir: destination.clone(),
                ambient_authority_blocked: false,
                admission: None,
                revoked_snapshot: None,
                destination: Some(DownloadDestination::Managed(held.clone())),
                filename: "other.gguf".to_string(),
                files: vec![FileToDownload {
                    filename: "other.gguf".to_string(),
                    size: Some(1),
                    sha256: None,
                }],
                files_completed: 0,
                download_request: Some(unrelated_request),
                known_sha256: None,
                huggingface_evidence: None,
            },
        );
        assert!(matches!(
            recovery_context(
                &*client.downloads.read().await,
                &held.identity(),
                "acme/model",
                &["weights-1.gguf".to_string(), "weights-2.gguf".to_string()],
            ),
            RecoveryContext::Mismatch
        ));
    }

    async fn admitted_download_fixture(root: &Path) -> (Arc<HuggingFaceClient>, PathBuf) {
        let mut client = HuggingFaceClient::new(root.join("cache")).unwrap();
        client.configure_download_destination_root(root).unwrap();
        let persistence = Arc::new(DownloadPersistence::new(root));
        let source = root.join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("weights.gguf.part"), b"abc").unwrap();
        let request = recovery_test_request("owner/model", &["weights.gguf".into()]);
        admit_snapshot_at_root(
            &persistence,
            &PersistedDownload {
                download_id: "admitted-cleanup".into(),
                repo_id: request.repo_id.clone(),
                filename: "weights.gguf".into(),
                filenames: vec!["weights.gguf".into()],
                dest_dir: source.clone(),
                total_bytes: Some(8),
                status: DownloadStatus::Paused,
                download_request: request,
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
                huggingface_evidence: None,
            },
            root,
        );
        client.set_persistence(persistence);
        client.restore_persisted_downloads().await.unwrap();
        (Arc::new(client), source)
    }

    #[tokio::test]
    async fn paused_head_and_queued_follower_release_root_and_refuse_stale_resume() {
        let temp = TempDir::new().unwrap();
        let (client, source) = admitted_download_fixture(temp.path()).await;
        let request = recovery_test_request("owner/follower", &["next.gguf".into()]);
        cache_repo_tree(
            &client,
            &request.repo_id,
            vec![LfsFileInfo {
                filename: "next.gguf".into(),
                size: 4,
                sha256: "a".repeat(64),
            }],
            Vec::new(),
        );
        let follower = client
            .start_download(&request, &source, None)
            .await
            .unwrap();
        let root =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(temp.path())
                .unwrap();
        let grant = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match root.try_acquire_execution_grant() {
                    Ok(grant) => break grant,
                    Err(PumasError::DownloadRootBusy) => tokio::task::yield_now().await,
                    Err(error) => panic!("independent root acquisition failed: {error}"),
                }
            }
        })
        .await
        .expect("paused head with queued follower must permit idle handoff");
        assert_eq!(
            client.get_download_status(&follower).await,
            Some(DownloadStatus::Queued)
        );
        let before_wake = std::fs::read(temp.path().join("downloads.json")).unwrap();
        {
            // Schedule an already accepted pause notification after idle handoff.
            // This uses the worker's existing wake seam, not a filesystem authority.
            let mut states = client.downloads.write().await;
            let state = states.get_mut(&follower).unwrap();
            state.pause_flag.store(true, Ordering::Release);
            state.status = DownloadStatus::Pausing;
        }
        client
            .download_tasks
            .active_worker_generation(&follower)
            .unwrap()
            .wake_pause();
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status(&follower).await != Some(DownloadStatus::Error) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("busy idle worker must publish its runtime-only refusal");
        assert_eq!(
            client.downloads.read().await[&follower].error.as_deref(),
            Some("Download library root is busy")
        );
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            before_wake
        );
        let competing_store = DownloadPersistence::new(temp.path());
        competing_store
            .reconcile_lifecycle_inventory_strict()
            .unwrap();
        let inventory = competing_store.load_lifecycle_inventory_strict().unwrap();
        let admission = &inventory.queue_admissions["admitted-cleanup"];
        let snapshot = inventory
            .downloads
            .iter()
            .find(|entry| entry.download_id == "admitted-cleanup")
            .unwrap();
        competing_store
            .revoke_admitted_for_recovery("admitted-cleanup", &admission.attempt_id, snapshot)
            .unwrap();
        let persisted = std::fs::read(temp.path().join("downloads.json")).unwrap();
        drop(grant);
        assert!(!client.resume_download("admitted-cleanup").await.unwrap());
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            persisted
        );
        assert_eq!(
            std::fs::read(source.join("weights.gguf.part")).unwrap(),
            b"abc"
        );
        assert!(!source.join("next.gguf.part").exists());
        assert!(!source.join(".pumas_download").exists());
        client.shutdown_downloads().await.unwrap();
        assert!(root.try_acquire_execution_grant().is_ok());
    }

    #[tokio::test]
    async fn cancellation_retry_without_retained_snapshot_refuses_filesystem_cleanup() {
        let temp = TempDir::new().unwrap();
        let (client, source) = admitted_download_fixture(temp.path()).await;
        let download_id = "admitted-cleanup";
        let store = client.persistence.as_ref().unwrap();
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        let admission = &inventory.queue_admissions[download_id];
        let completing_store = DownloadPersistence::new(temp.path());
        completing_store
            .begin_lifecycle_quarantine(
                &inventory.downloads[0],
                LifecycleQuarantineDomain::Ambient,
                false,
                Some(&admission.attempt_id),
            )
            .unwrap();
        std::fs::remove_file(source.join("weights.gguf.part")).unwrap();
        std::fs::File::open(&source).unwrap().sync_all().unwrap();
        assert!(completing_store
            .settle_queue_admission(download_id, &admission.attempt_id)
            .unwrap());
        assert!(store
            .load_lifecycle_inventory_strict()
            .unwrap()
            .quarantines
            .is_empty());
        let persisted_before = std::fs::read(temp.path().join("downloads.json")).unwrap();
        let persistence = CancellationPersistence {
            store: store.clone(),
            download_id: download_id.into(),
            domain: LifecycleQuarantineDomain::Ambient,
            admission_attempt: Some(admission.attempt_id.clone()),
            revoked_snapshot: None,
        };
        assert!(
            matches!(persistence.begin(true), Err(PumasError::Validation { field, message })
            if field == "download_cleanup" && message == "Admitted cancellation requires a retained cleanup snapshot")
        );
        {
            let mut states = client.downloads.write().await;
            let state = states.get_mut(download_id).unwrap();
            state.status = DownloadStatus::Error;
            state.lifecycle_failure_unverified = true;
        }
        let part = source.join("weights.gguf.part");
        let marker = source.join(".pumas_download");
        std::fs::write(&part, b"successor partial bytes").unwrap();
        std::fs::write(&marker, b"successor marker").unwrap();
        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(3), async {
            while !client.download_tasks.is_empty() {
                client.observe_finished_download_tasks().await;
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refused cleanup must drain its finalizer");
        assert_eq!(std::fs::read(part).unwrap(), b"successor partial bytes");
        assert_eq!(std::fs::read(marker).unwrap(), b"successor marker");
        assert_eq!(
            client.get_download_status(download_id).await,
            Some(DownloadStatus::Error)
        );
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            persisted_before
        );
    }

    #[tokio::test]
    async fn cancellation_retry_preserves_successor_files_after_unconfirmed_verified_cleanup() {
        for released in [false, true] {
            let temp = TempDir::new().unwrap();
            let (client, source) = admitted_download_fixture(temp.path()).await;
            let download_id = "admitted-cleanup";
            let client_store = client.persistence.as_ref().unwrap();
            let before = client_store.load_lifecycle_inventory_strict().unwrap();
            let snapshot = before.downloads.first().unwrap();
            let admission = &before.queue_admissions[download_id];
            // A separate store owner confirms cleanup; the old runtime owner
            // has not confirmed that publication and retains its Error state.
            let completing_store = DownloadPersistence::new(temp.path());
            completing_store
                .begin_lifecycle_quarantine(
                    snapshot,
                    LifecycleQuarantineDomain::Ambient,
                    true,
                    Some(&admission.attempt_id),
                )
                .unwrap();
            std::fs::remove_file(source.join("weights.gguf.part")).unwrap();
            std::fs::File::open(&source).unwrap().sync_all().unwrap();
            assert!(completing_store
                .verify_lifecycle_quarantine(download_id)
                .unwrap());
            if released {
                assert!(completing_store
                    .settle_queue_admission(download_id, &admission.attempt_id)
                    .unwrap());
            }
            assert_eq!(
                client_store
                    .load_lifecycle_inventory_strict()
                    .unwrap()
                    .quarantines[download_id]
                    .disposition,
                LifecycleCleanupDisposition::Pending
            );
            {
                let mut states = client.downloads.write().await;
                let state = states.get_mut(download_id).unwrap();
                state.status = DownloadStatus::Error;
                state.lifecycle_failure_unverified = true;
            }
            // These bytes represent work after the recorded cleanup, not
            // permission to repeat deletion from a stale Pending projection.
            let part = source.join("weights.gguf.part");
            let marker = source.join(".pumas_download");
            std::fs::write(&part, b"successor partial bytes").unwrap();
            std::fs::write(&marker, b"successor marker").unwrap();
            let deletions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            client.download_tasks.set_blocking_observer(Some(Arc::new({
                let deletions = deletions.clone();
                move |operation| {
                    if matches!(
                        operation,
                        "remove ambient partial download file" | "remove ambient download marker"
                    ) {
                        deletions.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })));
            assert!(client.cancel_download(download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(3), async {
                while !client.download_tasks.is_empty() {
                    client.observe_finished_download_tasks().await;
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("terminal retry must drain its owned finalizer");
            client.download_tasks.set_blocking_observer(None);

            assert_eq!(deletions.load(Ordering::SeqCst), 0, "released={released}");
            assert_eq!(std::fs::read(part).unwrap(), b"successor partial bytes");
            assert_eq!(std::fs::read(marker).unwrap(), b"successor marker");
            assert_eq!(
                client.get_download_status(download_id).await,
                Some(DownloadStatus::Error)
            );
            let after = client_store.load_lifecycle_inventory_strict().unwrap();
            assert!(!after.queue_admissions.contains_key(download_id));
            assert_eq!(
                after.quarantines[download_id].disposition,
                LifecycleCleanupDisposition::Verified
            );
            assert!(after.quarantines[download_id].sticky_failure);
            let persisted: serde_json::Value =
                serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                    .unwrap();
            assert_eq!(
                persisted["released_queue_admissions"][download_id],
                serde_json::to_value(admission).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn restore_settles_verified_ambient_cleanup_before_restoring_follower() {
        let temp = TempDir::new().unwrap();
        let (head, follower) = ambient_cleanup_cutpoint(temp.path(), true);
        let store = Arc::new(DownloadPersistence::new(temp.path()));
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(store.clone());

        assert!(client
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            client.get_download_status(&head.download_id).await,
            Some(DownloadStatus::Error)
        );
        assert_eq!(
            client.get_download_status(&follower.download_id).await,
            Some(DownloadStatus::Paused)
        );
        assert!(!client.resume_download(&head.download_id).await.unwrap());
        assert!(client.cancel_download(&head.download_id).await.is_err());
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        assert!(!inventory.queue_admissions.contains_key(&head.download_id));
        let admission = &inventory.queue_admissions[&follower.download_id];
        assert_eq!(admission.position.ordinal, 1);
        assert_eq!(
            admission.position.predecessor.as_ref().unwrap().download_id,
            head.download_id
        );
        let quarantine = &inventory.quarantines[&head.download_id];
        assert_eq!(
            quarantine.disposition,
            LifecycleCleanupDisposition::Verified
        );
        assert!(quarantine.sticky_failure);
        let mut expected = serde_json::to_value(&head).unwrap();
        expected["status"] = serde_json::to_value(DownloadStatus::Error).unwrap();
        assert_eq!(
            serde_json::to_value(&quarantine.snapshot).unwrap(),
            expected
        );

        // Fixture bytes prove queue progress without a network transfer claim.
        std::fs::write(follower.dest_dir.join(&follower.filename), b"complete").unwrap();
        assert!(client.resume_download(&follower.download_id).await.unwrap());
        let settled = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                client.observe_finished_download_tasks().await;
                if client.get_download_status(&follower.download_id).await
                    == Some(DownloadStatus::Completed)
                    && !client.download_tasks.contains(&follower.download_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            settled.is_ok(),
            "verified predecessor cleanup must not strand its follower: {:?}",
            client.get_download_progress(&follower.download_id).await
        );
        let mut fresh = HuggingFaceClient::new(temp.path().join("fresh-cache")).unwrap();
        fresh
            .configure_download_destination_root(temp.path())
            .unwrap();
        fresh.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        assert!(fresh
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        let history = fresh.list_downloads().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].download_id, head.download_id);
        assert_eq!(history[0].status, DownloadStatus::Error);
    }

    #[tokio::test]
    async fn restore_keeps_pending_ambient_cleanup_and_follower_fail_closed() {
        let temp = TempDir::new().unwrap();
        let (head, follower) = ambient_cleanup_cutpoint(temp.path(), false);
        let store = Arc::new(DownloadPersistence::new(temp.path()));
        store.reconcile_lifecycle_inventory_strict().unwrap();
        let before = store.load_lifecycle_inventory_strict().unwrap();
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(store.clone());
        assert!(matches!(client.restore_persisted_downloads().await,
            Err(PumasError::Other(message))
                if message == "Download restore requires unresolved admission or quarantine reconciliation"));
        assert!(client.list_downloads().await.is_empty());
        assert_eq!(
            std::fs::read(head.dest_dir.join("head.gguf.part")).unwrap(),
            b"old"
        );
        let after = store.load_lifecycle_inventory_strict().unwrap();
        assert_eq!(
            serde_json::to_value(&after.queue_admissions).unwrap(),
            serde_json::to_value(&before.queue_admissions).unwrap()
        );
        assert!(after.queue_admissions.contains_key(&follower.download_id));
        assert_eq!(
            after.quarantines[&head.download_id].disposition,
            LifecycleCleanupDisposition::Pending
        );
    }

    #[tokio::test]
    async fn restore_verified_settlement_remains_owned_after_caller_cancellation() {
        let temp = TempDir::new().unwrap();
        let (head, _) = ambient_cleanup_cutpoint(temp.path(), true);
        let store = Arc::new(DownloadPersistence::new(temp.path()));
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(store.clone());
        let client = Arc::new(client);
        let (entered, ready) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered));
        let (release, blocked) = std::sync::mpsc::channel();
        let blocked = std::sync::Mutex::new(blocked);
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "reconcile download restore inventory" {
                    if let Some(entered) = entered.lock().unwrap().take() {
                        let _ = entered.send(());
                        blocked.lock().unwrap().recv().unwrap();
                    }
                }
            })));
        let restore = tokio::spawn({
            let client = client.clone();
            async move { client.restore_persisted_downloads().await }
        });
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        restore.abort();
        assert!(restore.await.unwrap_err().is_cancelled());
        let remains_owned = !client.download_tasks.is_empty()
            || client.download_tasks.outstanding_retired_for_test() > 0;
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while !client.download_tasks.is_empty()
                || client.download_tasks.outstanding_retired_for_test() > 0
            {
                client.observe_finished_download_tasks().await;
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the retained restore operation must settle after its caller leaves");
        client.download_tasks.set_blocking_observer(None);
        assert!(remains_owned);
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        assert!(!inventory.queue_admissions.contains_key(&head.download_id));
        assert_eq!(
            inventory.quarantines[&head.download_id].disposition,
            LifecycleCleanupDisposition::Verified
        );
        assert!(client
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn restore_confirms_terminal_cleanup_intent_before_restoring_follower() {
        for domain in [
            LifecycleQuarantineDomain::Ambient,
            LifecycleQuarantineDomain::Recovery,
        ] {
            let temp = TempDir::new().unwrap();
            let (head, follower) = admitted_cleanup_queue(temp.path());
            let completing_store = DownloadPersistence::new(temp.path());
            completing_store
                .reconcile_lifecycle_inventory_strict()
                .unwrap();
            let attempt = completing_store
                .load_lifecycle_inventory_strict()
                .unwrap()
                .queue_admissions[&head.download_id]
                .attempt_id
                .clone();
            if domain == LifecycleQuarantineDomain::Recovery {
                completing_store
                    .revoke_admitted_for_recovery(&head.download_id, &attempt, &head)
                    .unwrap()
                    .into_result()
                    .unwrap();
            }
            completing_store
                .begin_lifecycle_quarantine(&head, domain, true, Some(&attempt))
                .unwrap();
            let part = head.dest_dir.join("head.gguf.part");
            let marker = head.dest_dir.join(".pumas_download");
            std::fs::remove_file(&part).unwrap();
            std::fs::File::open(&head.dest_dir)
                .unwrap()
                .sync_all()
                .unwrap();
            assert!(
                matches!(completing_store.verify_cleanup_with_interrupted_confirmation_for_test(&head.download_id),
                Err(PumasError::Other(message)) if message == "injected pre-publication failure")
            );
            let before: serde_json::Value =
                serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                    .unwrap();
            assert_eq!(
                before["lifecycle_quarantines"][&head.download_id]["disposition"],
                "verified_intent"
            );
            // Successor sentinels are installed after the completed cleanup.
            // Restore may finish publication, but must not replay that cleanup.
            std::fs::write(&part, b"successor partial bytes").unwrap();
            std::fs::write(&marker, b"successor marker").unwrap();
            let store = Arc::new(DownloadPersistence::new(temp.path()));
            let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
            client
                .configure_download_destination_root(temp.path())
                .unwrap();
            client.set_persistence(store.clone());
            let deletions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            client.download_tasks.set_blocking_observer(Some(Arc::new({
                let deletions = deletions.clone();
                move |operation| {
                    if operation.starts_with("remove ") {
                        deletions.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })));
            assert!(client
                .restore_persisted_downloads()
                .await
                .unwrap()
                .is_empty());
            client.download_tasks.set_blocking_observer(None);
            assert_eq!(deletions.load(Ordering::SeqCst), 0);
            assert_eq!(std::fs::read(&part).unwrap(), b"successor partial bytes");
            assert_eq!(std::fs::read(&marker).unwrap(), b"successor marker");
            let expected_head_status =
                (domain == LifecycleQuarantineDomain::Ambient).then_some(DownloadStatus::Error);
            assert_eq!(
                client.get_download_status(&head.download_id).await,
                expected_head_status
            );
            assert!(!client.resume_download(&head.download_id).await.unwrap());
            assert_eq!(
                client.get_download_status(&follower.download_id).await,
                Some(DownloadStatus::Paused)
            );
            let after: serde_json::Value =
                serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                    .unwrap();
            assert_eq!(
                after["released_queue_admissions"][&head.download_id],
                before["queue_admissions"][&head.download_id]
            );
            assert!(after["queue_admissions"].get(&head.download_id).is_none());
            assert_eq!(
                after["queue_admissions"][&follower.download_id],
                before["queue_admissions"][&follower.download_id]
            );
            assert_eq!(
                after["recovery_revocations"],
                before["recovery_revocations"]
            );
            let mut expected_quarantine =
                before["lifecycle_quarantines"][&head.download_id].clone();
            expected_quarantine["disposition"] = "verified".into();
            assert_eq!(
                after["lifecycle_quarantines"][&head.download_id],
                expected_quarantine
            );
            // Existing final fixture bytes exercise the released predecessor's
            // successor without making a network-transfer claim.
            std::fs::write(follower.dest_dir.join(&follower.filename), b"complete").unwrap();
            assert!(client.resume_download(&follower.download_id).await.unwrap());
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    client.observe_finished_download_tasks().await;
                    if client.get_download_status(&follower.download_id).await
                        == Some(DownloadStatus::Completed)
                        && !client.download_tasks.contains(&follower.download_id)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("confirmed terminal intent must release its exact follower");
            assert_eq!(std::fs::read(&part).unwrap(), b"successor partial bytes");
            let settled: serde_json::Value =
                serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                    .unwrap();
            let mut fresh = HuggingFaceClient::new(temp.path().join("fresh-cache")).unwrap();
            fresh
                .configure_download_destination_root(temp.path())
                .unwrap();
            fresh.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
            assert!(fresh
                .restore_persisted_downloads()
                .await
                .unwrap()
                .is_empty());
            assert_eq!(
                fresh.get_download_status(&head.download_id).await,
                expected_head_status
            );
            assert_eq!(
                fresh.list_downloads().await.len(),
                usize::from(domain == LifecycleQuarantineDomain::Ambient)
            );
            assert!(!fresh.resume_download(&head.download_id).await.unwrap());
            let repeated: serde_json::Value =
                serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                    .unwrap();
            assert_eq!(repeated, settled);
        }
    }

    fn ambient_cleanup_cutpoint(
        root: &Path,
        verified: bool,
    ) -> (PersistedDownload, PersistedDownload) {
        let (head, follower) = admitted_cleanup_queue(root);
        let store = DownloadPersistence::new(root);
        store.reconcile_lifecycle_inventory_strict().unwrap();
        let attempt = store
            .load_lifecycle_inventory_strict()
            .unwrap()
            .queue_admissions[&head.download_id]
            .attempt_id
            .clone();
        store
            .begin_lifecycle_quarantine(
                &head,
                LifecycleQuarantineDomain::Ambient,
                true,
                Some(&attempt),
            )
            .unwrap();
        if verified {
            // Persisted cutpoint after successful cleanup/verification, before
            // exact queue settlement; this is not a process-kill fixture.
            std::fs::remove_file(head.dest_dir.join("head.gguf.part")).unwrap();
            assert!(store
                .verify_lifecycle_quarantine(&head.download_id)
                .unwrap());
        }
        (head, follower)
    }

    fn admitted_cleanup_queue(root: &Path) -> (PersistedDownload, PersistedDownload) {
        let destination = root.join("model");
        std::fs::create_dir(&destination).unwrap();
        let store = DownloadPersistence::new(root);
        let request = recovery_test_request("owner/head", &["head.gguf".into()]);
        let head = PersistedDownload {
            download_id: "verified-cleanup-head".into(),
            repo_id: request.repo_id.clone(),
            filename: "head.gguf".into(),
            filenames: vec!["head.gguf".into()],
            dest_dir: destination.clone(),
            total_bytes: Some(8),
            status: DownloadStatus::Paused,
            download_request: request,
            created_at: "2026-09-03T00:00:00Z".into(),
            known_sha256: None,
            huggingface_evidence: None,
        };
        let mut follower = head.clone();
        follower.download_id = "verified-cleanup-follower".into();
        follower.repo_id = "owner/follower".into();
        follower.filename = "follower.gguf".into();
        follower.filenames = vec![follower.filename.clone()];
        follower.download_request = recovery_test_request(&follower.repo_id, &follower.filenames);
        admit_snapshot_at_root(&store, &head, root);
        admit_snapshot_at_root(&store, &follower, root);
        std::fs::write(destination.join("head.gguf.part"), b"old").unwrap();
        (head, follower)
    }

    fn recovery_cleanup_cutpoint(
        root: &Path,
        cleanup: Option<LifecycleCleanupDisposition>,
    ) -> (PersistedDownload, PersistedDownload) {
        let (head, follower) = admitted_cleanup_queue(root);
        let store = DownloadPersistence::new(root);
        store.reconcile_lifecycle_inventory_strict().unwrap();
        let attempt = store
            .load_lifecycle_inventory_strict()
            .unwrap()
            .queue_admissions[&head.download_id]
            .attempt_id
            .clone();
        store
            .revoke_admitted_for_recovery(&head.download_id, &attempt, &head)
            .unwrap()
            .into_result()
            .unwrap();
        if let Some(cleanup) = cleanup {
            store
                .begin_lifecycle_quarantine(
                    &head,
                    LifecycleQuarantineDomain::Recovery,
                    true,
                    Some(&attempt),
                )
                .unwrap();
            if cleanup == LifecycleCleanupDisposition::Verified {
                std::fs::remove_file(head.dest_dir.join("head.gguf.part")).unwrap();
                assert!(store
                    .verify_lifecycle_quarantine(&head.download_id)
                    .unwrap());
            }
        }
        // Intentionally omit exact settlement to represent its persisted
        // pre-settlement cutpoint, not a real process crash.
        (head, follower)
    }

    #[tokio::test]
    async fn restore_settles_verified_recovery_cleanup_without_restoring_revoked_authority() {
        let temp = TempDir::new().unwrap();
        let (head, follower) =
            recovery_cleanup_cutpoint(temp.path(), Some(LifecycleCleanupDisposition::Verified));
        let before: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        assert!(before["recovery_revocations"][&head.download_id].is_object());
        assert!(before["lifecycle_quarantines"][&head.download_id].is_object());
        let original_admission = &before["queue_admissions"][&head.download_id];
        assert!(original_admission.is_object());
        let store = Arc::new(DownloadPersistence::new(temp.path()));
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(store.clone());
        assert!(client
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        assert_eq!(client.get_download_status(&head.download_id).await, None);
        assert!(!client.resume_download(&head.download_id).await.unwrap());
        assert!(!client.cancel_download(&head.download_id).await.unwrap());
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        assert!(!inventory.queue_admissions.contains_key(&head.download_id));
        let admission = &inventory.queue_admissions[&follower.download_id];
        assert_eq!(admission.position.ordinal, 1);
        assert_eq!(
            admission.position.predecessor.as_ref().unwrap().download_id,
            head.download_id
        );
        assert_eq!(
            admission
                .position
                .predecessor
                .as_ref()
                .unwrap()
                .admission_attempt_id,
            original_admission["attempt_id"].as_str().unwrap()
        );
        let quarantine = &inventory.quarantines[&head.download_id];
        assert_eq!(quarantine.domain, LifecycleQuarantineDomain::Recovery);
        assert_eq!(
            quarantine.disposition,
            LifecycleCleanupDisposition::Verified
        );
        assert!(quarantine.sticky_failure);
        assert!(store.is_revoked(&head.download_id).unwrap());
        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        assert_eq!(
            after["released_queue_admissions"][&head.download_id],
            *original_admission
        );
        assert_eq!(
            after["recovery_revocations"][&head.download_id],
            before["recovery_revocations"][&head.download_id]
        );
        assert_eq!(
            after["lifecycle_quarantines"][&head.download_id],
            before["lifecycle_quarantines"][&head.download_id]
        );

        // Known final fixture bytes prove follower execution without HTTP.
        std::fs::write(follower.dest_dir.join(&follower.filename), b"complete").unwrap();
        assert!(client.resume_download(&follower.download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                client.observe_finished_download_tasks().await;
                if client.get_download_status(&follower.download_id).await
                    == Some(DownloadStatus::Completed)
                    && !client.download_tasks.contains(&follower.download_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the revoked predecessor must not strand its follower");
        let mut fresh = HuggingFaceClient::new(temp.path().join("fresh-cache")).unwrap();
        fresh
            .configure_download_destination_root(temp.path())
            .unwrap();
        fresh.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        assert!(fresh
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        assert!(fresh.list_downloads().await.is_empty());
        assert!(!fresh.resume_download(&head.download_id).await.unwrap());
    }

    #[tokio::test]
    async fn restore_refuses_pending_recovery_cleanup_without_changing_custody() {
        assert_recovery_restore_refusal(Some(LifecycleCleanupDisposition::Pending)).await;
    }

    #[tokio::test]
    async fn restore_refuses_unquarantined_recovery_without_changing_custody() {
        assert_recovery_restore_refusal(None).await;
    }

    async fn assert_recovery_restore_refusal(cleanup: Option<LifecycleCleanupDisposition>) {
        let temp = TempDir::new().unwrap();
        let (head, follower) = recovery_cleanup_cutpoint(temp.path(), cleanup);
        let before: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        assert!(before["recovery_revocations"][&head.download_id].is_object());
        assert!(before["queue_admissions"][&head.download_id].is_object());
        assert!(before["queue_admissions"][&follower.download_id].is_object());
        if cleanup.is_some() {
            assert!(before["lifecycle_quarantines"][&head.download_id].is_object());
        }
        let store = Arc::new(DownloadPersistence::new(temp.path()));
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(store.clone());
        assert!(matches!(client.restore_persisted_downloads().await,
            Err(PumasError::Validation { field, message })
                if field == "download_recovery"
                    && message == "Active recovery custody requires explicit reconciliation before restore"));
        assert!(client.list_downloads().await.is_empty());
        assert!(!client.resume_download(&head.download_id).await.unwrap());
        assert_eq!(
            std::fs::read(head.dest_dir.join("head.gguf.part")).unwrap(),
            b"old"
        );
        assert!(store.is_revoked(&head.download_id).unwrap());
        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        assert_eq!(after["queue_admissions"], before["queue_admissions"]);
        assert_eq!(
            after["recovery_revocations"],
            before["recovery_revocations"]
        );
        assert_eq!(
            after["lifecycle_quarantines"],
            before["lifecycle_quarantines"]
        );
    }

    #[tokio::test]
    async fn cancelling_persisted_download_quarantines_before_cleanup_and_restart() {
        let temp = TempDir::new().unwrap();
        let (client, source) = admitted_download_fixture(temp.path()).await;
        let (entered_sender, entered) = tokio::sync::oneshot::channel();
        let entered_sender = Arc::new(std::sync::Mutex::new(Some(entered_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_blocking_observer(Some(Arc::new(move |operation| {
                if operation == "remove ambient partial download file" {
                    if let Some(sender) = entered_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                        let _ = release.lock().unwrap().recv();
                    }
                }
            })));
        assert!(client.cancel_download("admitted-cleanup").await.unwrap());
        tokio::time::timeout(Duration::from_secs(3), entered)
            .await
            .unwrap()
            .unwrap();
        let mut reopened = HuggingFaceClient::new(temp.path().join("reopened")).unwrap();
        reopened
            .configure_download_destination_root(temp.path())
            .unwrap();
        reopened.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        let restore = reopened.restore_persisted_downloads().await;
        let retained = std::fs::read(source.join("weights.gguf.part")).unwrap();
        release_sender.send(()).unwrap();
        client.download_tasks.set_blocking_observer(None);
        assert!(
            restore.is_err(),
            "in-progress cancellation must not restore ordinary resumable authority"
        );
        assert_eq!(retained, b"abc");
        tokio::time::timeout(Duration::from_secs(3), async {
            while client.get_download_status("admitted-cleanup").await
                != Some(DownloadStatus::Cancelled)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        reopened.restore_persisted_downloads().await.unwrap();
        assert!(reopened.list_downloads().await.is_empty());
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
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
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
                    task_registered: false,
                    lifecycle_failure_unverified: false,
                    dest_dir: tmp.path().join("owner-model"),
                    ambient_authority_blocked: false,
                    admission: None,
                    revoked_snapshot: None,
                    destination: None,
                    filename: "model.safetensors".to_string(),
                    files: vec![FileToDownload {
                        filename: "model.safetensors".to_string(),
                        size: Some(1024),
                        sha256: None,
                    }],
                    files_completed: 0,
                    download_request: Some(request),
                    known_sha256: None,
                    huggingface_evidence: None,
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
    async fn test_list_downloads_scopes_file_group_selected_artifact_from_state_files() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let download_id = "dl-file-group-progress".to_string();

        let request = DownloadRequest {
            repo_id: "owner/multi-file".to_string(),
            family: "owner".to_string(),
            official_name: "Multi File".to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: Some(vec![
                "config.json".to_string(),
                "model.safetensors".to_string(),
            ]),
            pipeline_tag: Some("text-generation".to_string()),
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/multi-file".to_string(),
                    status: DownloadStatus::Paused,
                    progress: 0.25,
                    downloaded_bytes: 256,
                    total_bytes: Some(1024),
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 0,
                    retry_limit: None,
                    retrying: false,
                    next_retry_delay_seconds: None,
                    task_registered: false,
                    lifecycle_failure_unverified: false,
                    dest_dir: tmp.path().join("owner-multi-file"),
                    ambient_authority_blocked: false,
                    admission: None,
                    revoked_snapshot: None,
                    destination: None,
                    filename: "config.json".to_string(),
                    files: vec![
                        FileToDownload {
                            filename: "config.json".to_string(),
                            size: Some(128),
                            sha256: None,
                        },
                        FileToDownload {
                            filename: "model.safetensors".to_string(),
                            size: Some(896),
                            sha256: None,
                        },
                    ],
                    files_completed: 0,
                    download_request: Some(request),
                    known_sha256: None,
                    huggingface_evidence: None,
                },
            );
        }

        let list = client.list_downloads().await;
        let progress = list
            .into_iter()
            .find(|item| item.download_id == download_id)
            .expect("download progress should be present");

        assert_eq!(progress.repo_id.as_deref(), Some("owner/multi-file"));
        assert!(progress
            .selected_artifact_id
            .as_deref()
            .is_some_and(|artifact_id| artifact_id.starts_with("owner--multi-file__files_")));
    }

    #[tokio::test]
    async fn test_list_downloads_preserves_full_repo_identity_for_repo_scoped_state() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let download_id = "dl-full-repo-progress".to_string();

        let request = DownloadRequest {
            repo_id: "owner/multi-file".to_string(),
            family: "owner".to_string(),
            official_name: "Multi File".to_string(),
            model_type: Some("llm".to_string()),
            quant: None,
            filename: None,
            filenames: None,
            pipeline_tag: Some("text-generation".to_string()),
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/multi-file".to_string(),
                    status: DownloadStatus::Paused,
                    progress: 0.25,
                    downloaded_bytes: 256,
                    total_bytes: Some(1024),
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 0,
                    retry_limit: None,
                    retrying: false,
                    next_retry_delay_seconds: None,
                    task_registered: false,
                    lifecycle_failure_unverified: false,
                    dest_dir: tmp.path().join("owner-multi-file"),
                    ambient_authority_blocked: false,
                    admission: None,
                    revoked_snapshot: None,
                    destination: None,
                    filename: "config.json".to_string(),
                    files: vec![
                        FileToDownload {
                            filename: "config.json".to_string(),
                            size: Some(128),
                            sha256: None,
                        },
                        FileToDownload {
                            filename: "model.safetensors".to_string(),
                            size: Some(896),
                            sha256: None,
                        },
                    ],
                    files_completed: 0,
                    download_request: Some(request),
                    known_sha256: None,
                    huggingface_evidence: None,
                },
            );
        }

        let progress = client
            .list_downloads()
            .await
            .into_iter()
            .find(|item| item.download_id == download_id)
            .expect("download progress should be present");

        assert_eq!(
            progress.selected_artifact_id.as_deref(),
            Some("owner--multi-file__full_repo")
        );
    }

    #[tokio::test]
    async fn test_list_downloads_pauses_registered_active_state_without_task() {
        let tmp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(tmp.path()).unwrap();
        client
            .configure_download_destination_root(tmp.path())
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(tmp.path()));
        client.set_persistence(persistence.clone());

        let download_id = "dl-stale-active".to_string();
        let request = DownloadRequest {
            repo_id: "owner/model".to_string(),
            family: "owner".to_string(),
            official_name: "Model".to_string(),
            model_type: Some("llm".to_string()),
            quant: Some("Q4_K_M".to_string()),
            filename: None,
            filenames: None,
            pipeline_tag: Some("text-generation".to_string()),
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let attempt_id = admit_snapshot_at_root(
            &persistence,
            &PersistedDownload {
                download_id: download_id.clone(),
                repo_id: "owner/model".to_string(),
                filename: "model.Q4_K_M.gguf".to_string(),
                filenames: vec!["model.Q4_K_M.gguf".to_string()],
                dest_dir: tmp.path().join("owner-model"),
                total_bytes: Some(1024),
                status: DownloadStatus::Downloading,
                download_request: request.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
                huggingface_evidence: None,
            },
            tmp.path(),
        );

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/model".to_string(),
                    status: DownloadStatus::Downloading,
                    progress: 0.5,
                    downloaded_bytes: 512,
                    total_bytes: Some(1024),
                    speed: 1024.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 0,
                    retry_limit: None,
                    retrying: true,
                    next_retry_delay_seconds: Some(1.0),
                    task_registered: true,
                    lifecycle_failure_unverified: false,
                    dest_dir: tmp.path().join("owner-model"),
                    ambient_authority_blocked: false,
                    admission: Some(super::super::types::AdmittedDownload { attempt_id }),
                    revoked_snapshot: None,
                    destination: Some(super::super::types::DownloadDestination::Managed(
                        crate::model_library::download_recovery::DownloadDestinationRoot::open(
                            tmp.path(),
                        )
                        .unwrap()
                        .resolve(&tmp.path().join("owner-model"))
                        .unwrap(),
                    )),
                    filename: "model.Q4_K_M.gguf".to_string(),
                    files: vec![FileToDownload {
                        filename: "model.Q4_K_M.gguf".to_string(),
                        size: Some(1024),
                        sha256: None,
                    }],
                    files_completed: 0,
                    download_request: Some(request),
                    known_sha256: None,
                    huggingface_evidence: None,
                },
            );
        }

        let progress = client
            .list_downloads()
            .await
            .into_iter()
            .find(|progress| progress.download_id == download_id)
            .expect("download should remain listed");

        assert_eq!(progress.status, DownloadStatus::Paused);
        assert_eq!(progress.speed, Some(0.0));
        assert_eq!(progress.retrying, Some(false));
        assert_eq!(progress.next_retry_delay_seconds, None);
        assert_eq!(persistence.load_all()[0].status, DownloadStatus::Paused);
    }

    #[tokio::test]
    async fn reconciliation_without_a_durable_ambient_row_fails_closed() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        client.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "reconcile-missing-durable-row";
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true);
        state.make_managed_for_test();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);

        let _ = client.list_downloads().await;
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(!state.task_registered);
        assert!(state.lifecycle_failure_unverified);
        drop(downloads);
        assert!(!client.download_tasks.contains(download_id));

        assert!(client.cancel_download(download_id).await.unwrap());
        tokio::time::timeout(Duration::from_secs(2), async {
            while client
                .downloads
                .read()
                .await
                .get(download_id)
                .is_some_and(|state| state.status == DownloadStatus::Cancelling)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Error);
        assert!(state.lifecycle_failure_unverified);
    }

    #[tokio::test]
    async fn reconciliation_rechecks_owner_installed_after_its_initial_snapshot() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(recovery_test_client(
            temp.path().join("cache"),
            &library_root,
        ));
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "reconcile-install-race";
        let mut downloads_guard = client.downloads.write().await;
        downloads_guard.insert(
            download_id.to_string(),
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true),
        );

        let (snapshotted_sender, snapshotted) = tokio::sync::oneshot::channel();
        let snapshotted_sender = Arc::new(std::sync::Mutex::new(Some(snapshotted_sender)));
        client.download_tasks.set_ids_observer(Some(Arc::new({
            let snapshotted_sender = snapshotted_sender.clone();
            move || {
                if let Some(sender) = snapshotted_sender.lock().unwrap().take() {
                    let _ = sender.send(());
                }
            }
        })));
        let listing = {
            let client = client.clone();
            tokio::spawn(async move { client.list_downloads().await })
        };
        snapshotted.await.unwrap();

        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                panic!("finished successor sentinel")
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        while !client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.finished)
        {
            tokio::task::yield_now().await;
        }
        drop(downloads_guard);
        let _ = listing.await.unwrap();
        client.download_tasks.set_ids_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Downloading);
        assert!(state.task_registered);
        drop(downloads);
        assert!(client.cancel_download(download_id).await.unwrap());
    }

    #[tokio::test]
    async fn reconciliation_does_not_persist_pause_for_a_newly_installed_owner() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        std::fs::create_dir_all(&library_root).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        client.set_persistence(persistence.clone());
        let client = Arc::new(client);
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "reconcile-persist-install-race";
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true);
        state.make_managed_for_test();
        persist_state_fixture(&persistence, &mut state);
        let mut downloads_guard = client.downloads.write().await;
        downloads_guard.insert(download_id.to_string(), state);

        let (snapshotted_sender, snapshotted) = tokio::sync::oneshot::channel();
        let snapshotted_sender = Arc::new(std::sync::Mutex::new(Some(snapshotted_sender)));
        client.download_tasks.set_ids_observer(Some(Arc::new({
            let snapshotted_sender = snapshotted_sender.clone();
            move || {
                if let Some(sender) = snapshotted_sender.lock().unwrap().take() {
                    let _ = sender.send(());
                }
            }
        })));
        let listing = {
            let client = client.clone();
            tokio::spawn(async move { client.list_downloads().await })
        };
        snapshotted.await.unwrap();
        let prepared = client
            .download_tasks
            .prepare(download_id.to_string(), TaskRole::Worker, |_| async {
                panic!("finished persisted successor sentinel")
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();
        while !client
            .download_tasks
            .snapshot(download_id)
            .is_some_and(|task| task.finished)
        {
            tokio::task::yield_now().await;
        }
        drop(downloads_guard);
        let _ = listing.await.unwrap();
        client.download_tasks.set_ids_observer(None);

        let downloads = client.downloads.read().await;
        let state = downloads.get(download_id).unwrap();
        assert_eq!(state.status, DownloadStatus::Downloading);
        assert!(state.task_registered);
        drop(downloads);
        assert_eq!(
            persistence.load_all()[0].status,
            DownloadStatus::Downloading
        );
        assert!(client.cancel_download(download_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_remove_stale_part_for_completed_file_only_removes_matching_part() {
        let tmp = TempDir::new().unwrap();
        let final_path = tmp.path().join("model.gguf");
        let part_path = tmp.path().join("model.gguf.part");
        let other_part_path = tmp.path().join("other.gguf.part");

        tokio::fs::write(&final_path, b"done").await.unwrap();
        tokio::fs::write(&part_path, b"stale").await.unwrap();
        tokio::fs::write(&other_part_path, b"keep").await.unwrap();

        HuggingFaceClient::remove_stale_part_for_completed_file(&final_path, &part_path).await;

        assert!(tokio::fs::try_exists(&final_path).await.unwrap());
        assert!(!tokio::fs::try_exists(&part_path).await.unwrap());
        assert!(tokio::fs::try_exists(&other_part_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_finalize_complete_part_file_promotes_exact_expected_size() {
        let tmp = TempDir::new().unwrap();
        let final_path = tmp.path().join("model.gguf");
        let part_path = tmp.path().join("model.gguf.part");
        tokio::fs::write(&part_path, b"done").await.unwrap();

        let finalized =
            HuggingFaceClient::finalize_complete_part_file(&final_path, &part_path, Some(4))
                .await
                .unwrap();

        assert!(finalized);
        assert_eq!(tokio::fs::read(&final_path).await.unwrap(), b"done");
        assert!(!tokio::fs::try_exists(&part_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_finalize_complete_part_file_keeps_short_partial() {
        let tmp = TempDir::new().unwrap();
        let final_path = tmp.path().join("model.gguf");
        let part_path = tmp.path().join("model.gguf.part");
        tokio::fs::write(&part_path, b"short").await.unwrap();

        let finalized =
            HuggingFaceClient::finalize_complete_part_file(&final_path, &part_path, Some(10))
                .await
                .unwrap();

        assert!(!finalized);
        assert!(!tokio::fs::try_exists(&final_path).await.unwrap());
        assert!(tokio::fs::try_exists(&part_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_restore_auto_finalizes_byte_complete_persisted_download() {
        let tmp = TempDir::new().unwrap();
        let dest_dir = tmp.path().join("library").join("llm/test/ready-model");
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::write(dest_dir.join("model.gguf.part"), b"done").unwrap();
        std::fs::write(dest_dir.join(".pumas_download"), b"{}").unwrap();

        let persistence = Arc::new(DownloadPersistence::new(tmp.path()));
        admit_snapshot_at_root(
            &persistence,
            &PersistedDownload {
                download_id: "ready-download".to_string(),
                repo_id: "owner/model".to_string(),
                filename: "model.gguf".to_string(),
                filenames: vec!["model.gguf".to_string()],
                dest_dir: dest_dir.clone(),
                total_bytes: Some(4),
                status: DownloadStatus::Error,
                download_request: DownloadRequest {
                    repo_id: "owner/model".to_string(),
                    family: "test".to_string(),
                    official_name: "Ready Model".to_string(),
                    model_type: Some("llm".to_string()),
                    quant: None,
                    filename: Some("model.gguf".to_string()),
                    filenames: None,
                    pipeline_tag: Some("text-generation".to_string()),
                    bundle_format: None,
                    pipeline_class: None,
                    release_date: None,
                    download_url: None,
                    model_card_json: None,
                    license_status: None,
                },
                created_at: chrono::Utc::now().to_rfc3339(),
                known_sha256: None,
                huggingface_evidence: None,
            },
            tmp.path(),
        );

        let mut client = HuggingFaceClient::new(tmp.path()).unwrap();
        client.set_persistence(persistence.clone());
        client
            .configure_download_destination_root(tmp.path())
            .unwrap();
        let completed = client.restore_persisted_downloads().await.unwrap();

        assert!(client.list_downloads().await.is_empty());
        assert_eq!(completed.len(), 1);
        assert!(persistence.load_all().is_empty());
        assert_eq!(std::fs::read(dest_dir.join("model.gguf")).unwrap(), b"done");
        assert!(!dest_dir.join("model.gguf.part").exists());
        assert!(!dest_dir.join(".pumas_download").exists());
    }

    #[tokio::test]
    async fn restore_completion_handoff_survives_public_retirement_and_preserves_successor() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let verified = verified_recovery(&library_root, "acme/original", &["original.gguf"]);
        let download_id = "restore-completion-handoff";
        let mut original =
            recovery_test_state(&verified, download_id, DownloadStatus::Paused, false);
        original.make_managed_for_test();
        let persistence = Arc::new(DownloadPersistence::new(temp.path()));
        persist_state_fixture(&persistence, &mut original);
        std::fs::write(
            verified
                .destination
                .display_path()
                .join("original.gguf.part"),
            b"done",
        )
        .unwrap();
        std::fs::write(
            verified.destination.display_path().join(".pumas_download"),
            b"{}",
        )
        .unwrap();
        let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        client
            .configure_download_destination_root(&library_root)
            .unwrap();
        client.set_persistence(persistence);
        let client = Arc::new(client);
        let (reached_sender, reached) = tokio::sync::oneshot::channel();
        let reached_sender = Arc::new(std::sync::Mutex::new(Some(reached_sender)));
        let (release_sender, release) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Mutex::new(release));
        client
            .download_tasks
            .set_ambient_admission_observer(Some(Arc::new({
                let reached_sender = reached_sender.clone();
                let release = release.clone();
                move |operation, observed_id| {
                    if operation == "restore-finalization-result" && observed_id == download_id {
                        if let Some(sender) = reached_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                    }
                }
            })));
        let restore = std::thread::spawn({
            let client = client.clone();
            move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(client.restore_persisted_downloads())
            }
        });
        tokio::time::timeout(Duration::from_secs(3), reached)
            .await
            .expect("restore must reach its completed result handoff")
            .unwrap();

        tokio::time::timeout(Duration::from_secs(3), async {
            while !client
                .download_tasks
                .snapshot(download_id)
                .is_some_and(|task| task.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("original owner must finish independently of its result consumer");

        let listed = client.list_downloads().await;
        assert!(listed
            .iter()
            .filter(|entry| entry.download_id == download_id)
            .all(|entry| entry.status == DownloadStatus::Completed));
        assert_eq!(
            std::fs::read(verified.destination.display_path().join("original.gguf")).unwrap(),
            b"done"
        );
        assert!(
            !client.download_tasks.contains(download_id),
            "public observation must retire the original completed owner before result extraction"
        );

        // Install a distinct runtime generation to prove result extraction is
        // not an ID-only removal of whichever state happens to be present.
        let replacement =
            verified_recovery(&library_root, "acme/replacement", &["replacement.gguf"]);
        let (finish_sender, finish) = tokio::sync::oneshot::channel();
        let prepared = client
            .download_tasks
            .prepare(download_id.into(), TaskRole::Worker, move |_| async move {
                let _ = finish.await;
            })
            .unwrap();
        let installed = {
            let mut states = client.downloads.write().await;
            let installed = client.download_tasks.install_gated(prepared).unwrap();
            states.insert(
                download_id.into(),
                recovery_test_state(&replacement, download_id, DownloadStatus::Downloading, true),
            );
            installed
        };
        let successor_generation = installed.generation().clone();
        installed.start();
        release_sender.send(()).unwrap();
        let completed = restore.join().unwrap().unwrap();
        let successor_preserved = client
            .download_tasks
            .generation_is_current(download_id, &successor_generation);
        let replacement_filename = client
            .downloads
            .read()
            .await
            .get(download_id)
            .map(|state| state.filename.clone());
        let repeated = client.restore_persisted_downloads().await.unwrap();
        client.download_tasks.set_ambient_admission_observer(None);
        finish_sender.send(()).unwrap();

        assert_eq!(
            completed.len(),
            1,
            "the original owned completion must survive observer retirement"
        );
        assert_eq!(completed[0].download_id, download_id);
        assert_eq!(completed[0].filename, "original.gguf");
        assert_eq!(completed[0].dest_dir, verified.destination.display_path());
        assert!(successor_preserved);
        assert_eq!(replacement_filename.as_deref(), Some("replacement.gguf"));
        assert!(
            repeated.is_empty(),
            "the original completion must only be returned once"
        );
    }

    #[tokio::test]
    async fn restore_does_not_auto_finalize_complete_follower_before_incomplete_head() {
        let temp = TempDir::new().unwrap();
        let client = configured_download_client(temp.path().join("cache")).unwrap();
        let destination = temp.path().join("model");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("head.gguf.part"), b"old").unwrap();
        std::fs::write(destination.join("follower.gguf.part"), b"done").unwrap();
        let marker = br#"{"repo_id":"acme/head","sentinel":"untouched"}"#;
        std::fs::write(destination.join(".pumas_download"), marker).unwrap();
        for (id, filename, total) in [("head", "head.gguf", 8), ("follower", "follower.gguf", 4)] {
            admit_snapshot_at_root(
                client.persistence.as_ref().unwrap(),
                &PersistedDownload {
                    download_id: id.into(),
                    repo_id: format!("acme/{id}"),
                    filename: filename.into(),
                    filenames: vec![filename.into()],
                    dest_dir: destination.clone(),
                    total_bytes: Some(total),
                    status: DownloadStatus::Paused,
                    download_request: recovery_test_request(
                        &format!("acme/{id}"),
                        &[filename.into()],
                    ),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    known_sha256: None,
                    huggingface_evidence: None,
                },
                temp.path(),
            );
        }
        assert!(client
            .restore_persisted_downloads()
            .await
            .unwrap()
            .is_empty());
        assert_eq!(client.list_downloads().await.len(), 2);
        assert_eq!(
            std::fs::read(destination.join("head.gguf.part")).unwrap(),
            b"old"
        );
        assert_eq!(
            std::fs::read(destination.join("follower.gguf.part")).unwrap(),
            b"done"
        );
        assert_eq!(
            std::fs::read(destination.join(".pumas_download")).unwrap(),
            marker
        );
        assert!(!destination.join("follower.gguf").exists());
        assert_eq!(client.persistence.as_ref().unwrap().load_all().len(), 2);
    }

    #[tokio::test]
    async fn test_destination_lock_reuses_same_mutex_for_same_path() {
        let tmp = TempDir::new().unwrap();
        let client = configured_download_client(tmp.path().to_path_buf()).unwrap();
        let a = tmp.path().join("llm/owner/model");
        let b = tmp.path().join("llm/owner/model");
        let c = tmp.path().join("llm/owner/other-model");

        let lock_a = client
            .destination_lock(&destination_identity(&client, &a))
            .await;
        let lock_b = client
            .destination_lock(&destination_identity(&client, &b))
            .await;
        let lock_c = client
            .destination_lock(&destination_identity(&client, &c))
            .await;

        assert!(Arc::ptr_eq(&lock_a, &lock_b));
        assert!(!Arc::ptr_eq(&lock_a, &lock_c));
    }

    #[tokio::test]
    async fn test_cancel_download_aborts_tracked_task() {
        let tmp = TempDir::new().unwrap();
        let client = recovery_test_client(tmp.path().to_path_buf(), tmp.path());
        let destination =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(tmp.path())
                .unwrap()
                .resolve(&tmp.path().join("owner-model"))
                .unwrap();
        let download_id = "dl-cancel".to_string();
        let mut updates = client.subscribe_download_updates();

        {
            let mut downloads = client.downloads.write().await;
            downloads.insert(
                download_id.clone(),
                DownloadState {
                    download_id: download_id.clone(),
                    repo_id: "owner/model".to_string(),
                    status: DownloadStatus::Downloading,
                    progress: 0.25,
                    downloaded_bytes: 256,
                    total_bytes: Some(1024),
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    retry_attempt: 0,
                    retry_limit: None,
                    retrying: false,
                    next_retry_delay_seconds: None,
                    task_registered: true,
                    lifecycle_failure_unverified: false,
                    dest_dir: tmp.path().join("owner-model"),
                    ambient_authority_blocked: false,
                    admission: None,
                    revoked_snapshot: None,
                    destination: Some(DownloadDestination::Managed(destination)),
                    filename: "model.safetensors".to_string(),
                    files: vec![FileToDownload {
                        filename: "model.safetensors".to_string(),
                        size: Some(1024),
                        sha256: None,
                    }],
                    files_completed: 0,
                    download_request: None,
                    known_sha256: None,
                    huggingface_evidence: None,
                },
            );
        }

        let prepared = client
            .download_tasks
            .prepare(download_id.clone(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await
            })
            .unwrap();
        client
            .download_tasks
            .install_gated(prepared)
            .unwrap()
            .start();

        assert!(client.cancel_download(&download_id).await.unwrap());
        let first_update = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .expect("download update should be published")
            .expect("download update channel should stay open");
        let second_update = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .expect("terminal download update should be published")
            .expect("download update channel should stay open");

        let published_statuses = [first_update, second_update]
            .into_iter()
            .flat_map(|notification| notification.snapshot.downloads)
            .filter(|download| download.download_id == download_id)
            .map(|download| download.status)
            .collect::<Vec<_>>();
        assert!(published_statuses.contains(&DownloadStatus::Cancelling));
        assert!(published_statuses.contains(&DownloadStatus::Cancelled));

        let cancel_flag_set = client
            .downloads
            .read()
            .await
            .get(&download_id)
            .unwrap()
            .cancel_flag
            .load(Ordering::Relaxed);
        assert!(cancel_flag_set);
        client.observe_finished_download_tasks().await;
        assert!(!client.download_tasks.contains(&download_id));
        assert_eq!(
            client.get_download_status(&download_id).await,
            Some(DownloadStatus::Cancelled)
        );
    }

    #[tokio::test]
    async fn test_download_notification_since_current_cursor_returns_none() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let snapshot = client.download_snapshot().await;
        let cursor = snapshot.cursor.clone();

        assert!(client
            .download_notification_since(Some(&cursor), snapshot)
            .is_none());
    }

    #[tokio::test]
    async fn download_snapshots_cannot_deliver_active_after_newer_terminal_state() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = Arc::new(HuggingFaceClient::new(temp.path().join("cache")).unwrap());
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "ordered-publication";
        let mut state =
            recovery_test_state(&verified, download_id, DownloadStatus::Downloading, true);
        state.make_managed_for_test();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        let mut updates = client.subscribe_download_updates();

        let (older_built_sender, older_built_receiver) = tokio::sync::oneshot::channel();
        let older_built_sender = Arc::new(std::sync::Mutex::new(Some(older_built_sender)));
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let release_receiver = Arc::new(std::sync::Mutex::new(Some(release_receiver)));
        client
            .download_publications
            .set_dispatch_observer_for_test(Some(Arc::new({
                let older_built_sender = older_built_sender.clone();
                let release_receiver = release_receiver.clone();
                move |snapshot| {
                    let is_active = snapshot.downloads.iter().any(|download| {
                        download.download_id == download_id
                            && download.status == DownloadStatus::Downloading
                    });
                    if is_active {
                        if let Some(sender) = older_built_sender.lock().unwrap().take() {
                            let _ = sender.send(());
                            release_receiver
                                .lock()
                                .unwrap()
                                .take()
                                .unwrap()
                                .recv()
                                .unwrap();
                        }
                    }
                }
            })));

        let older = {
            let client = client.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(client.publish_download_snapshot())
            })
        };
        tokio::time::timeout(Duration::from_secs(1), older_built_receiver)
            .await
            .expect("older active snapshot should be built")
            .unwrap();
        client
            .downloads
            .write()
            .await
            .get_mut(download_id)
            .unwrap()
            .status = DownloadStatus::Completed;
        let newer = {
            let client = client.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(client.publish_download_snapshot())
            })
        };
        release_sender.send(()).unwrap();
        older.join().expect("older publication should settle");
        newer.join().expect("newer publication should settle");

        let first = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(first.snapshot.revision < second.snapshot.revision);
        assert_eq!(
            first
                .snapshot
                .downloads
                .iter()
                .find(|download| download.download_id == download_id)
                .unwrap()
                .status,
            DownloadStatus::Downloading
        );
        assert_eq!(
            second
                .snapshot
                .downloads
                .iter()
                .find(|download| download.download_id == download_id)
                .unwrap()
                .status,
            DownloadStatus::Completed
        );
    }

    #[tokio::test]
    async fn worker_snapshot_dispatch_never_holds_the_destination_lease() {
        let temp = TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
        let verified = verified_recovery(&library_root, "acme/model", &["weights.gguf"]);
        let download_id = "guard-free-worker-publication";
        let mut state = recovery_test_state(&verified, download_id, DownloadStatus::Queued, false);
        state.make_managed_for_test();
        let files = state.files.clone();
        let cancel_flag = state.cancel_flag.clone();
        let pause_flag = state.pause_flag.clone();
        client
            .downloads
            .write()
            .await
            .insert(download_id.to_string(), state);
        std::fs::write(
            verified.destination.display_path().join("weights.gguf"),
            b"done",
        )
        .unwrap();

        let destination_lock = client
            .destination_lock(&verified.destination.identity())
            .await;
        let dispatch_count = Arc::new(AtomicU64::new(0));
        let guard_was_released = Arc::new(AtomicBool::new(true));
        client
            .download_publications
            .set_dispatch_observer_for_test(Some(Arc::new({
                let destination_lock = destination_lock.clone();
                let dispatch_count = dispatch_count.clone();
                let guard_was_released = guard_was_released.clone();
                move |snapshot| {
                    if snapshot
                        .downloads
                        .iter()
                        .any(|download| download.download_id == download_id)
                    {
                        dispatch_count.fetch_add(1, Ordering::SeqCst);
                        if destination_lock.try_lock().is_err() {
                            guard_was_released.store(false, Ordering::SeqCst);
                        }
                    }
                }
            })));

        assert!(
            client
                .spawn_download_task(
                    download_id.to_string(),
                    verified.repo_id.clone(),
                    files,
                    DownloadDestination::Managed(verified.destination.clone()),
                    cancel_flag,
                    pause_flag,
                    None,
                    None,
                    None,
                )
                .await
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.get_download_status(download_id).await != Some(DownloadStatus::Completed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker should publish its terminal state");

        assert!(dispatch_count.load(Ordering::SeqCst) >= 2);
        assert!(guard_was_released.load(Ordering::SeqCst));
        client
            .download_publications
            .set_dispatch_observer_for_test(None);
    }

    #[tokio::test]
    async fn publication_dispatch_allows_a_subscriber_to_request_another_publication() {
        let temp = TempDir::new().unwrap();
        let client = Arc::new(HuggingFaceClient::new(temp.path()).unwrap());
        let mut updates = client.subscribe_download_updates();
        let reentrant = {
            let client = client.clone();
            tokio::spawn(async move {
                updates.recv().await.unwrap();
                client.publish_download_snapshot().await;
            })
        };

        client.publish_download_snapshot().await;
        tokio::time::timeout(Duration::from_secs(1), reentrant)
            .await
            .expect("subscriber publication must not deadlock on publisher custody")
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_notification_since_stale_cursor_requires_snapshot() {
        let tmp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(tmp.path()).unwrap();
        let snapshot = client.download_snapshot().await;

        let notification = client
            .download_notification_since(Some("not-a-download-cursor"), snapshot)
            .expect("invalid cursor should require snapshot recovery");

        assert!(notification.stale_cursor);
        assert!(notification.snapshot_required);
    }
}
