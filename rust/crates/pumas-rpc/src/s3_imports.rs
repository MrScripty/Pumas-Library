//! One bounded RPC observation/awaiting worker. Core owns acquisition/publication.
use crate::{contract::*, server::AppState};
use pumas_library::{
    acquisition::{
        AcquisitionConsumer, AcquisitionPhase, AcquisitionRecord, AcquisitionRetryPolicy,
        AcquisitionService, ReservedDirectory, S3Addressing, S3ManifestEntry, S3ReaderConfig,
        Sha256Evidence,
    },
    models::ModelImportSpec,
    network::RetryConfig,
    PumasError, Result, S3ModelImportControl, S3ModelImportError, S3ModelImportPhase,
    S3ModelImportRequest,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
pub(crate) struct ImportJob {
    request: S3ImportParams,
    entries: Vec<S3ManifestEntry>,
    credentials: Option<pumas_library::acquisition::S3Credentials>,
    control: S3ModelImportControl,
    retry: Option<Box<RetainedTransfer>>,
    attempt: Arc<()>,
}
#[derive(Clone)]
struct RetainedTransfer {
    request: S3ImportParams,
    entries: Vec<S3ManifestEntry>,
    reservation: ReservedDirectory,
    record: AcquisitionRecord,
    authenticated: bool,
}
pub(crate) enum Job {
    Import(ImportJob),
    Discovery(discovery::DiscoveryJob),
}
#[path = "s3_imports/discovery.rs"]
mod discovery;
struct Current {
    id: String,
    control: S3ModelImportControl,
    progress: watch::Receiver<pumas_library::S3ModelImportProgress>,
    bundle_progress: watch::Receiver<pumas_library::S3ModelBundleProgress>,
    result: Option<S3ImportResultWire>,
    retained: Option<RetainedTransfer>,
    attempt: Arc<()>,
}
struct Inner {
    sender: Option<mpsc::Sender<Job>>,
    current: Option<Current>,
    discovery: Option<discovery::CurrentDiscovery>,
}
#[derive(Clone)]
pub(crate) struct S3Imports(Arc<Mutex<Inner>>);
pub(crate) struct S3ImportWorker {
    client: S3Imports,
    completion: Option<JoinHandle<()>>,
}
fn unavailable() -> PumasError {
    PumasError::Config {
        message: "S3 import observer is unavailable".into(),
    }
}
impl S3Imports {
    pub(crate) fn channel() -> (Self, mpsc::Receiver<Job>) {
        let (sender, receiver) = mpsc::channel(1);
        (
            Self(Arc::new(Mutex::new(Inner {
                sender: Some(sender),
                current: None,
                discovery: None,
            }))),
            receiver,
        )
    }
    #[cfg(test)]
    pub(crate) fn unavailable() -> Self {
        Self(Arc::new(Mutex::new(Inner {
            sender: None,
            current: None,
            discovery: None,
        })))
    }
    pub(crate) fn admit(&self, request: S3ImportParams) -> Result<S3ImportOutcome> {
        if request.validate().is_err()
            || pumas_library::acquisition::S3Reader::new(source_config(&request)).is_err()
        {
            return Ok(S3ImportOutcome::Rejected {
                error: PublicError::invalid_params(),
            });
        }
        self.admit_ready(request, None, None)
    }
    pub(crate) fn admit_authenticated(
        &self,
        request: S3AuthenticatedImportParams,
    ) -> Result<S3ImportOutcome> {
        if request.validate().is_err()
            || request
                .credentials
                .preflight(source_config(&request.source))
                .is_err()
        {
            return Ok(S3ImportOutcome::Rejected {
                error: PublicError::invalid_params(),
            });
        }
        let credentials = request
            .credentials
            .into_native()
            .map_err(|_| unavailable())?;
        self.admit_ready(request.source, Some(credentials), None)
    }
    pub(crate) fn admit_bundle(
        &self,
        request: S3BundleImportParams,
        credentials: Option<S3CredentialParams>,
    ) -> Result<S3ImportOutcome> {
        // Auth configuration must be checked before originals are consumed.
        if let Some(credentials) = &credentials {
            let primary = match request.primary() {
                Ok(value) => value,
                Err(_) => {
                    return Ok(S3ImportOutcome::Rejected {
                        error: PublicError::invalid_params(),
                    })
                }
            };
            if credentials.preflight(source_config(&primary)).is_err() {
                return Ok(S3ImportOutcome::Rejected {
                    error: PublicError::invalid_params(),
                });
            }
        }
        let preflight = || -> std::result::Result<_, PublicError> {
            request.validate()?;
            let primary = request.primary()?;
            let entries = request.native_entries()?;
            // Construction is in-memory; both anonymous and authenticated paths
            // validate the exact native reader and complete set before admission.
            let native_credentials = credentials
                .map(S3CredentialParams::into_native)
                .transpose()?;
            let config = source_config(&primary);
            // Validate with a separate anonymous reader: pure object/namespace
            // checks do not depend on credentials. Auth constructor preflight
            // remains separate to avoid sharing or cloning its capability.
            let reader = pumas_library::acquisition::S3Reader::new(config)
                .map_err(|_| PublicError::invalid_params())?;
            reader
                .validate_manifest_entries(&entries)
                .map_err(|_| PublicError::invalid_params())?;
            Ok((primary, entries, native_credentials))
        };
        match preflight() {
            Ok((primary, entries, credentials)) => {
                self.admit_ready(primary, credentials, Some(entries))
            }
            Err(_) => Ok(S3ImportOutcome::Rejected {
                error: PublicError::invalid_params(),
            }),
        }
    }
    pub(crate) fn bundle_snapshot(&self, id: Option<&str>) -> Result<S3BundleImportObservation> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        let outcome = snapshot_inner(&state, id);
        let bundle_progress = if matches!(outcome, S3ImportOutcome::Running { .. }) {
            state.current.as_ref().map(|current| {
                let p = *current.bundle_progress.borrow();
                S3BundleProgressWire {
                    file_index: p.file_index.and_then(|i| u32::try_from(i).ok()),
                    files_total: p.files_total as u32,
                    files_acquired: p.files_acquired as u32,
                    bytes_acquired: p.bytes_acquired.to_string(),
                    total_expected_bytes: p.total_expected_bytes.map(|b| b.to_string()),
                    total_bytes_observed: (p.bytes_acquired + p.downloaded_for_current_file)
                        .to_string(),
                }
            })
        } else {
            None
        };
        Ok(S3BundleImportObservation {
            outcome,
            bundle_progress,
        })
    }
    fn admit_ready(
        &self,
        request: S3ImportParams,
        credentials: Option<pumas_library::acquisition::S3Credentials>,
        entries: Option<Vec<S3ManifestEntry>>,
    ) -> Result<S3ImportOutcome> {
        let mut state = self.0.lock().map_err(|_| unavailable())?;
        let Some(sender) = state.sender.as_ref() else {
            return Ok(S3ImportOutcome::Unavailable);
        };
        if sender.is_closed() {
            return Ok(S3ImportOutcome::Unavailable);
        }
        if state.discovery.as_ref().is_some_and(|d| d.result.is_none())
            || state.current.as_ref().is_some_and(|old| {
                old.result.is_none()
                    || old.id == request.operation_id
                    || matches!(
                        old.result,
                        Some(S3ImportResultWire::Failed {
                            retained_work: true,
                            ..
                        }) | Some(S3ImportResultWire::Cancelled {
                            retained_work: true
                        })
                    )
            })
        {
            return Ok(S3ImportOutcome::Rejected {
                error: s3_conflict(),
            });
        }
        let entries = match entries {
            Some(entries) => entries,
            None => vec![S3ManifestEntry {
                source_key: request.key.clone(),
                version: request.version_id.clone(),
                logical_path: request.filename.clone(),
                expected_sha256: Sha256Evidence::new("caller.sha256", request.sha256.clone())
                    .map_err(|_| unavailable())?,
            }],
        };
        let control = S3ModelImportControl::new();
        let attempt = Arc::new(());
        let current = Current {
            id: request.operation_id.clone(),
            control: control.clone(),
            progress: control.subscribe(),
            bundle_progress: control.subscribe_bundle(),
            result: None,
            retained: None,
            attempt: attempt.clone(),
        };
        sender
            .try_send(Job::Import(ImportJob {
                request,
                entries,
                credentials,
                control,
                retry: None,
                attempt,
            }))
            .map_err(|_| unavailable())?;
        state.current = Some(current);
        Ok(snapshot_inner(&state, None))
    }
    pub(crate) fn retry_state(&self, id: Option<&str>) -> Result<S3TransferRetryState> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        let reason = if state.sender.as_ref().is_none_or(mpsc::Sender::is_closed) {
            S3TransferRetryReason::NoLiveCustody
        } else if state.discovery.as_ref().is_some_and(|d| d.result.is_none())
            || state.current.as_ref().is_some_and(|c| c.result.is_none())
        {
            S3TransferRetryReason::Busy
        } else if let Some(current) = state
            .current
            .as_ref()
            .filter(|c| id.is_none_or(|id| id == c.id))
        {
            if let Some(retained) = &current.retained {
                return Ok(S3TransferRetryState::Ready {
                    operation_id: current.id.clone(),
                    authentication_required: retained.authenticated,
                });
            }
            S3TransferRetryReason::NotRetryable
        } else {
            S3TransferRetryReason::NoLiveCustody
        };
        Ok(S3TransferRetryState::Unavailable { reason })
    }
    pub(crate) fn retry(&self, request: S3TransferRetryParams) -> Result<S3ImportOutcome> {
        if request.validate().is_err() {
            return Ok(S3ImportOutcome::Rejected {
                error: PublicError::invalid_params(),
            });
        }
        let mut state = self.0.lock().map_err(|_| unavailable())?;
        let Some(sender) = state.sender.as_ref().filter(|s| !s.is_closed()) else {
            return Ok(S3ImportOutcome::Unavailable);
        };
        if state.discovery.as_ref().is_some_and(|d| d.result.is_none()) {
            return Ok(S3ImportOutcome::Rejected {
                error: s3_conflict(),
            });
        }
        let Some(current) = state
            .current
            .as_ref()
            .filter(|c| c.id == request.operation_id && c.result.is_some())
        else {
            return Ok(S3ImportOutcome::Rejected {
                error: s3_conflict(),
            });
        };
        let Some(retained) = current.retained.clone() else {
            return Ok(S3ImportOutcome::Rejected {
                error: s3_conflict(),
            });
        };
        // Access material is fresh and consumed by this attempt, never retained.
        if retained.authenticated != request.credentials.is_some() {
            return Ok(S3ImportOutcome::Rejected {
                error: PublicError::invalid_params(),
            });
        }
        let credentials = match request.credentials {
            Some(credentials) => {
                if credentials
                    .preflight(source_config(&retained.request))
                    .is_err()
                {
                    return Ok(S3ImportOutcome::Rejected {
                        error: PublicError::invalid_params(),
                    });
                }
                Some(credentials.into_native().map_err(|_| unavailable())?)
            }
            None => None,
        };
        let control = S3ModelImportControl::new();
        let attempt = Arc::new(());
        let next = Current {
            id: current.id.clone(),
            progress: control.subscribe(),
            bundle_progress: control.subscribe_bundle(),
            control: control.clone(),
            result: None,
            retained: None,
            attempt: attempt.clone(),
        };
        sender
            .try_send(Job::Import(ImportJob {
                request: retained.request.clone(),
                entries: retained.entries.clone(),
                credentials,
                control,
                retry: Some(Box::new(retained)),
                attempt,
            }))
            .map_err(|_| unavailable())?;
        state.current = Some(next);
        Ok(snapshot_inner(&state, None))
    }
    pub(crate) fn snapshot(&self, id: Option<&str>) -> Result<S3ImportOutcome> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        Ok(snapshot_inner(&state, id))
    }
    pub(crate) fn cancel(&self, id: &str) -> Result<S3ImportCancelOutcome> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        let accepted = state
            .current
            .as_ref()
            .is_some_and(|job| job.id == id && job.result.is_none() && job.control.cancel());
        Ok(S3ImportCancelOutcome {
            accepted,
            outcome: snapshot_inner(&state, Some(id)),
        })
    }
    pub(crate) fn close(&self) {
        // Poisoned observations still refuse service; shutdown must nevertheless
        // revoke admission and wake cancellation so the owned worker can drain.
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        state.sender.take();
        if let Some(discovery) = &state.discovery {
            discovery.cancel.send_replace(true);
        }
        if let Some(current) = &state.current {
            current.control.cancel();
        }
    }
    fn finish(
        &self,
        id: &str,
        attempt: &Arc<()>,
        result: S3ImportResultWire,
        retained: Option<RetainedTransfer>,
    ) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(current) = state
                .current
                .as_mut()
                .filter(|current| current.id == id && Arc::ptr_eq(&current.attempt, attempt))
            {
                current.result = Some(result);
                current.retained = retained;
            }
        }
    }
}
fn snapshot_inner(state: &Inner, id: Option<&str>) -> S3ImportOutcome {
    if state.sender.as_ref().is_none_or(mpsc::Sender::is_closed) {
        return S3ImportOutcome::Unavailable;
    }
    let Some(current) = &state.current else {
        return match id {
            Some(id) => S3ImportOutcome::NotFound {
                operation_id: id.into(),
            },
            None => S3ImportOutcome::Idle,
        };
    };
    if id.is_some_and(|id| id != current.id) {
        return S3ImportOutcome::NotFound {
            operation_id: id.unwrap().into(),
        };
    }
    if let Some(result) = &current.result {
        S3ImportOutcome::Finished {
            operation_id: current.id.clone(),
            result: result.clone(),
        }
    } else {
        let progress = *current.progress.borrow();
        S3ImportOutcome::Running {
            operation_id: current.id.clone(),
            progress: S3ImportProgressWire {
                phase: phase(progress.phase),
                downloaded_for_current_file: progress.downloaded_for_current_file.to_string(),
            },
        }
    }
}
fn phase(phase: S3ModelImportPhase) -> S3ImportPhaseWire {
    match phase {
        S3ModelImportPhase::Pending => S3ImportPhaseWire::Pending,
        S3ModelImportPhase::Selecting => S3ImportPhaseWire::Selecting,
        S3ModelImportPhase::Acquiring => S3ImportPhaseWire::Acquiring,
        S3ModelImportPhase::Cancelling => S3ImportPhaseWire::Cancelling,
        S3ModelImportPhase::Finalizing => S3ImportPhaseWire::Finalizing,
        S3ModelImportPhase::Completed => S3ImportPhaseWire::Completed,
        S3ModelImportPhase::Cancelled => S3ImportPhaseWire::Cancelled,
        S3ModelImportPhase::Failed => S3ImportPhaseWire::Failed,
        S3ModelImportPhase::Interrupted => S3ImportPhaseWire::Interrupted,
    }
}
impl S3ImportWorker {
    pub(crate) fn start(state: Arc<AppState>, mut receiver: mpsc::Receiver<Job>) -> Self {
        let client = state.s3_imports.clone();
        let observer = client.clone();
        let completion = tokio::spawn(async move {
            while let Some(job) = receiver.recv().await {
                match job {
                    Job::Import(job) => {
                        let id = job.request.operation_id.clone();
                        let attempt = job.attempt.clone();
                        let (result, retained) = run(&state, job).await;
                        observer.finish(&id, &attempt, result, retained);
                    }
                    Job::Discovery(job) => {
                        let id = job.id.clone();
                        let result = discovery::run(job).await;
                        observer.finish_discovery(&id, result);
                    }
                }
            }
        });
        Self {
            client,
            completion: Some(completion),
        }
    }
    pub(crate) async fn shutdown(mut self) -> anyhow::Result<()> {
        self.client.close();
        self.completion
            .take()
            .expect("owned worker")
            .await
            .map_err(|_| anyhow::anyhow!("S3 RPC import worker failed"))
    }
}
impl Drop for S3ImportWorker {
    fn drop(&mut self) {
        self.client.close();
    }
}
fn failure(
    error: PublicError,
    retained_work: bool,
    published_model_id: Option<String>,
) -> S3ImportResultWire {
    S3ImportResultWire::Failed {
        error,
        retained_work,
        published_model_id,
    }
}
async fn prepare_workspace(
    acquisition: &Arc<AcquisitionService>,
    retry: Option<RetainedTransfer>,
    prepare: impl FnOnce() -> Result<ReservedDirectory> + Send + 'static,
) -> std::result::Result<
    (AcquisitionConsumer, ReservedDirectory),
    Box<(S3ImportResultWire, Option<RetainedTransfer>)>,
