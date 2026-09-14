//! HuggingFace methods on PumasApi.

use crate::error::{PumasError, Result};
use crate::model_library;
use crate::model_library::artifact_identity::DownloadRevision;
use crate::models;
use crate::PumasApi;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::fs;
use tracing::{info, warn};

/// Canonical immutable download plan produced before destination mutation.
#[derive(Clone)]
pub(crate) struct PreparedIntentDownload {
    request: model_library::DownloadRequest,
    revision: DownloadRevision,
    evidence: Option<models::HuggingFaceEvidence>,
    model_type: String,
    architecture_family: String,
    selected_artifact: model_library::SelectedArtifactIdentity,
    model_ref: models::PumasModelRef,
}

impl PreparedIntentDownload {
    pub(crate) fn model_ref(&self) -> &models::PumasModelRef {
        &self.model_ref
    }

    pub(crate) fn repository_id(&self) -> &str {
        &self.request.repo_id
    }

    pub(crate) fn revision(&self) -> &str {
        self.revision.as_str()
    }

    pub(crate) fn filename(&self) -> Option<&str> {
        self.request.filename.as_deref()
    }

    pub(crate) fn format(&self) -> Option<models::PackageArtifactKind> {
        self.request
            .filename
            .as_deref()
            .and_then(package_artifact_kind_for_filename)
    }

    pub(crate) fn selected_artifact_id(&self) -> &str {
        &self.selected_artifact.artifact_id
    }
}

async fn start_recovered_download(
    client: &model_library::HuggingFaceClient,
    repo_id: &str,
    dest: &std::path::Path,
    model_type: Option<String>,
    filenames: Option<Vec<String>>,
) -> Result<String> {
    let (family, official_name) = repo_id.split_once('/').ok_or_else(|| PumasError::Config {
        message: "Invalid repo_id format (expected 'owner/name')".to_string(),
    })?;
    let request = model_library::DownloadRequest {
        repo_id: repo_id.to_string(),
        family: family.to_string(),
        official_name: official_name.to_string(),
        model_type,
        quant: None,
        filename: None,
        filenames,
        pipeline_tag: None,
        bundle_format: None,
        pipeline_class: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    };
    client.start_download(&request, dest, None).await
}

async fn load_hf_model_snapshot(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
    model_id: String,
) -> Result<(Option<models::ModelMetadata>, Option<std::path::PathBuf>)> {
    tokio::task::spawn_blocking(move || {
        let metadata = library.load_metadata(&model_dir)?;
        let primary_file = library.get_primary_model_file(&model_id);
        Ok((metadata, primary_file))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join HF model snapshot task: {}", err)))?
}

async fn load_model_metadata_or_default(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
) -> Result<models::ModelMetadata> {
    tokio::task::spawn_blocking(move || Ok(library.load_metadata(&model_dir)?.unwrap_or_default()))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HF metadata refresh load task: {}",
                err
            ))
        })?
}

async fn canonicalize_local_lookup_path(value: &str, field: &str) -> Result<std::path::PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PumasError::InvalidParams {
            message: format!("{field} is required"),
        });
    }

    let path = std::path::PathBuf::from(trimmed);
    fs::canonicalize(&path)
        .await
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => PumasError::InvalidParams {
                message: format!("{field} path not found: {}", path.display()),
            },
            _ => PumasError::io_with_path(source, &path),
        })
}

pub(crate) async fn validate_existing_local_file_lookup_path(
    value: &str,
    field: &str,
) -> Result<std::path::PathBuf> {
    let path = canonicalize_local_lookup_path(value, field).await?;
    let metadata = fs::metadata(&path)
        .await
        .map_err(|source| PumasError::io_with_path(source, &path))?;

    if metadata.is_file() {
        Ok(path)
    } else {
        Err(PumasError::InvalidParams {
            message: format!("{field} must reference a file: {}", path.display()),
        })
    }
}

pub(crate) async fn validate_existing_local_directory_lookup_path(
    value: &str,
    field: &str,
) -> Result<std::path::PathBuf> {
    let path = canonicalize_local_lookup_path(value, field).await?;
    let metadata = fs::metadata(&path)
        .await
        .map_err(|source| PumasError::io_with_path(source, &path))?;

    if metadata.is_dir() {
        Ok(path)
    } else {
        Err(PumasError::InvalidParams {
            message: format!("{field} must reference a directory: {}", path.display()),
        })
    }
}

impl PumasApi {
    // ========================================
    // HuggingFace Methods
    // ========================================

    /// Permanently close download admission and observe owned work through shutdown.
    ///
    /// Repeated callers receive the same result; cancelling a waiter does not
    /// cancel drainage. Recovery data is preserved. This does not shut down
    /// search, unrelated import operations, inference plugins, or the application's runtime.
    /// Import work owned by managed downloads is included in their drain.
    /// An API without an HF client has no download work to drain.
    pub async fn shutdown_downloads(&self) -> Result<()> {
        match &self.primary().hf_client {
            Some(client) => client.shutdown_downloads().await,
            None => Ok(()),
        }
    }

    /// Search for models on HuggingFace.
    ///
    /// Uses intelligent caching to minimize API calls:
    /// - Cached results are returned immediately if fresh (< 24 hours)
    /// - Model details including download sizes are enriched from cache
    /// - Falls back to API when cache is stale or missing
    pub async fn search_hf_models(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        self.search_hf_models_with_hydration(query, kind, limit, limit)
            .await
    }

    /// Search for models on HuggingFace with a bounded network hydration budget.
    pub async fn search_hf_models_with_hydration(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
        hydrate_limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        super::state_hf::search_hf_models_with_hydration(
            self.primary(),
            query,
            kind,
            limit,
            hydrate_limit,
        )
        .await
    }

