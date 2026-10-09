//! Explicit native S3-to-model composition. Source access is ephemeral; the
//! acquisition consumer and model importer retain custody and publication authority.
use crate::{
    acquisition::{
        AcquisitionDemand, AcquisitionHost, AcquisitionRetryPolicy, AcquisitionS3ManifestRequest,
        AcquisitionWorkspace, HttpAttemptHost, S3Credentials, S3ManifestEntry, S3Reader,
        S3ReaderConfig, S3ReaderError, Sha256Evidence,
    },
    model_library::{ModelImportResult, ModelImportSpec, ModelImporter},
    PumasApi, PumasError,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{watch, Notify};

const PENDING: u8 = 0;
const ACTIVE: u8 = 1;
const CANCELLED: u8 = 2;
const FINALIZING: u8 = 3;
const FINISHED: u8 = 4;
const CONSUMER: &str = "model.s3.workflow";

/// Explicit native request with no Debug or serialization implementation.
/// Caller-owned workspace authority is required.
/// `operation_id` must be retained for the same logical attempt; uncertain work
/// must be reconciled through the existing consumer, never assigned a new ID to replay.
pub struct S3ModelImportRequest {
    pub operation_id: uuid::Uuid,
    pub source: S3ReaderConfig,
    pub credentials: Option<S3Credentials>,
    pub entries: Vec<S3ManifestEntry>,
    /// Exact logical primary weight path and model metadata, not a source URL.
    pub import: ModelImportSpec,
    pub workspace: AcquisitionWorkspace,
    pub retry: AcquisitionRetryPolicy,
}

/// Opt-in single-object request for a non-versioned general-purpose bucket.
/// The exact object requires HEAD size, a strong quoted HTTP ETag (W/ is
/// refused), and whole-file SHA-256. Weak revision strength classifies mutable
/// provenance, not the HTTP validator; ETag/size alone never prove integrity.
/// These authorize byte acquisition, not model execution or package completeness.
/// This does not admit conditional bundles or prefix discovery. Like the versioned
/// request, credentials are ephemeral and the caller holds workspace custody.
pub struct S3ConditionalModelImportRequest {
    pub operation_id: uuid::Uuid,
    pub source: S3ReaderConfig,
    pub credentials: Option<S3Credentials>,
    pub source_key: String,
    pub expected_sha256: Sha256Evidence,
    /// Exact logical path of the single selected model file and model metadata.
    pub import: ModelImportSpec,
    pub workspace: AcquisitionWorkspace,
    pub retry: AcquisitionRetryPolicy,
}

struct ConditionalObject {
    source_key: String,
    expected_sha256: Sha256Evidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum S3ModelImportPhase {
    Pending,
    Selecting,
    Acquiring,
    Cancelling,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
    /// The caller dropped its result waiter; owned effects may still need drainage.
    Interrupted,
}

/// Safe observation, with no endpoints, object keys, credentials or error text.
/// Bytes are the existing host's observation for the current file, not aggregate
/// file-set progress; they may reset between files/retries and are not verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct S3ModelImportProgress {
    pub phase: S3ModelImportPhase,
    pub downloaded_for_current_file: u64,
}

/// Safe complete-set observation. File indices follow the selected manifest's
/// logical-path order. Current bytes can reset on retries; acquired staging bytes
/// are separate from complete-set verification and publication success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct S3ModelBundleProgress {
    pub phase: S3ModelImportPhase,
    pub file_index: Option<usize>,
    pub files_total: usize,
    pub files_acquired: usize,
    pub bytes_acquired: u64,
    pub downloaded_for_current_file: u64,
    pub total_expected_bytes: Option<u64>,
}

struct ControlState {
    gate: AtomicU8,
    cancelled: Notify,
    progress: watch::Sender<S3ModelImportProgress>,
    bundle_progress: watch::Sender<S3ModelBundleProgress>,
}