> {
    let consumer = match acquisition.open_consumer("rpc.s3.workspace") {
        Ok(consumer) => consumer,
        Err(error) => {
            return Err(Box::new((
                failure(PublicError::from_pumas(&error), retry.is_some(), None),
                retry,
            )));
        }
    };
    // Keep effect results nested: only an outer capacity refusal proves that
    // the reservation/validation closure never ran. A same-typed error from
    // inside that closure must not restore custody after possible effects.
    let prepared = consumer
        .run_blocking("reserve or validate desktop S3 workspace", move || {
            Ok(prepare())
        })
        .await;
    let (error, unstarted) = match prepared {
        Ok(Ok(reservation)) => return Ok((consumer, reservation)),
        Ok(Err(error)) => (error, false),
        Err(error) => {
            let unstarted = matches!(error, PumasError::AcquisitionCapacityExhausted { .. });
            (error, unstarted)
        }
    };
    if consumer.shutdown().await.is_err() {
        return Err(Box::new((
            failure(PublicError::internal(), true, None),
            None,
        )));
    }
    if unstarted {
        Err(Box::new((
            failure(PublicError::from_pumas(&error), retry.is_some(), None),
            retry,
        )))
    } else {
        Err(Box::new((
            failure(PublicError::from_pumas(&error), true, None),
            None,
        )))
    }
}
async fn run(state: &AppState, job: ImportJob) -> (S3ImportResultWire, Option<RetainedTransfer>) {
    let ImportJob {
        request,
        entries,
        credentials,
        control,
        retry,
        attempt: _,
    } = job;
    let retry = retry.map(|retained| *retained);
    if matches!(
        control.subscribe().borrow().phase,
        S3ModelImportPhase::Cancelled
    ) {
        // An explicitly retried operation still owns its prior inputs.
        return (
            S3ImportResultWire::Cancelled {
                retained_work: retry.is_some(),
            },
            retry,
        );
    }
    let authenticated = credentials.is_some();
    let template = request.clone();
    let original_entries = entries.clone();
    let root = state.api.launcher_data_dir();
    let id = request.operation_id.clone();
    let store = state.api.acquisition().store().clone();
    let receipts = state.api.acquisition().clone();
    let models_root = state.api.model_library().library_root().to_owned();
    let previous = retry.as_ref().map(|r| r.record.clone());
    let prepared = prepare_workspace(state.api.acquisition(), retry.clone(), move || {
        let canonical_root = std::fs::canonicalize(&root)?;
        if canonical_root.starts_with(std::fs::canonicalize(&models_root)?) {
            return Err(PumasError::Validation {
                field: "s3.workspace_layout".into(),
                message: "S3 staging must remain outside model discovery".into(),
            });
        }
        let records = store.acquisitions()?;
        if let Some(retained) = retry {
            retained.reservation.validate()?;
            let current = records.get(&retained.record.id).ok_or_else(unavailable)?;
            if current != &retained.record
                || !matches!(
                    current.phase,
                    AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
                )
                || receipts.consumer_receipt(current.id)?.is_some()
                || records.values().any(|r| {
                    r.demand.consumer == "model.s3.workflow"
                        && matches!(r.phase, AcquisitionPhase::Using { .. })
                })
            {
                return Err(PumasError::Validation {
                    field: "s3.retained_work".into(),
                    message: "Retained transfer changed or requires publication reconciliation"
                        .into(),
                });
            }
            return Ok(retained.reservation);
        }
        if records.values().any(|r| {
            r.demand.consumer == "model.s3.workflow"
                && (r.demand.operation == id || matches!(r.phase, AcquisitionPhase::Using { .. }))
        }) {
            return Err(PumasError::Validation {
                field: "s3.retained_work".into(),
                message: "Existing S3 demand requires its exact result or reconciliation".into(),
            });
        }
        reserve(root, &id)
    })
    .await;
    let (consumer, reservation) = match prepared {
        Ok(value) => value,
        Err(result) => return *result,
    };
    let mut drained_operation = false;
    let result = match reservation
        .acquisition_workspace()
        .and_then(|workspace| make_request(request, workspace, credentials, entries))
    {
        Err(error) => failure(PublicError::from_pumas(&error), true, None),
        Ok(request) => match state.api.import_s3_model(request, control).await {
            Ok(result) => {
                let model_id = result.model_id;
                let cleanup = consumer
                    .run_blocking("clean settled desktop S3 input", move || {
                        reservation.clear_contents()?;
                        reservation.remove_empty()
                    })
                    .await;
                let result = match (result.success, model_id, cleanup) {
                    (true, Some(model_id), Ok(())) => S3ImportResultWire::Completed { model_id },
                    (_, model_id, _) => failure(PublicError::internal(), true, model_id),
                };
                let result = if consumer.shutdown().await.is_ok() {
                    result
                } else {
                    failure(
                        PublicError::internal(),
                        true,
                        match result {
                            S3ImportResultWire::Completed { model_id } => Some(model_id),
                            S3ImportResultWire::Failed {
                                published_model_id, ..
                            } => published_model_id,
                            _ => None,
                        },
                    )
                };
                return (result, None);
            }
            Err(S3ModelImportError::Operation(PumasError::DownloadCancelled)) => {
                drained_operation = true;
                S3ImportResultWire::Cancelled {
                    retained_work: true,
                }
            }
            Err(S3ModelImportError::Operation(error)) => {
                drained_operation = true;
                failure(PublicError::from_pumas(&error), true, None)
            }
            Err(S3ModelImportError::Source(_)) => {
                drained_operation = true;
                failure(PublicError::unavailable(), true, None)
            }
            Err(S3ModelImportError::Drain {
                published_result, ..
            }) => failure(
                PublicError::internal(),
                true,
                published_result.and_then(|r| r.model_id),
            ),
        },
    };
    let held = reservation.clone();
    let store = state.api.acquisition().store().clone();
    let receipts = state.api.acquisition().clone();
    let operation = template.operation_id.clone();
    let retained_record = if drained_operation {
        consumer
            .run_blocking("observe exact retained S3 transfer", move || {
                Ok((|| {
                    held.validate()?;
                    let record = store
                        .acquisitions()?
                        .into_values()
                        .find(|r| {
                            r.demand.consumer == "model.s3.workflow"
                                && r.demand.operation == operation
                        })
                        .ok_or_else(unavailable)?;
                    if record.workspace != *held.workspace_identity()
                        || !matches!(
                            record.phase,
                            AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
                        )
                        || receipts.consumer_receipt(record.id)?.is_some()
                        || previous.as_ref().is_some_and(|p| {
                            p.id != record.id
                                || p.demand != record.demand
                                || p.manifest != record.manifest
                                || p.workspace != record.workspace
                        })
                    {
                        return Err(unavailable());
                    }
                    Ok(record)
                })())
            })
            .await
            .ok()
            .and_then(std::result::Result::ok)
    } else {
        None
    };
    if consumer.shutdown().await.is_err() {
        return (failure(PublicError::internal(), true, None), None);
    }
    // Both scopes have drained before the original physical reservation becomes retryable.
    (
        result,
        retained_record.map(|record| RetainedTransfer {
            request: template,
            entries: original_entries,
            reservation,
            record,
            authenticated,
        }),
    )
}