    /// Get exact download details for a single HuggingFace repository.
    pub async fn get_hf_download_details(
        &self,
        repo_id: &str,
        quants: &[String],
    ) -> Result<models::HfDownloadDetails> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_download_details(repo_id, quants).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        let primary = self.primary().clone();
        let client = primary
            .hf_client
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })?;
        Self::start_hf_download_owned(primary.model_library.clone(), client, request).await
    }

    pub(super) async fn start_hf_download_owned(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        Self::start_hf_download_owned_at_revision(
            library,
            client,
            request,
            DownloadRevision::legacy_main(),
        )
        .await
    }

    pub(crate) async fn prepare_hf_download_owned_at_revision(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        request: &model_library::DownloadRequest,
        revision: DownloadRevision,
    ) -> Result<PreparedIntentDownload> {
        let invocation_client = client.clone();
        let request = request.clone();
        client
            .run_download_invocation(move |context| async move {
                let client = invocation_client;
                let result = async {
                    let mut resolved_request = request.clone();
                    let mut resolved_pipeline_tag =
                        normalized_download_hint(resolved_request.pipeline_tag.as_deref())
                            .map(ToOwned::to_owned);
                    let mut remote_model = None;
                    let metadata_client = client.clone();
                    let metadata_repo = request.repo_id.clone();
                    let metadata_revision = revision.clone();
                    // Optional metadata refusal remains a policy outcome, while its
                    // async helper and cache effects stay owned through completion.
                    let snapshot = context
                        .run_fallible_async_named("capture download metadata", move || async move {
                            Ok::<_, PumasError>(
                                metadata_client
                                    .get_model_snapshot_at_revision(
                                        &metadata_repo,
                                        &metadata_revision,
                                    )
                                    .await,
                            )
                        })
                        .await
                        .map_err(|error| {
                            PumasError::Other(format!(
                                "Download metadata observation failed: {error}"
                            ))
                        })??;
                    let mut huggingface_evidence = match snapshot {
                        Ok((model, evidence)) => {
                            remote_model = Some(model);
                            Some(evidence)
                        }
                        Err(err) => {
                            if revision.as_persisted().is_some() {
                                return Err(err);
                            }
                            warn!(
                                "Failed to capture HF evidence for {} before download: {}",
                                request.repo_id, err
                            );
                            None
                        }
                    };
                    if let Some(remote_pipeline_tag) =
                        huggingface_evidence.as_ref().and_then(|evidence| {
                            normalized_download_hint(evidence.pipeline_tag.as_deref())
                        })
                    {
                        resolved_pipeline_tag = Some(remote_pipeline_tag.to_string());
                    }
                    let mut resolved_model_type = if let Some(ref evidence) = huggingface_evidence {
                        let index = library.index().clone();
                        let official_name = resolved_request.official_name.clone();
                        let pipeline_tag = resolved_pipeline_tag.clone();
                        let model_type = request.model_type.clone();
                        let evidence = evidence.clone();
                        let resolved = context
                            .run_fallible_blocking_named(
                                "resolve download model type evidence",
                                move || {
                                    model_library::resolve_model_type_from_huggingface_evidence(
                                        &index,
                                        Some(&official_name),
                                        pipeline_tag.as_deref(),
                                        model_type.as_deref(),
                                        Some(&evidence),
                                    )
                                },
                            )
                            .await
                            .map_err(|error| {
                                PumasError::Other(format!(
                                    "Download model type observation failed: {error}"
                                ))
                            })??;
                        (resolved.model_type != model_library::ModelType::Unknown)
                            .then(|| resolved.model_type.as_str().to_string())
                    } else {
                        None
                    };

                    if resolved_model_type.is_none() || resolved_pipeline_tag.is_none() {
                        // Fall back to repo metadata only when the request does not already
                        // carry enough information to place the download safely.
                        if remote_model.is_none() {
                            let metadata_client = client.clone();
                            let metadata_repo = request.repo_id.clone();
                            remote_model = Some(
                                context
                                    .run_fallible_async_named(
                                        "resolve download repository",
                                        move || async move {
                                            metadata_client.get_model_info(&metadata_repo).await
                                        },
                                    )
                                    .await
                                    .map_err(|error| {
                                        PumasError::Other(format!(
                                            "Download repository observation failed: {error}"
                                        ))
                                    })??,
                            );
                        }
                        let model_info =
                            remote_model.as_ref().expect("remote model must be present");
                        if resolved_pipeline_tag.is_none() {
                            resolved_pipeline_tag =
                                normalized_download_hint(Some(model_info.kind.as_str()))
                                    .map(ToOwned::to_owned);
                        }
                        if resolved_model_type.is_none() {
                            let index = library.index().clone();
                            let hints = vec![
                                normalized_download_hint(request.model_type.as_deref())
                                    .map(ToOwned::to_owned),
                                resolved_pipeline_tag.clone(),
                                normalized_download_hint(Some(model_info.kind.as_str()))
                                    .map(ToOwned::to_owned),
                            ];
                            resolved_model_type = context
                                .run_fallible_blocking_named(
                                    "resolve download model type hints",
                                    move || resolve_owned_model_type_hints(&index, hints),
                                )
                                .await
                                .map_err(|error| {
                                    PumasError::Other(format!(
                                        "Download model type observation failed: {error}"
                                    ))
                                })??;
                        }
                    }
                    if let Some(model_info) = remote_model.as_ref() {
                        apply_remote_model_metadata(&mut resolved_request, model_info);
                    } else if resolved_request.license_status.is_none() {
                        resolved_request.license_status = Some("license_unknown".to_string());
                    }

                    let should_check_bundle = resolved_model_type
                        .as_deref()
                        .is_none_or(|model_type| model_type == "diffusion")
                        || resolved_pipeline_tag.as_deref() == Some("text-to-image");
                    if should_check_bundle {
                        let metadata_client = client.clone();
                        let metadata_repo = request.repo_id.clone();
                        let bundle_revision = revision.clone();
                        let classification = context
                            .run_fallible_async_named(
                                "classify download repository",
                                move || async move {
                                    Ok::<_, PumasError>(
                                        metadata_client
                                            .classify_repo_bundle_at_revision(
                                                &metadata_repo,
                                                &bundle_revision,
                                            )
                                            .await,
                                    )
                                },
                            )
                            .await
                            .map_err(|error| {
                                PumasError::Other(format!(
                                    "Download classification observation failed: {error}"
                                ))
                            })??;
                        match classification {
                            Ok(Some(bundle)) => {
                                if resolved_request.filename.is_some()
                                    || resolved_request.filenames.is_some()
                                    || resolved_request.quant.is_some()
                                {
                                    info!(
                                    "HF repo {} classified as {:?}; forcing full bundle download",
                                    request.repo_id, bundle.bundle_format
                                );
                                }
                                resolved_request.filename = None;
                                resolved_request.filenames = None;
                                resolved_request.quant = None;
                                resolved_request.bundle_format = Some(bundle.bundle_format);
                                resolved_request.pipeline_class = Some(bundle.pipeline_class);
                                if resolved_pipeline_tag.is_none() {
                                    resolved_pipeline_tag = Some("text-to-image".to_string());
                                }
                                if resolved_model_type.is_none() {
                                    resolved_model_type = Some("diffusion".to_string());
                                }
                            }
                            Ok(None) => {}
                            Err(err) => {
                                if revision.as_persisted().is_some() {
                                    return Err(err);
                                }
                                warn!(
                                    "Failed to classify HF repo {} as a bundle: {}",
                                    request.repo_id, err
                                );
                            }
                        }
                    }

                    resolved_request.pipeline_tag = resolved_pipeline_tag;

                    // Determine destination directory.
                    let model_type = resolved_model_type.unwrap_or_else(|| "unknown".to_string());
                    let architecture_family = model_library::infer_architecture_family_for_download(
                        &resolved_request,
                        huggingface_evidence.as_ref(),
                    );
                    resolved_request.family = architecture_family.clone();
                    let selected_artifact =
                        model_library::SelectedArtifactIdentity::from_download_request_at_revision(
                            &resolved_request,
                            None,
                            &revision,
                        );
                    resolved_request.model_type = Some(model_type.clone());
                    if let Some(ref mut evidence) = huggingface_evidence {
                        evidence.requested_model_type = request.model_type.clone();
                        evidence.requested_pipeline_tag = request.pipeline_tag.clone();
                        evidence.requested_quant = request.quant.clone();
                    }
                    let model_id = library.build_artifact_model_id(
                        &model_type,
                        &architecture_family,
                        &selected_artifact.artifact_id,
                    );
                    let model_ref = models::PumasModelRef {
                        model_id,
                        revision: revision.as_persisted().map(ToOwned::to_owned),
                        selected_artifact_id: Some(selected_artifact.artifact_id.clone()),
                        ..Default::default()
                    };
                    Ok(PreparedIntentDownload {
                        request: resolved_request,
                        revision,
                        evidence: huggingface_evidence,
                        model_type,
                        architecture_family,
                        selected_artifact,
                        model_ref,
                    })
                }
                .await;
                owned_download_planning_result(result)
            })
            .await?
    }

    pub(crate) async fn start_hf_download_owned_at_revision(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        request: &model_library::DownloadRequest,
        revision: DownloadRevision,
    ) -> Result<String> {
        let prepared = Self::prepare_hf_download_owned_at_revision(
            library.clone(),
            client.clone(),
            request,
            revision,
        )
        .await?;
        Self::start_prepared_hf_download_owned(library, client, prepared, None).await
    }

    pub(crate) async fn start_prepared_hf_download_owned(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        prepared: PreparedIntentDownload,
        expected: Option<&models::PumasModelRef>,
    ) -> Result<String> {
        let expected = expected.cloned();
        let invocation_client = client.clone();
        client
            .run_download_invocation(move |context| async move {
                validate_prepared_download(&library, &prepared, expected.as_ref())?;
                let destination_type = prepared.model_type.clone();
                let architecture_family = prepared.architecture_family.clone();
                let artifact_id = prepared.selected_artifact.artifact_id.clone();
                let context = invocation_client.protect_download_mutation(&context).await?;
                let grant = context.held_root_execution_grant()?;
                let dest_dir = context
                    .run_fallible_blocking_named("prepare HF artifact destination", move || {
                        library.prepare_artifact_download_destination_under_grant(
                            &destination_type,
                            &architecture_family,
                            &artifact_id,
                            grant,
                        )
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!(
                            "Download destination preparation observation failed: {error}"
                        ))
                    })??;
                if prepared.model_type == "unknown" {
                    warn!(
                        "Download {} is starting with unknown model_type after HF metadata lookup; destination={}",
                        prepared.request.repo_id,
                        dest_dir.display()
                    );
                }
                invocation_client
                    .start_download_at_revision(
                        &prepared.request,
                        &dest_dir,
                        prepared.evidence,
                        prepared.revision,
                    )
                    .await
            })
            .await
    }

    /// Get download progress for a HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: &str,
    ) -> Result<Option<models::ModelDownloadProgress>> {
        super::state_hf::get_hf_download_progress(self.primary(), download_id).await
    }

    /// Cancel a HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: &str) -> Result<bool> {
        super::state_hf::cancel_hf_download(self.primary(), download_id).await
    }

    /// Pause a HuggingFace download, preserving the `.part` file for later resume.
    pub async fn pause_hf_download(&self, download_id: &str) -> Result<bool> {
        super::state_hf::pause_hf_download(self.primary(), download_id).await
    }

    /// Resume a paused or errored HuggingFace download.
    pub async fn resume_hf_download(&self, download_id: &str) -> Result<bool> {
        super::state_hf::resume_hf_download(self.primary(), download_id).await
    }

    /// List all HuggingFace downloads (active, paused, completed, etc.).
    pub async fn list_hf_downloads(&self) -> Result<Vec<models::ModelDownloadProgress>> {
        super::state_hf::list_hf_downloads(self.primary()).await
    }

    /// Snapshot all HuggingFace downloads with a monotonic cursor.
    pub async fn get_hf_download_snapshot(&self) -> models::ModelDownloadSnapshot {
        if let Some(ref client) = self.primary().hf_client {
            client.download_snapshot().await
        } else {
            models::ModelDownloadSnapshot {
                cursor: "download:0".to_string(),
                revision: 0,
                downloads: Vec::new(),
            }
        }
    }

    /// Subscribe to backend-owned HuggingFace download state updates.
    pub fn subscribe_hf_download_updates(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<models::ModelDownloadUpdateNotification>> {
        self.primary()
            .hf_client
            .as_ref()
            .map(|client| client.subscribe_download_updates())
    }

    /// Build a recovery notification for a download update cursor.
    pub async fn hf_download_notification_since(
        &self,
        cursor: Option<&str>,
    ) -> models::ModelDownloadUpdateNotification {
        let snapshot = self.get_hf_download_snapshot().await;
        if let Some(ref client) = self.primary().hf_client {
            client
                .download_notification_since(cursor, snapshot.clone())
                .unwrap_or(models::ModelDownloadUpdateNotification {
                    cursor: snapshot.cursor.clone(),
                    snapshot,
                    stale_cursor: false,
                    snapshot_required: false,
                })
        } else {
            models::ModelDownloadUpdateNotification {
                cursor: snapshot.cursor.clone(),
                snapshot,
                stale_cursor: cursor.is_some(),
                snapshot_required: true,
            }
        }
    }

    /// List directories with interrupted downloads (`.part` files) that have
    /// no download persistence entry and no metadata.
    ///
    /// These are downloads that lost their tracking state (e.g. due to crash).
    /// Use `recover_download()` with the correct repo_id to resume them.
    pub async fn list_interrupted_downloads(
        &self,
    ) -> Result<Vec<model_library::InterruptedDownload>> {
        super::state_hf::list_interrupted_downloads(self.primary()).await
    }

    /// Recover an interrupted download that lost its persistence state.
    ///
    /// Given the correct `repo_id` and the `dest_dir` path where the partial
    /// download exists, starts a new download targeting that directory. The
    /// download system handles `.part` file resume via HTTP Range headers and
    /// skips files that are already complete.
    pub async fn recover_download(&self, repo_id: &str, dest_dir: &str) -> Result<String> {
        let primary = self.primary().clone();
        let client = primary
            .hf_client
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })?;
        Self::recover_download_owned(primary.model_library.clone(), client, repo_id, dest_dir).await
    }

    pub(super) async fn recover_download_owned(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        repo_id: &str,
        dest_dir: &str,
    ) -> Result<String> {
        let operation_client = client.clone();
        let repo_id = repo_id.to_string();
        let dest_dir = dest_dir.to_string();
        client
            .run_download_invocation(move |context| async move {
                Self::recover_download_admitted(
                    library,
                    operation_client,
                    context,
                    repo_id,
                    dest_dir,
                )
                .await
            })
            .await
    }

    async fn recover_download_admitted(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        context: model_library::DownloadInvocationContext,
        repo_id: String,
        dest_dir: String,
    ) -> Result<String> {
        let dest = context
            .run_fallible_async_named("resolve recovery directory", move || async move {
                validate_existing_local_directory_lookup_path(&dest_dir, "dest_dir").await
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Recovery directory observation failed: {error}"))
            })??;

        // Determine model_type from directory path relative to library root
        let library_root = library.library_root();
        let model_type = dest
            .strip_prefix(library_root)
            .ok()
            .and_then(|rel| rel.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .map(String::from);

        let metadata_dest = dest.clone();
        let metadata = context
            .run_fallible_blocking_named("load recovery metadata", move || {
                Ok::<_, PumasError>(library.load_metadata(&metadata_dest)?.unwrap_or_default())
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Recovery metadata observation failed: {error}"))
            })??;
        let recovery_filenames = metadata
            .selected_artifact_files
            .clone()
            .filter(|files| !files.is_empty())
            .or_else(|| {
                metadata
                    .expected_files
                    .clone()
                    .filter(|files| !files.is_empty())
            });
        if recovery_filenames.is_some() {
            info!(
                "Recovering partial download for {} using artifact file metadata from {}",
                repo_id,
                dest.display()
            );
        }

        start_recovered_download(&client, &repo_id, &dest, model_type, recovery_filenames).await
    }

    /// Resume a partial download from a previously issued model-state ticket.
    /// - Resume an existing tracked paused/error download
    /// - Attach to an already active tracked download
    /// - Recover an orphan partial download
    ///
    /// The ticket binds the caller's observed recovery state; it is not a
    /// secret or an authentication credential. The core resolves the indexed
    /// model and repository itself and refuses changed recovery context.
    /// Callers provide no repository or filesystem authority through this API.
    /// Returns an action descriptor so callers can distinguish stale context,
    /// unavailable recovery, and actual lifecycle admission.
    pub async fn resume_partial_download_with_ticket(
        &self,
        model_id: &model_library::DownloadRecoveryModelId,
        recovery_token: &model_library::DownloadRecoveryToken,
    ) -> Result<models::PartialDownloadAction> {
        let primary = self.primary().clone();
        let client = match primary.hf_client.clone() {
            Some(client) => client,
            None => {
                return Ok(models::PartialDownloadAction {
                    action: "none".to_string(),
                    download_id: None,
                    status: None,
                    reason_code: Some("hf_client_unavailable".to_string()),
                    message: Some("HuggingFace client not initialized".to_string()),
                });
            }
        };

        let operation_client = client.clone();
        let model_id = model_id.clone();
        let recovery_token = recovery_token.clone();
        client
            .run_download_invocation(move |context| async move {
                let client = operation_client;
                let library = primary.model_library.clone();
                let record_id = model_id.as_str().to_string();
                let indexed_record = match context
                    .run_fallible_async_named("load ticket recovery model", move || async move {
                        library.get_model(&record_id).await
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Recovery model observation failed: {error}"))
                    })? {
                    Ok(Some(record)) => record,
                    Ok(None) => return Ok(partial_download_unavailable("model_not_found")),
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                let model_dir = match client
                    .inspect_recovery_model_directory(
                        primary.model_library.library_root().to_path_buf(),
                        indexed_record,
                    )
                    .await
                {
                    Ok(Some(model_dir)) => model_dir,
                    Ok(None) => return Ok(partial_download_unavailable("recovery_unavailable")),
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                let context = match client.protect_download_mutation(&context).await {
                    Ok(context) => context,
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                let library = primary.model_library.clone();
                if let Err(error) = context
                    .run_fallible_async_named("index ticket recovery model", move || async move {
                        library.index_model_dir(&model_dir).await
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Recovery index observation failed: {error}"))
                    })?
                {
                    return Ok(partial_download_error(&error));
                }
                let library = primary.model_library.clone();
                let record_id = model_id.as_str().to_string();
                let fresh_record = match context
                    .run_fallible_async_named("reload ticket recovery model", move || async move {
                        library.get_model(&record_id).await
                    })
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Recovery model observation failed: {error}"))
                    })? {
                    Ok(Some(record)) => record,
                    Ok(None) => return Ok(partial_download_unavailable("model_not_found")),
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                let verification = match client
                    .verify_recovery_model_snapshot(
                        primary.model_library.library_root().to_path_buf(),
                        fresh_record,
                        recovery_token.clone(),
                    )
                    .await
                {
                    Ok(verification) => verification,
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                let verified = match verification {
                    model_library::DownloadRecoveryVerification::Complete => {
                        return Ok(partial_download_unavailable("model_not_partial"));
                    }
                    model_library::DownloadRecoveryVerification::Unavailable => {
                        return Ok(partial_download_unavailable("recovery_unavailable"));
                    }
                    model_library::DownloadRecoveryVerification::Stale => {
                        return Ok(partial_download_unavailable("recovery_context_stale"));
                    }
                    model_library::DownloadRecoveryVerification::Verified(verified) => verified,
                };

                let model_type = model_id.as_str().split('/').next().map(str::to_string);
                let admission = match client.admit_recovery_download(&verified, model_type).await {
                    Ok(admission) => admission,
                    Err(error) => return Ok(partial_download_error(&error)),
                };
                match admission {
                    model_library::RecoveryDownloadAdmission::Recovered { download_id } => {
                        Ok(models::PartialDownloadAction {
                            action: "recover".to_string(),
                            download_id: Some(download_id),
                            status: Some(models::DownloadStatus::Queued),
                            reason_code: None,
                            message: None,
                        })
                    }
                    model_library::RecoveryDownloadAdmission::Resumed { download_id } => {
                        Ok(models::PartialDownloadAction {
                            action: "resume".to_string(),
                            download_id: Some(download_id),
                            status: Some(models::DownloadStatus::Queued),
                            reason_code: None,
                            message: None,
                        })
                    }
                    model_library::RecoveryDownloadAdmission::Attached {
                        download_id,
                        status,
                    } => Ok(models::PartialDownloadAction {
                        action: "attach".to_string(),
                        download_id: Some(download_id),
                        status: Some(status),
                        reason_code: None,
                        message: None,
                    }),
                    model_library::RecoveryDownloadAdmission::AlreadyCompleted { download_id } => {
                        Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: Some(download_id),
                            status: Some(models::DownloadStatus::Completed),
                            reason_code: Some("already_completed".to_string()),
                            message: Some("tracked download is already completed".to_string()),
                        })
                    }
                    model_library::RecoveryDownloadAdmission::AlreadyCancelled { download_id } => {
                        Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: Some(download_id),
                            status: Some(models::DownloadStatus::Cancelled),
                            reason_code: Some("already_cancelled".to_string()),
                            message: Some("tracked download was cancelled".to_string()),
                        })
                    }
                    model_library::RecoveryDownloadAdmission::ContextMismatch
                    | model_library::RecoveryDownloadAdmission::BoundFilesUnavailable => {
                        Ok(partial_download_unavailable("recovery_context_stale"))
                    }
                    model_library::RecoveryDownloadAdmission::CapabilityUnavailable => {
                        Ok(partial_download_unavailable("recovery_unavailable"))
                    }
                }
            })
            .await
    }

    /// Resume a partial download by choosing the correct action:
    /// - Resume an existing tracked paused/error download
    /// - Attach to an already active tracked download
    /// - Recover an orphan partial download
    ///
    /// Returns an action descriptor instead of failing hard so UI callers can
    /// surface precise next steps to users.
    pub async fn resume_partial_download(
        &self,
        repo_id: &str,
        dest_dir: &str,
    ) -> Result<models::PartialDownloadAction> {
        let primary = self.primary().clone();
        let client = match primary.hf_client.clone() {
            Some(client) => client,
            None => return Ok(partial_download_unavailable("hf_client_unavailable")),
        };
        Self::resume_partial_download_owned(
            primary.model_library.clone(),
            client,
            repo_id,
            dest_dir,
        )
        .await
    }

    pub(super) async fn resume_partial_download_owned(
        library: Arc<model_library::ModelLibrary>,
        client: Arc<model_library::HuggingFaceClient>,
        repo_id: &str,
        dest_dir: &str,
    ) -> Result<models::PartialDownloadAction> {
        let operation_client = client.clone();
        let repo_id = repo_id.to_string();
        let dest_dir = dest_dir.to_string();
        client
            .run_download_invocation(move |context| async move {
                let client = operation_client;
                let lookup_dir = dest_dir.clone();
                let dest = match context
                    .run_fallible_async_named(
                        "resolve partial download directory",
                        move || async move {
                            validate_existing_local_directory_lookup_path(&lookup_dir, "dest_dir")
                                .await
                        },
                    )
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Partial directory observation failed: {error}"))
                    })? {
                    Ok(dest) => dest,
                    Err(PumasError::InvalidParams { .. } | PumasError::NotFound { .. }) => {
                        return Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: None,
                            status: None,
                            reason_code: Some("dest_dir_missing".to_string()),
                            message: Some(format!("directory not found: {}", dest_dir)),
                        });
                    }
                    Err(err) => return Err(err),
                };

                if let Some(download_id) = client.find_download_id_by_dest_dir(&dest).await {
                    let status = client.get_download_status(&download_id).await;
                    if let Some(status) = status {
                        match status {
                            models::DownloadStatus::Paused | models::DownloadStatus::Error => {
                                match client.resume_download(&download_id).await {
                                    Ok(true) => {
                                        return Ok(models::PartialDownloadAction {
                                            action: "resume".to_string(),
                                            download_id: Some(download_id),
                                            status: Some(models::DownloadStatus::Queued),
                                            reason_code: None,
                                            message: None,
                                        });
                                    }
                                    Ok(false) => {
                                        return Ok(models::PartialDownloadAction {
                                            action: "none".to_string(),
                                            download_id: Some(download_id),
                                            status: Some(status),
                                            reason_code: Some("resume_rejected".to_string()),
                                            message: Some(format!(
                                        "tracked download cannot be resumed from status {:?}",
                                        status
                                    )),
                                        });
                                    }
                                    Err(err) => {
                                        let reason_code =
                                            partial_download_reason_code(&err).to_string();
                                        return Ok(models::PartialDownloadAction {
                                            action: "none".to_string(),
                                            download_id: Some(download_id),
                                            status: Some(status),
                                            reason_code: Some(reason_code),
                                            message: Some(err.to_string()),
                                        });
                                    }
                                }
                            }
                            models::DownloadStatus::Queued
                            | models::DownloadStatus::Downloading
                            | models::DownloadStatus::Pausing
                            | models::DownloadStatus::Cancelling => {
                                return Ok(models::PartialDownloadAction {
                                    action: "attach".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(status),
                                    reason_code: None,
                                    message: None,
                                });
                            }
                            models::DownloadStatus::Completed => {
                                return Ok(models::PartialDownloadAction {
                                    action: "none".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(status),
                                    reason_code: Some("already_completed".to_string()),
                                    message: Some(
                                        "tracked download is already completed".to_string(),
                                    ),
                                });
                            }
                            models::DownloadStatus::Cancelled => {
                                return Ok(models::PartialDownloadAction {
                                    action: "none".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(status),
                                    reason_code: Some("already_cancelled".to_string()),
                                    message: Some(
                                        "tracked download was cancelled; start a new download"
                                            .to_string(),
                                    ),
                                });
                            }
                        }
                    }
                }

                match Self::recover_download_admitted(library, client, context, repo_id, dest_dir)
                    .await
                {
                    Ok(download_id) => Ok(models::PartialDownloadAction {
                        action: "recover".to_string(),
                        download_id: Some(download_id),
                        status: Some(models::DownloadStatus::Queued),
                        reason_code: None,
                        message: None,
                    }),
                    Err(err) => {
                        let reason_code = partial_download_reason_code(&err).to_string();
                        Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: None,
                            status: None,
                            reason_code: Some(reason_code),
                            message: Some(err.to_string()),
                        })
                    }
                }
            })
            .await
    }

    /// Refetch metadata for a library model from HuggingFace.
    ///
    /// Uses the stored `repo_id` if available, otherwise falls back to
    /// filename-based lookup via `lookup_metadata()`. Returns the updated
    /// metadata on success.
    pub async fn refetch_metadata_from_hf(&self, model_id: &str) -> Result<models::ModelMetadata> {
        let primary = self.primary();
        let hf_client = primary
            .hf_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })?;
        let library = &primary.model_library;

        // Handle download-in-progress models: extract repo_id and fetch directly
        if let Some(repo_id) = model_id.strip_prefix("download:") {
            let model = hf_client.get_model_info(repo_id).await?;
            let model_type = resolve_model_type_from_hints_async(
                library.index().clone(),
                vec![Some(model.kind.clone()), None, None],
            )
            .await?;
            return Ok(models::ModelMetadata {
                repo_id: Some(model.repo_id),
                official_name: Some(model.name),
                model_type,
                download_url: Some(model.url),
                release_date: model.release_date,
                model_card: model.model_card,
                license_status: model
                    .license
                    .or_else(|| Some("license_unknown".to_string())),
                match_source: Some("hf".to_string()),
                match_method: Some("repo_id".to_string()),
                match_confidence: Some(1.0),
                ..Default::default()
            });
        }

        // Load current metadata
        let model_dir = library.library_root().join(model_id);
        let (current, primary_file) =
            load_hf_model_snapshot(library.clone(), model_dir.clone(), model_id.to_string())
                .await?;

        let repo_id = current
            .as_ref()
            .and_then(|m| m.repo_id.clone())
            .or_else(|| {
                // model_id is "{type}/{owner}/{name}" — extract "{owner}/{name}" as repo_id
                let parts: Vec<&str> = model_id.splitn(3, '/').collect();
                if parts.len() == 3 {
                    Some(format!("{}/{}", parts[1], parts[2]))
                } else {
                    None
                }
            });

        let hf_result = if let Some(ref repo_id) = repo_id {
            // Fetch model info directly by repo_id (bypasses search cache)
            let model = hf_client.get_model_info(repo_id).await?;
            let translated_model_type = resolve_model_type_from_hints_async(
                library.index().clone(),
                vec![Some(model.kind.clone()), None, None],
            )
            .await?;
            model_library::HfMetadataResult {
                repo_id: model.repo_id,
                official_name: Some(model.name),
                family: None,
                model_type: translated_model_type,
                subtype: None,
                variant: None,
                precision: None,
                tags: vec![],
                base_model: None,
                download_url: Some(model.url),
                release_date: model.release_date,
                model_card_json: serialize_model_card_json(model.model_card.as_ref()),
                license_status: model
                    .license
                    .or_else(|| Some("license_unknown".to_string())),
                description: None,
                match_confidence: 1.0,
                match_method: "repo_id".to_string(),
                requires_confirmation: false,
                hash_mismatch: false,
                matched_filename: None,
                pending_full_verification: false,
                fast_hash: None,
                expected_sha256: None,
            }
        } else {
            // Fallback: use filename-based lookup
            let file_path = primary_file.ok_or_else(|| PumasError::NotFound {
                resource: format!("primary model file for: {}", model_id),
            })?;
            let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            hf_client
                .lookup_metadata(filename, Some(&file_path), None)
                .await?
                .ok_or_else(|| PumasError::NotFound {
                    resource: format!("HuggingFace metadata for: {}", model_id),
                })?
        };

        // Update stored metadata (force=true to bypass manual guard)
        library
            .update_metadata_from_hf(model_id, &hf_result, true)
            .await?;

        // Return the freshly-updated metadata
        let updated = load_model_metadata_or_default(library.clone(), model_dir).await?;
        Ok(updated)
    }

    /// Look up HuggingFace metadata for a local file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        if let Some(ref client) = self.primary().hf_client {
            let path = validate_existing_local_file_lookup_path(file_path, "file_path").await?;
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            client.lookup_metadata(filename, Some(&path), None).await
        } else {
            Ok(None)
        }
    }

    /// Look up HuggingFace metadata for a local diffusers bundle directory.
    pub async fn lookup_hf_metadata_for_bundle_directory(
        &self,
        dir_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        let primary = self.primary();
        let Some(client) = primary.hf_client.as_ref() else {
            return Ok(None);
        };

        let dir_path = validate_existing_local_directory_lookup_path(dir_path, "dir_path").await?;
        let dir_path_for_lookup = dir_path.clone();
        let hints = tokio::task::spawn_blocking(move || {
            model_library::get_diffusers_bundle_lookup_hints(&dir_path_for_lookup)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join bundle lookup hint extraction task: {}",
                err
            ))
        })?;
        let Some(hints) = hints else {
            return Ok(None);
        };

        let search_results = collect_bundle_lookup_candidates(client, &hints.bundle_name).await?;

        for candidate in rank_bundle_lookup_candidates(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        ) {
            if client
                .classify_repo_bundle(&candidate.repo_id)
                .await?
                .is_none()
            {
                continue;
            }

            let candidate_repo_id = candidate.repo_id.clone();
            let match_confidence = if is_exact_bundle_lookup_match(
                &hints.bundle_name,
                &candidate_repo_id,
                &candidate.name,
            ) {
                0.95
            } else {
                0.72
            };

            return Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                candidate,
                if match_confidence >= 0.9 {
                    "filename_exact"
                } else {
                    "filename_fuzzy"
                },
                match_confidence,
                hints
                    .name_or_path
                    .as_ref()
                    .filter(|repo_id| *repo_id != &candidate_repo_id)
                    .cloned(),
            )?));
        }

        if let Some((candidate, match_method, match_confidence)) = fallback_bundle_lookup_candidate(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        ) {
            let candidate_repo_id = candidate.repo_id.clone();
            return Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                candidate,
                match_method,
                match_confidence,
                hints
                    .name_or_path
                    .as_ref()
                    .filter(|repo_id| *repo_id != &candidate_repo_id)
                    .cloned(),
            )?));
        }

        let Some(base_repo_id) = hints.name_or_path.as_deref() else {
            return Ok(None);
        };
        if !looks_like_repo_id(base_repo_id) {
            return Ok(None);
        }

        match client.get_model_info(base_repo_id).await {
            Ok(model) => Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                model,
                "filename_fuzzy",
                0.55,
                None,
            )?)),
            Err(err) => {
                warn!(
                    "Failed to resolve diffusers bundle base model {} for {}: {}",
                    base_repo_id,
                    dir_path.display(),
                    err
                );
                Ok(None)
            }
        }
    }

    // ========================================
    // HuggingFace Authentication
    // ========================================

    /// Set the HuggingFace authentication token.
    ///
    /// Persists to disk and updates the in-memory token for immediate use.
    pub async fn set_hf_token(&self, token: &str) -> Result<()> {
        if let Some(ref client) = self.primary().hf_client {
            client.set_auth_token(token).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Clear the HuggingFace authentication token.
    ///
    /// Removes the persisted token file and clears the in-memory value.
    pub async fn clear_hf_token(&self) -> Result<()> {
        if let Some(ref client) = self.primary().hf_client {
            client.clear_auth_token().await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Get current HuggingFace authentication status.
    ///
    /// Makes a lightweight API call to validate the token and retrieve
    /// the associated username.
    pub async fn get_hf_auth_status(&self) -> Result<model_library::HfAuthStatus> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_auth_status().await
        } else {
            Ok(model_library::HfAuthStatus {
                authenticated: false,
                username: None,
                token_source: None,
            })
        }
    }

    /// Get repository file tree from HuggingFace.
    pub async fn get_hf_repo_files(&self, repo_id: &str) -> Result<model_library::RepoFileTree> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_repo_files(repo_id).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }
}

