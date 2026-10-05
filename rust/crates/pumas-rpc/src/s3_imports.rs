//! One bounded RPC observation/awaiting worker. Core owns acquisition/publication.
use crate::{contract::*, server::AppState};
use pumas_library::{
    acquisition::{
        AcquisitionPhase, AcquisitionRetryPolicy, ReservedDirectory, S3Addressing, S3ManifestEntry,
        S3ReaderConfig, Sha256Evidence,
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
pub(crate) struct Job {
    request: S3ImportParams,
    control: S3ModelImportControl,
}
struct Current {
    id: String,
    control: S3ModelImportControl,
    progress: watch::Receiver<pumas_library::S3ModelImportProgress>,
    result: Option<S3ImportResultWire>,
}
struct Inner {
    sender: Option<mpsc::Sender<Job>>,
    current: Option<Current>,
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
            }))),
            receiver,
        )
    }
    #[cfg(test)]
    pub(crate) fn unavailable() -> Self {
        Self(Arc::new(Mutex::new(Inner {
            sender: None,
            current: None,
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
        let mut state = self.0.lock().map_err(|_| unavailable())?;
        let Some(sender) = state.sender.as_ref() else {
            return Ok(S3ImportOutcome::Unavailable);
        };
        if sender.is_closed() {
            return Ok(S3ImportOutcome::Unavailable);
        }
        if state.current.as_ref().is_some_and(|old| {
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
        }) {
            return Ok(S3ImportOutcome::Rejected {
                error: s3_conflict(),
            });
        }
        let control = S3ModelImportControl::new();
        let current = Current {
            id: request.operation_id.clone(),
            control: control.clone(),
            progress: control.subscribe(),
            result: None,
        };
        sender
            .try_send(Job { request, control })
            .map_err(|_| unavailable())?;
        state.current = Some(current);
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
        if let Some(current) = &state.current {
            current.control.cancel();
        }
    }
    fn finish(&self, id: &str, result: S3ImportResultWire) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(current) = state.current.as_mut().filter(|current| current.id == id) {
                current.result = Some(result);
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
                let id = job.request.operation_id.clone();
                let result = run(&state, job).await;
                observer.finish(&id, result);
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
async fn run(state: &AppState, job: Job) -> S3ImportResultWire {
    if matches!(
        job.control.subscribe().borrow().phase,
        S3ModelImportPhase::Cancelled
    ) {
        return S3ImportResultWire::Cancelled {
            retained_work: false,
        };
    }
    let request = job.request;
    let consumer = match state.api.acquisition().open_consumer("rpc.s3.workspace") {
        Ok(consumer) => consumer,
        Err(error) => return failure(PublicError::from_pumas(&error), false, None),
    };
    let root = state.api.launcher_data_dir();
    let id = request.operation_id.clone();
    let store = state.api.acquisition().store().clone();
    let models_root = state.api.model_library().library_root().to_owned();
    let prepared = consumer
        .run_blocking("reserve desktop S3 workspace", move || {
            // Refuse implicit replay of old demands or uncertain publication. Source
            // and publication state stay authoritative in the existing store.
            let canonical_root = std::fs::canonicalize(&root)?;
            let canonical_models = std::fs::canonicalize(&models_root)?;
            if canonical_root.starts_with(canonical_models) {
                return Err(PumasError::Validation {
                    field: "s3.workspace_layout".into(),
                    message: "S3 staging must remain outside model discovery".into(),
                });
            }
            if store.acquisitions()?.values().any(|record| {
                record.demand.consumer == "model.s3.workflow"
                    && (record.demand.operation == id
                        || matches!(record.phase, AcquisitionPhase::Using { .. }))
            }) {
                return Err(PumasError::Validation {
                    field: "s3.retained_work".into(),
                    message: "Existing S3 demand requires its exact result or reconciliation"
                        .into(),
                });
            }
            reserve(root, &id)
        })
        .await;
    let result = async {
        match prepared {
            Err(error) => failure(PublicError::from_pumas(&error), true, None),
            Ok(reservation) => {
                let workspace = match reservation.acquisition_workspace() {
                    Ok(workspace) => workspace,
                    Err(error) => return failure(PublicError::from_pumas(&error), true, None),
                };
                let request = make_request(request, workspace);
                match request {
                    Err(error) => failure(PublicError::from_pumas(&error), true, None),
                    Ok(request) => match state.api.import_s3_model(request, job.control).await {
                        Ok(result) => {
                            let model_id = result.model_id;
                            let cleanup = consumer
                                .run_blocking("clean settled desktop S3 input", move || {
                                    reservation.clear_contents()?;
                                    reservation.remove_empty()
                                })
                                .await;
                            match (result.success, model_id, cleanup) {
                                (true, Some(model_id), Ok(())) => {
                                    S3ImportResultWire::Completed { model_id }
                                }
                                (_, model_id, _) => {
                                    failure(PublicError::internal(), true, model_id)
                                }
                            }
                        }
                        Err(S3ModelImportError::Operation(PumasError::DownloadCancelled)) => {
                            S3ImportResultWire::Cancelled {
                                retained_work: true,
                            }
                        }
                        Err(S3ModelImportError::Source(_)) => {
                            failure(PublicError::unavailable(), true, None)
                        }
                        Err(S3ModelImportError::Operation(error)) => {
                            failure(PublicError::from_pumas(&error), true, None)
                        }
                        Err(S3ModelImportError::Drain {
                            published_result, ..
                        }) => failure(
                            PublicError::internal(),
                            true,
                            published_result.and_then(|result| result.model_id),
                        ),
                    },
                }
            }
        }
    }
    .await;
    match consumer.shutdown().await {
        Ok(()) => result,
        Err(_) => {
            let id = match result {
                S3ImportResultWire::Completed { model_id } => Some(model_id),
                S3ImportResultWire::Failed {
                    published_model_id, ..
                } => published_model_id,
                _ => None,
            };
            failure(PublicError::internal(), true, id)
        }
    }
}
fn make_request(
    request: S3ImportParams,
    workspace: pumas_library::acquisition::AcquisitionWorkspace,
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
        credentials: None,
        entries: vec![S3ManifestEntry {
            source_key: request.key,
            version: request.version_id,
            logical_path: request.filename.clone(),
            expected_sha256: Sha256Evidence::new("caller.sha256", request.sha256).map_err(
                |_| PumasError::Validation {
                    field: "s3.sha256".into(),
                    message: "Invalid expected S3 digest".into(),
                },
            )?,
        }],
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