fn make_request(
    request: S3ImportParams,
    workspace: pumas_library::acquisition::AcquisitionWorkspace,
    credentials: Option<pumas_library::acquisition::S3Credentials>,
    entries: Vec<S3ManifestEntry>,
) -> Result<S3ModelImportRequest> {
    let operation_id = request
        .operation_id
        .parse()
        .map_err(|_| PumasError::Validation {
            field: "s3.operation_id".into(),
            message: "Invalid S3 operation identity".into(),
        })?;
    Ok(S3ModelImportRequest {
        operation_id,
        source: source_config(&request),
        credentials,
        entries,
        import: ModelImportSpec {
            path: request.filename,
            family: request.family,
            official_name: request.official_name,
            model_type: None,
            repo_id: None,
            subtype: None,
            tags: None,
            security_acknowledged: None,
        },
        workspace,
        retry: AcquisitionRetryPolicy {
            attempts: Some(3),
            elapsed: Duration::from_secs(600),
            backoff: RetryConfig::default(),
        },
    })
}
fn reserve(root: PathBuf, id: &str) -> Result<ReservedDirectory> {
    use cap_std::fs::Dir;
    // All creation is relative to held parent authority, with no recursive
    // locator-based cleanup. Retained inputs survive failures/process exit.
    if !std::fs::symlink_metadata(&root)?.is_dir() {
        return Err(unavailable());
    }
    let parent = Arc::new(Dir::open_ambient_dir(&root, cap_std::ambient_authority())?);
    let parent_identity = same_file::Handle::from_file(parent.try_clone()?.into_std_file())?;
    let relative = PathBuf::from(format!(".s3-import-{id}"));
    parent.create_dir(&relative)?;
    let child = Arc::new(parent.open_dir(&relative)?);
    let child_identity = same_file::Handle::from_file(child.try_clone()?.into_std_file())?;
    let child_path = root.join(&relative);
    let validate_root = root.clone();
    ReservedDirectory::capture(
        &root,
        Path::new(&relative),
        Arc::new((parent, child)),
        move || {
            if std::fs::symlink_metadata(&validate_root)?
                .file_type()
                .is_symlink()
                || std::fs::symlink_metadata(&child_path)?
                    .file_type()
                    .is_symlink()
                || same_file::Handle::from_path(&validate_root)? != parent_identity
                || same_file::Handle::from_path(&child_path)? != child_identity
            {
                return Err(PumasError::Validation {
                    field: "s3.workspace".into(),
                    message: "S3 input reservation changed".into(),
                });
            }
            Ok(())
        },
    )
}

fn source_config(request: &S3ImportParams) -> S3ReaderConfig {
    S3ReaderConfig {
        endpoint: request.endpoint.clone(),
        region: request.region.clone(),
        bucket: request.bucket.clone(),
        addressing: match request.addressing {
            S3AddressingWire::Path => S3Addressing::Path,
            S3AddressingWire::VirtualHosted => S3Addressing::VirtualHosted,
        },
        allow_http: false,
        operation_timeout: Duration::from_secs(30),
    }
}

#[cfg(test)]
#[path = "s3_imports/tests.rs"]
mod tests;

#[cfg(all(test, target_os = "linux", not(feature = "inference-plugins")))]
#[path = "s3_imports/discovery_tests.rs"]
mod discovery_tests;

#[cfg(all(test, target_os = "linux", not(feature = "inference-plugins")))]
#[path = "s3_imports/retry_tests.rs"]
mod retry_tests;