pub(crate) fn resolve_model_type_from_hints<const N: usize>(
    index: &crate::index::ModelIndex,
    hints: [Option<&str>; N],
) -> Result<Option<String>> {
    let mut seen = HashSet::new();
    for raw_hint in hints.into_iter().flatten() {
        let normalized_hint = raw_hint.trim().to_lowercase();
        if normalized_hint.is_empty() || !seen.insert(normalized_hint.clone()) {
            continue;
        }
        if let Some(model_type) = index.resolve_model_type_hint(&normalized_hint)? {
            return Ok(Some(model_type));
        }
    }
    Ok(None)
}

pub(crate) async fn resolve_model_type_from_hints_async(
    index: crate::index::ModelIndex,
    hints: Vec<Option<String>>,
) -> Result<Option<String>> {
    tokio::task::spawn_blocking(move || resolve_owned_model_type_hints(&index, hints))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HuggingFace model-type hint resolution task: {}",
                err
            ))
        })?
}

fn resolve_owned_model_type_hints(
    index: &crate::index::ModelIndex,
    hints: Vec<Option<String>>,
) -> Result<Option<String>> {
    let mut seen = HashSet::new();
    for raw_hint in hints.into_iter().flatten() {
        let normalized_hint = raw_hint.trim().to_lowercase();
        if normalized_hint.is_empty() || !seen.insert(normalized_hint.clone()) {
            continue;
        }
        if let Some(model_type) = index.resolve_model_type_hint(&normalized_hint)? {
            return Ok(Some(model_type));
        }
    }
    Ok(None)
}

