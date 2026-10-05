//! Explicit native S3-to-model composition. Source access is ephemeral; the
//! acquisition consumer and model importer retain custody and publication authority.
use crate::{
    acquisition::{
        AcquisitionDemand, AcquisitionHost, AcquisitionRetryPolicy, AcquisitionS3ManifestRequest,
        AcquisitionWorkspace, HttpAttemptHost, S3Credentials, S3ManifestEntry, S3Reader,
        S3ReaderConfig, S3ReaderError,
    },
    model_library::{ModelImportResult, ModelImportSpec},
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
    /// Exact logical primary GGUF path and model metadata, not a source URL.
    pub import: ModelImportSpec,
    pub workspace: AcquisitionWorkspace,
    pub retry: AcquisitionRetryPolicy,
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

struct ControlState {
    gate: AtomicU8,
    cancelled: Notify,
    progress: watch::Sender<S3ModelImportProgress>,
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
        Self(Arc::new(ControlState {
            gate: AtomicU8::new(PENDING),
            cancelled: Notify::new(),
            progress,
        }))
    }
    pub fn subscribe(&self) -> watch::Receiver<S3ModelImportProgress> {
        self.0.progress.subscribe()
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
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> crate::Result<()> {
        Ok(())
    }
}

impl PumasApi {
    /// Resolve explicit immutable S3 pins, acquire/verify the complete set, and
    /// publish one GGUF with optional selected data/text auxiliaries. No prefix
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
        let result = self.import_s3_model_owned(request, control).await;
        progress.finish(&result);
        result
    }

    async fn import_s3_model_owned(
        &self,
        request: S3ModelImportRequest,
        control: S3ModelImportControl,
    ) -> std::result::Result<ModelImportResult, S3ModelImportError> {
        let consumer = self.acquisition().open_consumer(CONSUMER)?;
        let result = async {
            if !request
                .entries
                .iter()
                .any(|entry| entry.logical_path == request.import.path)
                || !std::path::Path::new(&request.import.path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
            {
                return Err(PumasError::Validation {
                    field: "s3.model.primary".into(),
                    message:
                        "The import spec must name the exact selected primary GGUF logical path"
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
            let selection = consumer
                .resolve_s3_manifest(
                    reader,
                    request.entries,
                    &demand,
                    &request.retry,
                    Box::new(Host(control.clone())),
                )
                .await??;
            if control.is_cancelled() {
                return Err(PumasError::DownloadCancelled.into());
            }
            control.phase(S3ModelImportPhase::Acquiring);
            let bundle = selection.manifest().files().len() > 1;
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
                        if bundle {
                            importer
                                .import_acquired_gguf_bundle(&acquired, &receipt, &spec)
                                .await
                        } else {
                            importer
                                .import_acquired_gguf(&acquired, &receipt, &spec)
                                .await
                        }
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