/// One operation's cancellation latch and coalesced live progress. Cloning keeps
/// that same operation's control. Disconnecting an observer does not cancel work.
#[derive(Clone)]
pub struct S3ModelImportControl(Arc<ControlState>);

impl Default for S3ModelImportControl {
    fn default() -> Self {
        Self::new()
    }
}
impl S3ModelImportControl {
    pub fn new() -> Self {
        let (progress, _) = watch::channel(S3ModelImportProgress {
            phase: S3ModelImportPhase::Pending,
            downloaded_for_current_file: 0,
        });
        let (bundle_progress, _) = watch::channel(S3ModelBundleProgress {
            phase: S3ModelImportPhase::Pending,
            file_index: None,
            files_total: 0,
            files_acquired: 0,
            bytes_acquired: 0,
            downloaded_for_current_file: 0,
            total_expected_bytes: None,
        });
        Self(Arc::new(ControlState {
            gate: AtomicU8::new(PENDING),
            cancelled: Notify::new(),
            progress,
            bundle_progress,
        }))
    }
    pub fn subscribe(&self) -> watch::Receiver<S3ModelImportProgress> {
        self.0.progress.subscribe()
    }
    /// Aggregate staging observations for the same owned operation. Existing
    /// current-file subscribers and request/receipt formats remain unchanged.
    pub fn subscribe_bundle(&self) -> watch::Receiver<S3ModelBundleProgress> {
        self.0.bundle_progress.subscribe()
    }
    /// Returns true only if cancellation won before finalization. Once finalization
    /// starts, cancellation is refused and the receipt/publication pipeline settles.
    /// A true return is a request acknowledgement, not proof that effects stopped.
    pub fn cancel(&self) -> bool {
        let mut current = self.0.gate.load(Ordering::SeqCst);
        loop {
            if current != PENDING && current != ACTIVE {
                return false;
            }
            match self.0.gate.compare_exchange(
                current,
                CANCELLED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.phase(if current == PENDING {
                        S3ModelImportPhase::Cancelled
                    } else {
                        S3ModelImportPhase::Cancelling
                    });
                    self.0.cancelled.notify_waiters();
                    return true;
                }
                Err(next) => current = next,
            }
        }
    }
    fn is_cancelled(&self) -> bool {
        self.0.gate.load(Ordering::SeqCst) == CANCELLED
    }
    fn phase(&self, phase: S3ModelImportPhase) {
        self.0.progress.send_modify(|progress| {
            // A delayed stage/cancellation notification cannot replace a newer
            // terminal observation. The gate owns cancellation admission.
            let gate = self.0.gate.load(Ordering::SeqCst);
            let valid = match phase {
                S3ModelImportPhase::Pending => gate == PENDING,
                S3ModelImportPhase::Selecting | S3ModelImportPhase::Acquiring => gate == ACTIVE,
                S3ModelImportPhase::Cancelling => gate == CANCELLED,
                S3ModelImportPhase::Finalizing => gate == FINALIZING,
                S3ModelImportPhase::Cancelled => gate == CANCELLED || gate == FINISHED,
                S3ModelImportPhase::Completed
                | S3ModelImportPhase::Failed
                | S3ModelImportPhase::Interrupted => gate == FINISHED,
            };
            if valid {
                progress.phase = phase;
                self.0
                    .bundle_progress
                    .send_modify(|bundle| bundle.phase = phase);
            }
        });
    }
    async fn wait_for_cancel(&self) {
        loop {
            let waiting = self.0.cancelled.notified();
            tokio::pin!(waiting);
            waiting.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            waiting.await;
        }
    }
    fn begin_finalization(&self) -> crate::Result<()> {
        self.0
            .gate
            .compare_exchange(ACTIVE, FINALIZING, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| PumasError::DownloadCancelled)?;
        self.phase(S3ModelImportPhase::Finalizing);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum S3ModelImportError {
    #[error(transparent)]
    Source(#[from] S3ReaderError),
    #[error(transparent)]
    Operation(#[from] PumasError),
    #[error("S3 model workflow could not drain its owned consumer scope")]
    Drain {
        #[source]
        source: PumasError,
        operation_error: Option<Box<S3ModelImportError>>,
        /// A published result may exist even though scope drainage failed.
        published_result: Option<ModelImportResult>,
    },
}

struct ProgressGuard {
    control: S3ModelImportControl,
    complete: bool,
}
impl ProgressGuard {
    fn finish(&mut self, result: &std::result::Result<ModelImportResult, S3ModelImportError>) {
        self.control.0.gate.store(FINISHED, Ordering::SeqCst);
        self.control.phase(match result {
            Ok(_) => S3ModelImportPhase::Completed,
            Err(S3ModelImportError::Operation(PumasError::DownloadCancelled)) => {
                S3ModelImportPhase::Cancelled
            }
            Err(_) => S3ModelImportPhase::Failed,
        });
        self.complete = true;
    }
}
impl Drop for ProgressGuard {
    fn drop(&mut self) {
        if !self.complete {
            self.control.0.gate.store(FINISHED, Ordering::SeqCst);
            self.control.phase(S3ModelImportPhase::Interrupted);
        }
    }
}
struct Host(S3ModelImportControl);
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        self.0.wait_for_cancel().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        self.0.is_cancelled()
    }
    async fn record_progress(&mut self, bytes: u64) -> crate::Result<()> {
        self.0
             .0
            .progress
            .send_modify(|progress| progress.downloaded_for_current_file = bytes);
        self.0
             .0
            .bundle_progress
            .send_modify(|progress| progress.downloaded_for_current_file = bytes);
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    fn file_started(&mut self, index: usize) {
        self.0
             .0
            .progress
            .send_modify(|progress| progress.downloaded_for_current_file = 0);
        self.0 .0.bundle_progress.send_modify(|progress| {
            progress.file_index = Some(index);
            progress.downloaded_for_current_file = 0;
        });
    }
    fn file_acquired(&mut self, index: usize, bytes: u64) {
        self.0 .0.bundle_progress.send_modify(|progress| {
            if progress.file_index == Some(index) {
                // Selection's shared manifest validator rejects total overflow;
                // acquired bytes must match those selected sizes.
                progress.bytes_acquired += bytes;
                progress.files_acquired += 1;
                progress.file_index = None;
                progress.downloaded_for_current_file = 0;
            }
        });
    }
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> crate::Result<()> {
        Ok(())
    }
}

impl PumasApi {
    /// Resolve explicit immutable S3 pins, acquire/verify the complete set, and
    /// publish a package qualified by the shared model importer. No prefix
    /// discovery, account setup, credential storage, implicit workspace or RPC is
    /// created. Success follows the existing consumer receipt's durable settlement.
    ///
    /// Cancellation is accepted during selection/transfer/verification. Finalization
    /// closes that admission before receipt issuance and runs to an owned result.
    /// Cancellation or error can retain staged/Using work; callers must use existing
    /// recovery custody. Dropping the waiter is interruption, not completion proof;
    /// shared acquisition shutdown drains its registered effects.
    pub async fn import_s3_model(
        &self,
        request: S3ModelImportRequest,
        control: S3ModelImportControl,
    ) -> std::result::Result<ModelImportResult, S3ModelImportError> {
        self.import_s3_model_mode(request, None, control).await
    }

    /// Acquire a digest-bound conditional single object and qualify it through
    /// the same shared importer, cancellation gate and durable publication path.
    /// Missing or HTTP weak (W/) validators and incomplete model packages fail.
    /// Mutable provenance remains Weak despite a strong HTTP ETag; the mandatory
    /// whole-file digest supplies content integrity. VersionId
    /// callers retain their immutable selection contract.
    pub async fn import_s3_conditional_model(
        &self,
        request: S3ConditionalModelImportRequest,
        control: S3ModelImportControl,
    ) -> std::result::Result<ModelImportResult, S3ModelImportError> {
        let conditional = ConditionalObject {
            source_key: request.source_key,
            expected_sha256: request.expected_sha256,
        };
        let request = S3ModelImportRequest {
            operation_id: request.operation_id,
            source: request.source,
            credentials: request.credentials,
            entries: Vec::new(),
            import: request.import,
            workspace: request.workspace,
            retry: request.retry,
        };
        self.import_s3_model_mode(request, Some(conditional), control)
            .await
    }

    async fn import_s3_model_mode(
        &self,
        request: S3ModelImportRequest,
        conditional: Option<ConditionalObject>,
        control: S3ModelImportControl,
    ) -> std::result::Result<ModelImportResult, S3ModelImportError> {
        match control
            .0
            .gate
            .compare_exchange(PENDING, ACTIVE, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => {}
            Err(CANCELLED) => return Err(PumasError::DownloadCancelled.into()),
            Err(_) => {
                return Err(PumasError::Validation {
                    field: "s3.model.control".into(),
                    message: "S3 model control already belongs to an operation".into(),
                }
                .into())
            }
        }
        let mut progress = ProgressGuard {
            control: control.clone(),
            complete: false,
        };
        let result = self
            .import_s3_model_owned(request, conditional, control)
            .await;
        progress.finish(&result);
        result
    }

    async fn import_s3_model_owned(
        &self,
        request: S3ModelImportRequest,
        conditional: Option<ConditionalObject>,
        control: S3ModelImportControl,
    ) -> std::result::Result<ModelImportResult, S3ModelImportError> {
        let paths = if conditional.is_some() {
            vec![request.import.path.as_str()]
        } else {
            request
                .entries
                .iter()
                .map(|entry| entry.logical_path.as_str())
                .collect()
        };
        ModelImporter::validate_acquired_payload_paths(&paths)?;
        let consumer = self.acquisition().open_consumer(CONSUMER)?;
        let result = async {
            if conditional.is_none()
                && !request
                    .entries
                    .iter()
                    .any(|entry| entry.logical_path == request.import.path)
            {
                return Err(PumasError::Validation {
                    field: "s3.model.primary".into(),
                    message:
                        "The import spec must name the exact selected primary weight logical path"
                            .into(),
                }
                .into());
            }
            let reader = match request.credentials {
                Some(credentials) => S3Reader::new_authenticated(request.source, credentials)?,
                None => S3Reader::new(request.source)?,
            };
            let demand = AcquisitionDemand {
                consumer: CONSUMER.into(),
                operation: request.operation_id.to_string(),
            };
            control.phase(S3ModelImportPhase::Selecting);
            let selection = match conditional {
                Some(object) => {
                    consumer
                        .resolve_s3_conditional(
                            reader,
                            (
                                object.source_key,
                                request.import.path.clone(),
                                object.expected_sha256,
                            ),
                            &demand,
                            &request.retry,
                            Box::new(Host(control.clone())),
                        )
                        .await??
                }
                None => {
                    consumer
                        .resolve_s3_manifest(
                            reader,
                            request.entries,
                            &demand,
                            &request.retry,
                            Box::new(Host(control.clone())),
                        )
                        .await??
                }
            };
            if control.is_cancelled() {
                return Err(PumasError::DownloadCancelled.into());
            }
            control.0.bundle_progress.send_modify(|progress| {
                progress.files_total = selection.manifest().files().len();
                progress.total_expected_bytes = selection
                    .manifest()
                    .files()
                    .iter()
                    .try_fold(0_u64, |sum, file| sum.checked_add(file.expected_size()?));
            });
            control.phase(S3ModelImportPhase::Acquiring);
            let spec = request.import;
            let prepared_spec = spec.clone();
            let importer = self.primary().model_importer.clone();
            consumer
                .acquire_s3_manifest(
                    AcquisitionS3ManifestRequest {
                        demand,
                        selection,
                        workspace: request.workspace,
                        retry: request.retry,
                    },
                    Box::new(Host(control.clone())),
                    move |acquired| async move {
                        control.begin_finalization()?;
                        Ok((acquired, serde_json::to_value(prepared_spec)?))
                    },
                    move |acquired, receipt| async move {
                        importer
                            .import_acquired_model(&acquired, &receipt, &spec)
                            .await
                    },
                )
                .await
                .map_err(S3ModelImportError::Operation)
        }
        .await;
        match consumer.shutdown().await {
            Ok(()) => result,
            Err(source) => {
                let (published_result, operation_error) = match result {
                    Ok(value) => (Some(value), None),
                    Err(error) => (None, Some(Box::new(error))),
                };
                Err(S3ModelImportError::Drain {
                    source,
                    published_result,
                    operation_error,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bundle_progress_separates_acquired_files_current_attempt_and_publication() {
        let control = S3ModelImportControl::new();
        control.0.gate.store(ACTIVE, Ordering::SeqCst);
        control.0.bundle_progress.send_modify(|p| {
            p.files_total = 3;
            p.total_expected_bytes = Some(12);
        });
        let bundle = control.subscribe_bundle();
        let legacy = control.subscribe();
        let mut host = Host(control.clone());
        control.phase(S3ModelImportPhase::Acquiring);
        host.file_started(0);
        host.record_progress(5).await.unwrap();
        assert_eq!(bundle.borrow().downloaded_for_current_file, 5);
        host.file_acquired(0, 5);
        host.file_started(1);
        host.record_progress(2).await.unwrap();
        assert_eq!(bundle.borrow().bytes_acquired, 5);
        assert_eq!(bundle.borrow().files_acquired, 1);
        assert_eq!(bundle.borrow().file_index, Some(1));
        // Retrying/current bytes may go backwards; never counted twice.
        host.record_progress(1).await.unwrap();
        assert_eq!(bundle.borrow().bytes_acquired, 5);
        host.file_acquired(1, 7);
        host.file_started(2);
        host.file_acquired(2, 0);
        assert_eq!(bundle.borrow().bytes_acquired, 12);
        assert_eq!(bundle.borrow().files_acquired, 3);
        assert_eq!(bundle.borrow().file_index, None);
        assert_eq!(legacy.borrow().downloaded_for_current_file, 0);
        assert_eq!(bundle.borrow().phase, S3ModelImportPhase::Acquiring);
        assert!(control.cancel());
        assert_eq!(bundle.borrow().phase, S3ModelImportPhase::Cancelling);
        control.phase(S3ModelImportPhase::Acquiring);
        assert_eq!(bundle.borrow().phase, S3ModelImportPhase::Cancelling);
    }
    #[test]
    fn cancellation_and_finalization_have_one_winner_and_stale_progress_is_ignored() {
        let control = S3ModelImportControl::new();
        let progress = control.subscribe();
        control.0.gate.store(ACTIVE, Ordering::SeqCst);
        assert!(control.cancel());
        assert!(matches!(
            control.begin_finalization(),
            Err(PumasError::DownloadCancelled)
        ));
        control.phase(S3ModelImportPhase::Acquiring);
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelling);
        control.0.gate.store(FINISHED, Ordering::SeqCst);
        control.phase(S3ModelImportPhase::Cancelled);
        control.phase(S3ModelImportPhase::Cancelling);
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelled);

        let control = S3ModelImportControl::new();
        let progress = control.subscribe();
        control.0.gate.store(ACTIVE, Ordering::SeqCst);
        control.begin_finalization().unwrap();
        assert!(!control.cancel());
        control.phase(S3ModelImportPhase::Selecting);
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Finalizing);
    }
}