fn validate_prepared_download(
    library: &model_library::ModelLibrary,
    prepared: &PreparedIntentDownload,
    expected: Option<&models::PumasModelRef>,
) -> Result<()> {
    let selected = model_library::SelectedArtifactIdentity::from_download_request_at_revision(
        &prepared.request,
        None,
        &prepared.revision,
    );
    let model_id = library.build_artifact_model_id(
        &prepared.model_type,
        &prepared.architecture_family,
        &selected.artifact_id,
    );
    if selected != prepared.selected_artifact
        || prepared.model_ref.model_id != model_id
        || prepared.model_ref.revision.as_deref() != prepared.revision.as_persisted()
        || prepared.model_ref.selected_artifact_id.as_deref()
            != Some(prepared.selected_artifact.artifact_id.as_str())
        || prepared.model_ref.selected_artifact_path.is_some()
    {
        return Err(PumasError::Validation {
            field: "intent.prepared_download".to_string(),
            message: "prepared download identity is inconsistent".to_string(),
        });
    }
    if expected.is_some_and(|expected| {
        expected.model_id != prepared.model_ref.model_id
            || expected.revision != prepared.model_ref.revision
            || expected.selected_artifact_id != prepared.model_ref.selected_artifact_id
            || expected.selected_artifact_path.is_some()
    }) {
        return Err(PumasError::Validation {
            field: "intent.prepared_download.expected_model_ref".to_string(),
            message: "prepared download does not match its durable immutable binding".to_string(),
        });
    }
    Ok(())
}

