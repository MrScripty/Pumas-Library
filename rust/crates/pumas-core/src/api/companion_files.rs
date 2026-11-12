//! Companion files of library models: the stored model card, license and
//! code (a read projection for display), and the backfill that fetches the
//! ones a model is missing.
//!
//! The backfill never fetches weights and never writes identity fields
//! (`selected_artifact_id`, `selected_artifact_files`, `expected_files`,
//! `upstream_revision`) or the model directory name.

use crate::model_library::companion_files::{
    record_companions, scan_companions, CompanionInventory, CompanionKind, CompanionSource,
};
use crate::model_library::{
    place_companions, CompanionBackfillTarget, CompanionPlanOutcome, LocalWeight, ModelLibrary,
};
use crate::models::{
    CompanionBackfillFile, CompanionBackfillModelReport, CompanionBackfillOutcome,
    CompanionBackfillReason, CompanionBackfillReport, MetadataArtifactRef, ModelCodeFile,
    ModelDocument, ModelDocumentsResponse, ModelMetadata, StorageKind,
};
use crate::{PumasApi, PumasError, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl PumasApi {
    /// The model card, license documents and code stored with a library
    /// model, read from its directory. Code is listed for inspection only.
    pub async fn get_model_documents(&self, model_id: &str) -> Result<ModelDocumentsResponse> {
        let primary = self.try_primary()?;
        let library = primary.model_library.clone();
        let model_dir = existing_model_dir(&library, model_id).await?;
        let model_id = model_id.to_string();
        tokio::task::spawn_blocking(move || model_documents(&library, &model_id, &model_dir))
            .await
            .map_err(|error| PumasError::Other(format!("Model document read failed: {error}")))?
    }

    /// Fetch the model card, license, code and metadata files one library
    /// model is missing, at the model's recorded revision. With `dry_run`,
    /// report what would be fetched without fetching or writing anything.
    pub async fn backfill_model_companion_files(
        &self,
        model_id: &str,
        dry_run: bool,
    ) -> Result<CompanionBackfillReport> {
        let primary = self.try_primary()?;
        existing_model_dir(&primary.model_library, model_id).await?;
        let report = backfill_one(
            primary.model_library.clone(),
            primary.hf_client.clone(),
            model_id,
            dry_run,
        )
        .await;
        Ok(CompanionBackfillReport::from_models(dry_run, vec![report]))
    }

    /// Backfill every library model in turn. Models that cannot be backfilled
    /// are reported as skipped with a reason; one failure does not stop the rest.
    pub async fn backfill_library_companion_files(
        &self,
        dry_run: bool,
    ) -> Result<CompanionBackfillReport> {
        let primary = self.try_primary()?;
        let mut model_ids = primary
            .model_library
            .list_models()
            .await?
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>();
        model_ids.sort();
        let mut reports = Vec::with_capacity(model_ids.len());
        for model_id in model_ids {
            reports.push(
                backfill_one(
                    primary.model_library.clone(),
                    primary.hf_client.clone(),
                    &model_id,
                    dry_run,
                )
                .await,
            );
        }
        Ok(CompanionBackfillReport::from_models(dry_run, reports))
    }
}

async fn existing_model_dir(library: &Arc<ModelLibrary>, model_id: &str) -> Result<PathBuf> {
    if library.get_model(model_id).await?.is_none() {
        return Err(PumasError::ModelNotFound {
            model_id: model_id.to_string(),
        });
    }
    Ok(library.library_root().join(model_id))
}

fn read_document(
    model_dir: &Path,
    path: &str,
    size_bytes: u64,
    recorded: Option<&MetadataArtifactRef>,
) -> Result<ModelDocument> {
    let bytes = std::fs::read(model_dir.join(path))?;
    let recorded = recorded.filter(|artifact| artifact.path.as_deref() == Some(path));
    Ok(ModelDocument {
        path: path.to_string(),
        size_bytes,
        sha256: recorded.and_then(|artifact| artifact.sha256.clone()),
        source_url: recorded.and_then(|artifact| artifact.source_url.clone()),
        content: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

fn model_documents(
    library: &ModelLibrary,
    model_id: &str,
    model_dir: &Path,
) -> Result<ModelDocumentsResponse> {
    let metadata = library.load_metadata(model_dir)?.unwrap_or_default();
    let inventory = scan_companions(model_dir)?;
    let model_card = inventory
        .model_card()
        .map(|card| {
            read_document(
                model_dir,
                &card.path,
                card.size_bytes,
                metadata.model_card_artifact.as_ref(),
            )
        })
        .transpose()?;
    let primary_license = inventory.license().map(|license| license.path.clone());
    let mut licenses = inventory
        .of_kind(CompanionKind::License)
        .map(|license| {
            read_document(
                model_dir,
                &license.path,
                license.size_bytes,
                metadata.license_artifact.as_ref(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    licenses.sort_by_key(|license| Some(&license.path) != primary_license.as_ref());
    let mut custom_code_sources = metadata
        .custom_code_sources
        .clone()
        .unwrap_or_default()
        .into_iter()
        .chain(inventory.auto_map_modules.iter().cloned())
        .chain(inventory.external_auto_map_references.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    custom_code_sources.sort();
    Ok(ModelDocumentsResponse {
        model_id: model_id.to_string(),
        model_card,
        licenses,
        code_files: inventory
            .of_kind(CompanionKind::Code)
            .map(|file| ModelCodeFile {
                path: file.path.clone(),
                size_bytes: file.size_bytes,
            })
            .collect(),
        requires_custom_code: metadata.requires_custom_code == Some(true)
            || !inventory.auto_map_modules.is_empty()
            || !inventory.external_auto_map_references.is_empty(),
        custom_code_sources,
        backfill_available: metadata.repo_id.is_some()
            && metadata.storage_kind != Some(StorageKind::ExternalReference),
    })
}

fn report(model_id: &str, outcome: CompanionBackfillOutcome) -> CompanionBackfillModelReport {
    CompanionBackfillModelReport {
        model_id: model_id.to_string(),
        outcome,
        reason: None,
        message: None,
        repo_id: None,
        revision: None,
        revision_binding: None,
        files: Vec::new(),
    }
}

fn refused(
    mut report: CompanionBackfillModelReport,
    outcome: CompanionBackfillOutcome,
    reason: CompanionBackfillReason,
    message: impl Into<String>,
) -> CompanionBackfillModelReport {
    report.outcome = outcome;
    report.reason = Some(reason);
    report.message = Some(message.into());
    report
}

/// What the backfill reads from one model directory. Blocking.
struct LocalModelState {
    metadata: ModelMetadata,
    weights: Vec<LocalWeight>,
    present: BTreeSet<String>,
}

fn relative_files(model_dir: &Path) -> Result<Vec<(String, u64)>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(model_dir).follow_links(false) {
        let entry = entry
            .map_err(|error| PumasError::Other(format!("Model directory scan failed: {error}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(relative) = entry
            .path()
            .strip_prefix(model_dir)
            .ok()
            .and_then(|relative| {
                relative
                    .components()
                    .map(|component| component.as_os_str().to_str())
                    .collect::<Option<Vec<_>>>()
            })
            .map(|components| components.join("/"))
        else {
            continue;
        };
        let size = entry
            .metadata()
            .map_err(|error| PumasError::Other(format!("Model file stat failed: {error}")))?
            .len();
        files.push((relative, size));
    }
    Ok(files)
}

/// The stored weights that scope the backfill, with recorded digests. The
/// whole-model SHA-256 in `hashes` describes the primary (largest) weight.
fn local_weights(metadata: &ModelMetadata, files: &[(String, u64)]) -> Vec<LocalWeight> {
    let selected = metadata
        .selected_artifact_files
        .clone()
        .or_else(|| metadata.expected_files.clone())
        .map(|paths| paths.into_iter().collect::<BTreeSet<_>>());
    let mut weights = files
        .iter()
        .filter(|(path, _)| crate::model_library::sharding::is_weight_file(path))
        .filter(|(path, _)| {
            selected
                .as_ref()
                .is_none_or(|selected| selected.contains(path))
        })
        .map(|(path, size)| LocalWeight {
            path: path.clone(),
            size: *size,
            sha256: metadata
                .files
                .as_ref()
                .and_then(|entries| entries.iter().find(|entry| entry.name == *path))
                .and_then(|entry| entry.sha256.clone())
                .filter(|sha256| !sha256.is_empty()),
        })
        .collect::<Vec<_>>();
    let whole_model_sha256 = metadata
        .hashes
        .as_ref()
        .and_then(|hashes| hashes.sha256.clone())
        .filter(|sha256| !sha256.is_empty());
    if let Some(sha256) = whole_model_sha256 {
        if let Some(primary) = weights
            .iter_mut()
            .max_by(|left, right| left.size.cmp(&right.size).then(right.path.cmp(&left.path)))
        {
            primary.sha256.get_or_insert(sha256);
        }
    }
    weights.sort_by(|left, right| left.path.cmp(&right.path));
    weights
}

fn read_local_state(library: &ModelLibrary, model_dir: &Path) -> Result<Option<LocalModelState>> {
    let Some(metadata) = library.load_metadata(model_dir)? else {
        return Ok(None);
    };
    let files = relative_files(model_dir)?;
    let weights = local_weights(&metadata, &files);
    Ok(Some(LocalModelState {
        metadata,
        weights,
        present: files.into_iter().map(|(path, _)| path).collect(),
    }))
}

fn selects_whole_repository(metadata: &ModelMetadata) -> bool {
    metadata
        .selected_artifact_id
        .as_deref()
        .is_some_and(|id| id.contains("__full_repo") || id.contains("__bundle_"))
}

async fn backfill_one(
    library: Arc<ModelLibrary>,
    client: Option<Arc<crate::model_library::HuggingFaceClient>>,
    model_id: &str,
    dry_run: bool,
) -> CompanionBackfillModelReport {
    let base = report(model_id, CompanionBackfillOutcome::Skipped);
    let model_dir = library.library_root().join(model_id);
    let state = {
        let library = library.clone();
        let model_dir = model_dir.clone();
        tokio::task::spawn_blocking(move || read_local_state(&library, &model_dir)).await
    };
    let state = match state {
        Ok(Ok(Some(state))) => state,
        Ok(Ok(None)) => {
            return refused(
                base,
                CompanionBackfillOutcome::Failed,
                CompanionBackfillReason::MetadataUnavailable,
                "The model has no metadata.json",
            )
        }
        Ok(Err(error)) => {
            return refused(
                base,
                CompanionBackfillOutcome::Failed,
                CompanionBackfillReason::MetadataUnavailable,
                error.to_string(),
            )
        }
        Err(error) => {
            return refused(
                base,
                CompanionBackfillOutcome::Failed,
                CompanionBackfillReason::MetadataUnavailable,
                error.to_string(),
            )
        }
    };
    let Some(repo_id) = state.metadata.repo_id.clone() else {
        return refused(
            base,
            CompanionBackfillOutcome::Skipped,
            CompanionBackfillReason::NoRepoId,
            "The model records no Hugging Face repository (a local import)",
        );
    };
    let mut base = base;
    base.repo_id = Some(repo_id.clone());
    if state.metadata.storage_kind == Some(StorageKind::ExternalReference) {
        return refused(
            base,
            CompanionBackfillOutcome::Skipped,
            CompanionBackfillReason::NotLibraryOwned,
            "The model's files live outside the library",
        );
    }
    let Some(client) = client else {
        return refused(
            base,
            CompanionBackfillOutcome::Failed,
            CompanionBackfillReason::HubUnavailable,
            "The Hugging Face client is not configured",
        );
    };
    let target = CompanionBackfillTarget {
        repo_id: repo_id.clone(),
        recorded_revision: state.metadata.upstream_revision.clone(),
        whole_repository: selects_whole_repository(&state.metadata),
        weights: state.weights,
        present: state.present,
    };
    let plan = match client.plan_companion_backfill(&target).await {
        Ok(CompanionPlanOutcome::Ready(plan)) => plan,
        Ok(CompanionPlanOutcome::Skipped { reason, message }) => {
            return refused(base, CompanionBackfillOutcome::Skipped, reason, message)
        }
        Err(error) => {
            return refused(
                base,
                CompanionBackfillOutcome::Failed,
                CompanionBackfillReason::HubUnavailable,
                error.to_string(),
            )
        }
    };
    base.revision = Some(plan.revision.as_str().to_string());
    base.revision_binding = Some(plan.binding);
    let planned_files = plan
        .files
        .iter()
        .map(|file| CompanionBackfillFile {
            path: file.path.clone(),
            kind: file.kind.as_str().to_string(),
            size_bytes: file.size,
        })
        .collect::<Vec<_>>();
    if dry_run {
        base.outcome = if planned_files.is_empty() {
            CompanionBackfillOutcome::UpToDate
        } else {
            CompanionBackfillOutcome::Planned
        };
        base.files = planned_files;
        return base;
    }

    let source = CompanionSource {
        repo_id,
        revision: plan.revision.as_str().to_string(),
    };
    match fetch_and_record(&library, &client, model_id, &model_dir, &plan, source).await {
        Ok(placed) => {
            base.outcome = if placed.is_empty() {
                CompanionBackfillOutcome::UpToDate
            } else {
                CompanionBackfillOutcome::Fetched
            };
            base.files = planned_files
                .into_iter()
                .filter(|file| placed.contains(&file.path))
                .collect();
            base
        }
        Err(error) => {
            let (outcome, reason) = match &error {
                PumasError::DownloadRootBusy => (
                    CompanionBackfillOutcome::Skipped,
                    CompanionBackfillReason::ModelBusy,
                ),
                PumasError::Validation { field, .. } if field == "model_library.mutation" => (
                    CompanionBackfillOutcome::Skipped,
                    CompanionBackfillReason::ModelBusy,
                ),
                PumasError::Validation { field, .. } if field == "companion_files.integrity" => (
                    CompanionBackfillOutcome::Failed,
                    CompanionBackfillReason::IntegrityMismatch,
                ),
                _ => (
                    CompanionBackfillOutcome::Failed,
                    CompanionBackfillReason::HubUnavailable,
                ),
            };
            refused(base, outcome, reason, error.to_string())
        }
    }
}

/// Fetch and place the planned files under the model's mutation guard, then
/// record the stored companions in its metadata and reindex it.
async fn fetch_and_record(
    library: &Arc<ModelLibrary>,
    client: &crate::model_library::HuggingFaceClient,
    model_id: &str,
    model_dir: &Path,
    plan: &crate::model_library::CompanionFetchPlan,
    source: CompanionSource,
) -> Result<BTreeSet<String>> {
    let authority = library.mutation_authority()?;
    let target = [(model_id.to_string(), model_dir.to_path_buf())];
    let mut guard = tokio::task::spawn_blocking(move || authority.protect_metadata(&target))
        .await
        .map_err(|error| PumasError::Other(format!("Model guard failed: {error}")))??;
    guard.mark_started();

    let place_dir = model_dir.to_path_buf();
    let placed = client
        .fetch_companions(
            plan,
            model_id,
            library.library_root(),
            move |files| async move {
                tokio::task::spawn_blocking(move || place_companions(&place_dir, files))
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Companion placement failed: {error}"))
                    })?
            },
        )
        .await?;

    let record_dir = model_dir.to_path_buf();
    let record_library = library.clone();
    let updated = tokio::task::spawn_blocking(move || -> Result<Option<ModelMetadata>> {
        let Some(mut metadata) = record_library.load_metadata(&record_dir)? else {
            return Ok(None);
        };
        let inventory: CompanionInventory = scan_companions(&record_dir)?;
        let changed = record_companions(&record_dir, &mut metadata, &inventory, Some(&source))?;
        Ok(changed.then_some(metadata))
    })
    .await
    .map_err(|error| PumasError::Other(format!("Companion record failed: {error}")))??;
    if let Some(metadata) = updated {
        library.save_metadata(model_dir, &metadata).await?;
        library.index_model_dir(model_dir).await?;
    }
    guard.finish_success()?;
    Ok(placed.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelFileInfo, ModelHashes};

    #[test]
    fn local_weights_follow_the_selection_and_attribute_the_whole_model_digest() {
        let metadata = ModelMetadata {
            selected_artifact_files: Some(vec![
                "config.json".into(),
                "model-00001-of-00002.safetensors".into(),
                "model-00002-of-00002.safetensors".into(),
            ]),
            hashes: Some(ModelHashes {
                sha256: Some("f".repeat(64)),
                ..Default::default()
            }),
            files: Some(vec![ModelFileInfo {
                name: "model-00002-of-00002.safetensors".into(),
                original_name: None,
                size: Some(5),
                sha256: Some("e".repeat(64)),
                blake3: None,
            }]),
            ..Default::default()
        };
        let files = [
            ("config.json".to_string(), 10),
            ("model-00001-of-00002.safetensors".to_string(), 50),
            ("model-00002-of-00002.safetensors".to_string(), 5),
            ("other.gguf".to_string(), 99),
        ];
        let weights = local_weights(&metadata, &files);
        assert_eq!(
            weights,
            [
                LocalWeight {
                    path: "model-00001-of-00002.safetensors".into(),
                    size: 50,
                    sha256: Some("f".repeat(64)),
                },
                LocalWeight {
                    path: "model-00002-of-00002.safetensors".into(),
                    size: 5,
                    sha256: Some("e".repeat(64)),
                },
            ]
        );
    }
}

#[cfg(test)]
#[path = "companion_files_tests.rs"]
mod backfill_tests;