fn owned_download_planning_result<T>(result: Result<T>) -> Result<Result<T>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(
            error @ (PumasError::Network { .. }
            | PumasError::Timeout(_)
            | PumasError::RateLimited { .. }
            | PumasError::CircuitBreakerOpen { .. }
            | PumasError::Validation { .. }
            | PumasError::Json { .. }),
        ) => Ok(Err(error)),
        Err(error) => Err(error),
    }
}

fn package_artifact_kind_for_filename(filename: &str) -> Option<models::PackageArtifactKind> {
    let filename = filename.to_ascii_lowercase();
    if filename.ends_with(".gguf") {
        Some(models::PackageArtifactKind::Gguf)
    } else if filename.ends_with(".onnx") {
        Some(models::PackageArtifactKind::Onnx)
    } else if filename.ends_with(".safetensors") {
        Some(models::PackageArtifactKind::Safetensors)
    } else {
        None
    }
}

pub(crate) fn normalized_download_hint(hint: Option<&str>) -> Option<&str> {
    hint.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(crate) fn serialize_model_card_json(
    model_card: Option<&std::collections::HashMap<String, serde_json::Value>>,
) -> Option<String> {
    model_card
        .filter(|card| !card.is_empty())
        .and_then(|card| serde_json::to_string(card).ok())
}

pub(crate) fn apply_remote_model_metadata(
    request: &mut model_library::DownloadRequest,
    model: &models::HuggingFaceModel,
) {
    if request.release_date.is_none() {
        request.release_date = model.release_date.clone();
    }
    if request.download_url.is_none() && !model.url.trim().is_empty() {
        request.download_url = Some(model.url.clone());
    }
    if request.model_card_json.is_none() {
        request.model_card_json = serialize_model_card_json(model.model_card.as_ref());
    }
    if request.license_status.is_none() {
        request.license_status = model
            .license
            .clone()
            .or_else(|| Some("license_unknown".to_string()));
    }
}

pub(crate) fn build_lookup_metadata_from_model(
    index: &crate::index::ModelIndex,
    model: models::HuggingFaceModel,
    match_method: &str,
    match_confidence: f64,
    base_model: Option<String>,
) -> Result<model_library::HfMetadataResult> {
    let model_type = resolve_model_type_from_hints(index, [Some(model.kind.as_str()), None, None])?;
    Ok(model_library::HfMetadataResult {
        repo_id: model.repo_id,
        official_name: Some(model.name),
        family: None,
        model_type,
        subtype: None,
        variant: None,
        precision: None,
        tags: vec![],
        base_model,
        download_url: Some(model.url),
        release_date: model.release_date,
        model_card_json: serialize_model_card_json(model.model_card.as_ref()),
        license_status: model
            .license
            .or_else(|| Some("license_unknown".to_string())),
        description: None,
        match_confidence,
        match_method: match_method.to_string(),
        requires_confirmation: match_confidence < 0.8,
        hash_mismatch: false,
        matched_filename: None,
        pending_full_verification: false,
        fast_hash: None,
        expected_sha256: None,
    })
}

pub(crate) fn rank_bundle_lookup_candidates(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidates: &[models::HuggingFaceModel],
) -> Vec<models::HuggingFaceModel> {
    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| {
        let left_score = bundle_lookup_score(bundle_name, hinted_repo_id, left);
        let right_score = bundle_lookup_score(bundle_name, hinted_repo_id, right);
        right_score
            .cmp(&left_score)
            .then_with(|| {
                right
                    .downloads
                    .unwrap_or(0)
                    .cmp(&left.downloads.unwrap_or(0))
            })
            .then_with(|| left.repo_id.cmp(&right.repo_id))
    });
    ranked
}

pub(crate) async fn collect_bundle_lookup_candidates(
    client: &model_library::HuggingFaceClient,
    bundle_name: &str,
) -> Result<Vec<models::HuggingFaceModel>> {
    let mut merged = Vec::new();
    let mut seen_repo_ids = HashSet::new();

    for query in bundle_lookup_query_variants(bundle_name) {
        for kind in [Some("text-to-image"), None] {
            let results = client
                .search(&model_library::HfSearchParams {
                    query: query.clone(),
                    kind: kind.map(str::to_string),
                    limit: Some(20),
                    hydrate_limit: Some(10),
                    ..Default::default()
                })
                .await?;

            for candidate in results {
                if seen_repo_ids.insert(candidate.repo_id.clone()) {
                    merged.push(candidate);
                }
            }
        }
    }

    Ok(merged)
}

fn bundle_lookup_query_variants(bundle_name: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let mut push = |value: String| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        let normalized = trimmed.to_lowercase();
        if seen.insert(normalized) {
            queries.push(trimmed.to_string());
        }
    };

    push(bundle_name.to_string());
    push(bundle_name.replace(['-', '_'], " "));
    push(
        bundle_name
            .chars()
            .map(|ch| {
                if ch == '-' || ch == '_' {
                    ' '
                } else if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                    ch
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    );

    queries
}

pub(crate) fn fallback_bundle_lookup_candidate(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidates: &[models::HuggingFaceModel],
) -> Option<(models::HuggingFaceModel, &'static str, f64)> {
    let ranked = rank_bundle_lookup_candidates(bundle_name, hinted_repo_id, candidates);
    let candidate = ranked
        .into_iter()
        .find(|candidate| bundle_lookup_score(bundle_name, hinted_repo_id, candidate) >= 35)?;

    let exact = is_exact_bundle_lookup_match(bundle_name, &candidate.repo_id, &candidate.name);
    Some((
        candidate,
        if exact {
            "filename_exact"
        } else {
            "filename_fuzzy"
        },
        if exact { 0.84 } else { 0.62 },
    ))
}

fn bundle_lookup_score(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidate: &models::HuggingFaceModel,
) -> i32 {
    let mut score = 0;
    if is_exact_bundle_lookup_match(bundle_name, &candidate.repo_id, &candidate.name) {
        score += 100;
    }

    let normalized_bundle = normalize_bundle_lookup_key(bundle_name);
    let repo_basename = repo_basename(&candidate.repo_id);
    let normalized_repo_basename = normalize_bundle_lookup_key(repo_basename);
    if !normalized_bundle.is_empty() && normalized_repo_basename.contains(&normalized_bundle) {
        score += 25;
    }

    if candidate.kind == "text-to-image" {
        score += 10;
    }

    if hinted_repo_id.is_some_and(|repo_id| repo_id == candidate.repo_id) {
        score += 5;
    }

    score
}

pub(crate) fn is_exact_bundle_lookup_match(
    bundle_name: &str,
    repo_id: &str,
    model_name: &str,
) -> bool {
    let normalized_bundle = normalize_bundle_lookup_key(bundle_name);
    if normalized_bundle.is_empty() {
        return false;
    }

    let repo_match = normalize_bundle_lookup_key(repo_basename(repo_id)) == normalized_bundle;
    let name_match = normalize_bundle_lookup_key(model_name) == normalized_bundle;
    repo_match || name_match
}

fn normalize_bundle_lookup_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn repo_basename(repo_id: &str) -> &str {
    repo_id.rsplit('/').next().unwrap_or(repo_id)
}

pub(crate) fn looks_like_repo_id(value: &str) -> bool {
    let mut parts = value.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.trim().is_empty() && !name.trim().is_empty()
    )
}

pub(crate) fn partial_download_reason_code(err: &PumasError) -> &'static str {
    match err {
        PumasError::NotFound { .. } => "dest_dir_missing",
        PumasError::ModelNotFound { .. } => "repo_not_found",
        PumasError::RateLimited { .. } => "rate_limited",
        PumasError::DownloadRootBusy => "download_root_busy",
        PumasError::PermissionDenied(_) => "permission_denied",
        PumasError::Network { message, .. } if message.contains("404 Not Found") => {
            "repo_not_found"
        }
        PumasError::Timeout(_)
        | PumasError::Network { .. }
        | PumasError::CircuitBreakerOpen { .. } => "network_error",
        PumasError::Config { message } if message.contains("Invalid repo_id format") => {
            "invalid_repo_id"
        }
        PumasError::Config { .. } => "hf_client_unavailable",
        _ => "recover_failed",
    }
}

fn partial_download_unavailable(reason_code: &str) -> models::PartialDownloadAction {
    models::PartialDownloadAction {
        action: "none".to_string(),
        download_id: None,
        status: None,
        reason_code: Some(reason_code.to_string()),
        message: Some("The partial download recovery context is unavailable.".to_string()),
    }
}

fn partial_download_error(error: &PumasError) -> models::PartialDownloadAction {
    let reason = match error {
        PumasError::NotFound { .. } => "recovery_unavailable",
        _ => partial_download_reason_code(error),
    };
    partial_download_unavailable(reason)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_without_hf_client_is_repeatable() {
        let root = tempfile::TempDir::new().unwrap();
        let mut api = recovery_api_fixture(root.path(), None).await;
        let crate::ApiInner::Primary(primary) = &mut api.inner;
        let client = Arc::get_mut(primary).unwrap().hf_client.take().unwrap();
        client.shutdown_downloads().await.unwrap();

        api.shutdown_downloads().await.unwrap();
        api.shutdown_downloads().await.unwrap();
        assert!(api.primary().hf_client.is_none());
    }

    #[tokio::test]
    async fn closed_download_api_refuses_before_metadata_or_destination_work() {
        let root = tempfile::TempDir::new().unwrap();
        let api = recovery_api_fixture(root.path(), None).await;
        api.shutdown_downloads().await.unwrap();
        let request = model_library::DownloadRequest {
            repo_id: "shutdown-fixture/model".into(),
            family: "shutdown-fixture".into(),
            official_name: "model".into(),
            model_type: Some("llm".into()),
            quant: None,
            filename: Some("weights.gguf".into()),
            filenames: None,
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            api.start_hf_download(&request),
        )
        .await
        .expect("closed admission must not attempt remote metadata resolution");
        assert!(matches!(result, Err(PumasError::DownloadLifecycleClosed)));
        assert!(matches!(
            api.recover_download("shutdown-fixture/model", "/missing-shutdown-fixture")
                .await,
            Err(PumasError::DownloadLifecycleClosed)
        ));
        assert!(matches!(
            api.resume_partial_download("shutdown-fixture/model", "/missing-shutdown-fixture")
                .await,
            Err(PumasError::DownloadLifecycleClosed)
        ));
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join("llm")
            .exists());
        api.shutdown_downloads().await.unwrap();
    }

    #[tokio::test]
    async fn pinned_download_refuses_unconfirmed_commit_before_destination_mutation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 2048];
            let read = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with(&format!("GET /api/models/acme/model/revision/{COMMIT} ")));
            let body = r#"{"modelId":"acme/model"}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let root = tempfile::TempDir::new().unwrap();
        let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
        let request = model_library::DownloadRequest {
            repo_id: "acme/model".into(),
            family: "acme".into(),
            official_name: "model".into(),
            model_type: Some("llm".into()),
            quant: None,
            filename: Some("weights.gguf".into()),
            filenames: None,
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        let result = PumasApi::start_hf_download_owned_at_revision(
            api.primary().model_library.clone(),
            api.primary().hf_client.clone().unwrap(),
            &request,
            DownloadRevision::from_commit(COMMIT).unwrap(),
        )
        .await;
        server.await.unwrap();

        assert!(matches!(result, Err(PumasError::Validation { .. })));
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join("llm")
            .exists());
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
        api.shutdown_downloads().await.unwrap();
    }

    #[tokio::test]
    async fn prepared_download_is_read_only_and_expected_identity_mismatch_precedes_mutation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 2048];
            let read = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with(&format!("GET /api/models/acme/model/revision/{COMMIT} ")));
            let body = format!(
                r#"{{"modelId":"acme/model","sha":"{COMMIT}","pipeline_tag":"text-generation","siblings":[{{"rfilename":"weights.gguf","lfs":{{"sha256":"{}","size":1}}}}]}}"#,
                "a".repeat(64)
            );
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let root = tempfile::TempDir::new().unwrap();
        let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
        let request = model_library::DownloadRequest {
            repo_id: "acme/model".into(),
            family: "acme".into(),
            official_name: "model".into(),
            model_type: Some("llm".into()),
            quant: None,
            filename: Some("weights.gguf".into()),
            filenames: None,
            pipeline_tag: Some("text-generation".into()),
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        let prepared = PumasApi::prepare_hf_download_owned_at_revision(
            api.primary().model_library.clone(),
            api.primary().hf_client.clone().unwrap(),
            &request,
            DownloadRevision::from_commit(COMMIT).unwrap(),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join("llm")
            .exists());

        let mut wrong = prepared.model_ref().clone();
        wrong.model_id = "llm/acme/wrong".to_string();
        let result = PumasApi::start_prepared_hf_download_owned(
            api.primary().model_library.clone(),
            api.primary().hf_client.clone().unwrap(),
            prepared,
            Some(&wrong),
        )
        .await;
        assert!(matches!(result, Err(PumasError::Validation { .. })));
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join("llm")
            .exists());
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
        api.shutdown_downloads().await.unwrap();
    }

    pub(crate) async fn recovery_api_fixture(
        root: &std::path::Path,
        download_base_url: Option<String>,
    ) -> PumasApi {
        use crate::api::{
            PrimaryState, ReconciliationCoordinator, RuntimeTasks, WatcherWriteSuppressor,
        };
        use std::time::Duration;
        use tokio::sync::{Mutex, RwLock};

        let library = Arc::new(
            model_library::ModelLibrary::new(root.join("shared-resources/models"))
                .await
                .unwrap(),
        );
        let mut client = model_library::HuggingFaceClient::new(root.join("cache")).unwrap();
        client
            .configure_download_destination_root(library.library_root())
            .unwrap();
        if let Some(base_url) = download_base_url {
            client.set_test_download_base_url(base_url);
        }
        std::fs::create_dir_all(root.join("state")).unwrap();
        let persistence = Arc::new(model_library::DownloadPersistence::new(&root.join("state")));
        client.set_persistence(persistence.clone());
        client.set_download_importer(Arc::new(model_library::ModelImporter::new(library.clone())));
        let tasks = RuntimeTasks::default();
        library
            .install_mutation_authority(
                tasks.clone(),
                model_library::DownloadDestinationRoot::open(library.library_root()).unwrap(),
                persistence,
            )
            .unwrap();
        let client = Arc::new(client);
        let intent_service = Arc::new(crate::intent::IntentService::new(
            library.clone(),
            Some(client.clone()),
            tasks.clone(),
        ));
        let provider_registry = crate::providers::ProviderRegistry::builtin();
        let primary = PrimaryState {
            _state: Arc::new(RwLock::new(crate::api::state::ApiState {
                background_fetch_completed: false,
            })),
            network_manager: Arc::new(crate::network::NetworkManager::new().unwrap()),
            process_manager: Arc::new(RwLock::new(None)),
            resource_tracker: Arc::new(crate::system::ResourceTracker::default()),
            status_telemetry: Arc::new(
                crate::api::status_telemetry::StatusTelemetryService::default(),
            ),
            system_utils: Arc::new(crate::system::SystemUtils::new(root)),
            model_importer: model_library::ModelImporter::new(library.clone()),
            conversion_manager: Arc::new(crate::conversion::ConversionManager::new(
                root.to_path_buf(),
                library.clone(),
                Arc::new(model_library::ModelImporter::new(library.clone())),
            )),
            runtime_profile_service: Arc::new(
                crate::runtime_profiles::RuntimeProfileService::with_provider_registry_and_adapters(
                    root,
                    provider_registry.clone(),
                    crate::runtime_profiles::RuntimeProviderAdapters::builtin(),
                ),
            ),
            serving_service: Arc::new(crate::serving::ServingService::with_provider_registry(
                provider_registry,
            )),
            model_library: library,
            hf_client: Some(client),
            intent_service,
            runtime_tasks: tasks.clone(),
            reconciliation: Arc::new(ReconciliationCoordinator::new(
                Duration::ZERO,
                Duration::ZERO,
            )),
            watcher_write_suppressor: Arc::new(WatcherWriteSuppressor::new(Duration::from_secs(1))),
            server_handle: Mutex::new(None),
            registry: None,
            instance_claim: Mutex::new(None),
        };
        // Exercise the production API without a global registry, IPC listener,
        // filesystem watcher, or background connectivity probe.
        PumasApi {
            launcher_root: root.to_path_buf(),
            inner: crate::ApiInner::Primary(Arc::new(primary)),
            model_watcher: None,
            runtime_tasks: tasks,
        }
    }

    async fn indexed_partial_ticket(
        api: &PumasApi,
    ) -> (
        model_library::DownloadRecoveryModelId,
        model_library::DownloadRecoveryToken,
        model_library::ModelMetadata,
    ) {
        let library = &api.primary().model_library;
        let model_dir = library.library_root().join("llm/acme/model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weights.gguf.part"), b"partial").unwrap();
        let metadata = model_library::ModelMetadata {
            model_id: Some("llm/acme/model".into()),
            family: Some("acme".into()),
            model_type: Some("llm".into()),
            cleaned_name: Some("model".into()),
            official_name: Some("Model".into()),
            repo_id: Some("acme/model".into()),
            selected_artifact_id: Some("acme/model::Q4_K_M".into()),
            selected_artifact_quant: Some("Q4_K_M".into()),
            selected_artifact_files: Some(vec!["weights.gguf".into()]),
            expected_files: Some(vec!["weights.gguf".into()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();
        let record = api.get_model("llm/acme/model").await.unwrap().unwrap();
        let ticket = model_library::issue_download_recovery_ticket(library.library_root(), &record)
            .unwrap()
            .expect("indexed partial must have a recovery ticket");
        (
            model_library::DownloadRecoveryModelId::parse(&record.id).unwrap(),
            model_library::DownloadRecoveryToken::parse(ticket.token()).unwrap(),
            metadata,
        )
    }

    #[tokio::test]
    async fn ticket_recovery_refuses_changed_artifact_without_download_mutation() {
        let root = tempfile::TempDir::new().unwrap();
        let api = recovery_api_fixture(root.path(), None).await;
        let (model_id, token, mut metadata) = indexed_partial_ticket(&api).await;
        let model_dir = api
            .primary()
            .model_library
            .library_root()
            .join(model_id.as_str());
        metadata.selected_artifact_id = Some("acme/model::Q5_K_M".into());
        metadata.selected_artifact_quant = Some("Q5_K_M".into());
        api.primary()
            .model_library
            .save_metadata(&model_dir, &metadata)
            .await
            .unwrap();

        let result = api
            .resume_partial_download_with_ticket(&model_id, &token)
            .await
            .unwrap();
        assert_eq!(result.action, "none");
        assert_eq!(
            result.reason_code.as_deref(),
            Some("recovery_context_stale")
        );
        assert!(result.download_id.is_none());
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
        assert_eq!(
            std::fs::read(model_dir.join("weights.gguf.part")).unwrap(),
            b"partial"
        );
        assert!(!model_dir.join(".pumas_download").exists());

        let missing = root.path().join("missing-model");
        let legacy = api
            .resume_partial_download("acme/model", missing.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(legacy.action, "none");
        assert_eq!(legacy.reason_code.as_deref(), Some("dest_dir_missing"));
        assert!(legacy.download_id.is_none());
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn ticket_recovery_admits_exact_partial_and_public_cancel_preserves_other_artifacts() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::time::{timeout, Duration};

        let root = tempfile::TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
        let (model_id, token, _) = indexed_partial_ticket(&api).await;
        let model_dir = api
            .primary()
            .model_library
            .library_root()
            .join(model_id.as_str());
        std::fs::write(model_dir.join("unrelated.bin"), b"preserve this artifact").unwrap();
        let tree = model_library::RepoFileTree {
            repo_id: "acme/model".into(),
            lfs_files: vec![model_library::LfsFileInfo {
                filename: "weights.gguf".into(),
                size: 12,
                sha256: "a".repeat(64),
            }],
            regular_files: Vec::new(),
            cached_at: chrono::Utc::now().to_rfc3339(),
            last_modified: None,
            cache_version: 2,
        };
        std::fs::write(
            root.path().join("cache/hf_acme_model_files.json"),
            serde_json::to_vec(&tree).unwrap(),
        )
        .unwrap();
        let (requested, request_received) = tokio::sync::oneshot::channel();
        let mut server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(
                    header.len() < 4096,
                    "fixture request header must be bounded"
                );
                header.push(socket.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            assert!(header.starts_with("GET /acme/model/resolve/main/weights.gguf HTTP/1.1"));
            assert!(header.to_ascii_lowercase().contains("range: bytes=7-"));
            socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 7-11/12\r\nConnection: close\r\n\r\n").await.unwrap();
            requested.send(()).unwrap();
            let mut byte = [0_u8; 1];
            assert_eq!(
                socket.read(&mut byte).await.unwrap(),
                0,
                "cancel must close the owned response"
            );
        });
        let outcome = timeout(Duration::from_secs(10), async {
            let action = api
                .resume_partial_download_with_ticket(&model_id, &token)
                .await
                .unwrap();
            assert_eq!(action.action, "recover");
            let download_id = action
                .download_id
                .expect("accepted recovery has an owned download ID");
            request_received.await.unwrap();
            assert!(api.cancel_hf_download(&download_id).await.unwrap());
            loop {
                let progress = api
                    .primary()
                    .hf_client
                    .as_ref()
                    .unwrap()
                    .get_download_progress(&download_id)
                    .await
                    .unwrap();
                if progress.status == models::DownloadStatus::Cancelled {
                    break;
                }
                assert_eq!(progress.status, models::DownloadStatus::Cancelling);
                tokio::task::yield_now().await;
            }
            assert!(!model_dir.join("weights.gguf.part").exists());
            assert!(!model_dir.join("weights.gguf").exists());
            assert_eq!(
                std::fs::read(model_dir.join("unrelated.bin")).unwrap(),
                b"preserve this artifact"
            );
            (&mut server).await.unwrap();
        })
        .await;
        if outcome.is_err() {
            server.abort();
            let _ = server.await;
        }
        outcome.expect("recovery and public cancellation must settle within the fixture bound");
    }
    use crate::models::HuggingFaceModel;
    use tempfile::TempDir;

    #[test]
    fn test_partial_download_reason_code_preserves_root_contention() {
        assert_eq!(
            partial_download_reason_code(&PumasError::DownloadRootBusy),
            "download_root_busy"
        );
    }

    #[test]
    fn test_partial_download_reason_code_maps_invalid_repo_id() {
        let err = PumasError::Config {
            message: "Invalid repo_id format (expected 'owner/name'): bad".to_string(),
        };
        assert_eq!(partial_download_reason_code(&err), "invalid_repo_id");
    }

    #[test]
    fn test_partial_download_reason_code_maps_post_verification_path_race() {
        let err = PumasError::NotFound {
            resource: "model directory".to_string(),
        };
        assert_eq!(partial_download_reason_code(&err), "dest_dir_missing");
        assert_eq!(
            partial_download_error(&err).reason_code.as_deref(),
            Some("recovery_unavailable")
        );
    }

    #[test]
    fn test_partial_download_reason_code_maps_network_errors() {
        let err = PumasError::Network {
            message: "connection dropped".to_string(),
            cause: None,
        };
        assert_eq!(partial_download_reason_code(&err), "network_error");
    }

    #[test]
    fn test_partial_download_reason_code_maps_hf_404_network_errors_to_repo_not_found() {
        let err = PumasError::Network {
            message: "HuggingFace API returned 404 Not Found".to_string(),
            cause: None,
        };
        assert_eq!(partial_download_reason_code(&err), "repo_not_found");
    }

    #[test]
    fn test_normalized_download_hint_rejects_unknown_values() {
        assert_eq!(normalized_download_hint(Some("unknown")), None);
        assert_eq!(normalized_download_hint(Some("  ")), None);
        assert_eq!(
            normalized_download_hint(Some("text-generation")),
            Some("text-generation")
        );
    }

    #[test]
    fn ranks_exact_bundle_repo_name_ahead_of_base_model_hint() {
        let ranked = rank_bundle_lookup_candidates(
            "tiny-sd-turbo",
            Some("stabilityai/sd-turbo"),
            &[
                hf_model("stabilityai/sd-turbo", "sd-turbo", 10),
                hf_model("cc-nms/tiny-sd-turbo", "tiny-sd-turbo", 1),
            ],
        );

        assert_eq!(ranked[0].repo_id, "cc-nms/tiny-sd-turbo");
    }

    #[test]
    fn exact_bundle_match_normalizes_separator_variants() {
        assert!(is_exact_bundle_lookup_match(
            "tiny-sd-turbo",
            "cc-nms/tiny_sd_turbo",
            "Tiny SD Turbo"
        ));
    }

    #[test]
    fn bundle_lookup_falls_back_to_exact_non_bundle_repo_match() {
        let fallback = fallback_bundle_lookup_candidate(
            "Juggernaut-X-v10",
            None,
            &[
                hf_model("foo/bar", "bar", 100),
                hf_model("RunDiffusion/Juggernaut-X-v10", "Juggernaut-X-v10", 5),
            ],
        )
        .unwrap();

        assert_eq!(fallback.0.repo_id, "RunDiffusion/Juggernaut-X-v10");
        assert_eq!(fallback.1, "filename_exact");
        assert_eq!(fallback.2, 0.84);
    }

    #[test]
    fn bundle_lookup_fallback_rejects_weak_unrelated_results() {
        let fallback = fallback_bundle_lookup_candidate(
            "Juggernaut-X-v10",
            None,
            &[
                hf_model("foo/bar", "bar", 100),
                hf_model("baz/qux", "qux", 50),
            ],
        );

        assert!(fallback.is_none());
    }

    #[test]
    fn bundle_lookup_query_variants_include_spaced_name_once() {
        let queries = bundle_lookup_query_variants("Juggernaut-X_v10");

        assert_eq!(queries[0], "Juggernaut-X_v10");
        assert!(queries.iter().any(|query| query == "Juggernaut X v10"));
        assert_eq!(
            queries
                .iter()
                .filter(|query| query.as_str() == "Juggernaut X v10")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn validate_existing_local_file_lookup_path_canonicalizes_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("weights.gguf");
        std::fs::write(&file_path, b"gguf").unwrap();

        let validated = validate_existing_local_file_lookup_path(
            file_path.to_string_lossy().as_ref(),
            "file_path",
        )
        .await
        .unwrap();

        assert_eq!(validated, file_path.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn validate_existing_local_directory_lookup_path_canonicalizes_existing_directory() {
        let temp_dir = TempDir::new().unwrap();

        let validated = validate_existing_local_directory_lookup_path(
            temp_dir.path().to_string_lossy().as_ref(),
            "dest_dir",
        )
        .await
        .unwrap();

        assert_eq!(validated, temp_dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn validate_existing_local_directory_lookup_path_rejects_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("model_index.json");
        std::fs::write(&file_path, b"{}").unwrap();

        let error = validate_existing_local_directory_lookup_path(
            file_path.to_string_lossy().as_ref(),
            "dir_path",
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("must reference a directory"));
    }

    fn hf_model(repo_id: &str, name: &str, downloads: u64) -> HuggingFaceModel {
        HuggingFaceModel {
            repo_id: repo_id.to_string(),
            name: name.to_string(),
            developer: String::new(),
            kind: "text-to-image".to_string(),
            formats: Vec::new(),
            quants: Vec::new(),
            download_options: Vec::new(),
            url: format!("https://huggingface.co/{}", repo_id),
            release_date: None,
            model_card: None,
            license: None,
            downloads: Some(downloads),
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: Vec::new(),
        }
    }

    const INTENT_TEST_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    struct IntentLifecycleServer {
        preflight_started: Arc<tokio::sync::Notify>,
        release_preflight: Arc<tokio::sync::Notify>,
        payload_started: Arc<tokio::sync::Notify>,
        release_payload: Arc<tokio::sync::Notify>,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        stop: Option<tokio::sync::oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl IntentLifecycleServer {
        async fn stop(mut self) {
            self.release_preflight.notify_waiters();
            self.release_payload.notify_waiters();
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            self.task.await.unwrap();
        }

        fn request_count(&self, suffix: &str) -> usize {
            self.requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.contains(suffix))
                .count()
        }
    }

    fn intent_test_requirement(
        policy: crate::intent::AcquisitionPolicy,
    ) -> crate::intent::ModelRequirement {
        crate::intent::ModelRequirement {
            selector: crate::intent::ModelSelector::UpstreamRepository {
                repository_id: "acme/model".into(),
                revision: None,
            },
            artifact: crate::intent::ArtifactRequirement {
                format: Some(crate::models::PackageArtifactKind::Gguf),
                quantization: None,
                selected_artifact_id: None,
            },
            acquisition_policy: policy,
        }
    }

    fn intent_test_gguf() -> Vec<u8> {
        let mut bytes = b"GGUF".to_vec();
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes
    }

    async fn intent_lifecycle_fixture(
        root: &std::path::Path,
        block_preflight: bool,
    ) -> (PumasApi, IntentLifecycleServer) {
        use sha2::{Digest, Sha256};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let payload = intent_test_gguf();
        let payload_sha = hex::encode(Sha256::digest(&payload));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let preflight_started = Arc::new(tokio::sync::Notify::new());
        let release_preflight = Arc::new(tokio::sync::Notify::new());
        let payload_started = Arc::new(tokio::sync::Notify::new());
        let release_payload = Arc::new(tokio::sync::Notify::new());
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task_preflight_started = preflight_started.clone();
        let task_release_preflight = release_preflight.clone();
        let task_payload_started = payload_started.clone();
        let task_release_payload = release_payload.clone();
        let task_requests = requests.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        let preflight_started = task_preflight_started.clone();
                        let release_preflight = task_release_preflight.clone();
                        let payload_started = task_payload_started.clone();
                        let release_payload = task_release_payload.clone();
                        let requests = task_requests.clone();
                        let payload = payload.clone();
                        let payload_sha = payload_sha.clone();
                        connections.spawn(async move {
                            let mut header = Vec::new();
                            while !header.ends_with(b"\r\n\r\n") {
                                assert!(header.len() < 8192);
                                header.push(socket.read_u8().await.unwrap());
                            }
                            let line = String::from_utf8(header)
                                .unwrap()
                                .lines()
                                .next()
                                .unwrap()
                                .to_string();
                            requests.lock().unwrap().push(line.clone());
                            let body = if line
                                == "GET /api/models/acme/model/revision/main HTTP/1.1"
                            {
                                preflight_started.notify_one();
                                if block_preflight {
                                    release_preflight.notified().await;
                                }
                                format!(
                                    r#"{{"modelId":"acme/model","sha":"{INTENT_TEST_COMMIT}","tags":["gguf"]}}"#
                                )
                            } else if line == format!(
                                "GET /api/models/acme/model/revision/{INTENT_TEST_COMMIT} HTTP/1.1"
                            ) {
                                format!(
                                    r#"{{"modelId":"acme/model","sha":"{INTENT_TEST_COMMIT}","tags":["gguf"]}}"#
                                )
                            } else if line == format!(
                                "GET /api/models/acme/model/tree/{INTENT_TEST_COMMIT}?recursive=true HTTP/1.1"
                            ) {
                                format!(
                                    r#"[{{"path":"model.gguf","type":"file","lfs":{{"oid":"{payload_sha}","size":{}}}}}]"#,
                                    payload.len()
                                )
                            } else if line == format!(
                                "GET /acme/model/resolve/{INTENT_TEST_COMMIT}/model.gguf HTTP/1.1"
                            ) {
                                payload_started.notify_one();
                                release_payload.notified().await;
                                socket
                                    .write_all(
                                        format!(
                                            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                                            payload.len()
                                        )
                                        .as_bytes(),
                                    )
                                    .await
                                    .unwrap();
                                socket.write_all(&payload).await.unwrap();
                                return;
                            } else {
                                panic!("unexpected intent lifecycle request: {line}");
                            };
                            socket
                                .write_all(
                                    format!(
                                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                                        body.len()
                                    )
                                    .as_bytes(),
                                )
                                .await
                                .unwrap();
                        });
                    }
                }
            }
            while let Some(result) = connections.join_next().await {
                result.unwrap();
            }
        });
        let api = recovery_api_fixture(root, Some(format!("http://{address}"))).await;
        (
            api,
            IntentLifecycleServer {
                preflight_started,
                release_preflight,
                payload_started,
                release_payload,
                requests,
                stop: Some(stop),
                task,
            },
        )
    }

    #[tokio::test]
    async fn concurrent_public_intent_gets_coalesce_to_one_owned_download() {
        let root = tempfile::TempDir::new().unwrap();
        let (api, server) = intent_lifecycle_fixture(root.path(), false).await;
        let requirement = intent_test_requirement(crate::intent::AcquisitionPolicy::AllowUpstream);

        let intent = api.intent();
        let (first, second) = tokio::join!(
            intent.get_model(&requirement),
            intent.get_model(&requirement)
        );
        let download_id =
            |observed: crate::Result<crate::intent::ObservedModelState>| match observed.unwrap() {
                crate::intent::ObservedModelState::Acquiring {
                    download_hint: Some(hint),
                    ..
                } => hint.download_id,
                state => panic!("expected acquiring intent state, got {state:?}"),
            };
        let first_id = download_id(first);
        let second_id = download_id(second);
        assert_eq!(first_id, second_id);

        server.release_payload.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let progress = api
                    .get_hf_download_progress(&first_id)
                    .await
                    .unwrap()
                    .expect("owned download remains observable through settlement");
                if progress.status == models::DownloadStatus::Completed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            server.request_count(&format!("/resolve/{INTENT_TEST_COMMIT}/model.gguf")),
            1
        );
        api.shutdown_downloads().await.unwrap();
        server.stop().await;
    }

    #[tokio::test]
    async fn dropped_public_intent_caller_after_admission_does_not_detach_owned_import() {
        let root = tempfile::TempDir::new().unwrap();
        let (api, server) = intent_lifecycle_fixture(root.path(), false).await;
        let api = Arc::new(api);
        let caller_api = api.clone();
        let requirement = intent_test_requirement(crate::intent::AcquisitionPolicy::AllowUpstream);
        let caller = tokio::spawn(async move { caller_api.intent().get_model(&requirement).await });

        server.payload_started.notified().await;
        caller.abort();
        let _ = caller.await;
        let owned = api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .intent_download_snapshot()
            .await
            .downloads
            .into_iter()
            .next()
            .expect("payload request follows durable owned admission");
        let model_ref = owned
            .model_ref
            .clone()
            .expect("owned pinned download exposes stable identity");
        server.release_payload.notify_one();

        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                if api
                    .get_hf_download_progress(&owned.download_id)
                    .await
                    .unwrap()
                    .is_some_and(|progress| progress.status == models::DownloadStatus::Completed)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(api.get_model(&model_ref.model_id).await.unwrap().is_some());
        api.shutdown_downloads().await.unwrap();
        server.stop().await;
    }

    #[tokio::test]
    async fn shutdown_during_public_intent_preflight_waits_for_owned_helper_settlement() {
        let root = tempfile::TempDir::new().unwrap();
        let (api, server) = intent_lifecycle_fixture(root.path(), true).await;
        let api = Arc::new(api);
        let requirement = intent_test_requirement(crate::intent::AcquisitionPolicy::AllowUpstream);
        let caller_api = api.clone();
        let caller = tokio::spawn(async move { caller_api.intent().get_model(&requirement).await });
        server.preflight_started.notified().await;
        let shutdown_api = api.clone();
        let mut shutdown = tokio::spawn(async move { shutdown_api.shutdown_downloads().await });
        tokio::task::yield_now().await;
        assert!(!shutdown.is_finished());

        server.release_preflight.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(10), &mut shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let observed = caller.await.unwrap().unwrap();
        assert!(matches!(
            observed,
            crate::intent::ObservedModelState::Blocked { .. }
                | crate::intent::ObservedModelState::Unavailable { .. }
        ));
        assert_eq!(
            server.request_count(&format!("/resolve/{INTENT_TEST_COMMIT}/model.gguf")),
            0
        );
        server.stop().await;
    }

    #[tokio::test]
    async fn local_only_intent_with_hf_configured_performs_no_http_or_library_writes() {
        let root = tempfile::TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
        let library_root = api.primary().model_library.library_root().to_path_buf();
        let before = std::fs::read_dir(&library_root).unwrap().count();

        let observed = api
            .intent()
            .get_model(&intent_test_requirement(
                crate::intent::AcquisitionPolicy::LocalOnly,
            ))
            .await
            .unwrap();

        assert!(matches!(
            observed,
            crate::intent::ObservedModelState::Missing { .. }
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&library_root).unwrap().count(), before);
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
        api.shutdown_downloads().await.unwrap();
    }

    async fn read_intent_test_request(socket: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;

        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            assert!(header.len() < 8192);
            header.push(socket.read_u8().await.unwrap());
        }
        String::from_utf8(header)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string()
    }

    async fn write_intent_test_response(
        socket: &mut tokio::net::TcpStream,
        status: &str,
        body: &str,
    ) {
        use tokio::io::AsyncWriteExt;

        socket
            .write_all(
                format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn public_intent_projects_upstream_403_and_503_as_typed_unavailable() {
        for status in ["403 Forbidden", "503 Service Unavailable"] {
            let root = tempfile::TempDir::new().unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                assert_eq!(
                    read_intent_test_request(&mut socket).await,
                    "GET /api/models/acme/model/revision/main HTTP/1.1"
                );
                write_intent_test_response(&mut socket, status, "{}").await;
            });
            let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
            let library_root = api.primary().model_library.library_root().to_path_buf();
            let before = std::fs::read_dir(&library_root).unwrap().count();

            let observed = api
                .intent()
                .get_model(&intent_test_requirement(
                    crate::intent::AcquisitionPolicy::AllowUpstream,
                ))
                .await
                .unwrap();

            assert!(matches!(
                observed,
                crate::intent::ObservedModelState::Unavailable { diagnostics }
                    if diagnostics.iter().any(|diagnostic| diagnostic.code
                        == crate::intent::IntentDiagnosticCode::UpstreamResolutionFailed)
            ));
            server.await.unwrap();
            assert!(api
                .primary()
                .hf_client
                .as_ref()
                .unwrap()
                .list_downloads()
                .await
                .is_empty());
            assert_eq!(std::fs::read_dir(&library_root).unwrap().count(), before);
            api.shutdown_downloads().await.unwrap();
        }
    }

    #[tokio::test]
    async fn public_intent_upstream_ambiguity_returns_distinct_candidates_without_admission() {
        let root = tempfile::TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let server_requests = requests.clone();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_intent_test_request(&mut socket).await;
                server_requests.lock().unwrap().push(request.clone());
                if request == "GET /api/models/acme/model/revision/main HTTP/1.1" {
                    write_intent_test_response(
                        &mut socket,
                        "200 OK",
                        &format!(
                            r#"{{"modelId":"acme/model","sha":"{INTENT_TEST_COMMIT}","tags":["gguf"]}}"#
                        ),
                    )
                    .await;
                } else {
                    assert_eq!(
                        request,
                        format!(
                            "GET /api/models/acme/model/tree/{INTENT_TEST_COMMIT}?recursive=true HTTP/1.1"
                        )
                    );
                    write_intent_test_response(
                        &mut socket,
                        "200 OK",
                        &format!(
                            r#"[{{"path":"alpha.gguf","type":"file","lfs":{{"oid":"{}","size":24}}}},{{"path":"beta.gguf","type":"file","lfs":{{"oid":"{}","size":24}}}}]"#,
                            "a".repeat(64),
                            "b".repeat(64)
                        ),
                    )
                    .await;
                }
            }
        });
        let api = recovery_api_fixture(root.path(), Some(format!("http://{address}"))).await;
        let library_root = api.primary().model_library.library_root().to_path_buf();
        let before = std::fs::read_dir(&library_root).unwrap().count();

        let observed = api
            .intent()
            .get_model(&intent_test_requirement(
                crate::intent::AcquisitionPolicy::AllowUpstream,
            ))
            .await
            .unwrap();

        let crate::intent::ObservedModelState::UpstreamAmbiguous {
            candidates,
            diagnostics,
        } = observed
        else {
            panic!("expected public upstream ambiguity outcome");
        };
        assert_eq!(candidates.len(), 2);
        assert_ne!(
            candidates[0].selected_artifact_id,
            candidates[1].selected_artifact_id
        );
        assert_ne!(candidates[0].filenames, candidates[1].filenames);
        assert!(candidates
            .iter()
            .all(|candidate| candidate.revision == INTENT_TEST_COMMIT));
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code
            == crate::intent::IntentDiagnosticCode::UpstreamArtifactAmbiguous));
        server.await.unwrap();
        assert_eq!(requests.lock().unwrap().len(), 2);
        assert!(requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| !request.contains("/resolve/")));
        assert!(api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .is_empty());
        assert_eq!(std::fs::read_dir(&library_root).unwrap().count(), before);
        api.shutdown_downloads().await.unwrap();
    }
}
